use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use anyhow::{ensure, Context as _, Result};
use gpui::{
    point, px, size, AnyWindowHandle, App, AppContext, Bounds, Entity, Pixels,
    VisualTestAppContext, Window, WindowHandle,
};
use gpui_component::Root;
use serde_json::{json, Value};

use super::bootstrap::{Bootstrap, GUARDS};
use super::runtime_query::QueryMode;
use super::{
    catalog, conversation_fixtures, dictation_fixtures, main_fixtures, notes_fixtures,
    prompt_fixtures, secondary_fixtures,
};
use crate::computer_use::gpui_runtime_bridge::{
    capture_render_window_on_gpui_thread, forget_owned_render_frame, publish_owned_render_frame,
    OwnedCompletedRenderFrame,
};
use crate::computer_use::runtime_bridge::ComputerUseCaptureRenderWindowRequest;
use crate::protocol::{
    AutomationTargetIdentitySnapshot, AutomationWindowInfo, AutomationWindowKind,
    AutomationWindowTarget, CompletedFrameIdentity, DesignCommand,
};
use crate::runtime_policy::WindowHostPolicy;
use crate::{AppView, ScriptListApp};

// Owned ingress cannot use the namespace reserved for forwarded batch steps.
pub(super) const BATCH_STEP_REQUEST_ID_PREFIX: &str = "owned-evaluation:batch:";

#[derive(Clone)]
pub(super) enum RootOwner {
    Main(Entity<ScriptListApp>),
    Notes(Entity<crate::notes::NotesApp>),
    AgentChat(Entity<crate::ai::agent_chat::ui::AgentChatView>),
    Dictation(Entity<crate::dictation::DictationOverlay>),
    Secondary(Rc<secondary_fixtures::MountedSecondaryFixture>),
    Footer,
    ShortcutRecorder,
    ConversationPopup,
    RegisteredChild,
}

#[derive(Clone)]
pub(super) struct Mounted {
    pub fixture_id: String,
    pub info: AutomationWindowInfo,
    pub handle: AnyWindowHandle,
    pub owner: RootOwner,
}

pub(super) struct Evaluator {
    pub cx: VisualTestAppContext,
    pub bootstrap: Bootstrap,
    pub main: Option<Entity<ScriptListApp>>,
    pub mounted: BTreeMap<String, Mounted>,
    pub sdk_controls: BTreeMap<String, conversation_fixtures::SdkChatFixtureControl>,
    pub sdk_prompt_controls: BTreeMap<String, super::runtime_sdk::SdkPromptControl>,
    pub dictation_controls: BTreeMap<String, crate::dictation::DictationFixtureControl>,
    pub flow_controls: BTreeMap<String, u64>,
    pub(super) theme_fixture: super::runtime_theme::ThemeFixtureState,
    pub(super) frames: super::runtime_frames::RuntimeFrames,
    preview: crate::theme::live_edit::LiveThemePreview,
    pub(super) response_sender: mpsc::SyncSender<crate::protocol::Message>,
    pub(super) response_receiver: mpsc::Receiver<crate::protocol::Message>,
    qualified: bool,
    ended: bool,
    cleanup_failure: Option<String>,
    registry_revision: u64,
    last_progress_at: Instant,
    retired_search_gate: Option<Arc<super::search_fixtures::SearchGate>>,
}

impl Evaluator {
    pub(super) fn new(bootstrap: Bootstrap) -> Result<Self> {
        let policy = crate::runtime_policy::owned_evaluation().context("owned_policy_missing")?;
        let cx = gpui_platform::owned_hidden_context(
            Arc::new(crate::utils::assets::AppAssets),
            Arc::new(move |path| policy.require_owned_path(path).map_err(anyhow::Error::from)),
        )?;
        ensure!(
            cx.owned_hidden_observation().installed,
            "native_guard_not_installed"
        );
        {
            let mut app = cx.app.borrow_mut();
            gpui_component::init(&mut app);
            crate::register_bundled_fonts(&mut app);
            crate::theme::service::initialize_theme(&mut app)?;
        }
        let (response_sender, response_receiver) = mpsc::sync_channel(100);
        Ok(Self {
            cx,
            bootstrap,
            main: None,
            mounted: BTreeMap::new(),
            sdk_controls: BTreeMap::new(),
            sdk_prompt_controls: BTreeMap::new(),
            dictation_controls: BTreeMap::new(),
            flow_controls: BTreeMap::new(),
            theme_fixture: Default::default(),
            frames: Default::default(),
            preview: Default::default(),
            response_sender,
            response_receiver,
            qualified: false,
            ended: false,
            cleanup_failure: None,
            registry_revision: 0,
            last_progress_at: Instant::now(),
            retired_search_gate: None,
        })
    }

    pub(super) fn tick(&mut self, paint: bool) -> Result<()> {
        self.tick_with_timer_progress(paint, false)
    }

    /// Explicit waits own timer progress even when ordinary search inspection
    /// pins GPUI time. The search fixture's separate logical clock is unchanged.
    pub(super) fn tick_explicit_wait(&mut self, started: Instant) -> Result<()> {
        if self.search_gate().is_some() {
            // Frozen time preceding this wait is not part of its progress budget.
            self.last_progress_at = self.last_progress_at.max(started);
        }
        self.tick_with_timer_progress(true, true)
    }

    fn tick_with_timer_progress(&mut self, paint: bool, explicit_wait: bool) -> Result<()> {
        // Every caller shares one wall-clock cursor. Retain time beyond the
        // bounded slice instead of double-counting nested pumps or fast-forwarding
        // animation timers with a fixed duration on each busy-loop iteration.
        let elapsed = if self.search_gate().is_some() && !explicit_wait {
            self.last_progress_at = Instant::now();
            Duration::ZERO
        } else {
            let elapsed = self
                .last_progress_at
                .elapsed()
                .min(Duration::from_millis(50));
            self.last_progress_at += elapsed;
            elapsed
        };
        let remaining_frames = if paint {
            u64::from(self.bootstrap.limits.max_frames)
                .saturating_sub(self.cx.owned_hidden_observation().completed_frames)
        } else {
            0
        };
        self.observe_scheduled_frames()?;
        let progress = self.cx.pump_owned_work(256, elapsed, remaining_frames)?;
        self.check_scheduled_frames()?;
        ensure!(
            !progress.budget_exhausted
                || progress.pending_foreground_tasks
                    + progress.pending_background_tasks
                    + progress.pending_effects
                    + progress.pending_entity_releases
                    <= 8192,
            "evaluation_pending_work_exhausted"
        );
        let native = self.cx.owned_hidden_observation();
        ensure!(
            native.live_windows <= u64::from(self.bootstrap.limits.max_windows),
            "evaluation_window_budget_exhausted"
        );
        ensure!(
            native.completed_frames <= u64::from(self.bootstrap.limits.max_frames),
            "evaluation_frame_budget_exhausted"
        );
        self.sync_registered_windows()?;
        Ok(())
    }

    pub(super) fn search_gate(&self) -> Option<Arc<super::search_fixtures::SearchGate>> {
        self.main
            .as_ref()?
            .read(&self.cx.app.borrow())
            .main_services
            .search_gate()
    }

