use super::capture::active_object_selector_for_input;
use super::payload::CaptureObjectKind;
use super::trigger_picker_keys::InlinePickerKeyIntent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSelectorCandidate {
    pub kind: CaptureObjectKind,
    pub id: String,
    pub label: String,
    pub subtitle: String,
}

impl ObjectSelectorCandidate {
    pub fn token(&self) -> String {
        format!("@{}:{}", self.kind.as_str(), self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSelectorRow {
    pub id: String,
    pub kind: CaptureObjectKind,
    pub title: String,
    pub token: Option<String>,
    pub subtitle: Option<String>,
    pub badges: Vec<String>,
    pub replacement_range: (usize, usize),
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSelectorSnapshot {
    pub kind: CaptureObjectKind,
    pub query: String,
    pub active_range: (usize, usize),
    pub rows: Vec<ObjectSelectorRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectSelectorContext {
    pub candidates: Vec<ObjectSelectorCandidate>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuSyntaxObjectSelectorState {
    pub snapshot: Option<ObjectSelectorSnapshot>,
    pub selected_row_id: Option<String>,
    pub visible_start: usize,
}

impl MenuSyntaxObjectSelectorState {
    pub fn owns_main_list(&self) -> bool {
        self.snapshot.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectSelectorTransition {
    NoChange,
    Close,
    Open {
        snapshot: ObjectSelectorSnapshot,
        selected_row_id: Option<String>,
    },
    Update {
        snapshot: ObjectSelectorSnapshot,
        selected_row_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectSelectorIntentOutcome {
    Ignored,
    SelectionChanged { new_index: usize },
    ReplaceInput { text: String },
    Close,
}

pub fn build_object_selector_snapshot(
    input: &str,
    registered_targets: &[String],
    ctx: &ObjectSelectorContext,
) -> Option<ObjectSelectorSnapshot> {
    let selector = active_object_selector_for_input(input, registered_targets)?;
    let mut rows = ctx
        .candidates
        .iter()
        .filter(|candidate| candidate.kind == selector.kind)
        .filter(|candidate| candidate_matches(candidate, &selector.query))
        .take(12)
        .enumerate()
        .map(|(index, candidate)| {
            let token = candidate.token();
            ObjectSelectorRow {
                id: format!(
                    "object:{}:{}:{}",
                    candidate.kind.as_str(),
                    index,
                    candidate.id
                ),
                kind: candidate.kind,
                title: candidate.label.clone(),
                token: Some(token),
                subtitle: Some(candidate.subtitle.clone()),
                badges: vec![candidate.kind.as_str().to_string()],
                replacement_range: selector.range,
                enabled: true,
            }
        })
        .collect::<Vec<_>>();

    if rows.is_empty() {
        rows.push(ObjectSelectorRow {
            id: format!("object-empty:{}", selector.kind.as_str()),
            kind: selector.kind,
            title: format!("No {} matches", selector.kind.as_str()),
            token: None,
            subtitle: Some("Keep typing to narrow the object selector".to_string()),
            badges: vec![selector.kind.as_str().to_string()],
            replacement_range: selector.range,
            enabled: false,
        });
    }

    Some(ObjectSelectorSnapshot {
        kind: selector.kind,
        query: selector.query,
        active_range: selector.range,
        rows,
    })
}

pub fn plan_object_selector_transition(
    current: &MenuSyntaxObjectSelectorState,
    raw_filter: &str,
    registered_targets: &[String],
    ctx: &ObjectSelectorContext,
) -> ObjectSelectorTransition {
    let next_snapshot = build_object_selector_snapshot(raw_filter, registered_targets, ctx);
    match (&current.snapshot, next_snapshot) {
        (None, None) => ObjectSelectorTransition::NoChange,
        (Some(_), None) => ObjectSelectorTransition::Close,
        (None, Some(snapshot)) => {
            let selected_row_id = preserve_or_pick_first_enabled(&snapshot, None);
            ObjectSelectorTransition::Open {
                snapshot,
                selected_row_id,
            }
        }
        (Some(_), Some(snapshot)) => {
            let selected_row_id =
                preserve_or_pick_first_enabled(&snapshot, current.selected_row_id.as_deref());
            if current.snapshot.as_ref() == Some(&snapshot)
                && current.selected_row_id == selected_row_id
            {
                ObjectSelectorTransition::NoChange
            } else {
                ObjectSelectorTransition::Update {
                    snapshot,
                    selected_row_id,
                }
            }
        }
    }
}

pub fn apply_object_selector_intent(
    intent: InlinePickerKeyIntent,
    snapshot: &ObjectSelectorSnapshot,
    selected_index: Option<usize>,
    raw_filter_text: &str,
) -> ObjectSelectorIntentOutcome {
    match intent {
        InlinePickerKeyIntent::MoveUp => match prev_selectable_index(snapshot, selected_index) {
            Some(new_index) => ObjectSelectorIntentOutcome::SelectionChanged { new_index },
            None => ObjectSelectorIntentOutcome::Ignored,
        },
        InlinePickerKeyIntent::MoveDown => match next_selectable_index(snapshot, selected_index) {
            Some(new_index) => ObjectSelectorIntentOutcome::SelectionChanged { new_index },
            None => ObjectSelectorIntentOutcome::Ignored,
        },
        InlinePickerKeyIntent::MoveHome | InlinePickerKeyIntent::PageUp => {
            match first_selectable_index(snapshot) {
                Some(new_index) => ObjectSelectorIntentOutcome::SelectionChanged { new_index },
                None => ObjectSelectorIntentOutcome::Ignored,
            }
        }
        InlinePickerKeyIntent::MoveEnd | InlinePickerKeyIntent::PageDown => {
            match last_selectable_index(snapshot) {
                Some(new_index) => ObjectSelectorIntentOutcome::SelectionChanged { new_index },
                None => ObjectSelectorIntentOutcome::Ignored,
            }
        }
        InlinePickerKeyIntent::Close => ObjectSelectorIntentOutcome::Close,
        InlinePickerKeyIntent::Accept | InlinePickerKeyIntent::Apply => {
            let idx = selected_index.or_else(|| first_selectable_index(snapshot));
            let Some(row) = idx.and_then(|idx| snapshot.rows.get(idx)) else {
                return ObjectSelectorIntentOutcome::Ignored;
            };
            if !row.enabled {
                return ObjectSelectorIntentOutcome::Ignored;
            }
            let Some(token) = row.token.as_ref() else {
                return ObjectSelectorIntentOutcome::Ignored;
            };
            ObjectSelectorIntentOutcome::ReplaceInput {
                text: apply_object_ref_replacement(raw_filter_text, row.replacement_range, token),
            }
        }
        InlinePickerKeyIntent::SecondaryAction | InlinePickerKeyIntent::CreateAction => {
            ObjectSelectorIntentOutcome::Ignored
        }
    }
}

pub fn object_selector_visible_start_for_selection(
    visible_start: usize,
    selected_index: usize,
    item_count: usize,
) -> usize {
    crate::components::inline_dropdown::inline_dropdown_visible_range_from_start(
        visible_start,
        selected_index,
        item_count,
        crate::components::inline_popup_window::INLINE_POPUP_MAX_VISIBLE_ROWS,
    )
    .start
}

fn candidate_matches(candidate: &ObjectSelectorCandidate, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let query = query.to_ascii_lowercase();
    let mut haystack = String::new();
    haystack.push_str(&candidate.id);
    haystack.push(' ');
    haystack.push_str(&candidate.label);
    haystack.push(' ');
    haystack.push_str(&candidate.subtitle);
    haystack.to_ascii_lowercase().contains(&query)
}

pub fn object_selector_candidate_matches(candidate: &ObjectSelectorCandidate, query: &str) -> bool {
    candidate_matches(candidate, query)
}

pub fn object_selector_row_to_main_list_row(row: &ObjectSelectorRow) -> crate::spine::SpineListRow {
    let meta = row
        .token
        .as_ref()
        .map(|token| gpui::SharedString::from(token.clone()));

    crate::spine::SpineListRow {
        id: gpui::SharedString::from(format!("menu-syntax-object:{}", row.id)),
        kind: crate::spine::list::SpineListRowKind::CaptureTarget {
            target: gpui::SharedString::from(row.kind.as_str().to_string()),
        },
        title: gpui::SharedString::from(row.title.clone()),
        subtitle: row
            .subtitle
            .as_ref()
            .map(|subtitle| gpui::SharedString::from(subtitle.clone())),
        meta,
        icon: Some(gpui::SharedString::from("at-sign")),
        badges: row
            .badges
            .iter()
            .map(|badge| gpui::SharedString::from(badge.clone()))
            .collect(),
        score: if row.enabled { 0 } else { i32::MIN },
        is_selectable: row.enabled,
        action_label: Some(gpui::SharedString::from("Insert")),
        action: crate::spine::SpineListAction::AcceptMenuSyntaxObject {
            row_id: row.id.clone().into(),
        },
    }
}

fn preserve_or_pick_first_enabled(
    snapshot: &ObjectSelectorSnapshot,
    previous_id: Option<&str>,
) -> Option<String> {
    if let Some(prev) = previous_id {
        if let Some(row) = snapshot
            .rows
            .iter()
            .find(|row| row.id == prev && row.enabled)
        {
            return Some(row.id.clone());
        }
    }
    snapshot
        .rows
        .iter()
        .find(|row| row.enabled)
        .map(|row| row.id.clone())
}

fn first_selectable_index(snapshot: &ObjectSelectorSnapshot) -> Option<usize> {
    snapshot.rows.iter().position(|row| row.enabled)
}

fn last_selectable_index(snapshot: &ObjectSelectorSnapshot) -> Option<usize> {
    snapshot
        .rows
        .iter()
        .enumerate()
        .rfind(|(_, row)| row.enabled)
        .map(|(idx, _)| idx)
}

fn next_selectable_index(
    snapshot: &ObjectSelectorSnapshot,
    current: Option<usize>,
) -> Option<usize> {
    let first = first_selectable_index(snapshot)?;
    let last = last_selectable_index(snapshot)?;
    let start = match current {
        Some(idx) if idx < last => idx + 1,
        _ => first,
    };

    for idx in start..=snapshot.rows.len().saturating_sub(1) {
        if snapshot.rows.get(idx).is_some_and(|row| row.enabled) {
            return Some(idx);
        }
    }
    Some(first)
}

fn prev_selectable_index(
    snapshot: &ObjectSelectorSnapshot,
    current: Option<usize>,
) -> Option<usize> {
    let first = first_selectable_index(snapshot)?;
    let last = last_selectable_index(snapshot)?;
    let start = match current {
        Some(idx) if idx > first => idx - 1,
        _ => last,
    };

    for idx in (0..=start).rev() {
        if snapshot.rows.get(idx).is_some_and(|row| row.enabled) {
            return Some(idx);
        }
    }
    Some(last)
}

fn apply_object_ref_replacement(
    raw_filter_text: &str,
    range: (usize, usize),
    replacement: &str,
) -> String {
    let (start, end) = range;
    if start > end || end > raw_filter_text.len() {
        return raw_filter_text.to_string();
    }
    let mut out = String::with_capacity(raw_filter_text.len() + replacement.len());
    out.push_str(&raw_filter_text[..start]);
    out.push_str(replacement);
    out.push_str(&raw_filter_text[end..]);
    if out.len() == start + replacement.len() {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_selector_snapshot_opens_after_capture_body_boundary() {
        let ctx = ObjectSelectorContext {
            candidates: vec![ObjectSelectorCandidate {
                kind: CaptureObjectKind::Snippet,
                id: "fetch-json".to_string(),
                label: "fetch-json".to_string(),
                subtitle: "Snippet · ts".to_string(),
            }],
        };
        let snap = build_object_selector_snapshot(";snippet update @fetch", &[], &ctx)
            .expect("object selector snapshot");
        assert_eq!(snap.kind, CaptureObjectKind::Snippet);
        assert_eq!(snap.rows[0].token.as_deref(), Some("@snippet:fetch-json"));
    }

    #[test]
    fn object_selector_filters_candidates_by_query() {
        let ctx = ObjectSelectorContext {
            candidates: vec![
                ObjectSelectorCandidate {
                    kind: CaptureObjectKind::Snippet,
                    id: "fetch-json".to_string(),
                    label: "fetch-json".to_string(),
                    subtitle: "Snippet · ts".to_string(),
                },
                ObjectSelectorCandidate {
                    kind: CaptureObjectKind::Snippet,
                    id: "format-date".to_string(),
                    label: "format-date".to_string(),
                    subtitle: "Snippet · ts".to_string(),
                },
            ],
        };
        let snap = build_object_selector_snapshot(";snippet update @format", &[], &ctx)
            .expect("object selector snapshot");
        assert_eq!(snap.rows.len(), 1);
        assert_eq!(snap.rows[0].token.as_deref(), Some("@snippet:format-date"));
    }

    #[test]
    fn object_selector_row_ids_are_unique_when_candidates_share_ids() {
        let ctx = ObjectSelectorContext {
            candidates: vec![
                ObjectSelectorCandidate {
                    kind: CaptureObjectKind::Snippet,
                    id: "@gma".to_string(),
                    label: "email".to_string(),
                    subtitle: "Snippet · md".to_string(),
                },
                ObjectSelectorCandidate {
                    kind: CaptureObjectKind::Snippet,
                    id: "@gma".to_string(),
                    label: "john@example.com".to_string(),
                    subtitle: "Snippet · json".to_string(),
                },
            ],
        };
        let snap =
            build_object_selector_snapshot(";snippet update @", &[], &ctx).expect("snapshot");

        assert_eq!(snap.rows.len(), 2);
        assert_ne!(snap.rows[0].id, snap.rows[1].id);
        assert_eq!(snap.rows[0].token.as_deref(), Some("@snippet:@gma"));
        assert_eq!(snap.rows[1].token.as_deref(), Some("@snippet:@gma"));
        for row in &snap.rows {
            let projected = object_selector_row_to_main_list_row(row);
            assert_eq!(
                projected.id.as_ref(),
                format!("menu-syntax-object:{}", row.id)
            );
            assert_eq!(
                projected.action,
                crate::spine::SpineListAction::AcceptMenuSyntaxObject {
                    row_id: row.id.clone().into()
                }
            );
            assert!(crate::scripts::SearchResult::SpineProjection(projected)
                .command_descriptor()
                .unwrap()
                .can_execute());
        }
    }

    #[test]
    fn accept_object_ref_row_replaces_only_active_at_token() {
        let snapshot = ObjectSelectorSnapshot {
            kind: CaptureObjectKind::Snippet,
            query: "fetch".to_string(),
            active_range: (16, 22),
            rows: vec![ObjectSelectorRow {
                id: "object:snippet:fetch-json".to_string(),
                kind: CaptureObjectKind::Snippet,
                title: "fetch-json".to_string(),
                token: Some("@snippet:fetch-json".to_string()),
                subtitle: Some("Snippet".to_string()),
                badges: vec!["snippet".to_string()],
                replacement_range: (16, 22),
                enabled: true,
            }],
        };

        assert_eq!(
            apply_object_selector_intent(
                InlinePickerKeyIntent::Accept,
                &snapshot,
                Some(0),
                ";snippet update @fetch due:tom"
            ),
            ObjectSelectorIntentOutcome::ReplaceInput {
                text: ";snippet update @snippet:fetch-json due:tom".to_string(),
            }
        );
    }

    #[test]
    fn object_selector_does_not_open_for_resolved_ref_token() {
        let ctx = ObjectSelectorContext {
            candidates: vec![ObjectSelectorCandidate {
                kind: CaptureObjectKind::Snippet,
                id: "fetch-json".to_string(),
                label: "fetch-json".to_string(),
                subtitle: "Snippet".to_string(),
            }],
        };
        assert!(
            build_object_selector_snapshot(";snippet update @snippet:fetch-json", &[], &ctx)
                .is_none()
        );
    }

    #[test]
    fn object_selector_stops_scanning_after_body_delimiter() {
        let ctx = ObjectSelectorContext {
            candidates: vec![ObjectSelectorCandidate {
                kind: CaptureObjectKind::Snippet,
                id: "fetch-json".to_string(),
                label: "fetch-json".to_string(),
                subtitle: "Snippet".to_string(),
            }],
        };
        assert!(
            build_object_selector_snapshot(";snippet add trigger:fj -- @fetch", &[], &ctx)
                .is_none()
        );
    }
}
