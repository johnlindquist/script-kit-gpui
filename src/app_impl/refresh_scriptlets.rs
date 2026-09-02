use super::*;

const HUD_PLUGIN_INVENTORY_MS: u64 = 1400;

/// Canonical string spelling of a scriptlet markdown path. Falls back to
/// canonicalizing the parent (deleted files can't canonicalize) and then to
/// the raw spelling.
fn canonical_scriptlet_path_string(path: &std::path::Path) -> String {
    if let Ok(canonical) = path.canonicalize() {
        return canonical.to_string_lossy().to_string();
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        if let Ok(parent) = parent.canonicalize() {
            return parent.join(name).to_string_lossy().to_string();
        }
    }
    path.to_string_lossy().to_string()
}

/// True when a stored scriptlet `file_path` (`/path/to/file.md#command`)
/// refers to the changed file. Compared through canonical spellings: the
/// watcher delivers FSEvents-canonical paths (e.g. `/private/tmp/...`) while
/// the loader records whatever spelling the kit path uses (`/tmp/...`,
/// symlinked dotfiles). A raw prefix compare misses across spellings, so the
/// incremental update would remove nothing and append duplicates — after
/// which every scriptlet shortcut conflicts with itself and HUDs fire.
fn scriptlet_file_path_matches(
    file_path: &str,
    changed_raw: &str,
    changed_canonical: &str,
) -> bool {
    let md_part = file_path.split('#').next().unwrap_or(file_path);
    md_part == changed_raw
        || md_part == changed_canonical
        || canonical_scriptlet_path_string(std::path::Path::new(md_part)) == changed_canonical
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PluginInventoryCounts {
    plugins: usize,
    scripts: usize,
    scriptlets: usize,
}

fn collect_plugin_inventory_counts(
    scripts: &[std::sync::Arc<scripts::Script>],
    scriptlets: &[std::sync::Arc<scripts::Scriptlet>],
) -> PluginInventoryCounts {
    let mut plugin_ids = std::collections::BTreeSet::new();
    for script in scripts {
        if !script.plugin_id.is_empty() {
            plugin_ids.insert(script.plugin_id.clone());
        }
    }
    for scriptlet in scriptlets {
        if !scriptlet.plugin_id.is_empty() {
            plugin_ids.insert(scriptlet.plugin_id.clone());
        }
    }
    PluginInventoryCounts {
        plugins: plugin_ids.len(),
        scripts: scripts.len(),
        scriptlets: scriptlets.len(),
    }
}

fn build_plugin_inventory_hud(
    before: PluginInventoryCounts,
    after: PluginInventoryCounts,
) -> Option<String> {
    if after.plugins == 0 && after.scripts == 0 && after.scriptlets == 0 {
        return Some(
            "No plugin entrypoints yet — add one in plugins/main or install a plugin".to_string(),
        );
    }

    if before == after {
        return None;
    }

    let verb = if after.plugins > before.plugins {
        "ready"
    } else if after.plugins < before.plugins {
        "remaining"
    } else {
        "loaded"
    };

    Some(format!(
        "{} plugins {} · {} scripts · {} snippets",
        after.plugins, verb, after.scripts, after.scriptlets
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ScriptHotkeyRefreshAction {
    Update {
        path: String,
        old_shortcut: Option<String>,
        new_shortcut: Option<String>,
    },
}

struct AsyncScriptRefreshLoadResult {
    scripts: Vec<std::sync::Arc<scripts::Script>>,
    scriptlets: scripts::ScriptletCatalogue,
    scripts_elapsed: std::time::Duration,
    scriptlets_elapsed: std::time::Duration,
    total_elapsed: std::time::Duration,
}

pub(super) struct RootCatalogueWork {
    source: &'static str,
    generation: u64,
    scope: String,
    script_revision: Option<u64>,
    pub(super) run: Option<crate::design_evaluation::search_fixtures::SearchRun>,
}

fn catalogue_completion_terminal<T>(
    result: &Result<anyhow::Result<T>, async_channel::RecvError>,
    count: impl FnOnce(&T) -> usize,
) -> crate::design_evaluation::search_fixtures::ProviderTerminal {
    use crate::design_evaluation::search_fixtures::ProviderTerminal;
    match result {
        Ok(Ok(value)) => ProviderTerminal::Completed {
            count: count(value),
        },
        Ok(Err(error)) => ProviderTerminal::for_error(error),
        Err(_) => ProviderTerminal::Disconnected,
    }
}

fn canonical_script_shortcut(shortcut: Option<&str>) -> Option<String> {
    shortcut
        .map(str::trim)
        .filter(|shortcut| !shortcut.is_empty())
        .map(|shortcut| shortcut.to_lowercase())
}

fn plan_script_hotkey_refresh(
    old_scripts: &[std::sync::Arc<scripts::Script>],
    new_scripts: &[std::sync::Arc<scripts::Script>],
) -> Vec<ScriptHotkeyRefreshAction> {
    let old_by_path = old_scripts
        .iter()
        .map(|script| {
            (
                script.path.to_string_lossy().to_string(),
                script.shortcut.clone(),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let new_by_path = new_scripts
        .iter()
        .map(|script| {
            (
                script.path.to_string_lossy().to_string(),
                script.shortcut.clone(),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();

    let mut paths = old_by_path
        .keys()
        .chain(new_by_path.keys())
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();

    paths
        .into_iter()
        .filter_map(|path| {
            let old_shortcut = old_by_path.get(&path).cloned().flatten();
            let new_shortcut = new_by_path.get(&path).cloned().flatten();

            if canonical_script_shortcut(old_shortcut.as_deref())
                == canonical_script_shortcut(new_shortcut.as_deref())
            {
                return None;
            }

            if old_shortcut.is_none() && new_shortcut.is_none() {
                return None;
            }

            Some(ScriptHotkeyRefreshAction::Update {
                path,
                old_shortcut,
                new_shortcut,
            })
        })
        .collect()
}

fn load_plugin_skills() -> anyhow::Result<Vec<std::sync::Arc<crate::plugins::PluginSkill>>> {
    let index = crate::plugins::discover_plugins()?;
    Ok(crate::plugins::discover_plugin_skills(&index)?
        .into_iter()
        .map(std::sync::Arc::new)
        .collect())
}

fn apply_script_hotkey_refresh(actions: &[ScriptHotkeyRefreshAction]) {
    for action in actions {
        let ScriptHotkeyRefreshAction::Update {
            path,
            old_shortcut,
            new_shortcut,
        } = action;

        if let Err(error) =
            hotkeys::update_script_hotkey(path, old_shortcut.as_deref(), new_shortcut.as_deref())
        {
            logging::log(
                "HOTKEY",
                &format!(
                    "Failed to refresh script hotkey for {}: {} (old={:?}, new={:?})",
                    path, error, old_shortcut, new_shortcut
                ),
            );
        }
    }
}

fn spawn_async_script_refresh_load(
    scripts_loader: impl FnOnce() -> anyhow::Result<Vec<std::sync::Arc<scripts::Script>>>
        + Send
        + 'static,
    scriptlets_loader: impl FnOnce() -> anyhow::Result<scripts::ScriptletCatalogue> + Send + 'static,
) -> async_channel::Receiver<anyhow::Result<AsyncScriptRefreshLoadResult>> {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let load_started_at = std::time::Instant::now();
        let result = std::thread::scope(|scope| -> anyhow::Result<_> {
            let scripts_handle = scope.spawn(move || {
                let started = std::time::Instant::now();
                (scripts_loader(), started.elapsed())
            });
            let scriptlets_handle = scope.spawn(move || {
                let started = std::time::Instant::now();
                (scriptlets_loader(), started.elapsed())
            });
            // Join both workers even when one failed; no panic becomes an
            // apparently successful empty catalogue that erases last-good data.
            let scripts = scripts_handle.join();
            let scriptlets = scriptlets_handle.join();
            let (scripts, scripts_elapsed) =
                scripts.map_err(|_| anyhow::anyhow!("scripts_loader_panicked"))?;
            let (scriptlets, scriptlets_elapsed) =
                scriptlets.map_err(|_| anyhow::anyhow!("scriptlets_loader_panicked"))?;
            let scripts = scripts?;
            let scriptlets = scriptlets?;
            Ok(AsyncScriptRefreshLoadResult {
                scripts,
                scriptlets,
                scripts_elapsed,
                scriptlets_elapsed,
                total_elapsed: load_started_at.elapsed(),
            })
        });
        let _ = tx.send_blocking(result);
    });
    rx
}

impl ScriptListApp {
    pub(crate) fn reset_owned_search_catalogues(
        &mut self,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.main_services.owned_sources().is_some(),
            "owned_catalogue_services_required"
        );
        anyhow::ensure!(
            self.root_search.computed_query_stamp().is_none(),
            "owned_catalogue_reset_requires_pending_query"
        );
        let scope = crate::runtime_policy::owned_evaluation()
            .ok_or_else(|| anyhow::anyhow!("owned_catalogue_scope_required"))?;
        scope.require_owned_path(scope.root())?;
        let catalogue = super::startup_data::OwnedMainCatalogueSources::initial(scope.root());
        self.spine_cwd = Some(catalogue.cwd);
        self.spine_cwd_label = Some(catalogue.cwd_label.into());
        #[expect(
            clippy::expect_used,
            reason = "Catalogue cwd revisions must fail on exhaustion rather than reuse an identity."
        )]
        let next_revision = self
            .spine_cwd_revision
            .checked_add(1)
            .expect("owned catalogue cwd revision exhausted");
        self.spine_cwd_revision = next_revision;
        self.install_loaded_skills(catalogue.skills);
        let scriptlets =
            scripts::ScriptletCatalogue::from_scriptlets(catalogue.scriptlets).publish();
        self.install_loaded_scripts_and_scriptlets(catalogue.scripts, scriptlets);
        self.apps.clear();
        self.complete_root_app_catalog(catalogue.apps, std::time::Duration::ZERO, cx);
        let _ = self.rebuild_registries();
        Ok(())
    }

    pub(super) fn begin_root_catalogue_refresh(
        &mut self,
        source: &'static str,
        scope: &str,
    ) -> Option<RootCatalogueWork> {
        if self.root_search.named_provider_in_flight(source) {
            self.root_search.note_desired_provider(
                source,
                "",
                scope,
                RootProviderPublicationPolicy::Visible,
            );
            return None;
        }
        let generation = self.root_search.allocate_named_provider_generation(source);
        let run = if let Some(gate) = self.main_services.search_gate() {
            Some(gate.begin(
                source,
                &self.filter_text,
                generation,
                RootProviderPublicationPolicy::Visible,
            )?)
        } else if self.main_services.is_production() {
            None
        } else {
            return None;
        };
        self.root_search.begin_named_provider(
            source,
            generation,
            "",
            scope,
            RootProviderPublicationPolicy::Visible,
            false,
        );
        let script_revision =
            (source == "validation").then(|| self.root_search.script_catalogue_revision());
        Some(RootCatalogueWork {
            source,
            generation,
            scope: scope.to_owned(),
            script_revision,
            run,
        })
    }

    fn restart_root_catalogue_refresh(&mut self, source: &'static str, cx: &mut Context<Self>) {
        match source {
            "scripts" => self.refresh_scripts(cx),
            "skills" => self.refresh_skills(cx),
            "apps" => self.start_root_app_catalog(cx),
            "validation" => self.refresh_root_validation(cx),
            "flow-roster" => self.refresh_root_flow_roster(cx),
            "icons" => self.refresh_root_app_icons(cx),
            _ => unreachable!("unknown catalogue source"),
        }
    }

    pub(super) fn complete_root_catalogue_refresh<T>(
        &mut self,
        work: &RootCatalogueWork,
        result: Result<anyhow::Result<T>, async_channel::RecvError>,
        count: impl FnOnce(&T) -> usize,
        cx: &mut Context<Self>,
        apply: impl FnOnce(&mut Self, &mut Context<Self>, T) -> anyhow::Result<bool>,
    ) {
        use crate::design_evaluation::search_fixtures::ProviderTerminal;
        if !self
            .root_search
            .named_provider_work_is_current(work.source, work.generation)
        {
            if let Some(run) = &work.run {
                run.finish(
                    ProviderTerminal::StaleDiscarded,
                    RootProviderPublicationPolicy::Visible,
                );
            }
            return;
        }
        // A source change supersedes this snapshot without starting a second
        // worker. Retire this exact producer, then read the latest source once.
        let desired = self.root_search.take_named_provider_desired(work.source);
        let source_replaced = work
            .script_revision
            .is_some_and(|revision| revision != self.root_search.script_catalogue_revision());
        if desired || source_replaced {
            self.root_search.finish_named_provider(
                work.source,
                work.generation,
                RootProviderTerminal::StaleDiscarded,
            );
            if let Some(run) = &work.run {
                run.finish(
                    ProviderTerminal::StaleDiscarded,
                    RootProviderPublicationPolicy::Visible,
                );
            }
            self.restart_root_catalogue_refresh(work.source, cx);
            return;
        }
        let visible = self
            .root_search
            .accepts_named_provider(work.source, work.generation)
            && (work.source != "flow-roster" || work.scope == self.flow_ux_cwd());
        let mut terminal = catalogue_completion_terminal(&result, count);
        let finish = |app: &mut Self, terminal| {
            let terminal = match terminal {
                ProviderTerminal::Completed { count: 0 } => RootProviderTerminal::Empty,
                ProviderTerminal::Completed { .. } => RootProviderTerminal::Success,
                ProviderTerminal::Failed => RootProviderTerminal::Failed,
                ProviderTerminal::Unavailable => RootProviderTerminal::Unavailable,
                ProviderTerminal::Disconnected => RootProviderTerminal::Disconnected,
                ProviderTerminal::Cancelled => RootProviderTerminal::Cancelled,
                ProviderTerminal::StaleDiscarded => RootProviderTerminal::StaleDiscarded,
            };
            app.root_search
                .finish_named_provider(work.source, work.generation, terminal);
        };
        if let Ok(Ok(value)) = result {
            if visible {
                self.commit_main_menu_results_refresh(
                    work.source,
                    Some((work.source, work.generation)),
                    cx,
                    |app, cx| {
                        let changed = match apply(app, cx, value) {
                            Ok(changed) => changed,
                            Err(error) => {
                                terminal = ProviderTerminal::for_error(&error);
                                false
                            }
                        };
                        finish(app, terminal);
                        changed
                    },
                );
            } else {
                if let Err(error) = apply(self, cx, value) {
                    terminal = ProviderTerminal::for_error(&error);
                }
                finish(self, terminal);
            }
        } else {
            finish(self, terminal);
        }
        let succeeded = matches!(terminal, ProviderTerminal::Completed { .. });
        if let Some(run) = &work.run {
            run.finish(
                terminal,
                if visible {
                    RootProviderPublicationPolicy::Visible
                } else {
                    RootProviderPublicationPolicy::CacheOnly
                },
            );
        }
        if !succeeded {
            cx.notify();
        }
    }

    pub(crate) fn refresh_scripts(&mut self, cx: &mut Context<Self>) {
        let Some(work) = self.begin_root_catalogue_refresh("scripts", "catalogue") else {
            return;
        };
        let (tx, rx) = if work.run.is_some() {
            let (tx, rx) = async_channel::bounded(1);
            (Some(tx), rx)
        } else {
            (
                None,
                spawn_async_script_refresh_load(scripts::read_scripts, scripts::load_scriptlets),
            )
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some(tx)) = (&work.run, tx) {
                #[expect(
                    clippy::result_large_err,
                    reason = "Keep the native channel error and owned payload without a boxing allocation."
                )]
                run.deliver(move |result| tx.try_send(result), |outcome, run| {
                    crate::design_evaluation::search_fixtures::script_catalog(outcome, run).map(|(scripts, scriptlets)| AsyncScriptRefreshLoadResult {
                        scripts, scriptlets: scripts::ScriptletCatalogue::from_scriptlets(scriptlets), scripts_elapsed: std::time::Duration::ZERO,
                        scriptlets_elapsed: std::time::Duration::ZERO, total_elapsed: std::time::Duration::ZERO,
                    })
                }).await;
            }
            let result = rx.recv().await;
            let update = this.update(cx, |app, cx| {
                app.complete_root_catalogue_refresh(&work, result, |loaded| loaded.scripts.len() + loaded.scriptlets.len(), cx, |app, cx, loaded| {
                    logging::log("APP", &format!("script_refresh_async: scripts_ms={:.2} scriptlets_ms={:.2} total_ms={:.2}",
                        loaded.scripts_elapsed.as_secs_f64() * 1000.0, loaded.scriptlets_elapsed.as_secs_f64() * 1000.0, loaded.total_elapsed.as_secs_f64() * 1000.0));
                    app.apply_loaded_scripts_and_scriptlets(loaded.scripts, loaded.scriptlets, cx);
                    Ok(true)
                });
            });
            if update.is_err() {
                if let Some(run) = &work.run {
                    run.finish(crate::design_evaluation::search_fixtures::ProviderTerminal::StaleDiscarded, RootProviderPublicationPolicy::Visible);
                }
            }
        }).detach();
    }

    pub(crate) fn refresh_skills(&mut self, cx: &mut Context<Self>) {
        let Some(work) = self.begin_root_catalogue_refresh("skills", "catalogue") else {
            return;
        };
        let (tx, rx) = async_channel::bounded(1);
        let owned_tx = if work.run.is_some() {
            Some(tx)
        } else {
            std::thread::spawn(move || {
                let _ = tx.send_blocking(load_plugin_skills());
            });
            None
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some(tx)) = (&work.run, owned_tx) {
                run.deliver(
                    move |result| tx.try_send(result),
                    crate::design_evaluation::search_fixtures::skill_catalog,
                )
                .await;
            }
            let result = rx.recv().await;
            let update = this.update(cx, |app, cx| {
                app.complete_root_catalogue_refresh(
                    &work,
                    result,
                    Vec::len,
                    cx,
                    |app, _cx, skills| {
                        app.install_loaded_skills(skills);
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

    fn install_loaded_skills(&mut self, skills: Vec<std::sync::Arc<crate::plugins::PluginSkill>>) {
        self.skills = skills;
        crate::ai::context_selector::publish_launcher_catalog(
            &self.scripts,
            &self.scriptlets,
            &self.skills,
        );
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();
    }

    fn install_script_validation_report(&mut self, validation: &scripts::ValidationReport) -> bool {
        let report = scripts::merge_scriptlet_validation_issues(validation, &self.scriptlets);
        if self.script_validation_report.as_ref().is_some_and(|old| {
            old.schema_version == report.schema_version
                && old.total_candidates == report.total_candidates
                && old.valid_count == report.valid_count
                && old.fatal_count == report.fatal_count
                && old.warning_count == report.warning_count
                && old.failed_scripts == report.failed_scripts
                && old.warnings == report.warnings
                && old.retained_issues == report.retained_issues
        }) {
            return false;
        }
        self.script_validation_report = Some(std::sync::Arc::new(report));
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();
        true
    }

    pub(crate) fn refresh_root_validation(&mut self, cx: &mut Context<Self>) {
        let Some(work) = self.begin_root_catalogue_refresh("validation", "catalogue") else {
            return;
        };
        let (_, candidates) = self.root_search.script_catalogue_candidates();
        let (tx, rx) = async_channel::bounded(1);
        let owned = if work.run.is_some() {
            Some((tx, candidates))
        } else {
            std::thread::spawn(move || {
                let _ = tx.send_blocking(Ok(scripts::validate_script_catalog(candidates.to_vec())));
            });
            None
        };
        cx.spawn(async move |this, cx| {
            if let (Some(run), Some((tx, candidates))) = (&work.run, owned) {
                run.deliver(
                    move |result| tx.try_send(result),
                    |outcome, run| {
                        crate::design_evaluation::search_fixtures::validation_catalog(
                            outcome,
                            run,
                            candidates.to_vec(),
                        )
                    },
                )
                .await;
            }
            let result = rx.recv().await;
            let update = this.update(cx, |app, cx| {
                app.complete_root_catalogue_refresh(
                    &work,
                    result,
                    |report| report.validation.total_candidates,
                    cx,
                    |app, _cx, report| Ok(app.install_script_validation_report(&report.validation)),
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

    pub(crate) fn refresh_root_flow_roster(&mut self, cx: &mut Context<Self>) {
        let cwd = self.flow_ux_cwd();
        if self.main_services.is_production() {
            crate::flows::catalog::flow_catalog().refresh(&cwd);
            return;
        }
        let Some(work) = self.begin_root_catalogue_refresh("flow-roster", &cwd) else {
            return;
        };
        let (tx, rx) = async_channel::bounded(1);
        cx.spawn(async move |this, cx| {
            let Some(run) = &work.run else {
                return;
            };
            run.deliver(
                move |result| tx.try_send(result),
                crate::design_evaluation::search_fixtures::flow_roster,
            )
            .await;
            let result = rx.recv().await;
            let update = this.update(cx, |app, cx| {
                app.complete_root_catalogue_refresh(
                    &work,
                    result,
                    Vec::len,
                    cx,
                    |app, _cx, flows| {
                        // Install only after the producer fence: an old fixture is
                        // never allowed to write this process-wide catalogue.
                        crate::flows::catalog::flow_catalog().install_owned_roster(cwd, flows)?;
                        app.invalidate_filter_cache();
                        app.invalidate_grouped_cache();
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

    fn install_loaded_scripts_and_scriptlets(
        &mut self,
        loaded_scripts: Vec<std::sync::Arc<scripts::Script>>,
        loaded_scriptlets: Vec<std::sync::Arc<scripts::Scriptlet>>,
    ) {
        let candidates: std::sync::Arc<[std::sync::Arc<scripts::Script>]> = loaded_scripts.into();
        let catalog_report = scripts::validate_script_catalog(candidates.to_vec());
        self.root_search
            .install_script_catalogue_candidates(candidates);
        self.scripts = catalog_report.scripts.to_vec();
        self.scriptlets = loaded_scriptlets;
        self.install_script_validation_report(&catalog_report.validation);
        crate::ai::context_selector::publish_launcher_catalog(
            &self.scripts,
            &self.scriptlets,
            &self.skills,
        );
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();
        self.invalidate_preview_cache();
    }

    fn apply_loaded_scripts_and_scriptlets(
        &mut self,
        loaded_scripts: Vec<std::sync::Arc<scripts::Script>>,
        loaded_scriptlets: scripts::ScriptletCatalogue,
        cx: &mut Context<Self>,
    ) {
        let loaded_scriptlets = loaded_scriptlets.publish();
        let before_counts = collect_plugin_inventory_counts(&self.scripts, &self.scriptlets);
        let after_counts = collect_plugin_inventory_counts(&loaded_scripts, &loaded_scriptlets);

        if self.main_services.is_production() {
            let hotkey_refresh_actions = plan_script_hotkey_refresh(&self.scripts, &loaded_scripts);
            apply_script_hotkey_refresh(&hotkey_refresh_actions);
        }

        self.install_loaded_scripts_and_scriptlets(loaded_scripts, loaded_scriptlets);

        let indexed_bodies = self.scripts.iter().filter(|s| s.body.is_some()).count();
        logging::log(
            "APP",
            &format!(
                "script_content_index_refresh: scripts={} indexed_bodies={} filter_cache_invalidated=true grouped_cache_invalidated=true preview_cache_invalidated=true",
                self.scripts.len(),
                indexed_bodies
            ),
        );

        tracing::info!(
            plugins = after_counts.plugins,
            scripts = after_counts.scripts,
            scriptlets = after_counts.scriptlets,
            "plugin_inventory_refreshed"
        );

        // Rebuild alias/shortcut registries and show HUD for newly appearing
        // conflicts only — persistent ones already toasted on a prior refresh.
        let conflicts = self.rebuild_registries();
        for conflict in self.take_unannounced_registry_conflicts(conflicts) {
            self.show_hud(conflict, Some(HUD_CONFLICT_MS), cx); // 4s for conflict messages
        }

        if let Some(message) = build_plugin_inventory_hud(before_counts, after_counts) {
            self.show_hud(message, Some(HUD_PLUGIN_INVENTORY_MS), cx);
        }

        logging::log(
            "APP",
            &format!(
                "Scripts refreshed: {} scripts, {} scriptlets loaded",
                self.scripts.len(),
                self.scriptlets.len()
            ),
        );
    }

    /// Refresh app launcher cache and invalidate search caches.
    ///
    /// Called by AppWatcher when applications are added/removed/updated.
    /// This properly invalidates filter/grouped caches so the main search
    /// immediately reflects new apps without requiring user to type.
    ///
    /// NOTE: cx.notify() is efficient - GPUI batches notifications and only
    /// re-renders when the event loop runs. We always call it because:
    /// 1. If user is in ScriptList, cached search results need updating
    /// 2. If user is in AppLauncherView, the list needs updating
    /// 3. The cost of an "unnecessary" notify is near-zero (just marks dirty)
    pub fn refresh_apps(&mut self, cx: &mut Context<Self>) {
        self.start_root_app_catalog(cx);
    }

    /// Dismiss the bun warning banner
    pub(crate) fn dismiss_bun_warning(&mut self, cx: &mut Context<Self>) {
        if !self.show_bun_warning {
            return;
        }
        logging::log("APP", "Bun warning banner dismissed by user");
        self.show_bun_warning = false;
        self.mark_main_data_changed();
        self.mark_main_presentation_changed();
        cx.notify();
    }

    /// Open bun.sh in the default browser
    pub(crate) fn open_bun_website(&self) -> anyhow::Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)?;
        logging::log("APP", "Opening https://bun.sh in default browser");
        std::process::Command::new("open")
            .arg("https://bun.sh")
            .spawn()?;
        Ok(())
    }

    /// Handle incremental scriptlet file change
    ///
    /// Instead of reloading all scriptlets, this method:
    /// 1. Parses only the changed file
    /// 2. Diffs against cached state to find what changed
    /// 3. Updates hotkeys/keyword triggers incrementally
    /// 4. Updates the scriptlets list
    ///
    /// # Arguments
    /// * `path` - Path to the changed/deleted scriptlet file
    /// * `is_deleted` - Whether the file was deleted (vs created/modified)
    /// * `cx` - The context for UI updates
    pub(crate) fn handle_scriptlet_file_change(
        &mut self,
        path: &std::path::Path,
        is_deleted: bool,
        cx: &mut Context<Self>,
    ) {
        use script_kit_gpui::scriptlet_cache::{diff_scriptlets, CachedScriptlet};
        if self.root_search.named_provider_in_flight("scripts") {
            self.root_search.note_desired_provider(
                "scripts",
                "",
                "catalogue",
                RootProviderPublicationPolicy::Visible,
            );
        }
        let source_generation = if self.root_search.named_provider_in_flight("scripts") {
            None
        } else {
            let generation = self
                .root_search
                .allocate_named_provider_generation("scripts");
            self.root_search.begin_named_provider(
                "scripts",
                generation,
                "",
                "catalogue",
                RootProviderPublicationPolicy::Visible,
                false,
            );
            Some(generation)
        };

        logging::log(
            "APP",
            &format!(
                "Incremental scriptlet change: {} (deleted={})",
                path.display(),
                is_deleted
            ),
        );

        // Watcher paths arrive FSEvents-canonical; stored file_paths use the
        // loader's spelling. Compare canonical forms or duplicates accumulate.
        let changed_raw = path.to_string_lossy().to_string();
        let changed_canonical = canonical_scriptlet_path_string(path);

        // Get old cached scriptlets for this file (if any)
        // Note: We're using a simple approach here - comparing name+shortcut+expand+alias
        let old_scriptlets: Vec<CachedScriptlet> = self
            .scriptlets
            .iter()
            .filter(|s| {
                s.file_path
                    .as_ref()
                    .map(|fp| scriptlet_file_path_matches(fp, &changed_raw, &changed_canonical))
                    .unwrap_or(false)
            })
            .map(|s| {
                CachedScriptlet::new(
                    s.name.clone(),
                    s.shortcut.clone(),
                    s.keyword.clone(),
                    s.alias.clone(),
                    s.file_path.clone().unwrap_or_default(),
                )
            })
            .collect();

        // A deletion is an explicit successful empty source. A failed read is
        // not a deletion and must preserve the old rows and capability snapshot.
        let parsed = if is_deleted {
            scripts::ScriptletCatalogue::empty_source(path)
        } else {
            match scripts::read_scriptlets_from_file(path) {
                Ok(parsed) => parsed,
                Err(error) => {
                    tracing::warn!(%error, "scriptlet_source_refresh_failed");
                    if let Some(generation) = source_generation {
                        self.root_search.finish_named_provider(
                            "scripts",
                            generation,
                            RootProviderTerminal::Failed,
                        );
                        cx.notify();
                    }
                    return;
                }
            }
        };

        let new_scriptlets: Vec<CachedScriptlet> = parsed
            .scriptlets()
            .map(|s| {
                CachedScriptlet::new(
                    s.name.clone(),
                    s.shortcut.clone(),
                    s.keyword.clone(),
                    s.alias.clone(),
                    s.file_path.clone().unwrap_or_default(),
                )
            })
            .collect();

        // Compute diff for registration metadata changes (shortcuts, aliases)
        let diff = diff_scriptlets(&old_scriptlets, &new_scriptlets);

        if diff.is_empty() {
            logging::log(
                "APP",
                &format!("No registration metadata changes in {}", path.display()),
            );
            // Still need to update the scriptlets list even if no registration changes
            // because the content might have changed
        } else {
            logging::log(
                "APP",
                &format!(
                    "Scriptlet diff: {} added, {} removed, {} shortcut changes, {} keyword changes, {} alias changes",
                    diff.added.len(),
                    diff.removed.len(),
                    diff.shortcut_changes.len(),
                    diff.keyword_changes.len(),
                    diff.alias_changes.len()
                ),
            );
        }

        // Apply hotkey changes
        for removed in &diff.removed {
            if removed.shortcut.is_some() {
                if let Err(e) = hotkeys::unregister_script_hotkey(&removed.file_path) {
                    logging::log(
                        "HOTKEY",
                        &format!("Failed to unregister hotkey for {}: {}", removed.name, e),
                    );
                }
            }
        }

        for added in &diff.added {
            if let Some(ref shortcut) = added.shortcut {
                if let Err(e) = hotkeys::register_script_hotkey(&added.file_path, shortcut) {
                    logging::log(
                        "HOTKEY",
                        &format!("Failed to register hotkey for {}: {}", added.name, e),
                    );
                }
            }
        }

        for change in &diff.shortcut_changes {
            if let Err(e) = hotkeys::update_script_hotkey(
                &change.file_path,
                change.old.as_deref(),
                change.new.as_deref(),
            ) {
                logging::log(
                    "HOTKEY",
                    &format!("Failed to update hotkey for {}: {}", change.name, e),
                );
            }
        }

        let generation = source_generation.unwrap_or_else(|| {
            self.root_search
                .allocate_named_provider_generation("scripts")
        });
        let apply = |app: &mut Self, cx: &mut Context<Self>| {
            let new_scripts_scriptlets = parsed.publish();
            // ALWAYS update keyword triggers when a file changes
            // This is needed because the diff only tracks registration metadata (name, shortcut, keyword, alias)
            // but NOT the actual content. So content changes like "success three" -> "success four"
            // would be missed if we only update on diff changes.
            #[cfg(target_os = "macos")]
            {
                let (added, removed, updated) =
                    crate::keyword_manager::update_keyword_triggers_for_file(
                        path,
                        &new_scripts_scriptlets,
                    );
                if added > 0 || removed > 0 || updated > 0 {
                    logging::log(
                        "KEYWORD",
                        &format!(
                            "Updated keyword triggers for {}: {} added, {} removed, {} updated",
                            path.display(),
                            added,
                            removed,
                            updated
                        ),
                    );
                }
            }
            // Canonical comparison also handles deleted source files.
            app.scriptlets.retain(|scriptlet| {
                !scriptlet.file_path.as_ref().is_some_and(|file_path| {
                    scriptlet_file_path_matches(file_path, &changed_raw, &changed_canonical)
                })
            });
            app.scriptlets.extend(new_scripts_scriptlets);
            app.scriptlets.sort_by(|a, b| a.name.cmp(&b.name));
            let (_, candidates) = app.root_search.script_catalogue_candidates();
            let report = scripts::validate_script_catalog(candidates.to_vec());
            app.install_script_validation_report(&report.validation);
            if let Some(generation) = source_generation {
                let terminal = if app.scripts.is_empty() && app.scriptlets.is_empty() {
                    RootProviderTerminal::Empty
                } else {
                    RootProviderTerminal::Success
                };
                app.root_search
                    .finish_named_provider("scripts", generation, terminal);
            }
            crate::ai::context_selector::publish_launcher_catalog(
                &app.scripts,
                &app.scriptlets,
                &app.skills,
            );
            app.invalidate_filter_cache();
            app.invalidate_grouped_cache();
            app.invalidate_preview_cache();
            let conflicts = app.rebuild_registries();
            for conflict in app.take_unannounced_registry_conflicts(conflicts) {
                app.show_hud(conflict, Some(HUD_CONFLICT_MS), cx);
            }
            true
        };
        if self.root_search.query_is_current() {
            self.commit_main_menu_results_refresh(
                "scriptlet-file-change",
                Some(("scripts", generation)),
                cx,
                apply,
            );
        } else {
            apply(self, cx);
        }

        logging::log(
            "APP",
            &format!(
                "Scriptlet file updated incrementally: {} now has {} total scriptlets",
                path.display(),
                self.scriptlets.len()
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripts::Script;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn test_spawn_async_script_refresh_load_returns_results_when_loaders_run_off_main_thread() {
        let main_thread_id = std::thread::current().id();
        let scripts_thread_id = Arc::new(Mutex::new(None));
        let scriptlets_thread_id = Arc::new(Mutex::new(None));

        let scripts_thread_id_clone = Arc::clone(&scripts_thread_id);
        let scriptlets_thread_id_clone = Arc::clone(&scriptlets_thread_id);

        let rx = spawn_async_script_refresh_load(
            move || {
                std::thread::sleep(Duration::from_millis(5));
                *scripts_thread_id_clone
                    .lock()
                    .expect("scripts thread id lock should succeed") =
                    Some(std::thread::current().id());
                Ok(Vec::new())
            },
            move || {
                std::thread::sleep(Duration::from_millis(5));
                *scriptlets_thread_id_clone
                    .lock()
                    .expect("scriptlets thread id lock should succeed") =
                    Some(std::thread::current().id());
                Ok(scripts::ScriptletCatalogue::from_scriptlets(Vec::new()))
            },
        );

        let result = rx
            .recv_blocking()
            .expect("background loaders should send exactly one result")
            .expect("background catalogue loading should succeed");

        assert!(result.total_elapsed >= result.scripts_elapsed);
        assert!(result.total_elapsed >= result.scriptlets_elapsed);

        let scripts_worker_thread = scripts_thread_id
            .lock()
            .expect("scripts thread id lock should succeed")
            .expect("scripts loader should execute");
        let scriptlets_worker_thread = scriptlets_thread_id
            .lock()
            .expect("scriptlets thread id lock should succeed")
            .expect("scriptlets loader should execute");

        assert_ne!(scripts_worker_thread, main_thread_id);
        assert_ne!(scriptlets_worker_thread, main_thread_id);
    }

    #[test]
    fn script_loader_panic_is_an_error_not_a_successful_empty_catalogue() {
        let rx = spawn_async_script_refresh_load(
            || panic!("owned loader failure"),
            || Ok(scripts::ScriptletCatalogue::from_scriptlets(Vec::new())),
        );
        let result = rx
            .recv_blocking()
            .expect("worker should report loader failure");
        assert!(result.is_err());
    }

    #[test]
    fn native_broken_pipe_error_is_not_a_catalogue_transport_disconnect() {
        use crate::design_evaluation::search_fixtures::ProviderTerminal;
        let native: Result<anyhow::Result<Vec<()>>, async_channel::RecvError> = Ok(Err(
            std::io::Error::from(std::io::ErrorKind::BrokenPipe).into(),
        ));
        assert!(matches!(
            catalogue_completion_terminal(&native, Vec::len),
            ProviderTerminal::Failed
        ));
        let (tx, rx) = async_channel::bounded::<anyhow::Result<Vec<()>>>(1);
        drop(tx);
        assert!(matches!(
            catalogue_completion_terminal(&rx.recv_blocking(), Vec::len),
            ProviderTerminal::Disconnected
        ));
    }

    #[test]
    fn native_source_read_failure_rejects_combined_catalogue_payload() {
        let rx = spawn_async_script_refresh_load(
            || Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into()),
            || Ok(scripts::ScriptletCatalogue::from_scriptlets(Vec::new())),
        );
        let result = rx
            .recv_blocking()
            .expect("worker returns actual read result");
        assert!(
            result.is_err(),
            "failed scripts cannot install successful empty scriptlets"
        );
        let rx = spawn_async_script_refresh_load(
            || Ok(Vec::new()),
            || Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into()),
        );
        assert!(rx
            .recv_blocking()
            .expect("worker returns scriptlet error")
            .is_err());
    }
    fn test_script(path: &str, shortcut: Option<&str>) -> Arc<Script> {
        Arc::new(Script {
            path: PathBuf::from(path),
            shortcut: shortcut.map(ToString::to_string),
            ..Default::default()
        })
    }

    #[test]
    fn test_plan_script_hotkey_refresh_detects_add_remove_and_change() {
        let old_scripts = vec![
            test_script("/tmp/keep.ts", Some("cmd+1")),
            test_script("/tmp/remove.ts", Some("cmd+2")),
            test_script("/tmp/change.ts", Some("cmd+3")),
        ];
        let new_scripts = vec![
            test_script("/tmp/keep.ts", Some("cmd+1")),
            test_script("/tmp/change.ts", Some("cmd+4")),
            test_script("/tmp/add.ts", Some("cmd+5")),
        ];

        let actions = plan_script_hotkey_refresh(&old_scripts, &new_scripts);

        assert_eq!(
            actions,
            vec![
                ScriptHotkeyRefreshAction::Update {
                    path: "/tmp/add.ts".to_string(),
                    old_shortcut: None,
                    new_shortcut: Some("cmd+5".to_string()),
                },
                ScriptHotkeyRefreshAction::Update {
                    path: "/tmp/change.ts".to_string(),
                    old_shortcut: Some("cmd+3".to_string()),
                    new_shortcut: Some("cmd+4".to_string()),
                },
                ScriptHotkeyRefreshAction::Update {
                    path: "/tmp/remove.ts".to_string(),
                    old_shortcut: Some("cmd+2".to_string()),
                    new_shortcut: None,
                },
            ]
        );
    }

    #[test]
    fn test_plan_script_hotkey_refresh_ignores_case_only_shortcut_changes() {
        let old_scripts = vec![test_script("/tmp/case.ts", Some("CMD+4"))];
        let new_scripts = vec![test_script("/tmp/case.ts", Some("cmd+4"))];

        let actions = plan_script_hotkey_refresh(&old_scripts, &new_scripts);

        assert!(actions.is_empty());
    }

    // ── collect_plugin_inventory_counts ─────────────────────────────

    fn test_script_with_plugin(path: &str, plugin_id: &str) -> Arc<Script> {
        Arc::new(Script {
            path: PathBuf::from(path),
            plugin_id: plugin_id.to_string(),
            ..Default::default()
        })
    }

    fn test_scriptlet(name: &str, plugin_id: &str) -> Arc<crate::scripts::Scriptlet> {
        Arc::new(crate::scripts::Scriptlet {
            name: name.to_string(),
            plugin_id: plugin_id.to_string(),
            code: String::new(),
            tool: "bash".to_string(),
            description: None,
            shortcut: None,
            keyword: None,
            group: None,
            plugin_title: None,
            file_path: None,
            command: None,
            alias: None,
            icon: None,
        })
    }

    #[test]
    fn test_collect_counts_groups_by_unique_plugin_id() {
        let scripts = vec![
            test_script_with_plugin("/a.ts", "main"),
            test_script_with_plugin("/b.ts", "main"),
            test_script_with_plugin("/c.ts", "tools"),
        ];
        let scriptlets = vec![
            test_scriptlet("s1", "main"),
            test_scriptlet("s2", "examples"),
        ];

        let counts = collect_plugin_inventory_counts(&scripts, &scriptlets);
        assert_eq!(counts.plugins, 3, "main + tools + examples");
        assert_eq!(counts.scripts, 3);
        assert_eq!(counts.scriptlets, 2);
    }

    #[test]
    fn test_collect_counts_skips_empty_plugin_id() {
        let scripts = vec![
            test_script_with_plugin("/a.ts", "main"),
            test_script_with_plugin("/b.ts", ""),
        ];
        let scriptlets = vec![test_scriptlet("s1", "")];

        let counts = collect_plugin_inventory_counts(&scripts, &scriptlets);
        assert_eq!(counts.plugins, 1, "only main counted");
        assert_eq!(counts.scripts, 2);
        assert_eq!(counts.scriptlets, 1);
    }

    #[test]
    fn test_collect_counts_empty_inventory() {
        let counts = collect_plugin_inventory_counts(&[], &[]);
        assert_eq!(counts.plugins, 0);
        assert_eq!(counts.scripts, 0);
        assert_eq!(counts.scriptlets, 0);
    }

    // ── build_plugin_inventory_hud ─────────────────────────────────

    #[test]
    fn test_hud_shows_empty_state_when_inventory_becomes_empty() {
        let before = PluginInventoryCounts {
            plugins: 2,
            scripts: 5,
            scriptlets: 3,
        };
        let after = PluginInventoryCounts {
            plugins: 0,
            scripts: 0,
            scriptlets: 0,
        };

        let hud = build_plugin_inventory_hud(before, after);
        assert!(hud.is_some());
        assert!(
            hud.as_ref()
                .expect("hud should be Some")
                .contains("No plugin entrypoints yet"),
            "got: {:?}",
            hud
        );
    }

    #[test]
    fn test_hud_returns_none_when_inventory_unchanged() {
        let same = PluginInventoryCounts {
            plugins: 3,
            scripts: 10,
            scriptlets: 5,
        };
        assert!(
            build_plugin_inventory_hud(same, same).is_none(),
            "unchanged inventory should suppress HUD"
        );
    }

    #[test]
    fn test_hud_says_ready_when_plugins_increase() {
        let before = PluginInventoryCounts {
            plugins: 2,
            scripts: 8,
            scriptlets: 4,
        };
        let after = PluginInventoryCounts {
            plugins: 3,
            scripts: 11,
            scriptlets: 6,
        };

        let hud = build_plugin_inventory_hud(before, after)
            .expect("should produce HUD for plugin increase");
        assert!(hud.contains("ready"), "got: {}", hud);
        assert!(hud.contains("3 plugins"), "got: {}", hud);
        assert!(hud.contains("11 scripts"), "got: {}", hud);
        assert!(hud.contains("6 snippets"), "got: {}", hud);
    }

    #[test]
    fn test_hud_says_remaining_when_plugins_decrease() {
        let before = PluginInventoryCounts {
            plugins: 5,
            scripts: 20,
            scriptlets: 10,
        };
        let after = PluginInventoryCounts {
            plugins: 4,
            scripts: 16,
            scriptlets: 8,
        };

        let hud = build_plugin_inventory_hud(before, after)
            .expect("should produce HUD for plugin decrease");
        assert!(hud.contains("remaining"), "got: {}", hud);
        assert!(hud.contains("4 plugins"), "got: {}", hud);
    }

    #[test]
    fn test_hud_says_loaded_when_plugin_count_same_but_scripts_change() {
        let before = PluginInventoryCounts {
            plugins: 3,
            scripts: 10,
            scriptlets: 5,
        };
        let after = PluginInventoryCounts {
            plugins: 3,
            scripts: 12,
            scriptlets: 5,
        };

        let hud = build_plugin_inventory_hud(before, after)
            .expect("should produce HUD for script count change");
        assert!(hud.contains("loaded"), "got: {}", hud);
        assert!(hud.contains("12 scripts"), "got: {}", hud);
    }

    #[test]
    fn test_hud_empty_state_from_zero() {
        let zero = PluginInventoryCounts {
            plugins: 0,
            scripts: 0,
            scriptlets: 0,
        };
        let hud = build_plugin_inventory_hud(zero, zero);
        assert!(
            hud.is_some(),
            "empty-to-empty should still show empty state"
        );
        assert!(hud
            .as_ref()
            .expect("hud should be Some")
            .contains("No plugin entrypoints yet"),);
    }
}

#[cfg(test)]
mod scriptlet_path_match_tests {
    use super::{canonical_scriptlet_path_string, scriptlet_file_path_matches};

    /// Reproduces the probe-session bug: the loader stored `/tmp/...` (kit
    /// path spelling) while the watcher delivered `/private/tmp/...`
    /// (FSEvents-canonical). The raw prefix compare missed, the incremental
    /// update appended duplicates, and every scriptlet shortcut conflicted
    /// with itself.
    #[test]
    fn symlinked_spelling_matches_canonical_watcher_path() {
        let real_root = tempfile::tempdir().expect("tempdir");
        let real_dir = real_root.path().join("scriptlets");
        std::fs::create_dir_all(&real_dir).expect("create scriptlets dir");
        let md = real_dir.join("main.md");
        std::fs::write(&md, "## Test\n").expect("write md");

        let link_root = tempfile::tempdir().expect("link tempdir");
        let link = link_root.path().join("kit-link");
        std::os::unix::fs::symlink(real_root.path(), &link).expect("symlink");
        let md_via_link = link.join("scriptlets").join("main.md");

        let stored = format!("{}#translate-to-english", md_via_link.display());
        let changed_raw = md.to_string_lossy().to_string();
        let changed_canonical = canonical_scriptlet_path_string(&md);

        assert!(
            scriptlet_file_path_matches(&stored, &changed_raw, &changed_canonical),
            "symlink spelling {stored} must match canonical watcher path {changed_canonical}"
        );
    }

    #[test]
    fn deleted_file_matches_via_parent_canonicalization() {
        let root = tempfile::tempdir().expect("tempdir");
        let md = root.path().join("gone.md");
        // Never created: canonicalize(md) fails, parent fallback must engage.
        let canonical = canonical_scriptlet_path_string(&md);
        let stored = format!("{}#cmd", md.display());
        assert!(scriptlet_file_path_matches(
            &stored,
            &md.to_string_lossy(),
            &canonical
        ));
    }

    #[test]
    fn unrelated_file_does_not_match() {
        let root = tempfile::tempdir().expect("tempdir");
        let a = root.path().join("a.md");
        let b = root.path().join("b.md");
        std::fs::write(&a, "## A\n").expect("write a");
        std::fs::write(&b, "## B\n").expect("write b");
        let stored = format!("{}#cmd", a.display());
        assert!(!scriptlet_file_path_matches(
            &stored,
            &b.to_string_lossy(),
            &canonical_scriptlet_path_string(&b)
        ));
    }
}
