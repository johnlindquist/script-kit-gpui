#[test]
fn detached_glass_requires_both_classes_and_honors_debug_fallback() {
    assert!(super::native_glass_composition_gate(true, true, false));
    assert!(!super::native_glass_composition_gate(false, true, false));
    assert!(!super::native_glass_composition_gate(true, false, false));
    assert!(!super::native_glass_composition_gate(true, true, true));
}

#[cfg(target_os = "macos")]
#[test]
fn glass_morph_tuning_drives_matching_frame_and_layer_geometry() {
    // Visible-tail calibration (2026-07-26, Oracle session
    // glass-entry-spotlight-retune): the first safe frame is
    // phase-aligned to Spotlight's measured t≈88ms state — 101.2% width
    // at the 0.85 alpha floor — with a 70ms ease-out compression, no
    // hold, and a 140ms ease-in-out rebound. Height participation is 0.
    let tuning = super::glass_morph_tuning_from(0.105, 0.006).expect("morph enabled");
    let epsilon = 1e-12;
    assert!((tuning.start_scale_x - 1.012).abs() < epsilon);
    assert!((tuning.start_scale_y - 1.0).abs() < epsilon);
    // Visible entry starts at 0.85 — the wallpaper-bleed floor (HITL
    // submission 98cab5e5-6f15-4311-8d49-83e31602e641): below it the
    // compositor shows desktop pixels, not faint glass.
    assert!((tuning.start_alpha - 0.85).abs() < epsilon);
    assert!((tuning.squish_scale_x - 0.987).abs() < epsilon);
    assert!((tuning.squish_scale_y - 1.0).abs() < epsilon);
    assert!((tuning.phase1 - 0.035).abs() < epsilon);
    assert!((tuning.phase2 - 0.07).abs() < epsilon);
    assert!((tuning.phase1_alpha_target - 0.99).abs() < epsilon);
    assert!((tuning.alpha_ramp_duration - 0.018).abs() < epsilon);
    assert!((tuning.alpha_finish_duration - 0.026).abs() < epsilon);
    // Material onset prefix (glass-entry-onset-v2, 2x tempo): 44ms
    // Clear→Regular ramp before the 105ms tail — 149ms total. Content
    // fades from the first photon (2026-08-13 content-timing retune):
    // hold 0ms, fade 44ms, ending exactly at tail start.
    assert!((tuning.material_onset_duration - 0.044).abs() < epsilon);
    assert!((tuning.content_hold_duration - 0.0).abs() < epsilon);
    assert!((tuning.content_fade_duration - 0.044).abs() < epsilon);
    assert!((tuning.visible_tail_duration() - 0.105).abs() < epsilon);
    assert!((tuning.total_entry_duration() - 0.149).abs() < epsilon);
    assert_eq!(tuning.visible_tail_start_delay_ms(), 44);
    // Geometry crossing is TAIL-relative (23ms after tail start = 111ms
    // absolute); the reveal anchor is ABSOLUTE from configure:
    // 88 + max(23, 35) = 123ms.
    assert_eq!(super::settled_size_crossing_delay_ms(tuning), 11);
    assert_eq!(super::entry_content_reveal_delay_ms(tuning), 62);
}

#[cfg(target_os = "macos")]
#[test]
fn main_footer_entry_inherits_window_material_without_independent_effects() {
    let policy = super::main_footer_entry_material_policy();
    assert_eq!(policy.target_alpha, 1.0);
    // The gap-spanning CONTAINER never fades (desktop pixels would mix
    // through inter-capsule gaps — the 2026-08-12 defect)…
    assert!(!policy.enroll_in_content_fade);
    assert_eq!(
        policy.defocus_scope,
        super::MainFooterEntryDefocusScope::PerCapsule
    );
    assert_eq!(policy.defocus_scope.log_name(), "per_capsule");
    assert_eq!(policy.defocus_radius, super::glass_main_entry_blur_radius());
    assert_eq!(super::GLASS_MAIN_ENTRY_BLUR_RADIUS, 12.0);
    // …but each clipped capsule materializes exactly like the main
    // backdrop (2026-08-13 parity retune): the Clear→Regular + tint ramp
    // across the onset prefix, with its own foreground contentView
    // joining the shared content fade at the presence floor.
    assert!(policy.material_onset_ramp);
    assert!(policy.foreground_content_fade);
    let epsilon = 1e-9;
    assert!((super::GLASS_ENTRY_CONTENT_START_ALPHA - 0.21).abs() < epsilon);
    assert!((super::glass_entry_content_start_alpha() - 0.21).abs() < epsilon);
}

