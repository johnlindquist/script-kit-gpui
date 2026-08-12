use std::ops::Range;

use super::list::{matches_query, ss, SpineListAction, SpineListRow, SpineListRowKind};

struct StyleSpec {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    icon: &'static str,
    /// The rewrite instruction the agent actually receives when this style
    /// is part of the submitted prompt plan.
    instruction: &'static str,
}

const SPINE_STYLES: &[StyleSpec] = &[
    StyleSpec {
        id: "professional",
        title: "Professional",
        description: "Polished workplace tone",
        icon: "briefcase",
        instruction: "Rewrite the attached selection in a polished, professional workplace tone. Preserve the meaning and approximate length; return only the rewritten text.",
    },
    StyleSpec {
        id: "concise",
        title: "Concise",
        description: "Shorten without losing meaning",
        icon: "minimize-2",
        instruction: "Rewrite the attached selection to be significantly more concise without losing meaning. Return only the rewritten text.",
    },
    StyleSpec {
        id: "friendly",
        title: "Friendly",
        description: "Warmer tone",
        icon: "smile",
        instruction: "Rewrite the attached selection in a warmer, friendlier tone. Preserve the meaning; return only the rewritten text.",
    },
    StyleSpec {
        id: "direct",
        title: "Direct",
        description: "Plainspoken and direct",
        icon: "arrow-right",
        instruction: "Rewrite the attached selection to be plainspoken and direct. Cut hedging and filler; return only the rewritten text.",
    },
];

/// A catalog style after merging built-ins with user-defined styles from
/// `config.ts` (`spineStyles`). User entries with a matching id override
/// built-ins; new ids extend the catalog.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedStyle {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub instruction: String,
}

pub(crate) fn resolved_styles() -> Vec<ResolvedStyle> {
    resolved_styles_with(&crate::config::load_config().spine_styles)
}

fn resolved_styles_with(user_styles: &[crate::config::SpineStyleConfig]) -> Vec<ResolvedStyle> {
    let mut styles: Vec<ResolvedStyle> = SPINE_STYLES
        .iter()
        .map(|spec| ResolvedStyle {
            id: spec.id.to_string(),
            title: spec.title.to_string(),
            description: spec.description.to_string(),
            icon: spec.icon.to_string(),
            instruction: spec.instruction.to_string(),
        })
        .collect();

    for user in user_styles {
        let resolved = ResolvedStyle {
            id: user.id.clone(),
            title: user
                .title
                .clone()
                .unwrap_or_else(|| capitalize_first(&user.id)),
            description: user.description.clone().unwrap_or_default(),
            icon: user.icon.clone().unwrap_or_else(|| "sparkles".to_string()),
            instruction: user.instruction.clone(),
        };
        if let Some(existing) = styles.iter_mut().find(|s| s.id == resolved.id) {
            *existing = resolved;
        } else {
            styles.push(resolved);
        }
    }
    styles
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Rewrite instruction for a known style id, used by the prompt plan so the
/// agent receives an explicit tone instruction instead of a bare `/rewrite`.
pub(crate) fn style_instruction(style_id: &str) -> Option<String> {
    resolved_styles()
        .into_iter()
        .find(|spec| spec.id == style_id)
        .map(|spec| spec.instruction)
}

/// Whether the style id is a known catalog style.
pub(crate) fn is_known_style(style_id: &str) -> bool {
    resolved_styles().iter().any(|spec| spec.id == style_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_styles_extend_and_override_builtins() {
        let user = vec![
            crate::config::SpineStyleConfig {
                id: "pirate".to_string(),
                title: None,
                description: None,
                icon: None,
                instruction: "Rewrite like a pirate.".to_string(),
            },
            crate::config::SpineStyleConfig {
                id: "concise".to_string(),
                title: Some("Ultra Concise".to_string()),
                description: Some("Configured brevity description".to_string()),
                icon: None,
                instruction: "Halve the word count.".to_string(),
            },
        ];
        let styles = resolved_styles_with(&user);
        let pirate = styles.iter().find(|s| s.id == "pirate").expect("pirate");
        assert_eq!(pirate.title, "Pirate");
        assert_eq!(pirate.instruction, "Rewrite like a pirate.");
        let concise = styles.iter().find(|s| s.id == "concise").expect("concise");
        assert_eq!(concise.title, "Ultra Concise");
        assert_eq!(concise.description, "Configured brevity description");
        assert_eq!(concise.instruction, "Halve the word count.");
        // Built-ins not overridden remain intact.
        assert!(styles.iter().any(|s| s.id == "professional"));
    }

    #[test]
    fn style_rows_surface_no_sigil_badge() {
        // The `.` sigil is plumbing; the rewrite UI must not surface it.
        let rows = build_style_rows("", 0, 0..0, true);
        assert!(!rows.is_empty());
        assert!(
            rows.iter().all(|row| row.badges.is_empty()),
            "style rows must not carry a '.' badge"
        );
    }
}

pub(super) fn build_style_rows(
    query: &str,
    segment_index: usize,
    segment_byte_range: Range<usize>,
    auto_submit_rewrite: bool,
) -> Vec<SpineListRow> {
    resolved_styles()
        .into_iter()
        .enumerate()
        .filter(|(_, spec)| {
            let dot_text = format!(".{}", spec.id);
            matches_query(&spec.id, query)
                || matches_query(&spec.title, query)
                || matches_query(&dot_text, query)
                || matches_query(&spec.description, query)
        })
        .map(|(rank, spec)| {
            let replacement = format!(".{}", spec.id);
            SpineListRow {
                id: ss(format!("spine:.:{}", spec.id)),
                kind: SpineListRowKind::Style {
                    style_id: ss(spec.id.clone()),
                },
                title: ss(spec.title),
                subtitle: Some(ss(spec.description)),
                meta: None,
                icon: Some(ss(spec.icon)),
                // No "." badge: the sigil is plumbing, not something the
                // rewrite UI should surface to the user.
                badges: Vec::new(),
                score: i32::MAX.saturating_sub(rank as i32),
                is_selectable: true,
                // Style-only input auto-submits the rewrite plan to Agent
                // Chat on Enter (A9 decision), so the footer verb is
                // "Rewrite", not the generic prompt-builder "Attach".
                action_label: auto_submit_rewrite.then(|| ss("Rewrite")),
                action: SpineListAction::ResolveSegment {
                    segment_index,
                    segment_byte_range: segment_byte_range.clone(),
                    replacement: ss(replacement.clone()),
                    resolution_id: ss(spec.id),
                    resolution_label: ss(replacement),
                    resolution_source: ss("style"),
                    trailing_space: true,
                },
            }
        })
        .collect()
}
