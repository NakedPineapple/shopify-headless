//! Database operations for the contact graph visualization.
//!
//! Read/write queries against `admin.contacts` and `admin.contact_relationships`
//! for the admin UI. The automations crate owns upsert-on-email-processing; this
//! module provides direct CRUD for manual graph editing.

use serde::Serialize;
use sqlx::PgPool;
use tracing::instrument;

use super::RepositoryError;

// =============================================================================
// View Types (serializable for JSON API + templates)
// =============================================================================

/// A contact node for the graph visualization.
#[derive(Debug, Clone, Serialize)]
pub struct ContactNode {
    pub id: i32,
    pub contact_type: String,
    pub name: String,
    pub email: Option<String>,
    pub domain: Option<String>,
    pub metadata: serde_json::Value,
}

/// A relationship edge for the graph visualization.
#[derive(Debug, Clone, Serialize)]
pub struct RelationshipEdge {
    pub id: i32,
    pub from_contact_id: i32,
    pub to_contact_id: i32,
    pub relationship_type: String,
    pub properties: serde_json::Value,
}

/// Full graph data for the Cytoscape.js visualization.
#[derive(Debug, Serialize)]
pub struct GraphData {
    pub nodes: Vec<ContactNode>,
    pub edges: Vec<RelationshipEdge>,
}

/// Contact detail with relationship and contribution counts.
#[derive(Debug)]
pub struct ContactDetail {
    pub id: i32,
    pub contact_type: String,
    pub name: String,
    pub email: Option<String>,
    pub domain: Option<String>,
    pub metadata: serde_json::Value,
    pub relationship_count: i64,
    pub email_contribution_count: i64,
}

/// A relationship's connected contact info (for detail view).
#[derive(Debug)]
pub struct RelationshipWithContacts {
    pub id: i32,
    pub from_contact_id: i32,
    pub from_contact_name: String,
    pub to_contact_id: i32,
    pub to_contact_name: String,
    pub relationship_type: String,
    pub properties: serde_json::Value,
    pub email_contribution_count: i64,
}

/// A contact's relationship (for the detail panel list).
#[derive(Debug)]
pub struct ContactRelationshipEntry {
    pub id: i32,
    pub relationship_type: String,
    pub direction: String,
    pub other_id: i32,
    pub other_name: String,
    pub other_type: String,
}

/// Search result for the contact picker.
#[derive(Debug, Serialize)]
pub struct ContactSearchResult {
    pub id: i32,
    pub name: String,
    pub contact_type: String,
    pub email: Option<String>,
}

// =============================================================================
// Read Queries
// =============================================================================

