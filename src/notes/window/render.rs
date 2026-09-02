use super::*;

impl NotesApp {
    /// Keep the automation registry's Notes bounds current for ANY frame
    /// change — native AppKit-tracked resizes never route through the custom
    /// bottom-resize observation, and stale automation bounds would poison
    /// every layout-based runtime proof.
    pub(super) fn sync_automation_bounds(&mut self, window: &Window) {
        let bounds = window.bounds();
        let tuple = [
            bounds.origin.x.as_f32(),
            bounds.origin.y.as_f32(),
            bounds.size.width.as_f32(),
            bounds.size.height.as_f32(),
        ];
        if self.last_automation_synced_bounds == Some(tuple) {
            return;
        }
        let Some(generation) = self.automation_generation else {
            return;
        };
        let updated = crate::windows::set_automation_bounds_if_generation(
            "notes",
            generation,
            Some(crate::protocol::AutomationWindowBounds {
                x: f64::from(tuple[0]),
                y: f64::from(tuple[1]),
                width: f64::from(tuple[2]),
                height: f64::from(tuple[3]),
            }),
        );
        if updated {
            self.last_automation_synced_bounds = Some(tuple);
        }
    }

    fn process_render_side_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_bottom_resize_observation(window);
        self.detect_manual_resize(window);
        self.sync_automation_bounds(window);
        self.drain_pending_focus(window, cx);
        self.maybe_update_theme_cache(cx);
        self.maybe_persist_bounds(window);

        if self.should_save_now() {
            tracing::debug!(
                surface = "notes_window",
                action = "autosave",
                has_selected_note = self.selected_note_id.is_some(),
                has_unsaved_changes = self.has_unsaved_changes,
                show_actions_panel = self.command_bar.is_open(),
                show_search = self.show_search,
                preview_enabled = self.preview_enabled,
                focus_mode = self.focus_mode,
                "ui_render_decision"
            );
            self.save_current_note();
        }

        // A day-page save that merged in concurrent external captures leaves the
        // merged text here; push it into the editor so those lines are visible
        // and the next keystroke doesn't overwrite them.
        if let Some(merged) = self.pending_day_editor_reconcile.take() {
            self.editor_state.update(cx, |state, cx| {
                let len = merged.len();
                state.set_value(&merged, window, cx);
                state.set_selection(len, len, window, cx);
            });
        }
    }
}

impl Render for NotesApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mouse_cursor_hidden = self.mouse_cursor_hidden;
        let body_visible = self.entry_reveal.body_visible;
        let body_reveal_generation = self.entry_reveal.generation;

        self.process_render_side_effects(window, cx);

        let theme = crate::theme::get_cached_theme();
        let vibrancy_bg = crate::ui_foundation::get_vibrancy_background(&theme);
        let theme_background_gradients =
            crate::ui_foundation::theme_background_gradient_layers("notes-bg-layer", &theme);

        let glass_active = crate::footer_popup::glass_scroll_bands_active();

        let content = self
            .render_editor(body_visible, body_reveal_generation, cx)
            .into_any_element();

        // Mention-preview hint, migrated from the removed Notes footer rail:
        // a transient bottom-right overlay that reserves no layout height and
        // appears only while the cursor sits on an inline `@` token.
        let mention_overlay = self
            .focused_note_mention_preview(cx)
            .map(|(token, detail)| {
                div()
                    .id("notes-mention-preview-overlay")
                    .absolute()
                    .bottom_2()
                    .right_2()
                    .max_w_full()
                    .overflow_hidden()
                    .text_xs()
                    .whitespace_nowrap()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{token} · {detail}"))
                    .into_any_element()
            });

        // The Notes content stage always fills the Notes window.
        let stage = div()
            .id("notes-window-content-stage")
            .relative()
            .w_full()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .overflow_hidden()
            // Full-window glass: the stage must match the rounded native
            // backdrop or square content corners poke out.
            .when(glass_active, |d| {
                d.rounded(px(crate::ui::chrome::LIQUID_GLASS_WINDOW_RADIUS_PX))
            })
            .when_some(vibrancy_bg, |d, bg| d.bg(bg))
            .children(theme_background_gradients)
            .child(content)
            .children(mention_overlay)
            .into_any_element();

        div()
            .id("notes-window-root")
            .debug_selector(|| "notes-window-root".to_string())
            .flex()
            .flex_col()
            .size_full()
            .relative()
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus_handle)
            .when(mouse_cursor_hidden, |d| d.cursor(CursorStyle::None))
            .on_any_mouse_down(cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                // The custom bottom-edge resize handoff is intentionally NOT
                // wired here anymore: the 2026-07-26 notes-live-resize probe
                // proved AppKit's native resizable frame tracks all eight
                // edges/corners of this NSPanel (receipt:
                // .artifacts/notes-live-resize/final). The classifier and its
                // receipts remain in resize.rs as the documented fallback.
                if this.command_bar.is_open() {
                    this.close_actions_panel(window, cx);
                }
                if this.note_switcher.is_open() {
                    this.close_browse_panel(window, cx);
                }
                if confirm::is_confirm_window_open() {
                    confirm::route_key_to_confirm_popup("escape", cx);
                }
            }))
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if this.force_hovered {
                    return;
                }

                this.window_hovered = *hovered;
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _, cx| {
                this.show_mouse_cursor(cx);
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .child(stage)
            .children(gpui_component::Root::render_dialog_layer(window, cx))
            .children(gpui_component::Root::render_notification_layer(window, cx))
    }
}
