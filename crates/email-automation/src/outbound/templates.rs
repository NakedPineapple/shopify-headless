//! Askama template structs for transactional email rendering.
//!
//! Each email type has an HTML and a plain text variant. Template files
//! live in `crates/email-automation/templates/email/`.

use askama::Template;

use super::{
    AddressData, DeliveryNotificationData, LineItemData, LowStockAlertData, LowStockVariantData,
    OrderConfirmationData, ReviewRequestData, ShippingUpdateData,
};

// ---------------------------------------------------------------------------
// Order Confirmation
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/order_confirmation.html")]
pub struct OrderConfirmationHtml {
    pub customer_name: String,
    pub order_name: String,
    pub order_date: String,
    pub line_items: Vec<TemplateLineItem>,
    pub subtotal: String,
    pub shipping: String,
    pub tax: String,
    pub total: String,
    pub shipping_address: Option<TemplateAddress>,
}

#[derive(Template)]
#[template(path = "email/order_confirmation.txt")]
pub struct OrderConfirmationText {
    pub customer_name: String,
    pub order_name: String,
    pub order_date: String,
    pub line_items: Vec<TemplateLineItem>,
    pub subtotal: String,
    pub shipping: String,
    pub tax: String,
    pub total: String,
    pub shipping_address: Option<TemplateAddress>,
}

/// Line item representation for templates.
pub struct TemplateLineItem {
    pub title: String,
    pub variant: Option<String>,
    pub quantity: i64,
    pub price: String,
}

/// Address representation for templates.
pub struct TemplateAddress {
    pub name: String,
    pub address1: String,
    pub address2: Option<String>,
    pub city: String,
    pub province: String,
    pub zip: String,
    pub country: String,
}

impl OrderConfirmationHtml {
    pub fn from_data(data: &OrderConfirmationData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            order_name: data.order_name.clone(),
            order_date: data.order_date.clone(),
            line_items: convert_line_items(&data.line_items),
            subtotal: data.subtotal.clone(),
            shipping: data.shipping.clone(),
            tax: data.tax.clone(),
            total: data.total.clone(),
            shipping_address: data.shipping_address.as_ref().map(convert_address),
        }
    }
}

impl OrderConfirmationText {
    pub fn from_data(data: &OrderConfirmationData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            order_name: data.order_name.clone(),
            order_date: data.order_date.clone(),
            line_items: convert_line_items(&data.line_items),
            subtotal: data.subtotal.clone(),
            shipping: data.shipping.clone(),
            tax: data.tax.clone(),
            total: data.total.clone(),
            shipping_address: data.shipping_address.as_ref().map(convert_address),
        }
    }
}

// ---------------------------------------------------------------------------
// Shipping Update
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/shipping_update.html")]
pub struct ShippingUpdateHtml {
    pub customer_name: String,
    pub order_name: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub items: Vec<String>,
}

#[derive(Template)]
#[template(path = "email/shipping_update.txt")]
pub struct ShippingUpdateText {
    pub customer_name: String,
    pub order_name: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub items: Vec<String>,
}

impl ShippingUpdateHtml {
    pub fn from_data(data: &ShippingUpdateData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            order_name: data.order_name.clone(),
            carrier: data.carrier.clone(),
            tracking_number: data.tracking_number.clone(),
            tracking_url: data.tracking_url.clone(),
            items: data.items.clone(),
        }
    }
}

