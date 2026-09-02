use super::*;

/// Prepared production records. Construction never discovers external state.
pub(crate) struct MainInitialData {
    pub scripts: Vec<Arc<scripts::Script>>,
    pub script_candidates: Arc<[Arc<scripts::Script>]>,
    pub scriptlets: Vec<Arc<scripts::Scriptlet>>,
    pub skills: Vec<Arc<crate::plugins::PluginSkill>>,
    pub script_validation_report: Option<Arc<scripts::ValidationReport>>,
    pub source_terminals: Vec<(&'static str, RootProviderTerminal)>,
    pub builtin_entries: Vec<builtins::BuiltInEntry>,
    pub apps: Vec<app_launcher::AppInfo>,
    pub windows: Vec<window_control::WindowInfo>,
    pub windows_status: window_control::RootWindowsProviderStatus,
    pub frecency_store: FrecencyStore,
    pub input_history: input_history::InputHistory,
    pub preferences: config::ScriptKitUserPreferences,
    pub launcher_context: crate::context_snapshot::launcher_context::LauncherContextSnapshot,
    pub theme: Arc<theme::Theme>,
    pub theme_revision: u64,
    pub cwd: Option<std::path::PathBuf>,
    pub cwd_label: Option<String>,
    pub cwd_revision: u64,
    pub agent_label: Option<String>,
    pub model_label: Option<String>,
    pub background_effect: Option<crate::effects::BackgroundEffect>,
    pub background_effect_intensity: f32,
    pub reduced_motion: bool,
}

pub(super) struct OwnedMainCatalogueSources {
    pub(super) scripts: Vec<Arc<scripts::Script>>,
    pub(super) scriptlets: Vec<Arc<scripts::Scriptlet>>,
    pub(super) skills: Vec<Arc<crate::plugins::PluginSkill>>,
    pub(super) apps: Vec<app_launcher::AppInfo>,
    pub(super) cwd: std::path::PathBuf,
    pub(super) cwd_label: &'static str,
}

impl OwnedMainCatalogueSources {
    pub(super) fn initial(root: &std::path::Path) -> Self {
        Self {
            scripts: [
                "Launch Alpha",
                "Launch Beta",
                "Launch Gamma",
                "Launch Delta",
                "Launch Epsilon",
                "Launch Zeta",
                "Omega Report",
            ]
            .into_iter()
            .map(|name| {
                Arc::new(scripts::Script {
                    name: name.into(),
                    description: Some(format!("Owned catalogue: {name}")),
                    path: root
                        .join("scripts")
                        .join(format!("{}.ts", name.to_lowercase().replace(' ', "-"))),
                    extension: "ts".into(),
                    plugin_id: "owned".into(),
                    ..Default::default()
                })
            })
            .collect(),
            scriptlets: vec![Arc::new(scripts::Scriptlet {
                name: "Launch Checklist".into(),
                description: Some("Review the release checklist".into()),
                code: "Release checklist".into(),
                tool: "paste".into(),
                shortcut: None,
                keyword: None,
                group: Some("Owned Fixtures".into()),
                plugin_id: "owned".into(),
                plugin_title: Some("Owned Fixtures".into()),
                file_path: Some(
                    root.join("scriptlets/checklist.md")
                        .to_string_lossy()
                        .into_owned(),
                ),
                command: Some("launch-checklist".into()),
                alias: None,
                icon: None,
            })],
            skills: vec![Arc::new(crate::plugins::PluginSkill {
                plugin_id: "owned".into(),
                plugin_title: "Owned Fixtures".into(),
                skill_id: "launch-review".into(),
                path: root.join("skills/launch-review/SKILL.md"),
                title: "Launch Review".into(),
                description: "Review a synthetic release".into(),
            })],
            apps: vec![app_launcher::AppInfo {
                name: "Fixture Editor".into(),
                path: root.join("Fixture Editor.app"),
                bundle_id: Some("dev.scriptkit.fixture-editor".into()),
                icon: None,
            }],
            cwd: root.into(),
            cwd_label: "Owned Workspace",
        }
    }
}

/// Typed sources are data, not a replacement search or rendering implementation.
#[derive(Clone)]
pub(crate) struct OwnedMainSources {
    pub files: Vec<crate::file_search::FileResult>,
    /// Optional root-provider corpus, separate from immediately visible recent files.
    pub root_file_provider_files: Option<Vec<crate::file_search::FileResult>>,
    pub file_delay: std::time::Duration,
    pub search_gate: Option<Arc<crate::design_evaluation::search_fixtures::SearchGate>>,
    pub brain_hits: Vec<crate::brain::RootBrainSearchHit>,
    pub brain_inbox: Vec<crate::brain::InboxItem>,
    pub has_custom_positions: bool,
    pub configure_snap_mode_available: bool,
    pub ghost_context: crate::scripts::search::ghost::GhostContext,
    pub permissions: Vec<(
        crate::permissions_wizard::PermissionKind,
        crate::platform::permiso_detect::PermissionStatus,
    )>,
    pub alias_overrides: std::collections::HashMap<String, String>,
    pub kits: Vec<crate::KitStoreSearchResult>,
    pub kit_error: Option<String>,
    pub shortcut_overrides: std::collections::HashMap<String, String>,
    pub sdk_host_availability: crate::mcp_resources::SdkHostAvailability,
    pub overlay_fixture_id: Option<&'static str>,
    pub overlay_logs: Vec<String>,
    pub overlay_root_dialog: Option<(u64, u64)>,
    pub overlay_root_notification: Option<u64>,
    pub overlay_actions: Vec<String>,
}

impl Default for OwnedMainSources {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            file_delay: std::time::Duration::ZERO,
            root_file_provider_files: None,
            search_gate: None,
            brain_hits: Vec::new(),
            brain_inbox: Vec::new(),
            has_custom_positions: false,
            configure_snap_mode_available: false,
            ghost_context: Default::default(),
            permissions: Vec::new(),
            alias_overrides: Default::default(),
            kits: Vec::new(),
            kit_error: None,
            shortcut_overrides: Default::default(),
            sdk_host_availability: crate::mcp_resources::SdkHostAvailability::current(Vec::new()),
            overlay_fixture_id: None,
            overlay_logs: Vec::new(),
            overlay_root_dialog: None,
            overlay_root_notification: None,
            overlay_actions: Vec::new(),
        }
    }
}

