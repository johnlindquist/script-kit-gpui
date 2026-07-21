//! Logical text selection.
//!
//! The authoritative selection is a pair of byte positions into a flattened
//! [`SelectionDocument`] — never pixel geometry and never per-run paint
//! state. Mouse points are hit-tested into document positions the moment an
//! event arrives; painting intersects each run's document range with the
//! logical range; copy slices the document text directly. This is what makes
//! copy exact (whitespace preserved), immune to repaint staleness, correct
//! for both drag directions, and correct for runs that are currently
//! off-screen.

use std::{
    ops::Range,
    sync::{Arc, Mutex},
};

use unicode_segmentation::UnicodeSegmentation;

use crate::text::{
    document::ParsedDocument,
    inline::InlineState,
    node::{BlockNode, Paragraph},
};

/// Selection granularity, chosen from the mouse-down click count and
/// reapplied while the drag extends (macOS-style granular tracking).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SelectionGranularity {
    #[default]
    Grapheme,
    Word,
    Paragraph,
}

impl SelectionGranularity {
    pub(crate) fn from_click_count(count: usize) -> Self {
        match count {
            0..=1 => Self::Grapheme,
            2 => Self::Word,
            _ => Self::Paragraph,
        }
    }
}

/// The active selection: anchor is where the mouse went down, focus follows
/// the drag; both are byte offsets into [`SelectionDocument::text`].
/// `anchor_unit` preserves the initially clicked grapheme/word/paragraph so
/// word- and paragraph-granular drags extend from the whole original unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextSelection {
    pub anchor: usize,
    pub focus: usize,
    pub granularity: SelectionGranularity,
    pub anchor_unit: Range<usize>,
}

impl TextSelection {
    pub(crate) fn begin(document: &SelectionDocument, byte: usize, click_count: usize) -> Self {
        let granularity = SelectionGranularity::from_click_count(click_count);
        let byte = snap_to_grapheme(&document.text, byte);
        let anchor_unit = unit_range_at(document, byte, granularity);
        Self {
            anchor: byte,
            focus: byte,
            granularity,
            anchor_unit,
        }
    }

    /// The selected document range: the plain ordered range for grapheme
    /// granularity, or the union of the anchored unit and the unit under
    /// the focus for word/paragraph granularity.
    pub(crate) fn effective_range(&self, document: &SelectionDocument) -> Range<usize> {
        let lo = self.anchor.min(self.focus).min(document.text.len());
        let hi = self.anchor.max(self.focus).min(document.text.len());
        match self.granularity {
            SelectionGranularity::Grapheme => lo..hi,
            SelectionGranularity::Word | SelectionGranularity::Paragraph => {
                let focus_unit = unit_range_at(document, self.focus, self.granularity);
                let start = self.anchor_unit.start.min(focus_unit.start);
                let end = self.anchor_unit.end.max(focus_unit.end);
                start.min(document.text.len())..end.min(document.text.len())
            }
        }
    }
}

/// The rendered document flattened to plain text, with the byte range each
/// selectable run occupies and the semantic paragraph ranges.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SelectionDocument {
    pub text: String,
    pub runs: Vec<Range<usize>>,
    pub paragraphs: Vec<Range<usize>>,
}

/// Walk the parsed document exactly the way rendering does, flattening it
/// into a [`SelectionDocument`] and stamping each rendered run's document
/// range into its shared [`InlineState`]. Must mirror `Paragraph::render`'s
/// batching: inline children accumulate until an image, the pre-image batch
/// renders through the image node's state, and the trailing batch through
/// the paragraph's own state.
pub(crate) fn build_selection_document(parsed: &ParsedDocument) -> SelectionDocument {
    let mut builder = DocumentBuilder::default();
    for block in &parsed.blocks {
        builder.block(block);
    }
    builder.document
}

#[derive(Default)]
struct DocumentBuilder {
    document: SelectionDocument,
    emitted: bool,
}

impl DocumentBuilder {
    /// Adjacent rendered blocks are separated by exactly one newline in the
    /// flattened text. The separator belongs to no run.
    fn block_separator(&mut self) {
        if self.emitted {
            self.document.text.push('\n');
        }
    }

    fn run(&mut self, state: &Arc<Mutex<InlineState>>, text: &str) {
        let start = self.document.text.len();
        self.document.text.push_str(text);
        let range = start..self.document.text.len();
        state.lock().unwrap().document_range = Some(range.clone());
        self.document.runs.push(range);
    }

