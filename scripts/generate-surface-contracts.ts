#!/usr/bin/env bun
/**
 * Generate the agent-readable launcher surface contract matrix.
 *
 * The source of truth is the typed Rust registry in
 * src/main_sections/app_view_state.rs. This script intentionally parses the
 * `AppView::surface_kind()` and `SurfaceKind::surface_contract()` matches
 * instead of maintaining a parallel hand-written matrix. Full fixture data comes
 * from the compiled design catalogue and is embedded in this one snapshot.
 *
 * Usage:
 *   bun scripts/generate-surface-contracts.ts --catalogue <design-discover.json> --write
 *   bun scripts/generate-surface-contracts.ts --check
 *   bun scripts/generate-surface-contracts.ts --catalogue <design-discover.json> --stdout
 */

import { readFileSync, writeFileSync } from "fs";
import { resolve } from "path";
import type { FixtureDescriptor } from "./devtools/lib/owned-evaluation.ts";

const PROJECT_ROOT = resolve(import.meta.dir, "..");
const SOURCE_PATH = "src/main_sections/app_view_state.rs";
const OUTPUT_PATH = "docs/ai/contracts/surface-contracts.json";
const SCHEMA_VERSION = 2;

type DismissPolicyName = "standard" | "explicit";

interface SurfaceContractEntry {
  surfaceKind: string;
  appViewVariants: string[];
  appViewFooters: Array<{
    variant: string;
    nativeFooterSurface: string | null;
    meaning: "routeLabelOnly";
  }>;
  vocabulary: {
    family: string;
    inputOwnership: string;
    previewRole: string;
  };
  focusPolicy: string;
  keyboardPolicy: string;
  actionsPolicy: string;
  proofPolicy: string;
  visualPolicy: string;
  dismissPolicy: {
    policy: DismissPolicyName;
    windowBlur: string;
    backdropClick: string;
    escape: string;
    cmdW: string;
  };
  automationSemanticSurface: string;
}

interface SurfaceContractMatrix {
  schemaVersion: number;
  generatedFrom: string;
  registry: string;
  entries: SurfaceContractEntry[];
  evidenceClass: "sourceContract";
  runtimeProof: false;
  fixtures: readonly FixtureDescriptor[];
  mainRoutes: RouteInventory[];
  secondaryFactories: FactoryInventory[];
  nativePeers: NativePeerInventory[];
}

export interface RouteInventory {
  appViewVariant: string;
  surfaceKind: string;
  fixtureIds: string[];
  presentationOwners: string[];
  proofBoundary: "owned-production-runtime" | "inactive-legacy-route";
}
interface FactoryInventory {
  rootOwner: string;
  factorySources: readonly string[];
  fixtureIds: string[];
  presentationOwners: string[];
  proofBoundary: "owned-production-runtime";
}
interface NativePeerInventory {
  owner: string;
  sources: string[];
  exclusion: string;
  proofBoundary: "native-only-excluded";
  fixtureIds: string[];
}


// Reviewed source census, not fixture dispatch. Fixture IDs are never authored
// here: only the compiled catalogue can provide them.
const SECONDARY_FACTORIES = [
  ["ActionsWindow", ["src/actions/window.rs"]],
  ["ConfirmPopupWindow", ["src/confirm/window.rs", "src/confirm/parent_dialog.rs"]],
  ["NotesApp", ["src/notes/window/window_ops.rs"]],
  ["AgentChatView", ["src/ai/agent_chat/ui/chat_window.rs"]],
  ["AgentChatHistoryPopup", ["src/ai/agent_chat/ui/history_popup.rs"]],
  ["DictationOverlay", ["src/dictation/window.rs"]],
  ["DictationMicrophonePopup", ["src/dictation/microphone_popup_window.rs"]],
  ["HudView", ["src/hud_manager/mod.rs"]],
  ["GpuiFooterOverlay", ["src/footer_popup.rs"]],
  ["ShortcutRecorder", ["src/app_impl/shortcut_recorder.rs"]],
  ["SnapOverlayView", ["src/window_control/snap_overlay.rs"]],
] as const;

