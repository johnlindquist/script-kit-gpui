//! Claim-gating tests for the mdflow pivot: flows are the primary
//! main-menu rows, so the scorer and unified ordering must actually put
//! them there. These gate the "flows outrank scripts" claim in the
//! flows-hoist work (43609c76c).

use crate::flows::model::{FlowDescriptor, FlowSource};

use super::super::flows::fuzzy_search_flows;
use super::super::unified::{fuzzy_search_unified_all_with_skills_and_flows, result_type_order};
use super::super::SearchResult;
use super::core_search::make_script;

fn make_flow(name: &str, description: Option<&str>) -> FlowDescriptor {
    FlowDescriptor {
        id: format!("project:{name}"),
        path: format!("/test/flows/{name}.md"),
        source: FlowSource::Project,
        name: name.to_string(),
        description: description.map(|d| d.to_string()),
        engine: "codex".to_string(),
        engine_source: None,
        inputs: Vec::new(),
        is_workflow: false,
        interactive: false,
        mtime_ms: 0,
        origin: Some("repo flows/".to_string()),
        wrapper_command: None,
    }
}

#[test]
fn flow_friendly_name_match_outranks_description_match() {
    let flows = vec![
        make_flow("flow-gmail", Some("Probe agent that triages email.")),
        make_flow("flow-notes", Some("Talks about gmail sometimes.")),
    ];
    let matches = fuzzy_search_flows(&flows, "gmail");
    assert_eq!(matches.len(), 2);
    assert_eq!(
        matches[0]
            .flow
            .as_ref()
            .expect("identity row has descriptor")
            .name,
        "flow-gmail"
    );
    assert!(matches[0].score > matches[1].score);
}

#[test]
fn flow_raw_name_matches_without_highlights() {
    let flows = vec![make_flow("flow-gmail", None)];
    let matches = fuzzy_search_flows(&flows, "flow-gm");
    assert_eq!(matches.len(), 1);
    assert!(
        matches[0].match_indices.name_indices.is_empty(),
        "raw-name matches must not highlight the friendly name"
    );
}

#[test]
fn empty_query_lists_all_flows_alphabetically() {
    let flows = vec![make_flow("flow-zeta", None), make_flow("flow-alpha", None)];
    let matches = fuzzy_search_flows(&flows, "");
    let names: Vec<&str> = matches.iter().map(|m| m.display_name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "Zeta"]);
}

#[test]
fn unified_search_ranks_flow_above_script_of_same_name() {
    // The primary-experience claim: when a flow and a script both match the
    // query, the flow row must sort first (type order breaks score ties,
    // and a friendly-name flow match scores at the primary tier).
    let scripts = vec![make_script("Gmail Agent", Some("legacy script"))];
    let flows = vec![make_flow("flow-gmail-agent", Some("Triage email."))];
    let results = fuzzy_search_unified_all_with_skills_and_flows(
        &scripts,
        &[],
        &[],
        &[],
        &[],
        &flows,
        "gmail agent",
    );
    assert!(!results.is_empty());
    assert!(
        matches!(results[0], SearchResult::Flow(_)),
        "flow must lead; got {:?}",
        results
            .iter()
            .map(|r| std::mem::discriminant(r))
            .collect::<Vec<_>>()
    );
    assert!(results.iter().any(|r| matches!(r, SearchResult::Script(_))));
}

#[test]
fn unified_empty_flow_corpus_changes_nothing() {
    let scripts = vec![make_script("Gmail Agent", None)];
    let with_flows =
        fuzzy_search_unified_all_with_skills_and_flows(&scripts, &[], &[], &[], &[], &[], "gmail");
    assert!(with_flows
        .iter()
        .all(|r| !matches!(r, SearchResult::Flow(_))));
    assert!(with_flows
        .iter()
        .any(|r| matches!(r, SearchResult::Script(_))));
}

#[test]
fn multi_word_recall_bar_words_match_in_any_order() {
    // Flows are the primary rows, so the launcher must recall a flow from
    // the words the user remembers, not the order the filename chose.
    // Two-sided: the flow carrying every query word must match; a flow
    // carrying only one of the words must not ride along.
    let flows = vec![
        make_flow(
            "flow-review-staged-changes",
            Some("Look over the diff before commit."),
        ),
        make_flow("flow-review-inbox", Some("Morning inbox pass.")),
    ];

    // Reference anchors: the in-order phrase and the adjacent-word swap
    // already match today (exact substring / compact fuzzy). If these fail,
    // suspect the harness, not the word-order support.
    for anchor in ["review staged", "staged review"] {
        let hits = fuzzy_search_flows(&flows, anchor);
        assert_eq!(hits.len(), 1, "anchor \"{anchor}\" must match");
        assert_eq!(
            hits[0]
                .flow
                .as_ref()
                .expect("identity row has descriptor")
                .name,
            "flow-review-staged-changes"
        );
    }

    // The bar: words that are NOT adjacent in the name still recall it in
    // any order. Compact-fuzzy spans cannot carry this case — it rides on
    // nucleo's AND-matched atoms plus the structured-abbreviation gate
    // (word-boundary runs), so tightening either one busts this bar.
    let out_of_order = fuzzy_search_flows(&flows, "changes review");
    assert_eq!(
        out_of_order.len(),
        1,
        "recall: non-adjacent out-of-order words must find the flow; \
         precision: flow-review-inbox carries only one word and must not match"
    );
    assert_eq!(
        out_of_order[0]
            .flow
            .as_ref()
            .expect("identity row has descriptor")
            .name,
        "flow-review-staged-changes"
    );

    // Precision: a word that appears in no flow keeps the result empty even
    // when the other word is common to both flows.
    let miss = fuzzy_search_flows(&flows, "review deploy");
    assert!(
        miss.is_empty(),
        "a missing word must not degrade to OR-matching"
    );
}

#[test]
fn flow_result_type_order_is_topmost() {
    let flows = vec![make_flow("flow-gmail", None)];
    let flow_result = {
        let mut matches = fuzzy_search_flows(&flows, "gmail");
        SearchResult::Flow(matches.remove(0))
    };
    let order = result_type_order(&flow_result);
    assert_eq!(order, -1, "flows share the topmost type order");
}
