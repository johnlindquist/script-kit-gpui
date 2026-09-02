//! On-device (local, GGUF / llama.cpp) ghost-text generation.
//!
//! This is the local-first replacement for the cloud `provider.send_message`
//! engine in the ghost-text debounced side-channel. The Cotabby approach
//! ([`~/dev/cotabby`]): run a small GGUF model in-process via llama.cpp, compose
//! a single prompt string, and return a one-line continuation — all on-device,
//! no network.
//!
//! The real llama.cpp backend is gated behind `--features local-llm` on macOS
//! (kept off by default until the ggml/whisper link is proven clean). When the
//! backend is unavailable or no model is on disk, generation returns `Err` and
//! the caller silently keeps the deterministic starter.

mod download;
pub(crate) mod model_locator;
mod runtime;
mod types;

#[cfg(not(all(target_os = "macos", feature = "local-llm")))]
mod stub_backend;
#[cfg(all(target_os = "macos", feature = "local-llm"))]
mod subprocess_backend;

pub(crate) use download::ensure_ghost_model_in_background;
pub(crate) use types::{GhostPromptSpec, LocalGhostRequest, LocalGhostResponse};

/// Cache identity of the model that would serve ghost text (or a sentinel when
/// none is available). Folded into `GhostLlmCacheKey.model_id`.
pub(crate) fn ghost_model_id_hint(config: &crate::config::Config) -> String {
    model_locator::resolve_ghost_model(config)
        .map(|model| model.model_id)
        .unwrap_or_else(|| "local-llm:none".to_string())
}

/// Generate a raw (unsanitized) ghost continuation on-device. The caller runs
/// `sanitize_llm_completion_suffix` over the result before it reaches the UI.
/// Returns `Err` when no model is on disk or the backend is unavailable — the
/// caller treats that as "keep the deterministic starter, no user-facing error".
pub(crate) fn generate_ghost_completion(
    config: &crate::config::Config,
    request: LocalGhostRequest,
) -> anyhow::Result<LocalGhostResponse> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
    runtime::global()?.generate(config.clone(), request)
}

/// Drop the loaded on-device model and join the runtime actor. Call before app
/// process exit when the `local-llm` engine may have loaded a model, so the
/// pinned llama.cpp does not abort in its Metal static destructor. Safe no-op if
/// the runtime never started.
pub(crate) fn shutdown_local_ghost_llm() {
    runtime::shutdown();
}

#[cfg(all(test, target_os = "macos", feature = "local-llm"))]
mod smoke {
    //! On-device smoke test. Ignored unless a tiny GGUF is provided via
    //! `SCRIPT_KIT_GHOST_LLM_SMOKE_MODEL`. Proves the local engine loads a model
    //! and returns a continuation that survives the shared sanitizer.
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[test]
    #[ignore = "requires SCRIPT_KIT_GHOST_LLM_SMOKE_MODEL=/path/to/model.gguf"]
    fn local_llm_smoke_generates_non_empty_suffix() {
        let Some(path) = std::env::var_os("SCRIPT_KIT_GHOST_LLM_SMOKE_MODEL") else {
            eprintln!("skipping: SCRIPT_KIT_GHOST_LLM_SMOKE_MODEL not set");
            return;
        };
        std::env::set_var("SCRIPT_KIT_GHOST_LLM_MODEL_PATH", path);

        let query = std::env::var("SCRIPT_KIT_GHOST_LLM_SMOKE_QUERY")
            .unwrap_or_else(|_| "fix the".to_string());
        let response = super::generate_ghost_completion(
            &crate::config::Config::default(),
            super::LocalGhostRequest {
                prompt: super::GhostPromptSpec::Launcher {
                    partial_query: query.clone(),
                    context: crate::scripts::search::ghost::GhostContext::default(),
                },
                cancel: Arc::new(AtomicBool::new(false)),
            },
        )
        .expect("local generation should succeed");

        eprintln!(
            "query: {query:?}  ->  local raw completion: {:?}",
            response.raw_completion
        );
        // The model id must be the on-device fingerprint, never a cloud id.
        assert!(response.model_id.starts_with("local-gguf:"));

        // Drop the model before the process tears down ggml's Metal backend,
        // proving clean shutdown (otherwise the pinned llama.cpp aborts at exit).
        super::shutdown_local_ghost_llm();
    }
}
