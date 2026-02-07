//! Conversions from `graphql_client`-generated types to domain types.

use crate::shopify::types::Money;

use super::queries::{
    get_customer, get_order, get_order_for_return, get_orders, get_store_credit, get_subscription,
    get_subscriptions, get_upcoming_billing_cycles,
};
use super::types::{
    Address, BillingCycleStatus, Customer, Order, OrderDetail, OrderForReturn, OrderLineItem,
    OrderLineItemImage, OrderLineItemWithReasons, Return, ReturnReasonDefinition, ReturnStatus,
    SubscriptionBillingCycle, SubscriptionBillingPolicy, SubscriptionContract,
    SubscriptionContractStatus, SubscriptionLine, SubscriptionLineImage,
};

// ─────────────────────────────────────────────────────────────────────────────
// Customer conversions
// ─────────────────────────────────────────────────────────────────────────────

fn convert_address_fields(a: &get_customer::AddressFields) -> Address {
    Address {
        id: a.id.clone(),
        first_name: a.first_name.clone(),
        last_name: a.last_name.clone(),
        company: a.company.clone(),
        address1: a.address1.clone(),
        address2: a.address2.clone(),
        city: a.city.clone(),
        province: a.province.clone(),
        province_code: a.zone_code.clone(),
        country: a.country.clone(),
        country_code: a.territory_code.as_ref().map(|c| format!("{c:?}")),
        zip: a.zip.clone(),
        phone: a.phone_number.clone(),
    }
}

pub fn convert_get_customer(data: get_customer::ResponseData) -> Customer {
    let c = data.customer;
    Customer {
        id: c.id,
        email: c.email_address.and_then(|e| e.email_address),
        first_name: c.first_name,
        last_name: c.last_name,
        phone: c.phone_number.map(|p| p.phone_number),
        default_address: c.default_address.as_ref().map(convert_address_fields),
    }
}

