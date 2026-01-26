//! Full-text search using Tantivy.
//!
//! This module provides a search index that is built asynchronously at startup from:
//! - Products and collections from Shopify
//! - Pages and blog posts from local content
//!
//! The app starts immediately with an empty index. A background task builds
//! the real index and swaps it in atomically when ready.

mod indexer;

use std::sync::{Arc, RwLock};

use std::ops::Bound;

use tantivy::collector::TopDocs;
use tantivy::query::{
    BooleanQuery, FuzzyTermQuery, Occur, Query, RangeQuery, RegexQuery, TermQuery,
};
use tantivy::schema::{
    Field, IndexRecordOption, STORED, Schema, TextFieldIndexing, TextOptions, Value,
};
use tantivy::{Index, IndexReader, ReloadPolicy, Term};
use tracing::instrument;

pub use indexer::build_index_async;

/// Document types that can be indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Product,
    Collection,
    Page,
    Article,
}

impl DocType {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Collection => "collection",
            Self::Page => "page",
            Self::Article => "article",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "product" => Some(Self::Product),
            "collection" => Some(Self::Collection),
            "page" => Some(Self::Page),
            "article" => Some(Self::Article),
            _ => None,
        }
    }
}

/// A search result item.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub doc_type: DocType,
    pub handle: String,
    pub title: String,
    pub description: String,
    pub image_url: Option<String>,
    pub price: Option<String>,
    pub price_cents: Option<u64>,
    pub available: bool,
    pub score: f32,
}

/// Schema field handles for the search index.
#[derive(Clone)]
pub struct SearchFields {
    // Stored fields (returned in results)
    pub doc_type: Field,
    pub handle: Field,
    pub title: Field,
    pub description: Field,
    pub image_url: Field,
    pub price: Field,
    pub price_cents: Field,
    pub available: Field,
    // Text fields for full-text search (not stored, just indexed)
    pub title_text: Field,
    pub description_text: Field,
    pub tags_text: Field,
}

/// Inner index state (once built).
struct ReadyIndex {
    #[allow(dead_code)]
    index: Index,
    reader: IndexReader,
    fields: SearchFields,
}

/// The search index.
///
/// Starts empty and is populated asynchronously by a background task.
#[derive(Clone)]
pub struct SearchIndex {
    inner: Arc<RwLock<Option<ReadyIndex>>>,
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchIndex {
    /// Create a new empty search index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if the index is ready.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Set the built index. Called by the background builder task.
    pub(crate) fn set_ready(&self, index: Index, fields: SearchFields) -> Result<(), SearchError> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| SearchError::Index(format!("Failed to create reader: {e}")))?;

        let ready = ReadyIndex {
            index,
            reader,
            fields,
        };

        *self
            .inner
            .write()
            .map_err(|_| SearchError::Index("Lock poisoned".to_string()))? = Some(ready);

