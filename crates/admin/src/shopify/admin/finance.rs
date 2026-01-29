//! Finance operations (payouts, disputes, bank accounts) for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError,
    queries::{
        GetBankAccounts, GetDispute, GetDisputes, GetPayout, GetPayoutDetail, GetPayoutSchedule,
        GetPayoutTransactions, GetPayouts,
    },
    sort_payouts,
};
use crate::shopify::types::{
    BalanceTransaction, BalanceTransactionConnection, BalanceTransactionSourceType,
    BalanceTransactionType, BankAccount, BankAccountStatus, Dispute, DisputeAddress,
    DisputeConnection, DisputeDetail, DisputeEvidence, DisputeFileUpload, DisputeFulfillment,
    DisputeReasonDetails, DisputeStatus, DisputeType, Money, PageInfo, Payout, PayoutConnection,
    PayoutDetail, PayoutSchedule, PayoutScheduleInterval, PayoutSortKey, PayoutStatus,
    PayoutSummary, PayoutTransactionType,
};

/// Convert GraphQL payout status to domain type.
const fn convert_payout_status(
    status: &super::queries::get_payouts::ShopifyPaymentsPayoutStatus,
) -> PayoutStatus {
    match status {
        super::queries::get_payouts::ShopifyPaymentsPayoutStatus::SCHEDULED
        | super::queries::get_payouts::ShopifyPaymentsPayoutStatus::Other(_) => {
            PayoutStatus::Scheduled
        }
        super::queries::get_payouts::ShopifyPaymentsPayoutStatus::IN_TRANSIT => {
            PayoutStatus::InTransit
        }
        super::queries::get_payouts::ShopifyPaymentsPayoutStatus::PAID => PayoutStatus::Paid,
        super::queries::get_payouts::ShopifyPaymentsPayoutStatus::FAILED => PayoutStatus::Failed,
        super::queries::get_payouts::ShopifyPaymentsPayoutStatus::CANCELED => {
            PayoutStatus::Canceled
        }
    }
}

/// Convert GraphQL payout status to domain type (for single payout query).
const fn convert_payout_status_single(
    status: &super::queries::get_payout::ShopifyPaymentsPayoutStatus,
) -> PayoutStatus {
    match status {
        super::queries::get_payout::ShopifyPaymentsPayoutStatus::SCHEDULED
        | super::queries::get_payout::ShopifyPaymentsPayoutStatus::Other(_) => {
            PayoutStatus::Scheduled
        }
        super::queries::get_payout::ShopifyPaymentsPayoutStatus::IN_TRANSIT => {
            PayoutStatus::InTransit
        }
        super::queries::get_payout::ShopifyPaymentsPayoutStatus::PAID => PayoutStatus::Paid,
        super::queries::get_payout::ShopifyPaymentsPayoutStatus::FAILED => PayoutStatus::Failed,
        super::queries::get_payout::ShopifyPaymentsPayoutStatus::CANCELED => PayoutStatus::Canceled,
    }
}

/// Convert GraphQL payout status (detail query) to domain type.
const fn convert_payout_status_detail(
    status: &super::queries::get_payout_detail::ShopifyPaymentsPayoutStatus,
) -> PayoutStatus {
    match status {
        super::queries::get_payout_detail::ShopifyPaymentsPayoutStatus::SCHEDULED
        | super::queries::get_payout_detail::ShopifyPaymentsPayoutStatus::Other(_) => {
            PayoutStatus::Scheduled
        }
        super::queries::get_payout_detail::ShopifyPaymentsPayoutStatus::IN_TRANSIT => {
            PayoutStatus::InTransit
        }
        super::queries::get_payout_detail::ShopifyPaymentsPayoutStatus::PAID => PayoutStatus::Paid,
        super::queries::get_payout_detail::ShopifyPaymentsPayoutStatus::FAILED => {
            PayoutStatus::Failed
        }
        super::queries::get_payout_detail::ShopifyPaymentsPayoutStatus::CANCELED => {
            PayoutStatus::Canceled
        }
    }
}

