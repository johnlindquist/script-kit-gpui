#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeGlassSurfaceRole {
    WindowBackdrop,
    FloatingCapsule,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeGlassStyleSignature {
    pub(crate) dark: bool,
    pub(crate) tint_rgb: u32,
    pub(crate) requested_tint_alpha_bits: Option<u32>,
    pub(crate) effective_tint_alpha_bits: u32,
    pub(crate) veil_alpha_bits: u32,
    pub(crate) rim_rgba: u32,
    pub(crate) rim_width_bits: u32,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeGlassStyle {
    pub(crate) role: NativeGlassSurfaceRole,
    pub(crate) signature: NativeGlassStyleSignature,
    pub(crate) effective_tint_alpha: f32,
    pub(crate) veil_alpha: f32,
    pub(crate) rim_width: f32,
}

#[cfg(target_os = "macos")]
pub(crate) fn resolve_native_glass_style(
    theme: &crate::theme::Theme,
    role: NativeGlassSurfaceRole,
) -> NativeGlassStyle {
    let requested_tint_alpha = theme.get_opacity().glass_tint_opacity;
    let matched = crate::ui_foundation::main_window_matched_background_rgba(theme);
    let tint_rgb = (matched >> 8) & 0x00ff_ffff;
    let tint_floor = crate::ui::chrome::LIQUID_GLASS_STABILITY_TINT_ALPHA_FLOOR;
    let effective_tint_alpha = requested_tint_alpha
        .unwrap_or(tint_floor)
        .max(tint_floor)
        .clamp(0.0, 1.0);
    let capsule = matches!(role, NativeGlassSurfaceRole::FloatingCapsule);
    let veil_alpha = if capsule {
        crate::ui::chrome::LIQUID_GLASS_CAPSULE_VEIL_ALPHA
    } else {
        0.0
    };
    let rim_alpha = if capsule {
        if theme.should_use_dark_vibrancy() {
            crate::ui::chrome::LIQUID_GLASS_CAPSULE_RIM_ALPHA_DARK
        } else {
            crate::ui::chrome::LIQUID_GLASS_CAPSULE_RIM_ALPHA_LIGHT
        }
    } else {
        0.0
    };
    let rim_width = if rim_alpha > 0.0 {
        crate::ui::chrome::LIQUID_GLASS_CAPSULE_RIM_WIDTH_PX
    } else {
        0.0
    };
    let rim_color = if theme.should_use_dark_vibrancy() {
        0xff_ff_ff
    } else {
        0x00_00_00
    };
    let rim_rgba = (rim_color << 8) | (rim_alpha * 255.0).round() as u32;
    NativeGlassStyle {
        role,
        signature: NativeGlassStyleSignature {
            dark: theme.should_use_dark_vibrancy(),
            tint_rgb,
            requested_tint_alpha_bits: requested_tint_alpha.map(f32::to_bits),
            effective_tint_alpha_bits: effective_tint_alpha.to_bits(),
            veil_alpha_bits: veil_alpha.to_bits(),
            rim_rgba,
            rim_width_bits: rim_width.to_bits(),
        },
        effective_tint_alpha,
        veil_alpha,
        rim_width,
    }
}

/// Why a native glass style application happened. The material contract
/// allows exactly two temporal shapes: the initial installation of a surface
/// and an explicitly recorded theme refresh. Anything else that lands between
/// morph start and settle is a per-frame material mutation — the exact class
/// of change the Glass Motion Calibration Lock forbids (tint RGB, tint alpha,
/// veil alpha, and native layer opacity must be static during entry).
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeGlassStyleApplicationReason {
    Install,
    ThemeRefresh,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct NativeGlassStyleApplication {
    window_number: i64,
    surface_id: usize,
    at_ns: u64,
    reason: NativeGlassStyleApplicationReason,
    signature: NativeGlassStyleSignature,
}

/// Pure, testable record of entry spans and style applications so the runtime
/// can prove `styleMutationCountDuringEntry == 0` instead of asserting it
/// from source reading.
#[cfg(target_os = "macos")]
#[derive(Default)]
struct NativeGlassStyleLedger {
    /// `(window_number, morph_start_ns, settle_end_ns)`; one live span per
    /// window (re-entry replaces the previous span).
    entry_spans: Vec<(i64, u64, u64)>,
    applications: Vec<NativeGlassStyleApplication>,
}

#[cfg(target_os = "macos")]
const NATIVE_GLASS_STYLE_LEDGER_CAPACITY: usize = 512;

#[cfg(target_os = "macos")]
impl NativeGlassStyleLedger {
    fn record_entry_span(&mut self, window_number: i64, start_ns: u64, end_ns: u64) {
        self.entry_spans
            .retain(|(window, _, _)| *window != window_number);
        self.entry_spans.push((window_number, start_ns, end_ns));
        if self.entry_spans.len() > NATIVE_GLASS_STYLE_LEDGER_CAPACITY {
            self.entry_spans.remove(0);
        }
    }

    fn entry_span(&self, window_number: i64) -> Option<(u64, u64)> {
        self.entry_spans
            .iter()
            .find(|(window, _, _)| *window == window_number)
            .map(|(_, start, end)| (*start, *end))
    }

    /// Whether an identical `Install` has already styled this exact native
    /// surface during the active entry span. Reapplying the same signature can
    /// still churn NSGlassEffectView's private material tree, so callers skip
    /// it until the entry settles; distinct capsules in the same window remain
    /// independent surfaces and must each receive their initial style.
    fn has_identical_surface_style_during_entry(
        &self,
        window_number: i64,
        surface_id: usize,
        at_ns: u64,
        signature: NativeGlassStyleSignature,
    ) -> bool {
        let in_span = self
            .entry_span(window_number)
            .is_some_and(|(start, end)| at_ns >= start && at_ns <= end);
        in_span
            && self.applications.iter().rev().any(|prior| {
                prior.window_number == window_number
                    && prior.surface_id == surface_id
                    && prior.signature == signature
            })
    }

    /// Record one application; returns `true` when it is a forbidden
    /// mid-entry mutation: an `Install`-shaped (re)application inside the
    /// window's morph span with any earlier application for the same native
    /// surface. The initial installation of each distinct surface and
    /// explicitly tagged theme refreshes are the only allowed in-span shapes.
    fn record_application(&mut self, application: NativeGlassStyleApplication) -> bool {
        let in_span = self
            .entry_span(application.window_number)
            .is_some_and(|(start, end)| application.at_ns >= start && application.at_ns <= end);
        let has_prior = self.applications.iter().any(|prior| {
            prior.window_number == application.window_number
                && prior.surface_id == application.surface_id
        });
        let mutation = in_span
            && has_prior
            && application.reason == NativeGlassStyleApplicationReason::Install;
        self.applications.push(application);
        if self.applications.len() > NATIVE_GLASS_STYLE_LEDGER_CAPACITY {
            self.applications.remove(0);
        }
        mutation
    }

    fn style_mutation_count_during_entry(&self, window_number: i64) -> usize {
        let Some((start, end)) = self.entry_span(window_number) else {
            return 0;
        };
        let mut seen_surfaces: std::collections::HashSet<usize> = self
            .applications
            .iter()
            .filter(|app| app.window_number == window_number && app.at_ns < start)
            .map(|app| app.surface_id)
            .collect();
        let mut count = 0;
        for application in self
            .applications
            .iter()
            .filter(|app| app.window_number == window_number)
            .filter(|app| app.at_ns >= start && app.at_ns <= end)
        {
            if seen_surfaces.contains(&application.surface_id)
                && application.reason == NativeGlassStyleApplicationReason::Install
            {
                count += 1;
            }
            seen_surfaces.insert(application.surface_id);
        }
        count
    }
}

#[cfg(target_os = "macos")]
static NATIVE_GLASS_STYLE_LEDGER: std::sync::Mutex<Option<NativeGlassStyleLedger>> =
    std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
fn with_native_glass_style_ledger<T>(
    operation: impl FnOnce(&mut NativeGlassStyleLedger) -> T,
) -> T {
    let mut guard = NATIVE_GLASS_STYLE_LEDGER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    operation(guard.get_or_insert_with(NativeGlassStyleLedger::default))
}

/// Record the morph span so any style application landing inside it can be
/// classified. Called at morph start by every entry variant.
#[cfg(target_os = "macos")]
unsafe fn record_native_glass_entry_span(window: id, duration_seconds: f64) {
    if window == nil {
        return;
    }
    let window_number: i64 = msg_send![window, windowNumber];
    if window_number <= 0 {
        return;
    }
    let start_ns = crate::platform::host_clock::host_time_ns();
    let end_ns = start_ns.saturating_add((duration_seconds.max(0.0) * 1e9) as u64);
    with_native_glass_style_ledger(|ledger| {
        ledger.record_entry_span(window_number, start_ns, end_ns)
    });
}

/// Runtime count of forbidden mid-entry style mutations for a window. A
/// healthy entry reports 0.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub(crate) fn native_glass_style_mutation_count_during_entry(window_number: i64) -> usize {
    with_native_glass_style_ledger(|ledger| ledger.style_mutation_count_during_entry(window_number))
}

/// Apply the complete shared native glass policy. AppKit mutations are made
/// in one disabled-actions transaction so a theme refresh cannot expose an
/// intermediate untinted or mismatched frame.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn apply_native_glass_style(glass_view: id, style: NativeGlassStyle) -> bool {
    apply_native_glass_style_with_reason(
        glass_view,
        style,
        NativeGlassStyleApplicationReason::Install,
    )
}

