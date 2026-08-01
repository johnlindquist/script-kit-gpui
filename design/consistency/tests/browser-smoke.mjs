import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const consistency = path.resolve(here, "..");
const root = path.resolve(consistency, "../..");
const manifest = JSON.parse(fs.readFileSync(path.join(consistency, "data/groups.json"), "utf8"));
const chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const artifacts = path.join(consistency, "artifacts");

if (!fs.existsSync(chrome)) {
  console.error("BLOCKED_BROWSER: Google Chrome executable not found");
  process.exit(2);
}

function mime(file) {
  if (file.endsWith(".html")) return "text/html; charset=utf-8";
  if (file.endsWith(".css")) return "text/css; charset=utf-8";
  if (file.endsWith(".js") || file.endsWith(".mjs")) return "text/javascript; charset=utf-8";
  if (file.endsWith(".json")) return "application/json; charset=utf-8";
  if (file.endsWith(".svg")) return "image/svg+xml";
  return "application/octet-stream";
}

const server = http.createServer((request, response) => {
  const url = new URL(request.url, "http://127.0.0.1");
  const requested = decodeURIComponent(url.pathname === "/" ? "/design/consistency/index.html" : url.pathname);
  const file = path.resolve(root, `.${requested}`);
  if (!file.startsWith(root) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    response.writeHead(404);
    response.end("Not found");
    return;
  }
  response.writeHead(200, { "content-type": mime(file), "cache-control": "no-store" });
  fs.createReadStream(file).pipe(response);
});

function runChrome(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(chrome, [
      "--headless=new",
      "--disable-gpu",
      "--no-sandbox",
      "--disable-background-networking",
      "--disable-default-apps",
      "--disable-extensions",
      "--disable-sync",
      "--hide-scrollbars",
      "--virtual-time-budget=2500",
      ...args,
    ], { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) reject(new Error(`Chrome exited ${code}: ${stderr.slice(-1000)}`));
      else resolve({ stdout, stderr });
    });
  });
}

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const address = server.address();
const base = `http://127.0.0.1:${address.port}`;

try {
  const index = await runChrome(["--dump-dom", `${base}/design/consistency/index.html`]);
  assert.match(index.stdout, /data-group-card="proof-truth"/);
  assert.equal([...index.stdout.matchAll(/data-group-card=/g)].length, manifest.groupCount);
  assert.doesNotMatch(index.stdout, /Review explorer could not load/);

  for (const group of manifest.groups) {
    const result = await runChrome(["--dump-dom", `${base}/design/consistency/groups/${group.id}.html`]);
    const scenes = [...result.stdout.matchAll(/<article class="cx-scene"/g)].length;
    assert.equal(scenes, group.taskRecords.length, `${group.id} scene count`);
    assert.match(result.stdout, /PROPOSAL · NOT IMPLEMENTED/);
    assert.match(result.stdout, /CURRENT · SOURCE-DERIVED/);
    assert.doesNotMatch(result.stdout, /Review explorer could not load/);
    for (const task of group.taskRecords) {
      assert.match(result.stdout, new RegExp(`data-scene-id="${task.id}"`));
    }
  }

  const narrow = await runChrome([
    "--window-size=390,844",
    "--dump-dom",
    `${base}/design/consistency/groups/context-identity.html`,
  ]);
  assert.match(narrow.stdout, /data-render-ready="true"/);
  assert.match(narrow.stdout, /data-horizontal-overflow="false"/);

  const interaction = await runChrome([
    "--window-size=1024,900",
    "--dump-dom",
    `${base}/design/consistency/groups/context-identity.html?selfTest=1`,
  ]);
  assert.match(interaction.stdout, /data-self-test="pass"/);

  fs.mkdirSync(artifacts, { recursive: true });
  await runChrome([
    "--window-size=1440,1000",
    `--screenshot=${path.join(artifacts, "index-1440x1000.png")}`,
    `${base}/design/consistency/index.html`,
  ]);
  await runChrome([
    "--window-size=1440,1100",
    `--screenshot=${path.join(artifacts, "context-identity-1440x1100.png")}`,
    `${base}/design/consistency/groups/context-identity.html`,
  ]);
  await runChrome([
    "--window-size=390,844",
    `--screenshot=${path.join(artifacts, "context-identity-390x844.png")}`,
    `${base}/design/consistency/groups/context-identity.html`,
  ]);

  for (const name of ["index-1440x1000.png", "context-identity-1440x1100.png", "context-identity-390x844.png"]) {
    const file = path.join(artifacts, name);
    assert.ok(fs.existsSync(file));
    assert.ok(fs.statSync(file).size > 20_000, `${name} is unexpectedly small`);
  }

  console.log(`browser smoke: PASS (${manifest.groups.length} groups, ${manifest.taskCount} task scenes)`);
  console.log(`artifacts: ${path.relative(root, artifacts)}`);
} finally {
  await new Promise((resolve) => server.close(resolve));
}
