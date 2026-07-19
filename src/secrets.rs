//! Secrets storage using age encryption
//!
//! Provides secure secret storage using age (https://age-encryption.org) with
//! scrypt passphrase-based encryption. Secrets are stored as encrypted JSON
//! in ~/.scriptkit/secrets.age.
//!
//! The passphrase is derived from machine-specific identifiers (hostname + app ID)
//! for transparent encryption without requiring user interaction.
//!
//! ## Performance
//!
//! Secrets are cached in memory after first load to avoid repeated scrypt
//! decryption (~1.3s per call). The cache is invalidated on write operations.
//!
//! ## Security
//!
//! - At-rest: Secrets encrypted with age/scrypt in ~/.scriptkit/secrets.age
//! - In-memory: Decrypted cache (standard practice for desktop apps)
//! - Cache cleared on app exit (process memory reclaimed by OS)
//!
//! API matches the keyring functions in prompts/env.rs for easy migration:
//! - `get_secret(key: &str) -> Option<String>`
//! - `set_secret(key: &str, value: &str) -> Result<(), String>`
//! - `delete_secret(key: &str) -> Result<(), String>`
//! - `get_secret_info(key: &str) -> Option<SecretInfo>` - includes metadata

use age::secrecy::SecretString;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::iter;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use crate::logging;

/// A secret entry with value and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretEntry {
    /// The secret value
    pub value: String,
    /// When the secret was last modified (ISO 8601)
    pub modified_at: DateTime<Utc>,
}

/// Information about a stored secret (returned to callers)
#[derive(Debug, Clone)]
pub struct SecretInfo {
    /// The secret value
    pub value: String,
    /// When the secret was last modified
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretStoreErrorKind {
    PathUnavailable,
    ReadFailed,
    InvalidFormat,
    UnsupportedEncryption,
    DecryptFailed,
    ParseFailed,
    CacheUnavailable,
}

impl SecretStoreErrorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SecretStoreErrorKind::PathUnavailable => "path_unavailable",
            SecretStoreErrorKind::ReadFailed => "read_failed",
            SecretStoreErrorKind::InvalidFormat => "invalid_format",
            SecretStoreErrorKind::UnsupportedEncryption => "unsupported_encryption",
            SecretStoreErrorKind::DecryptFailed => "decrypt_failed",
            SecretStoreErrorKind::ParseFailed => "parse_failed",
            SecretStoreErrorKind::CacheUnavailable => "cache_unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretStoreError {
    pub kind: SecretStoreErrorKind,
    pub message: String,
}

impl SecretStoreError {
    fn new(kind: SecretStoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind_str(&self) -> &'static str {
        self.kind.as_str()
    }

    pub fn user_message(&self) -> &'static str {
        match self.kind {
            SecretStoreErrorKind::PathUnavailable => {
                "Secret storage location is unavailable. Check your home directory and try again."
            }
            SecretStoreErrorKind::ReadFailed => {
                "Secret storage could not be read. Check file permissions and try again."
            }
            SecretStoreErrorKind::InvalidFormat
            | SecretStoreErrorKind::UnsupportedEncryption
            | SecretStoreErrorKind::DecryptFailed
            | SecretStoreErrorKind::ParseFailed => {
                "Saved secrets could not be loaded. The store may be corrupt or from another machine."
            }
            SecretStoreErrorKind::CacheUnavailable => {
                "Secret storage is temporarily unavailable. Try again."
            }
        }
    }
}

impl From<SecretEntry> for SecretInfo {
    fn from(entry: SecretEntry) -> Self {
        SecretInfo {
            value: entry.value,
            modified_at: entry.modified_at,
        }
    }
}

/// In-memory cache of decrypted secrets with metadata.
/// Avoids repeated scrypt decryption which takes ~1.3s per call.
type SecretsCache = Option<Result<HashMap<String, SecretEntry>, SecretStoreError>>;

static SECRETS_CACHE: LazyLock<Mutex<SecretsCache>> = LazyLock::new(|| Mutex::new(None));

/// Get the secrets cache mutex.
fn secrets_cache() -> &'static Mutex<SecretsCache> {
    &SECRETS_CACHE
}

/// Get cached secrets, loading from disk if not yet cached.
fn get_cached_secrets() -> Result<HashMap<String, SecretEntry>, SecretStoreError> {
    let mut guard = secrets_cache().lock().map_err(|e| {
        SecretStoreError::new(
            SecretStoreErrorKind::CacheUnavailable,
            format!("Secrets cache lock poisoned: {e}"),
        )
    })?;
    if let Some(ref cached) = *guard {
        return cached.clone();
    }

    // First access - load from disk and cache
    match load_secrets_from_disk() {
        Ok(secrets) => {
            *guard = Some(Ok(secrets.clone()));
            Ok(secrets)
        }
        Err(error) => Err(error),
    }
}

