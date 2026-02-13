//! Expense tracking domain models.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Type of expense for grouping and reporting.
///
/// Maps directly to the `PostgreSQL` `admin.expense_type` enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "expense_type", rename_all = "lowercase")]
pub enum ExpenseType {
    Advertising,
    Saas,
    Shipping,
    Labor,
    Supplies,
    Services,
    Other,
}

impl ExpenseType {
    /// Display label for the expense type.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Advertising => "Advertising",
            Self::Saas => "SaaS",
            Self::Shipping => "Shipping",
            Self::Labor => "Labor",
            Self::Supplies => "Supplies",
            Self::Services => "Services",
            Self::Other => "Other",
        }
    }
}

/// Recurrence interval for recurring expenses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurrenceInterval {
    Monthly,
    Quarterly,
    Yearly,
}

impl RecurrenceInterval {
    /// Parse from database string value.
    #[must_use]
    pub fn from_str_value(s: &str) -> Option<Self> {
        match s {
            "monthly" => Some(Self::Monthly),
            "quarterly" => Some(Self::Quarterly),
            "yearly" => Some(Self::Yearly),
            _ => None,
        }
    }

    /// Convert to database string value.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::Yearly => "yearly",
        }
    }
}

/// A predefined or user-created expense category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseCategory {
    pub id: i32,
    pub name: String,
    pub expense_type: ExpenseType,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An individual expense entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expense {
    pub id: i32,
    pub category_id: i32,
    pub description: String,
    pub amount: Decimal,
    pub currency_code: String,
    pub date: NaiveDate,
    pub is_recurring: bool,
    pub recurrence_interval: Option<RecurrenceInterval>,
    pub recurrence_end_date: Option<NaiveDate>,
    pub channel_name: Option<String>,
    pub vendor: Option<String>,
    pub notes: Option<String>,
    pub created_by: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An expense with its category information joined.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseWithCategory {
    pub expense: Expense,
    pub category_name: String,
    pub category_type: ExpenseType,
}

/// Input for creating a new expense.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateExpenseInput {
    pub category_id: i32,
    pub description: String,
    pub amount: Decimal,
    pub currency_code: String,
    pub expense_date: NaiveDate,
    pub is_recurring: bool,
    pub recurrence_interval: Option<String>,
    pub recurrence_end_date: Option<NaiveDate>,
    pub channel_name: Option<String>,
    pub vendor: Option<String>,
    pub notes: Option<String>,
}

/// Input for updating an expense.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateExpenseInput {
    pub category_id: Option<i32>,
    pub description: Option<String>,
    pub amount: Option<Decimal>,
    pub currency_code: Option<String>,
    pub expense_date: Option<NaiveDate>,
    pub is_recurring: Option<bool>,
    pub recurrence_interval: Option<String>,
    pub recurrence_end_date: Option<NaiveDate>,
    pub channel_name: Option<String>,
    pub vendor: Option<String>,
    pub notes: Option<String>,
}

/// Filter criteria for listing expenses.
#[derive(Debug, Clone, Default)]
pub struct ExpenseFilter {
    pub category_id: Option<i32>,
    pub expense_type: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub channel_name: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Aggregated expense totals by category.
#[derive(Debug, Clone, Serialize)]
pub struct ExpenseCategorySummary {
    pub category_id: i32,
    pub category_name: String,
    pub expense_type: ExpenseType,
    pub total_amount: Decimal,
    pub expense_count: i64,
}

/// Aggregated expense totals by type.
#[derive(Debug, Clone, Serialize)]
pub struct ExpenseTypeSummary {
    pub expense_type: ExpenseType,
    pub total_amount: Decimal,
    pub expense_count: i64,
}

/// Ad spend per channel for attribution.
#[derive(Debug, Clone, Serialize)]
pub struct ChannelAdSpend {
    pub channel_name: String,
    pub total_spend: Decimal,
    pub expense_count: i64,
}
