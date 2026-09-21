use crate::app::App;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, List, ListItem};

/// Renders the password store tree in the left panel.
pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered()
        .title(if app.favorites_only {
            " ★ Favorites · F show all "
        } else {
            " 🔐 Password Store "
        })
        .title_style(theme::title())
        .border_type(BorderType::Rounded)
        .border_style(theme::active_border());

    let items: Vec<ListItem> = app
        .visible
        .iter()
        .map(|entry| {
            let indent = "  ".repeat(entry.depth);
            let (icon, style) = if entry.is_dir {
                let arrow = if entry.expanded { "▼" } else { "▶" };
                (format!("{indent}{arrow} 📁 "), theme::folder())
            } else {
                (
                    format!(
                        "{indent}   {} ",
                        if app.favorites.contains(&entry.path) {
                            "★"
                        } else {
                            "🔑"
                        }
                    ),
                    theme::entry(),
                )
            };

            let mut spans = vec![Span::styled(icon, style)];
            let label = if app.search_query.is_empty() && !app.favorites_only {
                &entry.name
            } else {
                &entry.path
            };
            spans.extend(crate::ui::highlight_search(label, &app.search_query, style));
            let line = Line::from(spans);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::selected_item().add_modifier(Modifier::BOLD))
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}
