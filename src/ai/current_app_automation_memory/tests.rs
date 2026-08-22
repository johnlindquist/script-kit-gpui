use super::*;
use crate::ai::script_generation::{
    GeneratedScriptContractAudit, GeneratedScriptMetadataStyle, GeneratedScriptReceipt,
    GeneratedScriptVerificationReceipt, AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
};
use crate::menu_bar::current_app_commands::{
    CurrentAppCommandRecipe, CurrentAppIntentTraceReceipt, CurrentAppScriptPromptReceipt,
};

fn make_test_recipe(bundle_id: &str, effective_query: &str) -> CurrentAppCommandRecipe {
    CurrentAppCommandRecipe {
        schema_version: 1,
        recipe_type: "currentAppCommand".to_string(),
        raw_query: effective_query.to_string(),
        effective_query: effective_query.to_string(),
        suggested_script_name: "test-script".to_string(),
        trace: CurrentAppIntentTraceReceipt {
            schema_version: 1,
            source: "test".to_string(),
            app_name: "TestApp".to_string(),
            bundle_id: bundle_id.to_string(),
            raw_query: effective_query.to_string(),
            effective_query: effective_query.to_string(),
            normalized_query: effective_query.to_lowercase(),
            top_level_menu_count: 0,
            leaf_entry_count: 0,
            filtered_entries: 0,
            exact_matches: 0,
            action: "generate_script".to_string(),
            selected_entry: None,
            candidates: vec![],
            prompt_receipt: None,
            prompt_preview: None,
        },
        prompt_receipt: CurrentAppScriptPromptReceipt {
            app_name: "TestApp".to_string(),
            bundle_id: bundle_id.to_string(),
            total_menu_items: 0,
            included_menu_items: 0,
            included_user_request: true,
            included_selected_text: false,
            included_browser_url: false,
        },
        prompt: "test prompt".to_string(),
    }
}

fn make_test_receipt(
    slug: &str,
    receipt_path: &str,
    recipe: Option<CurrentAppCommandRecipe>,
) -> GeneratedScriptReceipt {
    GeneratedScriptReceipt {
        schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
        prompt: "summarize this article".to_string(),
        slug: slug.to_string(),
        slug_source: slug.to_string(),
        slug_source_kind: "test".to_string(),
        model_id: "test-model".to_string(),
        provider_id: "test-provider".to_string(),
        script_path: format!("/tmp/test-scripts/{slug}.ts"),
        receipt_path: receipt_path.to_string(),
        shell_execution_warning: false,
        contract: GeneratedScriptContractAudit {
            metadata_style: GeneratedScriptMetadataStyle::CommentHeaders,
            has_name: true,
            has_description: true,
            has_kit_import: true,
            has_current_app_recipe_header: recipe.is_some(),
            current_app_recipe_header_at_top: recipe.is_some(),
            declared_capabilities: vec![],
            execution_topology: None,
            metadata_parse_errors: vec![],
            capability_issues: vec![],
            warnings: vec![],
        },
        verification: GeneratedScriptVerificationReceipt::skipped("unit_test_fixture"),
        current_app_recipe: recipe,
    }
}

#[test]
fn current_app_automation_memory_index_path_is_stable() {
    let path = current_app_automation_memory_index_path().expect("should resolve path");
    assert!(
        path.ends_with(".scriptkit/scripts/.current-app-automation-memory.json"),
        "unexpected path: {}",
        path.display()
    );
}

