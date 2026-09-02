//! Real Dictation overlay and capture-free runtime event source.
use super::fixture_ids::DICTATION_FIXTURE_IDS;
use crate::dictation::{
    DictationOverlay, DictationOverlayState, DictationSessionPhase, DictationTarget,
    DictationTargetSelection,
};
use gpui::{App, AppContext, Entity, Window};

pub(crate) enum DictationFixtureEvent {
    Recording {
        transcript: String,
        bars: [f32; 9],
    },
    Confirm,
    Resume,
    Transcribe,
    Deliver,
    Retarget(DictationTargetSelection),
    DeliveryCompleted {
        request: Box<crate::dictation::DictationDeliveryRequest>,
        outcome: crate::dictation::DictationDeliveryOutcome,
    },
    OpenMicrophonePicker,
}

pub(crate) fn create_dictation_fixture(
    fixture_id: &str,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<Entity<DictationOverlay>> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    let view = cx.new(|cx| {
        DictationOverlay::from_presentation(
            crate::runtime_policy::WindowHostPolicy::OwnedHidden,
            false,
            cx,
        )
    });
    view.update(cx, |view, cx| {
        apply_dictation_fixture(fixture_id, view, window, cx)
    })?;
    Ok(view)
}

fn apply_dictation_fixture(
    fixture_id: &str,
    view: &mut DictationOverlay,
    window: &mut Window,
    cx: &mut gpui::Context<DictationOverlay>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        fixture_id != "dictation.microphone-picker",
        "dictation_microphone_requires_parent"
    );
    anyhow::ensure!(
        DICTATION_FIXTURE_IDS.contains(&fixture_id),
        "unknown_dictation_fixture"
    );
    let phase = match fixture_id {
        "dictation.confirming" => DictationSessionPhase::Confirming,
        "dictation.transcribing" => DictationSessionPhase::Transcribing,
        "dictation.delivering" => DictationSessionPhase::Delivering,
        "dictation.finished" => DictationSessionPhase::Finished,
        "dictation.failed" => {
            DictationSessionPhase::Failed(crate::dictation::dictation_pipeline_failure_state(
                1,
                DictationTarget::MainWindowFilter,
                "controlled fixture delivery failure",
            ))
        }
        _ => DictationSessionPhase::Recording,
    };
    view.set_state(
        DictationOverlayState {
            phase,
            elapsed: std::time::Duration::from_secs(3),
            bars: [0.3; 9],
            transcript: "A controlled dictation transcript.".into(),
            target: DictationTarget::MainWindowFilter,
        },
        window,
        cx,
    );
    Ok(())
}

pub(crate) fn open_owned_dictation_microphone_fixture(
    view: &Entity<DictationOverlay>,
    parent: &crate::protocol::AutomationWindowInfo,
    parent_handle: gpui::AnyWindowHandle,
    cx: &mut App,
) -> anyhow::Result<crate::protocol::AutomationWindowInfo> {
    let generation = parent
        .generation
        .ok_or_else(|| anyhow::anyhow!("dictation_parent_generation_missing"))?;
    anyhow::ensure!(
        parent.id == "dictation"
            && crate::windows::get_runtime_window_handle_for_generation(&parent.id, generation)
                == Some(parent_handle),
        "dictation_parent_stale"
    );
    parent_handle.update(cx, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.open_fixture_microphone_picker(window, cx)
        })
    })??;
    // The real popup publishes its identity only after the deferred attach
    // handshake. Drain that bounded effect chain before observing registration.
    cx.flush_owned_effects(256)?;
    let child = crate::windows::automation_window_by_id("dictation-microphone-popup")
        .ok_or_else(|| anyhow::anyhow!("dictation_microphone_registration_missing"))?;
    anyhow::ensure!(
        child.semantic_surface.as_deref() == Some("dictationMicrophonePopup")
            && child.parent_window_id.as_deref() == Some(parent.id.as_str())
            && child.parent_window_generation == Some(generation),
        "dictation_microphone_parent_mismatch"
    );
    Ok(child)
}

