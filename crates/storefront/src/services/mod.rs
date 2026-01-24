//! Business logic services for storefront.
//!
//! # Services
//!
//! - `auth` - User authentication (password, `WebAuthn`, OAuth)
//! - `email` - Email sending (verification, password reset)
//! - `cart` - Cart operations (wrapper around Shopify cart)
//! - `analytics` - Analytics event tracking
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! pub mod analytics;
//! pub mod auth;
//! pub mod cart;
//! pub mod email;
//!
//! // auth.rs
//! pub struct AuthService {
//!     pool: PgPool,
//!     webauthn: Webauthn,
//! }
//!
//! impl AuthService {
//!     pub async fn register(
//!         &self,
//!         email: &str,
//!         password: &str,
//!     ) -> Result<User, AuthError> {
//!         // Hash password with argon2
//!         let hash = argon2::hash_encoded(password.as_bytes(), &salt, &config)?;
//!
//!         // Create user
//!         let user = db::users::create(&self.pool, email, &hash).await?;
//!
//!         // Send verification email
//!         self.send_verification_email(&user).await?;
//!
//!         Ok(user)
//!     }
//!
//!     pub async fn login(
//!         &self,
//!         email: &str,
//!         password: &str,
//!     ) -> Result<User, AuthError> {
//!         let user = db::users::get_by_email(&self.pool, email)
//!             .await?
//!             .ok_or(AuthError::InvalidCredentials)?;
//!
//!         if !argon2::verify_encoded(&user.password_hash, password.as_bytes())? {
//!             return Err(AuthError::InvalidCredentials);
//!         }
//!
//!         Ok(user)
//!     }
//!
//!     // WebAuthn
//!     pub async fn start_passkey_registration(&self, user_id: i64) -> Result<CreationChallengeResponse, AuthError> { ... }
//!     pub async fn finish_passkey_registration(&self, user_id: i64, response: RegisterPublicKeyCredential) -> Result<(), AuthError> { ... }
//!     pub async fn start_passkey_authentication(&self, email: &str) -> Result<RequestChallengeResponse, AuthError> { ... }
//!     pub async fn finish_passkey_authentication(&self, response: PublicKeyCredential) -> Result<User, AuthError> { ... }
//! }
//!
//! // analytics.rs
//! pub struct AnalyticsService {
//!     config: AnalyticsConfig,
//! }
//!
//! impl AnalyticsService {
//!     /// Track a page view event.
//!     pub fn page_view(&self, path: &str, title: &str) -> AnalyticsEvents { ... }
//!
//!     /// Track an add-to-cart event.
//!     pub fn add_to_cart(&self, product: &Product, quantity: u32) -> AnalyticsEvents { ... }
//!
//!     /// Track a purchase event.
//!     pub fn purchase(&self, order: &Order) -> AnalyticsEvents { ... }
//! }
//!
//! /// Analytics events to inject into the page.
//! pub struct AnalyticsEvents {
//!     pub ga4: Option<String>,     // GA4 gtag() call
//!     pub meta: Option<String>,    // Meta fbq() call
//!     pub tiktok: Option<String>,  // TikTok ttq.track() call
//! }
//! ```

// TODO: Implement services