#[cfg(target_os = "macos")]
#[test]
fn glass_motion_fixture_matches_the_measured_production_calibration() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../scripts/agentic/fixtures/glass-motion-calibration-theme.json"
    ))
    .expect("glass motion calibration fixture should be valid JSON");
    let duration = fixture["opacity"]["glass_morph_duration"]
        .as_f64()
        .expect("fixture duration");
    let inset = fixture["opacity"]["glass_morph_inset"]
        .as_f64()
        .expect("fixture inset");

    let epsilon = 1e-6;
    assert!(
        (duration - f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)).abs() < epsilon
    );
    assert!((inset - f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET)).abs() < epsilon);
    // Visible-tail entry calibration (retuned 2026-07-26, Oracle session
    // glass-entry-spotlight-retune, user-authorized): the first safe
    // frame is phase-aligned to Spotlight's measured t≈88ms state (see
    // https://eager-hollow-dyyf.here.now/). Start alpha stays at the
    // 0.85 wallpaper-bleed floor (HITL 98cab5e5-6f15-4311-8d49-
    // 83e31602e641); it ramps to 0.99 over 35ms and finishes to 1.0
    // over 52ms from rebound start — never fully opaque while wider
    // than natural size. Hidden parking keeps 0.0.
    assert_eq!(super::GLASS_MORPH_ENTRY_START_ALPHA, 0.85);
    assert_eq!(super::GLASS_HIDDEN_PARK_ALPHA, 0.0);
    assert_eq!(super::GLASS_MORPH_VERTICAL_DAMPING, 0.0);
    assert_eq!(super::GLASS_MORPH_SQUISH_FACTOR, 0.25);
    assert_eq!(super::GLASS_MORPH_MIN_SQUISH, 0.0065);
    assert_eq!(super::GLASS_MORPH_MAX_SQUISH, 0.015);
    assert_eq!(super::GLASS_MORPH_SQUISH_HOLD, 0.0);
    assert_eq!(super::GLASS_MORPH_PHASE1_FRACTION, 1.0 / 3.0);
    assert_eq!(super::GLASS_MORPH_PHASE1_ALPHA_TARGET, 0.99);
    assert_eq!(super::GLASS_MORPH_ALPHA_RAMP_DURATION, 0.018);
    assert_eq!(super::GLASS_MORPH_ALPHA_FINISH_DURATION, 0.026);
    assert_eq!(super::GLASS_MORPH_FADE_FRACTION, 2.0 / 3.0);
    assert_eq!(super::GLASS_MATERIAL_ONSET_DURATION, 0.044);
    assert_eq!(super::GLASS_MAIN_ONSET_START_WIDTH_SCALE, 1.0305);
    assert_eq!(super::GLASS_MAIN_ONSET_GEOMETRY_DURATION, 0.018);
    assert_eq!(super::GLASS_ENTRY_CONTENT_HOLD_DURATION, 0.0);
    assert_eq!(super::GLASS_ENTRY_CONTENT_FADE_DURATION, 0.044);
    assert_eq!(super::GLASS_ENTRY_BLUR_RADIUS, 8.0);
    assert_eq!(super::GLASS_MAIN_ENTRY_BLUR_RADIUS, 12.0);
    let tuning =
        super::glass_morph_tuning_from(duration, inset).expect("fixture enables glass morph");
    assert_eq!(super::settled_size_crossing_delay_ms(tuning), 11);
    assert_eq!(super::entry_content_reveal_delay_ms(tuning), 62);
    assert_eq!(tuning.visible_tail_start_delay_ms(), 44);
    assert!((tuning.total_entry_duration() - 0.149).abs() < 1e-9);
    assert_eq!(super::GLASS_EXIT_DURATION, 0.12);
    assert_eq!(super::GLASS_EXIT_REMOVE_DELAY_MS, 135);
    assert_eq!(super::GLASS_EXIT_GROW_X, 0.03);
    assert_eq!(super::GLASS_EXIT_SHRINK_X, 0.05);
    assert_eq!(super::GLASS_EXIT_SHRINK_Y, 0.035);
    assert_eq!(super::GLASS_EXIT_GROW_Y, 0.012);
    assert_eq!(super::GLASS_EXIT_BLUR_RADIUS, 8.0);
}

