//! Expense tracking route handlers.
//!
//! Provides CRUD for expenses and expense categories.

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::{debug, info, instrument, warn};

use crate::{
    db::{ExpenseRepository, RepositoryError},
    filters,
    middleware::auth::RequireAdminAuth,
    models::expense::{
        CreateExpenseInput, ExpenseCategory, ExpenseFilter, ExpenseType, ExpenseWithCategory,
        RecurrenceInterval, UpdateExpenseInput,
    },
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for expense list.
#[derive(Debug, Deserialize)]
pub struct ExpensesQuery {
    pub category_id: Option<i32>,
    pub expense_type: Option<String>,
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    pub page: Option<i64>,
}

// =============================================================================
// Form Inputs
// =============================================================================

/// Form data for creating an expense.
#[derive(Debug, Deserialize)]
pub struct CreateExpenseForm {
    pub category_id: i32,
    pub description: String,
    pub amount: Decimal,
    pub currency_code: Option<String>,
    pub expense_date: NaiveDate,
    pub is_recurring: Option<String>,
    pub recurrence_interval: Option<RecurrenceInterval>,
    pub recurrence_end_date: Option<NaiveDate>,
    pub channel_name: Option<String>,
    pub vendor: Option<String>,
    pub notes: Option<String>,
}

/// Form data for updating an expense.
#[derive(Debug, Deserialize)]
pub struct UpdateExpenseForm {
    pub category_id: i32,
    pub description: String,
    pub amount: Decimal,
    pub currency_code: Option<String>,
    pub expense_date: NaiveDate,
    pub is_recurring: Option<String>,
    pub recurrence_interval: Option<RecurrenceInterval>,
    pub recurrence_end_date: Option<NaiveDate>,
    pub channel_name: Option<String>,
    pub vendor: Option<String>,
    pub notes: Option<String>,
}

/// Form data for creating a category.
#[derive(Debug, Deserialize)]
pub struct CreateCategoryForm {
    pub name: String,
    pub expense_type: ExpenseType,
    pub description: Option<String>,
}

// =============================================================================
// View Types
// =============================================================================

/// Expense view for templates.
#[derive(Debug, Clone)]
pub struct ExpenseView {
    pub id: i32,
    pub description: String,
    pub amount: String,
    pub amount_raw: Decimal,
    pub currency_code: String,
    pub date: String,
    pub category_name: String,
    pub category_type: String,
    pub is_recurring: bool,
    pub recurrence_interval: Option<String>,
    pub channel_name: Option<String>,
    pub vendor: Option<String>,
}

impl From<&ExpenseWithCategory> for ExpenseView {
    fn from(ewc: &ExpenseWithCategory) -> Self {
        let interval_label = ewc.expense.recurrence_interval.as_ref().map(|i| match i {
            RecurrenceInterval::Monthly => "Monthly",
            RecurrenceInterval::Quarterly => "Quarterly",
            RecurrenceInterval::Yearly => "Yearly",
        });
        Self {
            id: ewc.expense.id,
            description: ewc.expense.description.clone(),
            amount: format!("${:.2}", ewc.expense.amount),
            amount_raw: ewc.expense.amount,
            currency_code: ewc.expense.currency_code.clone(),
            date: ewc.expense.date.format("%Y-%m-%d").to_string(),
            category_name: ewc.category_name.clone(),
            category_type: ewc.category_type.label().to_string(),
            is_recurring: ewc.expense.is_recurring,
            recurrence_interval: interval_label.map(ToString::to_string),
            channel_name: ewc.expense.channel_name.clone(),
            vendor: ewc.expense.vendor.clone(),
        }
    }
}

/// Category view for templates.
#[derive(Debug, Clone)]
pub struct CategoryView {
    pub id: i32,
    pub name: String,
    pub expense_type: String,
    pub expense_type_raw: ExpenseType,
    pub description: Option<String>,
    pub is_system: bool,
}

impl From<&ExpenseCategory> for CategoryView {
    fn from(cat: &ExpenseCategory) -> Self {
        Self {
            id: cat.id,
            name: cat.name.clone(),
            expense_type: cat.expense_type.label().to_string(),
            expense_type_raw: cat.expense_type.clone(),
            description: cat.description.clone(),
            is_system: cat.is_system,
        }
    }
}

// =============================================================================
// Templates
// =============================================================================

/// Expenses index page.
#[derive(Template)]
#[template(path = "financials/expenses/index.html")]
struct ExpensesIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    expenses: Vec<ExpenseView>,
    categories: Vec<CategoryView>,
    query: ExpensesQuery,
    page: i64,
    total_count: i64,
    has_next: bool,
    has_prev: bool,
    total_amount: String,
}

