//! Universal attach resolution for spine `@source:` subsearch rows.
//!
//! Every context subsearch source the spine advertises must keep the same
//! promise on Enter: the selected result resolves into a compact
//! `@source:label` token in the input plus a `spine_mention_aliases` entry
//! carrying the real content, so the prompt plan attaches it at submit.
//!
//! Before this module existed only File and Clipboard were intercepted —
//! Enter on a `@notes:` / `@scripts:` / `@browser-history:` row fell through
//! to default launcher execution (opening the note, running the script) and
//! destroyed the prompt being built.

use std::ops::Range;

use crate::ai::message_parts::AiContextPart;
use crate::scripts::SearchResult;
use crate::spine::catalog_subsearch::{escape_ref_component, ContextSubsearchSource};
use crate::spine::SpineListAction;

/// The result of intercepting Enter on a rich subsearch row: the segment
/// resolution action plus the alias to register (token → content part)
/// before the action is applied.
pub(crate) struct SpineAttachOutcome {
    pub action: SpineListAction,
    /// `None` for sources (file, clipboard) whose alias registration is
    /// owned by the `ResolveSegment` apply arm for parity with the
    /// attachment portal path.
    pub alias: Option<(String, AiContextPart)>,
}

/// Canonical compact spine token for a selected file: `@file:` plus the
/// friendly escaped basename. Both the inline subsearch accept and the
/// file-search portal accept must produce the same token so the alias
/// registry and the prompt plan resolve it identically.
pub(crate) fn spine_file_mention_token(path: &str) -> String {
    let basename = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    // Friendly token: whitespace runs become `-` so the token reads as
    // one word instead of `%20` soup; reserved chars stay escaped.
    let friendly = basename.split_whitespace().collect::<Vec<_>>().join("-");
    format!("@file:{}", escape_ref_component(&friendly))
}

/// Compact display token for a resolved subsearch result: `@prefix:value`.
/// Whitespace runs become `-` so the token stays one contiguous, readable
/// word (`@notes:grocery-list`, not `@notes:grocery%20list`); remaining
/// reserved characters are escaped.
pub(crate) fn compact_subsearch_token(prefix: &str, value: &str) -> String {
    let compact = crate::spine::text_preview::single_line_truncate(value, 40);
    let friendly = compact.split_whitespace().collect::<Vec<_>>().join("-");
    format!("@{}:{}", prefix, escape_ref_component(&friendly))
}

/// Preserve every selected owner's identity when two context attachments
/// have the same friendly label or file basename.
///
/// Re-selecting the exact source reuses its existing token; a distinct owner
/// receives the next deterministic `-2` / `-3` suffix without an artificial
/// collision ceiling that could fall back to overwriting an existing alias.
pub(crate) fn unique_context_attachment_token(
    base: &str,
    part: &AiContextPart,
    aliases: &std::collections::HashMap<String, AiContextPart>,
) -> String {
    let belongs_to_selected_owner =
        |existing: &AiContextPart| existing.has_same_attachment_owner(part);

    match aliases.get(base) {
        None => return base.to_string(),
        Some(existing) if belongs_to_selected_owner(existing) => return base.to_string(),
        Some(_) => {}
    }

    for suffix in 2usize.. {
        let candidate = format!("{base}-{suffix}");
        match aliases.get(&candidate) {
            None => return candidate,
            Some(existing) if belongs_to_selected_owner(existing) => return candidate,
            Some(_) => {}
        }
    }

    unreachable!("an unbounded unique attachment suffix always resolves")
}

fn resolve_action(
    segment_index: usize,
    segment_byte_range: Range<usize>,
    token: &str,
    resolution_id: String,
    resolution_label: String,
    resolution_source: &'static str,
) -> SpineListAction {
    SpineListAction::ResolveSegment {
        segment_index,
        segment_byte_range,
        replacement: token.to_string().into(),
        resolution_id: resolution_id.into(),
        resolution_label: resolution_label.into(),
        resolution_source: resolution_source.into(),
        trailing_space: true,
    }
}

fn text_block(label: String, source: String, text: String) -> AiContextPart {
    AiContextPart::TextBlock {
        label,
        source,
        text,
        mime_type: None,
    }
}

