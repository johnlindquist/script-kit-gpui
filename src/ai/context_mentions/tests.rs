use super::*;

#[test]
fn parse_context_mentions_extracts_resource_directives_and_keeps_body() {
    let parsed = parse_context_mentions("@selection\n@browser\nCompare these.");

    assert_eq!(parsed.cleaned_content, "Compare these.");
    assert_eq!(parsed.parts.len(), 2);
    assert_eq!(
        parsed.parts[0],
        AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=1&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0"
                    .to_string(),
            label: "Selection".to_string(),
        }
    );
    assert_eq!(
        parsed.parts[1],
        AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=0&frontmostApp=0&menuBar=0&browserUrl=1&focusedWindow=0"
                    .to_string(),
            label: "Browser URL".to_string(),
        }
    );
}

#[test]
fn parse_context_mentions_extracts_file_directive() {
    let parsed = parse_context_mentions("@file /tmp/demo.rs\nRefactor this.");

    assert_eq!(parsed.cleaned_content, "Refactor this.");
    assert_eq!(
        parsed.parts,
        vec![AiContextPart::FilePath {
            path: "/tmp/demo.rs".to_string(),
            label: "demo.rs".to_string(),
        }]
    );
}

#[test]
fn parse_context_mentions_keeps_unknown_at_lines_as_content() {
    let parsed = parse_context_mentions("@unknown\nKeep this.");

    assert_eq!(parsed.cleaned_content, "@unknown\nKeep this.");
    assert!(parsed.parts.is_empty());
}

#[test]
fn parse_context_mentions_allows_directive_only_messages() {
    let parsed = parse_context_mentions("@snapshot\n@selection");

    assert_eq!(parsed.cleaned_content, "");
    assert_eq!(parsed.parts.len(), 2);
}

#[test]
fn parse_context_mentions_handles_all_resource_directives() {
    let input = "@snapshot\n@snapshot-full\n@selection\n@browser\n@window\n@diagnostics";
    let parsed = parse_context_mentions(input);

    assert_eq!(parsed.cleaned_content, "");
    assert_eq!(parsed.parts.len(), 6);
    assert_eq!(parsed.parts[0].label(), "What I\u{2019}m Looking At");
    assert_eq!(parsed.parts[1].label(), "Everything Available");
    assert_eq!(parsed.parts[2].label(), "Selection");
    assert_eq!(parsed.parts[3].label(), "Browser URL");
    assert_eq!(parsed.parts[4].label(), "Focused Window");
    assert_eq!(parsed.parts[5].label(), "Context Diagnostics");
}

#[test]
fn parse_context_mentions_preserves_body_ordering() {
    let parsed = parse_context_mentions("Line one.\n@snapshot\nLine two.\n@selection\nLine three.");

    assert_eq!(parsed.cleaned_content, "Line one.\nLine two.\nLine three.");
    assert_eq!(parsed.parts.len(), 2);
}

#[test]
fn parse_context_mentions_handles_file_with_tab_separator() {
    let parsed = parse_context_mentions("@file\t/tmp/test.txt");

    assert_eq!(
        parsed.parts,
        vec![AiContextPart::FilePath {
            path: "/tmp/test.txt".to_string(),
            label: "test.txt".to_string(),
        }]
    );
}

#[test]
fn parse_context_mentions_ignores_empty_file_path() {
    let parsed = parse_context_mentions("@file ");

    assert!(parsed.parts.is_empty());
    assert_eq!(parsed.cleaned_content, "@file ");
}

#[test]
fn parse_context_mentions_has_parts_helper() {
    let empty = parse_context_mentions("Just text.");
    assert!(!empty.has_parts());

    let with_parts = parse_context_mentions("@snapshot\nText.");
    assert!(with_parts.has_parts());
}

#[test]
fn parse_context_mentions_accepts_legacy_context_aliases() {
    let parsed = parse_context_mentions("@context\n@context-full");

    assert_eq!(parsed.cleaned_content, "");
    assert_eq!(parsed.parts.len(), 2);
    assert_eq!(parsed.parts[0].label(), "What I\u{2019}m Looking At");
    assert_eq!(parsed.parts[1].label(), "Everything Available");
}

// ── @file: colon-prefix parsing ────────────────────────────────

#[test]
fn parse_context_mentions_extracts_file_colon_directive() {
    let parsed = parse_context_mentions("@file:/tmp/demo.rs\nRefactor this.");

    assert_eq!(parsed.cleaned_content, "Refactor this.");
    assert_eq!(
        parsed.parts,
        vec![AiContextPart::FilePath {
            path: "/tmp/demo.rs".to_string(),
            label: "demo.rs".to_string(),
        }]
    );
}

// ── Inline mention parsing ─────────────────────────────────────

