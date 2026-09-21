use crate::app::{ActivePopup, AddField, App};
use crate::pass::generator::{self, GeneratorMode};
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};

/// Renders the active popup overlay, if any.
pub fn render(frame: &mut Frame, app: &App) {
    match &app.popup {
        ActivePopup::None => {}
        ActivePopup::OtpInstall { plan, error, .. } => {
            let area = centered_rect(76, 14, frame.area());
            clear_popup(frame, area);
            let text = format!(
                "OTP support needs oathtool.\n\nInstall using your system package manager:\n{}\n\nAdministrator authentication may be requested in the terminal.\n\n{}\ni install and continue · Esc cancel",
                plan.description(),
                error.as_deref().unwrap_or("")
            );
            frame.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: false })
                    .block(Block::bordered().title(" Install OTP support ")),
                area,
            );
        }
        ActivePopup::Recovery(view) => render_recovery(frame, view),
        ActivePopup::Totp { path, code, error } => {
            let area = centered_rect(72, 12, frame.area());
            clear_popup(frame, area);
            let value = code.as_ref().filter(|c| !c.remaining().is_zero());
            let mut lines = vec![Line::from(path.as_str()), Line::from("")];
            if let Some(code) = value {
                lines.push(Line::from(Span::styled(
                    code.value.as_str(),
                    theme::title(),
                )));
                lines.push(Line::from(format!(
                    "Expires in {}s",
                    code.remaining().as_secs() + 1
                )));
            } else {
                lines.push(Line::from(
                    "No current code. Press r to generate a fresh code.",
                ));
            }
            lines.push(Line::from(""));
            if let Some(error) = error {
                lines.push(Line::from(Span::styled(
                    error.as_str(),
                    theme::error_border(),
                )));
            }
            lines.push(Line::from(
                "y copy · r refresh · Esc close · Ctrl+L clear session",
            ));
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .block(Block::bordered().title(" Authentication code ")),
                area,
            );
        }
        ActivePopup::History(view) => render_history(frame, view),
        ActivePopup::Rename {
            source,
            destination,
        } => {
            let area = centered_rect(72, 10, frame.area());
            clear_popup(frame, area);
            let content = format!(
                "From: {source}\n\nTo: {}\n\n{}\n←/→ edit · Ctrl+u clear · Ctrl+z undo · Enter move · Esc cancel",
                app.rename_input.display(destination, false),
                app.form_error
                    .as_deref()
                    .unwrap_or("Browser login example: github.com/personal")
            );
            frame.render_widget(
                Paragraph::new(content)
                    .wrap(Wrap { trim: false })
                    .block(Block::bordered().title(" Rename / Move ")),
                area,
            );
        }
        ActivePopup::Help => render_help(frame, app),
        ActivePopup::Confirm {
            message,
            entry_path: _,
        } => render_confirm(frame, message),
        ActivePopup::AddEntry {
            name,
            password,
            active_field,
        } => render_add_entry(
            frame,
            name,
            password,
            active_field,
            app.form_error.as_deref(),
            &app.editor,
        ),
        ActivePopup::Generator {
            name,
            password,
            length,
            uppercase,
            numbers,
            symbols,
            mode,
        } => render_generator(
            frame,
            GeneratorView {
                name,
                password,
                length: *length,
                uppercase: *uppercase,
                numbers: *numbers,
                symbols: *symbols,
                mode: *mode,
            },
        ),
        ActivePopup::InitStore { mode } => render_init_store(frame, mode),
        ActivePopup::GitSync => render_git_sync(frame, app),
        ActivePopup::SetRemote { url } => render_set_remote(frame, url),
        ActivePopup::GithubRepo { name } => render_github_repo(frame, name),
        ActivePopup::Notification {
            message, is_error, ..
        } => render_notification(frame, message, *is_error),
    }
}

// ── Centering helper ───────────────────────────────────

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [_, vert, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height.min(area.height)),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [_, horiz, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width.min(area.width)),
        Constraint::Fill(1),
    ])
    .areas(vert);

    horiz
}

// ── Help popup ─────────────────────────────────────────

