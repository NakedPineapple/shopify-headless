//! Email routing after classification.
//!
//! Determines the action to take based on the classification result:
//! - Archive spam/newsletters
//! - Queue draft responses for Slack review
//! - Route complaints/praise to Klaviyo Helpdesk
//! - Send Slack notifications for business/vendor emails

use naked_pineapple_services::claude::{ClaudeClient, ClaudeError};
use naked_pineapple_services::klaviyo::{KlaviyoClient, KlaviyoError};
use naked_pineapple_services::slack::SlackClient;
use naked_pineapple_support::db::conversation::ConversationRepository;
use naked_pineapple_support::models::CreateConversationParams;
use sqlx::PgPool;
use tracing::{debug, error, info, instrument};

use crate::db::inbound_email;
use crate::shopify::ShopifyClient;
use crate::slack::messages as slack_messages;
use crate::triage::responder::{self, DraftResponse, ResponseContext};
use crate::triage::types::{ClassificationResult, EmailClassification, EmailStatus};
use naked_pineapple_services::microsoft_graph::M365Client;

/// Parameters for routing a classified email.
pub struct RouteParams<'a> {
    /// Database ID of the `inbound_email` record.
    pub email_id: i32,
    /// Mailbox the email was received on.
    pub mailbox: &'a str,
    /// M365 message ID.
    pub m365_message_id: &'a str,
    /// Sender address.
    pub from_address: &'a str,
    /// Sender name.
    pub from_name: Option<&'a str>,
    /// Email subject.
    pub subject: &'a str,
    /// Email body text.
    pub body: &'a str,
    /// Classification result.
    pub classification: &'a ClassificationResult,
    /// Conversation ID for threading checks.
    pub conversation_id: &'a str,
    /// Storefront support pool (optional — for creating support conversations from email).
    pub support_pool: Option<&'a PgPool>,
}

/// Route a classified email to the appropriate destination.
///
/// # Errors
///
/// Returns `TriageError` if any routing operation fails.
#[instrument(
    skip(pool, m365, claude, slack, route_params),
    fields(
        email_id = route_params.email_id,
        classification = %route_params.classification.classification
    )
)]
pub async fn route_email(
    pool: &PgPool,
    m365: &M365Client,
    claude: &ClaudeClient,
    slack: Option<&SlackClient>,
    klaviyo: Option<&KlaviyoClient>,
    shopify: Option<&ShopifyClient>,
    route_params: &RouteParams<'_>,
) -> Result<(), TriageError> {
    let classification = route_params.classification;

    // Check threading safety: if this conversation already has a sent response, skip auto-reply
    let has_prior_response =
        inbound_email::has_sent_response_in_thread(pool, route_params.conversation_id).await?;

    if has_prior_response && classification.classification.needs_draft_response() {
        info!(
            conversation_id = %route_params.conversation_id,
            "thread already has auto-reply, routing to notification only"
        );
        return route_notify_only(pool, slack, route_params).await;
    }

    if classification.classification.is_archivable() {
        return route_archive(pool, m365, route_params).await;
    }

    if classification.classification.needs_draft_response() {
        return route_draft_response(pool, claude, slack, shopify, route_params).await;
    }

    if classification.classification.routes_to_helpdesk() {
        return route_to_helpdesk(pool, slack, klaviyo, route_params).await;
    }

    // BusinessVendor: Slack notification only
    route_notify_only(pool, slack, route_params).await
}

/// Archive the email (spam/marketing).
async fn route_archive(
    pool: &PgPool,
    m365: &M365Client,
    params: &RouteParams<'_>,
) -> Result<(), TriageError> {
    debug!(email_id = params.email_id, "archiving email");

    // Archive in M365
    if let Err(e) = m365.archive(params.mailbox, params.m365_message_id).await {
        error!(error = %e, "failed to archive in M365, continuing with DB update");
    }

    // Update DB status
    inbound_email::update_status(
        pool,
        params.email_id,
        EmailStatus::Archived,
        Some("archive"),
    )
    .await?;

    info!(email_id = params.email_id, "email archived");

    Ok(())
}

