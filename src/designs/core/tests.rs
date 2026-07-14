use super::render::root_file_display_path;
use super::*;

#[test]
fn test_all_variants_count() {
    assert_eq!(DesignVariant::all().len(), 11);
}

#[test]
fn test_keyboard_number_round_trip() {
    for num in 0..=9 {
        let variant = DesignVariant::from_keyboard_number(num);
        assert!(
            variant.is_some(),
            "Keyboard number {} should map to a variant",
            num
        );

        let v = variant.unwrap();
        let shortcut = v.shortcut_number();

        // All variants except Playful should have shortcuts
        if v != DesignVariant::Playful {
            assert!(shortcut.is_some(), "Variant {:?} should have a shortcut", v);
            assert_eq!(
                shortcut.unwrap(),
                num,
                "Round-trip failed for number {}",
                num
            );
        }
    }
}

#[test]
fn test_playful_has_no_shortcut() {
    assert_eq!(DesignVariant::Playful.shortcut_number(), None);
}

#[test]
fn test_variant_names_not_empty() {
    for variant in DesignVariant::all() {
        assert!(
            !variant.name().is_empty(),
            "Variant {:?} should have a name",
            variant
        );
        assert!(
            !variant.description().is_empty(),
            "Variant {:?} should have a description",
            variant
        );
    }
}

#[test]
fn test_default_variant() {
    assert_eq!(DesignVariant::default(), DesignVariant::Default);
}

#[test]
fn test_uses_default_renderer() {
    // Minimal and RetroTerminal now have custom renderers
    assert!(
        !uses_default_renderer(DesignVariant::Minimal),
        "Minimal should NOT use default renderer"
    );
    assert!(
        !uses_default_renderer(DesignVariant::RetroTerminal),
        "RetroTerminal should NOT use default renderer"
    );

    // Default still uses default renderer
    assert!(
        uses_default_renderer(DesignVariant::Default),
        "Default should use default renderer"
    );

    // Other variants still use default renderer (until implemented)
    assert!(uses_default_renderer(DesignVariant::Brutalist));
    assert!(uses_default_renderer(DesignVariant::NeonCyberpunk));
}

#[test]
fn test_get_item_height() {
    // Minimal uses taller items (64px)
    assert_eq!(
        get_item_height(DesignVariant::Minimal),
        get_tokens(DesignVariant::Minimal).item_height()
    );
    assert_eq!(get_item_height(DesignVariant::Minimal), 64.0);

    // RetroTerminal uses denser items (28px)
    assert_eq!(
        get_item_height(DesignVariant::RetroTerminal),
        get_tokens(DesignVariant::RetroTerminal).item_height()
    );
    assert_eq!(get_item_height(DesignVariant::RetroTerminal), 28.0);

    // Compact uses the smallest items (24px)
    assert_eq!(
        get_item_height(DesignVariant::Compact),
        get_tokens(DesignVariant::Compact).item_height()
    );
    assert_eq!(get_item_height(DesignVariant::Compact), 24.0);

    // Default and others use standard height (40px - from design tokens)
    // Note: This differs from LIST_ITEM_HEIGHT (40.0) which is used for actual rendering
    assert_eq!(get_item_height(DesignVariant::Default), 40.0);
    assert_eq!(get_item_height(DesignVariant::Brutalist), 40.0);
}

#[test]
fn test_design_variant_dispatch_coverage() {
    // Ensure all variants are covered by the dispatch logic
    // This test verifies the match arms in render_design_item cover all cases
    for variant in DesignVariant::all() {
        let uses_default = uses_default_renderer(*variant);
        let height = get_item_height(*variant);

        // All variants should have a defined height
        assert!(
            height > 0.0,
            "Variant {:?} should have positive item height",
            variant
        );

        // Minimal and RetroTerminal should use custom renderers
        if *variant == DesignVariant::Minimal || *variant == DesignVariant::RetroTerminal {
            assert!(
                !uses_default,
                "Variant {:?} should use custom renderer",
                variant
            );
        }
    }
}

#[test]
fn test_design_keyboard_coverage() {
    // Verify all keyboard shortcuts 1-0 are mapped
    let mut mapped_variants = Vec::new();
    for num in 0..=9 {
        if let Some(variant) = DesignVariant::from_keyboard_number(num) {
            mapped_variants.push(variant);
        }
    }
    // Should have 10 mapped variants (Cmd+1 through Cmd+0)
    assert_eq!(
        mapped_variants.len(),
        10,
        "Expected 10 keyboard-mapped variants"
    );

    // All mapped variants should be unique
    let mut unique = mapped_variants.clone();
    unique.sort_by_key(|v| *v as u8);
    unique.dedup_by_key(|v| *v as u8);
    assert_eq!(unique.len(), 10, "All keyboard mappings should be unique");
}

#[test]
fn test_design_cycling() {
    // Test that next() cycles through all designs
    let all = DesignVariant::all();
    let mut current = DesignVariant::Default;

    // Cycle through all designs
    for (i, expected) in all.iter().enumerate() {
        assert_eq!(
            current, *expected,
            "Cycle iteration {} should be {:?}",
            i, expected
        );
        current = current.next();
    }

    // After cycling through all, we should be back at Default
    assert_eq!(
        current,
        DesignVariant::Default,
        "Should cycle back to Default"
    );
}

