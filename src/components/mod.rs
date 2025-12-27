//! Reusable UI Components for GPUI Script Kit
//!
//! This module provides a collection of reusable, theme-aware UI components
//! that follow consistent patterns across the application.
//!
//! # Components
//!
//! - [`Button`] - Interactive button with variants (Primary, Ghost, Icon)
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

// Re-export commonly used types
pub use button::{Button, ButtonColors, ButtonVariant};
