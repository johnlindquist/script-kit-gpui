use super::types::*;

const PROMPT_BUILDER_SIGILS: &[char] = &['@', '/', '|', '.', '>'];
/// `-` opens the flow search, but ONLY where the scanner is already at a
/// segment start (input start, or after another segment's boundary). Unlike
/// the prompt-builder sigils it is deliberately NOT a free-text/greedy break
/// character: hyphens and dashes are everyday prose (`buy milk - urgent`,
/// `;todo dry-cleaning - asap`), so a `-` mid-sentence must never split a
/// capture or free-text segment into a flow search.
const FLOW_SIGIL: char = '-';
const CAPTURE_SIGIL: char = ';';
const FILTER_SIGIL: char = ':';
const MODE_EXIT_SIGILS: &[char] = &['~', '?', '!'];

/// Parse an input string into a sequence of Spine segments.
///
/// The parser splits the input at sigil boundaries while preserving free text.
/// It does NOT validate segments against known entities — that is the resolution
/// step, done separately by catalog lookups.
pub fn parse_spine(input: &str) -> SpineParse {
    if input.is_empty() {
        return SpineParse {
            segments: vec![],
            input: input.to_string(),
        };
    }

    // Mode exit: if the input starts with ~, ?, or !, the entire input is one
    // ModeExit segment. These sigils exit the grammar flow entirely.
    // (`>` is a prompt-builder sigil — project cwd — not a mode exit.)
    if let Some(first) = input.chars().next() {
        if MODE_EXIT_SIGILS.contains(&first) {
            let rest = &input[first.len_utf8()..];
            return SpineParse {
                segments: vec![SpineSegment {
                    kind: SpineSegmentKind::ModeExit {
                        sigil: first,
                        rest: rest.to_string(),
                    },
                    byte_range: 0..input.len(),
                    raw: input.to_string(),
                    resolution: SpineSegmentResolution::Unresolved,
                }],
                input: input.to_string(),
            };
        }
    }

    // Postfix capture (`todo; buy milk`): the canonical capture spelling
    // since the A4 decision. The spine must classify it the same way as the
    // `;` prefix form so highlighting, projection ownership, and prompt-plan
    // blocking stay consistent with the capture grammar that executes it.
    if let Some((target, args)) = split_postfix_capture_head(input) {
        return SpineParse {
            segments: vec![SpineSegment {
                kind: SpineSegmentKind::Capture {
                    target: target.to_string(),
                    args: args.trim_start().to_string(),
                },
                byte_range: 0..input.len(),
                raw: input.to_string(),
                resolution: SpineSegmentResolution::Unresolved,
            }],
            input: input.to_string(),
        };
    }

    let segments = split_segments(input);

    SpineParse {
        segments,
        input: input.to_string(),
    }
}

/// Postfix capture head: `target;` at the very start of input, where the
/// target is a plain identifier (`[A-Za-z0-9_-]+`). Mirrors
/// `menu_syntax::parse::split_postfix_capture_head` so both grammars agree
/// on what counts as a capture.
fn split_postfix_capture_head(input: &str) -> Option<(&str, &str)> {
    let semi = input.find(';')?;
    let head = &input[..semi];
    if head.is_empty()
        || !head
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some((head, &input[semi + 1..]))
}

/// Given a parsed `SpineParse` and a cursor byte offset, compute which segment
/// the cursor is inside and what query to use for list projection.
pub fn project_cursor(parse: &SpineParse, cursor_byte: usize) -> SpineCursorProjection {
    if parse.segments.is_empty() {
        return SpineCursorProjection {
            active_segment_index: 0,
            active_segment_kind: SpineSegmentKind::FreeText,
            active_query: String::new(),
            is_tail: false,
            has_prompt_segments: false,
        };
    }

    let has_prompt_segments = parse.segments.iter().any(is_prompt_builder_segment);

    // Find the segment containing the cursor. If cursor is at a boundary,
    // it belongs to the segment whose range includes it (left-biased for
    // range end, since ranges are half-open).
    let mut active_index = parse.segments.len() - 1;
    for (i, seg) in parse.segments.iter().enumerate() {
        if cursor_byte >= seg.byte_range.start && cursor_byte <= seg.byte_range.end {
            active_index = i;
            break;
        }
    }

    let active_seg = &parse.segments[active_index];
    let active_query = extract_active_query(active_seg, cursor_byte);
    let is_tail = matches!(active_seg.kind, SpineSegmentKind::FreeText) && has_prompt_segments;

    SpineCursorProjection {
        active_segment_index: active_index,
        active_segment_kind: active_seg.kind.clone(),
        active_query,
        is_tail,
        has_prompt_segments,
    }
}

