use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeConfig,
    pub behavior: BehaviorConfig,
    pub keybindings: KeybindingsConfig,
    #[serde(skip)]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub auto_clear_clipboard_seconds: u64,
    pub idle_lock_seconds: u64,
    pub default_reveal_passwords: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub copy_password: char,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "catppuccin-mocha".to_string(),
            file: None,
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            auto_clear_clipboard_seconds: 45,
            idle_lock_seconds: 300,
            default_reveal_passwords: false,
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self { copy_password: 'y' }
    }
}

impl Config {
    pub fn load() -> Self {
        let defaults = Self::default();
        let Some(config_dir) = dirs::config_dir() else {
            return defaults;
        };
        let config_dir = config_dir.join("passtui");
        let path = config_dir.join("config.toml");

        match fs::read_to_string(&path) {
            Ok(contents) => Self::parse(&contents),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let _ = Self::write_default(&config_dir, &path, &defaults);
                defaults
            }
            Err(error) => Self {
                warning: Some(format!("Cannot read config: {error}. Using defaults.")),
                ..defaults
            },
        }
    }

    pub fn parse(contents: &str) -> Self {
        let mut config: Self = match toml::from_str(contents) {
            Ok(config) => config,
            Err(error) => {
                return Self {
                    warning: Some(format!("Invalid config: {error}. Using defaults.")),
                    ..Self::default()
                };
            }
        };
        let mut warnings = Vec::new();
        if "qQ?jkg0lhpdaeiGPU/ruwfFvHtR".contains(config.keybindings.copy_password)
            || config.keybindings.copy_password.is_control()
        {
            config.keybindings.copy_password = 'y';
            warnings.push("Copy shortcut conflicts with another action; using y.");
        }
        if !(1..=86400).contains(&config.behavior.auto_clear_clipboard_seconds) {
            config.behavior.auto_clear_clipboard_seconds = 45;
            warnings.push("Clipboard timeout must be 1–86400 seconds; using 45 seconds.");
        }
        if !warnings.is_empty() {
            config.warning = Some(warnings.join(" "));
        }
        config
    }

    fn write_default(
        config_dir: &std::path::Path,
        path: &std::path::Path,
        defaults: &Self,
    ) -> std::io::Result<()> {
        fs::create_dir_all(config_dir)?;
        let contents = toml::to_string_pretty(defaults).map_err(std::io::Error::other)?;
        fs::write(path, format!("# PassTUI configuration\n\n{contents}"))
    }

    #[cfg(test)]
    fn default_toml() -> String {
        toml::to_string_pretty(&Self::default()).expect("defaults serialize")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_documented_configuration() {
        let config = Config::default();
        assert_eq!(config.theme.name, "catppuccin-mocha");
        assert_eq!(config.behavior.auto_clear_clipboard_seconds, 45);
        assert_eq!(config.keybindings.copy_password, 'y');
        assert_eq!(config.behavior.idle_lock_seconds, 300);
    }

    #[test]
    fn parses_custom_configuration() {
        let config: Config = toml::from_str(
            r#"
                [theme]
                name = "dracula"

                [behavior]
                auto_clear_clipboard_seconds = 30
                default_reveal_passwords = true

                [keybindings]
                copy_password = "c"
                copy_totp = "t"
            "#,
        )
        .expect("valid config");

        assert_eq!(config.theme.name, "dracula");
        assert_eq!(config.behavior.auto_clear_clipboard_seconds, 30);
        assert!(config.behavior.default_reveal_passwords);
        assert_eq!(config.keybindings.copy_password, 'c');
    }

    #[test]
    fn conflicts_and_invalid_settings_are_visible_and_repaired() {
        let config = Config::parse(
            "[keybindings]\ncopy_password = \"e\"\n[behavior]\nauto_clear_clipboard_seconds = 0",
        );
        assert_eq!(config.keybindings.copy_password, 'y');
        assert_eq!(config.behavior.auto_clear_clipboard_seconds, 45);
        assert!(config.warning.is_some());
        let malformed = Config::parse("[broken");
        assert!(malformed.warning.is_some());
    }

    #[test]
    fn custom_theme_names_and_files_survive_configuration_parsing() {
        let named =
            Config::parse("[theme]\nname = 'ocean'\n[behavior]\nauto_clear_clipboard_seconds = 12");
        assert_eq!(named.theme.name, "ocean");
        assert!(named.theme.file.is_none());
        assert!(named.warning.is_none());
        assert_eq!(named.behavior.auto_clear_clipboard_seconds, 12);
        let file = Config::parse("[theme]\nfile = 'themes/ocean.toml'");
        assert_eq!(
            file.theme.file.as_deref(),
            Some(std::path::Path::new("themes/ocean.toml"))
        );
    }

    #[test]
    fn idle_timeout_can_be_disabled_and_totp_shortcut_is_reserved() {
        let config =
            Config::parse("[behavior]\nidle_lock_seconds = 0\n[keybindings]\ncopy_password = 't'");
        assert_eq!(config.behavior.idle_lock_seconds, 0);
        assert_eq!(config.keybindings.copy_password, 'y');
        assert!(config.warning.is_some());
    }

    #[test]
    fn serializes_all_default_sections() {
        let contents = Config::default_toml();
        assert!(contents.contains("[theme]"));
        assert!(contents.contains("[behavior]"));
        assert!(contents.contains("[keybindings]"));
        assert!(contents.contains("catppuccin-mocha"));
    }
}
