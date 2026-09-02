use super::*;

/// DivPrompt - HTML content display
///
/// Features:
/// - Parse and render HTML elements as native GPUI components
/// - Support for headers, paragraphs, bold, italic, code, lists, blockquotes
/// - Theme-aware styling
/// - Simple keyboard: Enter or Escape to submit
pub struct DivPrompt {
    pub id: String,
    pub html: String,
    pub tailwind: Option<String>,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling (defaults to Default for theme-based styling)
    pub design_variant: DesignVariant,
    /// Container customization options
    pub container_options: ContainerOptions,
    /// Scroll handle for tracking scroll position
    pub scroll_handle: ScrollHandle,
    /// Pre-extracted prompt colors for efficient rendering (Copy, 28 bytes)
    /// Avoids re-extracting colors from theme on every render
    pub(super) prompt_colors: theme::PromptColors,
}

impl DivPrompt {
    pub(crate) fn update_theme(&mut self, theme: Arc<theme::Theme>, cx: &mut Context<Self>) {
        self.prompt_colors = theme.colors.prompt_colors();
        self.theme = theme;
        cx.notify();
    }

    pub fn new(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_options(
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
            theme,
            DesignVariant::Default,
            ContainerOptions::default(),
        )
    }

    pub fn with_design(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
    ) -> Self {
        Self::with_options(
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
            theme,
            design_variant,
            ContainerOptions::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
        container_options: ContainerOptions,
    ) -> Self {
        // Extract colors ONCE during construction to avoid re-extraction on every render
        // PromptColors is Copy (28 bytes) - much cheaper than extracting on every frame
        let prompt_colors = theme.colors.prompt_colors();

        logging::log(
            "PROMPTS",
            &format!(
                "DivPrompt::new with theme colors: bg={:#x}, text={:#x}, design: {:?}, container_opts: {:?}",
                theme.colors.background.main,
                theme.colors.text.primary,
                design_variant,
                container_options
            ),
        );
        DivPrompt {
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
            theme,
            design_variant,
            container_options,
            scroll_handle: ScrollHandle::new(),
            prompt_colors,
        }
    }

    /// Submit - always with None value (just acknowledgment). Public so the
    /// simulateKey dispatcher can drive Enter/Escape on div prompts the same
    /// way real key dispatch does.
    pub fn submit(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Submit with a specific value (for submit:value links)
    fn submit_with_value(&mut self, value: String) {
        logging::log("DIV", &format!("Submit with value: {}", value));
        (self.on_submit)(self.id.clone(), Some(value));
    }

    /// Handle link click based on href pattern
    pub(super) fn handle_link_click(&mut self, href: &str) {
        logging::log("DIV", &format!("Link clicked: {}", href));

        if let Some(value) = href.strip_prefix("submit:") {
            self.submit_with_value(value.to_string());
        } else if href.starts_with("http://") || href.starts_with("https://") {
            if let Err(error) =
                crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)
            {
                logging::log("DIV", &error.to_string());
                return;
            }
            if let Err(e) = open::that(href) {
                logging::log("DIV", &format!("Failed to open URL {}: {}", href, e));
            }
        } else if href.starts_with("file://") {
            if let Err(error) =
                crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)
            {
                logging::log("DIV", &error.to_string());
                return;
            }
            if let Err(e) = open::that(href) {
                logging::log("DIV", &format!("Failed to open file {}: {}", href, e));
            }
        } else {
            logging::log("DIV", &format!("Unknown link protocol: {}", href));
        }
    }
}