/// Update the cache after a write operation.
fn update_cache(secrets: HashMap<String, SecretEntry>) -> anyhow::Result<()> {
    let mut guard = secrets_cache()
        .lock()
        .map_err(|e| anyhow::anyhow!("Secrets cache lock poisoned: {e}"))?;
    *guard = Some(Ok(secrets));
    Ok(())
}

/// Warm up the secrets cache (call at app startup).
/// Loads and decrypts secrets in the background so they're ready when needed.
pub fn warmup_cache() {
    std::thread::spawn(|| {
        let start = std::time::Instant::now();
        let secrets = match get_cached_secrets() {
            Ok(secrets) => secrets,
            Err(error) => {
                tracing::error!(
                    kind = error.kind_str(),
                    "Failed to warm secrets cache: {}",
                    error.message
                );
                HashMap::new()
            }
        };
        let elapsed = start.elapsed();
        logging::log(
            "SECRETS",
            &format!(
                "Warmed up secrets cache: {} keys in {:.2}s",
                secrets.len(),
                elapsed.as_secs_f64()
            ),
        );
    });
}

/// App identifier used in passphrase derivation
const APP_IDENTIFIER: &str = "com.scriptkit.secrets";

/// Get the path to the secrets file
fn secrets_path() -> anyhow::Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    Ok(home.join(".scriptkit").join("secrets.age"))
}

/// Derive a machine-specific passphrase
///
/// Combines the system hostname with the app identifier to create a passphrase
/// that is unique to this machine but consistent across app restarts.
fn derive_passphrase() -> SecretString {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown-host".to_string());

    // Combine hostname + app identifier for machine-specific passphrase
    SecretString::from(format!("{}:{}", hostname, APP_IDENTIFIER))
}

fn load_secrets_from_path(
    path: &std::path::Path,
) -> Result<HashMap<String, SecretEntry>, SecretStoreError> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let encrypted_data = match fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            return Err(SecretStoreError::new(
                SecretStoreErrorKind::ReadFailed,
                format!("Failed to read secrets file: {e}"),
            ));
        }
    };

    if encrypted_data.is_empty() {
        return Ok(HashMap::new());
    }

    let passphrase = derive_passphrase();
    let identity = age::scrypt::Identity::new(passphrase);

    let decryptor = match age::Decryptor::new(&encrypted_data[..]) {
        Ok(d) => d,
        Err(e) => {
            return Err(SecretStoreError::new(
                SecretStoreErrorKind::InvalidFormat,
                format!("Failed to create decryptor: {e}"),
            ));
        }
    };

    // Verify it's a passphrase-encrypted file
    if !decryptor.is_scrypt() {
        return Err(SecretStoreError::new(
            SecretStoreErrorKind::UnsupportedEncryption,
            "Secrets file is not passphrase-encrypted",
        ));
    }

    let mut decrypted = vec![];
    let mut reader = match decryptor.decrypt(iter::once(&identity as &dyn age::Identity)) {
        Ok(r) => r,
        Err(e) => {
            return Err(SecretStoreError::new(
                SecretStoreErrorKind::DecryptFailed,
                format!("Failed to decrypt secrets: {e}"),
            ));
        }
    };

    if let Err(e) = reader.read_to_end(&mut decrypted) {
        return Err(SecretStoreError::new(
            SecretStoreErrorKind::DecryptFailed,
            format!("Failed to read decrypted data: {e}"),
        ));
    }

    // Try to parse as new format first
    if let Ok(secrets) = serde_json::from_slice::<HashMap<String, SecretEntry>>(&decrypted) {
        logging::log("SECRETS", "Loaded secrets in new format with metadata");
        return Ok(secrets);
    }

    // Fall back to old format and migrate
    if let Ok(old_secrets) = serde_json::from_slice::<HashMap<String, String>>(&decrypted) {
        logging::log(
            "SECRETS",
            &format!(
                "Migrating {} secrets from old format to new format with timestamps",
                old_secrets.len()
            ),
        );
        let now = Utc::now();
        let migrated: HashMap<String, SecretEntry> = old_secrets
            .into_iter()
            .map(|(key, value)| {
                (
                    key,
                    SecretEntry {
                        value,
                        modified_at: now,
                    },
                )
            })
            .collect();
        return Ok(migrated);
    }

    Err(SecretStoreError::new(
        SecretStoreErrorKind::ParseFailed,
        "Failed to parse secrets JSON in any format",
    ))
}

