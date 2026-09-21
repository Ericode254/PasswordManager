pub mod detail;
pub mod layout;
pub mod popup;
pub mod statusbar;
pub mod theme;
pub mod tree;

pub fn highlight_search(
    text: &str,
    query: &str,
    base: ratatui::style::Style,
) -> Vec<ratatui::text::Span<'static>> {
    let lowered = query.to_lowercase();
    let mut wanted = lowered.chars().peekable();
    text.chars()
        .map(|character| {
            let mut matched = false;
            for lower in character.to_lowercase() {
                if wanted.peek() == Some(&lower) {
                    wanted.next();
                    matched = true;
                }
            }
            ratatui::text::Span::styled(
                character.to_string(),
                if matched {
                    theme::search_highlight()
                } else {
                    base
                },
            )
        })
        .collect()
}
