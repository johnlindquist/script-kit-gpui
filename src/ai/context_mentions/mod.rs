//! Parsing and resolution of @-mention context references in AI prompts.

use crate::ai::message_parts::AiContextPart;
use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedContextMentions {
    pub(crate) cleaned_content: String,
    pub(crate) parts: Vec<AiContextPart>,
}

impl ParsedContextMentions {
    pub(crate) fn has_parts(&self) -> bool {
        !self.parts.is_empty()
    }
}

/// A single inline `@mention` token found in text, with its character range
/// and the resolved `AiContextPart`.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineContextMention {
    /// Character-level range of the token in the source text.
    pub range: std::ops::Range<usize>,
    /// The raw token text (e.g. `@browser`, `@file:/tmp/demo.rs`).
    pub token: String,
    /// The canonical token for this mention, used for ownership tracking and
    /// highlight matching. Aliases resolve to the primary mention token.
    pub canonical_token: String,
    /// The resolved context part for this mention.
    pub part: AiContextPart,
}

/// A raw inline `@token` span found syntactically in text.
///
/// Unlike [`InlineContextMention`], this does not require successful
/// resolution to an `AiContextPart`. It exists so editor hosts can preview
/// and reopen typed portal tokens from persisted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlineTokenSpan {
    pub range: Range<usize>,
    pub token: String,
}

pub(crate) fn parse_context_mentions(raw_content: &str) -> ParsedContextMentions {
    let mut cleaned_lines = Vec::new();
    let mut parts = Vec::new();

    for line in raw_content.lines() {
        if let Some(part) = parse_context_mention_line(line) {
            let safe_directive = crate::logging::log_private_user_value(line.trim());
            let safe_label = crate::logging::log_private_user_value(part.label());
            tracing::info!(
                target: "ai",
                directive_bytes = safe_directive.raw_bytes,
                directive_sha256 = %safe_directive.sha256,
                label_bytes = safe_label.raw_bytes,
                label_sha256 = %safe_label.sha256,
                "context_mention_parsed"
            );
            parts.push(part);
        } else {
            cleaned_lines.push(line);
        }
    }

    let cleaned_content = cleaned_lines
        .join("\n")
        .trim_matches(|c| c == '\n' || c == '\r')
        .to_string();

    tracing::info!(
        target: "ai",
        raw_len = raw_content.len(),
        cleaned_len = cleaned_content.len(),
        parts_count = parts.len(),
        "context_mentions_parse_complete"
    );

    ParsedContextMentions {
        cleaned_content,
        parts,
    }
}

/// Trim trailing punctuation that is not part of the mention token itself.
/// E.g. `@browser,` → `@browser`, `@git-diff.` → `@git-diff`.
fn trim_inline_token_trailing_punctuation(token: &str) -> &str {
    token.trim_end_matches([',', '.', ';', ':', ')', ']', '}'])
}

/// Scan a single inline token starting at `start` (which must be `@`).
/// Handles quoted `@word:"..."` / `@word:'...'` tokens (any prefix, not just
/// `@file:`) that may contain spaces and escape sequences.
/// Returns `(raw_token, next_index)`.
fn scan_inline_token(chars: &[char], start: usize) -> (String, usize) {
    // Look for `@word:` prefix followed by a quote — generalized from @file: only.
    // Scan ahead for a colon within the first ~10 chars after @.
    let mut colon_pos = None;
    for (offset, &ch) in chars
        .get(start + 1..chars.len().min(start + 12))
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
        if ch == ':' {
            colon_pos = Some(start + 1 + offset);
            break;
        }
        if ch.is_whitespace() || ch == '@' {
            break;
        }
    }

    if let Some(cp) = colon_pos {
        // Check if char after colon is a quote
        if let Some(&quote) = chars.get(cp + 1) {
            if quote == '"' || quote == '\'' {
                let mut i = cp + 2;
                let mut escaped = false;
                while i < chars.len() {
                    let ch = chars[i];
                    if escaped {
                        escaped = false;
                        i += 1;
                        continue;
                    }
                    if ch == '\\' {
                        escaped = true;
                        i += 1;
                        continue;
                    }
                    if ch == quote {
                        i += 1; // consume closing quote
                        break;
                    }
                    i += 1;
                }
                return (chars[start..i].iter().collect(), i);
            }
        }
    }

    // Default: whitespace-delimited token
    let mut i = start + 1;
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    (chars[start..i].iter().collect(), i)
}

