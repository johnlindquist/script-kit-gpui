//! Pure observation logic: AX/CG correlation, native-tab grouping, and
//! search-visibility classification.
//!
//! Everything in this module operates on plain data so the rules are unit
//! testable without live AX. The correlation contract (locked):
//!
//! - Candidate CG rows for a TITLED AX row: identical PID, normalized exact
//!   title, maximum edge delta at most 12 points.
//! - Candidate CG rows for a TITLELESS `AXDialog`/`AXSheet`: identical PID,
//!   maximum edge delta at most 2 points.
//! - A native id is assigned only when exactly one candidate remains.
//! - Multiple AX rows mapping uniquely to one CG row become a native-tab
//!   group only when proven (equal parent key, fixture tab group, or shared
//!   unique CG row + bounds within 2 points + exactly one focused-or-main
//!   member). Anything else stays `Ambiguous` with no native id.
//! - Matched CG rows are consumed so they do not duplicate as CG-only rows.

use std::collections::HashMap;

use super::types::{Bounds, NativeIdConfidence, SearchVisibility};

const TITLED_EDGE_TOLERANCE: i32 = 12;
const TITLELESS_EDGE_TOLERANCE: i32 = 2;
const TAB_GROUP_BOUNDS_TOLERANCE: i32 = 2;

/// Plain AX row facts needed for correlation.
#[derive(Debug, Clone)]
pub(super) struct AxCorrelationRow {
    pub pid: i32,
    pub title: String,
    pub bounds: Bounds,
    pub role: Option<String>,
    pub focused: bool,
    pub main: bool,
    /// Opaque equality key for the AXParent element, when known.
    pub parent_key: Option<u64>,
    /// Fixture-declared native tab group, when running on the provider.
    pub declared_tab_group: Option<String>,
}

/// Plain CG row facts needed for correlation.
#[derive(Debug, Clone)]
pub(super) struct CgCorrelationRow {
    pub native_window_id: u32,
    pub pid: i32,
    pub title: String,
    pub bounds: Bounds,
}

/// Per-AX-row correlation verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AxCorrelationVerdict {
    pub native_window_id: Option<u32>,
    pub confidence: NativeIdConfidence,
    /// True for non-primary members of a proven native-tab group.
    pub internal_tab_member: bool,
}

/// Full correlation result.
#[derive(Debug, Clone)]
pub(super) struct CorrelationResult {
    /// One verdict per AX row, index-aligned with the input.
    pub verdicts: Vec<AxCorrelationVerdict>,
    /// Indices of CG rows consumed by AX matches (must not become CG-only rows).
    pub consumed_cg_indices: Vec<usize>,
}

fn max_edge_delta(a: Bounds, b: Bounds) -> i32 {
    let left = (a.x - b.x).abs();
    let top = (a.y - b.y).abs();
    let right = ((a.x + a.width as i32) - (b.x + b.width as i32)).abs();
    let bottom = ((a.y + a.height as i32) - (b.y + b.height as i32)).abs();
    left.max(top).max(right).max(bottom)
}

fn normalized_title(title: &str) -> &str {
    title.trim()
}

fn is_titleless_dialog_like(row: &AxCorrelationRow) -> bool {
    row.title.is_empty() && matches!(row.role.as_deref(), Some("AXDialog") | Some("AXSheet"))
}

fn candidate_tolerance(row: &AxCorrelationRow) -> Option<i32> {
    if !row.title.is_empty() {
        Some(TITLED_EDGE_TOLERANCE)
    } else if is_titleless_dialog_like(row) {
        Some(TITLELESS_EDGE_TOLERANCE)
    } else {
        None
    }
}

fn is_candidate(ax: &AxCorrelationRow, cg: &CgCorrelationRow) -> bool {
    if ax.pid != cg.pid {
        return false;
    }
    let Some(tolerance) = candidate_tolerance(ax) else {
        return false;
    };
    if !ax.title.is_empty() && normalized_title(&ax.title) != normalized_title(&cg.title) {
        return false;
    }
    max_edge_delta(ax.bounds, cg.bounds) <= tolerance
}

/// Select the primary member of a native-tab group:
/// focused member, then main member, then lowest original index.
pub(super) fn tab_group_primary(members: &[(usize, &AxCorrelationRow)]) -> usize {
    if let Some(&(index, _)) = members.iter().find(|(_, row)| row.focused) {
        return index;
    }
    if let Some(&(index, _)) = members.iter().find(|(_, row)| row.main) {
        return index;
    }
    members
        .iter()
        .map(|&(index, _)| index)
        .min()
        .expect("tab group must have members")
}