#[test]
fn parse_inline_context_mentions_finds_builtins() {
    let mentions = parse_inline_context_mentions("Fix @browser and @git-status");
    assert_eq!(mentions.len(), 2);
    assert_eq!(mentions[0].token, "@browser");
    assert_eq!(mentions[0].range, 4..12);
    assert_eq!(mentions[1].token, "@git-status");
}

#[test]
fn parse_inline_context_mentions_finds_file_colon() {
    let mentions = parse_inline_context_mentions("Check @file:/tmp/demo.rs please");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@file:/tmp/demo.rs");
    assert_eq!(
        mentions[0].part,
        AiContextPart::FilePath {
            path: "/tmp/demo.rs".to_string(),
            label: "demo.rs".to_string(),
        }
    );
}

#[test]
fn parse_inline_context_mentions_ignores_unknown() {
    let mentions = parse_inline_context_mentions("Hello @unknown world");
    assert!(mentions.is_empty());
}

#[test]
fn parse_inline_context_mentions_requires_word_boundary() {
    let mentions = parse_inline_context_mentions("email@browser.com");
    assert!(mentions.is_empty());
}

#[test]
fn inline_token_at_cursor_finds_unresolved_typed_note_token() {
    let token = inline_token_at_cursor("See @note:\"Daily Standup\" soon", 13)
        .expect("Expected token at cursor");

    assert_eq!(token.token, "@note:\"Daily Standup\"");
}

#[test]
fn next_inline_token_range_wraps_when_cursor_is_after_last_token() {
    let range = next_inline_token_range("a @note:\"One\" b @skill:\"Two\"", 30)
        .expect("Expected next token");

    assert_eq!(range, 2..13);
}

#[test]
fn previous_inline_token_range_wraps_when_cursor_is_before_first_token() {
    let range = previous_inline_token_range("a @note:\"One\" b @skill:\"Two\"", 1)
        .expect("Expected previous token");

    assert_eq!(range, 16..28);
}

#[test]
fn typed_mention_token_parts_unescapes_quoted_values() {
    let (prefix, value) = typed_mention_token_parts("@note:\"Daily \\\"Standup\\\"\"")
        .expect("Expected typed token parts");

    assert_eq!(prefix, "note");
    assert_eq!(value, "Daily \"Standup\"");
}

// ── part_to_inline_token round-trip ────────────────────────────

#[test]
fn part_to_inline_token_roundtrips_builtin() {
    let part = crate::ai::context_contract::ContextAttachmentKind::Browser.part();
    let token = part_to_inline_token(&part);
    assert_eq!(token, Some("@browser".to_string()));
}

#[test]
fn part_to_inline_token_roundtrips_file() {
    let part = AiContextPart::FilePath {
        path: "/tmp/demo.rs".to_string(),
        label: "demo.rs".to_string(),
    };
    let token = part_to_inline_token(&part);
    // Canonical @file:<basename.ext> — no extension-to-prefix morph, no truncation.
    assert_eq!(token, Some("@file:demo.rs".to_string()));
}

#[test]
fn part_to_inline_token_returns_typed_token_for_ambient() {
    let part = AiContextPart::AmbientContext {
        label: "test".to_string(),
    };
    assert_eq!(part_to_inline_token(&part), Some("@env:test".to_string()));
}

#[test]
fn part_to_inline_token_uses_selected_alias_for_note_selection_text_blocks() {
    let part = AiContextPart::TextBlock {
        label: "Selected Text".to_string(),
        source: "notes://demo-id#selection=6-11".to_string(),
        text: "world".to_string(),
        mime_type: Some("text/markdown".to_string()),
    };

    assert_eq!(part_to_inline_token(&part), Some("@selected".to_string()));
}

#[test]
fn part_to_inline_token_uses_focused_target_name_instead_of_prefixed_chip_label() {
    let part = AiContextPart::FocusedTarget {
        target: crate::ai::tab_context::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "builtin".to_string(),
            semantic_id: "choice:0:theme-designer".to_string(),
            label: "Theme Designer".to_string(),
            metadata: None,
        },
        label: "Command: Theme Designer".to_string(),
    };

    assert_eq!(
        part_to_inline_token(&part),
        Some("@cmd:\"Theme Designer\"".to_string())
    );
}

#[test]
fn part_to_inline_token_distinguishes_script_and_scriptlet_targets() {
    let script = AiContextPart::FocusedTarget {
        target: crate::ai::tab_context::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "choice:0:daily-notes".to_string(),
            label: "Daily Notes".to_string(),
            metadata: None,
        },
        label: "Command: Daily Notes".to_string(),
    };

    let scriptlet = AiContextPart::FocusedTarget {
        target: crate::ai::tab_context::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "scriptlet".to_string(),
            semantic_id: "choice:1:quick-copy".to_string(),
            label: "Quick Copy".to_string(),
            metadata: None,
        },
        label: "Command: Quick Copy".to_string(),
    };

    assert_eq!(
        part_to_inline_token(&script),
        Some("@script:\"Daily Notes\"".to_string())
    );
    assert_eq!(
        part_to_inline_token(&scriptlet),
        Some("@scriptlet:\"Quick Copy\"".to_string())
    );
}

