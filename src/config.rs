use serde::{Deserialize, Serialize};
use std::process::Command;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bun_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub modifiers: Vec<String>,
    pub key: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: "Semicolon".to_string(),  // Cmd+; matches main.rs default
            },
            bun_path: None,  // Will use system PATH if not specified
        }
    }
}

pub fn load_config() -> Config {
    let config_path = PathBuf::from(shellexpand::tilde("~/.kit/config.ts").as_ref());

    // Check if config file exists
    if !config_path.exists() {
        eprintln!("Config file not found at {:?}, using defaults", config_path);
        return Config::default();
    }

    // Step 1: Transpile TypeScript to JavaScript using bun build
    let tmp_js_path = "/tmp/kit-config.js";
    let build_output = Command::new("bun")
        .arg("build")
        .arg("--target=bun")
        .arg(config_path.to_string_lossy().to_string())
        .arg(format!("--outfile={}", tmp_js_path))
        .output();

    match build_output {
        Err(e) => {
            eprintln!("Failed to transpile config with bun: {}", e);
            return Config::default();
        }
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "bun build failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return Config::default();
            }
        }
    }

    // Step 2: Execute the transpiled JS and extract the default export as JSON
    let json_output = Command::new("bun")
        .arg("-e")
        .arg(format!(
            "console.log(JSON.stringify(require('{}').default))",
            tmp_js_path
        ))
        .output();

    match json_output {
        Err(e) => {
            eprintln!("Failed to execute bun to extract JSON: {}", e);
            return Config::default();
        }
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "bun execution failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return Config::default();
            }

            // Step 3: Parse the JSON output into Config struct
            let json_str = String::from_utf8_lossy(&output.stdout);
            match serde_json::from_str::<Config>(json_str.trim()) {
                Ok(config) => {
                    eprintln!("Successfully loaded config from {:?}", config_path);
                    config
                }
                Err(e) => {
                    eprintln!("Failed to parse config JSON: {}", e);
                    eprintln!("JSON output was: {}", json_str);
                    Config::default()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.hotkey.modifiers, vec!["meta"]);
        assert_eq!(config.hotkey.key, "Semicolon");
        assert_eq!(config.bun_path, None);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            hotkey: HotkeyConfig {
                modifiers: vec!["ctrl".to_string(), "alt".to_string()],
                key: "KeyA".to_string(),
            },
            bun_path: Some("/usr/local/bin/bun".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.hotkey.modifiers, config.hotkey.modifiers);
        assert_eq!(deserialized.hotkey.key, config.hotkey.key);
        assert_eq!(deserialized.bun_path, config.bun_path);
    }
}
