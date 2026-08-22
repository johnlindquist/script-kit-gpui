//! Structured launcher queries and app-independent category routing.
//!
//! Script and scriptlet metadata matching remains in the application because
//! those records deliberately do not belong to this domain crate.

/// A query with an optional recognized, structured filter prefix.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedQuery {
    /// Recognized filter kind, such as `tag`, `author`, `is`, or `type`.
    pub filter_kind: Option<String>,
    /// Lowercased filter value between the prefix and the first ASCII space.
    pub filter_value: Option<String>,
    /// Remaining search text after the optional structured filter.
    pub remainder: String,
}

/// Parses the launcher's established `kind:value remaining query` grammar.
pub fn parse_query_prefix(query: &str) -> ParsedQuery {
    let trimmed = query.trim();
    let prefixes = ["tag:", "author:", "kit:", "is:", "type:", "group:", "tool:"];

    for prefix in &prefixes {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let kind = prefix.trim_end_matches(':').to_string();
            let (value, remainder) = match rest.find(' ') {
                Some(pos) => (rest[..pos].to_string(), rest[pos + 1..].trim().to_string()),
                None => (rest.to_string(), String::new()),
            };
            if value.is_empty() {
                return ParsedQuery {
                    filter_kind: None,
                    filter_value: None,
                    remainder: trimmed.to_string(),
                };
            }
            return ParsedQuery {
                filter_kind: Some(kind),
                filter_value: Some(value.to_lowercase()),
                remainder,
            };
        }
    }

    ParsedQuery {
        filter_kind: None,
        filter_value: None,
        remainder: trimmed.to_string(),
    }
}

/// Whether built-in commands belong in this structured query.
pub fn builtin_passes_prefix_filter(parsed: &ParsedQuery) -> bool {
    let (kind, value) = match (&parsed.filter_kind, &parsed.filter_value) {
        (Some(k), Some(v)) => (k.as_str(), v.as_str()),
        _ => return true,
    };
    match kind {
        "type" => matches!(value, "command" | "commands" | "builtin" | "builtins"),
        _ => false,
    }
}

/// Whether installed applications belong in this structured query.
pub fn app_passes_prefix_filter(parsed: &ParsedQuery) -> bool {
    let (kind, value) = match (&parsed.filter_kind, &parsed.filter_value) {
        (Some(k), Some(v)) => (k.as_str(), v.as_str()),
        _ => return true,
    };
    match kind {
        "type" => matches!(value, "app" | "apps"),
        _ => false,
    }
}

/// Whether open windows belong in this structured query.
pub fn window_passes_prefix_filter(parsed: &ParsedQuery) -> bool {
    let (kind, value) = match (&parsed.filter_kind, &parsed.filter_value) {
        (Some(k), Some(v)) => (k.as_str(), v.as_str()),
        _ => return true,
    };
    match kind {
        "type" => matches!(value, "window" | "windows"),
        _ => false,
    }
}

/// Whether flows belong in this structured query.
pub fn flow_passes_prefix_filter(parsed: &ParsedQuery) -> bool {
    let (kind, value) = match (&parsed.filter_kind, &parsed.filter_value) {
        (Some(k), Some(v)) => (k.as_str(), v.as_str()),
        _ => return true,
    };
    match kind {
        "type" => matches!(value, "flow" | "flows" | "command" | "commands"),
        _ => false,
    }
}

/// Whether plugin skills belong in this structured query.
pub fn skill_passes_prefix_filter(parsed: &ParsedQuery) -> bool {
    let (kind, value) = match (&parsed.filter_kind, &parsed.filter_value) {
        (Some(k), Some(v)) => (k.as_str(), v.as_str()),
        _ => return true,
    };
    match kind {
        "type" => matches!(value, "skill" | "skills" | "command" | "commands"),
        _ => false,
    }
}

/// Whether application-owned scripts could possibly match this query.
pub fn should_search_scripts(parsed: &ParsedQuery) -> bool {
    match (
        parsed.filter_kind.as_deref(),
        parsed.filter_value.as_deref(),
    ) {
        (None, _) => true,
        (Some("type"), Some(v)) => matches!(v, "script" | "scripts"),
        (Some("tag" | "author" | "kit" | "is"), _) => true,
        (Some("group" | "tool"), _) => false,
        _ => true,
    }
}

