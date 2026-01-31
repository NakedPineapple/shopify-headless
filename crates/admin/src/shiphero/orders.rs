//! Order and shipment query methods for `ShipHero` API.
//!
//! Provides methods to fetch orders awaiting fulfillment, shipment history,
//! and order details from the warehouse.

use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::ShipHeroError;
use super::client::ShipHeroClient;
use super::queries::{GetOrder, GetOrderHistory, GetPendingOrders, GetShipment, GetShipments};

// =============================================================================
// Domain Types
// =============================================================================

/// An order in the `ShipHero` warehouse system.
#[derive(Debug, Clone, Serialize)]
pub struct WarehouseOrder {
    /// `ShipHero` order ID.
    pub id: String,
    /// Legacy numeric ID.
    pub legacy_id: Option<i64>,
    /// Order number (from Shopify).
    pub order_number: Option<String>,
    /// Partner/external order ID.
    pub partner_order_id: Option<String>,
    /// Shop/store name.
    pub shop_name: Option<String>,
    /// Current fulfillment status.
    pub fulfillment_status: Option<String>,
    /// Order date.
    pub order_date: Option<String>,
    /// Total order price.
    pub total_price: Option<String>,
    /// Shipping address.
    pub shipping_address: Option<OrderAddress>,
    /// Line items in the order.
    pub line_items: Vec<OrderLineItem>,
    /// Order holds (fraud, address, payment, operator).
    pub holds: Option<OrderHolds>,
}

/// An order with full details from `ShipHero`.
#[derive(Debug, Clone, Serialize)]
pub struct WarehouseOrderDetail {
    /// `ShipHero` order ID.
    pub id: String,
    /// Legacy numeric ID.
    pub legacy_id: Option<i64>,
    /// Order number.
    pub order_number: Option<String>,
    /// Partner/external order ID.
    pub partner_order_id: Option<String>,
    /// Shop/store name.
    pub shop_name: Option<String>,
    /// Current fulfillment status.
    pub fulfillment_status: Option<String>,
    /// Order date.
    pub order_date: Option<String>,
    /// Total order price.
    pub total_price: Option<String>,
    /// Subtotal before discounts/tax.
    pub subtotal: Option<String>,
    /// Total discounts applied.
    pub total_discounts: Option<String>,
    /// Total tax.
    pub total_tax: Option<String>,
    /// Shipping method info.
    pub shipping_lines: Option<ShippingLine>,
    /// Shipping address.
    pub shipping_address: Option<OrderAddress>,
    /// Billing address.
    pub billing_address: Option<OrderAddress>,
    /// Line items.
    pub line_items: Vec<OrderLineItem>,
    /// Order holds.
    pub holds: Option<OrderHolds>,
}

/// Shipping address for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderAddress {
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Company name.
    pub company: Option<String>,
    /// Address line 1.
    pub address1: Option<String>,
    /// Address line 2.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// State/province.
    pub state: Option<String>,
    /// Country.
    pub country: Option<String>,
    /// ZIP/postal code.
    pub zip: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
}

/// A line item in an order.
#[derive(Debug, Clone, Serialize)]
pub struct OrderLineItem {
    /// Line item ID.
    pub id: String,
    /// Product SKU.
    pub sku: Option<String>,
    /// Product name.
    pub product_name: Option<String>,
    /// Quantity ordered.
    pub quantity: Option<i64>,
    /// Unit price.
    pub price: Option<String>,
    /// Fulfillment status.
    pub fulfillment_status: Option<String>,
    /// Warehouse location.
    pub warehouse: Option<String>,
}

/// Shipping method information.
#[derive(Debug, Clone, Serialize)]
pub struct ShippingLine {
    /// Shipping method title.
    pub title: Option<String>,
    /// Shipping cost.
    pub price: Option<String>,
    /// Carrier name.
    pub carrier: Option<String>,
    /// Shipping method.
    pub method: Option<String>,
}

