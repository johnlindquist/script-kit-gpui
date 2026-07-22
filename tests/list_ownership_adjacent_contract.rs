//! Source-level contract for adjacent built-in visible-row ownership.

const CLIPBOARD: &str = include_str!("../src/render_builtins/clipboard.rs");
const BROWSER_TABS: &str = include_str!("../src/render_builtins/browser_tabs.rs");
const DICTATION_HISTORY: &str = include_str!("../src/render_builtins/dictation_history.rs");
const NOTES_BROWSE: &str = include_str!("../src/render_builtins/notes_browse.rs");
const AGENT_CHAT_HISTORY: &str = include_str!("../src/render_builtins/agent_chat_history.rs");
const COLLECT_ELEMENTS: &str = include_str!("../src/app_layout/collect_elements.rs");
const PROMPT_HANDLER: &str = include_str!("../src/prompt_handler/mod.rs");
const APP_VIEW_STATE: &str = include_str!("../src/main_sections/app_view_state.rs");

#[test]
fn adjacent_surfaces_define_visible_row_helper_families() {
    for (source, helpers) in [
        (
            CLIPBOARD,
            [
                "clipboard_history_visible_rows",
                "clipboard_history_selected_visible_row",
                "clipboard_history_dataset_and_visible_counts",
                "clipboard_history_visible_row_labels",
            ],
        ),
        (
            BROWSER_TABS,
            [
                "browser_tabs_visible_rows",
                "browser_tabs_selected_visible_row",
                "browser_tabs_dataset_and_visible_counts",
                "browser_tabs_visible_row_labels",
            ],
        ),
        (
            DICTATION_HISTORY,
            [
                "dictation_history_visible_rows",
                "dictation_history_selected_visible_row",
                "dictation_history_dataset_and_visible_counts",
                "dictation_history_visible_row_labels",
            ],
        ),
        (
            NOTES_BROWSE,
            [
                "notes_browse_visible_rows",
                "notes_browse_selected_visible_row",
                "notes_browse_dataset_and_visible_counts",
                "notes_browse_visible_row_labels",
            ],
        ),
        (
            AGENT_CHAT_HISTORY,
            [
                "agent_chat_history_visible_rows",
                "agent_chat_history_selected_visible_row",
                "agent_chat_history_dataset_and_visible_counts",
                "agent_chat_history_visible_row_labels",
            ],
        ),
    ] {
        for helper in helpers {
            assert!(source.contains(helper), "missing helper {helper}");
        }
    }
}

#[test]
fn adjacent_get_state_paths_use_visible_row_helpers() {
    for helper in [
        "clipboard_history_dataset_and_visible_counts",
        "clipboard_history_selected_visible_row",
        "browser_tabs_dataset_and_visible_counts",
        "browser_tabs_selected_visible_row",
        "dictation_history_dataset_and_visible_counts",
        "dictation_history_selected_visible_row",
        "notes_browse_dataset_and_visible_counts",
        "notes_browse_selected_visible_row",
        "agent_chat_history_dataset_and_visible_counts",
        "agent_chat_history_selected_visible_row",
    ] {
        assert!(
            PROMPT_HANDLER.contains(helper),
            "getState must use visible-row helper {helper}"
        );
    }
}

#[test]
fn adjacent_get_elements_paths_use_visible_row_label_helpers() {
    for helper in [
        "clipboard_history_visible_row_labels",
        "browser_tabs_visible_row_labels",
        "dictation_history_visible_row_labels",
        "notes_browse_visible_row_labels",
        "agent_chat_history_visible_row_labels",
    ] {
        assert!(
            COLLECT_ELEMENTS.contains(helper),
            "getElements must use visible-row label helper {helper}"
        );
    }
}

#[test]
fn about_surface_is_explicitly_exempted_as_static_content() {
    assert!(
        APP_VIEW_STATE.contains("ABOUT_SURFACE_EXEMPTION")
            && APP_VIEW_STATE.contains("static content surface with no list selection owner"),
        "About must carry an explicit visible-row ownership exemption"
    );
    assert!(
        APP_VIEW_STATE.contains("AppView::About { .. } => SurfaceKind::About"),
        "About must remain an explicit surface kind, not a list-like surface"
    );
}
