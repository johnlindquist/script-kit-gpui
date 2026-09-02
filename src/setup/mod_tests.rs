#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Run a test body while holding the shared SK_PATH lock.
    /// Automatically sets SK_PATH to `kit_root` and restores it on exit.
    pub(super) fn with_sk_path<F: FnOnce(&std::path::Path)>(f: F) {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let temp_dir = TempDir::new().unwrap();
        let kit_root = temp_dir.path().to_path_buf();
        struct RestoreSkPath(Option<std::ffi::OsString>);
        impl Drop for RestoreSkPath {
            fn drop(&mut self) {
                if let Some(previous) = &self.0 {
                    std::env::set_var(SK_PATH_ENV, previous);
                } else {
                    std::env::remove_var(SK_PATH_ENV);
                }
            }
        }
        let restore = RestoreSkPath(std::env::var_os(SK_PATH_ENV));
        std::env::set_var(SK_PATH_ENV, kit_root.to_str().unwrap());

        f(&kit_root);

        drop(restore);
        drop(lock);
    }

    /// Test that plugin directories live under plugins/
    /// Expected structure: ~/.scriptkit/plugins/main/scripts, ~/.scriptkit/plugins/main/scriptlets
    #[test]
    fn test_plugin_directory_uses_plugins_subdirectory() {
        with_sk_path(|kit_root| {
            let result = ensure_kit_setup();

            let kit_main_scripts = kit_root.join("plugins").join("main").join("scripts");
            let kit_main_extensions = kit_root.join("plugins").join("main").join("scriptlets");

            assert!(
                kit_main_scripts.exists(),
                "Expected plugins/main/scripts to exist at {:?}",
                kit_main_scripts
            );
            assert!(
                kit_main_extensions.exists(),
                "Expected plugins/main/scriptlets to exist at {:?}",
                kit_main_extensions
            );

            let old_main_scripts = kit_root.join("main").join("scripts");
            assert!(
                !old_main_scripts.exists(),
                "Old structure main/scripts should NOT exist at {:?}",
                old_main_scripts
            );

            assert!(!result.warnings.iter().any(|w| w.contains("Failed")));
        });
    }

    /// Test that sample files are created in plugins/main/scripts
    #[test]
    fn test_sample_files_in_plugins_subdirectory() {
        with_sk_path(|kit_root| {
            let result = ensure_kit_setup();

            if result.is_fresh_install {
                let hello_script = kit_root
                    .join("plugins")
                    .join("main")
                    .join("scripts")
                    .join("hello-world.ts");
                assert!(
                    hello_script.exists(),
                    "Expected hello-world.ts at {:?}",
                    hello_script
                );
            }
        });
    }

    #[test]
    fn test_fresh_install_seeds_canonical_menu_syntax_handlers() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp_dir = TempDir::new().unwrap();
        let kit_root = temp_dir.path().join("scriptkit-fresh-menu-syntax");
        std::env::set_var(SK_PATH_ENV, kit_root.to_str().unwrap());

        let result = ensure_kit_setup();
        assert!(result.is_fresh_install);

        let main_scripts = kit_root.join("plugins").join("main").join("scripts");
        for filename in [
            "capture-todo-inbox.ts",
            "create-calendar-event.ts",
            "create-mac-calendar-event.ts",
            "add-google-calendar-event.ts",
            "create-reminder.ts",
            "snooze-task.ts",
            "defer-task.ts",
            "append-daily-note.ts",
            "draft-social-post.ts",
            "save-tagged-link.ts",
        ] {
            let path = main_scripts.join(filename);
            assert!(path.exists(), "expected seeded handler at {:?}", path);
            let content = fs::read_to_string(&path).unwrap();
            assert!(
                content.contains("menuSyntax"),
                "{filename} should declare menuSyntax metadata"
            );
            assert!(
                content.contains(r#"family: "capture.v1""#),
                "{filename} should declare a capture.v1 handler"
            );
        }

        let hello_path = main_scripts.join("hello-world.ts");
        let hello_source = fs::read_to_string(&hello_path)
            .expect("the actual fresh-install Hello World script must be readable");
        let parsed = crate::metadata_parser::extract_typed_metadata(&hello_source);
        assert!(
            parsed.errors.is_empty(),
            "the seeded starter must have parseable host compatibility metadata: {:?}",
            parsed.errors
        );
        let metadata = parsed
            .metadata
            .expect("the seeded starter must expose typed metadata");
        assert_eq!(
            metadata.extra.get("sdkCapabilities"),
            Some(&serde_json::json!(["arg", "div"]))
        );
        assert_eq!(
            metadata.extra.get("executionTopology"),
            Some(&serde_json::json!("typescript-script"))
        );
        let hello_script = crate::scripts::Script {
            name: "Hello World".to_owned(),
            path: hello_path,
            extension: "ts".to_owned(),
            plugin_id: "main".to_owned(),
            typed_metadata: Some(metadata),
            ..crate::scripts::Script::default()
        };
        assert!(
            crate::scripts::validate_declared_sdk_capabilities(&hello_script).is_empty(),
            "the first-run starter must satisfy its actual interactive SDK contract"
        );

        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    #[test]
    fn test_bun_is_discoverable() {
        // This test just verifies the function doesn't panic
        let _ = bun_is_discoverable();
    }

    #[test]
    fn test_bun_exe_name() {
        let name = bun_exe_name();
        #[cfg(windows)]
        assert_eq!(name, "bun.exe");
        #[cfg(not(windows))]
        assert_eq!(name, "bun");
    }

    #[test]
    fn test_get_kit_path_default() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(SK_PATH_ENV);
        let path = get_kit_path();
        assert!(path.to_string_lossy().contains(".scriptkit"));
        drop(lock);
    }

    #[test]
    fn test_get_kit_path_with_override() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::env::set_var(SK_PATH_ENV, "/custom/path");
        let path = get_kit_path();
        assert_eq!(path, PathBuf::from("/custom/path"));
        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    #[test]
    fn test_get_kit_path_with_tilde() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::env::set_var(SK_PATH_ENV, "~/.config/kit");
        let path = get_kit_path();
        assert!(!path.to_string_lossy().contains("~"));
        assert!(path.to_string_lossy().contains(".config/kit"));
        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    #[test]
    fn test_get_kit_path_with_env_var_expansion() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let env_var = "SCRIPT_KIT_TEST_SK_PATH_ROOT";
        std::env::set_var(env_var, "/tmp/script-kit-env-root");
        std::env::set_var(SK_PATH_ENV, format!("${env_var}/kit"));

        let path = get_kit_path();
        assert_eq!(path, PathBuf::from("/tmp/script-kit-env-root/kit"));

        std::env::remove_var(SK_PATH_ENV);
        std::env::remove_var(env_var);
        drop(lock);
    }

    /// Comprehensive setup verification test
    /// Verifies the complete directory structure matches documentation:
    /// ```
    /// ~/.scriptkit/
    /// ├── kit/
    /// │   ├── main/
    /// │   │   ├── scripts/
    /// │   │   ├── scriptlets/
    /// │   │   └── agents/
    /// │   ├── config.ts
    /// │   ├── theme.json
    /// │   ├── package.json
    /// │   ├── tsconfig.json
    /// │   ├── AGENTS.md
    /// │   └── CLAUDE.md
    /// ├── sdk/
    /// │   └── kit-sdk.ts
    /// ├── db/
    /// ├── logs/
    /// ├── cache/
    /// └── GUIDE.md
    /// ```
    #[test]
    fn test_complete_setup_structure() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp_dir = TempDir::new().unwrap();
        // Use a subdirectory that definitely doesn't exist for fresh install detection
        let kit_root = temp_dir.path().join("scriptkit-test");

        std::env::set_var(SK_PATH_ENV, kit_root.to_str().unwrap());

        let result = ensure_kit_setup();
        // Don't assert is_fresh_install - just verify the structure is correct
        assert!(
            result.warnings.is_empty() || !result.warnings.iter().any(|w| w.contains("Failed"))
        );

        // Verify plugins/ subdirectory structure
        let plugins_dir = kit_root.join("plugins");
        assert!(plugins_dir.exists(), "plugins/ directory should exist");

        // Verify main kit directories
        let main_dir = plugins_dir.join("main");
        assert!(
            main_dir.join("scripts").exists(),
            "plugins/main/scripts/ should exist"
        );
        assert!(
            main_dir.join("scriptlets").exists(),
            "plugins/main/scriptlets/ should exist"
        );
        assert!(
            main_dir.join("agents").exists(),
            "plugins/main/agents/ should exist"
        );

        // Verify user config files at workspace root
        assert!(
            kit_root.join("config.ts").exists(),
            "config.ts should exist"
        );
        assert!(
            kit_root.join("theme.json").exists(),
            "theme.json should exist"
        );
        assert!(
            kit_root.join("package.json").exists(),
            "package.json should exist"
        );
        assert!(
            kit_root.join("tsconfig.json").exists(),
            "tsconfig.json should exist"
        );
        assert!(
            kit_root.join("AGENTS.md").exists(),
            "AGENTS.md should exist"
        );
        assert!(
            kit_root.join("CLAUDE.md").exists(),
            "CLAUDE.md should exist"
        );

        // Verify SDK directory
        assert!(
            kit_root.join("sdk").join("kit-sdk.ts").exists(),
            "sdk/kit-sdk.ts should exist"
        );
        assert!(
            kit_root.join("bin").join("scriptkit").exists(),
            "bin/scriptkit should exist"
        );

        // Verify other directories
        assert!(kit_root.join("db").exists(), "db/ directory should exist");
        assert!(
            kit_root.join("logs").exists(),
            "logs/ directory should exist"
        );
        assert!(
            kit_root.join("cache").exists(),
            "cache/ directory should exist"
        );

        // Verify GUIDE.md at root
        assert!(
            kit_root.join("GUIDE.md").exists(),
            "GUIDE.md should exist at root"
        );

        // Verify sample script on fresh install
        let hello_script = main_dir.join("scripts").join("hello-world.ts");
        assert!(
            hello_script.exists(),
            "hello-world.ts sample script should exist"
        );

        // Verify config.ts content
        let config_content = fs::read_to_string(kit_root.join("config.ts")).unwrap();
        assert!(
            config_content.contains("@scriptkit/sdk"),
            "config.ts should import @scriptkit/sdk"
        );
        assert!(
            config_content.contains("hotkey"),
            "config.ts should have hotkey config"
        );

        // Verify package.json has correct name and type
        let package_content = fs::read_to_string(kit_root.join("package.json")).unwrap();
        assert!(
            package_content.contains("@scriptkit/kit"),
            "package.json should have @scriptkit/kit name"
        );
        assert!(
            package_content.contains("\"type\": \"module\""),
            "package.json should enable ESM"
        );

        // Root-level AGENTS.md has the actual SDK reference
        let agents_content = fs::read_to_string(kit_root.join("AGENTS.md")).unwrap();
        assert!(
            agents_content.contains("Script Kit"),
            "Root AGENTS.md should mention Script Kit"
        );
        assert!(
            agents_content.contains("~/.scriptkit/config.ts"),
            "Root AGENTS.md should have correct config path"
        );

        // Root-level CLAUDE.md has the actual agent instructions
        let claude_content = fs::read_to_string(kit_root.join("CLAUDE.md")).unwrap();
        assert!(
            claude_content.contains("Script Kit"),
            "Root CLAUDE.md should mention Script Kit"
        );
        assert!(
            claude_content.contains("@scriptkit/sdk"),
            "Root CLAUDE.md should reference the SDK"
        );

        // Verify CleanShot X built-in extension
        let cleanshot_dir = plugins_dir.join("cleanshot").join("scriptlets");
        assert!(
            cleanshot_dir.exists(),
            "plugins/cleanshot/scriptlets/ should exist"
        );
        let cleanshot_extension = cleanshot_dir.join("main.md");
        assert!(
            cleanshot_extension.exists(),
            "plugins/cleanshot/scriptlets/main.md should exist"
        );
        let cleanshot_content = fs::read_to_string(&cleanshot_extension).unwrap();
        assert!(
            cleanshot_content.contains("CleanShot X"),
            "CleanShot extension should have CleanShot X title"
        );
        assert!(
            cleanshot_content.contains("cleanshot://capture-area"),
            "CleanShot extension should have Capture Area command"
        );
        assert!(
            cleanshot_content.contains("cleanshot://record-screen"),
            "CleanShot extension should have Record Screen command"
        );

        // Verify 1Password built-in extension
        let onepassword_dir = plugins_dir.join("1password").join("scriptlets");
        assert!(
            onepassword_dir.exists(),
            "plugins/1password/scriptlets/ should exist"
        );
        let onepassword_extension = onepassword_dir.join("main.md");
        assert!(
            onepassword_extension.exists(),
            "plugins/1password/scriptlets/main.md should exist"
        );
        let onepassword_content = fs::read_to_string(&onepassword_extension).unwrap();
        assert!(
            onepassword_content.contains("1Password"),
            "1Password extension should have 1Password title"
        );
        assert!(
            onepassword_content.contains("op item list"),
            "1Password extension should have item list command"
        );
        assert!(
            onepassword_content.contains("op whoami"),
            "1Password extension should have whoami command"
        );

        // Verify Quick Links built-in extension
        let quicklinks_dir = plugins_dir.join("quicklinks").join("scriptlets");
        assert!(
            quicklinks_dir.exists(),
            "plugins/quicklinks/scriptlets/ should exist"
        );
        let quicklinks_extension = quicklinks_dir.join("main.md");
        assert!(
            quicklinks_extension.exists(),
            "plugins/quicklinks/scriptlets/main.md should exist"
        );
        let quicklinks_content = fs::read_to_string(&quicklinks_extension).unwrap();
        assert!(
            quicklinks_content.contains("Quick Links"),
            "Quick Links extension should have Quick Links title"
        );
        assert!(
            quicklinks_content.contains("https://github.com"),
            "Quick Links extension should have GitHub link"
        );
        assert!(
            quicklinks_content.contains("https://www.google.com"),
            "Quick Links extension should have Google link"
        );

        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    /// Test that paths in AGENTS.md match actual setup paths
    #[test]
    fn test_agents_md_paths_match_setup() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp_dir = TempDir::new().unwrap();
        let kit_root = temp_dir.path().to_path_buf();

        std::env::set_var(SK_PATH_ENV, kit_root.to_str().unwrap());
        let _ = ensure_kit_setup();

        // Root-level AGENTS.md is the canonical location now
        let agents_content = fs::read_to_string(kit_root.join("AGENTS.md")).unwrap();

        // Verify documented paths actually exist
        let documented_paths = [
            ("plugins/main/scripts", "~/.scriptkit/plugins/main/scripts/"),
            (
                "plugins/main/scriptlets",
                "~/.scriptkit/plugins/main/scriptlets/",
            ),
            ("config.ts", "~/.scriptkit/config.ts"),
            ("theme.json", "~/.scriptkit/theme.json"),
            ("sdk/kit-sdk.ts", "~/.scriptkit/sdk/"),
            ("bin/scriptkit", "~/.scriptkit/bin/scriptkit"),
        ];

        for (relative_path, doc_path) in documented_paths {
            assert!(
                agents_content.contains(doc_path),
                "AGENTS.md should document path: {}",
                doc_path
            );

            let actual_path = kit_root.join(relative_path);
            // For directories, check they exist; for files, check the parent exists
            if relative_path.contains('.') {
                assert!(
                    actual_path.exists(),
                    "Documented path {} should exist as file: {:?}",
                    doc_path,
                    actual_path
                );
            } else {
                assert!(
                    actual_path.exists(),
                    "Documented path {} should exist as directory: {:?}",
                    doc_path,
                    actual_path
                );
            }
        }

        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }
}

