// Owned dictation sessions share the production session state and destination locks.
/// Drives a bounded owned session without opening an external capture device.
pub struct DictationFixtureControl {
    generation: u64,
    events: async_channel::Sender<DictationCaptureEvent>,
}

impl DictationFixtureControl {
    pub fn begin(selection: crate::dictation::DictationTargetSelection) -> Result<Self> {
        anyhow::ensure!(
            crate::runtime_policy::is_owned_evaluation(),
            "owned_dictation_required"
        );
        anyhow::ensure!(
            !matches!(
                selection.destination,
                crate::dictation::FrozenDictationDestination::ExternalApp { .. }
            ),
            "external_dictation_excluded"
        );
        let mut guard = SESSION.lock();
        anyhow::ensure!(guard.is_none(), "dictation_session_already_owned");
        let (events, rx) = async_channel::bounded(32);
        let mut session =
            DictationSession::from_source(selection.target, rx, None, None, false, None);
        let generation = session.session_generation;
        session.selection = Some(selection.clone());
        *LAST_FROZEN_SELECTION.lock() = Some(selection);
        *PARTIAL_TRANSCRIPT.lock() = None;
        *guard = Some(session);
        bump_dictation_state_generation();
        Ok(Self { generation, events })
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn bars(&self, bars: [f32; 9]) -> Result<()> {
        anyhow::ensure!(
            bars.iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "invalid_fixture_bars"
        );
        self.events
            .try_send(DictationCaptureEvent::Bars(bars))
            .map_err(|_| anyhow::anyhow!("dictation_fixture_events_full_or_closed"))
    }

    pub fn advance(&self, phase: DictationSessionPhase, transcript: Option<String>) -> Result<()> {
        let mut guard = SESSION.lock();
        let session = guard
            .as_mut()
            .filter(|s| s.session_generation == self.generation)
            .ok_or_else(|| anyhow::anyhow!("stale_dictation_session"))?;
        anyhow::ensure!(
            fixture_phase_transition_allowed(&session.overlay_phase, &phase),
            "invalid_dictation_phase_transition"
        );
        if let Some(text) = transcript {
            anyhow::ensure!(
                text.len() <= 64 * 1024,
                "dictation_fixture_transcript_limit"
            );
            *PARTIAL_TRANSCRIPT.lock() = Some((self.generation, text));
        }
        session.overlay_phase = phase;
        bump_dictation_state_generation();
        Ok(())
    }

    pub fn retarget(&self, selection: crate::dictation::DictationTargetSelection) -> Result<()> {
        anyhow::ensure!(
            !matches!(
                selection.destination,
                crate::dictation::FrozenDictationDestination::ExternalApp { .. }
            ),
            "external_dictation_excluded"
        );
        {
            let guard = SESSION.lock();
            anyhow::ensure!(
                guard
                    .as_ref()
                    .is_some_and(|s| s.session_generation == self.generation),
                "stale_dictation_session"
            );
        }
        set_dictation_session_selection(selection)
            .ok_or_else(|| anyhow::anyhow!("dictation_destination_locked"))?;
        Ok(())
    }

    pub fn delivery_request(&self) -> Result<crate::dictation::DictationDeliveryRequest> {
        let guard = SESSION.lock();
        let session = guard
            .as_ref()
            .filter(|s| s.session_generation == self.generation)
            .ok_or_else(|| anyhow::anyhow!("stale_dictation_session"))?;
        anyhow::ensure!(
            session.overlay_phase == DictationSessionPhase::Delivering,
            "dictation_not_delivering"
        );
        let selection = session
            .selection
            .clone()
            .ok_or_else(|| anyhow::anyhow!("dictation_destination_missing"))?;
        let text = PARTIAL_TRANSCRIPT
            .lock()
            .as_ref()
            .filter(|(id, _)| *id == self.generation)
            .map(|(_, text)| text.clone())
            .ok_or_else(|| anyhow::anyhow!("dictation_transcript_missing"))?;
        anyhow::ensure!(!text.trim().is_empty(), "dictation_transcript_empty");
        let delivery_id = next_dictation_delivery_id();
        Ok(crate::dictation::DictationDeliveryRequest {
            delivery_id,
            session_generation: self.generation,
            selection,
            transcript: crate::dictation::ImmutableDictationTranscript::new(
                format!("fixture-{delivery_id}"),
                text,
            ),
            history_entry_id: format!("fixture-{delivery_id}"),
            attempt: 1,
        })
    }
}

impl Drop for DictationFixtureControl {
    fn drop(&mut self) {
        let mut guard = SESSION.lock();
        if guard
            .as_ref()
            .is_some_and(|session| session.session_generation == self.generation)
        {
            guard.take();
            *PARTIAL_TRANSCRIPT.lock() = None;
            *LAST_FROZEN_SELECTION.lock() = None;
            bump_dictation_state_generation();
        }
    }
}

pub fn validate_owned_dictation_delivery_request(
    request: &crate::dictation::DictationDeliveryRequest,
) -> std::result::Result<(), String> {
    if !crate::runtime_policy::is_owned_evaluation() {
        return Err("owned_dictation_required".into());
    }
    let guard = SESSION.lock();
    let session = guard
        .as_ref()
        .filter(|session| {
            session.capture_handle.is_none()
                && session.session_generation == request.session_generation
        })
        .ok_or("stale_dictation_session")?;
    if session.overlay_phase != DictationSessionPhase::Delivering {
        return Err("dictation_not_delivering".into());
    }
    if session.selection.as_ref() != Some(&request.selection) {
        return Err("dictation_destination_stale".into());
    }
    let transcript = PARTIAL_TRANSCRIPT.lock();
    if !transcript.as_ref().is_some_and(|(generation, text)| {
        *generation == request.session_generation && text == request.transcript.text()
    }) {
        return Err("dictation_transcript_stale".into());
    }
    Ok(())
}

fn fixture_phase_transition_allowed(
    from: &DictationSessionPhase,
    to: &DictationSessionPhase,
) -> bool {
    use DictationSessionPhase::*;
    matches!(
        (from, to),
        (Recording, Recording | Confirming | Transcribing)
            | (Confirming, Recording | Transcribing)
            | (Transcribing, Delivering | Failed(_))
            | (Delivering, Finished | Failed(_))
            | (Failed(_), Delivering)
    )
}
