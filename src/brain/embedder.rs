//! Brain embedder: a thin JSON-lines client for the `script-kit-ghost-llm-helper`
//! subprocess's `embed` request.
//!
//! Deliberately independent from `ai::local_llm::subprocess_backend`:
//! - it is NOT gated behind the `local-llm` feature (the helper is a separate
//!   process, so the app never links ggml for this path);
//! - it speaks only `embed`/`shutdown`, with blocking request semantics and
//!   no cancellation (embedding batches are short).
//!
//! Model resolution: `SCRIPT_KIT_BRAIN_EMBED_MODEL_PATH` env override, then
//! any `*.gguf` under `~/.scriptkit/models/brain/`. When no model is present
//! the brain degrades gracefully to FTS-only search.

use anyhow::{anyhow, Context as _, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const HELPER_ENV: &str = "SCRIPT_KIT_GHOST_LLM_HELPER";
const HELPER_NAME: &str = "script-kit-ghost-llm-helper";
const MODEL_ENV: &str = "SCRIPT_KIT_BRAIN_EMBED_MODEL_PATH";

/// Hard ceiling for one embed batch call. 60s for a 16-doc chunked batch on
/// CPU is generous but bounded — the model runs CPU-only today, so do NOT drop
/// below 60s. A wedged helper must never stall the indexer thread for minutes.
const EMBED_BATCH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EmbedRequest {
    Embed {
        id: u64,
        model_path: String,
        model_id: String,
        texts: Vec<String>,
        gpu_layers: u32,
    },
    Shutdown {
        id: u64,
    },
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    id: u64,
    ok: bool,
    #[serde(default)]
    embeddings: Option<Vec<Vec<f32>>>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedEmbedModel {
    pub path: PathBuf,
    pub model_id: String,
}

/// Locate the brain embedding model on disk. `None` => FTS-only mode.
pub fn resolve_embed_model() -> Option<ResolvedEmbedModel> {
    let candidate = if let Some(path) = std::env::var_os(MODEL_ENV) {
        let path = PathBuf::from(path);
        path.is_file().then_some(path)
    } else {
        let dir = dirs::home_dir()?
            .join(".scriptkit")
            .join("models")
            .join("brain");
        std::fs::read_dir(&dir).ok().and_then(|entries| {
            let mut ggufs: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "gguf"))
                .collect();
            ggufs.sort();
            ggufs.into_iter().next()
        })
    }?;
    let meta = std::fs::metadata(&candidate).ok()?;
    let name = candidate.file_name()?.to_string_lossy().into_owned();
    // Key the model id on a CONTENT fingerprint (length + hash of the first
    // 1 MiB), never mtime: re-downloading or restoring the identical model file
    // bumps mtime, which previously made every stored chunk embedding look stale
    // and triggered hours of silent background re-embedding. The fingerprint is
    // stable across re-downloads of the same bytes.
    let fingerprint = fingerprint_model_file(&candidate);
    Some(ResolvedEmbedModel {
        model_id: format!("brain-embed:{name}:{}:{fingerprint}", meta.len()),
        path: candidate,
    })
}

