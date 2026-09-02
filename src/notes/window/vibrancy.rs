use super::*;

impl NotesApp {
    /// Get modal overlay background (theme-aware)
    ///
    /// For dark mode: black overlay (darkens content behind)
    /// For light mode: white overlay (keeps content readable on light backgrounds)
    /// 50% opacity (0x80) for good contrast without being too heavy
    pub(super) fn get_modal_overlay_background() -> gpui::Rgba {
        let sk_theme = crate::theme::get_cached_theme();
        crate::theme::modal_overlay_bg(&sk_theme, 0x80)
    }

    pub(super) fn set_mouse_cursor_hidden_state(
        mouse_cursor_hidden: &mut bool,
        hidden: bool,
    ) -> bool {
        if *mouse_cursor_hidden == hidden {
            return false;
        }
        *mouse_cursor_hidden = hidden;
        true
    }

    pub(super) fn hide_mouse_cursor(&mut self, cx: &mut Context<Self>) {
        if self.host_policy.is_hidden() {
            return;
        }
        if Self::set_mouse_cursor_hidden_state(&mut self.mouse_cursor_hidden, true) {
            crate::platform::hide_cursor_until_mouse_moves();
            cx.notify();
        }
    }

    pub(super) fn show_mouse_cursor(&mut self, cx: &mut Context<Self>) {
        if Self::set_mouse_cursor_hidden_state(&mut self.mouse_cursor_hidden, false) {
            cx.notify();
        }
    }
}
