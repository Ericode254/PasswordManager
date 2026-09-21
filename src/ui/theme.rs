mod custom;

use ratatui::style::{Color, Modifier, Style};
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, PartialEq)]
struct Palette {
    primary: Color,
    secondary: Color,
    accent: Color,
    text: Color,
    muted: Color,
    background: Color,
    selection_bg: Color,
    selection_fg: Color,
    success: Color,
    warning: Color,
    error: Color,
}

static PALETTE: OnceLock<RwLock<Palette>> = OnceLock::new();

fn rgb(value: (u8, u8, u8)) -> Color {
    Color::Rgb(value.0, value.1, value.2)
}

fn catppuccin_mocha() -> Palette {
    Palette {
        primary: rgb((137, 180, 250)),
        secondary: rgb((137, 180, 250)),
        accent: rgb((249, 226, 175)),
        text: rgb((205, 214, 244)),
        muted: rgb((127, 132, 156)),
        background: rgb((30, 30, 46)),
        selection_bg: rgb((137, 180, 250)),
        selection_fg: rgb((30, 30, 46)),
        success: rgb((166, 227, 161)),
        warning: rgb((249, 226, 175)),
        error: rgb((243, 139, 168)),
    }
}

fn palette_for(name: &str) -> Option<Palette> {
    Some(match name.to_lowercase().replace('_', "-").as_str() {
        "catppuccin-latte" => Palette {
            primary: rgb((30, 102, 245)),
            secondary: rgb((30, 102, 245)),
            accent: rgb((223, 142, 29)),
            text: rgb((76, 79, 105)),
            muted: rgb((156, 160, 176)),
            background: rgb((239, 241, 245)),
            selection_bg: rgb((30, 102, 245)),
            selection_fg: rgb((239, 241, 245)),
            success: rgb((64, 160, 43)),
            warning: rgb((223, 142, 29)),
            error: rgb((210, 15, 57)),
        },
        "tokyo-night" => Palette {
            primary: rgb((125, 207, 255)),
            secondary: rgb((122, 162, 247)),
            accent: rgb((224, 175, 104)),
            text: rgb((192, 202, 245)),
            muted: rgb((86, 95, 137)),
            background: rgb((26, 27, 38)),
            selection_bg: rgb((122, 162, 247)),
            selection_fg: rgb((26, 27, 38)),
            success: rgb((158, 206, 106)),
            warning: rgb((224, 175, 104)),
            error: rgb((247, 118, 142)),
        },
        "gruvbox" => Palette {
            primary: rgb((131, 165, 152)),
            secondary: rgb((142, 192, 124)),
            accent: rgb((250, 189, 47)),
            text: rgb((235, 219, 178)),
            muted: rgb((146, 131, 116)),
            background: rgb((40, 40, 40)),
            selection_bg: rgb((131, 165, 152)),
            selection_fg: rgb((40, 40, 40)),
            success: rgb((184, 187, 38)),
            warning: rgb((250, 189, 47)),
            error: rgb((251, 73, 52)),
        },
        "nord" => Palette {
            primary: rgb((136, 192, 208)),
            secondary: rgb((129, 161, 193)),
            accent: rgb((235, 203, 139)),
            text: rgb((216, 222, 233)),
            muted: rgb((118, 129, 149)),
            background: rgb((46, 52, 64)),
            selection_bg: rgb((136, 192, 208)),
            selection_fg: rgb((46, 52, 64)),
            success: rgb((163, 190, 140)),
            warning: rgb((235, 203, 139)),
            error: rgb((191, 97, 106)),
        },
        "dracula" => Palette {
            primary: rgb((139, 233, 253)),
            secondary: rgb((189, 147, 249)),
            accent: rgb((241, 250, 140)),
            text: rgb((248, 248, 242)),
            muted: rgb((98, 114, 164)),
            background: rgb((40, 42, 54)),
            selection_bg: rgb((189, 147, 249)),
            selection_fg: rgb((40, 42, 54)),
            success: rgb((80, 250, 123)),
            warning: rgb((241, 250, 140)),
            error: rgb((255, 85, 85)),
        },
        "catppuccin-mocha" => catppuccin_mocha(),
        _ => return None,
    })
}

fn current() -> Palette {
    PALETTE
        .get_or_init(|| RwLock::new(catppuccin_mocha()))
        .read()
        .expect("theme lock poisoned")
        .clone()
}

