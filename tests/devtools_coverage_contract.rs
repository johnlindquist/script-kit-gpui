use std::fs;

const COVERAGE: &str = include_str!("../scripts/devtools/coverage.ts");
const ACTIONS_DEVTOOLS: &str = include_str!("../scripts/devtools/actions.ts");
const MAIN_DEVTOOLS: &str = include_str!("../scripts/devtools/main.ts");
const ACTIONS_DIALOG: &str = include_str!("../src/actions/dialog.rs");
const DEVTOOLS_SKILL: &str = include_str!("../.agents/skills/script-kit-devtools/SKILL.md");
const COVERAGE_MAP: &str =
    include_str!("../.agents/skills/script-kit-devtools/references/devtools-api-coverage-map.md");
const COVERAGE_AUDIT: &str =
    include_str!("../.agents/skills/script-kit-devtools/references/devtools-coverage-audit.md");

#[test]
fn coverage_cli_is_a_devtools_primitive_not_a_recipe() {
    for needle in [
        "script-kit-devtools.coverage",
        "primitiveFamilies",
        "devtools.inspect",
        "devtools.measure",
        "devtools.act",
        "devtools.compare",
        "devtools.investigate",
        "--surface",
        "--domain",
        "--markdown",
    ] {
        assert!(
            COVERAGE.contains(needle),
            "coverage CLI must expose DevTools primitive contract: {needle}"
        );
    }

    assert!(
        !COVERAGE.contains("scripts/agentic/index.ts"),
        "coverage CLI must not route through the agentic recipe catalog"
    );
}

#[test]
fn coverage_domains_match_chrome_devtools_breadth() {
    for domain in [
        "Targets and Windows",
        "Elements and Semantics",
        "Layout and Box Model",
        "Styles, Theme, and Text Fit",
        "Console, Logs, and Events",
        "Sources, Scripts, and Owners",
        "Performance and Timeline",
        "Storage, Resources, and Privacy",
        "Accessibility",
        "Input, Focus, and Actions",
        "Media, Sensors, and Permissions",
        "Screenshots and Visual Proof",
        "Investigation Records",
    ] {
        assert!(
            COVERAGE.contains(domain),
            "coverage CLI must include Chrome-style domain: {domain}"
        );
        assert!(
            COVERAGE_MAP.contains(domain),
            "coverage map docs must explain Chrome-style domain: {domain}"
        );
    }
}

#[test]
fn coverage_pins_notes_features_shortcuts_and_missing_primitives() {
    for needle in [
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
        "src/notes/window.rs",
        "src/notes/window/keyboard.rs",
        "src/notes/window/ai_handoff.rs",
        "src/notes/actions_panel.rs",
        "src/notes/storage.rs",
        "Cmd+N",
        "Cmd+Shift+N",
        "Cmd+P",
        "Cmd+K",
        "Cmd+W",
        "Cmd+Shift+P",
        "Cmd+F",
        "Cmd+Shift+F",
        "Cmd+Shift+T",
        "Cmd+.",
        "Cmd+Shift+.",
        "Cmd+Shift+S",
        "Cmd+Z",
        "Cmd+D",
        "Cmd+Shift+D",
        "Cmd+Shift+X",
        "Cmd+Shift+L",
        "Cmd+Enter",
        "Cmd+Shift+A",
        "Cmd+Shift+O",
        "Cmd+1..Cmd+9",
        "Shift+Tab",
        "Alt+Shift+Up",
        "Ctrl+Shift+K",
        "Escape",
        "Tab",
        "Enter",
        "getLayoutInfo(target notes) NotesWindow/titlebar/editor/footer/panel bounds",
        "getState(target notes) editor scroll metrics and mounted preview anchor availability",
        "target-scoped batch togglePreview",
        "preview scroll handle populated content bounds",
        "getState(target notes) redacted active note",
        "getState(target notes) redacted draft snapshot fingerprint",
        "notes.resize-compare sandboxed auto-resize before/after receipt",
        "notes AI handoff receipt",
    ] {
        assert!(
            COVERAGE.contains(needle),
            "coverage CLI must keep Notes DevTools coverage explicit: {needle}"
        );
    }
}

