use anyhow::{Context, Result, bail};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph, Wrap},
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    event::{self, AppEvent},
    ui::theme,
};

const MAX_BYTES: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PromptMode {
    Setup,
    Unlock,
    VerifyCurrent,
    Replace,
}

/// Local application access control. GPG remains responsible for vault encryption.
pub struct Auth {
    path: PathBuf,
    hash: Option<String>,
}

impl Auth {
    pub fn load() -> Result<Self> {
        let path = dirs::config_dir()
            .context("Cannot locate the configuration directory")?
            .join("passtui/master-password");
        Self::load_from(path)
    }

    fn load_from(path: PathBuf) -> Result<Self> {
        let hash = match fs::File::open(&path) {
            Ok(file) => {
                let mut text = String::new();
                file.take(4097)
                    .read_to_string(&mut text)
                    .context("Cannot read master-password record")?;
                validate_hash(&text)?;
                Some(text)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // A dangling symlink is an error, not a first-time setup.
                if fs::symlink_metadata(&path).is_ok() {
                    bail!("Cannot read master-password record");
                }
                None
            }
            Err(error) => return Err(error).context("Cannot read master-password record"),
        };
        Ok(Self { path, hash })
    }

    fn enroll(&mut self, password: &str, confirmation: &str) -> Result<()> {
        self.save_password(password, confirmation, false)
    }

