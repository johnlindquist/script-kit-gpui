use super::*;

#[test]
fn test_extract_code_block_skips_metadata() {
    // This is the user's actual format - metadata block followed by paste block
    let text = r#"## Greet

```metadata
keyword: !testing
```

```paste
success!
```
"#;
    let result = extract_code_block(text);
    assert!(result.is_some());
    let (tool, code) = result.unwrap();
    assert_eq!(tool, "paste");
    assert_eq!(code, "success!");
}

#[test]
fn test_extract_code_block_skips_schema() {
    let text = r#"## Test

```schema
{"input": {"name": "string"}}
```

```ts
console.log("hello");
```
"#;
    let result = extract_code_block(text);
    assert!(result.is_some());
    let (tool, code) = result.unwrap();
    assert_eq!(tool, "ts");
    assert_eq!(code, "console.log(\"hello\");");
}

#[test]
fn test_extract_code_block_no_metadata() {
    // When there's no metadata block, should still work
    let text = r#"## Test

```paste
hello world
```
"#;
    let result = extract_code_block(text);
    assert!(result.is_some());
    let (tool, code) = result.unwrap();
    assert_eq!(tool, "paste");
    assert_eq!(code, "hello world");
}

#[test]
fn test_read_scriptlets_keeps_first_scriptlet_when_file_starts_with_heading() {
    let _lock = crate::test_utils::SK_PATH_TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    use crate::setup::SK_PATH_ENV;
    use std::fs;
    use tempfile::TempDir;

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.previous {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    let temp_dir = TempDir::new().expect("create temp dir");
    let scriptlets_dir = temp_dir
        .path()
        .join("plugins")
        .join("main")
        .join("scriptlets");
    fs::create_dir_all(&scriptlets_dir).expect("create scriptlets dir");

    let scriptlet_file = scriptlets_dir.join("scriptlets.md");
    fs::write(
        &scriptlet_file,
        r#"## First Scriptlet
```paste
one
```

## Second Scriptlet
```paste
two
```
"#,
    )
    .expect("write scriptlet file");

    let _guard = EnvVarGuard::set(SK_PATH_ENV, &temp_dir.path().to_string_lossy());
    let scriptlets = super::loading::read_scriptlets();

    let names: Vec<String> = scriptlets.iter().map(|s| s.name.clone()).collect();
    assert_eq!(names, vec!["First Scriptlet", "Second Scriptlet"]);
}

#[test]
fn power_syntax_scriptlet_examples_parse_command_slugs() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("kit-init")
        .join("scriptlets")
        .join("examples")
        .join("power-syntax.md");
    let content = std::fs::read_to_string(path).expect("read power syntax scriptlets");
    let scriptlets = crate::scriptlets::parse_markdown_as_scriptlets(&content, None);
    let commands: Vec<&str> = scriptlets
        .iter()
        .map(|scriptlet| scriptlet.command.as_str())
        .collect();

    assert!(commands.contains(&"ps-stamp"));
    assert!(commands.contains(&"ps-dupe"));
    assert_eq!(commands.len(), 2);
}

#[test]
fn parse_scriptlet_section_reads_icon_metadata() {
    let section = r#"## Tile App Left Half

<!--
description: Tile to left half of screen
icon: panel-left
-->

```ts
await tileWindow(1, 'left');
```
"#;
    let scriptlet = parse_scriptlet_section(section, None).expect("scriptlet should parse");
    assert_eq!(scriptlet.icon.as_deref(), Some("panel-left"));

    let section_without_icon = r#"## Plain

```ts
console.log("hi");
```
"#;
    let scriptlet = parse_scriptlet_section(section_without_icon, None).expect("should parse");
    assert_eq!(scriptlet.icon, None);
}

#[test]
fn window_management_scriptlets_all_declare_launcher_icons() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("kit-init")
        .join("scriptlets")
        .join("window-management")
        .join("main.md");
    let content = std::fs::read_to_string(path).expect("read window management scriptlets");
    let scriptlets = crate::scriptlets::parse_markdown_as_scriptlets(&content, None);
    assert!(!scriptlets.is_empty());

    for scriptlet in &scriptlets {
        let icon = scriptlet
            .metadata
            .extra
            .get("icon")
            .unwrap_or_else(|| panic!("scriptlet {:?} should declare an icon", scriptlet.name));
        assert!(
            crate::icons::lucide_from_str(icon).is_some(),
            "scriptlet {:?} icon {icon:?} must resolve to a Lucide icon",
            scriptlet.name
        );
    }
}

#[test]
fn section_parser_preserves_json_and_legacy_html_capability_diagnostics() {
    let _registry = crate::test_utils::SK_PATH_TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let interactive = r#"## Prompt Scriptlet
```metadata
{"sdkCapabilities":["arg"],"executionTopology":"typescript-scriptlet-interactive"}
```
```ts
await arg("Prompt");
```
"#;
    let source = std::path::Path::new("/tmp/sdk-section-interactive.md");
    let scriptlet = parse_scriptlet_section(interactive, Some(source)).expect("parse interactive");
    assert!(crate::scripts::validate_scriptlet_capabilities(&scriptlet).is_empty());

    let legacy = r#"## Invalid Shell
<!--
sdkCapabilities: ["readFile"]
-->
```bash
echo safe-fixture
```
"#;
    let source = std::path::Path::new("/tmp/sdk-section-legacy.md");
    let scriptlet = parse_scriptlet_section(legacy, Some(source)).expect("parse shell");
    let issues = crate::scripts::validate_scriptlet_capabilities(&scriptlet);
    assert_eq!(issues.len(), 1);
    assert!(matches!(
        issues[0].kind,
        crate::scripts::ScriptValidationKind::CapabilityUnavailable {
            code: crate::mcp_resources::SdkCapabilityDiagnosticCode::MissingSdkTransport,
            ..
        }
    ));
}