pub fn convert_customer_update(
    data: super::queries::customer_update::ResponseData,
) -> Result<Customer, Vec<String>> {
    let result = data
        .customer_update
        .ok_or_else(|| vec!["No response from customerUpdate".to_string()])?;

    let errors: Vec<String> = result
        .user_errors
        .iter()
        .map(|e| e.message.clone())
        .collect();

    if !errors.is_empty() {
        return Err(errors);
    }

    let c = result
        .customer
        .ok_or_else(|| vec!["No customer returned".to_string()])?;

    Ok(Customer {
        id: c.id,
        email: c.email_address.and_then(|e| e.email_address),
        first_name: c.first_name,
        last_name: c.last_name,
        phone: c.phone_number.map(|p| p.phone_number),
        default_address: c.default_address.as_ref().map(|a| Address {
            id: a.id.clone(),
            first_name: a.first_name.clone(),
            last_name: a.last_name.clone(),
            company: a.company.clone(),
            address1: a.address1.clone(),
            address2: a.address2.clone(),
            city: a.city.clone(),
            province: a.province.clone(),
            province_code: a.zone_code.clone(),
            country: a.country.clone(),
            country_code: a.territory_code.as_ref().map(|c| format!("{c:?}")),
            zip: a.zip.clone(),
            phone: a.phone_number.clone(),
        }),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Address conversions
// ─────────────────────────────────────────────────────────────────────────────

use super::queries::get_addresses;

fn convert_addresses_address_fields(a: &get_addresses::AddressFields) -> Address {
    Address {
        id: a.id.clone(),
        first_name: a.first_name.clone(),
        last_name: a.last_name.clone(),
        company: a.company.clone(),
        address1: a.address1.clone(),
        address2: a.address2.clone(),
        city: a.city.clone(),
        province: a.province.clone(),
        province_code: a.zone_code.clone(),
        country: a.country.clone(),
        country_code: a.territory_code.as_ref().map(|c| format!("{c:?}")),
        zip: a.zip.clone(),
        phone: a.phone_number.clone(),
    }
}

pub fn convert_get_addresses(data: &get_addresses::ResponseData) -> Vec<Address> {
    data.customer
        .addresses
        .edges
        .iter()
        .map(|e| convert_addresses_address_fields(&e.node))
        .collect()
}

use super::queries::customer_address_create;

pub fn convert_address_create(
    data: customer_address_create::ResponseData,
) -> Result<Address, Vec<String>> {
    let result = data
        .customer_address_create
        .ok_or_else(|| vec!["No response from customerAddressCreate".to_string()])?;

    let errors: Vec<String> = result
        .user_errors
        .iter()
        .map(|e| e.message.clone())
        .collect();

    if !errors.is_empty() {
        return Err(errors);
    }

    let a = result
        .customer_address
        .ok_or_else(|| vec!["No address returned".to_string()])?;

    Ok(Address {
        id: a.id,
        first_name: a.first_name,
        last_name: a.last_name,
        company: a.company,
        address1: a.address1,
        address2: a.address2,
        city: a.city,
        province: a.province,
        province_code: a.zone_code,
        country: a.country,
        country_code: a.territory_code.as_ref().map(|c| format!("{c:?}")),
        zip: a.zip,
        phone: a.phone_number,
    })
}

use super::queries::customer_address_update;

pub fn convert_address_update(
    data: customer_address_update::ResponseData,
) -> Result<Address, Vec<String>> {
    let result = data
        .customer_address_update
        .ok_or_else(|| vec!["No response from customerAddressUpdate".to_string()])?;

    let errors: Vec<String> = result
        .user_errors
        .iter()
        .map(|e| e.message.clone())
        .collect();

    if !errors.is_empty() {
        return Err(errors);
    }

    let a = result
        .customer_address
        .ok_or_else(|| vec!["No address returned".to_string()])?;

    Ok(Address {
        id: a.id,
        first_name: a.first_name,
        last_name: a.last_name,
        company: a.company,
        address1: a.address1,
        address2: a.address2,
        city: a.city,
        province: a.province,
        province_code: a.zone_code,
        country: a.country,
        country_code: a.territory_code.as_ref().map(|c| format!("{c:?}")),
        zip: a.zip,
        phone: a.phone_number,
    })
}

use super::queries::customer_address_delete;

pub fn convert_address_delete(
    data: customer_address_delete::ResponseData,
) -> Result<(), Vec<String>> {
    let result = data
        .customer_address_delete
        .ok_or_else(|| vec!["No response from customerAddressDelete".to_string()])?;

    let errors: Vec<String> = result
        .user_errors
        .iter()
        .map(|e| e.message.clone())
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Order conversions
// ─────────────────────────────────────────────────────────────────────────────

fn convert_money(m: &get_orders::MoneyFields) -> Money {
    Money {
        amount: m.amount.clone(),
        currency_code: format!("{:?}", m.currency_code),
    }
}

pub fn convert_get_orders(data: &get_orders::ResponseData) -> Vec<Order> {
    data.customer
        .orders
        .edges
        .iter()
        .map(|e| {
            let o = &e.node;
            Order {
                id: o.id.clone(),
                name: o.name.clone(),
                number: o.number,
                processed_at: o.processed_at.clone(),
                financial_status: o.financial_status.as_ref().map(|s| format!("{s:?}")),
                fulfillment_status: Some(format!("{:?}", o.fulfillment_status)),
                total_price: convert_money(&o.total_price),
            }
        })
        .collect()
}

fn convert_order_money(m: &get_order::MoneyFields) -> Money {
    Money {
        amount: m.amount.clone(),
        currency_code: format!("{:?}", m.currency_code),
    }
}

const fn convert_order_return_status(s: &get_order::ReturnStatus) -> ReturnStatus {
    match s {
        get_order::ReturnStatus::REQUESTED => ReturnStatus::Requested,
        get_order::ReturnStatus::OPEN | get_order::ReturnStatus::Other(_) => ReturnStatus::Open,
        get_order::ReturnStatus::CLOSED => ReturnStatus::Closed,
        get_order::ReturnStatus::CANCELED => ReturnStatus::Canceled,
        get_order::ReturnStatus::DECLINED => ReturnStatus::Declined,
    }
}

pub fn convert_get_order(data: get_order::ResponseData) -> Option<OrderDetail> {
    let o = data.order?;

    let line_items = o
        .line_items
        .edges
        .iter()
        .map(|e| {
            let item = &e.node;
            OrderLineItem {
                id: item.id.clone(),
                title: item.title.clone(),
                quantity: item.quantity,
                unit_price: item
                    .unit_price
                    .as_ref()
                    .map(|u| convert_order_money(&u.price)),
                total_price: item.total_price.as_ref().map(convert_order_money),
                image: item.image.as_ref().map(|img| OrderLineItemImage {
                    url: img.url.clone(),
                    alt_text: img.alt_text.clone(),
                }),
                variant_title: item.variant_title.clone(),
            }
        })
        .collect();

    let shipping_address = o.shipping_address.as_ref().map(|a| Address {
        id: a.id.clone(),
        first_name: a.first_name.clone(),
        last_name: a.last_name.clone(),
        company: a.company.clone(),
        address1: a.address1.clone(),
        address2: a.address2.clone(),
        city: a.city.clone(),
        province: a.province.clone(),
        province_code: a.zone_code.clone(),
        country: a.country.clone(),
        country_code: a.territory_code.as_ref().map(|c| format!("{c:?}")),
        zip: a.zip.clone(),
        phone: a.phone_number.clone(),
    });

    let returns = o
        .returns
        .edges
        .iter()
        .map(|e| Return {
            id: e.node.id.clone(),
            name: e.node.name.clone(),
            status: convert_order_return_status(&e.node.status),
        })
        .collect();

    Some(OrderDetail {
        id: o.id,
        name: o.name,
        number: o.number,
        processed_at: o.processed_at,
        financial_status: o.financial_status.as_ref().map(|s| format!("{s:?}")),
        fulfillment_status: Some(format!("{:?}", o.fulfillment_status)),
        total_price: Money {
            amount: o.total_price.amount.clone(),
            currency_code: format!("{:?}", o.total_price.currency_code),
        },
        subtotal: o.subtotal.as_ref().map(convert_order_money),
        total_shipping: Money {
            amount: o.total_shipping.amount.clone(),
            currency_code: format!("{:?}", o.total_shipping.currency_code),
        },
        total_tax: o.total_tax.as_ref().map(convert_order_money),
        line_items,
        shipping_address,
        returns,
    })
}

pub fn convert_get_order_for_return(
    data: get_order_for_return::ResponseData,
) -> Option<OrderForReturn> {
    let o = data.order?;

    let line_items = o
        .line_items
        .edges
        .iter()
        .map(|e| {
            let item = &e.node;
            let reasons = item
                .suggested_return_reason_definitions
                .as_ref()
                .map(|defs| {
                    defs.edges
                        .iter()
                        .map(|r| ReturnReasonDefinition {
                            id: r.node.id.clone(),
                            name: r.node.name.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            OrderLineItemWithReasons {
                id: item.id.clone(),
                title: item.title.clone(),
                quantity: item.quantity,
                image: item.image.as_ref().map(|img| OrderLineItemImage {
                    url: img.url.clone(),
                    alt_text: img.alt_text.clone(),
                }),
                variant_title: item.variant_title.clone(),
                suggested_reasons: reasons,
            }
        })
        .collect();

    Some(OrderForReturn {
        id: o.id,
        name: o.name,
        line_items,
    })
}

pub fn convert_order_request_return(
    data: super::queries::order_request_return::ResponseData,
) -> Result<(), Vec<String>> {
    let result = data
        .order_request_return
        .ok_or_else(|| vec!["No response from orderRequestReturn".to_string()])?;

    let errors: Vec<String> = result
        .user_errors
        .iter()
        .map(|e| e.message.clone())
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Subscription conversions
// ─────────────────────────────────────────────────────────────────────────────

const fn convert_subscription_status(
    s: &get_subscriptions::SubscriptionContractSubscriptionStatus,
) -> SubscriptionContractStatus {
    use get_subscriptions::SubscriptionContractSubscriptionStatus as S;
    match s {
        S::ACTIVE | S::Other(_) => SubscriptionContractStatus::Active,
        S::PAUSED => SubscriptionContractStatus::Paused,
        S::CANCELLED => SubscriptionContractStatus::Cancelled,
        S::EXPIRED => SubscriptionContractStatus::Expired,
        S::FAILED => SubscriptionContractStatus::Failed,
        S::STALE => SubscriptionContractStatus::Stale,
    }
}

fn convert_subscription_fields(s: &get_subscriptions::SubscriptionFields) -> SubscriptionContract {
    let lines = s
        .lines
        .edges
        .iter()
        .map(|e| {
            let line = &e.node;
            SubscriptionLine {
                id: line.id.clone(),
                name: line.name.clone(),
                quantity: line.quantity,
                current_price: Money {
                    amount: line.current_price.amount.clone(),
                    currency_code: format!("{:?}", line.current_price.currency_code),
                },
                image: line.image.as_ref().map(|img| SubscriptionLineImage {
                    url: img.url.clone(),
                    alt_text: img.alt_text.clone(),
                }),
            }
        })
        .collect();

    SubscriptionContract {
        id: s.id.clone(),
        status: convert_subscription_status(&s.status),
        created_at: s.created_at.clone(),
        next_billing_date: s.next_billing_date.clone(),
        billing_policy: SubscriptionBillingPolicy {
            interval: format!("{:?}", s.billing_policy.interval),
            interval_count: s.billing_policy.interval_count.as_ref().map(|c| c.count),
        },
        delivery_price: Money {
            amount: s.delivery_price.amount.clone(),
            currency_code: format!("{:?}", s.delivery_price.currency_code),
        },
        lines,
    }
}

pub fn convert_get_subscriptions(
    data: &get_subscriptions::ResponseData,
) -> Vec<SubscriptionContract> {
    data.customer
        .subscription_contracts
        .edges
        .iter()
        .map(|e| convert_subscription_fields(&e.node))
        .collect()
}

const fn convert_single_subscription_status(
    s: &get_subscription::SubscriptionContractSubscriptionStatus,
) -> SubscriptionContractStatus {
    use get_subscription::SubscriptionContractSubscriptionStatus as S;
    match s {
        S::ACTIVE | S::Other(_) => SubscriptionContractStatus::Active,
        S::PAUSED => SubscriptionContractStatus::Paused,
        S::CANCELLED => SubscriptionContractStatus::Cancelled,
        S::EXPIRED => SubscriptionContractStatus::Expired,
        S::FAILED => SubscriptionContractStatus::Failed,
        S::STALE => SubscriptionContractStatus::Stale,
    }
}

pub fn convert_get_subscription(
    data: &get_subscription::ResponseData,
) -> Option<SubscriptionContract> {
    let s = data.customer.subscription_contract.as_ref()?;

    let lines = s
        .lines
        .edges
        .iter()
        .map(|e| {
            let line = &e.node;
            SubscriptionLine {
                id: line.id.clone(),
                name: line.name.clone(),
                quantity: line.quantity,
                current_price: Money {
                    amount: line.current_price.amount.clone(),
                    currency_code: format!("{:?}", line.current_price.currency_code),
                },
                image: line.image.as_ref().map(|img| SubscriptionLineImage {
                    url: img.url.clone(),
                    alt_text: img.alt_text.clone(),
                }),
            }
        })
        .collect();

    Some(SubscriptionContract {
        id: s.id.clone(),
        status: convert_single_subscription_status(&s.status),
        created_at: s.created_at.clone(),
        next_billing_date: s.next_billing_date.clone(),
        billing_policy: SubscriptionBillingPolicy {
            interval: format!("{:?}", s.billing_policy.interval),
            interval_count: s.billing_policy.interval_count.as_ref().map(|c| c.count),
        },
        delivery_price: Money {
            amount: s.delivery_price.amount.clone(),
            currency_code: format!("{:?}", s.delivery_price.currency_code),
        },
        lines,
    })
}

/// Extract user errors from an `Option<T>` mutation result, unwrapping the Option first.
fn unwrap_mutation_errors<T, F>(result: Option<T>, extract: F) -> Result<(), Vec<String>>
where
    F: FnOnce(&T) -> Vec<String>,
{
    let inner = result.ok_or_else(|| vec!["No mutation response".to_string()])?;
    let errors = extract(&inner);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

use super::queries::pause_subscription;

pub fn convert_pause_subscription(
    data: pause_subscription::ResponseData,
) -> Result<(), Vec<String>> {
    unwrap_mutation_errors(data.subscription_contract_pause, |r| {
        r.user_errors.iter().map(|e| e.message.clone()).collect()
    })
}

use super::queries::cancel_subscription;

pub fn convert_cancel_subscription(
    data: cancel_subscription::ResponseData,
) -> Result<(), Vec<String>> {
    unwrap_mutation_errors(data.subscription_contract_cancel, |r| {
        r.user_errors.iter().map(|e| e.message.clone()).collect()
    })
}

use super::queries::activate_subscription;

pub fn convert_activate_subscription(
    data: activate_subscription::ResponseData,
) -> Result<(), Vec<String>> {
    unwrap_mutation_errors(data.subscription_contract_activate, |r| {
        r.user_errors.iter().map(|e| e.message.clone()).collect()
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Billing Cycle conversions
// ─────────────────────────────────────────────────────────────────────────────

const fn convert_billing_cycle_status(
    s: &get_upcoming_billing_cycles::SubscriptionBillingCycleBillingCycleStatus,
) -> BillingCycleStatus {
    use get_upcoming_billing_cycles::SubscriptionBillingCycleBillingCycleStatus as S;
    match s {
        S::BILLED => BillingCycleStatus::Billed,
        S::UNBILLED | S::Other(_) => BillingCycleStatus::Unbilled,
    }
}

pub fn convert_get_upcoming_billing_cycles(
    data: get_upcoming_billing_cycles::ResponseData,
) -> Vec<SubscriptionBillingCycle> {
    data.customer
        .subscription_contract
        .map(|c| {
            c.upcoming_billing_cycles
                .edges
                .iter()
                .map(|e| SubscriptionBillingCycle {
                    billing_attempt_expected_date: e.node.billing_attempt_expected_date.clone(),
                    cycle_index: e.node.cycle_index,
                    skipped: e.node.skipped,
                    status: convert_billing_cycle_status(&e.node.status),
                })
                .collect()
        })
        .unwrap_or_default()
}

use super::queries::skip_billing_cycle;

pub fn convert_skip_billing_cycle(
    data: skip_billing_cycle::ResponseData,
) -> Result<(), Vec<String>> {
    unwrap_mutation_errors(data.subscription_billing_cycle_skip, |r| {
        r.user_errors.iter().map(|e| e.message.clone()).collect()
    })
}

use super::queries::unskip_billing_cycle;

pub fn convert_unskip_billing_cycle(
    data: unskip_billing_cycle::ResponseData,
) -> Result<(), Vec<String>> {
    unwrap_mutation_errors(data.subscription_billing_cycle_unskip, |r| {
        r.user_errors.iter().map(|e| e.message.clone()).collect()
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Store Credit conversions
// ─────────────────────────────────────────────────────────────────────────────

pub fn convert_get_store_credit(data: &get_store_credit::ResponseData) -> Option<Money> {
    data.customer
        .store_credit_accounts
        .edges
        .first()
        .map(|e| Money {
            amount: e.node.balance.amount.clone(),
            currency_code: format!("{:?}", e.node.balance.currency_code),
        })
}