/// Queue a draft response for Slack review.
async fn route_draft_response(
    pool: &PgPool,
    claude: &ClaudeClient,
    slack: Option<&SlackClient>,
    shopify: Option<&ShopifyClient>,
    params: &RouteParams<'_>,
) -> Result<(), TriageError> {
    debug!(email_id = params.email_id, "composing draft response");

    let shopify_context = fetch_shopify_context(shopify, params).await;

    let response_context = ResponseContext {
        from_address: params.from_address.to_string(),
        from_name: params.from_name.map(String::from),
        subject: params.subject.to_string(),
        body: params.body.to_string(),
        classification: params.classification.clone(),
        shopify_context,
    };

    let draft = responder::compose_draft_response(claude, &response_context).await?;

    // Save draft to database
    inbound_email::save_response_draft(pool, params.email_id, &draft.body_text).await?;
    inbound_email::update_status(
        pool,
        params.email_id,
        EmailStatus::PendingReview,
        Some("slack_review"),
    )
    .await?;

    // Send Slack review message
    if let Some(slack) = slack {
        send_review_message(slack, params, &draft).await;
    }

    // Create support conversation in storefront DB for unified admin inbox
    if let Some(sp) = params.support_pool {
        create_support_conversation(sp, params).await;
    }

    info!(
        email_id = params.email_id,
        "draft response queued for Slack review"
    );

    Ok(())
}

/// Route to Klaviyo Helpdesk and send Slack notification.
async fn route_to_helpdesk(
    pool: &PgPool,
    slack: Option<&SlackClient>,
    klaviyo: Option<&KlaviyoClient>,
    params: &RouteParams<'_>,
) -> Result<(), TriageError> {
    debug!(email_id = params.email_id, "routing to helpdesk");

    // Track event in Klaviyo for helpdesk ticket creation
    if let Some(klaviyo) = klaviyo {
        let helpdesk_params = naked_pineapple_services::klaviyo::HelpdeskEventParams {
            email: params.from_address,
            customer_name: params
                .classification
                .extracted_entities
                .customer_name
                .as_deref(),
            subject: params.subject,
            classification: params.classification.classification.label(),
            reasoning: &params.classification.reasoning,
            email_id: params.email_id,
        };
        if let Err(e) = klaviyo.track_helpdesk_event(&helpdesk_params).await {
            error!(error = %e, "failed to track helpdesk event in Klaviyo");
        }
    }

    inbound_email::update_status(
        pool,
        params.email_id,
        EmailStatus::Routed,
        Some("klaviyo_helpdesk"),
    )
    .await?;

    // Send Slack notification
    if let Some(slack) = slack {
        send_notification_message(slack, params).await;
    }

    info!(email_id = params.email_id, "email routed to helpdesk");

    Ok(())
}

/// Send a Slack notification without routing elsewhere.
async fn route_notify_only(
    pool: &PgPool,
    slack: Option<&SlackClient>,
    params: &RouteParams<'_>,
) -> Result<(), TriageError> {
    debug!(email_id = params.email_id, "sending notification only");

    inbound_email::update_status(pool, params.email_id, EmailStatus::Routed, Some("notified"))
        .await?;

    if let Some(slack) = slack {
        send_notification_message(slack, params).await;
    }

    Ok(())
}

/// Send a Slack review message with approve/reject buttons.
async fn send_review_message(slack: &SlackClient, params: &RouteParams<'_>, draft: &DraftResponse) {
    let blocks = slack_messages::build_email_review_message(
        params.email_id,
        params.from_address,
        params.subject,
        params.classification.classification,
        &draft.body_text,
    );

    let channel = slack.default_channel();
    let fallback = format!(
        "Email review: {} from {} - {}",
        params.subject, params.from_address, params.classification.classification
    );

    if let Err(e) = slack.post_message(channel, blocks, Some(&fallback)).await {
        error!(error = %e, "failed to send Slack review message");
    }
}

