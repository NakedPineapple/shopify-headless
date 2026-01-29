//! Gift card management operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError,
    queries::{
        GetGiftCardConfiguration, GetGiftCardDetail, GetGiftCards, GetGiftCardsCount,
        GiftCardCreate, GiftCardCredit, GiftCardDeactivate, GiftCardDebit,
        GiftCardSendNotificationToCustomer, GiftCardSendNotificationToRecipient, GiftCardUpdate,
    },
};
use crate::shopify::types::{
    GiftCard, GiftCardConfiguration, GiftCardConnection, GiftCardDetail, GiftCardRecipient,
    GiftCardSortKey, GiftCardTransaction, Money, PageInfo,
};

impl AdminClient {
    /// Get a paginated list of gift cards with optional sorting.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of gift cards to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query (Shopify query syntax)
    /// * `sort_key` - Optional sort key
    /// * `reverse` - Whether to reverse sort order
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_gift_cards(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
        sort_key: Option<GiftCardSortKey>,
        reverse: bool,
    ) -> Result<GiftCardConnection, AdminShopifyError> {
        use super::queries::get_gift_cards::{GiftCardSortKeys, Variables};

        let sort_key_gql = sort_key.map(|sk| match sk {
            GiftCardSortKey::AmountSpent => GiftCardSortKeys::AMOUNT_SPENT,
            GiftCardSortKey::Balance => GiftCardSortKeys::BALANCE,
            GiftCardSortKey::Code => GiftCardSortKeys::CODE,
            GiftCardSortKey::CreatedAt => GiftCardSortKeys::CREATED_AT,
            GiftCardSortKey::CustomerName => GiftCardSortKeys::CUSTOMER_NAME,
            GiftCardSortKey::DisabledAt => GiftCardSortKeys::DISABLED_AT,
            GiftCardSortKey::ExpiresOn => GiftCardSortKeys::EXPIRES_ON,
            GiftCardSortKey::Id => GiftCardSortKeys::ID,
            GiftCardSortKey::InitialValue => GiftCardSortKeys::INITIAL_VALUE,
            GiftCardSortKey::UpdatedAt => GiftCardSortKeys::UPDATED_AT,
        });

        let variables = Variables {
            first: Some(first),
            after,
            query,
            sort_key: sort_key_gql,
            reverse: Some(reverse),
        };

        let response = self.execute::<GetGiftCards>(variables).await?;

        let gift_cards: Vec<GiftCard> = response
            .gift_cards
            .edges
            .into_iter()
            .map(|e| {
                let gc = e.node;
                GiftCard {
                    id: gc.id,
                    last_characters: gc.last_characters,
                    masked_code: Some(gc.masked_code),
                    balance: Money {
                        amount: gc.balance.amount,
                        currency_code: format!("{:?}", gc.balance.currency_code),
                    },
                    initial_value: Money {
                        amount: gc.initial_value.amount,
                        currency_code: format!("{:?}", gc.initial_value.currency_code),
                    },
                    expires_on: gc.expires_on,
                    enabled: gc.enabled,
                    deactivated_at: gc.deactivated_at,
                    created_at: gc.created_at,
                    updated_at: Some(gc.updated_at),
                    customer_id: gc.customer.as_ref().map(|c| c.id.clone()),
                    #[allow(deprecated)]
                    customer_email: gc.customer.as_ref().and_then(|c| c.email.clone()),
                    customer_name: gc.customer.as_ref().map(|c| c.display_name.clone()),
                    note: gc.note,
                    order_id: gc.order.as_ref().map(|o| o.id.clone()),
                    order_name: gc.order.as_ref().map(|o| o.name.clone()),
                }
            })
            .collect();

        Ok(GiftCardConnection {
            gift_cards,
            page_info: PageInfo {
                has_next_page: response.gift_cards.page_info.has_next_page,
                has_previous_page: response.gift_cards.page_info.has_previous_page,
                start_cursor: response.gift_cards.page_info.start_cursor,
                end_cursor: response.gift_cards.page_info.end_cursor,
            },
            total_count: None,
        })
    }