    /// Flatten one paragraph (also used by headings and table cells),
    /// mirroring `Paragraph::render`'s image-split batching. Returns the
    /// paragraph's document range when it emitted any text.
    fn paragraph(&mut self, paragraph: &Paragraph) -> Option<Range<usize>> {
        let start = self.document.text.len();
        let mut batch = String::new();
        for child in &paragraph.children {
            if let Some(image) = &child.image {
                if !batch.is_empty() {
                    self.run(&child.state, &batch);
                    batch.clear();
                }
                // Images copy as their alt text (most useful plain-text
                // form); the alt belongs to no run, so it never highlights.
                let alt = image.alt.clone().unwrap_or_default();
                if !alt.is_empty() {
                    self.document.text.push_str(&alt);
                }
            } else {
                batch.push_str(&child.text);
            }
        }
        if !batch.is_empty() {
            self.run(&paragraph.state, &batch);
        }
        let end = self.document.text.len();
        (end > start).then(|| {
            self.emitted = true;
            start..end
        })
    }

    fn block(&mut self, block: &BlockNode) {
        match block {
            BlockNode::Root { children, .. }
            | BlockNode::Blockquote { children, .. }
            | BlockNode::List { children, .. }
            | BlockNode::ListItem { children, .. } => {
                for child in children {
                    self.block(child);
                }
            }
            BlockNode::Paragraph(paragraph) => {
                self.block_separator();
                if let Some(range) = self.paragraph(paragraph) {
                    self.document.paragraphs.push(range);
                }
            }
            BlockNode::Heading { children, .. } => {
                self.block_separator();
                if let Some(range) = self.paragraph(children) {
                    self.document.paragraphs.push(range);
                }
            }
            BlockNode::CodeBlock(code_block) => {
                self.block_separator();
                let code = code_block.code();
                if !code.is_empty() {
                    let start = self.document.text.len();
                    self.run(code_block.state(), &code);
                    self.document
                        .paragraphs
                        .push(start..self.document.text.len());
                    self.emitted = true;
                }
            }
            BlockNode::Table(table) => {
                for row in &table.children {
                    self.block_separator();
                    let row_start = self.document.text.len();
                    let mut first_cell = true;
                    for cell in &row.children {
                        if !first_cell {
                            // Cells separate with a tab, rows with newlines.
                            self.document.text.push('\t');
                        }
                        first_cell = false;
                        self.paragraph(&cell.children);
                    }
                    let row_end = self.document.text.len();
                    if row_end > row_start {
                        self.document.paragraphs.push(row_start..row_end);
                        self.emitted = true;
                    }
                }
            }
            BlockNode::Break { .. }
            | BlockNode::Divider { .. }
            | BlockNode::Definition { .. }
            | BlockNode::Unknown => {}
        }
    }
}

/// Snap a byte offset to the start of the extended grapheme cluster that
/// contains it (offsets at or past the end snap to the end).
pub(crate) fn snap_to_grapheme(text: &str, byte: usize) -> usize {
    if byte >= text.len() {
        return text.len();
    }
    let mut start = 0;
    for (index, grapheme) in text.grapheme_indices(true) {
        let end = index + grapheme.len();
        if byte < end {
            start = index;
            break;
        }
        start = end;
    }
    start
}

/// The extended grapheme cluster range containing `byte`. Exercised by the
/// boundary tests today; keyboard caret movement (Shift+Arrow) will consume
/// it next.
#[allow(dead_code)]
pub(crate) fn grapheme_range_at(text: &str, byte: usize) -> Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let byte = byte.min(text.len().saturating_sub(1));
    for (index, grapheme) in text.grapheme_indices(true) {
        let end = index + grapheme.len();
        if byte < end {
            return index..end;
        }
    }
    text.len()..text.len()
}

/// The UAX #29 word-boundary segment containing `byte`. Whitespace and
/// punctuation runs are their own segments, so double-clicking a space
/// selects the space run and a comma selects itself.
pub(crate) fn word_range_at(text: &str, byte: usize) -> Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let byte = byte.min(text.len().saturating_sub(1));
    for (index, segment) in text.split_word_bound_indices() {
        let end = index + segment.len();
        if byte < end {
            return index..end;
        }
    }
    text.len()..text.len()
}

/// The semantic paragraph containing `byte`, else the nearest one, else the
/// whole document.
pub(crate) fn paragraph_range_at(document: &SelectionDocument, byte: usize) -> Range<usize> {
    if let Some(range) = document
        .paragraphs
        .iter()
        .find(|range| range.contains(&byte) || range.start == byte)
    {
        return range.clone();
    }
    document
        .paragraphs
        .iter()
        .min_by_key(|range| {
            if byte < range.start {
                range.start - byte
            } else {
                byte.saturating_sub(range.end)
            }
        })
        .cloned()
        .unwrap_or(0..document.text.len())
}

