#!/usr/bin/env bun
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { performance } from "node:perf_hooks";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const SCHEMA_VERSION = 1;

type Check = {
  name: string;
  status: "pass" | "fail";
  durationMs: number;
  details?: Record<string, unknown>;
  failure?: string;
};

type Receipt = {
  schemaVersion: number;
  status: "pass" | "fail";
  verifier: "modal-fast-verify";
  durationMs: number;
  thresholdMs: number;
  underThreshold: boolean;
  scope: {
    modalDefinition: string;
    excluded: string[];
    slowRuntimeProof: string;
  };
  files: string[];
  checks: Check[];
  proves: string[];
  stillNeedsRuntimeProof: string[];
};

const args = new Set(process.argv.slice(2));
const json = args.has("--json");
const thresholdArg = process.argv
  .slice(2)
  .find((arg) => arg.startsWith("--threshold-ms="));
const thresholdMs = thresholdArg
  ? Number(thresholdArg.split("=")[1])
  : 1000;

function read(relPath: string): string {
  return readFileSync(resolve(PROJECT_ROOT, relPath), "utf8");
}

function sliceBetween(
  source: string,
  startMarker: string,
  endMarker: string,
): string {
  const start = source.indexOf(startMarker);
  if (start === -1) {
    throw new Error(`missing start marker: ${startMarker}`);
  }
  const tail = source.slice(start);
  const end = tail.indexOf(endMarker);
  if (end === -1) {
    throw new Error(`missing end marker after ${startMarker}: ${endMarker}`);
  }
  return tail.slice(0, end);
}

function braceBlock(source: string, marker: string): string {
  const start = source.indexOf(marker);
  if (start === -1) {
    throw new Error(`missing block marker: ${marker}`);
  }
  const open = source.indexOf("{", start);
  if (open === -1) {
    throw new Error(`missing opening brace after marker: ${marker}`);
  }
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    const char = source[index];
    if (char === "{") depth += 1;
    if (char === "}") {
      depth -= 1;
      if (depth === 0) {
        return source.slice(start, index + 1);
      }
    }
  }
  throw new Error(`missing closing brace after marker: ${marker}`);
}

function requireContains(source: string, needles: string[]): string[] {
  return needles.filter((needle) => !source.includes(needle));
}

function requireAbsent(source: string, needles: string[]): string[] {
  return needles.filter((needle) => source.includes(needle));
}

function runCheck(
  name: string,
  fn: () => Record<string, unknown> | void,
): Check {
  const started = performance.now();
  try {
    const details = fn();
    return {
      name,
      status: "pass",
      durationMs: Number((performance.now() - started).toFixed(3)),
      details,
    };
  } catch (error) {
    return {
      name,
      status: "fail",
      durationMs: Number((performance.now() - started).toFixed(3)),
      failure: error instanceof Error ? error.message : String(error),
    };
  }
}

function assertNoMissingOrForbidden(
  missing: string[],
  forbidden: string[],
): void {
  if (missing.length > 0 || forbidden.length > 0) {
    throw new Error(JSON.stringify({ missing, forbidden }));
  }
}

const files = [
  "src/confirm/window.rs",
  "src/components/confirm_modal_shell.rs",
  "src/components/footer_chrome.rs",
  "src/platform/secondary_window_config.rs",
  "src/app_layout/build_layout_info.rs",
  "src/app_layout/collect_elements.rs",
  "src/app_impl/ui_window.rs",
  "src/app_impl/simulate_key_dispatch.rs",
  "src/prompt_handler/mod.rs",
  "scripts/agentic/scenario.ts",
  "tests/source_audits/confirm_modal_shared_shell.rs",
];

const sources = Object.fromEntries(files.map((file) => [file, read(file)]));

