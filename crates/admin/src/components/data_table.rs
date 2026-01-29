//! Data table component types.
//!
//! These types define the configuration for reusable data tables in the admin panel.

use serde::{Deserialize, Serialize};

/// Column definition for a data table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableColumn {
    /// Unique key for the column.
    pub key: String,
    /// Display label for the column header.
    pub label: String,
    /// Whether the column is sortable.
    pub sortable: bool,
    /// Whether the column is visible by default.
    pub default_visible: bool,
}

impl TableColumn {
    /// Create a new sortable column.
    #[must_use]
    pub fn sortable(key: &str, label: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            sortable: true,
            default_visible: true,
        }
    }

    /// Create a new non-sortable column.
    #[must_use]
    pub fn new(key: &str, label: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            sortable: false,
            default_visible: true,
        }
    }

    /// Set whether the column is visible by default.
    #[must_use]
    pub const fn visible(mut self, visible: bool) -> Self {
        self.default_visible = visible;
        self
    }
}

/// Filter type for data tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    /// Text input filter.
    Text,
    /// Single-select dropdown.
    Select,
    /// Multi-select checkboxes.
    MultiSelect,
    /// Date range picker.
    DateRange,
    /// Number range (min/max).
    NumberRange,
}

/// Filter definition for a data table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableFilter {
    /// Filter parameter key.
    pub key: String,
    /// Display label.
    pub label: String,
    /// Filter type.
    pub filter_type: FilterType,
    /// Placeholder text (for text inputs).
    pub placeholder: Option<String>,
    /// Available options (for select/multiselect).
    pub options: Vec<FilterOption>,
}

/// Option for select/multiselect filters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterOption {
    /// Option value.
    pub value: String,
    /// Display label.
    pub label: String,
}

impl FilterOption {
    /// Create a new filter option.
    #[must_use]
    pub fn new(value: &str, label: &str) -> Self {
        Self {
            value: value.to_string(),
            label: label.to_string(),
        }
    }
}

impl TableFilter {
    /// Create a text filter.
    #[must_use]
    pub fn text(key: &str, label: &str, placeholder: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            filter_type: FilterType::Text,
            placeholder: Some(placeholder.to_string()),
            options: vec![],
        }
    }

    /// Create a select filter.
    #[must_use]
    pub fn select(key: &str, label: &str, options: Vec<FilterOption>) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            filter_type: FilterType::Select,
            placeholder: None,
            options,
        }
    }

    /// Create a multi-select filter.
    #[must_use]
    pub fn multi_select(key: &str, label: &str, options: Vec<FilterOption>) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            filter_type: FilterType::MultiSelect,
            placeholder: None,
            options,
        }
    }

    /// Create a date range filter.
    #[must_use]
    pub fn date_range(key: &str, label: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            filter_type: FilterType::DateRange,
            placeholder: None,
            options: vec![],
        }
    }
}

/// Bulk action definition for data tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkAction {
    /// Action key (passed to event handler).
    pub key: String,
    /// Display label.
    pub label: String,
    /// Phosphor icon class.
    pub icon: String,
    /// Whether this is a destructive action.
    pub destructive: bool,
}

impl BulkAction {
    /// Create a new bulk action.
    #[must_use]
    pub fn new(key: &str, label: &str, icon: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            icon: icon.to_string(),
            destructive: false,
        }
    }

    /// Mark this action as destructive.
    #[must_use]
    pub const fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
}

/// Configuration for a data table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTableConfig {
    /// Unique table identifier.
    pub table_id: String,
    /// Column definitions.
    pub columns: Vec<TableColumn>,
    /// Filter definitions.
    pub filters: Vec<TableFilter>,
    /// Bulk action definitions.
    pub bulk_actions: Vec<BulkAction>,
    /// Search placeholder text.
    pub search_placeholder: String,
    /// Icon for empty state.
    pub empty_icon: String,
    /// Title for empty state.
    pub empty_title: String,
    /// Description for empty state.
    pub empty_description: Option<String>,
    /// Whether to show bulk action bar.
    pub has_bulk_actions: bool,
    /// Whether to show filter panel.
    pub has_filters: bool,
    /// Whether to show column picker.
    pub has_column_picker: bool,
}

