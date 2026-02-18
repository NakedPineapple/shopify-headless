//! Database operations for the contact graph.
//!
//! Two-table relational graph: `admin.contacts` (nodes) and
//! `admin.contact_relationships` (edges). Supports fuzzy name search,
//! multi-hop traversal, and upsert for dynamic graph growth.

use sqlx::PgPool;
use tracing::instrument;

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// A contact node (person or organization).
#[derive(Debug)]
pub struct Contact {
    pub id: i32,
    pub contact_type: String,
    pub name: String,
    pub email: Option<String>,
    pub domain: Option<String>,
    pub metadata: serde_json::Value,
}

/// A relationship edge with the connected contact.
#[derive(Debug)]
pub struct Relationship {
    pub relationship_type: String,
    pub properties: serde_json::Value,
    pub connected: Contact,
}

/// A contact with its immediate relationships.
#[derive(Debug)]
pub struct ContactWithRelationships {
    pub contact: Contact,
    pub relationships: Vec<Relationship>,
}

/// Parameters for upserting a contact.
pub struct UpsertContactParams<'a> {
    pub contact_type: &'a str,
    pub name: &'a str,
    pub email: Option<&'a str>,
    pub domain: Option<&'a str>,
}

/// Parameters for upserting a relationship.
pub struct UpsertRelationshipParams {
    pub from_id: i32,
    pub to_id: i32,
    pub relationship_type: String,
    pub properties: serde_json::Value,
}

/// IDs of contacts and relationships that an email contributed to.
#[derive(Debug, Default)]
pub struct GraphContributions {
    pub contact_ids: Vec<i32>,
    pub relationship_ids: Vec<i32>,
}

// =============================================================================
// Read Queries
// =============================================================================

/// Find a contact by exact email match.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<Contact>, RepositoryError> {
    let row = sqlx::query_as!(
        ContactRow,
        r#"
        SELECT id, contact_type, name, email, domain, metadata
        FROM admin.contacts
        WHERE email = $1
        "#,
        email,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(Into::into))
}

