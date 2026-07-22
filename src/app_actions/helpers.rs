// Actions handling methods - extracted from app_impl.rs
// This file is included via include!() macro in main.rs
// Contains: handle_action, trigger_action_by_name
//
// ============================================================================
// Feedback Consistency Matrix
// ============================================================================
//
// Every action handler MUST follow these rules:
//
// | Category             | Feedback Type | When                                      |
// |----------------------|---------------|-------------------------------------------|
// | **Side-effect: copy**| HUD           | Clipboard write succeeded                 |
// | **Side-effect: paste**| HUD (flash)  | Simulated Cmd+V succeeded                 |
// | **Side-effect: pin** | HUD           | Clipboard pin/unpin toggled               |
// | **Side-effect: share**| HUD          | Share sheet opened                        |
// | **Side-effect: save**| HUD           | File/snippet saved to disk                |
// | **Side-effect: delete**| HUD         | Entry/script moved to trash               |
// | **Side-effect: shortcut/alias change** | HUD | Shortcut removed, alias removed    |
// | **Side-effect: reload**| HUD         | Scripts reloaded                          |
// | **Side-effect: system action**| HUD  | System action (volume, dark mode) ok      |
// | **Side-effect: open external**| HUD  | Editor/Finder/app launched                |
// | **Side-effect: OCR** | HUD (via copy)| Text extracted and copied                 |
// | **View transition**  | Silent        | Opening ClipboardHistory, EmojiPicker,    |
// |                      |               | AppLauncher, WindowSwitcher, FileSearch,  |
// |                      |               | ThemeChooser, Webcam,                   |
// |                      |               | ScratchPad, QuickTerminal, ShortcutRec,   |
// |                      |               | AliasInput, NamingDialog, ActionsDialog   |
// | **Info / coming soon**| Toast (info) | Feature not yet available, empty state     |
// | **Warning**          | Toast (warning)| Missing permissions, unsupported platform |
// | **Error**            | Toast (error) | ALL failure paths — no exceptions          |
//
// Rules (enforced by tests in app_actions_tests):
//  1. Never use both HUD and Toast for the same action path.
//  2. All error paths MUST show Toast with .error() variant.
//  3. All HUD/Toast calls MUST use named duration constants below — no inline ms.
//  4. View transitions are Silent; the new view IS the feedback.
//  5. Use show_error_toast() helper for errors, copy_to_clipboard_with_feedback()
//     for clipboard writes, show_unsupported_platform_toast() for platform guards.
// ============================================================================

pub(crate) const HUD_FLASH_MS: u64 = 1000;
pub(crate) const HUD_SHORT_MS: u64 = 1500;
pub(crate) const HUD_MEDIUM_MS: u64 = 2000;
pub(crate) const HUD_2200_MS: u64 = 2200;
pub(crate) const HUD_2500_MS: u64 = 2500;
pub(crate) const HUD_LONG_MS: u64 = 3000;
pub(crate) const HUD_CONFLICT_MS: u64 = 4000;
pub(crate) const HUD_SLOW_MS: u64 = 5000;

pub(crate) const TOAST_SUCCESS_MS: u64 = 2500;
pub(crate) const TOAST_INFO_MS: u64 = 3500;
pub(crate) const TOAST_WARNING_MS: u64 = 5000;
pub(crate) const TOAST_ERROR_MS: u64 = 5000;
pub(crate) const TOAST_ERROR_DETAILED_MS: u64 = 8000;
pub(crate) const TOAST_CRITICAL_MS: u64 = 10000;

/// Unsupported platform message for macOS-only features.
/// Returns a consistent "only supported on macOS" string for the given feature.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn unsupported_platform_message(feature: &str) -> String {
    format!("{} is only supported on macOS", feature)
}

fn select_clipboard_entry_meta<'a>(
    entries: &'a [clipboard_history::ClipboardEntryMeta],
    filter: &str,
    selected_index: usize,
) -> Option<&'a clipboard_history::ClipboardEntryMeta> {
    if filter.is_empty() {
        if entries.is_empty() {
            return None;
        }
        let clamped_index = selected_index.min(entries.len().saturating_sub(1));
        return entries.get(clamped_index);
    }

    let filter_lower = filter.to_lowercase();
    let filtered_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.text_preview.to_lowercase().contains(&filter_lower))
        .collect();

    if filtered_entries.is_empty() {
        return None;
    }

    let clamped_index = selected_index.min(filtered_entries.len().saturating_sub(1));
    filtered_entries.get(clamped_index).copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchActionFeedbackPlan {
    Open,
    QuickLook,
    OpenWith,
    ShowInfo,
    Unsupported,
}

