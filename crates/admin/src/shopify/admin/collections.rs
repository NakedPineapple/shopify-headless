//! Collection management operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError,
    queries::{
        CollectionAddProductsV2, CollectionCreate, CollectionDelete, CollectionUpdate,
        CollectionUpdateFields, CollectionUpdateSortOrder, GetCollection,
        GetCollectionWithProducts, GetCollections, GetPublications, PublishablePublish,
        PublishableUnpublish,
    },
};
use crate::shopify::types::{
    Collection, CollectionConnection, CollectionProduct, CollectionWithProducts, Image, PageInfo,
};

impl AdminClient {
    /// Get a collection by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self), fields(collection_id = %id))]
    pub async fn get_collection(&self, id: &str) -> Result<Option<Collection>, AdminShopifyError> {
        let variables = super::queries::get_collection::Variables { id: id.to_string() };

        let response = self.execute::<GetCollection>(variables).await?;

        Ok(response.collection.map(|c| {
            use crate::shopify::types::{
                CollectionRule, CollectionRuleSet, CollectionSeo, Publication, ResourcePublication,
            };

            Collection {
                id: c.id,
                title: c.title,
                handle: c.handle,
                description: c.description,
                description_html: Some(c.description_html),
                products_count: c.products_count.map_or(0, |pc| pc.count),
                image: c.image.map(|img| Image {
                    id: img.id,
                    url: img.url,
                    alt_text: img.alt_text,
                    width: None,
                    height: None,
                }),
                updated_at: Some(c.updated_at),
                rule_set: c.rule_set.map(|rs| CollectionRuleSet {
                    applied_disjunctively: rs.applied_disjunctively,
                    rules: rs
                        .rules
                        .into_iter()
                        .map(|r| CollectionRule {
                            column: format!("{:?}", r.column),
                            relation: format!("{:?}", r.relation),
                            condition: r.condition,
                        })
                        .collect(),
                }),
                sort_order: Some(format!("{:?}", c.sort_order)),
                seo: Some(CollectionSeo {
                    title: c.seo.title,
                    description: c.seo.description,
                }),
                publications: c
                    .resource_publications_v2
                    .edges
                    .into_iter()
                    .map(|e| ResourcePublication {
                        publication: Publication {
                            id: e.node.publication.id.clone(),
                            #[allow(deprecated)]
                            name: e
                                .node
                                .publication
                                .catalog
                                .map(|c| c.title)
                                .unwrap_or(e.node.publication.name),
                        },
                        is_published: e.node.is_published,
                    })
                    .collect(),
            }
        }))
    }

    /// Get a paginated list of collections.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_collections(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<CollectionConnection, AdminShopifyError> {
        let variables = super::queries::get_collections::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetCollections>(variables).await?;

        let collections: Vec<Collection> = response
            .collections
            .edges
            .into_iter()
            .map(|e| {
                let c = e.node;
                Collection {
                    id: c.id,
                    title: c.title,
                    handle: c.handle,
                    description: c.description,
                    description_html: None,
                    products_count: c.products_count.map_or(0, |pc| pc.count),
                    image: c.image.map(|img| Image {
                        id: img.id,
                        url: img.url,
                        alt_text: img.alt_text,
                        width: None,
                        height: None,
                    }),
                    updated_at: Some(c.updated_at),
                    rule_set: None,
                    sort_order: None,
                    seo: None,
                    publications: vec![],
                }
            })
            .collect();