/// Hex fingerprint of a model file: SHA-256 over the first 1 MiB. GGUF headers
/// (metadata, tensor descriptors) live at the file start, so the leading MiB
/// distinguishes any two distinct models while staying cheap for multi-GB
/// weights. Combined with the file length in the id, this is a content identity
/// that survives re-download/restore of the same bytes. Falls back to `"0"` when
/// the file cannot be read (a missing model already degrades to FTS-only).
fn fingerprint_model_file(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    const FINGERPRINT_PREFIX: usize = 1024 * 1024;
    let Ok(mut file) = std::fs::File::open(path) else {
        return "0".to_string();
    };
    let mut hasher = Sha256::new();
    let mut remaining = FINGERPRINT_PREFIX;
    let mut buf = [0u8; 64 * 1024];
    while remaining > 0 {
        let want = remaining.min(buf.len());
        match file.read(&mut buf[..want]) {
            Ok(0) => break,
            Ok(n) => {
                hasher.update(&buf[..n]);
                remaining -= n;
            }
            Err(_) => return "0".to_string(),
        }
    }
    let digest = hasher.finalize();
    // First 16 hex chars (8 bytes) is ample collision resistance for a model id.
    let mut hex = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// A live helper subprocess dedicated to embeddings.
pub struct BrainEmbedder {
    model: ResolvedEmbedModel,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<EmbedResponse>>>>,
    next_id: AtomicU64,
    /// Tracks the helper with the global process manager for quit-time and
    /// post-crash reaping; dropping this embedder unregisters it.
    _registration: crate::process_manager::ChildRegistration,
}

impl BrainEmbedder {
    pub fn spawn(model: ResolvedEmbedModel) -> Result<Self> {
        let helper_path = resolve_helper_path()?;
        Self::spawn_with_helper(&helper_path, model)
    }

    /// Spawn against an explicit helper binary path. Splitting resolution out of
    /// the wiring lets tests point the embedder at a fake helper without the
    /// real `script-kit-ghost-llm-helper` binary being installed.
    pub(crate) fn spawn_with_helper(helper_path: &Path, model: ResolvedEmbedModel) -> Result<Self> {
        let mut command = Command::new(helper_path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // Own process group so orphan cleanup / kill_all can reap the helper.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("spawn brain embed helper {}", helper_path.display()))?;
        let registration = crate::process_manager::ChildRegistration::register(
            child.id(),
            &helper_path.to_string_lossy(),
        );
        let stdin = child.stdin.take().context("brain embed helper stdin")?;
        let stdout = child.stdout.take().context("brain embed helper stdout")?;
        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<EmbedResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = Arc::clone(&pending);
        std::thread::Builder::new()
            .name("script-kit-brain-embed-reader".to_string())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    let Ok(response) = serde_json::from_str::<EmbedResponse>(&line) else {
                        continue;
                    };
                    let sender = reader_pending
                        .lock()
                        .ok()
                        .and_then(|mut pending| pending.remove(&response.id));
                    if let Some(sender) = sender {
                        let _ = sender.send(response);
                    }
                }
            })
            .context("spawn brain embed reader thread")?;
        Ok(Self {
            model,
            child: Mutex::new(child),
            stdin: Mutex::new(Some(stdin)),
            pending,
            next_id: AtomicU64::new(1),
            _registration: registration,
        })
    }

    pub fn model_id(&self) -> &str {
        &self.model.model_id
    }

    /// True while the helper child process is still running.
    pub fn is_alive(&self) -> bool {
        self.child
            .lock()
            .ok()
            .map(|mut child| matches!(child.try_wait(), Ok(None)))
            .unwrap_or(false)
    }

    /// Embed a batch of texts. Blocking; returns unit-normalized vectors in
    /// input order (empty vec for empty inputs). Bounded by
    /// [`EMBED_BATCH_TIMEOUT`] so a wedged helper cannot stall the caller.
    pub fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        self.embed_with_timeout(texts, EMBED_BATCH_TIMEOUT)
    }

    /// [`embed`](Self::embed) with an explicit response timeout — tests use a
    /// short one to prove a dead helper fails fast without waiting the full
    /// production ceiling.
    pub(crate) fn embed_with_timeout(
        &self,
        texts: Vec<String>,
        timeout: Duration,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("brain embed pending lock poisoned"))?
            .insert(id, tx);
        let request = EmbedRequest::Embed {
            id,
            model_path: self.model.path.display().to_string(),
            model_id: self.model.model_id.clone(),
            texts,
            gpu_layers: 99,
        };
        if let Err(err) = self.write_request(&request) {
            let _ = self.pending.lock().map(|mut pending| pending.remove(&id));
            return Err(err);
        }
        let response = rx
            .recv_timeout(timeout)
            .context("brain embed helper timed out")?;
        if !response.ok {
            return Err(anyhow!(response
                .error
                .unwrap_or_else(|| "brain embed failed".to_string())));
        }
        response.embeddings.context("missing brain embeddings")
    }

    fn write_request(&self, request: &EmbedRequest) -> Result<()> {
        let mut guard = self
            .stdin
            .lock()
            .map_err(|_| anyhow!("brain embed stdin lock poisoned"))?;
        let stdin = guard.as_mut().context("brain embed stdin closed")?;
        serde_json::to_writer(&mut *stdin, request).context("write brain embed request")?;
        stdin
            .write_all(b"\n")
            .context("write brain embed newline")?;
        stdin.flush().context("flush brain embed request")
    }
}

