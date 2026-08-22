use crate::ai::context_contract::ContextAttachmentKind;
use crate::ai::message_parts::AiContextPart;
use crate::spine::{SpineParse, SpineSegmentKind, SpineSegmentResolution};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpinePromptPlan {
    pub raw_input: String,
    pub normalized_prompt: String,
    pub selected_profile: Option<SpinePromptProfile>,
    pub selected_style: Option<SpinePromptStyle>,
    pub slash_commands: Vec<SpinePromptSlashCommand>,
    pub context_parts: Vec<AiContextPart>,
    pub unknown_warnings: Vec<SpinePromptWarning>,
    pub free_text_tail: String,
    pub prompt_builder_segment_count: usize,
    pub blocked_reason: Option<SpinePromptPlanBlockReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpinePromptProfile {
    pub id: String,
    pub label: String,
    pub source: SpinePromptProfileSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpinePromptProfileSource {
    ProfileSegment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpinePromptStyle {
    pub id: String,
    pub label: String,
    /// The rewrite instruction delivered to the agent. Styles are NOT Agent
    /// Chat profiles — a previous implementation persisted the style id as a
    /// profile selection, which always failed lookup and silently dropped
    /// the tone. The instruction is the style's real payload.
    pub instruction: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpinePromptSlashCommand {
    pub command: String,
    pub name: String,
    pub segment_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpinePromptWarning {
    pub segment_index: usize,
    pub raw: String,
    pub preflight_instruction: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpinePromptPlanBlockReason {
    NoPromptBuilderSegments,
    Capture,
    ListFilter,
    ModeExit,
}

impl SpinePromptPlan {
    pub(crate) fn should_submit_to_chat(&self) -> bool {
        if self.blocked_reason.is_some() || self.prompt_builder_segment_count == 0 {
            return false;
        }
        // Require at least one form of substantive content. An unknown
        // warning alone — e.g. the user pressed Cmd+Enter while mid-typing
        // `@clip` — means we'd otherwise submit just a preflight warning
        // to the agent, which is almost never what the user intended.
        !self.free_text_tail.trim().is_empty()
            || !self.context_parts.is_empty()
            || !self.slash_commands.is_empty()
            || self.selected_profile.is_some()
            || self.selected_style.is_some()
    }
}

/// A9 decision (2026-06-09): `.` + picking a style is a single-keystroke
/// "rewrite selected text" flow. When the style segment is the ENTIRE input
/// (no other prose or sigil segments), accepting a style row auto-submits
/// the prompt plan — which style sugar expands to `@selection` + `/rewrite`
/// plus the style profile. Mixed inputs (`fix this .formal`) keep the normal
/// insert-and-keep-typing behavior.
pub(crate) fn spine_parse_is_style_only(parse: &SpineParse) -> bool {
    let mut saw_style = false;
    for segment in &parse.segments {
        match &segment.kind {
            SpineSegmentKind::Style { .. } => saw_style = true,
            SpineSegmentKind::FreeText if segment.raw.trim().is_empty() => {}
            _ => return false,
        }
    }
    saw_style
}

pub(crate) fn build_spine_prompt_plan(parse: &SpineParse) -> SpinePromptPlan {
    build_spine_prompt_plan_with_aliases(parse, &std::collections::HashMap::new())
}

/// Alias-aware variant: compact mention tokens (`@file:basename.ext`) are
/// resolved through the session alias registry before the literal `@file:`
/// path fallback, so the visible token can stay short while the attached
/// context keeps the absolute path. Mirrors
/// `context_mentions::parse_inline_context_mentions_with_aliases` ordering.
pub(crate) fn build_spine_prompt_plan_with_aliases(
    parse: &SpineParse,
    mention_aliases: &std::collections::HashMap<String, AiContextPart>,
) -> SpinePromptPlan {
    let raw_input = parse.input.clone();
    let mut plan = SpinePromptPlan {
        raw_input: raw_input.clone(),
        normalized_prompt: String::new(),
        selected_profile: None,
        selected_style: None,
        slash_commands: Vec::new(),
        context_parts: Vec::new(),
        unknown_warnings: Vec::new(),
        free_text_tail: String::new(),
        prompt_builder_segment_count: 0,
        blocked_reason: None,
    };

    let mut free_text_chunks: Vec<String> = Vec::new();

    for (index, segment) in parse.segments.iter().enumerate() {
        let text = segment.raw.trim().to_string();
        if text.is_empty() {
            continue;
        }

        match &segment.kind {
            SpineSegmentKind::FreeText => {
                free_text_chunks.push(text.clone());
                plan.free_text_tail = text;
            }
            SpineSegmentKind::ContextMention { context_type, .. } => {
                plan.prompt_builder_segment_count += 1;
                if let Some(kind) = ContextAttachmentKind::from_mention_line(&text) {
                    let part = kind.part();
                    push_context_part_dedup(&mut plan, part);
                } else if let Some(part) = mention_aliases.get(&text) {
                    push_context_part_dedup(&mut plan, part.clone());
                } else if text.starts_with("@file:") {
                    let path = text.strip_prefix("@file:").unwrap_or("").trim();
                    if !path.is_empty() {
                        let label = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path)
                            .to_string();
                        push_context_part_dedup(
                            &mut plan,
                            AiContextPart::FilePath {
                                path: path.to_string(),
                                label,
                            },
                        );
                    }
                } else if let SpineSegmentResolution::Resolved { id, label, source } =
                    &segment.resolution
                {
                    if source == "context-builtin" {
                        if let Some(kind) =
                            ContextAttachmentKind::from_mention_line(&format!("@{context_type}"))
                        {
                            push_context_part_dedup(&mut plan, kind.part());
                        }
                    } else {
                        push_context_part_dedup(
                            &mut plan,
                            AiContextPart::TextBlock {
                                label: label.clone(),
                                source: format!("spine:{source}:{id}"),
                                text: String::new(),
                                mime_type: None,
                            },
                        );
                    }
                } else {
                    plan.unknown_warnings.push(SpinePromptWarning {
                        segment_index: index,
                        raw: text.clone(),
                        preflight_instruction: format!(
                            "Preflight warning: the user typed `{text}`, but it did not resolve to a known context. Do not invent hidden context; ask for clarification if it matters."
                        ),
                    });
                }
            }
            SpineSegmentKind::SlashCommand { .. } => {
                plan.prompt_builder_segment_count += 1;
                if let Some(kind) = ContextAttachmentKind::from_slash_command(&text) {
                    push_context_part_dedup(&mut plan, kind.part());
                } else {
                    let name = text.trim_start_matches('/').to_string();
                    plan.slash_commands.push(SpinePromptSlashCommand {
                        command: text.clone(),
                        name,
                        segment_index: index,
                    });
                }
            }
            SpineSegmentKind::Profile { profile_id } => {
                plan.prompt_builder_segment_count += 1;
                if !profile_id.is_empty() {
                    plan.selected_profile = Some(SpinePromptProfile {
                        id: profile_id.clone(),
                        label: profile_id.clone(),
                        source: SpinePromptProfileSource::ProfileSegment,
                    });
                }
            }
            SpineSegmentKind::Style { style_id } => {
                plan.prompt_builder_segment_count += 1;
                if style_id.is_empty() {
                    // nothing to do
                } else if crate::spine::catalog_style::is_known_style(style_id) {
                    apply_style_sugar(&mut plan, index, style_id.clone());
                } else {
                    // Unknown `.foo` downgrades to free text (`.env`,
                    // `.gitignore`, trailing periods) instead of fabricating
                    // a rewrite instruction the user never picked.
                    free_text_chunks.push(text.clone());
                }
            }
            SpineSegmentKind::Capture { .. } => {
                plan.blocked_reason = Some(SpinePromptPlanBlockReason::Capture);
            }
            SpineSegmentKind::ListFilter { .. } => {
                plan.blocked_reason = Some(SpinePromptPlanBlockReason::ListFilter);
            }
            SpineSegmentKind::ProjectCwd { .. } => {
                plan.prompt_builder_segment_count += 1;
            }
            SpineSegmentKind::Flow { .. } => {
                // A resolved flow token (`-gmail`) stages its flow file as a
                // context part on acceptance (Agent Chat composer); the token
                // itself contributes nothing further to the prompt plan.
                plan.prompt_builder_segment_count += 1;
            }
            SpineSegmentKind::ModeExit { .. } => {
                plan.blocked_reason = Some(SpinePromptPlanBlockReason::ModeExit);
            }
        }
    }

    if plan.prompt_builder_segment_count == 0 && plan.blocked_reason.is_none() {
        plan.blocked_reason = Some(SpinePromptPlanBlockReason::NoPromptBuilderSegments);
    }

    plan.normalized_prompt = build_normalized_prompt_text(&plan, &free_text_chunks);
    plan
}

/// Only called for known styles; unknown `.foo` downgrades to free text in
/// the Style segment arm above.
fn apply_style_sugar(plan: &mut SpinePromptPlan, segment_index: usize, id: String) {
    let _ = segment_index;
    let instruction = crate::spine::catalog_style::style_instruction(&id)
        .unwrap_or_else(|| {
            format!(
                "Rewrite the attached selection in a {id} style. Preserve the meaning; return only the rewritten text."
            )
        });
    plan.selected_style = Some(SpinePromptStyle {
        id: id.clone(),
        label: id,
        instruction,
    });
    let selection = ContextAttachmentKind::Selection.part();
    push_context_part_dedup(plan, selection);
}

fn push_context_part_dedup(plan: &mut SpinePromptPlan, part: AiContextPart) {
    if !plan.context_parts.iter().any(|existing| existing == &part) {
        plan.context_parts.push(part);
    }
}

fn build_normalized_prompt_text(plan: &SpinePromptPlan, free_text_chunks: &[String]) -> String {
    let mut pieces: Vec<String> = Vec::new();

    if !plan.unknown_warnings.is_empty() {
        pieces.push(
            plan.unknown_warnings
                .iter()
                .map(|w| w.preflight_instruction.clone())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    if !plan.slash_commands.is_empty() {
        pieces.push(
            plan.slash_commands
                .iter()
                .map(|cmd| cmd.command.clone())
                .collect::<Vec<_>>()
                .join(" "),
        );
    }

    // The style's real payload: an explicit rewrite instruction. Without
    // this the agent only ever saw a bare `/rewrite` and the chosen tone
    // never reached it.
    if let Some(style) = &plan.selected_style {
        pieces.push(style.instruction.clone());
    }

    let prose = free_text_chunks
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if !prose.is_empty() {
        pieces.push(prose);
    }

    pieces
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spine::parse_spine;

    #[test]
    fn style_only_parse_detection_gates_auto_submit() {
        // A9: `.concise` alone (with or without trailing space) auto-submits.
        assert!(spine_parse_is_style_only(&parse_spine(".concise")));
        assert!(spine_parse_is_style_only(&parse_spine(".concise ")));
        // Mixed prose or other sigils keep manual submit.
        assert!(!spine_parse_is_style_only(&parse_spine(
            "fix this .concise"
        )));
        assert!(!spine_parse_is_style_only(&parse_spine(
            ".concise extra words"
        )));
        assert!(!spine_parse_is_style_only(&parse_spine(
            "@selection .concise"
        )));
        assert!(!spine_parse_is_style_only(&parse_spine("")));
        assert!(!spine_parse_is_style_only(&parse_spine("plain text")));
    }

    #[test]
    fn style_only_plan_expands_to_selection_rewrite() {
        // The auto-submitted plan must carry the rewrite-selection semantics:
        // an explicit tone instruction in the prompt + the selection part.
        let plan = build_spine_prompt_plan(&parse_spine(".concise"));
        assert!(plan.should_submit_to_chat());
        assert_eq!(plan.selected_style.as_ref().unwrap().id, "concise");
        assert!(
            plan.normalized_prompt.contains("more concise"),
            "style instruction must reach the prompt, got: {}",
            plan.normalized_prompt,
        );
        assert!(plan.context_parts.iter().any(|p| p.label() == "Selection"));
        // Styles are not profiles — the plan must not pretend they are.
        assert!(plan.selected_profile.is_none());
    }

    #[test]
    fn prompt_plan_builds_rewrite_selection_plan() {
        let parse = parse_spine("|creative /rewrite @selection make it punchier");
        let plan = build_spine_prompt_plan(&parse);
        assert!(plan.should_submit_to_chat());
        assert_eq!(plan.selected_profile.as_ref().unwrap().id, "creative");
        assert!(plan.slash_commands.iter().any(|c| c.command == "/rewrite"));
        assert!(plan.context_parts.iter().any(|p| p.label() == "Selection"));
        assert_eq!(plan.free_text_tail, "make it punchier");
    }

    #[test]
    fn prompt_plan_expands_style_sugar() {
        let parse = parse_spine(".professional make it shorter");
        let plan = build_spine_prompt_plan(&parse);
        assert!(plan.should_submit_to_chat());
        assert_eq!(plan.selected_style.as_ref().unwrap().id, "professional");
        assert!(
            plan.normalized_prompt.contains("professional"),
            "tone instruction missing from prompt: {}",
            plan.normalized_prompt,
        );
        assert!(plan.context_parts.iter().any(|p| p.label() == "Selection"));
        assert_eq!(plan.free_text_tail, "make it shorter");
        assert!(plan.selected_profile.is_none());
    }

    #[test]
    fn unknown_style_id_downgrades_to_free_text() {
        let plan = build_spine_prompt_plan(&parse_spine("rename .env to .env.local"));
        assert!(
            plan.selected_style.is_none(),
            "unknown .foo must not become a style: {:?}",
            plan.selected_style,
        );
        assert!(
            plan.normalized_prompt.contains(".env"),
            "downgraded style text must survive as prose: {}",
            plan.normalized_prompt,
        );
    }

    #[test]
    fn prompt_plan_blocks_capture() {
        let parse = parse_spine(";todo Buy milk");
        let plan = build_spine_prompt_plan(&parse);
        assert!(!plan.should_submit_to_chat());
        assert_eq!(
            plan.blocked_reason,
            Some(SpinePromptPlanBlockReason::Capture)
        );
    }

    #[test]
    fn prompt_plan_blocks_filter() {
        let parse = parse_spine(":type:script git");
        let plan = build_spine_prompt_plan(&parse);
        assert!(!plan.should_submit_to_chat());
        assert_eq!(
            plan.blocked_reason,
            Some(SpinePromptPlanBlockReason::ListFilter)
        );
    }

    #[test]
    fn prompt_plan_resolves_compact_file_tokens_via_aliases() {
        let parse = parse_spine("@file:demo.rs summarize");
        let mut aliases = std::collections::HashMap::new();
        aliases.insert(
            "@file:demo.rs".to_string(),
            AiContextPart::FilePath {
                path: "/tmp/nested/demo.rs".to_string(),
                label: "demo.rs".to_string(),
            },
        );
        let plan = build_spine_prompt_plan_with_aliases(&parse, &aliases);
        assert!(plan.should_submit_to_chat());
        assert!(
            plan.context_parts.iter().any(|part| matches!(
                part,
                AiContextPart::FilePath { path, label }
                    if path == "/tmp/nested/demo.rs" && label == "demo.rs"
            )),
            "compact @file token must resolve to its aliased full path, got {:?}",
            plan.context_parts,
        );
        assert_eq!(plan.free_text_tail, "summarize");
    }

    #[test]
    fn prompt_plan_preserves_duplicate_named_catalog_attachments_by_owner() {
        let first_skill = AiContextPart::FilePath {
            path: "/plugins/first/deploy/SKILL.md".to_string(),
            label: "Deploy".to_string(),
        };
        let second_skill = AiContextPart::FilePath {
            path: "/plugins/second/deploy/SKILL.md".to_string(),
            label: "Deploy".to_string(),
        };
        let first_scriptlet = AiContextPart::TextBlock {
            label: "Deploy".to_string(),
            source: "spine:scriptlets:/plugins/first/deploy.md".to_string(),
            text: "first owner's command".to_string(),
            mime_type: None,
        };
        let second_scriptlet = AiContextPart::TextBlock {
            label: "Deploy".to_string(),
            source: "spine:scriptlets:/plugins/second/deploy.md".to_string(),
            text: "second owner's command".to_string(),
            mime_type: None,
        };
        let aliases = std::collections::HashMap::from([
            ("@skills:Deploy".to_string(), first_skill.clone()),
            ("@skills:Deploy-2".to_string(), second_skill.clone()),
            ("@scriptlets:Deploy".to_string(), first_scriptlet.clone()),
            ("@scriptlets:Deploy-2".to_string(), second_scriptlet.clone()),
        ]);
        let parse = parse_spine(
            "@skills:Deploy @skills:Deploy-2 @scriptlets:Deploy @scriptlets:Deploy-2 compare",
        );

        let plan = build_spine_prompt_plan_with_aliases(&parse, &aliases);

        assert!(plan.should_submit_to_chat());
        assert_eq!(
            plan.context_parts,
            [first_skill, second_skill, first_scriptlet, second_scriptlet]
        );
        assert_eq!(plan.free_text_tail, "compare");
    }

    #[test]
    fn prompt_plan_without_alias_falls_back_to_literal_file_path() {
        let parse = parse_spine("@file:/tmp/demo.rs summarize");
        let plan = build_spine_prompt_plan(&parse);
        assert!(plan.context_parts.iter().any(|part| matches!(
            part,
            AiContextPart::FilePath { path, .. } if path == "/tmp/demo.rs"
        )));
    }

    #[test]
    fn prompt_plan_warns_for_unknown_context() {
        let parse = parse_spine("@unknownThing summarize");
        let plan = build_spine_prompt_plan(&parse);
        assert!(plan.should_submit_to_chat());
        assert_eq!(plan.unknown_warnings.len(), 1);
        assert!(plan.normalized_prompt.contains("Preflight warning"));
        assert!(plan.normalized_prompt.contains("summarize"));
    }

    #[test]
    fn partial_unresolved_sigil_alone_does_not_submit() {
        // Oracle scenario #17 regression: pressing Cmd+Enter while typing
        // `@clip` (no resolved context, no free text) used to submit a
        // synthetic "Preflight warning" prompt to the agent. Users hit
        // this constantly because they Cmd+Enter mid-typing.
        let parse = parse_spine("@clip");
        let plan = build_spine_prompt_plan(&parse);
        assert!(!plan.should_submit_to_chat());
        assert_eq!(plan.unknown_warnings.len(), 1);
        assert!(plan.context_parts.is_empty());
        assert!(plan.free_text_tail.is_empty());
    }

    #[test]
    fn plain_text_does_not_submit() {
        let parse = parse_spine("hello world");
        let plan = build_spine_prompt_plan(&parse);
        assert!(!plan.should_submit_to_chat());
        assert_eq!(
            plan.blocked_reason,
            Some(SpinePromptPlanBlockReason::NoPromptBuilderSegments)
        );
    }
}