const checks: Check[] = [
  runCheck("confirm-popup-window-kind-and-native-background", () => {
    const confirm = sources["src/confirm/window.rs"];
    const platform = sources["src/platform/secondary_window_config.rs"];
    const confirmOptions = sliceBetween(
      confirm,
      "let handle = cx.open_window(",
      "move |_window, cx|",
    );
    const actionsConfig = sliceBetween(
      platform,
      "pub unsafe fn configure_actions_popup_window(window: id, is_dark: bool)",
      "#[cfg(not(target_os = \"macos\"))]",
    );
    const confirmConfig = sliceBetween(
      platform,
      "pub unsafe fn configure_confirm_popup_window(window: id, is_dark: bool)",
      "#[cfg(not(target_os = \"macos\"))]",
    );
    const footerConfig = sliceBetween(
      platform,
      "pub unsafe fn configure_footer_popup_window(window: id, is_dark: bool)",
      "#[cfg(not(target_os = \"macos\"))]",
    );

    assertNoMissingOrForbidden(
      requireContains(confirm, [
        "WindowKind::PopUp",
        "platform::configure_confirm_popup_window(confirm_ns_window, is_dark_vibrancy)",
        "addChildWindow:confirm_ns_window ordered:NS_WINDOW_ABOVE",
        "orderFrontRegardless",
      ]).concat(
        requireContains(confirmOptions, ["focus: false"]),
        requireContains(actionsConfig, [
          "configure_attached_popup_window(",
          "GlassMorphVariant::ContentLayer",
        ]),
        requireContains(confirmConfig, [
          "configure_attached_popup_window(",
          "GlassMorphVariant::ContentLayer",
        ]),
        requireContains(footerConfig, [
          "configure_attached_popup_window(",
          "GlassMorphVariant::FadeOnly",
          "setIgnoresMouseEvents: true",
          "setHasShadow: false",
          "setCornerRadius: 0.0_f64",
        ]),
      ),
      requireAbsent(confirmOptions, ["focus: true"]).concat(
        requireAbsent(confirm, ["makeKeyWindow"]),
        requireAbsent(confirmConfig, [
          "setHasShadow: false",
          "setCornerRadius: 0.0_f64",
        ]),
      ),
    );

    return {
      windowKind: "WindowKind::PopUp",
      windowFocus: "confirm opens focus:false and avoids native makeKeyWindow promotion",
      nativeBackground: "confirm and actions share attached-popup material and content-layer motion",
      footerException: "footer owns the fade-only no-shadow/no-corner flush strip",
    };
  }),

  runCheck("confirm-popup-reuses-footer-button-contract", () => {
    const confirm = sources["src/confirm/window.rs"];
    const shell = sources["src/components/confirm_modal_shell.rs"];
    const footer = sources["src/components/footer_chrome.rs"];
    const renderBlock = braceBlock(confirm, "impl Render for ConfirmPopupWindow");
    assertNoMissingOrForbidden(
      requireContains(confirm, [
        "footer_action_slot_width",
        "footer_button_height",
        "current_main_menu_footer_height",
        "current_main_menu_footer_metrics().item_gap_px",
        "modal_action_row",
        "ModalActionRowButton",
        "FooterHintButtonLayoutOverrides",
        "FooterActionSlot::Close",
        "FooterActionSlot::Run",
        "edge_padding_x_px",
        "shrink_frame_to_content_px",
      ]).concat(
        requireContains(renderBlock, [
          "label: self.cancel_text.clone()",
          "key: \"Esc\".into()",
          "slot_width_px: cancel_slot_width",
          "label: self.confirm_text.clone()",
          "key: \"↵\".into()",
          "slot_width_px: confirm_slot_width",
          "modal_action_row(",
        ]),
        requireContains(shell, [
          "render_footer_hint_action_button_frame(",
          "FooterHintActionButtonFrameSpec",
          "key_first: false",
          "FooterHintContentJustify::Center",
        ]),
        requireContains(footer, [
          "pub(crate) fn render_footer_hint_action_button_frame(",
          "pub(crate) struct FooterHintActionButtonFrameSpec",
        ]),
      ),
      requireAbsent(renderBlock, [
        "Button::new(self.cancel_text",
        "Button::new(self.confirm_text",
        "on_mouse_down(MouseButton::Left",
        "accent_color = if is_danger",
      ]).concat(
        requireAbsent(confirm, [
          "components::button::BUTTON_GHOST_HEIGHT",
          "BUTTON_GAP",
        ]),
      ),
    );
    return {
      sizing: "footer_button_height + FooterActionSlot widths",
      shortcuts: "footer hint button renderer with Esc and Enter",
    };
  }),

  runCheck("in-window-confirm-prompt-layout-and-elements-use-footer-contract", () => {
    const layout = sources["src/app_layout/build_layout_info.rs"];
    const elements = sources["src/app_layout/collect_elements.rs"];
    const uiWindow = sources["src/app_impl/ui_window.rs"];
    assertNoMissingOrForbidden(
      requireContains(layout, [
        "AppView::ConfirmPrompt { .. }",
        "ConfirmPromptContent",
        "ConfirmPromptStack",
        "ConfirmPromptTitle",
        "ConfirmPromptBody",
        "ConfirmPromptFooter",
        "ConfirmPromptConfirmButton",
        "ConfirmPromptCancelButton",
      ]).concat(
        requireContains(elements, [
          "AppView::ConfirmPrompt",
          "options.confirm_text",
          "options.cancel_text",
          "role = Some(\"footer\".to_string())",
          "kind = Some(\"confirm\".to_string())",
          "kind = Some(\"cancel\".to_string())",
        ]),
        requireContains(uiWindow, [
          "fn confirm_prompt_footer_buttons",
          "FooterAction::Apply",
          "FooterAction::Close",
          "\"↵\"",
          "\"Esc\"",
          "options.confirm_text.to_string()",
          "options.cancel_text.to_string()",
          "selected(confirm_focused)",
          "selected(cancel_focused)",
        ]),
      ),
      [],
    );
    return {
      layout: "ConfirmPrompt layout exposes content, stack, body, footer and button bounds",
      elements: "collector exposes confirm/cancel footer button roles",
    };
  }),

  runCheck("confirm-keyboard-and-sdk-route-contract", () => {
    const keys = sources["src/app_impl/simulate_key_dispatch.rs"];
    const handler = sources["src/prompt_handler/mod.rs"];
    const scenario = sources["scripts/agentic/scenario.ts"];
    const showConfirmBlock = sliceBetween(
      handler,
      "PromptMessage::ShowConfirm {\n                id,",
      "PromptMessage::ShowChat {",
    );
    const sdkScenarioBlock = sliceBetween(
      scenario,
      "runSdkConfirmRuntimeProofScenario",
      "export async function runLoadingSkeletonProgressRestorationStressScenario",
    );

    assertNoMissingOrForbidden(
      requireContains(keys, [
        "SimulateKey: {key} - confirm popup handled={handled}",
        "crate::confirm::consume_main_window_key_while_confirm_open(",
        "AppView::ConfirmPrompt { .. }",
        "SimulateKey: Escape - cancel ConfirmPrompt",
        "SimulateKey: Enter - confirm ConfirmPrompt",
        "toggle_confirm_prompt_focus",
      ]).concat(
        requireContains(showConfirmBlock, [
          "self.open_confirm_prompt(",
          "Message::Submit",
          "Some(\"true\".to_string())",
          "Some(\"false\".to_string())",
        ]),
        requireContains(sdkScenarioBlock, [
          "type: \"run\"",
          "tests/smoke/test-confirm-sdk-runtime.ts",
          "expectedSurface: \"ConfirmPrompt\"",
          "simulateKey",
          "sdk-confirm-escape-cancel",
        ]),
      ),
      requireAbsent(sdkScenarioBlock, ["openConfirmPrompt"]),
    );
    return {
      keys: "Tab, Enter and Escape route through ConfirmPrompt",
      sdk: "SDK proof remains assigned to real script run route",
    };
  }),

  runCheck("rust-source-audit-covers-fast-contract", () => {
    const audit = sources["tests/source_audits/confirm_modal_shared_shell.rs"];
    assertNoMissingOrForbidden(
      requireContains(audit, [
        "confirm_popup_uses_shortcut_modal_button_and_chrome_tokens",
        "confirm_popup_native_background_matches_actions_popup_not_footer_flush_strip",
        "sdk_confirm_runtime_proof_uses_real_script_run_route",
        "FooterActionSlot::Close",
        "FooterActionSlot::Run",
        "configure_attached_popup_window(",
        "GlassMorphVariant::ContentLayer",
        "GlassMorphVariant::FadeOnly",
        "SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK",
      ]),
      [],
    );
    return {
      rustAudit: "compile-backed source audit mirrors modal fast verifier invariants",
    };
  }),
];