/// Fetch the entire graph (all contacts + all relationships).
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn get_full_graph(pool: &PgPool) -> Result<GraphData, RepositoryError> {
    let nodes = sqlx::query_as!(
        ContactNode,
        r#"
        SELECT id, contact_type, name, email, domain, metadata
        FROM admin.contacts
        ORDER BY name
        "#
    )
    .fetch_all(pool)
    .await?;

    let edges = sqlx::query_as!(
        RelationshipEdge,
        r#"
        SELECT id, from_contact_id, to_contact_id, relationship_type, properties
        FROM admin.contact_relationships
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(GraphData { nodes, edges })
}

/// Fetch a single contact with relationship and contribution counts.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn get_contact(pool: &PgPool, id: i32) -> Result<ContactDetail, RepositoryError> {
    let row = sqlx::query_as!(
        ContactDetailRow,
        r#"
        SELECT
            c.id, c.contact_type, c.name, c.email, c.domain, c.metadata,
            (SELECT COUNT(*) FROM admin.contact_relationships
             WHERE from_contact_id = c.id OR to_contact_id = c.id
            ) as "relationship_count!",
            (SELECT COUNT(*) FROM admin.email_contact_contributions
             WHERE contact_id = c.id
            ) as "email_contribution_count!"
        FROM admin.contacts c
        WHERE c.id = $1
        "#,
        id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(RepositoryError::NotFound)?;

    Ok(row.into())
}

/// Fetch relationships for a contact (for the detail panel list).
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn get_contact_relationships(
    pool: &PgPool,
    contact_id: i32,
) -> Result<Vec<ContactRelationshipEntry>, RepositoryError> {
    let rows = sqlx::query_as!(
        ContactRelationshipRow,
        r#"
        SELECT
            r.id,
            r.relationship_type,
            CASE WHEN r.from_contact_id = $1 THEN 'outgoing' ELSE 'incoming' END as "direction!",
            CASE WHEN r.from_contact_id = $1 THEN r.to_contact_id ELSE r.from_contact_id END as "other_id!",
            c.name as "other_name!",
            c.contact_type as "other_type!"
        FROM admin.contact_relationships r
        JOIN admin.contacts c
            ON c.id = CASE WHEN r.from_contact_id = $1 THEN r.to_contact_id
                           ELSE r.from_contact_id END
        WHERE r.from_contact_id = $1 OR r.to_contact_id = $1
        ORDER BY r.relationship_type
        "#,
        contact_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

/// Fetch a single relationship with connected contact names.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn get_relationship(
    pool: &PgPool,
    id: i32,
) -> Result<RelationshipWithContacts, RepositoryError> {
    let row = sqlx::query_as!(
        RelationshipDetailRow,
        r#"
        SELECT
            r.id,
            r.from_contact_id,
            cf.name as "from_contact_name!",
            r.to_contact_id,
            ct.name as "to_contact_name!",
            r.relationship_type,
            r.properties,
            (SELECT COUNT(*) FROM admin.email_relationship_contributions
             WHERE relationship_id = r.id
            ) as "email_contribution_count!"
        FROM admin.contact_relationships r
        JOIN admin.contacts cf ON cf.id = r.from_contact_id
        JOIN admin.contacts ct ON ct.id = r.to_contact_id
        WHERE r.id = $1
        "#,
        id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(RepositoryError::NotFound)?;

    Ok(row.into())
}

/// Search contacts by name/email/domain for the contact picker.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn search_contacts(
    pool: &PgPool,
    query: &str,
) -> Result<Vec<ContactSearchResult>, RepositoryError> {
    let pattern = format!("%{query}%");
    let rows = sqlx::query_as!(
        ContactSearchResult,
        r#"
        SELECT id, name, contact_type, email
        FROM admin.contacts
        WHERE name ILIKE $1 OR email ILIKE $1 OR domain ILIKE $1
        ORDER BY name
        LIMIT 20
        "#,
        pattern,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

// =============================================================================
// Write Queries
// =============================================================================

/// Parameters for creating a contact.
pub struct CreateContactParams<'a> {
    pub contact_type: &'a str,
    pub name: &'a str,
    pub email: Option<&'a str>,
    pub domain: Option<&'a str>,
}

/// Create a new contact node.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, params), fields(name = %params.name))]
pub async fn create_contact(
    pool: &PgPool,
    params: &CreateContactParams<'_>,
) -> Result<ContactNode, RepositoryError> {
    let row = sqlx::query_as!(
        ContactNode,
        r#"
        INSERT INTO admin.contacts (contact_type, name, email, domain)
        VALUES ($1, $2, $3, $4)
        RETURNING id, contact_type, name, email, domain, metadata
        "#,
        params.contact_type,
        params.name,
        params.email,
        params.domain,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Parameters for updating a contact.
pub struct UpdateContactParams<'a> {
    pub contact_type: &'a str,
    pub name: &'a str,
    pub email: Option<&'a str>,
    pub domain: Option<&'a str>,
}

/// Update an existing contact node.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, params), fields(contact_id = id))]
pub async fn update_contact(
    pool: &PgPool,
    id: i32,
    params: &UpdateContactParams<'_>,
) -> Result<ContactNode, RepositoryError> {
    let row = sqlx::query_as!(
        ContactNode,
        r#"
        UPDATE admin.contacts
        SET contact_type = $2, name = $3, email = $4, domain = $5,
            updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        WHERE id = $1
        RETURNING id, contact_type, name, email, domain, metadata
        "#,
        id,
        params.contact_type,
        params.name,
        params.email,
        params.domain,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(RepositoryError::NotFound)?;

    Ok(row)
}

/// Delete a contact node (cascades to relationships).
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn delete_contact(pool: &PgPool, id: i32) -> Result<(), RepositoryError> {
    let result = sqlx::query!("DELETE FROM admin.contacts WHERE id = $1", id,)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound);
    }

    Ok(())
}

/// Parameters for creating a relationship.
#[derive(Debug)]
pub struct CreateRelationshipParams {
    pub from_contact_id: i32,
    pub to_contact_id: i32,
    pub relationship_type: String,
}

/// Create a new relationship edge.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn create_relationship(
    pool: &PgPool,
    params: &CreateRelationshipParams,
) -> Result<RelationshipEdge, RepositoryError> {
    let row = sqlx::query_as!(
        RelationshipEdge,
        r#"
        INSERT INTO admin.contact_relationships
            (from_contact_id, to_contact_id, relationship_type)
        VALUES ($1, $2, $3)
        RETURNING id, from_contact_id, to_contact_id, relationship_type, properties
        "#,
        params.from_contact_id,
        params.to_contact_id,
        params.relationship_type,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Update an existing relationship edge.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn update_relationship(
    pool: &PgPool,
    id: i32,
    relationship_type: &str,
) -> Result<RelationshipEdge, RepositoryError> {
    let row = sqlx::query_as!(
        RelationshipEdge,
        r#"
        UPDATE admin.contact_relationships
        SET relationship_type = $2,
            updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        WHERE id = $1
        RETURNING id, from_contact_id, to_contact_id, relationship_type, properties
        "#,
        id,
        relationship_type,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(RepositoryError::NotFound)?;

    Ok(row)
}

/// Delete a relationship edge.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn delete_relationship(pool: &PgPool, id: i32) -> Result<(), RepositoryError> {
    let result = sqlx::query!("DELETE FROM admin.contact_relationships WHERE id = $1", id,)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound);
    }

    Ok(())
}

// =============================================================================
// Internal Row Types
// =============================================================================

struct ContactDetailRow {
    id: i32,
    contact_type: String,
    name: String,
    email: Option<String>,
    domain: Option<String>,
    metadata: serde_json::Value,
    relationship_count: i64,
    email_contribution_count: i64,
}

impl From<ContactDetailRow> for ContactDetail {
    fn from(row: ContactDetailRow) -> Self {
        Self {
            id: row.id,
            contact_type: row.contact_type,
            name: row.name,
            email: row.email,
            domain: row.domain,
            metadata: row.metadata,
            relationship_count: row.relationship_count,
            email_contribution_count: row.email_contribution_count,
        }
    }
}

struct RelationshipDetailRow {
    id: i32,
    from_contact_id: i32,
    from_contact_name: String,
    to_contact_id: i32,
    to_contact_name: String,
    relationship_type: String,
    properties: serde_json::Value,
    email_contribution_count: i64,
}

impl From<RelationshipDetailRow> for RelationshipWithContacts {
    fn from(row: RelationshipDetailRow) -> Self {
        Self {
            id: row.id,
            from_contact_id: row.from_contact_id,
            from_contact_name: row.from_contact_name,
            to_contact_id: row.to_contact_id,
            to_contact_name: row.to_contact_name,
            relationship_type: row.relationship_type,
            properties: row.properties,
            email_contribution_count: row.email_contribution_count,
        }
    }
}

struct ContactRelationshipRow {
    id: i32,
    relationship_type: String,
    direction: String,
    other_id: i32,
    other_name: String,
    other_type: String,
}

impl From<ContactRelationshipRow> for ContactRelationshipEntry {
    fn from(row: ContactRelationshipRow) -> Self {
        Self {
            id: row.id,
            relationship_type: row.relationship_type,
            direction: row.direction,
            other_id: row.other_id,
            other_name: row.other_name,
            other_type: row.other_type,
        }
    }
}
