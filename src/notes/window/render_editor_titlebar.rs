use super::*;

impl NotesApp {
    /// One segment of the persistent Notes / Agent surface switcher.
    ///
    /// The selected state is independent of hover; the inactive segment is a
    /// real click target with at least the shared minimum hit size.
    fn render_surface_segment(
        &self,
        id: &'static str,
        label: &'static str,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted_color = cx.theme().muted_foreground;
        let accent_color = cx.theme().accent;
        let target: NotesSurfaceMode = if id == "notes-switch-notes" {
            NotesSurfaceMode::Notes
        } else {
            NotesSurfaceMode::AgentChat
        };

        div()
            .id(id)
            .min_w(px(MIN_TARGET_SIZE))
            .min_h(px(MIN_TARGET_SIZE))
            .px_1()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .when(selected, |d| d.text_color(accent_color))
            .when(!selected, |d| {
                d.text_color(muted_color.opacity(OPACITY_MUTED))
                    .cursor_pointer()
                    .hover(|s| s.text_color(accent_color))
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                match target {
                    NotesSurfaceMode::Notes => {
                        if this.surface_mode != NotesSurfaceMode::Notes {
                            // Plain mode navigation: preserves the Agent
                            // entity, conversation, and any in-flight turn
                            // (prepare_for_host_hide keeps the live session).
                            this.switch_to_notes_surface(window, cx);
                        }
                    }
                    NotesSurfaceMode::AgentChat => {
                        if this.surface_mode != NotesSurfaceMode::AgentChat {
                            // Plain mode navigation MUST NOT stage the current
                            // note: reuse/focus with no initial input. The
                            // explicit "ask about this note" handoff stays on
                            // Cmd+Enter (`open_selected_note_cart_in_embedded_agent_chat`).
                            let _ = this.open_or_focus_embedded_agent_chat(None, window, cx);
                        }
                    }
                }
            }))
            .child(label)
            .into_any_element()
    }

    /// The persistent Notes / Agent switcher — the primary affordance for
    /// moving between the editor and the embedded Agent Chat surface. Visible
    /// in BOTH modes (the floating footer no longer exists in Notes mode).
    pub(super) fn render_surface_switcher(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted_color = cx.theme().muted_foreground;
        let selected = self.surface_mode;

        div()
            .id("notes-surface-switcher")
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .child(self.render_surface_segment(
                "notes-switch-notes",
                "Notes",
                selected == NotesSurfaceMode::Notes,
                cx,
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(muted_color.opacity(OPACITY_SUBTLE))
                    .child("/"),
            )
            .child(self.render_surface_segment(
                "notes-switch-agent_chat",
                "Agent",
                selected == NotesSurfaceMode::AgentChat,
                cx,
            ))
            .into_any_element()
    }

    pub(super) fn render_titlebar_trash_actions(
        &self,
        has_selection: bool,
        is_trash: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w(px(TITLEBAR_ICONS_W))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_end()
            .gap_1()
            .when(has_selection && is_trash, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new("restore")
                                .ghost()
                                .xsmall()
                                .label("Restore (⌘Z)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.restore_note(window, cx);
                                })),
                        )
                        .child(
                            Button::new("permanent-delete")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Delete)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.permanently_delete_note(window, cx);
                                })),
                        ),
                )
            })
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_editor_titlebar(
        &self,
        title: String,
        window_hovered: bool,
        has_selection: bool,
        is_trash: bool,
        _is_preview: bool,
        is_pinned: bool,
        in_focus_mode: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted_color = cx.theme().muted_foreground;
        let accent_color = cx.theme().accent;
        let metrics = style::adopted_metrics();

        let titlebar_actions = self.render_titlebar_trash_actions(has_selection, is_trash, cx);

        // Save-status glyph, migrated from the removed Notes footer rail:
        // ● while unsaved, a brief ✓ after a confirmed save, otherwise empty.
        // Non-interactive information only — it must not become a pseudo-footer.
        let has_unsaved = self.has_unsaved_changes;
        let show_saved = !has_unsaved
            && self
                .last_save_confirmed
                .map(|t| t.elapsed() < Duration::from_millis(SAVED_FLASH_MS))
                .unwrap_or(false);
        let status_glyph = if has_unsaved {
            "●"
        } else if show_saved {
            "✓"
        } else {
            ""
        };
        let status_color = if has_unsaved {
            accent_color
        } else {
            accent_color.opacity(OPACITY_MUTED)
        };

        div()
            .id("notes-titlebar")
            .flex()
            .items_center()
            .h(px(metrics.titlebar_height))
            // Contract-owned horizontal padding (was an inline `.px_3()`);
            // the design-contract exporter reads the same const.
            .px(px(super::contract::NOTES_TITLEBAR_PADDING_X))
            .when(is_trash, |d| {
                d.border_b_1()
                    .border_color(cx.theme().danger.opacity(OPACITY_ACCENT_BORDER))
            })
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if this.force_hovered {
                    return;
                }
                this.titlebar_hovered = *hovered;
                cx.notify();
            }))
            .child(div().w(px(TITLEBAR_TRAFFIC_LIGHT_W)).flex_shrink_0())
            // Leading lane: the current note title, subdued and ellipsized. It
            // yields space to the centered switcher and never displaces it.
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_xs()
                    .text_color(muted_color)
                    .when(!window_hovered, |d| d.opacity(OPACITY_MUTED))
                    .when(window_hovered, |d| d.opacity(1.0))
                    .when(in_focus_mode, |d| d.opacity(0.))
                    .when(is_pinned && !in_focus_mode, |d| {
                        d.child(div().text_xs().text_color(accent_color).child("●"))
                    })
                    .child(title),
            )
            // Center: the persistent Notes / Agent switcher — the primary
            // transition affordance in both modes. Fades with focus mode like
            // the rest of the chrome but never disappears for the title.
            .child(
                div()
                    .flex_none()
                    .when(in_focus_mode && !window_hovered, |d| d.opacity(0.))
                    .when(in_focus_mode && window_hovered, |d| {
                        d.opacity(OPACITY_DISABLED)
                    })
                    .child(self.render_surface_switcher(cx)),
            )
            // Trailing lane: save status + trash/actions cluster.
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .when(in_focus_mode && window_hovered, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(muted_color.opacity(OPACITY_DISABLED))
                                .child("esc  or  ⌘.  exit focus"),
                        )
                    })
                    .child(
                        div()
                            .id("notes-titlebar-save-status")
                            .flex()
                            .items_center()
                            .text_xs()
                            .text_color(status_color)
                            .when(in_focus_mode, |d| d.opacity(0.))
                            .child(status_glyph),
                    )
                    .child(titlebar_actions),
            )
            .into_any_element()
    }
}