#[cfg(test)]
mod tab_ai_agent_doc_contract_tests {
    const ROOT_CLAUDE: &str = include_str!("../../kit-init/ROOT_CLAUDE.md");
    const ROOT_AGENTS: &str = include_str!("../../kit-init/ROOT_AGENTS.md");
    const AI_MOD_SOURCE: &str = include_str!("../ai/mod.rs");
    const TAB_CONTEXT_SOURCE: &str = include_str!("../ai/tab_context.rs");

    fn assert_tab_ai_doc_contract(source: &str, label: &str) {
        for needle in [
            "Quick Terminal with Flat Context Injection",
            "AppView::QuickTerminalView",
            "TermPrompt",
            "TabAiHarnessSubmissionMode",
            "PasteOnly",
            "Submit",
            "claudeCode",
            "CaptureContextOptions::tab_ai_submit()",
            "Cmd+W",
            "Escape",
            "Agent Chat",
            "open_tab_ai_agent_chat_with_entry_intent",
        ] {
            assert!(source.contains(needle), "{label} must contain `{needle}`");
        }

        assert!(
            source.contains("PTY") || source.contains("pty"),
            "{label} must describe the landed PTY-backed path"
        );

        // The universal AI entry migrated from plain Tab to Cmd+Enter
        // (commit b6c5752bb "Deprecate Tab ACP agent chat surface").
        assert!(
            source.contains(
                "Command+Enter in `AppView::ScriptList` routes through the Agent Chat entry path"
            ),
            "{label} must describe Cmd+Enter as the Agent Chat entry path"
        );

        assert!(
            !source.contains(
                "Plain `Tab` in `AppView::ScriptList` routes through the Agent Chat entry path"
            ),
            "{label} must not describe plain Tab as the Agent Chat entry path"
        );
        assert!(
            !source.contains("Plain `Tab` opens the harness terminal"),
            "{label} must not describe plain Tab as opening the harness terminal"
        );
        assert!(
            !source.contains("`Shift+Tab` in `AppView::ScriptList` with non-empty filter text"),
            "{label} must not describe Shift+Tab in ScriptList as the default quick-submit path"
        );
    }

