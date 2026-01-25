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

use crate::content::Post;
use crate::filters;
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
}

/// Blog post detail template.
#[derive(Template, WebTemplate)]
#[template(path = "blog/show.html")]
pub struct BlogShowTemplate {
    pub post: PostView,
    pub recent_posts: Vec<PostView>,
}

/// Number of recent posts to show in sidebar.
const RECENT_POSTS_COUNT: usize = 3;

/// Display the blog index page with all published posts.
#[instrument(skip(state))]
pub async fn index(State(state): State<AppState>) -> impl IntoResponse {
    let posts: Vec<PostView> = state
        .content()
        .get_published_posts()
        .map(PostView::from)
        .collect();
    BlogIndexTemplate { posts }
}

/// Display a single blog post by slug.
///
/// # Errors
///
/// Returns 404 if the post doesn't exist or is a draft.
#[instrument(skip(state))]
pub async fn show(
    State(state): State<AppState>,
    Path(slug): Path<String>,
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

    Ok(BlogShowTemplate {
        post: PostView::from(post),
        recent_posts,
    })
}

/// Create the blog routes router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/{slug}", get(show))
}
