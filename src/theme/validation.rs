//! Theme validation and diagnostics
//!
//! Provides validation for theme configuration with:
//! - JSON-pointer style error paths (e.g., "/vibrancy/opacity/main")
//! - Severity levels (error, warning, info)
//! - Range validation for opacity, color values
//! - Unknown key detection
//! - Last-known-good fallback support

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

// ============================================================================
// Diagnostic types
// ============================================================================

/// Severity level for theme diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// Error: Theme won't work correctly (e.g., invalid color format)
    Error,
    /// Warning: Theme might not look right (e.g., unknown key, out-of-range opacity)
    Warning,
    /// Info: Informational note (e.g., default value applied)
    Info,
}

/// A single diagnostic message
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// JSON-pointer style path to the problematic field
    pub path: String,
    /// Severity of the issue
    pub severity: DiagnosticSeverity,
    /// Human-readable message describing the issue
    pub message: String,
    /// Optional suggestion for fixing the issue
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Collection of diagnostics from theme validation
#[derive(Debug, Clone, Default)]
pub struct ThemeDiagnostics {
    pub diagnostics: Vec<Diagnostic>,
}

impl ThemeDiagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.add(Diagnostic::error(path, message));
    }

    pub fn warning(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.add(Diagnostic::warning(path, message));
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Warning)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count()
    }

    pub fn format_for_log(&self) -> String {
        if self.diagnostics.is_empty() {
            return "Theme validation passed".to_string();
        }
        let mut output = format!(
            "Theme validation: {} error(s), {} warning(s)\n",
            self.error_count(),
            self.warning_count()
        );
        for diag in &self.diagnostics {
            let severity = match diag.severity {
                DiagnosticSeverity::Error => "ERROR",
                DiagnosticSeverity::Warning => "WARN",
                DiagnosticSeverity::Info => "INFO",
            };
            output.push_str(&format!(
                "  [{}] {}: {}\n",
                severity, diag.path, diag.message
            ));
            if let Some(ref suggestion) = diag.suggestion {
                output.push_str(&format!("    Suggestion: {}\n", suggestion));
            }
        }
        output
    }
}

// Known keys for validation
const KNOWN_TOP_LEVEL_KEYS: &[&str] = &[
    "appearance",
    "colors",
    "focus_aware",
    "opacity",
    "drop_shadow",
    "vibrancy",
    "fonts",
    "background_gradient",
];
const KNOWN_COLOR_KEYS: &[&str] = &["background", "text", "accent", "ui", "terminal"];
const KNOWN_FOCUS_AWARE_KEYS: &[&str] = &["focused", "unfocused"];
const KNOWN_FOCUS_COLOR_SCHEME_KEYS: &[&str] =
    &["background", "text", "accent", "ui", "cursor", "terminal"];
const KNOWN_CURSOR_KEYS: &[&str] = &["color", "blink_interval_ms"];
const KNOWN_BACKGROUND_KEYS: &[&str] = &["main", "title_bar", "search_box", "log_panel"];
const KNOWN_TEXT_KEYS: &[&str] = &[
    "primary",
    "secondary",
    "tertiary",
    "muted",
    "dimmed",
    "on_accent",
];
const KNOWN_ACCENT_KEYS: &[&str] = &["selected", "selected_subtle"];
const KNOWN_UI_KEYS: &[&str] = &["border", "success", "error", "warning", "info"];
const KNOWN_TERMINAL_KEYS: &[&str] = &[
    "foreground",
    "background",
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];
const KNOWN_OPACITY_KEYS: &[&str] = &[
    "main",
    "title_bar",
    "search_box",
    "log_panel",
    "selected",
    "hover",
    "preview",
    "dialog",
    "input",
    "panel",
    "input_inactive",
    "input_active",
    "border_inactive",
    "border_active",
    "vibrancy_background",
    "text_name",
    "text_strong",
    "text_muted_alpha",
    "text_hint",
    "text_placeholder",
    "text_icon",
    "glass_veil_opacity",
    "glass_tint_opacity",
    "glass_morph_duration",
    "glass_morph_inset",
];
const KNOWN_VIBRANCY_KEYS: &[&str] = &["enabled", "material", "backdrop_saturation"];
const KNOWN_DROP_SHADOW_KEYS: &[&str] = &[
    "enabled",
    "blur_radius",
    "spread_radius",
    "offset_x",
    "offset_y",
    "color",
    "opacity",
];
const KNOWN_FONT_KEYS: &[&str] = &["mono_family", "mono_size", "ui_family", "ui_size"];
const KNOWN_BACKGROUND_GRADIENT_KEYS: &[&str] =
    &["enabled", "from", "to", "angle", "opacity", "layers"];
