//! Database operations for expense tracking.
//!
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;
use crate::models::expense::{
    ChannelAdSpend, CreateExpenseInput, Expense, ExpenseCategory, ExpenseCategorySummary,
    ExpenseFilter, ExpenseType, ExpenseTypeSummary, ExpenseWithCategory, RecurrenceInterval,
    UpdateExpenseInput,
};

/// Convert chrono `NaiveDate` to `time::Date` for `SQLx` bind compatibility.
///
/// See `crates/admin/src/db/inventory_lot.rs` for the full explanation of why
/// this conversion is needed (chrono vs time type resolution with both crates present).
fn to_time_date(date: NaiveDate) -> time::Date {
    let month = u8::try_from(date.month()).expect("month in range 1-12");
    let day = u8::try_from(date.day()).expect("day in range 1-31");
    time::Date::from_calendar_date(
        date.year(),
        time::Month::try_from(month).expect("valid month"),
        day,
    )
    .expect("valid date")
}

// =============================================================================
// Internal Row Types
// =============================================================================

/// Internal row type for expense category queries.
#[derive(Debug, sqlx::FromRow)]
struct ExpenseCategoryRow {
    id: i32,
    name: String,
    expense_type: ExpenseType,
    description: Option<String>,
    is_system: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ExpenseCategoryRow> for ExpenseCategory {
    fn from(row: ExpenseCategoryRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            expense_type: row.expense_type,
            description: row.description,
            is_system: row.is_system,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Internal row type for expense queries.
#[derive(Debug, sqlx::FromRow)]
struct ExpenseRow {
    id: i32,
    category_id: i32,
    description: String,
    amount: Decimal,
    currency_code: String,
    expense_date: NaiveDate,
    is_recurring: bool,
    recurrence_interval: Option<RecurrenceInterval>,
    recurrence_end_date: Option<NaiveDate>,
    channel_name: Option<String>,
    vendor: Option<String>,
    notes: Option<String>,
    created_by: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ExpenseRow> for Expense {
    fn from(row: ExpenseRow) -> Self {
        Self {
            id: row.id,
            category_id: row.category_id,
            description: row.description,
            amount: row.amount,
            currency_code: row.currency_code,
            date: row.expense_date,
            is_recurring: row.is_recurring,
            recurrence_interval: row.recurrence_interval,
            recurrence_end_date: row.recurrence_end_date,
            channel_name: row.channel_name,
            vendor: row.vendor,
            notes: row.notes,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Internal row type for expense with category join.
#[derive(Debug, sqlx::FromRow)]
struct ExpenseWithCategoryRow {
    id: i32,
    category_id: i32,
    description: String,
    amount: Decimal,
    currency_code: String,
    expense_date: NaiveDate,
    is_recurring: bool,
    recurrence_interval: Option<RecurrenceInterval>,
    recurrence_end_date: Option<NaiveDate>,
    channel_name: Option<String>,
    vendor: Option<String>,
    notes: Option<String>,
    created_by: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    category_name: String,
    category_type: ExpenseType,
}

impl From<ExpenseWithCategoryRow> for ExpenseWithCategory {
    fn from(row: ExpenseWithCategoryRow) -> Self {
        Self {
            category_name: row.category_name,
            category_type: row.category_type,
            expense: Expense {
                id: row.id,
                category_id: row.category_id,
                description: row.description,
                amount: row.amount,
                currency_code: row.currency_code,
                date: row.expense_date,
                is_recurring: row.is_recurring,
                recurrence_interval: row.recurrence_interval,
                recurrence_end_date: row.recurrence_end_date,
                channel_name: row.channel_name,
                vendor: row.vendor,
                notes: row.notes,
                created_by: row.created_by,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
        }
    }
}

/// Internal row type for category summary aggregation.
#[derive(Debug, sqlx::FromRow)]
struct CategorySummaryRow {
    category_id: i32,
    category_name: String,
    expense_type: ExpenseType,
    total_amount: Decimal,
    expense_count: i64,
}

/// Internal row type for type summary aggregation.
#[derive(Debug, sqlx::FromRow)]
struct TypeSummaryRow {
    expense_type: ExpenseType,
    total_amount: Decimal,
    expense_count: i64,
}

/// Internal row type for channel ad spend aggregation.
#[derive(Debug, sqlx::FromRow)]
struct ChannelAdSpendRow {
    channel_name: String,
    total_spend: Decimal,
    expense_count: i64,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for expense database operations.
pub struct ExpenseRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> ExpenseRepository<'a> {
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // Category CRUD
    // =========================================================================

    /// Create a new expense category.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn create_category(
        &self,
        name: &str,
        expense_type: &ExpenseType,
        description: Option<&str>,
    ) -> Result<ExpenseCategory, RepositoryError> {
        debug!(%name, "Creating expense category");
        let row = sqlx::query_as!(
            ExpenseCategoryRow,
            r#"
            INSERT INTO admin.expense_category (name, expense_type, description)
            VALUES ($1, $2, $3)
            RETURNING
                id, name,
                expense_type as "expense_type: ExpenseType",
                description, is_system,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            name,
            expense_type as &ExpenseType,
            description
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.constraint() == Some("expense_category_name_key")
            {
                return RepositoryError::Conflict(format!("Category '{name}' already exists"));
            }
            RepositoryError::Database(e)
        })?;

        let cat: ExpenseCategory = row.into();
        info!(category_id = cat.id, "Expense category created");
        Ok(cat)
    }

    /// List all expense categories ordered by name.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn list_categories(&self) -> Result<Vec<ExpenseCategory>, RepositoryError> {
        debug!("Listing expense categories");
        let rows = sqlx::query_as!(
            ExpenseCategoryRow,
            r#"
            SELECT
                id, name,
                expense_type as "expense_type: ExpenseType",
                description, is_system,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.expense_category
            ORDER BY name
            "#
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found expense categories");
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Update an expense category (non-system only).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn update_category(
        &self,
        id: i32,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<ExpenseCategory, RepositoryError> {
        debug!(category_id = id, "Updating expense category");
        let row = sqlx::query_as!(
            ExpenseCategoryRow,
            r#"
            UPDATE admin.expense_category
            SET
                name = COALESCE($2, name),
                description = COALESCE($3, description)
            WHERE id = $1 AND is_system = false
            RETURNING
                id, name,
                expense_type as "expense_type: ExpenseType",
                description, is_system,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            id,
            name,
            description
        )
        .fetch_optional(self.pool)
        .await?;

        row.map(Into::into).ok_or(RepositoryError::NotFound)
    }

    /// Delete a non-system expense category.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn delete_category(&self, id: i32) -> Result<bool, RepositoryError> {
        debug!(category_id = id, "Deleting expense category");
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.expense_category
            WHERE id = $1 AND is_system = false
            "#,
            id
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!(category_id = id, "Expense category deleted");
        }
        Ok(deleted)
    }

    // =========================================================================
    // Expense CRUD
    // =========================================================================

    /// Create a new expense.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, input), level = "debug")]
    pub async fn create_expense(
        &self,
        input: &CreateExpenseInput,
        created_by: Option<i32>,
    ) -> Result<Expense, RepositoryError> {
        debug!(category_id = input.category_id, "Creating expense");
        let row = sqlx::query_as!(
            ExpenseRow,
            r#"
            INSERT INTO admin.expense (
                category_id, description, amount, currency_code,
                expense_date, is_recurring, recurrence_interval,
                recurrence_end_date, channel_name, vendor, notes, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING
                id, category_id, description,
                amount as "amount: Decimal",
                currency_code,
                expense_date as "expense_date: NaiveDate",
                is_recurring,
                recurrence_interval as "recurrence_interval: RecurrenceInterval",
                recurrence_end_date as "recurrence_end_date: NaiveDate",
                channel_name, vendor, notes, created_by,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            input.category_id,
            input.description,
            input.amount,
            input.currency_code,
            to_time_date(input.expense_date),
            input.is_recurring,
            input.recurrence_interval as Option<RecurrenceInterval>,
            input.recurrence_end_date.map(to_time_date),
            input.channel_name,
            input.vendor,
            input.notes,
            created_by
        )
        .fetch_one(self.pool)
        .await?;

        let expense: Expense = row.into();
        info!(expense_id = expense.id, "Expense created");
        Ok(expense)
    }

    /// Get an expense by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_expense(&self, id: i32) -> Result<Option<Expense>, RepositoryError> {
        debug!(expense_id = id, "Fetching expense");
        let row = sqlx::query_as!(
            ExpenseRow,
            r#"
            SELECT
                id, category_id, description,
                amount as "amount: Decimal",
                currency_code,
                expense_date as "expense_date: NaiveDate",
                is_recurring,
                recurrence_interval as "recurrence_interval: RecurrenceInterval",
                recurrence_end_date as "recurrence_end_date: NaiveDate",
                channel_name, vendor, notes, created_by,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.expense
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// List expenses with filtering, joined with category info.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, filter), level = "debug")]
    pub async fn list_expenses(
        &self,
        filter: &ExpenseFilter,
    ) -> Result<Vec<ExpenseWithCategory>, RepositoryError> {
        debug!("Listing expenses with filter");
        let limit = filter.limit.unwrap_or(50);
        let offset = filter.offset.unwrap_or(0);

        let rows = sqlx::query_as!(
            ExpenseWithCategoryRow,
            r#"
            SELECT
                e.id, e.category_id, e.description,
                e.amount as "amount: Decimal",
                e.currency_code,
                e.expense_date as "expense_date: NaiveDate",
                e.is_recurring,
                e.recurrence_interval as "recurrence_interval: RecurrenceInterval",
                e.recurrence_end_date as "recurrence_end_date: NaiveDate",
                e.channel_name, e.vendor, e.notes, e.created_by,
                e.created_at as "created_at: DateTime<Utc>",
                e.updated_at as "updated_at: DateTime<Utc>",
                c.name as category_name,
                c.expense_type as "category_type: ExpenseType"
            FROM admin.expense e
            INNER JOIN admin.expense_category c ON c.id = e.category_id
            WHERE
                ($1::int IS NULL OR e.category_id = $1)
                AND ($2::text IS NULL OR c.expense_type::text = $2)
                AND ($3::date IS NULL OR e.expense_date >= $3)
                AND ($4::date IS NULL OR e.expense_date <= $4)
                AND ($5::text IS NULL OR e.channel_name = $5)
            ORDER BY e.expense_date DESC, e.created_at DESC
            LIMIT $6 OFFSET $7
            "#,
            filter.category_id,
            filter.expense_type,
            filter.start_date.map(to_time_date),
            filter.end_date.map(to_time_date),
            filter.channel_name,
            limit,
            offset
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found expenses");
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Count expenses matching a filter (for pagination).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, filter), level = "debug")]
    pub async fn count_expenses(&self, filter: &ExpenseFilter) -> Result<i64, RepositoryError> {
        let row = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.expense e
            INNER JOIN admin.expense_category c ON c.id = e.category_id
            WHERE
                ($1::int IS NULL OR e.category_id = $1)
                AND ($2::text IS NULL OR c.expense_type::text = $2)
                AND ($3::date IS NULL OR e.expense_date >= $3)
                AND ($4::date IS NULL OR e.expense_date <= $4)
                AND ($5::text IS NULL OR e.channel_name = $5)
            "#,
            filter.category_id,
            filter.expense_type,
            filter.start_date.map(to_time_date),
            filter.end_date.map(to_time_date),
            filter.channel_name
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    /// Update an expense.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, input), level = "debug")]
    pub async fn update_expense(
        &self,
        id: i32,
        input: &UpdateExpenseInput,
    ) -> Result<Expense, RepositoryError> {
        debug!(expense_id = id, "Updating expense");
        let row = sqlx::query_as!(
            ExpenseRow,
            r#"
            UPDATE admin.expense
            SET
                category_id = COALESCE($2, category_id),
                description = COALESCE($3, description),
                amount = COALESCE($4, amount),
                currency_code = COALESCE($5, currency_code),
                expense_date = COALESCE($6, expense_date),
                is_recurring = COALESCE($7, is_recurring),
                recurrence_interval = COALESCE($8, recurrence_interval),
                recurrence_end_date = COALESCE($9, recurrence_end_date),
                channel_name = COALESCE($10, channel_name),
                vendor = COALESCE($11, vendor),
                notes = COALESCE($12, notes)
            WHERE id = $1
            RETURNING
                id, category_id, description,
                amount as "amount: Decimal",
                currency_code,
                expense_date as "expense_date: NaiveDate",
                is_recurring,
                recurrence_interval as "recurrence_interval: RecurrenceInterval",
                recurrence_end_date as "recurrence_end_date: NaiveDate",
                channel_name, vendor, notes, created_by,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            id,
            input.category_id,
            input.description,
            input.amount,
            input.currency_code,
            input.expense_date.map(to_time_date),
            input.is_recurring,
            input.recurrence_interval as Option<RecurrenceInterval>,
            input.recurrence_end_date.map(to_time_date),
            input.channel_name,
            input.vendor,
            input.notes
        )
        .fetch_optional(self.pool)
        .await?;

        row.map(Into::into).ok_or(RepositoryError::NotFound)
    }

    /// Delete an expense.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn delete_expense(&self, id: i32) -> Result<bool, RepositoryError> {
        debug!(expense_id = id, "Deleting expense");
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.expense WHERE id = $1
            "#,
            id
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!(expense_id = id, "Expense deleted");
        }
        Ok(deleted)
    }

    // =========================================================================
    // Reporting Queries
    // =========================================================================

    /// Get expenses grouped by category for a date range.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_expenses_by_category(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<ExpenseCategorySummary>, RepositoryError> {
        debug!("Aggregating expenses by category");
        let rows = sqlx::query_as!(
            CategorySummaryRow,
            r#"
            SELECT
                c.id as "category_id!",
                c.name as "category_name!",
                c.expense_type as "expense_type!: ExpenseType",
                COALESCE(SUM(e.amount), 0) as "total_amount!: Decimal",
                COUNT(e.id) as "expense_count!"
            FROM admin.expense_category c
            LEFT JOIN admin.expense e
                ON e.category_id = c.id
                AND e.expense_date >= $1
                AND e.expense_date <= $2
            GROUP BY c.id, c.name, c.expense_type
            HAVING COUNT(e.id) > 0
            ORDER BY SUM(e.amount) DESC NULLS LAST
            "#,
            to_time_date(start),
            to_time_date(end)
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ExpenseCategorySummary {
                category_id: r.category_id,
                category_name: r.category_name,
                expense_type: r.expense_type,
                total_amount: r.total_amount,
                expense_count: r.expense_count,
            })
            .collect())
    }

    /// Get expenses grouped by expense type for a date range.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_expenses_by_type(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<ExpenseTypeSummary>, RepositoryError> {
        debug!("Aggregating expenses by type");
        let rows = sqlx::query_as!(
            TypeSummaryRow,
            r#"
            SELECT
                c.expense_type as "expense_type!: ExpenseType",
                COALESCE(SUM(e.amount), 0) as "total_amount!: Decimal",
                COUNT(e.id) as "expense_count!"
            FROM admin.expense e
            INNER JOIN admin.expense_category c ON c.id = e.category_id
            WHERE e.expense_date >= $1 AND e.expense_date <= $2
            GROUP BY c.expense_type
            ORDER BY SUM(e.amount) DESC
            "#,
            to_time_date(start),
            to_time_date(end)
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ExpenseTypeSummary {
                expense_type: r.expense_type,
                total_amount: r.total_amount,
                expense_count: r.expense_count,
            })
            .collect())
    }

    /// Get ad spend grouped by channel for a date range.
    ///
    /// Only returns expenses from advertising categories that have a `channel_name`.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_ad_spend_by_channel(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<ChannelAdSpend>, RepositoryError> {
        debug!("Aggregating ad spend by channel");
        let rows = sqlx::query_as!(
            ChannelAdSpendRow,
            r#"
            SELECT
                e.channel_name as "channel_name!",
                COALESCE(SUM(e.amount), 0) as "total_spend!: Decimal",
                COUNT(e.id) as "expense_count!"
            FROM admin.expense e
            INNER JOIN admin.expense_category c ON c.id = e.category_id
            WHERE
                e.expense_date >= $1
                AND e.expense_date <= $2
                AND e.channel_name IS NOT NULL
                AND c.expense_type = 'advertising'
            GROUP BY e.channel_name
            ORDER BY SUM(e.amount) DESC
            "#,
            to_time_date(start),
            to_time_date(end)
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ChannelAdSpend {
                channel_name: r.channel_name,
                total_spend: r.total_spend,
                expense_count: r.expense_count,
            })
            .collect())
    }

    /// Get total expenses for a date range.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_total_expenses(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Decimal, RepositoryError> {
        debug!("Calculating total expenses");
        let total = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(amount), 0) as "total!: Decimal"
            FROM admin.expense
            WHERE expense_date >= $1 AND expense_date <= $2
            "#,
            to_time_date(start),
            to_time_date(end)
        )
        .fetch_one(self.pool)
        .await?;

        Ok(total)
    }
}
