#!/usr/bin/env bun
/**
 * scripts/devtools/coverage.ts — the canonical runtime coverage-profile
 * registry (PF-009).
 *
 * This module is IMPORTABLE: `import { coverageProfiles } from "./coverage.ts"`
 * produces no stdout and no side effects. The CLI report lives behind
 * `import.meta.main`.
 *
 * Namespace discipline (RPT-001): runtime coverage profiles are NOT contract
 * kinds, contract mappings, unique AppView variants, or orientation aliases.
 * Profile counts must never be mixed into the 37/54 source census.
 *
 * Binding support (PF-009) is derived ONLY from the machine-readable fields
 * `availablePrimitiveIds` and `bindingSelectors` — never from the prose
 * fields (`supportedNow`, `missingRuntimePrimitives`, `features`,
 * `shortcuts`), which remain human reporting.
 */

import { existsSync } from "node:fs";
import { isAbsolute, relative, resolve, sep } from "node:path";
import { receiptSchemaRegistry } from "./lib/receipt-schema.ts";

export type CoverageStatus = "supported" | "partial" | "missing" | "planned";

export type Domain = {
  id: string;
  name: string;
  chromeAnalogue: string;
  purpose: string;
  currentPrimitives: string[];
  nextPrimitives: string[];
};

/**
 * Typed selector joining canonical AppView→SurfaceKind mappings to this
 * profile. Selection precedence is priority-based and ties are INVALID —
 * never resolved by array order (see resolveCoverageProfile in surfaces.ts).
 */
export type CoverageProfileSelector = {
  relation: "Direct" | "Derived";
  priority: number;
  contractKinds?: readonly string[];
  appViewVariants?: readonly string[];
  families?: readonly string[];
  hostKinds?: readonly string[];
};

export type CoverageProfile = {
  id: string;
  name: string;
  status: CoverageStatus;
  domains: string[];
  sourceFiles: string[];
  features: string[];
  shortcuts: string[];
  supportedNow: string[];
  missingRuntimePrimitives: string[];
  regressionRecipeRole: string;
  /** Machine-readable primitive availability; every id must resolve in receiptSchemaRegistry. */
  availablePrimitiveIds: readonly string[];
  /** Machine-readable mapping selectors; empty = profile is not binding-addressable. */
  bindingSelectors: readonly CoverageProfileSelector[];
};

const notesShortcutCoverage = [
  "Cmd+K",
  "Cmd+P",
  "Cmd+Shift+P",
  "Cmd+F",
  "Cmd+Shift+F",
  "Cmd+N",
  "Cmd+Shift+N",
  "Cmd+Shift+T",
  "Cmd+W",
  "Cmd+.",
  "Cmd+Shift+.",
  "Cmd+Shift+S",
  "Cmd+Z",
  "Cmd+D",
  "Cmd+Shift+D",
  "Cmd+Shift+X",
  "Cmd+Shift+L",
  "Cmd+L",
  "Cmd+Shift+-",
  "Cmd+Shift+H",
  "Cmd+V",
  "Cmd+Shift+C",
  "Cmd+E",
  "Cmd+/",
  "Cmd+J",
  "Cmd+Shift+U",
  "Cmd+B",
  "Cmd+I",
  "Cmd+Shift+I",
  "Cmd+Enter",
  "Cmd+Shift+A",
  "Cmd+Shift+O",
  "Cmd+Up",
  "Cmd+Down",
  "Cmd+Shift+Up",
  "Cmd+Shift+Down",
  "Cmd+[",
  "Cmd+]",
  "Cmd+Shift+Backspace",
  "Cmd+Shift+Delete",
  "Cmd+Shift+7",
  "Cmd+Shift+8",
  "Cmd+1..Cmd+9",
  "Tab",
  "Shift+Tab",
  "Alt+Up",
  "Alt+Down",
  "Alt+Shift+Up",
  "Alt+Shift+Down",
  "Ctrl+Shift+K",
  "Escape",
  "Enter",
  "ArrowUp",
  "ArrowDown",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "Backspace",
  "Delete",
];

const dictationPhaseCoverage = [
  "idle/hidden",
  "recording",
  "quiet recording",
  "active speech",
  "confirming",
  "stop confirmation",
  "transcribing",
  "delivering",
  "finished",
  "failed/error",
  "Idle -> Recording",
  "Recording -> Confirming",
  "Recording -> Transcribing",
  "Recording -> Failed",
  "Confirming -> Recording",
  "Confirming -> Transcribing",
  "Transcribing -> Delivering",
  "Transcribing -> Failed",
  "Delivering -> Finished",
  "Delivering -> Failed",
  "Finished -> Idle",
  "Failed -> Idle",
];

