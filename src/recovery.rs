use crate::pass::commands::{self, DecryptedEntry};
use std::time::Instant;
use zeroize::Zeroizing;

pub struct Code<'a> {
    pub value: &'a str,
    pub used: bool,
}

pub fn field(key: &str) -> bool {
    key.trim().eq_ignore_ascii_case("recovery-code")
        || key.trim().eq_ignore_ascii_case("recovery-code-used")
}

pub fn parse_line(line: &str) -> Option<Code<'_>> {
    let (key, value) = line.split_once(':')?;
    if !field(key) || value.trim().is_empty() {
        return None;
    }
    Some(Code {
        value: value.trim(),
        used: key.trim().eq_ignore_ascii_case("recovery-code-used"),
    })
}

pub fn codes(content: &str) -> Vec<Code<'_>> {
    content.lines().skip(1).filter_map(parse_line).collect()
}

pub struct View {
    pub path: String,
    pub entry: DecryptedEntry,
    pub selected: usize,
    pub reveal_until: Option<Instant>,
    pub confirming: bool,
    pub error: Option<String>,
}
impl View {
    pub fn new(path: String, entry: DecryptedEntry) -> Self {
        Self {
            path,
            entry,
            selected: 0,
            reveal_until: None,
            confirming: false,
            error: None,
        }
    }
    pub fn codes(&self) -> Vec<Code<'_>> {
        codes(&self.entry.content)
    }
    pub fn revealed(&self) -> bool {
        self.reveal_until
            .is_some_and(|until| until > Instant::now())
    }
}

/// Change one occurrence, preserving all other lines (including duplicate codes).
pub fn toggle(content: &str, selected: usize) -> Result<Zeroizing<String>, String> {
    let mut output = Zeroizing::new(String::new());
    let mut index = 0;
    let mut found = false;
    for (line_number, line) in content.split_inclusive('\n').enumerate() {
        if line_number > 0
            && let Some(code) = parse_line(line)
        {
            if index == selected {
                output.push_str(if code.used {
                    "recovery-code: "
                } else {
                    "recovery-code-used: "
                });
                output.push_str(code.value);
                if line.ends_with("\r\n") {
                    output.push_str("\r\n");
                } else if line.ends_with('\n') {
                    output.push('\n');
                }
                found = true;
            } else {
                output.push_str(line);
            }
            index += 1;
        } else {
            output.push_str(line);
        }
    }
    if found {
        Ok(output)
    } else {
        Err("Recovery code no longer exists. Reopen the entry.".into())
    }
}

pub fn save_toggle(path: &str, original: &str, selected: usize) -> Result<DecryptedEntry, String> {
    let content = toggle(original, selected)?;
    if commands::pass_show(path)?.content != original {
        return Err(
            "Entry changed since opening. Close and reopen recovery codes before retrying.".into(),
        );
    }
    commands::pass_insert(path, &content, true)?;
    Ok(commands::parse_entry(&content))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toggle_preserves_other_fields_duplicates_and_line_endings() {
        let original = "password\r\nusername: alice\r\nrecovery-code: same\r\nrecovery-code: same\r\nnotes unchanged";
        let updated = toggle(original, 1).unwrap();
        assert_eq!(
            updated.as_str(),
            "password\r\nusername: alice\r\nrecovery-code: same\r\nrecovery-code-used: same\r\nnotes unchanged"
        );
        let values = codes(&updated);
        assert!(!values[0].used);
        assert!(values[1].used);
        assert_eq!(toggle(&updated, 1).unwrap().as_str(), original);
        assert!(toggle(original, 2).is_err());
        assert!(codes("recovery-code: password-only").is_empty());
    }
}
