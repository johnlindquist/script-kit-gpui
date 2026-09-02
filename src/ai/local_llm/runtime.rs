//! Process-wide local ghost-LLM runtime.
//!
//! The loaded model/context lives on ONE dedicated OS thread (an actor). This
//! satisfies the `!Send`/`!Sync` restriction of the llama.cpp context and keeps
//! the heavy FFI decode loop off the GPUI executor. Requests arrive over an
//! mpsc channel; each carries a reply channel and a cancel flag.

use super::types::{GhostPromptSpec, LocalGhostRequest, LocalGhostResponse, ResolvedLocalModel};
use anyhow::{Context as _, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(not(all(target_os = "macos", feature = "local-llm")))]
use super::stub_backend::LoadedLocalLlm;
#[cfg(all(target_os = "macos", feature = "local-llm"))]
use super::subprocess_backend::LoadedLocalLlm;

static LOCAL_GHOST_LLM: OnceLock<LocalLlmActor> = OnceLock::new();

/// The lazily-started global runtime actor.
pub(crate) fn global() -> Result<&'static LocalLlmActor> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
    Ok(LOCAL_GHOST_LLM.get_or_init(LocalLlmActor::start))
}

/// Drops the loaded model on the actor thread and joins it. Must be called
/// before process exit (e.g. on app quit) when a model was loaded: the pinned
/// llama.cpp aborts in its Metal device static destructor if a model's residency
/// set is still alive at `__cxa_finalize`. No-op if the actor never started.
pub(crate) fn shutdown() {
    if let Some(actor) = LOCAL_GHOST_LLM.get() {
        actor.shutdown();
    }
}

enum Message {
    Generate(Box<Job>),
    Shutdown,
}

pub(crate) struct LocalLlmActor {
    tx: mpsc::Sender<Message>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct Job {
    config: crate::config::Config,
    request: LocalGhostRequest,
    reply: mpsc::Sender<Result<LocalGhostResponse>>,
}

/// Unload the model (and its helper subprocess / Metal residency) after this
/// long without a ghost request. The next request reloads on demand, trading
/// one slower first hint for ~GBs of reclaimed RAM while idle.
const IDLE_UNLOAD: std::time::Duration = std::time::Duration::from_secs(5 * 60);

impl LocalLlmActor {
    fn start() -> Self {
        let (tx, rx) = mpsc::channel::<Message>();
        let spawned = std::thread::Builder::new()
            .name("script-kit-local-ghost-llm".to_string())
            .spawn(move || {
                let mut engine = LocalLlmEngine::default();
                loop {
                    let message = match rx.recv_timeout(IDLE_UNLOAD) {
                        Ok(message) => message,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if engine.loaded.take().is_some() {
                                tracing::debug!(
                                    target: "script_kit::ghost_text",
                                    idle_secs = IDLE_UNLOAD.as_secs(),
                                    "ghost local llm unloaded after idle"
                                );
                            }
                            continue;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    match message {
                        Message::Shutdown => break,
                        Message::Generate(job) => {
                            // Drop already-stale jobs cheaply before touching the model.
                            if job.request.cancel.load(Ordering::Relaxed) {
                                let _ = job
                                    .reply
                                    .send(Err(anyhow::anyhow!("ghost_local_llm_cancelled")));
                                continue;
                            }
                            let result = engine.generate(&job.config, job.request);
                            let _ = job.reply.send(result);
                        }
                    }
                }
                // Dropping `engine` here frees the loaded model + its Metal
                // residency set before the process tears down the ggml backend.
                drop(engine);
            });
        let handle = match spawned {
            Ok(handle) => Some(handle),
            Err(err) => {
                // The closure (and its `rx`) is dropped on failure, so the channel
                // disconnects and `generate()` returns Err quickly — ghost text then
                // silently keeps the deterministic starter. Never panic on startup.
                tracing::error!(
                    target: "script_kit::ghost_text",
                    error = %err,
                    "failed to spawn local ghost llm actor"
                );
                None
            }
        };
        Self {
            tx,
            handle: Mutex::new(handle),
        }
    }

    fn shutdown(&self) {
        let _ = self.tx.send(Message::Shutdown);
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }

    /// Blocking call: enqueue a job and wait for the actor's reply. Intended to
    /// run on a background executor task, never on the UI thread.
    pub(crate) fn generate(
        &self,
        config: crate::config::Config,
        request: LocalGhostRequest,
    ) -> Result<LocalGhostResponse> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        if request.cancel.load(Ordering::Relaxed) {
            anyhow::bail!("ghost_local_llm_cancelled");
        }
        let (reply, rx) = mpsc::channel();
        self.tx
            .send(Message::Generate(Box::new(Job {
                config,
                request,
                reply,
            })))
            .context("ghost local llm actor unavailable")?;
        rx.recv().context("ghost local llm actor disconnected")?
    }
}

/// Owns the loaded model across requests, reloading only when the resolved
/// model identity changes.
#[derive(Default)]
struct LocalLlmEngine {
    loaded: Option<LoadedLocalLlm>,
}

impl LocalLlmEngine {
    fn generate(
        &mut self,
        config: &crate::config::Config,
        request: LocalGhostRequest,
    ) -> Result<LocalGhostResponse> {
        if request.cancel.load(Ordering::Relaxed) {
            anyhow::bail!("ghost_local_llm_cancelled");
        }
        let model = super::model_locator::resolve_ghost_model(config)
            .context("ghost_local_llm_no_model")?;
        self.load_if_needed(&model, &request.cancel)?;
        let prompt = match &request.prompt {
            GhostPromptSpec::Launcher {
                partial_query,
                context,
            } => crate::scripts::search::ghost::build_local_ghost_prompt(partial_query, context),
            GhostPromptSpec::NotesContinuation { prompt } => prompt.clone(),
        };
        let raw_completion = self
            .loaded
            .as_mut()
            .context("ghost_local_llm_not_loaded")?
            .generate_one_line(&prompt, &request.cancel)?;
        Ok(LocalGhostResponse {
            model_id: model.model_id,
            raw_completion,
        })
    }

    fn load_if_needed(
        &mut self,
        model: &ResolvedLocalModel,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        if self
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.model_id() == model.model_id)
        {
            return Ok(());
        }
        if cancel.load(Ordering::Relaxed) {
            anyhow::bail!("ghost_local_llm_cancelled");
        }
        self.loaded = Some(LoadedLocalLlm::load(model)?);
        Ok(())
    }
}
