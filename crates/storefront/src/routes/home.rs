//! Home page route handler.

use askama::Template;
use askama_web::WebTemplate;
use axum::{extract::State, response::IntoResponse};
use tracing::instrument;

use crate::config::AnalyticsConfig;
use crate::filters;
use crate::shopify::types::{Money, Product as ShopifyProduct};
use crate::state::AppState;

// =============================================================================
// Hero Configuration (Static content for carousel)
// =============================================================================

/// Position for hero slide CTA button.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum ButtonPosition {
    #[default]
    Center,
    BottomLeft,
    BottomRight,
    BottomCenter,
}

/// Hero layout style.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum HeroLayout {
    #[default]
    Carousel,
    SplitLeft,
    SplitRight,
    Centered,
    FullBleed,
}

/// A single slide in the hero carousel.
#[derive(Clone)]
pub struct HeroSlide {
    pub eyebrow: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub button_text: Option<String>,
    pub button_url: Option<String>,
    pub image_path: String,
    pub image_alt: String,
    pub button_position: ButtonPosition,
    /// Ken Burns zoom start scale (e.g., "1.2")
    pub zoom_from: Option<String>,
    /// Ken Burns zoom end scale (e.g., "1.0")
    pub zoom_to: Option<String>,
    /// Ken Burns pan start position (e.g., "center top")
    pub pan_from: Option<String>,
    /// Ken Burns pan end position (e.g., "center center")
    pub pan_to: Option<String>,
}

/// Hero carousel configuration.
#[derive(Clone)]
pub struct HeroConfig {
    pub layout: HeroLayout,
    pub slides: Vec<HeroSlide>,
    pub autoplay_ms: Option<u32>,
}

impl Default for HeroConfig {
    fn default() -> Self {
        Self {
            layout: HeroLayout::Carousel,
            slides: vec![
                // Slide 1: Self-love hero - bottom-left button only
                HeroSlide {
                    eyebrow: None,
                    title: None,
                    subtitle: None,
                    button_text: Some("Explore Our Line".to_string()),
                    button_url: Some("/collections/frontpage".to_string()),
                    image_path: "/static/images/original/hero/hero-self-love.png".to_string(),
                    image_alt: "Explore our skincare line".to_string(),
                    button_position: ButtonPosition::BottomLeft,
                    zoom_from: None,
                    zoom_to: None,
                    pan_from: None,
                    pan_to: None,
                },
                // Slide 2: Glow better hero - bottom-left button only
                HeroSlide {
                    eyebrow: None,
                    title: None,
                    subtitle: None,
                    button_text: Some("Join the Model Program".to_string()),
                    button_url: Some("/pages/model-program".to_string()),
                    image_path: "/static/images/original/hero/hero-glow-better.png".to_string(),
                    image_alt: "Join the Model Program".to_string(),
                    button_position: ButtonPosition::BottomLeft,
                    zoom_from: None,
                    zoom_to: None,
                    pan_from: None,
                    pan_to: None,
                },
                // Slide 3: Tennis hero - centered with Ken Burns effect
                HeroSlide {
                    eyebrow: None,
                    title: Some("Confidence Looks Good on You".to_string()),
                    subtitle: Some(
                        "Clean beauty that works hard and plays harder. Your glow-up just got tropical.".to_string(),
                    ),
                    button_text: Some("Get the Glow".to_string()),
                    button_url: Some("/products/glow-up-bronzing-facial-oil".to_string()),
                    image_path: "/static/images/original/hero/hero-tennis.png".to_string(),
                    image_alt: "Confidence looks good on you".to_string(),
                    button_position: ButtonPosition::Center,
                    zoom_from: Some("1.75".to_string()),
                    zoom_to: Some("1.5".to_string()),
                    pan_from: Some("-15%, 10%".to_string()),
                    pan_to: Some("-5%, 3%".to_string()),
                },
                // Slide 4: Pickleball hero - centered, title only
                HeroSlide {
                    eyebrow: None,
                    title: Some(
                        "We celebrate authenticity, adventure and self-love in every drop."
                            .to_string(),
                    ),
                    subtitle: None,
                    button_text: Some("Shop Products".to_string()),
                    button_url: Some("/collections/frontpage".to_string()),
                    image_path: "/static/images/original/hero/hero-pickleball.png".to_string(),
                    image_alt: "Authenticity and self-love".to_string(),
                    button_position: ButtonPosition::Center,
                    zoom_from: None,
                    zoom_to: None,
                    pan_from: None,
                    pan_to: None,
                },
                // Slide 5: Holding product hero - bottom-center with full content
                HeroSlide {
                    eyebrow: None,
                    title: Some("Clean Skin Starts Here".to_string()),
                    subtitle: Some(
                        "Derived from nature, our skincare line delivers the ultimate pineapple exfoliation experience. Powered by bromelain, a natural enzyme found in pineapple - it gently combats aging, clears acne and promotes healing for radiant & rejuvenated skin.".to_string(),
                    ),
                    button_text: Some("Shop Essentials".to_string()),
                    button_url: Some("/products/naked-pineapple-vip-bundle".to_string()),
                    image_path: "/static/images/original/hero/hero-holding-product.png".to_string(),
                    image_alt: "Clean skin starts here".to_string(),
                    button_position: ButtonPosition::BottomCenter,
                    zoom_from: None,
                    zoom_to: None,
                    pan_from: None,
                    pan_to: None,
                },
            ],
            autoplay_ms: Some(5000),
        }
    }
}

