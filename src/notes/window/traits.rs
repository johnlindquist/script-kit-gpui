use super::*;

impl Focusable for NotesApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Drop for NotesApp {
    fn drop(&mut self) {
        // Save any unsaved changes before closing. Route through
        // save_current_note so the ACTIVE DAY BINDING is saved too — the macOS
        // traffic-light close only fires Drop (it bypasses the Escape/Cmd+W
        // paths), and the previous regular-note-only logic here silently lost
        // day-note edits. save_current_note needs no cx and no-ops when clean.
        if self.has_unsaved_changes && !self.save_current_note() {
            tracing::error!("Failed to save unsaved changes on Notes window close");
        }

        // GPUI releases entities lazily: when the Notes window is reopened, the
        // OLD NotesApp's Drop runs on a later effect flush — AFTER the new
        // instance has stored fresh global handles. An unconditional clear here
        // wiped the new instance's NOTES_WINDOW/NOTES_APP_ENTITY slots and its
        // embedded-AI automation registration, breaking every protocol notes
        // target after a toggle-reopen (chaos-15). Only the CURRENT instance
        // may clear the globals.
        let current = CURRENT_NOTES_INSTANCE.load(std::sync::atomic::Ordering::SeqCst);
        if current != self.instance_id {
            debug!(
                dropped_instance = self.instance_id,
                current_instance = current,
                "Stale NotesApp dropped after replacement - leaving globals for the live instance"
            );
            return;
        }

        // Clear the global window handle when NotesApp is dropped
        // This ensures is_notes_window_open() returns false after the window closes
        // regardless of how it was closed (Cmd+W, traffic light, toggle, etc.)
        if let Some(window_handle) = NOTES_WINDOW.get() {
            if let Ok(mut guard) = window_handle.lock() {
                *guard = None;
                debug!("NotesApp dropped - cleared global window handle");
            }
        }

        // Clear the global app entity handle
        if let Some(app_entity) = NOTES_APP_ENTITY.get() {
            if let Ok(mut guard) = app_entity.lock() {
                *guard = None;
                debug!("NotesApp dropped - cleared global app entity handle");
            }
        }
    }
}
