//! Host-facing configuration for the shared markdown notes editor.

pub const NOTES_EDITOR_STYLE_OWNER: &str = "components.notes_editor";
pub const NOTES_EDITOR_INPUT_RENDER_PATH: &str = "components.notes_editor.render_input";
pub const NOTES_EDITOR_PREVIEW_RENDER_PATH: &str = "components.notes_editor.render_preview";
pub const NOTES_EDITOR_OCCLUSION_ALPHA: u32 = 0xFF;

/// Layout padding for the editor body and preview surfaces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotesEditorLayout {
    pub padding_x: f32,
    pub padding_y: f32,
}

impl NotesEditorLayout {
    pub const fn new(padding_x: f32, padding_y: f32) -> Self {
        Self {
            padding_x,
            padding_y,
        }
    }
}

/// Initial configuration when constructing a [`super::NotesEditor`].
#[derive(Debug, Clone)]
pub struct NotesEditorConfig {
    pub placeholder: String,
    pub initial_content: String,
    pub layout: NotesEditorLayout,
    /// Stable host-specific identity for capture-only shaped text evidence.
    pub measurement_surface: String,
}

impl NotesEditorConfig {
    pub fn new(initial_content: impl Into<String>) -> Self {
        Self {
            placeholder: "Start typing your note...".to_string(),
            initial_content: initial_content.into(),
            layout: NotesEditorLayout::new(16.0, 12.0),
            measurement_surface: "notes-editor".to_string(),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn layout(mut self, layout: NotesEditorLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn measurement_surface(mut self, surface: impl Into<String>) -> Self {
        self.measurement_surface = surface.into();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesEditorInputSizing {
    Rows(usize),
    AutoGrow { min_rows: usize, max_rows: usize },
}

#[derive(Debug, Clone)]
pub struct NotesEditorMarkdownConfig {
    pub editor: NotesEditorConfig,
    pub sizing: NotesEditorInputSizing,
}

impl NotesEditorMarkdownConfig {
    pub fn new(initial_content: impl Into<String>) -> Self {
        Self {
            editor: NotesEditorConfig::new(initial_content),
            sizing: NotesEditorInputSizing::Rows(20),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.editor = self.editor.placeholder(placeholder);
        self
    }

    pub fn layout(mut self, layout: NotesEditorLayout) -> Self {
        self.editor = self.editor.layout(layout);
        self
    }

    pub fn measurement_surface(mut self, surface: impl Into<String>) -> Self {
        self.editor = self.editor.measurement_surface(surface);
        self
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.sizing = NotesEditorInputSizing::Rows(rows);
        self
    }

    pub fn auto_grow(mut self, min_rows: usize, max_rows: usize) -> Self {
        self.sizing = NotesEditorInputSizing::AutoGrow { min_rows, max_rows };
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotesEditorSurfaceStyle {
    pub owner: &'static str,
    pub input_render_path: &'static str,
    pub background_rgb: u32,
    pub occlusion_rgba: u32,
}

impl NotesEditorSurfaceStyle {
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        let background_rgb = theme.colors.background.main;
        Self {
            owner: NOTES_EDITOR_STYLE_OWNER,
            input_render_path: NOTES_EDITOR_INPUT_RENDER_PATH,
            background_rgb,
            occlusion_rgba: notes_editor_occlusion_rgba(background_rgb),
        }
    }
}

pub const fn notes_editor_occlusion_rgba(background_rgb: u32) -> u32 {
    (background_rgb << 8) | NOTES_EDITOR_OCCLUSION_ALPHA
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_editor_surface_occlusion_is_solid_theme_background() {
        assert_eq!(notes_editor_occlusion_rgba(0x112233), 0x112233ff);
    }

    #[test]
    fn notes_editor_preview_render_path_is_component_owned() {
        assert_eq!(
            NOTES_EDITOR_PREVIEW_RENDER_PATH,
            "components.notes_editor.render_preview"
        );
    }
}
