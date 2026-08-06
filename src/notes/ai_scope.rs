use std::ops::Range;

/// The exact live content a Notes/Today handoff stages in Agent Chat.
///
/// This type carries identity and byte-range shape only. Content remains in the
/// host-owned context part and never enters Debug or serialized receipts.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum NotesAiScope {
    WholeNote {
        document_id: String,
    },
    Selection {
        document_id: String,
        range: Range<usize>,
    },
    CurrentLine {
        document_id: String,
        range: Range<usize>,
    },
    Resource {
        uri_identity: String,
    },
    AttachedNote {
        note_id: String,
    },
}

impl std::fmt::Debug for NotesAiScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotesAiScope")
            .field("kind", &self.kind())
            .field(
                "document_semantic_id_length",
                &self.document_semantic_id().chars().count(),
            )
            .field("range_length", &self.range_length())
            .finish()
    }
}

impl NotesAiScope {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::WholeNote { .. } => "wholeNote",
            Self::Selection { .. } => "selection",
            Self::CurrentLine { .. } => "currentLine",
            Self::Resource { .. } => "resource",
            Self::AttachedNote { .. } => "attachedNote",
        }
    }

    pub(crate) fn document_semantic_id(&self) -> &str {
        match self {
            Self::WholeNote { document_id }
            | Self::Selection { document_id, .. }
            | Self::CurrentLine { document_id, .. } => document_id,
            Self::Resource { uri_identity } => uri_identity,
            Self::AttachedNote { note_id } => note_id,
        }
    }

    pub(crate) fn range(&self) -> Option<&Range<usize>> {
        match self {
            Self::Selection { range, .. } | Self::CurrentLine { range, .. } => Some(range),
            Self::WholeNote { .. } | Self::Resource { .. } | Self::AttachedNote { .. } => None,
        }
    }

    pub(crate) fn range_length(&self) -> Option<usize> {
        self.range()
            .map(|range| range.end.saturating_sub(range.start))
    }
}

/// Resolve Day Page's default Cmd+Enter scope. A non-empty valid selection
/// wins; otherwise only the current line is returned. The returned content is
/// exactly the selected range and never includes surrounding page bytes.
pub(crate) fn selected_or_current_line_scope(
    document_id: impl Into<String>,
    content: &str,
    selection: Range<usize>,
) -> (NotesAiScope, String) {
    let document_id = document_id.into();
    if selection.start < selection.end {
        if let Some(selected) = content.get(selection.clone()) {
            return (
                NotesAiScope::Selection {
                    document_id,
                    range: selection,
                },
                selected.to_string(),
            );
        }
    }

    let cursor = selection.end.min(content.len());
    let range = crate::components::notes_editor::spine::current_line_range(content, cursor);
    let selected = content.get(range.clone()).unwrap_or_default().to_string();
    (NotesAiScope::CurrentLine { document_id, range }, selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_scope_excludes_outside_content() {
        let content = "outside-before\nselected only\noutside-after";
        let start = content.find("selected").expect("selected marker");
        let end = start + "selected only".len();
        let (scope, selected) =
            selected_or_current_line_scope("day:2026-08-05", content, start..end);

        assert_eq!(scope.kind(), "selection");
        assert_eq!(scope.range_length(), Some("selected only".len()));
        assert_eq!(selected, "selected only");
        assert!(!selected.contains("outside"));
    }

    #[test]
    fn collapsed_selection_uses_only_current_line() {
        let content = "outside-before\ncurrent only\noutside-after";
        let cursor = content.find("current").expect("current marker") + 3;
        let (scope, selected) =
            selected_or_current_line_scope("day:2026-08-05", content, cursor..cursor);

        assert_eq!(scope.kind(), "currentLine");
        assert_eq!(selected, "current only");
        assert!(!selected.contains("outside"));
    }
}