/// Load and decrypt the secrets store from disk.
/// This is slow (~1.3s) due to scrypt. Use get_cached_secrets() instead.
///
/// Handles migration from old format (HashMap<String, String>) to new format
/// (HashMap<String, SecretEntry>) automatically.
fn load_secrets_from_disk() -> Result<HashMap<String, SecretEntry>, SecretStoreError> {
    let path = secrets_path().map_err(|e| {
        SecretStoreError::new(
            SecretStoreErrorKind::PathUnavailable,
            format!("Failed to resolve secrets path when loading from disk: {e}"),
        )
    })?;

    let result = load_secrets_from_path(&path);
    if let Err(error) = &result {
        logging::log(
            "SECRETS",
            &format!(
                "Failed to load secrets store kind={} error={}",
                error.kind_str(),
                error.message
            ),
        );
    }
    result
}

/// Encrypt and save the secrets store to the default path.
fn save_secrets(secrets: &HashMap<String, SecretEntry>) -> Result<(), String> {
    let path = secrets_path().map_err(|e| format!("Failed to resolve secrets path: {e}"))?;
    save_secrets_to_path(secrets, &path)
}

/// Encrypt and atomically persist the secrets store to `path`.
///
/// The encrypted bytes are written to a sibling temp file (created 0o600 on
/// Unix) and then renamed over the destination. This matters for two reasons:
///
/// 1. **Atomicity.** A crash or SIGKILL during a direct in-place `truncate` +
///    `write_all` could leave `secrets.age` truncated and undecryptable —
///    losing *every* stored secret. A temp-file rename is a same-filesystem
///    atomic replace, so an interrupted save leaves the previous store intact.
/// 2. **Permission enforcement.** `OpenOptions::mode(0o600)` is ignored when the
///    destination already exists, so the previous in-place write silently left a
///    pre-existing loose-permission file untouched. A fresh temp file is created
///    0o600 every time and then replaces the destination, so the secrets file is
///    always owner-only after a save.
fn save_secrets_to_path(
    secrets: &HashMap<String, SecretEntry>,
    path: &std::path::Path,
) -> Result<(), String> {
    // Ensure parent directory exists
    let parent = path
        .parent()
        .ok_or_else(|| format!("Secrets path has no parent directory: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("Failed to create secrets directory: {}", e))?;

    let json =
        serde_json::to_vec(secrets).map_err(|e| format!("Failed to serialize secrets: {}", e))?;

    let passphrase = derive_passphrase();

    // Use with_user_passphrase for simple passphrase-based encryption
    let encryptor = age::Encryptor::with_user_passphrase(passphrase);

    let mut encrypted = vec![];
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|e| format!("Failed to create encryption writer: {}", e))?;

    writer
        .write_all(&json)
        .map_err(|e| format!("Failed to write encrypted data: {}", e))?;

    writer
        .finish()
        .map_err(|e| format!("Failed to finish encryption: {}", e))?;

    // Encrypt into a sibling temp file, then atomically rename it over the
    // destination.
    let mut temp_file = tempfile::Builder::new()
        .prefix(".secrets-")
        .suffix(".age.tmp")
        .tempfile_in(parent)
        .map_err(|e| format!("Failed to create temp secrets file: {}", e))?;

    // Write secrets file with restrictive permissions (0o600 - owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = temp_file
            .as_file()
            .metadata()
            .map_err(|e| format!("Failed to read temp secrets metadata: {}", e))?
            .permissions();
        perms.set_mode(0o600);
        temp_file.as_file().set_permissions(perms).map_err(|e| {
            format!(
                "Failed to set secure permissions on temp secrets file: {}",
                e
            )
        })?;
    }

    temp_file
        .write_all(&encrypted)
        .map_err(|e| format!("Failed to write encrypted secrets to temp file: {}", e))?;
    temp_file
        .as_file()
        .sync_all()
        .map_err(|e| format!("Failed to sync temp secrets file: {}", e))?;

    temp_file
        .persist(path)
        .map_err(|e| format!("Failed to atomically replace secrets file: {}", e.error))?;

    logging::log(
        "SECRETS",
        &format!("Saved {} secrets to {:?} (atomic)", secrets.len(), path),
    );
    Ok(())
}