#[test]
fn part_to_inline_token_uses_note_prefix_for_note_targets() {
    let part = AiContextPart::FocusedTarget {
        target: crate::ai::tab_context::TabAiTargetContext {
            source: "NotesBrowse".to_string(),
            kind: "note".to_string(),
            semantic_id: "choice:0:agent-chat-conversation".to_string(),
            label: "Agent Chat Conversation".to_string(),
            metadata: None,
        },
        label: "Note: Agent Chat Conversation".to_string(),
    };

    assert_eq!(
        part_to_inline_token(&part),
        Some("@note:\"Agent Chat Conversation\"".to_string())
    );
}

#[test]
fn part_to_inline_token_uses_skill_prefix_for_skill_files() {
    let part = AiContextPart::SkillFile {
        path: "/tmp/SKILL.md".to_string(),
        label: "/review".to_string(),
        skill_name: "Review Diff".to_string(),
        owner_label: "Script Kit".to_string(),
        slash_name: "review".to_string(),
    };

    assert_eq!(
        part_to_inline_token(&part),
        Some("@skill:\"Review Diff\"".to_string())
    );
}

#[test]
fn part_to_inline_token_strips_known_prefixes_from_ambient_labels() {
    let part = AiContextPart::AmbientContext {
        label: "Context: Browser URL".to_string(),
    };

    assert_eq!(
        part_to_inline_token(&part),
        Some("@env:\"Browser URL\"".to_string())
    );
}

// ── Provider-backed token coverage ───────────────────────────────

#[test]
fn parse_inline_mentions_resolves_screenshot() {
    let mentions = parse_inline_context_mentions("Attach @screenshot please");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@screenshot");
    assert!(mentions[0].part.source().contains("screenshot=1"));
}

#[test]
fn parse_inline_mentions_resolves_clipboard() {
    let mentions = parse_inline_context_mentions("Check @clipboard");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@clipboard");
    assert_eq!(mentions[0].part.source(), "kit://clipboard-history");
}

#[test]
fn parse_inline_mentions_resolves_git_diff() {
    let mentions = parse_inline_context_mentions("Review @git-diff");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@git-diff");
    assert_eq!(mentions[0].part.source(), "kit://git-diff");
}

#[test]
fn parse_inline_mentions_resolves_recent_scripts() {
    let mentions = parse_inline_context_mentions("Show @recent-scripts");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@recent-scripts");
    assert_eq!(mentions[0].part.source(), "kit://scripts");
}

#[test]
fn parse_inline_mentions_resolves_calendar() {
    let _env = ProviderEnvGuard::new();
    std::env::set_var(
        "SCRIPT_KIT_CALENDAR_JSON",
        r#"{"ok":true,"available":true,"items":[{"title":"Demo"}]}"#,
    );
    let mentions = parse_inline_context_mentions("Check @calendar");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@calendar");
    assert_eq!(mentions[0].part.source(), "kit://calendar");
}

#[test]
fn parse_inline_mentions_resolves_git_status() {
    let mentions = parse_inline_context_mentions("What's in @git-status");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@git-status");
    assert_eq!(mentions[0].part.source(), "kit://git-status");
}

#[test]
fn parse_inline_mentions_resolves_processes() {
    let mentions = parse_inline_context_mentions("Show @processes");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@processes");
    assert_eq!(mentions[0].part.source(), "kit://processes");
}

#[test]
fn parse_inline_mentions_resolves_system() {
    let mentions = parse_inline_context_mentions("Show @system info");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@system");
    assert_eq!(mentions[0].part.source(), "kit://system");
}

#[test]
fn parse_context_mentions_handles_all_provider_backed_directives() {
    let _env = ProviderEnvGuard::new();
    // Set provider env vars so the gated kinds resolve.
    std::env::set_var(
        "SCRIPT_KIT_CALENDAR_JSON",
        r#"{"ok":true,"available":true,"items":[{"title":"Demo"}]}"#,
    );
    std::env::set_var(
        "SCRIPT_KIT_DICTATION_JSON",
        r#"{"ok":true,"available":true,"items":[{"text":"Hello"}]}"#,
    );
    std::env::set_var(
        "SCRIPT_KIT_NOTIFICATIONS_JSON",
        r#"{"ok":true,"available":true,"items":[{"title":"Alert"}]}"#,
    );

    let input = "@screenshot\n@clipboard\n@git-diff\n@git-status\n@recent-scripts\n@calendar\n@processes\n@system\n@notifications\n@dictation";
    let parsed = parse_context_mentions(input);

    assert_eq!(parsed.cleaned_content, "");
    assert_eq!(parsed.parts.len(), 10);
    assert_eq!(parsed.parts[0].label(), "Screenshot");
    assert_eq!(parsed.parts[1].label(), "Clipboard");
    assert_eq!(parsed.parts[2].label(), "Git Diff");
    assert_eq!(parsed.parts[3].label(), "Git Status");
    assert_eq!(parsed.parts[4].label(), "Recent Scripts");
    assert_eq!(parsed.parts[5].label(), "Calendar");
    assert_eq!(parsed.parts[6].label(), "Processes");
    assert_eq!(parsed.parts[7].label(), "System Info");
    assert_eq!(parsed.parts[8].label(), "Notifications");
    assert_eq!(parsed.parts[9].label(), "Dictation");
}

