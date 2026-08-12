#!/usr/bin/env bun

import { Driver, type Json } from "../devtools/driver.ts";

type ElementNode = Record<string, unknown>;

const INPUT = ".";
const SOURCE_APP = "Preview Fixture App";
const SEEDED_SELECTION =
  "P01_SELECTION_ALPHA private fixture text P01_SELECTION_OMEGA";
const SEEDED_SUBSTRINGS = [
  "P01_SELECTION_ALPHA",
  "P01_SELECTION_OMEGA",
] as const;
const EXPECTED_TARGET = `Rewrites your selection in ${SOURCE_APP}`;
const EXPECTED_ROWS = [
  ["Professional", "Polished workplace tone"],
  ["Concise", "Shorten without losing meaning"],
  ["Friendly", "Warmer tone"],
  ["Direct", "Plainspoken and direct"],
] as const;
const EXPECTED_HEADER = `Styles · ${EXPECTED_TARGET}`;
const CLOSED_TARGET_PATTERNS = [
  /^Rewrites your selection in .+$/,
  /^Rewrites your draft in .+$/,
  /^Rewrites the selected or focused text in .+$/,
  /^Rewrites the selected or focused text$/,
] as const;

function elementNodes(result: Json): ElementNode[] {
  return Array.isArray(result.elements)
    ? (result.elements as ElementNode[])
    : [];
}

function collectStrings(value: unknown, out: string[] = []): string[] {
  if (typeof value === "string") {
    out.push(value);
  } else if (Array.isArray(value)) {
    for (const item of value) collectStrings(item, out);
  } else if (value !== null && typeof value === "object") {
    for (const item of Object.values(value as Record<string, unknown>)) {
      collectStrings(item, out);
    }
  }
  return out;
}
function fingerprint(value: string): string {
  return `sha256:${new Bun.CryptoHasher("sha256").update(value).digest("hex")}`;
}

function contentFingerprint(
  node: ElementNode,
  field: "text" | "value",
): string | null {
  const content =
    node.content !== null && typeof node.content === "object"
      ? (node.content as Record<string, unknown>)
      : null;
  const descriptor =
    content?.[field] !== null && typeof content?.[field] === "object"
      ? (content[field] as Record<string, unknown>)
      : null;
  return typeof descriptor?.fingerprint === "string"
    ? descriptor.fingerprint
    : null;
}

function isSectionHeader(node: ElementNode): boolean {
  return node.role === "sectionHeader" || node.kind === "sectionHeader";
}

function styleSectionScope(elements: ElementNode[]): ElementNode[] | null {
  const start = elements.findIndex(
    (node) =>
      isSectionHeader(node) &&
      (node.text === "Styles" ||
        String(node.semanticId ?? "")
          .toLowerCase()
          .includes("styles")),
  );
  if (start < 0) return null;

  const next = elements.findIndex(
    (node, index) => index > start && isSectionHeader(node),
  );
  return elements.slice(start, next < 0 ? elements.length : next);
}

function matchingStyleRows(scope: ElementNode[]): ElementNode[] {
  return scope.filter(
    (node) =>
      node.role === "row" &&
      node.kind === "style" &&
      EXPECTED_ROWS.some(
        ([title]) => contentFingerprint(node, "text") === fingerprint(title),
      ),
  );
}

function rowReceipt(node: ElementNode): Json {
  const matched = EXPECTED_ROWS.find(
    ([title, subtitle]) =>
      contentFingerprint(node, "text") === fingerprint(title) &&
      contentFingerprint(node, "value") === fingerprint(subtitle),
  );
  return {
    semanticId: typeof node.semanticId === "string" ? node.semanticId : null,
    title: matched?.[0] ?? null,
    subtitle: matched?.[1] ?? null,
    titleFingerprint: contentFingerprint(node, "text"),
    subtitleFingerprint: contentFingerprint(node, "value"),
    role: typeof node.role === "string" ? node.role : null,
    kind: typeof node.kind === "string" ? node.kind : null,
  };
}

