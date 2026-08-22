use gpui::SharedString;

use super::list::{ss, SpineListAction, SpineListRow, SpineListRowKind, SpineListSection};

pub(crate) const SUBSEARCH_RENDER_LIMIT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextSubsearchSource {
    File,
    /// Files scoped to the working directory (the global cwd chip). Unlike
    /// `File` (global Spotlight), this searches `onlyin` the cwd with a
    /// filesystem-walk fallback so Spotlight-blind dot-directory cwds
    /// (`~/.scriptkit`) still return results.
    Project,
    BrowserHistory,
    Clipboard,
    Dictation,
    Scripts,
    Scriptlets,
    Skills,
    Notes,
    History,
    Calendar,
    Notifications,
}

impl ContextSubsearchSource {
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Project => "project",
            Self::BrowserHistory => "browser-history",
            Self::Clipboard => "clipboard",
            Self::Dictation => "dictation",
            Self::Scripts => "scripts",
            Self::Scriptlets => "scriptlets",
            Self::Skills => "skills",
            Self::Notes => "notes",
            Self::History => "history",
            Self::Calendar => "calendar",
            Self::Notifications => "notifications",
        }
    }

    fn section_title(self) -> &'static str {
        match self {
            Self::File => "Files",
            Self::Project => "Project Files",
            Self::BrowserHistory => "Browser History",
            Self::Clipboard => "Clipboard",
            Self::Dictation => "Dictation",
            Self::Scripts => "Scripts",
            Self::Scriptlets => "Scriptlets",
            Self::Skills => "Skills",
            Self::Notes => "Notes",
            Self::History => "Conversations",
            Self::Calendar => "Calendar Events",
            Self::Notifications => "Notifications",
        }
    }

    /// Lowercase noun for the empty colon-mode ghost hint in the filter
    /// input ("search clipboard…", "search files…").
    pub(crate) fn search_hint_noun(self) -> &'static str {
        match self {
            Self::File => "files",
            Self::Project => "project files",
            Self::BrowserHistory => "browser history",
            Self::Clipboard => "clipboard",
            Self::Dictation => "dictation",
            Self::Scripts => "scripts",
            Self::Scriptlets => "scriptlets",
            Self::Skills => "skills",
            Self::Notes => "notes",
            Self::History => "conversations",
            Self::Calendar => "calendar events",
            Self::Notifications => "notifications",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Project => "folder",
            Self::BrowserHistory => "globe",
            Self::Clipboard => "clipboard",
            Self::Dictation => "mic",
            Self::Scripts => "file-code",
            Self::Scriptlets => "scroll-text",
            Self::Skills => "workflow",
            Self::Notes => "notebook-text",
            Self::History => "message-circle",
            Self::Calendar => "calendar",
            Self::Notifications => "bell",
        }
    }
}

/// Spelling variants accepted for each subsearch source, longest first so
/// prefix matching picks the most specific trigger (`files` before `file`).
/// Every entry is also a valid colon-mode spelling (`@files:` ≡ `@file:`).
const SUBSEARCH_TRIGGERS: &[(&str, ContextSubsearchSource)] = &[
    ("browser-history", ContextSubsearchSource::BrowserHistory),
    ("notifications", ContextSubsearchSource::Notifications),
    ("scriptlets", ContextSubsearchSource::Scriptlets),
    ("clipboard", ContextSubsearchSource::Clipboard),
    ("dictation", ContextSubsearchSource::Dictation),
    ("projects", ContextSubsearchSource::Project),
    ("calendar", ContextSubsearchSource::Calendar),
    ("project", ContextSubsearchSource::Project),
    ("scripts", ContextSubsearchSource::Scripts),
    ("history", ContextSubsearchSource::History),
    ("skills", ContextSubsearchSource::Skills),
    ("files", ContextSubsearchSource::File),
    ("notes", ContextSubsearchSource::Notes),
    ("file", ContextSubsearchSource::File),
];

impl ContextSubsearchSource {
    /// Exact trigger match, including aliases (`files` → File). Used for
    /// colon-mode prefixes and exact root fragments.
    pub(crate) fn from_trigger(token: &str) -> Option<Self> {
        SUBSEARCH_TRIGGERS
            .iter()
            .find(|(trigger, _)| token.eq_ignore_ascii_case(trigger))
            .map(|(_, source)| *source)
    }
}

