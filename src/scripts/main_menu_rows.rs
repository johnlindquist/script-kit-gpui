use crate::list_item::GroupedListItem;
use crate::scripts;

pub(crate) const INLINE_CALCULATOR_RESULT_INDEX: usize = usize::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainMenuRowSubject {
    SearchResult { flat_index: usize },
    Calculator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MainMenuRowProjection {
    pub(crate) grouped_index: usize,
    pub(crate) selectable_ordinal: Option<usize>,
    pub(crate) subject: MainMenuRowSubject,
    pub(crate) stable_key: String,
    pub(crate) semantic_id: String,
    pub(crate) content_fingerprint: String,
    pub(crate) eligibility: crate::list_item::RowEligibility,
}
pub(crate) fn project_main_menu_rows(
    grouped: &mut Vec<GroupedListItem>,
    results: &mut Vec<scripts::SearchResult>,
    calculator: Option<&crate::calculator::CalculatorInlineResult>,
    query: &str,
) -> Result<Vec<MainMenuRowProjection>, String> {
    use crate::list_item::RowEligibility;
    use sha2::{Digest, Sha256};

    let mut identities = std::collections::HashMap::<String, (usize, String)>::new();
    let mut remap = Vec::with_capacity(results.len());
    let mut unique = Vec::with_capacity(results.len());
    let mut metadata = Vec::with_capacity(results.len());
    for result in results.iter() {
        let key = result
            .stable_selection_key()
            .ok_or("main_menu_row_identity_missing")?;
        let fingerprint = result.main_menu_content_fingerprint();
        if let Some((index, existing)) = identities.get(&key) {
            if existing != &fingerprint {
                return Err("main_menu_row_identity_conflict".to_string());
            }
            remap.push(*index);
            continue;
        }
        let index = unique.len();
        identities.insert(key.clone(), (index, fingerprint.clone()));
        remap.push(index);
        let eligibility = match result {
            scripts::SearchResult::SpineProjection(row) if !row.is_selectable => {
                RowEligibility::inert()
            }
            _ if result
                .command_descriptor()
                .is_ok_and(|command| command.can_execute()) =>
            {
                RowEligibility::enabled_action()
            }
            _ => RowEligibility::disabled_explanation(),
        };
        let semantic_id = scripts::command_contract::main_menu_row_semantic_id(&key);
        metadata.push((key, semantic_id, fingerprint, eligibility));
        unique.push(result.clone());
    }

    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::with_capacity(grouped.len());
    let mut rows = Vec::with_capacity(unique.len() + usize::from(calculator.is_some()));
    let mut ordinal = 0;
    for item in grouped.iter() {
        let grouped_index = normalized.len();
        let GroupedListItem::Item(flat_index) = item else {
            normalized.push(item.clone());
            continue;
        };
        let (subject, key, semantic_id, fingerprint, eligibility) =
            if *flat_index == INLINE_CALCULATOR_RESULT_INDEX {
                let Some(calculator) = calculator else {
                    normalized.push(item.clone());
                    continue;
                };
                let key = format!(
                    "calculator:{}",
                    scripts::command_contract::main_menu_row_semantic_id(query)
                );
                if !seen.insert(key.clone()) {
                    continue;
                }
                let mut digest = Sha256::new();
                for value in [
                    calculator.raw_input.as_str(),
                    calculator.normalized_expr.as_str(),
                    calculator.operation_name.as_str(),
                    calculator.formatted.as_str(),
                    calculator.words.as_str(),
                ] {
                    digest.update((value.len() as u64).to_be_bytes());
                    digest.update(value.as_bytes());
                }
                digest.update(calculator.value.to_bits().to_be_bytes());
                let fingerprint = format!("{:x}", digest.finalize());
                let semantic_id = scripts::command_contract::main_menu_row_semantic_id(&key);
                normalized.push(item.clone());
                (
                    MainMenuRowSubject::Calculator,
                    key,
                    semantic_id,
                    fingerprint,
                    RowEligibility::enabled_action(),
                )
            } else {
                let Some(&index) = remap.get(*flat_index) else {
                    normalized.push(item.clone());
                    continue;
                };
                let (key, semantic_id, fingerprint, eligibility) = &metadata[index];
                if !seen.insert(key.clone()) {
                    continue;
                }
                normalized.push(GroupedListItem::Item(index));
                (
                    MainMenuRowSubject::SearchResult { flat_index: index },
                    key.clone(),
                    semantic_id.clone(),
                    fingerprint.clone(),
                    *eligibility,
                )
            };
        let selectable_ordinal = eligibility.selectable.then(|| {
            let current = ordinal;
            ordinal += 1;
            current
        });
        rows.push(MainMenuRowProjection {
            grouped_index,
            selectable_ordinal,
            subject,
            stable_key: key,
            semantic_id,
            content_fingerprint: fingerprint,
            eligibility,
        });
    }
    *grouped = normalized;
    *results = unique;
    Ok(rows)
}

/// Keep the current query's already displayed rows in place. Newly arriving
/// matches remain reachable below them; the next query gets normal ranking.
pub(crate) fn preserve_displayed_main_menu_rows(
    previous_items: &[GroupedListItem],
    previous_rows: &[MainMenuRowProjection],
    items: &mut Vec<GroupedListItem>,
    rows: &mut Vec<MainMenuRowProjection>,
) {
    if previous_rows.is_empty() || rows.is_empty() {
        return;
    }
    let by_key: std::collections::HashMap<&str, usize> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.stable_key.as_str(), index))
        .collect();
    let mut retained = vec![false; rows.len()];
    let mut order = Vec::with_capacity(rows.len());
    let mut stable_items = Vec::with_capacity(items.len() + previous_items.len());
    let mut header_start = None;
    let mut header_end = 0;
    for (index, item) in previous_items.iter().enumerate() {
        match item {
            GroupedListItem::SectionHeader(..) | GroupedListItem::ReservedSectionSlot => {
                if header_start.is_none() || header_end != index {
                    header_start = Some(index);
                }
                header_end = index + 1;
            }
            GroupedListItem::Item(_) => {
                let Ok(old_index) =
                    previous_rows.binary_search_by_key(&index, |row| row.grouped_index)
                else {
                    continue;
                };
                let Some(&new_index) = by_key.get(previous_rows[old_index].stable_key.as_str())
                else {
                    continue;
                };
                if let Some(start) = header_start.take() {
                    stable_items.extend_from_slice(&previous_items[start..header_end]);
                }
                stable_items.push(items[rows[new_index].grouped_index].clone());
                retained[new_index] = true;
                order.push(new_index);
            }
            GroupedListItem::Status(_) => {}
        }
    }
    if order.is_empty() {
        return;
    }
    if order.len() != rows.len() {
        let last_header = stable_items.iter().rev().find_map(|item| match item {
            GroupedListItem::SectionHeader(label, _) => Some(label.as_str()),
            _ => None,
        });
        if last_header != Some("More results") {
            stable_items.push(GroupedListItem::SectionHeader("More results".into(), None));
        }
        for (index, row) in rows.iter().enumerate() {
            if !retained[index] {
                stable_items.push(items[row.grouped_index].clone());
                order.push(index);
            }
        }
    }
    drop(by_key);
    let mut original: Vec<_> = std::mem::take(rows).into_iter().map(Some).collect();
    let mut order = order.into_iter();
    let mut ordinal = 0;
    for (grouped_index, item) in stable_items.iter().enumerate() {
        if !matches!(item, GroupedListItem::Item(_)) {
            continue;
        }
        #[expect(
            clippy::expect_used,
            reason = "Canonical row keys and paired item/order construction form a bijection."
        )]
        let mut row = original[order.next().expect("every retained item has a row")]
            .take()
            .expect("each row is retained once");
        row.grouped_index = grouped_index;
        row.selectable_ordinal = row.eligibility.selectable.then(|| {
            let current = ordinal;
            ordinal += 1;
            current
        });
        rows.push(row);
    }
    *items = stable_items;
}

