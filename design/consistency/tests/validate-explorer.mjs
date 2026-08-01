import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const consistency = path.resolve(here, "..");
const root = path.resolve(consistency, "../..");
const manifestPath = path.join(consistency, "data/groups.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const source = fs.readFileSync(path.join(root, ".notes/CONSISTENCY-FIXES.md"), "utf8");
const css = fs.readFileSync(path.join(consistency, "shared/explorer.css"), "utf8");
const js = fs.readFileSync(path.join(consistency, "shared/explorer.js"), "utf8");

const sourceTaskIds = [...source.matchAll(/^###\s+([A-Z]+-\d+)\s+[—:-]/gm)].map((match) => match[1]);
const manifestTasks = manifest.groups.flatMap((group) => group.taskRecords);
const manifestTaskIds = manifestTasks.map((task) => task.id);

function sorted(values) {
  return [...values].sort();
}

test("manifest covers every reviewed task exactly once", () => {
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.groupCount, 12);
  assert.equal(manifest.taskCount, 75);
  assert.equal(new Set(sourceTaskIds).size, 75);
  assert.equal(new Set(manifestTaskIds).size, 75);
  assert.deepEqual(sorted(manifestTaskIds), sorted(sourceTaskIds));
});

test("every group and task carries complete review context", () => {
  for (const group of manifest.groups) {
    assert.match(group.id, /^[a-z0-9-]+$/);
    assert.ok(group.title);
    assert.ok(group.question);
    assert.ok(group.fixture.includes("/mockups/"));
    assert.equal(group.tasks.length, group.taskRecords.length);
    assert.deepEqual(group.tasks, group.taskRecords.map((task) => task.id));
    for (const task of group.taskRecords) {
      for (const field of ["title", "status", "owners", "before", "after", "acceptance", "proof", "guardrail", "recommendation"]) {
        assert.equal(typeof task[field], "string", `${group.id}/${task.id}.${field}`);
        assert.ok(task[field].trim().length > 0, `${group.id}/${task.id}.${field}`);
      }
    }
  }
});

test("all group entry pages use the shared explorer", () => {
  for (const group of manifest.groups) {
    const file = path.join(consistency, "groups", `${group.id}.html`);
    assert.ok(fs.existsSync(file), file);
    const html = fs.readFileSync(file, "utf8");
    assert.match(html, new RegExp(`data-group-id="${group.id}"`));
    assert.match(html, /data-scene-list/);
    assert.match(html, /data-view-mode="split"/);
    assert.match(html, /data-view-mode="before"/);
    assert.match(html, /data-view-mode="after"/);
    assert.match(html, /data-view-mode="overlay"/);
    assert.match(html, /shared\/explorer\.js/);
    assert.doesNotMatch(html, /\bchecked(?:=|\s|>)/i);
  }
});

test("explorer preserves token ownership boundaries", () => {
  assert.doesNotMatch(css, /--sk-[a-z0-9-]+\s*:/i, "consistency CSS must not declare product tokens");
  assert.match(css, /--cx-[a-z0-9-]+\s*:/i, "explorer chrome uses its own namespace");
  assert.match(css, /var\(--sk-/i, "product fragments consume generated tokens");
  assert.doesNotMatch(js, /design\/mockups\/generated\//, "renderer does not write generated outputs");
  assert.doesNotMatch(js, /LIQUID_GLASS_[A-Z_]+\s*=/, "renderer does not retune glass");
});

test("review controls remain neutral and local", () => {
  assert.match(js, /script-kit\.consistency\.review\.v1/);
  assert.match(js, /\["approve", "revise", "reject"\]/);
  assert.match(js, /PROPOSAL · NOT IMPLEMENTED/);
  assert.match(js, /CURRENT · SOURCE-DERIVED/);
  assert.match(js, /data-annotation-outside-frame/);
  assert.match(js, /data-product-emulation/);
  assert.match(js, /exportDecisions/);
  assert.match(fs.readFileSync(path.join(consistency, "index.html"), "utf8"), /Export decisions/);
});

test("every visual group has a dedicated renderer", () => {
  for (const group of manifest.groups) {
    assert.ok(js.includes(`"${group.id}": render`), `missing renderer for ${group.id}`);
  }
});
