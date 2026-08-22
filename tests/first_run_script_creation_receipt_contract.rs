use script_kit_gpui::test_utils::read_source;

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing start marker: {start}"));
    let after_start = &source[start_index..];
    let end_index = after_start
        .find(end)
        .unwrap_or_else(|| panic!("missing end marker after {start}: {end}"));
    &after_start[..end_index]
}

#[test]
fn naming_prompt_scripts_write_receipt_after_template_body_before_editor() {
    let source = read_source("src/app_impl/naming_dialog.rs");
    let body = source_between(
        &source,
        "pub(crate) fn handle_naming_dialog_completion(",
        "self.open_creation_feedback_payload(",
    );

    let template_render = body
        .find("render_script_template_file")
        .expect("script-template body should be rendered before creating its final file");
    let exclusive_create = body
        .find("create_new_script_with_contents")
        .expect("script-template final bytes must use the exclusive creation handle");
    let receipt_write = body
        .find("write_script_creation_receipt_for_path")
        .expect("script creation must write a generated-script receipt");
    let editor_open = body
        .find("script_creation::open_in_editor")
        .expect("created script should still open in the editor");

    assert!(
        template_render < exclusive_create
            && exclusive_create < receipt_write
            && receipt_write < editor_open,
        "rendered final bytes must be created exclusively before receipt verification and editor launch"
    );
    assert!(
        !body[exclusive_create..editor_open].contains("std::fs::write("),
        "an exclusively created script must never be reopened for a second vulnerable template write"
    );
    assert!(
        source.contains("self.open_creation_feedback_payload("),
        "script creation should continue to hand off to CreationFeedback"
    );
}

#[test]
fn receipt_plumbing_is_script_only_and_scriptlets_stay_unverified() {
    let source = read_source("src/app_impl/naming_dialog.rs");
    let body = source_between(
        &source,
        "let rendered_script_template =",
        "self.open_creation_feedback_payload(",
    );

    let create_start = body
        .find("let create_result = match result.target {")
        .expect("naming result must retain one target-specific creation match");
    let template_preparation = &body[..create_start];
    assert!(
        template_preparation.contains("result.target == prompts::NamingTarget::Script")
            && template_preparation.contains("find_script_template")
            && template_preparation.contains("render_script_template_file"),
        "only scripts may resolve and render their final starter before file creation"
    );

    let creation_match = source_between(
        &body[create_start..],
        "let create_result = match result.target {",
        "match create_result {",
    );
    let script_arm = source_between(
        creation_match,
        "prompts::NamingTarget::Script =>",
        "prompts::NamingTarget::Extension =>",
    );
    assert!(
        script_arm.contains("create_new_script_with_contents")
            && script_arm.contains("create_new_script(&filename_stem)"),
        "scripts must use exclusive final-byte creation while preserving their default starter fallback"
    );

    let extension_arm = source_between(creation_match, "prompts::NamingTarget::Extension =>", "};");
    assert!(
        extension_arm.contains("script_creation::create_new_scriptlet"),
        "extension/scriptlet creation should keep the existing create_new_scriptlet path"
    );
    assert!(
        !extension_arm.contains("write_script_creation_receipt_for_path"),
        "scriptlet creation must not pretend to have TypeScript generated-script verification"
    );

    let receipt_region = &body[body
        .find("match create_result {")
        .expect("successful creation must precede receipt verification")..];
    let receipt_guard = receipt_region
        .find("if result.target == prompts::NamingTarget::Script")
        .expect("receipt verification must remain guarded to scripts");
    assert!(
        receipt_region[receipt_guard..].contains("write_script_creation_receipt_for_path"),
        "script-only success handling must own generated-script receipt verification"
    );
}

#[test]
fn script_creation_receipts_use_existing_generated_script_receipt_schema() {
    let generator = read_source("src/ai/script_generation.rs");
    let helper = source_between(
        &generator,
        "pub(crate) fn write_script_creation_receipt_for_path(",
        "pub fn extract_current_app_recipe_from_script(",
    );

    for required in [
        "GeneratedScriptReceipt",
        "AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION",
        "audit_generated_script_contract(&source)",
        "verify_generated_script_with_bun_build(script_path)",
        "generated_script_receipt_path(script_path)",
        "write_generated_script_receipt(&receipt_path, &receipt)?",
        "current_app_recipe: None",
    ] {
        assert!(
            helper.contains(required),
            "script-creation receipt helper must reuse existing receipt contract: {required}"
        );
    }
    assert!(
        helper.contains("file_stem()"),
        "receipt slug must derive from the actual created file stem to preserve collision suffixes"
    );
}

#[test]
fn generated_script_build_verification_externalizes_scriptkit_sdk() {
    let generator = read_source("src/ai/script_generation.rs");
    let verifier = source_between(
        &generator,
        "fn verify_generated_script_with_bun_build(",
        "pub(crate) fn write_script_creation_receipt_for_path(",
    );

    assert!(
        verifier.contains("\"--external\".to_string()")
            && verifier.contains("SCRIPT_KIT_SDK_IMPORT_MODULE.to_string()"),
        "receipt verification command must externalize @scriptkit/sdk"
    );
    assert!(
        verifier.contains(".arg(\"--external\")")
            && verifier.contains(".arg(SCRIPT_KIT_SDK_IMPORT_MODULE)"),
        "spawned verification command must pass the externalization args"
    );
}
