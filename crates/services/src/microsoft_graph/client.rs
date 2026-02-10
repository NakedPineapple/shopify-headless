//! Microsoft Graph API client.

use super::auth::TokenManager;
use crate::config::M365Config;

/// Client for interacting with the Microsoft Graph API.
///
/// Handles authentication (`OAuth2` client credentials flow) and provides
/// methods for mail operations on shared mailboxes.
pub struct M365Client {
    pub(super) http: reqwest::Client,
    pub(super) auth: TokenManager,
    pub(super) mailboxes: Vec<String>,
}

impl M365Client {
    /// Create a new Microsoft Graph client.
    ///
    /// The client authenticates lazily — the first API call will trigger
    /// token acquisition.
    #[must_use]
    pub fn new(config: &M365Config) -> Self {
        let http = reqwest::Client::new();
        let auth = TokenManager::new(config, &http);

        Self {
            http,
            auth,
            mailboxes: config.shared_mailboxes.clone(),
        }
    }

    /// Returns the list of shared mailbox addresses this client is configured to poll.
    #[must_use]
    pub fn mailboxes(&self) -> &[String] {
        &self.mailboxes
    }
}
