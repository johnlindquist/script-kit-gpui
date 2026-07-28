#!/usr/bin/env node
// Mockup honesty lint: hand-written mockup CSS may not contain visual
// literals — every color, size, radius, gap, opacity, and font size must
// resolve through a generated --sk-* custom property, keeping HTML mockups
// incapable of drifting from the Rust design contract.
//
// Generated files (design/mockups/generated/**) are exempt. Values allowed
// in hand-written CSS: 0, 1 (flex factors), 100%, auto, none, inherit,
// currentColor, transparent, var(...), calc() over vars, and --sk-emulator-*
// declarations (browser-only calibration, annotated in known-divergence).
//
// Usage: node design/mockups/tests/lint-mockups.mjs
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const failures = [];

function* cssFiles(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      if (entry === "generated" || entry === "node_modules") continue;
      yield* cssFiles(path);
    } else if (entry.endsWith(".css")) {
      yield path;
    }
  }
}

const DECL_RE = /([\w-]+)\s*:\s*([^;{}]+);/g;
// Properties whose values must be token-derived when they carry magnitude.
const VISUAL_PROPS =
  /^(color|background|background-color|border|border-.*|outline|box-shadow|font-size|font-weight|line-height|letter-spacing|opacity|padding.*|margin.*|gap|row-gap|column-gap|width|min-width|max-width|height|min-height|max-height|top|right|bottom|left|inset.*|border-radius|backdrop-filter|-webkit-backdrop-filter|fill|stroke|stroke-width)$/;
// vh/vw are allowed: they position the harness stage around the window and
// cannot encode app-design magnitudes.
const LITERAL_RE =
  /(#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(|\d*\.?\d+(px|pt|rem|em)\b)/;

function valueIsClean(value) {
  // Strip var() references and calc arithmetic over vars before scanning.
  const stripped = value
    .replace(/var\(--sk-[\w-]+\)/g, "VAR")
    .replace(/calc\(([^()]|\([^()]*\))*\)/g, (m) =>
      /\d*\.?\d+(px|pt|rem|em)/.test(m.replace(/var\(--sk-[\w-]+\)/g, "")) ? m : "CALC",
    );
  return !LITERAL_RE.test(stripped);
}

for (const file of cssFiles(root)) {
  const css = readFileSync(file, "utf8");
  const rel = relative(root, file);
  let match;
  while ((match = DECL_RE.exec(css))) {
    const [, prop, value] = match;
    // Emulator variables are declared literals by design — allowed, but only
    // under the --sk-emulator- namespace.
    if (prop.startsWith("--sk-emulator-")) continue;
    if (prop.startsWith("--")) {
      // Alias hooks (host-parameterized shared components, e.g. mapping
      // --sk-compact-caret-* onto a screen's generated tokens) are allowed
      // as long as the value itself is literal-free: pure var()/calc-over-var
      // indirection cannot smuggle in a design magnitude.
      if (!valueIsClean(value)) {
        failures.push(`${rel}: custom property ${prop} carries a literal outside the emulator namespace`);
      }
      continue;
    }
    if (!VISUAL_PROPS.test(prop)) continue;
    if (!valueIsClean(value)) {
      failures.push(`${rel}: literal visual value in "${prop}: ${value.trim()}"`);
    }
  }
}

// ── Routes AROUND the CSS contract ────────────────────────────────────────
// The CSS walk above only saw .css files. surface-adapters.js used to inject a
// <style> string and assign inline pixel styles, so literals like 2px / 17px /
// 1px / 1.05s lived in JS and were never checked — the lint passed while the
// contract leaked. These passes close that hole for HTML and JS.
//
// Scope note: only properties that carry MAGNITUDE or COLOR are checked.
// Layout toggles (display/position) and offsets computed from measured layout
// (e.g. `left = rect.right - shellRect.left + "px"`) are legitimate and must
// not be flagged, or the rule becomes noise instead of architecture.

function* filesWithExt(dir, exts) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      if (entry === "generated" || entry === "node_modules") continue;
      yield* filesWithExt(path, exts);
    } else if (exts.some((e) => entry.endsWith(e))) {
      yield path;
    }
  }
}

