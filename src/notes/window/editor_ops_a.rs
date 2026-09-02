use super::*;

impl NotesApp {
    pub(super) fn toggle_checklist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.toggle_checklist(window, cx);
        });
        self.has_unsaved_changes = true;
    }

    pub(super) fn toggle_task_marker_at(
        &mut self,
        marker_range: std::ops::Range<usize>,
        currently_checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let toggled = self.notes_editor.update(cx, |editor, cx| {
            editor.toggle_task_marker_at(marker_range, currently_checked, window, cx)
        });
        if toggled {
            self.has_unsaved_changes = true;
        }
        toggled
    }

    pub(crate) fn toggle_task_marker_for_owned_evaluation(
        &mut self,
        marker_range: std::ops::Range<usize>,
        checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.host_policy.is_hidden() && crate::runtime_policy::owned_evaluation().is_some(),
            "owned_notes_required"
        );
        anyhow::ensure!(self.preview_enabled, "notes_preview_required");
        anyhow::ensure!(
            self.toggle_task_marker_at(marker_range, checked, window, cx),
            "stale_task_marker"
        );
        Ok(())
    }

    pub(super) fn insert_horizontal_rule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.insert_horizontal_rule(window, cx);
        });
        self.has_unsaved_changes = true;
    }

    pub(super) fn cycle_heading(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.cycle_heading(window, cx);
        });
        self.has_unsaved_changes = true;
    }

    pub(super) fn move_line_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.move_line_up(window, cx);
        });
        self.has_unsaved_changes = true;
    }

    pub(super) fn move_line_down(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.move_line_down(window, cx);
        });
        self.has_unsaved_changes = true;
    }

    pub(super) fn select_current_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.select_current_line(window, cx);
        });
    }

    pub(super) fn try_smart_paste(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let clipboard = Self::read_clipboard(cx);
        let handled = self.notes_editor.update(cx, |editor, cx| {
            editor.try_smart_paste(&clipboard, window, cx)
        });
        if handled {
            self.has_unsaved_changes = true;
        }
        handled
    }

    pub(super) fn toggle_blockquote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.toggle_blockquote(window, cx);
        });
        self.has_unsaved_changes = true;
    }

    pub(super) fn duplicate_line(
        &mut self,
        direction_down: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.notes_editor.update(cx, |editor, cx| {
            editor.duplicate_line(direction_down, window, cx);
        });
        self.has_unsaved_changes = true;
    }
}
