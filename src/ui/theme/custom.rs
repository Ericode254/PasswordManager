use super::{Palette, palette_for};
use crate::config::ThemeConfig;
use ratatui::style::Color;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    #[serde(default = "default_base")]
    base: String,
    #[serde(default)]
    colors: BTreeMap<String, ThemeColor>,
}

fn default_base() -> String {
    "catppuccin-mocha".into()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ThemeColor {
    Text(String),
    Indexed(u8),
}

impl ThemeColor {
    fn parse(&self) -> Result<Color, String> {
        match self {
            Self::Indexed(index) => Ok(Color::Indexed(*index)),
            Self::Text(value) if value == "default" => Ok(Color::Reset),
            Self::Text(value) => {
                let hex = value
                    .strip_prefix('#')
                    .filter(|hex| {
                        hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
                    .ok_or_else(|| {
                        "expected \"#RRGGBB\", \"default\", or an integer from 0 to 255".to_string()
                    })?;
                let color = u32::from_str_radix(hex, 16).map_err(|error| error.to_string())?;
                Ok(Color::Rgb(
                    (color >> 16) as u8,
                    (color >> 8) as u8,
                    color as u8,
                ))
            }
        }
    }
}

fn parse_theme(contents: &str) -> Result<Palette, String> {
    let definition: ThemeFile =
        toml::from_str(contents).map_err(|error| format!("Invalid theme TOML: {error}"))?;
    let mut palette = palette_for(&definition.base).ok_or_else(|| {
        format!(
            "Unknown base theme '{}'; choose a built-in preset",
            definition.base
        )
    })?;
    for (name, value) in definition.colors {
        let destination = match name.as_str() {
            "primary" => &mut palette.primary,
            "secondary" => &mut palette.secondary,
            "accent" => &mut palette.accent,
            "text" => &mut palette.text,
            "muted" => &mut palette.muted,
            "background" => &mut palette.background,
            "selection_bg" => &mut palette.selection_bg,
            "selection_fg" => &mut palette.selection_fg,
            "success" => &mut palette.success,
            "warning" => &mut palette.warning,
            "error" => &mut palette.error,
            _ => {
                return Err(format!(
                    "Unknown theme color '{name}'; see themes/README.md for supported keys"
                ));
            }
        };
        *destination = value
            .parse()
            .map_err(|error| format!("Color '{name}': {error}"))?;
    }
    Ok(palette)
}

fn theme_path(config: &ThemeConfig, config_dir: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(file) = &config.file {
        if file.as_os_str().is_empty() {
            return Err("Theme file path cannot be empty".into());
        }
        return if file.is_absolute() {
            Ok(file.clone())
        } else {
            Ok(config_dir
                .ok_or("Cannot locate the PassTUI configuration directory")?
                .join(file))
        };
    }
    if config.name.is_empty()
        || !config
            .name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("Theme names may contain letters, numbers, '-' and '_'; use theme.file for an explicit path".into());
    }
    Ok(config_dir
        .ok_or("Cannot locate the PassTUI configuration directory")?
        .join("themes")
        .join(format!("{}.toml", config.name)))
}

pub(super) fn load(config: &ThemeConfig, config_dir: Option<&Path>) -> Result<Palette, String> {
    if config.file.is_none()
        && let Some(palette) = palette_for(&config.name)
    {
        return Ok(palette);
    }
    let path = theme_path(config, config_dir)?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("Cannot read theme {}: {error}", path.display()))?;
    parse_theme(&contents).map_err(|error| format!("Theme {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_theme_inherits_omitted_colors() {
        let theme = parse_theme(
            "base = 'nord'\n[colors]\nprimary = '#012aEF'\nbackground = 'default'\naccent = 208",
        )
        .unwrap();
        let base = palette_for("nord").unwrap();
        assert_eq!(theme.primary, Color::Rgb(1, 42, 239));
        assert_eq!(theme.background, Color::Reset);
        assert_eq!(theme.accent, Color::Indexed(208));
        assert_eq!(theme.selection_bg, base.selection_bg);
        assert_eq!(theme.text, base.text);
    }

    #[test]
    fn malformed_and_misspelled_colors_are_rejected() {
        for value in ["'#123'", "'#gggggg'", "'#ééé'", "'red'", "256", "-1", "1.5"] {
            assert!(
                parse_theme(&format!("[colors]\nprimary = {value}")).is_err(),
                "accepted {value}"
            );
        }
        assert!(
            parse_theme("[colors]\nprimry = '#123456'")
                .unwrap_err()
                .contains("primry")
        );
        assert!(parse_theme("[colours]\nprimary = '#123456'").is_err());
        assert!(parse_theme("base = 'missing'").is_err());
        assert!(parse_theme("[colors").is_err());
    }

    #[test]
    fn paths_resolve_relative_to_config_not_working_directory() {
        let root = Path::new("/tmp/passtui-theme-config");
        let mut config = ThemeConfig {
            name: "my-theme".into(),
            file: None,
        };
        assert_eq!(
            theme_path(&config, Some(root)).unwrap(),
            root.join("themes/my-theme.toml")
        );
        config.file = Some("themes/custom.toml".into());
        assert_eq!(
            theme_path(&config, Some(root)).unwrap(),
            root.join("themes/custom.toml")
        );
        config.file = Some("/tmp/shared-theme.toml".into());
        assert_eq!(
            theme_path(&config, None).unwrap(),
            Path::new("/tmp/shared-theme.toml")
        );
        config.file = None;
        config.name = "../escape".into();
        assert!(theme_path(&config, Some(root)).is_err());
    }

    #[test]
    fn named_and_relative_file_selection_load_the_same_palette() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let named = ThemeConfig {
            name: "my-theme".into(),
            file: None,
        };
        let file = ThemeConfig {
            name: "nord".into(),
            file: Some("themes/my-theme.toml".into()),
        };
        assert_eq!(
            load(&named, Some(root)).unwrap(),
            load(&file, Some(root)).unwrap()
        );
        assert_eq!(
            load(&ThemeConfig::default(), None).unwrap(),
            palette_for("catppuccin-mocha").unwrap()
        );
    }

    #[test]
    fn shipped_example_loads_all_colors_from_file() {
        let config = ThemeConfig {
            name: "irrelevant".into(),
            file: Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("themes/my-theme.toml")),
        };
        let theme = load(&config, None).unwrap();
        assert_eq!(theme.background, Color::Rgb(16, 24, 39));
        assert_eq!(theme.selection_fg, Color::Rgb(16, 24, 39));
        assert_eq!(theme.primary, Color::Rgb(125, 211, 252));
    }
}
