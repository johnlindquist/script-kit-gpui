//! Source-text and contract tests for Tab AI input/selection coverage.
//!
//! Validates that `current_input_text()` and `snapshot_tab_ai_ui()` produce
//! meaningful data for each supported view type, and that degraded surfaces
//! are explicitly handled rather than silently falling through.

const TAB_AI_SOURCE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");

// ---------------------------------------------------------------------------
// current_input_text coverage — every AppView variant must be named
// ---------------------------------------------------------------------------

/// Views that MUST have explicit entity-based text extraction in
/// `current_input_text()`.  These have editable user text that Tab AI
/// should surface as `inputText`.
const ENTITY_VIEWS_WITH_TEXT: &[&str] = &[
    "EditorPrompt",
    "ScratchPadView",
    "ChatPrompt",
    "PathPrompt",
    "EnvPrompt",
    "SelectPrompt",
    "NamingPrompt",
    "TemplatePrompt",
    "CreateAiPresetView",
    "BrowseKitsView",
];

/// Views that are explicitly handled as having no meaningful user text.
/// These must appear in the `current_input_text` match to prevent
/// silent fallthrough.
const EXPLICITLY_NO_TEXT: &[&str] = &[
    "DivPrompt",
    "FormPrompt",
    "TermPrompt",
    "QuickTerminalView",
    "DropPrompt",
    "WebcamView",
    "CreationFeedback",
    "ActionsDialog",
    "SettingsView",
    "InstalledKitsView",
];

#[test]
fn current_input_text_handles_all_entity_views_with_text() {
    // Find the function body
    let fn_start = TAB_AI_SOURCE
        .find("fn current_input_text(")
        .expect("current_input_text must exist");
    let fn_body = &TAB_AI_SOURCE[fn_start..];

    for view in ENTITY_VIEWS_WITH_TEXT {
        assert!(
            fn_body.contains(&format!("AppView::{view}")),
            "current_input_text must explicitly handle AppView::{view} for text extraction"
        );
    }
}

#[test]
fn current_input_text_explicitly_degrades_no_text_views() {
    let fn_start = TAB_AI_SOURCE
        .find("fn current_input_text(")
        .expect("current_input_text must exist");
    let fn_body = &TAB_AI_SOURCE[fn_start..];

    for view in EXPLICITLY_NO_TEXT {
        assert!(
            fn_body.contains(&format!("AppView::{view}")),
            "current_input_text must explicitly list AppView::{view} (not silently fall through)"
        );
    }
}

#[test]
fn current_input_text_has_no_wildcard_catchall() {
    // After exhaustive handling, there should be no `_ => None` catchall
    // in current_input_text. This ensures new AppView variants trigger
    // a compile error instead of silently degrading.
    let fn_start = TAB_AI_SOURCE
        .find("fn current_input_text(")
        .expect("current_input_text must exist");
    // Take enough of the function body to check (up to next fn or 3000 chars)
    let fn_body = &TAB_AI_SOURCE[fn_start..fn_start + 3000.min(TAB_AI_SOURCE.len() - fn_start)];

    // The wildcard `_ =>` should not appear — all variants are explicit
    let lines: Vec<&str> = fn_body.lines().collect();
    let wildcard_lines: Vec<&&str> = lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("_ =>") && !trimmed.contains("cfg")
        })
        .collect();

    assert!(
        wildcard_lines.is_empty(),
        "current_input_text should not have a wildcard `_ =>` catchall — found: {:?}",
        wildcard_lines
    );
}

// ---------------------------------------------------------------------------
// snapshot_tab_ai_ui — structured degradation logging
// ---------------------------------------------------------------------------

