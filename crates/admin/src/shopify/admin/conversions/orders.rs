//! Order type conversion functions.

use crate::shopify::types::{
    Address, FinancialStatus, Fulfillment, FulfillmentStatus, Money, Order, OrderConnection,
    OrderLineItem, PageInfo, TrackingInfo,
};

use super::super::queries::{get_order, get_orders};
use super::currency_code_to_string;

// =============================================================================
// Shared helpers
// =============================================================================

fn default_money() -> Money {
    Money {
        amount: "0.00".to_string(),
        currency_code: "USD".to_string(),
    }
}

struct OrderPricing {
    subtotal: Money,
    shipping: Money,
    tax: Money,
    total: Money,
    discounts: Money,
    currency: String,
}

// =============================================================================
// GetOrder conversions
// =============================================================================

pub fn convert_order(order: get_order::GetOrderOrder) -> Order {
    let pricing = build_pricing_single(&order);
    Order {
        id: order.id,
        name: order.name,
        number: order.number,
        created_at: order.created_at,
        updated_at: order.updated_at,
        financial_status: order
            .display_financial_status
            .as_ref()
            .map(convert_financial_single),
        fulfillment_status: Some(convert_fulfillment_single(
            &order.display_fulfillment_status,
        )),
        fully_paid: order.fully_paid,
        test: order.test,
        email: order.email,
        phone: order.phone,
        note: order.note,
        subtotal_price: pricing.subtotal,
        total_shipping_price: pricing.shipping,
        total_tax: pricing.tax,
        total_price: pricing.total,
        total_discounts: pricing.discounts,
        currency_code: pricing.currency,
        line_items: order
            .line_items
            .edges
            .into_iter()
            .map(|e| convert_line_item_single(e.node))
            .collect(),
        fulfillments: order
            .fulfillments
            .into_iter()
            .map(convert_fulfillment_obj_single)
            .collect(),
        billing_address: order.billing_address.map(convert_billing_single),
        shipping_address: order.shipping_address.map(convert_shipping_single),
        customer_id: order.customer.map(|c| c.id),
    }
}

fn build_pricing_single(order: &get_order::GetOrderOrder) -> OrderPricing {
    OrderPricing {
        subtotal: order
            .subtotal_price_set
            .as_ref()
            .map_or_else(default_money, |s| Money {
                amount: s.shop_money.amount.clone(),
                currency_code: currency_code_to_string(s.shop_money.currency_code.clone()),
            }),
        shipping: Money {
            amount: order.total_shipping_price_set.shop_money.amount.clone(),
            currency_code: currency_code_to_string(
                order
                    .total_shipping_price_set
                    .shop_money
                    .currency_code
                    .clone(),
            ),
        },
        tax: order
            .total_tax_set
            .as_ref()
            .map_or_else(default_money, |s| Money {
                amount: s.shop_money.amount.clone(),
                currency_code: currency_code_to_string(s.shop_money.currency_code.clone()),
            }),
        total: Money {
            amount: order.total_price_set.shop_money.amount.clone(),
            currency_code: currency_code_to_string(
                order.total_price_set.shop_money.currency_code.clone(),
            ),
        },
        discounts: order
            .total_discounts_set
            .as_ref()
            .map_or_else(default_money, |s| Money {
                amount: s.shop_money.amount.clone(),
                currency_code: currency_code_to_string(s.shop_money.currency_code.clone()),
            }),
        currency: currency_code_to_string(order.currency_code.clone()),
    }
}

const fn convert_financial_single(s: &get_order::OrderDisplayFinancialStatus) -> FinancialStatus {
    match s {
        get_order::OrderDisplayFinancialStatus::AUTHORIZED => FinancialStatus::Authorized,
        get_order::OrderDisplayFinancialStatus::PAID => FinancialStatus::Paid,
        get_order::OrderDisplayFinancialStatus::PARTIALLY_PAID => FinancialStatus::PartiallyPaid,
        get_order::OrderDisplayFinancialStatus::REFUNDED => FinancialStatus::Refunded,
        get_order::OrderDisplayFinancialStatus::PARTIALLY_REFUNDED => {
            FinancialStatus::PartiallyRefunded
        }
        get_order::OrderDisplayFinancialStatus::VOIDED => FinancialStatus::Voided,
        get_order::OrderDisplayFinancialStatus::EXPIRED => FinancialStatus::Expired,
        get_order::OrderDisplayFinancialStatus::PENDING
        | get_order::OrderDisplayFinancialStatus::Other(_) => FinancialStatus::Pending,
    }
}

