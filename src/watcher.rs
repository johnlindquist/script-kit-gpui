#![allow(dead_code)]
use notify::{recommended_watcher, RecursiveMode, Result as NotifyResult, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use std::process::Command;
use tracing::{info, warn};

/// Event emitted when config needs to be reloaded
#[derive(Debug, Clone)]
pub enum ConfigReloadEvent {
    Reload,
}

/// Event emitted when theme needs to be reloaded
#[derive(Debug, Clone)]
pub enum ThemeReloadEvent {
    Reload,
}

/// Event emitted when scripts need to be reloaded
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptReloadEvent {
    /// A specific file was modified
    FileChanged(PathBuf),
    /// A new file was created
    FileCreated(PathBuf),
    /// A file was deleted
    FileDeleted(PathBuf),
    /// Fallback for complex events (e.g., bulk changes, renames)
    FullReload,
}

/// Event emitted when system appearance changes (light/dark mode)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppearanceChangeEvent {
    /// Dark mode is now active
    Dark,
    /// Light mode is now active
    Light,
}

/// Watches ~/.kenv/config.ts for changes and emits reload events
pub struct ConfigWatcher {
    tx: Option<Sender<ConfigReloadEvent>>,
    watcher_thread: Option<thread::JoinHandle<()>>,
}

impl ConfigWatcher {
    /// Create a new ConfigWatcher
    ///
    /// Returns a tuple of (watcher, receiver) where receiver will emit ConfigReloadEvent
    /// when the config file changes.
    pub fn new() -> (Self, Receiver<ConfigReloadEvent>) {
        let (tx, rx) = channel();
        let watcher = ConfigWatcher {
            tx: Some(tx),
            watcher_thread: None,
        };
        (watcher, rx)
    }