pub(crate) fn parse_context_subsearch<'a>(
    context_type: &'a str,
    sub_query: Option<&'a str>,
) -> Option<(ContextSubsearchSource, &'a str)> {
    if let Some(sq) = sub_query {
        let source = ContextSubsearchSource::from_trigger(context_type)?;
        return Some((source, sq));
    }
    parse_root_subsearch_fragment(context_type)
}

/// Recognize the exact active browser-history mention using the same parser
/// and source resolver as rich rows. Root refresh ownership must use this
/// library-tested decision before a cold snapshot exists; partial mentions or
/// the separate `@history` conversation source never qualify.
pub(crate) fn active_browser_history_subsearch(query_text: &str) -> bool {
    if !query_text.contains('@') {
        return false;
    }

    let parse = super::parse_spine(query_text);
    let projection = super::project_cursor(&parse, query_text.len());
    let super::SpineSegmentKind::ContextMention {
        context_type,
        sub_query,
    } = &projection.active_segment_kind
    else {
        return false;
    };

    matches!(
        parse_context_subsearch(context_type, sub_query.as_deref()),
        Some((ContextSubsearchSource::BrowserHistory, _))
    )
}

/// Rank launcher-owned command catalogs with the exact same visibility,
/// metadata matching, and deterministic ordering as ordinary root search.
///
/// Filtering and truncation happen only after canonical scoring, so a hidden
/// script never leaks into an attachment portal and a late exact match cannot
/// be pushed out by earlier low-relevance rows.
pub(crate) fn rank_context_catalog_results(
    source: ContextSubsearchSource,
    query: &str,
    scripts: &[std::sync::Arc<crate::scripts::Script>],
    scriptlets: &[std::sync::Arc<crate::scripts::Scriptlet>],
    skills: &[std::sync::Arc<crate::plugins::PluginSkill>],
) -> Vec<crate::scripts::SearchResult> {
    let (scripts, scriptlets, skills) = match source {
        ContextSubsearchSource::Scripts => (scripts, &[][..], &[][..]),
        ContextSubsearchSource::Scriptlets => (&[][..], scriptlets, &[][..]),
        ContextSubsearchSource::Skills => (&[][..], &[][..], skills),
        _ => return Vec::new(),
    };

    crate::scripts::fuzzy_search_unified_all_with_skills(
        scripts,
        scriptlets,
        &[],
        &[],
        skills,
        query,
    )
    .into_iter()
    .filter(|result| {
        matches!(
            (source, result),
            (
                ContextSubsearchSource::Scripts,
                crate::scripts::SearchResult::Script(_)
            ) | (
                ContextSubsearchSource::Scriptlets,
                crate::scripts::SearchResult::Scriptlet(_)
            ) | (
                ContextSubsearchSource::Skills,
                crate::scripts::SearchResult::Skill(_)
            )
        )
    })
    .take(SUBSEARCH_RENDER_LIMIT)
    .collect()
}

/// Colon-less `@` fragments switch into search mode the moment the trigger is
/// recognized — typing `@files` must already BE file search (recents,
/// unarmed), with no "press Enter to refine" picker step. Continuations keep
/// searching seamlessly: `@filesreadme` searches files for "readme" exactly
/// like `@file:readme`. Partial fragments (`@fi`) return None so the context
/// catalog can keep offering completion rows.
fn parse_root_subsearch_fragment(context_type: &str) -> Option<(ContextSubsearchSource, &str)> {
    if context_type.is_empty() {
        return None;
    }
    if let Some(source) = ContextSubsearchSource::from_trigger(context_type) {
        return Some((source, ""));
    }
    SUBSEARCH_TRIGGERS
        .iter()
        .find(|(trigger, _)| {
            context_type.len() > trigger.len()
                && context_type[..trigger.len()].eq_ignore_ascii_case(trigger)
        })
        .map(|(trigger, source)| (*source, &context_type[trigger.len()..]))
}