/// See [`apply_native_glass_style`]; `reason` feeds the style-application
/// ledger that proves the material stack stays static during entry.
///
/// # Safety
/// Same contract as [`apply_native_glass_style`].
#[cfg(target_os = "macos")]
pub(crate) unsafe fn apply_native_glass_style_with_reason(
    glass_view: id,
    style: NativeGlassStyle,
    reason: NativeGlassStyleApplicationReason,
) -> bool {
    if glass_view == nil {
        return false;
    }
    let responds: bool = msg_send![glass_view, respondsToSelector: sel!(setTintColor:)];
    if !responds {
        return false;
    }
    let window: id = msg_send![glass_view, window];
    let window_number: i64 = if window != nil {
        msg_send![window, windowNumber]
    } else {
        -1
    };
    let surface_id = glass_view as usize;
    let at_ns = crate::platform::host_clock::host_time_ns();
    let skip_identical_install = reason == NativeGlassStyleApplicationReason::Install
        && with_native_glass_style_ledger(|ledger| {
            ledger.has_identical_surface_style_during_entry(
                window_number,
                surface_id,
                at_ns,
                style.signature,
            )
        });
    if skip_identical_install {
        tracing::debug!(
            target: "script_kit::native_glass",
            event = "native_glass_style_identical_install_skipped_during_entry",
            window_number,
            surface_id,
            at_ns,
            "native_glass_style_identical_install_skipped_during_entry"
        );
        return true;
    }
    let transaction_class = objc::runtime::Class::get("CATransaction");
    if let Some(transaction_class) = transaction_class {
        let _: () = msg_send![transaction_class, begin];
        let _: () = msg_send![transaction_class, setDisableActions: cocoa::base::YES];
    }
    let appearance_name = if style.signature.dark {
        tahoe_ns_string("NSAppearanceNameVibrantDark")
    } else {
        tahoe_ns_string("NSAppearanceNameVibrantLight")
    };
    if appearance_name != nil {
        let appearance: id = msg_send![class!(NSAppearance), appearanceNamed: appearance_name];
        if appearance != nil {
            let _: () = msg_send![glass_view, setAppearance: appearance];
        }
    }
    let red = f64::from((style.signature.tint_rgb >> 16) & 0xff) / 255.0;
    let green = f64::from((style.signature.tint_rgb >> 8) & 0xff) / 255.0;
    let blue = f64::from(style.signature.tint_rgb & 0xff) / 255.0;
    let tint: id = msg_send![
        class!(NSColor),
        colorWithCalibratedRed: red
        green: green
        blue: blue
        alpha: f64::from(style.effective_tint_alpha)
    ];
    let _: () = msg_send![glass_view, setTintColor: tint];

    let _: () = msg_send![glass_view, setWantsLayer: cocoa::base::YES];
    let content_view: id = msg_send![glass_view, contentView];
    let mut content_layer = nil;
    if content_view != nil {
        let _: () = msg_send![content_view, setWantsLayer: cocoa::base::YES];
        content_layer = msg_send![content_view, layer];
        if content_layer != nil {
            let veil: id = msg_send![
                class!(NSColor),
                colorWithCalibratedRed: red
                green: green
                blue: blue
                alpha: f64::from(style.veil_alpha)
            ];
            let veil_cg: *const std::ffi::c_void = msg_send![veil, CGColor];
            let _: () = msg_send![content_layer, setBackgroundColor: veil_cg];
            let _: () = msg_send![content_layer, setMasksToBounds: cocoa::base::YES];
            if matches!(style.role, NativeGlassSurfaceRole::FloatingCapsule) {
                let _: () = msg_send![
                    content_layer,
                    setCornerRadius:
                        f64::from(crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX)
                ];
            }
        }
    }
    let layer: id = msg_send![glass_view, layer];
    if layer != nil {
        if matches!(style.role, NativeGlassSurfaceRole::FloatingCapsule) {
            let _: () = msg_send![
                layer,
                setCornerRadius:
                    f64::from(crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX)
            ];
        }
        let rim_red = f64::from((style.signature.rim_rgba >> 24) & 0xff) / 255.0;
        let rim_green = f64::from((style.signature.rim_rgba >> 16) & 0xff) / 255.0;
        let rim_blue = f64::from((style.signature.rim_rgba >> 8) & 0xff) / 255.0;
        let rim_alpha = f64::from(style.signature.rim_rgba & 0xff) / 255.0;
        let rim: id = msg_send![
            class!(NSColor),
            colorWithCalibratedRed: rim_red
            green: rim_green
            blue: rim_blue
            alpha: rim_alpha
        ];
        let rim_cg: *const std::ffi::c_void = msg_send![rim, CGColor];
        // The foreground content layer is the final visible capsule surface.
        // Put the separation rim there rather than behind NSGlassEffectView's
        // private material hierarchy.
        if content_layer != nil {
            let _: () = msg_send![content_layer, setBorderColor: rim_cg];
            let _: () = msg_send![content_layer, setBorderWidth: f64::from(style.rim_width)];
        }
        let _: () = msg_send![layer, setBorderWidth: 0.0f64];
        // R is the locked production treatment. Clear any stale shadow state
        // left by a recycled AppKit view so RS cannot leak into production.
        let _: () = msg_send![layer, setShadowOpacity: 0.0f32];
        let _: () = msg_send![layer, setShadowRadius: 0.0f64];
        let shadow_offset = cocoa::foundation::NSSize::new(0.0, 0.0);
        let _: () = msg_send![layer, setShadowOffset: shadow_offset];
        let _: () = msg_send![layer, setShadowPath: nil];
    }
    if let Some(transaction_class) = transaction_class {
        let _: () = msg_send![transaction_class, commit];
    }
    let mid_entry_mutation = with_native_glass_style_ledger(|ledger| {
        ledger.record_application(NativeGlassStyleApplication {
            window_number,
            surface_id,
            at_ns,
            reason,
            signature: style.signature,
        })
    });
    let role_name = match style.role {
        NativeGlassSurfaceRole::WindowBackdrop => "window_backdrop",
        NativeGlassSurfaceRole::FloatingCapsule => "floating_capsule",
    };
    tracing::info!(
        target: "script_kit::native_glass",
        event = "native_glass_style_applied",
        window_number,
        surface_id,
        at_ns,
        role = role_name,
        reason = match reason {
            NativeGlassStyleApplicationReason::Install => "install",
            NativeGlassStyleApplicationReason::ThemeRefresh => "theme_refresh",
        },
        material = current_window_material_name(current_window_material()),
        dark = style.signature.dark,
        tint_rgb = style.signature.tint_rgb,
        requested_tint_alpha_bits = ?style.signature.requested_tint_alpha_bits,
        effective_tint_alpha_bits = style.signature.effective_tint_alpha_bits,
        effective_tint_alpha = style.effective_tint_alpha,
        veil_alpha_bits = style.signature.veil_alpha_bits,
        veil_alpha = style.veil_alpha,
        rim_rgba = style.signature.rim_rgba,
        rim_width_bits = style.signature.rim_width_bits,
        "native_glass_style_applied"
    );
    if mid_entry_mutation {
        // The material stack must be static between morph start and settle.
        // This is the runtime tripwire the probes assert against: a healthy
        // entry emits zero of these events.
        tracing::error!(
            target: "script_kit::native_glass",
            event = "native_glass_style_mutation_during_entry",
            window_number,
            surface_id,
            at_ns,
            role = role_name,
            "native_glass_style_mutation_during_entry"
        );
    }
    true
}

/// Compatibility entry point for callers that do not yet own a role.
///
/// # Safety
/// `glass_view` must be a valid NSGlassEffectView (or nil-checked upstream)
/// on the main thread.
#[cfg(target_os = "macos")]
#[allow(
    dead_code,
    reason = "the legacy native glass-tint adapter is retained for callers without a surface role"
)]
pub(crate) unsafe fn apply_theme_glass_tint(glass_view: id) -> bool {
    let theme = crate::theme::get_cached_theme();
    apply_native_glass_style_with_reason(
        glass_view,
        resolve_native_glass_style(&theme, NativeGlassSurfaceRole::WindowBackdrop),
        NativeGlassStyleApplicationReason::ThemeRefresh,
    )
}