impl DataTableConfig {
    /// Create a new data table configuration.
    #[must_use]
    pub fn new(table_id: &str) -> Self {
        Self {
            table_id: table_id.to_string(),
            columns: vec![],
            filters: vec![],
            bulk_actions: vec![],
            search_placeholder: "Search...".to_string(),
            empty_icon: "ph-list".to_string(),
            empty_title: "No items found".to_string(),
            empty_description: None,
            has_bulk_actions: false,
            has_filters: false,
            has_column_picker: true,
        }
    }

    /// Add a column.
    #[must_use]
    pub fn column(mut self, column: TableColumn) -> Self {
        self.columns.push(column);
        self
    }

    /// Add a filter.
    #[must_use]
    pub fn filter(mut self, filter: TableFilter) -> Self {
        self.has_filters = true;
        self.filters.push(filter);
        self
    }

    /// Add a bulk action.
    #[must_use]
    pub fn bulk_action(mut self, action: BulkAction) -> Self {
        self.has_bulk_actions = true;
        self.bulk_actions.push(action);
        self
    }

    /// Set search placeholder.
    #[must_use]
    pub fn search_placeholder(mut self, placeholder: &str) -> Self {
        self.search_placeholder = placeholder.to_string();
        self
    }

    /// Set empty state configuration.
    #[must_use]
    pub fn empty_state(mut self, icon: &str, title: &str, description: Option<&str>) -> Self {
        self.empty_icon = icon.to_string();
        self.empty_title = title.to_string();
        self.empty_description = description.map(ToString::to_string);
        self
    }

    /// Get default visible columns.
    #[must_use]
    pub fn default_columns(&self) -> Vec<String> {
        self.columns
            .iter()
            .filter(|c| c.default_visible)
            .map(|c| c.key.clone())
            .collect()
    }
}

/// Build the orders table configuration.
#[must_use]
pub fn orders_table_config() -> DataTableConfig {
    DataTableConfig::new("orders")
        .column(TableColumn::sortable("order", "Order"))
        .column(TableColumn::sortable("customer", "Customer"))
        .column(TableColumn::new("payment", "Payment"))
        .column(TableColumn::new("fulfillment", "Fulfillment"))
        .column(TableColumn::new("return", "Return").visible(false))
        .column(TableColumn::sortable("items", "Items"))
        .column(TableColumn::sortable("total", "Total"))
        .column(TableColumn::new("delivery", "Delivery").visible(false))
        .column(TableColumn::new("channel", "Channel").visible(false))
        .column(TableColumn::new("tags", "Tags").visible(false))
        .column(TableColumn::new("risk", "Risk").visible(false))
        .column(TableColumn::new("destination", "Destination").visible(false))
        .filter(TableFilter::multi_select(
            "financial_status",
            "Payment Status",
            vec![
                FilterOption::new("paid", "Paid"),
                FilterOption::new("pending", "Pending"),
                FilterOption::new("authorized", "Authorized"),
                FilterOption::new("partially_paid", "Partially Paid"),
                FilterOption::new("partially_refunded", "Partially Refunded"),
                FilterOption::new("refunded", "Refunded"),
                FilterOption::new("voided", "Voided"),
            ],
        ))
        .filter(TableFilter::multi_select(
            "fulfillment_status",
            "Fulfillment Status",
            vec![
                FilterOption::new("unfulfilled", "Unfulfilled"),
                FilterOption::new("partial", "Partial"),
                FilterOption::new("fulfilled", "Fulfilled"),
                FilterOption::new("scheduled", "Scheduled"),
                FilterOption::new("on_hold", "On Hold"),
            ],
        ))
        .filter(TableFilter::multi_select(
            "return_status",
            "Return Status",
            vec![
                FilterOption::new("return_requested", "Return Requested"),
                FilterOption::new("in_progress", "In Progress"),
                FilterOption::new("returned", "Returned"),
            ],
        ))
        .filter(TableFilter::select(
            "status",
            "Order Status",
            vec![
                FilterOption::new("open", "Open"),
                FilterOption::new("closed", "Archived"),
                FilterOption::new("cancelled", "Cancelled"),
            ],
        ))
        .filter(TableFilter::select(
            "risk_level",
            "Risk Level",
            vec![
                FilterOption::new("high", "High Risk"),
                FilterOption::new("medium", "Medium Risk"),
                FilterOption::new("low", "Low Risk"),
            ],
        ))
        .filter(TableFilter::multi_select(
            "delivery_method",
            "Delivery Method",
            vec![
                FilterOption::new("shipping", "Shipping"),
                FilterOption::new("local_delivery", "Local Delivery"),
                FilterOption::new("pickup", "Pickup"),
            ],
        ))
        .filter(TableFilter::text(
            "channel",
            "Sales Channel",
            "e.g., Online Store",
        ))
        .filter(TableFilter::date_range("created_at", "Created Date"))
        .filter(TableFilter::text("tag", "Tag", "Enter tag..."))
        .filter(TableFilter::text(
            "discount_code",
            "Discount Code",
            "Enter code...",
        ))
        .bulk_action(BulkAction::new("add_tags", "Add Tags", "ph-tag"))
        .bulk_action(BulkAction::new("remove_tags", "Remove Tags", "ph-tag"))
        .bulk_action(BulkAction::new("archive", "Archive", "ph-archive"))
        .bulk_action(BulkAction::new("cancel", "Cancel", "ph-x-circle").destructive())
        .search_placeholder("Search orders by number, customer, or email...")
        .empty_state(
            "ph-receipt",
            "No orders found",
            Some("Try adjusting your search or filters"),
        )
}

