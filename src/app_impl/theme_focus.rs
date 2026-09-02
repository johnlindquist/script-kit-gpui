use super::*;
use tracing::{debug, info};

impl ScriptListApp {
    pub(crate) fn sync_open_actions_dialog_theme(&mut self, cx: &mut Context<Self>) {
        if let Some(ref dialog) = self.actions_dialog {
            let theme_arc = std::sync::Arc::clone(&self.theme);
            dialog.update(cx, |d, _| {
                d.update_theme(theme_arc);
            });
            debug!(target: "APP", "Theme propagated to ActionsDialog");
        }
    }

    pub(crate) fn sync_open_terminal_theme(&mut self, cx: &mut Context<Self>) {
        let theme = std::sync::Arc::clone(&self.theme);
        let terminal = match &self.current_view {
            AppView::TermPrompt { entity, .. } => Some(entity.clone()),
            AppView::QuickTerminalView { entity, .. } => Some(entity.clone()),
            _ => None,
        };

        if let Some(terminal) = terminal {
            terminal.update(cx, |term, _| {
                term.terminal.update_theme(&theme);
            });
            debug!(target: "APP", "Theme propagated to terminal");
        }
    }

    pub(crate) fn refresh_main_menu_theme_layout_metrics(
        &mut self,
        reason: &'static str,
        _cx: &mut Context<Self>,
    ) {
        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        let average_item_height =
            crate::list_item::effective_average_item_height_for_scroll_for_theme(
                self.current_main_menu_theme,
            );
        self.main_list_state = ListState::new(
            self.main_list_state.item_count(),
            ListAlignment::Top,
            px(average_item_height),
        );
        self.main_list_row_generation = self.main_list_row_generation.wrapping_add(1);
        self.last_scrolled_index = None;
        tracing::info!(
            target: "THEME",
            reason,
            theme = self.current_main_menu_theme.name(),
            row_height = self.current_main_menu_theme.def().list.item_height,
            average_item_height,
            row_generation = self.main_list_row_generation,
            "refreshed main menu theme list metrics"
        );
    }

    pub(crate) fn update_theme(&mut self, cx: &mut Context<Self>) {
        let base_theme = theme::get_cached_theme();

        // Preserve opacity offset in light mode, reset in dark mode
        if base_theme.is_dark_mode() {
            self.light_opacity_offset = 0.0;
            self.theme = std::sync::Arc::new(base_theme);
        } else if self.light_opacity_offset != 0.0 {
            // Apply the opacity offset if set
            self.theme =
                std::sync::Arc::new(base_theme.with_opacity_offset(self.light_opacity_offset));
        } else {
            self.theme = std::sync::Arc::new(base_theme);
        }
        self.update_prompt_theme(cx);

        info!(target: "APP", "Theme reloaded based on system appearance");

        // Propagate theme to open ActionsDialog (if any) for hot-reload support
        self.sync_open_actions_dialog_theme(cx);
        self.sync_open_terminal_theme(cx);

        cx.notify();
    }

    pub(crate) fn update_config(&mut self, cx: &mut Context<Self>) {
        self.config = config::load_config();
        clipboard_history::set_max_text_content_len(
            self.config.get_clipboard_history_max_text_length(),
        );
        let secret_rejection = self.config.get_clipboard_history_secret_rejection();
        clipboard_history::configure_secret_rejection(clipboard_history::SecretRejectionConfig {
            extra_blocked_source_apps: secret_rejection.extra_blocked_source_apps,
            extra_secret_patterns: secret_rejection.extra_secret_patterns,
        });
        // Hot-reload hotkeys from updated config
        hotkeys::update_hotkeys(&self.config);
        info!(
            target: "APP",
            padding = ?self.config.get_padding(),
            "Config reloaded"
        );
        if matches!(self.current_view, AppView::ScriptList) {
            let value = self.filter_text.clone();
            let retrieval_changed = self.accept_root_search_input_intent(&value);
            if retrieval_changed || !self.root_search.query_is_current() {
                self.flush_pending_main_menu_query(cx);
            } else {
                self.commit_main_menu_results_refresh("config_reload", None, cx, |app, _cx| {
                    app.invalidate_root_passive_and_grouped_cache();
                    true
                });
            }
        } else {
            cx.notify();
        }
    }