#[cfg(target_os = "macos")]
#[test]
fn glass_morph_tuning_respects_slider_disable_thresholds() {
    assert!(super::glass_morph_tuning_from(0.0, 0.03).is_none());
    assert!(super::glass_morph_tuning_from(0.105, 0.0).is_none());
}

// ── Entry surface policy (Oracle `glass-entry-feel-options` WP1) ────────
//
// These four lock the BASELINE that the typed-policy refactor had to
// preserve. They are ordinary behavior tests on pure functions, not source
// audits: they assert what the geometry resolves to, so a future retune
// fails them loudly with the actual numbers rather than a string diff.
//
// WP2 will deliberately flip `Main` to grow-in. When it does, the two
// `*_before_the_retune` tests are the ones that must be UPDATED (with the
// measured evidence in the commit), while the two `fractional_*_profile_*`
// tests must keep passing untouched — they pin the shape of each
// direction, not which surface uses it.

/// The shrink-in profile is Spotlight's outset enter: the first visible
/// frame is 1.2% wider than settled, and phase one compresses to 1.3%
/// narrower. At the default inset the squish lands on its 0.0065 floor.
#[cfg(target_os = "macos")]
#[test]
fn fractional_shrink_profile_reproduces_1_012_to_0_987() {
    let tuning = super::glass_morph_tuning_from(
        f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION),
        f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET),
    )
    .expect("default sliders enable the morph");
    let sign = super::GlassEntryDirection::ShrinkIn.geometry_sign();
    assert_eq!(sign, 1.0);

    let final_width = 750.0_f64;
    let travel = super::glass_entry_travel(
        super::GlassEntryTravelPolicy::Fractional,
        final_width,
        tuning.inset_fraction,
        super::GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
    );
    // Width path, expressed the way the motion contract states it.
    let start_scale = 1.0 + (travel.start_per_side * 2.0 / final_width) * sign;
    let extreme_scale = 1.0 - (travel.extreme_per_side * 2.0 / final_width) * sign;
    assert!(
        (start_scale - 1.012).abs() < 1e-9,
        "shrink-in must start at 101.2% width, got {start_scale}"
    );
    assert!(
        (extreme_scale - 0.987).abs() < 1e-9,
        "shrink-in must compress to 98.7% width, got {extreme_scale}"
    );
    // Squish floor: 0.006 × 0.25 = 0.0015 clamps up to 0.0065 per side.
    assert!((travel.extreme_per_side / final_width - super::GLASS_MORPH_MIN_SQUISH).abs() < 1e-9);
}

/// Grow-in is the same magnitudes with inverted travel: start 1.2% NARROWER
/// and overshoot 1.3% wider before settling.
#[cfg(target_os = "macos")]
#[test]
fn fractional_grow_profile_reproduces_0_988_to_1_013() {
    let tuning = super::glass_morph_tuning_from(
        f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION),
        f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET),
    )
    .expect("default sliders enable the morph");
    let sign = super::GlassEntryDirection::GrowIn.geometry_sign();
    assert_eq!(sign, -1.0);

    let final_width = 340.0_f64;
    let travel = super::glass_entry_travel(
        super::GlassEntryTravelPolicy::Fractional,
        final_width,
        tuning.inset_fraction,
        super::GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
    );
    let start_scale = 1.0 + (travel.start_per_side * 2.0 / final_width) * sign;
    let extreme_scale = 1.0 - (travel.extreme_per_side * 2.0 / final_width) * sign;
    assert!(
        (start_scale - 0.988).abs() < 1e-9,
        "grow-in must start at 98.8% width, got {start_scale}"
    );
    assert!(
        (extreme_scale - 1.013).abs() < 1e-9,
        "grow-in must overshoot to 101.3% width, got {extreme_scale}"
    );

    // The point-travel asymmetry Oracle identified: the SAME fractional
    // policy moves a 750pt main window ~2.2x further per side than this
    // 340pt popup, over the same 35ms phase one.
    let main_travel = super::glass_entry_travel(
        super::GlassEntryTravelPolicy::Fractional,
        750.0,
        tuning.inset_fraction,
        super::GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
    );
    let ratio = main_travel.start_per_side / travel.start_per_side;
    assert!(
        (ratio - 750.0 / 340.0).abs() < 1e-9,
        "fractional travel must scale linearly with width, got {ratio}"
    );
    // Capping a 750pt window to this popup's reference makes the point
    // travel identical — the WP2 candidate, proven here before it ships.
    let capped = super::glass_entry_travel(
        super::GlassEntryTravelPolicy::ActionsPointCapped,
        750.0,
        tuning.inset_fraction,
        super::GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
    );
    assert!((capped.start_per_side - travel.start_per_side).abs() < 1e-9);
    assert!((capped.extreme_per_side - travel.extreme_per_side).abs() < 1e-9);
}

