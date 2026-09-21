use crate::app::App;
use crate::ui::{detail, popup, statusbar, tree};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

/// Renders the full application layout and all UI components.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if app.locked {
        render_locked(frame, app.loading.is_some(), app.lock_outcome.as_deref());
        return;
    }
    frame.render_widget(
        ratatui::widgets::Block::default().style(crate::ui::theme::base()),
        area,
    );

    // ── Vertical split: main content | footer ──────────
    let [main_area, footer_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

    if area.width < 80 {
        if app.detail.is_some() {
            detail::render(frame, app, main_area);
        } else {
            tree::render(frame, app, main_area);
        }
        statusbar::render(frame, app, footer_area);
        popup::render(frame, app);
        return;
    }

    // ── Horizontal split: tree (left) | detail (right) ─
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
            .areas(main_area);

    // ── Render panels ──────────────────────────────────
    tree::render(frame, app, left_area);
    detail::render(frame, app, right_area);
    statusbar::render(frame, app, footer_area);

    // ── Popup overlay (rendered on top of everything) ──
    popup::render(frame, app);
}

pub fn render_locked(frame: &mut Frame, pending: bool, outcome: Option<&str>) {
    use ratatui::widgets::{Block, Paragraph, Wrap};
    frame.render_widget(
        Block::default().style(crate::ui::theme::base()),
        frame.area(),
    );
    let message = format!(
        "Session cleared\n\nDecrypted details, previews and unsaved drafts have been discarded.\nGPG may still be unlocked by its agent.\n\n{}\n{}",
        if pending {
            "Waiting for the background action to finish…"
        } else {
            "Enter resumes browsing · q quits"
        },
        outcome.unwrap_or("")
    );
    frame.render_widget(
        Paragraph::new(message)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" PassTUI · Privacy lock ")),
        frame.area(),
    );
}
