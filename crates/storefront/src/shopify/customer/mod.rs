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
//!
//! # Example
//!
//! ```rust,ignore
//! use naked_pineapple_storefront::shopify::CustomerClient;
//!
//! // Create client
//! let client = CustomerClient::new(&config.shopify);
//!
//! // Generate login URL
//! let state = generate_random_state();
//! let nonce = generate_random_nonce();
//! let auth_url = client.authorization_url("https://example.com/callback", &state, &nonce);
//!
//! // After OAuth callback, exchange code for token
//! let token = client.exchange_code(&code, "https://example.com/callback").await?;
//!
//! // Use token for API calls
//! let customer = client.get_customer(&token.access_token).await?;
//! ```

mod types;

pub use types::*;

use std::sync::Arc;

use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::config::ShopifyStorefrontConfig;
use crate::shopify::ShopifyError;

// ─────────────────────────────────────────────────────────────────────────────
// GraphQL Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct GraphQLRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLErrorResponse>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLErrorResponse {
    message: String,
}

impl<T> GraphQLResponse<T> {
    fn into_result(self) -> Result<T, ShopifyError> {
        if let Some(errors) = self.errors
            && !errors.is_empty()
        {
            let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            return Err(ShopifyError::OAuth(messages.join("; ")));
        }

        self.data
            .ok_or_else(|| ShopifyError::OAuth("No data in response".to_string()))
    }
}

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
    /// # Arguments
    ///
    /// * `code` - The authorization code from the OAuth callback
    /// * `redirect_uri` - The same redirect URI used in the authorization request
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
    /// # Arguments
    ///
    /// * `refresh_token` - The refresh token from a previous authentication
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
    async fn query<T: DeserializeOwned>(
        &self,
        access_token: &str,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> Result<T, ShopifyError> {
        let url = format!(
            "https://shopify.com/{}/account/customer/api/{}/graphql",
            self.inner.store_id, self.inner.api_version
        );

        let request = GraphQLRequest {
            query: query.to_string(),
            variables,
        };

        let response = self
            .inner
            .client
            .post(&url)
            .header("Authorization", access_token)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ShopifyError::OAuth(format!(
                "Customer API request failed ({status}): {text}"
            )));
        }

        let gql_response: GraphQLResponse<T> = response.json().await?;
        gql_response.into_result()
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
        #[derive(Deserialize)]
        struct Response {
            customer: Customer,
        }

        const QUERY: &str = r"
            query getCustomer {
                customer {
                    id
                    email
                    firstName
                    lastName
                    phone
                    acceptsMarketing
                    defaultAddress {
                        id
                        firstName
                        lastName
                        company
                        address1
                        address2
                        city
                        province
                        provinceCode
                        country
                        countryCode
                        zip
                        phone
                    }
                }
            }
        ";

        let response: Response = self.query(access_token, QUERY, None).await?;
        Ok(response.customer)
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "customerUpdate")]
            customer_update: CustomerUpdateResult,
        }

        #[derive(Deserialize)]
        struct CustomerUpdateResult {
            customer: Option<Customer>,
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation customerUpdate($input: CustomerUpdateInput!) {
                customerUpdate(input: $input) {
                    customer {
                        id
                        email
                        firstName
                        lastName
                        phone
                        acceptsMarketing
                        defaultAddress {
                            id
                            firstName
                            lastName
                            company
                            address1
                            address2
                            city
                            province
                            provinceCode
                            country
                            countryCode
                            zip
                            phone
                        }
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "input": input });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.customer_update.user_errors.is_empty() {
            let messages: Vec<_> = response
                .customer_update
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        response
            .customer_update
            .customer
            .ok_or_else(|| ShopifyError::OAuth("No customer returned".to_string()))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Order Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the customer's order history.
    ///
    /// # Arguments
    ///
    /// * `access_token` - The customer's access token
    /// * `first` - The number of orders to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_orders(
        &self,
        access_token: &str,
        first: u32,
    ) -> Result<Vec<Order>, ShopifyError> {
        #[derive(Deserialize)]
        struct Response {
            customer: CustomerWithOrders,
        }

        #[derive(Deserialize)]
        struct CustomerWithOrders {
            orders: OrderConnection,
        }

        #[derive(Deserialize)]
        struct OrderConnection {
            edges: Vec<OrderEdge>,
        }

        #[derive(Deserialize)]
        struct OrderEdge {
            node: Order,
        }

        const QUERY: &str = r"
            query getOrders($first: Int!) {
                customer {
                    orders(first: $first, sortKey: PROCESSED_AT, reverse: true) {
                        edges {
                            node {
                                id
                                name
                                orderNumber
                                processedAt
                                financialStatus
                                fulfillmentStatus
                                totalPrice {
                                    amount
                                    currencyCode
                                }
                            }
                        }
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "first": first });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        Ok(response
            .customer
            .orders
            .edges
            .into_iter()
            .map(|e| e.node)
            .collect())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Address Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the customer's addresses.
    ///
    /// # Arguments
    ///
    /// * `access_token` - The customer's access token
    /// * `first` - The number of addresses to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_addresses(
        &self,
        access_token: &str,
        first: u32,
    ) -> Result<Vec<Address>, ShopifyError> {
        #[derive(Deserialize)]
        struct Response {
            customer: CustomerWithAddresses,
        }

        #[derive(Deserialize)]
        struct CustomerWithAddresses {
            addresses: AddressConnection,
        }

        #[derive(Deserialize)]
        struct AddressConnection {
            edges: Vec<AddressEdge>,
        }

        #[derive(Deserialize)]
        struct AddressEdge {
            node: Address,
        }

        const QUERY: &str = r"
            query getAddresses($first: Int!) {
                customer {
                    addresses(first: $first) {
                        edges {
                            node {
                                id
                                firstName
                                lastName
                                company
                                address1
                                address2
                                city
                                province
                                provinceCode
                                country
                                countryCode
                                zip
                                phone
                            }
                        }
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "first": first });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        Ok(response
            .customer
            .addresses
            .edges
            .into_iter()
            .map(|e| e.node)
            .collect())
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "customerAddressCreate")]
            address_create: AddressCreateResult,
        }

        #[derive(Deserialize)]
        struct AddressCreateResult {
            #[serde(rename = "customerAddress")]
            address: Option<Address>,
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation createAddress($address: CustomerAddressInput!) {
                customerAddressCreate(address: $address) {
                    customerAddress {
                        id
                        firstName
                        lastName
                        company
                        address1
                        address2
                        city
                        province
                        provinceCode
                        country
                        countryCode
                        zip
                        phone
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "address": address });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.address_create.user_errors.is_empty() {
            let messages: Vec<_> = response
                .address_create
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        response
            .address_create
            .address
            .ok_or_else(|| ShopifyError::OAuth("No address returned".to_string()))
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "customerAddressUpdate")]
            address_update: AddressUpdateResult,
        }

        #[derive(Deserialize)]
        struct AddressUpdateResult {
            #[serde(rename = "customerAddress")]
            address: Option<Address>,
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation updateAddress($addressId: ID!, $address: CustomerAddressInput!) {
                customerAddressUpdate(addressId: $addressId, address: $address) {
                    customerAddress {
                        id
                        firstName
                        lastName
                        company
                        address1
                        address2
                        city
                        province
                        provinceCode
                        country
                        countryCode
                        zip
                        phone
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({
            "addressId": address_id,
            "address": address
        });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.address_update.user_errors.is_empty() {
            let messages: Vec<_> = response
                .address_update
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        response
            .address_update
            .address
            .ok_or_else(|| ShopifyError::OAuth("No address returned".to_string()))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Subscription Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the customer's subscription contracts.
    ///
    /// # Arguments
    ///
    /// * `access_token` - The customer's access token
    /// * `first` - The number of subscriptions to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    pub async fn get_subscriptions(
        &self,
        access_token: &str,
        first: u32,
    ) -> Result<Vec<SubscriptionContract>, ShopifyError> {
        #[derive(Deserialize)]
        struct Response {
            customer: CustomerWithSubscriptions,
        }

        #[derive(Deserialize)]
        struct CustomerWithSubscriptions {
            #[serde(rename = "subscriptionContracts")]
            subscription_contracts: SubscriptionContractConnection,
        }

        #[derive(Deserialize)]
        struct SubscriptionContractConnection {
            edges: Vec<SubscriptionContractEdge>,
        }

        #[derive(Deserialize)]
        struct SubscriptionContractEdge {
            node: SubscriptionContract,
        }

        const QUERY: &str = r"
            query getSubscriptions($first: Int!) {
                customer {
                    subscriptionContracts(
                        first: $first,
                        sortKey: CREATED_AT,
                        reverse: true
                    ) {
                        edges {
                            node {
                                id
                                status
                                createdAt
                                nextBillingDate
                                billingPolicy {
                                    interval
                                    intervalCount {
                                        count
                                    }
                                }
                                deliveryPrice {
                                    amount
                                    currencyCode
                                }
                                lines(first: 10) {
                                    edges {
                                        node {
                                            id
                                            name
                                            quantity
                                            currentPrice {
                                                amount
                                                currencyCode
                                            }
                                            image {
                                                url
                                                altText
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "first": first });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        Ok(response
            .customer
            .subscription_contracts
            .edges
            .into_iter()
            .map(|e| e.node)
            .collect())
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
        #[derive(Deserialize)]
        struct Response {
            customer: CustomerWithSubscription,
        }

        #[derive(Deserialize)]
        struct CustomerWithSubscription {
            #[serde(rename = "subscriptionContract")]
            subscription_contract: Option<SubscriptionContract>,
        }

        const QUERY: &str = r"
            query getSubscription($id: ID!) {
                customer {
                    subscriptionContract(id: $id) {
                        id
                        status
                        createdAt
                        nextBillingDate
                        billingPolicy {
                            interval
                            intervalCount {
                                count
                            }
                        }
                        deliveryPrice {
                            amount
                            currencyCode
                        }
                        lines(first: 10) {
                            edges {
                                node {
                                    id
                                    name
                                    quantity
                                    currentPrice {
                                        amount
                                        currencyCode
                                    }
                                    image {
                                        url
                                        altText
                                    }
                                }
                            }
                        }
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "id": id });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        Ok(response.customer.subscription_contract)
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "subscriptionContractPause")]
            result: MutationResult,
        }

        #[derive(Deserialize)]
        struct MutationResult {
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation pauseSubscription($id: ID!) {
                subscriptionContractPause(subscriptionContractId: $id) {
                    contract {
                        id
                        status
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "id": id });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.result.user_errors.is_empty() {
            let messages: Vec<_> = response
                .result
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        Ok(())
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "subscriptionContractCancel")]
            result: MutationResult,
        }

        #[derive(Deserialize)]
        struct MutationResult {
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation cancelSubscription($id: ID!) {
                subscriptionContractCancel(subscriptionContractId: $id) {
                    contract {
                        id
                        status
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "id": id });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.result.user_errors.is_empty() {
            let messages: Vec<_> = response
                .result
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        Ok(())
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "subscriptionContractActivate")]
            result: MutationResult,
        }

        #[derive(Deserialize)]
        struct MutationResult {
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation activateSubscription($id: ID!) {
                subscriptionContractActivate(subscriptionContractId: $id) {
                    contract {
                        id
                        status
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "id": id });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.result.user_errors.is_empty() {
            let messages: Vec<_> = response
                .result
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Address Operations
    // ─────────────────────────────────────────────────────────────────────────

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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "customerAddressDelete")]
            address_delete: AddressDeleteResult,
        }

        #[derive(Deserialize)]
        struct AddressDeleteResult {
            #[serde(rename = "deletedAddressId")]
            #[allow(dead_code)]
            deleted_address_id: Option<String>,
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation deleteAddress($addressId: ID!) {
                customerAddressDelete(addressId: $addressId) {
                    deletedAddressId
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "addressId": address_id });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.address_delete.user_errors.is_empty() {
            let messages: Vec<_> = response
                .address_delete
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        Ok(())
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
        #[derive(Deserialize)]
        struct Response {
            customer: CustomerWithOrder,
        }

        #[derive(Deserialize)]
        struct CustomerWithOrder {
            order: Option<OrderDetail>,
        }

        const QUERY: &str = r"
            query getOrder($id: ID!) {
                customer {
                    order(id: $id) {
                        id
                        name
                        orderNumber
                        processedAt
                        financialStatus
                        fulfillmentStatus
                        totalPrice { amount currencyCode }
                        subtotal { amount currencyCode }
                        totalShipping { amount currencyCode }
                        totalTax { amount currencyCode }
                        lineItems(first: 50) {
                            edges {
                                node {
                                    id
                                    title
                                    quantity
                                    unitPrice { amount currencyCode }
                                    totalPrice { amount currencyCode }
                                    image { url altText }
                                    variantTitle
                                }
                            }
                        }
                        shippingAddress {
                            id
                            firstName
                            lastName
                            company
                            address1
                            address2
                            city
                            province
                            provinceCode
                            country
                            countryCode
                            zip
                            phone
                        }
                        returns(first: 10) {
                            edges {
                                node {
                                    id
                                    name
                                    status
                                }
                            }
                        }
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "id": id });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        Ok(response.customer.order)
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
        #[derive(Deserialize)]
        struct Response {
            customer: CustomerWithOrderForReturn,
        }

        #[derive(Deserialize)]
        struct CustomerWithOrderForReturn {
            order: Option<OrderForReturn>,
        }

        const QUERY: &str = r"
            query getOrderForReturn($id: ID!) {
                customer {
                    order(id: $id) {
                        id
                        name
                        lineItems(first: 50) {
                            edges {
                                node {
                                    id
                                    title
                                    quantity
                                    image { url altText }
                                    variantTitle
                                    suggestedReturnReasonDefinitions(first: 20) {
                                        edges {
                                            node {
                                                id
                                                name
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "id": order_id });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        Ok(response.customer.order)
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "orderRequestReturn")]
            result: ReturnResult,
        }

        #[derive(Deserialize)]
        struct ReturnResult {
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation requestReturn($orderId: ID!, $items: [RequestedLineItemInput!]!) {
                orderRequestReturn(orderId: $orderId, requestedLineItems: $items) {
                    return {
                        id
                        name
                        status
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({
            "orderId": order_id,
            "items": items,
        });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.result.user_errors.is_empty() {
            let messages: Vec<_> = response
                .result
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        Ok(())
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
        #[derive(Deserialize)]
        struct Response {
            customer: CustomerWithStoreCredit,
        }

        #[derive(Deserialize)]
        struct CustomerWithStoreCredit {
            #[serde(rename = "storeCreditAccounts")]
            store_credit_accounts: StoreCreditConnection,
        }

        #[derive(Deserialize)]
        struct StoreCreditConnection {
            edges: Vec<StoreCreditEdge>,
        }

        #[derive(Deserialize)]
        struct StoreCreditEdge {
            node: StoreCreditAccount,
        }

        const QUERY: &str = r"
            query getStoreCredit {
                customer {
                    storeCreditAccounts(first: 1) {
                        edges {
                            node {
                                balance {
                                    amount
                                    currencyCode
                                }
                            }
                        }
                    }
                }
            }
        ";

        let response: Response = self.query(access_token, QUERY, None).await?;

        Ok(response
            .customer
            .store_credit_accounts
            .edges
            .first()
            .map(|e| e.node.balance.clone()))
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
        #[derive(Deserialize)]
        struct Response {
            customer: CustomerWithContract,
        }

        #[derive(Deserialize)]
        struct CustomerWithContract {
            #[serde(rename = "subscriptionContract")]
            subscription_contract: Option<ContractWithCycles>,
        }

        #[derive(Deserialize)]
        struct ContractWithCycles {
            #[serde(rename = "upcomingBillingCycles")]
            upcoming_billing_cycles: BillingCycleConnection,
        }

        #[derive(Deserialize)]
        struct BillingCycleConnection {
            edges: Vec<BillingCycleEdge>,
        }

        #[derive(Deserialize)]
        struct BillingCycleEdge {
            node: SubscriptionBillingCycle,
        }

        const QUERY: &str = r"
            query getUpcomingBillingCycles($id: ID!, $first: Int!) {
                customer {
                    subscriptionContract(id: $id) {
                        upcomingBillingCycles(first: $first) {
                            edges {
                                node {
                                    billingAttemptExpectedDate
                                    cycleIndex
                                    skipped
                                    status
                                }
                            }
                        }
                    }
                }
            }
        ";

        let variables = serde_json::json!({ "id": contract_id, "first": first });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        Ok(response
            .customer
            .subscription_contract
            .map(|c| {
                c.upcoming_billing_cycles
                    .edges
                    .into_iter()
                    .map(|e| e.node)
                    .collect()
            })
            .unwrap_or_default())
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "subscriptionBillingCycleSkip")]
            result: BillingCycleMutationResult,
        }

        #[derive(Deserialize)]
        struct BillingCycleMutationResult {
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation skipBillingCycle($input: SubscriptionBillingCycleInput!) {
                subscriptionBillingCycleSkip(billingCycleInput: $input) {
                    billingCycle {
                        cycleIndex
                        skipped
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({
            "input": {
                "contractId": contract_id,
                "selector": { "index": cycle_index }
            }
        });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.result.user_errors.is_empty() {
            let messages: Vec<_> = response
                .result
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        Ok(())
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
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "subscriptionBillingCycleUnskip")]
            result: BillingCycleMutationResult,
        }

        #[derive(Deserialize)]
        struct BillingCycleMutationResult {
            #[serde(rename = "userErrors")]
            user_errors: Vec<CustomerUserError>,
        }

        const QUERY: &str = r"
            mutation unskipBillingCycle($input: SubscriptionBillingCycleInput!) {
                subscriptionBillingCycleUnskip(billingCycleInput: $input) {
                    billingCycle {
                        cycleIndex
                        skipped
                    }
                    userErrors {
                        field
                        message
                        code
                    }
                }
            }
        ";

        let variables = serde_json::json!({
            "input": {
                "contractId": contract_id,
                "selector": { "index": cycle_index }
            }
        });
        let response: Response = self.query(access_token, QUERY, Some(variables)).await?;

        if !response.result.user_errors.is_empty() {
            let messages: Vec<_> = response
                .result
                .user_errors
                .iter()
                .map(|e| e.message.as_str())
                .collect();
            return Err(ShopifyError::UserError(messages.join(", ")));
        }

        Ok(())
    }
}