#[test]
fn snapshot_emits_structured_log_event() {
    let fn_start = TAB_AI_SOURCE
        .find("fn snapshot_tab_ai_ui(")
        .expect("snapshot_tab_ai_ui must exist");
    let fn_body = &TAB_AI_SOURCE[fn_start..fn_start + 2000.min(TAB_AI_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("tab_ai_snapshot_captured"),
        "snapshot_tab_ai_ui must emit a tab_ai_snapshot_captured tracing event"
    );
    assert!(
        fn_body.contains("input_status"),
        "snapshot log must include input_status field"
    );
    assert!(
        fn_body.contains("focus_status"),
        "snapshot log must include focus_status field"
    );
    assert!(
        fn_body.contains("elements_status"),
        "snapshot log must include elements_status field"
    );
    assert!(
        fn_body.contains("prompt_type"),
        "snapshot log must include prompt_type field"
    );
}

#[test]
fn snapshot_builds_invocation_receipt() {
    let fn_start = TAB_AI_SOURCE
        .find("fn snapshot_tab_ai_ui(")
        .expect("snapshot_tab_ai_ui must exist");
    let fn_body = &TAB_AI_SOURCE[fn_start..fn_start + 2000.min(TAB_AI_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("TabAiInvocationReceipt::from_snapshot"),
        "snapshot_tab_ai_ui must build a TabAiInvocationReceipt"
    );
    assert!(
        fn_body.contains("receipt.rich"),
        "snapshot log must include the receipt's rich field"
    );
    assert!(
        fn_body.contains("degradation_reasons"),
        "snapshot log must include degradation_reasons"
    );
}

// ---------------------------------------------------------------------------
// current_input_text accepts cx parameter (entity reads require App context)
// ---------------------------------------------------------------------------

#[test]
fn current_input_text_accepts_context_parameter() {
    // The function signature must accept cx for entity.read(cx) calls
    assert!(
        TAB_AI_SOURCE.contains("fn current_input_text(&self, cx:"),
        "current_input_text must accept a context parameter for entity-based text extraction"
    );
}

// ---------------------------------------------------------------------------
// EditorPrompt content_from_app method
// ---------------------------------------------------------------------------

const EDITOR_SOURCE: &str = include_str!("../src/editor/mod.rs");

#[test]
fn editor_prompt_has_content_from_app_method() {
    assert!(
        EDITOR_SOURCE.contains("fn content_from_app("),
        "EditorPrompt must expose content_from_app(&self, cx: &App) for parent-context reads"
    );
}

// ---------------------------------------------------------------------------
// TabAiUiSnapshot contract — struct fields for context quality
// ---------------------------------------------------------------------------

use script_kit_gpui::ai::TabAiUiSnapshot;

#[test]
fn ui_snapshot_default_has_empty_prompt_type() {
    let snap = TabAiUiSnapshot::default();
    assert!(snap.prompt_type.is_empty());
    assert!(snap.input_text.is_none());
    assert!(snap.focused_semantic_id.is_none());
    assert!(snap.selected_semantic_id.is_none());
    assert!(snap.visible_elements.is_empty());
}

#[test]
fn ui_snapshot_with_input_text_serializes_correctly() {
    let snap = TabAiUiSnapshot {
        prompt_type: "EditorPrompt".to_string(),
        input_text: Some("hello world".to_string()),
        focused_semantic_id: Some("editor-language".to_string()),
        selected_semantic_id: None,
        visible_elements: vec![],
    };
    let json = serde_json::to_value(&snap).expect("serialize");
    assert_eq!(json["promptType"], "EditorPrompt");
    assert_eq!(json["inputText"], "hello world");
    assert_eq!(json["focusedSemanticId"], "editor-language");
    assert!(json.get("selectedSemanticId").is_none());
}

#[test]
fn ui_snapshot_secret_env_input_does_not_leak_value() {
    // When EnvPrompt is secret, current_input_text returns "[secret]"
    // Verify that this placeholder serializes safely
    let snap = TabAiUiSnapshot {
        prompt_type: "EnvPrompt".to_string(),
        input_text: Some("[secret]".to_string()),
        ..Default::default()
    };
    let json = serde_json::to_string(&snap).expect("serialize");
    assert!(json.contains("[secret]"));
    assert!(!json.contains("my-api-key")); // paranoia check
}

// ---------------------------------------------------------------------------
// TabAiInvocationReceipt — machine-readable richness/degradation signals
// ---------------------------------------------------------------------------

use script_kit_gpui::ai::{
    TabAiDegradationReason, TabAiFieldStatus, TabAiInvocationReceipt,
    TAB_AI_INVOCATION_RECEIPT_SCHEMA_VERSION,
};

#[test]
fn receipt_rich_when_all_captured() {
    let receipt = TabAiInvocationReceipt::from_snapshot(
        "ScriptList",
        &Some("hello".to_string()),
        &Some("input:filter".to_string()),
        &Some("choice:0:slack".to_string()),
        5,
        &[],
    );
    assert!(receipt.rich);
    assert_eq!(receipt.input_status, TabAiFieldStatus::Captured);
    assert_eq!(receipt.focus_status, TabAiFieldStatus::Captured);
    assert_eq!(receipt.elements_status, TabAiFieldStatus::Captured);
    assert!(receipt.degradation_reasons.is_empty());
    assert_eq!(
        receipt.schema_version,
        TAB_AI_INVOCATION_RECEIPT_SCHEMA_VERSION
    );
}

#[test]
fn receipt_degraded_when_panel_only() {
    let receipt = TabAiInvocationReceipt::from_snapshot(
        "FormPrompt",
        &None,
        &None,
        &None,
        0,
        &["panel_only_form_prompt".to_string()],
    );
    assert!(!receipt.rich);
    assert_eq!(receipt.elements_status, TabAiFieldStatus::Degraded);
    assert!(receipt
        .degradation_reasons
        .contains(&TabAiDegradationReason::PanelOnlyElements));
    assert!(receipt
        .degradation_reasons
        .contains(&TabAiDegradationReason::MissingFocusTarget));
}

#[test]
fn receipt_input_unavailable_for_no_input_surface() {
    // Use "Webcam" — the name app_view_name() returns at runtime
    let receipt = TabAiInvocationReceipt::from_snapshot("Webcam", &None, &None, &None, 0, &[]);
    assert_eq!(receipt.input_status, TabAiFieldStatus::Unavailable);
    assert!(receipt
        .degradation_reasons
        .contains(&TabAiDegradationReason::InputNotApplicable));
}

#[test]
fn receipt_input_degraded_for_terminal() {
    // TermPrompt is not in the no-input list, so missing input is degraded
    let receipt = TabAiInvocationReceipt::from_snapshot("TermPrompt", &None, &None, &None, 0, &[]);
    assert_eq!(receipt.input_status, TabAiFieldStatus::Degraded);
    assert!(receipt
        .degradation_reasons
        .contains(&TabAiDegradationReason::InputNotExtractable));
}

#[test]
fn receipt_serializes_to_stable_json() {
    let receipt = TabAiInvocationReceipt::from_snapshot(
        "ArgPrompt",
        &Some("test".to_string()),
        &Some("input:filter".to_string()),
        &None,
        3,
        &[],
    );
    let json = serde_json::to_value(&receipt).expect("serialize");
    assert_eq!(json["promptType"], "ArgPrompt");
    assert_eq!(json["inputStatus"], "captured");
    assert_eq!(json["focusStatus"], "captured");
    assert_eq!(json["elementsStatus"], "captured");
    assert_eq!(json["rich"], true);
    assert!(json.get("degradationReasons").is_none()); // skip_serializing_if empty
}

#[test]
fn receipt_json_includes_degradation_reasons_when_present() {
    let receipt = TabAiInvocationReceipt::from_snapshot(
        "DivPrompt",
        &None,
        &None,
        &None,
        0,
        &["panel_only_div_prompt".to_string()],
    );
    let json = serde_json::to_value(&receipt).expect("serialize");
    let reasons = json["degradationReasons"]
        .as_array()
        .expect("should be array");
    assert!(!reasons.is_empty());
    // Check all reasons are strings (machine-readable)
    for reason in reasons {
        assert!(
            reason.is_string(),
            "degradation reason must be a string, got: {reason}"
        );
    }
}

#[test]
fn receipt_roundtrips_through_serde() {
    let receipt = TabAiInvocationReceipt::from_snapshot(
        "ClipboardHistory",
        &Some("search".to_string()),
        &None,
        &Some("choice:2:text".to_string()),
        8,
        &[],
    );
    let json = serde_json::to_string(&receipt).expect("serialize");
    let deserialized: TabAiInvocationReceipt = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(receipt, deserialized);
}

#[test]
fn receipt_no_semantic_elements_reason() {
    // Use "Settings" — the name app_view_name() returns at runtime
    let receipt = TabAiInvocationReceipt::from_snapshot(
        "Settings",
        &None,
        &None,
        &None,
        0,
        &[], // no warnings, no elements → unavailable
    );
    assert_eq!(receipt.elements_status, TabAiFieldStatus::Unavailable);
    assert!(receipt
        .degradation_reasons
        .contains(&TabAiDegradationReason::NoSemanticElements));
}

#[test]
fn receipt_field_status_display_matches_serde() {
    // Ensure Display output matches what serde produces
    assert_eq!(format!("{}", TabAiFieldStatus::Captured), "captured");
    assert_eq!(format!("{}", TabAiFieldStatus::Degraded), "degraded");
    assert_eq!(format!("{}", TabAiFieldStatus::Unavailable), "unavailable");
}

/// Verify that all prompt types mentioned in the design doc produce
/// a valid snapshot with the correct prompt_type string.
#[test]
fn known_prompt_types_produce_valid_snapshots() {
    let prompt_types = vec![
        "ScriptList",
        "ArgPrompt",
        "MiniPrompt",
        "MicroPrompt",
        "EditorPrompt",
        "ChatPrompt",
        "PathPrompt",
        "EnvPrompt",
        "SelectPrompt",
        "NamingPrompt",
        "TemplatePrompt",
        "ScratchPad",
        "ClipboardHistory",
        "AppLauncher",
        "WindowSwitcher",
        "FileSearch",
        "BrowseKits",
    ];

    for pt in prompt_types {
        let snap = TabAiUiSnapshot {
            prompt_type: pt.to_string(),
            input_text: Some("test".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize");
        assert_eq!(
            json["promptType"], pt,
            "prompt type {pt} must round-trip correctly"
        );
        assert_eq!(json["inputText"], "test");
    }
}

// ===========================================================================
// Context quality assertions per surface
//
// These tests go beyond routing: they verify that for each surface, the
// receipt has the correct `promptType`, the expected `inputStatus`,
// `focusStatus`, `elementsStatus`, and the right degradation reasons.
// All tests are hermetic — no live desktop, network, or GPUI context.
// ===========================================================================

use script_kit_gpui::protocol::ElementInfo;

/// Helper: build a receipt simulating a rich list-based surface with input,
/// focus, and semantic elements.
fn rich_list_receipt(prompt_type: &str) -> TabAiInvocationReceipt {
    TabAiInvocationReceipt::from_snapshot(
        prompt_type,
        &Some("filter text".to_string()),
        &Some("input:filter".to_string()),
        &Some("choice:0:first-item".to_string()),
        5,
        &[],
    )
}

/// Helper: build a receipt for a surface that has entity-derived input
/// and elements, but no focused/selected semantic ID (e.g. editor with no
/// tabstops).
fn entity_with_input_no_focus_receipt(prompt_type: &str) -> TabAiInvocationReceipt {
    TabAiInvocationReceipt::from_snapshot(
        prompt_type,
        &Some("editor content".to_string()),
        &None,
        &None,
        2,
        &[],
    )
}

/// Helper: build a receipt for a panel-only degraded surface.
fn panel_only_receipt(prompt_type: &str, warning: &str) -> TabAiInvocationReceipt {
    TabAiInvocationReceipt::from_snapshot(
        prompt_type,
        &None,
        &None,
        &None,
        1, // panel element counted
        &[warning.to_string()],
    )
}

// ---------------------------------------------------------------------------
// Rich surfaces — input + focus + semantic elements → all Captured, rich=true
// ---------------------------------------------------------------------------

/// ScriptList: filter text + focused input + selected choice → rich
#[test]
fn tab_ai_context_quality_script_list_is_rich() {
    let r = rich_list_receipt("ScriptList");
    assert_eq!(r.prompt_type, "ScriptList");
    assert!(r.rich, "ScriptList must be rich");
    assert_eq!(r.input_status, TabAiFieldStatus::Captured);
    assert_eq!(r.focus_status, TabAiFieldStatus::Captured);
    assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
    assert!(r.degradation_reasons.is_empty());
    assert!(r.has_input_text);
    assert!(r.has_focus_target);
    assert!(r.element_count >= 5);
}

/// ArgPrompt: same pattern as ScriptList — filter + choices → rich
#[test]
fn tab_ai_context_quality_arg_prompt_is_rich() {
    let r = rich_list_receipt("ArgPrompt");
    assert_eq!(r.prompt_type, "ArgPrompt");
    assert!(r.rich);
    assert_eq!(r.input_status, TabAiFieldStatus::Captured);
    assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
}

/// MiniPrompt: filter + choices → rich
#[test]
fn tab_ai_context_quality_mini_prompt_is_rich() {
    let r = rich_list_receipt("MiniPrompt");
    assert!(r.rich);
    assert_eq!(r.prompt_type, "MiniPrompt");
}

/// MicroPrompt: filter + choices → rich
#[test]
fn tab_ai_context_quality_micro_prompt_is_rich() {
    let r = rich_list_receipt("MicroPrompt");
    assert!(r.rich);
    assert_eq!(r.prompt_type, "MicroPrompt");
}

/// ClipboardHistory: filter + clipboard items → rich
#[test]
fn tab_ai_context_quality_clipboard_history_is_rich() {
    let r = rich_list_receipt("ClipboardHistory");
    assert!(r.rich);
    assert_eq!(r.prompt_type, "ClipboardHistory");
}

/// AppLauncher: filter + app list → rich
#[test]
fn tab_ai_context_quality_app_launcher_is_rich() {
    let r = rich_list_receipt("AppLauncher");
    assert!(r.rich);
    assert_eq!(r.prompt_type, "AppLauncher");
}

/// WindowSwitcher: filter + windows → rich
#[test]
fn tab_ai_context_quality_window_switcher_is_rich() {
    let r = rich_list_receipt("WindowSwitcher");
    assert!(r.rich);
}

/// FileSearch: query + file results → rich
#[test]
fn tab_ai_context_quality_file_search_is_rich() {
    let r = rich_list_receipt("FileSearch");
    assert!(r.rich);
    assert_eq!(r.prompt_type, "FileSearch");
}

/// ProcessManager: filter + processes → rich
#[test]
fn tab_ai_context_quality_process_manager_is_rich() {
    let r = rich_list_receipt("ProcessManager");
    assert!(r.rich);
}

/// CurrentAppCommands: filter + menu commands → rich
#[test]
fn tab_ai_context_quality_current_app_commands_is_rich() {
    let r = rich_list_receipt("CurrentAppCommands");
    assert!(r.rich);
}

/// EmojiPicker: filter + emoji → rich
#[test]
fn tab_ai_context_quality_emoji_picker_is_rich() {
    let r = rich_list_receipt("EmojiPicker");
    assert!(r.rich);
}

/// BrowseKits: search + kits → rich
#[test]
fn tab_ai_context_quality_browse_kits_is_rich() {
    let r = rich_list_receipt("BrowseKits");
    assert!(r.rich);
}

/// SelectPrompt: delegated element collection → rich
#[test]
fn tab_ai_context_quality_select_prompt_is_rich() {
    let r = rich_list_receipt("SelectPrompt");
    assert!(r.rich);
    assert_eq!(r.prompt_type, "SelectPrompt");
}

// ---------------------------------------------------------------------------
// Entity-based surfaces with input text — may lack focus targets
// ---------------------------------------------------------------------------

/// EditorPrompt: has content (from content_from_app) + language element,
/// but no focused/selected ID when no tabstops.
#[test]
fn tab_ai_context_quality_editor_prompt_with_content() {
    let r = entity_with_input_no_focus_receipt("EditorPrompt");
    assert_eq!(r.prompt_type, "EditorPrompt");
    assert_eq!(r.input_status, TabAiFieldStatus::Captured);
    assert!(r.has_input_text);
    // No focus targets → focus is unavailable (no warnings)
    assert_eq!(r.focus_status, TabAiFieldStatus::Unavailable);
    assert!(!r.rich, "EditorPrompt without tabstops is not fully rich");
    assert!(
        !r.degradation_reasons
            .contains(&TabAiDegradationReason::MissingFocusTarget),
        "MissingFocusTarget requires Degraded, not Unavailable"
    );
}

/// EditorPrompt WITH tabstops: rich (has focused tabstop)
#[test]
fn tab_ai_context_quality_editor_prompt_with_tabstops_is_rich() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "EditorPrompt",
        &Some("fn main() {}".to_string()),
        &Some("input:language".to_string()),
        &Some("choice:0:tabstop1".to_string()),
        3,
        &[],
    );
    assert!(r.rich, "EditorPrompt with tabstops should be fully rich");
    assert_eq!(r.input_status, TabAiFieldStatus::Captured);
    assert_eq!(r.focus_status, TabAiFieldStatus::Captured);
    assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
}

/// ScratchPadView: has editor content, may lack focus
#[test]
fn tab_ai_context_quality_scratch_pad_with_content() {
    let r = entity_with_input_no_focus_receipt("ScratchPad");
    assert_eq!(r.prompt_type, "ScratchPad");
    assert_eq!(r.input_status, TabAiFieldStatus::Captured);
    assert!(r.has_input_text);
}

/// ChatPrompt: has input text from composer
#[test]
fn tab_ai_context_quality_chat_prompt_with_input() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "ChatPrompt",
        &Some("tell me about this".to_string()),
        &Some("input:message".to_string()),
        &Some("choice:2:last-message".to_string()),
        4,
        &[],
    );
    assert!(r.rich, "ChatPrompt with input + messages should be rich");
    assert_eq!(r.prompt_type, "ChatPrompt");
}

