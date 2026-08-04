// Day Page Agent Chat handoff helpers.

use crate::components::notes_editor::spine::mention_atomic_delete_fixup;

impl DayPageView {
    pub(crate) fn collect_day_page_elements(
        &self,
        limit: usize,
        app_state: &ScriptListApp,
        cx: &App,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let content = self.notes_editor.read(cx).content(cx);
        let selection = self.notes_editor.read(cx).selection(cx);
        let mut editor =
            protocol::ElementInfo::input("day-page-editor", Some(content.as_str()), true);
        let metrics = crate::notes::window::style::adopted_metrics();
        let editor_surface =
            crate::components::notes_editor::NotesEditorSurfaceStyle::from_theme(&app_state.theme);
        editor.role = Some("day_page_editor".to_string());
        editor.kind = Some("editor_selection".to_string());
        editor.source = Some(format!("{}:{}", selection.start, selection.end));
        editor.source_name = Some(content.len().to_string());
        editor.style = Some(protocol::ElementStyleInfo {
            owner: editor_surface.owner.to_string(),
            input_render_path: Some(editor_surface.input_render_path.to_string()),
            editor_runtime: Some(
                self.notes_editor
                    .read(cx)
                    .markdown_runtime_info_with_scroll(cx),
            ),
            surface_background_rgb: Some(editor_surface.background_rgb),
            occlusion_rgba: Some(editor_surface.occlusion_rgba),
            padding_x: Some(metrics.editor_padding_x),
            padding_y: Some(metrics.editor_padding_y),
            font_family_source: Some("theme.mono_font_family".to_string()),
            text_size_source: Some("theme.mono_font_size".to_string()),
        });
        let mut text_plane = protocol::ElementInfo::panel("day-page-editor-text-plane");
        text_plane.role = Some("day_page_editor_text_plane".to_string());
        text_plane.kind = Some("layout_receipt".to_string());
        text_plane.source = Some("DayPageEditorTextPlane".to_string());
        text_plane.source_name = Some("getLayoutInfo".to_string());
        text_plane.selectable = Some(false);

        let bound_date = self.session.bound_date();
        let identity = crate::components::main_view_chrome::SemanticChipSpec::enabled_identity(
            bound_date
                .map(|date| format!("day-page-document:{date}"))
                .unwrap_or_else(|| "day-page-document:unbound".to_string()),
            bound_date
                .map(|date| format!("Today · {date}"))
                .unwrap_or_else(|| "Today".to_string()),
            crate::components::main_view_chrome::SemanticChipAction::OpenDetails,
            "⌘P",
        );
        let mut elements = vec![
            protocol::ElementInfo::panel("day-page"),
            crate::windows::automation_surface_collector::collect_semantic_chip_element(&identity),
            editor,
            text_plane,
        ];

        if !self.clipboard_shelf.is_empty() && self.kit_resource_preview.is_none() {
            let count = self.clipboard_shelf.len();
            let mut shelf = protocol::ElementInfo::panel("day-page-clipboard-shelf");
            shelf.text = Some(format!(
                "Clipboard · {count} kept {}",
                if count == 1 { "entry" } else { "entries" }
            ));
            shelf.value = Some(if self.clipboard_shelf_expanded {
                "expanded".to_string()
            } else {
                "collapsed".to_string()
            });
            shelf.selected = Some(self.clipboard_shelf_expanded);
            shelf.role = Some("day_page_clipboard_shelf".to_string());
            shelf.kind = Some("layout_receipt".to_string());
            shelf.source = Some("DayPageClipboardShelf".to_string());
            shelf.source_name = Some("getLayoutInfo".to_string());
            shelf.selectable = Some(true);
            elements.push(shelf);
        }

        if self.read_mode && self.kit_resource_preview.is_none() {
            elements.push(protocol::ElementInfo {
                semantic_id: "day-page-read-preview".to_string(),
                element_type: protocol::ElementType::Panel,
                text: Some("Day Page Preview".to_string()),
                value: Some(content.clone()),
                selected: None,
                focused: None,
                index: None,
                role: Some("day_page_read_preview".to_string()),
                kind: Some("markdown_preview".to_string()),
                source: Some(
                    crate::components::notes_editor::NOTES_EDITOR_PREVIEW_RENDER_PATH.to_string(),
                ),
                source_name: Some(
                    crate::components::notes_editor::NOTES_EDITOR_STYLE_OWNER.to_string(),
                ),
                selectable: Some(false),
                status_kind: None,
                action_disabled: None,
                style: Some(protocol::ElementStyleInfo {
                    owner: editor_surface.owner.to_string(),
                    input_render_path: Some(
                        crate::components::notes_editor::NOTES_EDITOR_PREVIEW_RENDER_PATH
                            .to_string(),
                    ),
                    editor_runtime: None,
                    surface_background_rgb: Some(editor_surface.background_rgb),
                    occlusion_rgba: Some(editor_surface.occlusion_rgba),
                    padding_x: Some(metrics.editor_padding_x),
                    padding_y: Some(metrics.editor_padding_y),
                    font_family_source: Some("theme.mono_font_family".to_string()),
                    text_size_source: Some("theme.mono_font_size".to_string()),
                }),
            });
        }

        if self.session.is_viewing_fragment() {
            elements.push(protocol::ElementInfo {
                semantic_id: script_kit_gpui::day_page::FRAGMENT_BACK_ID.to_string(),
                element_type: protocol::ElementType::Button,
                text: Some("Back to Today".to_string()),
                value: Some("day_page:back_to_today".to_string()),
                selected: None,
                focused: None,
                index: None,
                role: Some("day_page_fragment_back".to_string()),
                kind: Some("FragmentBack".to_string()),
                source: None,
                source_name: None,
                selectable: Some(true),
                status_kind: None,
                action_disabled: None,
                style: None,
            });
        }

        // NOTE: the live day/note switcher is `note_switcher`, a centered
        // CommandBar popup that surfaces its own rows through its popup
        // automation window — not through this main-window element collection.
        // The old inline `day_switcher` state was dead (always None), so its
        // element block was removed along with the machinery.

        let total_count = elements.len();
        (elements.into_iter().take(limit).collect(), total_count)
    }
}