        Ok(())
    }

    /// Build the schema for the search index.
    pub(crate) fn build_schema() -> (Schema, SearchFields) {
        use tantivy::schema::{FAST, INDEXED, NumericOptions, STRING};

        let mut schema_builder = Schema::builder();

        // Stored and indexed fields
        // STRING means indexed but not tokenized (exact match)
        let doc_type = schema_builder.add_text_field("doc_type", STRING | STORED);
        let handle = schema_builder.add_text_field("handle", STORED);
        let title = schema_builder.add_text_field("title", STORED);
        let description = schema_builder.add_text_field("description", STORED);
        let image_url = schema_builder.add_text_field("image_url", STORED);
        let price = schema_builder.add_text_field("price", STORED);

        // Numeric fields for filtering/sorting
        let price_cents = schema_builder.add_u64_field(
            "price_cents",
            NumericOptions::default()
                .set_stored()
                .set_indexed()
                .set_fast(),
        );
        let available = schema_builder.add_u64_field(
            "available",
            NumericOptions::default()
                .set_stored()
                .set_indexed()
                .set_fast(),
        );

        // Text indexing options for full-text search
        let text_indexing = TextFieldIndexing::default()
            .set_tokenizer("en_stem")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let text_options = TextOptions::default().set_indexing_options(text_indexing);

        // Indexed text fields (for searching)
        let title_text = schema_builder.add_text_field("title_text", text_options.clone());
        let description_text =
            schema_builder.add_text_field("description_text", text_options.clone());
        let tags_text = schema_builder.add_text_field("tags_text", text_options);

        let schema = schema_builder.build();
        let fields = SearchFields {
            doc_type,
            handle,
            title,
            description,
            image_url,
            price,
            price_cents,
            available,
            title_text,
            description_text,
            tags_text,
        };

        (schema, fields)
    }

    /// Search the index with the given query string and filters.
    ///
    /// Returns empty results if the index isn't ready yet.
    ///
    /// # Errors
    ///
    /// Returns an error if the index lock is poisoned or the search query fails.
    #[instrument(skip(self))]
    // Allow: The RwLockReadGuard must be held for the entire search operation because
    // `ready` is a reference that borrows from the guard's protected data. Dropping
    // the guard early would release the read lock and invalidate the `ready` reference,
    // causing use-after-free. The searcher, fields, and all document access depend on
    // this lock being held.
    #[allow(clippy::significant_drop_tightening)]
    pub fn search_filtered(
        &self,
        query_str: &str,
        filters: &SearchFilters,
        sort: SearchSort,
        limit: usize,
    ) -> Result<SearchResults, SearchError> {
        let query_str = query_str.trim().to_lowercase();

        let guard = self
            .inner
            .read()
            .map_err(|_| SearchError::Index("Lock poisoned".to_string()))?;

        let Some(ready) = guard.as_ref() else {
            return Ok(SearchResults {
                query: query_str,
                ..Default::default()
            });
        };

        let searcher = ready.reader.searcher();

        // Build the query
        let query: Box<dyn Query> = if query_str.is_empty() {
            // Match all products when no query
            Box::new(tantivy::query::AllQuery)
        } else {
            let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

            for term in query_str.split_whitespace() {
                let title_term = Term::from_field_text(ready.fields.title_text, term);
                subqueries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(title_term.clone(), IndexRecordOption::Basic)),
                ));

                if term.len() >= 3 {
                    let fuzzy_title = FuzzyTermQuery::new(title_term, 1, true);
                    subqueries.push((Occur::Should, Box::new(fuzzy_title)));
                }

                let desc_term = Term::from_field_text(ready.fields.description_text, term);
                if term.len() >= 3 {
                    let fuzzy_desc = FuzzyTermQuery::new(desc_term, 1, true);
                    subqueries.push((Occur::Should, Box::new(fuzzy_desc)));
                }

                let tags_term = Term::from_field_text(ready.fields.tags_text, term);
                subqueries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(tags_term, IndexRecordOption::Basic)),
                ));
            }

            Box::new(BooleanQuery::new(subqueries))
        };

        // Add filters
        let query = Self::apply_filters(query, &ready.fields, filters);

        // Collect results based on sort order
        let results = match sort {
            SearchSort::Relevance => {
                let top_docs = searcher
                    .search(&query, &TopDocs::with_limit(limit))
                    .map_err(|e| SearchError::Query(format!("Search failed: {e}")))?;
                Self::collect_results(&searcher, &ready.fields, top_docs)?
            }
            SearchSort::PriceAsc | SearchSort::PriceDesc => {
                // For price sorting, we need to collect all and sort manually
                // since Tantivy's fast field sorting requires more setup
                let top_docs = searcher
                    .search(&query, &TopDocs::with_limit(limit * 2))
                    .map_err(|e| SearchError::Query(format!("Search failed: {e}")))?;
                let mut results = Self::collect_results(&searcher, &ready.fields, top_docs)?;

                results.sort_by(|a, b| {
                    let price_a = a.price_cents.unwrap_or(0);
                    let price_b = b.price_cents.unwrap_or(0);
                    if sort == SearchSort::PriceAsc {
                        price_a.cmp(&price_b)
                    } else {
                        price_b.cmp(&price_a)
                    }
                });

                results.truncate(limit);
                results
            }
        };

        // Count totals for facets
        let (total_count, in_stock_count, out_of_stock_count, min_price, max_price) =
            Self::compute_facets(&searcher, &ready.fields, &query_str)?;

        Ok(SearchResults {
            products: results,
            collections: Vec::new(),
            pages: Vec::new(),
            articles: Vec::new(),
            query: query_str,
            total_count,
            in_stock_count,
            out_of_stock_count,
            min_price_cents: min_price,
            max_price_cents: max_price,
        })
    }

    /// Apply filters to a query.
    fn apply_filters(
        base_query: Box<dyn Query>,
        fields: &SearchFields,
        filters: &SearchFilters,
    ) -> Box<dyn Query> {
        let mut must_clauses: Vec<(Occur, Box<dyn Query>)> = vec![(Occur::Must, base_query)];

        // Filter by doc_type = product only for filtered searches
        let product_term = Term::from_field_text(fields.doc_type, "product");
        must_clauses.push((
            Occur::Must,
            Box::new(TermQuery::new(product_term, IndexRecordOption::Basic)),
        ));

        // Availability filter
        if let Some(available) = filters.available {
            let val = u64::from(available);
            let term = Term::from_field_u64(fields.available, val);
            must_clauses.push((
                Occur::Must,
                Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
            ));
        }

        // Price range filter
        if filters.min_price_cents.is_some() || filters.max_price_cents.is_some() {
            let min = filters.min_price_cents.unwrap_or(0);
            let max = filters.max_price_cents.unwrap_or(u64::MAX);
            let range_query = RangeQuery::new(
                Bound::Included(Term::from_field_u64(fields.price_cents, min)),
                Bound::Included(Term::from_field_u64(fields.price_cents, max)),
            );
            must_clauses.push((Occur::Must, Box::new(range_query)));
        }

        Box::new(BooleanQuery::new(must_clauses))
    }

    /// Collect search results from top docs.
    fn collect_results(
        searcher: &tantivy::Searcher,
        fields: &SearchFields,
        top_docs: Vec<(f32, tantivy::DocAddress)>,
    ) -> Result<Vec<SearchResult>, SearchError> {
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc = searcher
                .doc::<tantivy::TantivyDocument>(doc_address)
                .map_err(|e| SearchError::Query(format!("Failed to retrieve doc: {e}")))?;
            results.push(Self::doc_to_result(fields, &doc, score)?);
        }
        Ok(results)
    }

    /// Compute facet counts for the current query.
    fn compute_facets(
        searcher: &tantivy::Searcher,
        fields: &SearchFields,
        query_str: &str,
    ) -> Result<(usize, usize, usize, u64, u64), SearchError> {
        // Build base query for products only
        let product_term = Term::from_field_text(fields.doc_type, "product");
        let product_filter = TermQuery::new(product_term, IndexRecordOption::Basic);

        let base_query: Box<dyn Query> = if query_str.is_empty() {
            Box::new(product_filter)
        } else {
            let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
            for term in query_str.split_whitespace() {
                let title_term = Term::from_field_text(fields.title_text, term);
                subqueries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(title_term.clone(), IndexRecordOption::Basic)),
                ));
                if term.len() >= 3 {
                    subqueries.push((
                        Occur::Should,
                        Box::new(FuzzyTermQuery::new(title_term, 1, true)),
                    ));
                }
            }
            let text_query = BooleanQuery::new(subqueries);
            Box::new(BooleanQuery::new(vec![
                (Occur::Must, Box::new(product_filter)),
                (Occur::Must, Box::new(text_query)),
            ]))
        };

        // Get all matching products to compute facets
        let all_docs = searcher
            .search(&base_query, &TopDocs::with_limit(10000))
            .map_err(|e| SearchError::Query(format!("Facet query failed: {e}")))?;

        let mut total = 0;
        let mut in_stock = 0;
        let mut out_of_stock = 0;
        let mut min_price = u64::MAX;
        let mut max_price = 0u64;

        for (_score, doc_address) in all_docs {
            if let Ok(doc) = searcher.doc::<tantivy::TantivyDocument>(doc_address) {
                total += 1;

                let available = doc
                    .get_first(fields.available)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if available == 1 {
                    in_stock += 1;
                } else {
                    out_of_stock += 1;
                }

                let price = doc
                    .get_first(fields.price_cents)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if price > 0 {
                    min_price = min_price.min(price);
                    max_price = max_price.max(price);
                }
            }
        }

        if min_price == u64::MAX {
            min_price = 0;
        }

        Ok((total, in_stock, out_of_stock, min_price, max_price))
    }

    /// Search the index with the given query string (simple version for suggestions).
    ///
    /// Returns empty results if the index isn't ready yet.
    ///
    /// # Errors
    ///
    /// Returns an error if the index lock is poisoned or the search query fails.
    #[instrument(skip(self))]
    // Allow: The RwLockReadGuard must be held for the entire search operation because
    // `ready` is a reference that borrows from the guard's protected data. Dropping
    // the guard early would release the read lock and invalidate the `ready` reference,
    // causing use-after-free. The searcher, fields, and all document access depend on
    // this lock being held.
    #[allow(clippy::significant_drop_tightening)]
    pub fn search(&self, query_str: &str, limit: usize) -> Result<SearchResults, SearchError> {
        let query_str = query_str.trim().to_lowercase();
        if query_str.is_empty() {
            return Ok(SearchResults::default());
        }

        let guard = self
            .inner
            .read()
            .map_err(|_| SearchError::Index("Lock poisoned".to_string()))?;

        let Some(ready) = guard.as_ref() else {
            // Index not ready yet, return empty results
            return Ok(SearchResults {
                query: query_str,
                ..Default::default()
            });
        };

        let searcher = ready.reader.searcher();

        // Build a boolean query combining prefix and fuzzy matches
        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        // For each search term, create queries on searchable fields
        for term in query_str.split_whitespace() {
            // For short terms, use prefix matching (regex)
            // For longer terms, use exact + fuzzy matching
            if term.len() < 3 {
                // Prefix match on title using regex (e.g., "f" matches "facial")
                // Escape basic regex metacharacters for safety
                let escaped: String = term
                    .chars()
                    .flat_map(|c| match c {
                        '.' | '*' | '+' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}'
                        | '|' | '\\' => vec!['\\', c],
                        _ => vec![c],
                    })
                    .collect();
                let prefix_pattern = format!("{escaped}.*");
                if let Ok(regex_query) =
                    RegexQuery::from_pattern(&prefix_pattern, ready.fields.title_text)
                {
                    subqueries.push((Occur::Should, Box::new(regex_query)));
                }
                // Also try prefix on tags
                if let Ok(regex_query) =
                    RegexQuery::from_pattern(&prefix_pattern, ready.fields.tags_text)
                {
                    subqueries.push((Occur::Should, Box::new(regex_query)));
                }
            } else {
                // Title exact match
                let title_term = Term::from_field_text(ready.fields.title_text, term);
                subqueries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(title_term.clone(), IndexRecordOption::Basic)),
                ));

                // Title fuzzy match (1 edit distance)
                let fuzzy_title = FuzzyTermQuery::new(title_term, 1, true);
                subqueries.push((Occur::Should, Box::new(fuzzy_title)));

                // Description fuzzy match
                let desc_term = Term::from_field_text(ready.fields.description_text, term);
                let fuzzy_desc = FuzzyTermQuery::new(desc_term, 1, true);
                subqueries.push((Occur::Should, Box::new(fuzzy_desc)));

                // Tags exact match
                let tags_term = Term::from_field_text(ready.fields.tags_text, term);
                subqueries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(tags_term, IndexRecordOption::Basic)),
                ));
            }
        }

        let query = BooleanQuery::new(subqueries);

        // Search for more results than needed to allow grouping by type
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit * 4))
            .map_err(|e| SearchError::Query(format!("Search failed: {e}")))?;

        // Collect and group results
        let mut products = Vec::new();
        let mut collections = Vec::new();
        let mut pages = Vec::new();
        let mut articles = Vec::new();

        for (score, doc_address) in top_docs {
            let doc = searcher
                .doc::<tantivy::TantivyDocument>(doc_address)
                .map_err(|e| SearchError::Query(format!("Failed to retrieve doc: {e}")))?;

            let result = Self::doc_to_result(&ready.fields, &doc, score)?;

            match result.doc_type {
                DocType::Product if products.len() < limit => products.push(result),
                DocType::Collection if collections.len() < limit => collections.push(result),
                DocType::Page if pages.len() < limit => pages.push(result),
                DocType::Article if articles.len() < limit => articles.push(result),
                _ => {}
            }
        }

        Ok(SearchResults {
            products,
            collections,
            pages,
            articles,
            query: query_str,
            total_count: 0,
            in_stock_count: 0,
            out_of_stock_count: 0,
            min_price_cents: 0,
            max_price_cents: 0,
        })
    }

    /// Convert a Tantivy document to a search result.
    fn doc_to_result(
        fields: &SearchFields,
        doc: &tantivy::TantivyDocument,
        score: f32,
    ) -> Result<SearchResult, SearchError> {
        let get_text = |field: Field| -> String {
            doc.get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let get_u64 =
            |field: Field| -> Option<u64> { doc.get_first(field).and_then(|v| v.as_u64()) };

        let doc_type_str = get_text(fields.doc_type);
        let doc_type = DocType::parse(&doc_type_str)
            .ok_or_else(|| SearchError::Query(format!("Invalid doc_type: {doc_type_str}")))?;

        let image_url = get_text(fields.image_url);
        let price = get_text(fields.price);
        let price_cents = get_u64(fields.price_cents);
        let available = get_u64(fields.available).is_some_and(|v| v == 1);

        Ok(SearchResult {
            doc_type,
            handle: get_text(fields.handle),
            title: get_text(fields.title),
            description: get_text(fields.description),
            image_url: if image_url.is_empty() {
                None
            } else {
                Some(image_url)
            },
            price: if price.is_empty() { None } else { Some(price) },
            price_cents,
            available,
            score,
        })
    }

    /// Get the number of documents in the index, or 0 if not ready.
    #[must_use]
    pub fn num_docs(&self) -> u64 {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|r| r.reader.searcher().num_docs()))
            .unwrap_or(0)
    }
}

