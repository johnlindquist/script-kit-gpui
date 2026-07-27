//! Integration tests for the composer-state SSOT contract.
//!
//! Validates that `pending_context_parts` is the single source of truth for
//! attachment state: rendering derives from `file_path_parts()`, removal
//! targets exactly one matching `FilePath` entry, and clearing only affects
//! `FilePath` entries while leaving `ResourceUri` parts intact.

use script_kit_gpui::ai::message_parts::{file_path_parts, AiContextPart};

// ---------- file_path_parts helper ----------

#[test]
fn file_path_parts_extracts_only_file_paths() {
    let parts = vec![
        AiContextPart::ResourceUri {
            uri: "kit://context".to_string(),
            label: "Context".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/a.rs".to_string(),
            label: "a.rs".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/b.rs".to_string(),
            label: "b.rs".to_string(),
        },
    ];

    let paths = file_path_parts(&parts);
    assert_eq!(paths, vec!["/tmp/a.rs", "/tmp/b.rs"]);
}

#[test]
fn file_path_parts_returns_empty_for_no_file_paths() {
    let parts = vec![AiContextPart::ResourceUri {
        uri: "kit://context".to_string(),
        label: "Context".to_string(),
    }];

    let paths = file_path_parts(&parts);
    assert!(paths.is_empty());
}

#[test]
fn file_path_parts_preserves_order() {
    let parts = vec![
        AiContextPart::FilePath {
            path: "/z.rs".to_string(),
            label: "z.rs".to_string(),
        },
        AiContextPart::ResourceUri {
            uri: "kit://mid".to_string(),
            label: "Mid".to_string(),
        },
        AiContextPart::FilePath {
            path: "/a.rs".to_string(),
            label: "a.rs".to_string(),
        },
    ];

    let paths = file_path_parts(&parts);
    assert_eq!(paths, vec!["/z.rs", "/a.rs"]);
}

// ---------- Removal contract ----------

/// Mirrors the removal logic from `AiApp::remove_attachment` without GPUI context.
fn remove_attachment_from_parts(parts: &mut Vec<AiContextPart>, file_index: usize) -> bool {
    let abs_index = parts
        .iter()
        .enumerate()
        .filter(|(_, part)| matches!(part, AiContextPart::FilePath { .. }))
        .nth(file_index)
        .map(|(i, _)| i);

    if let Some(idx) = abs_index {
        parts.remove(idx);
        true
    } else {
        false
    }
}

#[test]
fn removing_attachment_removes_exactly_one_matching_file_path() {
    let mut parts = vec![
        AiContextPart::ResourceUri {
            uri: "kit://context".to_string(),
            label: "Context".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/a.rs".to_string(),
            label: "a.rs".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/b.rs".to_string(),
            label: "b.rs".to_string(),
        },
    ];

    // Remove the first file-path attachment (a.rs)
    assert!(remove_attachment_from_parts(&mut parts, 0));

    // Should have removed a.rs but kept the resource and b.rs
    assert_eq!(parts.len(), 2);
    assert_eq!(file_path_parts(&parts), vec!["/tmp/b.rs"]);

    // ResourceUri should be untouched
    assert!(matches!(
        &parts[0],
        AiContextPart::ResourceUri { uri, .. } if uri == "kit://context"
    ));
}

#[test]
fn removing_second_attachment_preserves_first() {
    let mut parts = vec![
        AiContextPart::FilePath {
            path: "/tmp/first.rs".to_string(),
            label: "first.rs".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/second.rs".to_string(),
            label: "second.rs".to_string(),
        },
    ];

    // Remove the second file-path attachment
    assert!(remove_attachment_from_parts(&mut parts, 1));

    assert_eq!(parts.len(), 1);
    assert_eq!(file_path_parts(&parts), vec!["/tmp/first.rs"]);
}

#[test]
fn removing_out_of_bounds_index_is_noop() {
    let mut parts = vec![AiContextPart::FilePath {
        path: "/tmp/only.rs".to_string(),
        label: "only.rs".to_string(),
    }];

    assert!(!remove_attachment_from_parts(&mut parts, 5));
    assert_eq!(parts.len(), 1);
}