impl OwnedMainSources {
    pub(crate) fn launcher(root: &std::path::Path) -> Self {
        Self {
            files: ["launch-plan.md", "launch-notes.md", "omega-report.md"]
                .into_iter()
                .map(|name| crate::file_search::FileResult {
                    path: root.join(name).to_string_lossy().into_owned(),
                    name: name.into(),
                    size: 128,
                    modified: 1_777_593_600,
                    file_type: crate::file_search::FileType::Document,
                })
                .collect(),
            file_delay: std::time::Duration::from_millis(180),
            brain_hits: vec![crate::brain::RootBrainSearchHit {
                title: "Fixture Alpha".into(),
                excerpt: "Review the document before the launch".into(),
                source_label: "Note",
                source: crate::brain::DocSource::Note,
                source_id: "d0197594-1111-4000-8000-000000000001".into(),
            }],
            brain_inbox: vec![crate::brain::InboxItem {
                id: 1,
                kind: crate::brain::inbox::InboxKind::Commitment,
                title: "Review the launch checklist".into(),
                detail: "Confirm the document is ready".into(),
                source: "note".into(),
                source_id: "d0197594-1111-4000-8000-000000000001".into(),
                created_at: 1_782_907_200,
                resolved_at: None,
            }],
            ..Default::default()
        }
    }
}

#[derive(Clone)]
pub(crate) enum MainServices {
    Production,
    OwnedFixtures(Arc<OwnedMainSources>),
}

impl MainServices {
    pub(crate) fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    pub(crate) fn owned_sources(&self) -> Option<&OwnedMainSources> {
        match self {
            Self::Production => None,
            Self::OwnedFixtures(sources) => Some(sources),
        }
    }

    pub(crate) fn search_gate(
        &self,
    ) -> Option<Arc<crate::design_evaluation::search_fixtures::SearchGate>> {
        self.owned_sources()
            .and_then(|sources| sources.search_gate.clone())
    }