#[test]
fn parse_inline_mentions_multiple_provider_backed() {
    let mentions = parse_inline_context_mentions("Review @clipboard and @git-diff for issues");
    assert_eq!(mentions.len(), 2);
    assert_eq!(mentions[0].token, "@clipboard");
    assert_eq!(mentions[1].token, "@git-diff");
}

#[test]
fn part_to_inline_token_roundtrips_all_provider_backed() {
    use crate::ai::context_contract::ContextAttachmentKind;

    let _env = ProviderEnvGuard::new();
    // Set provider env vars so gated kinds resolve during round-trip.
    std::env::set_var(
        "SCRIPT_KIT_CALENDAR_JSON",
        r#"{"ok":true,"available":true,"items":[{"title":"Demo"}]}"#,
    );
    std::env::set_var(
        "SCRIPT_KIT_DICTATION_JSON",
        r#"{"ok":true,"available":true,"items":[{"text":"Hello"}]}"#,
    );
    std::env::set_var(
        "SCRIPT_KIT_NOTIFICATIONS_JSON",
        r#"{"ok":true,"available":true,"items":[{"title":"Alert"}]}"#,
    );

    let kinds = [
        ContextAttachmentKind::Screenshot,
        ContextAttachmentKind::Clipboard,
        ContextAttachmentKind::GitDiff,
        ContextAttachmentKind::GitStatus,
        ContextAttachmentKind::RecentScripts,
        ContextAttachmentKind::Calendar,
        ContextAttachmentKind::Processes,
        ContextAttachmentKind::System,
        ContextAttachmentKind::Notifications,
        ContextAttachmentKind::Dictation,
    ];
    for kind in kinds {
        let part = kind.part();
        let token = part_to_inline_token(&part);
        assert!(
            token.is_some(),
            "part_to_inline_token should return Some for {kind:?}"
        );
        // Verify the token round-trips through inline parse
        let mentions = parse_inline_context_mentions(&token.clone().unwrap());
        assert_eq!(
            mentions.len(),
            1,
            "token {:?} should round-trip through inline parse",
            token
        );
        assert_eq!(mentions[0].part, part, "round-trip mismatch for {kind:?}");
    }
}

// ── Quoted @file: token parsing ──────────────────────────────