/// Load a preset or custom palette once at startup; retain other config warnings.
pub fn configure(config: &mut crate::config::Config) {
    let directory = dirs::config_dir().map(|directory| directory.join("passtui"));
    let palette = match custom::load(&config.theme, directory.as_deref()) {
        Ok(palette) => palette,
        Err(error) => {
            let warning = format!("{error}. Using catppuccin-mocha.");
            config.warning = Some(match config.warning.take() {
                Some(existing) => format!("{existing} {warning}"),
                None => warning,
            });
            catppuccin_mocha()
        }
    };
    let lock = PALETTE.get_or_init(|| RwLock::new(catppuccin_mocha()));
    *lock.write().expect("theme lock poisoned") = palette;
}

pub fn base() -> Style {
    let palette = current();
    Style::default().fg(palette.text).bg(palette.background)
}

pub fn active_border() -> Style {
    let p = current();
    Style::default().fg(p.primary).add_modifier(Modifier::BOLD)
}

pub fn inactive_border() -> Style {
    Style::default().fg(current().muted)
}

pub fn selected_item() -> Style {
    let p = current();
    Style::default()
        .bg(p.selection_bg)
        .fg(p.selection_fg)
        .add_modifier(Modifier::BOLD)
}

pub fn folder() -> Style {
    let p = current();
    Style::default()
        .fg(p.secondary)
        .add_modifier(Modifier::BOLD)
}

pub fn entry() -> Style {
    Style::default().fg(current().text)
}

pub fn popup_border() -> Style {
    let p = current();
    Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
}

pub fn error_border() -> Style {
    let p = current();
    Style::default().fg(p.error).add_modifier(Modifier::BOLD)
}

pub fn success_border() -> Style {
    let p = current();
    Style::default().fg(p.success).add_modifier(Modifier::BOLD)
}

pub fn label() -> Style {
    let p = current();
    Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
}

pub fn value() -> Style {
    Style::default().fg(current().text)
}

pub fn password_hidden() -> Style {
    Style::default().fg(current().muted)
}

pub fn status_bar() -> Style {
    let p = current();
    Style::default().bg(p.background).fg(p.text)
}

pub fn title() -> Style {
    let p = current();
    Style::default().fg(p.success).add_modifier(Modifier::BOLD)
}

pub fn search_highlight() -> Style {
    let p = current();
    Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
}

pub fn input_active() -> Style {
    let p = current();
    Style::default().fg(p.primary).add_modifier(Modifier::BOLD)
}

pub fn input_inactive() -> Style {
    Style::default().fg(current().muted)
}

pub fn strength(score: u8) -> Style {
    let p = current();
    let color = match score {
        0 | 1 => p.error,
        2 => p.warning,
        _ => p.success,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

pub fn git_synced() -> Style {
    let p = current();
    Style::default().fg(p.success).add_modifier(Modifier::BOLD)
}

pub fn git_ahead() -> Style {
    let p = current();
    Style::default().fg(p.primary).add_modifier(Modifier::BOLD)
}

pub fn git_behind() -> Style {
    let p = current();
    Style::default().fg(p.warning).add_modifier(Modifier::BOLD)
}

#[allow(dead_code)]
pub fn git_badge() -> Style {
    let p = current();
    Style::default()
        .bg(p.secondary)
        .fg(p.selection_fg)
        .add_modifier(Modifier::BOLD)
}

pub fn git_commit_hash() -> Style {
    let p = current();
    Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
}

pub fn git_commit_msg() -> Style {
    Style::default().fg(current().text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_palette_applies_and_failure_preserves_other_warnings() {
        let mut config = crate::config::Config::default();
        config.theme.file =
            Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("themes/my-theme.toml"));
        configure(&mut config);
        assert_eq!(base().bg, Some(Color::Rgb(16, 24, 39)));
        assert_eq!(selected_item().fg, Some(Color::Rgb(16, 24, 39)));
        config.theme.file = Some(std::path::PathBuf::new());
        config.warning = Some("Existing configuration warning.".into());
        config.behavior.auto_clear_clipboard_seconds = 12;
        configure(&mut config);
        assert_eq!(current(), catppuccin_mocha());
        assert!(
            config
                .warning
                .as_ref()
                .unwrap()
                .contains("Existing configuration warning.")
        );
        assert!(config.warning.as_ref().unwrap().contains("empty"));
        assert_eq!(config.behavior.auto_clear_clipboard_seconds, 12);
    }

    #[test]
    fn supports_all_bundled_presets() {
        for name in [
            "catppuccin-mocha",
            "catppuccin-latte",
            "tokyo-night",
            "gruvbox",
            "nord",
            "dracula",
        ] {
            let palette = palette_for(name).expect("bundled preset");
            assert!(matches!(palette.text, Color::Rgb(_, _, _)));
        }
    }
}
