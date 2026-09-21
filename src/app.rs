use crate::clipboard::ClipboardManager;
use crate::config::Config;
use crate::pass::commands::{self, DecryptedEntry};
use crate::pass::generator::{self, GeneratorMode};
use crate::pass::git::{self, GitStatus};
use crate::pass::github;
use crate::pass::store::{self, FlatEntry, StoreNode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Instant;
use zeroize::{Zeroize, Zeroizing};

const STATUS_EXPIRE_SECS: u64 = 4;

// ── Input mode ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
}

// ── Popup variants ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddField {
    Name,
    Password,
    Username,
    Url,
    Notes,
    RecoveryCodes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitField {
    Name,
    Email,
}

#[derive(Debug, Clone)]
pub enum InitMode {
    SelectKey {
        keys: Vec<commands::GpgKey>,
        selected_index: usize,
        custom_id: String,
        is_custom: bool,
    },
    CreateKey {
        name: String,
        email: String,
        active_field: InitField,
    },
}

pub enum ActivePopup {
    None,
    Help,
    Recovery(Box<crate::recovery::View>),
    OtpInstall {
        path: String,
        plan: crate::otp_setup::Plan,
        error: Option<String>,
    },
    History(Box<crate::history::HistoryView>),
    Totp {
        path: String,
        code: Option<crate::totp::Code>,
        error: Option<String>,
    },
    Rename {
        source: String,
        destination: String,
    },
    Confirm {
        message: String,
        entry_path: String,
    },
    AddEntry {
        name: String,
        password: String,
        active_field: AddField,
    },
    Generator {
        name: String,
        password: String,
        length: usize,
        uppercase: bool,
        numbers: bool,
        symbols: bool,
        mode: GeneratorMode,
    },
    InitStore {
        mode: InitMode,
    },
    GitSync,
    SetRemote {
        url: String,
    },
    GithubRepo {
        name: String,
    },
    Notification {
        message: String,
        is_error: bool,
    },
}

impl Drop for ActivePopup {
    fn drop(&mut self) {
        match self {
            Self::AddEntry { password, .. } | Self::Generator { password, .. } => {
                password.zeroize()
            }
            _ => (),
        }
    }
}

pub struct LoadingState {
    pub message: String,
    pub frame: usize,
}

enum BackgroundResult {
    RecoveryOpen(String, Result<DecryptedEntry, String>),
    RecoverySave(Result<DecryptedEntry, String>),
    Totp(Result<crate::totp::Code, String>),
    TotpCopy(Result<Instant, String>),
    History(Result<Vec<crate::history::Revision>, String>),
    Preview(Result<crate::history::Preview, String>),
    Restore(String, Result<(), String>),
    View(String, Result<DecryptedEntry, String>),
    Edit(String, Result<DecryptedEntry, String>),
    Move(String, String, Result<(), String>),
    Copy(Result<(), String>),
    Insert(Result<(), String>),
    Delete(Result<(), String>),
    Initialize(Result<(), String>),
    Push(Result<String, String>),
    Pull(Result<String, String>),
    Sync(Result<String, String>),
    Github(Result<String, String>),
}

// ── App ────────────────────────────────────────────────

pub struct App {
    pub running: bool,
    pub locked: bool,
    pub install_request: Option<crate::otp_setup::Plan>,
    pub lock_outcome: Option<String>,
    last_activity: Instant,
    clipboard_session: crate::clipboard::ClipboardSession,

    // Password store
    pub tree: Vec<StoreNode>,
    pub visible: Vec<FlatEntry>,
    pub list_state: ListState,
    pub store_dir: PathBuf,
    pub pass_available: bool,
    pub is_initialized: bool,
    pub git_status: GitStatus,
    pub config: Config,

    // UI state
    pub popup: ActivePopup,
    pub input_mode: InputMode,
    pub search_query: String,
    pub detail: Option<DecryptedEntry>,
    pub show_password: bool,
    pub status_message: Option<(String, Instant)>,
    pub loading: Option<LoadingState>,
    pub form_error: Option<String>,
    pub editor: crate::editor::EntryEditor,
    pub rename_input: crate::text_input::TextInput,
    pub search_input: crate::text_input::TextInput,
    pub favorites: crate::favorites::Favorites,
    pub favorites_only: bool,
    pub detail_scroll: u16,
    pub clipboard_expires: Option<Instant>,
    reveal_expires: Option<Instant>,
    generator_draft: Option<Zeroizing<(String, String)>>,
    pending_action: Option<Receiver<BackgroundResult>>,

    // Clipboard
    pub clipboard: ClipboardManager,
}