const VALID_MATERIALS: &[&str] = &["hud", "popover", "menu", "sidebar", "content"];

/// Validate a theme JSON value and return diagnostics
pub fn validate_theme_json(json: &Value) -> ThemeDiagnostics {
    let mut diags = ThemeDiagnostics::new();
    if let Value::Object(map) = json {
        check_unknown_keys(&mut diags, "", map.keys(), KNOWN_TOP_LEVEL_KEYS);
        if let Some(colors) = map.get("colors") {
            validate_colors(&mut diags, "/colors", colors);
        }
        if let Some(focus_aware) = map.get("focus_aware") {
            if !focus_aware.is_null() {
                validate_focus_aware(&mut diags, "/focus_aware", focus_aware);
            }
        }
        if let Some(opacity) = map.get("opacity") {
            if !opacity.is_null() {
                validate_opacity(&mut diags, "/opacity", opacity);
            }
        }
        if let Some(vibrancy) = map.get("vibrancy") {
            if !vibrancy.is_null() {
                validate_vibrancy(&mut diags, "/vibrancy", vibrancy);
            }
        }
        if let Some(shadow) = map.get("drop_shadow") {
            if !shadow.is_null() {
                validate_drop_shadow(&mut diags, "/drop_shadow", shadow);
            }
        }
        if let Some(fonts) = map.get("fonts") {
            if !fonts.is_null() {
                validate_fonts(&mut diags, "/fonts", fonts);
            }
        }
        if let Some(gradient) = map.get("background_gradient") {
            if !gradient.is_null() {
                validate_background_gradient(&mut diags, "/background_gradient", gradient);
            }
        }
    } else {
        diags.error("", "Theme must be a JSON object");
    }
    diags
}

fn check_unknown_keys<'a>(
    diags: &mut ThemeDiagnostics,
    parent_path: &str,
    keys: impl Iterator<Item = &'a String>,
    known_keys: &[&str],
) {
    let known_set: HashSet<&str> = known_keys.iter().copied().collect();
    for key in keys {
        if !known_set.contains(key.as_str()) {
            let path = if parent_path.is_empty() {
                format!("/{}", key)
            } else {
                format!("{}/{}", parent_path, key)
            };
            diags.add(
                Diagnostic::warning(&path, format!("Unknown key '{}' will be ignored", key))
                    .with_suggestion("Remove this key or rename it to a supported theme field"),
            );
        }
    }
}

fn validate_colors(diags: &mut ThemeDiagnostics, path: &str, colors: &Value) {
    if let Value::Object(map) = colors {
        check_unknown_keys(diags, path, map.keys(), KNOWN_COLOR_KEYS);
        if let Some(bg) = map.get("background") {
            validate_color_object(
                diags,
                &format!("{}/background", path),
                bg,
                KNOWN_BACKGROUND_KEYS,
                &[],
            );
        }
        if let Some(text) = map.get("text") {
            validate_color_object(diags, &format!("{}/text", path), text, KNOWN_TEXT_KEYS, &[]);
        }
        if let Some(accent) = map.get("accent") {
            validate_color_object(
                diags,
                &format!("{}/accent", path),
                accent,
                KNOWN_ACCENT_KEYS,
                &[],
            );
        }
        if let Some(ui) = map.get("ui") {
            validate_color_object(diags, &format!("{}/ui", path), ui, KNOWN_UI_KEYS, &[]);
        }
        if let Some(terminal) = map.get("terminal") {
            validate_color_object(
                diags,
                &format!("{}/terminal", path),
                terminal,
                KNOWN_TERMINAL_KEYS,
                &["foreground", "background"],
            );
        }
    } else {
        diags.error(path, "colors must be an object");
    }
}

fn validate_focus_aware(diags: &mut ThemeDiagnostics, path: &str, focus_aware: &Value) {
    if let Value::Object(map) = focus_aware {
        check_unknown_keys(diags, path, map.keys(), KNOWN_FOCUS_AWARE_KEYS);

        if let Some(focused) = map.get("focused").filter(|value| !value.is_null()) {
            validate_focus_color_scheme(diags, &format!("{}/focused", path), focused);
        }

        if let Some(unfocused) = map.get("unfocused").filter(|value| !value.is_null()) {
            validate_focus_color_scheme(diags, &format!("{}/unfocused", path), unfocused);
        }
    } else {
        diags.error(path, "focus_aware must be an object");
    }
}