/// Strip outer quotes and unescape `\\`, `\"`, `\'` inside a quoted path.
fn unescape_quoted_path(path: &str) -> String {
    let bytes = path.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        let inner = &path[1..path.len() - 1];
        inner
            .replace("\\\\", "\0ESCAPED_BACKSLASH\0")
            .replace("\\\"", "\"")
            .replace("\\'", "'")
            .replace("\0ESCAPED_BACKSLASH\0", "\\")
    } else {
        path.to_string()
    }
}

/// Format a file path as a canonical inline `@file:` token, quoting paths
/// that contain whitespace.
pub(crate) fn format_inline_file_token(path: &str) -> String {
    if path.chars().any(char::is_whitespace) {
        format!(
            "@file:\"{}\"",
            path.replace('\\', "\\\\").replace('"', "\\\"")
        )
    } else {
        format!("@file:{path}")
    }
}

/// All path-shaped mentions use the canonical `@file:` prefix — the
/// extension stays in the inline name instead of being projected into a
/// short type prefix.
pub(crate) fn typed_mention_prefix(_path: &str) -> &'static str {
    "file"
}

/// Map a file path to a "dir" prefix if it looks like a directory.
pub(crate) fn typed_mention_prefix_for_dir() -> &'static str {
    "dir"
}

/// Map a focused-target kind to a typed mention prefix.
pub(crate) fn typed_mention_prefix_for_target_kind(kind: &str) -> &'static str {
    match kind {
        "script" => "script",
        "scriptlet" => "scriptlet",
        "note" => "note",
        "browser_history_entry" => "browser-history",
        "builtin" | "menu_command" => "cmd",
        "search_query" => "search",
        "input" => "input",
        "clipboard_entry" => "clipboard",
        "window" => "window",
        "app" => "app",
        "process" => "process",
        "agent" => "agent",
        "skill" => "skill",
        "fallback" => "suggestion",
        _ => "cmd",
    }
}

/// The inline display name for a path-shaped mention: the full basename
/// including its extension so `@file:power-syntax.ts` stays readable instead
/// of collapsing to `@ts:power-s…`.
pub(crate) fn typed_mention_display_name(path: &str) -> String {
    let p = Path::new(path);
    p.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "file".to_string())
}

/// Format a typed mention token: `@type:name` or `@type:"name with spaces"`.
pub(crate) fn format_typed_mention_token(prefix: &str, name: &str) -> String {
    if name.chars().any(|ch| ch.is_whitespace()) {
        format!(
            "@{prefix}:\"{}\"",
            name.replace('\\', "\\\\").replace('"', "\\\"")
        )
    } else {
        format!("@{prefix}:{name}")
    }
}

fn strip_typed_mention_label_prefix(label: &str) -> &str {
    let trimmed = label.trim();
    let Some((prefix, rest)) = trimmed.split_once(':') else {
        return trimmed;
    };

    let normalized = prefix.trim().to_ascii_lowercase();
    let is_known_prefix = matches!(
        normalized.as_str(),
        "file"
            | "folder"
            | "search"
            | "input"
            | "clipboard"
            | "command"
            | "window"
            | "app"
            | "process"
            | "menu command"
            | "agent"
            | "suggestion"
            | "selection"
            | "context"
            | "ambient"
    );

    if is_known_prefix && !rest.trim().is_empty() {
        rest.trim()
    } else {
        trimmed
    }
}

fn typed_mention_label_name(label: &str) -> String {
    strip_typed_mention_label_prefix(label).to_string()
}

/// Format a typed mention token using the label-truncation rules used by
/// inline context attachments.
pub(crate) fn format_typed_label_mention_token(prefix: &str, label: &str) -> String {
    let name = typed_mention_label_name(label);
    format_typed_mention_token(prefix, &name)
}