/// PathPrompt: has current-directory + filter input
#[test]
fn tab_ai_context_quality_path_prompt_is_rich() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "PathPrompt",
        &Some("/usr/local".to_string()),
        &Some("input:path-filter".to_string()),
        &Some("choice:0:/usr/local/bin".to_string()),
        4,
        &[],
    );
    assert!(r.rich);
    assert_eq!(r.prompt_type, "PathPrompt");
}

/// EnvPrompt: has key + value (possibly masked) + keyring choice
#[test]
fn tab_ai_context_quality_env_prompt_is_rich() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "EnvPrompt",
        &Some("[secret]".to_string()),
        &Some("input:env-value".to_string()),
        &None,
        3,
        &[],
    );
    assert_eq!(r.prompt_type, "EnvPrompt");
    assert_eq!(r.input_status, TabAiFieldStatus::Captured);
    // Secret masking should still count as captured input
    assert!(r.has_input_text);
}

/// NamingPrompt: friendly name + filename
#[test]
fn tab_ai_context_quality_naming_prompt_is_rich() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "NamingPrompt",
        &Some("My Script".to_string()),
        &Some("input:friendly-name".to_string()),
        &None,
        2,
        &[],
    );
    assert_eq!(r.prompt_type, "NamingPrompt");
    assert_eq!(r.input_status, TabAiFieldStatus::Captured);
}

