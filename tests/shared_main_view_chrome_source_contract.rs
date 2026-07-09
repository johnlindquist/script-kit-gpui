use std::fs;

fn read_source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn production_source(path: &str) -> String {
    read_source(path)
        .split("#[cfg(test)]")
        .next()
        .expect("production source should exist")
        .to_string()
}

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = source
        .find(start)
        .unwrap_or_else(|| panic!("start marker not found: {start}"));
    let rest = &source[start_idx..];
    let end_idx = rest
        .find(end)
        .unwrap_or_else(|| panic!("end marker not found after {start}: {end}"));
    &rest[..end_idx]
}

#[test]
fn main_window_search_surfaces_use_shared_main_view_input_shell() {
    let shared = read_source("src/components/main_view_chrome.rs");
    let script_list = production_source("src/render_script_list/mod.rs");
    let agent_chat = production_source("src/ai/agent_chat/ui/view.rs");
    let file_search = production_source("src/render_builtins/file_search.rs");
    let clipboard = production_source("src/render_builtins/clipboard.rs");
    let profile_search = production_source("src/render_builtins/profile_search.rs");
    let ui_window = read_source("src/app_impl/ui_window.rs");
    let render_impl = read_source("src/main_sections/render_impl.rs");
    let app_view_state = read_source("src/main_sections/app_view_state.rs");

    assert!(shared.contains("pub(crate) fn render_main_view_input_shell"));
    assert!(shared.contains("pub(crate) fn render_main_view_header"));
    assert!(shared.contains("pub(crate) fn render_main_view_context_header"));
    assert!(shared.contains("pub(crate) fn render_main_view_header_divider"));
    assert!(shared.contains("pub(crate) fn render_main_view_main_slot"));
    assert!(shared.contains("pub(crate) fn render_main_view_shell"));
    assert!(shared.contains("pub(crate) fn main_view_input_text_inset_left"));
    assert!(!shared.contains("render_main_view_state_icon"));
    assert!(shared.contains("pub(crate) fn render_main_view_context_zone"));
    assert!(shared.contains("pub(crate) fn render_main_view_context_zone_required"));
    assert!(shared.contains("pub(crate) fn render_main_view_context_zone_inert"));
    assert!(shared.contains("pub(crate) struct MainViewContextLabels"));
    assert!(shared.contains("MAIN_VIEW_CWD_UNAVAILABLE_LABEL"));
    assert!(shared.contains("MAIN_VIEW_AGENT_MODEL_UNAVAILABLE_LABEL"));
    assert!(shared.contains("\"Agent model unavailable\""));
    assert!(!shared.contains("\"Choose agent · model\""));
    assert!(shared.contains("pub(crate) fn render_main_view_chrome"));
    assert!(shared.contains("pub(crate) struct MainViewInputChrome"));
    assert!(shared.contains("pub(crate) struct MainViewHeaderChrome"));
    assert!(shared.contains("pub(crate) struct MainViewDividerChrome"));
    assert!(shared.contains("pub(crate) struct MainViewChrome"));
    assert!(shared.contains("pub(crate) struct MainViewColumnMetrics"));
    assert!(shared.contains("pub(crate) fn main_view_content_columns"));
    assert!(shared.contains("pub(crate) fn main_view_text_column_x"));
    assert!(
        !shared.contains("pre_main") && !shared.contains("post_main"),
        "shared main-view chrome should expose exactly one main slot so ScriptList and Agent Chat only swap the main section"
    );
    assert!(shared.contains("MAIN_VIEW_SHELL_ID"));
    assert!(shared.contains("MAIN_VIEW_HEADER_ID"));
    assert!(shared.contains("MAIN_VIEW_CONTEXT_ZONE_ID"));
    assert!(shared.contains("MAIN_VIEW_CONTEXT_CWD_BUTTON_ID"));
    assert!(shared.contains("MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID"));
    assert!(!shared.contains("MAIN_VIEW_CONTEXT_VARIATION_BADGE_ID"));
    assert!(shared.contains("MAIN_VIEW_INPUT_SHELL_ID"));
    assert!(!shared.contains("MAIN_VIEW_INPUT_STATE_ICON_ID"));
    assert!(shared.contains("MAIN_VIEW_HEADER_DIVIDER_ID"));
    assert!(shared.contains("MAIN_VIEW_MAIN_ID"));
    assert!(shared.contains(".id(MAIN_VIEW_HEADER_ID)"));
    assert!(shared.contains(".id(MAIN_VIEW_CONTEXT_ZONE_ID)"));
    assert!(shared.contains(".id(MAIN_VIEW_CONTEXT_CWD_BUTTON_ID)"));
    assert!(shared.contains(".id(MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID)"));
    assert!(shared.contains(".id(MAIN_VIEW_SHELL_ID)"));
    assert!(shared.contains(".id(MAIN_VIEW_INPUT_SHELL_ID)"));
    assert!(!shared.contains(".id(MAIN_VIEW_INPUT_STATE_ICON_ID)"));
    assert!(
        !shared.contains("leading: Option<AnyElement>"),
        "shared main-view input chrome must not expose a leading slot that can shift input geometry"
    );
    assert!(
        !shared.contains("main_view_state_icon_uses_script_kit_logo")
            && !shared.contains("main_view_should_show_state_icon"),
        "state icons belong outside the persistent MainMenuInput chrome"
    );
    assert!(shared.contains(".id(MAIN_VIEW_HEADER_DIVIDER_ID)"));
    assert!(shared.contains(".id(MAIN_VIEW_MAIN_ID)"));
    assert!(shared.contains("def.search"));
    assert!(shared.contains("search.text_inset_x"));
    assert!(shared.contains("def.icon.container_size"));
    assert!(shared.contains("def.row.icon_text_gap"));
    assert!(shared.contains("def.header_info_bar"));
    assert!(shared.contains("render_footer_hint_button_like"));
    assert!(shared.contains("FooterHintButtonSpec"));
    assert!(shared.contains(".h(px(info.height_px))"));
    assert!(!shared.contains("(def.variant.index() + 1).to_string()"));
    assert!(!shared.contains("variation_badge"));
    assert!(!shared.contains("MAIN_VIEW_CONTEXT_EDGE_OUTSET_X: f32 = 8.0"));
    assert!(shared.contains("info.pill_hover_bg_alpha"));
    assert!(shared.contains("info.pill_hover_text_alpha"));
    assert!(shared.contains("info.pill_hover_key_alpha"));
    assert!(shared.contains("info.pill_hover_border_alpha"));
    assert!(!shared.contains("def.footer.button.hover"));
    assert!(!shared.contains("theme.colors.accent.selected << 8"));
    assert!(
        !shared.contains(
            "font_weight(gpui::FontWeight::SEMIBOLD)\n                        .child(\"Tab\")"
        ),
        "header Tab text should use the footer key/button renderer instead of local styling"
    );
    assert!(shared.contains("key: \"⇥\".into()"));
    assert!(shared.contains("key: \"⇧⇥\".into()"));
    assert!(!shared.contains("key: \"Tab\".into()"));
    assert!(!shared.contains("key: \"Shift+Tab\".into()"));
    assert!(shared.contains("slot_width_px: None"));
    assert!(!shared.contains("slot_width_px: Some(280.0)"));
    assert!(!shared.contains("slot_width_px: Some(310.0)"));
    assert!(shared.contains("keycap_font_size_px: Some(header_keycap_font_size)"));
    assert!(shared.contains("hover_text_alpha: Some(info.pill_hover_text_alpha)"));
    assert!(shared.contains("hover_glyph_alpha: Some(info.pill_hover_key_alpha)"));
    assert!(shared.contains("hover_keycap_border_alpha: Some(info.pill_hover_border_alpha)"));
    assert!(shared.contains("keycap_height_px: Some(header_keycap_height)"));
    assert!(ui_window.contains("pub(crate) fn main_view_context_labels"));
    assert!(ui_window.contains("render_clickable_main_view_context_zone"));
    assert!(ui_window.contains("render_clickable_main_view_context_header"));
    assert!(ui_window.contains("render_inert_main_view_context_zone"));
    assert!(ui_window.contains("render_main_view_context_zone_required"));
    assert!(ui_window.contains("FooterAction::Cwd"));
    assert!(ui_window.contains("FooterAction::AgentModel"));
    assert!(
        !ui_window.contains("FooterButtonConfig::new(FooterAction::Cwd"),
        "cwd must be a shared-header affordance, not a duplicated native footer chip"
    );
    assert!(
        !ui_window.contains("FooterButtonConfig::new(FooterAction::AgentModel"),
        "Agent/model must be a shared-header affordance, not a duplicated native footer chip"
    );
    assert!(
        !ui_window.contains("prepend_global_main_window_left_chips")
            && !ui_window.contains("global_main_window_left_chip_buttons")
            && !ui_window.contains("current_view_shows_global_left_chips"),
        "main-window footer config must not prepend duplicated cwd/model context chips"
    );
    assert!(
        ui_window.contains("config.left_info = None;"),
        "Agent Chat footer enrichment must suppress old left-info model/cwd marker now owned by the header"
    );
    assert!(app_view_state.contains("pub(crate) fn uses_shared_main_view_header"));
    assert!(app_view_state.contains("AppView::ScriptList"));
    assert!(app_view_state.contains("| AppView::FileSearchView { .. }"));
    assert!(app_view_state.contains("| AppView::ClipboardHistoryView { .. }"));
    assert!(app_view_state.contains("| AppView::ProfileSearchView { .. }"));
    assert!(app_view_state.contains("| AppView::AgentChatView { .. }"));
    assert!(render_impl.contains(
        "let shared_header_owned_by_view = self.current_view.uses_shared_main_view_header();"
    ));
    assert!(render_impl.contains("render_clickable_main_view_context_header"));
    assert!(render_impl.contains("main_content_container"));

    assert!(script_list.contains("render_main_view_input_shell"));
    assert!(script_list.contains("render_clickable_main_view_context_zone"));
    assert!(script_list.contains("hide_initial_section_header"));
    assert!(!script_list.contains("main_view_state_icon_name_for_script_list"));
    assert!(script_list.contains("render_main_view_shell()"));
    assert!(script_list.contains("render_main_view_chrome"));
    assert!(script_list.contains("MainViewInputChrome"));
    assert!(script_list.contains("MainViewHeaderChrome"));
    assert!(script_list.contains("context: Some("));
    assert!(script_list.contains("trailing: Vec::new()"));
    assert!(!script_list.contains("leading:"));
    assert!(
        !script_list.contains("render_launcher_ask_ai_hint"),
        "ScriptList input should stay query-only; Agent belongs in the footer action zone"
    );
    assert!(script_list.contains("MainViewDividerChrome"));
    assert!(script_list.contains("visible: false"));
    assert!(script_list.contains("MainViewChrome"));
    assert!(
        script_list.contains("let header_padding_x = shell.header_padding_x;"),
        "ScriptList should source left/right main-view input padding from the active theme shell"
    );
    assert!(
        script_list.contains("margin_x: shell.divider_margin_x"),
        "ScriptList divider should align with the active theme shell inset"
    );
    assert!(agent_chat.contains("render_main_view_input_shell"));
    assert!(agent_chat.contains("render_main_view_shell()"));
    assert!(agent_chat.contains("render_main_view_chrome"));
    assert!(agent_chat.contains("MainViewInputChrome"));
    assert!(agent_chat.contains("MainViewHeaderChrome"));
    assert!(
        !agent_chat.contains("\"message-circle\""),
        "Agent Chat composer input must not inject an extra leading message icon"
    );
    assert!(
        !agent_chat.contains("leading:"),
        "Agent Chat composer input should match main-menu input positioning without a leading slot"
    );
    assert!(
        agent_chat.contains("footer_snapshot.profile_display"),
        "Agent Chat shared header must show the active profile, not a model-only label"
    );
    assert!(
        !agent_chat.contains("action_label: Some(SharedString::from(\"Attach\"))"),
        "Agent Chat composer picker rows must not show per-row Attach accessories"
    );
    assert!(
        agent_chat.contains("FooterAction::Run if button.label == \"Attach\" => \"↵ Attach\""),
        "Agent Chat footer label must derive the Attach primary action from the button spec"
    );
    assert!(agent_chat.contains("context: Some("));
    assert!(agent_chat.contains("MainViewDividerChrome"));
    assert!(agent_chat.contains("visible: false"));
    assert!(agent_chat.contains("MainViewChrome"));
    assert!(file_search.contains("render_main_view_input_shell"));
    assert!(file_search.contains("render_clickable_main_view_context_zone"));
    assert!(!file_search.contains("render_main_view_state_icon"));
    assert!(file_search.contains("render_main_view_shell()"));
    assert!(file_search.contains("render_main_view_chrome"));
    assert!(file_search.contains("MainViewInputChrome"));
    assert!(file_search.contains("MainViewHeaderChrome"));
    assert!(file_search.contains("context: Some("));
    assert!(file_search.contains("trailing: Vec::new()"));
    assert!(!file_search.contains("leading:"));
    assert!(file_search.contains("MainViewDividerChrome"));
    assert!(file_search.contains("visible: false"));
    assert!(file_search.contains("MainViewChrome"));
    assert!(
        !file_search.contains("render_expanded_view_scaffold_with_footer")
            && !file_search.contains("render_minimal_list_prompt_shell_with_footer"),
        "File Search should use the shared main-view chrome instead of feature-local prompt scaffolds"
    );
    assert!(clipboard.contains("render_main_view_input_shell"));
    assert!(clipboard.contains("render_clickable_main_view_context_zone"));
    assert!(clipboard.contains("render_main_view_shell()"));
    assert!(clipboard.contains("render_main_view_chrome"));
    assert!(clipboard.contains("MainViewInputChrome"));
    assert!(clipboard.contains("MainViewHeaderChrome"));
    assert!(clipboard.contains("context: Some("));
    assert!(clipboard.contains("trailing: Vec::new()"));
    assert!(!clipboard.contains("leading:"));
    assert!(clipboard.contains("MainViewDividerChrome"));
    assert!(clipboard.contains("visible: false"));
    assert!(
        !clipboard.contains("render_expanded_view_scaffold_with_hints")
            && !clipboard.contains("render_expanded_view_scaffold_with_footer")
            && !clipboard.contains("render_minimal_list_prompt_shell_with_footer"),
        "Clipboard History should use the shared main-view chrome instead of feature-local prompt scaffolds"
    );
    assert!(profile_search.contains("render_main_view_input_shell"));
    assert!(profile_search.contains("render_clickable_main_view_context_zone"));
    assert!(profile_search.contains("render_main_view_shell()"));
    assert!(profile_search.contains("render_main_view_chrome"));
    assert!(profile_search.contains("MainViewInputChrome"));
    assert!(profile_search.contains("MainViewHeaderChrome"));
    assert!(profile_search.contains("context: Some("));
    assert!(profile_search.contains("trailing: Vec::new()"));
    assert!(!profile_search.contains("leading:"));
    assert!(profile_search.contains("MainViewDividerChrome"));
    assert!(profile_search.contains("visible: false"));
    assert!(
        !profile_search.contains("render_expanded_view_scaffold_with_footer")
            && !profile_search.contains("render_minimal_list_prompt_shell_with_footer"),
        "Profile Search should use the shared main-view chrome instead of feature-local prompt scaffolds"
    );
}

