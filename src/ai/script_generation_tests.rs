#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn generated_receipt_fixture(path: &Path, prompt: &str) -> GeneratedScriptReceipt {
        GeneratedScriptReceipt {
            schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
            prompt: safe_generated_script_detail(prompt),
            slug: "fixture".to_string(),
            slug_source: "Fixture".to_string(),
            slug_source_kind: "metadata_export".to_string(),
            model_id: "test-model".to_string(),
            provider_id: "test-provider".to_string(),
            script_path: path.with_extension("ts").display().to_string(),
            receipt_path: path.display().to_string(),
            shell_execution_warning: false,
            contract: GeneratedScriptContractAudit {
                metadata_style: GeneratedScriptMetadataStyle::MetadataExport,
                has_name: true,
                has_description: true,
                has_kit_import: false,
                has_current_app_recipe_header: false,
                current_app_recipe_header_at_top: true,
                declared_capabilities: vec![],
                execution_topology: None,
                metadata_parse_errors: vec![],
                capability_issues: vec![],
                warnings: vec![],
            },
            verification: GeneratedScriptVerificationReceipt::skipped("safe_test_fixture"),
            current_app_recipe: None,
        }
    }

    #[test]
    fn test_slugify_script_name_handles_spaces_and_symbols() {
        assert_eq!(
            slugify_script_name("Build: API Client!"),
            "build-api-client"
        );
        assert_eq!(slugify_script_name("  ___  "), "ai-script");
    }

    #[test]
    fn generated_persistence_plan_applies_identical_shell_policy_to_both_save_paths() {
        let source = r#"import { execSync } from "child_process";
const output = execSync("printenv");"#;
        let provider_save = generated_script_persistence_plan(
            "Show deployment status",
            source,
            "deployment-status",
            None,
        )
        .expect("provider-result save has a valid derived name");
        let direct_save = generated_script_persistence_plan(
            "Show deployment status",
            source,
            "deployment-status",
            Some("../../Private API_TOKEN"),
        )
        .expect("Agent Chat save sanitizes its override before any filesystem work");

        assert!(provider_save.shell_execution_warning);
        assert!(direct_save.shell_execution_warning);
        assert_eq!(
            provider_save.suspicious_shell_patterns,
            direct_save.suspicious_shell_patterns
        );
        assert_eq!(direct_save.requested_slug, "private-api-token");
        assert!(!direct_save.requested_slug.contains(".."));
        assert!(!direct_save.requested_slug.contains('/'));

        let explicitly_approved = generated_script_persistence_plan(
            "Run a shell command in the terminal",
            source,
            "approved-shell",
            None,
        )
        .expect("an explicitly approved shell request has a valid plan");
        assert!(!explicitly_approved.shell_execution_warning);
    }

    #[test]
    fn generated_persistence_plan_rejects_empty_hostile_override_without_leaking_it() {
        let raw_override = "../../🔥";
        let error = generated_script_persistence_plan(
            "Summarize this note",
            "await div(\"safe\");",
            "safe-default",
            Some(raw_override),
        )
        .expect_err("a hostile override that sanitizes to empty must fail before creation");
        let safe_error = error.to_string();
        assert!(safe_error.contains("empty after safe sanitization"));
        assert!(!safe_error.contains(raw_override));
        assert!(!safe_error.contains(".."));
    }

    #[test]
    fn generated_persistence_identity_uses_actual_collision_resolved_safe_stem() {
        let path = Path::new("/synthetic/plugins/main/scripts/safe-script-7.ts");
        let slug = generated_script_created_slug(path)
            .expect("the collision-resolved filename is the only receipt identity");
        assert_eq!(slug, "safe-script-7");

        let unsafe_path = Path::new("/synthetic/plugins/main/scripts/Unsafe Script.ts");
        let error = generated_script_created_slug(unsafe_path)
            .expect_err("an unsanitized external path must never enter a script receipt");
        assert!(error.to_string().contains("safely sanitized"));
    }

    #[test]
    fn generated_receipt_atomic_rewrites_preserve_safe_content_without_temp_artifacts() {
        let temp = tempdir().expect("create isolated receipt workspace");
        let destination = temp.path().join("fixture.scriptkit.json");
        let mut receipt = generated_receipt_fixture(
            &destination,
            "Summarize this article token=sk-private-token Authorization: Bearer sk-private-bearer api_key=sk-private-api",
        );
        write_generated_script_receipt(&destination, &receipt)
            .expect("publish initial receipt atomically");
        receipt.model_id = "updated-model".to_string();
        write_generated_script_receipt(&destination, &receipt)
            .expect("replace existing regular receipt atomically");

        let persisted = fs::read_to_string(&destination).expect("read final receipt");
        let saved: GeneratedScriptReceipt =
            serde_json::from_str(&persisted).expect("parse final receipt");
        assert_eq!(saved.model_id, "updated-model");
        assert!(saved.prompt.contains("Summarize this article"));
        assert!(saved.prompt.contains("[REDACTED]"));
        assert!(!persisted.contains("sk-private-token"));
        assert!(!persisted.contains("sk-private-bearer"));
        assert!(!persisted.contains("sk-private-api"));
        let entries = fs::read_dir(temp.path())
            .expect("inspect isolated receipt workspace")
            .count();
        assert_eq!(
            entries, 1,
            "temporary sibling receipts must always be cleaned"
        );
    }

    #[cfg(unix)]
    #[test]
    fn generated_receipt_refuses_symlink_destination_without_touching_target() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("create isolated receipt workspace");
        let protected = temp.path().join("protected.txt");
        fs::write(&protected, "protected-content").expect("seed protected fixture");
        let destination = temp.path().join("fixture.scriptkit.json");
        symlink(&protected, &destination).expect("install isolated hostile receipt symlink");
        let receipt = generated_receipt_fixture(&destination, "Summarize safely");

        let error = write_generated_script_receipt(&destination, &receipt)
            .expect_err("receipt publishing must never follow an existing symlink");
        assert!(error.to_string().contains("symbolic-link"));
        assert_eq!(
            fs::read_to_string(&protected).expect("read protected fixture"),
            "protected-content"
        );
    }

    #[test]
    fn generated_prompt_and_verifier_output_redact_secrets_without_losing_safe_copy() {
        let prompt = "Summarize deployment token=sk-private-do-not-save for the team";
        let safe_prompt = safe_generated_script_detail(prompt);
        assert!(safe_prompt.contains("Summarize deployment"));
        assert!(safe_prompt.contains("[REDACTED]"));
        assert!(!safe_prompt.contains("sk-private-do-not-save"));

        let stderr = b"Build failed: api_key=sk-never-persist-this at /Users/alice/private.ts";
        let excerpt = truncate_verification_output(stderr).expect("safe diagnostics remain useful");
        assert!(excerpt.contains("Build failed"));
        assert!(!excerpt.contains("sk-never-persist-this"));
        assert!(!excerpt.contains("/Users/alice"));
    }

    #[test]
    fn generated_author_prompt_distinguishes_interactive_and_legacy_scriptlets() {
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT
            .contains("launcher-opened TypeScript scriptlets have interactive SDK transport"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT
            .contains("Only legacy synchronous TypeScript scriptlets lack interactive stdin"));
    }

    #[test]
    fn test_build_script_generation_messages_wraps_prompt_with_request_delimiters() {
        let messages = build_script_generation_messages("show today's weather");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "user");
        assert!(messages[1]
            .content
            .contains(AI_SCRIPT_USER_REQUEST_START_DELIMITER));
        assert!(messages[1]
            .content
            .contains(AI_SCRIPT_USER_REQUEST_END_DELIMITER));
        assert!(messages[1]
            .content
            .contains("---USER_REQUEST---\nshow today's weather\n---END_REQUEST---"));
    }

    #[test]
    fn test_detect_unexpected_shell_execution_patterns_returns_patterns_when_prompt_disallows_shell(
    ) {
        let prompt = "Show CPU usage in a rich UI";
        let script_source = r#"
import { execSync } from "child_process";
await div(execSync("top -l 1").toString());
"#;

        let patterns = detect_unexpected_shell_execution_patterns(prompt, script_source);
        assert_eq!(patterns, vec!["child_process", "execSync"]);
    }

    #[test]
    fn test_detect_unexpected_shell_execution_patterns_returns_empty_when_prompt_allows_shell() {
        let prompt = "Run a shell command in the terminal and show output";
        let script_source = r#"
import { execSync } from "child_process";
await div(execSync("pwd").toString());
"#;

        let patterns = detect_unexpected_shell_execution_patterns(prompt, script_source);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_extract_script_code_prefers_typescript_fence_when_multiple_blocks_exist() {
        let response = r#"
Here's one idea:
```markdown
Not code
```
```typescript
await div("hello");
```
"#;
        assert_eq!(extract_script_code(response), "await div(\"hello\");");
    }

    #[test]
    fn test_extract_script_code_falls_back_to_first_fenced_block() {
        let response = r#"
```python
print("hello")
```
"#;
        assert_eq!(extract_script_code(response), "print(\"hello\")");
    }

    #[test]
    fn test_extract_script_code_returns_trimmed_response_when_no_fence_exists() {
        let response = "const answer = 42;";
        assert_eq!(extract_script_code(response), "const answer = 42;");
    }

    #[test]
    fn test_enforce_script_kit_conventions_adds_missing_metadata_as_export() {
        let script = "const name = await arg(\"Name?\");";
        let output = enforce_script_kit_conventions(script, "Ask for user name", "ask-user-name");

        assert!(
            output.contains("export const metadata = {"),
            "should inject export const metadata when no metadata exists"
        );
        assert!(
            output.contains("name: \"Ask User Name\""),
            "metadata should contain name"
        );
        assert!(
            output.contains("description: \"Ask for user name\""),
            "metadata should contain description"
        );
        assert!(
            !output.contains("// Name:"),
            "should not inject legacy comment-header metadata"
        );
        assert!(
            !output.contains("// Description:"),
            "should not inject legacy comment-header metadata"
        );
        assert!(
            !output.contains("import \"@scriptkit/sdk\";"),
            "SDK import should be stripped (preload provides globals)"
        );
        assert!(output.contains("await arg(\"Name?\");"));
    }

    #[test]
    fn test_enforce_script_kit_conventions_keeps_existing_metadata_and_import() {
        let script = r#"// Name: Existing
// Description: Existing description
import "@scriptkit/sdk";

await div("ready");
"#;
        let output = enforce_script_kit_conventions(script, "ignored", "ignored");

        assert_eq!(
            output.matches("// Name:").count(),
            1,
            "should not duplicate existing Name metadata"
        );
        assert_eq!(
            output.matches("// Description:").count(),
            1,
            "should not duplicate existing Description metadata"
        );
        assert_eq!(
            output.matches("import \"@scriptkit/sdk\";").count(),
            0,
            "SDK import should be stripped (preload provides globals)"
        );
    }

    #[test]
    fn test_has_kit_import_accepts_scriptkit_sdk_and_rejects_legacy_kit_import() {
        assert!(has_kit_import("import \"@scriptkit/sdk\";"));
        assert!(has_kit_import("import '@scriptkit/sdk'"));
        assert!(!has_kit_import("import \"@johnlindquist/kit\";"));
    }

    #[test]
    fn test_enforce_conventions_strips_legacy_johnlindquist_kit_import() {
        let script = "// Name: Focus Notion\n// Description: Bring Notion to the front\n\nimport \"@johnlindquist/kit\";\n\nawait $`open -a \"Notion\"`;";
        let output = enforce_script_kit_conventions(script, "focus on this app", "focus-notion");
        assert!(
            !output.contains("@johnlindquist/kit"),
            "should strip legacy @johnlindquist/kit import"
        );
        assert!(
            !output.contains("@scriptkit/sdk"),
            "should not add @scriptkit/sdk import (preload provides globals)"
        );
        assert!(
            output.contains("open -a"),
            "should preserve the actual script body"
        );
    }

    #[test]
    fn ai_script_generation_system_prompt_is_not_accidentally_truncated() {
        assert!(
            AI_SCRIPT_GENERATION_SYSTEM_PROMPT.len() > 100,
            "AI_SCRIPT_GENERATION_SYSTEM_PROMPT looks truncated (len={})",
            AI_SCRIPT_GENERATION_SYSTEM_PROMPT.len()
        );
    }

    #[test]
    fn ai_script_generation_system_prompt_keeps_typescript_only_contract() {
        let prompt = AI_SCRIPT_GENERATION_SYSTEM_PROMPT;

        assert!(
            prompt.contains("production-ready Script Kit TypeScript scripts"),
            "system prompt must keep the Script Kit TypeScript framing"
        );
        assert!(
            prompt.contains("ONLY TypeScript code"),
            "system prompt must explicitly forbid extra commentary"
        );
        assert!(
            prompt
                .to_ascii_lowercase()
                .contains("typescript source code"),
            "system prompt must explicitly require TypeScript source output"
        );
        // System prompt must NOT bless legacy comment-header metadata
        assert!(
            !prompt.contains("Format A (comment headers)"),
            "system prompt must not document legacy comment-header format"
        );
        assert!(
            prompt.contains("Do NOT use legacy comment-header metadata"),
            "system prompt must explicitly forbid legacy comment headers"
        );
    }

    #[test]
    fn test_ai_script_generation_system_prompt_uses_modern_sdk_conventions() {
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("import \"@scriptkit/sdk\";"));
        assert!(!AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("@johnlindquist/kit"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("arg("));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("div("));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("editor("));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("notify("));
        // New prompt includes examples and comprehensive API reference
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("TEACH BY EXAMPLE"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("COMPACT API REFERENCE"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("aiStartChat("));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("clipboard"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("home("));
        // All examples must use export const metadata, not comment headers
        assert!(
            AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("export const metadata"),
            "system prompt examples must use export const metadata"
        );
    }

    #[test]
    fn ai_script_generation_reference_preserves_actual_workspace_and_api_contracts() {
        let prompt = AI_SCRIPT_GENERATION_SYSTEM_PROMPT;

        assert!(prompt.contains("skPath(\"plugins\", \"main\", ...)"));
        assert!(!prompt.contains("home(\".scriptkit\", \"kit\", \"main\""));
        assert!(prompt.contains("typed dispatch receipt; OS delivery is not guaranteed"));
        assert!(!prompt.contains("typed delivery result"));
        assert!(prompt.contains("* setActions(actions)"));
        assert!(!prompt.contains("* setActions(actions, options?)"));
    }

    #[test]
    fn test_ai_script_generation_system_prompt_defaults_to_main_menu_sized_flows() {
        let prompt = AI_SCRIPT_GENERATION_SYSTEM_PROMPT;

        assert!(
            prompt.contains("Default to the shared main-menu-sized prompt flow"),
            "system prompt should bias generated commands toward main-menu-sized flows"
        );
        assert!(
            prompt.contains("Expanded split-view browsers are rare exceptions"),
            "system prompt should make expanded split-view browsers opt-in exceptions"
        );
        assert!(
            prompt.contains("Do not use choice `preview` fields, `setPreview()`, or `setPanel()`"),
            "system prompt should steer models away from dense preview APIs by default"
        );
    }

    #[test]
    fn generated_system_message_uses_the_live_host_unsupported_capability_inventory() {
        let messages = build_script_generation_messages("Write a note");
        let system = &messages[0];

        assert_eq!(system.role, "system");
        assert!(system.content.contains("HOST SDK CAPABILITY CONTRACT"));
        for unsupported in crate::mcp_resources::unsupported_sdk_capability_names() {
            assert!(
                system.content.contains(unsupported),
                "system prompt must reject the host-owned unsupported capability {unsupported}"
            );
        }
        assert!(!system.content.contains("* find(...) — file search prompt"));
        assert!(!system
            .content
            .contains("* keyboard.type(...textOrKeys) — type (use with caution)"));
        assert!(!system
            .content
            .contains("* mouse.move(points) — move mouse (use with caution)"));
        assert!(system.content.contains("kit://command-doctor"));
        assert!(system.content.contains("kit://failed-scripts"));
        assert!(system.content.contains("never callable SDK functions"));
        assert!(system.content.contains("do not invent `commandDoctor()`"));
    }

    #[test]
    fn script_generation_examples_use_real_host_globals_and_no_imaginary_helpers() {
        use crate::mcp_resources::{diagnose_sdk_capability, SdkExecutionTopology};

        for capability in [
            "arg",
            "select",
            "fields",
            "editor",
            "div",
            "form",
            "drop",
            "path",
            "mini",
            "micro",
            "hotkey",
            "confirm",
            "md",
            "hud",
            "notify",
            "home",
            "skPath",
            "kitPath",
            "tmpPath",
            "readFile",
            "writeFile",
            "fileSearch",
            "exec",
            "browse",
            "editFile",
            "clipboard.readText",
            "clipboard.writeText",
            "clipboardHistory",
            "aiStartChat",
            "aiSendMessage",
            "mcp.discover",
        ] {
            assert!(
                diagnose_sdk_capability(capability, SdkExecutionTopology::TypeScriptScript)
                    .is_none(),
                "AI script guidance advertises unsupported SDK capability {capability}"
            );
        }

        for imaginary in [
            "* grid(",
            "* textarea(",
            "* onTab(",
            "* toast(",
            "* openActions(",
            "* setHint(",
            "* setLoading(",
            "* setProgress(",
            "* db(",
            "* store(",
            "* ensureDir(",
            "* globby(",
            "* getClipboardHistory(",
            "* removeClipboardItem(",
            "* clearClipboardHistory(",
            "* ai(",
            "* assistant(",
            "* generate(",
            "* mcp(options?)",
            "path.join/etc",
            "exec(command, options?)",
            "`${cmd}",
        ] {
            assert!(
                !AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains(imaginary),
                "AI script guidance advertises imaginary SDK helper {imaginary}"
            );
        }
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("exec(\"open\", [filePath])"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("noResponse: true"));
    }

    #[test]
    fn test_prepare_script_from_ai_response_adds_conventions_when_ai_omits_them() {
        let prompt = "Create a weather checker";
        let response = "await div(\"Sunny\");";

        let (slug, source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert_eq!(slug, "create-a-weather-checker");
        assert!(
            source.contains("export const metadata = {"),
            "should inject export const metadata"
        );
        assert!(
            source.contains("name: \"Create A Weather Checker\""),
            "metadata should contain name"
        );
        assert!(
            source.contains("description: \"Create a weather checker\""),
            "metadata should contain description"
        );
        assert!(
            !source.contains("// Name:"),
            "should not use legacy comment-header metadata"
        );
        assert!(
            !source.contains("import \"@scriptkit/sdk\";"),
            "SDK import should be stripped (preload provides globals)"
        );
        assert!(source.contains("await div(\"Sunny\");"));
    }

    #[test]
    fn test_enforce_script_kit_conventions_escapes_metadata_string_literals() {
        let script = "await div(\"ok\");";
        let output = enforce_script_kit_conventions(
            script,
            "Say \"hello\" from C:\\Temp\nnow",
            "say-\"hello\"-tool",
        );

        assert!(
            output.contains("name: \"Say \\\"hello\\\" Tool\""),
            "metadata name should escape double quotes"
        );
        assert!(
            output.contains("description: \"Say \\\"hello\\\" from C:\\\\Temp now\""),
            "metadata description should escape quotes and backslashes"
        );
    }

    #[test]
    fn test_prepare_script_from_ai_response_uses_name_comment_for_slug() {
        let prompt =
            "Generate a Script Kit script that automates what I am doing in the current app";
        let response = r#"// Name: cmux Quick Actions
// Description: Quick action palette for cmux
import "@scriptkit/sdk";

await arg("Pick an action");
"#;
        let (slug, source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert_eq!(
            slug, "cmux-quick-actions",
            "slug should come from // Name:, not the prompt"
        );
        assert!(source.contains("// Name: cmux Quick Actions"));
    }

    #[test]
    fn test_extract_name_comment_finds_name_line() {
        assert_eq!(
            extract_name_comment("// Name: My Cool Script\nimport \"@scriptkit/sdk\";"),
            Some("My Cool Script".to_string())
        );
        assert_eq!(
            extract_name_comment("import \"@scriptkit/sdk\";\nawait arg(\"hi\");"),
            None
        );
        assert_eq!(
            extract_name_comment("// Name: \nimport \"@scriptkit/sdk\";"),
            None,
            "empty name should return None"
        );
    }

    #[test]
    fn test_prepare_script_from_ai_response_extracts_typescript_fence_when_present() {
        let prompt = "Build script";
        let response = r#"
```typescript
await arg("Name?");
```
"#;

        let (_slug, source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert!(source.contains("await arg(\"Name?\");"));
        assert!(!source.contains("```"));
    }

    #[test]
    fn test_strip_leading_prose_removes_markdown_preamble() {
        let response = r#"**Assumed:** You're using cmux and want a quick-action palette.

**Required permissions:** Accessibility access for Script Kit.

// Name: cmux Quick Actions
// Description: Quick action palette for cmux
import "@scriptkit/sdk";

await arg("Pick an action");
"#;
        let stripped = strip_leading_prose(response.trim());
        assert!(
            stripped.starts_with("// Name:"),
            "Should start with // Name:, got: {}",
            &stripped[..stripped.len().min(50)]
        );
        assert!(!stripped.contains("**Assumed:**"));
        assert!(stripped.contains("await arg"));
    }

    #[test]
    fn test_strip_leading_prose_preserves_clean_response() {
        let response = r#"// Name: My Script
// Description: Does something
import "@scriptkit/sdk";

await div("hello");
"#;
        let stripped = strip_leading_prose(response.trim());
        assert!(stripped.starts_with("// Name:"));
        assert!(stripped.contains("await div"));
    }

    #[test]
    fn test_extract_script_code_strips_prose_when_no_fence() {
        let response = r#"Here's a script for you:

// Name: Test
import "@scriptkit/sdk";

await arg("hello");
"#;
        let extracted = extract_script_code(response);
        assert!(
            extracted.starts_with("// Name:"),
            "Should start with code, got: {}",
            &extracted[..extracted.len().min(50)]
        );
        assert!(!extracted.contains("Here's a script"));
    }

    #[test]
    fn test_extract_metadata_name_finds_double_quoted_name() {
        let source = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "My Cool Script",
  description: "Does cool things",
};
await arg("hello");
"#;
        assert_eq!(
            extract_metadata_name(source),
            Some("My Cool Script".to_string())
        );
    }

    #[test]
    fn test_extract_metadata_name_finds_single_quoted_name() {
        let source = r#"import "@scriptkit/sdk";
export const metadata = {
  name: 'Single Quoted',
  description: 'desc',
};
"#;
        assert_eq!(
            extract_metadata_name(source),
            Some("Single Quoted".to_string())
        );
    }

    #[test]
    fn test_extract_metadata_name_returns_none_when_no_metadata() {
        let source = r#"import "@scriptkit/sdk";
await arg("hello");
"#;
        assert_eq!(extract_metadata_name(source), None);
    }

    #[test]
    fn test_extract_metadata_name_returns_none_for_empty_name() {
        let source = r#"export const metadata = {
  name: "",
  description: "desc",
};
"#;
        assert_eq!(
            extract_metadata_name(source),
            None,
            "empty name should return None"
        );
    }

    #[test]
    fn test_extract_metadata_name_ignores_name_outside_metadata_block() {
        let source = r#"const config = { name: "Not This" };
export const metadata = {
  name: "Correct Name",
};
"#;
        assert_eq!(
            extract_metadata_name(source),
            Some("Correct Name".to_string()),
            "should find name only after export const metadata"
        );
    }

    #[test]
    fn test_extract_metadata_name_finds_inline_metadata_export_name() {
        let source = r#"import "@scriptkit/sdk";
export const metadata = { name: "Inline Name", description: "desc" };
await arg("hello");
"#;
        assert_eq!(
            extract_metadata_name(source),
            Some("Inline Name".to_string())
        );
    }

    #[test]
    fn test_extract_metadata_name_does_not_leak_past_metadata_block() {
        let source = r#"export const metadata = {
  description: "desc",
};
const other = {
  name: "Wrong Name",
};
"#;
        assert_eq!(
            extract_metadata_name(source),
            None,
            "name outside metadata block should not be used"
        );
    }

    #[test]
    fn test_prepare_script_from_ai_response_uses_metadata_name_for_slug() {
        let prompt = "Generate a Script Kit script for the current app";
        let response = r#"import "@scriptkit/sdk";

export const metadata = {
  name: "App Automator",
  description: "Automates the current app",
};

await arg("Pick an action");
"#;
        let (slug, _source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert_eq!(
            slug, "app-automator",
            "slug should come from metadata export name, not the prompt"
        );
    }

    #[test]
    fn test_prepare_script_comment_header_takes_priority_over_metadata_export_for_slug() {
        // When both comment headers and metadata export are present (hybrid),
        // the comment header name still wins for slug resolution (backwards compat).
        let prompt = "do something";
        let response = r#"// Name: Comment Winner
// Description: from comment
import "@scriptkit/sdk";

export const metadata = {
  name: "Metadata Loser",
  description: "from metadata",
};

await arg("hi");
"#;
        let (slug, _source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert_eq!(
            slug, "comment-winner",
            "comment header should take priority over metadata export for slug"
        );
    }

    // --- Contract-aware finalization tests ---

    #[test]
    fn metadata_export_prevents_comment_header_injection() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Save Selection",
  description: "Save the current selection",
};

await div("ok");
"#;

        let output = enforce_script_kit_conventions(input, "save selection", "save-selection");

        assert!(
            !output.contains("// Name: Save Selection"),
            "should not inject // Name: when metadata export has name"
        );
        assert!(
            !output.contains("// Description: Save the current selection"),
            "should not inject // Description: when metadata export has description"
        );
        assert!(output.contains("export const metadata = {"));
    }

    #[test]
    fn metadata_name_is_used_for_slug_source() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "My AI Tool",
  description: "Do something useful",
};

