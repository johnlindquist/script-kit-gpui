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
        crate::platform::tahoe_liquid_glass_available() && theme.is_vibrancy_enabled();
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
