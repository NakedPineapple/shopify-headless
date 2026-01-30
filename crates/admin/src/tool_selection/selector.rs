//! Tool selector that orchestrates the three-stage selection process.
//!
//! 1. Classify query into domains using Haiku
//! 2. Search for similar example queries using embeddings
//! 3. Map example queries to unique tools

use sqlx::{FromRow, PgPool, Row};
use tracing::{debug, info, instrument};

use super::{DomainClassifier, EmbeddingClient, ToolSelectionError};

/// Default number of tools to select.
pub const DEFAULT_TOOL_LIMIT: usize = 10;

/// Minimum similarity score for example queries (0.0 to 1.0).
const MIN_SIMILARITY_SCORE: f64 = 0.5;

/// Tool selector that performs three-stage selection.
pub struct ToolSelector {
    classifier: DomainClassifier,
    embeddings: EmbeddingClient,
    pool: PgPool,
}

impl ToolSelector {
    /// Create a new tool selector.
    ///
    /// # Arguments
    ///
    /// * `classifier` - Domain classifier using Haiku
    /// * `embeddings` - `OpenAI` embedding client
    /// * `pool` - Database connection pool
    #[must_use]
    pub const fn new(
        classifier: DomainClassifier,
        embeddings: EmbeddingClient,
        pool: PgPool,
    ) -> Self {
        Self {
            classifier,
            embeddings,
            pool,
        }
    }

    /// Select tools relevant to the given query.
    ///
    /// Performs three-stage selection:
    /// 1. Classify query into 1-3 domains
    /// 2. Search for similar example queries filtered by domains
    /// 3. Return unique tools mapped from examples
    ///
    /// # Arguments
    ///
    /// * `query` - The user's query text
    /// * `limit` - Maximum number of tools to return (default 10)
    ///
    /// # Returns
    ///
    /// A vector of tools relevant to the query.
    ///
    /// # Errors
    ///
    /// Returns an error if any stage fails.
    #[instrument(skip(self, query), fields(query_len = query.len(), limit = limit))]
    pub async fn select_tools(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<SelectionResult, ToolSelectionError> {
        let limit = limit.unwrap_or(DEFAULT_TOOL_LIMIT);

        // Stage 1: Classify domains
        let domains = self.classifier.classify(query).await?;
        info!(domains = ?domains, "Stage 1: Classified into domains");

        // Stage 2: Generate embedding and search
        let embedding = self.embeddings.embed(query).await?;
        debug!("Stage 2: Generated embedding");

        // Search for similar examples filtered by domains
        let similar_tools = self
            .search_similar_tools(&embedding, &domains, limit)
            .await?;

        if similar_tools.is_empty() {
            info!("No similar tools found, falling back to domain-based selection");
            let fallback_tools = self.get_domain_tools(&domains, limit).await?;
            return Ok(SelectionResult {
                tools: fallback_tools,
                domains,
                used_fallback: true,
            });
        }

        info!(tool_count = similar_tools.len(), "Stage 3: Selected tools");

        Ok(SelectionResult {
            tools: similar_tools,
            domains,
            used_fallback: false,
        })
    }

    /// Search for tools similar to the query embedding.
    ///
    /// Uses pgvector's cosine distance operator (`<=>`) for similarity search.
    /// `SQLx` doesn't have built-in pgvector support, so we use runtime queries.
    async fn search_similar_tools(
        &self,
        embedding: &[f32],
        domains: &[String],
        limit: usize,
    ) -> Result<Vec<String>, ToolSelectionError> {
        // Convert embedding to pgvector format
        let embedding_str = format_embedding(embedding);

        // Query for similar examples, filtered by domain
        // Using runtime query since SQLx doesn't support pgvector's vector type
        let rows = sqlx::query(
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
        .bind(MIN_SIMILARITY_SCORE)
        .fetch_all(&self.pool)
        .await?;

        // Sort by similarity and limit
        let mut tools: Vec<(String, f64)> = rows
            .into_iter()
            .filter_map(|r| {
                let tool_name: String = r.get("tool_name");
                let similarity: Option<f64> = r.get("similarity");
                similarity.map(|s| (tool_name, s))
            })
            .collect();

        tools.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let tools: Vec<String> = tools.into_iter().take(limit).map(|(t, _)| t).collect();

        Ok(tools)
    }

    /// Get tools for domains when no similar examples are found.
    ///
    /// Falls back to selecting tools by usage count within the classified domains.
    async fn get_domain_tools(
        &self,
        domains: &[String],
        limit: usize,
    ) -> Result<Vec<String>, ToolSelectionError> {
        // Use subquery to get distinct tools ordered by max usage count
        let rows = sqlx::query(
            r"
            SELECT tool_name, MAX(usage_count) as max_usage
            FROM admin.tool_example_queries
            WHERE domain = ANY($1)
            GROUP BY tool_name
            ORDER BY max_usage DESC
            LIMIT $2
            ",
        )
        .bind(domains)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await?;

        let tools: Vec<String> = rows.into_iter().map(|r| r.get("tool_name")).collect();

        Ok(tools)
    }

    /// Record a successful tool use for learning.
    ///
    /// If the query led to successful tool execution, add it as a learned example.
    ///
    /// # Arguments
    ///
    /// * `query` - The original user query
    /// * `tool_name` - The tool that was successfully used
    /// * `domain` - The domain of the tool
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    #[instrument(skip(self, query))]
    pub async fn learn_from_success(
        &self,
        query: &str,
        tool_name: &str,
        domain: &str,
    ) -> Result<(), ToolSelectionError> {
        // Check if this exact query already exists
        let existing: Option<i32> = sqlx::query_scalar(
            r"
            SELECT id FROM admin.tool_example_queries
            WHERE tool_name = $1 AND example_query = $2
            LIMIT 1
            ",
        )
        .bind(tool_name)
        .bind(query)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(id) = existing {
            // Increment usage count
            sqlx::query(
                r"
                UPDATE admin.tool_example_queries
                SET usage_count = usage_count + 1
                WHERE id = $1
                ",
            )
            .bind(id)
            .execute(&self.pool)
            .await?;

            debug!(tool_name, "Incremented usage count for existing example");
        } else {
            // Generate embedding and add new learned example
            let embedding = self.embeddings.embed(query).await?;
            let embedding_str = format_embedding(&embedding);

            sqlx::query(
                r"
                INSERT INTO admin.tool_example_queries
                    (tool_name, domain, example_query, embedding, is_learned, usage_count)
                VALUES ($1, $2, $3, $4::vector, TRUE, 1)
                ",
            )
            .bind(tool_name)
            .bind(domain)
            .bind(query)
            .bind(&embedding_str)
            .execute(&self.pool)
            .await?;

            info!(tool_name, domain, "Added new learned example");
        }

        Ok(())
    }
}

/// Result of tool selection.
#[derive(Debug)]
pub struct SelectionResult {
    /// Selected tool names.
    pub tools: Vec<String>,
    /// Domains that were classified.
    pub domains: Vec<String>,
    /// Whether fallback selection was used (no similar examples found).
    pub used_fallback: bool,
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
