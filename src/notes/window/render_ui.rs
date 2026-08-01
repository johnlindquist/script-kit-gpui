use super::*;

use crate::components::notes_editor::{NotesEditorToolbarAction, NOTES_EDITOR_TOOLBAR_ACTIONS};
use crate::ui_foundation::{compact_action_row, log_ui_action, UiActionSpec, UiSurface};

/// Opacity for toolbar section borders — matches Notes window token.
const OPACITY_SECTION_BORDER: f32 = 0.2;

pub(super) const NOTES_RESOURCE_HINT_OPEN_SOURCE_ID: &str =
    "notes-kit-resource-preview-hint-open-source";
pub(super) const NOTES_RESOURCE_HINT_COPY_URI_ID: &str = "notes-kit-resource-preview-hint-copy-uri";
pub(super) const NOTES_RESOURCE_HINT_BACK_ID: &str = "notes-kit-resource-preview-hint-back";

impl NotesApp {
    pub(super) fn render_kit_resource_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::components::hint_strip::ClickableHint;
        use crate::components::resource_preview::{
            render_resource_preview, ResourcePreviewAction, ResourcePreviewSurface,
        };

        let Some(preview) = self.kit_resource_preview.as_ref() else {
            return div().into_any_element();
        };
        let has_source_note = self.kit_resource_preview_note_source().is_some();

        let mut actions = vec![ResourcePreviewAction {
            id: "notes-kit-resource-preview-copy-uri".into(),
            label: "Copy URI".into(),
            muted: false,
            on_click: std::rc::Rc::new(cx.listener(|this, _, _window, cx| {
                this.copy_kit_resource_preview_uri(cx);
            })),
        }];
        if has_source_note {
            actions.push(ResourcePreviewAction {
                id: "notes-kit-resource-preview-open-source".into(),
                label: "Open Source".into(),
                muted: false,
                on_click: std::rc::Rc::new(cx.listener(|this, _, window, cx| {
                    this.open_kit_resource_preview_source(window, cx);
                })),
            });
        }
        actions.push(ResourcePreviewAction {
            id: "notes-kit-resource-preview-close".into(),
            // "Back to Note", not "Close": Escape restores the note editor
            // underneath — it never closes the Notes window from a preview.
            label: "Back to Note".into(),
            muted: true,
            on_click: std::rc::Rc::new(cx.listener(|this, _, window, cx| {
                this.close_kit_resource_preview(window, cx);
            })),
        });

        let mut footer_hints = Vec::new();
        if has_source_note {
            footer_hints.push(ClickableHint::new(
                NOTES_RESOURCE_HINT_OPEN_SOURCE_ID,
                "↵ Open Source",
                cx.listener(|this, _, window, cx| {
                    this.open_kit_resource_preview_source(window, cx);
                }),
            ));
        }
        footer_hints.push(ClickableHint::new(
            NOTES_RESOURCE_HINT_COPY_URI_ID,
            "⌘C Copy URI",
            cx.listener(|this, _, _window, cx| {
                this.copy_kit_resource_preview_uri(cx);
            }),
        ));
        footer_hints.push(ClickableHint::new(
            NOTES_RESOURCE_HINT_BACK_ID,
            "Esc Back to Note",
            cx.listener(|this, _, window, cx| {
                this.close_kit_resource_preview(window, cx);
            }),
        ));

        render_resource_preview(
            ResourcePreviewSurface {
                id_prefix: "notes-kit-resource-preview",
                title: preview.title.clone().into(),
                uri: preview.uri.clone().into(),
                mime_type: preview.mime_type.clone().into(),
                text: preview.text.clone().into(),
                truncated: preview.truncated,
                // Match the Notes editor text inset so preview content aligns
                // with note prose.
                inset_x: crate::notes::window::style::adopted_metrics().editor_padding_x,
                actions,
                footer_hints,
            },
            cx,
        )
    }

    fn render_toolbar_button(
        &self,
        item: NotesEditorToolbarAction,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Button::new(item.spec.id)
            .ghost()
            .xsmall()
            .label(item.spec.label)
            .on_click(cx.listener(move |this, _, window, cx| {
                log_ui_action(UiSurface::NotesToolbar, item.spec, "click");
                this.notes_editor.update(cx, |editor, cx| {
                    (item.run)(editor, window, cx);
                });
                this.has_unsaved_changes = true;
            }))
    }

    /// Render the formatting toolbar backed by the shared notes editor component.
    pub(super) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        compact_action_row()
            .py_1()
            .px_3()
            .border_b_1()
            .border_color(cx.theme().border.opacity(OPACITY_SECTION_BORDER))
            .children(
                NOTES_EDITOR_TOOLBAR_ACTIONS
                    .iter()
                    .copied()
                    .map(|item| self.render_toolbar_button(item, cx)),
            )
    }

    /// Render the export menu
    pub(super) fn render_export_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        compact_action_row().children(
            NOTES_EXPORT_ACTIONS
                .iter()
                .copied()
                .map(|item| self.render_export_button(item, cx)),
        )
    }
}