pub(crate) fn build_context_subsearch_section(
    source: ContextSubsearchSource,
    query: &str,
) -> SpineListSection {
    let rows = match source {
        ContextSubsearchSource::File => vec![hint_row(
            if query.trim().is_empty() {
                "Recent files"
            } else {
                "Searching files\u{2026}"
            },
            "File results are loaded by the launcher",
            ContextSubsearchSource::File,
        )],
        ContextSubsearchSource::Project => vec![hint_row(
            if query.trim().is_empty() {
                "Recent project files"
            } else {
                "Searching project files\u{2026}"
            },
            "Project file results are loaded by the launcher",
            ContextSubsearchSource::Project,
        )],
        ContextSubsearchSource::BrowserHistory
        | ContextSubsearchSource::Clipboard
        | ContextSubsearchSource::Dictation
        | ContextSubsearchSource::Notes
        | ContextSubsearchSource::History => vec![hint_row(
            "Loading\u{2026}",
            "Results are loaded by the launcher",
            source,
        )],
        ContextSubsearchSource::Scripts
        | ContextSubsearchSource::Scriptlets
        | ContextSubsearchSource::Skills => vec![hint_row(
            "Loading\u{2026}",
            "Results are loaded by the launcher",
            source,
        )],
        ContextSubsearchSource::Calendar | ContextSubsearchSource::Notifications => vec![hint_row(
            "Loading\u{2026}",
            "Results are loaded by the launcher",
            source,
        )],
    };

    SpineListSection {
        id: ss(format!("spine-section-subsearch:{}", source.prefix())),
        title: ss(source.section_title()),
        subtitle: Some(ss(format!("@{}:", source.prefix()))),
        icon: Some(ss(source.icon())),
        rows,
    }
}

// --- Helpers ---

fn hint_row(title: &str, subtitle: &str, source: ContextSubsearchSource) -> SpineListRow {
    SpineListRow {
        id: ss(format!("spine:@:subsearch-hint:{}", source.prefix())),
        kind: SpineListRowKind::Hint,
        title: SharedString::from(title.to_string()),
        subtitle: Some(SharedString::from(subtitle.to_string())),
        icon: Some(ss(source.icon())),
        meta: None,
        badges: vec![],
        score: 0,
        is_selectable: false,
        action_label: None,
        action: SpineListAction::Noop,
    }
}

