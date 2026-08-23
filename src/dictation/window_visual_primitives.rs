/// Format elapsed duration as `M:SS` for the compact timer display.
pub(crate) fn format_elapsed(elapsed: Duration) -> SharedString {
    let elapsed_secs = elapsed.as_secs();
    format!("{}:{:02}", elapsed_secs / 60, elapsed_secs % 60).into()
}

/// Compute waveform bar opacity from a 0.0–1.0 audio level.
///
/// Matches vercel-voice JS: `clamp(0.3, value * 1.5, 1.0)`.
pub(crate) fn waveform_bar_opacity(level: f32) -> f32 {
    (level.clamp(0.0, 1.0) * 1.5).clamp(0.3, 1.0)
}

/// Compute waveform bar height from a 0.0–1.0 audio level.
///
/// Compact capsule curve: `min + pow(v, 0.7) * (max - min)`.
pub(crate) fn waveform_bar_height(level: f32) -> f32 {
    (WAVEFORM_BAR_MIN_HEIGHT_PX
        + level.clamp(0.0, 1.0).powf(0.7)
            * (WAVEFORM_BAR_MAX_HEIGHT_PX - WAVEFORM_BAR_MIN_HEIGHT_PX))
        .min(WAVEFORM_BAR_MAX_HEIGHT_PX)
}

/// Returns true if any bar exceeds the sound threshold.
pub(crate) fn has_sound(bars: &[f32; WAVEFORM_BAR_COUNT]) -> bool {
    bars.iter().any(|&bar| bar > SOUND_THRESHOLD)
}

/// Resolve a chip's Lucide icon name to an embedded asset path.
pub(crate) fn chip_icon_path(lucide_name: &str) -> Option<gpui::SharedString> {
    use gpui_component::IconNamed;
    crate::icons::lucide_from_str(lucide_name).map(|icon| icon.path())
}