/// TemplatePrompt: template input + inputs list
#[test]
fn tab_ai_context_quality_template_prompt_is_rich() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "TemplatePrompt",
        &Some("{{name}}".to_string()),
        &Some("input:template-input-0".to_string()),
        &None,
        3,
        &[],
    );
    assert_eq!(r.prompt_type, "TemplatePrompt");
    assert_eq!(r.input_status, TabAiFieldStatus::Captured);
}

// ---------------------------------------------------------------------------
// Intentionally degraded / opaque surfaces — panel-only or no elements
// ---------------------------------------------------------------------------

/// DivPrompt: script-rendered HTML, always panel-only.
/// This is the canonical degraded surface: no input, no focus, only a
/// panel placeholder element.
#[test]
fn tab_ai_context_quality_div_prompt_is_degraded() {
    let r = panel_only_receipt("DivPrompt", "panel_only_div_prompt");
    assert_eq!(r.prompt_type, "DivPrompt");
    assert!(!r.rich, "DivPrompt is always degraded");
    assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
    assert_eq!(r.focus_status, TabAiFieldStatus::Degraded);
    assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::PanelOnlyElements));
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::InputNotApplicable));
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::MissingFocusTarget));
}

/// WebcamView: camera feed with no text — panel-only.
/// Uses "Webcam" — the name app_view_name() returns at runtime.
#[test]
fn tab_ai_context_quality_webcam_is_degraded() {
    let r = panel_only_receipt("Webcam", "panel_only_webcam");
    assert_eq!(r.prompt_type, "Webcam");
    assert!(!r.rich);
    assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::PanelOnlyElements));
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::InputNotApplicable));
}