    fn assert_tab_ai_schema_detail_contract(source: &str, label: &str) {
        for needle in [
            "pub const TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION: u32 = 2;",
            "pub struct TabAiExecutionRecord",
            "PanelOnlyElements",
            "CollectorFallback",
            "NoSemanticElements",
            "MissingFocusTarget",
            "InputNotExtractable",
            "InputNotApplicable",
        ] {
            assert!(source.contains(needle), "{label} must contain `{needle}`");
        }

        for stale in [
            "TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION: u32 = 1",
            "record + status + output + duration",
            "Persisted memory: intent, script, target bundle_id, outcome",
            "PanelOnlyWarning",
            "MissingInput",
        ] {
            assert!(
                !source.contains(stale),
                "{label} contains stale Tab AI schema detail: {stale}"
            );
        }
    }

    #[test]
    fn root_claude_doc_matches_landed_tab_ai_contract() {
        assert_tab_ai_doc_contract(ROOT_CLAUDE, "kit-init/ROOT_CLAUDE.md");
    }

    #[test]
    fn root_agents_doc_matches_landed_tab_ai_contract() {
        assert_tab_ai_doc_contract(ROOT_AGENTS, "kit-init/ROOT_AGENTS.md");
    }

    #[test]
    fn tab_ai_schema_detail_matches_current_tab_ai_types() {
        assert_tab_ai_schema_detail_contract(TAB_CONTEXT_SOURCE, "src/ai/tab_context.rs");
    }