/// Shared icon+verb chip styling for the destination row.
///
/// Used by both the runtime overlay (which adds click/tooltip handlers) and
/// the Storybook preview (which stays static), so the two can never drift.
fn destination_chip_base(
    verb: &'static str,
    icon: &'static str,
    is_active: bool,
    dimmed: bool,
) -> gpui::Stateful<Div> {
    let theme = get_cached_theme();
    let text_muted = theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED);
    let text_active = theme.colors.text.primary.with_opacity(OPACITY_ACTIVE);
    let label_color = if is_active { text_active } else { text_muted };

    let mut chip = div()
        .id(SharedString::from(format!("dictation-chip-{verb}")))
        .px(px(8.))
        .py(px(1.))
        .rounded(px(999.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .text_size(px(STATUS_TEXT_SIZE_PX - 1.0))
        .font_family(FONT_SYSTEM_UI)
        .text_color(label_color);

    if let Some(icon_path) = chip_icon_path(icon) {
        chip = chip.child(
            svg()
                .path(icon_path)
                .size(px(CHIP_ICON_SIZE_PX))
                .flex_shrink_0()
                .text_color(label_color),
        );
    }
    chip = chip.child(verb);

    if is_active {
        chip = chip
            .bg(theme.colors.background.main.with_opacity(OPACITY_ACTIVE))
            .border_1()
            .border_color(theme.colors.ui.border.with_opacity(OPACITY_SELECTED));
    } else {
        chip = chip.border_1().border_color(gpui::transparent_black());
    }

    if dimmed {
        chip = chip.opacity(0.55).cursor_default();
    }

    chip
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChipClickBehavior {
    Ignore,
    Retarget,
}

pub(crate) fn chip_click_behavior(
    phase: &DictationSessionPhase,
    _armed: bool,
    _option_held: bool,
) -> ChipClickBehavior {
    if matches!(
        phase,
        DictationSessionPhase::Recording | DictationSessionPhase::Confirming
    ) {
        ChipClickBehavior::Retarget
    } else {
        ChipClickBehavior::Ignore
    }
}

/// Tooltip copy for a destination selector. Selecting a destination never
/// stops or delivers the current dictation session.
pub(crate) fn chip_tooltip_label(target: crate::dictation::DictationTarget) -> SharedString {
    if target == crate::dictation::DictationTarget::ExternalApp {
        return crate::dictation::get_dictation_target_selection()
            .filter(|selection| selection.is_compatible_with(target))
            .map(|selection| format!("Dictate into {}", selection.display_label).into())
            .unwrap_or_else(|| target.descriptor().description.into());
    }

    target.descriptor().description.into()
}

/// Estimate how many lines `text` wraps to at ~`chars_per_line` characters,
/// using greedy word wrap. Drives window growth only — the text system does
/// the real wrapping — so being off by a character or two just means the
/// block clips a hair earlier or later.
pub(crate) fn estimate_caption_lines(text: &str, chars_per_line: usize) -> usize {
    let chars_per_line = chars_per_line.max(1);
    let mut lines = 0usize;
    let mut current = 0usize;
    for word in text.split_whitespace() {
        let word_len = word.chars().count().min(chars_per_line);
        let needed = if current == 0 {
            word_len
        } else {
            current + 1 + word_len
        };
        if current == 0 || needed > chars_per_line {
            lines += 1;
            current = word_len;
        } else {
            current = needed;
        }
    }
    lines.max(1)
}

/// Build the recording-time caption text as styled wrapped runs.
///
/// The full committed transcript wraps naturally at the pill width. Only the
/// newest revealed word fades in — everything before it renders at full
/// opacity and never shifts, because new words append at the wrap point
/// instead of pushing the line sideways. A muted dot marker trails the
/// newest word while the session is live.
fn live_caption_text(
    caption: &crate::dictation::live_caption::LiveCaption,
    reduced_motion: bool,
) -> AnyElement {
    let theme = get_cached_theme();
    let base_color = theme.colors.text.primary.with_opacity(OPACITY_ACTIVE);
    let marker_color = theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED);

    let visible = caption.visible_text();
    if visible.is_empty() {
        return div().into_any_element();
    }

    // Fade computed at render time: the overlay pump re-renders every 16 ms
    // while recording, so the newest word's alpha eases in without a
    // separate animation element (which cannot restyle individual runs).
    let fade = if reduced_motion {
        1.0
    } else {
        caption
            .last_reveal_at()
            .map(|at| {
                (at.elapsed().as_millis() as f32 / TRANSCRIPT_FADE_IN_MS as f32).clamp(0.0, 1.0)
            })
            .unwrap_or(1.0)
    };

    let fresh_chars = caption.fresh_char_offset();
    let fresh_byte = visible
        .char_indices()
        .nth(fresh_chars)
        .map(|(ix, _)| ix)
        .unwrap_or(visible.len());

    let font = gpui::font(FONT_SYSTEM_UI);
    let mut text = visible;
    let mut runs: Vec<TextRun> = Vec::with_capacity(3);
    let run = |len: usize, color| TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    if fresh_byte > 0 {
        runs.push(run(fresh_byte, base_color));
    }
    if text.len() > fresh_byte {
        runs.push(run(text.len() - fresh_byte, base_color.opacity(fade)));
    }
    text.push_str(LIVE_CAPTION_MARKER);
    runs.push(run(LIVE_CAPTION_MARKER.len(), marker_color));

    StyledText::new(text).with_runs(runs).into_any_element()
}

/// Shared container for the wrapped caption block: full width, bottom
/// anchored so the newest line stays visible when the text outgrows the
/// window, clipped above.
fn render_caption_block_container() -> Div {
    div()
        .flex_1()
        .min_w(px(0.))
        .min_h(px(0.))
        .h_full()
        .flex()
        .flex_col()
        .justify_end()
        .overflow_hidden()
        .px(px(6.))
        .text_size(px(TRANSCRIPT_TEXT_SIZE_PX))
        .font_family(FONT_SYSTEM_UI)
        .line_height(px(TRANSCRIPT_LINE_HEIGHT_PX))
}

/// Render a static transcript as a wrapped bottom-anchored block (processing
/// and terminal phases — same geometry as the live block, no marker).
fn render_transcript_block(transcript: &str, muted: bool) -> Div {
    let theme = get_cached_theme();
    let color = if muted {
        theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED)
    } else {
        theme.colors.text.primary.with_opacity(OPACITY_ACTIVE)
    };
    render_caption_block_container()
        .text_color(color)
        .child(transcript.trim().to_string())
}