/// ActionsDialog: always panel-only.
#[test]
fn tab_ai_context_quality_actions_dialog_is_degraded() {
    let r = panel_only_receipt("ActionsDialog", "panel_only_actions_dialog");
    assert_eq!(r.prompt_type, "ActionsDialog");
    assert!(!r.rich);
    assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
    assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
}

/// ThemeChooserView: always panel-only.
#[test]
fn tab_ai_context_quality_theme_chooser_is_degraded() {
    let r = panel_only_receipt("ThemeChooser", "panel_only_theme_chooser");
    assert_eq!(r.prompt_type, "ThemeChooser");
    assert!(!r.rich);
    assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::PanelOnlyElements));
}

/// CreationFeedback: read-only confirmation with semantic receipt/action elements.
#[test]
fn tab_ai_context_quality_creation_feedback_is_read_only_but_semantic() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "CreationFeedback",
        &None,
        &None,
        &Some("creation-feedback:artifact-path".to_string()),
        12,
        &[],
    );
    assert_eq!(r.prompt_type, "CreationFeedback");
    assert!(!r.rich);
    assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
    assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
    assert!(!r
        .degradation_reasons
        .contains(&TabAiDegradationReason::PanelOnlyElements));
}

/// SettingsView: no elements, no warnings, no input → all unavailable.
/// Uses "Settings" — the name app_view_name() returns at runtime.
#[test]
fn tab_ai_context_quality_settings_is_unavailable() {
    let r = TabAiInvocationReceipt::from_snapshot("Settings", &None, &None, &None, 0, &[]);
    assert_eq!(r.prompt_type, "Settings");
    assert!(!r.rich);
    assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
    assert_eq!(r.focus_status, TabAiFieldStatus::Unavailable);
    assert_eq!(r.elements_status, TabAiFieldStatus::Unavailable);
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::InputNotApplicable));
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::NoSemanticElements));
}

/// InstalledKitsView: no elements, no input.
/// Uses "InstalledKits" — the name app_view_name() returns at runtime.
#[test]
fn tab_ai_context_quality_installed_kits_is_unavailable() {
    let r = TabAiInvocationReceipt::from_snapshot("InstalledKits", &None, &None, &None, 0, &[]);
    assert_eq!(r.prompt_type, "InstalledKits");
    assert!(!r.rich);
    assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
    assert_eq!(r.elements_status, TabAiFieldStatus::Unavailable);
}

// ---------------------------------------------------------------------------
// Terminal surfaces — content exists but not user-typed → InputNotExtractable
// ---------------------------------------------------------------------------

/// TermPrompt: has terminal output lines as elements, but no extractable
/// input text (terminal content is not user-typed filter text).
#[test]
fn tab_ai_context_quality_term_prompt_input_degraded() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "TermPrompt",
        &None,
        &None,
        &None,
        10, // visible terminal lines
        &[],
    );
    assert_eq!(r.prompt_type, "TermPrompt");
    assert_eq!(r.input_status, TabAiFieldStatus::Degraded);
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::InputNotExtractable));
    // Elements are captured (terminal lines are real content)
    assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
}

/// QuickTerminalView: similar to TermPrompt — terminal content, no typed input.
#[test]
fn tab_ai_context_quality_quick_terminal_input_degraded() {
    let r = TabAiInvocationReceipt::from_snapshot("QuickTerminal", &None, &None, &None, 8, &[]);
    assert_eq!(r.prompt_type, "QuickTerminal");
    assert_eq!(r.input_status, TabAiFieldStatus::Degraded);
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::InputNotExtractable));
    assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
}

// ---------------------------------------------------------------------------
// FormPrompt — multi-field surface, elements rich when populated
// ---------------------------------------------------------------------------

/// FormPrompt with collected form fields → elements captured,
/// but input is structurally not a single text field.
#[test]
fn tab_ai_context_quality_form_prompt_with_fields() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "FormPrompt",
        &None, // FormPrompt is multi-field, no single inputText
        &Some("input:form-field-0".to_string()),
        &None,
        4,
        &[],
    );
    assert_eq!(r.prompt_type, "FormPrompt");
    assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
    assert!(r.has_focus_target);
    // Input is degraded (not unavailable — FormPrompt has text, just multi-field)
    assert_eq!(r.input_status, TabAiFieldStatus::Degraded);
}

/// FormPrompt when empty (no fields populated) falls back to panel-only
#[test]
fn tab_ai_context_quality_form_prompt_panel_fallback() {
    let r = panel_only_receipt("FormPrompt", "panel_only_form_prompt");
    assert!(!r.rich);
    assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::PanelOnlyElements));
}

// ---------------------------------------------------------------------------
// DropPrompt — degraded when empty, somewhat rich when files dropped
// ---------------------------------------------------------------------------

/// DropPrompt with dropped files → elements captured, but input unavailable
#[test]
fn tab_ai_context_quality_drop_prompt_with_files() {
    let r = TabAiInvocationReceipt::from_snapshot(
        "DropPrompt",
        &None,
        &None,
        &Some("choice:0:readme.md".to_string()),
        3,
        &[],
    );
    assert_eq!(r.prompt_type, "DropPrompt");
    assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
    assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
    assert!(r.has_focus_target);
    assert!(r
        .degradation_reasons
        .contains(&TabAiDegradationReason::InputNotApplicable));
}

