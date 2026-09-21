use crate::app::App;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};

/// Renders the entry detail panel on the right side.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered()
        .title(format!(
            " {} ",
            app.selected_entry()
                .map(|entry| entry.path.as_str())
                .unwrap_or("Details")
        ))
        .title_style(theme::title())
        .border_type(BorderType::Rounded)
        .border_style(theme::inactive_border());

    let content = match &app.detail {
        Some(entry) => build_detail_lines(app, entry),
        None => build_placeholder(app),
    };

    let paragraph = Paragraph::new(content)
        .block(block)
        .scroll((app.detail_scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn build_detail_lines<'a>(
    app: &App,
    entry: &'a crate::pass::commands::DecryptedEntry,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();

    // Password
    lines.push(Line::from(""));
    let pass_display = if entry.password.contains("otpauth://") {
        Span::styled("OTP token · t for code", theme::password_hidden())
    } else if app.show_password {
        Span::styled(entry.password.as_str(), theme::value())
    } else {
        Span::styled("••••••••••••••••", theme::password_hidden())
    };
    lines.push(Line::from(vec![
        Span::styled("  Password  ", theme::label()),
        pass_display,
    ]));

    let toggle_hint = if app.show_password { "hide" } else { "show" };
    lines.push(Line::from(Span::styled(
        format!("              [p to {toggle_hint}]"),
        theme::password_hidden(),
    )));

    // Metadata fields
    if !entry.fields.is_empty() {
        lines.push(Line::from(""));
        for (key, val) in &entry.fields {
            if crate::recovery::field(key) {
                continue;
            }
            let padded_key = format!("  {key:<10}  ");
            lines.push(Line::from(vec![
                Span::styled(padded_key, theme::label()),
                Span::styled(
                    if key.eq_ignore_ascii_case("otpauth") || val.contains("otpauth://") {
                        "OTP secret hidden · t for code"
                    } else {
                        val.as_str()
                    },
                    theme::value(),
                ),
            ]));
        }
    }

    let recovery = crate::recovery::codes(&entry.content);
    if !recovery.is_empty() {
        lines.push(Line::from(format!(
            "  Recovery codes: {} available · {} used [R to manage]",
            recovery.iter().filter(|c| !c.used).count(),
            recovery.iter().filter(|c| c.used).count()
        )));
    }

    // Notes
    if !entry.notes.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  Notes", theme::label())));
        for note in &entry.notes {
            lines.push(Line::from(Span::styled(
                if note.contains("otpauth://") {
                    "  OTP secret hidden · t for code".into()
                } else {
                    format!("  {note}")
                },
                theme::value(),
            )));
        }
    }

    lines
}

fn build_placeholder<'a>(app: &App) -> Vec<Line<'a>> {
    let mut lines = vec![Line::from("")];

    if !app.pass_available {
        lines.push(Line::from(Span::styled(
            "  ⚠  `pass` is not installed",
            theme::error_border(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Install it with:",
            theme::value(),
        )));
        lines.push(Line::from(Span::styled(
            "    sudo pacman -S pass",
            theme::label(),
        )));
        lines.push(Line::from(Span::styled(
            "    sudo apt install pass",
            theme::label(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Then initialize:",
            theme::value(),
        )));
        lines.push(Line::from(Span::styled(
            "    pass init <your-gpg-id>",
            theme::label(),
        )));
    } else if !app.is_initialized {
        lines.push(Line::from(Span::styled(
            "  ⚠  Password Store Not Initialized",
            theme::popup_border(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Before you can store passwords, you need to",
            theme::value(),
        )));
        lines.push(Line::from(Span::styled(
            "  initialize the store with a GPG key.",
            theme::value(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press  i  to initialize your store now!",
            theme::title(),
        )));
        lines.push(Line::from(Span::styled(
            "  (We can generate a GPG key for you if needed)",
            theme::password_hidden(),
        )));
    } else if app.visible.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No entries found",
            theme::password_hidden(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            if app.search_query.is_empty() {
                "  Press a to add a new entry"
            } else {
                "  No matches. Press / to change your search."
            },
            theme::value(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  Select an entry and press",
            theme::password_hidden(),
        )));
        lines.push(Line::from(Span::styled(
            "  Enter to view details",
            theme::password_hidden(),
        )));
    }

    lines
}