await div("ok");
"#;

        let (slug, _) = prepare_script_from_ai_response("fallback prompt", input).unwrap();
        assert_eq!(slug, "my-ai-tool");
    }

    #[test]
    fn incomplete_metadata_export_only_injects_missing_fields() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Only Name Present",
};

await div("ok");
"#;

        let output =
            enforce_script_kit_conventions(input, "fallback description", "only-name-present");

        assert!(
            !output.contains("// Name: Only Name Present"),
            "should not inject // Name: when metadata export has name"
        );
        assert!(
            output.contains("// Description: fallback description"),
            "should inject // Description: when metadata export lacks description"
        );
        assert!(matches!(
            audit_generated_script_contract(&output).metadata_style,
            GeneratedScriptMetadataStyle::Hybrid
        ));
    }

    #[test]
    fn current_app_recipe_headers_are_stripped_during_enforcement() {
        let input = r#"// Current-App-Recipe-Base64: abc123
// Current-App-Recipe-Name: Safari Save Selection

export const metadata = {
  name: "Safari Save Selection",
  description: "Save the current Safari selection",
};

await div("ok");
"#;

        let output = enforce_script_kit_conventions(
            input,
            "save the current Safari selection",
            "safari-save-selection",
        );

        assert!(!output.contains("Current-App-Recipe-"));
        let contract = audit_generated_script_contract(&output);
        assert!(!contract.has_current_app_recipe_header);
        assert!(contract.current_app_recipe_header_at_top);
    }

    #[test]
    fn audit_detects_missing_metadata() {
        let input = r#"import "@scriptkit/sdk";
await div("hello");
"#;
        let contract = audit_generated_script_contract(input);
        assert!(matches!(
            contract.metadata_style,
            GeneratedScriptMetadataStyle::Missing
        ));
        assert!(!contract.has_name);
        assert!(!contract.has_description);
        assert!(contract
            .warnings
            .contains(&"missing_name_contract".to_string()));
        assert!(contract
            .warnings
            .contains(&"missing_description_contract".to_string()));
    }

    #[test]
    fn audit_detects_hybrid_metadata() {
        let input = r#"// Name: Comment Name
import "@scriptkit/sdk";
export const metadata = {
  name: "Metadata Name",
  description: "desc",
};
"#;
        let contract = audit_generated_script_contract(input);
        assert!(matches!(
            contract.metadata_style,
            GeneratedScriptMetadataStyle::Hybrid
        ));
        assert!(contract
            .warnings
            .contains(&"mixed_metadata_formats".to_string()));
    }

    #[test]
    fn audit_detects_concurrent_prompt_apis() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Open 3 Windows",
  description: "Open 3 new browser windows to different URLs",
};