fn is_prompt_builder_segment(seg: &SpineSegment) -> bool {
    matches!(
        seg.kind,
        SpineSegmentKind::ContextMention { .. }
            | SpineSegmentKind::SlashCommand { .. }
            | SpineSegmentKind::Profile { .. }
            | SpineSegmentKind::Style { .. }
            | SpineSegmentKind::ProjectCwd { .. }
            | SpineSegmentKind::Flow { .. }
    )
}

/// Split the input into segments at sigil boundaries.
///
/// Prompt-builder sigils (`@`, `/`, `|`, `.`) claim only the sigil + the
/// immediately following whitespace-delimited word.  Greedy sigils (`;`, `:`)
/// consume until the next boundary-sigil or end of input.  Everything else is
/// free text.  Segments do NOT include inter-segment whitespace.
fn split_segments(input: &str) -> Vec<SpineSegment> {
    let mut segments = Vec::new();
    let bytes = input.as_bytes();
    let len = input.len();
    let mut pos = 0;

    while pos < len {
        // Skip whitespace between segments
        if bytes[pos] == b' ' {
            pos += 1;
            continue;
        }

        let Some(ch) = char_at(input, pos) else {
            break;
        };
        let ch_len = ch.len_utf8();
        let at_boundary = pos == 0 || bytes[pos - 1] == b' ';

        let is_prompt_sigil =
            (PROMPT_BUILDER_SIGILS.contains(&ch) || ch == FLOW_SIGIL) && at_boundary;
        let is_greedy_sigil = (ch == CAPTURE_SIGIL || ch == FILTER_SIGIL) && at_boundary;

        if is_prompt_sigil {
            // Prompt-builder sigil: claim sigil + one word (up to next space)
            let seg_start = pos;
            pos += ch_len;
            // Consume non-space chars (the value word)
            while pos < len && bytes[pos] != b' ' {
                let Some(next_ch) = char_at(input, pos) else {
                    break;
                };
                pos += next_ch.len_utf8();
            }
            let raw = input[seg_start..pos].to_string();
            let rest = &input[seg_start + ch_len..pos];
            let kind = classify_sigil_segment(ch, rest);
            segments.push(SpineSegment {
                kind,
                byte_range: seg_start..pos,
                raw,
                resolution: SpineSegmentResolution::Unresolved,
            });
        } else if is_greedy_sigil {
            // Greedy sigil (`;`, `:`): consume until next boundary-sigil or end
            let seg_start = pos;
            pos += ch_len;
            while pos < len {
                let Some(next_ch) = char_at(input, pos) else {
                    break;
                };
                let next_at_boundary = bytes[pos - 1] == b' ';
                let next_is_any_sigil = PROMPT_BUILDER_SIGILS.contains(&next_ch)
                    || next_ch == CAPTURE_SIGIL
                    || next_ch == FILTER_SIGIL;
                if next_is_any_sigil && next_at_boundary {
                    break;
                }
                pos += next_ch.len_utf8();
            }
            // Trim trailing whitespace from greedy segment
            let mut seg_end = pos;
            while seg_end > seg_start && bytes[seg_end - 1] == b' ' {
                seg_end -= 1;
            }
            let raw = input[seg_start..seg_end].to_string();
            let rest = &input[seg_start + ch_len..seg_end];
            let kind = classify_sigil_segment(ch, rest);
            segments.push(SpineSegment {
                kind,
                byte_range: seg_start..seg_end,
                raw,
                resolution: SpineSegmentResolution::Unresolved,
            });
        } else {
            // Free text: consume words until we hit a boundary-sigil
            let seg_start = pos;
            while pos < len {
                if bytes[pos] == b' ' {
                    // Check if the next non-space char is a sigil at boundary
                    let peek = pos + 1;
                    if peek < len {
                        let Some(next_ch) = char_at(input, peek) else {
                            break;
                        };
                        let next_is_sigil = PROMPT_BUILDER_SIGILS.contains(&next_ch)
                            || next_ch == CAPTURE_SIGIL
                            || next_ch == FILTER_SIGIL;
                        if next_is_sigil {
                            break;
                        }
                    }
                }
                let Some(next_ch) = char_at(input, pos) else {
                    break;
                };
                pos += next_ch.len_utf8();
            }
            // Trim trailing whitespace
            let mut seg_end = pos;
            while seg_end > seg_start && bytes[seg_end - 1] == b' ' {
                seg_end -= 1;
            }
            let raw = input[seg_start..seg_end].to_string();
            segments.push(SpineSegment {
                kind: SpineSegmentKind::FreeText,
                byte_range: seg_start..seg_end,
                raw,
                resolution: SpineSegmentResolution::Unresolved,
            });
        }
    }

    segments
}

