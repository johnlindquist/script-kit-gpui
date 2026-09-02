/// One-shot introspection: log NSGlassEffectView's declared properties so we
/// can discover any rim/style knobs Apple exposes (macOS 26 API surface is
/// underdocumented). Debug aid; logs once per process.
#[cfg(target_os = "macos")]
unsafe fn log_glass_effect_view_properties_once(glass_class: &objc::runtime::Class) {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        #[link(name = "objc")]
        extern "C" {
            fn class_copyPropertyList(
                cls: *const objc::runtime::Class,
                out_count: *mut u32,
            ) -> *mut *const std::ffi::c_void;
            fn property_getName(property: *const std::ffi::c_void) -> *const std::os::raw::c_char;
            fn property_getAttributes(
                property: *const std::ffi::c_void,
            ) -> *const std::os::raw::c_char;
            fn free(ptr: *mut std::ffi::c_void);
        }
        let mut count: u32 = 0;
        let list = class_copyPropertyList(glass_class as *const _, &mut count);
        if list.is_null() {
            return;
        }
        let mut names = Vec::new();
        for index in 0..count as usize {
            let property = *list.add(index);
            let name = property_getName(property);
            let attrs = property_getAttributes(property);
            if !name.is_null() {
                let attr_text = if attrs.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(attrs)
                        .to_string_lossy()
                        .into_owned()
                };
                names.push(format!(
                    "{}[{}]",
                    std::ffi::CStr::from_ptr(name).to_string_lossy(),
                    attr_text,
                ));
            }
        }
        free(list as *mut std::ffi::c_void);
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "glass_effect_view_properties",
            properties = %names.join(","),
            "NSGlassEffectView declared properties"
        );
    });
}

struct GpuiFooterOverlaySlot {
    handle: WindowHandle<GpuiFooterOverlay>,
    parent_window_handle: AnyWindowHandle,
    info: crate::protocol::AutomationWindowInfo,
    presentation_revision: u64,
    applied_theme_revision: u64,
}

/// Stable automation-registry identity for the GPUI footer overlay window so
/// DevTools primitives (captureWindow, inspectAutomationWindow) can target it.
const GPUI_FOOTER_OVERLAY_AUTOMATION_ID: &str = "footer-overlay";
const GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID: &str = "gpui-footer-overlay";
const GPUI_FOOTER_OVERLAY_WINDOW_TITLE: &str = "Script Kit Footer Overlay";

fn automation_bounds_from_gpui(bounds: Bounds<Pixels>) -> crate::protocol::AutomationWindowBounds {
    crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}


#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AppKitFidelityCaptureOutcome {
    pub status: crate::protocol::FidelityCaptureStatus,
    pub snapshot: Option<crate::protocol::AppKitFidelitySnapshot>,
}

impl AppKitFidelityCaptureOutcome {
    fn blocked(status: crate::protocol::FidelityCaptureStatus) -> Self {
        Self {
            status,
            snapshot: None,
        }
    }

    fn captured(snapshot: crate::protocol::AppKitFidelitySnapshot) -> Self {
        Self {
            status: crate::protocol::FidelityCaptureStatus::Captured,
            snapshot: Some(snapshot),
        }
    }
}

fn appkit_fidelity_inventory_blocker(
    nodes: &[crate::protocol::AppKitFidelityNode],
) -> Option<crate::protocol::FidelityCaptureStatus> {
    if nodes.is_empty() {
        return Some(crate::protocol::FidelityCaptureStatus::EmptyInventory);
    }
    let unique_ids: BTreeSet<_> = nodes.iter().map(|node| node.id.as_str()).collect();
    (unique_ids.len() != nodes.len())
        .then_some(crate::protocol::FidelityCaptureStatus::DuplicateIdentifiers)
}

fn clear_footer_overlay_fidelity_snapshot(parent: AnyWindowHandle) {
    if let Some(host) = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&parent.window_id()) { host.fidelity = None; }
}