/// Convert GraphQL payout transaction type to domain type.
const fn convert_payout_transaction_type(
    tx_type: &super::queries::get_payout_detail::ShopifyPaymentsPayoutTransactionType,
) -> PayoutTransactionType {
    match tx_type {
        super::queries::get_payout_detail::ShopifyPaymentsPayoutTransactionType::DEPOSIT
        | super::queries::get_payout_detail::ShopifyPaymentsPayoutTransactionType::Other(_) => {
            PayoutTransactionType::Deposit
        }
        super::queries::get_payout_detail::ShopifyPaymentsPayoutTransactionType::WITHDRAWAL => {
            PayoutTransactionType::Withdrawal
        }
    }
}

/// Convert GraphQL balance transaction type to domain type.
const fn convert_transaction_type(
    tx_type: &super::queries::get_payout_transactions::ShopifyPaymentsTransactionType,
) -> BalanceTransactionType {
    use super::queries::get_payout_transactions::ShopifyPaymentsTransactionType;
    match tx_type {
        ShopifyPaymentsTransactionType::CHARGE => BalanceTransactionType::Charge,
        ShopifyPaymentsTransactionType::REFUND | ShopifyPaymentsTransactionType::REFUND_FAILURE => {
            BalanceTransactionType::Refund
        }
        ShopifyPaymentsTransactionType::DISPUTE_WITHDRAWAL
        | ShopifyPaymentsTransactionType::DISPUTE_REVERSAL => BalanceTransactionType::Dispute,
        ShopifyPaymentsTransactionType::TRANSFER
        | ShopifyPaymentsTransactionType::TRANSFER_FAILURE
        | ShopifyPaymentsTransactionType::TRANSFER_CANCEL
        | ShopifyPaymentsTransactionType::TRANSFER_REFUND => BalanceTransactionType::Payout,
        ShopifyPaymentsTransactionType::RESERVED_FUNDS
        | ShopifyPaymentsTransactionType::RESERVED_FUNDS_WITHDRAWAL
        | ShopifyPaymentsTransactionType::RESERVED_FUNDS_REVERSAL => {
            BalanceTransactionType::ReservedFunds
        }
        _ => BalanceTransactionType::Adjustment,
    }
}

/// Convert GraphQL source type to domain type.
const fn convert_source_type(
    src: Option<&super::queries::get_payout_transactions::ShopifyPaymentsSourceType>,
) -> BalanceTransactionSourceType {
    use super::queries::get_payout_transactions::ShopifyPaymentsSourceType;
    match src {
        Some(ShopifyPaymentsSourceType::CHARGE) => BalanceTransactionSourceType::Charge,
        Some(ShopifyPaymentsSourceType::REFUND) => BalanceTransactionSourceType::Refund,
        Some(
            ShopifyPaymentsSourceType::ADJUSTMENT
            | ShopifyPaymentsSourceType::ADJUSTMENT_REVERSAL
            | ShopifyPaymentsSourceType::SYSTEM_ADJUSTMENT,
        ) => BalanceTransactionSourceType::Adjustment,
        Some(ShopifyPaymentsSourceType::DISPUTE) => BalanceTransactionSourceType::Dispute,
        Some(ShopifyPaymentsSourceType::TRANSFER) => BalanceTransactionSourceType::Payout,
        _ => BalanceTransactionSourceType::Unknown,
    }
}

/// Convert GraphQL dispute status to domain type.
const fn convert_dispute_status(
    status: &super::queries::get_disputes::DisputeStatus,
) -> DisputeStatus {
    match status {
        super::queries::get_disputes::DisputeStatus::NEEDS_RESPONSE
        | super::queries::get_disputes::DisputeStatus::Other(_) => DisputeStatus::NeedsResponse,
        super::queries::get_disputes::DisputeStatus::UNDER_REVIEW => DisputeStatus::UnderReview,
        super::queries::get_disputes::DisputeStatus::CHARGE_REFUNDED => {
            DisputeStatus::ChargeRefunded
        }
        super::queries::get_disputes::DisputeStatus::ACCEPTED => DisputeStatus::Accepted,
        super::queries::get_disputes::DisputeStatus::WON => DisputeStatus::Won,
        super::queries::get_disputes::DisputeStatus::LOST => DisputeStatus::Lost,
    }
}

