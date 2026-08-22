//! Agent Chat favorite model persistence.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

use super::config::AgentChatModelEntry;

static FAVORITE_MODELS_WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
struct FavoriteModelsFile {
    favorite_model_ids: Vec<String>,
}

fn favorites_path() -> std::path::PathBuf {
    #[cfg(test)]
    if let Ok(path) = std::env::var("AGENT_CHAT_FAVORITE_MODELS_PATH") {
        return std::path::PathBuf::from(path);
    }
    crate::setup::get_kit_path().join("agent_chat-favorite-models.json")
}

pub(crate) fn load_favorite_model_ids() -> Vec<String> {
    match load_favorite_model_ids_at(&favorites_path()) {
        Ok(ids) => ids,
        Err(error) => {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_favorite_models_load_failed",
                error_kind = ?error.kind(),
                diagnostic_fingerprint = %crate::ai::reliability::redacted_fingerprint(
                    &error.to_string()
                ),
            );
            Vec::new()
        }
    }
}

fn load_favorite_model_ids_at(path: &Path) -> std::io::Result<Vec<String>> {
    if !crate::atomic_file::inspect_private_file(path)? {
        return Ok(Vec::new());
    }

    let content = crate::atomic_file::read_private_file(path)?;
    let file = serde_json::from_str::<FavoriteModelsFile>(&content)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(normalize_favorites(file.favorite_model_ids))
}

fn save_favorite_model_ids_at(path: &Path, ids: &[String]) -> std::io::Result<()> {
    let file = FavoriteModelsFile {
        favorite_model_ids: normalize_favorites(ids.to_vec()),
    };
    let json = serde_json::to_vec_pretty(&file)?;
    crate::atomic_file::write_private_atomic(path, &json)
}

pub(crate) fn toggle_favorite_model_id(model_id: &str) -> std::io::Result<Vec<String>> {
    toggle_favorite_model_id_at(&favorites_path(), model_id)
}

fn toggle_favorite_model_id_at(path: &Path, model_id: &str) -> std::io::Result<Vec<String>> {
    let _owner = FAVORITE_MODELS_WRITE_LOCK
        .lock()
        .map_err(|_| std::io::Error::other("favorite model persistence lock poisoned"))?;
    let mut ids = load_favorite_model_ids_at(path)?;
    if let Some(index) = ids.iter().position(|id| id == model_id) {
        ids.remove(index);
    } else if !model_id.trim().is_empty() {
        ids.push(model_id.to_string());
    }
    save_favorite_model_ids_at(path, &ids)?;
    Ok(ids)
}

pub(crate) fn is_favorite_model_id(model_id: &str) -> bool {
    load_favorite_model_ids().iter().any(|id| id == model_id)
}

pub(crate) fn next_favorite_model_id(
    current_model_id: Option<&str>,
    favorite_ids: &[String],
    available_models: &[AgentChatModelEntry],
) -> Option<String> {
    let available_favorites = favorite_ids
        .iter()
        .filter(|id| available_models.iter().any(|model| model.id == **id))
        .cloned()
        .collect::<Vec<_>>();

    if available_favorites.is_empty() {
        return None;
    }

    let next_index = current_model_id
        .and_then(|current| available_favorites.iter().position(|id| id == current))
        .map(|index| (index + 1) % available_favorites.len())
        .unwrap_or(0);
    available_favorites.get(next_index).cloned()
}