fn char_at(input: &str, pos: usize) -> Option<char> {
    input.get(pos..)?.chars().next()
}

/// Classify a sigil segment by its leading character.
fn classify_sigil_segment(sigil: char, rest: &str) -> SpineSegmentKind {
    match sigil {
        '@' => {
            // Check for sub-search pattern: @type:query
            if let Some(colon_pos) = rest.find(':') {
                let context_type = rest[..colon_pos].to_string();
                let sub_query = rest[colon_pos + 1..].to_string();
                SpineSegmentKind::ContextMention {
                    context_type,
                    sub_query: Some(sub_query),
                }
            } else {
                SpineSegmentKind::ContextMention {
                    context_type: rest.to_string(),
                    sub_query: None,
                }
            }
        }
        '/' => {
            // A second slash means a filesystem path (`/tmp/`, `/Users/me/notes`),
            // never a slash command — command names contain no `/`. Fall through to
            // free text so the launcher's directory-browse grammar owns the input.
            if rest.contains('/') {
                SpineSegmentKind::FreeText
            } else {
                SpineSegmentKind::SlashCommand {
                    command: rest.to_string(),
                }
            }
        }
        '-' => SpineSegmentKind::Flow {
            query: rest.to_string(),
        },
        '|' => SpineSegmentKind::Profile {
            profile_id: rest.to_string(),
        },
        '.' => {
            // `./…` and `../…` are relative-path browses, not style ids.
            if rest.starts_with('/') || rest.starts_with("./") {
                SpineSegmentKind::FreeText
            } else {
                SpineSegmentKind::Style {
                    style_id: rest.to_string(),
                }
            }
        }
        ';' => {
            let first_space = rest.find(' ');
            match first_space {
                Some(pos) => SpineSegmentKind::Capture {
                    target: rest[..pos].to_string(),
                    args: rest[pos + 1..].to_string(),
                },
                None => SpineSegmentKind::Capture {
                    target: rest.to_string(),
                    args: String::new(),
                },
            }
        }
        ':' => SpineSegmentKind::ListFilter {
            query: rest.to_string(),
        },
        '>' => {
            let trimmed = rest.strip_prefix(':').unwrap_or(rest);
            if trimmed.is_empty() && !rest.contains(':') {
                SpineSegmentKind::ProjectCwd { sub_query: None }
            } else {
                SpineSegmentKind::ProjectCwd {
                    sub_query: Some(trimmed.to_string()),
                }
            }
        }
        _ => SpineSegmentKind::FreeText,
    }
}

