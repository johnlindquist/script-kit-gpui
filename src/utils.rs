//! Shared utility functions for Script Kit GPUI

/// Strip HTML tags from a string, returning plain text.
///
/// This function removes all HTML tags (content between < and >) and normalizes
/// whitespace. Consecutive whitespace is collapsed to a single space, and the
/// result is trimmed.
///
/// # Examples
///
/// ```
/// use script_kit_gpui::utils::strip_html_tags;
///
/// assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
/// assert_eq!(strip_html_tags("<div><span>A</span><span>B</span></div>"), "A B");
/// ```
pub fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut pending_space = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                pending_space = true; // Add space between tags
            }
            _ if !in_tag => {
                if ch.is_whitespace() {
                    if !result.is_empty() && !result.ends_with(' ') {
                        pending_space = true;
                    }
                } else {
                    if pending_space && !result.is_empty() {
                        result.push(' ');
                    }
                    pending_space = false;
                    result.push(ch);
                }
            }
            _ => {} // Skip characters inside tags
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tag_removal() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
    }

    #[test]
    fn test_nested_tags() {
        assert_eq!(
            strip_html_tags("<div><span>A</span><span>B</span></div>"),
            "A B"
        );
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn test_no_tags() {
        assert_eq!(strip_html_tags("plain text"), "plain text");
    }

    #[test]
    fn test_whitespace_normalization() {
        assert_eq!(strip_html_tags("<p>Hello   World</p>"), "Hello World");
    }

    #[test]
    fn test_multiple_tags_with_content() {
        assert_eq!(
            strip_html_tags("<h1>Title</h1><p>Paragraph</p>"),
            "Title Paragraph"
        );
    }

    #[test]
    fn test_self_closing_tags() {
        assert_eq!(strip_html_tags("Hello<br/>World"), "Hello World");
    }

    #[test]
    fn test_tags_with_attributes() {
        assert_eq!(
            strip_html_tags("<a href=\"https://example.com\">Link</a>"),
            "Link"
        );
    }

    #[test]
    fn test_deeply_nested() {
        assert_eq!(
            strip_html_tags("<div><div><div>Deep</div></div></div>"),
            "Deep"
        );
    }

    #[test]
    fn test_only_tags() {
        assert_eq!(strip_html_tags("<div><span></span></div>"), "");
    }

    #[test]
    fn test_leading_trailing_whitespace() {
        assert_eq!(strip_html_tags("  <p>  Hello  </p>  "), "Hello");
    }

    #[test]
    fn test_newlines_in_html() {
        assert_eq!(
            strip_html_tags("<p>\n  Hello\n  World\n</p>"),
            "Hello World"
        );
    }
}
