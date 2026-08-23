//! Shared stdin `triggerBuiltin` dispatcher.
//!
//! Collapses the three previously-duplicated match arms (app_run_setup.rs
//! and the two orphan `runtime_stdin*.rs` audit targets) into one
//! compiler-enforced exhaustive dispatch keyed by the canonical
//! [`TriggerBuiltin`] enum.
//!
//! All unknown-name bookkeeping lives here — the `PROTOCOL_STATS`
//! counter, the rate-limited `tracing::warn!`, the payload cap — so
//! regressions can't re-introduce O(payload) log spam by editing one
//! call site and forgetting the others.

use super::routes::{plan_trigger_builtin_route, AppRoute, FilterableView};
use super::*;
use crate::builtins::trigger_registry::{registry as trigger_registry, TriggerBuiltin};
use crate::protocol_stats::{self, PROTOCOL_STATS};
use crate::stdin_commands::{BuiltinRef, ExternalCommand};

#[derive(Debug)]
enum FilterableRouteState {
    Start(FilterableView),
    Prepared(Box<FilterableRoutePlan>),
    Failed {
        view: FilterableView,
        reason: String,
    },
    Applied {
        surface_kind: SurfaceKind,
    },
}

#[derive(Debug)]
struct FilterableRoutePlan {
    next_view: AppView,
    reset_shared_filter: bool,
    pending_placeholder: Option<&'static str>,
    pending_focus: Option<FocusTarget>,
    clear_hover: bool,
    resize: bool,
}

