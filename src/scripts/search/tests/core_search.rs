use std::path::PathBuf;
use std::sync::Arc;

use crate::builtins::{BuiltInEntry, BuiltInFeature, BuiltInGroup};
use crate::plugins::PluginSkill;
use crate::scripts::{MatchEvidenceField, ScriptMatchKind};

use super::super::*;

// ============================================
// Search scoring tests
// ============================================

pub(super) fn make_script(name: &str, desc: Option<&str>) -> Arc<Script> {
    Arc::new(Script {
        name: name.to_string(),
        path: PathBuf::from(format!(
            "/test/{}.ts",
            name.to_lowercase().replace(' ', "-")
        )),
        extension: "ts".to_string(),
        description: desc.map(|d| d.to_string()),
        ..Default::default()
    })
}

fn make_script_with_body(name: &str, body: &str) -> Arc<Script> {
    Arc::new(Script {
        name: name.to_string(),
        path: PathBuf::from(format!(
            "/test/{}.ts",
            name.to_lowercase().replace(' ', "-")
        )),
        extension: "ts".to_string(),
        body: Some(body.into()),
        ..Default::default()
    })
}

#[test]
fn test_exact_name_match_ranks_first() {
    let scripts = vec![
        make_script("Notes Helper", Some("Manages notes")),
        make_script("Notes", Some("Take quick notes")),
        make_script("Notebook Viewer", Some("View notebooks")),
    ];
    let results = fuzzy_search_scripts(&scripts, "Notes");
    assert!(!results.is_empty());
    // Exact match "Notes" should be first
    assert_eq!(results[0].script.name, "Notes");
}

#[test]
fn test_word_boundary_bonus() {
    let scripts = vec![
        make_script("Renewal Plan", Some("Renew subscriptions")),
        make_script("New Tab", Some("Open new tab")),
    ];
    let results = fuzzy_search_scripts(&scripts, "new");
    assert!(!results.is_empty());
    // "New Tab" should rank higher because "new" is at a word start
    assert_eq!(results[0].script.name, "New Tab");
}

#[test]
fn test_single_char_query_no_nucleo() {
    // With MIN_FUZZY_QUERY_LEN=2, single char queries should only use
    // substring matching, not nucleo fuzzy. This reduces false positives.
    let scripts = vec![
        make_script("X Tool", None),
        make_script("Backup Files", None),
    ];
    let results = fuzzy_search_scripts(&scripts, "x");
    // "X Tool" should match, but "Backup Files" should only match if
    // it actually contains "x" as substring in name/filename/path
    for r in &results {
        let name_lower = r.script.name.to_lowercase();
        let filename_lower = r.filename.to_lowercase();
        let path_lower = r.script.path.to_string_lossy().to_lowercase();
        assert!(
            name_lower.contains('x') || filename_lower.contains('x') || path_lower.contains('x'),
            "Single-char query should only match via substring, not fuzzy: {}",
            r.script.name
        );
    }
}

#[test]
fn test_compute_match_indices_for_result_handles_unicode_normalization_in_name() {
    let scripts = vec![make_script("Café Utility", Some("Unicode accent test"))];
    let mut matches = fuzzy_search_scripts(&scripts, "cafe");
    assert!(
        !matches.is_empty(),
        "Expected fuzzy search to match Unicode-normalized script name"
    );

    let result = SearchResult::Script(matches.remove(0));
    let indices = compute_match_indices_for_result(&result, "cafe");

    assert!(
        !indices.name_indices.is_empty(),
        "Unicode-normalized name match should produce highlight indices"
    );
}

#[test]
fn test_compute_match_indices_for_result_handles_unicode_normalization_in_description() {
    let scripts = vec![make_script(
        "Invoice Tool",
        Some("Résumé template generator"),
    )];
    let mut matches = fuzzy_search_scripts(&scripts, "resume");
    assert!(
        !matches.is_empty(),
        "Expected fuzzy search to match Unicode-normalized description"
    );

    let result = SearchResult::Script(matches.remove(0));
    let indices = compute_match_indices_for_result(&result, "resume");

    assert!(
        !indices.description_indices.is_empty(),
        "Unicode-normalized description match should produce highlight indices"
    );
}

#[test]
fn test_event_highlight_prefers_contiguous_substring_in_script_name() {
    let scripts = vec![make_script("The event", Some("Create an event"))];
    let mut matches = fuzzy_search_scripts(&scripts, "event");

    assert_eq!(matches.len(), 1);
    let result = SearchResult::Script(matches.remove(0));
    let indices = compute_match_indices_for_result(&result, "event");

    assert_eq!(indices.name_indices, vec![4, 5, 6, 7, 8]);
}

