use gpui::{div, px, rgb, AnyElement, ElementId, FontWeight, Rgba, SharedString};
use gpui::{InteractiveElement, IntoElement, ParentElement, Styled};

use super::{FormFieldColors, FormFieldMetrics};

/// Explicit validation state for the shared form-field shell.
///
/// Neutral is intentionally distinct from Valid: the absence of a validation
/// result must never be painted or exposed as successful validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FormFieldValidation {
    Neutral,
    Valid,
    Invalid { message: SharedString },
}

impl FormFieldValidation {
    fn validate(&self) -> Result<(), &'static str> {
        if let Self::Invalid { message } = self {
            if message.trim().is_empty() {
                return Err("invalid form fields require a non-empty validation message");
            }
        }
        Ok(())
    }

    pub(crate) fn status_kind(&self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Valid => "valid",
            Self::Invalid { .. } => "invalid",
        }
    }
}

/// Renderer-neutral anatomy and state for one labeled form field.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FormFieldShellSpec {
    pub(crate) semantic_id: SharedString,
    pub(crate) label: Option<SharedString>,
    pub(crate) focused: bool,
    pub(crate) disabled: bool,
    pub(crate) disabled_reason: Option<SharedString>,
    pub(crate) validation: FormFieldValidation,
    pub(crate) multiline: bool,
    pub(crate) min_height: f32,
    pub(crate) max_height: Option<f32>,
}