/// New expense form.
#[derive(Template)]
#[template(path = "financials/expenses/new.html")]
struct ExpenseNewTemplate {
    admin_user: AdminUserView,
    current_path: String,
    categories: Vec<CategoryView>,
}

/// Edit expense form.
#[derive(Template)]
#[template(path = "financials/expenses/edit.html")]
struct ExpenseEditTemplate {
    admin_user: AdminUserView,
    current_path: String,
    expense: ExpenseView,
    expense_raw: ExpenseWithCategory,
    categories: Vec<CategoryView>,
}

/// Categories management page.
#[derive(Template)]
#[template(path = "financials/expenses/categories.html")]
struct CategoriesTemplate {
    admin_user: AdminUserView,
    current_path: String,
    categories: Vec<CategoryView>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Expense list with filters and pagination.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32()))]
pub async fn expenses_index(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Query(query): Query<ExpensesQuery>,
) -> impl IntoResponse {
    debug!("Listing expenses");
    let repo = ExpenseRepository::new(state.pool());

    let page = query.page.unwrap_or(1).max(1);
    let limit = 25_i64;
    let offset = (page - 1) * limit;

    let filter = ExpenseFilter {
        category_id: query.category_id,
        expense_type: query.expense_type.clone(),
        start_date: query.start,
        end_date: query.end,
        channel_name: None,
        limit: Some(limit),
        offset: Some(offset),
    };

    let (expenses, total_count, categories) = tokio::join!(
        repo.list_expenses(&filter),
        repo.count_expenses(&filter),
        repo.list_categories()
    );

    let expenses = expenses.unwrap_or_else(|e| {
        tracing::error!(?e, "Failed to list expenses");
        vec![]
    });
    let total_count = total_count.unwrap_or(0);
    let categories = categories.unwrap_or_else(|e| {
        tracing::error!(?e, "Failed to list categories");
        vec![]
    });

    let total_amount: Decimal = expenses.iter().map(|e| e.expense.amount).sum();
    let expense_views: Vec<ExpenseView> = expenses.iter().map(ExpenseView::from).collect();
    let category_views: Vec<CategoryView> = categories.iter().map(CategoryView::from).collect();

    let template = ExpensesIndexTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/expenses".to_string(),
        expenses: expense_views,
        categories: category_views,
        query,
        page,
        total_count,
        has_next: (page * limit) < total_count,
        has_prev: page > 1,
        total_amount: format!("${total_amount:.2}"),
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// New expense form page.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32()))]
pub async fn expense_new(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
) -> impl IntoResponse {
    debug!("Rendering new expense form");
    let repo = ExpenseRepository::new(state.pool());
    let categories = repo.list_categories().await.unwrap_or_else(|e| {
        tracing::error!(?e, "Failed to list categories");
        vec![]
    });

    let template = ExpenseNewTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/expenses".to_string(),
        categories: categories.iter().map(CategoryView::from).collect(),
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// Create a new expense.
#[instrument(skip(state, form), fields(admin_id = %user.id.as_i32()))]
pub async fn expense_create(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Form(form): Form<CreateExpenseForm>,
) -> impl IntoResponse {
    debug!(description = %form.description, "Creating expense");
    let repo = ExpenseRepository::new(state.pool());

    let is_recurring = form.is_recurring.as_deref() == Some("on");
    let input = CreateExpenseInput {
        category_id: form.category_id,
        description: form.description,
        amount: form.amount,
        currency_code: form.currency_code.unwrap_or_else(|| "USD".to_string()),
        expense_date: form.expense_date,
        is_recurring,
        recurrence_interval: if is_recurring {
            form.recurrence_interval
        } else {
            None
        },
        recurrence_end_date: if is_recurring {
            form.recurrence_end_date
        } else {
            None
        },
        channel_name: form.channel_name.filter(|s| !s.is_empty()),
        vendor: form.vendor.filter(|s| !s.is_empty()),
        notes: form.notes.filter(|s| !s.is_empty()),
    };

    match repo.create_expense(&input, Some(user.id.as_i32())).await {
        Ok(expense) => {
            info!(expense_id = expense.id, "Created expense");
            Redirect::to("/financials/expenses").into_response()
        }
        Err(e) => {
            tracing::error!(?e, "Failed to create expense");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Edit expense form page.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), expense_id = id))]
pub async fn expense_edit(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    debug!("Rendering expense edit form");
    let repo = ExpenseRepository::new(state.pool());

    let (expense, categories) = tokio::join!(repo.get_expense(id), repo.list_categories());

    let expense = match expense {
        Ok(Some(exp)) => exp,
        Ok(None) => return Redirect::to("/financials/expenses").into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to get expense");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let categories = categories.unwrap_or_default();

    // Build ExpenseWithCategory for the template by finding the category
    let category = categories.iter().find(|c| c.id == expense.category_id);
    let ewc = ExpenseWithCategory {
        category_name: category.map_or_else(|| "Unknown".to_string(), |c| c.name.clone()),
        category_type: category.map_or(ExpenseType::Other, |c| c.expense_type.clone()),
        expense,
    };

    let template = ExpenseEditTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/expenses".to_string(),
        expense: ExpenseView::from(&ewc),
        expense_raw: ewc,
        categories: categories.iter().map(CategoryView::from).collect(),
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Update an expense.
#[instrument(skip(state, form), fields(admin_id = %user.id.as_i32(), expense_id = id))]
pub async fn expense_update(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<UpdateExpenseForm>,
) -> impl IntoResponse {
    debug!("Updating expense");
    let repo = ExpenseRepository::new(state.pool());

    let is_recurring = form.is_recurring.as_deref() == Some("on");
    let input = UpdateExpenseInput {
        category_id: Some(form.category_id),
        description: Some(form.description),
        amount: Some(form.amount),
        currency_code: Some(form.currency_code.unwrap_or_else(|| "USD".to_string())),
        expense_date: Some(form.expense_date),
        is_recurring: Some(is_recurring),
        recurrence_interval: if is_recurring {
            form.recurrence_interval
        } else {
            None
        },
        recurrence_end_date: if is_recurring {
            form.recurrence_end_date
        } else {
            None
        },
        channel_name: form.channel_name,
        vendor: form.vendor,
        notes: form.notes,
    };

    match repo.update_expense(id, &input).await {
        Ok(_) => {
            info!("Updated expense");
            Redirect::to("/financials/expenses").into_response()
        }
        Err(RepositoryError::NotFound) => {
            warn!("Expense not found for update");
            Redirect::to("/financials/expenses").into_response()
        }
        Err(e) => {
            tracing::error!(?e, "Failed to update expense");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Delete an expense.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), expense_id = id))]
pub async fn expense_delete(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Redirect {
    debug!("Deleting expense");
    let repo = ExpenseRepository::new(state.pool());

    match repo.delete_expense(id).await {
        Ok(true) => info!("Deleted expense"),
        Ok(false) => warn!("Expense not found for deletion"),
        Err(e) => tracing::error!(?e, "Failed to delete expense"),
    }
    Redirect::to("/financials/expenses")
}

/// Categories management page.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32()))]
pub async fn categories_index(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
) -> impl IntoResponse {
    debug!("Listing expense categories");
    let repo = ExpenseRepository::new(state.pool());
    let categories = repo.list_categories().await.unwrap_or_else(|e| {
        tracing::error!(?e, "Failed to list categories");
        vec![]
    });

    let template = CategoriesTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/expenses/categories".to_string(),
        categories: categories.iter().map(CategoryView::from).collect(),
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// Create a new expense category.
#[instrument(skip(state, form), fields(admin_id = %user.id.as_i32()))]
pub async fn category_create(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Form(form): Form<CreateCategoryForm>,
) -> impl IntoResponse {
    debug!(name = %form.name, "Creating expense category");
    let repo = ExpenseRepository::new(state.pool());

    match repo
        .create_category(&form.name, &form.expense_type, form.description.as_deref())
        .await
    {
        Ok(cat) => {
            info!(category_id = cat.id, "Created expense category");
            Redirect::to("/financials/expenses/categories").into_response()
        }
        Err(RepositoryError::Conflict(msg)) => {
            warn!(%msg, "Category creation conflict");
            Html(format!("Error: {msg}")).into_response()
        }
        Err(e) => {
            tracing::error!(?e, "Failed to create category");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Delete an expense category (non-system only).
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), category_id = id))]
pub async fn category_delete(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Redirect {
    debug!("Deleting expense category");
    let repo = ExpenseRepository::new(state.pool());

    match repo.delete_category(id).await {
        Ok(true) => info!("Deleted expense category"),
        Ok(false) => warn!("Category not found or is system category"),
        Err(e) => tracing::error!(?e, "Failed to delete category"),
    }
    Redirect::to("/financials/expenses/categories")
}

// =============================================================================
// Router
// =============================================================================

/// Build expense routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/financials/expenses",
            get(expenses_index).post(expense_create),
        )
        .route("/financials/expenses/new", get(expense_new))
        .route(
            "/financials/expenses/categories",
            get(categories_index).post(category_create),
        )
        .route("/financials/expenses/{id}/edit", get(expense_edit))
        .route("/financials/expenses/{id}", post(expense_update))
        .route("/financials/expenses/{id}/delete", post(expense_delete))
        .route(
            "/financials/expenses/categories/{id}/delete",
            post(category_delete),
        )
}
