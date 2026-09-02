use std::ops::Range;

use super::model::{Note, NoteId};

const MIN_PREFIX_CHARS: usize = 2;
const MAX_SOURCE_CHARS_CURRENT_NOTE: usize = 24_000;
const MAX_SOURCE_CHARS_OTHER_NOTE: usize = 8_000;
const MAX_SOURCE_CHARS_CLIPBOARD: usize = 4_000;
const MAX_OTHER_NOTES: usize = 20;
const MAX_CLIPBOARD_TEXTS: usize = 20;
pub(crate) const MAX_NOTES_GHOST_SUFFIX_CHARS: usize = 96;
const MIN_ACCEPT_CONFIDENCE: f32 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotesGhostSourceKind {
    CurrentNote,
    OtherNote,
    Clipboard,
    /// Note title completion inside an unclosed `[[wiki link`.
    NoteTitle,
    /// On-device LLM continuation grounded in brain recall (see
    /// [`super::ghost_llm`]). Produced by the debounced side-channel, never by
    /// the deterministic scorer.
    Brain,
}

impl NotesGhostSourceKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::CurrentNote => "currentNote",
            Self::OtherNote => "otherNote",
            Self::Clipboard => "clipboard",
            Self::NoteTitle => "noteTitle",
            Self::Brain => "brain",
        }
    }

    fn priority(self) -> i32 {
        match self {
            Self::CurrentNote => 3,
            Self::OtherNote => 2,
            Self::Clipboard => 1,
            Self::NoteTitle => 4,
            // Brain predictions never enter the deterministic scorer; the
            // priority only matters if a candidate ever flows through it.
            Self::Brain => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotesGhostClipboardText {
    pub(crate) text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NotesGhostPrediction {
    pub(crate) query_prefix: String,
    pub(crate) suffix: String,
    pub(crate) source_kind: NotesGhostSourceKind,
    pub(crate) source_rank: usize,
    pub(crate) confidence: f32,
    pub(crate) generation: u64,
    pub(crate) accepts_tab: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct NotesGhostInput<'a> {
    pub(crate) editor_text: &'a str,
    pub(crate) selection: Range<usize>,
    pub(crate) selected_note_id: Option<NoteId>,
    pub(crate) notes: &'a [Note],
    pub(crate) clipboard_texts: &'a [NotesGhostClipboardText],
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LinePrefix {
    pub(crate) text: String,
}

#[derive(Clone, Debug)]
struct NotesGhostCandidate {
    text: String,
    source_kind: NotesGhostSourceKind,
    source_rank: usize,
    after_markdown_list_marker: bool,
}

#[derive(Clone, Debug)]
struct ScoredCandidate<'a> {
    candidate: &'a NotesGhostCandidate,
    suffix: String,
    score: i32,
}

#[derive(Debug, Default)]
pub(crate) struct NotesGhostCorpus {
    sources: Vec<CachedGhostSource>,
}

#[derive(Debug)]
struct CachedGhostSource {
    text: String,
    at_limit: bool,
    source_kind: NotesGhostSourceKind,
    source_rank: usize,
    candidates: Vec<NotesGhostCandidate>,
}

impl NotesGhostCorpus {
    fn update_source(
        &mut self,
        index: usize,
        source_kind: NotesGhostSourceKind,
        source_rank: usize,
        text: &str,
        max_chars: usize,
    ) {
        if self.sources.get(index).is_some_and(|source| {
            source.source_kind == source_kind
                && source.source_rank == source_rank
                && if source.at_limit {
                    text.starts_with(&source.text)
                } else {
                    text == source.text
                }
        }) {
            return;
        }

        // Compare the actual bounded text, not timestamps: imported or externally
        // edited notes can change content without advancing their stored revision.
        let text = truncate_chars(text, max_chars);
        let at_limit = text.chars().count() == max_chars;
        let mut candidates = Vec::new();
        collect_candidates_from_text(source_kind, source_rank, &text, &mut candidates);
        let source = CachedGhostSource {
            text,
            at_limit,
            source_kind,
            source_rank,
            candidates,
        };
        if let Some(slot) = self.sources.get_mut(index) {
            *slot = source;
        } else {
            self.sources.push(source);
        }
    }
}

pub(crate) fn compute_notes_ghost_prediction(
    input: NotesGhostInput<'_>,
    corpus: &mut NotesGhostCorpus,
) -> Option<NotesGhostPrediction> {
    let line = current_line_prefix(input.editor_text, input.selection.clone())?;

    // Wiki-link completion takes precedence: inside an unclosed `[[…` the
    // only sensible completion is a note title.
    if let Some(prediction) = wiki_link_title_prediction(&line, &input) {
        return Some(prediction);
    }

    if line.text.trim().chars().count() < MIN_PREFIX_CHARS {
        return None;
    }

    corpus.update_source(
        0,
        NotesGhostSourceKind::CurrentNote,
        0,
        input.editor_text,
        MAX_SOURCE_CHARS_CURRENT_NOTE,
    );
    let mut source_count = 1;

    for (rank, note) in input
        .notes
        .iter()
        .filter(|note| Some(note.id) != input.selected_note_id)
        .filter(|note| note.deleted_at.is_none())
        .take(MAX_OTHER_NOTES)
        .enumerate()
    {
        corpus.update_source(
            source_count,
            NotesGhostSourceKind::OtherNote,
            rank,
            &note.content,
            MAX_SOURCE_CHARS_OTHER_NOTE,
        );
        source_count += 1;
    }

    for (rank, clip) in input
        .clipboard_texts
        .iter()
        .take(MAX_CLIPBOARD_TEXTS)
        .enumerate()
    {
        corpus.update_source(
            source_count,
            NotesGhostSourceKind::Clipboard,
            rank,
            &clip.text,
            MAX_SOURCE_CHARS_CLIPBOARD,
        );
        source_count += 1;
    }

    corpus.sources.truncate(source_count);
    let best = corpus
        .sources
        .iter()
        .flat_map(|source| &source.candidates)
        .filter_map(|candidate| score_candidate(&line.text, candidate))
        .min_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| {
                    b.candidate
                        .source_kind
                        .priority()
                        .cmp(&a.candidate.source_kind.priority())
                })
                .then_with(|| a.candidate.source_rank.cmp(&b.candidate.source_rank))
                .then_with(|| a.suffix.chars().count().cmp(&b.suffix.chars().count()))
                .then_with(|| a.candidate.text.cmp(&b.candidate.text))
        })?;
    let confidence = (best.score as f32 / 1100.0).clamp(0.0, 1.0);
    if confidence < MIN_ACCEPT_CONFIDENCE {
        return None;
    }

    Some(NotesGhostPrediction {
        query_prefix: line.text,
        suffix: best.suffix,
        source_kind: best.candidate.source_kind,
        source_rank: best.candidate.source_rank,
        confidence,
        generation: input.generation,
        accepts_tab: true,
    })
}

