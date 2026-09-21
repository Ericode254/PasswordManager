use super::*;

fn app() -> App {
    App {
        running: true,
        locked: false,
        install_request: None,
        lock_outcome: None,
        last_activity: Instant::now(),
        clipboard_session: Default::default(),
        tree: vec![],
        visible: vec![],
        list_state: ListState::default(),
        store_dir: PathBuf::from("/nonexistent-passtui-test-store"),
        pass_available: true,
        is_initialized: true,
        git_status: GitStatus::not_repo(),
        config: Config::default(),
        popup: ActivePopup::None,
        input_mode: InputMode::Normal,
        search_query: String::new(),
        detail: None,
        show_password: false,
        status_message: None,
        loading: None,
        form_error: None,
        editor: crate::editor::EntryEditor::default(),
        rename_input: Default::default(),
        search_input: Default::default(),
        favorites: Default::default(),
        favorites_only: false,
        detail_scroll: 0,
        clipboard_expires: None,
        reveal_expires: None,
        generator_draft: None,
        pending_action: None,
        clipboard: ClipboardManager::new(),
    }
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn ordinary_letters_and_generator_cancel_preserve_draft() {
    let mut app = app();
    app.start_add();
    for c in "github".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Tab);
    for c in "jkg-password".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    assert!(matches!(app.popup, ActivePopup::Generator { .. }));
    press(&mut app, KeyCode::Esc);
    assert!(
        matches!(&app.popup, ActivePopup::AddEntry {name, password, ..} if name == "github" && password == "jkg-password")
    );
}

#[test]
fn validation_and_failed_save_preserve_form() {
    let mut app = app();
    app.start_add();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert!(app.form_error.is_some());
    assert!(matches!(app.popup, ActivePopup::AddEntry { .. }));
    let (sender, receiver) = mpsc::channel();
    app.pending_action = Some(receiver);
    sender
        .send(BackgroundResult::Insert(Err("GPG unavailable".into())))
        .unwrap();
    app.tick();
    assert_eq!(app.form_error.as_deref(), Some("GPG unavailable"));
    assert!(matches!(app.popup, ActivePopup::AddEntry { .. }));
}

#[test]
fn enter_cancels_delete_and_success_does_not_interrupt() {
    let mut app = app();
    app.popup = ActivePopup::Confirm {
        message: "Delete?".into(),
        entry_path: "never-delete".into(),
    };
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.popup, ActivePopup::None));
    assert!(app.pending_action.is_none());
    app.start_add();
    app.notify("Copied".into(), false);
    assert!(matches!(app.popup, ActivePopup::AddEntry { .. }));
    assert!(app.status_message.is_some());
}

#[test]
fn reveal_expires_and_escape_returns_to_list() {
    let mut app = app();
    app.detail = Some(commands::parse_entry("secret"));
    press(&mut app, KeyCode::Char('p'));
    assert!(app.show_password);
    app.reveal_expires = Some(Instant::now());
    app.tick();
    assert!(!app.show_password);
    press(&mut app, KeyCode::Esc);
    assert!(app.detail.is_none());
}

#[test]
fn search_and_navigation_clear_stale_details() {
    let mut app = app();
    app.tree = vec![StoreNode {
        name: "GitHub".into(),
        path: "Work/GitHub".into(),
        is_dir: false,
        children: vec![],
        expanded: false,
    }];
    app.rebuild_visible();
    app.detail = Some(commands::parse_entry("secret"));
    press(&mut app, KeyCode::Char('/'));
    for c in "wgh".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.visible.len(), 1);
    assert!(app.detail.is_none());
    press(&mut app, KeyCode::Esc);
    assert!(app.search_query.is_empty());
}

#[test]
fn compact_and_wide_layouts_render_without_exposing_form_password() {
    for (width, height) in [(30, 10), (79, 24), (120, 30)] {
        let mut app = app();
        app.popup = ActivePopup::AddEntry {
            name: "example".into(),
            password: "secret-marker".into(),
            active_field: AddField::Password,
        };
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| crate::ui::layout::render(frame, &mut app))
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!rendered.contains("secret-marker"));
    }
}

