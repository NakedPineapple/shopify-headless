//! Support route handlers: chat API, customer hub, FAQ, and ticket history.
//!
//! # Routes
//!
//! ```text
//! # Customer Hub Pages
//! GET  /support                            -- Support hub (public)
//! GET  /support/faq                        -- FAQ category index (public)
//! GET  /support/faq/{category}             -- FAQ category detail (public)
//! GET  /support/tickets                    -- Conversation history (authenticated)
//! GET  /support/tickets/{id}              -- Conversation detail (authenticated)
//!
//! # Chat API
//! POST /support/chat                      -- Start or resume conversation (Turnstile verified)
//! POST /support/chat/{id}/messages/stream -- Send message, SSE streaming response
//! GET  /support/chat/{id}/messages        -- Get conversation history (JSON)
//! ```

use std::collections::HashSet;
use std::convert::Infallible;
use std::fmt::Write;
use std::future::Future;
use std::pin::Pin;

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures::StreamExt;
use naked_pineapple_core::{SupportConversationId, SupportConversationStatus, SupportMessageRole};
use naked_pineapple_support::{
    db::conversation::ConversationRepository,
    db::message::MessageRepository,
    models::{ChatStreamEvent, CreateConversationParams, SupportConversation, SupportMessage},
    service::{SupportChatService, SupportChatServiceParams},
    tools::ToolContext,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use tracing::{debug, error, info, warn};

use crate::config::{AnalyticsConfig, AnalyticsUserInfo};
use crate::content::{ContentStore, Page};
use crate::filters;
use crate::middleware::{OptionalShopifyCustomer, RequireShopifyCustomer};
use crate::search::SearchIndex;
use crate::shopify::customer::{CustomerClient, Order, SubscriptionContract};
use crate::shopify::{Product, StorefrontClient};
use crate::state::AppState;

const SYSTEM_PROMPT: &str = include_str!("../../templates/claude/support_system_prompt.txt");

// =============================================================================
// Request / Response Types
// =============================================================================

/// Request to start or resume a conversation.
#[derive(Debug, Deserialize)]
pub struct StartChatRequest {
    /// Cloudflare Turnstile token for bot verification.
    #[serde(rename = "cf-turnstile-response")]
    pub turnstile_token: String,
}

/// Request to send a message.
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
}

/// Response for conversation creation/resumption.
#[derive(Debug, Serialize)]
pub struct ConversationResponse {
    pub id: i32,
    pub status: String,
    pub is_new: bool,
}

/// Response for a single message.
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: i32,
    pub role: String,
    pub content: serde_json::Value,
    pub created_at: String,
}

