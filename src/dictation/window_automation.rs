impl DictationOverlay {
    pub fn fixture_state(&self) -> &DictationOverlayState {
        &self.state
    }
    pub(crate) fn semantic_revision(&self) -> u64 {
        self.semantic_revision
    }

    /// Read-only selection receipt for diagnostics alongside the overlay state.
    pub fn microphone_selection(&self) -> (u64, Option<&str>) {
        (
            self.microphone_selection_count,
            self.selected_microphone_semantic_id.as_deref(),
        )
    }

    pub(crate) fn record_microphone_selection(
        &mut self,
        semantic_id: String,
        cx: &mut Context<Self>,
    ) {
        self.microphone_selection_count = self.microphone_selection_count.strict_add(1);
        self.semantic_revision = self.semantic_revision.strict_add(1);
        self.selected_microphone_semantic_id = Some(semantic_id);
        cx.notify();
    }

    pub fn open_fixture_microphone_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(self.host_policy.is_hidden(), "owned_dictation_required");
        self.open_microphone_picker(window, cx);
        anyhow::ensure!(
            self.microphone_popup_lifetime.is_some(),
            "microphone_popup_not_opened"
        );
        Ok(())
    }

}

pub(crate) fn dictation_overlay_revision_facts(
    generation: u64,
    window: Option<&Window>,
    cx: &App,
) -> Option<(u64, u64, u64, u64)> {
    let info = crate::windows::automation_window_by_id(DICTATION_OVERLAY_AUTOMATION_ID)?;
    if info.generation != Some(generation) {
        return None;
    }
    let handle = (*DICTATION_OVERLAY_WINDOW.get()?.lock())?;
    if crate::windows::get_runtime_window_handle_for_generation(
        DICTATION_OVERLAY_AUTOMATION_ID,
        generation,
    ) != Some(handle.into())
    {
        return None;
    }
    crate::windows::automation_surface_collector::read_window_root(handle, window, cx, |view, _| {
        (
            generation,
            view.semantic_revision(),
            view.semantic_revision(),
            view.applied_theme_revision(),
        )
    })
    .ok()
}

pub(crate) fn get_dictation_overlay_state_for_instance(
    generation: u64,
    cx: &App,
) -> Option<DictationOverlayState> {
    dictation_overlay_revision_facts(generation, None, cx)?;
    let handle = (*DICTATION_OVERLAY_WINDOW.get()?.lock())?;
    handle.read_with(cx, |view, _| view.state.clone()).ok()
}

#[allow(
    dead_code,
    reason = "the separately compiled application binary owns the isolated dictation popup fixture command"
)]
pub(crate) fn open_dictation_microphone_popup_fixture(cx: &mut App) -> anyhow::Result<()> {
    if !dictation_overlay_fixture_mode() {
        anyhow::bail!("Dictation microphone fixture requires an active overlay fixture");
    }
    let handle = DICTATION_OVERLAY_WINDOW
        .get_or_init(|| Mutex::new(None))
        .lock()
        .as_ref()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("Dictation overlay fixture is not open"))?;
    handle
        .update(cx, |view, window, cx| {
            view.open_microphone_picker(window, cx);
        })
        .map_err(|error| anyhow::anyhow!("Failed to open Dictation microphone fixture: {error}"))
}


fn dictation_automation_bounds(
    bounds: gpui::Bounds<gpui::Pixels>,
) -> crate::protocol::AutomationWindowBounds {
    crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}

pub fn automation_layout_info(
    resolved: &crate::protocol::AutomationWindowInfo,
) -> crate::protocol::LayoutInfo {
    automation_layout_info_with_radius(resolved, Some(OVERLAY_RADIUS_PX))
}

fn synthetic_dictation_failure_state() -> crate::dictation::DictationFailureState {
    let failure = crate::ai::reliability::destination_failure(
        false,
        "synthetic Dictation recovery fixture failure",
    );
    crate::dictation::DictationFailureState {
        operation_id: 0,
        destination_id: "fixture-destination".to_string(),
        destination_label: "Fixture Destination".to_string(),
        identity_generation: 1,
        transcript_id: "fixture-transcript".to_string(),
        history_entry_id: "fixture-history".to_string(),
        failure,
        retry_safety: sk_protocol::ai_reliability::RetrySafety::Never,
        preservation_receipt: crate::dictation::DictationTranscriptPreservationReceipt {
            transcript_id: "fixture-transcript".to_string(),
            transcript_len: 0,
            transcript_fingerprint: crate::dictation::redacted_transcript_fingerprint(""),
            history_entry_id: "fixture-history".to_string(),
            history_saved: true,
        },
        capabilities: crate::dictation::DictationFailureRecoveryCapabilities {
            retry_same_destination: false,
            choose_destination: true,
            copy_transcript: true,
            open_dictation_history: true,
        },
    }
}

