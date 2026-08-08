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
//! - [`PromptFooter`] - Footer component with logo, primary/secondary action buttons
//! - [`PromptContainer`] - Container component for consistent prompt window layout
//! - [`ShortcutRecorder`] - Modal for recording keyboard shortcuts with visual feedback
//! - [`AliasInput`] - Modal for entering command aliases with keyboard focus
//!
//!
//! # Design Patterns
//!
//! All components follow these patterns:
//! - **Colors struct**: Pre-computed colors (Copy/Clone) for efficient closure use
//! - **Builder pattern**: Fluent API with `.method()` chaining
//! - **IntoElement trait**: Compatible with GPUI's element system
//! - **Theme integration**: Use `from_theme()` or `from_design()` for colors

pub mod ai_recovery;
pub mod alias_input;
pub(crate) mod braille_loading;
pub(crate) mod builtin_leading_separator;
pub mod button;
pub(crate) mod confirm_modal_shell;
/// Shared per-turn conversation action affordances (response copy control).
pub(crate) mod conversation_actions;
/// Shared typed owner of conversation paint values for Agent Chat AND Flow.
/// All callers import this canonical owner directly.
pub mod conversation_style;
/// Shared selectable Markdown renderer for every conversation surface.
/// Selection is opt-in on the vendored `TextView`, so building one outside
/// this module silently produces unselectable text.
pub(crate) mod conversation_text;
#[cfg(test)]
mod error_handling_audit_tests;
pub mod focusable_prompt_wrapper;
pub(crate) mod footer_chrome;
pub mod form_fields;
#[cfg(test)]
mod form_fields_tests;
pub mod hint_strip;
pub(crate) mod info_state;
pub(crate) mod inline_dropdown;
pub mod inline_picker;
pub mod inline_popup_window;
pub mod inline_prompt_input;
pub(crate) mod launcher_ask_ai_hint;
pub(crate) mod list_scroll_affordance;
pub(crate) mod main_view_chrome;
pub mod minimal_prompt_shell;
pub(crate) mod modal_dim;
pub(crate) mod navigation_affordance;
pub(crate) mod non_list_state;
pub mod notes_editor;
pub(crate) mod overlay_modal;
pub mod prompt_container;
pub mod prompt_footer;
pub mod prompt_layout_shell;
pub(crate) mod resource_preview;
pub mod scrollbar;
pub mod section_divider;
pub mod shortcut_recorder;
pub mod text_input;
pub mod toast;
pub mod unified_list_item;
#[cfg(test)]
mod unified_list_item_tests;

