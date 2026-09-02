//! GitHub-releases-backed update checker for Script Kit.
//!
//! This module deliberately stops at a manifest-backed, check-only decision. It
//! does not download, stage, install, or restart the app.

use semver::Version;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

const RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/johnlindquist/script-kit-next/releases/latest";
const USER_AGENT: &str = "script-kit-gpui";
const EXPECTED_MACOS_ASSET: &str = "Script-Kit-macos.zip";
const EXPECTED_MANIFEST_ASSET: &str = "release-manifest.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckKind {
    Automatic,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedArtifact {
    pub name: String,
    pub download_url: String,
    pub size_bytes: Option<u64>,
    pub sha256: String,
    pub github_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRelease {
    pub version: Version,
    pub tag: String,
    pub release_page_url: String,
    pub manifest_url: String,
    pub artifact: VerifiedArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseNotReadyReason {
    MissingExpectedArtifact,
    MissingManifest,
    ManifestFetchFailed,
    ManifestInvalid,
    ManifestMissingArtifactHash,
    ManifestHashInvalid,
    ManifestHashMismatch,
    AssetDigestMismatch,
    NonHttpsAssetUrl,
    NonHttpsManifestUrl,
}

impl ReleaseNotReadyReason {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::MissingExpectedArtifact => "expected macOS asset is missing",
            Self::MissingManifest => "release manifest is missing",
            Self::ManifestFetchFailed => "release manifest could not be fetched",
            Self::ManifestInvalid => "release manifest is invalid",
            Self::ManifestMissingArtifactHash => "release manifest is missing the app hash",
            Self::ManifestHashInvalid => "release manifest hash is invalid",
            Self::ManifestHashMismatch => "release manifest does not match the asset",
            Self::AssetDigestMismatch => "GitHub asset digest does not match the manifest",
            Self::NonHttpsAssetUrl => "asset URL is not HTTPS",
            Self::NonHttpsManifestUrl => "manifest URL is not HTTPS",
        }
    }

    fn is_transient(&self) -> bool {
        matches!(self, Self::ManifestFetchFailed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateFailure {
    PolicyRefused(crate::runtime_policy::EffectRefusal),
    InvalidCurrentVersion,
    InvalidReleaseTag,
    GitHubRateLimited,
    GitHubApiUnavailable,
    Network,
    Timeout,
    InvalidResponse,
}

impl UpdateFailure {
    fn message(&self) -> &'static str {
        match self {
            Self::PolicyRefused(refusal) => refusal.code,
            Self::InvalidCurrentVersion => "invalid current app version",
            Self::InvalidReleaseTag => "invalid release tag",
            Self::GitHubRateLimited => "GitHub rate limited the update check",
            Self::GitHubApiUnavailable => "GitHub release API is unavailable",
            Self::Network => "network request failed",
            Self::Timeout => "update check timed out",
            Self::InvalidResponse => "GitHub returned an invalid response",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    UpToDate,
    Available {
        release: VerifiedRelease,
    },
    ReleaseNotReady {
        version: Version,
        tag: String,
        release_page_url: String,
        reason: ReleaseNotReadyReason,
    },
    Failed(UpdateFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking {
        kind: CheckKind,
    },
    UpToDate,
    Available {
        release: VerifiedRelease,
    },
    ReleaseNotReady {
        version: String,
        release_page_url: String,
        reason: ReleaseNotReadyReason,
    },
    Error {
        message: String,
        failure: UpdateFailure,
    },
}

impl UpdateState {
    pub fn release_page_url(&self) -> Option<&str> {
        match self {
            UpdateState::Available { release } => Some(release.release_page_url.as_str()),
            UpdateState::ReleaseNotReady {
                release_page_url, ..
            } => Some(release_page_url.as_str()),
            _ => None,
        }
    }

    pub fn release_url(&self) -> Option<&str> {
        // Compatibility alias. This intentionally returns the GitHub release
        // page, never the asset download URL.
        self.release_page_url()
    }

    pub fn is_checking(&self) -> bool {
        matches!(self, UpdateState::Checking { .. })
    }
}

/// Spawn a background HTTP fetch and update `state` when the response arrives.
/// Calls `on_complete` once after the shared state has settled. Refusal completes
/// synchronously without a worker; permitted checks complete on a worker thread.
pub fn check_now<F>(state: Arc<RwLock<UpdateState>>, kind: CheckKind, on_complete: F)
where
    F: FnOnce() + Send + 'static,
{
    if let Err(refusal) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
    {
        if let Ok(mut guard) = state.write() {
            *guard = UpdateState::Error {
                message: refusal.to_string(),
                failure: UpdateFailure::PolicyRefused(refusal),
            };
        }
        on_complete();
        return;
    }

    let previous = {
        let Ok(mut guard) = state.write() else {
            on_complete();
            return;
        };

        if let UpdateState::Checking { kind: in_flight } = *guard {
            if matches!(kind, CheckKind::Manual) && matches!(in_flight, CheckKind::Automatic) {
                *guard = UpdateState::Checking {
                    kind: CheckKind::Manual,
                };
            }
            let state_for_wait = state.clone();
            thread::spawn(move || {
                wait_until_check_settled(&state_for_wait);
                on_complete();
            });
            return;
        }

        let previous = guard.clone();
        *guard = UpdateState::Checking { kind };
        previous
    };

    thread::spawn(move || {
        let decision = fetch_latest_decision();
        let effective_kind = current_check_kind(&state).unwrap_or(kind);
        let next = decision_to_state(decision, effective_kind, previous);
        if let Ok(mut guard) = state.write() {
            *guard = next;
        }
        on_complete();
    });
}

fn wait_until_check_settled(state: &Arc<RwLock<UpdateState>>) {
    loop {
        let settled = state
            .read()
            .map(|guard| !guard.is_checking())
            .unwrap_or(true);
        if settled {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn current_check_kind(state: &Arc<RwLock<UpdateState>>) -> Option<CheckKind> {
    state.read().ok().and_then(|guard| match *guard {
        UpdateState::Checking { kind } => Some(kind),
        _ => None,
    })
}

fn fetch_latest_decision() -> UpdateDecision {
    if let Err(refusal) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
    {
        return UpdateDecision::Failed(UpdateFailure::PolicyRefused(refusal));
    }

    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(10)))
        .timeout_connect(Some(Duration::from_secs(5)))
        .build()
        .new_agent();

    let release = match fetch_json(&agent, RELEASES_LATEST_URL) {
        Ok(json) => json,
        Err(failure) => return UpdateDecision::Failed(failure),
    };

    let current = env!("CARGO_PKG_VERSION");
    let preliminary = decide_release(current, &release, None);
    let UpdateDecision::ReleaseNotReady {
        version,
        tag,
        release_page_url,
        reason: ReleaseNotReadyReason::MissingManifest,
    } = preliminary
    else {
        return preliminary;
    };

    let Some(manifest_url) = select_manifest_asset(&release)
        .and_then(|asset| asset.get("browser_download_url"))
        .and_then(|url| url.as_str())
    else {
        return UpdateDecision::ReleaseNotReady {
            version,
            tag,
            release_page_url,
            reason: ReleaseNotReadyReason::MissingManifest,
        };
    };

    if !is_https_url(manifest_url) {
        return UpdateDecision::ReleaseNotReady {
            version,
            tag,
            release_page_url,
            reason: ReleaseNotReadyReason::NonHttpsManifestUrl,
        };
    }

    let manifest = match fetch_json(&agent, manifest_url) {
        Ok(json) => json,
        Err(failure @ UpdateFailure::PolicyRefused(_)) => return UpdateDecision::Failed(failure),
        Err(_) => {
            return UpdateDecision::ReleaseNotReady {
                version,
                tag,
                release_page_url,
                reason: ReleaseNotReadyReason::ManifestFetchFailed,
            };
        }
    };

    decide_release(current, &release, Some(&manifest))
}

fn fetch_json(agent: &ureq::Agent, url: &str) -> Result<serde_json::Value, UpdateFailure> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
        .map_err(UpdateFailure::PolicyRefused)?;
    let response = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call();

    let mut response = response.map_err(map_ureq_error)?;
    response
        .body_mut()
        .read_json()
        .map_err(|_| UpdateFailure::InvalidResponse)
}

fn map_ureq_error(error: ureq::Error) -> UpdateFailure {
    match error {
        ureq::Error::StatusCode(403 | 429) => UpdateFailure::GitHubRateLimited,
        ureq::Error::StatusCode(status) if status >= 500 => UpdateFailure::GitHubApiUnavailable,
        ureq::Error::Timeout(_) => UpdateFailure::Timeout,
        _ => UpdateFailure::Network,
    }
}

fn decision_to_state(
    decision: UpdateDecision,
    kind: CheckKind,
    previous: UpdateState,
) -> UpdateState {
    match decision {
        UpdateDecision::UpToDate => UpdateState::UpToDate,
        UpdateDecision::Available { release } => UpdateState::Available { release },
        UpdateDecision::ReleaseNotReady {
            version,
            release_page_url,
            reason,
            ..
        } => {
            if reason.is_transient() {
                match kind {
                    CheckKind::Automatic => previous,
                    CheckKind::Manual => UpdateState::Error {
                        message: reason.label().to_string(),
                        failure: UpdateFailure::Network,
                    },
                }
            } else {
                UpdateState::ReleaseNotReady {
                    version: version.to_string(),
                    release_page_url,
                    reason,
                }
            }
        }
        UpdateDecision::Failed(failure) => {
            if matches!(kind, CheckKind::Automatic)
                && !matches!(failure, UpdateFailure::PolicyRefused(_))
            {
                previous
            } else {
                UpdateState::Error {
                    message: failure.message().to_string(),
                    failure,
                }
            }
        }
    }
}

fn decide_release(
    current: &str,
    release: &serde_json::Value,
    manifest: Option<&serde_json::Value>,
) -> UpdateDecision {
    let current_version = match parse_version_tag(current) {
        Ok(version) => version,
        Err(_) => return UpdateDecision::Failed(UpdateFailure::InvalidCurrentVersion),
    };

    let Some(raw_tag) = release.get("tag_name").and_then(|v| v.as_str()) else {
        return UpdateDecision::Failed(UpdateFailure::InvalidReleaseTag);
    };
    let release_version = match parse_version_tag(raw_tag) {
        Ok(version) => version,
        Err(_) => return UpdateDecision::Failed(UpdateFailure::InvalidReleaseTag),
    };

    if release_version <= current_version {
        return UpdateDecision::UpToDate;
    }

    let release_page_url = release
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !is_https_url(&release_page_url) {
        return UpdateDecision::Failed(UpdateFailure::InvalidResponse);
    }

    let Some(asset) = select_expected_artifact(release) else {
        return release_not_ready(
            release_version,
            raw_tag,
            release_page_url,
            ReleaseNotReadyReason::MissingExpectedArtifact,
        );
    };

    let Some(download_url) = asset.get("browser_download_url").and_then(|v| v.as_str()) else {
        return release_not_ready(
            release_version,
            raw_tag,
            release_page_url,
            ReleaseNotReadyReason::NonHttpsAssetUrl,
        );
    };
    if !is_https_url(download_url) {
        return release_not_ready(
            release_version,
            raw_tag,
            release_page_url,
            ReleaseNotReadyReason::NonHttpsAssetUrl,
        );
    }

    let Some(manifest_asset) = select_manifest_asset(release) else {
        return release_not_ready(
            release_version,
            raw_tag,
            release_page_url,
            ReleaseNotReadyReason::MissingManifest,
        );
    };
    let Some(manifest_url) = manifest_asset
        .get("browser_download_url")
        .and_then(|v| v.as_str())
    else {
        return release_not_ready(
            release_version,
            raw_tag,
            release_page_url,
            ReleaseNotReadyReason::NonHttpsManifestUrl,
        );
    };
    if !is_https_url(manifest_url) {
        return release_not_ready(
            release_version,
            raw_tag,
            release_page_url,
            ReleaseNotReadyReason::NonHttpsManifestUrl,
        );
    }

    let Some(manifest) = manifest else {
        return release_not_ready(
            release_version,
            raw_tag,
            release_page_url,
            ReleaseNotReadyReason::MissingManifest,
        );
    };

    let sha256 = match validate_manifest_for_artifact(manifest, release, asset) {
        Ok(sha256) => sha256,
        Err(reason) => {
            return release_not_ready(release_version, raw_tag, release_page_url, reason);
        }
    };

    UpdateDecision::Available {
        release: VerifiedRelease {
            version: release_version,
            tag: raw_tag.to_string(),
            release_page_url,
            manifest_url: manifest_url.to_string(),
            artifact: VerifiedArtifact {
                name: EXPECTED_MACOS_ASSET.to_string(),
                download_url: download_url.to_string(),
                size_bytes: asset.get("size").and_then(|v| v.as_u64()),
                sha256,
                github_digest: asset
                    .get("digest")
                    .and_then(|v| v.as_str())
                    .map(|digest| digest.to_string()),
            },
        },
    }
}

fn release_not_ready(
    version: Version,
    tag: &str,
    release_page_url: String,
    reason: ReleaseNotReadyReason,
) -> UpdateDecision {
    UpdateDecision::ReleaseNotReady {
        version,
        tag: tag.to_string(),
        release_page_url,
        reason,
    }
}

fn parse_version_tag(tag: &str) -> Result<Version, UpdateFailure> {
    Version::parse(tag.trim().trim_start_matches('v')).map_err(|_| UpdateFailure::InvalidReleaseTag)
}

fn select_expected_artifact(release: &serde_json::Value) -> Option<&serde_json::Value> {
    release
        .get("assets")?
        .as_array()?
        .iter()
        .find(|asset| asset.get("name").and_then(|v| v.as_str()) == Some(EXPECTED_MACOS_ASSET))
}

fn select_manifest_asset(release: &serde_json::Value) -> Option<&serde_json::Value> {
    release
        .get("assets")?
        .as_array()?
        .iter()
        .find(|asset| asset.get("name").and_then(|v| v.as_str()) == Some(EXPECTED_MANIFEST_ASSET))
}

fn validate_manifest_for_artifact(
    manifest: &serde_json::Value,
    release: &serde_json::Value,
    asset: &serde_json::Value,
) -> Result<String, ReleaseNotReadyReason> {
    let release_tag = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or(ReleaseNotReadyReason::ManifestInvalid)?;
    let release_version = parse_version_tag(release_tag)
        .map_err(|_| ReleaseNotReadyReason::ManifestInvalid)?
        .to_string();

    if manifest.get("tag").and_then(|v| v.as_str()) != Some(release_tag) {
        return Err(ReleaseNotReadyReason::ManifestInvalid);
    }
    if manifest.get("version").and_then(|v| v.as_str()) != Some(release_version.as_str()) {
        return Err(ReleaseNotReadyReason::ManifestInvalid);
    }

    let artifact = manifest
        .get("artifacts")
        .and_then(|v| v.as_array())
        .ok_or(ReleaseNotReadyReason::ManifestInvalid)?
        .iter()
        .find(|artifact| {
            artifact.get("name").and_then(|v| v.as_str()) == Some(EXPECTED_MACOS_ASSET)
                && artifact.get("platform").and_then(|v| v.as_str()) == Some("macos")
        })
        .ok_or(ReleaseNotReadyReason::ManifestMissingArtifactHash)?;

    let sha256 = artifact
        .get("sha256")
        .and_then(|v| v.as_str())
        .ok_or(ReleaseNotReadyReason::ManifestMissingArtifactHash)?;
    if !is_sha256_hex(sha256) {
        return Err(ReleaseNotReadyReason::ManifestHashInvalid);
    }

    let manifest_size = artifact
        .get("size_bytes")
        .and_then(|v| v.as_u64())
        .ok_or(ReleaseNotReadyReason::ManifestInvalid)?;
    if let Some(asset_size) = asset.get("size").and_then(|v| v.as_u64()) {
        if asset_size != manifest_size {
            return Err(ReleaseNotReadyReason::ManifestHashMismatch);
        }
    }

    if let Some(digest) = asset.get("digest").and_then(|v| v.as_str()) {
        let expected = format!("sha256:{sha256}");
        if !digest.eq_ignore_ascii_case(&expected) {
            return Err(ReleaseNotReadyReason::AssetDigestMismatch);
        }
    }

    Ok(sha256.to_string())
}

fn is_https_url(url: &str) -> bool {
    url.starts_with("https://")
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};

    const SHA: &str = "022689519147819b7eb0ef2dba102e03677e53eb550eddd5f3b10f78aa5d3427";

    fn release_with_assets(assets: serde_json::Value) -> serde_json::Value {
        json!({
            "tag_name": "v9.9.9",
            "html_url": "https://example.com/releases/v9.9.9",
            "assets": assets
        })
    }

    fn expected_asset() -> serde_json::Value {
        json!({
            "name": EXPECTED_MACOS_ASSET,
            "browser_download_url": "https://example.com/downloads/Script-Kit-macos.zip",
            "size": 123,
            "digest": format!("sha256:{SHA}")
        })
    }

    fn manifest() -> serde_json::Value {
        json!({
            "version": "9.9.9",
            "tag": "v9.9.9",
            "artifacts": [{
                "name": EXPECTED_MACOS_ASSET,
                "platform": "macos",
                "sha256": SHA,
                "size_bytes": 123
            }]
        })
    }

    fn manifest_asset() -> serde_json::Value {
        json!({
            "name": EXPECTED_MANIFEST_ASSET,
            "browser_download_url": "https://example.com/downloads/release-manifest.json"
        })
    }

    fn verified_release() -> serde_json::Value {
        release_with_assets(json!([expected_asset(), manifest_asset()]))
    }

    #[test]
    fn parses_versions_with_optional_v_prefix() {
        assert_eq!(
            parse_version_tag("v1.2.3").unwrap(),
            Version::parse("1.2.3").unwrap()
        );
        assert_eq!(
            parse_version_tag("1.2.3").unwrap(),
            Version::parse("1.2.3").unwrap()
        );
        assert!(parse_version_tag("v1.2.nope").is_err());
    }

    #[test]
    fn semver_build_metadata_does_not_create_update() {
        let release = release_with_assets(json!([]));
        assert_eq!(
            decide_release("9.9.9+local", &release, None),
            UpdateDecision::UpToDate
        );
    }

    #[test]
    fn same_or_older_release_is_up_to_date_without_assets() {
        let release = json!({
            "tag_name": "v1.0.0",
            "html_url": "https://example.com/releases/v1.0.0",
            "assets": []
        });
        assert_eq!(
            decide_release("1.0.0", &release, None),
            UpdateDecision::UpToDate
        );
        assert_eq!(
            decide_release("1.0.1", &release, None),
            UpdateDecision::UpToDate
        );
    }

    #[test]
    fn exact_expected_asset_is_required() {
        let release = release_with_assets(json!([
            {
                "name": "Random.zip",
                "browser_download_url": "https://example.com/downloads/Random.zip"
            },
            manifest_asset()
        ]));

        assert!(matches!(
            decide_release("1.0.0", &release, Some(&manifest())),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::MissingExpectedArtifact,
                ..
            }
        ));
    }

    #[test]
    fn missing_manifest_is_not_up_to_date() {
        let release = release_with_assets(json!([expected_asset()]));
        assert!(matches!(
            decide_release("1.0.0", &release, None),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::MissingManifest,
                ..
            }
        ));
    }

    #[test]
    fn non_https_asset_url_is_rejected() {
        let release = release_with_assets(json!([
            {
                "name": EXPECTED_MACOS_ASSET,
                "browser_download_url": "http://example.com/downloads/Script-Kit-macos.zip",
                "size": 123
            },
            manifest_asset()
        ]));

        assert!(matches!(
            decide_release("1.0.0", &release, Some(&manifest())),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::NonHttpsAssetUrl,
                ..
            }
        ));
    }

    #[test]
    fn non_https_manifest_url_is_rejected() {
        let release = release_with_assets(json!([
            expected_asset(),
            {
                "name": EXPECTED_MANIFEST_ASSET,
                "browser_download_url": "http://example.com/downloads/release-manifest.json"
            }
        ]));

        assert!(matches!(
            decide_release("1.0.0", &release, Some(&manifest())),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::NonHttpsManifestUrl,
                ..
            }
        ));
    }

    #[test]
    fn verified_manifest_produces_available_release_page_decision() {
        let decision = decide_release("1.0.0", &verified_release(), Some(&manifest()));
        let UpdateDecision::Available { release } = decision else {
            panic!("expected available decision");
        };
        assert_eq!(release.version, Version::parse("9.9.9").unwrap());
        assert_eq!(
            release.release_page_url,
            "https://example.com/releases/v9.9.9"
        );
        assert_eq!(
            release.artifact.download_url,
            "https://example.com/downloads/Script-Kit-macos.zip"
        );
        assert_ne!(release.release_page_url, release.artifact.download_url);
    }

    #[test]
    fn manifest_tag_or_version_mismatch_is_invalid() {
        let bad_tag = json!({
            "version": "9.9.9",
            "tag": "v9.9.8",
            "artifacts": [{
                "name": EXPECTED_MACOS_ASSET,
                "platform": "macos",
                "sha256": SHA,
                "size_bytes": 123
            }]
        });
        assert!(matches!(
            decide_release("1.0.0", &verified_release(), Some(&bad_tag)),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::ManifestInvalid,
                ..
            }
        ));
    }

    #[test]
    fn manifest_missing_hash_is_rejected() {
        let bad = json!({
            "version": "9.9.9",
            "tag": "v9.9.9",
            "artifacts": [{
                "name": EXPECTED_MACOS_ASSET,
                "platform": "macos",
                "size_bytes": 123
            }]
        });
        assert!(matches!(
            decide_release("1.0.0", &verified_release(), Some(&bad)),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::ManifestMissingArtifactHash,
                ..
            }
        ));
    }

    #[test]
    fn manifest_invalid_hash_is_rejected() {
        let bad = json!({
            "version": "9.9.9",
            "tag": "v9.9.9",
            "artifacts": [{
                "name": EXPECTED_MACOS_ASSET,
                "platform": "macos",
                "sha256": "deadbeef",
                "size_bytes": 123
            }]
        });
        assert!(matches!(
            decide_release("1.0.0", &verified_release(), Some(&bad)),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::ManifestHashInvalid,
                ..
            }
        ));
    }

    #[test]
    fn manifest_size_mismatch_is_rejected() {
        let bad = json!({
            "version": "9.9.9",
            "tag": "v9.9.9",
            "artifacts": [{
                "name": EXPECTED_MACOS_ASSET,
                "platform": "macos",
                "sha256": SHA,
                "size_bytes": 999
            }]
        });
        assert!(matches!(
            decide_release("1.0.0", &verified_release(), Some(&bad)),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::ManifestHashMismatch,
                ..
            }
        ));
    }

    #[test]
    fn github_digest_mismatch_is_rejected() {
        let release = release_with_assets(json!([
            {
                "name": EXPECTED_MACOS_ASSET,
                "browser_download_url": "https://example.com/downloads/Script-Kit-macos.zip",
                "size": 123,
                "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            manifest_asset()
        ]));
        assert!(matches!(
            decide_release("1.0.0", &release, Some(&manifest())),
            UpdateDecision::ReleaseNotReady {
                reason: ReleaseNotReadyReason::AssetDigestMismatch,
                ..
            }
        ));
    }

    #[test]
    fn automatic_failure_restores_previous_state() {
        let previous = UpdateState::UpToDate;
        assert_eq!(
            decision_to_state(
                UpdateDecision::Failed(UpdateFailure::Network),
                CheckKind::Automatic,
                previous.clone()
            ),
            previous
        );
    }

    #[test]
    fn manual_failure_surfaces_error() {
        let state = decision_to_state(
            UpdateDecision::Failed(UpdateFailure::Network),
            CheckKind::Manual,
            UpdateState::Idle,
        );
        assert!(matches!(
            state,
            UpdateState::Error {
                failure: UpdateFailure::Network,
                ..
            }
        ));
    }

    #[test]
    fn policy_refusal_surfaces_error_for_every_check_kind() {
        let refusal = crate::runtime_policy::EffectRefusal {
            code: crate::runtime_policy::ExternalEffect::Provider.code(),
        };
        for kind in [CheckKind::Automatic, CheckKind::Manual] {
            assert_eq!(
                decision_to_state(
                    UpdateDecision::Failed(UpdateFailure::PolicyRefused(refusal)),
                    kind,
                    UpdateState::UpToDate,
                ),
                UpdateState::Error {
                    message: refusal.to_string(),
                    failure: UpdateFailure::PolicyRefused(refusal),
                },
            );
        }
    }

    #[test]
    fn automatic_manifest_fetch_failure_restores_previous_state() {
        let previous = UpdateState::UpToDate;
        assert_eq!(
            decision_to_state(
                UpdateDecision::ReleaseNotReady {
                    version: Version::parse("9.9.9").unwrap(),
                    tag: "v9.9.9".to_string(),
                    release_page_url: "https://example.com/releases/v9.9.9".to_string(),
                    reason: ReleaseNotReadyReason::ManifestFetchFailed,
                },
                CheckKind::Automatic,
                previous.clone(),
            ),
            previous
        );
    }

    #[test]
    fn manual_manifest_fetch_failure_surfaces_error() {
        let state = decision_to_state(
            UpdateDecision::ReleaseNotReady {
                version: Version::parse("9.9.9").unwrap(),
                tag: "v9.9.9".to_string(),
                release_page_url: "https://example.com/releases/v9.9.9".to_string(),
                reason: ReleaseNotReadyReason::ManifestFetchFailed,
            },
            CheckKind::Manual,
            UpdateState::Idle,
        );
        assert!(matches!(
            state,
            UpdateState::Error {
                failure: UpdateFailure::Network,
                ..
            }
        ));
    }

    #[test]
    fn manual_promotion_changes_final_failure_semantics() {
        let state = Arc::new(RwLock::new(UpdateState::Checking {
            kind: CheckKind::Automatic,
        }));
        assert_eq!(current_check_kind(&state), Some(CheckKind::Automatic));

        {
            let mut guard = state.write().unwrap();
            *guard = UpdateState::Checking {
                kind: CheckKind::Manual,
            };
        }

        let effective_kind = current_check_kind(&state).unwrap_or(CheckKind::Automatic);
        let settled = decision_to_state(
            UpdateDecision::Failed(UpdateFailure::Network),
            effective_kind,
            UpdateState::UpToDate,
        );
        assert!(matches!(
            settled,
            UpdateState::Error {
                failure: UpdateFailure::Network,
                ..
            }
        ));
    }

    #[test]
    fn release_page_url_exposes_release_page_not_download_url() {
        let UpdateDecision::Available { release } =
            decide_release("1.0.0", &verified_release(), Some(&manifest()))
        else {
            panic!("expected available decision");
        };
        let state = UpdateState::Available { release };
        assert_eq!(
            state.release_page_url(),
            Some("https://example.com/releases/v9.9.9")
        );
    }

    #[test]
    fn duplicate_check_invokes_completion() {
        let state = Arc::new(RwLock::new(UpdateState::Checking {
            kind: CheckKind::Automatic,
        }));
        let called = Arc::new(AtomicBool::new(false));
        let called_for_callback = called.clone();
        check_now(state.clone(), CheckKind::Manual, move || {
            called_for_callback.store(true, Ordering::SeqCst);
        });
        assert!(!called.load(Ordering::SeqCst));
        assert_eq!(
            *state.read().unwrap(),
            UpdateState::Checking {
                kind: CheckKind::Manual
            }
        );
        {
            let mut guard = state.write().unwrap();
            *guard = UpdateState::Error {
                message: "done".to_string(),
                failure: UpdateFailure::Network,
            };
        }
        for _ in 0..20 {
            if called.load(Ordering::SeqCst) {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("joined check callback did not wait for settled state");
    }
}
