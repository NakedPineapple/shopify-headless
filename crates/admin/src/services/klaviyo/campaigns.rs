//! Campaign management operations for Klaviyo API.

use std::fmt::Write;

use tracing::instrument;

use super::{
    ApiListResponse, ApiResponse, Campaign, CampaignChannel, CampaignSendJob, CampaignStatus,
    CreateCampaignAttributes, CreateCampaignData, CreateCampaignInput,
    CreateCampaignMessageAttributes, CreateCampaignMessageData, CreateCampaignMessages,
    KlaviyoClient, KlaviyoError, List, Profile, SendCampaignData, SendCampaignInput,
    SubscriberStats, UpdateCampaignAttributes, UpdateCampaignData, UpdateCampaignInput,
};

impl KlaviyoClient {
    /// List all campaigns with optional filters.
    ///
    /// Returns campaigns sorted by creation date (newest first).
    ///
    /// # Arguments
    ///
    /// * `status` - Optional status filter (Draft, Sent, etc.)
    /// * `channel` - Optional channel filter (Email, Sms)
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails.
    #[instrument(skip(self))]
    pub async fn list_campaigns(
        &self,
        status: Option<CampaignStatus>,
        channel: Option<CampaignChannel>,
    ) -> Result<Vec<Campaign>, KlaviyoError> {
        let mut path = "/campaigns?sort=-created_at".to_string();
        let mut filters = Vec::new();

        if let Some(status) = status {
            let status_str = match status {
                CampaignStatus::Draft => "draft",
                CampaignStatus::Scheduled => "scheduled",
                CampaignStatus::Sending => "sending",
                CampaignStatus::Sent => "sent",
                CampaignStatus::Cancelled => "cancelled",
            };
            filters.push(format!("equals(messages.channel,\"{status_str}\")"));
        }

        if let Some(channel) = channel {
            filters.push(format!("equals(messages.channel,\"{}\")", channel.as_str()));
        }

        if !filters.is_empty() {
            let _ = write!(path, "&filter={}", filters.join(","));
        }

        let response: ApiListResponse<Campaign> = self.get(&path).await?;
        Ok(response.data)
    }

    /// Get a single campaign by ID.
    ///
    /// # Errors
    ///
    /// Returns error if the campaign is not found or API request fails.
    #[instrument(skip(self), fields(campaign_id = %id))]
    pub async fn get_campaign(&self, id: &str) -> Result<Campaign, KlaviyoError> {
        let path = format!("/campaigns/{id}");
        let response: ApiResponse<Campaign> = self.get(&path).await?;
        Ok(response.data)
    }

    /// Create a new email campaign.
    ///
    /// # Arguments
    ///
    /// * `name` - Campaign name (internal identifier)
    /// * `subject` - Email subject line
    /// * `from_email` - Sender email address
    /// * `from_name` - Sender display name
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails.
    #[instrument(skip(self))]
    pub async fn create_email_campaign(
        &self,
        name: &str,
        subject: &str,
        from_email: &str,
        from_name: &str,
    ) -> Result<Campaign, KlaviyoError> {
        let input = CreateCampaignInput {
            data: CreateCampaignData {
                resource_type: "campaign",
                attributes: CreateCampaignAttributes {
                    name: name.to_string(),
                    audiences: super::CampaignAudiences {
                        included: vec![self.list_id().to_string()],
                        excluded: vec![],
                    },
                    send_options: Some(super::CampaignSendOptions {
                        use_smart_sending: true,
                    }),
                    campaign_messages: CreateCampaignMessages {
                        data: vec![CreateCampaignMessageData {
                            resource_type: "campaign-message",
                            attributes: CreateCampaignMessageAttributes {
                                channel: "email",
                                label: "Email".to_string(),
                                content: super::CampaignMessageContent {
                                    subject: subject.to_string(),
                                    preview_text: None,
                                    from_email: from_email.to_string(),
                                    from_label: from_name.to_string(),
                                    reply_to_email: Some(from_email.to_string()),
                                    cc_email: None,
                                    bcc_email: None,
                                },
                                render_options: None,
                            },
                        }],
                    },
                },
            },
        };

        let response: ApiResponse<Campaign> = self.post("/campaigns", &input).await?;
        Ok(response.data)
    }

    /// Create a new SMS campaign.
    ///
    /// # Arguments
    ///
    /// * `name` - Campaign name (internal identifier)
    /// * `body` - SMS message body (160 chars standard, 70 with emoji)
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails.
    #[instrument(skip(self))]
    pub async fn create_sms_campaign(
        &self,
        name: &str,
        body: &str,
    ) -> Result<Campaign, KlaviyoError> {
        // SMS campaigns use a different message structure
        // The body goes in a "definition" object instead of "content"
        let input = serde_json::json!({
            "data": {
                "type": "campaign",
                "attributes": {
                    "name": name,
                    "audiences": {
                        "included": [self.list_id()],
                        "excluded": []
                    },
                    "send_options": {
                        "use_smart_sending": true
                    },
                    "campaign-messages": {
                        "data": [{
                            "type": "campaign-message",
                            "attributes": {
                                "channel": "sms",
                                "label": "SMS",
                                "definition": {
                                    "body": body
                                },
                                "render_options": {
                                    "shorten_links": true,
                                    "add_org_prefix": true,
                                    "add_opt_out_language": true
                                }
                            }
                        }]
                    }
                }
            }
        });

        let response: ApiResponse<Campaign> = self.post("/campaigns", &input).await?;
        Ok(response.data)
    }