/// Error response body.
#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorBody {
            error: message.to_string(),
        }),
    )
        .into_response()
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Start or resume a support conversation.
///
/// POST /support/chat
///
/// Requires a valid Turnstile token. Creates a new conversation or resumes
/// an existing active one for the session.
pub async fn start_chat(
    State(state): State<AppState>,
    session: Session,
    OptionalShopifyCustomer(customer): OptionalShopifyCustomer,
    Json(request): Json<StartChatRequest>,
) -> Response {
    // Verify chat is enabled
    let Some(turnstile_secret) = &state.config().turnstile_secret_key else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "Chat is not available");
    };

    // Verify Turnstile token
    if let Err(e) =
        crate::middleware::verify_turnstile_token(turnstile_secret, &request.turnstile_token).await
    {
        warn!(error = %e, "Turnstile verification failed");
        return error_response(StatusCode::FORBIDDEN, "Bot verification failed");
    }

    let Some(session_id) = session.id() else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "No session");
    };
    let session_token = session_id.to_string();
    let is_authenticated = customer.is_some();

    let conv_repo = ConversationRepository::new(state.pool());

    // Try to resume an existing active conversation
    match conv_repo.find_active_by_session(&session_token).await {
        Ok(Some(conv)) => {
            debug!(
                conversation_id = conv.id.as_i32(),
                "Resuming existing conversation"
            );
            return (
                StatusCode::OK,
                Json(ConversationResponse {
                    id: conv.id.as_i32(),
                    status: format!("{:?}", conv.status).to_lowercase(),
                    is_new: false,
                }),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            error!(error = %e, "Failed to look up existing conversation");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong");
        }
    }

    // Extract customer identity from the id_token JWT claims
    let (shopify_customer_id, customer_email, customer_name) = customer
        .as_ref()
        .and_then(|c| c.id_token.as_deref())
        .and_then(decode_id_token_claims)
        .map_or((None, None, None), |claims| {
            (claims.sub, claims.email, claims.name)
        });

    // Create a new conversation
    let params = CreateConversationParams {
        session_token,
        shopify_customer_id,
        customer_email,
        customer_name,
        is_authenticated,
        source: None,
    };

    match conv_repo.create(&params).await {
        Ok(conv) => {
            info!(
                conversation_id = conv.id.as_i32(),
                "Created new support conversation"
            );
            (
                StatusCode::CREATED,
                Json(ConversationResponse {
                    id: conv.id.as_i32(),
                    status: format!("{:?}", conv.status).to_lowercase(),
                    is_new: true,
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to create conversation");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong")
        }
    }
}

/// Send a message and stream the AI response via SSE.
///
/// POST /support/chat/{id}/messages/stream
pub async fn send_message_stream(
    State(state): State<AppState>,
    session: Session,
    OptionalShopifyCustomer(customer): OptionalShopifyCustomer,
    Path(id): Path<i32>,
    Json(request): Json<SendMessageRequest>,
) -> Response {
    let (Some(claude), Some(embedding)) = (state.claude(), state.embedding()) else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "Chat is not available");
    };

    let conversation_id = SupportConversationId::new(id);
    let Some(session_id) = session.id() else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "No session");
    };
    let session_token = session_id.to_string();

    // Verify conversation ownership
    let conv_repo = ConversationRepository::new(state.pool());
    let Ok(conversation) = conv_repo.get_by_id(conversation_id).await else {
        return error_response(StatusCode::NOT_FOUND, "Conversation not found");
    };

    if conversation.session_token != session_token {
        warn!(
            conversation_id = id,
            "Session token mismatch — possible ownership violation"
        );
        return error_response(StatusCode::FORBIDDEN, "Access denied");
    }

    // Reject messages on closed/resolved conversations
    if matches!(
        conversation.status,
        SupportConversationStatus::Closed | SupportConversationStatus::Resolved
    ) {
        return error_response(StatusCode::GONE, "This conversation has been closed");
    }

    let access_token = customer.as_ref().map(|c| c.access_token.clone());
    let tool_context = SupportToolContext {
        search_index: state.search().clone(),
        content_store: state.content().clone(),
        storefront: state.storefront().clone(),
        customer: state.customer().clone(),
        access_token,
    };

    let sse_stream = stream_support_chat(ChatStreamParams {
        claude: claude.clone(),
        embedding: embedding.clone(),
        pool: state.pool().clone(),
        conversation_id,
        message: request.message,
        tool_context,
    });

    Sse::new(sse_stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Parameters for the SSE chat stream.
///
/// Bundles all owned dependencies that the `'static` stream needs.
struct ChatStreamParams {
    claude: naked_pineapple_services::claude::ClaudeClient,
    embedding: naked_pineapple_services::openai::EmbeddingClient,
    pool: sqlx::PgPool,
    conversation_id: SupportConversationId,
    message: String,
    tool_context: SupportToolContext,
}

/// Create a `'static` SSE stream that owns all dependencies.
fn stream_support_chat(
    params: ChatStreamParams,
) -> impl futures::Stream<Item = Result<Event, Infallible>> + Send {
    async_stream::stream! {
        let service = SupportChatService::new(SupportChatServiceParams {
            claude: &params.claude,
            embedding: &params.embedding,
            pool: &params.pool,
            system_prompt: SYSTEM_PROMPT,
            is_authenticated: params.tool_context.access_token.is_some(),
            tool_context: &params.tool_context,
            slack: None,
        });

        let event_stream = service.send_message_streaming(
            params.conversation_id, params.message,
        );
        let mut event_stream = std::pin::pin!(event_stream);

        while let Some(result) = event_stream.next().await {
            let event = match result {
                Ok(e) => e,
                Err(e) => {
                    error!(error = %e, "Support chat stream error");
                    ChatStreamEvent::Error {
                        message: "Something went wrong. Please try again.".to_string(),
                    }
                }
            };
            let json = serde_json::to_string(&event).unwrap_or_else(|_| {
                r#"{"type":"error","message":"Failed to serialize event"}"#.to_string()
            });
            yield Ok(Event::default().data(json));
        }
    }
}

// =============================================================================
// Tool Context Implementation
// =============================================================================

/// Provides real Shopify API access and content search for support tools.
///
/// Owns cloned (Arc-wrapped) clients so it can be moved into the `'static`
/// SSE stream alongside the `SupportChatService`.
struct SupportToolContext {
    search_index: SearchIndex,
    content_store: ContentStore,
    storefront: StorefrontClient,
    customer: CustomerClient,
    access_token: Option<String>,
}

impl ToolContext for SupportToolContext {
    fn search_content(&self, query: &str, limit: usize) -> String {
        let results = match self.search_index.search(query, limit) {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "Content search failed");
                return String::new();
            }
        };

        let mut output = String::new();
        for result in &results.pages {
            if let Some(page) = self.content_store.get_page(&result.handle) {
                let _ = write!(
                    output,
                    "### {} (Page)\n{}\n\n",
                    page.meta.title,
                    strip_html_tags(&page.content_html),
                );
            }
        }
        for result in &results.articles {
            if let Some(post) = self.content_store.get_post(&result.handle) {
                let _ = write!(
                    output,
                    "### {} (Article)\n{}\n\n",
                    post.meta.title,
                    strip_html_tags(&post.content_html),
                );
            }
        }
        output.truncate(output.trim_end().len());
        output
    }

    fn lookup_product<'a>(
        &'a self,
        query: &'a str,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>> {
        Box::pin(async move {
            // Try exact handle lookup first
            if let Ok(product) = self.storefront.get_product_by_handle(query).await {
                return format_product(&product);
            }
            // Fall back to search
            match self
                .storefront
                .get_products(Some(3), None, Some(query.to_string()), None, None)
                .await
            {
                Ok(conn) if conn.products.is_empty() => {
                    format!(
                        "No products found matching '{query}'. \
                         Please try a different search term."
                    )
                }
                Ok(ref conn) if conn.products.len() == 1 => {
                    // SAFETY: length is checked by the guard
                    conn.products
                        .first()
                        .map_or_else(String::new, format_product)
                }
                Ok(conn) => {
                    let mut out = format!(
                        "Found {} products matching '{query}':\n\n",
                        conn.products.len()
                    );
                    for p in &conn.products {
                        out.push_str(&format_product_summary(p));
                        out.push('\n');
                    }
                    out
                }
                Err(e) => {
                    warn!(error = %e, "Product lookup failed");
                    "I'm having trouble looking up product information right now. \
                     Please try again in a moment."
                        .to_string()
                }
            }
        })
    }

    fn lookup_order<'a>(
        &'a self,
        order_number: &'a str,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>> {
        Box::pin(async move {
            let Some(ref token) = self.access_token else {
                return "Unable to look up orders — no customer session.".to_string();
            };
            let orders = match self.customer.get_orders(token, 20).await {
                Ok(o) => o,
                Err(e) => {
                    warn!(error = %e, "Order lookup failed");
                    return "I'm having trouble looking up order information \
                            right now. Please try again in a moment."
                        .to_string();
                }
            };
            let normalized = order_number.trim_start_matches('#');
            let matching = orders.iter().find(|o| {
                o.name.trim_start_matches('#') == normalized || o.number.to_string() == normalized
            });
            matching.map_or_else(
                || {
                    format!(
                        "No order found matching '{order_number}'. \
                         Please double-check the order number and try again."
                    )
                },
                format_order,
            )
        })
    }

    fn lookup_subscription<'a>(
        &'a self,
        subscription_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>> {
        Box::pin(async move {
            let Some(ref token) = self.access_token else {
                return "Unable to look up subscriptions — no customer session.".to_string();
            };
            if subscription_id == "all" {
                match self.customer.get_subscriptions(token, 10).await {
                    Ok(subs) if subs.is_empty() => {
                        "No active subscriptions found for this account.".to_string()
                    }
                    Ok(subs) => {
                        let mut out = format!("Found {} subscription(s):\n\n", subs.len());
                        for sub in &subs {
                            out.push_str(&format_subscription(sub));
                            out.push('\n');
                        }
                        out
                    }
                    Err(e) => {
                        warn!(error = %e, "Subscription listing failed");
                        "I'm having trouble looking up subscription \
                         information right now. Please try again."
                            .to_string()
                    }
                }
            } else {
                match self.customer.get_subscription(token, subscription_id).await {
                    Ok(Some(sub)) => format_subscription(&sub),
                    Ok(None) => format!("No subscription found with ID '{subscription_id}'."),
                    Err(e) => {
                        warn!(error = %e, "Subscription lookup failed");
                        "I'm having trouble looking up subscription \
                         information right now. Please try again."
                            .to_string()
                    }
                }
            }
        })
    }
}

