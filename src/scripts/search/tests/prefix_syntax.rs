use std::path::PathBuf;

use super::super::*;

#[test]
fn test_script_passes_tag_filter() {
    use crate::metadata_parser::TypedMetadata;
    let mut script = Script {
        name: "Test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    let meta = TypedMetadata {
        tags: vec!["productivity".to_string(), "notes".to_string()],
        ..Default::default()
    };
    script.typed_metadata = Some(meta);

    let parsed = parse_query_prefix("tag:prod");
    assert!(script_passes_prefix_filter(&script, &parsed));

    let parsed_no = parse_query_prefix("tag:gaming");
    assert!(!script_passes_prefix_filter(&script, &parsed_no));
}

#[test]
fn test_script_passes_author_filter() {
    use crate::metadata_parser::TypedMetadata;
    let mut script = Script {
        name: "Test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    let meta = TypedMetadata {
        author: Some("John Lindquist".to_string()),
        ..Default::default()
    };
    script.typed_metadata = Some(meta);

    let parsed = parse_query_prefix("author:john");
    assert!(script_passes_prefix_filter(&script, &parsed));
}

#[test]
fn test_script_passes_kit_filter() {
    let script = Script {
        name: "Test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        kit_name: Some("cleanshot".to_string()),
        ..Default::default()
    };

    let parsed = parse_query_prefix("kit:cleanshot");
    assert!(script_passes_prefix_filter(&script, &parsed));

    let parsed_no = parse_query_prefix("kit:main");
    assert!(!script_passes_prefix_filter(&script, &parsed_no));
}

#[test]
fn test_script_passes_is_cron_filter() {
    use crate::metadata_parser::TypedMetadata;
    let mut script = Script {
        name: "Backup".to_string(),
        path: PathBuf::from("/backup.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    let meta = TypedMetadata {
        cron: Some("0 0 * * *".to_string()),
        ..Default::default()
    };
    script.typed_metadata = Some(meta);

    let parsed = parse_query_prefix("is:cron");
    assert!(script_passes_prefix_filter(&script, &parsed));

    let parsed_sched = parse_query_prefix("is:scheduled");
    assert!(script_passes_prefix_filter(&script, &parsed_sched));
}

#[test]
fn test_script_passes_is_bg_filter() {
    use crate::metadata_parser::TypedMetadata;
    let mut script = Script {
        name: "Monitor".to_string(),
        path: PathBuf::from("/monitor.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    let meta = TypedMetadata {
        background: true,
        ..Default::default()
    };
    script.typed_metadata = Some(meta);

    let parsed = parse_query_prefix("is:bg");
    assert!(script_passes_prefix_filter(&script, &parsed));

    let parsed_full = parse_query_prefix("is:background");
    assert!(script_passes_prefix_filter(&script, &parsed_full));
}

#[test]
fn test_script_fails_wrong_is_filter() {
    let script = Script {
        name: "Test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };

    let parsed = parse_query_prefix("is:cron");
    assert!(!script_passes_prefix_filter(&script, &parsed));
}

#[test]
fn test_scriptlet_passes_group_filter() {
    let scriptlet = Scriptlet {
        icon: None,
        name: "Deploy".to_string(),
        code: "echo deploy".to_string(),
        tool: "bash".to_string(),
        group: Some("Development".to_string()),
        description: None,
        shortcut: None,
        keyword: None,
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    };

    let parsed = parse_query_prefix("group:dev");
    assert!(scriptlet_passes_prefix_filter(&scriptlet, &parsed));
}

#[test]
fn test_scriptlet_passes_tool_filter() {
    let scriptlet = Scriptlet {
        icon: None,
        name: "Deploy".to_string(),
        code: "echo deploy".to_string(),
        tool: "bash".to_string(),
        group: None,
        description: None,
        shortcut: None,
        keyword: None,
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    };

    let parsed = parse_query_prefix("tool:bash");
    assert!(scriptlet_passes_prefix_filter(&scriptlet, &parsed));

    let parsed_display = parse_query_prefix("tool:shell");
    assert!(scriptlet_passes_prefix_filter(&scriptlet, &parsed_display));
}

#[test]
fn test_type_filter_script_excludes_scriptlets() {
    let parsed = parse_query_prefix("type:script");
    // Scripts should pass
    let script = Script {
        name: "Test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    assert!(script_passes_prefix_filter(&script, &parsed));

    // Scriptlets should not pass
    let scriptlet = Scriptlet {
        icon: None,
        name: "Snippet".to_string(),
        code: "echo hi".to_string(),
        tool: "bash".to_string(),
        group: None,
        description: None,
        shortcut: None,
        keyword: None,
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    };
    assert!(!scriptlet_passes_prefix_filter(&scriptlet, &parsed));
}

#[test]
fn test_type_filter_snippet_excludes_scripts() {
    let parsed = parse_query_prefix("type:snippet");
    let script = Script {
        name: "Test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    assert!(!script_passes_prefix_filter(&script, &parsed));
}

#[test]
fn test_no_filter_passes_everything() {
    let parsed = parse_query_prefix("hello");
    let script = Script {
        name: "Test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    assert!(script_passes_prefix_filter(&script, &parsed));
}