#[test]
fn test_design_prev() {
    // Test that prev() goes backwards
    let current = DesignVariant::Default;
    let prev = current.prev();

    // Default.prev() should be Playful (last in list)
    assert_eq!(prev, DesignVariant::Playful);

    // And prev of that should be Compact
    assert_eq!(prev.prev(), DesignVariant::Compact);
}

// =========================================================================
// DesignTokens Tests
// =========================================================================

#[test]
fn test_get_tokens_item_height_matches() {
    // Verify token item_height matches get_item_height function
    for variant in DesignVariant::all() {
        let tokens = get_tokens(*variant);
        let fn_height = get_item_height(*variant);
        let token_height = tokens.item_height();

        assert_eq!(
            fn_height, token_height,
            "Item height mismatch for {:?}: get_item_height={}, tokens.item_height={}",
            variant, fn_height, token_height
        );
    }
}

#[test]
fn test_design_colors_defaults() {
    let colors = DesignColors::default();

    // Verify expected defaults
    assert_eq!(colors.background, 0x1e1e1e);
    assert_eq!(colors.text_primary, 0xffffff);
    assert_eq!(colors.accent, 0xfbbf24);
    assert_eq!(colors.border, 0x464647);
}

#[test]
fn test_design_spacing_defaults() {
    let spacing = DesignSpacing::default();

    // Verify expected defaults
    assert_eq!(spacing.padding_xs, 4.0);
    assert_eq!(spacing.padding_md, 12.0);
    assert_eq!(spacing.gap_md, 8.0);
    assert_eq!(spacing.item_padding_x, 16.0);
}

#[test]
fn test_design_typography_defaults() {
    let typography = DesignTypography::default();

    // Verify expected defaults
    assert_eq!(typography.font_family, ".AppleSystemUIFont");
    assert_eq!(typography.font_family_mono, "Menlo");
    assert_eq!(typography.font_size_md, 14.0);
}

#[test]
fn test_design_visual_defaults() {
    let visual = DesignVisual::default();

    // Verify expected defaults
    assert_eq!(visual.radius_sm, 4.0);
    assert_eq!(visual.radius_md, 8.0);
    assert_eq!(visual.shadow_opacity, 0.25);
    assert_eq!(visual.border_thin, 1.0);
}

#[test]
fn test_design_tokens_are_copy() {
    // Verify all token structs are Copy (needed for closure efficiency)
    fn assert_copy<T: Copy>() {}

    assert_copy::<DesignColors>();
    assert_copy::<DesignSpacing>();
    assert_copy::<DesignTypography>();
    assert_copy::<DesignVisual>();
}

#[test]
fn test_minimal_tokens_distinctive() {
    let tokens = MinimalDesignTokens;

    // Minimal should have taller items and more generous padding
    assert_eq!(tokens.item_height(), 64.0);
    assert_eq!(tokens.spacing().item_padding_x, 80.0);
    assert_eq!(tokens.visual().radius_md, 0.0); // No borders
}

#[test]
fn test_retro_terminal_tokens_distinctive() {
    let tokens = RetroTerminalDesignTokens;

    // Terminal should have dense items and phosphor green colors
    assert_eq!(tokens.item_height(), 28.0);
    assert_eq!(tokens.colors().text_primary, 0x00ff00); // Phosphor green
    assert_eq!(tokens.colors().background, 0x000000); // Pure black
    assert_eq!(tokens.typography().font_family, "Menlo");
}

#[test]
fn test_compact_tokens_distinctive() {
    let tokens = CompactDesignTokens;

    // Compact should have smallest items
    assert_eq!(tokens.item_height(), 24.0);
    assert!(tokens.spacing().padding_md < DesignSpacing::default().padding_md);
}

#[test]
fn test_all_variants_have_positive_item_height() {
    for variant in DesignVariant::all() {
        let tokens = get_tokens(*variant);
        assert!(
            tokens.item_height() > 0.0,
            "Variant {:?} has non-positive item height",
            variant
        );
    }
}

#[test]
fn test_all_variants_have_valid_colors() {
    for variant in DesignVariant::all() {
        let tokens = get_tokens(*variant);
        let colors = tokens.colors();

        // Background should be different from text (for contrast)
        assert_ne!(
            colors.background, colors.text_primary,
            "Variant {:?} has no contrast between bg and text",
            variant
        );
    }
}

// =========================================================================
// Auto-description tests
// =========================================================================

// --- merged from part_02.rs ---
use crate::metadata_parser::TypedMetadata;
use crate::scripts::{
    MatchIndices, Script, ScriptMatch, ScriptMatchKind, Scriptlet, ScriptletMatch, SearchResult,
};
use std::path::PathBuf;
use std::sync::Arc;

fn make_test_script(name: &str) -> Script {
    Script {
        name: name.to_string(),
        path: PathBuf::from(format!(
            "/test/{}.ts",
            name.to_lowercase().replace(' ', "-")
        )),
        extension: "ts".to_string(),
        ..Default::default()
    }
}