/// Parse a typed mention token like `@note:"Daily Standup"` or
/// `@file:/tmp/demo.rs` into `(prefix, value)`.
pub(crate) fn typed_mention_token_parts(token: &str) -> Option<(String, String)> {
    let trimmed = trim_inline_token_trailing_punctuation(token).trim();
    let body = trimmed.strip_prefix('@')?;
    let (prefix, raw_value) = body.split_once(':')?;
    let prefix = prefix.trim().to_ascii_lowercase();
    if prefix.is_empty() || raw_value.trim().is_empty() {
        return None;
    }

    Some((prefix, unescape_quoted_path(raw_value.trim())))
}

/// Collect all syntactic inline `@token` spans from `text`.
pub(crate) fn inline_token_spans(text: &str) -> Vec<InlineTokenSpan> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i += 1;
            continue;
        }

        let start = i;
        let (raw_token, next_i) = scan_inline_token(&chars, start);
        i = next_i;

        let trimmed = trim_inline_token_trailing_punctuation(&raw_token);
        if trimmed.len() <= 1 {
            continue;
        }

        out.push(InlineTokenSpan {
            range: start..(start + trimmed.chars().count()),
            token: trimmed.to_string(),
        });
    }
    out
}

/// Return the raw inline token whose range covers `cursor`.
pub(crate) fn inline_token_at_cursor(text: &str, cursor: usize) -> Option<InlineTokenSpan> {
    inline_token_spans(text)
        .into_iter()
        .find(|span| cursor > span.range.start && cursor <= span.range.end)
}

/// Return the next raw inline token range after `cursor`, wrapping to the first.
pub(crate) fn next_inline_token_range(text: &str, cursor: usize) -> Option<Range<usize>> {
    let spans = inline_token_spans(text);
    if spans.is_empty() {
        return None;
    }

    spans
        .iter()
        .find(|span| span.range.start > cursor)
        .map(|span| span.range.clone())
        .or_else(|| spans.first().map(|span| span.range.clone()))
}

/// Return the previous raw inline token range before `cursor`, wrapping to the last.
pub(crate) fn previous_inline_token_range(text: &str, cursor: usize) -> Option<Range<usize>> {
    let spans = inline_token_spans(text);
    if spans.is_empty() {
        return None;
    }

    spans
        .iter()
        .rev()
        .find(|span| span.range.end < cursor)
        .map(|span| span.range.clone())
        .or_else(|| spans.last().map(|span| span.range.clone()))
}

/// Scan `text` for inline `@mention` tokens and resolve each to an
/// `AiContextPart`. Supports built-in mentions (`@browser`, `@git-status`,
/// etc.), file mentions (`@file:/path`), and typed mentions (`@type:name`)
/// via the session alias registry.
pub fn parse_inline_context_mentions(text: &str) -> Vec<InlineContextMention> {
    parse_inline_context_mentions_with_aliases(text, &std::collections::HashMap::new())
}

/// Parse inline mentions with a session-local alias registry for typed
/// `@type:name` tokens. Aliases map short display tokens to full `AiContextPart`s.
pub fn parse_inline_context_mentions_with_aliases(
    text: &str,
    aliases: &std::collections::HashMap<String, AiContextPart>,
) -> Vec<InlineContextMention> {
    let mut out = Vec::new();
    for span in inline_token_spans(text) {
        // Resolution order: built-in → typed dictation entry → alias registry
        // → literal @file:/path. Alias lookup must precede literal file
        // parsing so compact selected-file tokens like `@file:demo.rs`
        // keep resolving to their full attached path.
        let (part, alias_backed) = if let Some(part) = resolve_builtin_mention_token(&span.token) {
            (part, false)
        } else if let Some(part) = parse_dictation_history_mention(&span.token) {
            (part, false)
        } else if let Some(part) = aliases.get(&span.token) {
            (part.clone(), true)
        } else if let Some(part) = parse_file_mention(&span.token) {
            (part, false)
        } else {
            continue;
        };

        // Alias keys retain exact selected-owner identity: both plugin skill
        // paths can end in SKILL.md, but @skills:Deploy and
        // @skills:Deploy-2 must remain independently owned. Built-in aliases
        // such as @context still normalize to their canonical @here token.
        let canonical_token = if alias_backed {
            span.token.to_string()
        } else {
            part_to_inline_token(&part).unwrap_or_else(|| span.token.to_string())
        };
        let safe_token = crate::logging::log_private_user_value(&span.token);
        let safe_canonical_token = crate::logging::log_private_user_value(&canonical_token);
        let safe_source = crate::logging::log_private_user_value(part.source());
        let safe_label = crate::logging::log_private_user_value(part.label());
        tracing::info!(
            target: "ai",
            event = "inline_context_token_resolved",
            token_bytes = safe_token.raw_bytes,
            token_sha256 = %safe_token.sha256,
            canonical_token_bytes = safe_canonical_token.raw_bytes,
            canonical_token_sha256 = %safe_canonical_token.sha256,
            source_bytes = safe_source.raw_bytes,
            source_sha256 = %safe_source.sha256,
            label_bytes = safe_label.raw_bytes,
            label_sha256 = %safe_label.sha256,
        );
        out.push(InlineContextMention {
            range: span.range,
            token: span.token,
            canonical_token,
            part,
        });
    }
    out
}

