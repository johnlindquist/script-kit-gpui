// Name: SDK Test - captureScreenshot()
// Description: Tests captureScreenshot() function for visual testing support

/**
 * SDK TEST: test-capture-screenshot.ts
 *
 * Tests the captureScreenshot() function which captures the Script Kit window.
 *
 * Test cases:
 * 1. captureScreenshot-function-exists: Verify function is defined
 * 2. captureScreenshot-basic: Basic capture returns valid ScreenshotData
 * 3. captureScreenshot-dimensions: Captured dimensions are reasonable
 * 4. captureScreenshot-png-data: Data is valid base64 PNG
 * 5. captureScreenshot-hidpi-option: hiDpi option changes dimensions
 *
 * Expected behavior:
 * - Returns { data: string, width: number, height: number }
 * - data is base64-encoded PNG
 * - width/height match window dimensions (or 2x for hiDpi)
 */

import "../../scripts/kit-sdk";

// =============================================================================
// Test Infrastructure
// =============================================================================

interface TestResult {
  test: string;
  status: "running" | "pass" | "fail" | "skip";
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
  expected?: string;
  actual?: string;
}

function logTest(
  name: string,
  status: TestResult["status"],
  extra?: Partial<TestResult>
) {
  const result: TestResult = {
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra,
  };
  console.log(JSON.stringify(result));
}

function debug(msg: string) {
  console.error(`[TEST] ${msg}`);
}

// =============================================================================
// Tests
// =============================================================================

debug("test-capture-screenshot.ts starting...");
debug(`SDK globals: captureScreenshot=${typeof captureScreenshot}`);

// -----------------------------------------------------------------------------
// Test 1: Verify captureScreenshot function exists
// -----------------------------------------------------------------------------
const test1 = "captureScreenshot-function-exists";
logTest(test1, "running");
const start1 = Date.now();

try {
  debug("Test 1: Verify captureScreenshot function exists");

  if (typeof captureScreenshot !== "function") {
    logTest(test1, "fail", {
      error: `Expected captureScreenshot to be a function, got ${typeof captureScreenshot}`,
      duration_ms: Date.now() - start1,
    });
  } else {
    logTest(test1, "pass", {
      result: { type: typeof captureScreenshot },
      duration_ms: Date.now() - start1,
    });
  }
} catch (err) {
  logTest(test1, "fail", {
    error: String(err),
    duration_ms: Date.now() - start1,
  });
}

// -----------------------------------------------------------------------------
// Test 2: Basic capture returns valid ScreenshotData
// -----------------------------------------------------------------------------
const test2 = "captureScreenshot-basic";
logTest(test2, "running");
const start2 = Date.now();

try {
  debug("Test 2: Basic capture returns valid ScreenshotData");

  // Display something first so we have content to capture
  await div("<div class='p-4 bg-blue-500 text-white'>Screenshot Test</div>");

  // Wait for render
  await wait(500);

  const screenshot = await captureScreenshot();

  debug(`Screenshot: ${screenshot.width}x${screenshot.height}, data length: ${screenshot.data.length}`);

  const checks = [
    typeof screenshot === "object",
    typeof screenshot.data === "string",
    typeof screenshot.width === "number",
    typeof screenshot.height === "number",
    screenshot.data.length > 0,
    screenshot.width > 0,
    screenshot.height > 0,
  ];

  if (checks.every(Boolean)) {
    logTest(test2, "pass", {
      result: {
        width: screenshot.width,
        height: screenshot.height,
        dataLength: screenshot.data.length,
      },
      duration_ms: Date.now() - start2,
    });
  } else {
    logTest(test2, "fail", {
      error: "Screenshot data structure is invalid",
      actual: JSON.stringify({
        hasData: typeof screenshot.data === "string",
        hasWidth: typeof screenshot.width === "number",
        hasHeight: typeof screenshot.height === "number",
      }),
      duration_ms: Date.now() - start2,
    });
  }
} catch (err) {
  logTest(test2, "fail", {
    error: String(err),
    duration_ms: Date.now() - start2,
  });
}

// -----------------------------------------------------------------------------
// Test 3: Captured dimensions are reasonable
// -----------------------------------------------------------------------------
const test3 = "captureScreenshot-dimensions";
logTest(test3, "running");
const start3 = Date.now();

try {
  debug("Test 3: Captured dimensions are reasonable");

  // Capture after the previous div is still displayed
  const screenshot = await captureScreenshot();

  // Typical Script Kit window is around 500-600px wide, 300-800px tall
  const minWidth = 100;
  const maxWidth = 2000;
  const minHeight = 100;
  const maxHeight = 2000;

  const widthOk = screenshot.width >= minWidth && screenshot.width <= maxWidth;
  const heightOk =
    screenshot.height >= minHeight && screenshot.height <= maxHeight;

  debug(
    `Dimensions: ${screenshot.width}x${screenshot.height} (expected ${minWidth}-${maxWidth}x${minHeight}-${maxHeight})`
  );

  if (widthOk && heightOk) {
    logTest(test3, "pass", {
      result: { width: screenshot.width, height: screenshot.height },
      duration_ms: Date.now() - start3,
    });
  } else {
    logTest(test3, "fail", {
      error: `Dimensions out of expected range`,
      expected: `${minWidth}-${maxWidth}x${minHeight}-${maxHeight}`,
      actual: `${screenshot.width}x${screenshot.height}`,
      duration_ms: Date.now() - start3,
    });
  }
} catch (err) {
  logTest(test3, "fail", {
    error: String(err),
    duration_ms: Date.now() - start3,
  });
}

