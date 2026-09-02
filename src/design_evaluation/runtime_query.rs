use super::conversation_fixtures;
use super::runtime::{Evaluator, Mounted, RootOwner};
use crate::{
    protocol::{AutomationWindowTarget, CompletedFrameIdentity},
    AppView,
};
use anyhow::{ensure, Context as _, Result};
use serde_json::{json, Value};

pub(super) const CACHE_READINESS_SOURCES: &[&str] = &[
    "tabs",
    "files",
    "directory",
    "history",
    "notes",
    "todos",
    "clipboard",
    "dictation",
    "conversations",
    "windows",
];

#[derive(Clone, Copy)]
pub(super) enum QueryMode<'a> {
    Progress,
    CompletedFrame(&'a CompletedFrameIdentity),
}

impl Evaluator {
    pub(super) fn query(
        &mut self,
        request_id: &str,
        raw: &Value,
        mode: QueryMode<'_>,
    ) -> Result<Value> {
        let operation = raw["type"].as_str().context("query_type_required")?;
        let frame_cursor =
            crate::protocol::parse_owned_frame_cursor(raw).map_err(anyhow::Error::msg)?;
        let allow_progress = matches!(mode, QueryMode::Progress)
            && !matches!(operation, "getState" | "getElements" | "getLayoutInfo");
        ensure!(
            allow_progress || matches!(operation, "getState" | "getElements" | "getLayoutInfo"),
            "atomic_query_operation_not_permitted"
        );
        if operation == "listAutomationWindows" {
            let windows: Vec<_> = self
                .mounted
                .values()
                .filter_map(|mounted| {
                    let target = Self::instance(mounted).ok()?;
                    self.resolve(&target).ok()?;
                    crate::windows::automation_surface_collector::current_surface_metadata(
                        &mounted.info,
                    )
                })
                .collect();
            return serde_json::to_value(crate::protocol::Message::automation_window_list_result(
                request_id.into(),
                windows,
                None,
            ))
            .map_err(Into::into);
        }
        if operation == "getLogs" {
            let crate::protocol::Message::GetLogs {
                limit,
                level,
                target,
                contains,
                ..
            } = serde_json::from_value(raw.clone())?
            else {
                anyhow::bail!("invalid_log_request");
            };
            let (entries, matched) = crate::logging::query_log_ring(
                limit.unwrap_or(100).min(500),
                level.as_deref(),
                target.as_deref(),
                contains.as_deref(),
            );
            let entries = entries
                .into_iter()
                .map(serde_json::to_value)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            return serde_json::to_value(crate::protocol::Message::LogsResult {
                request_id: request_id.into(),
                entries,
                matched,
                capacity: crate::logging::LOG_RING_CAPACITY,
            })
            .map_err(Into::into);
        }
        let target: AutomationWindowTarget = serde_json::from_value(raw["target"].clone())?;
        let mut mounted = self.resolve(&target)?.clone();
        mounted.info =
            crate::windows::automation_surface_collector::current_surface_metadata(&mounted.info)
                .context("surface_lifetime_missing")?;
        if let Some(expected) = raw.get("expected") {
            self.validate_expected(&target, &serde_json::from_value(expected.clone())?)?;
        }
        if let QueryMode::CompletedFrame(frame) = mode {
            ensure!(
                self.identity(&target)? == frame.target,
                "capture_frame_identity_stale"
            );
            crate::computer_use::owned_render_capture::with_owned_completed_frame(
                frame,
                &mut self.cx.app.borrow_mut(),
                |_, _| Ok(()),
            )?;
        }
        // Validate the cursor before forwarding even an otherwise read-only
        // query. Retaining a delta never pumps work or changes native history.
        let frame_evidence = if operation == "getState" {
            self.passive_frame_evidence(&target, frame_cursor)?
        } else {
            Value::Null
        };
        let mut reply = match operation {
            "getState" if !matches!(mounted.owner, RootOwner::Main(_)) => {
                self.secondary_state(request_id, &mounted)?
            }
            "getState" => {
                let mut state = if frame_cursor.is_some() {
                    let mut ordinary = raw.clone();
                    ordinary
                        .as_object_mut()
                        .context("query_object_required")?
                        .remove("frameCursor");
                    self.forward_main(request_id, &ordinary, allow_progress)?
                } else {
                    self.forward_main(request_id, raw, allow_progress)?
                };
                state["mainOverlay"] = self.main_overlay_state(&mounted)?;
                state["searchProviders"] = self
                    .search_gate()
                    .map(|gate| gate.observation())
                    .unwrap_or(Value::Null);
                state
            }
            "getAgentChatState" => {
                let view = self.agent_chat_owner(&mounted)?;
                let cx = self.cx.app.borrow();
                let state = view.read(&cx).collect_agent_chat_state_snapshot(&cx);
                serde_json::to_value(crate::protocol::Message::agent_chat_state_result(
                    request_id.into(),
                    state,
                ))?
            }
            "getElements" if !matches!(mounted.owner, RootOwner::Main(_)) => {
                let limit = raw
                    .get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(200)
                    .min(500) as usize;
                let cx = self.cx.app.borrow();
                if matches!(mounted.owner, RootOwner::Footer) {
                    let mut elements = crate::footer_popup::footer_fixture_elements(
                        &mounted.info.id,
                        mounted
                            .info
                            .generation
                            .context("footer_generation_missing")?,
                        &cx,
                    )?;
                    let total = elements.len();
                    elements.truncate(limit);
                    json!({"type":"elementsResult","requestId":request_id,"elements":elements,"totalCount":total,
                        "window":mounted.info,"semanticQuality":"full","projectionQuality":"complete"})
                } else if matches!(mounted.owner, RootOwner::ShortcutRecorder) {
                    let mut elements = crate::shortcut_recorder::shortcut_fixture_elements(
                        &mounted.info.id,
                        mounted
                            .info
                            .generation
                            .context("recorder_generation_missing")?,
                        &cx,
                    )?;
                    let total = elements.len();
                    elements.truncate(limit);
                    json!({"type":"elementsResult","requestId":request_id,"elements":elements,"totalCount":total,
                        "window":mounted.info,"semanticQuality":"full","projectionQuality":"complete"})
                } else {
                    let snapshot =
                        crate::windows::automation_surface_collector::collect_surface_snapshot(
                            &mounted.info,
                            limit,
                            &cx,
                        )
                        .context("surface_owner_missing")?;
                    ensure!(
                        snapshot.quality
                            == crate::windows::automation_surface_collector::SnapshotQuality::Full,
                        "surface_projection_incomplete"
                    );
                    json!({"type":"elementsResult","requestId":request_id,"elements":snapshot.elements,
                        "totalCount":snapshot.total_count,"focusedSemanticId":snapshot.focused_semantic_id,
                        "selectedSemanticId":snapshot.selected_semantic_id,"warnings":snapshot.warnings,
                        "window":mounted.info,"semanticQuality":"full","projectionQuality":snapshot.quality.projection_quality()})
                }
            }
            "getLayoutInfo" if matches!(mounted.owner, RootOwner::ShortcutRecorder) => {
                let info = crate::shortcut_recorder::shortcut_fixture_layout(
                    &mounted.info.id,
                    mounted
                        .info
                        .generation
                        .context("recorder_generation_missing")?,
                    &mut self.cx.app.borrow_mut(),
                )?;
                serde_json::to_value(crate::protocol::Message::layout_info_result(
                    request_id.into(),
                    info,
                ))?
            }
            "getLayoutInfo" if matches!(mounted.owner, RootOwner::Footer) => {
                let info = crate::footer_popup::footer_fixture_layout(
                    &mounted.info.id,
                    mounted
                        .info
                        .generation
                        .context("footer_generation_missing")?,
                    &mut self.cx.app.borrow_mut(),
                )?;
                serde_json::to_value(crate::protocol::Message::layout_info_result(
                    request_id.into(),
                    info,
                ))?
            }
            "getLayoutInfo" if !matches!(mounted.owner, RootOwner::Main(_)) => {
                let info = match mode {
                    QueryMode::Progress => crate::windows::automation_surface_collector::collect_registered_surface_layout(
                        &mounted.info, &mut self.cx.app.borrow_mut())?,
                    QueryMode::CompletedFrame(frame) => crate::windows::automation_surface_collector::collect_registered_surface_layout_for_completed_frame(
                        &mounted.info, frame, &mut self.cx.app.borrow_mut())?,
                };
                serde_json::to_value(crate::protocol::Message::layout_info_result(
                    request_id.into(),
                    info,
                ))?
            }
            "waitFor" => self.wait_registered(request_id, &mounted, raw)?,
            "getElements" => {
                let mut reply = self.forward_main(request_id, raw, allow_progress)?;
                self.append_main_overlay_elements(&mounted, raw, &mut reply)?;
                reply
            }
            "getLayoutInfo" => self.forward_main(request_id, raw, allow_progress)?,
            _ => anyhow::bail!("owned_query_not_permitted"),
        };
        if self.resolve(&target).is_ok() {
            reply["targetIdentity"] = serde_json::to_value(self.identity(&target)?)?;
            if operation == "getState" {
                let theme = crate::theme::get_theme_snapshot();
                reply["liveTheme"] =
                    json!({"revision": theme.revision, "resolved": theme.resolved.as_ref()});
                reply["fixtureObservation"] = self.fixture_observation(&mounted)?;
                reply["window"] = serde_json::to_value(&mounted.info)?;
                if let RootOwner::Main(main) = &mounted.owner {
                    let cx = self.cx.app.borrow();
                    let app = main.read(&cx);
                    reply["promptObservation"] = serde_json::to_value(app.prompt_observation(&cx))?;
                    reply["searchObservation"] = app.owned_search_observation();
                    reply["searchObservation"]["sourceCacheReadiness"] = Value::Array(
                        CACHE_READINESS_SOURCES
                            .iter()
                            .filter_map(|source| Self::source_cache_readiness(app, source))
                            .collect(),
                    );
                    reply["fileSearch"] = app
                        .owned_file_search_frame_evidence()
                        .unwrap_or(Value::Null);
                    if !reply["fileSearch"].is_null() {
                        reply["fileSearch"]["stream"] =
                            serde_json::to_value(app.owned_file_search_stream_state())?;
                    }
                }
                reply["frameEvidence"] = frame_evidence;
                reply["copySink"] = serde_json::to_value(
                    crate::runtime_policy::owned_evaluation()
                        .context("owned_runtime_policy_missing")?
                        .owned_copy_snapshot()?,
                )?;
            }
        }
        if let QueryMode::CompletedFrame(frame) = mode {
            ensure!(
                self.identity(&target)? == frame.target,
                "capture_frame_identity_stale"
            );
            crate::computer_use::owned_render_capture::with_owned_completed_frame(
                frame,
                &mut self.cx.app.borrow_mut(),
                |_, _| Ok(()),
            )?;
        }
        if operation == "getState" && frame_cursor.is_some() && matches!(mode, QueryMode::Progress)
        {
            Self::pack_search_metadata_refs(&mut reply["frameEvidence"])?;
        }
        Ok(reply)
    }