// Re-export commonly used types
pub use ai_recovery::{
    ai_recovery_footer_hint_labels, decide_recovery_key, recovery_semantic_tree,
    render_ai_recovery_card, render_ai_recovery_footer, AiRecoveryActionHandler,
    AiRecoveryCardHandlers, AiRecoveryDismissHandler, AiRecoveryFocusTarget, AiRecoveryKey,
    AiRecoveryKeyDecision, AiRecoverySemanticNode, AI_RECOVERY_FOOTER_ID,
};
#[allow(unused_imports)]
pub use alias_input::{AliasInput, AliasInputAction, AliasInputColors};
pub use button::{Button, ButtonColors, ButtonVariant};
#[allow(unused_imports)]
pub(crate) use confirm_modal_shell::{
    confirm_modal_header, confirm_modal_shell, modal_action_row, ConfirmModalShellConfig,
    ModalActionRowButton, CONFIRM_MODAL_RADIUS, CONFIRM_MODAL_SHELL_ID,
    MODAL_ACTION_DISABLED_OPACITY, MODAL_ACTION_ROW_TOP_MARGIN_PX, MODAL_WIDTH_PX,
    PARENT_MODAL_WIDTH_PX,
};
#[allow(unused_imports)]
pub use focusable_prompt_wrapper::{
    match_focusable_prompt_intercepted_key, FocusablePrompt, FocusablePromptInterceptedKey,
};
#[allow(unused_imports)]
pub(crate) use form_fields::{
    apply_form_field_shell_test_fixture, menu_syntax_form_field_shell_spec,
    render_form_field_shell, resolve_form_field_shell_style, FormFieldShellSpec,
    FormFieldShellStyle, FormFieldValidation,
};
#[allow(unused_imports)]
pub use form_fields::{
    FormCheckbox, FormFieldColors, FormFieldMetrics, FormFieldState, FormTextArea, FormTextField,
};
#[allow(unused_imports)]
pub use hint_strip::{
    render_hint_icons_clickable, render_selectable_hint_icons, render_static_hint_icons,
    render_static_hint_icons_hsla, ClickableHint, HintStrip, SelectableHint,
};
#[allow(unused_imports)]
pub(crate) use info_state::{
    agent_chat_empty_guidance_spec, agent_setup_info_spec, info_metrics, info_palette,
    launcher_empty_or_no_results_spec, permission_onboarding_intro_spec,
    render_agent_chat_empty_guidance, render_info_state, render_info_state_full_width_panel,
    render_launcher_empty_or_no_results, render_shared_empty_state, render_simple_empty_state,
    shared_empty_state_spec, simple_empty_state_spec, InfoCue, InfoCueSemanticSnapshot,
    InfoCueValidationError, InfoEmptySurface, InfoGuidanceItem, InfoMetrics, InfoSection,
    InfoStateDensity, InfoStateLayout, InfoStateSemanticSnapshot, InfoStateSpec, InfoStateTone,
    InfoTextMetric, InfoTonePresentation, InfoTypeScale, INFO_SPACING, INFO_TYPE_SCALE,
};
#[allow(unused_imports)]
pub(crate) use inline_dropdown::{
    inline_dropdown_clamp_selected_index, inline_dropdown_select_next, inline_dropdown_select_prev,
    inline_dropdown_visible_range, inline_dropdown_visible_range_from_start, InlineDropdown,
    InlineDropdownColors, InlineDropdownEmptyState, InlineDropdownSynopsis,
};
#[allow(unused_imports)]
pub use inline_prompt_input::InlinePromptInput;
#[allow(unused_imports)]
pub(crate) use launcher_ask_ai_hint::render_launcher_ask_ai_hint;
#[allow(unused_imports)]
pub use minimal_prompt_shell::MinimalPromptShell;
#[allow(unused_imports)]
pub(crate) use navigation_affordance::render_back_affordance;
#[allow(unused_imports)]
pub(crate) use non_list_state::{
    NonListComposition, NonListCompositionOwner, NonListDensity, NonListMetrics, NonListPalette,
};
#[allow(unused_imports)]
pub use notes_editor::{
    NotesEditor, NotesEditorConfig, NotesEditorInputSizing, NotesEditorLayout,
    NotesEditorMarkdownConfig,
};
#[allow(unused_imports)]
pub use scrollbar::{
    Scrollbar, ScrollbarColors, MIN_THUMB_HEIGHT, SCROLLBAR_PADDING, SCROLLBAR_WIDTH,
};
// These re-exports form the public API - allow unused since not all are used in every crate
#[allow(unused_imports)]
pub use prompt_container::{PromptContainer, PromptContainerColors, PromptContainerConfig};
#[allow(unused_imports)]
pub use prompt_footer::{PromptFooter, PromptFooterColors, PromptFooterConfig};
#[allow(unused_imports)]
pub(crate) use prompt_layout_shell::{
    editor_prompt_hints, emit_prompt_chrome_audit, emit_prompt_hint_audit,
    emit_surface_prompt_hint_audit, is_universal_prompt_hints,
    render_footer_actions_agent_button_frames, render_universal_footer_action_button_frames,
    render_universal_footer_action_buttons, render_universal_footer_action_rail,
    render_universal_prompt_hint_strip, render_universal_prompt_hint_strip_clickable,
    render_universal_prompt_hint_strip_clickable_with_primary_key_label,
    render_universal_prompt_hint_strip_clickable_with_primary_label, template_prompt_hints,
    universal_prompt_hints, universal_prompt_hints_with_primary_key_label,
    universal_prompt_hints_with_primary_label, PromptChromeAudit, PromptHintAudit,
    UNIVERSAL_PROMPT_HINT_COUNT,
};
#[allow(unused_imports)]
pub(crate) use prompt_layout_shell::{
    prompt_detail_card, prompt_field_style, prompt_form_help, prompt_form_intro,
    prompt_form_section, prompt_scroll_value, prompt_scroll_value_with_id,
    prompt_single_line_control_metrics, prompt_single_line_control_surface, prompt_surface,
    prompt_text_field, prompt_text_palette, render_inset_prompt_body, PromptBodyInsets,
    PromptFieldState, PromptFieldStyle, PromptSingleLineControlMetrics, PromptTextPalette,
    ResolvedPromptBodyInsets,
};
#[allow(unused_imports)]
pub use prompt_layout_shell::{prompt_shell_container, prompt_shell_content};
#[allow(unused_imports)]
pub(crate) use prompt_layout_shell::{
    render_expanded_view_prompt_shell, render_expanded_view_scaffold,
    render_expanded_view_scaffold_with_footer, render_expanded_view_scaffold_with_hints,
    render_hint_strip_leading_text, render_minimal_list_prompt_scaffold,
    render_minimal_list_prompt_shell, render_minimal_list_prompt_shell_with_footer,
    render_simple_hint_strip, render_simple_prompt_shell,
};
#[allow(unused_imports)]
pub use section_divider::SectionDivider;
#[allow(unused_imports)]
pub use shortcut_recorder::{
    RecordedShortcut, ShortcutConflict, ShortcutRecorder, ShortcutRecorderColors,
};
#[allow(unused_imports)]
pub use text_input::{TextInputState, TextSelection};
#[allow(unused_imports)]
pub use toast::{Toast, ToastAction, ToastActionId, ToastColors, ToastId, ToastVariant};
#[allow(unused_imports)]
pub use unified_list_item::{
    Density, ItemState, LeadingContent, ListItemLayout, SectionHeader, TextContent,
    TrailingContent, UnifiedListItem, UnifiedListItemColors, SECTION_HEADER_HEIGHT,
};
