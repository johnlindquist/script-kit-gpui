use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use crate::{config, logging, scripts, shortcuts};

// HOTKEY_CHANNEL: Event-driven async_channel for hotkey events (replaces AtomicBool polling)
#[allow(dead_code)]
static HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

/// Get the hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn hotkey_channel() -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>)
{
    HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

// SCRIPT_HOTKEY_CHANNEL: Channel for script shortcut events (sends script path)
#[allow(dead_code)]
static SCRIPT_HOTKEY_CHANNEL: OnceLock<(
    async_channel::Sender<String>,
    async_channel::Receiver<String>,
)> = OnceLock::new();

/// Get the script hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn script_hotkey_channel() -> &'static (
    async_channel::Sender<String>,
    async_channel::Receiver<String>,
) {
    SCRIPT_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

// NOTES_HOTKEY_CHANNEL: Channel for notes hotkey events
#[allow(dead_code)]
static NOTES_HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

/// Get the notes hotkey channel, initializing it on first access.
#[allow(dead_code)]
pub(crate) fn notes_hotkey_channel(
) -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    NOTES_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

#[allow(dead_code)]
static HOTKEY_TRIGGER_COUNT: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
pub(crate) fn start_hotkey_listener(config: config::Config) {
    std::thread::spawn(move || {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                logging::log("HOTKEY", &format!("Failed to create hotkey manager: {}", e));
                return;
            }
        };

        // Convert config hotkey to global_hotkey::Code
        let code = match config.hotkey.key.as_str() {
            "Semicolon" => Code::Semicolon,
            "KeyK" => Code::KeyK,
            "KeyP" => Code::KeyP,
            "Space" => Code::Space,
            "Enter" => Code::Enter,
            "Digit0" => Code::Digit0,
            "Digit1" => Code::Digit1,
            "Digit2" => Code::Digit2,
            "Digit3" => Code::Digit3,
            "Digit4" => Code::Digit4,
            "Digit5" => Code::Digit5,
            "Digit6" => Code::Digit6,
            "Digit7" => Code::Digit7,
            "Digit8" => Code::Digit8,
            "Digit9" => Code::Digit9,
            "KeyA" => Code::KeyA,
            "KeyB" => Code::KeyB,
            "KeyC" => Code::KeyC,
            "KeyD" => Code::KeyD,
            "KeyE" => Code::KeyE,
            "KeyF" => Code::KeyF,
            "KeyG" => Code::KeyG,
            "KeyH" => Code::KeyH,
            "KeyI" => Code::KeyI,
            "KeyJ" => Code::KeyJ,
            "KeyL" => Code::KeyL,
            "KeyM" => Code::KeyM,
            "KeyN" => Code::KeyN,
            "KeyO" => Code::KeyO,
            "KeyQ" => Code::KeyQ,
            "KeyR" => Code::KeyR,
            "KeyS" => Code::KeyS,
            "KeyT" => Code::KeyT,
            "KeyU" => Code::KeyU,
            "KeyV" => Code::KeyV,
            "KeyW" => Code::KeyW,
            "KeyX" => Code::KeyX,
            "KeyY" => Code::KeyY,
            "KeyZ" => Code::KeyZ,
            // Function keys
            "F1" => Code::F1,
            "F2" => Code::F2,
            "F3" => Code::F3,
            "F4" => Code::F4,
            "F5" => Code::F5,
            "F6" => Code::F6,
            "F7" => Code::F7,
            "F8" => Code::F8,
            "F9" => Code::F9,
            "F10" => Code::F10,
            "F11" => Code::F11,
            "F12" => Code::F12,
            other => {
                logging::log(
                    "HOTKEY",
                    &format!(
                        "Unknown key code: '{}'. Valid keys: KeyA-KeyZ, Digit0-Digit9, F1-F12, Space, Enter, Semicolon. Falling back to Semicolon",
                        other
                    ),
                );
                Code::Semicolon
            }
        };

        // Convert modifiers from config strings to Modifiers flags
        let mut modifiers = Modifiers::empty();
        for modifier in &config.hotkey.modifiers {
            match modifier.as_str() {
                "meta" => modifiers |= Modifiers::META,
                "ctrl" => modifiers |= Modifiers::CONTROL,
                "alt" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                other => {
                    logging::log("HOTKEY", &format!("Unknown modifier: {}", other));
                }
            }
        }

        let hotkey = HotKey::new(Some(modifiers), code);
        let main_hotkey_id = hotkey.id();

        let hotkey_display = format!(
            "{}{}",
            config.hotkey.modifiers.join("+"),
            if config.hotkey.modifiers.is_empty() {
                String::new()
            } else {
                "+".to_string()
            }
        ) + &config.hotkey.key;

        if let Err(e) = manager.register(hotkey) {
            logging::log(
                "HOTKEY",
                &format!("Failed to register {}: {}", hotkey_display, e),
            );
            return;
        }

        logging::log(
            "HOTKEY",
            &format!(
                "Registered global hotkey {} (id: {})",
                hotkey_display, main_hotkey_id
            ),
        );

        // Register notes hotkey (Cmd+Shift+N by default)
        let notes_config = config.get_notes_hotkey();
        let notes_code = match notes_config.key.as_str() {
            "KeyN" => Code::KeyN,
            "KeyM" => Code::KeyM,
            "KeyO" => Code::KeyO,
            "KeyP" => Code::KeyP,
            _ => Code::KeyN, // Default to N
        };

        let mut notes_modifiers = Modifiers::empty();
        for modifier in &notes_config.modifiers {
            match modifier.as_str() {
                "meta" => notes_modifiers |= Modifiers::META,
                "ctrl" => notes_modifiers |= Modifiers::CONTROL,
                "alt" => notes_modifiers |= Modifiers::ALT,
                "shift" => notes_modifiers |= Modifiers::SHIFT,
                _ => {}
            }
        }

        let notes_hotkey = HotKey::new(Some(notes_modifiers), notes_code);
        let notes_hotkey_id = notes_hotkey.id();

        let notes_display = format!(
            "{}{}",
            notes_config.modifiers.join("+"),
            if notes_config.modifiers.is_empty() {
                String::new()
            } else {
                "+".to_string()
            }
        ) + &notes_config.key;

        if let Err(e) = manager.register(notes_hotkey) {
            logging::log(
                "HOTKEY",
                &format!("Failed to register notes hotkey {}: {}", notes_display, e),
            );
        } else {
            logging::log(
                "HOTKEY",
                &format!(
                    "Registered notes hotkey {} (id: {})",
                    notes_display, notes_hotkey_id
                ),
            );
        }

        // Register script shortcuts
        // Map from hotkey ID to script path
        let mut script_hotkey_map: std::collections::HashMap<u32, String> =
            std::collections::HashMap::new();

        // Load scripts with shortcuts
        let all_scripts = scripts::read_scripts();
        for script in &all_scripts {
            if let Some(ref shortcut) = script.shortcut {
                if let Some((mods, key_code)) = shortcuts::parse_shortcut(shortcut) {
                    let script_hotkey = HotKey::new(Some(mods), key_code);
                    let script_hotkey_id = script_hotkey.id();

                    match manager.register(script_hotkey) {
                        Ok(()) => {
                            script_hotkey_map.insert(
                                script_hotkey_id,
                                script.path.to_string_lossy().to_string(),
                            );
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Registered script shortcut '{}' for {} (id: {})",
                                    shortcut, script.name, script_hotkey_id
                                ),
                            );
                        }
                        Err(e) => {
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Failed to register shortcut '{}' for {}: {}",
                                    shortcut, script.name, e
                                ),
                            );
                        }
                    }
                } else {
                    logging::log(
                        "HOTKEY",
                        &format!(
                            "Failed to parse shortcut '{}' for script {}",
                            shortcut, script.name
                        ),
                    );
                }
            }
        }

        // Load scriptlets with shortcuts
        let all_scriptlets = scripts::load_scriptlets();
        for scriptlet in &all_scriptlets {
            if let Some(ref shortcut) = scriptlet.shortcut {
                if let Some((mods, key_code)) = shortcuts::parse_shortcut(shortcut) {
                    let scriptlet_hotkey = HotKey::new(Some(mods), key_code);
                    let scriptlet_hotkey_id = scriptlet_hotkey.id();

                    // Use file_path as the identifier (already includes #command)
                    let scriptlet_path = scriptlet
                        .file_path
                        .clone()
                        .unwrap_or_else(|| scriptlet.name.clone());

                    match manager.register(scriptlet_hotkey) {
                        Ok(()) => {
                            script_hotkey_map.insert(scriptlet_hotkey_id, scriptlet_path.clone());
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Registered scriptlet shortcut '{}' for {} (id: {})",
                                    shortcut, scriptlet.name, scriptlet_hotkey_id
                                ),
                            );
                        }
                        Err(e) => {
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Failed to register shortcut '{}' for {}: {}",
                                    shortcut, scriptlet.name, e
                                ),
                            );
                        }
                    }
                }
            }
        }

        logging::log(
            "HOTKEY",
            &format!(
                "Registered {} script/scriptlet shortcuts",
                script_hotkey_map.len()
            ),
        );

        let receiver = GlobalHotKeyEvent::receiver();

        loop {
            if let Ok(event) = receiver.recv() {
                // Only respond to key PRESS, not release
                if event.state != HotKeyState::Pressed {
                    continue;
                }

                // Check if it's the main app hotkey
                if event.id == main_hotkey_id {
                    let count = HOTKEY_TRIGGER_COUNT.fetch_add(1, Ordering::SeqCst);
                    // Send via async_channel for immediate event-driven handling
                    if hotkey_channel().0.send_blocking(()).is_err() {
                        logging::log("HOTKEY", "Hotkey channel closed, cannot send");
                    }
                    logging::log(
                        "HOTKEY",
                        &format!("{} pressed (trigger #{})", hotkey_display, count + 1),
                    );
                }
                // Check if it's the notes hotkey
                else if event.id == notes_hotkey_id {
                    logging::log("HOTKEY", &format!("{} pressed (notes)", notes_display));
                    if notes_hotkey_channel().0.send_blocking(()).is_err() {
                        logging::log("HOTKEY", "Notes hotkey channel closed, cannot send");
                    }
                }
                // Check if it's a script shortcut
                else if let Some(script_path) = script_hotkey_map.get(&event.id) {
                    logging::log(
                        "HOTKEY",
                        &format!("Script shortcut triggered: {}", script_path),
                    );
                    // Send the script path to be executed
                    if script_hotkey_channel()
                        .0
                        .send_blocking(script_path.clone())
                        .is_err()
                    {
                        logging::log("HOTKEY", "Script hotkey channel closed, cannot send");
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_channel::TryRecvError;

    #[test]
    fn hotkey_channels_are_independent() {
        while hotkey_channel().1.try_recv().is_ok() {}
        while script_hotkey_channel().1.try_recv().is_ok() {}

        hotkey_channel().0.send_blocking(()).expect("send hotkey");
        assert!(matches!(
            script_hotkey_channel().1.try_recv(),
            Err(TryRecvError::Empty)
        ));
        assert!(hotkey_channel().1.try_recv().is_ok());

        script_hotkey_channel()
            .0
            .send_blocking("script".to_string())
            .expect("send script hotkey");
        assert_eq!(
            script_hotkey_channel()
                .1
                .try_recv()
                .expect("recv script hotkey"),
            "script"
        );
    }
}
