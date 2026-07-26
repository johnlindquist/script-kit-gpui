use std::cmp::Ordering;

use crate::flows::model::FlowDescriptor;

use super::super::types::{FlowMatch, MatchIndices};
use super::{
    low_tier_substring_match, primary_text_match, NucleoCtx, TIER_DESCRIPTION, TIER_FILENAME,
};

fn flow_subtitle(flow: &FlowDescriptor) -> String {
    let purpose = flow
        .description
        .clone()
        .unwrap_or_else(|| flow.name.clone());
    format!("{purpose} · {} · {}", flow.engine, flow.origin_label())
}

/// Fuzzy search mdflow flows by query string.
/// Matches against the friendly display name, raw flow name, and description.
/// Returns results sorted by relevance score (highest first).
pub fn fuzzy_search_flows(flows: &[FlowDescriptor], query: &str) -> Vec<FlowMatch> {
    if query.is_empty() {
        let mut all: Vec<FlowMatch> = flows
            .iter()
            .map(|flow| FlowMatch {
                target: super::super::types::ConversationRowTarget::FlowIdentity {
                    flow_id: flow.id.clone(),
                },
                flow: Some(flow.clone()),
                display_name: flow.friendly_name(),
                subtitle: flow_subtitle(flow),
                score: 0,
                match_indices: MatchIndices::default(),
            })
            .collect();
        all.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        return all;
    }

    let query_lower = query.to_lowercase();
    let mut nucleo = NucleoCtx::new(&query_lower);
    let mut matches: Vec<FlowMatch> = Vec::new();

    for flow in flows {
        let display_name = flow.friendly_name();

        let mut best_score = 0i32;
        let mut name_indices: Vec<usize> = Vec::new();
        let mut description_indices: Vec<usize> = Vec::new();

        if let Some(m) = primary_text_match(&display_name, &query_lower, &mut nucleo) {
            best_score = m.score;
            name_indices = m.indices;
        }

        // Raw flow name (e.g. `flow-gmail`) matches too, without highlights
        // because the row renders the friendly name.
        if let Some(m) = low_tier_substring_match(&flow.name, &query_lower, TIER_FILENAME) {
            best_score = best_score.max(m.score);
        }

        if let Some(description) = flow.description.as_deref() {
            if let Some(m) = low_tier_substring_match(description, &query_lower, TIER_DESCRIPTION) {
                if m.score > best_score {
                    best_score = m.score;
                }
                if name_indices.is_empty() {
                    description_indices = m.indices;
                }
            }
        }

        if best_score > 0 {
            matches.push(FlowMatch {
                target: super::super::types::ConversationRowTarget::FlowIdentity {
                    flow_id: flow.id.clone(),
                },
                flow: Some(flow.clone()),
                subtitle: flow_subtitle(flow),
                display_name,
                score: best_score,
                match_indices: MatchIndices {
                    name_indices,
                    filename_indices: Vec::new(),
                    description_indices,
                },
            });
        }
    }

    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => a.display_name.cmp(&b.display_name),
        other => other,
    });

    matches
}
