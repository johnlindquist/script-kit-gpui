//! Executable negative-only fixtures. Nothing here is a production-family adoption root.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use anyhow::{ensure, Context as _, Result};
use gpui::{
    div, img, point, prelude::*, px, size, svg, App, Bounds, Context, Entity, IntoElement, Render,
    Window, WindowBounds, WindowHandle, WindowKind,
};
use serde_json::{json, Value};

use super::runtime::{Evaluator, Mounted};
use crate::ai::agent_chat::ui::components::setup_card::AgentChatSetupCard;
use crate::ai::agent_chat::ui::setup_state::{AgentChatInlineSetupState, AgentChatSetupAction};
use crate::computer_use::gpui_runtime_bridge::{
    capture_render_window_on_gpui_thread, forget_owned_render_frame,
};
use crate::computer_use::owned_render_capture::observe_owned_native_window;
use crate::computer_use::runtime_bridge::ComputerUseCaptureRenderWindowRequest;
use crate::protocol::{
    AutomationTargetIdentitySnapshot, AutomationWindowTarget, NativeSafetyProbe,
};
use crate::runtime_policy::WindowHostPolicy;

const MISSING_IMAGE: &str = "__owned_negative_only_missing_required_image__.png";
const MISSING_SVG: &str = "__owned_negative_only_missing_required_svg__.svg";

/// Uses actual GPUI assets and production setup cards/buttons, with deliberately
/// invalid inputs/composition. Never registered in the production target catalogue.
enum NegativeRoot {
    MissingImage,
    MissingSvg,
    OversizedImage(std::sync::Arc<gpui::RenderImage>),
    Cards(Vec<Entity<AgentChatSetupCard>>),
}

impl NegativeRoot {
    fn new(probe: NativeSafetyProbe, cx: &mut App) -> Self {
        match probe {
            NativeSafetyProbe::MissingRequiredImage => Self::MissingImage,
            NativeSafetyProbe::MissingRequiredSvg => Self::MissingSvg,
            NativeSafetyProbe::OversizedImage => {
                // A bounded CPU buffer exercises rejection before atlas/GPU allocation.
                let frame = image::Frame::new(image::RgbaImage::new(2049, 2048));
                Self::OversizedImage(std::sync::Arc::new(gpui::RenderImage::new(
                    smallvec::smallvec![frame],
                )))
            }
            NativeSafetyProbe::DuplicateSemanticIdentity
            | NativeSafetyProbe::DuplicateMeasurementIdentity => Self::Cards(
                (0..2)
                    .map(|_| {
                        cx.new(|cx| {
                            AgentChatSetupCard::new(AgentChatInlineSetupState {
                reason_code: "negative_only_duplicate_identity",
                title: "Negative-only identity fixture".into(),
                body: "Two real setup cards deliberately share control identities.".into(),
                primary_action: AgentChatSetupAction::Retry,
                secondary_action: None,
                selected_agent: None,
                catalog_entries: Vec::new(),
                launch_requirements: Default::default(),
            }, None, cx)
                        })
                    })
                    .collect(),
            ),
            _ => Self::Cards(Vec::new()),
        }
    }
}

impl Render for NegativeRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().flex().flex_col();
        match self {
            Self::MissingImage => root.child(img(MISSING_IMAGE).w(px(48.)).h(px(48.))),
            // SVG painting requires a foreground color; without one GPUI never
            // attempts the asset lookup, so required-resource accounting stays empty.
            Self::MissingSvg => root.child(
                svg()
                    .path(MISSING_SVG)
                    .text_color(gpui::rgb(
                        crate::theme::get_cached_theme().colors.text.primary,
                    ))
                    .w(px(48.))
                    .h(px(48.)),
            ),
            Self::OversizedImage(image) => root.child(img(image.clone()).w(px(48.)).h(px(48.))),
            Self::Cards(cards) => root.children(cards.iter().cloned()),
        }
    }
}

