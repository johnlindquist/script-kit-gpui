use crate::theme::Theme;

/// Shortcut-recorder-specific layout constants
pub(super) const RECORDER_MODAL_PADDING: f32 = 18.0;
pub(super) const KEY_DISPLAY_HEIGHT: f32 = 44.0;
pub(super) const KEY_DISPLAY_PADDING: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutRecorderFocusedAction {
    Save,
    Clear,
    Cancel,
}

impl ShortcutRecorderFocusedAction {
    pub fn next(self, clear_visible: bool) -> Self {
        match (self, clear_visible) {
            (Self::Save, true) => Self::Clear,
            (Self::Save, false) => Self::Cancel,
            (Self::Clear, _) => Self::Cancel,
            (Self::Cancel, _) => Self::Save,
        }
    }

    pub fn previous(self, clear_visible: bool) -> Self {
        match (self, clear_visible) {
            (Self::Save, _) => Self::Cancel,
            (Self::Clear, _) => Self::Save,
            (Self::Cancel, true) => Self::Clear,
            (Self::Cancel, false) => Self::Save,
        }
    }
}

/// Pre-computed colors for ShortcutRecorder rendering
#[derive(Clone, Copy, Debug)]
pub struct ShortcutRecorderColors {
    /// Background color for the modal overlay
    pub overlay_bg: u32,
    /// Background color for the modal itself
    pub modal_bg: u32,
    /// Border color for the modal
    pub border: u32,
    /// Primary text color
    pub text_primary: u32,
    /// Secondary text color (for descriptions)
    pub text_secondary: u32,
    /// Muted text color (for hints)
    pub text_muted: u32,
    /// Accent color for highlights
    pub accent: u32,
    /// Warning color for conflicts
    pub warning: u32,
    /// Key display area background
    pub key_display_bg: u32,
    /// Keycap background color
    pub keycap_bg: u32,
    /// Keycap border color
    pub keycap_border: u32,
}

impl ShortcutRecorderColors {
    /// Create colors from theme reference
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            overlay_bg: theme.colors.background.main,
            modal_bg: theme.colors.background.main,
            border: theme.colors.ui.border,
            text_primary: theme.colors.text.primary,
            text_secondary: theme.colors.text.secondary,
            text_muted: theme.colors.text.muted,
            accent: theme.colors.accent.selected,
            warning: theme.colors.ui.warning,
            key_display_bg: theme.colors.background.search_box,
            keycap_bg: theme.colors.background.title_bar,
            keycap_border: theme.colors.ui.border,
        }
    }
}

impl Default for ShortcutRecorderColors {
    fn default() -> Self {
        Self::from_theme(&Theme::default())
    }
}

/// Represents a recorded keyboard shortcut
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordedShortcut {
    /// Command key (macOS) / Super key
    pub cmd: bool,
    /// Control key
    pub ctrl: bool,
    /// Option/Alt key
    pub alt: bool,
    /// Shift key
    pub shift: bool,
    /// The actual key pressed (single character or key name)
    pub key: Option<String>,
}

impl RecordedShortcut {
    /// Create a new empty shortcut
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the shortcut has any content
    pub fn is_empty(&self) -> bool {
        !self.cmd && !self.ctrl && !self.alt && !self.shift && self.key.is_none()
    }

    /// Check if only modifiers are set (no key yet)
    pub fn has_only_modifiers(&self) -> bool {
        (self.cmd || self.ctrl || self.alt || self.shift) && self.key.is_none()
    }

    /// Check if the shortcut is complete (has modifiers + key)
    pub fn is_complete(&self) -> bool {
        (self.cmd || self.ctrl || self.alt || self.shift) && self.key.is_some()
    }

    /// Format as a display string using macOS symbols
    pub fn to_display_string(&self) -> String {
        let mut parts = Vec::new();

        if self.ctrl {
            parts.push("⌃");
        }
        if self.alt {
            parts.push("⌥");
        }
        if self.cmd {
            parts.push("⌘");
        }
        if self.shift {
            parts.push("⇧");
        }

        if let Some(ref key) = self.key {
            parts.push(key.as_str());
        }

        parts.join("")
    }

    /// Format as a config string (e.g., "cmd+shift+k")
    pub fn to_config_string(&self) -> String {
        let mut parts = Vec::new();

        if self.ctrl {
            parts.push("ctrl".to_string());
        }
        if self.alt {
            parts.push("alt".to_string());
        }
        if self.cmd {
            parts.push("cmd".to_string());
        }
        if self.shift {
            parts.push("shift".to_string());
        }

        if let Some(ref key) = self.key {
            parts.push(key.to_lowercase());
        }

        parts.join("+")
    }

    /// Get individual keycaps for display through the shared shortcut parser.
    pub fn to_keycaps(&self) -> Vec<String> {
        crate::components::hint_strip::shortcut_tokens_from_hint(&self.to_config_string())
    }

    /// Format as the transient SDK `HotkeyInfo` payload.
    pub fn to_hotkey_info_json(&self) -> String {
        let key = self.key.clone().unwrap_or_default();
        serde_json::json!({
            "key": key,
            "command": self.cmd,
            "shift": self.shift,
            "option": self.alt,
            "control": self.ctrl,
            "shortcut": self.to_display_string(),
            "keyCode": key,
        })
        .to_string()
    }

    /// Format a key for display (uppercase letters, special key names)
    pub(super) fn format_key_display(key: &str) -> String {
        crate::components::hint_strip::shortcut_tokens_from_hint(key).join("")
    }
}

/// Conflict information for a shortcut
#[derive(Clone, Debug)]
pub struct ShortcutConflict {
    /// Name of the command that has this shortcut
    pub command_name: String,
    /// The conflicting shortcut string
    pub shortcut: String,
}

/// Callback types for shortcut recorder
pub type OnSaveCallback = Box<dyn Fn(RecordedShortcut) + 'static>;
pub type OnCancelCallback = Box<dyn Fn() + 'static>;
pub type ConflictChecker = Box<dyn Fn(&RecordedShortcut) -> Option<ShortcutConflict> + 'static>;

/// Actions that can be triggered by the recorder
#[derive(Clone, Debug, PartialEq)]
pub enum RecorderAction {
    /// User wants to save the shortcut
    Save(RecordedShortcut),
    /// User wants to cancel
    Cancel,
}