/// Build the inventory table configuration.
#[must_use]
pub fn inventory_table_config() -> DataTableConfig {
    DataTableConfig::new("inventory")
        .column(TableColumn::sortable("product", "Product"))
        .column(TableColumn::sortable("sku", "SKU"))
        .column(TableColumn::new("tracked", "Tracked").visible(false))
        .column(TableColumn::sortable("on_hand", "On Hand"))
        .column(TableColumn::sortable("available", "Available"))
        .column(TableColumn::new("committed", "Committed").visible(false))
        .column(TableColumn::new("incoming", "Incoming").visible(false))
        .column(TableColumn::new("cost", "Cost").visible(false))
        .column(TableColumn::new("status", "Status"))
        .filter(TableFilter::select(
            "tracking",
            "Tracking",
            vec![
                FilterOption::new("tracked", "Tracked"),
                FilterOption::new("untracked", "Not Tracked"),
            ],
        ))
        .filter(TableFilter::multi_select(
            "stock_status",
            "Stock Status",
            vec![
                FilterOption::new("in_stock", "In Stock"),
                FilterOption::new("low_stock", "Low Stock"),
                FilterOption::new("out_of_stock", "Out of Stock"),
            ],
        ))
        .filter(TableFilter::multi_select(
            "product_status",
            "Product Status",
            vec![
                FilterOption::new("ACTIVE", "Active"),
                FilterOption::new("DRAFT", "Draft"),
                FilterOption::new("ARCHIVED", "Archived"),
            ],
        ))
        .bulk_action(BulkAction::new(
            "adjust_quantity",
            "Adjust Quantity",
            "ph-plus-minus",
        ))
        .bulk_action(BulkAction::new(
            "toggle_tracked",
            "Toggle Tracking",
            "ph-check-circle",
        ))
        .bulk_action(BulkAction::new("update_sku", "Update SKU", "ph-barcode"))
        .bulk_action(BulkAction::new(
            "update_customs",
            "Update Customs",
            "ph-globe",
        ))
        .search_placeholder("Search products by name or SKU...")
        .empty_state(
            "ph-package",
            "No inventory items found",
            Some("Try adjusting your search or filters"),
        )
}