fn make_builtin(name: &str, description: &str) -> BuiltInEntry {
    BuiltInEntry {
        id: name.to_lowercase().replace(' ', "-"),
        name: name.to_string(),
        description: description.to_string(),
        keywords: Vec::new(),
        feature: BuiltInFeature::Settings,
        icon: None,
        group: BuiltInGroup::Core,
    }
}

fn make_app(name: &str) -> crate::app_launcher::AppInfo {
    crate::app_launcher::AppInfo {
        name: name.to_string(),
        path: PathBuf::from(format!("/Applications/{}.app", name)),
        bundle_id: None,
        icon: None,
    }
}

fn make_app_with_bundle_path(
    name: &str,
    bundle_id: Option<&str>,
    path: &str,
) -> crate::app_launcher::AppInfo {
    crate::app_launcher::AppInfo {
        name: name.to_string(),
        path: PathBuf::from(path),
        bundle_id: bundle_id.map(str::to_string),
        icon: None,
    }
}

fn make_skill(title: &str) -> Arc<PluginSkill> {
    Arc::new(PluginSkill {
        plugin_id: "test-plugin".to_string(),
        plugin_title: "Test Plugin".to_string(),
        skill_id: title.to_lowercase().replace(' ', "-"),
        path: PathBuf::from(format!(
            "/test/{}.md",
            title.to_lowercase().replace(' ', "-")
        )),
        title: title.to_string(),
        description: String::new(),
    })
}

fn make_skill_with_description(title: &str, description: &str) -> Arc<PluginSkill> {
    Arc::new(PluginSkill {
        plugin_id: "test-plugin".to_string(),
        plugin_title: "Test Plugin".to_string(),
        skill_id: title.to_lowercase().replace(' ', "-"),
        path: PathBuf::from(format!(
            "/test/{}.md",
            title.to_lowercase().replace(' ', "-")
        )),
        title: title.to_string(),
        description: description.to_string(),
    })
}

fn make_scriptlet(name: &str, description: Option<&str>) -> Arc<Scriptlet> {
    Arc::new(Scriptlet {
        icon: None,
        name: name.to_string(),
        description: description.map(str::to_string),
        code: "echo hi".to_string(),
        tool: "bash".to_string(),
        shortcut: None,
        keyword: None,
        group: None,
        plugin_id: "test-plugin".to_string(),
        plugin_title: Some("Test Plugin".to_string()),
        file_path: None,
        command: None,
        alias: None,
    })
}

#[test]
fn test_compact_fuzzy_query_keeps_meaningful_word_match() {
    let builtins = vec![make_builtin(
        "Reset Window Positions",
        "Restore all windows to default positions",
    )];

    let results = fuzzy_search_builtins(&builtins, "posit");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "Reset Window Positions");
}

#[test]
fn test_compact_fuzzy_query_rejects_sparse_builtin_name_match() {
    let builtins = vec![make_builtin(
        "Accessibility Permission Assistant",
        "Open the Permission Assistant for Accessibility",
    )];

    let results = fuzzy_search_builtins(&builtins, "posit");

    assert!(
        results.is_empty(),
        "sparse ordered letters should not admit unrelated permission assistant rows"
    );
}

#[test]
fn test_short_fuzzy_query_keeps_posi_results_targeted() {
    let builtins = vec![
        make_builtin(
            "Reset Window Positions",
            "Restore all windows to default positions",
        ),
        make_builtin(
            "Change Tone (Professional)",
            "Rewrite text in a professional tone",
        ),
        make_builtin(
            "Open Force Quit Apps",
            "Open the macOS Force Quit Applications dialog",
        ),
    ];

    let results = fuzzy_search_builtins(&builtins, "posi");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "Reset Window Positions");
}

#[test]
fn test_short_fuzzy_query_rejects_mid_word_app_matches() {
    let apps = vec![
        make_app("AirPort Base Station Agent"),
        make_app("PeopleMessageService"),
        make_app("PeopleViewService"),
    ];

    let results = fuzzy_search_apps(&apps, "posi");

    assert!(
        results.is_empty(),
        "short ordered query should not match app/service names through mid-word chunks"
    );
}