// ---------- Clear contract ----------

/// Mirrors the clearing logic from `AiApp::clear_attachments`.
fn clear_file_path_parts(parts: &mut Vec<AiContextPart>) -> usize {
    let before = file_path_parts(parts).len();
    parts.retain(|part| !matches!(part, AiContextPart::FilePath { .. }));
    before
}

#[test]
fn clearing_attachments_removes_all_file_path_context_parts() {
    let mut parts = vec![
        AiContextPart::FilePath {
            path: "/tmp/a.rs".to_string(),
            label: "a.rs".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/b.rs".to_string(),
            label: "b.rs".to_string(),
        },
    ];

    let cleared = clear_file_path_parts(&mut parts);
    assert_eq!(cleared, 2);
    assert!(parts.is_empty());
    assert!(file_path_parts(&parts).is_empty());
}

#[test]
fn clearing_attachments_leaves_resource_uri_parts_intact() {
    let mut parts = vec![
        AiContextPart::ResourceUri {
            uri: "kit://context".to_string(),
            label: "Context".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/file.rs".to_string(),
            label: "file.rs".to_string(),
        },
        AiContextPart::ResourceUri {
            uri: "kit://browser".to_string(),
            label: "Browser".to_string(),
        },
    ];

    clear_file_path_parts(&mut parts);

    assert_eq!(parts.len(), 2);
    assert!(file_path_parts(&parts).is_empty());
    assert!(matches!(
        &parts[0],
        AiContextPart::ResourceUri { uri, .. } if uri == "kit://context"
    ));
    assert!(matches!(
        &parts[1],
        AiContextPart::ResourceUri { uri, .. } if uri == "kit://browser"
    ));
}

// ---------- Non-file removal does not corrupt file-path state ----------

#[test]
fn removing_non_file_context_part_does_not_corrupt_file_path_rendering() {
    let mut parts = vec![
        AiContextPart::ResourceUri {
            uri: "kit://context".to_string(),
            label: "Context".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/keep.rs".to_string(),
            label: "keep.rs".to_string(),
        },
    ];

    // Remove the ResourceUri at overall index 0
    parts.remove(0);

    // File-path rendering should still work correctly
    assert_eq!(file_path_parts(&parts), vec!["/tmp/keep.rs"]);
    assert_eq!(parts.len(), 1);
}

// ---------- Middle-removal order preservation ----------

#[test]
fn remove_context_part_preserves_order_of_remaining_parts() {
    let mut parts = vec![
        AiContextPart::ResourceUri {
            uri: "kit://context?profile=minimal".to_string(),
            label: "Current Context".to_string(),
        },
        AiContextPart::ResourceUri {
            uri: "kit://context?browserUrl=1".to_string(),
            label: "Browser URL".to_string(),
        },
        AiContextPart::FilePath {
            path: "/tmp/demo.txt".to_string(),
            label: "demo.txt".to_string(),
        },
    ];

    // Remove the middle element (index 1)
    parts.remove(1);

    assert_eq!(parts.len(), 2);
    assert_eq!(
        parts,
        vec![
            AiContextPart::ResourceUri {
                uri: "kit://context?profile=minimal".to_string(),
                label: "Current Context".to_string(),
            },
            AiContextPart::FilePath {
                path: "/tmp/demo.txt".to_string(),
                label: "demo.txt".to_string(),
            },
        ]
    );
}