const durationMs = Number(
  checks.reduce((total, check) => total + check.durationMs, 0).toFixed(3),
);
const status = checks.every((check) => check.status === "pass") ? "pass" : "fail";
const receipt: Receipt = {
  schemaVersion: SCHEMA_VERSION,
  status,
  verifier: "modal-fast-verify",
  durationMs,
  thresholdMs,
  underThreshold: durationMs <= thresholdMs,
  scope: {
    modalDefinition: "confirm/deny interactions only",
    excluded: ["actions menu", "trigger pickers", "generic popups"],
    slowRuntimeProof:
      "bun scripts/agentic/index.ts sdk-confirm-runtime-proof --session confirm-modal-sdk-proof --json",
  },
  files,
  checks,
  proves: [
    "Confirm popup window kind is popup and uses the same native background/depth config as actions popup.",
    "Footer popup, not confirm popup, owns the flush no-shadow/no-corner exception.",
    "Confirm popup buttons reuse footer button height, slot widths, gaps, keycaps, hover and active styling.",
    "In-window ConfirmPrompt layout and element collection expose footer-style confirm/cancel controls.",
    "ConfirmPrompt keyboard handling and SDK host route source contracts remain present.",
  ],
  stillNeedsRuntimeProof: [
    "The app actually remains responsive after a real SDK global confirm() script run.",
    "Escape resolves the live script promise to boolean false through stdin/stdout.",
    "macOS renders the expected vibrancy/background pixels in a visible NSWindow.",
    "Runtime cleanup succeeds after the SDK confirm proof.",
  ],
};

if (json) {
  process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
} else {
  process.stdout.write(
    [
      `modal-fast-verify: ${receipt.status}`,
      `durationMs: ${receipt.durationMs}`,
      `underThreshold(${receipt.thresholdMs}ms): ${receipt.underThreshold}`,
      ...checks.map((check) => `- ${check.status}: ${check.name} (${check.durationMs}ms)`),
    ].join("\n") + "\n",
  );
}

process.exit(status === "pass" && receipt.underThreshold ? 0 : 1);