#[test]
fn parses_quoted_file_inline_mentions_with_spaces() {
    let parsed = parse_inline_context_mentions(r#"Please check @file:"/tmp/My File.rs" now"#);
    assert_eq!(parsed.len(), 1);
    assert_eq!(
        parsed[0].part,
        AiContextPart::FilePath {
            path: "/tmp/My File.rs".to_string(),
            label: "My File.rs".to_string(),
        }
    );
}

#[test]
fn parses_single_quoted_file_inline_mentions() {
    let parsed = parse_inline_context_mentions("Check @file:'/tmp/My File.rs' please");
    assert_eq!(parsed.len(), 1);
    assert_eq!(
        parsed[0].part,
        AiContextPart::FilePath {
            path: "/tmp/My File.rs".to_string(),
            label: "My File.rs".to_string(),
        }
    );
}

#[test]
fn parses_quoted_file_with_escaped_quote() {
    let parsed = parse_inline_context_mentions(r#"See @file:"/tmp/has\"quote.rs" here"#);
    assert_eq!(parsed.len(), 1);
    assert_eq!(
        parsed[0].part,
        AiContextPart::FilePath {
            path: "/tmp/has\"quote.rs".to_string(),
            label: "has\"quote.rs".to_string(),
        }
    );
}

#[test]
fn formats_file_inline_mentions_with_quotes_when_needed() {
    let token = part_to_inline_token(&AiContextPart::FilePath {
        path: "/tmp/My File.rs".to_string(),
        label: "My File.rs".to_string(),
    });
    // Whitespace-containing basenames stay quoted; extension is preserved.
    assert_eq!(token.as_deref(), Some("@file:\"My File.rs\""));
}

#[test]
fn formats_file_inline_mentions_without_quotes_when_no_spaces() {
    let token = part_to_inline_token(&AiContextPart::FilePath {
        path: "/tmp/demo.rs".to_string(),
        label: "demo.rs".to_string(),
    });
    assert_eq!(token.as_deref(), Some("@file:demo.rs"));
}

#[test]
fn file_inline_token_uses_canonical_file_prefix() {
    let original = AiContextPart::FilePath {
        path: "/tmp/My File.rs".to_string(),
        label: "My File.rs".to_string(),
    };
    let token = part_to_inline_token(&original).unwrap();
    assert_eq!(token, "@file:\"My File.rs\"");
}

#[test]
fn quoted_file_and_builtin_mixed_inline() {
    let parsed = parse_inline_context_mentions(r#"Compare @file:"/tmp/My File.rs" with @browser"#);
    assert_eq!(parsed.len(), 2);
    assert_eq!(
        parsed[0].part,
        AiContextPart::FilePath {
            path: "/tmp/My File.rs".to_string(),
            label: "My File.rs".to_string(),
        }
    );
    assert_eq!(
        parsed[1].part,
        AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=0&frontmostApp=0&menuBar=0&browserUrl=1&focusedWindow=0"
                    .to_string(),
            label: "Browser URL".to_string(),
        }
    );
}

// ── Punctuation trimming ──────────────────────────────────────

#[test]
fn parse_inline_mentions_trims_trailing_comma() {
    let mentions = parse_inline_context_mentions("Check @browser, please");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@browser");
    assert_eq!(mentions[0].range, 6..14);
}

#[test]
fn parse_inline_mentions_trims_trailing_period() {
    let mentions = parse_inline_context_mentions("See @git-diff.");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@git-diff");
}

#[test]
fn parse_inline_mentions_trims_trailing_semicolon() {
    let mentions = parse_inline_context_mentions("Use @clipboard;");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@clipboard");
}

#[test]
fn parse_inline_mentions_trims_multiple_trailing_punctuation() {
    let mentions = parse_inline_context_mentions("(@browser).");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@browser");
}

// ── Canonical token tracking ──────────────────────────────────

#[test]
fn inline_mention_alias_gets_canonical_token() {
    let mentions = parse_inline_context_mentions("Use @context please");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@context");
    assert_eq!(
        mentions[0].canonical_token, "@here",
        "alias @context should have canonical token @here"
    );
}

#[test]
fn inline_mention_primary_token_is_its_own_canonical() {
    let mentions = parse_inline_context_mentions("Use @here please");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@here");
    assert_eq!(mentions[0].canonical_token, "@here");
}

#[test]
fn inline_mention_canonical_token_with_punctuation() {
    let mentions = parse_inline_context_mentions("Compare @context, @browser.");
    assert_eq!(mentions.len(), 2);
    assert_eq!(mentions[0].canonical_token, "@here");
    assert_eq!(mentions[1].canonical_token, "@browser");
}

// ── mention_range_at_cursor ──────────────────────────────────

#[test]
fn mention_range_at_cursor_returns_range_when_inside() {
    // "Fix @browser and @git-diff"
    //      ^4      ^12
    let text = "Fix @browser and @git-diff";
    let range = mention_range_at_cursor(text, 8);
    assert_eq!(range, Some(4..12));
}

#[test]
fn mention_range_at_cursor_returns_range_at_trailing_edge() {
    let text = "Fix @browser and @git-diff";
    let range = mention_range_at_cursor(text, 12);
    assert_eq!(range, Some(4..12));
}

#[test]
fn mention_range_at_cursor_returns_none_outside() {
    let text = "Fix @browser and @git-diff";
    // Cursor at 3 is in "Fix" before the @
    assert_eq!(mention_range_at_cursor(text, 3), None);
}

#[test]
fn mention_range_at_cursor_returns_none_at_start_boundary() {
    let text = "Fix @browser and @git-diff";
    // Cursor at 4 is at the leading edge (start of @browser), not inside
    assert_eq!(mention_range_at_cursor(text, 4), None);
}

#[test]
fn mention_range_at_cursor_for_quoted_file_token() {
    let text = r#"Check @file:"/tmp/My File.ts" done"#;
    // @file:"/tmp/My File.ts" spans chars 6..29
    let range = mention_range_at_cursor(text, 29);
    assert!(range.is_some());
    let r = range.unwrap();
    assert_eq!(r.start, 6);
    // The token is @file:"/tmp/My File.ts" which is 23 chars
    assert_eq!(r.end, 29);
}

// ── mention_range_for_atomic_delete ─────────────────────────

#[test]
fn atomic_delete_backspace_at_trailing_edge() {
    let text = "Fix @browser and @git-diff";
    // Backspace at trailing edge of @browser (cursor=12)
    let range = mention_range_for_atomic_delete(text, 12, false);
    assert_eq!(range, Some(4..12));
}

#[test]
fn atomic_delete_backspace_inside_token() {
    let text = "Fix @browser and @git-diff";
    let range = mention_range_for_atomic_delete(text, 8, false);
    assert_eq!(range, Some(4..12));
}

#[test]
fn atomic_delete_backspace_at_leading_edge_returns_none() {
    let text = "Fix @browser and @git-diff";
    // Backspace at leading edge (cursor=4) should NOT match
    let range = mention_range_for_atomic_delete(text, 4, false);
    assert_eq!(range, None);
}

#[test]
fn atomic_delete_forward_at_leading_edge() {
    let text = "@git-diff rest";
    // Forward delete at cursor=0 (leading edge of @git-diff)
    let range = mention_range_for_atomic_delete(text, 0, true);
    assert_eq!(range, Some(0..9));
}

#[test]
fn atomic_delete_forward_at_leading_edge_with_prefix() {
    let text = "Fix @browser and";
    // Forward delete at cursor=4 (leading edge of @browser)
    let range = mention_range_for_atomic_delete(text, 4, true);
    assert_eq!(range, Some(4..12));
}

#[test]
fn atomic_delete_forward_inside_token() {
    let text = "Fix @browser and";
    let range = mention_range_for_atomic_delete(text, 8, true);
    assert_eq!(range, Some(4..12));
}

#[test]
fn atomic_delete_forward_outside_token_returns_none() {
    let text = "Fix @browser and";
    // Forward delete at cursor=3 (before the @)
    let range = mention_range_for_atomic_delete(text, 3, true);
    assert_eq!(range, None);
}

#[test]
fn atomic_delete_forward_after_token_returns_none() {
    let text = "Fix @browser and";
    // Forward delete at cursor=13 (space after @browser)
    let range = mention_range_for_atomic_delete(text, 13, true);
    assert_eq!(range, None);
}

#[test]
fn atomic_delete_forward_respects_quoted_file_token_boundaries() {
    let text = r#"Open @file:"/tmp/my file.rs" now"#;
    let range = mention_range_for_atomic_delete(text, 5, true);
    assert_eq!(range, Some(5..28));
}

// ── Quoted @file: parse/serialize canonical tests ───────────

#[test]
fn parse_inline_mentions_supports_quoted_file_paths() {
    // Legacy @file:/path format still parses correctly.
    let mentions = parse_inline_context_mentions(r#"Use @file:"/tmp/My File.ts""#);
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, r#"@file:"/tmp/My File.ts""#);
    // Canonical token now uses typed format from part_to_inline_token.
    assert_eq!(mentions[0].canonical_token, "@file:\"My File.ts\"");
}

#[test]
fn part_to_inline_token_uses_typed_format_for_paths_with_spaces() {
    let part = AiContextPart::FilePath {
        path: "/tmp/My File.ts".to_string(),
        label: "My File.ts".to_string(),
    };
    // Canonical @file: prefix with full basename + extension; quoted because of the space.
    assert_eq!(
        part_to_inline_token(&part),
        Some("@file:\"My File.ts\"".to_string())
    );
}

#[test]
fn compact_file_alias_resolves_to_full_attached_path() {
    let full_part = AiContextPart::FilePath {
        path: "/Users/me/project/src/demo.rs".to_string(),
        label: "demo.rs".to_string(),
    };
    let token = part_to_inline_token(&full_part).expect("file parts should have inline tokens");
    assert_eq!(token, "@file:demo.rs");

    let mut aliases = std::collections::HashMap::new();
    aliases.insert(token.clone(), full_part.clone());

    let parsed = parse_inline_context_mentions_with_aliases("Review @file:demo.rs", &aliases);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].part, full_part);
    assert_eq!(parsed[0].canonical_token, token);
}

fn assert_distinct_owner_aliases_survive_parser_and_sync(
    base_token: &str,
    first: AiContextPart,
    second: AiContextPart,
) {
    use std::collections::{HashMap, HashSet};

    let second_token = format!("{base_token}-2");
    let aliases = HashMap::from([
        (base_token.to_string(), first.clone()),
        (second_token.clone(), second.clone()),
    ]);
    let text = format!("Compare {base_token} {second_token}");
    let parsed = parse_inline_context_mentions_with_aliases(&text, &aliases);

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].canonical_token, base_token);
    assert_eq!(parsed[1].canonical_token, second_token);
    assert_eq!(parsed[0].part, first);
    assert_eq!(parsed[1].part, second);

    let initial = super::sync::build_inline_mention_sync_plan_with_aliases(
        &text,
        &[],
        &HashSet::new(),
        &aliases,
    );
    assert_eq!(initial.added_parts, [first.clone(), second.clone()]);
    assert_eq!(
        initial.added_tokens,
        [base_token.to_string(), second_token.clone()]
    );

    let owned: HashSet<String> = initial.added_tokens.iter().cloned().collect();
    let attached = initial.added_parts;
    let unchanged = super::sync::build_inline_mention_sync_plan_with_aliases(
        &text, &attached, &owned, &aliases,
    );
    assert!(unchanged.added_parts.is_empty());
    assert!(unchanged.stale_indices.is_empty());
    assert!(
        super::sync::visible_context_chip_indices_with_aliases(&text, &attached, &aliases)
            .is_empty(),
        "inline owner-backed selections must not also render duplicate context chips"
    );

    let removed_first = super::sync::build_inline_mention_sync_plan_with_aliases(
        &second_token,
        &attached,
        &owned,
        &aliases,
    );
    assert_eq!(removed_first.stale_indices, [0]);
    assert!(removed_first.added_parts.is_empty());
    assert!(removed_first.desired_tokens.contains(&second_token));
    assert_eq!(
        super::sync::visible_context_chip_indices_with_aliases(&second_token, &attached, &aliases,),
        [0]
    );
}

