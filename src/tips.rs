//! Local Tips catalog. A future `refresh_tips_remote()` may follow
//! `updates::check_now`: use a detached thread and `ureq` GET, validate the
//! response, update shared state, then optionally `write_string_if_changed`.
//! This module intentionally performs no network IO.

use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tip {
    pub id: String,
    /// Key/sigil that invokes the trick (";", "Space", "⌘↵"). Rendered as a
    /// footer-style keycap chip; None for tips with no single trigger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint_key: Option<String>,
    /// Hint text WITHOUT the key (the keycap carries it).
    pub hint: String,
    pub title: String,
    pub description: String,
    pub examples: Vec<TipExample>,
    pub keywords: Vec<String>,
}

impl Tip {
    /// Key + hint as one sentence, for surfaces without keycap chrome
    /// (list subtitles, search).
    pub fn full_hint(&self) -> String {
        match self.hint_key.as_deref() {
            Some(key) => format!("{key} {}", self.hint),
            None => self.hint.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TipExample {
    pub input: String,
    pub note: String,
}

static TIPS: OnceLock<Vec<Tip>> = OnceLock::new();
static TIP_ROTATION: AtomicUsize = AtomicUsize::new(0);

/// Placeholder resolved at load time to the registered main hotkey's display
/// glyphs (e.g. "⌘;"), so tips about the hotkey always show the real binding.
const MAIN_HOTKEY_PLACEHOLDER: &str = "{mainHotkey}";

fn resolve_tip_placeholders(mut tips: Vec<Tip>, main_hotkey: &str) -> Vec<Tip> {
    let substitute = |text: &mut String| *text = text.replace(MAIN_HOTKEY_PLACEHOLDER, main_hotkey);
    for tip in &mut tips {
        if let Some(hint_key) = tip.hint_key.as_mut() {
            substitute(hint_key);
        }
        substitute(&mut tip.hint);
        substitute(&mut tip.title);
        substitute(&mut tip.description);
        for example in &mut tip.examples {
            substitute(&mut example.input);
            substitute(&mut example.note);
        }
    }
    tips
}

fn tips_json_path() -> PathBuf {
    crate::setup::get_kit_path().join("tips.json")
}
fn embedded_tips() -> Vec<Tip> {
    serde_json::from_str(crate::setup::EMBEDDED_TIPS).unwrap_or_else(|error| {
        tracing::warn!(%error, "embedded tips catalog is invalid");
        Vec::new()
    })
}

pub fn load_tips() -> Vec<Tip> {
    let path = tips_json_path();
    let tips = match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|error| {
            tracing::warn!(%error, path = %path.display(), "failed to parse tips catalog; using defaults");
            embedded_tips()
        }),
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "failed to read tips catalog; using defaults");
            embedded_tips()
        }
    };
    resolve_tip_placeholders(
        tips,
        &crate::config::load_config().hotkey.to_display_string(),
    )
}

/// Single filter authority for the Tips browser. The renderer, the key
/// handler, the visible-count label, and the native footer button enablement
/// all consume this so the list rows, the "N tips" count, and the footer can
/// never disagree about what matches.
pub fn tip_matches_filter(tip: &Tip, query_lowercase: &str) -> bool {
    query_lowercase.is_empty()
        || tip.title.to_lowercase().contains(query_lowercase)
        || tip.full_hint().to_lowercase().contains(query_lowercase)
        || tip.description.to_lowercase().contains(query_lowercase)
        || tip
            .keywords
            .iter()
            .any(|keyword| keyword.to_lowercase().contains(query_lowercase))
}

/// Indices into `entries` visible under `filter`, in catalog order.
pub fn visible_tip_indices(entries: &[Tip], filter: &str) -> Vec<usize> {
    let query = filter.trim().to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, tip)| tip_matches_filter(tip, &query).then_some(index))
        .collect()
}