fn render_help(frame: &mut Frame, app: &App) {
    let area = centered_rect(76, 23, frame.area());
    clear_popup(frame, area);
    let lines = vec![
        "j/k, ↑/↓       Navigate entries".to_string(),
        "Enter/l, h     View / expand, collapse folder".into(),
        "0 / End        First / last entry".into(),
        format!(
            "{}              Copy password",
            app.config.keybindings.copy_password
        ),
        "p              Reveal password for 15 seconds".into(),
        "t / R          OTP / recovery codes".into(),
        "Ctrl+L         Clear session and discard drafts".into(),
        "u / w          Copy username / URL from open entry".into(),
        "a / e / r      Add / edit / rename entry".into(),
        "d              Delete (explicit y confirms)".into(),
        "g              Password / passphrase generator".into(),
        "f / F          Toggle favorite / favorites filter".into(),
        "v / H          Entry history / store history (incl. deleted)".into(),
        "G / P / U      Git menu / push / pull".into(),
        "i              Initialize store".into(),
        "/              Search the whole store".into(),
        "PgUp / PgDn    Scroll entry details".into(),
        "Forms: ←/→, Home/End, Delete, Ctrl+u clear".into(),
        "Ctrl+z / Ctrl+y undo / redo; paste with terminal shortcut".into(),
        "Tab fields · Alt+Enter newline in notes · Ctrl+g generate".into(),
        "Esc            Close / cancel (q quits from main screen)".into(),
    ];
    frame.render_widget(
        Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>()).block(
            Block::bordered()
                .title(" Keyboard shortcuts ")
                .border_style(theme::popup_border()),
        ),
        area,
    );
}

fn render_recovery(frame: &mut Frame, view: &crate::recovery::View) {
    use ratatui::widgets::{List, ListItem, ListState};
    let area = centered_rect(80, 23, frame.area());
    clear_popup(frame, area);
    let block = Block::bordered().title(format!(" Recovery codes · {} ", view.path));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [list, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(7)]).areas(inner);
    let codes = view.codes();
    if codes.is_empty() {
        frame.render_widget(Paragraph::new("No recovery codes stored.\nClose this dialog, press e, then Tab to Recovery codes.\nPaste one code per line and save."), list);
    } else {
        let items = codes
            .iter()
            .enumerate()
            .map(|(index, code)| {
                let value = if index == view.selected && view.revealed() {
                    code.value
                } else {
                    "••••••••••••"
                };
                ListItem::new(format!(
                    "{}  {}  {}",
                    index + 1,
                    if code.used { "used     " } else { "available" },
                    value
                ))
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(Some(view.selected));
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("▸ ")
                .highlight_style(theme::selected_item()),
            list,
            &mut state,
        );
    }
    let available = codes.iter().filter(|code| !code.used).count();
    let action = if codes.get(view.selected).is_some_and(|code| code.used) {
        "available again"
    } else {
        "used"
    };
    let mut lines = vec![Line::from(format!(
        "{available} available · {} used",
        codes.len() - available
    ))];
    if let Some(error) = &view.error {
        lines.push(Line::from(Span::styled(error, theme::error_border())));
    }
    if view.confirming {
        lines.push(Line::from(format!(
            "Mark this code {action}? y confirms; any other key cancels"
        )));
    } else {
        lines.push(Line::from(
            "↑/↓ select · p reveal for 15s · y copy · x change used status",
        ));
        lines.push(Line::from(
            "Mark a code used after the website accepts it. Esc closes.",
        ));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), footer);
}

fn render_history(frame: &mut Frame, view: &crate::history::HistoryView) {
    use ratatui::widgets::{List, ListItem, ListState};
    let area = centered_rect(90, 25, frame.area());
    clear_popup(frame, area);
    let inner = Block::bordered().inner(area);
    frame.render_widget(
        Block::bordered()
            .title(" History · last 100 matching commits ")
            .border_style(theme::popup_border()),
        area,
    );
    let [list, preview] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(9)]).areas(inner);
    let items: Vec<_> = view
        .revisions
        .iter()
        .map(|revision| {
            ListItem::new(format!(
                "{}  {}  {}",
                revision.date,
                &revision.commit[..8],
                revision.path
            ))
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(view.selected));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(theme::selected_item())
            .highlight_symbol("▸ "),
        list,
        &mut state,
    );
    let mut lines = Vec::new();
    if view.revisions.is_empty() {
        lines.push(Line::from(
            "No saved versions found in this history window.",
        ));
    }
    if let Some(preview) = &view.preview {
        lines.push(Line::from(format!(
            "Preview: {} @ {}",
            preview.revision.path,
            &preview.revision.commit[..8]
        )));
        lines.push(Line::from(format!(
            "Password: hidden ({} characters)",
            preview.entry.password.chars().count()
        )));
        lines.push(Line::from(format!(
            "{} metadata fields · {} note lines",
            preview.entry.fields.len(),
            preview.entry.notes.len()
        )));
        lines.push(Line::from(
            "Restore replaces the entire entry, including username, URL and notes.",
        ));
    } else {
        lines.push(Line::from(
            "Enter decrypts the selected version for preview.",
        ));
    }
    if let Some(error) = &view.error {
        lines.push(Line::from(Span::styled(error, theme::error_border())));
    }
    if view.confirming {
        lines.push(Line::from(Span::styled(
            "Restore this version at its original path? y confirms; any other key cancels",
            theme::search_highlight(),
        )));
    } else {
        lines.push(Line::from(
            "↑/↓ select · Enter preview · r restore · Esc close",
        ));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), preview);
}