/// Complete `[[partial` with a matching note title and closing `]]`.
///
/// Deterministic: case-insensitive prefix match against other notes' titles,
/// preferring the shortest (most specific) title, then lexicographic order.
fn wiki_link_title_prediction(
    line: &LinePrefix,
    input: &NotesGhostInput<'_>,
) -> Option<NotesGhostPrediction> {
    let open = line.text.rfind("[[")?;
    let query = &line.text[open + 2..];
    // Already closed or contains a newline-ish boundary: not an open link.
    if query.contains("]]") || query.contains('[') {
        return None;
    }
    if query.trim().is_empty() {
        return None;
    }

    let query_lower = query.to_lowercase();
    let mut matches: Vec<&str> = input
        .notes
        .iter()
        .filter(|note| Some(note.id) != input.selected_note_id)
        .filter(|note| note.deleted_at.is_none())
        .map(|note| note.title.as_str())
        .filter(|title| !title.trim().is_empty())
        .filter(|title| {
            let title_lower = title.to_lowercase();
            title_lower.starts_with(&query_lower) && title_lower != query_lower
        })
        .collect();
    matches.sort_by(|a, b| a.chars().count().cmp(&b.chars().count()).then(a.cmp(b)));

    let title = matches.first()?;
    let completion: String = title.chars().skip(query.chars().count()).collect();
    let suffix = format!("{completion}]]");

    Some(NotesGhostPrediction {
        query_prefix: line.text.clone(),
        suffix,
        source_kind: NotesGhostSourceKind::NoteTitle,
        source_rank: 0,
        confidence: 0.9,
        generation: input.generation,
        accepts_tab: true,
    })
}

