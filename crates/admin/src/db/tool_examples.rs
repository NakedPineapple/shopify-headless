//! Database operations for tool example queries used in embedding-based tool selection.
//!
//! The `tool_example_queries` table stores example natural language queries
//! mapped to specific tools. These are used with pgvector for similarity search
//! to dynamically select relevant tools for user queries.

use sqlx::PgPool;
use tracing::{debug, instrument};

use super::RepositoryError;

/// A tool example query record.
#[derive(Debug, Clone)]
pub struct ToolExample {
    /// Primary key.
    pub id: i32,
    /// Tool name (e.g., `get_orders`).
    pub tool_name: String,
    /// Domain (e.g., "orders").
    pub domain: String,
    /// Example query text.
    pub example_query: String,
    /// Whether this was learned from successful usage vs pre-seeded.
    pub is_learned: bool,
    /// Number of times this example led to successful tool use.
    pub usage_count: i32,
}

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

/// Insert a new tool example with its embedding.
///
/// # Errors
///
/// Returns error if the database insert fails.
#[instrument(skip(pool, params), fields(tool = %params.tool_name, domain = %params.domain))]
pub async fn insert_tool_example(
    pool: &PgPool,
    params: CreateToolExample,
) -> Result<i32, RepositoryError> {
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

    let result = result.0;

    debug!(id = result, "Inserted tool example");
    Ok(result)
}

/// Batch insert multiple tool examples.
///
/// This is more efficient than inserting one at a time when seeding.
///
/// # Errors
///
/// Returns error if any insert fails.
#[instrument(skip(pool, examples), fields(count = examples.len()))]
pub async fn batch_insert_tool_examples(
    pool: &PgPool,
    examples: Vec<CreateToolExample>,
) -> Result<u64, RepositoryError> {
    let mut count = 0u64;

    for example in examples {
        insert_tool_example(pool, example).await?;
        count += 1;
    }

    debug!(count, "Batch inserted tool examples");
    Ok(count)
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
) -> Result<bool, RepositoryError> {
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

/// Increment usage count for an existing example.
///
/// # Errors
///
/// Returns error if the database update fails.
pub async fn increment_usage_count(pool: &PgPool, id: i32) -> Result<(), RepositoryError> {
    sqlx::query::<sqlx::Postgres>(
        r"
        UPDATE admin.tool_example_queries
        SET usage_count = usage_count + 1
        WHERE id = $1
        ",
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Find example ID by tool name and query text.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn find_example_id(
    pool: &PgPool,
    tool_name: &str,
    example_query: &str,
) -> Result<Option<i32>, RepositoryError> {
    let result: Option<(i32,)> = sqlx::query_as(
        r"
        SELECT id FROM admin.tool_example_queries
        WHERE tool_name = $1 AND example_query = $2
        LIMIT 1
        ",
    )
    .bind(tool_name)
    .bind(example_query)
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| r.0))
}

/// Search for similar tools using embedding similarity.
///
/// Uses pgvector's cosine distance operator for similarity search.
/// Returns tool names ordered by similarity score.
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `embedding` - Query embedding vector
/// * `domains` - List of domains to filter by
/// * `min_similarity` - Minimum similarity score (0.0 to 1.0)
/// * `limit` - Maximum number of results
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool, embedding), fields(domains = ?domains, limit))]
pub async fn search_similar_tools(
    pool: &PgPool,
    embedding: &[f32],
    domains: &[String],
    min_similarity: f64,
    limit: usize,
) -> Result<Vec<SimilarTool>, RepositoryError> {
    let embedding_str = format_embedding(embedding);

    // Using runtime query since SQLx doesn't have built-in pgvector support
    // Note: LIMIT is applied in Rust after sorting since DISTINCT ON requires
    // specific ORDER BY that doesn't match our final sort order
    let rows = sqlx::query_as::<_, SimilarToolRow>(
        r"
        SELECT DISTINCT ON (tool_name) tool_name,
               1 - (embedding <=> $1::vector) as similarity
        FROM admin.tool_example_queries
        WHERE domain = ANY($2)
        AND 1 - (embedding <=> $1::vector) > $3
        ORDER BY tool_name, similarity DESC
        ",
    )
    .bind(&embedding_str)
    .bind(domains)
    .bind(min_similarity)
    .fetch_all(pool)
    .await?;

    // Sort by similarity and apply limit in Rust
    let mut tools: Vec<SimilarTool> = rows
        .into_iter()
        .filter_map(|r| {
            r.similarity.map(|s| SimilarTool {
                tool_name: r.tool_name,
                similarity: s,
            })
        })
        .collect();

    tools.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tools.truncate(limit);

    debug!(count = tools.len(), "Found similar tools");
    Ok(tools)
}

