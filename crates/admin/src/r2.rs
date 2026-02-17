//! Cloudflare R2 client for document and image storage.
//!
//! R2 is S3-compatible, so this uses the AWS SDK with a custom endpoint.

use std::collections::HashMap;

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{Delete, ObjectIdentifier};
use bytes::Bytes;
use chrono::{DateTime, Utc};
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

    /// Head request failed.
    #[error("R2 head failed: {0}")]
    Head(String),

    /// List request failed.
    #[error("R2 list failed: {0}")]
    List(String),

    /// Copy request failed.
    #[error("R2 copy failed: {0}")]
    Copy(String),

    /// Byte stream collection failed.
    #[error("byte stream error: {0}")]
    ByteStream(String),
}

/// An entry returned from listing objects in R2.
pub struct R2ListEntry {
    pub key: String,
    pub size: i64,
    pub last_modified: DateTime<Utc>,
}

/// Object metadata returned from a HEAD request.
pub struct R2ObjectMeta {
    pub content_type: String,
    pub content_length: i64,
    pub last_modified: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
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
            .map_err(|e| R2Error::Upload(format!("{e:?}")))?;

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
            .map_err(|e| R2Error::Download(format!("{e:?}")))?;

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
            .map_err(|e| R2Error::Delete(format!("{e:?}")))?;

        debug!("Deleted from R2");
        Ok(())
    }

    /// List objects in R2 under a given prefix.
    ///
    /// Returns both files and common prefixes (subfolders) when a delimiter
    /// is provided. Handles pagination internally.
    ///
    /// # Errors
    ///
    /// Returns error if the list request fails.
    #[instrument(skip(self), fields(prefix = %prefix))]
    pub async fn list_objects(
        &self,
        prefix: &str,
        delimiter: &str,
    ) -> Result<(Vec<R2ListEntry>, Vec<String>), R2Error> {
        let mut entries = Vec::new();
        let mut prefixes = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .delimiter(delimiter);

            if let Some(token) = &continuation_token {
                req = req.continuation_token(token);
            }

            let output = req
                .send()
                .await
                .map_err(|e| R2Error::List(format!("{e:?}")))?;

            if let Some(contents) = output.contents {
                for obj in contents {
                    let key = obj.key.unwrap_or_default();
                    if key == prefix {
                        continue; // skip the prefix itself (folder marker)
                    }
                    let size = obj.size.unwrap_or(0);
                    let last_modified = obj
                        .last_modified
                        .and_then(|dt| DateTime::from_timestamp(dt.secs(), dt.subsec_nanos()))
                        .unwrap_or_else(Utc::now);
                    entries.push(R2ListEntry {
                        key,
                        size,
                        last_modified,
                    });
                }
            }

            if let Some(common_prefixes) = output.common_prefixes {
                for cp in common_prefixes {
                    if let Some(p) = cp.prefix {
                        prefixes.push(p);
                    }
                }
            }

            if output.is_truncated == Some(true) {
                continuation_token = output.next_continuation_token;
            } else {
                break;
            }
        }

        debug!(
            files = entries.len(),
            folders = prefixes.len(),
            "Listed R2 objects"
        );
        Ok((entries, prefixes))
    }

    /// List only the subdirectories (common prefixes) under a given prefix.
    ///
    /// Convenience wrapper around [`list_objects`] that discards file entries.
    ///
    /// # Errors
    ///
    /// Returns error if the list request fails.
    #[instrument(skip(self), fields(prefix = %prefix))]
    pub async fn list_common_prefixes(
        &self,
        prefix: &str,
        delimiter: &str,
    ) -> Result<Vec<String>, R2Error> {
        let (_, prefixes) = self.list_objects(prefix, delimiter).await?;
        Ok(prefixes)
    }

    /// Get object metadata without downloading the body.
    ///
    /// # Errors
    ///
    /// Returns error if the head request fails.
    #[instrument(skip(self), fields(key = %key))]
    pub async fn head_object(&self, key: &str) -> Result<R2ObjectMeta, R2Error> {
        let output = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| R2Error::Head(format!("{e:?}")))?;

        let content_type = output
            .content_type
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let content_length = output.content_length.unwrap_or(0);
        let last_modified = output
            .last_modified
            .and_then(|dt| DateTime::from_timestamp(dt.secs(), dt.subsec_nanos()))
            .unwrap_or_else(Utc::now);
        let metadata = output.metadata.unwrap_or_default();

        debug!("Head object from R2");
        Ok(R2ObjectMeta {
            content_type,
            content_length,
            last_modified,
            metadata,
        })
    }

    /// Bulk delete objects from R2 (up to 1000 keys per call).
    ///
    /// # Errors
    ///
    /// Returns error if the delete request fails.
    #[instrument(skip(self), fields(count = keys.len()))]
    pub async fn delete_objects(&self, keys: &[String]) -> Result<(), R2Error> {
        if keys.is_empty() {
            return Ok(());
        }

        for chunk in keys.chunks(1000) {
            let objects: Vec<ObjectIdentifier> = chunk
                .iter()
                .filter_map(|k| ObjectIdentifier::builder().key(k).build().ok())
                .collect();

            let delete = Delete::builder()
                .set_objects(Some(objects))
                .build()
                .map_err(|e| R2Error::Delete(format!("{e:?}")))?;

            self.client
                .delete_objects()
                .bucket(&self.bucket)
                .delete(delete)
                .send()
                .await
                .map_err(|e| R2Error::Delete(format!("{e:?}")))?;
        }

        debug!("Bulk deleted from R2");
        Ok(())
    }

    /// Server-side copy within the same bucket.
    ///
    /// # Errors
    ///
    /// Returns error if the copy request fails.
    #[instrument(skip(self), fields(source = %source_key, dest = %dest_key))]
    pub async fn copy_object(&self, source_key: &str, dest_key: &str) -> Result<(), R2Error> {
        let copy_source = format!("{}/{source_key}", self.bucket);

        self.client
            .copy_object()
            .bucket(&self.bucket)
            .copy_source(&copy_source)
            .key(dest_key)
            .send()
            .await
            .map_err(|e| R2Error::Copy(format!("{e:?}")))?;

        debug!("Copied object in R2");
        Ok(())
    }
}
