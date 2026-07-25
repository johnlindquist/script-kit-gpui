use std::collections::BTreeSet;

use super::read_source;

fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing function signature `{signature}`"));
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing opening brace for `{signature}`"));
    let mut depth = 0usize;
    for (offset, byte) in source.as_bytes()[open..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open..=open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("missing closing brace for `{signature}`");
}

fn app_view_names(line: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = line;
    while let Some(offset) = rest.find("AppView::") {
        let after = &rest[offset + "AppView::".len()..];
        let name = after
            .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .next()
            .unwrap_or("");
        if !name.is_empty() {
            names.push(name.to_string());
        }
        rest = &after[name.len()..];
    }
    names
}

fn variants_for_policy(function: &str, policy: &str) -> BTreeSet<String> {
    let marker = format!("MainViewHeaderInputPolicy::{policy}");
    let mut pending = BTreeSet::new();
    let mut classified = BTreeSet::new();
    for line in function.lines() {
        pending.extend(app_view_names(line));
        if line.contains("MainViewHeaderInputPolicy::") {
            if line.contains(&marker) {
                classified.append(&mut pending);
            } else {
                pending.clear();
            }
        }
    }
    classified
}

fn canonical_input_renderers() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        (
            "ScriptList",
            "src/render_script_list/mod.rs",
            "render_main_view_input_shell(",
        ),
        (
            "ClipboardHistoryView",
            "src/render_builtins/clipboard.rs",
            "render_main_view_input_shell(",
        ),
        (
            "AppLauncherView",
            "src/render_builtins/app_launcher.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "WindowSwitcherView",
            "src/render_builtins/window_switcher.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "BrowserTabsView",
            "src/render_builtins/browser_tabs.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "FileSearchView",
            "src/render_builtins/file_search.rs",
            "render_main_view_input_shell(",
        ),
        (
            "ProfileSearchView",
            "src/render_builtins/profile_search.rs",
            "render_main_view_input_shell(",
        ),
        (
            "ThemeChooserView",
            "src/render_builtins/theme_chooser.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "EmojiPickerView",
            "src/render_builtins/emoji_picker.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "SdkReferenceView",
            "src/render_builtins/sdk_reference.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "TipsView",
            "src/render_builtins/tips.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "ScriptTemplateCatalogView",
            "src/render_builtins/script_templates.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "BrowseKitsView",
            "src/render_builtins/kit_store.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "MigrateV1View",
            "src/render_builtins/migrate_v1.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "InstalledKitsView",
            "src/render_builtins/kit_store.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "ProcessManagerView",
            "src/render_builtins/process_manager.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "FlowUxView",
            "src/render_builtins/flow_ux.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "FlowSessionView",
            "src/render_builtins/flow_ux.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "SearchAiPresetsView",
            "src/render_builtins/ai_presets.rs",
            "render_generic_filterable_search_surface(",
        ),
        (
            "SettingsView",
            "src/render_builtins/settings.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "FavoritesBrowseView",
            "src/render_builtins/favorites.rs",
            "render_generic_filterable_search_surface(",
        ),
        (
            "CurrentAppCommandsView",
            "src/render_builtins/current_app_commands.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "AgentChatHistoryView",
            "src/render_builtins/agent_chat_history.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "BrowserHistoryView",
            "src/render_builtins/browser_history.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "DictationHistoryView",
            "src/render_builtins/dictation_history.rs",
            "render_builtin_main_input_header(",
        ),
        (
            "NotesBrowseView",
            "src/render_builtins/notes_browse.rs",
            "render_builtin_main_input_header(",
        ),
    ]
}