fn store_footer_overlay_fidelity_snapshot(parent: AnyWindowHandle, snapshot: crate::protocol::FidelityPaintTargetSnapshot) {
    if let Some(host) = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&parent.window_id()) { host.fidelity = Some(snapshot); }
}

pub(crate) fn main_footer_overlay_fidelity_snapshot() -> Option<crate::protocol::FidelityPaintTargetSnapshot> {
    let handle = main_footer_handle()?;
    FOOTER_HOSTS.lock().ok()?.get(&handle.window_id())?.fidelity.clone()
}

#[cfg(target_os = "macos")]
fn window_gpui_view_and_ns_window(window: &Window) -> Option<(id, id)> {
    if let Ok(window_handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = window_handle.as_raw() {
            use objc::{msg_send, sel, sel_impl};

            let ns_view = appkit.ns_view.as_ptr() as id;
            // SAFETY: `ns_view` comes from a live GPUI window on the AppKit
            // main thread. `-[NSView window]` returns the owning NSWindow or nil.
            unsafe {
                let ns_window: id = msg_send![ns_view, window];
                if ns_window != nil {
                    return Some((ns_view, ns_window));
                }
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn appkit_layout_bounds(rect: cocoa::foundation::NSRect) -> crate::protocol::LayoutBounds {
    crate::protocol::LayoutBounds {
        x: rect.origin.x as f32,
        y: rect.origin.y as f32,
        width: rect.size.width as f32,
        height: rect.size.height as f32,
    }
}

#[cfg(target_os = "macos")]
fn appkit_screenshot_bounds(
    window_rect: cocoa::foundation::NSRect,
    screenshot_height: f64,
) -> crate::protocol::LayoutBounds {
    crate::protocol::LayoutBounds {
        x: window_rect.origin.x as f32,
        y: (screenshot_height - window_rect.origin.y - window_rect.size.height) as f32,
        width: window_rect.size.width as f32,
        height: window_rect.size.height as f32,
    }
}

#[cfg(target_os = "macos")]
unsafe fn appkit_ns_string(value: id) -> Option<String> {
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CStr;

    if value == nil {
        return None;
    }
    let utf8: *const std::os::raw::c_char = msg_send![value, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
unsafe fn appkit_view_identifier(view: id) -> Option<String> {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return None;
    }
    let identifier: id = msg_send![view, identifier];
    appkit_ns_string(identifier).filter(|identifier| !identifier.is_empty())
}

#[cfg(target_os = "macos")]
unsafe fn appkit_class_name(view: id) -> String {
    use objc::{msg_send, sel, sel_impl};

    let class: id = msg_send![view, class];
    let class_name: id = if class == nil {
        nil
    } else {
        msg_send![class, className]
    };
    appkit_ns_string(class_name).unwrap_or_else(|| "unknown".to_string())
}

#[cfg(target_os = "macos")]
unsafe fn appkit_color_from_ns_color(color: id) -> Option<crate::protocol::AppKitFidelityColor> {
    use objc::{class, msg_send, sel, sel_impl};

    if color == nil {
        return None;
    }
    let color_space: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
    let color: id = msg_send![color, colorUsingColorSpace: color_space];
    if color == nil {
        return None;
    }
    let mut red = 0.0_f64;
    let mut green = 0.0_f64;
    let mut blue = 0.0_f64;
    let mut alpha = 0.0_f64;
    let _: () = msg_send![
        color,
        getRed: &mut red
        green: &mut green
        blue: &mut blue
        alpha: &mut alpha
    ];
    Some(crate::protocol::AppKitFidelityColor {
        red,
        green,
        blue,
        alpha,
    })
}

#[cfg(target_os = "macos")]
unsafe fn appkit_color_from_cg_color(color: id) -> Option<crate::protocol::AppKitFidelityColor> {
    use objc::{class, msg_send, sel, sel_impl};

    if color == nil {
        return None;
    }
    let ns_color: id = msg_send![class!(NSColor), colorWithCGColor: color];
    appkit_color_from_ns_color(ns_color)
}

#[cfg(target_os = "macos")]
unsafe fn appkit_layer_fidelity(view: id) -> Option<crate::protocol::AppKitFidelityLayer> {
    use objc::{msg_send, sel, sel_impl};

    let layer: id = msg_send![view, layer];
    if layer == nil {
        return None;
    }
    let contents_scale: f64 = msg_send![layer, contentsScale];
    let masks_to_bounds: cocoa::base::BOOL = msg_send![layer, masksToBounds];
    let border_width: f64 = msg_send![layer, borderWidth];
    let corner_radius: f64 = msg_send![layer, cornerRadius];
    let background_color: id = msg_send![layer, backgroundColor];
    let border_color: id = msg_send![layer, borderColor];
    let shadow_opacity: f32 = msg_send![layer, shadowOpacity];
    let shadow_radius: f64 = msg_send![layer, shadowRadius];
    let shadow_offset: cocoa::foundation::NSSize = msg_send![layer, shadowOffset];
    let shadow_path: id = msg_send![layer, shadowPath];
    Some(crate::protocol::AppKitFidelityLayer {
        contents_scale,
        masks_to_bounds: masks_to_bounds == YES,
        border_width,
        corner_radius,
        background_color: appkit_color_from_cg_color(background_color),
        border_color: appkit_color_from_cg_color(border_color),
        shadow_opacity: f64::from(shadow_opacity),
        shadow_radius,
        shadow_offset_x: shadow_offset.width,
        shadow_offset_y: shadow_offset.height,
        has_shadow_path: shadow_path != nil,
    })
}

#[cfg(target_os = "macos")]
unsafe fn appkit_text_fidelity(view: id) -> Option<crate::protocol::AppKitFidelityText> {
    use objc::{class, msg_send, sel, sel_impl};
    use sha2::{Digest as _, Sha256};

    let is_text_field: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSTextField)];
    if is_text_field != YES {
        return None;
    }
    let value: id = msg_send![view, stringValue];
    let value = appkit_ns_string(value).unwrap_or_default();
    let font: id = msg_send![view, font];
    let font_name = if font == nil {
        String::new()
    } else {
        let name: id = msg_send![font, fontName];
        appkit_ns_string(name).unwrap_or_default()
    };
    let font_size: f64 = if font == nil {
        0.0
    } else {
        msg_send![font, pointSize]
    };
    let font_weight: isize = if font == nil {
        0
    } else {
        let manager: id = msg_send![class!(NSFontManager), sharedFontManager];
        msg_send![manager, weightOfFont: font]
    };
    let alignment: usize = msg_send![view, alignment];
    let fitting_size: cocoa::foundation::NSSize = msg_send![view, fittingSize];
    let text_color: id = msg_send![view, textColor];
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    Some(crate::protocol::AppKitFidelityText {
        value,
        value_sha256: format!("{:x}", hasher.finalize()),
        font_name,
        font_size,
        font_weight: font_weight as i64,
        alignment: alignment as i64,
        fitting_size: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width: fitting_size.width as f32,
            height: fitting_size.height as f32,
        },
        color: appkit_color_from_ns_color(text_color),
    })
}

#[cfg(target_os = "macos")]
unsafe fn appkit_image_fidelity(view: id) -> Option<crate::protocol::AppKitFidelityImage> {
    use objc::{class, msg_send, sel, sel_impl};

    let is_image_view: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSImageView)];
    if is_image_view != YES {
        return None;
    }
    let image: id = msg_send![view, image];
    let size = if image == nil {
        cocoa::foundation::NSSize::new(0.0, 0.0)
    } else {
        msg_send![image, size]
    };
    let supports_tint: cocoa::base::BOOL =
        msg_send![view, respondsToSelector: sel!(contentTintColor)];
    let tint: id = if supports_tint == YES {
        msg_send![view, contentTintColor]
    } else {
        nil
    };
    Some(crate::protocol::AppKitFidelityImage {
        width: size.width,
        height: size.height,
        tint: appkit_color_from_ns_color(tint),
    })
}

