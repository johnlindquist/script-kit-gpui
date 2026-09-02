use std::sync::Arc;
use std::time::Instant;

use gpui::{div, prelude::*, px, rgb, rgba, Context, FocusHandle, IntoElement};

use crate::components::overlay_modal::OverlayAnimation;
use crate::components::text_input::{pulse_cursor_bar, TextInputState};
use crate::logging;
use crate::panel::{CURSOR_HEIGHT_LG, CURSOR_WIDTH, PROMPT_INPUT_FIELD_HEIGHT};
use crate::theme::Theme;

use super::types::{
    validate_alias_input, AliasInputAction, AliasInputColors, AliasValidationError,
    ALIAS_INPUT_PLACEHOLDER, ALIAS_VALID_HELP_TEXT, INPUT_PADDING, OVERLAY_BACKDROP_ALPHA,
    OVERLAY_BACKDROP_HOVER_ALPHA,
};
use crate::components::overlay_modal::overlay_color_with_alpha;

#[path = "render.rs"]
mod render;

/// Alias Input Modal Component
///
/// A modal dialog for entering command aliases with full keyboard support.
pub struct AliasInput {
    /// Focus handle for keyboard input - CRITICAL for keyboard events
    pub focus_handle: FocusHandle,
    /// Theme for styling
    pub theme: Arc<Theme>,
    /// Pre-computed colors
    pub colors: AliasInputColors,
    /// Name of the command being configured
    pub command_name: String,
    /// ID of the command being configured
    pub command_id: String,
    /// Text input state (handles selection, cursor, etc.)
    pub input: TextInputState,
    /// Current alias (if editing an existing one)
    pub current_alias: Option<String>,
    /// Pending action consumed by the owning view's mutation observer.
    pub pending_action: Option<AliasInputAction>,
    /// Cursor visibility for blinking (controlled by parent's blink timer)
    pub cursor_visible: bool,
    /// Timestamp for enter animation start (fade/slide-in)
    overlay_animation_started_at: Instant,
    /// Ensures we schedule at most one animation tick task at a time
    overlay_animation_tick_scheduled: bool,
}

impl OverlayAnimation for AliasInput {
    fn overlay_animation_started_at(&self) -> Instant {
        self.overlay_animation_started_at
    }
    fn overlay_animation_tick_scheduled(&self) -> bool {
        self.overlay_animation_tick_scheduled
    }
    fn set_overlay_animation_tick_scheduled(&mut self, scheduled: bool) {
        self.overlay_animation_tick_scheduled = scheduled;
    }
}

impl AliasInput {
    /// Create a new alias input
    /// The focus_handle is created from the entity's own context (cx.focus_handle())
    /// for keyboard events to work properly.
    pub fn new(cx: &mut Context<Self>, theme: Arc<Theme>) -> Self {
        let colors = AliasInputColors::from_theme(&theme);
        // Create focus handle from THIS entity's context - critical for keyboard events
        let focus_handle = cx.focus_handle();
        logging::log("ALIAS_INPUT", "Created AliasInput with new focus handle");
        Self {
            focus_handle,
            theme,
            colors,
            command_name: String::new(),
            command_id: String::new(),
            input: TextInputState::new(),
            current_alias: None,
            pending_action: None,
            cursor_visible: true,
            overlay_animation_started_at: Instant::now(),
            overlay_animation_tick_scheduled: false,
        }
    }

    /// Set the command name
    pub fn with_command_name(mut self, name: impl Into<String>) -> Self {
        self.command_name = name.into();
        self
    }

    /// Set the command ID
    pub fn with_command_id(mut self, id: impl Into<String>) -> Self {
        self.command_id = id.into();
        self
    }

    /// Set the current alias (for editing)
    pub fn with_current_alias(mut self, alias: Option<String>) -> Self {
        if let Some(ref a) = alias {
            self.input.set_text(a.clone());
        }
        self.current_alias = alias;
        self
    }

