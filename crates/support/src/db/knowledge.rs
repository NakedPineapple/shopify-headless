//! CRUD + vector search for `storefront.support_knowledge`.

use chrono::{DateTime, Utc};
use naked_pineapple_core::SupportKnowledgeId;
use sqlx::PgPool;

use crate::error::SupportError;
use crate::models::{
    CreateKnowledgeParams, KnowledgeSearchResult, SupportKnowledge, UpdateKnowledgeParams,
};

#[derive(Debug, sqlx::FromRow)]
struct KnowledgeRow {
    id: i32,
    title: String,
    content: String,
    category: String,
    is_active: bool,
    created_by: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<KnowledgeRow> for SupportKnowledge {
    fn from(row: KnowledgeRow) -> Self {
        Self {
            id: SupportKnowledgeId::new(row.id),
            title: row.title,
            content: row.content,
            category: row.category,
            is_active: row.is_active,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct KnowledgeSearchRow {
    id: i32,
    title: String,
    content: String,
    category: String,
    is_active: bool,
    created_by: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    similarity: Option<f64>,
}

/// Repository for support knowledge base operations.
pub struct KnowledgeRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> KnowledgeRepository<'a> {
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create a new knowledge entry.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn create(
        &self,
        params: &CreateKnowledgeParams,
    ) -> Result<SupportKnowledge, SupportError> {
        let embedding_str = format_embedding(&params.embedding);

        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            KnowledgeRow,
            r#"
            INSERT INTO storefront.support_knowledge
                (title, content, category, embedding, created_by)
            VALUES ($1, $2, $3, $4::text::vector, $5)
            RETURNING id, title, content, category, is_active, created_by,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            params.title,
            params.content,
            params.category,
            embedding_str,
            params.created_by,
        )
        .fetch_one(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, KnowledgeRow>(
            "INSERT INTO storefront.support_knowledge
                (title, content, category, embedding, created_by)
            VALUES ($1, $2, $3, $4::text::vector, $5)
            RETURNING id, title, content, category, is_active, created_by,
                      created_at, updated_at",
        )
        .bind(&params.title)
        .bind(&params.content)
        .bind(&params.category)
        .bind(&embedding_str)
        .bind(params.created_by)
        .fetch_one(self.pool)
        .await?;

        Ok(row.into())
    }

    /// Update a knowledge entry (re-generates embedding).
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn update(
        &self,
        id: SupportKnowledgeId,
        params: &UpdateKnowledgeParams,
    ) -> Result<SupportKnowledge, SupportError> {
        let embedding_str = format_embedding(&params.embedding);

        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            KnowledgeRow,
            r#"
            UPDATE storefront.support_knowledge
            SET title = $2, content = $3, category = $4, embedding = $5::text::vector
            WHERE id = $1
            RETURNING id, title, content, category, is_active, created_by,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            id.as_i32(),
            params.title,
            params.content,
            params.category,
            embedding_str,
        )
        .fetch_one(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, KnowledgeRow>(
            "UPDATE storefront.support_knowledge
            SET title = $2, content = $3, category = $4, embedding = $5::text::vector
            WHERE id = $1
            RETURNING id, title, content, category, is_active, created_by,
                      created_at, updated_at",
        )
        .bind(id.as_i32())
        .bind(&params.title)
        .bind(&params.content)
        .bind(&params.category)
        .bind(&embedding_str)
        .fetch_one(self.pool)
        .await?;

        Ok(row.into())
    }

    /// Get a knowledge entry by ID.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn get_by_id(
        &self,
        id: SupportKnowledgeId,
    ) -> Result<SupportKnowledge, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            KnowledgeRow,
            r#"
            SELECT id, title, content, category, is_active, created_by,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM storefront.support_knowledge
            WHERE id = $1
            "#,
            id.as_i32(),
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(SupportError::ConversationNotFound)?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, KnowledgeRow>(
            "SELECT id, title, content, category, is_active, created_by,
                   created_at, updated_at
            FROM storefront.support_knowledge
            WHERE id = $1",
        )
        .bind(id.as_i32())
        .fetch_optional(self.pool)
        .await?
        .ok_or(SupportError::ConversationNotFound)?;

        Ok(row.into())
    }

    /// Toggle active status.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn toggle_active(
        &self,
        id: SupportKnowledgeId,
        active: bool,
    ) -> Result<(), SupportError> {
        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_knowledge
            SET is_active = $2
            WHERE id = $1
            "#,
            id.as_i32(),
            active,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            "UPDATE storefront.support_knowledge
            SET is_active = $2
            WHERE id = $1",
        )
        .bind(id.as_i32())
        .bind(active)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Delete a knowledge entry.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn delete(&self, id: SupportKnowledgeId) -> Result<(), SupportError> {
        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            DELETE FROM storefront.support_knowledge
            WHERE id = $1
            "#,
            id.as_i32(),
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            "DELETE FROM storefront.support_knowledge
            WHERE id = $1",
        )
        .bind(id.as_i32())
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// List knowledge entries with optional category filter.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn list(
        &self,
        category_filter: Option<&str>,
        active_only: bool,
    ) -> Result<Vec<SupportKnowledge>, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let rows = sqlx::query_as!(
            KnowledgeRow,
            r#"
            SELECT id, title, content, category, is_active, created_by,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM storefront.support_knowledge
            WHERE ($1::text IS NULL OR category = $1)
              AND ($2 = FALSE OR is_active = TRUE)
            ORDER BY category, title
            "#,
            category_filter,
            active_only,
        )
        .fetch_all(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let rows = sqlx::query_as::<_, KnowledgeRow>(
            "SELECT id, title, content, category, is_active, created_by,
                   created_at, updated_at
            FROM storefront.support_knowledge
            WHERE ($1::text IS NULL OR category = $1)
              AND ($2 = FALSE OR is_active = TRUE)
            ORDER BY category, title",
        )
        .bind(category_filter)
        .bind(active_only)
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Search knowledge base by vector similarity (for RAG).
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn search_by_embedding(
        &self,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<KnowledgeSearchResult>, SupportError> {
        let embedding_str = format_embedding(embedding);

        #[cfg(feature = "sqlx-macros")]
        let rows = sqlx::query_as!(
            KnowledgeSearchRow,
            r#"
            SELECT
                id, title, content, category, is_active, created_by,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                1 - (embedding <=> $1::text::vector) as similarity
            FROM storefront.support_knowledge
            WHERE is_active = TRUE
            ORDER BY embedding <=> $1::text::vector
            LIMIT $2
            "#,
            embedding_str,
            limit,
        )
        .fetch_all(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let rows = sqlx::query_as::<_, KnowledgeSearchRow>(
            "SELECT
                id, title, content, category, is_active, created_by,
                created_at, updated_at,
                1 - (embedding <=> $1::text::vector) as similarity
            FROM storefront.support_knowledge
            WHERE is_active = TRUE
            ORDER BY embedding <=> $1::text::vector
            LIMIT $2",
        )
        .bind(&embedding_str)
        .bind(limit)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| KnowledgeSearchResult {
                similarity: row.similarity.unwrap_or(0.0),
                entry: SupportKnowledge {
                    id: SupportKnowledgeId::new(row.id),
                    title: row.title,
                    content: row.content,
                    category: row.category,
                    is_active: row.is_active,
                    created_by: row.created_by,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
            })
            .collect())
    }
}

/// Format an embedding vector as a `PostgreSQL` vector literal string.
fn format_embedding(embedding: &[f32]) -> String {
    let values: Vec<String> = embedding.iter().map(ToString::to_string).collect();
    format!("[{}]", values.join(","))
}