impl FormFieldShellSpec {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        semantic_id: impl Into<SharedString>,
        label: Option<SharedString>,
        focused: bool,
        disabled: bool,
        disabled_reason: Option<SharedString>,
        validation: FormFieldValidation,
        multiline: bool,
        min_height: f32,
        max_height: Option<f32>,
    ) -> Result<Self, &'static str> {
        let semantic_id = semantic_id.into();
        if semantic_id.trim().is_empty() {
            return Err("form field semantic IDs cannot be blank");
        }
        if label.as_ref().is_some_and(|label| label.trim().is_empty()) {
            return Err("form field labels cannot be blank");
        }
        if !min_height.is_finite() || min_height <= 0.0 {
            return Err("form field minimum height must be positive and finite");
        }
        if max_height.is_some_and(|max_height| !max_height.is_finite() || max_height < min_height) {
            return Err("form field maximum height must be finite and at least the minimum");
        }
        validation.validate()?;
        match (disabled, disabled_reason.as_ref()) {
            (true, Some(reason)) if reason.trim().is_empty() => {
                return Err("disabled form fields require a non-empty reason")
            }
            (true, None) => return Err("disabled form fields require a reason"),
            (false, Some(_)) => return Err("enabled form fields cannot carry a disabled reason"),
            _ => {}
        }
        if disabled && validation != FormFieldValidation::Neutral {
            return Err("disabled form fields cannot also claim a validation result");
        }

        Ok(Self {
            semantic_id,
            label,
            focused: focused && !disabled,
            disabled,
            disabled_reason,
            validation,
            multiline,
            min_height,
            max_height,
        })
    }

    pub(crate) fn neutral(
        semantic_id: impl Into<SharedString>,
        label: Option<SharedString>,
        focused: bool,
        multiline: bool,
        min_height: f32,
        max_height: Option<f32>,
    ) -> Self {
        Self::try_new(
            semantic_id,
            label,
            focused,
            false,
            None,
            FormFieldValidation::Neutral,
            multiline,
            min_height,
            max_height,
        )
        .expect("neutral form field shell must be valid")
    }

    pub(crate) fn supporting_message(&self) -> Option<&SharedString> {
        if let Some(reason) = self.disabled_reason.as_ref() {
            return Some(reason);
        }
        match &self.validation {
            FormFieldValidation::Invalid { message } => Some(message),
            FormFieldValidation::Neutral | FormFieldValidation::Valid => None,
        }
    }

    pub(crate) fn editable(&self) -> bool {
        !self.disabled
    }

    pub(crate) fn surface_id(&self) -> SharedString {
        format!("{}:surface", self.semantic_id).into()
    }

    pub(crate) fn label_id(&self) -> SharedString {
        format!("{}:label", self.semantic_id).into()
    }

    pub(crate) fn supporting_message_id(&self) -> SharedString {
        format!("{}:message", self.semantic_id).into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FormFieldShellStyle {
    pub(crate) background: Rgba,
    pub(crate) border: Rgba,
    pub(crate) text: Rgba,
    pub(crate) placeholder: Rgba,
    pub(crate) label: Rgba,
    pub(crate) supporting: Rgba,
}

pub(crate) fn resolve_form_field_shell_style(
    spec: &FormFieldShellSpec,
    colors: FormFieldColors,
) -> FormFieldShellStyle {
    let surface = colors.whisper_surface(spec.focused);
    let disabled = spec.disabled;
    let invalid = matches!(spec.validation, FormFieldValidation::Invalid { .. });
    FormFieldShellStyle {
        background: surface.background,
        border: if invalid {
            rgb(colors.error)
        } else {
            surface.border
        },
        text: if disabled {
            colors.disabled
        } else {
            colors.text
        },
        placeholder: if disabled {
            colors.disabled
        } else {
            colors.placeholder
        },
        label: if disabled {
            colors.disabled
        } else {
            colors.label
        },
        supporting: if invalid {
            rgb(colors.error)
        } else {
            colors.disabled
        },
    }
}

/// Render the one shared label/surface/supporting-message anatomy.
///
/// The body must be borderless. This shell is the sole owner of field border,
/// background, radius, padding, and validation/disabled supporting copy.
pub(crate) fn render_form_field_shell(
    spec: &FormFieldShellSpec,
    colors: FormFieldColors,
    metrics: FormFieldMetrics,
    body: AnyElement,
) -> AnyElement {
    let style = resolve_form_field_shell_style(spec, colors);
    let mut surface = div()
        .id(ElementId::Name(spec.surface_id()))
        .w_full()
        .min_h(px(spec.min_height))
        .px(px(metrics.field_padding_x_px))
        .bg(style.background)
        .border_1()
        .border_color(style.border)
        .rounded(px(metrics.field_radius_px));

    if spec.multiline {
        surface = surface.py(px(metrics.field_padding_y_px));
        if let Some(max_height) = spec.max_height {
            surface = surface.max_h(px(max_height));
        }
    } else {
        surface = surface
            .h(px(spec.min_height))
            .max_h(px(spec.min_height))
            .flex()
            .items_center();
    }

    let mut root = div()
        .id(ElementId::Name(spec.semantic_id.clone()))
        .w_full()
        .flex()
        .flex_col()
        .gap(px(metrics.field_gap_px));

    if let Some(label) = spec.label.as_ref() {
        root = root.child(
            div()
                .id(ElementId::Name(spec.label_id()))
                .text_size(px(metrics.label_font_size))
                .line_height(px(metrics.label_line_height))
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.label)
                .child(label.clone()),
        );
    }

    root = root.child(surface.child(body));
    if let Some(message) = spec.supporting_message() {
        root = root.child(
            div()
                .id(ElementId::Name(spec.supporting_message_id()))
                .text_size(px(metrics.label_font_size))
                .line_height(px(metrics.label_line_height))
                .text_color(style.supporting)
                .child(message.clone()),
        );
    }
    root.into_any_element()
}

/// Map the menu-syntax domain snapshot into the same shell model consumed by
/// rendering and semantic collection.
pub(crate) fn menu_syntax_form_field_shell_spec(
    target: &str,
    field: &crate::menu_syntax::MenuSyntaxFormFieldSnapshot,
    metrics: FormFieldMetrics,
) -> FormFieldShellSpec {
    let validation = if field.required && !field.satisfied {
        FormFieldValidation::Invalid {
            message: if field.value.trim().is_empty() {
                "Required".into()
            } else {
                "Check this value".into()
            },
        }
    } else {
        FormFieldValidation::Neutral
    };
    let (min_height, max_height) = if field.multiline {
        (
            metrics.menu_syntax_multiline_min_height_px(),
            Some(metrics.menu_syntax_multiline_max_height_px()),
        )
    } else {
        (metrics.menu_syntax_single_line_height_px(), None)
    };
    let mut spec = FormFieldShellSpec::try_new(
        format!("handler-form:{target}:{}", field.id),
        Some(field.label.clone().into()),
        field.focused,
        false,
        None,
        validation,
        field.multiline,
        min_height,
        max_height,
    )
    .expect("menu-syntax form field shell must be valid");
    apply_form_field_shell_test_fixture(&mut spec);
    spec
}