fn validate_focus_color_scheme(diags: &mut ThemeDiagnostics, path: &str, scheme: &Value) {
    if let Value::Object(map) = scheme {
        check_unknown_keys(diags, path, map.keys(), KNOWN_FOCUS_COLOR_SCHEME_KEYS);

        if let Some(bg) = map.get("background") {
            validate_color_object(
                diags,
                &format!("{}/background", path),
                bg,
                KNOWN_BACKGROUND_KEYS,
                &[],
            );
        }

        if let Some(text) = map.get("text") {
            validate_color_object(diags, &format!("{}/text", path), text, KNOWN_TEXT_KEYS, &[]);
        }

        if let Some(accent) = map.get("accent") {
            validate_color_object(
                diags,
                &format!("{}/accent", path),
                accent,
                KNOWN_ACCENT_KEYS,
                &[],
            );
        }

        if let Some(ui) = map.get("ui") {
            validate_color_object(diags, &format!("{}/ui", path), ui, KNOWN_UI_KEYS, &[]);
        }

        if let Some(cursor) = map.get("cursor").filter(|value| !value.is_null()) {
            validate_cursor(diags, &format!("{}/cursor", path), cursor);
        }

        if let Some(terminal) = map.get("terminal") {
            validate_color_object(
                diags,
                &format!("{}/terminal", path),
                terminal,
                KNOWN_TERMINAL_KEYS,
                &["foreground", "background"],
            );
        }
    } else {
        diags.error(path, "Expected an object");
    }
}

fn validate_cursor(diags: &mut ThemeDiagnostics, path: &str, cursor: &Value) {
    if let Value::Object(map) = cursor {
        check_unknown_keys(diags, path, map.keys(), KNOWN_CURSOR_KEYS);

        if let Some(color) = map.get("color") {
            validate_color_value(diags, &format!("{}/color", path), color);
        }

        if let Some(blink_interval_ms) = map.get("blink_interval_ms") {
            if !blink_interval_ms.is_u64() {
                diags.error(
                    format!("{}/blink_interval_ms", path),
                    "blink_interval_ms must be a non-negative integer",
                );
            }
        }
    } else {
        diags.error(path, "cursor must be an object");
    }
}

fn validate_color_object(
    diags: &mut ThemeDiagnostics,
    path: &str,
    obj: &Value,
    known_keys: &[&str],
    nullable_keys: &[&str],
) {
    if let Value::Object(map) = obj {
        check_unknown_keys(diags, path, map.keys(), known_keys);
        for (key, value) in map {
            if value.is_null() && nullable_keys.contains(&key.as_str()) {
                continue;
            }
            validate_color_value(diags, &format!("{}/{}", path, key), value);
        }
    } else {
        diags.error(path, "Expected an object");
    }
}

fn validate_color_value(diags: &mut ThemeDiagnostics, path: &str, value: &Value) {
    match value {
        Value::Number(n) => {
            if let Some(v) = n.as_u64() {
                if v > 0xFFFF_FFFF {
                    diags.error(
                        path,
                        "Color value exceeds 0xFFFFFFFF — expected RGB (0xRRGGBB) or RGBA (0xRRGGBBAA)",
                    );
                } else if v > 0x00FF_FFFF {
                    diags.warning(
                        path,
                        "Numeric color includes alpha channel (0xRRGGBBAA) — alpha will be stripped",
                    );
                }
            } else if let Some(v) = n.as_i64() {
                if v < 0 {
                    diags.error(path, "Color value cannot be negative");
                }
            } else {
                diags.error(
                    path,
                    "Color value must be an integer — channel values must be 0-255",
                );
            }
        }
        Value::String(s) => {
            if !is_valid_color_string(s) {
                diags.error(path, format!("Invalid color format: '{}'", s));
            }
        }
        _ => diags.error(path, "Color must be a number or string"),
    }
}

fn is_valid_color_string(s: &str) -> bool {
    crate::theme::hex_color::hex_color_serde::parse_color_string(s).is_ok()
}

