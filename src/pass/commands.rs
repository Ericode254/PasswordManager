use std::io::Write;
use std::process::Command;
use zeroize::{Zeroize, Zeroizing};

/// Parsed content of a decrypted pass entry.
#[derive(Debug, Clone)]
pub struct DecryptedEntry {
    pub content: String,
    /// First line — the password.
    pub password: String,
    /// Subsequent `key: value` lines.
    pub fields: Vec<(String, String)>,
    /// Lines that aren't key-value pairs.
    pub notes: Vec<String>,
}

impl DecryptedEntry {
    pub fn login_password(&self) -> Result<&str, String> {
        if self.password.contains("otpauth://") {
            Err("OTP-only entry: use t in PassTUI to generate a code.".into())
        } else {
            Ok(&self.password)
        }
    }
}

impl Drop for DecryptedEntry {
    fn drop(&mut self) {
        self.content.zeroize();
        self.password.zeroize();
        for (key, value) in &mut self.fields {
            key.zeroize();
            value.zeroize();
        }
        self.notes.zeroize();
    }
}

/// Checks whether the `pass` binary is available on `$PATH`.
pub fn is_pass_available() -> bool {
    Command::new("pass")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Decrypts and parses a password entry.
pub fn pass_show(entry: &str) -> Result<DecryptedEntry, String> {
    let output = Command::new("pass")
        .arg("show")
        .arg(entry)
        .output()
        .map_err(|e| format!("Failed to run `pass`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pass show failed: {}", stderr.trim()));
    }

    let bytes = Zeroizing::new(output.stdout);
    let content = Zeroizing::new(String::from_utf8_lossy(&bytes).into_owned());
    Ok(parse_entry(&content))
}

/// Parses the output of `pass show` into password, key-value fields, and notes.
pub fn parse_entry(content: &str) -> DecryptedEntry {
    let mut lines = content.lines();

    let password = lines.next().unwrap_or("").to_string();
    let mut fields = Vec::new();
    let mut notes = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            fields.push((key.trim().to_string(), value.trim().to_string()));
        } else {
            notes.push(trimmed.to_string());
        }
    }

    DecryptedEntry {
        content: content.to_string(),
        password,
        fields,
        notes,
    }
}

/// Inserts a new entry into the password store using `pass insert --multiline`.
pub fn pass_insert(entry: &str, content: &str, overwrite: bool) -> Result<(), String> {
    pass_insert_in(
        &crate::pass::store::get_store_dir(),
        entry,
        content,
        overwrite,
    )
}

pub fn pass_insert_in(
    store: &std::path::Path,
    entry: &str,
    content: &str,
    overwrite: bool,
) -> Result<(), String> {
    let mut command = Command::new("pass");
    command.env("PASSWORD_STORE_DIR", store);
    command.args(["insert", "--multiline"]);
    if overwrite {
        command.arg("--force");
    }
    let mut child = command
        .arg(entry)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run `pass`: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write to pass stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for pass: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pass insert failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Removes an entry from the password store (with --force to skip confirmation).
pub fn pass_remove(entry: &str) -> Result<(), String> {
    let output = Command::new("pass")
        .arg("rm")
        .arg("--force")
        .arg(entry)
        .output()
        .map_err(|e| format!("Failed to run `pass`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pass rm failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Move within the encrypted store; never overwrite a destination implicitly.
pub fn pass_move(source: &str, destination: &str) -> Result<(), String> {
    let output = Command::new("pass")
        .args(["mv", "--", source, destination])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Move failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// A discovered GPG key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpgKey {
    pub id: String,
    pub uid: String,
}

/// Checks if the password store is initialized with a .gpg-id file.
pub fn is_store_initialized(store_dir: &std::path::Path) -> bool {
    store_dir.join(".gpg-id").exists()
}

/// Lists existing secret/private GPG keys on the system.
pub fn list_gpg_keys() -> Vec<GpgKey> {
    let output = Command::new("gpg")
        .args(["--list-secret-keys", "--with-colons"])
        .output();

    let output = match output {
        Ok(out) if out.status.success() => out,
        _ => return Vec::new(),
    };

    let content = String::from_utf8_lossy(&output.stdout);
    parse_gpg_colons(&content)
}

/// Parses machine-readable colons format from `gpg --with-colons`.
pub fn parse_gpg_colons(content: &str) -> Vec<GpgKey> {
    let mut keys = Vec::new();
    let mut current_id: Option<String> = None;

    for line in content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "sec" | "pub" => {
                if parts.len() > 4 && !parts[4].is_empty() {
                    current_id = Some(parts[4].to_string());
                }
            }
            "uid" => {
                if let Some(ref id) = current_id
                    && parts.len() > 9
                    && !parts[9].is_empty()
                {
                    keys.push(GpgKey {
                        id: id.clone(),
                        uid: parts[9].to_string(),
                    });
                }
            }
            _ => {}
        }
    }

    keys
}

/// Generates a GPG key using the agent’s pinentry for passphrase protection.
pub fn generate_gpg_key(name: &str, email: &str) -> Result<String, String> {
    let user_id = format!("{name} <{email}>");
    let output = Command::new("gpg")
        .args([
            "--batch",
            "--pinentry-mode",
            "ask",
            "--quick-generate-key",
            &user_id,
            "default",
            "default",
            "0",
        ])
        .output()
        .map_err(|e| format!("Failed to run gpg: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("GPG key generation failed: {}", stderr.trim()));
    }

    Ok(email.to_string())
}

/// Initializes `pass` with a GPG key ID or email.
pub fn pass_init(gpg_id: &str) -> Result<(), String> {
    let output = Command::new("pass")
        .arg("init")
        .arg(gpg_id)
        .output()
        .map_err(|e| format!("Failed to run `pass init`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pass init failed: {}", stderr.trim()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entry_simple() {
        let text = "secret123\nlogin: user@example.com\nurl: https://github.com\nSome random note";
        let parsed = parse_entry(text);
        assert_eq!(parsed.password, "secret123");
        assert_eq!(parsed.fields.len(), 2);
        assert_eq!(
            parsed.fields[0],
            ("login".to_string(), "user@example.com".to_string())
        );
        assert_eq!(
            parsed.fields[1],
            ("url".to_string(), "https://github.com".to_string())
        );
        assert_eq!(parsed.notes.len(), 1);
        assert_eq!(parsed.notes[0], "Some random note");
    }

    #[test]
    fn test_parse_entry_only_password() {
        let text = "justapassword";
        let parsed = parse_entry(text);
        assert_eq!(parsed.password, "justapassword");
        assert!(parsed.fields.is_empty());
        assert!(parsed.notes.is_empty());
    }

    #[test]
    fn test_parse_entry_empty() {
        let parsed = parse_entry("");
        assert_eq!(parsed.password, "");
        assert!(parsed.fields.is_empty());
        assert!(parsed.notes.is_empty());
    }

    #[test]
    fn test_parse_gpg_colons() {
        let sample = "\
sec:u:255:22:1496D47CB1A4A4C7:1788468142:::u:::scESC:::+::ed25519:::0:
fpr:::::::::B2EF3F4F88F2A0BC32C6C03F1496D47CB1A4A4C7:
uid:u::::1788468142::9C2BD3B4A80856A48A554A5909B74B623E586E17::Alice Smith <alice@example.com>::::::::::0:
";
        let keys = parse_gpg_colons(sample);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].id, "1496D47CB1A4A4C7");
        assert_eq!(keys[0].uid, "Alice Smith <alice@example.com>");
    }
}