#[cfg(target_os = "macos")]
unsafe fn appkit_action_selector(view: id) -> Option<String> {
    use objc::{class, msg_send, sel, sel_impl};

    let is_button: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSButton)];
    if is_button != YES {
        return None;
    }
    let action: objc::runtime::Sel = msg_send![view, action];
    (!action.as_ptr().is_null()).then(|| action.name().to_string())
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct AppKitAccessibilityFidelity {
    identifier: Option<String>,
    role: Option<String>,
    label_sha256: Option<String>,
    label_length: Option<usize>,
    enabled: Option<bool>,
    focused: Option<bool>,
    element: Option<bool>,
}

#[cfg(target_os = "macos")]
unsafe fn appkit_accessibility_fidelity(view: id) -> AppKitAccessibilityFidelity {
    use objc::{class, msg_send, sel, sel_impl};
    use sha2::{Digest, Sha256};

    let string_property = |selector: objc::runtime::Sel| -> Option<String> {
        let responds: cocoa::base::BOOL = msg_send![view, respondsToSelector: selector];
        if responds != YES {
            return None;
        }
        let value: id = msg_send![view, performSelector: selector];
        appkit_ns_string(value)
    };
    let identifier = string_property(sel!(accessibilityIdentifier));
    let role = string_property(sel!(accessibilityRole));
    let label = string_property(sel!(accessibilityLabel));
    let (label_sha256, label_length) = label.map_or((None, None), |label| {
        let mut hasher = Sha256::new();
        hasher.update(label.as_bytes());
        (
            Some(format!("{:x}", hasher.finalize())),
            Some(label.chars().count()),
        )
    });
    let is_button: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSButton)];
    let enabled = (is_button == YES).then(|| {
        let value: cocoa::base::BOOL = msg_send![view, isEnabled];
        value == YES
    });
    let focused = {
        let responds: cocoa::base::BOOL =
            msg_send![view, respondsToSelector: sel!(isAccessibilityFocused)];
        (responds == YES).then(|| {
            let value: cocoa::base::BOOL = msg_send![view, isAccessibilityFocused];
            value == YES
        })
    };
    let element = {
        let responds: cocoa::base::BOOL =
            msg_send![view, respondsToSelector: sel!(isAccessibilityElement)];
        (responds == YES).then(|| {
            let value: cocoa::base::BOOL = msg_send![view, isAccessibilityElement];
            value == YES
        })
    };

    AppKitAccessibilityFidelity {
        identifier,
        role,
        label_sha256,
        label_length,
        enabled,
        focused,
        element,
    }
}