/// 2026-08-13 soft-materialize retune lock: main ALONE owns the wider
/// first-photon onset geometry, while secondary and child-popup
/// directions, travel, and material onset remain unchanged.
#[cfg(target_os = "macos")]
#[test]
fn main_owns_the_soft_materialize_prefix_while_secondary_remains_tail_aligned() {
    let main = super::glass_entry_policy(super::GlassEntrySurface::Main);
    assert_eq!(main.direction, super::GlassEntryDirection::ShrinkIn);
    assert_eq!(main.travel, super::GlassEntryTravelPolicy::Fractional);
    assert_eq!(main.onset, super::GlassEntryOnsetPolicy::Full);
    assert_eq!(
        main.onset_geometry,
        super::GlassEntryOnsetGeometry::SpotlightSoftMaterialize
    );
    let secondary = super::glass_entry_policy(super::GlassEntrySurface::FreeStandingSecondary);
    assert_eq!(secondary.direction, super::GlassEntryDirection::ShrinkIn);
    assert_eq!(secondary.travel, super::GlassEntryTravelPolicy::Fractional);
    assert_eq!(secondary.onset, super::GlassEntryOnsetPolicy::Full);
    assert_eq!(
        secondary.onset_geometry,
        super::GlassEntryOnsetGeometry::TailAligned
    );
}

/// The Cmd+K Actions popup keeps the grow-in direction the user named as
/// the reference feel, and stays tail-aligned.
#[cfg(target_os = "macos")]
#[test]
fn child_popup_remains_tail_aligned_grow_in() {
    let child = super::glass_entry_policy(super::GlassEntrySurface::ChildPopup);
    assert_eq!(child.direction, super::GlassEntryDirection::GrowIn);
    assert_eq!(child.travel, super::GlassEntryTravelPolicy::Fractional);
    assert_eq!(child.onset, super::GlassEntryOnsetPolicy::Full);
    assert_eq!(
        child.onset_geometry,
        super::GlassEntryOnsetGeometry::TailAligned
    );
    assert_eq!(child.direction.geometry_sign(), -1.0);
    assert_eq!(child.direction.log_name(), "grow_in");
}