// ── Confirm popup ──────────────────────────────────────

fn render_confirm(frame: &mut Frame, message: &str) {
    let area = centered_rect(48, 7, frame.area());
    clear_popup(frame, area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(format!("  {message}"), theme::value())),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [y] ", theme::label()),
            Span::styled("Confirm    ", theme::value()),
            Span::styled("[Enter / n / Esc] ", theme::label()),
            Span::styled("Cancel", theme::value()),
        ]),
        Line::from(""),
    ];

    let block = Block::bordered()
        .title(" ⚠  Confirm ")
        .title_style(theme::error_border())
        .border_type(BorderType::Rounded)
        .border_style(theme::error_border());

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}

// ── Add entry popup ────────────────────────────────────

fn render_add_entry(
    frame: &mut Frame,
    name: &str,
    password: &str,
    active: &AddField,
    error: Option<&str>,
    editor: &crate::editor::EntryEditor,
) {
    let area = centered_rect(76, 23, frame.area());
    clear_popup(frame, area);
    let masked = "•".repeat(password.chars().count());
    let recovery_mask: String = editor
        .recovery_codes
        .chars()
        .map(|c| if c == '\n' { '\n' } else { '•' })
        .collect();
    let fields = [
        (
            AddField::Name,
            "Entry path (browser login: github.com/personal)",
            name,
        ),
        (AddField::Password, "Password", masked.as_str()),
        (AddField::Username, "Username", editor.username.as_str()),
        (AddField::Url, "URL", editor.url.as_str()),
        (
            AddField::Notes,
            "Notes / other fields (Alt+Enter adds a line)",
            editor.notes.as_str(),
        ),
        (
            AddField::RecoveryCodes,
            "Recovery codes (one per line; Alt+Enter adds a line)",
            recovery_mask.as_str(),
        ),
    ];
    let mut content = Vec::new();
    let mut active_line = 0;
    for (index, (field, label, value)) in fields.into_iter().enumerate() {
        let selected = field == *active;
        let style = if selected {
            theme::input_active()
        } else {
            theme::input_inactive()
        };
        content.push(Line::from(Span::styled(label, style)));
        let displayed = if selected {
            editor.inputs[index].display(
                if index == 1 {
                    password
                } else if index == 5 {
                    &editor.recovery_codes
                } else {
                    value
                },
                index == 1 || index == 5,
            )
        } else {
            value.to_string()
        };
        for line in displayed.split('\n') {
            let mut line = line.to_string();
            if selected && let Some(cursor) = line.find('█') {
                active_line = content.len() as u16;
                let width = usize::from(area.width.saturating_sub(7));
                let mut start = 0;
                while Line::from(&line[start..cursor]).width() >= width.max(1) {
                    start += line[start..].chars().next().map_or(1, char::len_utf8);
                }
                line = line[start..].to_string();
            }
            content.push(Line::from(Span::styled(format!(" > {line}"), style)));
        }
        content.push(Line::from(""));
    }
    let [body, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(4)]).areas(area);
    let title = if editor.editing_path.is_some() {
        " Edit Entry "
    } else {
        " New Entry "
    };
    let scroll = active_line.saturating_sub(body.height.saturating_sub(5));
    frame.render_widget(
        Paragraph::new(content)
            .scroll((scroll, 0))
            .block(Block::bordered().title(title)),
        body,
    );
    let (score, entropy) = generator::strength(password);
    let footer_lines = vec![
        strength_line(score, entropy),
        Line::from(Span::styled(error.unwrap_or(""), theme::error_border())),
        Line::from("Tab / Shift+Tab fields · Ctrl+g generate"),
        Line::from("←/→ edit · Ctrl+z/y undo/redo · Enter save · Esc cancel"),
    ];
    frame.render_widget(Paragraph::new(footer_lines), footer);
}

