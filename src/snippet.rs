//! VSCode snippet syntax parser for template() SDK function
//!
//! Parses snippet syntax into a structured data model for tabstop navigation.
//!
//! Supported syntax:
//! - `$1`, `$2`, `$3` - Simple tabstops (numbered positions)
//! - `${1:default}` - Tabstops with placeholder text
//! - `${1|a,b,c|}` - Choice tabstops (dropdown options)
//! - `$0` - Final cursor position
//! - `$$` - Escaped literal dollar sign

/// Represents a parsed part of a snippet template
#[derive(Debug, Clone, PartialEq)]
pub enum SnippetPart {
    /// Literal text (no special meaning)
    Text(String),
    /// A tabstop position
    Tabstop {
        /// Tabstop index: 0 = final cursor, 1+ = navigation order
        index: usize,
        /// Default placeholder text (from `${1:text}` syntax)
        placeholder: Option<String>,
        /// Choice options (from `${1|a,b,c|}` syntax)
        choices: Option<Vec<String>>,
        /// Byte range in the expanded text where this tabstop appears
        range: (usize, usize),
    },
}

/// Information about a tabstop, with all occurrences of the same index merged
#[derive(Debug, Clone, PartialEq)]
pub struct TabstopInfo {
    /// Tabstop index
    pub index: usize,
    /// All byte ranges where this tabstop appears (for linked editing)
    pub ranges: Vec<(usize, usize)>,
    /// Placeholder text (if any)
    pub placeholder: Option<String>,
    /// Choice options (if any)
    pub choices: Option<Vec<String>>,
}

/// A fully parsed snippet with expanded text and tabstop metadata
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSnippet {
    /// Sequential parts of the snippet (text and tabstops interleaved)
    pub parts: Vec<SnippetPart>,
    /// Fully expanded text with placeholders filled in
    pub text: String,
    /// Tabstops sorted by navigation order (1, 2, 3... then 0)
    pub tabstops: Vec<TabstopInfo>,
}

