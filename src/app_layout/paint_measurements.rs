fn paint_measurement_component_type(stable_id: &str) -> protocol::LayoutComponentType {
    use protocol::LayoutComponentType;

    if stable_id.contains("transcript-row-")
        || stable_id.starts_with("dictation-history-row-")
        || stable_id.starts_with("chat-transcript-user-")
        || stable_id.starts_with("chat-transcript-response-")
    {
        return LayoutComponentType::ListItem;
    }

    if stable_id == crate::components::builtin_leading_separator::BUILTIN_LEADING_SEPARATOR_ID {
        return LayoutComponentType::Header;
    }

    match stable_id {
        "main-view-context-cwd-button"
        | "main-view-context-model-button"
        | "agent-chat-send-button" => LayoutComponentType::Button,
        "main-view-input-shell"
        | "main-view-input-body"
        | "focused-text-mini-input-row"
        | "focused-text-mini-scope-row" => LayoutComponentType::Input,
        "agent-chat-transcript-viewport" | "chat-transcript-viewport" => LayoutComponentType::List,
        "native-main-window-footer-spacer" => LayoutComponentType::Panel,
        "main-view-header" => LayoutComponentType::Header,
        "main-view-shell" | "main-view-context-zone" | "main-view-main" => {
            LayoutComponentType::Container
        }
        _ => LayoutComponentType::Other,
    }
}

#[cfg(test)]
mod paint_measurement_component_type_tests {
    #[test]
    fn dictation_history_row_selectors_are_list_items() {
        assert_eq!(
            super::paint_measurement_component_type("dictation-history-row-entry-123"),
            crate::protocol::LayoutComponentType::ListItem
        );
    }
}

impl ScriptListApp {
    /// Append bounds recorded by GPUI's current rendered frame.
    ///
    /// These nodes intentionally use their debug-selector IDs rather than the
    /// formula component names above. Missing selectors remain missing so
    /// fidelity comparisons fail closed instead of silently using estimates.
    fn append_paint_measurements(layout: &mut protocol::LayoutInfo, window: &gpui::Window) {
        use protocol::LayoutComponentInfo;

        let mut measurements: Vec<_> = window.debug_bounds_entries().iter().collect();
        measurements.sort_by(|left, right| left.selector.cmp(&right.selector));
        let frame_generation = window.rendered_frame_generation();

        for measurement in measurements {
            let component_type = paint_measurement_component_type(measurement.selector.as_str());
            let bounds = measurement.bounds;
            let visible = measurement.visible_bounds;
            let clip = measurement.clip_bounds;

            layout.components.push(
                LayoutComponentInfo::new(measurement.selector.clone(), component_type)
                    .with_bounds(
                        bounds.origin.x.as_f32(),
                        bounds.origin.y.as_f32(),
                        bounds.size.width.as_f32(),
                        bounds.size.height.as_f32(),
                    )
                    .with_measurement("paint-time", "window")
                    .with_paint_visibility(
                        visible.origin.x.as_f32(),
                        visible.origin.y.as_f32(),
                        visible.size.width.as_f32(),
                        visible.size.height.as_f32(),
                        clip.origin.x.as_f32(),
                        clip.origin.y.as_f32(),
                        clip.size.width.as_f32(),
                        clip.size.height.as_f32(),
                    )
                    .with_measurement_frame(frame_generation),
            );
        }

        if window.fidelity_capture_active() {
            let main =
                crate::fidelity_capture::paint_target_snapshot(window, "main", "mainWindow", None);
            let appkit = crate::footer_popup::collect_main_footer_appkit_fidelity_snapshot(window);
            let overlay = crate::footer_popup::main_footer_overlay_fidelity_snapshot();
            let overlay_status = if overlay.is_some() {
                protocol::FidelityCaptureStatus::Captured
            } else {
                protocol::FidelityCaptureStatus::MissingOverlay
            };
            layout.fidelity = Some(protocol::FidelityLayoutSnapshot {
                capture_target: "agent-chat".to_string(),
                frame_generation: main.frame_generation,
                nodes: main.nodes,
                unscoped: main.unscoped,
                appkit_status: appkit.status,
                appkit: appkit.snapshot,
                overlay_status,
                overlays: overlay.into_iter().collect(),
            });
        }
    }
}
