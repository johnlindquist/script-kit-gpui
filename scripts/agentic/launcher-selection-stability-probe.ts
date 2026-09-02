#!/usr/bin/env bun
import { runDesign } from "../devtools/design.ts";
import { EvaluationContractError } from "../devtools/lib/owned-evaluation.ts";
import { searchContractSpec } from "./launcher-search-contract.ts";

export async function runSelectionProbe(argv: string[]): Promise<void> {
  const command = argv[0] ?? "discover";
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log("launcher-selection-stability-probe <discover|spec|run> [--artifact <reference.json> --out <fresh-directory> --case <case-id> --shard <zero-based-index>]\nDiscover/spec are passive. Run uses only the hidden owned evaluator; omitted schedules remain explicitly uncovered.");
    return;
  }
  if (command === "discover" || command === "spec" || argv.includes("--describe-contract")) { console.log(JSON.stringify(searchContractSpec())); return; }
  if (command !== "run") throw new EvaluationContractError("expected-discover-spec-or-run");
  const mapped = argv.slice(1).map(arg => arg === "--case" ? "--search-case" : arg === "--shard" ? "--search-shard" : arg);
  await runDesign(["run", "--scenario", "launcher-ranking-provider", ...mapped]);
}
if (import.meta.main) await runSelectionProbe(Bun.argv.slice(2));