// =============================================================================
// Formatting Helpers
// =============================================================================

fn format_product(product: &Product) -> String {
    let price = &product.price_range.min_variant_price;
    let availability = if product.available_for_sale {
        "In stock"
    } else {
        "Out of stock"
    };

    let mut out = format!(
        "**{}**\nPrice: {} {}\nAvailability: {}\nDescription: {}\n\
         URL: /products/{}",
        product.title,
        price.amount,
        price.currency_code,
        availability,
        product.description,
        product.handle,
    );
    if let Some(ref ingredients) = product.ingredients {
        let _ = write!(out, "\nIngredients: {ingredients}");
    }
    out
}

fn format_product_summary(product: &Product) -> String {
    let price = &product.price_range.min_variant_price;
    let availability = if product.available_for_sale {
        "In stock"
    } else {
        "Out of stock"
    };
    format!(
        "- **{}** — {} {} ({})",
        product.title, price.amount, price.currency_code, availability,
    )
}

fn format_order(order: &Order) -> String {
    let financial = order.financial_status.as_deref().unwrap_or("unknown");
    let fulfillment = order.fulfillment_status.as_deref().unwrap_or("unfulfilled");
    format!(
        "**Order {}**\nDate: {}\nPayment: {}\n\
         Fulfillment: {}\nTotal: {} {}",
        order.name,
        order.processed_at,
        financial,
        fulfillment,
        order.total_price.amount,
        order.total_price.currency_code,
    )
}