fn native_observation(native: gpui::OwnedHiddenObservation) -> Value {
    json!({"installed":native.installed,"openedWindows":native.opened_windows,
        "liveWindows":native.live_windows,"completedFrames":native.completed_frames,
        "readbackImages":native.readback_images,"refusedOperations":native.refused_operations})
}

// Only fixed machine vocabulary is returned, never paths, credentials, provider
// diagnostics, or operator data from an unexpectedly changed production owner.
fn error_code(error: &dyn std::fmt::Display) -> &'static str {
    let message = error.to_string();
    [
        "owned_hidden_show_or_focus",
        "owned_hidden_window_kind",
        "owned_hidden_tabbing",
        "owned_hidden_pixel_limit",
        "owned_readback_fault_failure",
        "owned_render_asset_failed",
        "owned_render_resources_incomplete",
        "process_forbidden",
        "provider_forbidden",
        "device_forbidden",
        "system_clipboard_forbidden",
        "external_open_forbidden",
        "completed_frame_timeout",
        "evaluation_frame_budget_exhausted",
        "negative_probe_unexpected_root_creation",
    ]
    .into_iter()
    .find(|code| message.contains(code))
    .unwrap_or("probe_owner_error")
}

fn result_observation<T, E: std::fmt::Display>(result: std::result::Result<T, E>) -> Value {
    match result {
        Ok(_) => json!({"returnedOk":true}),
        Err(error) => json!({"returnedOk":false,"errorCode":error_code(&error)}),
    }
}

