#[cfg(target_os = "macos")]
unsafe fn find_subview_by_identifier(parent: id, identifier: &str) -> id {
    use objc::{msg_send, sel, sel_impl};

    let ns_identifier = ns_string(identifier);
    if parent == nil || ns_identifier == nil {
        return nil;
    }

    let subviews: id = msg_send![parent, subviews];
    if subviews == nil {
        return nil;
    }

    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let view: id = msg_send![subviews, objectAtIndex: index];
        if view == nil {
            continue;
        }
        let view_identifier: id = msg_send![view, identifier];
        if view_identifier != nil {
            let matches: cocoa::base::BOOL =
                msg_send![view_identifier, isEqualToString: ns_identifier];
            if matches == YES {
                return view;
            }
        }

        // Glass foregrounds live below NSGlassEffectView.contentView. Search
        // the actual hierarchy instead of assuming every identified node is a
        // direct child of the footer host.
        let nested = find_subview_by_identifier(view, identifier);
        if nested != nil {
            return nested;
        }
    }

    nil
}

#[cfg(target_os = "macos")]
unsafe fn find_subview_by_accessibility_identifier(parent: id, identifier: &str) -> id {
    use objc::{msg_send, sel, sel_impl};

    let ns_identifier = ns_string(identifier);
    if parent == nil || ns_identifier == nil {
        return nil;
    }
    let subviews: id = msg_send![parent, subviews];
    if subviews == nil {
        return nil;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let view: id = msg_send![subviews, objectAtIndex: index];
        if view == nil {
            continue;
        }
        let responds: cocoa::base::BOOL =
            msg_send![view, respondsToSelector: sel!(accessibilityIdentifier)];
        if responds == YES {
            let view_identifier: id = msg_send![view, accessibilityIdentifier];
            if view_identifier != nil {
                let matches: cocoa::base::BOOL =
                    msg_send![view_identifier, isEqualToString: ns_identifier];
                if matches == YES {
                    return view;
                }
            }
        }
        let nested = find_subview_by_accessibility_identifier(view, identifier);
        if nested != nil {
            return nested;
        }
    }
    nil
}

#[cfg(target_os = "macos")]
fn footer_hint_side_inset(glass_active: bool) -> f64 {
    f64::from(crate::components::footer_chrome::footer_rail_side_inset_px(
        glass_active,
        FOOTER_HINT_SIDE_INSET as f32,
    ))
}

