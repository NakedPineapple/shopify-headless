//! Customer management operations for the Admin API.

use graphql_client::GraphQLQuery;
use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError,
    conversions::{convert_customer, convert_customer_connection},
    queries::{
        CustomerAddressCreate, CustomerAddressDelete, CustomerAddressUpdate, CustomerCreate,
        CustomerDelete, CustomerEmailMarketingConsentUpdate, CustomerGenerateAccountActivationUrl,
        CustomerMerge, CustomerSendAccountInviteEmail, CustomerSmsMarketingConsentUpdate,
        CustomerUpdate, CustomerUpdateDefaultAddress, GetCustomer, GetCustomers, TagsAdd,
        TagsRemove,
    },
    sort_customers,
};
use crate::shopify::types::{
    Address, AddressInput, Customer, CustomerConnection, CustomerListParams,
    CustomerMergeOverrides, CustomerSortKey, CustomerUpdateParams,
};

impl AdminClient {
    /// Get a customer by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify customer ID (e.g., `gid://shopify/Customer/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self), fields(customer_id = %id))]
    pub async fn get_customer(&self, id: &str) -> Result<Option<Customer>, AdminShopifyError> {
        let variables = super::queries::get_customer::Variables {
            id: id.to_string(),
            address_count: Some(10),
            order_count: Some(10),
        };

        let response = self.execute::<GetCustomer>(variables).await?;

        Ok(response.customer.map(convert_customer))
    }

