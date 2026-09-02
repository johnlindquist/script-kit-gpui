//! Validated runtime edits. The protocol owns the closed wire vocabulary;
//! this module owns preparation, resolved values and optimistic preview state.
use std::sync::Arc;

use super::{gpui_integration::PreparedComponentTheme, Theme};
use crate::protocol::{LiveThemeEdit, MAX_LIVE_THEME_EDITS};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThemeEditError {
    #[error("invalid_edit_count")]
    InvalidEditCount,
    #[error("duplicate_token: {0}")]
    DuplicateToken(&'static str),
    #[error("invalid_rgb24: {0}")]
    InvalidRgb24(&'static str),
    #[error("invalid_opacity: {0}")]
    InvalidOpacity(&'static str),
    #[error("invalid_theme_state_ladder")]
    InvalidStateLadder,
    #[error("invalid_text_tier_order")]
    InvalidTextLadder,
    #[error("invalid_effective_launcher_state_ladder")]
    InvalidEffectiveStateLadder,
    #[error("invalid_theme_document: {0}")]
    InvalidDocument(String),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLiveTheme {
    /// The same eleven tokenId/value records accepted by the wire decoder.
    pub values: Vec<LiveThemeEdit>,
    pub text_primary_rgb24: u32,
    pub chrome_selection_rgba8: u32,
    pub chrome_hover_rgba8: u32,
    pub main_menu_hover_rgba8: Option<u32>,
    pub main_menu_selected_rgba8: Option<u32>,
    pub native_material_captured: bool,
}

impl ResolvedLiveTheme {
    pub(super) fn from_theme(theme: &Theme) -> Self {
        let opacity = theme.get_opacity();
        let chrome = super::AppChromeColors::from_theme(theme);
        let row = super::resolve_main_menu_row_state_palette(
            theme,
            crate::designs::current_main_menu_theme(),
        );
        Self {
            values: vec![
                LiveThemeEdit::Accent(theme.colors.accent.selected),
                LiveThemeEdit::MainBackground(theme.colors.background.main),
                LiveThemeEdit::SearchBackground(theme.colors.background.search_box),
                LiveThemeEdit::ErrorColor(theme.colors.ui.error),
                LiveThemeEdit::Hover(opacity.hover),
                LiveThemeEdit::Selected(opacity.selected),
                LiveThemeEdit::TextStrong(opacity.text_strong),
                LiveThemeEdit::TextMuted(opacity.text_muted_alpha),
                LiveThemeEdit::TextHint(opacity.text_hint),
                LiveThemeEdit::TextPlaceholder(opacity.text_placeholder),
                LiveThemeEdit::TextIcon(opacity.text_icon),
            ],
            text_primary_rgb24: theme.colors.text.primary,
            chrome_selection_rgba8: chrome.selection_rgba,
            chrome_hover_rgba8: chrome.hover_rgba,
            main_menu_hover_rgba8: row.hover.background_rgba,
            main_menu_selected_rgba8: row.active.background_rgba,
            native_material_captured: false,
        }
    }
}

/// Validated theme publication input; construction preserves normalization and validation.
pub struct PreparedTheme {
    pub(super) theme: Arc<Theme>,
    pub(super) component_theme: PreparedComponentTheme,
    pub(super) resolved: Arc<ResolvedLiveTheme>,
}

/// A single immutable cache publication. Obtain this once when both theme and
/// revision are needed; separate accessor calls can observe different commits.
#[derive(Debug, Clone)]
pub struct PublishedTheme {
    pub revision: u64,
    pub theme: Arc<Theme>,
    pub resolved: Arc<ResolvedLiveTheme>,
}

/// Complete themes (presets, legacy files) retain their existing warning/clamp
/// semantics. Strict live edits additionally enforce both opacity ladders.
pub fn prepare_theme(theme: Theme) -> Result<PreparedTheme, ThemeEditError> {
    let theme = super::types::normalize_theme_primary_text(theme);
    let document = serde_json::to_value(&theme)
        .map_err(|error| ThemeEditError::InvalidDocument(error.to_string()))?;
    let diagnostics = super::validation::validate_theme_json(&document);
    if diagnostics.has_errors() {
        return Err(ThemeEditError::InvalidDocument(
            diagnostics.format_for_log(),
        ));
    }
    let component_theme = PreparedComponentTheme::new(&theme);
    let resolved = Arc::new(ResolvedLiveTheme::from_theme(&theme));
    Ok(PreparedTheme {
        theme: Arc::new(theme),
        component_theme,
        resolved,
    })
}

pub(crate) fn prepare_live_theme(
    base: &Theme,
    edits: &[LiveThemeEdit],
) -> Result<PreparedTheme, ThemeEditError> {
    if edits.is_empty() || edits.len() > MAX_LIVE_THEME_EDITS {
        return Err(ThemeEditError::InvalidEditCount);
    }
    let mut theme = base.clone();
    let mut opacity = theme.get_opacity();
    for (index, edit) in edits.iter().enumerate() {
        if edits[..index]
            .iter()
            .any(|previous| previous.id() == edit.id())
        {
            return Err(ThemeEditError::DuplicateToken(edit.id()));
        }
        match *edit {
            LiveThemeEdit::Accent(value)
            | LiveThemeEdit::MainBackground(value)
            | LiveThemeEdit::SearchBackground(value)
            | LiveThemeEdit::ErrorColor(value) => {
                if value > 0x00ff_ffff {
                    return Err(ThemeEditError::InvalidRgb24(edit.id()));
                }
            }
            LiveThemeEdit::Hover(value)
            | LiveThemeEdit::Selected(value)
            | LiveThemeEdit::TextStrong(value)
            | LiveThemeEdit::TextMuted(value)
            | LiveThemeEdit::TextHint(value)
            | LiveThemeEdit::TextPlaceholder(value)
            | LiveThemeEdit::TextIcon(value) => {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(ThemeEditError::InvalidOpacity(edit.id()));
                }
            }
        }
        match *edit {
            LiveThemeEdit::Accent(value) => theme.colors.accent.selected = value,
            LiveThemeEdit::MainBackground(value) => theme.colors.background.main = value,
            LiveThemeEdit::SearchBackground(value) => theme.colors.background.search_box = value,
            LiveThemeEdit::ErrorColor(value) => theme.colors.ui.error = value,
            LiveThemeEdit::Hover(value) => opacity.hover = value,
            LiveThemeEdit::Selected(value) => opacity.selected = value,
            LiveThemeEdit::TextStrong(value) => opacity.text_strong = value,
            LiveThemeEdit::TextMuted(value) => opacity.text_muted_alpha = value,
            LiveThemeEdit::TextHint(value) => opacity.text_hint = value,
            LiveThemeEdit::TextPlaceholder(value) => opacity.text_placeholder = value,
            LiveThemeEdit::TextIcon(value) => opacity.text_icon = value,
        }
    }
    if opacity.hover >= opacity.selected {
        return Err(ThemeEditError::InvalidStateLadder);
    }
    if !(opacity.text_placeholder <= opacity.text_hint
        && opacity.text_hint <= opacity.text_muted_alpha
        && opacity.text_muted_alpha <= opacity.text_strong
        && opacity.text_strong <= opacity.text_name)
    {
        return Err(ThemeEditError::InvalidTextLadder);
    }
    theme.opacity = Some(opacity);
    let prepared = prepare_theme(theme)?;
    if !matches!((prepared.resolved.main_menu_hover_rgba8, prepared.resolved.main_menu_selected_rgba8),
        (Some(hover), Some(selected)) if (hover & 0xff) < (selected & 0xff))
    {
        return Err(ThemeEditError::InvalidEffectiveStateLadder);
    }
    Ok(prepared)
}

#[derive(Default)]
pub struct LiveThemePreview {
    baseline: Option<Arc<Theme>>,
    last_revision: Option<u64>,
}

impl LiveThemePreview {
    pub fn apply(
        &mut self,
        cx: &mut gpui::App,
        expected_revision: u64,
        edits: &[LiveThemeEdit],
    ) -> Result<super::service::ThemePublicationReceipt, super::service::ThemePublishError> {
        let snapshot = super::get_theme_snapshot();
        let prepared = prepare_live_theme(&snapshot.theme, edits)?;
        let receipt = super::service::publish_runtime_theme(
            cx,
            expected_revision,
            prepared,
            super::service::ThemePublicationSource::LivePreview,
        )?;
        self.baseline.get_or_insert_with(|| snapshot.theme.clone());
        self.last_revision = Some(receipt.revision);
        Ok(receipt)
    }

    pub fn revert(
        &mut self,
        cx: &mut gpui::App,
    ) -> Result<super::service::ThemePublicationReceipt, super::service::ThemePublishError> {
        let baseline = self
            .baseline
            .as_ref()
            .ok_or(super::service::ThemePublishError::NoPreview)?;
        let expected = self
            .last_revision
            .ok_or(super::service::ThemePublishError::NoPreview)?;
        let receipt = super::service::publish_runtime_theme(
            cx,
            expected,
            prepare_theme((**baseline).clone())?,
            super::service::ThemePublicationSource::Revert,
        )?;
        self.baseline = None;
        self.last_revision = None;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_edits_leave_the_base_unchanged() {
        let base = Theme::dark_default();
        let before = serde_json::to_value(&base).unwrap();
        for edits in [
            vec![],
            vec![LiveThemeEdit::Accent(0xff00ff00)],
            vec![LiveThemeEdit::Hover(f32::NAN)],
            vec![LiveThemeEdit::TextIcon(1.1)],
            vec![LiveThemeEdit::Accent(1), LiveThemeEdit::Accent(2)],
            vec![LiveThemeEdit::Hover(1.0)],
            vec![LiveThemeEdit::TextPlaceholder(1.0)],
        ] {
            assert!(prepare_live_theme(&base, &edits).is_err());
            assert_eq!(serde_json::to_value(&base).unwrap(), before);
        }
    }

    #[test]
    fn edits_normalize_text_and_preserve_locked_values() {
        let base = Theme::dark_default();
        let prepared =
            prepare_live_theme(&base, &[LiveThemeEdit::MainBackground(0xffffff)]).unwrap();
        assert_eq!(prepared.theme.colors.text.primary, 0x000000);
        assert_eq!(
            prepared.theme.get_opacity().glass_morph_duration,
            base.get_opacity().glass_morph_duration
        );
        assert_eq!(
            prepared.theme.get_opacity().glass_morph_inset,
            base.get_opacity().glass_morph_inset
        );
        assert_eq!(prepared.resolved.values.len(), 11);
    }
}