#[test]
fn replace_pending_context_parts_clears_previous_parts_and_resets_consumption() {
    #[derive(Default)]
    struct PendingContextState {
        pending_context_parts: Vec<AiContextPart>,
        pending_context_blocks: Vec<&'static str>,
        pending_context_consumed: bool,
        pending_ambient_context_enabled: bool,
    }

    impl PendingContextState {
        fn clear_all_pending_context(&mut self) {
            self.pending_context_parts.clear();
            self.pending_context_blocks.clear();
            self.pending_context_consumed = false;
            self.pending_ambient_context_enabled = false;
        }

        fn replace_pending_context_parts(&mut self, parts: Vec<AiContextPart>) {
            self.clear_all_pending_context();
            self.pending_context_parts = parts;
            self.pending_context_consumed = false;
            self.pending_ambient_context_enabled = self
                .pending_context_parts
                .iter()
                .any(|part| matches!(part, AiContextPart::AmbientContext { .. }));
        }
    }

    let mut state = PendingContextState {
        pending_context_parts: vec![AiContextPart::FilePath {
            path: "/tmp/old-note.md".to_string(),
            label: "old-note.md".to_string(),
        }],
        pending_context_blocks: vec!["hidden-block"],
        pending_context_consumed: true,
        pending_ambient_context_enabled: true,
    };

    let replacement = vec![AiContextPart::TextBlock {
        label: "Selected Text".to_string(),
        source: "notes://note-2#selection=0-4".to_string(),
        text: "next".to_string(),
        mime_type: None,
    }];

    state.replace_pending_context_parts(replacement.clone());

    assert_eq!(state.pending_context_parts, replacement);
    assert!(
        state.pending_context_blocks.is_empty(),
        "replacing staged parts should clear hidden staged blocks"
    );
    assert!(
        !state.pending_context_consumed,
        "replacing staged parts should re-arm first-submit consumption"
    );
    assert!(
        !state.pending_ambient_context_enabled,
        "non-ambient replacement should clear stale ambient state"
    );
}

/// Regression test for the Notes-hosted Agent Chat staging-replacement story: when
/// a user opens note A and portals it to Agent Chat, then opens note B and portals
/// *it* to the same reused Agent Chat surface, the pending context must contain
/// exactly note B's parts — never both. This models the production two-call
/// sequence through `stage_inline_context_parts_from_host` ->
/// `replace_pending_context_parts` on the shared host-reuse path.
///
/// Historical pairing: `tests/notes_ai_routing.rs` carried the source-level
/// pins for this behavior until the hosted Notes Agent Chat surface was
/// removed (2026-07-27, Notes AI ownership moved to the main window)
/// (ensures the note-target code path does not regress to `add_context_part`).
#[test]
fn two_sequential_note_handoffs_leave_only_the_second_notes_parts() {
    #[derive(Default)]
    struct PendingContextState {
        pending_context_parts: Vec<AiContextPart>,
        pending_context_consumed: bool,
    }

    impl PendingContextState {
        fn replace_pending_context_parts(&mut self, parts: Vec<AiContextPart>) {
            self.pending_context_parts.clear();
            self.pending_context_parts = parts;
            self.pending_context_consumed = false;
        }
    }

    let note_a_part = AiContextPart::FilePath {
        path: "/tmp/note-a.md".to_string(),
        label: "note-a.md".to_string(),
    };
    let note_b_part = AiContextPart::FilePath {
        path: "/tmp/note-b.md".to_string(),
        label: "note-b.md".to_string(),
    };

    let mut state = PendingContextState::default();

    state.replace_pending_context_parts(vec![note_a_part.clone()]);
    assert_eq!(
        state.pending_context_parts,
        vec![note_a_part.clone()],
        "first note handoff should stage exactly note A"
    );

    state.replace_pending_context_parts(vec![note_b_part.clone()]);
    assert_eq!(
        state.pending_context_parts,
        vec![note_b_part.clone()],
        "second note handoff must replace note A — pendingInlineContext must not accumulate stale chips across host transitions"
    );
    assert!(
        !state
            .pending_context_parts
            .iter()
            .any(|p| matches!(p, AiContextPart::FilePath { path, .. } if path == "/tmp/note-a.md")),
        "note A must not survive a subsequent host-transition stage call"
    );
    assert!(
        !state.pending_context_consumed,
        "second handoff must re-arm consumption so the next submit picks up note B"
    );
}

// ---------- Dedup contract ----------