#[cfg(target_os = "macos")]
unsafe fn collect_identified_appkit_views(
    view: id,
    content_view: id,
    parent_id: Option<String>,
    subview_order: usize,
    screenshot_height: f64,
    nodes: &mut Vec<crate::protocol::AppKitFidelityNode>,
) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }
    let identifier = appkit_view_identifier(view);
    let node_parent_id = parent_id.clone();
    let child_parent_id = identifier.clone().or(parent_id);
    if let Some(identifier) = identifier {
        let frame: cocoa::foundation::NSRect = msg_send![view, frame];
        let bounds: cocoa::foundation::NSRect = msg_send![view, bounds];
        let window_frame: cocoa::foundation::NSRect =
            msg_send![view, convertRect: bounds toView: content_view];
        let hidden: cocoa::base::BOOL = msg_send![view, isHidden];
        let alpha: f64 = msg_send![view, alphaValue];
        let layer = appkit_layer_fidelity(view);
        let layer_masks_to_bounds = layer
            .as_ref()
            .map(|layer| layer.masks_to_bounds)
            .unwrap_or(false);
        let accessibility = appkit_accessibility_fidelity(view);
        nodes.push(crate::protocol::AppKitFidelityNode {
            id: identifier,
            parent_id: node_parent_id,
            class_name: appkit_class_name(view),
            subview_order,
            frame: appkit_layout_bounds(frame),
            bounds: appkit_layout_bounds(bounds),
            window_frame: appkit_layout_bounds(window_frame),
            screenshot_frame: appkit_screenshot_bounds(window_frame, screenshot_height),
            hidden: hidden == YES,
            alpha,
            // `-[NSView clipsToBounds]` is not available on every supported
            // macOS SDK/runtime pair. The backing layer is the raster clipping
            // authority here and avoids an unrecognized-selector crash.
            clips_to_bounds: layer_masks_to_bounds,
            layer,
            text: appkit_text_fidelity(view),
            image: appkit_image_fidelity(view),
            action_selector: appkit_action_selector(view),
            accessibility_identifier: accessibility.identifier,
            accessibility_role: accessibility.role,
            accessibility_label_sha256: accessibility.label_sha256,
            accessibility_label_length: accessibility.label_length,
            accessibility_enabled: accessibility.enabled,
            accessibility_focused: accessibility.focused,
            accessibility_element: accessibility.element,
        });
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        collect_identified_appkit_views(
            child,
            content_view,
            child_parent_id.clone(),
            index,
            screenshot_height,
            nodes,
        );
    }
}

