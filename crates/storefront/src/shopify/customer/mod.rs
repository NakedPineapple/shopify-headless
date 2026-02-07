//! Shopify Customer Account API client.
//!
//! The Customer Account API provides access to customer authentication and
//! account management. Uses OAuth 2.0 with PKCE for authentication.
//!
//! # OAuth Flow
//!
//! 1. Generate authorization URL with `authorization_url()`
//! 2. Redirect customer to Shopify's login page
//! 3. Shopify redirects back with authorization code
//! 4. Exchange code for tokens with `exchange_code()`
//! 5. Use access token for customer-scoped API calls

mod conversions;
mod queries;
mod types;

pub use types::*;

use std::sync::Arc;

use graphql_client::{GraphQLQuery, Response};
use secrecy::ExposeSecret;
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use crate::config::ShopifyStorefrontConfig;
use crate::shopify::ShopifyError;

use conversions::{
    convert_activate_subscription, convert_address_create, convert_address_delete,
    convert_address_update, convert_cancel_subscription, convert_customer_update,
    convert_get_addresses, convert_get_customer, convert_get_order, convert_get_order_for_return,
    convert_get_orders, convert_get_store_credit, convert_get_subscription,
    convert_get_subscriptions, convert_get_upcoming_billing_cycles, convert_order_request_return,
    convert_pause_subscription, convert_skip_billing_cycle, convert_unskip_billing_cycle,
};
use queries::{
    ActivateSubscription, CancelSubscription, CustomerAddressCreate, CustomerAddressDelete,
    CustomerAddressUpdate, CustomerUpdate, GetAddresses, GetCustomer, GetOrder, GetOrderForReturn,
    GetOrders, GetStoreCredit, GetSubscription, GetSubscriptions, GetUpcomingBillingCycles,
    OrderRequestReturn, PauseSubscription, SkipBillingCycle, UnskipBillingCycle,
};

// ─────────────────────────────────────────────────────────────────────────────
// Customer Account Client
// ─────────────────────────────────────────────────────────────────────────────

/// Client for the Shopify Customer Account API.
///
/// This client handles OAuth authentication and provides methods for
/// accessing customer data, orders, and addresses.
#[derive(Clone)]
pub struct CustomerClient {
    inner: Arc<CustomerClientInner>,
}

struct CustomerClientInner {
    client: reqwest::Client,
    store: String,
    store_id: String,
    api_version: String,
    client_id: String,
    client_secret: String,
}

impl CustomerClient {
    /// Create a new Customer Account API client.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built (invalid TLS backend).
    #[must_use]
    pub fn new(config: &ShopifyStorefrontConfig) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("NakedPineapple/1.0")
            .build()
            .expect("reqwest client with default user-agent builds successfully");

