// Name: SDK Test - Capability Boundaries
// Description: Supported prompts stay truthful; unsupported APIs reject before dispatch.

interface Outcome {
  test: string;
  status: "running" | "pass" | "fail";
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
}

function emit(test: string, status: Outcome["status"], details: Partial<Outcome> = {}): void {
  console.log(JSON.stringify({ test, status, timestamp: new Date().toISOString(), ...details }));
}

async function verify(test: string, assertion: () => Promise<unknown>): Promise<void> {
  emit(test, "running");
  const started = Date.now();
  try {
    const result = await assertion();
    emit(test, "pass", { result, duration_ms: Date.now() - started });
  } catch (error) {
    emit(test, "fail", { error: String(error), duration_ms: Date.now() - started });
  }
}

async function expectUnsupportedWithoutDispatch(
  feature: string,
  operation: () => unknown,
): Promise<void> {
  const writes: string[] = [];
  const originalWrite = process.stdout.write;
  process.stdout.write = ((chunk: unknown, ..._args: unknown[]) => {
    writes.push(String(chunk));
    return true;
  }) as typeof process.stdout.write;

  let failure: any;
  try {
    await operation();
  } catch (error) {
    failure = error;
  } finally {
    process.stdout.write = originalWrite;
  }

  if (
    failure?.name !== "UnsupportedSdkFeatureError" ||
    failure?.code !== "ERR_UNSUPPORTED_SDK_FEATURE" ||
    failure?.supported !== false ||
    failure?.feature !== feature ||
    !Array.isArray(failure?.alternatives) ||
    failure.alternatives.length === 0
  ) {
    throw new Error(`Expected typed ${feature} failure, got ${String(failure)}`);
  }
  if (writes.length > 0) {
    throw new Error(`${feature} dispatched unsupported protocol traffic: ${writes.join("")}`);
  }
}

await verify("supported-prompts-have-no-false-unsupported-warning", async () => {
  const warnings: string[] = [];
  const originalWarn = console.warn;
  console.warn = (...args: unknown[]) => warnings.push(args.map(String).join(" "));

  try {
    const compact = await mini("Pick", ["compact"]);
    const minimal = await micro("Pick", ["minimal"]);
    const shortcut = await hotkey("Record a shortcut");
    const formValues = await fields([
      { name: "query", label: "Query", type: "search", value: "needle" },
    ]);

    if (
      compact !== "compact" ||
      minimal !== "minimal" ||
      typeof shortcut.shortcut !== "string" ||
      JSON.stringify(formValues) !== JSON.stringify(["needle"])
    ) {
      throw new Error("Supported prompt result contracts did not match expected values.");
    }
  } finally {
    console.warn = originalWarn;
  }

  if (warnings.length > 0) {
    throw new Error(`Supported prompts emitted misleading warnings: ${warnings.join("; ")}`);
  }
  return { features: ["mini", "micro", "hotkey", "fields"], warnings: 0 };
});

const unsupported = [
  { feature: "widget", operation: () => widget("<div>No widget</div>") },
  { feature: "find", operation: () => find("Find", { onlyin: "/tmp" }) },
  { feature: "setPanel", operation: () => setPanel("<div>No panel</div>") },
  { feature: "setPreview", operation: () => setPreview("<div>No preview</div>") },
  { feature: "setPrompt", operation: () => setPrompt("<div>No prompt</div>") },
  { feature: "keyboard.type", operation: () => keyboard.type("never injected") },
  { feature: "keyboard.tap", operation: () => keyboard.tap("command", "k") },
  { feature: "mouse.move", operation: () => mouse.move([{ x: 0, y: 0 }]) },
  { feature: "mouse.leftClick", operation: () => mouse.leftClick() },
  { feature: "mouse.rightClick", operation: () => mouse.rightClick() },
  { feature: "mouse.setPosition", operation: () => mouse.setPosition({ x: 0, y: 0 }) },
] as const;

for (const { feature, operation } of unsupported) {
  await verify(`unsupported-${feature}-fails-before-dispatch`, async () => {
    await expectUnsupportedWithoutDispatch(feature, operation);
    return { feature, protocolMessages: 0 };
  });
}

await verify("unsupported-media-apis-are-declared-without-accessing-devices", async () => {
  for (const [name, capability] of [
    ["webcam", webcam],
    ["mic", mic],
    ["eyeDropper", eyeDropper],
  ] as const) {
    if (typeof capability !== "function") {
      throw new Error(`${name} must remain a declared compatibility boundary.`);
    }
  }
  return { devicesAccessed: false, features: ["webcam", "mic", "eyeDropper"] };
});