/// Convert GraphQL dispute status (detail query) to domain type.
const fn convert_dispute_status_detail(
    status: &super::queries::get_dispute::DisputeStatus,
) -> DisputeStatus {
    match status {
        super::queries::get_dispute::DisputeStatus::NEEDS_RESPONSE
        | super::queries::get_dispute::DisputeStatus::Other(_) => DisputeStatus::NeedsResponse,
        super::queries::get_dispute::DisputeStatus::UNDER_REVIEW => DisputeStatus::UnderReview,
        super::queries::get_dispute::DisputeStatus::CHARGE_REFUNDED => {
            DisputeStatus::ChargeRefunded
        }
        super::queries::get_dispute::DisputeStatus::ACCEPTED => DisputeStatus::Accepted,
        super::queries::get_dispute::DisputeStatus::WON => DisputeStatus::Won,
        super::queries::get_dispute::DisputeStatus::LOST => DisputeStatus::Lost,
    }
}

/// Convert GraphQL dispute type to domain type.
const fn convert_dispute_type(
    dispute_type: &super::queries::get_disputes::DisputeType,
) -> DisputeType {
    match dispute_type {
        super::queries::get_disputes::DisputeType::CHARGEBACK
        | super::queries::get_disputes::DisputeType::Other(_) => DisputeType::Chargeback,
        super::queries::get_disputes::DisputeType::INQUIRY => DisputeType::Inquiry,
    }
}

/// Convert GraphQL dispute type (detail query) to domain type.
const fn convert_dispute_type_detail(
    dispute_type: &super::queries::get_dispute::DisputeType,
) -> DisputeType {
    match dispute_type {
        super::queries::get_dispute::DisputeType::CHARGEBACK
        | super::queries::get_dispute::DisputeType::Other(_) => DisputeType::Chargeback,
        super::queries::get_dispute::DisputeType::INQUIRY => DisputeType::Inquiry,
    }
}

/// Convert GraphQL bank account status to domain type.
const fn convert_bank_account_status(
    status: &super::queries::get_bank_accounts::ShopifyPaymentsBankAccountStatus,
) -> BankAccountStatus {
    use super::queries::get_bank_accounts::ShopifyPaymentsBankAccountStatus;
    match status {
        ShopifyPaymentsBankAccountStatus::NEW | ShopifyPaymentsBankAccountStatus::Other(_) => {
            BankAccountStatus::Pending
        }
        ShopifyPaymentsBankAccountStatus::VALIDATED
        | ShopifyPaymentsBankAccountStatus::VERIFIED => BankAccountStatus::Verified,
        ShopifyPaymentsBankAccountStatus::ERRORED => BankAccountStatus::Deleted,
    }
}

/// Convert GraphQL payout schedule interval to domain type.
const fn convert_payout_interval(
    interval: &super::queries::get_payout_schedule::ShopifyPaymentsPayoutInterval,
) -> PayoutScheduleInterval {
    match interval {
        super::queries::get_payout_schedule::ShopifyPaymentsPayoutInterval::DAILY
        | super::queries::get_payout_schedule::ShopifyPaymentsPayoutInterval::Other(_) => {
            PayoutScheduleInterval::Daily
        }
        super::queries::get_payout_schedule::ShopifyPaymentsPayoutInterval::WEEKLY => {
            PayoutScheduleInterval::Weekly
        }
        super::queries::get_payout_schedule::ShopifyPaymentsPayoutInterval::MONTHLY => {
            PayoutScheduleInterval::Monthly
        }
        super::queries::get_payout_schedule::ShopifyPaymentsPayoutInterval::MANUAL => {
            PayoutScheduleInterval::Manual
        }
    }
}

/// Order info extracted from dispute for the detail view.
type DisputeOrderInfo = (
    Option<String>,
    Option<String>,
    Option<Money>,
    Option<String>,
    Option<String>,
);