/// 2026-08-13 soft-materialize retune lock (measured from the
/// user-supplied 57fps Spotlight reference): main's first photon is
/// 103.05% of settled width converging to the preserved 101.2% tail start
/// over 18ms; its onset defocus is 12pt resolved inside the 44ms material
/// prefix; the total entry stays 149ms; non-main surfaces stay
/// tail-aligned with the historical full-entry 8pt ramp.
#[cfg(target_os = "macos")]
#[test]
fn main_entry_restores_the_spotlight_soft_materialize_prefix() {
    let tuning = super::glass_morph_tuning_from(0.105, 0.006).expect("morph enabled");
    let main = super::glass_entry_policy(super::GlassEntrySurface::Main);
    assert_eq!(
        super::glass_entry_surface_for_window_frame(true),
        super::GlassEntrySurface::Main
    );
    assert_eq!(
        super::glass_entry_surface_for_window_frame(false),
        super::GlassEntrySurface::FreeStandingSecondary
    );
    let tail_start_per_side = 750.0 * tuning.inset_fraction;
    let onset_start_per_side = main
        .onset_geometry
        .start_per_side(750.0, tail_start_per_side);
    let onset_start_scale = 1.0 + onset_start_per_side * 2.0 / 750.0;
    let tail_start_scale = 1.0 + tail_start_per_side * 2.0 / 750.0;
    let epsilon = 1e-12;

    assert_eq!(
        main.onset_geometry,
        super::GlassEntryOnsetGeometry::SpotlightSoftMaterialize
    );
    assert!((onset_start_scale - 1.0305).abs() < epsilon);
    assert!((tail_start_scale - 1.012).abs() < epsilon);
    assert!((main.onset_geometry.duration() - 0.018).abs() < epsilon);
    assert_eq!(super::GLASS_MAIN_ENTRY_BLUR_RADIUS, 12.0);
    assert!((super::GlassEntrySurface::Main.entry_blur_duration(tuning) - 0.044).abs() < epsilon);
    let footer = super::main_footer_entry_material_policy();
    assert_eq!(
        footer.defocus_scope,
        super::MainFooterEntryDefocusScope::PerCapsule
    );
    assert_eq!(
        footer.defocus_radius,
        super::GlassEntrySurface::Main.entry_blur_radius()
    );
    assert!((tuning.material_onset_duration - 0.044).abs() < epsilon);
    assert!((tuning.total_entry_duration() - 0.149).abs() < epsilon);

    for surface in [
        super::GlassEntrySurface::ChildPopup,
        super::GlassEntrySurface::FreeStandingSecondary,
    ] {
        let policy = super::glass_entry_policy(surface);
        assert_eq!(
            policy.onset_geometry,
            super::GlassEntryOnsetGeometry::TailAligned
        );
        assert_eq!(policy.onset_geometry.duration(), 0.0);
        assert!((surface.entry_blur_duration(tuning) - 0.149).abs() < epsilon);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn detached_main_backdrop_excludes_footer_and_exact_eight_point_gutter() {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};

    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(750.0, 501.0));
    let layout = super::TahoeBackdropLayout::ContentAboveDetachedFooter { bottom_inset: 40.0 };
    let frame = layout.frame(bounds);
    assert_eq!(frame.origin.x, 0.0);
    assert_eq!(frame.origin.y, 40.0);
    assert_eq!(frame.size.width, 750.0);
    assert_eq!(frame.size.height, 461.0);
    assert_eq!(layout.bottom_inset(), 40.0);

    let footer_top = 32.0;
    assert_eq!(frame.origin.y - footer_top, 8.0);
    assert!(footer_top <= frame.origin.y);
}

/// The exit-motion contract and the backdrop partition are now SEPARATE
/// decisions: Notes keeps the calibrated fixed-frame fade exit in both
/// surface modes while its DEFAULT backdrop is full-window (its Agent
/// mode reserves the footer band dynamically through
/// `set_gpui_window_backdrop_bottom_inset`).
#[cfg(target_os = "macos")]
#[test]
fn notes_defaults_to_full_backdrop() {
    // Default (static) partition owners: Main + Dictation only.
    for window_name in ["Main window", "Dictation overlay"] {
        assert!(
            super::window_name_owns_detached_footer(window_name),
            "{window_name} must own the default floating footer partition"
        );
    }
    assert!(
        !super::window_name_owns_detached_footer("Notes"),
        "Notes must default to the full-window backdrop (Notes mode has no footer)"
    );
    assert!(!super::window_name_owns_detached_footer("Actions popup"));
}

#[cfg(target_os = "macos")]
#[test]
fn notes_exit_remains_fixed_frame_fade_only_in_both_modes() {
    // The calibrated exit motion keys off the fixed-frame-exit set, NOT
    // the backdrop layout — Notes stays fade-only with zero inset.
    for window_name in ["Main window", "Notes", "Dictation overlay"] {
        assert!(super::window_name_uses_fixed_frame_exit(window_name));
        assert_eq!(
            super::glass_exit_mode(window_name),
            super::GlassExitMode::DetachedRegionsFadeOnly
        );
    }
    for window_name in ["Actions popup", "Confirm popup", "Inline popup"] {
        assert!(!super::window_name_uses_fixed_frame_exit(window_name));
        assert_eq!(
            super::glass_exit_mode(window_name),
            super::GlassExitMode::PopupTransformAndBlur
        );
    }
    assert_eq!(super::glass_exit_remove_delay().as_millis(), 135);
}