/// Get a secret value from the encrypted store
///
/// Returns `Some(value)` if the secret exists, `None` otherwise.
/// For metadata (modification time), use `get_secret_info()` instead.
///
/// # Example
/// ```ignore
/// if let Some(api_key) = get_secret("OPENAI_API_KEY") {
///     // Use the key
/// }
/// ```
pub fn get_secret(key: &str) -> Option<String> {
    match get_secret_result(key) {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(
                key,
                kind = error.kind_str(),
                "Failed to get cached secrets for get_secret: {}",
                error.message
            );
            None
        }
    }
}

pub fn get_secret_result(key: &str) -> Result<Option<String>, SecretStoreError> {
    let secrets = get_cached_secrets()?;
    let result = secrets.get(key).map(|entry| entry.value.clone());

    if result.is_some() {
        logging::log("SECRETS", &format!("Retrieved secret for key: {}", key));
    } else {
        logging::log("SECRETS", &format!("No secret found for key: {}", key));
    }

    Ok(result)
}

/// Get a secret with its metadata from the encrypted store
///
/// Returns `Some(SecretInfo)` if the secret exists, including the value
/// and when it was last modified. Returns `None` if not found.
///
/// # Example
/// ```ignore
/// if let Some(info) = get_secret_info("OPENAI_API_KEY") {
///     println!("Key set on: {}", info.modified_at);
///     // Use info.value
/// }
/// ```
pub fn get_secret_info(key: &str) -> Option<SecretInfo> {
    match get_secret_info_result(key) {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(
                key,
                kind = error.kind_str(),
                "Failed to get cached secrets for get_secret_info: {}",
                error.message
            );
            None
        }
    }
}

pub fn get_secret_info_result(key: &str) -> Result<Option<SecretInfo>, SecretStoreError> {
    let secrets = get_cached_secrets()?;
    let result = secrets
        .get(key)
        .map(|entry| SecretInfo::from(entry.clone()));

    if result.is_some() {
        logging::log(
            "SECRETS",
            &format!("Retrieved secret info for key: {}", key),
        );
    } else {
        logging::log("SECRETS", &format!("No secret found for key: {}", key));
    }

    Ok(result)
}

/// Set a secret in the encrypted store
///
/// Creates or updates the secret with the given key.
/// Updates the modification timestamp to now.
///
/// # Example
/// ```ignore
/// set_secret("OPENAI_API_KEY", "sk-...")?;
/// ```
pub fn set_secret(key: &str, value: &str) -> Result<(), String> {
    let mut secrets = get_cached_secrets()
        .map_err(|e| format!("Failed to get cached secrets for set_secret: {}", e.message))?;
    secrets.insert(
        key.to_string(),
        SecretEntry {
            value: value.to_string(),
            modified_at: Utc::now(),
        },
    );
    save_secrets(&secrets)?;

    // Update the in-memory cache
    update_cache(secrets).map_err(|e| format!("Failed to update secrets cache: {e}"))?;

    logging::log("SECRETS", &format!("Stored secret for key: {}", key));
    Ok(())
}

