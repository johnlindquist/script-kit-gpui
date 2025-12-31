use global_hotkey::hotkey::{Code, Modifiers};

use crate::logging;

/// Parse a shortcut string into (Modifiers, Code).
///
/// Supports flexible formats:
/// - Space-separated: "opt i", "cmd shift k"
/// - Plus-separated: "cmd+shift+k", "ctrl+alt+delete"
/// - Mixed: "cmd + shift + k"
/// - Various modifier names: cmd/command/meta, ctrl/control/^, alt/opt/option, shift
///
/// Returns None if the shortcut string is invalid.
pub fn parse_shortcut(shortcut: &str) -> Option<(Modifiers, Code)> {
    // Normalize the shortcut: replace + with space, collapse whitespace
    let normalized = shortcut
        .replace('+', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let parts: Vec<&str> = normalized.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers = Modifiers::empty();
    let mut key_part: Option<&str> = None;

    for part in &parts {
        let part_lower = part.to_lowercase();
        match part_lower.as_str() {
            // Meta/Command key - many variations
            "cmd" | "command" | "meta" | "super" | "win" | "⌘" => modifiers |= Modifiers::META,
            // Control key
            "ctrl" | "control" | "ctl" | "^" => modifiers |= Modifiers::CONTROL,
            // Alt/Option key
            "alt" | "opt" | "option" | "⌥" => modifiers |= Modifiers::ALT,
            // Shift key
            "shift" | "shft" | "⇧" => modifiers |= Modifiers::SHIFT,
            // If not a modifier, it's the key
            _ => key_part = Some(part),
        }
    }

    let key = key_part?;
    let key_lower = key.to_lowercase();

    let code = match key_lower.as_str() {
        // Letters
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        // Numbers
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        // Function keys
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        // Special keys
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "escape" | "esc" => Code::Escape,
        "backspace" | "back" => Code::Backspace,
        "delete" | "del" => Code::Delete,
        ";" | "semicolon" => Code::Semicolon,
        "'" | "quote" | "apostrophe" => Code::Quote,
        "," | "comma" => Code::Comma,
        "." | "period" | "dot" => Code::Period,
        "/" | "slash" | "forwardslash" => Code::Slash,
        "\\" | "backslash" => Code::Backslash,
        "[" | "bracketleft" | "leftbracket" => Code::BracketLeft,
        "]" | "bracketright" | "rightbracket" => Code::BracketRight,
        "-" | "minus" | "dash" | "hyphen" => Code::Minus,
        "=" | "equal" | "equals" => Code::Equal,
        "`" | "backquote" | "backtick" | "grave" => Code::Backquote,
        // Arrow keys
        "up" | "arrowup" | "uparrow" => Code::ArrowUp,
        "down" | "arrowdown" | "downarrow" => Code::ArrowDown,
        "left" | "arrowleft" | "leftarrow" => Code::ArrowLeft,
        "right" | "arrowright" | "rightarrow" => Code::ArrowRight,
        // Home/End/PageUp/PageDown
        "home" => Code::Home,
        "end" => Code::End,
        "pageup" | "pgup" => Code::PageUp,
        "pagedown" | "pgdn" | "pgdown" => Code::PageDown,
        _ => {
            logging::log(
                "SHORTCUT",
                &format!("Unknown key in shortcut '{}': '{}'", shortcut, key),
            );
            return None;
        }
    };

    Some((modifiers, code))
}

/// Normalize a shortcut string for consistent comparison.
/// Converts "cmd+shift+c" and "Cmd+Shift+C" to "cmd+shift+c".
pub fn normalize_shortcut(shortcut: &str) -> String {
    let mut parts: Vec<&str> = shortcut.split('+').collect();

    // Separate modifiers from key
    let mut modifiers: Vec<&str> = Vec::new();
    let mut key: Option<&str> = None;

    for part in parts.drain(..) {
        let lower = part.trim().to_lowercase();
        match lower.as_str() {
            "cmd" | "command" | "meta" | "super" => modifiers.push("cmd"),
            "ctrl" | "control" => modifiers.push("ctrl"),
            "alt" | "option" | "opt" => modifiers.push("alt"),
            "shift" => modifiers.push("shift"),
            _ => key = Some(part.trim()),
        }
    }

    // Sort modifiers for consistent ordering
    modifiers.sort();

    // Rebuild with sorted modifiers + key
    let mut result = modifiers.join("+");
    if let Some(k) = key {
        if !result.is_empty() {
            result.push('+');
        }
        result.push_str(&k.to_lowercase());
    }

    result
}

/// Convert a GPUI keystroke to a normalized shortcut string.
pub fn keystroke_to_shortcut(key: &str, modifiers: &gpui::Modifiers) -> String {
    let mut parts: Vec<&str> = Vec::new();

    // Add modifiers in sorted order
    if modifiers.alt {
        parts.push("alt");
    }
    if modifiers.platform {
        parts.push("cmd");
    }
    if modifiers.control {
        parts.push("ctrl");
    }
    if modifiers.shift {
        parts.push("shift");
    }

    // Add the key
    let key_lower = key.to_lowercase();
    let mut result = parts.join("+");
    if !result.is_empty() {
        result.push('+');
    }
    result.push_str(&key_lower);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_shortcut_accepts_space_and_plus() {
        let (mods, code) = parse_shortcut("cmd shift k").expect("shortcut should parse");
        assert!(mods.contains(Modifiers::META));
        assert!(mods.contains(Modifiers::SHIFT));
        assert_eq!(code, Code::KeyK);

        let (mods, code) = parse_shortcut("ctrl+alt+delete").expect("shortcut should parse");
        assert!(mods.contains(Modifiers::CONTROL));
        assert!(mods.contains(Modifiers::ALT));
        assert_eq!(code, Code::Delete);
    }

    #[test]
    fn parse_shortcut_handles_arrows_and_invalid_keys() {
        let (mods, code) = parse_shortcut("shift down").expect("shortcut should parse");
        assert!(mods.contains(Modifiers::SHIFT));
        assert_eq!(code, Code::ArrowDown);

        assert!(parse_shortcut("cmd+madeup").is_none());
    }

    #[test]
    fn normalize_shortcut_sorts_and_lowercases() {
        assert_eq!(normalize_shortcut("Cmd+Shift+C"), "cmd+shift+c");
        assert_eq!(normalize_shortcut("shift+cmd+C"), "cmd+shift+c");
        assert_eq!(normalize_shortcut("ctrl+alt+delete"), "alt+ctrl+delete");
        assert_eq!(normalize_shortcut("command+opt+K"), "alt+cmd+k");
    }

    #[test]
    fn keystroke_to_shortcut_orders_modifiers() {
        let mut modifiers = gpui::Modifiers::default();
        modifiers.alt = true;
        modifiers.shift = true;
        assert_eq!(keystroke_to_shortcut("K", &modifiers), "alt+shift+k");

        let modifiers = gpui::Modifiers::default();
        assert_eq!(keystroke_to_shortcut("A", &modifiers), "a");
    }
}