    /// Set command name (mutable version)
    pub fn set_command_name(&mut self, name: impl Into<String>) {
        self.command_name = name.into();
    }

    /// Set command ID (mutable version)
    pub fn set_command_id(&mut self, id: impl Into<String>) {
        self.command_id = id.into();
    }

    /// Set current alias (mutable version)
    pub fn set_current_alias(&mut self, alias: Option<String>) {
        if let Some(ref a) = alias {
            self.input.set_text(a.clone());
        }
        self.current_alias = alias;
    }

    /// Get the current input text
    pub fn text(&self) -> &str {
        self.input.text()
    }

    pub fn semantic_token(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut token = std::collections::hash_map::DefaultHasher::new();
        self.command_id.hash(&mut token);
        self.current_alias.hash(&mut token);
        self.input.text().hash(&mut token);
        self.input.cursor().hash(&mut token);
        self.input.selection().range().hash(&mut token);
        token.finish()
    }

    pub fn automation_elements(&self) -> Vec<crate::protocol::ElementInfo> {
        use crate::protocol::{ElementContentKind, ElementInfo};
        let mut title = ElementInfo::panel("alias-input-modal-content");
        title.semantic_id = "alias-input-modal-content".into();
        title.text = Some(format!("Alias for \"{}\"", self.command_name));
        let mut input = ElementInfo::input("alias", Some(self.text()), false);
        input.semantic_id = "alias-input-field".into();
        input.focused = None;
        let mut elements = vec![title.redact_text(ElementContentKind::UserContent), input];
        if self.current_alias.is_some() {
            let mut clear = ElementInfo::button(0, "Clear");
            clear.semantic_id = "alias-input:clear".into();
            elements.push(clear);
        }
        let mut cancel = ElementInfo::button(1, "Cancel");
        cancel.semantic_id = "alias-input:cancel".into();
        elements.push(cancel);
        let mut save = ElementInfo::button(2, "Save");
        save.semantic_id = "alias-input:save".into();
        save.action_disabled = self
            .validation_feedback()
            .err()
            .map(|_| "alias_validation_failed".into());
        save.selectable = Some(save.action_disabled.is_none());
        elements.push(save);
        elements
    }

    pub fn activate_semantic_action(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        match id {
            "alias-input:save" => {
                self.validation_feedback()
                    .map_err(|error| error.to_string())?;
                self.save();
            }
            "alias-input:cancel" => self.cancel(),
            "alias-input:clear" if self.current_alias.is_some() => self.clear_alias(),
            _ => return Ok(false),
        }
        cx.notify();
        Ok(true)
    }