#[test]
fn private_current_app_automation_memory_upsert_is_real_atomic_and_idempotent() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let index_path = dir.path().join(".current-app-automation-memory.json");

    let recipe = make_test_recipe("com.apple.Safari", "summarize this article");
    let mut receipt = make_test_receipt(
        "summarize-article",
        "/tmp/test-scripts/summarize-article.scriptkit.json",
        Some(recipe),
    );

    upsert_current_app_automation_memory_from_receipt_at(&index_path, &receipt).unwrap();
    let parsed = read_current_app_automation_memory_index_at(&index_path).unwrap();
    assert_eq!(parsed.len(), 1, "first write should produce one entry");
    assert_eq!(parsed[0].prompt, "summarize this article");

    receipt.prompt = "updated private medical prompt".to_owned();
    upsert_current_app_automation_memory_from_receipt_at(&index_path, &receipt).unwrap();
    let parsed_2 = read_current_app_automation_memory_index_at(&index_path).unwrap();
    assert_eq!(
        parsed_2.len(),
        1,
        "second upsert with same receipt_path should still produce one entry"
    );
    assert_eq!(
        parsed_2[0].prompt, "updated private medical prompt",
        "the actual production upsert should replace the owned entry"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&index_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn private_current_app_automation_memory_repairs_legacy_permissions_before_reading() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir().expect("isolated legacy automation-memory fixture");
    let path = directory.path().join("private-automation-memory.json");
    let entry = make_memory_entry(
        "com.apple.Safari",
        "private patient appointment",
        "private-recipe",
    );
    std::fs::write(&path, serde_json::to_vec(&vec![entry]).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let entries = read_current_app_automation_memory_index_at(&path).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].effective_query, "private patient appointment");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn private_current_app_automation_memory_rejects_symlinks_without_exposing_private_paths() {
    let directory = tempfile::tempdir().expect("isolated automation-memory symlink fixture");
    let external = directory.path().join("unrelated-private-data.json");
    let planted = directory
        .path()
        .join("secret-cancer-patient-automation.json");
    std::fs::write(&external, "preserve unrelated private data").unwrap();
    std::os::unix::fs::symlink(&external, &planted).unwrap();
    let receipt = make_test_receipt(
        "private-recipe",
        "/tmp/private-recipe.scriptkit.json",
        Some(make_test_recipe("com.apple.Safari", "private prompt")),
    );

    let read_error = read_current_app_automation_memory_index_at(&planted)
        .expect_err("private memory must not read a planted symlink");
    assert!(!read_error.to_string().contains("secret-cancer-patient"));
    let write_error = upsert_current_app_automation_memory_from_receipt_at(&planted, &receipt)
        .expect_err("private memory must not write through a planted symlink");
    assert!(!write_error.to_string().contains("secret-cancer-patient"));
    assert_eq!(
        std::fs::read_to_string(&external).unwrap(),
        "preserve unrelated private data"
    );
    assert!(std::fs::symlink_metadata(&planted)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn normalize_automation_memory_text_handles_arrows_and_case() {
    assert_eq!(
        normalize_automation_memory_text("File → Save As"),
        "file save as"
    );
    assert_eq!(
        normalize_automation_memory_text("  Hello   World  "),
        "hello world"
    );
    assert_eq!(normalize_automation_memory_text(""), "");
}

#[test]
fn lookup_key_is_deterministic() {
    let recipe = make_test_recipe("com.apple.Safari", "summarize this article");
    let key1 = current_app_recipe_lookup_key(&recipe);
    let key2 = current_app_recipe_lookup_key(&recipe);
    assert_eq!(key1, key2);
    assert_eq!(key1, "com apple safari::summarize this article");
}

#[test]
fn score_candidate_exact_match_returns_high_score() {
    let recipe = make_test_recipe("com.apple.Safari", "summarize this article");
    let receipt = make_test_receipt(
        "summarize-article",
        "/tmp/test.scriptkit.json",
        Some(recipe),
    );

    let recipe_ref = receipt.current_app_recipe.clone().unwrap();
    let entry = CurrentAppAutomationMemoryIndexEntry {
        schema_version: 1,
        slug: receipt.slug.clone(),
        script_path: receipt.script_path.clone(),
        receipt_path: receipt.receipt_path.clone(),
        bundle_id: "com.apple.Safari".to_string(),
        app_name: "Safari".to_string(),
        effective_query: "summarize this article".to_string(),
        raw_query: "summarize this article".to_string(),
        prompt: receipt.prompt.clone(),
        provider_id: receipt.provider_id.clone(),
        model_id: receipt.model_id.clone(),
        lookup_key: current_app_recipe_lookup_key(&recipe_ref),
        auto_replay_eligible: true,
        written_at_unix_ms: 1000,
        recipe: recipe_ref,
    };

    let score = score_candidate("summarize this article", &entry);
    assert!(
        score >= 0.90,
        "exact match should score >= 0.90, got {score}"
    );
}

#[test]
fn score_candidate_partial_overlap_returns_moderate_score() {
    let recipe = make_test_recipe("com.apple.Safari", "summarize this article");
    let recipe_ref = recipe.clone();
    let entry = CurrentAppAutomationMemoryIndexEntry {
        schema_version: 1,
        slug: "test".to_string(),
        script_path: "/tmp/test.ts".to_string(),
        receipt_path: "/tmp/test.scriptkit.json".to_string(),
        bundle_id: "com.apple.Safari".to_string(),
        app_name: "Safari".to_string(),
        effective_query: "summarize this article".to_string(),
        raw_query: "summarize this article".to_string(),
        prompt: "test".to_string(),
        provider_id: "test".to_string(),
        model_id: "test".to_string(),
        lookup_key: current_app_recipe_lookup_key(&recipe_ref),
        auto_replay_eligible: true,
        written_at_unix_ms: 1000,
        recipe: recipe_ref,
    };

    let score = score_candidate("summarize this article and save it to notes", &entry);
    assert!(
        score > 0.0 && score < 0.90,
        "partial overlap should score between 0 and 0.90, got {score}"
    );
}

#[test]
fn score_candidate_no_overlap_returns_low_score() {
    let recipe = make_test_recipe("com.apple.Safari", "summarize this article");
    let recipe_ref = recipe.clone();
    let entry = CurrentAppAutomationMemoryIndexEntry {
        schema_version: 1,
        slug: "test".to_string(),
        script_path: "/tmp/test.ts".to_string(),
        receipt_path: "/tmp/test.scriptkit.json".to_string(),
        bundle_id: "com.apple.Safari".to_string(),
        app_name: "Safari".to_string(),
        effective_query: "summarize this article".to_string(),
        raw_query: "summarize this article".to_string(),
        prompt: "test".to_string(),
        provider_id: "test".to_string(),
        model_id: "test".to_string(),
        lookup_key: current_app_recipe_lookup_key(&recipe_ref),
        auto_replay_eligible: true,
        written_at_unix_ms: 1000,
        recipe: recipe_ref,
    };

    let score = score_candidate("completely unrelated query about dogs", &entry);
    assert!(score < 0.55, "no overlap should score < 0.55, got {score}");
}

#[test]
fn private_current_app_automation_memory_ignores_receipts_without_recipes() {
    let directory = tempfile::tempdir().expect("isolated empty automation-memory fixture");
    let path = directory.path().join("must-not-exist.json");
    let receipt = make_test_receipt("no-recipe", "/tmp/no-recipe.scriptkit.json", None);
    upsert_current_app_automation_memory_from_receipt_at(&path, &receipt)
        .expect("recipe-free receipt should have no persistence side effects");
    assert!(!path.exists());
}

// ---------------------------------------------------------------------------
// Flow-integration tests: verify resolve_current_app_automation_from_memory
// drives the correct decision for the GenerateScript branch in
// builtin_execution.rs.
// ---------------------------------------------------------------------------

fn make_test_snapshot(
    bundle_id: &str,
    app_name: &str,
) -> crate::menu_bar::current_app_commands::FrontmostMenuSnapshot {
    crate::menu_bar::current_app_commands::FrontmostMenuSnapshot {
        pid: 42,
        app_name: app_name.to_string(),
        bundle_id: bundle_id.to_string(),
        items: vec![],
    }
}

fn write_index_to_temp(dir: &std::path::Path, entries: &[CurrentAppAutomationMemoryIndexEntry]) {
    let scripts_dir = dir.join(".scriptkit").join("scripts");
    std::fs::create_dir_all(&scripts_dir).expect("create scripts dir");
    let index_path = scripts_dir.join(".current-app-automation-memory.json");
    let json = serde_json::to_string_pretty(entries).expect("serialize index");
    std::fs::write(index_path, json).expect("write index");
}

fn make_memory_entry(
    bundle_id: &str,
    effective_query: &str,
    slug: &str,
) -> CurrentAppAutomationMemoryIndexEntry {
    let recipe = make_test_recipe(bundle_id, effective_query);
    let recipe_ref = recipe.clone();
    CurrentAppAutomationMemoryIndexEntry {
        schema_version: CURRENT_APP_AUTOMATION_MEMORY_INDEX_SCHEMA_VERSION,
        slug: slug.to_string(),
        script_path: format!("/tmp/test-scripts/{slug}.ts"),
        receipt_path: format!("/tmp/test-scripts/{slug}.scriptkit.json"),
        bundle_id: bundle_id.to_string(),
        app_name: "TestApp".to_string(),
        effective_query: effective_query.to_string(),
        raw_query: effective_query.to_string(),
        prompt: format!("test prompt for {effective_query}"),
        provider_id: "test-provider".to_string(),
        model_id: "test-model".to_string(),
        lookup_key: current_app_recipe_lookup_key(&recipe_ref),
        auto_replay_eligible: true,
        written_at_unix_ms: 1000,
        recipe: recipe_ref,
    }
}

#[test]
fn do_in_current_app_generate_script_replays_from_memory() {
    let dir = tempfile::tempdir().expect("create temp dir");

    let entry = make_memory_entry(
        "com.apple.Safari",
        "summarize this article",
        "summarize-article",
    );
    write_index_to_temp(dir.path(), &[entry]);

    // Point HOME at the temp dir so the resolver reads our index
    // SAFETY: test-only; each test uses a unique temp dir so no collision risk
    #[allow(deprecated)]
    unsafe {
        std::env::set_var("HOME", dir.path().to_str().unwrap())
    };

    let snapshot = make_test_snapshot("com.apple.Safari", "TestApp");
    let decision = resolve_current_app_automation_from_memory(
        "summarize this article",
        &snapshot,
        &[], // no builtin entries → replay routes to generate_script
        None,
        None,
    )
    .expect("resolve should succeed");

    // Exact query match scores >= 0.90 but the test recipe's prompt ("test prompt")
    // doesn't match the live-rebuilt prompt, so verification produces a warning,
    // routing to repair_recipe instead of replay_recipe.
    assert_eq!(
        decision.action, "repair_recipe",
        "exact match with prompt drift should repair; got action={}, reason={}",
        decision.action, decision.reason
    );
    assert!(
        decision.best_score >= 0.90,
        "exact match should score >= 0.90, got {}",
        decision.best_score
    );
    assert!(
        decision.matched.is_some(),
        "repair decision must include matched entry"
    );
    assert!(
        decision.replay.is_some(),
        "repair decision must include replay receipt"
    );
}

#[test]
fn do_in_current_app_generate_script_repairs_from_memory() {
    let dir = tempfile::tempdir().expect("create temp dir");

    let entry = make_memory_entry(
        "com.apple.Safari",
        "summarize this article",
        "summarize-article",
    );
    write_index_to_temp(dir.path(), &[entry]);

    // SAFETY: test-only; each test uses a unique temp dir so no collision risk
    #[allow(deprecated)]
    unsafe {
        std::env::set_var("HOME", dir.path().to_str().unwrap())
    };

    let snapshot = make_test_snapshot("com.apple.Safari", "TestApp");
    // Similar but not identical query → repair path
    let decision = resolve_current_app_automation_from_memory(
        "summarize this article and save it to notes",
        &snapshot,
        &[],
        None,
        None,
    )
    .expect("resolve should succeed");

    // The jaccard similarity between "summarize this article" and
    // "summarize this article and save it to notes" is 3/8 = 0.375.
    // With score = 0.20 * 0.375 + 0.10 * 0.375 = 0.1125, this falls
    // below the 0.55 threshold → generate_new.
    assert_eq!(
        decision.action, "generate_new",
        "dissimilar query should generate new; got action={}, reason={}",
        decision.action, decision.reason
    );
    assert!(
        decision.best_score < 0.55,
        "dissimilar query should score < 0.55, got {}",
        decision.best_score
    );
}

#[test]
fn do_in_current_app_generate_script_miss_falls_back_to_generation() {
    let dir = tempfile::tempdir().expect("create temp dir");

    let entry = make_memory_entry(
        "com.apple.Safari",
        "summarize this article",
        "summarize-article",
    );
    write_index_to_temp(dir.path(), &[entry]);

    // SAFETY: test-only; each test uses a unique temp dir so no collision risk
    #[allow(deprecated)]
    unsafe {
        std::env::set_var("HOME", dir.path().to_str().unwrap())
    };

    // Different bundle_id → no match
    let snapshot = make_test_snapshot("com.microsoft.VSCode", "Visual Studio Code");
    let decision = resolve_current_app_automation_from_memory(
        "summarize this article",
        &snapshot,
        &[],
        None,
        None,
    )
    .expect("resolve should succeed");

    assert_eq!(
        decision.action, "generate_new",
        "different bundle_id should generate_new; got action={}, reason={}",
        decision.action, decision.reason
    );
    assert!(
        decision.matched.is_none(),
        "generate_new should have no matched entry"
    );
    assert!(
        decision.replay.is_none(),
        "generate_new should have no replay receipt"
    );
}

#[test]
fn serde_roundtrip_index_entry() {
    let recipe = make_test_recipe("com.apple.Safari", "summarize this article");
    let recipe_ref = recipe.clone();
    let entry = CurrentAppAutomationMemoryIndexEntry {
        schema_version: 1,
        slug: "test".to_string(),
        script_path: "/tmp/test.ts".to_string(),
        receipt_path: "/tmp/test.scriptkit.json".to_string(),
        bundle_id: "com.apple.Safari".to_string(),
        app_name: "Safari".to_string(),
        effective_query: "summarize this article".to_string(),
        raw_query: "summarize this article".to_string(),
        prompt: "test prompt".to_string(),
        provider_id: "test".to_string(),
        model_id: "test".to_string(),
        lookup_key: current_app_recipe_lookup_key(&recipe_ref),
        auto_replay_eligible: true,
        written_at_unix_ms: 12345,
        recipe: recipe_ref,
    };

    let json = serde_json::to_string_pretty(&entry).expect("serialize");
    let parsed: CurrentAppAutomationMemoryIndexEntry =
        serde_json::from_str(&json).expect("deserialize");
    assert_eq!(entry, parsed);
}
