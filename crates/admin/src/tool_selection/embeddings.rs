//! `OpenAI` embedding client for semantic similarity search.
//!
//! Uses `OpenAI`'s `text-embedding-3-small` model to generate 1536-dimensional
//! embeddings for user queries. These embeddings are compared against stored
//! example queries using pgvector's cosine similarity.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::ToolSelectionError;

const OPENAI_EMBEDDINGS_URL: &str = "https://api.openai.com/v1/embeddings";
const EMBEDDING_MODEL: &str = "text-embedding-3-small";
const EMBEDDING_DIMENSIONS: usize = 1536;

/// Client for generating text embeddings via `OpenAI` API.
#[derive(Clone)]
pub struct EmbeddingClient {
    client: reqwest::Client,
}

impl EmbeddingClient {
    /// Create a new embedding client.
    ///
    /// # Arguments
    ///
    /// * `api_key` - `OpenAI` API key
    ///
    /// # Panics
    ///
    /// Panics if the API key contains invalid header characters.
    #[must_use]
    pub fn new(api_key: &secrecy::SecretString) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key.expose_secret()))
                .expect("Invalid API key for header"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client");

        Self { client }
    }

    /// Generate an embedding vector for the given text.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to embed
    ///
    /// # Returns
    ///
    /// A 1536-dimensional embedding vector.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an invalid response.
    #[instrument(skip(self, text), fields(text_len = text.len()))]
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, ToolSelectionError> {
        let request = EmbeddingRequest {
            model: EMBEDDING_MODEL.to_string(),
            input: text.to_string(),
        };

        let response = self
            .client
            .post(OPENAI_EMBEDDINGS_URL)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ToolSelectionError::Embedding(format!(
                "OpenAI API error ({status}): {body}"
            )));
        }

        let response: EmbeddingResponse = response.json().await?;

        let embedding = response
            .data
            .into_iter()
            .next()
            .ok_or_else(|| {
                ToolSelectionError::InvalidResponse("No embedding data in response".to_string())
            })?
            .embedding;

        if embedding.len() != EMBEDDING_DIMENSIONS {
            return Err(ToolSelectionError::InvalidResponse(format!(
                "Expected {} dimensions, got {}",
                EMBEDDING_DIMENSIONS,
                embedding.len()
            )));
        }

        Ok(embedding)
    }

    /// Generate embeddings for multiple texts in a single request.
    ///
    /// # Arguments
    ///
    /// * `texts` - The texts to embed
    ///
    /// # Returns
    ///
    /// A vector of 1536-dimensional embedding vectors, one for each input text.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an invalid response.
    #[instrument(skip(self, texts), fields(count = texts.len()))]
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, ToolSelectionError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let request = BatchEmbeddingRequest {
            model: EMBEDDING_MODEL.to_string(),
            input: texts.iter().map(|s| (*s).to_string()).collect(),
        };

        let response = self
            .client
            .post(OPENAI_EMBEDDINGS_URL)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ToolSelectionError::Embedding(format!(
                "OpenAI API error ({status}): {body}"
            )));
        }

        let response: EmbeddingResponse = response.json().await?;

        let embeddings: Vec<Vec<f32>> = response.data.into_iter().map(|d| d.embedding).collect();

        if embeddings.len() != texts.len() {
            return Err(ToolSelectionError::InvalidResponse(format!(
                "Expected {} embeddings, got {}",
                texts.len(),
                embeddings.len()
            )));
        }

        for (i, emb) in embeddings.iter().enumerate() {
            if emb.len() != EMBEDDING_DIMENSIONS {
                return Err(ToolSelectionError::InvalidResponse(format!(
                    "Embedding {} has {} dimensions, expected {}",
                    i,
                    emb.len(),
                    EMBEDDING_DIMENSIONS
                )));
            }
        }

        Ok(embeddings)
    }
}

/// Request body for single text embedding.
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
}

/// Request body for batch text embedding.
#[derive(Debug, Serialize)]
struct BatchEmbeddingRequest {
    model: String,
    input: Vec<String>,
}

/// Response from `OpenAI` embeddings API.
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

/// Single embedding data in response.
#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_dimensions_constant() {
        assert_eq!(EMBEDDING_DIMENSIONS, 1536);
    }

    #[test]
    fn test_embedding_model_constant() {
        assert_eq!(EMBEDDING_MODEL, "text-embedding-3-small");
    }
}
