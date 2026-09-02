use super::*;

/// Who owns Escape for a `handle_global_shortcut_with_options` call site.
///
/// Callers may not pass a raw bool: Escape-closes-the-window is derived from
/// the one [`DismissPolicy`] table (`AppView::dismiss_policy`), so per-surface
/// escape behavior cannot drift renderer by renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlobalShortcutEscape {
    /// Escape derives from `current_view.dismiss_policy()` — the normal case.
    FromDismissPolicy,
    /// The caller implements its own Escape ladder (ScriptList) and this
    /// handler must leave Escape alone.
    CallerOwned,
}

impl ScriptListApp {
    pub(crate) fn handle_global_shortcut_with_options(
        &mut self,
        event: &gpui::KeyDownEvent,
        escape: GlobalShortcutEscape,
        cx: &mut Context<Self>,
    ) -> bool {
        let is_dismissable = match escape {
            GlobalShortcutEscape::CallerOwned => false,
            GlobalShortcutEscape::FromDismissPolicy => self
                .current_view
                .dismiss_policy()
                .closes_main_window_on(DismissTrigger::Escape),
        };
        // If the shortcut recorder is active, don't process any shortcuts here.
        // The recorder has its own key handlers and should receive all key events.
        if self.shortcut_recorder_state.is_some() {
            return false;
        }

        let key_str = event.keystroke.key.as_str();
        let has_cmd = event.keystroke.modifiers.platform;
        let has_shift = event.keystroke.modifiers.shift;

        // Cmd+W always closes window
        if has_cmd && key_str.eq_ignore_ascii_case("w") {
            logging::log("KEY", "Cmd+W - closing window");
            self.close_and_reset_window(cx);
            return true;
        }

        // Cmd+Shift+M cycles vibrancy material (for debugging)
        if has_cmd && has_shift && key_str.eq_ignore_ascii_case("m") {
            let result = crate::platform::cycle_vibrancy_material();
            logging::log("KEY", &format!("Cmd+Shift+M - {}", result));
            // Show HUD with the material name
            self.show_hud(result, None, cx);
            return true;
        }

        // Cmd+Shift+P toggles pin mode (window stays open on blur)
        if has_cmd && has_shift && key_str.eq_ignore_ascii_case("p") {
            self.is_pinned = !self.is_pinned;
            let status = if self.is_pinned {
                "📌 Window Pinned"
            } else {
                "📌 Window Unpinned"
            };
            logging::log("KEY", &format!("Cmd+Shift+P - {}", status));
            self.show_hud(status.to_string(), None, cx);
            cx.notify();
            return true;
        }

        // ESC closes dismissable prompts (when actions popup is not showing)
        if is_dismissable
            && crate::ui_foundation::is_key_escape(key_str)
            && !self.show_actions_popup
        {
            logging::log("KEY", "ESC in dismissable prompt - closing window");
            self.close_and_reset_window(cx);
            return true;
        }

        false
    }

    /// Check if the current view is a dismissable prompt.
    ///
    /// Per-view dismiss behavior is owned by [`AppView::dismiss_policy`] in
    /// `src/main_sections/app_view_state.rs` — rustc exhaustiveness there
    /// is the contract. This wrapper stays for callers that only care about
    /// the `WindowBlur` click-outside path. Oracle-Session
    /// `shortcuts-hud-grid-dismiss-logic`.
    #[allow(dead_code)]
    pub(crate) fn is_dismissable_view(&self) -> bool {
        self.current_view
            .dismiss_policy()
            .closes_main_window_on(DismissTrigger::WindowBlur)
    }

    /// Show a HUD (heads-up display) overlay message
    ///
    /// This creates a separate floating window positioned at bottom-center of the
    /// screen containing the mouse cursor. The HUD is independent of the main
    /// Script Kit window and will remain visible even when the main window is hidden.
    ///
    /// Position: Bottom-center (85% down screen)
    /// Duration: 2000ms default, configurable
    /// Shape: Pill (40px tall, variable width)
    pub(crate) fn show_hud(
        &mut self,
        text: String,
        duration_ms: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        // Delegate to the HUD manager which creates a separate floating window
        // This ensures the HUD is visible even when the main app window is hidden
        hud_manager::show_hud(text, duration_ms, cx);
    }

    /// Show the debug grid overlay with specified options
    ///
    /// This method converts protocol::GridOptions to debug_grid::GridConfig
    /// and enables the grid overlay rendering.
    pub(crate) fn show_grid(&mut self, options: protocol::GridOptions, cx: &mut Context<Self>) {
        use debug_grid::{GridColorScheme, GridConfig, GridDepth};
        use protocol::GridDepthOption;

        // Convert protocol depth to debug_grid depth
        let depth = match &options.depth {
            GridDepthOption::Preset(s) if s == "all" => GridDepth::All,
            GridDepthOption::Preset(_) => GridDepth::Prompts,
            GridDepthOption::Components(names) => GridDepth::Components(names.clone()),
        };

        self.grid_config = Some(GridConfig {
            grid_size: options.grid_size,
            show_bounds: options.show_bounds,
            show_box_model: options.show_box_model,
            show_alignment_guides: options.show_alignment_guides,
            show_dimensions: options.show_dimensions,
            depth,
            color_scheme: GridColorScheme::default(),
        });
        self.mark_main_data_changed();
        self.mark_main_presentation_changed();

        logging::log(
            "DEBUG_GRID",
            &format!(
                "Grid overlay enabled: size={}, bounds={}, box_model={}, guides={}, dimensions={}",
                options.grid_size,
                options.show_bounds,
                options.show_box_model,
                options.show_alignment_guides,
                options.show_dimensions
            ),
        );

        cx.notify();
    }

    /// Hide the debug grid overlay
    pub(crate) fn hide_grid(&mut self, cx: &mut Context<Self>) {
        if self.grid_config.is_none() { return; }
        self.grid_config = None;
        self.mark_main_data_changed();
        self.mark_main_presentation_changed();
        logging::log("DEBUG_GRID", "Grid overlay hidden");
        cx.notify();
    }
}

#[cfg(test)]
#[gpui::test]
fn grid_overlay_open_close_advance_source_revisions(cx: &mut gpui::TestAppContext) {
    let app = main_menu_selection_test_app(cx);
    app.update(cx, |app, cx| {
        let before = app.owned_revision_facts();
        app.show_grid(crate::protocol::GridOptions::default(), cx);
        let shown = app.owned_revision_facts();
        assert!(shown.data_generation > before.data_generation);
        assert!(shown.presentation_revision > before.presentation_revision);
        assert_eq!(app.owned_revision_facts(), shown);
        app.hide_grid(cx);
        let hidden = app.owned_revision_facts();
        assert!(hidden.presentation_revision > shown.presentation_revision);
        app.hide_grid(cx);
        assert_eq!(app.owned_revision_facts(), hidden);
    });
}