    fn main_overlay_state(&self, mounted: &Mounted) -> Result<Value> {
        let RootOwner::Main(main) = &mounted.owner else {
            anyhow::bail!("main_owner_required");
        };
        mounted
            .handle
            .update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                super::main_fixtures::main_overlay_state(main.read(cx), window, cx)
            })
    }

    fn append_main_overlay_elements(
        &self,
        mounted: &Mounted,
        raw: &Value,
        reply: &mut Value,
    ) -> Result<()> {
        let RootOwner::Main(main) = &mounted.owner else {
            anyhow::bail!("main_owner_required");
        };
        let (overlay, covered) =
            mounted
                .handle
                .update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                    let layers = gpui_component::Root::read(window, cx).layer_snapshot(cx);
                    let mut snapshot =
                        crate::windows::automation_surface_collector::SurfaceElementSnapshot {
                            elements: super::main_fixtures::main_overlay_elements(
                                main.read(cx),
                                window,
                                cx,
                            ),
                            ..Default::default()
                        };
                    crate::windows::automation_surface_collector::append_root_layer_elements(
                        &mut snapshot,
                        &layers,
                    );
                    (snapshot, !layers.dialogs.is_empty())
                })?;
        let mut base: Vec<crate::protocol::ElementInfo> =
            serde_json::from_value(reply["elements"].clone())?;
        let overlay_ids: std::collections::HashSet<_> = overlay
            .elements
            .iter()
            .map(|element| element.semantic_id.as_str())
            .collect();
        let duplicate_count = base
            .iter()
            .filter(|element| overlay_ids.contains(element.semantic_id.as_str()))
            .count();
        base.retain(|element| !overlay_ids.contains(element.semantic_id.as_str()));
        if covered {
            for element in &mut base {
                if element.selectable == Some(true)
                    || element.element_type == crate::protocol::ElementType::Input
                {
                    element.action_disabled = Some("covered_by_root_dialog".into());
                    element.selectable = Some(false);
                }
            }
            reply["focusedSemanticId"] = Value::Null;
        }
        if !covered {
            if let Some(focused) = overlay
                .elements
                .iter()
                .find(|element| element.focused == Some(true))
            {
                reply["focusedSemanticId"] = json!(focused.semantic_id);
            }
        }
        let total = reply["totalCount"]
            .as_u64()
            .context("main_element_total_missing")? as usize
            + overlay.elements.len()
            - duplicate_count;
        let mut elements = overlay.elements;
        elements.extend(base);
        elements.truncate(raw["limit"].as_u64().unwrap_or(50).clamp(1, 1000) as usize);
        reply["truncated"] = json!(elements.len() < total);
        reply["totalCount"] = json!(total);
        reply["elements"] = serde_json::to_value(elements)?;
        Ok(())
    }

    fn source_cache_readiness(app: &crate::ScriptListApp, source: &str) -> Option<Value> {
        let readiness = app.owned_search_source_cache_readiness(source)?;
        Some(
            json!({"source":source,"query":readiness.query,"cacheIdentity":readiness.identity,
            "cacheStateRevision":readiness.generation,"rowCount":readiness.row_count}),
        )
    }

    fn wait_search_provider(
        &mut self,
        request_id: &str,
        mounted: &Mounted,
        raw: &Value,
    ) -> Result<Value> {
        use crate::protocol::OwnedSearchProviderCondition;
        let OwnedSearchProviderCondition::SearchProvider {
            source,
            query,
            after_run_id,
            accept_cached,
        } = serde_json::from_value(raw["condition"].clone())
            .map_err(|_| anyhow::anyhow!("search_provider_condition_invalid"))?;
        ensure!(
            !accept_cached || after_run_id == 0,
            "search_provider_condition_invalid"
        );
        ensure!(
            mounted.fixture_id == super::search_fixtures::FIXTURE_ID,
            "search_provider_wait_unsupported"
        );
        let RootOwner::Main(main) = &mounted.owner else {
            anyhow::bail!("search_provider_wait_unsupported");
        };
        let timeout_ms = match raw.get("timeout") {
            Some(value) => value
                .as_u64()
                .context("search_provider_condition_invalid")?,
            None => 5_000,
        };
        ensure!(
            timeout_ms <= self.bootstrap.limits.max_lifetime_ms,
            "wait_timeout_exceeds_session_limit"
        );
        let target = Self::instance(mounted)?;
        let initial = self.identity(&target)?;
        let expected_query = crate::RootSearchQueryStamp {
            lifetime: query.lifetime,
            revision: query.revision,
            scope_revision: query.scope_revision,
        };
        let gate = self
            .search_gate()
            .context("search_provider_wait_unsupported")?;
        let start = std::time::Instant::now();
        loop {
            let current = self.identity(&target)?;
            ensure!(
                current.target_generation == initial.target_generation
                    && current.surface_generation == initial.surface_generation
                    && current.app_view_variant == initial.app_view_variant,
                "search_provider_target_stale"
            );
            let observation = {
                let cx = self.cx.app.borrow();
                let app = main.read(&cx);
                let root = &app.root_search;
                ensure!(
                    root.query_stamp() == expected_query,
                    "search_provider_query_stale"
                );
                if root.query_is_current() {
                    let admission = Self::search_provider_admission(
                        root,
                        &gate,
                        source.as_str(),
                        query,
                        after_run_id,
                    )?;
                    if admission.is_some() || !accept_cached {
                        admission
                    } else {
                        Self::source_cache_readiness(app, source.as_str()).map(|cache| {
                            json!({"version":1,"source":source,"query":query,"afterRunId":after_run_id,
                                "status":"cached","owner":null,"run":null,"blockers":[],
                                "pendingDesired":root.provider_has_pending_desire(source.as_str()),
                                "availabilityReason":"sourceCacheReuse","cache":cache})
                        })
                    }
                } else {
                    None
                }
            };
            if let Some(observation) = observation {
                let mut reply = serde_json::to_value(crate::protocol::Message::wait_for_result(
                    request_id.into(),
                    true,
                    start.elapsed().as_millis() as u64,
                    None,
                ))?;
                reply["searchProvider"] = observation;
                return Ok(reply);
            }
            if start.elapsed() >= std::time::Duration::from_millis(timeout_ms) {
                return serde_json::to_value(crate::protocol::Message::wait_for_result(
                    request_id.into(), false, start.elapsed().as_millis() as u64,
                    Some(crate::protocol::TransactionError::wait_timeout(
                        "The current provider did not reach admission or terminal ownership before its deadline",
                    )),
                )).map_err(Into::into);
            }
            self.tick_explicit_wait(start)?;
        }
    }

    fn wait_file_search_stream(
        &mut self,
        request_id: &str,
        mounted: &Mounted,
        raw: &Value,
    ) -> Result<Value> {
        use super::search_fixtures::FileSearchStreamPhase;
        let crate::protocol::OwnedFileSearchStreamCondition::FileSearchStream { generation, query } =
            serde_json::from_value(raw["condition"].clone())
                .map_err(|_| anyhow::anyhow!("file_search_stream_condition_invalid"))?;
        ensure!(generation > 0, "file_search_stream_condition_invalid");
        ensure!(
            mounted.fixture_id == super::search_fixtures::FIXTURE_ID,
            "file_search_stream_wait_unsupported"
        );
        let RootOwner::Main(main) = &mounted.owner else {
            anyhow::bail!("file_search_stream_wait_unsupported");
        };
        let timeout_ms = match raw.get("timeout") {
            Some(value) => value
                .as_u64()
                .context("file_search_stream_condition_invalid")?,
            None => 5_000,
        };
        ensure!(
            timeout_ms <= self.bootstrap.limits.max_lifetime_ms,
            "wait_timeout_exceeds_session_limit"
        );
        let target = Self::instance(mounted)?;
        let initial = self.identity(&target)?;
        let start = std::time::Instant::now();
        loop {
            let current = self.identity(&target)?;
            ensure!(
                current.target_generation == initial.target_generation
                    && current.surface_generation == initial.surface_generation
                    && current.app_view_variant == initial.app_view_variant,
                "file_search_stream_target_stale"
            );
            let terminal = {
                let cx = self.cx.app.borrow();
                let app = main.read(&cx);
                let snapshot = app
                    .owned_file_search_stream_state()
                    .context("file_search_stream_unavailable")?;
                ensure!(
                    snapshot.generation == generation,
                    "file_search_stream_generation_stale"
                );
                ensure!(snapshot.query == query, "file_search_stream_query_stale");
                match snapshot.phase {
                    FileSearchStreamPhase::Accepted | FileSearchStreamPhase::Running => None,
                    FileSearchStreamPhase::Completed
                    | FileSearchStreamPhase::Failed
                    | FileSearchStreamPhase::Cancelled
                    | FileSearchStreamPhase::Unavailable => Some(serde_json::to_value(snapshot)?),
                }
            };
            if let Some(terminal) = terminal {
                let mut reply = serde_json::to_value(crate::protocol::Message::wait_for_result(
                    request_id.into(),
                    true,
                    start.elapsed().as_millis() as u64,
                    None,
                ))?;
                reply["fileSearchStream"] = terminal;
                return Ok(reply);
            }
            if start.elapsed() >= std::time::Duration::from_millis(timeout_ms) {
                return serde_json::to_value(crate::protocol::Message::wait_for_result(
                    request_id.into(), false, start.elapsed().as_millis() as u64,
                    Some(crate::protocol::TransactionError::wait_timeout(
                        "The exact File Search stream did not reach a terminal state before its deadline",
                    )),
                )).map_err(Into::into);
            }
            self.tick_explicit_wait(start)?;
        }
    }

    fn wait_file_search_preview(
        &mut self,
        request_id: &str,
        mounted: &Mounted,
        raw: &Value,
    ) -> Result<Value> {
        let crate::protocol::OwnedFileSearchPreviewCondition::FileSearchPreview {
            generation,
            query,
            work_sequence,
        } = serde_json::from_value(raw["condition"].clone())
            .map_err(|_| anyhow::anyhow!("file_search_preview_condition_invalid"))?;
        let generation = generation.get();
        let work_sequence = work_sequence.get();
        ensure!(
            mounted.fixture_id == super::search_fixtures::FIXTURE_ID,
            "file_search_preview_unavailable"
        );
        let RootOwner::Main(main) = &mounted.owner else {
            anyhow::bail!("file_search_preview_unavailable");
        };
        let timeout_ms = match raw.get("timeout") {
            Some(value) => value
                .as_u64()
                .context("file_search_preview_condition_invalid")?,
            None => 5_000,
        };
        ensure!(
            timeout_ms <= self.bootstrap.limits.max_lifetime_ms,
            "wait_timeout_exceeds_session_limit"
        );
        let target = Self::instance(mounted)?;
        let initial = self.identity(&target)?;
        let start = std::time::Instant::now();
        loop {
            let current = self.identity(&target)?;
            ensure!(
                current.target_generation == initial.target_generation
                    && current.surface_generation == initial.surface_generation
                    && current.app_view_variant == initial.app_view_variant,
                "file_search_preview_target_stale"
            );
            let held = {
                let gate = self
                    .search_gate()
                    .context("file_search_preview_unavailable")?;
                let cx = self.cx.app.borrow();
                let app = main.read(&cx);
                let stream = app
                    .owned_file_search_stream_state()
                    .context("file_search_preview_unavailable")?;
                ensure!(
                    stream.generation == generation,
                    "file_search_preview_generation_stale"
                );
                ensure!(stream.query == query, "file_search_preview_query_stale");
                let request = app
                    .file_search_preview_request()
                    .context("file_search_preview_unavailable")?;
                ensure!(
                    request.sequence == work_sequence
                        && app.file_search_preview_request_is_current(request),
                    "file_search_preview_work_stale"
                );
                ensure!(
                    matches!(
                        &app.file_search_preview_thumbnail,
                        crate::FileSearchThumbnailPreviewState::Loading { .. }
                    ),
                    "file_search_preview_unavailable"
                );
                let held =
                    gate.file_search_preview_completion(generation, &query, work_sequence)?;
                if let Some(completion) = &held {
                    ensure!(
                        completion["path"].as_str() == Some(request.file.path.as_str()),
                        "file_search_preview_work_stale"
                    );
                }
                held
            };
            if let Some(held) = held {
                let mut reply = serde_json::to_value(crate::protocol::Message::wait_for_result(
                    request_id.into(),
                    true,
                    start.elapsed().as_millis() as u64,
                    None,
                ))?;
                reply["fileSearchPreview"] = held;
                return Ok(reply);
            }
            if start.elapsed() >= std::time::Duration::from_millis(timeout_ms) {
                return serde_json::to_value(crate::protocol::Message::wait_for_result(
                    request_id.into(), false, start.elapsed().as_millis() as u64,
                    Some(crate::protocol::TransactionError::wait_timeout(
                        "The exact File Search decoder did not reach its held completion before its deadline",
                    )),
                )).map_err(Into::into);
            }
            self.tick_explicit_wait(start)?;
        }
    }

    fn search_provider_admission(
        root: &crate::RootSearchStore,
        gate: &super::search_fixtures::SearchGate,
        source: &str,
        query: crate::protocol::OwnedSearchQueryStamp,
        after_run_id: u64,
    ) -> Result<Option<Value>> {
        let owner = root.provider_admission_owner(source);
        let pending_desired = root.provider_has_pending_desire(source);
        if pending_desired {
            let sibling = match source {
                "files" => Some("directory"),
                "directory" => Some("files"),
                _ => None,
            };
            let mut blockers = Vec::new();
            for lane in std::iter::once(source).chain(sibling) {
                let Some(blocker) = root
                    .provider_admission_owner(lane)
                    .filter(|owner| owner.terminal.is_none())
                else {
                    continue;
                };
                let Some(run) = gate.provider_run_observation(lane, blocker.generation)? else {
                    continue;
                };
                if run["kind"] == "worker" && run["state"] == "held" {
                    blockers.push(json!({"owner":blocker,"run":run}));
                }
            }
            if !blockers.is_empty() {
                return Ok(Some(
                    json!({"version":1,"source":source,"query":query,"afterRunId":after_run_id,
                    "status":"blocked","owner":owner,"run":null,"blockers":blockers,
                    "pendingDesired":true,"availabilityReason":"pendingReplacement"}),
                ));
            }
            return Ok(None);
        }
        let Some(owner) = owner else {
            return Ok(None);
        };
        if owner.query_bound && owner.consumer != Some(root.query_stamp()) {
            return Ok(None);
        }
        let Some(run) = gate.provider_run_observation(source, owner.generation)? else {
            return Ok(None);
        };
        if !run["id"].as_u64().is_some_and(|id| id > after_run_id) {
            return Ok(None);
        }
        let (status, reason) = match owner.terminal {
            None if run["kind"] == "worker" && run["state"] == "held" => {
                ("admitted", "heldCurrentRun")
            }
            Some(terminal) => {
                use crate::RootProviderTerminal;
                let (state, outcome) = match terminal {
                    RootProviderTerminal::Success => ("completed", "success"),
                    RootProviderTerminal::Empty => ("completed", "empty"),
                    RootProviderTerminal::Failed => ("failed", "error"),
                    RootProviderTerminal::Unavailable => ("unavailable", "unavailable"),
                    RootProviderTerminal::Disconnected => ("failed", "disconnected"),
                    RootProviderTerminal::Cancelled | RootProviderTerminal::StaleDiscarded => {
                        return Ok(None)
                    }
                };
                if run["state"] != state || run["outcome"] != outcome {
                    return Ok(None);
                }
                ("settled", outcome)
            }
            _ => return Ok(None),
        };
        Ok(Some(
            json!({"version":1,"source":source,"query":query,"afterRunId":after_run_id,
            "status":status,"owner":owner,"run":run,"blockers":[],"pendingDesired":false,
            "availabilityReason":reason}),
        ))
    }

    pub(super) fn wait_registered(
        &mut self,
        request_id: &str,
        mounted: &Mounted,
        raw: &Value,
    ) -> Result<Value> {
        if raw["condition"]["type"].as_str() == Some("searchProvider") {
            return self.wait_search_provider(request_id, mounted, raw);
        }
        if raw["condition"]["type"].as_str() == Some("fileSearchStream") {
            return self.wait_file_search_stream(request_id, mounted, raw);
        }
        if raw["condition"]["type"].as_str() == Some("fileSearchPreview") {
            return self.wait_file_search_preview(request_id, mounted, raw);
        }
        let condition: crate::protocol::WaitCondition =
            serde_json::from_value(raw["condition"].clone())?;
        if matches!(mounted.owner, RootOwner::Main(_))
            && crate::is_agent_chat_wait_condition(&condition)
        {
            return self.forward_main(request_id, raw, true);
        }
        let timeout = std::time::Duration::from_millis(raw["timeout"].as_u64().unwrap_or(5_000));
        ensure!(
            timeout <= std::time::Duration::from_millis(self.bootstrap.limits.max_lifetime_ms),
            "wait_timeout_exceeds_session_limit"
        );
        let target = Self::instance(mounted)?;
        let start = std::time::Instant::now();
        loop {
            self.resolve(&target)?;
            let current = crate::windows::automation_surface_collector::current_surface_metadata(
                &mounted.info,
            )
            .context("surface_lifetime_missing")?;
            let satisfied = if let RootOwner::Main(main) = &mounted.owner {
                mounted.handle.update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                    main.update(cx, |app, cx| {
                        let mut state = app.build_main_ui_snapshot(cx);
                        state.window_visible = current.visible;
                        state.window_focused = current.focused;
                        let layers = gpui_component::Root::read(window, cx).layer_snapshot(cx);
                        let mut overlay = crate::windows::automation_surface_collector::SurfaceElementSnapshot {
                            elements: super::main_fixtures::main_overlay_elements(app, window, cx),
                            ..Default::default()
                        };
                        crate::windows::automation_surface_collector::append_root_layer_elements(&mut overlay, &layers);
                        if let Some(input) = overlay.elements.iter().find(|element| element.element_type == crate::protocol::ElementType::Input) {
                            state.input_value = input.value.clone();
                            state.focused_semantic_id = (input.focused == Some(true)).then(|| input.semantic_id.clone());
                        }
                        if !layers.dialogs.is_empty() { state.focused_semantic_id = None; }
                        state.visible_semantic_ids.extend(overlay.elements.into_iter().map(|element| element.semantic_id));
                        crate::protocol::transaction_executor::matches_ui_wait_condition(&state, &condition).unwrap_or(false)
                    })
                })?
            } else {
                crate::registered_surface_wait_satisfied(
                    &mounted.info,
                    &condition,
                    &self.cx.app.borrow(),
                )?
            };
            if satisfied {
                return serde_json::to_value(crate::protocol::Message::wait_for_result(
                    request_id.into(),
                    true,
                    start.elapsed().as_millis() as u64,
                    None,
                ))
                .map_err(Into::into);
            }
            if start.elapsed() >= timeout {
                return serde_json::to_value(crate::protocol::Message::wait_for_result(request_id.into(), false, start.elapsed().as_millis() as u64,
                    Some(crate::protocol::TransactionError::wait_timeout("The requested registered-surface condition was not observed before its deadline")))).map_err(Into::into);
            }
            self.tick_explicit_wait(start)?;
        }
    }

    fn secondary_state(&self, request_id: &str, mounted: &Mounted) -> Result<Value> {
        let cx = self.cx.app.borrow();
        let mut state = serde_json::to_value(crate::registered_surface_state_result(
            request_id,
            &mounted.info,
            &cx,
        )?)?;
        if let Some(main) = &self.main {
            state["activeFooter"] =
                serde_json::to_value(main.read(&cx).active_footer_snapshot(&mounted.info))?;
            state["surfaceContract"] = serde_json::to_value(
                main.read(&cx)
                    .current_surface_contract_snapshot(&mounted.info, &cx),
            )?;
        }
        Ok(state)
    }

    pub(super) fn fixture_observation(&self, mounted: &Mounted) -> Result<Value> {
        if matches!(mounted.owner, RootOwner::Main(_))
            && super::fixture_ids::MAIN_OVERLAY_FIXTURE_IDS.contains(&mounted.fixture_id.as_str())
        {
            let mut state = self.main_overlay_state(mounted)?;
            state["family"] = json!("mainOverlay");
            return Ok(state);
        }
        let cx = self.cx.app.borrow();
        match &mounted.owner {
            RootOwner::Main(main) => {
                let app = main.read(&cx);
                if mounted.fixture_id == "flow.session" {
                    let session_id = *self
                        .flow_controls
                        .get(&mounted.info.id)
                        .context("flow_fixture_session_missing")?;
                    return flow_observation(app, session_id, &cx);
                }
                match &app.current_view {
                    AppView::ThemeChooserView { .. } => {
                        let selected = app.selected_theme_chooser_catalog_entry();
                        let management = app.theme_chooser_management_snapshot(selected.as_ref());
                        Ok(json!({
                            "family": "themeChooser",
                            "panelMode": app.theme_chooser_panel_mode.as_str(),
                            "isDirty": management.is_dirty,
                            "status": management.status_label,
                            "statusKind": management.status_kind,
                            "saveName": management.save_name,
                            "resolvedSaveName": management.resolved_save_name,
                        }))
                    }
                    AppView::DayPage { entity } => {
                        Ok(entity.read(&cx).owned_dictation_observation(&cx))
                    }
                    AppView::AgentChatView { entity } => agent_chat_observation(entity, &cx),
                    AppView::ChatPrompt { entity, .. } => {
                        let prompt = entity.read(&cx);
                        let Some(control) = self.sdk_controls.get(&mounted.info.id) else {
                            return Ok(Value::Null);
                        };
                        Ok(
                            json!({"family":"sdkChat","input":prompt.input_text(),"messageCount":prompt.message_count(),
                            "messages":prompt.messages,"streamingMessageId":prompt.current_stream_message_id(),
                            "useBuiltinAi":prompt.has_builtin_ai(),"saveHistory":prompt.saves_history(),"sinkRequests":control.sink_requests,
                            "acceptedRequests":control.accepted_requests,"stopRequests":control.stop_requests}),
                        )
                    }
                    AppView::FlowSessionView { session_id } => {
                        flow_observation(app, *session_id, &cx)
                    }
                    AppView::FlowUxView { .. } if !app.conversations.flow_sessions.is_empty() => {
                        let session_id = app
                            .conversations
                            .flow_sessions
                            .last()
                            .context("flow_session_missing")?
                            .0
                            .id;
                        flow_observation(app, session_id, &cx)
                    }
                    _ => Ok(Value::Null),
                }
            }
            RootOwner::Notes(entity) => super::notes_fixtures::observe_notes_fixture_presentation(
                entity,
                &mounted.info,
                mounted.handle,
                &cx,
            ),
            RootOwner::AgentChat(view) => agent_chat_observation(view, &cx),
            RootOwner::Dictation(view) => {
                let state = view.read(&cx).fixture_state();
                let (microphone_selection_count, selected_microphone_semantic_id) =
                    view.read(&cx).microphone_selection();
                let phase = match &state.phase {
                    crate::dictation::DictationSessionPhase::Idle => "idle",
                    crate::dictation::DictationSessionPhase::Recording => "recording",
                    crate::dictation::DictationSessionPhase::Confirming => "confirming",
                    crate::dictation::DictationSessionPhase::Transcribing => "transcribing",
                    crate::dictation::DictationSessionPhase::Delivering => "delivering",
                    crate::dictation::DictationSessionPhase::Finished => "finished",
                    crate::dictation::DictationSessionPhase::Failed(_) => "failed",
                };
                let outcome = match &state.phase {
                    crate::dictation::DictationSessionPhase::Finished => Some("delivered"),
                    crate::dictation::DictationSessionPhase::Failed(failure)
                        if failure.failure.failure.code
                            == sk_protocol::ai_reliability::AiFailureCode::DestinationStale =>
                    {
                        Some("staleTarget")
                    }
                    crate::dictation::DictationSessionPhase::Failed(_) => Some("refused"),
                    _ => None,
                };
                let outcome = self
                    .dictation_controls
                    .contains_key(&mounted.info.id)
                    .then_some(outcome)
                    .flatten();
                Ok(
                    json!({"family":"dictation","phase":phase,"transcript":state.transcript.as_ref(),
                    "microphoneSelectionCount":microphone_selection_count,"selectedMicrophoneSemanticId":selected_microphone_semantic_id,
                    "generation":self.dictation_controls.get(&mounted.info.id).map(|control|control.generation()),
                    "deliveryOutcome":outcome,"destinationLocked":!matches!(state.phase,crate::dictation::DictationSessionPhase::Recording|crate::dictation::DictationSessionPhase::Confirming)}),
                )
            }
            RootOwner::Secondary(fixture) => fixture.controls.observation(&cx),
            RootOwner::Footer => {
                let state = crate::footer_popup::footer_runtime_state(
                    &mounted.info.id,
                    mounted
                        .info
                        .generation
                        .context("footer_generation_missing")?,
                )
                .context("footer_owner_missing")?;
                Ok(
                    json!({"family":"footer","owner":state.binding.window_id,"completedActionCount":state.completed_action_count,
                    "semanticRevision":state.semantic_revision,"presentationRevision":state.presentation_revision,
                    "appliedThemeRevision":state.applied_theme_revision,"heldAction":format!("{:?}",state.held_action)}),
                )
            }
            RootOwner::ShortcutRecorder => {
                Ok(crate::shortcut_recorder::shortcut_fixture_observation(
                    &mounted.info.id,
                    mounted
                        .info
                        .generation
                        .context("recorder_generation_missing")?,
                    None,
                    &cx,
                )?
                .value)
            }
            _ => Ok(Value::Null),
        }
    }
}

