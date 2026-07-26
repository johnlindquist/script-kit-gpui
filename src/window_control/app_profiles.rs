//! Per-app mutation profiles.
//!
//! Profiles change ONLY write ordering, retry pacing, and verification
//! tolerance. They never change capabilities and never bypass verification.
//! The table is locked by the plan (window-engine-foundation); the
//! profile-contradiction decision rule governs any future change: a sequence
//! may be replaced only by one that verifies 100/100 live cycles with a
//! tolerance no higher than the table's value.

use std::path::Path;
use std::time::Duration;

/// Order of AX writes for a bounds mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundsMutationSequence {
    PositionThenSize,
    SizeThenPosition,
    PositionSizePosition,
    SizePositionSize,
}

/// Verification tolerance in points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundsTolerance {
    pub position: i32,
    pub size: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppMutationProfile {
    pub sequence: BoundsMutationSequence,
    /// Sleep between a failed readback and the next attempt (never before
    /// the first readback).
    pub retry_settle_delay: Duration,
    pub max_attempts: u8,
    pub tolerance: BoundsTolerance,
}

const DEFAULT_PROFILE: AppMutationProfile = AppMutationProfile {
    sequence: BoundsMutationSequence::PositionThenSize,
    retry_settle_delay: Duration::from_millis(10),
    max_attempts: 2,
    tolerance: BoundsTolerance {
        position: 2,
        size: 2,
    },
};

const ELECTRON_PROFILE: AppMutationProfile = AppMutationProfile {
    sequence: BoundsMutationSequence::PositionSizePosition,
    retry_settle_delay: Duration::from_millis(30),
    max_attempts: 3,
    tolerance: BoundsTolerance {
        position: 2,
        size: 2,
    },
};

/// The locked per-bundle profile table.
fn profile_for_bundle_id(bundle_id: &str) -> Option<AppMutationProfile> {
    Some(match bundle_id {
        "com.google.Chrome" | "company.thebrowser.Browser" => AppMutationProfile {
            sequence: BoundsMutationSequence::PositionSizePosition,
            retry_settle_delay: Duration::from_millis(40),
            max_attempts: 3,
            tolerance: BoundsTolerance {
                position: 2,
                size: 2,
            },
        },
        "com.tinyspeck.slackmacgap" | "com.microsoft.VSCode" => AppMutationProfile {
            sequence: BoundsMutationSequence::PositionSizePosition,
            retry_settle_delay: Duration::from_millis(30),
            max_attempts: 3,
            tolerance: BoundsTolerance {
                position: 2,
                size: 2,
            },
        },
        "com.apple.finder" => AppMutationProfile {
            sequence: BoundsMutationSequence::PositionThenSize,
            retry_settle_delay: Duration::from_millis(10),
            max_attempts: 2,
            tolerance: BoundsTolerance {
                position: 2,
                size: 2,
            },
        },
        "com.apple.Terminal" => AppMutationProfile {
            sequence: BoundsMutationSequence::SizeThenPosition,
            retry_settle_delay: Duration::from_millis(20),
            max_attempts: 2,
            tolerance: BoundsTolerance {
                position: 2,
                // Terminal resizes in character-cell increments.
                size: 12,
            },
        },
        _ => return None,
    })
}

/// Generic Electron detection (decision rule: filesystem check performed
/// during OBSERVATION, cached — never in a hot transaction path).
pub(super) fn is_electron_app(app_path: Option<&Path>) -> bool {
    app_path.is_some_and(|path| {
        path.join("Contents/Frameworks/Electron Framework.framework")
            .is_dir()
    })
}

/// Resolve the profile for an app.
pub(super) fn resolve_profile(
    bundle_id: Option<&str>,
    electron_detected: bool,
) -> AppMutationProfile {
    if let Some(profile) = bundle_id.and_then(profile_for_bundle_id) {
        return profile;
    }
    if electron_detected {
        return ELECTRON_PROFILE;
    }
    DEFAULT_PROFILE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locked_bundle_mapping_matches_the_table() {
        let chrome = resolve_profile(Some("com.google.Chrome"), false);
        assert_eq!(
            chrome.sequence,
            BoundsMutationSequence::PositionSizePosition
        );
        assert_eq!(chrome.retry_settle_delay, Duration::from_millis(40));
        assert_eq!(chrome.max_attempts, 3);

        let arc = resolve_profile(Some("company.thebrowser.Browser"), false);
        assert_eq!(arc, chrome);

        let slack = resolve_profile(Some("com.tinyspeck.slackmacgap"), false);
        assert_eq!(slack.retry_settle_delay, Duration::from_millis(30));
        let vscode = resolve_profile(Some("com.microsoft.VSCode"), false);
        assert_eq!(vscode, slack);

        let finder = resolve_profile(Some("com.apple.finder"), false);
        assert_eq!(finder.sequence, BoundsMutationSequence::PositionThenSize);
        assert_eq!(finder.max_attempts, 2);

        let terminal = resolve_profile(Some("com.apple.Terminal"), false);
        assert_eq!(terminal.sequence, BoundsMutationSequence::SizeThenPosition);
        assert_eq!(terminal.tolerance.size, 12);
        assert_eq!(terminal.tolerance.position, 2);
    }

    #[test]
    fn electron_detection_selects_the_electron_profile() {
        let electron = resolve_profile(Some("com.example.someelectron"), true);
        assert_eq!(
            electron.sequence,
            BoundsMutationSequence::PositionSizePosition
        );
        assert_eq!(electron.retry_settle_delay, Duration::from_millis(30));
    }

    #[test]
    fn unknown_apps_use_the_default_profile() {
        let unknown = resolve_profile(Some("com.example.unknown"), false);
        assert_eq!(unknown, DEFAULT_PROFILE);
        let none = resolve_profile(None, false);
        assert_eq!(none, DEFAULT_PROFILE);
    }

    #[test]
    fn known_bundle_beats_electron_detection() {
        // Slack IS Electron, but the exact bundle mapping wins.
        let slack = resolve_profile(Some("com.tinyspeck.slackmacgap"), true);
        assert_eq!(slack.retry_settle_delay, Duration::from_millis(30));
        assert_eq!(slack.max_attempts, 3);
    }

    #[test]
    fn electron_filesystem_probe_is_path_based() {
        assert!(!is_electron_app(None));
        assert!(!is_electron_app(Some(Path::new("/nonexistent/App.app"))));
    }
}