fn validate_opacity(diags: &mut ThemeDiagnostics, path: &str, opacity: &Value) {
    if let Value::Object(map) = opacity {
        check_unknown_keys(diags, path, map.keys(), KNOWN_OPACITY_KEYS);
        for (key, value) in map {
            if value.is_null()
                && matches!(
                    key.as_str(),
                    "vibrancy_background"
                        | "glass_veil_opacity"
                        | "glass_tint_opacity"
                        | "glass_morph_duration"
                        | "glass_morph_inset"
                )
            {
                continue;
            }
            let field = format!("{path}/{key}");
            let maximum = match key.as_str() {
                "glass_morph_duration" => 2.0,
                "glass_morph_inset" => 0.4,
                _ => {
                    validate_opacity_value(diags, &field, value);
                    continue;
                }
            };
            match value.as_f64() {
                Some(number) if number.is_finite() && (0.0..=maximum).contains(&number) => {}
                Some(_) => diags.warning(field, format!("Value will be clamped to 0..={maximum}")),
                None => diags.error(field, "Value must be a number"),
            }
        }
        if let (Some(selected), Some(hover)) = (map.get("selected"), map.get("hover")) {
            validate_hover_less_than_selected(diags, path, selected, hover);
        }
    } else {
        diags.error(path, "opacity must be an object");
    }
}

fn validate_opacity_value(diags: &mut ThemeDiagnostics, path: &str, value: &Value) {
    if let Value::Number(n) = value {
        if let Some(f) = n.as_f64() {
            if f < 0.0 {
                diags.warning(path, "Opacity cannot be negative, will be clamped to 0.0");
            } else if f > 1.0 {
                diags.warning(path, "Opacity exceeds 1.0, will be clamped to 1.0");
            }
        }
    } else {
        diags.error(path, "Opacity must be a number");
    }
}

fn validate_hover_less_than_selected(
    diags: &mut ThemeDiagnostics,
    path: &str,
    selected: &Value,
    hover: &Value,
) {
    let Some(selected) = selected.as_f64() else {
        return;
    };
    let Some(hover) = hover.as_f64() else {
        return;
    };

    if hover >= selected {
        diags.add(
            Diagnostic::warning(
                format!("{}/hover", path),
                "Hover opacity should stay below selected opacity so focused rows remain stronger",
            )
            .with_suggestion("Lower /opacity/hover or raise /opacity/selected to keep focus more visible than hover"),
        );
    }
}

fn validate_vibrancy(diags: &mut ThemeDiagnostics, path: &str, vibrancy: &Value) {
    if let Value::Object(map) = vibrancy {
        check_unknown_keys(diags, path, map.keys(), KNOWN_VIBRANCY_KEYS);
        if let Some(material) = map.get("material") {
            let material_path = format!("{}/material", path);
            if let Value::String(m) = material {
                if !VALID_MATERIALS.contains(&m.as_str()) {
                    diags.error(material_path.clone(), format!("Invalid material '{}'", m));
                }
            } else {
                diags.error(material_path, "material must be a string");
            }
        }
        if let Some(enabled) = map.get("enabled") {
            if !enabled.is_boolean() {
                diags.error(format!("{}/enabled", path), "enabled must be a boolean");
            }
        }
    } else {
        diags.error(path, "vibrancy must be an object");
    }
}

fn validate_drop_shadow(diags: &mut ThemeDiagnostics, path: &str, shadow: &Value) {
    if let Value::Object(map) = shadow {
        check_unknown_keys(diags, path, map.keys(), KNOWN_DROP_SHADOW_KEYS);
        for key in ["blur_radius", "spread_radius", "offset_x", "offset_y"] {
            if let Some(value) = map.get(key) {
                if !value.is_number() {
                    diags.error(
                        format!("{}/{}", path, key),
                        format!("{} must be a number", key),
                    );
                }
            }
        }
        if let Some(opacity) = map.get("opacity") {
            validate_opacity_value(diags, &format!("{}/opacity", path), opacity);
        }
        if let Some(color) = map.get("color") {
            validate_color_value(diags, &format!("{}/color", path), color);
        }
        if let Some(enabled) = map.get("enabled") {
            if !enabled.is_boolean() {
                diags.error(format!("{}/enabled", path), "enabled must be a boolean");
            }
        }
    } else {
        diags.error(path, "drop_shadow must be an object");
    }
}

fn validate_fonts(diags: &mut ThemeDiagnostics, path: &str, fonts: &Value) {
    if let Value::Object(map) = fonts {
        check_unknown_keys(diags, path, map.keys(), KNOWN_FONT_KEYS);
        for key in ["mono_family", "ui_family"] {
            if let Some(value) = map.get(key) {
                if !value.is_string() {
                    diags.error(
                        format!("{}/{}", path, key),
                        format!("{} must be a string", key),
                    );
                }
            }
        }
        for key in ["mono_size", "ui_size"] {
            if let Some(value) = map.get(key) {
                let field_path = format!("{}/{}", path, key);
                if let Value::Number(n) = value {
                    if let Some(f) = n.as_f64() {
                        if f <= 0.0 {
                            diags.warning(field_path, "Font size must be positive");
                        } else if f > 100.0 {
                            diags.warning(field_path, "Font size unusually large");
                        }
                    }
                } else {
                    diags.error(field_path, format!("{} must be a number", key));
                }
            }
        }
    } else {
        diags.error(path, "fonts must be an object");
    }
}