impl FileSearchActionFeedbackPlan {
    fn from_action_id(action_id: &str) -> Self {
        match action_id {
            "open_file" | "open_directory" => Self::Open,
            "quick_look" => Self::QuickLook,
            "open_with" => Self::OpenWith,
            "show_info" => Self::ShowInfo,
            _ => Self::Unsupported,
        }
    }

    fn success_hud(self) -> Option<&'static str> {
        match self {
            Self::Open => Some("Opened"),
            Self::QuickLook => Some("Quick Look opened"),
            Self::OpenWith => Some("Open With opened"),
            Self::ShowInfo => Some("Info opened"),
            Self::Unsupported => None,
        }
    }

    fn error_prefix(self) -> Option<&'static str> {
        match self {
            Self::Open => Some("Failed to open"),
            Self::QuickLook => Some("Failed to Quick Look"),
            Self::OpenWith => Some("Failed to Open With"),
            Self::ShowInfo => Some("Failed to Show Info"),
            Self::Unsupported => None,
        }
    }
}

fn file_search_action_success_hud(action_id: &str) -> Option<&'static str> {
    FileSearchActionFeedbackPlan::from_action_id(action_id).success_hud()
}

fn file_search_action_error_hud_prefix(action_id: &str) -> Option<&'static str> {
    FileSearchActionFeedbackPlan::from_action_id(action_id).error_prefix()
}

fn should_transition_to_script_list_after_action(current_view: &AppView) -> bool {
    matches!(current_view, AppView::ScriptList | AppView::ActionsDialog)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionRequiredMessagePlan {
    CopyPath,
    CopyDeeplink,
    ConfigureShortcut,
    RemoveShortcut,
    ConfigureAlias,
    RemoveAlias,
    EditScript,
    EditScriptlet,
    RevealScriptlet,
    CopyScriptletPath,
    CopyContent,
    RemoveScript,
    ResetRanking,
    RunScriptletAction,
    Default,
}

impl SelectionRequiredMessagePlan {
    fn from_action_id(action_id: &str) -> Self {
        match action_id {
            "copy_path" => Self::CopyPath,
            "copy_deeplink" => Self::CopyDeeplink,
            "configure_shortcut" | "add_shortcut" | "update_shortcut" => Self::ConfigureShortcut,
            "remove_shortcut" => Self::RemoveShortcut,
            "add_alias" | "update_alias" => Self::ConfigureAlias,
            "remove_alias" => Self::RemoveAlias,
            "edit_script" => Self::EditScript,
            "edit_scriptlet" => Self::EditScriptlet,
            "reveal_scriptlet_in_finder" => Self::RevealScriptlet,
            "copy_scriptlet_path" => Self::CopyScriptletPath,
            "copy_content" => Self::CopyContent,
            "remove_script" | "delete_script" => Self::RemoveScript,
            "reset_ranking" => Self::ResetRanking,
            action if action.starts_with("scriptlet_action:") => Self::RunScriptletAction,
            _ => Self::Default,
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::CopyPath => "Select an item to copy its path.",
            Self::CopyDeeplink => "Select an item to copy its deeplink.",
            Self::ConfigureShortcut => "Select an item to configure its shortcut.",
            Self::RemoveShortcut => "Select an item to remove its shortcut.",
            Self::ConfigureAlias => "Select an item to add or update its alias.",
            Self::RemoveAlias => "Select an item to remove its alias.",
            Self::EditScript => "Select a script to edit.",
            Self::EditScriptlet => "Select a scriptlet to edit.",
            Self::RevealScriptlet => "Select a scriptlet to reveal in Finder.",
            Self::CopyScriptletPath => "Select a scriptlet to copy its path.",
            Self::CopyContent => "Select a script, agent, or scriptlet to copy its content.",
            Self::RemoveScript => "Select a script to remove.",
            Self::ResetRanking => "Select an item to reset its ranking.",
            Self::RunScriptletAction => "Select a scriptlet to run this action.",
            Self::Default => "Select an item to continue.",
        }
    }
}

fn selection_required_message_for_action(action_id: &str) -> &'static str {
    SelectionRequiredMessagePlan::from_action_id(action_id).message()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScriptRemovalTarget {
    path: std::path::PathBuf,
    name: String,
    item_kind: &'static str,
}

fn extract_scriptlet_source_path(
    file_path_with_anchor: Option<&String>,
) -> Option<std::path::PathBuf> {
    file_path_with_anchor
        .and_then(|path| path.split('#').next())
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
}

fn script_removal_target_from_result(
    result: &crate::scripts::SearchResult,
) -> Option<ScriptRemovalTarget> {
    match result {
        crate::scripts::SearchResult::Script(m) => Some(ScriptRemovalTarget {
            path: m.script.path.clone(),
            name: m.script.name.clone(),
            item_kind: "script",
        }),
        crate::scripts::SearchResult::Scriptlet(m) => {
            let path = extract_scriptlet_source_path(m.scriptlet.file_path.as_ref())?;
            Some(ScriptRemovalTarget {
                path,
                name: m.scriptlet.name.clone(),
                item_kind: "scriptlet",
            })
        }
        crate::scripts::SearchResult::Agent(m) => Some(ScriptRemovalTarget {
            path: m.agent.path.clone(),
            name: m.agent.name.clone(),
            item_kind: "agent",
        }),
        _ => None,
    }
}

fn move_path_to_trash(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let escaped_path = crate::utils::escape_applescript_string(&path.to_string_lossy());
        let script = format!(
            r#"tell application "Finder"
    delete POSIX file "{}"
end tell"#,
            escaped_path
        );

        crate::platform::run_osascript(&script, "app_actions_move_path_to_trash")
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        if path.is_dir() {
            std::fs::remove_dir_all(path)
                .map_err(|err| format!("failed to remove directory '{}': {err}", path.display()))
        } else {
            std::fs::remove_file(path)
                .map_err(|err| format!("failed to remove file '{}': {err}", path.display()))
        }
    }
}