#[cfg(target_os = "macos")]
fn footer_hints_frame(width: f64) -> cocoa::foundation::NSRect {
    let side_inset = footer_hint_side_inset(glass_scroll_bands_active());
    cocoa::foundation::NSRect::new(
        cocoa::foundation::NSPoint::new(side_inset, 0.0),
        cocoa::foundation::NSSize::new((width - (side_inset * 2.0)).max(0.0), footer_height()),
    )
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct NativeFooterLaneLayout {
    hints_width: f64,
    left_pinned_end_x: f64,
    trailing_start_x: f64,
    left_info_x: f64,
    left_info_width: f64,
    trailing_overflow: bool,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FooterLeftInfoDegradation {
    Full,
    TruncatedLabels,
    CwdAffordanceOnly,
    PrimaryOnly,
    PrimaryAffordanceOnly,
    Hidden,
}

#[cfg(target_os = "macos")]
impl FooterLeftInfoDegradation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::TruncatedLabels => "truncatedLabels",
            Self::CwdAffordanceOnly => "cwdAffordanceOnly",
            Self::PrimaryOnly => "primaryOnly",
            Self::PrimaryAffordanceOnly => "primaryAffordanceOnly",
            Self::Hidden => "hidden",
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct FooterLeftInfoAllocation {
    degradation: FooterLeftInfoDegradation,
    available_width: f64,
    cwd_label_width: f64,
    primary_label_width: f64,
}

#[cfg(target_os = "macos")]
static LAST_FOOTER_LEFT_ALLOCATION: OnceLock<
    Mutex<Option<crate::protocol::AppKitFooterLeftAllocation>>,
> = OnceLock::new();

#[cfg(target_os = "macos")]
fn record_footer_left_allocation(allocation: FooterLeftInfoAllocation) {
    let snapshot = crate::protocol::AppKitFooterLeftAllocation {
        degradation: allocation.degradation.as_str().to_string(),
        available_width: allocation.available_width,
        cwd_label_width: allocation.cwd_label_width,
        primary_label_width: allocation.primary_label_width,
    };
    if let Ok(mut slot) = LAST_FOOTER_LEFT_ALLOCATION
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *slot = Some(snapshot);
    }
}

#[cfg(target_os = "macos")]
fn footer_left_allocation_snapshot() -> Option<crate::protocol::AppKitFooterLeftAllocation> {
    LAST_FOOTER_LEFT_ALLOCATION
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FooterLeftInfoMeasurements {
    cwd_fixed_width: f64,
    cwd_label_width: f64,
    primary_fixed_width: f64,
    primary_label_width: f64,
    has_cwd: bool,
    primary_visible_without_label: bool,
}

#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_CAPSULE_PAD_X: f64 = 8.0;

#[cfg(target_os = "macos")]
fn resolve_native_footer_lanes(
    hints_width: f64,
    left_pinned_end_x: f64,
    trailing_start_x: f64,
) -> NativeFooterLaneLayout {
    resolve_native_footer_lanes_with_mode(
        hints_width,
        left_pinned_end_x,
        trailing_start_x,
        glass_scroll_bands_active(),
    )
}

#[cfg(target_os = "macos")]
fn resolve_native_footer_lanes_with_mode(
    hints_width: f64,
    left_pinned_end_x: f64,
    trailing_start_x: f64,
    edge_flush: bool,
) -> NativeFooterLaneLayout {
    let gap = f64::from(crate::components::footer_chrome::FOOTER_LEFT_RIGHT_MIN_GAP_PX);
    let left_pinned_end_x = left_pinned_end_x.clamp(0.0, hints_width.max(0.0));
    let trailing_start_x = trailing_start_x.clamp(0.0, hints_width.max(0.0));
    // The inter-lane gap separates the left-info capsule from a PRECEDING
    // left-pinned chip. When no left-pinned lane exists in the edge-flush
    // floating footer, the gap would render as a bare sliver between the
    // window edge and the first capsule (user report 2026-08-12), so it only
    // applies when there is something to separate from. The strip origin is
    // already edge-flush via `footer_hint_side_inset`; the capsule's visual
    // leading edge sits at `left_info_x - FOOTER_LEFT_INFO_CAPSULE_PAD_X`.
    let lead_gap = if edge_flush && left_pinned_end_x <= 0.0 {
        0.0
    } else {
        gap
    };
    let left_info_x = left_pinned_end_x + lead_gap + FOOTER_LEFT_INFO_CAPSULE_PAD_X;
    let left_info_end_x = trailing_start_x - gap - FOOTER_LEFT_INFO_CAPSULE_PAD_X;
    let trailing_overflow = trailing_start_x < left_pinned_end_x + gap;
    NativeFooterLaneLayout {
        hints_width,
        left_pinned_end_x,
        trailing_start_x,
        left_info_x,
        left_info_width: if trailing_overflow {
            0.0
        } else {
            (left_info_end_x - left_info_x).max(0.0)
        },
        trailing_overflow,
    }
}

#[cfg(target_os = "macos")]
fn resolve_footer_left_info_allocation(
    available_width: f64,
    measured: FooterLeftInfoMeasurements,
) -> FooterLeftInfoAllocation {
    let available_width = available_width.max(0.0);
    let cwd_min = measured.cwd_label_width.min(f64::from(
        crate::components::footer_chrome::FOOTER_CWD_LABEL_MIN_WIDTH_PX,
    ));
    let primary_min = measured.primary_label_width.min(f64::from(
        crate::components::footer_chrome::FOOTER_PRIMARY_LABEL_MIN_WIDTH_PX,
    ));
    let fixed = measured.cwd_fixed_width + measured.primary_fixed_width;
    let full_required = fixed + measured.cwd_label_width + measured.primary_label_width;
    let truncated_required = fixed + cwd_min + primary_min;
    let cwd_affordance_required = fixed + primary_min;
    let primary_only_required = measured.primary_fixed_width + primary_min;

    let (degradation, cwd_label_width, primary_label_width) = if full_required <= 0.0 {
        (FooterLeftInfoDegradation::Hidden, 0.0, 0.0)
    } else if available_width >= full_required {
        (
            FooterLeftInfoDegradation::Full,
            measured.cwd_label_width,
            measured.primary_label_width,
        )
    } else if measured.has_cwd && available_width >= truncated_required {
        let flexible = (available_width - fixed - cwd_min - primary_min).max(0.0);
        let cwd_extra = (measured.cwd_label_width - cwd_min).max(0.0);
        let primary_extra = (measured.primary_label_width - primary_min).max(0.0);
        let total_extra = cwd_extra + primary_extra;
        let cwd_share = if total_extra > 0.0 {
            flexible * cwd_extra / total_extra
        } else {
            0.0
        };
        let cwd_width = (cwd_min + cwd_share).min(measured.cwd_label_width);
        (
            FooterLeftInfoDegradation::TruncatedLabels,
            cwd_width,
            (available_width - fixed - cwd_width)
                .max(primary_min)
                .min(measured.primary_label_width),
        )
    } else if measured.has_cwd && available_width >= cwd_affordance_required {
        (
            FooterLeftInfoDegradation::CwdAffordanceOnly,
            0.0,
            (available_width - fixed)
                .max(0.0)
                .min(measured.primary_label_width),
        )
    } else if available_width >= primary_only_required {
        (
            FooterLeftInfoDegradation::PrimaryOnly,
            0.0,
            (available_width - measured.primary_fixed_width)
                .max(0.0)
                .min(measured.primary_label_width),
        )
    } else if measured.primary_visible_without_label
        && available_width >= measured.primary_fixed_width
    {
        (FooterLeftInfoDegradation::PrimaryAffordanceOnly, 0.0, 0.0)
    } else {
        (FooterLeftInfoDegradation::Hidden, 0.0, 0.0)
    };
    FooterLeftInfoAllocation {
        degradation,
        available_width,
        cwd_label_width,
        primary_label_width,
    }
}

#[cfg(target_os = "macos")]
fn footer_left_info_frame(layout: NativeFooterLaneLayout) -> cocoa::foundation::NSRect {
    cocoa::foundation::NSRect::new(
        cocoa::foundation::NSPoint::new(
            footer_hint_side_inset(glass_scroll_bands_active()) + layout.left_info_x,
            0.0,
        ),
        cocoa::foundation::NSSize::new(layout.left_info_width, footer_height()),
    )
}

/// Return the owning foreground view for left-info visuals. Glass foregrounds
/// must be mounted through `NSGlassEffectView.contentView`; siblings are
/// treated as refracted background and become washed out. The returned
/// offsets preserve the left-info coordinate system while the capsule extends
/// beyond it by its shared horizontal padding.
#[cfg(target_os = "macos")]
unsafe fn ensure_footer_left_info_visual_parent(left_info_view: id, height: f64) -> (id, f64, f64) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    if !glass_scroll_bands_active() {
        return (left_info_view, 0.0, 0.0);
    }
    let Some(glass_class) = objc::runtime::Class::get("NSGlassEffectView") else {
        return (left_info_view, 0.0, 0.0);
    };

    const PAD_X: f64 = FOOTER_LEFT_INFO_CAPSULE_PAD_X;
    let item_height =
        crate::components::footer_chrome::footer_button_height(footer_height() as f32) as f64;
    let capsule_y = ((height - item_height) / 2.0).round();
    let provisional_frame = NSRect::new(
        NSPoint::new(-PAD_X, capsule_y),
        NSSize::new(PAD_X * 2.0 + 1.0, item_height),
    );
    let existing = find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_CAPSULE_ID);
    let capsule = if existing != nil {
        existing
    } else {
        let capsule: id = msg_send![glass_class, alloc];
        let capsule: id = msg_send![capsule, initWithFrame: provisional_frame];
        if capsule == nil {
            return (left_info_view, 0.0, 0.0);
        }
        let identifier = ns_string(FOOTER_LEFT_INFO_CAPSULE_ID);
        if identifier != nil {
            let _: () = msg_send![capsule, setIdentifier: identifier];
        }
        let _: () = msg_send![
            capsule,
            setCornerRadius:
                crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
        ];
        style_float_footer_capsule(capsule, &crate::theme::get_cached_theme());
        let _: () = msg_send![
            left_info_view,
            addSubview: capsule
            positioned: -1isize
            relativeTo: cocoa::base::nil
        ];
        capsule
    };

    let existing_content = find_subview_by_identifier(capsule, FOOTER_LEFT_INFO_CAPSULE_CONTENT_ID);
    let content = if existing_content != nil {
        existing_content
    } else {
        let content: id = msg_send![class!(NSView), alloc];
        let content: id = msg_send![
            content,
            initWithFrame: NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(PAD_X * 2.0 + 1.0, item_height)
            )
        ];
        if content == nil {
            return (left_info_view, 0.0, 0.0);
        }
        let identifier = ns_string(FOOTER_LEFT_INFO_CAPSULE_CONTENT_ID);
        if identifier != nil {
            let _: () = msg_send![content, setIdentifier: identifier];
        }
        let _: () = msg_send![content, setAutoresizingMask: 18u64];
        let _: () = msg_send![capsule, setContentView: content];
        content
    };
    let _: () = msg_send![content, setWantsLayer: YES];
    let content_layer: id = msg_send![content, layer];
    if content_layer != nil {
        let _: () = msg_send![
            content_layer,
            setCornerRadius:
                crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
        ];
        let _: () = msg_send![content_layer, setMasksToBounds: YES];
    }
    let state_view = find_subview_by_identifier(content, FOOTER_LEFT_INFO_STATE_LAYER_ID);
    let state_view = if state_view != nil {
        state_view
    } else {
        let state_view: id = msg_send![class!(NSView), alloc];
        let state_view: id = msg_send![
            state_view,
            initWithFrame: NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(PAD_X * 2.0 + 1.0, item_height)
            )
        ];
        if state_view != nil {
            let identifier = ns_string(FOOTER_LEFT_INFO_STATE_LAYER_ID);
            if identifier != nil {
                let _: () = msg_send![state_view, setIdentifier: identifier];
            }
            let _: () = msg_send![state_view, setAutoresizingMask: 18u64];
            let _: () = msg_send![state_view, setWantsLayer: YES];
            let state_layer: id = msg_send![state_view, layer];
            if state_layer != nil {
                let _: () = msg_send![
                    state_layer,
                    setCornerRadius:
                        crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
                ];
                let _: () = msg_send![state_layer, setMasksToBounds: YES];
            }
            let _: () = msg_send![
                content,
                addSubview: state_view
                positioned: -1isize
                relativeTo: cocoa::base::nil
            ];
        }
        state_view
    };
    if state_view != nil {
        let content_bounds: NSRect = msg_send![content, bounds];
        let _: () = msg_send![state_view, setFrame: content_bounds];
    }
    style_float_footer_capsule(capsule, &crate::theme::get_cached_theme());
    let _: () = msg_send![capsule, setHidden: NO];
    (content, PAD_X, -capsule_y)
}

