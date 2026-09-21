use crate::clipboard::ClipboardManager;
use crate::config::Config;
use crate::event::{self, AppEvent};
use crate::favorites::Favorites;
use crate::pass::{commands, store};
use crate::text_input::TextInput;
use crate::ui::theme;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph};
use std::time::Duration;

pub fn run(terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let mut config = Config::load();
    theme::configure(&mut config);
    let store_dir = store::get_store_dir();
    let tree = store::scan_store(&store_dir);
    let mut entries = Vec::new();
    store::flatten_tree_all(&tree, 0, &mut entries);
    entries.retain(|entry| !entry.is_dir);

    let mut pending: Option<std::sync::mpsc::Receiver<Result<(), String>>> = None;
    let mut message: Option<String> = config.warning.clone();
    let mut favorites = Favorites::load(&store_dir).unwrap_or_else(|error| {
        message = Some(error);
        Favorites::default()
    });
    let mut input = TextInput::default();
    let mut focus: Option<String> = None;
    let mut query = String::new();
    let mut selected = 0;
    let mut locked = false;
    let mut last_activity = std::time::Instant::now();
    let mut session = crate::clipboard::ClipboardSession::default();

    loop {
        if !locked
            && config.behavior.idle_lock_seconds > 0
            && last_activity.elapsed().as_secs() >= config.behavior.idle_lock_seconds
        {
            session.revoke();
            locked = true;
            query.clear();
            input = Default::default();
            message = None;
        }
        if let Some(receiver) = &pending {
            match receiver.try_recv() {
                Ok(Ok(())) if !locked => return Ok(()),
                Ok(Ok(())) => pending = None,
                Ok(Err(error)) => {
                    pending = None;
                    if !locked {
                        message = Some(error);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    pending = None;
                    message = Some("Copy operation stopped. Press Enter to retry.".into());
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let matches = matching_entries(&entries, &query, &favorites);
        if let Some(path) = focus.take() {
            selected = matches
                .iter()
                .position(|entry| entry.path == path)
                .unwrap_or(selected);
        }
        if selected >= matches.len() {
            selected = matches.len().saturating_sub(1);
        }

        terminal.draw(|frame| {
            if locked {
                crate::ui::layout::render_locked(frame, pending.is_some(), None);
                return;
            }
            render(
                frame,
                &matches,
                &input.display(&query, false),
                &query,
                selected,
                message.as_deref(),
                &favorites,
            )
        })?;

        let input_event = event::poll_event(Duration::from_millis(100))?;
        if !locked
            && config.behavior.idle_lock_seconds > 0
            && last_activity.elapsed().as_secs() >= config.behavior.idle_lock_seconds
        {
            session.revoke();
            locked = true;
            query.clear();
            input = Default::default();
            message = None;
        }
        let key = match input_event {
            Some(AppEvent::Key(key)) => key,
            Some(AppEvent::Paste(text)) => {
                let text = zeroize::Zeroizing::new(text);
                if locked {
                    continue;
                }
                last_activity = std::time::Instant::now();
                if let Err(error) = input.insert(&mut query, &text, false) {
                    message = Some(error);
                }
                selected = 0;
                continue;
            }
            _ => continue,
        };
        if locked {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                KeyCode::Enter if pending.is_none() => {
                    locked = false;
                    session = Default::default();
                    last_activity = std::time::Instant::now();
                }
                _ => (),
            }
            continue;
        }
        last_activity = std::time::Instant::now();
        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
            session.revoke();
            locked = true;
            query.clear();
            input = Default::default();
            message = None;
            continue;
        }
        {
            match key.code {
                KeyCode::Esc => {
                    session.revoke();
                    return Ok(());
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(entry) = matches.get(selected) {
                        let path = entry.path.clone();
                        match favorites.toggle(&path) {
                            Ok(_) => {
                                focus = Some(path);
                                message = None;
                            }
                            Err(error) => message = Some(error),
                        }
                    }
                }
                KeyCode::Enter if pending.is_none() => {
                    if let Some(entry) = matches.get(selected) {
                        let path = entry.path.clone();
                        let seconds = config.behavior.auto_clear_clipboard_seconds;
                        let (sender, receiver) = std::sync::mpsc::channel();
                        pending = Some(receiver);
                        message = Some("Decrypting and copying… Esc closes picker".into());
                        let session = session.clone();
                        std::thread::spawn(move || {
                            let result = commands::pass_show(&path).and_then(|entry| {
                                ClipboardManager::new().copy_for_session(
                                    entry.login_password()?,
                                    Duration::from_secs(seconds),
                                    &session,
                                )
                            });
                            let _ = sender.send(result);
                        });
                    }
                }
                KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if selected + 1 < matches.len() {
                        selected += 1;
                    }
                }
                _ => {
                    if input.handle(&mut query, key) {
                        selected = 0;
                    }
                }
            }
        }
    }
}

fn matching_entries<'a>(
    entries: &'a [store::FlatEntry],
    query: &str,
    favorites: &Favorites,
) -> Vec<&'a store::FlatEntry> {
    let query = query.to_lowercase();
    let mut entries: Vec<_> = entries
        .iter()
        .filter(|entry| crate::search::matches(&entry.path, &query))
        .collect();
    entries.sort_by_key(|entry| {
        (
            !favorites.contains(&entry.path),
            crate::search::rank(&entry.path, &query),
        )
    });
    entries
}