const fn convert_fulfillment_single(
    s: &get_order::OrderDisplayFulfillmentStatus,
) -> FulfillmentStatus {
    match s {
        get_order::OrderDisplayFulfillmentStatus::PARTIALLY_FULFILLED => {
            FulfillmentStatus::PartiallyFulfilled
        }
        get_order::OrderDisplayFulfillmentStatus::FULFILLED => FulfillmentStatus::Fulfilled,
        get_order::OrderDisplayFulfillmentStatus::ON_HOLD => FulfillmentStatus::OnHold,
        get_order::OrderDisplayFulfillmentStatus::IN_PROGRESS => FulfillmentStatus::InProgress,
        get_order::OrderDisplayFulfillmentStatus::RESTOCKED => FulfillmentStatus::Restocked,
        get_order::OrderDisplayFulfillmentStatus::SCHEDULED => FulfillmentStatus::Scheduled,
        get_order::OrderDisplayFulfillmentStatus::PENDING_FULFILLMENT => {
            FulfillmentStatus::PendingFulfillment
        }
        get_order::OrderDisplayFulfillmentStatus::OPEN => FulfillmentStatus::Open,
        get_order::OrderDisplayFulfillmentStatus::REQUEST_DECLINED => {
            FulfillmentStatus::RequestDeclined
        }
        get_order::OrderDisplayFulfillmentStatus::UNFULFILLED
        | get_order::OrderDisplayFulfillmentStatus::Other(_) => FulfillmentStatus::Unfulfilled,
    }
}

fn convert_line_item_single(item: get_order::GetOrderOrderLineItemsEdgesNode) -> OrderLineItem {
    OrderLineItem {
        id: item.id,
        title: item.title,
        variant_title: item.variant_title,
        sku: item.sku,
        quantity: item.quantity,
        original_unit_price: Money {
            amount: item.original_unit_price_set.shop_money.amount,
            currency_code: currency_code_to_string(
                item.original_unit_price_set.shop_money.currency_code,
            ),
        },
        discounted_unit_price: Money {
            amount: item.discounted_unit_price_set.shop_money.amount,
            currency_code: currency_code_to_string(
                item.discounted_unit_price_set.shop_money.currency_code,
            ),
        },
        total_discount: Money {
            amount: item.total_discount_set.shop_money.amount,
            currency_code: currency_code_to_string(
                item.total_discount_set.shop_money.currency_code,
            ),
        },
        product_id: item.product.map(|p| p.id),
        variant_id: item.variant.map(|v| v.id),
        requires_shipping: item.requires_shipping,
        is_gift_card: item.is_gift_card,
    }
}

fn convert_fulfillment_obj_single(f: get_order::GetOrderOrderFulfillments) -> Fulfillment {
    Fulfillment {
        id: f.id,
        status: format!("{:?}", f.status),
        tracking_info: f
            .tracking_info
            .into_iter()
            .map(|t| TrackingInfo {
                company: t.company,
                number: t.number,
                url: t.url,
            })
            .collect(),
        created_at: f.created_at,
        updated_at: f.updated_at,
    }
}

fn convert_billing_single(a: get_order::GetOrderOrderBillingAddress) -> Address {
    Address {
        address1: a.address1,
        address2: a.address2,
        city: a.city,
        province_code: a.province_code,
        country_code: a.country_code_v2.map(|c| format!("{c:?}")),
        zip: a.zip,
        first_name: a.first_name,
        last_name: a.last_name,
        company: a.company,
        phone: a.phone,
    }
}

fn convert_shipping_single(a: get_order::GetOrderOrderShippingAddress) -> Address {
    Address {
        address1: a.address1,
        address2: a.address2,
        city: a.city,
        province_code: a.province_code,
        country_code: a.country_code_v2.map(|c| format!("{c:?}")),
        zip: a.zip,
        first_name: a.first_name,
        last_name: a.last_name,
        company: a.company,
        phone: a.phone,
    }
}