// A JS style assignment is only a violation when the assigned value is a
// STRING LITERAL containing a magnitude or color. Concatenations of computed
// numbers are measurement, not design.
const JS_STYLE_RE = /\.style\.([A-Za-z]+)\s*=\s*(["'])((?:(?!\2).)*)\2/g;
const JS_CSSTEXT_RE = /\.(?:style\.cssText|textContent|innerHTML)\s*=\s*([`"'])((?:(?!\1)[\s\S])*)\1/g;

function camelToKebab(s) {
  return s.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
}

for (const file of filesWithExt(root, [".js", ".mjs"])) {
  const rel = relative(root, file);
  if (rel.startsWith("tests/")) continue; // the linters themselves
  const src = readFileSync(file, "utf8");
  let m;
  while ((m = JS_STYLE_RE.exec(src))) {
    const prop = camelToKebab(m[1]);
    const value = m[3];
    if (!VISUAL_PROPS.test(prop)) continue;
    if (!valueIsClean(value)) {
      failures.push(`${rel}: JS assigns literal visual value — style.${m[1]} = "${value}"`);
    }
  }
  // Injected stylesheet text: any CSS-looking magnitude in a JS string that is
  // written into the document is subject to the same contract.
  while ((m = JS_CSSTEXT_RE.exec(src))) {
    const body = m[2];
    if (!/[{;]/.test(body)) continue; // not CSS-ish
    let d;
    const re = new RegExp(DECL_RE.source, "g");
    while ((d = re.exec(body))) {
      const [, prop, value] = d;
      if (prop.startsWith("--sk-emulator-")) continue;
      if (!VISUAL_PROPS.test(prop)) continue;
      if (!valueIsClean(value)) {
        failures.push(`${rel}: JS-injected CSS carries a literal — "${prop}: ${value.trim()}"`);
      }
    }
    if (/\b\d*\.?\d+(px|pt|rem|em|s|ms)\b/.test(body) && !/var\(--sk-/.test(body)) {
      failures.push(`${rel}: JS-injected CSS contains a bare magnitude (${body.slice(0, 60)}…)`);
    }
  }
}

// HTML style="" attributes and <style> blocks — PRODUCT FIXTURES ONLY.
//
// Scope boundary (from the storyboard architecture review): story-shell and
// tooling presentation chrome may carry literals; PRODUCT surfaces may not.
// Product surfaces are the screen fixtures themselves — screens/<name>/index.html
// — which are what the Rust renderers are held against. Deliberately excluded:
//   - the mockup gallery/index pages and operate/ (navigation chrome)
//   - screens/*/compare.html (onion-skin difference tooling for humans, whose
//     overlay colors are diagnostic instruments, not product design)
//   - stories/**/index.html (story shell chrome; see story-adapter.css)
// Measured when this rule landed: product fixtures had ZERO violations, so the
// boundary reflects the real state rather than grandfathering product debt.
const HTML_STYLE_ATTR_RE = /\sstyle\s*=\s*"([^"]*)"/g;
const HTML_STYLE_BLOCK_RE = /<style[^>]*>([\s\S]*?)<\/style>/g;
const isProductFixture = (rel) => /^screens\/[^/]+\/index\.html$/.test(rel);

for (const file of filesWithExt(root, [".html"])) {
  const rel = relative(root, file);
  if (!isProductFixture(rel)) continue;
  const src = readFileSync(file, "utf8");
  let m;
  while ((m = HTML_STYLE_ATTR_RE.exec(src))) {
    let d;
    const re = new RegExp(DECL_RE.source, "g");
    const body = m[1].endsWith(";") ? m[1] : m[1] + ";";
    while ((d = re.exec(body))) {
      const [, prop, value] = d;
      if (prop.startsWith("--sk-emulator-")) continue;
      if (!VISUAL_PROPS.test(prop)) continue;
      if (!valueIsClean(value)) {
        failures.push(`${rel}: inline style attribute carries a literal — "${prop}: ${value.trim()}"`);
      }
    }
  }
  while ((m = HTML_STYLE_BLOCK_RE.exec(src))) {
    let d;
    const re = new RegExp(DECL_RE.source, "g");
    while ((d = re.exec(m[1]))) {
      const [, prop, value] = d;
      if (prop.startsWith("--sk-emulator-")) continue;
      if (!VISUAL_PROPS.test(prop)) continue;
      if (!valueIsClean(value)) {
        failures.push(`${rel}: <style> block carries a literal — "${prop}: ${value.trim()}"`);
      }
    }
  }
}

if (failures.length) {
  console.error(`✗ mockup lint: ${failures.length} literal(s) found`);
  for (const failure of failures) console.error("  " + failure);
  process.exit(1);
}
console.log("✓ mockup lint: CSS, JS-assigned styles, JS-injected CSS, and HTML style attributes are all token-derived");
