use super::fixture_ids::{main_fixture_ids, MAIN_OVERLAY_FIXTURE_IDS, SHORTCUT_FIXTURE_IDS};
use crate::*;
use anyhow::Context as _;

pub(crate) fn mount_shortcut_fixture(
    fixture_id: &str,
    app: Entity<ScriptListApp>,
    parent: &crate::protocol::AutomationWindowInfo,
    parent_handle: gpui::AnyWindowHandle,
    cx: &mut App,
) -> anyhow::Result<crate::protocol::AutomationWindowInfo> {
    anyhow::ensure!(
        SHORTCUT_FIXTURE_IDS.contains(&fixture_id),
        "unknown_shortcut_fixture"
    );
    crate::shortcut_recorder::mount_owned_shortcut_recorder(app, parent, parent_handle, cx)
}

pub(crate) fn close_shortcut_fixture(
    id: &str,
    generation: u64,
    cx: &mut App,
) -> anyhow::Result<()> {
    anyhow::ensure!(id == "shortcut-recorder-popup", "not_shortcut_recorder");
    crate::shortcut_recorder::close_shortcut_recorder_instance(generation, cx)
}

/// Data injection into the real ScriptListApp route; never normal startup followed by clearing.
pub(crate) fn mount_main_fixture(
    app: &mut ScriptListApp,
    id: &str,
    window: &mut Window,
    cx: &mut Context<ScriptListApp>,
) -> anyhow::Result<()> {
    if MAIN_OVERLAY_FIXTURE_IDS.contains(&id) {
        return mount_main_overlay_fixture(app, id, window, cx);
    }
    anyhow::ensure!(
        main_fixture_ids().contains(&id),
        "unknown main fixture: {id}"
    );
    anyhow::ensure!(
        matches!(app.main_services, MainServices::OwnedFixtures(_)),
        "main_fixture_requires_owned_services"
    );
    if let MainServices::OwnedFixtures(sources) = &mut app.main_services {
        Arc::make_mut(sources).root_file_provider_files = None;
    }
    let scope =
        crate::runtime_policy::owned_evaluation().context("main_fixture_requires_owned_policy")?;
    let root = scope.root();
    scope.require_owned_path(&crate::setup::get_kit_path())?;
    scope.require_owned_path(&dirs::home_dir().context("owned HOME absent")?)?;
    crate::design_evaluation::conversation_fixtures::seed_owned_flow_catalogue()?;
    app.prompt_completion = None;
    app.opened_from_main_menu = true;
    // Mini is entered by the production ~/ trigger. An empty input correctly
    // exits that presentation on the next InputState change notification.
    let initial_filter = if id == "main.file-search-mini" {
        "~/"
    } else {
        ""
    };
    app.filter_text = initial_filter.into();
    app.computed_filter_text = initial_filter.into();
    app.selected_index = 0;
    app.reset_main_menu_selection_intent();
    app.main_window_mode = MainWindowMode::Full;
    app.pending_focus = Some(FocusTarget::MainFilter);
    app.focused_input = FocusedInput::MainFilter;
    app.pending_filter_sync = false;
    app.gpui_input_state
        .update(cx, |input, cx| input.set_value(initial_filter, window, cx));
    let view = match id {
        "main.script-list" | "main.root-search-frame-stability" | "main-search-contract" => {
            if id == "main.root-search-frame-stability" {
                let MainServices::OwnedFixtures(sources) = &mut app.main_services else {
                    unreachable!("owned services checked above");
                };
                let sources = Arc::make_mut(sources);
                // This record must arrive only through the delayed root provider,
                // not through the immediately available recent-file corpus.
                sources.root_file_provider_files = Some(vec![crate::file_search::FileResult {
                    path: root
                        .join("zzqxframeproof-delayed.md")
                        .to_string_lossy()
                        .into_owned(),
                    name: "zzqxframeproof-delayed.md".into(),
                    size: 128,
                    modified: 1_777_593_600,
                    file_type: crate::file_search::FileType::Document,
                }]);
            }
            prepare_launcher_files(app, root)?;
            AppView::ScriptList
        }
        "main.menu-syntax-trigger" | "main.menu-syntax-object" | "main.menu-syntax-history" => {
            if id == "main.menu-syntax-object" {
                crate::design_evaluation::notes_fixtures::prepare_notes_storage()?;
            }
            app.input_history.add_entry(":type:script launch");
            app.input_history.add_entry(":has:menuSyntax");
            AppView::ScriptList
        }
        "main.about" => AppView::About {
            previous: Box::new(AppView::ScriptList),
            state: crate::about::AboutState::new(),
            update_state: Arc::new(std::sync::RwLock::new(crate::updates::UpdateState::Idle)),
        },
        "main.clipboard-history" => {
            scope.require_owned_path(&root.join("clipboard"))?;
            for text in [
                "Launch checklist: review, build, release",
                "Omega release notes",
                "https://example.invalid/launch",
            ] {
                let entry_id = crate::clipboard_history::add_entry(
                    text,
                    crate::clipboard_history::ContentType::Text,
                )?;
                anyhow::ensure!(
                    crate::clipboard_history::get_entry_content(&entry_id).as_deref() == Some(text),
                    "clipboard_fixture_content_unavailable"
                );
                crate::runtime_policy::record_completed_fixture_effect();
            }
            app.cached_clipboard_entries =
                crate::clipboard_history::get_clipboard_history_meta(32, 0)?;
            AppView::ClipboardHistoryView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.app-launcher" => AppView::AppLauncherView {
            filter: String::new(),
            selected_index: 0,
        },
        "main.window-switcher" => {
            app.install_root_windows(fixture_windows(root), cx);
            AppView::WindowSwitcherView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.browser-tabs" => {
            app.cached_browser_tabs = fixture_browser_tabs();
            AppView::BrowserTabsView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.file-search-mini" | "main.file-search-full" => {
            prepare_launcher_files(app, root)?;
            app.cached_file_results = app
                .main_services
                .owned_sources()
                .context("missing owned sources")?
                .files
                .clone();
            app.file_search_loading = false;
            app.file_search_selection_mode = FileSearchSelectionMode::AutoFirst;
            AppView::FileSearchView {
                query: initial_filter.into(),
                selected_index: 0,
                presentation: if id == "main.file-search-mini" {
                    FileSearchPresentation::Mini
                } else {
                    FileSearchPresentation::Full
                },
            }
        }
        "main.profile-search" => AppView::ProfileSearchView {
            filter: String::new(),
            selected_index: 0,
        },
        "main.theme-chooser" => {
            let snapshot = crate::theme::get_theme_snapshot();
            app.theme_before_chooser = Some(snapshot.theme.clone());
            app.theme_chooser_preview_revision = Some(snapshot.revision);
            AppView::ThemeChooserView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.emoji-picker" => AppView::EmojiPickerView {
            filter: String::new(),
            selected_index: 0,
            selected_category: None,
        },
        "main.script-issues" => {
            let candidates = ["Conflicting Alpha", "Conflicting Beta"]
                .into_iter()
                .map(|name| {
                    Arc::new(crate::scripts::Script {
                        name: name.into(),
                        path: root.join(format!("{name}.ts")),
                        alias: Some("duplicate".into()),
                        extension: "ts".into(),
                        plugin_id: "owned".into(),
                        ..Default::default()
                    })
                })
                .collect();
            AppView::ScriptIssuesView {
                report: crate::scripts::validate_script_catalog(candidates).validation,
            }
        }
        "main.sdk-reference" => AppView::SdkReferenceView {
            filter: String::new(),
            selected_index: 0,
            entries: crate::mcp_resources::sdk_reference_entries_for_ui(),
        },
        "main.tips" => AppView::TipsView {
            filter: String::new(),
            selected_index: 0,
            entries: serde_json::from_str(crate::setup::EMBEDDED_TIPS)?,
        },
        "main.script-template-catalog" => AppView::ScriptTemplateCatalogView {
            filter: String::new(),
            selected_index: 0,
            templates: crate::mcp_resources::script_template_entries_for_ui(),
        },
        "main.browse-kits" | "main.browse-kits-loading" | "main.browse-kits-failed" => {
            let kits = vec![KitStoreSearchResult {
                name: "release-tools".into(),
                full_name: "owned/release-tools".into(),
                description: "Release checklists and local text tools".into(),
                stars: 42,
                html_url: "https://example.invalid/owned/release-tools".into(),
                clone_url: "https://example.invalid/owned/release-tools.git".into(),
            }];
            app.kit_store_browse_state = match id {
                "main.browse-kits-loading" => KitStoreBrowseState::Loading,
                "main.browse-kits-failed" => {
                    KitStoreBrowseState::Failed("Owned catalogue source unavailable".to_string())
                }
                _ => KitStoreBrowseState::Ready,
            };
            if let MainServices::OwnedFixtures(sources) = &mut app.main_services {
                let sources = Arc::make_mut(sources);
                sources.kits = kits.clone();
                sources.kit_error = match &app.kit_store_browse_state {
                    KitStoreBrowseState::Failed(error) => Some(error.clone()),
                    _ => None,
                };
            }
            AppView::BrowseKitsView {
                query: String::new(),
                selected_index: 0,
                results: if id == "main.browse-kits" {
                    kits
                } else {
                    Vec::new()
                },
            }
        }
        "main.migrate-v1"
        | "main.migrate-v1-scanning"
        | "main.migrate-v1-porting"
        | "main.migrate-v1-done"
        | "main.migrate-v1-unavailable" => {
            let phase = match id {
                "main.migrate-v1" => MigrateBoardPhase::Report,
                "main.migrate-v1-scanning" => MigrateBoardPhase::Scanning,
                "main.migrate-v1-porting" => MigrateBoardPhase::Porting,
                "main.migrate-v1-done" => MigrateBoardPhase::Done,
                "main.migrate-v1-unavailable" => MigrateBoardPhase::Unavailable(
                    "No legacy installation in owned workspace".into(),
                ),
                _ => unreachable!(),
            };
            AppView::MigrateV1View {
                filter: String::new(),
                selected_index: 0,
                board: MigrateBoardState {
                    phase,
                    v1_dir: root.join("legacy").to_string_lossy().into_owned(),
                    rows: vec![MigrateScriptRow {
                        file: "launch.ts".into(),
                        path: root.join("legacy/launch.ts").to_string_lossy().into_owned(),
                        bucket: "compatible".into(),
                        phase: "review".into(),
                        note_summary: Some("Uses the supported arg prompt".into()),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            }
        }
        "main.installed-kits" => AppView::InstalledKitsView {
            filter: String::new(),
            selected_index: 0,
            kits: vec![script_kit_gpui::kit_store::InstalledKit {
                name: "owned-release-tools".into(),
                path: root.join("plugins/owned-release-tools"),
                repo_url: "https://example.invalid/owned/release-tools".into(),
                git_hash: "0000000000000000000000000000000000000000".into(),
                installed_at: "2026-08-28T12:00:00Z".into(),
            }],
        },
        "main.process-manager" => {
            app.cached_processes = vec![crate::process_manager::ProcessInfo {
                pid: 2_000_000_001,
                script_path: root
                    .join("scripts/launch-alpha.ts")
                    .to_string_lossy()
                    .into_owned(),
                started_at: chrono::DateTime::parse_from_rfc3339("2026-08-28T12:00:00Z")?
                    .with_timezone(&chrono::Utc),
            }];
            AppView::ProcessManagerView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.search-ai-presets" => {
            crate::ai::presets::save_presets(&[crate::ai::presets::SavedAiPreset {
                id: "owned-release-review".into(),
                name: "Release Review".into(),
                description: "Review a synthetic release".into(),
                system_prompt: "Review the supplied synthetic checklist.".into(),
                icon: "check".into(),
                preferred_model: None,
            }])?;
            AppView::SearchAiPresetsView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.settings" => {
            if let MainServices::OwnedFixtures(sources) = &mut app.main_services {
                let sources = Arc::make_mut(sources);
                sources.has_custom_positions = true;
                sources.configure_snap_mode_available = false;
            }
            AppView::SettingsView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.permissions-wizard" => {
            if let MainServices::OwnedFixtures(sources) = &mut app.main_services {
                Arc::make_mut(sources).permissions =
                    crate::permissions_wizard::PermissionKind::all()
                        .iter()
                        .enumerate()
                        .map(|(index, &kind)| {
                            (
                                kind,
                                if index == 0 {
                                    crate::platform::permiso_detect::PermissionStatus::Authorized
                                } else {
                                    crate::platform::permiso_detect::PermissionStatus::Denied
                                },
                            )
                        })
                        .collect();
            }
            AppView::PermissionsWizardView { selected_index: 0 }
        }
        "main.favorites-browse" => {
            let script_ids = app
                .scripts
                .iter()
                .take(3)
                .map(|script| script.path.to_string_lossy().into_owned())
                .collect();
            script_kit_gpui::favorites::save_favorites(&script_kit_gpui::favorites::Favorites {
                script_ids,
            })?;
            AppView::FavoritesBrowseView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.current-app-commands" => {
            app.cached_current_app_entries = ["New Document", "Save Document", "Close Document"]
                .into_iter()
                .map(|name| {
                    crate::builtins::BuiltInEntry::new_with_group(
                        format!("menubar-owned-{name}"),
                        format!("File → {name}"),
                        "Fixture Editor",
                        vec!["file".into()],
                        crate::builtins::BuiltInFeature::MenuBarAction(
                            crate::builtins::MenuBarActionInfo {
                                bundle_id: "dev.scriptkit.fixture-editor".into(),
                                menu_path: vec!["File".into(), name.into()],
                                enabled: true,
                                shortcut: None,
                            },
                        ),
                        Some("file".into()),
                        crate::builtins::BuiltInGroup::MenuBar,
                    )
                })
                .collect();
            AppView::CurrentAppCommandsView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.agent-chat-history" => {
            use crate::ai::agent_chat::ui::history::{SavedConversation, SavedMessage};
            crate::ai::agent_chat::ui::history::seed_owned_history(&[SavedConversation {
                session_id: "owned-main-history".into(),
                timestamp: "2026-08-28T12:00:00Z".into(),
                custom_title: Some("Launch Review".into()),
                messages: vec![
                    SavedMessage {
                        role: "user".into(),
                        body: "Review the launch checklist".into(),
                    },
                    SavedMessage {
                        role: "assistant".into(),
                        body: "The release checklist has three items.".into(),
                    },
                ],
            }])?;
            AppView::AgentChatHistoryView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.browser-history" => {
            app.cached_browser_history = fixture_browser_history();
            AppView::BrowserHistoryView {
                filter: String::new(),
                selected_index: 0,
            }
        }
        "main.dictation-history" => {
            let entries = (0..18)
                .map(|index| crate::dictation::DictationHistoryEntry {
                    version: 1,
                    id: format!("owned-transcript-{index}"),
                    timestamp: "2026-08-28T12:00:00Z".into(),
                    transcript: format!("Release checklist item {index}: review the launch plan."),
                    preview: format!("Release checklist item {index}"),
                    target_id: "main".into(),
                    target_label_snapshot: "Launcher".into(),
                    audio_duration_ms: 2500,
                })
                .collect::<Vec<_>>();
            crate::dictation::seed_owned_dictation_history(&entries)?;
            AppView::DictationHistoryView {
                filter: String::new(),
                selected_index: 0,
                visible_limit: 10,
            }
        }
        "main.notes-browse"
        | "main.notes-browse-no-match"
        | "main.notes-browse-loading"
        | "main.notes-browse-failed"
        | "main.notes-browse-empty" => {
            use crate::notes::search_model::{
                NoteSearchDestination, NoteSearchHostState, NoteSearchState,
            };
            crate::design_evaluation::notes_fixtures::prepare_notes_storage()?;
            let mut search = NoteSearchHostState::load(
                if id.ends_with("no-match") {
                    "zzz_no_match"
                } else {
                    ""
                },
                NoteSearchDestination::OpenInNotes,
                &crate::notes::notes_brain_days_dir(),
            );
            match id {
                "main.notes-browse-loading" => {
                    search.state = NoteSearchState::Loading {
                        generation: search.generation,
                        prior_snapshot: search.state.snapshot().cloned(),
                    }
                }
                "main.notes-browse-failed" => {
                    search.state = NoteSearchState::Failed {
                        generation: search.generation,
                        prior_snapshot: search.state.snapshot().cloned(),
                        failure: crate::ai::reliability::context_unavailable_failure(
                            "Owned Notes source unavailable",
                        ),
                    }
                }
                "main.notes-browse-empty" => {
                    search.state = NoteSearchState::ReadyEmpty {
                        generation: search.generation,
                        corpus_empty: true,
                    };
                    search.selected_id = None;
                    search.scroll_anchor = None;
                }
                _ => {}
            }
            AppView::NotesBrowseView { search }
        }
        _ => anyhow::bail!("unknown main fixture: {id}"),
    };
    app.transition_current_view_and_rekey_main_automation_surface(view);
    app.invalidate_filter_cache();
    app.invalidate_grouped_cache();
    app.mark_main_data_changed();
    app.bind_owned_surface_revision_observers(cx);
    let fixture_query = match id {
        "main.menu-syntax-trigger" => Some(":"),
        "main.menu-syntax-object" => Some(";note @Alpha"),
        "main.menu-syntax-history" => Some(":type:script "),
        "main.notes-browse-no-match" => Some("zzz_no_match"),
        _ => None,
    };
    if let Some(query) = fixture_query {
        app.set_filter_text_immediate(query.into(), window, cx);
    }
    if matches!(app.current_view, AppView::ScriptList) {
        app.flush_pending_main_menu_query(cx);
    } else if matches!(app.current_view, AppView::FileSearchView { .. }) {
        app.recompute_file_search_display_indices();
    }
    app.gpui_input_state
        .update(cx, |input, cx| input.focus(window, cx));
    cx.notify();
    Ok(())
}

fn prepare_launcher_files(app: &ScriptListApp, root: &std::path::Path) -> anyhow::Result<()> {
    let scope = crate::runtime_policy::owned_evaluation().context("missing owned policy")?;
    let sources = app
        .main_services
        .owned_sources()
        .context("missing owned sources")?;
    for file in sources
        .files
        .iter()
        .chain(sources.root_file_provider_files.iter().flatten())
    {
        let path = std::path::Path::new(&file.path);
        scope.require_owned_path(path)?;
        anyhow::ensure!(path.starts_with(root), "fixture file outside owned root");
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(
                path,
                format!("# {}\n\nSynthetic launch checklist.\n", file.name),
            )?;
        }
    }
    for script in &app.scripts {
        scope.require_owned_path(&script.path)?;
        if !script.path.exists() {
            if let Some(parent) = script.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(
                &script.path,
                "// Synthetic catalogue source; never executed.\n",
            )?;
        }
    }
    Ok(())
}

fn fixture_windows(root: &std::path::Path) -> Vec<crate::window_control::WindowInfo> {
    (0..3)
        .map(|index| crate::window_control::WindowInfo {
            id: index as u32 + 1,
            app: "Fixture Editor".into(),
            title: format!("Launch document {}", index + 1),
            bounds: crate::window_control::Bounds::new(0, 0, 800, 600),
            pid: 2_000_000_001,
            bundle_id: Some("dev.scriptkit.fixture-editor".into()),
            app_path: Some(root.join("Fixture Editor.app")),
            app_order: 0,
            window_index: index,
            global_order: index,
            is_frontmost_app: false,
            is_focused: false,
            is_main: index == 0,
            is_minimized: false,
            is_on_current_space: false,
            descriptor: "Fixture Editor · owned synthetic metadata".into(),
            handle: crate::window_control::WindowHandle {
                pid: 2_000_000_001,
                native_window_id: None,
                registry_generation: 0,
                nonce: index as u64 + 1,
            },
        })
        .collect()
}
fn fixture_browser_tabs() -> Vec<crate::browser_tabs::BrowserTabInfo> {
    ["Launch Plan", "Release Checklist", "Omega Notes"]
        .into_iter()
        .enumerate()
        .map(|(index, title)| crate::browser_tabs::BrowserTabInfo {
            browser_name: "Fixture Browser".into(),
            browser_bundle_id: "dev.scriptkit.fixture-browser".into(),
            window_index: 1,
            tab_index: index + 1,
            title: title.into(),
            url: format!("https://example.invalid/{index}").into(),
        })
        .collect()
}
fn fixture_browser_history() -> Vec<crate::browser_history::BrowserHistoryEntry> {
    fixture_browser_tabs()
        .into_iter()
        .map(|tab| crate::browser_history::BrowserHistoryEntry {
            browser_name: tab.browser_name,
            browser_bundle_id: tab.browser_bundle_id,
            title: tab.title,
            url: tab.url,
            host: "example.invalid".into(),
            last_visited_at_ms: 1_777_593_600_000,
            visit_count: 3,
            profile: "Owned".into(),
        })
        .collect()
}

struct OwnedMainRootNotification;

fn overlay_sources(app: &mut ScriptListApp) -> anyhow::Result<&mut OwnedMainSources> {
    match &mut app.main_services {
        MainServices::OwnedFixtures(sources) => Ok(Arc::make_mut(sources)),
        MainServices::Production => anyhow::bail!("main_overlay_requires_owned_sources"),
    }
}

fn complete_root_dialog_fixture(
    app: &mut ScriptListApp,
    action: &'static str,
    cx: &mut Context<ScriptListApp>,
) -> bool {
    let Ok(sources) = overlay_sources(app) else {
        return false;
    };
    if sources.overlay_fixture_id != Some("main-overlay.root-dialog")
        || sources.overlay_actions.len() >= 16
    {
        return false;
    }
    sources.overlay_actions.push(action.into());
    app.mark_main_data_changed();
    crate::runtime_policy::record_completed_fixture_effect();
    cx.notify();
    true
}

/// Mount typed overlay data into the existing production Main/Root presentation.
pub(crate) fn mount_main_overlay_fixture(
    app: &mut ScriptListApp,
    fixture_id: &str,
    window: &mut Window,
    cx: &mut Context<ScriptListApp>,
) -> anyhow::Result<()> {
    use gpui_component::{notification::Notification, WindowExt as _};
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    anyhow::ensure!(
        window.is_owned_hidden(),
        "main_overlay_requires_owned_window"
    );
    let fixture_id = MAIN_OVERLAY_FIXTURE_IDS
        .iter()
        .copied()
        .find(|id| *id == fixture_id)
        .context("unknown_main_overlay_fixture")?;
    let scope = crate::runtime_policy::owned_evaluation().context("main_overlay_policy_missing")?;
    app.close_alias_input(cx);
    app.close_tab_ai_save_offer(cx);
    if app.show_bun_warning {
        app.dismiss_bun_warning(cx);
    }
    if app.show_logs {
        app.toggle_logs(cx);
    }
    if app.background_effect.is_some() {
        app.set_background_effect(None, cx);
    }
    app.hide_grid(cx);
    app.toast_manager.drain_pending();
    window.close_all_dialogs(cx);
    window.clear_notifications(cx);
    mount_main_fixture(app, "main.script-list", window, cx)?;
    {
        let sources = overlay_sources(app)?;
        sources.overlay_fixture_id = Some(fixture_id);
        sources.overlay_root_dialog = None;
        sources.overlay_root_notification = None;
        sources.overlay_actions.clear();
        sources.overlay_logs.clear();
    }
    match fixture_id {
        "main-overlay.alias" => {
            scope.require_owned_path(&crate::aliases::default_aliases_path())?;
            let command = "builtin/clipboard-history";
            let mut aliases = crate::aliases::load_alias_overrides()?;
            if !aliases.contains_key(command) {
                crate::aliases::save_alias_override(command, "clip")?;
                aliases.insert(command.into(), "clip".into());
                crate::runtime_policy::record_completed_fixture_effect();
            }
            overlay_sources(app)?.alias_overrides = aliases;
            app.show_alias_input(command.into(), "Clipboard History".into(), cx);
        }
        "main-overlay.tab-ai-save-offer" => {
            let record = crate::ai::TabAiExecutionRecord::from_parts(
                "Review the launch checklist".into(),
                "// Name: Owned Launch Review\nimport \"@scriptkit/sdk\";\nawait div(\"Owned launch review\");\n".into(),
                scope.root().join("synthetic-execution.ts").to_string_lossy().into_owned(),
                "owned-launch-review".into(), "ScriptList".into(), None,
                "owned-fixture".into(), "owned-fixture".into(), 0, "2026-08-28T12:00:00Z".into(),
            );
            app.open_tab_ai_save_offer(record, cx);
        }
        "main-overlay.root-dialog" => {
            app.pending_focus = None;
            let app_weak = cx.entity().downgrade();
            window.open_dialog(cx, move |dialog, _, _| {
                let accept = app_weak.clone();
                let cancel = app_weak.clone();
                dialog
                    .title("Review local change")
                    .child("This action completes only the owned fixture.")
                    .confirm()
                    .button_props(
                        gpui_component::dialog::DialogButtonProps::default()
                            .ok_text("OK")
                            .cancel_text("Cancel"),
                    )
                    .on_ok(move |_, _, cx| {
                        accept.upgrade().is_some_and(|app| {
                            app.update(cx, |app, cx| {
                                complete_root_dialog_fixture(app, "confirmed", cx)
                            })
                        })
                    })
                    .on_cancel(move |_, _, cx| {
                        cancel.upgrade().is_some_and(|app| {
                            app.update(cx, |app, cx| {
                                complete_root_dialog_fixture(app, "cancelled", cx)
                            })
                        })
                    })
            });
            let snapshot = gpui_component::Root::read(window, cx).layer_snapshot(cx);
            let dialog = snapshot.dialogs.last().context("root_dialog_not_opened")?;
            overlay_sources(app)?.overlay_root_dialog =
                Some((dialog.root_entity_id, dialog.generation));
        }
        "main-overlay.root-notification" => {
            window.push_notification(
                Notification::info("The owned document is ready to review.")
                    .title("Local update")
                    .id::<OwnedMainRootNotification>()
                    .autohide(false),
                cx,
            );
            let snapshot = gpui_component::Root::read(window, cx).layer_snapshot(cx);
            overlay_sources(app)?.overlay_root_notification = Some(
                snapshot
                    .notifications
                    .first()
                    .context("root_notification_not_opened")?
                    .entity_id,
            );
        }
        "main-overlay.warning" => app.show_bun_warning = true,
        "main-overlay.logs" => {
            overlay_sources(app)?.overlay_logs = vec![
                "[fixture] Launch catalogue prepared".into(),
                "[fixture] Local provider accepted the query".into(),
                "[fixture] Waiting for the next command".into(),
            ];
            app.toggle_logs(cx);
        }
        "main-overlay.loading" => {
            overlay_sources(app)?.file_delay = std::time::Duration::from_millis(1500);
            app.root_search.root_file_results.clear();
            app.root_search.root_file_result_cache.clear();
            app.set_filter_text_immediate("files: launch".into(), window, cx);
            app.ensure_main_list_loading_animation(cx);
            anyhow::ensure!(
                app.main_list_loading_kind().is_some(),
                "fixture_loading_source_not_started"
            );
        }
        "main-overlay.toast" => {
            app.toast_manager.push(
                components::toast::Toast::info("The owned action is ready to review.", &app.theme)
                    .duration_ms(Some(TOAST_INFO_MS)),
            );
            app.flush_pending_toasts(window, cx);
            let snapshot = gpui_component::Root::read(window, cx).layer_snapshot(cx);
            overlay_sources(app)?.overlay_root_notification = Some(
                snapshot
                    .notifications
                    .first()
                    .context("toast_not_presented")?
                    .entity_id,
            );
        }
        "main-overlay.effects" => {
            app.set_background_effect(Some(crate::effects::BackgroundEffect::Starfield), cx)
        }
        "main-overlay.debug-grid" => app.show_grid(crate::protocol::GridOptions::default(), cx),
        _ => unreachable!(),
    }
    app.mark_main_data_changed();
    app.mark_main_presentation_changed();
    cx.notify();
    Ok(())
}

pub(crate) fn main_overlay_state(
    app: &ScriptListApp,
    window: &Window,
    cx: &App,
) -> serde_json::Value {
    let layers = gpui_component::Root::read(window, cx).layer_snapshot(cx);
    let sources = app.main_services.owned_sources();
    serde_json::json!({
        "fixtureId": sources.and_then(|sources| sources.overlay_fixture_id),
        "alias": app.alias_input_entity.as_ref().filter(|_| app.alias_input_state.is_some()).map(|entity| {
            let input = entity.read(cx);
            serde_json::json!({"entityId": entity.entity_id().as_u64(), "text": input.text(), "cursor": input.input.cursor(), "selection": input.input.selection().range(), "semanticToken": input.semantic_token()})
        }),
        "saveOffer": app.tab_ai_save_offer_state.as_ref().map(|state| serde_json::json!({"filename": state.filename_stem, "error": state.error.as_ref().map(|error| error.as_ref()), "syntheticExecution": state.record.provider_id == "owned-fixture"})),
        "warningVisible": app.show_bun_warning, "logsVisible": app.show_logs,
        "logLines": if app.show_logs { sources.map(|sources| sources.overlay_logs.as_slice()).unwrap_or(&[]) } else { &[] },
        "loading": app.main_list_loading_kind().map(|kind| kind.footer_label()),
        "backgroundEffect": crate::effects::BackgroundEffect::persisted_slug(app.background_effect),
        "debugGrid": app.grid_config.as_ref().map(|grid| serde_json::json!({"gridSize":grid.grid_size,"showBounds":grid.show_bounds,"showBoxModel":grid.show_box_model,"showAlignmentGuides":grid.show_alignment_guides,"showDimensions":grid.show_dimensions})),
        "rootLayerRevision": layers.revision,
        "dialogs": layers.dialogs.iter().map(|dialog| serde_json::json!({"rootEntityId":dialog.root_entity_id,"generation":dialog.generation})).collect::<Vec<_>>(),
        "notifications": layers.notifications.iter().map(|notification| serde_json::json!({"entityId":notification.entity_id,"closing":notification.closing,"title":notification.title,"message":notification.message})).collect::<Vec<_>>(),
        "completedActions": sources.map(|sources| sources.overlay_actions.as_slice()).unwrap_or(&[]),
    })
}

/// Fixture-specific controls complement the shared append_root_layer_elements projection.
pub(crate) fn main_overlay_elements(
    app: &ScriptListApp,
    window: &Window,
    cx: &App,
) -> Vec<crate::protocol::ElementInfo> {
    use crate::protocol::{ElementContentKind, ElementInfo};
    let mut elements = Vec::new();
    let layers = gpui_component::Root::read(window, cx).layer_snapshot(cx);
    if app.alias_input_state.is_some() {
        if let Some(input) = &app.alias_input_entity {
            elements.extend(input.read(cx).automation_elements());
        }
    }
    if let Some(state) = &app.tab_ai_save_offer_state {
        let mut panel = ElementInfo::panel("tab-ai-save-offer");
        panel.semantic_id = "tab-ai-save-offer".into();
        panel.text = Some(format!("Save as {}.ts?", state.filename_stem));
        panel.value = state.error.as_ref().map(|error| error.to_string());
        elements.push(panel.redact_content(ElementContentKind::UserContent));
        for (id, label) in [
            ("tab-ai-save-offer:save", "Save"),
            ("tab-ai-save-offer:dismiss", "Dismiss"),
        ] {
            let mut command = ElementInfo::panel(id);
            command.semantic_id = id.into();
            command.text = Some(label.into());
            command.role = Some("keyboardAction".into());
            command.selectable = Some(true);
            elements.push(command);
        }
    }
    if app.show_bun_warning {
        let mut banner = ElementInfo::panel("main-warning-banner");
        banner.semantic_id = "main-warning-banner".into();
        banner.text = Some("bun is not installed. Install from bun.sh".into());
        elements.push(banner);
        let mut dismiss = ElementInfo::button(0, "Dismiss");
        dismiss.semantic_id = "main-warning-banner:dismiss".into();
        elements.push(dismiss);
    }
    if app.show_logs {
        let mut panel = ElementInfo::panel("main-log-panel");
        panel.semantic_id = "main-log-panel".into();
        elements.push(panel);
    }
    if let Some(kind) = app.main_list_loading_kind() {
        let mut loading = ElementInfo::panel("main-loading");
        loading.semantic_id = "main-loading".into();
        loading.text = Some(kind.footer_label().into());
        loading.kind = Some("loading".into());
        elements.push(loading);
    }
    if app.background_effect.is_some() {
        let mut effect = ElementInfo::panel("main-background-effect");
        effect.semantic_id = "main-background-effect".into();
        effect.value = Some(crate::effects::BackgroundEffect::persisted_slug(
            app.background_effect,
        ));
        elements.push(effect);
    }
    if app.grid_config.is_some() {
        let mut grid = ElementInfo::panel("main-debug-grid");
        grid.semantic_id = "main-debug-grid".into();
        elements.push(grid);
    }
    if !layers.dialogs.is_empty() {
        for element in &mut elements {
            if element.element_type == crate::protocol::ElementType::Button
                || element.role.as_deref() == Some("keyboardAction")
            {
                element.action_disabled = Some("covered_by_root_dialog".into());
                element.selectable = Some(false);
            }
        }
    }
    if let Some(sources) = app.main_services.owned_sources() {
        if let Some(dialog) = layers.dialogs.last().filter(|dialog| {
            sources.overlay_root_dialog == Some((dialog.root_entity_id, dialog.generation))
        }) {
            for (index, label, action) in [(0, "OK", "confirm"), (1, "Cancel", "cancel")] {
                let mut button = ElementInfo::button(index, label);
                button.semantic_id = format!(
                    "root-dialog:{}:{}:{action}",
                    dialog.root_entity_id, dialog.generation
                );
                elements.push(button);
            }
        }
    }
    elements
}

pub(crate) fn set_main_overlay_input(
    app: &mut ScriptListApp,
    text: &str,
    window: &Window,
    cx: &mut Context<ScriptListApp>,
) -> anyhow::Result<bool> {
    anyhow::ensure!(text.len() <= 4096, "overlay_input_too_large");
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    anyhow::ensure!(
        gpui_component::Root::read(window, cx)
            .layer_snapshot(cx)
            .dialogs
            .is_empty(),
        "root_dialog_has_no_editable_fixture_field"
    );
    if app.alias_input_state.is_none() || app.alias_input_entity.is_none() {
        return Ok(false);
    }
    app.update_alias_text(text.to_owned(), cx);
    Ok(true)
}

pub(crate) fn select_main_overlay_element(
    app: &mut ScriptListApp,
    id: &str,
    submit: bool,
    window: &mut Window,
    cx: &mut Context<ScriptListApp>,
) -> anyhow::Result<bool> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    anyhow::ensure!(
        window.is_owned_hidden(),
        "main_overlay_requires_owned_window"
    );
    let elements = main_overlay_elements(app, window, cx);
    let Some(element) = elements.iter().find(|element| element.semantic_id == id) else {
        return Ok(false);
    };
    if let Some(reason) = &element.action_disabled {
        anyhow::bail!("{reason}");
    }
    if !submit {
        return Ok(true);
    }
    if id.starts_with("alias-input:") && app.alias_input_state.is_some() {
        if let Some(input) = &app.alias_input_entity {
            return input
                .update(cx, |input, cx| input.activate_semantic_action(id, cx))
                .map_err(anyhow::Error::msg);
        }
    }
    match id {
        "tab-ai-save-offer:save" if app.tab_ai_save_offer_state.is_some() => {
            app.save_tab_ai_script(cx);
            return Ok(true);
        }
        "tab-ai-save-offer:dismiss" if app.tab_ai_save_offer_state.is_some() => {
            app.close_tab_ai_save_offer(cx);
            return Ok(true);
        }
        "main-warning-banner:dismiss" if app.show_bun_warning => {
            app.dismiss_bun_warning(cx);
            return Ok(true);
        }
        _ => {}
    }
    let layers = gpui_component::Root::read(window, cx).layer_snapshot(cx);
    let sources = app
        .main_services
        .owned_sources()
        .context("main_overlay_sources_missing")?;
    if let Some(dialog) = layers.dialogs.last().filter(|dialog| {
        sources.overlay_root_dialog == Some((dialog.root_entity_id, dialog.generation))
    }) {
        let prefix = format!(
            "root-dialog:{}:{}:",
            dialog.root_entity_id, dialog.generation
        );
        if id == format!("{prefix}confirm") {
            window.dispatch_action(
                Box::new(gpui_component::actions::Confirm { secondary: false }),
                cx,
            );
            return Ok(true);
        }
        if id == format!("{prefix}cancel") {
            window.dispatch_action(Box::new(gpui_component::actions::Cancel), cx);
            return Ok(true);
        }
    }
    anyhow::bail!("overlay_element_not_actionable")
}
