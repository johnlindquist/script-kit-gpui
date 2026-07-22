#!/usr/bin/env bun

type SurfaceContract = {
  surfaceKind: string;
  appViewVariants: string[];
  appViewFooters: Array<{ variant: string; nativeFooterSurface: string | null }>;
  vocabulary?: {
    family?: string;
    inputOwnership?: string;
    previewRole?: string;
  };
  focusPolicy?: string;
  keyboardPolicy?: string;
  actionsPolicy?: string;
  proofPolicy?: string;
  visualPolicy?: string;
  dismissPolicy?: {
    policy: string;
    windowBlur: string;
    backdropClick: string;
    escape: string;
    cmdW: string;
  };
  automationSemanticSurface?: string;
};

type FeatureMapEntry = {
  id: string;
  feature: string;
  cluster: string;
  primaryOwners: string[];
  rawOracle: string;
  chapter: string;
};

type OracleBatch = {
  id: string;
  name: string;
  priority: number;
  outsideInPhase: "window-container" | "surface-shell" | "content-controls" | "supporting-systems";
  priorityRationale: string;
  owners: string[];
  surfaceKinds: string[];
  featureIds: string[];
  requiredDevToolsPrimitives: string[];
  questionsForOracle: string[];
};

const root = new URL("../..", import.meta.url);

const paths = {
  contracts: "docs/ai/contracts/surface-contracts.json",
  coverage: "scripts/devtools/coverage.ts",
};

const coverageSurfaceAliases = [
  { alias: "main", resolvesTo: { surfaceKind: "ScriptList" }, countsAsCoverage: false },
  { alias: "actions-dialog", resolvesTo: { surfaceKind: "ActionsDialog" }, countsAsCoverage: false },
  { alias: "dictation-history", resolvesTo: { surfaceKind: "AttachmentPortalBrowser", appViewVariant: "DictationHistoryView" }, countsAsCoverage: false },
  { alias: "notes-agent_chat", resolvesTo: { surfaceKind: "AgentChat", hostKind: "NotesWindow" }, countsAsCoverage: false },
] as const;

const liquidGlassAuditExclusions: readonly { surfaceKind: string; reason: string }[] = [];

function aliasesForSurfaceKind(surfaceKind: string) {
  return coverageSurfaceAliases.filter((alias) => alias.resolvesTo.surfaceKind === surfaceKind).map((alias) => alias.alias);
}

function readText(path: string) {
  return Bun.file(new URL(path, root)).text();
}

function parseArgs(argv: string[]) {
  return {
    markdown: argv.includes("--markdown"),
  };
}

function extractCoverageSurfaceIds(source: string) {
  const surfacesStart = source.indexOf("const surfaces: Surface[] = [");
  const argsStart = source.indexOf("function parseArgs", surfacesStart);
  const surfacesSource = surfacesStart >= 0 && argsStart > surfacesStart ? source.slice(surfacesStart, argsStart) : source;
  return [...surfacesSource.matchAll(/\bid:\s*"([^"]+)"/g)].map((match) => match[1]);
}