fn render(
    frame: &mut ratatui::Frame,
    entries: &[&store::FlatEntry],
    display: &str,
    query: &str,
    selected: usize,
    message: Option<&str>,
    favorites: &Favorites,
) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme::base()), area);
    let width = 64.min(area.width);
    let height = (entries.len() as u16 + 6).clamp(8, 16).min(area.height);
    let [_, picker_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, picker_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .areas(picker_area);

    frame.render_widget(Clear, picker_area);
    frame.render_widget(Block::default().style(theme::base()), picker_area);
    let [search_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(picker_area);

    let search = Paragraph::new(Line::from(vec![
        Span::styled(" > ", theme::label()),
        Span::styled(display, theme::input_active()),
    ]))
    .block(
        Block::bordered()
            .title(" PassTUI Pick ")
            .title_style(theme::title())
            .border_type(BorderType::Rounded)
            .border_style(theme::popup_border()),
    );
    frame.render_widget(search, search_area);

    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new("  No matches. Try a shorter search."),
            list_area,
        );
    }
    let items: Vec<ListItem> = entries
        .iter()
        .map(|entry| {
            let mut spans = vec![Span::styled(
                if favorites.contains(&entry.path) {
                    "★ "
                } else {
                    "  "
                },
                theme::label(),
            )];
            spans.extend(crate::ui::highlight_search(
                &entry.path,
                query,
                theme::entry(),
            ));
            ListItem::new(Line::from(spans))
        })
        .collect();
    let list = List::new(items)
        .highlight_style(theme::selected_item())
        .highlight_symbol("▸ ");
    let mut state = ratatui::widgets::ListState::default();
    if !entries.is_empty() {
        state.select(Some(selected));
    }
    if !entries.is_empty() {
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    let footer =
        Paragraph::new(message.unwrap_or("  Enter copy  ↑/↓ move  Ctrl+f favorite  Esc quit"))
            .style(theme::password_hidden());
    frame.render_widget(footer, footer_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, path: &str) -> store::FlatEntry {
        store::FlatEntry {
            name: name.to_string(),
            path: path.to_string(),
            is_dir: false,
            depth: 0,
            expanded: false,
            has_children: false,
        }
    }

    #[test]
    fn matching_entries_filters_name_and_path_case_insensitively() {
        let entries = vec![
            entry("GitHub", "Work/GitHub"),
            entry("Email", "Personal/email"),
        ];

        let matches = matching_entries(&entries, "personal", &Favorites::default());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "Email");

        let matches = matching_entries(&entries, "GITHUB", &Favorites::default());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "Work/GitHub");
    }
}
