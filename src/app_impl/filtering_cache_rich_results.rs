use super::*;

/// Which file-backed colon mode the rows belong to: `@file:` (global
/// Spotlight) or `@project:` (cwd-scoped). Same row anatomy, different
/// header language and icon.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileSubsearchFlavor {
    Global,
    Project,
}

impl FileSubsearchFlavor {
    fn icon(self) -> &'static str {
        match self {
            Self::Global => "file",
            Self::Project => "folder",
        }
    }

    fn recent_header(self) -> &'static str {
        match self {
            Self::Global => "Recent Files",
            Self::Project => "Recent Project Files",
        }
    }

    fn finding_header(self) -> &'static str {
        match self {
            Self::Global => "Finding recent files\u{2026}",
            Self::Project => "Finding project files\u{2026}",
        }
    }

    fn empty_section_header(self) -> &'static str {
        match self {
            Self::Global => "Files",
            Self::Project => "Project Files",
        }
    }

    fn no_recent_header(self) -> &'static str {
        match self {
            Self::Global => "No recent files",
            Self::Project => "No recent project files",
        }
    }

    fn matching_header(self, query: &str) -> String {
        match self {
            Self::Global => format!("Files matching \u{201c}{query}\u{201d}"),
            Self::Project => format!("Project files matching \u{201c}{query}\u{201d}"),
        }
    }

    fn searching_header(self) -> &'static str {
        match self {
            Self::Global => "Searching files\u{2026}",
            Self::Project => "Searching project files\u{2026}",
        }
    }

    fn no_match_header(self, query: &str) -> String {
        match self {
            Self::Global => format!("No files matching \u{201c}{query}\u{201d}"),
            Self::Project => format!("No project files matching \u{201c}{query}\u{201d}"),
        }
    }
}

pub(crate) fn build_rich_file_subsearch_rows(
    flavor: FileSubsearchFlavor,
    query: &str,
    loading: bool,
    provider_results: &[crate::file_search::FileResult],
    recent_results: &[crate::file_search::FileResult],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let query = query.trim();
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let icon = flavor.icon().to_string();

    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    if query.is_empty() {
        // A3 recent-files decision (2026-06-09): the empty landing state
        // blends frecency picks (files chosen through Script Kit) with the
        // provider's recently-used seed, deduped by path.
        let mut seen = std::collections::HashSet::new();
        let combined: Vec<&crate::file_search::FileResult> = recent_results
            .iter()
            .chain(provider_results.iter())
            .filter(|file| seen.insert(file.path.as_str()))
            .take(limit)
            .collect();
        if !combined.is_empty() {
            grouped.push(GroupedListItem::SectionHeader(
                flavor.recent_header().to_string(),
                Some(icon),
            ));
            for file in combined {
                let idx = flat.len();
                flat.push(scripts::SearchResult::File(scripts::FileMatch {
                    file: (*file).clone(),
                    score: 0,
                }));
                grouped.push(GroupedListItem::Item(idx));
            }
        } else if loading {
            grouped.push(GroupedListItem::SectionHeader(
                flavor.finding_header().to_string(),
                Some(icon),
            ));
        } else {
            grouped.push(GroupedListItem::SectionHeader(
                flavor.empty_section_header().to_string(),
                Some(icon),
            ));
            grouped.push(GroupedListItem::SectionHeader(
                flavor.no_recent_header().to_string(),
                None,
            ));
        }
    } else if !provider_results.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            flavor.matching_header(query),
            Some(icon),
        ));
        for file in provider_results.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::File(scripts::FileMatch {
                file: file.clone(),
                score: 0,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    } else if loading {
        grouped.push(GroupedListItem::SectionHeader(
            flavor.searching_header().to_string(),
            Some(icon),
        ));
    } else {
        grouped.push(GroupedListItem::SectionHeader(
            flavor.no_match_header(query),
            Some(icon),
        ));
    }

    (grouped, flat)
}

pub(crate) fn build_rich_clipboard_subsearch_rows(
    query: &str,
    hits: &[crate::clipboard_history::ClipboardEntryMeta],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    let header = if query.trim().is_empty() {
        "Recent Clipboard".to_string()
    } else {
        format!("Clipboard matching \u{201c}{}\u{201d}", query.trim())
    };

    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("clipboard".to_string()),
    ));

    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "Clipboard is empty".to_string()
            } else {
                format!(
                    "No clipboard entries matching \u{201c}{}\u{201d}",
                    query.trim()
                )
            },
            None,
        ));
    } else {
        for entry in hits.iter().take(limit) {
            let idx = flat.len();
            let title = crate::spine::text_preview::single_line_truncate(&entry.text_preview, 72);
            flat.push(scripts::SearchResult::ClipboardHistory(
                scripts::ClipboardHistoryMatch {
                    entry: entry.clone(),
                    title: title.clone(),
                    subtitle: "Clipboard History".to_string(),
                    score: 0,
                },
            ));
            grouped.push(GroupedListItem::Item(idx));
        }
    }

    (grouped, flat)
}