/// Extract order info from dispute order for detail view.
#[allow(deprecated)]
fn extract_dispute_order_info(
    order: Option<&super::queries::get_dispute::GetDisputeNodeOnShopifyPaymentsDisputeOrder>,
) -> DisputeOrderInfo {
    order.map_or((None, None, None, None, None), |o| {
        let total = Some(Money {
            amount: o.total_price_set.shop_money.amount.clone(),
            currency_code: format!("{:?}", o.total_price_set.shop_money.currency_code),
        });
        let (email, name) = o.customer.as_ref().map_or((None, None), |c| {
            let name = match (&c.first_name, &c.last_name) {
                (Some(f), Some(l)) => Some(format!("{f} {l}")),
                (Some(f), None) => Some(f.clone()),
                (None, Some(l)) => Some(l.clone()),
                (None, None) => None,
            };
            (c.email.clone(), name)
        });
        (Some(o.id.clone()), Some(o.name.clone()), total, email, name)
    })
}

/// Convert dispute evidence from GraphQL response to domain type.
#[allow(deprecated)]
fn convert_dispute_evidence(
    e: &super::queries::get_dispute::GetDisputeNodeOnShopifyPaymentsDisputeDisputeEvidence,
) -> DisputeEvidence {
    DisputeEvidence {
        product_description: e.product_description.clone(),
        customer_first_name: e.customer_first_name.clone(),
        customer_last_name: e.customer_last_name.clone(),
        customer_email: e.customer_email_address.clone(),
        shipping_carrier: None,
        shipping_tracking_number: None,
        uncategorized_text: e.uncategorized_text.clone(),
        submitted: e.submitted,
        access_activity_log: e.access_activity_log.clone(),
        cancellation_policy_disclosure: e.cancellation_policy_disclosure.clone(),
        cancellation_rebuttal: e.cancellation_rebuttal.clone(),
        refund_policy_disclosure: e.refund_policy_disclosure.clone(),
        refund_refusal_explanation: e.refund_refusal_explanation.clone(),
        billing_address: e.billing_address.as_ref().map(|a| DisputeAddress {
            address1: a.address1.clone(),
            address2: a.address2.clone(),
            city: a.city.clone(),
            province_code: a.province_code.clone(),
            country_code: a.country_code.clone(),
            zip: a.zip.clone(),
        }),
        shipping_address: e.shipping_address.as_ref().map(|a| DisputeAddress {
            address1: a.address1.clone(),
            address2: a.address2.clone(),
            city: a.city.clone(),
            province_code: a.province_code.clone(),
            country_code: a.country_code.clone(),
            zip: a.zip.clone(),
        }),
        file_uploads: e
            .dispute_file_uploads
            .iter()
            .map(|f| DisputeFileUpload {
                id: f.id.clone(),
                evidence_type: format!("{:?}", f.dispute_evidence_type),
                file_size: Some(f.file_size),
                original_file_name: f.original_file_name.clone(),
                file_type: Some(f.file_type.clone()),
                url: Some(f.url.clone()),
            })
            .collect(),
        fulfillments: e
            .fulfillments
            .iter()
            .map(|f| DisputeFulfillment {
                id: f.id.clone(),
                carrier: f.shipping_carrier.clone(),
                date: f.shipping_date.clone(),
                tracking_number: f.shipping_tracking_number.clone(),
            })
            .collect(),
    }
}