/// Return the character-level range of the inline mention token whose span
/// covers `cursor`. Returns `None` when the cursor is not inside or at the
/// boundary of any recognised mention.
pub fn mention_range_at_cursor(text: &str, cursor: usize) -> Option<std::ops::Range<usize>> {
    mention_range_at_cursor_with_aliases(text, cursor, &std::collections::HashMap::new())
}

/// Alias-aware variant of `mention_range_at_cursor`.
pub fn mention_range_at_cursor_with_aliases(
    text: &str,
    cursor: usize,
    aliases: &std::collections::HashMap<String, AiContextPart>,
) -> Option<std::ops::Range<usize>> {
    parse_inline_context_mentions_with_aliases(text, aliases)
        .into_iter()
        .find(|mention| cursor > mention.range.start && cursor <= mention.range.end)
        .map(|mention| mention.range)
}

/// Return the character-level range of the inline mention token to remove for
/// atomic deletion.
///
/// Backspace matches when the cursor is inside or at the trailing boundary.
/// Delete matches when the cursor is inside or at the leading boundary.
pub fn mention_range_for_atomic_delete(
    text: &str,
    cursor: usize,
    delete_forward: bool,
) -> Option<std::ops::Range<usize>> {
    mention_range_for_atomic_delete_with_aliases(
        text,
        cursor,
        delete_forward,
        &std::collections::HashMap::new(),
    )
}

/// Alias-aware variant of `mention_range_for_atomic_delete`.
pub fn mention_range_for_atomic_delete_with_aliases(
    text: &str,
    cursor: usize,
    delete_forward: bool,
    aliases: &std::collections::HashMap<String, AiContextPart>,
) -> Option<std::ops::Range<usize>> {
    if !delete_forward {
        return mention_range_at_cursor_with_aliases(text, cursor, aliases);
    }
    if let Some(range) = mention_range_at_cursor_with_aliases(text, cursor, aliases) {
        return Some(range);
    }
    mention_range_at_cursor_with_aliases(text, cursor.saturating_add(1), aliases)
        .filter(|range| cursor == range.start)
}

/// Convert an `AiContextPart` back to its canonical inline `@token` form.
///
/// Uses typed `@type:name` format for files, commands, and ambient context.
/// Built-in `ResourceUri` parts use their existing `@mention` tokens.
pub(crate) fn part_to_inline_token(part: &AiContextPart) -> Option<String> {
    match part {
        AiContextPart::ResourceUri { uri, .. } => {
            if let Some(id) = uri.strip_prefix("kit://dictation-history?id=") {
                return Some(format_typed_mention_token("dictation", id));
            }
            crate::ai::context_contract::context_attachment_specs()
                .iter()
                .find(|spec| spec.uri == uri.as_str())
                .and_then(|spec| spec.mention)
                .map(ToString::to_string)
        }
        AiContextPart::FilePath { path, .. } => {
            if let AiContextPart::FilePath { label, .. } = part {
                if let Some(token) = crate::pasted_image::token_for_label(label) {
                    return Some(token);
                }
            }
            let prefix = typed_mention_prefix(path);
            let name = typed_mention_display_name(path);
            Some(format_typed_mention_token(prefix, &name))
        }
        AiContextPart::SkillFile {
            skill_name,
            slash_name,
            ..
        } => {
            let name = if skill_name.trim().is_empty() {
                slash_name
            } else {
                skill_name
            };
            Some(format_typed_label_mention_token("skill", name))
        }
        AiContextPart::FocusedTarget { target, .. } => {
            // File/directory targets use canonical @file: / @dir: prefixes with the full basename.
            if let Some(path) = (target.kind == "file" || target.kind == "directory")
                .then_some(target.metadata.as_ref())
                .flatten()
                .and_then(|metadata| metadata.get("path"))
                .and_then(|path| path.as_str())
            {
                let prefix = if target.kind == "directory" {
                    typed_mention_prefix_for_dir()
                } else {
                    typed_mention_prefix(path)
                };
                let name = typed_mention_display_name(path);
                return Some(format_typed_mention_token(prefix, &name));
            }

            // Non-file targets use kind-specific prefixes so script, scriptlet,
            // app, and other Agent Chat attachments stay distinguishable inline.
            Some(format_typed_label_mention_token(
                typed_mention_prefix_for_target_kind(&target.kind),
                &target.label,
            ))
        }
        AiContextPart::AmbientContext { label } => {
            Some(format_typed_label_mention_token("env", label))
        }
        AiContextPart::TextBlock { label, source, .. } => {
            if source.contains("#selection=") {
                return Some("@selected".to_string());
            }
            Some(format_typed_label_mention_token("text", label))
        }
    }
}

