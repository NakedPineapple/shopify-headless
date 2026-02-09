//! Tool execution context trait.
//!
//! Defines external capabilities needed by the support chat service for
//! tool execution and RAG content injection. Implemented by the storefront
//! crate to provide access to Shopify APIs and the content search index.
//!
//! All methods return pre-formatted `String` results suitable for injection
//! into the Claude conversation. This keeps the trait boundary minimal with
//! no storefront-specific types crossing the crate boundary.

use std::future::Future;
use std::pin::Pin;

/// External capabilities for support tool execution.
///
/// Implemented by the storefront crate to provide access to the Tantivy
/// search index, Shopify Storefront API (products), and Shopify Customer
/// Account API (orders, subscriptions).
pub trait ToolContext: Send + Sync {
    /// Keyword-search static pages and blog articles.
    ///
    /// Returns formatted text snippets from matching content, or an empty
    /// string if no results are found. Used for both pre-LLM RAG injection
    /// and the `lookup_faq` tool.
    fn search_content(&self, query: &str, limit: usize) -> String;

    /// Look up a product by name or search query.
    ///
    /// Returns formatted product information (title, price, availability,
    /// description) or a user-friendly error message.
    fn lookup_product<'a>(
        &'a self,
        query: &'a str,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>>;

    /// Look up an order by order number (e.g., "#1001" or "1001").
    ///
    /// Returns formatted order details (status, items, total) or a
    /// user-friendly error message. The customer access token is held
    /// internally by the implementation.
    fn lookup_order<'a>(
        &'a self,
        order_number: &'a str,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>>;

    /// Look up subscription(s) by ID, or `"all"` to list all.
    ///
    /// Returns formatted subscription details (status, frequency, items,
    /// next billing date) or a user-friendly error message.
    fn lookup_subscription<'a>(
        &'a self,
        subscription_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>>;
}