    #[test]
    fn ai_mod_docs_reflect_agent_chat_primary_path() {
        for needle in [
            "//! AI surfaces and shared contracts.",
            "//! - User-facing AI chat surface: Agent Chat",
            "//! - Entry points should route to `open_tab_ai_agent_chat_with_entry_intent(...)` when they need the canonical chat UI",
        ] {
            assert!(
                AI_MOD_SOURCE.contains(needle),
                "src/ai/mod.rs docs must contain `{needle}`"
            );
        }

        assert!(
            !AI_MOD_SOURCE.contains("//! - Submission shape: flat text-native"),
            "src/ai/mod.rs must not describe QuickTerminalView as the primary AI surface"
        );
    }
}

#[cfg(test)]
mod asset_destination_tests {
    use std::path::PathBuf;

    /// Resolve the relative destination path for an embedded kit-init asset.
    ///
    /// Skills map to the workspace root (`skills/…`), the config template
    /// maps to `config.ts`, and everything else passes through unchanged.
    fn embedded_asset_destination_relative(asset: &str) -> PathBuf {
        if let Some(_rest) = asset.strip_prefix("skills/") {
            // skills/ already carries the correct relative prefix
            return PathBuf::from(asset);
        }
        match asset {
            "config-template.ts" => PathBuf::from("config.ts"),
            other => PathBuf::from(other),
        }
    }