pub(crate) fn current_line_prefix(text: &str, selection: Range<usize>) -> Option<LinePrefix> {
    if selection.start != selection.end {
        return None;
    }
    let cursor = selection.start.min(text.len());
    if !text.is_char_boundary(cursor) {
        return None;
    }
    let line_start = text[..cursor].rfind('\n').map_or(0, |idx| idx + 1);
    Some(LinePrefix {
        text: text[line_start..cursor].to_string(),
    })
}

pub(crate) fn first_word_acceptance_suffix(suffix: &str) -> &str {
    let mut saw_non_whitespace = false;
    for (idx, ch) in suffix.char_indices() {
        if ch.is_whitespace() {
            if saw_non_whitespace {
                return &suffix[..idx];
            }
        } else {
            saw_non_whitespace = true;
        }
    }
    suffix
}

fn collect_candidates_from_text(
    source_kind: NotesGhostSourceKind,
    source_rank: usize,
    text: &str,
    out: &mut Vec<NotesGhostCandidate>,
) {
    let mut in_code_fence = false;
    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence || trimmed.is_empty() || looks_sensitive(trimmed) {
            continue;
        }
        let normalized = collapse_spaces(trimmed);
        if normalized.chars().count() < 4 {
            continue;
        }
        out.push(NotesGhostCandidate {
            after_markdown_list_marker: starts_with_markdown_list_marker(trimmed),
            text: cap_completion_chars(&normalized, 160),
            source_kind,
            source_rank,
        });
    }
}

fn score_candidate<'a>(
    prefix: &str,
    candidate: &'a NotesGhostCandidate,
) -> Option<ScoredCandidate<'a>> {
    let prefix = prefix.trim_start();
    let suffix = suffix_for_prefix(prefix, &candidate.text)?;
    if suffix.trim().is_empty() || looks_sensitive(&suffix) {
        return None;
    }
    let suffix = cap_completion_chars(&suffix, MAX_NOTES_GHOST_SUFFIX_CHARS);
    if suffix.trim().is_empty() {
        return None;
    }

    let mut score = match candidate.source_kind {
        NotesGhostSourceKind::CurrentNote => 1000,
        NotesGhostSourceKind::OtherNote => 700,
        NotesGhostSourceKind::Clipboard => 550,
        // Wiki-link completions short-circuit before scoring, but keep them
        // top-ranked if a candidate ever flows through here.
        NotesGhostSourceKind::NoteTitle => 1100,
        // Brain predictions are produced by the LLM side-channel
        // (`super::ghost_llm`), never by this deterministic scorer; rank them
        // between other-note and clipboard if one ever flows through.
        NotesGhostSourceKind::Brain => 600,
    };
    score -= (candidate.source_rank.min(50) as i32) * 4;
    score += 80;
    if candidate.after_markdown_list_marker {
        score += 30;
    }
    if (4..=80).contains(&suffix.chars().count()) {
        score += 40;
    }

    Some(ScoredCandidate {
        candidate,
        suffix,
        score,
    })
}

