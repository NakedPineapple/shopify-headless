//! Search index builder.
//!
//! Builds the search index asynchronously from Shopify products/collections
//! and local content.

use tantivy::Index;
use tracing::{debug, error, info, instrument, warn};

use crate::content::ContentStore;
use crate::shopify::StorefrontClient;

use super::{DocType, SearchFields, SearchIndex};

/// Spawn a background task to build the search index.
///
/// The index will be populated asynchronously. Until complete,
/// `SearchIndex::search()` returns empty results.
pub fn build_index_async(
    search_index: SearchIndex,
    storefront: StorefrontClient,
    content: ContentStore,
) {
    info!("Spawning background search index build task");
    tokio::spawn(async move {
        info!("Search index build task started");
        match build_index(&storefront, &content).await {
            Ok((index, fields)) => {
                info!("Search index built successfully, setting as ready");
                if let Err(e) = search_index.set_ready(index, fields) {
                    error!(error = %e, "Failed to set search index as ready");
                } else {
                    let docs = search_index.num_docs();
                    info!(docs, "Search index is now ready and serving requests");
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to build search index");
            }
        }
    });
}

/// Build the search index (called by background task).
#[instrument(skip_all)]
async fn build_index(
    storefront: &StorefrontClient,
    content: &ContentStore,
) -> Result<(Index, SearchFields), BuildError> {
    info!("Building search schema");
    let (schema, fields) = SearchIndex::build_schema();

    // Create in-memory index
    info!("Creating in-memory index");
    let index = Index::create_in_ram(schema);

    // Register the English stemmer tokenizer
    let tokenizer_manager = index.tokenizers();
    tokenizer_manager.register(
        "en_stem",
        tantivy::tokenizer::TextAnalyzer::builder(tantivy::tokenizer::SimpleTokenizer::default())
            .filter(tantivy::tokenizer::RemoveLongFilter::limit(40))
            .filter(tantivy::tokenizer::LowerCaser)
            .filter(tantivy::tokenizer::Stemmer::new(
                tantivy::tokenizer::Language::English,
            ))
            .build(),
    );

    let mut writer = index
        .writer(50_000_000) // 50MB buffer
        .map_err(|e| BuildError(format!("Failed to create writer: {e}")))?;

    // Index products from Shopify
    info!("Fetching and indexing products from Shopify");
    let products_count = index_products(storefront, &writer, &fields).await;
    info!(count = products_count, "Indexed products");

    // Index collections from Shopify
    info!("Fetching and indexing collections from Shopify");
    let collections_count = index_collections(storefront, &writer, &fields).await;
    info!(count = collections_count, "Indexed collections");

    // Index pages from local content
    info!("Indexing local pages");
    let pages_count = index_pages(content, &writer, &fields);
    info!(count = pages_count, "Indexed pages");

    // Index articles from local content
    info!("Indexing local articles");
    let articles_count = index_articles(content, &writer, &fields);
    info!(count = articles_count, "Indexed articles");

    // Commit the index
    info!("Committing index");
    writer
        .commit()
        .map_err(|e| BuildError(format!("Failed to commit index: {e}")))?;

    let total = products_count + collections_count + pages_count + articles_count;
    info!(total, "Search index built successfully");

    Ok((index, fields))
}

/// Index all products from Shopify.
async fn index_products(
    storefront: &StorefrontClient,
    writer: &tantivy::IndexWriter,
    fields: &SearchFields,
) -> usize {
    debug!("Starting to fetch products from Shopify");
    let mut count = 0;
    let mut cursor: Option<String> = None;
    let mut page = 0;

    loop {
        page += 1;
        debug!(page, cursor = ?cursor, "Fetching products page");
        let result = storefront
            .get_products(Some(50), cursor.clone(), None, None, None)
            .await;

        match result {
            Ok(connection) => {
                let batch_size = connection.products.len();
                debug!(page, batch_size, "Received products batch");
                for product in &connection.products {
                    let price_cents =
                        parse_price_cents(&product.price_range.min_variant_price.amount);
                    let available = u64::from(product.available_for_sale);

                    let doc = tantivy::doc!(
                        fields.doc_type => DocType::Product.as_str(),
                        fields.handle => product.handle.clone(),
                        fields.title => product.title.clone(),
                        fields.description => strip_html(&product.description_html),
                        fields.image_url => product.featured_image.as_ref().map_or(String::new(), |img| img.url.clone()),
                        fields.price => format_price(&product.price_range.min_variant_price.amount),
                        fields.price_cents => price_cents,
                        fields.available => available,
                        fields.title_text => product.title.clone(),
                        fields.description_text => strip_html(&product.description_html),
                        fields.tags_text => product.tags.join(" ")
                    );

                    if let Err(e) = writer.add_document(doc) {
                        warn!(error = %e, handle = %product.handle, "Failed to index product");
                    } else {
                        count += 1;
                    }
                }

                if connection.page_info.has_next_page {
                    cursor = connection.page_info.end_cursor;
                } else {
                    break;
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to fetch products for indexing");
                break;
            }
        }
    }

    count
}

/// Index all collections from Shopify.
async fn index_collections(
    storefront: &StorefrontClient,
    writer: &tantivy::IndexWriter,
    fields: &SearchFields,
) -> usize {
    let mut count = 0;
    let mut cursor: Option<String> = None;

    loop {
        let result = storefront
            .get_collections(Some(50), cursor.clone(), None)
            .await;

        match result {
            Ok(connection) => {
                for collection in &connection.collections {
                    let doc = tantivy::doc!(
                        fields.doc_type => DocType::Collection.as_str(),
                        fields.handle => collection.handle.clone(),
                        fields.title => collection.title.clone(),
                        fields.description => strip_html(&collection.description_html),
                        fields.image_url => collection.image.as_ref().map_or(String::new(), |img| img.url.clone()),
                        fields.price => String::new(),
                        fields.price_cents => 0u64,
                        fields.available => 1u64, // Collections are always "available"
                        fields.title_text => collection.title.clone(),
                        fields.description_text => strip_html(&collection.description_html),
                        fields.tags_text => String::new()
                    );

                    if let Err(e) = writer.add_document(doc) {
                        warn!(error = %e, handle = %collection.handle, "Failed to index collection");
                    } else {
                        count += 1;
                    }
                }

                if connection.page_info.has_next_page {
                    cursor = connection.page_info.end_cursor;
                } else {
                    break;
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to fetch collections for indexing");
                break;
            }
        }
    }

    count
}

/// Index all pages from local content.
fn index_pages(
    content: &ContentStore,
    writer: &tantivy::IndexWriter,
    fields: &SearchFields,
) -> usize {
    let mut count = 0;

    for page in content.get_all_pages() {
        let doc = tantivy::doc!(
            fields.doc_type => DocType::Page.as_str(),
            fields.handle => page.slug.clone(),
            fields.title => page.meta.title.clone(),
            fields.description => page.meta.description.clone().unwrap_or_default(),
            fields.image_url => String::new(),
            fields.price => String::new(),
            fields.price_cents => 0u64,
            fields.available => 1u64, // Pages are always "available"
            fields.title_text => page.meta.title.clone(),
            fields.description_text => strip_html(&page.content_html),
            fields.tags_text => String::new()
        );

        if let Err(e) = writer.add_document(doc) {
            warn!(error = %e, slug = %page.slug, "Failed to index page");
        } else {
            count += 1;
        }
    }

    count
}

/// Index all articles from local content.
fn index_articles(
    content: &ContentStore,
    writer: &tantivy::IndexWriter,
    fields: &SearchFields,
) -> usize {
    let mut count = 0;

    for post in content.get_published_posts() {
        let doc = tantivy::doc!(
            fields.doc_type => DocType::Article.as_str(),
            fields.handle => post.slug.clone(),
            fields.title => post.meta.title.clone(),
            fields.description => post.meta.description.clone().unwrap_or_default(),
            fields.image_url => post.meta.featured_image.clone().unwrap_or_default(),
            fields.price => String::new(),
            fields.price_cents => 0u64,
            fields.available => 1u64, // Articles are always "available"
            fields.title_text => post.meta.title.clone(),
            fields.description_text => strip_html(&post.content_html),
            fields.tags_text => post.meta.tags.join(" ")
        );

        if let Err(e) = writer.add_document(doc) {
            warn!(error = %e, slug = %post.slug, "Failed to index article");
        } else {
            count += 1;
        }
    }

    count
}

/// Strip HTML tags from a string.
fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    // Decode common HTML entities
    result
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Format a price string from Shopify's decimal format.
fn format_price(amount: &str) -> String {
    amount
        .parse::<f64>()
        .map_or_else(|_| format!("${amount}"), |n| format!("${n:.2}"))
}

/// Parse a decimal price string to cents (e.g., "24.99" -> 2499).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_price_cents(amount: &str) -> u64 {
    amount
        .parse::<f64>()
        .map(|n| (n * 100.0).round() as u64)
        .unwrap_or(0)
}

/// Build error wrapper.
#[derive(Debug)]
struct BuildError(String);

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BuildError {}
