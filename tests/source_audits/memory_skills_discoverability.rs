//! Source audit for the de-scoped OpenClicky "memory/skills drawer" idea.
//!
//! The chosen direction is to keep skills/files discoverable through main input
//! sigils and existing pickers, not add a separate drawer UI.

use super::read_source as read;

#[test]
fn context_subsearch_keeps_files_and_skills_in_main_input_path() {
    let context = read("src/spine/catalog_context.rs");
    let subsearch = read("src/spine/catalog_subsearch.rs");

    for expected in [
        r#"prefix: "file""#,
        r#"title: "Files""#,
        r#"prefix: "skills""#,
        r#"title: "Skills""#,
        r#"subtitle: "Search plugin skills""#,
    ] {
        assert!(
            context.contains(expected),
            "context catalog must keep `{expected}` discoverable"
        );
    }

    // Both directions must exist: token -> source, and source -> prefix.
    //
    // The token -> source half used to be asserted as match arms
    // (`"file" => Some(Self::File)`). It is now the SUBSEARCH_TRIGGERS table,
    // which is a better mechanism than the arms it replaced — one ordered
    // table, matched case-insensitively, and able to carry aliases such as
    // `files` -> File that the match arms could not express without
    // duplication. Asserting the arm spelling made this audit red for the
    // upgrade.
    for entry in [
        r#"("file", ContextSubsearchSource::File)"#,
        r#"("skills", ContextSubsearchSource::Skills)"#,
    ] {
        assert!(
            subsearch.contains(entry),
            "context subsearch must route {entry} through the shared trigger table"
        );
    }
    for prefix in [r#"Self::File => "file""#, r#"Self::Skills => "skills""#] {
        assert!(
            subsearch.contains(prefix),
            "context subsearch must map {prefix} back to its main-input prefix"
        );
    }
}

#[test]
fn root_filter_keeps_type_skill_qualifier_without_new_drawer() {
    let filter = read("src/spine/catalog_filter.rs");

    assert!(
        filter.contains(r#"token: "type:skill""#)
            && filter.contains(r#"title: "Skills only""#)
            && filter.contains(r#"subtitle: "Find agent skills""#),
        "root search must keep skill filtering discoverable through typed qualifiers"
    );
}

#[test]
fn agent_chat_slash_skills_stage_context_parts_instead_of_opening_drawer() {
    let view = read("src/ai/agent_chat/ui/view.rs");

    assert!(
        view.contains("discover_plugin_skills(&index)")
            && view.contains("agent_chat_slash_skill_cataloged")
            && view.contains("SlashCommandPayload::PluginSkill(skill)")
            && view.contains("build_skill_slash_command_text(&skill.skill_id)")
            && view.contains("build_skill_context_part(")
            && view.contains("thread.add_context_part(part, cx)"),
        "Agent Chat slash skill acceptance must stay in the slash/context-part path"
    );
}

#[test]
fn existing_skill_contract_tests_cover_duplicate_slugs_and_staging() {
    let plugin_skill_search = read("tests/plugin_skill_search.rs");
    let agent_chat_tests = read("src/ai/agent_chat/ui/tests.rs");

    assert!(
        plugin_skill_search.contains("duplicate_skill_slugs_across_plugins_are_distinct_results")
            && plugin_skill_search.contains("skill_frecency_key_is_plugin_qualified"),
        "plugin skill search tests must keep duplicate skill slugs distinct"
    );
    assert!(
        agent_chat_tests.contains("agent_chat_plugin_slash_accept_stages_selected_skill_prompt")
            && agent_chat_tests
                .contains("agent_chat_claude_skill_staged_prompt_uses_claude_owner_phrase"),
        "Agent Chat tests must keep skill slash acceptance staged as context parts"
    );
}
