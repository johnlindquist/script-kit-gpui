# Visual Testing Infrastructure Audit

> **Scope**: Visual regression testing capabilities  
> **Status**: Functional but incomplete  
> **Readiness**: Local testing ready, CI NOT ready

## Summary

The visual testing infrastructure exists and is functional for local development, but lacks CI integration and has limited baseline coverage. The system uses a pure TypeScript PNG comparison engine.

### Status Overview

```
Infrastructure:
├── captureScreenshot() SDK:  ████████████ Complete
├── screenshot-diff.ts:       ████████████ Complete
├── Baseline images:          ██░░░░░░░░░░ 2 files only
├── CI configuration:         ░░░░░░░░░░░░ Not configured
└── Cross-platform:           ░░░░░░░░░░░░ macOS only
```

## Architecture

### Components

```
┌─────────────────────────────────────────────────────────────┐
│                    Visual Testing Flow                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Test Script          SDK                    Rust/GPUI       │
│  ┌──────────┐    ┌─────────────┐    ┌─────────────────────┐ │
│  │ test.ts  │───>│ captureScreen│───>│ Window.screenshot() │ │
│  └──────────┘    │ shot()       │    │ + base64 encode     │ │
│       │          └──────┬──────┘    └─────────────────────┘ │
│       │                 │                                    │
│       v                 v                                    │
│  ┌──────────┐    ┌─────────────┐                            │
│  │ Save to  │<───│ PNG data    │                            │
│  │ ./test-  │    │ (base64)    │                            │
│  │ screenshots/  └─────────────┘                            │
│  └──────────┘                                                │
│       │                                                      │
│       v                                                      │
│  ┌────────────────────────────────────────────────────────┐ │
│  │              screenshot-diff.ts                         │ │
│  │  Compare against baseline → Generate diff image         │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### File Locations

| Component | Path |
|-----------|------|
| SDK function | `scripts/kit-sdk.ts` → `captureScreenshot()` |
| Diff engine | `tests/autonomous/screenshot-diff.ts` |
| Utilities | `tests/autonomous/screenshot-utils.ts` |
| Baselines | `.test-screenshots/baselines/` |
| Test output | `.test-screenshots/` |

## SDK Function: captureScreenshot()

### Usage

```typescript
import '../../scripts/kit-sdk';

// Capture current window state
const screenshot = await captureScreenshot();

// Returns:
interface ScreenshotResult {
  width: number;   // Pixel width
  height: number;  // Pixel height
  data: string;    // Base64-encoded PNG
}
```

### Saving Screenshots

```typescript
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

const screenshot = await captureScreenshot();

const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });

const filename = `test-${Date.now()}.png`;
const filepath = join(dir, filename);

writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[SCREENSHOT] Saved: ${filepath}`);
```

## Diff Engine: screenshot-diff.ts

### Features

| Feature | Status | Notes |
|---------|--------|-------|
| PNG parsing | Complete | Pure TypeScript |
| Pixel comparison | Complete | Per-pixel diff |
| Tolerance config | Complete | Adjustable threshold |
| Diff image output | Complete | Highlights differences |
| Perceptual diff | Missing | Would reduce false positives |

### Usage

```typescript
import { compareScreenshots, ComparisonResult } from './screenshot-diff';

const result: ComparisonResult = await compareScreenshots(
  '.test-screenshots/current.png',
  '.test-screenshots/baselines/expected.png',
  {
    tolerance: 0.01,  // 1% difference allowed
    generateDiff: true,
    diffOutputPath: 'test-screenshots/diff.png'
  }
);

if (result.match) {
  console.log('Screenshots match!');
} else {
  console.log(`Difference: ${result.diffPercentage}%`);
  console.log(`Diff image: ${result.diffImagePath}`);
}
```

### Configuration Options

```typescript
interface ComparisonOptions {
  tolerance: number;        // 0.0 to 1.0, default 0.01
  generateDiff: boolean;    // Create diff image
  diffOutputPath?: string;  // Where to save diff
  ignoreRegions?: Region[]; // Skip dynamic areas
}

interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}
```

## Current Baselines

### test-screenshots/baselines/

| Baseline | Description | Size |
|----------|-------------|------|
| `test-visual-diff-diff.png` | Diff output example | ~50KB |
| `test-visual-diff.png` | Basic UI state | ~100KB |

**Coverage Gap**: Only 2 baselines for entire application.

### Missing Baselines

