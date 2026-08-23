#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_chat::ui::catalog::{
        AgentChatAgentAuthState, AgentChatAgentCatalogEntry, AgentChatAgentConfigState,
        AgentChatAgentInstallState, AgentChatAgentSource,
    };
    use tempfile::tempdir;

    #[test]
    fn cwd_recents_push_dedupe_cap_and_ignore_default() {
        let mut file = AgentChatCwdRecentsFile::default();
        let default = Path::new("/tmp/default");

        assert!(!file.push_recent_for_profile("general", default.to_path_buf(), Some(default)));
        for name in ["one", "two", "three", "four", "five"] {
            assert!(file.push_recent_for_profile(
                "general",
                PathBuf::from(format!("/tmp/{name}")),
                Some(default),
            ));
        }
        assert!(file.push_recent_for_profile("general", PathBuf::from("/tmp/six"), Some(default),));
        assert!(file.push_recent_for_profile(
            "general",
            PathBuf::from("/tmp/three"),
            Some(default),
        ));

        assert_eq!(
            file.recents_for_profile("general"),
            vec![
                PathBuf::from("/tmp/three"),
                PathBuf::from("/tmp/six"),
                PathBuf::from("/tmp/five"),
                PathBuf::from("/tmp/four"),
                PathBuf::from("/tmp/two"),
            ]
        );
    }

    #[test]
    fn cwd_recents_are_isolated_per_profile_and_absolute_only() {
        let mut file = AgentChatCwdRecentsFile::default();

        assert!(file.push_recent_for_profile("general", PathBuf::from("/tmp/general"), None));
        assert!(file.push_recent_for_profile("brain", PathBuf::from("/tmp/brain"), None));
        assert!(!file.push_recent_for_profile("general", PathBuf::from("relative"), None));

        assert_eq!(
            file.recents_for_profile("general"),
            vec![PathBuf::from("/tmp/general")]
        );
        assert_eq!(
            file.recents_for_profile("brain"),
            vec![PathBuf::from("/tmp/brain")]
        );
    }

    fn catalog_entry(id: &str, display_name: &str) -> AgentChatAgentCatalogEntry {
        AgentChatAgentCatalogEntry {
            id: id.to_string().into(),
            display_name: display_name.to_string().into(),
            source: AgentChatAgentSource::BuiltIn,
            install_state: AgentChatAgentInstallState::Ready,
            auth_state: AgentChatAgentAuthState::Unknown,
            config_state: AgentChatAgentConfigState::Valid,
            install_hint: None,
            config_hint: None,
            supports_embedded_context: None,
            supports_image: None,
            last_session_ok: false,
            config: None,
        }
    }

    #[test]
    fn round_trip_minimal_config() {
        let json = r#"{
            "id": "test-agent",
            "displayName": "Test Agent",
            "command": "test-agent"
        }"#;
        let config: AgentChatAgentConfig =
            serde_json::from_str(json).expect("minimal config should parse");
        assert_eq!(config.id, "test-agent");
        assert_eq!(config.display_name, "Test Agent");
        assert_eq!(config.command, "test-agent");
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
        assert!(config.models.is_empty());
    }

    #[test]
    fn round_trip_full_config() {
        let json = r#"{
            "id": "claude-code",
            "displayName": "Claude Code (Agent Chat)",
            "command": "claude-agent_chat",
            "args": ["--profile", "default"],
            "env": {"CLAUDE_CONFIG_DIR": "/tmp/claude"},
            "models": [
                {"id": "claude-sonnet-4-6", "displayName": "Claude Sonnet 4.6", "contextWindow": 200000}
            ]
        }"#;
        let config: AgentChatAgentConfig =
            serde_json::from_str(json).expect("full config should parse");
        assert_eq!(config.command, "claude-agent_chat");
        assert_eq!(config.args, vec!["--profile", "default"]);
        assert_eq!(
            config.env.get("CLAUDE_CONFIG_DIR"),
            Some(&"/tmp/claude".to_string())
        );
        assert_eq!(config.models.len(), 1);
        assert_eq!(config.models[0].id, "claude-sonnet-4-6");
    }

    #[test]
    fn provider_id_and_display_name() {
        let config = AgentChatAgentConfig {
            id: "opencode".into(),
            display_name: "OpenCode".into(),
            command: "opencode".into(),
            args: vec!["agent_chat".into()],
            env: HashMap::new(),
            models: vec![],
            install: None,
            auth: None,
        };
        assert_eq!(config.provider_id(), "opencode");
        assert_eq!(config.display_name(), "OpenCode");
    }

    #[test]
    fn serialize_round_trip() {
        let config = AgentChatAgentConfig {
            id: "codex".into(),
            display_name: "Codex (Agent Chat)".into(),
            command: "codex-agent_chat".into(),
            args: vec![],
            env: HashMap::new(),
            models: vec![],
            install: None,
            auth: None,
        };
        let json = serde_json::to_string(&config).expect("should serialize");
        let back: AgentChatAgentConfig = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(back.id, config.id);
        assert_eq!(back.command, config.command);
    }

    #[test]
    fn starter_catalog_entries_include_common_agent_chat_agents() {
        let starters = starter_agent_chat_agent_configs();
        let ids = starters
            .iter()
            .map(|agent| agent.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["opencode", "codex-agent_chat"]);
    }

    #[test]
    fn codex_starter_uses_direct_adapter_command() {
        let codex = starter_agent_chat_agent_configs()
            .into_iter()
            .find(|agent| agent.id == "codex-agent_chat")
            .expect("codex-agent_chat starter");

        assert_eq!(codex.display_name, "Codex");
        assert_eq!(codex.command, CODEX_AGENT_CHAT_AGENT_ID);
        assert!(codex.args.is_empty());
        assert!(codex.install.is_none());
        assert!(codex
            .auth
            .expect("codex-agent_chat auth hint")
            .summary
            .contains("OPENAI_API_KEY"));
    }

    #[test]
    fn agent_chat_catalog_refresh_merge_keeps_fresh_codex_and_snapshot_selection() {
        let fresh = vec![
            catalog_entry("opencode", "OpenCode"),
            catalog_entry("codex-agent_chat", "Codex"),
        ];
        let snapshot = vec![catalog_entry("opencode", "Stale OpenCode")];

        let merged = merge_agent_chat_agent_catalog_entries_with_snapshot(fresh, &snapshot);
        let ids = merged
            .iter()
            .map(|entry| entry.id.as_ref())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["opencode", "codex-agent_chat"]);
        assert_eq!(merged[0].display_name.as_ref(), "OpenCode");
    }

    #[test]
    fn legacy_codex_npx_config_normalizes_to_resolved_adapter_without_npx() {
        let mut codex = codex_agent_chat_agent_config();
        codex.command = "npx".into();
        codex.args = vec![
            "-y".into(),
            CODEX_AGENT_CHAT_NPX_PACKAGE.into(),
            "--verbose".into(),
        ];

        let normalized = normalize_codex_agent_chat_agent_config_with_path(
            codex,
            Some(PathBuf::from(
                "/Applications/Script Kit.app/Contents/MacOS/codex-agent_chat",
            )),
        );

        assert_eq!(
            normalized.command,
            "/Applications/Script Kit.app/Contents/MacOS/codex-agent_chat"
        );
        assert_eq!(normalized.args, vec!["--verbose"]);
        assert!(normalized.install.is_none());
    }

    #[test]
    fn legacy_codex_agent_chat_command_normalizes_to_resolved_adapter_without_npx() {
        let mut codex = codex_agent_chat_agent_config();
        codex.command = "codex-agent_chat".into();
        codex.args = vec!["--verbose".into()];

        let normalized = normalize_codex_agent_chat_agent_config_with_path(
            codex,
            Some(PathBuf::from(
                "/tmp/Script Kit.app/Contents/MacOS/codex-agent_chat",
            )),
        );

        assert_eq!(
            normalized.command,
            "/tmp/Script Kit.app/Contents/MacOS/codex-agent_chat"
        );
        assert_eq!(normalized.args, vec!["--verbose"]);
        assert!(normalized.install.is_none());
    }

    #[test]
    fn missing_adapter_does_not_normalize_to_npx_runtime() {
        let mut codex = codex_agent_chat_agent_config();
        codex.command =
            "/Users/example/dev/codex-agent_chat/target/release/codex-agent_chat".into();
        codex.args = Vec::new();

        let normalized = normalize_codex_agent_chat_agent_config_with_path(codex, None);

        assert_eq!(normalized.command, CODEX_AGENT_CHAT_AGENT_ID);
        assert!(normalized.args.is_empty());
        assert!(normalized.install.is_none());
    }

    #[test]
    fn codex_agent_chat_install_state_accepts_direct_adapter_only() {
        let codex = codex_agent_chat_agent_config();

        assert_eq!(
            install_state_from_probe(&codex, true, false, true, true),
            crate::ai::agent_chat::ui::catalog::AgentChatAgentInstallState::Unsupported
        );
        assert_eq!(
            install_state_from_probe(&codex, false, false, true, true),
            crate::ai::agent_chat::ui::catalog::AgentChatAgentInstallState::Unsupported
        );

        let mut legacy = codex_agent_chat_agent_config();
        legacy.command = "codex-agent_chat".into();
        legacy.args = Vec::new();
        assert_eq!(
            install_state_from_probe(&legacy, false, true, true, true),
            crate::ai::agent_chat::ui::catalog::AgentChatAgentInstallState::Ready
        );
        assert_eq!(
            install_state_from_probe(&legacy, false, true, false, true),
            crate::ai::agent_chat::ui::catalog::AgentChatAgentInstallState::Unsupported,
            "Codex Agent Chat adapter alone is not usable without the installed codex CLI"
        );
    }

    #[test]
    fn codex_default_probe_tracks_cli_and_adapter_separately() {
        let ready = codex_agent_chat_default_probe_state_from_parts(true, true, false, None);
        assert!(ready.codex_cli_ready);
        assert!(ready.npx_ready);
        assert!(!ready.codex_agent_chat_binary_ready);
        assert!(!ready.adapter_ready);
        assert!(!ready.launch_ready);
        assert!(!ready.should_be_implicit_codex_default);
        assert!(!ready.npx_runtime_fallback_enabled);

        let adapter_blocked =
            codex_agent_chat_default_probe_state_from_parts(true, false, false, None);
        assert!(adapter_blocked.codex_cli_ready);
        assert!(!adapter_blocked.adapter_ready);
        assert!(
            !adapter_blocked.should_be_implicit_codex_default,
            "local codex CLI must not own default setup when the Agent Chat adapter is missing"
        );

        let adapter_ready = codex_agent_chat_default_probe_state_from_parts(
            true,
            true,
            true,
            Some(CodexAgentChatAdapterSource::Path),
        );
        assert!(adapter_ready.codex_cli_ready);
        assert!(adapter_ready.npx_ready);
        assert!(adapter_ready.codex_agent_chat_binary_ready);
        assert!(adapter_ready.adapter_ready);
        assert!(adapter_ready.launch_ready);
        assert!(adapter_ready.should_be_implicit_codex_default);
        assert!(!adapter_ready.npx_runtime_fallback_enabled);

        let missing_cli = codex_agent_chat_default_probe_state_from_parts(
            false,
            true,
            true,
            Some(CodexAgentChatAdapterSource::Path),
        );
        assert!(missing_cli.adapter_ready);
        assert!(!missing_cli.launch_ready);
        assert!(
            !missing_cli.should_be_implicit_codex_default,
            "adapter discovery must not select Codex by default when the codex CLI is missing"
        );
    }

    #[test]
    fn sibling_codex_agent_chat_candidates_cover_release_before_debug() {
        let root = PathBuf::from("/Users/example/dev");
        let candidates = sibling_repo_codex_agent_chat_candidates(&root);
        assert_eq!(
            candidates,
            vec![
                PathBuf::from(
                    "/Users/example/dev/codex-agent_chat/target/release/codex-agent_chat"
                ),
                PathBuf::from("/Users/example/dev/codex-agent_chat/target/debug/codex-agent_chat"),
            ]
        );
    }

    #[test]
    fn merge_catalog_with_starters_preserves_existing_entries() {
        let mut file = crate::ai::agent_chat::ui::catalog::AgentChatAgentCatalogFile {
            schema_version:
                crate::ai::agent_chat::ui::catalog::AGENT_CHAT_AGENT_CATALOG_SCHEMA_VERSION,
            agents: vec![AgentChatAgentConfig {
                id: "opencode".into(),
                display_name: "OpenCode".into(),
                command: "opencode".into(),
                args: vec!["agent_chat".into()],
                env: HashMap::new(),
                models: vec![],
                install: None,
                auth: None,
            }],
        };

        let added = merge_catalog_with_starter_agents(&mut file);
        assert_eq!(added, 1);
        assert_eq!(file.agents[0].id, "opencode");
        assert!(file
            .agents
            .iter()
            .any(|agent| agent.id == "codex-agent_chat"));
    }

    #[test]
    fn prune_deprecated_google_cli_agents_removes_old_rows() {
        let deprecated_id = ["gemini", "cli"].join("-");
        let mut file = crate::ai::agent_chat::ui::catalog::AgentChatAgentCatalogFile {
            schema_version:
                crate::ai::agent_chat::ui::catalog::AGENT_CHAT_AGENT_CATALOG_SCHEMA_VERSION,
            agents: vec![AgentChatAgentConfig {
                id: deprecated_id,
                display_name: "Deprecated Google CLI".into(),
                command: "gemini".into(),
                args: vec!["--agent_chat".into()],
                env: HashMap::new(),
                models: vec![],
                install: None,
                auth: None,
            }],
        };

        let pruned = prune_deprecated_google_cli_agents(&mut file);
        assert_eq!(pruned, 1);
        assert!(file.agents.is_empty());
        assert!(file.agents.is_empty());
    }

    #[test]
    fn model_infos_defaults() {
        let config = AgentChatAgentConfig {
            id: "test-agent".into(),
            display_name: "Test".into(),
            command: "test".into(),
            args: vec![],
            env: HashMap::new(),
            models: vec![AgentChatModelEntry {
                id: "model-1".into(),
                display_name: None,
                context_window: None,
            }],
            install: None,
            auth: None,
        };
        let infos = config.model_infos();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, "model-1");
        assert_eq!(infos[0].display_name, "model-1");
        assert_eq!(infos[0].provider, "test-agent");
        assert!(infos[0].supports_streaming);
        assert_eq!(infos[0].context_window, DEFAULT_CONTEXT_WINDOW);
    }

    #[test]
    fn model_infos_explicit_values() {
        let config = AgentChatAgentConfig {
            id: "test-agent".into(),
            display_name: "Test Agent".into(),
            command: "test-agent".into(),
            args: vec![],
            env: HashMap::new(),
            models: vec![AgentChatModelEntry {
                id: "default".into(),
                display_name: Some("Test Agent Default".into()),
                context_window: Some(1_000_000),
            }],
            install: None,
            auth: None,
        };
        let infos = config.model_infos();
        assert_eq!(infos[0].display_name, "Test Agent Default");
        assert_eq!(infos[0].context_window, 1_000_000);
    }

    #[test]
    fn runtime_state_file_round_trip() {
        let json = r#"{
            "schemaVersion": 1,
            "agents": {
                "codex-agent_chat": {
                    "authState": "needsAuthentication",
                    "authMethods": ["chatgpt-login", "openai-api-key"],
                    "supportsEmbeddedContext": true,
                    "supportsImage": false,
                    "lastSessionOk": false
                }
            }
        }"#;
        let file: AgentChatAgentRuntimeStateFile =
            serde_json::from_str(json).expect("runtime state should parse");
        assert_eq!(file.schema_version, 1);
        assert_eq!(file.agents.len(), 1);
        let codex = file
            .agents
            .get("codex-agent_chat")
            .expect("codex-agent_chat entry");
        assert_eq!(
            codex.auth_state,
            Some(crate::ai::agent_chat::ui::catalog::AgentChatAgentAuthState::NeedsAuthentication)
        );
        assert_eq!(codex.auth_methods, vec!["chatgpt-login", "openai-api-key"]);
        assert_eq!(codex.supports_embedded_context, Some(true));
        assert_eq!(codex.supports_image, Some(false));
        assert!(!codex.last_session_ok);
    }

    #[test]
    fn runtime_state_file_defaults_on_missing_fields() {
        let json = r#"{"schemaVersion": 1, "agents": {"test": {}}}"#;
        let file: AgentChatAgentRuntimeStateFile =
            serde_json::from_str(json).expect("should parse with defaults");
        let state = file.agents.get("test").expect("test entry");
        assert!(state.auth_state.is_none());
        assert!(state.auth_methods.is_empty());
        assert!(state.supports_embedded_context.is_none());
        assert!(state.supports_image.is_none());
        assert!(!state.last_session_ok);
    }

    #[test]
    fn runtime_state_serialize_skips_none_fields() {
        let state = AgentChatAgentRuntimeState {
            auth_state: Some(
                crate::ai::agent_chat::ui::catalog::AgentChatAgentAuthState::Authenticated,
            ),
            auth_methods: vec!["terminal".to_string()],
            supports_embedded_context: None,
            supports_image: None,
            last_session_ok: true,
        };
        let json = serde_json::to_string(&state).expect("should serialize");
        assert!(!json.contains("supportsEmbeddedContext"));
        assert!(!json.contains("supportsImage"));
        assert!(json.contains("authenticated"));
    }

    #[test]
    fn runtime_state_merge_does_not_regress_auth_state() {
        let current = AgentChatAgentRuntimeState {
            auth_state: Some(
                crate::ai::agent_chat::ui::catalog::AgentChatAgentAuthState::Authenticated,
            ),
            auth_methods: vec!["chatgpt-login".to_string()],
            supports_embedded_context: Some(true),
            supports_image: Some(true),
            last_session_ok: true,
        };
        let stale_initialize = AgentChatAgentRuntimeState {
            auth_state: Some(crate::ai::agent_chat::ui::catalog::AgentChatAgentAuthState::Unknown),
            auth_methods: vec!["chatgpt-login".to_string(), "openai-api-key".to_string()],
            supports_embedded_context: Some(true),
            supports_image: Some(false),
            last_session_ok: false,
        };

        let merged = current.merged_with(&stale_initialize);
        assert_eq!(
            merged.auth_state,
            Some(crate::ai::agent_chat::ui::catalog::AgentChatAgentAuthState::Authenticated)
        );
        assert_eq!(
            merged.auth_methods,
            vec!["chatgpt-login".to_string(), "openai-api-key".to_string()]
        );
        assert_eq!(merged.supports_embedded_context, Some(true));
        assert_eq!(merged.supports_image, Some(false));
        assert!(merged.last_session_ok);
    }

    #[test]
    fn runtime_state_merge_allows_auth_required_to_override_unknown() {
        let current = AgentChatAgentRuntimeState {
            auth_state: Some(crate::ai::agent_chat::ui::catalog::AgentChatAgentAuthState::Unknown),
            auth_methods: vec!["chatgpt-login".to_string()],
            supports_embedded_context: Some(true),
            supports_image: Some(true),
            last_session_ok: false,
        };
        let auth_required = AgentChatAgentRuntimeState {
            auth_state: Some(
                crate::ai::agent_chat::ui::catalog::AgentChatAgentAuthState::NeedsAuthentication,
            ),
            auth_methods: vec!["chatgpt-login".to_string()],
            supports_embedded_context: Some(true),
            supports_image: Some(true),
            last_session_ok: false,
        };

        let merged = current.merged_with(&auth_required);
        assert_eq!(
            merged.auth_state,
            Some(crate::ai::agent_chat::ui::catalog::AgentChatAgentAuthState::NeedsAuthentication)
        );
        assert!(!merged.last_session_ok);
    }

    #[test]
    fn sync_script_kit_mcp_to_claude_preserves_unmanaged_servers() {
        let temp = tempdir().expect("temp dir");
        let claude_config_path = temp.path().join(".claude.json");
        let state_path = temp.path().join("claude-sync.json");

        let existing = serde_json::json!({
            "mcpServers": {
                "user-server": {
                    "type": "http",
                    "url": "https://example.com/mcp"
                },
                "old-script-kit": {
                    "type": "stdio",
                    "command": "old"
                }
            }
        });
        std::fs::write(
            &claude_config_path,
            serde_json::to_vec_pretty(&existing).expect("serialize existing config"),
        )
        .expect("write existing config");

        write_claude_managed_mcp_state(&state_path, &["old-script-kit".to_string()])
            .expect("seed sync state");

        let desired_servers = vec![(
            "linear".to_string(),
            serde_json::json!({
                "type": "http",
                "url": "https://mcp.linear.app/sse"
            }),
        )];

        sync_script_kit_mcp_to_claude_at(
            &desired_servers,
            &["linear".to_string()],
            &claude_config_path,
            &state_path,
        )
        .expect("sync MCP config");

        let synced = serde_json::from_slice::<Value>(
            &std::fs::read(&claude_config_path).expect("read synced config"),
        )
        .expect("parse synced config");
        let servers = synced["mcpServers"]
            .as_object()
            .expect("mcpServers object after sync");
        assert!(servers.contains_key("user-server"));
        assert!(servers.contains_key("linear"));
        assert!(!servers.contains_key("old-script-kit"));
    }

    #[test]
    fn sync_script_kit_mcp_to_claude_removes_state_when_empty() {
        let temp = tempdir().expect("temp dir");
        let claude_config_path = temp.path().join(".claude.json");
        let state_path = temp.path().join("claude-sync.json");

        let existing = serde_json::json!({
            "theme": "dark",
            "mcpServers": {
                "old-script-kit": {
                    "type": "stdio",
                    "command": "old"
                }
            }
        });
        std::fs::write(
            &claude_config_path,
            serde_json::to_vec_pretty(&existing).expect("serialize existing config"),
        )
        .expect("write existing config");

        write_claude_managed_mcp_state(&state_path, &["old-script-kit".to_string()])
            .expect("seed sync state");

        sync_script_kit_mcp_to_claude_at(&[], &[], &claude_config_path, &state_path)
            .expect("clear managed servers");

        let synced = serde_json::from_slice::<Value>(
            &std::fs::read(&claude_config_path).expect("read synced config"),
        )
        .expect("parse synced config");
        assert_eq!(synced["theme"], "dark");
        assert!(synced.get("mcpServers").is_none());
        assert!(!state_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_claude_mcp_credentials_are_owner_only_after_legacy_repair() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = tempdir().expect("isolated Claude credentials fixture");
        let config_path = fixture.path().join(".claude.json");
        let state_path = fixture.path().join("claude-sync.json");
        let existing = serde_json::json!({
            "mcpServers": {
                "personal": {
                    "type": "stdio",
                    "command": "synthetic-agent",
                    "env": { "OPENAI_API_KEY": "sk-private-existing-user-token" }
                },
                "previous-managed": { "type": "stdio", "command": "old" }
            }
        });
        std::fs::write(&config_path, serde_json::to_vec(&existing).unwrap()).unwrap();
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_claude_managed_mcp_state(&state_path, &["previous-managed".to_string()]).unwrap();
        std::fs::set_permissions(&state_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let desired = vec![(
            "managed".to_string(),
            serde_json::json!({
                "type": "http",
                "url": "https://synthetic.invalid/mcp",
                "headers": { "Authorization": "Bearer private-managed-token" }
            }),
        )];

        sync_script_kit_mcp_to_claude_at(
            &desired,
            &["managed".to_string()],
            &config_path,
            &state_path,
        )
        .expect("secure real Claude MCP synchronization");

        for path in [&config_path, &state_path] {
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let saved: Value = read_private_agent_chat_json(&config_path)
            .unwrap()
            .expect("saved private Claude config");
        assert_eq!(
            saved["mcpServers"]["personal"]["env"]["OPENAI_API_KEY"],
            "sk-private-existing-user-token"
        );
        assert_eq!(
            saved["mcpServers"]["managed"]["headers"]["Authorization"],
            "Bearer private-managed-token"
        );
        assert!(saved["mcpServers"].get("previous-managed").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_claude_rejects_symlinked_user_configuration() {
        use std::os::unix::fs::{symlink, PermissionsExt as _};

        let fixture = tempdir().expect("isolated symlinked Claude configuration fixture");
        let external = fixture.path().join("foreign.json");
        let planted = fixture.path().join(".claude.json");
        let state_path = fixture.path().join("claude-sync.json");
        let foreign = r#"{"apiKey":"foreign private credential"}"#;
        std::fs::write(&external, foreign).unwrap();
        std::fs::set_permissions(&external, std::fs::Permissions::from_mode(0o644)).unwrap();
        symlink(&external, &planted).unwrap();

        assert!(sync_script_kit_mcp_to_claude_at(&[], &[], &planted, &state_path).is_err());
        assert_eq!(std::fs::read_to_string(&external).unwrap(), foreign);
        assert_eq!(
            std::fs::metadata(&external).unwrap().permissions().mode() & 0o777,
            0o644
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_claude_rejects_symlinked_state_before_mutating_config() {
        use std::os::unix::fs::symlink;

        let fixture = tempdir().expect("isolated symlinked Claude sync state fixture");
        let config_path = fixture.path().join(".claude.json");
        let external = fixture.path().join("foreign-state.json");
        let planted = fixture.path().join("claude-sync.json");
        let original = r#"{"mcpServers":{"personal":{"env":{"TOKEN":"private"}}}}"#;
        std::fs::write(&config_path, original).unwrap();
        std::fs::write(&external, r#"{"schemaVersion":1,"managedServers":[]}"#).unwrap();
        symlink(&external, &planted).unwrap();

        assert!(sync_script_kit_mcp_to_claude_at(&[], &[], &config_path, &planted).is_err());
        assert_eq!(std::fs::read_to_string(config_path).unwrap(), original);
        assert_eq!(
            std::fs::read_to_string(external).unwrap(),
            r#"{"schemaVersion":1,"managedServers":[]}"#
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_agent_catalog_protects_user_environment_credentials() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = tempdir().expect("isolated private agent catalog fixture");
        let path = fixture.path().join("agents.json");
        let mut file = super::super::catalog::AgentChatAgentCatalogFile::default();
        file.agents.push(AgentChatAgentConfig {
            id: "private-agent".to_string(),
            display_name: "Private Agent".to_string(),
            command: "private-agent".to_string(),
            args: Vec::new(),
            env: HashMap::from([(
                "OPENAI_API_KEY".to_string(),
                "sk-private-catalog-token".to_string(),
            )]),
            models: Vec::new(),
            install: None,
            auth: None,
        });
        std::fs::write(&path, serde_json::to_vec(&file).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let (existed, starters, _, total) = ensure_agent_chat_agents_catalog_seeded_at(&path)
            .expect("seed actual private agent catalog owner");
        assert!(existed);
        assert_eq!(starters, 2);
        assert_eq!(total, 3);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let stored: super::super::catalog::AgentChatAgentCatalogFile =
            read_private_agent_chat_json(&path).unwrap().unwrap();
        let private = stored
            .agents
            .iter()
            .find(|entry| entry.id == "private-agent")
            .unwrap();
        assert_eq!(private.env["OPENAI_API_KEY"], "sk-private-catalog-token");
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_agent_catalog_rejects_foreign_symlink() {
        use std::os::unix::fs::symlink;

        let fixture = tempdir().expect("isolated symlinked agent catalog fixture");
        let external = fixture.path().join("foreign.json");
        let planted = fixture.path().join("agents.json");
        std::fs::write(&external, r#"{"private":"foreign provider token"}"#).unwrap();
        symlink(&external, &planted).unwrap();

        assert!(ensure_agent_chat_agents_catalog_seeded_at(&planted).is_err());
        assert_eq!(
            std::fs::read_to_string(external).unwrap(),
            r#"{"private":"foreign provider token"}"#
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_cwd_history_repairs_legacy_permissions_before_read() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = tempdir().expect("isolated private project history fixture");
        let path = fixture.path().join("cwd-recents.json");
        let mut recents = AgentChatCwdRecentsFile::default();
        assert!(recents.push_recent_for_profile(
            "private-client",
            PathBuf::from("/Users/private/medical-project"),
            None,
        ));
        persist_agent_chat_cwd_recents_file_at(&path, &recents)
            .expect("persist real private project MRU");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let restored =
            load_agent_chat_cwd_recents_file_at(&path).expect("repair older project history");
        assert_eq!(
            restored.recents_for_profile("private-client"),
            vec![PathBuf::from("/Users/private/medical-project")]
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_cwd_history_rejects_symlinked_read_and_write() {
        use std::os::unix::fs::symlink;

        let fixture = tempdir().expect("isolated symlinked project history fixture");
        let external = fixture.path().join("foreign.json");
        let planted = fixture.path().join("cwd-recents.json");
        std::fs::write(
            &external,
            "do not read or overwrite foreign project history",
        )
        .unwrap();
        symlink(&external, &planted).unwrap();

        assert!(load_agent_chat_cwd_recents_file_at(&planted).is_err());
        assert!(persist_agent_chat_cwd_recents_file_at(
            &planted,
            &AgentChatCwdRecentsFile::default(),
        )
        .is_err());
        assert_eq!(
            std::fs::read_to_string(external).unwrap(),
            "do not read or overwrite foreign project history"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_runtime_state_is_owner_only_and_preserves_auth() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = tempdir().expect("isolated private authentication state fixture");
        let path = fixture.path().join("agent-runtime-state.json");
        let authenticated = AgentChatAgentRuntimeState {
            auth_state: Some(AgentChatAgentAuthState::Authenticated),
            auth_methods: vec!["private-login".to_string()],
            supports_embedded_context: Some(true),
            supports_image: Some(true),
            last_session_ok: true,
        };
        persist_agent_chat_agent_runtime_state_at(&path, "private-agent", &authenticated)
            .expect("persist real authentication state");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let stale = AgentChatAgentRuntimeState {
            auth_state: Some(AgentChatAgentAuthState::Unknown),
            ..AgentChatAgentRuntimeState::default()
        };
        let merged = persist_agent_chat_agent_runtime_state_at(&path, "private-agent", &stale)
            .expect("repair legacy auth state and preserve known facts");
        assert_eq!(
            merged.auth_state,
            Some(AgentChatAgentAuthState::Authenticated)
        );
        assert_eq!(merged.auth_methods, vec!["private-login"]);
        assert!(merged.last_session_ok);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn private_agent_chat_config_concurrent_runtime_writers_preserve_every_agent() {
        use std::sync::{Arc, Barrier};

        let fixture = tempdir().expect("isolated concurrent agent-state fixture");
        let path = Arc::new(fixture.path().join("agent-runtime-state.json"));
        let start = Arc::new(Barrier::new(8));
        let workers = (0..8)
            .map(|index| {
                let path = Arc::clone(&path);
                let start = Arc::clone(&start);
                std::thread::spawn(move || {
                    let next = AgentChatAgentRuntimeState {
                        auth_state: Some(AgentChatAgentAuthState::Authenticated),
                        auth_methods: vec![format!("private-method-{index}")],
                        ..AgentChatAgentRuntimeState::default()
                    };
                    start.wait();
                    persist_agent_chat_agent_runtime_state_at(
                        &path,
                        &format!("agent-{index}"),
                        &next,
                    )
                    .expect("serialized production auth-state write");
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }

        let saved: AgentChatAgentRuntimeStateFile =
            read_private_agent_chat_json(&path).unwrap().unwrap();
        assert_eq!(saved.agents.len(), 8);
        for index in 0..8 {
            let state = saved.agents.get(&format!("agent-{index}")).unwrap();
            assert_eq!(state.auth_methods, vec![format!("private-method-{index}")]);
        }
    }

    #[cfg(unix)]
    #[test]
    fn private_agent_chat_config_runtime_state_refuses_symlinks_and_malformed_history() {
        use std::os::unix::fs::symlink;

        let fixture = tempdir().expect("isolated hostile authentication state fixture");
        let external = fixture.path().join("foreign.json");
        let planted = fixture.path().join("agent-runtime-state.json");
        std::fs::write(&external, "never mutate foreign authentication state").unwrap();
        symlink(&external, &planted).unwrap();
        let next = AgentChatAgentRuntimeState::default();
        assert!(persist_agent_chat_agent_runtime_state_at(&planted, "agent", &next).is_err());
        assert_eq!(
            std::fs::read_to_string(&external).unwrap(),
            "never mutate foreign authentication state"
        );

        let malformed = fixture.path().join("malformed.json");
        std::fs::write(&malformed, "{ user data that must never be overwritten").unwrap();
        assert!(persist_agent_chat_agent_runtime_state_at(&malformed, "agent", &next).is_err());
        assert_eq!(
            std::fs::read_to_string(malformed).unwrap(),
            "{ user data that must never be overwritten"
        );
    }

    #[test]
    fn private_agent_chat_config_failure_logs_hide_paths_provider_errors_and_profile_names() {
        use std::sync::Arc;

        #[derive(Clone)]
        struct EventWriter(Arc<Mutex<Vec<u8>>>);

        impl std::io::Write for EventWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let path = Path::new("/Users/private/medical-client/auth.json");
        let error = anyhow::anyhow!("provider rejected sk-private-secret bearer token");
        let owner = "private-client-project";
        let expected_path = crate::logging::log_private_user_value(&path.to_string_lossy());
        let expected_error = crate::logging::log_private_user_value(&error.to_string());
        let expected_owner = crate::logging::log_private_user_value(owner);
        let captured = Arc::new(Mutex::new(Vec::new()));
        let writer = Arc::clone(&captured);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || EventWriter(Arc::clone(&writer)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            log_private_agent_chat_state_failure(
                "agent_chat_agent_runtime_state_persist_failed",
                path,
                &error,
                Some(owner),
            );
        });

        let raw = String::from_utf8(captured.lock().unwrap().clone()).unwrap();
        for secret in [
            "medical-client",
            "sk-private-secret",
            "private-client-project",
        ] {
            assert!(
                !raw.contains(secret),
                "private Agent Chat event leaked {secret}"
            );
        }
        let event: Value = serde_json::from_str(raw.trim()).unwrap();
        assert_eq!(event["fields"]["path_sha256"], expected_path.sha256);
        assert_eq!(event["fields"]["error_sha256"], expected_error.sha256);
        assert_eq!(event["fields"]["owner_sha256"], expected_owner.sha256);
    }
}