#[test]
fn header_info_bar_reuses_footer_key_button_components() {
    let shared = read_source("src/components/main_view_chrome.rs");
    let footer = read_source("src/components/footer_chrome.rs");

    assert!(footer.contains("pub(crate) struct FooterHintButtonSpec"));
    assert!(footer.contains("pub(crate) fn render_footer_hint_button_like"));
    assert!(shared.contains("render_footer_hint_button_like"));
    assert!(shared.contains("FooterHintButtonSpec"));
    assert!(shared.contains("label: cwd_label.clone().into()"));
    assert!(shared.contains("key: \"⇥\".into()"));
    assert!(shared.contains("key: \"⇧⇥\".into()"));
    assert!(!shared.contains("key: \"Tab\".into()"));
    assert!(!shared.contains("key: \"Shift+Tab\".into()"));
    assert!(shared.contains("slot_width_px: None"));
    assert!(!shared.contains("slot_width_px: Some(280.0)"));
    assert!(!shared.contains("slot_width_px: Some(310.0)"));
    assert!(shared.contains("label_font_size_px: Some(info.font_size)"));
    assert!(shared.contains("keycap_font_size_px: Some(header_keycap_font_size)"));
    assert!(
        shared.contains(".opacity(info.key_opacity.clamp(0.0, 1.0))"),
        "header key affordance may vary opacity but must wrap the shared footer renderer"
    );
    assert!(
        !shared.contains(".text_color(key_color)") && !shared.contains(".child(\"Shift+Tab\"),"),
        "header keys must not carry bespoke color/weight/text rendering that can drift from footer key buttons"
    );
}

