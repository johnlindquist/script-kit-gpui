use super::types::{GhostSamplingParams, ResolvedLocalModel};
use anyhow::{anyhow, Context as _, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

const HELPER_ENV: &str = "SCRIPT_KIT_GHOST_LLM_HELPER";
const HELPER_NAME: &str = "script-kit-ghost-llm-helper";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct WireSamplingParams {
    max_prediction_tokens: usize,
    temperature: f32,
    top_k: i32,
    top_p: f32,
    min_p: f32,
    repeat_penalty: f32,
    ctx_tokens: u32,
    batch_size: u32,
    gpu_layers: u32,
}

impl From<GhostSamplingParams> for WireSamplingParams {
    fn from(value: GhostSamplingParams) -> Self {
        Self {
            max_prediction_tokens: value.max_prediction_tokens,
            temperature: value.temperature,
            top_k: value.top_k,
            top_p: value.top_p,
            min_p: value.min_p,
            repeat_penalty: value.repeat_penalty,
            ctx_tokens: value.ctx_tokens,
            batch_size: value.batch_size,
            gpu_layers: value.gpu_layers,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HelperRequest {
    Load {
        id: u64,
        model_path: String,
        model_id: String,
        sampling: WireSamplingParams,
    },
    Generate {
        id: u64,
        model_path: String,
        model_id: String,
        prompt: String,
        sampling: WireSamplingParams,
    },
    Cancel {
        id: u64,
    },
    Shutdown {
        id: u64,
    },
}

#[derive(Debug, Deserialize)]
struct HelperResponse {
    id: u64,
    ok: bool,
    model_id: Option<String>,
    raw_completion: Option<String>,
    error: Option<String>,
}

pub(crate) struct LoadedLocalLlm {
    model_id: String,
    model_path: PathBuf,
    sampling: GhostSamplingParams,
    client: HelperClient,
}

impl LoadedLocalLlm {
    pub(crate) fn load(model: &ResolvedLocalModel) -> Result<Self> {
        let sampling = GhostSamplingParams::default();
        let client = HelperClient::spawn()?;
        client.load(
            &model.path,
            &model.model_id,
            WireSamplingParams::from(sampling),
        )?;
        Ok(Self {
            model_id: model.model_id.clone(),
            model_path: model.path.clone(),
            sampling,
            client,
        })
    }

    pub(crate) fn model_id(&self) -> &str {
        &self.model_id
    }

    pub(crate) fn generate_one_line(
        &mut self,
        prompt: &str,
        cancel: &Arc<AtomicBool>,
    ) -> Result<String> {
        let started = Instant::now();
        let raw = self.client.generate(
            &self.model_path,
            &self.model_id,
            prompt,
            WireSamplingParams::from(self.sampling),
            cancel,
        )?;
        tracing::debug!(target: "script_kit::ghost_text", elapsed_ms = started.elapsed().as_millis(), model_id = %self.model_id, "ghost local llm helper generate");
        Ok(raw)
    }
}

struct HelperClient {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<HelperResponse>>>>,
    next_id: AtomicU64,
    /// Tracks the helper with the global process manager for quit-time and
    /// post-crash reaping; dropping this client unregisters it.
    _registration: crate::process_manager::ChildRegistration,
}

impl HelperClient {
    fn spawn() -> Result<Self> {
        let helper_path = resolve_helper_path()?;
        let mut command = Command::new(&helper_path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        // Own process group so orphan cleanup / kill_all can reap the helper.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("spawn ghost llm helper {}", helper_path.display()))?;
        let registration = crate::process_manager::ChildRegistration::register(
            child.id(),
            &helper_path.to_string_lossy(),
        );
        let stdin = child
            .stdin
            .take()
            .context("ghost llm helper stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("ghost llm helper stdout unavailable")?;
        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<HelperResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = Arc::clone(&pending);
        std::thread::Builder::new()
            .name("script-kit-ghost-llm-helper-reader".to_string())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    let Ok(line) = line else { break; };
                    let Ok(response) = serde_json::from_str::<HelperResponse>(&line) else {
                        let diagnostic = crate::ai::reliability::redact_diagnostic(&line);
                        tracing::warn!(target: "script_kit::ghost_text", diagnostic_fingerprint = %diagnostic.fingerprint.0, stdout_bytes = line.len(), "ignored malformed ghost llm helper stdout line");
                        continue;
                    };
                    let sender = reader_pending.lock().ok().and_then(|mut pending| pending.remove(&response.id));
                    if let Some(sender) = sender { let _ = sender.send(response); }
                }
            })
            .context("spawn ghost llm helper reader thread")?;
        Ok(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(Some(stdin)),
            pending,
            next_id: AtomicU64::new(1),
            _registration: registration,
        })
    }

    fn load(&self, model_path: &Path, model_id: &str, sampling: WireSamplingParams) -> Result<()> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = self.request_blocking(HelperRequest::Load {
            id,
            model_path: model_path.display().to_string(),
            model_id: model_id.to_string(),
            sampling,
        })?;
        response.into_result().map(|_| ())
    }

    fn generate(
        &self,
        model_path: &Path,
        model_id: &str,
        prompt: &str,
        sampling: WireSamplingParams,
        cancel: &Arc<AtomicBool>,
    ) -> Result<String> {
        if cancel.load(Ordering::Relaxed) {
            anyhow::bail!("ghost_local_llm_cancelled");
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("ghost llm helper pending lock poisoned"))?
            .insert(id, tx);
        let request = HelperRequest::Generate {
            id,
            model_path: model_path.display().to_string(),
            model_id: model_id.to_string(),
            prompt: prompt.to_string(),
            sampling,
        };
        if let Err(err) = self.write_request(&request) {
            let _ = self.pending.lock().map(|mut pending| pending.remove(&id));
            return Err(err);
        }
        let mut cancel_sent = false;
        loop {
            match rx.recv_timeout(Duration::from_millis(10)) {
                Ok(response) => {
                    return response.into_result().and_then(|response| {
                        response.raw_completion.context("missing helper completion")
                    });
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if cancel.load(Ordering::Relaxed) && !cancel_sent {
                        self.cancel(id)?;
                        cancel_sent = true;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("ghost llm helper disconnected")
                }
            }
        }
    }

    fn cancel(&self, id: u64) -> Result<()> {
        self.write_request(&HelperRequest::Cancel { id })
    }

    fn request_blocking(&self, request: HelperRequest) -> Result<HelperResponse> {
        let id = request.id();
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("ghost llm helper pending lock poisoned"))?
            .insert(id, tx);
        if let Err(err) = self.write_request(&request) {
            let _ = self.pending.lock().map(|mut pending| pending.remove(&id));
            return Err(err);
        }
        rx.recv().context("ghost llm helper disconnected")
    }

    fn write_request(&self, request: &HelperRequest) -> Result<()> {
        let mut guard = self
            .stdin
            .lock()
            .map_err(|_| anyhow!("ghost llm helper stdin lock poisoned"))?;
        let stdin = guard
            .as_mut()
            .context("ghost llm helper stdin already closed")?;
        serde_json::to_writer(&mut *stdin, request).context("write ghost llm helper request")?;
        stdin
            .write_all(b"\n")
            .context("write ghost llm helper request newline")?;
        stdin.flush().context("flush ghost llm helper request")
    }
}

impl Drop for HelperClient {
    fn drop(&mut self) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.write_request(&HelperRequest::Shutdown { id });
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = stdin.take();
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.wait();
        }
    }
}