fn apply_test_dictation_overlay_override(
    mut state: DictationOverlayState,
) -> DictationOverlayState {
    if !dictation_overlay_fixture_mode()
        || std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() != Some("1")
    {
        return state;
    }

    if let Ok(phase) = std::env::var("SCRIPT_KIT_TEST_DICTATION_FIXTURE_PHASE") {
        state.phase = match phase.as_str() {
            "recording" => DictationSessionPhase::Recording,
            "confirming" => DictationSessionPhase::Confirming,
            "transcribing" => DictationSessionPhase::Transcribing,
            "delivering" => DictationSessionPhase::Delivering,
            "finished" => DictationSessionPhase::Finished,
            "failed" => DictationSessionPhase::Failed(synthetic_dictation_failure_state()),
            _ => state.phase,
        };
    }
    if let Ok(target) = std::env::var("SCRIPT_KIT_TEST_DICTATION_FIXTURE_TARGET") {
        if let Some(target) = crate::dictation::parse_dictation_target_label(&target) {
            state.target = target;
        }
    }
    if std::env::var("SCRIPT_KIT_TEST_DICTATION_FIXTURE_ARMED")
        .ok()
        .as_deref()
        == Some("1")
    {
        state.transcript = "Synthetic armed fixture".into();
    }
    state
}

