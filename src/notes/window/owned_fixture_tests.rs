use super::init::NotesInitialData;
use super::*;

#[gpui::test]
fn injected_notes_preserve_reveal_and_real_editor_mutations(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);
    let handle = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                show: false,
                focus: false,
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    NotesApp::from_initial_data(
                        NotesInitialData {
                            notes: vec![Note::with_content(
                                "# Alpha\n\n- [ ] Review\n[Link](https://example.invalid)\n",
                            )],
                            deleted_notes: Vec::new(),
                            host_policy: crate::runtime_policy::WindowHostPolicy::Interactive,
                            ghost_clipboard: Some(Vec::new()),
                            now: Some(
                                chrono::DateTime::parse_from_rfc3339("2026-08-28T12:00:00Z")
                                    .unwrap()
                                    .with_timezone(&chrono::Utc),
                            ),
                        },
                        window,
                        cx,
                    )
                })
            },
        )
        .expect("real Notes host")
    });
    handle
        .update(cx, |app, window, cx| {
            assert!(!app.entry_reveal.body_visible);
            assert_eq!(app.entry_reveal.completed_frame_count, 0);
            let before = app.semantic_revision(cx);
            let content = app.notes_editor.read(cx).content(cx);
            app.set_editor_text_for_automation(content.replace("Alpha", "Bravo"), window, cx);
            assert!(
                app.semantic_revision(cx) > before,
                "equal-length edits are semantic mutations"
            );
            let after_edit = app.semantic_revision(cx);
            assert_eq!(
                app.semantic_revision(cx),
                after_edit,
                "inspection does not advance revision"
            );
            let content = app.notes_editor.read(cx).content(cx);
            let marker = content.find("[ ]").unwrap();
            app.toggle_preview(window, cx);
            assert!(app.preview_enabled && app.focus_handle.is_focused(window));
            assert!(app.toggle_task_marker_at(marker..marker + 3, false, window, cx));
            app.on_editor_change(window, cx);
            assert!(app.notes_editor.read(cx).content(cx).contains("[x] Review"));
            assert!(app.semantic_revision(cx) > after_edit);
            assert!(
                app.focus_handle.is_focused(window),
                "preview task edits must not focus the unrendered editor"
            );
            let checked = app.notes_editor.read(cx).content(cx);
            assert!(!app.toggle_task_marker_at(marker..marker + 3, false, window, cx));
            assert_eq!(app.notes_editor.read(cx).content(cx), checked);
            assert!(app.focus_handle.is_focused(window));
            app.toggle_preview(window, cx);
            assert!(!app.preview_enabled);
            assert!(app
                .editor_state
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
            app.editor_state
                .update(cx, |state, cx| state.edit_undo(window, cx));
            app.on_editor_change(window, cx);
            assert!(
                app.notes_editor.read(cx).content(cx).contains("[ ] Review"),
                "task toggle must use normal undo history"
            );
            let after_toggle = app.notes_editor.read(cx).content(cx);
            assert!(!app
                .notes_editor
                .update(cx, |editor, cx| editor.toggle_task_marker_at(
                    0..usize::MAX,
                    false,
                    window,
                    cx
                )));
            assert_eq!(app.notes_editor.read(cx).content(cx), after_toggle);
            let frozen = app.capture_dictation_destination(cx);
            let selection = app.notes_editor.read(cx).selection(cx);
            let alternate = if selection == (0..0) {
                after_toggle.len()
            } else {
                0
            };
            app.notes_editor.update(cx, |editor, cx| {
                editor.set_selection(alternate, alternate, window, cx);
                editor.set_selection(selection.start, selection.end, window, cx);
            });
            let refusal = app
                .inject_dictation_text_into_snapshot(&frozen, "must not land", window, cx)
                .unwrap_err();
            assert!(
                refusal.starts_with("stale_destination:"),
                "selection ABA must invalidate frozen delivery"
            );
            assert_eq!(app.editor_text(cx), after_toggle);
            assert!(
                !app.entry_reveal.body_visible,
                "editing must not shortcut ordered reveal"
            );
        })
        .expect("Notes edits");
}

#[gpui::test]
fn theme_recolors_existing_links_without_manufacturing_a_data_revision(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui_component::init);
    let handle = cx.update(|cx| {
        cx.open_window(
            WindowOptions {
                show: false,
                focus: false,
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    NotesApp::from_initial_data(
                        NotesInitialData {
                            notes: vec![Note::with_content("[Reference](https://example.invalid)")],
                            deleted_notes: Vec::new(),
                            host_policy: crate::runtime_policy::WindowHostPolicy::Interactive,
                            ghost_clipboard: Some(Vec::new()),
                            now: None,
                        },
                        window,
                        cx,
                    )
                })
            },
        )
        .expect("Notes host")
    });
    handle
        .update(cx, |app, _window, cx| {
            let before_revision = app.semantic_revision(cx);
            let before = app.editor_state.read(cx).highlight_ranges().to_vec();
            let before_observed = app
                .notes_editor
                .read(cx)
                .markdown_link_highlight_runtime_info(cx);
            gpui_component::Theme::global_mut(cx).colors.accent = gpui::hsla(0.35, 0.8, 0.6, 1.0);
            app.notes_editor
                .update(cx, |editor, cx| editor.sync_markdown_link_highlights(cx));
            let after = app.editor_state.read(cx).highlight_ranges().to_vec();
            assert!(!before.is_empty());
            assert_ne!(
                before, after,
                "same text and selection must repaint with the new accent"
            );
            assert_eq!(app.semantic_revision(cx), before_revision);
            let after_observed = app
                .notes_editor
                .read(cx)
                .markdown_link_highlight_runtime_info(cx);
            assert_eq!(before_observed["count"], after_observed["count"]);
            assert_eq!(
                before_observed["ranges"][0]["range"],
                after_observed["ranges"][0]["range"]
            );
            assert_eq!(
                before_observed["ranges"][0]["content"],
                after_observed["ranges"][0]["content"]
            );
            assert_ne!(
                before_observed["ranges"][0]["color"],
                after_observed["ranges"][0]["color"]
            );
        })
        .expect("theme recoloring");
}