fn format_subscription(sub: &SubscriptionContract) -> String {
    let items: Vec<String> = sub
        .lines
        .iter()
        .map(|l| format!("{} (x{})", l.name, l.quantity))
        .collect();
    let next_billing = sub.next_billing_date.as_deref().unwrap_or("N/A");
    format!(
        "**Subscription**\nStatus: {}\nFrequency: {}\n\
         Items: {}\nNext billing: {}",
        sub.status.label(),
        sub.billing_policy.frequency_label(),
        items.join(", "),
        next_billing,
    )
}

/// Strip all HTML tags, returning plain text.
fn strip_html_tags(html: &str) -> String {
    ammonia::Builder::new()
        .tags(HashSet::new())
        .clean(html)
        .to_string()
}

// =============================================================================
// JWT Helpers
// =============================================================================

/// OIDC claims from Shopify's `id_token` JWT.
#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    sub: Option<String>,
    email: Option<String>,
    name: Option<String>,
}

/// Decode OIDC claims from a JWT `id_token` without signature verification.
///
/// The token was already verified during the OAuth exchange. We only need
/// the payload claims for customer enrichment.
fn decode_id_token_claims(id_token: &str) -> Option<IdTokenClaims> {
    let payload = id_token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes)
        .inspect_err(|e| warn!(error = %e, "Failed to parse id_token claims"))
        .ok()
}

// =============================================================================
// Route Handlers (continued)
// =============================================================================

/// Get conversation message history.
///
/// GET /support/chat/{id}/messages
pub async fn get_messages(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    let conversation_id = SupportConversationId::new(id);
    let Some(session_id) = session.id() else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "No session");
    };
    let session_token = session_id.to_string();

    // Verify conversation ownership
    let conv_repo = ConversationRepository::new(state.pool());
    let Ok(conversation) = conv_repo.get_by_id(conversation_id).await else {
        return error_response(StatusCode::NOT_FOUND, "Conversation not found");
    };

    if conversation.session_token != session_token {
        return error_response(StatusCode::FORBIDDEN, "Access denied");
    }

    let msg_repo = MessageRepository::new(state.pool());
    match msg_repo.list_by_conversation(conversation_id).await {
        Ok(messages) => {
            let responses: Vec<MessageResponse> = messages
                .into_iter()
                .map(|m| MessageResponse {
                    id: m.id.as_i32(),
                    role: format!("{:?}", m.role).to_lowercase(),
                    content: m.content,
                    created_at: m.created_at.to_rfc3339(),
                })
                .collect();
            Json(responses).into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to fetch messages");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong")
        }
    }
}

// =============================================================================
// Customer Hub: Templates
// =============================================================================

/// View model for messages in ticket detail template.
pub struct MessageView {
    pub role: SupportMessageRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
    text: String,
}

impl MessageView {
    fn from_message(msg: &SupportMessage) -> Self {
        let text = msg
            .content
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Self {
            role: msg.role,
            created_at: msg.created_at,
            text,
        }
    }

    /// Get the text content of this message.
    #[must_use]
    pub fn content_text(&self) -> &str {
        &self.text
    }
}