fn make_script_search_result(script: Script) -> SearchResult {
    SearchResult::Script(ScriptMatch {
        filename: format!("{}.ts", script.name.to_lowercase().replace(' ', "-")),
        script: Arc::new(script),
        score: 100,
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::default(),
        content_match: None,
        match_evidence: None,
    })
}

fn make_scriptlet_search_result(scriptlet: Scriptlet) -> SearchResult {
    SearchResult::Scriptlet(ScriptletMatch {
        scriptlet: Arc::new(scriptlet),
        score: 100,
        display_file_path: None,
        match_indices: MatchIndices::default(),
        match_evidence: None,
    })
}

#[test]
fn test_search_accessories_hide_source_hint_during_filtering() {
    let mut script = make_test_script("Clipboard Variables");
    script.kit_name = Some("clipboard".to_string());
    script.shortcut = Some("cmd shift v".to_string());
    let result = make_script_search_result(script);

    let accessories = resolve_search_accessories(&result, "clip");
    let type_accessory = accessories
        .type_accessory
        .expect("type icon should stay visible");
    assert_eq!(type_accessory.label, "Script");
    assert_eq!(type_accessory.icon_name, "file-code");
    assert_eq!(
        accessories.source_hint, None,
        "source/category metadata should be hidden during filtering"
    );
}

#[test]
fn recent_files_row_presentation_is_concise_and_nonredundant() {
    let home = dirs::home_dir().expect("test HOME");
    let path = home.join("Documents/report.pdf");
    let path = path.to_string_lossy().into_owned();
    assert_eq!(root_file_display_path(&path), "~/Documents/report.pdf");

    let result = SearchResult::File(crate::scripts::FileMatch {
        file: crate::file_search::FileResult {
            path,
            name: "report.pdf".to_string(),
            size: 0,
            modified: 0,
            file_type: crate::file_search::FileType::Document,
        },
        score: 1,
    });
    let accessories = resolve_search_accessories(&result, "files:");
    assert!(accessories.type_accessory.is_none());
    assert!(accessories.source_hint.is_none());
}

#[test]
fn test_resolve_tool_badge_hidden_during_filtering_for_scriptlets() {
    let scriptlet = Scriptlet {
        icon: None,
        name: "Paste Rich Link".to_string(),
        description: Some("Paste as markdown link".to_string()),
        code: "https://example.com".to_string(),
        tool: "paste".to_string(),
        shortcut: None,
        keyword: Some("!mdlink".to_string()),
        group: Some("Clipboard Transformations".to_string()),
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    };
    let result = make_scriptlet_search_result(scriptlet);

    assert_eq!(resolve_tool_badge(&result, true), None);
}

#[test]
fn test_resolve_tool_badge_kept_when_not_filtering_for_scriptlets() {
    let scriptlet = Scriptlet {
        icon: None,
        name: "Paste Rich Link".to_string(),
        description: Some("Paste as markdown link".to_string()),
        code: "https://example.com".to_string(),
        tool: "paste".to_string(),
        shortcut: None,
        keyword: Some("!mdlink".to_string()),
        group: Some("Clipboard Transformations".to_string()),
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    };
    let result = make_scriptlet_search_result(scriptlet);

    assert_eq!(
        resolve_tool_badge(&result, false),
        Some("paste".to_string())
    );
}

#[test]
fn test_auto_description_preserves_explicit() {
    let mut s = make_test_script("My Script");
    s.description = Some("Explicit description".to_string());
    assert_eq!(
        auto_description_for_script(&s),
        Some("Explicit description".to_string())
    );
}

#[test]
fn test_auto_description_cron() {
    let mut s = make_test_script("Daily Backup");
    s.typed_metadata = Some(TypedMetadata {
        cron: Some("0 0 * * *".to_string()),
        ..Default::default()
    });
    assert_eq!(
        auto_description_for_script(&s),
        Some("Cron: 0 0 * * *".to_string())
    );
}

#[test]
fn test_auto_description_schedule_over_cron() {
    let mut s = make_test_script("Scheduled Task");
    s.typed_metadata = Some(TypedMetadata {
        schedule: Some("every weekday at 9am".to_string()),
        cron: Some("0 9 * * 1-5".to_string()),
        ..Default::default()
    });
    // schedule takes priority over cron
    assert_eq!(
        auto_description_for_script(&s),
        Some("Scheduled: every weekday at 9am".to_string())
    );
}

#[test]
fn test_auto_description_watch() {
    let mut s = make_test_script("Config Watcher");
    s.typed_metadata = Some(TypedMetadata {
        watch: vec!["~/.config/**".to_string()],
        ..Default::default()
    });
    assert_eq!(
        auto_description_for_script(&s),
        Some("Watches: ~/.config/**".to_string())
    );
}

#[test]
fn test_auto_description_watch_truncates_long_pattern() {
    let mut s = make_test_script("Long Watcher");
    let long_pattern =
        "/very/long/path/to/some/deeply/nested/directory/with/many/levels/**/*.json".to_string();
    s.typed_metadata = Some(TypedMetadata {
        watch: vec![long_pattern],
        ..Default::default()
    });
    let desc = auto_description_for_script(&s).unwrap();
    assert!(desc.starts_with("Watches: "));
    assert!(desc.ends_with("..."));
}

