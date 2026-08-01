use crate::actions::constants::POPUP_MAX_HEIGHT;

/// Represents a single action item in the actions menu.
///
/// Actions are created by builder functions in `builders.rs` or converted from
/// SDK-provided `ProtocolAction` messages. Each action has a unique identifier,
/// display title, and category for grouping.
///
/// # Action ID Convention
///
/// - Built-in actions: snake_case IDs (`edit_script`, `copy_path`, etc.)
/// - SDK actions: Use the `name` field from ProtocolAction as-is
/// - Scriptlet actions: Prefixed with `scriptlet_action:` followed by command
///
/// # Routing via has_action
///
/// The `has_action` field determines how actions are executed:
/// - `false` (default for built-ins): Handle locally in Rust via `handle_action()`
/// - `true` (SDK actions): Send `ActionTriggered` message to script for handling
///
/// Note: The routing logic in `handle_action()` may also read from the original
/// `ProtocolAction` for SDK-provided actions to ensure consistency.
///
/// # Examples
///
/// ```ignore
/// // Built-in action (has_action defaults to false)
/// let action = Action::new(
///     "edit_script",
///     "Edit Script",
///     Some("Open in $EDITOR".to_string()),
///     ActionCategory::ScriptContext,
/// ).with_shortcut("⌘E");
///
/// // Scriptlet action (has_action=true for SDK handling)
/// let mut action = Action::new(
///     "scriptlet_action:copy-to-clipboard",
///     "Copy to Clipboard",
///     None,
///     ActionCategory::ScriptContext,
/// );
/// action.has_action = true;
/// action.value = Some("copy-to-clipboard".to_string());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
enum ActionAvailability {
    Enabled,
    Disabled { reason: String },
}

impl ActionAvailability {
    pub fn disabled(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        assert!(
            !reason.trim().is_empty(),
            "disabled actions require a non-empty reason"
        );
        Self::Disabled { reason }
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub fn disabled_reason(&self) -> Option<&str> {
        match self {
            Self::Enabled => None,
            Self::Disabled { reason } => Some(reason.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    /// Unique identifier for action routing.
    /// Built-in IDs use snake_case (e.g., `edit_script`, `copy_path`).
    /// SDK action IDs match the ProtocolAction name.
    pub id: String,

    /// Display title shown in the actions menu
    pub title: String,

    /// Optional description shown below the title
    #[allow(dead_code)]
    pub description: Option<String>,

    /// Category for grouping actions in the menu
    pub category: ActionCategory,

    /// Explicit execution availability. Disabled actions remain visible and
    /// selectable so their explanation can be read, but cannot activate.
    availability: ActionAvailability,

    /// Optional keyboard shortcut hint (e.g., "⌘E", "⇧⌘K")
    /// Displayed as a badge next to the action title
    pub shortcut: Option<String>,

    /// Pre-tokenized shortcut parts for compact inline rendering.
    /// Filled once during action construction to avoid per-render parsing.
    pub shortcut_tokens: Option<Vec<String>>,

    /// Routing flag: if true, send ActionTriggered to SDK; if false, handle locally.
    /// Built-in actions default to false. SDK actions with handlers set this to true.
    #[allow(dead_code)]
    pub has_action: bool,

    /// Optional value to submit when action is triggered.
    /// For scriptlet actions, this contains the command to execute.
    #[allow(dead_code)]
    pub value: Option<String>,

    /// Optional icon to display next to the action title
    pub icon: Option<IconName>,

    /// Section/group name for display (used with SectionStyle::Headers)
    pub section: Option<String>,

    // === Cached lowercase fields for fast filtering (performance optimization) ===
    // These are pre-computed on Action creation to avoid repeated to_lowercase() calls
    // during search/filter operations which happen on every keystroke.
    /// Cached lowercase title for fast filtering
    pub title_lower: String,

    /// Cached lowercase description for fast filtering
    pub description_lower: Option<String>,

    /// Cached lowercase shortcut for fast filtering
    pub shortcut_lower: Option<String>,
}

/// Configuration for how the search input is positioned
#[allow(dead_code)] // Public API - will be used by AI window integration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchPosition {
    /// Search input at top (AI chat style - list grows downward)
    Top,
    /// Search input at bottom (main menu style - list grows upward)
    #[default]
    Bottom,
    /// No search input (external search handling)
    Hidden,
}

/// Configuration for how sections/categories are displayed
#[allow(dead_code)] // Public API - will be used by AI window integration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectionStyle {
    /// Show text headers for sections — spacing alone defines groups per .impeccable.md
    #[default]
    Headers,
    /// Legacy: inline separators between categories (deprecated — no production caller should use this)
    #[allow(dead_code)]
    Separators,
    /// No section indicators
    None,
}

/// Configuration for dialog anchor position during resize
#[allow(dead_code)] // Public API - will be used by AI window integration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnchorPosition {
    /// Dialog grows/shrinks from top (content pinned to top)
    Top,
    /// Dialog grows/shrinks from bottom (content pinned to bottom)
    #[default]
    Bottom,
}

/// Complete configuration for ActionsDialog appearance and behavior
#[allow(dead_code)] // Public API - will be used by AI window integration
#[derive(Debug, Clone)]
pub struct ActionsDialogConfig {
    /// Position of search input
    pub search_position: SearchPosition,
    /// How to display section/category divisions
    pub section_style: SectionStyle,
    /// Which edge the dialog anchors to during resize
    pub anchor: AnchorPosition,
    /// Whether to show icons for actions (if available)
    pub show_icons: bool,
    /// Legacy footer flag. Actions dialogs intentionally render without a footer;
    /// shortcuts stay inline with each action row.
    pub show_footer: bool,
    /// Placeholder shown when the actions search is empty
    pub search_placeholder: Option<String>,
    /// Whether to render the separate context header row
    pub show_context_header: bool,
    /// Maximum popup height before the action list scrolls.
    pub max_height: f32,
    /// Whether rows render `Action::description` as a subtitle line.
    ///
    /// Action menus stay title-only (compact rows); switcher-style hosts
    /// (e.g. the Notes recent-note switcher) opt in to show preview/metadata
    /// with main-list row anatomy.
    pub show_subtitles: bool,
}

impl Default for ActionsDialogConfig {
    fn default() -> Self {
        Self {
            search_position: SearchPosition::default(),
            section_style: SectionStyle::default(),
            anchor: AnchorPosition::default(),
            show_icons: false,
            show_footer: false,
            search_placeholder: None,
            show_context_header: true,
            max_height: POPUP_MAX_HEIGHT,
            show_subtitles: false,
        }
    }
}

/// Category for grouping actions in the actions menu.
///
/// Actions are organized by category to help users find relevant options:
/// - `ScriptContext`: Actions specific to the currently focused script/item
/// - `ScriptOps`: Script management operations (test-only legacy category)
/// - `GlobalOps`: Application-wide actions (test-only legacy category)
///
/// Currently, most actions are `ScriptContext` since they operate on the
/// focused list item. Test-only categories remain for compatibility coverage.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionCategory {
    /// Actions specific to the currently focused script/item.
    /// Examples: Run, Edit, Copy Path, Configure Shortcut, Reset Ranking
    ScriptContext,

