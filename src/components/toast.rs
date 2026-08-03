//! Reusable Toast component for GPUI Script Kit
//!
//! This module provides a theme-aware toast notification component with multiple variants
//! (Success, Warning, Error, Info) and support for auto-dismiss, action buttons, and
//! expandable details.
//!

#[path = "toast/constructors.rs"]
mod constructors;
#[path = "toast/model.rs"]
mod model;
#[path = "toast/render.rs"]
mod render;
#[cfg(test)]
#[path = "toast/tests.rs"]
mod tests;
#[path = "toast/types.rs"]
mod types;

pub use model::Toast;
pub use types::{
    ToastAction, ToastActionId, ToastColors, ToastDismissCallback, ToastId, ToastVariant,
};