/// Order holds that prevent fulfillment.
#[derive(Debug, Clone, Serialize)]
pub struct OrderHolds {
    /// Fraud hold.
    pub fraud: Option<bool>,
    /// Address verification hold.
    pub address: Option<bool>,
    /// Payment hold.
    pub payment: Option<bool>,
    /// Operator/manual hold.
    pub operator: Option<bool>,
}

/// Order history event.
#[derive(Debug, Clone, Serialize)]
pub struct OrderHistoryEvent {
    /// Event ID.
    pub id: String,
    /// Timestamp.
    pub created_at: Option<String>,
    /// Event description.
    pub information: Option<String>,
    /// User who triggered the event.
    pub username: Option<String>,
}

/// A shipment from the warehouse.
#[derive(Debug, Clone, Serialize)]
pub struct Shipment {
    /// Shipment ID.
    pub id: String,
    /// Legacy numeric ID.
    pub legacy_id: Option<i64>,
    /// Associated order ID.
    pub order_id: Option<String>,
    /// Order number.
    pub order_number: Option<String>,
    /// User ID who processed the shipment.
    pub user_id: Option<String>,
    /// Warehouse ID.
    pub warehouse_id: Option<String>,
    /// Shipping address.
    pub address: Option<ShipmentAddress>,
    /// Whether shipped outside `ShipHero`.
    pub shipped_off_shiphero: Option<bool>,
    /// Whether this is a dropshipment.
    pub dropshipment: Option<bool>,
    /// Shipment creation date.
    pub created_date: Option<String>,
    /// Line items in the shipment.
    pub line_items: Vec<ShipmentLineItem>,
    /// Shipping labels.
    pub shipping_labels: Vec<ShippingLabel>,
    /// Total number of packages.
    pub total_packages: Option<i64>,
}

/// Shipping address for a shipment.
#[derive(Debug, Clone, Serialize)]
pub struct ShipmentAddress {
    /// Name on the address.
    pub name: Option<String>,
    /// Address line 1.
    pub address1: Option<String>,
    /// Address line 2.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// State/province.
    pub state: Option<String>,
    /// Country.
    pub country: Option<String>,
    /// ZIP/postal code.
    pub zip: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
}

/// Line item in a shipment.
#[derive(Debug, Clone, Serialize)]
pub struct ShipmentLineItem {
    /// Line item ID.
    pub id: String,
    /// SKU.
    pub sku: Option<String>,
    /// Product name.
    pub product_name: Option<String>,
    /// Quantity shipped.
    pub quantity: Option<i64>,
}

/// Shipping label information.
#[derive(Debug, Clone, Serialize)]
pub struct ShippingLabel {
    /// Label ID.
    pub id: String,
    /// Tracking number.
    pub tracking_number: Option<String>,
    /// Carrier.
    pub carrier: Option<String>,
    /// Shipping service name.
    pub shipping_name: Option<String>,
    /// Shipping method.
    pub shipping_method: Option<String>,
    /// Shipping cost.
    pub cost: Option<String>,
    /// Label creation date.
    pub created_date: Option<String>,
    /// Label status.
    pub status: Option<String>,
}

/// Paginated list of orders.
#[derive(Debug, Clone, Serialize)]
pub struct OrderConnection {
    /// Orders in this page.
    pub orders: Vec<WarehouseOrder>,
    /// Whether there are more pages.
    pub has_next_page: bool,
    /// Cursor for the next page.
    pub end_cursor: Option<String>,
}

/// Paginated list of shipments.
#[derive(Debug, Clone, Serialize)]
pub struct ShipmentConnection {
    /// Shipments in this page.
    pub shipments: Vec<Shipment>,
    /// Whether there are more pages.
    pub has_next_page: bool,
    /// Cursor for the next page.
    pub end_cursor: Option<String>,
}

// =============================================================================
// ShipHeroClient Order Methods
// =============================================================================