#[cfg(test)]
mod app_actions_tests {
    use super::{
        extract_scriptlet_source_path, file_search_action_error_hud_prefix,
        file_search_action_success_hud, script_removal_target_from_result,
        select_clipboard_entry_meta, selection_required_message_for_action,
        should_transition_to_script_list_after_action, ScriptRemovalTarget,
    };
    use crate::clipboard_history::{ClipboardEntryMeta, ContentType};
    use crate::scripts;
    use crate::{AppView, FileSearchPresentation};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn entry(id: &str, preview: &str) -> ClipboardEntryMeta {
        ClipboardEntryMeta {
            id: id.to_string(),
            content_type: ContentType::Text,
            timestamp: 0,
            pinned: false,
            text_preview: preview.to_string(),
            image_width: None,
            image_height: None,
            byte_size: 0,
            ocr_text: None,
        }
    }

    #[test]
    fn test_select_clipboard_entry_meta_filters_and_clamps() {
        let entries = vec![entry("1", "Alpha"), entry("2", "Beta"), entry("3", "Gamma")];

        let filtered = select_clipboard_entry_meta(&entries, "et", 0).unwrap();
        assert_eq!(filtered.id, "2");

        let clamped = select_clipboard_entry_meta(&entries, "", 99).unwrap();
        assert_eq!(clamped.id, "3");
    }

    #[test]
    fn test_select_clipboard_entry_meta_empty_entries_returns_none() {
        let entries: Vec<ClipboardEntryMeta> = vec![];

        assert!(
            select_clipboard_entry_meta(&entries, "", 0).is_none(),
            "Empty entries with no filter should return None"
        );
        assert!(
            select_clipboard_entry_meta(&entries, "search", 0).is_none(),
            "Empty entries with filter should return None"
        );
    }

    #[test]
    fn test_select_clipboard_entry_meta_filter_no_match_returns_none() {
        let entries = vec![entry("1", "Alpha"), entry("2", "Beta")];

        assert!(
            select_clipboard_entry_meta(&entries, "zzz", 0).is_none(),
            "Filter with no matches should return None"
        );
    }

    #[test]
    fn test_select_clipboard_entry_meta_case_insensitive_filter() {
        let entries = vec![entry("1", "Hello World"), entry("2", "goodbye")];

        let result = select_clipboard_entry_meta(&entries, "HELLO", 0).unwrap();
        assert_eq!(result.id, "1", "Filter should be case-insensitive");

        let result = select_clipboard_entry_meta(&entries, "Goodbye", 0).unwrap();
        assert_eq!(result.id, "2", "Filter should be case-insensitive");
    }

    #[test]
    fn test_select_clipboard_entry_meta_zero_index_no_filter() {
        let entries = vec![entry("1", "First"), entry("2", "Second")];

        let result = select_clipboard_entry_meta(&entries, "", 0).unwrap();
        assert_eq!(
            result.id, "1",
            "Index 0 with no filter should return first entry"
        );
    }