/// Floating-chrome mode: size the left-info capsule to its laid-out content.
/// Hidden when there is no content or float mode is off.
#[cfg(target_os = "macos")]
unsafe fn ensure_footer_left_info_capsule(left_info_view: id, content_width: f64, height: f64) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    // Safe in-window: the footer container is bounded to the 32pt footer band
    // and separated from the main backdrop by the transparent gutter.
    const PAD_X: f64 = 8.0;
    let existing = find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_CAPSULE_ID);
    let active = glass_scroll_bands_active() && content_width > 1.0;
    if !active {
        if existing != nil {
            let _: () = msg_send![existing, setHidden: YES];
        }
        return;
    }
    let item_height =
        crate::components::footer_chrome::footer_button_height(footer_height() as f32) as f64;
    let frame = NSRect::new(
        NSPoint::new(-PAD_X, ((height - item_height) / 2.0).round()),
        NSSize::new(content_width + PAD_X * 2.0, item_height),
    );
    let capsule = if existing != nil {
        existing
    } else {
        let Some(glass_class) = objc::runtime::Class::get("NSGlassEffectView") else {
            return;
        };
        let capsule: id = msg_send![glass_class, alloc];
        let capsule: id = msg_send![capsule, initWithFrame: frame];
        if capsule == nil {
            return;
        }
        let identifier = ns_string(FOOTER_LEFT_INFO_CAPSULE_ID);
        if identifier != nil {
            let _: () = msg_send![capsule, setIdentifier: identifier];
        }
        let _: () = msg_send![
            capsule,
            setCornerRadius:
                crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
        ];
        style_float_footer_capsule(capsule, &crate::theme::get_cached_theme());
        let _: () = msg_send![
            left_info_view,
            addSubview: capsule
            positioned: -1isize
            relativeTo: cocoa::base::nil
        ];
        capsule
    };
    let _: () = msg_send![capsule, setHidden: NO];
    let _: () = msg_send![capsule, setFrame: frame];
    // Resizing/attaching an NSGlassEffectView may replace its private
    // foreground backing. Reapply the shared policy after the final frame so
    // the left capsule keeps the same veil and rim as trailing capsules.
    style_float_footer_capsule(capsule, &crate::theme::get_cached_theme());
}

#[cfg(target_os = "macos")]
unsafe fn remove_identified_subview(parent: id, identifier: &str) {
    use objc::{msg_send, sel, sel_impl};

    // Remove every match. Older refreshes could leave duplicate nested nodes;
    // one teardown must restore the closed-world identifier inventory.
    loop {
        let view = find_subview_by_identifier(parent, identifier);
        if view == nil {
            break;
        }
        let layer: id = msg_send![view, layer];
        if layer != nil {
            remove_active_dot_pulse_animation(layer);
        }
        let _: () = msg_send![view, removeFromSuperview];
    }
}

#[cfg(target_os = "macos")]
unsafe fn set_footer_view_identifier(view: id, identifier: &str) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }
    let identifier = ns_string(identifier);
    if identifier == nil {
        return;
    }
    let _: () = msg_send![view, setIdentifier: identifier];
    let supports_ax_identifier: cocoa::base::BOOL =
        msg_send![view, respondsToSelector: sel!(setAccessibilityIdentifier:)];
    if supports_ax_identifier == YES {
        let _: () = msg_send![view, setAccessibilityIdentifier: identifier];
    }
}

