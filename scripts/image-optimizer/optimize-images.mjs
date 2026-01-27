#!/usr/bin/env node
/**
 * Image Optimization Script
 *
 * Discovers images used in templates and Rust source files, then generates
 * responsive variants in multiple formats (AVIF, WebP, JPEG) with content-based
 * hashing for immutable CDN caching.
 *
 * Usage:
 *   node optimize-images.mjs                    # Optimize all referenced images
 *   node optimize-images.mjs path/to/image.jpg  # Optimize a specific image
 *
 * The path should be relative to static/images/original/
 * Example: node optimize-images.mjs lifestyle/DSC_2634.jpg
 *
 * Output:
 *   - crates/storefront/static/images/derived/ (optimized images)
 *   - crates/storefront/src/image_manifest.rs (Rust manifest for hash lookups)
 */

import { readFile, writeFile, mkdir, copyFile, readdir, stat } from "node:fs/promises";
import { createHash } from "node:crypto";
import { dirname, join, basename, extname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import fg from "fast-glob";
import sharp from "sharp";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, "..", "..");
const STOREFRONT_ROOT = join(PROJECT_ROOT, "crates", "storefront");
const IMAGES_ROOT = join(STOREFRONT_ROOT, "static", "images");
const ORIGINAL_DIR = join(IMAGES_ROOT, "original");
const DERIVED_DIR = join(IMAGES_ROOT, "derived");
const MANIFEST_PATH = join(STOREFRONT_ROOT, "src", "image_manifest.rs");

// Responsive sizes to generate (width in pixels)
const SIZES = [320, 640, 1024, 1600, 2400];

// Quality settings per format
const QUALITY = {
  avif: 60,
  webp: 80,
  jpeg: 85,
};

// Raster image extensions to process
const RASTER_EXTENSIONS = new Set([".jpg", ".jpeg", ".png", ".webp"]);

// SVG extension (copy as-is)
const SVG_EXTENSION = ".svg";

/**
 * Generate a short content hash from file contents
 */
async function getContentHash(filePath) {
  const content = await readFile(filePath);
  return createHash("sha256").update(content).digest("hex").slice(0, 8);
}

/**
 * Discover images referenced in template and source files
 */
async function discoverUsedImages() {
  const usedImages = new Set();

  // Pattern to match /static/images/original/ paths
  const imagePathRegex = /\/static\/images\/original\/([^"'\s)]+\.(jpg|jpeg|png|webp|svg))/gi;

  // Pattern to match filter-based references like "path/to/image"|image_hash
  const filterPathRegex = /"([^"]+)"\s*\|\s*image_hash/g;

  // Scan HTML templates
  const templateFiles = await fg("crates/storefront/templates/**/*.html", {
    cwd: PROJECT_ROOT,
    absolute: true,
  });

  // Scan Rust source files
  const rustFiles = await fg("crates/storefront/src/**/*.rs", {
    cwd: PROJECT_ROOT,
    absolute: true,
  });

  const allFiles = [...templateFiles, ...rustFiles];

  for (const file of allFiles) {
    const content = await readFile(file, "utf-8");
    let match;

    // Find /static/images/original/ paths
    while ((match = imagePathRegex.exec(content)) !== null) {
      // match[1] is the path after /static/images/
      const imagePath = match[1];

      // Skip Shopify CDN URLs (they contain cdn.shopify.com)
      if (imagePath.includes("cdn.shopify.com")) {
        continue;
      }

      // Skip template variables (contain {{ or {%)
      if (imagePath.includes("{{") || imagePath.includes("{%")) {
        continue;
      }

      usedImages.add(imagePath);
    }

    // Find filter-based references (base path without extension)
    while ((match = filterPathRegex.exec(content)) !== null) {
      const basePath = match[1];

      // Skip if it looks like a full path (already handled above)
      if (basePath.startsWith("/")) {
        continue;
      }

      // Try to find the actual file with common extensions
      for (const ext of [".svg", ".jpg", ".jpeg", ".png", ".webp"]) {
        const fullPath = join(ORIGINAL_DIR, basePath + ext);
        try {
          await stat(fullPath);
          usedImages.add(basePath + ext);
          break;
        } catch {
          // File doesn't exist with this extension, try next
        }
      }
    }
  }

  return usedImages;
}