#[test]
fn shared_main_view_columns_own_text_column_math() {
    let shared = read_source("src/components/main_view_chrome.rs");

    assert!(
        shared.contains("def.row.outer_padding_x + def.row.inner_padding_x"),
        "row leading x should come from the same row padding used by list rows"
    );
    assert!(
        shared.contains("main_view_row_leading_x(def) + def.icon.container_size + def.row.icon_text_gap"),
        "text column math should stay available for rows without leaking icons into the input chrome"
    );
    assert!(
        shared.contains("def.search.text_inset_x"),
        "input text inset should come directly from shared search tokens"
    );
    assert!(shared.contains("text_column_x"));
    assert!(
        shared.contains("content_right_inset_x: def.shell.header_padding_x"),
        "main-view info/content surfaces should share the shell right inset"
    );
    assert!(
        shared.contains("top_inset_y: def.list.first_section_header_height"),
        "main-view info/content surfaces should start on the same vertical rhythm as first list content"
    );
    assert!(
        !shared.contains("main_view_state_icon_left"),
        "MainMenuInput must not carry a leading state icon positioner"
    );
    assert!(
        shared.contains(".pl(px(text_inset_left))"),
        "input text should use the shared tokenized inset"
    );
}

#[test]
fn main_view_input_chrome_rejects_left_state_icons() {
    let script_list = read_source("src/render_script_list/mod.rs");
    let file_search = read_source("src/render_builtins/file_search.rs");

    assert!(!script_list.contains("render_main_view_state_icon"));
    assert!(!script_list.contains("main_view_state_icon_name_for_script_list"));
    assert!(!script_list.contains("main_view_should_show_state_icon"));
    assert!(!file_search.contains("render_main_view_state_icon"));
    assert!(!file_search.contains("leading:"));
}

