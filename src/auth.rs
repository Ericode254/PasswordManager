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
        fs::create_dir_all(self.path.parent().context("Invalid authentication path")?)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // Never overwrite a credential created by another running instance.
        let mut file = options
            .open(&self.path)
            .context("Cannot create master-password record; close and reopen PassTUI to retry")?;
        file.write_all(hash.as_bytes())
            .context("Cannot save master-password record")?;
        file.sync_all()
            .context("Cannot persist master-password record")?;
        self.hash = Some(hash);
        Ok(())
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
        let setup = self.hash.is_none();
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
                    if setup {
                        "Welcome! Set a strong master password to protect access to PassTUI."
                            .to_string()
                    } else {
                        "Enter your master password to unlock PassTUI.".to_string()
                    },
                    String::new(),
                ];
                if setup {
                    lines.push(
                        "Use at least 15 characters and a strong, unpredictable password.".into(),
                    );
                    lines.push(
                        "Tip: use six or more randomly chosen words. Avoid common phrases.".into(),
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
                        .block(Block::bordered().title(if setup {
                            " Create master password "
                        } else {
                            " Unlock PassTUI "
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
                                self.enroll(&fields[0], &fields[1])?;
                                return Ok(true);
                            }
                        } else if self.verify(&fields[0]) {
                            return Ok(true);
                        } else {
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

fn validate_password(password: &str) -> Result<()> {
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
