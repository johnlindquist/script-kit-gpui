use super::*;

impl NotesApp {
    pub(super) fn render_editor_footer(
        &self,
        _is_preview: bool,
        in_focus_mode: bool,
        window_hovered: bool,
        _char_count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
            cx.theme().accent
        } else {
            cx.theme().accent.opacity(OPACITY_MUTED)
        };
        let leading = div()
            .id("notes-footer-save-status")
            .min_w(px(MIN_TARGET_SIZE))
            .flex()
            .items_center()
            .text_xs()
            .text_color(status_color)
            .child(status_glyph)
            .into_any_element();

        let hints = crate::components::universal_prompt_hints_with_primary_key_label("⌘P", "Notes");
        crate::components::emit_surface_prompt_hint_audit(
            "notes::editor_footer",
            &hints,
            "notes_primary_action_is_switcher",
        );

        // Same footer_chrome button frames as the main window footer — shared
        // component, shared keycap language (one cap per key), shared slots.
        let buttons = if in_focus_mode && !window_hovered {
            Vec::new()
        } else {
            crate::components::render_universal_footer_action_button_frames(
                "notes",
                "⌘P",
                "Notes",
                cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                    this.close_actions_panel(window, cx);
                    if this.note_switcher.is_open() {
                        this.close_browse_panel(window, cx);
                    } else {
                        this.open_browse_panel(window, cx);
                    }
                }),
                cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                    if this.command_bar.is_open() {
                        this.close_actions_panel(window, cx);
                    } else {
                        this.open_actions_panel(window, cx);
                    }
                }),
                cx.listener(|this, _: &gpui::ClickEvent, _window, cx| {
                    let _ = this
                        .open_selected_note_cart_in_embedded_agent_chat("NotesFooterCmdEnter", cx);
                }),
            )
        };

        // Mention-preview hint occupies the flexible middle lane: it may
        // truncate (informational), and its presence never moves the
        // never-shrink buttons on the right.
        let mention_preview = self.focused_note_mention_preview(cx);
        let leading = div()
            .flex()
            .flex_row()
            .items_center()
            .child(leading)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .text_xs()
                    .whitespace_nowrap()
                    .text_color(cx.theme().muted_foreground)
                    .when_some(mention_preview, |d, (token, detail)| {
                        d.child(format!("{token} · {detail}"))
                    }),
            )
            .into_any_element();
        let hint_strip = crate::components::footer_chrome::render_footer_action_rail_with_leading(
            "notes-footer-action-rail",
            Some(leading),
            buttons,
        );

        div()
            .child(hint_strip)
            .when(in_focus_mode && !window_hovered, |d| d.opacity(0.))
            .when(in_focus_mode && window_hovered, |d| {
                d.opacity(OPACITY_DISABLED)
            })
            .when(!in_focus_mode && !window_hovered, |d| {
                d.opacity(OPACITY_SUBTLE)
            })
            .when(!in_focus_mode && window_hovered, |d| d.opacity(1.0))
            .into_any_element()
    }
}