impl App {
    pub fn new() -> Self {
        let mut config = Config::load();
        crate::ui::theme::configure(&mut config);
        let mut warning = config.warning.clone();
        let store_dir = store::get_store_dir();
        let favorites = crate::favorites::Favorites::load(&store_dir).unwrap_or_else(|error| {
            warning = Some(format!(
                "{}{}",
                warning.take().map(|w| format!("{w}\n")).unwrap_or_default(),
                error
            ));
            crate::favorites::Favorites::default()
        });
        let pass_available = commands::is_pass_available();
        let is_initialized = commands::is_store_initialized(&store_dir);
        let git_status = git::get_git_status(&store_dir);
        let tree = store::scan_store(&store_dir);

        let mut visible = Vec::new();
        store::flatten_tree(&tree, 0, &mut visible);

        let mut list_state = ListState::default();
        if !visible.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            running: true,
            locked: false,
            install_request: None,
            lock_outcome: None,
            last_activity: Instant::now(),
            clipboard_session: Default::default(),
            tree,
            visible,
            list_state,
            store_dir,
            pass_available,
            is_initialized,
            git_status,
            config,
            popup: warning.map_or(ActivePopup::None, |message| ActivePopup::Notification {
                message,
                is_error: true,
            }),
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
            favorites,
            favorites_only: false,
            detail_scroll: 0,
            clipboard_expires: None,
            reveal_expires: None,
            generator_draft: None,
            pending_action: None,
            clipboard: ClipboardManager::new(),
        }
    }

    // ── Helpers ────────────────────────────────────────

    fn open_totp(&mut self, path: String) {
        if !crate::otp_setup::available() {
            match crate::otp_setup::detect() {
                Ok(plan) => {
                    self.popup = ActivePopup::OtpInstall {
                        path,
                        plan,
                        error: None,
                    }
                }
                Err(error) => self.notify(error, true),
            }
            return;
        }
        self.popup = ActivePopup::Totp {
            path: path.clone(),
            code: None,
            error: None,
        };
        self.start_background_action("Generating authentication code…", move || {
            BackgroundResult::Totp(crate::totp::generate(&path))
        });
    }

    pub fn finish_otp_install(&mut self, result: Result<(), String>) {
        self.check_idle();
        if self.locked {
            self.lock_outcome = Some(
                if result.is_ok() {
                    "OTP support installed. Resume and press t."
                } else {
                    "OTP installation failed. Resume and press t to retry."
                }
                .into(),
            );
            return;
        }
        if let ActivePopup::OtpInstall { path, error, .. } = &mut self.popup {
            match result {
                Ok(()) => {
                    let path = path.clone();
                    self.open_totp(path);
                }
                Err(message) => *error = Some(message),
            }
        }
    }

    fn handle_recovery_key(&mut self, key: KeyEvent) {
        let ActivePopup::Recovery(view) = &mut self.popup else {
            return;
        };
        if view.confirming {
            view.confirming = false;
            if key.code == KeyCode::Char('y') {
                let path = view.path.clone();
                let original = Zeroizing::new(view.entry.content.clone());
                let selected = view.selected;
                view.reveal_until = None;
                self.start_background_action("Saving recovery-code status…", move || {
                    BackgroundResult::RecoverySave(crate::recovery::save_toggle(
                        &path, &original, selected,
                    ))
                });
            }
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.popup = ActivePopup::None,
            KeyCode::Up | KeyCode::Char('k') => {
                view.selected = view.selected.saturating_sub(1);
                view.reveal_until = None;
                view.error = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                view.selected = (view.selected + 1).min(view.codes().len().saturating_sub(1));
                view.reveal_until = None;
                view.error = None;
            }
            KeyCode::Char('p') => {
                view.reveal_until = if view.revealed() {
                    None
                } else {
                    Some(Instant::now() + std::time::Duration::from_secs(15))
                }
            }
            KeyCode::Char('x') if !view.codes().is_empty() => view.confirming = true,
            KeyCode::Char('y') => {
                if let Some(code) = view.codes().get(view.selected) {
                    if code.used {
                        view.error = Some("This code is marked used. Press x to mark it available if that was a mistake.".into());
                        return;
                    }
                    let text = Zeroizing::new(code.value.to_string());
                    let seconds = self.config.behavior.auto_clear_clipboard_seconds;
                    let session = self.clipboard_session.clone();
                    self.start_background_action("Copying recovery code…", move || {
                        BackgroundResult::Copy(ClipboardManager::new().copy_for_session(
                            &text,
                            std::time::Duration::from_secs(seconds),
                            &session,
                        ))
                    });
                }
            }
            _ => (),
        }
    }

    fn check_idle(&mut self) {
        let seconds = self.config.behavior.idle_lock_seconds;
        if !self.locked && seconds > 0 && self.last_activity.elapsed().as_secs() >= seconds {
            self.lock_session();
        }
    }

    fn lock_session(&mut self) {
        self.locked = true;
        self.install_request = None;
        self.clipboard_session.revoke();
        self.clear_detail();
        self.popup = ActivePopup::None;
        self.editor = Default::default();
        self.rename_input = Default::default();
        self.search_input = Default::default();
        self.search_query.zeroize();
        self.input_mode = InputMode::Normal;
        self.favorites_only = false;
        self.form_error.zeroize();
        self.status_message = None;
        if let Some(draft) = &mut self.generator_draft {
            draft.zeroize();
        }
        self.generator_draft = None;
        self.clipboard_expires = None;
    }

    pub fn selected_entry(&self) -> Option<&FlatEntry> {
        self.list_state.selected().and_then(|i| self.visible.get(i))
    }

    #[allow(dead_code)]
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    fn notify(&mut self, message: String, is_error: bool) {
        if !is_error {
            self.set_status(message);
            return;
        }
        self.popup = ActivePopup::Notification { message, is_error };
    }

    pub fn refresh_tree(&mut self) {
        let selected = self.selected_entry().map(|entry| entry.path.clone());
        let collapsed = store::collapsed_paths(&self.tree);
        self.clear_detail();
        self.is_initialized = commands::is_store_initialized(&self.store_dir);
        self.git_status = git::get_git_status(&self.store_dir);
        self.tree = store::scan_store(&self.store_dir);
        for path in collapsed {
            store::toggle_node(&mut self.tree, &path);
        }
        self.rebuild_visible();
        if let Some(index) = self
            .visible
            .iter()
            .position(|entry| Some(&entry.path) == selected.as_ref())
        {
            self.list_state.select(Some(index));
        }
    }

    fn focus_path(&mut self, path: &str) {
        if !self.favorites.contains(path) {
            self.favorites_only = false;
        }
        self.search_query.clear();
        self.search_input = Default::default();
        store::reveal_path(&mut self.tree, path);
        self.rebuild_visible();
        self.list_state
            .select(self.visible.iter().position(|entry| entry.path == path));
    }

    fn rebuild_visible(&mut self) {
        self.visible.clear();

        if self.search_query.is_empty() && !self.favorites_only {
            store::flatten_tree(&self.tree, 0, &mut self.visible);
        } else {
            // Search the whole store, including collapsed directories, so
            // matches aren't hidden by unrelated expand/collapse state.
            store::flatten_tree_all(&self.tree, 0, &mut self.visible);
            let q = self.search_query.to_lowercase();
            self.visible.retain(|e| {
                !e.is_dir
                    && crate::search::matches(&e.path, &q)
                    && (!self.favorites_only || self.favorites.contains(&e.path))
            });
            self.visible
                .sort_by_key(|entry| crate::search::rank(&entry.path, &q));
        }

        // Fix selection bounds
        if self.visible.is_empty() {
            self.list_state.select(None);
        } else {
            match self.list_state.selected() {
                Some(i) if i >= self.visible.len() => {
                    self.list_state.select(Some(self.visible.len() - 1));
                }
                None => self.list_state.select(Some(0)),
                _ => {}
            }
        }
    }

    // ── Tick (periodic updates) ────────────────────────

    pub fn tick(&mut self) {
        self.check_idle();
        if let ActivePopup::Totp { code, .. } = &mut self.popup
            && code.as_ref().is_some_and(|code| code.remaining().is_zero())
        {
            *code = None;
        }

        if self
            .reveal_expires
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.show_password = false;
            self.reveal_expires = None;
        }
        if let Some(loading) = &mut self.loading {
            loading.frame = loading.frame.wrapping_add(1);
        }
        self.finish_background_action();

        if let Some((_, ts)) = &self.status_message
            && ts.elapsed().as_secs() >= STATUS_EXPIRE_SECS
        {
            self.status_message = None;
        }
    }

    fn start_background_action<F>(&mut self, message: &str, action: F)
    where
        F: FnOnce() -> BackgroundResult + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        self.loading = Some(LoadingState {
            message: message.to_string(),
            frame: 0,
        });
        self.pending_action = Some(receiver);
        thread::spawn(move || {
            let _ = sender.send(action());
        });
    }

    fn finish_background_action(&mut self) {
        let result = match self
            .pending_action
            .as_ref()
            .map(|receiver| receiver.try_recv())
        {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Empty)) | None => None,
            Some(Err(TryRecvError::Disconnected)) => Some(BackgroundResult::Github(Err(
                "Background action stopped unexpectedly".into(),
            ))),
        };

        let Some(result) = result else { return };
        self.pending_action = None;
        self.loading = None;

        if self.locked {
            let failed = match &result {
                BackgroundResult::Move(source, path, Ok(())) => {
                    self.favorites.rename(source, path).is_err()
                }
                BackgroundResult::Totp(result) => result.is_err(),
                BackgroundResult::TotpCopy(result) => result.is_err(),
                BackgroundResult::Copy(result)
                | BackgroundResult::Insert(result)
                | BackgroundResult::Delete(result)
                | BackgroundResult::Initialize(result)
                | BackgroundResult::Move(_, _, result)
                | BackgroundResult::Restore(_, result) => result.is_err(),
                BackgroundResult::RecoveryOpen(_, result)
                | BackgroundResult::RecoverySave(result)
                | BackgroundResult::View(_, result)
                | BackgroundResult::Edit(_, result) => result.is_err(),
                BackgroundResult::History(result) => result.is_err(),
                BackgroundResult::Preview(result) => result.is_err(),
                BackgroundResult::Push(result)
                | BackgroundResult::Pull(result)
                | BackgroundResult::Sync(result)
                | BackgroundResult::Github(result) => result.is_err(),
            };
            self.lock_outcome = Some(if failed { "Background action failed or was cancelled. Resume and check the store before retrying." } else { "Background action finished. Resume to refresh the store." }.into());
            return;
        }
        match result {
            BackgroundResult::RecoveryOpen(path, result) => match result {
                Ok(entry) => {
                    self.popup =
                        ActivePopup::Recovery(Box::new(crate::recovery::View::new(path, entry)))
                }
                Err(error) => self.notify(error, true),
            },
            BackgroundResult::RecoverySave(result) => {
                if let ActivePopup::Recovery(view) = &mut self.popup {
                    match result {
                        Ok(entry) => {
                            view.entry = entry;
                            view.error = None;
                            self.set_status("Recovery-code status saved");
                        }
                        Err(error) => view.error = Some(error),
                    }
                }
            }
            BackgroundResult::Totp(result) => {
                if let ActivePopup::Totp { code, error, .. } = &mut self.popup {
                    match result {
                        Ok(value) => {
                            *code = Some(value);
                            *error = None;
                        }
                        Err(message) => *error = Some(message),
                    }
                }
            }
            BackgroundResult::TotpCopy(result) => match result {
                Ok(deadline) => {
                    self.clipboard_expires = Some(deadline);
                    self.set_status("Authentication code copied; clipboard expires with the code")
                }
                Err(message) => {
                    if let ActivePopup::Totp { error, .. } = &mut self.popup {
                        *error = Some(message);
                    }
                }
            },
            BackgroundResult::History(result) => match result {
                Ok(revisions) => {
                    self.popup = ActivePopup::History(Box::new(crate::history::HistoryView {
                        revisions,
                        ..Default::default()
                    }))
                }
                Err(error) => self.notify(error, true),
            },
            BackgroundResult::Preview(result) => {
                if let ActivePopup::History(view) = &mut self.popup {
                    match result {
                        Ok(preview) => {
                            view.preview = Some(preview);
                            view.error = None;
                        }
                        Err(error) => view.error = Some(error),
                    }
                }
            }
            BackgroundResult::Restore(path, result) => match result {
                Ok(()) => {
                    self.popup = ActivePopup::None;
                    self.refresh_tree();
                    self.favorites_only = false;
                    self.focus_path(&path);
                    self.set_status(format!("Restored {path} as a new save"));
                }
                Err(error) => {
                    if let ActivePopup::History(view) = &mut self.popup {
                        view.error = Some(error);
                    }
                }
            },
            BackgroundResult::Edit(path, result) => match result {
                Ok(entry) => {
                    self.form_error = None;
                    self.editor = crate::editor::EntryEditor::from_entry(path.clone(), &entry);
                    self.popup = ActivePopup::AddEntry {
                        name: path,
                        password: entry.password.clone(),
                        active_field: AddField::Password,
                    };
                }
                Err(error) => self.notify(error, true),
            },
            BackgroundResult::Move(source, path, result) => match result {
                Ok(()) => {
                    let favorite_result = self.favorites.rename(&source, &path);
                    self.popup = ActivePopup::None;
                    self.rename_input = Default::default();
                    self.clear_detail();
                    self.refresh_tree();
                    self.focus_path(&path);
                    self.set_status(format!("Moved to {path}"));
                    if let Err(error) = favorite_result {
                        self.notify(
                            format!("Entry moved, but favorites could not be updated: {error}"),
                            true,
                        );
                    }
                }
                Err(error) => self.form_error = Some(error),
            },
            BackgroundResult::View(path, result) => {
                if self
                    .selected_entry()
                    .is_some_and(|entry| entry.path == path)
                {
                    match result {
                        Ok(entry) => {
                            self.detail = Some(entry);
                            self.show_password = self.config.behavior.default_reveal_passwords;
                            self.reveal_expires = self
                                .show_password
                                .then(|| Instant::now() + std::time::Duration::from_secs(15));
                        }
                        Err(error) => self.notify(error, true),
                    }
                }
            }
            BackgroundResult::Initialize(result) => match result {
                Ok(()) => {
                    self.popup = ActivePopup::None;
                    self.refresh_tree();
                    self.set_status("Store initialized. Press a to add your first entry.");
                }
                Err(error) => self.notify(error, true),
            },
            BackgroundResult::Delete(result) => match result {
                Ok(()) => {
                    self.clear_detail();
                    self.refresh_tree();
                    self.set_status("Entry deleted");
                }
                Err(error) => self.notify(error, true),
            },
            BackgroundResult::Copy(result) => match result {
                Ok(()) => {
                    self.clipboard_expires = Some(
                        Instant::now()
                            + std::time::Duration::from_secs(
                                self.config.behavior.auto_clear_clipboard_seconds.max(1),
                            ),
                    );
                    self.set_status("Copied to clipboard");
                }
                Err(error) => self.notify(error, true),
            },
            BackgroundResult::Insert(result) => match result {
                Ok(()) => {
                    let saved = match &self.popup {
                        ActivePopup::AddEntry { name, .. } => Some(name.trim().to_string()),
                        _ => None,
                    };
                    self.popup = ActivePopup::None;
                    self.refresh_tree();
                    if let Some(path) = saved {
                        self.focus_path(&path);
                    }
                    self.editor = crate::editor::EntryEditor::default();
                    self.generator_draft = None;
                    self.set_status("Entry saved");
                }
                Err(error) => self.form_error = Some(error),
            },
            BackgroundResult::Push(Ok(message)) => {
                self.git_status = git::get_git_status(&self.store_dir);
                self.notify(format!("✓ {message}"), false);
            }
            BackgroundResult::Push(Err(error)) => self.notify(format!("✗ {error}"), true),
            BackgroundResult::Pull(Ok(message)) => {
                self.refresh_tree();
                self.notify(format!("✓ {message}"), false);
            }
            BackgroundResult::Pull(Err(error)) => self.notify(format!("✗ {error}"), true),
            BackgroundResult::Sync(Ok(message)) => {
                self.git_status = git::get_git_status(&self.store_dir);
                self.notify(format!("✓ {message}"), false);
            }
            BackgroundResult::Sync(Err(error)) => self.notify(format!("✗ {error}"), true),
            BackgroundResult::Github(Ok(message)) => {
                self.refresh_tree();
                self.notify(format!("✓ {message}"), false);
            }
            BackgroundResult::Github(Err(error)) => self.notify(format!("✗ {error}"), true),
        }
    }

    // ── Actions ────────────────────────────────────────

    fn toggle_selected_dir(&mut self) {
        if let Some(entry) = self.selected_entry()
            && entry.is_dir
        {
            let path = entry.path.clone();
            store::toggle_node(&mut self.tree, &path);
            self.rebuild_visible();
        }
    }

    fn view_selected_entry(&mut self) {
        if let Some(entry) = self.selected_entry() {
            if entry.is_dir {
                self.toggle_selected_dir();
            } else {
                let path = entry.path.clone();
                self.start_background_action("Decrypting entry…", move || {
                    let result = commands::pass_show(&path);
                    BackgroundResult::View(path, result)
                });
            }
        }
    }

    fn copy_password(&mut self) {
        let path = match self.selected_entry() {
            Some(e) if !e.is_dir => e.path.clone(),
            _ => return,
        };
        let seconds = self.config.behavior.auto_clear_clipboard_seconds;
        let session = self.clipboard_session.clone();
        self.start_background_action("Copying password…", move || {
            BackgroundResult::Copy(commands::pass_show(&path).and_then(|entry| {
                ClipboardManager::new().copy_for_session(
                    entry.login_password()?,
                    std::time::Duration::from_secs(seconds),
                    &session,
                )
            }))
        });
    }

    fn start_delete(&mut self) {
        if let Some(entry) = self.selected_entry()
            && !entry.is_dir
        {
            let name = entry.path.clone();
            let path = entry.path.clone();
            self.popup = ActivePopup::Confirm {
                message: format!("Delete '{name}'?"),
                entry_path: path,
            };
        }
    }

    fn confirm_delete(&mut self, path: &str) {
        let path = path.to_string();
        self.popup = ActivePopup::None;
        self.start_background_action("Deleting entry…", move || {
            BackgroundResult::Delete(commands::pass_remove(&path))
        });
    }

    pub fn start_init_store(&mut self) {
        let keys = commands::list_gpg_keys();
        if keys.is_empty() {
            self.popup = ActivePopup::InitStore {
                mode: InitMode::CreateKey {
                    name: String::new(),
                    email: String::new(),
                    active_field: InitField::Name,
                },
            };
        } else {
            self.popup = ActivePopup::InitStore {
                mode: InitMode::SelectKey {
                    keys,
                    selected_index: 0,
                    custom_id: String::new(),
                    is_custom: false,
                },
            };
        }
    }

    fn start_add(&mut self) {
        self.form_error = None;
        self.editor = crate::editor::EntryEditor::default();
        if !self.is_initialized {
            self.start_init_store();
            return;
        }
        self.popup = ActivePopup::AddEntry {
            name: String::new(),
            password: String::new(),
            active_field: AddField::Name,
        };
    }

    fn open_generator(&mut self, name: String) {
        self.popup = ActivePopup::Generator {
            name,
            password: generator::generate_password(16, true, true, true),
            length: 16,
            uppercase: true,
            numbers: true,
            symbols: true,
            mode: GeneratorMode::Password,
        };
    }

    fn regenerate_password(
        password: &mut String,
        length: usize,
        uppercase: bool,
        numbers: bool,
        symbols: bool,
        mode: GeneratorMode,
    ) {
        password.zeroize();
        *password = match mode {
            GeneratorMode::Password => {
                generator::generate_password(length, uppercase, numbers, symbols)
            }
            GeneratorMode::Passphrase => generator::generate_passphrase(length),
        };
    }

    fn submit_add(&mut self, name: String, password: String) {
        let password = Zeroizing::new(password);
        self.form_error = None;
        if name.trim().is_empty() || password.is_empty() {
            self.form_error = Some("Name and password are required".into());
            return;
        }
        if !crate::editor::valid_path(name.trim()) {
            self.form_error = Some("Use a relative entry path, such as Work/github.".into());
            return;
        }
        let overwrite = self.editor.editing_path.is_some();
        if let Some(original) = &self.editor.editing_path {
            if original != name.trim() {
                self.form_error =
                    Some("Use r from the entry list to rename or move an entry.".into());
                return;
            }
        } else if self.store_dir.join(format!("{}.gpg", name.trim())).exists() {
            self.form_error = Some("This entry already exists. Choose a different name.".into());
            return;
        }
        let content = Zeroizing::new(self.editor.content(&password));
        let original = self.editor.original_content.clone().map(Zeroizing::new);
        let path = name.trim().to_string();
        self.start_background_action("Saving entry…", move || {
            if let Some(original) = original {
                match commands::pass_show(&path) {
                    Ok(current) if current.content == *original => {}
                    Ok(_) => return BackgroundResult::Insert(Err(
                        "Entry changed since editing began. Cancel and reopen it before saving."
                            .into(),
                    )),
                    Err(error) => return BackgroundResult::Insert(Err(error)),
                }
            }
            BackgroundResult::Insert(commands::pass_insert(&path, &content, overwrite))
        });
    }

    pub fn open_git_sync(&mut self) {
        self.git_status = git::get_git_status(&self.store_dir);
        self.popup = ActivePopup::GitSync;
    }

    pub fn trigger_git_push(&mut self) {
        if self.pending_action.is_some() {
            return;
        }
        if !self.git_status.is_git_repo {
            self.notify(
                "✗ Not a git repository. Press G then [i] to init".into(),
                true,
            );
            return;
        }
        let branch = self.git_status.branch.clone();
        self.start_background_action("Pushing encrypted passwords...", move || {
            BackgroundResult::Push(git::pass_git_push(&branch))
        });
    }

    pub fn trigger_git_pull(&mut self) {
        if self.pending_action.is_some() {
            return;
        }
        if !self.git_status.is_git_repo {
            self.notify(
                "✗ Not a git repository. Press G then [i] to init".into(),
                true,
            );
            return;
        }
        self.start_background_action("Pulling encrypted passwords...", || {
            BackgroundResult::Pull(git::pass_git_pull())
        });
    }

    fn trigger_git_sync(&mut self) {
        if self.pending_action.is_some() {
            return;
        }
        if !self.git_status.is_git_repo {
            self.notify(
                "✗ Not a git repository. Press G then [i] to init".into(),
                true,
            );
            return;
        }
        let branch = self.git_status.branch.clone();
        self.start_background_action("Syncing encrypted passwords...", move || {
            let result = git::pass_git_pull().and_then(|pull_message| {
                git::pass_git_push(&branch)
                    .map(|push_message| format!("Pull: {pull_message}; Push: {push_message}"))
            });
            BackgroundResult::Sync(result)
        });
    }

    pub fn trigger_git_init(&mut self) {
        match git::pass_git_init() {
            Ok(()) => {
                self.notify(
                    "✓ Git repository initialized in password store!".into(),
                    false,
                );
                self.refresh_tree();
            }
            Err(e) => {
                self.notify(format!("✗ {e}"), true);
            }
        }
    }

    pub fn open_github_repo(&mut self) {
        if !github::is_available() {
            self.notify("✗ GitHub CLI (gh) is not installed".into(), true);
            return;
        }
        self.popup = ActivePopup::GithubRepo {
            name: "passwords".to_string(),
        };
    }

    fn create_github_repo(&mut self, name: &str) {
        if self.pending_action.is_some() {
            return;
        }
        if !self.git_status.is_git_repo {
            if let Err(error) = git::pass_git_init() {
                self.notify(format!("✗ Could not initialize Git: {error}"), true);
                return;
            }
            self.refresh_tree();
        }

        let store_dir = self.store_dir.clone();
        let name = name.to_string();
        self.start_background_action("Checking GitHub login...", move || {
            let result = match github::is_authenticated() {
                Ok(true) => Ok(()),
                Ok(false) => github::login().map(|_| ()),
                Err(error) => Err(format!("Could not check GitHub login: {error}")),
            };
            BackgroundResult::Github(
                result.and_then(|()| github::create_private_repo(&store_dir, &name)),
            )
        });
    }

    fn copy_remote_url(&mut self) {
        let Some(url) = self.git_status.remote_url.clone() else {
            self.notify("✗ No remote repository URL configured".into(), true);
            return;
        };
        match self.clipboard.copy_for_session(
            &url,
            std::time::Duration::from_secs(45),
            &self.clipboard_session,
        ) {
            Ok(()) => self.notify("✓ Repository URL copied".into(), false),
            Err(error) => self.notify(format!("✗ {error}"), true),
        }
    }

    // ── Key dispatch ───────────────────────────────────

    pub fn handle_paste(&mut self, value: &str) {
        self.check_idle();
        if self.locked {
            return;
        }
        self.last_activity = Instant::now();
        if self.pending_action.is_some() {
            return;
        }
        let result = match &mut self.popup {
            ActivePopup::AddEntry {
                name,
                password,
                active_field,
            } => {
                let (index, text) = match active_field {
                    AddField::Name => (0, name),
                    AddField::Password => (1, password),
                    AddField::Username => (2, &mut self.editor.username),
                    AddField::Url => (3, &mut self.editor.url),
                    AddField::Notes => (4, &mut self.editor.notes),
                    AddField::RecoveryCodes => (5, &mut self.editor.recovery_codes),
                };
                self.editor.inputs[index].insert(text, value, index >= 4)
            }
            ActivePopup::Rename { destination, .. } => {
                self.rename_input.insert(destination, value, false)
            }
            ActivePopup::SetRemote { url } => {
                crate::text_input::TextInput::default().insert(url, value, false)
            }
            ActivePopup::GithubRepo { name } => {
                crate::text_input::TextInput::default().insert(name, value, false)
            }
            ActivePopup::InitStore { mode } => match mode {
                InitMode::SelectKey {
                    custom_id,
                    is_custom: true,
                    ..
                } => crate::text_input::TextInput::default().insert(custom_id, value, false),
                InitMode::CreateKey {
                    name,
                    email,
                    active_field,
                } => crate::text_input::TextInput::default().insert(
                    match active_field {
                        InitField::Name => name,
                        InitField::Email => email,
                    },
                    value,
                    false,
                ),
                _ => return,
            },
            ActivePopup::None if self.input_mode == InputMode::Search => {
                let result = self
                    .search_input
                    .insert(&mut self.search_query, value, false);
                self.clear_detail();
                self.list_state.select(Some(0));
                self.rebuild_visible();
                if let Err(error) = result {
                    self.set_status(error);
                }
                return;
            }
            _ => return,
        };
        self.form_error = result.err();
        if let Some(error) = self.form_error.clone() {
            self.set_status(error);
        }
    }

    fn handle_history_key(&mut self, key: KeyEvent) {
        let ActivePopup::History(view) = &mut self.popup else {
            return;
        };
        if view.confirming {
            view.confirming = false;
            if key.code == KeyCode::Char('y')
                && let Some(preview) = view.preview.take()
            {
                let store = self.store_dir.clone();
                let path = preview.revision.path.clone();
                self.start_background_action("Restoring entry…", move || {
                    BackgroundResult::Restore(path, crate::history::restore(&store, preview))
                });
            }
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.popup = ActivePopup::None,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
                view.selected = if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                    view.selected.saturating_sub(1)
                } else {
                    (view.selected + 1).min(view.revisions.len().saturating_sub(1))
                };
                view.preview = None;
                view.error = None;
            }
            KeyCode::Enter => {
                if let Some(revision) = view.revisions.get(view.selected).cloned() {
                    view.preview = None;
                    view.error = None;
                    let store = self.store_dir.clone();
                    self.start_background_action("Decrypting historical entry…", move || {
                        BackgroundResult::Preview(crate::history::preview(&store, revision))
                    });
                }
            }
            KeyCode::Char('r') if view.preview.is_some() => view.confirming = true,
            _ => (),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.check_idle();
        if self.locked {
            match key.code {
                KeyCode::Char('q') if self.pending_action.is_none() => self.running = false,
                KeyCode::Enter if self.pending_action.is_none() => {
                    self.locked = false;
                    self.last_activity = Instant::now();
                    self.clipboard_session = Default::default();
                    self.refresh_tree();
                    if let Some(outcome) = self.lock_outcome.take() {
                        self.set_status(outcome);
                    }
                }
                _ => (),
            }
            return;
        }
        self.last_activity = Instant::now();
        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.lock_session();
            return;
        }
        if self.pending_action.is_some() {
            if matches!(self.popup, ActivePopup::GitSync) && key.code == KeyCode::Esc {
                self.popup = ActivePopup::None;
                return;
            }
            if self.input_mode == InputMode::Normal && matches!(self.popup, ActivePopup::None) {
                match key.code {
                    KeyCode::Up => {
                        self.list_state.select_previous();
                        self.clear_detail();
                    }
                    KeyCode::Down => {
                        self.list_state.select_next();
                        self.clear_detail();
                    }
                    KeyCode::Char('q') => self.running = false,
                    _ => {}
                }
            }
            return;
        }
        if let ActivePopup::OtpInstall { plan, .. } = &self.popup {
            match key.code {
                KeyCode::Char('i') => self.install_request = Some(plan.clone()),
                KeyCode::Esc | KeyCode::Char('q') => self.popup = ActivePopup::None,
                _ => (),
            }
            return;
        }
        if matches!(self.popup, ActivePopup::Recovery(_)) {
            self.handle_recovery_key(key);
            return;
        }
        if let ActivePopup::Totp { path, code, error } = &mut self.popup {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.popup = ActivePopup::None,
                KeyCode::Char('r') => {
                    let path = path.clone();
                    *code = None;
                    *error = None;
                    self.start_background_action("Generating authentication code…", move || {
                        BackgroundResult::Totp(crate::totp::generate(&path))
                    });
                }
                KeyCode::Char('y') => {
                    if let Some(value) = code.as_ref().filter(|c| c.remaining().as_millis() > 1000)
                    {
                        let text = value.value.clone();
                        let deadline = Instant::now()
                            + value.remaining().min(std::time::Duration::from_secs(
                                self.config.behavior.auto_clear_clipboard_seconds,
                            ));
                        let session = self.clipboard_session.clone();
                        self.start_background_action("Copying authentication code…", move || {
                            let remaining = deadline.saturating_duration_since(Instant::now());
                            BackgroundResult::TotpCopy(if remaining.as_millis() < 500 {
                                Err("Code expired; press r to refresh".into())
                            } else {
                                ClipboardManager::new()
                                    .copy_for_session(&text, remaining, &session)
                                    .map(|()| deadline)
                            })
                        });
                    } else {
                        *error =
                            Some("Code is expired or nearly expired. Press r to refresh.".into());
                    }
                }
                _ => (),
            }
            return;
        }
        if let ActivePopup::Rename {
            source,
            destination,
        } = &mut self.popup
        {
            match key.code {
                KeyCode::Esc => {
                    self.popup = ActivePopup::None;
                    self.rename_input = Default::default();
                }
                KeyCode::Enter => {
                    let source = source.clone();
                    let destination = destination.trim().to_string();
                    if !crate::editor::valid_path(&destination) {
                        self.form_error = Some("Use a relative path, such as Work/github.".into());
                    } else if self.store_dir.join(format!("{destination}.gpg")).exists()
                        || self.store_dir.join(&destination).exists()
                    {
                        self.form_error =
                            Some("The destination already exists. Choose another path.".into());
                    } else {
                        self.start_background_action("Moving entry…", move || {
                            let result = commands::pass_move(&source, &destination);
                            BackgroundResult::Move(source, destination, result)
                        });
                    }
                }
                _ => {
                    self.rename_input.handle(destination, key);
                }
            }
            return;
        }
        if matches!(self.popup, ActivePopup::History(_)) {
            self.handle_history_key(key);
            return;
        }
        // ── Popups take priority ───────────────────────
        match &mut self.popup {
            ActivePopup::Help => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')
                ) {
                    self.popup = ActivePopup::None;
                }
                return;
            }
            ActivePopup::Confirm { entry_path, .. } => {
                let path = entry_path.clone();
                match key.code {
                    KeyCode::Char('y') => self.confirm_delete(&path),
                    KeyCode::Char('n') | KeyCode::Esc | KeyCode::Enter => {
                        self.popup = ActivePopup::None
                    }
                    _ => {}
                }
                return;
            }
            ActivePopup::AddEntry {
                name,
                password,
                active_field,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.popup = ActivePopup::None;
                        self.editor = crate::editor::EntryEditor::default();
                        self.generator_draft = None;
                        self.form_error = None;
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        let fields = [
                            AddField::Name,
                            AddField::Password,
                            AddField::Username,
                            AddField::Url,
                            AddField::Notes,
                            AddField::RecoveryCodes,
                        ];
                        let index = fields
                            .iter()
                            .position(|field| field == active_field)
                            .unwrap_or(0);
                        let next = if key.code == KeyCode::BackTab {
                            (index + fields.len() - 1) % fields.len()
                        } else {
                            (index + 1) % fields.len()
                        };
                        *active_field = fields[next].clone();
                    }
                    KeyCode::Enter
                        if matches!(*active_field, AddField::Notes | AddField::RecoveryCodes)
                            && key.modifiers.contains(KeyModifiers::ALT) =>
                    {
                        let (index, text) = if *active_field == AddField::Notes {
                            (4, &mut self.editor.notes)
                        } else {
                            (5, &mut self.editor.recovery_codes)
                        };
                        let _ = self.editor.inputs[index].insert(text, "\n", true);
                    }
                    KeyCode::Enter => {
                        if *active_field != AddField::Name {
                            let n = name.clone();
                            let p = password.clone();
                            self.submit_add(n, p);
                        } else {
                            *active_field = AddField::Password;
                        }
                    }
                    KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let n = name.clone();
                        self.generator_draft = Some(Zeroizing::new((n.clone(), password.clone())));
                        self.open_generator(n);
                    }
                    _ => {
                        let index = match active_field {
                            AddField::Name => 0,
                            AddField::Password => 1,
                            AddField::Username => 2,
                            AddField::Url => 3,
                            AddField::Notes => 4,
                            AddField::RecoveryCodes => 5,
                        };
                        let text = match active_field {
                            AddField::Name => name,
                            AddField::Password => password,
                            AddField::Username => &mut self.editor.username,
                            AddField::Url => &mut self.editor.url,
                            AddField::Notes => &mut self.editor.notes,
                            AddField::RecoveryCodes => &mut self.editor.recovery_codes,
                        };
                        self.editor.inputs[index].handle(text, key);
                    }
                }
                return;
            }
            ActivePopup::Generator {
                name,
                password,
                length,
                uppercase,
                numbers,
                symbols,
                mode,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        let (n, p) = self
                            .generator_draft
                            .take()
                            .map(|draft| (draft.0.clone(), draft.1.clone()))
                            .unwrap_or_else(|| (name.clone(), String::new()));
                        self.popup = ActivePopup::AddEntry {
                            name: n,
                            password: p,
                            active_field: AddField::Password,
                        };
                    }
                    KeyCode::Enter => {
                        self.generator_draft = None;
                        self.editor.inputs[1] = Default::default();
                        let n = name.clone();
                        let p = password.clone();
                        self.popup = ActivePopup::AddEntry {
                            name: n,
                            password: p,
                            active_field: AddField::Password,
                        };
                    }
                    KeyCode::Char('m') => {
                        *mode = match mode {
                            GeneratorMode::Password => GeneratorMode::Passphrase,
                            GeneratorMode::Passphrase => GeneratorMode::Password,
                        };
                        *length = if *mode == GeneratorMode::Passphrase {
                            6
                        } else {
                            16
                        };
                        Self::regenerate_password(
                            password, *length, *uppercase, *numbers, *symbols, *mode,
                        );
                    }
                    KeyCode::Char('u') if *mode == GeneratorMode::Password => {
                        *uppercase = !*uppercase;
                        Self::regenerate_password(
                            password, *length, *uppercase, *numbers, *symbols, *mode,
                        );
                    }
                    KeyCode::Char('n') if *mode == GeneratorMode::Password => {
                        *numbers = !*numbers;
                        Self::regenerate_password(
                            password, *length, *uppercase, *numbers, *symbols, *mode,
                        );
                    }
                    KeyCode::Char('s') if *mode == GeneratorMode::Password => {
                        *symbols = !*symbols;
                        Self::regenerate_password(
                            password, *length, *uppercase, *numbers, *symbols, *mode,
                        );
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        *length = (*length + 1).min(64);
                        Self::regenerate_password(
                            password, *length, *uppercase, *numbers, *symbols, *mode,
                        );
                    }
                    KeyCode::Char('-') => {
                        *length =
                            length
                                .saturating_sub(1)
                                .max(if *mode == GeneratorMode::Passphrase {
                                    6
                                } else {
                                    8
                                });
                        Self::regenerate_password(
                            password, *length, *uppercase, *numbers, *symbols, *mode,
                        );
                    }
                    KeyCode::Char('r') => Self::regenerate_password(
                        password, *length, *uppercase, *numbers, *symbols, *mode,
                    ),
                    _ => {}
                }
                return;
            }
            ActivePopup::InitStore { mode } => {
                match mode {
                    InitMode::SelectKey {
                        keys,
                        selected_index,
                        custom_id,
                        is_custom,
                    } => match key.code {
                        KeyCode::Esc => self.popup = ActivePopup::None,
                        KeyCode::Char('n') if !*is_custom => {
                            *mode = InitMode::CreateKey {
                                name: String::new(),
                                email: String::new(),
                                active_field: InitField::Name,
                            };
                        }
                        KeyCode::Tab => {
                            *is_custom = !*is_custom;
                        }
                        KeyCode::Char('j') | KeyCode::Down if !*is_custom => {
                            if !*is_custom && *selected_index + 1 < keys.len() {
                                *selected_index += 1;
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up if !*is_custom => {
                            if !*is_custom && *selected_index > 0 {
                                *selected_index -= 1;
                            }
                        }
                        KeyCode::Char(c) if *is_custom => {
                            custom_id.push(c);
                        }
                        KeyCode::Backspace if *is_custom => {
                            custom_id.pop();
                        }
                        KeyCode::Enter => {
                            let gpg_id = if *is_custom {
                                custom_id.trim().to_string()
                            } else if let Some(k) = keys.get(*selected_index) {
                                k.id.clone()
                            } else {
                                String::new()
                            };

                            if gpg_id.is_empty() {
                                self.notify("✗ Please select or enter a GPG key ID".into(), true);
                                return;
                            }

                            self.start_background_action(
                                "Initializing password store…",
                                move || BackgroundResult::Initialize(commands::pass_init(&gpg_id)),
                            );
                        }
                        _ => {}
                    },
                    InitMode::CreateKey {
                        name,
                        email,
                        active_field,
                    } => match key.code {
                        KeyCode::Esc => self.popup = ActivePopup::None,
                        KeyCode::Tab | KeyCode::BackTab => {
                            *active_field = match active_field {
                                InitField::Name => InitField::Email,
                                InitField::Email => InitField::Name,
                            };
                        }
                        KeyCode::Char(c) => match active_field {
                            InitField::Name => name.push(c),
                            InitField::Email => email.push(c),
                        },
                        KeyCode::Backspace => match active_field {
                            InitField::Name => {
                                name.pop();
                            }
                            InitField::Email => {
                                email.pop();
                            }
                        },
                        KeyCode::Enter => {
                            if *active_field == InitField::Name && !name.trim().is_empty() {
                                *active_field = InitField::Email;
                                return;
                            }

                            let n = name.trim().to_string();
                            let e = email.trim().to_string();

                            if n.is_empty() || e.is_empty() {
                                self.notify(
                                    "✗ Both name and email are required to create a GPG key".into(),
                                    true,
                                );
                                return;
                            }

                            self.start_background_action(
                                "Creating GPG key and initializing store…",
                                move || {
                                    BackgroundResult::Initialize(
                                        commands::generate_gpg_key(&n, &e)
                                            .and_then(|key| commands::pass_init(&key)),
                                    )
                                },
                            );
                        }
                        _ => {}
                    },
                }
                return;
            }
            ActivePopup::GitSync => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => self.popup = ActivePopup::None,
                    KeyCode::Char('p') => {
                        self.trigger_git_push();
                    }
                    KeyCode::Char('u') => {
                        self.trigger_git_pull();
                    }
                    KeyCode::Char('s') => {
                        self.trigger_git_sync();
                    }
                    KeyCode::Char('c') => self.copy_remote_url(),
                    KeyCode::Char('r') => {
                        let current_url = self.git_status.remote_url.clone().unwrap_or_default();
                        self.popup = ActivePopup::SetRemote { url: current_url };
                    }
                    KeyCode::Char('i') => {
                        if !self.git_status.is_git_repo {
                            self.trigger_git_init();
                        }
                    }
                    KeyCode::Char('h') => self.open_github_repo(),
                    _ => {}
                }
                return;
            }
            ActivePopup::SetRemote { url } => {
                match key.code {
                    KeyCode::Esc => {
                        self.popup = ActivePopup::GitSync;
                    }
                    KeyCode::Enter => {
                        let target_url = url.trim().to_string();
                        if target_url.is_empty() {
                            self.notify("✗ Remote URL cannot be empty".into(), true);
                            return;
                        }
                        match git::pass_git_set_remote(&target_url) {
                            Ok(()) => {
                                self.notify(format!("✓ Remote set to {target_url}"), false);
                                self.git_status = git::get_git_status(&self.store_dir);
                                self.popup = ActivePopup::GitSync;
                            }
                            Err(e) => {
                                self.notify(format!("✗ {e}"), true);
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        url.pop();
                    }
                    KeyCode::Char(c) => {
                        url.push(c);
                    }
                    _ => {}
                }
                return;
            }
            ActivePopup::GithubRepo { name } => {
                match key.code {
                    KeyCode::Esc => self.popup = ActivePopup::None,
                    KeyCode::Backspace => {
                        name.pop();
                    }
                    KeyCode::Char(c) => name.push(c),
                    KeyCode::Enter => {
                        let repo_name = name.trim().to_string();
                        if repo_name.is_empty() {
                            self.notify("✗ Repository name cannot be empty".into(), true);
                        } else {
                            self.popup = ActivePopup::None;
                            self.create_github_repo(&repo_name);
                        }
                    }
                    _ => {}
                }
                return;
            }
            ActivePopup::Notification { .. } => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                    self.popup = ActivePopup::None;
                }
                return;
            }
            ActivePopup::None
            | ActivePopup::Rename { .. }
            | ActivePopup::History(_)
            | ActivePopup::Totp { .. }
            | ActivePopup::Recovery(_)
            | ActivePopup::OtpInstall { .. } => {}
        }

        // ── Search mode ───────────────────────────────
        if self.input_mode == InputMode::Search {
            match key.code {
                KeyCode::Esc => {
                    self.clear_detail();
                    self.input_mode = InputMode::Normal;
                    self.search_query.clear();
                    self.search_input = Default::default();
                    self.rebuild_visible();
                }
                KeyCode::Enter => {
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Up => {
                    self.list_state.select_previous();
                    self.clear_detail();
                }
                KeyCode::Down => {
                    self.list_state.select_next();
                    self.clear_detail();
                }
                _ => {
                    if self.search_input.handle(&mut self.search_query, key) {
                        self.clear_detail();
                        self.list_state.select(Some(0));
                        self.rebuild_visible();
                    }
                }
            }
            return;
        }

        // ── Normal mode ───────────────────────────────
        match key.code {
            KeyCode::Char('R') => {
                if let Some(entry) = self.selected_entry().filter(|entry| !entry.is_dir) {
                    let path = entry.path.clone();
                    self.clear_detail();
                    self.start_background_action("Opening recovery codes…", move || {
                        let result = commands::pass_show(&path);
                        BackgroundResult::RecoveryOpen(path, result)
                    });
                } else {
                    self.notify(
                        "Select a password entry first, then press R for recovery codes.".into(),
                        true,
                    );
                }
            }
            KeyCode::Char('t') => {
                if let Some(entry) = self.selected_entry().filter(|entry| !entry.is_dir) {
                    let path = entry.path.clone();
                    self.clear_detail();
                    self.open_totp(path);
                } else {
                    self.notify("Select a password entry first. If a folder is highlighted, press Enter to expand it, then select an entry and press t.".into(), true);
                }
            }
            KeyCode::Char('f') => {
                if let Some(entry) = self.selected_entry().filter(|e| !e.is_dir) {
                    let path = entry.path.clone();
                    match self.favorites.toggle(&path) {
                        Ok(pinned) => {
                            self.clear_detail();
                            self.rebuild_visible();
                            self.set_status(if pinned {
                                "Added to favorites"
                            } else {
                                "Removed from favorites"
                            });
                        }
                        Err(error) => self.notify(error, true),
                    }
                }
            }
            KeyCode::Char('F') => {
                self.favorites_only = !self.favorites_only;
                self.clear_detail();
                self.list_state.select(Some(0));
                self.rebuild_visible();
            }
            KeyCode::Char('v') | KeyCode::Char('H') => {
                let path = if key.code == KeyCode::Char('v') {
                    match self.selected_entry().filter(|e| !e.is_dir) {
                        Some(entry) => Some(entry.path.clone()),
                        None => {
                            self.set_status("Select an entry, or press H for store history");
                            return;
                        }
                    }
                } else {
                    None
                };
                let store = self.store_dir.clone();
                self.clear_detail();
                self.start_background_action("Loading history…", move || {
                    BackgroundResult::History(crate::history::list(&store, path.as_deref()))
                });
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => self.running = false,
            KeyCode::Char('?') => self.popup = ActivePopup::Help,
            KeyCode::Char('j') | KeyCode::Down => {
                self.list_state.select_next();
                self.clear_detail();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.list_state.select_previous();
                self.clear_detail();
            }
            KeyCode::Char('g') => {
                self.generator_draft = None;
                self.editor = crate::editor::EntryEditor::default();
                self.form_error = None;
                self.open_generator(String::new());
            }
            KeyCode::Char('0') => {
                self.list_state.select_first();
                self.clear_detail();
            }
            KeyCode::End => {
                if !self.visible.is_empty() {
                    self.list_state.select(Some(self.visible.len() - 1));
                    self.clear_detail();
                }
            }
            KeyCode::Enter | KeyCode::Char('l') => self.view_selected_entry(),
            KeyCode::Char('h') | KeyCode::Backspace => self.toggle_selected_dir(),
            KeyCode::Char(c) if c == self.config.keybindings.copy_password => self.copy_password(),
            KeyCode::Char('p') => {
                self.show_password = !self.show_password;
                self.reveal_expires = self
                    .show_password
                    .then(|| Instant::now() + std::time::Duration::from_secs(15));
            }
            KeyCode::Char('d') => self.start_delete(),
            KeyCode::Char('a') => self.start_add(),
            KeyCode::Char('e') => {
                if let Some(entry) = self.selected_entry().filter(|entry| !entry.is_dir) {
                    let path = entry.path.clone();
                    self.start_background_action("Opening editor…", move || {
                        let result = commands::pass_show(&path);
                        BackgroundResult::Edit(path, result)
                    });
                }
            }
            KeyCode::Char('r') => {
                if let Some(entry) = self.selected_entry().filter(|entry| !entry.is_dir) {
                    let path = entry.path.clone();
                    self.form_error = None;
                    self.rename_input = Default::default();
                    self.popup = ActivePopup::Rename {
                        source: path.clone(),
                        destination: path,
                    };
                }
            }
            KeyCode::Char('u') | KeyCode::Char('w') => {
                let field = if key.code == KeyCode::Char('u') {
                    "username"
                } else {
                    "url"
                };
                if let Some(detail) = &self.detail {
                    let value = detail
                        .fields
                        .iter()
                        .find(|(key, _)| {
                            key.eq_ignore_ascii_case(field)
                                || (field == "username"
                                    && matches!(key.to_lowercase().as_str(), "login" | "user"))
                        })
                        .map(|(_, value)| value.clone());
                    if let Some(value) = value {
                        let seconds = self.config.behavior.auto_clear_clipboard_seconds;
                        let session = self.clipboard_session.clone();
                        let value = Zeroizing::new(value);
                        self.start_background_action("Copying field…", move || {
                            BackgroundResult::Copy(ClipboardManager::new().copy_for_session(
                                &value,
                                std::time::Duration::from_secs(seconds),
                                &session,
                            ))
                        });
                    } else {
                        self.set_status(format!("No {field} on this entry"));
                    }
                } else {
                    self.set_status("Open an entry with Enter first");
                }
            }
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(5),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(5),
            KeyCode::Char('i') => self.start_init_store(),
            KeyCode::Char('G') => self.open_git_sync(),
            KeyCode::Char('P') => self.trigger_git_push(),
            KeyCode::Char('U') => self.trigger_git_pull(),
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
                self.clear_detail();
                self.search_query.clear();
                self.search_input = Default::default();
                self.rebuild_visible();
            }
            KeyCode::Esc => {
                self.clear_detail();
                self.favorites_only = false;
                self.search_query.clear();
                self.search_input = Default::default();
                self.rebuild_visible();
            }
            _ => {}
        }
    }

    fn clear_detail(&mut self) {
        self.detail_scroll = 0;
        self.detail = None;
        self.show_password = false;
        self.reveal_expires = None;
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