#[test]
fn test_apps_do_not_match_bundle_or_path_only_in_normal_search() {
    let apps = vec![make_app_with_bundle_path(
        "Safari",
        Some("com.example.positions-helper"),
        "/Applications/Position Helper.app",
    )];

    let results = fuzzy_search_apps(&apps, "posi");

    assert!(
        results.is_empty(),
        "normal app search should admit apps by visible name, not bundle id or path"
    );
}

#[test]
fn test_compact_fuzzy_query_rejects_sparse_script_description_match() {
    let scripts = vec![make_script(
        "Sync to GitHub",
        Some("Initialize git and sync the Script Kit workspace to GitHub"),
    )];

    let results = fuzzy_search_scripts(&scripts, "posit");

    assert!(
        results.is_empty(),
        "description-only sparse fuzzy matches should not clutter short ordered queries"
    );
}

#[test]
fn test_description_normalized_substring_rejects_gapped_match() {
    let scripts = vec![make_script("Helper", Some("Restore default positions"))];

    let results = fuzzy_search_scripts(&scripts, "psit");

    assert!(
        results.is_empty(),
        "description matching should require a contiguous exact/normalized substring"
    );
}

#[test]
fn test_short_fuzzy_query_rejects_sparse_script_description_match() {
    let scripts = vec![make_script(
        "Sync to GitHub",
        Some("Initialize git and sync the Script Kit workspace to GitHub"),
    )];

    let results = fuzzy_search_scripts(&scripts, "posi");

    assert!(
        results.is_empty(),
        "short ordered query should not match script descriptions through mid-word chunks"
    );
}

#[test]
fn test_short_fuzzy_query_rejects_sparse_script_body_match() {
    let scripts = vec![make_script_with_body(
        "Add to Google Calendar",
        "const capture = await createPromptFromSavedInput();",
    )];

    let results = fuzzy_search_scripts(&scripts, "posi");

    assert!(
        results.is_empty(),
        "short ordered query should not match script body lines through scattered source letters"
    );
}

#[test]
fn test_script_body_search_keeps_exact_content_match() {
    let scripts = vec![make_script_with_body(
        "Window Helper",
        "const position = await getWindowPosition();",
    )];

    let results = fuzzy_search_scripts(&scripts, "position");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "Window Helper");
    assert!(matches!(results[0].match_kind, ScriptMatchKind::Content));
    assert!(results[0].content_match.is_some());
}

#[test]
fn test_script_body_search_uses_character_indices_for_non_ascii() {
    let scripts = vec![make_script_with_body(
        "Cafe Helper",
        "const label = \"Résumé position\";",
    )];

    let results = fuzzy_search_scripts(&scripts, "resume");

    assert_eq!(results.len(), 1);
    let hit = results[0].content_match.as_ref().expect("content hit");
    assert_eq!(hit.line_match_indices, vec![15, 16, 17, 18, 19, 20]);
}

#[test]
fn test_compact_fuzzy_query_preserves_common_abbreviation() {
    let scripts = vec![make_script("Google Calendar", None)];

    let results = fuzzy_search_scripts(&scripts, "gcal");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "Google Calendar");
}

#[test]
fn test_gcal_highlight_uses_structured_abbreviation_indices() {
    let scripts = vec![make_script("Google Calendar", None)];
    let mut matches = fuzzy_search_scripts(&scripts, "gcal");

    assert_eq!(matches.len(), 1);
    let result = SearchResult::Script(matches.remove(0));
    let indices = compute_match_indices_for_result(&result, "gcal");

    assert_eq!(indices.name_indices, vec![0, 7, 8, 9]);
}

#[test]
fn test_unified_posi_filters_unrelated_main_menu_rows() {
    let builtins = vec![
        make_builtin(
            "Reset Window Positions",
            "Restore all windows to default positions",
        ),
        make_builtin(
            "Open Force Quit Apps",
            "Open the macOS Force Quit Applications dialog",
        ),
        make_builtin("Draft Social Post", "Create a social media post"),
    ];
    let apps = vec![
        make_app("AirPort Base Station Agent"),
        make_app("PeopleMessageService"),
        make_app("PeopleViewService"),
    ];
    let scripts = vec![
        make_script(
            "Sync to GitHub",
            Some("Initialize git and sync the Script Kit workspace to GitHub"),
        ),
        make_script_with_body(
            "Add to Google Calendar",
            "const capture = await createPromptFromSavedInput();",
        ),
    ];

    let results = fuzzy_search_unified_all(&scripts, &[], &builtins, &apps, "posi");
    let names = results.iter().map(SearchResult::name).collect::<Vec<_>>();

    assert_eq!(names, vec!["Reset Window Positions"]);
}

