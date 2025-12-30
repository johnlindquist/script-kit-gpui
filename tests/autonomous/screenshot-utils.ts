// tests/autonomous/screenshot-utils.ts
// Screenshot analysis utilities for autonomous visual testing
//
// This file uses Bun's built-in fs and path APIs.
// Run with: bun run tests/autonomous/screenshot-utils.ts

// Bun globals for Node.js compatibility
declare const process: { cwd: () => string };

// Use Bun's built-in APIs
// @ts-ignore - Bun supports node: protocol
const fs = await import('node:fs/promises');
// @ts-ignore - Bun supports node: protocol
const path = await import('node:path');

// Import diff utilities
import { compareImages, type DiffResult, type DiffOptions } from './screenshot-diff';

// Directory where screenshots are saved (relative to project root)
const SCREENSHOT_DIR = '.test-screenshots';

/**
 * Ensure the screenshot directory exists
 */
export async function ensureScreenshotDir(): Promise<void> {
  const dir = path.resolve(process.cwd(), SCREENSHOT_DIR);
  await fs.mkdir(dir, { recursive: true });
}

/**
 * Generate a timestamp string for filenames
 * Format: YYYYMMDD-HHmmss
 */
function generateTimestamp(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  const hour = String(now.getHours()).padStart(2, '0');
  const minute = String(now.getMinutes()).padStart(2, '0');
  const second = String(now.getSeconds()).padStart(2, '0');
  return `${year}${month}${day}-${hour}${minute}${second}`;
}

/**
 * Save a screenshot buffer to the .test-screenshots directory
 * @param data - Base64-encoded PNG data
 * @param name - Test name (used in filename)
 * @returns Full path to saved screenshot
 */
export async function saveScreenshot(data: string, name: string): Promise<string> {
  await ensureScreenshotDir();

  // Sanitize name for filesystem
  const safeName = name.replace(/[^a-zA-Z0-9_-]/g, '-');
  const timestamp = generateTimestamp();
  const filename = `${safeName}-${timestamp}.png`;
  const filepath = path.resolve(process.cwd(), SCREENSHOT_DIR, filename);

  // Decode base64 and write to file
  // Use Bun's Buffer which is globally available
  const buffer = Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
  await fs.writeFile(filepath, buffer);

  return filepath;
}

/**
 * Parse PNG header to get image dimensions
 * Works with both base64 strings and file paths
 *
 * PNG format:
 * - Magic bytes (8): 89 50 4E 47 0D 0A 1A 0A
 * - IHDR chunk length (4): always 0x0000000D (13)
 * - IHDR chunk type (4): "IHDR"
 * - Width (4): big-endian uint32 at offset 16
 * - Height (4): big-endian uint32 at offset 20
 *
 * @param input - Base64-encoded PNG or file path
 * @returns {width, height} in pixels
 */
export async function getImageDimensions(input: string): Promise<{ width: number; height: number }> {
  let bytes: Uint8Array;

  // Determine if input is a file path or base64 data
  if (input.includes('/') || input.includes('\\') || input.endsWith('.png')) {
    // It's a file path - read only the first 24 bytes (header)
    const fileBuffer = await fs.readFile(input);
    bytes = new Uint8Array(fileBuffer);
  } else {
    // It's base64 data - decode only what we need for header
    const decoded = atob(input.slice(0, 48)); // 48 base64 chars = 36 bytes, enough for header
    bytes = Uint8Array.from(decoded, (c) => c.charCodeAt(0));
  }

  // Validate PNG magic bytes
  const PNG_MAGIC = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
  for (let i = 0; i < PNG_MAGIC.length; i++) {
    if (bytes[i] !== PNG_MAGIC[i]) {
      throw new Error(`Invalid PNG file: magic bytes mismatch at position ${i}`);
    }
  }

  // Read width and height from IHDR chunk (big-endian uint32)
  // Width is at bytes 16-19, Height is at bytes 20-23
  const dataView = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = dataView.getUint32(16, false); // false = big-endian
  const height = dataView.getUint32(20, false);

  return { width, height };
}

/**
 * Analysis result for content fill check
 */
export interface FillAnalysis {
  pass: boolean;
  message: string;
  actualWidth?: number;
  actualHeight?: number;
  expectedHeight?: number;
  heightDifference?: number;
  emptySpacePercent?: number;
}

/**
 * Analyze if content fills the expected window area
 * Checks if there's excessive empty space at the bottom (indicates layout bug)
 * @param screenshotPath - Path to screenshot PNG
 * @param expectedHeight - Expected content height
 * @param tolerance - Acceptable height difference in pixels (default: 20)
 * @returns Analysis result with pass/fail and details
 */
