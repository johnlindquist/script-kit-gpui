//! Stable, app-independent protocol primitives for Script Kit.

pub mod ai_reliability;
mod semantic_id;

pub use semantic_id::{generate_semantic_id, generate_semantic_id_named, value_to_slug};