    /// Get the count of gift cards matching a query.
    ///
    /// # Arguments
    ///
    /// * `query` - Optional search query (Shopify query syntax)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_gift_cards_count(
        &self,
        query: Option<String>,
    ) -> Result<i64, AdminShopifyError> {
        let variables = super::queries::get_gift_cards_count::Variables { query };
        let response = self.execute::<GetGiftCardsCount>(variables).await?;
        Ok(response.gift_cards_count.map_or(0, |c| c.count))
    }

    /// Get a single gift card with full details including transactions.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or gift card not found.
    #[instrument(skip(self))]
    pub async fn get_gift_card_detail(
        &self,
        id: &str,
    ) -> Result<GiftCardDetail, AdminShopifyError> {
        let variables = super::queries::get_gift_card_detail::Variables { id: id.to_string() };
        let response = self.execute::<GetGiftCardDetail>(variables).await?;

        let gc = response.gift_card.ok_or_else(|| {
            AdminShopifyError::GraphQL(vec![GraphQLError {
                message: "Gift card not found".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })?;

        use super::queries::get_gift_card_detail::GetGiftCardDetailGiftCardTransactionsEdgesNodeOn;

        let transactions: Vec<GiftCardTransaction> = gc
            .transactions
            .map(|t| {
                t.edges
                    .into_iter()
                    .map(|e| {
                        let tx = e.node;
                        let is_credit = matches!(
                            tx.on,
                            GetGiftCardDetailGiftCardTransactionsEdgesNodeOn::GiftCardCreditTransaction
                        );
                        GiftCardTransaction {
                            id: tx.id,
                            amount: Money {
                                amount: tx.amount.amount,
                                currency_code: format!("{:?}", tx.amount.currency_code),
                            },
                            processed_at: tx.processed_at,
                            note: tx.note,
                            is_credit,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        #[allow(deprecated)]
        let recipient = gc.recipient_attributes.map(|r| GiftCardRecipient {
            recipient_id: Some(r.recipient.id.clone()),
            recipient_name: Some(r.recipient.display_name.clone()),
            recipient_email: r.recipient.email.clone(),
            preferred_name: r.preferred_name,
            message: r.message,
            send_notification_at: r.send_notification_at,
        });

        Ok(GiftCardDetail {
            id: gc.id,
            last_characters: gc.last_characters,
            masked_code: gc.masked_code,
            balance: Money {
                amount: gc.balance.amount,
                currency_code: format!("{:?}", gc.balance.currency_code),
            },
            initial_value: Money {
                amount: gc.initial_value.amount,
                currency_code: format!("{:?}", gc.initial_value.currency_code),
            },
            expires_on: gc.expires_on,
            enabled: gc.enabled,
            deactivated_at: gc.deactivated_at,
            created_at: gc.created_at,
            updated_at: gc.updated_at,
            note: gc.note,
            template_suffix: gc.template_suffix,
            customer_id: gc.customer.as_ref().map(|c| c.id.clone()),
            customer_name: gc.customer.as_ref().map(|c| c.display_name.clone()),
            #[allow(deprecated)]
            customer_email: gc.customer.as_ref().and_then(|c| c.email.clone()),
            #[allow(deprecated)]
            customer_phone: gc.customer.as_ref().and_then(|c| c.phone.clone()),
            recipient,
            order_id: gc.order.as_ref().map(|o| o.id.clone()),
            order_name: gc.order.as_ref().map(|o| o.name.clone()),
            order_created_at: gc.order.as_ref().map(|o| o.created_at.clone()),
            transactions,
        })
    }

    /// Get gift card configuration (shop limits).
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_gift_card_configuration(
        &self,
    ) -> Result<GiftCardConfiguration, AdminShopifyError> {
        let variables = super::queries::get_gift_card_configuration::Variables {};
        let response = self.execute::<GetGiftCardConfiguration>(variables).await?;
        let config = response.gift_card_configuration;

        Ok(GiftCardConfiguration {
            issue_limit: Some(Money {
                amount: config.issue_limit.amount,
                currency_code: format!("{:?}", config.issue_limit.currency_code),
            }),
            purchase_limit: Some(Money {
                amount: config.purchase_limit.amount,
                currency_code: format!("{:?}", config.purchase_limit.currency_code),
            }),
        })
    }

    /// Create a new gift card.
    ///
    /// # Arguments
    ///
    /// * `initial_value` - Initial value amount as decimal string
    /// * `customer_id` - Optional customer to associate
    /// * `expires_on` - Optional expiration date (YYYY-MM-DD)
    /// * `note` - Optional internal note
    ///
    /// # Returns
    ///
    /// Returns a tuple of (gift card ID, gift card code) on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn create_gift_card(
        &self,
        initial_value: &str,
        customer_id: Option<&str>,
        expires_on: Option<&str>,
        note: Option<&str>,
        recipient_id: Option<&str>,
        recipient_message: Option<&str>,
    ) -> Result<(String, String), AdminShopifyError> {
        use super::queries::gift_card_create::{
            GiftCardCreateInput, GiftCardRecipientInput, Variables,
        };

        let recipient_attributes = recipient_id.map(|id| GiftCardRecipientInput {
            id: id.to_string(),
            message: recipient_message.map(String::from),
            preferred_name: None,
            send_notification_at: None,
        });

        let variables = Variables {
            input: GiftCardCreateInput {
                initial_value: initial_value.to_string(),
                customer_id: customer_id.map(String::from),
                expires_on: expires_on.map(String::from),
                note: note.map(String::from),
                code: None,
                template_suffix: None,
                recipient_attributes,
            },
        };

        let response = self.execute::<GiftCardCreate>(variables).await?;

        if let Some(payload) = response.gift_card_create {
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

            if let (Some(gc), Some(code)) = (payload.gift_card, payload.gift_card_code) {
                return Ok((gc.id, code));
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No gift card returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Deactivate a gift card permanently.
    ///
    /// Warning: This action cannot be undone. Once deactivated, the gift card
    /// can no longer be used.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID to deactivate
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn deactivate_gift_card(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = super::queries::gift_card_deactivate::Variables { id: id.to_string() };

        let response = self.execute::<GiftCardDeactivate>(variables).await?;

        if let Some(payload) = response.gift_card_deactivate
            && !payload.user_errors.is_empty()
        {
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

        Ok(())
    }

    /// Update a gift card's details.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID to update
    /// * `note` - New internal note (None = no change)
    /// * `expires_on` - New expiration date (None = no change, Some("") = remove expiration)
    /// * `customer_id` - Customer to assign (None = no change)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_gift_card(
        &self,
        id: &str,
        note: Option<&str>,
        expires_on: Option<&str>,
        customer_id: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::gift_card_update::{GiftCardUpdateInput, Variables};

        let variables = Variables {
            id: id.to_string(),
            input: GiftCardUpdateInput {
                note: note.map(String::from),
                expires_on: expires_on.map(String::from),
                customer_id: customer_id.map(String::from),
                template_suffix: None,
                recipient_attributes: None,
            },
        };

        let response = self.execute::<GiftCardUpdate>(variables).await?;

        if let Some(payload) = response.gift_card_update
            && !payload.user_errors.is_empty()
        {
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

        Ok(())
    }

    /// Credit a gift card (add funds).
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    /// * `amount` - Amount to credit as decimal string
    /// * `currency_code` - Currency code (e.g., "USD")
    /// * `note` - Optional note for the transaction
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn credit_gift_card(
        &self,
        id: &str,
        amount: &str,
        currency_code: &str,
        note: Option<&str>,
    ) -> Result<GiftCardTransaction, AdminShopifyError> {
        use super::queries::gift_card_credit::{
            CurrencyCode, GiftCardCreditInput, MoneyInput, Variables,
        };

        let currency = match currency_code.to_uppercase().as_str() {
            "CAD" => CurrencyCode::CAD,
            "EUR" => CurrencyCode::EUR,
            "GBP" => CurrencyCode::GBP,
            "AUD" => CurrencyCode::AUD,
            _ => CurrencyCode::USD,
        };

        let variables = Variables {
            id: id.to_string(),
            credit_input: GiftCardCreditInput {
                credit_amount: MoneyInput {
                    amount: amount.to_string(),
                    currency_code: currency,
                },
                note: note.map(String::from),
                processed_at: None,
            },
        };

        let response = self.execute::<GiftCardCredit>(variables).await?;

        if let Some(payload) = response.gift_card_credit {
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

            if let Some(tx) = payload.gift_card_credit_transaction {
                return Ok(GiftCardTransaction {
                    id: tx.id,
                    amount: Money {
                        amount: tx.amount.amount,
                        currency_code: format!("{:?}", tx.amount.currency_code),
                    },
                    processed_at: tx.processed_at,
                    note: tx.note,
                    is_credit: true,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No transaction returned from credit".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Debit a gift card (remove funds).
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    /// * `amount` - Amount to debit as decimal string
    /// * `currency_code` - Currency code (e.g., "USD")
    /// * `note` - Optional note for the transaction
    ///
    /// # Errors
    ///
    /// Returns an error if insufficient funds or API request fails.
    #[instrument(skip(self))]
    pub async fn debit_gift_card(
        &self,
        id: &str,
        amount: &str,
        currency_code: &str,
        note: Option<&str>,
    ) -> Result<GiftCardTransaction, AdminShopifyError> {
        use super::queries::gift_card_debit::{
            CurrencyCode, GiftCardDebitInput, MoneyInput, Variables,
        };

        let currency = match currency_code.to_uppercase().as_str() {
            "CAD" => CurrencyCode::CAD,
            "EUR" => CurrencyCode::EUR,
            "GBP" => CurrencyCode::GBP,
            "AUD" => CurrencyCode::AUD,
            _ => CurrencyCode::USD,
        };

        let variables = Variables {
            id: id.to_string(),
            debit_input: GiftCardDebitInput {
                debit_amount: MoneyInput {
                    amount: amount.to_string(),
                    currency_code: currency,
                },
                note: note.map(String::from),
                processed_at: None,
            },
        };

        let response = self.execute::<GiftCardDebit>(variables).await?;

        if let Some(payload) = response.gift_card_debit {
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

            if let Some(tx) = payload.gift_card_debit_transaction {
                return Ok(GiftCardTransaction {
                    id: tx.id,
                    amount: Money {
                        amount: tx.amount.amount,
                        currency_code: format!("{:?}", tx.amount.currency_code),
                    },
                    processed_at: tx.processed_at,
                    note: tx.note,
                    is_credit: false,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No transaction returned from debit".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Send gift card notification to the assigned customer.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    ///
    /// # Errors
    ///
    /// Returns an error if no customer is assigned or API request fails.
    #[instrument(skip(self))]
    pub async fn send_gift_card_notification_to_customer(
        &self,
        id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::gift_card_send_notification_to_customer::Variables {
            id: id.to_string(),
        };

        let response = self
            .execute::<GiftCardSendNotificationToCustomer>(variables)
            .await?;

        if let Some(payload) = response.gift_card_send_notification_to_customer
            && !payload.user_errors.is_empty()
        {
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

        Ok(())
    }

    /// Send gift card notification to the designated recipient.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    ///
    /// # Errors
    ///
    /// Returns an error if no recipient is set or API request fails.
    #[instrument(skip(self))]
    pub async fn send_gift_card_notification_to_recipient(
        &self,
        id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::gift_card_send_notification_to_recipient::Variables {
            id: id.to_string(),
        };

        let response = self
            .execute::<GiftCardSendNotificationToRecipient>(variables)
            .await?;

        if let Some(payload) = response.gift_card_send_notification_to_recipient
            && !payload.user_errors.is_empty()
        {
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

        Ok(())
    }
}
