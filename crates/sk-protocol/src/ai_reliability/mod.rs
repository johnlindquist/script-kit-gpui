//! Pure, app-independent reliability decisions for AI-backed surfaces.
//!
//! This module intentionally contains no provider, process, filesystem, UI,
//! clock, randomness, logging, or persistence effects. Callers interpret the
//! typed commands returned by [`transition`].

mod reducer;
mod types;

#[cfg(test)]
mod model_tests;

pub use reducer::{recovery_plan_for, transition};
pub use types::*;