impl ParsedSnippet {
    /// Parse a VSCode snippet template string into a structured representation
    ///
    /// # Examples
    ///
    /// ```
    /// use script_kit_gpui::snippet::ParsedSnippet;
    ///
    /// let snippet = ParsedSnippet::parse("Hello $1!");
    /// assert_eq!(snippet.text, "Hello !");
    /// assert_eq!(snippet.tabstops.len(), 1);
    /// ```
    pub fn parse(template: &str) -> Self {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut char_count: usize = 0; // Track char count for char-based indices
        let mut chars = template.chars().peekable();
        let mut current_text = String::new();

        while let Some(c) = chars.next() {
            if c == '$' {
                match chars.peek() {
                    // Escaped dollar: $$ -> $
                    Some('$') => {
                        chars.next();
                        current_text.push('$');
                    }
                    // Tabstop with braces: ${...}
                    Some('{') => {
                        // Flush current text
                        if !current_text.is_empty() {
                            text.push_str(&current_text);
                            char_count += current_text.chars().count();
                            parts.push(SnippetPart::Text(current_text.clone()));
                            current_text.clear();
                        }
                        chars.next(); // consume '{'

                        let tabstop = Self::parse_braced_tabstop(&mut chars, char_count);
                        let placeholder_text = tabstop
                            .placeholder
                            .as_deref()
                            .or(tabstop
                                .choices
                                .as_ref()
                                .and_then(|c| c.first().map(|s| s.as_str())))
                            .unwrap_or("");

                        text.push_str(placeholder_text);
                        char_count += placeholder_text.chars().count();
                        parts.push(SnippetPart::Tabstop {
                            index: tabstop.index,
                            placeholder: tabstop.placeholder,
                            choices: tabstop.choices,
                            range: tabstop.range,
                        });
                    }
                    // Simple tabstop: $N
                    Some(d) if d.is_ascii_digit() => {
                        // Flush current text
                        if !current_text.is_empty() {
                            text.push_str(&current_text);
                            char_count += current_text.chars().count();
                            parts.push(SnippetPart::Text(current_text.clone()));
                            current_text.clear();
                        }

                        let mut num_str = String::new();
                        while let Some(&d) = chars.peek() {
                            if d.is_ascii_digit() {
                                num_str.push(d);
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        let index: usize = num_str.parse().unwrap_or(0);
                        // Simple tabstop has empty placeholder, so range is (char_count, char_count)
                        parts.push(SnippetPart::Tabstop {
                            index,
                            placeholder: None,
                            choices: None,
                            range: (char_count, char_count),
                        });
                    }
                    // Just a lone $ at end or followed by non-special char
                    _ => {
                        current_text.push('$');
                    }
                }
            } else {
                current_text.push(c);
            }
        }

        // Flush remaining text
        if !current_text.is_empty() {
            text.push_str(&current_text);
            parts.push(SnippetPart::Text(current_text));
        }

        // Build tabstop info, merging same indices
        let tabstops = Self::build_tabstop_info(&parts);

        Self {
            parts,
            text,
            tabstops,
        }
    }

    /// Parse a braced tabstop: `{1}`, `{1:default}`, or `{1|a,b,c|}`
    ///
    /// `char_offset` is the current position in char indices (not bytes).
    fn parse_braced_tabstop(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        char_offset: usize,
    ) -> TabstopParseResult {
        let mut index_str = String::new();

        // Parse index number
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                index_str.push(c);
                chars.next();
            } else {
                break;
            }
        }

        let index: usize = index_str.parse().unwrap_or(0);

        // Check what follows the index
        match chars.peek() {
            // Placeholder: ${1:text}
            Some(':') => {
                chars.next(); // consume ':'
                let placeholder = Self::parse_until_close_brace(chars);
                // Use char count, not byte length
                let placeholder_char_len = placeholder.chars().count();
                let range = (char_offset, char_offset + placeholder_char_len);
                TabstopParseResult {
                    index,
                    placeholder: Some(placeholder),
                    choices: None,
                    range,
                }
            }
            // Choices: ${1|a,b,c|}
            Some('|') => {
                chars.next(); // consume '|'
                let choices = Self::parse_choices(chars);
                // Use char count of first choice, not byte length
                let first_choice_char_len = choices.first().map(|s| s.chars().count()).unwrap_or(0);
                let range = (char_offset, char_offset + first_choice_char_len);
                TabstopParseResult {
                    index,
                    placeholder: None,
                    choices: Some(choices),
                    range,
                }
            }
            // Simple: ${1}
            Some('}') => {
                chars.next(); // consume '}'
                TabstopParseResult {
                    index,
                    placeholder: None,
                    choices: None,
                    range: (char_offset, char_offset),
                }
            }
            // Unexpected - consume until }
            _ => {
                Self::parse_until_close_brace(chars);
                TabstopParseResult {
                    index,
                    placeholder: None,
                    choices: None,
                    range: (char_offset, char_offset),
                }
            }
        }
    }