#[cfg(test)]
mod canonical_row_projection_tests {
    use super::*;

    fn explanation(id: &str, title: &str, selectable: bool) -> scripts::SearchResult {
        scripts::SearchResult::SpineProjection(crate::spine::SpineListRow {
            id: id.to_owned().into(),
            kind: crate::spine::SpineListRowKind::Hint,
            title: title.to_owned().into(),
            subtitle: None,
            meta: None,
            icon: None,
            badges: Vec::new(),
            score: 0,
            is_selectable: selectable,
            action_label: None,
            action: crate::spine::SpineListAction::Noop,
        })
    }

    fn projected(ids: &[(&str, &str)]) -> (Vec<GroupedListItem>, Vec<MainMenuRowProjection>) {
        let mut results = ids
            .iter()
            .map(|(id, title)| explanation(id, title, true))
            .collect();
        let mut items = vec![GroupedListItem::SectionHeader("Matches".into(), None)];
        items.extend((0..ids.len()).map(GroupedListItem::Item));
        let rows = project_main_menu_rows(&mut items, &mut results, None, "query").unwrap();
        (items, rows)
    }

    #[test]
    fn later_matches_preserve_displayed_positions_and_use_current_content() {
        let (mut initial_items, mut initial_rows) =
            projected(&[("first", "First"), ("second", "Second")]);
        initial_items.insert(0, GroupedListItem::ReservedSectionSlot);
        for row in &mut initial_rows {
            row.grouped_index += 1;
        }
        let (mut items, mut rows) =
            projected(&[("late", "Late"), ("first", "Updated"), ("second", "Second")]);
        let late_key = rows[0].stable_key.clone();
        let updated_fingerprint = rows[1].content_fingerprint.clone();
        preserve_displayed_main_menu_rows(&initial_items, &initial_rows, &mut items, &mut rows);
        assert_eq!(
            rows.iter()
                .map(|row| row.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec![
                initial_rows[0].stable_key.as_str(),
                initial_rows[1].stable_key.as_str(),
                late_key.as_str()
            ]
        );
        assert_eq!(rows[0].grouped_index, initial_rows[0].grouped_index);
        assert_eq!(rows[1].grouped_index, initial_rows[1].grouped_index);
        assert_eq!(rows[0].content_fingerprint, updated_fingerprint);
        assert_ne!(
            rows[0].content_fingerprint,
            initial_rows[0].content_fingerprint
        );
        assert_eq!(
            rows[0].subject,
            MainMenuRowSubject::SearchResult { flat_index: 1 }
        );
        assert_eq!(
            rows.iter()
                .map(|row| row.selectable_ordinal)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1), Some(2)]
        );