pub(crate) fn build_rich_browser_history_rows(
    query: &str,
    hits: &[crate::browser_history::RootBrowserHistorySearchHit],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Recent Browser History".to_string()
    } else {
        format!("Browser history matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("globe".to_string()),
    ));
    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "No browser history".to_string()
            } else {
                format!("No history matching \u{201c}{}\u{201d}", query.trim())
            },
            None,
        ));
    } else {
        for hit in hits.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::BrowserHistory(
                scripts::BrowserHistoryMatch {
                    hit: hit.clone(),
                    subtitle: hit.url.clone(),
                    score: 0,
                },
            ));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_notes_rows(
    query: &str,
    hits: &[crate::notes::RootNoteSearchHit],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Recent Notes".to_string()
    } else {
        format!("Notes matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("notebook-text".to_string()),
    ));
    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "No notes".to_string()
            } else {
                format!("No notes matching \u{201c}{}\u{201d}", query.trim())
            },
            None,
        ));
    } else {
        for hit in hits.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::Note(scripts::NoteMatch {
                hit: hit.clone(),
                title: hit.title.clone(),
                subtitle: format!("{} chars", hit.char_count),
                score: 0,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_dictation_rows(
    query: &str,
    hits: &[crate::dictation::RootDictationHistorySearchHit],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Recent Dictation".to_string()
    } else {
        format!("Dictation matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("mic".to_string()),
    ));
    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "No dictation history".to_string()
            } else {
                format!("No dictation matching \u{201c}{}\u{201d}", query.trim())
            },
            None,
        ));
    } else {
        for hit in hits.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::DictationHistory(
                scripts::DictationHistoryMatch {
                    id: hit.id.clone(),
                    preview: hit.preview.clone(),
                    target: hit.target.clone(),
                    timestamp: hit.timestamp.clone(),
                    audio_duration_ms: hit.audio_duration_ms,
                    subtitle: hit.target.clone(),
                    score: 0,
                    matched_field: hit.matched_field,
                    evidence: hit.evidence.clone(),
                },
            ));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_agent_chat_history_rows(
    query: &str,
    hits: &[crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Recent Agent Chat".to_string()
    } else {
        format!("Chat history matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("message-square".to_string()),
    ));
    if hits.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            if query.trim().is_empty() {
                "No chat history".to_string()
            } else {
                format!("No history matching \u{201c}{}\u{201d}", query.trim())
            },
            None,
        ));
    } else {
        for hit in hits.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::AgentChatHistory(
                scripts::AgentChatHistoryMatch {
                    entry: hit.entry.clone(),
                    score: 0,
                    matched_field: hit.matched_field,
                    subtitle: hit.entry.title_display().to_string(),
                    evidence: hit.evidence.clone(),
                },
            ));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_script_rows(
    query: &str,
    all_scripts: &[std::sync::Arc<scripts::Script>],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Scripts".to_string()
    } else {
        format!("Scripts matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("code".to_string()),
    ));
    let matches = crate::spine::catalog_subsearch::rank_context_catalog_results(
        crate::spine::catalog_subsearch::ContextSubsearchSource::Scripts,
        query.trim(),
        all_scripts,
        &[],
        &[],
    );
    if matches.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            format!("No scripts matching \u{201c}{}\u{201d}", query.trim()),
            None,
        ));
    } else {
        for result in matches {
            let idx = flat.len();
            flat.push(result);
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_scriptlet_rows(
    query: &str,
    all_scriptlets: &[std::sync::Arc<scripts::Scriptlet>],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Scriptlets".to_string()
    } else {
        format!("Scriptlets matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("workflow".to_string()),
    ));
    let matches = crate::spine::catalog_subsearch::rank_context_catalog_results(
        crate::spine::catalog_subsearch::ContextSubsearchSource::Scriptlets,
        query.trim(),
        &[],
        all_scriptlets,
        &[],
    );
    if matches.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            format!("No scriptlets matching \u{201c}{}\u{201d}", query.trim()),
            None,
        ));
    } else {
        for result in matches {
            let idx = flat.len();
            flat.push(result);
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_skill_rows(
    query: &str,
    all_skills: &[std::sync::Arc<crate::plugins::PluginSkill>],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();
    let header = if query.trim().is_empty() {
        "Skills".to_string()
    } else {
        format!("Skills matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("zap".to_string()),
    ));
    let matches = crate::spine::catalog_subsearch::rank_context_catalog_results(
        crate::spine::catalog_subsearch::ContextSubsearchSource::Skills,
        query.trim(),
        &[],
        &[],
        all_skills,
    );
    if matches.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            format!("No skills matching \u{201c}{}\u{201d}", query.trim()),
            None,
        ));
    } else {
        for result in matches {
            let idx = flat.len();
            flat.push(result);
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(crate) fn build_rich_provider_json_rows(
    query: &str,
    kind: crate::mcp_resources::ProviderJsonResourceKind,
    section_label: &str,
    icon: &str,
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    use crate::spine::list::{ss, SpineListAction, SpineListRow, SpineListRowKind};

    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    let items = crate::mcp_resources::read_provider_json_items(kind);
    let query_lower = query.trim().to_lowercase();

    let header = if query_lower.is_empty() {
        section_label.to_string()
    } else {
        format!("{section_label} matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some(icon.to_string()),
    ));

    let matches: Vec<_> = items
        .iter()
        .filter(|item| {
            query_lower.is_empty()
                || item.title.to_lowercase().contains(&query_lower)
                || item
                    .subtitle
                    .as_deref()
                    .is_some_and(|s| s.to_lowercase().contains(&query_lower))
        })
        .take(limit)
        .collect();

    if matches.is_empty() {
        if items.is_empty() {
            grouped.push(GroupedListItem::SectionHeader(
                format!("No {section_label} available"),
                None,
            ));
        } else {
            grouped.push(GroupedListItem::SectionHeader(
                format!(
                    "No {section_label} matching \u{201c}{}\u{201d}",
                    query.trim()
                ),
                None,
            ));
        }
    } else {
        for (rank, item) in matches.iter().enumerate() {
            let idx = flat.len();
            let prefix = match kind {
                crate::mcp_resources::ProviderJsonResourceKind::Calendar => "calendar",
                crate::mcp_resources::ProviderJsonResourceKind::Notifications => "notifications",
                crate::mcp_resources::ProviderJsonResourceKind::Dictation => "dictation",
            };
            flat.push(scripts::SearchResult::SpineProjection(SpineListRow {
                id: ss(format!("spine:provider-json:{prefix}:{rank}")),
                kind: SpineListRowKind::ContextResult {
                    context_type: ss(prefix),
                    result_id: ss(format!("{rank}")),
                },
                title: ss(item.title.clone()),
                subtitle: item.subtitle.clone().map(ss),
                meta: None,
                icon: Some(ss(icon.to_string())),
                badges: vec![ss("@")],
                score: i32::MAX.saturating_sub(rank as i32),
                is_selectable: true,
                action_label: Some(ss("Attach")),
                action: SpineListAction::Noop,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    }
    (grouped, flat)
}

pub(super) fn main_menu_agent_chat_cwd_context(
) -> Option<(std::path::PathBuf, Vec<std::path::PathBuf>)> {
    let ai_preferences = crate::config::load_user_preferences().ai;
    let profile_ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
    let profile =
        crate::ai::agent_chat::profiles::resolve_effective_profile(&ai_preferences, &profile_ctx);
    // Pure cwd-only resolution: the full launch resolver runs create_dir_all
    // and logs per call, which is unacceptable in this per-keystroke path.
    let cwd =
        crate::ai::agent_chat::launch::resolve_selected_launch_cwd(&ai_preferences, &profile_ctx);
    let recents = crate::ai::agent_chat::ui::agent_chat_cwd_recents_for_profile(&profile.id);
    Some((cwd, recents))
}

pub(crate) fn build_rich_cwd_root_rows(
    recent_dirs: &[crate::file_search::FileResult],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    if !recent_dirs.is_empty() {
        grouped.push(GroupedListItem::SectionHeader(
            "Recent Directories".to_string(),
            Some("folder".to_string()),
        ));
        for dir in recent_dirs.iter().take(limit) {
            let idx = flat.len();
            flat.push(scripts::SearchResult::File(scripts::FileMatch {
                file: dir.clone(),
                score: 0,
            }));
            grouped.push(GroupedListItem::Item(idx));
        }
    } else {
        grouped.push(GroupedListItem::SectionHeader(
            "Project / CWD".to_string(),
            Some("folder".to_string()),
        ));
        grouped.push(GroupedListItem::SectionHeader(
            "No recent directories".to_string(),
            None,
        ));
    }
    (grouped, flat)
}

pub(crate) fn build_rich_cwd_subsearch_rows(
    query: &str,
    recent_dirs: &[crate::file_search::FileResult],
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let mut grouped = Vec::new();
    let mut flat: Vec<scripts::SearchResult> = Vec::new();

    let q = query.trim().to_lowercase();
    let matches: Vec<_> = recent_dirs
        .iter()
        .filter(|d| d.name.to_lowercase().contains(&q) || d.path.to_lowercase().contains(&q))
        .take(limit)
        .collect();

    let header = if matches.is_empty() {
        format!("No directories matching \u{201c}{}\u{201d}", query.trim())
    } else {
        format!("Directories matching \u{201c}{}\u{201d}", query.trim())
    };
    grouped.push(GroupedListItem::SectionHeader(
        header,
        Some("folder".to_string()),
    ));

    for dir in matches {
        let idx = flat.len();
        flat.push(scripts::SearchResult::File(scripts::FileMatch {
            file: dir.clone(),
            score: 0,
        }));
        grouped.push(GroupedListItem::Item(idx));
    }
    (grouped, flat)
}