#[cfg(target_os = "macos")]
unsafe fn appkit_subview_order(parent: id, child: id) -> usize {
    use objc::{msg_send, sel, sel_impl};

    let subviews: id = msg_send![parent, subviews];
    if subviews == nil {
        return 0;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let candidate: id = msg_send![subviews, objectAtIndex: index];
        if candidate == child {
            return index;
        }
    }
    0
}

/// Collect capture-only AppKit telemetry for the in-window footer material
/// host. The separate GPUI glyph overlay is intentionally excluded and emitted
/// through `main_footer_overlay_fidelity_snapshot`.
pub(crate) fn collect_main_footer_appkit_fidelity_snapshot(
    window: &Window,
) -> AppKitFidelityCaptureOutcome {
    if !window.fidelity_capture_active() {
        return AppKitFidelityCaptureOutcome::blocked(
            crate::protocol::FidelityCaptureStatus::NotRequested,
        );
    }

    #[cfg(target_os = "macos")]
    {
        let Some((_, ns_window)) = window_gpui_view_and_ns_window(window) else {
            return AppKitFidelityCaptureOutcome::blocked(
                crate::protocol::FidelityCaptureStatus::MissingWindow,
            );
        };
        // SAFETY: `ns_window` and its content tree belong to the live main
        // window. getLayoutInfo invokes this on the AppKit/GPUI main thread.
        unsafe {
            use objc::{msg_send, sel, sel_impl};

            let content_view: id = msg_send![ns_window, contentView];
            if content_view == nil {
                return AppKitFidelityCaptureOutcome::blocked(
                    crate::protocol::FidelityCaptureStatus::MissingContentView,
                );
            }
            let search_root = main_window_footer_search_root(ns_window);
            let footer_view = find_subview_by_identifier(search_root, FOOTER_EFFECT_ID);
            if footer_view == nil {
                return AppKitFidelityCaptureOutcome::blocked(
                    crate::protocol::FidelityCaptureStatus::MissingFooterHost,
                );
            }
            let content_bounds: cocoa::foundation::NSRect = msg_send![search_root, bounds];
            let mut nodes = Vec::new();
            let footer_order = appkit_subview_order(search_root, footer_view);
            collect_identified_appkit_views(
                footer_view,
                search_root,
                None,
                footer_order,
                content_bounds.size.height,
                &mut nodes,
            );
            if let Some(status) = appkit_fidelity_inventory_blocker(&nodes) {
                return AppKitFidelityCaptureOutcome::blocked(status);
            }

            let backdrop = find_subview_by_identifier(
                content_view,
                crate::platform::TAHOE_GLASS_BACKDROP_IDENTIFIER,
            );
            let footer_container =
                find_subview_by_identifier(content_view, FOOTER_GLASS_CONTAINER_ID);
            let main_backdrop_frame = (backdrop != nil).then(|| {
                let frame: cocoa::foundation::NSRect = msg_send![backdrop, frame];
                appkit_layout_bounds(frame)
            });
            let footer_container_frame = (footer_container != nil).then(|| {
                let frame: cocoa::foundation::NSRect = msg_send![footer_container, frame];
                appkit_layout_bounds(frame)
            });
            let (transparent_gap_points, backdrop_footer_intersection_area) =
                match (&main_backdrop_frame, &footer_container_frame) {
                    (Some(backdrop), Some(footer)) => {
                        let gap = backdrop.y - (footer.y + footer.height);
                        let overlap_width = (backdrop.x + backdrop.width)
                            .min(footer.x + footer.width)
                            - backdrop.x.max(footer.x);
                        let overlap_height = (backdrop.y + backdrop.height)
                            .min(footer.y + footer.height)
                            - backdrop.y.max(footer.y);
                        (
                            Some(gap),
                            Some(overlap_width.max(0.0) * overlap_height.max(0.0)),
                        )
                    }
                    _ => (None, None),
                };
            let has_shadow: cocoa::base::BOOL = msg_send![ns_window, hasShadow];
            let main_backdrop_layer = (backdrop != nil)
                .then(|| appkit_layer_fidelity(backdrop))
                .flatten();
            let mut material_bearing_view_ids = nodes
                .iter()
                .filter(|node| node.class_name == "NSGlassEffectView")
                .map(|node| node.id.clone())
                .collect::<Vec<_>>();
            if backdrop != nil {
                material_bearing_view_ids
                    .push(crate::platform::TAHOE_GLASS_BACKDROP_IDENTIFIER.to_string());
            }
            material_bearing_view_ids.sort();
            material_bearing_view_ids.dedup();

            AppKitFidelityCaptureOutcome::captured(crate::protocol::AppKitFidelitySnapshot {
                target_id: "main-footer-host".to_string(),
                target_kind: "appKitFooterHost".to_string(),
                coordinate_space: "appkit-content-bottom-left+screenshot-top-left".to_string(),
                window_bounds: crate::fidelity_capture::layout_bounds(window.bounds()),
                main_backdrop_frame,
                footer_container_frame,
                transparent_gap_points,
                backdrop_footer_intersection_area,
                outer_window_has_shadow: Some(has_shadow == YES),
                main_backdrop_layer,
                footer_left_allocation: footer_left_allocation_snapshot(),
                material_bearing_view_ids,
                nodes,
            })
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        AppKitFidelityCaptureOutcome::blocked(
            crate::protocol::FidelityCaptureStatus::UnsupportedPlatform,
        )
    }
}

/// Capture-safe facts from a native AppKit footer activation attempt.
///
/// The semantic descriptor remains the authority: automation may dispatch only
/// when the requested semantic ID resolves to the current footer config, the
/// AppKit peer is an enabled AXButton, and its target/action selector matches
/// that descriptor. Disabled controls refuse before `performClick:`.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeFooterActivationReceipt {
    pub semantic_id: String,
    pub surface: Option<String>,
    pub structural_id: Option<String>,
    pub accessibility_role: Option<String>,
    pub action_selector: Option<String>,
    pub expected_action_selector: Option<String>,
    pub descriptor_enabled: Option<bool>,
    pub appkit_enabled: Option<bool>,
    pub accessibility_focused_before: Option<bool>,
    pub accessibility_focused_after: Option<bool>,
    pub refused_disabled: bool,
    pub dispatched: bool,
    pub error_code: Option<String>,
}