impl AdminClient {
    /// Get a paginated list of payouts from Shopify Payments.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of payouts to return
    /// * `after` - Cursor for pagination
    /// * `sort_key` - Column to sort by (see `PayoutSortKey` for supported options)
    /// * `reverse` - Whether to reverse the sort order
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or Shopify Payments is not enabled.
    #[instrument(skip(self))]
    pub async fn get_payouts(
        &self,
        first: i64,
        after: Option<String>,
        sort_key: Option<PayoutSortKey>,
        reverse: bool,
    ) -> Result<PayoutConnection, AdminShopifyError> {
        use super::queries::get_payouts::PayoutSortKeys;

        let client_side_sort = sort_key.is_some_and(|sk| !sk.is_shopify_native());

        let api_sort_key = sort_key.and_then(|sk| match sk {
            PayoutSortKey::IssuedAt => Some(PayoutSortKeys::ISSUED_AT),
            PayoutSortKey::Status => Some(PayoutSortKeys::STATUS),
            PayoutSortKey::Amount => Some(PayoutSortKeys::AMOUNT),
            PayoutSortKey::ChargeGross => Some(PayoutSortKeys::CHARGE_GROSS),
            PayoutSortKey::FeeAmount => Some(PayoutSortKeys::FEE_AMOUNT),
            PayoutSortKey::Id => Some(PayoutSortKeys::ID),
            PayoutSortKey::TransactionType => None,
        });

        tracing::debug!(
            input_sort_key = ?sort_key,
            api_sort_key = ?api_sort_key,
            client_side_sort = client_side_sort,
            reverse = reverse,
            "get_payouts sort parameters"
        );

        let variables = super::queries::get_payouts::Variables {
            first: Some(first),
            after,
            sort_key: api_sort_key,
            reverse: Some(if client_side_sort { false } else { reverse }),
        };

        let response = self.execute::<GetPayouts>(variables).await?;

        let Some(account) = response.shopify_payments_account else {
            return Err(AdminShopifyError::NotFound(
                "Shopify Payments is not enabled for this store".to_string(),
            ));
        };

        #[allow(deprecated)]
        let mut payouts: Vec<Payout> = account
            .payouts
            .edges
            .into_iter()
            .map(|e| {
                let p = e.node;
                Payout {
                    id: p.id,
                    legacy_resource_id: Some(p.legacy_resource_id.clone()),
                    status: convert_payout_status(&p.status),
                    net: Money {
                        amount: p.net.amount,
                        currency_code: format!("{:?}", p.net.currency_code),
                    },
                    issued_at: Some(p.issued_at),
                }
            })
            .collect();

        if client_side_sort && let Some(sk) = sort_key {
            sort_payouts(&mut payouts, sk, reverse);
        }

        let balance = account.balance.into_iter().next().map(|b| Money {
            amount: b.amount,
            currency_code: format!("{:?}", b.currency_code),
        });

        Ok(PayoutConnection {
            payouts,
            page_info: PageInfo {
                has_next_page: account.payouts.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: account.payouts.page_info.end_cursor,
            },
            balance,
        })
    }

    /// Get a single payout by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The payout's global ID (e.g., `gid://shopify/ShopifyPaymentsPayout/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the payout is not found.
    #[instrument(skip(self), fields(payout_id = %id))]
    pub async fn get_payout(&self, id: &str) -> Result<Payout, AdminShopifyError> {
        let variables = super::queries::get_payout::Variables { id: id.to_string() };

        let response = self.execute::<GetPayout>(variables).await?;

        let Some(node) = response.node else {
            return Err(AdminShopifyError::NotFound(format!(
                "Payout {id} not found"
            )));
        };

        use super::queries::get_payout::GetPayoutNode;
        match node {
            GetPayoutNode::ShopifyPaymentsPayout(p) => Ok(Payout {
                id: p.id,
                legacy_resource_id: Some(p.legacy_resource_id.clone()),
                status: convert_payout_status_single(&p.status),
                net: Money {
                    amount: p.net.amount,
                    currency_code: format!("{:?}", p.net.currency_code),
                },
                issued_at: Some(p.issued_at),
            }),
            _ => Err(AdminShopifyError::NotFound(format!(
                "Node {id} is not a payout"
            ))),
        }
    }

