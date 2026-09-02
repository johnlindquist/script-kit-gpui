#[derive(Clone, Copy, Debug)]
enum ThemeChooserSliderBinding {
    SurfaceOpacity,
    SecondaryTextOpacity,
    FocusedBackgroundOpacity,
    GlassVeilOpacity,
    GlassTintOpacity,
    GlassMorphDuration,
    GlassMorphInset,
    UiFontSize,
    GradientAngle { layer_index: Option<usize> },
    GradientOpacity { layer_index: Option<usize> },
}

#[derive(Clone, Copy, Debug)]
struct ThemeChooserSliderRange {
    min: f32,
    max: f32,
    step: f32,
    initial: f32,
}

#[derive(Clone, Copy, Debug)]
struct ThemeChooserGradientValues {
    from: u32,
    to: u32,
    angle: f32,
    opacity: f32,
}

#[derive(Clone, Copy, Debug)]
struct ThemeChooserManagementButtonStyle {
    text_hex: u32,
    bg_rgba: u32,
    border_rgba: u32,
    hover_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeChooserManagementButtonKind {
    Primary,
    Neutral,
    Destructive,
}

#[derive(Clone, Debug)]
pub(crate) enum ThemeChooserBase {
    BuiltIn {
        index: usize,
        name: String,
        fingerprint: u64,
    },
    User {
        slug: String,
        name: String,
        fingerprint: u64,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ThemeChooserSaveReceipt {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) fingerprint: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ThemeChooserDeleteCandidate {
    pub(crate) slug: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ThemeChooserDeletedTheme {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) contents: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ThemeChooserManagementSnapshot {
    pub(crate) status_label: String,
    pub(crate) status_value: String,
    pub(crate) status_kind: String,
    pub(crate) is_dirty: bool,
    pub(crate) save_name: String,
    pub(crate) resolved_save_name: String,
    pub(crate) duplicate_status_kind: Option<String>,
    pub(crate) base_name: Option<String>,
    pub(crate) base_slug: Option<String>,
    pub(crate) can_update: bool,
    pub(crate) update_disabled: Option<String>,
    pub(crate) delete_disabled: Option<String>,
    pub(crate) restore_disabled: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ThemeChooserMatchSummary {
    catalog_total: usize,
    catalog_dark: usize,
    catalog_light: usize,
    visible_total: usize,
    visible_dark: usize,
    visible_light: usize,
}

#[derive(Clone, Debug)]
struct ThemeChooserContrastRow {
    label: String,
    ratio: f32,
    minimum: f32,
    passes: bool,
}

#[derive(Clone, Debug)]
struct ThemeChooserContrastSnapshot {
    rows: Vec<ThemeChooserContrastRow>,
    passing: usize,
    total: usize,
    worst_label: String,
    worst_ratio: f32,
}

fn build_theme_chooser_contrast_snapshot(
    theme: &crate::theme::Theme,
) -> ThemeChooserContrastSnapshot {
    let rows = theme::audit_theme_contrast(theme)
        .into_iter()
        .map(|sample| ThemeChooserContrastRow {
            label: sample.label.to_string(),
            ratio: sample.ratio,
            minimum: sample.minimum,
            passes: sample.passes(),
        })
        .collect::<Vec<_>>();

    let passing = rows.iter().filter(|row| row.passes).count();
    let total = rows.len();

    let worst = rows
        .iter()
        .min_by(|left, right| {
            left.ratio
                .partial_cmp(&right.ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
        .unwrap_or(ThemeChooserContrastRow {
            label: "n/a".to_string(),
            ratio: 0.0,
            minimum: 4.5,
            passes: false,
        });

    ThemeChooserContrastSnapshot {
        rows,
        passing,
        total,
        worst_label: worst.label,
        worst_ratio: worst.ratio,
    }
}

fn cached_theme_chooser_contrast_snapshot(
    theme: &std::sync::Arc<crate::theme::Theme>,
) -> ThemeChooserContrastSnapshot {
    static THEME_CHOOSER_CONTRAST_CACHE: std::sync::LazyLock<
        parking_lot::Mutex<std::collections::HashMap<usize, ThemeChooserContrastSnapshot>>,
    > = std::sync::LazyLock::new(|| parking_lot::Mutex::new(std::collections::HashMap::new()));

    let cache_key = std::sync::Arc::as_ptr(theme) as usize;

    if let Some(snapshot) = THEME_CHOOSER_CONTRAST_CACHE.lock().get(&cache_key).cloned() {
        return snapshot;
    }

    let snapshot = build_theme_chooser_contrast_snapshot(theme.as_ref());

    let mut cache = THEME_CHOOSER_CONTRAST_CACHE.lock();
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(cache_key, snapshot.clone());
    snapshot
}

impl ScriptListApp {
    pub(crate) fn theme_chooser_theme_fingerprint(theme: &crate::theme::Theme) -> u64 {
        use std::hash::{Hash, Hasher};
        let bytes = serde_json::to_vec(theme).unwrap_or_default();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }
}