// ---------------------------------------------------------------------------
// Full context blob assembly — snapshot + context round-trip
// ---------------------------------------------------------------------------

use script_kit_gpui::ai::TabAiContextBlob;
use script_kit_gpui::context_snapshot::AiContextSnapshot;

/// Verify that a full context blob assembled from a rich surface includes
/// the correct promptType and meaningful elements in the serialized JSON.
#[test]
fn tab_ai_full_context_blob_preserves_surface_quality() {
    let snap = TabAiUiSnapshot {
        prompt_type: "ScriptList".to_string(),
        input_text: Some("docker".to_string()),
        focused_semantic_id: Some("input:filter".to_string()),
        selected_semantic_id: Some("choice:0:docker-logs".to_string()),
        visible_elements: vec![
            ElementInfo::input("filter", Some("docker"), true),
            ElementInfo::choice(0, "docker-logs", "docker-logs", true),
            ElementInfo::choice(1, "docker-stop", "docker-stop", false),
        ],
    };
    let blob = TabAiContextBlob::from_parts(
        snap,
        AiContextSnapshot::default(),
        vec!["previous query".to_string()],
        None,
        vec![],
        vec![],
        "2026-03-28T00:00:00Z".to_string(),
    );

    let json = serde_json::to_value(&blob).expect("serialize");

    // Verify promptType is correct
    assert_eq!(json["ui"]["promptType"], "ScriptList");
    // Verify inputText is present
    assert_eq!(json["ui"]["inputText"], "docker");
    // Verify focused/selected IDs survived
    assert_eq!(json["ui"]["focusedSemanticId"], "input:filter");
    assert_eq!(json["ui"]["selectedSemanticId"], "choice:0:docker-logs");
    // Verify elements are present with correct types
    // ElementInfo serializes element_type as "type" with lowercase values
    let elements = json["ui"]["visibleElements"].as_array().expect("array");
    assert_eq!(elements.len(), 3);
    assert_eq!(elements[0]["type"], "input");
    assert_eq!(elements[1]["type"], "choice");
    // Verify recent inputs survived
    let inputs = json["recentInputs"].as_array().expect("array");
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0], "previous query");
}

/// Verify that a degraded surface blob omits optional fields properly.
#[test]
fn tab_ai_full_context_blob_degraded_surface_omits_optional() {
    let snap = TabAiUiSnapshot {
        prompt_type: "DivPrompt".to_string(),
        input_text: None,
        focused_semantic_id: None,
        selected_semantic_id: None,
        visible_elements: vec![ElementInfo::panel("div-prompt")],
    };
    let blob = TabAiContextBlob::from_parts(
        snap,
        AiContextSnapshot::default(),
        vec![],
        None,
        vec![],
        vec![],
        "2026-03-28T00:00:00Z".to_string(),
    );

    let json = serde_json::to_value(&blob).expect("serialize");
    assert_eq!(json["ui"]["promptType"], "DivPrompt");
    // inputText should be absent (skip_serializing_if = None)
    assert!(json["ui"].get("inputText").is_none());
    assert!(json["ui"].get("focusedSemanticId").is_none());
    assert!(json["ui"].get("selectedSemanticId").is_none());
    // recentInputs should be absent (skip_serializing_if = empty)
    assert!(json.get("recentInputs").is_none());
    // clipboard should be absent
    assert!(json.get("clipboard").is_none());
    // But visibleElements should still have the panel
    let elements = json["ui"]["visibleElements"].as_array().expect("array");
    assert_eq!(elements.len(), 1);
    assert_eq!(elements[0]["type"], "panel");
}

// ---------------------------------------------------------------------------
// Receipt <-> snapshot consistency: receipt built from snapshot data matches
// what the blob would tell an AI agent about context quality
// ---------------------------------------------------------------------------

/// For each known rich surface, verify the receipt says rich AND the blob
/// has non-empty elements and input.
#[test]
fn tab_ai_receipt_matches_blob_quality_for_rich_surfaces() {
    let rich_surfaces = vec![
        "ScriptList",
        "ArgPrompt",
        "ClipboardHistory",
        "AppLauncher",
        "FileSearch",
        "ProcessManager",
        "EmojiPicker",
    ];

    for surface in &rich_surfaces {
        let receipt = rich_list_receipt(surface);
        let snap = TabAiUiSnapshot {
            prompt_type: surface.to_string(),
            input_text: Some("filter text".to_string()),
            focused_semantic_id: Some("input:filter".to_string()),
            selected_semantic_id: Some("choice:0:first-item".to_string()),
            visible_elements: vec![
                ElementInfo::input("filter", Some("filter text"), true),
                ElementInfo::choice(0, "item", "first-item", true),
            ],
        };

        assert!(receipt.rich, "{surface} receipt must be rich");
        assert!(
            snap.input_text.is_some(),
            "{surface} snapshot must have input_text"
        );
        assert!(
            !snap.visible_elements.is_empty(),
            "{surface} snapshot must have visible_elements"
        );
        assert!(
            snap.focused_semantic_id.is_some() || snap.selected_semantic_id.is_some(),
            "{surface} snapshot must have a focus target"
        );
    }
}

/// For each known degraded surface, verify the receipt says NOT rich AND
/// the degradation_reasons are non-empty.
#[test]
fn tab_ai_receipt_matches_blob_quality_for_degraded_surfaces() {
    // Use runtime names from app_view_name(), not AppView variant names
    let degraded_surfaces = vec![
        ("DivPrompt", "panel_only_div_prompt"),
        ("Webcam", "panel_only_webcam"),
        ("ActionsDialog", "panel_only_actions_dialog"),
        ("ThemeChooser", "panel_only_theme_chooser"),
    ];

    for (surface, warning) in &degraded_surfaces {
        let receipt = panel_only_receipt(surface, warning);
        assert!(!receipt.rich, "{surface} receipt must NOT be rich");
        assert!(
            !receipt.degradation_reasons.is_empty(),
            "{surface} receipt must have degradation_reasons"
        );
        assert!(
            receipt
                .degradation_reasons
                .contains(&TabAiDegradationReason::PanelOnlyElements),
            "{surface} must report PanelOnlyElements degradation"
        );
    }
}