const NATIVE_PEERS: NativePeerInventory[] = [
  { owner: "PassiveOverlayPanel", sources: ["src/platform/permiso/overlay_window.rs"], exclusion: "Raw AppKit NSPanel; native composition and ordering excluded" },
  { owner: "ScreenAreaCapture", sources: ["src/platform/screen_capture_overlay.rs"], exclusion: "OS screencapture selection; operator desktop capture excluded" },
  { owner: "ScriptKitSelfie", sources: ["src/platform/selfie_capture.rs"], exclusion: "CoreGraphics/xcap desktop capture; no GPUI presentation" },
  { owner: "NativeWindowScreenshot", sources: ["src/platform/screenshots_window_open.rs", "src/platform/screen_capture_sck.rs"], exclusion: "WindowServer/SCK capture; no GPUI presentation" },
  { owner: "NativeFooterGlassHosts", sources: ["src/footer_popup.rs", "src/platform/glass_button_host.rs"], exclusion: "AppKit material and glyph pixels excluded; GPUI footer has its own factory" },
  { owner: "SystemMenusAndDialogs", sources: ["src/tray.rs", "src/platform/mod.rs"], exclusion: "Tray/system menus, share sheets, native path and permission dialogs excluded" },
].map((peer) => ({ ...peer, proofBoundary: "native-only-excluded", fixtureIds: [] as string[] }));

export function linkRouteInventory(entries: readonly Pick<SurfaceContractEntry, "surfaceKind" | "appViewVariants">[], fixtures: readonly FixtureDescriptor[]): RouteInventory[] {
  const ids = new Set<string>();
  for (const fixture of fixtures) {
    if (!fixture.id || ids.has(fixture.id) || !fixture.owner || fixture.proofBoundary !== "owned-production-runtime") {
      throw new Error("Invalid or duplicate compiled fixture descriptor");
    }
    ids.add(fixture.id);
  }
  return entries.flatMap((entry) => entry.appViewVariants.map((variant) => {
    const matching = fixtures.filter((fixture) => fixture.root === "main" && fixture.appViewVariant === variant
      && (variant !== "FileSearchView" || fixture.surfaceVariant === entry.surfaceKind));
    const inactive = variant === "ActionsDialog";
    if (!inactive && matching.length === 0) throw new Error(`Missing compiled fixture for ${variant}/${entry.surfaceKind}`);
    for (const fixture of matching) {
      if (!fixture.presentationOwner) throw new Error(`Missing presentation owner for ${fixture.id}`);
    }
    return { appViewVariant: variant, surfaceKind: entry.surfaceKind,
      fixtureIds: matching.map((fixture) => fixture.id).sort(),
      presentationOwners: [...new Set(matching.map((fixture) => fixture.presentationOwner!))].sort(),
      proofBoundary: inactive ? "inactive-legacy-route" : "owned-production-runtime" };
  }));
}

function linkFactoryInventory(fixtures: readonly FixtureDescriptor[]): FactoryInventory[] {
  return SECONDARY_FACTORIES.map(([rootOwner, factorySources]) => {
    const matching = fixtures.filter((fixture) => factorySources.some((path) =>
      [fixture.owner, ...(fixture.factoryOwners ?? [])].some((owner) => owner === path || owner.startsWith(`${path}::`))));
    if (matching.length === 0) throw new Error(`Missing compiled fixture for factory ${rootOwner}`);
    for (const fixture of matching) {
      if (!fixture.presentationOwner) throw new Error(`Missing presentation owner for ${fixture.id}`);
    }
    return { rootOwner, factorySources, fixtureIds: matching.map((fixture) => fixture.id).sort(),
      presentationOwners: [...new Set(matching.map((fixture) => fixture.presentationOwner!))].sort(),
      proofBoundary: "owned-production-runtime" };
  });
}

function readCatalogue(): FixtureDescriptor[] {
  const index = process.argv.indexOf("--catalogue");
  const useSnapshot = index < 0 && hasFlag("--check") && !hasFlag("--write") && !hasFlag("--stdout");
  const path = useSnapshot ? OUTPUT_PATH : index < 0 ? undefined : process.argv[index + 1];
  if (!path || path.startsWith("--")) {
    throw new Error("--write and --stdout require --catalogue <fixture-catalogue.json>; --check alone uses the checked-in matrix fixtures");
  }
  const value = JSON.parse(readFileSync(resolve(PROJECT_ROOT, path), "utf8"));
  const schemaVersion = useSnapshot ? SCHEMA_VERSION : 1;
  if (value.schemaVersion !== schemaVersion || !Array.isArray(value.fixtures) || value.fixtures.length === 0) {
    throw new Error(`Expected schemaVersion:${schemaVersion} ${useSnapshot ? "surface contract matrix" : "compiled catalogue"} with nonempty fixtures`);
  }
  return value.fixtures;
}

function sourceBetween(source: string, start: string, end: string): string {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    throw new Error(`Missing start marker: ${start}`);
  }
  const afterStart = source.slice(startIndex);
  const endIndex = afterStart.indexOf(end);
  if (endIndex < 0) {
    throw new Error(`Missing end marker after ${start}: ${end}`);
  }
  return afterStart.slice(0, endIndex);
}