impl ShipHeroClient {
    /// Get orders awaiting fulfillment.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self))]
    pub async fn get_pending_orders(
        &self,
        first: Option<i64>,
        after: Option<String>,
        fulfillment_status: Option<String>,
    ) -> Result<OrderConnection, ShipHeroError> {
        use super::queries::get_pending_orders::Variables;

        let variables = Variables {
            first,
            after,
            fulfillment_status,
        };

        let response = self.execute_query::<GetPendingOrders>(variables).await?;

        // Navigate: response.orders (Option) -> data (Option) -> edges
        let Some(orders_result) = response.orders else {
            return Ok(OrderConnection {
                orders: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        let Some(data) = orders_result.data else {
            return Ok(OrderConnection {
                orders: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        // Extract pagination - page_info is a struct (not Option)
        let has_next_page = data.page_info.has_next_page;
        let end_cursor = data.page_info.end_cursor;

        // Extract orders from edges
        let orders: Vec<WarehouseOrder> = data
            .edges
            .into_iter()
            .flatten()
            .filter_map(|edge| {
                let node = edge.node?;
                Some(WarehouseOrder {
                    id: node.id?,
                    legacy_id: node.legacy_id,
                    order_number: node.order_number,
                    partner_order_id: node.partner_order_id,
                    shop_name: node.shop_name,
                    fulfillment_status: node.fulfillment_status,
                    order_date: node.order_date,
                    total_price: node.total_price,
                    shipping_address: node.shipping_address.map(|addr| OrderAddress {
                        first_name: addr.first_name,
                        last_name: addr.last_name,
                        company: None,
                        address1: None,
                        address2: None,
                        city: addr.city,
                        state: addr.state,
                        country: addr.country,
                        zip: addr.zip,
                        phone: None,
                    }),
                    line_items: convert_pending_order_line_items(node.line_items),
                    holds: node.holds.map(|h| OrderHolds {
                        fraud: h.fraud_hold,
                        address: h.address_hold,
                        payment: h.payment_hold,
                        operator: h.operator_hold,
                    }),
                })
            })
            .collect();

        Ok(OrderConnection {
            orders,
            has_next_page,
            end_cursor,
        })
    }

    /// Get a single order by ID.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn get_order(&self, id: &str) -> Result<Option<WarehouseOrderDetail>, ShipHeroError> {
        use super::queries::get_order::Variables;

        let variables = Variables { id: id.to_string() };
        let response = self.execute_query::<GetOrder>(variables).await?;

        // Navigate: response.order (Option) -> data (the Order)
        let order = response
            .order
            .and_then(|order_result| order_result.data)
            .map(|order| WarehouseOrderDetail {
                id: order.id.unwrap_or_default(),
                legacy_id: order.legacy_id,
                order_number: order.order_number,
                partner_order_id: order.partner_order_id,
                shop_name: order.shop_name,
                fulfillment_status: order.fulfillment_status,
                order_date: order.order_date,
                total_price: order.total_price,
                subtotal: order.subtotal,
                total_discounts: order.total_discounts,
                total_tax: order.total_tax,
                shipping_lines: order.shipping_lines.map(|sl| ShippingLine {
                    title: sl.title,
                    price: sl.price,
                    carrier: sl.carrier,
                    method: sl.method,
                }),
                shipping_address: order.shipping_address.map(|addr| OrderAddress {
                    first_name: addr.first_name,
                    last_name: addr.last_name,
                    company: addr.company,
                    address1: addr.address1,
                    address2: addr.address2,
                    city: addr.city,
                    state: addr.state,
                    country: addr.country,
                    zip: addr.zip,
                    phone: addr.phone,
                }),
                billing_address: order.billing_address.map(|addr| OrderAddress {
                    first_name: addr.first_name,
                    last_name: addr.last_name,
                    company: addr.company,
                    address1: addr.address1,
                    address2: addr.address2,
                    city: addr.city,
                    state: addr.state,
                    country: addr.country,
                    zip: addr.zip,
                    phone: addr.phone,
                }),
                line_items: convert_order_detail_line_items(order.line_items),
                holds: order.holds.map(|h| OrderHolds {
                    fraud: h.fraud_hold,
                    address: h.address_hold,
                    payment: h.payment_hold,
                    operator: h.operator_hold,
                }),
            });

        Ok(order)
    }

    /// Get order history/timeline.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_order_history(
        &self,
        order_id: &str,
    ) -> Result<Vec<OrderHistoryEvent>, ShipHeroError> {
        use super::queries::get_order_history::Variables;

        let variables = Variables {
            order_id: order_id.to_string(),
        };

        let response = self.execute_query::<GetOrderHistory>(variables).await?;

        let events = response
            .order_history
            .and_then(|history_result| history_result.data)
            .map(|data| {
                data.edges
                    .into_iter()
                    .flatten()
                    .filter_map(|edge| {
                        let node = edge.node?;
                        Some(OrderHistoryEvent {
                            id: node.id.unwrap_or_default(),
                            created_at: node.created_at,
                            information: node.information,
                            username: node.username,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(events)
    }

    /// Get recent shipments.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self))]
    pub async fn get_shipments(
        &self,
        first: Option<i64>,
        after: Option<String>,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<ShipmentConnection, ShipHeroError> {
        use super::queries::get_shipments::Variables;

        let variables = Variables {
            first,
            after,
            date_from,
            date_to,
        };

        let response = self.execute_query::<GetShipments>(variables).await?;

        let Some(shipments_result) = response.shipments else {
            return Ok(ShipmentConnection {
                shipments: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        let Some(data) = shipments_result.data else {
            return Ok(ShipmentConnection {
                shipments: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        // Extract pagination - page_info is a struct (not Option)
        let has_next_page = data.page_info.has_next_page;
        let end_cursor = data.page_info.end_cursor;

        let shipments: Vec<Shipment> = data
            .edges
            .into_iter()
            .flatten()
            .filter_map(|edge| {
                let node = edge.node?;
                Some(Shipment {
                    id: node.id?,
                    legacy_id: node.legacy_id,
                    order_id: node.order_id,
                    order_number: node.order.and_then(|o| o.order_number),
                    user_id: node.user_id,
                    warehouse_id: node.warehouse_id,
                    address: node.address.map(|addr| ShipmentAddress {
                        name: addr.name,
                        address1: None,
                        address2: None,
                        city: addr.city,
                        state: addr.state,
                        country: addr.country,
                        zip: addr.zip,
                        phone: None,
                    }),
                    shipped_off_shiphero: node.shipped_off_shiphero,
                    dropshipment: node.dropshipment,
                    created_date: node.created_date,
                    line_items: convert_shipment_line_items(node.line_items),
                    shipping_labels: convert_shipping_labels(node.shipping_labels),
                    total_packages: node.total_packages,
                })
            })
            .collect();

        Ok(ShipmentConnection {
            shipments,
            has_next_page,
            end_cursor,
        })
    }

    /// Get a single shipment by ID.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self), fields(shipment_id = %id))]
    pub async fn get_shipment(&self, id: &str) -> Result<Option<Shipment>, ShipHeroError> {
        use super::queries::get_shipment::Variables;

        let variables = Variables { id: id.to_string() };
        let response = self.execute_query::<GetShipment>(variables).await?;

        let shipment = response
            .shipment
            .and_then(|shipment_result| shipment_result.data)
            .map(|s| Shipment {
                id: s.id.unwrap_or_default(),
                legacy_id: s.legacy_id,
                order_id: s.order_id,
                order_number: s.order.and_then(|o| o.order_number),
                user_id: s.user_id,
                warehouse_id: s.warehouse_id,
                address: s.address.map(|addr| ShipmentAddress {
                    name: addr.name,
                    address1: addr.address1,
                    address2: addr.address2,
                    city: addr.city,
                    state: addr.state,
                    country: addr.country,
                    zip: addr.zip,
                    phone: addr.phone,
                }),
                shipped_off_shiphero: s.shipped_off_shiphero,
                dropshipment: s.dropshipment,
                created_date: s.created_date,
                line_items: convert_shipment_detail_line_items(s.line_items),
                shipping_labels: convert_shipment_detail_labels(s.shipping_labels),
                total_packages: s.total_packages,
            });

        Ok(shipment)
    }
}

// =============================================================================
// Conversion Helper Functions
// =============================================================================

fn convert_pending_order_line_items(
    line_items: Option<
        super::queries::get_pending_orders::GetPendingOrdersOrdersDataEdgesNodeLineItems,
    >,
) -> Vec<OrderLineItem> {
    let Some(li) = line_items else {
        return Vec::new();
    };
    li.edges
        .into_iter()
        .flatten()
        .filter_map(|edge| {
            let node = edge.node?;
            Some(OrderLineItem {
                id: node.id?,
                sku: node.sku,
                product_name: node.product_name,
                quantity: node.quantity,
                price: node.price,
                fulfillment_status: None,
                warehouse: None,
            })
        })
        .collect()
}

fn convert_order_detail_line_items(
    line_items: Option<super::queries::get_order::GetOrderOrderDataLineItems>,
) -> Vec<OrderLineItem> {
    let Some(li) = line_items else {
        return Vec::new();
    };
    li.edges
        .into_iter()
        .flatten()
        .filter_map(|edge| {
            let node = edge.node?;
            Some(OrderLineItem {
                id: node.id?,
                sku: node.sku,
                product_name: node.product_name,
                quantity: node.quantity,
                price: node.price,
                fulfillment_status: node.fulfillment_status,
                warehouse: node.warehouse,
            })
        })
        .collect()
}

fn convert_shipment_line_items(
    line_items: Option<super::queries::get_shipments::GetShipmentsShipmentsDataEdgesNodeLineItems>,
) -> Vec<ShipmentLineItem> {
    let Some(li) = line_items else {
        return Vec::new();
    };
    li.edges
        .into_iter()
        .flatten()
        .filter_map(|edge| {
            let node = edge.node?;
            let line_item = node.line_item;
            Some(ShipmentLineItem {
                id: node.id?,
                sku: line_item.as_ref().and_then(|l| l.sku.clone()),
                product_name: line_item.and_then(|l| l.product_name),
                quantity: node.quantity,
            })
        })
        .collect()
}

fn convert_shipment_detail_line_items(
    line_items: Option<super::queries::get_shipment::GetShipmentShipmentDataLineItems>,
) -> Vec<ShipmentLineItem> {
    let Some(li) = line_items else {
        return Vec::new();
    };
    li.edges
        .into_iter()
        .flatten()
        .filter_map(|edge| {
            let node = edge.node?;
            let line_item = node.line_item;
            Some(ShipmentLineItem {
                id: node.id?,
                sku: line_item.as_ref().and_then(|l| l.sku.clone()),
                product_name: line_item.and_then(|l| l.product_name),
                quantity: node.quantity,
            })
        })
        .collect()
}

fn convert_shipping_labels(
    labels: Option<
        Vec<
            Option<super::queries::get_shipments::GetShipmentsShipmentsDataEdgesNodeShippingLabels>,
        >,
    >,
) -> Vec<ShippingLabel> {
    let Some(labels) = labels else {
        return Vec::new();
    };
    labels
        .into_iter()
        .flatten()
        .map(|sl| ShippingLabel {
            id: sl.id.unwrap_or_default(),
            tracking_number: sl.tracking_number,
            carrier: sl.carrier,
            shipping_name: sl.shipping_name,
            shipping_method: sl.shipping_method,
            cost: sl.cost,
            created_date: sl.created_date,
            status: None,
        })
        .collect()
}

fn convert_shipment_detail_labels(
    labels: Option<
        Vec<Option<super::queries::get_shipment::GetShipmentShipmentDataShippingLabels>>,
    >,
) -> Vec<ShippingLabel> {
    let Some(labels) = labels else {
        return Vec::new();
    };
    labels
        .into_iter()
        .flatten()
        .map(|sl| ShippingLabel {
            id: sl.id.unwrap_or_default(),
            tracking_number: sl.tracking_number,
            carrier: sl.carrier,
            shipping_name: sl.shipping_name,
            shipping_method: sl.shipping_method,
            cost: sl.cost,
            created_date: sl.created_date,
            status: sl.status,
        })
        .collect()
}