// =============================================================================
// GetOrders conversions
// =============================================================================

pub fn convert_order_connection(conn: get_orders::GetOrdersOrders) -> OrderConnection {
    OrderConnection {
        orders: conn
            .edges
            .into_iter()
            .map(|e| convert_order_list(e.node))
            .collect(),
        page_info: PageInfo {
            has_next_page: conn.page_info.has_next_page,
            has_previous_page: conn.page_info.has_previous_page,
            start_cursor: conn.page_info.start_cursor,
            end_cursor: conn.page_info.end_cursor,
        },
    }
}

fn convert_order_list(order: get_orders::GetOrdersOrdersEdgesNode) -> Order {
    let pricing = build_pricing_list(&order);
    Order {
        id: order.id,
        name: order.name,
        number: order.number,
        created_at: order.created_at,
        updated_at: order.updated_at,
        financial_status: order
            .display_financial_status
            .as_ref()
            .map(convert_financial_list),
        fulfillment_status: Some(convert_fulfillment_list(&order.display_fulfillment_status)),
        fully_paid: order.fully_paid,
        test: order.test,
        email: order.email,
        phone: order.phone,
        note: order.note,
        subtotal_price: pricing.subtotal,
        total_shipping_price: pricing.shipping,
        total_tax: pricing.tax,
        total_price: pricing.total,
        total_discounts: pricing.discounts,
        currency_code: pricing.currency,
        line_items: order
            .line_items
            .edges
            .into_iter()
            .map(|e| convert_line_item_list(e.node))
            .collect(),
        fulfillments: order
            .fulfillments
            .into_iter()
            .map(convert_fulfillment_obj_list)
            .collect(),
        billing_address: order.billing_address.map(convert_billing_list),
        shipping_address: order.shipping_address.map(convert_shipping_list),
        customer_id: order.customer.map(|c| c.id),
    }
}

fn build_pricing_list(order: &get_orders::GetOrdersOrdersEdgesNode) -> OrderPricing {
    OrderPricing {
        subtotal: order
            .subtotal_price_set
            .as_ref()
            .map_or_else(default_money, |s| Money {
                amount: s.shop_money.amount.clone(),
                currency_code: currency_code_to_string(s.shop_money.currency_code.clone()),
            }),
        shipping: Money {
            amount: order.total_shipping_price_set.shop_money.amount.clone(),
            currency_code: currency_code_to_string(
                order
                    .total_shipping_price_set
                    .shop_money
                    .currency_code
                    .clone(),
            ),
        },
        tax: order
            .total_tax_set
            .as_ref()
            .map_or_else(default_money, |s| Money {
                amount: s.shop_money.amount.clone(),
                currency_code: currency_code_to_string(s.shop_money.currency_code.clone()),
            }),
        total: Money {
            amount: order.total_price_set.shop_money.amount.clone(),
            currency_code: currency_code_to_string(
                order.total_price_set.shop_money.currency_code.clone(),
            ),
        },
        discounts: order
            .total_discounts_set
            .as_ref()
            .map_or_else(default_money, |s| Money {
                amount: s.shop_money.amount.clone(),
                currency_code: currency_code_to_string(s.shop_money.currency_code.clone()),
            }),
        currency: currency_code_to_string(order.currency_code.clone()),
    }
}

const fn convert_financial_list(s: &get_orders::OrderDisplayFinancialStatus) -> FinancialStatus {
    match s {
        get_orders::OrderDisplayFinancialStatus::AUTHORIZED => FinancialStatus::Authorized,
        get_orders::OrderDisplayFinancialStatus::PAID => FinancialStatus::Paid,
        get_orders::OrderDisplayFinancialStatus::PARTIALLY_PAID => FinancialStatus::PartiallyPaid,
        get_orders::OrderDisplayFinancialStatus::REFUNDED => FinancialStatus::Refunded,
        get_orders::OrderDisplayFinancialStatus::PARTIALLY_REFUNDED => {
            FinancialStatus::PartiallyRefunded
        }
        get_orders::OrderDisplayFinancialStatus::VOIDED => FinancialStatus::Voided,
        get_orders::OrderDisplayFinancialStatus::EXPIRED => FinancialStatus::Expired,
        get_orders::OrderDisplayFinancialStatus::PENDING
        | get_orders::OrderDisplayFinancialStatus::Other(_) => FinancialStatus::Pending,
    }
}