/**
 * Get all images in a directory recursively
 */
async function getAllImages(dir) {
  const images = [];

  async function scan(currentDir) {
    try {
      const entries = await readdir(currentDir, { withFileTypes: true });

      for (const entry of entries) {
        const fullPath = join(currentDir, entry.name);

        if (entry.isDirectory()) {
          await scan(fullPath);
        } else if (entry.isFile()) {
          const ext = extname(entry.name).toLowerCase();
          if (RASTER_EXTENSIONS.has(ext) || ext === SVG_EXTENSION) {
            images.push(fullPath);
          }
        }
      }
    } catch {
      // Directory doesn't exist, skip
    }
  }

  await scan(dir);
  return images;
}

/**
 * Process a single raster image into multiple sizes and formats with hashed filenames
 * Returns { files: string[], maxWidth: number }
 */
async function processRasterImage(inputPath, outputDir, relativePath, hash) {
  const ext = extname(relativePath).toLowerCase();
  const nameWithoutExt = relativePath.slice(0, -ext.length);

  // Get original image metadata
  let image;
  let metadata;
  try {
    image = sharp(inputPath);
    metadata = await image.metadata();
  } catch (err) {
    console.log(`      ⚠️  Skipping (unsupported format): ${err.message}`);
    return { files: [], maxWidth: 0 };
  }
  const originalWidth = metadata.width || 0;

  // Create output directory
  const outputSubDir = join(outputDir, dirname(relativePath));
  await mkdir(outputSubDir, { recursive: true });

  const files = [];
  let maxGeneratedWidth = 0;

  // Generate each size variant (capped at original size)
  for (const targetWidth of SIZES) {
    if (targetWidth > originalWidth) {
      continue; // Skip sizes larger than original
    }

    maxGeneratedWidth = targetWidth;

    const resized = sharp(inputPath).resize(targetWidth, null, {
      withoutEnlargement: true,
      fit: "inside",
    });

    // Generate AVIF with hash: image.{hash}-{size}.avif
    const avifPath = join(outputDir, `${nameWithoutExt}.${hash}-${targetWidth}.avif`);
    await mkdir(dirname(avifPath), { recursive: true });
    await resized.clone().avif({ quality: QUALITY.avif }).toFile(avifPath);
    files.push(avifPath);

    // Generate WebP with hash
    const webpPath = join(outputDir, `${nameWithoutExt}.${hash}-${targetWidth}.webp`);
    await resized.clone().webp({ quality: QUALITY.webp }).toFile(webpPath);
    files.push(webpPath);

    // Generate JPEG with hash
    const jpegPath = join(outputDir, `${nameWithoutExt}.${hash}-${targetWidth}.jpg`);
    await resized.clone().jpeg({ quality: QUALITY.jpeg, progressive: true }).toFile(jpegPath);
    files.push(jpegPath);
  }

  return { files, maxWidth: maxGeneratedWidth };
}

/**
 * Copy SVG file with hash in filename
 */
async function copySvgFile(inputPath, outputDir, relativePath, hash) {
  const ext = extname(relativePath);
  const nameWithoutExt = relativePath.slice(0, -ext.length);
  const outputPath = join(outputDir, `${nameWithoutExt}.${hash}${ext}`);
  await mkdir(dirname(outputPath), { recursive: true });
  await copyFile(inputPath, outputPath);
  return [outputPath];
}

/**
 * Load existing manifest from Rust file
 */
async function loadExistingManifest() {
  try {
    const content = await readFile(MANIFEST_PATH, "utf-8");
    const manifest = {};

    // Parse entries - handles both single-line and multi-line (after rustfmt)
    // Single-line: ("path", ("hash", 1024)),
    // Multi-line:  (\n  "path",\n  ("hash", 1024),\n),
    const entryRegex = /\(\s*"([^"]+)",\s*\("([^"]+)",\s*(\d+)\),?\s*\)/g;
    let match;
    while ((match = entryRegex.exec(content)) !== null) {
      const [, path, hash, maxWidth] = match;
      manifest[path] = { hash, maxWidth: parseInt(maxWidth, 10) };
    }

    // Warn if manifest file exists but no entries were parsed
    const entryCount = Object.keys(manifest).length;
    if (entryCount === 0 && content.includes("HashMap::from")) {
      console.error("⚠️  Warning: Manifest file exists but no entries could be parsed!");
      console.error("   This may indicate a format change. Run full optimization to regenerate.");
      process.exit(1);
    }

    return manifest;
  } catch (err) {
    if (err.code === "ENOENT") {
      // Manifest doesn't exist yet - that's fine
      return {};
    }
    // Other errors should be reported
    console.error(`⚠️  Error reading manifest: ${err.message}`);
    return {};
  }
}

