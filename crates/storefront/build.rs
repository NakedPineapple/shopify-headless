//! Build script for storefront crate.
//!
//! Generates content-based hashes for static assets (CSS) to enable
//! immutable CDN caching.

use std::env;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

fn main() {
    hash_css();
}

/// Hash main.css and copy to derived directory with hash in filename.
///
/// Sets `CSS_HASH` environment variable for use with `env!("CSS_HASH")`.
fn hash_css() {
    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by Cargo");
    let css_path = Path::new(&manifest_dir).join("static/css/main.css");

    // Tell Cargo to rerun if main.css changes
    println!("cargo:rerun-if-changed={}", css_path.display());

    // Read CSS content
    let content = match fs::read(&css_path) {
        Ok(content) => content,
        Err(e) => {
            // CSS might not exist yet during initial build
            println!("cargo:warning=Could not read main.css: {e}");
            println!("cargo:rustc-env=CSS_HASH=");
            return;
        }
    };

    // Compute hash (first 8 chars of SHA256)
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let hash = format!("{:x}", hasher.finalize());
    let short_hash = &hash[..8];

    // Set environment variable for compile-time access
    println!("cargo:rustc-env=CSS_HASH={short_hash}");

    // Copy CSS to derived directory with hash in filename
    let derived_dir = Path::new(&manifest_dir).join("static/css/derived");
    fs::create_dir_all(&derived_dir).expect("Failed to create derived CSS directory");

    let derived_path = derived_dir.join(format!("main.{short_hash}.css"));
    fs::copy(&css_path, &derived_path).expect("Failed to copy CSS to derived directory");

    println!("cargo:warning=CSS hash: {short_hash}");
}