impl NativeFooterActivationReceipt {
    fn blocked(semantic_id: &str, error_code: &str) -> Self {
        Self {
            semantic_id: semantic_id.to_string(),
            surface: None,
            structural_id: None,
            accessibility_role: None,
            action_selector: None,
            expected_action_selector: None,
            descriptor_enabled: None,
            appkit_enabled: None,
            accessibility_focused_before: None,
            accessibility_focused_after: None,
            refused_disabled: false,
            dispatched: false,
            error_code: Some(error_code.to_string()),
        }
    }
}

/// Activate the exact native footer button identified by the semantic footer
/// descriptor. This is intentionally narrower than generic AppKit automation.
pub(crate) fn activate_native_main_footer_button(
    window: &Window,
    semantic_id: &str,
) -> NativeFooterActivationReceipt {
    let config = footer_config_for_window(window.window_handle());
    let Some(config) = config else {
        return NativeFooterActivationReceipt::blocked(semantic_id, "missing_footer_config");
    };
    let Some(descriptor) = config
        .buttons
        .iter()
        .find(|button| button.id.as_ref() == semantic_id)
        .cloned()
    else {
        return NativeFooterActivationReceipt::blocked(semantic_id, "semantic_id_not_found");
    };
    let expected_action_selector = format!("{}FooterAction:", descriptor.action.semantic_key());

    #[cfg(target_os = "macos")]
    {
        let Some((_, ns_window)) = window_gpui_view_and_ns_window(window) else {
            return NativeFooterActivationReceipt::blocked(semantic_id, "missing_window");
        };
        // SAFETY: this runs on the GPUI/AppKit main thread against the live main
        // window and only searches inside the installed native footer host.
        unsafe {
            use objc::{class, msg_send, sel, sel_impl};

            let search_root = main_window_footer_search_root(ns_window);
            let footer_view = find_subview_by_identifier(search_root, FOOTER_EFFECT_ID);
            if footer_view == nil {
                return NativeFooterActivationReceipt::blocked(semantic_id, "missing_footer_host");
            }
            let button = find_subview_by_accessibility_identifier(footer_view, semantic_id);
            if button == nil {
                return NativeFooterActivationReceipt::blocked(semantic_id, "missing_ax_peer");
            }

            let structural_id = appkit_view_identifier(button);
            let accessibility = appkit_accessibility_fidelity(button);
            let action_selector = appkit_action_selector(button);
            let is_button: cocoa::base::BOOL = msg_send![button, isKindOfClass: class!(NSButton)];
            let hidden: cocoa::base::BOOL = msg_send![button, isHidden];
            let alpha: f64 = msg_send![button, alphaValue];
            let appkit_enabled = accessibility.enabled;
            let mut receipt = NativeFooterActivationReceipt {
                semantic_id: semantic_id.to_string(),
                surface: Some(config.surface.to_string()),
                structural_id,
                accessibility_role: accessibility.role,
                action_selector: action_selector.clone(),
                expected_action_selector: Some(expected_action_selector.clone()),
                descriptor_enabled: Some(descriptor.enabled),
                appkit_enabled,
                accessibility_focused_before: accessibility.focused,
                accessibility_focused_after: accessibility.focused,
                refused_disabled: false,
                dispatched: false,
                error_code: None,
            };

            if is_button != YES || receipt.accessibility_role.as_deref() != Some("AXButton") {
                receipt.error_code = Some("wrong_ax_role".to_string());
                return receipt;
            }
            if hidden == YES || alpha <= 0.0 {
                receipt.error_code = Some("hidden_control".to_string());
                return receipt;
            }
            if action_selector.as_deref() != Some(expected_action_selector.as_str()) {
                receipt.error_code = Some("wrong_action".to_string());
                return receipt;
            }
            if !descriptor.enabled || appkit_enabled != Some(true) {
                receipt.refused_disabled = true;
                receipt.error_code = Some("action_disabled".to_string());
                return receipt;
            }

            let _: () = msg_send![button, performClick: nil];
            receipt.dispatched = true;
            receipt.accessibility_focused_after = appkit_accessibility_fidelity(button).focused;
            receipt
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, descriptor, expected_action_selector);
        NativeFooterActivationReceipt::blocked(semantic_id, "unsupported_platform")
    }
}
