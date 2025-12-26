use notify::{Watcher, RecursiveMode, Result as NotifyResult, recommended_watcher};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use shellexpand;
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
#[derive(Debug, Clone)]
pub enum ScriptReloadEvent {
    Reload,
}

/// Event emitted when system appearance changes (light/dark mode)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppearanceChangeEvent {
    /// Dark mode is now active
    Dark,
    /// Light mode is now active
    Light,
}

/// Watches ~/.kit/config.ts for changes and emits reload events
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
    /// This spawns a background thread that watches ~/.kit/config.ts and sends
    /// reload events through the receiver when changes are detected.
    pub fn start(&mut self) -> NotifyResult<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "watcher already started")
            })?;

        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::watch_loop(tx) {
                eprintln!("Config watcher error: {}", e);
            }
        });

        self.watcher_thread = Some(thread_handle);
        Ok(())
    }

    /// Internal watch loop running in background thread
    fn watch_loop(tx: Sender<ConfigReloadEvent>) -> NotifyResult<()> {
        // Expand the config path
        let config_path = PathBuf::from(
            shellexpand::tilde("~/.kit/config.ts").as_ref()
        );

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
        eprintln!(
            "Config watcher started, watching {:?} for changes to config.ts",
            watch_path
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
                                info!(file = "config.ts", "Config file changed, emitting reload event");
                                eprintln!("Config file changed, emitting reload event");
                            });
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, watcher = "config", "File watcher error");
                    eprintln!("File watcher error: {}", e);
                }
                Err(_) => {
                    // Channel closed, exit watch loop
                    info!(watcher = "config", "Config watcher shutting down");
                    eprintln!("Config watcher shutting down");
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

/// Watches ~/.kit/theme.json for changes and emits reload events
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
    /// This spawns a background thread that watches ~/.kit/theme.json and sends
    /// reload events through the receiver when changes are detected.
    pub fn start(&mut self) -> NotifyResult<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "watcher already started")
            })?;

        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::watch_loop(tx) {
                eprintln!("Theme watcher error: {}", e);
            }
        });

        self.watcher_thread = Some(thread_handle);
        Ok(())
    }

    /// Internal watch loop running in background thread
    fn watch_loop(tx: Sender<ThemeReloadEvent>) -> NotifyResult<()> {
        // Expand the theme path
        let theme_path = PathBuf::from(
            shellexpand::tilde("~/.kit/theme.json").as_ref()
        );

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
        eprintln!(
            "Theme watcher started, watching {:?} for changes to theme.json",
            watch_path
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
                                info!(file = "theme.json", "Theme file changed, emitting reload event");
                                eprintln!("Theme file changed, emitting reload event");
                            });
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, watcher = "theme", "File watcher error");
                    eprintln!("File watcher error: {}", e);
                }
                Err(_) => {
                    // Channel closed, exit watch loop
                    info!(watcher = "theme", "Theme watcher shutting down");
                    eprintln!("Theme watcher shutting down");
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

/// Watches ~/.kenv/scripts directory for changes and emits reload events
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
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "watcher already started")
            })?;

        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::watch_loop(tx) {
                eprintln!("Script watcher error: {}", e);
            }
        });

        self.watcher_thread = Some(thread_handle);
        Ok(())
    }

    /// Internal watch loop running in background thread
    fn watch_loop(tx: Sender<ScriptReloadEvent>) -> NotifyResult<()> {
        // Expand the scripts path
        let scripts_path = PathBuf::from(
            shellexpand::tilde("~/.kenv/scripts").as_ref()
        );

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

        // Watch the scripts directory recursively
        watcher.watch(&scripts_path, RecursiveMode::Recursive)?;

        info!(
            path = %scripts_path.display(),
            recursive = true,
            "Script watcher started"
        );
        eprintln!(
            "Script watcher started, watching {:?} recursively for changes",
            scripts_path
        );

        // Main watch loop
        loop {
            match watch_rx.recv() {
                Ok(Ok(event)) => {
                    // Care about Create, Modify, and Remove events in scripts directory
                    let is_relevant_event = matches!(
                        event.kind,
                        notify::EventKind::Create(_)
                            | notify::EventKind::Modify(_)
                            | notify::EventKind::Remove(_)
                    );

                    if is_relevant_event {
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
                                let _ = tx_clone.send(ScriptReloadEvent::Reload);
                                let mut flag = debounce_flag.lock().unwrap();
                                *flag = false;
                                info!(directory = "scripts", "Script directory changed, emitting reload event");
                                eprintln!("Script directory changed, emitting reload event");
                            });
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, watcher = "scripts", "File watcher error");
                    eprintln!("File watcher error: {}", e);
                }
                Err(_) => {
                    // Channel closed, exit watch loop
                    info!(watcher = "scripts", "Script watcher shutting down");
                    eprintln!("Script watcher shutting down");
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
    tx: Option<Sender<AppearanceChangeEvent>>,
    watcher_thread: Option<thread::JoinHandle<()>>,
}

impl AppearanceWatcher {
    /// Create a new AppearanceWatcher
    ///
    /// Returns a tuple of (watcher, receiver) where receiver will emit AppearanceChangeEvent
    /// when the system appearance changes.
    pub fn new() -> (Self, Receiver<AppearanceChangeEvent>) {
        let (tx, rx) = channel();
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
        let tx = self.tx.take().ok_or_else(|| "watcher already started".to_string())?;

        let thread_handle = thread::spawn(move || {
            if let Err(e) = Self::watch_loop(tx) {
                eprintln!("Appearance watcher error: {}", e);
            }
        });

        self.watcher_thread = Some(thread_handle);
        Ok(())
    }

    /// Internal watch loop running in background thread
    fn watch_loop(tx: Sender<AppearanceChangeEvent>) -> Result<(), String> {
        let mut last_appearance: Option<AppearanceChangeEvent> = None;
        let poll_interval = Duration::from_secs(2);

        info!(poll_interval_secs = 2, "Appearance watcher started");
        eprintln!("Appearance watcher started, polling system appearance every 2 seconds");

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
                eprintln!(
                    "System appearance changed to: {}",
                    mode
                );
                if let Err(_) = tx.send(current_appearance.clone()) {
                    info!(watcher = "appearance", "Appearance watcher receiver dropped, shutting down");
                    eprintln!("Appearance watcher receiver dropped, shutting down");
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
            .args(&["read", "-g", "AppleInterfaceStyle"])
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
    use std::time::Duration;

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
        let event = ScriptReloadEvent::Reload;
        let _cloned = event.clone();
        // Event should be cloneable
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