    /// Parse content until closing brace, handling nested braces
    #[allow(clippy::while_let_on_iterator)]
    fn parse_until_close_brace(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut result = String::new();
        let mut brace_depth = 1;

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    brace_depth += 1;
                    result.push(c);
                }
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        break;
                    }
                    result.push(c);
                }
                _ => result.push(c),
            }
        }

        result
    }

    /// Parse choice options: `a,b,c|}`
    fn parse_choices(chars: &mut std::iter::Peekable<std::str::Chars>) -> Vec<String> {
        let mut choices = Vec::new();
        let mut current = String::new();

        #[allow(clippy::while_let_on_iterator)]
        while let Some(c) = chars.next() {
            match c {
                ',' => {
                    choices.push(current.clone());
                    current.clear();
                }
                '|' => {
                    // End of choices, expect }
                    choices.push(current);
                    // Consume the closing }
                    if chars.peek() == Some(&'}') {
                        chars.next();
                    }
                    break;
                }
                '\\' => {
                    // Escaped character in choice
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                _ => current.push(c),
            }
        }

        choices
    }

    /// Build TabstopInfo from parts, merging same indices
    fn build_tabstop_info(parts: &[SnippetPart]) -> Vec<TabstopInfo> {
        use std::collections::BTreeMap;

        let mut tabstop_map: BTreeMap<usize, TabstopInfo> = BTreeMap::new();

        for part in parts {
            if let SnippetPart::Tabstop {
                index,
                placeholder,
                choices,
                range,
            } = part
            {
                tabstop_map
                    .entry(*index)
                    .and_modify(|info| {
                        info.ranges.push(*range);
                        // Keep first placeholder/choices found
                        if info.placeholder.is_none() && placeholder.is_some() {
                            info.placeholder = placeholder.clone();
                        }
                        if info.choices.is_none() && choices.is_some() {
                            info.choices = choices.clone();
                        }
                    })
                    .or_insert_with(|| TabstopInfo {
                        index: *index,
                        ranges: vec![*range],
                        placeholder: placeholder.clone(),
                        choices: choices.clone(),
                    });
            }
        }

        // Sort: all non-zero indices in order, then 0 (final cursor) at end
        let mut result: Vec<TabstopInfo> = tabstop_map
            .into_iter()
            .filter(|(idx, _)| *idx != 0)
            .map(|(_, info)| info)
            .collect();

        // Add $0 at the end if it exists
        if let Some(final_cursor) = parts.iter().find_map(|p| {
            if let SnippetPart::Tabstop {
                index: 0,
                placeholder,
                choices,
                range,
            } = p
            {
                Some(TabstopInfo {
                    index: 0,
                    ranges: vec![*range],
                    placeholder: placeholder.clone(),
                    choices: choices.clone(),
                })
            } else {
                None
            }
        }) {
            result.push(final_cursor);
        }

        result
    }

    /// Get tabstop info by index
    #[allow(dead_code)]
    pub fn get_tabstop(&self, index: usize) -> Option<&TabstopInfo> {
        self.tabstops.iter().find(|t| t.index == index)
    }

    /// Get the navigation order of tabstops (1, 2, 3... then 0)
    #[allow(dead_code)]
    pub fn tabstop_order(&self) -> Vec<usize> {
        self.tabstops.iter().map(|t| t.index).collect()
    }

    /// Update tabstop ranges after an edit operation.
    ///
    /// This method adjusts all tabstop ranges to account for text changes in the document.
    /// Ranges are stored as char indices (not byte offsets) to match editor cursor positions.
    ///
    /// # Arguments
    /// * `current_tabstop_idx` - Index into self.tabstops of the tabstop currently being edited.
    ///   Ranges within this tabstop that contain the edit point will be resized.
    ///   Pass `usize::MAX` if editing outside any tabstop.
    /// * `edit_start` - Char index where the edit begins
    /// * `old_len` - Number of chars that were removed
    /// * `new_len` - Number of chars that were inserted
    ///
    /// # Behavior
    /// - Ranges **after** the edit point are shifted by `delta = new_len - old_len`
    /// - Ranges **containing** the edit point (within current tabstop) are resized by `delta`
    /// - Ranges **before** the edit point are unchanged
    pub fn update_tabstops_after_edit(
        &mut self,
        current_tabstop_idx: usize,
        edit_start: usize,
        old_len: usize,
        new_len: usize,
    ) {
        let delta = new_len as isize - old_len as isize;
        if delta == 0 {
            return;
        }

        let edit_end = edit_start + old_len;

        for (tabstop_idx, tabstop) in self.tabstops.iter_mut().enumerate() {
            for range in tabstop.ranges.iter_mut() {
                let (range_start, range_end) = *range;

                // Case 1: Range is entirely before the edit - no change
                if range_end <= edit_start {
                    continue;
                }

                // Case 2: Range is entirely after the edit - shift by delta
                if range_start > edit_end
                    || (range_start == edit_end && tabstop_idx != current_tabstop_idx)
                {
                    *range = (
                        (range_start as isize + delta) as usize,
                        (range_end as isize + delta) as usize,
                    );
                    continue;
                }

                // Case 3: Edit is within or at the boundary of this range
                // For the current tabstop, we resize (keep start, adjust end)
                // For other tabstops, the edit should not overlap (they're not being edited)
                if tabstop_idx == current_tabstop_idx {
                    // Edit is within this range - keep start, resize end
                    *range = (range_start, (range_end as isize + delta) as usize);
                } else {
                    // This range starts at or after the edit point but before edit_end
                    // This means it overlaps with the edit region
                    // Shift the entire range by delta
                    *range = (
                        (range_start as isize + delta).max(0) as usize,
                        (range_end as isize + delta).max(0) as usize,
                    );
                }
            }
        }
    }
}