    pub(crate) fn search_now(&self) -> std::time::Instant {
        self.owned_sources()
            .and_then(|sources| sources.search_gate.as_ref())
            .map_or_else(std::time::Instant::now, |gate| gate.now())
    }

    pub(crate) fn host_policy(&self) -> crate::runtime_policy::WindowHostPolicy {
        match self {
            Self::Production => crate::runtime_policy::WindowHostPolicy::Interactive,
            Self::OwnedFixtures(_) => crate::runtime_policy::WindowHostPolicy::OwnedHidden,
        }
    }
}

impl MainInitialData {
    pub(crate) fn owned_fixture(
        config: &config::Config,
        root: &std::path::Path,
        theme: Arc<theme::Theme>,
        theme_revision: u64,
    ) -> Self {
        let catalogue = OwnedMainCatalogueSources::initial(root);
        let scripts = catalogue.scripts;
        let mut input_history =
            input_history::InputHistory::with_path(root.join("input-history.json"));
        input_history.add_entry("Launch Beta");
        input_history.add_entry("Omega Report");
        let script_candidates = Arc::from(scripts.clone());
        let script_report = scripts::validate_script_catalog(scripts.clone());
        let script_validation_report = Some(Arc::new(scripts::merge_scriptlet_validation_issues(
            &script_report.validation,
            &catalogue.scriptlets,
        )));
        Self {
            scripts,
            script_candidates,
            scriptlets: catalogue.scriptlets,
            skills: catalogue.skills,
            script_validation_report,
            source_terminals: Vec::new(),
            builtin_entries: builtins::get_builtin_entries(&config.get_builtins())
                .into_iter()
                .filter(|entry| !config.is_command_hidden(&entry.id))
                .collect(),
            apps: catalogue.apps,
            windows: Vec::new(),
            windows_status: window_control::RootWindowsProviderStatus::Ready { count: 0 },
            frecency_store: FrecencyStore::with_path(root.join("frecency.json")),
            input_history,
            preferences: config::ScriptKitUserPreferences::default(),
            launcher_context: Default::default(),
            theme,
            theme_revision,
            cwd: Some(catalogue.cwd),
            cwd_label: Some(catalogue.cwd_label.into()),
            cwd_revision: 1,
            agent_label: Some("Fixture Agent".into()),
            model_label: Some("Provider-free".into()),
            background_effect: None,
            background_effect_intensity: 1.0,
            reduced_motion: false,
        }
    }