| UI State | Priority | Notes |
|----------|----------|-------|
| Empty prompt | High | Initial state |
| Populated list | High | With items |
| Selected item | High | Highlight state |
| Focused window | High | Focus styling |
| Unfocused window | High | Dimmed styling |
| Editor view | Medium | Code editing |
| Terminal view | Medium | Terminal output |
| Actions dialog | Medium | Menu overlay |
| Error state | Medium | Error display |
| Loading state | Low | Loading indicator |

## Workflow

### Creating a Visual Test

```typescript
// tests/smoke/test-visual-example.ts
import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

// 1. Set up UI state to test
await div(`
  <div class="p-4 bg-blue-500 text-white">
    Visual Test Content
  </div>
`);

// 2. Wait for render to complete
await new Promise(resolve => setTimeout(resolve, 500));

// 3. Capture screenshot
const screenshot = await captureScreenshot();
console.error(`[VISUAL] Captured: ${screenshot.width}x${screenshot.height}`);

// 4. Save to test-screenshots/
const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });

const filename = `visual-test-${Date.now()}.png`;
const filepath = join(dir, filename);

writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[VISUAL] Saved: ${filepath}`);

// 5. Exit
process.exit(0);
```

### Running Visual Test

```bash
# Build and run
cargo build && \
  echo '{"type":"run","path":"'$(pwd)'/tests/smoke/test-visual-example.ts"}' | \
  SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1

# Check output
ls -la .test-screenshots/
```

### Comparing to Baseline

```bash
# Run comparison script
bun run tests/autonomous/compare-baseline.ts \
  .test-screenshots/visual-test-xxx.png \
  .test-screenshots/baselines/expected.png
```

## Gaps and Limitations

### Critical Gaps

| Gap | Impact | Priority |
|-----|--------|----------|
| No CI configuration | Tests never run automatically | P0 |
| Headless mode missing | Can't run in CI environments | P0 |
| Limited baselines | Most UI states uncovered | P1 |

### Technical Limitations

| Limitation | Workaround | Future Fix |
|------------|------------|------------|
| macOS only | Skip on other platforms | Add cross-platform support |
| No perceptual diff | Use higher tolerance | Implement SSIM algorithm |
| Fixed window size | Normalize before compare | Multi-resolution support |
| No animation handling | Wait longer | Frame capture option |

## Recommendations

### P0: Add Headless Support

For CI compatibility, add headless rendering:

```rust
// Option 1: Offscreen rendering
let options = WindowOptions {
    visible: false,
    ..Default::default()
};

// Option 2: Virtual display (Xvfb on Linux)
// CI script would start Xvfb before tests
```

### P1: Create Core Baselines

Add baselines for these states:

```bash
# Generate baselines script
./scripts/generate-baselines.sh

# Would create:
# - .test-screenshots/baselines/empty-prompt.png
# - .test-screenshots/baselines/list-with-items.png
# - .test-screenshots/baselines/selected-item.png
# - .test-screenshots/baselines/focused-window.png
# - .test-screenshots/baselines/unfocused-window.png
```

### P2: Add CI Integration

```yaml
# .github/workflows/visual-tests.yml
name: Visual Tests

on: [pull_request]

jobs:
  visual:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Build
        run: cargo build
      
      - name: Run visual tests
        run: |
          for test in tests/smoke/test-visual-*.ts; do
            echo '{"type":"run","path":"'$test'"}' | \
              ./target/debug/script-kit-gpui 2>&1
          done
      
      - name: Compare baselines
        run: bun run tests/autonomous/compare-all-baselines.ts
      
      - name: Upload diffs on failure
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: visual-diffs
          path: .test-screenshots/*-diff.png
```

### P3: Add Perceptual Diff

Implement SSIM (Structural Similarity Index) for better comparison:

```typescript
// Perceptual comparison reduces false positives from:
// - Anti-aliasing differences
// - Slight font rendering variations
// - Sub-pixel color differences

function ssimCompare(img1: ImageData, img2: ImageData): number {
  // Returns 0.0 to 1.0, where 1.0 is identical
  // Threshold of 0.99 typically catches real regressions
}
```

## Test Quality Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Baseline Count | 2 | 15+ |
| UI States Covered | 10% | 80% |
| CI Integration | None | Full |
| Cross-platform | macOS only | macOS + Linux |

---

*Part of [Testing Audit](../TESTING_AUDIT.md)*