    /// Adjust the light theme opacity by a delta amount
    ///
    /// Use Cmd+Shift+[ to decrease and Cmd+Shift+] to increase.
    /// The offset is clamped to the range -0.5 to +0.5.
    pub(crate) fn adjust_light_opacity(&mut self, delta: f32, cx: &mut Context<Self>) {
        // Only adjust if we're in light mode
        let base_theme = theme::get_cached_theme();
        if base_theme.is_dark_mode() {
            debug!(target: "APP", "Opacity adjustment only works in light mode");
            return;
        }

        // Adjust the offset
        self.light_opacity_offset = (self.light_opacity_offset + delta).clamp(-0.5, 0.5);

        // Create new theme with adjusted opacity
        let adjusted_theme = base_theme.with_opacity_offset(self.light_opacity_offset);
        self.theme = std::sync::Arc::new(adjusted_theme);
        self.update_prompt_theme(cx);
        self.sync_open_terminal_theme(cx);

        let new_opacity = self.theme.get_opacity().main;
        info!(
            target: "APP",
            offset = self.light_opacity_offset,
            main_opacity = new_opacity,
            "Light opacity adjusted"
        );

        // Show toast with current opacity level
        let percent = (new_opacity * 100.0).round() as i32;
        self.toast_manager.push(
            components::toast::Toast::info(format!("Opacity: {}%", percent), &self.theme)
                .duration_ms(Some(TOAST_INFO_MS)),
        );

        cx.notify();
    }

    /// Request focus for a specific target. Focus will be applied once on the
    /// next render when window access is available, then cleared.
    ///
    /// This avoids the "perpetually enforce focus in render()" anti-pattern.
    /// Use this instead of directly calling window.focus() from non-render code.
    #[allow(dead_code)] // Public API for external callers without direct pending_focus access
    pub fn request_focus(&mut self, target: FocusTarget, cx: &mut Context<Self>) {
        self.pending_focus = Some(target);
        cx.notify();
    }

    // === FocusCoordinator Integration Methods ===
    // These methods provide a unified focus management API using the new FocusCoordinator.
    // They exist alongside the old system for gradual migration.

    /// Request focus using the new coordinator system.
    ///
    /// This sets both the coordinator's pending request AND syncs to the old system
    /// for backward compatibility during migration.
    #[allow(dead_code)]
    pub fn focus_via_coordinator(
        &mut self,
        request: focus_coordinator::FocusRequest,
        cx: &mut Context<Self>,
    ) {
        self.focus_coordinator.request(request);
        // Sync to old system for backward compatibility
        self.sync_coordinator_to_legacy();
        cx.notify();
    }

    /// Push an overlay (like actions dialog) with automatic restore on pop.
    ///
    /// Saves current focus state and requests focus to the overlay.
    /// Call `pop_focus_overlay()` when the overlay closes to restore.
    pub fn push_focus_overlay(
        &mut self,
        overlay_request: focus_coordinator::FocusRequest,
        cx: &mut Context<Self>,
    ) {
        self.focus_coordinator.push_overlay(overlay_request);
        // Sync to old system
        self.sync_coordinator_to_legacy();
        cx.notify();
    }

    /// Pop an overlay and restore previous focus state.
    ///
    /// Called when an overlay (actions dialog, shortcut recorder, etc.) closes.
    /// Restores focus to whatever was focused before the overlay opened.
    pub fn pop_focus_overlay(&mut self, cx: &mut Context<Self>) {
        self.focus_coordinator.pop_overlay();
        // Sync to old system
        self.sync_coordinator_to_legacy();
        cx.notify();
    }

    /// Clear all overlays and return to main filter focus.
    ///
    /// Useful for "escape all" or error recovery scenarios.
    #[allow(dead_code)]
    pub fn clear_focus_overlays(&mut self, cx: &mut Context<Self>) {
        self.focus_coordinator.clear_overlays();
        // Sync to old system
        self.sync_coordinator_to_legacy();
        cx.notify();
    }

    /// Get the current cursor owner from the coordinator.
    #[allow(dead_code)]
    pub fn current_cursor_owner(&self) -> focus_coordinator::CursorOwner {
        self.focus_coordinator.cursor_owner()
    }