/// Find contacts by exact domain match.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn find_by_domain(pool: &PgPool, domain: &str) -> Result<Vec<Contact>, RepositoryError> {
    let rows = sqlx::query_as!(
        ContactRow,
        r#"
        SELECT id, contact_type, name, email, domain, metadata
        FROM admin.contacts
        WHERE domain = $1
        "#,
        domain,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

/// Search contacts by trigram similarity on name, email, or domain.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn search(pool: &PgPool, query: &str) -> Result<Vec<Contact>, RepositoryError> {
    let pattern = format!("%{query}%");
    let rows = sqlx::query_as!(
        ContactRow,
        r#"
        SELECT id, contact_type, name, email, domain, metadata
        FROM admin.contacts
        WHERE name ILIKE $1 OR email ILIKE $1 OR domain ILIKE $1
        ORDER BY similarity(name, $2) DESC
        LIMIT 5
        "#,
        pattern,
        query,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

/// 2-hop graph traversal from a starting contact.
///
/// Returns all contacts reachable within `max_depth` hops, along with the
/// relationship that connects them. Uses a recursive CTE to traverse both
/// directions of the relationship edges.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn get_neighborhood(
    pool: &PgPool,
    contact_id: i32,
    max_depth: i32,
) -> Result<Vec<ContactWithRelationships>, RepositoryError> {
    let rows = sqlx::query_as!(
        NeighborRow,
        r#"
        WITH RECURSIVE graph AS (
            SELECT c.id, c.name, c.contact_type, c.email, c.domain, c.metadata,
                   0 as depth, ARRAY[c.id] as path
            FROM admin.contacts c WHERE c.id = $1
            UNION ALL
            SELECT c2.id, c2.name, c2.contact_type, c2.email, c2.domain, c2.metadata,
                   g.depth + 1, g.path || c2.id
            FROM graph g
            JOIN admin.contact_relationships r
                ON g.id = r.from_contact_id OR g.id = r.to_contact_id
            JOIN admin.contacts c2
                ON c2.id = CASE WHEN g.id = r.from_contact_id THEN r.to_contact_id
                                ELSE r.from_contact_id END
            WHERE g.depth < $2 AND NOT c2.id = ANY(g.path)
        )
        SELECT DISTINCT ON (g.id)
            g.id as "contact_id!",
            g.name as "contact_name!",
            g.contact_type as "contact_type!",
            g.email,
            g.domain,
            g.metadata as "contact_metadata!",
            g.depth as "depth!"
        FROM graph g
        ORDER BY g.id, g.depth
        "#,
        contact_id,
        max_depth,
    )
    .fetch_all(pool)
    .await?;

    let mut results = Vec::new();
    for row in &rows {
        let contact = Contact {
            id: row.contact_id,
            contact_type: row.contact_type.clone(),
            name: row.contact_name.clone(),
            email: row.email.clone(),
            domain: row.domain.clone(),
            metadata: row.contact_metadata.clone(),
        };

        let rels = fetch_relationships(pool, row.contact_id, &rows).await?;
        results.push(ContactWithRelationships {
            contact,
            relationships: rels,
        });
    }

    Ok(results)
}

/// Fetch relationships for a contact, filtered to only include those
/// connecting to other contacts in the neighborhood.
async fn fetch_relationships(
    pool: &PgPool,
    contact_id: i32,
    neighborhood: &[NeighborRow],
) -> Result<Vec<Relationship>, RepositoryError> {
    let neighbor_ids: Vec<i32> = neighborhood.iter().map(|n| n.contact_id).collect();

    let rows = sqlx::query_as!(
        RelationshipRow,
        r#"
        SELECT
            r.relationship_type,
            r.properties,
            c.id as "connected_id!",
            c.contact_type as "connected_type!",
            c.name as "connected_name!",
            c.email as connected_email,
            c.domain as connected_domain,
            c.metadata as "connected_metadata!"
        FROM admin.contact_relationships r
        JOIN admin.contacts c
            ON c.id = CASE WHEN r.from_contact_id = $1 THEN r.to_contact_id
                           ELSE r.from_contact_id END
        WHERE (r.from_contact_id = $1 OR r.to_contact_id = $1)
          AND c.id = ANY($2)
        "#,
        contact_id,
        &neighbor_ids,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

/// Look up a sender by email (falling back to domain) and format the graph
/// context as a human-readable string suitable for prompt injection.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn lookup_sender(
    pool: &PgPool,
    email: &str,
    domain: &str,
) -> Result<Option<String>, RepositoryError> {
    // Try exact email match first
    let contact = find_by_email(pool, email).await?;

    // Fall back to domain match
    let contact = match contact {
        Some(c) => Some(c),
        None => find_by_domain(pool, domain).await?.into_iter().next(),
    };

    let Some(contact) = contact else {
        return Ok(None);
    };

    let neighborhood = get_neighborhood(pool, contact.id, 2).await?;
    Ok(Some(format_graph_context(&neighborhood)))
}

/// Format a neighborhood graph as human-readable text for LLM context.
#[must_use]
pub fn format_graph_context(neighborhood: &[ContactWithRelationships]) -> String {
    use std::fmt::Write;
    let mut output = String::new();

    for entry in neighborhood {
        let c = &entry.contact;
        let _ = write!(output, "{} ({})", c.name, c.contact_type);
        if let Some(ref email) = c.email {
            let _ = write!(output, " <{email}>");
        }
        if let Some(ref domain) = c.domain {
            let _ = write!(output, " [{domain}]");
        }
        output.push('\n');

        for rel in &entry.relationships {
            let _ = write!(
                output,
                "  → {} → {} ({})",
                rel.relationship_type.to_uppercase(),
                rel.connected.name,
                rel.connected.contact_type,
            );
            if let Some(ctx) = rel.properties.get("context").and_then(|v| v.as_str()) {
                let _ = write!(output, " {{{ctx}}}");
            }
            output.push('\n');
        }
    }

    output
}

// =============================================================================
// Write Queries (Graph Growth)
// =============================================================================

/// Upsert a contact node: insert or update on email/domain conflict.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, params), fields(name = %params.name))]
pub async fn upsert_contact(
    pool: &PgPool,
    params: &UpsertContactParams<'_>,
) -> Result<Contact, RepositoryError> {
    let row = sqlx::query_as!(
        ContactRow,
        r#"
        INSERT INTO admin.contacts (contact_type, name, email, domain)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (email) WHERE email IS NOT NULL DO UPDATE
            SET name = EXCLUDED.name,
                updated_at = NOW()
        RETURNING id, contact_type, name, email, domain, metadata
        "#,
        params.contact_type,
        params.name,
        params.email,
        params.domain,
    )
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

/// Find an existing organization by domain, or create a new one.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn find_or_create_org_from_domain(
    pool: &PgPool,
    domain: &str,
) -> Result<Contact, RepositoryError> {
    if let Some(existing) = find_by_domain(pool, domain)
        .await?
        .into_iter()
        .find(|c| c.contact_type == "organization")
    {
        return Ok(existing);
    }

    // Derive a readable name from the domain
    let name = domain
        .split('.')
        .next()
        .unwrap_or(domain)
        .replace(['-', '_'], " ");

    // Title-case the name
    let name: String = name
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                let upper: String = first.to_uppercase().collect();
                format!("{upper}{}", chars.as_str())
            })
        })
        .collect::<Vec<_>>()
        .join(" ");

    upsert_contact(
        pool,
        &UpsertContactParams {
            contact_type: "organization",
            name: &name,
            email: None,
            domain: Some(domain),
        },
    )
    .await
}

