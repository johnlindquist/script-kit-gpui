use super::*;

static SCRIPT_REFRESH_REQUEST_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

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
    scriptlets: Vec<std::sync::Arc<scripts::Scriptlet>>,
    scripts_elapsed: std::time::Duration,
    scriptlets_elapsed: std::time::Duration,
    total_elapsed: std::time::Duration,
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

fn load_plugin_skills() -> Vec<std::sync::Arc<crate::plugins::PluginSkill>> {
    crate::plugins::discover_plugins()
        .and_then(|index| crate::plugins::discover_plugin_skills(&index))
        .unwrap_or_default()
        .into_iter()
        .map(std::sync::Arc::new)
        .collect()
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
    scripts_loader: impl FnOnce() -> Vec<std::sync::Arc<scripts::Script>> + Send + 'static,
    scriptlets_loader: impl FnOnce() -> Vec<std::sync::Arc<scripts::Scriptlet>> + Send + 'static,
) -> async_channel::Receiver<AsyncScriptRefreshLoadResult> {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let load_started_at = std::time::Instant::now();
        let (scripts, scripts_elapsed, scriptlets, scriptlets_elapsed) = std::thread::scope(
            |scope| {
                let scripts_handle = scope.spawn(move || {
                    let started = std::time::Instant::now();
                    (scripts_loader(), started.elapsed())
                });
                let scriptlets_handle = scope.spawn(move || {
                    let started = std::time::Instant::now();
                    (scriptlets_loader(), started.elapsed())
                });

                let (scripts, scripts_elapsed) = match scripts_handle.join() {
                    Ok(result) => result,
                    Err(_) => {
                        logging::log(
                        "ERROR",
                        "script_refresh_async: attempted=load_scripts failed=thread_panicked state=background_loading",
                    );
                        (Vec::new(), std::time::Duration::ZERO)
                    }
                };
                let (scriptlets, scriptlets_elapsed) = match scriptlets_handle.join() {
                    Ok(result) => result,
                    Err(_) => {
                        logging::log(
                        "ERROR",
                        "script_refresh_async: attempted=load_scriptlets failed=thread_panicked state=background_loading",
                    );
                        (Vec::new(), std::time::Duration::ZERO)
                    }
                };

                (scripts, scripts_elapsed, scriptlets, scriptlets_elapsed)
            },
        );

        let result = AsyncScriptRefreshLoadResult {
            scripts,
            scriptlets,
            scripts_elapsed,
            scriptlets_elapsed,
            total_elapsed: load_started_at.elapsed(),
        };

        if tx.send_blocking(result).is_err() {
            logging::log(
                "ERROR",
                "script_refresh_async: attempted=send_load_result failed=receiver_dropped state=background_complete",
            );
        }
    });
    rx
}

