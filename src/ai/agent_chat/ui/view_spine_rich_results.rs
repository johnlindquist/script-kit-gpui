impl AgentChatView {
    fn agent_chat_spine_sections(&self) -> Vec<SpineListSection> {
        if !self.agent_chat_spine_has_context_projection() {
            return Vec::new();
        }
        let Some(projection) = self.composer_spine.input.projection.as_ref() else {
            return Vec::new();
        };

        if let crate::spine::SpineSegmentKind::ContextMention {
            context_type,
            sub_query,
        } = &projection.active_segment_kind
        {
            if let Some((source, rich_query)) =
                crate::spine::catalog_subsearch::parse_context_subsearch(
                    context_type,
                    sub_query.as_deref(),
                )
            {
                let segment_index = projection.active_segment_index;
                let Some(segment_byte_range) = self
                    .composer_spine
                    .input
                    .parse
                    .segments
                    .get(segment_index)
                    .map(|segment| segment.byte_range.clone())
                else {
                    return Vec::new();
                };

                return match source {
                    crate::spine::catalog_subsearch::ContextSubsearchSource::File => {
                        let files = crate::file_search::search_files(rich_query, None, 10);
                        vec![self.agent_chat_rich_file_subsearch_section(
                            rich_query,
                            segment_index,
                            segment_byte_range,
                            &files,
                        )]
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Project => {
                        // Scoped to the thread cwd snapshot; `search_files`
                        // already falls back to a filesystem walk when
                        // Spotlight can't serve the scope (dot-directories).
                        let scope = self
                            .composer_spine
                            .project_scope_cwd
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .or_else(|| {
                                dirs::home_dir().map(|home| home.to_string_lossy().to_string())
                            });
                        let files =
                            crate::file_search::search_files(rich_query, scope.as_deref(), 10);
                        vec![self.agent_chat_rich_project_subsearch_section(
                            rich_query,
                            segment_index,
                            segment_byte_range,
                            &files,
                        )]
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard => {
                        let options =
                            crate::clipboard_history::RootClipboardHistorySectionOptions {
                                enabled: true,
                                max_results: 10,
                                min_query_chars: 0,
                                ..Default::default()
                            };
                        let hits =
                            crate::clipboard_history::search_root_clipboard_history_meta_direct(
                                rich_query, options,
                            );
                        vec![self.agent_chat_rich_clipboard_subsearch_section(
                            rich_query,
                            segment_index,
                            segment_byte_range,
                            &hits,
                        )]
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Notes
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::BrowserHistory
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::Dictation
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::History
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::Calendar
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::Notifications => {
                        // Composer parity with the main window: these sources
                        // resolve through the shared spine attach resolver.
                        match crate::spine::attach::composer_subsearch_section(
                            source,
                            rich_query,
                            segment_index,
                            segment_byte_range,
                        ) {
                            Some(section) => {
                                vec![Self::agent_chat_rich_shared_subsearch_section(
                                    section, rich_query,
                                )]
                            }
                            None => Vec::new(),
                        }
                    }
                    source @ (crate::spine::catalog_subsearch::ContextSubsearchSource::Scripts
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::Scriptlets
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::Skills) => {
                        match crate::ai::context_selector::composer_catalog_subsearch_section(
                            source,
                            rich_query,
                            segment_index,
                            segment_byte_range,
                        ) {
                            Some(section) => {
                                vec![Self::agent_chat_rich_shared_subsearch_section(
                                    section, rich_query,
                                )]
                            }
                            None => Vec::new(),
                        }
                    }
                };
            }
        }

        let sections =
            crate::spine::list::build_spine_list_sections_full_with_resolved_tokens_and_context(
                &self.composer_spine.input.parse,
                projection,
                None,
                &|token| self.typed_mention_aliases.contains_key(token),
                crate::spine::list::SpineListBuildContext {
                    current_cwd: self.composer_spine.project_scope_cwd.as_deref(),
                    cwd_recents: &self.composer_spine.project_scope_cwd_recents,
                },
            );
        // C-R3: strip CWD + profile sections from the projection so typing `>`
        // (cwd) or the profile trigger cannot even DISPLAY, let alone select,
        // an affordance the session policy denies (Quick AI).
        self.filter_agent_chat_spine_sections_by_policy(sections)
    }

    /// C-R3 projection guard: drop CWD/profile rows the session policy forbids
    /// and any section left with no selectable row. Uses the view's captured
    /// (thread-derived, tighten-only) policy — safe cx-free because it is never
    /// LESS restrictive than the thread, and this is a restriction filter.
    fn filter_agent_chat_spine_sections_by_policy(
        &self,
        sections: Vec<SpineListSection>,
    ) -> Vec<SpineListSection> {
        let caps = self.session_policy.capabilities();
        if caps.cwd_picker && caps.profile_switch {
            return sections;
        }
        sections
            .into_iter()
            .filter_map(|mut section| {
                let before = section.rows.len();
                section.rows.retain(|row| match &row.action {
                    SpineListAction::ResolveSegment {
                        resolution_source, ..
                    } => match resolution_source.as_ref() {
                        "cwd" => caps.cwd_picker,
                        "profile" => caps.profile_switch,
                        _ => true,
                    },
                    _ => true,
                });
                // A section that lost rows and now has nothing selectable is a
                // denied section — drop it rather than show an orphan header.
                if section.rows.len() != before && !section.rows.iter().any(|row| row.is_selectable)
                {
                    return None;
                }
                Some(section)
            })
            .collect()
    }

    fn agent_chat_rich_file_subsearch_section(
        &self,
        query: &str,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        files: &[crate::file_search::FileResult],
    ) -> SpineListSection {
        self.agent_chat_rich_file_backed_subsearch_section(
            query,
            segment_index,
            segment_byte_range,
            files,
            "Files",
            "@file:",
        )
    }

    fn agent_chat_rich_project_subsearch_section(
        &self,
        query: &str,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        files: &[crate::file_search::FileResult],
    ) -> SpineListSection {
        self.agent_chat_rich_file_backed_subsearch_section(
            query,
            segment_index,
            segment_byte_range,
            files,
            "Project Files",
            "@project:",
        )
    }

    fn agent_chat_rich_file_backed_subsearch_section(
        &self,
        query: &str,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        files: &[crate::file_search::FileResult],
        noun: &str,
        prefix: &str,
    ) -> SpineListSection {
        let trimmed = query.trim();
        let title = if trimmed.is_empty() {
            noun.to_string()
        } else {
            format!("{noun} matching \"{trimmed}\"")
        };
        let empty_subtitle = format!("Type after {prefix} to search");
        let rows = if files.is_empty() {
            vec![Self::agent_chat_spine_hint_row(
                "No files",
                if trimmed.is_empty() {
                    &empty_subtitle
                } else {
                    "No matching files"
                },
                Some("file"),
            )]
        } else {
            files
                .iter()
                .take(10)
                .enumerate()
                .map(|(index, file)| {
                    let short_path = crate::file_search::shorten_path(&file.path);
                    let replacement = format!(
                        "@file:{}",
                        crate::spine::catalog_subsearch::escape_ref_component(&short_path),
                    );
                    SpineListRow {
                        id: SharedString::from(format!(
                            "agent_chat-spine:file:{index}:{}",
                            file.path
                        )),
                        kind: SpineListRowKind::ContextResult {
                            context_type: SharedString::from("file"),
                            result_id: SharedString::from(file.path.clone()),
                        },
                        title: SharedString::from(file.name.clone()),
                        subtitle: Some(SharedString::from(short_path)),
                        meta: None,
                        icon: Some(SharedString::from("file")),
                        badges: Vec::new(),
                        score: 0,
                        is_selectable: true,
                        action_label: None,
                        action: SpineListAction::ResolveSegment {
                            segment_index,
                            segment_byte_range: segment_byte_range.clone(),
                            replacement: SharedString::from(replacement),
                            resolution_id: SharedString::from(file.path.clone()),
                            resolution_label: SharedString::from(file.name.clone()),
                            resolution_source: SharedString::from("file"),
                            trailing_space: true,
                        },
                    }
                })
                .collect()
        };

        SpineListSection {
            id: SharedString::from("agent_chat-spine-section-subsearch:file"),
            title: SharedString::from(title),
            subtitle: Some(SharedString::from("@file:")),
            icon: Some(SharedString::from("file")),
            rows,
        }
    }

    fn agent_chat_rich_clipboard_subsearch_section(
        &self,
        query: &str,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        hits: &[crate::clipboard_history::ClipboardEntryMeta],
    ) -> SpineListSection {
        let trimmed = query.trim();
        let title = if trimmed.is_empty() {
            "Recent Clipboard".to_string()
        } else {
            format!("Clipboard matching \"{trimmed}\"")
        };
        let rows = if hits.is_empty() {
            vec![Self::agent_chat_spine_hint_row(
                "No clipboard entries",
                if trimmed.is_empty() {
                    "Clipboard is empty"
                } else {
                    "No matching clipboard entries"
                },
                Some("clipboard"),
            )]
        } else {
            hits.iter()
                .take(10)
                .enumerate()
                .map(|(index, entry)| {
                    let preview =
                        crate::spine::text_preview::single_line_truncate(&entry.text_preview, 72);
                    let replacement = format!(
                        "@clipboard:{}",
                        crate::spine::catalog_subsearch::escape_ref_component(&entry.id),
                    );
                    SpineListRow {
                        id: SharedString::from(format!(
                            "agent_chat-spine:clipboard:{index}:{}",
                            entry.id
                        )),
                        kind: SpineListRowKind::ContextResult {
                            context_type: SharedString::from("clipboard"),
                            result_id: SharedString::from(entry.id.clone()),
                        },
                        title: SharedString::from(preview.clone()),
                        subtitle: Some(SharedString::from("Clipboard History")),
                        meta: None,
                        icon: Some(SharedString::from("clipboard")),
                        badges: Vec::new(),
                        score: 0,
                        is_selectable: true,
                        action_label: None,
                        action: SpineListAction::ResolveSegment {
                            segment_index,
                            segment_byte_range: segment_byte_range.clone(),
                            replacement: SharedString::from(replacement),
                            resolution_id: SharedString::from(entry.id.clone()),
                            resolution_label: SharedString::from(format!("Clipboard: {preview}")),
                            resolution_source: SharedString::from("clipboard"),
                            trailing_space: true,
                        },
                    }
                })
                .collect()
        };

        SpineListSection {
            id: SharedString::from("agent_chat-spine-section-subsearch:clipboard"),
            title: SharedString::from(title),
            subtitle: Some(SharedString::from("@clipboard:")),
            icon: Some(SharedString::from("clipboard")),
            rows,
        }
    }

    /// Convert a shared-resolver subsearch section into the composer's
    /// dropdown section, with an explicit empty row when nothing matches.
    fn agent_chat_rich_shared_subsearch_section(
        section: crate::spine::attach::ComposerSubsearchSection,
        query: &str,
    ) -> SpineListSection {
        let trimmed = query.trim();
        let rows = if section.rows.is_empty() {
            vec![Self::agent_chat_spine_hint_row(
                "No results",
                if trimmed.is_empty() {
                    "Nothing to attach from this source yet"
                } else {
                    "No matching entries"
                },
                Some(section.icon),
            )]
        } else {
            section.rows.into_iter().map(|row| row.row).collect()
        };
        SpineListSection {
            id: SharedString::from(format!(
                "agent_chat-spine-section-subsearch:{}",
                section.source_id
            )),
            title: SharedString::from(section.title),
            subtitle: Some(SharedString::from(format!("@{}:", section.source_id))),
            icon: Some(SharedString::from(section.icon.to_string())),
            rows,
        }
    }

    /// Re-derive the alias content for a shared-resolver subsearch token at
    /// accept time. The projection still reflects the pre-replacement input,
    /// so re-running the same deterministic query finds the accepted row.
    fn agent_chat_rich_subsearch_alias(
        &self,
        token: &str,
        resolution_id: &str,
    ) -> Option<crate::ai::message_parts::AiContextPart> {
        let projection = self.composer_spine.input.projection.as_ref()?;
        let crate::spine::SpineSegmentKind::ContextMention {
            context_type,
            sub_query,
        } = &projection.active_segment_kind
        else {
            return None;
        };
        let (source, rich_query) = crate::spine::catalog_subsearch::parse_context_subsearch(
            context_type,
            sub_query.as_deref(),
        )?;
        let section = crate::spine::attach::composer_subsearch_section(source, rich_query, 0, 0..0)
            .or_else(|| {
                crate::ai::context_selector::composer_catalog_subsearch_section(
                    source,
                    rich_query,
                    0,
                    0..0,
                )
            })?;
        crate::spine::attach::composer_subsearch_alias_for_resolution(section, token, resolution_id)
    }

    fn agent_chat_spine_hint_row(title: &str, subtitle: &str, icon: Option<&str>) -> SpineListRow {
        SpineListRow {
            id: SharedString::from(format!("agent_chat-spine:hint:{title}:{subtitle}")),
            kind: SpineListRowKind::Hint,
            title: SharedString::from(title.to_string()),
            subtitle: Some(SharedString::from(subtitle.to_string())),
            meta: None,
            icon: icon.map(|icon| SharedString::from(icon.to_string())),
            badges: Vec::new(),
            score: 0,
            is_selectable: false,
            action_label: None,
            action: SpineListAction::Noop,
        }
    }

    fn agent_chat_spine_selectable_rows(&self) -> Vec<SpineListRow> {
        self.agent_chat_spine_rows()
            .into_iter()
            .filter(|row| row.is_selectable)
            .collect()
    }
}
