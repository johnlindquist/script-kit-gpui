#!/usr/bin/env bun
import { spawn } from "bun";
import { resolve, basename } from "path";
import { mkdirSync, writeFileSync, readFileSync } from "fs";
import { assertNoninteractiveVisualProbe } from "./lib/operator-safety.ts";

type JsonObject = Record<string, unknown>;

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/perf.ts record --pid <pid> --template <name> [--output <path>] [--duration <sec>]",
    "  bun scripts/devtools/perf.ts analyze --input <path> [--output <json_path>]",
  ].join("\n");
}

async function run(cmd: string[]): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = spawn(cmd, { stdout: "pipe", stderr: "pipe" });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const exitCode = await proc.exited;
  return { stdout, stderr, exitCode };
}

async function record(args: { pid: number; template: string; output?: string; duration?: number }) {
  const output = args.output ?? `perf_${Date.now()}.trace`;
  const cmd = ["xcrun", "xctrace", "record", "--template", args.template, "--attach", String(args.pid), "--output", output];
  
  console.error(`[*] Recording with template "${args.template}" on PID ${args.pid}...`);
  console.error(`[*] Output: ${output}`);

  const proc = spawn(cmd, { stdio: ["inherit", "inherit", "inherit"] });
  
  if (args.duration) {
    setTimeout(() => {
      console.error(`[*] Duration reached (${args.duration}s). Stopping...`);
      proc.kill("SIGINT");
    }, args.duration * 1000);
  }

  const exitCode = await proc.exited;
  if (exitCode !== 0 && exitCode !== 130) { // 130 is SIGINT
    console.error(`[!] xctrace exited with code ${exitCode}`);
    process.exit(1);
  }

  console.log(JSON.stringify({
    status: "ok",
    command: "record",
    output: resolve(output),
    template: args.template,
    pid: args.pid,
  }, null, 2));
}

async function analyze(args: { input: string; output?: string }) {
  const xmlOutput = `${args.input}.xml`;
  const tocOutput = `${args.input}.toc.xml`;
  
  console.error(`[*] Exporting trace TOC...`);
  await run(["xcrun", "xctrace", "export", "--input", args.input, "--output", tocOutput, "--toc"]);
  const tocContent = readFileSync(tocOutput, "utf8");

  console.error(`[*] Exporting leak data (if any)...`);
  // Try common leak-related xpaths
  const exportCmd = ["xcrun", "xctrace", "export", "--input", args.input, "--output", xmlOutput, "--xpath", "//table[contains(@schema, 'leak')] | //row[contains(@category, 'Leak')]"];
  const { exitCode, stderr } = await run(exportCmd);
  
  if (exitCode !== 0) {
    console.error(`[!] xctrace export failed: ${stderr}`);
    process.exit(1);
  }

  const xmlContent = readFileSync(xmlOutput, "utf8");
  const result: JsonObject = {
    status: "ok",
    command: "analyze",
    input: resolve(args.input),
    summary: {},
  };

  const hasLeaksTrack = tocContent.includes('name="Leaks"');
  
  if (xmlContent.includes("<row") || xmlContent.includes("<leak")) {
    const leakCount = (xmlContent.match(/<row /g) || xmlContent.match(/<leak /g) || []).length;
    result.summary = {
      type: "leaks",
      leakCount,
      leaksFound: true,
    };
    
    // Extract first few leaks
    const rows = xmlContent.match(/<row [^>]+>/g) || xmlContent.match(/<leak [^>]+>/g) || [];
    (result.summary as any).leaks = rows.slice(0, 10);
  } else if (hasLeaksTrack) {
    result.summary = {
      type: "leaks",
      leakCount: 0,
      leaksFound: false,
      message: "Leak detection scan completed. No leaks detected in this recording session.",
    };
  } else {
    result.summary = {
        type: "unknown",
        message: "No recognized performance data found in export. If this was a leak test, the Leaks instrument might not have triggered.",
    };
  }

  if (args.output) {
    writeFileSync(args.output, JSON.stringify(result, null, 2));
    console.error(`[*] Results written to ${args.output}`);
  }

  console.log(JSON.stringify(result, null, 2));
}

export async function main() {
  const argv = process.argv.slice(2);
  const command = argv[0];
  if (command === "--help" || command === "-h") {
    console.log(usage());
    return;
  }
  if (command === "record" || command === "analyze") {
    assertNoninteractiveVisualProbe(`performance.${command}`);
  }
  
  if (command === "record") {
    const params: any = {};
    for (let i = 1; i < argv.length; i++) {
      if (argv[i] === "--pid") params.pid = Number(argv[++i]);
      if (argv[i] === "--template") params.template = argv[++i];
      if (argv[i] === "--output") params.output = argv[++i];
      if (argv[i] === "--duration") params.duration = Number(argv[++i]);
    }
    if (!params.pid || !params.template) {
      console.error(usage());
      process.exit(1);
    }
    await record(params);
  } else if (command === "analyze") {
    const params: any = {};
    for (let i = 1; i < argv.length; i++) {
      if (argv[i] === "--input") params.input = argv[++i];
      if (argv[i] === "--output") params.output = argv[++i];
    }
    if (!params.input) {
      console.error(usage());
      process.exit(1);
    }
    await analyze(params);
  } else {
    console.error(usage());
    process.exit(1);
  }
}

if (import.meta.main) await main();
