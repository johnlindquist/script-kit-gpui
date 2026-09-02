use super::panel::PermisoPanel;
use crate::platform::permiso::locator::{settings_window_snapshot, AppKitRect};

pub struct PassiveOverlayPanel;

pub struct OverlayController {
    panel: PermisoPanel,
    overlay: Option<PassiveOverlayPanel>,
    dismissed: bool,
}

impl OverlayController {
    pub fn present(panel: PermisoPanel) -> anyhow::Result<Self> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeVisibility)?;
        let _target = settings_window_snapshot();
        Ok(Self {
            panel,
            overlay: Some(PassiveOverlayPanel),
            dismissed: false,
        })
    }

    pub fn panel(&self) -> PermisoPanel {
        self.panel
    }

    pub fn dismiss(&mut self) {
        self.dismissed = true;
        self.overlay = None;
    }

    pub fn is_dismissed(&self) -> bool {
        self.dismissed
    }
}

impl Drop for OverlayController {
    fn drop(&mut self) {
        self.dismiss();
    }
}

pub fn anchored_origin(settings_frame: AppKitRect, overlay_size: (f64, f64)) -> (f64, f64) {
    let x = settings_frame.x + ((settings_frame.width - overlay_size.0) / 2.0).max(0.0);
    let y = settings_frame.y + settings_frame.height - overlay_size.1 - 24.0;
    (x, y.max(settings_frame.y + 24.0))
}

pub fn spring_frame_at(from: AppKitRect, to: AppKitRect, t: f64) -> AppKitRect {
    let t = t.clamp(0.0, 1.0);
    let eased = 1.0 - (-8.0 * t).exp() * (1.0 + 8.0 * t);
    AppKitRect {
        x: from.x + (to.x - from.x) * eased,
        y: from.y + (to.y - from.y) * eased,
        width: from.width + (to.width - from.width) * eased,
        height: from.height + (to.height - from.height) * eased,
    }
}

#[cfg(target_os = "macos")]
pub fn configure_passive_panel(_panel: cocoa::base::id) {
    // NSPanel subclass contract: .nonactivatingPanel, canBecomeKey = false,
    // canBecomeMain = false, statusBar level, orderFrontRegardless. This
    // overlay never calls setActivationPolicy: and never activates Script Kit.
}