#[test]
fn coverage_pins_dictation_states_media_and_privacy_boundaries() {
    for needle in [
        "idle/hidden",
        "recording",
        "quiet recording",
        "active speech",
        "confirming",
        "Script Kit target delivery",
        "Notes editor target delivery",
        "Agent Chat target delivery",
        "Tab AI target delivery",
        "external app target delivery",
        "frontmost app paste delivery",
        "stop confirmation",
        "transcribing",
        "delivering",
        "finished",
        "failed/error",
        "Idle -> Recording",
        "Recording -> Confirming",
        "Transcribing -> Delivering",
        "Delivering -> Finished",
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
        "dictation hotkey",
        "target badge click",
        "devtools.media.inspect",
        "dictation.deliver-fixture pushDictationResult target delivery generation, transcript fingerprint, and main-filter insertion range receipt",
        "passive microphone permission status",
        "hotkey binding snapshot",
        "src/dictation/window.rs",
        "src/dictation/runtime.rs",
        "src/dictation/types.rs",
        "src/dictation/setup.rs",
        "src/main_entry/runtime_tray_hotkeys.rs",
    ] {
        assert!(
            COVERAGE.contains(needle),
            "coverage CLI must keep Dictation DevTools coverage explicit: {needle}"
        );
    }
}

#[test]
fn coverage_pins_focused_text_mini_agent_chat_and_missing_runtime_proof() {
    for needle in [
        "Focused-text mini Agent Chat",
        "whole focused-field capture before main-window focus",
        "main-window mini Agent Chat mode",
        "prompt placeholder Edit, refine, ask...",
        "Thinking... processing state",
        "Replace, Append, Copy, and Chat actions",
        "expanded same-session Agent Chat panel",
        "Cue - N turns header",
        "Stop and Retry controls",
        "Agent Chat Pi Text profile executor",
        "isolated focused-text Pi cwd",
        "warm Pi session prepare/acquire/dismiss-reset",
        "no Agent Chat backend fallback for focused-text mini",
        "privacy-safe prompt and output logging",
        "getAgentChatState(target main) uiVariant focused-text-mini",
        "getAgentChatState(target main) redacted focusedText char count, capabilities, output-ready, and last-apply envelope",
        "getElements(target main) focused-text-mini-root, focused-text-input, focused-text-preview semantic ids",
        "getElements(target main) focused-text Replace, Append, Copy, Expand, Stop, Retry semantic action ids",
        "openFocusedTextAgentChatWithMockData stdin fixture for mock focused text and deterministic Agent Chat output",
        "openFocusedTextAgentChatWithPiData stdin fixture for real warm Pi Text-profile stream proof",
        "TextEdit capture/replace/append receipts",
        "native double-Command trigger delivery proof",
        "src/app_impl/agent_handoff/focused_text_entry.rs",
        "src/ai/agent_chat/ui/view.rs",
        "src/ai/agent_chat/ui/ui_variant.rs",
        "src/ai/focused_text/platform_bridge.rs",
        "src/ai/agent_chat/launch.rs",
        "src/ai/agent_chat/profiles.rs",
        "src/platform/accessibility/focused_text.rs",
        "src/app_layout/collect_elements.rs",
    ] {
        assert!(
            COVERAGE.contains(needle),
            "coverage CLI must keep focused-text mini Agent Chat coverage explicit: {needle}"
        );
    }
}

#[test]
fn main_cli_reports_open_close_freshness_proof() {
    for needle in [
        "--prove-open-close-freshness",
        "openCloseFreshnessProof",
        "main.openCloseFreshnessProof",
        "target-scoped main-window open/close stale-view freshness proof",
        "markerApplied",
        "closeObserved",
        "reopenVisible",
        "noStaleInputValue",
        "targetStable",
        "sampledReopenFrames",
        "rawValueRedacted",
        "blocked-by-stale-view",
    ] {
        assert!(
            MAIN_DEVTOOLS.contains(needle)
                || COVERAGE.contains(needle)
                || DEVTOOLS_SKILL.contains(needle),
            "main DevTools CLI must expose open/close freshness proof receipt: {needle}"
        );
    }
}