    /// Sync coordinator state to legacy focused_input/pending_focus fields.
    ///
    /// This bridges the new and old systems during migration.
    pub(crate) fn sync_coordinator_to_legacy(&mut self) {
        // Sync cursor owner to focused_input
        self.focused_input = match self.focus_coordinator.cursor_owner() {
            focus_coordinator::CursorOwner::MainFilter => FocusedInput::MainFilter,
            focus_coordinator::CursorOwner::ActionsSearch => FocusedInput::ActionsSearch,
            focus_coordinator::CursorOwner::ArgPrompt => FocusedInput::ArgPrompt,
            focus_coordinator::CursorOwner::ChatPrompt => FocusedInput::None, // ChatPrompt not in old enum
            focus_coordinator::CursorOwner::None => FocusedInput::None,
        };

        // Sync pending target to pending_focus
        if let Some(request) = self.focus_coordinator.peek_pending() {
            self.pending_focus = Some(match request.target {
                focus_coordinator::FocusTarget::MainFilter => FocusTarget::MainFilter,
                focus_coordinator::FocusTarget::ActionsDialog => FocusTarget::ActionsDialog,
                focus_coordinator::FocusTarget::ArgPrompt => FocusTarget::AppRoot, // ArgPrompt uses AppRoot
                focus_coordinator::FocusTarget::PathPrompt => FocusTarget::PathPrompt,
                focus_coordinator::FocusTarget::FormPrompt => FocusTarget::FormPrompt,
                focus_coordinator::FocusTarget::EditorPrompt => FocusTarget::EditorPrompt,
                focus_coordinator::FocusTarget::SelectPrompt => FocusTarget::SelectPrompt,
                focus_coordinator::FocusTarget::EnvPrompt => FocusTarget::EnvPrompt,
                focus_coordinator::FocusTarget::DropPrompt => FocusTarget::DropPrompt,
                focus_coordinator::FocusTarget::TemplatePrompt => FocusTarget::TemplatePrompt,
                focus_coordinator::FocusTarget::TermPrompt => FocusTarget::TermPrompt,
                focus_coordinator::FocusTarget::ChatPrompt => FocusTarget::ChatPrompt,
                focus_coordinator::FocusTarget::AgentChat => FocusTarget::AgentChat,
                focus_coordinator::FocusTarget::DivPrompt => FocusTarget::AppRoot, // DivPrompt uses AppRoot
                focus_coordinator::FocusTarget::ScratchPad => FocusTarget::EditorPrompt,
                focus_coordinator::FocusTarget::QuickTerminal => FocusTarget::TermPrompt,
            });
        }
    }

    /// Apply pending focus if set. Called at the start of render() when window
    /// is focused. This applies focus exactly once, then clears pending_focus.
    ///
    /// Returns true if focus was applied (for logging/debugging).
    pub(crate) fn apply_pending_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(target) = self.pending_focus.take() else {
            return false;
        };

        // Also consume the coordinator's pending request to keep current_cursor_owner
        // in sync. This is critical for push_overlay/pop_overlay's infer_current_request()
        // to know what was focused before the overlay opened.
        self.focus_coordinator.take_pending();