fn automation_layout_info_with_radius(
    resolved: &crate::protocol::AutomationWindowInfo,
    overlay_radius: Option<f32>,
) -> crate::protocol::LayoutInfo {
    use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
    use crate::ui::chrome as chrome_tokens;

    let bounds = resolved
        .bounds
        .clone()
        .unwrap_or(crate::protocol::AutomationWindowBounds {
            x: 0.0,
            y: 0.0,
            width: OVERLAY_WIDTH_PX as f64,
            height: OVERLAY_HEIGHT_PX as f64,
        });
    let width = bounds.width as f32;
    let height = bounds.height as f32;
    let theme = get_cached_theme();
    let detached_footer =
        crate::platform::tahoe_native_glass_composition_available() && theme.is_vibrancy_enabled();
    let footer_height = crate::components::footer_chrome::footer_rail_chrome(&theme).height_px;
    let footer_gap = if detached_footer {
        crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
    } else {
        0.0
    };
    let regions = crate::footer_popup::main_window_detached_footer_regions_gpui(
        width,
        height,
        footer_height,
        footer_gap,
        1.0,
    );
    let stage_height = regions.main_content.height;
    let content_parent = if detached_footer {
        "DictationContentStage"
    } else {
        "DictationOverlayWindow"
    };
    let header_top = 5.0;
    let header_height = OVERLAY_HEADER_ROW_HEIGHT_PX;
    let caption_top = header_top + header_height;
    let caption_height = (stage_height - caption_top).max(0.0);

    let mut components =
        vec![
            LayoutComponentInfo::new("DictationOverlayWindow", LayoutComponentType::Container)
                .with_bounds(0.0, 0.0, width, height)
                .with_visual_style(
                    if detached_footer {
                        chrome_tokens::CHROME_LAYER_WINDOW_BACKDROP
                    } else {
                        chrome_tokens::CHROME_LAYER_FLOATING
                    },
                    if detached_footer {
                        chrome_tokens::MATERIAL_NATIVE_WINDOW_BACKDROP
                    } else {
                        chrome_tokens::MATERIAL_NS_VISUAL_EFFECT
                    },
                    if detached_footer {
                        None
                    } else {
                        overlay_radius
                    },
                )
                .with_hit_bounds(0.0, 0.0, width, height)
                .with_padding(0.0, 0.0, 0.0, 0.0),
        ];
    if detached_footer {
        components.push(
            LayoutComponentInfo::new("DictationContentStage", LayoutComponentType::Container)
                .with_bounds(
                    regions.main_content.x,
                    regions.main_content.y,
                    regions.main_content.width,
                    regions.main_content.height,
                )
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FLOATING,
                    chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                    overlay_radius,
                )
                .with_hit_bounds(
                    regions.main_content.x,
                    regions.main_content.y,
                    regions.main_content.width,
                    regions.main_content.height,
                )
                .with_depth(1)
                .with_parent("DictationOverlayWindow"),
        );
    }
    components.extend([
        LayoutComponentInfo::new("DictationHeaderRow", LayoutComponentType::Container)
            .with_bounds(0.0, header_top, width, header_height)
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                None,
            )
            .with_hit_bounds(0.0, header_top, width, header_height)
            .with_padding(
                0.0,
                OVERLAY_HORIZONTAL_PADDING_PX,
                0.0,
                OVERLAY_HORIZONTAL_PADDING_PX,
            )
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationTimerSlot", LayoutComponentType::Other)
            .with_bounds(
                OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                TARGET_BADGE_SLOT_WIDTH_PX,
                header_height,
            )
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_CONTENT,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                None,
            )
            .with_padding(0.0, 0.0, 0.0, 0.0)
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationDestinationChips", LayoutComponentType::Button)
            .with_bounds(
                TARGET_BADGE_SLOT_WIDTH_PX + OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                (width - 2.0 * (TARGET_BADGE_SLOT_WIDTH_PX + OVERLAY_HORIZONTAL_PADDING_PX))
                    .max(0.0),
                header_height,
            )
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                Some(chrome_tokens::LIQUID_GLASS_CONTROL_RADIUS_PX),
            )
            .with_hit_bounds(
                TARGET_BADGE_SLOT_WIDTH_PX + OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                (width - 2.0 * (TARGET_BADGE_SLOT_WIDTH_PX + OVERLAY_HORIZONTAL_PADDING_PX))
                    .max(0.0),
                header_height,
            )
            .with_gap(6.0)
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationTargetBadge", LayoutComponentType::Button)
            .with_bounds(
                width - TARGET_BADGE_SLOT_WIDTH_PX - OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                TARGET_BADGE_SLOT_WIDTH_PX,
                header_height,
            )
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                Some(chrome_tokens::LIQUID_GLASS_CONTROL_RADIUS_PX),
            )
            .with_hit_bounds(
                width - TARGET_BADGE_SLOT_WIDTH_PX - OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                TARGET_BADGE_SLOT_WIDTH_PX,
                header_height,
            )
            .with_padding(2.0, 8.0, 2.0, 8.0)
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationSignalBand", LayoutComponentType::Container)
            .with_bounds(0.0, caption_top, width, caption_height)
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                overlay_radius,
            )
            .with_hit_bounds(0.0, caption_top, width, caption_height)
            .with_padding(
                6.0,
                OVERLAY_HORIZONTAL_PADDING_PX,
                6.0,
                OVERLAY_HORIZONTAL_PADDING_PX,
            )
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationWaveform", LayoutComponentType::Container)
            .with_bounds(
                (width - 48.0) / 2.0,
                caption_top + (caption_height - WAVEFORM_BAR_MAX_HEIGHT_PX).max(0.0) / 2.0,
                48.0,
                WAVEFORM_BAR_MAX_HEIGHT_PX,
            )
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_CONTENT,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
            )
            .with_gap(WAVEFORM_BAR_GAP_PX)
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
    ]);
    if detached_footer {
        components.push(
            LayoutComponentInfo::new(
                "DictationFooterDesktopGutter",
                LayoutComponentType::Other,
            )
            .with_bounds(
                regions.transparent_gap.x,
                regions.transparent_gap.y,
                regions.transparent_gap.width,
                regions.transparent_gap.height,
            )
            .with_depth(1)
            .with_parent("DictationOverlayWindow")
            .with_explanation(
                "Fully transparent 8-point desktop gutter separating the dictation capsule from its floating controls.",
            ),
        );
    }
    components.push(
        LayoutComponentInfo::new("DictationFooterRail", LayoutComponentType::Panel)
            .with_bounds(
                regions.footer.x,
                regions.footer.y,
                regions.footer.width,
                regions.footer.height,
            )
            .with_visual_style(
                if detached_footer {
                    chrome_tokens::CHROME_LAYER_FLOATING
                } else {
                    chrome_tokens::CHROME_LAYER_FUNCTIONAL
                },
                if detached_footer {
                    chrome_tokens::MATERIAL_NS_VISUAL_EFFECT
                } else {
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN
                },
                Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
            )
            .with_hit_bounds(
                regions.footer.x,
                regions.footer.y,
                regions.footer.width,
                regions.footer.height,
            )
            .with_padding(0.0, 0.0, 0.0, 0.0)
            .with_depth(1)
            .with_parent("DictationOverlayWindow")
            .with_explanation(if detached_footer {
                "Transparent positioning rail containing discrete native glass capsules; it has no full-width footer surface."
            } else {
                "In-window dictation action rail."
            }),
    );

    LayoutInfo {
        window_width: width,
        window_height: height,
        prompt_type: "dictation".to_string(),
        components,
        fidelity: None,
        handler_form: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}