async function waitForStyleSnapshot(driver: Driver): Promise<{
  response: Json;
  scope: ElementNode[];
  rows: ElementNode[];
}> {
  const deadline = performance.now() + 5_000;
  let lastResponse: Json = {};
  while (performance.now() < deadline) {
    lastResponse = await driver.getElements(
      { includeHeaders: true, limit: 64 },
      { timeoutMs: 2_000 },
    );
    const scope = styleSectionScope(elementNodes(lastResponse));
    if (scope) {
      const rows = matchingStyleRows(scope);
      const rowsReady = EXPECTED_ROWS.every(([title, subtitle]) =>
        rows.some(
          (row) =>
            contentFingerprint(row, "text") === fingerprint(title) &&
            contentFingerprint(row, "value") === fingerprint(subtitle),
        ),
      );
      const headerReady = scope[0]?.value === EXPECTED_HEADER;
      if (rowsReady && headerReady) {
        return { response: lastResponse, scope, rows };
      }
    }
    await Bun.sleep(25);
  }

  throw new Error(
    `Styles semantic snapshot did not settle: ${JSON.stringify(lastResponse)}`,
  );
}

const receipt: Json = {
  schemaVersion: 1,
  probe: "spine-style-preview",
  status: "fail",
  input: INPUT,
  targetTemplate: null,
  section: null,
  rows: [],
  assertions: {},
  cleanup: { closed: false },
  error: null,
};

let driver: Driver | null = null;
try {
  driver = await Driver.launch({
    sessionName: "spine-style-preview-probe",
    sandboxHome: true,
    sharedModels: false,
    defaultTimeoutMs: 5_000,
    env: {
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_TEST_SPINE_STYLE_SELECTION_TEXT: SEEDED_SELECTION,
      SCRIPT_KIT_TEST_SPINE_STYLE_SELECTION_KIND: "selection",
      SCRIPT_KIT_TEST_SPINE_STYLE_SOURCE_APP: SOURCE_APP,
    },
  });

  driver.send({ type: "show" });
  await driver.waitForState({ windowVisible: true }, { timeoutMs: 5_000 });
  await driver.simulateGpuiKeyDown(INPUT, {
    text: INPUT,
    target: { type: "id", id: "main" },
    timeoutMs: 5_000,
  });
  await driver.waitForState({ inputValue: INPUT }, { timeoutMs: 5_000 });
  const typedState = await driver.getState({ timeoutMs: 2_000 });

  const snapshot = await waitForStyleSnapshot(driver);
  const sectionHeader = snapshot.scope[0] ?? null;
  const headerValue =
    typeof sectionHeader?.value === "string" ? sectionHeader.value : null;
  const targetTemplate = headerValue?.startsWith("Styles · ")
    ? headerValue.slice("Styles · ".length)
    : null;
  const rows = snapshot.rows.map(rowReceipt);
  const rowDescriptionsMatch = EXPECTED_ROWS.every(([title, subtitle]) =>
    rows.some((row) => row.title === title && row.subtitle === subtitle),
  );
  const targetUsesClosedTemplate =
    targetTemplate !== null &&
    CLOSED_TARGET_PATTERNS.some((pattern) => pattern.test(targetTemplate));
  const semanticSectionPresent =
    sectionHeader !== null && isSectionHeader(sectionHeader);
  const seededTextAbsent = SEEDED_SUBSTRINGS.every(
    (substring) => !JSON.stringify(snapshot.scope).includes(substring),
  );
  const typedDot = typedState.inputValue === INPUT;

  receipt.targetTemplate = targetTemplate;
  receipt.section = sectionHeader
    ? {
        semanticId:
          typeof sectionHeader.semanticId === "string"
            ? sectionHeader.semanticId
            : null,
        title: headerValue,
        role:
          typeof sectionHeader.role === "string" ? sectionHeader.role : null,
        kind:
          typeof sectionHeader.kind === "string" ? sectionHeader.kind : null,
      }
    : null;
  receipt.rows = rows;
  receipt.assertions = {
    typedDot,
    semanticSectionPresent,
    rowDescriptionsMatch,
    targetUsesClosedTemplate,
    exactSelectionTemplate: targetTemplate === EXPECTED_TARGET,
    seededTextAbsent,
  };
  const pass = Object.values(receipt.assertions).every(Boolean);
  receipt.status = pass ? "pass" : "fail";
} catch (error) {
  receipt.error = error instanceof Error ? error.message : String(error);
} finally {
  if (driver) {
    try {
      await driver.close();
      receipt.cleanup = { closed: !driver.alive };
    } catch (error) {
      receipt.cleanup = {
        closed: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }
}

const passed =
  receipt.status === "pass" &&
  (receipt.cleanup as { closed?: boolean }).closed === true;
console.log(JSON.stringify(receipt));
if (!passed) process.exitCode = 1;