/**
 * Generate Rust manifest file with image hashes and max widths
 *
 * Generates the manifest and runs rustfmt for proper formatting.
 */
async function generateRustManifest(manifest) {
  const entries = Object.entries(manifest)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, { hash, maxWidth }]) => `        ("${path}", ("${hash}", ${maxWidth})),`)
    .join("\n");

  // Note: imports must be in alphabetical order for rustfmt
  const rustCode = `//! Auto-generated image manifest for content-based hashing.
//!
//! DO NOT EDIT - Generated by scripts/image-optimizer/optimize-images.mjs

use std::collections::HashMap;
use std::sync::LazyLock;

/// Image metadata: (hash, \`max_width\`)
/// - hash: 8-character content hash for cache busting
/// - \`max_width\`: largest generated size in pixels (0 for SVGs)
pub type ImageInfo = (&'static str, u32);

/// Maps image base paths to their metadata.
///
/// Key: base path without extension (e.g., \`"lifestyle/DSC_1068"\`)
/// Value: (hash, \`max_width\`)
pub static IMAGE_INFO: LazyLock<HashMap<&'static str, ImageInfo>> = LazyLock::new(|| {
    HashMap::from([
${entries}
    ])
});

/// Look up the content hash for an image path.
///
/// Returns the hash if found, or an empty string if not found.
#[must_use]
pub fn get_image_hash(base_path: &str) -> &'static str {
    IMAGE_INFO.get(base_path).map_or("", |(hash, _)| *hash)
}

/// Look up the maximum generated width for an image path.
///
/// Returns the max width if found, or 0 if not found.
/// SVGs return 0 (they are resolution-independent).
#[must_use]
pub fn get_image_max_width(base_path: &str) -> u32 {
    IMAGE_INFO.get(base_path).map_or(0, |(_, width)| *width)
}
`;

  await writeFile(MANIFEST_PATH, rustCode, "utf-8");
  console.log(`\n📝 Generated Rust manifest: ${MANIFEST_PATH}`);

  // Run rustfmt to ensure proper formatting
  const { execSync } = await import("node:child_process");
  try {
    execSync(`rustfmt ${MANIFEST_PATH}`, { stdio: "inherit" });
    console.log("   ✓ Formatted with rustfmt");
  } catch {
    console.log("   ⚠️  rustfmt not available, skipping formatting");
  }
}

/**
 * Main optimization function
 */