fn proven_tab_group(members: &[(usize, &AxCorrelationRow)]) -> bool {
    if members.len() < 2 {
        return false;
    }
    // Proof 1: all AXParent keys present and equal.
    let parents: Vec<Option<u64>> = members.iter().map(|(_, row)| row.parent_key).collect();
    if parents.iter().all(|parent| parent.is_some()) {
        let first = parents[0];
        if parents.iter().all(|parent| *parent == first) {
            return true;
        }
    }
    // Proof 2: deterministic fixture supplies the same tab group.
    let groups: Vec<&Option<String>> = members
        .iter()
        .map(|(_, row)| &row.declared_tab_group)
        .collect();
    if groups.iter().all(|group| group.is_some()) {
        let first = groups[0];
        if groups.iter().all(|group| *group == first) {
            return true;
        }
    }
    // Proof 3: shared unique CG row (given), exact bounds within 2 points,
    // and exactly one focused-or-main member.
    let first_bounds = members[0].1.bounds;
    let bounds_agree = members
        .iter()
        .all(|(_, row)| max_edge_delta(row.bounds, first_bounds) <= TAB_GROUP_BOUNDS_TOLERANCE);
    let focused_or_main = members
        .iter()
        .filter(|(_, row)| row.focused || row.main)
        .count();
    bounds_agree && focused_or_main == 1
}

/// Correlate AX rows against CG rows.
pub(super) fn correlate(
    ax_rows: &[AxCorrelationRow],
    cg_rows: &[CgCorrelationRow],
) -> CorrelationResult {
    // Build candidate sets.
    let mut candidates_per_ax: Vec<Vec<usize>> = ax_rows
        .iter()
        .map(|ax| {
            cg_rows
                .iter()
                .enumerate()
                .filter(|(_, cg)| is_candidate(ax, cg))
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        })
        .collect();

    // CG rows claimed by AX rows that have exactly one candidate.
    // ax_rows_by_unique_cg: cg index -> list of ax indices claiming it uniquely.
    let mut ax_by_unique_cg: HashMap<usize, Vec<usize>> = HashMap::new();
    for (ax_index, candidates) in candidates_per_ax.iter().enumerate() {
        if candidates.len() == 1 {
            ax_by_unique_cg
                .entry(candidates[0])
                .or_default()
                .push(ax_index);
        }
    }

    let mut verdicts = vec![
        AxCorrelationVerdict {
            native_window_id: None,
            confidence: NativeIdConfidence::Unavailable,
            internal_tab_member: false,
        };
        ax_rows.len()
    ];
    let mut consumed: Vec<usize> = Vec::new();

    for (cg_index, ax_indices) in &ax_by_unique_cg {
        let cg = &cg_rows[*cg_index];
        match ax_indices.as_slice() {
            [single] => {
                verdicts[*single] = AxCorrelationVerdict {
                    native_window_id: Some(cg.native_window_id),
                    confidence: NativeIdConfidence::UniquePublicCorrelation,
                    internal_tab_member: false,
                };
                consumed.push(*cg_index);
            }
            many => {
                let members: Vec<(usize, &AxCorrelationRow)> =
                    many.iter().map(|&index| (index, &ax_rows[index])).collect();
                if proven_tab_group(&members) {
                    let primary = tab_group_primary(&members);
                    for &(index, _) in &members {
                        verdicts[index] = AxCorrelationVerdict {
                            native_window_id: Some(cg.native_window_id),
                            confidence: NativeIdConfidence::NativeTabGroup,
                            internal_tab_member: index != primary,
                        };
                    }
                } else {
                    for &(index, _) in &members {
                        verdicts[index] = AxCorrelationVerdict {
                            native_window_id: None,
                            confidence: NativeIdConfidence::Ambiguous,
                            internal_tab_member: false,
                        };
                    }
                }
                // Consume the CG row either way to avoid duplicate rows.
                consumed.push(*cg_index);
            }
        }
    }

    // AX rows with multiple candidates are ambiguous; they assign no native
    // id, but they consume nothing (each CG candidate may still be valid
    // elsewhere or become a CG-only row deduped by the bounds fallback).
    for (ax_index, candidates) in candidates_per_ax.iter_mut().enumerate() {
        if candidates.len() > 1 {
            verdicts[ax_index] = AxCorrelationVerdict {
                native_window_id: None,
                confidence: NativeIdConfidence::Ambiguous,
                internal_tab_member: false,
            };
        }
    }

    consumed.sort_unstable();
    consumed.dedup();
    CorrelationResult {
        verdicts,
        consumed_cg_indices: consumed,
    }
}