fn validate_background_gradient(diags: &mut ThemeDiagnostics, path: &str, gradient: &Value) {
    if let Value::Object(map) = gradient {
        check_unknown_keys(diags, path, map.keys(), KNOWN_BACKGROUND_GRADIENT_KEYS);
        if let Some(enabled) = map.get("enabled") {
            if !enabled.is_boolean() {
                diags.error(format!("{}/enabled", path), "enabled must be a boolean");
            }
        }
        for key in ["from", "to"] {
            if let Some(value) = map.get(key) {
                validate_color_value(diags, &format!("{}/{}", path, key), value);
            }
        }
        if let Some(angle) = map.get("angle") {
            if !angle.is_number() {
                diags.error(format!("{}/angle", path), "angle must be a number");
            }
        }
        if let Some(opacity) = map.get("opacity") {
            validate_opacity_value(diags, &format!("{}/opacity", path), opacity);
        }
        if let Some(layers) = map.get("layers") {
            match layers.as_array() {
                Some(layers) if layers.len() <= 8 => {
                    for (index, layer) in layers.iter().enumerate() {
                        let path = format!("{path}/layers/{index}");
                        if layer.get("layers").is_some() {
                            diags.error(path, "Gradient layers cannot contain nested layers");
                        } else {
                            validate_background_gradient(diags, &path, layer);
                        }
                    }
                }
                Some(_) => diags.error(
                    format!("{path}/layers"),
                    "At most eight layers are supported",
                ),
                None => diags.error(format!("{path}/layers"), "layers must be an array"),
            }
        }
    } else {
        diags.error(path, "background_gradient must be an object");
    }
}

#[cfg(test)]
mod tests {
    use super::validate_theme_json;
    use serde_json::json;

    #[test]
    fn test_validate_theme_json_skips_errors_when_terminal_nullable_keys_are_null() {
        let theme = json!({
            "colors": {
                "terminal": {
                    "foreground": null,
                    "background": null,
                    "red": "#ff0000"
                }
            }
        });

        let diagnostics = validate_theme_json(&theme);

        assert_eq!(
            diagnostics.error_count(),
            0,
            "nullable terminal keys should accept null values",
        );
    }

    #[test]
    fn test_validate_theme_json_skips_focus_aware_validation_when_section_is_null() {
        let theme = json!({
            "focus_aware": null
        });

        let diagnostics = validate_theme_json(&theme);

        assert_eq!(
            diagnostics.error_count(),
            0,
            "null optional focus_aware section should be treated as unset",
        );
    }

    #[test]
    fn test_validate_theme_json_reports_error_when_required_color_is_null() {
        let theme = json!({
            "colors": {
                "background": {
                    "main": null
                }
            }
        });

        let diagnostics = validate_theme_json(&theme);

        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diag| diag.path == "/colors/background/main"
                    && diag.message == "Color must be a number or string"),
            "required color fields should still reject null values",
        );
    }

    #[test]
    fn test_validate_theme_json_accepts_text_opacity_keys() {
        let theme = json!({
            "opacity": {
                "selected": 0.20,
                "hover": 0.06,
                "text_name": 1.0,
                "text_strong": 0.8,
                "text_muted_alpha": 0.65,
                "text_hint": 0.45,
                "text_placeholder": 0.40,
                "text_icon": 0.5
            }
        });

        let diagnostics = validate_theme_json(&theme);

        assert!(
            !diagnostics
                .diagnostics
                .iter()
                .any(|diag| diag.path.starts_with("/opacity/text_")
                    && diag.message.contains("Unknown key")),
            "text opacity keys should be recognized by theme validation",
        );
    }

    #[test]
    fn test_validate_theme_json_warns_when_hover_matches_or_exceeds_selected() {
        let theme = json!({
            "opacity": {
                "selected": 0.18,
                "hover": 0.18
            }
        });

        let diagnostics = validate_theme_json(&theme);

        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diag| diag.path == "/opacity/hover"
                    && diag
                        .message
                        .contains("Hover opacity should stay below selected opacity")),
            "theme validation should flag hover values that collapse into focused styling",
        );
    }
}