    /// Start watching the config file for changes
    ///
    /// This spawns a background thread that watches ~/.kenv/config.ts and sends
    /// reload events through the receiver when changes are detected.
    pub fn start(&mut self) -> NotifyResult<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| std::io::Error::other("watcher already started"))?;

        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::watch_loop(tx) {
                warn!(error = %e, watcher = "config", "Config watcher error");
            }
        });

        self.watcher_thread = Some(thread_handle);
        Ok(())
    }

    /// Internal watch loop running in background thread
    fn watch_loop(tx: Sender<ConfigReloadEvent>) -> NotifyResult<()> {
        // Expand the config path
        let config_path = PathBuf::from(shellexpand::tilde("~/.kenv/config.ts").as_ref());

        // Get the parent directory to watch
        let watch_path = config_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        // Create a debounce timer using Arc<Mutex>
        let debounce_active = Arc::new(Mutex::new(false));
        let debounce_active_clone = debounce_active.clone();

        // Channel for the file watcher thread
        let (watch_tx, watch_rx) = channel();

        // Create the watcher with a callback
        let mut watcher: Box<dyn Watcher> = Box::new(recommended_watcher(
            move |res: notify::Result<notify::Event>| {
                let _ = watch_tx.send(res);
            },
        )?);

        // Watch the directory containing config.ts
        watcher.watch(watch_path, RecursiveMode::NonRecursive)?;

        info!(
            path = %watch_path.display(),
            target = "config.ts",
            "Config watcher started"
        );

        // Main watch loop
        loop {
            match watch_rx.recv() {
                Ok(Ok(event)) => {
                    // Check if this is an event for config.ts
                    let is_config_change = event.paths.iter().any(|path: &PathBuf| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .map(|name| name == "config.ts")
                            .unwrap_or(false)
                    });

                    // Only care about Create and Modify events
                    let is_relevant_event = matches!(
                        event.kind,
                        notify::EventKind::Create(_) | notify::EventKind::Modify(_)
                    );

                    if is_config_change && is_relevant_event {
                        // Check if debounce is already active
                        let mut debounce = debounce_active_clone.lock().unwrap();
                        if !*debounce {
                            *debounce = true;
                            drop(debounce); // Release lock before spawning thread

                            let tx_clone = tx.clone();
                            let debounce_flag = debounce_active_clone.clone();

                            // Spawn debounce thread
                            thread::spawn(move || {
                                thread::sleep(Duration::from_millis(500));
                                let _ = tx_clone.send(ConfigReloadEvent::Reload);
                                let mut flag = debounce_flag.lock().unwrap();
                                *flag = false;
                                info!(
                                    file = "config.ts",
                                    "Config file changed, emitting reload event"
                                );
                            });
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, watcher = "config", "File watcher error");
                }
                Err(_) => {
                    // Channel closed, exit watch loop
                    info!(watcher = "config", "Config watcher shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        // Wait for watcher thread to finish (with timeout)
        if let Some(handle) = self.watcher_thread.take() {
            let _ = handle.join();
        }
    }
}

/// Watches ~/.kenv/theme.json for changes and emits reload events
pub struct ThemeWatcher {
    tx: Option<Sender<ThemeReloadEvent>>,
    watcher_thread: Option<thread::JoinHandle<()>>,
}

impl ThemeWatcher {
    /// Create a new ThemeWatcher
    ///
    /// Returns a tuple of (watcher, receiver) where receiver will emit ThemeReloadEvent
    /// when the theme file changes.
    pub fn new() -> (Self, Receiver<ThemeReloadEvent>) {
        let (tx, rx) = channel();
        let watcher = ThemeWatcher {
            tx: Some(tx),
            watcher_thread: None,
        };
        (watcher, rx)
    }

    /// Start watching the theme file for changes
    ///
    /// This spawns a background thread that watches ~/.kenv/theme.json and sends
    /// reload events through the receiver when changes are detected.
    pub fn start(&mut self) -> NotifyResult<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| std::io::Error::other("watcher already started"))?;

        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::watch_loop(tx) {
                warn!(error = %e, watcher = "theme", "Theme watcher error");
            }
        });

        self.watcher_thread = Some(thread_handle);
        Ok(())
    }

    /// Internal watch loop running in background thread
    fn watch_loop(tx: Sender<ThemeReloadEvent>) -> NotifyResult<()> {
        // Expand the theme path
        let theme_path = PathBuf::from(shellexpand::tilde("~/.kenv/theme.json").as_ref());

        // Get the parent directory to watch
        let watch_path = theme_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        // Create a debounce timer using Arc<Mutex>
        let debounce_active = Arc::new(Mutex::new(false));
        let debounce_active_clone = debounce_active.clone();

        // Channel for the file watcher thread
        let (watch_tx, watch_rx) = channel();

        // Create the watcher with a callback
        let mut watcher: Box<dyn Watcher> = Box::new(recommended_watcher(
            move |res: notify::Result<notify::Event>| {
                let _ = watch_tx.send(res);
            },
        )?);

        // Watch the directory containing theme.json
        watcher.watch(watch_path, RecursiveMode::NonRecursive)?;

        info!(
            path = %watch_path.display(),
            target = "theme.json",
            "Theme watcher started"
        );

        // Main watch loop
        loop {
            match watch_rx.recv() {
                Ok(Ok(event)) => {
                    // Check if this is an event for theme.json
                    let is_theme_change = event.paths.iter().any(|path: &PathBuf| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .map(|name| name == "theme.json")
                            .unwrap_or(false)
                    });

                    // Only care about Create and Modify events
                    let is_relevant_event = matches!(
                        event.kind,
                        notify::EventKind::Create(_) | notify::EventKind::Modify(_)
                    );

                    if is_theme_change && is_relevant_event {
                        // Check if debounce is already active
                        let mut debounce = debounce_active_clone.lock().unwrap();
                        if !*debounce {
                            *debounce = true;
                            drop(debounce); // Release lock before spawning thread

                            let tx_clone = tx.clone();
                            let debounce_flag = debounce_active_clone.clone();

                            // Spawn debounce thread
                            thread::spawn(move || {
                                thread::sleep(Duration::from_millis(500));
                                let _ = tx_clone.send(ThemeReloadEvent::Reload);
                                let mut flag = debounce_flag.lock().unwrap();
                                *flag = false;
                                info!(
                                    file = "theme.json",
                                    "Theme file changed, emitting reload event"
                                );
                            });
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, watcher = "theme", "File watcher error");
                }
                Err(_) => {
                    // Channel closed, exit watch loop
                    info!(watcher = "theme", "Theme watcher shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

impl Drop for ThemeWatcher {
    fn drop(&mut self) {
        // Wait for watcher thread to finish (with timeout)
        if let Some(handle) = self.watcher_thread.take() {
            let _ = handle.join();
        }
    }
}

/// Check if a file path is a relevant script file (ts, js, or md)
fn is_relevant_script_file(path: &std::path::Path) -> bool {
    // Skip hidden files
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        if file_name.starts_with('.') {
            return false;
        }
    }

    // Check for relevant extensions
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ts") | Some("js") | Some("md")
    )
}

/// Watches ~/.kenv/scripts and ~/.kenv/scriptlets directories for changes and emits reload events
pub struct ScriptWatcher {
    tx: Option<Sender<ScriptReloadEvent>>,
    watcher_thread: Option<thread::JoinHandle<()>>,
}

impl ScriptWatcher {
    /// Create a new ScriptWatcher
    ///
    /// Returns a tuple of (watcher, receiver) where receiver will emit ScriptReloadEvent
    /// when files in the scripts directory change.
    pub fn new() -> (Self, Receiver<ScriptReloadEvent>) {
        let (tx, rx) = channel();
        let watcher = ScriptWatcher {
            tx: Some(tx),
            watcher_thread: None,
        };
        (watcher, rx)
    }

    /// Start watching the scripts directory for changes
    ///
    /// This spawns a background thread that watches ~/.kenv/scripts recursively and sends
    /// reload events through the receiver when scripts are added, modified, or deleted.
    pub fn start(&mut self) -> NotifyResult<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| std::io::Error::other("watcher already started"))?;

        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::watch_loop(tx) {
                warn!(error = %e, watcher = "scripts", "Script watcher error");
            }
        });

        self.watcher_thread = Some(thread_handle);
        Ok(())
    }

    /// Internal watch loop running in background thread
    fn watch_loop(tx: Sender<ScriptReloadEvent>) -> NotifyResult<()> {
        // Expand the scripts and scriptlets paths
        let scripts_path = PathBuf::from(shellexpand::tilde("~/.kenv/scripts").as_ref());
        let scriptlets_path = PathBuf::from(shellexpand::tilde("~/.kenv/scriptlets").as_ref());

        // Track pending events for debouncing (path -> (event_type, timestamp))
        let pending_events: Arc<
            Mutex<std::collections::HashMap<PathBuf, (ScriptReloadEvent, std::time::Instant)>>,
        > = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let pending_events_clone = pending_events.clone();

        // Debounce interval
        let debounce_ms = 500;

        // Channel for the file watcher thread
        let (watch_tx, watch_rx) = channel();

        // Create the watcher with a callback
        let mut watcher: Box<dyn Watcher> = Box::new(recommended_watcher(
            move |res: notify::Result<notify::Event>| {
                let _ = watch_tx.send(res);
            },
        )?);

        // Watch the scripts directory recursively
        watcher.watch(&scripts_path, RecursiveMode::Recursive)?;

        // Watch the scriptlets directory recursively (for *.md files)
        if scriptlets_path.exists() {
            watcher.watch(&scriptlets_path, RecursiveMode::Recursive)?;
            info!(
                path = %scriptlets_path.display(),
                recursive = true,
                "Scriptlets watcher started"
            );
        }

        info!(
            path = %scripts_path.display(),
            recursive = true,
            "Script watcher started"
        );

        // Spawn a background thread to flush pending events after debounce interval
        let tx_clone = tx.clone();
        let flush_pending = pending_events_clone.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(100)); // Check every 100ms

                let now = std::time::Instant::now();
                let mut events_to_send = Vec::new();

                {
                    let mut pending = flush_pending.lock().unwrap();
                    let debounce_threshold = Duration::from_millis(debounce_ms);

                    // Find events that have been pending long enough
                    let expired: Vec<PathBuf> = pending
                        .iter()
                        .filter(|(_, (_, timestamp))| {
                            now.duration_since(*timestamp) >= debounce_threshold
                        })
                        .map(|(path, _)| path.clone())
                        .collect();

                    // Remove expired events and collect them for sending
                    for path in expired {
                        if let Some((event, _)) = pending.remove(&path) {
                            events_to_send.push((path, event));
                        }
                    }
                }

                // Send events outside the lock
                for (path, event) in events_to_send {
                    info!(
                        path = %path.display(),
                        event_type = ?event,
                        "Emitting script reload event"
                    );
                    if tx_clone.send(event).is_err() {
                        // Channel closed, exit flush thread
                        return;
                    }
                }
            }
        });

        // Main watch loop
        loop {
            match watch_rx.recv() {
                Ok(Ok(event)) => {
                    // Process each path in the event
                    for path in event.paths.iter() {
                        // Skip non-relevant files
                        if !is_relevant_script_file(path) {
                            continue;
                        }

                        // Determine the event type based on notify::EventKind
                        let reload_event = match event.kind {
                            notify::EventKind::Create(_) => {
                                ScriptReloadEvent::FileCreated(path.clone())
                            }
                            notify::EventKind::Modify(_) => {
                                ScriptReloadEvent::FileChanged(path.clone())
                            }
                            notify::EventKind::Remove(_) => {
                                ScriptReloadEvent::FileDeleted(path.clone())
                            }
                            // For other events (Access, Other), use FullReload as fallback
                            _ => continue,
                        };

                        // Update pending events map (this implements per-file debouncing)
                        let mut pending = pending_events.lock().unwrap();
                        pending.insert(path.clone(), (reload_event, std::time::Instant::now()));
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, watcher = "scripts", "File watcher error");
                }
                Err(_) => {
                    // Channel closed, exit watch loop
                    info!(watcher = "scripts", "Script watcher shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

impl Drop for ScriptWatcher {
    fn drop(&mut self) {
        // Wait for watcher thread to finish (with timeout)
        if let Some(handle) = self.watcher_thread.take() {
            let _ = handle.join();
        }
    }
}

/// Watches system appearance (light/dark mode) for changes and emits events
///
/// This watcher polls the system appearance setting every 2 seconds by running
/// the `defaults read -g AppleInterfaceStyle` command on macOS.
pub struct AppearanceWatcher {
    tx: Option<async_channel::Sender<AppearanceChangeEvent>>,
    watcher_thread: Option<thread::JoinHandle<()>>,
}

impl AppearanceWatcher {
    /// Create a new AppearanceWatcher
    ///
    /// Returns a tuple of (watcher, receiver) where receiver will emit AppearanceChangeEvent
    /// when the system appearance changes.
    pub fn new() -> (Self, async_channel::Receiver<AppearanceChangeEvent>) {
        let (tx, rx) = async_channel::bounded(100);
        let watcher = AppearanceWatcher {
            tx: Some(tx),
            watcher_thread: None,
        };
        (watcher, rx)
    }

    /// Start watching the system appearance for changes
    ///
    /// This spawns a background thread that polls the system appearance every 2 seconds
    /// and sends appearance change events through the receiver when changes are detected.
    pub fn start(&mut self) -> Result<(), String> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| "watcher already started".to_string())?;

        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::watch_loop(tx) {
                warn!(error = %e, watcher = "appearance", "Appearance watcher error");
            }
        });

        self.watcher_thread = Some(thread_handle);
        Ok(())
    }

    /// Internal watch loop running in background thread
    fn watch_loop(tx: async_channel::Sender<AppearanceChangeEvent>) -> Result<(), String> {
        let mut last_appearance: Option<AppearanceChangeEvent> = None;
        let poll_interval = Duration::from_secs(2);

        info!(poll_interval_secs = 2, "Appearance watcher started");

        loop {
            // Detect current system appearance
            let current_appearance = Self::detect_appearance();

            // Send event if appearance changed
            if last_appearance != Some(current_appearance.clone()) {
                let mode = match current_appearance {
                    AppearanceChangeEvent::Dark => "dark",
                    AppearanceChangeEvent::Light => "light",
                };
                info!(mode = mode, "System appearance changed");
                if tx.send_blocking(current_appearance.clone()).is_err() {
                    info!(
                        watcher = "appearance",
                        "Appearance watcher receiver dropped, shutting down"
                    );
                    break;
                }
                last_appearance = Some(current_appearance);
            }

            // Poll every 2 seconds
            thread::sleep(poll_interval);
        }

        Ok(())
    }

    /// Detect the current system appearance
    fn detect_appearance() -> AppearanceChangeEvent {
        match Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.to_lowercase().contains("dark") {
                    AppearanceChangeEvent::Dark
                } else {
                    AppearanceChangeEvent::Light
                }
            }
            Err(_) => {
                // Command failed, likely in light mode on macOS
                AppearanceChangeEvent::Light
            }
        }
    }
}

