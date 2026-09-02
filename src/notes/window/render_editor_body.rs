use super::*;

impl NotesApp {
    pub(super) fn render_editor_body(
        &mut self,
        is_trash: bool,
        has_selection: bool,
        is_preview: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Render receipt default: no hover chip this frame. The editor branch
        // below overwrites it when a chip is actually built.
        self.last_deeplink_hover_hint = None;

        if self.kit_resource_preview.is_some() {
            return self.render_kit_resource_preview(cx);
        }

        let no_notes = self.get_visible_notes().is_empty();

        if no_notes && !has_selection && is_trash {
            return div()
                .id("notes-empty-trash")
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .text_base()
                        .text_color(cx.theme().muted_foreground)
                        .child("Trash is empty"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground.opacity(OPACITY_SUBTLE))
                        .child("Deleted notes will appear here"),
                )
                .child(
                    div()
                        .id("back-to-notes-link")
                        .text_xs()
                        .text_color(cx.theme().accent)
                        .cursor_pointer()
                        .hover(|s| s.text_color(cx.theme().foreground))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_view_mode(NotesViewMode::AllNotes, window, cx);
                        }))
                        .child("← Back to Notes"),
                )
                .into_any_element();
        }

        if no_notes && !has_selection {
            return div()
                .id("notes-empty-state")
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    div()
                        .text_base()
                        .text_color(cx.theme().muted_foreground.opacity(OPACITY_MUTED))
                        .child("No notes yet"),
                )
                .child(
                    div()
                        .id("create-first-note")
                        .text_sm()
                        .text_color(cx.theme().accent)
                        .cursor_pointer()
                        .hover(|s| s.text_color(cx.theme().foreground))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.create_note(window, cx);
                        }))
                        .child("Create your first note"),
                )
                .child(
                    div().flex().flex_col().items_center().gap_1().pt_2().child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground.opacity(OPACITY_MUTED))
                                    .child("⌘N  new"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground.opacity(OPACITY_MUTED))
                                    .child("⌘⇧N  from clipboard"),
                            ),
                    ),
                )
                .into_any_element();
        }

        if is_preview {
            let content = self.notes_editor.read(cx).content(cx);
            let entity = cx.entity().downgrade();
            let on_toggle_task: markdown::TaskToggleHandler =
                std::rc::Rc::new(move |marker_range, checked, window, cx| {
                    if let Some(app) = entity.upgrade() {
                        app.update(cx, |app, cx| {
                            app.toggle_task_marker_at(marker_range, checked, window, cx);
                        });
                    }
                });
            return self
                .notes_editor
                .read(cx)
                .render_preview(&content, on_toggle_task, cx.theme());
        }

        // Ghost text renders through the editor's native inline-completion
        // channel (`sync_notes_ghost_inline_completion`), shaped inside the
        // editor's own text layout, so it aligns with the caret exactly. Do
        // not reintroduce an absolutely positioned overlay here: hand-derived
        // padding/advance/line-height math drifts from the Input's real
        // metrics and renders the ghost offset from the text.
        let input = self.notes_editor.read(cx).render_input(cx);
        // Hover discoverability: while the mouse is over a deeplink the
        // vendored input paints an underline + pointer cursor, and this chip
        // names the click action so previews are learnable before the first
        // blind click. Absolute overlay — never reflows the editor.
        let hover_hint_model = crate::notes::deeplink_activation::hover_hint_model(
            self.notes_editor.read(cx).hovered_deeplink(cx),
            crate::notes::deeplink_activation::ActivationSurface::NotesWindow,
        );
        self.last_deeplink_hover_hint = hover_hint_model
            .as_ref()
            .map(|(verb, href)| serde_json::json!({ "verb": verb, "href": href }));
        let hover_hint = hover_hint_model.map(|(verb, href)| {
            crate::components::resource_preview::render_deeplink_hover_hint(
                "notes-deeplink-hover-hint",
                verb,
                &href,
                cx,
            )
        });
        let input = div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .h_full()
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                    this.activate_deeplink_from_mouse_up(event.clone(), window, cx);
                }),
            )
            .child(input);
        let spine_panel = self.render_notes_spine_panel(cx);
        div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .h_full()
            .child(input)
            .when_some(hover_hint, |d, chip| d.child(chip))
            .when_some(spine_panel, |d, panel| d.child(panel))
            .into_any_element()
    }
}

impl NotesApp {
    pub(super) fn notes_spine_surface_allows_editor_list(&self) -> bool {
        self.selected_note_id.is_some()
            && self.view_mode != NotesViewMode::Trash
            && !self.preview_enabled
            && self.kit_resource_preview.is_none()
            && !self.show_search
            && !self.command_bar.is_open()
            && !self.note_switcher.is_open()
    }

    pub(super) fn notes_spine_input(
        &self,
        cx: &gpui::App,
    ) -> Option<crate::components::notes_editor::spine::NotesEditorSpineInput> {
        if !self.notes_spine_surface_allows_editor_list() {
            return None;
        }
        let content = self.notes_editor.read(cx).content(cx);
        let selection = self.notes_editor.read(cx).selection(cx);
        crate::components::notes_editor::spine::local_spine_input_for_contract(
            crate::components::notes_editor::spine::NotesEditorHostSpineContract::notes(),
            &content,
            selection,
        )
    }

    pub(super) fn notes_spine_model(
        &mut self,
        cx: &gpui::App,
    ) -> Option<crate::components::notes_editor::spine::NotesEditorSpineModel> {
        let input = self.notes_spine_input(cx)?;
        crate::components::notes_editor::spine::spine_model_for_runtime(
            &mut self.spine_runtime,
            input,
            Self::build_notes_spine_rows,
        )
    }