/// Map a selected rich subsearch result to its attach outcome.
/// Returns `None` only for result/source combinations that are not
/// attachable rows (section headers, hint rows, mismatched results); the
/// caller must still consume Enter so the launcher default never executes
/// a row while a `@source:` segment is active.
pub(crate) fn attach_outcome_for_result(
    source: ContextSubsearchSource,
    result: &SearchResult,
    segment_index: usize,
    segment_byte_range: Range<usize>,
) -> Option<SpineAttachOutcome> {
    match (source, result) {
        // Project rows are files too: a `@project:` pick resolves into the
        // same compact `@file:basename` token + "file" resolution source so
        // alias dedup and the prompt plan treat it identically.
        (
            ContextSubsearchSource::File | ContextSubsearchSource::Project,
            SearchResult::File(file_match),
        ) => {
            // Alias registration owned by the ResolveSegment apply arm
            // ("file" source) for parity with the file-search portal.
            let token = spine_file_mention_token(&file_match.file.path);
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("file/{}", file_match.file.path),
                    file_match.file.name.clone(),
                    "file",
                ),
                alias: None,
            })
        }
        (ContextSubsearchSource::Clipboard, SearchResult::ClipboardHistory(clip_match)) => {
            let token = format!("@clipboard:{}", escape_ref_component(&clip_match.entry.id));
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("clipboard/{}", clip_match.entry.id),
                    clip_match.title.clone(),
                    "clipboard",
                ),
                alias: None,
            })
        }
        (ContextSubsearchSource::Notes, SearchResult::Note(note_match)) => {
            let hit = &note_match.hit;
            let token = compact_subsearch_token("notes", &hit.title);
            let content = crate::notes::get_note(hit.id)
                .ok()
                .flatten()
                .map(|note| note.content)
                .unwrap_or_default();
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("notes/{}", hit.id),
                    hit.title.clone(),
                    "notes",
                ),
                alias: Some((
                    token,
                    text_block(
                        hit.title.clone(),
                        format!("spine:notes:{}", hit.id),
                        content,
                    ),
                )),
            })
        }
        (ContextSubsearchSource::BrowserHistory, SearchResult::BrowserHistory(history_match)) => {
            let hit = &history_match.hit;
            let label = if hit.title.trim().is_empty() {
                hit.domain.clone()
            } else {
                hit.title.clone()
            };
            let token = compact_subsearch_token("browser-history", &label);
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("browser-history/{}", hit.stable_key),
                    label.clone(),
                    "browser-history",
                ),
                alias: Some((
                    token,
                    text_block(
                        label,
                        format!("spine:browser-history:{}", hit.stable_key),
                        format!("{}\n{}", hit.title, hit.url),
                    ),
                )),
            })
        }
        (ContextSubsearchSource::Dictation, SearchResult::DictationHistory(dictation_match)) => {
            let token = compact_subsearch_token("dictation", &dictation_match.preview);
            let transcript = crate::dictation::get_history_entry(&dictation_match.id)
                .map(|entry| entry.transcript)
                .unwrap_or_else(|| dictation_match.preview.clone());
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("dictation/{}", dictation_match.id),
                    dictation_match.preview.clone(),
                    "dictation",
                ),
                alias: Some((
                    token,
                    text_block(
                        dictation_match.preview.clone(),
                        format!("spine:dictation:{}", dictation_match.id),
                        transcript,
                    ),
                )),
            })
        }
        (ContextSubsearchSource::History, SearchResult::AgentChatHistory(history_match)) => {
            let entry = &history_match.entry;
            let title = entry.title_display().to_string();
            let token = compact_subsearch_token("history", &title);
            let text = format!(
                "Conversation: {title}\nFirst message: {}\nLast reply: {}",
                entry.first_message.trim(),
                entry.preview.trim(),
            );
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("history/{}", entry.session_id),
                    title.clone(),
                    "history",
                ),
                alias: Some((
                    token,
                    text_block(title, format!("spine:history:{}", entry.session_id), text),
                )),
            })
        }
        (ContextSubsearchSource::Scripts, SearchResult::Script(script_match)) => {
            let script = &script_match.script;
            let path = script.path.to_string_lossy().to_string();
            let token = compact_subsearch_token("scripts", &script_match.filename);
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("scripts/{path}"),
                    script.name.clone(),
                    "scripts",
                ),
                alias: Some((
                    token,
                    AiContextPart::FilePath {
                        path,
                        label: script_match.filename.clone(),
                    },
                )),
            })
        }
        (ContextSubsearchSource::Scriptlets, SearchResult::Scriptlet(scriptlet_match)) => {
            let scriptlet = &scriptlet_match.scriptlet;
            let token = compact_subsearch_token("scriptlets", &scriptlet.name);
            let source_id = scriptlet
                .file_path
                .clone()
                .unwrap_or_else(|| scriptlet.name.clone());
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("scriptlets/{source_id}"),
                    scriptlet.name.clone(),
                    "scriptlets",
                ),
                alias: Some((
                    token,
                    text_block(
                        scriptlet.name.clone(),
                        format!("spine:scriptlets:{source_id}"),
                        scriptlet.code.clone(),
                    ),
                )),
            })
        }
        (ContextSubsearchSource::Skills, SearchResult::Skill(skill_match)) => {
            let skill = &skill_match.skill;
            let path = skill.path.to_string_lossy().to_string();
            let token = compact_subsearch_token("skills", &skill.title);
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("skills/{path}"),
                    skill.title.clone(),
                    "skills",
                ),
                alias: Some((
                    token,
                    AiContextPart::FilePath {
                        path,
                        label: skill.title.clone(),
                    },
                )),
            })
        }
        (
            ContextSubsearchSource::Calendar | ContextSubsearchSource::Notifications,
            SearchResult::SpineProjection(row),
        ) if row.is_selectable
            && matches!(&row.action, SpineListAction::AttachContextResult { source: owner } if owner.as_ref() == source.prefix()) =>
        {
            // Provider-JSON rows carry their content in title/subtitle.
            let prefix = source.prefix();
            let title = row.title.to_string();
            let token = compact_subsearch_token(prefix, &title);
            let text = match row.subtitle.as_ref() {
                Some(subtitle) => format!("{title}\n{subtitle}"),
                None => title.clone(),
            };
            Some(SpineAttachOutcome {
                action: resolve_action(
                    segment_index,
                    segment_byte_range,
                    &token,
                    format!("{prefix}/{}", row.id),
                    title.clone(),
                    match source {
                        ContextSubsearchSource::Calendar => "calendar",
                        _ => "notifications",
                    },
                ),
                alias: Some((
                    token,
                    text_block(title, format!("spine:{prefix}:{}", row.id), text),
                )),
            })
        }
        // SpineProjection utility rows (Open full File Search, hints, empty
        // guards) keep their own action — let the normal row accept run it.
        (_, SearchResult::SpineProjection(row)) => {
            if row.is_selectable
                && !matches!(
                    &row.action,
                    SpineListAction::Noop
                        | SpineListAction::AttachContextResult { .. }
                        | SpineListAction::AcceptMenuSyntaxTrigger { .. }
                        | SpineListAction::AcceptMenuSyntaxObject { .. }
                )
            {
                Some(SpineAttachOutcome {
                    action: row.action.clone(),
                    alias: None,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// A composer-facing rich subsearch row: the displayable spine row plus the
/// alias (token → content part) the composer registers when the row is
/// accepted.
pub(crate) struct ComposerSubsearchRow {
    pub row: crate::spine::SpineListRow,
    pub alias: Option<(String, AiContextPart)>,
}

/// A composer-facing rich subsearch section for one `@source:` query.
pub(crate) struct ComposerSubsearchSection {
    pub source_id: &'static str,
    pub title: String,
    pub icon: &'static str,
    pub rows: Vec<ComposerSubsearchRow>,
}

/// Shared resolver for `@source:` subsearch in the Agent Chat composer.
///
/// Covers every source whose backing store is directly queryable without
/// `ScriptListApp` state (notes, browser history, dictation, chat history,
/// calendar, notifications). File and Clipboard keep the composer's existing
/// dedicated paths; Scripts/Scriptlets/Skills need the launcher's loaded
/// registries and return `None`.
pub(crate) fn composer_subsearch_section(
    source: ContextSubsearchSource,
    query: &str,
    segment_index: usize,
    segment_byte_range: Range<usize>,
) -> Option<ComposerSubsearchSection> {
    let limit = crate::spine::catalog_subsearch::SUBSEARCH_RENDER_LIMIT;
    let (source_id, icon, empty_title, match_title): (&'static str, &'static str, &str, &str) =
        match source {
            ContextSubsearchSource::Notes => ("notes", "notebook-text", "Recent Notes", "Notes"),
            ContextSubsearchSource::BrowserHistory => (
                "browser-history",
                "globe",
                "Recent Browser History",
                "Browser history",
            ),
            ContextSubsearchSource::Dictation => {
                ("dictation", "mic", "Recent Dictation", "Dictation")
            }
            ContextSubsearchSource::History => (
                "history",
                "message-square",
                "Recent Agent Chat",
                "Chat history",
            ),
            ContextSubsearchSource::Calendar => {
                ("calendar", "calendar", "Calendar Events", "Calendar events")
            }
            ContextSubsearchSource::Notifications => {
                ("notifications", "bell", "Notifications", "Notifications")
            }
            ContextSubsearchSource::File
            | ContextSubsearchSource::Project
            | ContextSubsearchSource::Clipboard
            | ContextSubsearchSource::Scripts
            | ContextSubsearchSource::Scriptlets
            | ContextSubsearchSource::Skills => return None,
        };

    let results = composer_subsearch_results(source, query, limit);
    Some(composer_subsearch_section_from_results(
        source,
        query,
        segment_index,
        segment_byte_range,
        (source_id, icon, empty_title, match_title),
        results,
    ))
}

/// Resolve launcher-owned script, scriptlet, and skill catalogs without
/// performing discovery or reading files on the composer's typing path.
///
/// The launcher publishes an immutable in-memory snapshot after startup and
/// every refresh. The existing store-backed resolver deliberately continues
/// rejecting these sources when no launcher catalogs were supplied.
pub(crate) fn composer_catalog_subsearch_section(
    source: ContextSubsearchSource,
    query: &str,
    segment_index: usize,
    segment_byte_range: Range<usize>,
    scripts: &[std::sync::Arc<crate::scripts::Script>],
    scriptlets: &[std::sync::Arc<crate::scripts::Scriptlet>],
    skills: &[std::sync::Arc<crate::plugins::PluginSkill>],
) -> Option<ComposerSubsearchSection> {
    let (source_id, icon, title) = match source {
        ContextSubsearchSource::Scripts => ("scripts", "file-code", "Scripts"),
        ContextSubsearchSource::Scriptlets => ("scriptlets", "scroll-text", "Scriptlets"),
        ContextSubsearchSource::Skills => ("skills", "workflow", "Skills"),
        _ => return None,
    };
    let results = crate::spine::catalog_subsearch::rank_context_catalog_results(
        source, query, scripts, scriptlets, skills,
    );
    Some(composer_subsearch_section_from_results(
        source,
        query,
        segment_index,
        segment_byte_range,
        (source_id, icon, title, title),
        results,
    ))
}

fn composer_subsearch_section_from_results(
    source: ContextSubsearchSource,
    query: &str,
    segment_index: usize,
    segment_byte_range: Range<usize>,
    descriptor: (&'static str, &'static str, &str, &str),
    results: Vec<SearchResult>,
) -> ComposerSubsearchSection {
    let (source_id, icon, empty_title, match_title) = descriptor;
    let trimmed = query.trim();
    let rows = results
        .into_iter()
        .enumerate()
        .filter_map(|(index, result)| {
            let outcome = attach_outcome_for_result(
                source,
                &result,
                segment_index,
                segment_byte_range.clone(),
            )?;
            let (title, subtitle) = composer_result_display(&result)?;
            Some(ComposerSubsearchRow {
                row: crate::spine::SpineListRow {
                    id: format!("agent_chat-spine:{source_id}:{index}").into(),
                    kind: crate::spine::SpineListRowKind::ContextResult {
                        context_type: source_id.into(),
                        result_id: index.to_string().into(),
                    },
                    title: title.into(),
                    subtitle: subtitle.map(Into::into),
                    meta: None,
                    icon: Some(icon.into()),
                    badges: Vec::new(),
                    score: result.score(),
                    is_selectable: true,
                    action_label: None,
                    action: outcome.action,
                },
                alias: outcome.alias,
            })
        })
        .collect();

    let title = if trimmed.is_empty() {
        empty_title.to_string()
    } else {
        format!("{match_title} matching \u{201c}{trimmed}\u{201d}")
    };
    ComposerSubsearchSection {
        source_id,
        title,
        icon,
        rows,
    }
}

/// Recover exactly the accepted row's attachment, even when two plugins use
/// the same visible label and therefore produce the same friendly token.
pub(crate) fn composer_subsearch_alias_for_resolution(
    section: ComposerSubsearchSection,
    token: &str,
    resolution_id: &str,
) -> Option<AiContextPart> {
    section.rows.into_iter().find_map(|row| {
        let SpineListAction::ResolveSegment {
            replacement,
            resolution_id: row_resolution_id,
            ..
        } = &row.row.action
        else {
            return None;
        };
        if replacement.as_ref() != token || row_resolution_id.as_ref() != resolution_id {
            return None;
        }
        match row.alias {
            Some((alias_token, part)) if alias_token == token => Some(part),
            _ => None,
        }
    })
}

fn composer_subsearch_results(
    source: ContextSubsearchSource,
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    match source {
        ContextSubsearchSource::Notes => {
            let options = crate::notes::RootNotesSectionOptions {
                enabled: true,
                max_results: limit,
                min_query_chars: 0,
                ..Default::default()
            };
            crate::notes::search_root_notes_meta_direct(query, options)
                .into_iter()
                .map(|hit| {
                    let subtitle = format!("{} chars", hit.char_count);
                    SearchResult::Note(crate::scripts::NoteMatch {
                        title: hit.title.clone(),
                        subtitle,
                        score: 0,
                        hit,
                    })
                })
                .collect()
        }
        ContextSubsearchSource::BrowserHistory => {
            let options = crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                max_results: limit,
                min_query_chars: 0,
                ..Default::default()
            };
            crate::browser_history::search_root_browser_history_meta_direct(query, options)
                .into_iter()
                .map(|hit| {
                    SearchResult::BrowserHistory(crate::scripts::BrowserHistoryMatch {
                        subtitle: hit.url.clone(),
                        score: 0,
                        hit,
                    })
                })
                .collect()
        }
        ContextSubsearchSource::Dictation => {
            let options = crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: limit,
                min_query_chars: 0,
                ..Default::default()
            };
            crate::dictation::search_root_dictation_history_direct(query, options)
                .into_iter()
                .map(|hit| {
                    SearchResult::DictationHistory(crate::scripts::DictationHistoryMatch {
                        id: hit.id.clone(),
                        preview: hit.preview.clone(),
                        target: hit.target.clone(),
                        timestamp: hit.timestamp.clone(),
                        audio_duration_ms: hit.audio_duration_ms,
                        subtitle: hit.target.clone(),
                        score: 0,
                        matched_field: hit.matched_field,
                        evidence: hit.evidence.clone(),
                    })
                })
                .collect()
        }
        ContextSubsearchSource::History => {
            crate::ai::agent_chat::ui::history::search_history_direct(query, limit)
                .into_iter()
                .map(|hit| {
                    let subtitle = hit.entry.title_display().to_string();
                    SearchResult::AgentChatHistory(crate::scripts::AgentChatHistoryMatch {
                        entry: hit.entry,
                        score: 0,
                        matched_field: hit.matched_field,
                        subtitle,
                        evidence: hit.evidence,
                    })
                })
                .collect()
        }
        ContextSubsearchSource::Calendar | ContextSubsearchSource::Notifications => {
            let (kind, prefix, icon) = match source {
                ContextSubsearchSource::Calendar => (
                    crate::mcp_resources::ProviderJsonResourceKind::Calendar,
                    "calendar",
                    "calendar",
                ),
                _ => (
                    crate::mcp_resources::ProviderJsonResourceKind::Notifications,
                    "notifications",
                    "bell",
                ),
            };
            let query_lower = query.trim().to_lowercase();
            crate::mcp_resources::read_provider_json_items(kind)
                .into_iter()
                .filter(|item| {
                    query_lower.is_empty()
                        || item.title.to_lowercase().contains(&query_lower)
                        || item
                            .subtitle
                            .as_deref()
                            .is_some_and(|s| s.to_lowercase().contains(&query_lower))
                })
                .take(limit)
                .enumerate()
                .map(|(rank, item)| {
                    SearchResult::SpineProjection(crate::spine::SpineListRow {
                        id: format!("spine:provider-json:{prefix}:{rank}").into(),
                        kind: crate::spine::SpineListRowKind::ContextResult {
                            context_type: prefix.into(),
                            result_id: rank.to_string().into(),
                        },
                        title: item.title.into(),
                        subtitle: item.subtitle.map(Into::into),
                        meta: None,
                        icon: Some(icon.into()),
                        badges: Vec::new(),
                        score: 0,
                        is_selectable: true,
                        action_label: None,
                        action: SpineListAction::AttachContextResult {
                            source: prefix.into(),
                        },
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Display title/subtitle for a composer subsearch row.
fn composer_result_display(result: &SearchResult) -> Option<(String, Option<String>)> {
    match result {
        SearchResult::Script(script_match) => Some((
            script_match.script.name.clone(),
            script_match
                .script
                .description
                .clone()
                .or_else(|| Some(script_match.filename.clone())),
        )),
        SearchResult::Scriptlet(scriptlet_match) => Some((
            scriptlet_match.scriptlet.name.clone(),
            scriptlet_match
                .scriptlet
                .description
                .clone()
                .or_else(|| scriptlet_match.display_file_path.clone()),
        )),
        SearchResult::Skill(skill_match) => Some((
            skill_match.skill.title.clone(),
            (!skill_match.skill.description.is_empty())
                .then(|| skill_match.skill.description.clone())
                .or_else(|| Some(skill_match.skill.plugin_title.clone())),
        )),
        SearchResult::Note(note_match) => Some((
            note_match.hit.title.clone(),
            Some(note_match.subtitle.clone()),
        )),
        SearchResult::BrowserHistory(history_match) => {
            let hit = &history_match.hit;
            let title = if hit.title.trim().is_empty() {
                hit.domain.clone()
            } else {
                hit.title.clone()
            };
            Some((title, Some(hit.url.clone())))
        }
        SearchResult::DictationHistory(dictation_match) => Some((
            dictation_match.preview.clone(),
            Some(dictation_match.target.clone()),
        )),
        SearchResult::AgentChatHistory(history_match) => Some((
            history_match.entry.title_display().to_string(),
            Some(history_match.entry.preview_display().to_string()),
        )),
        SearchResult::SpineProjection(row) => Some((
            row.title.to_string(),
            row.subtitle.as_ref().map(|s| s.to_string()),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_token_is_friendly_and_truncates() {
        let token = compact_subsearch_token("notes", "grocery list for the week");
        assert_eq!(token, "@notes:grocery-list-for-the-week");
        let long = "x".repeat(120);
        let token = compact_subsearch_token("notes", &long);
        assert!(token.chars().count() <= "@notes:".len() + 40 + 1);
    }

    #[test]
    fn duplicate_file_attachment_names_keep_distinct_owners_and_reuse_same_owner() {
        let first = AiContextPart::FilePath {
            path: "/first/plugin/SKILL.md".to_string(),
            label: "Deploy".to_string(),
        };
        let second = AiContextPart::FilePath {
            path: "/second/plugin/SKILL.md".to_string(),
            label: "Deploy".to_string(),
        };
        let mut aliases = std::collections::HashMap::new();

        let first_token = unique_context_attachment_token("@skills:Deploy", &first, &aliases);
        assert_eq!(first_token, "@skills:Deploy");
        aliases.insert(first_token.clone(), first.clone());

        let second_token = unique_context_attachment_token("@skills:Deploy", &second, &aliases);
        assert_eq!(second_token, "@skills:Deploy-2");
        aliases.insert(second_token.clone(), second.clone());

        assert_eq!(
            unique_context_attachment_token("@skills:Deploy", &first, &aliases),
            first_token
        );
        assert_eq!(
            unique_context_attachment_token("@skills:Deploy", &second, &aliases),
            second_token
        );
    }

    #[test]
    fn duplicate_scriptlet_attachment_names_preserve_distinct_source_identity() {
        let first = AiContextPart::TextBlock {
            label: "Deploy".to_string(),
            source: "spine:scriptlets:/first/deploy.md".to_string(),
            text: "first owner's command".to_string(),
            mime_type: None,
        };
        let second = AiContextPart::TextBlock {
            label: "Deploy".to_string(),
            source: "spine:scriptlets:/second/deploy.md".to_string(),
            text: "second owner's command".to_string(),
            mime_type: None,
        };
        let aliases = std::collections::HashMap::from([("@scriptlets:Deploy".to_string(), first)]);

        assert_eq!(
            unique_context_attachment_token("@scriptlets:Deploy", &second, &aliases),
            "@scriptlets:Deploy-2"
        );
    }

    #[test]
    fn attachment_token_collisions_never_fall_back_to_an_existing_owner() {
        let mut aliases = std::collections::HashMap::new();
        for index in 1..=128 {
            let token = if index == 1 {
                "@file:README.md".to_string()
            } else {
                format!("@file:README.md-{index}")
            };
            aliases.insert(
                token,
                AiContextPart::FilePath {
                    path: format!("/plugin-{index}/README.md"),
                    label: "README.md".to_string(),
                },
            );
        }
        let next = AiContextPart::FilePath {
            path: "/plugin-129/README.md".to_string(),
            label: "README.md".to_string(),
        };

        assert_eq!(
            unique_context_attachment_token("@file:README.md", &next, &aliases),
            "@file:README.md-129"
        );
    }

    #[test]
    fn scriptlet_attach_resolves_to_text_block_with_code() {
        let scriptlet = std::sync::Arc::new(crate::scripts::Scriptlet {
            icon: None,
            name: "Open GitHub".to_string(),
            description: None,
            code: "open https://github.com".to_string(),
            tool: "bash".to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: "main".to_string(),
            plugin_title: None,
            file_path: Some("/tmp/url.md#open-github".to_string()),
            command: Some("open-github".to_string()),
            alias: None,
        });
        let result = SearchResult::Scriptlet(crate::scripts::ScriptletMatch {
            scriptlet,
            score: 0,
            display_file_path: None,
            match_indices: Default::default(),
            match_evidence: None,
        });
        let outcome =
            attach_outcome_for_result(ContextSubsearchSource::Scriptlets, &result, 0, 0..12)
                .expect("scriptlet rows must attach");
        let (token, part) = outcome.alias.expect("scriptlet alias");
        assert!(token.starts_with("@scriptlets:"));
        match part {
            AiContextPart::TextBlock { text, .. } => {
                assert_eq!(text, "open https://github.com");
            }
            other => panic!("expected TextBlock, got {other:?}"),
        }
        match outcome.action {
            SpineListAction::ResolveSegment {
                resolution_source, ..
            } => assert_eq!(resolution_source.as_ref(), "scriptlets"),
            other => panic!("expected ResolveSegment, got {other:?}"),
        }
    }

    #[test]
    fn browser_history_attach_includes_url_in_text() {
        let result = SearchResult::BrowserHistory(crate::scripts::BrowserHistoryMatch {
            hit: crate::browser_history::RootBrowserHistorySearchHit {
                stable_key: "abc".to_string(),
                provider_label: "Chrome".to_string(),
                profile_label: "Default".to_string(),
                title: "Rust Book".to_string(),
                url: "https://doc.rust-lang.org/book/".to_string(),
                domain: "doc.rust-lang.org".to_string(),
                last_visit_unix_ms: 0,
                visit_count: 3,
            },
            subtitle: "https://doc.rust-lang.org/book/".to_string(),
            score: 0,
        });
        let outcome =
            attach_outcome_for_result(ContextSubsearchSource::BrowserHistory, &result, 0, 0..20)
                .expect("browser history rows must attach");
        let (token, part) = outcome.alias.expect("browser alias");
        assert!(token.starts_with("@browser-history:"));
        match part {
            AiContextPart::TextBlock { text, .. } => {
                assert!(text.contains("https://doc.rust-lang.org/book/"));
            }
            other => panic!("expected TextBlock, got {other:?}"),
        }
    }

    #[test]
    fn provider_json_row_attaches_title_and_subtitle() {
        use crate::spine::list::ss;
        let row = crate::spine::SpineListRow {
            id: ss("spine:provider-json:calendar:0"),
            kind: crate::spine::SpineListRowKind::ContextResult {
                context_type: ss("calendar"),
                result_id: ss("0"),
            },
            title: ss("Standup"),
            subtitle: Some(ss("9:30 AM · Daily")),
            meta: None,
            icon: Some(ss("calendar")),
            badges: vec![],
            score: 0,
            is_selectable: true,
            action_label: Some(ss("Attach")),
            action: SpineListAction::AttachContextResult {
                source: "calendar".into(),
            },
        };
        let result = SearchResult::SpineProjection(row);
        assert!(
            attach_outcome_for_result(ContextSubsearchSource::Notifications, &result, 0, 0..10)
                .is_none(),
            "a different source cannot own this attachment"
        );
        let outcome =
            attach_outcome_for_result(ContextSubsearchSource::Calendar, &result, 0, 0..10)
                .expect("calendar rows must attach");
        let (token, part) = outcome.alias.expect("calendar alias");
        assert!(token.starts_with("@calendar:Standup"));
        match part {
            AiContextPart::TextBlock { text, .. } => {
                assert_eq!(text, "Standup\n9:30 AM · Daily");
            }
            other => panic!("expected TextBlock, got {other:?}"),
        }
        // The owning subsearch resolves the typed attachment intent.
        assert!(matches!(
            outcome.action,
            SpineListAction::ResolveSegment { .. }
        ));
        let SearchResult::SpineProjection(mut row) = result else {
            unreachable!()
        };
        row.action = SpineListAction::Noop;
        let inert = SearchResult::SpineProjection(row.clone());
        assert!(
            attach_outcome_for_result(ContextSubsearchSource::Calendar, &inert, 0, 0..10).is_none(),
            "a selectable explanation is not an attachment"
        );
        row.action = SpineListAction::AttachContextResult {
            source: "unknown-provider".into(),
        };
        assert!(attach_outcome_for_result(
            ContextSubsearchSource::Calendar,
            &SearchResult::SpineProjection(row),
            0,
            0..10
        )
        .is_none());
    }

    /// Composer parity end-to-end: a provider-backed source queried through
    /// `composer_subsearch_section` yields displayable rows whose actions
    /// resolve the segment and whose aliases carry the real content.
    #[test]
    fn composer_section_builds_rows_and_aliases_from_calendar_slot() {
        crate::mcp_resources::publish_calendar_json(
            r#"{"items":[
                {"title":"Standup","subtitle":"9:30 AM Daily"},
                {"title":"Design Review","subtitle":"2:00 PM"}
            ]}"#,
        );

        let section =
            composer_subsearch_section(ContextSubsearchSource::Calendar, "design", 3, 5..14)
                .expect("calendar is a composer-shared source");
        crate::mcp_resources::clear_provider_json_slots();

        assert_eq!(section.source_id, "calendar");
        assert_eq!(section.rows.len(), 1, "query should filter to one event");
        let row = &section.rows[0];
        assert_eq!(row.row.title.as_ref(), "Design Review");
        assert!(row.row.is_selectable);
        match &row.row.action {
            SpineListAction::ResolveSegment {
                segment_index,
                segment_byte_range,
                resolution_source,
                replacement,
                ..
            } => {
                assert_eq!(*segment_index, 3);
                assert_eq!(segment_byte_range.clone(), 5..14);
                assert_eq!(resolution_source.as_ref(), "calendar");
                assert!(replacement.starts_with("@calendar:"));
            }
            other => panic!("expected ResolveSegment, got {other:?}"),
        }
        let (token, part) = row.alias.clone().expect("calendar alias");
        assert!(token.starts_with("@calendar:Design-Review"));
        match part {
            AiContextPart::TextBlock { text, .. } => {
                assert_eq!(text, "Design Review\n2:00 PM");
            }
            other => panic!("expected TextBlock, got {other:?}"),
        }
    }

    /// Sources that need launcher state stay out of the composer resolver.
    #[test]
    fn composer_section_rejects_launcher_only_sources() {
        for source in [
            ContextSubsearchSource::File,
            ContextSubsearchSource::Clipboard,
            ContextSubsearchSource::Scripts,
            ContextSubsearchSource::Scriptlets,
            ContextSubsearchSource::Skills,
        ] {
            assert!(composer_subsearch_section(source, "", 0, 0..0).is_none());
        }
    }
}