// =============================================================================
// Review Data
// =============================================================================

/// A customer review for display on the homepage.
#[derive(Clone)]
pub struct ReviewView {
    pub reviewer_name: String,
    pub rating: i64,
    pub content: String,
    pub product_title: String,
    pub product_handle: String,
    pub product_image_path: Option<String>,
}

/// Static reviews for the homepage (can be replaced with dynamic data later).
fn get_featured_reviews() -> Vec<ReviewView> {
    vec![
        ReviewView {
            reviewer_name: "David".to_string(),
            rating: 5,
            content: "I have loved this serum. It leaves my face feeling tight but not dry. I have noticed a nice healthy glow when I combine this with the exotic nourishing cream.".to_string(),
            product_title: "Bright & Tight Super Serum".to_string(),
            product_handle: "bright-tight-super-serum".to_string(),
            product_image_path: Some("/static/images/original/products/bright-tight-super-serum/NP_SuperSerum_SET.png".to_string()),
        },
        ReviewView {
            reviewer_name: "Amanda".to_string(),
            rating: 5,
            content: "It gently exfoliates my skin without feeling harsh and leaves my face feeling refreshed, smooth, and incredibly clean. The scent is a light, tropical pineapple that makes my skincare routine feel like a mini spa experience.".to_string(),
            product_title: "Pineapple Exfoliating Gel Cleanser".to_string(),
            product_handle: "pineapple-enzyme-cleanser".to_string(),
            product_image_path: Some("/static/images/original/products/pineapple-enzyme-cleanser/NP_Cleanser_SET.png".to_string()),
        },
        ReviewView {
            reviewer_name: "Shenae".to_string(),
            rating: 5,
            content: "Dear god my skin is looking flawless!".to_string(),
            product_title: "Exotic Nourishing Cream".to_string(),
            product_handle: "skin-tight-exotic-cream".to_string(),
            product_image_path: Some("/static/images/original/products/skin-tight-exotic-cream/NP_ExoticCream_SET.png".to_string()),
        },
        ReviewView {
            reviewer_name: "Brie".to_string(),
            rating: 5,
            content: "This bronzing elixir gives my skin a gorgeous, sun-kissed glow while keeping it soft and hydrated. The texture is lightweight, and the subtle scent is delightful! This product is now a must-have in my daily routine for that perfect, healthy glow.".to_string(),
            product_title: "Hydrating Glow Up Bronzing Elixir".to_string(),
            product_handle: "glow-up-bronzing-facial-oil".to_string(),
            product_image_path: Some("/static/images/original/products/glow-up-bronzing-facial-oil/NP_BronzingOil_SET.png".to_string()),
        },
        ReviewView {
            reviewer_name: "Tyler".to_string(),
            rating: 5,
            content: "I love all the Naked Pineapple products! My skin breaks out easily but it has been so much better since using only Naked Pineapple products. So happy I can feel confident in my glowing skin!".to_string(),
            product_title: "Facial Repair Tropical Oil".to_string(),
            product_handle: "pineapple-facial-oil".to_string(),
            product_image_path: Some("/static/images/original/products/pineapple-facial-oil/NP_TropicalOil_SET.png".to_string()),
        },
    ]
}

