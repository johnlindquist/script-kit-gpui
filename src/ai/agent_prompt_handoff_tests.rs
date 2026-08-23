#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    fn env_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env_test() -> std::sync::MutexGuard<'static, ()> {
        env_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn adapter_from_action_id_recognizes_cmux_codex() {
        assert_eq!(
            adapter_from_action_id(CMUX_CODEX_ACTION_ID),
            Some(AgentPromptHandoffAdapterId::CmuxCodex)
        );
        assert_eq!(adapter_from_action_id("agent_chat:handoff:other"), None);
    }

    #[test]
    fn cmux_args_do_not_include_raw_prompt() {
        let prompt = "summarize this private prompt";
        let workspace_args =
            build_cmux_workspace_create_rpc_args(Path::new("/Users/example/project"))
                .expect("build workspace args");
        let surface_args = build_cmux_surface_create_rpc_args(
            "workspace:99",
            Path::new("/Users/example/project"),
            "/bin/zsh '/tmp/script-kit-agent-handoff/abc/run.zsh'",
        )
        .expect("build surface args");
        let joined = workspace_args
            .iter()
            .chain(surface_args.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!joined.contains(prompt));
        assert_eq!(workspace_args[0], "rpc");
        assert_eq!(workspace_args[1], "workspace.create");
        let workspace_params: serde_json::Value =
            serde_json::from_str(&workspace_args[2]).expect("workspace rpc params");
        assert_eq!(
            workspace_params["working_directory"],
            "/Users/example/project"
        );
        assert_eq!(workspace_params["focus"], true);
        assert_eq!(workspace_params["eager_load_terminal"], true);
        assert!(workspace_params.get("initial_command").is_none());

        assert_eq!(surface_args[0], "rpc");
        assert_eq!(surface_args[1], "surface.create");
        let surface_params: serde_json::Value =
            serde_json::from_str(&surface_args[2]).expect("surface rpc params");
        assert_eq!(surface_params["workspace_id"], "workspace:99");
        assert_eq!(
            surface_params["working_directory"],
            "/Users/example/project"
        );
        assert_eq!(
            surface_params["initial_command"],
            "/bin/zsh '/tmp/script-kit-agent-handoff/abc/run.zsh'"
        );
        assert_eq!(
            surface_params["tmux_start_command"],
            "/bin/zsh '/tmp/script-kit-agent-handoff/abc/run.zsh'"
        );
        assert_eq!(surface_params["focus"], true);
    }

    #[test]
    fn receipt_does_not_serialize_raw_prompt() {
        let receipt = AgentPromptHandoffReceipt {
            adapter_id: CMUX_CODEX_ADAPTER_ID.to_string(),
            action_id: CMUX_CODEX_ACTION_ID.to_string(),
            dry_run: true,
            cwd: "/tmp".to_string(),
            prompt_chars: 26,
            prompt_sha256: sha256_hex("summarize private prompt"),
            command_kind: "cmux_workspace_surface_create_initial_command".to_string(),
            cmux_binary: "cmux".to_string(),
            codex_binary: "codex".to_string(),
            prompt_file_created: false,
            script_file_created: false,
            spawned: false,
            pid: None,
        };
        let json = serde_json::to_string(&receipt).expect("serialize receipt");
        assert!(!json.contains("summarize private prompt"));
        assert!(json.contains("\"promptSha256\""));
        assert!(json.contains("\"promptChars\""));
    }

    #[test]
    fn export_receipt_does_not_serialize_raw_prompt() {
        let receipt = AgentPromptExportReceipt {
            action_id: EXPORT_FILE_ACTION_ID.to_string(),
            dry_run: true,
            cwd: "/tmp".to_string(),
            prompt_chars: 26,
            prompt_sha256: sha256_hex("summarize private prompt"),
            context_part_count: 2,
            prompt_builder_segment_count: 3,
            export_kind: "file".to_string(),
            path: Some("/tmp/prompt.md".to_string()),
            url: None,
            command_kind: "prompt_export_file".to_string(),
            clipboard_written: false,
            spawned: false,
        };
        let json = serde_json::to_string(&receipt).expect("serialize receipt");
        assert!(!json.contains("summarize private prompt"));
        assert!(json.contains("\"promptSha256\""));
        assert!(json.contains("\"promptChars\""));
        assert!(json.contains("\"contextPartCount\""));
        assert!(json.contains("\"promptBuilderSegmentCount\""));
        assert!(json.contains("\"clipboardWritten\""));
    }

    #[test]
    fn export_file_writes_prompt_to_configured_directory() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let receipt_path = temp_dir.path().join("receipt.json");
        let export_dir = temp_dir.path().join("exports");
        let prompt = "file export proof prompt";
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (GH_BINARY_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(receipt_path.to_string_lossy().to_string()),
            ),
            (
                PROMPT_EXPORT_DIR_ENV,
                Some(export_dir.to_string_lossy().to_string()),
            ),
        ]);

        let receipt = export_prompt(&test_payload(prompt), AgentPromptActionId::ExportFile)
            .expect("export prompt");

        assert!(!receipt.dry_run);
        assert_eq!(receipt.action_id, EXPORT_FILE_ACTION_ID);
        assert_eq!(receipt.export_kind, "file");
        assert_eq!(receipt.command_kind, "prompt_export_file");
        assert_eq!(receipt.prompt_sha256, sha256_hex(prompt));
        assert_eq!(receipt.context_part_count, 0);
        assert_eq!(receipt.prompt_builder_segment_count, 0);
        assert!(!receipt.clipboard_written);
        let path = PathBuf::from(receipt.path.as_deref().expect("export path"));
        assert!(path.starts_with(&export_dir));
        assert_eq!(
            std::fs::read_to_string(&path).expect("exported prompt"),
            prompt
        );
        let serialized_receipt =
            std::fs::read_to_string(&receipt_path).expect("serialized export receipt");
        assert!(serialized_receipt.contains("\"prompt_export_file\""));
        assert!(!serialized_receipt.contains(prompt));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            assert_eq!(
                std::fs::metadata(&export_dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(&receipt_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn private_prompt_export_repairs_legacy_directory_and_receipt_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let _guard = lock_env_test();
        let fixture = tempfile::tempdir().expect("isolated legacy export fixture");
        let export_dir = fixture.path().join("exports");
        let receipt_path = fixture.path().join("receipt.json");
        std::fs::create_dir(&export_dir).unwrap();
        std::fs::set_permissions(&export_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(&receipt_path, "older unsafe receipt").unwrap();
        std::fs::set_permissions(&receipt_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _environment = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (GH_BINARY_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(receipt_path.to_string_lossy().to_string()),
            ),
            (
                PROMPT_EXPORT_DIR_ENV,
                Some(export_dir.to_string_lossy().to_string()),
            ),
        ]);

        let prompt = "private user prompt with attached account data";
        let receipt = export_prompt(&test_payload(prompt), AgentPromptActionId::ExportFile)
            .expect("repair legacy private export owners");
        let prompt_path = PathBuf::from(receipt.path.unwrap());

        assert_eq!(
            std::fs::metadata(&export_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&prompt_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&receipt_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_to_string(prompt_path).unwrap(), prompt);
        assert!(!std::fs::read_to_string(receipt_path)
            .unwrap()
            .contains(prompt));
    }

    #[cfg(unix)]
    #[test]
    fn private_prompt_export_rejects_symlinked_directory_without_touching_foreign_owner() {
        use std::os::unix::fs::{symlink, PermissionsExt as _};

        let _guard = lock_env_test();
        let fixture = tempfile::tempdir().expect("isolated symlinked export directory fixture");
        let external = fixture.path().join("foreign");
        let planted = fixture.path().join("exports");
        std::fs::create_dir(&external).unwrap();
        std::fs::set_permissions(&external, std::fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&external, &planted).unwrap();
        let _environment = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (GH_BINARY_ENV, None),
            (RECEIPT_PATH_ENV, None),
            (
                PROMPT_EXPORT_DIR_ENV,
                Some(planted.to_string_lossy().to_string()),
            ),
        ]);

        assert!(export_prompt(
            &test_payload("never export my private prompt through a link"),
            AgentPromptActionId::ExportFile,
        )
        .is_err());
        assert_eq!(std::fs::read_dir(&external).unwrap().count(), 0);
        assert_eq!(
            std::fs::metadata(&external).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn private_prompt_export_rejects_symlinked_receipt_without_touching_foreign_file() {
        use std::os::unix::fs::symlink;

        let _guard = lock_env_test();
        let fixture = tempfile::tempdir().expect("isolated symlinked export receipt fixture");
        let external = fixture.path().join("foreign.txt");
        let planted = fixture.path().join("receipt.json");
        let export_dir = fixture.path().join("exports");
        std::fs::write(&external, "foreign receipt must not change").unwrap();
        symlink(&external, &planted).unwrap();
        let _environment = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (GH_BINARY_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(planted.to_string_lossy().to_string()),
            ),
            (
                PROMPT_EXPORT_DIR_ENV,
                Some(export_dir.to_string_lossy().to_string()),
            ),
        ]);

        assert!(export_prompt(
            &test_payload("private export receipt payload"),
            AgentPromptActionId::ExportFile,
        )
        .is_err());
        assert_eq!(
            std::fs::read_to_string(&external).unwrap(),
            "foreign receipt must not change"
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn private_prompt_handoff_file_rejects_symlinked_destination_without_corruption() {
        use std::os::unix::fs::symlink;

        let fixture = tempfile::tempdir().expect("isolated symlinked handoff prompt fixture");
        let external = fixture.path().join("foreign.txt");
        let planted = fixture.path().join("prompt.md");
        std::fs::write(&external, "foreign private content").unwrap();
        symlink(&external, &planted).unwrap();

        assert!(write_private_handoff_file(&planted, b"new private user prompt").is_err());
        assert_eq!(
            std::fs::read_to_string(&external).unwrap(),
            "foreign private content"
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn private_prompt_handoff_receipt_is_owner_only_and_rejects_symlinked_target() {
        use std::os::unix::fs::{symlink, PermissionsExt as _};

        let _guard = lock_env_test();
        let fixture = tempfile::tempdir().expect("isolated private handoff receipt fixture");
        let receipt_path = fixture.path().join("handoff.json");
        let _environment = HandoffEnvGuard::set([(
            RECEIPT_PATH_ENV,
            Some(receipt_path.to_string_lossy().to_string()),
        )]);
        let receipt = AgentPromptHandoffReceipt {
            adapter_id: CMUX_CODEX_ADAPTER_ID.to_string(),
            action_id: CMUX_CODEX_ACTION_ID.to_string(),
            dry_run: true,
            cwd: "/synthetic/private-project".to_string(),
            prompt_chars: 14,
            prompt_sha256: sha256_hex("private prompt"),
            command_kind: "synthetic-handoff".to_string(),
            cmux_binary: "cmux".to_string(),
            codex_binary: "codex".to_string(),
            prompt_file_created: false,
            script_file_created: false,
            spawned: false,
            pid: None,
        };
        write_receipt_if_requested(&receipt).expect("owner-only actual handoff receipt");
        assert_eq!(
            std::fs::metadata(&receipt_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let external = fixture.path().join("foreign.txt");
        std::fs::write(&external, "never overwrite this foreign receipt").unwrap();
        std::fs::remove_file(&receipt_path).unwrap();
        symlink(&external, &receipt_path).unwrap();
        assert!(write_receipt_if_requested(&receipt).is_err());
        assert_eq!(
            std::fs::read_to_string(external).unwrap(),
            "never overwrite this foreign receipt"
        );
    }

    #[test]
    fn private_prompt_handoff_diagnostics_never_expose_guessable_hashes_or_locations() {
        let prompt = "private customer prompt";
        let digest = sha256_hex(prompt);
        let handoff = AgentPromptHandoffReceipt {
            adapter_id: CMUX_CODEX_ADAPTER_ID.to_string(),
            action_id: CMUX_CODEX_ACTION_ID.to_string(),
            dry_run: true,
            cwd: "/synthetic/private-project".to_string(),
            prompt_chars: prompt.chars().count(),
            prompt_sha256: digest.clone(),
            command_kind: "synthetic-handoff".to_string(),
            cmux_binary: "cmux".to_string(),
            codex_binary: "codex".to_string(),
            prompt_file_created: false,
            script_file_created: false,
            spawned: false,
            pid: None,
        };
        let export = AgentPromptExportReceipt {
            action_id: EXPORT_FILE_ACTION_ID.to_string(),
            dry_run: true,
            cwd: "/synthetic/private-project".to_string(),
            prompt_chars: prompt.chars().count(),
            prompt_sha256: digest.clone(),
            context_part_count: 1,
            prompt_builder_segment_count: 1,
            export_kind: "file".to_string(),
            path: Some("/Users/private/medical-client/prompt.md".to_string()),
            url: Some("https://gist.github.com/private-secret-gist".to_string()),
            command_kind: "synthetic-export".to_string(),
            clipboard_written: false,
            spawned: false,
        };

        let expected = crate::logging::log_private_user_value(&digest).sha256;
        assert_eq!(handoff.diagnostic_prompt_fingerprint(), expected);
        assert_eq!(export.diagnostic_prompt_fingerprint(), expected);
        assert_ne!(expected, digest);
        assert_eq!(
            export.diagnostic_path_fingerprint().unwrap(),
            crate::logging::log_private_user_value("/Users/private/medical-client/prompt.md")
                .sha256
        );
        assert_eq!(
            export.diagnostic_url_fingerprint().unwrap(),
            crate::logging::log_private_user_value("https://gist.github.com/private-secret-gist")
                .sha256
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_prompt_wrapper_is_owner_only_without_starting_external_process() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = tempfile::tempdir().expect("isolated wrapper preparation fixture");
        let handoff_root = fixture.path().join("handoff-root");
        let prompt = "private wrapper handoff prompt";
        let prepared = prepare_cmux_codex_wrapper_at(
            prompt,
            "/synthetic/never-start-codex",
            fixture.path(),
            &handoff_root,
        )
        .expect("prepare private wrapper without running it");
        let script_path = prepared
            .command_string
            .strip_prefix("/bin/zsh '")
            .and_then(|value| value.strip_suffix('\''))
            .map(PathBuf::from)
            .expect("synthetic quoted wrapper path");
        let handoff_dir = script_path.parent().unwrap();
        let prompt_path = handoff_dir.join("prompt.md");

        for (path, expected) in [
            (handoff_root.as_path(), 0o700),
            (handoff_dir, 0o700),
            (prompt_path.as_path(), 0o600),
            (script_path.as_path(), 0o700),
        ] {
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                expected
            );
        }
        assert_eq!(std::fs::read_to_string(prompt_path).unwrap(), prompt);
        assert!(!std::fs::read_to_string(script_path)
            .unwrap()
            .contains(prompt));
    }

    #[test]
    fn export_gist_uses_private_gh_gist_create_without_leaking_prompt_in_receipt() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let gh_stub_path = temp_dir.path().join("gh-stub.py");
        let gh_receipt_path = temp_dir.path().join("gh-receipt.json");
        let export_receipt_path = temp_dir.path().join("export-receipt.json");
        let prompt = "gist export proof prompt";
        std::fs::write(
            &gh_stub_path,
            r#"#!/usr/bin/env python3
import hashlib
import json
import os
import sys

args = sys.argv[1:]
prompt_path = args[2]
with open(prompt_path, 'r') as handle:
    prompt = handle.read()
with open(os.environ['GH_STUB_RECEIPT'], 'w') as handle:
    json.dump({
        'argv': args,
        'promptSha256': hashlib.sha256(prompt.encode()).hexdigest(),
        'hasPrivateFlag': '--private' in args,
        'hasFilenameFlag': '--filename' in args,
    }, handle, indent=2)
print('https://gist.github.com/fake/private-gist')
"#,
        )
        .expect("write gh stub");
        set_file_mode(&gh_stub_path, 0o700).expect("chmod gh stub");
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(export_receipt_path.to_string_lossy().to_string()),
            ),
            (
                GH_BINARY_ENV,
                Some(gh_stub_path.to_string_lossy().to_string()),
            ),
            (
                "GH_STUB_RECEIPT",
                Some(gh_receipt_path.to_string_lossy().to_string()),
            ),
            (PROMPT_EXPORT_DIR_ENV, None),
        ]);

        let receipt = export_prompt(&test_payload(prompt), AgentPromptActionId::ExportGist)
            .expect("export gist");

        assert_eq!(receipt.action_id, EXPORT_GIST_ACTION_ID);
        assert_eq!(receipt.export_kind, "gist");
        assert_eq!(receipt.command_kind, "prompt_export_gist_private");
        assert_eq!(receipt.context_part_count, 0);
        assert_eq!(receipt.prompt_builder_segment_count, 0);
        assert!(!receipt.clipboard_written);
        assert_eq!(
            receipt.url.as_deref(),
            Some("https://gist.github.com/fake/private-gist")
        );
        assert!(receipt.spawned);
        assert_eq!(receipt.path, None);

        let gh_receipt = std::fs::read_to_string(&gh_receipt_path).expect("gh receipt");
        assert!(gh_receipt.contains("\"gist\""));
        assert!(gh_receipt.contains("\"create\""));
        assert!(gh_receipt.contains("\"hasPrivateFlag\": true"));
        assert!(gh_receipt.contains("\"hasFilenameFlag\": true"));
        assert!(gh_receipt.contains(&sha256_hex(prompt)));
        let export_receipt = std::fs::read_to_string(&export_receipt_path).expect("export receipt");
        assert!(export_receipt.contains("\"prompt_export_gist_private\""));
        assert!(!export_receipt.contains(prompt));
    }

    #[test]
    fn builtin_prompt_actions_include_copy_prompt_clipboard_action() {
        let actions = builtin_prompt_actions();
        assert!(actions.contains(&AgentPromptActionId::CopyPrompt));
        assert_eq!(
            prompt_action_from_action_id(COPY_PROMPT_ACTION_ID),
            Some(AgentPromptActionId::CopyPrompt)
        );
        assert_eq!(
            AgentPromptActionId::CopyPrompt.id(),
            COPY_PROMPT_PROMPT_ACTION_ID
        );
        assert_eq!(
            AgentPromptActionId::CopyPrompt.action_id(),
            COPY_PROMPT_ACTION_ID
        );
        assert_eq!(
            prompt_action_id(COPY_PROMPT_PROMPT_ACTION_ID),
            COPY_PROMPT_ACTION_ID
        );
    }

    #[test]
    fn copy_prompt_dry_run_receipt_hash_matches_exact_prompt_without_clipboard_write() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let receipt_path = temp_dir.path().join("copy-receipt.json");
        let prompt = "copy prompt proof";
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, Some("1".to_string())),
            (
                RECEIPT_PATH_ENV,
                Some(receipt_path.to_string_lossy().to_string()),
            ),
        ]);

        let receipt = copy_prompt_to_clipboard_with_writer(
            &test_payload(prompt),
            AgentPromptActionId::CopyPrompt,
            |_| panic!("dry-run copy must not write clipboard"),
        )
        .expect("copy prompt dry-run");

        assert!(receipt.dry_run);
        assert_eq!(receipt.action_id, COPY_PROMPT_ACTION_ID);
        assert_eq!(receipt.export_kind, "clipboard");
        assert_eq!(receipt.command_kind, "prompt_copy_clipboard");
        assert_eq!(receipt.prompt_sha256, sha256_hex(prompt));
        assert!(!receipt.clipboard_written);
        let serialized_receipt =
            std::fs::read_to_string(&receipt_path).expect("serialized copy receipt");
        assert!(serialized_receipt.contains("\"prompt_copy_clipboard\""));
        assert!(!serialized_receipt.contains(prompt));
    }

    #[test]
    fn copy_prompt_to_clipboard_writer_receives_exact_prompt() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let copied_path = temp_dir.path().join("copied.txt");
        let receipt_path = temp_dir.path().join("copy-receipt.json");
        let prompt = "copy prompt exact payload\nwith newline";
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(receipt_path.to_string_lossy().to_string()),
            ),
        ]);

        let receipt = copy_prompt_to_clipboard_with_writer(
            &test_payload(prompt),
            AgentPromptActionId::CopyPrompt,
            |value| {
                std::fs::write(&copied_path, value)
                    .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))
            },
        )
        .expect("copy prompt");

        assert!(!receipt.dry_run);
        assert_eq!(receipt.export_kind, "clipboard");
        assert_eq!(receipt.command_kind, "prompt_copy_clipboard");
        assert!(receipt.clipboard_written);
        assert_eq!(
            std::fs::read_to_string(&copied_path).expect("copied prompt"),
            prompt
        );
        let serialized_receipt =
            std::fs::read_to_string(&receipt_path).expect("serialized copy receipt");
        assert!(serialized_receipt.contains("\"clipboardWritten\": true"));
        assert!(!serialized_receipt.contains(prompt));
    }

    #[test]
    fn handoff_compiler_preserves_every_ai_context_part_variant() {
        let fixture = RichPromptContextFixture::new();
        assert_ai_context_part_variant_coverage(&fixture.parts);

        let payload = compile_fixture_payload(&fixture);

        assert_eq!(payload.context_part_count, fixture.parts.len());
        assert_eq!(payload.prompt_builder_segment_count, 1);
        assert_compiled_prompt_contains_all_context_fingerprints(&payload.prompt, &fixture);
        assert!(
            !payload
                .prompt
                .contains("PROMPT_EXPORT_AMBIENT_DISPLAY_ONLY_SENTINEL"),
            "AmbientContext is display-only; staged content must arrive as a ResourceUri or TextBlock"
        );
    }

    #[test]
    fn ambient_context_export_policy_is_explicit() {
        let part = AiContextPart::AmbientContext {
            label: "PROMPT_EXPORT_AMBIENT_DISPLAY_ONLY_SENTINEL".to_string(),
        };
        let prepared =
            crate::ai::message_parts::prepare_user_message_with_receipt("ask", &[part], &[], &[]);

        assert_eq!(prepared.context.attempted, 1);
        assert_eq!(prepared.context.resolved, 0);
        assert_eq!(prepared.final_user_content, "ask");
        assert_eq!(
            prepared.outcomes[0].kind,
            crate::ai::message_parts::ContextPartPreparationOutcomeKind::DisplayOnly
        );
    }

    #[test]
    fn prompt_actions_share_identical_compiled_prompt_hash_for_file_and_clipboard() {
        let _guard = lock_env_test();
        let fixture = RichPromptContextFixture::new();
        let payload = compile_fixture_payload(&fixture);
        let export_dir = fixture.temp_dir.path().join("exports");
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (RECEIPT_PATH_ENV, None),
            (
                PROMPT_EXPORT_DIR_ENV,
                Some(export_dir.to_string_lossy().to_string()),
            ),
        ]);

        let file_receipt =
            export_prompt(&payload, AgentPromptActionId::ExportFile).expect("file export");
        let copied = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let copied_for_writer = copied.clone();
        let copy_receipt = copy_prompt_to_clipboard_with_writer(
            &payload,
            AgentPromptActionId::CopyPrompt,
            move |value| {
                *copied_for_writer.lock().expect("copy lock") = value.to_string();
                Ok(())
            },
        )
        .expect("copy prompt");

        assert_export_receipt_matches_payload(
            &file_receipt,
            &payload,
            "file",
            "prompt_export_file",
        );
        assert_export_receipt_matches_payload(
            &copy_receipt,
            &payload,
            "clipboard",
            "prompt_copy_clipboard",
        );
        assert_eq!(file_receipt.prompt_sha256, copy_receipt.prompt_sha256);
        assert_eq!(file_receipt.prompt_chars, copy_receipt.prompt_chars);
        assert_eq!(
            file_receipt.context_part_count,
            copy_receipt.context_part_count
        );
        assert_eq!(
            file_receipt.prompt_builder_segment_count,
            copy_receipt.prompt_builder_segment_count
        );
        let exported_path = PathBuf::from(file_receipt.path.as_deref().expect("export path"));
        assert_eq!(
            std::fs::read_to_string(exported_path).expect("exported rich prompt"),
            payload.prompt
        );
        assert_eq!(*copied.lock().expect("copied prompt"), payload.prompt);
        assert!(!file_receipt.clipboard_written);
        assert!(copy_receipt.clipboard_written);
    }

    #[test]
    fn handoff_compiler_matches_spine_submit_plan_for_prompt_builder_inputs() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let file_path = temp_dir.path().join("brief.txt");
        std::fs::write(&file_path, "briefing contents").expect("write fixture file");
        let file_input = format!("@file:{} summarize", file_path.to_string_lossy());
        let profile_file_input = format!(
            "|creative /rewrite @file:{} make it useful",
            file_path.to_string_lossy()
        );
        let cases = [
            (
                "/rewrite make this concise",
                None,
                Some("/rewrite\n\nmake this concise"),
            ),
            (
                profile_file_input.as_str(),
                Some("creative"),
                Some("briefing contents"),
            ),
            // Styles are not profiles: `.professional` carries an explicit
            // tone instruction in the prompt and selects no profile.
            (
                ".professional make it shorter",
                None,
                Some("professional workplace tone"),
            ),
            (">:demo explain setup", None, Some("explain setup")),
            ("@unknownThing summarize", None, Some("Preflight warning")),
            (file_input.as_str(), None, Some("briefing contents")),
        ];

        for (raw, expected_profile, expected_prompt_fragment) in cases {
            let parse = crate::spine::parse_spine(raw);
            let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);
            assert!(plan.should_submit_to_chat(), "{raw} should be submittable");

            let scripts: Vec<std::sync::Arc<crate::scripts::Script>> = Vec::new();
            let scriptlets: Vec<std::sync::Arc<crate::scripts::Scriptlet>> = Vec::new();
            let expected = crate::ai::message_parts::prepare_user_message_with_receipt(
                plan.normalized_prompt.trim(),
                &plan.context_parts,
                &scripts,
                &scriptlets,
            );
            let result = compile_handoff_payload_from_spine_plan(
                AgentPromptHandoffAdapterId::CmuxCodex,
                raw.to_string(),
                PathBuf::from("/tmp/project"),
                Some("gpt-test".to_string()),
                Vec::new(),
                plan.clone(),
            );

            if expected.decision == PreparedMessageDecision::Blocked {
                assert!(
                    matches!(result, Err(AgentPromptHandoffError::UnsupportedPrompt(_))),
                    "{raw} should block like normal message preparation"
                );
                continue;
            }

            let payload =
                result.unwrap_or_else(|error| panic!("compile {raw}: {}", error.user_message()));

            let expected_content = expected.final_user_content.trim();
            if plan.context_parts.is_empty() {
                assert_eq!(payload.prompt, expected_content, "{raw}");
            } else {
                let normalized_prompt = plan.normalized_prompt.trim();
                assert!(
                    normalized_prompt.is_empty() || payload.prompt.contains(normalized_prompt),
                    "{raw} payload prompt did not contain normalized prompt {normalized_prompt:?}: {:?}",
                    payload.prompt
                );
            }
            assert_eq!(
                payload.prompt_builder_segment_count, plan.prompt_builder_segment_count,
                "{raw}"
            );
            assert_eq!(
                payload.context_part_count,
                plan.context_parts.len(),
                "{raw}"
            );
            assert_eq!(payload.profile_id.as_deref(), expected_profile, "{raw}");
            if let Some(fragment) = expected_prompt_fragment {
                assert!(
                    payload.prompt.contains(fragment),
                    "{raw} prompt did not contain {fragment:?}: {:?}",
                    payload.prompt
                );
            }
        }
    }

    #[test]
    fn handoff_compiler_blocks_non_submittable_prompt_builder_and_mode_inputs() {
        for raw in [
            "@clip",
            ">",
            ";todo Buy milk",
            ":type:script git",
            "!echo hi",
            "?help",
            "~note",
        ] {
            let parse = crate::spine::parse_spine(raw);
            let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);
            let result = compile_handoff_payload_from_spine_plan(
                AgentPromptHandoffAdapterId::CmuxCodex,
                raw.to_string(),
                PathBuf::from("/tmp/project"),
                None,
                Vec::new(),
                plan,
            );

            assert!(
                matches!(result, Err(AgentPromptHandoffError::UnsupportedPrompt(_))),
                "{raw} should be blocked, got {result:?}"
            );
        }
    }

    #[test]
    fn handoff_policy_covers_all_main_input_spine_construct_classes() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Expected {
            Submit,
            PlainFallback,
            Block,
        }

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let file_path = temp_dir.path().join("brief.txt");
        std::fs::write(&file_path, "briefing contents").expect("write fixture file");
        let file_input = format!("@file:{} summarize this", file_path.to_string_lossy());
        let cases = [
            (
                "free text fallback",
                "plain request",
                Expected::PlainFallback,
            ),
            (
                "context mention builtin",
                "@diagnostics explain this",
                Expected::Submit,
            ),
            (
                "context mention file",
                file_input.as_str(),
                Expected::Submit,
            ),
            (
                "context mention unknown",
                "@unknownThing explain",
                Expected::Submit,
            ),
            (
                "slash command",
                "/rewrite make this concise",
                Expected::Submit,
            ),
            ("profile", "|creative brainstorm options", Expected::Submit),
            (
                "style sugar",
                ".professional make it shorter",
                Expected::Submit,
            ),
            (
                "project cwd",
                ">:demo inspect this project",
                Expected::Submit,
            ),
            ("capture syntax", ";todo Buy milk", Expected::Block),
            ("list filter", ":type:script git", Expected::Block),
            ("mode exit shell", "!echo hi", Expected::Block),
            ("mode exit help", "?help", Expected::Block),
            ("mode exit note", "~note", Expected::Block),
            ("incomplete context draft", "@clip", Expected::Block),
            ("incomplete cwd draft", ">", Expected::Block),
        ];

        for (label, raw, expected) in cases {
            let parse = crate::spine::parse_spine(raw);
            let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);
            let result = compile_handoff_payload_from_spine_plan(
                AgentPromptHandoffAdapterId::CmuxCodex,
                raw.to_string(),
                PathBuf::from("/tmp/project"),
                None,
                Vec::new(),
                plan.clone(),
            );

            match expected {
                Expected::Submit => {
                    let payload = result.unwrap_or_else(|error| {
                        panic!("{label} should submit, got {}", error.user_message())
                    });
                    assert!(
                        payload.prompt_builder_segment_count > 0,
                        "{label} should use prompt-builder semantics"
                    );
                    assert!(
                        plan.should_submit_to_chat(),
                        "{label} should match Spine submit"
                    );
                }
                Expected::PlainFallback => {
                    let payload = result.unwrap_or_else(|error| {
                        panic!(
                            "{label} should hand off as plain text, got {}",
                            error.user_message()
                        )
                    });
                    assert_eq!(payload.prompt_builder_segment_count, 0, "{label}");
                    assert_eq!(payload.prompt, raw.trim(), "{label}");
                }
                Expected::Block => {
                    assert!(
                        matches!(result, Err(AgentPromptHandoffError::UnsupportedPrompt(_))),
                        "{label} should block, got {result:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn configured_prompt_target_launch_sets_prompt_env_and_placeholders() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let target_path = temp_dir.path().join("target.py");
        let receipt_path = temp_dir.path().join("target-receipt.json");
        let handoff_receipt_path = temp_dir.path().join("handoff-receipt.json");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir(&project_dir).expect("project dir");
        std::fs::write(
            &target_path,
            r#"#!/usr/bin/env python3
import hashlib
import json
import os
import pathlib
import sys

prompt = os.environ["SCRIPT_KIT_PROMPT"]
prompt_file = pathlib.Path(sys.argv[2])
with open(os.environ["TARGET_RECEIPT"], "w") as handle:
    json.dump({
        "argv": sys.argv[1:],
        "cwd": os.getcwd(),
        "promptSha256": hashlib.sha256(prompt.encode()).hexdigest(),
        "promptFileSha256": hashlib.sha256(prompt_file.read_text().encode()).hexdigest(),
        "targetId": os.environ["SCRIPT_KIT_PROMPT_TARGET_ID"],
        "customEnv": os.environ["CUSTOM_PROMPT"],
    }, handle, indent=2)
"#,
        )
        .expect("write target stub");
        set_file_mode(&target_path, 0o700).expect("chmod target stub");

        let prompt = "custom target prompt";
        let target = AgentPromptCommandTarget {
            id: "custom-app".to_string(),
            title: "Custom App".to_string(),
            description: None,
            command: target_path.to_string_lossy().to_string(),
            args: vec!["--prompt-file".to_string(), "{promptFile}".to_string()],
            cwd: Some(project_dir.clone()),
            env: HashMap::from([
                (
                    "TARGET_RECEIPT".to_string(),
                    receipt_path.to_string_lossy().to_string(),
                ),
                ("CUSTOM_PROMPT".to_string(), "{prompt}".to_string()),
            ]),
        };
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(handoff_receipt_path.to_string_lossy().to_string()),
            ),
        ]);
        let payload = AgentPromptHandoffPayload {
            source: AgentPromptHandoffSource::AgentChatComposer,
            adapter_id: AgentPromptHandoffAdapterId::Command(target),
            raw_input: prompt.to_string(),
            prompt: prompt.to_string(),
            cwd: PathBuf::from("/tmp"),
            model_id: None,
            profile_id: None,
            context_part_count: 0,
            prompt_builder_segment_count: 0,
            warnings: Vec::new(),
        };

        let receipt = launch_prompt_handoff(&payload).expect("launch custom target");
        assert!(receipt.spawned);
        assert_eq!(receipt.action_id, "prompt-target/custom-app");
        wait_for_file(&receipt_path, Duration::from_secs(5)).expect("target receipt");

        let target_receipt = std::fs::read_to_string(&receipt_path).expect("target receipt");
        let target_receipt_json: serde_json::Value =
            serde_json::from_str(&target_receipt).expect("target receipt json");
        let actual_cwd = target_receipt_json["cwd"]
            .as_str()
            .map(PathBuf::from)
            .expect("target receipt cwd");
        let canonical_project_dir =
            std::fs::canonicalize(&project_dir).expect("canonical project dir");
        assert_eq!(
            std::fs::canonicalize(actual_cwd).expect("canonical target cwd"),
            canonical_project_dir
        );
        assert!(target_receipt.contains(&format!("\"promptSha256\": \"{}\"", sha256_hex(prompt))));
        assert!(
            target_receipt.contains(&format!("\"promptFileSha256\": \"{}\"", sha256_hex(prompt)))
        );
        assert!(target_receipt.contains("\"targetId\": \"custom-app\""));
        assert!(target_receipt.contains("\"customEnv\": \"custom target prompt\""));

        let handoff_receipt =
            std::fs::read_to_string(&handoff_receipt_path).expect("handoff receipt");
        assert!(handoff_receipt.contains("\"commandKind\": \"prompt_target_command\""));
        assert!(!handoff_receipt.contains(prompt));
    }

    #[test]
    fn handoff_compiler_preserves_plain_text_and_dedupes_attached_context() {
        let attached = AiContextPart::TextBlock {
            label: "Note".to_string(),
            source: "test://note".to_string(),
            text: "attached note".to_string(),
            mime_type: None,
        };
        let parse = crate::spine::parse_spine("plain question");
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);

        let payload = compile_handoff_payload_from_spine_plan(
            AgentPromptHandoffAdapterId::CmuxCodex,
            "plain question".to_string(),
            PathBuf::from("/tmp/project"),
            None,
            vec![attached.clone(), attached],
            plan,
        )
        .expect("plain text handoff with attached context");

        assert_eq!(payload.prompt_builder_segment_count, 0);
        assert_eq!(payload.context_part_count, 1);
        assert!(payload.prompt.contains("attached note"));
        assert!(payload.prompt.ends_with("plain question"));
    }

    #[test]
    fn handoff_compiler_blocks_when_context_preparation_blocks_normal_submit() {
        let missing_file = AiContextPart::FilePath {
            path: "/definitely/missing/script-kit-handoff.txt".to_string(),
            label: "missing.txt".to_string(),
        };
        let parse = crate::spine::parse_spine("plain question");
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);

        let result = compile_handoff_payload_from_spine_plan(
            AgentPromptHandoffAdapterId::CmuxCodex,
            "plain question".to_string(),
            PathBuf::from("/tmp/project"),
            None,
            vec![missing_file],
            plan,
        );

        assert!(
            matches!(result, Err(AgentPromptHandoffError::UnsupportedPrompt(ref reason)) if reason == "This context could not be prepared. Retry or remove it before sending." && !reason.contains("script-kit-handoff.txt")),
            "missing context should block handoff like normal submit: {result:?}"
        );
    }

    #[test]
    fn cmux_codex_wrapper_uses_codex_cd_and_secure_temp_files() {
        let stub_dir = tempfile::tempdir().expect("stub dir");
        let stub_path = stub_dir.path().join("codex-stub.py");
        let receipt_path = stub_dir.path().join("codex-receipt.txt");
        let project_dir = stub_dir.path().join("project");
        std::fs::create_dir(&project_dir).expect("project dir");
        std::fs::write(
            &stub_path,
            "#!/usr/bin/env python3\nimport hashlib\nimport os\nimport pathlib\nimport sys\nprompt = sys.argv[4]\npathlib.Path(os.environ['CODEX_STUB_RECEIPT']).write_text('\\n'.join([\n    'pwd=' + os.getcwd(),\n    'argv0=' + sys.argv[1],\n    'argv1=' + sys.argv[2],\n    'argv2=' + sys.argv[3],\n    'prompt_sha=' + hashlib.sha256(prompt.encode()).hexdigest(),\n    'prompt_repr=' + repr(prompt),\n]))\n",
        )
        .expect("write codex stub");
        set_file_mode(&stub_path, 0o700).expect("chmod codex stub");
        let prompt = "prompt with trailing newline\n";
        let prepared =
            prepare_cmux_codex_wrapper(prompt, &stub_path.to_string_lossy(), &project_dir)
                .expect("prepare wrapper");
        let script_path = prepared
            .command_string
            .strip_prefix("/bin/zsh '")
            .and_then(|value| value.strip_suffix('\''))
            .map(PathBuf::from)
            .expect("quoted script path");
        let prompt_path = script_path.parent().expect("script dir").join("prompt.md");
        let script = std::fs::read_to_string(&script_path).expect("read wrapper");

        assert!(script.contains("os.execvp(codex_binary"));
        assert!(script.contains("'--cd', cwd, '--', prompt"));
        assert!(script.contains("os.unlink(path)"));
        assert!(!script.contains("$(cat"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir_mode = std::fs::metadata(script_path.parent().unwrap())
                .expect("dir metadata")
                .permissions()
                .mode()
                & 0o777;
            let prompt_mode = std::fs::metadata(&prompt_path)
                .expect("prompt metadata")
                .permissions()
                .mode()
                & 0o777;
            let script_mode = std::fs::metadata(&script_path)
                .expect("script metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
            assert_eq!(prompt_mode, 0o600);
            assert_eq!(script_mode, 0o700);
        }

        let status = std::process::Command::new("/bin/zsh")
            .arg(&script_path)
            .env("CODEX_STUB_RECEIPT", &receipt_path)
            .status()
            .expect("run wrapper");
        assert!(status.success(), "wrapper should launch codex stub");
        let receipt = std::fs::read_to_string(&receipt_path).expect("stub receipt");
        let canonical_project_dir =
            std::fs::canonicalize(&project_dir).expect("canonical project dir");
        assert!(receipt.contains(&format!("pwd={}", canonical_project_dir.to_string_lossy())));
        assert!(receipt.contains("argv0=--cd"));
        assert!(receipt.contains(&format!("argv1={}", project_dir.to_string_lossy())));
        assert!(receipt.contains("argv2=--"));
        assert!(receipt.contains(&format!("prompt_sha={}", sha256_hex(prompt))));
        assert!(receipt.contains("prompt_repr='prompt with trailing newline\\n'"));
        assert!(
            !prompt_path.exists(),
            "prompt file should be removed before Codex runs"
        );
        assert!(
            !script_path.exists(),
            "wrapper script should be removed before Codex runs"
        );
        assert!(
            !script_path.parent().expect("script dir").exists(),
            "handoff temp dir should be removed before Codex runs"
        );
    }

    #[test]
    fn launch_cmux_codex_spawns_cmux_stub_that_executes_codex_stub() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir(&project_dir).expect("project dir");
        let cmux_stub_path = temp_dir.path().join("cmux-stub.py");
        let codex_stub_path = temp_dir.path().join("codex-stub.py");
        let cmux_receipt_path = temp_dir.path().join("cmux-receipt.json");
        let codex_receipt_path = temp_dir.path().join("codex-receipt.json");
        let handoff_receipt_path = temp_dir.path().join("handoff-receipt.json");
        let prompt = "cmux stub chain prompt\nwith trailing newline\n";

        std::fs::write(
            &cmux_stub_path,
            r#"#!/usr/bin/env python3
import json
import os
import subprocess
import sys

args = sys.argv[1:]
method = args[1]
params = json.loads(args[2])
raw_prompt = os.environ['RAW_PROMPT']
path = os.environ['CMUX_STUB_RECEIPT']
if method == 'workspace.create':
    with open(path, 'w') as handle:
        json.dump({
            'calls': [{
                'argv': args,
                'commandVerb': args[:2],
                'cwd': params['working_directory'],
                'focus': params.get('focus'),
                'eagerLoadTerminal': params.get('eager_load_terminal'),
                'hasInitialCommand': 'initial_command' in params,
                'rawPromptInArgv': raw_prompt in '\n'.join(args),
            }],
        }, handle, indent=2)
    print(json.dumps({'workspace_ref': 'workspace:stub', 'workspace_id': 'WORKSPACE-STUB'}))
    sys.exit(0)
if method == 'surface.create':
    cwd = params['working_directory']
    command = params['initial_command']
    result = subprocess.run(command, cwd=cwd, shell=True, executable='/bin/zsh')
    try:
        with open(path) as handle:
            receipt = json.load(handle)
    except FileNotFoundError:
        receipt = {'calls': []}
    receipt['calls'].append({
        'argv': args,
        'commandVerb': args[:2],
        'workspace': params.get('workspace_id'),
        'cwd': cwd,
        'focus': params.get('focus'),
        'tmuxStartMatchesInitial': params.get('tmux_start_command') == command,
        'rawPromptInArgv': raw_prompt in '\n'.join(args),
        'rawPromptInCommand': raw_prompt in command,
        'wrapperExitCode': result.returncode,
    })
    with open(path, 'w') as handle:
        json.dump(receipt, handle, indent=2)
    sys.exit(result.returncode)
raise SystemExit(f'unexpected method: {method}')
"#,
        )
        .expect("write cmux stub");
        std::fs::write(
            &codex_stub_path,
            r#"#!/usr/bin/env python3
import hashlib
import json
import os
import sys

prompt = sys.argv[4]
with open(os.environ['CODEX_STUB_RECEIPT'], 'w') as handle:
    json.dump({
        'pwd': os.getcwd(),
        'argv': sys.argv[1:],
        'promptChars': len(prompt),
        'promptSha256': hashlib.sha256(prompt.encode()).hexdigest(),
        'promptRepr': repr(prompt),
    }, handle, indent=2)
"#,
        )
        .expect("write codex stub");
        set_file_mode(&cmux_stub_path, 0o700).expect("chmod cmux stub");
        set_file_mode(&codex_stub_path, 0o700).expect("chmod codex stub");

        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(handoff_receipt_path.to_string_lossy().to_string()),
            ),
            (
                CMUX_BINARY_ENV,
                Some(cmux_stub_path.to_string_lossy().to_string()),
            ),
            (
                CODEX_BINARY_ENV,
                Some(codex_stub_path.to_string_lossy().to_string()),
            ),
            (
                "CMUX_STUB_RECEIPT",
                Some(cmux_receipt_path.to_string_lossy().to_string()),
            ),
            (
                "CODEX_STUB_RECEIPT",
                Some(codex_receipt_path.to_string_lossy().to_string()),
            ),
            ("RAW_PROMPT", Some(prompt.to_string())),
        ]);

        let payload = AgentPromptHandoffPayload {
            source: AgentPromptHandoffSource::AgentChatComposer,
            adapter_id: AgentPromptHandoffAdapterId::CmuxCodex,
            raw_input: prompt.to_string(),
            prompt: prompt.to_string(),
            cwd: project_dir.clone(),
            model_id: Some("gpt-5.1-codex".to_string()),
            profile_id: Some("script-kit".to_string()),
            context_part_count: 2,
            prompt_builder_segment_count: 3,
            warnings: Vec::new(),
        };
        let receipt = launch_prompt_handoff(&payload).expect("launch handoff");
        assert!(!receipt.dry_run);
        assert!(receipt.spawned);
        assert!(receipt.pid.is_some());
        assert!(receipt.prompt_file_created);
        assert!(receipt.script_file_created);
        assert_eq!(
            receipt.command_kind,
            "cmux_workspace_surface_create_initial_command"
        );
        assert_eq!(receipt.prompt_sha256, sha256_hex(prompt));

        wait_for_file(&codex_receipt_path, Duration::from_secs(5)).expect("codex receipt");
        wait_for_file_containing(
            &cmux_receipt_path,
            "\"surface.create\"",
            Duration::from_secs(5),
        )
        .expect("cmux receipt");
        wait_for_file(&handoff_receipt_path, Duration::from_secs(5)).expect("handoff receipt");

        let cmux_receipt = std::fs::read_to_string(&cmux_receipt_path).expect("cmux receipt");
        assert!(cmux_receipt.contains("\"workspace.create\""));
        assert!(cmux_receipt.contains("\"surface.create\""));
        assert!(cmux_receipt.contains(&format!("\"cwd\": \"{}\"", project_dir.to_string_lossy())));
        assert!(cmux_receipt.contains("\"focus\": true"));
        assert!(cmux_receipt.contains("\"eagerLoadTerminal\": true"));
        assert!(cmux_receipt.contains("\"hasInitialCommand\": false"));
        assert!(cmux_receipt.contains("\"workspace\": \"workspace:stub\""));
        assert!(cmux_receipt.contains("\"tmuxStartMatchesInitial\": true"));
        assert!(cmux_receipt.contains("\"rawPromptInArgv\": false"));
        assert!(cmux_receipt.contains("\"rawPromptInCommand\": false"));
        assert!(cmux_receipt.contains("\"wrapperExitCode\": 0"));

        let codex_receipt = std::fs::read_to_string(&codex_receipt_path).expect("codex receipt");
        let canonical_project_dir =
            std::fs::canonicalize(&project_dir).expect("canonical project dir");
        assert!(codex_receipt.contains(&format!(
            "\"pwd\": \"{}\"",
            canonical_project_dir.to_string_lossy()
        )));
        assert!(codex_receipt.contains("\"--cd\""));
        assert!(codex_receipt.contains(&format!("\"{}\"", project_dir.to_string_lossy())));
        assert!(codex_receipt.contains("\"--\""));
        assert!(codex_receipt.contains(&format!("\"promptChars\": {}", prompt.chars().count())));
        assert!(codex_receipt.contains(&format!("\"promptSha256\": \"{}\"", sha256_hex(prompt))));
        assert!(codex_receipt.contains("trailing newline\\\\n'"));

        let handoff_receipt =
            std::fs::read_to_string(&handoff_receipt_path).expect("handoff receipt");
        assert!(handoff_receipt.contains("\"spawned\": true"));
        assert!(handoff_receipt.contains(&format!("\"promptSha256\": \"{}\"", sha256_hex(prompt))));
        assert!(!handoff_receipt.contains(prompt));
    }

    #[test]
    fn cmux_codex_rejects_nul_prompt_before_launch() {
        let payload = AgentPromptHandoffPayload {
            source: AgentPromptHandoffSource::AgentChatComposer,
            adapter_id: AgentPromptHandoffAdapterId::CmuxCodex,
            raw_input: "contains nul".to_string(),
            prompt: "contains\0nul".to_string(),
            cwd: PathBuf::from("/tmp"),
            model_id: None,
            profile_id: None,
            context_part_count: 0,
            prompt_builder_segment_count: 0,
            warnings: Vec::new(),
        };

        assert!(matches!(
            launch_prompt_handoff(&payload),
            Err(AgentPromptHandoffError::UnsupportedPrompt(reason))
                if reason.contains("NUL bytes")
        ));
    }

    struct RichPromptContextFixture {
        temp_dir: tempfile::TempDir,
        raw_prompt: String,
        parts: Vec<AiContextPart>,
        expected_fragments: Vec<String>,
    }

    impl RichPromptContextFixture {
        fn new() -> Self {
            let temp_dir = tempfile::tempdir().expect("temp dir");
            let text_file_path = temp_dir.path().join("brief.txt");
            let image_file_path = temp_dir.path().join("screenshot.png");
            let skill_path = temp_dir.path().join("SKILL.md");

            std::fs::write(
                &text_file_path,
                "PROMPT_EXPORT_FILE_REFERENCE_SENTINEL\nUse this local file.",
            )
            .expect("write text file");
            std::fs::write(&image_file_path, [0x89, b'P', b'N', b'G', 0xff])
                .expect("write binary screenshot");
            std::fs::write(
                &skill_path,
                "# Prompt Export Skill\nPROMPT_EXPORT_SKILL_FILE_SENTINEL",
            )
            .expect("write skill");

            let focused_target = crate::ai::tab_context::TabAiTargetContext {
                source: "ClipboardHistory".to_string(),
                kind: "clipboard_entry".to_string(),
                semantic_id: "clipboard-entry:PROMPT_EXPORT_FOCUSED_TARGET_SENTINEL".to_string(),
                label: "Focused clipboard entry".to_string(),
                metadata: Some(serde_json::json!({
                    "preview": "PROMPT_EXPORT_FOCUSED_TARGET_METADATA_SENTINEL",
                    "contentType": "text",
                })),
            };

            let parts = vec![
                AiContextPart::ResourceUri {
                    uri: "kit://context/schema".to_string(),
                    label: "Context Schema".to_string(),
                },
                AiContextPart::FilePath {
                    path: text_file_path.to_string_lossy().to_string(),
                    label: "brief.txt".to_string(),
                },
                AiContextPart::FilePath {
                    path: image_file_path.to_string_lossy().to_string(),
                    label: "screenshot.png".to_string(),
                },
                AiContextPart::SkillFile {
                    path: skill_path.to_string_lossy().to_string(),
                    label: "/prompt-export-skill".to_string(),
                    skill_name: "Prompt Export Skill".to_string(),
                    owner_label: "Script Kit Test".to_string(),
                    slash_name: "prompt-export-skill".to_string(),
                },
                AiContextPart::FocusedTarget {
                    target: focused_target,
                    label: "Focused clipboard entry".to_string(),
                },
                AiContextPart::AmbientContext {
                    label: "PROMPT_EXPORT_AMBIENT_DISPLAY_ONLY_SENTINEL".to_string(),
                },
                AiContextPart::TextBlock {
                    label: "Clipboard history text".to_string(),
                    source: "clipboard-history://entry/PROMPT_EXPORT_CLIPBOARD_SOURCE_SENTINEL"
                        .to_string(),
                    text: "PROMPT_EXPORT_CLIPBOARD_HISTORY_SENTINEL".to_string(),
                    mime_type: Some("text/plain".to_string()),
                },
                AiContextPart::TextBlock {
                    label: "Browser tab text".to_string(),
                    source: "browser-tab://PROMPT_EXPORT_BROWSER_TAB_SOURCE_SENTINEL".to_string(),
                    text: "PROMPT_EXPORT_BROWSER_TAB_SENTINEL".to_string(),
                    mime_type: Some("text/uri-list".to_string()),
                },
            ];

            let expected_fragments = vec![
                "kit://context/schema".to_string(),
                "PROMPT_EXPORT_FILE_REFERENCE_SENTINEL".to_string(),
                image_file_path.to_string_lossy().to_string(),
                "unreadable=\"true\"".to_string(),
                "PROMPT_EXPORT_SKILL_FILE_SENTINEL".to_string(),
                "PROMPT_EXPORT_FOCUSED_TARGET_SENTINEL".to_string(),
                "PROMPT_EXPORT_FOCUSED_TARGET_METADATA_SENTINEL".to_string(),
                "PROMPT_EXPORT_CLIPBOARD_HISTORY_SENTINEL".to_string(),
                "PROMPT_EXPORT_BROWSER_TAB_SENTINEL".to_string(),
                "Rewrite the prompt export proof with every context source".to_string(),
            ];

            Self {
                temp_dir,
                raw_prompt: "/rewrite Rewrite the prompt export proof with every context source"
                    .to_string(),
                parts,
                expected_fragments,
            }
        }
    }

    fn compile_fixture_payload(fixture: &RichPromptContextFixture) -> AgentPromptHandoffPayload {
        let parse = crate::spine::parse_spine(&fixture.raw_prompt);
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);
        assert!(plan.should_submit_to_chat(), "fixture prompt should submit");

        compile_handoff_payload_from_spine_plan(
            AgentPromptHandoffAdapterId::CmuxCodex,
            fixture.raw_prompt.clone(),
            fixture.temp_dir.path().to_path_buf(),
            Some("gpt-test".to_string()),
            fixture.parts.clone(),
            plan,
        )
        .expect("compile rich fixture")
    }

    fn assert_compiled_prompt_contains_all_context_fingerprints(
        prompt: &str,
        fixture: &RichPromptContextFixture,
    ) {
        for fragment in &fixture.expected_fragments {
            assert!(
                prompt.contains(fragment),
                "compiled prompt missing {fragment:?}: {prompt}"
            );
        }
    }

    fn assert_export_receipt_matches_payload(
        receipt: &AgentPromptExportReceipt,
        payload: &AgentPromptHandoffPayload,
        export_kind: &str,
        command_kind: &str,
    ) {
        assert_eq!(receipt.export_kind, export_kind);
        assert_eq!(receipt.command_kind, command_kind);
        assert_eq!(receipt.prompt_sha256, sha256_hex(&payload.prompt));
        assert_eq!(receipt.prompt_chars, payload.prompt.chars().count());
        assert_eq!(receipt.context_part_count, payload.context_part_count);
        assert_eq!(
            receipt.prompt_builder_segment_count,
            payload.prompt_builder_segment_count
        );
    }

    fn assert_ai_context_part_variant_coverage(parts: &[AiContextPart]) {
        let mut names = parts
            .iter()
            .map(ai_context_part_variant_name)
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        assert_eq!(
            names,
            vec![
                "ambientContext",
                "filePath",
                "focusedTarget",
                "resourceUri",
                "skillFile",
                "textBlock"
            ]
        );
    }

    fn ai_context_part_variant_name(part: &AiContextPart) -> &'static str {
        match part {
            AiContextPart::ResourceUri { .. } => "resourceUri",
            AiContextPart::FilePath { .. } => "filePath",
            AiContextPart::SkillFile { .. } => "skillFile",
            AiContextPart::FocusedTarget { .. } => "focusedTarget",
            AiContextPart::AmbientContext { .. } => "ambientContext",
            AiContextPart::TextBlock { .. } => "textBlock",
        }
    }

    fn wait_for_file(path: &Path, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if path.exists() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(format!("timed out waiting for {}", path.display()))
    }

    fn wait_for_file_containing(
        path: &Path,
        expected: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if contents.contains(expected) {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(format!(
            "timed out waiting for {} to contain {expected:?}",
            path.display()
        ))
    }

    fn test_payload(prompt: &str) -> AgentPromptHandoffPayload {
        AgentPromptHandoffPayload {
            source: AgentPromptHandoffSource::AgentChatComposer,
            adapter_id: AgentPromptHandoffAdapterId::CmuxCodex,
            raw_input: prompt.to_string(),
            prompt: prompt.to_string(),
            cwd: PathBuf::from("/tmp/script-kit-prompt-export-test"),
            model_id: Some("gpt-test".to_string()),
            profile_id: None,
            context_part_count: 0,
            prompt_builder_segment_count: 0,
            warnings: Vec::new(),
        }
    }

    struct HandoffEnvGuard(Vec<(&'static str, Option<String>)>);

    impl HandoffEnvGuard {
        fn set<const N: usize>(values: [(&'static str, Option<String>); N]) -> Self {
            let mut previous = Vec::with_capacity(N);
            for (name, value) in values {
                previous.push((name, std::env::var(name).ok()));
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
            Self(previous)
        }
    }

    impl Drop for HandoffEnvGuard {
        fn drop(&mut self) {
            restore_handoff_env(std::mem::take(&mut self.0));
        }
    }

    fn restore_handoff_env(previous: Vec<(&'static str, Option<String>)>) {
        for (name, value) in previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}