function parseSurfaceKinds(source: string): string[] {
  const enumBody = sourceBetween(source, "pub(crate) enum SurfaceKind {", "}\n\n/// First-pass vocabulary");
  return enumBody
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.endsWith(","))
    .filter((line) => !line.startsWith("#["))
    .map((line) => line.replace(/,$/, ""))
    .filter((line) => /^[A-Za-z][A-Za-z0-9_]*$/.test(line));
}

function parseAppViewVariantsByKind(source: string): Map<string, string[]> {
  const body = sourceBetween(
    source,
    "pub(crate) fn surface_kind(&self) -> SurfaceKind",
    "/// Exhaustive behavior contract for every top-level launcher view.",
  );
  const result = new Map<string, string[]>();
  const armRegex = /([\s\S]*?)=>\s*\{?\s*SurfaceKind::([A-Za-z0-9_]+)/g;
  let match: RegExpExecArray | null;
  while ((match = armRegex.exec(body)) !== null) {
    const armSource = match[1] ?? "";
    const kind = match[2] ?? "";
    const variants = [...armSource.matchAll(/AppView::([A-Za-z0-9_]+)/g)].map(
      (variantMatch) => variantMatch[1],
    );
    if (variants.length === 0) {
      continue;
    }
    const existing = result.get(kind) ?? [];
    result.set(kind, [...new Set([...existing, ...variants])]);
  }
  return result;
}

function parseNativeFooterSurfaceByVariant(source: string): Map<string, string | null> {
  const body = sourceBetween(
    source,
    "pub(crate) fn native_footer_surface(&self) -> Option<&'static str>",
    "}\n}\n\nimpl SurfaceKind",
  );
  const result = new Map<string, string | null>();
  const armRegex = /([\s\S]*?)=>\s*(Some\("([^"]+)"\)|None)/g;
  let match: RegExpExecArray | null;
  while ((match = armRegex.exec(body)) !== null) {
    const armSource = match[1] ?? "";
    const footer = match[3] ?? null;
    for (const variantMatch of armSource.matchAll(/AppView::([A-Za-z0-9_]+)/g)) {
      const variant = variantMatch[1];
      if (variant) {
        result.set(variant, footer);
      }
    }
  }
  return result;
}

function surfaceKindArms(source: string): Array<{ kind: string; body: string }> {
  const body = sourceBetween(
    source,
    "pub(crate) fn surface_contract(self) -> LauncherSurfaceContract",
    "/// Map an [`AppView`] variant to the automation",
  );
  const markers = [...body.matchAll(/SurfaceKind::([A-Za-z0-9_]+)\s*=>/g)].map((match) => ({
    kind: match[1] ?? "",
    index: match.index ?? 0,
  }));
  return markers.map((marker, index) => {
    const next = markers[index + 1]?.index ?? body.length;
    return {
      kind: marker.kind,
      body: body.slice(marker.index, next),
    };
  });
}

function dismissPolicy(token: string): SurfaceContractEntry["dismissPolicy"] {
  if (token === "standard") {
    return {
      policy: "standard",
      windowBlur: "CloseMainWindow",
      backdropClick: "CloseMainWindow",
      escape: "CloseMainWindow",
      cmdW: "CloseMainWindow",
    };
  }
  if (token === "explicit") {
    return {
      policy: "explicit",
      windowBlur: "Ignore",
      backdropClick: "Ignore",
      escape: "LetViewHandle",
      cmdW: "CloseMainWindow",
    };
  }
  if (token === "DismissPolicy::cancel_to_script_prompt()") {
    return {
      policy: "cancelToScriptPrompt",
      windowBlur: "CloseMainWindow",
      backdropClick: "CloseMainWindow",
      escape: "LetViewHandle",
      cmdW: "CloseMainWindow",
    };
  }
  if (token === "DismissPolicy::sticky_escape_closes()") {
    return {
      policy: "stickyEscapeCloses",
      windowBlur: "Ignore",
      backdropClick: "Ignore",
      escape: "CloseMainWindow",
      cmdW: "CloseMainWindow",
    };
  }
  if (token === "DismissPolicy::blur_closes_escape_view_owned()") {
    return {
      policy: "blurClosesEscapeViewOwned",
      windowBlur: "CloseMainWindow",
      backdropClick: "CloseMainWindow",
      escape: "LetViewHandle",
      cmdW: "CloseMainWindow",
    };
  }
  throw new Error(`Unknown dismiss policy token: ${token}`);
}

function parseContractMatrix(fixtures: readonly FixtureDescriptor[]): SurfaceContractMatrix {
  const source = readFileSync(resolve(PROJECT_ROOT, SOURCE_PATH), "utf8");
  const surfaceKinds = parseSurfaceKinds(source);
  const appViewVariantsByKind = parseAppViewVariantsByKind(source);
  const nativeFooterSurfaceByVariant = parseNativeFooterSurfaceByVariant(source);
  const arms = surfaceKindArms(source);

  const entries = arms.map(({ kind, body }) => {
    const vocabulary = body.match(
      /LauncherSurfaceContractVocabulary::new\(\s*([A-Za-z0-9_]+),\s*([A-Za-z0-9_]+),\s*([A-Za-z0-9_]+),?\s*\)/,
    );
    if (!vocabulary) {
      throw new Error(`Missing vocabulary tuple for SurfaceKind::${kind}`);
    }
    const policyAndSurface = body.match(
      /\)\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z0-9_]+)\s*,\s*(standard|explicit|DismissPolicy::[A-Za-z0-9_]+\(\))\s*,\s*"([^"]+)"/,
    );
    if (!policyAndSurface) {
      throw new Error(
        `Missing focus, keyboard, actions, proof, visual, dismiss policy, or semantic surface for SurfaceKind::${kind}`,
      );
    }
    return {
      surfaceKind: kind,
      appViewVariants: appViewVariantsByKind.get(kind) ?? [],
      appViewFooters: (appViewVariantsByKind.get(kind) ?? []).map((variant) => ({
        variant,
        nativeFooterSurface: nativeFooterSurfaceByVariant.get(variant) ?? null,
        meaning: "routeLabelOnly" as const,
      })),
      vocabulary: {
        family: vocabulary[1] ?? "",
        inputOwnership: vocabulary[2] ?? "",
        previewRole: vocabulary[3] ?? "",
      },
      focusPolicy: policyAndSurface[1] ?? "",
      keyboardPolicy: policyAndSurface[2] ?? "",
      actionsPolicy: policyAndSurface[3] ?? "",
      proofPolicy: policyAndSurface[4] ?? "",
      visualPolicy: policyAndSurface[5] ?? "",
      dismissPolicy: dismissPolicy(policyAndSurface[6] ?? ""),
      automationSemanticSurface: policyAndSurface[7] ?? "",
    };
  });

  const missingContract = surfaceKinds.filter(
    (kind) => !entries.some((entry) => entry.surfaceKind === kind),
  );
  if (missingContract.length > 0) {
    throw new Error(`SurfaceKind contract entries missing: ${missingContract.join(", ")}`);
  }

  const missingIdentity = entries.filter((entry) => entry.appViewVariants.length === 0);
  if (missingIdentity.length > 0) {
    throw new Error(
      `SurfaceKind identity entries missing AppView variants: ${missingIdentity
        .map((entry) => entry.surfaceKind)
        .join(", ")}`,
    );
  }

  const missingFooter = entries
    .flatMap((entry) => entry.appViewVariants)
    .filter((variant) => !nativeFooterSurfaceByVariant.has(variant));
  if (missingFooter.length > 0) {
    throw new Error(
      `AppView native footer entries missing: ${[...new Set(missingFooter)].join(", ")}`,
    );
  }

  return {
    schemaVersion: SCHEMA_VERSION,
    generatedFrom: SOURCE_PATH,
    registry: "AppView::surface_kind -> SurfaceKind::surface_contract",
    evidenceClass: "sourceContract",
    runtimeProof: false,
    fixtures,
    mainRoutes: linkRouteInventory(entries, fixtures),
    secondaryFactories: linkFactoryInventory(fixtures),
    nativePeers: NATIVE_PEERS,
    entries,
  };
}

function renderJson(matrix: SurfaceContractMatrix): string {
  return `${JSON.stringify(matrix, null, 2)}\n`;
}

function hasFlag(flag: string): boolean {
  return process.argv.includes(flag);
}

if (import.meta.main) {
  if (!["--check", "--write", "--stdout"].some(hasFlag)) {
    process.stderr.write("Usage: bun scripts/generate-surface-contracts.ts --check | --catalogue <fixture-catalogue.json> --write|--stdout\n");
    process.exit(2);
  }
  const output = renderJson(parseContractMatrix(readCatalogue()));
  const outputPath = resolve(PROJECT_ROOT, OUTPUT_PATH);
  if (hasFlag("--stdout")) {
    process.stdout.write(output);
  } else if (hasFlag("--check")) {
    const current = readFileSync(outputPath, "utf8");
    if (current !== output) {
      throw new Error(`${OUTPUT_PATH} is stale. Regenerate with design discover, then --catalogue <fixture-catalogue.json> --write`);
    }
  } else if (hasFlag("--write")) {
    writeFileSync(outputPath, output);
  }
}
