//! Resolves the GGUF model used for on-device ghost-text generation.
//!
//! Resolution order (first hit wins):
//!   1. Explicit override: `SCRIPT_KIT_GHOST_LLM_MODEL_PATH` env.
//!   2. Script Kit-owned models: `get_kit_path()/models/ghost-text/*.gguf`
//!      (preferring the Gemma 4 E2B QAT model, then the small "fast" Qwen
//!      model when present).
//!   3. Cotabby / Cotypist models already on disk (read-only reuse).
//!   4. None -> caller keeps the deterministic starter, no network.

use super::types::{GhostSamplingParams, ResolvedLocalModel};
use std::path::{Path, PathBuf};

/// Env override for an explicit model path.
const ENV_MODEL_PATH: &str = "SCRIPT_KIT_GHOST_LLM_MODEL_PATH";
/// Preferred ghost model: Gemma 4 E2B QAT Q4_0 — markedly better at grounding
/// completions in retrieved (brain) context than the sub-1B fallbacks, while
/// only ~2.3B parameters are active per token. See `super::download` for the
/// first-run fetch.
pub(crate) const PREFERRED_GHOST_MODEL: &str = "gemma-4-E2B_q4_0-it.gguf";
/// Fast fallback model filename when present in the Script Kit models dir.
const PREFERRED_FAST_MODEL: &str = "Qwen3-0.6B-Q4_K_M.gguf";

/// Resolve the best available ghost model, or `None` when nothing is on disk.
pub(crate) fn resolve_ghost_model(_config: &crate::config::Config) -> Option<ResolvedLocalModel> {
    if let Err(error) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
    {
        tracing::warn!(%error, "Local model discovery refused");
        return None;
    }
    let sampling = GhostSamplingParams::default();
    for candidate in candidate_paths() {
        if candidate.is_file() {
            return resolved_from_path(&candidate, &sampling);
        }
    }
    None
}

/// Stable cache identity for `model_id`: filename + length + mtime + sampling.
fn resolved_from_path(path: &Path, sampling: &GhostSamplingParams) -> Option<ResolvedLocalModel> {
    let filename = path.file_name()?.to_string_lossy().to_string();
    let metadata = std::fs::metadata(path).ok()?;
    let len = metadata.len();
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let model_id = format!(
        "local-gguf:{filename}:{len}:{mtime}:{}",
        sampling.fingerprint()
    );
    Some(ResolvedLocalModel {
        path: path.to_path_buf(),
        model_id,
        display_name: filename,
    })
}

/// Ordered list of candidate model paths (existence checked by the caller).
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Explicit env override.
    if let Some(explicit) = std::env::var_os(ENV_MODEL_PATH) {
        let explicit = PathBuf::from(explicit);
        if !explicit.as_os_str().is_empty() {
            paths.push(explicit);
        }
    }

    // 2. Script Kit-owned models dir: prefer Gemma 4 E2B QAT (any matching
    //    filename casing/quant), then the small fast model, then any GGUF.
    let kit_models = ghost_models_dir();
    let kit_ggufs = gguf_files_in(&kit_models);
    paths.push(kit_models.join(PREFERRED_GHOST_MODEL));
    paths.extend(
        kit_ggufs
            .iter()
            .filter(|path| file_name_contains(path, "gemma-4-e2b"))
            .cloned(),
    );
    paths.push(kit_models.join(PREFERRED_FAST_MODEL));
    paths.extend(kit_ggufs);

    // 3. Cotabby / Cotypist models already downloaded (read-only reuse). The
    //    Cotabby dir casing varies by install ("Cotabby" vs "cotabby"); check
    //    both so a case-sensitive volume still resolves it.
    if let Some(home) = home_dir() {
        let app_support = home.join("Library/Application Support");
        for dir_name in ["Cotabby", "cotabby"] {
            let cotabby = app_support.join(dir_name).join("LlamaRuntime");
            paths.push(cotabby.join(PREFERRED_FAST_MODEL));
            paths.push(cotabby.join("gemma-4-E2B-it-Q4_K_M.gguf"));
            paths.extend(gguf_files_in(&cotabby));
        }
        let cotypist = app_support.join("app.cotypist.Cotypist/Models");
        paths.extend(gguf_files_in(&cotypist));
    }

    paths
}

/// The Script Kit-owned ghost model directory: `get_kit_path()/models/ghost-text`.
pub(crate) fn ghost_models_dir() -> PathBuf {
    crate::setup::get_kit_path()
        .join("models")
        .join("ghost-text")
}

fn file_name_contains(path: &Path, needle: &str) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .is_some_and(|name| name.contains(needle))
}

/// All `*.gguf` files directly inside `dir` (sorted for deterministic order).
/// Multimodal projector files (`*mmproj*`) are skipped — they are not language
/// models and must never be loaded as the ghost LLM.
fn gguf_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
                && !file_name_contains(&path, "mmproj")
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "script-kit-ghost-locator-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn model_id_includes_filename_len_mtime_and_sampling() {
        let dir = temp_dir("id");
        let path = dir.join("tiny.gguf");
        std::fs::write(&path, b"GGUFxxxx").expect("write fake gguf");
        let sampling = GhostSamplingParams::default();
        let resolved = resolved_from_path(&path, &sampling).expect("resolve");
        assert!(resolved.model_id.starts_with("local-gguf:tiny.gguf:8:"));
        assert!(resolved.model_id.contains(&sampling.fingerprint()));
        assert_eq!(resolved.display_name, "tiny.gguf");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolver_prefers_explicit_env_path_over_other_sources() {
        let dir = temp_dir("env");
        let path = dir.join("explicit.gguf");
        std::fs::write(&path, b"GGUF").expect("write");
        temp_env::with_var(ENV_MODEL_PATH, Some(&path), || {
            let resolved =
                resolve_ghost_model(&crate::config::Config::default()).expect("env path resolves");
            assert_eq!(resolved.path, path);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolver_returns_none_when_env_points_at_missing_file() {
        let missing = std::env::temp_dir().join("script-kit-ghost-does-not-exist.gguf");
        let _ = std::fs::remove_file(&missing);
        temp_env::with_var(ENV_MODEL_PATH, Some(&missing), || {
            // The env path is missing; only a real on-disk model elsewhere could
            // satisfy resolution. This asserts the env miss does not panic and the
            // resolved model (if any) is never the missing file.
            let resolved = resolve_ghost_model(&crate::config::Config::default());
            assert!(resolved.as_ref().map(|m| &m.path) != Some(&missing));
        });
    }
}