/// Get tools by domain when no similar examples are found (fallback).
///
/// Returns tools ordered by usage count.
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool), fields(domains = ?domains, limit))]
pub async fn get_tools_by_domain(
    pool: &PgPool,
    domains: &[String],
    limit: usize,
) -> Result<Vec<String>, RepositoryError> {
    let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);

    // Using runtime query to avoid SQLx offline mode cache requirements
    let rows = sqlx::query_scalar::<_, String>(
        r"
        SELECT tool_name
        FROM (
            SELECT tool_name, MAX(usage_count) as max_usage
            FROM admin.tool_example_queries
            WHERE domain = ANY($1)
            GROUP BY tool_name
            ORDER BY max_usage DESC
            LIMIT $2
        ) sub
        ",
    )
    .bind(domains)
    .bind(limit_i64)
    .fetch_all(pool)
    .await?;

    debug!(count = rows.len(), "Got tools by domain");
    Ok(rows)
}

/// Get count of examples per domain.
///
/// Useful for verifying seeding worked correctly.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_domain_counts(pool: &PgPool) -> Result<Vec<DomainCount>, RepositoryError> {
    // Using runtime query to avoid SQLx offline mode cache requirements
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

/// Get total count of tool examples.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_total_count(pool: &PgPool) -> Result<i64, RepositoryError> {
    // Using runtime query to avoid SQLx offline mode cache requirements
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM admin.tool_example_queries")
        .fetch_one(pool)
        .await?;

    Ok(count.0)
}

/// Delete all tool examples (for re-seeding).
///
/// # Errors
///
/// Returns error if the database delete fails.
pub async fn delete_all(pool: &PgPool) -> Result<u64, RepositoryError> {
    // Using runtime query to avoid SQLx offline mode cache requirements
    let result = sqlx::query::<sqlx::Postgres>("DELETE FROM admin.tool_example_queries")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

/// Delete only pre-seeded examples (keep learned ones).
///
/// # Errors
///
/// Returns error if the database delete fails.
pub async fn delete_preseeded(pool: &PgPool) -> Result<u64, RepositoryError> {
    // Using runtime query to avoid SQLx offline mode cache requirements
    let result = sqlx::query::<sqlx::Postgres>(
        "DELETE FROM admin.tool_example_queries WHERE is_learned = FALSE",
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// A tool with its similarity score.
#[derive(Debug, Clone)]
pub struct SimilarTool {
    /// Tool name.
    pub tool_name: String,
    /// Similarity score (0.0 to 1.0).
    pub similarity: f64,
}

/// Internal row type for similarity query.
#[derive(sqlx::FromRow)]
struct SimilarToolRow {
    tool_name: String,
    similarity: Option<f64>,
}

/// Internal row type for domain count query.
#[derive(sqlx::FromRow)]
struct DomainCountRow {
    domain: String,
    count: Option<i64>,
}

/// Domain count for statistics.
#[derive(Debug)]
pub struct DomainCount {
    /// Domain name.
    pub domain: String,
    /// Number of examples.
    pub count: i64,
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

    #[test]
    fn test_format_embedding_single() {
        let embedding = vec![0.5];
        let result = format_embedding(&embedding);
        assert_eq!(result, "[0.5]");
    }
}
