//! Stable, app-independent protocol primitives for Script Kit.

pub mod ai_reliability;
pub mod ascii_search;
pub mod command_contract;
pub mod execution_contract;
pub mod filter_coalescer;
pub mod latency_contract;
pub mod query_prefix;
pub mod search_contract;
pub mod search_primitives;
mod semantic_id;
pub mod sentence_search;

pub use semantic_id::{generate_semantic_id, generate_semantic_id_named, value_to_slug};
