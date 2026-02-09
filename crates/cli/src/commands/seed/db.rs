//! Database operations for tool example seeding.
//!
//! These queries use runtime `query_as` / `query` to avoid `SQLx` offline
//! cache requirements for the CLI.

use sqlx::PgPool;
use tracing::{debug, instrument};

/// Parameters for creating a new tool example.
#[derive(Debug)]
pub struct CreateToolExample {
    /// Tool name.
    pub tool_name: String,
    /// Domain.
    pub domain: String,
    /// Example query text.
    pub example_query: String,
    /// Embedding vector (1536 dimensions for `OpenAI` `text-embedding-3-small`).
    pub embedding: Vec<f32>,
    /// Whether this is a learned example.
    pub is_learned: bool,
}

/// Domain count for statistics.
#[derive(Debug)]
pub struct DomainCount {
    /// Domain name.
    pub domain: String,
    /// Number of examples.
    pub count: i64,
}

/// Internal row type for domain count query.
#[derive(sqlx::FromRow)]
struct DomainCountRow {
    domain: String,
    count: Option<i64>,
}

/// Insert a new tool example with its embedding.
///
/// # Errors
///
/// Returns error if the database insert fails.
#[instrument(skip(pool, params), fields(tool = %params.tool_name, domain = %params.domain))]
pub async fn insert_tool_example(
    pool: &PgPool,
    params: CreateToolExample,
) -> Result<i32, sqlx::Error> {
    let embedding_str = format_embedding(&params.embedding);

    let result: (i32,) = sqlx::query_as(
        r"
        INSERT INTO admin.tool_example_queries
            (tool_name, domain, example_query, embedding, is_learned, usage_count)
        VALUES ($1, $2, $3, $4::vector, $5, 0)
        RETURNING id
        ",
    )
    .bind(&params.tool_name)
    .bind(&params.domain)
    .bind(&params.example_query)
    .bind(&embedding_str)
    .bind(params.is_learned)
    .fetch_one(pool)
    .await?;

    debug!(id = result.0, "Inserted tool example");
    Ok(result.0)
}

/// Check if an example query already exists for a tool.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn example_exists(
    pool: &PgPool,
    tool_name: &str,
    example_query: &str,
) -> Result<bool, sqlx::Error> {
    let row: (bool,) = sqlx::query_as(
        r"
        SELECT EXISTS(
            SELECT 1 FROM admin.tool_example_queries
            WHERE tool_name = $1 AND example_query = $2
        )
        ",
    )
    .bind(tool_name)
    .bind(example_query)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Delete only pre-seeded examples (keep learned ones).
///
/// # Errors
///
/// Returns error if the database delete fails.
pub async fn delete_preseeded(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query::<sqlx::Postgres>(
        "DELETE FROM admin.tool_example_queries WHERE is_learned = FALSE",
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Get total count of tool examples.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_total_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM admin.tool_example_queries")
        .fetch_one(pool)
        .await?;

    Ok(count.0)
}

/// Get count of examples per domain.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_domain_counts(pool: &PgPool) -> Result<Vec<DomainCount>, sqlx::Error> {
    let rows: Vec<DomainCountRow> = sqlx::query_as(
        r"
        SELECT domain, COUNT(*) as count
        FROM admin.tool_example_queries
        GROUP BY domain
        ORDER BY domain
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| DomainCount {
            domain: r.domain,
            count: r.count.unwrap_or(0),
        })
        .collect())
}

/// Format an embedding vector for pgvector.
fn format_embedding(embedding: &[f32]) -> String {
    let values: Vec<String> = embedding.iter().map(ToString::to_string).collect();
    format!("[{}]", values.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_embedding() {
        let embedding = vec![0.1, 0.2, 0.3];
        let result = format_embedding(&embedding);
        assert_eq!(result, "[0.1,0.2,0.3]");
    }

    #[test]
    fn test_format_embedding_empty() {
        let embedding: Vec<f32> = vec![];
        let result = format_embedding(&embedding);
        assert_eq!(result, "[]");
    }
}
