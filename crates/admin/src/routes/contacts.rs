//! Contact graph visualization and editing routes.
//!
//! Provides an interactive graph view of contacts and their relationships,
//! with CRUD operations for manual graph editing.
//!
//! # Routes
//!
//! ```text
//! GET  /contacts                            -- Graph page (full layout)
//! GET  /contacts/api/graph                  -- JSON: full graph for Cytoscape
//! GET  /contacts/api/search?q=              -- JSON: contact search (picker)
//! GET  /contacts/new                        -- HTMX fragment: new contact form
//! POST /contacts/new                        -- Create contact
//! GET  /contacts/{id}                       -- HTMX fragment: contact detail
//! POST /contacts/{id}                       -- Update contact
//! POST /contacts/{id}/delete                -- Delete contact
//! GET  /contacts/relationships/new          -- HTMX fragment: new relationship form
//! POST /contacts/relationships/new          -- Create relationship
//! GET  /contacts/relationships/{id}         -- HTMX fragment: relationship detail
//! POST /contacts/relationships/{id}         -- Update relationship
//! POST /contacts/relationships/{id}/delete  -- Delete relationship
//! ```

use askama::Template;
use axum::{
    Form, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::error;

use crate::db::contact_graph;
use crate::filters;
use crate::middleware::auth::RequireAdminAuth;
use crate::routes::dashboard::AdminUserView;
use crate::state::AppState;

// =============================================================================
// Constants
// =============================================================================

const RELATIONSHIP_TYPES: &[&str] = &[
    "works_at",
    "ceo_of",
    "founder_of",
    "supplies",
    "manufactures_for",
    "partners_with",
    "customer_of",
];

// =============================================================================
// Query / Form Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ContactForm {
    pub contact_type: String,
    pub name: String,
    pub email: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RelationshipForm {
    pub from_contact_id: i32,
    pub to_contact_id: i32,
    pub relationship_type: String,
}

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "contacts/index.html")]
struct ContactsIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
}

/// Flattened view of a contact for the template (avoids nested `&String` comparison issues).
struct ContactView {
    id: i32,
    contact_type: String,
    name: String,
    email: String,
    domain: String,
    relationship_count: i64,
    email_contribution_count: i64,
}

impl From<contact_graph::ContactDetail> for ContactView {
    fn from(c: contact_graph::ContactDetail) -> Self {
        Self {
            id: c.id,
            contact_type: c.contact_type,
            name: c.name,
            email: c.email.unwrap_or_default(),
            domain: c.domain.unwrap_or_default(),
            relationship_count: c.relationship_count,
            email_contribution_count: c.email_contribution_count,
        }
    }
}

/// A relationship type option with pre-computed selected state.
struct RelationshipTypeOption {
    value: String,
    selected: bool,
}

/// Flattened view of a relationship for the template.
struct RelationshipView {
    id: i32,
    from_contact_id: i32,
    from_contact_name: String,
    to_contact_id: i32,
    to_contact_name: String,
    relationship_type: String,
    properties: String,
    email_contribution_count: i64,
}

impl From<contact_graph::RelationshipWithContacts> for RelationshipView {
    fn from(r: contact_graph::RelationshipWithContacts) -> Self {
        Self {
            id: r.id,
            from_contact_id: r.from_contact_id,
            from_contact_name: r.from_contact_name,
            to_contact_id: r.to_contact_id,
            to_contact_name: r.to_contact_name,
            relationship_type: r.relationship_type,
            properties: serde_json::to_string_pretty(&r.properties).unwrap_or_default(),
            email_contribution_count: r.email_contribution_count,
        }
    }
}

#[derive(Template)]
#[template(path = "contacts/_contact_detail.html")]
struct ContactDetailTemplate {
    contact: ContactView,
    relationships: Vec<contact_graph::ContactRelationshipEntry>,
    relationship_types: Vec<String>,
}

#[derive(Template)]
#[template(path = "contacts/_relationship_detail.html")]
struct RelationshipDetailTemplate {
    relationship: RelationshipView,
    relationship_types: Vec<RelationshipTypeOption>,
}

#[derive(Template)]
#[template(path = "contacts/_contact_form.html")]
struct ContactFormTemplate {}

