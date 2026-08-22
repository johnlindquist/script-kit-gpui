//! Shared search thresholds, deterministic scores, paths, and safe highlight spans.

use std::ops::Range;
use std::path::Path;

/// Root-provider thresholds count bytes so one CJK character or emoji remains searchable.
pub fn query_meets_min_query_chars(trimmed_query: &str, min_query_chars: usize) -> bool {
    trimmed_query.len() >= min_query_chars
}

/// Produces a stable tier-major search score without overflow or cross-tier bonuses.
pub fn score_from_tier(tier: i32, bonus: i32) -> i32 {
    tier.saturating_mul(1000)
        .saturating_add(bonus.clamp(0, 999))
}

/// Recovers the canonical ranking tier while treating invalid scores as unranked.
pub fn match_tier_from_score(score: i32) -> i32 {
    if score <= 0 { 0 } else { score / 1000 }
}

/// Converts a valid UTF-8 byte boundary and bounded span into character indices.
pub fn char_indices_for_span(
    haystack: &str,
    byte_start: usize,
    char_len: usize,
) -> Option<Vec<usize>> {
    let before = haystack.get(..byte_start)?;
    let remaining = haystack.get(byte_start..)?;
    let start_char = before.chars().count();
    let end_char = start_char.checked_add(char_len)?;
    if remaining.chars().take(char_len).count() != char_len {
        return None;
    }
    Some((start_char..end_char).collect())
}

/// Converts ordered in-bounds character indices without allocating an offset table.
pub fn byte_range_for_char_indices(haystack: &str, indices: &[usize]) -> Option<Range<usize>> {
    if indices.windows(2).any(|pair| pair[0] >= pair[1]) {
        return None;
    }
    let first = *indices.first()?;
    let last = *indices.last()?;
    let mut remaining = last.checked_sub(first)?;
    let start = haystack.char_indices().nth(first)?.0;

    for (offset, ch) in haystack[start..].char_indices() {
        if remaining == 0 {
            return Some(start..start + offset + ch.len_utf8());
        }
        remaining -= 1;
    }
    None
}

/// Extracts the display filename without inventing a value for root or invalid paths.
pub fn extract_filename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

