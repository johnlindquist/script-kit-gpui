#!/usr/bin/env bun
/**
 * Single entry point for the devtools suite:
 *
 *   bun scripts/devtools/devtools.ts <tool> [verb] [args...]
 *   bun scripts/devtools/devtools.ts list
 *
 * Dispatches to the per-dimension CLI files so callers (and flows) don't have
 * to memorize file names. Arguments after the tool name are passed through
 * verbatim — each tool owns its own verb and flag parsing.
 */

import { join } from "node:path";
import { assertNoninteractiveVisualProbe } from "./lib/operator-safety.ts";

const TOOLS: Record<string, { file: string; summary: string }> = {
  targets: { file: "targets.ts", summary: "List/inspect automation windows and resolve target identity (list | inspect)" },
  elements: { file: "elements.ts", summary: "Semantic element snapshot for a target (snapshot)" },
  focus: { file: "focus.ts", summary: "Focus and keyboard-ownership inspection (inspect)" },
  keyboard: { file: "keyboard.ts", summary: "Keyboard policy, footer/popup bindings, duplicate keys (inspect)" },
  text: { file: "text.ts", summary: "Text values, lengths, fingerprints (measure)" },
  scroll: { file: "scroll.ts", summary: "Scroll geometry and selected-row visibility (inspect)" },
  layout: { file: "layout.ts", summary: "Layout nodes, regions, overlaps, resize pressure (measure)" },
  surface: { file: "surface.ts", summary: "Surface contract vs runtime state (inspect --surface <Kind>)" },
  surfaces: { file: "surfaces.ts", summary: "Enumerate known surface contracts" },
  act: { file: "act.ts", summary: "Perform actions against a target (set-input, key, click, ...)" },
  events: { file: "events.ts", summary: "Protocol bus + app log tailing (tail | record | logs | crashes)" },
  notes: { file: "notes.ts", summary: "Notes-specific inspection and resize compare (inspect | resize-compare)" },
  actions: { file: "actions.ts", summary: "Actions dialog inspection" },
  "agent-chat": { file: "agent_chat.ts", summary: "Agent Chat surface inspection" },
  dictation: { file: "dictation.ts", summary: "Dictation surface inspection" },
  media: { file: "media.ts", summary: "Screenshot/media capture helpers" },
  measure: { file: "measure.ts", summary: "Generic measurement helpers" },
  compare: { file: "compare.ts", summary: "Compare receipts/screenshots" },
  coverage: { file: "coverage.ts", summary: "Automation coverage report" },
  inspect: { file: "inspect.ts", summary: "General inspection entry" },
  investigate: { file: "investigate.ts", summary: "Story investigation loop (red proof -> green proof)" },
  perf: { file: "perf.ts", summary: "Performance probes" },
  main: { file: "main.ts", summary: "Main window inspection" },
  driver: { file: "driver.ts", summary: "Event-driven app driver (smoke | attach-smoke [session])" },
  schema: { file: "schema.ts", summary: "Receipt schema and classification vocabulary" },
};

function list() {
  const width = Math.max(...Object.keys(TOOLS).map((name) => name.length));
  const lines = Object.entries(TOOLS)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([name, tool]) => `  ${name.padEnd(width)}  ${tool.summary}`);
  console.log(["Usage: bun scripts/devtools/devtools.ts <tool> [args...]", "", "Tools:", ...lines].join("\n"));
}

const [tool, ...rest] = Bun.argv.slice(2);
if (!tool || tool === "list" || tool === "--help" || tool === "-h") {
  list();
  process.exit(tool && tool === "list" ? 0 : tool ? 0 : 2);
}

const entry = TOOLS[tool];
if (!entry) {
  console.error(`Unknown tool '${tool}'. Run: bun scripts/devtools/devtools.ts list`);
  process.exit(2);
}

const unsafeTool = ["act", "driver", "perf"].includes(tool);
const unsafeFlags = rest.some((argument) => [
  "--start",
  "--show",
  "--open",
  "--open-actions",
  "--prove-hover",
  "--prove-click-select",
  "--prove-click-activate",
].includes(argument));
if (unsafeTool || unsafeFlags) {
  assertNoninteractiveVisualProbe(`devtools.${tool}`);
}

const proc = Bun.spawn(["bun", join(import.meta.dir, entry.file), ...rest], {
  stdin: "inherit",
  stdout: "inherit",
  stderr: "inherit",
});
process.exit(await proc.exited);