    fn production(config: &config::Config) -> Self {
        // PERF: Parallelize script + scriptlet loading to reduce startup wall time.
        let load_start = std::time::Instant::now();
        let loaded = std::thread::scope(|scope| -> anyhow::Result<_> {
            let scripts_handle = scope.spawn(scripts::read_scripts);
            let scriptlets_handle = scope.spawn(scripts::load_scriptlets);
            let scripts_result = scripts_handle.join();
            let scriptlets_result = scriptlets_handle.join();
            let scripts =
                scripts_result.map_err(|_| anyhow::anyhow!("scripts_loader_panicked"))??;
            let scriptlets =
                scriptlets_result.map_err(|_| anyhow::anyhow!("scriptlets_loader_panicked"))??;
            Ok((scripts, scriptlets))
        });
        let mut source_terminals = Vec::new();
        let (scripts, script_candidates, scriptlets, script_validation_report) = match loaded {
            Ok((candidates, catalogue)) => {
                let script_candidates: Arc<[Arc<scripts::Script>]> = candidates.into();
                let report = scripts::validate_script_catalog(script_candidates.to_vec());
                // Both source reads succeeded; cold startup owns this snapshot.
                let scriptlets = catalogue.publish();
                let validation = Arc::new(scripts::merge_scriptlet_validation_issues(
                    &report.validation,
                    &scriptlets,
                ));
                let scripts: Vec<_> = report.scripts.iter().cloned().collect();
                source_terminals.push((
                    "scripts",
                    if scripts.is_empty() && scriptlets.is_empty() {
                        RootProviderTerminal::Empty
                    } else {
                        RootProviderTerminal::Success
                    },
                ));
                (scripts, script_candidates, scriptlets, Some(validation))
            }
            Err(error) => {
                tracing::error!(%error, "startup_script_catalogue_failed");
                source_terminals.push(("scripts", RootProviderTerminal::Failed));
                (
                    Vec::new(),
                    Arc::<[Arc<scripts::Script>]>::from([]),
                    Vec::new(),
                    None,
                )
            }
        };

        // Theme cache was initialized earlier in app startup before window creation.
        // Reuse it here so ScriptListApp construction does not re-read theme files
        // or re-run system appearance detection.
        let theme_load_started = std::time::Instant::now();
        let theme = std::sync::Arc::new(theme::get_cached_theme());
        let theme_revision_seen = crate::theme::service::theme_revision();
        logging::log(
            "PERF",
            &format!(
                "Startup theme reuse: source=cached elapsed_ms={:.2}",
                theme_load_started.elapsed().as_secs_f64() * 1000.0
            ),
        );
        // Config is now passed in from main() to avoid duplicate load (~100-300ms savings)

        // Load frecency data for suggested section tracking
        let suggested_config = config.get_suggested();
        let mut frecency_store = FrecencyStore::with_config(&suggested_config);
        frecency_store.load().ok(); // Ignore errors - starts fresh if file doesn't exist

        // Load built-in entries based on config, filtering out commands hidden via
        // `hiddenCommands` or per-command `commands.*.hidden` overrides.
        let builtin_entries: Vec<_> = builtins::get_builtin_entries(&config.get_builtins())
            .into_iter()
            .filter(|entry| !config.is_command_hidden(&entry.id))
            .collect();

        // Apps are loaded in the background to avoid blocking startup
        // Start with empty list, will be populated asynchronously
        let apps = Vec::new();

        let total_elapsed = load_start.elapsed();
        logging::log(
            "PERF",
            &format!(
                "Startup loading: {:.2}ms total ({} scripts, {} scriptlets, apps loading in background)",
                total_elapsed.as_secs_f64() * 1000.0,
                scripts.len(), scriptlets.len()
            ),
        );
        logging::log(
            "APP",
            &format!(
                "Loaded {} scripts from ~/.scriptkit/plugins/*/scripts",
                scripts.len()
            ),
        );
        logging::log(
            "APP",
            &format!(
                "Loaded {} scriptlets from ~/.scriptkit/plugins/*/scriptlets",
                scriptlets.len()
            ),
        );
        logging::log(
            "APP",
            &format!("Loaded {} built-in features", builtin_entries.len()),
        );
        logging::log("APP", "Applications loading in background...");
        logging::log("APP", "Loaded theme with system appearance detection");
        logging::log(
            "APP",
            &format!(
                "Loaded config: hotkey={:?}+{}, bun_path={:?}",
                config.hotkey.modifiers, config.hotkey.key, config.bun_path
            ),
        );

        let plugin_skills = match crate::plugins::discover_plugins()
            .and_then(|index| crate::plugins::discover_plugin_skills(&index))
        {
            Ok(skills) => {
                source_terminals.push((
                    "skills",
                    if skills.is_empty() {
                        RootProviderTerminal::Empty
                    } else {
                        RootProviderTerminal::Success
                    },
                ));
                skills.into_iter().map(Arc::new).collect()
            }
            Err(error) => {
                tracing::error!(%error, "startup_skill_catalogue_failed");
                source_terminals.push(("skills", RootProviderTerminal::Failed));
                Vec::new()
            }
        };
        let window_search_test_provider =
            std::env::var_os("SCRIPT_KIT_WINDOW_SEARCH_TEST_PROVIDER").is_some();
        let initial_cached_windows = if window_search_test_provider {
            crate::window_control::list_windows().unwrap_or_default()
        } else {
            Vec::new()
        };
        let initial_root_windows_provider_status = if window_search_test_provider {
            crate::window_control::RootWindowsProviderStatus::Ready {
                count: initial_cached_windows.len(),
            }
        } else {
            crate::window_control::RootWindowsProviderStatus::Unknown
        };
        let (initial_spine_cwd, initial_spine_cwd_label, initial_spine_cwd_revision) = {
            let persisted = crate::config::load_user_preferences()
                .ai
                .cwd
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_dir());
            match persisted {
                Some(path) => {
                    let label = crate::file_search::shorten_path(&path.to_string_lossy())
                        .trim_end_matches('/')
                        .to_string();
                    (Some(path), Some(label), 1_u64)
                }
                None => (
                    dirs::home_dir().map(|h| h.join(".scriptkit")),
                    Some("~/.scriptkit".to_string()),
                    0_u64,
                ),
            }
        };
        // Resolve the persisted profile/model into header display labels so the
        // selection (Shift+Tab Profile Switcher) is visible on first paint.
        let (initial_spine_agent_label, initial_spine_model_label) =
            ScriptListApp::resolve_agent_model_footer_labels();
        let mut input_history = input_history::InputHistory::new();
        if let Err(error) = input_history.load() {
            tracing::warn!(%error, "Failed to load input history");
        }
        Self {
            scripts,
            script_candidates,
            scriptlets,
            skills: plugin_skills,
            script_validation_report,
            source_terminals,
            builtin_entries,
            apps,
            windows: initial_cached_windows,
            windows_status: initial_root_windows_provider_status,
            frecency_store,
            input_history,
            preferences: config::load_user_preferences(),
            launcher_context: Default::default(),
            theme,
            theme_revision: theme_revision_seen,
            cwd: initial_spine_cwd,
            cwd_label: initial_spine_cwd_label,
            cwd_revision: initial_spine_cwd_revision,
            agent_label: initial_spine_agent_label,
            model_label: initial_spine_model_label,
            background_effect: crate::effects::initial_background_effect(),
            background_effect_intensity: crate::effects::initial_background_effect_intensity(),
            reduced_motion: crate::platform::prefers_reduced_motion(),
        }
    }
}