pub fn current_footer_tip() -> Option<Tip> {
    let tips = TIPS.get_or_init(load_tips);
    (!tips.is_empty()).then(|| tips[TIP_ROTATION.load(Ordering::Relaxed) % tips.len()].clone())
}
pub fn advance_footer_tip() {
    TIP_ROTATION.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_kit(contents: Option<&str>, test: impl FnOnce()) {
        let _guard = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let previous = std::env::var_os(crate::setup::SK_PATH_ENV);
        std::env::set_var("SK_PATH", dir.path());
        if let Some(value) = contents {
            std::fs::write(dir.path().join("tips.json"), value).unwrap();
        }
        test();
        match previous {
            Some(value) => std::env::set_var(crate::setup::SK_PATH_ENV, value),
            None => std::env::remove_var(crate::setup::SK_PATH_ENV),
        }
    }
    #[test]
    fn missing_file_uses_embedded_defaults() {
        with_temp_kit(None, || assert!(!load_tips().is_empty()));
    }
    #[test]
    fn corrupt_file_uses_embedded_defaults() {
        with_temp_kit(Some("{"), || assert!(!load_tips().is_empty()));
    }
    #[test]
    fn visible_tip_indices_match_all_text_fields() {
        let tip = |id: &str, title: &str, hint: &str, description: &str, keyword: &str| Tip {
            id: id.into(),
            hint_key: None,
            hint: hint.into(),
            title: title.into(),
            description: description.into(),
            examples: Vec::new(),
            keywords: vec![keyword.into()],
        };
        let entries = vec![
            tip("a", "Capture a thought", "hint-a", "desc-a", "brain"),
            tip("b", "Jump to Today", "hint-b", "unique-description", "day"),
            tip("c", "Slash commands", "unique-hint", "desc-c", "slash"),
        ];
        assert_eq!(visible_tip_indices(&entries, ""), vec![0, 1, 2]);
        assert_eq!(visible_tip_indices(&entries, "  Capture "), vec![0]);
        assert_eq!(visible_tip_indices(&entries, "unique-description"), vec![1]);
        assert_eq!(visible_tip_indices(&entries, "UNIQUE-HINT"), vec![2]);
        assert_eq!(visible_tip_indices(&entries, "brain"), vec![0]);
        assert!(visible_tip_indices(&entries, "no-match").is_empty());
    }

    #[test]
    fn resolve_tip_placeholders_substitutes_main_hotkey_everywhere() {
        let tips = vec![Tip {
            id: "double-tap-quick-ai".into(),
            hint_key: Some(MAIN_HOTKEY_PLACEHOLDER.into()),
            hint: "twice for Quick AI".into(),
            title: "Quick AI question".into(),
            description: format!("Tap {MAIN_HOTKEY_PLACEHOLDER} twice."),
            examples: vec![TipExample {
                input: format!("(double-tap {MAIN_HOTKEY_PLACEHOLDER})"),
                note: "note".into(),
            }],
            keywords: Vec::new(),
        }];
        let resolved = resolve_tip_placeholders(tips, "⌘;");
        assert_eq!(resolved[0].hint_key.as_deref(), Some("⌘;"));
        assert_eq!(resolved[0].description, "Tap ⌘; twice.");
        assert_eq!(resolved[0].examples[0].input, "(double-tap ⌘;)");
        assert_eq!(resolved[0].full_hint(), "⌘; twice for Quick AI");
    }

    #[test]
    fn embedded_catalog_quick_ai_tip_uses_main_hotkey_placeholder() {
        let tips = embedded_tips();
        let quick_ai = tips
            .iter()
            .find(|tip| tip.id == "double-tap-quick-ai")
            .expect("embedded catalog should keep the quick AI tip");
        assert_eq!(quick_ai.hint_key.as_deref(), Some(MAIN_HOTKEY_PLACEHOLDER));
        assert_eq!(quick_ai.hint, "twice for Quick AI");
    }

    #[test]
    fn parses_local_catalog() {
        let json = r#"[{"id":"x","hint":"h","title":"t","description":"d","examples":[{"input":"i","note":"n"}],"keywords":["k"]}]"#;
        with_temp_kit(Some(json), || assert_eq!(load_tips()[0].id, "x"));
    }
}