#[test]
fn main_coverage_mentions_open_close_freshness_proof() {
    assert!(
        COVERAGE.contains("target-scoped main-window open/close stale-view freshness proof"),
        "coverage CLI must advertise main-window open/close stale-view freshness proof"
    );
}

#[test]
fn main_cli_uses_target_scoped_state_not_screenshots() {
    for needle in [
        "target: { type: \"main\" }",
        "summaryOnly: true",
        "scripts/devtools/targets.ts",
        "strictTargetMatch",
        "samplesAfterReopen",
    ] {
        assert!(
            MAIN_DEVTOOLS.contains(needle),
            "main DevTools CLI must use target-scoped protocol state: {needle}"
        );
    }
    assert!(
        !MAIN_DEVTOOLS.contains("captureScreenshot"),
        "main open/close freshness proof must not use screenshots as proof"
    );
}

#[test]
fn main_cli_reports_early_frame_freshness_proof() {
    for needle in [
        "--prove-early-frame-freshness",
        "earlyFrameFreshnessProof",
        "main.earlyFrameFreshnessProof",
        "firstVisibleFrameFresh",
        "noPromptIdOnReopen",
        "noActivePopupOnReopen",
        "footerSurfaceFresh",
        "generationFieldsAvailable",
        "generationMonotonic",
        "generationMonotonicWhenAvailable",
        "blocked-by-missing-primitive",
        "blocked-by-stale-view",
    ] {
        assert!(
            MAIN_DEVTOOLS.contains(needle)
                || COVERAGE.contains(needle)
                || DEVTOOLS_SKILL.contains(needle),
            "main DevTools CLI must expose early-frame freshness proof receipt: {needle}"
        );
    }
}

#[test]
fn main_coverage_mentions_early_frame_freshness_proof() {
    assert!(
        COVERAGE.contains(
            "target-scoped main-window early-frame surface/footer/chrome freshness proof"
        ),
        "coverage CLI must advertise main-window early-frame surface/footer/chrome freshness proof"
    );
}

#[test]
fn main_cli_early_frame_freshness_uses_target_scoped_state_not_screenshots() {
    for needle in [
        "target: { type: \"main\" }",
        "summaryOnly: true",
        "activePopupPresent",
        "activeFooter",
        "surfaceContract",
        "targetGeneration",
        "surfaceGeneration",
        "dataGeneration",
        "generationFieldsAvailable",
        "generationMonotonic",
        "blocked-by-missing-primitive",
    ] {
        assert!(
            MAIN_DEVTOOLS.contains(needle),
            "early-frame freshness proof must use target-scoped protocol state: {needle}"
        );
    }
    assert!(
        !MAIN_DEVTOOLS.contains("captureScreenshot"),
        "main early-frame freshness proof must not use screenshots as proof"
    );
}

#[test]
fn coverage_source_files_exist_for_notes_and_dictation() {
    for path in [
        "src/notes/window.rs",
        "src/notes/window/keyboard.rs",
        "src/notes/window/ai_handoff.rs",
        "src/notes/window/window_ops.rs",
        "src/notes/actions_panel.rs",
        "src/notes/window/panels.rs",
        "src/notes/storage.rs",
        "src/notes/model.rs",
        "src/dictation/window.rs",
        "src/dictation/runtime.rs",
        "src/dictation/types.rs",
        "src/dictation/setup.rs",
        "src/dictation/capture.rs",
        "src/dictation/device.rs",
        "src/dictation/transcription.rs",
        "src/main_entry/runtime_tray_hotkeys.rs",
        "src/app_impl/agent_handoff/focused_text_entry.rs",
        "src/ai/agent_chat/ui/view.rs",
        "src/ai/agent_chat/ui/ui_variant.rs",
        "src/ai/focused_text/platform_bridge.rs",
        "src/ai/agent_chat/launch.rs",
        "src/ai/agent_chat/profiles.rs",
        "src/platform/accessibility/focused_text.rs",
        "src/platform/accessibility/mutation.rs",
        "src/app_layout/collect_elements.rs",
    ] {
        assert!(
            fs::metadata(path).is_ok(),
            "coverage source reference should exist: {path}"
        );
        assert!(
            COVERAGE.contains(path),
            "coverage CLI should expose source reference: {path}"
        );
    }
}