pub(crate) fn escape_ref_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '\n' | '\r' | '\t' => out.push_str("%20"),
            '%' => out.push_str("%25"),
            '#' => out.push_str("%23"),
            '@' => out.push_str("%40"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn catalog_script(name: &str) -> Arc<crate::scripts::Script> {
        Arc::new(crate::scripts::Script {
            name: name.to_owned(),
            path: std::path::PathBuf::from(format!("/synthetic/{name}.ts")),
            plugin_id: "main".to_owned(),
            ..Default::default()
        })
    }

    fn catalog_scriptlet(name: &str, alias: Option<&str>) -> Arc<crate::scripts::Scriptlet> {
        Arc::new(crate::scripts::Scriptlet {
            name: name.to_owned(),
            description: Some(format!("Synthetic {name}")),
            code: "// synthetic".to_owned(),
            tool: "ts".to_owned(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: "main".to_owned(),
            plugin_title: None,
            file_path: Some(format!("/synthetic/scriptlets.md#{name}")),
            command: Some(name.to_owned()),
            alias: alias.map(str::to_owned),
            icon: None,
        })
    }

    fn catalog_skill(title: &str, skill_id: &str) -> Arc<crate::plugins::PluginSkill> {
        Arc::new(crate::plugins::PluginSkill {
            plugin_id: "synthetic".to_owned(),
            plugin_title: "Synthetic".to_owned(),
            skill_id: skill_id.to_owned(),
            path: std::path::PathBuf::from(format!("/synthetic/skills/{skill_id}/SKILL.md")),
            title: title.to_owned(),
            description: "Synthetic skill".to_owned(),
        })
    }

    #[test]
    fn context_catalog_never_exposes_hidden_scripts() {
        let visible = catalog_script("Visible deployment");
        let hidden = Arc::new(crate::scripts::Script {
            name: "Hidden deployment".to_owned(),
            path: std::path::PathBuf::from("/synthetic/hidden-deployment.ts"),
            typed_metadata: Some(crate::metadata_parser::TypedMetadata {
                hidden: true,
                ..Default::default()
            }),
            ..Default::default()
        });

        for query in ["", "deployment", "hidden"] {
            let results = rank_context_catalog_results(
                ContextSubsearchSource::Scripts,
                query,
                &[Arc::clone(&hidden), Arc::clone(&visible)],
                &[],
                &[],
            );
            assert!(
                results.iter().all(|result| result.name() != hidden.name),
                "hidden script leaked for query {query:?}"
            );
        }
    }

    #[test]
    fn context_catalog_ranks_before_truncating_and_preserves_evidence() {
        let mut scripts: Vec<_> = (0..SUBSEARCH_RENDER_LIMIT + 4)
            .map(|index| catalog_script(&format!("needle helper {index:02}")))
            .collect();
        scripts.push(catalog_script("needle"));

        let results = rank_context_catalog_results(
            ContextSubsearchSource::Scripts,
            "needle",
            &scripts,
            &[],
            &[],
        );

        assert_eq!(results.len(), SUBSEARCH_RENDER_LIMIT);
        assert_eq!(results[0].name(), "needle");
        match &results[0] {
            crate::scripts::SearchResult::Script(script_match) => {
                assert!(script_match.score > 0);
                assert!(script_match.match_evidence.is_some());
            }
            _ => panic!("script catalog returned a different command family"),
        }
    }

    #[test]
    fn context_catalog_searches_aliases_and_skill_identifiers() {
        let scriptlet = catalog_scriptlet("Open dashboard", Some("launch-control"));
        let skill = catalog_skill("Incident response", "production-triage");

        let scriptlet_results = rank_context_catalog_results(
            ContextSubsearchSource::Scriptlets,
            "launch-control",
            &[],
            std::slice::from_ref(&scriptlet),
            std::slice::from_ref(&skill),
        );
        assert_eq!(scriptlet_results.len(), 1);
        match &scriptlet_results[0] {
            crate::scripts::SearchResult::Scriptlet(result) => {
                assert_eq!(result.scriptlet.name, "Open dashboard");
                assert!(result.match_evidence.is_some());
            }
            _ => panic!("scriptlet alias returned a different command family"),
        }

        let skill_results = rank_context_catalog_results(
            ContextSubsearchSource::Skills,
            "production-triage",
            &[],
            std::slice::from_ref(&scriptlet),
            std::slice::from_ref(&skill),
        );
        assert_eq!(skill_results.len(), 1);
        match &skill_results[0] {
            crate::scripts::SearchResult::Skill(result) => {
                assert_eq!(result.skill.title, "Incident response");
                assert!(result.match_evidence.is_some());
            }
            _ => panic!("skill identifier returned a different command family"),
        }
    }

    #[test]
    fn context_catalog_equal_scores_use_canonical_deterministic_order() {
        let alpha = catalog_script("alpha");
        let zeta = catalog_script("zeta");
        let forward = rank_context_catalog_results(
            ContextSubsearchSource::Scripts,
            "",
            &[Arc::clone(&zeta), Arc::clone(&alpha)],
            &[],
            &[],
        );
        let reverse = rank_context_catalog_results(
            ContextSubsearchSource::Scripts,
            "",
            &[alpha, zeta],
            &[],
            &[],
        );

        let forward_names: Vec<_> = forward.iter().map(|result| result.name()).collect();
        let reverse_names: Vec<_> = reverse.iter().map(|result| result.name()).collect();
        assert_eq!(forward_names, reverse_names);
        assert_eq!(forward_names, ["alpha", "zeta"]);
    }

    #[test]
    fn active_browser_history_subsearch_recognizes_empty_colon_mode() {
        assert!(active_browser_history_subsearch("@browser-history:"));
    }

    #[test]
    fn active_browser_history_subsearch_recognizes_typed_subquery() {
        assert!(active_browser_history_subsearch(
            "@browser-history:private-query"
        ));
    }

    #[test]
    fn active_browser_history_subsearch_recognizes_colonless_alias() {
        assert!(active_browser_history_subsearch("@browser-history"));
    }

    #[test]
    fn active_browser_history_subsearch_rejects_ordinary_queries() {
        assert!(!active_browser_history_subsearch("ordinary query"));
        assert!(!active_browser_history_subsearch("person@example.invalid"));
    }

    #[test]
    fn active_browser_history_subsearch_never_confuses_conversation_history() {
        assert!(!active_browser_history_subsearch("@history:"));
        assert!(!active_browser_history_subsearch("@history:private-query"));
    }

    #[test]
    fn active_browser_history_subsearch_rejects_malformed_and_partial_mentions() {
        for input in ["@", "@browser", "@browser-histor", "@browser_history:"] {
            assert!(
                !active_browser_history_subsearch(input),
                "partial or malformed mention must not start browser-history refresh: {input}"
            );
        }
    }

    #[test]
    fn parses_known_subsearch_prefixes() {
        let (source, query) = parse_context_subsearch("file", Some("readme")).unwrap();
        assert_eq!(source, ContextSubsearchSource::File);
        assert_eq!(query, "readme");
    }

    #[test]
    fn unknown_prefix_returns_none() {
        assert!(parse_context_subsearch("unknown", Some("foo")).is_none());
    }

    #[test]
    fn colon_mode_accepts_plural_alias() {
        let (source, query) = parse_context_subsearch("files", Some("readme")).unwrap();
        assert_eq!(source, ContextSubsearchSource::File);
        assert_eq!(query, "readme");
    }

    /// Root fragments (no colon typed yet) must already BE the search mode —
    /// the list switches automatically the moment the trigger is recognized,
    /// with no "press Enter to refine" picker step (user rule: no
    /// informational/initiation list items in the main menu).
    #[test]
    fn exact_root_fragment_enters_search_mode_with_empty_query() {
        for (fragment, expected) in [
            ("file", ContextSubsearchSource::File),
            ("files", ContextSubsearchSource::File),
            ("project", ContextSubsearchSource::Project),
            ("projects", ContextSubsearchSource::Project),
            ("clipboard", ContextSubsearchSource::Clipboard),
            ("history", ContextSubsearchSource::History),
            ("browser-history", ContextSubsearchSource::BrowserHistory),
            ("notes", ContextSubsearchSource::Notes),
            ("scripts", ContextSubsearchSource::Scripts),
            ("scriptlets", ContextSubsearchSource::Scriptlets),
            ("skills", ContextSubsearchSource::Skills),
            ("dictation", ContextSubsearchSource::Dictation),
            ("calendar", ContextSubsearchSource::Calendar),
            ("notifications", ContextSubsearchSource::Notifications),
        ] {
            let (source, query) = parse_context_subsearch(fragment, None)
                .unwrap_or_else(|| panic!("@{fragment} must enter search mode"));
            assert_eq!(source, expected, "@{fragment}");
            assert_eq!(query, "", "@{fragment} starts with an empty query");
        }
    }

    /// Continuations keep searching seamlessly — the user just keeps typing.
    #[test]
    fn root_fragment_continuation_becomes_the_query() {
        for (fragment, expected_source, expected_query) in [
            ("filesreadme", ContextSubsearchSource::File, "readme"),
            ("filereadme", ContextSubsearchSource::File, "readme"),
            ("clipboardsnip", ContextSubsearchSource::Clipboard, "snip"),
            ("historyagent", ContextSubsearchSource::History, "agent"),
        ] {
            let (source, query) = parse_context_subsearch(fragment, None)
                .unwrap_or_else(|| panic!("@{fragment} must stay in search mode"));
            assert_eq!(source, expected_source, "@{fragment}");
            assert_eq!(query, expected_query, "@{fragment}");
        }
    }

    /// Partial fragments are NOT yet a recognized trigger: the context
    /// catalog keeps offering completion rows (`@fi` → Files, Project Files).
    #[test]
    fn partial_and_unknown_root_fragments_keep_the_catalog() {
        for fragment in ["", "f", "fi", "fil", "clip", "hist", "selection", "zzz"] {
            assert!(
                parse_context_subsearch(fragment, None).is_none(),
                "@{fragment} must not auto-enter a search mode"
            );
        }
    }

    /// Triggers that share a prefix resolve to the most specific source.
    #[test]
    fn longest_trigger_wins_for_continuations() {
        let (source, query) = parse_context_subsearch("scriptletsfoo", None).unwrap();
        assert_eq!(source, ContextSubsearchSource::Scriptlets);
        assert_eq!(query, "foo");
        let (source, query) = parse_context_subsearch("scriptsfoo", None).unwrap();
        assert_eq!(source, ContextSubsearchSource::Scripts);
        assert_eq!(query, "foo");
    }

    #[test]
    fn escape_ref_handles_special_chars() {
        assert_eq!(escape_ref_component("hello world"), "hello%20world");
        assert_eq!(escape_ref_component("a@b"), "a%40b");
        assert_eq!(escape_ref_component("50%"), "50%25");
    }
}