fn strength_line(score: u8, entropy: f64) -> Line<'static> {
    let filled = usize::from(score) + 1;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(5 - filled));
    let label = match score {
        0 => "Very weak",
        1 => "Weak",
        2 => "Fair",
        3 => "Strong",
        _ => "Very strong",
    };
    Line::from(vec![
        Span::styled("  Strength: ", theme::label()),
        Span::styled(bar, theme::strength(score)),
        Span::styled(format!("  {label} ({entropy:.1} bits-ish)"), theme::value()),
    ])
}

struct GeneratorView<'a> {
    name: &'a str,
    password: &'a str,
    length: usize,
    uppercase: bool,
    numbers: bool,
    symbols: bool,
    mode: GeneratorMode,
}

fn render_generator(frame: &mut Frame, view: GeneratorView<'_>) {
    let GeneratorView {
        name,
        password,
        length,
        uppercase,
        numbers,
        symbols,
        mode,
    } = view;
    let area = centered_rect(72, 17, frame.area());
    clear_popup(frame, area);

    let (score, entropy) = generator::strength(password);
    let entropy = if mode == GeneratorMode::Passphrase {
        generator::passphrase_entropy(length)
    } else {
        entropy
    };
    let mode_label = match mode {
        GeneratorMode::Password => "Password",
        GeneratorMode::Passphrase => "Diceware / Passphrase",
    };
    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Entry: ", theme::label()),
            Span::styled(
                if name.is_empty() { "(new entry)" } else { name },
                theme::value(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Mode:   ", theme::label()),
            Span::styled(format!("{mode_label}  [m] change"), theme::value()),
        ]),
        Line::from(Span::styled(
            format!("  > {password}"),
            theme::input_active(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Length: ", theme::label()),
            Span::styled(
                format!(
                    "[{length} {}]  [- / +] adjust",
                    if mode == GeneratorMode::Passphrase {
                        "words"
                    } else {
                        "characters"
                    }
                ),
                theme::value(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Sets:   ", theme::label()),
            Span::styled(format!("[{}] Uppercase  ", mark(uppercase)), theme::value()),
            Span::styled(format!("[{}] Numbers  ", mark(numbers)), theme::value()),
            Span::styled(format!("[{}] Symbols", mark(symbols)), theme::value()),
        ]),
        Line::from(Span::styled(
            "          [u]            [n]         [s]",
            theme::password_hidden(),
        )),
        Line::from(""),
        if mode == GeneratorMode::Passphrase {
            Line::from(format!("  Generation entropy: {entropy:.1} bits"))
        } else {
            strength_line(score, entropy)
        },
        Line::from(""),
        Line::from(Span::styled(
            "  [r] Regenerate  [Enter] Use password  [Esc] Back",
            theme::password_hidden(),
        )),
        Line::from(""),
    ];

    let block = Block::bordered()
        .title(" 🔐 Password Generator ")
        .title_style(theme::title())
        .border_type(BorderType::Rounded)
        .border_style(theme::popup_border());
    frame.render_widget(Paragraph::new(content).block(block), area);
}

fn mark(enabled: bool) -> &'static str {
    if enabled { "x" } else { " " }
}

// ── Notification popup ─────────────────────────────────

fn render_notification(frame: &mut Frame, message: &str, is_error: bool) {
    let width = (message.len() as u16 + 6).clamp(30, 60);
    let area = centered_rect(width, 5, frame.area());
    clear_popup(frame, area);

    let border_style = if is_error {
        theme::error_border()
    } else {
        theme::success_border()
    };
    let title = if is_error { " Error " } else { " Info " };

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(format!("  {message}"), theme::value())),
        Line::from(""),
    ];

    let block = Block::bordered()
        .title(title)
        .title_style(border_style)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}

// ── Init store popup ───────────────────────────────────