/// Whether application-owned scriptlets could possibly match this query.
pub fn should_search_scriptlets(parsed: &ParsedQuery) -> bool {
    match (
        parsed.filter_kind.as_deref(),
        parsed.filter_value.as_deref(),
    ) {
        (None, _) => true,
        (Some("type"), Some(v)) => matches!(v, "snippet" | "snippets" | "scriptlet" | "scriptlets"),
        (Some("group" | "tool"), _) => true,
        (Some("tag" | "author" | "kit" | "is"), _) => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_prefix_tag() {
        let parsed = parse_query_prefix("tag:productivity");
        assert_eq!(parsed.filter_kind.as_deref(), Some("tag"));
        assert_eq!(parsed.filter_value.as_deref(), Some("productivity"));
        assert_eq!(parsed.remainder, "");
    }

    #[test]
    fn test_parse_query_prefix_tag_with_remainder() {
        let parsed = parse_query_prefix("tag:productivity notes");
        assert_eq!(parsed.filter_kind.as_deref(), Some("tag"));
        assert_eq!(parsed.filter_value.as_deref(), Some("productivity"));
        assert_eq!(parsed.remainder, "notes");
    }

    #[test]
    fn test_parse_query_prefix_no_prefix() {
        let parsed = parse_query_prefix("hello world");
        assert_eq!(parsed.filter_kind, None);
        assert_eq!(parsed.filter_value, None);
        assert_eq!(parsed.remainder, "hello world");
    }

    #[test]
    fn test_parse_query_prefix_empty_value() {
        let parsed = parse_query_prefix("tag:");
        assert_eq!(parsed.filter_kind, None);
    }

    #[test]
    fn test_parse_query_prefix_is_cron() {
        let parsed = parse_query_prefix("is:cron");
        assert_eq!(parsed.filter_kind.as_deref(), Some("is"));
        assert_eq!(parsed.filter_value.as_deref(), Some("cron"));
    }

    #[test]
    fn test_parse_query_prefix_type_script() {
        let parsed = parse_query_prefix("type:script");
        assert_eq!(parsed.filter_kind.as_deref(), Some("type"));
        assert_eq!(parsed.filter_value.as_deref(), Some("script"));
    }

    #[test]
    fn test_parse_query_prefix_author() {
        let parsed = parse_query_prefix("author:john search term");
        assert_eq!(parsed.filter_kind.as_deref(), Some("author"));
        assert_eq!(parsed.filter_value.as_deref(), Some("john"));
        assert_eq!(parsed.remainder, "search term");
    }

    #[test]
    fn test_builtin_prefix_filter_allows_command_type_and_rejects_non_builtin_types() {
        assert!(builtin_passes_prefix_filter(&parse_query_prefix(
            "type:command"
        )));
        assert!(builtin_passes_prefix_filter(&parse_query_prefix(
            "type:builtins"
        )));
        assert!(!builtin_passes_prefix_filter(&parse_query_prefix(
            "type:script"
        )));
    }

    #[test]
    fn every_recognized_prefix_lowercases_its_value() {
        for kind in ["tag", "author", "kit", "is", "type", "group", "tool"] {
            let parsed = parse_query_prefix(&format!("{kind}:VaLuE remaining words"));
            assert_eq!(parsed.filter_kind.as_deref(), Some(kind));
            assert_eq!(parsed.filter_value.as_deref(), Some("value"));
            assert_eq!(parsed.remainder, "remaining words");
        }
    }

    #[test]
    fn unknown_and_uppercase_prefixes_remain_regular_queries() {
        for query in ["owner:john", "TAG:work", "Type:script"] {
            let parsed = parse_query_prefix(query);
            assert_eq!(parsed.filter_kind, None);
            assert_eq!(parsed.filter_value, None);
            assert_eq!(parsed.remainder, query);
        }
    }

    #[test]
    fn trims_outer_query_and_remaining_search_text() {
        let parsed = parse_query_prefix("  tag:WORK   release checklist  ");
        assert_eq!(parsed.filter_kind.as_deref(), Some("tag"));
        assert_eq!(parsed.filter_value.as_deref(), Some("work"));
        assert_eq!(parsed.remainder, "release checklist");
    }

    #[test]
    fn empty_value_followed_by_search_text_remains_unstructured() {
        let parsed = parse_query_prefix("  tag: release notes  ");
        assert_eq!(parsed.filter_kind, None);
        assert_eq!(parsed.filter_value, None);
        assert_eq!(parsed.remainder, "tag: release notes");
    }

    #[test]
    fn filter_values_preserve_colons_and_lowercase_unicode() {
        let parsed = parse_query_prefix("tag:ÉQUIPE:OPS notes");
        assert_eq!(parsed.filter_value.as_deref(), Some("équipe:ops"));
        assert_eq!(parsed.remainder, "notes");
    }

    #[test]
    fn unstructured_queries_include_every_category() {
        let parsed = parse_query_prefix("regular launcher search");
        assert!(builtin_passes_prefix_filter(&parsed));
        assert!(app_passes_prefix_filter(&parsed));
        assert!(window_passes_prefix_filter(&parsed));
        assert!(flow_passes_prefix_filter(&parsed));
        assert!(skill_passes_prefix_filter(&parsed));
        assert!(should_search_scripts(&parsed));
        assert!(should_search_scriptlets(&parsed));
    }

    #[test]
    fn applications_accept_only_their_singular_and_plural_type() {
        for accepted in ["type:app", "type:apps"] {
            assert!(app_passes_prefix_filter(&parse_query_prefix(accepted)));
        }
        for rejected in ["type:script", "type:window", "tag:work"] {
            assert!(!app_passes_prefix_filter(&parse_query_prefix(rejected)));
        }
    }

    #[test]
    fn windows_accept_only_their_singular_and_plural_type() {
        for accepted in ["type:window", "type:windows"] {
            assert!(window_passes_prefix_filter(&parse_query_prefix(accepted)));
        }
        for rejected in ["type:app", "type:command", "author:john"] {
            assert!(!window_passes_prefix_filter(&parse_query_prefix(rejected)));
        }
    }

    #[test]
    fn flows_preserve_explicit_flow_and_shared_command_types() {
        for accepted in ["type:flow", "type:flows", "type:command", "type:commands"] {
            assert!(flow_passes_prefix_filter(&parse_query_prefix(accepted)));
        }
        for rejected in ["type:skill", "type:script", "group:work"] {
            assert!(!flow_passes_prefix_filter(&parse_query_prefix(rejected)));
        }
    }

    #[test]
    fn skills_preserve_explicit_skill_and_shared_command_types() {
        for accepted in ["type:skill", "type:skills", "type:command", "type:commands"] {
            assert!(skill_passes_prefix_filter(&parse_query_prefix(accepted)));
        }
        for rejected in ["type:flow", "type:script", "tool:bash"] {
            assert!(!skill_passes_prefix_filter(&parse_query_prefix(rejected)));
        }
    }

    #[test]
    fn script_traversal_only_runs_for_possible_script_categories() {
        for accepted in [
            "type:script",
            "type:scripts",
            "tag:work",
            "author:john",
            "kit:main",
            "is:cron",
        ] {
            assert!(should_search_scripts(&parse_query_prefix(accepted)));
        }
        for rejected in ["type:snippet", "type:app", "group:work", "tool:bash"] {
            assert!(!should_search_scripts(&parse_query_prefix(rejected)));
        }
    }

    #[test]
    fn scriptlet_traversal_only_runs_for_possible_scriptlet_categories() {
        for accepted in [
            "type:snippet",
            "type:snippets",
            "type:scriptlet",
            "type:scriptlets",
            "group:work",
            "tool:bash",
        ] {
            assert!(should_search_scriptlets(&parse_query_prefix(accepted)));
        }
        for rejected in [
            "type:script",
            "type:app",
            "tag:work",
            "author:john",
            "kit:main",
            "is:cron",
        ] {
            assert!(!should_search_scriptlets(&parse_query_prefix(rejected)));
        }
    }

    #[test]
    fn incomplete_manual_filters_remain_nonrestrictive_for_categories() {
        for parsed in [
            ParsedQuery {
                filter_kind: Some("type".to_string()),
                filter_value: None,
                remainder: String::new(),
            },
            ParsedQuery {
                filter_kind: None,
                filter_value: Some("script".to_string()),
                remainder: String::new(),
            },
        ] {
            assert!(builtin_passes_prefix_filter(&parsed));
            assert!(app_passes_prefix_filter(&parsed));
            assert!(window_passes_prefix_filter(&parsed));
            assert!(flow_passes_prefix_filter(&parsed));
            assert!(skill_passes_prefix_filter(&parsed));
            assert!(should_search_scripts(&parsed));
            assert!(should_search_scriptlets(&parsed));
        }
    }
}