    #[test]
    fn skills_install_to_workspace_root_skills_directory() {
        assert_eq!(
            embedded_asset_destination_relative("skills/update-config/SKILL.md"),
            PathBuf::from("skills/update-config/SKILL.md")
        );
    }

    #[test]
    fn skills_readme_installs_to_workspace_root() {
        assert_eq!(
            embedded_asset_destination_relative("skills/README.md"),
            PathBuf::from("skills/README.md")
        );
    }

    #[test]
    fn config_template_installs_under_kit_directory() {
        assert_eq!(
            embedded_asset_destination_relative("config-template.ts"),
            PathBuf::from("config.ts")
        );
    }

    #[test]
    fn passthrough_asset_unchanged() {
        assert_eq!(
            embedded_asset_destination_relative("GUIDE.md"),
            PathBuf::from("GUIDE.md")
        );
    }

    /// Verify that `ensure_kit_setup` writes bundled skills under the Script Kit plugin.
    #[test]
    fn setup_creates_skills_under_scriptkit_plugin() {
        super::tests::with_sk_path(|kit_root| {
            let result = super::ensure_kit_setup();
            assert_eq!(result.kit_path, kit_root, "setup must stay inside the fixture root");
            let skills = kit_root.join("plugins/scriptkit/skills");
            assert_eq!(
                std::fs::read_to_string(skills.join("README.md")).expect("installed skills library"),
                super::EMBEDDED_SKILLS_README
            );
            assert_eq!(
                std::fs::read_to_string(skills.join("update-config/SKILL.md"))
                    .expect("installed update-config skill"),
                super::EMBEDDED_SKILL_UPDATE_CONFIG
            );
            assert!(!kit_root.join("kit/skills").exists(), "skills must not be nested under kit/");
        });
    }
}
