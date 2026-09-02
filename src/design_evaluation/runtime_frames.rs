//! Search evidence retained at actual GPUI draw completion, never reconstructed
//! by repainting a query or by attaching current model state to previous pixels.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{ensure, Context as _, Result};
use gpui::{App, Bounds, Focusable, Pixels, Window};
use serde::Serialize;
use serde_json::{json, Value};

use super::runtime::{Evaluator, Mounted, RootOwner};
use crate::computer_use::gpui_runtime_bridge::{
    forget_owned_render_frame, publish_owned_render_frame, OwnedCompletedRenderFrame,
};
use crate::protocol::{
    AutomationTargetIdentitySnapshot, AutomationWindowTarget, CompletedFrameIdentity,
    OwnedFrameCursor, PixelProbe, PixelProbeResult, ScheduledFrameRequirement,
};

const SEARCH_FIXTURE: &str = "main-search-contract";
const MAX_COMPLETED_STAMPS: usize = 256;
const MAX_RETAINED_TRACE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Serialize)]
struct PaintBounds {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl From<Bounds<Pixels>> for PaintBounds {
    fn from(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: bounds.origin.x.as_f32(),
            y: bounds.origin.y.as_f32(),
            width: bounds.size.width.as_f32(),
            height: bounds.size.height.as_f32(),
        }
    }
}