// ---------------------------------------------------------------------------
// app_view_name coverage — source must map every variant
// ---------------------------------------------------------------------------

/// Verify that `app_view_name` covers all surfaces known to Tab AI.
/// If a new AppView variant is added without updating app_view_name,
/// this test's source-text check will catch the gap.
#[test]
fn app_view_name_covers_all_known_surfaces() {
    let fn_start = TAB_AI_SOURCE
        .find("fn app_view_name(")
        .expect("app_view_name must exist");
    let fn_body = &TAB_AI_SOURCE[fn_start..fn_start + 3500.min(TAB_AI_SOURCE.len() - fn_start)];

    let expected_views = vec![
        "ScriptList",
        "ArgPrompt",
        "MiniPrompt",
        "MicroPrompt",
        "DivPrompt",
        "FormPrompt",
        "TermPrompt",
        "EditorPrompt",
        "SelectPrompt",
        "PathPrompt",
        "EnvPrompt",
        "DropPrompt",
        "TemplatePrompt",
        "ChatPrompt",
        "ClipboardHistoryView",
        "AppLauncherView",
        "WindowSwitcherView",
        "FileSearchView",
        "ThemeChooserView",
        "EmojiPickerView",
        "WebcamView",
        "ScratchPadView",
        "QuickTerminalView",
        "NamingPrompt",
        "CreationFeedback",
        "ActionsDialog",
        "BrowseKitsView",
        "InstalledKitsView",
        "ProcessManagerView",
        "SettingsView",
        "CurrentAppCommandsView",
    ];

    for view in expected_views {
        assert!(
            fn_body.contains(&format!("AppView::{view}")),
            "app_view_name must handle AppView::{view}"
        );
    }
}

// ---------------------------------------------------------------------------
// collect_visible_elements — source audit: every rich surface uses
// real element constructors, not just panel placeholders
// ---------------------------------------------------------------------------

const COLLECT_ELEMENTS_SOURCE: &str = include_str!("../src/app_layout/collect_elements.rs");

/// Rich list surfaces must use collect_named_rows or collect_choice_view_elements,
/// not fall through to panel placeholders.
#[test]
fn collect_elements_rich_list_surfaces_use_real_collectors() {
    let rich_list_views = vec![
        ("ClipboardHistoryView", "collect_named_rows"),
        ("AppLauncherView", "collect_named_rows"),
        ("WindowSwitcherView", "collect_named_rows"),
        ("ProcessManagerView", "collect_named_rows"),
        ("CurrentAppCommandsView", "collect_named_rows"),
        ("EmojiPickerView", "collect_named_rows"),
        ("BrowseKitsView", "collect_named_rows"),
    ];

    for (view, collector) in rich_list_views {
        // Find the AppView match arm
        let view_pattern = format!("AppView::{view}");
        let pos = COLLECT_ELEMENTS_SOURCE
            .find(&view_pattern)
            .unwrap_or_else(|| panic!("collect_visible_elements must handle {view_pattern}"));
        // Check that the next ~500 chars reference the expected collector
        let arm_body =
            &COLLECT_ELEMENTS_SOURCE[pos..pos + 800.min(COLLECT_ELEMENTS_SOURCE.len() - pos)];
        assert!(
            arm_body.contains(collector),
            "{view} should use {collector}, not a panel placeholder"
        );
    }
}

/// Choice-based surfaces must use collect_choice_view_elements.
#[test]
fn collect_elements_choice_surfaces_use_choice_collector() {
    let choice_views = vec!["ArgPrompt", "MiniPrompt", "MicroPrompt"];

    for view in choice_views {
        let view_pattern = format!("AppView::{view}");
        let pos = COLLECT_ELEMENTS_SOURCE
            .find(&view_pattern)
            .unwrap_or_else(|| panic!("collect_visible_elements must handle {view_pattern}"));
        let arm_body =
            &COLLECT_ELEMENTS_SOURCE[pos..pos + 300.min(COLLECT_ELEMENTS_SOURCE.len() - pos)];
        assert!(
            arm_body.contains("collect_choice_view_elements"),
            "{view} should use collect_choice_view_elements"
        );
    }
}

/// Entity-based prompts must use finalize_surface_outcome
/// (which handles rich vs panel-only fallback).
#[test]
fn collect_elements_entity_prompts_use_finalize_surface_outcome() {
    let entity_prompts = vec![
        "FormPrompt",
        "TermPrompt",
        "EditorPrompt",
        "PathPrompt",
        "ChatPrompt",
        "EnvPrompt",
        "DropPrompt",
        "TemplatePrompt",
        "NamingPrompt",
        "ScratchPadView",
        "QuickTerminalView",
    ];

    for view in entity_prompts {
        let view_pattern = format!("AppView::{view}");
        let pos = COLLECT_ELEMENTS_SOURCE
            .find(&view_pattern)
            .unwrap_or_else(|| panic!("collect_visible_elements must handle {view_pattern}"));
        let arm_body =
            &COLLECT_ELEMENTS_SOURCE[pos..pos + 800.min(COLLECT_ELEMENTS_SOURCE.len() - pos)];
        assert!(
            arm_body.contains("finalize_surface_outcome"),
            "{view} should use finalize_surface_outcome for rich-vs-degraded fallback"
        );
    }
}

/// Panel-only surfaces must emit a panel_only_* warning.
#[test]
fn collect_elements_panel_only_surfaces_emit_warnings() {
    let panel_only_views = vec![
        ("ThemeChooserView", "panel_only_theme_chooser"),
        ("ActionsDialog", "panel_only_actions_dialog"),
        ("DivPrompt", "panel_only_div_prompt"),
        ("WebcamView", "panel_only_webcam"),
    ];

    for (view, expected_warning) in panel_only_views {
        assert!(
            COLLECT_ELEMENTS_SOURCE.contains(expected_warning),
            "{view} must emit {expected_warning} in collect_visible_elements"
        );
    }

    assert!(
        !COLLECT_ELEMENTS_SOURCE.contains("panel_only_creation_feedback"),
        "CreationFeedback must expose semantic receipt/action elements instead of a panel-only warning"
    );
}

