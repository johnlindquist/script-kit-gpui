use super::*;

impl NotesApp {
    /// Render the Agent Chat surface with a thin Notes-owned titlebar containing a
    /// mode switch so the user can toggle back to the Notes editor.
    pub(super) fn render_agent_chat_surface(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted_color = cx.theme().muted_foreground;
        let accent_color = cx.theme().accent;
        let window_hovered = self.window_hovered || self.force_hovered;
        let metrics = style::adopted_metrics();

        let titlebar = div()
            .id("notes-agent_chat-titlebar")
            .flex()
            .items_center()
            .h(px(metrics.titlebar_height))
            .px_3()
            // Traffic light padding on the left.
            .child(div().w(px(TITLEBAR_TRAFFIC_LIGHT_W)).flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    // Notes/Agent switch — two clickable labels.
                    .child(
                        div()
                            .id("notes-switch-notes")
                            .cursor_pointer()
                            .text_sm()
                            .text_color(muted_color.opacity(OPACITY_MUTED))
                            .hover(|s| s.text_color(accent_color))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.switch_to_notes_surface(window, cx);
                            }))
                            .child("Notes"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted_color.opacity(OPACITY_SUBTLE))
                            .child("/"),
                    )
                    .child(
                        div()
                            .id("notes-switch-agent_chat")
                            .text_sm()
                            .text_color(accent_color)
                            .child("Agent"),
                    ),
            )
            .child(
                div()
                    .w(px(TITLEBAR_ICONS_W))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .when(window_hovered, |d| {
                        d.child(
                            div()
                                .id("agent_chat-titlebar-actions-icon")
                                .min_w(px(MIN_TARGET_SIZE))
                                .min_h(px(MIN_TARGET_SIZE))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_sm()
                                .text_color(muted_color.opacity(OPACITY_MUTED))
                                .cursor_pointer()
                                .hover(|s| s.text_color(muted_color))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_agent_chat_actions(window, cx);
                                }))
                                .child("⌘"),
                        )
                    }),
            );

        let agent_chat_body = if let Some(ref agent_chat_entity) = self.embedded_agent_chat {
            div()
                .flex_1()
                .min_h(px(0.))
                .child(agent_chat_entity.clone())
                .into_any_element()
        } else {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted_color.opacity(OPACITY_MUTED))
                .child("Agent Chat is loading...")
                .into_any_element()
        };

        div()
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .child(titlebar)
            .child(agent_chat_body)
            .into_any_element()
    }

    fn render_agent_chat_window_footer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let agent_chat_entity = self.embedded_agent_chat.as_ref()?;
        let view = agent_chat_entity.read(cx);
        view.build_external_host_footer(agent_chat_entity.downgrade(), cx)
    }

    fn process_render_side_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_bottom_resize_observation(window);
        self.detect_manual_resize(window);
        self.drain_pending_focus(window, cx);
        self.maybe_update_theme_cache();
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
        let body_opacity = if self.entry_reveal.body_visible {
            1.0
        } else {
            0.0
        };

        self.process_render_side_effects(window, cx);

        let theme = crate::theme::get_cached_theme();
        let vibrancy_bg = crate::ui_foundation::get_vibrancy_background(&theme);
        let theme_background_gradients =
            crate::ui_foundation::theme_background_gradient_layers("notes-bg-layer", &theme);

        let in_agent_chat_mode = self.surface_mode == NotesSurfaceMode::AgentChat;
        let detached_footer = crate::footer_popup::glass_scroll_bands_active();
        let window_size = window.bounds().size;
        let regions = crate::footer_popup::main_window_detached_footer_regions_gpui(
            f32::from(window_size.width),
            f32::from(window_size.height),
            if detached_footer {
                crate::components::footer_chrome::current_main_menu_footer_height()
            } else {
                0.0
            },
            if detached_footer {
                crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
            } else {
                0.0
            },
            window.scale_factor(),
        );

        let footer = if in_agent_chat_mode {
            self.render_agent_chat_window_footer(cx)
        } else if self.selected_note_id.is_some() {
            Some(self.render_editor_footer(
                self.preview_enabled,
                self.focus_mode,
                self.window_hovered || self.force_hovered,
                self.get_character_count(cx),
                cx,
            ))
        } else {
            None
        };
        if footer.is_none() {
            crate::components::footer_chrome::remove_glass_capsule_group(
                window,
                "notes-footer-action-rail",
            );
            crate::platform::footer_hit_regions::sync_for_window(window, "notes-footer-empty", &[]);
        } else {
            crate::platform::footer_hit_regions::remove_group(window, "notes-footer-empty");
        }

        let content = if in_agent_chat_mode {
            self.render_agent_chat_surface(cx)
        } else {
            self.render_editor(body_opacity, cx).into_any_element()
        };

        let stage = if detached_footer {
            div()
                .id("notes-window-content-stage")
                .w_full()
                .h(px(regions.main_content.height))
                .min_h(px(regions.main_content.height))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(crate::ui::chrome::LIQUID_GLASS_WINDOW_RADIUS_PX))
                .when_some(vibrancy_bg, |d, bg| d.bg(bg))
                .children(theme_background_gradients)
                .child(content)
                .into_any_element()
        } else {
            div()
                .id("notes-window-content-stage")
                .w_full()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .overflow_hidden()
                .when_some(vibrancy_bg, |d, bg| d.bg(bg))
                .children(theme_background_gradients)
                .child(content)
                .into_any_element()
        };

        div()
            .id("notes-window-root")
            .flex()
            .flex_col()
            .size_full()
            .relative()
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus_handle)
            .when(mouse_cursor_hidden, |d| d.cursor(CursorStyle::None))
            .on_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                let overlay_was_active = this.command_bar.is_open()
                    || this.note_switcher.is_open()
                    || confirm::is_confirm_window_open()
                    || window.has_active_dialog(cx);
                if this.command_bar.is_open() {
                    this.close_actions_panel(window, cx);
                }
                if this.note_switcher.is_open() {
                    this.close_browse_panel(window, cx);
                }
                if confirm::is_confirm_window_open() {
                    confirm::route_key_to_confirm_popup("escape", cx);
                }
                this.handle_bottom_resize_mouse_down(event, overlay_was_active, window, cx);
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
            .when(detached_footer, |d| {
                d.child(
                    div()
                        .w_full()
                        .h(px(regions.transparent_gap.height))
                        .flex_none(),
                )
            })
            .when_some(footer, |d, footer| d.child(footer))
            .children(gpui_component::Root::render_dialog_layer(window, cx))
            .children(gpui_component::Root::render_notification_layer(window, cx))
    }
}
