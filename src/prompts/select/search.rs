use super::*;

fn score_field(
    nucleo: &mut scripts::NucleoCtx,
    haystack: &str,
    haystack_lower: &str,
    query_lower: &str,
    field_boost: u32,
) -> Option<u32> {
    if haystack.is_empty() {
        return None;
    }

    let mut score = nucleo.score(haystack)?;

    if haystack_lower == query_lower {
        score += 600;
    } else if haystack_lower.starts_with(query_lower) {
        score += 320;
    } else if haystack_lower.contains(query_lower) {
        score += 140;
    }

    Some(score + field_boost)
}

pub(super) fn score_choice_for_filter(
    choice: &Choice,
    indexed_choice: &SelectChoiceIndex,
    query_lower: &str,
    nucleo: &mut scripts::NucleoCtx,
) -> Option<u32> {
    let mut best_score: Option<u32> = None;

    for score in [
        score_field(
            nucleo,
            &choice.name,
            &indexed_choice.name_lower,
            query_lower,
            900,
        ),
        score_field(
            nucleo,
            choice.description.as_deref().unwrap_or_default(),
            &indexed_choice.description_lower,
            query_lower,
            450,
        ),
        score_field(
            nucleo,
            &choice.value,
            &indexed_choice.value_lower,
            query_lower,
            260,
        ),
        score_field(
            nucleo,
            indexed_choice
                .metadata
                .item_type
                .as_deref()
                .unwrap_or_default(),
            &indexed_choice.item_type_lower,
            query_lower,
            240,
        ),
        score_field(
            nucleo,
            indexed_choice
                .metadata
                .last_run
                .as_deref()
                .unwrap_or_default(),
            &indexed_choice.last_run_lower,
            query_lower,
            180,
        ),
        score_field(
            nucleo,
            indexed_choice
                .metadata
                .shortcut
                .as_deref()
                .unwrap_or_default(),
            &indexed_choice.shortcut_lower,
            query_lower,
            120,
        ),
    ]
    .into_iter()
    .flatten()
    {
        best_score = Some(best_score.map_or(score, |current| current.max(score)));
    }

    best_score
}

pub(super) fn char_indices_to_byte_ranges(text: &str, indices: &[usize]) -> Vec<Range<usize>> {
    if indices.is_empty() {
        return Vec::new();
    }

    let mut offsets: Vec<usize> = text.char_indices().map(|(byte_idx, _)| byte_idx).collect();
    offsets.push(text.len());

    let mut ranges: Vec<Range<usize>> = Vec::new();
    for &char_index in indices {
        if char_index + 1 >= offsets.len() {
            continue;
        }
        let start = offsets[char_index];
        let end = offsets[char_index + 1];
        if start >= end {
            continue;
        }

        if let Some(last) = ranges.last_mut() {
            if last.end == start {
                last.end = end;
                continue;
            }
        }
        ranges.push(start..end);
    }

    ranges
}

pub(super) fn highlighted_choice_title(choice_name: &str, query: &str) -> TextContent {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return TextContent::plain(choice_name.to_string());
    }

    let query_lower = trimmed_query.to_lowercase();
    let (matched, indices) =
        crate::scripts::search::fuzzy_match_with_indices_ascii(choice_name, &query_lower);
    if !matched || indices.is_empty() {
        return TextContent::plain(choice_name.to_string());
    }

    let ranges = char_indices_to_byte_ranges(choice_name, &indices);
    if ranges.is_empty() {
        TextContent::plain(choice_name.to_string())
    } else {
        TextContent::highlighted(choice_name.to_string(), ranges)
    }
}

pub(super) fn choice_selection_indicator(is_multiple: bool, is_selected: bool) -> &'static str {
    if is_multiple {
        if is_selected {
            "☑"
        } else {
            "☐"
        }
    } else if is_selected {
        "●"
    } else {
        "○"
    }
}
pub(super) fn should_append_to_filter(ch: char) -> bool {
    !ch.is_control()
}

pub(super) fn are_all_filtered_selected(
    selected_indices: &HashSet<usize>,
    filtered_indices: &[usize],
) -> bool {
    !filtered_indices.is_empty()
        && filtered_indices
            .iter()
            .all(|idx| selected_indices.contains(idx))
}

pub(super) fn toggle_filtered_selection(
    selected_indices: &mut HashSet<usize>,
    filtered_indices: &[usize],
) {
    if are_all_filtered_selected(selected_indices, filtered_indices) {
        for idx in filtered_indices {
            selected_indices.remove(idx);
        }
    } else {
        selected_indices.extend(filtered_indices.iter().copied());
    }
}

pub(super) fn toggle_choice_selection(
    selected_indices: &mut HashSet<usize>,
    choice_index: usize,
    is_multiple: bool,
) -> bool {
    if !is_multiple {
        return false;
    }

    if selected_indices.contains(&choice_index) {
        selected_indices.remove(&choice_index);
    } else {
        selected_indices.insert(choice_index);
    }

    true
}

pub(super) fn resolve_submission_indices(
    is_multiple: bool,
    selected_indices: &[usize],
    focused_choice_index: Option<usize>,
) -> Vec<usize> {
    if !is_multiple {
        return focused_choice_index.into_iter().collect();
    }

    if !selected_indices.is_empty() {
        return selected_indices.to_vec();
    }

    Vec::new()
}

pub(crate) fn select_submission_is_allowed(is_multiple: bool, selected_count: usize) -> bool {
    !is_multiple || selected_count > 0
}

pub(super) fn resolve_search_box_bg_hex(
    theme: &theme::Theme,
    design_variant: DesignVariant,
    design_colors: &DesignColors,
) -> u32 {
    if design_variant == DesignVariant::Default {
        theme.colors.background.search_box
    } else {
        design_colors.background_secondary
    }
}