impl HelperRequest {
    fn id(&self) -> u64 {
        match self {
            HelperRequest::Load { id, .. }
            | HelperRequest::Generate { id, .. }
            | HelperRequest::Cancel { id }
            | HelperRequest::Shutdown { id } => *id,
        }
    }
}

impl HelperResponse {
    fn into_result(self) -> Result<Self> {
        if self.ok {
            Ok(self)
        } else {
            let error = match self.error {
                Some(error) => error,
                None => "ghost llm helper request failed".to_string(),
            };
            Err(anyhow!("{}", error))
        }
    }
}

/// Locate the ghost-llm helper binary.
///
/// Resolution order (first hit wins):
/// 1. `SCRIPT_KIT_GHOST_LLM_HELPER` env override.
/// 2. The stable install dir `~/.scriptkit/bin/<helper>` — survives transient
///    build-pool purges (the dev `target-agent/pools/*` dirs are reclaimed under
///    disk pressure, so a sibling-of-current_exe lookup is unreliable in dev).
/// 3. Walk up from `current_exe()` looking for a sibling (covers the shipped
///    bundle where the helper sits next to the app binary).
fn resolve_helper_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(HELPER_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        tracing::warn!(target: "script_kit::ghost_text", env = %path.display(), "{HELPER_ENV} set but not a file; falling back");
    }

    if let Some(stable) = stable_helper_path() {
        if stable.is_file() {
            return Ok(stable);
        }
    }

    let exe = std::env::current_exe().context("resolve current executable path")?;
    let mut dir = exe.parent();
    while let Some(candidate_dir) = dir {
        let candidate = candidate_dir.join(HELPER_NAME);
        if candidate.is_file() {
            return Ok(candidate);
        }
        dir = candidate_dir.parent();
    }

    anyhow::bail!(
        "ghost llm helper not found; set {HELPER_ENV}, install it at {}, or place {HELPER_NAME} next to the app binary",
        stable_helper_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.scriptkit/bin".to_string())
    )
}

/// `~/.scriptkit/bin/<helper>` — the stable, non-purged install location.
fn stable_helper_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".scriptkit")
            .join("bin")
            .join(HELPER_NAME)
    })
}