#[test]
fn docs_route_agents_to_coverage_before_more_scripts() {
    for needle in [
        "bun scripts/devtools/coverage.ts",
        "Chrome-DevTools-level breadth",
        "Notes coverage",
        "Dictation coverage",
        "coverage command",
    ] {
        assert!(
            DEVTOOLS_SKILL.contains(needle)
                || COVERAGE_MAP.contains(needle)
                || COVERAGE_AUDIT.contains(needle),
            "DevTools docs must route agents through coverage map: {needle}"
        );
    }

    assert!(
        COVERAGE_MAP.contains("Recipes should only wrap these primitives"),
        "coverage docs must keep recipes bounded to regression wrappers"
    );
}

#[test]
fn devtools_coverage_artifacts_are_checked_in() {
    for path in [
        "scripts/devtools/coverage.ts",
        ".agents/skills/script-kit-devtools/references/devtools-api-coverage-map.md",
    ] {
        assert!(
            fs::metadata(path).is_ok(),
            "expected checked-in DevTools coverage artifact at {path}"
        );
    }
}

#[test]
fn actions_cli_reports_protocol_hover_proof() {
    for needle in [
        "--prove-hover",
        "hoverProof",
        "simulateGpuiEvent",
        "target-scoped ActionsDialog hover proof",
        "noNativeEscalation",
        "submitAttempted",
        "activationAttempted",
        "hoveredRequestedRow",
        "popupLogicalPx",
    ] {
        assert!(
            ACTIONS_DEVTOOLS.contains(needle),
            "actions DevTools CLI must expose protocol hover proof receipt: {needle}"
        );
    }
}

#[test]
fn actions_dialog_rows_expose_mouse_move_hover_state() {
    for needle in [
        "fn update_hovered_row_from_popup_y",
        "let list_top = if search_at_top { search_height } else { 0.0 } + header_height",
        "this.update_hovered_row_from_popup_y(f32::from(event.position.y), cx)",
        ".on_mouse_move({",
        "move |_event: &gpui::MouseMoveEvent, _window, cx|",
        "this.hovered_row = Some(ix)",
        "cx.notify()",
        ".on_hover({",
    ] {
        assert!(
            ACTIONS_DIALOG.contains(needle),
            "actions row hover must update from protocol mouseMove and native hover: {needle}"
        );
    }
}

#[test]
fn actions_cli_reports_protocol_click_select_proof() {
    for needle in [
        "--prove-click-select",
        "clickSelectProof",
        "actions.clickSelectProof",
        "target-scoped ActionsDialog first-click selection proof",
        "mouseDown",
        "mouseUp",
        "selectedRequestedRow",
        "mouseArmedRequestedRow",
        "activationObserved",
    ] {
        assert!(
            ACTIONS_DEVTOOLS.contains(needle),
            "actions DevTools CLI must expose first-click select proof receipt: {needle}"
        );
    }
}

#[test]
fn actions_cli_reports_protocol_click_activate_proof() {
    for needle in [
        "--prove-click-activate",
        "clickActivateProof",
        "actions.clickActivateProof",
        "target-scoped ActionsDialog second-click activation proof",
        "CLICK_ACTIVATE_ALLOWED_ACTION_IDS",
        "toggle_info",
        "firstClickSelectedRequestedRow",
        "firstClickArmedRequestedRow",
        "secondClickDispatchedExactHandle",
        "sourceClosed",
        "parentLive",
        "destructiveActionAllowed",
    ] {
        assert!(
            ACTIONS_DEVTOOLS.contains(needle),
            "actions DevTools CLI must expose second-click activation proof receipt: {needle}"
        );
    }
}