async function optimize() {
  console.log("🍍 Naked Pineapple Image Optimizer (with content hashing)\n");

  // Step 1: Discover used images
  console.log("📋 Discovering images used in templates and source files...");
  const usedImages = await discoverUsedImages();
  console.log(`   Found ${usedImages.size} unique image references\n`);

  if (usedImages.size === 0) {
    console.log("⚠️  No images found in templates. Scanning all images in original/...\n");
    // Fallback: process all images in original/
    const allImages = await getAllImages(ORIGINAL_DIR);
    for (const img of allImages) {
      usedImages.add(relative(ORIGINAL_DIR, img));
    }
    console.log(`   Found ${usedImages.size} images in original/\n`);
  }

  // Step 2: Create derived directory
  await mkdir(DERIVED_DIR, { recursive: true });

  // Step 3: Process each used image and build manifest
  const manifest = {}; // Maps base path (without extension) to { hash, maxWidth }
  let processedCount = 0;
  let skippedCount = 0;
  let totalVariants = 0;

  for (const imagePath of usedImages) {
    const inputPath = join(ORIGINAL_DIR, imagePath);
    const ext = extname(imagePath).toLowerCase();

    // Check if source file exists
    try {
      await stat(inputPath);
    } catch {
      console.log(`   ⚠️  Skipping (not found): ${imagePath}`);
      skippedCount++;
      continue;
    }

    // Generate content hash from source file
    const hash = await getContentHash(inputPath);

    // Store in manifest (base path without extension)
    const basePath = imagePath.slice(0, -ext.length);

    try {
      if (ext === SVG_EXTENSION) {
        // Copy SVG with hash (maxWidth = 0 for SVGs, they're resolution-independent)
        const results = await copySvgFile(inputPath, DERIVED_DIR, imagePath, hash);
        manifest[basePath] = { hash, maxWidth: 0 };
        console.log(`   ✓ Copied SVG: ${imagePath} [${hash}]`);
        totalVariants += results.length;
      } else if (RASTER_EXTENSIONS.has(ext)) {
        // Process raster image with hash
        console.log(`   🖼️  Processing: ${imagePath} [${hash}]`);
        const { files, maxWidth } = await processRasterImage(inputPath, DERIVED_DIR, imagePath, hash);
        if (files.length === 0) {
          skippedCount++;
          continue;
        }
        manifest[basePath] = { hash, maxWidth };
        console.log(`      Generated ${files.length} variants (max: ${maxWidth}px)`);
        totalVariants += files.length;
      } else {
        console.log(`   ⚠️  Skipping (unknown type): ${imagePath}`);
        skippedCount++;
        continue;
      }

      processedCount++;
    } catch (err) {
      console.log(`   ❌ Error processing ${imagePath}: ${err.message}`);
      skippedCount++;
      continue;
    }
  }

  // Step 4: Generate Rust manifest
  await generateRustManifest(manifest);

  // Summary
  console.log("\n✅ Optimization complete!");
  console.log(`   Processed: ${processedCount} images`);
  console.log(`   Skipped: ${skippedCount} images`);
  console.log(`   Generated: ${totalVariants} total variants`);
  console.log(`   Output: ${DERIVED_DIR}`);
}

/**
 * Optimize a single image and update the manifest
 */
async function optimizeSingle(imagePath) {
  console.log("🍍 Naked Pineapple Image Optimizer (single image mode)\n");

  const inputPath = join(ORIGINAL_DIR, imagePath);
  const ext = extname(imagePath).toLowerCase();

  // Check if source file exists
  try {
    await stat(inputPath);
  } catch {
    console.error(`❌ Image not found: ${inputPath}`);
    process.exit(1);
  }

  // Load existing manifest
  console.log("📋 Loading existing manifest...");
  const manifest = await loadExistingManifest();
  console.log(`   Found ${Object.keys(manifest).length} existing entries\n`);

  // Create derived directory
  await mkdir(DERIVED_DIR, { recursive: true });

  // Generate content hash from source file
  const hash = await getContentHash(inputPath);
  const basePath = imagePath.slice(0, -ext.length);

  try {
    if (ext === SVG_EXTENSION) {
      const results = await copySvgFile(inputPath, DERIVED_DIR, imagePath, hash);
      manifest[basePath] = { hash, maxWidth: 0 };
      console.log(`✓ Copied SVG: ${imagePath} [${hash}]`);
      console.log(`  Generated ${results.length} file(s)`);
    } else if (RASTER_EXTENSIONS.has(ext)) {
      console.log(`🖼️  Processing: ${imagePath} [${hash}]`);
      const { files, maxWidth } = await processRasterImage(inputPath, DERIVED_DIR, imagePath, hash);
      if (files.length === 0) {
        console.error(`❌ Failed to process image`);
        process.exit(1);
      }
      manifest[basePath] = { hash, maxWidth };
      console.log(`   Generated ${files.length} variants (max: ${maxWidth}px)`);
    } else {
      console.error(`❌ Unknown image type: ${ext}`);
      process.exit(1);
    }
  } catch (err) {
    console.error(`❌ Error processing ${imagePath}: ${err.message}`);
    process.exit(1);
  }

  // Update manifest
  await generateRustManifest(manifest);

  console.log("\n✅ Done!");
}

// Parse command line arguments and run
const args = process.argv.slice(2);

if (args.length > 0) {
  // Single image mode
  optimizeSingle(args[0]).catch((err) => {
    console.error("❌ Error:", err);
    process.exit(1);
  });
} else {
  // Full optimization mode
  optimize().catch((err) => {
    console.error("❌ Error:", err);
    process.exit(1);
  });
}