#[cfg(target_os = "macos")]
#[test]
fn notes_backdrop_layout_can_toggle_without_changing_outer_frame() {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};

    // The same outer content bounds partition into full-window and
    // footer-inset layouts; toggling layouts never changes the outer
    // frame, only the backdrop's share of it.
    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(350.0, 280.0));
    let full = super::TahoeBackdropLayout::FullWindow.frame(bounds);
    assert_eq!(full.origin.y, 0.0);
    assert_eq!(full.size.height, 280.0);

    let inset = 44.0;
    let partitioned = super::TahoeBackdropLayout::ContentAboveDetachedFooter {
        bottom_inset: inset,
    }
    .frame(bounds);
    assert_eq!(partitioned.origin.y, inset);
    assert_eq!(partitioned.size.height, 280.0 - inset);
    assert_eq!(partitioned.size.width, full.size.width);

    // Round-trip back to full-window restores the exact original frame.
    let restored = super::TahoeBackdropLayout::FullWindow.frame(bounds);
    assert_eq!(restored.origin.y, full.origin.y);
    assert_eq!(restored.size.height, full.size.height);
}

#[cfg(target_os = "macos")]
#[test]
fn native_glass_style_locks_tint_floor_and_preserves_requested_tint_semantics() {
    let mut inherited = crate::theme::Theme::default();
    inherited.opacity.as_mut().unwrap().glass_tint_opacity = None;
    let mut explicit_zero = inherited.clone();
    explicit_zero.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.0);
    let mut below_floor = inherited.clone();
    below_floor.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.34);
    let mut at_floor = inherited.clone();
    at_floor.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.35);
    let mut above_floor = inherited.clone();
    above_floor.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.72);
    let light = crate::theme::Theme::light_default();

    let backdrop = super::resolve_native_glass_style(
        &inherited,
        super::NativeGlassSurfaceRole::WindowBackdrop,
    );
    let capsule = super::resolve_native_glass_style(
        &inherited,
        super::NativeGlassSurfaceRole::FloatingCapsule,
    );
    let explicit = super::resolve_native_glass_style(
        &explicit_zero,
        super::NativeGlassSurfaceRole::FloatingCapsule,
    );
    let below = super::resolve_native_glass_style(
        &below_floor,
        super::NativeGlassSurfaceRole::FloatingCapsule,
    );
    let at = super::resolve_native_glass_style(
        &at_floor,
        super::NativeGlassSurfaceRole::FloatingCapsule,
    );
    let above = super::resolve_native_glass_style(
        &above_floor,
        super::NativeGlassSurfaceRole::FloatingCapsule,
    );
    let light_capsule =
        super::resolve_native_glass_style(&light, super::NativeGlassSurfaceRole::FloatingCapsule);

    assert_eq!(backdrop.signature.tint_rgb, capsule.signature.tint_rgb);
    assert_eq!(
        backdrop.signature.effective_tint_alpha_bits,
        capsule.signature.effective_tint_alpha_bits
    );
    // Material/appearance parity: both roles resolve the same appearance
    // mode from the same theme — the capsule may not diverge to its own
    // dark/light decision.
    assert_eq!(backdrop.signature.dark, capsule.signature.dark);
    let light_backdrop =
        super::resolve_native_glass_style(&light, super::NativeGlassSurfaceRole::WindowBackdrop);
    assert_eq!(
        light_backdrop.signature.tint_rgb,
        light_capsule.signature.tint_rgb
    );
    assert_eq!(
        light_backdrop.signature.effective_tint_alpha_bits,
        light_capsule.signature.effective_tint_alpha_bits
    );
    assert_eq!(light_backdrop.signature.dark, light_capsule.signature.dark);
    // The native capsule veil resolves through the chrome token. It is
    // 0.0 during the 2026-07-27 user-authorized "perfectly match the main
    // window" experiment, so the capsule material intentionally equals the
    // backdrop material; only the separation rim distinguishes them.
    assert_eq!(
        capsule.veil_alpha,
        crate::ui::chrome::LIQUID_GLASS_CAPSULE_VEIL_ALPHA
    );
    assert_eq!(
        backdrop.effective_tint_alpha,
        crate::ui::chrome::LIQUID_GLASS_STABILITY_TINT_ALPHA_FLOOR
    );
    assert_eq!(backdrop.signature.requested_tint_alpha_bits, None);
    assert_eq!(
        explicit.signature.requested_tint_alpha_bits,
        Some(0.0_f32.to_bits())
    );
    for style in [backdrop, capsule, explicit, below, at] {
        assert_eq!(style.effective_tint_alpha, 0.35);
    }
    assert_eq!(above.effective_tint_alpha, 0.72);
    assert_eq!(backdrop.veil_alpha, 0.0);
    assert_eq!(backdrop.rim_width, 0.0);
    assert_eq!(backdrop.signature.rim_rgba, 0xFFFF_FF00);
    assert_eq!(capsule.veil_alpha, 0.0);
    assert_eq!(capsule.rim_width, 1.0);
    assert_eq!(capsule.signature.rim_rgba, 0xFFFF_FF3D);
    assert_eq!(light_capsule.veil_alpha, 0.0);
    assert_eq!(light_capsule.rim_width, 1.0);
    assert_eq!(light_capsule.signature.rim_rgba, 0x0000_002E);
    assert_ne!(backdrop.signature, explicit.signature);
}