/// Extract the query text from the active segment at the given cursor position.
/// This is what drives list filtering for the projected rows.
fn extract_active_query(seg: &SpineSegment, cursor_byte: usize) -> String {
    match &seg.kind {
        SpineSegmentKind::FreeText => {
            // Return text from segment start to cursor
            let offset = cursor_byte.saturating_sub(seg.byte_range.start);
            let clamped = offset.min(seg.raw.len());
            // Find a valid char boundary
            let boundary = find_char_boundary(&seg.raw, clamped);
            seg.raw[..boundary].trim().to_string()
        }
        SpineSegmentKind::ContextMention {
            sub_query: Some(sq),
            ..
        } => {
            // For @file:readme, the query is the sub-query portion
            sq.clone()
        }
        SpineSegmentKind::ContextMention {
            context_type,
            sub_query: None,
        } => context_type.clone(),
        SpineSegmentKind::SlashCommand { command } => command.clone(),
        SpineSegmentKind::Flow { query } => query.clone(),
        SpineSegmentKind::Profile { profile_id } => profile_id.clone(),
        SpineSegmentKind::Style { style_id } => style_id.clone(),
        SpineSegmentKind::Capture { target, .. } => target.clone(),
        SpineSegmentKind::ListFilter { query } => query.clone(),
        SpineSegmentKind::ProjectCwd {
            sub_query: Some(sq),
        } => sq.clone(),
        SpineSegmentKind::ProjectCwd { sub_query: None } => String::new(),
        SpineSegmentKind::ModeExit { rest, .. } => rest.clone(),
    }
}