#[test]
fn script_list_no_longer_owns_local_main_view_chrome() {
    let script_list = read_source("src/render_script_list/mod.rs");

    assert!(
        !script_list.contains(".id(\"main-menu-search-shell\")"),
        "ScriptList must use the shared main-view input shell instead of local search chrome"
    );
    assert!(
        !script_list.contains(".id(\"main-menu-shell\")"),
        "ScriptList must use the shared main-view root shell instead of local feature shell chrome"
    );
    assert!(
        !script_list.contains(".id(\"main-menu-header\")"),
        "ScriptList must use the shared main-view header instead of local header chrome"
    );
    assert!(
        !script_list.contains(".id(\"main-menu-header-divider\")"),
        "ScriptList must use the shared main-view divider instead of local divider chrome"
    );
    assert!(
        !script_list.contains(".id(\"main-menu-content\")"),
        "ScriptList must use the shared main-view main slot instead of local content chrome"
    );
}

#[test]
fn agent_chat_composer_shell_consumes_main_menu_header_geometry() {
    let agent_chat = read_source("src/ai/agent_chat/ui/view.rs");
    let ui_variant = read_source("src/ai/agent_chat/ui/ui_variant.rs");

    assert!(agent_chat.contains("crate::designs::current_main_menu_theme().def()"));
    assert!(agent_chat.contains("fn render_composer_input_shell"));
    assert!(agent_chat.contains("render_main_view_input_shell"));
    assert!(agent_chat.contains("render_main_view_header"));
    assert!(!agent_chat.contains("render_main_view_header_divider("));
    assert!(agent_chat.contains("render_main_view_context_zone_inert"));
    assert!(agent_chat.contains("MainViewContextLabels::new"));
    assert!(agent_chat.contains("pub(crate) fn agent_model_header_label(&self) -> String"));
    let agent_chat_header_labels = source_between(
        &agent_chat,
        "let context_labels = crate::components::main_view_chrome::MainViewContextLabels::new(",
        "let header = crate::components::main_view_chrome::MainViewHeaderChrome",
    );
    assert!(
        agent_chat_header_labels.contains("footer_snapshot.agent_model_header_label()"),
        "Agent Chat shared header must use the same Agent · Model label shape as the main menu header"
    );
    assert!(
        !agent_chat_header_labels.contains("footer_snapshot.profile_display"),
        "Agent Chat shared header must not pass a profile-only label into the model chip"
    );
    assert!(agent_chat.contains("snapshot.profile_display.clone()"));
    assert!(agent_chat.contains("snapshot.model_display.clone()"));
    assert!(agent_chat.contains(".id(\"agent_chat-profile-display\")"));
    assert!(agent_chat.contains(".id(\"agent_chat-model-display\")"));
    assert!(agent_chat.contains("render_main_view_main_slot"));
    assert!(agent_chat.contains("render_main_view_chrome"));
    assert!(!agent_chat.contains("\"agent-chat-input-profile-icon\""));
    assert!(!agent_chat.contains("trailing: vec![profile_icon]"));
    assert!(agent_chat.contains("trailing: Vec::new()"));
    assert!(agent_chat.contains("padding_x: menu_def.shell.header_padding_x"));
    assert!(agent_chat.contains("margin_x: menu_def.shell.divider_margin_x"));
    assert!(agent_chat.contains("visible: false"));
    assert!(agent_chat.contains("padding_y: menu_def.shell.header_padding_y"));
    assert!(
        !agent_chat.contains(".id(\"agent-chat-shell\")"),
        "standard Agent Chat must use the shared main-view root shell instead of local feature shell chrome"
    );
    assert!(
        agent_chat.contains(".id(\"agent_chat-conversation\")"),
        "standard Agent Chat should wrap all conversation chrome inside one swapped main child"
    );
    assert!(
        ui_variant.contains("Self::Standard => AgentChatUiConfig {\n                transcript: AgentChatTranscriptPresentation::Standard,\n                composer: AgentChatComposerPlacement::Default"),
        "standard Agent Chat must stay on the default composer path that returns shared main-view chrome"
    );
}