fn app_view_variants() -> BTreeSet<String> {
    let source = read_source("src/main_sections/app_view_state.rs");
    let enum_start = source
        .find("enum AppView {")
        .expect("AppView enum should exist");
    let enum_body = source[enum_start..]
        .split("/// Exhaustive host-level header/input ownership")
        .next()
        .expect("AppView enum should precede the header/input policy docs");

    enum_body
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if !line.starts_with("    ")
                || trimmed.starts_with("//")
                || trimmed.starts_with("#[")
                || trimmed.starts_with("///")
                || trimmed.is_empty()
            {
                return None;
            }

            let name = trimmed
                .split(|ch: char| ch == '{' || ch == ',' || ch.is_whitespace())
                .next()
                .unwrap_or("");
            if name.chars().next().is_some_and(char::is_uppercase) {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn every_app_view_is_classified_for_main_window_header_input_policy() {
    let actual = app_view_variants();
    let source = read_source("src/main_sections/app_view_state.rs");
    let policy = function_body(
        &source,
        "pub(crate) fn main_view_header_input_policy(&self)",
    );

    for variant in actual {
        assert!(
            policy.contains(&format!("AppView::{variant}")),
            "new AppView variant {variant} must be classified by main_view_header_input_policy"
        );
    }
    assert!(
        !policy.contains("_ =>"),
        "main_view_header_input_policy must stay exhaustive so rustc catches new views"
    );
}

#[test]
fn searchable_main_window_surfaces_route_through_shared_input_chrome() {
    for (surface, path, required) in canonical_input_renderers() {
        let source = read_source(path);
        // Matched by MODULE, not by an enumerated list of entry points. The
        // list used to name four exact helpers; ScriptList moved to
        // `render_main_view_chrome_header_overlay_footer_flush(`, a fifth
        // variant in the very same shared module, and this went red for a
        // surface that routes through shared chrome more thoroughly than most.
        // Enumerating spellings means the audit fails every time the shared
        // component grows a variant — the opposite of encouraging its use.
        assert!(
            source.contains("main_view_chrome::render_")
                || source.contains("render_builtin_main_input_surface(")
                || source.contains("render_generic_filterable_search_surface("),
            "{surface} must route its outer shell through shared main-view chrome"
        );
        assert!(
            source.contains(required),
            "{surface} must use the shared main-window input/header primitive `{required}`"
        );
    }
}

#[test]
fn searchable_main_window_surfaces_have_shared_runtime_layout_context_zone() {
    let layout_info = read_source("src/app_layout/build_layout_info.rs");
    let component_bounds = read_source("src/app_layout/build_component_bounds.rs");

    for (name, model) in [
        ("layout info", layout_info),
        ("component bounds", component_bounds),
    ] {
        assert!(model.contains("resolved_main_view_header_input_policy"));
        assert!(model.contains("main_view_header_metrics(menu_def, input_height)"));
        for component in [
            "MainViewHeader",
            "MainViewContextZone",
            "MainViewInput",
            "MainViewMain",
        ] {
            assert!(model.contains(component), "{name} must emit {component}");
        }
        assert!(
            !model.contains("let main_view_has_context_zone = matches!"),
            "{name} must not duplicate the exhaustive AppView policy"
        );
    }
}

#[test]
fn searchable_shared_header_inventory_is_owned_by_the_exhaustive_app_view_policy() {
    let app_view_state = read_source("src/main_sections/app_view_state.rs");
    let policy = function_body(
        &app_view_state,
        "pub(crate) fn main_view_header_input_policy(&self)",
    );
    let policy_inventory = variants_for_policy(policy, "ViewOwnedCanonicalInput");
    let renderer_inventory = canonical_input_renderers()
        .iter()
        .map(|(variant, _, _)| (*variant).to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        policy_inventory, renderer_inventory,
        "canonical-input policy membership and shared renderer inventory must match exactly"
    );
    assert_eq!(
        variants_for_policy(policy, "ViewOwnedCanonicalMultilineInput"),
        BTreeSet::from(["AgentChatView".to_string()]),
        "standard Agent Chat must remain the sole canonical multiline owner"
    );
    assert_eq!(
        variants_for_policy(policy, "ViewOwnedContextOnly"),
        BTreeSet::from(["DayPage".to_string()]),
        "Day Page must remain the sole view-owned context-only owner"
    );

    let resolved = function_body(
        &app_view_state,
        "pub(crate) fn resolved_main_view_header_input_policy(",
    );
    assert_eq!(
        variants_for_policy(resolved, "ViewOwnedIntentionalCompact"),
        BTreeSet::from(["AgentChatView".to_string()]),
        "Focused Text Mini must remain the sole intentional-compact resolution"
    );
    assert!(
        resolved.contains("entity.read(cx).is_focused_text_mini()"),
        "the intentional-compact Agent Chat resolution must stay scoped to Focused Text Mini"
    );
}

#[test]
fn prompt_and_child_content_surfaces_use_root_shared_context_header_fallback() {
    let render_impl = read_source("src/main_sections/render_impl.rs");
    let app_view_state = read_source("src/main_sections/app_view_state.rs");
    let ownership_body = function_body(
        &app_view_state,
        "pub(crate) fn main_view_header_input_policy(&self)",
    );

    assert!(
        render_impl.contains(".uses_view_owned_main_window_shell(&*cx)"),
        "root renderer must ask the AppView ownership policy before wrapping content"
    );
    assert!(
        render_impl.contains("render_clickable_main_view_context_header("),
        "root renderer must provide the shared context header fallback for prompt/script child surfaces"
    );

    for variant in [
        "AppView::ArgPrompt { .. }",
        "AppView::DivPrompt { .. }",
        "AppView::FormPrompt { .. }",
        "AppView::TermPrompt { .. }",
        "AppView::EditorPrompt { .. }",
        "AppView::SelectPrompt { .. }",
        "AppView::PathPrompt { .. }",
        "AppView::EnvPrompt { .. }",
        "AppView::DropPrompt { .. }",
        "AppView::TemplatePrompt { .. }",
        "AppView::HotkeyPrompt { .. }",
        "AppView::ChatPrompt { .. }",
        "AppView::MiniPrompt { .. }",
        "AppView::MicroPrompt { .. }",
        "AppView::NamingPrompt { .. }",
        "AppView::CreateAiPresetView { .. }",
        "AppView::QuickTerminalView { .. }",
        "AppView::ScratchPadView { .. }",
    ] {
        assert!(
            ownership_body.contains(variant),
            "{variant} must be explicitly classified by the root shared context-header fallback"
        );
    }
    assert!(ownership_body.contains("MainViewHeaderInputPolicy::RootContextOnly"));
}

#[test]
fn split_preview_builtins_do_not_use_legacy_expanded_header_scaffolds() {
    for path in [
        "src/render_builtins/agent_chat_history.rs",
        "src/render_builtins/browser_history.rs",
        "src/render_builtins/dictation_history.rs",
        "src/render_builtins/notes_browse.rs",
        "src/render_builtins/sdk_reference.rs",
        "src/render_builtins/script_templates.rs",
    ] {
        let source = read_source(path);
        assert!(
            source.contains("render_builtin_split_main_content("),
            "{path} should share split main-slot content geometry"
        );
        assert!(
            !source.contains("render_expanded_view_scaffold_with_footer(")
                && !source.contains("render_expanded_view_scaffold_with_hints("),
            "{path} must not rebuild local header padding through expanded-view scaffolds"
        );
    }
}
