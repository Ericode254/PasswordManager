use crate::ui::theme;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::Line,
    widgets::Paragraph,
};
use std::time::Duration;

pub const HEIGHT: u16 = 10;
const FRAME_TIME: Duration = Duration::from_millis(140);
const CAT: [[&str; 5]; 4] = [
    [
        r"     /\_/\     ",
        r"    ( o.o )    ",
        r"    / >o--==   ",
        r"   /|     |\   ",
        r"    (_) (_)    ",
    ],
    [
        r"     /\_/\     ",
        r"    ( o.o )    ",
        r"    / >o--==   ",
        r"    |     |\   ",
        r"    (_)-(_) \  ",
    ],
    [
        r"     /\_/\     ",
        r"    ( -.- )    ",
        r"    / >o--==   ",
        r"   /|     |    ",
        r"  / (_) (_)    ",
    ],
    [
        r"     /\_/\     ",
        r"    ( ^.^ )    ",
        r"    / >o--==   ",
        r"   /|     |\   ",
        r"    (_)-(_)    ",
    ],
];

/// Draw alongside authentication; input and animation share its event loop.
pub fn render(frame: &mut Frame, area: Rect, elapsed: Duration) {
    let step = (elapsed.as_millis() / FRAME_TIME.as_millis()) as usize;
    let mut lines = Vec::new();
    if area.height >= HEIGHT {
        lines.extend([Line::styled("PassTUI", theme::title()), Line::from("")]);
    }
    lines.extend(
        CAT[step % CAT.len()]
            .iter()
            .map(|line| Line::styled(*line, theme::label())),
    );
    if area.height >= HEIGHT {
        lines.extend([
            Line::from(""),
            Line::styled("Your tiny vault guardian", theme::value()),
        ]);
    }
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}