pub(crate) fn open_owned_dictation_fixture(
    fixture_id: &str,
    cx: &mut App,
) -> anyhow::Result<(
    Entity<DictationOverlay>,
    crate::protocol::AutomationWindowInfo,
)> {
    anyhow::ensure!(
        DICTATION_FIXTURE_IDS.contains(&fixture_id),
        "unknown_dictation_fixture"
    );
    let bounds = gpui::Bounds::new(
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
        gpui::size(gpui::px(480.0), gpui::px(260.0)),
    );
    let handle = crate::dictation::open_dictation_overlay_with_policy(
        crate::runtime_policy::WindowHostPolicy::OwnedHidden,
        Some(bounds),
        cx,
    )?;
    let entity = handle.update(cx, |view, window, cx| {
        apply_dictation_fixture(fixture_id, view, window, cx)?;
        Ok::<_, anyhow::Error>(cx.entity())
    })??;
    let info = crate::windows::automation_window_by_id("dictation")
        .ok_or_else(|| anyhow::anyhow!("dictation_registration_missing"))?;
    Ok((entity, info))
}

pub(crate) fn begin_dictation_fixture(
    view: &Entity<DictationOverlay>,
    selection: DictationTargetSelection,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<crate::dictation::DictationFixtureControl> {
    let control = crate::dictation::DictationFixtureControl::begin(selection)?;
    sync_dictation_fixture(view, window, cx)?;
    Ok(control)
}

pub(crate) fn sync_dictation_fixture(
    view: &Entity<DictationOverlay>,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    let state = crate::dictation::snapshot_overlay_state()
        .ok_or_else(|| anyhow::anyhow!("dictation_fixture_session_missing"))?;
    view.update(cx, |view, cx| view.set_state(state, window, cx));
    Ok(())
}

pub(crate) fn drive_dictation_fixture(
    view: &Entity<DictationOverlay>,
    control: &crate::dictation::DictationFixtureControl,
    event: DictationFixtureEvent,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<()> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    match event {
        DictationFixtureEvent::Recording { transcript, bars } => {
            control.bars(bars)?;
            control.advance(DictationSessionPhase::Recording, Some(transcript))?;
        }
        DictationFixtureEvent::Confirm => {
            control.advance(DictationSessionPhase::Confirming, None)?
        }
        DictationFixtureEvent::Resume => control.advance(DictationSessionPhase::Recording, None)?,
        DictationFixtureEvent::Transcribe => {
            control.advance(DictationSessionPhase::Transcribing, None)?
        }
        DictationFixtureEvent::Deliver => {
            control.advance(DictationSessionPhase::Delivering, None)?
        }
        DictationFixtureEvent::Retarget(selection) => control.retarget(selection)?,
        DictationFixtureEvent::OpenMicrophonePicker => view.update(cx, |view, cx| {
            view.open_fixture_microphone_picker(window, cx)
        })?,
        DictationFixtureEvent::DeliveryCompleted { request, outcome } => {
            anyhow::ensure!(
                request.session_generation == control.generation(),
                "stale_dictation_delivery"
            );
            let phase = match outcome {
                crate::dictation::DictationDeliveryOutcome::Delivered {
                    mutation_receipt, ..
                } => {
                    anyhow::ensure!(
                        mutation_receipt.delivery_id == request.delivery_id,
                        "wrong_dictation_delivery_receipt"
                    );
                    anyhow::ensure!(
                        mutation_receipt.inserted_length > 0 || mutation_receipt.duplicate,
                        "empty_dictation_mutation"
                    );
                    crate::runtime_policy::record_completed_fixture_effect();
                    DictationSessionPhase::Finished
                }
                crate::dictation::DictationDeliveryOutcome::Refused { failure, .. }
                | crate::dictation::DictationDeliveryOutcome::Failed { failure, .. } => {
                    let preserved = crate::dictation::DictationTranscriptPreservationReceipt {
                        transcript_id: request.transcript.id().into(),
                        transcript_len: request.transcript.len(),
                        transcript_fingerprint: request.transcript.fingerprint().into(),
                        history_entry_id: request.history_entry_id.clone(),
                        history_saved: false,
                    };
                    DictationSessionPhase::Failed(crate::dictation::DictationFailureState {
                        operation_id: request.delivery_id,
                        destination_id: request.selection.destination.kind().into(),
                        destination_label: request.selection.display_label,
                        identity_generation: request.selection.selection_generation,
                        transcript_id: request.transcript.id().into(),
                        history_entry_id: request.history_entry_id,
                        failure,
                        retry_safety:
                            sk_protocol::ai_reliability::RetrySafety::ExplicitUserConfirmation,
                        preservation_receipt: preserved,
                        capabilities: Default::default(),
                    })
                }
            };
            control.advance(phase, None)?;
        }
    }
    sync_dictation_fixture(view, window, cx)
}