#[test]
fn duplicate_skill_owner_aliases_remain_distinct_through_parser_and_sync() {
    assert_distinct_owner_aliases_survive_parser_and_sync(
        "@skills:Deploy",
        AiContextPart::FilePath {
            path: "/plugins/first/deploy/SKILL.md".to_string(),
            label: "Deploy".to_string(),
        },
        AiContextPart::FilePath {
            path: "/plugins/second/deploy/SKILL.md".to_string(),
            label: "Deploy".to_string(),
        },
    );
}

#[test]
fn duplicate_scriptlet_owner_aliases_remain_distinct_through_parser_and_sync() {
    assert_distinct_owner_aliases_survive_parser_and_sync(
        "@scriptlets:Deploy",
        AiContextPart::TextBlock {
            label: "Deploy".to_string(),
            source: "spine:scriptlets:/plugins/first/deploy.md".to_string(),
            text: "first owner's command".to_string(),
            mime_type: None,
        },
        AiContextPart::TextBlock {
            label: "Deploy".to_string(),
            source: "spine:scriptlets:/plugins/second/deploy.md".to_string(),
            text: "second owner's command".to_string(),
            mime_type: None,
        },
    );
}

// ── Provider-backed mention gating ──────────────────────────────

fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}

struct ProviderEnvGuard {
    _guard: std::sync::MutexGuard<'static, ()>,
    prev_calendar: Option<std::ffi::OsString>,
    prev_dictation: Option<std::ffi::OsString>,
    prev_notifications: Option<std::ffi::OsString>,
}