#[cfg(target_os = "macos")]
unsafe fn set_footer_button_accessibility(view: id, button: &FooterButtonConfig) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }
    let semantic_id = ns_string(button.id.as_ref());
    let label = ns_string(button.label.as_ref());
    if semantic_id != nil {
        let supports: cocoa::base::BOOL =
            msg_send![view, respondsToSelector: sel!(setAccessibilityIdentifier:)];
        if supports == YES {
            let _: () = msg_send![view, setAccessibilityIdentifier: semantic_id];
        }
    }
    if label != nil {
        let supports: cocoa::base::BOOL =
            msg_send![view, respondsToSelector: sel!(setAccessibilityLabel:)];
        if supports == YES {
            let _: () = msg_send![view, setAccessibilityLabel: label];
        }
    }
    let role = ns_string("AXButton");
    let supports_role: cocoa::base::BOOL =
        msg_send![view, respondsToSelector: sel!(setAccessibilityRole:)];
    if supports_role == YES && role != nil {
        let _: () = msg_send![view, setAccessibilityRole: role];
    }
    if let Some(reason) = button.disabled_reason.as_ref() {
        let help = ns_string(reason.as_ref());
        let supports: cocoa::base::BOOL =
            msg_send![view, respondsToSelector: sel!(setAccessibilityHelp:)];
        if supports == YES && help != nil {
            let _: () = msg_send![view, setAccessibilityHelp: help];
        }
    }
    let supports_element: cocoa::base::BOOL =
        msg_send![view, respondsToSelector: sel!(setAccessibilityElement:)];
    if supports_element == YES {
        let _: () = msg_send![view, setAccessibilityElement: YES];
    }
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_status_dot_view(left_info_view: id, visual_parent: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let existing = find_subview_by_identifier(left_info_view, FOOTER_STATUS_DOT_ID);
    if existing != nil {
        return existing;
    }

    let dot_view: id = msg_send![class!(NSView), alloc];
    let dot_view: id = msg_send![
        dot_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(FOOTER_STREAMING_DOT_SIZE, FOOTER_STREAMING_DOT_SIZE),
        )
    ];
    if dot_view == nil {
        return nil;
    }

    let identifier = ns_string(FOOTER_STATUS_DOT_ID);
    if identifier != nil {
        let _: () = msg_send![dot_view, setIdentifier: identifier];
    }

    let layer: id = msg_send![class!(CALayer), layer];
    if layer != nil {
        let _: () = msg_send![layer, setMasksToBounds: NO];
        let _: () = msg_send![layer, setCornerRadius: FOOTER_STREAMING_DOT_SIZE / 2.0_f64];
        let _: () = msg_send![dot_view, setLayer: layer];
    }
    let _: () = msg_send![dot_view, setWantsLayer: YES];
    let _: () = msg_send![visual_parent, addSubview: dot_view];
    dot_view
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_model_label(
    left_info_view: id,
    visual_parent: id,
    text: &str,
    text_color: id,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let font: id = msg_send![
        class!(NSFont),
        systemFontOfSize: crate::components::footer_chrome::current_main_menu_footer_metrics().label_font_size as f64
        weight: crate::components::footer_chrome::current_main_menu_footer_appkit_font_weight()
    ];
    let label = find_subview_by_identifier(left_info_view, FOOTER_MODEL_LABEL_ID);
    if label != nil {
        let string_value = ns_string(text);
        if string_value != nil {
            let _: () = msg_send![label, setStringValue: string_value];
        }
        if font != nil {
            let _: () = msg_send![label, setFont: font];
        }
        if text_color != nil {
            let _: () = msg_send![label, setTextColor: text_color];
        }
        let _: () = msg_send![label, setAlignment: FOOTER_HINT_TEXT_ALIGN_LEFT];
        let _: () = msg_send![label, sizeToFit];
        return label;
    }

    let label = make_footer_hint_text_field(text, font, text_color, FOOTER_HINT_TEXT_ALIGN_LEFT);
    if label != nil {
        let identifier = ns_string(FOOTER_MODEL_LABEL_ID);
        if identifier != nil {
            let _: () = msg_send![label, setIdentifier: identifier];
        }
        let _: () = msg_send![visual_parent, addSubview: label];
    }
    label
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_left_profile_icon_view(left_info_view: id, visual_parent: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let existing = find_subview_by_identifier(left_info_view, FOOTER_LEFT_PROFILE_ICON_ID);
    if existing != nil {
        return existing;
    }

    let image_view: id = msg_send![class!(NSImageView), alloc];
    let image_view: id = msg_send![
        image_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(FOOTER_LEFT_PROFILE_ICON_SIZE, FOOTER_LEFT_PROFILE_ICON_SIZE),
        )
    ];
    if image_view == nil {
        return nil;
    }
    let identifier = ns_string(FOOTER_LEFT_PROFILE_ICON_ID);
    if identifier != nil {
        let _: () = msg_send![image_view, setIdentifier: identifier];
    }
    let _: () = msg_send![image_view, setWantsLayer: YES];
    let _: () = msg_send![visual_parent, addSubview: image_view];
    image_view
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_cwd_chip_icon_view(left_info_view: id, visual_parent: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let existing = find_subview_by_identifier(left_info_view, FOOTER_CWD_CHIP_ICON_ID);
    if existing != nil {
        return existing;
    }

    let image_view: id = msg_send![class!(NSImageView), alloc];
    let image_view: id = msg_send![
        image_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(FOOTER_LEFT_PROFILE_ICON_SIZE, FOOTER_LEFT_PROFILE_ICON_SIZE),
        )
    ];
    if image_view == nil {
        return nil;
    }
    let identifier = ns_string(FOOTER_CWD_CHIP_ICON_ID);
    if identifier != nil {
        let _: () = msg_send![image_view, setIdentifier: identifier];
    }
    let _: () = msg_send![image_view, setWantsLayer: YES];
    let _: () = msg_send![visual_parent, addSubview: image_view];
    image_view
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_cwd_chip_label(
    left_info_view: id,
    visual_parent: id,
    text: &str,
    text_color: id,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let font: id = msg_send![
        class!(NSFont),
        systemFontOfSize: crate::components::footer_chrome::current_main_menu_footer_metrics().label_font_size as f64
        weight: crate::components::footer_chrome::current_main_menu_footer_appkit_font_weight()
    ];
    let label = find_subview_by_identifier(left_info_view, FOOTER_CWD_CHIP_LABEL_ID);
    if label != nil {
        let string_value = ns_string(text);
        if string_value != nil {
            let _: () = msg_send![label, setStringValue: string_value];
        }
        if font != nil {
            let _: () = msg_send![label, setFont: font];
        }
        if text_color != nil {
            let _: () = msg_send![label, setTextColor: text_color];
        }
        let _: () = msg_send![label, setAlignment: FOOTER_HINT_TEXT_ALIGN_LEFT];
        let _: () = msg_send![label, sizeToFit];
        return label;
    }

    let label = make_footer_hint_text_field(text, font, text_color, FOOTER_HINT_TEXT_ALIGN_LEFT);
    if label != nil {
        let identifier = ns_string(FOOTER_CWD_CHIP_LABEL_ID);
        if identifier != nil {
            let _: () = msg_send![label, setIdentifier: identifier];
        }
        let _: () = msg_send![visual_parent, addSubview: label];
    }
    label
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
unsafe fn layout_footer_left_keycap(
    search_root: id,
    visual_parent: id,
    keycap_id: &str,
    glyph_id: &str,
    glyph: &str,
    x: f64,
    host_height: f64,
    visual_offset_x: f64,
    visual_offset_y: f64,
    text_color: id,
) -> f64 {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
    let keycap_height = metrics.keycap_height as f64;
    let shortcut_tokens = crate::components::footer_chrome::split_footer_shortcut(glyph);
    if shortcut_tokens.is_empty() {
        remove_identified_subview(search_root, keycap_id);
        return 0.0;
    }
    let font: id = msg_send![
        class!(NSFont),
        systemFontOfSize: metrics.keycap_font_size as f64
        weight: crate::components::footer_chrome::current_main_menu_footer_appkit_font_weight()
    ];
    let mut keycap = find_subview_by_identifier(search_root, keycap_id);
    if keycap == nil {
        keycap = msg_send![class!(NSView), alloc];
        keycap = msg_send![keycap, initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(keycap_height, keycap_height))];
        if keycap == nil {
            return 0.0;
        }
        set_footer_view_identifier(keycap, keycap_id);
        let _: () = msg_send![keycap, setWantsLayer: YES];
        let _: () = msg_send![visual_parent, addSubview: keycap];
    }

    // The left lane used to treat a shortcut such as ⌘↵ as one text run in
    // one wide chip. Trailing buttons split that shortcut into one keycap per
    // token, so the left return glyph never received the calibrated ↵ offset.
    // Keep the outer view as a transparent run container, then build every
    // token with the same tokenizer, gap, padding, and optical correction
    // helpers as the trailing button path.
    remove_identified_subview(keycap, glyph_id);
    let keycap_layer: id = msg_send![keycap, layer];
    if keycap_layer != nil {
        let _: () = msg_send![keycap_layer, setCornerRadius: 0.0_f64];
        let _: () = msg_send![keycap_layer, setBorderWidth: 0.0_f64];
    }

    let theme = crate::theme::get_cached_theme();
    let border = ns_color_from_hex_with_alpha(
        footer_keycap_hex(&theme),
        footer_keycap_border_alpha(&theme, false),
    );
    let key_gap = metrics.content_gap as f64;
    let mut keycap_run_width = 0.0_f64;

    for (index, token) in shortcut_tokens.iter().enumerate() {
        let token_keycap_id = format!("{keycap_id}-{index}");
        let token_glyph_id = format!("{glyph_id}-{index}");
        let mut token_keycap = find_subview_by_identifier(keycap, &token_keycap_id);
        if token_keycap == nil {
            token_keycap = msg_send![class!(NSView), alloc];
            token_keycap = msg_send![
                token_keycap,
                initWithFrame: NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(keycap_height, keycap_height)
                )
            ];
            if token_keycap == nil {
                continue;
            }
            set_footer_view_identifier(token_keycap, &token_keycap_id);
            let _: () = msg_send![token_keycap, setWantsLayer: YES];
            let _: () = msg_send![keycap, addSubview: token_keycap];
        }

        let mut glyph_view = find_subview_by_identifier(token_keycap, &token_glyph_id);
        if glyph_view == nil {
            glyph_view = make_footer_hint_text_field(token, font, text_color, 1usize);
            if glyph_view == nil {
                continue;
            }
            set_footer_view_identifier(glyph_view, &token_glyph_id);
            let _: () = msg_send![token_keycap, addSubview: glyph_view];
        }
        let value = ns_string(token);
        if value != nil {
            let _: () = msg_send![glyph_view, setStringValue: value];
        }
        if font != nil {
            let _: () = msg_send![glyph_view, setFont: font];
        }
        if text_color != nil {
            let _: () = msg_send![glyph_view, setTextColor: text_color];
        }
        let _: () = msg_send![glyph_view, sizeToFit];
        let glyph_size: NSSize = msg_send![glyph_view, fittingSize];
        let keycap_padding_x =
            crate::components::footer_chrome::footer_keycap_padding_x_for_token(token, &metrics)
                as f64;
        let token_keycap_width = (glyph_size.width + keycap_padding_x * 2.0).max(keycap_height);
        let glyph_x = crate::components::footer_chrome::footer_appkit_glyph_x(
            token,
            token_keycap_width,
            glyph_size.width,
        );
        let glyph_y = metrics.keycap_padding_y as f64
            + crate::components::footer_chrome::footer_appkit_glyph_y(
                token,
                (keycap_height - metrics.keycap_padding_y as f64 * 2.0).max(0.0),
                glyph_size.height,
            );
        let _: () = msg_send![
            glyph_view,
            setFrame: NSRect::new(NSPoint::new(glyph_x, glyph_y), glyph_size)
        ];

        let token_layer: id = msg_send![token_keycap, layer];
        if token_layer != nil {
            let _: () = msg_send![
                token_layer,
                setCornerRadius: metrics.keycap_radius as f64
            ];
            let _: () = msg_send![token_layer, setBorderWidth: 1.0_f64];
            if border != nil {
                let cg: id = msg_send![border, CGColor];
                if cg != nil {
                    let _: () = msg_send![token_layer, setBorderColor: cg];
                }
            }
        }
        let _: () = msg_send![
            token_keycap,
            setFrame: NSRect::new(
                NSPoint::new(keycap_run_width, 0.0),
                NSSize::new(token_keycap_width, keycap_height)
            )
        ];
        let _: () = msg_send![token_keycap, setHidden: NO];
        keycap_run_width += token_keycap_width;
        if index + 1 < shortcut_tokens.len() {
            keycap_run_width += key_gap;
        }
    }

    // A tip can rotate from a longer shortcut to a shorter one while the
    // native footer host is reused. Remove any no-longer-owned token views.
    for index in shortcut_tokens.len()..16 {
        let stale_id = format!("{keycap_id}-{index}");
        remove_identified_subview(keycap, &stale_id);
    }
    let keycap_y = ((host_height - keycap_height) / 2.0).round();
    let _: () = msg_send![keycap, setFrame: NSRect::new(
        NSPoint::new(x + visual_offset_x, keycap_y + visual_offset_y),
        NSSize::new(keycap_run_width, keycap_height),
    )];
    let _: () = msg_send![keycap, setHidden: NO];
    keycap_run_width
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeFooterLeftHitTargetFlags {
    selected: bool,
    enabled: bool,
}

fn native_footer_left_hit_target_flags(
    selected: bool,
    enabled: bool,
) -> NativeFooterLeftHitTargetFlags {
    NativeFooterLeftHitTargetFlags { selected, enabled }
}

#[cfg(target_os = "macos")]
unsafe fn layout_footer_cwd_chip_hit_target(
    left_info_view: id,
    frame: cocoa::foundation::NSRect,
    tooltip: Option<&str>,
    selected: bool,
    enabled: bool,
) {
    use objc::{msg_send, sel, sel_impl};

    if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_HIT_TARGET_ID);
        return;
    }

    let mut button = find_subview_by_identifier(left_info_view, FOOTER_CWD_CHIP_HIT_TARGET_ID);
    if button == nil {
        button = msg_send![footer_button_class(), alloc];
        button = msg_send![button, initWithFrame: frame];
        if button == nil {
            return;
        }
        set_footer_view_identifier(button, FOOTER_CWD_CHIP_HIT_TARGET_ID);
        let _: () = msg_send![button, setBordered: NO];
        let _: () = msg_send![button, setBezelStyle: 0usize];
        let _: () = msg_send![button, setButtonType: 0usize];
        let _: () = msg_send![button, setTransparent: YES];
        if let Some(object) = button.as_mut() {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
            object.set_ivar::<usize>("_stateView", 0);
            object.set_ivar::<usize>("_visualRoot", left_info_view as usize);
        }
        let _: () = msg_send![left_info_view, addSubview: button];
    }
    let _: () = msg_send![button, setFrame: frame];
    let _: () = msg_send![button, setEnabled: if enabled { YES } else { NO }];
    let _: () = msg_send![button, setTarget: footer_action_target()];
    let action_selector = footer_action_selector(FooterAction::Cwd);
    let previous_action: objc::runtime::Sel = msg_send![button, action];
    let _: () = msg_send![button, setAction: action_selector];
    let flags = native_footer_left_hit_target_flags(selected, enabled);
    if let Some(object) = button.as_mut() {
        object.set_ivar::<cocoa::base::BOOL>("_isActionsButton", NO);
        object.set_ivar::<cocoa::base::BOOL>("_selected", if flags.selected { YES } else { NO });
        object.set_ivar::<cocoa::base::BOOL>("_enabled", if flags.enabled { YES } else { NO });
        if previous_action != action_selector {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
        }
        object.set_ivar::<usize>(
            "_stateView",
            find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_STATE_LAYER_ID) as usize,
        );
        object.set_ivar::<usize>("_visualRoot", left_info_view as usize);
    }
    let tooltip = tooltip.map(ns_string).unwrap_or(nil);
    let _: () = msg_send![button, setToolTip: tooltip];
    refresh_footer_button_visual_states(left_info_view);
}