export const coverageDomains: Domain[] = [
  {
    id: "targets",
    name: "Targets and Windows",
    chromeAnalogue: "Target/Page",
    purpose: "Discover exact app windows, attached popups, detached panels, parentage, bounds, and screenshot identity.",
    currentPrimitives: ["listAutomationWindows", "inspectAutomationWindow", "devtools.inspect"],
    nextPrimitives: ["devtools.targets.watch", "target capability discovery", "window lifetime timeline"],
  },
  {
    id: "elements",
    name: "Elements and Semantics",
    chromeAnalogue: "DOM/Accessibility tree",
    purpose: "Expose visible UI nodes, roles, labels, selected/focused ids, disabled reasons, actions, owners, and stable semantic ids.",
    currentPrimitives: ["getElements", "inspectAutomationWindow.semanticQuality"],
    nextPrimitives: ["target-scoped semantic collectors", "stable action ids", "semantic tree diff"],
  },
  {
    id: "layout",
    name: "Layout and Box Model",
    chromeAnalogue: "Elements box model / Overlay",
    purpose: "Measure bounds, scroll extents, anchor rects, safe areas, overlap pairs, footer/input/list geometry, and resize deltas.",
    currentPrimitives: ["getLayoutInfo"],
    nextPrimitives: ["devtools.measure.layout", "target-scoped layout info", "scroll geometry", "anchor and overlap reports"],
  },
  {
    id: "styles",
    name: "Styles, Theme, and Text Fit",
    chromeAnalogue: "CSS/Computed styles",
    purpose: "Expose theme tokens, foreground/background colors, contrast, font metrics, wrap lines, truncation intent, and text clipping.",
    currentPrimitives: ["theme contrast source audits", "screenshot pixel probes"],
    nextPrimitives: ["devtools.measure.text", "devtools.measure.contrast", "computed theme tokens per node"],
  },
  {
    id: "console",
    name: "Console, Logs, and Events",
    chromeAnalogue: "Console/Log",
    purpose: "Correlate user actions with app logs, protocol parse failures, warnings, event traces, and structured diagnostics.",
    currentPrimitives: ["scripts/agentic/session.sh logs", "response logs", "app logs"],
    nextPrimitives: ["devtools.events.tail", "action-correlated log spans", "warning taxonomy"],
  },
  {
    id: "sources",
    name: "Sources, Scripts, and Owners",
    chromeAnalogue: "Sources",
    purpose: "Map observed UI nodes and failed measurements to script metadata, prompt type, source files, and likely Rust owners.",
    currentPrimitives: ["promptType", "surfaceContract", "doc refs"],
    nextPrimitives: ["owner metadata on semantic nodes", "script provenance receipts", "source jump hints"],
  },
  {
    id: "performance",
    name: "Performance and Timeline",
    chromeAnalogue: "Performance",
    purpose: "Capture resize, filtering, provider refresh, render, async delivery, and focus-transition timelines.",
    currentPrimitives: ["trace logs", "FILTER_PERF logs", "scenario receipts"],
    nextPrimitives: ["devtools.timeline.record", "layout shift timeline", "input-to-paint timings"],
  },
  {
    id: "storage",
    name: "Storage, Resources, and Privacy",
    chromeAnalogue: "Application/Storage",
    purpose: "Inspect redacted resource rows, cache/store identities, context resources, attachment provenance, and privacy boundaries.",
    currentPrimitives: ["kit://context resources", "surface-specific state receipts"],
    nextPrimitives: ["devtools.resources.inspect", "redaction fingerprints", "cache/store generation ids"],
  },
  {
    id: "accessibility",
    name: "Accessibility",
    chromeAnalogue: "Accessibility",
    purpose: "Compare semantic nodes with AX roles, labels, focus order, disabled state, activation affordances, and keyboard reachability.",
    currentPrimitives: ["native computer observation", "semantic roles"],
    nextPrimitives: ["devtools.ax.snapshot", "semantic-to-AX parity diff", "tab order graph"],
  },
  {
    id: "input",
    name: "Input, Focus, and Actions",
    chromeAnalogue: "Input/Runtime",
    purpose: "Drive user-like keys, text, selection, safe clicks, popup dismissal, focus ownership, and wrong-target refusal.",
    currentPrimitives: ["batch", "waitFor", "simulateKey", "target-scoped batch.setInput"],
    nextPrimitives: ["devtools.act", "focus owner transitions", "safe click receipts", "shortcut registry snapshot"],
  },
  {
    id: "media",
    name: "Media, Sensors, and Permissions",
    chromeAnalogue: "Media/Sensors/Permissions",
    purpose: "Inspect microphone readiness, dictation recording states, model readiness, target delivery, permission status, and media cleanup.",
    currentPrimitives: ["dictation story states", "dictation fail-closed scenario specs"],
    nextPrimitives: ["devtools.media.inspect", "passive permission receipts", "transcript delivery generation ids"],
  },
  {
    id: "screenshots",
    name: "Screenshots and Visual Proof",
    chromeAnalogue: "Page.captureScreenshot / Overlay",
    purpose: "Capture strict target screenshots, crop identity, nonblank checks, pixel probes, visual agreement with semantic state, and before/after evidence.",
    currentPrimitives: ["captureScreenshot", "captureWindow", "verify-shot.ts", "inspectAutomationWindow screenshot metadata"],
    nextPrimitives: ["devtools.visual.compare", "semantic text agreement", "occlusion candidates"],
  },
  {
    id: "investigation",
    name: "Investigation Records",
    chromeAnalogue: "Recorder/Protocol Monitor",
    purpose: "Store bug intake, hypotheses, actions, receipts, missing primitives, classification, likely owner, and red/green proof.",
    currentPrimitives: ["manual reports", "scenario receipts"],
    nextPrimitives: ["devtools.investigate", "paired red/green artifact schema", "missing primitive backlog export"],
  },
];