/// Classify a window's ordinary-search visibility.
pub(super) fn classify_visibility(
    role: Option<&str>,
    title: &str,
    bounds: Bounds,
) -> SearchVisibility {
    // Untitled dialogs/sheets are observable but internal-only.
    if title.is_empty() && matches!(role, Some("AXDialog") | Some("AXSheet")) {
        return SearchVisibility::InternalOnly;
    }
    // Small/transient utility rows stay internal.
    if bounds.width < 50 || bounds.height < 50 {
        return SearchVisibility::InternalOnly;
    }
    // Titled sheets attached to a host remain internal (they cannot be
    // independently placed); titled dialogs are ordinary.
    if matches!(role, Some("AXSheet")) {
        return SearchVisibility::InternalOnly;
    }
    SearchVisibility::Ordinary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ax(pid: i32, title: &str, bounds: Bounds) -> AxCorrelationRow {
        AxCorrelationRow {
            pid,
            title: title.to_string(),
            bounds,
            role: Some("AXWindow".to_string()),
            focused: false,
            main: false,
            parent_key: None,
            declared_tab_group: None,
        }
    }

    fn cg(native: u32, pid: i32, title: &str, bounds: Bounds) -> CgCorrelationRow {
        CgCorrelationRow {
            native_window_id: native,
            pid,
            title: title.to_string(),
            bounds,
        }
    }

    #[test]
    fn one_ax_row_with_one_matching_cg_row_merges_exactly_once() {
        let bounds = Bounds::new(10, 10, 800, 600);
        let result = correlate(
            &[ax(5, "Doc", bounds)],
            &[cg(900, 5, "Doc", Bounds::new(12, 10, 800, 600))],
        );
        assert_eq!(result.verdicts[0].native_window_id, Some(900));
        assert_eq!(
            result.verdicts[0].confidence,
            NativeIdConfidence::UniquePublicCorrelation
        );
        assert_eq!(result.consumed_cg_indices, vec![0]);
    }

    #[test]
    fn same_title_same_bounds_different_native_ids_stay_distinct() {
        let bounds = Bounds::new(100, 700, 500, 300);
        let ax_rows = [ax(5, "Twin", bounds), ax(5, "Twin", bounds)];
        let cg_rows = [cg(9105, 5, "Twin", bounds), cg(9106, 5, "Twin", bounds)];
        let result = correlate(&ax_rows, &cg_rows);
        // Two candidates each: ambiguous, no native ids, nothing merged wrongly.
        for verdict in &result.verdicts {
            assert_eq!(verdict.native_window_id, None);
            assert_eq!(verdict.confidence, NativeIdConfidence::Ambiguous);
        }
    }

    #[test]
    fn pid_mismatch_is_never_a_candidate() {
        let bounds = Bounds::new(0, 0, 800, 600);
        let result = correlate(&[ax(5, "Doc", bounds)], &[cg(900, 6, "Doc", bounds)]);
        assert_eq!(result.verdicts[0].native_window_id, None);
        assert_eq!(
            result.verdicts[0].confidence,
            NativeIdConfidence::Unavailable
        );
        assert!(result.consumed_cg_indices.is_empty());
    }

    #[test]
    fn edge_delta_beyond_tolerance_disqualifies_titled_candidates() {
        let result = correlate(
            &[ax(5, "Doc", Bounds::new(0, 0, 800, 600))],
            &[cg(900, 5, "Doc", Bounds::new(20, 0, 800, 600))],
        );
        assert_eq!(result.verdicts[0].native_window_id, None);
    }

    #[test]
    fn titleless_dialog_correlates_with_two_point_tolerance() {
        let mut dialog = ax(5, "", Bounds::new(200, 200, 420, 260));
        dialog.role = Some("AXDialog".to_string());
        let result = correlate(
            &[dialog.clone()],
            &[cg(901, 5, "", Bounds::new(201, 200, 420, 260))],
        );
        assert_eq!(result.verdicts[0].native_window_id, Some(901));

        let result = correlate(
            &[dialog],
            &[cg(901, 5, "", Bounds::new(204, 200, 420, 260))],
        );
        assert_eq!(result.verdicts[0].native_window_id, None);
    }

    #[test]
    fn native_tab_fixture_produces_one_movable_group_with_primary() {
        let bounds = Bounds::new(700, 700, 800, 500);
        let mut tab_one = ax(5, "Tab One", bounds);
        tab_one.focused = true;
        tab_one.declared_tab_group = Some("group-a".to_string());
        let mut tab_two = ax(5, "Tab Two", bounds);
        tab_two.declared_tab_group = Some("group-a".to_string());
        // Both tabs claim the SAME unique CG row (titles differ, but the CG
        // row carries the active tab's title).
        let cg_rows = [cg(9107, 5, "Tab One", bounds)];
        // Tab Two's title doesn't match the CG title, so only Tab One is a
        // titled candidate. Simulate the shared-CG case via equal parents.
        let mut tab_two_parented = tab_two.clone();
        tab_two_parented.title = "Tab One".to_string(); // same CG title snapshot
        tab_two_parented.parent_key = Some(77);
        let mut tab_one_parented = tab_one.clone();
        tab_one_parented.parent_key = Some(77);
        let result = correlate(&[tab_one_parented, tab_two_parented], &cg_rows);
        assert_eq!(
            result.verdicts[0].confidence,
            NativeIdConfidence::NativeTabGroup
        );
        assert_eq!(
            result.verdicts[1].confidence,
            NativeIdConfidence::NativeTabGroup
        );
        assert_eq!(result.verdicts[0].native_window_id, Some(9107));
        assert_eq!(result.verdicts[1].native_window_id, Some(9107));
        // Focused member is primary; the other is an internal tab member.
        assert!(!result.verdicts[0].internal_tab_member);
        assert!(result.verdicts[1].internal_tab_member);
        assert_eq!(result.consumed_cg_indices, vec![0]);
    }

    #[test]
    fn unproven_multi_ax_cluster_stays_ambiguous_and_consumes_cg() {
        let bounds = Bounds::new(0, 0, 800, 600);
        // Two focused/main-less rows sharing one CG row without parent proof:
        // bounds agree but zero focused-or-main members -> not proven.
        let rows = [ax(5, "Same", bounds), ax(5, "Same", bounds)];
        let result = correlate(&rows, &[cg(900, 5, "Same", bounds)]);
        for verdict in &result.verdicts {
            assert_eq!(verdict.confidence, NativeIdConfidence::Ambiguous);
            assert_eq!(verdict.native_window_id, None);
        }
        assert_eq!(result.consumed_cg_indices, vec![0]);
    }

    #[test]
    fn tab_group_primary_prefers_focused_then_main_then_lowest_index() {
        let bounds = Bounds::new(0, 0, 800, 600);
        let mut a = ax(5, "A", bounds);
        let mut b = ax(5, "B", bounds);
        let c = ax(5, "C", bounds);

        b.main = true;
        let members: Vec<(usize, &AxCorrelationRow)> = vec![(0, &a), (1, &b), (2, &c)];
        assert_eq!(tab_group_primary(&members), 1);

        a.focused = true;
        let members: Vec<(usize, &AxCorrelationRow)> = vec![(0, &a), (1, &b), (2, &c)];
        assert_eq!(tab_group_primary(&members), 0);

        let plain_a = ax(5, "A", bounds);
        let plain_b = ax(5, "B", bounds);
        let members: Vec<(usize, &AxCorrelationRow)> = vec![(1, &plain_b), (0, &plain_a)];
        assert_eq!(tab_group_primary(&members), 0);
    }

    #[test]
    fn untitled_dialog_is_internal_only_but_ordinary_windows_are_not() {
        assert_eq!(
            classify_visibility(Some("AXDialog"), "", Bounds::new(0, 0, 400, 300)),
            SearchVisibility::InternalOnly
        );
        assert_eq!(
            classify_visibility(Some("AXSheet"), "Save", Bounds::new(0, 0, 400, 300)),
            SearchVisibility::InternalOnly
        );
        assert_eq!(
            classify_visibility(Some("AXWindow"), "Doc", Bounds::new(0, 0, 400, 300)),
            SearchVisibility::Ordinary
        );
        assert_eq!(
            classify_visibility(Some("AXWindow"), "Tiny", Bounds::new(0, 0, 30, 30)),
            SearchVisibility::InternalOnly
        );
    }
}