/// Upsert a relationship edge between two contacts.
///
/// On conflict, merges JSONB properties, increments `email_count`, and
/// updates `last_contact_date`. Returns the relationship ID.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, params))]
pub async fn upsert_relationship(
    pool: &PgPool,
    params: &UpsertRelationshipParams,
) -> Result<i32, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.contact_relationships
            (from_contact_id, to_contact_id, relationship_type, properties)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (from_contact_id, to_contact_id, relationship_type) DO UPDATE
            SET properties = admin.contact_relationships.properties || EXCLUDED.properties
                || jsonb_build_object(
                    'last_contact_date', to_char(NOW(), 'YYYY-MM-DD'),
                    'email_count', COALESCE(
                        (admin.contact_relationships.properties->>'email_count')::int, 0
                    ) + 1
                ),
                updated_at = NOW()
        RETURNING id
        "#,
        params.from_id,
        params.to_id,
        params.relationship_type,
        params.properties,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Find a contact by name (exact, case-insensitive).
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<Contact>, RepositoryError> {
    let row = sqlx::query_as!(
        ContactRow,
        r#"
        SELECT id, contact_type, name, email, domain, metadata
        FROM admin.contacts
        WHERE LOWER(name) = LOWER($1)
        LIMIT 1
        "#,
        name,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(Into::into))
}

// =============================================================================
// Contribution Tracking
// =============================================================================