/**
 * Generic target-scoped primitive set available for main-window launcher
 * surfaces: every producer here is target-scoped and works against any
 * main-window AppView (elements/layout/scroll/focus/text/keyboard/act).
 */
const mainWindowPrimitiveIds = [
  "devtools.targets.inspect",
  "devtools.surface.inspect",
  "devtools.elements.snapshot",
  "devtools.layout.measure",
  "devtools.scroll.inspect",
  "devtools.focus.inspect",
  "devtools.text.measure",
  "devtools.keyboard.inspect",
  "devtools.act",
] as const;

export const coverageProfiles: CoverageProfile[] = [
  {
    id: "agent-chat",
    name: "Agent Chat and Quick AI reliability",
    status: "partial",
    domains: ["targets", "elements", "layout", "input", "console", "investigation"],
    sourceFiles: [
      "src/ai/agent_chat/ui/view.rs",
      "src/ai/reliability/devtools.rs",
      "src/protocol/types/ai_reliability_state.rs",
    ],
    features: ["typed reliability state", "redacted diagnostics", "composer and transcript fingerprints"],
    shortcuts: ["Escape", "Enter", "Cmd+K"],
    supportedNow: [
      "getAiReliabilityState(target)",
      "getAgentChatState(target).reliability",
      "setAiReliabilityTestFixture",
      "agent_chat.ts inspect --fixture image-2-search-budget --strict",
    ],
    missingRuntimePrimitives: ["green recovery action activation receipt"],
    regressionRecipeRole: "Use fixtures for deterministic red/green recovery proof without provider credentials.",
    availablePrimitiveIds: [...mainWindowPrimitiveIds],
    bindingSelectors: [
      { relation: "Direct", priority: 100, appViewVariants: ["AgentChatView"], hostKinds: ["MainWindow"] },
    ],
  },
  {
    id: "chat-prompt",
    name: "Legacy ChatPrompt reliability",
    status: "partial",
    domains: ["targets", "elements", "layout", "input", "investigation"],
    sourceFiles: [
      "src/prompts/chat",
      "src/ai/reliability/devtools.rs",
      "src/protocol/types/ai_reliability_state.rs",
    ],
    features: ["typed reliability state", "redacted screenshot-one defect fixture"],
    shortcuts: ["Escape", "Enter"],
    supportedNow: [
      "getAiReliabilityState(target)",
      "chat_prompt.ts inspect --fixture image-1-client-too-old --strict",
    ],
    missingRuntimePrimitives: ["green recovery action activation receipt"],
    regressionRecipeRole: "Keep ChatPrompt proof compatibility-only while primary behavior migrates to shared recovery.",
    availablePrimitiveIds: [...mainWindowPrimitiveIds],
    bindingSelectors: [
      { relation: "Direct", priority: 100, appViewVariants: ["ChatPrompt"], hostKinds: ["MainWindow"] },
    ],
  },
  {
    id: "flow-ux-view",
    name: "Flow conversation reliability",
    status: "partial",
    domains: ["targets", "elements", "layout", "input", "investigation"],
    sourceFiles: ["src/render_builtins/flow_ux.rs", "src/flows/session.rs"],
    features: ["flow conversation identity", "protocol failure fixture", "preservation fingerprints"],
    shortcuts: ["Escape", "Enter"],
    supportedNow: [
      "getAiReliabilityState(target)",
      "flows.ts inspect --fixture protocol-failure --strict",
    ],
    missingRuntimePrimitives: ["green rethread action activation receipt"],
    regressionRecipeRole: "Use deterministic protocol fixtures before live mdflow smoke coverage.",
    availablePrimitiveIds: [...mainWindowPrimitiveIds],
    bindingSelectors: [
      { relation: "Direct", priority: 90, contractKinds: ["FlowUx"] },
    ],
  },
  {
    id: "flow-session-view",
    name: "Flow run/session reliability",
    status: "partial",
    domains: ["targets", "elements", "layout", "input", "investigation"],
    sourceFiles: ["src/flows/session.rs", "src/flows/runner.rs"],
    features: ["flow run identity", "session preservation", "restart and reattach state"],
    shortcuts: ["Escape", "Enter"],
    supportedNow: ["getAiReliabilityState(target)"],
    missingRuntimePrimitives: ["green restart and reattach action receipts"],
    regressionRecipeRole: "Use live mdflow only after deterministic state/action receipts are green.",
    availablePrimitiveIds: [...mainWindowPrimitiveIds],
    bindingSelectors: [
      { relation: "Direct", priority: 90, contractKinds: ["FlowSession"] },
    ],
  },
  {
    id: "main",
    name: "Main launcher and prompt host",
    status: "partial",
    domains: ["targets", "elements", "layout", "input", "screenshots", "console", "sources"],
    sourceFiles: [
      "src/main_sections/render_impl.rs",
      "src/main_sections/app_state.rs",
      "src/render_script_list/mod.rs",
    ],
    features: ["script list", "prompt state", "footer", "input", "preview", "surface contract", "source chips"],
    shortcuts: ["Cmd+K", "Escape", "Enter", "Tab", "ArrowUp", "ArrowDown"],
    supportedNow: [
      "devtools.inspect --main",
      "getState",
      "getElements",
      "getLayoutInfo",
      "captureScreenshot",
      "target-scoped main-window open/close stale-view freshness proof",
      "target-scoped main-window early-frame surface/footer/chrome freshness proof",
    ],
    missingRuntimePrimitives: ["text fit", "scroll geometry", "layout overlap pairs", "focus ring bounds"],
    regressionRecipeRole: "Use recipes only for stable launcher regressions after direct measurements isolate the bug.",
    availablePrimitiveIds: [...mainWindowPrimitiveIds, "devtools.actions.inspect"],
    bindingSelectors: [
      // Host-wide Derived fallback for every ordinary Main-window mapping.
      // Exact profiles (agent-chat, chat-prompt, flow-*, dictation-history)
      // outrank this by priority; ties are invalid, never array-ordered.
      { relation: "Derived", priority: 10, hostKinds: ["MainWindow"] },
    ],
  },
  {
    id: "actions-dialog",
    name: "Actions dialog and attached action menus",
    status: "partial",
    domains: ["targets", "elements", "layout", "input", "screenshots", "accessibility"],
    sourceFiles: ["src/actions/window.rs", "src/actions/command_bar.rs", "src/actions/types/action_model.rs"],
    features: ["route stack", "sections", "filter input", "shortcut hints", "disabled reasons", "anchor placement", "resize"],
    shortcuts: ["Cmd+K", "Escape", "Enter", "Backspace", "ArrowUp", "ArrowDown"],
    supportedNow: [
      "devtools.actions.inspect",
      "devtools.actions.inspect --start --keep-open --open-target-kind notes",
      "inspectAutomationWindow target kind actionsDialog",
      "getState target kind actionsDialog",
      "attached popup parent/anchor geometry",
      "attached popup resize generation",
      "runtime row and section bounds model",
      "runtime hover row availability",
      "target-scoped ActionsDialog first-click selection proof",
      "target-scoped ActionsDialog second-click activation lifecycle proof",
      "target-scoped ActionsDialog semantic freshness proof after first-click selection",
      "target-scoped ActionsDialog close cleanup proof after activation",
      "target-scoped ActionsDialog Cmd+K shortcut-open first-frame freshness proof",
      "target-scoped ActionsDialog Cmd+K shortcut-close cleanup proof",
      "target-scoped ActionsDialog Escape close cleanup proof",
      "runtime shortcut layout bounds model",
      "getElements(target)",
      "getLayoutInfo(target actionsDialog) ActionsDialog/search/header/list/visible-row/shortcut bounds",
      "target bounds",
      "proof-session keep-open guard",
    ],
    missingRuntimePrimitives: ["disabled reason bounds for routes that render visible disabled explanations"],
    regressionRecipeRole: "Smoke actions menu invariants only after direct target-scoped popup layout receipts isolate the bug.",
    availablePrimitiveIds: [...mainWindowPrimitiveIds, "devtools.actions.inspect"],
    bindingSelectors: [
      { relation: "Direct", priority: 95, contractKinds: ["ActionsDialog"], hostKinds: ["ActionsDialog"] },
    ],
  },
  {
    id: "notes",
    name: "Notes window",
    status: "partial",
    domains: ["targets", "elements", "layout", "input", "storage", "screenshots", "accessibility", "investigation"],
    sourceFiles: [
      "src/notes/window.rs",
      "src/notes/window/keyboard.rs",
      "src/notes/window/ai_handoff.rs",
      "src/notes/window/window_ops.rs",
      "src/notes/window/render_ui.rs",
      "src/notes/actions_panel.rs",
      "src/notes/window/panels.rs",
      "src/notes/storage.rs",
      "src/notes/model.rs",
    ],
    features: [
      "floating notes host",
      "editor mode",
      "browse/list mode",
      "trash mode",
      "markdown editor",
      "markdown preview",
      "editor find",
      "global search",
      "format toolbar",
      "focus mode",
      "pinning",
      "sort cycling",
      "command bar",
      "actions panel",
      "recent note switcher",
      "note cart",
      "clipboard-backed note creation",
      "embedded Agent Chat mode",
      "Agent Chat actions popup",
      "Agent Chat history portal",
      "attachment/context chips",
      "draft snapshots",
      "auto-resize",
      "autosave and dirty state",
      "history back/forward",
      "scroll collapse after deleting trailing lines",
      "independent app-hide behavior",
    ],
    shortcuts: notesShortcutCoverage,
    supportedNow: [
      "stable notes automation parent",
      "inspectAutomationWindow target id notes",
      "getElements(target) for notes-owned surfaces when registered",
      "getState(target notes) redacted active note, dirty state, selection, focus, and autosize envelope",
      "getState(target notes) command bar route stack, filtered actions, selection, shortcuts, and redacted search fingerprints",
      "getState(target notes) shortcut registry scopes and active focus owner",
      "getState(target notes) focus owner transition timeline",
      "getState(target notes) redacted draft snapshot fingerprint",
      "getState(target notes) editor scroll metrics and mounted preview anchor availability",
      "target-scoped batch togglePreview for mounting markdown preview before measurement",
      "target-scoped simulateKey Cmd+Shift+P Notes preview activation receipt",
      "notes.resize-compare sandboxed auto-resize before/after receipt",
      "notes.inspect --open-actions target-scoped batch openActions receipt",
      "notes.inspect lastAiHandoff redacted receipt (notes→main Agent Chat handoff)",
      "getLayoutInfo(target notes) NotesWindow/titlebar/editor/footer/panel bounds and resize pressure",
      "notes storage generation and redacted sandbox identity",
      "notes-window-resize-stress regression receipt",
    ],
    missingRuntimePrimitives: [
      "preview scroll handle populated content bounds under mounted markdown preview",
      "notes AI handoff receipt beyond simulateKey Cmd+Enter activation",
      "portal session provenance",
      "remaining Notes shortcut activation parity receipts beyond Cmd+Shift+P",
    ],
    regressionRecipeRole: "Keep notes recipes as regression guards for resize, Agent Chat handoff, preview sync, and origin safety after DevTools receipts exist.",
    availablePrimitiveIds: [
      "devtools.targets.inspect",
      "devtools.surface.inspect",
      "devtools.elements.snapshot",
      "devtools.layout.measure",
      "devtools.focus.inspect",
      "devtools.keyboard.inspect",
      "devtools.act",
      "devtools.notes.inspect",
      "devtools.notes.resizeCompare",
    ],
    bindingSelectors: [
      // Host-wide fallback for Notes-hosted orientation aliases only; no
      // canonical Main-window mapping resolves here.
      { relation: "Derived", priority: 10, hostKinds: ["NotesWindow"] },
    ],
  },
  {
    id: "notes-main-handoff",
    name: "Notes → main Agent Chat handoff",
    // Reconciled 2026-08-07 (C07): the wrong-host negative proof named in
    // missingRuntimePrimitives is still absent from the tree
    // (notes-main-agent-chat-handoff.ts has no wrong-host case), so this
    // profile cannot carry an unqualified "supported" claim.
    status: "partial",
    domains: ["targets", "elements", "input", "logs", "investigation"],
    sourceFiles: ["src/notes/window/ai_handoff.rs", "src/app_impl/agent_handoff/agent_chat_entry.rs", "src/ai/agent_chat/ui/view.rs"],
    features: ["Cmd+Enter handoff", "Ask AI titlebar command", "actions-panel Ask AI About This Note", "@note composer token", "focusedTarget note chip", "cart supplemental chips", "redacted lastAiHandoff receipt"],
    shortcuts: ["Cmd+Enter", "Cmd+Shift+A"],
    supportedNow: [
      "target-scoped simulateKey Cmd+Enter openMainAgentChatFromNotesCmdEnter receipt",
      "getState(target notes) lastAiHandoff redacted receipt",
      "getAgentChatState contextParts focusedTarget note identity",
      "notes-main-agent-chat-handoff.ts cross-window probe (Notes stays open)",
    ],
    missingRuntimePrimitives: ["wrong-host negative proof"],
    regressionRecipeRole: "Run scripts/devtools/notes-main-agent-chat-handoff.ts after any change to the handoff payload, entry contract, or composer staging.",
    availablePrimitiveIds: [
      "devtools.targets.inspect",
      "devtools.surface.inspect",
      "devtools.elements.snapshot",
      "devtools.act",
      "devtools.notes.inspect",
    ],
    bindingSelectors: [],
  },
  {
    id: "inline-agent",
    name: "Focused-text mini Agent Chat",
    status: "partial",
    domains: ["targets", "elements", "layout", "input", "screenshots", "accessibility", "console", "storage"],
    sourceFiles: [
      "src/app_impl/agent_handoff/focused_text_entry.rs",
      "src/ai/agent_chat/ui/view.rs",
      "src/ai/agent_chat/ui/ui_variant.rs",
      "src/ai/focused_text/platform_bridge.rs",
      "src/ai/agent_chat/launch.rs",
      "src/ai/agent_chat/profiles.rs",
      "src/platform/accessibility/focused_text.rs",
      "src/platform/accessibility/mutation.rs",
      "src/app_layout/collect_elements.rs",
    ],
    features: [
      "whole focused-field capture before main-window focus",
      "main-window mini Agent Chat mode",
      "app badge and text metrics",
      "prompt placeholder Edit, refine, ask...",
      "Thinking... processing state",
      "streaming output preview",
      "Replace, Append, Copy, and Chat actions",
      "expanded same-session Agent Chat panel",
      "Cue - N turns header",
      "persistent expanded composer",
      "Collapse back to focused-text mini preserving latest output",
      "Stop and Retry controls",
      "Agent Chat Pi Text profile executor",
      "isolated focused-text Pi cwd",
      "warm Pi session prepare/acquire/dismiss-reset",
      "no Agent Chat backend fallback for focused-text mini",
      "privacy-safe prompt and output logging",
      "DevTools semantic IDs for focused-text mini and expanded modes",
    ],
    shortcuts: ["inline AI hotkey", "double Command", "Escape", "Enter"],
    supportedNow: [
      "getAgentChatState(target main) uiVariant focused-text-mini",
      "getAgentChatState(target main) redacted focusedText char count, capabilities, output-ready, and last-apply envelope",
      "getElements(target main) focused-text-mini-root, focused-text-input, focused-text-preview semantic ids",
      "getElements(target main) focused-text Replace, Append, Copy, Expand, Stop, Retry semantic action ids",
      "getElements(target main) action disabled reasons for no-output, active-turn, and not-retryable",
      "openFocusedTextAgentChatWithMockData stdin fixture for mock focused text and deterministic Agent Chat output",
      "openFocusedTextAgentChatWithPiData stdin fixture for real warm Pi Text-profile stream proof",
      "openInlineAgentWithMockData and openInlineAgentWithPiData compatibility aliases to focused-text Agent Chat",
      "source audits for capture-before-main-window-focus and no selected-text fallback",
      "source audits for Agent Chat Pi Text profile ownership and no Agent Chat backend fallback",
      "source audits for privacy-safe provider timing logs",
      "contract tests for prompt, streaming state, actions, layout, theme, session retry, Pi launch, and Agent Chat adapter",
    ],
    missingRuntimePrimitives: [
      "runtime receipts for main-window focused-text mini layout regions",
      "TextEdit capture/replace/append receipts",
      "browser textarea capture/replace/append receipts",
      "native double-Command trigger delivery proof",
      "dark and light visual contrast screenshots",
    ],
    regressionRecipeRole: "Use focused-text Agent Chat recipes only after direct getAgentChatState/getElements/layout receipts isolate the behavior.",
    availablePrimitiveIds: [...mainWindowPrimitiveIds],
    // The focused-text mini is a ui-variant of AgentChatView, not a distinct
    // canonical mapping; giving it a selector would tie with agent-chat.
    bindingSelectors: [],
  },
  {
    id: "dictation",
    name: "Dictation window and media flow",
    status: "partial",
    domains: ["targets", "elements", "media", "input", "storage", "screenshots", "accessibility", "investigation"],
    sourceFiles: [
      "src/dictation/window.rs",
      "src/dictation/runtime.rs",
      "src/dictation/types.rs",
      "src/dictation/setup.rs",
      "src/dictation/capture.rs",
      "src/dictation/device.rs",
      "src/dictation/transcription.rs",
      "src/main_entry/runtime_tray_hotkeys.rs",
    ],
    features: [
      ...dictationPhaseCoverage,
      "Script Kit target delivery",
      "Agent Chat target delivery",
      "external app target delivery",
      "Notes editor target delivery",
      "Tab AI target delivery",
      "frontmost app paste delivery",
      "waveform/audio level bars",
      "microphone permission",
      "microphone device",
      "preferred device fallback",
      "model readiness",
      "model download/extract/failure status",
      "hotkey readiness",
      "hotkey registration",
      "hotkey conflict detection",
      "target identity",
      "transcript generation",
      "cursor insertion range",
      "wrong-target rejection",
      "cleanup without TCC/System Settings mutation",
    ],
    shortcuts: ["dictation hotkey", "Escape", "Enter", "Space", "Cmd+W", "target badge click"],
    supportedNow: [
      "dictation story states",
      "kit://dictation",
      "kit://dictation-history",
      "getState dictation passive runtime phase/target/elapsed envelope",
      "getState dictation passive model readiness generation",
      "getState dictation passive microphone permission status and redacted device snapshot",
      "getState dictation passive hotkey binding snapshot",
      "getState dictation recording state generation and idle audio-level receipt",
      "getState dictation media cleanup receipt",
      "dictation.deliver-fixture pushDictationResult target delivery generation, transcript fingerprint, and main-filter insertion range receipt",
      "dictation.deliver-fixture --expect-refusal wrong-target refusal receipt",
      "devtools.media.inspect passive receipt gate",
      "fail-closed dictation stress specs",
    ],
    missingRuntimePrimitives: [
      "cursor insertion range for Notes/Agent Chat/frontmost destinations",
    ],
    regressionRecipeRole: "Do not use live dictation recipes as proof until passive media receipts can avoid permission prompts and target mutations.",
    availablePrimitiveIds: [
      "devtools.targets.inspect",
      "devtools.surface.inspect",
      "devtools.dictation.inspect",
      "devtools.dictation.deliverFixture",
    ],
    bindingSelectors: [],
  },
  {
    id: "dictation-history",
    name: "Dictation History surface",
    status: "planned",
    domains: ["targets", "elements", "layout", "storage", "input", "screenshots", "accessibility"],
    sourceFiles: [
      "src/dictation/history.rs",
      "src/dictation/types.rs",
      "src/mcp_resources/mod.rs",
    ],
    features: ["transcript rows", "search/filter", "preview", "redaction", "missing audio fallback", "selection reanchor", "portal attachment"],
    shortcuts: ["Enter", "Escape", "Tab", "ArrowUp", "ArrowDown"],
    supportedNow: ["kit://dictation-history", "filterable surface architecture"],
    missingRuntimePrimitives: [
      "fixture dictation store identity",
      "transcript row generation",
      "preview generation",
      "redacted transcript fingerprint",
      "audio path redaction proof",
      "scroll and selection anchor metrics",
    ],
    regressionRecipeRole: "Use history recipes to prevent privacy and selection regressions once resource receipts are first-class.",
    availablePrimitiveIds: [
      "devtools.targets.inspect",
      "devtools.surface.inspect",
    ],
    bindingSelectors: [
      { relation: "Direct", priority: 100, appViewVariants: ["DictationHistoryView"], hostKinds: ["MainWindow"] },
    ],
  },
];

