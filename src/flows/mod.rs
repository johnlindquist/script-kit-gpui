//! Flow-first launcher domain (mdflow front end).
//!
//! mdflow (the standalone CLI) owns flow discovery, resolution, and
//! execution; this module is the app-side client of the frozen contract in
//! `docs/ai/flow-ux-protocol.md`:
//!
//! - [`catalog`] — `md roster --json` per-cwd cache.
//! - [`package_source`] — installed `@johnlindquist/flows` corpus with true
//!   provenance and bun-linked wrapper commands.
//! - [`explain_cache`] — free `md explain --json` previews.
//! - [`runner`] — `md <flow> --events` run-once spawn + NDJSON reader.
//! - [`run_registry`] — the single source of truth for run-once state.
//! - [`codex_client`] — persistent `codex app-server` JSON-RPC client, the
//!   native transport for codex-engine flow conversations.
//! - [`session`] — conversational session metadata (Enter = converse; the
//!   live `ChatPrompt` entities live on `ScriptListApp`).
//! - [`router`] — Tab flow router: free text → best flow.
//! - [`automation`] — the `flowUx` getState payload for devtools receipts.
//! - [`eval_ledger`] — fail-closed verification-freshness verdicts over
//!   mdflow's eval trust ledger (`~/.mdflow/eval-results.json`).
//!
//! Design rule: the desk is a thin renderer over the registry/catalog.
//! UI changes must not touch the run lifecycle.

pub mod automation;
pub mod catalog;
pub mod codex_client;
pub mod eval_ledger;
pub mod explain_cache;
pub mod model;
pub mod package_source;
pub mod router;
pub mod run_registry;
pub mod runner;
pub mod session;

/// Last cwd a flow was launched from; updated on every launch so flow
/// surfaces follow the user's working context. Retained from the removed
/// detached Flow Manager window — the Flow Desk still resolves through it.
static LAST_FLOW_CWD: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn remember_flow_cwd(cwd: &str) {
    if let Ok(mut guard) = LAST_FLOW_CWD.lock() {
        *guard = Some(cwd.to_string());
    }
}

pub fn last_flow_cwd() -> Option<String> {
    LAST_FLOW_CWD.lock().ok().and_then(|guard| guard.clone())
}

/// THE cwd resolver for every flow surface (Flow UX variations, automation
/// snapshots). Precedence:
/// 1. `SCRIPT_KIT_FLOW_UX_CWD` (probe/test seam),
/// 2. the caller's context cwd (spine chip) when it has one,
/// 3. the last cwd a flow was launched from,
/// 4. `$HOME`.
///
/// Surfaces that disagree about cwd show different flow lists for no visible
/// reason — never resolve cwd any other way.
pub fn resolve_flow_cwd(context_cwd: Option<String>) -> String {
    if let Ok(dir) = std::env::var("SCRIPT_KIT_FLOW_UX_CWD") {
        if !dir.is_empty() {
            return dir;
        }
    }
    if let Some(cwd) = context_cwd {
        if !cwd.is_empty() {
            return cwd;
        }
    }
    if let Some(remembered) = last_flow_cwd() {
        return remembered;
    }
    std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
}