// ---------------------------------------------------------------------------
// Structured log field coverage in build_tab_ai_context
// ---------------------------------------------------------------------------

#[test]
fn build_tab_ai_context_from_exists() {
    assert!(
        TAB_AI_SOURCE.contains("fn build_tab_ai_context_from("),
        "build_tab_ai_context_from must exist as the context assembly function"
    );
}

// ---------------------------------------------------------------------------
// Receipt schema version stability
// ---------------------------------------------------------------------------

#[test]
fn tab_ai_receipt_schema_version_is_stable() {
    // If this changes, downstream consumers (agents, dashboards) need updating.
    // Bump intentionally, not accidentally.
    assert_eq!(
        TAB_AI_INVOCATION_RECEIPT_SCHEMA_VERSION, 1,
        "Receipt schema version changed — update downstream consumers"
    );
}

#[test]
fn tab_ai_context_schema_version_is_stable() {
    assert_eq!(
        script_kit_gpui::ai::TAB_AI_CONTEXT_SCHEMA_VERSION,
        3,
        "Context blob schema version changed — update downstream consumers"
    );
}

// ---------------------------------------------------------------------------
// Stale-selection clamping in file search
// ---------------------------------------------------------------------------

const FILE_SEARCH_RENDER_SOURCE: &str = include_str!("../src/render_builtins/file_search.rs");
const UTILITY_VIEWS_SOURCE: &str = include_str!("../src/app_execute/utility_views.rs");

#[test]
fn file_search_render_clamps_selected_index() {
    assert!(
        FILE_SEARCH_RENDER_SOURCE.contains("clamp_file_search_display_index"),
        "render_file_search must clamp the selected index before resolving the selected file"
    );
    assert!(
        FILE_SEARCH_RENDER_SOURCE.contains("clamped_selected_index"),
        "render_file_search must use a clamped selected-index variable"
    );
}

#[test]
fn file_search_key_handler_clamps_for_get_selected_file() {
    // The get_selected_file closure inside the key handler must also use
    // clamp_file_search_display_index to avoid out-of-bounds access.
    let handler_start = FILE_SEARCH_RENDER_SOURCE
        .find("let get_selected_file = ||")
        .or_else(|| FILE_SEARCH_RENDER_SOURCE.find("let get_selected_file"))
        .expect("get_selected_file closure must exist in file_search.rs");
    let handler_body = &FILE_SEARCH_RENDER_SOURCE
        [handler_start..handler_start + 300.min(FILE_SEARCH_RENDER_SOURCE.len() - handler_start)];

    assert!(
        handler_body.contains("clamp_file_search_display_index"),
        "get_selected_file must clamp the index to avoid out-of-bounds on shrinking results"
    );
}

#[test]
fn clamp_file_search_display_index_returns_none_for_empty() {
    // clamp_file_search_display_index must return None when there are no
    // display indices, not panic or return a garbage index.
    let fn_start = UTILITY_VIEWS_SOURCE
        .find("fn clamp_file_search_display_index(")
        .expect("clamp_file_search_display_index must exist");
    let fn_body =
        &UTILITY_VIEWS_SOURCE[fn_start..fn_start + 300.min(UTILITY_VIEWS_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("file_search_display_indices.is_empty()"),
        "clamping must check for empty display indices"
    );
}

#[test]
fn clamp_uses_min_not_modulo() {
    // Clamping must use .min(len - 1), not modulo, so the last valid row is
    // selected rather than wrapping around to the top.
    let fn_start = UTILITY_VIEWS_SOURCE
        .find("fn clamp_file_search_display_index(")
        .expect("clamp_file_search_display_index must exist");
    let fn_body =
        &UTILITY_VIEWS_SOURCE[fn_start..fn_start + 300.min(UTILITY_VIEWS_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains(".min("),
        "clamping must use .min() to clamp to the last valid index"
    );
    assert!(
        !fn_body.contains(" % "),
        "clamping must NOT use modulo — that causes confusing wrap-around"
    );
}

#[test]
fn selected_file_search_result_uses_clamping() {
    // selected_file_search_result must delegate to clamp_file_search_display_index
    // so AI and preview share the same clamped resolution.
    let fn_start = UTILITY_VIEWS_SOURCE
        .find("fn selected_file_search_result(")
        .expect("selected_file_search_result must exist");
    let fn_body =
        &UTILITY_VIEWS_SOURCE[fn_start..fn_start + 300.min(UTILITY_VIEWS_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("clamp_file_search_display_index"),
        "selected_file_search_result must use the clamped helper"
    );
}

#[test]
fn row_highlight_uses_clamped_index() {
    // The row highlight in render_file_search must use the clamped index,
    // not the raw selected_index, so highlight stays on a valid row.
    let render_fn_start = FILE_SEARCH_RENDER_SOURCE
        .find("fn render_file_search(")
        .expect("render_file_search must exist");
    let render_fn_body = &FILE_SEARCH_RENDER_SOURCE[render_fn_start..];

    assert!(
        render_fn_body.contains("clamped_selected_index.unwrap_or(usize::MAX)"),
        "row highlight must use clamped_selected_index (not raw selected_index)"
    );
}

// ---------------------------------------------------------------------------
// FileSearchView in current_input_text coverage
// ---------------------------------------------------------------------------

#[test]
fn file_search_view_is_in_current_input_text() {
    // FileSearchView must be explicitly handled in current_input_text
    // so Tab AI input extraction works from file search.
    let fn_start = TAB_AI_SOURCE
        .find("fn current_input_text(")
        .expect("current_input_text must exist");
    let fn_body = &TAB_AI_SOURCE[fn_start..fn_start + 4000.min(TAB_AI_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("FileSearchView"),
        "current_input_text must handle FileSearchView"
    );
}