        let (mut next_items, mut next_rows) = projected(&[
            ("newest", "Newest"),
            ("late", "Late"),
            ("first", "Updated"),
            ("second", "Second"),
        ]);
        preserve_displayed_main_menu_rows(&items, &rows, &mut next_items, &mut next_rows);
        assert_eq!(
            &next_rows[..3],
            rows.as_slice()
                .iter()
                .map(|old| {
                    let mut row = old.clone();
                    row.subject = MainMenuRowSubject::SearchResult {
                        flat_index: match old.selectable_ordinal {
                            Some(0) => 2,
                            Some(1) => 3,
                            _ => 1,
                        },
                    };
                    row
                })
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(next_items.iter().filter(|item| matches!(item, GroupedListItem::SectionHeader(label, _) if label == "More results")).count(), 1);
    }

    #[test]
    fn disappeared_rows_are_not_retained_and_empty_history_uses_normal_ranking() {
        let (old_items, old_rows) = projected(&[("old", "Old")]);
        let (mut items, mut rows) = projected(&[("best", "Best"), ("next", "Next")]);
        let expected = rows.clone();
        preserve_displayed_main_menu_rows(&old_items, &old_rows, &mut items, &mut rows);
        assert_eq!(rows, expected);
        preserve_displayed_main_menu_rows(&[], &[], &mut items, &mut rows);
        assert_eq!(rows, expected);
    }

    #[test]
    fn headers_reserved_slots_and_invalid_indices_have_no_subject() {
        let mut grouped = vec![
            GroupedListItem::ReservedSectionSlot,
            GroupedListItem::SectionHeader("Results".into(), None),
            GroupedListItem::Item(7),
            GroupedListItem::Item(INLINE_CALCULATOR_RESULT_INDEX),
        ];
        let rows = project_main_menu_rows(&mut grouped, &mut Vec::new(), None, "query")
            .expect("inert chrome is a valid projection");
        assert!(rows.is_empty());
        assert!(
            matches!(grouped.as_slice(), [GroupedListItem::ReservedSectionSlot, GroupedListItem::SectionHeader(label, None), GroupedListItem::Item(7), GroupedListItem::Item(INLINE_CALCULATOR_RESULT_INDEX)] if label == "Results")
        );
    }

