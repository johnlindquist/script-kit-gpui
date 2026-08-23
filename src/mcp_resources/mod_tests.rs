#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Helper to wrap Vec<Script> into Vec<Arc<Script>> for tests
    fn wrap_scripts(scripts: Vec<Script>) -> Vec<Arc<Script>> {
        scripts.into_iter().map(Arc::new).collect()
    }

    /// Helper to wrap Vec<Scriptlet> into Vec<Arc<Scriptlet>> for tests
    fn wrap_scriptlets(scriptlets: Vec<Scriptlet>) -> Vec<Arc<Scriptlet>> {
        scriptlets.into_iter().map(Arc::new).collect()
    }

    fn provider_json_test_lock() -> &'static std::sync::Mutex<()> {
        crate::test_utils::PROVIDER_JSON_TEST_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    fn unique_notes_resource_token(prefix: &str) -> String {
        format!("{}_{}", prefix, uuid::Uuid::new_v4().simple())
    }

    // =======================================================
    // TDD Tests - Written FIRST per spec requirements
    // =======================================================

    /// Helper to create a test script
    fn test_script(name: &str, description: Option<&str>) -> Script {
        Script {
            name: name.to_string(),
            path: PathBuf::from(format!(
                "/test/{}.ts",
                name.to_lowercase().replace(' ', "-")
            )),
            extension: "ts".to_string(),
            description: description.map(|s| s.to_string()),
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: None,
            plugin_id: String::new(),
            plugin_title: None,
            kit_name: None,
            body: None,
        }
    }

    /// Helper to create a test scriptlet
    fn test_scriptlet(name: &str, tool: &str, description: Option<&str>) -> Scriptlet {
        Scriptlet {
            icon: None,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            code: "echo test".to_string(),
            tool: tool.to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: String::new(),
            plugin_title: None,
            file_path: None,
            command: None,
            alias: None,
        }
    }

    #[test]
    fn test_resources_list_includes_all() {
        // REQUIREMENT: resources/list returns the full MCP resource registry.
        let resources = get_resource_definitions();

        assert_eq!(
            resources.len(),
            29,
            "Resource registry count should be updated when new MCP resources land"
        );

        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(uris.contains(&"kit://state"), "Should include kit://state");
        assert!(uris.contains(&"kit://notes"), "Should include kit://notes");
        assert!(uris.contains(&"kit://brain"), "Should include kit://brain");
        assert!(uris.contains(&"kit://audit"), "Should include kit://audit");
        assert!(uris.contains(&"scripts://"), "Should include scripts://");
        assert!(
            uris.contains(&"scriptlets://"),
            "Should include scriptlets://"
        );
        assert!(
            uris.contains(&"kit://transactions/latest"),
            "Should include kit://transactions/latest"
        );
        assert!(
            uris.contains(&"kit://transactions/schema"),
            "Should include kit://transactions/schema"
        );

        // Verify all have required fields
        for resource in &resources {
            assert!(!resource.name.is_empty(), "Resource should have a name");
            assert!(
                resource.mime_type == "application/json"
                    || resource.mime_type == "text/plain"
                    || resource.mime_type == "text/markdown",
                "Should be JSON, text, or markdown mime type, got: {}",
                resource.mime_type
            );
            assert!(resource.description.is_some(), "Should have a description");
        }
    }

    #[test]
    fn brain_resource_description_lists_provenance_reads() {
        let resources = get_resource_definitions();
        let brain = resources
            .iter()
            .find(|resource| resource.uri == "kit://brain")
            .expect("brain resource definition");
        let description = brain.description.as_deref().unwrap_or("");
        assert!(description.contains("format=json"));
        assert!(description.contains("kit://brain/doc"));
        assert!(description.contains("kit://brain/docs"));
    }

    #[test]
    fn test_scripts_resource_read() {
        // REQUIREMENT: scripts:// returns array of script metadata
        let scripts = wrap_scripts(vec![
            test_script("My Script", Some("Does something")),
            test_script("Another Script", None),
        ]);

        let result = read_resource("scripts://", &scripts, &[], None);
        assert!(result.is_ok(), "Should successfully read scripts resource");

        let content = result.unwrap();
        assert_eq!(content.uri, "scripts://");
        assert_eq!(content.mime_type, "application/json");

        // Parse the JSON and verify structure
        let parsed: Vec<ScriptResourceEntry> =
            serde_json::from_str(&content.text).expect("Should be valid JSON array");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "My Script");
        assert_eq!(parsed[0].description, Some("Does something".to_string()));
        assert_eq!(parsed[1].name, "Another Script");
        assert_eq!(parsed[1].description, None);
    }

    #[test]
    fn test_scriptlets_resource_read() {
        // REQUIREMENT: scriptlets:// returns array of scriptlet metadata
        let scriptlets = wrap_scriptlets(vec![
            test_scriptlet("Open URL", "open", Some("Opens a URL")),
            test_scriptlet("Paste Text", "paste", None),
        ]);

        let result = read_resource("scriptlets://", &[], &scriptlets, None);
        assert!(
            result.is_ok(),
            "Should successfully read scriptlets resource"
        );

        let content = result.unwrap();
        assert_eq!(content.uri, "scriptlets://");
        assert_eq!(content.mime_type, "application/json");

        // Parse the JSON and verify structure
        let parsed: Vec<ScriptletResourceEntry> =
            serde_json::from_str(&content.text).expect("Should be valid JSON array");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "Open URL");
        assert_eq!(parsed[0].tool, "open");
        assert_eq!(parsed[0].description, Some("Opens a URL".to_string()));
        assert_eq!(parsed[1].name, "Paste Text");
        assert_eq!(parsed[1].tool, "paste");
    }

    #[test]
    fn test_state_resource_read() {
        // REQUIREMENT: kit://state returns current app state
        let app_state = AppStateResource {
            visible: true,
            focused: true,
            script_count: 10,
            scriptlet_count: 5,
            filter_text: Some("test".to_string()),
            selected_index: Some(3),
        };

        let result = read_resource("kit://state", &[], &[], Some(&app_state));
        assert!(result.is_ok(), "Should successfully read state resource");

        let content = result.unwrap();
        assert_eq!(content.uri, "kit://state");
        assert_eq!(content.mime_type, "application/json");

        // Parse and verify
        let parsed: AppStateResource =
            serde_json::from_str(&content.text).expect("Should be valid JSON");

        assert!(parsed.visible);
        assert!(parsed.focused);
        assert_eq!(parsed.script_count, 10);
        assert_eq!(parsed.scriptlet_count, 5);
        assert_eq!(parsed.filter_text, Some("test".to_string()));
        assert_eq!(parsed.selected_index, Some(3));
    }

    #[test]
    fn test_state_resource_read_default() {
        // When no app state is provided, should return defaults
        let result = read_resource("kit://state", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: AppStateResource = serde_json::from_str(&content.text).unwrap();

        assert!(!parsed.visible);
        assert!(!parsed.focused);
        assert_eq!(parsed.script_count, 0);
        assert_eq!(parsed.scriptlet_count, 0);
        assert_eq!(parsed.filter_text, None);
        assert_eq!(parsed.selected_index, None);
    }

    #[test]
    fn test_unknown_resource_returns_error() {
        // REQUIREMENT: Unknown URI returns error
        let result = read_resource("unknown://resource", &[], &[], None);

        assert!(result.is_err(), "Unknown resource should return error");
        let error = result.unwrap_err();
        assert!(
            error.contains("Resource not found"),
            "Error should mention resource not found"
        );
        assert!(
            error.contains("unknown://resource"),
            "Error should include the URI"
        );
    }

    #[test]
    fn test_resource_content_to_value() {
        let content = ResourceContent {
            uri: "test://uri".to_string(),
            mime_type: "application/json".to_string(),
            text: r#"{"foo":"bar"}"#.to_string(),
        };

        let value = resource_content_to_value(content);

        // Should have contents array
        let contents = value.get("contents").and_then(|c| c.as_array());
        assert!(contents.is_some());

        let contents = contents.unwrap();
        assert_eq!(contents.len(), 1);

        let first = &contents[0];
        assert_eq!(
            first.get("uri").and_then(|u| u.as_str()),
            Some("test://uri")
        );
        assert_eq!(
            first.get("mimeType").and_then(|m| m.as_str()),
            Some("application/json")
        );
    }

    #[test]
    fn test_resource_list_to_value() {
        let resources = get_resource_definitions();
        let value = resource_list_to_value(&resources);

        // Should have resources array
        let resource_array = value.get("resources").and_then(|r| r.as_array());
        assert!(resource_array.is_some());

        let resource_array = resource_array.unwrap();
        assert_eq!(resource_array.len(), resources.len());

        // First resource should have expected fields
        let first = &resource_array[0];
        assert!(first.get("uri").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("mimeType").is_some());
    }

    // =======================================================
    // Additional Unit Tests
    // =======================================================

    #[test]
    fn test_script_resource_entry_from_script() {
        use crate::schema_parser::{FieldDef, FieldType, Schema};
        use std::collections::HashMap;

        // Script without schema
        let script_no_schema = test_script("No Schema", Some("Test"));
        let entry: ScriptResourceEntry = (&script_no_schema).into();
        assert!(!entry.has_schema);

        // Script with schema
        let mut input = HashMap::new();
        input.insert(
            "name".to_string(),
            FieldDef {
                field_type: FieldType::String,
                required: true,
                ..Default::default()
            },
        );

        let script_with_schema = Script {
            name: "With Schema".to_string(),
            path: PathBuf::from("/test/with-schema.ts"),
            extension: "ts".to_string(),
            description: None,
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: Some(Schema {
                input,
                output: HashMap::new(),
            }),
            plugin_id: String::new(),
            plugin_title: None,
            kit_name: None,
            body: None,
        };

        let entry: ScriptResourceEntry = (&script_with_schema).into();
        assert!(entry.has_schema);
    }

    #[test]
    fn test_scriptlet_resource_entry_from_scriptlet() {
        let scriptlet = Scriptlet {
            icon: None,
            name: "Full Scriptlet".to_string(),
            description: Some("Test description".to_string()),
            code: "echo test".to_string(),
            tool: "bash".to_string(),
            shortcut: Some("cmd k".to_string()),
            keyword: Some(":test".to_string()),
            group: Some("My Group".to_string()),
            plugin_id: String::new(),
            plugin_title: None,
            file_path: None,
            command: None,
            alias: None,
        };

        let entry: ScriptletResourceEntry = (&scriptlet).into();

        assert_eq!(entry.name, "Full Scriptlet");
        assert_eq!(entry.description, Some("Test description".to_string()));
        assert_eq!(entry.tool, "bash");
        assert_eq!(entry.shortcut, Some("cmd k".to_string()));
        assert_eq!(entry.keyword, Some(":test".to_string()));
        assert_eq!(entry.group, Some("My Group".to_string()));
    }

    #[test]
    fn test_mcp_resource_serialization() {
        let resource = McpResource {
            uri: "test://".to_string(),
            name: "Test".to_string(),
            description: Some("Test description".to_string()),
            mime_type: "application/json".to_string(),
        };

        let json = serde_json::to_string(&resource).unwrap();

        // Should have mimeType (camelCase)
        assert!(json.contains("\"mimeType\""));
        assert!(!json.contains("\"mime_type\""));

        // Deserialize back
        let parsed: McpResource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uri, "test://");
        assert_eq!(parsed.mime_type, "application/json");
    }

    #[test]
    fn test_empty_scripts_resource() {
        let result = read_resource("scripts://", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: Vec<ScriptResourceEntry> = serde_json::from_str(&content.text).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_empty_scriptlets_resource() {
        let result = read_resource("scriptlets://", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: Vec<ScriptletResourceEntry> = serde_json::from_str(&content.text).unwrap();
        assert!(parsed.is_empty());
    }

    // =======================================================
    // Context resource URI parsing tests
    // =======================================================

    #[test]
    fn parse_context_bare_uri_returns_default() {
        let request = parse_context_resource_request("kit://context").unwrap();
        assert_eq!(
            request.options,
            crate::context_snapshot::CaptureContextOptions::default()
        );
        assert_eq!(request.effective_profile, "full");
        assert!(!request.diagnostics);
    }

    #[test]
    fn parse_context_resource_options_supports_minimal_profile() {
        let request = parse_context_resource_request("kit://context?profile=minimal").unwrap();
        assert_eq!(
            request.options,
            crate::context_snapshot::CaptureContextOptions::minimal()
        );
        assert_eq!(request.effective_profile, "minimal");
    }

    #[test]
    fn parse_context_resource_options_allows_profile_overrides() {
        let request = parse_context_resource_request(
            "kit://context?profile=minimal&menuBar=1&selectedText=0",
        )
        .unwrap();

        assert!(!request.options.include_selected_text);
        assert!(request.options.include_menu_bar);
        assert!(request.options.include_frontmost_app);
        assert!(request.options.include_browser_url);
        assert!(request.options.include_focused_window);
        assert_eq!(request.effective_profile, "custom");
    }

    #[test]
    fn parse_context_resource_options_rejects_unknown_flags() {
        let error = parse_context_resource_request("kit://context?nope=1").unwrap_err();
        assert!(
            error.contains("Invalid kit://context parameter: nope"),
            "Error should mention the invalid parameter"
        );
    }

    #[test]
    fn parse_context_rejects_unknown_profile() {
        let error = parse_context_resource_request("kit://context?profile=heavy").unwrap_err();
        assert!(error.contains("Unknown profile"), "Error: {error}");
    }

    #[test]
    fn context_resource_preserves_query_uri() {
        crate::context_snapshot::enable_deterministic_context_capture();
        let content =
            read_resource("kit://context?profile=minimal", &[], &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://context?profile=minimal");
    }

    #[test]
    fn is_context_resource_uri_only_matches_supported_forms() {
        assert!(is_context_resource_uri("kit://context"));
        assert!(is_context_resource_uri("kit://context?profile=minimal"));
        assert!(is_context_resource_uri("kit://context/schema"));
        assert!(!is_context_resource_uri("kit://contextual"));
        assert!(!is_context_resource_uri("kit://context-schema"));
        assert!(!is_context_resource_uri("unknown://context"));
    }

    // =======================================================
    // Context resource: diagnostics, schema, and self-describing tests
    // =======================================================

    #[test]
    fn parse_context_resource_request_supports_diagnostics_flag() {
        let request =
            parse_context_resource_request("kit://context?profile=minimal&diagnostics=1").unwrap();

        assert!(matches!(request.kind, ContextResourceKind::Snapshot));
        assert_eq!(
            request.options,
            crate::context_snapshot::CaptureContextOptions::minimal()
        );
        assert_eq!(request.effective_profile, "minimal");
        assert!(request.diagnostics);
    }

    #[test]
    fn parse_context_resource_request_marks_profile_override_as_custom() {
        let request =
            parse_context_resource_request("kit://context?profile=minimal&selectedText=1").unwrap();

        assert_eq!(request.effective_profile, "custom");
        assert!(request.options.include_selected_text);
    }

    #[test]
    fn parse_context_resource_request_supports_schema_uri() {
        let request = parse_context_resource_request("kit://context/schema").unwrap();
        assert!(matches!(request.kind, ContextResourceKind::Schema));
    }

    /// Per-field queries inherit their baseline from `all()`, which includes
    /// pixel capture. Pixel data must be explicit opt-in: the `@selection`
    /// attachment URI once inherited `include_screenshot` silently and shipped
    /// a 758KB base64 PNG as prompt text, overflowing the model's context.
    #[test]
    fn parse_context_resource_request_field_overrides_disable_pixels_unless_explicit() {
        let request = parse_context_resource_request(
            "kit://context?selectedText=1&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0",
        )
        .unwrap();
        assert!(request.options.include_selected_text);
        assert!(!request.options.include_screenshot);
        assert!(!request.options.include_panel_screenshot);

        let diagnostics = parse_context_resource_request("kit://context?diagnostics=1").unwrap();
        assert!(!diagnostics.options.include_screenshot);
        assert!(!diagnostics.options.include_panel_screenshot);

        let explicit = parse_context_resource_request(
            "kit://context?screenshot=1&selectedText=0&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0",
        )
        .unwrap();
        assert!(explicit.options.include_screenshot);
        assert!(!explicit.options.include_panel_screenshot);

        // An explicit profile keeps its documented pixel semantics.
        let minimal = parse_context_resource_request("kit://context?profile=minimal").unwrap();
        assert_eq!(
            minimal.options,
            crate::context_snapshot::CaptureContextOptions::minimal()
        );
    }

    #[test]
    fn serialize_context_resource_diagnostics_includes_machine_readable_meta() {
        let request =
            parse_context_resource_request("kit://context?profile=minimal&diagnostics=1").unwrap();

        let snapshot = crate::context_snapshot::AiContextSnapshot {
            schema_version: crate::context_snapshot::AI_CONTEXT_SNAPSHOT_SCHEMA_VERSION,
            frontmost_app: Some(crate::context_snapshot::FrontmostAppContext {
                pid: 42,
                bundle_id: "com.example.App".to_string(),
                name: "Example App".to_string(),
            }),
            browser: Some(crate::context_snapshot::BrowserContext::from_url(
                "https://example.com".to_string(),
            )),
            warnings: vec!["focusedWindow: permission denied".to_string()],
            ..Default::default()
        };

        let json = serialize_context_resource(
            "kit://context?profile=minimal&diagnostics=1",
            &request,
            Some(&snapshot),
            12,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["kind"], "context_diagnostics");
        assert_eq!(value["meta"]["effectiveProfile"], "minimal");
        assert_eq!(value["meta"]["status"], "partial");
        assert_eq!(value["meta"]["durationMs"], 12);
        // minimal() enables frontmostApp, browserUrl, focusedWindow, and (since
        // 19db0e0e5, "Enable screenshots in @here (minimal) ... profiles")
        // screenshot — 4 fields total.
        assert_eq!(value["meta"]["enabledFieldCount"], 4);
        assert_eq!(value["meta"]["warningCount"], 1);
        assert_eq!(value["meta"]["fieldStatuses"][0]["field"], "selectedText");
        assert_eq!(value["meta"]["fieldStatuses"][0]["state"], "disabled");
        assert_eq!(value["meta"]["fieldStatuses"][4]["field"], "focusedWindow");
        assert_eq!(value["meta"]["fieldStatuses"][4]["state"], "failed");
        assert_eq!(
            value["meta"]["warnings"][0]["code"],
            "focused_window_capture_failed"
        );
        assert_eq!(value["meta"]["warnings"][0]["message"], "permission denied");
    }

    #[test]
    fn serialize_context_schema_includes_diagnostics_parameter() {
        let request = parse_context_resource_request("kit://context/schema").unwrap();

        let json = serialize_context_resource("kit://context/schema", &request, None, 0).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["kind"], "context_schema");
        assert_eq!(value["diagnosticsSupported"], true);

        let parameter_names: Vec<&str> = value["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|param| param["name"].as_str())
            .collect();

        assert!(parameter_names.contains(&"diagnostics"));

        let has_diagnostics_example =
            value["examples"].as_array().unwrap().iter().any(|example| {
                example.as_str() == Some("kit://context?profile=minimal&diagnostics=1")
            });

        assert!(has_diagnostics_example);
    }

    // =======================================================
    // Schema-versioned script/scriptlet/sdk-reference resources
    // =======================================================

    #[test]
    fn kit_scripts_resource_returns_schema_versioned_envelope() {
        let scripts = wrap_scripts(vec![
            test_script("Hello World", Some("A greeting script")),
            test_script("Fetch Data", None),
        ]);

        let content = read_resource("kit://scripts", &scripts, &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://scripts");
        assert_eq!(content.mime_type, "application/json");

        let doc: ScriptsResourceDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, SCRIPTS_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, 2);
        assert_eq!(doc.scripts.len(), 2);
        assert_eq!(doc.scripts[0].name, "Hello World");
        assert_eq!(
            doc.scripts[0].description,
            Some("A greeting script".to_string())
        );
    }

    #[test]
    fn kit_scripts_resource_empty_returns_zero_count() {
        let content = read_resource("kit://scripts", &[], &[], None).expect("should resolve");
        let doc: ScriptsResourceDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, SCRIPTS_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, 0);
        assert!(doc.scripts.is_empty());
    }

    #[test]
    fn kit_scriptlets_resource_returns_schema_versioned_envelope() {
        let scriptlets = wrap_scriptlets(vec![
            test_scriptlet("Open URL", "open", Some("Opens a URL")),
            test_scriptlet("Paste Text", "paste", None),
        ]);

        let content =
            read_resource("kit://scriptlets", &[], &scriptlets, None).expect("should resolve");
        assert_eq!(content.uri, "kit://scriptlets");

        let doc: ScriptletsResourceDocument =
            serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, SCRIPTLETS_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, 2);
        assert_eq!(doc.scriptlets.len(), 2);
        assert_eq!(doc.scriptlets[0].name, "Open URL");
        assert_eq!(doc.scriptlets[0].tool, "open");
    }

    #[test]
    fn kit_scriptlets_resource_empty_returns_zero_count() {
        let content = read_resource("kit://scriptlets", &[], &[], None).expect("should resolve");
        let doc: ScriptletsResourceDocument =
            serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.count, 0);
        assert!(doc.scriptlets.is_empty());
    }

    #[test]
    fn sdk_reference_resource_returns_valid_document() {
        let content = read_resource("kit://sdk-reference", &[], &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://sdk-reference");

        let doc: SdkReferenceDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, SDK_REFERENCE_SCHEMA_VERSION);
        assert_eq!(doc.sdk_package, "@scriptkit/sdk");
        assert!(!doc.functions.is_empty());

        // Verify key functions are present
        let names: Vec<&str> = doc.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"arg"), "should include arg()");
        assert!(names.contains(&"div"), "should include div()");
        assert!(names.contains(&"exec"), "should include exec()");
        assert!(names.contains(&"copy"), "should include copy()");
    }

    #[test]
    fn sdk_reference_has_categories() {
        let doc = build_sdk_reference_document();
        let categories: Vec<&str> = doc
            .functions
            .iter()
            .map(|f| f.category.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        assert!(categories.contains(&"prompts"));
        assert!(categories.contains(&"system"));
        assert!(categories.contains(&"clipboard"));
        assert!(categories.contains(&"filesystem"));
    }

    #[test]
    fn kit_scripts_resource_json_uses_camel_case() {
        let scripts = wrap_scripts(vec![test_script("Test", None)]);
        let content = read_resource("kit://scripts", &scripts, &[], None).unwrap();
        assert!(content.text.contains("\"schemaVersion\""));
        assert!(!content.text.contains("\"schema_version\""));
    }

    #[test]
    fn resource_definitions_include_new_resources() {
        let resources = get_resource_definitions();
        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(uris.contains(&"kit://scripts"));
        assert!(uris.contains(&"kit://scriptlets"));
        assert!(uris.contains(&"kit://sdk-reference"));
    }

    #[test]
    fn sdk_reference_includes_metadata_format() {
        let doc = build_sdk_reference_document();
        assert!(doc.metadata_format.contains("export const metadata"));
        assert!(doc.script_directory.contains("plugins/main/scripts"));
        assert!(doc.scriptlet_pattern.contains("scriptlets"));
    }

    #[test]
    fn sdk_reference_discovers_host_diagnostics_without_inventing_sdk_globals() {
        let doc = build_sdk_reference_document();
        let doctor = doc
            .authoring_resources
            .iter()
            .find(|resource| resource.uri == COMMAND_DOCTOR_RESOURCE_URI)
            .expect("command doctor is discoverable in the host authoring reference");
        assert_eq!(doctor.name, "Command Doctor");
        assert!(doc
            .authoring_resources
            .iter()
            .any(|resource| resource.uri == FAILED_SCRIPTS_RESOURCE_URI));
        assert!(!doc
            .functions
            .iter()
            .any(|function| function.name == "commandDoctor"));

        let json = serde_json::to_string(&doc).expect("serialize machine-readable reference");
        assert!(json.contains("\"authoringResources\""));
        assert!(json.contains(COMMAND_DOCTOR_RESOURCE_URI));
    }

    #[test]
    fn sdk_reference_roundtrips_through_json() {
        let doc = build_sdk_reference_document();
        let json = serde_json::to_string(&doc).expect("serialize");
        let parsed: SdkReferenceDocument = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(doc, parsed);
    }

    // =======================================================
    // kit://failed-scripts resource tests
    // =======================================================

    #[test]
    fn command_doctor_reports_supported_unsupported_and_malformed_commands() {
        use crate::metadata_parser::TypedMetadata;
        use std::path::PathBuf;

        let make_script = |name: &str, extension: &str, capabilities: Value| {
            Arc::new(Script {
                name: name.to_string(),
                path: PathBuf::from(format!("/tmp/{name}.{extension}")),
                extension: extension.to_string(),
                plugin_id: "main".to_string(),
                plugin_title: Some("Main".to_string()),
                typed_metadata: Some(TypedMetadata {
                    extra: HashMap::from([("sdkCapabilities".to_string(), capabilities)]),
                    ..TypedMetadata::default()
                }),
                ..Script::default()
            })
        };
        let supported = make_script("supported", "ts", serde_json::json!(["home"]));
        let unsupported = make_script("unsupported", "ts", serde_json::json!(["widget"]));
        let malformed = make_script("malformed", "ts", serde_json::json!("home"));
        let no_transport = make_script("shell", "sh", serde_json::json!(["readFile"]));

        let report = build_command_doctor_report(
            &[no_transport, unsupported, supported, malformed],
            &[],
            None,
        );
        assert_eq!(report.total_commands, 4);
        assert_eq!(report.ready_count, 1);
        assert_eq!(report.unsupported_count, 1);
        assert_eq!(report.blocked_count, 2);
        assert!(!report.permission_inventory_known);
        assert_eq!(
            report
                .commands
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["malformed", "shell", "supported", "unsupported"]
        );
        let denied = report
            .commands
            .iter()
            .find(|entry| entry.name == "unsupported")
            .expect("unsupported command stays visible");
        assert_eq!(denied.state, CommandDoctorState::Unsupported);
        assert!(!denied.executable);
        assert!(!denied.alternatives.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn command_doctor_treats_unknown_permission_as_pending_not_denied() {
        let script = Arc::new(Script {
            name: "Move Window".to_string(),
            path: std::path::PathBuf::from("/tmp/move-window.ts"),
            extension: "ts".to_string(),
            plugin_id: "main".to_string(),
            typed_metadata: Some(crate::metadata_parser::TypedMetadata {
                extra: HashMap::from([(
                    "sdkCapabilities".to_string(),
                    serde_json::json!(["moveWindow"]),
                )]),
                ..crate::metadata_parser::TypedMetadata::default()
            }),
            ..Script::default()
        });

        let pending = build_command_doctor_report(&[Arc::clone(&script)], &[], None);
        assert_eq!(pending.permission_pending_count, 1);
        assert_eq!(pending.blocked_count, 0);
        assert!(!pending.commands[0].executable);
        assert_eq!(
            pending.commands[0].state,
            CommandDoctorState::PermissionPending
        );
        let pending_action = pending.commands[0]
            .primary_action
            .as_ref()
            .expect("pending script retains its actual canonical launcher action");
        assert!(!pending_action.enabled);
        assert_eq!(pending_action.reason.as_deref(), Some("permission_pending"));

        let known = SdkHostAvailability {
            host_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: "macos".to_string(),
            granted_permissions: vec!["accessibility".to_string()],
        };
        let ready = build_command_doctor_report(&[script], &[], Some(&known));
        assert!(ready.permission_inventory_known);
        assert_eq!(ready.ready_count, 1);
        assert_eq!(ready.commands[0].state, CommandDoctorState::Ready);
        assert!(
            ready.commands[0]
                .primary_action
                .as_ref()
                .expect("granted script exposes its actual canonical launcher action")
                .enabled
        );
    }

    #[test]
    fn command_doctor_experimental_features_remain_explicitly_executable() {
        let script = Arc::new(Script {
            name: "Experimental Feedback".to_string(),
            path: std::path::PathBuf::from("/tmp/experimental-feedback.ts"),
            extension: "ts".to_string(),
            plugin_id: "main".to_string(),
            typed_metadata: Some(crate::metadata_parser::TypedMetadata {
                extra: HashMap::from([(
                    "sdkCapabilities".to_string(),
                    serde_json::json!(["beep"]),
                )]),
                ..crate::metadata_parser::TypedMetadata::default()
            }),
            ..Script::default()
        });
        let host = SdkHostAvailability {
            host_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: "macos".to_string(),
            granted_permissions: Vec::new(),
        };

        let report = build_command_doctor_report(&[script], &[], Some(&host));
        assert_eq!(report.experimental_count, 1);
        assert_eq!(report.commands[0].state, CommandDoctorState::Experimental);
        assert_eq!(
            report.commands[0].capabilities[0].support,
            SdkSupport::Experimental
        );
        assert!(report.commands[0].executable);
    }

    #[test]
    fn command_doctor_preview_uses_real_descriptor_without_leaking_identity() {
        use sk_protocol::command_contract::{
            CommandAvailability, CommandDescriptor, CommandIdentity, CommandSource,
        };

        let identity = CommandIdentity::new(CommandSource::Script, "main:private-script")
            .expect("canonical identity");
        let mut descriptor = CommandDescriptor::new(identity, "Private Script", "Run Script")
            .expect("real canonical descriptor");
        let ready = command_doctor_preview_from_descriptor(&descriptor)
            .expect("descriptor has real primary action");
        assert_eq!(ready.title, "Run Script");
        assert!(ready.enabled);
        let digest = ready
            .identity_fingerprint
            .strip_prefix("sha256:")
            .expect("identity uses the shared cryptographic receipt-redaction contract");
        assert_eq!(digest.len(), 64);
        assert!(digest
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
        assert!(!ready.identity_fingerprint.contains("private-script"));

        descriptor.availability = CommandAvailability::TemporarilyUnavailable;
        descriptor.actions[0].availability = CommandAvailability::TemporarilyUnavailable;
        let blocked = command_doctor_preview_from_descriptor(&descriptor)
            .expect("blocked real action remains inspectable");
        assert!(!blocked.enabled);
        assert_eq!(blocked.reason.as_deref(), Some("temporarily_unavailable"));
        assert_eq!(blocked.identity_fingerprint, ready.identity_fingerprint);
    }

    #[test]
    fn command_doctor_excludes_source_code_credentials_and_custom_secret_values() {
        let secret = "sk_live_doctor_must_never_appear";
        let script = Arc::new(Script {
            name: "Safe author diagnostics".to_string(),
            path: std::path::PathBuf::from("/tmp/safe-author-command.ts"),
            extension: "ts".to_string(),
            plugin_id: "main".to_string(),
            body: Some(format!("const token = '{secret}';")),
            typed_metadata: Some(crate::metadata_parser::TypedMetadata {
                extra: HashMap::from([
                    ("sdkCapabilities".to_string(), serde_json::json!(["home"])),
                    ("privateToken".to_string(), serde_json::json!(secret)),
                ]),
                ..crate::metadata_parser::TypedMetadata::default()
            }),
            ..Script::default()
        });

        let report = build_command_doctor_report(&[script], &[], None);
        let json = serde_json::to_string(&report).expect("serialize safe receipt");
        assert!(json.contains("/tmp/safe-author-command.ts"));
        assert!(json.contains("Safe author diagnostics"));
        assert!(!json.contains(secret));
        assert!(!json.contains("privateToken"));
        assert!(!json.contains("const token"));
    }

    #[test]
    fn command_doctor_resource_uses_only_explicit_loaded_snapshots() {
        let resource = read_resource(COMMAND_DOCTOR_RESOURCE_URI, &[], &[], None)
            .expect("command doctor resolves without app/provider access");
        assert_eq!(resource.uri, COMMAND_DOCTOR_RESOURCE_URI);
        let report: CommandDoctorReport =
            serde_json::from_str(&resource.text).expect("parse command doctor receipt");
        assert_eq!(
            report.schema_version,
            COMMAND_DOCTOR_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(report.total_commands, 0);
        assert!(report.commands.is_empty());
        assert!(!report.permission_inventory_known);
    }

    #[test]
    fn failed_scripts_resource_is_listed() {
        let resources = get_resource_definitions();
        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(
            uris.contains(&FAILED_SCRIPTS_RESOURCE_URI),
            "{FAILED_SCRIPTS_RESOURCE_URI} should be in resource definitions"
        );
    }

    #[test]
    fn failed_scripts_resource_lists_validation_failures() {
        use crate::scripts::{
            BindingKind, FailedScript, MetadataField, RelatedScript, ScriptValidationIssue,
            ScriptValidationKind, ValidationReport, ValidationSeverity, VALIDATION_SCHEMA_VERSION,
        };
        use std::path::PathBuf;

        // Two scripts colliding on `cmd k` — mirrors what `validate_script_catalog`
        // would emit for real duplicate-shortcut metadata on disk.
        let issue_for = |path: &str, peer: &str| ScriptValidationIssue {
            severity: ValidationSeverity::Fatal,
            path: PathBuf::from(path),
            script_name: path.into(),
            field: Some(MetadataField::Shortcut),
            message: "Shortcut `cmd k` is declared by 2 scripts".into(),
            kind: ScriptValidationKind::DuplicateBinding {
                binding: BindingKind::Shortcut,
                value: "cmd k".into(),
            },
            related: vec![RelatedScript {
                path: PathBuf::from(peer),
                name: peer.into(),
            }],
        };
        let failed = vec![
            FailedScript {
                path: PathBuf::from("/tmp/a.ts"),
                name: "a".into(),
                fatal: Arc::from(vec![issue_for("/tmp/a.ts", "/tmp/b.ts")]),
            },
            FailedScript {
                path: PathBuf::from("/tmp/b.ts"),
                name: "b".into(),
                fatal: Arc::from(vec![issue_for("/tmp/b.ts", "/tmp/a.ts")]),
            },
        ];
        let report = ValidationReport {
            schema_version: VALIDATION_SCHEMA_VERSION,
            total_candidates: 2,
            valid_count: 0,
            fatal_count: 2,
            warning_count: 0,
            failed_scripts: Arc::from(failed),
            warnings: Arc::from(Vec::<ScriptValidationIssue>::new()),
            retained_issues: Arc::from(Vec::<ScriptValidationIssue>::new()),
        };

        let doc = build_failed_scripts_document(&report);
        assert_eq!(doc.schema_version, FAILED_SCRIPTS_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.validation_schema_version, VALIDATION_SCHEMA_VERSION);
        assert_eq!(doc.total_candidates, 2);
        assert_eq!(doc.valid_count, 0);
        assert_eq!(doc.fatal_count, 2);
        assert_eq!(doc.failed_scripts.len(), 2);

        // Each failure must name its peer so the author can repair both sides.
        for entry in &doc.failed_scripts {
            assert_eq!(entry.fatal.len(), 1);
            assert_eq!(entry.fatal[0].related.len(), 1);
            assert!(matches!(
                entry.fatal[0].kind,
                ScriptValidationKind::DuplicateBinding {
                    binding: BindingKind::Shortcut,
                    ..
                }
            ));
        }

        let json = serde_json::to_string(&doc).expect("serialize");
        assert!(json.contains("\"schemaVersion\""));
        assert!(!json.contains("\"schema_version\""));
        assert!(json.contains("\"duplicateBinding\""));
        let parsed: FailedScriptsResourceDocument =
            serde_json::from_str(&json).expect("round-trip");
        assert_eq!(parsed.failed_scripts.len(), 2);
    }

    #[test]
    fn failed_scripts_resource_empty_report_serializes_cleanly() {
        use crate::scripts::{ValidationReport, VALIDATION_SCHEMA_VERSION};

        let report = ValidationReport {
            schema_version: VALIDATION_SCHEMA_VERSION,
            total_candidates: 0,
            valid_count: 0,
            fatal_count: 0,
            warning_count: 0,
            failed_scripts: Arc::from(Vec::new()),
            warnings: Arc::from(Vec::new()),
            retained_issues: Arc::from(Vec::new()),
        };
        let doc = build_failed_scripts_document(&report);
        assert_eq!(doc.fatal_count, 0);
        assert!(doc.failed_scripts.is_empty());
        assert!(doc.warnings.is_empty());

        let json = serde_json::to_string(&doc).expect("serialize");
        let parsed: FailedScriptsResourceDocument =
            serde_json::from_str(&json).expect("round-trip");
        assert_eq!(
            parsed.schema_version,
            FAILED_SCRIPTS_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(parsed.retained_issue_count, 0);
        assert!(parsed.retained_issues.is_empty());
    }

    #[test]
    fn failed_scripts_resource_keeps_retained_fatal_scriptlet_issues_distinct() {
        use crate::scripts::{
            MetadataField, ScriptValidationIssue, ScriptValidationKind, ValidationReport,
            ValidationSeverity, VALIDATION_SCHEMA_VERSION,
        };

        let issue = ScriptValidationIssue {
            severity: ValidationSeverity::Fatal,
            path: std::path::PathBuf::from("/tmp/retained-scriptlet.md"),
            script_name: "Retained Shell Command".to_string(),
            field: Some(MetadataField::Capability),
            message: "Shell scriptlets do not receive SDK globals.".to_string(),
            kind: ScriptValidationKind::CapabilityUnavailable {
                capability: "readFile".to_string(),
                code: SdkCapabilityDiagnosticCode::MissingSdkTransport,
                alternatives: vec!["Move the command into a TypeScript script.".to_string()],
            },
            related: Vec::new(),
        };
        let report = ValidationReport {
            schema_version: VALIDATION_SCHEMA_VERSION,
            total_candidates: 1,
            valid_count: 0,
            fatal_count: 1,
            warning_count: 0,
            failed_scripts: Arc::from(Vec::new()),
            warnings: Arc::from(Vec::new()),
            retained_issues: Arc::from(vec![issue]),
        };

        let document = build_failed_scripts_document(&report);
        assert!(document.failed_scripts.is_empty());
        assert!(document.warnings.is_empty());
        assert_eq!(document.retained_issue_count, 1);
        assert_eq!(
            document.retained_issues[0].severity,
            ValidationSeverity::Fatal
        );
        assert_eq!(
            document.retained_issues[0].path,
            std::path::PathBuf::from("/tmp/retained-scriptlet.md")
        );
        assert!(!document.retained_issues[0].message.is_empty());

        let json = serde_json::to_value(&document).expect("serialize resource");
        assert_eq!(json["retainedIssueCount"], 1);
        assert_eq!(json["retainedIssues"][0]["severity"], "fatal");
        assert_eq!(json["failedScripts"], serde_json::json!([]));
    }

    #[test]
    fn failed_scripts_resource_accepts_legacy_documents_without_retained_fields() {
        let report = ValidationReport {
            schema_version: crate::scripts::VALIDATION_SCHEMA_VERSION,
            total_candidates: 0,
            valid_count: 0,
            fatal_count: 0,
            warning_count: 0,
            failed_scripts: Arc::from(Vec::new()),
            warnings: Arc::from(Vec::new()),
            retained_issues: Arc::from(Vec::new()),
        };
        let mut legacy = serde_json::to_value(build_failed_scripts_document(&report))
            .expect("serialize current resource");
        let object = legacy.as_object_mut().expect("resource object");
        object.remove("retainedIssueCount");
        object.remove("retainedIssues");

        let restored: FailedScriptsResourceDocument =
            serde_json::from_value(legacy).expect("legacy authoring resources remain readable");
        assert_eq!(restored.retained_issue_count, 0);
        assert!(restored.retained_issues.is_empty());
    }

    #[test]
    fn failed_scripts_resource_read_returns_parseable_envelope() {
        // End-to-end: resolves the URI through `read_resource` which calls
        // `read_scripts_report()` internally. Machine state may be non-empty,
        // so assert envelope shape, not failure count.
        let content = read_resource(FAILED_SCRIPTS_RESOURCE_URI, &[], &[], None)
            .expect("resource should resolve");
        assert_eq!(content.uri, FAILED_SCRIPTS_RESOURCE_URI);
        assert_eq!(content.mime_type, "application/json");

        let doc: FailedScriptsResourceDocument =
            serde_json::from_str(&content.text).expect("valid envelope JSON");
        assert_eq!(doc.schema_version, FAILED_SCRIPTS_RESOURCE_SCHEMA_VERSION);
        // If any script failed, its fatal-issue total must be at least as large
        // as the distinct failed-script count (each failed script has ≥1 issue).
        assert!(doc.fatal_count >= doc.failed_scripts.len());
    }

    #[test]
    fn parse_context_request_accepts_panel_screenshot_flag() {
        let request = parse_context_resource_request(
            "kit://context?screenshot=1&panelScreenshot=1&diagnostics=1",
        )
        .expect("request");
        assert!(request.options.include_screenshot);
        assert!(request.options.include_panel_screenshot);
        assert!(request.diagnostics);
    }

    #[test]
    fn diagnostics_surface_reports_panel_screenshot_state() {
        let request =
            parse_context_resource_request("kit://context?panelScreenshot=1&diagnostics=1")
                .expect("request");

        let snapshot = crate::context_snapshot::AiContextSnapshot {
            schema_version: crate::context_snapshot::AI_CONTEXT_SNAPSHOT_SCHEMA_VERSION,
            script_kit_panel_image: Some(crate::context_snapshot::Base64PngContext {
                mime_type: "image/png".to_string(),
                width: 700,
                height: 520,
                base64_data: "cGFuZWw=".to_string(),
                title: Some("Script Kit - Clipboard History".to_string()),
            }),
            ..Default::default()
        };

        let doc = build_context_diagnostics_document(
            "kit://context?panelScreenshot=1&diagnostics=1",
            &request,
            &snapshot,
            1,
        );
        assert!(doc
            .meta
            .field_statuses
            .iter()
            .any(|field| field.field == "panelScreenshot"
                && field.enabled
                && field.present
                && matches!(field.state, ContextFieldCaptureState::Captured)));
    }

    #[test]
    fn schema_document_includes_panel_screenshot_parameter() {
        let schema = build_context_schema_document();
        assert!(
            schema
                .parameters
                .iter()
                .any(|p| p.name == "panelScreenshot"),
            "schema must list panelScreenshot parameter"
        );
    }

    // =======================================================
    // Clipboard history resource tests
    // =======================================================

    #[test]
    fn clipboard_history_resource_is_listed() {
        let resources = get_resource_definitions();
        assert!(
            resources.iter().any(|r| r.uri == "kit://clipboard-history"),
            "kit://clipboard-history should be in resource definitions"
        );
    }

    #[test]
    fn clipboard_history_resource_resolves_with_valid_schema() {
        let content =
            read_resource("kit://clipboard-history", &[], &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://clipboard-history");
        assert_eq!(content.mime_type, "application/json");

        let doc: ClipboardHistoryDocument =
            serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(
            doc.schema_version,
            CLIPBOARD_HISTORY_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(doc.count, doc.entries.len());
    }

    #[test]
    fn clipboard_history_parse_accepts_limit_param() {
        let req = parse_clipboard_history_request("kit://clipboard-history?limit=5").unwrap();
        match req {
            ClipboardHistoryRequest::List { limit, diagnostics } => {
                assert_eq!(limit, 5);
                assert!(!diagnostics);
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn clipboard_history_parse_clamps_limit_to_max() {
        let req = parse_clipboard_history_request("kit://clipboard-history?limit=999").unwrap();
        match req {
            ClipboardHistoryRequest::List { limit, .. } => {
                assert_eq!(limit, CLIPBOARD_HISTORY_MAX_LIMIT);
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn clipboard_history_parse_rejects_unknown_param() {
        let err = parse_clipboard_history_request("kit://clipboard-history?foo=1").unwrap_err();
        assert!(err.contains("Invalid kit://clipboard-history parameter"));
    }

    #[test]
    fn clipboard_history_parse_accepts_id_param() {
        let req = parse_clipboard_history_request("kit://clipboard-history?id=abc123").unwrap();
        match req {
            ClipboardHistoryRequest::SingleEntry { id } => {
                assert_eq!(id, "abc123");
            }
            other => panic!("Expected SingleEntry, got {other:?}"),
        }
    }

    #[test]
    fn clipboard_history_diagnostics_returns_wrapper() {
        let content = read_resource("kit://clipboard-history?diagnostics=1", &[], &[], None)
            .expect("should resolve");

        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(value["kind"], "clipboard_history_diagnostics");
        assert_eq!(
            value["document"]["schemaVersion"],
            CLIPBOARD_HISTORY_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(value["meta"]["source"], "cached_entries");
    }

    #[test]
    fn clipboard_history_entry_serialization_roundtrip() {
        let entry = ClipboardHistoryEntry {
            id: "abc-123".to_string(),
            content_type: "text".to_string(),
            timestamp: 1711700000,
            text_preview: Some("Hello world".to_string()),
            ocr_text: None,
            image_width: None,
            image_height: None,
            pinned: false,
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: ClipboardHistoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(entry, parsed);
    }

    // =======================================================
    // Focused item resource tests
    // =======================================================

    #[test]
    fn focused_item_resource_is_listed() {
        let resources = get_resource_definitions();
        assert!(
            resources.iter().any(|r| r.uri == "kit://focused-item"),
            "kit://focused-item should be in resource definitions"
        );
    }

    #[test]
    fn focused_item_resource_returns_empty_when_no_slot() {
        // Ensure slot is clear
        clear_focused_item();

        let content = read_resource("kit://focused-item", &[], &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://focused-item");

        let doc: FocusedItemDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, FOCUSED_ITEM_RESOURCE_SCHEMA_VERSION);
        assert!(!doc.has_focused_item);
        assert!(doc.focused_item.is_none());
        assert!(
            !doc.warnings.is_empty(),
            "should have a warning when no item"
        );
    }

    #[test]
    fn focused_item_resource_returns_published_item() {
        publish_focused_item(FocusedItemInfo {
            source: "ClipboardHistory".to_string(),
            kind: "clipboard_entry".to_string(),
            semantic_id: "choice:0:hello".to_string(),
            label: "hello world".to_string(),
            metadata: Some(serde_json::json!({"contentType": "text"})),
        });

        let content = read_resource("kit://focused-item", &[], &[], None).expect("should resolve");

        let doc: FocusedItemDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert!(doc.has_focused_item);
        let item = doc.focused_item.expect("item present");
        assert_eq!(item.source, "ClipboardHistory");
        assert_eq!(item.semantic_id, "choice:0:hello");
        assert!(doc.warnings.is_empty());

        // Clean up
        clear_focused_item();
    }

    #[test]
    fn focused_item_parse_rejects_unknown_param() {
        let err = parse_focused_item_request("kit://focused-item?foo=1").unwrap_err();
        assert!(err.contains("Invalid kit://focused-item parameter"));
    }

    #[test]
    fn focused_item_diagnostics_returns_wrapper() {
        clear_focused_item();

        let content = read_resource("kit://focused-item?diagnostics=1", &[], &[], None)
            .expect("should resolve");

        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(value["kind"], "focused_item_diagnostics");
        assert_eq!(
            value["document"]["schemaVersion"],
            FOCUSED_ITEM_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(value["meta"]["source"], "focused_item_slot");
        assert_eq!(value["meta"]["hasFocusedItem"], false);
        assert!(value["meta"]["warningCount"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn focused_item_info_serialization_roundtrip() {
        let item = FocusedItemInfo {
            source: "FileSearch".to_string(),
            kind: "file".to_string(),
            semantic_id: "choice:2:readme".to_string(),
            label: "README.md".to_string(),
            metadata: Some(serde_json::json!({"path": "/tmp/README.md"})),
        };
        let json = serde_json::to_string(&item).expect("serialize");
        let parsed: FocusedItemInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(item, parsed);
    }

    #[test]
    fn test_notes_list_resource_full_param_returns_full_content() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::notes::init_notes_db().expect("notes db should initialize before resource test");
        let token = unique_notes_resource_token("resource_full");
        let body: String = format!(
            "---\ntags: [{token}]\n---\n# Full Body\n{}",
            "x".repeat(600)
        );
        let note = crate::notes::Note::with_content(body.clone());
        let note_id = note.id;
        crate::notes::save_note(&note).expect("failed to save notes full-content test note");

        let content = read_notes_list_resource(&format!("kit://notes?tag={token}&full=true"))
            .expect("full-content notes resource should resolve");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
        let notes = value["notes"].as_array().expect("notes array");
        let entry = notes
            .iter()
            .find(|candidate| candidate["id"] == note_id.as_str())
            .expect("created note should be returned by full-content resource");
        let entry = entry.clone();

        crate::notes::delete_note_permanently(note_id)
            .expect("cleanup failed for notes full-content test");

        assert_eq!(
            entry["content"].as_str().expect("content string"),
            body,
            "full=true should return the complete note body, not a preview"
        );
        assert!(entry.get("preview").is_none(), "full entries drop preview");
        assert_eq!(entry["contentTruncated"], serde_json::Value::Bool(false));
    }

    #[test]
    fn test_notes_list_resource_can_filter_and_report_metadata() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::notes::init_notes_db().expect("notes db should initialize before resource test");
        let token = unique_notes_resource_token("resource_tag");
        let note = crate::notes::Note::with_content(format!(
            "---\ntags: [{token}]\naliases: [{token} Alias]\n---\n# Resource Metadata\nBody [[{token} Target]]"
        ));
        let note_id = note.id;
        crate::notes::save_note(&note).expect("failed to save notes resource test note");

        let content = read_notes_list_resource(&format!("kit://notes?tag={token}&limit=10"))
            .expect("tag-filtered notes resource should resolve");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
        let notes = value["notes"].as_array().expect("notes array");
        let summary = notes
            .iter()
            .find(|candidate| candidate["id"] == note_id.as_str())
            .expect("created note should be returned by tag-filtered resource");

        crate::notes::delete_note_permanently(note_id)
            .expect("cleanup failed for notes resource metadata test");

        assert_eq!(value["query"], format!("tag:{token}"));
        assert!(
            summary["metadata"]["tags"]
                .as_array()
                .expect("tags array")
                .iter()
                .any(|tag| tag == token.as_str()),
            "summary metadata should include indexed tags"
        );
        assert!(
            summary["metadata"]["aliases"]
                .as_array()
                .expect("aliases array")
                .iter()
                .any(|alias| alias == format!("{token} Alias").as_str()),
            "summary metadata should include indexed aliases"
        );
        assert_eq!(summary["metadata"]["outboundLinkCount"], 1);
    }

    #[test]
    fn test_single_note_resource_reports_metadata() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::notes::init_notes_db().expect("notes db should initialize before resource test");
        let token = unique_notes_resource_token("single_resource");
        let note = crate::notes::Note::with_content(format!(
            "---\ntags: [{token}]\naliases: [{token} Alias]\n---\n# Single Resource\nBody [[{token} Target]]"
        ));
        let note_id = note.id;
        crate::notes::save_note(&note).expect("failed to save single notes resource test note");

        let content = read_single_note_resource(&format!("kit://notes/{note_id}"))
            .expect("single notes resource should resolve");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        crate::notes::delete_note_permanently(note_id)
            .expect("cleanup failed for single notes resource metadata test");

        assert_eq!(value["note"]["id"], note_id.as_str());
        assert!(
            value["metadata"]["tags"]
                .as_array()
                .expect("tags array")
                .iter()
                .any(|tag| tag == token.as_str()),
            "single note metadata should include indexed tags"
        );
        assert!(
            value["metadata"]["aliases"]
                .as_array()
                .expect("aliases array")
                .iter()
                .any(|alias| alias == format!("{token} Alias").as_str()),
            "single note metadata should include indexed aliases"
        );
        assert_eq!(value["metadata"]["outboundLinkCount"], 1);
    }

    #[test]
    fn test_notes_resource_query_params_are_url_decoded() {
        assert_eq!(
            query_string_param("kit://notes?q=project%20plan", "q"),
            Some("project plan".to_string())
        );
        assert_eq!(
            query_string_param("kit://notes?alias=Project+Plan", "alias"),
            Some("Project Plan".to_string())
        );
        assert_eq!(
            notes_list_search_query("kit://notes?tag=projects%2Fscript-kit"),
            Some("tag:projects/script-kit".to_string())
        );
    }

    #[test]
    fn test_notes_list_resource_filters_alias_link_q_and_plus_decoding() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::notes::init_notes_db().expect("notes db should initialize before resource test");
        let token = unique_notes_resource_token("resource_query");
        let alias = format!("{token} Project Plan");
        let target_title = format!("{token} Target Note");
        let body_token = format!("{token}_body");
        let note = crate::notes::Note::with_content(format!(
            "---\naliases: [{alias}]\n---\n# Resource Query\n{body_token} links to [[{target_title}]]"
        ));
        let note_id = note.id;
        crate::notes::save_note(&note).expect("failed to save notes resource query test note");

        let alias_uri = format!("kit://notes?alias={}&limit=10", alias.replace(' ', "+"));
        let link_uri = format!(
            "kit://notes?link={}&limit=10",
            target_title.replace(' ', "+")
        );
        let text_uri = format!("kit://notes?q={body_token}&limit=10");
        let alias_content =
            read_notes_list_resource(&alias_uri).expect("alias-filtered notes should resolve");
        let link_content =
            read_notes_list_resource(&link_uri).expect("link-filtered notes should resolve");
        let text_content =
            read_notes_list_resource(&text_uri).expect("text-filtered notes should resolve");

        crate::notes::delete_note_permanently(note_id)
            .expect("cleanup failed for notes resource query test");

        for (label, content) in [
            ("alias", alias_content),
            ("link", link_content),
            ("q", text_content),
        ] {
            let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
            let notes = value["notes"].as_array().expect("notes array");
            assert!(
                notes
                    .iter()
                    .any(|candidate| candidate["id"] == note_id.as_str()),
                "{label} resource filter should return the created note"
            );
        }
    }

    // ── Provider-backed JSON resource tests ───────────────────────

    #[test]
    fn dictation_resource_empty_fallback_has_explicit_envelope() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        std::env::remove_var("SCRIPT_KIT_DICTATION_JSON");

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://dictation", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["type"], "dictation");
        assert_eq!(value["ok"], true);
        assert_eq!(value["available"], false);
        assert_eq!(value["source"], "empty-fallback");
        assert!(value["items"].is_array());
        assert!(value["note"].is_string());
        assert!(value["nextStep"].is_string());
    }

    #[test]
    fn calendar_resource_empty_fallback_has_explicit_envelope() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        std::env::remove_var("SCRIPT_KIT_CALENDAR_JSON");

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://calendar", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["type"], "calendar");
        assert_eq!(value["ok"], true);
        assert_eq!(value["available"], false);
        assert_eq!(value["source"], "empty-fallback");
        assert!(value["items"].is_array());
        assert!(value["note"].is_string());
        assert!(value["nextStep"].is_string());
    }

    #[test]
    fn notifications_resource_empty_fallback_has_explicit_envelope() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        std::env::remove_var("SCRIPT_KIT_NOTIFICATIONS_JSON");

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://notifications", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["type"], "notifications");
        assert_eq!(value["ok"], true);
        assert_eq!(value["available"], false);
        assert_eq!(value["source"], "empty-fallback");
        assert!(value["items"].is_array());
        assert!(value["note"].is_string());
        assert!(value["nextStep"].is_string());
    }

    #[test]
    fn dictation_resource_prefers_slot_data() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        publish_dictation_json(
            r#"{"schemaVersion":1,"type":"dictation","ok":true,"available":true,"source":"slot","items":[{"text":"hello"}]}"#,
        );

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://dictation", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["available"], true);
        assert_eq!(value["source"], "slot");
        assert_eq!(value["items"].as_array().expect("items array").len(), 1);

        clear_provider_json_slots();
    }

    #[test]
    fn dictation_provider_history_projects_real_preview_and_target_into_searchable_items() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        publish_dictation_json(
            r#"{"schemaVersion":1,"type":"dictation","ok":true,"available":true,"items":[{"preview":"private spoken preview","text":"complete private spoken transcript","target":"Notes"},{"text":"legacy private transcript","target":"Agent Chat"}]}"#,
        );

        let items = read_provider_json_items(ProviderJsonResourceKind::Dictation);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "private spoken preview");
        assert_eq!(items[0].subtitle.as_deref(), Some("Notes"));
        assert_eq!(items[1].title, "legacy private transcript");
        assert_eq!(items[1].subtitle.as_deref(), Some("Agent Chat"));
        clear_provider_json_slots();
    }

    #[test]
    fn dictation_provider_history_keeps_calendar_titles_strict_and_rejects_empty_transcripts() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        publish_dictation_json(
            r#"{"schemaVersion":1,"type":"dictation","ok":true,"available":true,"items":[{"preview":"","text":"recoverable private transcript"},{"preview":"","text":""}]}"#,
        );
        publish_calendar_json(
            r#"{"schemaVersion":1,"type":"calendar","ok":true,"available":true,"items":[{"text":"calendar text is not an event title"}]}"#,
        );

        let dictation = read_provider_json_items(ProviderJsonResourceKind::Dictation);
        let calendar = read_provider_json_items(ProviderJsonResourceKind::Calendar);

        assert_eq!(dictation.len(), 1);
        assert_eq!(dictation[0].title, "recoverable private transcript");
        assert!(calendar.is_empty());
        clear_provider_json_slots();
    }

    #[test]
    fn calendar_resource_prefers_slot_data() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        publish_calendar_json(
            r#"{"schemaVersion":1,"type":"calendar","ok":true,"available":true,"source":"slot","items":[{"title":"Demo"}]}"#,
        );

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://calendar", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["available"], true);
        assert_eq!(value["source"], "slot");
        assert_eq!(value["items"].as_array().expect("items array").len(), 1);

        clear_provider_json_slots();
    }

    #[test]
    fn notifications_resource_prefers_slot_data() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        publish_notifications_json(
            r#"{"schemaVersion":1,"type":"notifications","ok":true,"available":true,"source":"slot","items":[{"title":"Build complete"}]}"#,
        );

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://notifications", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["available"], true);
        assert_eq!(value["source"], "slot");
        assert_eq!(value["items"].as_array().expect("items array").len(), 1);

        clear_provider_json_slots();
    }

    fn sdk_ref(name: &str, signature: &str, description: &str, category: &str) -> SdkFunctionRef {
        SdkFunctionRef::supported(name, signature, description, category)
    }

    #[test]
    fn filter_sdk_reference_entries_empty_filter_returns_all_indices() {
        let entries = vec![
            sdk_ref("arg", "arg(prompt)", "Prompt user", "input"),
            sdk_ref("div", "div(html)", "Render HTML", "output"),
        ];
        let indices = filter_sdk_reference_entries(&entries, "");
        assert_eq!(indices, vec![0, 1]);
    }

    #[test]
    fn filter_sdk_reference_entries_whitespace_filter_returns_all_indices() {
        let entries = vec![sdk_ref("arg", "arg(p)", "Prompt", "input")];
        let indices = filter_sdk_reference_entries(&entries, "   ");
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn filter_sdk_reference_entries_matches_case_insensitively_across_fields() {
        let entries = vec![
            sdk_ref("arg", "arg(prompt)", "Prompts the user", "input"),
            sdk_ref("div", "div(html)", "Renders HTML content", "output"),
            sdk_ref("path", "path(opts)", "File picker", "input"),
        ];
        assert_eq!(filter_sdk_reference_entries(&entries, "INPUT"), vec![0, 2]);
        assert_eq!(filter_sdk_reference_entries(&entries, "html"), vec![1]);
        assert_eq!(filter_sdk_reference_entries(&entries, "picker"), vec![2]);
        assert_eq!(
            filter_sdk_reference_entries(&entries, "no-such-thing"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn format_sdk_reference_entry_markdown_contains_all_fields() {
        let entry = sdk_ref(
            "arg",
            "arg(prompt: string)",
            "Prompts the user for input",
            "input",
        );
        let md = format_sdk_reference_entry_markdown(&entry);
        assert!(md.contains("# arg"), "missing heading: {md}");
        assert!(
            md.contains("`arg(prompt: string)`"),
            "missing signature: {md}"
        );
        assert!(md.contains("_input_"), "missing category: {md}");
        assert!(
            md.contains("Prompts the user for input"),
            "missing description: {md}"
        );
        assert!(md.contains(COMMAND_DOCTOR_RESOURCE_URI));
        assert!(md.contains(FAILED_SCRIPTS_RESOURCE_URI));
        assert!(md.contains("host MCP resources, not callable SDK globals"));
    }

    #[test]
    fn sdk_support_serde_roundtrips_lowercase() {
        // Pins the wire shape: lowercase strings, not PascalCase.
        let supported = serde_json::to_string(&SdkSupport::Supported).expect("serialize");
        let unsupported = serde_json::to_string(&SdkSupport::Unsupported).expect("serialize");
        let experimental = serde_json::to_string(&SdkSupport::Experimental).expect("serialize");
        assert_eq!(supported, "\"supported\"");
        assert_eq!(unsupported, "\"unsupported\"");
        assert_eq!(experimental, "\"experimental\"");

        for raw in [&supported, &unsupported, &experimental] {
            let parsed: SdkSupport = serde_json::from_str(raw).expect("deserialize");
            let again = serde_json::to_string(&parsed).expect("re-serialize");
            assert_eq!(&again, raw, "round-trip mismatch for {raw}");
        }
    }

    #[test]
    fn sdk_function_ref_deserializes_old_shape_as_supported() {
        // Pins backward compatibility: older JSON without `support` still
        // parses, defaulting to Supported with no note.
        let json = r#"{
            "name": "arg",
            "signature": "arg(prompt)",
            "description": "Prompt",
            "category": "prompts"
        }"#;
        let parsed: SdkFunctionRef = serde_json::from_str(json).expect("legacy shape must parse");
        assert_eq!(parsed.support, SdkSupport::Supported);
        assert!(parsed.unsupported_note.is_none());
    }

    #[test]
    fn sdk_function_ref_always_serializes_support_field() {
        // Agents should not have to infer support from field absence.
        let entry = SdkFunctionRef::supported("arg", "arg(p)", "Prompt", "prompts");
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(
            json.contains("\"support\":\"supported\""),
            "support field must be serialized for Supported entries: {json}"
        );
        assert!(
            !json.contains("unsupportedNote"),
            "Option::None should not emit unsupportedNote: {json}"
        );
    }

    #[test]
    fn sdk_reference_marks_notify_as_supported_system_notification_api() {
        // Pins the user's correction: notify() is intentional OS-level
        // feedback (macOS Notification Center via notify-rust), distinct
        // from hud(message) which is in-launcher. Both must coexist, and
        // kit://sdk-reference must not treat notify() as a dead end.
        let doc = build_sdk_reference_document();
        let notify = doc
            .functions
            .iter()
            .find(|entry| entry.name == "notify")
            .expect("notify must appear in the SDK reference");
        assert_eq!(notify.support, SdkSupport::Supported);
        assert!(
            notify.unsupported_note.is_none(),
            "notify is Supported; it must not carry an unsupported_note"
        );
        let description = notify.description.as_str();
        assert!(
            description.to_lowercase().contains("system notification")
                || description.to_lowercase().contains("notification center"),
            "notify description must advertise it as an OS-level notification API: {description}"
        );
        assert!(
            description.contains("hud"),
            "notify description must contrast itself with hud(message) so readers can pick the right API: {description}"
        );
    }

    #[test]
    fn sdk_reference_marks_every_documented_unsupported_api() {
        let doc = build_sdk_reference_document();
        for unsupported_name in SDK_NOT_YET_IMPLEMENTED_IN_GPUI {
            let entry = doc
                .functions
                .iter()
                .find(|entry| entry.name == *unsupported_name)
                .unwrap_or_else(|| panic!("unsupported SDK API `{unsupported_name}` is missing from the author-facing reference"));
            assert_eq!(
                entry.support,
                SdkSupport::Unsupported,
                "`{unsupported_name}` appears in the unsupported inventory but is marked available in the SDK reference"
            );
            assert!(
                entry.unsupported_note.is_some(),
                "`{unsupported_name}` must carry an actionable support explanation"
            );
        }
    }

    #[test]
    fn sdk_reference_marks_implemented_prompt_variants_as_supported() {
        let doc = build_sdk_reference_document();

        for name in [
            "mini", "micro", "hotkey", "fields", "form", "select", "path",
        ] {
            let entry = doc
                .functions
                .iter()
                .find(|entry| entry.name == name)
                .unwrap_or_else(|| panic!("implemented native prompt `{name}` is missing"));

            assert_eq!(entry.support, SdkSupport::Supported);
            assert!(entry.unsupported_note.is_none());
            assert!(
                !SDK_NOT_YET_IMPLEMENTED_IN_GPUI.contains(&name),
                "implemented prompt `{name}` must not appear in the unsupported inventory"
            );
        }
    }

    #[test]
    fn sdk_capability_catalog_matches_every_reference_row_exactly_once() {
        let doc = build_sdk_reference_document();
        assert_eq!(
            doc.capability_catalog.schema_version,
            SDK_CAPABILITY_CATALOG_SCHEMA_VERSION
        );
        assert_eq!(
            doc.capability_catalog.host_version,
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(
            doc.capability_catalog.capabilities.len(),
            doc.functions.len()
        );

        let mut seen = std::collections::HashSet::new();
        for (entry, capability) in doc
            .functions
            .iter()
            .zip(doc.capability_catalog.capabilities.iter())
        {
            assert!(seen.insert(capability.name.as_str()));
            assert_eq!(capability.name, entry.name);
            assert_eq!(capability.support, entry.support);
            assert!(!capability.minimum_host_version.is_empty());
            if capability.support == SdkSupport::Unsupported {
                assert!(!capability.alternatives.is_empty());
                assert!(capability.migration_note.is_some());
            }
        }
    }

    #[test]
    fn sdk_capability_catalog_reuses_index_until_explicit_invalidation() {
        let first = sdk_capability_catalog_index();
        let second = sdk_capability_catalog_index();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            first.catalog.schema_version,
            SDK_CAPABILITY_CATALOG_SCHEMA_VERSION
        );
        assert_eq!(first.catalog.host_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(first.positions.len(), first.catalog.capabilities.len());

        let next_generation = invalidate_sdk_capability_catalog();
        let refreshed = sdk_capability_catalog_index();
        assert!(!Arc::ptr_eq(&first, &refreshed));
        assert_eq!(refreshed.generation, next_generation);
        assert_eq!(sdk_capability_catalog_generation(), next_generation);
        assert_eq!(refreshed.catalog, first.catalog);
    }

    #[test]
    fn sdk_capability_catalog_declares_native_permission_and_platform_boundaries() {
        let catalog = sdk_capability_catalog();
        let move_window = catalog
            .capabilities
            .iter()
            .find(|capability| capability.name == "moveWindow")
            .expect("moveWindow capability");
        assert_eq!(move_window.required_permissions, vec!["accessibility"]);
        assert_eq!(move_window.supported_platforms, vec!["macos"]);

        let screenshot = catalog
            .capabilities
            .iter()
            .find(|capability| capability.name == "computer.captureNativeWindow")
            .expect("capture capability");
        assert_eq!(
            screenshot.required_permissions,
            vec!["accessibility", "screen-recording"]
        );

        for name in [
            "closeWindow",
            "minimizeWindow",
            "maximizeWindow",
            "moveToNextDisplay",
            "moveToPreviousDisplay",
            "getMenuBar",
            "executeMenuAction",
        ] {
            let capability = catalog
                .capabilities
                .iter()
                .find(|capability| capability.name == name)
                .unwrap_or_else(|| panic!("missing native capability `{name}`"));
            assert_eq!(capability.required_permissions, vec!["accessibility"]);
            assert_eq!(capability.supported_platforms, vec!["macos"]);
        }
    }

    #[test]
    fn sdk_capability_catalog_covers_real_namespaces_without_claiming_input_injection() {
        let catalog = sdk_capability_catalog();
        for name in [
            "exec",
            "readFile",
            "writeFile",
            "confirm",
            "chat",
            "clipboard.readImage",
            "clipboardHistoryPin",
            "chat.addMessage",
            "chat.startStream",
            "chat.getMessages",
            "chat.getResult",
            "memoryMap.get",
            "mcp.call",
            "aiGetActiveChat",
            "aiSendMessage",
        ] {
            let capability = catalog
                .capabilities
                .iter()
                .find(|capability| capability.name == name)
                .unwrap_or_else(|| panic!("missing executable capability `{name}`"));
            assert_eq!(capability.support, SdkSupport::Supported);
        }

        for name in ["keyboard", "mouse"] {
            let capability = catalog
                .capabilities
                .iter()
                .find(|capability| capability.name == name)
                .unwrap_or_else(|| panic!("missing denied input namespace `{name}`"));
            assert_eq!(capability.support, SdkSupport::Unsupported);
            assert!(!capability.alternatives.is_empty());
        }

        let paste = build_sdk_reference_document()
            .functions
            .into_iter()
            .find(|entry| entry.name == "paste")
            .expect("paste reference");
        assert_eq!(paste.signature, "await paste(): Promise<string>");
        assert!(paste.description.contains("does not inject global input"));
    }

    #[test]
    fn unsupported_sdk_capability_inventory_matches_public_author_contract() {
        let catalog = sdk_capability_catalog();
        for name in unsupported_sdk_capability_names() {
            let capability = catalog
                .capabilities
                .iter()
                .find(|capability| capability.name == *name)
                .unwrap_or_else(|| panic!("missing denied capability `{name}`"));
            assert_eq!(capability.support, SdkSupport::Unsupported);
        }
    }

    #[test]
    fn sdk_capability_transport_names_match_the_typescript_wire_contract() {
        for (topology, expected) in [
            (SdkExecutionTopology::TypeScriptScript, "typescript-script"),
            (
                SdkExecutionTopology::TypeScriptScriptlet,
                "typescript-scriptlet",
            ),
            (
                SdkExecutionTopology::TypeScriptScriptletInteractive,
                "typescript-scriptlet-interactive",
            ),
            (SdkExecutionTopology::ShellScriptlet, "shell-scriptlet"),
            (SdkExecutionTopology::PythonScriptlet, "python-scriptlet"),
        ] {
            assert_eq!(
                serde_json::to_value(topology).expect("serialize topology"),
                serde_json::json!(expected)
            );
        }
    }

    #[test]
    fn sdk_reference_deserializes_legacy_documents_without_a_capability_catalog() {
        let mut legacy = serde_json::to_value(build_sdk_reference_document())
            .expect("serialize current SDK reference");
        legacy
            .as_object_mut()
            .expect("reference object")
            .remove("capabilityCatalog");
        let restored: SdkReferenceDocument =
            serde_json::from_value(legacy).expect("legacy SDK reference remains readable");
        assert!(restored.capability_catalog.capabilities.is_empty());
    }

    #[test]
    fn sdk_capability_diagnostics_reject_unsupported_apis_before_dispatch() {
        for name in [
            "widget",
            "setPanel",
            "keyboard.type",
            "mouse.leftClick",
            "find",
        ] {
            let diagnostic = diagnose_sdk_capability(name, SdkExecutionTopology::TypeScriptScript)
                .unwrap_or_else(|| panic!("unsupported capability `{name}` needs a diagnostic"));
            assert_eq!(
                diagnostic.code,
                SdkCapabilityDiagnosticCode::UnsupportedCapability
            );
            assert!(!diagnostic.alternatives.is_empty());
        }

        assert!(diagnose_sdk_capability("mini", SdkExecutionTopology::TypeScriptScript).is_none());
        assert!(
            diagnose_sdk_capability("fields", SdkExecutionTopology::TypeScriptScript).is_none()
        );
    }

    #[test]
    fn sdk_capability_diagnostics_reject_impossible_scriptlet_prompt_topologies() {
        let interactive = diagnose_sdk_capability("arg", SdkExecutionTopology::TypeScriptScriptlet)
            .expect("interactive scriptlet prompt must fail closed");
        assert_eq!(
            interactive.code,
            SdkCapabilityDiagnosticCode::InteractivePromptUnavailable
        );
        assert!(interactive.message.contains("stdin"));

        assert!(diagnose_sdk_capability(
            "arg",
            SdkExecutionTopology::TypeScriptScriptletInteractive,
        )
        .is_none());
        assert!(diagnose_sdk_capability(
            "chat.startStream",
            SdkExecutionTopology::TypeScriptScriptletInteractive,
        )
        .is_none());

        for topology in [
            SdkExecutionTopology::ShellScriptlet,
            SdkExecutionTopology::PythonScriptlet,
        ] {
            let unavailable = diagnose_sdk_capability("home", topology)
                .expect("non-TypeScript scriptlets have no SDK transport");
            assert_eq!(
                unavailable.code,
                SdkCapabilityDiagnosticCode::MissingSdkTransport
            );
        }

        assert!(
            diagnose_sdk_capability("home", SdkExecutionTopology::TypeScriptScriptlet).is_none()
        );

        let active_chat = diagnose_sdk_capability(
            "chat.startStream",
            SdkExecutionTopology::TypeScriptScriptlet,
        )
        .expect("inline-chat mutations require an interactive active chat session");
        assert_eq!(
            active_chat.code,
            SdkCapabilityDiagnosticCode::InteractivePromptUnavailable
        );
        assert!(diagnose_sdk_capability(
            "chat.getMessages",
            SdkExecutionTopology::TypeScriptScriptlet
        )
        .is_none());
        assert!(diagnose_sdk_capability(
            "chat.getResult",
            SdkExecutionTopology::TypeScriptScriptlet
        )
        .is_none());
    }

    #[test]
    fn sdk_capability_diagnostics_reject_unknown_apis() {
        let diagnostic =
            diagnose_sdk_capability("doesNotExist", SdkExecutionTopology::TypeScriptScript)
                .expect("unknown capability must not be assumed supported");
        assert_eq!(
            diagnostic.code,
            SdkCapabilityDiagnosticCode::UnknownCapability
        );
    }

    #[test]
    fn sdk_capability_context_rejects_unknown_or_outdated_semver_fail_closed() {
        let mut host = SdkHostAvailability {
            host_version: "not-a-semver".into(),
            platform: "macos".into(),
            granted_permissions: Vec::new(),
        };
        let malformed = diagnose_sdk_capability_with_context(
            "home",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("invalid version must not pass capability preflight");
        assert_eq!(
            malformed.code,
            SdkCapabilityDiagnosticCode::InvalidHostVersion
        );

        host.host_version = "0.0.0".into();
        let outdated = diagnose_sdk_capability_with_context(
            "home",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("older host must not claim current capability support");
        assert_eq!(
            outdated.code,
            SdkCapabilityDiagnosticCode::HostVersionTooOld
        );
        assert!(outdated.message.contains("0.0.0"));
    }

    #[test]
    fn sdk_capability_context_enforces_platform_then_explicit_permission_facts() {
        let mut host = SdkHostAvailability {
            host_version: env!("CARGO_PKG_VERSION").into(),
            platform: "linux".into(),
            granted_permissions: vec!["accessibility".into()],
        };
        let unsupported = diagnose_sdk_capability_with_context(
            "moveWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("native macOS capability must reject other platforms");
        assert_eq!(
            unsupported.code,
            SdkCapabilityDiagnosticCode::UnsupportedPlatform
        );

        host.platform = "macos".into();
        host.granted_permissions.clear();
        let missing = diagnose_sdk_capability_with_context(
            "moveWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("accessibility grant is an explicit capability prerequisite");
        assert_eq!(missing.code, SdkCapabilityDiagnosticCode::MissingPermission);
        assert!(missing.message.contains("accessibility"));

        host.granted_permissions.push("accessibility".into());
        assert!(diagnose_sdk_capability_with_context(
            "moveWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .is_none());

        let capture = diagnose_sdk_capability_with_context(
            "computer.captureNativeWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("capture requires both accessibility and screen-recording");
        assert_eq!(capture.code, SdkCapabilityDiagnosticCode::MissingPermission);
        assert!(capture.message.contains("screen-recording"));

        host.granted_permissions.push("screen-recording".into());
        assert!(diagnose_sdk_capability_with_context(
            "computer.captureNativeWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .is_none());
    }

    #[test]
    fn sdk_capability_current_host_never_assumes_unknown_permission_granted() {
        assert!(diagnose_sdk_capability_for_current_host(
            "home",
            SdkExecutionTopology::TypeScriptScript,
        )
        .is_none());

        let host = SdkHostDiagnosticContext {
            host_version: env!("CARGO_PKG_VERSION"),
            platform: "macos",
            granted_permissions: None,
        };
        let pending = diagnose_sdk_capability_inner(
            "moveWindow",
            SdkExecutionTopology::TypeScriptScript,
            Some(host),
        )
        .expect("unknown permission inventory must never be treated as a grant");
        assert_eq!(
            pending.code,
            SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable
        );
        assert!(pending
            .message
            .contains("no already-known permission inventory"));
        assert!(!pending.message.contains("has not granted"));
    }

    #[test]
    fn sdk_capability_context_preserves_topology_and_unsupported_precedence() {
        let host = SdkHostAvailability {
            host_version: "not-a-semver".into(),
            platform: "linux".into(),
            granted_permissions: Vec::new(),
        };
        let denied = diagnose_sdk_capability_with_context(
            "keyboard.type",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("unsupported global input must reject before inspecting host facts");
        assert_eq!(
            denied.code,
            SdkCapabilityDiagnosticCode::UnsupportedCapability
        );

        let missing_transport = diagnose_sdk_capability_with_context(
            "home",
            SdkExecutionTopology::ShellScriptlet,
            &host,
        )
        .expect("missing SDK transport must reject before inspecting host facts");
        assert_eq!(
            missing_transport.code,
            SdkCapabilityDiagnosticCode::MissingSdkTransport
        );
    }

    #[test]
    fn sdk_host_availability_wire_is_explicit_and_does_not_probe_permissions() {
        let host = SdkHostAvailability::current(vec!["accessibility".into()]);
        assert_eq!(host.host_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(host.platform, std::env::consts::OS);
        assert_eq!(host.granted_permissions, vec!["accessibility"]);

        let encoded = serde_json::to_value(&host).expect("serialize host availability");
        assert_eq!(encoded["hostVersion"], env!("CARGO_PKG_VERSION"));
        assert_eq!(encoded["platform"], std::env::consts::OS);
        assert_eq!(
            encoded["grantedPermissions"],
            serde_json::json!(["accessibility"])
        );
    }

    #[test]
    fn sdk_reference_marks_find_as_unsupported_prompt_gap() {
        let doc = build_sdk_reference_document();
        let find = doc
            .functions
            .iter()
            .find(|entry| entry.name == "find")
            .expect("find must appear in the SDK reference");
        assert_eq!(find.support, SdkSupport::Unsupported);
        let note = find
            .unsupported_note
            .as_deref()
            .expect("find must explain its unsupported GPUI boundary");
        assert!(
            note.contains("fileSearch") && note.contains("onlyin"),
            "find unsupported note must point users to the supported onlyin-capable fileSearch API: {note}"
        );
        assert!(
            find.description
                .to_lowercase()
                .contains("does not currently implement"),
            "find description must not imply a working GPUI prompt: {}",
            find.description
        );
    }

    #[test]
    fn filter_sdk_reference_entries_includes_unsupported_results() {
        // Pins: unsupported entries stay discoverable. Filtering does NOT
        // skip them — the label is the only thing that changes.
        let entries = vec![
            sdk_ref("arg", "arg(prompt)", "Prompt user", "prompts"),
            SdkFunctionRef::unsupported(
                "notify",
                "notify(message)",
                "Show notification",
                "feedback",
                "Use hud(...) in GPUI today.",
            ),
        ];
        assert_eq!(filter_sdk_reference_entries(&entries, "notify"), vec![1]);
        assert_eq!(
            filter_sdk_reference_entries(&entries, "hud"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn format_sdk_reference_entry_markdown_warns_for_unsupported() {
        let entry = SdkFunctionRef::unsupported(
            "notify",
            "notify(message)",
            "Show notification",
            "feedback",
            "Use hud(message) instead.",
        );
        let md = format_sdk_reference_entry_markdown(&entry);
        assert!(
            md.starts_with("> ⚠ Unsupported in GPUI"),
            "unsupported entry markdown must lead with a blockquote warning: {md}"
        );
        assert!(
            md.contains("Use hud(message) instead."),
            "unsupported entry markdown must surface the note: {md}"
        );
        // Body sections still present.
        assert!(md.contains("# notify"), "missing heading: {md}");
        assert!(md.contains("`notify(message)`"), "missing signature: {md}");
        assert!(md.contains("_feedback_"), "missing category: {md}");
        assert!(
            md.contains("Show notification"),
            "missing description: {md}"
        );
    }

    #[test]
    fn format_sdk_reference_entry_markdown_does_not_warn_for_supported() {
        let entry = sdk_ref("arg", "arg(p)", "Prompt", "prompts");
        let md = format_sdk_reference_entry_markdown(&entry);
        assert!(
            !md.contains("Unsupported in GPUI"),
            "supported entry markdown must not carry an unsupported warning: {md}"
        );
    }

    #[test]
    fn sdk_reference_supported_count_exceeds_unsupported_count() {
        let doc = build_sdk_reference_document();
        let supported = doc
            .functions
            .iter()
            .filter(|f| f.support == SdkSupport::Supported)
            .count();
        let unsupported = doc
            .functions
            .iter()
            .filter(|f| f.support == SdkSupport::Unsupported)
            .count();
        assert!(
            unsupported > 0,
            "at least one SDK entry (notify) must be labeled unsupported"
        );
        assert!(
            supported > unsupported,
            "SDK reference is meant to guide authors to working APIs: supported ({supported}) should exceed unsupported ({unsupported})"
        );
    }

    #[test]
    fn sdk_reference_schema_version_is_six() {
        // Pin the current schema version so any accidental bump is visible
        // in the diff and stays paired with an envelope-shape change.
        assert_eq!(SDK_REFERENCE_SCHEMA_VERSION, 6);
    }

    #[test]
    fn script_templates_do_not_reference_unsupported_sdk_apis() {
        // Starter templates cannot silently depend on a stub SDK API. If a
        // future template calls e.g. `notify(...)` or `keyboard.type(...)`,
        // this test must fail so the template author either chooses a
        // working API or we intentionally upgrade the SDK entry's support
        // status first.
        let templates = build_script_templates_document().templates;
        let needles = unsupported_sdk_reference_scan_needles();
        assert!(
            !needles.is_empty(),
            "needle list must be non-empty — if every SDK entry becomes Supported, the needle builder drifted and this test becomes a no-op"
        );
        for template in &templates {
            let rendered = render_script_template_file(template, "Demo");
            for needle in &needles {
                assert!(
                    !rendered.contains(needle.as_str()),
                    "Template `{}` references unsupported SDK API `{needle}`. Rendered body:\n{rendered}",
                    template.id
                );
            }
        }
    }

    #[test]
    fn harness_workflow_examples_do_not_reference_unsupported_sdk_apis() {
        // The kit://sdk-reference harness workflow ships concrete example
        // scripts (test-script + scriptlet) that agents and users copy
        // verbatim. After i008 started flagging `notify` as Unsupported in
        // kit://sdk-reference, any example that still calls `notify(...)`
        // contradicts the product. This test pins the invariant.
        let workflow = build_harness_workflow();
        let examples: [(&str, &str); 2] = [
            ("example_test_script", workflow.example_test_script.as_str()),
            ("example_scriptlet", workflow.example_scriptlet.as_str()),
        ];
        let needles = unsupported_sdk_reference_scan_needles();
        assert!(
            !needles.is_empty(),
            "needle list must be non-empty — if every SDK entry becomes Supported, the needle builder drifted and this test becomes a no-op"
        );
        for (label, body) in &examples {
            for needle in &needles {
                assert!(
                    !body.contains(needle.as_str()),
                    "Harness workflow `{label}` references unsupported SDK API `{needle}`.\nBody:\n{body}"
                );
            }
        }
    }

    #[test]
    fn harness_workflow_example_scriptlet_uses_hud_for_feedback() {
        // Pins the intent of the copy-today's-date scriptlet: because the
        // desired feedback is launcher-local (flash a confirmation while the
        // launcher is the active surface), the canonical example uses
        // `hud(...)` rather than `notify(...)`. `notify(...)` is a
        // Supported, real OS-notification API — equally legitimate when the
        // caller wants Notification Center delivery that lasts past a dismiss
        // — but mixing it into this example would misinform authors about
        // when to pick each one.
        let workflow = build_harness_workflow();
        assert!(
            workflow
                .example_scriptlet
                .contains("hud(\"Copied today's date\")"),
            "example_scriptlet must give launcher-local feedback via `hud(...)`; reach for `notify(...)` only when you want OS Notification Center delivery.\nBody:\n{}",
            workflow.example_scriptlet
        );
        assert!(
            !workflow.example_scriptlet.contains("notify("),
            "example_scriptlet must not call `notify(...)`; this copy-date scriptlet is a launcher-local feedback example — `hud(message)` is the right choice here.\nBody:\n{}",
            workflow.example_scriptlet
        );
    }

    // =======================================================
    // kit://script-templates resource tests
    // =======================================================

    fn template_ref(id: &str, title: &str, description: &str, category: &str) -> ScriptTemplateRef {
        ScriptTemplateRef {
            id: id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            category: category.to_string(),
            filename_hint: id.to_string(),
            body_template: "// placeholder for {{NAME}}\n".to_string(),
            metadata_defaults: ScriptTemplateMetadataDefaults::default(),
        }
    }

    #[test]
    fn script_templates_document_has_schema_version_and_templates() {
        let doc = build_script_templates_document();
        assert_eq!(doc.schema_version, SCRIPT_TEMPLATES_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, doc.templates.len());
        assert!(
            !doc.templates.is_empty(),
            "v1 should ship at least one starter template"
        );
        // Blank Starter must stay in row #1 so the fast path feels identical
        // to the pre-catalog experience.
        assert_eq!(
            doc.templates[0].id, "blank-starter",
            "Blank Starter must be the first row"
        );
    }

    #[test]
    fn every_starter_template_declares_only_real_supported_host_capabilities() {
        for template in build_script_templates_document().templates {
            let source = render_script_template_file(&template, "Compatibility Fixture");
            let parsed = crate::metadata_parser::extract_typed_metadata(&source);
            assert!(
                parsed.errors.is_empty(),
                "template {} has malformed metadata: {:?}",
                template.id,
                parsed.errors
            );
            let metadata = parsed
                .metadata
                .expect("starter template declares typed metadata");
            assert_eq!(
                metadata.extra.get("sdkCapabilities"),
                Some(&serde_json::json!(["arg", "div", "md"])),
                "template {} must truthfully declare the globals it invokes",
                template.id
            );
            assert_eq!(
                metadata.extra.get("executionTopology"),
                Some(&serde_json::json!("typescript-script")),
                "template {} must declare its real interactive script transport",
                template.id
            );

            let script = Script {
                name: "Compatibility Fixture".to_string(),
                path: std::path::PathBuf::from("compatibility-fixture.ts"),
                extension: "ts".to_string(),
                plugin_id: "main".to_string(),
                typed_metadata: Some(metadata),
                ..Script::default()
            };
            assert!(
                crate::scripts::validate_declared_sdk_capabilities(&script).is_empty(),
                "template {} must satisfy its actual host capability contract",
                template.id
            );
        }
    }

    #[test]
    fn script_template_ids_are_unique() {
        let doc = build_script_templates_document();
        let mut ids: Vec<&str> = doc.templates.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        let original_len = ids.len();
        ids.dedup();
        assert_eq!(
            ids.len(),
            original_len,
            "Template ids must be unique: {ids:?}"
        );
    }

    #[test]
    fn filter_script_template_entries_matches_title_description_and_category() {
        let entries = vec![
            template_ref("t-1", "Blank Starter", "Empty shape", "starter"),
            template_ref("t-2", "Choice List", "Pick one from a list", "prompts"),
            template_ref("t-3", "Daily Note", "Writes today's text", "files"),
        ];
        let all = filter_script_template_entries(&entries, "");
        assert_eq!(all, vec![0, 1, 2]);
        let whitespace = filter_script_template_entries(&entries, "   ");
        assert_eq!(whitespace, vec![0, 1, 2]);

        // Title match (case-insensitive).
        assert_eq!(filter_script_template_entries(&entries, "CHOICE"), vec![1]);
        // Description match.
        assert_eq!(filter_script_template_entries(&entries, "today"), vec![2]);
        // Category match.
        assert_eq!(filter_script_template_entries(&entries, "starter"), vec![0]);
        // No matches.
        assert_eq!(
            filter_script_template_entries(&entries, "no-such-thing"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn render_script_template_file_includes_metadata_name() {
        let template = ScriptTemplateRef {
            id: "demo".into(),
            title: "Demo".into(),
            description: "test".into(),
            category: "starter".into(),
            filename_hint: "demo".into(),
            body_template: concat!(
                "export const metadata = {\n",
                "  name: \"{{NAME}}\",\n",
                "  description: \"{{DESCRIPTION}}\",\n",
                "};\n",
            )
            .into(),
            metadata_defaults: ScriptTemplateMetadataDefaults {
                description: Some("seeded description".into()),
            },
        };
        let rendered = render_script_template_file(&template, "My Friendly Name");
        assert!(
            rendered.contains("name: \"My Friendly Name\""),
            "friendly name should be substituted into metadata.name: {rendered}"
        );
        assert!(
            rendered.contains("description: \"seeded description\""),
            "description default should be substituted: {rendered}"
        );
        assert!(
            !rendered.contains("{{NAME}}"),
            "all placeholders should be replaced: {rendered}"
        );
        assert!(
            !rendered.contains("{{DESCRIPTION}}"),
            "all placeholders should be replaced: {rendered}"
        );
    }

    #[test]
    fn render_script_template_file_escapes_valid_names_without_changing_host_metadata() {
        let friendly_names = [
            r#"John's "Favorite" Script"#,
            "Crème brûlée 東京 🦀 {draft}",
            "Literal {{DESCRIPTION}} and {{NAME}}",
            r#"Harmless"; globalThis.__scriptKitTemplateInjection = true; const text = "data"#,
        ];

        for template in build_script_templates_document().templates {
            for friendly_name in friendly_names {
                let rendered = render_script_template_file(&template, friendly_name);
                let parsed = crate::metadata_parser::extract_typed_metadata(&rendered);
                assert!(
                    parsed.errors.is_empty(),
                    "template {} must parse an accepted friendly name safely: {:?}",
                    template.id,
                    parsed.errors,
                );
                let metadata = parsed
                    .metadata
                    .expect("escaped starter must retain its real typed metadata");
                assert_eq!(metadata.name.as_deref(), Some(friendly_name));
                assert_eq!(
                    metadata.extra.get("sdkCapabilities"),
                    Some(&serde_json::json!(["arg", "div", "md"])),
                );
                assert_eq!(
                    metadata.extra.get("executionTopology"),
                    Some(&serde_json::json!("typescript-script")),
                );

                let name_line = rendered
                    .lines()
                    .find(|line| line.trim_start().starts_with("name:"))
                    .expect("starter must expose one metadata name field");
                let expected_literal = Value::String(friendly_name.to_owned()).to_string();
                assert_eq!(name_line.trim(), format!("name: {expected_literal},"));

                let script = Script {
                    name: friendly_name.to_owned(),
                    path: std::path::PathBuf::from("escaped-starter.ts"),
                    extension: "ts".to_owned(),
                    plugin_id: "main".to_owned(),
                    typed_metadata: Some(metadata),
                    ..Script::default()
                };
                assert!(
                    crate::scripts::validate_declared_sdk_capabilities(&script).is_empty(),
                    "escaping must preserve the actual supported starter capabilities"
                );
            }
        }
    }

    #[test]
    fn render_script_template_file_never_recursively_expands_name_or_description_data() {
        let mut template = find_script_template("blank-starter")
            .expect("the real first-run starter must remain available");
        let friendly_name = r#"Keep {{DESCRIPTION}}, {{NAME}}, {braces}, and "quotes" 東京"#;
        let description =
            "Keep {{NAME}}, {{DESCRIPTION}}, braces {}, \\slashes\\, \"quotes\", and\nnewlines";
        template.metadata_defaults.description = Some(description.to_owned());

        let rendered = render_script_template_file(&template, friendly_name);
        let parsed = crate::metadata_parser::extract_typed_metadata(&rendered);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let metadata = parsed
            .metadata
            .expect("both escaped template fields must parse as ordinary strings");
        assert_eq!(metadata.name.as_deref(), Some(friendly_name));
        assert_eq!(metadata.description.as_deref(), Some(description));

        let expected_name = Value::String(friendly_name.to_owned()).to_string();
        let expected_description = Value::String(description.to_owned()).to_string();
        assert!(rendered.contains(&format!("  name: {expected_name},\n")));
        assert!(rendered.contains(&format!("  description: {expected_description},\n")));
        assert!(rendered.contains("{{NAME}}"));
        assert!(rendered.contains("{{DESCRIPTION}}"));
    }

    #[test]
    fn render_script_template_file_keeps_statement_injection_inside_one_string_literal() {
        let template = find_script_template("choice-list")
            .expect("the production choice-list starter must remain available");
        let friendly_name = r#"Safe"}; globalThis.__SCRIPT_KIT_HOSTILE__ = true; {"name":"Again"#;
        let rendered = render_script_template_file(&template, friendly_name);

        let parsed = crate::metadata_parser::extract_typed_metadata(&rendered);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let metadata = parsed
            .metadata
            .expect("hostile-looking text must remain one parsed metadata value");
        assert_eq!(metadata.name.as_deref(), Some(friendly_name));
        assert_eq!(
            metadata.extra.get("sdkCapabilities"),
            Some(&serde_json::json!(["arg", "div", "md"])),
        );

        let expected_literal = Value::String(friendly_name.to_owned()).to_string();
        let name_line = rendered
            .lines()
            .find(|line| line.trim_start().starts_with("name:"))
            .expect("starter name must remain on one metadata line");
        assert_eq!(name_line.trim(), format!("name: {expected_literal},"));
        assert!(!rendered.contains("name: \"Safe\"}; globalThis"));
    }

    #[test]
    fn render_script_template_file_falls_back_to_title_when_no_description_default() {
        let mut template = ScriptTemplateRef {
            id: "demo".into(),
            title: "Demo Title".into(),
            description: "card text".into(),
            category: "starter".into(),
            filename_hint: "demo".into(),
            body_template: "{{DESCRIPTION}}".into(),
            metadata_defaults: ScriptTemplateMetadataDefaults::default(),
        };
        template.metadata_defaults.description = None;
        let rendered = render_script_template_file(&template, "unused");
        assert_eq!(
            rendered, "Demo Title",
            "missing description_default should fall back to title"
        );
    }

    #[test]
    fn find_script_template_returns_template_by_id() {
        let found = find_script_template("blank-starter").expect("blank-starter must exist");
        assert_eq!(found.id, "blank-starter");
    }

    #[test]
    fn find_script_template_returns_none_for_unknown_id() {
        assert!(find_script_template("no-such-template-id").is_none());
    }

    #[test]
    fn starter_templates_do_not_emit_collision_binding_fields() {
        let doc = build_script_templates_document();
        for template in &doc.templates {
            let rendered = render_script_template_file(template, "Demo");
            for banned in ["alias:", "shortcut:", "keyword:", "trigger:"] {
                assert!(
                    !rendered.contains(banned),
                    "Template `{}` must not emit `{}` (would be fatally hidden by validate_script_catalog). Rendered:\n{}",
                    template.id,
                    banned,
                    rendered
                );
            }
        }
    }

    #[test]
    fn script_templates_resource_is_listed_and_readable() {
        let resources = get_resource_definitions();
        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(
            uris.contains(&SCRIPT_TEMPLATES_RESOURCE_URI),
            "{SCRIPT_TEMPLATES_RESOURCE_URI} should be in resource definitions"
        );

        let content = read_resource(SCRIPT_TEMPLATES_RESOURCE_URI, &[], &[], None)
            .expect("script-templates resource should be readable");
        assert_eq!(content.uri, SCRIPT_TEMPLATES_RESOURCE_URI);
        assert_eq!(content.mime_type, "application/json");
        let doc: ScriptTemplatesResourceDocument =
            serde_json::from_str(&content.text).expect("valid JSON envelope");
        assert_eq!(doc.schema_version, SCRIPT_TEMPLATES_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, doc.templates.len());
    }
}
