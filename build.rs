// Build script for script-kit-gpui
//
// This script tells Cargo to rebuild when key files change.
// SDK deployment to ~/.scriptkit is now handled at runtime by setup::ensure_kit_setup()
// rather than at build time, ensuring the SDK is always in sync with the running binary.

use std::path::PathBuf;
use std::process::Command;

fn read_git_hash() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

fn resolve_git_dir() -> Option<PathBuf> {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| PathBuf::from(s.trim()))
            } else {
                None
            }
        })
}

fn emit_git_rerun_triggers() {
    if let Some(git_dir) = resolve_git_dir() {
        let head_path = git_dir.join("HEAD");
        println!("cargo:rerun-if-changed={}", head_path.display());
        println!(
            "cargo:rerun-if-changed={}",
            git_dir.join("packed-refs").display()
        );

        if let Ok(head_contents) = std::fs::read_to_string(&head_path) {
            if let Some(reference_path) = head_contents.strip_prefix("ref:").map(str::trim) {
                println!(
                    "cargo:rerun-if-changed={}",
                    git_dir.join(reference_path).display()
                );
            }
        }
    } else {
        // Fallback for environments where git is unavailable.
        println!("cargo:rerun-if-changed=.git/HEAD");
        println!("cargo:rerun-if-changed=.git/packed-refs");
    }
}

fn should_track_git_head(
    profile: &str,
    github_sha: Option<&str>,
    override_value: Option<&str>,
) -> bool {
    profile == "release"
        || github_sha.is_some_and(|sha| !sha.trim().is_empty())
        || override_value == Some("1")
}

fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    let github_sha = std::env::var("GITHUB_SHA").ok();

    // Expose the git commit hash as a compile-time env var (GIT_HASH).
    // Falls back to CI-provided SHA or "unknown" if git is unavailable.
    let git_hash = read_git_hash()
        .or_else(|| github_sha.as_ref().map(|sha| sha.chars().take(7).collect()))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_HASH={git_hash}");

    // Expose the build profile (debug/release) as a compile-time env var (BUILD_PROFILE).
    println!("cargo:rustc-env=BUILD_PROFILE={profile}");

    // A docs-only commit must not recompile/link the 67-second local app test
    // harness. The embedded hash still identifies the commit the binary was
    // actually built from. Exact packaged/CI provenance remains mandatory,
    // and local operators can explicitly opt into eager HEAD tracking.
    let track_override = std::env::var("SCRIPT_KIT_TRACK_GIT_HEAD").ok();
    if should_track_git_head(&profile, github_sha.as_deref(), track_override.as_deref()) {
        emit_git_rerun_triggers();
    }
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=SCRIPT_KIT_TRACK_GIT_HEAD");

    // Trigger rebuild when SDK source changes (it's embedded via include_str!)
    println!("cargo:rerun-if-changed=scripts/kit-sdk.ts");

    // Trigger rebuild when kit-init files change (embedded and shipped to ~/.scriptkit/)
    println!("cargo:rerun-if-changed=kit-init/config-template.ts");
    println!("cargo:rerun-if-changed=kit-init/theme.example.json");
    println!("cargo:rerun-if-changed=kit-init/GUIDE.md");

    // Trigger rebuild when bundled fonts change (embedded via include_bytes!)
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-Regular.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-Bold.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-Italic.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-BoldItalic.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-Medium.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-SemiBold.ttf");
}

#[cfg(test)]
mod tests {
    use super::should_track_git_head;

    #[test]
    fn local_debug_builds_do_not_recompile_after_docs_only_commits() {
        assert!(!should_track_git_head("debug", None, None));
        assert!(!should_track_git_head("debug", Some("  "), None));
        assert!(!should_track_git_head("debug", None, Some("0")));
    }

    #[test]
    fn release_ci_and_explicit_opt_in_preserve_exact_git_provenance() {
        assert!(should_track_git_head("release", None, None));
        assert!(should_track_git_head("debug", Some("abc1234"), None));
        assert!(should_track_git_head("debug", None, Some("1")));
    }
}