#[cfg(target_os = "macos")]
unsafe fn layout_footer_left_info_hit_target(
    left_info_view: id,
    action: Option<FooterAction>,
    frame: cocoa::foundation::NSRect,
    selected: bool,
    enabled: bool,
) {
    use objc::{msg_send, sel, sel_impl};

    let Some(action) = action else {
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_HIT_TARGET_ID);
        return;
    };
    if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_HIT_TARGET_ID);
        return;
    }

    let mut button = find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_HIT_TARGET_ID);
    if button == nil {
        button = msg_send![footer_button_class(), alloc];
        button = msg_send![button, initWithFrame: frame];
        if button == nil {
            return;
        }
        set_footer_view_identifier(button, FOOTER_LEFT_INFO_HIT_TARGET_ID);
        let _: () = msg_send![button, setBordered: NO];
        let _: () = msg_send![button, setBezelStyle: 0usize];
        let _: () = msg_send![button, setButtonType: 0usize];
        let _: () = msg_send![button, setTransparent: YES];
        if let Some(object) = button.as_mut() {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
            object.set_ivar::<usize>("_stateView", 0);
            object.set_ivar::<usize>("_visualRoot", left_info_view as usize);
        }
        let _: () = msg_send![left_info_view, addSubview: button];
    }
    let _: () = msg_send![button, setFrame: frame];
    let _: () = msg_send![button, setEnabled: if enabled { YES } else { NO }];
    let _: () = msg_send![button, setTarget: footer_action_target()];
    let action_selector = footer_action_selector(action);
    let previous_action: objc::runtime::Sel = msg_send![button, action];
    let _: () = msg_send![button, setAction: action_selector];
    let flags = native_footer_left_hit_target_flags(selected, enabled);
    if let Some(object) = button.as_mut() {
        object.set_ivar::<cocoa::base::BOOL>("_isActionsButton", NO);
        object.set_ivar::<cocoa::base::BOOL>("_selected", if flags.selected { YES } else { NO });
        object.set_ivar::<cocoa::base::BOOL>("_enabled", if flags.enabled { YES } else { NO });
        if previous_action != action_selector {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
        }
        object.set_ivar::<usize>(
            "_stateView",
            find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_STATE_LAYER_ID) as usize,
        );
        object.set_ivar::<usize>("_visualRoot", left_info_view as usize);
    }
    refresh_footer_button_visual_states(left_info_view);
}