impl ProviderEnvGuard {
    fn new() -> Self {
        let guard = crate::test_utils::lock_provider_json_test();
        let prev_calendar = std::env::var_os("SCRIPT_KIT_CALENDAR_JSON");
        let prev_dictation = std::env::var_os("SCRIPT_KIT_DICTATION_JSON");
        let prev_notifications = std::env::var_os("SCRIPT_KIT_NOTIFICATIONS_JSON");
        std::env::remove_var("SCRIPT_KIT_DICTATION_JSON");
        std::env::remove_var("SCRIPT_KIT_CALENDAR_JSON");
        std::env::remove_var("SCRIPT_KIT_NOTIFICATIONS_JSON");
        crate::mcp_resources::clear_provider_json_slots();
        Self {
            _guard: guard,
            prev_calendar,
            prev_dictation,
            prev_notifications,
        }
    }
}

impl Drop for ProviderEnvGuard {
    fn drop(&mut self) {
        crate::mcp_resources::clear_provider_json_slots();
        restore_env("SCRIPT_KIT_DICTATION_JSON", self.prev_dictation.take());
        restore_env("SCRIPT_KIT_CALENDAR_JSON", self.prev_calendar.take());
        restore_env(
            "SCRIPT_KIT_NOTIFICATIONS_JSON",
            self.prev_notifications.take(),
        );
    }
}

#[test]
fn unavailable_provider_backed_mentions_do_not_resolve_inline() {
    let _env = ProviderEnvGuard::new();

    let mentions = parse_inline_context_mentions("Check @dictation @calendar @notifications");
    assert!(
        mentions.is_empty(),
        "manual inline mentions must not attach provider-backed fallback resources when no provider data exists"
    );
}

#[test]
fn available_provider_backed_mentions_still_resolve_inline() {
    let _env = ProviderEnvGuard::new();
    std::env::set_var(
        "SCRIPT_KIT_CALENDAR_JSON",
        r#"{"schemaVersion":1,"type":"calendar","ok":true,"available":true,"source":"env","items":[{"title":"Demo"}]}"#,
    );

    let mentions = parse_inline_context_mentions("Check @calendar");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].token, "@calendar");
    assert_eq!(mentions[0].part.source(), "kit://calendar");
}

#[test]
fn incomplete_file_token_does_not_attach_any_part() {
    let mentions = parse_inline_context_mentions("Open @file:");
    assert!(
        mentions.is_empty(),
        "an incomplete @file: token must not attach a stale FilePath part"
    );
}