/// Preserves a scriptlet's complete anchor while removing its private parent path.
pub fn extract_scriptlet_display_path(file_path: &Option<String>) -> Option<String> {
    file_path.as_ref().map(|path| {
        let (path_part, anchor) = match path.split_once('#') {
            Some((path, anchor)) => (path, Some(anchor)),
            None => (path.as_str(), None),
        };
        let filename = Path::new(path_part)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path_part);
        match anchor {
            Some(anchor) => format!("{filename}#{anchor}"),
            None => filename.to_string(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ascii_queries_gate_on_length() {
        assert!(!query_meets_min_query_chars("ab", 3));
        assert!(query_meets_min_query_chars("abc", 3));
    }

    #[test]
    fn single_cjk_char_and_emoji_meet_a_min_of_three() {
        assert!(query_meets_min_query_chars("日", 3));
        assert!(query_meets_min_query_chars("🚀", 3));
        assert!(query_meets_min_query_chars("日本", 4));
    }

    #[test]
    fn empty_queries_only_pass_an_explicit_zero_threshold() {
        assert!(query_meets_min_query_chars("", 0));
        assert!(!query_meets_min_query_chars("", 1));
    }

    #[test]
    fn ranking_bonus_stays_inside_its_owner_tier() {
        assert_eq!(score_from_tier(700, -100), 700_000);
        assert_eq!(score_from_tier(700, 250), 700_250);
        assert_eq!(score_from_tier(700, 10_000), 700_999);
    }

    #[test]
    fn ranking_scores_saturate_without_overflow() {
        assert_eq!(score_from_tier(i32::MAX, 999), i32::MAX);
        assert_eq!(score_from_tier(i32::MIN, 0), i32::MIN);
    }

    #[test]
    fn ranking_tiers_round_trip_and_reject_invalid_scores() {
        assert_eq!(match_tier_from_score(score_from_tier(950, 999)), 950);
        assert_eq!(match_tier_from_score(0), 0);
        assert_eq!(match_tier_from_score(-1234), 0);
    }

    #[test]
    fn ascii_character_spans_keep_original_positions() {
        assert_eq!(
            char_indices_for_span("launcher", 2, 4),
            Some(vec![2, 3, 4, 5])
        );
    }

    #[test]
    fn unicode_character_spans_use_character_not_byte_positions() {
        assert_eq!(char_indices_for_span("é🚀notes", 2, 2), Some(vec![1, 2]));
    }

    #[test]
    fn non_boundary_byte_offsets_fail_without_panicking() {
        assert_eq!(char_indices_for_span("é🚀notes", 1, 1), None);
        assert_eq!(char_indices_for_span("é🚀notes", 3, 1), None);
    }

    #[test]
    fn impossible_character_spans_fail_without_inventing_highlights() {
        assert_eq!(char_indices_for_span("notes", 9, 1), None);
        assert_eq!(char_indices_for_span("notes", 4, 2), None);
        assert_eq!(char_indices_for_span("notes", 1, usize::MAX), None);
    }

    #[test]
    fn empty_character_spans_remain_valid_at_real_boundaries() {
        assert_eq!(char_indices_for_span("é", 0, 0), Some(Vec::new()));
        assert_eq!(char_indices_for_span("é", 2, 0), Some(Vec::new()));
    }

    #[test]
    fn ascii_highlight_ranges_preserve_existing_span_semantics() {
        assert_eq!(
            byte_range_for_char_indices("launcher", &[1, 2, 3]),
            Some(1..4)
        );
        assert_eq!(byte_range_for_char_indices("launcher", &[1, 3]), Some(1..4));
    }

    #[test]
    fn unicode_highlight_ranges_follow_exact_utf8_boundaries() {
        assert_eq!(byte_range_for_char_indices("é🚀notes", &[0]), Some(0..2));
        assert_eq!(byte_range_for_char_indices("é🚀notes", &[1]), Some(2..6));
        assert_eq!(byte_range_for_char_indices("é🚀notes", &[1, 2]), Some(2..7));
    }

    #[test]
    fn highlight_ranges_reject_empty_out_of_order_and_out_of_bounds_indices() {
        assert_eq!(byte_range_for_char_indices("notes", &[]), None);
        assert_eq!(byte_range_for_char_indices("notes", &[4, 2]), None);
        assert_eq!(byte_range_for_char_indices("notes", &[1, 1]), None);
        assert_eq!(byte_range_for_char_indices("notes", &[1, 4, 2]), None);
        assert_eq!(byte_range_for_char_indices("notes", &[5]), None);
        assert_eq!(byte_range_for_char_indices("notes", &[2, 8]), None);
    }

    #[test]
    fn hostile_maximum_highlight_offsets_cannot_overflow_or_panic() {
        assert_eq!(byte_range_for_char_indices("notes", &[usize::MAX]), None);
        assert_eq!(byte_range_for_char_indices("notes", &[0, usize::MAX]), None);
        assert_eq!(
            byte_range_for_char_indices("notes", &[0, usize::MAX, 2]),
            None
        );
    }

    #[test]
    fn every_real_unicode_character_span_round_trips_without_invalid_offsets() {
        for text in ["", "notes", "é", "🚀", "a\u{0301}", "日本🚀café"] {
            let character_count = text.chars().count();
            for first in 0..character_count {
                for last in first..character_count {
                    let indices = (first..=last).collect::<Vec<_>>();
                    let range = byte_range_for_char_indices(text, &indices)
                        .expect("valid character positions must produce a byte span");
                    assert!(text.is_char_boundary(range.start));
                    assert!(text.is_char_boundary(range.end));
                    assert_eq!(text[range].chars().count(), last - first + 1);
                }
            }
            for invalid in [character_count, character_count + 1, usize::MAX] {
                assert_eq!(byte_range_for_char_indices(text, &[invalid]), None);
            }
        }
    }

    #[test]
    fn test_extract_filename() {
        assert_eq!(
            extract_filename(&PathBuf::from("/path/to/script.ts")),
            "script.ts"
        );
        assert_eq!(
            extract_filename(&PathBuf::from("relative/path.js")),
            "path.js"
        );
        assert_eq!(extract_filename(&PathBuf::from("single.ts")), "single.ts");
    }

    #[test]
    fn filenames_preserve_unicode_without_leaking_parent_directories() {
        assert_eq!(
            extract_filename(Path::new("/private/équipe/🚀.ts")),
            "🚀.ts"
        );
        assert_eq!(extract_filename(Path::new("/")), "");
    }

    #[test]
    fn test_extract_scriptlet_display_path() {
        assert_eq!(
            extract_scriptlet_display_path(&Some("/path/to/file.md#slug".to_string())),
            Some("file.md#slug".to_string())
        );
        assert_eq!(
            extract_scriptlet_display_path(&Some("/path/to/file.md".to_string())),
            Some("file.md".to_string())
        );
        assert_eq!(extract_scriptlet_display_path(&None), None);
    }

    #[test]
    fn scriptlet_paths_preserve_complete_empty_and_nested_anchors() {
        assert_eq!(
            extract_scriptlet_display_path(&Some("/private/file.md#one#two".to_string())),
            Some("file.md#one#two".to_string())
        );
        assert_eq!(
            extract_scriptlet_display_path(&Some("/private/file.md#".to_string())),
            Some("file.md#".to_string())
        );
    }

    #[test]
    fn scriptlet_paths_preserve_unicode_and_root_fallbacks() {
        assert_eq!(
            extract_scriptlet_display_path(&Some("/équipe/🚀.md#déployer".to_string())),
            Some("🚀.md#déployer".to_string())
        );
        assert_eq!(
            extract_scriptlet_display_path(&Some("/#root".to_string())),
            Some("/#root".to_string())
        );
    }
}