impl Drop for AppearanceWatcher {
    fn drop(&mut self) {
        // Wait for watcher thread to finish (with timeout)
        if let Some(handle) = self.watcher_thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_watcher_creation() {
        let (_watcher, _rx) = ConfigWatcher::new();
        // Watcher should be created without panicking
    }

    #[test]
    fn test_config_reload_event_clone() {
        let event = ConfigReloadEvent::Reload;
        let _cloned = event.clone();
        // Event should be cloneable
    }

    #[test]
    fn test_theme_watcher_creation() {
        let (_watcher, _rx) = ThemeWatcher::new();
        // Watcher should be created without panicking
    }

    #[test]
    fn test_theme_reload_event_clone() {
        let event = ThemeReloadEvent::Reload;
        let _cloned = event.clone();
        // Event should be cloneable
    }

    #[test]
    fn test_script_watcher_creation() {
        let (_watcher, _rx) = ScriptWatcher::new();
        // Watcher should be created without panicking
    }

    #[test]
    fn test_script_reload_event_clone() {
        let event = ScriptReloadEvent::FullReload;
        let _cloned = event.clone();
        // Event should be cloneable
    }

    #[test]
    fn test_script_reload_event_file_changed() {
        let path = PathBuf::from("/test/path/script.ts");
        let event = ScriptReloadEvent::FileChanged(path.clone());

        // Verify the event contains the correct path
        if let ScriptReloadEvent::FileChanged(event_path) = event {
            assert_eq!(event_path, path);
        } else {
            panic!("Expected FileChanged variant");
        }
    }

    #[test]
    fn test_script_reload_event_file_created() {
        let path = PathBuf::from("/test/path/new-script.ts");
        let event = ScriptReloadEvent::FileCreated(path.clone());

        // Verify the event contains the correct path
        if let ScriptReloadEvent::FileCreated(event_path) = event {
            assert_eq!(event_path, path);
        } else {
            panic!("Expected FileCreated variant");
        }
    }

    #[test]
    fn test_script_reload_event_file_deleted() {
        let path = PathBuf::from("/test/path/deleted-script.ts");
        let event = ScriptReloadEvent::FileDeleted(path.clone());

        // Verify the event contains the correct path
        if let ScriptReloadEvent::FileDeleted(event_path) = event {
            assert_eq!(event_path, path);
        } else {
            panic!("Expected FileDeleted variant");
        }
    }

    #[test]
    fn test_script_reload_event_equality() {
        let path1 = PathBuf::from("/test/path/script.ts");
        let path2 = PathBuf::from("/test/path/script.ts");
        let path3 = PathBuf::from("/test/path/other.ts");

        // Same path should be equal
        assert_eq!(
            ScriptReloadEvent::FileChanged(path1.clone()),
            ScriptReloadEvent::FileChanged(path2.clone())
        );

        // Different paths should not be equal
        assert_ne!(
            ScriptReloadEvent::FileChanged(path1.clone()),
            ScriptReloadEvent::FileChanged(path3.clone())
        );

        // Different event types should not be equal
        assert_ne!(
            ScriptReloadEvent::FileChanged(path1.clone()),
            ScriptReloadEvent::FileCreated(path1.clone())
        );

        // FullReload should equal itself
        assert_eq!(ScriptReloadEvent::FullReload, ScriptReloadEvent::FullReload);
    }

    #[test]
    fn test_extract_file_path_from_event() {
        // Test helper function for extracting paths from notify events
        use notify::event::{CreateKind, ModifyKind, RemoveKind};

        let test_path = PathBuf::from("/Users/test/.kenv/scripts/hello.ts");

        // Test Create event
        let create_event = notify::Event {
            kind: notify::EventKind::Create(CreateKind::File),
            paths: vec![test_path.clone()],
            attrs: Default::default(),
        };
        assert_eq!(create_event.paths.first(), Some(&test_path));

        // Test Modify event
        let modify_event = notify::Event {
            kind: notify::EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![test_path.clone()],
            attrs: Default::default(),
        };
        assert_eq!(modify_event.paths.first(), Some(&test_path));

        // Test Remove event
        let remove_event = notify::Event {
            kind: notify::EventKind::Remove(RemoveKind::File),
            paths: vec![test_path.clone()],
            attrs: Default::default(),
        };
        assert_eq!(remove_event.paths.first(), Some(&test_path));
    }

    #[test]
    fn test_is_relevant_script_file() {
        use std::path::Path;

        // Test that we correctly identify relevant script files
        let ts_path = Path::new("/Users/test/.kenv/scripts/hello.ts");
        let js_path = Path::new("/Users/test/.kenv/scripts/hello.js");
        let md_path = Path::new("/Users/test/.kenv/scriptlets/hello.md");
        let txt_path = Path::new("/Users/test/.kenv/scripts/readme.txt");
        let hidden_path = Path::new("/Users/test/.kenv/scripts/.hidden.ts");

        // TypeScript files should be relevant
        assert!(is_relevant_script_file(ts_path));

        // JavaScript files should be relevant
        assert!(is_relevant_script_file(js_path));

        // Markdown files in scriptlets should be relevant
        assert!(is_relevant_script_file(md_path));

        // Other file types should not be relevant
        assert!(!is_relevant_script_file(txt_path));

        // Hidden files should not be relevant
        assert!(!is_relevant_script_file(hidden_path));
    }

    #[test]
    fn test_appearance_change_event_clone() {
        let event_dark = AppearanceChangeEvent::Dark;
        let _cloned = event_dark.clone();
        let event_light = AppearanceChangeEvent::Light;
        let _cloned = event_light.clone();
        // Events should be cloneable
    }

    #[test]
    fn test_appearance_change_event_equality() {
        let dark1 = AppearanceChangeEvent::Dark;
        let dark2 = AppearanceChangeEvent::Dark;
        let light = AppearanceChangeEvent::Light;

        assert_eq!(dark1, dark2);
        assert_ne!(dark1, light);
    }

    #[test]
    fn test_appearance_watcher_creation() {
        let (_watcher, _rx) = AppearanceWatcher::new();
        // Watcher should be created without panicking
    }
}