        Ok(CollectionConnection {
            collections,
            page_info: PageInfo {
                has_next_page: response.collections.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: response.collections.page_info.end_cursor,
            },
        })
    }

    /// Create a new collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn create_collection(
        &self,
        title: &str,
        description_html: Option<&str>,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::collection_create::{CollectionInput, Variables};

        let variables = Variables {
            input: CollectionInput {
                title: Some(title.to_string()),
                description_html: description_html.map(String::from),
                handle: None,
                id: None,
                image: None,
                metafields: None,
                products: None,
                redirect_new_handle: None,
                rule_set: None,
                seo: None,
                sort_order: None,
                template_suffix: None,
            },
        };

        let response = self.execute::<CollectionCreate>(variables).await?;

        if let Some(payload) = response.collection_create {
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

            if let Some(collection) = payload.collection {
                return Ok(collection.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No collection returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_collection(
        &self,
        id: &str,
        title: Option<&str>,
        description_html: Option<&str>,
        sort_order: Option<&str>,
        seo_title: Option<&str>,
        seo_description: Option<&str>,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::collection_update_fields::{CollectionSortOrder, Variables};

        let sort_order_enum = match sort_order.unwrap_or("MANUAL") {
            "BEST_SELLING" => CollectionSortOrder::BEST_SELLING,
            "ALPHA_ASC" => CollectionSortOrder::ALPHA_ASC,
            "ALPHA_DESC" => CollectionSortOrder::ALPHA_DESC,
            "PRICE_ASC" => CollectionSortOrder::PRICE_ASC,
            "PRICE_DESC" => CollectionSortOrder::PRICE_DESC,
            "CREATED_DESC" => CollectionSortOrder::CREATED_DESC,
            "CREATED" => CollectionSortOrder::CREATED,
            _ => CollectionSortOrder::MANUAL,
        };

        let variables = Variables {
            id: id.to_string(),
            title: title.unwrap_or("").to_string(),
            description_html: description_html.unwrap_or("").to_string(),
            sort_order: sort_order_enum,
            seo_title: seo_title.unwrap_or("").to_string(),
            seo_description: seo_description.unwrap_or("").to_string(),
        };

        let response = self.execute::<CollectionUpdateFields>(variables).await?;

        if let Some(payload) = response.collection_update {
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

            if let Some(collection) = payload.collection {
                return Ok(collection.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No collection returned from update".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update only the sort order of a collection.
    ///
    /// This uses a focused mutation that only sends the sort order field,
    /// avoiding the `graphql_client` `skip_none` bug.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_collection_sort_order(
        &self,
        id: &str,
        sort_order: &str,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::collection_update_sort_order::{CollectionSortOrder, Variables};

        let sort_order_enum = match sort_order {
            "BEST_SELLING" => CollectionSortOrder::BEST_SELLING,
            "ALPHA_ASC" => CollectionSortOrder::ALPHA_ASC,
            "ALPHA_DESC" => CollectionSortOrder::ALPHA_DESC,
            "PRICE_ASC" => CollectionSortOrder::PRICE_ASC,
            "PRICE_DESC" => CollectionSortOrder::PRICE_DESC,
            "CREATED_DESC" => CollectionSortOrder::CREATED_DESC,
            "CREATED" => CollectionSortOrder::CREATED,
            "MANUAL" => CollectionSortOrder::MANUAL,
            _ => {
                return Err(AdminShopifyError::UserError(format!(
                    "Invalid sort order: {sort_order}"
                )));
            }
        };

        let variables = Variables {
            id: id.to_string(),
            sort_order: sort_order_enum,
        };

        let response = self.execute::<CollectionUpdateSortOrder>(variables).await?;

        if let Some(payload) = response.collection_update
            && !payload.user_errors.is_empty()
        {
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

        Ok(())
    }

    /// Delete a collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn delete_collection(&self, id: &str) -> Result<String, AdminShopifyError> {
        use super::queries::collection_delete::{CollectionDeleteInput, Variables};

        let variables = Variables {
            input: CollectionDeleteInput { id: id.to_string() },
        };

        let response = self.execute::<CollectionDelete>(variables).await?;

        if let Some(payload) = response.collection_delete {
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

            if let Some(deleted_id) = payload.deleted_collection_id {
                return Ok(deleted_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Collection deletion failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update a collection's image.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn update_collection_image(
        &self,
        id: &str,
        image_url: &str,
        alt_text: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        let image_obj = alt_text.map_or_else(
            || serde_json::json!({ "src": image_url }),
            |alt| serde_json::json!({ "src": image_url, "altText": alt }),
        );

        let query = r"
            mutation CollectionUpdateImage($input: CollectionInput!) {
                collectionUpdate(input: $input) {
                    collection {
                        id
                        image {
                            id
                            url
                        }
                    }
                    userErrors {
                        field
                        message
                    }
                }
            }
        ";

        let body = serde_json::json!({
            "query": query,
            "variables": {
                "input": {
                    "id": id,
                    "image": image_obj
                }
            }
        });

        let response = self.execute_raw_graphql(body).await?;

        if let Some(errors) = response
            .get("collectionUpdate")
            .and_then(|p| p.get("userErrors"))
            .and_then(|e| e.as_array())
        {
            let error_messages: Vec<String> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .map(String::from)
                .collect();

            if !error_messages.is_empty() {
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }
        }

        Ok(())
    }

    /// Delete a collection's image.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn delete_collection_image(&self, id: &str) -> Result<(), AdminShopifyError> {
        let query = r"
            mutation CollectionDeleteImage($input: CollectionInput!) {
                collectionUpdate(input: $input) {
                    collection {
                        id
                    }
                    userErrors {
                        field
                        message
                    }
                }
            }
        ";

        let body = serde_json::json!({
            "query": query,
            "variables": {
                "input": {
                    "id": id,
                    "image": serde_json::Value::Null
                }
            }
        });

        let response = self.execute_raw_graphql(body).await?;

        if let Some(errors) = response
            .get("collectionUpdate")
            .and_then(|p| p.get("userErrors"))
            .and_then(|e| e.as_array())
        {
            let error_messages: Vec<String> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .map(String::from)
                .collect();

            if !error_messages.is_empty() {
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }
        }

        Ok(())
    }

    /// Get a collection with its products.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self), fields(collection_id = %id))]
    pub async fn get_collection_with_products(
        &self,
        id: &str,
        first: i64,
        after: Option<String>,
    ) -> Result<Option<CollectionWithProducts>, AdminShopifyError> {
        use crate::shopify::types::{
            CollectionRule, CollectionRuleSet, CollectionSeo, Publication, ResourcePublication,
        };

        let variables = super::queries::get_collection_with_products::Variables {
            id: id.to_string(),
            first: Some(first),
            after,
        };

        let response = self.execute::<GetCollectionWithProducts>(variables).await?;

        Ok(response.collection.map(|c| {
            let products: Vec<CollectionProduct> = c
                .products
                .edges
                .into_iter()
                .map(|e| {
                    let p = e.node;
                    let min_price = &p.price_range_v2.min_variant_price;
                    let price = min_price.amount.clone();
                    let currency_code = format!("{:?}", min_price.currency_code);

                    #[allow(deprecated)]
                    CollectionProduct {
                        id: p.id,
                        title: p.title,
                        handle: p.handle,
                        status: format!("{:?}", p.status),
                        image_url: p.featured_image.map(|img| img.url),
                        total_inventory: p.total_inventory,
                        price,
                        currency_code,
                    }
                })
                .collect();

            let has_next_page = c.products.page_info.has_next_page;
            let end_cursor = c.products.page_info.end_cursor;

            let collection = Collection {
                id: c.id,
                title: c.title,
                handle: c.handle,
                description: c.description,
                description_html: Some(c.description_html),
                products_count: c.products_count.map_or(0, |pc| pc.count),
                image: c.image.map(|img| Image {
                    id: img.id,
                    url: img.url,
                    alt_text: img.alt_text,
                    width: None,
                    height: None,
                }),
                updated_at: Some(c.updated_at),
                rule_set: c.rule_set.map(|rs| CollectionRuleSet {
                    applied_disjunctively: rs.applied_disjunctively,
                    rules: rs
                        .rules
                        .into_iter()
                        .map(|r| CollectionRule {
                            column: format!("{:?}", r.column),
                            relation: format!("{:?}", r.relation),
                            condition: r.condition,
                        })
                        .collect(),
                }),
                sort_order: Some(format!("{:?}", c.sort_order)),
                seo: Some(CollectionSeo {
                    title: c.seo.title,
                    description: c.seo.description,
                }),
                publications: c
                    .resource_publications_v2
                    .edges
                    .into_iter()
                    .map(|e| ResourcePublication {
                        publication: Publication {
                            id: e.node.publication.id.clone(),
                            #[allow(deprecated)]
                            name: e
                                .node
                                .publication
                                .catalog
                                .map(|c| c.title)
                                .unwrap_or(e.node.publication.name),
                        },
                        is_published: e.node.is_published,
                    })
                    .collect(),
            };

            CollectionWithProducts {
                collection,
                products,
                has_next_page,
                end_cursor,
            }
        }))
    }

    /// Add products to a collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn add_products_to_collection(
        &self,
        collection_id: &str,
        product_ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::collection_add_products_v2::Variables {
            id: collection_id.to_string(),
            product_ids,
        };

        let response = self.execute::<CollectionAddProductsV2>(variables).await?;

        if let Some(payload) = response.collection_add_products_v2 {
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
            message: "Add products to collection failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove products from a collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn remove_products_from_collection(
        &self,
        collection_id: &str,
        product_ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::CollectionRemoveProducts;

        let variables = super::queries::collection_remove_products::Variables {
            id: collection_id.to_string(),
            product_ids,
        };

        let response = self.execute::<CollectionRemoveProducts>(variables).await?;

        if let Some(payload) = response.collection_remove_products {
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
            message: "Remove products from collection failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Reorder products in a collection (manual sort only).
    ///
    /// # Arguments
    ///
    /// * `collection_id` - The collection GID
    /// * `moves` - List of (`product_id`, `new_position`) tuples
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn reorder_collection_products(
        &self,
        collection_id: &str,
        moves: Vec<(String, i64)>,
    ) -> Result<(), AdminShopifyError> {
        let query = r"
            mutation CollectionReorderProducts($id: ID!, $moves: [MoveInput!]!) {
                collectionReorderProducts(id: $id, moves: $moves) {
                    job {
                        id
                        done
                    }
                    userErrors {
                        field
                        message
                    }
                }
            }
        ";

        let moves_input: Vec<serde_json::Value> = moves
            .into_iter()
            .map(|(id, new_position)| {
                serde_json::json!({
                    "id": id,
                    "newPosition": new_position
                })
            })
            .collect();

        let body = serde_json::json!({
            "query": query,
            "variables": {
                "id": collection_id,
                "moves": moves_input
            }
        });

        let response = self.execute_raw_graphql(body).await?;

        if let Some(errors) = response
            .get("collectionReorderProducts")
            .and_then(|p| p.get("userErrors"))
            .and_then(|e| e.as_array())
        {
            let error_messages: Vec<String> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .map(String::from)
                .collect();

            if !error_messages.is_empty() {
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }
        }

        Ok(())
    }

    /// Get all publications (sales channels) for the shop.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_publications(
        &self,
    ) -> Result<Vec<crate::shopify::types::Publication>, AdminShopifyError> {
        let variables = super::queries::get_publications::Variables {};
        let response = self.execute::<GetPublications>(variables).await?;

        Ok(response
            .publications
            .edges
            .into_iter()
            .map(|e| {
                let id = e.node.id;
                #[allow(deprecated)]
                let name = e.node.catalog.map(|c| c.title).unwrap_or(e.node.name);
                crate::shopify::types::Publication { id, name }
            })
            .collect())
    }

    /// Publish a collection to specified publications.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn publish_collection(
        &self,
        collection_id: &str,
        publication_ids: &[String],
    ) -> Result<(), AdminShopifyError> {
        if publication_ids.is_empty() {
            return Ok(());
        }

        let variables = super::queries::publishable_publish::Variables {
            id: collection_id.to_string(),
            input: publication_ids
                .iter()
                .map(
                    |pub_id| super::queries::publishable_publish::PublicationInput {
                        publication_id: Some(pub_id.clone()),
                        publish_date: None,
                    },
                )
                .collect(),
        };

        let response = self.execute::<PublishablePublish>(variables).await?;

        if let Some(payload) = response.publishable_publish
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{field}: {}", e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Unpublish a collection from specified publications.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn unpublish_collection(
        &self,
        collection_id: &str,
        publication_ids: &[String],
    ) -> Result<(), AdminShopifyError> {
        if publication_ids.is_empty() {
            return Ok(());
        }

        let variables = super::queries::publishable_unpublish::Variables {
            id: collection_id.to_string(),
            input: publication_ids
                .iter()
                .map(
                    |pub_id| super::queries::publishable_unpublish::PublicationInput {
                        publication_id: Some(pub_id.clone()),
                        publish_date: None,
                    },
                )
                .collect(),
        };

        let response = self.execute::<PublishableUnpublish>(variables).await?;

        if let Some(payload) = response.publishable_unpublish
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{field}: {}", e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }
}