#[test]
fn test_auto_description_background() {
    let mut s = make_test_script("Bg Task");
    s.typed_metadata = Some(TypedMetadata {
        background: true,
        ..Default::default()
    });
    assert_eq!(
        auto_description_for_script(&s),
        Some("Background process".to_string())
    );
}

#[test]
fn test_auto_description_system() {
    let mut s = make_test_script("Sys Handler");
    s.typed_metadata = Some(TypedMetadata {
        system: true,
        ..Default::default()
    });
    assert_eq!(
        auto_description_for_script(&s),
        Some("System event handler".to_string())
    );
}

#[test]
fn test_auto_description_filename_fallback() {
    // Script name differs from filename
    let s = make_test_script("My Script");
    // Path is /test/my-script.ts, filename is "my-script.ts", name is "My Script"
    let desc = auto_description_for_script(&s);
    assert_eq!(desc, Some("my-script.ts".to_string()));
}

#[test]
fn test_auto_description_no_filename_when_same_as_name() {
    let mut s = make_test_script("exact");
    s.path = PathBuf::from("/test/exact");
    s.name = "exact".to_string();
    // filename == name → falls through to language label (extension is "ts")
    assert_eq!(
        auto_description_for_script(&s),
        Some("TypeScript".to_string())
    );
}

// =========================================================================
// Grouped view hint tests
// =========================================================================

#[test]
fn test_hint_shortcut_shows_alias() {
    let mut s = make_test_script("Git Commit");
    s.shortcut = Some("opt g".to_string());
    s.alias = Some("gc".to_string());
    assert_eq!(grouped_view_hint_for_script(&s), Some("/gc".to_string()));
}

#[test]
fn test_hint_shortcut_falls_back_to_tags() {
    let mut s = make_test_script("Git Commit");
    s.shortcut = Some("opt g".to_string());
    s.typed_metadata = Some(TypedMetadata {
        tags: vec!["git".to_string(), "dev".to_string()],
        ..Default::default()
    });
    // No alias, so falls back to tags
    assert_eq!(
        grouped_view_hint_for_script(&s),
        Some("git · dev".to_string())
    );
}

#[test]
fn test_hint_alias_badge_shows_tags() {
    let mut s = make_test_script("Git Commit");
    s.alias = Some("gc".to_string());
    s.typed_metadata = Some(TypedMetadata {
        tags: vec!["git".to_string()],
        ..Default::default()
    });
    // Alias is badge, tags shown as hint
    assert_eq!(grouped_view_hint_for_script(&s), Some("git".to_string()));
}

#[test]
fn test_hint_alias_badge_falls_back_to_kit() {
    let mut s = make_test_script("Capture Window");
    s.alias = Some("cw".to_string());
    s.kit_name = Some("cleanshot".to_string());
    // Alias is badge, no tags, so falls back to kit name
    assert_eq!(
        grouped_view_hint_for_script(&s),
        Some("cleanshot".to_string())
    );
}

#[test]
fn test_hint_no_badge_shows_tags() {
    let mut s = make_test_script("Notes");
    s.typed_metadata = Some(TypedMetadata {
        tags: vec!["productivity".to_string(), "notes".to_string()],
        ..Default::default()
    });
    assert_eq!(
        grouped_view_hint_for_script(&s),
        Some("productivity · notes".to_string())
    );
}

#[test]
fn test_hint_no_badge_falls_back_to_kit() {
    let mut s = make_test_script("Annotate");
    s.kit_name = Some("cleanshot".to_string());
    assert_eq!(
        grouped_view_hint_for_script(&s),
        Some("cleanshot".to_string())
    );
}

#[test]
fn test_hint_main_kit_not_shown() {
    let mut s = make_test_script("Notes");
    s.kit_name = Some("main".to_string());
    // "main" kit should not produce a hint
    assert_eq!(grouped_view_hint_for_script(&s), None);
}

#[test]
fn test_scriptlet_hint_group_shown() {
    use crate::scripts::Scriptlet;
    let sl = Scriptlet {
        icon: None,
        name: "Open GitHub".to_string(),
        description: None,
        code: "open https://github.com".to_string(),
        tool: "open".to_string(),
        shortcut: None,
        keyword: None,
        group: Some("Development".to_string()),
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    };
    assert_eq!(
        grouped_view_hint_for_scriptlet(&sl),
        Some("Development".to_string())
    );
}

#[test]
fn test_scriptlet_hint_main_group_hidden() {
    use crate::scripts::Scriptlet;
    let sl = Scriptlet {
        icon: None,
        name: "Hello".to_string(),
        description: None,
        code: "echo hello".to_string(),
        tool: "bash".to_string(),
        shortcut: None,
        keyword: None,
        group: Some("main".to_string()),
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    };
    assert_eq!(grouped_view_hint_for_scriptlet(&sl), None);
}

// =========================================================================
// Enter text hint tests
// =========================================================================

#[test]
fn test_hint_enter_text_shown_as_fallback() {
    let mut s = make_test_script("Deploy");
    s.kit_name = Some("main".to_string());
    s.typed_metadata = Some(TypedMetadata {
        enter: Some("Deploy Now".to_string()),
        ..Default::default()
    });
    // No tags, main kit → falls back to enter text
    assert_eq!(
        grouped_view_hint_for_script(&s),
        Some("→ Deploy Now".to_string())
    );
}

