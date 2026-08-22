//! Allocation-conscious ASCII matching shared by launcher and passive search.
//!
//! These byte-level helpers intentionally require ASCII input or an already
//! lowercase pattern. Unicode-aware callers must keep their nucleo fallback.

/// Whether byte-level case folding is safe for both search inputs.
#[inline]
pub fn is_ascii_pair(a: &str, b: &str) -> bool {
    a.is_ascii() && b.is_ascii()
}

/// Finds an already-lowercase ASCII needle without allocating a haystack copy.
#[inline]
pub fn contains_ignore_ascii_case(haystack: &str, needle_lower: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();
    if n.is_empty() {
        return true;
    }
    if n.len() > h.len() {
        return false;
    }
    'outer: for i in 0..=(h.len() - n.len()) {
        for j in 0..n.len() {
            if h[i + j].to_ascii_lowercase() != n[j] {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

/// Returns the byte position of an already-lowercase ASCII needle.
#[inline]
pub fn find_ignore_ascii_case(haystack: &str, needle_lower: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();
    if n.is_empty() {
        return Some(0);
    }
    if n.len() > h.len() {
        return None;
    }
    'outer: for i in 0..=(h.len() - n.len()) {
        for j in 0..n.len() {
            if h[i + j].to_ascii_lowercase() != n[j] {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}

/// True at the start, after punctuation, or at an ASCII camel-case boundary.
#[inline]
pub fn is_word_boundary_match(haystack: &str, match_pos: usize) -> bool {
    if match_pos == 0 {
        return true;
    }
    let bytes = haystack.as_bytes();
    if match_pos >= bytes.len() {
        return false;
    }
    let prev = bytes[match_pos - 1];
    let curr = bytes[match_pos];
    if !prev.is_ascii_alphanumeric() {
        return true;
    }
    prev.is_ascii_lowercase() && curr.is_ascii_uppercase()
}

/// Checks a complete ASCII name against its already-lowercase query.
#[inline]
pub fn is_exact_name_match(haystack: &str, query_lower: &str) -> bool {
    haystack.len() == query_lower.len()
        && haystack
            .as_bytes()
            .iter()
            .zip(query_lower.as_bytes())
            .all(|(h, q)| h.to_ascii_lowercase() == *q)
}

/// Single-character queries use substring matching rather than broad fuzzy matching.
pub const MIN_FUZZY_QUERY_LEN: usize = 2;

/// Returns the ordered character indices for an already-lowercase ASCII pattern.
#[inline]
pub fn fuzzy_match_with_indices_ascii(haystack: &str, pattern_lower: &str) -> (bool, Vec<usize>) {
    let mut indices = Vec::new();
    let mut pattern_chars = pattern_lower.chars().peekable();

    for (idx, ch) in haystack.chars().enumerate() {
        if let Some(&p) = pattern_chars.peek() {
            if ch.to_ascii_lowercase() == p {
                indices.push(idx);
                pattern_chars.next();
            }
        }
    }

    let matched = pattern_chars.peek().is_none();
    (matched, if matched { indices } else { Vec::new() })
}

/// Whether pattern characters appear in order, ignoring ASCII case.
pub fn is_fuzzy_match(haystack: &str, pattern: &str) -> bool {
    let mut pattern_chars = pattern.chars().peekable();
    for ch in haystack.chars() {
        if let Some(&p) = pattern_chars.peek() {
            if ch.eq_ignore_ascii_case(&p) {
                pattern_chars.next();
            }
        }
    }
    pattern_chars.peek().is_none()
}

/// Returns ordered character indices for an ASCII-case-insensitive pattern.
pub fn fuzzy_match_with_indices(haystack: &str, pattern: &str) -> (bool, Vec<usize>) {
    let mut indices = Vec::new();
    let mut pattern_chars = pattern.chars().peekable();

    for (idx, ch) in haystack.chars().enumerate() {
        if let Some(&p) = pattern_chars.peek() {
            if ch.eq_ignore_ascii_case(&p) {
                indices.push(idx);
                pattern_chars.next();
            }
        }
    }

    let matched = pattern_chars.peek().is_none();
    (matched, if matched { indices } else { Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_ignore_ascii_case_basic() {
        assert!(contains_ignore_ascii_case("OpenFile", "open"));
        assert!(contains_ignore_ascii_case("OPENFILE", "open"));
        assert!(contains_ignore_ascii_case("openfile", "open"));
        assert!(contains_ignore_ascii_case("MyOpenFile", "open"));
    }

    #[test]
    fn test_contains_ignore_ascii_case_not_found() {
        assert!(!contains_ignore_ascii_case("OpenFile", "save"));
        assert!(!contains_ignore_ascii_case("test", "testing"));
    }

    #[test]
    fn test_contains_ignore_ascii_case_empty_needle() {
        assert!(contains_ignore_ascii_case("OpenFile", ""));
        assert!(contains_ignore_ascii_case("", ""));
    }

    #[test]
    fn test_contains_ignore_ascii_case_needle_longer() {
        assert!(!contains_ignore_ascii_case("ab", "abc"));
    }

    #[test]
    fn test_find_ignore_ascii_case_at_start() {
        assert_eq!(find_ignore_ascii_case("OpenFile", "open"), Some(0));
        assert_eq!(find_ignore_ascii_case("OPENFILE", "open"), Some(0));
    }

    #[test]
    fn test_find_ignore_ascii_case_in_middle() {
        assert_eq!(find_ignore_ascii_case("MyOpenFile", "open"), Some(2));
    }

    #[test]
    fn test_find_ignore_ascii_case_not_found() {
        assert_eq!(find_ignore_ascii_case("OpenFile", "save"), None);
    }

    #[test]
    fn test_find_ignore_ascii_case_empty_needle() {
        assert_eq!(find_ignore_ascii_case("OpenFile", ""), Some(0));
    }

    #[test]
    fn test_fuzzy_match_with_indices_ascii_basic() {
        let (matched, indices) = fuzzy_match_with_indices_ascii("OpenFile", "of");
        assert!(matched);
        assert_eq!(indices, vec![0, 4]);
    }

    #[test]
    fn test_fuzzy_match_with_indices_ascii_case_insensitive() {
        let (matched, indices) = fuzzy_match_with_indices_ascii("OpenFile", "of");
        assert!(matched);
        assert_eq!(indices, vec![0, 4]);
    }

    #[test]
    fn test_fuzzy_match_with_indices_ascii_no_match() {
        let (matched, indices) = fuzzy_match_with_indices_ascii("test", "xyz");
        assert!(!matched);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_fuzzy_match_with_indices_ascii_empty_pattern() {
        let (matched, indices) = fuzzy_match_with_indices_ascii("test", "");
        assert!(matched);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_is_word_boundary_match_start() {
        assert!(is_word_boundary_match("Hello World", 0));
    }

    #[test]
    fn test_is_word_boundary_match_after_space() {
        assert!(is_word_boundary_match("Hello World", 6));
    }

    #[test]
    fn test_is_word_boundary_match_after_dash() {
        assert!(is_word_boundary_match("git-commit", 4));
    }

    #[test]
    fn test_is_word_boundary_match_camel_case() {
        assert!(is_word_boundary_match("gitCommit", 3));
    }

    #[test]
    fn test_is_word_boundary_match_mid_word() {
        assert!(!is_word_boundary_match("Hello", 1));
    }

    #[test]
    fn test_is_exact_name_match() {
        assert!(is_exact_name_match("Hello", "hello"));
        assert!(is_exact_name_match("Agent Chat", "agent chat"));
        assert!(!is_exact_name_match("Hello World", "hello"));
        assert!(!is_exact_name_match("Hi", "hello"));
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        assert!(is_fuzzy_match("OPENFILE", "open"));
        assert!(is_fuzzy_match("Open File", "of"));
        assert!(is_fuzzy_match("OpenFile", "OP"));
    }

    #[test]
    fn test_fuzzy_match_single_char() {
        assert!(is_fuzzy_match("test", "t"));
        assert!(is_fuzzy_match("test", "e"));
        assert!(is_fuzzy_match("test", "s"));
    }

    #[test]
    fn test_fuzzy_match_not_in_order() {
        assert!(is_fuzzy_match("test", "st"));
        assert!(!is_fuzzy_match("abc", "cab"));
        assert!(!is_fuzzy_match("open", "nope"));
    }

    #[test]
    fn test_fuzzy_match_exact_match() {
        assert!(is_fuzzy_match("test", "test"));
        assert!(is_fuzzy_match("open", "open"));
    }

    #[test]
    fn test_fuzzy_match_empty_pattern() {
        assert!(is_fuzzy_match("test", ""));
        assert!(is_fuzzy_match("", ""));
    }

    #[test]
    fn test_fuzzy_match_pattern_longer_than_haystack() {
        assert!(!is_fuzzy_match("ab", "abc"));
        assert!(!is_fuzzy_match("x", "xyz"));
    }

    #[test]
    fn test_fuzzy_match_with_indices_basic() {
        let (matched, indices) = fuzzy_match_with_indices("openfile", "opf");
        assert!(matched);
        assert_eq!(indices, vec![0, 1, 4]);
    }

    #[test]
    fn test_fuzzy_match_with_indices_no_match() {
        let (matched, indices) = fuzzy_match_with_indices("test", "xyz");
        assert!(!matched);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_fuzzy_match_with_indices_case_insensitive() {
        let (matched, indices) = fuzzy_match_with_indices("OpenFile", "of");
        assert!(matched);
        assert_eq!(indices, vec![0, 4]);
    }

    #[test]
    fn ascii_fast_path_rejects_unicode_on_either_side() {
        assert!(is_ascii_pair("Open File", "of"));
        assert!(!is_ascii_pair("Café", "cafe"));
        assert!(!is_ascii_pair("Cafe", "café"));
    }

    #[test]
    fn word_boundary_rejects_positions_beyond_the_original_text() {
        assert!(!is_word_boundary_match("Open", 4));
        assert!(!is_word_boundary_match("Open", usize::MAX));
    }

    #[test]
    fn fuzzy_highlights_keep_original_ascii_character_positions() {
        let (matched, indices) = fuzzy_match_with_indices_ascii("Open File", "of");
        assert!(matched);
        assert_eq!(indices, vec![0, 5]);
    }

    #[test]
    fn single_character_queries_do_not_enable_broad_fuzzy_matching() {
        assert_eq!(MIN_FUZZY_QUERY_LEN, 2);
    }
}