/// Record that an email contributed to a set of contacts and relationships.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, contributions))]
pub async fn record_contributions(
    pool: &PgPool,
    email_id: i32,
    contributions: &GraphContributions,
) -> Result<(), RepositoryError> {
    if !contributions.contact_ids.is_empty() {
        sqlx::query!(
            r#"
            INSERT INTO admin.email_contact_contributions (email_id, contact_id)
            SELECT $1, UNNEST($2::int[])
            ON CONFLICT DO NOTHING
            "#,
            email_id,
            &contributions.contact_ids,
        )
        .execute(pool)
        .await?;
    }

    if !contributions.relationship_ids.is_empty() {
        sqlx::query!(
            r#"
            INSERT INTO admin.email_relationship_contributions (email_id, relationship_id)
            SELECT $1, UNNEST($2::int[])
            ON CONFLICT DO NOTHING
            "#,
            email_id,
            &contributions.relationship_ids,
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Retract an email's graph contributions and delete orphaned entities.
///
/// Removes the email's contribution records, then deletes any relationships
/// and contacts that no longer have contributions from any email. A contact
/// is also kept if it still participates in at least one relationship.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn retract_contributions(pool: &PgPool, email_id: i32) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    // Snapshot what this email contributed to
    let old = get_contributions_tx(&mut tx, email_id).await?;

    // Remove contribution records
    sqlx::query!(
        "DELETE FROM admin.email_contact_contributions WHERE email_id = $1",
        email_id,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "DELETE FROM admin.email_relationship_contributions WHERE email_id = $1",
        email_id,
    )
    .execute(&mut *tx)
    .await?;

    // Delete orphaned relationships (no remaining contributions)
    if !old.relationship_ids.is_empty() {
        delete_orphaned_relationships_tx(&mut tx, &old.relationship_ids).await?;
    }

    // Delete orphaned contacts (no remaining contributions AND no remaining edges)
    if !old.contact_ids.is_empty() {
        delete_orphaned_contacts_tx(&mut tx, &old.contact_ids).await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Fetch the contacts and relationships an email contributed to (within a tx).
async fn get_contributions_tx(
    conn: &mut sqlx::PgConnection,
    email_id: i32,
) -> Result<GraphContributions, RepositoryError> {
    let contact_ids: Vec<i32> = sqlx::query_scalar!(
        "SELECT contact_id FROM admin.email_contact_contributions WHERE email_id = $1",
        email_id,
    )
    .fetch_all(&mut *conn)
    .await?;

    let relationship_ids: Vec<i32> = sqlx::query_scalar!(
        "SELECT relationship_id FROM admin.email_relationship_contributions WHERE email_id = $1",
        email_id,
    )
    .fetch_all(&mut *conn)
    .await?;

    Ok(GraphContributions {
        contact_ids,
        relationship_ids,
    })
}

/// Delete relationships that no other email claims.
async fn delete_orphaned_relationships_tx(
    conn: &mut sqlx::PgConnection,
    relationship_ids: &[i32],
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        DELETE FROM admin.contact_relationships
        WHERE id = ANY($1)
          AND NOT EXISTS (
              SELECT 1 FROM admin.email_relationship_contributions
              WHERE relationship_id = admin.contact_relationships.id
          )
        "#,
        relationship_ids,
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Delete contacts that no other email claims and that have no remaining edges.
async fn delete_orphaned_contacts_tx(
    conn: &mut sqlx::PgConnection,
    contact_ids: &[i32],
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        DELETE FROM admin.contacts
        WHERE id = ANY($1)
          AND NOT EXISTS (
              SELECT 1 FROM admin.email_contact_contributions
              WHERE contact_id = admin.contacts.id
          )
          AND NOT EXISTS (
              SELECT 1 FROM admin.contact_relationships
              WHERE from_contact_id = admin.contacts.id
                 OR to_contact_id = admin.contacts.id
          )
        "#,
        contact_ids,
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

// =============================================================================
// Internal Row Types
// =============================================================================

#[derive(Debug)]
struct ContactRow {
    id: i32,
    contact_type: String,
    name: String,
    email: Option<String>,
    domain: Option<String>,
    metadata: serde_json::Value,
}

impl From<ContactRow> for Contact {
    fn from(row: ContactRow) -> Self {
        Self {
            id: row.id,
            contact_type: row.contact_type,
            name: row.name,
            email: row.email,
            domain: row.domain,
            metadata: row.metadata,
        }
    }
}

#[derive(Debug)]
struct NeighborRow {
    contact_id: i32,
    contact_name: String,
    contact_type: String,
    email: Option<String>,
    domain: Option<String>,
    contact_metadata: serde_json::Value,
    // Selected by the CTE for depth filtering/ordering but unused in Rust.
    #[allow(dead_code)]
    depth: i32,
}

#[derive(Debug)]
struct RelationshipRow {
    relationship_type: String,
    properties: serde_json::Value,
    connected_id: i32,
    connected_type: String,
    connected_name: String,
    connected_email: Option<String>,
    connected_domain: Option<String>,
    connected_metadata: serde_json::Value,
}

impl From<RelationshipRow> for Relationship {
    fn from(row: RelationshipRow) -> Self {
        Self {
            relationship_type: row.relationship_type,
            properties: row.properties,
            connected: Contact {
                id: row.connected_id,
                contact_type: row.connected_type,
                name: row.connected_name,
                email: row.connected_email,
                domain: row.connected_domain,
                metadata: row.connected_metadata,
            },
        }
    }
}