#[test]
fn test_hint_enter_text_not_shown_for_generic_run() {
    let mut s = make_test_script("Basic");
    s.kit_name = Some("main".to_string());
    s.typed_metadata = Some(TypedMetadata {
        enter: Some("Run".to_string()),
        ..Default::default()
    });
    // "Run" is generic, should not show
    assert_eq!(grouped_view_hint_for_script(&s), None);
}

#[test]
fn test_hint_tags_preferred_over_enter_text() {
    let mut s = make_test_script("Deploy");
    s.typed_metadata = Some(TypedMetadata {
        enter: Some("Deploy Now".to_string()),
        tags: vec!["devops".to_string()],
        ..Default::default()
    });
    // Tags take priority over enter text
    assert_eq!(grouped_view_hint_for_script(&s), Some("devops".to_string()));
}

// --- merged from part_03.rs ---
// =========================================================================
// Code preview tests
// =========================================================================

fn make_test_scriptlet(name: &str, code: &str, tool: &str) -> crate::scripts::Scriptlet {
    crate::scripts::Scriptlet {
        icon: None,
        name: name.to_string(),
        description: None,
        code: code.to_string(),
        tool: tool.to_string(),
        shortcut: None,
        keyword: None,
        group: None,
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    }
}

#[test]
fn test_code_preview_shows_first_line() {
    let sl = make_test_scriptlet("Hello", "echo hello world", "bash");
    assert_eq!(
        code_preview_for_scriptlet(&sl),
        Some("echo hello world".to_string())
    );
}

#[test]
fn test_code_preview_skips_comments() {
    let sl = make_test_scriptlet(
        "Script",
        "#!/bin/bash\n# This is a comment\n// Another comment\nls -la",
        "bash",
    );
    assert_eq!(code_preview_for_scriptlet(&sl), Some("ls -la".to_string()));
}

#[test]
fn test_code_preview_empty_code() {
    let sl = make_test_scriptlet("Empty", "", "bash");
    assert_eq!(code_preview_for_scriptlet(&sl), None);
}

#[test]
fn test_code_preview_only_comments() {
    let sl = make_test_scriptlet("Comments", "# comment\n// another\n/* block */", "bash");
    assert_eq!(code_preview_for_scriptlet(&sl), None);
}

#[test]
fn test_code_preview_truncates_long_lines() {
    let long_code = "const result = await fetchDataFromRemoteServerWithComplexAuthenticationAndRetryLogic(url, options)";
    let sl = make_test_scriptlet("Long", long_code, "ts");
    let preview = code_preview_for_scriptlet(&sl).unwrap();
    assert!(preview.ends_with("..."));
    assert!(preview.chars().count() <= 60);
}

#[test]
fn test_code_preview_paste_shows_content() {
    let sl = make_test_scriptlet("Sig", "Best regards,\nJohn", "paste");
    // Short first line (< 20 chars) appends second line with arrow
    assert_eq!(
        code_preview_for_scriptlet(&sl),
        Some("Best regards, → John".to_string())
    );
}

#[test]
fn test_code_preview_open_shows_url() {
    let sl = make_test_scriptlet("GitHub", "https://github.com", "open");
    assert_eq!(
        code_preview_for_scriptlet(&sl),
        Some("https://github.com".to_string())
    );
}

// =========================================================================
// Match reason detection tests
// =========================================================================

#[test]
fn test_match_reason_name_match_returns_none() {
    let s = make_test_script("Notes");
    // Query matches name → no reason indicator needed
    assert_eq!(detect_match_reason_for_script(&s, "notes"), None);
}

#[test]
fn test_match_reason_short_query_returns_none() {
    let s = make_test_script("Notes");
    // Single char query → skip
    assert_eq!(detect_match_reason_for_script(&s, "n"), None);
}

#[test]
fn test_match_reason_tag_match() {
    let mut s = make_test_script("Daily Backup");
    s.typed_metadata = Some(TypedMetadata {
        tags: vec!["productivity".to_string()],
        ..Default::default()
    });
    assert_eq!(
        detect_match_reason_for_script(&s, "productivity"),
        Some("tag: productivity".to_string())
    );
}

#[test]
fn test_match_reason_author_match() {
    let mut s = make_test_script("My Tool");
    s.typed_metadata = Some(TypedMetadata {
        author: Some("John Lindquist".to_string()),
        ..Default::default()
    });
    assert_eq!(
        detect_match_reason_for_script(&s, "john"),
        Some("by John Lindquist".to_string())
    );
}

#[test]
fn test_match_reason_shortcut_match() {
    let mut s = make_test_script("Quick Notes");
    s.shortcut = Some("opt n".to_string());
    assert_eq!(
        detect_match_reason_for_script(&s, "opt n"),
        Some("shortcut".to_string())
    );
}

#[test]
fn test_match_reason_kit_match() {
    let mut s = make_test_script("Capture");
    s.kit_name = Some("cleanshot".to_string());
    assert_eq!(
        detect_match_reason_for_script(&s, "cleanshot"),
        Some("kit: cleanshot".to_string())
    );
}