impl PaintBounds {
    fn visible(self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }
    fn contains(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.width <= self.x + self.width
            && other.y + other.height <= self.y + self.height
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaintBinding {
    kind: &'static str,
    id: gpui::SharedString,
    bounds: PaintBounds,
    visible_bounds: PaintBounds,
    clip_bounds: PaintBounds,
    metadata: Rc<Value>,
}

impl From<&gpui::OwnedPaintBinding> for PaintBinding {
    fn from(binding: &gpui::OwnedPaintBinding) -> Self {
        Self {
            kind: binding.kind,
            id: binding.id.clone(),
            bounds: binding.bounds.into(),
            visible_bounds: binding.visible_bounds.into(),
            clip_bounds: binding.clip_bounds.into(),
            metadata: binding.metadata.clone(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaintPixelEvidence {
    kind: &'static str,
    semantic_id: String,
    bounds: PaintBounds,
    visible_bounds: PaintBounds,
    probe: PixelProbeResult,
    /// Calculator overlays are translucent: retain the real paint color, never
    /// pretend it is the expected composited framebuffer color.
    selected_surface_color: Option<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameStamp {
    frame: CompletedFrameIdentity,
    #[serde(skip)]
    accounted_bytes: usize,
    trace_generation: u64,
    mode: &'static str,
    invalidation_epoch: u64,
    notification_epoch: u64,
    notification_cause: Option<gpui::OwnedNotificationCause>,
    cause: &'static str,
    local_input_focused: bool,
    native_window_active: bool,
    native_window: Value,
    search: Rc<Value>,
    file_search: Rc<Value>,
    paint_bindings: Vec<PaintBinding>,
    paint_failures: Vec<&'static str>,
    pixel_evidence: Vec<PaintPixelEvidence>,
    pixel_evidence_complete: bool,
    pending_resources: u32,
    failed_resources: u32,
}

#[derive(Default)]
struct FrameTrace {
    generation: u64,
    retired_before_frame_generation: u64,
    latest_frame_generation: u64,
    notification_floor: u64,
    retained_bytes: usize,
    stamps: Vec<FrameStamp>,
    latest: Option<OwnedCompletedRenderFrame>,
    error: Option<String>,
    negative_readback: bool,
}

impl FrameTrace {
    /// Retire only history the driver has retained, keeping its exact capture
    /// baseline. Read cursors never imply acknowledgement or mutate this trace.
    fn acknowledge(&mut self, cursor: OwnedFrameCursor) -> Result<usize> {
        ensure!(self.error.is_none(), "completed_frame_trace_invalid");
        cursor
            .validate(
                self.generation,
                self.retired_before_frame_generation,
                self.latest_frame_generation,
            )
            .map_err(anyhow::Error::msg)?;
        let before = self.stamps.partition_point(|stamp| {
            stamp.frame.target.frame_generation < Some(cursor.after_frame_generation)
        });
        ensure!(
            self.stamps.get(before).is_some_and(|stamp| {
                stamp.frame.target.frame_generation == Some(cursor.after_frame_generation)
            }),
            "frame_cursor_unknown"
        );
        let released_bytes: usize = self.stamps[..before]
            .iter()
            .map(|stamp| stamp.accounted_bytes)
            .sum();
        ensure!(
            released_bytes <= self.retained_bytes,
            "completed_frame_trace_accounting_invalid"
        );
        self.stamps.drain(..before);
        self.retained_bytes -= released_bytes;
        self.retired_before_frame_generation = cursor.after_frame_generation;
        Ok(before)
    }
}

struct ObservedWindow {
    mounted: Mounted,
    trace: Rc<RefCell<FrameTrace>>,
}

#[derive(Default)]
pub(super) struct RuntimeFrames {
    windows: BTreeMap<String, ObservedWindow>,
}

/// A negative control must exercise the real readback without contributing
/// faulted pixels or scheduled-success stamps to the production trace.
pub(super) struct NegativeReadbackScope {
    trace: Rc<RefCell<FrameTrace>>,
}

impl Drop for NegativeReadbackScope {
    fn drop(&mut self) {
        self.trace.borrow_mut().negative_readback = false;
    }
}

impl crate::ScriptListApp {
    /// Pure current search state; getState owns its authoritative preflight separately.
    pub(crate) fn owned_search_observation(&self) -> Value {
        let rows = self.main_menu_committed_rows();
        let selected = self
            .resolved_main_menu_selected_subject()
            .map(|subject| match subject {
                crate::ResolvedMainMenuSelection::SearchResult { row, .. }
                | crate::ResolvedMainMenuSelection::Calculator { row, .. } => row,
            });
        let selection_intent = match self.main_menu_selection_intent() {
            crate::MainMenuSelectionIntent::AutomaticTop => json!({"kind":"automaticTop"}),
            crate::MainMenuSelectionIntent::AutomaticAnchor { stable_key } => json!({
                "kind":"automaticAnchor", "semanticId":rows.iter().find(|row| row.stable_key == *stable_key && row.eligibility.selectable).map(|row| row.semantic_id.as_str()),
            }),
            crate::MainMenuSelectionIntent::ExplicitAnchor { stable_key } => json!({
                "kind":"explicitAnchor", "semanticId":rows.iter().find(|row| row.stable_key == *stable_key && row.eligibility.selectable).map(|row| row.semantic_id.as_str()),
            }),
        };
        json!({
            "version":1,
            "query":self.root_search.query_stamp(), "computedQuery":self.root_search.computed_query_stamp(),
            "pending":!self.root_search.query_is_current(),
            "selectionArmed":!self.spine_empty_subsearch_selection_suppressed(),
            "rawInput":self.filter_text, "computedInput":self.computed_filter_text,
            "resultRevision":self.main_menu_result_revision(), "selectionRevision":self.main_menu_selection_revision(),
            "viewportRevision":self.main_menu_viewport_revision(), "selectionIntent":selection_intent,
            "viewportIntent":match self.main_menu_viewport_intent() {
                crate::MainMenuViewportIntent::FollowSelection => "followSelection",
                crate::MainMenuViewportIntent::UserControlled => "userControlled",
            },
            "reconciliationReason":self.main_menu_selection_cause(),
            "selectedSemanticId":selected.map(|row| row.semantic_id.as_str()),
            "selectedOrdinal":selected.and_then(|row| row.selectable_ordinal),
            "selectedIndex":selected.map(|row| row.grouped_index),
            "publication":self.main_menu_last_publication(), "publicationError":self.main_menu_last_publication_error(),
            "providers":self.root_search.provider_observation(),
            "dispatch":self.main_menu_dispatch_observation(),
            "previewWork":self.main_menu_preview_work_observations(),
            "selectionMarkerColor":self.theme.colors.accent.selected,
            "committedRows":rows.iter().map(|row| json!({
                "semanticId":row.semantic_id,"stableKey":row.stable_key,"contentFingerprint":row.content_fingerprint,
                "groupedIndex":row.grouped_index,"selectableOrdinal":row.selectable_ordinal,
                "subjectKind":match row.subject { crate::MainMenuRowSubject::SearchResult { .. } => "searchResult", crate::MainMenuRowSubject::Calculator => "calculator" },
                "selectable":row.eligibility.selectable,"activatable":row.eligibility.activatable,
                "ranking":self.main_menu_ranking_evidence(&row.stable_key),
            })).collect::<Vec<_>>(),
            "scroll":self.main_list_scroll_receipt(),
        })
    }

    /// Paint-time evidence keeps the complete preflight with the scene it describes.
    pub(crate) fn owned_search_frame_evidence(&self) -> Value {
        let mut evidence = self.owned_search_observation();
        evidence["preflight"] = json!(self.cached_main_window_preflight);
        evidence
    }

    pub(crate) fn owned_file_search_frame_evidence(&self) -> Option<Value> {
        let crate::AppView::FileSearchView {
            query,
            selected_index,
            presentation,
        } = &self.current_view
        else {
            return None;
        };
        let selection = Self::resolve_file_search_selection_projection(
            &self.file_search_display_indices,
            self.cached_file_results.len(),
            *selected_index,
        );
        let selected =
            selection.and_then(|selection| self.cached_file_results.get(selection.result_index));
        Some(json!({
            "preview":self.owned_file_search_preview_evidence(),"previewWork":self.main_menu_preview_work_observations(),
            "version":1,"query":query,"filterText":self.filter_text,"loading":self.file_search_loading,
            "queryStamp":self.root_search.query_stamp(),
            "selectionMode":match self.file_search_selection_mode { crate::FileSearchSelectionMode::AutoFirst => "AutoFirst", crate::FileSearchSelectionMode::UserLockedPath => "UserLockedPath" },
            "presentation":match presentation { crate::FileSearchPresentation::Full => "Full", crate::FileSearchPresentation::Mini => "Mini" },
            "selectedPath":selected.map(|file| file.path.as_str()),"selectedOrdinal":selection.map(|selection| selection.display_index),
            "selectedSemanticId":selection.map(|selection| format!("file-search-row:{}",selection.display_index)),
            "selectionMarkerColor":self.theme.colors.accent.selected,
            "rows":self.file_search_display_indices.iter().enumerate().filter_map(|(display_index, &result_index)| {
                self.cached_file_results.get(result_index).map(|file| json!({"semanticId":format!("file-search-row:{display_index}"),
                    "displayIndex":display_index,"resultIndex":result_index,"path":file.path,"name":file.name}))
            }).collect::<Vec<_>>(),
        }))
    }

    pub(crate) fn owned_file_search_preview_evidence(&self) -> Option<Value> {
        let request = self.file_search_preview_request()?;
        let load_state = match &self.file_search_preview_thumbnail {
            crate::FileSearchThumbnailPreviewState::Idle => "idle",
            crate::FileSearchThumbnailPreviewState::Loading { .. } => "loading",
            crate::FileSearchThumbnailPreviewState::Ready { .. } => "ready",
            crate::FileSearchThumbnailPreviewState::Unavailable { .. } => "failed",
        };
        Some(
            json!({"query":request.binding.query,"queryText":request.query_text,
            "stableKey":request.binding.stable_key,"contentFingerprint":request.binding.content_fingerprint,
            "workSequence":request.sequence,"loadState":load_state,"path":request.file.path,"contentHash":request.content_hash}),
        )
    }
}

fn current_search(mounted: &Mounted, cx: &App) -> Result<(Value, Value)> {
    let RootOwner::Main(entity) = &mounted.owner else {
        anyhow::bail!("scheduled_frame_main_owner_required");
    };
    let app = entity.read(cx);
    Ok(match &app.current_view {
        crate::AppView::ScriptList => (app.owned_search_frame_evidence(), Value::Null),
        crate::AppView::FileSearchView { .. } => (
            Value::Null,
            app.owned_file_search_frame_evidence()
                .context("file_search_frame_scope_missing")?,
        ),
        _ => (Value::Null, Value::Null),
    })
}

fn same_search_authority(painted: &Value, current: &Value) -> bool {
    if painted.is_null() || current.is_null() {
        return painted == current;
    }
    [
        "query",
        "computedQuery",
        "pending",
        "resultRevision",
        "selectionRevision",
        "viewportRevision",
        "selectionIntent",
        "viewportIntent",
        "selectedSemanticId",
        "selectedOrdinal",
        "publication",
        "committedRows",
    ]
    .iter()
    .all(|field| painted[*field] == current[*field])
}

fn same_file_search_authority(painted: &Value, current: &Value) -> bool {
    if painted.is_null() || current.is_null() {
        return painted == current;
    }
    [
        "query",
        "queryStamp",
        "filterText",
        "loading",
        "selectionMode",
        "presentation",
        "selectedPath",
        "selectedOrdinal",
        "rows",
        "preview",
    ]
    .iter()
    .all(|field| painted[*field] == current[*field])
}

fn paint_bindings(window: &Window) -> Result<(Rc<Value>, Rc<Value>, Vec<PaintBinding>)> {
    let bindings: Vec<_> = window
        .owned_paint_bindings()?
        .iter()
        .map(PaintBinding::from)
        .collect();
    let root = |kind, id: &str| -> Result<Rc<Value>> {
        let mut roots = bindings
            .iter()
            .filter(|binding| binding.kind == kind && binding.id.as_ref() == id);
        let payload = roots
            .next()
            .map(|binding| binding.metadata.clone())
            .unwrap_or_else(|| Rc::new(Value::Null));
        ensure!(roots.next().is_none(), "search_paint_binding_ambiguous");
        Ok(payload)
    };
    Ok((
        root("mainSearch", "main-search")?,
        root("fileSearch", "file-search")?,
        bindings,
    ))
}

fn check_paint_bindings(search: &Value, bindings: &[PaintBinding]) -> Vec<&'static str> {
    let mut failures = Vec::new();
    let Some(rows) = search["committedRows"].as_array() else {
        return failures;
    };
    let mut seen = std::collections::HashSet::new();
    for binding in bindings
        .iter()
        .filter(|binding| binding.kind == "mainSearchRow")
    {
        if !seen.insert(binding.id.as_ref()) {
            failures.push("painted_row_identity_ambiguous");
        }
        let Some(row) = rows
            .iter()
            .find(|row| row["semanticId"].as_str() == Some(binding.id.as_ref()))
        else {
            failures.push("painted_row_subject_unknown");
            continue;
        };
        if [
            "stableKey",
            "contentFingerprint",
            "groupedIndex",
            "selectableOrdinal",
            "subjectKind",
            "activatable",
        ]
        .iter()
        .any(|field| row[*field] != binding.metadata[*field])
        {
            failures.push("painted_row_content_action_mismatch");
        }
        let selected = search["selectedSemanticId"].as_str() == Some(binding.id.as_ref());
        if binding.metadata["selected"].as_bool() != Some(selected) {
            failures.push("painted_row_selection_mismatch");
        }
    }
    for binding in bindings
        .iter()
        .filter(|binding| binding.kind == "mainSearchPreview")
    {
        let row = rows
            .iter()
            .find(|row| row["semanticId"].as_str() == Some(binding.id.as_ref()));
        if search["selectedSemanticId"].as_str() != Some(binding.id.as_ref())
            || row.is_none_or(|row| {
                row["stableKey"] != binding.metadata["stableKey"]
                    || row["contentFingerprint"] != binding.metadata["contentFingerprint"]
            })
        {
            failures.push("painted_preview_subject_mismatch");
        }
    }
    failures.sort_unstable();
    failures.dedup();
    failures
}

fn check_file_paint_bindings(
    search: &Value,
    bindings: &[PaintBinding],
    failures: &mut Vec<&'static str>,
) {
    let Some(rows) = search["rows"].as_array() else {
        return;
    };
    let mut seen = std::collections::HashSet::new();
    for binding in bindings
        .iter()
        .filter(|binding| binding.kind == "fileSearchRow")
    {
        if !seen.insert(binding.id.as_ref()) {
            failures.push("painted_file_row_identity_ambiguous");
        }
        let Some(row) = rows
            .iter()
            .find(|row| row["semanticId"].as_str() == Some(binding.id.as_ref()))
        else {
            failures.push("painted_file_row_subject_unknown");
            continue;
        };
        if ["displayIndex", "resultIndex", "path", "name"]
            .iter()
            .any(|field| row[*field] != binding.metadata[*field])
        {
            failures.push("painted_file_row_subject_mismatch");
        }
        if binding.metadata["selected"].as_bool()
            != Some(search["selectedSemanticId"].as_str() == Some(binding.id.as_ref()))
        {
            failures.push("painted_file_row_selection_mismatch");
        }
    }
    for binding in bindings
        .iter()
        .filter(|binding| matches!(binding.kind, "fileSearchPreview" | "fileSearchPreviewImage"))
    {
        if search["preview"] != *binding.metadata
            || search["selectedPath"] != binding.metadata["path"]
        {
            failures.push("painted_file_preview_subject_mismatch");
        }
    }
}

fn sample_painted_selection(
    window: &Window,
    frame: &OwnedCompletedRenderFrame,
    search: &Value,
    bindings: &[PaintBinding],
    failures: &mut Vec<&'static str>,
) -> Result<Vec<PaintPixelEvidence>> {
    let mut samples = Vec::new();
    let viewport = PaintBounds::from(Bounds {
        origin: gpui::point(gpui::px(0.0), gpui::px(0.0)),
        size: window.viewport_size(),
    });
    for marker in window
        .debug_bounds_entries()
        .iter()
        .filter(|entry| entry.selector.ends_with(":selection-marker"))
    {
        let Some(id) = marker.selector.strip_suffix(":selection-marker") else {
            continue;
        };
        let Some(row) = bindings.iter().find(|binding| {
            matches!(binding.kind, "mainSearchRow" | "fileSearchRow") && binding.id.as_ref() == id
        }) else {
            continue;
        };
        let bounds = PaintBounds::from(marker.bounds);
        let visible_bounds = PaintBounds::from(marker.visible_bounds);
        if !visible_bounds.visible() {
            continue;
        }
        if search["selectedSemanticId"].as_str() != Some(id) {
            failures.push("unexpected_visible_selection_marker");
        }
        if !row.bounds.contains(bounds)
            || !PaintBounds::from(marker.clip_bounds).contains(visible_bounds)
            || !viewport.contains(visible_bounds)
        {
            failures.push("selection_marker_bounds_invalid");
            continue;
        }
        let probe = PixelProbe {
            x: ((visible_bounds.x + visible_bounds.width / 2.0) * frame.scale_factor).floor()
                as u32,
            y: ((visible_bounds.y + visible_bounds.height / 2.0) * frame.scale_factor).floor()
                as u32,
        };
        ensure!(samples.len() < 64, "pixel_probe_budget_exhausted");
        let sampled =
            crate::computer_use::owned_render_capture::sample_retained_pixel(frame, &probe)?;
        if visible_bounds.contains(bounds) {
            if let Some(color) = search["selectionMarkerColor"].as_u64() {
                if [sampled.r, sampled.g, sampled.b, sampled.a]
                    != [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255]
                {
                    failures.push("selected_marker_pixel_mismatch");
                }
            }
        }
        samples.push(PaintPixelEvidence {
            kind: "selectionMarker",
            semantic_id: id.into(),
            bounds,
            visible_bounds,
            probe: sampled,
            selected_surface_color: None,
        });
    }
    for calculator in bindings.iter().filter(|binding| {
        binding.kind == "mainSearchCalculator" && binding.metadata["selected"] == true
    }) {
        let bounds = calculator.bounds;
        if !calculator.visible_bounds.contains(bounds) || !viewport.contains(bounds) {
            continue;
        }
        let probe = PixelProbe {
            x: ((bounds.x + bounds.width - 3.0) * frame.scale_factor).floor() as u32,
            y: ((bounds.y + bounds.height / 2.0) * frame.scale_factor).floor() as u32,
        };
        ensure!(samples.len() < 64, "pixel_probe_budget_exhausted");
        let sampled =
            crate::computer_use::owned_render_capture::sample_retained_pixel(frame, &probe)?;
        samples.push(PaintPixelEvidence {
            kind: "calculatorSurface",
            semantic_id: calculator.id.to_string(),
            bounds,
            visible_bounds: calculator.visible_bounds,
            probe: sampled,
            selected_surface_color: calculator.metadata["selectedSurfaceColor"]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok()),
        });
    }
    for image in bindings
        .iter()
        .filter(|binding| binding.kind == "fileSearchPreviewImage")
    {
        let visible_bounds = image.visible_bounds;
        if !visible_bounds.visible() || !viewport.contains(visible_bounds) {
            continue;
        }
        let probe = PixelProbe {
            x: ((visible_bounds.x + visible_bounds.width / 2.0) * frame.scale_factor).floor()
                as u32,
            y: ((visible_bounds.y + visible_bounds.height / 2.0) * frame.scale_factor).floor()
                as u32,
        };
        ensure!(samples.len() < 64, "pixel_probe_budget_exhausted");
        let sampled =
            crate::computer_use::owned_render_capture::sample_retained_pixel(frame, &probe)?;
        samples.push(PaintPixelEvidence {
            kind: "fileSearchPreviewImage",
            semantic_id: image.id.to_string(),
            bounds: image.bounds,
            visible_bounds,
            probe: sampled,
            selected_surface_color: None,
        });
    }
    ensure!(samples.len() <= 64, "pixel_probe_budget_exhausted");
    if let Some(selected) = search["selectedSemanticId"].as_str() {
        if let Some(row) = bindings.iter().find(|binding| {
            matches!(binding.kind, "mainSearchRow" | "fileSearchRow")
                && binding.id.as_ref() == selected
                && binding.visible_bounds.contains(binding.bounds)
        }) {
            if row.metadata["subjectKind"] != "calculator"
                && !samples.iter().any(|sample| sample.semantic_id == selected)
            {
                failures.push("visible_selected_marker_missing");
            }
        }
    }
    failures.sort_unstable();
    failures.dedup();
    Ok(samples)
}

impl Evaluator {
    pub(super) fn isolate_negative_readback(
        &mut self,
        target: &AutomationWindowTarget,
    ) -> Result<Option<NegativeReadbackScope>> {
        self.observe_scheduled_frames()?;
        // Never erase or bypass a pre-existing production observation failure.
        self.check_scheduled_frames()?;
        let mounted = self.resolve(target)?;
        let Some(observed) = self.frames.windows.get(&mounted.info.id) else {
            return Ok(None);
        };
        let mut trace = observed.trace.borrow_mut();
        ensure!(
            !trace.negative_readback,
            "negative_readback_probe_already_active"
        );
        trace.latest = None;
        trace.negative_readback = true;
        Ok(Some(NegativeReadbackScope {
            trace: observed.trace.clone(),
        }))
    }

    pub(super) fn observe_scheduled_frames(&mut self) -> Result<()> {
        let retired: Vec<_> = self
            .frames
            .windows
            .iter()
            .filter(|(id, observed)| {
                !self.mounted.get(*id).is_some_and(|current| {
                    current.fixture_id == SEARCH_FIXTURE
                        && current.handle == observed.mounted.handle
                        && current.info.generation == observed.mounted.info.generation
                })
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in retired {
            if let Some(observed) = self.frames.windows.remove(&id) {
                forget_owned_render_frame(
                    &observed.mounted.info.id,
                    observed
                        .mounted
                        .info
                        .generation
                        .context("window_generation_missing")?,
                );
                let _ = observed
                    .mounted
                    .handle
                    .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                        window.clear_owned_frame_completion_observer()
                    });
            }
        }
        for mounted in self
            .mounted
            .values()
            .filter(|mounted| mounted.fixture_id == SEARCH_FIXTURE)
        {
            if self.frames.windows.contains_key(&mounted.info.id) {
                continue;
            }
            let RootOwner::Main(entity) = &mounted.owner else {
                anyhow::bail!("scheduled_frame_main_owner_required");
            };
            let root_entity = entity.entity_id();
            let trace = Rc::new(RefCell::new(FrameTrace::default()));
            let observed = trace.clone();
            let owner = mounted.clone();
            let runtime = self.bootstrap.identity.clone();
            let target = Self::instance(mounted)?;
            mounted.handle.update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                window.observe_owned_frame_completion(Some(root_entity), move |window, cx, completion| {
                    let mut trace = observed.borrow_mut();
                    if let Some(error) = &trace.error { anyhow::bail!("{error}"); }
                    trace.latest_frame_generation = completion.generation;
                    if trace.negative_readback {
                        // Notifications consumed by the negative draw cannot be
                        // claimed as the cause of a later production frame.
                        trace.notification_floor = trace.notification_floor.max(completion.observed_notification_epoch);
                        return Ok(());
                    }
                    let result = (|| -> Result<()> {
                        ensure!(trace.stamps.len() < MAX_COMPLETED_STAMPS, "completed_frame_trace_overflow");
                        let (search, file_search, bindings) = paint_bindings(window)?;
                        let bytes = window.owned_paint_binding_bytes().saturating_add(32 * 1024);
                        ensure!(trace.retained_bytes.saturating_add(bytes) <= MAX_RETAINED_TRACE_BYTES, "completed_frame_trace_bytes_exhausted");
                        let identity = Self::snapshot_for(&owner, window, cx)?;
                        ensure!(identity.frame_generation == Some(completion.generation), "completed_draw_stamp_mismatch");
                        if identity.app_view_variant == "ScriptList" { ensure!(!search.is_null(), "main_search_paint_binding_missing"); }
                        if identity.app_view_variant == "FileSearchView" { ensure!(!file_search.is_null(), "file_search_paint_binding_missing"); }
                        let frame = CompletedFrameIdentity { runtime: runtime.clone(), requested_target: target.clone(), target: identity, native_window_id: None };
                        let notification_epoch = completion.observed_notification_epoch;
                        let previous_notification = trace.stamps.last().map(|stamp| stamp.notification_epoch).unwrap_or(0).max(trace.notification_floor);
                        let resources = window.owned_render_resource_status();
                        let RootOwner::Main(entity) = &owner.owner else { anyhow::bail!("scheduled_frame_main_owner_required"); };
                        let local_input_focused = entity.read(cx).gpui_input_state.read(cx).focus_handle(cx).is_focused(window);
                        let mut stamp = FrameStamp {
                            frame: frame.clone(), trace_generation: trace.generation,
                            accounted_bytes: bytes,
                            mode: if completion.scheduled { "scheduled" } else { "forced" },
                            invalidation_epoch: completion.invalidation_epoch, notification_epoch,
                            notification_cause: completion.observed_notification_cause,
                            cause: if notification_epoch > previous_notification { "rootEntityNotify" } else { "windowInvalidation" },
                            local_input_focused, native_window_active: window.is_window_active(),
                            native_window: serde_json::to_value(crate::computer_use::owned_render_capture::observe_owned_native_window(window)?)?,
                            paint_failures: check_paint_bindings(search.as_ref(), &bindings), search, file_search, paint_bindings: bindings,
                            pixel_evidence: Vec::new(), pixel_evidence_complete: false,
                            pending_resources: resources.pending, failed_resources: resources.failed,
                        };
                        check_file_paint_bindings(stamp.file_search.as_ref(), &stamp.paint_bindings, &mut stamp.paint_failures);
                        trace.latest = None;
                        {
                            let size = window.viewport_size().to_device_pixels(window.scale_factor());
                            ensure!(size.width.0 > 0 && size.height.0 > 0
                                && size.width.0 as u64 * size.height.0 as u64 <= u64::from(crate::protocol::OWNED_EVALUATION_LIMITS.max_image_pixels), "frame_pixel_budget_exhausted");
                            let started = Instant::now();
                            let record = OwnedCompletedRenderFrame { identity: frame, image: window.render_to_image()?, scale_factor: window.scale_factor(),
                                phase_durations_ms: BTreeMap::from([("gpuReadback".into(), started.elapsed().as_secs_f64() * 1000.0)]) };
                            let scope = if stamp.file_search.is_null() { stamp.search.as_ref() } else { stamp.file_search.as_ref() };
                            stamp.pixel_evidence = sample_painted_selection(window, &record, scope, &stamp.paint_bindings, &mut stamp.paint_failures)?;
                            stamp.pixel_evidence_complete = true;
                            trace.latest = Some(record);
                        }
                        trace.retained_bytes += bytes;
                        trace.stamps.push(stamp);
                        Ok(())
                    })();
                    if let Err(error) = &result { trace.latest = None; trace.error = Some(error.to_string()); }
                    result
                })
            })??;
            self.frames.windows.insert(
                mounted.info.id.clone(),
                ObservedWindow {
                    mounted: mounted.clone(),
                    trace,
                },
            );
        }
        Ok(())
    }

    pub(super) fn reset_search_frame_trace(
        &mut self,
        target: &AutomationWindowTarget,
    ) -> Result<()> {
        self.observe_scheduled_frames()?;
        let mounted = self.resolve(target)?.clone();
        ensure!(
            mounted.fixture_id == SEARCH_FIXTURE,
            "scheduled_frame_capability_unavailable"
        );
        let observed = self
            .frames
            .windows
            .get(&mounted.info.id)
            .context("scheduled_frame_observer_missing")?;
        let completed = mounted
            .handle
            .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                window.rendered_frame_generation()
            })?;
        let mut trace = observed.trace.borrow_mut();
        let generation = trace
            .generation
            .checked_add(1)
            .context("frame_trace_generation_exhausted")?;
        let notification_floor = trace
            .stamps
            .last()
            .map(|stamp| stamp.notification_epoch)
            .unwrap_or(trace.notification_floor);
        *trace = FrameTrace {
            generation,
            retired_before_frame_generation: completed,
            latest_frame_generation: completed,
            notification_floor,
            ..Default::default()
        };
        forget_owned_render_frame(
            &mounted.info.id,
            mounted
                .info
                .generation
                .context("window_generation_missing")?,
        );
        Ok(())
    }

    /// Drop observer and retained-capture closures before the source owner is
    /// unmounted, so evidence does not artificially keep its weak callbacks live.
    pub(super) fn retire_search_frame_trace(
        &mut self,
        target: &AutomationWindowTarget,
    ) -> Result<()> {
        let mounted = self.resolve(target)?.clone();
        forget_owned_render_frame(
            &mounted.info.id,
            mounted
                .info
                .generation
                .context("window_generation_missing")?,
        );
        if let Some(observed) = self.frames.windows.remove(&mounted.info.id) {
            if observed.mounted.info.generation != mounted.info.generation {
                forget_owned_render_frame(
                    &observed.mounted.info.id,
                    observed
                        .mounted
                        .info
                        .generation
                        .context("window_generation_missing")?,
                );
            }
            let _ = observed
                .mounted
                .handle
                .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                    window.clear_owned_frame_completion_observer()
                });
        }
        Ok(())
    }

    pub(super) fn check_scheduled_frames(&self) -> Result<()> {
        for observed in self.frames.windows.values() {
            if let Some(error) = &observed.trace.borrow().error {
                anyhow::bail!("{error}");
            }
        }
        Ok(())
    }

    pub(super) fn take_observed_search_frame(
        &mut self,
        expected: &AutomationTargetIdentitySnapshot,
    ) -> Result<Option<OwnedCompletedRenderFrame>> {
        let Some(observed) = self.frames.windows.get(&expected.window_id) else {
            return Ok(None);
        };
        let mut trace = observed.trace.borrow_mut();
        ensure!(trace.error.is_none(), "completed_frame_trace_invalid");
        if trace.negative_readback {
            // Use completed_frame's ordinary direct native readback. Its real
            // injected failure is returned to the probe, not latched as a
            // production-observer failure or replaced with old retained pixels.
            return Ok(None);
        }
        let record = trace
            .latest
            .take()
            .context("completed_frame_readback_missing")?;
        ensure!(
            record.identity.target == *expected,
            "capture_frame_identity_stale"
        );
        Ok(Some(record))
    }

    pub(super) fn scheduled_completed_frame(
        &mut self,
        target: &AutomationWindowTarget,
        requirement: &ScheduledFrameRequirement,
    ) -> Result<(CompletedFrameIdentity, BTreeMap<String, f64>)> {
        self.validate_expected(target, &requirement.expected)?;
        let mounted = self.resolve(target)?.clone();
        ensure!(
            mounted.fixture_id == SEARCH_FIXTURE,
            "scheduled_frame_capability_unavailable"
        );
        self.observe_scheduled_frames()?;
        let trace = self
            .frames
            .windows
            .get(&mounted.info.id)
            .context("scheduled_frame_observer_missing")?
            .trace
            .clone();
        let baseline_publication = {
            let trace = trace.borrow();
            let baseline = trace
                .stamps
                .iter()
                .find(|stamp| {
                    stamp.frame.target.frame_generation == Some(requirement.after_frame_generation)
                })
                .context("scheduled_frame_baseline_unknown")?;
            ensure!(
                baseline.notification_epoch == requirement.after_notification_epoch,
                "scheduled_frame_baseline_mismatch"
            );
            baseline.search["publication"]["sequence"]
                .as_u64()
                .unwrap_or(0)
        };
        let started = Instant::now();
        let deadline = started + Duration::from_secs(5);
        let mut pumps = 0;
        loop {
            self.check_scheduled_frames()?;
            self.validate_expected(target, &requirement.expected)?;
            let current = self.identity(target)?;
            let (current_search, current_file_search) =
                current_search(&mounted, &self.cx.app.borrow())?;
            let candidate = {
                let mut trace = trace.borrow_mut();
                // Notification ownership belongs to the committed search scene,
                // not every later revision of the broader app state. A follow-up
                // scheduled draw may carry the same scene and notification while
                // input semantics advance data_generation. Keep that causal
                // witness separate from the exact current-frame fence below.
                let notified_scene = trace.stamps.last().is_some_and(|completed| {
                    trace.stamps.iter().any(|stamp| {
                        let publication = stamp.search["publication"]["sequence"]
                            .as_u64()
                            .unwrap_or(0);
                        let publication_notified = publication <= baseline_publication
                            || stamp.notification_cause.is_some_and(|cause| {
                                cause.kind == "mainSearchPublication"
                                    && cause.sequence == publication
                                    && cause.notification_epoch
                                        > requirement.after_notification_epoch
                            });
                        stamp.mode == "scheduled"
                            && stamp.cause == "rootEntityNotify"
                            && publication_notified
                            && stamp.notification_epoch > requirement.after_notification_epoch
                            && stamp.notification_epoch == completed.notification_epoch
                            && stamp
                                .notification_cause
                                .map(|cause| (cause.kind, cause.sequence, cause.notification_epoch))
                                == completed.notification_cause.map(|cause| {
                                    (cause.kind, cause.sequence, cause.notification_epoch)
                                })
                            && stamp
                                .frame
                                .target
                                .frame_generation
                                .is_some_and(|generation| {
                                    generation > requirement.after_frame_generation
                                })
                            && stamp.frame.target.target_generation == current.target_generation
                            && stamp.frame.target.surface_generation == current.surface_generation
                            && stamp.frame.target.presentation_revision
                                == current.presentation_revision
                            && stamp.frame.target.theme_revision == current.theme_revision
                            && same_search_authority(stamp.search.as_ref(), &current_search)
                            && same_file_search_authority(
                                stamp.file_search.as_ref(),
                                &current_file_search,
                            )
                    })
                });
                let matches = trace.stamps.last().is_some_and(|stamp| {
                    notified_scene
                        && stamp.mode == "scheduled"
                        && stamp.frame.target == current
                        && same_search_authority(stamp.search.as_ref(), &current_search)
                        && same_file_search_authority(
                            stamp.file_search.as_ref(),
                            &current_file_search,
                        )
                        && stamp.pending_resources == 0
                        && stamp.failed_resources == 0
                        && stamp
                            .frame
                            .target
                            .frame_generation
                            .is_some_and(|generation| {
                                generation > requirement.after_frame_generation
                            })
                        && stamp.notification_epoch > requirement.after_notification_epoch
                        && stamp.pixel_evidence_complete
                });
                if matches {
                    trace.latest.take()
                } else {
                    None
                }
            };
            if let Some(record) = candidate {
                let identity = record.identity.clone();
                let mut phases = record.phase_durations_ms.clone();
                phases.insert(
                    "scheduledWait".into(),
                    started.elapsed().as_secs_f64() * 1000.0,
                );
                let owner = mounted.clone();
                let current_identity = Rc::new(move |cx: &mut App| {
                    owner
                        .handle
                        .update(cx, |_, window, cx| Self::snapshot_for(&owner, window, cx))?
                });
                publish_owned_render_frame(
                    record,
                    current_identity,
                    &mut self.cx.app.borrow_mut(),
                )?;
                return Ok((identity, phases));
            }
            ensure!(
                Instant::now() < deadline && pumps < 32,
                "scheduled_frame_notification_missing"
            );
            self.tick(true)?;
            pumps += 1;
        }
    }

    pub(super) fn acknowledge_frames(
        &mut self,
        target: &AutomationWindowTarget,
        expected: &AutomationTargetIdentitySnapshot,
        cursor: OwnedFrameCursor,
    ) -> Result<Value> {
        self.validate_expected(target, expected)?;
        ensure!(
            expected
                .frame_generation
                .is_some_and(|generation| cursor.after_frame_generation <= generation),
            "invalid_frame_acknowledgement_expectation"
        );
        let mounted = self.resolve(target)?;
        let observed = self
            .frames
            .windows
            .get(&mounted.info.id)
            .context("scheduled_frame_capability_unavailable")?;
        let mut trace = observed.trace.borrow_mut();
        let retired_frames = trace.acknowledge(cursor)?;
        Ok(json!({
            "target": target,
            "expected": expected,
            "acknowledgedCursor": cursor,
            "retiredFrames": retired_frames,
            "retainedFrames": trace.stamps.len(),
            "retainedTraceBytes": trace.retained_bytes,
        }))
    }

    /// Admission-only check: no serialization, draw, clock, or cursor mutation.
    pub(super) fn validate_frame_cursor(
        &self,
        target: &AutomationWindowTarget,
        cursor: OwnedFrameCursor,
    ) -> Result<()> {
        let mounted = self.resolve(target)?;
        let observed = self
            .frames
            .windows
            .get(&mounted.info.id)
            .context("scheduled_frame_capability_unavailable")?;
        let trace = observed.trace.borrow();
        cursor
            .validate(
                trace.generation,
                trace.retired_before_frame_generation,
                trace.latest_frame_generation,
            )
            .map_err(anyhow::Error::msg)
    }

    pub(super) fn current_frame_cursor(
        &self,
        target: &AutomationWindowTarget,
        after_frame_generation: u64,
    ) -> Result<OwnedFrameCursor> {
        let mounted = self.resolve(target)?;
        let observed = self
            .frames
            .windows
            .get(&mounted.info.id)
            .context("scheduled_frame_capability_unavailable")?;
        let trace = observed.trace.borrow();
        let cursor = OwnedFrameCursor {
            trace_generation: trace.generation,
            after_frame_generation,
        };
        cursor
            .validate(
                trace.generation,
                trace.retired_before_frame_generation,
                trace.latest_frame_generation,
            )
            .map_err(anyhow::Error::msg)?;
        Ok(cursor)
    }

    pub(super) fn passive_frame_evidence(
        &self,
        target: &AutomationWindowTarget,
        cursor: Option<OwnedFrameCursor>,
    ) -> Result<Value> {
        let mounted = self.resolve(target)?;
        let Some(observed) = self.frames.windows.get(&mounted.info.id) else {
            ensure!(cursor.is_none(), "scheduled_frame_capability_unavailable");
            return Ok(json!({"scheduledCapability":false}));
        };
        let trace = observed.trace.borrow();
        if let Some(cursor) = cursor {
            cursor
                .validate(
                    trace.generation,
                    trace.retired_before_frame_generation,
                    trace.latest_frame_generation,
                )
                .map_err(anyhow::Error::msg)?;
        }
        let after = cursor.map(|cursor| cursor.after_frame_generation);
        let start = after.map_or(0, |after| {
            trace.stamps.partition_point(|stamp| {
                stamp
                    .frame
                    .target
                    .frame_generation
                    .is_some_and(|generation| generation <= after)
            })
        });
        Ok(
            json!({"scheduledCapability":true,"traceGeneration":trace.generation,
            "retiredBeforeFrameGeneration":trace.retired_before_frame_generation,"completedFrames":&trace.stamps[start..],
            "afterFrameGeneration":after,"latestFrameGeneration":trace.latest_frame_generation,
            "traceOverflow":trace.error.is_some(),"traceError":trace.error,"retainedTraceBytes":trace.retained_bytes,
            "maxCompletedStamps":MAX_COMPLETED_STAMPS,"maxRetainedTraceBytes":MAX_RETAINED_TRACE_BYTES}),
        )
    }

    pub(super) fn frame_evidence(
        &self,
        frame: &CompletedFrameIdentity,
        after: Option<u64>,
        cursor: Option<OwnedFrameCursor>,
    ) -> Result<Value> {
        let Some(observed) = self.frames.windows.get(&frame.target.window_id) else {
            ensure!(cursor.is_none(), "scheduled_frame_capability_unavailable");
            return Ok(json!({"mode":"forced","frame":frame,"scheduledCapability":false}));
        };
        let trace = observed.trace.borrow();
        ensure!(trace.error.is_none(), "completed_frame_trace_invalid");
        if let Some(cursor) = cursor {
            cursor
                .validate(
                    trace.generation,
                    trace.retired_before_frame_generation,
                    trace.latest_frame_generation,
                )
                .map_err(anyhow::Error::msg)?;
        }
        let after = cursor.map(|cursor| cursor.after_frame_generation).or(after);
        let stamp = trace
            .stamps
            .iter()
            .find(|stamp| stamp.frame == *frame)
            .context("completed_frame_stamp_unknown")?;
        let start = after.map_or(0, |after| {
            trace.stamps.partition_point(|stamp| {
                stamp
                    .frame
                    .target
                    .frame_generation
                    .is_some_and(|generation| generation <= after)
            })
        });
        let mut evidence = serde_json::to_value(stamp)?;
        let object = evidence
            .as_object_mut()
            .context("completed_frame_evidence_invalid")?;
        object.extend([
            (
                "completedFrames".into(),
                serde_json::to_value(&trace.stamps[start..])?,
            ),
            ("afterFrameGeneration".into(), json!(after)),
            (
                "latestFrameGeneration".into(),
                json!(trace.latest_frame_generation),
            ),
            ("traceOverflow".into(), json!(false)),
            ("maxCompletedStamps".into(), json!(MAX_COMPLETED_STAMPS)),
            (
                "maxRetainedTraceBytes".into(),
                json!(MAX_RETAINED_TRACE_BYTES),
            ),
            ("scheduledCapability".into(), json!(true)),
            ("transientPixelsRetained".into(), json!(true)),
            (
                "transientPixelEvidence".into(),
                json!("bounded-native-selection-samples; full latest framebuffer only"),
            ),
        ]);
        Ok(evidence)
    }
}