// -----------------------------------------------------------------------------
// Test 4: Data is valid base64 PNG
// -----------------------------------------------------------------------------
const test4 = "captureScreenshot-png-data";
logTest(test4, "running");
const start4 = Date.now();

try {
  debug("Test 4: Data is valid base64 PNG");

  const screenshot = await captureScreenshot();

  // Try to decode the base64 data using Uint8Array (works in Bun without Node types)
  const binaryString = atob(screenshot.data);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }

  // PNG magic bytes: 89 50 4E 47 0D 0A 1A 0A
  const pngMagic = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  // Check first 8 bytes against PNG magic
  let headerMatch = true;
  for (let i = 0; i < 8; i++) {
    if (bytes[i] !== pngMagic[i]) {
      headerMatch = false;
      break;
    }
  }

  debug(
    `Buffer size: ${bytes.length} bytes, PNG header match: ${headerMatch}`
  );

  if (headerMatch) {
    logTest(test4, "pass", {
      result: {
        bufferSize: bytes.length,
        isPng: true,
      },
      duration_ms: Date.now() - start4,
    });
  } else {
    // Convert header bytes to hex for debugging
    const headerHex = Array.from(bytes.subarray(0, 8))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join(" ");
    logTest(test4, "fail", {
      error: "Data does not start with PNG magic bytes",
      expected: "89 50 4E 47 0D 0A 1A 0A",
      actual: headerHex,
      duration_ms: Date.now() - start4,
    });
  }
} catch (err) {
  logTest(test4, "fail", {
    error: String(err),
    duration_ms: Date.now() - start4,
  });
}

// -----------------------------------------------------------------------------
// Test 5: hiDpi option changes dimensions
// -----------------------------------------------------------------------------
const test5 = "captureScreenshot-hidpi-option";
logTest(test5, "running");
const start5 = Date.now();

try {
  debug("Test 5: hiDpi option changes dimensions");

  // Capture without hiDpi (default 1x)
  const screenshot1x = await captureScreenshot({ hiDpi: false });

  // Capture with hiDpi (2x resolution)
  const screenshot2x = await captureScreenshot({ hiDpi: true });

  debug(
    `1x: ${screenshot1x.width}x${screenshot1x.height}, 2x: ${screenshot2x.width}x${screenshot2x.height}`
  );

  // On a retina display, 2x should be double the dimensions
  // On non-retina, they might be the same
  // We'll check that both return valid data
  const bothValid =
    screenshot1x.width > 0 &&
    screenshot1x.height > 0 &&
    screenshot2x.width > 0 &&
    screenshot2x.height > 0;

  // On retina displays, 2x should be >= 1x dimensions
  const dimensionsReasonable =
    screenshot2x.width >= screenshot1x.width &&
    screenshot2x.height >= screenshot1x.height;

  if (bothValid && dimensionsReasonable) {
    logTest(test5, "pass", {
      result: {
        "1x": { width: screenshot1x.width, height: screenshot1x.height },
        "2x": { width: screenshot2x.width, height: screenshot2x.height },
        ratio: screenshot2x.width / screenshot1x.width,
      },
      duration_ms: Date.now() - start5,
    });
  } else {
    logTest(test5, "fail", {
      error: "hiDpi screenshots are not valid or 2x is smaller than 1x",
      actual: JSON.stringify({
        "1x": { width: screenshot1x.width, height: screenshot1x.height },
        "2x": { width: screenshot2x.width, height: screenshot2x.height },
      }),
      duration_ms: Date.now() - start5,
    });
  }
} catch (err) {
  logTest(test5, "fail", {
    error: String(err),
    duration_ms: Date.now() - start5,
  });
}

// -----------------------------------------------------------------------------
// Show Summary
// -----------------------------------------------------------------------------
debug("test-capture-screenshot.ts completed!");

await div(
  md(`# captureScreenshot() Tests Complete

All captureScreenshot() tests have been executed.

## Test Cases Run

| # | Test | Description |
|---|------|-------------|
| 1 | captureScreenshot-function-exists | Verify function is defined |
| 2 | captureScreenshot-basic | Basic capture returns valid data |
| 3 | captureScreenshot-dimensions | Dimensions are reasonable |
| 4 | captureScreenshot-png-data | Data is valid base64 PNG |
| 5 | captureScreenshot-hidpi-option | hiDpi option works |

---

*Check the JSONL output for detailed results*

Press Escape or click to exit.`)
);

debug("test-capture-screenshot.ts exiting...");