    /// Get a paginated list of customers.
    ///
    /// # Arguments
    ///
    /// * `params` - Customer list parameters including pagination, query, sort
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self))]
    pub async fn get_customers(
        &self,
        params: CustomerListParams,
    ) -> Result<CustomerConnection, AdminShopifyError> {
        use super::queries::get_customers::CustomerSortKeys;

        let client_side_sort = params.sort_key.is_some_and(|sk| !sk.is_shopify_native());

        let sort_key = params.sort_key.and_then(|sk| match sk {
            CustomerSortKey::CreatedAt => Some(CustomerSortKeys::CREATED_AT),
            CustomerSortKey::Id => Some(CustomerSortKeys::ID),
            CustomerSortKey::Location => Some(CustomerSortKeys::LOCATION),
            CustomerSortKey::Name => Some(CustomerSortKeys::NAME),
            CustomerSortKey::Relevance => Some(CustomerSortKeys::RELEVANCE),
            CustomerSortKey::UpdatedAt => Some(CustomerSortKeys::UPDATED_AT),
            CustomerSortKey::AmountSpent | CustomerSortKey::OrdersCount => None,
        });

        let variables = super::queries::get_customers::Variables {
            first: params.first,
            after: params.after,
            query: params.query.clone(),
            sort_key,
            reverse: Some(if client_side_sort {
                false
            } else {
                params.reverse
            }),
        };

        let response = self.execute::<GetCustomers>(variables).await?;
        let mut connection = convert_customer_connection(response.customers);

        if client_side_sort && let Some(sk) = params.sort_key {
            sort_customers(&mut connection.customers, sk, params.reverse);
        }

        Ok(connection)
    }

    /// Create a new customer.
    ///
    /// # Arguments
    ///
    /// * `email` - Customer email address
    /// * `first_name` - Customer first name
    /// * `last_name` - Customer last name
    /// * `phone` - Optional phone number
    /// * `note` - Optional customer note
    /// * `tags` - Optional tags
    ///
    /// # Returns
    ///
    /// Returns the created customer's ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn create_customer(
        &self,
        email: &str,
        first_name: Option<&str>,
        last_name: Option<&str>,
        phone: Option<&str>,
        note: Option<&str>,
        tags: Vec<String>,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::customer_create::{CustomerInput, Variables};

        let variables = Variables {
            input: CustomerInput {
                id: None,
                email: Some(email.to_string()),
                first_name: first_name.map(String::from),
                last_name: last_name.map(String::from),
                phone: phone.map(String::from),
                note: note.map(String::from),
                tags: Some(tags),
                addresses: None,
                locale: None,
                metafields: None,
                multipass_identifier: None,
                sms_marketing_consent: None,
                email_marketing_consent: None,
                tax_exempt: None,
                tax_exemptions: None,
            },
        };

        let response: <CustomerCreate as GraphQLQuery>::ResponseData =
            self.execute::<CustomerCreate>(variables).await?;

        if let Some(payload) = response.customer_create {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(customer) = payload.customer {
                return Ok(customer.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No customer returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing customer.
    ///
    /// # Arguments
    ///
    /// * `id` - Customer ID
    /// * `params` - Update parameters
    ///
    /// # Returns
    ///
    /// Returns the updated customer's ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, params))]
    pub async fn update_customer(
        &self,
        id: &str,
        params: CustomerUpdateParams,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::customer_update::{CustomerInput, Variables};

        let variables = Variables {
            input: CustomerInput {
                id: Some(id.to_string()),
                email: params.email,
                first_name: params.first_name,
                last_name: params.last_name,
                phone: params.phone,
                note: params.note,
                tags: params.tags,
                addresses: None,
                locale: None,
                metafields: None,
                multipass_identifier: None,
                sms_marketing_consent: None,
                email_marketing_consent: None,
                tax_exempt: None,
                tax_exemptions: None,
            },
        };

        let response: <CustomerUpdate as GraphQLQuery>::ResponseData =
            self.execute::<CustomerUpdate>(variables).await?;

        if let Some(payload) = response.customer_update {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(customer) = payload.customer {
                return Ok(customer.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No customer returned from update".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Delete a customer.
    ///
    /// Note: Customers with orders cannot be deleted.
    ///
    /// # Arguments
    ///
    /// * `id` - Customer ID to delete
    ///
    /// # Returns
    ///
    /// Returns the deleted customer's ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails, the customer has orders,
    /// or returns user errors.
    #[instrument(skip(self), fields(customer_id = %id))]
    pub async fn delete_customer(&self, id: &str) -> Result<String, AdminShopifyError> {
        use super::queries::customer_delete::{CustomerDeleteInput, Variables};

        let variables = Variables {
            input: CustomerDeleteInput { id: id.to_string() },
        };

        let response: <CustomerDelete as GraphQLQuery>::ResponseData =
            self.execute::<CustomerDelete>(variables).await?;

        if let Some(payload) = response.customer_delete {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(deleted_id) = payload.deleted_customer_id {
                return Ok(deleted_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Customer deletion failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add tags to a customer.
    ///
    /// # Arguments
    ///
    /// * `id` - Customer ID
    /// * `tags` - Tags to add
    ///
    /// # Returns
    ///
    /// Returns the updated tags list on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %id))]
    pub async fn add_customer_tags(
        &self,
        id: &str,
        tags: Vec<String>,
    ) -> Result<Vec<String>, AdminShopifyError> {
        use super::queries::tags_add::Variables;

        let variables = Variables {
            id: id.to_string(),
            tags,
        };

        let response: <TagsAdd as GraphQLQuery>::ResponseData =
            self.execute::<TagsAdd>(variables).await?;

        if let Some(payload) = response.tags_add {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(vec![]);
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Tags add failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove tags from a customer.
    ///
    /// # Arguments
    ///
    /// * `id` - Customer ID
    /// * `tags` - Tags to remove
    ///
    /// # Returns
    ///
    /// Returns the updated tags list on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %id))]
    pub async fn remove_customer_tags(
        &self,
        id: &str,
        tags: Vec<String>,
    ) -> Result<Vec<String>, AdminShopifyError> {
        use super::queries::tags_remove::Variables;

        let variables = Variables {
            id: id.to_string(),
            tags,
        };

        let response: <TagsRemove as GraphQLQuery>::ResponseData =
            self.execute::<TagsRemove>(variables).await?;

        if let Some(payload) = response.tags_remove {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(vec![]);
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Tags remove failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Send account invitation email to a customer.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Customer ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn send_customer_invite(&self, customer_id: &str) -> Result<(), AdminShopifyError> {
        use super::queries::customer_send_account_invite_email::Variables;

        let variables = Variables {
            customer_id: customer_id.to_string(),
        };

        let response: <CustomerSendAccountInviteEmail as GraphQLQuery>::ResponseData = self
            .execute::<CustomerSendAccountInviteEmail>(variables)
            .await?;

        if let Some(payload) = response.customer_send_account_invite_email {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Send invite failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Generate account activation URL for a customer.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Customer ID
    ///
    /// # Returns
    ///
    /// Returns the activation URL string.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn generate_customer_activation_url(
        &self,
        customer_id: &str,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::customer_generate_account_activation_url::Variables;

        let variables = Variables {
            customer_id: customer_id.to_string(),
        };

        let response: <CustomerGenerateAccountActivationUrl as GraphQLQuery>::ResponseData = self
            .execute::<CustomerGenerateAccountActivationUrl>(variables)
            .await?;

        if let Some(payload) = response.customer_generate_account_activation_url {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(url) = payload.account_activation_url {
                return Ok(url);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Generate activation URL failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Create a new address for a customer.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `address` - Address details
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, address), fields(customer_id = %customer_id))]
    pub async fn create_customer_address(
        &self,
        customer_id: &str,
        address: AddressInput,
    ) -> Result<Address, AdminShopifyError> {
        use super::queries::customer_address_create::{
            CountryCode, MailingAddressInput, Variables,
        };

        let country_code =
            address
                .country_code
                .and_then(|code| match code.to_uppercase().as_str() {
                    "US" => Some(CountryCode::US),
                    "CA" => Some(CountryCode::CA),
                    "GB" => Some(CountryCode::GB),
                    "AU" => Some(CountryCode::AU),
                    "DE" => Some(CountryCode::DE),
                    "FR" => Some(CountryCode::FR),
                    "JP" => Some(CountryCode::JP),
                    "CN" => Some(CountryCode::CN),
                    "MX" => Some(CountryCode::MX),
                    "BR" => Some(CountryCode::BR),
                    _ => None,
                });

        let variables = Variables {
            customer_id: customer_id.to_string(),
            address: MailingAddressInput {
                address1: address.address1,
                address2: address.address2,
                city: address.city,
                province_code: address.province_code,
                country_code,
                zip: address.zip,
                first_name: address.first_name,
                last_name: address.last_name,
                company: address.company,
                phone: address.phone,
            },
        };

        let response = self.execute::<CustomerAddressCreate>(variables).await?;

        if let Some(payload) = response.customer_address_create {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(addr) = payload.address {
                return Ok(Address {
                    id: Some(addr.id),
                    address1: addr.address1,
                    address2: addr.address2,
                    city: addr.city,
                    province_code: addr.province_code,
                    country_code: addr.country_code_v2.map(|c| format!("{c:?}")),
                    zip: addr.zip,
                    first_name: addr.first_name,
                    last_name: addr.last_name,
                    company: addr.company,
                    phone: addr.phone,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Create customer address failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing customer address.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `address_id` - Shopify address GID
    /// * `address` - Updated address details
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, address), fields(customer_id = %customer_id, address_id = %address_id))]
    pub async fn update_customer_address(
        &self,
        customer_id: &str,
        address_id: &str,
        address: AddressInput,
    ) -> Result<Address, AdminShopifyError> {
        use super::queries::customer_address_update::{
            CountryCode, MailingAddressInput, Variables,
        };

        let country_code =
            address
                .country_code
                .and_then(|code| match code.to_uppercase().as_str() {
                    "US" => Some(CountryCode::US),
                    "CA" => Some(CountryCode::CA),
                    "GB" => Some(CountryCode::GB),
                    "AU" => Some(CountryCode::AU),
                    "DE" => Some(CountryCode::DE),
                    "FR" => Some(CountryCode::FR),
                    "JP" => Some(CountryCode::JP),
                    "CN" => Some(CountryCode::CN),
                    "MX" => Some(CountryCode::MX),
                    "BR" => Some(CountryCode::BR),
                    _ => None,
                });

        let variables = Variables {
            customer_id: customer_id.to_string(),
            address_id: address_id.to_string(),
            address: MailingAddressInput {
                address1: address.address1,
                address2: address.address2,
                city: address.city,
                province_code: address.province_code,
                country_code,
                zip: address.zip,
                first_name: address.first_name,
                last_name: address.last_name,
                company: address.company,
                phone: address.phone,
            },
        };

        let response = self.execute::<CustomerAddressUpdate>(variables).await?;

        if let Some(payload) = response.customer_address_update {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(addr) = payload.address {
                return Ok(Address {
                    id: Some(addr.id),
                    address1: addr.address1,
                    address2: addr.address2,
                    city: addr.city,
                    province_code: addr.province_code,
                    country_code: addr.country_code_v2.map(|c| format!("{c:?}")),
                    zip: addr.zip,
                    first_name: addr.first_name,
                    last_name: addr.last_name,
                    company: addr.company,
                    phone: addr.phone,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Update customer address failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Delete a customer address.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `address_id` - Shopify address GID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id, address_id = %address_id))]
    pub async fn delete_customer_address(
        &self,
        customer_id: &str,
        address_id: &str,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::customer_address_delete::Variables;

        let variables = Variables {
            customer_id: customer_id.to_string(),
            address_id: address_id.to_string(),
        };

        let response: <CustomerAddressDelete as GraphQLQuery>::ResponseData =
            self.execute::<CustomerAddressDelete>(variables).await?;

        if let Some(payload) = response.customer_address_delete {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(deleted_id) = payload.deleted_address_id {
                return Ok(deleted_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Delete customer address failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Set a customer's default address.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `address_id` - Shopify address GID to set as default
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id, address_id = %address_id))]
    pub async fn set_customer_default_address(
        &self,
        customer_id: &str,
        address_id: &str,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::customer_update_default_address::Variables;

        let variables = Variables {
            customer_id: customer_id.to_string(),
            address_id: address_id.to_string(),
        };

        let response: <CustomerUpdateDefaultAddress as GraphQLQuery>::ResponseData = self
            .execute::<CustomerUpdateDefaultAddress>(variables)
            .await?;

        if let Some(payload) = response.customer_update_default_address {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Set default address failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update customer email marketing consent.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `marketing_state` - New marketing state (SUBSCRIBED, UNSUBSCRIBED, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn update_customer_email_marketing(
        &self,
        customer_id: &str,
        marketing_state: &str,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::customer_email_marketing_consent_update::{
            CustomerEmailMarketingConsentInput, CustomerEmailMarketingConsentUpdateInput,
            CustomerEmailMarketingState, Variables,
        };

        let state = match marketing_state {
            "SUBSCRIBED" => CustomerEmailMarketingState::SUBSCRIBED,
            "UNSUBSCRIBED" => CustomerEmailMarketingState::UNSUBSCRIBED,
            "PENDING" => CustomerEmailMarketingState::PENDING,
            _ => CustomerEmailMarketingState::NOT_SUBSCRIBED,
        };

        let variables = Variables {
            input: CustomerEmailMarketingConsentUpdateInput {
                customer_id: customer_id.to_string(),
                email_marketing_consent: CustomerEmailMarketingConsentInput {
                    consent_updated_at: None,
                    marketing_opt_in_level: None,
                    marketing_state: state,
                    source_location_id: None,
                },
            },
        };

        let response: <CustomerEmailMarketingConsentUpdate as GraphQLQuery>::ResponseData = self
            .execute::<CustomerEmailMarketingConsentUpdate>(variables)
            .await?;

        if let Some(payload) = response.customer_email_marketing_consent_update {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Update email marketing consent failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update customer SMS marketing consent.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `marketing_state` - New marketing state (SUBSCRIBED, UNSUBSCRIBED, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn update_customer_sms_marketing(
        &self,
        customer_id: &str,
        marketing_state: &str,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::customer_sms_marketing_consent_update::{
            CustomerSmsMarketingConsentInput, CustomerSmsMarketingConsentUpdateInput,
            CustomerSmsMarketingState, Variables,
        };

        let state = match marketing_state {
            "SUBSCRIBED" => CustomerSmsMarketingState::SUBSCRIBED,
            "UNSUBSCRIBED" => CustomerSmsMarketingState::UNSUBSCRIBED,
            "PENDING" => CustomerSmsMarketingState::PENDING,
            _ => CustomerSmsMarketingState::NOT_SUBSCRIBED,
        };

        let variables = Variables {
            input: CustomerSmsMarketingConsentUpdateInput {
                customer_id: customer_id.to_string(),
                sms_marketing_consent: CustomerSmsMarketingConsentInput {
                    consent_updated_at: None,
                    marketing_opt_in_level: None,
                    marketing_state: state,
                    source_location_id: None,
                },
            },
        };

        let response: <CustomerSmsMarketingConsentUpdate as GraphQLQuery>::ResponseData = self
            .execute::<CustomerSmsMarketingConsentUpdate>(variables)
            .await?;

        if let Some(payload) = response.customer_sms_marketing_consent_update {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Update SMS marketing consent failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Merge two customers.
    ///
    /// # Arguments
    ///
    /// * `customer_one_id` - Customer to merge INTO (will remain)
    /// * `customer_two_id` - Customer to merge FROM (will be deleted)
    /// * `overrides` - Fields to take from `customer_two` instead of `customer_one`
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_one_id = %customer_one_id, customer_two_id = %customer_two_id))]
    pub async fn merge_customers(
        &self,
        customer_one_id: &str,
        customer_two_id: &str,
        overrides: CustomerMergeOverrides,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::customer_merge::{CustomerMergeOverrideFields, Variables};

        let has_overrides = overrides.first_name
            || overrides.last_name
            || overrides.email
            || overrides.phone
            || overrides.default_address;

        let override_fields = if has_overrides {
            Some(CustomerMergeOverrideFields {
                customer_id_of_first_name_to_keep: if overrides.first_name {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                customer_id_of_last_name_to_keep: if overrides.last_name {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                customer_id_of_email_to_keep: if overrides.email {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                customer_id_of_phone_number_to_keep: if overrides.phone {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                customer_id_of_default_address_to_keep: if overrides.default_address {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                note: None,
                tags: None,
            })
        } else {
            None
        };

        let variables = Variables {
            customer_one_id: customer_one_id.to_string(),
            customer_two_id: customer_two_id.to_string(),
            override_fields,
        };

        let response: <CustomerMerge as GraphQLQuery>::ResponseData =
            self.execute::<CustomerMerge>(variables).await?;

        if let Some(payload) = response.customer_merge {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(resulting_id) = payload.resulting_customer_id {
                return Ok(resulting_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Merge customers failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }
}