    /// Clear the input
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.input.clear();
        logging::log("ALIAS_INPUT", "Input cleared");
        cx.notify();
    }

    /// Handle save action
    pub fn save(&mut self) {
        if let Ok(alias) = validate_alias_input(self.input.text()) {
            logging::log("ALIAS_INPUT", &format!("Saving alias: {}", alias));
            self.pending_action = Some(AliasInputAction::Save(alias));
        }
    }

    /// Handle cancel action
    pub fn cancel(&mut self) {
        logging::log("ALIAS_INPUT", "Alias input cancelled");
        self.pending_action = Some(AliasInputAction::Cancel);
    }

    /// Handle clear action (remove existing alias)
    pub fn clear_alias(&mut self) {
        logging::log("ALIAS_INPUT", "Clearing existing alias");
        self.pending_action = Some(AliasInputAction::Clear);
    }

    /// Take the pending action (returns it and clears the field)
    pub fn take_pending_action(&mut self) -> Option<AliasInputAction> {
        self.pending_action.take()
    }

    /// Update theme
    pub fn update_theme(&mut self, theme: Arc<Theme>) {
        self.colors = AliasInputColors::from_theme(&theme);
        self.theme = theme;
    }

    /// Update cursor visibility (called from parent's blink timer)
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    pub(crate) fn input_hover_border_token(colors: AliasInputColors) -> u32 {
        overlay_color_with_alpha(colors.accent, 0x90)
    }

    pub(crate) fn backdrop_hover_bg_token(colors: AliasInputColors) -> u32 {
        overlay_color_with_alpha(colors.overlay_bg, OVERLAY_BACKDROP_HOVER_ALPHA)
    }

    pub(crate) fn backdrop_bg_token(colors: AliasInputColors) -> u32 {
        overlay_color_with_alpha(colors.overlay_bg, OVERLAY_BACKDROP_ALPHA)
    }

    /// Render the text input field with cursor and selection
    fn render_input_field(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let text = self.input.text();
        let cursor_pos = self.input.cursor();
        let selection = self.input.selection();
        let hover_border_color = rgba(Self::input_hover_border_token(colors));

        // Build the input content with cursor indicator
        let display_text = if text.is_empty() {
            // Placeholder
            div()
                .text_base()
                .text_color(rgb(colors.text_muted))
                .child(ALIAS_INPUT_PLACEHOLDER)
        } else {
            // Show text with cursor position indicator
            let (before, after) = {
                let chars: Vec<char> = text.chars().collect();
                let before: String = chars[..cursor_pos.min(chars.len())].iter().collect();
                let after: String = chars[cursor_pos.min(chars.len())..].iter().collect();
                (before, after)
            };

            // Determine if we have a selection
            let has_selection = !selection.is_empty();
            let (sel_start, sel_end) = selection.range();

            if has_selection {
                // Render with selection highlight
                let chars: Vec<char> = text.chars().collect();
                let before_sel: String = chars[..sel_start].iter().collect();
                let selected: String = chars[sel_start..sel_end].iter().collect();
                let after_sel: String = chars[sel_end..].iter().collect();

                div()
                    .flex()
                    .flex_row()
                    .text_base()
                    .text_color(rgb(colors.text_primary))
                    .child(before_sel)
                    .child(
                        div()
                            .bg(rgba((colors.selection_bg << 8) | 0x80))
                            .rounded(px(2.))
                            .child(selected),
                    )
                    .child(after_sel)
            } else {
                // Render with cursor indicator
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .text_base()
                    .text_color(rgb(colors.text_primary))
                    .child(before)
                    .when(self.cursor_visible, |d| {
                        d.child(pulse_cursor_bar(
                            div()
                                .w(px(CURSOR_WIDTH))
                                .h(px(CURSOR_HEIGHT_LG))
                                .rounded(px(CURSOR_WIDTH / 2.0))
                                .bg(rgb(colors.accent)),
                            "alias-input-cursor-pulse",
                        ))
                    })
                    .child(after)
            }
        };

        div()
            .id("alias-input-field")
            .debug_selector(|| "alias-input-field".into())
            .w_full()
            .h(px(PROMPT_INPUT_FIELD_HEIGHT))
            .px(px(INPUT_PADDING))
            .flex()
            .items_center()
            .bg(rgba((colors.input_bg << 8) | 0xFF))
            .border_1()
            .border_color(rgba((colors.input_border << 8) | 0x80))
            .rounded(px(8.))
            .cursor_text()
            .hover(move |style| style.border_color(hover_border_color))
            .child(display_text)
    }

    fn validation_feedback(&self) -> Result<&'static str, String> {
        match validate_alias_input(self.input.text()) {
            Ok(_) => Ok(ALIAS_VALID_HELP_TEXT),
            Err(AliasValidationError::Empty) => Err("Alias is required".to_string()),
            Err(AliasValidationError::ContainsWhitespace) => {
                Err("Alias cannot contain spaces".to_string())
            }
            Err(AliasValidationError::InvalidCharacters) => {
                Err("Use only letters, numbers, hyphens, or underscores".to_string())
            }
            Err(AliasValidationError::TooLong { max_length }) => {
                Err(format!("Alias must be {max_length} characters or fewer"))
            }
        }
    }
}