    #[test]
    fn test_file_search_action_success_hud_messages() {
        assert_eq!(file_search_action_success_hud("open_file"), Some("Opened"));
        assert_eq!(
            file_search_action_success_hud("open_directory"),
            Some("Opened")
        );
        assert_eq!(
            file_search_action_success_hud("quick_look"),
            Some("Quick Look opened")
        );
        assert_eq!(
            file_search_action_success_hud("open_with"),
            Some("Open With opened")
        );
        assert_eq!(
            file_search_action_success_hud("show_info"),
            Some("Info opened")
        );
        assert_eq!(file_search_action_success_hud("copy_filename"), None);
    }

    #[test]
    fn test_file_search_action_error_hud_prefixes() {
        assert_eq!(
            file_search_action_error_hud_prefix("open_file"),
            Some("Failed to open")
        );
        assert_eq!(
            file_search_action_error_hud_prefix("open_directory"),
            Some("Failed to open")
        );
        assert_eq!(
            file_search_action_error_hud_prefix("quick_look"),
            Some("Failed to Quick Look")
        );
        assert_eq!(
            file_search_action_error_hud_prefix("open_with"),
            Some("Failed to Open With")
        );
        assert_eq!(
            file_search_action_error_hud_prefix("show_info"),
            Some("Failed to Show Info")
        );
        assert_eq!(file_search_action_error_hud_prefix("copy_filename"), None);
    }

    #[test]
    fn test_selection_required_message_for_action_returns_action_specific_guidance() {
        assert_eq!(
            selection_required_message_for_action("copy_path"),
            "Select an item to copy its path."
        );
        assert_eq!(
            selection_required_message_for_action("remove_shortcut"),
            "Select an item to remove its shortcut."
        );
        assert_eq!(
            selection_required_message_for_action("scriptlet_action:test"),
            "Select a scriptlet to run this action."
        );
    }

    #[test]
    fn test_selection_required_message_for_action_returns_safe_default() {
        assert_eq!(
            selection_required_message_for_action("unknown-action"),
            "Select an item to continue."
        );
    }

    // Comprehensive coverage for all selection_required_message_for_action branches

    #[test]
    fn test_selection_required_message_copy_deeplink() {
        assert_eq!(
            selection_required_message_for_action("copy_deeplink"),
            "Select an item to copy its deeplink."
        );
    }

    #[test]
    fn test_selection_required_message_shortcut_variants() {
        // All shortcut-related actions share the same message
        for action in &["configure_shortcut", "add_shortcut", "update_shortcut"] {
            assert_eq!(
                selection_required_message_for_action(action),
                "Select an item to configure its shortcut.",
                "Action '{action}' should produce shortcut configuration message"
            );
        }
    }

    #[test]
    fn test_selection_required_message_alias_variants() {
        for action in &["add_alias", "update_alias"] {
            assert_eq!(
                selection_required_message_for_action(action),
                "Select an item to add or update its alias.",
                "Action '{action}' should produce alias message"
            );
        }
        assert_eq!(
            selection_required_message_for_action("remove_alias"),
            "Select an item to remove its alias."
        );
    }

    #[test]
    fn test_selection_required_message_edit_actions() {
        assert_eq!(
            selection_required_message_for_action("edit_script"),
            "Select a script to edit."
        );
        assert_eq!(
            selection_required_message_for_action("edit_scriptlet"),
            "Select a scriptlet to edit."
        );
    }

    #[test]
    fn test_selection_required_message_scriptlet_finder_and_copy() {
        assert_eq!(
            selection_required_message_for_action("reveal_scriptlet_in_finder"),
            "Select a scriptlet to reveal in Finder."
        );
        assert_eq!(
            selection_required_message_for_action("copy_scriptlet_path"),
            "Select a scriptlet to copy its path."
        );
    }

    #[test]
    fn test_selection_required_message_copy_content() {
        assert_eq!(
            selection_required_message_for_action("copy_content"),
            "Select a script, agent, or scriptlet to copy its content."
        );
    }

    #[test]
    fn test_selection_required_message_remove_script() {
        assert_eq!(
            selection_required_message_for_action("remove_script"),
            "Select a script to remove."
        );
        assert_eq!(
            selection_required_message_for_action("delete_script"),
            "Select a script to remove."
        );
    }

    #[test]
    fn test_selection_required_message_reset_ranking() {
        assert_eq!(
            selection_required_message_for_action("reset_ranking"),
            "Select an item to reset its ranking."
        );
    }