// =============================================================================
// Product and Image Views
// =============================================================================

/// Product display data for templates.
#[derive(Clone)]
pub struct ProductView {
    pub handle: String,
    pub title: String,
    pub price: String,
    pub compare_at_price: Option<String>,
    pub featured_image: Option<ImageView>,
    pub hover_image: Option<ImageView>,
    pub product_type: Option<String>,
}

/// Image display data for templates.
#[derive(Clone)]
pub struct ImageView {
    pub url: String,
    pub alt: String,
}

// =============================================================================
// Type Conversions
// =============================================================================

/// Format a Shopify Money type as a price string.
fn format_price(money: &Money) -> String {
    money.amount.parse::<f64>().map_or_else(
        |_| format!("${}", money.amount),
        |amount| format!("${amount:.2}"),
    )
}

impl From<&ShopifyProduct> for ProductView {
    fn from(product: &ShopifyProduct) -> Self {
        // Get featured image (first image)
        let featured_image = product.featured_image.as_ref().map(|img| ImageView {
            url: img.url.clone(),
            alt: img.alt_text.clone().unwrap_or_default(),
        });

        // Get hover image (second image if available)
        let hover_image = product.images.get(1).map(|img| ImageView {
            url: img.url.clone(),
            alt: img.alt_text.clone().unwrap_or_default(),
        });

        // Product type (called `kind` in Shopify types)
        let product_type = if product.kind.is_empty() {
            None
        } else {
            Some(product.kind.clone())
        };

        Self {
            handle: product.handle.clone(),
            title: product.title.clone(),
            price: format_price(&product.price_range.min_variant_price),
            compare_at_price: product
                .compare_at_price_range
                .as_ref()
                .filter(|r| r.min_variant_price.amount != "0.0")
                .map(|r| format_price(&r.min_variant_price)),
            featured_image,
            hover_image,
            product_type,
        }
    }
}

/// Home page template.
#[derive(Template, WebTemplate)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    /// Hero carousel configuration.
    pub hero: HeroConfig,
    /// Skincare products for the tabbed grid.
    pub skincare_products: Vec<ProductView>,
    /// Merch products for the tabbed grid.
    pub merch_products: Vec<ProductView>,
    /// Featured customer reviews.
    pub featured_reviews: Vec<ReviewView>,
    /// Analytics tracking configuration.
    pub analytics: AnalyticsConfig,
    /// CSP nonce for inline scripts.
    pub nonce: String,
    /// Base URL for canonical links and structured data.
    pub base_url: String,
    /// Logo URL for Organization schema.
    pub logo_url: String,
}

/// Number of products to show per collection tab.
const PRODUCTS_PER_COLLECTION: i64 = 8;

/// Collection handles for the tabbed product grid.
const SKINCARE_COLLECTION: &str = "frontpage";
const MERCH_COLLECTION: &str = "merch";

/// Display the home page.
#[instrument(skip(state, nonce))]
pub async fn home(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> impl IntoResponse {
    // Fetch skincare products from collection
    let skincare_products = state
        .storefront()
        .get_collection_by_handle(
            SKINCARE_COLLECTION,
            Some(PRODUCTS_PER_COLLECTION),
            None,
            None,
            None,
            None,
        )
        .await
        .map_or_else(
            |e| {
                tracing::error!("Failed to fetch skincare collection: {e}");
                Vec::new()
            },
            |collection| collection.products.iter().map(ProductView::from).collect(),
        );

    // Fetch merch products from collection
    let merch_products = state
        .storefront()
        .get_collection_by_handle(
            MERCH_COLLECTION,
            Some(PRODUCTS_PER_COLLECTION),
            None,
            None,
            None,
            None,
        )
        .await
        .map_or_else(
            |e| {
                tracing::error!("Failed to fetch merch collection: {e}");
                Vec::new()
            },
            |collection| collection.products.iter().map(ProductView::from).collect(),
        );

    let base_url = state.config().base_url.clone();
    let logo_url = crate::filters::get_logo_url(&base_url);

    HomeTemplate {
        hero: HeroConfig::default(),
        skincare_products,
        merch_products,
        featured_reviews: get_featured_reviews(),
        analytics: state.config().analytics.clone(),
        nonce,
        base_url,
        logo_url,
    }
}