#[cfg(target_os = "macos")]
unsafe fn update_footer_dot_layer(layer: id, info: &FooterLeftInfo) {
    update_footer_dot_layer_for_status(
        layer,
        info.dot_status,
        info.prefer_accent_for_active_states,
    );
}

/// Status-driven dot layer update, shared by the legacy left-info marker and the
/// per-button leading dot (Agent·Model chip). `Hidden` collapses the dot to fully
/// transparent + no pulse so a reserved lane can stay width-stable without
/// showing anything.
#[cfg(target_os = "macos")]
unsafe fn update_footer_dot_layer_for_status(
    layer: id,
    dot_status: FooterDotStatus,
    prefer_accent_for_active_states: bool,
) {
    use objc::{msg_send, sel, sel_impl};

    let _: () = msg_send![layer, setCornerRadius: FOOTER_STREAMING_DOT_SIZE / 2.0_f64];

    if matches!(dot_status, FooterDotStatus::Hidden) {
        remove_active_dot_pulse_animation(layer);
        let _: () = msg_send![layer, setOpacity: 0.0_f32];
        return;
    }

    let theme = crate::theme::get_cached_theme();
    let dot_hex = footer_dot_hex(dot_status, &theme, prefer_accent_for_active_states);

    let dot_ns = ns_color_from_hex_with_alpha(dot_hex, 1.0);
    if dot_ns != nil {
        let cg: id = msg_send![dot_ns, CGColor];
        if cg != nil {
            let _: () = msg_send![layer, setBackgroundColor: cg];
        }
    }

    let should_pulse = matches!(
        dot_status,
        FooterDotStatus::Streaming | FooterDotStatus::WaitingForPermission
    );
    if should_pulse {
        ensure_active_dot_pulse_animation(layer);
    } else {
        remove_active_dot_pulse_animation(layer);
        let _: () = msg_send![layer, setOpacity: 1.0_f32];
    }
}

#[cfg(target_os = "macos")]
unsafe fn update_footer_icon_layer(layer: id, info: &FooterLeftInfo) {
    use objc::{msg_send, sel, sel_impl};

    let should_pulse = matches!(
        info.dot_status,
        FooterDotStatus::Streaming | FooterDotStatus::WaitingForPermission
    );
    if should_pulse {
        ensure_active_dot_pulse_animation(layer);
    } else {
        remove_active_dot_pulse_animation(layer);
        let _: () = msg_send![layer, setOpacity: 1.0_f32];
    }
}