#[test]
fn test_match_reason_main_kit_not_shown() {
    let mut s = make_test_script("Capture");
    s.kit_name = Some("main".to_string());
    assert_eq!(detect_match_reason_for_script(&s, "main"), None);
}

#[test]
fn test_scriptlet_match_reason_keyword() {
    let mut sl = make_test_scriptlet("Signature", "Best regards", "paste");
    sl.keyword = Some("!sig".to_string());
    assert_eq!(
        detect_match_reason_for_scriptlet(&sl, "!sig"),
        Some("keyword: !sig".to_string())
    );
}

#[test]
fn test_scriptlet_match_reason_code_match() {
    let sl = make_test_scriptlet("Open URL", "https://github.com", "open");
    assert_eq!(
        detect_match_reason_for_scriptlet(&sl, "github"),
        Some("code match".to_string())
    );
}

#[test]
fn test_scriptlet_match_reason_name_match_returns_none() {
    let sl = make_test_scriptlet("Open GitHub", "https://github.com", "open");
    // Query matches name → no reason indicator
    assert_eq!(detect_match_reason_for_scriptlet(&sl, "github"), None);
}

#[test]
fn test_scriptlet_match_reason_group() {
    let mut sl = make_test_scriptlet("Hello", "echo hello", "bash");
    sl.group = Some("Development".to_string());
    assert_eq!(
        detect_match_reason_for_scriptlet(&sl, "development"),
        Some("group: Development".to_string())
    );
}

// =========================================================================
// Excerpt helper tests
// =========================================================================

#[test]
fn test_excerpt_short_text_no_truncation() {
    let result = excerpt_around_match("short text", "short", 40);
    assert_eq!(result, "short text");
}

#[test]
fn test_excerpt_long_text_shows_ellipsis() {
    let text = "This is a very long description that talks about managing clipboard history and other features";
    let result = excerpt_around_match(text, "clipboard", 30);
    assert!(
        result.contains("clipboard"),
        "Excerpt should contain the matched term"
    );
    assert!(
        result.contains("..."),
        "Long text should be truncated with ellipsis"
    );
}

#[test]
fn test_excerpt_match_at_start() {
    let text = "clipboard manager that helps you organize your copy history across all apps";
    let result = excerpt_around_match(text, "clipboard", 30);
    // Match is at the start, so excerpt starts from beginning
    assert!(result.starts_with("clipboard"));
}

#[test]
fn test_excerpt_match_at_end() {
    let text = "A tool that helps you organize and manage your clipboard";
    let result = excerpt_around_match(text, "clipboard", 30);
    assert!(result.contains("clipboard"));
}

// =========================================================================
// Script match reason: description excerpt tests
// =========================================================================

#[test]
fn test_match_reason_description_excerpt() {
    let mut s = make_test_script("My Tool");
    s.description = Some("Manages clipboard history across all your devices".to_string());
    let reason = detect_match_reason_for_script(&s, "clipboard");
    assert!(
        reason.is_some(),
        "Description match should produce a reason"
    );
    let reason = reason.unwrap();
    assert!(
        reason.starts_with("desc: "),
        "Should start with 'desc: ', got: {}",
        reason
    );
    assert!(
        reason.contains("clipboard"),
        "Excerpt should contain the match term"
    );
}

#[test]
fn test_match_reason_description_not_shown_when_name_matches() {
    let mut s = make_test_script("Clipboard Manager");
    s.description = Some("Manages clipboard history".to_string());
    // Name matches "clipboard" so no reason needed
    assert_eq!(detect_match_reason_for_script(&s, "clipboard"), None);
}

// =========================================================================
// Script match reason: alias tests
// =========================================================================

#[test]
fn test_match_reason_alias_match() {
    let mut s = make_test_script("Git Commit Helper");
    s.alias = Some("gc".to_string());
    let reason = detect_match_reason_for_script(&s, "gc");
    assert_eq!(reason, Some("alias: /gc".to_string()));
}

#[test]
fn test_match_reason_alias_not_shown_when_name_matches() {
    let mut s = make_test_script("GC Cleaner");
    s.alias = Some("gc".to_string());
    // Name contains "GC" so no reason needed
    assert_eq!(detect_match_reason_for_script(&s, "gc"), None);
}

// =========================================================================
// Script match reason: path match tests
// =========================================================================

#[test]
fn test_match_reason_path_match() {
    let mut s = make_test_script("My Tool");
    s.path = std::path::PathBuf::from("/Users/john/.kenv/scripts/secret-helper.ts");
    let reason = detect_match_reason_for_script(&s, "secret-helper");
    assert_eq!(reason, Some("path match".to_string()));
}

// =========================================================================
// Scriptlet match reason: alias tests
// =========================================================================

#[test]
fn test_scriptlet_match_reason_alias() {
    let mut sl = make_test_scriptlet("Quick Paste", "Best regards", "paste");
    sl.alias = Some("qp".to_string());
    assert_eq!(
        detect_match_reason_for_scriptlet(&sl, "qp"),
        Some("alias: /qp".to_string())
    );
}

// =========================================================================
// Scriptlet match reason: tool type tests
// =========================================================================

