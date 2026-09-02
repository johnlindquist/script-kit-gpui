use gpui::{div, prelude::*, px, rgb, rgba, FontWeight, IntoElement};

use super::types::{KEY_DISPLAY_HEIGHT, KEY_DISPLAY_PADDING};
use super::ShortcutRecorder;

impl ShortcutRecorder {
    /// Get keycaps for live display - shows current modifiers while recording,
    /// or the final recorded shortcut when complete
    pub(super) fn get_display_keycaps(&self) -> Vec<String> {
        if self.shortcut.is_complete() {
            self.shortcut.to_keycaps()
        } else if self.is_recording {
            let mut keycaps = Vec::new();
            if self.current_modifiers.control {
                keycaps.push("⌃".to_string());
            }
            if self.current_modifiers.alt {
                keycaps.push("⌥".to_string());
            }
            if self.current_modifiers.platform {
                keycaps.push("⌘".to_string());
            }
            if self.current_modifiers.shift {
                keycaps.push("⇧".to_string());
            }
            keycaps
        } else {
            self.shortcut.to_keycaps()
        }
    }

    /// Render the live shortcut preview as inline glyphs without a boxed shell.
    /// The surrounding modal provides visual framing; the preview area stays quiet.
    pub(super) fn render_key_display(&self) -> impl IntoElement {
        let colors = self.colors;
        let keycaps = self.get_display_keycaps();

        crate::components::hint_strip::emit_shortcut_chrome_audit(
            "shortcut_recorder_key_display",
            "compact-inline-unboxed",
        );

        let content = if keycaps.is_empty() {
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgba((colors.text_primary << 8) | 0x66))
                .child("Press keys")
                .into_any_element()
        } else {
            crate::components::hint_strip::render_inline_shortcut_keys(
                keycaps.iter().map(String::as_str),
                crate::components::hint_strip::whisper_inline_shortcut_colors(
                    rgba((colors.text_primary << 8) | 0xCC).into(),
                    rgba((colors.text_primary << 8) | 0xFF).into(),
                    true,
                ),
            )
        };

        div()
            .debug_selector(|| "shortcut-key-display".into())
            .w_full()
            .h(px(KEY_DISPLAY_HEIGHT))
            .px(px(KEY_DISPLAY_PADDING))
            .rounded(px(6.0))
            .bg(rgba((colors.text_primary << 8) | 0x0C))
            .border_1()
            .border_color(rgba((colors.text_primary << 8) | 0x18))
            .flex()
            .items_center()
            .justify_center()
            .child(content)
    }

    /// Render conflict warning if present
    pub(super) fn render_conflict_warning(&self) -> impl IntoElement {
        let colors = self.colors;

        if let Some(ref conflict) = self.conflict {
            div()
                .debug_selector(|| "shortcut-conflict-warning".into())
                .w_full()
                .mt(px(8.))
                .text_xs()
                .text_color(rgb(colors.warning))
                .text_center()
                .child(format!("Conflicts with \"{}\"", conflict.command_name))
                .into_any_element()
        } else {
            div().into_any_element()
        }
    }
}