#[test]
fn layout_model_exposes_shared_main_view_chrome_names() {
    let layout = read_source("src/app_layout/build_layout_info.rs");

    for name in [
        "MainViewHeader",
        "MainViewContextZone",
        "MainViewInput",
        "MainViewMain",
        "MainViewFooter",
    ] {
        assert!(layout.contains(name), "layout model missing {name}");
    }

    assert!(layout.contains(".with_parent(\"MainViewHeader\")"));
    assert!(layout.contains(".with_parent(\"MainViewMain\")"));
    assert!(
        !layout.contains("LayoutComponentInfo::new(\"SearchInput\""),
        "layout model should report the shared MainViewInput chrome name"
    );
    for stale_name in ["LogoButton", "ActionsButton", "RunButton"] {
        assert!(
            !layout.contains(&format!("LayoutComponentInfo::new(\"{stale_name}\"")),
            "layout model should not report stale header button chrome after MainViewInput became the shared full-width header"
        );
    }
    assert!(
        layout.contains(
            "let input_width = (window_width - (shell_horizontal_padding * 2.0)).max(0.0);"
        ),
        "layout model should derive MainViewInput width from shared header padding, not stale button edges"
    );
    assert!(
        layout.contains("main_view_input_text_inset_left("),
        "layout model should report the shared input text inset used by the render layer"
    );
    assert!(
        !layout.contains("MainViewInputStateIcon"),
        "layout model should not report a left state icon inside MainMenuInput"
    );
}