export function coverageProfileById(id: string): CoverageProfile | undefined {
  return coverageProfiles.find((profile) => profile.id === id);
}

const coverageStatuses: readonly CoverageStatus[] = ["supported", "partial", "missing", "planned"];

/**
 * Structural validation of the typed registry. Returns human-readable errors;
 * an empty array means the registry is usable for binding generation.
 */
export function validateCoverageProfiles(
  profiles: readonly CoverageProfile[] = coverageProfiles,
  options: {
    repoRoot?: string;
    ownerExists?: (absolutePath: string) => boolean;
  } = {},
): string[] {
  const errors: string[] = [];
  const repoRoot = resolve(options.repoRoot ?? resolve(import.meta.dir, "../.."));
  const ownerExists = options.ownerExists ?? existsSync;
  const seenIds = new Set<string>();
  const knownPrimitiveIds = new Set(receiptSchemaRegistry.map((entry) => entry.primitiveId));
  const knownDomainIds = new Set(coverageDomains.map((domain) => domain.id));

  for (const profile of profiles) {
    if (seenIds.has(profile.id)) errors.push(`duplicate profile id: ${profile.id}`);
    seenIds.add(profile.id);
    if (!coverageStatuses.includes(profile.status)) {
      errors.push(`profile ${profile.id} has unknown status: ${profile.status}`);
    }
    if (profile.sourceFiles.length === 0) {
      errors.push(`profile ${profile.id} has no source owners`);
    }
    const seenOwners = new Set<string>();
    for (const owner of profile.sourceFiles) {
      if (typeof owner !== "string" || owner.trim().length === 0) {
        errors.push(`profile ${profile.id} has an empty source owner`);
        continue;
      }
      const absoluteOwner = resolve(repoRoot, owner);
      const relativeOwner = relative(repoRoot, absoluteOwner);
      if (
        isAbsolute(owner) ||
        relativeOwner === ".." ||
        relativeOwner.startsWith(`..${sep}`)
      ) {
        errors.push(`profile ${profile.id} source owner escapes repository: ${owner}`);
        continue;
      }
      if (seenOwners.has(relativeOwner)) {
        errors.push(`profile ${profile.id} lists source owner twice: ${owner}`);
        continue;
      }
      seenOwners.add(relativeOwner);
      if (!ownerExists(absoluteOwner)) {
        errors.push(`profile ${profile.id} references missing source owner: ${owner}`);
      }
    }
    for (const primitiveId of profile.availablePrimitiveIds) {
      if (!knownPrimitiveIds.has(primitiveId)) {
        errors.push(`profile ${profile.id} references unknown primitive id: ${primitiveId}`);
      }
    }
    const duplicatePrimitives = profile.availablePrimitiveIds.filter(
      (id, index) => profile.availablePrimitiveIds.indexOf(id) !== index,
    );
    for (const duplicate of duplicatePrimitives) {
      errors.push(`profile ${profile.id} lists primitive id twice: ${duplicate}`);
    }
    for (const selector of profile.bindingSelectors) {
      if (selector.relation !== "Direct" && selector.relation !== "Derived") {
        errors.push(`profile ${profile.id} selector has invalid relation`);
      }
      if (!Number.isFinite(selector.priority)) {
        errors.push(`profile ${profile.id} selector priority is not a number`);
      }
      const dimensions = [selector.contractKinds, selector.appViewVariants, selector.families, selector.hostKinds]
        .filter((dimension) => Array.isArray(dimension) && dimension.length > 0);
      if (dimensions.length === 0) {
        errors.push(`profile ${profile.id} selector matches everything (no dimensions)`);
      }
    }
    // A "supported" claim must not coexist with a required missing primitive
    // list — the prose list is human reporting, but an unqualified supported
    // status contradicting it is exactly the drift PF-009 forbids.
    if (profile.status === "supported" && profile.missingRuntimePrimitives.length > 0) {
      errors.push(
        `profile ${profile.id} is "supported" while listing missing runtime primitives; downgrade to partial or clear the list`,
      );
    }
  }
  // Non-blocking sanity: unknown domain references are structural drift too.
  for (const profile of profiles) {
    for (const domain of profile.domains) {
      if (!knownDomainIds.has(domain) && domain !== "logs") {
        errors.push(`profile ${profile.id} references unknown domain: ${domain}`);
      }
    }
  }
  return errors;
}