/// Search filters.
#[derive(Debug, Default, Clone)]
pub struct SearchFilters {
    /// Filter by availability (Some(true) = in stock only, Some(false) = out of stock only)
    pub available: Option<bool>,
    /// Minimum price in cents (inclusive)
    pub min_price_cents: Option<u64>,
    /// Maximum price in cents (inclusive)
    pub max_price_cents: Option<u64>,
}

/// Search sort order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SearchSort {
    #[default]
    Relevance,
    PriceAsc,
    PriceDesc,
}

impl SearchSort {
    /// Parse from URL parameter value.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "price-ascending" | "price_asc" => Self::PriceAsc,
            "price-descending" | "price_desc" => Self::PriceDesc,
            _ => Self::Relevance,
        }
    }

    /// Convert to URL parameter value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::PriceAsc => "price-ascending",
            Self::PriceDesc => "price-descending",
        }
    }
}

/// Grouped search results.
#[derive(Debug, Default)]
pub struct SearchResults {
    pub products: Vec<SearchResult>,
    pub collections: Vec<SearchResult>,
    pub pages: Vec<SearchResult>,
    pub articles: Vec<SearchResult>,
    pub query: String,
    /// Total number of matching products (before limit)
    pub total_count: usize,
    /// Number of in-stock products matching query
    pub in_stock_count: usize,
    /// Number of out-of-stock products matching query
    pub out_of_stock_count: usize,
    /// Minimum price in cents across all matching products
    pub min_price_cents: u64,
    /// Maximum price in cents across all matching products
    pub max_price_cents: u64,
}

impl SearchResults {
    /// Check if there are any results.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.products.is_empty()
            && self.collections.is_empty()
            && self.pages.is_empty()
            && self.articles.is_empty()
    }

    /// Get the total number of results.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.products.len() + self.collections.len() + self.pages.len() + self.articles.len()
    }
}

/// Search errors.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Index error: {0}")]
    Index(String),
    #[error("Query error: {0}")]
    Query(String),
    #[error("Build error: {0}")]
    Build(String),
}