impl ScriptListApp {
    pub(crate) fn refresh_scripts(&mut self, cx: &mut Context<Self>) {
        let request_id =
            SCRIPT_REFRESH_REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        logging::log(
            "APP",
            &format!(
                "script_refresh_async: state=dispatching_background_load request_id={} current_scripts={} current_scriptlets={}",
                request_id,
                self.scripts.len(),
                self.scriptlets.len()
            ),
        );

        let rx = spawn_async_script_refresh_load(scripts::read_scripts, scripts::load_scriptlets);
        cx.spawn(async move |this, cx| {
            let Ok(load_result) = rx.recv().await else {
                logging::log(
                    "ERROR",
                    "script_refresh_async: attempted=receive_load_result failed=channel_closed state=awaiting_background_load",
                );
                return;
            };

            let scripts_count = load_result.scripts.len();
            let scriptlets_count = load_result.scriptlets.len();
            let scripts_elapsed_ms = load_result.scripts_elapsed.as_secs_f64() * 1000.0;
            let scriptlets_elapsed_ms = load_result.scriptlets_elapsed.as_secs_f64() * 1000.0;
            let total_elapsed_ms = load_result.total_elapsed.as_secs_f64() * 1000.0;
            let latest_request_id =
                SCRIPT_REFRESH_REQUEST_ID.load(std::sync::atomic::Ordering::Relaxed);
            if request_id != latest_request_id {
                logging::log(
                    "APP",
                    &format!(
                        "script_refresh_async: state=discarding_stale_result request_id={} latest_request_id={} scripts={} scriptlets={}",
                        request_id,
                        latest_request_id,
                        scripts_count,
                        scriptlets_count
                    ),
                );
                return;
            }

            let update_result = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    app.apply_loaded_scripts_and_scriptlets(
                        load_result.scripts,
                        load_result.scriptlets,
                        cx,
                    );
                    logging::log(
                        "APP",
                        &format!(
                            "script_refresh_async: state=applied_to_ui request_id={} scripts={} scriptlets={} scripts_ms={:.2} scriptlets_ms={:.2} total_ms={:.2}",
                            request_id,
                            scripts_count,
                            scriptlets_count,
                            scripts_elapsed_ms,
                            scriptlets_elapsed_ms,
                            total_elapsed_ms
                        ),
                    );
                })
            });

            if update_result.is_err() {
                logging::log(
                    "ERROR",
                    "script_refresh_async: attempted=apply_loaded_results failed=ui_entity_unavailable state=applying_to_ui",
                );
            }
        })
        .detach();
    }

    pub(crate) fn refresh_skills(&mut self, cx: &mut Context<Self>) {
        let before_count = self.skills.len();
        self.skills = load_plugin_skills();
        crate::ai::context_selector::publish_launcher_catalog(
            &self.scripts,
            &self.scriptlets,
            &self.skills,
        );

        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();

        self.sync_list_state();
        self.selected_index = 0;
        self.validate_selection_bounds(cx);
        self.main_list_state
            .scroll_to_reveal_item(self.selected_index);
        self.last_scrolled_index = Some(self.selected_index);

        if before_count != self.skills.len() {
            logging::log(
                "APP",
                &format!(
                    "Plugin skills refreshed: {} -> {} available",
                    before_count,
                    self.skills.len()
                ),
            );
        } else {
            logging::log(
                "APP",
                &format!("Plugin skills refreshed: {} available", self.skills.len()),
            );
        }

        cx.notify();
    }

    fn apply_loaded_scripts_and_scriptlets(
        &mut self,
        loaded_scripts: Vec<std::sync::Arc<scripts::Script>>,
        loaded_scriptlets: Vec<std::sync::Arc<scripts::Scriptlet>>,
        cx: &mut Context<Self>,
    ) {
        let before_counts = collect_plugin_inventory_counts(&self.scripts, &self.scriptlets);
        let after_counts = collect_plugin_inventory_counts(&loaded_scripts, &loaded_scriptlets);

        let hotkey_refresh_actions = plan_script_hotkey_refresh(&self.scripts, &loaded_scripts);
        apply_script_hotkey_refresh(&hotkey_refresh_actions);

        // Re-run validation on the freshly loaded catalog so the "Script Issues"
        // launcher row reflects current disk state. Validation is in-memory and
        // cheap compared to file I/O.
        let catalog_report = crate::scripts::validate_script_catalog(loaded_scripts);
        let loaded_scripts: Vec<std::sync::Arc<scripts::Script>> =
            catalog_report.scripts.iter().cloned().collect();
        self.script_validation_report = Some(std::sync::Arc::new(
            crate::scripts::merge_scriptlet_validation_issues(
                &catalog_report.validation,
                &loaded_scriptlets,
            ),
        ));

        self.scripts = loaded_scripts;
        // Use load_scriptlets() to load from all plugins (plugins/*/scriptlets/*.md)
        self.scriptlets = loaded_scriptlets;
        crate::ai::context_selector::publish_launcher_catalog(
            &self.scripts,
            &self.scriptlets,
            &self.skills,
        );
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();
        self.invalidate_preview_cache();

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

        // Sync list component state and validate selection
        // This moves state mutation OUT of render() (anti-pattern fix)
        self.sync_list_state();
        self.selected_index = 0;
        self.validate_selection_bounds(cx);
        self.main_list_state
            .scroll_to_reveal_item(self.selected_index);
        self.last_scrolled_index = Some(self.selected_index);

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
        cx.notify();
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
        self.apps = crate::app_launcher::get_cached_apps();
        self.rebuild_root_windows_after_app_icon_cache_update(
            "refresh_apps_root_windows_icons",
            cx,
        );
        // Invalidate caches so main search includes new apps
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();

        // Sync list component state and validate selection
        // This ensures the GPUI list component knows about the new app count
        self.sync_list_state();
        self.validate_selection_bounds(cx);

        logging::log(
            "APP",
            &format!("Apps refreshed: {} applications loaded", self.apps.len()),
        );
        cx.notify();
    }

    /// Dismiss the bun warning banner
    pub(crate) fn dismiss_bun_warning(&mut self, cx: &mut Context<Self>) {
        logging::log("APP", "Bun warning banner dismissed by user");
        self.show_bun_warning = false;
        cx.notify();
    }

    /// Open bun.sh in the default browser
    pub(crate) fn open_bun_website(&self) {
        logging::log("APP", "Opening https://bun.sh in default browser");
        if let Err(e) = std::process::Command::new("open")
            .arg("https://bun.sh")
            .spawn()
        {
            logging::log("APP", &format!("Failed to open bun.sh: {}", e));
        }
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

        // Parse new scriptlets from file (empty if deleted)
        let new_scripts_scriptlets = if is_deleted {
            vec![]
        } else {
            scripts::read_scriptlets_from_file(path)
        };

        let new_scriptlets: Vec<CachedScriptlet> = new_scripts_scriptlets
            .iter()
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

        // Update the scriptlets list
        // Remove old scriptlets from this file (canonical compare — see
        // scriptlet_file_path_matches for why raw prefix match is not enough)
        self.scriptlets.retain(|s| {
            !s.file_path
                .as_ref()
                .map(|fp| scriptlet_file_path_matches(fp, &changed_raw, &changed_canonical))
                .unwrap_or(false)
        });

        // Add new scriptlets from this file
        self.scriptlets.extend(new_scripts_scriptlets);

        // Sort by name to maintain consistent ordering
        self.scriptlets.sort_by(|a, b| a.name.cmp(&b.name));
        crate::ai::context_selector::publish_launcher_catalog(
            &self.scripts,
            &self.scriptlets,
            &self.skills,
        );

        // Invalidate caches
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();
        self.invalidate_preview_cache();

        // Sync list component state so GPUI renders the correct item count
        self.sync_list_state();
        self.validate_selection_bounds(cx);

        // Rebuild alias/shortcut registries for this file's scriptlets;
        // toast only conflicts that were not already announced.
        let conflicts = self.rebuild_registries();
        for conflict in self.take_unannounced_registry_conflicts(conflicts) {
            self.show_hud(conflict, Some(HUD_CONFLICT_MS), cx);
        }

        logging::log(
            "APP",
            &format!(
                "Scriptlet file updated incrementally: {} now has {} total scriptlets",
                path.display(),
                self.scriptlets.len()
            ),
        );

        cx.notify();
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
                Vec::new()
            },
            move || {
                std::thread::sleep(Duration::from_millis(5));
                *scriptlets_thread_id_clone
                    .lock()
                    .expect("scriptlets thread id lock should succeed") =
                    Some(std::thread::current().id());
                Vec::new()
            },
        );

        let result = rx
            .recv_blocking()
            .expect("background loaders should send exactly one result");

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