/// Oracle-Session `logging-observability-next-pass` PR1 migrated the
/// three log sites below off the ad-hoc `UNKNOWN_NAME_PREVIEW_CHAR_LIMIT`
/// and `chars().take(N)` preview. Every user-value preview now flows
/// through [`logging::log_user_value`] (byte-capped + UTF-8-safe), and
/// every site is gated by [`logging::log_rate_limit`] on
/// `(category, key)` so same-name bursts cannot leak O(untrusted-input)
/// warn lines even when `protocol_stats::should_log_occurrence` would
/// have let them through.
impl ScriptListApp {
    /// Normalize an `ExternalCommand::TriggerBuiltin` payload via
    /// [`ExternalCommand::trigger_builtin_ref`] and dispatch.
    ///
    /// * `BuiltinRef::CanonicalId(id)` — resolved via registry command-id.
    /// * `BuiltinRef::LegacyAlias(name)` — resolved via legacy alias and
    ///   bumps `trigger_builtin_deprecated_name_total`. A rate-limited
    ///   `deprecated_name` warn fires on the 1st / 100th occurrence so
    ///   operators can see drift without log-spam.
    /// * `Err(_)` (both fields set / neither set) — structured warn,
    ///   `trigger_builtin_unknown_total` bump, no-op return.
    ///
    /// Returns `None` for non-`TriggerBuiltin` variants (caller mismatch).
    pub fn dispatch_trigger_builtin(
        &mut self,
        cmd: &ExternalCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<TriggerBuiltin> {
        match cmd.trigger_builtin_ref() {
            Ok(Some(BuiltinRef::CanonicalId(id))) => {
                self.dispatch_trigger_builtin_name(id, window, cx)
            }
            Ok(Some(BuiltinRef::LegacyAlias(name))) => {
                self.log_deprecated_trigger_builtin_name(name);
                self.dispatch_trigger_builtin_name(name, window, cx)
            }
            Ok(None) => None,
            Err(reason) => {
                self.log_invalid_trigger_builtin(&reason);
                None
            }
        }
    }

    /// Resolve and execute a stdin `triggerBuiltin` name.
    ///
    /// Returns `Some(TriggerBuiltin)` when the name resolves to a known
    /// canonical built-in (via command-id or legacy alias), `None` when
    /// the name is unknown — in which case a single structured,
    /// payload-capped, rate-limited `warn` is emitted and the
    /// `trigger_builtin_unknown_total` counter is incremented.
    ///
    /// The caller retains the invariant that the automation semantic
    /// surface must be refreshed afterwards by calling
    /// [`Self::rekey_main_automation_surface_after_trigger_builtin_dispatch`].
    /// Keeping that as a named post-dispatch step lets the three stdin
    /// call sites expose the same contract without recomputing window
    /// metadata they do not own.
    pub fn dispatch_trigger_builtin_name(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<TriggerBuiltin> {
        // Opened via protocol command — ESC must close the window, NOT
        // return to the main menu. This is identical across all three
        // previous call sites and must stay that way. Preserve the
        // reset even on unknown names, which the original
        // string-entry path already did.
        self.mark_opened_directly("protocol");

        let Some(resolved) = trigger_registry().resolve(name) else {
            self.log_unknown_trigger_builtin(name);
            return None;
        };

        Some(self.dispatch_trigger_builtin_enum(resolved, window, cx))
    }

    /// Typed entry point for `triggerBuiltin` dispatch.
    ///
    /// Callers that already hold a resolved [`TriggerBuiltin`] (e.g.
    /// via the pure resolver in
    /// [`crate::builtins::trigger_resolve::resolve_trigger_builtin`])
    /// should prefer this over [`Self::dispatch_trigger_builtin_name`]
    /// so the registry lookup isn't paid twice. Oracle-Session
    /// `protocol-builtin-boundary-engineering-plan` Pass 4 introduced
    /// this sub-pass of rank #3 — "Move triggerBuiltin string
    /// resolution into ingress; accept enum only at dispatch." The
    /// full migration will move the resolver into ingress and retire
    /// the string entry point; today this method is the typed entry
    /// point without changing the existing behavior.
    pub fn dispatch_trigger_builtin_enum(
        &mut self,
        id: TriggerBuiltin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TriggerBuiltin {
        self.mark_opened_directly("protocol");
        self.apply_trigger_builtin(id, window, cx);
        id
    }

    /// Exhaustive dispatch for a resolved [`TriggerBuiltin`]. Drives
    /// the side-effect half from the pure planner in `super::routes` —
    /// every branch of the outer match is produced by
    /// [`plan_trigger_builtin_route`], and every `FilterableView` arm
    /// below is reachable from some `TriggerBuiltin` (pinned by tests
    /// `every_filterable_view_is_reachable` and
    /// `non_filterable_routes_are_one_to_one` in `routes.rs`). There
    /// is no wildcard catch-all on either level, so adding a new
    /// `TriggerBuiltin` variant forces a matching planner arm AND a
    /// matching `FilterableView` / `AppRoute` arm here.
    fn apply_trigger_builtin(
        &mut self,
        id: TriggerBuiltin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match plan_trigger_builtin_route(id) {
            AppRoute::ShowFilterableView(view) => self.show_filterable_view(view, window, cx),
            AppRoute::OpenFileSearch => {
                self.open_file_search(String::new(), cx);
            }
            AppRoute::OpenTabAi => {
                self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
            }
            AppRoute::OpenCurrentAppCommands => {
                if let Err(e) = self.open_current_app_commands_from_tray(cx) {
                    logging::log(
                        "ERROR",
                        &format!("triggerBuiltin current-app-commands failed: {e}"),
                    );
                }
                self.update_window_size_deferred(window, cx);
            }
            AppRoute::ExecuteBuiltin(command_id) => {
                if let Some(entry) = self
                    .builtin_entries
                    .iter()
                    .find(|entry| entry.id == command_id)
                    .cloned()
                {
                    self.execute_builtin(&entry, cx);
                } else {
                    tracing::warn!(
                        target: "script_kit::trigger_builtin",
                        command_id,
                        "triggerBuiltin ExecuteBuiltin route missing launcher entry"
                    );
                }
            }
        }
    }

    /// Re-key the main window's automation `semanticSurface` after a
    /// `triggerBuiltin` dispatch mutates `current_view`.
    ///
    /// This is intentionally a post-dispatch method: the route planner
    /// and view mutation happen first, then the automation registry is
    /// updated from the now-current `AppView::surface_contract()` tag.
    /// Callers must not upsert the whole window here because the stdin
    /// dispatchers do not own the latest bounds, focus, or title.
    pub(crate) fn rekey_main_automation_surface_after_trigger_builtin_dispatch(&self) -> bool {
        self.rekey_main_automation_surface_from_current_view()
    }

    /// Entry point for the imperative half of [`AppRoute::ShowFilterableView`].
    ///
    /// The route-entry state machine owns cache preloads, shared-filter reset,
    /// focus/hover cleanup, `current_view` assignment, and deferred resizing.
    /// Keeping that ordered transition in one owner makes this path auditable
    /// without moving triggerBuiltin's post-dispatch automation re-key.
    fn show_filterable_view(
        &mut self,
        view: FilterableView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.run_filterable_route_state_machine(view, window, cx) {
            FilterableRouteState::Applied { surface_kind } => {
                tracing::trace!(
                    target: "script_kit::trigger_builtin",
                    ?surface_kind,
                    "filterable route applied"
                );
            }
            FilterableRouteState::Failed { view, reason } => {
                tracing::debug!(
                    target: "script_kit::trigger_builtin",
                    ?view,
                    reason,
                    "filterable route failed"
                );
            }
            FilterableRouteState::Start(_) | FilterableRouteState::Prepared(_) => {
                unreachable!("filterable route state machine must return a terminal state")
            }
        }
    }

    fn run_filterable_route_state_machine(
        &mut self,
        view: FilterableView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FilterableRouteState {
        let mut state = FilterableRouteState::Start(view);
        loop {
            state = match state {
                FilterableRouteState::Start(view) => match self.prepare_filterable_route(view) {
                    Ok(plan) => FilterableRouteState::Prepared(Box::new(plan)),
                    Err(reason) => FilterableRouteState::Failed { view, reason },
                },
                FilterableRouteState::Prepared(plan) => {
                    self.apply_filterable_route_plan(*plan, window, cx)
                }
                terminal @ FilterableRouteState::Failed { .. }
                | terminal @ FilterableRouteState::Applied { .. } => return terminal,
            };
        }
    }

    fn prepare_filterable_route(
        &mut self,
        view: FilterableView,
    ) -> Result<FilterableRoutePlan, String> {
        let plan = match view {
            FilterableView::ClipboardHistory => {
                self.cached_clipboard_entries = crate::clipboard_history::get_cached_entries(100);
                self.focused_clipboard_entry_id = self
                    .cached_clipboard_entries
                    .first()
                    .map(|entry| entry.id.clone());
                FilterableRoutePlan {
                    next_view: AppView::ClipboardHistoryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    reset_shared_filter: false,
                    pending_placeholder: None,
                    pending_focus: Some(FocusTarget::MainFilter),
                    clear_hover: false,
                    resize: true,
                }
            }
            FilterableView::AppLauncher => FilterableRoutePlan {
                next_view: AppView::AppLauncherView {
                    filter: String::new(),
                    selected_index: 0,
                },
                reset_shared_filter: false,
                pending_placeholder: None,
                pending_focus: Some(FocusTarget::MainFilter),
                clear_hover: false,
                resize: true,
            },
            FilterableView::BrowserTabs => match crate::browser_tabs::list_open_tabs() {
                Ok(tabs) => {
                    self.cached_browser_tabs = tabs;
                    FilterableRoutePlan {
                        next_view: AppView::BrowserTabsView {
                            filter: String::new(),
                            selected_index: 0,
                        },
                        reset_shared_filter: true,
                        pending_placeholder: Some("Search open browser tabs..."),
                        pending_focus: Some(FocusTarget::MainFilter),
                        clear_hover: true,
                        resize: true,
                    }
                }
                Err(error) => {
                    let reason = format!("Failed to list browser tabs: {error}");
                    logging::log("ERROR", &reason);
                    return Err(reason);
                }
            },
            FilterableView::EmojiPicker => FilterableRoutePlan {
                next_view: AppView::EmojiPickerView {
                    filter: String::new(),
                    selected_index: 0,
                    selected_category: None,
                },
                reset_shared_filter: true,
                pending_placeholder: Some("Search Emoji & Symbols..."),
                pending_focus: Some(FocusTarget::MainFilter),
                clear_hover: true,
                resize: true,
            },
            FilterableView::WindowSwitcher => match crate::window_control::list_windows() {
                Ok(windows) => {
                    self.cached_windows = windows;
                    FilterableRoutePlan {
                        next_view: AppView::WindowSwitcherView {
                            filter: String::new(),
                            selected_index: 0,
                        },
                        reset_shared_filter: true,
                        pending_placeholder: Some("Search windows..."),
                        pending_focus: Some(FocusTarget::MainFilter),
                        clear_hover: true,
                        resize: true,
                    }
                }
                Err(error) => {
                    let reason = format!("Failed to list windows: {error}");
                    logging::log("ERROR", &reason);
                    return Err(reason);
                }
            },
            FilterableView::ProcessManager => {
                self.cached_processes =
                    crate::process_manager::PROCESS_MANAGER.get_active_processes_sorted();
                FilterableRoutePlan {
                    next_view: AppView::ProcessManagerView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    reset_shared_filter: true,
                    pending_placeholder: Some("Search running scripts..."),
                    pending_focus: Some(FocusTarget::MainFilter),
                    clear_hover: true,
                    resize: true,
                }
            }
            FilterableView::KitStoreBrowse => FilterableRoutePlan {
                next_view: AppView::BrowseKitsView {
                    query: String::new(),
                    selected_index: 0,
                    results: Vec::new(),
                },
                reset_shared_filter: true,
                pending_placeholder: Some("Search Kit Store..."),
                pending_focus: Some(FocusTarget::MainFilter),
                clear_hover: true,
                resize: true,
            },
            FilterableView::KitStoreInstalled => FilterableRoutePlan {
                next_view: AppView::InstalledKitsView {
                    filter: String::new(),
                    selected_index: 0,
                    kits: Self::kit_store_list_installed(),
                },
                reset_shared_filter: true,
                pending_placeholder: Some("Search installed kits..."),
                pending_focus: Some(FocusTarget::MainFilter),
                clear_hover: true,
                resize: true,
            },
            FilterableView::Favorites => FilterableRoutePlan {
                next_view: AppView::FavoritesBrowseView {
                    filter: String::new(),
                    selected_index: 0,
                },
                reset_shared_filter: true,
                pending_placeholder: Some("Search favorites..."),
                pending_focus: Some(FocusTarget::MainFilter),
                clear_hover: true,
                resize: true,
            },
            FilterableView::SearchAiPresets => FilterableRoutePlan {
                next_view: AppView::SearchAiPresetsView {
                    filter: String::new(),
                    selected_index: 0,
                },
                reset_shared_filter: true,
                pending_placeholder: Some("Search AI presets..."),
                pending_focus: Some(FocusTarget::MainFilter),
                clear_hover: true,
                resize: true,
            },
        };
        Ok(plan)
    }

    fn apply_filterable_route_plan(
        &mut self,
        plan: FilterableRoutePlan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FilterableRouteState {
        let resets_uniform_list = match &plan.next_view {
            AppView::ClipboardHistoryView { .. } => {
                self.clipboard_list_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top);
                true
            }
            AppView::ProcessManagerView { .. } => {
                self.process_list_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top);
                true
            }
            AppView::AppLauncherView { .. }
            | AppView::BrowseKitsView { .. }
            | AppView::InstalledKitsView { .. } => {
                self.list_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top);
                true
            }
            _ => false,
        };
        if resets_uniform_list {
            self.hovered_index = None;
            self.list_suppress_hover_until_pointer_move = true;
            self.last_list_interaction_source =
                crate::scrolling::list_interaction::ListViewportInputSource::Filter;
        }
        if plan.reset_shared_filter {
            self.filter_text = String::new();
            self.pending_filter_sync = true;
        }
        if let Some(placeholder) = plan.pending_placeholder {
            self.pending_placeholder = Some(placeholder.to_string());
        }
        if plan.clear_hover {
            self.hovered_index = None;
        }
        if let Some(focus) = plan.pending_focus {
            self.restore_current_view_with_focus(plan.next_view, focus);
        } else {
            self.current_view = plan.next_view;
        }
        self.builtin_row_stack_scroll_handle.scroll_to_item(0);
        let surface_kind = self.current_view.surface_kind();
        if plan.resize {
            self.update_window_size_deferred(window, cx);
        }

        FilterableRouteState::Applied { surface_kind }
    }

    /// Rate-limited, byte-capped log for the unknown-name no-op path.
    /// Extracted so all three ingress points share the same semantics —
    /// no more "120 here, 256 there, raw %name elsewhere" drift.
    fn log_unknown_trigger_builtin(&self, name: &str) {
        let total = protocol_stats::increment(&PROTOCOL_STATS.trigger_builtin_unknown_total);
        let rate = logging::log_rate_limit("trigger_builtin_unknown", name);
        if !rate.emit {
            return;
        }
        let name_safe = logging::log_user_value(name);
        tracing::warn!(
            category = "STDIN",
            event_type = "trigger_builtin_unknown",
            name_preview = %name_safe,
            name_bytes = name_safe.raw_bytes,
            name_safe_bytes = name_safe.safe_bytes,
            name_truncated = name_safe.truncated,
            suppressed = rate.suppressed,
            occurrences_total = total,
            "triggerBuiltin unknown name — dispatch no-op"
        );
    }

    /// Rate-limited warn for the deprecated legacy-`name` ingress. The
    /// dispatch still runs (legacy aliases still resolve), but we bump
    /// `trigger_builtin_deprecated_name_total` and — per Oracle-Session
    /// `logging-observability-next-pass` PR1 — the emit is gated by the
    /// shared `(category, key)` time window so a stuck client cannot
    /// produce back-to-back warns for the same legacy alias.
    fn log_deprecated_trigger_builtin_name(&self, name: &str) {
        let total =
            protocol_stats::increment(&PROTOCOL_STATS.trigger_builtin_deprecated_name_total);
        let rate = logging::log_rate_limit("trigger_builtin_deprecated_name", name);
        if !rate.emit {
            return;
        }
        let name_safe = logging::log_user_value(name);
        tracing::warn!(
            category = "STDIN",
            event_type = "trigger_builtin_deprecated_name",
            name_preview = %name_safe,
            name_bytes = name_safe.raw_bytes,
            name_safe_bytes = name_safe.safe_bytes,
            name_truncated = name_safe.truncated,
            suppressed = rate.suppressed,
            occurrences_total = total,
            "triggerBuiltin legacy `name` field — migrate to `builtinId`"
        );
    }

    /// Rate-limited warn for a `TriggerBuiltin` payload that cannot be
    /// normalized — both `builtinId` and `name` present, or neither.
    /// Bumps `trigger_builtin_unknown_total` so we don't need a fourth
    /// counter just for this edge case; the `event_type` field
    /// discriminates in log queries.
    fn log_invalid_trigger_builtin(&self, reason: &str) {
        let total = protocol_stats::increment(&PROTOCOL_STATS.trigger_builtin_unknown_total);
        let rate = logging::log_rate_limit("trigger_builtin_invalid_payload", reason);
        if !rate.emit {
            return;
        }
        let reason_safe = logging::log_user_value(reason);
        tracing::warn!(
            category = "STDIN",
            event_type = "trigger_builtin_invalid_payload",
            reason_preview = %reason_safe,
            reason_bytes = reason_safe.raw_bytes,
            reason_safe_bytes = reason_safe.safe_bytes,
            reason_truncated = reason_safe.truncated,
            suppressed = rate.suppressed,
            occurrences_total = total,
            "triggerBuiltin payload rejected — dispatch no-op"
        );
    }
}