const urls = await Promise.all([
  arg("URL 1"),
  arg("URL 2"),
  arg("URL 3"),
]);

await div(JSON.stringify(urls));
"#;

        let contract = audit_generated_script_contract(input);
        assert!(contract
            .warnings
            .contains(&"concurrent_prompt_apis".to_string()));
    }

    #[test]
    fn audit_allows_sequential_prompt_apis() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Open 3 Windows",
  description: "Open 3 new browser windows to different URLs",
};

const url1 = await arg("URL 1");
const url2 = await arg("URL 2");
const url3 = await arg("URL 3");

await div(JSON.stringify([url1, url2, url3]));
"#;

        let contract = audit_generated_script_contract(input);
        assert!(!contract
            .warnings
            .contains(&"concurrent_prompt_apis".to_string()));
    }

    #[test]
    fn prepare_script_rejects_concurrent_prompt_apis() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Open 3 Windows",
  description: "Open 3 new browser windows to different URLs",
};

const urls = await Promise.all([
  arg("URL 1"),
  arg("URL 2"),
  arg("URL 3"),
]);

await div(JSON.stringify(urls));
"#;

        let error = prepare_script_from_ai_response_with_contract("open three windows", input)
            .expect_err("concurrent prompt APIs must be rejected");

        let message = format!("{error:#}");
        assert!(message.contains("concurrent_prompt_apis"));
    }

    #[test]
    fn generated_script_rejects_unsupported_capabilities_before_any_file_creation() {
        let input = r#"export const metadata = {
  name: "Unavailable Widget",
  description: "An unsupported native widget",
  sdkCapabilities: ["widget"],
  executionTopology: "typescript-script",
};
await widget("<p>Hello</p>");
"#;

        let error = save_generated_script_from_response("render a widget", input)
            .expect_err("unsupported generated commands must fail before create_new_script");
        let message = format!("{error:#}");
        assert!(message.contains("generated_script_capability_unavailable"));
        assert!(message.contains("widget"));
    }

    #[test]
    fn generated_script_rejects_malformed_capability_and_topology_declarations() {
        let malformed_capabilities = r#"export const metadata = {
  name: "Bad Capability Shape",
  description: "Invalid metadata",
  sdkCapabilities: "arg",
};
await arg("Name?");
"#;
        let capability_error =
            prepare_script_from_ai_response_with_contract("ask for a name", malformed_capabilities)
                .expect_err("a bare capability string is never a valid author declaration");
        assert!(format!("{capability_error:#}").contains("must be an array"));

        let malformed_topology = r#"export const metadata = {
  name: "Bad Topology",
  description: "Invalid transport",
  executionTopology: "ruby-scriptlet",
};
await arg("Name?");
"#;
        let topology_error =
            prepare_script_from_ai_response_with_contract("ask for a name", malformed_topology)
                .expect_err("topology must be checked even without sdkCapabilities");
        assert!(format!("{topology_error:#}").contains("executionTopology"));
    }

    #[test]
    fn generated_script_rejects_unparseable_typed_metadata_before_creation() {
        let malformed = r#"export const metadata = {
  name: "Broken Metadata",
  description: "Missing closing brace",
  sdkCapabilities: ["arg"];
await arg("Name?");
"#;
        let error = prepare_script_from_ai_response_with_contract("ask a question", malformed)
            .expect_err("invalid typed metadata must not produce a hidden generated script");
        assert!(format!("{error:#}").contains("generated_script_metadata_invalid"));
    }

    #[test]
    fn generated_script_audit_reports_supported_capabilities_and_topology() {
        let valid = r#"export const metadata = {
  name: "Supported Author Flow",
  description: "Prompt and format a response",
  sdkCapabilities: ["arg", "div", "md"],
  executionTopology: "typescript-script",
};
const answer = await arg("Question?");
await div(md(answer));
"#;

        let prepared = prepare_script_from_ai_response_with_contract("ask a question", valid)
            .expect("supported author declarations should remain saveable");
        assert_eq!(
            prepared.contract.declared_capabilities,
            ["arg", "div", "md"]
        );
        assert_eq!(
            prepared.contract.execution_topology,
            Some(crate::mcp_resources::SdkExecutionTopology::TypeScriptScript)
        );
        assert!(prepared.contract.metadata_parse_errors.is_empty());
        assert!(prepared.contract.capability_issues.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn generated_permission_pending_script_stays_recoverable_without_privacy_probe() {
        let input = r#"export const metadata = {
  name: "Move Existing Window",
  description: "Needs explicitly granted Accessibility",
  sdkCapabilities: ["moveWindow"],
  executionTopology: "typescript-script",
};
await moveWindow("example", { x: 0, y: 0 });
"#;

        let prepared = prepare_script_from_ai_response_with_contract("move a window", input)
            .expect("unknown permission inventory is a recoverable author warning");
        assert_eq!(prepared.contract.capability_issues.len(), 1);
        assert_eq!(
            prepared.contract.capability_issues[0].severity,
            crate::scripts::ValidationSeverity::Warning
        );
    }

    #[test]
    fn receipt_serde_roundtrip() {
        let receipt = GeneratedScriptReceipt {
            schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
            prompt: "close duplicate tabs".to_string(),
            slug: "close-duplicate-tabs".to_string(),
            slug_source: "Close Duplicate Tabs".to_string(),
            slug_source_kind: "metadata_export".to_string(),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            script_path: "/tmp/test.ts".to_string(),
            receipt_path: "/tmp/test.scriptkit.json".to_string(),
            shell_execution_warning: false,
            contract: GeneratedScriptContractAudit {
                metadata_style: GeneratedScriptMetadataStyle::MetadataExport,
                has_name: true,
                has_description: true,
                has_kit_import: true,
                has_current_app_recipe_header: false,
                current_app_recipe_header_at_top: true,
                declared_capabilities: vec![],
                execution_topology: None,
                metadata_parse_errors: vec![],
                capability_issues: vec![],
                warnings: vec![],
            },
            verification: GeneratedScriptVerificationReceipt::skipped("unit_test_fixture"),
            current_app_recipe: None,
        };

        let json = serde_json::to_string_pretty(&receipt).expect("serialize receipt");
        let deserialized: GeneratedScriptReceipt =
            serde_json::from_str(&json).expect("deserialize receipt");
        assert_eq!(receipt, deserialized);
        assert!(json.contains("\"schemaVersion\": 2"));
        assert!(json.contains("\"metadataStyle\": \"metadataExport\""));
        assert!(json.contains("\"verification\""));
        assert!(json.contains("\"status\": \"skipped\""));
        assert!(!json.contains("\"currentAppRecipe\""));
    }

    #[test]
    fn receipt_serde_defaults_missing_verification_to_skipped() {
        let json = r#"{
  "schemaVersion": 1,
  "prompt": "close duplicate tabs",
  "slug": "close-duplicate-tabs",
  "slugSource": "Close Duplicate Tabs",
  "slugSourceKind": "metadata_export",
  "modelId": "gpt-4",
  "providerId": "openai",
  "scriptPath": "/tmp/test.ts",
  "receiptPath": "/tmp/test.scriptkit.json",
  "shellExecutionWarning": false,
  "contract": {
    "metadataStyle": "metadataExport",
    "hasName": true,
    "hasDescription": true,
    "hasKitImport": true,
    "hasCurrentAppRecipeHeader": false,
    "currentAppRecipeHeaderAtTop": true
  }
}"#;

        let receipt: GeneratedScriptReceipt =
            serde_json::from_str(json).expect("deserialize legacy receipt");
        assert_eq!(
            receipt.verification.status,
            GeneratedScriptVerificationStatus::Skipped
        );
        assert!(receipt
            .verification
            .diagnostics
            .contains(&"legacy_receipt_missing_verification".to_string()));
    }

    #[test]
    fn recipe_header_with_metadata_export_gets_correct_slug_source() {
        let input = r#"// Current-App-Recipe-Base64: abc123
// Current-App-Recipe-Name: Safari Close Duplicate Tabs

export const metadata = {
  name: "Safari Close Duplicate Tabs",
  description: "Close duplicate tabs in the current Safari window",
};

await div("Ready");
"#;

        let prepared =
            prepare_script_from_ai_response_with_contract("close duplicate tabs in Safari", input)
                .unwrap();
        assert_eq!(prepared.slug_source_kind, "metadata_export");
        assert_eq!(prepared.slug, "safari-close-duplicate-tabs");
        assert!(!prepared.source.contains("Current-App-Recipe-"));
        assert!(!prepared.contract.has_current_app_recipe_header);
    }

    #[test]
    fn extract_metadata_description_finds_value() {
        let source = r#"export const metadata = {
  name: "Test",
  description: "A useful test script",
};
"#;
        assert_eq!(
            extract_metadata_description(source),
            Some("A useful test script".to_string())
        );
    }

    #[test]
    fn generated_script_receipt_path_replaces_extension() {
        let script_path = PathBuf::from("/tmp/my-script.ts");
        let receipt = generated_script_receipt_path(&script_path);
        assert_eq!(receipt, PathBuf::from("/tmp/my-script.scriptkit.json"));
    }

    #[test]
    fn generated_script_receipt_omits_current_app_recipe_payloads() {
        let script_source = format!(
            r#"export const metadata = {{
  name: "Safari Close Duplicate Tabs",
  description: "Close duplicate tabs in the current Safari window",
}};

