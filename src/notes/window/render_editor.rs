use super::*;

impl NotesApp {
    pub(super) fn selected_note_title(&self, is_trash: bool) -> String {
        let title = self
            .selected_note_id
            .and_then(|id| self.get_visible_notes().iter().find(|n| n.id == id))
            .map(|n| {
                if n.title.is_empty() {
                    "Untitled Note".to_string()
                } else {
                    n.title.clone()
                }
            })
            .unwrap_or_else(|| {
                if is_trash {
                    "No deleted notes".to_string()
                } else {
                    "No note selected".to_string()
                }
            });

        if is_trash {
            format!("🗑 {}", title)
        } else {
            title
        }
    }

    pub(super) fn render_editor(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_trash = self.view_mode == NotesViewMode::Trash;
        let has_selection = self.selected_note_id.is_some();
        let show_toolbar = self.show_format_toolbar;
        let is_preview = self.preview_enabled;
        let is_pinned = self.is_current_note_pinned();
        let in_focus_mode = self.focus_mode;
        let window_hovered = self.window_hovered || self.force_hovered;
        let title = self.selected_note_title(is_trash);
        let titlebar = self.render_editor_titlebar(
            title,
            window_hovered,
            has_selection,
            is_trash,
            is_preview,
            is_pinned,
            in_focus_mode,
            cx,
        );

        let editor_body = self.render_editor_body(is_trash, has_selection, is_preview, cx);

        div()
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .child(titlebar)
            .when(self.show_search && !in_focus_mode, |d| {
                d.child(self.render_search(cx))
            })
            .when(
                !is_trash && has_selection && show_toolbar && !in_focus_mode,
                |d| d.child(self.render_toolbar(cx)),
            )
            .child(div().flex_1().min_h(px(0.)).h_full().child(editor_body))
    }
}
