        if let AppView::DayPage { entity } = &self.current_view {
            let day_page = entity.read(cx);
            let editor_layout = day_page.notes_editor.read(cx).layout();
            let shelf_count = if day_page.kit_resource_preview.is_none() {
                day_page.clipboard_shelf.len()
            } else {
                0
            };
            let day_page_tokens = crate::designs::get_tokens(self.current_design);
            let day_page_header_padding_y = if self.current_design.is_default() {
                shell.header_padding_y
            } else {
                day_page_tokens.spacing().padding_sm
            };
            let day_page_header_gap = if self.current_design.is_default() {
                shell.header_gap
            } else {
                day_page_tokens.spacing().gap_md
            };
            // Day Page's shared header owns only the context row; its input
            // slot is empty, so the generic search height is not rendered.
            let day_page_content_top = day_page_header_padding_y * 2.0
                + menu_def.header_info_bar.height_px
                + day_page_header_gap;
            let footer_height = footer_metrics.height_px;
            let budget = day_page_layout_budget(
                window_height,
                day_page_content_top,
                footer_height,
                shelf_count,
                day_page.clipboard_shelf_expanded,
                editor_layout.padding_y,
            );
            let columns = crate::components::main_view_chrome::main_view_content_columns(menu_def);
            let editor_x = columns.content_right_inset_x;
            let editor_width = (window_width - editor_x * 2.0).max(0.0);
            let text_plane_x = editor_x + editor_layout.padding_x;
            let text_plane_width = (editor_width - editor_layout.padding_x * 2.0).max(0.0);
            let text_plane_y = day_page_content_top + editor_layout.padding_y;
            let text_plane_height = (budget.editor_height - editor_layout.padding_y * 2.0).max(0.0);

            components.push(
                LayoutComponentInfo::new("DayPageSurface", LayoutComponentType::Container)
                    .with_bounds(
                        0.0,
                        day_page_content_top,
                        window_width,
                        budget.body_height,
                    )
                    .with_flex_column()
                    .with_depth(2)
                    .with_parent("MainViewMain")
                    .with_explanation(
                        "Day Page owns one vertical budget for editor, clipboard accessory, and native footer.",
                    ),
            );
            components.push(
                LayoutComponentInfo::new("DayPageEditor", LayoutComponentType::Input)
                    .with_bounds(
                        editor_x,
                        day_page_content_top,
                        editor_width,
                        budget.editor_height,
                    )
                    .with_padding(
                        editor_layout.padding_y,
                        editor_layout.padding_x,
                        editor_layout.padding_y,
                        editor_layout.padding_x,
                    )
                    .with_flex_grow(1.0)
                    .with_depth(3)
                    .with_parent("DayPageSurface")
                    .with_explanation(format!(
                        "Editor receives the remaining Day Page body height and preserves a {}px minimum before the expanded shelf can grow.",
                        DAY_PAGE_MIN_EDITOR_HEIGHT_PX
                    )),
            );
            components.push(
                LayoutComponentInfo::new(
                    "DayPageEditorTextPlane",
                    LayoutComponentType::Container,
                )
                .with_bounds(
                    text_plane_x,
                    text_plane_y,
                    text_plane_width,
                    text_plane_height,
                )
                .with_depth(4)
                .with_parent("DayPageEditor")
                .with_explanation(
                    "Shared NotesEditor text plane after its horizontal and vertical content insets.",
                ),
            );

            if shelf_count > 0 {
                let shelf_y = day_page_content_top + budget.editor_height;
                components.push(
                    LayoutComponentInfo::new(
                        "DayPageClipboardShelf",
                        LayoutComponentType::Container,
                    )
                    .with_bounds(
                        text_plane_x,
                        shelf_y,
                        text_plane_width,
                        budget.shelf_height,
                    )
                    .with_padding(
                        DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX,
                        0.0,
                        editor_layout.padding_y,
                        0.0,
                    )
                    .with_flex_column()
                    .with_depth(3)
                    .with_parent("DayPageSurface")
                    .with_explanation(format!(
                        "NotesEditor bottom accessory on the same x={} text plane; expanded list height is responsive ({}px), not a fixed nested-scroll cap.",
                        text_plane_x, budget.shelf_list_height
                    )),
                );
                components.push(
                    LayoutComponentInfo::new(
                        "DayPageClipboardShelfToggle",
                        LayoutComponentType::Button,
                    )
                    .with_bounds(
                        text_plane_x,
                        shelf_y + DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX,
                        text_plane_width,
                        DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX,
                    )
                    .with_depth(4)
                    .with_parent("DayPageClipboardShelf")
                    .with_explanation("Disclosure label starts on the editor text plane."),
                );
                if budget.shelf_list_height > 0.0 {
                    components.push(
                        LayoutComponentInfo::new(
                            "DayPageClipboardShelfList",
                            LayoutComponentType::List,
                        )
                        .with_bounds(
                            text_plane_x,
                            shelf_y
                                + DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX
                                + DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX
                                + DAY_PAGE_CLIPBOARD_SHELF_GAP_PX,
                            text_plane_width,
                            budget.shelf_list_height,
                        )
                        .with_flex_column()
                        .with_depth(4)
                        .with_parent("DayPageClipboardShelf")
                        .with_explanation(format!(
                            "{} clipboard rows share the editor text plane and scroll only within the responsive accessory budget.",
                            shelf_count
                        )),
                    );
                }
            }
        } else if let AppView::AgentChatView { entity } = &self.current_view {
            let agent_chat_view = entity.read(cx);
            let agent_chat_state = agent_chat_view.collect_agent_chat_state_snapshot(cx);
            let transcript_viewport_bounds = agent_chat_view.transcript_viewport_bounds_px(cx);
            let agent_chat_is_empty = agent_chat_state.message_count == 0
                && !agent_chat_state.awaiting_first_assistant_text;
            let info_metrics = crate::components::info_state::info_metrics(
                crate::components::info_state::InfoStateDensity::Comfortable,
            );
            let info_columns =
                crate::components::main_view_chrome::main_view_content_columns(menu_def);
            let info_x = info_columns.text_column_x;
            let info_y = content_top + info_columns.top_inset_y;
            let info_width = (window_width - info_x - info_columns.content_right_inset_x).max(0.0);
            let shortcut_slot_width = crate::components::footer_chrome::FOOTER_KEYCAP_HEIGHT_PX
                * 2.0
                + crate::components::footer_chrome::FOOTER_ACTION_CONTENT_GAP_PX;
            let guidance_label_x =
                info_x + shortcut_slot_width + crate::components::info_state::INFO_SPACING.sm;
            let guidance_label_width =
                (info_width - shortcut_slot_width - crate::components::info_state::INFO_SPACING.sm)
                    .max(0.0);

            components.push(
                LayoutComponentInfo::new("AgentChatConversation", LayoutComponentType::List)
                    .with_bounds(
                        0.0,
                        content_top,
                        window_width,
                        (content_height - shell.content_inset_bottom).max(0.0),
                    )
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_CONTENT,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                    )
                    .with_visual_token("content.agent_chatConversation")
                    .with_flex_column()
                    .with_flex_grow(1.0)
                    .with_depth(2)
                    .with_parent("MainViewMain")
                    .with_explanation(format!(
                        "AgentChat conversation region. message_count={}, awaiting_first_assistant_text={}.",
                        agent_chat_state.message_count, agent_chat_state.awaiting_first_assistant_text
                    )),
            );

            if agent_chat_is_empty {
                components.push(
                    LayoutComponentInfo::new("AgentChatEmptyGuidance", LayoutComponentType::Container)
                        .with_bounds(
                            info_x,
                            info_y,
                            info_width,
                            (content_height
                                - info_columns.top_inset_y
                                - shell.content_inset_bottom)
                                .max(0.0),
                        )
                        .with_visual_style(
                            chrome_tokens::CHROME_LAYER_CONTENT,
                            chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                            Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                        )
                        .with_visual_token("content.agent_chatEmptyGuidance")
                        .with_flex_column()
                        .with_gap(info_metrics.block_gap)
                        .with_depth(3)
                        .with_parent("AgentChatConversation")
                        .with_explanation(format!(
                            "ComposerEmpty InfoState is anchored to the shared main-view text column: x={} = row outer + row inner + icon + gap. Width = window({}) - x({}) - right inset({}).",
                            info_x, window_width, info_x, info_columns.content_right_inset_x
                        )),
                );
                components.push(
                    LayoutComponentInfo::new("AgentChatEmptyGuidanceTitle", LayoutComponentType::Header)
                        .with_bounds(
                            info_x,
                            info_y,
                            info_width,
                            crate::components::info_state::INFO_TYPE_SCALE.title.line,
                        )
                        .with_typography(
                            "infoTitle",
                            Some(self.theme_font_family()),
                            crate::components::info_state::INFO_TYPE_SCALE.title.size,
                            "semibold",
                            gpui::FontWeight::SEMIBOLD.0,
                            crate::components::info_state::INFO_TYPE_SCALE.title.line,
                            "left",
                        )
                        .with_depth(4)
                        .with_parent("AgentChatEmptyGuidance")
                        .with_explanation(
                            "Comfortable ComposerEmpty title uses the InfoState title scale and starts on the shared main-view text column."
                                .to_string(),
                        ),
                );
                components.push(
                    LayoutComponentInfo::new("AgentChatEmptyGuidanceBody", LayoutComponentType::Other)
                        .with_bounds(
                            info_x,
                            info_y
                                + crate::components::info_state::INFO_TYPE_SCALE.title.line
                                + crate::components::info_state::INFO_SPACING.xs * 0.5,
                            info_width,
                            crate::components::info_state::INFO_TYPE_SCALE.body.line,
                        )
                        .with_typography(
                            "infoBody",
                            Some(self.theme_font_family()),
                            crate::components::info_state::INFO_TYPE_SCALE.body.size,
                            "regular",
                            gpui::FontWeight::NORMAL.0,
                            crate::components::info_state::INFO_TYPE_SCALE.body.line,
                            "left",
                        )
                        .with_depth(4)
                        .with_parent("AgentChatEmptyGuidance")
                        .with_explanation(
                            "ComposerEmpty body follows the title inside the same shared main-view column."
                                .to_string(),
                        ),
                );
                components.push(
                    LayoutComponentInfo::new(
                        "AgentChatEmptyGuidanceShortcutSlot",
                        LayoutComponentType::Other,
                    )
                    .with_bounds(
                        info_x,
                        info_y + info_metrics.block_gap + crate::components::info_state::INFO_TYPE_SCALE.title.line + crate::components::info_state::INFO_SPACING.xs * 0.5 + crate::components::info_state::INFO_TYPE_SCALE.body.line,
                        shortcut_slot_width,
                        info_metrics.row_min_h,
                    )
                    .with_depth(4)
                    .with_parent("AgentChatEmptyGuidance")
                    .with_explanation(
                        "Shortcut slot width uses the same footer keycap geometry as InfoState guidance rows."
                            .to_string(),
                    ),
                );
                components.push(
                    LayoutComponentInfo::new(
                        "AgentChatEmptyGuidanceLabelColumn",
                        LayoutComponentType::Other,
                    )
                    .with_bounds(
                        guidance_label_x,
                        info_y + info_metrics.block_gap + crate::components::info_state::INFO_TYPE_SCALE.title.line + crate::components::info_state::INFO_SPACING.xs * 0.5 + crate::components::info_state::INFO_TYPE_SCALE.body.line,
                        guidance_label_width,
                        info_metrics.row_min_h,
                    )
                    .with_typography(
                        "infoGuidanceLabel",
                        Some(self.theme_font_family()),
                        crate::components::info_state::INFO_TYPE_SCALE.caption.size,
                        "regular",
                        gpui::FontWeight::NORMAL.0,
                        crate::components::info_state::INFO_TYPE_SCALE.caption.line,
                        "left",
                    )
                    .with_depth(4)
                    .with_parent("AgentChatEmptyGuidance")
                    .with_explanation(
                        "Guidance labels start after the footer-keycap shortcut slot and the InfoState row gap; the whole block remains anchored to the main-view text column."
                            .to_string(),
                    ),
                );
            } else {
                let (transcript_x, transcript_y, transcript_width, transcript_height) =
                    transcript_viewport_bounds.unwrap_or((
                        0.0,
                        content_top,
                        window_width,
                        (content_height - shell.content_inset_bottom).max(0.0),
                    ));
                components.push(
                    LayoutComponentInfo::new("AgentChatTranscript", LayoutComponentType::List)
                        .with_bounds(
                            transcript_x,
                            transcript_y,
                            transcript_width,
                            transcript_height,
                        )
                        .with_visual_style(
                            chrome_tokens::CHROME_LAYER_CONTENT,
                            chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                            Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                        )
                        .with_visual_token("content.agent_chatTranscript")
                        .with_flex_column()
                        .with_flex_grow(1.0)
                        .with_depth(3)
                        .with_parent("AgentChatConversation")
                        .with_explanation(format!(
                            "Agent Chat transcript viewport for {} live thread messages; bounds come from the transcript ListState viewport.",
                            agent_chat_state.message_count,
                        )),
                );
            }
        } else {
            // Script list: full width for MainWindow, left panel for split-preview surfaces.
            components.push(
                LayoutComponentInfo::new("ScriptList", LayoutComponentType::List)
                    .with_bounds(
                        0.0,
                        content_top,
                        list_width,
                        (content_height - shell.content_inset_bottom).max(0.0),
                    )
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_CONTENT,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                    )
                    .with_visual_token("content.list")
                    .with_flex_column()
                    .with_depth(2)
                    .with_parent("MainViewMain")
                    .with_explanation(format!(
                        "Width = {}px. Uses uniform_list for virtualized scrolling with {}px item height.",
                        list_width, list.item_height
                    )),
            );

            if uses_split_preview {
                // Preview panel (right panel) - remaining 50%
                let preview_width = window_width - list_width;
                components.push(
                    LayoutComponentInfo::new("PreviewPanel", LayoutComponentType::Panel)
                        .with_bounds(list_width, content_top, preview_width, content_height)
                        .with_visual_style(
                            chrome_tokens::CHROME_LAYER_CONTENT,
                            chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                            Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                        )
                        .with_visual_token("content.previewPanel")
                        .with_padding(16.0, 16.0, 16.0, 16.0)
                        .with_flex_column()
                        .with_depth(2)
                        .with_parent("MainViewMain")
                        .with_explanation(format!(
                            "Width = remaining 50% = {}px. Has 16px padding on all sides.",
                            preview_width
                        )),
                );
            }

            // List items (sample of first few visible)
            let visible_items = ((content_height / list.item_height) as usize).min(5);
            for i in 0..visible_items {
                let item_top = content_top + (i as f32 * list.item_height);
                components.push(
                    LayoutComponentInfo::new(
                        format!("ListItem[{}]", i),
                        LayoutComponentType::ListItem,
                    )
                    .with_bounds(row.outer_padding_x, item_top, list_width, list.item_height)
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_CONTENT,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(row.radius),
                    )
                    .with_visual_token("content.listItem")
                    .with_padding(
                        row.inner_padding_y,
                        row.inner_padding_x,
                        row.inner_padding_y,
                        row.inner_padding_x,
                    )
                    .with_gap(row.icon_text_gap)
                    .with_flex_row()
                    .with_depth(3)
                    .with_parent("ScriptList")
                    .with_explanation(format!(
                        "Fixed height = {}px for {}. Uses flex-row with theme gap {}px and padding {}px vertical / {}px horizontal.",
                        list.item_height,
                        menu_theme.name(),
                        row.icon_text_gap,
                        row.inner_padding_y,
                        row.inner_padding_x
                    )),
                );
            }
        }
