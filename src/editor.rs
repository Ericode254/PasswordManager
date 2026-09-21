use crate::pass::commands::DecryptedEntry;
use zeroize::Zeroize;

#[derive(Default)]
pub struct EntryEditor {
    pub inputs: [crate::text_input::TextInput; 6],
    pub username: String,
    username_key: Option<String>,
    pub url: String,
    pub notes: String,
    pub recovery_codes: String,
    used_recovery_codes: String,
    pub editing_path: Option<String>,
    pub original_content: Option<String>,
}

impl Drop for EntryEditor {
    fn drop(&mut self) {
        self.username.zeroize();
        self.url.zeroize();
        self.notes.zeroize();
        self.recovery_codes.zeroize();
        self.used_recovery_codes.zeroize();
        self.original_content.zeroize();
    }
}

impl EntryEditor {
    pub fn from_entry(path: String, entry: &DecryptedEntry) -> Self {
        let mut editor = Self::default();
        editor.editing_path = Some(path);
        editor.original_content = Some(entry.content.clone());
        let mut remaining = Vec::new();
        let mut found_username = false;
        let mut found_url = false;
        for line in entry
            .content
            .split_once('\n')
            .map(|(_, body)| body)
            .unwrap_or("")
            .split('\n')
        {
            if let Some(code) = crate::recovery::parse_line(line) {
                let field = if code.used {
                    &mut editor.used_recovery_codes
                } else {
                    &mut editor.recovery_codes
                };
                if !field.is_empty() {
                    field.push('\n');
                }
                field.push_str(code.value);
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                if !found_username
                    && matches!(
                        key.trim().to_lowercase().as_str(),
                        "username" | "login" | "user"
                    )
                {
                    editor.username = value.trim().to_string();
                    editor.username_key = Some(key.to_string());
                    found_username = true;
                    continue;
                }
                if !found_url && key.trim().eq_ignore_ascii_case("url") {
                    editor.url = value.trim().to_string();
                    found_url = true;
                    continue;
                }
            }
            remaining.push(line);
        }
        editor.notes = remaining.join("\n");
        editor
    }

    pub fn content(&self, password: &str) -> String {
        let mut content = password.to_string();
        if !self.username.is_empty() {
            content.push_str(&format!(
                "\n{}: {}",
                self.username_key.as_deref().unwrap_or("username"),
                self.username
            ));
        }
        if !self.url.is_empty() {
            content.push_str(&format!("\nurl: {}", self.url));
        }
        if !self.notes.is_empty() {
            content.push('\n');
            content.push_str(&self.notes);
        }
        for (key, codes) in [
            ("recovery-code", &self.recovery_codes),
            ("recovery-code-used", &self.used_recovery_codes),
        ] {
            for code in codes.lines().map(str::trim).filter(|line| !line.is_empty()) {
                content.push('\n');
                content.push_str(key);
                content.push_str(": ");
                content.push_str(code);
            }
        }
        content
    }
}

pub fn valid_path(name: &str) -> bool {
    !name.trim().is_empty()
        && !name.starts_with('-')
        && !name.chars().any(char::is_control)
        && std::path::Path::new(name)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_codes_round_trip_and_preserve_used_status_and_notes() {
        let entry = crate::pass::commands::parse_entry(
            "password\nlogin: alice\nrecovery-code: available-one\nrecovery-code-used: used-one\notpauth://totp/Test?secret=AA\nplain note",
        );
        let mut editor = EntryEditor::from_entry("example".into(), &entry);
        assert_eq!(editor.recovery_codes, "available-one");
        assert!(!editor.notes.contains("available-one"));
        editor.recovery_codes.push_str("\nnew-code");
        let saved = zeroize::Zeroizing::new(editor.content("new-password"));
        let codes = crate::recovery::codes(&saved);
        assert_eq!(codes.len(), 3);
        assert!(codes.iter().any(|c| c.used && c.value == "used-one"));
        assert!(saved.contains("otpauth://totp/Test?secret=AA\nplain note"));
        assert!(saved.starts_with("new-password\nlogin: alice"));
    }
    #[test]
    fn editing_preserves_unknown_fields_and_multiline_notes() {
        let entry = crate::pass::commands::parse_entry(
            "secret\nlogin: alice\nurl: https://example.com\notpauth: opaque\n\nNote: do not lose this\nnext line",
        );
        let editor = EntryEditor::from_entry("Work/example".into(), &entry);
        assert_eq!(editor.username, "alice");
        assert_eq!(editor.url, "https://example.com");
        let output = editor.content("new-secret");
        assert!(output.contains("otpauth: opaque\n\nNote: do not lose this\nnext line"));
        assert!(output.starts_with("new-secret\nlogin: alice"));
    }
    #[test]
    fn entry_paths_stay_within_store() {
        assert!(valid_path("Work/github"));
        for name in [
            "",
            "../secret",
            "/tmp/secret",
            "Work/../secret",
            "--force",
            "name\nother",
        ] {
            assert!(!valid_path(name), "accepted {name:?}");
        }
    }
}
