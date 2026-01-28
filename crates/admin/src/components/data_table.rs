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
