use gpui::{Context, Window};

use crate::ui_foundation::UiActionSpec;

use super::NotesEditor;

/// Toolbar action routed through the shared notes editor formatting entry points.
#[derive(Clone, Copy)]
pub struct NotesEditorToolbarAction {
    pub(crate) spec: UiActionSpec,
    pub(crate) run: fn(&mut NotesEditor, &mut Window, &mut Context<NotesEditor>),
}

fn apply_bold(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.insert_formatting("**", "**", window, cx);
}

fn apply_italic(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.insert_formatting("_", "_", window, cx);
}

fn apply_heading(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.cycle_heading(window, cx);
}

fn apply_bullet_list(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.toggle_bullet_list(window, cx);
}

fn apply_numbered_list(
    editor: &mut NotesEditor,
    window: &mut Window,
    cx: &mut Context<NotesEditor>,
) {
    editor.toggle_numbered_list(window, cx);
}

fn apply_inline_code(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.insert_formatting("`", "`", window, cx);
}

fn apply_code_block(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.insert_formatting("\n```\n", "\n```", window, cx);
}

fn apply_strikethrough(
    editor: &mut NotesEditor,
    window: &mut Window,
    cx: &mut Context<NotesEditor>,
) {
    editor.insert_formatting("~~", "~~", window, cx);
}

fn apply_checklist(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.toggle_checklist(window, cx);
}

fn apply_link(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.insert_formatting("[", "](url)", window, cx);
}

fn apply_rule(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.insert_horizontal_rule(window, cx);
}

fn apply_blockquote(editor: &mut NotesEditor, window: &mut Window, cx: &mut Context<NotesEditor>) {
    editor.insert_formatting("\n> ", "", window, cx);
}

pub const NOTES_EDITOR_TOOLBAR_ACTIONS: [NotesEditorToolbarAction; 12] = [
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "bold",
            label: "B",
            shortcut: None,
        },
        run: apply_bold,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "italic",
            label: "I",
            shortcut: None,
        },
        run: apply_italic,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "heading",
            label: "H",
            shortcut: None,
        },
        run: apply_heading,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "list",
            label: "\u{2022}",
            shortcut: None,
        },
        run: apply_bullet_list,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "numbered-list",
            label: "1.",
            shortcut: None,
        },
        run: apply_numbered_list,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "code",
            label: "</>",
            shortcut: None,
        },
        run: apply_inline_code,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "codeblock",
            label: "```",
            shortcut: None,
        },
        run: apply_code_block,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "strikethrough",
            label: "S\u{0336}",
            shortcut: None,
        },
        run: apply_strikethrough,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "checklist",
            label: "\u{2610}",
            shortcut: None,
        },
        run: apply_checklist,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "link",
            label: "\u{1F517}",
            shortcut: None,
        },
        run: apply_link,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "rule",
            label: "\u{2015}",
            shortcut: None,
        },
        run: apply_rule,
    },
    NotesEditorToolbarAction {
        spec: UiActionSpec {
            id: "blockquote",
            label: ">",
            shortcut: None,
        },
        run: apply_blockquote,
    },
];

pub fn notes_editor_toolbar_action_by_id(id: &str) -> Option<NotesEditorToolbarAction> {
    NOTES_EDITOR_TOOLBAR_ACTIONS
        .iter()
        .copied()
        .find(|action| action.spec.id == id)
}

pub fn notes_editor_toolbar_action_title(id: &str) -> &'static str {
    match id {
        "bold" => "Bold",
        "italic" => "Italic",
        "heading" => "Heading",
        "list" => "Bullet List",
        "numbered-list" => "Numbered List",
        "code" => "Inline Code",
        "codeblock" => "Code Block",
        "strikethrough" => "Strikethrough",
        "checklist" => "Checklist",
        "link" => "Link",
        "rule" => "Horizontal Rule",
        "blockquote" => "Blockquote",
        _ => "Markdown Action",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn toolbar_action_ids_are_unique() {
        let mut ids = HashSet::new();
        for action in NOTES_EDITOR_TOOLBAR_ACTIONS {
            assert!(
                ids.insert(action.spec.id),
                "duplicate notes editor toolbar action id: {}",
                action.spec.id
            );
        }
    }
}
