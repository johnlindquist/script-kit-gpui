use super::super::types::{Script, Scriptlet};
use super::contains_ignore_ascii_case;
pub(crate) use sk_protocol::query_prefix::{
    app_passes_prefix_filter, builtin_passes_prefix_filter, flow_passes_prefix_filter,
    parse_query_prefix, should_search_scriptlets, should_search_scripts,
    skill_passes_prefix_filter, window_passes_prefix_filter, ParsedQuery,
};

/// Check if a script passes a prefix filter.
/// Returns true if no filter is active or if the script matches the filter.
pub(crate) fn script_passes_prefix_filter(script: &Script, parsed: &ParsedQuery) -> bool {
    let (kind, value) = match (&parsed.filter_kind, &parsed.filter_value) {
        (Some(k), Some(v)) => (k.as_str(), v.as_str()),
        _ => return true, // No filter active
    };

    match kind {
        "tag" => {
            if let Some(ref meta) = script.typed_metadata {
                meta.tags
                    .iter()
                    .any(|t| contains_ignore_ascii_case(t, value))
            } else {
                false
            }
        }
        "author" => script
            .typed_metadata
            .as_ref()
            .and_then(|m| m.author.as_deref())
            .is_some_and(|a| contains_ignore_ascii_case(a, value)),
        "kit" => script
            .kit_name
            .as_deref()
            .is_some_and(|k| contains_ignore_ascii_case(k, value)),
        "is" => {
            if let Some(ref meta) = script.typed_metadata {
                match value {
                    "cron" => meta.cron.is_some(),
                    "scheduled" | "schedule" => meta.cron.is_some() || meta.schedule.is_some(),
                    "bg" | "background" => meta.background,
                    "watch" | "watching" => !meta.watch.is_empty(),
                    "system" | "sys" => meta.system,
                    _ => false,
                }
            } else {
                false
            }
        }
        "type" => matches!(value, "script" | "scripts"),
        // group: and tool: don't apply to scripts
        "group" | "tool" => false,
        _ => true,
    }
}

/// Check if a scriptlet passes a prefix filter.
pub(crate) fn scriptlet_passes_prefix_filter(scriptlet: &Scriptlet, parsed: &ParsedQuery) -> bool {
    let (kind, value) = match (&parsed.filter_kind, &parsed.filter_value) {
        (Some(k), Some(v)) => (k.as_str(), v.as_str()),
        _ => return true,
    };

    match kind {
        "group" => scriptlet
            .group
            .as_deref()
            .is_some_and(|g| contains_ignore_ascii_case(g, value)),
        "tool" => {
            contains_ignore_ascii_case(&scriptlet.tool, value)
                || contains_ignore_ascii_case(scriptlet.tool_display_name(), value)
        }
        "type" => matches!(value, "snippet" | "snippets" | "scriptlet" | "scriptlets"),
        // tag:, author:, kit:, is: don't apply to scriptlets (they don't have these fields)
        "tag" | "author" | "kit" | "is" => false,
        _ => true,
    }
}