fn unit_range_at(
    document: &SelectionDocument,
    byte: usize,
    granularity: SelectionGranularity,
) -> Range<usize> {
    match granularity {
        SelectionGranularity::Grapheme => {
            let byte = snap_to_grapheme(&document.text, byte);
            byte..byte
        }
        SelectionGranularity::Word => word_range_at(&document.text, byte),
        SelectionGranularity::Paragraph => paragraph_range_at(document, byte),
    }
}

/// Intersect the document-level selection with one run's document range,
/// yielding the run-local byte range to highlight.
pub(crate) fn run_local_selection(
    selection: &Range<usize>,
    run: &Range<usize>,
) -> Option<Range<usize>> {
    let start = selection.start.max(run.start);
    let end = selection.end.min(run.end);
    (start < end).then(|| (start - run.start)..(end - run.start))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> SelectionDocument {
        SelectionDocument {
            text: text.to_string(),
            runs: vec![0..text.len()],
            paragraphs: vec![0..text.len()],
        }
    }

    #[test]
    fn grapheme_ranges_never_split_clusters() {
        for (text, expected) in [
            ("e\u{301}x", "e\u{301}"),
            ("👍🏽!", "👍🏽"),
            ("👩‍👩‍👧‍👦!", "👩‍👩‍👧‍👦"),
            ("🇺🇸x", "🇺🇸"),
            ("1️⃣x", "1️⃣"),
            ("\u{301}x", "\u{301}"),
        ] {
            let range = grapheme_range_at(text, 0);
            assert_eq!(&text[range.clone()], expected, "text {text:?}");
            assert!(text.is_char_boundary(range.start));
            assert!(text.is_char_boundary(range.end));
            // An interior byte of the cluster snaps back to its start.
            let interior = range.start + 1;
            if interior < range.end {
                assert_eq!(snap_to_grapheme(text, interior), range.start);
            }
        }
    }

    #[test]
    fn word_ranges_follow_uax29() {
        let text = "hello there, it's fine don’t stop";
        let word_at = |byte: usize| &text[word_range_at(text, byte)];
        assert_eq!(word_at(2), "hello");
        assert_eq!(word_at(7), "there");
        assert_eq!(word_at(11), ",");
        assert_eq!(word_at(14), "it's");
        // Curly apostrophes stay inside the word (UAX #29 MidLetter).
        assert_eq!(word_at(24), "don’t");
        assert_eq!(word_at(5), " ");
    }

    #[test]
    fn granular_drag_extends_from_the_anchored_unit() {
        let document = doc("one two three");
        // Double-click "two", then drag into "three".
        let mut selection = TextSelection::begin(&document, 5, 2);
        assert_eq!(&document.text[selection.effective_range(&document)], "two");
        selection.focus = 9;
        assert_eq!(
            &document.text[selection.effective_range(&document)],
            "two three"
        );
        // Drag back into "one": the anchored word stays whole.
        selection.focus = 1;
        assert_eq!(
            &document.text[selection.effective_range(&document)],
            "one two"
        );
        // Return the focus inside the anchor word: just the word again.
        selection.focus = 6;
        assert_eq!(&document.text[selection.effective_range(&document)], "two");
    }

    #[test]
    fn document_selection_intersects_runs_without_paint_state() {
        // Document: "alpha\nbeta" as two runs with a separator.
        let selection = 2..8;
        assert_eq!(run_local_selection(&selection, &(0..5)), Some(2..5));
        assert_eq!(run_local_selection(&selection, &(6..10)), Some(0..2));
        assert_eq!(run_local_selection(&selection, &(8..10)), None);
    }

    #[test]
    fn paragraph_granularity_uses_semantic_paragraphs() {
        let document = SelectionDocument {
            text: "first\nsecond".to_string(),
            runs: vec![0..5, 6..12],
            paragraphs: vec![0..5, 6..12],
        };
        let selection = TextSelection::begin(&document, 2, 3);
        assert_eq!(
            &document.text[selection.effective_range(&document)],
            "first"
        );
        let mut selection = selection;
        selection.focus = 8;
        assert_eq!(
            &document.text[selection.effective_range(&document)],
            "first\nsecond"
        );
    }

    #[test]
    fn whitespace_selections_survive_exactly() {
        let document = doc(" hi \t there ");
        let mut selection = TextSelection::begin(&document, 0, 1);
        selection.focus = 6;
        assert_eq!(
            &document.text[selection.effective_range(&document)],
            " hi \t "
        );
    }
}