impl ShippingUpdateText {
    pub fn from_data(data: &ShippingUpdateData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            order_name: data.order_name.clone(),
            carrier: data.carrier.clone(),
            tracking_number: data.tracking_number.clone(),
            tracking_url: data.tracking_url.clone(),
            items: data.items.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Delivery Notification
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/delivery_notification.html")]
pub struct DeliveryNotificationHtml {
    pub customer_name: String,
    pub order_name: String,
}

#[derive(Template)]
#[template(path = "email/delivery_notification.txt")]
pub struct DeliveryNotificationText {
    pub customer_name: String,
    pub order_name: String,
}

impl DeliveryNotificationHtml {
    pub fn from_data(data: &DeliveryNotificationData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            order_name: data.order_name.clone(),
        }
    }
}

impl DeliveryNotificationText {
    pub fn from_data(data: &DeliveryNotificationData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            order_name: data.order_name.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Review Request
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/review_request.html")]
pub struct ReviewRequestHtml {
    pub customer_name: String,
    pub product_names: Vec<String>,
    pub store_url: String,
}

#[derive(Template)]
#[template(path = "email/review_request.txt")]
pub struct ReviewRequestText {
    pub customer_name: String,
    pub product_names: Vec<String>,
    pub store_url: String,
}

impl ReviewRequestHtml {
    pub fn from_data(data: &ReviewRequestData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            product_names: data.product_names.clone(),
            store_url: data.store_url.clone(),
        }
    }
}

impl ReviewRequestText {
    pub fn from_data(data: &ReviewRequestData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            product_names: data.product_names.clone(),
            store_url: data.store_url.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Low Stock Alert
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/low_stock_alert.html")]
pub struct LowStockAlertHtml {
    pub product_title: String,
    pub total_inventory: i32,
    pub threshold: i32,
    pub variants: Vec<TemplateStockVariant>,
}

#[derive(Template)]
#[template(path = "email/low_stock_alert.txt")]
pub struct LowStockAlertText {
    pub product_title: String,
    pub total_inventory: i32,
    pub threshold: i32,
    pub variants: Vec<TemplateStockVariant>,
}

/// Variant inventory for low stock alert templates.
pub struct TemplateStockVariant {
    pub title: String,
    pub sku: Option<String>,
    pub inventory_quantity: i32,
}

impl LowStockAlertHtml {
    pub fn from_data(data: &LowStockAlertData) -> Self {
        Self {
            product_title: data.product_title.clone(),
            total_inventory: data.total_inventory,
            threshold: data.threshold,
            variants: convert_stock_variants(&data.variants),
        }
    }
}

impl LowStockAlertText {
    pub fn from_data(data: &LowStockAlertData) -> Self {
        Self {
            product_title: data.product_title.clone(),
            total_inventory: data.total_inventory,
            threshold: data.threshold,
            variants: convert_stock_variants(&data.variants),
        }
    }
}

fn convert_stock_variants(variants: &[LowStockVariantData]) -> Vec<TemplateStockVariant> {
    variants
        .iter()
        .map(|v| TemplateStockVariant {
            title: v.title.clone(),
            sku: v.sku.clone(),
            inventory_quantity: v.inventory_quantity,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Subscription Renewal Reminder
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/subscription_renewal.html")]
pub struct SubscriptionRenewalHtml {
    pub customer_name: String,
    pub renewal_date: String,
    pub product_names: Vec<String>,
}

#[derive(Template)]
#[template(path = "email/subscription_renewal.txt")]
pub struct SubscriptionRenewalText {
    pub customer_name: String,
    pub renewal_date: String,
    pub product_names: Vec<String>,
}

impl SubscriptionRenewalHtml {
    pub fn from_data(data: &super::SubscriptionRenewalData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            renewal_date: data.renewal_date.clone(),
            product_names: data.product_names.clone(),
        }
    }
}

impl SubscriptionRenewalText {
    pub fn from_data(data: &super::SubscriptionRenewalData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            renewal_date: data.renewal_date.clone(),
            product_names: data.product_names.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Payment Failure
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/payment_failure.html")]
pub struct PaymentFailureHtml {
    pub customer_name: String,
    pub product_names: Vec<String>,
}

#[derive(Template)]
#[template(path = "email/payment_failure.txt")]
pub struct PaymentFailureText {
    pub customer_name: String,
    pub product_names: Vec<String>,
}

impl PaymentFailureHtml {
    pub fn from_data(data: &super::PaymentFailureData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            product_names: data.product_names.clone(),
        }
    }
}

impl PaymentFailureText {
    pub fn from_data(data: &super::PaymentFailureData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            product_names: data.product_names.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Subscription Win-Back
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/subscription_winback.html")]
pub struct WinBackHtml {
    pub customer_name: String,
    pub product_names: Vec<String>,
    pub store_url: String,
}

#[derive(Template)]
#[template(path = "email/subscription_winback.txt")]
pub struct WinBackText {
    pub customer_name: String,
    pub product_names: Vec<String>,
    pub store_url: String,
}

impl WinBackHtml {
    pub fn from_data(data: &super::WinBackData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            product_names: data.product_names.clone(),
            store_url: data.store_url.clone(),
        }
    }
}

impl WinBackText {
    pub fn from_data(data: &super::WinBackData) -> Self {
        Self {
            customer_name: data.customer_name.clone(),
            product_names: data.product_names.clone(),
            store_url: data.store_url.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn convert_line_items(items: &[LineItemData]) -> Vec<TemplateLineItem> {
    items
        .iter()
        .map(|item| TemplateLineItem {
            title: item.title.clone(),
            variant: item.variant.clone(),
            quantity: item.quantity,
            price: item.price.clone(),
        })
        .collect()
}

fn convert_address(addr: &AddressData) -> TemplateAddress {
    TemplateAddress {
        name: addr.name.clone(),
        address1: addr.address1.clone(),
        address2: addr.address2.clone(),
        city: addr.city.clone(),
        province: addr.province.clone(),
        zip: addr.zip.clone(),
        country: addr.country.clone(),
    }
}
