use super::*;

impl NotesApp {
    /// Render the Agent Chat surface with a thin Notes-owned titlebar containing a
    /// mode switch so the user can toggle back to the Notes editor.
    pub(super) fn render_agent_chat_surface(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted_color = cx.theme().muted_foreground;
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
                    // Shared persistent Notes / Agent switcher — same owner as
                    // the Notes-mode titlebar.
                    .child(self.render_surface_switcher(cx)),
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

    /// Mode-owned capsule/hit-region teardown. Explicit removal on the same
    /// render decision that selects the footer — a Notes ↔ Agent switch must
    /// not leave a stale clickable capsule for the 250ms stale-TTL sweep.
    fn sync_mode_owned_footer_groups(&self, window: &Window, chrome: contract::NotesChromePolicy) {
        match chrome.footer_mode {
            contract::NotesFooterMode::None => {
                crate::components::footer_chrome::remove_glass_capsule_group(
                    window,
                    contract::NOTES_EDITOR_FOOTER_GROUP,
                );
                crate::components::footer_chrome::remove_glass_capsule_group(
                    window,
                    crate::components::footer_chrome::MAIN_WINDOW_FOOTER_CONFIG_RAIL_GROUP,
                );
                crate::platform::footer_hit_regions::sync_for_window(
                    window,
                    contract::NOTES_EMPTY_FOOTER_HIT_GROUP,
                    &[],
                );
            }
            contract::NotesFooterMode::AgentChatExternal => {
                crate::components::footer_chrome::remove_glass_capsule_group(
                    window,
                    contract::NOTES_EDITOR_FOOTER_GROUP,
                );
                crate::platform::footer_hit_regions::remove_group(
                    window,
                    contract::NOTES_EMPTY_FOOTER_HIT_GROUP,
                );
            }
        }
    }

    /// Keep the native Tahoe backdrop partition in lockstep with the chrome
    /// policy (Notes: full window; Agent: footer band + gap reserved).
    fn sync_backdrop_partition(&mut self, window: &Window, chrome: contract::NotesChromePolicy) {
        let desired = chrome.backdrop_bottom_inset;
        let needs_sync = match self.last_synced_backdrop_inset {
            Some(previous) => previous != desired,
            // Nothing synced yet: the configure-time default is already
            // full-window, so only a positive inset needs a push.
            None => desired > 0.0,
        };
        if !needs_sync {
            return;
        }
        if crate::platform::set_gpui_window_backdrop_bottom_inset(
            window,
            "Notes",
            f64::from(desired),
        ) {
            self.last_synced_backdrop_inset = Some(desired);
        }
    }

    /// Keep the automation registry's Notes bounds current for ANY frame
    /// change — native AppKit-tracked resizes never route through the custom
    /// bottom-resize observation, and stale automation bounds would poison
    /// every layout-based runtime proof.
    fn sync_automation_bounds_on_change(&mut self, window: &Window) {
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
        self.last_automation_synced_bounds = Some(tuple);
        crate::windows::set_automation_bounds(
            "notes",
            Some(crate::protocol::AutomationWindowBounds {
                x: f64::from(tuple[0]),
                y: f64::from(tuple[1]),
                width: f64::from(tuple[2]),
                height: f64::from(tuple[3]),
            }),
        );
    }

    fn process_render_side_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_bottom_resize_observation(window);
        self.detect_manual_resize(window);
        self.sync_automation_bounds_on_change(window);
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
        let body_visible = self.entry_reveal.body_visible;
        let body_reveal_generation = self.entry_reveal.generation;

        self.process_render_side_effects(window, cx);

        let theme = crate::theme::get_cached_theme();
        let vibrancy_bg = crate::ui_foundation::get_vibrancy_background(&theme);
        let theme_background_gradients =
            crate::ui_foundation::theme_background_gradient_layers("notes-bg-layer", &theme);

        let in_agent_chat_mode = self.surface_mode == NotesSurfaceMode::AgentChat;
        let glass_active = crate::footer_popup::glass_scroll_bands_active();
        // One chrome decision drives the stage partition, gap reservation,
        // footer rendering, native backdrop inset, and capsule cleanup.
        let chrome = contract::notes_chrome_policy(self.surface_mode, glass_active);
        let detached_footer = chrome.reserves_external_footer();
        let footer_strip_height = if detached_footer {
            crate::components::footer_chrome::current_main_menu_footer_height()
        } else {
            0.0
        };
        let window_size = window.bounds().size;
        let regions = crate::footer_popup::main_window_detached_footer_regions_gpui(
            f32::from(window_size.width),
            f32::from(window_size.height),
            footer_strip_height,
            if detached_footer {
                crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
            } else {
                0.0
            },
            window.scale_factor(),
        );

        self.sync_mode_owned_footer_groups(window, chrome);
        self.sync_backdrop_partition(window, chrome);

        // Agent mode reserves its footer band IMMEDIATELY — even while the
        // chat entity is still loading — so the body never jumps when the
        // real footer arrives. Notes mode owns the full window: no footer,
        // no reserved gap.
        let footer = if in_agent_chat_mode {
            self.render_agent_chat_window_footer(cx).or_else(|| {
                detached_footer.then(|| {
                    div()
                        .w_full()
                        .h(px(footer_strip_height))
                        .flex_none()
                        .into_any_element()
                })
            })
        } else {
            None
        };

        let content = if in_agent_chat_mode {
            self.render_agent_chat_surface(cx)
        } else {
            self.render_editor(body_visible, body_reveal_generation, cx)
                .into_any_element()
        };

        // Mention-preview hint, migrated from the removed Notes footer rail:
        // a transient bottom-right overlay that reserves no layout height and
        // appears only while the cursor sits on an inline `@` token.
        let mention_overlay = if in_agent_chat_mode {
            None
        } else {
            self.focused_note_mention_preview(cx)
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
                })
        };

        let stage = if detached_footer {
            div()
                .id("notes-window-content-stage")
                .relative()
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
                .children(mention_overlay)
                .into_any_element()
        } else {
            div()
                .id("notes-window-content-stage")
                .relative()
                .w_full()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .overflow_hidden()
                // Full-window glass (Notes mode): the stage must match the
                // rounded native backdrop or square content corners poke out.
                .when(glass_active, |d| {
                    d.rounded(px(crate::ui::chrome::LIQUID_GLASS_WINDOW_RADIUS_PX))
                })
                .when_some(vibrancy_bg, |d, bg| d.bg(bg))
                .children(theme_background_gradients)
                .child(content)
                .children(mention_overlay)
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