        Self {
            inner: Arc::new(CustomerClientInner {
                client,
                store: config.store.clone(),
                store_id: config.customer_shop_id.clone(),
                api_version: config.api_version.clone(),
                client_id: config.customer_client_id.clone(),
                client_secret: config.customer_client_secret.expose_secret().to_string(),
            }),
        }
    }

    /// Get the store domain.
    #[must_use]
    pub fn store(&self) -> &str {
        &self.inner.store
    }

    /// Get the OAuth client ID (safe to expose in frontend).
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.inner.client_id
    }

    // ─────────────────────────────────────────────────────────────────────────
    // OAuth Flow
    // ─────────────────────────────────────────────────────────────────────────

    /// Generate the authorization URL for customer login.
    ///
    /// Redirect customers to this URL to begin the OAuth flow.
    ///
    /// # Arguments
    ///
    /// * `redirect_uri` - The callback URL to redirect to after authentication
    /// * `state` - A random string stored in the session to prevent CSRF attacks
    /// * `nonce` - A random string for `OpenID` Connect replay protection
    ///
    /// # Returns
    ///
    /// The full authorization URL to redirect the customer to.
    #[must_use]
    pub fn authorization_url(&self, redirect_uri: &str, state: &str, nonce: &str) -> String {
        // Shopify Customer Account API accepts exactly three scope values.
        // Granular permissions (customer_read_orders, etc.) are configured
        // at the app level in Shopify admin, not in the OAuth request.
        let scopes = [
            "openid",
            "email",
            "https://api.customers.com/auth/customer.graphql",
        ]
        .join(" ");

        format!(
            "https://shopify.com/{store}/auth/oauth/authorize?\
            client_id={client_id}&\
            response_type=code&\
            redirect_uri={redirect_uri}&\
            scope={scopes}&\
            state={state}&\
            nonce={nonce}",
            store = self.inner.store_id,
            client_id = urlencoding::encode(&self.inner.client_id),
            redirect_uri = urlencoding::encode(redirect_uri),
            scopes = urlencoding::encode(&scopes),
            state = urlencoding::encode(state),
            nonce = urlencoding::encode(nonce),
        )
    }

    /// Generate the logout URL.
    ///
    /// # Arguments
    ///
    /// * `id_token` - The ID token from the current session
    /// * `post_logout_redirect_uri` - Where to redirect after logout
    ///
    /// # Returns
    ///
    /// The full logout URL to redirect the customer to.
    #[must_use]
    pub fn logout_url(&self, id_token: &str, post_logout_redirect_uri: &str) -> String {
        format!(
            "https://shopify.com/{}/auth/oauth/logout?\
            id_token_hint={}&\
            post_logout_redirect_uri={}",
            self.inner.store_id,
            urlencoding::encode(id_token),
            urlencoding::encode(post_logout_redirect_uri)
        )
    }

    /// Exchange an authorization code for access tokens.
    ///
    /// # Errors
    ///
    /// Returns an error if the token exchange fails.
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<CustomerAccessToken, ShopifyError> {
        let url = format!(
            "https://shopify.com/{}/auth/oauth/token",
            self.inner.store_id
        );

        let params = [
            ("grant_type", "authorization_code"),
            ("client_id", &self.inner.client_id),
            ("client_secret", &self.inner.client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
        ];

        let response = self.inner.client.post(&url).form(&params).send().await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ShopifyError::OAuth(format!(
                "Token exchange failed: {text}"
            )));
        }

        let token_response: TokenResponse = response.json().await?;

        Ok(CustomerAccessToken {
            access_token: token_response.access_token,
            id_token: token_response.id_token,
            refresh_token: token_response.refresh_token,
            expires_in: token_response.expires_in,
            obtained_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Refresh an access token using a refresh token.
    ///
    /// # Errors
    ///
    /// Returns an error if the token refresh fails.
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<CustomerAccessToken, ShopifyError> {
        let url = format!(
            "https://shopify.com/{}/auth/oauth/token",
            self.inner.store_id
        );

        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", &self.inner.client_id),
            ("client_secret", &self.inner.client_secret),
            ("refresh_token", refresh_token),
        ];

        let response = self.inner.client.post(&url).form(&params).send().await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ShopifyError::OAuth(format!("Token refresh failed: {text}")));
        }

        let token_response: TokenResponse = response.json().await?;

        Ok(CustomerAccessToken {
            access_token: token_response.access_token,
            id_token: token_response.id_token,
            refresh_token: token_response.refresh_token,
            expires_in: token_response.expires_in,
            obtained_at: chrono::Utc::now().timestamp(),
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // GraphQL Execution
    // ─────────────────────────────────────────────────────────────────────────

    /// Execute a GraphQL query against the Customer Account API.
    #[instrument(skip(self, access_token, variables))]
    async fn execute<Q: GraphQLQuery>(
        &self,
        access_token: &str,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, ShopifyError>
    where
        Q::Variables: serde::Serialize,
    {
        let query_name = std::any::type_name::<Q>()
            .split("::")
            .last()
            .unwrap_or("Unknown");
        debug!(query = %query_name, "Executing Customer Account GraphQL query");

        let start = std::time::Instant::now();
        let url = format!(
            "https://shopify.com/{}/account/customer/api/{}/graphql",
            self.inner.store_id, self.inner.api_version
        );

        let request_body = Q::build_query(variables);

        let response = self
            .inner
            .client
            .post(&url)
            .header("Authorization", access_token)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            let body_preview: String = response_text.chars().take(500).collect();
            warn!(
                query = %query_name,
                status = %status,
                body = %body_preview,
                duration_ms = %start.elapsed().as_millis(),
                "Customer Account API returned non-success status"
            );
            return Err(ShopifyError::OAuth(format!(
                "Customer API request failed ({status}): {}",
                response_text.chars().take(200).collect::<String>()
            )));
        }

        let gql_response: Response<Q::ResponseData> = serde_json::from_str(&response_text)
            .map_err(|e| {
                let body_preview: String = response_text.chars().take(500).collect();
                warn!(
                    query = %query_name,
                    error = %e,
                    body = %body_preview,
                    duration_ms = %start.elapsed().as_millis(),
                    "Failed to parse Customer Account GraphQL response"
                );
                ShopifyError::Parse(e)
            })?;

        if let Some(errors) = gql_response.errors
            && !errors.is_empty()
        {
            let error_messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            warn!(
                query = %query_name,
                errors = ?error_messages,
                duration_ms = %start.elapsed().as_millis(),
                "GraphQL errors in Customer Account API response"
            );
            let messages = error_messages.join("; ");
            return Err(ShopifyError::OAuth(messages));
        }

        debug!(
            query = %query_name,
            duration_ms = %start.elapsed().as_millis(),
            "Customer Account GraphQL query completed successfully"
        );

        gql_response
            .data
            .ok_or_else(|| ShopifyError::OAuth("No data in response".to_string()))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Customer Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the current customer's information.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_customer(&self, access_token: &str) -> Result<Customer, ShopifyError> {
        let data = self
            .execute::<GetCustomer>(access_token, queries::get_customer::Variables)
            .await?;
        Ok(convert_get_customer(data))
    }

    /// Update the current customer's information.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or if there are validation errors.
    pub async fn update_customer(
        &self,
        access_token: &str,
        input: CustomerUpdateInput,
    ) -> Result<Customer, ShopifyError> {
        let variables = queries::customer_update::Variables {
            input: queries::customer_update::CustomerUpdateInput {
                first_name: input.first_name,
                last_name: input.last_name,
            },
        };
        let data = self
            .execute::<CustomerUpdate>(access_token, variables)
            .await?;
        convert_customer_update(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Order Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the customer's order history.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_orders(
        &self,
        access_token: &str,
        first: u32,
    ) -> Result<Vec<Order>, ShopifyError> {
        let variables = queries::get_orders::Variables {
            first: i64::from(first),
        };
        let data = self.execute::<GetOrders>(access_token, variables).await?;
        Ok(convert_get_orders(&data))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Address Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the customer's addresses.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_addresses(
        &self,
        access_token: &str,
        first: u32,
    ) -> Result<Vec<Address>, ShopifyError> {
        let variables = queries::get_addresses::Variables {
            first: i64::from(first),
        };
        let data = self
            .execute::<GetAddresses>(access_token, variables)
            .await?;
        Ok(convert_get_addresses(&data))
    }

    /// Create a new address for the customer.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or if there are validation errors.
    pub async fn create_address(
        &self,
        access_token: &str,
        address: AddressInput,
    ) -> Result<Address, ShopifyError> {
        let variables = queries::customer_address_create::Variables {
            address: to_gql_address_input_create(address),
        };
        let data = self
            .execute::<CustomerAddressCreate>(access_token, variables)
            .await?;
        convert_address_create(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    /// Update an existing address.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or if there are validation errors.
    pub async fn update_address(
        &self,
        access_token: &str,
        address_id: &str,
        address: AddressInput,
    ) -> Result<Address, ShopifyError> {
        let variables = queries::customer_address_update::Variables {
            address_id: address_id.to_string(),
            address: to_gql_address_input_update(address),
        };
        let data = self
            .execute::<CustomerAddressUpdate>(access_token, variables)
            .await?;
        convert_address_update(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    /// Delete an address.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or if there are validation errors.
    pub async fn delete_address(
        &self,
        access_token: &str,
        address_id: &str,
    ) -> Result<(), ShopifyError> {
        let variables = queries::customer_address_delete::Variables {
            address_id: address_id.to_string(),
        };
        let data = self
            .execute::<CustomerAddressDelete>(access_token, variables)
            .await?;
        convert_address_delete(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Subscription Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the customer's subscription contracts.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_subscriptions(
        &self,
        access_token: &str,
        first: u32,
    ) -> Result<Vec<SubscriptionContract>, ShopifyError> {
        let variables = queries::get_subscriptions::Variables {
            first: i64::from(first),
        };
        let data = self
            .execute::<GetSubscriptions>(access_token, variables)
            .await?;
        Ok(convert_get_subscriptions(&data))
    }

    /// Get a specific subscription contract by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_subscription(
        &self,
        access_token: &str,
        id: &str,
    ) -> Result<Option<SubscriptionContract>, ShopifyError> {
        let variables = queries::get_subscription::Variables { id: id.to_string() };
        let data = self
            .execute::<GetSubscription>(access_token, variables)
            .await?;
        Ok(convert_get_subscription(&data))
    }

    /// Pause a subscription contract.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutation fails or returns user errors.
    pub async fn pause_subscription(
        &self,
        access_token: &str,
        id: &str,
    ) -> Result<(), ShopifyError> {
        let variables = queries::pause_subscription::Variables { id: id.to_string() };
        let data = self
            .execute::<PauseSubscription>(access_token, variables)
            .await?;
        convert_pause_subscription(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    /// Cancel a subscription contract.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutation fails or returns user errors.
    pub async fn cancel_subscription(
        &self,
        access_token: &str,
        id: &str,
    ) -> Result<(), ShopifyError> {
        let variables = queries::cancel_subscription::Variables { id: id.to_string() };
        let data = self
            .execute::<CancelSubscription>(access_token, variables)
            .await?;
        convert_cancel_subscription(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    /// Activate (resume) a paused subscription contract.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutation fails or returns user errors.
    pub async fn activate_subscription(
        &self,
        access_token: &str,
        id: &str,
    ) -> Result<(), ShopifyError> {
        let variables = queries::activate_subscription::Variables { id: id.to_string() };
        let data = self
            .execute::<ActivateSubscription>(access_token, variables)
            .await?;
        convert_activate_subscription(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Order Detail Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get a single order with full details.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_order(
        &self,
        access_token: &str,
        id: &str,
    ) -> Result<Option<OrderDetail>, ShopifyError> {
        let variables = queries::get_order::Variables { id: id.to_string() };
        let data = self.execute::<GetOrder>(access_token, variables).await?;
        Ok(convert_get_order(data))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Return Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get an order's line items with suggested return reasons.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_order_for_return(
        &self,
        access_token: &str,
        order_id: &str,
    ) -> Result<Option<OrderForReturn>, ShopifyError> {
        let variables = queries::get_order_for_return::Variables {
            id: order_id.to_string(),
        };
        let data = self
            .execute::<GetOrderForReturn>(access_token, variables)
            .await?;
        Ok(convert_get_order_for_return(data))
    }

    /// Request a return on an order.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutation fails or returns user errors.
    pub async fn request_return(
        &self,
        access_token: &str,
        order_id: &str,
        items: &[ReturnRequestLineItemInput],
    ) -> Result<(), ShopifyError> {
        let gql_items: Vec<_> = items
            .iter()
            .map(
                |item| queries::order_request_return::RequestedLineItemInput {
                    line_item_id: item.line_item_id.clone(),
                    quantity: i64::from(item.quantity),
                    return_reason_definition_id: item.reason_id.clone(),
                    customer_note: item.note.clone(),
                },
            )
            .collect();

        let variables = queries::order_request_return::Variables {
            order_id: order_id.to_string(),
            items: gql_items,
        };
        let data = self
            .execute::<OrderRequestReturn>(access_token, variables)
            .await?;
        convert_order_request_return(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Store Credit Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the customer's store credit balance.
    ///
    /// Returns `None` if no store credit account exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_store_credit(
        &self,
        access_token: &str,
    ) -> Result<Option<crate::shopify::types::Money>, ShopifyError> {
        let data = self
            .execute::<GetStoreCredit>(access_token, queries::get_store_credit::Variables)
            .await?;
        Ok(convert_get_store_credit(&data))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Billing Cycle Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get upcoming billing cycles for a subscription contract.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_upcoming_billing_cycles(
        &self,
        access_token: &str,
        contract_id: &str,
        first: u32,
    ) -> Result<Vec<SubscriptionBillingCycle>, ShopifyError> {
        let variables = queries::get_upcoming_billing_cycles::Variables {
            id: contract_id.to_string(),
            first: i64::from(first),
        };
        let data = self
            .execute::<GetUpcomingBillingCycles>(access_token, variables)
            .await?;
        Ok(convert_get_upcoming_billing_cycles(data))
    }

    /// Skip a billing cycle.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutation fails or returns user errors.
    pub async fn skip_billing_cycle(
        &self,
        access_token: &str,
        contract_id: &str,
        cycle_index: i64,
    ) -> Result<(), ShopifyError> {
        let variables = queries::skip_billing_cycle::Variables {
            input: queries::skip_billing_cycle::SubscriptionBillingCycleInput {
                contract_id: contract_id.to_string(),
                selector: queries::skip_billing_cycle::SubscriptionBillingCycleSelector {
                    index: Some(cycle_index),
                    date: None,
                },
            },
        };
        let data = self
            .execute::<SkipBillingCycle>(access_token, variables)
            .await?;
        convert_skip_billing_cycle(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }

    /// Unskip a billing cycle.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutation fails or returns user errors.
    pub async fn unskip_billing_cycle(
        &self,
        access_token: &str,
        contract_id: &str,
        cycle_index: i64,
    ) -> Result<(), ShopifyError> {
        let variables = queries::unskip_billing_cycle::Variables {
            input: queries::unskip_billing_cycle::SubscriptionBillingCycleInput {
                contract_id: contract_id.to_string(),
                selector: queries::unskip_billing_cycle::SubscriptionBillingCycleSelector {
                    index: Some(cycle_index),
                    date: None,
                },
            },
        };
        let data = self
            .execute::<UnskipBillingCycle>(access_token, variables)
            .await?;
        convert_unskip_billing_cycle(data).map_err(|e| ShopifyError::UserError(e.join(", ")))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Input Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn to_gql_address_input_create(
    input: AddressInput,
) -> queries::customer_address_create::CustomerAddressInput {
    queries::customer_address_create::CustomerAddressInput {
        first_name: input.first_name,
        last_name: input.last_name,
        company: input.company,
        address1: input.address1,
        address2: input.address2,
        city: input.city,
        zone_code: input.province,
        territory_code: input.country,
        zip: input.zip,
        phone_number: input.phone,
    }
}

fn to_gql_address_input_update(
    input: AddressInput,
) -> queries::customer_address_update::CustomerAddressInput {
    queries::customer_address_update::CustomerAddressInput {
        first_name: input.first_name,
        last_name: input.last_name,
        company: input.company,
        address1: input.address1,
        address2: input.address2,
        city: input.city,
        zone_code: input.province,
        territory_code: input.country,
        zip: input.zip,
        phone_number: input.phone,
    }
}