    #[test]
    fn placeholders_and_disabled_explanations_keep_distinct_eligibility() {
        let mut results = vec![
            explanation("pending", "Loading", false),
            explanation("disabled", "Unavailable", true),
        ];
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Results".into(), None),
            GroupedListItem::Item(0),
            GroupedListItem::ReservedSectionSlot,
            GroupedListItem::Item(1),
        ];
        let rows = project_main_menu_rows(&mut grouped, &mut results, None, "query").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].grouped_index, 1);
        assert_eq!(rows[0].selectable_ordinal, None);
        assert_eq!(
            rows[0].eligibility,
            crate::list_item::RowEligibility::inert()
        );
        assert_eq!(rows[1].grouped_index, 3);
        assert_eq!(rows[1].selectable_ordinal, Some(0));
        assert_eq!(
            rows[1].eligibility,
            crate::list_item::RowEligibility::disabled_explanation()
        );
    }

    #[test]
    fn calculator_commit_reserves_inert_chrome_and_selects_explicit_subject() {
        let mut grouped = vec![GroupedListItem::Item(INLINE_CALCULATOR_RESULT_INDEX)];
        let mut results = Vec::new();
        let calculator = crate::calculator::try_build("2 + 2").unwrap();
        crate::list_item::ensure_launcher_section_slot(&mut grouped);
        let rows =
            project_main_menu_rows(&mut grouped, &mut results, Some(&calculator), "2 + 2").unwrap();
        assert!(matches!(grouped[0], GroupedListItem::ReservedSectionSlot));
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.subject, MainMenuRowSubject::Calculator);
        assert_eq!(row.grouped_index, 1);
        assert_eq!(row.selectable_ordinal, Some(0));
        assert!(row.eligibility.selectable && row.eligibility.activatable);
        assert_eq!(
            rows.iter()
                .find(|row| row.eligibility.selectable)
                .map(|row| row.grouped_index),
            Some(1)
        );
        assert_eq!(
            rows.iter()
                .rfind(|row| row.eligibility.selectable)
                .map(|row| row.grouped_index),
            Some(1)
        );
    }

    #[test]
    fn identical_duplicates_share_one_subject_and_remap_flat_indices() {
        let first = explanation("first", "First", true);
        let mut results = vec![first.clone(), first, explanation("second", "Second", true)];
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
            GroupedListItem::Item(2),
        ];
        let rows = project_main_menu_rows(&mut grouped, &mut results, None, "query").unwrap();
        assert_eq!(results.len(), 2);
        assert!(matches!(
            grouped.as_slice(),
            [GroupedListItem::Item(0), GroupedListItem::Item(1)]
        ));
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[1].subject,
            MainMenuRowSubject::SearchResult { flat_index: 1 }
        );
        assert_eq!(rows[1].selectable_ordinal, Some(1));
    }

    #[test]
    fn conflicting_identity_rejects_without_mutating_inputs() {
        let mut grouped = vec![GroupedListItem::Item(0), GroupedListItem::Item(1)];
        let mut results = vec![
            explanation("same", "One", true),
            explanation("same", "Two", true),
        ];
        let original_results = results.clone();
        let result = project_main_menu_rows(&mut grouped, &mut results, None, "query");
        assert_eq!(result, Err("main_menu_row_identity_conflict".into()));
        assert!(matches!(
            grouped.as_slice(),
            [GroupedListItem::Item(0), GroupedListItem::Item(1)]
        ));
        assert_eq!(results.len(), original_results.len());
        for (actual, original) in results.iter().zip(&original_results) {
            let (
                scripts::SearchResult::SpineProjection(actual),
                scripts::SearchResult::SpineProjection(original),
            ) = (actual, original)
            else {
                panic!("conflicting input changed result kind");
            };
            assert_eq!(actual, original);
        }
    }
}
