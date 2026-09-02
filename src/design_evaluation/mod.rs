//! Owned execution of the production application roots over the existing JSONL protocol.
//! Prompt construction is also used by ordinary SDK requests; native evaluation
//! remains an explicit build feature and never falls through to normal startup.

pub(crate) mod prompt_fixtures;
pub(crate) mod search_fixtures;

#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod bootstrap;
#[cfg(all(any(test, feature = "owned-ui-evaluation"), target_os = "macos"))]
mod catalog;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod conversation_fixtures;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod dictation_fixtures;
#[cfg(all(any(test, feature = "owned-ui-evaluation"), target_os = "macos"))]
pub(crate) mod fixture_ids;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
pub(crate) mod main_fixtures;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod notes_fixtures;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod runtime;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod runtime_actions;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod runtime_deferred;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod runtime_frames;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod runtime_query;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod runtime_safety;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod runtime_sdk;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod runtime_theme;
#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
mod secondary_fixtures;

pub(crate) fn run_if_requested() -> Option<anyhow::Result<()>> {
    let requested = std::env::args_os()
        .skip(1)
        .any(|arg| arg == "--owned-ui-evaluation")
        || std::env::var_os("SCRIPT_KIT_OWNED_EVALUATION").is_some();
    if !requested {
        return None;
    }
    #[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
    {
        Some(bootstrap::run())
    }
    #[cfg(not(all(feature = "owned-ui-evaluation", target_os = "macos")))]
    {
        Some(Err(anyhow::anyhow!("owned_evaluation_feature_unavailable")))
    }
}