fn agent_chat_observation(
    view: &gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>,
    cx: &gpui::App,
) -> Result<Value> {
    let view = view.read(cx);
    let Some(thread) = view.thread() else {
        return Ok(
            json!({"family":"agentChat","state":view.collect_agent_chat_state_snapshot(cx)}),
        );
    };
    let mut value = serde_json::to_value(conversation_fixtures::agent_chat_fixture_receipt(
        &thread, cx,
    )?)?;
    value["family"] = json!("agentChat");
    value["ownedDictation"] = serde_json::to_value(view.owned_dictation_observation(cx))?;
    value["state"] = serde_json::to_value(view.collect_agent_chat_state_snapshot(cx))?;
    Ok(value)
}

fn flow_observation(app: &crate::ScriptListApp, session_id: u64, cx: &gpui::App) -> Result<Value> {
    let (meta, prompt) = app
        .conversations
        .flow_sessions
        .iter()
        .find(|(meta, _)| meta.id == session_id)
        .context("flow_session_missing")?;
    Ok(
        json!({"family":"flow","sessionId":session_id,"draftText":meta.active_draft,
        "activeMessageId":meta.active_turn.as_ref().map(|turn|turn.message_id.as_str()),
        "state":format!("{:?}",meta.state),"runtimeGeneration":meta.runtime_generation,
        "messages":prompt.read(cx).messages}),
    )
}
