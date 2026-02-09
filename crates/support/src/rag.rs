//! RAG (Retrieval-Augmented Generation) context retrieval.
//!
//! Embeds a user query and searches the knowledge base for relevant content.
//! Results are injected into the system prompt before each Claude API call.

use naked_pineapple_services::openai::EmbeddingClient;
use sqlx::PgPool;
use tracing::warn;

use crate::db::knowledge::KnowledgeRepository;
use crate::models::KnowledgeSearchResult;

/// Number of knowledge entries to retrieve per query.
const RAG_TOP_K: i64 = 5;

/// Minimum cosine similarity threshold for including a result.
const SIMILARITY_THRESHOLD: f64 = 0.3;

/// Retrieve relevant knowledge context for a user query.
///
/// 1. Embeds the query via the `OpenAI` embedding API
/// 2. Searches `storefront.support_knowledge` by vector similarity
/// 3. Filters results below the similarity threshold
/// 4. Formats matching entries for system prompt injection
///
/// Returns an empty string if embedding fails, no results are found, or all
/// results fall below the threshold. Errors are logged but not propagated —
/// chat degrades gracefully without RAG context.
pub async fn retrieve_context(
    embedding_client: &EmbeddingClient,
    pool: &PgPool,
    query: &str,
) -> String {
    let embedding = match embedding_client.embed(query).await {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "Failed to embed query for RAG");
            return String::new();
        }
    };

    let results = match KnowledgeRepository::new(pool)
        .search_by_embedding(&embedding, RAG_TOP_K)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "Failed to search knowledge base");
            return String::new();
        }
    };

    format_results(&results)
}

/// Format knowledge search results as markdown sections for a system prompt.
fn format_results(results: &[KnowledgeSearchResult]) -> String {
    results
        .iter()
        .filter(|r| r.similarity > SIMILARITY_THRESHOLD)
        .map(|r| {
            format!(
                "### {} ({})\n{}\n",
                r.entry.title, r.entry.category, r.entry.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