/// The material stack must be STATIC between morph start and settle: only
/// the initial installation and explicitly recorded theme refreshes may
/// apply native styles. A per-frame tint/veil/opacity animation during
/// entry would land here as an in-span `Install` re-application — the
/// ledger flags it so runtime receipts (and probes reading
/// `native_glass_style_mutation_during_entry` error events) can prove
/// `styleMutationCountDuringEntry == 0`.
#[cfg(target_os = "macos")]
#[test]
fn native_glass_style_ledger_flags_only_mid_entry_reapplications() {
    let theme = crate::theme::Theme::default();
    let signature =
        super::resolve_native_glass_style(&theme, super::NativeGlassSurfaceRole::FloatingCapsule)
            .signature;
    let application = |window_number: i64,
                       surface_id: usize,
                       at_ns: u64,
                       reason: super::NativeGlassStyleApplicationReason| {
        super::NativeGlassStyleApplication {
            window_number,
            surface_id,
            at_ns,
            reason,
            signature,
        }
    };
    use super::NativeGlassStyleApplicationReason::{Install, ThemeRefresh};

    let mut ledger = super::NativeGlassStyleLedger::default();
    ledger.record_entry_span(7, 1_000, 2_000);
    // Initial install before the morph span: allowed.
    assert!(!ledger.record_application(application(7, 70, 900, Install)));
    // Pooled capsule views style themselves before attaching to a window
    // (window number -1): never counted against a real window's span.
    assert!(!ledger.record_application(application(-1, 71, 1_100, Install)));
    // A distinct capsule in the same window gets its own initial install;
    // it is not a reapplication of surface 70.
    assert!(!ledger.record_application(application(7, 71, 1_100, Install)));
    // Theme refresh mid-span is the explicitly recorded exception.
    assert!(!ledger.record_application(application(7, 70, 1_200, ThemeRefresh)));
    assert_eq!(ledger.style_mutation_count_during_entry(7), 0);
    assert!(ledger.has_identical_surface_style_during_entry(7, 70, 1_300, signature));
    assert!(!ledger.has_identical_surface_style_during_entry(7, 72, 1_300, signature));
    // A tint/veil/opacity animation during entry looks exactly like this:
    // an Install-shaped re-application of the same native surface inside
    // the span. It must be flagged — this is the assertion that fails if
    // anyone temporarily mutates capsule tint during entry.
    assert!(ledger.record_application(application(7, 70, 1_500, Install)));
    assert_eq!(ledger.style_mutation_count_during_entry(7), 1);
    // After settle, re-applications are outside the entry contract.
    assert!(!ledger.record_application(application(7, 70, 2_500, Install)));
    assert_eq!(ledger.style_mutation_count_during_entry(7), 1);
    // A first-ever installation landing inside its own span (window whose
    // backdrop is created mid-morph) is the initial install, not a
    // mutation.
    ledger.record_entry_span(9, 1_000, 2_000);
    assert!(!ledger.record_application(application(9, 90, 1_050, Install)));
    assert_eq!(ledger.style_mutation_count_during_entry(9), 0);
    assert!(ledger.record_application(application(9, 90, 1_060, Install)));
    assert_eq!(ledger.style_mutation_count_during_entry(9), 1);
}

#[cfg(target_os = "macos")]
#[test]
fn secondary_backdrop_remains_full_window() {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};

    let bounds = NSRect::new(NSPoint::new(3.0, 5.0), NSSize::new(420.0, 280.0));
    let frame = super::TahoeBackdropLayout::FullWindow.frame(bounds);
    assert_eq!(frame.origin.x, bounds.origin.x);
    assert_eq!(frame.origin.y, bounds.origin.y);
    assert_eq!(frame.size.width, bounds.size.width);
    assert_eq!(frame.size.height, bounds.size.height);
}