impl ScriptListApp {
    pub(crate) fn new(
        config: config::Config,
        bun_available: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        assert!(
            !crate::runtime_policy::is_owned_evaluation(),
            "production main startup in owned evaluator"
        );
        let data = MainInitialData::production(&config);
        let mut app = Self::from_initial_data(
            config,
            bun_available,
            data,
            MainServices::Production,
            create_stdout_response_sender(),
            window,
            cx,
        );
        app.start_production_services(cx);
        app
    }
    pub(super) fn complete_root_app_catalog(
        &mut self,
        mut apps: Vec<app_launcher::AppInfo>,
        elapsed: std::time::Duration,
        _cx: &mut Context<Self>,
    ) {
        let icons: std::collections::HashMap<_, _> = self
            .apps
            .iter()
            .filter_map(|app| app.icon.as_ref().map(|icon| (&app.path, icon)))
            .collect();
        for app in &mut apps {
            if app.icon.is_none() {
                app.icon = icons.get(&app.path).map(|icon| (*icon).clone());
            }
        }
        let app_count = apps.len();
        self.apps = apps;
        self.main_menu_result_caches.mark_apps_loaded();
        self.root_search
            .rebuild_root_windows(&self.cached_windows, &self.apps);
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();
        logging::log(
            "APP",
            &format!(
                "Background app loading complete: {} apps in {:.2}ms",
                app_count,
                elapsed.as_secs_f64() * 1000.0
            ),
        );
    }