#[test]
fn quoted_file_token_with_spaces_round_trips_to_typed_format() {
    // Legacy @file:/path parses to a FilePath part...
    let mentions = parse_inline_context_mentions(r#"Open @file:"/tmp/my file.rs""#);
    assert_eq!(mentions.len(), 1);
    // ...but part_to_inline_token now produces typed format.
    let token = part_to_inline_token(&mentions[0].part).expect("round-trip token");
    assert_eq!(token, "@file:\"my file.rs\"");
}

#[test]
fn placeholder_provider_envelope_does_not_resolve_inline() {
    let _env = ProviderEnvGuard::new();
    std::env::set_var(
        "SCRIPT_KIT_CALENDAR_JSON",
        r#"{"schemaVersion":1,"type":"calendar","ok":true,"available":false,"source":"env","items":[]}"#,
    );

    let mentions = parse_inline_context_mentions("Check @calendar");
    assert!(
        mentions.is_empty(),
        "placeholder calendar envelope must not make @calendar attachable"
    );
}

#[test]
fn placeholder_provider_envelope_with_items_empty_stays_unavailable() {
    let _env = ProviderEnvGuard::new();
    std::env::set_var(
        "SCRIPT_KIT_NOTIFICATIONS_JSON",
        r#"{"schemaVersion":1,"type":"notifications","ok":true,"available":true,"source":"env","items":[]}"#,
    );

    let mentions = parse_inline_context_mentions("Check @notifications");
    assert!(
        mentions.is_empty(),
        "provider-backed mentions must stay unavailable when items is empty"
    );
}

// =========================================================================
// Shared sync kernel tests
// =========================================================================

#[test]
fn inline_sync_plan_uses_canonical_tokens() {
    use super::sync::build_inline_mention_sync_plan;
    use std::collections::HashSet;

    let attached = vec![crate::ai::context_contract::ContextAttachmentKind::Current.part()];
    let owned: HashSet<String> = ["@here".to_string()].into_iter().collect();
    let plan = build_inline_mention_sync_plan("Use @context and @browser", &attached, &owned);

    assert!(plan.desired_tokens.contains("@here"));
    assert!(plan.desired_tokens.contains("@browser"));
    // @snapshot is still desired (via @context alias), so not stale.
    assert!(
        plan.stale_indices.is_empty(),
        "existing @snapshot part should not be stale when @context alias is present"
    );
    assert_eq!(plan.added_parts.len(), 1, "only @browser should be added");
    assert_eq!(
        plan.added_parts[0],
        crate::ai::context_contract::ContextAttachmentKind::Browser.part()
    );
}

#[test]
fn visible_chip_indices_hide_inline_backed_parts_only() {
    use super::sync::visible_context_chip_indices;

    let parts = vec![
        crate::ai::context_contract::ContextAttachmentKind::Browser.part(),
        crate::ai::message_parts::AiContextPart::AmbientContext {
            label: "Ask Anything".to_string(),
        },
    ];
    let visible = visible_context_chip_indices("Check @browser", &parts);
    assert_eq!(visible, vec![1]);
}

#[test]
fn remove_inline_mention_at_cursor_consumes_trailing_space() {
    use super::sync::remove_inline_mention_at_cursor;

    let (next, cursor) = remove_inline_mention_at_cursor("Fix @browser now", 8, false)
        .expect("inline token should delete atomically");
    assert_eq!(next, "Fix now");
    assert_eq!(cursor, 4);
}

#[test]
fn alias_and_primary_mentions_share_one_visible_chip_decision() {
    use super::sync::visible_context_chip_indices;

    let parts = vec![crate::ai::context_contract::ContextAttachmentKind::Current.part()];
    let visible = visible_context_chip_indices("Use @context", &parts);
    assert!(
        visible.is_empty(),
        "alias @context should suppress the Current Context chip"
    );
}

#[test]
fn sync_plan_detects_stale_tokens() {
    use super::sync::build_inline_mention_sync_plan;
    use std::collections::HashSet;

    let attached = vec![
        crate::ai::context_contract::ContextAttachmentKind::Browser.part(),
        crate::ai::context_contract::ContextAttachmentKind::Current.part(),
    ];
    let owned: HashSet<String> = ["@browser".to_string(), "@here".to_string()]
        .into_iter()
        .collect();

    // Text only has @snapshot (no @browser) — @browser should be stale.
    let plan = build_inline_mention_sync_plan("Use @snapshot", &attached, &owned);
    assert_eq!(
        plan.stale_indices,
        vec![0],
        "browser at index 0 should be stale"
    );
    assert!(plan.added_parts.is_empty());
}

#[test]
fn replace_text_in_char_range_works() {
    use super::sync::replace_text_in_char_range;

    let result = replace_text_in_char_range("hello world", 6..11, "rust");
    assert_eq!(result, "hello rust");
}

#[test]
fn caret_after_replacement_computes_correctly() {
    use super::sync::caret_after_replacement;

    let pos = caret_after_replacement(&(4..8), "@browser ");
    assert_eq!(pos, 13); // 4 + 9 chars
}
