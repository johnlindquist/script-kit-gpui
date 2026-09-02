//! First-run download of the preferred ghost-text model (Gemma 4 E2B QAT).
//!
//! Ghost text should be zero-setup: when the Notes side-channel wants a model
//! and nothing resolves on disk, this module fetches Google's official
//! Gemma 4 E2B QAT Q4_0 GGUF (~3.35 GB, one file, resumable) into
//! `get_kit_path()/models/ghost-text/`. Deterministic ghost predictions keep
//! working throughout; the moment the file lands, the next side-channel
//! request resolves and loads it.
//!
//! Politeness rules (same shape as `brain::download`):
//! - never more than one attempt per 24h (mtime of a `.download-attempt`
//!   marker file — the ghost layer has no sqlite meta store)
//! - resumable via a `.partial` file + HTTP Range
//! - opt out by creating `models/ghost-text/.no-download`
//! - at most one in-flight download per process.

use anyhow::{Context as _, Result};
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const MODEL_URL: &str = "https://huggingface.co/google/gemma-4-E2B-it-qat-q4_0-gguf/resolve/main/gemma-4-E2B_q4_0-it.gguf";
const MODEL_FILE: &str = super::model_locator::PREFERRED_GHOST_MODEL;
const MODEL_SIZE: u64 = 3_349_514_112;
const ATTEMPT_MARKER: &str = ".download-attempt";
const ATTEMPT_COOLDOWN_SECS: u64 = 24 * 60 * 60;

static DOWNLOAD_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Kick off a background download of the preferred ghost model when no ghost
/// model resolves on disk. Fire-and-forget: returns immediately, never blocks
/// the caller, and silently respects the opt-out marker and 24h cooldown.
pub(crate) fn ensure_ghost_model_in_background(config: &crate::config::Config) {
    if let Err(error) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
    {
        tracing::warn!(%error, "Local model download refused");
        return;
    }
    if super::model_locator::resolve_ghost_model(config).is_some() {
        return;
    }
    let dir = super::model_locator::ghost_models_dir();
    if dir.join(".no-download").exists() {
        return;
    }
    if attempted_recently(&dir) {
        return;
    }
    if DOWNLOAD_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("script-kit-ghost-model-download".to_string())
        .spawn(move || {
            let _ = mark_attempt(&dir);
            match download_model(&dir) {
                Ok(()) => tracing::info!(
                    target: "script_kit::ghost_text",
                    model = MODEL_FILE,
                    "ghost model downloaded; llm ghost text enabled"
                ),
                Err(error) => tracing::warn!(
                    target: "script_kit::ghost_text",
                    error = %error,
                    "ghost model download failed; ghost text stays deterministic (retry in 24h)"
                ),
            }
            DOWNLOAD_IN_FLIGHT.store(false, Ordering::SeqCst);
        });
    if spawned.is_err() {
        DOWNLOAD_IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

fn attempted_recently(dir: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(dir.join(ATTEMPT_MARKER)) else {
        return false;
    };
    meta.modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|elapsed| elapsed.as_secs() < ATTEMPT_COOLDOWN_SECS)
}

fn mark_attempt(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).context("create ghost models dir")?;
    std::fs::write(dir.join(ATTEMPT_MARKER), b"").context("write ghost download marker")
}

fn download_model(dir: &Path) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
    std::fs::create_dir_all(dir).context("create ghost models dir")?;
    let final_path = dir.join(MODEL_FILE);
    let partial_path = dir.join(format!("{MODEL_FILE}.partial"));

    let resume_from = std::fs::metadata(&partial_path)
        .map(|meta| meta.len())
        .unwrap_or(0);
    if resume_from < MODEL_SIZE {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .new_agent();
        let mut request = agent.get(MODEL_URL);
        if resume_from > 0 {
            request = request.header("Range", &format!("bytes={resume_from}-"));
        }
        let response = request.call().context("ghost model HTTP request")?;
        let status = response.status().as_u16();
        let actual_resume = match status {
            206 => resume_from,
            200 => 0,
            other => anyhow::bail!("unexpected HTTP status {other} downloading ghost model"),
        };
        let mut file = if actual_resume > 0 {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&partial_path)
                .context("open partial ghost model")?
        } else {
            std::fs::File::create(&partial_path).context("create partial ghost model")?
        };
        let mut reader = response.into_body().into_reader();
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let read = reader.read(&mut buf).context("read ghost model body")?;
            if read == 0 {
                break;
            }
            file.write_all(&buf[..read]).context("write ghost model")?;
        }
        file.flush().context("flush ghost model")?;
    }

    let downloaded = std::fs::metadata(&partial_path)
        .map(|meta| meta.len())
        .unwrap_or(0);
    if downloaded != MODEL_SIZE {
        anyhow::bail!("ghost model incomplete: {downloaded} of {MODEL_SIZE} bytes");
    }
    std::fs::rename(&partial_path, &final_path).context("finalize ghost model")?;
    Ok(())
}
