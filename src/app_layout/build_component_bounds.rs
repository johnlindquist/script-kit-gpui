impl ScriptListApp {
    fn build_component_bounds(
        &self,
        window_size: gpui::Size<gpui::Pixels>,
        cx: &gpui::App,
    ) -> Vec<debug_grid::ComponentBounds> {
        use crate::list_item::LIST_ITEM_HEIGHT;
        use debug_grid::{BoxModel, ComponentBounds, ComponentType};

        let mut bounds = Vec::new();
        let width = window_size.width;
        let height = window_size.height;
        let menu_def = self.current_main_menu_theme.def();

        // Header bounds follow the same active main-menu theme tokens as the
        // shared rendered chrome.

        // Content padding matches HEADER_PADDING_X
        let content_padding = menu_def.shell.header_padding_x;

        // Determine the current view type and build appropriate bounds
        let view_name = match &self.current_view {
            AppView::ScriptList => "ScriptList",
            AppView::About { .. } => "About",
            AppView::ArgPrompt { .. } => "ArgPrompt",
            AppView::DivPrompt { .. } => "DivPrompt",
            AppView::EditorPrompt { .. } => "EditorPrompt",
            AppView::TermPrompt { .. } => "TermPrompt",
            AppView::FormPrompt { .. } => "FormPrompt",
            AppView::SelectPrompt { .. } => "SelectPrompt",
            AppView::PathPrompt { .. } => "PathPrompt",
            AppView::EnvPrompt { .. } => "EnvPrompt",
            AppView::DropPrompt { .. } => "DropPrompt",
            AppView::TemplatePrompt { .. } => "TemplatePrompt",
            AppView::HotkeyPrompt { .. } => "HotkeyPrompt",
            AppView::ChatPrompt { .. } => "ChatPrompt",
            AppView::MiniPrompt { .. } => "MiniPrompt",
            AppView::MicroPrompt { .. } => "MicroPrompt",
            AppView::ClipboardHistoryView { .. } => "ClipboardHistory",
            AppView::AppLauncherView { .. } => "AppLauncher",
            AppView::WindowSwitcherView { .. } => "WindowSwitcher",
            AppView::BrowserTabsView { .. } => "BrowserTabs",
            AppView::ScratchPadView { .. } => "ScratchPad",
            AppView::QuickTerminalView { .. } => "QuickTerminal",
            AppView::FlowSessionView { .. } => "FlowSession",
            AppView::FileSearchView { .. } => "FileSearch",
            AppView::ProfileSearchView { .. } => "ProfileSearch",
            AppView::ThemeChooserView { .. } => "ThemeChooser",
            AppView::EmojiPickerView { .. } => "EmojiPicker",
            AppView::ActionsDialog => "ActionsDialog",
            AppView::WebcamView { .. } => "Webcam",
            AppView::CreationFeedback { .. } => "CreationFeedback",
            AppView::NamingPrompt { .. } => "NamingPrompt",
            AppView::BrowseKitsView { .. } => "BrowseKits",
            AppView::MigrateV1View { .. } => "MigrateV1",
            AppView::InstalledKitsView { .. } => "InstalledKits",
            AppView::ProcessManagerView { .. } => "ProcessManager",
            AppView::FlowUxView { .. } => "FlowUx",
            AppView::CurrentAppCommandsView { .. } => "CurrentAppCommands",
            AppView::SearchAiPresetsView { .. } => "SearchAiPresets",
            AppView::CreateAiPresetView { .. } => "CreateAiPreset",
            AppView::SettingsView { .. } => "Settings",
            AppView::PermissionsWizardView { .. } => "PermissionsWizard",
            AppView::FavoritesBrowseView { .. } => "FavoritesBrowse",
            AppView::AgentChatHistoryView { .. } => "AgentChatHistory",
            AppView::BrowserHistoryView { .. } => "BrowserHistory",
            AppView::DictationHistoryView { .. } => "DictationHistory",
            AppView::NotesBrowseView { .. } => "NotesBrowse",
            AppView::AgentChatView { .. } => "AgentChat",
            AppView::DayPage { .. } => "DayPage",
            AppView::ScriptIssuesView { .. } => "ScriptIssues",
            AppView::SdkReferenceView { .. } => "SdkReference",
            AppView::TipsView { .. } => "Tips",
            AppView::ScriptTemplateCatalogView { .. } => "ScriptTemplateCatalog",
            AppView::ConfirmPrompt { .. } => "ConfirmPrompt",
        };

        let header_policy = self.current_view.resolved_main_view_header_input_policy(cx);
        if header_policy == MainViewHeaderInputPolicy::ViewOwnedIntentionalCompact {
            let AppView::AgentChatView { entity } = &self.current_view else {
                unreachable!("Focused Text Mini is the only intentional compact main-window view")
            };
            let target = protocol::AutomationWindowInfo {
                id: "main".to_string(),
                kind: protocol::AutomationWindowKind::Main,
                title: None,
                focused: true,
                visible: true,
                semantic_surface: Some("FocusedTextMini".to_string()),
                bounds: Some(protocol::AutomationWindowBounds {
                    x: 0.0,
                    y: 0.0,
                    width: f32::from(width) as f64,
                    height: f32::from(height) as f64,
                }),
                parent_window_id: None,
                parent_kind: None,
                pid: None,
            };
            let layout = entity.read(cx).automation_layout_info(&target, cx);
            for component in layout.components {
                let component_type = match component.component_type {
                    protocol::LayoutComponentType::Prompt => ComponentType::Prompt,
                    protocol::LayoutComponentType::Input => ComponentType::Input,
                    protocol::LayoutComponentType::Button => ComponentType::Button,
                    protocol::LayoutComponentType::List => ComponentType::List,
                    protocol::LayoutComponentType::ListItem => ComponentType::ListItem,
                    protocol::LayoutComponentType::Header => ComponentType::Header,
                    protocol::LayoutComponentType::Container
                    | protocol::LayoutComponentType::Panel => ComponentType::Container,
                    protocol::LayoutComponentType::Other
                    | protocol::LayoutComponentType::Unknown => ComponentType::Other,
                };
                bounds.push(
                    ComponentBounds::new(
                        component.name,
                        gpui::Bounds {
                            origin: gpui::point(px(component.bounds.x), px(component.bounds.y)),
                            size: gpui::size(
                                px(component.bounds.width),
                                px(component.bounds.height),
                            ),
                        },
                    )
                    .with_type(component_type)
                    .with_padding(BoxModel::uniform(0.0)),
                );
            }
            return bounds;
        }
        let input_height = match header_policy {
            MainViewHeaderInputPolicy::ViewOwnedCanonicalInput => Some(menu_def.search.height),
            MainViewHeaderInputPolicy::ViewOwnedCanonicalMultilineInput => {
                // `build_component_bounds` has no GPUI `App` access, so it
                // cannot measure the live wrapped Agent Chat composer. Keep
                // this debug-grid model honest by reporting the canonical
                // one-line baseline; `build_layout_info` is runtime-aware and
                // measures `AgentChatComposerBar` from the entity when that
                // nested receipt is available.
                let agent_chat_baseline_input_height = menu_def.search.height;
                Some(agent_chat_baseline_input_height)
            }
            MainViewHeaderInputPolicy::ViewOwnedContextOnly
            | MainViewHeaderInputPolicy::RootContextOnly => None,
            MainViewHeaderInputPolicy::ViewOwnedIntentionalCompact => {
                unreachable!("compact main-window layout returns before canonical header metrics")
            }
        };
        let header_metrics =
            crate::components::main_view_chrome::main_view_header_metrics(menu_def, input_height);
        let header_height = px(header_metrics.header_height);
        let content_top = header_height;
        let content_height = height - header_height;

        // Header bounds (includes padding + context + input; no divider) - common to all views
        bounds.push(
            ComponentBounds::new(
                "MainViewHeader",
                gpui::Bounds {
                    origin: gpui::point(px(0.), px(0.)),
                    size: gpui::size(width, header_height),
                },
            )
            .with_type(ComponentType::Header)
            .with_padding(BoxModel::symmetric(
                menu_def.shell.header_padding_y,
                content_padding,
            )),
        );

        bounds.push(
            ComponentBounds::new(
                "MainViewMain",
                gpui::Bounds {
                    origin: gpui::point(px(0.), content_top),
                    size: gpui::size(width, content_height),
                },
            )
            .with_type(ComponentType::Container)
            .with_padding(BoxModel::uniform(0.0)),
        );

        // Build view-specific bounds
        match &self.current_view {
            AppView::ScriptList => {
                let uses_split_preview = matches!(self.main_window_mode, MainWindowMode::Full);
                let list_width = if uses_split_preview {
                    width * 0.5
                } else {
                    width
                };
                let item_height = px(LIST_ITEM_HEIGHT);

                bounds.push(
                    ComponentBounds::new(
                        "ScriptList",
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(list_width, content_height),
                        },
                    )
                    .with_type(ComponentType::List)
                    .with_padding(BoxModel::uniform(0.0)),
                );

                // Add sample list items
                for i in 0..5 {
                    let item_top = content_top + px(i as f32 * LIST_ITEM_HEIGHT);
                    if item_top + item_height > height {
                        break;
                    }
                    bounds.push(
                        ComponentBounds::new(
                            format!("ListItem[{}]", i),
                            gpui::Bounds {
                                origin: gpui::point(px(0.), item_top),
                                size: gpui::size(list_width, item_height),
                            },
                        )
                        .with_type(ComponentType::ListItem)
                        .with_padding(BoxModel::symmetric(12.0, content_padding))
                        .with_margin(BoxModel::uniform(0.0)),
                    );
                }

                if uses_split_preview {
                    // Preview panel (right side)
                    bounds.push(
                        ComponentBounds::new(
                            "PreviewPanel",
                            gpui::Bounds {
                                origin: gpui::point(list_width, content_top),
                                size: gpui::size(width - list_width, content_height),
                            },
                        )
                        .with_type(ComponentType::Container)
                        .with_padding(BoxModel::uniform(content_padding)),
                    );
                }
            }

            AppView::DivPrompt { .. } => {
                // DivPrompt takes full width below header
                bounds.push(
                    ComponentBounds::new(
                        "DivContent",
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(width, content_height),
                        },
                    )
                    .with_type(ComponentType::Prompt)
                    .with_padding(BoxModel::uniform(content_padding)),
                );
            }

            AppView::EditorPrompt { .. } => {
                // EditorPrompt takes full width below header
                bounds.push(
                    ComponentBounds::new(
                        "EditorContent",
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(width, content_height),
                        },
                    )
                    .with_type(ComponentType::Prompt)
                    .with_padding(BoxModel::uniform(content_padding)),
                );
            }

            AppView::TermPrompt { .. } => {
                // TermPrompt takes full width below header
                bounds.push(
                    ComponentBounds::new(
                        "TerminalContent",
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(width, content_height),
                        },
                    )
                    .with_type(ComponentType::Prompt)
                    .with_padding(BoxModel::uniform(content_padding)),
                );
            }

            AppView::ArgPrompt { choices, .. } => {
                // ArgPrompt may have choices list
                if choices.is_empty() {
                    // No choices - just input area
                    bounds.push(
                        ComponentBounds::new(
                            "ArgInput",
                            gpui::Bounds {
                                origin: gpui::point(px(0.), content_top),
                                size: gpui::size(width, content_height),
                            },
                        )
                        .with_type(ComponentType::Prompt)
                        .with_padding(BoxModel::uniform(content_padding)),
                    );
                } else {
                    // Has choices - show list
                    let item_height = px(LIST_ITEM_HEIGHT);
                    bounds.push(
                        ComponentBounds::new(
                            "ChoicesList",
                            gpui::Bounds {
                                origin: gpui::point(px(0.), content_top),
                                size: gpui::size(width, content_height),
                            },
                        )
                        .with_type(ComponentType::List)
                        .with_padding(BoxModel::uniform(0.0)),
                    );

                    // Add choice items
                    for i in 0..choices.len().min(5) {
                        let item_top = content_top + px(i as f32 * LIST_ITEM_HEIGHT);
                        if item_top + item_height > height {
                            break;
                        }
                        bounds.push(
                            ComponentBounds::new(
                                format!("Choice[{}]", i),
                                gpui::Bounds {
                                    origin: gpui::point(px(0.), item_top),
                                    size: gpui::size(width, item_height),
                                },
                            )
                            .with_type(ComponentType::ListItem)
                            .with_padding(BoxModel::symmetric(12.0, content_padding)),
                        );
                    }
                }
            }

            AppView::FormPrompt { .. } => {
                bounds.push(
                    ComponentBounds::new(
                        "FormContent",
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(width, content_height),
                        },
                    )
                    .with_type(ComponentType::Prompt)
                    .with_padding(BoxModel::uniform(content_padding)),
                );
            }

            AppView::SelectPrompt { .. } | AppView::PathPrompt { .. } => {
                // List-based prompts
                let item_height = px(LIST_ITEM_HEIGHT);
                bounds.push(
                    ComponentBounds::new(
                        view_name,
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(width, content_height),
                        },
                    )
                    .with_type(ComponentType::List)
                    .with_padding(BoxModel::uniform(0.0)),
                );

                for i in 0..5 {
                    let item_top = content_top + px(i as f32 * LIST_ITEM_HEIGHT);
                    if item_top + item_height > height {
                        break;
                    }
                    bounds.push(
                        ComponentBounds::new(
                            format!("Item[{}]", i),
                            gpui::Bounds {
                                origin: gpui::point(px(0.), item_top),
                                size: gpui::size(width, item_height),
                            },
                        )
                        .with_type(ComponentType::ListItem)
                        .with_padding(BoxModel::symmetric(12.0, content_padding)),
                    );
                }
            }

            AppView::AgentChatView { .. } => {
                let info_columns =
                    crate::components::main_view_chrome::main_view_content_columns(menu_def);
                let info_metrics = crate::components::info_state::info_metrics(
                    crate::components::info_state::InfoStateDensity::Comfortable,
                );
                let info_x = px(info_columns.text_column_x);
                let info_y = content_top + px(info_columns.top_inset_y);
                let info_width =
                    (width - info_x - px(info_columns.content_right_inset_x)).max(px(0.));
                let shortcut_slot_width = px(
                    crate::components::footer_chrome::FOOTER_KEYCAP_HEIGHT_PX * 2.0
                        + crate::components::footer_chrome::FOOTER_ACTION_CONTENT_GAP_PX,
                );
                let guidance_label_x = info_x
                    + shortcut_slot_width
                    + px(crate::components::info_state::INFO_SPACING.sm);
                let guidance_label_width = (info_width
                    - shortcut_slot_width
                    - px(crate::components::info_state::INFO_SPACING.sm))
                .max(px(0.));

                bounds.push(
                    ComponentBounds::new(
                        "AgentChatConversation",
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(width, content_height),
                        },
                    )
                    .with_type(ComponentType::List)
                    .with_padding(BoxModel::uniform(0.0)),
                );
                bounds.push(
                    ComponentBounds::new(
                        "AgentChatEmptyGuidance",
                        gpui::Bounds {
                            origin: gpui::point(info_x, info_y),
                            size: gpui::size(
                                info_width,
                                (content_height
                                    - px(info_columns.top_inset_y)
                                    - px(menu_def.shell.content_inset_bottom))
                                .max(px(0.)),
                            ),
                        },
                    )
                    .with_type(ComponentType::Container)
                    .with_padding(BoxModel::uniform(0.0)),
                );
                bounds.push(
                    ComponentBounds::new(
                        "AgentChatEmptyGuidanceTitle",
                        gpui::Bounds {
                            origin: gpui::point(info_x, info_y),
                            size: gpui::size(
                                info_width,
                                px(crate::components::info_state::INFO_TYPE_SCALE.title.line),
                            ),
                        },
                    )
                    .with_type(ComponentType::Header)
                    .with_padding(BoxModel::uniform(0.0)),
                );
                bounds.push(
                    ComponentBounds::new(
                        "AgentChatEmptyGuidanceShortcutSlot",
                        gpui::Bounds {
                            origin: gpui::point(
                                info_x,
                                info_y
                                    + px(info_metrics.block_gap)
                                    + px(crate::components::info_state::INFO_TYPE_SCALE.title.line)
                                    + px(crate::components::info_state::INFO_SPACING.xs * 0.5)
                                    + px(crate::components::info_state::INFO_TYPE_SCALE.body.line),
                            ),
                            size: gpui::size(shortcut_slot_width, px(info_metrics.row_min_h)),
                        },
                    )
                    .with_type(ComponentType::Other)
                    .with_padding(BoxModel::uniform(0.0)),
                );
                bounds.push(
                    ComponentBounds::new(
                        "AgentChatEmptyGuidanceLabelColumn",
                        gpui::Bounds {
                            origin: gpui::point(
                                guidance_label_x,
                                info_y
                                    + px(info_metrics.block_gap)
                                    + px(crate::components::info_state::INFO_TYPE_SCALE.title.line)
                                    + px(crate::components::info_state::INFO_SPACING.xs * 0.5)
                                    + px(crate::components::info_state::INFO_TYPE_SCALE.body.line),
                            ),
                            size: gpui::size(guidance_label_width, px(info_metrics.row_min_h)),
                        },
                    )
                    .with_type(ComponentType::Other)
                    .with_padding(BoxModel::uniform(0.0)),
                );
            }

            AppView::DayPage { .. } => {
                let footer_height = px(menu_def.footer.metrics.height_px);
                let body_height = (content_height - footer_height).max(px(0.));
                bounds.push(
                    ComponentBounds::new(
                        "DayPageSurface",
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(width, body_height),
                        },
                    )
                    .with_type(ComponentType::Container)
                    .with_padding(BoxModel::uniform(0.0)),
                );
            }

            // Other prompts - generic full-width content
            _ => {
                bounds.push(
                    ComponentBounds::new(
                        view_name,
                        gpui::Bounds {
                            origin: gpui::point(px(0.), content_top),
                            size: gpui::size(width, content_height),
                        },
                    )
                    .with_type(ComponentType::Prompt)
                    .with_padding(BoxModel::uniform(content_padding)),
                );
            }
        }

        let context_outset_x = px(menu_def.header_info_bar.context_edge_outset_x);
        bounds.push(
            ComponentBounds::new(
                "MainViewContextZone",
                gpui::Bounds {
                    origin: gpui::point(px(header_metrics.context_x), px(header_metrics.context_y)),
                    size: gpui::size(
                        (width - px(menu_def.shell.header_padding_x * 2.0)
                            + context_outset_x * 2.0)
                            .max(px(0.)),
                        px(header_metrics.context_height),
                    ),
                },
            )
            .with_type(ComponentType::Container),
        );

        if let Some(input_height) = header_metrics.input_height {
            let input_x = px(header_metrics.input_x);
            let input_y = px(header_metrics.input_y);
            let input_width = (width - (input_x * 2.)).max(px(0.));
            bounds.push(
                ComponentBounds::new(
                    "MainViewInput",
                    gpui::Bounds {
                        origin: gpui::point(input_x, input_y),
                        size: gpui::size(input_width, px(input_height)),
                    },
                )
                .with_type(ComponentType::Input)
                .with_padding(BoxModel::symmetric(0.0, 0.0)),
            );
        }

        if matches!(self.current_view, AppView::ScriptList) {
            let uses_split_preview = matches!(self.main_window_mode, MainWindowMode::Full);
            let list_width = if uses_split_preview {
                width * 0.5
            } else {
                width
            };

            if uses_split_preview {
                // Preview panel contents (right 50% of window)
                // Preview has its own padding, content starts at list_width + padding
                let preview_padding = 16.0_f32;
                let preview_left = list_width + px(preview_padding);
                let preview_width = width * 0.5 - px(preview_padding * 2.0);

                // Script path label (small text at top of preview)
                bounds.push(
                    ComponentBounds::new(
                        "ScriptPath",
                        gpui::Bounds {
                            origin: gpui::point(preview_left, content_top + px(8.)),
                            size: gpui::size(preview_width, px(16.)),
                        },
                    )
                    .with_type(ComponentType::Other)
                    .with_padding(BoxModel::symmetric(2.0, 0.0)),
                );

                // Script title (large heading)
                bounds.push(
                    ComponentBounds::new(
                        "ScriptTitle",
                        gpui::Bounds {
                            origin: gpui::point(preview_left, content_top + px(32.)),
                            size: gpui::size(preview_width, px(32.)),
                        },
                    )
                    .with_type(ComponentType::Header)
                    .with_padding(BoxModel::symmetric(4.0, 0.0)),
                );

                // Description label
                bounds.push(
                    ComponentBounds::new(
                        "DescLabel", // Shortened
                        gpui::Bounds {
                            origin: gpui::point(preview_left, content_top + px(72.)),
                            size: gpui::size(px(80.), px(16.)),
                        },
                    )
                    .with_type(ComponentType::Other)
                    .with_padding(BoxModel::uniform(2.0)),
                );

                // Description value
                bounds.push(
                    ComponentBounds::new(
                        "DescValue", // Shortened
                        gpui::Bounds {
                            origin: gpui::point(preview_left, content_top + px(92.)),
                            size: gpui::size(preview_width, px(20.)),
                        },
                    )
                    .with_type(ComponentType::Other)
                    .with_padding(BoxModel::symmetric(2.0, 0.0)),
                );

                // Code Preview label
                bounds.push(
                    ComponentBounds::new(
                        "CodeLabel", // Shortened from CodePreviewLabel
                        gpui::Bounds {
                            origin: gpui::point(preview_left, content_top + px(130.)),
                            size: gpui::size(px(100.), px(16.)),
                        },
                    )
                    .with_type(ComponentType::Other)
                    .with_padding(BoxModel::uniform(2.0)),
                );

                // Code preview area
                bounds.push(
                    ComponentBounds::new(
                        "CodePreview",
                        gpui::Bounds {
                            origin: gpui::point(preview_left, content_top + px(150.)),
                            size: gpui::size(preview_width, height - content_top - px(170.)),
                        },
                    )
                    .with_type(ComponentType::Container)
                    .with_padding(BoxModel::uniform(12.0)),
                );
            }

            // List item icons (left side of each list item)
            // Icons are typically 24x24, positioned with some padding from left edge
            // Item height is LIST_ITEM_HEIGHT, icon vertically centered: (LIST_ITEM_HEIGHT - 24) / 2 from top
            let item_height = px(LIST_ITEM_HEIGHT);
            for i in 0..5 {
                let item_top = content_top + px(i as f32 * LIST_ITEM_HEIGHT);
                if item_top + item_height > height {
                    break;
                }
                bounds.push(
                    ComponentBounds::new(
                        format!("Icon[{}]", i),
                        gpui::Bounds {
                            origin: gpui::point(
                                px(content_padding),
                                item_top + px((LIST_ITEM_HEIGHT - 24.0) / 2.0),
                            ),
                            size: gpui::size(px(24.), px(24.)),
                        },
                    )
                    .with_type(ComponentType::Other)
                    .with_padding(BoxModel::uniform(0.0)),
                );
            }
        } // End of ScriptList-specific bounds

        if self.current_view.native_footer_surface().is_some() {
            let footer_height = px(menu_def.footer.metrics.height_px);
            bounds.push(
                ComponentBounds::new(
                    "MainViewFooter",
                    gpui::Bounds {
                        origin: gpui::point(px(0.), (height - footer_height).max(px(0.))),
                        size: gpui::size(width, footer_height),
                    },
                )
                .with_type(ComponentType::Container)
                .with_padding(BoxModel::symmetric(
                    menu_def.footer.metrics.button_padding_y,
                    menu_def.footer.metrics.side_inset_px,
                )),
            );
        }

        bounds
    }
}
