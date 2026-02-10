//! Database operations for uploaded documents and embedding chunks.

use sqlx::PgPool;
use tracing::{debug, instrument};

use super::RepositoryError;

/// Document metadata row from the database.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DocumentRow {
    pub id: i32,
    pub filename: String,
    pub content_type: String,
    pub file_size: i64,
    pub r2_key: String,
    pub uploaded_by: i32,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Result from a document chunk similarity search.
#[derive(Debug, Clone)]
pub struct ChunkSearchResult {
    pub chunk_text: String,
    pub chunk_index: i32,
    pub document_id: i32,
    pub filename: String,
    pub description: Option<String>,
    pub similarity: f64,
}

/// Row type for similarity search query.
#[derive(sqlx::FromRow)]
struct ChunkSearchRow {
    chunk_text: String,
    chunk_index: i32,
    document_id: i32,
    filename: String,
    description: Option<String>,
    similarity: Option<f64>,
}

/// List all documents ordered by creation date (newest first).
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool))]
pub async fn list_documents(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<DocumentRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        DocumentRow,
        r#"
        SELECT id, filename, content_type, file_size, r2_key, uploaded_by,
               description,
               created_at AS "created_at: chrono::DateTime<chrono::Utc>",
               updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
        FROM admin.documents
        ORDER BY created_at DESC
        LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await?;

    debug!(count = rows.len(), "Listed documents");
    Ok(rows)
}

/// Get a document by ID.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_document(pool: &PgPool, id: i32) -> Result<Option<DocumentRow>, RepositoryError> {
    let doc = sqlx::query_as!(
        DocumentRow,
        r#"
        SELECT id, filename, content_type, file_size, r2_key, uploaded_by,
               description,
               created_at AS "created_at: chrono::DateTime<chrono::Utc>",
               updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
        FROM admin.documents
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(doc)
}

/// Get the number of chunks for a document.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_chunk_count(pool: &PgPool, document_id: i32) -> Result<i64, RepositoryError> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM admin.document_chunks WHERE document_id = $1"#,
        document_id
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}

/// Delete a document (chunks cascade-delete).
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool))]
pub async fn delete_document(pool: &PgPool, id: i32) -> Result<(), RepositoryError> {
    sqlx::query!("DELETE FROM admin.documents WHERE id = $1", id)
        .execute(pool)
        .await?;

    debug!(document_id = id, "Deleted document");
    Ok(())
}

/// Search document chunks by embedding similarity.
///
/// Uses pgvector cosine similarity to find the most relevant chunks.
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool, embedding), fields(min_similarity, limit))]
pub async fn search_chunks(
    pool: &PgPool,
    embedding: &[f32],
    min_similarity: f64,
    limit: i64,
) -> Result<Vec<ChunkSearchResult>, RepositoryError> {
    let embedding_str = format_embedding(embedding);

    // Runtime query because SQLx doesn't natively support pgvector types
    let rows = sqlx::query_as::<_, ChunkSearchRow>(
        r"
        SELECT c.chunk_text, c.chunk_index, c.document_id,
               d.filename, d.description,
               1 - (c.embedding <=> $1::vector) AS similarity
        FROM admin.document_chunks c
        JOIN admin.documents d ON d.id = c.document_id
        WHERE 1 - (c.embedding <=> $1::vector) > $2
        ORDER BY c.embedding <=> $1::vector
        LIMIT $3
        ",
    )
    .bind(&embedding_str)
    .bind(min_similarity)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let results: Vec<ChunkSearchResult> = rows
        .into_iter()
        .filter_map(|r| {
            r.similarity.map(|s| ChunkSearchResult {
                chunk_text: r.chunk_text,
                chunk_index: r.chunk_index,
                document_id: r.document_id,
                filename: r.filename,
                description: r.description,
                similarity: s,
            })
        })
        .collect();

    debug!(count = results.len(), "Found similar document chunks");
    Ok(results)
}

/// Format an embedding vector as a pgvector literal string.
fn format_embedding(embedding: &[f32]) -> String {
    let values: Vec<String> = embedding.iter().map(ToString::to_string).collect();
    format!("[{}]", values.join(","))
}