impl Evaluator {
    fn safety_window_state(&mut self, mounted: &Mounted) -> Result<Value> {
        mounted
            .handle
            .update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                let bounds = window.bounds();
                Ok(json!({"ownedHidden":window.is_owned_hidden(),"active":window.is_window_active(),
                "native":observe_owned_native_window(window)?,
                "focus":window.focused(cx).map(|focus| format!("{focus:?}")),
                "bounds":{"x":bounds.origin.x.as_f32(),"y":bounds.origin.y.as_f32(),
                    "width":bounds.size.width.as_f32(),"height":bounds.size.height.as_f32()}}))
            })?
    }

    pub(super) fn probe_safety(
        &mut self,
        target: &AutomationWindowTarget,
        expected: &AutomationTargetIdentitySnapshot,
        probe: NativeSafetyProbe,
    ) -> Result<Value> {
        let policy = crate::runtime_policy::owned_evaluation().context("owned_policy_missing")?;
        ensure!(!self.ended(), "evaluation_session_ended");
        self.validate_expected(target, expected)?;
        ensure!(self.identity(target)? == *expected, "stale_frame_identity");
        if matches!(
            probe,
            NativeSafetyProbe::DeferredDispatch
                | NativeSafetyProbe::ClipboardRead
                | NativeSafetyProbe::ClipboardWrite
        ) && self.main.is_none()
        {
            // The production Main owner is a support dependency, not target-family proof.
            self.ensure_main()?;
        }
        let mounted = self.resolve(target)?.clone();
        let generation = mounted
            .info
            .generation
            .context("window_generation_missing")?;
        ensure!(
            crate::windows::runtime_window_host_policy(&mounted.info.id, generation)?
                == WindowHostPolicy::OwnedHidden,
            "owned_hidden_window_required"
        );
        ensure!(
            !mounted.info.visible
                && !mounted.info.focused
                && mounted.info.pid == Some(std::process::id()),
            "owned_window_visible_metadata"
        );
        ensure!(
            self.bootstrap.identity.process_instance_id == policy.process_instance_id()
                && self.bootstrap.identity.session_generation == policy.session_generation(),
            "owned_policy_identity_mismatch"
        );
        let state_before = self.safety_window_state(&mounted)?;
        ensure!(
            state_before["ownedHidden"] == true && state_before["active"] == false,
            "owned_hidden_window_required"
        );
        let native_before = self.cx.owned_hidden_observation();
        ensure!(native_before.installed, "native_guard_not_installed");
        let refused_before = policy.refused_effect_count();
        let effects_before = policy.completed_fixture_effect_count();
        let copy_before = policy.owned_copy_snapshot()?;
        let started = Instant::now();
        let observation = match probe {
            NativeSafetyProbe::InvalidShow
            | NativeSafetyProbe::InvalidFocus
            | NativeSafetyProbe::InvalidDialog
            | NativeSafetyProbe::InvalidTabbing
            | NativeSafetyProbe::InvalidOversize => self.probe_invalid_window(probe)?,
            NativeSafetyProbe::NativeActivation => {
                mounted
                    .handle
                    .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                        window.activate_window()
                    })?;
                json!({"owner":"MacWindow::activate","returnedVoid":true})
            }
            NativeSafetyProbe::NativeIme => {
                mounted
                    .handle
                    .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                        window.probe_owned_ime_position()
                    })??;
                json!({"owner":"MacWindow::update_ime_position","returnedVoid":true})
            }
            NativeSafetyProbe::GlobalPointer => {
                let position = mounted
                    .handle
                    .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                        window.probe_owned_global_pointer()
                    })??;
                json!({"owner":"MacWindow::mouse_position","returnedInertOrigin":position == point(px(0.), px(0.))})
            }
            NativeSafetyProbe::ClipboardRead => json!({
                "owner":"capture_general_pasteboard_snapshot",
                "result":result_observation(crate::platform::accessibility::clipboard::capture_general_pasteboard_snapshot()),
                "applyBack":self.probe_apply_back_clipboard()?}),
            NativeSafetyProbe::ClipboardWrite => json!({
                "owner":"write_plain_text_to_pasteboard",
                "result":result_observation(crate::platform::accessibility::clipboard::write_plain_text_to_pasteboard("negative-only clipboard refusal probe")),
                "applyBack":self.probe_apply_back_clipboard()?}),
            NativeSafetyProbe::DirectAppActivation => {
                self.cx.app.borrow().activate(true);
                json!({"owner":"App::activate","returnedVoid":true})
            }
            NativeSafetyProbe::Process => {
                use crate::ai::agent_prompt_handoff::{
                    launch_prompt_handoff, AgentPromptHandoffAdapterId, AgentPromptHandoffError,
                    AgentPromptHandoffPayload, AgentPromptHandoffSource,
                };
                let result = launch_prompt_handoff(&AgentPromptHandoffPayload {
                    source: AgentPromptHandoffSource::AgentChatComposer,
                    adapter_id: AgentPromptHandoffAdapterId::CmuxCodex,
                    raw_input: "negative-only process refusal probe".into(),
                    prompt: "negative-only process refusal probe".into(),
                    cwd: policy.root().to_path_buf(),
                    model_id: None,
                    profile_id: None,
                    context_part_count: 0,
                    prompt_builder_segment_count: 0,
                    warnings: Vec::new(),
                });
                let observation = match &result {
                    Ok(_) => json!({"returnedOk":true}),
                    Err(AgentPromptHandoffError::Spawn(code)) if code == "process_forbidden" => {
                        json!({"returnedOk":false,"errorCode":code})
                    }
                    Err(_) => json!({"returnedOk":false,"errorCode":"probe_owner_error"}),
                };
                json!({"owner":"launch_prompt_handoff","result":observation})
            }
            NativeSafetyProbe::Provider => {
                use crate::ai::local_llm::{
                    generate_ghost_completion, GhostPromptSpec, LocalGhostRequest,
                };
                let request = LocalGhostRequest {
                    prompt: GhostPromptSpec::NotesContinuation {
                        prompt: "negative-only provider refusal probe".into(),
                    },
                    cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                };
                json!({"owner":"generate_ghost_completion",
                    "result":result_observation(generate_ghost_completion(&crate::config::Config::default(), request))})
            }
            NativeSafetyProbe::Credentials => {
                let settings = crate::ai::session::read_user_credential_settings();
                json!({"owner":"read_user_credential_settings","returnedEmptyObject":settings.as_object().is_some_and(|object| object.is_empty())})
            }
            NativeSafetyProbe::Device => json!({"owner":"dictation::list_input_devices",
                "result":result_observation(crate::dictation::list_input_devices())}),
            NativeSafetyProbe::OpenExternal => json!({"owner":"platform::open_in_default_app",
                "result":result_observation(crate::platform::open_in_default_app(policy.root()))}),
            NativeSafetyProbe::Notification => {
                crate::ai::agent_chat::ui::notifications::dispatch_agent_chat_notification(
                    "Negative-only notification refusal probe",
                    "No notification may be delivered.".into(),
                );
                json!({"owner":"dispatch_agent_chat_notification","returnedVoid":true})
            }
            NativeSafetyProbe::BlankReadback | NativeSafetyProbe::FailedReadback => {
                self.probe_readback(target, expected, probe)?
            }
            NativeSafetyProbe::MissingRequiredImage
            | NativeSafetyProbe::MissingRequiredSvg
            | NativeSafetyProbe::OversizedImage
            | NativeSafetyProbe::DuplicateSemanticIdentity
            | NativeSafetyProbe::DuplicateMeasurementIdentity => self.probe_negative_root(probe)?,
            NativeSafetyProbe::DeferredDispatch => self.probe_deferred_dispatch(target)?,
        };
        let native_after = self.cx.owned_hidden_observation();
        let state_after = self.safety_window_state(&mounted)?;
        let actual = self.identity(target)?;
        let refusal_probe = !matches!(
            probe,
            NativeSafetyProbe::BlankReadback
                | NativeSafetyProbe::FailedReadback
                | NativeSafetyProbe::MissingRequiredImage
                | NativeSafetyProbe::MissingRequiredSvg
                | NativeSafetyProbe::OversizedImage
                | NativeSafetyProbe::DuplicateSemanticIdentity
                | NativeSafetyProbe::DuplicateMeasurementIdentity
                | NativeSafetyProbe::DeferredDispatch
        );
        let gap = (refusal_probe
            && native_after.refused_operations == native_before.refused_operations
            && policy.refused_effect_count() == refused_before)
            .then_some("production_refusal_not_observed");
        Ok(json!({"operation":"probeSafety","ok":true,"probe":probe,
            "negativeOnly":true,"productionEvidence":false,"target":target,"targetIdentity":actual,
            "implementationGap":gap,
            "before":{"native":native_observation(native_before),"refusedEffects":refused_before,
                "completedFixtureEffects":effects_before,"window":state_before},
            "after":{"native":native_observation(native_after),"refusedEffects":policy.refused_effect_count(),
                "completedFixtureEffects":policy.completed_fixture_effect_count(),"window":state_after},
            "windowStateUnchanged":state_before == state_after,
            "ownedCopyUnchanged":copy_before == policy.owned_copy_snapshot()?,
            "observation":observation,"elapsedMs":started.elapsed().as_secs_f64()*1000.0}))
    }

    fn probe_apply_back_clipboard(&mut self) -> Result<Value> {
        use super::prompt_fixtures::{prompt_fixture_seed, PromptSeed};
        use crate::agent_handoff::{
            probe_tab_ai_apply_back_clipboard_boundary, TabAiApplyBackClipboardProbe,
        };

        let app = self.main.clone().context("main_fixture_missing")?;
        let probe = probe_tab_ai_apply_back_clipboard_boundary()?;
        let (weak_terminal, mut observation) =
            app.update(&mut **self.cx.app.borrow_mut(), |app, cx| -> Result<_> {
                let PromptSeed::QuickTerminal(seed) =
                    prompt_fixture_seed("prompt.quick-terminal", &app.theme)?
                else {
                    anyhow::bail!("quick_terminal_fixture_missing");
                };
                // Reuse the existing PTY-free seed and real TermPrompt constructor.
                // No render/refresh timer or native focus is needed to invoke apply-back.
                let prompt = crate::term_prompt::TermPrompt::with_existing_terminal(
                    seed.common.completion.instance().id.clone(),
                    seed.terminal,
                    cx.focus_handle(),
                    seed.common.completion.submit_callback(),
                    app.theme.clone(),
                    std::sync::Arc::new(app.config.clone()),
                    Some(
                        crate::window_resize::layout::MAX_HEIGHT
                            - px(crate::window_resize::layout::FOOTER_HEIGHT),
                    ),
                )?;
                ensure!(
                    prompt.selected_text_for_apply().is_none(),
                    "terminal_fixture_selection_not_empty"
                );
                let terminal = cx.new(|_| prompt);
                let weak_terminal = terminal.downgrade();
                let previous_view = std::mem::replace(
                    &mut app.current_view,
                    crate::AppView::QuickTerminalView {
                        entity: terminal.clone(),
                    },
                );
                app.note_main_route_changed();
                let previous_route = app.tab_ai_harness_apply_back_route.take();
                let previous_toasts = std::mem::take(&mut app.toast_manager);
                // This is the real terminal entrypoint. Owned policy refuses
                // synchronously before priming or scheduling its read callback.
                app.apply_tab_ai_result_from_terminal(terminal.clone(), cx);
                let observation = probe.observation();
                // Restore the exact owner state before any fallible progress or
                // proof assertions. No fixture state or error toast is retained.
                app.current_view = previous_view;
                app.note_main_route_changed();
                app.tab_ai_harness_apply_back_route = previous_route;
                app.toast_manager = previous_toasts;
                drop(terminal);
                if matches!(app.current_view, crate::AppView::ScriptList) {
                    app.flush_pending_main_menu_query(cx);
                }
                Ok((weak_terminal, observation))
            })?;
        drop(probe);
        self.tick(false)?;
        observation["probeCleared"] = json!(!TabAiApplyBackClipboardProbe::is_active());
        observation["terminalFixtureReleased"] = json!(weak_terminal.upgrade().is_none());
        Ok(observation)
    }

    fn probe_invalid_window(&mut self, probe: NativeSafetyProbe) -> Result<Value> {
        let mut options = crate::main_window_options(
            Bounds::new(point(px(0.), px(0.)), size(px(320.), px(240.))),
            crate::WindowBackgroundAppearance::Transparent,
            WindowHostPolicy::OwnedHidden,
        )?;
        match probe {
            NativeSafetyProbe::InvalidShow => options.show = true,
            NativeSafetyProbe::InvalidFocus => options.focus = true,
            NativeSafetyProbe::InvalidDialog => options.kind = WindowKind::Dialog,
            NativeSafetyProbe::InvalidTabbing => {
                options.tabbing_identifier = Some("negative-only".into())
            }
            NativeSafetyProbe::InvalidOversize => {
                options.window_bounds = Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(4096.), px(4096.)),
                )))
            }
            _ => anyhow::bail!("not_an_invalid_window_probe"),
        }
        let before = self.cx.owned_hidden_observation().live_windows;
        let mut root_called = false;
        let result: Result<WindowHandle<NegativeRoot>> = self
            .cx
            .app
            .borrow_mut()
            .open_window_fallible(options, |_, _| {
                root_called = true;
                anyhow::bail!("negative_probe_unexpected_root_creation")
            });
        // The fallible production factory owns rollback even on unexpected entry
        // into the constructor. No auxiliary root can escape this request.
        self.drain_auxiliary_close(before)?;
        Ok(
            json!({"owner":"App::open_window_fallible","result":result_observation(result),
            "rootConstructorCalled":root_called,"auxiliaryWindowsRemaining":self.cx.owned_hidden_observation().live_windows.saturating_sub(before)}),
        )
    }

    fn probe_readback(
        &mut self,
        target: &AutomationWindowTarget,
        expected: &AutomationTargetIdentitySnapshot,
        probe: NativeSafetyProbe,
    ) -> Result<Value> {
        let mounted = self.resolve(target)?.clone();
        let generation = mounted
            .info
            .generation
            .context("window_generation_missing")?;
        let fault = if probe == NativeSafetyProbe::BlankReadback {
            gpui::OwnedReadbackFault::Blank
        } else {
            gpui::OwnedReadbackFault::Failure
        };
        let after = expected
            .frame_generation
            .context("frame_generation_missing")?;
        let _negative_frames = self.isolate_negative_readback(target)?;
        let readbacks_before = self.cx.owned_hidden_observation().readback_images;
        mounted
            .handle
            .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                window.arm_owned_readback_fault(fault)
            })??;
        let completed = self.completed_frame(target, expected, after);
        let reached_boundary = self.cx.owned_hidden_observation().readback_images
            > readbacks_before
            || completed
                .as_ref()
                .err()
                .is_some_and(|error| error_code(error) == "owned_readback_fault_failure");
        // On success require the exact newly stamped identity. On failure request
        // the old identity deliberately: retained old pixels must no longer work.
        let capture_expected = match &completed {
            Ok((identity, _)) => identity.target.clone(),
            Err(_) => expected.clone(),
        };
        let clear = mounted
            .handle
            .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                window.clear_owned_readback_fault()
            });
        let capture = capture_render_window_on_gpui_thread(
            &ComputerUseCaptureRenderWindowRequest {
                target: target.clone(),
                expected: Some(capture_expected),
                hi_dpi: true,
                include_image: false,
                probes: Vec::new(),
                correlation_id: "negative-only-readback-probe".into(),
            },
            &mut self.cx.app.borrow_mut(),
        );
        let retired = forget_owned_render_frame(&mounted.info.id, generation);
        clear??;
        let capture = match capture {
            Ok(snapshot) => serde_json::to_value(snapshot)?,
            Err(error) => json!({"bridgeErrorCode":error.error_code()}),
        };
        Ok(
            json!({"owner":"MacWindow::render_to_image","faultArmed":probe,
            "faultReachedBoundary":reached_boundary,
            "implementationGap":(!reached_boundary).then_some("readback_fault_boundary_not_reached"),
            "pixelsArePristine":false,"completedFrame":result_observation(completed),
            "capture":capture,"faultCleared":true,"retiredFaultFrame":retired,
            "auxiliaryWindowsRemaining":0}),
        )
    }

    fn drain_auxiliary_close(&mut self, baseline: u64) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.tick(true)?;
            let progress = self.cx.pump_owned_work(0, Duration::ZERO, 0)?;
            let native = self.cx.owned_hidden_observation();
            if native.live_windows <= baseline
                && progress.pending_foreground_tasks == 0
                && progress.pending_effects == 0
            {
                ensure!(
                    native.completed_frames <= u64::from(self.bootstrap.limits.max_frames),
                    "evaluation_frame_budget_exhausted"
                );
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "negative_probe_auxiliary_close_timeout"
            );
        }
    }

    fn probe_negative_root(&mut self, probe: NativeSafetyProbe) -> Result<Value> {
        let baseline = self.cx.owned_hidden_observation().live_windows;
        ensure!(
            baseline < u64::from(self.bootstrap.limits.max_windows),
            "evaluation_window_budget_exhausted"
        );
        let options = crate::main_window_options(
            Bounds::new(point(px(0.), px(0.)), size(px(640.), px(480.))),
            crate::WindowBackgroundAppearance::Transparent,
            WindowHostPolicy::OwnedHidden,
        )?;
        let mut root = None;
        let opened = self
            .cx
            .open_owned_hidden_window_fallible(options, |window, cx| {
                let view = cx.new(|cx| NegativeRoot::new(probe, cx));
                root = Some(view.clone());
                Ok(cx.new(|cx| gpui_component::Root::new(view, window, cx)))
            });
        let handle: gpui::AnyWindowHandle = match opened {
            Ok(handle) => handle.into(),
            Err(error) => {
                drop(root);
                self.drain_auxiliary_close(baseline)?;
                return Err(error);
            }
        };
        let observation = (|| -> Result<Value> {
            let root = root.as_ref().context("negative_root_missing")?;
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut pending_observed = false;
            loop {
                ensure!(
                    self.cx.owned_hidden_observation().completed_frames
                        < u64::from(self.bootstrap.limits.max_frames),
                    "evaluation_frame_budget_exhausted"
                );
                let resources =
                    handle.update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                        window.refresh();
                        window.draw_owned_frame(cx, |window, _| {
                            Ok(window.owned_render_resource_status())
                        })
                    })??;
                pending_observed |= resources.pending > 0;
                if resources.pending == 0 || resources.failed > 0 {
                    return handle.update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                        let mut layout = crate::protocol::LayoutInfo {
                            window_width: window.viewport_size().width.as_f32(),
                            window_height: window.viewport_size().height.as_f32(),
                            prompt_type: "negativeOnly".into(), ..Default::default()
                        };
                        crate::windows::automation_surface_collector::append_window_paint_measurements(&mut layout, window);
                        let elements: Vec<_> = match root.read(cx) {
                            NegativeRoot::Cards(cards) => cards.iter()
                                .flat_map(|card| card.read(cx).collect_semantic_elements()).collect(),
                            _ => Vec::new(),
                        };
                        let fault_image = if let NegativeRoot::OversizedImage(image) = root.read(cx) {
                            let dimensions = image.size(0);
                            Some(json!({"source":"predecoded-negative-only","width":dimensions.width.0,
                                "height":dimensions.height.0,"frameCount":image.frame_count(),
                                "byteLength":image.as_bytes(0).map(<[u8]>::len),
                                "pixels":i64::from(dimensions.width.0) * i64::from(dimensions.height.0),
                                "pixelLimit":gpui::OWNED_HIDDEN_MAX_PIXELS}))
                        } else { None };
                        let mut semantic_counts = BTreeMap::<String, usize>::new();
                        for element in &elements { *semantic_counts.entry(element.semantic_id.clone()).or_default() += 1; }
                        let mut measurement_counts = BTreeMap::<String, usize>::new();
                        for component in &layout.components { *measurement_counts.entry(component.name.clone()).or_default() += 1; }
                        let readback = if matches!(probe, NativeSafetyProbe::MissingRequiredImage | NativeSafetyProbe::MissingRequiredSvg | NativeSafetyProbe::OversizedImage) {
                            Some(result_observation(window.render_to_image()))
                        } else { None };
                        let gap = match probe {
                            NativeSafetyProbe::DuplicateSemanticIdentity if !semantic_counts.values().any(|count| *count > 1) => Some("duplicate_semantic_identity_not_materialized"),
                            NativeSafetyProbe::DuplicateMeasurementIdentity if !measurement_counts.values().any(|count| *count > 1) => Some("duplicate_measurement_identity_not_materialized"),
                            NativeSafetyProbe::MissingRequiredImage | NativeSafetyProbe::MissingRequiredSvg if resources.failed == 0 => Some("missing_required_asset_not_observed"),
                            NativeSafetyProbe::OversizedImage if resources.failed == 0 => Some("oversized_image_failure_not_observed"),
                            _ => None,
                        };
                        Ok(json!({"owner":"negative-only GPUI root","negativeOnly":true,
                            "productionEvidence":false,"registeredProductionTarget":false,
                            "frameGeneration":window.rendered_frame_generation(),
                            "resources":{"pending":resources.pending,"failed":resources.failed,"pendingObserved":pending_observed},
                            "faultImage":fault_image,
                            "readback":readback,"elements":elements,"layout":layout,
                            "semanticOccurrences":semantic_counts,"measurementOccurrences":measurement_counts,
                            "implementationGap":gap,"publishedProductionFrame":false}))
                    })?;
                }
                ensure!(Instant::now() < deadline, "completed_frame_timeout");
                self.tick(true)?;
            }
        })();
        // Cleanup is unconditional, including asset timeouts and draw failures.
        let close = handle.update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
            window.remove_window()
        });
        drop(root);
        let drained = self.drain_auxiliary_close(baseline);
        close?;
        drained?;
        let mut observation = observation?;
        observation["auxiliaryWindowsRemaining"] = json!(self
            .cx
            .owned_hidden_observation()
            .live_windows
            .saturating_sub(baseline));
        observation["auxiliaryWindowClosed"] = json!(handle
            .update(&mut **self.cx.app.borrow_mut(), |_, _, _| ())
            .is_err());
        Ok(observation)
    }
}