/// Deterministic runtime-only state fixtures for disabled/invalid shell proof.
pub(crate) fn apply_form_field_shell_test_fixture(spec: &mut FormFieldShellSpec) {
    if std::env::var_os("SCRIPT_KIT_TEST_STATUS").is_none() {
        return;
    }
    match std::env::var("SCRIPT_KIT_TEST_FORM_FIELD_FIXTURE").as_deref() {
        Ok("disabled") => {
            spec.focused = false;
            spec.disabled = true;
            spec.disabled_reason = Some("Unavailable in this test fixture".into());
            spec.validation = FormFieldValidation::Neutral;
        }
        Ok("invalid") => {
            spec.disabled = false;
            spec.disabled_reason = None;
            spec.validation = FormFieldValidation::Invalid {
                message: "Check this value".into(),
            };
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral() -> FormFieldShellSpec {
        FormFieldShellSpec::neutral("field:body", Some("Body".into()), true, false, 38.0, None)
    }

    #[test]
    fn neutral_is_not_implicitly_valid() {
        let spec = neutral();
        assert_eq!(spec.validation.status_kind(), "neutral");
        assert!(spec.supporting_message().is_none());
        assert!(spec.editable());
    }

    #[test]
    fn disabled_fields_require_a_reason_and_drop_focus() {
        assert!(FormFieldShellSpec::try_new(
            "field:body",
            Some("Body".into()),
            true,
            true,
            None,
            FormFieldValidation::Neutral,
            false,
            38.0,
            None,
        )
        .is_err());
        let spec = FormFieldShellSpec::try_new(
            "field:body",
            Some("Body".into()),
            true,
            true,
            Some("Read only".into()),
            FormFieldValidation::Neutral,
            false,
            38.0,
            None,
        )
        .unwrap();
        assert!(!spec.focused);
        assert!(!spec.editable());
        assert_eq!(
            spec.supporting_message().map(AsRef::as_ref),
            Some("Read only")
        );
    }

    #[test]
    fn validation_requires_visible_copy_and_cannot_hide_behind_border_only() {
        assert!(FormFieldShellSpec::try_new(
            "field:body",
            Some("Body".into()),
            false,
            false,
            None,
            FormFieldValidation::Invalid { message: "".into() },
            false,
            38.0,
            None,
        )
        .is_err());
        let spec = FormFieldShellSpec::try_new(
            "field:body",
            Some("Body".into()),
            false,
            false,
            None,
            FormFieldValidation::Invalid {
                message: "Required".into(),
            },
            false,
            38.0,
            None,
        )
        .unwrap();
        assert_eq!(spec.validation.status_kind(), "invalid");
        assert_eq!(
            spec.supporting_message().map(AsRef::as_ref),
            Some("Required")
        );
    }

    #[test]
    fn impossible_height_and_disabled_validation_combinations_fail_closed() {
        assert!(FormFieldShellSpec::try_new(
            "field:body",
            None,
            false,
            false,
            None,
            FormFieldValidation::Neutral,
            true,
            80.0,
            Some(40.0),
        )
        .is_err());
        assert!(FormFieldShellSpec::try_new(
            "field:body",
            None,
            false,
            true,
            Some("Unavailable".into()),
            FormFieldValidation::Valid,
            false,
            38.0,
            None,
        )
        .is_err());
    }

    #[test]
    fn stable_ids_derive_all_shell_anatomy_without_display_copy() {
        let mut spec = neutral();
        let surface = spec.surface_id();
        let label = spec.label_id();
        spec.label = Some("Renamed body".into());
        assert_eq!(surface.as_ref(), "field:body:surface");
        assert_eq!(label.as_ref(), "field:body:label");
        assert_eq!(spec.surface_id(), surface);
        assert_eq!(spec.label_id(), label);
    }
}
