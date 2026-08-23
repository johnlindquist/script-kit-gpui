impl ScriptListApp {
    pub(crate) fn sync_main_window_resize_lock(
        &self,
        window: &mut gpui::Window,
        cx: &Context<Self>,
    ) {
        let should_lock = match &self.current_view {
            AppView::AgentChatView { entity } => {
                entity.read_with(cx, |view, _cx| view.locks_main_window_resize())
            }
            _ => false,
        };
        crate::platform::set_window_resizable(window, !should_lock);
    }

    /// Hide the mouse cursor while typing.
    /// The cursor will be shown again when the mouse moves.
    pub(crate) fn hide_mouse_cursor(&mut self, cx: &mut Context<Self>) {
        if !self.mouse_cursor_hidden {
            self.mouse_cursor_hidden = true;
            crate::platform::hide_cursor_until_mouse_moves();
            cx.notify();
        }
    }

    /// Show the mouse cursor (called when mouse moves).
    /// Also switches to Mouse input mode to re-enable hover effects.
    /// Only calls cx.notify() when state actually changes, to avoid
    /// render churn during passive scrolling.
    pub(crate) fn show_mouse_cursor(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;

        if !matches!(self.input_mode, InputMode::Mouse) {
            self.input_mode = InputMode::Mouse;
            changed = true;
        }

        if self.mouse_cursor_hidden {
            self.mouse_cursor_hidden = false;
            changed = true;
        }

        if changed {
            cx.notify();
        }
    }
}
