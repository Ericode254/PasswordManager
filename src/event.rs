use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use std::time::Duration;

/// Application-level events.
pub enum AppEvent {
    /// A key was pressed.
    Key(KeyEvent),
    Paste(String),
    /// No input within the poll window — use for periodic updates.
    Tick,
}

/// Polls for terminal events with the given timeout.
/// Returns `Key` on a key press, `Tick` on timeout, or `Ok(None)` for irrelevant events.
pub fn poll_event(timeout: Duration) -> std::io::Result<Option<AppEvent>> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                return Ok(Some(AppEvent::Key(key)));
            }
            Event::Paste(text) => return Ok(Some(AppEvent::Paste(text))),
            _ => (),
        }
        Ok(None)
    } else {
        Ok(Some(AppEvent::Tick))
    }
}

pub struct PasteGuard;
impl PasteGuard {
    pub fn enable() -> std::io::Result<Self> {
        if let Err(error) = crossterm::execute!(std::io::stdout(), event::EnableBracketedPaste) {
            ratatui::restore();
            return Err(error);
        }
        Ok(Self)
    }
}
pub fn disable_paste() {
    let _ = crossterm::execute!(std::io::stdout(), event::DisableBracketedPaste);
}
impl Drop for PasteGuard {
    fn drop(&mut self) {
        disable_paste();
    }
}
