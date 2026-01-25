//! Content management for markdown-based pages and blog posts.
//!
//! This module loads markdown files from the `/content` directory at startup,
//! parses frontmatter metadata, and renders markdown to HTML.

use chrono::NaiveDate;
use comrak::{Options, markdown_to_html};
use gray_matter::{Matter, ParsedEntity, engine::YAML};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Metadata for static pages (terms, privacy, etc.)
#[derive(Debug, Clone, Deserialize)]
pub struct PageMeta {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub updated_at: Option<NaiveDate>,
}

/// Metadata for blog posts
#[derive(Debug, Clone, Deserialize)]
pub struct PostMeta {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    pub published_at: NaiveDate,
    #[serde(default)]
    pub updated_at: Option<NaiveDate>,
    #[serde(default)]
    pub featured_image: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub draft: bool,
}

/// A rendered page with metadata and HTML content
#[derive(Debug, Clone)]
pub struct Page {
    pub slug: String,
    pub meta: PageMeta,
    pub content_html: String,
}

/// A rendered blog post with metadata and HTML content
#[derive(Debug, Clone)]
pub struct Post {
    pub slug: String,
    pub meta: PostMeta,
    pub content_html: String,
    pub reading_time_minutes: u32,
}

/// Content store that holds all loaded content in memory
#[derive(Debug, Clone)]
pub struct ContentStore {
    pages: Arc<HashMap<String, Page>>,
    posts: Arc<Vec<Post>>,
}

impl ContentStore {
    /// Load all content from the filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error if the content directory cannot be read.
    pub fn load(content_dir: &Path) -> Result<Self, ContentError> {
        let pages = Self::load_pages(&content_dir.join("pages"))?;
        let posts = Self::load_posts(&content_dir.join("blog"))?;

        Ok(Self {
            pages: Arc::new(pages),
            posts: Arc::new(posts),
        })
    }

    /// Load all pages from the pages directory
    fn load_pages(dir: &Path) -> Result<HashMap<String, Page>, ContentError> {
        let mut pages = HashMap::new();

        if !dir.exists() {
            tracing::warn!("Pages directory does not exist: {:?}", dir);
            return Ok(pages);
        }

        let entries = std::fs::read_dir(dir).map_err(|e| ContentError::Io(e.to_string()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                match Self::load_page(&path) {
                    Ok(page) => {
                        tracing::info!("Loaded page: {}", page.slug);
                        pages.insert(page.slug.clone(), page);
                    }
                    Err(e) => {
                        tracing::error!("Failed to load page {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(pages)
    }

    /// Load a single page from a markdown file
    fn load_page(path: &Path) -> Result<Page, ContentError> {
        let content = std::fs::read_to_string(path).map_err(|e| ContentError::Io(e.to_string()))?;

        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ContentError::Parse("Invalid filename".to_string()))?
            .to_string();

        let matter = Matter::<YAML>::new();
        let parsed: ParsedEntity<PageMeta> = matter
            .parse(&content)
            .map_err(|e| ContentError::Parse(format!("Failed to parse frontmatter: {e}")))?;
        let meta = parsed
            .data
            .ok_or_else(|| ContentError::Parse("Missing frontmatter".to_string()))?;

        let content_html = render_markdown(&parsed.content);

        Ok(Page {
            slug,
            meta,
            content_html,
        })
    }

    /// Load all blog posts from the blog directory
    fn load_posts(dir: &Path) -> Result<Vec<Post>, ContentError> {
        let mut posts = Vec::new();

        if !dir.exists() {
            tracing::info!("Blog directory does not exist yet: {:?}", dir);
            return Ok(posts);
        }

        let entries = std::fs::read_dir(dir).map_err(|e| ContentError::Io(e.to_string()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                match Self::load_post(&path) {
                    Ok(post) => {
                        tracing::info!("Loaded post: {}", post.slug);
                        posts.push(post);
                    }
                    Err(e) => {
                        tracing::error!("Failed to load post {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort posts by published date (newest first)
        posts.sort_by(|a, b| b.meta.published_at.cmp(&a.meta.published_at));

        Ok(posts)
    }

    /// Load a single blog post from a markdown file
    fn load_post(path: &Path) -> Result<Post, ContentError> {
        let content = std::fs::read_to_string(path).map_err(|e| ContentError::Io(e.to_string()))?;

        // Extract slug from filename (e.g., "2025-01-15-my-post.md" -> "my-post")
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ContentError::Parse("Invalid filename".to_string()))?;

        // Remove date prefix if present (YYYY-MM-DD-)
        let slug = if filename.len() > 11 && filename.chars().nth(4) == Some('-') {
            filename[11..].to_string()
        } else {
            filename.to_string()
        };

        let matter = Matter::<YAML>::new();
        let parsed: ParsedEntity<PostMeta> = matter
            .parse(&content)
            .map_err(|e| ContentError::Parse(format!("Failed to parse frontmatter: {e}")))?;
        let meta = parsed
            .data
            .ok_or_else(|| ContentError::Parse("Missing frontmatter".to_string()))?;

        let content_html = render_markdown(&parsed.content);

        // Estimate reading time (average 200 words per minute)
        let word_count = parsed.content.split_whitespace().count();
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let reading_time_minutes = ((word_count as f32) / 200.0).ceil() as u32;

        Ok(Post {
            slug,
            meta,
            content_html,
            reading_time_minutes: reading_time_minutes.max(1),
        })
    }

    /// Get a page by slug
    #[must_use]
    pub fn get_page(&self, slug: &str) -> Option<&Page> {
        self.pages.get(slug)
    }

    /// Get all pages
    pub fn get_all_pages(&self) -> impl Iterator<Item = &Page> {
        self.pages.values()
    }

    /// Get a blog post by slug
    #[must_use]
    pub fn get_post(&self, slug: &str) -> Option<&Post> {
        self.posts.iter().find(|p| p.slug == slug)
    }

    /// Get all published blog posts (excludes drafts)
    pub fn get_published_posts(&self) -> impl Iterator<Item = &Post> {
        self.posts.iter().filter(|p| !p.meta.draft)
    }

    /// Get posts by tag
    pub fn get_posts_by_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Post> {
        let tag_lower = tag.to_lowercase();
        self.posts.iter().filter(move |p| {
            !p.meta.draft && p.meta.tags.iter().any(|t| t.to_lowercase() == tag_lower)
        })
    }

    /// Get all unique tags from published posts
    #[must_use]
    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .posts
            .iter()
            .filter(|p| !p.meta.draft)
            .flat_map(|p| p.meta.tags.clone())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Get recent published posts, optionally excluding a specific slug
    #[must_use]
    pub fn get_recent_posts(&self, limit: usize, exclude_slug: Option<&str>) -> Vec<&Post> {
        self.posts
            .iter()
            .filter(|p| !p.meta.draft && exclude_slug.is_none_or(|s| p.slug != s))
            .take(limit)
            .collect()
    }
}

/// Render markdown to HTML with GitHub Flavored Markdown support
fn render_markdown(content: &str) -> String {
    let mut options = Options::default();

    // Enable GFM extensions
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.superscript = true;
    options.extension.header_ids = Some(String::new());
    options.extension.footnotes = true;

    // Render options
    options.render.r#unsafe = true; // Allow raw HTML in markdown

    markdown_to_html(content, &options)
}

/// Content loading errors
#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
}
