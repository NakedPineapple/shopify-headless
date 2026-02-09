//! Mail operations via the Microsoft Graph API.
//!
//! Provides methods for reading, sending, replying to, and archiving
//! emails in shared mailboxes.

use super::error::M365Error;
use super::types::{
    GraphErrorResponse, GraphMessage, MessageListResponse, MoveRequest, OutboundBody, ReplyBody,
    ReplyRequest,
};
use crate::microsoft_graph::M365Client;

/// Well-known folder name for the Archive folder in Microsoft 365.
const ARCHIVE_FOLDER: &str = "archive";

/// Maximum messages per page when listing unread emails.
const MAX_PAGE_SIZE: u32 = 50;

impl M365Client {
    /// List unread messages in a shared mailbox.
    ///
    /// Returns up to `MAX_PAGE_SIZE` unread messages, ordered by most recent first.
    ///
    /// # Errors
    ///
    /// Returns `M365Error` if the request fails or the response cannot be parsed.
    pub async fn list_unread(&self, mailbox: &str) -> Result<Vec<GraphMessage>, M365Error> {
        let url = format!(
            "https://graph.microsoft.com/v1.0/users/{mailbox}/messages\
             ?$filter=isRead%20eq%20false\
             &$orderby=receivedDateTime%20desc\
             &$top={MAX_PAGE_SIZE}\
             &$select=id,conversationId,subject,bodyPreview,body,from,toRecipients,receivedDateTime,isRead"
        );

        let response: MessageListResponse = self.graph_get(&url).await?;
        Ok(response.value)
    }

    /// Mark a message as read.
    ///
    /// # Errors
    ///
    /// Returns `M365Error` if the request fails.
    pub async fn mark_read(&self, mailbox: &str, message_id: &str) -> Result<(), M365Error> {
        let url = format!("https://graph.microsoft.com/v1.0/users/{mailbox}/messages/{message_id}");

        let body = serde_json::json!({ "isRead": true });
        self.graph_patch(&url, &body).await
    }

    /// Reply to a message.
    ///
    /// # Errors
    ///
    /// Returns `M365Error` if the request fails.
    pub async fn reply(
        &self,
        mailbox: &str,
        message_id: &str,
        html_body: &str,
    ) -> Result<(), M365Error> {
        let url =
            format!("https://graph.microsoft.com/v1.0/users/{mailbox}/messages/{message_id}/reply");

        let body = ReplyRequest {
            message: ReplyBody {
                body: OutboundBody {
                    content_type: "HTML".to_string(),
                    content: html_body.to_string(),
                },
                to_recipients: None,
            },
        };

        self.graph_post(&url, &body).await
    }

    /// Move a message to the Archive folder.
    ///
    /// # Errors
    ///
    /// Returns `M365Error` if the request fails.
    pub async fn archive(&self, mailbox: &str, message_id: &str) -> Result<(), M365Error> {
        let url =
            format!("https://graph.microsoft.com/v1.0/users/{mailbox}/messages/{message_id}/move");

        let body = MoveRequest {
            destination_id: ARCHIVE_FOLDER.to_string(),
        };

        self.graph_post(&url, &body).await
    }

    /// Make a GET request to the Graph API and deserialize the response.
    async fn graph_get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, M365Error> {
        let token = self.auth.get_token().await?;

        let response = self.http.get(url).bearer_auth(&token).send().await?;

        handle_response(response).await
    }

    /// Make a PATCH request to the Graph API.
    async fn graph_patch(
        &self,
        url: &str,
        body: &(impl serde::Serialize + Sync),
    ) -> Result<(), M365Error> {
        let token = self.auth.get_token().await?;

        let response = self
            .http
            .patch(url)
            .bearer_auth(&token)
            .json(body)
            .send()
            .await?;

        handle_empty_response(response).await
    }

    /// Make a POST request to the Graph API.
    async fn graph_post(
        &self,
        url: &str,
        body: &(impl serde::Serialize + Sync),
    ) -> Result<(), M365Error> {
        let token = self.auth.get_token().await?;

        let response = self
            .http
            .post(url)
            .bearer_auth(&token)
            .json(body)
            .send()
            .await?;

        handle_empty_response(response).await
    }
}

/// Handle a Graph API response that returns a JSON body.
async fn handle_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, M365Error> {
    let status = response.status();
    if !status.is_success() {
        return Err(parse_error_response(response).await);
    }

    response
        .json()
        .await
        .map_err(|e| M365Error::Parse(e.to_string()))
}

/// Handle a Graph API response that returns no body (or we ignore the body).
async fn handle_empty_response(response: reqwest::Response) -> Result<(), M365Error> {
    let status = response.status();
    if !status.is_success() {
        return Err(parse_error_response(response).await);
    }
    Ok(())
}

/// Parse a Graph API error response into an `M365Error`.
async fn parse_error_response(response: reqwest::Response) -> M365Error {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();

    if let Ok(error_response) = serde_json::from_str::<GraphErrorResponse>(&body) {
        M365Error::Api {
            status,
            message: error_response.error.message,
            error_code: error_response.error.code,
        }
    } else {
        M365Error::Api {
            status,
            message: body,
            error_code: None,
        }
    }
}