/// Send a Slack notification about an email (no review buttons).
async fn send_notification_message(slack: &SlackClient, params: &RouteParams<'_>) {
    let blocks = slack_messages::build_email_notification_message(
        params.from_address,
        params.subject,
        params.classification.classification,
        &params.classification.reasoning,
    );

    let channel = slack.default_channel();
    let fallback = format!(
        "Email notification: {} from {} - {}",
        params.subject, params.from_address, params.classification.classification
    );

    if let Err(e) = slack.post_message(channel, blocks, Some(&fallback)).await {
        error!(error = %e, "failed to send Slack notification");
    }
}

/// Fetch Shopify context (orders, products) based on the classification and extracted entities.
async fn fetch_shopify_context(
    shopify: Option<&ShopifyClient>,
    params: &RouteParams<'_>,
) -> Option<String> {
    let client = shopify?;
    let entities = &params.classification.extracted_entities;
    let classification = params.classification.classification;

    let mut context_parts: Vec<String> = Vec::new();

    // Look up orders for order-related classifications
    let needs_orders = matches!(
        classification,
        EmailClassification::OrderInquiry
            | EmailClassification::ReturnRequest
            | EmailClassification::ShippingIssue
    );

    if needs_orders {
        // Look up by extracted order numbers
        for order_number in &entities.order_numbers {
            match crate::shopify::orders::lookup_by_number(client, order_number).await {
                Ok(orders) if !orders.is_empty() => {
                    context_parts.push(crate::shopify::orders::format_orders_for_prompt(&orders));
                }
                Ok(_) => {}
                Err(e) => {
                    debug!(error = %e, "Shopify order lookup failed, continuing without");
                }
            }
        }

        // If no order numbers extracted, try looking up by sender email
        if entities.order_numbers.is_empty() {
            match crate::shopify::orders::lookup_by_email(client, params.from_address).await {
                Ok(orders) if !orders.is_empty() => {
                    context_parts.push(crate::shopify::orders::format_orders_for_prompt(&orders));
                }
                Ok(_) => {}
                Err(e) => {
                    debug!(error = %e, "Shopify order-by-email lookup failed, continuing without");
                }
            }
        }
    }

    // Look up products for product-related classifications
    if classification == EmailClassification::ProductQuestion {
        for product_name in &entities.product_names {
            match crate::shopify::products::search(client, product_name).await {
                Ok(products) if !products.is_empty() => {
                    context_parts.push(crate::shopify::products::format_products_for_prompt(
                        &products,
                    ));
                }
                Ok(_) => {}
                Err(e) => {
                    debug!(error = %e, "Shopify product search failed, continuing without");
                }
            }
        }
    }

    if context_parts.is_empty() {
        None
    } else {
        Some(context_parts.join("\n"))
    }
}

/// Create a support conversation in the storefront DB so the email appears in the admin inbox.
async fn create_support_conversation(support_pool: &PgPool, params: &RouteParams<'_>) {
    let repo = ConversationRepository::new(support_pool);
    let conv_params = CreateConversationParams {
        session_token: format!("email-{}", params.email_id),
        shopify_customer_id: None,
        customer_email: Some(params.from_address.to_string()),
        customer_name: params.from_name.map(String::from),
        is_authenticated: false,
        source: Some("email".to_string()),
    };

    match repo.create(&conv_params).await {
        Ok(conv) => {
            info!(
                email_id = params.email_id,
                conversation_id = conv.id.as_i32(),
                "support conversation created from email"
            );
        }
        Err(e) => {
            error!(
                email_id = params.email_id,
                error = %e,
                "failed to create support conversation from email"
            );
        }
    }
}

/// Errors that can occur during email routing.
#[derive(Debug, thiserror::Error)]
pub enum TriageError {
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] crate::db::RepositoryError),

    /// Claude AI error during response composition.
    #[error("AI error: {0}")]
    Claude(#[from] ClaudeError),

    /// Microsoft Graph API error.
    #[error("M365 error: {0}")]
    M365(#[from] naked_pineapple_services::microsoft_graph::M365Error),

    /// Klaviyo API error.
    #[error("Klaviyo error: {0}")]
    Klaviyo(#[from] KlaviyoError),
}