impl Drop for BrainEmbedder {
    fn drop(&mut self) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.write_request(&EmbedRequest::Shutdown { id });
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = stdin.take();
        }
        if let Ok(mut child) = self.child.lock() {
            // A replaced or dropped embedder must never leak a zombie helper,
            // even if the shutdown request above never reached a wedged child.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Whether the embed helper binary is locatable — health surface only;
/// spawning still does its own resolution.
pub fn helper_available() -> bool {
    resolve_helper_path().is_ok()
}

/// Same resolution order as the ghost-text helper client (env override, then
/// the stable install dir, then a sibling of the current executable).
fn resolve_helper_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(HELPER_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let stable = home.join(".scriptkit").join("bin").join(HELPER_NAME);
        if stable.is_file() {
            return Ok(stable);
        }
    }
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from));
    while let Some(current) = dir {
        let candidate = current.join(HELPER_NAME);
        if candidate.is_file() {
            return Ok(candidate);
        }
        dir = current.parent().map(PathBuf::from);
    }
    Err(anyhow!("brain embed helper binary not found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn write_fake_helper(dir: &Path, script: &str) -> PathBuf {
        let path = dir.join("fake-brain-embed-helper.sh");
        let mut file = std::fs::File::create(&path).expect("create fake helper");
        file.write_all(script.as_bytes())
            .expect("write fake helper");
        file.flush().expect("flush fake helper");
        drop(file);
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod fake helper");
        path
    }

    fn fake_model() -> ResolvedEmbedModel {
        ResolvedEmbedModel {
            path: PathBuf::from("/nonexistent/brain-embed-model.gguf"),
            model_id: "brain-embed:fake:0:0".to_string(),
        }
    }

    /// A helper child that exits immediately must (a) be reported dead by
    /// `is_alive()` and (b) make `embed()` return an error FAST — the whole
    /// point of Plan 05. The old code blocked `recv_timeout` for the full
    /// batch ceiling; this test would take minutes if the fix regressed, so it
    /// asserts the call returns in well under the (short, test-only) timeout.
    #[test]
    fn dead_helper_is_detected_and_embed_fails_fast() {
        let dir = tempfile::tempdir().expect("tempdir");
        let helper = write_fake_helper(dir.path(), "#!/bin/sh\nexit 0\n");
        let embedder =
            BrainEmbedder::spawn_with_helper(&helper, fake_model()).expect("spawn fake helper");

        // is_alive() flips to false once the child exits (poll up to ~2s).
        let mut alive_after_exit = true;
        for _ in 0..40 {
            if !embedder.is_alive() {
                alive_after_exit = false;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            !alive_after_exit,
            "is_alive() must report the exited child as dead"
        );

        // embed() must error rather than hang: the child is gone, so writing to
        // its closed stdin fails immediately (and the reader saw EOF), well
        // inside the short test timeout.
        let started = std::time::Instant::now();
        let result = embedder.embed_with_timeout(vec!["x".to_string()], Duration::from_secs(2));
        let elapsed = started.elapsed();
        assert!(
            result.is_err(),
            "embed against a dead helper must return Err"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "embed must fail fast, not block on the timeout: took {elapsed:?}"
        );
    }

    /// The model fingerprint must be keyed on file CONTENT, not mtime. Rewriting
    /// the identical bytes (which bumps mtime) must NOT change the fingerprint —
    /// that stability is the whole point of the fix (mtime churn triggered
    /// re-embed storms). Changing a byte in the first 1 MiB must change it.
    #[test]
    fn fingerprint_is_content_stable_across_mtime_bumps() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("model.gguf");

        let bytes = vec![0xABu8; 4096];
        std::fs::write(&path, &bytes).expect("write model");
        let first = fingerprint_model_file(&path);

        // Rewrite the same bytes after a delay so mtime advances but content is
        // identical. The fingerprint must be unchanged.
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&path, &bytes).expect("rewrite identical model");
        let after_mtime_bump = fingerprint_model_file(&path);
        assert_eq!(
            first, after_mtime_bump,
            "fingerprint must be stable across an mtime bump of identical bytes"
        );

        // Flip one byte inside the first 1 MiB: fingerprint must differ.
        let mut changed = bytes.clone();
        changed[0] = 0x00;
        std::fs::write(&path, &changed).expect("write changed model");
        let after_content_change = fingerprint_model_file(&path);
        assert_ne!(
            first, after_content_change,
            "fingerprint must change when file content changes"
        );
    }
}
