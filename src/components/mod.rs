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
//!
//! # Usage
//!
//! ```ignore
//! use crate::components::{Button, ButtonColors, ButtonVariant};
//!
//! let colors = ButtonColors::from_theme(&theme);
//! let button = Button::new("Run", colors)
//!     .variant(ButtonVariant::Primary)
//!     .shortcut("↵")
//!     .on_click(Box::new(|_, _, _| println!("Clicked!")));
//!
//! // Toast example
//! use crate::components::{Toast, ToastColors, ToastVariant};
//!
//! let toast_colors = ToastColors::from_theme(&theme, ToastVariant::Error);
//! let toast = Toast::new("An error occurred", toast_colors)
//!     .variant(ToastVariant::Error)
//!     .details("Stack trace here...")
//!     .dismissible(true);
//! ```
//!
//! # Design Patterns
//!
//! All components follow these patterns:
//! - **Colors struct**: Pre-computed colors (Copy/Clone) for efficient closure use
//! - **Builder pattern**: Fluent API with `.method()` chaining
//! - **IntoElement trait**: Compatible with GPUI's element system
//! - **Theme integration**: Use `from_theme()` or `from_design()` for colors

pub mod button;
pub mod scrollbar;
pub mod toast;

// Re-export commonly used types
pub use button::{Button, ButtonColors, ButtonVariant};
#[allow(unused_imports)]
pub use scrollbar::{Scrollbar, ScrollbarColors, SCROLLBAR_WIDTH, MIN_THUMB_HEIGHT, SCROLLBAR_PADDING};
// These re-exports form the public API - allow unused since not all are used in every crate
#[allow(unused_imports)]
pub use toast::{Toast, ToastAction, ToastColors, ToastVariant};
