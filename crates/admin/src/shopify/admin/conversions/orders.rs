//! Order type conversion functions.

use crate::shopify::types::{
    Address, DeliveryCategory, FinancialStatus, Fulfillment, FulfillmentStatus, Money, Order,
    OrderChannelInfo, OrderConnection, OrderLineItem, OrderListConnection, OrderListItem,
    OrderReturnStatus, OrderRisk, OrderRiskLevel, OrderShippingLine, PageInfo, TrackingInfo,
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
        id: None, // Order addresses don't have IDs
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
        id: None, // Order addresses don't have IDs
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
        id: None, // Order addresses don't have IDs
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
        id: None, // Order addresses don't have IDs
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
// Extended OrderListConnection conversions (for data table view)
// =============================================================================

/// Convert `GetOrders` response to `OrderListConnection` with extended fields.
pub fn convert_order_list_connection(conn: get_orders::GetOrdersOrders) -> OrderListConnection {
    OrderListConnection {
        orders: conn
            .edges
            .into_iter()
            .map(|e| convert_order_list_item(e.node))
            .collect(),
        page_info: PageInfo {
            has_next_page: conn.page_info.has_next_page,
            has_previous_page: conn.page_info.has_previous_page,
            start_cursor: conn.page_info.start_cursor,
            end_cursor: conn.page_info.end_cursor,
        },
    }
}

// Allow deprecated field usage: Shopify risks field is deprecated but we still use it
// for backwards compatibility until OrderRiskAssessment is fully rolled out.
#[allow(deprecated)]
fn convert_order_list_item(order: get_orders::GetOrdersOrdersEdgesNode) -> OrderListItem {
    let pricing = build_pricing_list(&order);

    // Calculate total items quantity from line items
    let total_items_quantity: i64 = order.line_items.edges.iter().map(|e| e.node.quantity).sum();

    // Convert risks (using deprecated risks field for now)
    let risks: Vec<OrderRisk> = order
        .risks
        .iter()
        .filter_map(|r| {
            r.level.as_ref().map(|level| OrderRisk {
                level: convert_risk_level(level),
                message: r.message.clone(),
            })
        })
        .collect();

    // Convert channel info
    let channel_info = order.channel_information.and_then(|ci| {
        ci.channel_definition.map(|cd| OrderChannelInfo {
            channel_name: Some(cd.channel_name),
        })
    });

    // Convert shipping line
    let shipping_line = order.shipping_line.map(|sl| OrderShippingLine {
        title: sl.title,
        delivery_category: sl
            .delivery_category
            .as_deref()
            .map(convert_delivery_category),
    });

    let cancelled = order.cancelled_at.is_some();
    let closed = order.closed_at.is_some();

    OrderListItem {
        id: order.id,
        name: order.name,
        number: order.number,
        created_at: order.created_at,
        updated_at: order.updated_at,
        closed_at: order.closed_at,
        cancelled_at: order.cancelled_at,
        financial_status: order
            .display_financial_status
            .as_ref()
            .map(convert_financial_list),
        fulfillment_status: Some(convert_fulfillment_list(&order.display_fulfillment_status)),
        return_status: Some(convert_return_status(&order.return_status)),
        fully_paid: order.fully_paid,
        cancelled,
        closed,
        test: order.test,
        email: order.email,
        phone: order.phone,
        note: order.note,
        tags: order.tags,
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
        total_items_quantity,
        fulfillments: order
            .fulfillments
            .into_iter()
            .map(convert_fulfillment_obj_list)
            .collect(),
        billing_address: order.billing_address.map(convert_billing_list),
        shipping_address: order.shipping_address.map(convert_shipping_list),
        customer_id: order.customer.as_ref().map(|c| c.id.clone()),
        customer_name: order.customer.map(|c| c.display_name),
        risks,
        channel_info,
        shipping_line,
        discount_codes: order.discount_codes,
    }
}

const fn convert_risk_level(level: &get_orders::OrderRiskLevel) -> OrderRiskLevel {
    match level {
        get_orders::OrderRiskLevel::MEDIUM => OrderRiskLevel::Medium,
        get_orders::OrderRiskLevel::HIGH => OrderRiskLevel::High,
        get_orders::OrderRiskLevel::LOW | get_orders::OrderRiskLevel::Other(_) => {
            OrderRiskLevel::Low
        }
    }
}

const fn convert_return_status(status: &get_orders::OrderReturnStatus) -> OrderReturnStatus {
    match status {
        get_orders::OrderReturnStatus::RETURN_REQUESTED => OrderReturnStatus::ReturnRequested,
        get_orders::OrderReturnStatus::IN_PROGRESS => OrderReturnStatus::InProgress,
        get_orders::OrderReturnStatus::RETURNED
        | get_orders::OrderReturnStatus::INSPECTION_COMPLETE => OrderReturnStatus::Returned,
        get_orders::OrderReturnStatus::NO_RETURN
        | get_orders::OrderReturnStatus::RETURN_FAILED
        | get_orders::OrderReturnStatus::Other(_) => OrderReturnStatus::NoReturn,
    }
}

fn convert_delivery_category(cat: &str) -> DeliveryCategory {
    match cat {
        "SHIPPING" => DeliveryCategory::Shipping,
        "LOCAL_DELIVERY" => DeliveryCategory::LocalDelivery,
        "PICKUP" => DeliveryCategory::Pickup,
        _ => DeliveryCategory::None,
    }
}

// =============================================================================
// Fulfillment Order conversions
// =============================================================================

use super::super::queries::get_fulfillment_orders;
use crate::shopify::types::{FulfillmentOrder, FulfillmentOrderLineItem};

/// Convert `GetFulfillmentOrders` response to `Vec<FulfillmentOrder>`.
pub fn convert_fulfillment_orders(
    order: Option<get_fulfillment_orders::GetFulfillmentOrdersOrder>,
) -> Vec<FulfillmentOrder> {
    let Some(order) = order else {
        return vec![];
    };

    order
        .fulfillment_orders
        .edges
        .into_iter()
        .map(|e| convert_fulfillment_order(e.node))
        .collect()
}

fn convert_fulfillment_order(
    fo: get_fulfillment_orders::GetFulfillmentOrdersOrderFulfillmentOrdersEdgesNode,
) -> FulfillmentOrder {
    let (location_id, location_name) = fo
        .assigned_location
        .location
        .map_or((None, None), |loc| (Some(loc.id), Some(loc.name)));

    FulfillmentOrder {
        id: fo.id,
        status: format!("{:?}", fo.status),
        location_id,
        location_name,
        line_items: fo
            .line_items
            .edges
            .into_iter()
            .map(|e| convert_fulfillment_order_line_item(e.node))
            .collect(),
    }
}

fn convert_fulfillment_order_line_item(
    item: get_fulfillment_orders::GetFulfillmentOrdersOrderFulfillmentOrdersEdgesNodeLineItemsEdgesNode,
) -> FulfillmentOrderLineItem {
    FulfillmentOrderLineItem {
        id: item.id,
        title: item.line_item.title,
        variant_title: item.line_item.variant_title,
        sku: item.line_item.sku,
        total_quantity: item.total_quantity,
        remaining_quantity: item.remaining_quantity,
    }
}

// =============================================================================
// Order Edit conversions
// =============================================================================

use super::super::queries::order_edit_begin;
use crate::shopify::types::{
    CalculatedDiscountAllocation, CalculatedLineItem, CalculatedOrder, CalculatedShippingLine,
    CalculatedShippingLineStagedStatus, Image,
};

/// Convert `OrderEditBegin` response to `CalculatedOrder`.
pub fn convert_calculated_order(
    data: order_edit_begin::OrderEditBeginOrderEditBeginCalculatedOrder,
) -> CalculatedOrder {
    CalculatedOrder {
        id: data.id,
        original_order_id: data.original_order.id,
        original_order_name: data.original_order.name,
        line_items: data
            .line_items
            .edges
            .into_iter()
            .map(|e| convert_calculated_line_item_begin(e.node))
            .collect(),
        added_line_items: data
            .added_line_items
            .edges
            .into_iter()
            .map(|e| convert_calculated_line_item_added(e.node))
            .collect(),
        shipping_lines: data
            .shipping_lines
            .into_iter()
            .map(convert_calculated_shipping_line)
            .collect(),
        subtotal_price: data
            .subtotal_price_set
            .map_or_else(default_money, |s| Money {
                amount: s.shop_money.amount,
                currency_code: currency_code_to_string(s.shop_money.currency_code),
            }),
        total_price: Money {
            amount: data.total_price_set.shop_money.amount,
            currency_code: currency_code_to_string(data.total_price_set.shop_money.currency_code),
        },
        total_outstanding: Money {
            amount: data.total_outstanding_set.shop_money.amount,
            currency_code: currency_code_to_string(
                data.total_outstanding_set.shop_money.currency_code,
            ),
        },
        subtotal_line_items_quantity: data.subtotal_line_items_quantity,
        notification_preview_title: Some(data.notification_preview_title),
    }
}

fn convert_calculated_line_item_begin(
    item: order_edit_begin::OrderEditBeginOrderEditBeginCalculatedOrderLineItemsEdgesNode,
) -> CalculatedLineItem {
    CalculatedLineItem {
        id: item.id,
        title: item.title,
        variant_title: item.variant_title,
        sku: item.sku,
        quantity: item.quantity,
        editable_quantity: item.editable_quantity,
        editable_quantity_before_changes: item.editable_quantity_before_changes,
        restockable: item.restockable,
        restocking: item.restocking,
        has_staged_line_item_discount: item.has_staged_line_item_discount,
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
        editable_subtotal: Money {
            amount: item.editable_subtotal_set.shop_money.amount,
            currency_code: currency_code_to_string(
                item.editable_subtotal_set.shop_money.currency_code,
            ),
        },
        image: item.image.map(|img| Image {
            id: None,
            url: img.url,
            alt_text: img.alt_text,
            width: None,
            height: None,
        }),
        variant_id: item.variant.map(|v| v.id),
        discount_allocations: item
            .calculated_discount_allocations
            .into_iter()
            .map(|alloc| CalculatedDiscountAllocation {
                allocated_amount: Money {
                    amount: alloc.allocated_amount_set.shop_money.amount,
                    currency_code: currency_code_to_string(
                        alloc.allocated_amount_set.shop_money.currency_code,
                    ),
                },
                description: None,
            })
            .collect(),
    }
}

fn convert_calculated_line_item_added(
    item: order_edit_begin::OrderEditBeginOrderEditBeginCalculatedOrderAddedLineItemsEdgesNode,
) -> CalculatedLineItem {
    CalculatedLineItem {
        id: item.id,
        title: item.title,
        variant_title: item.variant_title,
        sku: item.sku,
        quantity: item.quantity,
        editable_quantity: item.editable_quantity,
        editable_quantity_before_changes: 0, // Added items start at 0
        restockable: false,
        restocking: false,
        has_staged_line_item_discount: false,
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
        editable_subtotal: Money {
            amount: item.editable_subtotal_set.shop_money.amount,
            currency_code: currency_code_to_string(
                item.editable_subtotal_set.shop_money.currency_code,
            ),
        },
        image: item.image.map(|img| Image {
            id: None,
            url: img.url,
            alt_text: img.alt_text,
            width: None,
            height: None,
        }),
        variant_id: item.variant.map(|v| v.id),
        discount_allocations: vec![],
    }
}

fn convert_calculated_shipping_line(
    line: order_edit_begin::OrderEditBeginOrderEditBeginCalculatedOrderShippingLines,
) -> CalculatedShippingLine {
    CalculatedShippingLine {
        id: line.id,
        title: line.title,
        price: Money {
            amount: line.price.shop_money.amount,
            currency_code: currency_code_to_string(line.price.shop_money.currency_code),
        },
        staged_status: convert_shipping_staged_status(&line.staged_status),
    }
}

const fn convert_shipping_staged_status(
    status: &order_edit_begin::CalculatedShippingLineStagedStatus,
) -> CalculatedShippingLineStagedStatus {
    match status {
        order_edit_begin::CalculatedShippingLineStagedStatus::ADDED => {
            CalculatedShippingLineStagedStatus::Added
        }
        order_edit_begin::CalculatedShippingLineStagedStatus::REMOVED => {
            CalculatedShippingLineStagedStatus::Removed
        }
        order_edit_begin::CalculatedShippingLineStagedStatus::NONE
        | order_edit_begin::CalculatedShippingLineStagedStatus::Other(_) => {
            CalculatedShippingLineStagedStatus::None
        }
    }
}