#[test]
fn test_scriptlet_match_reason_tool_type() {
    let sl = make_test_scriptlet("Run Server", "npm start", "bash");
    let reason = detect_match_reason_for_scriptlet(&sl, "bash");
    assert!(reason.is_some(), "Tool type match should produce a reason");
    let reason = reason.unwrap();
    assert!(
        reason.starts_with("tool: "),
        "Should start with 'tool: ', got: {}",
        reason
    );
}

#[test]
fn test_scriptlet_match_reason_tool_not_shown_when_name_matches() {
    let sl = make_test_scriptlet("Bash Helper", "echo hi", "bash");
    // Name matches "bash" so no reason needed
    assert_eq!(detect_match_reason_for_scriptlet(&sl, "bash"), None);
}

// =========================================================================
// Scriptlet match reason: description excerpt tests
// =========================================================================

#[test]
fn test_scriptlet_match_reason_description_excerpt() {
    let mut sl = make_test_scriptlet("Quick Action", "echo done", "bash");
    sl.description = Some("Automates the deployment pipeline for staging".to_string());
    let reason = detect_match_reason_for_scriptlet(&sl, "deployment");
    assert!(reason.is_some());
    let reason = reason.unwrap();
    assert!(reason.starts_with("desc: "));
    assert!(reason.contains("deployment"));
}

// --- merged from part_04.rs ---
// =========================================================================
// Enhanced code preview tests (multi-line)
// =========================================================================

#[test]
fn test_code_preview_short_first_line_appends_second() {
    let sl = make_test_scriptlet("Deploy", "cd ~/projects\nnpm run build", "bash");
    let preview = code_preview_for_scriptlet(&sl).unwrap();
    assert!(
        preview.contains("\u{2192}"),
        "Short first line should append second line with arrow: {}",
        preview
    );
    assert!(preview.contains("cd ~/projects"));
    assert!(preview.contains("npm run build"));
}

#[test]
fn test_code_preview_long_first_line_no_append() {
    let sl = make_test_scriptlet(
        "Long",
        "const result = fetchData()\nconsole.log(result)",
        "ts",
    );
    let preview = code_preview_for_scriptlet(&sl).unwrap();
    // First line is > 20 chars, should NOT append second line
    assert!(
        !preview.contains("\u{2192}"),
        "Long first line should not append second: {}",
        preview
    );
}

#[test]
fn test_code_preview_short_first_only_line() {
    let sl = make_test_scriptlet("Short", "ls -la", "bash");
    let preview = code_preview_for_scriptlet(&sl).unwrap();
    // Only one line, can't append second
    assert_eq!(preview, "ls -la");
}

#[test]
fn test_code_preview_multi_line_truncates_combined() {
    let sl = make_test_scriptlet(
        "Deploy",
        "cd ~/projects\nexport NODE_ENV=production && npm run build && npm run deploy --target staging",
        "bash",
    );
    let preview = code_preview_for_scriptlet(&sl).unwrap();
    // Combined is long, should truncate
    assert!(preview.contains("\u{2192}"));
    assert!(
        preview.chars().count() <= 63,
        "Combined preview should be truncated, got {} chars: {}",
        preview.chars().count(),
        preview
    );
}

// =========================================================================
// Extension default icon tests
// =========================================================================

#[test]
fn test_extension_default_icon_shell() {
    assert_eq!(extension_default_icon("sh"), "Terminal");
    assert_eq!(extension_default_icon("bash"), "Terminal");
    assert_eq!(extension_default_icon("zsh"), "Terminal");
}

#[test]
fn test_extension_default_icon_applescript() {
    assert_eq!(extension_default_icon("applescript"), "Terminal");
    assert_eq!(extension_default_icon("scpt"), "Terminal");
}

#[test]
fn test_extension_default_icon_default_code() {
    assert_eq!(extension_default_icon("ts"), "Code");
    assert_eq!(extension_default_icon("js"), "Code");
    assert_eq!(extension_default_icon("py"), "Code");
    assert_eq!(extension_default_icon("rb"), "Code");
}

#[test]
fn test_root_file_type_svg_icon_mapping() {
    use crate::file_search::FileType;

    assert_eq!(root_file_type_svg_icon(FileType::Directory), "FolderOpen");
    assert_eq!(root_file_type_svg_icon(FileType::Application), "package");
    assert_eq!(root_file_type_svg_icon(FileType::Image), "file-image");
    assert_eq!(root_file_type_svg_icon(FileType::Document), "file-text");
    assert_eq!(root_file_type_svg_icon(FileType::Audio), "file-audio");
    assert_eq!(root_file_type_svg_icon(FileType::Video), "file-video");
    assert_eq!(root_file_type_svg_icon(FileType::File), "File");
    assert_eq!(root_file_type_svg_icon(FileType::Other), "File");
}

// =========================================================================
// Extension language label tests
// =========================================================================

#[test]
fn test_extension_language_label_typescript() {
    assert_eq!(extension_language_label("ts"), Some("TypeScript"));
    assert_eq!(extension_language_label("tsx"), Some("TypeScript"));
}