#[test]
fn rename_can_clear_destination_without_changing_source() {
    let mut app = app();
    app.popup = ActivePopup::Rename {
        source: "Work/github".into(),
        destination: "Work/github".into(),
    };
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    for character in "github.com/work".chars() {
        press(&mut app, KeyCode::Char(character));
    }
    assert!(
        matches!(&app.popup, ActivePopup::Rename { source, destination } if source == "Work/github" && destination == "github.com/work")
    );
    press(&mut app, KeyCode::Esc);
    assert!(app.pending_action.is_none());
}

#[test]
fn form_paste_edits_at_cursor_and_undo_is_per_field() {
    let mut app = app();
    app.start_add();
    app.handle_paste("example");
    press(&mut app, KeyCode::Tab);
    app.handle_paste("ab🔑cd");
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Left);
    app.handle_paste("XY");
    assert!(matches!(&app.popup, ActivePopup::AddEntry { password, .. } if password == "ab🔑XYcd"));
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert!(matches!(&app.popup, ActivePopup::AddEntry { password, .. } if password == "ab🔑cd"));
    app.handle_paste("bad\nusername: injected");
    assert!(app.form_error.is_some());
    assert!(app.pending_action.is_none());
    for _ in 0..3 {
        press(&mut app, KeyCode::Tab);
    }
    app.handle_paste("notes\nqdyf");
    assert_eq!(app.editor.notes, "notes\nqdyf");
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert!(app.editor.notes.is_empty());
    assert!(app.running);
    press(&mut app, KeyCode::Esc);
    app.start_add();
    press(&mut app, KeyCode::Tab);
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert!(matches!(&app.popup, ActivePopup::AddEntry { password, .. } if password.is_empty()));
}

#[test]
fn history_confirmation_requires_explicit_yes() {
    let mut app = app();
    app.popup = ActivePopup::History(Box::new(crate::history::HistoryView {
        confirming: true,
        ..Default::default()
    }));
    press(&mut app, KeyCode::Enter);
    assert!(app.pending_action.is_none());
    assert!(matches!(&app.popup, ActivePopup::History(view) if !view.confirming));
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.popup, ActivePopup::None));
}

#[test]
fn idle_lock_clears_drafts_details_and_undo_without_accepting_paste() {
    let mut app = app();
    app.start_add();
    app.handle_paste("example");
    press(&mut app, KeyCode::Tab);
    app.handle_paste("draft-secret");
    app.editor.notes = "private notes".into();
    app.detail = Some(commands::parse_entry("decrypted-secret"));
    app.generator_draft = Some(Zeroizing::new((
        "example".into(),
        "generated-secret".into(),
    )));
    app.config.behavior.idle_lock_seconds = 1;
    app.last_activity = Instant::now() - std::time::Duration::from_secs(2);
    app.handle_paste("must not revive the session");
    assert!(app.locked);
    assert!(app.clipboard_session.revoked());
    assert!(app.detail.is_none());
    assert!(app.editor.notes.is_empty());
    assert!(app.generator_draft.is_none());
    assert!(matches!(app.popup, ActivePopup::None));
    press(&mut app, KeyCode::Char('a'));
    assert!(app.locked);
    press(&mut app, KeyCode::Enter);
    assert!(!app.locked);
    assert!(!app.clipboard_session.revoked());
    app.is_initialized = true; // The fixture deliberately has no real store.
    app.start_add();
    press(&mut app, KeyCode::Tab);
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert!(matches!(&app.popup, ActivePopup::AddEntry { password, .. } if password.is_empty()));
}

#[test]
fn manual_lock_during_worker_discards_late_decryption_and_blocks_resume_until_done() {
    let mut app = app();
    app.config.behavior.idle_lock_seconds = 0;
    app.last_activity = Instant::now() - std::time::Duration::from_secs(1000);
    app.tick();
    assert!(!app.locked);
    let (sender, receiver) = mpsc::channel();
    app.pending_action = Some(receiver);
    app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    assert!(app.locked);
    press(&mut app, KeyCode::Enter);
    assert!(app.locked);
    sender
        .send(BackgroundResult::Edit(
            "example".into(),
            Ok(commands::parse_entry("late-secret")),
        ))
        .unwrap();
    app.tick();
    assert!(app.pending_action.is_none());
    assert!(app.detail.is_none());
    assert!(matches!(app.popup, ActivePopup::None));
    press(&mut app, KeyCode::Enter);
    assert!(!app.locked);
}