    /// Update an existing campaign.
    ///
    /// # Errors
    ///
    /// Returns error if the campaign is not found or API request fails.
    #[instrument(skip(self), fields(campaign_id = %id))]
    pub async fn update_campaign(
        &self,
        id: &str,
        name: Option<&str>,
    ) -> Result<Campaign, KlaviyoError> {
        let input = UpdateCampaignInput {
            data: UpdateCampaignData {
                resource_type: "campaign",
                id: id.to_string(),
                attributes: UpdateCampaignAttributes {
                    name: name.map(String::from),
                    audiences: None,
                    send_options: None,
                },
            },
        };

        let path = format!("/campaigns/{id}");
        let response: ApiResponse<Campaign> = self.patch(&path, &input).await?;
        Ok(response.data)
    }

    /// Delete a draft campaign.
    ///
    /// Only draft campaigns can be deleted.
    ///
    /// # Errors
    ///
    /// Returns error if the campaign is not found, not a draft, or API request fails.
    #[instrument(skip(self), fields(campaign_id = %id))]
    pub async fn delete_campaign(&self, id: &str) -> Result<(), KlaviyoError> {
        let path = format!("/campaigns/{id}");
        self.delete(&path).await
    }

    /// Send a campaign immediately.
    ///
    /// The campaign must be in draft status.
    ///
    /// # Errors
    ///
    /// Returns error if the campaign cannot be sent or API request fails.
    #[instrument(skip(self), fields(campaign_id = %id))]
    pub async fn send_campaign(&self, id: &str) -> Result<CampaignSendJob, KlaviyoError> {
        let input = SendCampaignInput {
            data: SendCampaignData {
                resource_type: "campaign-send-job",
                id: id.to_string(),
            },
        };

        let response: ApiResponse<CampaignSendJob> =
            self.post("/campaign-send-jobs", &input).await?;
        Ok(response.data)
    }

    /// Get the newsletter list details.
    ///
    /// # Errors
    ///
    /// Returns error if the list is not found or API request fails.
    #[instrument(skip(self))]
    pub async fn get_list(&self) -> Result<List, KlaviyoError> {
        let path = format!("/lists/{}", self.list_id());
        let response: ApiResponse<List> = self.get(&path).await?;
        Ok(response.data)
    }

    /// Get subscriber statistics (email and SMS counts) for the newsletter list.
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_subscriber_stats(&self) -> Result<SubscriberStats, KlaviyoError> {
        // Fetch profiles with subscription info
        // Note: This is a simplified implementation that fetches a batch.
        // For accurate counts, you'd need to paginate through all profiles
        // or use Klaviyo's metrics/reporting endpoints.
        let path = format!(
            "/lists/{}/profiles?page[size]=100&additional-fields[profile]=subscriptions",
            self.list_id()
        );

        let response: ApiListResponse<Profile> = self.get(&path).await?;

        let mut stats = SubscriberStats::default();

        for profile in &response.data {
            if let Some(subs) = &profile.attributes.subscriptions {
                // Count email subscribers
                if let Some(email) = &subs.email
                    && email.marketing.consent == "SUBSCRIBED"
                {
                    stats.email_subscribers += 1;
                }
                // Count SMS subscribers
                if let Some(sms) = &subs.sms
                    && sms.marketing.consent == "SUBSCRIBED"
                {
                    stats.sms_subscribers += 1;
                }
            }
        }

        Ok(stats)
    }

    /// Get profiles (subscribers) from the newsletter list.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of profiles to return (1-100)
    /// * `cursor` - Pagination cursor for next page
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_list_profiles(
        &self,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<Profile>, Option<String>), KlaviyoError> {
        let page_size = limit.min(100);
        let mut path = format!("/lists/{}/profiles?page[size]={page_size}", self.list_id());

        if let Some(cursor) = cursor {
            let _ = write!(path, "&page[cursor]={cursor}");
        }

        let response: ApiListResponse<Profile> = self.get(&path).await?;

        let next_cursor = response.links.and_then(|l| {
            l.next.and_then(|url| {
                // Extract cursor from URL
                url.split("page%5Bcursor%5D=")
                    .nth(1)
                    .or_else(|| url.split("page[cursor]=").nth(1))
                    .map(|s| s.split('&').next().unwrap_or(s).to_string())
            })
        });

        Ok((response.data, next_cursor))
    }
}
