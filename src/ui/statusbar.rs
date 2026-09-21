use crate::app::{App, InputMode};
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Renders the bottom status / keybinding hint bar.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let line = if let Some(loading) = &app.loading {
        Line::from(format!(" ⏳ {}", loading.message))
    } else if app.input_mode == InputMode::Search {
        Line::from(vec![
            Span::styled(" /", theme::search_highlight()),
            Span::styled(
                app.search_input.display(&app.search_query, false),
                theme::value(),
            ),
            Span::styled("  [Enter] confirm  [Esc] cancel", theme::password_hidden()),
        ])
    } else if let Some((msg, _)) = &app.status_message {
        Line::from(Span::styled(format!(" ℹ  {msg}"), theme::value()))
    } else if let Some(deadline) = app
        .clipboard_expires
        .filter(|deadline| *deadline > std::time::Instant::now())
    {
        Line::from(format!(
            " Copied · clipboard expires in {}s",
            deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_secs()
                + 1
        ))
    } else if !app.is_initialized {
        Line::from(vec![
            Span::styled(" i", theme::search_highlight()),
            Span::styled(" Init Store  ", theme::active_border()),
            Span::styled("a", theme::label()),
            Span::styled(" Add  ", theme::value()),
            Span::styled("?", theme::label()),
            Span::styled(" Help  ", theme::value()),
            Span::styled("q", theme::label()),
            Span::styled(" Quit", theme::value()),
        ])
    } else {
        let mut spans = Vec::new();

        // Live Git indicator
        if app.git_status.is_git_repo {
            let branch = &app.git_status.branch;
            let sync_info = if app.git_status.ahead > 0 && app.git_status.behind > 0 {
                format!(" ↑{} ↓{}", app.git_status.ahead, app.git_status.behind)
            } else if app.git_status.ahead > 0 {
                format!(" ↑{}", app.git_status.ahead)
            } else if app.git_status.behind > 0 {
                format!(" ↓{}", app.git_status.behind)
            } else if app.git_status.remote_url.is_some() {
                " ✓".to_string()
            } else {
                "".to_string()
            };

            let git_style = if app.git_status.ahead > 0 {
                theme::git_ahead()
            } else if app.git_status.behind > 0 {
                theme::git_behind()
            } else if app.git_status.remote_url.is_some() {
                theme::git_synced()
            } else {
                theme::value()
            };

            spans.push(Span::styled(format!(" 🌿 {branch}{sync_info} "), git_style));
            spans.push(Span::styled("│", theme::password_hidden()));
        }

        spans.extend(vec![
            Span::styled(" j/k", theme::label()),
            Span::styled(" Nav  ", theme::value()),
            Span::styled("Enter", theme::label()),
            Span::styled(" View  ", theme::value()),
            Span::styled(
                app.config.keybindings.copy_password.to_string(),
                theme::label(),
            ),
            Span::styled(" Copy  ", theme::value()),
            Span::styled("a", theme::label()),
            Span::styled(" Add  ", theme::value()),
            Span::styled("t", theme::label()),
            Span::styled(" OTP  ", theme::value()),
            Span::styled("f/F", theme::label()),
            Span::styled(" Favorites  ", theme::value()),
            Span::styled("v", theme::label()),
            Span::styled(" History  ", theme::value()),
            Span::styled("G", theme::label()),
            Span::styled(" Git  ", theme::value()),
            Span::styled("P", theme::label()),
            Span::styled(" Push  ", theme::value()),
            Span::styled("/", theme::label()),
            Span::styled(" Search  ", theme::value()),
            Span::styled("?", theme::label()),
            Span::styled(" Help  ", theme::value()),
            Span::styled("q", theme::label()),
            Span::styled(" Quit", theme::value()),
        ]);

        Line::from(spans)
    };

    let bar = Paragraph::new(line).style(theme::status_bar());
    frame.render_widget(bar, area);
}