/// Internal helper for parsing braced tabstops
struct TabstopParseResult {
    index: usize,
    placeholder: Option<String>,
    choices: Option<Vec<String>>,
    range: (usize, usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_tabstop() {
        let snippet = ParsedSnippet::parse("$1");
        assert_eq!(snippet.parts.len(), 1);
        match &snippet.parts[0] {
            SnippetPart::Tabstop { index, .. } => assert_eq!(*index, 1),
            _ => panic!("Expected Tabstop"),
        }
        assert_eq!(snippet.text, "");
    }

    #[test]
    fn test_parse_tabstop_with_placeholder() {
        let snippet = ParsedSnippet::parse("${1:name}");
        assert_eq!(snippet.parts.len(), 1);
        match &snippet.parts[0] {
            SnippetPart::Tabstop {
                index, placeholder, ..
            } => {
                assert_eq!(*index, 1);
                assert_eq!(placeholder.as_deref(), Some("name"));
            }
            _ => panic!("Expected Tabstop"),
        }
        assert_eq!(snippet.text, "name");
    }

    #[test]
    fn test_parse_tabstop_with_choices() {
        let snippet = ParsedSnippet::parse("${1|a,b,c|}");
        assert_eq!(snippet.parts.len(), 1);
        match &snippet.parts[0] {
            SnippetPart::Tabstop { index, choices, .. } => {
                assert_eq!(*index, 1);
                assert_eq!(
                    choices.as_ref().unwrap(),
                    &vec!["a".to_string(), "b".to_string(), "c".to_string()]
                );
            }
            _ => panic!("Expected Tabstop"),
        }
        // First choice is used as expanded text
        assert_eq!(snippet.text, "a");
    }

    #[test]
    fn test_parse_text_and_tabstop() {
        let snippet = ParsedSnippet::parse("Hello $1!");
        assert_eq!(snippet.parts.len(), 3);

        match &snippet.parts[0] {
            SnippetPart::Text(t) => assert_eq!(t, "Hello "),
            _ => panic!("Expected Text"),
        }
        match &snippet.parts[1] {
            SnippetPart::Tabstop { index, .. } => assert_eq!(*index, 1),
            _ => panic!("Expected Tabstop"),
        }
        match &snippet.parts[2] {
            SnippetPart::Text(t) => assert_eq!(t, "!"),
            _ => panic!("Expected Text"),
        }

        assert_eq!(snippet.text, "Hello !");
    }

    #[test]
    fn test_parse_escaped_dollar() {
        let snippet = ParsedSnippet::parse("$$100");
        assert_eq!(snippet.parts.len(), 1);
        match &snippet.parts[0] {
            SnippetPart::Text(t) => assert_eq!(t, "$100"),
            _ => panic!("Expected Text"),
        }
        assert_eq!(snippet.text, "$100");
    }

    #[test]
    fn test_parse_linked_tabstops() {
        let snippet = ParsedSnippet::parse("${1:foo} and ${1:bar}");

        // Should have 3 parts: tabstop, text, tabstop
        assert_eq!(snippet.parts.len(), 3);

        // Both tabstops should have index 1
        let tabstop1 = &snippet.parts[0];
        let tabstop2 = &snippet.parts[2];

        match (tabstop1, tabstop2) {
            (
                SnippetPart::Tabstop {
                    index: idx1,
                    placeholder: p1,
                    ..
                },
                SnippetPart::Tabstop {
                    index: idx2,
                    placeholder: p2,
                    ..
                },
            ) => {
                assert_eq!(*idx1, 1);
                assert_eq!(*idx2, 1);
                assert_eq!(p1.as_deref(), Some("foo"));
                assert_eq!(p2.as_deref(), Some("bar"));
            }
            _ => panic!("Expected two Tabstops"),
        }

        // Should only have one TabstopInfo with two ranges
        assert_eq!(snippet.tabstops.len(), 1);
        assert_eq!(snippet.tabstops[0].index, 1);
        assert_eq!(snippet.tabstops[0].ranges.len(), 2);
        // First placeholder should be kept
        assert_eq!(snippet.tabstops[0].placeholder.as_deref(), Some("foo"));
    }

    #[test]
    fn test_parse_final_cursor() {
        let snippet = ParsedSnippet::parse("$0");
        assert_eq!(snippet.parts.len(), 1);
        match &snippet.parts[0] {
            SnippetPart::Tabstop { index, .. } => assert_eq!(*index, 0),
            _ => panic!("Expected Tabstop"),
        }
    }

    #[test]
    fn test_parse_empty_string() {
        let snippet = ParsedSnippet::parse("");
        assert_eq!(snippet.parts.len(), 0);
        assert_eq!(snippet.text, "");
        assert_eq!(snippet.tabstops.len(), 0);
    }

    #[test]
    fn test_tabstop_order() {
        let snippet = ParsedSnippet::parse("$3 $1 $2 $0");
        let order = snippet.tabstop_order();
        // Should be sorted: 1, 2, 3, then 0 at end
        assert_eq!(order, vec![1, 2, 3, 0]);
    }

    #[test]
    fn test_get_tabstop() {
        let snippet = ParsedSnippet::parse("${1:hello} ${2:world}");

        let t1 = snippet.get_tabstop(1).unwrap();
        assert_eq!(t1.index, 1);
        assert_eq!(t1.placeholder.as_deref(), Some("hello"));

        let t2 = snippet.get_tabstop(2).unwrap();
        assert_eq!(t2.index, 2);
        assert_eq!(t2.placeholder.as_deref(), Some("world"));

        assert!(snippet.get_tabstop(3).is_none());
    }

    #[test]
    fn test_tabstop_ranges() {
        let snippet = ParsedSnippet::parse("Hello ${1:world}!");

        // "Hello " is 6 chars, "world" is 5 chars
        // Range should be (6, 11)
        let t1 = snippet.get_tabstop(1).unwrap();
        assert_eq!(t1.ranges, vec![(6, 11)]);
    }

    #[test]
    fn test_multiple_tabstops_with_text() {
        let snippet = ParsedSnippet::parse("function ${1:name}(${2:args}) { $0 }");

        assert_eq!(snippet.text, "function name(args) {  }");

        let order = snippet.tabstop_order();
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn test_simple_braced_tabstop() {
        let snippet = ParsedSnippet::parse("${1}");
        assert_eq!(snippet.parts.len(), 1);
        match &snippet.parts[0] {
            SnippetPart::Tabstop {
                index, placeholder, ..
            } => {
                assert_eq!(*index, 1);
                assert!(placeholder.is_none());
            }
            _ => panic!("Expected Tabstop"),
        }
    }

    #[test]
    fn test_lone_dollar_preserved() {
        let snippet = ParsedSnippet::parse("$x");
        // $ followed by non-digit/non-brace should be preserved
        assert_eq!(snippet.parts.len(), 1);
        match &snippet.parts[0] {
            SnippetPart::Text(t) => assert_eq!(t, "$x"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_dollar_at_end() {
        let snippet = ParsedSnippet::parse("test$");
        assert_eq!(snippet.parts.len(), 1);
        match &snippet.parts[0] {
            SnippetPart::Text(t) => assert_eq!(t, "test$"),
            _ => panic!("Expected Text"),
        }
    }

    // --- Tests for update_tabstops_after_edit ---

    #[test]
    fn test_update_tabstops_after_insert_first_tabstop() {
        // Template: "${1:hello} ${2:world}"
        // Initial text: "hello world" (char indices)
        // Tabstop 1 at (0, 5), Tabstop 2 at (6, 11)
        //
        // If we type "XX" at position 0 (replacing "hello" with "XXhello"):
        // - Tabstop 1 should expand from (0, 5) to (0, 7)
        // - Tabstop 2 should shift from (6, 11) to (8, 13)
        let mut snippet = ParsedSnippet::parse("${1:hello} ${2:world}");

        // Verify initial state
        assert_eq!(snippet.tabstops.len(), 2);
        assert_eq!(snippet.tabstops[0].ranges, vec![(0, 5)]);
        assert_eq!(snippet.tabstops[1].ranges, vec![(6, 11)]);

        // Simulate inserting "XX" at position 0, which replaces nothing (old_len=0)
        // edit_start=0, old_len=0, new_len=2
        snippet.update_tabstops_after_edit(0, 0, 0, 2);

        // Tabstop 1 was being edited (contains edit point), should expand
        // Original: (0, 5), +2 chars inserted at start -> still (0, 5+2) = (0, 7)
        // But the current tabstop (0) is the one being edited, so its end expands
        assert_eq!(snippet.tabstops[0].ranges, vec![(0, 7)]);
        // Tabstop 2 should shift right by 2
        assert_eq!(snippet.tabstops[1].ranges, vec![(8, 13)]);
    }

    #[test]
    fn test_update_tabstops_after_delete_in_first_tabstop() {
        // Template: "${1:hello} ${2:world}"
        // Initial: Tabstop 1 at (0, 5), Tabstop 2 at (6, 11)
        //
        // If we delete "hel" (positions 0-3), leaving "lo":
        // - Tabstop 1 shrinks from (0, 5) to (0, 2)
        // - Tabstop 2 shifts from (6, 11) to (3, 8)
        let mut snippet = ParsedSnippet::parse("${1:hello} ${2:world}");

        // Delete 3 chars at position 0 (old_len=3, new_len=0)
        snippet.update_tabstops_after_edit(0, 0, 3, 0);

        // Tabstop 1 shrinks by 3 chars
        assert_eq!(snippet.tabstops[0].ranges, vec![(0, 2)]);
        // Tabstop 2 shifts left by 3 chars
        assert_eq!(snippet.tabstops[1].ranges, vec![(3, 8)]);
    }

    #[test]
    fn test_update_tabstops_after_replace_in_first_tabstop() {
        // Template: "${1:hello} ${2:world}"
        // Initial: Tabstop 1 at (0, 5), Tabstop 2 at (6, 11)
        //
        // If we replace "hello" (0-5) with "hi" (delta = 2 - 5 = -3):
        // - Tabstop 1 shrinks from (0, 5) to (0, 2)
        // - Tabstop 2 shifts from (6, 11) to (3, 8)
        let mut snippet = ParsedSnippet::parse("${1:hello} ${2:world}");

        // Replace 5 chars with 2 chars at position 0
        snippet.update_tabstops_after_edit(0, 0, 5, 2);

        assert_eq!(snippet.tabstops[0].ranges, vec![(0, 2)]);
        assert_eq!(snippet.tabstops[1].ranges, vec![(3, 8)]);
    }

    #[test]
    fn test_update_tabstops_no_change_before_edit() {
        // Edits before a tabstop should shift it
        // Template: "prefix ${1:hello}"
        // Initial: Tabstop 1 at (7, 12)
        //
        // If we add "XX" at position 2 (in "prefix"):
        // - Tabstop 1 shifts from (7, 12) to (9, 14)
        let mut snippet = ParsedSnippet::parse("prefix ${1:hello}");

        assert_eq!(snippet.tabstops[0].ranges, vec![(7, 12)]);

        // Insert 2 chars at position 2 (inside "prefix")
        // current_tabstop_idx is irrelevant here since edit is in text, not tabstop
        // But we need to pass it - use a value that won't affect the tabstop
        snippet.update_tabstops_after_edit(usize::MAX, 2, 0, 2);

        // Tabstop 1 shifts right by 2
        assert_eq!(snippet.tabstops[0].ranges, vec![(9, 14)]);
    }

    #[test]
    fn test_update_tabstops_linked_tabstops() {
        // Template: "${1:foo} and ${1:bar}"
        // This creates a single TabstopInfo with multiple ranges
        // Initial ranges: [(0, 3), (8, 11)]
        //
        // If we edit the first occurrence, both should update appropriately
        let mut snippet = ParsedSnippet::parse("${1:foo} and ${1:bar}");

        assert_eq!(snippet.tabstops.len(), 1);
        assert_eq!(snippet.tabstops[0].ranges, vec![(0, 3), (8, 11)]);

        // Insert 2 chars at position 0 (start of first range)
        // Current tabstop is 0 (the only one)
        snippet.update_tabstops_after_edit(0, 0, 0, 2);

        // First range expands, second range shifts
        assert_eq!(snippet.tabstops[0].ranges, vec![(0, 5), (10, 13)]);
    }

    #[test]
    fn test_choices_with_commas() {
        let snippet = ParsedSnippet::parse("${1|apple,banana,cherry|}");
        match &snippet.parts[0] {
            SnippetPart::Tabstop { choices, .. } => {
                let c = choices.as_ref().unwrap();
                assert_eq!(c.len(), 3);
                assert_eq!(c[0], "apple");
                assert_eq!(c[1], "banana");
                assert_eq!(c[2], "cherry");
            }
            _ => panic!("Expected Tabstop"),
        }
    }

    #[test]
    fn test_complex_template() {
        let template = r#"import { ${1:Component} } from '${2:react}';

export default function ${1:Component}() {
    return (
        <div>$0</div>
    );
}"#;

        let snippet = ParsedSnippet::parse(template);

        // Should have Component tabstop (index 1) twice
        let t1 = snippet.get_tabstop(1).unwrap();
        assert_eq!(t1.ranges.len(), 2);
        assert_eq!(t1.placeholder.as_deref(), Some("Component"));

        // Should have react tabstop (index 2) once
        let t2 = snippet.get_tabstop(2).unwrap();
        assert_eq!(t2.ranges.len(), 1);
        assert_eq!(t2.placeholder.as_deref(), Some("react"));

        // Order should be 1, 2, 0
        assert_eq!(snippet.tabstop_order(), vec![1, 2, 0]);
    }
}
