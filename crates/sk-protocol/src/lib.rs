//! Stable, app-independent protocol primitives for Script Kit.

pub mod ai_reliability;
pub mod command_contract;
pub mod execution_contract;
pub mod latency_contract;
pub mod search_contract;
mod semantic_id;
pub mod sentence_search;

pub use semantic_id::{generate_semantic_id, generate_semantic_id_named, value_to_slug};