/// Static header row for Storybook previews: timer, chips, badge — same
/// anatomy as the runtime header, without click handlers.
#[allow(dead_code)] // preview-chain helper (see render_dictation_overlay_state_preview)
fn render_static_header_row(state: &DictationOverlayState) -> impl IntoElement {
    let theme = get_cached_theme();
    let live = matches!(
        state.phase,
        DictationSessionPhase::Recording | DictationSessionPhase::Confirming
    );
    let timer_color = if live {
        theme.colors.text.primary.with_opacity(OPACITY_ACTIVE)
    } else {
        theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED)
    };

    let mut chip_row = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(6.));
    for descriptor in crate::dictation::DictationTarget::quick_chip_descriptors() {
        chip_row = chip_row.child(destination_chip_base(
            descriptor.delivery_verb,
            descriptor.icon,
            state.target == descriptor.target,
            !live,
        ));
    }

    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(OVERLAY_HEADER_ROW_HEIGHT_PX))
        .child(
            div()
                .w(px(TARGET_BADGE_SLOT_WIDTH_PX))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(STATUS_TEXT_SIZE_PX))
                        .font_family(FONT_SYSTEM_UI)
                        .text_color(timer_color)
                        .child(format_elapsed(state.elapsed)),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .child(chip_row),
        )
        .child(render_static_target_badge_slot(state.target, !live))
}

impl DictationOverlay {
    /// Render the caption band while the app processes the capture.
    ///
    /// While no text is recognized yet the status label ("Transcribing…",
    /// "Delivering…") plus the staggered dot pulse fill the band. Once text
    /// exists the caption alone pulses gently — the pulse IS the working
    /// indicator, so no status label competes with the transcript for the
    /// band's width and the text stays inside the side padding.
    fn render_processing_band(&self, status: &'static str) -> gpui::AnyElement {
        let theme = get_cached_theme();
        let muted_text = theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED);

        let caption_text = self.caption.visible_text();
        if caption_text.trim().is_empty() {
            // Nothing recognized yet: status label + staggered dot pulse.
            let dot_opacities = if self.reduced_motion {
                transcribing_dot_opacities_static()
            } else if let Some(started) = self.processing_started_at {
                transcribing_dot_opacities_at(started.elapsed().as_secs_f64())
            } else {
                transcribing_dot_opacities_static()
            };
            return div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .overflow_hidden()
                .child(
                    div()
                        .text_size(px(STATUS_TEXT_SIZE_PX))
                        .font_family(FONT_SYSTEM_UI)
                        .text_color(muted_text)
                        .whitespace_nowrap()
                        .child(status),
                )
                .child(render_transcribing_dots(&dot_opacities))
                .into_any_element();
        }

        let pulse = if self.reduced_motion {
            processing_pulse_opacity_static()
        } else if let Some(started) = self.processing_started_at {
            processing_pulse_opacity_at(started.elapsed().as_secs_f64())
        } else {
            processing_pulse_opacity_static()
        };

        // The text being worked on keeps its place and pulses; underneath, a
        // status label plus a real progress bar (fed by the chunked finalize
        // pass) says exactly what is happening and how far along it is.
        div()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .gap(px(4.))
            .child(
                self.caption_scroll_container()
                    .text_color(theme.colors.text.primary.with_opacity(OPACITY_ACTIVE))
                    .child(caption_text.trim().to_string())
                    .opacity(pulse),
            )
            .child(self.render_processing_status_row(status, muted_text))
            .into_any_element()
    }

    /// Status label + finalize progress bar shown under the pulsing caption
    /// while the app processes the capture.
    fn render_processing_status_row(
        &self,
        status: &'static str,
        muted_text: gpui::Hsla,
    ) -> gpui::AnyElement {
        let theme = get_cached_theme();
        let progress = crate::dictation::finalize_progress();

        let mut row = div()
            .w_full()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(6.))
            .pb(px(2.))
            .child(
                div()
                    .text_size(px(STATUS_TEXT_SIZE_PX))
                    .font_family(FONT_SYSTEM_UI)
                    .text_color(muted_text)
                    .whitespace_nowrap()
                    .child(status),
            );

        if let Some(fraction) = progress {
            let track_color = theme.colors.ui.border.with_opacity(OPACITY_SUBTLE);
            let fill_color = theme.colors.accent.selected.with_opacity(OPACITY_ACTIVE);
            row = row.child(
                div()
                    .flex_1()
                    .h(px(3.))
                    .rounded(px(999.))
                    .bg(track_color)
                    .child(
                        div()
                            .h_full()
                            .rounded(px(999.))
                            .bg(fill_color)
                            .w(relative(fraction.clamp(0.02, 1.0))),
                    ),
            );
        }

        row.into_any_element()
    }
}