const fn convert_fulfillment_list(
    s: &get_orders::OrderDisplayFulfillmentStatus,
) -> FulfillmentStatus {
    match s {
        get_orders::OrderDisplayFulfillmentStatus::PARTIALLY_FULFILLED => {
            FulfillmentStatus::PartiallyFulfilled
        }
        get_orders::OrderDisplayFulfillmentStatus::FULFILLED => FulfillmentStatus::Fulfilled,
        get_orders::OrderDisplayFulfillmentStatus::ON_HOLD => FulfillmentStatus::OnHold,
        get_orders::OrderDisplayFulfillmentStatus::IN_PROGRESS => FulfillmentStatus::InProgress,
        get_orders::OrderDisplayFulfillmentStatus::RESTOCKED => FulfillmentStatus::Restocked,
        get_orders::OrderDisplayFulfillmentStatus::SCHEDULED => FulfillmentStatus::Scheduled,
        get_orders::OrderDisplayFulfillmentStatus::PENDING_FULFILLMENT => {
            FulfillmentStatus::PendingFulfillment
        }
        get_orders::OrderDisplayFulfillmentStatus::OPEN => FulfillmentStatus::Open,
        get_orders::OrderDisplayFulfillmentStatus::REQUEST_DECLINED => {
            FulfillmentStatus::RequestDeclined
        }
        get_orders::OrderDisplayFulfillmentStatus::UNFULFILLED
        | get_orders::OrderDisplayFulfillmentStatus::Other(_) => FulfillmentStatus::Unfulfilled,
    }
}

fn convert_line_item_list(
    item: get_orders::GetOrdersOrdersEdgesNodeLineItemsEdgesNode,
) -> OrderLineItem {
    OrderLineItem {
        id: item.id,
        title: item.title,
        variant_title: item.variant_title,
        sku: item.sku,
        quantity: item.quantity,
        original_unit_price: Money {
            amount: item.original_unit_price_set.shop_money.amount,
            currency_code: currency_code_to_string(
                item.original_unit_price_set.shop_money.currency_code,
            ),
        },
        discounted_unit_price: Money {
            amount: item.discounted_unit_price_set.shop_money.amount,
            currency_code: currency_code_to_string(
                item.discounted_unit_price_set.shop_money.currency_code,
            ),
        },
        total_discount: Money {
            amount: item.total_discount_set.shop_money.amount,
            currency_code: currency_code_to_string(
                item.total_discount_set.shop_money.currency_code,
            ),
        },
        product_id: item.product.map(|p| p.id),
        variant_id: item.variant.map(|v| v.id),
        requires_shipping: item.requires_shipping,
        is_gift_card: item.is_gift_card,
    }
}

fn convert_fulfillment_obj_list(
    f: get_orders::GetOrdersOrdersEdgesNodeFulfillments,
) -> Fulfillment {
    Fulfillment {
        id: f.id,
        status: format!("{:?}", f.status),
        tracking_info: f
            .tracking_info
            .into_iter()
            .map(|t| TrackingInfo {
                company: t.company,
                number: t.number,
                url: t.url,
            })
            .collect(),
        created_at: f.created_at,
        updated_at: f.updated_at,
    }
}

fn convert_billing_list(a: get_orders::GetOrdersOrdersEdgesNodeBillingAddress) -> Address {
    Address {
        address1: a.address1,
        address2: a.address2,
        city: a.city,
        province_code: a.province_code,
        country_code: a.country_code_v2.map(|c| format!("{c:?}")),
        zip: a.zip,
        first_name: a.first_name,
        last_name: a.last_name,
        company: a.company,
        phone: a.phone,
    }
}

fn convert_shipping_list(a: get_orders::GetOrdersOrdersEdgesNodeShippingAddress) -> Address {
    Address {
        address1: a.address1,
        address2: a.address2,
        city: a.city,
        province_code: a.province_code,
        country_code: a.country_code_v2.map(|c| format!("{c:?}")),
        zip: a.zip,
        first_name: a.first_name,
        last_name: a.last_name,
        company: a.company,
        phone: a.phone,
    }
}