#[cfg(target_os = "macos")]
unsafe fn layer_has_animation(layer: id, key: &str) -> bool {
    use objc::{msg_send, sel, sel_impl};

    let key = ns_string(key);
    if key == nil {
        return false;
    }
    let animation: id = msg_send![layer, animationForKey: key];
    animation != nil
}

#[cfg(target_os = "macos")]
unsafe fn ensure_active_dot_pulse_animation(layer: id) {
    if layer == nil {
        return;
    }
    let has_opacity = layer_has_animation(layer, "pulseOpacity");
    if has_opacity {
        remove_active_dot_scale_animation(layer);
        return;
    }
    remove_active_dot_pulse_animation(layer);
    add_active_dot_pulse_animation(layer);
}

#[cfg(target_os = "macos")]
unsafe fn remove_active_dot_pulse_animation(layer: id) {
    use objc::{msg_send, sel, sel_impl};

    let opacity_key = ns_string("pulseOpacity");
    if opacity_key != nil {
        let _: () = msg_send![layer, removeAnimationForKey: opacity_key];
    }
    remove_active_dot_scale_animation(layer);
}

#[cfg(target_os = "macos")]
unsafe fn remove_active_dot_scale_animation(layer: id) {
    use objc::{msg_send, sel, sel_impl};

    let scale_key = ns_string("pulseScale");
    if scale_key != nil {
        let _: () = msg_send![layer, removeAnimationForKey: scale_key];
    }
}

#[cfg(target_os = "macos")]
unsafe fn recolor_footer_hint_subviews(view: id, theme: &crate::theme::Theme) {
    recolor_footer_hint_subviews_with_visual_theme(
        view,
        theme,
        resolve_native_footer_visual_theme(theme),
    );
}

#[cfg(target_os = "macos")]
unsafe fn recolor_footer_hint_subviews_with_visual_theme(
    view: id,
    _theme: &crate::theme::Theme,
    visual_theme: NativeFooterVisualTheme,
) {
    if view == nil {
        return;
    }

    let text_color = ns_color_from_rgba(visual_theme.row_palette.rest.primary_foreground_rgba);
    let border_color = ns_color_from_hex_with_alpha(
        visual_theme.keycap_hex,
        visual_theme.border_alpha(crate::theme::MainMenuRowState::Rest) as f64,
    );

    recolor_footer_hint_subviews_with_colors(view, text_color, border_color);
    refresh_footer_button_visual_states_with_theme(view, visual_theme);
}

#[cfg(target_os = "macos")]
unsafe fn refresh_footer_button_visual_states(view: id) {
    let theme = crate::theme::get_cached_theme();
    refresh_footer_button_visual_states_with_theme(
        view,
        resolve_native_footer_visual_theme(&theme),
    );
}

#[cfg(target_os = "macos")]
unsafe fn refresh_footer_button_visual_states_with_theme(
    view: id,
    visual_theme: NativeFooterVisualTheme,
) {
    if view == nil {
        return;
    }

    let mut states_by_visual_root = std::collections::HashMap::new();
    collect_footer_button_visual_states(view, &mut states_by_visual_root);
    for (_visual_root, (button, state)) in states_by_visual_root {
        apply_footer_button_visual_state_with_theme(button, state, visual_theme);
    }
}

#[cfg(target_os = "macos")]
unsafe fn collect_footer_button_visual_states(
    view: id,
    states_by_visual_root: &mut std::collections::HashMap<
        usize,
        (id, crate::theme::MainMenuRowState),
    >,
) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }

    let is_footer_button: cocoa::base::BOOL = msg_send![view, isKindOfClass: footer_button_class()];
    if is_footer_button == YES {
        if let Some(object) = view.as_ref() {
            let selected = *object.get_ivar::<cocoa::base::BOOL>("_selected") == YES;
            let hovered = *object.get_ivar::<cocoa::base::BOOL>("_hovered") == YES;
            let is_actions = *object.get_ivar::<cocoa::base::BOOL>("_isActionsButton") == YES;
            let state = resolved_native_footer_button_state(
                selected,
                hovered,
                crate::actions::is_actions_window_open(),
                is_actions,
            );
            let visual_root = footer_button_visual_root(view) as usize;
            match states_by_visual_root.get(&visual_root) {
                Some((_, existing_state))
                    if native_footer_visual_root_state(Some(*existing_state), state)
                        == *existing_state => {}
                _ => {
                    states_by_visual_root.insert(visual_root, (view, state));
                }
            }
        }
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        collect_footer_button_visual_states(child, states_by_visual_root);
    }
}

fn native_footer_state_rank(state: crate::theme::MainMenuRowState) -> u8 {
    match state {
        crate::theme::MainMenuRowState::Rest => 0,
        crate::theme::MainMenuRowState::Hover => 1,
        crate::theme::MainMenuRowState::Active => 2,
    }
}

fn native_footer_visual_root_state(
    current: Option<crate::theme::MainMenuRowState>,
    incoming: crate::theme::MainMenuRowState,
) -> crate::theme::MainMenuRowState {
    match current {
        Some(current)
            if native_footer_state_rank(current) >= native_footer_state_rank(incoming) =>
        {
            current
        }
        _ => incoming,
    }
}

#[cfg(target_os = "macos")]
unsafe fn restyle_footer_glass_capsules(view: id, theme: &crate::theme::Theme) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }
    if let Some(glass_class) = objc::runtime::Class::get("NSGlassEffectView") {
        let is_glass: cocoa::base::BOOL = msg_send![view, isKindOfClass: glass_class];
        if is_glass == YES {
            style_float_footer_capsule(view, theme);
        }
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        restyle_footer_glass_capsules(child, theme);
    }
}

#[cfg(target_os = "macos")]
unsafe fn recolor_footer_hint_subviews_with_colors(view: id, text_color: id, border_color: id) {
    use objc::{class, msg_send, sel, sel_impl};

    if view == nil {
        return;
    }

    if text_color != nil {
        let is_text_field: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSTextField)];
        if is_text_field == YES {
            let _: () = msg_send![view, setTextColor: text_color];
        }
        let is_image_view: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSImageView)];
        if is_image_view == YES {
            let _: () = msg_send![view, setContentTintColor: text_color];
        }
    }

    if border_color != nil
        && appkit_view_identifier(view)
            .as_deref()
            .is_some_and(footer_identifier_uses_keycap_border)
    {
        let layer: id = msg_send![view, layer];
        if layer != nil {
            let border_width: f64 = msg_send![layer, borderWidth];
            if border_width > 0.0 {
                let cg_border: id = msg_send![border_color, CGColor];
                if cg_border != nil {
                    let _: () = msg_send![layer, setBorderColor: cg_border];
                }
            }
        }
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for i in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: i];
        recolor_footer_hint_subviews_with_colors(child, text_color, border_color);
    }
}