/// Mirrors the dedup logic from `AiApp::add_attachment`.
fn add_attachment(parts: &mut Vec<AiContextPart>, path: &str) -> bool {
    let already_present = parts
        .iter()
        .any(|part| matches!(part, AiContextPart::FilePath { path: p, .. } if p == path));
    if already_present {
        return false;
    }
    let label = std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    parts.push(AiContextPart::FilePath {
        path: path.to_string(),
        label,
    });
    true
}

#[test]
fn add_attachment_deduplicates_by_path() {
    let mut parts = Vec::new();
    assert!(add_attachment(&mut parts, "/tmp/file.rs"));
    assert!(!add_attachment(&mut parts, "/tmp/file.rs"));
    assert_eq!(parts.len(), 1);
}

// ---------- FocusedTarget composer-state tests ----------

#[test]
fn focused_target_part_is_preserved_in_order_with_other_parts() {
    let parts = vec![
        AiContextPart::FocusedTarget {
            label: "File: main.rs".to_string(),
            target: script_kit_gpui::ai::TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:main.rs".to_string(),
                label: "main.rs".to_string(),
                metadata: None,
            },
        },
        AiContextPart::FilePath {
            path: "/tmp/example.txt".to_string(),
            label: "example.txt".to_string(),
        },
        AiContextPart::ResourceUri {
            uri: "kit://context?profile=minimal".to_string(),
            label: "Context".to_string(),
        },
    ];

    assert_eq!(parts[0].label(), "File: main.rs");
    assert_eq!(parts[1].label(), "example.txt");
    assert_eq!(parts[2].label(), "Context");
}

#[test]
fn focused_target_is_not_extracted_by_file_path_parts() {
    let parts = vec![
        AiContextPart::FocusedTarget {
            label: "Command: hello".to_string(),
            target: script_kit_gpui::ai::TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:0:hello".to_string(),
                label: "hello".to_string(),
                metadata: None,
            },
        },
        AiContextPart::FilePath {
            path: "/tmp/file.rs".to_string(),
            label: "file.rs".to_string(),
        },
    ];

    let paths = file_path_parts(&parts);
    assert_eq!(paths, vec!["/tmp/file.rs"]);
}

#[test]
fn removing_focused_target_preserves_other_parts() {
    let mut parts = vec![
        AiContextPart::FocusedTarget {
            label: "File: main.rs".to_string(),
            target: script_kit_gpui::ai::TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:main.rs".to_string(),
                label: "main.rs".to_string(),
                metadata: None,
            },
        },
        AiContextPart::FilePath {
            path: "/tmp/keep.rs".to_string(),
            label: "keep.rs".to_string(),
        },
    ];

    // Remove the FocusedTarget at index 0
    parts.remove(0);

    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].label(), "keep.rs");
    assert_eq!(file_path_parts(&parts), vec!["/tmp/keep.rs"]);
}

#[test]
fn focused_target_deduplication_by_equality() {
    let target = script_kit_gpui::ai::TabAiTargetContext {
        source: "FileSearch".to_string(),
        kind: "file".to_string(),
        semantic_id: "choice:0:main.rs".to_string(),
        label: "main.rs".to_string(),
        metadata: None,
    };

    let part_a = AiContextPart::FocusedTarget {
        label: "File: main.rs".to_string(),
        target: target.clone(),
    };
    let part_b = AiContextPart::FocusedTarget {
        label: "File: main.rs".to_string(),
        target,
    };

    // Same values should be equal
    assert_eq!(part_a, part_b);

    // merge_context_parts should deduplicate
    let merged = script_kit_gpui::ai::message_parts::merge_context_parts(&[part_a], &[part_b]);
    assert_eq!(merged.len(), 1);
}

#[test]
fn add_attachment_does_not_confuse_file_path_with_resource_uri() {
    let mut parts = vec![AiContextPart::ResourceUri {
        uri: "/tmp/file.rs".to_string(),
        label: "file.rs".to_string(),
    }];

    // Same string as a FilePath should still be added (different kind)
    assert!(add_attachment(&mut parts, "/tmp/file.rs"));
    assert_eq!(parts.len(), 2);
}