/// Support hub page template.
#[derive(Template, WebTemplate)]
#[template(path = "support/hub.html")]
pub struct HubTemplate {
    pub faq_pages: Vec<Page>,
    pub conversations: Option<Vec<SupportConversation>>,
    pub chat_enabled: bool,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// FAQ index page template.
#[derive(Template, WebTemplate)]
#[template(path = "support/faq.html")]
pub struct FaqIndexTemplate {
    pub faq_pages: Vec<Page>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// FAQ category detail page template.
#[derive(Template, WebTemplate)]
#[template(path = "support/faq_category.html")]
pub struct FaqCategoryTemplate {
    pub title: String,
    pub description: String,
    pub content_html: String,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Customer ticket history page template.
#[derive(Template, WebTemplate)]
#[template(path = "support/tickets.html")]
pub struct TicketsTemplate {
    pub conversations: Vec<SupportConversation>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Ticket detail page template.
#[derive(Template, WebTemplate)]
#[template(path = "support/ticket_detail.html")]
pub struct TicketDetailTemplate {
    pub conversation: SupportConversation,
    pub messages: Vec<MessageView>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

// =============================================================================
// Customer Hub: Route Handlers
// =============================================================================

/// Get sorted FAQ pages from the content store.
fn sorted_faq_pages(state: &AppState) -> Vec<Page> {
    let mut pages: Vec<Page> = state.content().get_all_support_pages().cloned().collect();
    pages.sort_by(|a, b| a.meta.title.cmp(&b.meta.title));
    pages
}

/// Support hub page.
///
/// GET /support
pub async fn hub(
    State(state): State<AppState>,
    OptionalShopifyCustomer(customer): OptionalShopifyCustomer,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    let customer_id = customer
        .as_ref()
        .and_then(|c| c.id_token.as_deref())
        .and_then(decode_id_token_claims)
        .and_then(|claims| claims.sub);

    let conversations = match customer_id {
        Some(ref id) => {
            let conv_repo = ConversationRepository::new(state.pool());
            Some(conv_repo.list_by_customer(id, 5).await.unwrap_or_default())
        }
        None => None,
    };

    HubTemplate {
        faq_pages: sorted_faq_pages(&state),
        conversations,
        chat_enabled: site.chat_enabled,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        site,
        nonce,
    }
}

/// FAQ index page.
///
/// GET /support/faq
pub async fn faq_index(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    FaqIndexTemplate {
        faq_pages: sorted_faq_pages(&state),
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        site,
        nonce,
    }
}

/// FAQ category detail page.
///
/// GET /support/faq/{category}
///
/// # Errors
///
/// Returns 404 if the category doesn't exist.
pub async fn faq_category(
    State(state): State<AppState>,
    Path(category): Path<String>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Result<impl IntoResponse, StatusCode> {
    let page = state
        .content()
        .get_support_page(&category)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(FaqCategoryTemplate {
        title: page.meta.title.clone(),
        description: page.meta.description.clone().unwrap_or_default(),
        content_html: page.content_html.clone(),
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        site,
        nonce,
    })
}

/// Customer ticket history page.
///
/// GET /support/tickets
pub async fn tickets(
    State(state): State<AppState>,
    RequireShopifyCustomer(customer): RequireShopifyCustomer,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    let customer_id = customer
        .id_token
        .as_deref()
        .and_then(decode_id_token_claims)
        .and_then(|claims| claims.sub);

    let conversations = match customer_id {
        Some(ref id) => {
            let conv_repo = ConversationRepository::new(state.pool());
            conv_repo.list_by_customer(id, 50).await.unwrap_or_default()
        }
        None => Vec::new(),
    };

    TicketsTemplate {
        conversations,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        site,
        nonce,
    }
}

/// Ticket detail (conversation thread) page.
///
/// GET /support/tickets/{id}
///
/// # Errors
///
/// Returns 404 if not found or 403 if the conversation doesn't belong to the customer.
pub async fn ticket_detail(
    State(state): State<AppState>,
    RequireShopifyCustomer(customer): RequireShopifyCustomer,
    Path(id): Path<i32>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Result<impl IntoResponse, StatusCode> {
    let customer_id = customer
        .id_token
        .as_deref()
        .and_then(decode_id_token_claims)
        .and_then(|claims| claims.sub)
        .ok_or(StatusCode::FORBIDDEN)?;

    let conversation_id = SupportConversationId::new(id);
    let conv_repo = ConversationRepository::new(state.pool());

    let conversation = conv_repo
        .get_by_id(conversation_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Verify the conversation belongs to this customer
    if conversation.shopify_customer_id.as_deref() != Some(&customer_id) {
        return Err(StatusCode::FORBIDDEN);
    }

    let msg_repo = MessageRepository::new(state.pool());
    let raw_messages = msg_repo
        .list_by_conversation(conversation_id)
        .await
        .unwrap_or_default();

    let messages: Vec<MessageView> = raw_messages.iter().map(MessageView::from_message).collect();

    Ok(TicketDetailTemplate {
        conversation,
        messages,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        site,
        nonce,
    })
}