    pub(super) fn search_fixture_control(
        &mut self,
        mounted: &Mounted,
        command: crate::protocol::SearchFixtureCommand,
    ) -> Result<Value> {
        use crate::protocol::SearchFixtureCommand;
        ensure!(
            mounted.fixture_id == super::search_fixtures::FIXTURE_ID,
            "search_fixture_target_required"
        );
        let RootOwner::Main(entity) = &mounted.owner else {
            anyhow::bail!("search_fixture_main_required");
        };
        match command {
            SearchFixtureCommand::Prepare { scenario } => {
                let gate = mounted.handle.update(
                    &mut **self.cx.app.borrow_mut(),
                    |_, window, cx| {
                        entity.update(cx, |app, cx| -> Result<_> {
                            let gate = super::search_fixtures::prepare(app, &scenario, window, cx)?;
                            app.start_owned_search_catalogues(cx)?;
                            Ok(gate)
                        })
                    },
                )??;
                let retired = self.retired_search_gate.take();
                if let Some(old) = retired {
                    gate.retain_retired_gate(old);
                }
                self.reset_search_frame_trace(&Self::instance(mounted)?)?;
            }
            SearchFixtureCommand::Release { run_ids } => {
                let gate = self.search_gate().context("search_fixture_not_prepared")?;
                gate.release(&run_ids)?;
                // All selected admissions become eligible before pumping any
                // receiver. Synchronous source invalidations are not workers.
                for admission in gate.take_released_source_changes() {
                    mounted
                        .handle
                        .update(&mut **self.cx.app.borrow_mut(), |_, _, cx| {
                            entity.update(cx, |app, cx| {
                                app.apply_owned_search_source_change(admission.source(), cx)
                            })
                        })??;
                    admission.finish_source_change();
                }
            }
            SearchFixtureCommand::Advance { milliseconds } => {
                let gate = self.search_gate().context("search_fixture_not_prepared")?;
                gate.advance(milliseconds)?;
                for source in gate.take_due_source_changes()? {
                    mounted
                        .handle
                        .update(&mut **self.cx.app.borrow_mut(), |_, _, cx| {
                            entity.update(cx, |app, cx| {
                                app.apply_owned_search_source_change(source, cx)
                            })
                        })??;
                }
                self.observe_scheduled_frames()?;
                let remaining = u64::from(self.bootstrap.limits.max_frames)
                    .saturating_sub(self.cx.owned_hidden_observation().completed_frames);
                self.cx.pump_owned_work(
                    256,
                    Duration::from_millis(u64::from(milliseconds)),
                    remaining,
                )?;
                self.check_scheduled_frames()?;
            }
        }
        self.tick(true)?;
        let gate = self.search_gate().context("search_fixture_not_prepared")?;
        let pending = self.cx.pump_owned_work(0, Duration::ZERO, 0)?;
        Ok(
            json!({"searchProviders":gate.observation(),"suggestedInput":super::search_fixtures::suggested_input(gate.scenario()),
            "sourcePlans":super::search_fixtures::source_plans(gate.scenario())?,"fileViewInputs":super::search_fixtures::file_view_inputs()?,
            "pendingForegroundTasks":pending.pending_foreground_tasks,"pendingBackgroundTasks":pending.pending_background_tasks,
            "pendingEffects":pending.pending_effects,"pendingDirtyWindows":pending.pending_dirty_windows,
            "hasPendingTasksOrTimers":pending.has_pending_tasks_or_timers}),
        )
    }

    fn sync_registered_windows(&mut self) -> Result<()> {
        let revision =
            crate::windows::automation_runtime_handles::runtime_window_registry_revision();
        if revision == self.registry_revision {
            return Ok(());
        }
        let instances = crate::windows::automation_runtime_handles::runtime_window_instances();
        let stale: Vec<_> = self
            .mounted
            .iter()
            .filter(|(id, mounted)| {
                !instances.iter().any(|(live_id, generation, _)| {
                    live_id == *id && mounted.info.generation == Some(*generation)
                })
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in stale {
            if let Some(mounted) = self.mounted.remove(&id) {
                if let Some(generation) = mounted.info.generation {
                    forget_owned_render_frame(&id, generation);
                }
            }
            self.sdk_controls.remove(&id);
            self.sdk_prompt_controls.remove(&id);
            self.dictation_controls.remove(&id);
            self.flow_controls.remove(&id);
            if id == "main" {
                self.main = None;
            }
        }
        let mut result = Ok(());
        for (id, generation, policy) in instances {
            let registration = (|| -> Result<()> {
                ensure!(
                    policy == WindowHostPolicy::OwnedHidden,
                    "foreign_runtime_window"
                );
                if let Some(mounted) = self.mounted.get(&id) {
                    ensure!(
                        mounted.info.generation == Some(generation)
                            && crate::windows::get_runtime_window_handle_for_generation(
                                &id, generation
                            ) == Some(mounted.handle),
                        "stale_runtime_handle"
                    );
                    return Ok(());
                }
                let info = crate::windows::automation_window_by_id(&id)
                    .context("runtime_metadata_missing")?;
                ensure!(
                    info.generation == Some(generation) && !info.visible && !info.focused,
                    "invalid_owned_window_metadata"
                );
                let handle =
                    crate::windows::get_runtime_window_handle_for_generation(&id, generation)
                        .context("runtime_handle_missing")?;
                let owner = match info.semantic_surface.as_deref() {
                    Some("footerOverlay") => RootOwner::Footer,
                    Some("shortcutRecorder") => RootOwner::ShortcutRecorder,
                    _ => RootOwner::RegisteredChild,
                };
                self.mounted.insert(
                    id,
                    Mounted {
                        fixture_id: String::new(),
                        info,
                        handle,
                        owner,
                    },
                );
                Ok(())
            })();
            result = result.and(registration);
        }
        if result.is_ok() {
            self.registry_revision = revision;
        }
        result
    }

    fn paint_mounted(&mut self, target: &AutomationWindowTarget) -> Result<()> {
        ensure!(
            self.cx.owned_hidden_observation().completed_frames
                < u64::from(self.bootstrap.limits.max_frames),
            "evaluation_frame_budget_exhausted"
        );
        let mounted = self.resolve(target)?.clone();
        mounted
            .handle
            .update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                window.refresh();
                window.draw_owned_frame(cx, |_, _| Ok(()))
            })??;
        // Footer projections publish after their real draw, outside the window
        // borrow. Commit those effects without spending another frame permit.
        self.cx.app.borrow_mut().flush_owned_effects(256)?;
        self.sync_registered_windows()
    }

    pub(super) fn ended(&self) -> bool {
        self.ended
    }

    pub(super) fn resolve(&self, target: &AutomationWindowTarget) -> Result<&Mounted> {
        let AutomationWindowTarget::Instance { id, generation } = target else {
            anyhow::bail!("exact_instance_required");
        };
        let mounted = self.mounted.get(id).context("target_not_mounted")?;
        ensure!(
            *generation > 0 && mounted.info.generation == Some(*generation),
            "stale_window_generation"
        );
        ensure!(
            crate::windows::get_runtime_window_handle_for_generation(id, *generation)
                == Some(mounted.handle),
            "stale_runtime_handle"
        );
        ensure!(
            crate::windows::automation_window_by_id(id).and_then(|info| info.generation)
                == Some(*generation),
            "stale_window_metadata"
        );
        Ok(mounted)
    }

    pub(super) fn instance(mounted: &Mounted) -> Result<AutomationWindowTarget> {
        Ok(AutomationWindowTarget::Instance {
            id: mounted.info.id.clone(),
            generation: mounted
                .info
                .generation
                .context("window_generation_missing")?,
        })
    }

    pub(super) fn snapshot_for(
        mounted: &Mounted,
        window: &Window,
        cx: &App,
    ) -> Result<AutomationTargetIdentitySnapshot> {
        let info =
            crate::windows::automation_surface_collector::current_surface_metadata(&mounted.info)
                .context("window_metadata_missing")?;
        let main = match &mounted.owner {
            RootOwner::Main(entity) => Some(entity),
            _ => None,
        };
        crate::live_gpui_target_identity(main, &info, window, cx)
    }