    fn save_password(&mut self, password: &str, confirmation: &str, replace: bool) -> Result<()> {
        validate_password(password)?;
        if password != confirmation {
            bail!("Passwords do not match. Re-enter both fields.");
        }
        let salt = SaltString::encode_b64(&rand::random::<[u8; 16]>())
            .map_err(|_| anyhow::anyhow!("Cannot generate password salt"))?;
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| anyhow::anyhow!("Cannot hash master password"))?
            .to_string();
        let directory = self.path.parent().context("Invalid authentication path")?;
        fs::create_dir_all(directory)?;
        let mut lock_options = private_file_options();
        lock_options.create(true).truncate(false);
        let lock = lock_options.open(directory.join(".master-password.lock"))?;
        lock.try_lock()
            .map_err(|_| anyhow::anyhow!("Another password update is in progress. Try again."))?;
        if replace {
            let current = Self::load_from(self.path.clone())?;
            if self.hash.is_none() || current.hash != self.hash {
                bail!(
                    "Master password changed elsewhere. Cancel and verify the current password again."
                );
            }
        }
        let temporary = directory.join(format!(
            ".master-password-{:016x}.tmp",
            rand::random::<u64>()
        ));
        let mut file = private_file_options().create_new(true).open(&temporary)?;
        let result = (|| -> Result<()> {
            file.write_all(hash.as_bytes())
                .context("Cannot save master-password record")?;
            file.sync_all()
                .context("Cannot persist master-password record")?;
            if replace {
                fs::rename(&temporary, &self.path)
                    .context("Cannot replace master-password record")?;
            } else {
                // Publish a complete record without overwriting another enrollment.
                fs::hard_link(&temporary, &self.path).context(
                    "Cannot create master-password record; close and reopen PassTUI to retry",
                )?;
            }
            Ok(())
        })();
        drop(file);
        let _ = fs::remove_file(&temporary);
        result?;
        self.hash = Some(hash);
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        let current = Self::load_from(self.path.clone())?;
        if self.hash.is_some() && current.hash.is_none() {
            bail!("Master-password record is missing; access denied");
        }
        self.hash = current.hash;
        Ok(())
    }

    pub fn change_password(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<bool> {
        self.refresh()?;
        if self.hash.is_none() {
            bail!("Launch PassTUI to set a master password first.");
        }
        if !self.prompt_mode(terminal, PromptMode::VerifyCurrent)? {
            return Ok(false);
        }
        self.prompt_mode(terminal, PromptMode::Replace)
    }

    fn verify(&self, password: &str) -> bool {
        self.hash
            .as_deref()
            .and_then(|hash| PasswordHash::new(hash).ok())
            .is_some_and(|hash| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &hash)
                    .is_ok()
            })
    }

    /// Returns false on cancellation. Never exposes store contents before success.
    pub fn prompt(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<bool> {
        self.refresh()?;
        let mode = if self.hash.is_none() {
            PromptMode::Setup
        } else {
            PromptMode::Unlock
        };
        self.prompt_mode(terminal, mode)
    }

    fn prompt_mode(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
        mode: PromptMode,
    ) -> Result<bool> {
        let setup = matches!(mode, PromptMode::Setup | PromptMode::Replace);
        // Reserve the input limit so typing does not leave old allocations behind.
        let mut fields = [
            Zeroizing::new(String::with_capacity(MAX_BYTES)),
            Zeroizing::new(String::with_capacity(MAX_BYTES)),
        ];
        let mut active = 0;
        let mut message = String::new();
        let mut failures = 0u32;
        let mut retry_at = Instant::now();
        let mut generated: Option<Zeroizing<String>> = None;
        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                frame.render_widget(Block::default().style(theme::base()), area);
                let [_, panel, _] = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(17),
                    Constraint::Fill(1),
                ])
                .areas(area);
                let [_, panel, _] = Layout::horizontal([
                    Constraint::Fill(1),
                    Constraint::Length(74),
                    Constraint::Fill(1),
                ])
                .areas(panel);
                if let Some(password) = &generated {
                    use ratatui::text::Line;
                    frame.render_widget(
                        Paragraph::new(vec![
                            Line::from("Your generated master password:"),
                            Line::from(""),
                            Line::from(password.as_str()),
                            Line::from(""),
                            Line::from("Write this down somewhere safe before continuing."),
                            Line::from(
                                "You will retype it to confirm, and need it to unlock PassTUI.",
                            ),
                            Line::from(""),
                            Line::from(
                                "Enter uses this password · Ctrl+G generates another · Esc back",
                            ),
                        ])
                        .wrap(Wrap { trim: false })
                        .block(Block::bordered().title(" Generated master password ")),
                        panel,
                    );
                    return;
                }
                let mut lines = vec![
                    match mode {
                        PromptMode::Setup => {
                            "Welcome! Set a strong master password to protect access to PassTUI."
                        }
                        PromptMode::Unlock => "Enter your master password to unlock PassTUI.",
                        PromptMode::VerifyCurrent => {
                            "Enter your current master password before changing it."
                        }
                        PromptMode::Replace => "Choose and confirm a new master password.",
                    }
                    .to_string(),
                    String::new(),
                ];
                if setup {
                    lines.push(
                        "Use at least 15 characters. Spaces and other whitespace are not allowed."
                            .into(),
                    );
                    lines.push(
                        "Tip: join six or more random words with hyphens, not spaces.".into(),
                    );
                    lines.push(String::new());
                    lines.push("Ctrl+G generates a strong six-word passphrase for you.".into());
                }
                for (index, value) in fields.iter().enumerate().take(if setup { 2 } else { 1 }) {
                    lines.push(format!(
                        "{} {}: {}{}",
                        if active == index { ">" } else { " " },
                        if index == 0 {
                            "Password"
                        } else {
                            "Confirm password"
                        },
                        "•".repeat(value.chars().count().min(48)),
                        if active == index { "█" } else { "" }
                    ));
                }
                lines.push(String::new());
                lines.push(message.clone());
                if Instant::now() < retry_at {
                    lines.push("Please wait before trying again.".into());
                }
                lines.push(String::new());
                lines.push(
                    "Enter continues · Tab switches fields · Ctrl+U clears · Esc quits".into(),
                );
                frame.render_widget(
                    Paragraph::new(lines.join("\n"))
                        .wrap(Wrap { trim: false })
                        .block(Block::bordered().title(match mode {
                            PromptMode::Setup => " Create master password ",
                            PromptMode::Unlock => " Unlock PassTUI ",
                            PromptMode::VerifyCurrent => " Verify current master password ",
                            PromptMode::Replace => " Change master password ",
                        })),
                    panel,
                );
            })?;
            let input = event::poll_event(Duration::from_millis(100))?;
            if generated.is_some() {
                match input {
                    Some(AppEvent::Key(key)) => match key.code {
                        KeyCode::Esc => generated = None,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(false);
                        }
                        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            generated = Some(generate_master_password());
                        }
                        KeyCode::Enter => {
                            for field in &mut fields {
                                field.zeroize();
                            }
                            fields[0].push_str(generated.take().as_deref().unwrap());
                            active = 1;
                            message = "Retype the generated password to confirm it.".into();
                        }
                        _ => {}
                    },
                    Some(AppEvent::Paste(text)) => drop(Zeroizing::new(text)),
                    _ => {}
                }
                continue;
            }
            match input {
                Some(AppEvent::Key(key)) => match key.code {
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(false);
                    }
                    KeyCode::Tab | KeyCode::BackTab if setup => active = 1 - active,
                    KeyCode::Char('g')
                        if setup && key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        generated = Some(generate_master_password());
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        fields[active].zeroize()
                    }
                    KeyCode::Backspace => {
                        fields[active].pop();
                    }
                    KeyCode::Enter if Instant::now() >= retry_at => {
                        if setup && active == 0 {
                            match validate_password(&fields[0]) {
                                Ok(()) => {
                                    active = 1;
                                    message.clear();
                                }
                                Err(error) => message = error.to_string(),
                            }
                            continue;
                        }
                        if setup {
                            if let Err(error) = validate_password(&fields[0]) {
                                message = error.to_string();
                                active = 0;
                                continue;
                            }
                            if fields[0] != fields[1] {
                                message = "Passwords do not match. Re-enter both fields.".into();
                            } else {
                                let result = if mode == PromptMode::Replace {
                                    self.save_password(&fields[0], &fields[1], true)
                                } else {
                                    self.enroll(&fields[0], &fields[1])
                                };
                                match result {
                                    Ok(()) => return Ok(true),
                                    Err(error) => {
                                        message = error.to_string();
                                        continue;
                                    }
                                }
                            }
                        } else {
                            // An already-open prompt must also notice password changes.
                            self.refresh()?;
                            if self.verify(&fields[0]) {
                                return Ok(true);
                            }
                            message = "Incorrect master password. Try again.".into();
                            failures = failures.saturating_add(1);
                            retry_at = Instant::now()
                                + Duration::from_secs((1u64 << failures.min(5)).min(30));
                        }
                        for field in &mut fields {
                            field.zeroize();
                        }
                        active = 0;
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && !c.is_control() =>
                    {
                        if fields[active].len() + c.len_utf8() <= MAX_BYTES {
                            fields[active].push(c);
                        } else {
                            message = "Password is too long (maximum 1024 bytes).".into();
                        }
                    }
                    _ => {}
                },
                Some(AppEvent::Paste(text)) => {
                    let text = Zeroizing::new(text);
                    if text.chars().any(char::is_control)
                        || fields[active].len() + text.len() > MAX_BYTES
                    {
                        message = "Paste a single line, at most 1024 bytes.".into();
                    } else {
                        fields[active].push_str(&text);
                    }
                }
                _ => {}
            }
        }
    }
}

