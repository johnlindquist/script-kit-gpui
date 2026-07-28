#!/usr/bin/env node
/**
 * DOM convergence: the transcript DOM must be a pure function of state.
 *
 * WHY A SHIM
 * ----------
 * jsdom is not a dependency of this repo (decision rule DR2 in
 * .notes/oracle/story-player-determinism/plan.md). Rather than claim DOM proof
 * we do not have, this test implements the smallest DOM surface
 * `reconcileChatMessages` actually touches and drives the REAL adapter code
 * against it. That is weaker than a browser, and deliberately narrow — it
 * proves reconciliation convergence, not layout or pixels. Rect-level proof of
 * `conversation.same-shell-rects` still requires a browser probe and is NOT
 * claimed here.
 *
 * The invariant: applying reconcile(messages) in any order — forward, backward,
 * jumped, repeated — converges to the same DOM for the same final state. The
 * old append-with-random-id adapter could not satisfy this: backward seek left
 * orphaned nodes, and random ids made nodes unaddressable.
 *
 * Usage: node design/mockups/tests/story-dom-convergence.test.mjs
 */
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const sharedDir = join(here, "..", "stories", "shared");
const failures = [];

// ── Minimal DOM ────────────────────────────────────────────────────────────
class El {
  constructor(tag, doc) {
    this.tagName = String(tag).toUpperCase();
    this._doc = doc;
    this.children = [];
    this.parentNode = null;
    this.attrs = {};
    this._text = "";
    this._html = "";
    this.classList = {
      _s: new Set(),
      add: (c) => this.classList._s.add(c),
      remove: (c) => this.classList._s.delete(c),
      contains: (c) => this.classList._s.has(c),
    };
    this.style = {};
  }
  set className(v) {
    this.classList._s = new Set(String(v).split(/\s+/).filter(Boolean));
  }
  get className() {
    return [...this.classList._s].join(" ");
  }
  set textContent(v) {
    this._text = String(v);
    this._html = "";
    this.children = [];
  }
  get textContent() {
    return this._text || this.children.map((c) => c.textContent).join("");
  }
  set innerHTML(v) {
    this._html = String(v);
    this._text = "";
  }
  get innerHTML() {
    return this._html;
  }
  setAttribute(k, v) {
    this.attrs[k] = String(v);
  }
  getAttribute(k) {
    return Object.prototype.hasOwnProperty.call(this.attrs, k) ? this.attrs[k] : null;
  }
  removeAttribute(k) {
    delete this.attrs[k];
  }
  appendChild(c) {
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    this.children.push(c);
    return c;
  }
  removeChild(c) {
    const i = this.children.indexOf(c);
    if (i >= 0) this.children.splice(i, 1);
    c.parentNode = null;
    return c;
  }
  insertBefore(c, ref) {
    const i = this.children.indexOf(ref);
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    this.children.splice(i < 0 ? this.children.length : i, 0, c);
    return c;
  }
  scrollIntoView() {}
  getBoundingClientRect() {
    return { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 };
  }
  _all() {
    return this.children.flatMap((c) => [c, ...c._all()]);
  }
  _matches(sel) {
    return sel
      .split(",")
      .map((s) => s.trim())
      .some((s) => {
        const attr = s.match(/^\[([\w-]+)(?:="([^"]*)")?\]$/);
        if (attr) {
          const v = this.getAttribute(attr[1]);
          return attr[2] == null ? v !== null : v === attr[2];
        }
        const cls = s.match(/^\.([\w-]+)$/);
        if (cls) return this.classList.contains(cls[1]);
        return false;
      });
  }
  querySelector(sel) {
    return this._all().find((e) => e._matches(sel)) || null;
  }
  querySelectorAll(sel) {
    return this._all().filter((e) => e._matches(sel));
  }
}

function makeDoc() {
  const doc = {
    createElement: (t) => new El(t, doc),
    createTextNode: (t) => {
      const e = new El("#text", doc);
      e.textContent = t;
      return e;
    },
    getElementById: () => null,
  };
  doc.documentElement = new El("html", doc);
  doc.head = new El("head", doc);
  doc.body = new El("body", doc);
  doc.documentElement.appendChild(doc.head);
  doc.documentElement.appendChild(doc.body);
  const wrap = (fn) => (sel) => fn.call(doc.documentElement, sel);
  doc.querySelector = wrap(El.prototype.querySelector);
  doc.querySelectorAll = wrap(El.prototype.querySelectorAll);
  return doc;
}

// ── Load the REAL adapter module ───────────────────────────────────────────
const src = readFileSync(join(sharedDir, "surface-adapters.js"), "utf8");
const sandbox = {
  window: {},
  document: makeDoc(),
  getComputedStyle: () => ({ paddingLeft: "0px", fontSize: "14px" }),
  console,
};
vm.createContext(sandbox);
vm.runInContext(src, sandbox, { filename: "surface-adapters.js" });

const adapters = sandbox.window.StorySurfaces && sandbox.window.StorySurfaces.adapters;
if (!adapters) {
  console.error("✗ surface-adapters.js did not expose window.StorySurfaces.adapters");
  process.exit(1);
}
const withReconcile = Object.entries(adapters).filter(([, a]) => a && a.reconcileMessages);
if (!withReconcile.length) {
  console.error("✗ no adapter exposes reconcileMessages — the append-only model is still in place");
  process.exit(1);
}
const [adapterName, adapter] = withReconcile[0];

// ── Fixture: a transcript host plus a non-story node that must survive ─────
function freshDoc() {
  const doc = makeDoc();
  const host = doc.createElement("section");
  host.className = "sk-agent-chat-transcript";
  const fixtureNode = doc.createElement("div");
  fixtureNode.className = "sk-fixture-content";
  fixtureNode.textContent = "fixture row that the story does not own";
  host.appendChild(fixtureNode);
  doc.body.appendChild(host);
  return doc;
}

function snapshot(doc) {
  return doc.querySelectorAll("[data-story-msg]").map((el) => ({
    id: el.getAttribute("data-story-msg"),
    state: el.getAttribute("data-turn-state"),
    cls: el.className,
    text: el.textContent,
  }));
}

const S = {
  empty: [],
  one: [{ id: "u1", role: "user", text: "first question", state: "complete" }],
  two: [
    { id: "u1", role: "user", text: "first question", state: "complete" },
    { id: "a1", role: "assistant", text: "partial ans", state: "streaming" },
  ],
  three: [
    { id: "u1", role: "user", text: "first question", state: "complete" },
    { id: "a1", role: "assistant", text: "complete answer", state: "complete" },
    { id: "u2", role: "user", text: "follow up", state: "complete" },
  ],
};

function applyPath(path) {
  const doc = freshDoc();
  for (const key of path) adapter.reconcileMessages(doc, S[key]);
  return doc;
}

// 1. Forward vs jumped: arriving at `three` directly must equal stepping to it.
const forward = JSON.stringify(snapshot(applyPath(["empty", "one", "two", "three"])));
const jumped = JSON.stringify(snapshot(applyPath(["three"])));
if (forward !== jumped) {
  failures.push(`forward !== jumped\n      forward: ${forward}\n      jumped:  ${jumped}`);
}

// 2. Backward seek: this is the case the old append-only adapter could not do.
const backward = JSON.stringify(snapshot(applyPath(["three", "one"])));
const directOne = JSON.stringify(snapshot(applyPath(["one"])));
if (backward !== directOne) {
  failures.push(
    `backward seek left orphaned nodes\n      after 3->1: ${backward}\n      direct 1:   ${directOne}`,
  );
}

// 3. Idempotence: repeating a state must not duplicate nodes.
const once = JSON.stringify(snapshot(applyPath(["two"])));
const thrice = JSON.stringify(snapshot(applyPath(["two", "two", "two"])));
if (once !== thrice) failures.push(`repeat application duplicated nodes:\n      ${thrice}`);

// 4. Streaming -> complete updates in place, never appends a second node.
const streamingDoc = applyPath(["two"]);
const beforeCount = streamingDoc.querySelectorAll("[data-story-msg]").length;
adapter.reconcileMessages(streamingDoc, S.three);
const afterIds = snapshot(streamingDoc).map((m) => m.id).join(",");
if (beforeCount !== 2) failures.push(`expected 2 story nodes while streaming, saw ${beforeCount}`);
if (afterIds !== "u1,a1,u2") failures.push(`stream completion reordered/duplicated: ${afterIds}`);
const a1 = streamingDoc.querySelector('[data-story-msg="a1"]');
if (a1 && a1.getAttribute("data-turn-state") !== "complete") {
  failures.push("streaming turn did not transition to complete in place");
}

// 5. Fixture content the story does not own must survive reconciliation.
const survivorDoc = applyPath(["three", "empty"]);
if (!survivorDoc.querySelector(".sk-fixture-content")) {
  failures.push("reconciliation destroyed non-story fixture content");
}
if (snapshot(survivorDoc).length !== 0) {
  failures.push("clearing to empty state left story nodes behind");
}

if (failures.length) {
  console.error(`✗ DOM convergence (${adapterName}): ${failures.length} failure(s)`);
  for (const f of failures) console.error("  - " + f);
  process.exit(1);
}
console.log(
  `✓ DOM convergence (${adapterName}): forward==jumped, backward converges, idempotent, ` +
    `stream completes in place, fixture content preserved`,
);
