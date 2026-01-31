//! Klaviyo API client for subscription management.
//!
//! Provides functionality for managing newsletter subscriptions,
//! including subscribing users to email lists and unsubscribing from
//! email and SMS lists.

use reqwest::header::{HeaderMap, HeaderValue};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::KlaviyoConfig;

/// Klaviyo API version.
const API_REVISION: &str = "2024-10-15";

/// Klaviyo API base URL.
const BASE_URL: &str = "https://a.klaviyo.com/api";

/// Errors that can occur when interacting with Klaviyo API.
#[derive(Debug, Error)]
pub enum KlaviyoError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// API returned an error response.
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Profile not found.
    #[error("Profile not found for email: {0}")]
    ProfileNotFound(String),

    /// Failed to parse response.
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Klaviyo API client for subscription management.
#[derive(Clone)]
pub struct KlaviyoClient {
    client: reqwest::Client,
    list_id: String,
}

impl KlaviyoClient {
    /// Create a new Klaviyo API client.
    ///
    /// # Errors
    ///
    /// Returns error if the HTTP client fails to build.
    pub fn new(config: &KlaviyoConfig) -> Result<Self, KlaviyoError> {
        let mut headers = HeaderMap::new();

        // Authorization header
        let auth_value = format!("Klaviyo-API-Key {}", config.api_key.expose_secret());
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&auth_value)
                .map_err(|e| KlaviyoError::Parse(format!("Invalid API key format: {e}")))?,
        );

        // Revision header for API versioning
        headers.insert("revision", HeaderValue::from_static(API_REVISION));

        // Content-Type for JSON:API
        headers.insert(
            "Content-Type",
            HeaderValue::from_static("application/vnd.api+json"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            list_id: config.list_id.clone(),
        })
    }

    /// Find a profile by email address.
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails or profile is not found.
    pub async fn find_profile_by_email(&self, email: &str) -> Result<Profile, KlaviyoError> {
        let url = format!(
            "{BASE_URL}/profiles?filter=equals(email,\"{}\")",
            urlencoding::encode(email)
        );

        let response = self.client.get(&url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(KlaviyoError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let api_response: ApiListResponse<Profile> = response
            .json()
            .await
            .map_err(|e| KlaviyoError::Parse(e.to_string()))?;

        api_response
            .data
            .into_iter()
            .next()
            .ok_or_else(|| KlaviyoError::ProfileNotFound(email.to_string()))
    }

    /// Subscribe an email to the newsletter list.
    ///
    /// Creates or updates a profile and subscribes them to the configured list.
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails.
    pub async fn subscribe_email(&self, email: &str) -> Result<(), KlaviyoError> {
        let url = format!("{BASE_URL}/profile-subscription-bulk-create-jobs");

        let body = serde_json::json!({
            "data": {
                "type": "profile-subscription-bulk-create-job",
                "attributes": {
                    "custom_source": "Naked Pineapple Website",
                    "profiles": {
                        "data": [{
                            "type": "profile",
                            "attributes": {
                                "email": email,
                                "subscriptions": {
                                    "email": {
                                        "marketing": {
                                            "consent": "SUBSCRIBED"
                                        }
                                    }
                                }
                            }
                        }]
                    }
                },
                "relationships": {
                    "list": {
                        "data": {
                            "type": "list",
                            "id": self.list_id
                        }
                    }
                }
            }
        });

        let response = self.client.post(&url).json(&body).send().await?;
        let status = response.status();

        // 202 Accepted is the expected response for bulk jobs
        if !status.is_success() && status.as_u16() != 202 {
            let message = response.text().await.unwrap_or_default();
            return Err(KlaviyoError::Api {
                status: status.as_u16(),
                message,
            });
        }

        Ok(())
    }

    /// Unsubscribe a profile from email marketing.
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails.
    pub async fn unsubscribe_email(&self, profile_id: &str) -> Result<(), KlaviyoError> {
        self.suppress_profile(profile_id, "email").await
    }

    /// Unsubscribe a profile from SMS marketing.
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails.
    pub async fn unsubscribe_sms(&self, profile_id: &str) -> Result<(), KlaviyoError> {
        self.suppress_profile(profile_id, "sms").await
    }

    /// Suppress a profile from a specific channel.
    async fn suppress_profile(&self, profile_id: &str, channel: &str) -> Result<(), KlaviyoError> {
        let url = format!("{BASE_URL}/profile-suppression-bulk-create-jobs");

        let body = serde_json::json!({
            "data": {
                "type": "profile-suppression-bulk-create-job",
                "attributes": {
                    "profiles": {
                        "data": [{
                            "type": "profile",
                            "id": profile_id
                        }]
                    }
                },
                "relationships": {
                    "list": {
                        "data": {
                            "type": "list",
                            "id": self.list_id
                        }
                    }
                }
            }
        });

        let response = self.client.post(&url).json(&body).send().await?;
        let status = response.status();

        if !status.is_success() && status.as_u16() != 202 {
            let message = response.text().await.unwrap_or_default();
            return Err(KlaviyoError::Api {
                status: status.as_u16(),
                message,
            });
        }

        // Also update profile subscription consent directly for immediate effect
        self.update_subscription_consent(profile_id, channel, "UNSUBSCRIBED")
            .await
    }

    /// Update a profile's subscription consent for a channel.
    async fn update_subscription_consent(
        &self,
        profile_id: &str,
        channel: &str,
        consent: &str,
    ) -> Result<(), KlaviyoError> {
        let url = format!("{BASE_URL}/profiles/{profile_id}");

        let subscriptions = if channel == "email" {
            serde_json::json!({
                "email": {
                    "marketing": {
                        "consent": consent
                    }
                }
            })
        } else {
            serde_json::json!({
                "sms": {
                    "marketing": {
                        "consent": consent
                    }
                }
            })
        };

        let body = serde_json::json!({
            "data": {
                "type": "profile",
                "id": profile_id,
                "attributes": {
                    "subscriptions": subscriptions
                }
            }
        });

        let response = self.client.patch(&url).json(&body).send().await?;
        let status = response.status();

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(KlaviyoError::Api {
                status: status.as_u16(),
                message,
            });
        }

        Ok(())
    }
}

/// Wrapper for JSON:API list response.
#[derive(Debug, Deserialize)]
struct ApiListResponse<T> {
    data: Vec<T>,
}

/// Profile resource from Klaviyo API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub attributes: ProfileAttributes,
}

/// Profile attributes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileAttributes {
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}
