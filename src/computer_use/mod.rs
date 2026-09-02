//! Computer-use vocabulary for agent-facing desktop automation.
//!
//! This module is intentionally small: it maps computer-use language onto
//! Script Kit's existing state-first automation inspection protocol without
//! introducing a second targeting or screenshot model.

pub mod gpui_runtime_bridge;
pub mod native_window_capture;
#[cfg(target_os = "macos")]
pub(crate) mod owned_render_capture;
pub mod runtime_bridge;
pub mod see;
pub mod types;
pub mod window_observation;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseReadinessInput {
    pub enabled: bool,
    pub accessibility: crate::platform::permiso_detect::PermissionStatus,
    pub screen_recording: crate::platform::permiso_detect::PermissionStatus,
    pub keyboard_backend_available: bool,
    pub visible_window_count: Option<usize>,
    pub focused_target: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseReadinessReceipt {
    pub schema_version: u32,
    pub ready: bool,
    pub mode: &'static str,
    pub attention: Vec<ComputerUseReadinessAttention>,
    pub enabled: bool,
    pub accessibility: crate::platform::permiso_detect::PermissionStatus,
    pub screen_recording: crate::platform::permiso_detect::PermissionStatus,
    pub keyboard_backend_available: bool,
    pub visible_window_count: Option<usize>,
    pub focused_target: Option<String>,
    pub last_error: Option<String>,
    pub redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseReadinessAttention {
    pub code: &'static str,
    pub message: &'static str,
}

pub const COMPUTER_USE_READINESS_SCHEMA_VERSION: u32 = 1;
pub const COMPUTER_USE_READINESS_RESOURCE_URI: &str = "kit://computer-use/readiness";

pub fn build_computer_use_readiness_receipt(
    input: ComputerUseReadinessInput,
) -> ComputerUseReadinessReceipt {
    use crate::platform::permiso_detect::PermissionStatus;

    let mut attention = Vec::new();
    if !input.enabled {
        attention.push(ComputerUseReadinessAttention {
            code: "computer_use_disabled",
            message: "Computer Use is disabled.",
        });
    }
    if input.accessibility != PermissionStatus::Authorized {
        attention.push(ComputerUseReadinessAttention {
            code: "accessibility_missing",
            message: "Accessibility permission is required before controlling other apps.",
        });
    }
    if input.screen_recording != PermissionStatus::Authorized {
        attention.push(ComputerUseReadinessAttention {
            code: "screen_recording_missing",
            message: "Screen Recording permission is required before visual inspection.",
        });
    }
    if !input.keyboard_backend_available {
        attention.push(ComputerUseReadinessAttention {
            code: "keyboard_backend_unavailable",
            message: "Keyboard/event synthesis is not available.",
        });
    }
    if input.visible_window_count == Some(0) {
        attention.push(ComputerUseReadinessAttention {
            code: "no_visible_windows",
            message: "No third-party windows are visible for Computer Use.",
        });
    }
    if input.last_error.is_some() {
        attention.push(ComputerUseReadinessAttention {
            code: "last_error_present",
            message: "A previous Computer Use attempt recorded an error.",
        });
    }

    let ready = attention.is_empty() && input.visible_window_count.is_some();

    ComputerUseReadinessReceipt {
        schema_version: COMPUTER_USE_READINESS_SCHEMA_VERSION,
        ready,
        mode: "readOnlyPreflight",
        attention,
        enabled: input.enabled,
        accessibility: input.accessibility,
        screen_recording: input.screen_recording,
        keyboard_backend_available: input.keyboard_backend_available,
        visible_window_count: input.visible_window_count,
        focused_target: input.focused_target,
        last_error: input.last_error,
        redacted: true,
    }
}

pub fn current_computer_use_readiness_receipt() -> ComputerUseReadinessReceipt {
    build_computer_use_readiness_receipt(ComputerUseReadinessInput {
        enabled: true,
        accessibility: crate::platform::permiso_detect::ax_is_trusted(),
        screen_recording: crate::platform::permiso_detect::screen_capture_authorized(),
        keyboard_backend_available: true,
        visible_window_count: None,
        focused_target: None,
        last_error: None,
    })
}