#[derive(Clone, Copy)]
struct NotesExportAction {
    spec: UiActionSpec,
    run: fn(&mut NotesApp, &mut Context<NotesApp>),
}

fn notes_export_plain_text(app: &mut NotesApp, cx: &mut Context<NotesApp>) {
    app.export_note(ExportFormat::PlainText, cx);
}

fn notes_export_markdown(app: &mut NotesApp, cx: &mut Context<NotesApp>) {
    app.export_note(ExportFormat::Markdown, cx);
}

fn notes_export_html(app: &mut NotesApp, cx: &mut Context<NotesApp>) {
    app.export_note(ExportFormat::Html, cx);
}

const NOTES_EXPORT_ACTIONS: [NotesExportAction; 3] = [
    NotesExportAction {
        spec: UiActionSpec {
            id: "export-txt",
            label: "TXT",
            shortcut: None,
        },
        run: notes_export_plain_text,
    },
    NotesExportAction {
        spec: UiActionSpec {
            id: "export-md",
            label: "MD",
            shortcut: None,
        },
        run: notes_export_markdown,
    },
    NotesExportAction {
        spec: UiActionSpec {
            id: "export-html",
            label: "HTML",
            shortcut: None,
        },
        run: notes_export_html,
    },
];

impl NotesApp {
    fn render_export_button(
        &self,
        item: NotesExportAction,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Button::new(item.spec.id)
            .ghost()
            .xsmall()
            .label(item.spec.label)
            .on_click(cx.listener(move |this, _, _, cx| {
                log_ui_action(UiSurface::NotesExportMenu, item.spec, "click");
                (item.run)(this, cx);
            }))
    }

    pub(super) fn format_search_match_counter(
        note_position: Option<(usize, usize)>,
        total_matches: usize,
    ) -> String {
        let current_match = note_position.map(|(position, _)| position).unwrap_or(0);
        format!("{current_match}/{total_matches}")
    }

    /// Render the search input bar (shown when Cmd+F is pressed)
    pub(super) fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let sk_theme = crate::theme::get_cached_theme();
        let opacity = sk_theme.get_opacity();
        let total_matches = self.get_visible_notes().len();
        let counter_text =
            Self::format_search_match_counter(self.get_note_position(), total_matches);
        let search_surface = rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
            sk_theme.colors.background.search_box,
            opacity.search_box,
        ));
        let counter_surface = rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
            sk_theme.colors.accent.selected_subtle,
            OPACITY_SUBTLE,
        ));

        div().w_full().px_3().pt_2().pb_2().child(
            Input::new(&self.search_state)
                .w_full()
                .small()
                .prefix(IconName::Search)
                .suffix(
                    div()
                        .h(px(18.))
                        .px_2()
                        .rounded_full()
                        .bg(counter_surface)
                        .flex()
                        .items_center()
                        .text_xs()
                        .text_color(theme.muted_foreground.opacity(OPACITY_MUTED))
                        .child(counter_text),
                )
                .bg(search_surface)
                .border_color(theme.border.opacity(OPACITY_SECTION_BORDER)),
        )
    }
}

#[cfg(test)]
mod action_spec_tests {
    use super::*;
    use crate::components::notes_editor::NOTES_EDITOR_TOOLBAR_ACTIONS;
    use std::collections::BTreeSet;

    #[test]
    fn toolbar_action_ids_are_unique() {
        let mut ids = BTreeSet::new();
        for item in NOTES_EDITOR_TOOLBAR_ACTIONS.iter() {
            assert!(
                ids.insert(item.spec.id),
                "duplicate toolbar action id: {}",
                item.spec.id
            );
        }
    }

    #[test]
    fn export_action_ids_are_unique() {
        let mut ids = BTreeSet::new();
        for item in NOTES_EXPORT_ACTIONS.iter() {
            assert!(
                ids.insert(item.spec.id),
                "duplicate export action id: {}",
                item.spec.id
            );
        }
    }
}
