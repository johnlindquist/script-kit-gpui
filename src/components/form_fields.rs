//! Native Form Field Components for GPUI Script Kit
//!
//! This module provides reusable form field components for rendering HTML form fields
//! as native GPUI elements. Components include:
//!
//! - [`FormTextField`] - Text input for text/password/email/number types
//! - [`FormTextArea`] - Multi-line text input
//! - [`FormCheckbox`] - Checkbox with label
//!
//!
//! # Design Patterns
//!
//! All components follow these patterns:
//! - **Colors struct**: Pre-computed colors (Copy/Clone) for efficient closure use
//! - **FocusHandle**: Each component manages its own focus for Tab navigation
//! - **Value state**: Components maintain their own value state
//! - **IntoElement trait**: Compatible with GPUI's element system

#![allow(dead_code)]

#[path = "form_fields/checkbox.rs"]
mod checkbox;
#[path = "form_fields/colors.rs"]
mod colors;
#[path = "form_fields/helpers.rs"]
mod helpers;
#[path = "form_fields/shell.rs"]
mod shell;
#[path = "form_fields/state.rs"]
mod state;
#[path = "form_fields/text_area/mod.rs"]
mod text_area;
#[path = "form_fields/text_field/mod.rs"]
mod text_field;

pub use checkbox::FormCheckbox;
pub use colors::{FormFieldColors, FormFieldMetrics};
pub(crate) use helpers::form_field_type_allows_candidate_value;
pub(crate) use shell::{
    apply_form_field_shell_test_fixture, menu_syntax_form_field_shell_spec,
    render_form_field_shell, resolve_form_field_shell_style, FormFieldShellSpec,
    FormFieldShellStyle, FormFieldValidation,
};
pub use state::FormFieldState;
pub use text_area::FormTextArea;
pub use text_field::FormTextField;

// Note: Full GPUI component tests require the test harness which has macro recursion
// limit issues. The form field components are integration-tested via the main
// application's form prompt rendering. Unit tests for helper functions are in
// src/components/form_fields_tests.rs.