fn normalize_favorites(ids: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for id in ids {
        let id = id.trim();
        if !id.is_empty() && !out.iter().any(|existing| existing == id) {
            out.push(id.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn favorite_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn model(id: &str) -> AgentChatModelEntry {
        AgentChatModelEntry {
            id: id.to_string(),
            display_name: Some(id.to_string()),
            context_window: None,
        }
    }

    #[test]
    fn favorite_models_toggle_and_persist_round_trip() {
        let _guard = favorite_env_lock().lock().expect("favorite env lock");
        let previous_path = std::env::var("AGENT_CHAT_FAVORITE_MODELS_PATH").ok();
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("favorites.json");
        std::env::set_var("AGENT_CHAT_FAVORITE_MODELS_PATH", &path);

        assert!(load_favorite_model_ids().is_empty());
        assert_eq!(
            toggle_favorite_model_id("m1").expect("persist favorite"),
            vec!["m1".to_string()]
        );
        assert_eq!(load_favorite_model_ids(), vec!["m1".to_string()]);
        assert!(toggle_favorite_model_id("m1")
            .expect("persist favorite removal")
            .is_empty());
        assert!(load_favorite_model_ids().is_empty());

        match previous_path {
            Some(path) => std::env::set_var("AGENT_CHAT_FAVORITE_MODELS_PATH", path),
            None => std::env::remove_var("AGENT_CHAT_FAVORITE_MODELS_PATH"),
        }
    }

    #[test]
    #[cfg(unix)]
    fn favorite_models_store_is_owner_only_and_repairs_legacy_permissions_before_read() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("favorites.json");
        let ids = vec!["private-custom-model".to_string()];

        save_favorite_model_ids_at(&path, &ids).expect("save private favorites");
        assert_eq!(
            std::fs::metadata(&path)
                .expect("favorite metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("make legacy file permissive");
        assert_eq!(
            load_favorite_model_ids_at(&path).expect("repair before loading private model"),
            ids
        );
        assert_eq!(
            std::fs::metadata(&path)
                .expect("repaired favorite metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    #[cfg(unix)]
    fn favorite_models_reject_planted_symlinks_without_reading_or_replacing_foreign_state() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temp dir");
        let foreign = temp.path().join("foreign.json");
        let path = temp.path().join("favorites.json");
        let original = br#"{"favoriteModelIds":["private-foreign-model"]}"#;
        std::fs::write(&foreign, original).expect("seed foreign favorite owner");
        symlink(&foreign, &path).expect("plant favorite symlink");

        assert!(load_favorite_model_ids_at(&path).is_err());
        assert!(save_favorite_model_ids_at(&path, &["new-model".to_string()]).is_err());
        assert!(toggle_favorite_model_id_at(&path, "new-model").is_err());
        assert_eq!(
            std::fs::read(&foreign).expect("foreign favorite owner remains untouched"),
            original
        );
        assert!(std::fs::symlink_metadata(&path)
            .expect("planted symlink remains in place")
            .file_type()
            .is_symlink());
    }

    #[test]
    fn favorite_models_malformed_store_refuses_toggle_without_erasing_existing_preferences() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("favorites.json");
        let original = br#"{"favoriteModelIds":["private-preserved-model""#;
        std::fs::write(&path, original).expect("seed malformed recoverable favorites");

        let error = toggle_favorite_model_id_at(&path, "new-model")
            .expect_err("malformed favorites must fail closed");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(
            std::fs::read(&path).expect("malformed favorite bytes remain recoverable"),
            original
        );
    }

    #[test]
    fn favorite_models_concurrent_toggles_preserve_every_successfully_saved_owner() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("favorites.json");

        std::thread::scope(|scope| {
            for index in 0..8 {
                let favorite_path = &path;
                scope.spawn(move || {
                    toggle_favorite_model_id_at(favorite_path, &format!("model-{index}"))
                        .expect("serialize and persist favorite owner");
                });
            }
        });

        let mut ids = load_favorite_model_ids_at(&path).expect("load every persisted owner");
        ids.sort();
        assert_eq!(
            ids,
            (0..8)
                .map(|index| format!("model-{index}"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn favorite_models_cycle_wraps_and_skips_missing() {
        let favorites = vec!["missing".to_string(), "m1".to_string(), "m2".to_string()];
        let available = vec![model("m1"), model("m2")];

        assert_eq!(
            next_favorite_model_id(None, &favorites, &available).as_deref(),
            Some("m1")
        );
        assert_eq!(
            next_favorite_model_id(Some("m1"), &favorites, &available).as_deref(),
            Some("m2")
        );
        assert_eq!(
            next_favorite_model_id(Some("m2"), &favorites, &available).as_deref(),
            Some("m1")
        );
        assert_eq!(
            next_favorite_model_id(Some("missing"), &favorites, &available).as_deref(),
            Some("m1")
        );
        assert!(
            next_favorite_model_id(Some("m1"), &[String::from("missing")], &available).is_none()
        );
    }
}