/// Legacy format for backward compatibility — used only by the parser to
/// recognize manually typed `@file:/path` tokens.
#[allow(dead_code)]
fn format_inline_file_token_legacy(path: &str) -> String {
    format_inline_file_token(path)
}

/// Returns `true` when the provider-backed mention kind has real data
/// available (slot or env var), as opposed to only the static fallback.
fn provider_backed_mention_available(
    kind: crate::ai::context_contract::ContextAttachmentKind,
) -> bool {
    kind.provider_data_available()
}

/// Resolve a built-in mention token, gating provider-backed kinds on data
/// availability so that manual typing cannot bypass the picker's provider
/// check.
fn resolve_builtin_mention_token(trimmed: &str) -> Option<AiContextPart> {
    let kind = crate::ai::context_contract::ContextAttachmentKind::from_mention_line(trimmed)?;
    if !provider_backed_mention_available(kind) {
        let safe_token = crate::logging::log_private_user_value(trimmed);
        tracing::info!(
            target: "ai",
            event = "inline_context_token_skipped_provider_unavailable",
            token_bytes = safe_token.raw_bytes,
            token_sha256 = %safe_token.sha256,
            kind = ?kind,
        );
        return None;
    }
    Some(kind.part())
}

fn parse_context_mention_line(line: &str) -> Option<AiContextPart> {
    let trimmed = line.trim();
    resolve_builtin_mention_token(trimmed).or_else(|| parse_file_mention(trimmed))
}

fn parse_file_mention(trimmed: &str) -> Option<AiContextPart> {
    let raw_path = trimmed
        .strip_prefix("@file:")
        .or_else(|| trimmed.strip_prefix("@file "))
        .or_else(|| trimmed.strip_prefix("@file\t"))?
        .trim();

    if raw_path.is_empty() {
        return None;
    }

    let path = unescape_quoted_path(raw_path);

    if path.is_empty() {
        return None;
    }

    let label = Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    Some(AiContextPart::FilePath { path, label })
}

pub(crate) fn dictation_history_part_for_entry(
    entry: &crate::dictation::DictationHistoryEntry,
) -> AiContextPart {
    AiContextPart::ResourceUri {
        uri: format!("kit://dictation-history?id={}", entry.id),
        label: format!("Dictation: {}", entry.preview),
    }
}

fn parse_dictation_history_mention(trimmed: &str) -> Option<AiContextPart> {
    let (prefix, value) = typed_mention_token_parts(trimmed)?;
    if prefix != "dictation" {
        return None;
    }

    let entry = crate::dictation::get_history_entry(value.trim())?;
    Some(dictation_history_part_for_entry(&entry))
}

mod sync;
pub(crate) use sync::{
    build_inline_mention_sync_plan, build_inline_mention_sync_plan_with_aliases,
    caret_after_replacement, remove_inline_mention_at_cursor,
    remove_inline_mention_at_cursor_with_aliases, replace_text_in_char_range,
    should_claim_inline_mention_ownership, visible_context_chip_indices_with_aliases,
    InlineMentionSyncPlan,
};

#[cfg(test)]
mod tests;
