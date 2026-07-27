//! Per-shell user-resize policy — the single decision point for which window
//! shells the user may resize and their size constraints.
//!
//! Contract (Oracle session `floating-buttons-window-resize`, 2026-07-26):
//! - Resizability belongs to the WINDOW SHELL, not the content currently
//!   occupying it. Embedded surfaces (Agent Chat hosted inside Notes) are NOT
//!   shell kinds; they inherit the host shell's policy.
//! - The match below is exhaustive with no wildcard arm so every future shell
//!   must choose a policy at compile time (enforcement ladder rung 1).

/// Every top-level window shell the app creates. Embedded content must not
/// appear here — Agent Chat inside Notes inherits [`WindowShellKind::Notes`],
/// and Agent Chat inside the main launcher inherits
/// [`WindowShellKind::MainLauncher`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // variants are wired up as their owners migrate
pub(crate) enum WindowShellKind {
    MainLauncher,
    Notes,
    DetachedAgentChat,
    Dictation,
    ActionsPopup,
    ConfirmPopup,
    InlinePopup,
    Hud,
}

/// User-resize capability + content-size constraints for one shell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WindowResizePolicy {
    pub user_resizable: bool,
    pub min_content_width: f64,
    pub min_content_height: f64,
    /// `None` = no product maximum; the visible display clamps naturally.
    pub max_content_width: Option<f64>,
    pub max_content_height: Option<f64>,
}

impl WindowResizePolicy {
    /// A shell locked to one app-owned size (the main launcher).
    pub(crate) const fn fixed(width: f64, height: f64) -> Self {
        Self {
            user_resizable: false,
            min_content_width: width,
            min_content_height: height,
            max_content_width: Some(width),
            max_content_height: Some(height),
        }
    }

    /// A transient, content-driven shell (popups, HUD, dictation): never
    /// user-resizable, no app-authored constraints.
    pub(crate) const fn content_sized() -> Self {
        Self {
            user_resizable: false,
            min_content_width: 0.0,
            min_content_height: 0.0,
            max_content_width: None,
            max_content_height: None,
        }
    }
}

/// The one decision point mapping a shell to its resize policy.
///
/// Minimums for Notes come from the proven default geometry
/// (`notes::window::contract::NOTES_DEFAULT_WIDTH/HEIGHT`); do not raise them
/// speculatively — the Agent footer must degrade responsively at 350pt first
/// (see the Notes chrome contract).
pub(crate) const fn resize_policy(kind: WindowShellKind) -> WindowResizePolicy {
    match kind {
        WindowShellKind::MainLauncher => WindowResizePolicy::fixed(
            super::MAIN_WINDOW_WIDTH as f64,
            super::MAIN_WINDOW_MIN_HEIGHT as f64,
        ),
        WindowShellKind::Notes => WindowResizePolicy {
            user_resizable: true,
            min_content_width: crate::notes::window::contract::NOTES_DEFAULT_WIDTH as f64,
            min_content_height: crate::notes::window::contract::NOTES_DEFAULT_HEIGHT as f64,
            max_content_width: None,
            max_content_height: None,
        },
        WindowShellKind::DetachedAgentChat => WindowResizePolicy {
            user_resizable: true,
            // Shared with the Notes shell until measured footer/layout proof
            // motivates a different derived minimum.
            min_content_width: crate::notes::window::contract::NOTES_DEFAULT_WIDTH as f64,
            min_content_height: crate::notes::window::contract::NOTES_DEFAULT_HEIGHT as f64,
            max_content_width: None,
            max_content_height: None,
        },
        WindowShellKind::Dictation
        | WindowShellKind::ActionsPopup
        | WindowShellKind::ConfirmPopup
        | WindowShellKind::InlinePopup
        | WindowShellKind::Hud => WindowResizePolicy::content_sized(),
    }
}

/// Sanitize a restored (persisted) content size against a shell policy:
/// non-finite / non-positive dimensions fall back to the policy minimum, and
/// the result is clamped into `[min, max]` (max only when the policy has one
/// — display clamping is the persistence layer's job).
pub(crate) fn clamp_restored_content_size(
    width: f64,
    height: f64,
    policy: &WindowResizePolicy,
) -> (f64, f64) {
    let sanitize = |value: f64, minimum: f64, maximum: Option<f64>| {
        let value = if value.is_finite() && value > 0.0 {
            value
        } else {
            minimum
        };
        let value = value.max(minimum);
        match maximum {
            Some(maximum) => value.min(maximum),
            None => value,
        }
    };
    (
        sanitize(width, policy.min_content_width, policy.max_content_width),
        sanitize(height, policy.min_content_height, policy.max_content_height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_shell_is_user_resizable() {
        let policy = resize_policy(WindowShellKind::Notes);
        assert!(policy.user_resizable);
        assert_eq!(policy.min_content_width, 350.0);
        assert_eq!(policy.min_content_height, 280.0);
        // Manual resize must NOT be capped by the 600pt auto-size ceiling.
        assert_eq!(policy.max_content_width, None);
        assert_eq!(policy.max_content_height, None);
    }

    #[test]
    fn main_launcher_remains_fixed() {
        let policy = resize_policy(WindowShellKind::MainLauncher);
        assert!(!policy.user_resizable);
        assert_eq!(policy.min_content_width, 750.0);
        assert_eq!(policy.min_content_height, 480.0);
        assert_eq!(policy.max_content_width, Some(750.0));
        assert_eq!(policy.max_content_height, Some(480.0));
    }

    #[test]
    fn detached_agent_chat_shell_is_user_resizable() {
        let policy = resize_policy(WindowShellKind::DetachedAgentChat);
        assert!(policy.user_resizable);
        assert_eq!(policy.max_content_height, None);
    }

    #[test]
    fn persisted_notes_bounds_are_clamped_to_resize_policy() {
        let policy = resize_policy(WindowShellKind::Notes);

        // Below-minimum restored bounds clamp up to the policy minimum.
        assert_eq!(
            clamp_restored_content_size(100.0, 50.0, &policy),
            (350.0, 280.0)
        );
        // Non-finite / non-positive values sanitize to the minimum.
        assert_eq!(
            clamp_restored_content_size(f64::NAN, -20.0, &policy),
            (350.0, 280.0)
        );
        // A legitimate user-chosen size — including one far beyond the 600pt
        // auto-size ceiling — restores unchanged (no product maximum).
        assert_eq!(
            clamp_restored_content_size(900.0, 1200.0, &policy),
            (900.0, 1200.0)
        );

        // Fixed shells clamp both directions to their single size.
        let launcher = resize_policy(WindowShellKind::MainLauncher);
        assert_eq!(
            clamp_restored_content_size(900.0, 1200.0, &launcher),
            (750.0, 480.0)
        );
    }

    #[test]
    fn transient_window_shells_remain_content_sized() {
        for kind in [
            WindowShellKind::Dictation,
            WindowShellKind::ActionsPopup,
            WindowShellKind::ConfirmPopup,
            WindowShellKind::InlinePopup,
            WindowShellKind::Hud,
        ] {
            let policy = resize_policy(kind);
            assert!(!policy.user_resizable, "{kind:?} must not be resizable");
            assert_eq!(policy, WindowResizePolicy::content_sized());
        }
    }
}