#[test]
fn file_search_layout_model_uses_main_view_context_chrome() {
    let layout = read_source("src/app_layout/build_layout_info.rs");
    let bounds = read_source("src/app_layout/build_component_bounds.rs");

    assert!(layout.contains("| AppView::FileSearchView { .. }"));
    assert!(layout.contains("| AppView::ClipboardHistoryView { .. }"));
    assert!(layout.contains("| AppView::ProfileSearchView { .. }"));
    assert!(layout.contains("| AppView::AgentChatView { .. }"));
    assert!(layout
        .contains("| AppView::AgentChatView { .. } => crate::window_resize::ViewType::MainWindow"));
    assert!(bounds.contains("| AppView::FileSearchView { .. }"));
    assert!(bounds.contains("| AppView::ClipboardHistoryView { .. }"));
    assert!(bounds.contains("| AppView::ProfileSearchView { .. }"));
    assert!(bounds.contains("| AppView::AgentChatView { .. }"));
}

#[test]
fn main_window_footer_keeps_header_context_out_of_action_zone() {
    let ui_window = read_source("src/app_impl/ui_window.rs");
    let standard_footer = ui_window
        .split("fn standard_main_window_footer_buttons")
        .nth(1)
        .and_then(|tail| tail.split("fn main_window_footer_buttons_blocked").next())
        .expect("standard_main_window_footer_buttons body should be present");
    assert!(standard_footer.contains("FooterAction::Run"));
    assert!(standard_footer.contains("FooterAction::Actions"));
    assert!(standard_footer.contains("FooterAction::Ai"));
    assert!(standard_footer.contains("matches!(self.current_view, AppView::ScriptList)"));
    assert!(
        !standard_footer.contains("FooterAction::Cwd")
            && !standard_footer.contains("FooterAction::AgentModel"),
        "cwd/model context should render in MainViewContextZone, not as footer action buttons"
    );

    let footer_config = ui_window
        .split("pub(crate) fn main_window_footer_config_with_cx")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn main_window_uses_native_footer")
                .next()
        })
        .expect("main_window_footer_config_with_cx body should be present");
    assert!(
        !footer_config.contains("prepend_global_main_window_left_chips")
            && !footer_config.contains("FooterAction::Cwd")
            && !footer_config.contains("FooterAction::AgentModel"),
        "main-window footer config should not inject duplicated cwd/model footer chips"
    );
}