#[test]
fn test_type_app_filter_excludes_matching_skills() {
    let apps = vec![make_app("Calendar")];
    let skills = vec![make_skill("Calendar Helper")];

    let results =
        fuzzy_search_unified_all_with_skills(&[], &[], &[], &apps, &skills, "type:app calendar");
    let names = results.iter().map(SearchResult::name).collect::<Vec<_>>();

    assert_eq!(names, vec!["Calendar"]);
}

#[test]
fn test_primary_name_tier_beats_body_only_match() {
    let builtins = vec![make_builtin("Position", "A visible exact command")];
    let scripts = vec![make_script_with_body(
        "Window Helper",
        "const position = await getWindowPosition();",
    )];

    let results = fuzzy_search_unified_all(&scripts, &[], &builtins, &[], "position");

    assert!(!results.is_empty());
    assert_eq!(results[0].name(), "Position");
}

#[test]
fn test_scriptlet_description_evidence_prevents_sparse_name_highlight() {
    let scriptlets = vec![make_scriptlet(
        "PeopleMessageService",
        Some("Restore default positions"),
    )];

    let mut matches = fuzzy_search_scriptlets(&scriptlets, "posi");

    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].match_evidence.as_ref().map(|e| e.field),
        Some(MatchEvidenceField::Description)
    );

    let result = SearchResult::Scriptlet(matches.remove(0));
    let indices = compute_match_indices_for_result(&result, "posi");

    assert!(indices.name_indices.is_empty());
    assert_eq!(indices.description_indices, vec![16, 17, 18, 19]);
}

#[test]
fn test_punctuation_only_query_does_not_match_scriptlet_metadata() {
    let mut scriptlet = (*make_scriptlet("Format Text", Some("Normalize prose."))).clone();
    scriptlet.file_path = Some("/Users/test/.kit/scriptlets/format-text.md".to_string());
    scriptlet.keyword = Some("format.text".to_string());
    scriptlet.alias = Some("fmt.".to_string());
    scriptlet.shortcut = Some("cmd+.".to_string());
    scriptlet.group = Some("utilities.tools".to_string());
    scriptlet.tool = "bash.shell".to_string();
    let scriptlets = vec![Arc::new(scriptlet)];

    for query in [".", "...", ":", ";", "!"] {
        assert!(
            fuzzy_search_scriptlets(&scriptlets, query).is_empty(),
            "query {query:?} should not produce scriptlet matches"
        );
    }
}

#[test]
fn test_text_query_still_matches_scriptlet_metadata() {
    let mut scriptlet = (*make_scriptlet("Format Text", Some("Normalize prose"))).clone();
    scriptlet.file_path = Some("/Users/test/.kit/scriptlets/format-text.md".to_string());
    scriptlet.keyword = Some("format-text".to_string());
    scriptlet.alias = Some("fmt".to_string());
    scriptlet.group = Some("utilities".to_string());
    let scriptlets = vec![Arc::new(scriptlet)];

    assert_eq!(fuzzy_search_scriptlets(&scriptlets, "normalize").len(), 1);
    assert_eq!(fuzzy_search_scriptlets(&scriptlets, "format-text").len(), 1);
    assert_eq!(fuzzy_search_scriptlets(&scriptlets, "utilities").len(), 1);
}

#[test]
fn test_builtin_description_evidence_prevents_sparse_name_highlight() {
    let builtins = vec![make_builtin(
        "PeopleMessageService",
        "Restore default positions",
    )];

    let mut matches = fuzzy_search_builtins(&builtins, "posi");

    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].match_evidence.as_ref().map(|e| e.field),
        Some(MatchEvidenceField::Description)
    );

    let result = SearchResult::BuiltIn(matches.remove(0));
    let indices = compute_match_indices_for_result(&result, "posi");

    assert!(indices.name_indices.is_empty());
    assert_eq!(indices.description_indices, vec![16, 17, 18, 19]);
}

#[test]
fn test_skill_description_evidence_prevents_stale_title_indices() {
    let skills = vec![make_skill_with_description(
        "PeopleMessageService",
        "Restore default positions",
    )];

    let mut matches = fuzzy_search_skills(&skills, "posi");

    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].match_evidence.as_ref().map(|e| e.field),
        Some(MatchEvidenceField::Description)
    );

    let result = SearchResult::Skill(matches.remove(0));
    let indices = compute_match_indices_for_result(&result, "posi");

    assert!(indices.name_indices.is_empty());
    assert_eq!(indices.description_indices, vec![16, 17, 18, 19]);
}