function parseOwners(raw: string) {
  return [...raw.matchAll(/`([^`]+)`/g)].map((match) => match[1]);
}

function parseLink(raw: string) {
  const match = raw.match(/\]\(([^)]+)\)/);
  return match?.[1] ?? "";
}

function parseFeatureMap(markdown: string): FeatureMapEntry[] {
  return markdown
    .split("\n")
    .filter((line) => /^\|\s*\d{3}\s*\|/.test(line))
    .map((line) => {
      const cells = line
        .slice(1, -1)
        .split("|")
        .map((cell) => cell.trim());
      return {
        id: cells[0],
        feature: cells[1],
        cluster: cells[2],
        primaryOwners: parseOwners(cells[4] ?? ""),
        rawOracle: parseLink(cells[5] ?? ""),
        chapter: parseLink(cells[6] ?? ""),
      };
    });
}

function featureIds(entries: FeatureMapEntry[], owners: string[], terms: string[]) {
  return entries
    .filter((entry) => {
      const haystack = `${entry.feature} ${entry.cluster}`.toLowerCase();
      return (
        entry.primaryOwners.some((owner) => owners.includes(owner)) ||
        terms.some((term) => haystack.includes(term.toLowerCase()))
      );
    })
    .map((entry) => entry.id);
}

function buildOracleBatches(contracts: SurfaceContract[], features: FeatureMapEntry[]): OracleBatch[] {
  const surfaceKinds = new Set(contracts.map((entry) => entry.surfaceKind));
  const keepKinds = (kinds: string[]) => kinds.filter((kind) => surfaceKinds.has(kind));

  return [
    {
      id: "platform-windowing-permissions",
      name: "Platform windows, containers, materials, resizing, screenshots, lifecycle",
      priority: 1,
      outsideInPhase: "window-container",
      priorityRationale: "Highest impact: outer windows, materials, safe areas, lifecycle, and resize behavior constrain every inner layout.",
      owners: ["platform-windowing-macos", "window-resizing", "launcher-surface-contracts"],
      surfaceKinds: keepKinds(["About", "Feedback"]),
      featureIds: featureIds(features, ["platform-windowing-macos", "window-resizing", "launcher-surface-contracts"], [
        "window",
        "permission",
        "tray",
        "sizing",
      ]),
      requiredDevToolsPrimitives: ["devtools.windows.inspect", "devtools.permissions.inspect", "devtools.visual.compare", "devtools.lifecycle.trace"],
      questionsForOracle: [
        "How should Script Kit prove outer window/container material, resize, safe-area, and backdrop behavior before auditing inner controls?",
        "Which permission and screenshot receipts stay passive and avoid changing macOS settings?",
      ],
    },
    {
      id: "launcher-main-actions",
      name: "Launcher, main menu, source filters, actions, shortcuts, aliases",
      priority: 2,
      outsideInPhase: "surface-shell",
      priorityRationale: "Main window shell, launcher container, action popup container, and footer chrome define the default app layout.",
      owners: ["main-menu-search-selection", "actions-popups", "keyboard-focus-routing"],
      surfaceKinds: keepKinds(["ScriptList", "ActionsDialog", "ConfirmPrompt"]),
      featureIds: featureIds(features, ["main-menu-search-selection", "actions-popups", "keyboard-focus-routing"], [
        "main menu",
        "source filter",
        "shortcut",
        "alias",
      ]),
      requiredDevToolsPrimitives: ["devtools.targets.watch", "devtools.act", "devtools.measure.layout", "devtools.measure.text"],
      questionsForOracle: [
        "Which target-scoped action and shortcut receipts make main-menu bugs reproducible without recipes?",
        "How should actions popups expose route stack, anchor rects, disabled reasons, and clipping metrics?",
      ],
    },
    {
      id: "prompt-runtime-family",
      name: "Prompt runtime family, child content, terminal, editor, forms, path, drop, env, confirm",
      priority: 3,
      outsideInPhase: "surface-shell",
      priorityRationale: "Prompt windows and child-content containers are the broadest SDK-facing layout shells after the launcher.",
      owners: ["prompt-runtime", "sdk-script-execution", "quick-terminal-pty", "file-search-portals"],
      surfaceKinds: keepKinds(["PromptEntity", "PromptChildContent", "ExplicitPromptEntity", "UtilityChildContent", "Webcam", "ConfirmPrompt"]),
      featureIds: featureIds(features, ["prompt-runtime", "sdk-script-execution", "quick-terminal-pty", "file-search-portals"], [
        "prompt",
        "term",
        "editor",
        "path",
        "drop",
        "env",
      ]),
      requiredDevToolsPrimitives: ["devtools.prompt.inspect", "devtools.measure.scroll", "devtools.measure.selection", "devtools.act.safeSubmit"],
      questionsForOracle: [
        "What per-prompt contract fields should exist before an agent can call a prompt UX bug reproduced?",
        "How should oversized div, md, editor, terminal, and form containers report scrollability and resize pressure?",
      ],
    },
    {
      id: "builtins-filterable",
      name: "Built-in filterable views and split-preview rows",
      priority: 4,
      outsideInPhase: "surface-shell",
      priorityRationale: "Filterable built-ins reuse shared list and preview containers; prove the shell before row-level controls.",
      owners: ["builtin-filterable-surfaces", "theme-config-preferences", "storage-cache-security"],
      surfaceKinds: keepKinds([
        "ClipboardHistory",
        "AppLauncher",
        "WindowSwitcher",
        "BrowserTabs",
        "GenericFilterableList",
        "Settings",
        "KitStoreBrowse",
        "KitStoreInstalled",
        "ProcessManager",
        "CurrentAppCommands",
        "ThemeChooser",
        "EmojiPicker",
        "AgentChatHistory",
      ]),
      featureIds: featureIds(features, ["builtin-filterable-surfaces", "theme-config-preferences", "storage-cache-security"], [
        "built-in",
        "clipboard",
        "settings",
        "theme",
      ]),
      requiredDevToolsPrimitives: ["devtools.resources.inspect", "devtools.measure.preview", "devtools.list.diff", "devtools.storage.fingerprint"],
      questionsForOracle: [
        "Which shared list, preview, cache, and privacy receipts cover all filterable built-ins?",
        "How should split-preview surfaces expose preview overflow, stale selection, and row action availability?",
      ],
    },
    {
      id: "portals-resources-context",
      name: "File portals, attachment portals, MCP resources, context catalogs",
      priority: 5,
      outsideInPhase: "surface-shell",
      priorityRationale: "Portal windows and return containers affect attachment, resource, and context layouts before individual rows.",
      owners: ["file-search-portals", "mcp-context-resources", "agent_chat-context-composer"],
      surfaceKinds: keepKinds(["FileSearchMini", "FileSearchFull", "AttachmentPortalBrowser", "ScriptTemplateCatalog", "SdkReference"]),
      featureIds: featureIds(features, ["file-search-portals", "mcp-context-resources", "agent_chat-context-composer"], [
        "portal",
        "resource",
        "context",
        "file",
      ]),
      requiredDevToolsPrimitives: ["devtools.portal.inspect", "devtools.resources.inspect", "devtools.act.portalReturn", "devtools.privacy.redaction"],
      questionsForOracle: [
        "How should origin, return target, staged parts, and privacy-safe resource rows be proven across portals?",
        "Which receipts prevent agents from confusing portal fixture data with real user files or context?",
      ],
    },
    {
      id: "agent_chat-chat-ai",
      name: "Agent Chat chat, composer, history, SDK AI APIs, model setup",
      priority: 6,
      outsideInPhase: "content-controls",
      priorityRationale: "Agent Chat has important window shells, but after detached/window proof the remaining work is composer and transcript internals.",
      owners: ["agent_chat-chat-core", "agent_chat-context-composer", "sdk-script-execution"],
      surfaceKinds: keepKinds(["AgentChat", "AgentChatHistory", "AttachmentPortalBrowser", "GenericFilterableList"]),
      featureIds: featureIds(features, ["agent_chat-chat-core", "agent_chat-context-composer", "sdk-script-execution"], ["agent_chat", "agent chat", "ai"]),
      requiredDevToolsPrimitives: ["devtools.agent_chat.inspect", "devtools.agent_chat.timeline", "devtools.composer.inspect", "devtools.turn.diff"],
      questionsForOracle: [
        "What generation, host, composer, model, and context-part receipts are required for Agent Chat UI bugs?",
        "How should agents prove wrong-host, stale-turn, and delayed-action failures without starting external AI calls?",
      ],
    },
    {
      id: "notes-dictation-media",
      name: "Notes, notes-hosted Agent Chat, dictation, media, history, target delivery",
      priority: 7,
      outsideInPhase: "content-controls",
      priorityRationale: "Notes and Dictation have practical window proof; remaining work is embedded Agent Chat, media state, history, and delivery details.",
      owners: ["notes-window", "dictation-media", "agent_chat-chat-core"],
      surfaceKinds: keepKinds(["AgentChat", "AgentChatHistory", "ClipboardHistory"]),
      featureIds: featureIds(features, ["notes-window", "dictation-media", "agent_chat-chat-core"], ["notes", "dictation", "media"]),
      requiredDevToolsPrimitives: ["devtools.notes.inspect", "devtools.media.inspect", "devtools.measure.selection", "devtools.delivery.trace"],
      questionsForOracle: [
        "Which passive media, target-delivery, editor-selection, and notes-resize receipts unlock reliable Dictation and Notes bug proof?",
        "How should the tools separate visible Notes UI state, embedded Agent Chat state, and background storage state?",
      ],
    },
    {
      id: "observability-security-storage",
      name: "Observability, storage, sharing, security, diagnostics, replay",
      priority: 8,
      outsideInPhase: "supporting-systems",
      priorityRationale: "Supporting receipts and diagnostics are essential, but they should follow visible window/container proof.",
      owners: ["dev-loop-observability", "storage-cache-security", "testing-quality-gates"],
      surfaceKinds: keepKinds(["Feedback", "SdkReference", "ScriptTemplateCatalog"]),
      featureIds: featureIds(features, ["dev-loop-observability", "storage-cache-security", "testing-quality-gates"], [
        "logging",
        "diagnostics",
        "sharing",
        "storage",
      ]),
      requiredDevToolsPrimitives: ["devtools.events.tail", "devtools.storage.fingerprint", "devtools.security.inspect", "devtools.investigate"],
      questionsForOracle: [
        "What event, storage, privacy, and replay receipts should every investigation artifact include?",
        "How should missing primitive reports become a prioritized build backlog instead of failed bug investigations?",
      ],
    },
  ];
}

function buildReport(contractsText: string, featureMapText: string, coverageText: string) {
  const contracts = JSON.parse(contractsText) as {
    schemaVersion: number;
    generatedFrom: string;
    registry: string;
    entries: SurfaceContract[];
  };
  const featureMap = parseFeatureMap(featureMapText);
  const coverageSurfaceIds = extractCoverageSurfaceIds(coverageText);
  const coveredNames = new Set(coverageSurfaceIds);
  const contractSurfaceKinds = contracts.entries.map((entry) => entry.surfaceKind);
  const excludedAuditKinds = new Set(liquidGlassAuditExclusions.map((entry) => entry.surfaceKind));
  const auditContracts = contracts.entries.filter((entry) => !excludedAuditKinds.has(entry.surfaceKind));
  const contractFamilies = [...new Set(contracts.entries.map((entry) => entry.vocabulary?.family ?? "Unknown"))].sort();
  const ownerSkills = [...new Set(featureMap.flatMap((entry) => entry.primaryOwners))].sort();
  const batches = buildOracleBatches(auditContracts, featureMap);
  const serializeContract = (entry: SurfaceContract) => ({
    surfaceKind: entry.surfaceKind,
    appViewVariants: entry.appViewVariants,
    nativeFooterSurfaces: entry.appViewFooters
      .map((footer) => footer.nativeFooterSurface)
      .filter((footer): footer is string => Boolean(footer)),
    vocabulary: entry.vocabulary,
    focusPolicy: entry.focusPolicy,
    keyboardPolicy: entry.keyboardPolicy,
    actionsPolicy: entry.actionsPolicy,
    proofPolicy: entry.proofPolicy,
    visualPolicy: entry.visualPolicy,
    dismissPolicy: entry.dismissPolicy ?? null,
    automationSemanticSurface: entry.automationSemanticSurface,
    coverageAliases: aliasesForSurfaceKind(entry.surfaceKind),
  });

  return {
    schemaVersion: 1,
    tool: "script-kit-devtools.surfaces",
    generatedAt: new Date().toISOString(),
    philosophy:
      "Inventory app surfaces first, then build protocol/MCP/CLI DevTools primitives; scripted recipes remain regression packs after direct proof exists.",
    sourceArtifacts: [
      {
        path: paths.contracts,
        role: "Generated AppView to SurfaceKind contracts and proof policies.",
        generatedFrom: contracts.generatedFrom,
        registry: contracts.registry,
      },
      {
        path: paths.featureMap,
        role: "Feature ownership map and Oracle-backed chapters.",
      },
      {
        path: paths.coverage,
        role: "Currently checked-in DevTools domain and surface coverage.",
      },
    ],
    totals: {
      surfaceContractCount: contracts.entries.length,
      liquidGlassAuditSurfaceCount: auditContracts.length,
      appViewVariantCount: contracts.entries.reduce((count, entry) => count + entry.appViewVariants.length, 0),
      featureMapCount: featureMap.length,
      ownerSkillCount: ownerSkills.length,
      currentlyCoveredSurfacesCount: coverageSurfaceIds.length,
      oracleBatchCount: batches.length,
    },
    surfaceContracts: contracts.entries.map(serializeContract),
    auditSurfaceContracts: auditContracts.map(serializeContract),
    liquidGlassAuditExclusions,
    featureMap,
    existingDevToolsCoverage: {
      surfaceIds: coverageSurfaceIds,
      source: paths.coverage,
      note: "These are the only surfaces with explicit coverage.ts entries today; every other contract and feature family should be treated as backlog until a direct primitive exists.",
    },
    coverageSurfaceAliases,
    uncoveredContractSurfaceKinds: contractSurfaceKinds.filter((kind) => {
      const kebabKind = kind.replace(/[A-Z]/g, (char, index) => `${index ? "-" : ""}${char.toLowerCase()}`);
      return !coveredNames.has(kind) && !coveredNames.has(kebabKind);
    }),
    contractFamilies,
    ownerSkills,
    recommendedOracleBatches: batches,
    recommendedNext: [
      "Work outside-in: prove window/container material, resizing, and lifecycle before inner controls and content.",
      "Ask Oracle to turn each batch into inspect, measure, act, compare, media, resources, events, and investigate primitives.",
      "Add fail-closed CLI contracts before implementing runtime behavior so agents cannot confuse screenshots or recipes for proof.",
      "Promote recurring direct-primitive flows into agentic-testing recipes only after red/green receipts stabilize.",
    ],
  };
}

function markdown(report: ReturnType<typeof buildReport>) {
  const lines = [
    "# Script Kit DevTools Surface Inventory",
    "",
    report.philosophy,
    "",
    "## Totals",
    "",
    `- Surface contracts: ${report.totals.surfaceContractCount}`,
    `- Liquid Glass audit surfaces: ${report.totals.liquidGlassAuditSurfaceCount}`,
    `- AppView variants: ${report.totals.appViewVariantCount}`,
    `- Feature-map entries: ${report.totals.featureMapCount}`,
    `- Owner skills: ${report.totals.ownerSkillCount}`,
    `- Current explicit coverage surfaces: ${report.totals.currentlyCoveredSurfacesCount}`,
    `- Oracle batches: ${report.totals.oracleBatchCount}`,
    "",
    "## Surface Contracts",
    "",
    "| SurfaceKind | AppView variants | Family | Focus | Keyboard | Proof | Visual |",
    "| --- | --- | --- | --- | --- | --- | --- |",
    ...report.surfaceContracts.map((entry) =>
      `| ${entry.surfaceKind} | ${entry.appViewVariants.join(", ")} | ${entry.vocabulary?.family ?? ""} | ${entry.focusPolicy ?? ""} | ${entry.keyboardPolicy ?? ""} | ${entry.proofPolicy ?? ""} | ${entry.visualPolicy ?? ""} |`
    ),
    "",
    "## Liquid Glass Audit Exclusions",
    "",
    "| SurfaceKind | Reason |",
    "| --- | --- |",
    ...report.liquidGlassAuditExclusions.map((entry) => `| ${entry.surfaceKind} | ${entry.reason} |`),
    "",
    "## Current Explicit Coverage",
    "",
    report.existingDevToolsCoverage.surfaceIds.join(", "),
    "",
    "## Uncovered Contract SurfaceKinds",
    "",
    report.uncoveredContractSurfaceKinds.join(", "),
    "",
    "## Feature Map",
    "",
    "| ID | Feature | Cluster | Owners |",
    "| --- | --- | --- | --- |",
    ...report.featureMap.map((entry) => `| ${entry.id} | ${entry.feature} | ${entry.cluster} | ${entry.primaryOwners.join(", ")} |`),
    "",
    "## Oracle Batches",
    "",
    "| Priority | Phase | Batch | SurfaceKinds | Feature IDs | Required primitives |",
    "| --- | --- | --- | --- | --- | --- |",
    ...report.recommendedOracleBatches.map((batch) =>
      `| ${batch.priority} | ${batch.outsideInPhase} | ${batch.name} | ${batch.surfaceKinds.join(", ")} | ${batch.featureIds.join(", ")} | ${batch.requiredDevToolsPrimitives.join(", ")} |`
    ),
  ];
  return lines.join("\n");
}

const args = parseArgs(Bun.argv.slice(2));
const report = buildReport(await readText(paths.contracts), "", await readText(paths.coverage));

if (args.markdown) {
  console.log(markdown(report));
} else {
  console.log(JSON.stringify(report, null, 2));
}