    /// Get detailed payout information including summary breakdown.
    ///
    /// # Errors
    ///
    /// Returns an error if the payout is not found or the API request fails.
    #[instrument(skip(self), fields(payout_id = %id))]
    pub async fn get_payout_detail(&self, id: &str) -> Result<PayoutDetail, AdminShopifyError> {
        let variables = super::queries::get_payout_detail::Variables { id: id.to_string() };

        let response = self.execute::<GetPayoutDetail>(variables).await?;

        let Some(node) = response.node else {
            return Err(AdminShopifyError::NotFound(format!(
                "Payout {id} not found"
            )));
        };

        use super::queries::get_payout_detail::GetPayoutDetailNode;
        match node {
            GetPayoutDetailNode::ShopifyPaymentsPayout(p) => {
                let s = &p.summary;
                let summary = Some(PayoutSummary {
                    charges_gross: Money {
                        amount: s.charges_gross.amount.clone(),
                        currency_code: format!("{:?}", s.charges_gross.currency_code),
                    },
                    charges_fee: Money {
                        amount: s.charges_fee.amount.clone(),
                        currency_code: format!("{:?}", s.charges_fee.currency_code),
                    },
                    refunds_fee_gross: Money {
                        amount: s.refunds_fee_gross.amount.clone(),
                        currency_code: format!("{:?}", s.refunds_fee_gross.currency_code),
                    },
                    refunds_fee: Money {
                        amount: s.refunds_fee.amount.clone(),
                        currency_code: format!("{:?}", s.refunds_fee.currency_code),
                    },
                    adjustments_gross: Money {
                        amount: s.adjustments_gross.amount.clone(),
                        currency_code: format!("{:?}", s.adjustments_gross.currency_code),
                    },
                    adjustments_fee: Money {
                        amount: s.adjustments_fee.amount.clone(),
                        currency_code: format!("{:?}", s.adjustments_fee.currency_code),
                    },
                    reserved_funds_gross: Money {
                        amount: s.reserved_funds_gross.amount.clone(),
                        currency_code: format!("{:?}", s.reserved_funds_gross.currency_code),
                    },
                    reserved_funds_fee: Money {
                        amount: s.reserved_funds_fee.amount.clone(),
                        currency_code: format!("{:?}", s.reserved_funds_fee.currency_code),
                    },
                    retried_payouts_gross: Money {
                        amount: s.retried_payouts_gross.amount.clone(),
                        currency_code: format!("{:?}", s.retried_payouts_gross.currency_code),
                    },
                    retried_payouts_fee: Money {
                        amount: s.retried_payouts_fee.amount.clone(),
                        currency_code: format!("{:?}", s.retried_payouts_fee.currency_code),
                    },
                });

                #[allow(deprecated)]
                let gross = Money {
                    amount: p.gross.amount.clone(),
                    currency_code: format!("{:?}", p.gross.currency_code),
                };

                Ok(PayoutDetail {
                    id: p.id,
                    legacy_resource_id: Some(p.legacy_resource_id.clone()),
                    status: convert_payout_status_detail(&p.status),
                    transaction_type: convert_payout_transaction_type(&p.transaction_type),
                    net: Money {
                        amount: p.net.amount,
                        currency_code: format!("{:?}", p.net.currency_code),
                    },
                    gross,
                    issued_at: Some(p.issued_at),
                    summary,
                })
            }
            _ => Err(AdminShopifyError::NotFound(format!(
                "Node {id} is not a payout"
            ))),
        }
    }