        debug!(target: "FOCUS", focus_target = ?target, "Applying pending focus");
        match target {
            FocusTarget::MainFilter => {
                let input_state = self.gpui_input_state.clone();
                input_state.update(cx, |state, cx| {
                    state.focus(window, cx);
                });
                self.focused_input = FocusedInput::MainFilter;
            }
            FocusTarget::ActionsDialog => {
                if let Some(ref dialog) = self.actions_dialog {
                    let fh = dialog.read(cx).focus_handle.clone();
                    window.focus(&fh, cx);
                    self.focused_input = FocusedInput::ActionsSearch;
                }
            }
            FocusTarget::EditorPrompt => {
                if let AppView::DayPage { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |view, cx| view.focus_editor(window, cx));
                    self.focused_input = FocusedInput::None;
                } else {
                    let entity = match &self.current_view {
                        AppView::EditorPrompt { entity, .. } => Some(entity),
                        AppView::ScratchPadView { entity, .. } => Some(entity),
                        _ => None,
                    };
                    if let Some(entity) = entity {
                        let fh = entity.read(cx).focus_handle(cx);
                        window.focus(&fh, cx);
                        // EditorPrompt has its own cursor management
                        self.focused_input = FocusedInput::None;
                    }
                }
            }
            FocusTarget::PathPrompt => {
                if let AppView::PathPrompt { focus_handle, .. } = &self.current_view {
                    let fh = focus_handle.clone();
                    window.focus(&fh, cx);
                    // PathPrompt has its own cursor management
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::FormPrompt => {
                if let AppView::FormPrompt { entity, .. } = &self.current_view {
                    let fh = entity.read(cx).focus_handle(cx);
                    window.focus(&fh, cx);
                    // FormPrompt has its own focus handling
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::SelectPrompt => {
                if let AppView::SelectPrompt { entity, .. } = &self.current_view {
                    let fh = entity.read(cx).focus_handle(cx);
                    window.focus(&fh, cx);
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::EnvPrompt => {
                if let AppView::EnvPrompt { entity, .. } = &self.current_view {
                    let fh = entity.read(cx).focus_handle(cx);
                    window.focus(&fh, cx);
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::DropPrompt => {
                if let AppView::DropPrompt { entity, .. } = &self.current_view {
                    let fh = entity.read(cx).focus_handle(cx);
                    window.focus(&fh, cx);
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::TemplatePrompt => {
                if let AppView::TemplatePrompt { entity, .. } = &self.current_view {
                    let fh = entity.read(cx).focus_handle(cx);
                    window.focus(&fh, cx);
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::TermPrompt => {
                let entity = match &self.current_view {
                    AppView::TermPrompt { entity, .. } => Some(entity),
                    AppView::QuickTerminalView { entity, .. } => Some(entity),
                    _ => None,
                };
                if let Some(entity) = entity {
                    let fh = entity.read(cx).focus_handle.clone();
                    window.focus(&fh, cx);
                    // Terminal handles its own cursor
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::ChatPrompt => {
                let entity = match &self.current_view {
                    AppView::ChatPrompt { entity, .. } => Some(entity.read(cx).focus_handle(cx)),
                    AppView::AgentChatView { .. } => self.embedded_agent_chat_focus_handle.clone(),
                    _ => None,
                };
                if let Some(fh) = entity {
                    window.focus(&fh, cx);
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::AgentChat => {
                if matches!(self.current_view, AppView::AgentChatView { .. }) {
                    let Some(fh) = self.embedded_agent_chat_focus_handle.clone() else {
                        return false;
                    };
                    window.focus(&fh, cx);
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::NamingPrompt => {
                if let AppView::NamingPrompt { entity, .. } = &self.current_view {
                    let fh = entity.read(cx).focus_handle(cx);
                    window.focus(&fh, cx);
                    self.focused_input = FocusedInput::None;
                }
            }
            FocusTarget::AppRoot => {
                window.focus(&self.focus_handle, cx);
                // Don't reset focused_input here - the caller already set it appropriately.
                // For example, ArgPrompt sets focused_input = ArgPrompt before setting
                // pending_focus = AppRoot, and we want to preserve that so the cursor blinks.
            }
        }

        true
    }
}

#[cfg(test)]
mod focus_restore_regression_tests {
    use std::fs;

    #[test]
    fn apply_pending_focus_restores_launcher_agent_chat_via_dedicated_target() {
        let source = fs::read_to_string("src/app_impl/theme_focus.rs")
            .expect("Failed to read src/app_impl/theme_focus.rs");

        assert!(
            source.contains("focus_coordinator::FocusTarget::AgentChat => FocusTarget::AgentChat"),
            "coordinator sync should preserve the AgentChat target through the legacy bridge"
        );
        assert!(
            source
                .contains("AppView::AgentChatView { .. } => self.embedded_agent_chat_focus_handle.clone()")
                && source.contains("FocusTarget::AgentChat => {")
                && source.contains("matches!(self.current_view, AppView::AgentChatView { .. })"),
            "launcher Agent Chat focus should work through cached focus handles for both the legacy ChatPrompt compatibility path and the dedicated AgentChat target"
        );
        assert!(
            source.contains("FocusTarget::AgentChat => {")
                && source.contains("self.embedded_agent_chat_focus_handle.clone()"),
            "apply_pending_focus should restore launcher Agent Chat via the cached AgentChatView focus handle"
        );
    }
}