export interface CoverageReportArgs {
  surface?: string;
  domain?: string;
}

export function buildCoverageReport(args: CoverageReportArgs = {}) {
  const filteredDomains = args.domain
    ? coverageDomains.filter((domain) => domain.id === args.domain)
    : coverageDomains;
  const filteredSurfaces = args.surface
    ? coverageProfiles.filter((surface) => surface.id === args.surface)
    : coverageProfiles;
  const referencedDomainIds = new Set(filteredSurfaces.flatMap((surface) => surface.domains));
  const scopedDomains = args.domain
    ? filteredDomains
    : filteredDomains.filter((domain) => referencedDomainIds.has(domain.id) || !args.surface);

  const statusCounts = coverageProfiles.reduce<Record<CoverageStatus, number>>(
    (counts, surface) => {
      counts[surface.status] += 1;
      return counts;
    },
    { supported: 0, partial: 0, missing: 0, planned: 0 },
  );

  return {
    schemaVersion: 1,
    tool: "script-kit-devtools.coverage",
    generatedAt: new Date().toISOString(),
    evidenceStatus: "SOURCE-CONFIRMED" as const,
    evidenceClass: "STATIC_INVENTORY" as const,
    runtimeProof: {
      disposition: "NOT_EVALUATED" as const,
      provenSurfaceCount: 0,
      note: "A Direct profile binding and a valid source-owner inventory do not prove runtime behavior.",
    },
    philosophy: "Chrome DevTools-style protocol and API coverage first; recipes are smoke/regression wrappers after direct primitives exist.",
    inventoryNamespaces: {
      runtimeCoverageProfileCount: coverageProfiles.length,
      selectedRuntimeCoverageProfileCount: filteredSurfaces.length,
      statusCounts,
      note: "Runtime coverage profiles are not contract kinds, contract mappings, unique AppView variants, or orientation aliases.",
    },
    registryValidation: {
      errors: validateCoverageProfiles(),
      validatesSourceOwners: true,
    },
    primitiveFamilies: ["devtools.inspect", "devtools.measure", "devtools.act", "devtools.compare", "devtools.investigate"],
    domains: scopedDomains,
    surfaces: filteredSurfaces,
    criticalGaps: [
      "target-scoped layout and scroll geometry for Notes, popups, detached Agent Chat, prompt containers, and Dictation",
      "text-fit, contrast, overlap, and occlusion measurements tied to semantic ids",
      "passive media permission/model readiness and transcript delivery receipts for Dictation",
      "red/green investigation artifacts with stable metric names and missing-primitive classification",
      "semantic-to-AX parity and tab-order graphs for keyboard and accessibility bugs",
    ],
    recommendedNext: [
      "Build devtools.measure layout/text/scroll/contrast around stable target ids.",
      "Build devtools.act with safe protocol-first user actions and explicit native escalation.",
      "Build devtools.media.inspect before treating live Dictation bugs as verifiable.",
      "Add Notes target-scoped layout, editor selection, scroll anchors, and Agent Chat generation receipts.",
    ],
  };
}

