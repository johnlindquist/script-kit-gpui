//! Crash- and concurrency-safe file writes.
//!
//! The naive "write to `<path>.tmp`, then rename" idiom is atomic against a
//! crash, but NOT against concurrent writers: two savers that share one FIXED
//! temp path (`file.json.tmp`) interleave their `write` calls into the same
//! file, so the renamed result can be a torn mix of both payloads even though
//! each rename is itself atomic. That is reachable across processes (two app
//! instances sharing `~/.scriptkit/`, or a crash-relaunch overlap — no in-process
//! mutex spans processes) and, for un-serialized writers like the window-state
//! saver, within a process too. A reproduction of the fixed-temp pattern under
//! 8 concurrent writers produced thousands of torn reads.
//!
//! [`write_atomic`] avoids this by giving every write its own UNIQUE temp file
//! (via `tempfile`) in the destination directory, then persisting (renaming) it
//! over the target. Concurrent writers therefore never share a temp file, so the
//! final file is always exactly one writer's complete output (clean
//! last-writer-wins), never a torn mix.

use std::io::Write;
use std::path::Path;

/// Atomically write `bytes` to `path` using a unique temp file + rename.
///
/// Creates the parent directory if needed. On Unix the rename is atomic; a
/// concurrent reader always sees either the old file or one writer's complete
/// new file, never a partial/torn one.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent)?;
    }
    let dir = parent.unwrap_or_else(|| Path::new("."));

    let mut temp = tempfile::Builder::new()
        .prefix(".sk-atomic-")
        .suffix(".tmp")
        .tempfile_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    // `persist` renames the unique temp over `path`; on failure the temp is
    // cleaned up by `TempPath`'s drop rather than left as litter.
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_atomic;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn write_atomic_never_tears_under_concurrent_writers() {
        // Regression: the previous fixed-temp-path pattern
        // (`path.with_extension("json.tmp")` + write + rename), shared by
        // input_history / frecency / window_state, corrupted the destination
        // when >1 saver ran at once (a standalone repro of that exact pattern
        // produced thousands of torn reads). `write_atomic` uses a unique temp
        // per call, so a concurrent reader must always see one writer's COMPLETE
        // payload — never a mix.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = Arc::new(dir.path().join("state.json"));
        // Two distinct, differently-sized valid payloads (large enough to need
        // multiple write syscalls, which is where tearing happened).
        let a: Arc<String> = Arc::new(format!(
            "{{\"who\":\"A\",\"v\":[{}]}}",
            (0..1500)
                .map(|i| format!("\"aaaaaaaaaa-{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        ));
        let b: Arc<String> = Arc::new(format!(
            "{{\"who\":\"B\",\"v\":[{}]}}",
            (0..800)
                .map(|i| format!("\"bbbbbbbbbbbbbbbbbbbb-{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        ));
        let torn = Arc::new(AtomicUsize::new(0));
        let iters = 1500usize;

        let mut handles = Vec::new();
        for t in 0..8 {
            let (path, a, b) = (path.clone(), a.clone(), b.clone());
            handles.push(std::thread::spawn(move || {
                for _ in 0..iters {
                    let payload = if t % 2 == 0 { &*a } else { &*b };
                    write_atomic(&path, payload.as_bytes()).expect("write_atomic");
                }
            }));
        }
        let reader = {
            let (path, a, b, torn) = (path.clone(), a.clone(), b.clone(), torn.clone());
            std::thread::spawn(move || {
                for _ in 0..(iters * 6) {
                    if let Ok(s) = std::fs::read_to_string(&*path) {
                        if !s.is_empty() && s != *a && s != *b {
                            torn.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
        };
        for h in handles {
            h.join().unwrap();
        }
        reader.join().unwrap();

        assert_eq!(
            torn.load(Ordering::Relaxed),
            0,
            "write_atomic produced a torn/mixed file under concurrent writers"
        );
        // And the final file is one complete, parseable payload.
        let final_contents = std::fs::read_to_string(&*path).expect("final read");
        assert!(final_contents == *a || final_contents == *b);
    }
}
