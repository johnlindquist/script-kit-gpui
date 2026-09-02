fn default_selected_opacity() -> f32 {
    DARK_ROW_SELECTED_OPACITY
}

fn default_hover_opacity() -> f32 {
    DARK_ROW_HOVER_OPACITY
}

fn default_preview_opacity() -> f32 {
    0.50
}

fn default_dialog_opacity() -> f32 {
    0.50
}

fn default_input_opacity() -> f32 {
    0.50
}

fn default_panel_opacity() -> f32 {
    0.50
}

fn default_input_inactive_opacity() -> f32 {
    0.50
}

fn default_input_active_opacity() -> f32 {
    0.50
}

fn default_border_inactive_opacity() -> f32 {
    0.125 // 0x20 / 255 ≈ 0.125
}

fn default_border_active_opacity() -> f32 {
    0.25 // 0x40 / 255 ≈ 0.25
}

// ── Text grading defaults (Liquid Glass) ─────────────────────────────
// Applied to text_primary. Keep labels bright while supporting copy recedes
// on translucent 50% surfaces; do not reintroduce secondary/muted hex dimming.
const TEXT_NAME_OPACITY: f32 = 1.00; // Names / primary labels (0xFF)
const TEXT_STRONG_OPACITY: f32 = 0.80; // Badges, shortcuts, section headers (0xCC)
const TEXT_MUTED_OPACITY: f32 = 0.65; // Focused descriptions, source hints (0xA5)
const TEXT_HINT_OPACITY: f32 = 0.45; // Hovered descriptions, type labels (0x72)
const TEXT_PLACEHOLDER_OPACITY: f32 = 0.40; // Placeholders, idle captions (0x66)
const TEXT_ICON_OPACITY: f32 = 0.50; // Idle icons (0x7F)

fn default_text_name() -> f32 {
    TEXT_NAME_OPACITY
}
fn default_text_strong() -> f32 {
    TEXT_STRONG_OPACITY
}
fn default_text_muted() -> f32 {
    TEXT_MUTED_OPACITY
}
fn default_text_hint() -> f32 {
    TEXT_HINT_OPACITY
}
fn default_text_placeholder() -> f32 {
    TEXT_PLACEHOLDER_OPACITY
}
fn default_text_icon() -> f32 {
    TEXT_ICON_OPACITY
}
