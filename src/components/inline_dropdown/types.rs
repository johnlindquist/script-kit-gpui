use gpui::{AnyElement, Hsla, SharedString};

#[derive(Clone, Copy, Debug)]
pub(crate) struct InlineDropdownColors {
    pub(crate) surface_rgba: u32,
    pub(crate) border_rgba: u32,
    pub(crate) divider_rgba: u32,
    pub(crate) foreground: Hsla,
    pub(crate) muted_foreground: Hsla,
    pub(crate) accent: Hsla,
}

impl InlineDropdownColors {
    #[cfg(test)]
    pub(crate) fn from_theme(theme: &crate::theme::Theme) -> Self {
        let chrome = crate::theme::AppChromeColors::from_theme(theme);
        Self {
            surface_rgba: chrome.inline_dropdown_surface_rgba,
            border_rgba: chrome.border_rgba,
            divider_rgba: chrome.divider_rgba,
            foreground: gpui::rgb(theme.colors.text.primary).into(),
            muted_foreground: gpui::rgb(theme.colors.text.muted).into(),
            accent: gpui::rgb(theme.colors.accent.selected).into(),
        }
    }

    pub(crate) fn popup_from_theme(theme: &crate::theme::Theme) -> Self {
        let chrome = crate::theme::AppChromeColors::from_theme(theme);
        Self {
            surface_rgba: chrome.popup_surface_rgba,
            border_rgba: chrome.border_rgba,
            divider_rgba: chrome.divider_rgba,
            foreground: gpui::rgb(theme.colors.text.primary).into(),
            muted_foreground: gpui::rgb(theme.colors.text.muted).into(),
            accent: gpui::rgb(theme.colors.accent.selected).into(),
        }
    }
}

pub(crate) struct InlineDropdownEmptyState {
    pub(crate) message: SharedString,
    pub(crate) hints: Vec<AnyElement>,
}

pub(crate) struct InlineDropdownSynopsis {
    pub(crate) label: SharedString,
    pub(crate) meta: SharedString,
    pub(crate) description: SharedString,
}

#[cfg(test)]
mod tests {
    use super::InlineDropdownColors;

    #[test]
    fn default_inline_dropdown_colors_use_inline_surface_token() {
        let theme = crate::theme::Theme::dark_default();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let colors = InlineDropdownColors::from_theme(&theme);

        assert_eq!(colors.surface_rgba, chrome.inline_dropdown_surface_rgba);
    }

    #[test]
    fn popup_inline_dropdown_colors_use_popup_surface_token() {
        let theme = crate::theme::Theme::dark_default();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let colors = InlineDropdownColors::popup_from_theme(&theme);

        assert_eq!(colors.surface_rgba, chrome.popup_surface_rgba);
    }
}