/// Delete a secret from the encrypted store
///
/// Removes the secret if it exists. Returns Ok even if the key doesn't exist.
///
/// # Example
/// ```ignore
/// delete_secret("OPENAI_API_KEY")?;
/// ```
pub fn delete_secret(key: &str) -> Result<(), String> {
    let mut secrets = get_cached_secrets().map_err(|e| {
        format!(
            "Failed to get cached secrets for delete_secret: {}",
            e.message
        )
    })?;

    if secrets.remove(key).is_some() {
        save_secrets(&secrets)?;
        // Update the in-memory cache
        update_cache(secrets).map_err(|e| format!("Failed to update secrets cache: {e}"))?;
        logging::log("SECRETS", &format!("Deleted secret for key: {}", key));
    } else {
        logging::log("SECRETS", &format!("No secret to delete for key: {}", key));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_passphrase_derivation() {
        let passphrase = derive_passphrase();
        // SecretString doesn't expose its content easily in tests,
        // but we can verify it was created successfully
        drop(passphrase);
    }

    #[test]
    fn test_secrets_path() {
        let path = secrets_path().expect("secrets path should resolve in tests");
        assert!(path.ends_with("secrets.age"));
        assert!(path.to_string_lossy().contains(".scriptkit"));
    }

    fn encrypted_test_payload(bytes: &[u8]) -> Vec<u8> {
        let passphrase = derive_passphrase();
        let encryptor = age::Encryptor::with_user_passphrase(passphrase);
        let mut encrypted = vec![];
        let mut writer = encryptor.wrap_output(&mut encrypted).unwrap();
        writer.write_all(bytes).unwrap();
        writer.finish().unwrap();
        encrypted
    }

    #[test]
    fn missing_secrets_file_loads_as_empty_store() {
        let temp_dir = TempDir::new().unwrap();
        let missing_path = temp_dir.path().join("missing-secrets.age");

        let secrets = load_secrets_from_path(&missing_path).expect("missing file is not an error");

        assert!(secrets.is_empty());
    }

    #[test]
    fn invalid_secret_store_format_is_not_missing_secret() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("invalid-secrets.age");
        fs::write(&path, b"not an age file").unwrap();

        let error = load_secrets_from_path(&path).expect_err("invalid store should be distinct");

        assert_eq!(error.kind, SecretStoreErrorKind::InvalidFormat);
    }

    #[test]
    fn unparsable_decrypted_secret_store_is_not_missing_secret() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("invalid-json-secrets.age");
        fs::write(&path, encrypted_test_payload(b"not-json")).unwrap();

        let error = load_secrets_from_path(&path).expect_err("parse failure should be distinct");

        assert_eq!(error.kind, SecretStoreErrorKind::ParseFailed);
    }

    #[test]
    fn unreadable_secret_store_path_is_not_missing_secret() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("secret-store-directory.age");
        fs::create_dir(&path).unwrap();

        let error = load_secrets_from_path(&path).expect_err("read failure should be distinct");

        assert_eq!(error.kind, SecretStoreErrorKind::ReadFailed);
    }

    #[test]
    #[cfg(unix)]
    fn test_secrets_file_has_secure_permissions() {
        use std::os::unix::fs::PermissionsExt;

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().join("test-secrets.age");

        // Create test secrets
        let mut test_secrets = HashMap::new();
        test_secrets.insert(
            "TEST_KEY".to_string(),
            SecretEntry {
                value: "test_value".to_string(),
                modified_at: chrono::Utc::now(),
            },
        );

        // Manually save secrets to temp path (copy save_secrets logic)
        let json = serde_json::to_vec(&test_secrets).unwrap();
        let passphrase = derive_passphrase();
        let encryptor = age::Encryptor::with_user_passphrase(passphrase);
        let mut encrypted = vec![];
        let mut writer = encryptor.wrap_output(&mut encrypted).unwrap();
        writer.write_all(&json).unwrap();
        writer.finish().unwrap();

        // Write with secure permissions
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp_path)
            .and_then(|mut file| file.write_all(&encrypted))
            .unwrap();

        // Verify file permissions
        let metadata = fs::metadata(&temp_path).unwrap();
        let mode = metadata.permissions().mode();
        let file_perms = mode & 0o777;

        assert_eq!(
            file_perms, 0o600,
            "Secrets file should have 0o600 permissions, got 0o{:o}",
            file_perms
        );
    }

    #[test]
    #[cfg(unix)]
    fn save_secrets_to_path_is_atomic_and_reenforces_0600_on_existing_file() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("secrets.age");

        // Simulate a stale / loose pre-existing store (e.g. written by an older
        // build, or chmod'd by the user). Then empirically characterize the bug
        // the fix addresses: re-opening an *existing* file with `mode(0o600)`
        // does NOT tighten it — which is exactly what the previous in-place
        // `save_secrets` did, silently leaving loose permissions in place.
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o644)
            .open(&path)
            .unwrap()
            .write_all(b"stale")
            .unwrap();
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .unwrap()
            .write_all(b"still-loose")
            .unwrap();
        let leftover = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            leftover, 0o644,
            "characterization: OpenOptions::mode is ignored on an existing file (the bug)"
        );

        // The fixed save must re-establish 0o600 and round-trip the contents.
        let mut secrets = HashMap::new();
        secrets.insert(
            "API_KEY".to_string(),
            SecretEntry {
                value: "sk-secret".to_string(),
                modified_at: Utc::now(),
            },
        );
        save_secrets_to_path(&secrets, &path).expect("atomic save should succeed");

        // (a) Permissions are re-tightened to 0o600 even though the destination
        // already existed at 0o644. Fails if the fix regresses to an in-place write.
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "secrets file must be 0o600 after save, got 0o{mode:o}"
        );

        // (b) Contents decrypt through the real load path.
        let loaded = load_secrets_from_path(&path).expect("saved store must decrypt");
        assert_eq!(
            loaded.get("API_KEY").map(|entry| entry.value.as_str()),
            Some("sk-secret")
        );

        // (c) The atomic rename leaves no temp file litter behind.
        let leftovers: Vec<String> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "secrets.age")
            .collect();
        assert!(
            leftovers.is_empty(),
            "no temp files should remain after atomic save, found: {leftovers:?}"
        );
    }

    // Integration tests that actually read/write would go here
    // but should be feature-gated to avoid modifying user data
}
