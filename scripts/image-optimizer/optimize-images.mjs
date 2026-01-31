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
 *   node optimize-images.mjs --upload           # Optimize and upload to R2
 *   node optimize-images.mjs --upload-only      # Upload existing derived images to R2
 *
 * The path should be relative to static/images/original/
 * Example: node optimize-images.mjs lifestyle/DSC_2634.jpg
 *
 * R2 Upload Environment Variables:
 *   R2_ENDPOINT               - Cloudflare R2 endpoint (https://<account>.r2.cloudflarestorage.com)
 *   R2_ACCESS_KEY_ID          - R2 access key
 *   R2_SECRET_ACCESS_KEY      - R2 secret key
 *   R2_BUCKET_DERIVED_IMAGES  - Bucket name for derived images
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
import { S3Client, PutObjectCommand, ListObjectsV2Command } from "@aws-sdk/client-s3";
import { lookup as getMimeType } from "mime-types";
import dotenv from "dotenv";

// Load .env from project root
const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, "..", "..");
dotenv.config({ path: join(PROJECT_ROOT, ".env") });
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

// Raster image extensions to process (resize + convert to multiple formats)
const RASTER_EXTENSIONS = new Set([".jpg", ".jpeg", ".webp"]);

// PNG extension - check if it's a favicon (copy as-is) or regular image (process)
const PNG_EXTENSION = ".png";

// Extensions to copy as-is (no processing, just add hash)
const COPY_EXTENSIONS = new Set([".svg", ".ico"]);

// R2 configuration from environment variables
const R2_CONFIG = {
  endpoint: process.env.R2_ENDPOINT,
  accessKeyId: process.env.R2_ACCESS_KEY_ID,
  secretAccessKey: process.env.R2_SECRET_ACCESS_KEY,
  bucket: process.env.R2_BUCKET_DERIVED_IMAGES,
};

/**
 * Create S3 client for R2
 */
function createR2Client() {
  if (!R2_CONFIG.endpoint || !R2_CONFIG.accessKeyId || !R2_CONFIG.secretAccessKey) {
    throw new Error(
      "Missing R2 credentials. Set R2_ENDPOINT, R2_ACCESS_KEY_ID, and R2_SECRET_ACCESS_KEY environment variables."
    );
  }
  if (!R2_CONFIG.bucket) {
    throw new Error("Missing R2_BUCKET_DERIVED_IMAGES environment variable.");
  }

  return new S3Client({
    region: "auto",
    endpoint: R2_CONFIG.endpoint,
    credentials: {
      accessKeyId: R2_CONFIG.accessKeyId,
      secretAccessKey: R2_CONFIG.secretAccessKey,
    },
  });
}

/**
 * Upload a file to R2 with immutable caching headers
 */
async function uploadToR2(client, localPath, key) {
  const content = await readFile(localPath);
  const contentType = getMimeType(localPath) || "application/octet-stream";

  const command = new PutObjectCommand({
    Bucket: R2_CONFIG.bucket,
    Key: key,
    Body: content,
    ContentType: contentType,
    CacheControl: "public, max-age=31536000, immutable",
  });

  await client.send(command);
}

/**
 * List all existing objects in R2 bucket
 */
async function listExistingR2Objects(client) {
  const existingKeys = new Set();
  let continuationToken;

  do {
    const command = new ListObjectsV2Command({
      Bucket: R2_CONFIG.bucket,
      ContinuationToken: continuationToken,
    });

    const response = await client.send(command);

    if (response.Contents) {
      for (const obj of response.Contents) {
        existingKeys.add(obj.Key);
      }
    }

    continuationToken = response.IsTruncated ? response.NextContinuationToken : undefined;
  } while (continuationToken);

  return existingKeys;
}

/**
 * Upload all derived images to R2
 */