#[test]
fn agent_chat_component_bounds_model_uses_main_view_chrome() {
    let bounds = read_source("src/app_layout/build_component_bounds.rs");

    assert!(bounds.contains("\"MainViewHeader\""));
    assert!(bounds.contains("\"MainViewContextZone\""));
    assert!(bounds.contains("AppView::AgentChatView { .. } =>"));
    assert!(bounds.contains("\"MainViewMain\""));
    assert!(bounds.contains("\"AgentChatConversation\""));
    assert!(bounds.contains("\"MainViewInput\""));
    assert!(!bounds.contains("\"MainViewInputStateIcon\""));
    assert!(bounds.contains("\"MainViewFooter\""));
    assert!(bounds.contains("\"AgentChatEmptyGuidance\""));
    assert!(bounds.contains("\"AgentChatEmptyGuidanceTitle\""));
    assert!(bounds.contains("\"AgentChatEmptyGuidanceShortcutSlot\""));
    assert!(bounds.contains("\"AgentChatEmptyGuidanceLabelColumn\""));
    assert!(bounds.contains("main_view_content_columns(menu_def)"));
    assert!(
        bounds.contains("AppView::ScriptList")
            && bounds.contains("| AppView::FileSearchView { .. }")
            && bounds.contains("| AppView::ClipboardHistoryView { .. }")
            && bounds.contains("| AppView::ProfileSearchView { .. }")
            && bounds.contains("| AppView::AgentChatView { .. }"),
        "debug component bounds should emit shared input details for ScriptList, FileSearch, Clipboard History, ProfileSearch, and AgentChat"
    );
}

#[test]
fn standard_agent_chat_mock_fixture_bypasses_provider_warmup() {
    let agent_chat_launch = read_source("src/app_impl/agent_handoff/agent_chat_launch.rs");
    let runtime_stdin = read_source("src/main_entry/runtime_stdin.rs");
    let app_run_setup = read_source("src/main_entry/app_run_setup.rs");
    let runtime_tail = read_source("src/main_entry/runtime_stdin_match_tail.rs");

    assert!(agent_chat_launch.contains("fn open_standard_agent_chat_mock_fixture"));
    assert!(agent_chat_launch.contains("StandardAgentChatMockFixtureConnection"));
    assert!(agent_chat_launch.contains("AgentChatUiVariant::Standard"));
    assert!(
        agent_chat_launch.contains("self.enter_embedded_agent_chat_surface(view_entity, cx);"),
        "fixture should use the same embedded Agent Chat surface transition as real Agent Chat"
    );

    for (path, source) in [
        ("src/main_entry/runtime_stdin.rs", runtime_stdin),
        ("src/main_entry/app_run_setup.rs", app_run_setup),
        ("src/main_entry/runtime_stdin_match_tail.rs", runtime_tail),
    ] {
        assert!(
            source.contains("view.open_standard_agent_chat_mock_fixture(ctx);"),
            "{path} should open the deterministic standard Agent Chat fixture for openAiWithMockData"
        );
        assert!(
            !source.contains("Ignoring deprecated mock-data AI alias and opening Agent Chat"),
            "{path} should not route mock-data proof through provider warm-up"
        );
    }
}