fn render_init_store(frame: &mut Frame, mode: &crate::app::InitMode) {
    match mode {
        crate::app::InitMode::SelectKey {
            keys,
            selected_index,
            custom_id,
            is_custom,
        } => {
            let height = (13 + keys.len() as u16).clamp(14, 22);
            let area = centered_rect(62, height, frame.area());
            clear_popup(frame, area);

            let mut content = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Select a GPG key to initialize your password store:",
                    theme::value(),
                )),
                Line::from(""),
            ];

            for (i, key) in keys.iter().enumerate() {
                let is_selected = !*is_custom && i == *selected_index;
                let prefix = if is_selected { "  ▸ " } else { "    " };
                let style = if is_selected {
                    theme::selected_item()
                } else {
                    theme::value()
                };
                content.push(Line::from(Span::styled(
                    format!("{prefix}[{i}] {} ({})", key.uid, key.id),
                    style,
                )));
            }

            content.push(Line::from(""));
            let custom_style = if *is_custom {
                theme::input_active()
            } else {
                theme::input_inactive()
            };
            let cursor = if *is_custom { "█" } else { "" };
            content.push(Line::from(Span::styled(
                "  Or enter custom GPG Key ID / Email:",
                custom_style,
            )));
            content.push(Line::from(Span::styled(
                format!("  > {custom_id}{cursor}"),
                custom_style,
            )));

            content.push(Line::from(""));
            content.push(Line::from(Span::styled(
                "  [j/k] Select  [Tab] Custom  [n] New Key  [Enter] Init  [Esc] Cancel",
                theme::password_hidden(),
            )));
            content.push(Line::from(""));

            let block = Block::bordered()
                .title(" 🔑 Initialize Password Store ")
                .title_style(theme::title())
                .border_type(BorderType::Rounded)
                .border_style(theme::popup_border());

            let paragraph = Paragraph::new(content).block(block);
            frame.render_widget(paragraph, area);
        }
        crate::app::InitMode::CreateKey {
            name,
            email,
            active_field,
        } => {
            let area = centered_rect(72, 17, frame.area());
            clear_popup(frame, area);

            let name_style = if *active_field == crate::app::InitField::Name {
                theme::input_active()
            } else {
                theme::input_inactive()
            };
            let email_style = if *active_field == crate::app::InitField::Email {
                theme::input_active()
            } else {
                theme::input_inactive()
            };

            let name_cursor = if *active_field == crate::app::InitField::Name {
                "█"
            } else {
                ""
            };
            let email_cursor = if *active_field == crate::app::InitField::Email {
                "█"
            } else {
                ""
            };

            let content = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  No GPG key found. Generate one to encrypt passwords:",
                    theme::value(),
                )),
                Line::from(""),
                Line::from(Span::styled("  Full Name (e.g. Alice Smith):", name_style)),
                Line::from(Span::styled(format!("  > {name}{name_cursor}"), name_style)),
                Line::from(""),
                Line::from(Span::styled(
                    "  Email (e.g. alice@example.com):",
                    email_style,
                )),
                Line::from(Span::styled(
                    format!("  > {email}{email_cursor}"),
                    email_style,
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  [Tab] Switch   [Enter] Generate & Init   [Esc] Cancel",
                    theme::password_hidden(),
                )),
                Line::from(""),
            ];

            let block = Block::bordered()
                .title(" 🔑 Generate GPG Key & Init Store ")
                .title_style(theme::title())
                .border_type(BorderType::Rounded)
                .border_style(theme::popup_border());

            let paragraph = Paragraph::new(content).block(block);
            frame.render_widget(paragraph, area);
        }
    }
}

// ── Git synchronization popup ──────────────────────────