async function uploadDerivedImagesToR2() {
  console.log("☁️  Uploading derived images to R2...\n");

  const client = createR2Client();
  const files = await getAllImages(DERIVED_DIR);

  if (files.length === 0) {
    console.log("   No files found in derived directory.");
    return;
  }

  console.log(`   Found ${files.length} local files`);

  // List existing objects in R2 to skip already-uploaded files
  console.log("   Checking existing files in R2...");
  const existingKeys = await listExistingR2Objects(client);
  console.log(`   Found ${existingKeys.size} existing files in R2\n`);

  let uploaded = 0;
  let skipped = 0;
  let errors = 0;

  for (const filePath of files) {
    const key = relative(DERIVED_DIR, filePath);

    // Skip if file already exists in R2
    if (existingKeys.has(key)) {
      skipped++;
      continue;
    }

    try {
      await uploadToR2(client, filePath, key);
      console.log(`   ✓ ${key}`);
      uploaded++;
    } catch (err) {
      console.log(`   ❌ ${key}: ${err.message}`);
      errors++;
    }
  }

  console.log(`\n✅ Upload complete: ${uploaded} uploaded, ${skipped} skipped (already exist), ${errors} errors`);

  if (errors > 0) {
    process.exit(1);
  }
}

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

  // Pattern to match /static/images/original/ paths (allows spaces in filenames)
  const imagePathRegex = /\/static\/images\/original\/([^"')]+\.(jpg|jpeg|png|webp|svg|ico))/gi;

  // Pattern to match filter-based references like "path/to/image"|image_hash
  const filterPathRegex = /"([^"]+)"\s*\|\s*image_hash/g;

  // Pattern to match Rust function calls like get_image_hash("path/to/image")
  const rustFunctionRegex = /get_image_hash\("([^"]+)"\)/g;

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
      for (const ext of [".svg", ".ico", ".jpg", ".jpeg", ".png", ".webp"]) {
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

    // Find Rust function calls like get_image_hash("path")
    while ((match = rustFunctionRegex.exec(content)) !== null) {
      const basePath = match[1];

      // Try to find the actual file with common extensions
      for (const ext of [".svg", ".ico", ".jpg", ".jpeg", ".png", ".webp"]) {
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
          if (RASTER_EXTENSIONS.has(ext) || COPY_EXTENSIONS.has(ext) || ext === PNG_EXTENSION) {
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

  // Build list of sizes to generate: preset sizes + original size if it doesn't match a preset
  const sizesToGenerate = SIZES.filter((size) => size <= originalWidth);

  // Add original size if it's under the max (2400) and doesn't match an existing preset
  const maxPreset = SIZES[SIZES.length - 1];
  if (originalWidth <= maxPreset && !SIZES.includes(originalWidth)) {
    sizesToGenerate.push(originalWidth);
    sizesToGenerate.sort((a, b) => a - b);
  }

  // Generate each size variant
  for (const targetWidth of sizesToGenerate) {
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
 * Copy file with hash in filename (for SVG, ICO, favicon PNGs, etc.)
 */
async function copyFileWithHash(inputPath, outputDir, relativePath, hash) {
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
 * Check if derived files exist for an image with the given hash
 */
async function derivedFilesExist(imagePath, hash) {
  const ext = extname(imagePath).toLowerCase();
  const nameWithoutExt = imagePath.slice(0, -ext.length);

  if (COPY_EXTENSIONS.has(ext) || (ext === PNG_EXTENSION && imagePath.startsWith("favicon/"))) {
    // For copied files, just check the single hashed file exists
    const derivedPath = join(DERIVED_DIR, `${nameWithoutExt}.${hash}${ext}`);
    try {
      await stat(derivedPath);
      return true;
    } catch {
      return false;
    }
  }

  // For raster images, check that at least one derived file exists
  // We check for the smallest size variant in AVIF format as a proxy
  const smallestSize = SIZES[0];
  const avifPath = join(DERIVED_DIR, `${nameWithoutExt}.${hash}-${smallestSize}.avif`);
  try {
    await stat(avifPath);
    return true;
  } catch {
    return false;
  }
}

/**
 * Main optimization function
 */
async function optimize() {
  console.log("🍍 Naked Pineapple Image Optimizer (with content hashing)\n");

  // Step 1: Load existing manifest for cache checking
  console.log("📋 Loading existing manifest...");
  const existingManifest = await loadExistingManifest();
  console.log(`   Found ${Object.keys(existingManifest).length} existing entries\n`);

  // Step 2: Discover used images
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

  // Step 3: Create derived directory
  await mkdir(DERIVED_DIR, { recursive: true });

  // Step 4: Process each used image and build manifest
  const manifest = {}; // Maps base path (without extension) to { hash, maxWidth }
  let processedCount = 0;
  let skippedCount = 0;
  let cachedCount = 0;
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

    // Check if this image was already processed with the same hash
    const existingEntry = existingManifest[basePath];
    if (existingEntry && existingEntry.hash === hash) {
      // Hash matches - check if derived files actually exist
      const filesExist = await derivedFilesExist(imagePath, hash);
      if (filesExist) {
        // Reuse existing manifest entry
        manifest[basePath] = existingEntry;
        cachedCount++;
        continue;
      }
      // Files missing, need to regenerate
    }

    try {
      if (COPY_EXTENSIONS.has(ext)) {
        // Copy SVG/ICO with hash (maxWidth = 0, they're not resized)
        const results = await copyFileWithHash(inputPath, DERIVED_DIR, imagePath, hash);
        manifest[basePath] = { hash, maxWidth: 0 };
        console.log(`   ✓ Copied: ${imagePath} [${hash}]`);
        totalVariants += results.length;
      } else if (ext === PNG_EXTENSION && imagePath.startsWith("favicon/")) {
        // Favicon PNGs - copy as-is with hash (already at correct size)
        const results = await copyFileWithHash(inputPath, DERIVED_DIR, imagePath, hash);
        manifest[basePath] = { hash, maxWidth: 0 };
        console.log(`   ✓ Copied favicon: ${imagePath} [${hash}]`);
        totalVariants += results.length;
      } else if (RASTER_EXTENSIONS.has(ext) || ext === PNG_EXTENSION) {
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

  // Step 5: Generate Rust manifest
  await generateRustManifest(manifest);

  // Summary
  console.log("\n✅ Optimization complete!");
  console.log(`   Processed: ${processedCount} images`);
  console.log(`   Cached: ${cachedCount} images (unchanged)`);
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
    if (COPY_EXTENSIONS.has(ext)) {
      // Copy SVG/ICO with hash
      const results = await copyFileWithHash(inputPath, DERIVED_DIR, imagePath, hash);
      manifest[basePath] = { hash, maxWidth: 0 };
      console.log(`✓ Copied: ${imagePath} [${hash}]`);
      console.log(`  Generated ${results.length} file(s)`);
    } else if (ext === PNG_EXTENSION && imagePath.startsWith("favicon/")) {
      // Favicon PNGs - copy as-is with hash
      const results = await copyFileWithHash(inputPath, DERIVED_DIR, imagePath, hash);
      manifest[basePath] = { hash, maxWidth: 0 };
      console.log(`✓ Copied favicon: ${imagePath} [${hash}]`);
      console.log(`  Generated ${results.length} file(s)`);
    } else if (RASTER_EXTENSIONS.has(ext) || ext === PNG_EXTENSION) {
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
const hasUploadOnly = args.includes("--upload-only");
const hasUpload = args.includes("--upload");
const imageArg = args.find((arg) => !arg.startsWith("--"));

async function main() {
  if (hasUploadOnly) {
    // Upload-only mode: just upload existing derived images to R2
    await uploadDerivedImagesToR2();
  } else if (imageArg) {
    // Single image mode
    await optimizeSingle(imageArg);
    if (hasUpload) {
      await uploadDerivedImagesToR2();
    }
  } else {
    // Full optimization mode
    await optimize();
    if (hasUpload) {
      await uploadDerivedImagesToR2();
    }
  }
}

main().catch((err) => {
  console.error("❌ Error:", err);
  process.exit(1);
});