    /// Get balance transactions for a payout.
    ///
    /// # Errors
    ///
    /// Returns an error if Shopify Payments is not enabled or the API request fails.
    #[instrument(skip(self))]
    pub async fn get_payout_transactions(
        &self,
        first: i64,
        after: Option<String>,
        payout_id: Option<String>,
        payout_date: Option<String>,
    ) -> Result<BalanceTransactionConnection, AdminShopifyError> {
        let query = payout_date.map(|date| {
            let date_only = date.split('T').next().unwrap_or(&date);
            format!("payout_date:{date_only}")
        });

        tracing::info!(
            first = first,
            payout_id = ?payout_id,
            query = ?query,
            "Fetching balance transactions"
        );

        let variables = super::queries::get_payout_transactions::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetPayoutTransactions>(variables).await?;

        let Some(account) = response.shopify_payments_account else {
            return Err(AdminShopifyError::NotFound(
                "Shopify Payments is not enabled for this store".to_string(),
            ));
        };

        if payout_id.is_some() && !account.balance_transactions.edges.is_empty() {
            let sample_ids: Vec<_> = account
                .balance_transactions
                .edges
                .iter()
                .take(3)
                .map(|e| e.node.associated_payout.id.as_deref().unwrap_or("None"))
                .collect();
            tracing::info!(
                filter_payout_id = ?payout_id,
                sample_transaction_payout_ids = ?sample_ids,
                "Comparing payout IDs for filtering"
            );
        }

        let transactions: Vec<BalanceTransaction> = account
            .balance_transactions
            .edges
            .into_iter()
            .filter_map(|e| {
                let t = e.node;

                if let Some(ref filter_payout_id) = payout_id {
                    let tx_payout_id = t.associated_payout.id.as_deref();
                    if tx_payout_id != Some(filter_payout_id.as_str()) {
                        return None;
                    }
                }

                let (order_id, order_name) = t
                    .associated_order
                    .map_or((None, None), |o| (Some(o.id), Some(o.name)));
                Some(BalanceTransaction {
                    id: t.id,
                    amount: Money {
                        amount: t.amount.amount,
                        currency_code: format!("{:?}", t.amount.currency_code),
                    },
                    fee: Money {
                        amount: t.fee.amount,
                        currency_code: format!("{:?}", t.fee.currency_code),
                    },
                    net: Money {
                        amount: t.net.amount,
                        currency_code: format!("{:?}", t.net.currency_code),
                    },
                    transaction_date: t.transaction_date,
                    transaction_type: convert_transaction_type(&t.type_),
                    source_type: convert_source_type(t.source_type.as_ref()),
                    order_id,
                    order_name,
                })
            })
            .collect();

        tracing::info!(
            transaction_count = transactions.len(),
            payout_filter = ?payout_id,
            has_next_page = account.balance_transactions.page_info.has_next_page,
            "Balance transactions fetched"
        );

        Ok(BalanceTransactionConnection {
            transactions,
            page_info: PageInfo {
                has_next_page: account.balance_transactions.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: account.balance_transactions.page_info.end_cursor,
            },
        })
    }

