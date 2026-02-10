//! Cloudflare R2 client for document storage.
//!
//! R2 is S3-compatible, so this uses the AWS SDK with a custom endpoint.

use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use thiserror::Error;
use tracing::{debug, instrument};

use secrecy::{ExposeSecret, SecretString};

/// Errors that can occur during R2 operations.
#[derive(Debug, Error)]
pub enum R2Error {
    /// Upload failed.
    #[error("R2 upload failed: {0}")]
    Upload(String),

    /// Download failed.
    #[error("R2 download failed: {0}")]
    Download(String),

    /// Delete failed.
    #[error("R2 delete failed: {0}")]
    Delete(String),

    /// Byte stream collection failed.
    #[error("byte stream error: {0}")]
    ByteStream(String),
}

/// Cloudflare R2 client (S3-compatible).
#[derive(Clone)]
pub struct R2Client {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl R2Client {
    /// Create a new R2 client.
    #[must_use]
    pub fn new(
        account_id: &str,
        access_key_id: &str,
        secret_access_key: &SecretString,
        bucket: String,
    ) -> Self {
        let endpoint = format!("https://{account_id}.r2.cloudflarestorage.com");

        let creds = aws_sdk_s3::config::Credentials::new(
            access_key_id,
            secret_access_key.expose_secret(),
            None,
            None,
            "r2",
        );

        let config = aws_sdk_s3::Config::builder()
            .endpoint_url(&endpoint)
            .region(aws_sdk_s3::config::Region::new("auto"))
            .credentials_provider(creds)
            .behavior_version_latest()
            .build();

        let client = aws_sdk_s3::Client::from_conf(config);

        Self { client, bucket }
    }

    /// Upload bytes to R2.
    ///
    /// # Errors
    ///
    /// Returns error if the upload fails.
    #[instrument(skip(self, data), fields(key = %key, size = data.len()))]
    pub async fn put_object(
        &self,
        key: &str,
        data: Bytes,
        content_type: &str,
    ) -> Result<(), R2Error> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| R2Error::Upload(e.to_string()))?;

        debug!("Uploaded to R2");
        Ok(())
    }

    /// Download bytes from R2.
    ///
    /// # Errors
    ///
    /// Returns error if the download fails or object is not found.
    #[instrument(skip(self), fields(key = %key))]
    pub async fn get_object(&self, key: &str) -> Result<Bytes, R2Error> {
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| R2Error::Download(e.to_string()))?;

        let bytes = output
            .body
            .collect()
            .await
            .map_err(|e| R2Error::ByteStream(e.to_string()))?
            .into_bytes();

        debug!(size = bytes.len(), "Downloaded from R2");
        Ok(bytes)
    }

    /// Delete object from R2.
    ///
    /// # Errors
    ///
    /// Returns error if the deletion fails.
    #[instrument(skip(self), fields(key = %key))]
    pub async fn delete_object(&self, key: &str) -> Result<(), R2Error> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| R2Error::Delete(e.to_string()))?;

        debug!("Deleted from R2");
        Ok(())
    }
}
