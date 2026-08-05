//! Notes action catalog
//!
//! Defines the built-in Notes actions (labels, ids, icons, shortcuts) consumed
//! by the unified CommandBar (Cmd+K) and the notes action builders. The legacy
//! `NotesActionsPanel` overlay was removed; the CommandBar in
//! `src/notes/window/panels.rs` owns presentation and keyboard handling now.

use crate::designs::icon_variations::IconName;

/// Available actions in the Notes actions panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesAction {
    /// Create a new note
    NewNote,
    /// Duplicate the current note
    DuplicateNote,
    /// Open the note browser/picker
    BrowseNotes,
    /// Toggle rendered Markdown preview
    TogglePreview,
    /// Cycle note sort order
    CycleSortMode,
    /// Open trash view
    OpenTrash,
    /// Empty all notes from trash
    EmptyTrash,
    /// Return from trash view to active notes
    BackToNotes,
    /// Navigate to the previous note history entry
    HistoryBack,
    /// Navigate to the next note history entry
    HistoryForward,
    /// Search within the current note
    FindInNote,
    /// Copy note content as a formatted export
    CopyNoteAs,
    /// Copy deeplink to the current note
    CopyDeeplink,
    /// Copy quicklink to the current note
    CreateQuicklink,
    /// Copy notes that link to the current note
    CopyBacklinks,
    /// Export note content
    Export,
    /// Move list item (current line) up
    MoveListItemUp,
    /// Move list item (current line) down
    MoveListItemDown,
    /// Open formatting commands
    Format,
    /// Delete the current note (soft delete / move to trash)
    DeleteNote,
    /// Restore a note from trash
    RestoreNote,
    /// Permanently delete a note from trash
    PermanentlyDeleteNote,
    /// Toggle auto-sizing (window grows/shrinks with content)
    EnableAutoSizing,
    /// Reset the window to its default position on the active display
    ResetWindowPosition,
    /// Send the current note content to Agent Chat
    SendToAi,
    /// Panel was cancelled (Escape pressed)
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesActionSurface {
    Editor,
    Preview,
    Trash,
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotesActionContext {
    pub surface: NotesActionSurface,
    pub has_current_note: bool,
    pub auto_sizing_enabled: bool,
}

impl NotesActionContext {
    pub const fn editor(has_current_note: bool, auto_sizing_enabled: bool) -> Self {
        Self {
            surface: NotesActionSurface::Editor,
            has_current_note,
            auto_sizing_enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesActionAvailability {
    Enabled,
    Disabled { reason: &'static str },
}

impl NotesActionAvailability {
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub const fn disabled_reason(self) -> Option<&'static str> {
        match self {
            Self::Enabled => None,
            Self::Disabled { reason } => Some(reason),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesActionConfirmation {
    None,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesActionDescriptor {
    pub action: NotesAction,
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub availability: NotesActionAvailability,
    pub destructive: bool,
    pub confirmation: NotesActionConfirmation,
    pub semantic_action_id: String,
}

impl NotesActionDescriptor {
    pub fn disabled_reason(&self) -> Option<&'static str> {
        self.availability.disabled_reason()
    }
}

impl NotesAction {
    /// Get the display label for this action
    pub fn label(&self) -> &'static str {
        match self {
            NotesAction::NewNote => "New Note",
            NotesAction::DuplicateNote => "Duplicate Note",
            NotesAction::BrowseNotes => "Switch Note",
            NotesAction::TogglePreview => "Toggle Preview",
            NotesAction::CycleSortMode => "Cycle Sort",
            NotesAction::OpenTrash => "Open Trash",
            NotesAction::EmptyTrash => "Empty Trash",
            NotesAction::BackToNotes => "Back to Notes",
            NotesAction::HistoryBack => "History Back",
            NotesAction::HistoryForward => "History Forward",
            NotesAction::FindInNote => "Find in Note",
            NotesAction::CopyNoteAs => "Copy Note as Markdown",
            NotesAction::CopyDeeplink => "Copy Deeplink",
            NotesAction::CreateQuicklink => "Create Quicklink",
            NotesAction::CopyBacklinks => "Copy Backlinks",
            NotesAction::Export => "Copy as HTML",
            NotesAction::MoveListItemUp => "Move List Item Up",
            NotesAction::MoveListItemDown => "Move List Item Down",
            NotesAction::Format => "Format...",
            NotesAction::DeleteNote => "Delete Note",
            NotesAction::RestoreNote => "Restore Note",
            NotesAction::PermanentlyDeleteNote => "Delete Permanently",
            NotesAction::EnableAutoSizing => "Toggle Auto-Sizing",
            NotesAction::ResetWindowPosition => "Reset Window Position",
            NotesAction::SendToAi => "Ask AI About This Note",
            NotesAction::Cancel => "Cancel",
        }
    }

    /// Get the hint-format shortcut string for this action (e.g. "cmd+n", "escape").
    ///
    /// This is the single source of truth for shortcut tokens; all rendering and
    /// display methods derive from this via the shared tokenizer in `hint_strip`.
    pub fn shortcut_hint(&self) -> Option<&'static str> {
        match self {
            NotesAction::NewNote => Some("cmd+n"),
            NotesAction::DuplicateNote => Some("cmd+d"),
            NotesAction::BrowseNotes => Some("cmd+p"),
            NotesAction::TogglePreview => Some("shift+cmd+p"),
            NotesAction::CycleSortMode => Some("shift+cmd+s"),
            NotesAction::OpenTrash => Some("shift+cmd+t"),
            NotesAction::EmptyTrash => None,
            NotesAction::BackToNotes => None,
            NotesAction::HistoryBack => Some("cmd+["),
            NotesAction::HistoryForward => Some("cmd+]"),
            NotesAction::FindInNote => Some("cmd+f"),
            NotesAction::CopyNoteAs => Some("shift+cmd+c"),
            NotesAction::CopyDeeplink => Some("shift+cmd+d"),
            NotesAction::CreateQuicklink => Some("shift+cmd+l"),
            NotesAction::CopyBacklinks => None,
            NotesAction::Export => Some("shift+cmd+e"),
            NotesAction::MoveListItemUp => Some("ctrl+cmd+up"),
            NotesAction::MoveListItemDown => Some("ctrl+cmd+down"),
            NotesAction::Format => None,
            NotesAction::DeleteNote => Some("shift+cmd+backspace"),
            NotesAction::RestoreNote => Some("cmd+z"),
            NotesAction::PermanentlyDeleteNote => None,
            NotesAction::EnableAutoSizing => None,
            NotesAction::ResetWindowPosition => None,
            NotesAction::SendToAi => Some("cmd+enter"),
            NotesAction::Cancel => Some("escape"),
        }
    }

    /// Get normalized shortcut tokens via the shared tokenizer.
    pub fn shortcut_tokens(&self) -> Vec<String> {
        self.shortcut_hint()
            .map(crate::components::hint_strip::shortcut_tokens_from_hint)
            .unwrap_or_default()
    }

    /// Get the formatted shortcut display string
    pub fn shortcut_display(&self) -> String {
        self.shortcut_tokens().join("")
    }

    /// Get the icon for this action (uses local IconName from designs module)
    pub fn icon(&self) -> IconName {
        match self {
            NotesAction::NewNote => IconName::Plus,
            NotesAction::DuplicateNote => IconName::Copy,
            NotesAction::BrowseNotes => IconName::FolderOpen,
            NotesAction::TogglePreview => IconName::Code,
            NotesAction::CycleSortMode => IconName::Refresh,
            NotesAction::OpenTrash => IconName::Trash,
            NotesAction::EmptyTrash => IconName::Trash,
            NotesAction::BackToNotes => IconName::FolderOpen,
            NotesAction::HistoryBack => IconName::Refresh,
            NotesAction::HistoryForward => IconName::ArrowRight,
            NotesAction::FindInNote => IconName::MagnifyingGlass,
            NotesAction::CopyNoteAs => IconName::Copy,
            NotesAction::CopyDeeplink => IconName::ArrowRight,
            NotesAction::CreateQuicklink => IconName::Star,
            NotesAction::CopyBacklinks => IconName::FolderOpen,
            NotesAction::Export => IconName::ArrowRight,
            NotesAction::MoveListItemUp => IconName::ArrowUp,
            NotesAction::MoveListItemDown => IconName::ArrowDown,
            NotesAction::Format => IconName::Code,
            NotesAction::DeleteNote => IconName::Trash,
            NotesAction::RestoreNote => IconName::Refresh,
            NotesAction::PermanentlyDeleteNote => IconName::Trash,
            NotesAction::EnableAutoSizing => IconName::Settings,
            NotesAction::ResetWindowPosition => IconName::Refresh,
            NotesAction::SendToAi => IconName::BoltFilled,
            NotesAction::Cancel => IconName::Close,
        }
    }

    /// Get action ID for lookup
    pub fn id(&self) -> &'static str {
        match self {
            NotesAction::NewNote => "new_note",
            NotesAction::DuplicateNote => "duplicate_note",
            NotesAction::BrowseNotes => "browse_notes",
            NotesAction::TogglePreview => "toggle_preview",
            NotesAction::CycleSortMode => "cycle_sort_mode",
            NotesAction::OpenTrash => "open_trash",
            NotesAction::EmptyTrash => "empty_trash",
            NotesAction::BackToNotes => "back_to_notes",
            NotesAction::HistoryBack => "history_back",
            NotesAction::HistoryForward => "history_forward",
            NotesAction::FindInNote => "find_in_note",
            NotesAction::CopyNoteAs => "copy_note_as",
            NotesAction::CopyDeeplink => "copy_deeplink",
            NotesAction::CreateQuicklink => "create_quicklink",
            NotesAction::CopyBacklinks => "copy_backlinks",
            NotesAction::Export => "export",
            NotesAction::MoveListItemUp => "move_list_item_up",
            NotesAction::MoveListItemDown => "move_list_item_down",
            NotesAction::Format => "format",
            NotesAction::DeleteNote => "delete_note",
            NotesAction::RestoreNote => "restore_note",
            NotesAction::PermanentlyDeleteNote => "permanently_delete_note",
            NotesAction::EnableAutoSizing => "toggle_auto_sizing",
            NotesAction::ResetWindowPosition => "reset_window_position",
            NotesAction::SendToAi => "send_to_ai",
            NotesAction::Cancel => "cancel",
        }
    }

    pub const fn catalog() -> &'static [NotesAction] {
        &[
            NotesAction::NewNote,
            NotesAction::DuplicateNote,
            NotesAction::BrowseNotes,
            NotesAction::TogglePreview,
            NotesAction::CycleSortMode,
            NotesAction::OpenTrash,
            NotesAction::EmptyTrash,
            NotesAction::BackToNotes,
            NotesAction::HistoryBack,
            NotesAction::HistoryForward,
            NotesAction::FindInNote,
            NotesAction::CopyNoteAs,
            NotesAction::CopyDeeplink,
            NotesAction::CreateQuicklink,
            NotesAction::CopyBacklinks,
            NotesAction::Export,
            NotesAction::MoveListItemUp,
            NotesAction::MoveListItemDown,
            NotesAction::Format,
            NotesAction::DeleteNote,
            NotesAction::RestoreNote,
            NotesAction::PermanentlyDeleteNote,
            NotesAction::EnableAutoSizing,
            NotesAction::ResetWindowPosition,
            NotesAction::SendToAi,
        ]
    }

    pub fn semantic_action_id(self) -> String {
        format!("notes.action.{}", self.id())
    }

    pub fn descriptor(self, context: NotesActionContext) -> Option<NotesActionDescriptor> {
        if !notes_action_is_visible(self, context) {
            return None;
        }
        let destructive = matches!(
            self,
            NotesAction::DeleteNote | NotesAction::EmptyTrash | NotesAction::PermanentlyDeleteNote
        );
        Some(NotesActionDescriptor {
            action: self,
            id: self.id(),
            label: match self {
                NotesAction::EnableAutoSizing if context.auto_sizing_enabled => {
                    "Disable Auto-Sizing"
                }
                NotesAction::EnableAutoSizing => "Enable Auto-Sizing",
                _ => self.label(),
            },
            shortcut: self.shortcut_hint(),
            availability: NotesActionAvailability::Enabled,
            destructive,
            confirmation: if destructive {
                NotesActionConfirmation::Required
            } else {
                NotesActionConfirmation::None
            },
            semantic_action_id: self.semantic_action_id(),
        })
    }
}

fn notes_action_is_visible(action: NotesAction, context: NotesActionContext) -> bool {
    use NotesAction::*;
    use NotesActionSurface::*;

    match context.surface {
        ReadOnly => matches!(action, BrowseNotes | ResetWindowPosition),
        Trash => match action {
            NewNote | BrowseNotes | TogglePreview | HistoryBack | HistoryForward | BackToNotes
            | EmptyTrash | EnableAutoSizing | ResetWindowPosition => true,
            RestoreNote | PermanentlyDeleteNote => context.has_current_note,
            _ => false,
        },
        Editor | Preview => match action {
            NewNote | BrowseNotes | TogglePreview | HistoryBack | HistoryForward
            | CycleSortMode | OpenTrash | EnableAutoSizing | ResetWindowPosition => true,
            DuplicateNote | DeleteNote | CopyNoteAs | CopyDeeplink | CreateQuicklink
            | CopyBacklinks | Export | SendToAi => context.has_current_note,
            FindInNote | MoveListItemUp | MoveListItemDown | Format => {
                context.has_current_note && context.surface == Editor
            }
            _ => false,
        },
    }
}

pub fn notes_action_descriptors(context: NotesActionContext) -> Vec<NotesActionDescriptor> {
    const DISPLAY_ORDER: &[NotesAction] = &[
        NotesAction::NewNote,
        NotesAction::RestoreNote,
        NotesAction::PermanentlyDeleteNote,
        NotesAction::DuplicateNote,
        NotesAction::DeleteNote,
        NotesAction::BrowseNotes,
        NotesAction::TogglePreview,
        NotesAction::HistoryBack,
        NotesAction::HistoryForward,
        NotesAction::BackToNotes,
        NotesAction::EmptyTrash,
        NotesAction::CycleSortMode,
        NotesAction::OpenTrash,
        NotesAction::FindInNote,
        NotesAction::Format,
        NotesAction::MoveListItemUp,
        NotesAction::MoveListItemDown,
        NotesAction::CopyNoteAs,
        NotesAction::CopyDeeplink,
        NotesAction::CreateQuicklink,
        NotesAction::CopyBacklinks,
        NotesAction::Export,
        NotesAction::SendToAi,
        NotesAction::EnableAutoSizing,
        NotesAction::ResetWindowPosition,
    ];

    DISPLAY_ORDER
        .iter()
        .copied()
        .filter_map(|action| action.descriptor(context))
        .collect()
}

pub fn notes_action_for_id(
    context: NotesActionContext,
    action_id: &str,
) -> Option<NotesActionDescriptor> {
    notes_action_descriptors(context)
        .into_iter()
        .find(|descriptor| descriptor.id == action_id)
}

fn notes_action_for_canonical_shortcut(
    context: NotesActionContext,
    canonical: &str,
) -> Option<NotesActionDescriptor> {
    let mut matches = notes_action_descriptors(context)
        .into_iter()
        .filter(|descriptor| {
            descriptor.availability.is_enabled()
                && descriptor
                    .shortcut
                    .map(crate::components::hint_strip::canonical_shortcut_hint)
                    .is_some_and(|shortcut| shortcut == canonical)
        });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

pub fn notes_action_for_keystroke(
    context: NotesActionContext,
    key: &str,
    modifiers: &gpui::Modifiers,
) -> Option<NotesActionDescriptor> {
    let canonical = crate::shortcuts::keystroke_to_shortcut(key, modifiers);
    notes_action_for_canonical_shortcut(context, &canonical)
}

// Panel sizing constants and `panel_height_for_rows` were removed: the
// detached CommandBar window is sized exclusively by the shared
// `compute_popup_height` / `actions_window_dynamic_height` formula in
// `crate::actions::window`, driven by `crate::actions::constants` and the
// actions popup theme tokens. Do not reintroduce a parallel formula here.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notes_action_labels() {
        assert_eq!(NotesAction::NewNote.label(), "New Note");
        assert_eq!(NotesAction::DuplicateNote.label(), "Duplicate Note");
        assert_eq!(NotesAction::BrowseNotes.label(), "Switch Note");
        assert_eq!(NotesAction::FindInNote.label(), "Find in Note");
        assert_eq!(NotesAction::CopyNoteAs.label(), "Copy Note as Markdown");
        assert_eq!(NotesAction::CopyDeeplink.label(), "Copy Deeplink");
        assert_eq!(NotesAction::CreateQuicklink.label(), "Create Quicklink");
        assert_eq!(NotesAction::Export.label(), "Copy as HTML");
        assert_eq!(NotesAction::MoveListItemUp.label(), "Move List Item Up");
        assert_eq!(NotesAction::MoveListItemDown.label(), "Move List Item Down");
        assert_eq!(NotesAction::Format.label(), "Format...");
    }

    #[test]
    fn test_notes_action_shortcuts() {
        assert_eq!(NotesAction::NewNote.shortcut_display(), "⌘N");
        assert_eq!(NotesAction::DuplicateNote.shortcut_display(), "⌘D");
        assert_eq!(NotesAction::BrowseNotes.shortcut_display(), "⌘P");
        assert_eq!(NotesAction::FindInNote.shortcut_display(), "⌘F");
        assert_eq!(NotesAction::CopyNoteAs.shortcut_display(), "⇧⌘C");
        assert_eq!(NotesAction::CopyDeeplink.shortcut_display(), "⇧⌘D");
        assert_eq!(NotesAction::CreateQuicklink.shortcut_display(), "⇧⌘L");
        assert_eq!(NotesAction::Export.shortcut_display(), "⇧⌘E");
        assert_eq!(NotesAction::MoveListItemUp.shortcut_display(), "⌃⌘↑");
        assert_eq!(NotesAction::MoveListItemDown.shortcut_display(), "⌃⌘↓");
        assert_eq!(NotesAction::OpenTrash.shortcut_display(), "⇧⌘T");
        assert_eq!(NotesAction::Format.shortcut_display(), "");
        assert_eq!(NotesAction::DeleteNote.shortcut_display(), "⇧⌘⌫");
        assert_eq!(NotesAction::SendToAi.shortcut_display(), "⌘↵");
    }

    #[test]
    fn test_shortcut_hint_normalizes_cancel_and_movement() {
        // Cancel renders as the normalized escape glyph, not "Esc"
        assert_eq!(NotesAction::Cancel.shortcut_tokens(), vec!["⎋"]);
        assert_eq!(NotesAction::Cancel.shortcut_display(), "⎋");

        // Movement shortcuts normalize through the shared tokenizer
        assert_eq!(
            NotesAction::MoveListItemUp.shortcut_tokens(),
            vec!["⌃", "⌘", "↑"]
        );
        assert_eq!(
            NotesAction::MoveListItemDown.shortcut_tokens(),
            vec!["⌃", "⌘", "↓"]
        );

        // Delete normalizes backspace glyph
        assert_eq!(
            NotesAction::DeleteNote.shortcut_tokens(),
            vec!["⇧", "⌘", "⌫"]
        );

        // PermanentlyDeleteNote has no shortcut
        assert!(NotesAction::PermanentlyDeleteNote.shortcut_hint().is_none());
        assert!(NotesAction::PermanentlyDeleteNote
            .shortcut_tokens()
            .is_empty());
        assert_eq!(NotesAction::PermanentlyDeleteNote.shortcut_display(), "");
    }

    #[test]
    fn test_shortcut_hint_covers_all_actions() {
        // Destructive/navigation toggles without a stable global chord remain
        // CommandBar-only; every other catalog action has a shortcut hint.
        for action in NotesAction::catalog() {
            if !matches!(
                action,
                NotesAction::EmptyTrash
                    | NotesAction::BackToNotes
                    | NotesAction::CopyBacklinks
                    | NotesAction::Format
                    | NotesAction::PermanentlyDeleteNote
                    | NotesAction::EnableAutoSizing
                    | NotesAction::ResetWindowPosition
            ) {
                assert!(
                    action.shortcut_hint().is_some(),
                    "{:?} should have a shortcut hint",
                    action
                );
            }
        }
    }

    #[test]
    fn catalog_is_complete_and_unique() {
        let expected = [
            NotesAction::NewNote,
            NotesAction::DuplicateNote,
            NotesAction::BrowseNotes,
            NotesAction::TogglePreview,
            NotesAction::CycleSortMode,
            NotesAction::OpenTrash,
            NotesAction::EmptyTrash,
            NotesAction::BackToNotes,
            NotesAction::HistoryBack,
            NotesAction::HistoryForward,
            NotesAction::FindInNote,
            NotesAction::CopyNoteAs,
            NotesAction::CopyDeeplink,
            NotesAction::CreateQuicklink,
            NotesAction::CopyBacklinks,
            NotesAction::Export,
            NotesAction::MoveListItemUp,
            NotesAction::MoveListItemDown,
            NotesAction::Format,
            NotesAction::DeleteNote,
            NotesAction::RestoreNote,
            NotesAction::PermanentlyDeleteNote,
            NotesAction::EnableAutoSizing,
            NotesAction::ResetWindowPosition,
            NotesAction::SendToAi,
        ];
        assert_eq!(NotesAction::catalog(), expected);
        for (index, action) in NotesAction::catalog().iter().enumerate() {
            assert!(
                !NotesAction::catalog()[index + 1..].contains(action),
                "duplicate catalog action: {action:?}"
            );
        }
    }

    #[test]
    fn test_notes_action_ids() {
        assert_eq!(NotesAction::NewNote.id(), "new_note");
        assert_eq!(NotesAction::DuplicateNote.id(), "duplicate_note");
        assert_eq!(NotesAction::BrowseNotes.id(), "browse_notes");
        assert_eq!(NotesAction::FindInNote.id(), "find_in_note");
        assert_eq!(NotesAction::CopyNoteAs.id(), "copy_note_as");
        assert_eq!(NotesAction::CopyDeeplink.id(), "copy_deeplink");
        assert_eq!(NotesAction::CreateQuicklink.id(), "create_quicklink");
        assert_eq!(NotesAction::Export.id(), "export");
        assert_eq!(NotesAction::MoveListItemUp.id(), "move_list_item_up");
        assert_eq!(NotesAction::MoveListItemDown.id(), "move_list_item_down");
        assert_eq!(NotesAction::Format.id(), "format");
    }

    fn contexts() -> [NotesActionContext; 6] {
        [
            NotesActionContext::editor(false, false),
            NotesActionContext::editor(true, true),
            NotesActionContext {
                surface: NotesActionSurface::Preview,
                has_current_note: true,
                auto_sizing_enabled: true,
            },
            NotesActionContext {
                surface: NotesActionSurface::Trash,
                has_current_note: false,
                auto_sizing_enabled: true,
            },
            NotesActionContext {
                surface: NotesActionSurface::Trash,
                has_current_note: true,
                auto_sizing_enabled: true,
            },
            NotesActionContext {
                surface: NotesActionSurface::ReadOnly,
                has_current_note: true,
                auto_sizing_enabled: true,
            },
        ]
    }

    #[test]
    fn every_catalog_action_has_one_descriptor_in_an_applicable_mode() {
        for action in NotesAction::catalog() {
            let count = contexts()
                .into_iter()
                .filter(|context| action.descriptor(*context).is_some())
                .count();
            assert!(count > 0, "{action:?} has no applicable descriptor");
        }
    }

    #[test]
    fn every_mode_has_unique_ids_semantics_and_normalized_shortcuts() {
        for context in contexts() {
            let descriptors = notes_action_descriptors(context);
            let mut ids = std::collections::HashSet::new();
            let mut semantics = std::collections::HashSet::new();
            let mut shortcuts = std::collections::HashSet::new();
            for descriptor in descriptors {
                assert!(ids.insert(descriptor.id), "duplicate id in {context:?}");
                assert!(
                    semantics.insert(descriptor.semantic_action_id.clone()),
                    "duplicate semantic id in {context:?}"
                );
                assert_eq!(
                    descriptor.semantic_action_id,
                    format!("notes.action.{}", descriptor.id)
                );
                if let Some(shortcut) = descriptor.shortcut {
                    let canonical =
                        crate::components::hint_strip::canonical_shortcut_hint(shortcut);
                    assert!(
                        shortcuts.insert(canonical.clone()),
                        "duplicate {canonical} in {context:?}"
                    );
                }
                assert_eq!(
                    descriptor.availability.is_enabled(),
                    descriptor.disabled_reason().is_none()
                );
            }
        }
    }

    #[test]
    fn exact_c07_shortcuts_resolve_to_one_current_action() {
        let context = NotesActionContext::editor(true, true);
        let mut command = gpui::Modifiers::default();
        command.platform = true;
        for (key, expected) in [
            ("enter", NotesAction::SendToAi),
            ("n", NotesAction::NewNote),
            ("p", NotesAction::BrowseNotes),
            ("f", NotesAction::FindInNote),
            ("[", NotesAction::HistoryBack),
            ("]", NotesAction::HistoryForward),
            ("bracketleft", NotesAction::HistoryBack),
            ("bracketright", NotesAction::HistoryForward),
        ] {
            assert_eq!(
                notes_action_for_keystroke(context, key, &command)
                    .map(|descriptor| descriptor.action),
                Some(expected),
                "wrong action for {key}"
            );
        }

        let mut shift_command = command;
        shift_command.shift = true;
        assert_eq!(
            notes_action_for_keystroke(context, "backspace", &shift_command)
                .map(|descriptor| descriptor.action),
            Some(NotesAction::DeleteNote)
        );
        assert_eq!(
            notes_action_for_keystroke(context, "t", &shift_command)
                .map(|descriptor| descriptor.action),
            Some(NotesAction::OpenTrash)
        );
        assert_eq!(
            notes_action_for_keystroke(context, "a", &shift_command),
            None
        );
        assert_eq!(
            notes_action_for_keystroke(context, "delete", &shift_command),
            None
        );
        assert!(NotesAction::Format.shortcut_hint().is_none());
    }

    #[test]
    fn every_enabled_advertised_shortcut_resolves_to_its_descriptor() {
        for context in contexts() {
            for descriptor in notes_action_descriptors(context) {
                let Some(shortcut) = descriptor.shortcut else {
                    continue;
                };
                if !descriptor.availability.is_enabled() {
                    continue;
                }
                let canonical = crate::components::hint_strip::canonical_shortcut_hint(shortcut);
                assert_eq!(
                    notes_action_for_canonical_shortcut(context, &canonical)
                        .map(|matched| matched.action),
                    Some(descriptor.action),
                    "advertised shortcut {canonical} did not resolve in {context:?}"
                );
            }
        }
    }

    #[test]
    fn every_destructive_descriptor_requires_confirmation() {
        for context in contexts() {
            for descriptor in notes_action_descriptors(context) {
                assert_eq!(
                    descriptor.confirmation == NotesActionConfirmation::Required,
                    descriptor.destructive,
                    "confirmation drift for {:?}",
                    descriptor.action
                );
            }
        }
    }
}