fn generate_master_password() -> Zeroizing<String> {
    loop {
        let password = Zeroizing::new(crate::pass::generator::generate_passphrase(6));
        if validate_password(&password).is_ok() {
            return password;
        }
    }
}

fn private_file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

fn validate_password(password: &str) -> Result<()> {
    if password.chars().any(char::is_whitespace) {
        bail!("Spaces and other whitespace are not allowed. Use hyphens between words.");
    }
    if password.chars().count() < 15 {
        bail!("Use at least 15 characters.");
    }
    if password.len() > MAX_BYTES || password.chars().any(char::is_control) {
        bail!("Use a single line, at most 1024 bytes.");
    }
    if crate::pass::generator::strength(password).0 < 4 {
        bail!("Password is too predictable. Use more unrelated words or random characters.");
    }
    Ok(())
}

fn validate_hash(text: &str) -> Result<()> {
    let hash = PasswordHash::new(text)
        .map_err(|_| anyhow::anyhow!("Invalid master-password record; access denied"))?;
    // Restrict to our format, including costs, before allocating hashing memory.
    if hash.algorithm.as_str() != "argon2id"
        || hash.version != Some(19)
        || hash.params.to_string() != "m=19456,t=2,p=1"
        || hash.salt.is_none()
        || hash.hash.is_none_or(|value| value.len() != 32)
    {
        bail!("Unsupported master-password record; access denied");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const STRONG: &str = "orchid-lantern-quartz-voyage-cobalt-finch";

    #[test]
    fn generated_master_password_meets_enrollment_requirements() {
        let password = generate_master_password();
        // Some bundled words are themselves hyphenated (for example, "yo-yo").
        assert!(password.split('-').count() >= 6);
        assert!(validate_password(&password).is_ok());
    }

    #[test]
    fn rejects_weak_and_short_passwords() {
        for password in [
            "",
            "short!9Aa",
            "passwordpasswordpassword",
            "12345678901234567890",
        ] {
            assert!(validate_password(password).is_err());
        }
        assert!(validate_password(STRONG).is_ok());
    }

    #[test]
    fn rejects_whitespace_without_normalizing_passwords() {
        for separator in [" ", "\t", "\n", "\u{a0}", "\u{2003}", "\u{202f}"] {
            assert!(validate_password(&STRONG.replace('-', separator)).is_err());
            assert!(validate_password(&format!("{separator}{STRONG}")).is_err());
            assert!(validate_password(&format!("{STRONG}{separator}")).is_err());
        }
    }

    #[test]
    fn password_changes_preserve_existing_credentials_on_failure_and_refresh_other_sessions() {
        const NEW: &str = "meteor-canvas-saffron-dolphin-velvet-bridge";
        let directory =
            std::env::temp_dir().join(format!("passtui-change-test-{}", rand::random::<u64>()));
        let path = directory.join("master-password");
        let mut auth = Auth::load_from(path.clone()).unwrap();
        auth.enroll(STRONG, STRONG).unwrap();
        let mut other_session = Auth::load_from(path.clone()).unwrap();
        let original = fs::read_to_string(&path).unwrap();
        assert!(auth.save_password("weak", "weak", true).is_err());
        assert!(auth.save_password(NEW, "mismatch", true).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let lock = private_file_options()
            .open(directory.join(".master-password.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert!(auth.save_password(NEW, NEW, true).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        drop(lock);
        auth.save_password(NEW, NEW, true).unwrap();
        let updated = fs::read_to_string(&path).unwrap();
        assert_ne!(original, updated);
        assert!(auth.verify(NEW));
        assert!(!auth.verify(STRONG));
        assert!(other_session.save_password(STRONG, STRONG, true).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), updated);
        other_session.refresh().unwrap();
        assert!(other_session.verify(NEW));
        assert!(!other_session.verify(STRONG));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert!(!fs::read_dir(&directory).unwrap().any(|entry| {
            entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "tmp")
        }));
        fs::remove_file(&path).unwrap();
        assert!(other_session.refresh().is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn existing_password_with_spaces_can_still_unlock_and_be_replaced() {
        let directory =
            std::env::temp_dir().join(format!("passtui-legacy-test-{}", rand::random::<u64>()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("master-password");
        let legacy = STRONG.replace('-', " ");
        let salt = SaltString::encode_b64(&rand::random::<[u8; 16]>()).unwrap();
        let hash = Argon2::default()
            .hash_password(legacy.as_bytes(), &salt)
            .unwrap()
            .to_string();
        fs::write(&path, hash).unwrap();
        let mut auth = Auth::load_from(path).unwrap();
        assert!(auth.verify(&legacy));
        assert!(!auth.verify(STRONG));
        auth.save_password(STRONG, STRONG, true).unwrap();
        assert!(auth.verify(STRONG));
        assert!(!auth.verify(&legacy));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn enrollment_persistence_verification_and_fail_closed() {
        let directory =
            std::env::temp_dir().join(format!("passtui-auth-test-{}", rand::random::<u64>()));
        let path = directory.join("master-password");
        let mut auth = Auth::load_from(path.clone()).unwrap();
        assert!(auth.hash.is_none());
        assert!(auth.enroll(STRONG, "different").is_err());
        assert!(!path.exists());
        auth.enroll(STRONG, STRONG).unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(!saved.contains(STRONG));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let auth = Auth::load_from(path.clone()).unwrap();
        assert!(auth.verify(STRONG));
        assert!(!auth.verify("wrong"));
        let mut competing = Auth {
            path: path.clone(),
            hash: None,
        };
        assert!(competing.enroll(STRONG, STRONG).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), saved);
        fs::write(&path, "corrupted").unwrap();
        assert!(Auth::load_from(path).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