/// Build the customers table configuration.
#[must_use]
pub fn customers_table_config() -> DataTableConfig {
    DataTableConfig::new("customers")
        .column(TableColumn::sortable("name", "Customer"))
        .column(TableColumn::new("phone", "Phone").visible(false))
        .column(TableColumn::sortable("location", "Location"))
        .column(TableColumn::new("orders", "Orders"))
        .column(TableColumn::new("spent", "Spent"))
        .column(TableColumn::new("state", "State"))
        .column(TableColumn::new("tags", "Tags").visible(false))
        .column(TableColumn::new("marketing", "Marketing").visible(false))
        .column(TableColumn::sortable("created", "Created").visible(false))
        .filter(TableFilter::multi_select(
            "state",
            "State",
            vec![
                FilterOption::new("ENABLED", "Enabled"),
                FilterOption::new("INVITED", "Invited"),
                FilterOption::new("DISABLED", "Disabled"),
                FilterOption::new("DECLINED", "Declined"),
            ],
        ))
        .filter(TableFilter::select(
            "accepts_marketing",
            "Email Subscribed",
            vec![
                FilterOption::new("true", "Yes"),
                FilterOption::new("false", "No"),
            ],
        ))
        .filter(TableFilter::date_range("created_at", "Created Date"))
        .bulk_action(BulkAction::new("add_tags", "Add Tags", "ph-tag"))
        .bulk_action(BulkAction::new("remove_tags", "Remove Tags", "ph-tag"))
        .bulk_action(BulkAction::new(
            "subscribe",
            "Subscribe to Marketing",
            "ph-envelope",
        ))
        .search_placeholder("Search customers by name or email...")
        .empty_state(
            "ph-users",
            "No customers found",
            Some("Try adjusting your search or filters"),
        )
}

/// Build the discounts table configuration.
#[must_use]
pub fn discounts_table_config() -> DataTableConfig {
    DataTableConfig::new("discounts")
        // Columns (default visible)
        .column(TableColumn::sortable("title", "Discount"))
        .column(TableColumn::sortable("code", "Code"))
        .column(TableColumn::sortable("type", "Type"))
        .column(TableColumn::sortable("status", "Status"))
        .column(TableColumn::sortable("value", "Value"))
        .column(TableColumn::sortable("usage", "Usage"))
        // Hidden by default
        .column(TableColumn::sortable("method", "Method").visible(false))
        .column(TableColumn::sortable("minimum", "Minimum").visible(false))
        .column(TableColumn::sortable("combines_with", "Combines With").visible(false))
        .column(TableColumn::sortable("starts_at", "Start Date").visible(false))
        .column(TableColumn::sortable("ends_at", "End Date").visible(false))
        // Filters
        .filter(TableFilter::multi_select(
            "status",
            "Status",
            vec![
                FilterOption::new("ACTIVE", "Active"),
                FilterOption::new("SCHEDULED", "Scheduled"),
                FilterOption::new("EXPIRED", "Expired"),
            ],
        ))
        .filter(TableFilter::multi_select(
            "type",
            "Type",
            vec![
                FilterOption::new("percentage", "Percentage"),
                FilterOption::new("fixed_amount", "Fixed Amount"),
                FilterOption::new("bxgy", "Buy X Get Y"),
                FilterOption::new("free_shipping", "Free Shipping"),
            ],
        ))
        .filter(TableFilter::select(
            "method",
            "Method",
            vec![
                FilterOption::new("code", "Code"),
                FilterOption::new("automatic", "Automatic"),
            ],
        ))
        .filter(TableFilter::date_range("starts_at", "Start Date"))
        .filter(TableFilter::date_range("ends_at", "End Date"))
        // Bulk actions
        .bulk_action(BulkAction::new("activate", "Activate", "ph-play"))
        .bulk_action(BulkAction::new("deactivate", "Deactivate", "ph-pause"))
        .bulk_action(BulkAction::new("delete", "Delete", "ph-trash").destructive())
        // Empty state
        .search_placeholder("Search discounts by title or code...")
        .empty_state(
            "ph-tag",
            "No discounts found",
            Some("Create your first discount to get started"),
        )
}