    pub(super) fn identity(
        &mut self,
        target: &AutomationWindowTarget,
    ) -> Result<AutomationTargetIdentitySnapshot> {
        let mounted = self.resolve(target)?.clone();
        mounted
            .handle
            .update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                Self::snapshot_for(&mounted, window, cx)
            })?
    }

    pub(super) fn validate_expected(
        &mut self,
        target: &AutomationWindowTarget,
        expected: &AutomationTargetIdentitySnapshot,
    ) -> Result<()> {
        let actual = self.identity(target)?;
        ensure!(
            actual.window_id == expected.window_id
                && actual.window_generation == expected.window_generation
                && actual.app_view_variant == expected.app_view_variant
                && actual.target_generation == expected.target_generation
                && actual.surface_generation == expected.surface_generation
                && actual.data_generation == expected.data_generation
                && actual.presentation_revision == expected.presentation_revision
                && actual.theme_revision == expected.theme_revision,
            "stale_target_identity"
        );
        ensure!(
            expected.frame_generation.is_some()
                && actual.frame_generation >= expected.frame_generation,
            "stale_frame_identity"
        );
        Ok(())
    }

    fn descriptor(
        id: &str,
        kind: AutomationWindowKind,
        surface: &str,
        bounds: Bounds<Pixels>,
    ) -> AutomationWindowInfo {
        AutomationWindowInfo {
            id: id.into(),
            kind,
            title: Some(surface.into()),
            focused: false,
            visible: false,
            semantic_surface: Some(surface.into()),
            bounds: Some(crate::automation_window_bounds_from_gpui(bounds)),
            parent_window_id: None,
            parent_window_generation: None,
            parent_kind: None,
            generation: None,
            pid: Some(std::process::id()),
        }
    }

    pub(super) fn ensure_main(&mut self) -> Result<Mounted> {
        if let Some(main) = self.mounted.get("main") {
            return Ok(main.clone());
        }
        conversation_fixtures::seed_owned_flow_catalogue()?;
        notes_fixtures::prepare_notes_storage()?;
        let config = crate::config::Config::default();
        let snapshot = crate::theme::get_theme_snapshot();
        let data = crate::MainInitialData::owned_fixture(
            &config,
            &self.bootstrap.root,
            snapshot.theme.clone(),
            snapshot.revision,
        );
        let services = crate::MainServices::OwnedFixtures(Arc::new(
            crate::OwnedMainSources::launcher(&self.bootstrap.root),
        ));
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(750.0), px(520.0)));
        let options = crate::main_window_options(
            bounds,
            crate::WindowBackgroundAppearance::Transparent,
            WindowHostPolicy::OwnedHidden,
        )?;
        let sender = self.response_sender.clone();
        let mut entity = None;
        let handle = self
            .cx
            .open_owned_hidden_window_fallible(options, |window, cx| {
                let app = cx.new(|cx| {
                    ScriptListApp::from_initial_data(
                        config, false, data, services, sender, window, cx,
                    )
                });
                entity = Some(app.clone());
                Ok(cx.new(|cx| Root::new_transparent(app, window, cx)))
            })?;
        let entity = entity.context("main_root_not_created")?;
        let info = crate::register_main_runtime_window(
            handle.into(),
            bounds,
            &mut self.cx.app.borrow_mut(),
        )?;
        self.main = Some(entity.clone());
        let mounted = Mounted {
            fixture_id: "main.script-list".into(),
            info,
            handle: handle.into(),
            owner: RootOwner::Main(entity),
        };
        self.mounted.insert("main".into(), mounted.clone());
        Ok(mounted)
    }

    fn finish_registered_presentation(
        &mut self,
        fixture_id: &str,
        info: AutomationWindowInfo,
    ) -> Result<AutomationTargetIdentitySnapshot> {
        self.sync_registered_windows()?;
        let target = AutomationWindowTarget::Instance {
            id: info.id.clone(),
            generation: info.generation.context("presentation_generation_missing")?,
        };
        self.resolve(&target)?;
        self.mounted
            .get_mut(&info.id)
            .context("presentation_lifetime_missing")?
            .fixture_id = fixture_id.into();
        self.paint_mounted(&target)?;
        self.identity(&target)
    }

    fn mount(
        &mut self,
        fixture_id: &str,
        parent: Option<AutomationWindowTarget>,
    ) -> Result<AutomationTargetIdentitySnapshot> {
        ensure!(
            self.bootstrap.fixture_ids.contains(fixture_id),
            "fixture_outside_launch_subset"
        );
        let descriptor = catalog::fixture(fixture_id).context("unknown_fixture")?;
        if descriptor.family == "dictationMicrophone" {
            let parent = self
                .resolve(parent.as_ref().context("dictation_parent_required")?)?
                .clone();
            let RootOwner::Dictation(view) = &parent.owner else {
                anyhow::bail!("dictation_parent_required");
            };
            let info = dictation_fixtures::open_owned_dictation_microphone_fixture(
                view,
                &parent.info,
                parent.handle,
                &mut self.cx.app.borrow_mut(),
            )?;
            return self.finish_registered_presentation(fixture_id, info);
        }
        if matches!(
            descriptor.family.as_str(),
            "notesAuxiliary" | "dayPageAuxiliary"
        ) {
            let parent = self
                .resolve(parent.as_ref().context("auxiliary_parent_required")?)?
                .clone();
            let info = match (&parent.owner, descriptor.family.as_str()) {
                (RootOwner::Notes(entity), "notesAuxiliary") => {
                    notes_fixtures::mount_notes_fixture_presentation(
                        fixture_id,
                        entity,
                        &parent.info,
                        parent.handle,
                        &mut self.cx.app.borrow_mut(),
                    )?
                }
                (RootOwner::Main(entity), "dayPageAuxiliary") => {
                    let day = match &entity.read(&self.cx.app.borrow()).current_view {
                        AppView::DayPage { entity } => entity.clone(),
                        _ => anyhow::bail!("day_page_parent_required"),
                    };
                    notes_fixtures::mount_day_page_fixture_presentation(
                        fixture_id,
                        &day,
                        &parent.info,
                        parent.handle,
                        &mut self.cx.app.borrow_mut(),
                    )?
                }
                _ => anyhow::bail!("auxiliary_parent_owner_mismatch"),
            };
            return self.finish_registered_presentation(fixture_id, info);
        }
        if descriptor.root == "main" && descriptor.family != "agentChatPopup" {
            let mut mounted = self.ensure_main()?;
            let RootOwner::Main(entity) = mounted.owner.clone() else {
                anyhow::bail!("main_owner_mismatch");
            };
            let mut sdk_control = None;
            let mut flow_control = None;
            mounted.handle.update(
                &mut **self.cx.app.borrow_mut(),
                |_, window, cx| -> Result<()> {
                    if fixture_id != super::search_fixtures::FIXTURE_ID {
                        entity.update(cx, |app, _| super::search_fixtures::retire(app))?;
                    }
                    window.resize(size(px(750.0), px(520.0)));
                    match descriptor.family.as_str() {
                        "main" => entity.update(cx, |app, cx| {
                            if fixture_id == super::search_fixtures::FIXTURE_ID {
                                super::search_fixtures::prepare(app, "tab-domain-hoist", window, cx)
                                    .map(|_| ())
                            } else {
                                main_fixtures::mount_main_fixture(app, fixture_id, window, cx)
                            }
                        })?,
                        "mainOverlay" => entity.update(cx, |app, cx| {
                            main_fixtures::mount_main_overlay_fixture(app, fixture_id, window, cx)
                        })?,
                        "prompt" => entity.update(cx, |app, cx| {
                            let seed =
                                prompt_fixtures::prompt_fixture_seed(fixture_id, &app.theme)?;
                            app.mount_prompt_seed(seed, window, cx).map(|_| ())
                        })?,
                        "dayPage" => {
                            let view = notes_fixtures::create_day_page_fixture(
                                fixture_id,
                                entity.clone(),
                                window,
                                cx,
                            )?;
                            view.update(cx, |view, cx| view.focus_editor(window, cx));
                            entity.update(cx, |app, cx| {
                                app.transition_current_view_and_rekey_main_automation_surface(
                                    AppView::DayPage { entity: view },
                                );
                                cx.notify();
                            });
                        }
                        "agentChat" => {
                            let view = conversation_fixtures::create_agent_chat_fixture(
                                fixture_id, window, cx,
                            )?;
                            entity.update(cx, |app, cx| {
                                app.transition_current_view_and_rekey_main_automation_surface(
                                    AppView::AgentChatView { entity: view },
                                );
                                cx.notify();
                            });
                        }
                        "flow" => {
                            flow_control = entity
                                .update(cx, |app, cx| app.mount_flow_fixture(fixture_id, cx))?;
                        }
                        "sdkChat" => {
                            let (view, control) = conversation_fixtures::create_sdk_chat_fixture(
                                fixture_id, window, cx,
                            )?;
                            sdk_control = Some(control);
                            entity.update(cx, |app, cx| {
                                app.transition_current_view_and_rekey_main_automation_surface(
                                    AppView::ChatPrompt {
                                        id: fixture_id.into(),
                                        entity: view,
                                    },
                                );
                                cx.notify();
                            });
                        }
                        _ => anyhow::bail!("catalogue_main_owner_mismatch"),
                    }
                    entity.update(cx, |app, cx| app.bind_owned_surface_revision_observers(cx));
                    Ok(())
                },
            )??;
            if let Some(control) = sdk_control {
                self.sdk_controls.insert("main".into(), control);
            }
            self.flow_controls.remove("main");
            if let Some(session_id) = flow_control {
                self.flow_controls.insert("main".into(), session_id);
            }
            mounted.fixture_id = fixture_id.into();
            let target = Self::instance(&mounted)?;
            self.mounted.insert("main".into(), mounted);
            self.paint_mounted(&target)?;
            if fixture_id == super::search_fixtures::FIXTURE_ID {
                self.reset_search_frame_trace(&target)?;
            }
            return self.identity(&target);
        }
        let mounted = match descriptor.family.as_str() {
            "notes" => {
                let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(750.0), px(520.0)));
                let options = crate::notes::window::window_ops::notes_window_options(
                    bounds,
                    WindowHostPolicy::OwnedHidden,
                )?;
                let mut entity = None;
                let handle: WindowHandle<Root> =
                    self.cx
                        .open_owned_hidden_window_fallible(options, |window, cx| {
                            let view =
                                notes_fixtures::create_notes_fixture(fixture_id, window, cx)?;
                            entity = Some(view.clone());
                            Ok(cx.new(|cx| Root::new(view, window, cx)))
                        })?;
                let entity = entity.context("notes_root_not_created")?;
                let info = crate::windows::register_runtime_window_instance(
                    Self::descriptor("notes", AutomationWindowKind::Notes, "notes", bounds),
                    handle.into(),
                    &mut self.cx.app.borrow_mut(),
                )?;
                crate::notes::window::window_ops::bind_owned_notes_host(
                    handle,
                    entity.clone(),
                    info.generation.context("notes_generation_missing")?,
                    &mut self.cx.app.borrow_mut(),
                )?;
                Mounted {
                    fixture_id: fixture_id.into(),
                    info,
                    handle: handle.into(),
                    owner: RootOwner::Notes(entity),
                }
            }
            "agentChat" => {
                let entity = conversation_fixtures::open_detached_agent_chat_fixture(
                    &mut self.cx.app.borrow_mut(),
                )?;
                let id = format!("agentChatDetached:{fixture_id}");
                let info = crate::windows::automation_window_by_id(&id)
                    .context("detached_chat_registration_missing")?;
                let handle = crate::windows::get_runtime_window_handle_for_generation(
                    &id,
                    info.generation.context("chat_generation_missing")?,
                )
                .context("chat_runtime_missing")?;
                Mounted {
                    fixture_id: fixture_id.into(),
                    info,
                    handle,
                    owner: RootOwner::AgentChat(entity),
                }
            }
            "dictation" => {
                let (entity, info) = dictation_fixtures::open_owned_dictation_fixture(
                    fixture_id,
                    &mut self.cx.app.borrow_mut(),
                )?;
                let generation = info.generation.context("dictation_generation_missing")?;
                let handle =
                    crate::windows::get_runtime_window_handle_for_generation(&info.id, generation)
                        .context("dictation_runtime_missing")?;
                Mounted {
                    fixture_id: fixture_id.into(),
                    info,
                    handle,
                    owner: RootOwner::Dictation(entity),
                }
            }
            "actions" | "confirm" | "hud" | "snap" => {
                let parent = if descriptor.parent_fixture_id.is_some() {
                    let mounted = match parent.as_ref() {
                        Some(target) => self.resolve(target)?.clone(),
                        None => self.ensure_main()?,
                    };
                    Some(secondary_fixtures::SecondaryFixtureParent {
                        id: mounted.info.id.clone(),
                        generation: mounted
                            .info
                            .generation
                            .context("parent_generation_missing")?,
                        handle: mounted.handle,
                    })
                } else {
                    None
                };
                let fixture = Rc::new(secondary_fixtures::mount_secondary_fixture(
                    fixture_id,
                    parent,
                    &mut self.cx.app.borrow_mut(),
                )?);
                let info = crate::windows::automation_window_by_id(&fixture.automation_id)
                    .context("secondary_registration_missing")?;
                Mounted {
                    fixture_id: fixture_id.into(),
                    info,
                    handle: fixture.handle,
                    owner: RootOwner::Secondary(fixture),
                }
            }
            "footer" | "shortcutRecorder" | "agentChatPopup" => {
                self.mount_auxiliary(fixture_id, &descriptor.family, parent.as_ref())?
            }
            _ => anyhow::bail!("catalogue_root_owner_mismatch"),
        };
        let target = Self::instance(&mounted)?;
        if let Some(existing) = self.mounted.get(&mounted.info.id) {
            ensure!(
                existing.info.generation == mounted.info.generation
                    && existing.handle == mounted.handle,
                "duplicate_mounted_window"
            );
        }
        self.mounted.insert(mounted.info.id.clone(), mounted);
        self.paint_mounted(&target)?;
        self.identity(&target)
    }

    pub(super) fn enqueue_main(&mut self, raw: &Value) -> Result<()> {
        let mounted = self
            .mounted
            .get("main")
            .context("main_root_not_mounted")?
            .clone();
        self.resolve(&Self::instance(&mounted)?)?;
        let RootOwner::Main(entity) = mounted.owner else {
            anyhow::bail!("main_owner_mismatch");
        };
        let message: crate::protocol::Message = serde_json::from_value(raw.clone())?;
        crate::windows::with_runtime_window_dispatch(mounted.handle, || {
            mounted
                .handle
                .update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                    entity.update(cx, |app, cx| {
                        app.handle_stdin_protocol_message(message, window, cx)
                    });
                })
        })?;
        Ok(())
    }

    pub(super) fn forward_main(
        &mut self,
        request_id: &str,
        raw: &Value,
        allow_progress: bool,
    ) -> Result<Value> {
        let explicit_wait = match raw["type"].as_str() {
            Some("waitFor") => true,
            Some("batch") => raw["commands"].as_array().is_some_and(|commands| {
                commands
                    .iter()
                    .any(|command| command["type"].as_str() == Some("waitFor"))
            }),
            _ => false,
        };
        self.enqueue_main(raw)?;
        let started = Instant::now();
        let deadline = started + Duration::from_secs(5);
        loop {
            match self.response_receiver.try_recv() {
                Ok(message) => {
                    let response = serde_json::to_value(message)?;
                    ensure!(
                        response["requestId"].as_str() == Some(request_id),
                        "protocol_response_correlation_mismatch"
                    );
                    return Ok(response);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    anyhow::bail!("protocol_response_channel_closed")
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ensure!(allow_progress, "atomic_query_response_deferred");
                }
            }
            ensure!(Instant::now() < deadline, "protocol_response_timeout");
            if explicit_wait {
                self.tick_explicit_wait(started)?;
            } else {
                self.tick(true)?;
            }
        }
    }

    pub(super) fn completed_frame(
        &mut self,
        target: &AutomationWindowTarget,
        expected: &AutomationTargetIdentitySnapshot,
        after: u64,
    ) -> Result<(CompletedFrameIdentity, BTreeMap<String, f64>)> {
        self.validate_expected(target, expected)?;
        let mounted = self.resolve(target)?.clone();
        let generation = mounted
            .info
            .generation
            .context("window_generation_missing")?;
        forget_owned_render_frame(&mounted.info.id, generation);
        let started = Instant::now();
        let deadline = started + Duration::from_secs(5);
        let rendered = loop {
            ensure!(Instant::now() < deadline, "completed_frame_timeout");
            self.tick(true)?;
            self.resolve(target)?;
            ensure!(
                self.cx.owned_hidden_observation().completed_frames
                    < u64::from(self.bootstrap.limits.max_frames),
                "evaluation_frame_budget_exhausted"
            );
            let (rendered, resources) =
                mounted
                    .handle
                    .update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                        window.refresh();
                        window.draw_owned_frame(cx, |window, cx| {
                            Ok::<_, anyhow::Error>((
                                Self::snapshot_for(&mounted, window, cx)?,
                                window.owned_render_resource_status(),
                            ))
                        })
                    })??;

            // The draw stamp belongs to these pixels, not to state subsequently
            // changed by observers/deferred effects. Drain those effects outside
            // the window borrow, and redraw rather than relabeling an old scene.
            self.cx.app.borrow_mut().flush_owned_effects(256)?;
            let pending = self.cx.pump_owned_work(0, Duration::ZERO, 0)?;
            // Queued scheduler work can belong to an ongoing stream or the next
            // animation frame. It is not evidence that these completed pixels
            // are stale; the exact owner/scene identity and resources decide.
            if pending.pending_effects != 0 || self.identity(target)? != rendered {
                continue;
            }
            ensure!(resources.failed == 0, "owned_render_asset_failed");
            if resources.pending != 0 {
                continue;
            }
            ensure!(Instant::now() < deadline, "completed_frame_timeout");
            // Future animation callbacks/timers are deliberately not a global
            // quiescence condition. A notification for the next animation frame
            // may dirty this window without changing the owner of this scene.
            // Its callback remains scheduled; capture never repaints it for us.
            break rendered;
        };
        ensure!(
            rendered.frame_generation.is_some_and(|frame| frame > after),
            "frame_not_advanced"
        );
        ensure!(
            rendered.theme_revision == Some(crate::theme::service::theme_revision()),
            "owner_theme_not_applied"
        );
        let layout_paint = started.elapsed().as_secs_f64() * 1000.0;
        let mut record = if let Some(record) = self.take_observed_search_frame(&rendered)? {
            record
        } else {
            mounted
                .handle
                .update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                    ensure!(
                        Self::snapshot_for(&mounted, window, cx)? == rendered,
                        "capture_frame_identity_stale"
                    );
                    let readback_started = Instant::now();
                    let image = window.render_to_image()?;
                    let gpu_readback = readback_started.elapsed().as_secs_f64() * 1000.0;
                    Ok::<_, anyhow::Error>(OwnedCompletedRenderFrame {
                        identity: CompletedFrameIdentity {
                            runtime: self.bootstrap.identity.clone(),
                            requested_target: target.clone(),
                            target: rendered,
                            native_window_id: None,
                        },
                        image,
                        scale_factor: window.scale_factor(),
                        phase_durations_ms: BTreeMap::from([
                            ("layoutPaint".into(), layout_paint),
                            ("gpuReadback".into(), gpu_readback),
                        ]),
                    })
                })??
        };
        record
            .phase_durations_ms
            .insert("layoutPaint".into(), layout_paint);
        let identity = record.identity.clone();
        let phases = record.phase_durations_ms.clone();
        let current_identity = Rc::new(move |cx: &mut App| {
            mounted
                .handle
                .update(cx, |_, window, cx| Self::snapshot_for(&mounted, window, cx))?
        });
        publish_owned_render_frame(record, current_identity, &mut self.cx.app.borrow_mut())?;
        Ok((identity, phases))
    }

    fn capture(&mut self, request_id: &str, raw: &Value) -> Result<Value> {
        let mut request: ComputerUseCaptureRenderWindowRequest =
            serde_json::from_value(raw["request"].clone())?;
        request.correlation_id = request_id.to_owned();
        let expected = request
            .expected
            .as_ref()
            .context("expected_identity_required")?;
        self.validate_expected(&request.target, expected)?;
        let snapshot =
            capture_render_window_on_gpui_thread(&request, &mut self.cx.app.borrow_mut())
                .map_err(|error| anyhow::anyhow!(error.error_code()))?;
        Ok(json!({"type":"captureRenderWindowResult","snapshot":snapshot}))
    }

    fn pack_frame_search_metadata(stamp: &mut Value) -> Result<()> {
        ensure!(
            stamp.get("searchMetadataRef").is_none(),
            "frame_search_metadata_ref_invalid"
        );
        let Some(search) = stamp.get("search").filter(|value| value.is_object()) else {
            return Ok(());
        };
        let Some(bindings) = stamp.get("paintBindings").and_then(Value::as_array) else {
            return Ok(());
        };
        let mut roots = bindings.iter().enumerate().filter(|(_, binding)| {
            binding["kind"] == "mainSearch" && binding["id"] == "main-search"
        });
        let Some((index, binding)) = roots.next() else {
            return Ok(());
        };
        ensure!(roots.next().is_none(), "search_paint_binding_ambiguous");
        if binding.get("metadata") != Some(search) {
            return Ok(());
        }
        let fields = stamp
            .as_object_mut()
            .context("frame_search_metadata_ref_invalid")?;
        fields.remove("search");
        fields.insert("searchMetadataRef".into(), json!(index));
        Ok(())
    }

    pub(super) fn pack_search_metadata_refs(page: &mut Value) -> Result<()> {
        Self::pack_frame_search_metadata(page)?;
        if let Some(stamps) = page
            .get_mut("completedFrames")
            .and_then(Value::as_array_mut)
        {
            for stamp in stamps {
                Self::pack_frame_search_metadata(stamp)?;
            }
        }
        Ok(())
    }

    fn pack_capture_frame_histories(capture: &mut Value, state: &mut Value) -> Result<Value> {
        let Value::Array(mut capture_frames) = capture["completedFrames"].take() else {
            anyhow::bail!("capture_frame_history_invalid");
        };
        let Value::Array(mut state_frames) = state["completedFrames"].take() else {
            anyhow::bail!("capture_frame_history_invalid");
        };
        let capture_count = capture_frames.len();
        let state_count = state_frames.len();
        let generation = |stamp: &Value| {
            stamp
                .pointer("/frame/target/frameGeneration")
                .and_then(Value::as_u64)
                .context("capture_frame_history_invalid")
        };
        let current_generation = generation(capture)?;
        let same_current = |stamp: &Value| {
            stamp["frame"] == capture["frame"]
                && stamp.as_object().is_some_and(|fields| {
                    fields
                        .iter()
                        .all(|(key, value)| capture.get(key) == Some(value))
                })
        };
        let mut conflict = false;
        capture_frames.retain(|stamp| match generation(stamp) {
            Ok(id) if id == current_generation => {
                let matches = same_current(stamp);
                conflict |= !matches;
                !matches
            }
            Ok(_) => true,
            Err(_) => {
                conflict = true;
                true
            }
        });
        ensure!(!conflict, "capture_frame_history_conflict");
        let mut represented = BTreeMap::new();
        for stamp in &capture_frames {
            ensure!(
                represented.insert(generation(stamp)?, stamp).is_none(),
                "capture_frame_history_conflict"
            );
        }
        state_frames.retain(|stamp| match generation(stamp) {
            Ok(id) if id == current_generation => {
                let matches = same_current(stamp);
                conflict |= !matches;
                !matches
            }
            Ok(id) => match represented.get(&id) {
                Some(existing) => {
                    let matches = existing["frame"] == stamp["frame"] && *existing == stamp;
                    conflict |= !matches;
                    !matches
                }
                None => true,
            },
            Err(_) => {
                conflict = true;
                true
            }
        });
        ensure!(!conflict, "capture_frame_history_conflict");
        drop(represented);
        capture["completedFrames"] = Value::Array(capture_frames);
        state["completedFrames"] = Value::Array(state_frames);
        capture["historyScope"] = json!("captureBundle");
        state["historyScope"] = json!("captureBundle");
        Ok(json!({"version":1,"captureFrameCount":capture_count,"stateFrameCount":state_count}))
    }

    fn capture_frame(
        &mut self,
        request_id: &str,
        target: &AutomationWindowTarget,
        include_image: bool,
        scheduled: Option<crate::protocol::ScheduledFrameRequirement>,
        frame_cursor: Option<crate::protocol::OwnedFrameCursor>,
    ) -> Result<Value> {
        if let Some(cursor) = frame_cursor {
            self.validate_frame_cursor(target, cursor)?;
        }
        // Resolve the live instance now, not a revision observed in an earlier
        // command. Complete and read back its frame without yielding to ingress.
        let (frame, mut phases) = if let Some(requirement) = scheduled.as_ref() {
            self.scheduled_completed_frame(target, requirement)?
        } else {
            let expected = self.identity(target)?;
            let after = expected
                .frame_generation
                .context("frame_generation_missing")?;
            self.completed_frame(target, &expected, after)?
        };
        let state_frame_cursor = match frame_cursor {
            Some(cursor) => Some(cursor),
            None => scheduled
                .as_ref()
                .map(|requirement| {
                    self.current_frame_cursor(target, requirement.after_frame_generation)
                })
                .transpose()?,
        };
        let mut observe = |facet, operation| -> Result<Value> {
            let observation_id = format!("{request_id}:{facet}");
            let mut query = json!({
                "type":operation,"requestId":observation_id,"target":target
            });
            if operation == "getState" {
                if let Some(cursor) = state_frame_cursor {
                    query["frameCursor"] = serde_json::to_value(cursor)?;
                }
            }
            let observation =
                self.query(&observation_id, &query, QueryMode::CompletedFrame(&frame))?;
            ensure!(
                observation["requestId"].as_str() == Some(observation_id.as_str()),
                "protocol_response_correlation_mismatch"
            );
            let observed: AutomationTargetIdentitySnapshot =
                serde_json::from_value(observation["targetIdentity"].clone())?;
            ensure!(observed == frame.target, "capture_frame_identity_stale");
            Ok(observation)
        };
        let mut state = observe("state", "getState")?;
        let elements = observe("elements", "getElements")?;
        let layout = observe("layout", "getLayoutInfo")?;
        let request = ComputerUseCaptureRenderWindowRequest {
            target: target.clone(),
            expected: Some(frame.target.clone()),
            hi_dpi: true,
            include_image,
            correlation_id: request_id.to_owned(),
            probes: Vec::new(),
        };
        let snapshot =
            capture_render_window_on_gpui_thread(&request, &mut self.cx.app.borrow_mut())
                .map_err(|error| anyhow::anyhow!(error.error_code()))?;
        phases.extend(
            snapshot
                .phase_durations_ms
                .iter()
                .map(|(name, ms)| (name.clone(), *ms)),
        );
        let mut frame_evidence = self.frame_evidence(
            &frame,
            scheduled
                .as_ref()
                .map(|requirement| requirement.after_frame_generation),
            frame_cursor,
        )?;
        let bundle = if frame_cursor.is_some() {
            Some(Self::pack_capture_frame_histories(
                &mut frame_evidence,
                &mut state["frameEvidence"],
            )?)
        } else {
            None
        };
        if frame_cursor.is_some() {
            Self::pack_search_metadata_refs(&mut frame_evidence)?;
            Self::pack_search_metadata_refs(&mut state["frameEvidence"])?;
        }
        let mut result = json!({"operation":"captureFrame","ok":true,"frame":frame,
            "snapshot":snapshot,"state":state,"elements":elements,"layout":layout,"phaseDurationsMs":phases,"frameEvidence":frame_evidence});
        if let Some(bundle) = bundle {
            result["frameHistoryBundle"] = bundle;
        }
        Ok(result)
    }

    fn theme(&mut self, command: DesignCommand) -> Result<Value> {
        let started = Instant::now();
        let (operation, receipt) = match command {
            DesignCommand::ApplyTheme {
                expected_revision,
                edits,
            } => {
                ensure!(
                    crate::theme::service::theme_revision() == expected_revision,
                    "stale_theme_revision"
                );
                (
                    "applyTheme",
                    self.preview
                        .apply(&mut self.cx.app.borrow_mut(), expected_revision, &edits)?,
                )
            }
            DesignCommand::RevertTheme { expected_revision } => {
                ensure!(
                    crate::theme::service::theme_revision() == expected_revision,
                    "stale_theme_revision"
                );
                (
                    "revertTheme",
                    self.preview.revert(&mut self.cx.app.borrow_mut())?,
                )
            }
            _ => anyhow::bail!("not_a_theme_command"),
        };
        Ok(
            json!({"operation":operation,"ok":true,"revision":receipt.revision,"previousRevision":receipt.previous_revision,
            "invalidations":crate::windows::automation_runtime_handles::theme_invalidations(receipt.revision),
            "resolved":crate::theme::get_theme_snapshot().resolved.values,
            "phaseDurationsMs":{"publication":started.elapsed().as_secs_f64()*1000.0}}),
        )
    }

    fn design(&mut self, request_id: &str, command: DesignCommand) -> Result<Value> {
        if let DesignCommand::Bootstrap {
            launch_nonce,
            policy_sha256,
        } = command
        {
            ensure!(
                !self.qualified
                    && launch_nonce == self.bootstrap.launch_nonce
                    && policy_sha256 == self.bootstrap.policy_sha256,
                "bootstrap_identity_mismatch"
            );
            ensure!(
                self.cx.owned_hidden_observation().installed
                    && crate::runtime_policy::is_owned_evaluation(),
                "bootstrap_guards_missing"
            );
            self.qualified = true;
            let guards: BTreeMap<_, _> = GUARDS.into_iter().map(|guard| (guard, true)).collect();
            return Ok(
                json!({"operation":"bootstrap","ok":true,"identity":self.bootstrap.identity,"launchNonce":launch_nonce,"policySha256":policy_sha256,"guards":guards,"limits":self.bootstrap.limits}),
            );
        }
        ensure!(self.qualified, "bootstrap_required");
        match command {
            DesignCommand::Catalog {} => {
                let targets = self.targets()?;
                Ok(
                    json!({"operation":"catalog","ok":true,"fixtures":catalog::fixtures(),"targets":targets,
                    "operations":["catalog","mount","getState","getElements","getLayoutInfo","batch","simulateGpuiEvent","waitFor","captureFrame","acknowledgeFrames","captureRenderWindow","applyTheme","revertTheme","fixtureControl","sdkPrompt","probeSafety","unmount","diagnose","end"],
                    "safetyProbes":crate::protocol::NativeSafetyProbe::ALL,
                    "searchFixtures":super::search_fixtures::catalogue(),
                    "reservedRequestIdPrefixes":[BATCH_STEP_REQUEST_ID_PREFIX],
                    "completedFrameWait":{"conditionType":"completedFrame","targetType":"instance","expectedOptional":true,"omittedExpectedObservesCurrent":true},
                    "responseEncoding":crate::protocol::OWNED_RESPONSE_CODEC,
                    "frameCursor":{"version":1,"operation":"getState","captureFrame":true,
                        "captureHistoryBundle":{"version":1,"requiresFrameCursor":true,"pageScope":"captureBundle","decodedScope":"complete"},
                        "searchMetadataRef":{"version":1,"paintBindingIndex":true}},
                    "frameAcknowledgement":{"version":1,"operation":"acknowledgeFrames",
                        "retainsCursorFrame":true,"readCursorsArePassive":true,"draws":false},
                    "searchProviderWait":{"version":1,"conditionType":"searchProvider","sources":super::search_fixtures::PROVIDERS,
                        "statuses":["admitted","blocked","settled","cached"],"sourceChange":"explicitFixtureControl",
                        "acceptCached":true,"cacheAfterRunId":0,"cacheSources":super::runtime_query::CACHE_READINESS_SOURCES},
                    "fileSearchStreamWait":{"version":1,"conditionType":"fileSearchStream",
                        "identityFields":["generation","query"],"terminalPhases":["completed","failed","cancelled","unavailable"]},
                    "fileSearchPreviewWait":{"version":1,"conditionType":"fileSearchPreview",
                        "identityFields":["generation","query","workSequence"],"phase":"held"},
                    "settings":{"themeRevision":crate::theme::service::theme_revision(),"limits":self.bootstrap.limits},"runtimeQualified":self.qualified}),
                )
            }
            DesignCommand::Mount { fixture_id, parent } => Ok(
                json!({"operation":"mount","ok":true,"fixtureId":fixture_id,"target":self.mount(&fixture_id,parent)?}),
            ),
            DesignCommand::CaptureFrame {
                target,
                include_image,
                scheduled,
                frame_cursor,
            } => self.capture_frame(request_id, &target, include_image, scheduled, frame_cursor),
            DesignCommand::AcknowledgeFrames {
                target,
                expected,
                cursor,
            } => {
                let mut result = self.acknowledge_frames(&target, &expected, cursor)?;
                result["operation"] = json!("acknowledgeFrames");
                result["ok"] = json!(true);
                Ok(result)
            }
            command @ (DesignCommand::ApplyTheme { .. } | DesignCommand::RevertTheme { .. }) => {
                self.theme(command)
            }
            DesignCommand::FixtureControl {
                target,
                expected,
                control,
            } => self.fixture_control(request_id, &target, &expected, control),
            DesignCommand::SdkPrompt {
                target,
                expected,
                command,
            } => self.sdk_prompt(&target, &expected, command),
            DesignCommand::ProbeSafety {
                target,
                expected,
                probe,
            } => self.probe_safety(&target, &expected, probe),
            DesignCommand::Unmount { target, expected } => {
                self.validate_expected(&target, &expected)?;
                self.unmount(&target)?;
                Ok(json!({"operation":"unmount","ok":true,"target":target,"closed":true}))
            }
            DesignCommand::Diagnose {} => {
                let targets = self.targets()?;
                let policy =
                    crate::runtime_policy::owned_evaluation().context("owned_policy_missing")?;
                let progress = self.cx.pump_owned_work(0, Duration::ZERO, 0)?;
                let native = self.cx.owned_hidden_observation();
                let copy_sink = policy.owned_copy_snapshot()?;
                Ok(
                    json!({"operation":"diagnose","ok":true,"identity":self.bootstrap.identity,"targets":targets,
                    "refusedEffects":policy.refused_effect_count(),"completedFixtureEffects":policy.completed_fixture_effect_count(),
                    "copySink":copy_sink,
                    "searchFixtures":super::search_fixtures::catalogue(),"searchProviders":self.search_gate().map(|gate|gate.observation()),
                    "reservedRequestIdPrefixes":[BATCH_STEP_REQUEST_ID_PREFIX],
                    "completedFrameWait":{"conditionType":"completedFrame","targetType":"instance","expectedOptional":true,"omittedExpectedObservesCurrent":true},
                    "pendingEffects":progress.pending_effects,"pendingForegroundTasks":progress.pending_foreground_tasks,
                    "pendingBackgroundTasks":progress.pending_background_tasks,"pendingDirtyWindows":progress.pending_dirty_windows,
                    "pendingEntityReleases":progress.pending_entity_releases,
                    "hasPendingTasksOrTimers":progress.has_pending_tasks_or_timers,"framesCompleted":native.completed_frames,
                    "native":{"installed":native.installed,"openedWindows":native.opened_windows,"liveWindows":native.live_windows,
                        "completedFrames":native.completed_frames,"readbackImages":native.readback_images,"refusedOperations":native.refused_operations}}),
                )
            }
            DesignCommand::End {} => {
                self.close()?;
                let native = self.cx.owned_hidden_observation();
                Ok(
                    json!({"operation":"end","ok":true,"ownedWindowsClosed":self.ended,"remainingWindows":native.live_windows,
                    "nativeRefusedOperations":native.refused_operations,"refusedEffects":crate::runtime_policy::owned_evaluation().map(|policy|policy.refused_effect_count())}),
                )
            }
            DesignCommand::Bootstrap { .. } => anyhow::bail!("bootstrap_already_completed"),
        }
    }

    pub(super) fn targets(&mut self) -> Result<Vec<AutomationTargetIdentitySnapshot>> {
        let targets: Vec<_> = self
            .mounted
            .values()
            .map(Self::instance)
            .collect::<Result<_>>()?;
        targets.iter().map(|target| self.identity(target)).collect()
    }

    pub(super) fn request(&mut self, request_id: &str, raw: Value) -> Value {
        let command_type = raw["type"].as_str().unwrap_or("");
        let operation = raw["command"]["operation"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        let result = (|| -> Result<Value> {
            ensure!(
                !request_id.starts_with(BATCH_STEP_REQUEST_ID_PREFIX),
                "evaluation_reserved_request_id"
            );
            ensure!(
                command_type == "getState" || raw.get("frameCursor").is_none(),
                "frame_cursor_invalid"
            );
            if command_type == "design" {
                let command: DesignCommand = serde_json::from_value(raw["command"].clone())?;
                return Ok(
                    json!({"type":"designResult","result":self.design(request_id,command)?}),
                );
            }
            ensure!(
                self.qualified && !self.ended,
                "qualified_evaluator_required"
            );
            match command_type {
                "captureRenderWindow" => self.capture(request_id, &raw),
                "simulateGpuiEvent" | "batch" => self.act(request_id, &raw),
                "waitFor" if raw["condition"]["type"].as_str() == Some("completedFrame") => {
                    let target = serde_json::from_value(raw["target"].clone())?;
                    let after = raw["condition"]["afterFrameGeneration"]
                        .as_u64()
                        .context("after_frame_generation_required")?;
                    let started = Instant::now();
                    // Omission observes this exact live instance; an explicit
                    // expectation, including null, still takes the strict path.
                    let expected = match raw["condition"].get("expected") {
                        Some(expected) => serde_json::from_value(expected.clone())?,
                        None => self.identity(&target)?,
                    };
                    let (frame, phases) = self.completed_frame(&target, &expected, after)?;
                    Ok(
                        json!({"type":"waitForResult","success":true,"elapsed":started.elapsed().as_millis(),"frameIdentity":frame,"phaseDurationsMs":phases}),
                    )
                }
                "getState"
                | "getElements"
                | "getLayoutInfo"
                | "getLogs"
                | "getAgentChatState"
                | "listAutomationWindows"
                | "waitFor" => self.query(request_id, &raw, QueryMode::Progress),
                _ => anyhow::bail!("owned_operation_not_permitted"),
            }
        })();
        match result {
            Ok(reply) => reply,
            Err(error) if command_type == "design" => {
                json!({"type":"designResult","result":{"operation":operation,"ok":false,"error":{"code":error.to_string(),"message":"Owned evaluation operation refused"}}})
            }
            Err(error) => {
                json!({"type":"error","code":error.to_string(),"message":"Owned evaluation operation refused"})
            }
        }
    }

    fn unmount(&mut self, target: &AutomationWindowTarget) -> Result<()> {
        let mounted = self.resolve(target)?.clone();
        let generation = mounted
            .info
            .generation
            .context("window_generation_missing")?;
        ensure!(
            crate::windows::runtime_window_host_policy(&mounted.info.id, generation)?
                == WindowHostPolicy::OwnedHidden,
            "foreign_runtime_window"
        );
        ensure!(
            mounted
                .handle
                .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                    window.is_owned_hidden()
                })?,
            "foreign_runtime_window"
        );
        let children: Vec<_> = self
            .mounted
            .values()
            .filter(|child| {
                child.info.parent_window_id.as_deref() == Some(&mounted.info.id)
                    && child.info.parent_window_generation == Some(generation)
            })
            .map(Self::instance)
            .collect();
        let mut result = if mounted.info.id == "main" {
            self.theme_fixture.restore()
        } else {
            Ok(())
        };
        for child in children {
            let closed = child.and_then(|child| {
                if let AutomationWindowTarget::Instance { id, generation } = &child {
                    if !self.mounted.contains_key(id)
                        && crate::windows::get_runtime_window_handle_for_generation(id, *generation)
                            .is_none()
                    {
                        return Ok(());
                    }
                }
                self.unmount(&child)
            });
            result = result.and(closed);
        }
        if matches!(mounted.owner, RootOwner::Main(_)) {
            self.retired_search_gate = self.search_gate();
        }
        self.retire_search_frame_trace(target)?;
        let owner_close = (|| -> Result<()> {
            let mut app = self.cx.app.borrow_mut();
            crate::footer_popup::retire_footer_owner(mounted.handle, &mut app);
            match &mounted.owner {
                RootOwner::Secondary(fixture) => fixture.close(&mut app)?,
                RootOwner::Footer => crate::footer_popup::close_owned_footer_fixture(
                    &mounted.info.id,
                    generation,
                    &mut app,
                )?,
                RootOwner::ShortcutRecorder => {
                    main_fixtures::close_shortcut_fixture(&mounted.info.id, generation, &mut app)?
                }
                RootOwner::Main(entity) => {
                    entity.update(&mut **app, |app, _| super::search_fixtures::retire(app))?;
                    mounted
                        .handle
                        .update(&mut **app, |_, window, _| window.remove_window())?;
                    crate::windows::remove_runtime_window_instance(&mounted.info.id, generation);
                }
                _ => crate::windows::automation_surface_collector::close_owned_registered_surface(
                    &mounted.info,
                    &mut app,
                )?,
            }
            Ok(())
        })();
        if owner_close.is_ok() {
            self.mounted.remove(&mounted.info.id);
            forget_owned_render_frame(&mounted.info.id, generation);
            self.sdk_controls.remove(&mounted.info.id);
            self.sdk_prompt_controls.remove(&mounted.info.id);
            self.dictation_controls.remove(&mounted.info.id);
            self.flow_controls.remove(&mounted.info.id);
            if mounted.info.id == "main" {
                self.main = None;
                crate::clear_main_window_handle_if_matches(mounted.handle);
            }
        }
        result = result.and(owner_close);
        // Even a failed owner can have scheduled partial teardown. Preserve its
        // calibrated delay and first error while observing bounded real progress.
        let deadline = Instant::now() + Duration::from_secs(2);
        while crate::windows::get_runtime_window_handle_for_generation(&mounted.info.id, generation)
            .is_some()
            || mounted
                .handle
                .update(&mut **self.cx.app.borrow_mut(), |_, _, _| ())
                .is_ok()
        {
            if Instant::now() >= deadline {
                result = result.and(Err(anyhow::anyhow!("window_teardown_timeout")));
                break;
            }
            result = result.and(self.tick(true));
        }
        result
    }

    pub(super) fn lifecycle_observation(
        &self,
        shutdown_reason: &str,
        close_succeeded: bool,
    ) -> Value {
        let native = self.cx.owned_hidden_observation();
        let registered = crate::windows::list_automation_windows().len();
        let refused_effects =
            crate::runtime_policy::owned_evaluation().map(|policy| policy.refused_effect_count());
        let closed = close_succeeded
            && self.ended
            && native.installed
            && native.live_windows == 0
            && registered == 0
            && refused_effects.is_some();
        json!({
            "type": "designResult", "protocolVersion": crate::protocol::version::CURRENT_PROTOCOL_VERSION,
            "result": {
                "operation": "end", "lifecycle": true, "schemaVersion": 1, "ok": closed,
                "shutdownReason": shutdown_reason,
                "identity": self.bootstrap.identity, "launchNonce": self.bootstrap.launch_nonce,
                "policySha256": self.bootstrap.policy_sha256,
                "ownedWindowsClosed": closed, "remainingWindows": native.live_windows,
                "refusedEffects": refused_effects,
                "native": {
                    "installed": native.installed, "openedWindows": native.opened_windows,
                    "liveWindows": native.live_windows, "automationWindows": registered,
                    "completedFrames": native.completed_frames, "readbackImages": native.readback_images,
                    "refusedOperations": native.refused_operations
                }
            }
        })
    }

    pub(super) fn close(&mut self) -> Result<()> {
        if self.ended {
            return Ok(());
        }
        // Keep every registered owner's handle out of the orphan fallback,
        // including owners that unregister before their close animation ends.
        let mut registered_handles: Vec<_> = self
            .mounted
            .values()
            .map(|mounted| mounted.handle)
            .collect();
        registered_handles.extend(
            crate::windows::list_automation_windows()
                .iter()
                .filter_map(|info| crate::windows::get_runtime_window_handle(&info.id)),
        );
        registered_handles.extend(
            crate::windows::automation_runtime_handles::runtime_window_instances()
                .into_iter()
                .filter_map(|(id, generation, _)| {
                    crate::windows::get_runtime_window_handle_for_generation(&id, generation)
                }),
        );
        let mut result = match &self.cleanup_failure {
            Some(error) => Err(anyhow::anyhow!(error.clone())),
            None => Ok(()),
        };
        result = result.and(self.sync_registered_windows());
        registered_handles.extend(self.mounted.values().map(|mounted| mounted.handle));
        let targets: Vec<_> = self.mounted.values().map(Self::instance).collect();
        for target in targets {
            let closed = target.and_then(|target| {
                if let AutomationWindowTarget::Instance { id, generation } = &target {
                    if !self.mounted.contains_key(id)
                        && crate::windows::get_runtime_window_handle_for_generation(id, *generation)
                            .is_none()
                    {
                        return Ok(());
                    }
                }
                self.unmount(&target)
            });
            result = result.and(closed);
        }
        // Only never-registered owned remnants from a failed construction may
        // use direct removal. Failed real owners must keep their teardown path.
        registered_handles.extend(
            crate::windows::list_automation_windows()
                .iter()
                .filter_map(|info| crate::windows::get_runtime_window_handle(&info.id)),
        );
        registered_handles.extend(
            crate::windows::automation_runtime_handles::runtime_window_instances()
                .into_iter()
                .filter_map(|(id, generation, _)| {
                    crate::windows::get_runtime_window_handle_for_generation(&id, generation)
                }),
        );
        let remaining = self.cx.app.borrow().windows();
        for handle in remaining {
            if registered_handles.contains(&handle) {
                continue;
            }
            let removed = handle
                .update(&mut **self.cx.app.borrow_mut(), |_, window, _| {
                    ensure!(window.is_owned_hidden(), "foreign_runtime_window");
                    window.remove_window();
                    Ok::<_, anyhow::Error>(())
                })
                .and_then(|removed| removed);
            result = result.and(removed);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            result = result.and(self.tick(false));
            match self.cx.pump_owned_work(0, Duration::ZERO, 0) {
                Ok(pending)
                    if self.cx.owned_hidden_observation().live_windows == 0
                        && pending.pending_entity_releases == 0
                        && pending.pending_effects == 0 =>
                {
                    break
                }
                Ok(_) => {}
                Err(error) => result = result.and(Err(error)),
            }
            if Instant::now() >= deadline {
                result = result.and(Err(anyhow::anyhow!("native_window_cleanup_incomplete")));
                break;
            }
        }
        if !crate::windows::list_automation_windows().is_empty() {
            result = result.and(Err(anyhow::anyhow!("automation_window_cleanup_incomplete")));
        }
        result = result.and(self.theme_fixture.restore());
        if let Err(error) = result {
            self.cleanup_failure
                .get_or_insert_with(|| error.to_string());
            return Err(error);
        }
        self.ended = true;
        Ok(())
    }
}
