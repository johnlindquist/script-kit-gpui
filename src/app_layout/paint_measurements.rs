
impl ScriptListApp {
    /// Append bounds recorded by GPUI's current rendered frame.
    ///
    /// These nodes intentionally use their debug-selector IDs rather than the
    /// formula component names above. Missing selectors remain missing so
    /// fidelity comparisons fail closed instead of silently using estimates.
    fn append_paint_measurements(layout: &mut protocol::LayoutInfo, window: &gpui::Window) {
        crate::windows::automation_surface_collector::append_window_paint_measurements(layout, window);

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
