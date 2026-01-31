//! Blog route handlers.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::NaiveDate;
use tracing::instrument;

use crate::config::AnalyticsConfig;
use crate::content::Post;
use crate::filters;
use crate::routes::products::BreadcrumbItem;
use crate::state::AppState;

/// Post view for templates.
#[derive(Clone)]
pub struct PostView {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub published_at: NaiveDate,
    pub featured_image: Option<String>,
    pub tags: Vec<String>,
    pub content_html: String,
    pub reading_time_minutes: u32,
}

impl From<&Post> for PostView {
    fn from(post: &Post) -> Self {
        Self {
            slug: post.slug.clone(),
            title: post.meta.title.clone(),
            description: post.meta.description.clone(),
            author: post.meta.author.clone(),
            published_at: post.meta.published_at,
            featured_image: post.meta.featured_image.clone(),
            tags: post.meta.tags.clone(),
            content_html: post.content_html.clone(),
            reading_time_minutes: post.reading_time_minutes,
        }
    }
}

/// Blog index page template.
#[derive(Template, WebTemplate)]
#[template(path = "blog/index.html")]
pub struct BlogIndexTemplate {
    pub posts: Vec<PostView>,
    pub analytics: AnalyticsConfig,
    pub nonce: String,
    /// Base URL for canonical links.
    pub base_url: String,
}

/// Blog post detail template.
#[derive(Template, WebTemplate)]
#[template(path = "blog/show.html")]
pub struct BlogShowTemplate {
    pub post: PostView,
    pub recent_posts: Vec<PostView>,
    pub analytics: AnalyticsConfig,
    pub nonce: String,
    /// Base URL for canonical links and structured data.
    pub base_url: String,
    /// Logo URL for publisher in Article schema.
    pub logo_url: String,
    /// Breadcrumb trail for SEO.
    pub breadcrumbs: Vec<BreadcrumbItem>,
}

/// Number of recent posts to show in sidebar.
const RECENT_POSTS_COUNT: usize = 3;

/// Display the blog index page with all published posts.
#[instrument(skip(state, nonce))]
pub async fn index(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> impl IntoResponse {
    let posts: Vec<PostView> = state
        .content()
        .get_published_posts()
        .map(PostView::from)
        .collect();
    BlogIndexTemplate {
        posts,
        analytics: state.config().analytics.clone(),
        nonce,
        base_url: state.config().base_url.clone(),
    }
}

/// Display a single blog post by slug.
///
/// # Errors
///
/// Returns 404 if the post doesn't exist or is a draft.
#[instrument(skip(state, nonce))]
pub async fn show(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    let post = state
        .content()
        .get_post(&slug)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Don't show draft posts
    if post.meta.draft {
        return Err(StatusCode::NOT_FOUND);
    }

    let recent_posts: Vec<PostView> = state
        .content()
        .get_recent_posts(RECENT_POSTS_COUNT, Some(&slug))
        .into_iter()
        .map(PostView::from)
        .collect();

    let post_view = PostView::from(post);

    // SEO breadcrumbs
    let breadcrumbs = vec![
        BreadcrumbItem {
            name: "Home".to_string(),
            url: Some("/".to_string()),
        },
        BreadcrumbItem {
            name: "Blog".to_string(),
            url: Some("/blog".to_string()),
        },
        BreadcrumbItem {
            name: post_view.title.clone(),
            url: None,
        },
    ];

    let base_url = state.config().base_url.clone();
    let logo_url = crate::filters::get_logo_url(&base_url);

    Ok(BlogShowTemplate {
        post: post_view,
        recent_posts,
        analytics: state.config().analytics.clone(),
        nonce,
        base_url,
        logo_url,
        breadcrumbs,
    })
}

/// Create the blog routes router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/{slug}", get(show))
}
