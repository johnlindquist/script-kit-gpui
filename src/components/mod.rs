//! Reusable UI Components for GPUI Script Kit
//!
//! This module provides a collection of reusable, theme-aware UI components
//! that follow consistent patterns across the application.
//!
//! # Components
//!
//! - [`Button`] - Interactive button with variants (Primary, Ghost, Icon)
//! - [`Toast`] - Toast notification with variants (Success, Warning, Error, Info)
//! - [`Scrollbar`] - Minimal native-style scrollbar for overlay on lists
//! - [`FormTextField`] - Text input for text/password/email/number types
//! - [`FormTextArea`] - Multi-line text input
//! - [`FormCheckbox`] - Checkbox with label
//! - [`PromptHeader`] - Header component with search input, buttons, and logo
//! - [`PromptContainer`] - Container component for consistent prompt window layout
//!
//!
//! # Design Patterns
//!
//! All components follow these patterns:
//! - **Colors struct**: Pre-computed colors (Copy/Clone) for efficient closure use
//! - **Builder pattern**: Fluent API with `.method()` chaining
//! - **IntoElement trait**: Compatible with GPUI's element system
//! - **Theme integration**: Use `from_theme()` or `from_design()` for colors

pub mod button;
pub mod form_fields;
pub mod prompt_container;
pub mod prompt_header;
pub mod scrollbar;
pub mod text_input;
pub mod toast;

// Re-export commonly used types
pub use button::{Button, ButtonColors, ButtonVariant};
#[allow(unused_imports)]
pub use form_fields::{FormCheckbox, FormFieldColors, FormFieldState, FormTextArea, FormTextField};
#[allow(unused_imports)]
pub use scrollbar::{
    Scrollbar, ScrollbarColors, MIN_THUMB_HEIGHT, SCROLLBAR_PADDING, SCROLLBAR_WIDTH,
};
// These re-exports form the public API - allow unused since not all are used in every crate
#[allow(unused_imports)]
pub use prompt_container::{PromptContainer, PromptContainerColors, PromptContainerConfig};
#[allow(unused_imports)]
pub use prompt_header::{PromptHeader, PromptHeaderColors, PromptHeaderConfig};
#[allow(unused_imports)]
pub use text_input::{TextInputState, TextSelection};
#[allow(unused_imports)]
pub use toast::{Toast, ToastAction, ToastColors, ToastVariant};