#[derive(Template)]
#[template(path = "contacts/_relationship_form.html")]
struct RelationshipFormTemplate {
    contacts: Vec<contact_graph::ContactSearchResult>,
    relationship_types: Vec<String>,
}

#[derive(Template)]
#[template(path = "contacts/_empty_detail.html")]
struct EmptyDetailTemplate {}

// =============================================================================
// Router
// =============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/contacts", get(index))
        .route("/contacts/api/graph", get(graph_json))
        .route("/contacts/api/search", get(search_json))
        .route("/contacts/new", get(new_contact_form).post(create_contact))
        .route(
            "/contacts/relationships/new",
            get(new_relationship_form).post(create_relationship),
        )
        .route(
            "/contacts/relationships/{id}",
            get(relationship_detail).post(update_relationship),
        )
        .route(
            "/contacts/relationships/{id}/delete",
            post(delete_relationship),
        )
        .route("/contacts/{id}", get(contact_detail).post(update_contact))
        .route("/contacts/{id}/delete", post(delete_contact))
}

// =============================================================================
// Handlers: Pages
// =============================================================================

async fn index(RequireAdminAuth(admin): RequireAdminAuth) -> Response {
    let template = ContactsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/contacts".to_string(),
    };
    Html(template.render().unwrap_or_default()).into_response()
}

// =============================================================================
// Handlers: JSON API
// =============================================================================

