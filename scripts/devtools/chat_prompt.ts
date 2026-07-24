#!/usr/bin/env bun

import { inspectAiReliabilityFixture } from "./ai_reliability_cli.ts";

const argv = Bun.argv.slice(2);
if (argv[0] !== "inspect") {
  console.error(
    "Usage: bun scripts/devtools/chat_prompt.ts inspect --fixture image-1-client-too-old [--strict]",
  );
  process.exit(2);
}
const fixtureIndex = argv.indexOf("--fixture");
const fixture = fixtureIndex >= 0 ? argv[fixtureIndex + 1] : undefined;
if (!fixture) {
  console.error("--fixture is required");
  process.exit(2);
}
await inspectAiReliabilityFixture(
  "script-kit-devtools.chat_prompt",
  fixture,
  "chatPrompt",
  argv.includes("--strict"),
);