import "@scriptkit/sdk";
await div("Ready");
"#
        );

        let prepared = prepare_script_from_ai_response_with_contract(
            "close duplicate tabs in Safari",
            &script_source,
        )
        .expect("should prepare script");

        let receipt = GeneratedScriptReceipt {
            schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
            prompt: "close duplicate tabs in Safari".to_string(),
            slug: prepared.slug.clone(),
            slug_source: prepared.slug_source.clone(),
            slug_source_kind: prepared.slug_source_kind.to_string(),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            script_path: "/tmp/test.ts".to_string(),
            receipt_path: "/tmp/test.scriptkit.json".to_string(),
            shell_execution_warning: false,
            contract: prepared.contract.clone(),
            verification: GeneratedScriptVerificationReceipt::skipped("unit_test_fixture"),
            current_app_recipe: None,
        };

        let json = serde_json::to_string_pretty(&receipt).expect("serialize receipt");
        let deserialized: GeneratedScriptReceipt =
            serde_json::from_str(&json).expect("deserialize receipt");

        assert_eq!(receipt, deserialized);
        assert!(!json.contains("\"currentAppRecipe\""));
    }

    #[test]
    fn generated_script_receipt_strips_invalid_current_app_recipe_header() {
        let script_with_bad_base64 = r#"// Current-App-Recipe-Base64: not-valid-base64!!!
// Current-App-Recipe-Name: Bad Recipe

export const metadata = {
  name: "Bad Recipe Test",
  description: "A script with invalid recipe header",
};

import "@scriptkit/sdk";
await div("ok");
"#;

        let extracted = extract_current_app_recipe_from_script(script_with_bad_base64);
        assert!(
            extracted.is_none(),
            "invalid base64 should return None, not error"
        );

        let prepared = prepare_script_from_ai_response_with_contract(
            "test with bad recipe",
            script_with_bad_base64,
        )
        .expect("should prepare script despite invalid recipe header");

        assert!(!prepared.source.contains("Current-App-Recipe-"));
        assert!(!prepared.contract.has_current_app_recipe_header);

        let receipt = GeneratedScriptReceipt {
            schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
            prompt: "test with bad recipe".to_string(),
            slug: prepared.slug.clone(),
            slug_source: prepared.slug_source.clone(),
            slug_source_kind: prepared.slug_source_kind.to_string(),
            model_id: "unknown".to_string(),
            provider_id: "unknown".to_string(),
            script_path: "/tmp/test.ts".to_string(),
            receipt_path: "/tmp/test.scriptkit.json".to_string(),
            shell_execution_warning: false,
            contract: prepared.contract.clone(),
            verification: GeneratedScriptVerificationReceipt::skipped("unit_test_fixture"),
            current_app_recipe: None,
        };

        let json = serde_json::to_string_pretty(&receipt).expect("serialize receipt");
        let deserialized: GeneratedScriptReceipt =
            serde_json::from_str(&json).expect("deserialize receipt");
        assert_eq!(receipt, deserialized);
        assert!(!json.contains("\"currentAppRecipe\""));
    }

    #[test]
    fn extract_current_app_recipe_returns_none_for_no_header() {
        let script = r#"import "@scriptkit/sdk";
// Name: Simple Script
// Description: No recipe here
await div("hello");
"#;
        assert!(extract_current_app_recipe_from_script(script).is_none());
    }

    #[test]
    fn extract_current_app_recipe_returns_none_for_empty_base64() {
        let script = r#"// Current-App-Recipe-Base64:
import "@scriptkit/sdk";
await div("hello");
"#;
        assert!(extract_current_app_recipe_from_script(script).is_none());
    }

    #[test]
    fn chaos_nn29_hostile_generated_artifact_stays_in_scratch_and_is_never_executed() {
        let temp = tempdir().expect("create NN29 scratch workspace");
        let kit_path = temp.path().join(".scriptkit");
        std::fs::create_dir_all(&kit_path).expect("create scratch SK_PATH");
        let original_sk_path = std::env::var_os("SK_PATH");
        // SAFETY: this focused test runs alone in the NN29 probe invocation and
        // restores the process environment before returning.
        unsafe { std::env::set_var("SK_PATH", &kit_path) };

        let execution_canary = temp.path().join("generated-script-executed");
        let canary_literal = serde_json::to_string(&execution_canary.display().to_string())
            .expect("encode canary path as a TypeScript string");
        let raw_response = format!(
            r#"```typescript
export const metadata = {{
  name: "../../../../tmp/NN29 👩🏽‍💻 مرحبا",
  description: "Hostile path and Unicode must remain inert",
}};

await Bun.write({canary_literal}, "NN29_EXECUTED");
await div("NN29_INERT_CANARY Z̴̙̓͗a̷̻͒l̵͎͋g̶̯͗o̴̰̕");
```"#
        );

        let script_path = save_generated_script_from_response(
            "Create an inert NN29 script with hostile metadata",
            &raw_response,
        )
        .expect("save generated script through the real contract pipeline");
        let scripts_dir = kit_path.join("plugins/main/scripts");
        let canonical_script = script_path
            .canonicalize()
            .expect("canonicalize generated script");
        let canonical_scripts_dir = scripts_dir
            .canonicalize()
            .expect("canonicalize scratch scripts directory");
        assert!(
            canonical_script.starts_with(&canonical_scripts_dir),
            "generated path escaped scratch scripts dir: {}",
            canonical_script.display()
        );
        assert_eq!(
            script_path.extension().and_then(|value| value.to_str()),
            Some("ts")
        );
        assert!(!script_path.to_string_lossy().contains(".."));

        let source = std::fs::read_to_string(&script_path).expect("read generated artifact");
        assert!(source.contains("NN29_INERT_CANARY"));
        assert!(source.contains("Hostile path and Unicode must remain inert"));
        assert!(
            !execution_canary.exists(),
            "generated script ran during save"
        );

        let receipt_path = generated_script_receipt_path(&script_path);
        let mut final_verification = None;
        for _ in 0..200 {
            if let Ok(body) = std::fs::read_to_string(&receipt_path) {
                if let Ok(receipt) = serde_json::from_str::<GeneratedScriptReceipt>(&body) {
                    if receipt.verification.status != GeneratedScriptVerificationStatus::Skipped {
                        final_verification = Some(receipt.verification.status);
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(
            final_verification.is_some(),
            "bounded Bun-build verification never finalized"
        );
        assert!(
            !execution_canary.exists(),
            "generated script executed instead of being build-verified only"
        );

        match original_sk_path {
            Some(value) => unsafe { std::env::set_var("SK_PATH", value) },
            None => unsafe { std::env::remove_var("SK_PATH") },
        }
    }

    #[test]
    fn chaos_nn29_unavailable_generation_fails_before_artifact_work_with_context() {
        let registry = ProviderRegistry::new();
        let error = match select_generation_model(&registry) {
            Ok(_) => panic!("an empty provider registry must reject generation"),
            Err(error) => error,
        };
        assert_eq!(
            error.to_string(),
            "No AI models available in provider registry"
        );
    }
}
