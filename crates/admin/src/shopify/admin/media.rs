//! Media and file management operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError,
    queries::{FileDelete, FileUpdate, ProductReorderMedia, ProductSetMedia, StagedUploadsCreate},
};
use crate::shopify::types::StagedUploadTarget;

impl AdminClient {
    /// Delete files (images, videos, etc.) from the store.
    ///
    /// # Arguments
    ///
    /// * `file_ids` - List of file IDs to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn delete_files(
        &self,
        file_ids: Vec<String>,
    ) -> Result<Vec<String>, AdminShopifyError> {
        use super::queries::file_delete::Variables;

        let variables = Variables { file_ids };

        let response = self.execute::<FileDelete>(variables).await?;

        if let Some(payload) = response.file_delete {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(payload.deleted_file_ids.unwrap_or_default());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "File deletion failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Reorder product media (images).
    ///
    /// # Arguments
    ///
    /// * `product_id` - The product ID
    /// * `moves` - List of media moves (id, `new_position`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn reorder_product_media(
        &self,
        product_id: &str,
        moves: Vec<(String, i64)>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::product_reorder_media::{MoveInput, Variables};

        let move_inputs: Vec<MoveInput> = moves
            .into_iter()
            .map(|(id, new_position)| MoveInput {
                id,
                new_position: new_position.to_string(),
            })
            .collect();

        let variables = Variables {
            id: product_id.to_string(),
            moves: move_inputs,
        };

        let response = self.execute::<ProductReorderMedia>(variables).await?;

        if let Some(payload) = response.product_reorder_media {
            if !payload.media_user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .media_user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Media reorder failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update media alt text.
    ///
    /// # Arguments
    ///
    /// * `media_id` - The media/file ID to update
    /// * `alt_text` - The new alt text
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_media_alt_text(
        &self,
        media_id: &str,
        alt_text: &str,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::file_update::{FileUpdateInput, Variables};

        let variables = Variables {
            files: vec![FileUpdateInput {
                id: media_id.to_string(),
                alt: Some(alt_text.to_string()),
                original_source: None,
                preview_image_source: None,
                filename: None,
                references_to_add: None,
                references_to_remove: None,
            }],
        };

        let response = self.execute::<FileUpdate>(variables).await?;

        if let Some(payload) = response.file_update {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Media alt text update failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Create a staged upload target for uploading files.
    ///
    /// # Arguments
    ///
    /// * `filename` - The filename to upload
    /// * `mime_type` - The MIME type (e.g., "image/jpeg")
    /// * `file_size` - The file size in bytes
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn create_staged_upload(
        &self,
        filename: &str,
        mime_type: &str,
        file_size: i64,
        resource: &str,
    ) -> Result<StagedUploadTarget, AdminShopifyError> {
        use super::queries::staged_uploads_create::{
            StagedUploadHttpMethodType, StagedUploadInput,
            StagedUploadTargetGenerateUploadResource, Variables,
        };

        let resource_type = match resource {
            "VIDEO" => StagedUploadTargetGenerateUploadResource::VIDEO,
            "FILE" => StagedUploadTargetGenerateUploadResource::FILE,
            _ => StagedUploadTargetGenerateUploadResource::IMAGE,
        };

        let variables = Variables {
            input: vec![StagedUploadInput {
                filename: filename.to_string(),
                mime_type: mime_type.to_string(),
                resource: resource_type,
                file_size: Some(file_size.to_string()),
                http_method: Some(StagedUploadHttpMethodType::POST),
            }],
        };

        let response = self.execute::<StagedUploadsCreate>(variables).await?;

        if let Some(payload) = response.staged_uploads_create {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(targets) = payload.staged_targets
                && let Some(target) = targets.into_iter().next()
            {
                let parameters: Vec<(String, String)> = target
                    .parameters
                    .into_iter()
                    .map(|p| (p.name, p.value))
                    .collect();

                return Ok(StagedUploadTarget {
                    url: target.url.unwrap_or_default(),
                    resource_url: target.resource_url.unwrap_or_default(),
                    parameters,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Staged upload creation failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Attach uploaded media to a product.
    ///
    /// Uses `productSet` mutation with files parameter (non-deprecated).
    ///
    /// # Arguments
    ///
    /// * `product_id` - The product ID
    /// * `resource_url` - The URL returned from staged upload
    /// * `alt_text` - Optional alt text for the image
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn attach_media_to_product(
        &self,
        product_id: &str,
        resource_url: &str,
        alt_text: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::product_set_media::{
            FileSetInput, ProductSetIdentifiers, ProductSetInput, Variables,
        };

        let variables = Variables {
            input: ProductSetInput {
                files: Some(vec![FileSetInput {
                    id: None,
                    original_source: Some(resource_url.to_string()),
                    alt: alt_text.map(String::from),
                    content_type: None,
                    filename: None,
                    duplicate_resolution_mode: None,
                }]),
                description_html: None,
                handle: None,
                seo: None,
                product_type: None,
                category: None,
                tags: None,
                template_suffix: None,
                gift_card_template_suffix: None,
                title: None,
                vendor: None,
                gift_card: None,
                redirect_new_handle: None,
                collections: None,
                metafields: None,
                variants: None,
                status: None,
                requires_selling_plan: None,
                product_options: None,
                claim_ownership: None,
                combined_listing_role: None,
            },
            identifier: Some(ProductSetIdentifiers {
                id: Some(product_id.to_string()),
                handle: None,
                custom_id: None,
            }),
        };

        let response = self.execute::<ProductSetMedia>(variables).await?;

        if let Some(payload) = response.product_set {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Media attachment failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }
}