fn footer_identifier_uses_keycap_border(identifier: &str) -> bool {
    identifier.contains("keycap")
}

#[cfg(target_os = "macos")]
fn footer_hint_item_gap(glass_active: bool, ordinary_gap: f64) -> f64 {
    if glass_active {
        crate::components::footer_chrome::FOOTER_GLASS_BUTTON_GAP_PX as f64
    } else {
        ordinary_gap
    }
}

#[cfg(target_os = "macos")]
const FOOTER_MIC_ICON_SVG: &str =
    include_str!("../vendor/gpui-component/crates/assets/assets/icons/mic.svg");
#[cfg(target_os = "macos")]
const FOOTER_PROFILE_ICON_SVG: &str =
    include_str!("../vendor/gpui-component/crates/assets/assets/icons/bot.svg");

#[cfg(target_os = "macos")]
fn footer_icon_png_from_svg(svg: &str) -> Option<Vec<u8>> {
    let svg = svg.replace("currentColor", "white");
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &opts).ok()?;
    let size = 32_u32;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let svg_size = tree.size();
    let scale = (size as f32 / svg_size.width()).min(size as f32 / svg_size.height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap.take();
    if !rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0) {
        return None;
    }
    let image = image::RgbaImage::from_raw(size, size, rgba)?;
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .ok()?;
    Some(cursor.into_inner())
}

#[cfg(target_os = "macos")]
fn footer_mic_icon_png_data() -> Option<&'static [u8]> {
    static PNG_DATA: std::sync::OnceLock<Option<Vec<u8>>> = std::sync::OnceLock::new();
    PNG_DATA
        .get_or_init(|| footer_icon_png_from_svg(FOOTER_MIC_ICON_SVG))
        .as_deref()
}

#[cfg(target_os = "macos")]
fn footer_profile_icon_png_data() -> Option<&'static [u8]> {
    static PNG_DATA: std::sync::OnceLock<Option<Vec<u8>>> = std::sync::OnceLock::new();
    PNG_DATA
        .get_or_init(|| footer_icon_png_from_svg(FOOTER_PROFILE_ICON_SVG))
        .as_deref()
}

#[cfg(target_os = "macos")]
fn footer_icon_png_data(token: &str) -> Option<&'static [u8]> {
    match token {
        crate::components::footer_chrome::FOOTER_MIC_ICON_TOKEN => footer_mic_icon_png_data(),
        crate::components::footer_chrome::FOOTER_PROFILE_ICON_TOKEN => {
            footer_profile_icon_png_data()
        }
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn footer_icon_png_bytes(token: &str) -> Option<Vec<u8>> {
    if let Some(data) = footer_icon_png_data(token) {
        return Some(data.to_vec());
    }
    let path = crate::components::footer_chrome::footer_icon_path(token)
        .unwrap_or_else(|| crate::components::footer_chrome::FOOTER_PROFILE_ICON_PATH.to_string());
    let svg = if std::path::Path::new(&path).is_absolute() {
        std::fs::read_to_string(path).ok()?
    } else {
        String::from_utf8(crate::utils::assets::embedded_asset_bytes(&path)?).ok()?
    };
    footer_icon_png_from_svg(&svg)
}

#[cfg(target_os = "macos")]
unsafe fn footer_icon_image(token: &str) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let Some(png_data) = footer_icon_png_bytes(token) else {
        return nil;
    };
    let data: id = msg_send![
        class!(NSData),
        dataWithBytes: png_data.as_ptr()
        length: png_data.len()
    ];
    if data == nil {
        return nil;
    }
    let image: id = msg_send![class!(NSImage), alloc];
    let image: id = msg_send![image, initWithData: data];
    if image != nil {
        let _: () = msg_send![image, setTemplate: YES];
    }
    image
}

/// Build a small status-dot NSView for the leading edge of a footer button
/// (the Agent Chat streaming/idle dot inside the Agent·Model chip). Uses
/// accent-preferred active states to match the legacy Agent Chat left-info marker.
#[cfg(target_os = "macos")]
unsafe fn make_footer_hint_leading_dot_view(
    action: FooterAction,
    dot_status: FooterDotStatus,
) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let dot_view: id = msg_send![class!(NSView), alloc];
    let dot_view: id = msg_send![
        dot_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(FOOTER_STREAMING_DOT_SIZE, FOOTER_STREAMING_DOT_SIZE),
        )
    ];
    if dot_view == nil {
        return nil;
    }

    let identifier = ns_string(&format!(
        "{}{}",
        FOOTER_HINT_LEADING_DOT_ID_PREFIX,
        footer_action_key(action)
    ));
    if identifier != nil {
        let _: () = msg_send![dot_view, setIdentifier: identifier];
    }

    let _: () = msg_send![dot_view, setWantsLayer: YES];
    let layer: id = msg_send![dot_view, layer];
    if layer != nil {
        let _: () = msg_send![layer, setMasksToBounds: NO];
        update_footer_dot_layer_for_status(layer, dot_status, true);
    }
    let _: () = msg_send![
        dot_view,
        setHidden: if matches!(dot_status, FooterDotStatus::Hidden) {
            YES
        } else {
            NO
        }
    ];
    dot_view
}

#[cfg(target_os = "macos")]
unsafe fn make_footer_hint_text_field(
    text: &str,
    font: id,
    text_color: id,
    alignment: usize,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let field: id = msg_send![class!(NSTextField), alloc];
    let field: id = msg_send![field, init];
    if field == nil {
        return nil;
    }

    let string_value = ns_string(text);
    if string_value == nil {
        return nil;
    }

    let _: () = msg_send![field, setStringValue: string_value];
    let _: () = msg_send![field, setBezeled: NO];
    let _: () = msg_send![field, setBordered: NO];
    let _: () = msg_send![field, setDrawsBackground: NO];
    let _: () = msg_send![field, setEditable: NO];
    let _: () = msg_send![field, setSelectable: NO];
    if font != nil {
        let _: () = msg_send![field, setFont: font];
    }
    if text_color != nil {
        let _: () = msg_send![field, setTextColor: text_color];
    }
    let _: () = msg_send![field, setAlignment: alignment];
    let _: () = msg_send![field, setLineBreakMode: 4usize];
    let _: () = msg_send![field, setUsesSingleLineMode: YES];
    let _: () = msg_send![field, sizeToFit];
    field
}