/// Find the nearest valid UTF-8 char boundary at or before `pos` in `s`.
fn find_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        let parse = parse_spine("");
        assert!(parse.segments.is_empty());
    }

    #[test]
    fn plain_text_only() {
        let parse = parse_spine("hello world");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(parse.segments[0].kind, SpineSegmentKind::FreeText));
        assert_eq!(parse.segments[0].raw, "hello world");
    }

    /// Chaos battery 05 (2026-07-18): `/tmp/` was classified as
    /// `SlashCommand { command: "tmp/" }`, so the spine's Commands section
    /// hijacked the main list while root directory browse loaded rows the UI
    /// could never render. A second `/` is path syntax — command names never
    /// contain slashes — and must stay free text for the launcher to own.
    #[test]
    fn absolute_path_with_second_slash_is_free_text() {
        for input in ["/tmp/", "/tmp/chaos", "/Users/me/dev/"] {
            let parse = parse_spine(input);
            assert_eq!(parse.segments.len(), 1, "input {input:?}");
            assert!(
                matches!(parse.segments[0].kind, SpineSegmentKind::FreeText),
                "input {input:?} must be free text, got {:?}",
                parse.segments[0].kind
            );
            assert_eq!(parse.segments[0].raw, input);
        }
    }

    #[test]
    fn single_slash_prefix_is_still_a_slash_command() {
        let parse = parse_spine("/rew");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::SlashCommand { command } if command == "rew"
        ));
    }

    /// `./…` and `../…` are relative directory browses, not `.style` ids.
    #[test]
    fn relative_path_prefixes_are_free_text() {
        for input in ["./scripts", "../sibling/dir"] {
            let parse = parse_spine(input);
            assert_eq!(parse.segments.len(), 1, "input {input:?}");
            assert!(
                matches!(parse.segments[0].kind, SpineSegmentKind::FreeText),
                "input {input:?} must be free text, got {:?}",
                parse.segments[0].kind
            );
        }
    }

    #[test]
    fn style_sigil_without_path_shape_is_still_style() {
        let parse = parse_spine(".formal");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Style { style_id } if style_id == "formal"
        ));
    }

    #[test]
    fn single_context_mention() {
        let parse = parse_spine("@selection");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ContextMention { context_type, sub_query: None }
            if context_type == "selection"
        ));
    }

    #[test]
    fn context_mention_with_sub_query() {
        let parse = parse_spine("@file:readme.md");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ContextMention { context_type, sub_query: Some(sq) }
            if context_type == "file" && sq == "readme.md"
        ));
    }

    #[test]
    fn context_mention_sub_search_empty_query() {
        let parse = parse_spine("@file:");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ContextMention { context_type, sub_query: Some(sq) }
            if context_type == "file" && sq.is_empty()
        ));
    }

    #[test]
    fn slash_command() {
        let parse = parse_spine("/rewrite");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::SlashCommand { command }
            if command == "rewrite"
        ));
    }

    #[test]
    fn profile() {
        let parse = parse_spine("|creative");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Profile { profile_id }
            if profile_id == "creative"
        ));
    }

    #[test]
    fn style() {
        let parse = parse_spine(".professional");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Style { style_id }
            if style_id == "professional"
        ));
    }

    #[test]
    fn capture_with_args() {
        let parse = parse_spine(";todo buy milk");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Capture { target, args }
            if target == "todo" && args == "buy milk"
        ));
    }

    #[test]
    fn capture_no_args() {
        let parse = parse_spine(";todo");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Capture { target, args }
            if target == "todo" && args.is_empty()
        ));
    }

    #[test]
    fn postfix_capture_classifies_as_capture() {
        // `todo; buy milk` is the canonical capture spelling (A4) — the
        // spine must agree with the menu-syntax grammar that executes it.
        let parse = parse_spine("todo; buy milk");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Capture { target, args }
            if target == "todo" && args == "buy milk"
        ));
        assert_eq!(parse.segments[0].byte_range, 0..14);
    }

    #[test]
    fn postfix_capture_requires_identifier_head_at_start() {
        // A semicolon later in prose must not turn free text into a capture.
        let parse = parse_spine("hello world; not a capture");
        assert!(!matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Capture { .. }
        ));
        // URLs and paths before the semicolon disqualify the head.
        let parse = parse_spine("https://example.com; nope");
        assert!(!matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Capture { .. }
        ));
    }

    #[test]
    fn list_filter() {
        let parse = parse_spine(":type:script");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ListFilter { query }
            if query == "type:script"
        ));
    }

    #[test]
    fn flow_sigil_at_input_start() {
        let parse = parse_spine("-gmail");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Flow { query } if query == "gmail"
        ));

        let parse = parse_spine("-");
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Flow { query } if query.is_empty()
        ));
    }

    #[test]
    fn flow_sigil_after_another_segment() {
        let parse = parse_spine("@selection -gmail");
        assert_eq!(parse.segments.len(), 2);
        assert!(matches!(
            &parse.segments[1].kind,
            SpineSegmentKind::Flow { query } if query == "gmail"
        ));
    }

    /// `-` must NOT split prose or captures: hyphens/dashes are everyday
    /// text. Only a `-` where the scanner is already at a segment start
    /// (input start / after another segment) opens the flow search.
    #[test]
    fn dash_inside_free_text_stays_free_text() {
        let parse = parse_spine("buy milk - urgent");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(parse.segments[0].kind, SpineSegmentKind::FreeText));
        assert_eq!(parse.segments[0].raw, "buy milk - urgent");
    }

    #[test]
    fn dash_inside_capture_args_stays_in_capture() {
        let parse = parse_spine(";todo dry-cleaning - asap");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Capture { target, args }
            if target == "todo" && args == "dry-cleaning - asap"
        ));
    }

    #[test]
    fn mode_exit_tilde() {
        let parse = parse_spine("~/Documents");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ModeExit { sigil: '~', .. }
        ));
    }

    #[test]
    fn project_cwd_root() {
        let parse = parse_spine(">");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ProjectCwd { sub_query: None }
        ));
    }

    #[test]
    fn project_cwd_with_query() {
        let parse = parse_spine(">:dev");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ProjectCwd { sub_query: Some(sq) }
            if sq == "dev"
        ));
    }

    #[test]
    fn project_cwd_directory_browse() {
        let parse = parse_spine(">:~/dev/");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ProjectCwd { sub_query: Some(sq) }
            if sq == "~/dev/"
        ));
    }

    #[test]
    fn mode_exit_bang() {
        let parse = parse_spine("!ls -la");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ModeExit { sigil: '!', .. }
        ));
    }

    #[test]
    fn mode_exit_question() {
        let parse = parse_spine("?");
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ModeExit { sigil: '?', .. }
        ));
    }

    #[test]
    fn full_prompt_builder() {
        let input = "@file:readme |creative /rewrite make it punchier";
        let parse = parse_spine(input);

        assert_eq!(parse.segments.len(), 4);

        // @file:readme
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ContextMention { context_type, sub_query: Some(sq) }
            if context_type == "file" && sq == "readme"
        ));

        // |creative
        assert!(matches!(
            &parse.segments[1].kind,
            SpineSegmentKind::Profile { profile_id }
            if profile_id == "creative"
        ));

        // /rewrite
        assert!(matches!(
            &parse.segments[2].kind,
            SpineSegmentKind::SlashCommand { command }
            if command == "rewrite"
        ));

        // make it punchier (free text tail)
        assert!(matches!(parse.segments[3].kind, SpineSegmentKind::FreeText));
        assert_eq!(parse.segments[3].raw, "make it punchier");
    }

    #[test]
    fn cursor_in_context_mention() {
        let input = "@file:readme |creative /rewrite make it punchier";
        let parse = parse_spine(input);

        // Cursor inside "readme" (byte 6-11)
        let proj = project_cursor(&parse, 8);
        assert_eq!(proj.active_segment_index, 0);
        assert!(matches!(
            &proj.active_segment_kind,
            SpineSegmentKind::ContextMention { .. }
        ));
        assert_eq!(proj.active_query, "readme");
        assert!(!proj.is_tail);
        assert!(proj.has_prompt_segments);
    }

    #[test]
    fn cursor_in_slash_command() {
        let input = "@file:readme |creative /rewrite make it punchier";
        let parse = parse_spine(input);

        // Cursor inside "/rewrite" — find its byte range
        let rewrite_seg = &parse.segments[2];
        let proj = project_cursor(&parse, rewrite_seg.byte_range.start + 2);
        assert_eq!(proj.active_segment_index, 2);
        assert!(matches!(
            &proj.active_segment_kind,
            SpineSegmentKind::SlashCommand { command }
            if command == "rewrite"
        ));
    }

    #[test]
    fn cursor_in_free_text_tail() {
        let input = "@file:readme |creative /rewrite make it punchier";
        let parse = parse_spine(input);

        // Cursor at end of input
        let proj = project_cursor(&parse, input.len());
        assert_eq!(proj.active_segment_index, parse.segments.len() - 1);
        assert!(matches!(
            proj.active_segment_kind,
            SpineSegmentKind::FreeText
        ));
        assert!(proj.is_tail);
        assert!(proj.has_prompt_segments);
    }

    #[test]
    fn free_text_without_prompt_segments_is_not_tail() {
        let input = "hello world";
        let parse = parse_spine(input);

        let proj = project_cursor(&parse, input.len());
        assert!(!proj.is_tail);
        assert!(!proj.has_prompt_segments);
    }

    #[test]
    fn unknown_segment_gets_preflight_instruction() {
        let input = "@unknownThing";
        let parse = parse_spine(input);
        assert_eq!(parse.segments.len(), 1);

        // Resolution is Unresolved by default — resolution happens at catalog lookup time.
        // The parser only classifies; it doesn't resolve.
        assert!(matches!(
            parse.segments[0].resolution,
            SpineSegmentResolution::Unresolved
        ));
    }

    #[test]
    fn unicode_input_safety() {
        let input = "@file:日本語 /rewrite 文章を修正";
        let parse = parse_spine(input);

        assert_eq!(parse.segments.len(), 3);

        // Verify byte ranges are valid UTF-8 boundaries
        for seg in &parse.segments {
            assert!(input.is_char_boundary(seg.byte_range.start));
            assert!(input.is_char_boundary(seg.byte_range.end));
            assert_eq!(&input[seg.byte_range.clone()], seg.raw);
        }

        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ContextMention { context_type, sub_query: Some(sq) }
            if context_type == "file" && sq == "日本語"
        ));
    }

    #[test]
    fn sigil_in_middle_of_word_is_not_a_segment() {
        let input = "hello@world.com";
        let parse = parse_spine(input);
        // The @ is not at a word boundary, so this should be plain free text
        assert_eq!(parse.segments.len(), 1);
        assert!(matches!(parse.segments[0].kind, SpineSegmentKind::FreeText));
    }

    #[test]
    fn multiple_context_mentions() {
        let input = "@selection @clipboard @file:notes.md";
        let parse = parse_spine(input);

        assert_eq!(parse.segments.len(), 3);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ContextMention { context_type, .. }
            if context_type == "selection"
        ));
        assert!(matches!(
            &parse.segments[1].kind,
            SpineSegmentKind::ContextMention { context_type, .. }
            if context_type == "clipboard"
        ));
        assert!(matches!(
            &parse.segments[2].kind,
            SpineSegmentKind::ContextMention { context_type, sub_query: Some(sq) }
            if context_type == "file" && sq == "notes.md"
        ));
    }

    #[test]
    fn style_sugar_with_tail() {
        let input = ".professional make it shorter";
        let parse = parse_spine(input);

        // `.professional` is one prompt-builder segment, "make it shorter" is free text
        assert_eq!(parse.segments.len(), 2);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Style { style_id }
            if style_id == "professional"
        ));
        assert!(matches!(parse.segments[1].kind, SpineSegmentKind::FreeText));
        assert_eq!(parse.segments[1].raw, "make it shorter");
    }

    #[test]
    fn capture_with_context_attachment() {
        let input = ";todo buy milk @clipboard";
        let parse = parse_spine(input);

        // ";todo buy milk" is a capture segment, then "@clipboard" is a context mention
        assert_eq!(parse.segments.len(), 2);
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::Capture { target, args }
            if target == "todo" && args == "buy milk"
        ));
        assert!(matches!(
            &parse.segments[1].kind,
            SpineSegmentKind::ContextMention { context_type, .. }
            if context_type == "clipboard"
        ));
    }

    #[test]
    fn byte_ranges_are_ordered_and_non_overlapping() {
        let input = "@file:readme |creative /rewrite make it punchier";
        let parse = parse_spine(input);

        // Verify segments are ordered, non-overlapping, and their raw text
        // matches the byte range slice. Inter-segment whitespace is unowned.
        let mut prev_end = 0;
        for seg in &parse.segments {
            assert!(
                seg.byte_range.start >= prev_end,
                "Segment starts before previous end: {} < {}",
                seg.byte_range.start,
                prev_end
            );
            assert_eq!(
                &input[seg.byte_range.clone()],
                seg.raw,
                "Byte range doesn't match raw text"
            );
            prev_end = seg.byte_range.end;
        }
        assert!(prev_end <= input.len(), "Last segment extends past input");
    }

    #[test]
    fn cwd_plus_file_plus_freetext() {
        let input = ">:dev @file:README.md explain setup";
        let parse = parse_spine(input);

        for seg in &parse.segments {
            assert!(
                input.is_char_boundary(seg.byte_range.start),
                "Invalid start boundary"
            );
            assert!(
                input.is_char_boundary(seg.byte_range.end),
                "Invalid end boundary"
            );
            assert_eq!(
                &input[seg.byte_range.clone()],
                seg.raw,
                "Byte range doesn't match raw for {:?}",
                seg.kind
            );
        }

        assert!(
            parse.segments.len() >= 3,
            "Expected 3+ segments, got {}",
            parse.segments.len()
        );
        assert!(matches!(
            &parse.segments[0].kind,
            SpineSegmentKind::ProjectCwd { sub_query: Some(sq) }
            if sq == "dev"
        ));
        assert!(matches!(
            &parse.segments[1].kind,
            SpineSegmentKind::ContextMention { context_type, sub_query: Some(sq) }
            if context_type == "file" && sq == "README.md"
        ));
        assert!(matches!(parse.segments[2].kind, SpineSegmentKind::FreeText));
        assert_eq!(parse.segments[2].raw, "explain setup");
    }
}