function parseArgs(argv: string[]) {
  const args = {
    surface: "",
    domain: "",
    markdown: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--surface") {
      args.surface = argv[++index] ?? "";
    } else if (arg === "--domain") {
      args.domain = argv[++index] ?? "";
    } else if (arg === "--markdown") {
      args.markdown = true;
    }
  }
  return args;
}

function markdown(report: ReturnType<typeof buildCoverageReport>) {
  const lines = [
    "# Script Kit DevTools Coverage",
    "",
    report.philosophy,
    "",
    "## Inventory namespace",
    "",
    `- Runtime coverage profiles: ${report.inventoryNamespaces.runtimeCoverageProfileCount}`,
    `- Selected runtime coverage profiles: ${report.inventoryNamespaces.selectedRuntimeCoverageProfileCount}`,
    `- Statuses: supported ${report.inventoryNamespaces.statusCounts.supported}, partial ${report.inventoryNamespaces.statusCounts.partial}, missing ${report.inventoryNamespaces.statusCounts.missing}, planned ${report.inventoryNamespaces.statusCounts.planned}`,
    `- ${report.inventoryNamespaces.note}`,
    "",
    "## Domains",
    "",
    "| Domain | Chrome analogue | Current primitives | Next primitives |",
    "| --- | --- | --- | --- |",
    ...report.domains.map((domain) =>
      `| ${domain.name} | ${domain.chromeAnalogue} | ${domain.currentPrimitives.join(", ")} | ${domain.nextPrimitives.join(", ")} |`
    ),
    "",
    "## Surfaces",
    "",
    "| Surface | Status | Features | Shortcuts | Missing runtime primitives |",
    "| --- | --- | --- | --- | --- |",
    ...report.surfaces.map((surface) =>
      `| ${surface.name} | ${surface.status} | ${surface.features.join(", ")} | ${surface.shortcuts.join(", ")} | ${surface.missingRuntimePrimitives.join(", ")} |`
    ),
  ];
  return lines.join("\n");
}

export function main(argv: string[] = Bun.argv.slice(2)) {
  const args = parseArgs(argv);
  const report = buildCoverageReport(args);
  if (args.markdown) {
    console.log(markdown(report));
  } else {
    console.log(JSON.stringify(report, null, 2));
  }
}

if (import.meta.main) {
  main();
}
