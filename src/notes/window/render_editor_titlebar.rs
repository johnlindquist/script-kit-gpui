use super::*;

impl NotesApp {
    /// Center titlebar control: a one-shot "Ask AI" command that hands the
    /// selected note off to the MAIN window's Agent Chat. This is a command,
    /// not a mode — it never displays a toggled/selected state, and the Notes
    /// window stays open.
    pub(super) fn render_ask_ai_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(descriptor) = crate::notes::notes_action_for_id(
            self.notes_action_context(),
            NotesAction::SendToAi.id(),
        ) else {
            return div().into_any_element();
        };
        let action = descriptor.action;
        Button::new(descriptor.semantic_action_id)
            .ghost()
            .xsmall()
            .label(descriptor.label)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.handle_action(action, window, cx);
            }))
            .into_any_element()
    }

    pub(super) fn render_titlebar_trash_actions(
        &self,
        has_selection: bool,
        is_trash: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let actions = (has_selection && is_trash)
            .then(|| {
                let context = self.notes_action_context();
                Some((
                    crate::notes::notes_action_for_id(context, NotesAction::RestoreNote.id())?,
                    crate::notes::notes_action_for_id(
                        context,
                        NotesAction::PermanentlyDeleteNote.id(),
                    )?,
                ))
            })
            .flatten();

        div()
            .w(px(TITLEBAR_ICONS_W))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_end()
            .gap_1()
            .when_some(actions, |d, (restore, permanently_delete)| {
                let restore_action = restore.action;
                let permanently_delete_action = permanently_delete.action;
                let restore_shortcut = restore
                    .shortcut
                    .map(crate::components::hint_strip::compact_shortcut_display_string)
                    .unwrap_or_default();
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new(restore.semantic_action_id)
                                .ghost()
                                .xsmall()
                                .label(format!("{} ({restore_shortcut})", restore.label))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.handle_action(restore_action, window, cx);
                                })),
                        )
                        .child(
                            Button::new(permanently_delete.semantic_action_id)
                                .ghost()
                                .xsmall()
                                .icon(IconName::Delete)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.handle_action(permanently_delete_action, window, cx);
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
            .debug_selector(|| "notes-titlebar".to_string())
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
            // Center: the one-shot Ask AI command (replaces the removed
            // Notes/Agent mode switcher). Fades with focus mode like the rest
            // of the chrome but never disappears for the title.
            .child(
                div()
                    .flex_none()
                    .when(in_focus_mode && !window_hovered, |d| d.opacity(0.))
                    .when(in_focus_mode && window_hovered, |d| {
                        d.opacity(OPACITY_DISABLED)
                    })
                    .child(self.render_ask_ai_button(cx)),
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