    fn build_notes_spine_rows(
        parse: &crate::spine::SpineParse,
        projection: &crate::spine::SpineCursorProjection,
    ) -> Option<crate::components::notes_editor::spine::NotesEditorSpineRows> {
        if matches!(
            projection.active_segment_kind,
            crate::spine::SpineSegmentKind::ProjectCwd { .. }
        ) {
            return None;
        }
        let sections = crate::spine::list::build_spine_list_sections_full_with_resolved_tokens(
            parse,
            projection,
            None,
            &|_token| false,
        );
        let rows = crate::components::notes_editor::spine::push_spine_sections_as_grouped(
            sections,
            crate::components::notes_editor::spine::notes_editor_supports_insert_resolve_action,
        );
        (!rows.grouped.is_empty()).then_some(rows)
    }

    fn render_notes_spine_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let model = self.notes_spine_model(cx)?;
        crate::components::notes_editor::spine::render_spine_overlay(
            crate::components::notes_editor::spine::NotesEditorHostSpineContract::notes(),
            &model,
            self.spine_runtime.selected_index,
            cx,
            |this: &mut NotesApp, ix, _window, cx| {
                this.spine_runtime.selected_index = ix;
                cx.notify();
            },
        )
    }

    pub(super) fn selected_notes_spine_row(
        &self,
        model: &crate::components::notes_editor::spine::NotesEditorSpineModel,
    ) -> Option<crate::spine::SpineListRow> {
        model.selected_row(self.spine_runtime.selected_index)
    }

    pub(super) fn move_notes_spine_selection(
        &mut self,
        direction: isize,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(model) = self.notes_spine_model(cx) else {
            return false;
        };
        let len = model.grouped.len();
        if len == 0 {
            return false;
        }
        let mut next = self.spine_runtime.selected_index.min(len - 1);
        loop {
            next = if direction < 0 {
                next.saturating_sub(1)
            } else {
                (next + 1).min(len - 1)
            };
            if matches!(
                model.grouped.get(next),
                Some(crate::list_item::GroupedListItem::Item(_))
            ) {
                self.spine_runtime.selected_index = next;
                cx.notify();
                return true;
            }
            if next == 0 || next == len - 1 {
                return true;
            }
        }
    }

    pub(super) fn reset_notes_spine_navigation(&mut self, cx: &mut Context<Self>) {
        let key = self.notes_spine_input(cx).map(|input| input.key);
        self.spine_runtime.dismiss_current_key(key);
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    fn replace_notes_spine_segment(
        &mut self,
        model: &crate::components::notes_editor::spine::NotesEditorSpineModel,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        replacement: &str,
        trailing_space: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let content = self.notes_editor.read(cx).content(cx);
        let Some(segment) = model.parse.segments.get(segment_index) else {
            return false;
        };
        if segment.byte_range != segment_byte_range {
            return false;
        }
        let Some((new_content, cursor)) =
            crate::components::notes_editor::spine::replace_segment_content(
                &content,
                model.line_range.clone(),
                segment_byte_range,
                replacement,
                trailing_space,
            )
        else {
            return false;
        };

        self.notes_editor.update(cx, |editor, cx| {
            editor.set_value_preserving_scroll(new_content.clone(), cursor, window, cx);
        });
        self.spine_runtime.selected_index = 0;
        self.spine_runtime.clear_alias_cache();
        self.on_editor_change(window, cx);
        cx.notify();
        true
    }

    fn apply_notes_spine_action(
        &mut self,
        action: crate::spine::SpineListAction,
        model: &crate::components::notes_editor::spine::NotesEditorSpineModel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match action {
            crate::spine::SpineListAction::InsertSegmentText {
                segment_index,
                segment_byte_range,
                text,
                trailing_space,
            } => self.replace_notes_spine_segment(
                model,
                segment_index,
                segment_byte_range,
                text.as_ref(),
                trailing_space,
                window,
                cx,
            ),
            crate::spine::SpineListAction::ResolveSegment {
                segment_index,
                segment_byte_range,
                replacement,
                resolution_source,
                trailing_space,
                ..
            } => {
                if resolution_source.as_ref() == "cwd" {
                    return false;
                }
                let (replacement, alias) = if resolution_source.as_ref() == "context-builtin" {
                    crate::components::notes_editor::spine::context_reference_for_part(
                        replacement.as_ref(),
                        None,
                    )
                    .map(|(reference, part)| (reference, Some(part)))
                    .unwrap_or_else(|| (replacement.to_string(), None))
                } else {
                    (replacement.to_string(), None)
                };
                let applied = self.replace_notes_spine_segment(
                    model,
                    segment_index,
                    segment_byte_range,
                    &replacement,
                    trailing_space,
                    window,
                    cx,
                );
                if applied {
                    if let Some(part) = alias {
                        self.spine_runtime.register_mention_alias(replacement, part);
                    }
                }
                applied
            }
            crate::spine::SpineListAction::OpenFileSearchPortal { .. }
            | crate::spine::SpineListAction::OpenModeExit { .. }
            | crate::spine::SpineListAction::AcceptMenuSyntaxTrigger { .. }
            | crate::spine::SpineListAction::AcceptMenuSyntaxObject { .. }
            | crate::spine::SpineListAction::AttachContextResult { .. }
            | crate::spine::SpineListAction::Noop => false,
        }
    }

    pub(super) fn accept_notes_spine_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(model) = self.notes_spine_model(cx) else {
            return false;
        };
        let Some(row) = self.selected_notes_spine_row(&model) else {
            return false;
        };
        let row_id = row.id.to_string();
        let handled = self.apply_notes_spine_action(row.action, &model, window, cx);
        if !handled {
            tracing::warn!(
                target: "script_kit::spine",
                event = "notes_spine_action_unhandled",
                row_id = %row_id,
            );
        }
        handled
    }
}