#[test]
fn actions_cli_top_level_classification_aggregates_requested_proofs() {
    let final_start = ACTIONS_DEVTOOLS
        .find("const finalClassification = firstNonOkClassification([")
        .expect("actions CLI must aggregate final classification through helper");
    let final_block =
        &ACTIONS_DEVTOOLS[final_start..(final_start + 700).min(ACTIONS_DEVTOOLS.len())];

    for needle in [
        "classification",
        "hoverProof?.classification",
        "clickSelectProof?.classification",
        "clickActivateProof?.classification",
        "semanticFreshnessProof?.classification",
        "closeCleanupProof?.classification",
        "shortcutOpenFreshnessProof?.classification",
        "shortcutCloseCleanupProof?.classification",
        "escapeCloseCleanupProof?.classification",
    ] {
        assert!(
            final_block.contains(needle),
            "actions CLI final classification must include requested proof classification: {needle}"
        );
    }

    assert!(
        ACTIONS_DEVTOOLS.contains("function firstNonOkClassification("),
        "actions CLI must keep a fail-closed classification aggregator"
    );
}

#[test]
fn actions_dialog_coverage_mentions_second_click_activation_proof() {
    for needle in [
        "target-scoped ActionsDialog second-click activation lifecycle proof",
        "target-scoped ActionsDialog first-click selection proof",
    ] {
        assert!(
            COVERAGE.contains(needle),
            "coverage CLI must advertise ActionsDialog click lifecycle proof: {needle}"
        );
    }
}

#[test]
fn actions_cli_reports_semantic_freshness_proof() {
    for needle in [
        "--prove-semantic-freshness",
        "semanticFreshnessProof",
        "actions.semanticFreshnessProof",
        "elementsSelectedMatchesRowGeometry",
        "selectedNodeMatches",
        "noPanelOnlyFallback",
        "blocked-by-stale-view",
    ] {
        assert!(
            ACTIONS_DEVTOOLS.contains(needle),
            "actions DevTools CLI must expose semantic freshness proof receipt: {needle}"
        );
    }
}

#[test]
fn actions_dialog_elements_use_grouped_visual_index_semantic_ids() {
    let collector = fs::read_to_string("src/windows/automation_surface_collector.rs")
        .expect("read automation_surface_collector.rs");
    for needle in [
        "for (visual_index, grouped_item) in dialog.grouped_items.iter().enumerate()",
        "crate::actions::GroupedActionItem::Item(filter_idx)",
        "dialog.selected_index == visual_index",
        "format!(\"choice:{visual_index}:{}\", action.id)",
        "Some(visual_index)",
    ] {
        assert!(
            collector.contains(needle),
            "ActionsDialog semantic collector must use grouped visual indexes: {needle}"
        );
    }
}

#[test]
fn actions_dialog_coverage_mentions_semantic_freshness_proof() {
    assert!(
        COVERAGE.contains(
            "target-scoped ActionsDialog semantic freshness proof after first-click selection"
        ),
        "coverage CLI must advertise ActionsDialog semantic freshness proof"
    );
}

#[test]
fn actions_cli_reports_close_cleanup_proof() {
    for needle in [
        "--prove-close-cleanup",
        "closeCleanupProof",
        "actions.closeCleanupProof",
        "sourceTargetGone",
        "elementsNotFresh",
        "staleEventRefused",
        "noExactHandleDispatchAfterClose",
        "blocked-by-stale-view",
    ] {
        assert!(
            ACTIONS_DEVTOOLS.contains(needle),
            "actions DevTools CLI must expose close-cleanup stale-target proof: {needle}"
        );
    }
}