export async function analyzeContentFill(
  screenshotPath: string,
  expectedHeight: number,
  tolerance: number = 20
): Promise<FillAnalysis> {
  try {
    const dimensions = await getImageDimensions(screenshotPath);
    const heightDiff = Math.abs(dimensions.height - expectedHeight);
    const pass = heightDiff <= tolerance;

    const emptySpacePercent = dimensions.height > expectedHeight
      ? ((dimensions.height - expectedHeight) / dimensions.height) * 100
      : 0;

    return {
      pass,
      message: pass
        ? `Height ${dimensions.height}px matches expected ${expectedHeight}px (within ±${tolerance}px tolerance)`
        : `Height mismatch: actual ${dimensions.height}px vs expected ${expectedHeight}px (diff: ${heightDiff}px, tolerance: ${tolerance}px)`,
      actualWidth: dimensions.width,
      actualHeight: dimensions.height,
      expectedHeight,
      heightDifference: heightDiff,
      emptySpacePercent: Math.round(emptySpacePercent * 10) / 10,
    };
  } catch (error) {
    return {
      pass: false,
      message: `Failed to analyze screenshot: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

/**
 * Generate a test report with screenshot reference
 * @param testName - Name of the test
 * @param screenshotPath - Path to screenshot
 * @param analysis - Analysis result
 * @returns Formatted string suitable for test output
 */
export function generateReport(
  testName: string,
  screenshotPath: string,
  analysis: FillAnalysis
): string {
  const status = analysis.pass ? 'PASS' : 'FAIL';
  const lines = [
    `══════════════════════════════════════════════════════`,
    `${status}: ${testName}`,
    `══════════════════════════════════════════════════════`,
    `Screenshot: ${screenshotPath}`,
    `Result: ${analysis.message}`,
  ];

  if (analysis.actualWidth !== undefined && analysis.actualHeight !== undefined) {
    lines.push(`Dimensions: ${analysis.actualWidth}x${analysis.actualHeight}px`);
  }

  if (analysis.expectedHeight !== undefined) {
    lines.push(`Expected Height: ${analysis.expectedHeight}px`);
  }

  if (analysis.heightDifference !== undefined) {
    lines.push(`Height Difference: ${analysis.heightDifference}px`);
  }

  if (analysis.emptySpacePercent !== undefined && analysis.emptySpacePercent > 0) {
    lines.push(`Empty Space: ${analysis.emptySpacePercent}%`);
  }

  lines.push(`══════════════════════════════════════════════════════`);

  return lines.join('\n');
}

/**
 * Compare two screenshots to detect visual differences
 * @param path1 - First screenshot path
 * @param path2 - Second screenshot path
 * @returns true if screenshots are byte-identical
 */
export async function screenshotsMatch(
  path1: string,
  path2: string
): Promise<boolean> {
  try {
    const [buffer1, buffer2] = await Promise.all([
      fs.readFile(path1),
      fs.readFile(path2),
    ]);

    const bytes1 = new Uint8Array(buffer1);
    const bytes2 = new Uint8Array(buffer2);

    // Quick length check
    if (bytes1.length !== bytes2.length) {
      return false;
    }

    // Byte-by-byte comparison
    for (let i = 0; i < bytes1.length; i++) {
      if (bytes1[i] !== bytes2[i]) {
        return false;
      }
    }

    return true;
  } catch {
    return false;
  }
}

/**
 * Get basic stats about a screenshot
 * @param screenshotPath - Path to screenshot PNG
 * @returns Object with dimensions and file size
 */
export async function getScreenshotStats(screenshotPath: string): Promise<{
  width: number;
  height: number;
  fileSizeBytes: number;
  fileSizeKB: number;
}> {
  const [dimensions, stats] = await Promise.all([
    getImageDimensions(screenshotPath),
    fs.stat(screenshotPath),
  ]);

  return {
    width: dimensions.width,
    height: dimensions.height,
    fileSizeBytes: stats.size,
    fileSizeKB: Math.round(stats.size / 1024),
  };
}

/**
 * List all screenshots in the screenshot directory
 * @returns Array of screenshot file paths, sorted by modification time (newest first)
 */
export async function listScreenshots(): Promise<string[]> {
  const dir = path.resolve(process.cwd(), SCREENSHOT_DIR);

  try {
    const files = await fs.readdir(dir);
    const pngFiles = files.filter((f: string) => f.endsWith('.png'));

    // Get stats for each file to sort by mtime
    const filesWithStats = await Promise.all(
      pngFiles.map(async (file: string) => {
        const filepath = path.join(dir, file);
        const stat = await fs.stat(filepath);
        return { filepath, mtime: stat.mtime };
      })
    );

    // Sort by mtime, newest first
    filesWithStats.sort((a: { mtime: Date }, b: { mtime: Date }) =>
      b.mtime.getTime() - a.mtime.getTime()
    );

    return filesWithStats.map((f: { filepath: string }) => f.filepath);
  } catch {
    return [];
  }
}

/**
 * Clean up old screenshots, keeping only the most recent N
 * @param keepCount - Number of screenshots to keep (default: 10)
 * @returns Number of screenshots deleted
 */
export async function cleanupOldScreenshots(keepCount: number = 10): Promise<number> {
  const screenshots = await listScreenshots();

  if (screenshots.length <= keepCount) {
    return 0;
  }

  const toDelete = screenshots.slice(keepCount);
  await Promise.all(toDelete.map((f: string) => fs.unlink(f)));

  return toDelete.length;
}

/**
 * Get stats about the screenshot directory
 * @returns Directory-level statistics
 */
export async function getScreenshotDirectoryStats(): Promise<{
  count: number;
  totalSize: number;
  oldest: string;
  newest: string;
}> {
  const dir = path.resolve(process.cwd(), SCREENSHOT_DIR);

  try {
    const files = await fs.readdir(dir);
    const pngFiles = files.filter((f: string) => f.endsWith('.png'));

    if (pngFiles.length === 0) {
      return {
        count: 0,
        totalSize: 0,
        oldest: '',
        newest: '',
      };
    }

    // Get stats for each file
    const filesWithStats = await Promise.all(
      pngFiles.map(async (file: string) => {
        const filepath = path.join(dir, file);
        const stat = await fs.stat(filepath);
        return { filepath, mtime: stat.mtime, size: stat.size };
      })
    );

    // Sort by mtime to find oldest and newest
    filesWithStats.sort((a: { mtime: Date }, b: { mtime: Date }) =>
      a.mtime.getTime() - b.mtime.getTime()
    );

    const totalSize = filesWithStats.reduce((sum: number, f: { size: number }) => sum + f.size, 0);

    return {
      count: filesWithStats.length,
      totalSize,
      oldest: filesWithStats[0].filepath,
      newest: filesWithStats[filesWithStats.length - 1].filepath,
    };
  } catch {
    return {
      count: 0,
      totalSize: 0,
      oldest: '',
      newest: '',
    };
  }
}

// ============================================================================
// Visual Regression Testing Functions
// ============================================================================

// Directory for baseline screenshots
const BASELINE_DIR = '.test-screenshots/baselines';

/**
 * Detailed diff result with additional metadata
 */
export interface DetailedDiffResult extends DiffResult {
  /** Test name for reporting */
  testName: string;
  /** Path to baseline image */
  baselinePath: string;
  /** Path to actual image */
  actualPath: string;
  /** Whether this is a new baseline (no previous baseline existed) */
  isNewBaseline: boolean;
}

/**
 * Options for visual comparison
 */
export interface VisualCompareOptions {
  /** Tolerance for color difference per channel (0-255, default: 0) */
  tolerance?: number;
  /** Threshold percentage for considering images as matching (default: 0.1) */
  thresholdPercent?: number;
  /** Whether to generate a diff image on mismatch (default: true) */
  generateDiffImage?: boolean;
  /** Whether to auto-create baseline if missing (default: false) */
  autoCreateBaseline?: boolean;
}

/**
 * Ensure the baseline directory exists
 */
async function ensureBaselineDir(): Promise<void> {
  const dir = path.resolve(process.cwd(), BASELINE_DIR);
  await fs.mkdir(dir, { recursive: true });
}

/**
 * Get the baseline path for a test
 */
function getBaselinePath(testName: string): string {
  const safeName = testName.replace(/[^a-zA-Z0-9_-]/g, '-');
  return path.resolve(process.cwd(), BASELINE_DIR, `${safeName}.png`);
}

/**
 * Check if a baseline exists for a test
 */
export async function baselineExists(testName: string): Promise<boolean> {
  const baselinePath = getBaselinePath(testName);
  try {
    await fs.access(baselinePath);
    return true;
  } catch {
    return false;
  }
}

/**
 * Create or update a baseline image
 * @param testName - Name of the test (used for filename)
 * @param screenshotPath - Path to the screenshot to use as baseline
 * @returns Path to the created baseline
 */
export async function createBaseline(testName: string, screenshotPath: string): Promise<string> {
  await ensureBaselineDir();

  const baselinePath = getBaselinePath(testName);

  // Copy screenshot to baseline location
  const data = await fs.readFile(screenshotPath);
  await fs.writeFile(baselinePath, data);

  return baselinePath;
}

/**
 * Compare a screenshot against its baseline with detailed results
 *
 * @param testName - Name of the test (determines baseline filename)
 * @param actualPath - Path to the actual screenshot to compare
 * @param options - Comparison options
 * @returns Detailed diff result
 */
export async function screenshotsDiff(
  testName: string,
  actualPath: string,
  options: VisualCompareOptions = {}
): Promise<DetailedDiffResult> {
  const {
    tolerance = 0,
    thresholdPercent = 0.1,
    generateDiffImage = true,
    autoCreateBaseline = false,
  } = options;

  const baselinePath = getBaselinePath(testName);
  const hasBaseline = await baselineExists(testName);

  // If no baseline exists
  if (!hasBaseline) {
    if (autoCreateBaseline) {
      await createBaseline(testName, actualPath);
      return {
        testName,
        baselinePath,
        actualPath,
        isNewBaseline: true,
        match: true,
        diffPercent: 0,
        diffPixelCount: 0,
        totalPixels: 0,
        width: 0,
        height: 0,
        dimensionsMatch: true,
      };
    }

    return {
      testName,
      baselinePath,
      actualPath,
      isNewBaseline: false,
      match: false,
      diffPercent: 100,
      diffPixelCount: 0,
      totalPixels: 0,
      width: 0,
      height: 0,
      dimensionsMatch: false,
      error: `No baseline exists for test "${testName}". Create one with createBaseline() or set autoCreateBaseline: true`,
    };
  }

  // Compare against baseline
  const diffResult = await compareImages(baselinePath, actualPath, {
    tolerance,
    thresholdPercent,
    generateDiffImage,
    diffImagePath: generateDiffImage
      ? path.resolve(process.cwd(), BASELINE_DIR, `${testName.replace(/[^a-zA-Z0-9_-]/g, '-')}-diff.png`)
      : undefined,
  });

  return {
    testName,
    baselinePath,
    actualPath,
    isNewBaseline: false,
    ...diffResult,
  };
}

/**
 * Assert that a screenshot matches its baseline within tolerance
 * Throws an error if images don't match, suitable for test assertions
 *
 * @param testName - Name of the test
 * @param actualPath - Path to the actual screenshot
 * @param options - Comparison options
 * @throws Error if images don't match or baseline is missing
 */
export async function assertVisuallySimilar(
  testName: string,
  actualPath: string,
  options: VisualCompareOptions = {}
): Promise<void> {
  const result = await screenshotsDiff(testName, actualPath, {
    thresholdPercent: 0.1, // Default 0.1% tolerance
    ...options,
  });

  if (result.isNewBaseline) {
    console.log(`[BASELINE] Created new baseline for "${testName}": ${result.baselinePath}`);
    return;
  }

  if (!result.match) {
    const details = [
      `Visual regression detected for "${testName}"`,
      `Diff: ${result.diffPercent}% (${result.diffPixelCount} pixels)`,
      `Baseline: ${result.baselinePath}`,
      `Actual: ${result.actualPath}`,
    ];

    if (result.diffImagePath) {
      details.push(`Diff image: ${result.diffImagePath}`);
    }

    if (result.error) {
      details.push(`Error: ${result.error}`);
    }

    throw new Error(details.join('\n'));
  }
}

/**
 * Format detailed diff result as JSONL for machine parsing
 */
export function formatDiffResultJSONL(result: DetailedDiffResult): string {
  return JSON.stringify({
    test: result.testName,
    status: result.match ? 'pass' : 'fail',
    timestamp: new Date().toISOString(),
    is_new_baseline: result.isNewBaseline,
    diff_percent: result.diffPercent,
    diff_pixels: result.diffPixelCount,
    total_pixels: result.totalPixels,
    dimensions: `${result.width}x${result.height}`,
    dimensions_match: result.dimensionsMatch,
    baseline_path: result.baselinePath,
    actual_path: result.actualPath,
    diff_image: result.diffImagePath,
    error: result.error,
  });
}

/**
 * List all baselines
 * @returns Array of { testName, path } objects
 */
export async function listBaselines(): Promise<Array<{ testName: string; path: string }>> {
  await ensureBaselineDir();
  const dir = path.resolve(process.cwd(), BASELINE_DIR);

  try {
    const files = await fs.readdir(dir);
    const pngFiles = files.filter((f: string) => f.endsWith('.png') && !f.endsWith('-diff.png'));

    return pngFiles.map((f: string) => ({
      testName: f.replace('.png', ''),
      path: path.join(dir, f),
    }));
  } catch {
    return [];
  }
}

/**
 * Delete a baseline
 * @param testName - Name of the test
 * @returns true if deleted, false if didn't exist
 */
export async function deleteBaseline(testName: string): Promise<boolean> {
  const baselinePath = getBaselinePath(testName);
  try {
    await fs.unlink(baselinePath);
    // Also try to delete diff image if it exists
    const diffPath = baselinePath.replace('.png', '-diff.png');
    try {
      await fs.unlink(diffPath);
    } catch {
      // Ignore if diff doesn't exist
    }
    return true;
  } catch {
    return false;
  }
}

// Re-export types from screenshot-diff
export type { DiffResult, DiffOptions };