fn suffix_for_prefix(prefix: &str, candidate: &str) -> Option<String> {
    if !candidate
        .to_lowercase()
        .starts_with(prefix.to_lowercase().as_str())
    {
        return None;
    }
    Some(candidate.chars().skip(prefix.chars().count()).collect())
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn cap_completion_chars(text: &str, max_chars: usize) -> String {
    let mut capped = text.chars().take(max_chars).collect::<String>();
    if let Some(newline) = capped.find('\n') {
        capped.truncate(newline);
    }
    capped
}

fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn starts_with_markdown_list_marker(text: &str) -> bool {
    text.starts_with("- ")
        || text.starts_with("* ")
        || text.starts_with("+ ")
        || text
            .split_once(". ")
            .is_some_and(|(head, _)| head.chars().all(|c| c.is_ascii_digit()))
}

pub(crate) fn looks_sensitive(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("password")
        || lower.contains("secret")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("token=")
        || lower.contains("bearer ")
        || lower.contains("-----BEGIN ")
        || lower.contains("sk-")
        || lower.contains("http://")
        || lower.contains("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(content: &str) -> Note {
        Note::with_content(content)
    }

    fn input<'a>(
        editor_text: &'a str,
        cursor: usize,
        notes: &'a [Note],
        clipboard_texts: &'a [NotesGhostClipboardText],
    ) -> NotesGhostInput<'a> {
        NotesGhostInput {
            editor_text,
            selection: cursor..cursor,
            selected_note_id: notes.first().map(|note| note.id),
            notes,
            clipboard_texts,
            generation: 1,
        }
    }

    #[test]
    fn notes_ghost_returns_current_note_suffix_for_matching_line_prefix() {
        let notes = vec![note("Call Alice about alpha launch blockers\n\nCall Al")];
        let text = notes[0].content.as_str();
        let prediction = compute_notes_ghost_prediction(
            input(text, text.len(), &notes, &[]),
            &mut NotesGhostCorpus::default(),
        )
        .expect("current note line should produce suffix");
        assert_eq!(prediction.source_kind, NotesGhostSourceKind::CurrentNote);
        assert_eq!(prediction.suffix, "ice about alpha launch blockers");
        assert!(prediction.accepts_tab);
    }

    #[test]
    fn notes_ghost_uses_other_note_when_current_note_has_no_candidate() {
        let current = note("Email Da");
        let other = note("Email Dana about beta launch blockers");
        let mut notes = vec![current, other];
        let mut corpus = NotesGhostCorpus::default();
        let text = notes[0].content.as_str();
        let prediction =
            compute_notes_ghost_prediction(input(text, text.len(), &notes, &[]), &mut corpus)
                .expect("other note should produce suffix");
        assert_eq!(prediction.source_kind, NotesGhostSourceKind::OtherNote);
        assert_eq!(prediction.suffix, "na about beta launch blockers");

        let prediction =
            compute_notes_ghost_prediction(input("Email D", 7, &notes, &[]), &mut corpus)
                .expect("typing must reuse the other note with the new prefix");
        assert_eq!(prediction.suffix, "ana about beta launch blockers");

        // An external replacement need not change the note's timestamp.
        notes[1].content = "Email David about release blockers".into();
        let prediction =
            compute_notes_ghost_prediction(input("Email Da", 8, &notes, &[]), &mut corpus)
                .expect("changed source content must replace cached candidates");
        assert_eq!(prediction.suffix, "vid about release blockers");

        let mut selected_other = input("Email Da", 8, &notes, &[]);
        selected_other.selected_note_id = Some(notes[1].id);
        assert!(compute_notes_ghost_prediction(selected_other, &mut corpus).is_none());

        notes[1].deleted_at = Some(chrono::Utc::now());
        assert!(
            compute_notes_ghost_prediction(input("Email Da", 8, &notes, &[]), &mut corpus)
                .is_none()
        );
        notes.truncate(1);
        assert!(
            compute_notes_ghost_prediction(input("Email Da", 8, &notes, &[]), &mut corpus)
                .is_none()
        );
    }

    #[test]
    fn notes_ghost_uses_recent_clipboard_text_when_notes_have_no_candidate() {
        let notes = vec![note("Review pl")];
        let mut clips = vec![NotesGhostClipboardText {
            text: "Review planning packet before launch".to_string(),
        }];
        let text = notes[0].content.as_str();
        let mut corpus = NotesGhostCorpus::default();
        let prediction =
            compute_notes_ghost_prediction(input(text, text.len(), &notes, &clips), &mut corpus)
                .expect("clipboard text should produce suffix");
        assert_eq!(prediction.source_kind, NotesGhostSourceKind::Clipboard);
        assert_eq!(prediction.suffix, "anning packet before launch");

        clips[0].text = "Review platform changes before launch".into();
        let prediction =
            compute_notes_ghost_prediction(input(text, text.len(), &notes, &clips), &mut corpus)
                .expect("clipboard changes must replace cached candidates");
        assert_eq!(prediction.suffix, "atform changes before launch");
        clips.clear();
        assert!(compute_notes_ghost_prediction(
            input(text, text.len(), &notes, &clips),
            &mut corpus
        )
        .is_none());
    }

    #[test]
    fn notes_ghost_rejects_non_collapsed_selection() {
        let notes = vec![note("Call Alice\nCall Al")];
        let text = notes[0].content.as_str();
        let prediction = compute_notes_ghost_prediction(
            NotesGhostInput {
                editor_text: text,
                selection: 0..4,
                selected_note_id: Some(notes[0].id),
                notes: &notes,
                clipboard_texts: &[],
                generation: 1,
            },
            &mut NotesGhostCorpus::default(),
        );
        assert!(prediction.is_none());
    }

    #[test]
    fn notes_ghost_rejects_short_prefix() {
        let notes = vec![note("Call Alice\nC")];
        let text = notes[0].content.as_str();
        assert!(compute_notes_ghost_prediction(
            input(text, text.len(), &notes, &[]),
            &mut NotesGhostCorpus::default()
        )
        .is_none());
    }

    #[test]
    fn notes_ghost_rejects_sensitive_candidates() {
        let notes = vec![note("token=secret-value\n\nTo")];
        let text = notes[0].content.as_str();
        assert!(compute_notes_ghost_prediction(
            input(text, text.len(), &notes, &[]),
            &mut NotesGhostCorpus::default()
        )
        .is_none());
    }

    #[test]
    fn notes_ghost_completes_wiki_link_from_other_note_title() {
        let current = note("Linking to [[Mee");
        let other = note("Meeting Notes\n\nAgenda items");
        let notes = vec![current, other];
        let text = notes[0].content.as_str();
        let prediction = compute_notes_ghost_prediction(
            input(text, text.len(), &notes, &[]),
            &mut NotesGhostCorpus::default(),
        )
        .expect("open wiki link should complete from note title");
        assert_eq!(prediction.source_kind, NotesGhostSourceKind::NoteTitle);
        assert_eq!(prediction.suffix, "ting Notes]]");
        assert!(prediction.accepts_tab);
    }

    #[test]
    fn notes_ghost_wiki_link_prefers_shortest_matching_title() {
        let current = note("See [[Pro");
        let longer = note("Project Roadmap Long Term\n\nbody");
        let shorter = note("Project Plan\n\nbody");
        let notes = vec![current, longer, shorter];
        let text = notes[0].content.as_str();
        let prediction = compute_notes_ghost_prediction(
            input(text, text.len(), &notes, &[]),
            &mut NotesGhostCorpus::default(),
        )
        .expect("open wiki link should complete");
        assert_eq!(prediction.suffix, "ject Plan]]");
    }

    #[test]
    fn notes_ghost_ignores_closed_wiki_links() {
        let current = note("See [[Done Link]] then Cal");
        let other = note("Done Link Extended\n\nbody");
        let notes = vec![current, other];
        let text = notes[0].content.as_str();
        let prediction = compute_notes_ghost_prediction(
            input(text, text.len(), &notes, &[]),
            &mut NotesGhostCorpus::default(),
        );
        // No open `[[`; should not produce a NoteTitle prediction.
        assert!(prediction
            .map(|p| p.source_kind != NotesGhostSourceKind::NoteTitle)
            .unwrap_or(true));
    }

    #[test]
    fn notes_ghost_first_word_acceptance_completes_partial_word() {
        assert_eq!(first_word_acceptance_suffix("ice about alpha"), "ice");
    }

    #[test]
    fn notes_ghost_first_word_acceptance_keeps_leading_space_with_next_word() {
        assert_eq!(first_word_acceptance_suffix(" about alpha"), " about");
    }

    #[test]
    fn notes_ghost_first_word_acceptance_keeps_full_suffix_when_single_word() {
        assert_eq!(first_word_acceptance_suffix("done"), "done");
    }
}