#[test]
fn actions_cli_reports_shortcut_open_first_frame_freshness_proof() {
    for needle in [
        "--prove-shortcut-open-freshness",
        "shortcutOpenFreshnessProof",
        "actions.shortcutOpenFreshnessProof",
        "cmdKDispatched",
        "openedViaShortcut",
        "firstObservableFrameTargetStable",
        "everySampleTargetStable",
        "parentStillMainScriptList",
        "attachedPopupGenerationAvailable",
        "generationMonotonicWhenAvailable",
        "chromeContractOk",
        "actionsDialogFooterless",
        "noFooterOwnershipLeak",
        "blocked-by-missing-primitive",
        "blocked-by-stale-view",
    ] {
        assert!(
            ACTIONS_DEVTOOLS.contains(needle)
                || COVERAGE.contains(needle)
                || DEVTOOLS_SKILL.contains(needle),
            "ActionsDialog DevTools CLI must expose Cmd+K shortcut-open first-frame freshness proof: {needle}"
        );
    }
}

#[test]
fn actions_cli_reports_shortcut_close_cleanup_proof() {
    for needle in [
        "--prove-shortcut-close-cleanup",
        "shortcutCloseCleanupProof",
        "actions.shortcutCloseCleanupProof",
        "cmdKOpenDispatched",
        "openedViaShortcut",
        "cmdKCloseDispatched",
        "sourceTargetGone",
        "elementsNotFresh",
        "staleEventRefused",
        "noExactHandleDispatchAfterClose",
        "parentLiveAfterClose",
        "parentScriptListStableAfterClose",
        "noActivePopupAfterClose",
        "noFooterOwnershipLeakAfterClose",
    ] {
        assert!(
            ACTIONS_DEVTOOLS.contains(needle)
                || COVERAGE.contains(needle)
                || DEVTOOLS_SKILL.contains(needle),
            "ActionsDialog DevTools CLI must expose Cmd+K shortcut-close cleanup proof: {needle}"
        );
    }
}

#[test]
fn actions_cli_reports_escape_close_cleanup_proof() {
    for needle in [
        "--prove-escape-close-cleanup",
        "escapeCloseCleanupProof",
        "actions.escapeCloseCleanupProof",
        "cmdKOpenDispatched",
        "openedViaShortcut",
        "escapeCloseDispatched",
        "sourceTargetGone",
        "elementsNotFresh",
        "staleEventRefused",
        "noExactHandleDispatchAfterClose",
        "parentLiveAfterClose",
        "parentScriptListStableAfterClose",
        "noActivePopupAfterClose",
        "noFooterOwnershipLeakAfterClose",
    ] {
        assert!(
            ACTIONS_DEVTOOLS.contains(needle)
                || COVERAGE.contains(needle)
                || DEVTOOLS_SKILL.contains(needle),
            "ActionsDialog DevTools CLI must expose Escape close cleanup proof: {needle}"
        );
    }
}

#[test]
fn actions_popup_defer_close_removes_runtime_handle() {
    let source = fs::read_to_string("src/actions/window.rs").expect("read src/actions/window.rs");
    let start = source.find("fn defer_close").expect("defer_close exists");
    let end = source[start..]
        .find("fn request_close")
        .map(|idx| start + idx)
        .expect("request_close follows defer_close");
    let body = &source[start..end];
    assert!(
        body.contains("remove_runtime_window_handle(\"actions-dialog\")")
            || body.contains("unregister_actions_dialog_automation_surfaces()"),
        "activation-driven ActionsDialog close must remove the runtime handle, not only the automation window"
    );
}

#[test]
fn actions_dialog_coverage_mentions_close_cleanup_proof() {
    assert!(
        COVERAGE.contains("target-scoped ActionsDialog close cleanup proof after activation"),
        "coverage CLI must advertise ActionsDialog post-close stale-target cleanup proof"
    );
}

#[test]
fn actions_dialog_rows_expose_mouse_click_arm_state() {
    for needle in [
        "\"mouseArmed\": self.mouse_armed_row == Some(visual_index)",
        "let mouse_armed_row = self.mouse_armed_row.and_then(|armed_index|",
        "\"mouseArmedRowAvailable\": true",
        "\"state\": if mouse_armed_row.is_some() { \"armed\" } else { \"none\" }",
        "\"row\": mouse_armed_row",
    ] {
        assert!(
            ACTIONS_DIALOG.contains(needle),
            "ActionsDialog row geometry must expose click-arm state: {needle}"
        );
    }
}