    /// Script management operations (test-only reserved category).
    #[cfg(test)]
    ScriptOps,

    /// Application-wide actions surfaced when no script-context entries apply.
    GlobalOps,

    /// Terminal actions (Clear, Copy, Paste, Scroll, etc.)
    #[allow(dead_code)]
    Terminal,
}

impl Action {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        category: ActionCategory,
    ) -> Self {
        let title_str = title.into();
        let title_lower = title_str.to_lowercase();
        let description_lower = description.as_ref().map(|d| d.to_lowercase());

        Action {
            id: id.into(),
            title: title_str,
            description,
            category,
            availability: ActionAvailability::Enabled,
            shortcut: None,
            shortcut_tokens: None,
            has_action: false,
            value: None,
            icon: None,
            section: None,
            // Pre-compute lowercase for fast filtering
            title_lower,
            description_lower,
            shortcut_lower: None,
        }
    }

    pub fn disabled(mut self, reason: impl Into<String>) -> Self {
        self.availability = ActionAvailability::disabled(reason);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.availability.is_enabled()
    }

    pub fn disabled_reason(&self) -> Option<&str> {
        self.availability.disabled_reason()
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        let shortcut_str = shortcut.into();
        let shortcut_tokens =
            crate::components::hint_strip::shortcut_tokens_from_hint(&shortcut_str);
        // trace!: action vectors are rebuilt on hot paths (render-time
        // keybinding sync); info-level logging here is per-frame I/O.
        tracing::trace!(
            target: "script_kit::actions",
            action_id = %self.id,
            shortcut = %shortcut_str,
            token_count = shortcut_tokens.len(),
            "action_shortcut_tokens_cached"
        );
        self.shortcut_lower = Some(shortcut_str.to_lowercase());
        self.shortcut_tokens = Some(shortcut_tokens);
        self.shortcut = Some(shortcut_str);
        self
    }

    #[allow(dead_code)] // Public API - used by get_ai_command_bar_actions
    /// Add an optional shortcut to the action
    pub fn with_shortcut_opt(mut self, shortcut: Option<String>) -> Self {
        if let Some(s) = shortcut {
            let shortcut_tokens = crate::components::hint_strip::shortcut_tokens_from_hint(&s);
            tracing::trace!(
                target: "script_kit::actions",
                action_id = %self.id,
                shortcut = %s,
                token_count = shortcut_tokens.len(),
                "action_shortcut_tokens_cached"
            );
            self.shortcut_lower = Some(s.to_lowercase());
            self.shortcut_tokens = Some(shortcut_tokens);
            self.shortcut = Some(s);
        }
        self
    }
    pub fn with_icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    #[allow(dead_code)] // Public API - used by get_ai_command_bar_actions
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.section = Some(section.into());
        self
    }
}