async fn graph_json(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
) -> Response {
    match contact_graph::get_full_graph(state.pool()).await {
        Ok(data) => Json(data).into_response(),
        Err(e) => {
            error!(error = %e, "failed to fetch graph data");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn search_json(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Query(query): Query<SearchQuery>,
) -> Response {
    let q = query.q.unwrap_or_default();
    if q.is_empty() {
        return Json(Vec::<contact_graph::ContactSearchResult>::new()).into_response();
    }

    match contact_graph::search_contacts(state.pool(), &q).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => {
            error!(error = %e, "failed to search contacts");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// =============================================================================
// Handlers: Contact CRUD
// =============================================================================

async fn contact_detail(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    let pool = state.pool();

    let contact = match contact_graph::get_contact(pool, id).await {
        Ok(c) => c,
        Err(crate::db::RepositoryError::NotFound) => {
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(e) => {
            error!(error = %e, "failed to fetch contact");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let relationships = contact_graph::get_contact_relationships(pool, id)
        .await
        .unwrap_or_default();

    let template = ContactDetailTemplate {
        contact: contact.into(),
        relationships,
        relationship_types: relationship_type_list(),
    };
    Html(template.render().unwrap_or_default()).into_response()
}

async fn new_contact_form(RequireAdminAuth(_admin): RequireAdminAuth) -> Response {
    let template = ContactFormTemplate {};
    Html(template.render().unwrap_or_default()).into_response()
}

async fn create_contact(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Form(form): Form<ContactForm>,
) -> Response {
    let params = contact_graph::CreateContactParams {
        contact_type: &form.contact_type,
        name: &form.name,
        email: form.email.as_deref().filter(|s| !s.is_empty()),
        domain: form.domain.as_deref().filter(|s| !s.is_empty()),
    };

    match contact_graph::create_contact(state.pool(), &params).await {
        Ok(node) => graph_updated_response(node.id, state.pool()).await,
        Err(e) => {
            error!(error = %e, "failed to create contact");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn update_contact(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<ContactForm>,
) -> Response {
    let params = contact_graph::UpdateContactParams {
        contact_type: &form.contact_type,
        name: &form.name,
        email: form.email.as_deref().filter(|s| !s.is_empty()),
        domain: form.domain.as_deref().filter(|s| !s.is_empty()),
    };

    match contact_graph::update_contact(state.pool(), id, &params).await {
        Ok(_) => graph_updated_response(id, state.pool()).await,
        Err(e) => {
            error!(error = %e, contact_id = id, "failed to update contact");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn delete_contact(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    match contact_graph::delete_contact(state.pool(), id).await {
        Ok(()) => empty_with_graph_trigger(),
        Err(e) => {
            error!(error = %e, contact_id = id, "failed to delete contact");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// =============================================================================
// Handlers: Relationship CRUD
// =============================================================================

async fn relationship_detail(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    match contact_graph::get_relationship(state.pool(), id).await {
        Ok(rel) => {
            let rel_type = rel.relationship_type.clone();
            let template = RelationshipDetailTemplate {
                relationship: rel.into(),
                relationship_types: relationship_type_options(&rel_type),
            };
            Html(template.render().unwrap_or_default()).into_response()
        }
        Err(crate::db::RepositoryError::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!(error = %e, "failed to fetch relationship");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn new_relationship_form(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
) -> Response {
    let contacts = contact_graph::search_contacts(state.pool(), "")
        .await
        .unwrap_or_default();

    // If search with empty string returns nothing, fetch all contacts
    let contacts = if contacts.is_empty() {
        fetch_all_contacts_for_picker(state.pool()).await
    } else {
        contacts
    };

    let template = RelationshipFormTemplate {
        contacts,
        relationship_types: relationship_type_list(),
    };
    Html(template.render().unwrap_or_default()).into_response()
}

async fn create_relationship(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Form(form): Form<RelationshipForm>,
) -> Response {
    if form.from_contact_id == form.to_contact_id {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let params = contact_graph::CreateRelationshipParams {
        from_contact_id: form.from_contact_id,
        to_contact_id: form.to_contact_id,
        relationship_type: form.relationship_type,
    };

    match contact_graph::create_relationship(state.pool(), &params).await {
        Ok(_) => empty_with_graph_trigger(),
        Err(e) => {
            error!(error = %e, "failed to create relationship");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn update_relationship(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<RelationshipForm>,
) -> Response {
    let pool = state.pool();

    match contact_graph::update_relationship(pool, id, &form.relationship_type).await {
        Ok(_) => {
            // Return updated detail
            match contact_graph::get_relationship(pool, id).await {
                Ok(rel) => {
                    let rel_type = rel.relationship_type.clone();
                    let template = RelationshipDetailTemplate {
                        relationship: rel.into(),
                        relationship_types: relationship_type_options(&rel_type),
                    };
                    let html = template.render().unwrap_or_default();
                    ([("HX-Trigger", "graph-updated")], Html(html)).into_response()
                }
                Err(_) => empty_with_graph_trigger(),
            }
        }
        Err(e) => {
            error!(error = %e, relationship_id = id, "failed to update relationship");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn delete_relationship(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    match contact_graph::delete_relationship(state.pool(), id).await {
        Ok(()) => empty_with_graph_trigger(),
        Err(e) => {
            error!(error = %e, relationship_id = id, "failed to delete relationship");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn relationship_type_list() -> Vec<String> {
    RELATIONSHIP_TYPES
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

fn relationship_type_options(selected: &str) -> Vec<RelationshipTypeOption> {
    RELATIONSHIP_TYPES
        .iter()
        .map(|s| RelationshipTypeOption {
            value: (*s).to_string(),
            selected: *s == selected,
        })
        .collect()
}

/// Return the contact detail panel with an `HX-Trigger: graph-updated` header.
async fn graph_updated_response(contact_id: i32, pool: &sqlx::PgPool) -> Response {
    let Ok(contact) = contact_graph::get_contact(pool, contact_id).await else {
        return empty_with_graph_trigger();
    };

    let relationships = contact_graph::get_contact_relationships(pool, contact_id)
        .await
        .unwrap_or_default();

    let template = ContactDetailTemplate {
        contact: contact.into(),
        relationships,
        relationship_types: relationship_type_list(),
    };

    let html = template.render().unwrap_or_default();
    ([("HX-Trigger", "graph-updated")], Html(html)).into_response()
}

/// Return the empty detail panel with `HX-Trigger: graph-updated`.
fn empty_with_graph_trigger() -> Response {
    let template = EmptyDetailTemplate {};
    let html = template.render().unwrap_or_default();
    ([("HX-Trigger", "graph-updated")], Html(html)).into_response()
}

/// Fetch all contacts for the relationship picker (when search returns empty).
async fn fetch_all_contacts_for_picker(
    pool: &sqlx::PgPool,
) -> Vec<contact_graph::ContactSearchResult> {
    sqlx::query_as!(
        contact_graph::ContactSearchResult,
        r#"
        SELECT id, name, contact_type, email
        FROM admin.contacts
        ORDER BY name
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}