    #[test]
    fn test_selection_required_message_scriptlet_action_prefix() {
        // Any action starting with "scriptlet_action:" should match
        assert_eq!(
            selection_required_message_for_action("scriptlet_action:run"),
            "Select a scriptlet to run this action."
        );
        assert_eq!(
            selection_required_message_for_action("scriptlet_action:foo_bar"),
            "Select a scriptlet to run this action."
        );
    }

    #[test]
    fn test_selection_required_message_default_for_empty_string() {
        assert_eq!(
            selection_required_message_for_action(""),
            "Select an item to continue."
        );
    }

    #[test]
    fn test_should_transition_to_script_list_after_action_is_context_aware() {
        assert!(should_transition_to_script_list_after_action(
            &AppView::ScriptList
        ));
        assert!(should_transition_to_script_list_after_action(
            &AppView::ActionsDialog
        ));
        assert!(!should_transition_to_script_list_after_action(
            &AppView::ClipboardHistoryView {
                filter: String::new(),
                selected_index: 0,
            }
        ));
        assert!(!should_transition_to_script_list_after_action(
            &AppView::FileSearchView {
                query: String::new(),
                selected_index: 0,
                presentation: FileSearchPresentation::Full,
            }
        ));
    }

    #[test]
    fn test_extract_scriptlet_source_path_removes_anchor() {
        let path_with_anchor = Some("/tmp/snippets/tools.md#open-github".to_string());
        let extracted = extract_scriptlet_source_path(path_with_anchor.as_ref());
        assert_eq!(extracted, Some(PathBuf::from("/tmp/snippets/tools.md")));
    }

    #[test]
    fn test_script_removal_target_from_result_for_script_and_scriptlet() {
        let script_result = scripts::SearchResult::Script(scripts::ScriptMatch {
            script: Arc::new(scripts::Script {
                name: "Deploy".to_string(),
                path: PathBuf::from("/tmp/deploy.ts"),
                ..Default::default()
            }),
            score: 0,
            filename: "deploy.ts".to_string(),
            match_indices: scripts::MatchIndices::default(),
            match_kind: scripts::ScriptMatchKind::default(),
            content_match: None,
            match_evidence: None,
        });

        let script_target = script_removal_target_from_result(&script_result);
        assert_eq!(
            script_target,
            Some(ScriptRemovalTarget {
                path: PathBuf::from("/tmp/deploy.ts"),
                name: "Deploy".to_string(),
                item_kind: "script",
            })
        );

        let scriptlet_result = scripts::SearchResult::Scriptlet(scripts::ScriptletMatch {
            scriptlet: Arc::new(scripts::Scriptlet {
                name: "Open GitHub".to_string(),
                description: Some("Open project page".to_string()),
                code: "https://github.com".to_string(),
                tool: "open".to_string(),
                shortcut: None,
                keyword: None,
                group: Some("Tools".to_string()),
                plugin_id: String::new(),
                plugin_title: None,
                file_path: Some("/tmp/snippets/tools.md#open-github".to_string()),
                command: Some("open-github".to_string()),
                alias: None,
                icon: None,
            }),
            score: 0,
            display_file_path: Some("tools.md#open-github".to_string()),
            match_indices: scripts::MatchIndices::default(),
            match_evidence: None,
        });

        let scriptlet_target = script_removal_target_from_result(&scriptlet_result);
        assert_eq!(
            scriptlet_target,
            Some(ScriptRemovalTarget {
                path: PathBuf::from("/tmp/snippets/tools.md"),
                name: "Open GitHub".to_string(),
                item_kind: "scriptlet",
            })
        );
    }

    // -----------------------------------------------------------------------
    // Scriptlet edit/copy-path/reveal — error handling and HUD feedback
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // Script removal — confirmation requirement and Toast on failure
    // -----------------------------------------------------------------------

    #[test]
    fn test_remove_script_shows_error_when_no_selection() {
        let content = crate::test_utils::read_all_handle_action_sources();

        let remove_section = content
            .find("\"remove_script\" | \"delete_script\"")
            .expect("Expected remove_script action handler");
        let block = &content[remove_section..remove_section + 400];

        assert!(
            block.contains("ScriptRemovalTargetError::NoSelection"),
            "Expected remove_script to use the named no-selection target error"
        );
    }

    // -----------------------------------------------------------------------
    // Async failure paths — coded error toasts
    // -----------------------------------------------------------------------

}