#[test]
fn otp_seed_and_locked_entry_names_are_not_rendered() {
    let mut app = app();
    app.detail = Some(commands::parse_entry(
        "otpauth://totp/Test?secret=SECRETSEED\notpauth://totp/Other?secret=SECONDSEED",
    ));
    app.show_password = true;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 25)).unwrap();
    terminal
        .draw(|frame| crate::ui::layout::render(frame, &mut app))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(!text.contains("SECRETSEED"));
    assert!(!text.contains("SECONDSEED"));
    app.lock_session();
    terminal
        .draw(|frame| crate::ui::layout::render(frame, &mut app))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Session cleared"));
    assert!(!text.contains("Password Store"));
}

#[test]
fn totp_shortcut_explains_empty_selection_and_folders() {
    let mut app = app();
    press(&mut app, KeyCode::Char('t'));
    assert!(
        matches!(&app.popup, ActivePopup::Notification { message, is_error: true } if message.contains("Select a password entry"))
    );
    assert!(app.pending_action.is_none());
    press(&mut app, KeyCode::Esc);
    app.visible.push(FlatEntry {
        name: "Work".into(),
        path: "Work".into(),
        is_dir: true,
        depth: 0,
        expanded: false,
        has_children: true,
    });
    app.list_state.select(Some(0));
    press(&mut app, KeyCode::Char('t'));
    assert!(
        matches!(&app.popup, ActivePopup::Notification { message, .. } if message.contains("expand"))
    );
    assert!(app.pending_action.is_none());
}

#[test]
fn recovery_codes_are_masked_and_status_change_requires_confirmation() {
    let mut app = app();
    app.popup = ActivePopup::Recovery(Box::new(crate::recovery::View::new(
        "example".into(),
        commands::parse_entry("password\nrecovery-code: recovery-secret-marker"),
    )));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 25)).unwrap();
    terminal
        .draw(|frame| crate::ui::layout::render(frame, &mut app))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(!text.contains("recovery-secret-marker"));
    press(&mut app, KeyCode::Char('p'));
    assert!(matches!(&app.popup, ActivePopup::Recovery(view) if view.revealed()));
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Enter);
    assert!(app.pending_action.is_none());
    assert!(
        matches!(&app.popup, ActivePopup::Recovery(view) if !view.confirming && !view.codes()[0].used)
    );
    app.lock_session();
    assert!(matches!(app.popup, ActivePopup::None));
}

#[test]
fn recovery_field_accepts_multiline_paste_without_exposing_codes() {
    let mut app = app();
    app.start_add();
    for _ in 0..5 {
        press(&mut app, KeyCode::Tab);
    }
    app.handle_paste("first-secret\nsecond-secret");
    assert_eq!(app.editor.recovery_codes, "first-secret\nsecond-secret");
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 25)).unwrap();
    terminal
        .draw(|frame| crate::ui::layout::render(frame, &mut app))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(!text.contains("first-secret"));
    assert!(!text.contains("second-secret"));
    app.lock_session();
    assert!(app.editor.recovery_codes.is_empty());
}

#[test]
fn otp_installation_is_requested_only_by_explicit_install_action() {
    let mut app = app();
    app.popup = ActivePopup::OtpInstall {
        path: "example".into(),
        plan: crate::otp_setup::Plan {
            program: "pacman",
            args: vec!["-S", "--needed", "--noconfirm", "oath-toolkit"],
            needs_root: true,
        },
        error: None,
    };
    press(&mut app, KeyCode::Enter);
    assert!(app.install_request.is_none());
    press(&mut app, KeyCode::Char('i'));
    assert!(app.install_request.is_some());
    app.lock_session();
    assert!(app.install_request.is_none());
}