    /// Get a paginated list of disputes.
    ///
    /// # Errors
    ///
    /// Returns an error if Shopify Payments is not enabled or the API request fails.
    #[instrument(skip(self))]
    pub async fn get_disputes(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<DisputeConnection, AdminShopifyError> {
        let variables = super::queries::get_disputes::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetDisputes>(variables).await?;

        let Some(account) = response.shopify_payments_account else {
            return Err(AdminShopifyError::NotFound(
                "Shopify Payments is not enabled for this store".to_string(),
            ));
        };

        let disputes: Vec<Dispute> = account
            .disputes
            .edges
            .into_iter()
            .map(|e| {
                let d = e.node;
                let (order_id, order_name) =
                    d.order.map_or((None, None), |o| (Some(o.id), Some(o.name)));
                Dispute {
                    id: d.id,
                    legacy_resource_id: Some(d.legacy_resource_id.clone()),
                    status: convert_dispute_status(&d.status),
                    kind: convert_dispute_type(&d.type_),
                    amount: Money {
                        amount: d.amount.amount,
                        currency_code: format!("{:?}", d.amount.currency_code),
                    },
                    initiated_at: d.initiated_at,
                    evidence_due_by: d.evidence_due_by,
                    finalized_on: d.finalized_on,
                    reason_details: Some(DisputeReasonDetails {
                        reason: format!("{:?}", d.reason_details.reason),
                        network_reason_code: d.reason_details.network_reason_code,
                    }),
                    order_id,
                    order_name,
                }
            })
            .collect();

        Ok(DisputeConnection {
            disputes,
            page_info: PageInfo {
                has_next_page: account.disputes.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: account.disputes.page_info.end_cursor,
            },
        })
    }

    /// Get a single dispute with full details including evidence.
    ///
    /// # Errors
    ///
    /// Returns an error if Shopify Payments is not enabled, the dispute is not
    /// found, or the API request fails.
    #[instrument(skip(self), fields(dispute_id = %id))]
    pub async fn get_dispute(&self, id: &str) -> Result<DisputeDetail, AdminShopifyError> {
        let variables = super::queries::get_dispute::Variables { id: id.to_string() };
        let response = self.execute::<GetDispute>(variables).await?;

        let Some(node) = response.node else {
            return Err(AdminShopifyError::NotFound(format!(
                "Dispute {id} not found"
            )));
        };

        use super::queries::get_dispute::GetDisputeNode;
        match node {
            GetDisputeNode::ShopifyPaymentsDispute(d) => {
                let (order_id, order_name, order_total, customer_email, customer_name) =
                    extract_dispute_order_info(d.order.as_ref());
                let evidence = Some(convert_dispute_evidence(&d.dispute_evidence));
                let reason_details = Some(DisputeReasonDetails {
                    reason: format!("{:?}", d.reason_details.reason),
                    network_reason_code: d.reason_details.network_reason_code.clone(),
                });

                let dispute = Dispute {
                    id: d.id.clone(),
                    legacy_resource_id: Some(d.legacy_resource_id.clone()),
                    status: convert_dispute_status_detail(&d.status),
                    kind: convert_dispute_type_detail(&d.type_),
                    amount: Money {
                        amount: d.amount.amount.clone(),
                        currency_code: format!("{:?}", d.amount.currency_code),
                    },
                    initiated_at: d.initiated_at.clone(),
                    evidence_due_by: d.evidence_due_by.clone(),
                    finalized_on: d.finalized_on.clone(),
                    reason_details,
                    order_id,
                    order_name,
                };

                Ok(DisputeDetail {
                    dispute,
                    customer_email,
                    customer_name,
                    order_total,
                    evidence,
                    evidence_sent_on: d.evidence_sent_on,
                })
            }
            _ => Err(AdminShopifyError::NotFound(format!(
                "Node {id} is not a dispute"
            ))),
        }
    }

    /// Get connected bank accounts.
    ///
    /// # Errors
    ///
    /// Returns an error if Shopify Payments is not enabled or the API request fails.
    #[instrument(skip(self))]
    pub async fn get_bank_accounts(&self) -> Result<Vec<BankAccount>, AdminShopifyError> {
        let variables = super::queries::get_bank_accounts::Variables { first: Some(10) };

        let response = self.execute::<GetBankAccounts>(variables).await?;

        let Some(account) = response.shopify_payments_account else {
            return Err(AdminShopifyError::NotFound(
                "Shopify Payments is not enabled for this store".to_string(),
            ));
        };

        let accounts: Vec<BankAccount> = account
            .bank_accounts
            .edges
            .into_iter()
            .map(|e| {
                let b = e.node;
                BankAccount {
                    id: b.id,
                    account_number_last_digits: b.account_number_last_digits,
                    bank_name: b.bank_name,
                    country: format!("{:?}", b.country),
                    currency: format!("{:?}", b.currency),
                    status: convert_bank_account_status(&b.status),
                    created_at: Some(b.created_at),
                }
            })
            .collect();

        Ok(accounts)
    }

    /// Get the payout schedule.
    ///
    /// # Errors
    ///
    /// Returns an error if Shopify Payments is not enabled or the API request fails.
    #[instrument(skip(self))]
    pub async fn get_payout_schedule(&self) -> Result<PayoutSchedule, AdminShopifyError> {
        let variables = super::queries::get_payout_schedule::Variables {};

        let response = self.execute::<GetPayoutSchedule>(variables).await?;

        let Some(account) = response.shopify_payments_account else {
            return Err(AdminShopifyError::NotFound(
                "Shopify Payments is not enabled for this store".to_string(),
            ));
        };

        let schedule = account.payout_schedule;
        Ok(PayoutSchedule {
            interval: convert_payout_interval(&schedule.interval),
            monthly_anchor: schedule.monthly_anchor,
            weekly_anchor: schedule.weekly_anchor.map(|w| format!("{w:?}")),
        })
    }
}
