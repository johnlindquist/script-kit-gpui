/// Configure the vibrancy blur for the main window based on appearance mode.
///
/// This is the appearance-aware version that should be called after loading the theme.
/// Uses VibrantLight for light mode, VibrantDark for dark mode.
///
/// # Arguments
/// * `is_dark` - true for dark mode (VibrantDark), false for light mode (VibrantLight)
/// * `material` - the user-selected NSVisualEffect material to apply
///
/// # macOS Behavior
///
/// Clears the window's NSAppearance (sets to nil) so GPUI can detect system appearance
/// changes, then sets the appearance on each NSVisualEffectView individually along with
/// appropriate material and state.
///
/// # Safety
///
/// Uses Objective-C message sending internally.
#[cfg(target_os = "macos")]
static LAST_MAIN_WINDOW_VIBRANCY_SIGNATURE: std::sync::Mutex<
    Option<(usize, bool, crate::theme::VibrancyMaterial, u32, u32)>,
> = std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
pub fn configure_window_vibrancy_material_for_appearance(
    is_dark: bool,
    material: crate::theme::VibrancyMaterial,
) {
    if require_main_thread("configure_window_vibrancy_material_for_appearance") {
        return;
    }
    // SAFETY: Main thread verified. Window from WindowManager is valid.
    // NSAppearance set to nil on window, set on views instead.
    // NSVisualEffectView methods are standard AppKit APIs.
    unsafe {
        let window = match window_manager::get_main_window() {
            Some(w) => w,
            None => {
                logging::log(
                    "PANEL",
                    "WARNING: Main window not registered, cannot configure vibrancy material",
                );
                return;
            }
        };

        // Get the content view
        let content_view: id = msg_send![window, contentView];
        if content_view.is_null() {
            logging::log("PANEL", "WARNING: Window has no content view");
            return;
        }

        let theme = crate::theme::get_cached_theme();
        let background_tint = crate::ui_foundation::main_window_matched_background_rgba(&theme);
        // Fold the glass sliders in so theme-designer tweaks re-run this pass.
        let opacity_settings = theme.get_opacity();
        let glass_bits = ((opacity_settings.glass_veil_opacity.unwrap_or(-1.0) * 1000.0) as i32
            as u32)
            .wrapping_shl(16)
            ^ ((opacity_settings.glass_tint_opacity.unwrap_or(-1.0) * 1000.0) as i32 as u32);
        let signature = (
            window as usize,
            is_dark,
            material,
            background_tint,
            glass_bits,
        );
        {
            let mut guard = LAST_MAIN_WINDOW_VIBRANCY_SIGNATURE
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if guard.as_ref() == Some(&signature) {
                return;
            }
            *guard = Some(signature);
        }

        // Clear window appearance so GPUI can detect system appearance changes.
        // The appearance is set on individual NSVisualEffectViews instead (see
        // configure_visual_effect_views_recursive).
        let _: () = msg_send![window, setAppearance: nil];
        logging::log(
            "PANEL",
            &format!(
                "Cleared window appearance (nil) for {} mode; appearance set on views instead",
                if is_dark { "dark" } else { "light" }
            ),
        );

        // ╔════════════════════════════════════════════════════════════════════════════╗
        // ║ WINDOW BACKGROUND COLOR - DO NOT CHANGE WITHOUT TESTING                   ║
        // ╠════════════════════════════════════════════════════════════════════════════╣
        // ║ windowBackgroundColor provides the native ~1px border around the window.  ║
        // ║ Using clearColor removes the border but allows more blur.                 ║
        // ║ This setting was tested against Raycast/Spotlight appearance.             ║
        // ║ Glass mode: near-clear instead — the semi-opaque base renders UNDER the   ║
        // ║ NSGlassEffectView backdrop and dims the whole material.                   ║
        // ╚════════════════════════════════════════════════════════════════════════════╝
        let glass_mode = tahoe_liquid_glass_available() && theme.is_vibrancy_enabled();
        let window_bg_color: id = if glass_mode {
            // Matches the gpui fork's Transparent path (0.0001 alpha keeps
            // the window shadow machinery alive, unlike clearColor).
            msg_send![
                class!(NSColor),
                colorWithSRGBRed: 0.0f64 green: 0.0f64 blue: 0.0f64 alpha: 0.0001f64
            ]
        } else {
            msg_send![class!(NSColor), windowBackgroundColor]
        };
        let _: () = msg_send![window, setBackgroundColor: window_bg_color];

        // Enable shadow for native depth perception (Raycast/Spotlight have shadows)
        let _: () = msg_send![window, setHasShadow: true];

        // Mark window as non-opaque to allow transparency/vibrancy
        let _: () = msg_send![window, setOpaque: false];

        logging::log(
            "PANEL",
            "Set window backgroundColor to windowBackgroundColor, hasShadow=true, opaque=false",
        );

        // Recursively find and configure ALL NSVisualEffectViews
        // Expert feedback: GPUI may nest effect views, so we need to walk the whole tree
        let mut count = 0;
        configure_visual_effect_views_recursive(content_view, &mut count, is_dark, material);
        configure_tahoe_window_backdrop(window, "PANEL", "Main window");
        crate::footer_popup::sync_main_window_glass_scroll_bands(window);

        let material_name = vibrancy_material_name(material);
        if count == 0 {
            logging::log(
                "PANEL",
                "WARNING: No NSVisualEffectView found in window hierarchy",
            );
        } else {
            logging::log(
                "PANEL",
                &format!(
                    "Configured {} NSVisualEffectView(s): {} + {} + emphasized",
                    count,
                    if is_dark {
                        "VibrantDark"
                    } else {
                        "VibrantLight"
                    },
                    material_name
                ),
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_window_vibrancy_material_for_appearance(
    _is_dark: bool,
    _material: crate::theme::VibrancyMaterial,
) {
    // No-op on non-macOS platforms
}

#[cfg(target_os = "macos")]
fn vibrancy_material_value(material: crate::theme::VibrancyMaterial) -> isize {
    match material {
        crate::theme::VibrancyMaterial::Hud => ns_visual_effect_material::HUD_WINDOW,
        crate::theme::VibrancyMaterial::Popover => ns_visual_effect_material::POPOVER,
        crate::theme::VibrancyMaterial::Menu => ns_visual_effect_material::MENU,
        crate::theme::VibrancyMaterial::Sidebar => ns_visual_effect_material::SIDEBAR,
        crate::theme::VibrancyMaterial::Content => ns_visual_effect_material::CONTENT_BACKGROUND,
    }
}

#[cfg(target_os = "macos")]
fn vibrancy_material_name(material: crate::theme::VibrancyMaterial) -> &'static str {
    match material {
        crate::theme::VibrancyMaterial::Hud => "HUD_WINDOW",
        crate::theme::VibrancyMaterial::Popover => "POPOVER",
        crate::theme::VibrancyMaterial::Menu => "MENU",
        crate::theme::VibrancyMaterial::Sidebar => "SIDEBAR",
        crate::theme::VibrancyMaterial::Content => "CONTENT_BACKGROUND",
    }
}

/// Recursively walk view hierarchy and configure all NSVisualEffectViews.
///
/// # Arguments
/// * `view` - The view to configure
/// * `count` - Counter for configured views
/// * `is_dark` - Whether to use dark appearance (VibrantDark) or light appearance (VibrantLight)
/// * `material` - The configured vibrancy material to apply
///
/// # Safety
///
/// `view` must be a valid NSView pointer. Caller must be on the main thread.
/// Subview array and its elements are accessed via count-bounded iteration.
/// UTF8String pointers are nil-checked before CStr::from_ptr.
#[cfg(target_os = "macos")]
unsafe fn configure_visual_effect_views_recursive(
    view: id,
    count: &mut usize,
    is_dark: bool,
    material: crate::theme::VibrancyMaterial,
) {
    // Check if this view is an NSVisualEffectView
    let is_vev: bool = msg_send![view, isKindOfClass: class!(NSVisualEffectView)];
    if is_vev {
        // Log current state BEFORE configuration
        let old_material: isize = msg_send![view, material];
        let old_state: isize = msg_send![view, state];
        let old_blending: isize = msg_send![view, blendingMode];
        let old_emphasized: bool = msg_send![view, isEmphasized];

        // Set appearance on the NSVisualEffectView (NOT on the window) so that
        // GPUI can still detect system appearance changes via the window.
        let view_appearance: id = if is_dark {
            msg_send![
                class!(NSAppearance),
                appearanceNamed: NSAppearanceNameVibrantDark
            ]
        } else {
            msg_send![
                class!(NSAppearance),
                appearanceNamed: NSAppearanceNameVibrantLight
            ]
        };
        if !view_appearance.is_null() {
            let _: () = msg_send![view, setAppearance: view_appearance];
        }

        // ╔════════════════════════════════════════════════════════════════════════════╗
        // ║ NSVISUALEFFECTVIEW SETTINGS - DO NOT CHANGE WITHOUT TESTING               ║
        // ╠════════════════════════════════════════════════════════════════════════════╣
        // ║ These settings use 'active' state to prevent dimming when child windows   ║
        // ║ (like Actions popup) take focus. Combined with tint alpha in              ║
        // ║ gpui_integration.rs. See: /Users/johnlindquist/dev/mac-panel-window/      ║
        // ╚════════════════════════════════════════════════════════════════════════════╝
        let material_value = vibrancy_material_value(material);
        let _: () = msg_send![view, setMaterial: material_value];
        // State: 1=active for dark (prevents dimming), 0=followsWindow for light (cleaner look)
        // NSVisualEffectState: 0=followsWindowActiveState, 1=active, 2=inactive
        // Dark mode: active state prevents dimming when Actions popup opens
        // Light mode: followsWindow gives cleaner appearance like the POC
        let state = if is_dark { 1isize } else { 0isize };
        let _: () = msg_send![view, setState: state];
        // BehindWindow blending (0) - blur content behind the window
        let _: () = msg_send![view, setBlendingMode: 0isize];
        // Emphasized adds more contrast/tint - use only in dark mode
        // Light mode without emphasis matches POC's cleaner look
        let _: () = msg_send![view, setEmphasized: is_dark];

        // Glass mode: the Tahoe NSGlassEffectView backdrop is the window
        // material. Header/footer NSVisualEffectViews would composite their
        // own blur bands on top of the glass, so hide them entirely (and
        // restore them whenever glass mode is unavailable or vibrancy is off).
        // SCRIPT_KIT_DEBUG_HIDE_VEV keeps forcing the hide for measurement.
        let glass_mode = tahoe_liquid_glass_available()
            && crate::theme::get_cached_theme().is_vibrancy_enabled();
        let hide_vev = glass_mode || std::env::var("SCRIPT_KIT_DEBUG_HIDE_VEV").is_ok();
        let _: () = msg_send![view, setHidden: hide_vev];
        if hide_vev {
            logging::log(
                "VIBRANCY",
                "NSVisualEffectView hidden (glass mode or SCRIPT_KIT_DEBUG_HIDE_VEV)",
            );
        }

        // NOTE: the backdrop saturation boost is NOT applied here — at
        // configure time the CABackdropLayer does not exist yet (measured:
        // the search always comes up empty). It is applied in the BlurredView
        // updateLayer swizzle (vibrancy_swizzle_materials.rs), where the
        // material has just (re)installed its filter chain.

        // Log state AFTER configuration
        let new_material: isize = msg_send![view, material];
        let new_state: isize = msg_send![view, state];
        let new_blending: isize = msg_send![view, blendingMode];
        let new_emphasized: bool = msg_send![view, isEmphasized];

        // Get effective appearance
        let effective_appearance: id = msg_send![view, effectiveAppearance];
        let appearance_name: id = if !effective_appearance.is_null() {
            msg_send![effective_appearance, name]
        } else {
            nil
        };
        let appearance_str = if !appearance_name.is_null() {
            let s: *const std::os::raw::c_char = msg_send![appearance_name, UTF8String];
            if !s.is_null() {
                std::ffi::CStr::from_ptr(s).to_string_lossy().to_string()
            } else {
                "nil".to_string()
            }
        } else {
            "nil".to_string()
        };

        let material_name = vibrancy_material_name(material);
        logging::log(
            "VIBRANCY",
            &format!(
                "NSVisualEffectView config: mat {} -> {} ({}), state {} -> {}, blend {} -> {}, emph {} -> {}, mode={}, appearance={}",
                old_material, new_material, material_name,
                old_state, new_state,
                old_blending, new_blending,
                old_emphasized, new_emphasized,
                if is_dark { "dark" } else { "light" },
                appearance_str
            ),
        );

        *count += 1;
    }

    // Recurse into subviews
    let subviews: id = msg_send![view, subviews];
    if !subviews.is_null() {
        let subview_count: usize = msg_send![subviews, count];
        for i in 0..subview_count {
            let child: id = msg_send![subviews, objectAtIndex: i];
            configure_visual_effect_views_recursive(child, count, is_dark, material);
        }
    }
}

/// Saturation multiplier for the blurred backdrop.
///
/// `SCRIPT_KIT_VIBRANCY_SATURATION` (e.g. "1.8") overrides for live tuning;
/// otherwise the theme's `vibrancy.backdrop_saturation` applies. Values are
/// clamped to a sane 0.0..=4.0 range.
#[cfg(target_os = "macos")]
fn backdrop_saturation_amount() -> f64 {
    let env_override = std::env::var("SCRIPT_KIT_VIBRANCY_SATURATION")
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|amount| amount.is_finite());
    let amount = env_override.unwrap_or_else(|| {
        f64::from(
            crate::theme::get_cached_theme()
                .get_vibrancy()
                .backdrop_saturation,
        )
    });
    amount.clamp(0.0, 4.0)
}

/// Find the `CABackdropLayer` inside a layer tree (the layer that holds the
/// captured behind-window content an NSVisualEffectView blurs).
///
/// # Safety
/// `layer` must be a valid CALayer pointer or nil. Main thread only.
#[cfg(target_os = "macos")]
unsafe fn find_backdrop_layer(layer: id, backdrop_class: id, depth: usize) -> id {
    if layer.is_null() || depth > 6 {
        return nil;
    }
    let is_backdrop: bool = msg_send![layer, isKindOfClass: backdrop_class];
    if is_backdrop {
        return layer;
    }
    let sublayers: id = msg_send![layer, sublayers];
    if !sublayers.is_null() {
        let count: usize = msg_send![sublayers, count];
        for i in 0..count {
            let child: id = msg_send![sublayers, objectAtIndex: i];
            let found = find_backdrop_layer(child, backdrop_class, depth + 1);
            if !found.is_null() {
                return found;
            }
        }
    }
    nil
}

/// Boost backdrop saturation on the CABackdropLayer inside an
/// NSVisualEffectView before the material and window tints composite over it.
///
/// The material installs its own filter chain on the backdrop layer
/// (sdrNormalize, gaussianBlur, colorSaturate); when a colorSaturate filter is
/// already present its `inputAmount` is overridden, otherwise a new CAFilter
/// is appended. Either way the filters array is reassigned, because Core
/// Animation ignores in-place filter mutation after attachment. Uses private
/// CoreAnimation API (CAFilter / CABackdropLayer).
///
/// # Safety
/// `view` must be a valid NSView pointer. Main thread only.
#[cfg(target_os = "macos")]
unsafe fn apply_backdrop_saturation_filter(view: id, amount: f64) -> bool {
    #[link(name = "Foundation", kind = "framework")]
    extern "C" {
        fn NSClassFromString(a_class_name: id) -> id;
    }

    let view_class: id = msg_send![view, class];
    let view_class_name: id = msg_send![view_class, className];
    let view_class_str: *const std::os::raw::c_char = msg_send![view_class_name, UTF8String];
    let view_name = if view_class_str.is_null() {
        "<unknown>".to_string()
    } else {
        std::ffi::CStr::from_ptr(view_class_str)
            .to_string_lossy()
            .to_string()
    };

    let fail = |step: &str| {
        logging::log(
            "VIBRANCY",
            &format!("Backdrop saturation: failed at step '{step}' (view={view_name})"),
        );
        false
    };

    let layer: id = msg_send![view, layer];
    if layer.is_null() {
        return fail("view.layer is nil");
    }

    let backdrop_class_name: id = msg_send![
        class!(NSString),
        stringWithUTF8String: c"CABackdropLayer".as_ptr()
    ];
    let backdrop_class = NSClassFromString(backdrop_class_name);
    if backdrop_class.is_null() {
        return fail("CABackdropLayer class unavailable");
    }
    let backdrop = find_backdrop_layer(layer, backdrop_class, 0);
    if backdrop.is_null() {
        // NSGlassEffectView has no in-process CABackdropLayer. Do NOT fall
        // back to filtering the view's root layer: the glass spans the whole
        // window, and a root-layer filter degrades what within-window
        // NSVisualEffectViews (footer trio) composite on top of it.
        return fail("no CABackdropLayer in layer tree");
    }

    let input_amount_key: id =
        msg_send![class!(NSString), stringWithUTF8String: c"inputAmount".as_ptr()];
    let amount_number: id = msg_send![class!(NSNumber), numberWithDouble: amount];
    let saturate_name: id =
        msg_send![class!(NSString), stringWithUTF8String: c"colorSaturate".as_ptr()];

    // Prefer overriding the material's existing colorSaturate filter.
    let existing_filters: id = msg_send![backdrop, filters];
    let existing_count: usize = if existing_filters.is_null() {
        0
    } else {
        msg_send![existing_filters, count]
    };
    let mutable: id = if existing_filters.is_null() {
        msg_send![class!(NSMutableArray), array]
    } else {
        msg_send![existing_filters, mutableCopy]
    };
    let mut overrode_existing = false;
    for i in 0..existing_count {
        let filter: id = msg_send![mutable, objectAtIndex: i];
        let name: id = msg_send![filter, name];
        if !name.is_null() {
            let matches: bool = msg_send![name, isEqualToString: saturate_name];
            if matches {
                let prior: id = msg_send![filter, valueForKey: input_amount_key];
                let prior_amount: f64 = if prior.is_null() {
                    f64::NAN
                } else {
                    msg_send![prior, doubleValue]
                };
                let _: () = msg_send![filter, setValue: amount_number forKey: input_amount_key];
                if !overrode_existing {
                    logging::log(
                        "VIBRANCY",
                        &format!(
                            "Backdrop saturation: material default inputAmount={prior_amount}, overriding to {amount}"
                        ),
                    );
                }
                overrode_existing = true;
            }
        }
    }

    if !overrode_existing {
        let filter_class_name: id =
            msg_send![class!(NSString), stringWithUTF8String: c"CAFilter".as_ptr()];
        let filter_class = NSClassFromString(filter_class_name);
        if filter_class.is_null() {
            return fail("CAFilter class unavailable");
        }
        let filter: id = msg_send![filter_class, filterWithType: saturate_name];
        if filter.is_null() {
            return fail("CAFilter filterWithType returned nil");
        }
        let _: () = msg_send![filter, setValue: amount_number forKey: input_amount_key];
        let _: () = msg_send![mutable, addObject: filter];
    }

    // Reassign to commit the change.
    let _: () = msg_send![backdrop, setFilters: mutable];
    logging::log(
        "VIBRANCY",
        &format!(
            "Backdrop saturation: {} colorSaturate inputAmount={amount}",
            if overrode_existing {
                "overrode existing"
            } else {
                "appended new"
            }
        ),
    );
    true
}