#[test]
fn incremental_markdown_loader_replaces_stale_diagnostics_without_hiding_rows() {
    let _registry = crate::test_utils::SK_PATH_TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let temp = tempfile::TempDir::new().expect("create isolated markdown fixture");
    let source = temp.path().join("sdk-incremental.md");
    std::fs::write(
        &source,
        r#"## Retained Command
```metadata
{"sdkCapabilities":["readFile"]}
```
```bash
echo no-sdk
```
"#,
    )
    .expect("write noninteractive fixture");

    let first = super::loading::read_scriptlets_from_file(&source);
    assert_eq!(first.len(), 1, "invalid commands must stay visible");
    assert_eq!(
        crate::scripts::validate_scriptlet_capabilities(&first[0]).len(),
        1
    );

    std::fs::write(
        &source,
        r#"## Retained Command
```metadata
{"sdkCapabilities":["arg"]}
```
```ts
await arg("Interactive");
```
"#,
    )
    .expect("write repaired interactive fixture");
    let repaired = super::loading::read_scriptlets_from_file(&source);
    assert_eq!(repaired.len(), 1);
    assert!(crate::scripts::validate_scriptlet_capabilities(&repaired[0]).is_empty());

    std::fs::write(
        &source,
        "## Retained Command\n```ts\nitems.find(item => item.ready);\n```\n",
    )
    .expect("write safe legacy fixture");
    let legacy = super::loading::read_scriptlets_from_file(&source);
    assert_eq!(legacy.len(), 1);
    assert!(crate::scripts::validate_scriptlet_capabilities(&legacy[0]).is_empty());
}

#[test]
fn full_markdown_loader_preserves_interactive_and_blocked_scriptlet_rows() {
    let _lock = crate::test_utils::SK_PATH_TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    struct SkPathGuard(Option<String>);
    impl Drop for SkPathGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.0 {
                std::env::set_var(crate::setup::SK_PATH_ENV, previous);
            } else {
                std::env::remove_var(crate::setup::SK_PATH_ENV);
            }
        }
    }

    let temp = tempfile::TempDir::new().expect("create isolated plugin tree");
    let directory = temp.path().join("plugins/main/scriptlets");
    std::fs::create_dir_all(&directory).expect("create plugin scriptlet directory");
    std::fs::write(
        directory.join("capabilities.md"),
        r#"## Interactive Prompt
```metadata
{"sdkCapabilities":["arg"]}
```
```ts
await arg("Safe launcher prompt");
```

## Impossible Legacy Prompt
```metadata
{"sdkCapabilities":["arg"],"executionTopology":"typescript-scriptlet"}
```
```ts
await arg("No response pipe");
```

## Missing Shell Transport
```metadata
{"sdkCapabilities":["readFile"]}
```
```bash
echo no-sdk
```

## Existing Legacy Command
```bash
echo untouched
```
"#,
    )
    .expect("write realistic markdown bundle");

    let guard = SkPathGuard(std::env::var(crate::setup::SK_PATH_ENV).ok());
    std::env::set_var(crate::setup::SK_PATH_ENV, temp.path());
    let generation_before = crate::scripts::scriptlet_capability_registry_generation();
    let loaded = super::loading::load_scriptlets();
    assert_eq!(loaded.len(), 4, "invalid scriptlets remain visible");
    assert!(crate::scripts::scriptlet_capability_registry_generation() > generation_before);

    let interactive = loaded
        .iter()
        .find(|entry| entry.name == "Interactive Prompt")
        .expect("launcher-interactive scriptlet");
    assert!(crate::scripts::validate_scriptlet_capabilities(interactive).is_empty());

    let legacy = loaded
        .iter()
        .find(|entry| entry.name == "Impossible Legacy Prompt")
        .expect("explicit noninteractive scriptlet remains indexed");
    assert!(matches!(
        crate::scripts::validate_scriptlet_capabilities(legacy)[0].kind,
        crate::scripts::ScriptValidationKind::CapabilityUnavailable {
            code: crate::mcp_resources::SdkCapabilityDiagnosticCode::InteractivePromptUnavailable,
            ..
        }
    ));

    let shell = loaded
        .iter()
        .find(|entry| entry.name == "Missing Shell Transport")
        .expect("shell scriptlet remains indexed");
    assert!(matches!(
        crate::scripts::validate_scriptlet_capabilities(shell)[0].kind,
        crate::scripts::ScriptValidationKind::CapabilityUnavailable {
            code: crate::mcp_resources::SdkCapabilityDiagnosticCode::MissingSdkTransport,
            ..
        }
    ));

    let old = loaded
        .iter()
        .find(|entry| entry.name == "Existing Legacy Command")
        .expect("old scriptlet remains indexed");
    assert!(crate::scripts::validate_scriptlet_capabilities(old).is_empty());
    drop(guard);
}