    pub(crate) fn start_root_app_catalog(&mut self, cx: &mut Context<Self>) {
        let Some(work) = self.begin_root_catalogue_refresh("apps", "catalogue") else {
            return;
        };
        let (tx, rx) = async_channel::bounded(1);
        let owned_tx = if work.run.is_some() {
            Some(tx)
        } else {
            std::thread::spawn(move || {
                let started = std::time::Instant::now();
                let result = app_launcher::scan_applications_fresh();
                let _ = tx.send_blocking(result.map(|apps| (apps, started.elapsed())));
            });
            None
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some(tx)) = (&work.run, owned_tx) {
                run.deliver(
                    move |result| tx.try_send(result),
                    |outcome, run| {
                        crate::design_evaluation::search_fixtures::app_catalog(outcome, run)
                            .map(|apps| (apps, std::time::Duration::ZERO))
                    },
                )
                .await;
            }
            let result = rx.recv().await;
            let update = this.update(cx, |app, cx| {
                let mut accepted = false;
                app.complete_root_catalogue_refresh(
                    &work,
                    result,
                    |(apps, _)| apps.len(),
                    cx,
                    |app, cx, (apps, elapsed)| {
                        app.complete_root_app_catalog(apps, elapsed, cx);
                        accepted = true;
                        Ok(true)
                    },
                );
                if accepted {
                    app.ensure_root_app_icons(cx);
                }
            });
            if update.is_err() {
                if let Some(run) = &work.run {
                    run.finish(
                        crate::design_evaluation::search_fixtures::ProviderTerminal::StaleDiscarded,
                        RootProviderPublicationPolicy::Visible,
                    );
                }
            }
        })
        .detach();
    }

    fn root_app_icon_paths(&self) -> Vec<std::path::PathBuf> {
        let mut paths: Vec<_> = self.apps.iter().map(|app| app.path.clone()).collect();
        paths.sort();
        paths.dedup();
        paths
    }

    fn root_app_icon_scope(paths: &[std::path::PathBuf]) -> String {
        use sha2::Digest;
        let mut digest = sha2::Sha256::new();
        for path in paths {
            let bytes = path.as_os_str().as_encoded_bytes();
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        format!("app-icons:{:x}", digest.finalize())
    }

    pub(crate) fn ensure_root_app_icons(&mut self, cx: &mut Context<Self>) {
        if !self.apps.iter().any(|app| app.icon.is_none()) {
            return;
        }
        let paths = self.root_app_icon_paths();
        let scope = Self::root_app_icon_scope(&paths);
        if !self
            .root_search
            .named_provider_has_current_consumer("icons", &scope)
        {
            self.start_root_app_icon_work(paths, scope, cx);
        }
    }

    pub(crate) fn refresh_root_app_icons(&mut self, cx: &mut Context<Self>) {
        let paths = self.root_app_icon_paths();
        let scope = Self::root_app_icon_scope(&paths);
        self.start_root_app_icon_work(paths, scope, cx);
    }

    fn start_root_app_icon_work(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        scope: String,
        cx: &mut Context<Self>,
    ) {
        let Some(work) = self.begin_root_catalogue_refresh("icons", &scope) else {
            return;
        };
        let (tx, rx) = async_channel::bounded(1);
        cx.spawn(async move |this, cx| {
            if let Some(run) = &work.run {
                run.deliver(
                    move |result| tx.try_send(result),
                    move |outcome, run| {
                        if let Some(error) = outcome.error() {
                            return Err(error);
                        }
                        if outcome == crate::design_evaluation::search_fixtures::Outcome::Empty {
                            return Ok(Vec::new());
                        }
                        let rgba = if run.changed_payload() {
                            [128, 96, 72, 255]
                        } else {
                            [72, 96, 128, 255]
                        };
                        let frame = image::Frame::new(image::RgbaImage::from_pixel(
                            16,
                            16,
                            image::Rgba(rgba),
                        ));
                        let icon = Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame]));
                        Ok(paths.into_iter().map(|path| (path, icon.clone())).collect())
                    },
                )
                .await;
            } else {
                std::thread::spawn(move || {
                    let _ = tx.send_blocking(app_launcher::read_app_icons(paths));
                });
            }
            let result = rx.recv().await;
            let update = this.update(cx, |app, cx| {
                app.complete_root_catalogue_refresh(
                    &work,
                    result,
                    Vec::len,
                    cx,
                    |app, cx, icons| {
                        let icons: std::collections::HashMap<_, _> = icons.into_iter().collect();
                        if !app.apps.iter().any(|app| icons.contains_key(&app.path)) {
                            return Ok(false);
                        }
                        let mut current = app.apps.clone();
                        for app in &mut current {
                            if let Some(icon) = icons.get(&app.path) {
                                app.icon = Some(icon.clone());
                            }
                        }
                        app.complete_root_app_catalog(current, std::time::Duration::ZERO, cx);
                        Ok(true)
                    },
                );
            });
            if update.is_err() {
                if let Some(run) = &work.run {
                    run.finish(
                        crate::design_evaluation::search_fixtures::ProviderTerminal::StaleDiscarded,
                        RootProviderPublicationPolicy::Visible,
                    );
                }
            }
        })
        .detach();
    }

    fn start_production_services(&mut self, cx: &mut Context<Self>) {
        assert!(self.main_services.is_production());
        assert!(!crate::runtime_policy::is_owned_evaluation());
        if self.frontmost_menu_subscription_task.is_none() {
            let subscription = crate::frontmost_app_tracker::subscribe_menu_changes();
            self.frontmost_menu_subscription_task = Some(cx.spawn(async move |this, cx| {
                while subscription.changed().await.is_ok() {
                    let updated = this.update(cx, |app, cx| {
                        if !app.main_services.is_production()
                            || !matches!(app.current_view, AppView::ScriptList)
                            || !app.root_search.query_is_current()
                        {
                            // A pending query or route restoration consumes the latest
                            // producer snapshot through its normal query publication.
                            return;
                        }
                        app.commit_main_menu_results_refresh(
                            "frontmost_menu_changed",
                            None,
                            cx,
                            |app, _cx| {
                                app.invalidate_root_passive_and_grouped_cache();
                                true
                            },
                        );
                    });
                    if updated.is_err() {
                        break;
                    }
                }
            }));
        }
        // The detached chat window code is compiled into the lib, which
        // cannot name ScriptListApp; register the binary-side reattach hook
        // it dispatches through.
        crate::ai::agent_chat::ui::chat_window::register_reattach_into_main_hook(
            Self::reattach_detached_chat_hook,
        );
        // Load apps in background thread to avoid blocking startup
        let app_launcher_enabled = self.config.get_builtins().app_launcher;
        if app_launcher_enabled {
            self.start_root_app_catalog(cx);
        }

        #[cfg(not(test))]
        {
            let share_rx = crate::script_sharing::spawn_clipboard_share_watcher();
            cx.spawn(async move |this, cx| {
                while let Ok(import) = share_rx.recv().await {
                    tracing::info!(
                        share_uri = %import.uri,
                        title = %import.bundle.title,
                        kind = ?import.bundle.kind,
                        "clipboard_share_bundle_detected"
                    );
                    script_kit_gpui::request_show_main_window();
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(180))
                        .await;

                    let options = crate::confirm::ParentConfirmOptions {
                        title: import.bundle.prompt_title().into(),
                        body: import.bundle.prompt_body().into(),
                        confirm_text: "Install".into(),
                        cancel_text: "Ignore".into(),
                        ..Default::default()
                    };
                    let trace_id = format!(
                        "share-import-{}-{}",
                        import.bundle.kind.display_name().to_lowercase(),
                        import.bundle.title.to_lowercase().replace(' ', "-")
                    );

                    let confirmed =
                        match crate::confirm::confirm_with_parent_dialog(cx, options, &trace_id)
                            .await
                        {
                            Ok(confirmed) => confirmed,
                            Err(error) => {
                                tracing::error!(
                                    ?error,
                                    title = %import.bundle.title,
                                    "clipboard_share_confirm_failed"
                                );
                                continue;
                            }
                        };
                    if !confirmed {
                        continue;
                    }

                    let install_result =
                        crate::script_sharing::install_share_bundle(&import.bundle);
                    let title = import.bundle.title.clone();
                    let kind = import.bundle.kind.display_name().to_lowercase();
                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| match install_result {
                            Ok(outcome) => {
                                app.refresh_scripts(cx);
                                app.refresh_skills(cx);
                                app.transition_current_view_and_rekey_main_automation_surface(
                                    AppView::ScriptList,
                                );
                                app.reset_main_menu_selection_intent();
                                app.flush_pending_main_menu_query(cx);
                                app.show_hud(
                                    format!("Installed shared {} into {}", kind, outcome.plugin_id),
                                    Some(2000),
                                    cx,
                                );
                            }
                            Err(error) => {
                                app.show_error_toast(
                                    format!(
                                        "Failed to install shared {} '{}': {}",
                                        kind, title, error
                                    ),
                                    cx,
                                );
                            }
                        })
                    });
                }
            })
            .detach();
        }
        crate::dictation::hydrate_dictation_resource_from_history();
        // Prewarm the flow roster for the RESTORED effective cwd so flows
        // are already in the main-menu corpus by the first open. Must run
        // after the spine_cwd restore above — resolve_flow_cwd(None) would
        // warm the wrong cache key when a persisted cwd exists.
        {
            let restored_cwd = self
                .spine_cwd
                .as_ref()
                .map(|path| path.to_string_lossy().to_string());
            crate::flows::catalog::flow_catalog()
                .roster_for(&crate::flows::resolve_flow_cwd(restored_cwd));
        }

        // Push-driven roster arrival: rosters land on a background fetch
        // thread; without this hook an idle open window never repaints when
        // flows appear. The generation poll in filtering_cache stays as the
        // fallback for the same signal.
        #[cfg(not(test))]
        {
            let (roster_tx, roster_rx) = async_channel::bounded::<()>(1);
            // This is pending producer delivery, not observer-maintained state:
            // each cwd keeps only its newest completion until the single wake
            // is consumed, so unrelated cwd traffic cannot drop the root's data.
            let pending = Arc::new(parking_lot::Mutex::new(std::collections::BTreeMap::<
                String,
                u64,
            >::new()));
            let publisher_pending = pending.clone();
            crate::flows::catalog::flow_catalog().set_notify_hook(move |cwd, generation| {
                if roster_tx.is_closed() {
                    return;
                }
                {
                    let mut pending = publisher_pending.lock();
                    if let Some(previous) = pending.get_mut(cwd) {
                        *previous = (*previous).max(generation);
                    } else {
                        pending.insert(cwd.to_owned(), generation);
                    }
                }
                let _ = roster_tx.try_send(());
            });
            cx.spawn(async move |this, cx| {
                while roster_rx.recv().await.is_ok() {
                    let completed = std::mem::take(&mut *pending.lock());
                    let updated = this.update(cx, |app, cx| {
                        let cwd = app.flow_ux_cwd();
                        let Some(generation) = completed.get(&cwd).copied() else {
                            return;
                        };
                        if crate::flows::catalog::flow_catalog().roster_generation_for(&cwd)
                            != generation
                        {
                            return;
                        }
                        if app.root_search.query_is_current() {
                            app.commit_main_menu_results_refresh(
                                "flow-roster",
                                Some(("flow-roster", generation)),
                                cx,
                                |app, _cx| {
                                    app.invalidate_filter_cache();
                                    app.invalidate_grouped_cache();
                                    true
                                },
                            );
                        } else {
                            app.invalidate_filter_cache();
                            app.invalidate_grouped_cache();
                        }
                    });
                    if updated.is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        // Build provider registry in background to avoid blocking UI when opening AI chat
        {
            let config_clone = self.config.clone();
            let (tx, rx) = async_channel::bounded::<crate::ai::ProviderRegistry>(1);

            std::thread::spawn(move || {
                let registry =
                    crate::ai::ProviderRegistry::from_environment_with_config(Some(&config_clone));
                if tx.send_blocking(registry).is_err() {
                    logging::log(
                        "APP",
                        "Provider registry build result dropped: receiver unavailable",
                    );
                }
            });

            cx.spawn(async move |this, cx| {
                let Ok(registry) = rx.recv().await else {
                    logging::log(
                        "APP",
                        "Background provider registry build failed: channel closed",
                    );
                    return;
                };

                let provider_count = registry.provider_ids().len();
                let _ = cx.update(|cx| {
                    this.update(cx, |app, _cx| {
                        app.cached_provider_registry = Some(registry);
                        logging::log(
                            "APP",
                            &format!(
                                "Background provider registry ready: {} providers",
                                provider_count
                            ),
                        );
                    })
                });
            })
            .detach();
        }
        // Prewarm Agent Chat config and the hidden Agent Chat connection so the first
        // compatible Agent Chat submit can reuse an initialized runtime/session.
        crate::ai::agent_chat::ui::prewarm_agent_config();

        // Prewarm Agent Chat and the Tab AI harness asynchronously so AI-entry
        // shortcuts do not pay subprocess/session startup cost on submit.
        let app_entity_for_tab_ai_warm = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;
            cx.update(|cx| {
                let Some(app) = app_entity_for_tab_ai_warm.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    this.warm_agent_chat_on_startup(cx);
                    this.warm_tab_ai_harness_on_startup(cx);
                    this.warm_quick_terminal_pty(cx);
                });
            });
        })
        .detach();
    }
}