fn render_git_sync(frame: &mut Frame, app: &App) {
    let git = &app.git_status;
    let height = if git.is_git_repo { 20 } else { 11 };
    let area = centered_rect(72, height, frame.area());
    clear_popup(frame, area);

    let mut content = vec![Line::from("")];

    if !git.is_git_repo {
        content.push(Line::from(Span::styled(
            "  Password store is not a Git repository yet.",
            theme::value(),
        )));
        content.push(Line::from(""));
        content.push(Line::from(Span::styled(
            "  Initialize Git to track encrypted revisions & sync with GitHub:",
            theme::password_hidden(),
        )));
        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::styled("  [i] ", theme::label()),
            Span::styled("Initialize Git Repo    ", theme::title()),
            Span::styled("[Esc] ", theme::label()),
            Span::styled("Close", theme::value()),
        ]));
        content.push(Line::from(""));
    } else {
        let branch = &git.branch;
        let remote_str = git.remote_url.as_deref().unwrap_or("None configured");
        let remote_name = git.remote_name.as_deref().unwrap_or("");

        content.push(Line::from(vec![
            Span::styled("  Branch:  ", theme::label()),
            Span::styled(branch, theme::value()),
        ]));

        content.push(Line::from(vec![
            Span::styled("  Remote:  ", theme::label()),
            Span::styled(
                if remote_name.is_empty() {
                    remote_str.to_string()
                } else {
                    format!("{remote_str} ({remote_name})")
                },
                if git.remote_url.is_some() {
                    theme::value()
                } else {
                    theme::password_hidden()
                },
            ),
        ]));

        let sync_status = if git.ahead > 0 && git.behind > 0 {
            Span::styled(
                format!("Ahead by {} commit(s), behind by {}", git.ahead, git.behind),
                theme::git_behind(),
            )
        } else if git.ahead > 0 {
            Span::styled(
                format!("Ahead by {} commit(s) (Ready to push)", git.ahead),
                theme::git_ahead(),
            )
        } else if git.behind > 0 {
            Span::styled(
                format!("Behind by {} commit(s) (Pull required)", git.behind),
                theme::git_behind(),
            )
        } else if git.remote_url.is_some() {
            Span::styled("Up to date with remote ✓", theme::git_synced())
        } else {
            Span::styled(
                "Local repository only (no remote configured)",
                theme::password_hidden(),
            )
        };

        content.push(Line::from(vec![
            Span::styled("  Status:  ", theme::label()),
            sync_status,
        ]));

        content.push(Line::from(""));
        content.push(Line::from(Span::styled(
            "  Recent Commits:",
            theme::label(),
        )));
        if git.recent_commits.is_empty() {
            content.push(Line::from(Span::styled(
                "    No commits yet",
                theme::password_hidden(),
            )));
        } else {
            for commit in git.recent_commits.iter().take(4) {
                if let Some((hash, msg)) = commit.split_once(' ') {
                    content.push(Line::from(vec![
                        Span::styled("    ▸ ", theme::value()),
                        Span::styled(format!("{hash:<8}"), theme::git_commit_hash()),
                        Span::styled(msg, theme::git_commit_msg()),
                    ]));
                } else {
                    content.push(Line::from(Span::styled(
                        format!("    ▸ {commit}"),
                        theme::value(),
                    )));
                }
            }
        }

        content.push(Line::from(""));
        content.push(Line::from(Span::styled(
            "  [p] Push  [u] Pull  [s] Sync  [c] Copy URL",
            theme::password_hidden(),
        )));
        content.push(Line::from(Span::styled(
            "  [h] Create GitHub Repo  [r] Set Remote  [Esc] Close",
            theme::password_hidden(),
        )));
        content.push(Line::from(""));
    }

    let block = Block::bordered()
        .title(" 🌿 Git Synchronization ")
        .title_style(theme::title())
        .border_type(BorderType::Rounded)
        .border_style(theme::popup_border());

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_github_repo(frame: &mut Frame, name: &str) {
    let area = centered_rect(66, 11, frame.area());
    clear_popup(frame, area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Create a private GitHub repository for this password store.",
            theme::value(),
        )),
        Line::from(Span::styled(
            "  If needed, gh will open your browser for GitHub login.",
            theme::password_hidden(),
        )),
        Line::from(""),
        Line::from(Span::styled("  Repository name:", theme::label())),
        Line::from(Span::styled(format!("  > {name}█"), theme::input_active())),
        Line::from(""),
        Line::from(Span::styled(
            "  [Enter] Create private repo & push   [Esc] Cancel",
            theme::password_hidden(),
        )),
        Line::from(""),
    ];

    let block = Block::bordered()
        .title(" ◉ Create GitHub Repository ")
        .title_style(theme::title())
        .border_type(BorderType::Rounded)
        .border_style(theme::popup_border());
    frame.render_widget(Paragraph::new(content).block(block), area);
}

// ── Set remote popup ───────────────────────────────────

fn render_set_remote(frame: &mut Frame, url: &str) {
    let area = centered_rect(66, 9, frame.area());
    clear_popup(frame, area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Enter Remote Git URL (e.g. git@github.com:user/passwords.git):",
            theme::value(),
        )),
        Line::from(Span::styled(format!("  > {url}█"), theme::input_active())),
        Line::from(""),
        Line::from(Span::styled(
            "  [Enter] Save Remote   [Esc] Cancel",
            theme::password_hidden(),
        )),
        Line::from(""),
    ];

    let block = Block::bordered()
        .title(" 🔗 Configure Git Remote ")
        .title_style(theme::title())
        .border_type(BorderType::Rounded)
        .border_style(theme::popup_border());

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}

fn clear_popup(frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(theme::base()), area);
}
