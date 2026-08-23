#[cfg(target_os = "macos")]
unsafe fn configure_window_vibrancy_common_impl(
    window: id,
    log_target: &str,
    window_name: &str,
    is_dark: bool,
    morph_variant: GlassMorphVariant,
) -> NativeGlassEntryReceipt {
    // Clear window appearance so GPUI can detect system appearance changes.
    // Appearance is set on individual NSVisualEffectViews instead.
    let _: () = msg_send![window, setAppearance: nil];
    logging::log(
        log_target,
        &format!(
            "{}: Cleared window appearance (nil) for {} mode; appearance set on views",
            window_name,
            if is_dark { "dark" } else { "light" }
        ),
    );

    // Use windowBackgroundColor for semi-opaque background — except in glass
    // mode, where that base renders UNDER the NSGlassEffectView backdrop and
    // dims the whole material; use the near-clear base instead (0.0001 alpha
    // keeps the window shadow machinery alive, unlike clearColor).
    let glass_mode = tahoe_native_glass_composition_available()
        && crate::theme::get_cached_theme().is_vibrancy_enabled();
    let window_bg_color: id = if glass_mode {
        msg_send![
            class!(NSColor),
            colorWithSRGBRed: 0.0f64 green: 0.0f64 blue: 0.0f64 alpha: 0.0001f64
        ]
    } else {
        msg_send![class!(NSColor), windowBackgroundColor]
    };
    let _: () = msg_send![window, setBackgroundColor: window_bg_color];
    logging::log(
        log_target,
        &format!(
            "{}: Set backgroundColor ({} base)",
            window_name,
            if glass_mode {
                "glass near-clear"
            } else {
                "windowBackgroundColor semi-opaque"
            }
        ),
    );

    // Mark window as non-opaque to allow transparency/vibrancy.
    let _: () = msg_send![window, setOpaque: false];

    // Enable shadow for native depth perception.
    let _: () = msg_send![window, setHasShadow: true];

    // Configure NSVisualEffectViews in the window hierarchy.
    let content_view: id = msg_send![window, contentView];
    if !content_view.is_null() {
        let mut count = 0;
        let material = current_window_material();
        configure_visual_effect_views_recursive(content_view, &mut count, is_dark, material);
        let material_name = current_window_material_name(material);
        logging::log(
            log_target,
            &format!(
                "{}: Configured {} NSVisualEffectView(s) with {} material",
                window_name, count, material_name
            ),
        );
    }

    let backdrop = configure_tahoe_window_backdrop_with_result(window, log_target, window_name);
    let glass_created = backdrop.is_some_and(|result| result.created);
    let morph_tuning = glass_morph_tuning();
    // Secondary/overlay windows (notes, dictation, confirm, actions, AI,
    // flow manager, inline popups) are created per appearance, so a freshly
    // created backdrop means the window just appeared: morph it in.
    // Child-attached panels transform the content layer because animating a
    // child NSWindow frame fights AppKit's parent-child machinery and lags.
    if glass_created {
        match morph_variant {
            GlassMorphVariant::WindowFrame => {
                animate_tahoe_glass_window_frame_appearance(window, log_target, window_name)
            }
            GlassMorphVariant::ContentLayer => {
                // Runtime-proven (real-pixel capture + static-transform
                // experiment): CALayer transforms on the contentView's
                // NSViewBackingLayer are neutralized by AppKit — even a
                // static 0.85 model scale renders at full size. No
                // layer-transform morph can ever work on AppKit-managed
                // backing layers. Instead: detach from the parent window for
                // the morph's duration and run the SAME NSWindow frame morph
                // the main window uses, then reattach — the frame animation
                // only fights the parent-child machinery while attached.
                animate_tahoe_glass_child_appearance(window, log_target, window_name);
            }
            GlassMorphVariant::FadeOnly => {
                animate_tahoe_glass_fade_appearance(window, log_target, window_name)
            }
        }
    }

    let appearance_name = if is_dark {
        "VibrantDark"
    } else {
        "VibrantLight"
    };
    let material_name = current_window_material_name(current_window_material());
    logging::log(
        log_target,
        &format!(
            "{} vibrancy configured ({} + {} + blur)",
            window_name, appearance_name, material_name
        ),
    );
    let window_number: i64 = msg_send![window, windowNumber];
    let style_signature = backdrop
        .map(|result| result.style_signature)
        .unwrap_or_else(|| {
            resolve_native_glass_style(
                &crate::theme::get_cached_theme(),
                NativeGlassSurfaceRole::WindowBackdrop,
            )
            .signature
        });
    let configured_at_monotonic_ns = crate::platform::host_clock::host_time_ns();
    let configured_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let backdrop_found_or_created = backdrop.is_some_and(|result| result.found_or_created);
    let native_selectors_supported =
        backdrop.is_some_and(|result| result.native_selectors_supported);
    let style_applied = backdrop.is_some_and(|result| result.style_applied);
    NativeGlassEntryReceipt {
        window_number,
        configured: window_number > 0
            && content_view != nil
            && backdrop_found_or_created
            && native_selectors_supported
            && style_applied,
        backdrop_found_or_created,
        native_selectors_supported,
        style_applied,
        style_signature,
        morph_started: glass_created && morph_tuning.is_some(),
        morph_start_alpha_bits: if glass_created {
            morph_tuning.map(|tuning| tuning.start_alpha.to_bits())
        } else {
            None
        },
        settle_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.total_entry_duration() * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        material_onset_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| tuning.visible_tail_start_delay_ms())
                .unwrap_or(0)
        } else {
            0
        },
        visible_tail_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.visible_tail_duration() * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        content_hold_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.content_hold_duration * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        content_fade_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.content_fade_duration * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        settled_crossing_delay_ms: if glass_created {
            morph_tuning
                .map(settled_size_crossing_delay_ms)
                .unwrap_or(0)
        } else {
            0
        },
        content_reveal_delay_ms: if glass_created {
            morph_tuning.map(entry_content_reveal_delay_ms).unwrap_or(0)
        } else {
            0
        },
        phase_one_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.phase1 * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        phase_two_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.phase2 * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        alpha_ramp_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.alpha_ramp_duration * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        alpha_finish_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.alpha_finish_duration * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        configured_at_monotonic_ns,
        configured_at_unix_ms,
    }
}