#[test]
fn test_extension_language_label_javascript() {
    assert_eq!(extension_language_label("js"), Some("JavaScript"));
    assert_eq!(extension_language_label("mjs"), Some("JavaScript"));
}

#[test]
fn test_extension_language_label_shell() {
    assert_eq!(extension_language_label("sh"), Some("Shell script"));
    assert_eq!(extension_language_label("bash"), Some("Shell script"));
    assert_eq!(extension_language_label("zsh"), Some("Zsh script"));
}

#[test]
fn test_extension_language_label_python() {
    assert_eq!(extension_language_label("py"), Some("Python script"));
}

#[test]
fn test_extension_language_label_unknown() {
    assert_eq!(extension_language_label("xyz"), None);
    assert_eq!(extension_language_label(""), None);
}

// =========================================================================
// Auto-description with language label fallback tests
// =========================================================================

#[test]
fn test_auto_description_language_label_fallback() {
    // Script with same-name filename and no metadata -> should get language label
    let script = crate::scripts::Script {
        name: "my-script".to_string(),
        path: std::path::PathBuf::from("/test/my-script.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    let desc = auto_description_for_script(&script);
    // Filename "my-script.ts" differs from name "my-script", so filename wins
    assert_eq!(desc, Some("my-script.ts".to_string()));
}

#[test]
fn test_auto_description_language_label_when_filename_matches() {
    // Script where filename equals name -> language label should appear
    // This happens when the name IS the filename (without extension somehow)
    let script = crate::scripts::Script {
        name: "my-script.ts".to_string(),
        path: std::path::PathBuf::from("/test/my-script.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    };
    let desc = auto_description_for_script(&script);
    // Filename "my-script.ts" == name "my-script.ts", so language label fallback
    assert_eq!(desc, Some("TypeScript".to_string()));
}

#[test]
fn test_auto_description_shell_script_language_label() {
    let script = crate::scripts::Script {
        name: "backup.sh".to_string(),
        path: std::path::PathBuf::from("/test/backup.sh"),
        extension: "sh".to_string(),
        ..Default::default()
    };
    let desc = auto_description_for_script(&script);
    assert_eq!(desc, Some("Shell script".to_string()));
}

#[test]
fn test_auto_description_explicit_description_unchanged() {
    let script = crate::scripts::Script {
        name: "test".to_string(),
        path: std::path::PathBuf::from("/test/test.ts"),
        extension: "ts".to_string(),
        description: Some("My custom description".to_string()),
        ..Default::default()
    };
    let desc = auto_description_for_script(&script);
    assert_eq!(desc, Some("My custom description".to_string()));
}

// =========================================================================
// Content snippet row tests
// =========================================================================

use crate::scripts::ScriptContentMatch;

/// Helper to create a ScriptMatch with content match data
fn make_content_script_match(
    name: &str,
    line_number: usize,
    line_text: &str,
    line_match_indices: Vec<usize>,
) -> ScriptMatch {
    let script = Arc::new(Script {
        name: name.to_string(),
        path: PathBuf::from(format!("/test/{}.ts", name)),
        extension: "ts".to_string(),
        description: Some("original description".to_string()),
        ..Default::default()
    });
    ScriptMatch {
        filename: format!("{}.ts", name),
        script,
        score: 5,
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::Content,
        content_match: Some(ScriptContentMatch {
            line_number,
            line_text: line_text.to_string(),
            line_match_indices,
            byte_range: 0..line_text.len(),
        }),
        match_evidence: None,
    }
}

#[test]
fn test_non_content_match_keeps_original_description() {
    let script = Arc::new(Script {
        name: "my-script".to_string(),
        path: PathBuf::from("/test/my-script.ts"),
        extension: "ts".to_string(),
        description: Some("A helpful script".to_string()),
        ..Default::default()
    });
    let sm = ScriptMatch {
        filename: "my-script.ts".to_string(),
        script,
        score: 100,
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::Name,
        content_match: None,
        match_evidence: None,
    };
    // Non-content match: description should come from auto_description_for_script
    assert_eq!(sm.match_kind, ScriptMatchKind::Name);
    assert!(sm.content_match.is_none());
    assert_eq!(sm.script.description.as_deref(), Some("A helpful script"));
}

#[test]
fn test_content_match_keeps_explicit_description_in_render_logic() {
    let sm = make_content_script_match(
        "my-script",
        15,
        "const result = searchForToken();",
        vec![15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29],
    );

    let description = auto_description_for_script(&sm.script);
    assert_eq!(description.as_deref(), Some("original description"));
}

#[test]
fn test_content_match_uses_normal_fallback_description_in_render_logic() {
    let script = Arc::new(Script {
        name: "fallback-demo".to_string(),
        path: PathBuf::from("/test/fallback-demo.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    });
    let sm = ScriptMatch {
        filename: "fallback-demo.ts".to_string(),
        script,
        score: 5,
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::Content,
        content_match: Some(ScriptContentMatch {
            line_number: 12,
            line_text: "const result = searchForToken();".to_string(),
            line_match_indices: vec![15, 16, 17],
            byte_range: 0..31,
        }),
        match_evidence: None,
    };

    let description = auto_description_for_script(&sm.script);
    assert_eq!(description.as_deref(), Some("fallback-demo.ts"));
}
