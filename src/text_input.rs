use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use zeroize::{Zeroize, Zeroizing};

/// Byte offsets always land on a UTF-8 boundary. Undo stays in memory only.
#[derive(Default)]
pub struct TextInput {
    cursor: Option<usize>,
    undo: Vec<(String, usize)>,
    redo: Vec<(String, usize)>,
}

impl Drop for TextInput {
    fn drop(&mut self) {
        for (text, _) in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            text.zeroize();
        }
    }
}

impl TextInput {
    pub fn position(&self, text: &str) -> usize {
        let mut pos = self.cursor.unwrap_or(text.len()).min(text.len());
        while !text.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    fn remember(&mut self, text: &str) {
        if self.undo.len() == 100 {
            self.undo.remove(0).0.zeroize();
        }
        self.undo.push((text.to_string(), self.position(text)));
        for (value, _) in &mut self.redo {
            value.zeroize();
        }
        self.redo.clear();
    }

    pub fn insert(
        &mut self,
        text: &mut String,
        value: &str,
        multiline: bool,
    ) -> Result<(), String> {
        let normalized = Zeroizing::new(value.replace("\r\n", "\n"));
        let value = Zeroizing::new(normalized.replace('\r', "\n"));
        if value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\t')))
        {
            return Err("This field accepts a single line without control characters.".into());
        }
        if !value.is_empty() {
            self.remember(text);
            let pos = self.position(text);
            text.insert_str(pos, &value);
            self.cursor = Some(pos + value.len());
        }
        Ok(())
    }

    pub fn handle(&mut self, text: &mut String, key: KeyEvent) -> bool {
        let pos = self.position(text);
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('z') | KeyCode::Char('y') => {
                    let undoing = key.code == KeyCode::Char('z');
                    let source = if undoing {
                        &mut self.undo
                    } else {
                        &mut self.redo
                    };
                    if let Some((previous, cursor)) = source.pop() {
                        let current = (std::mem::replace(text, previous), pos);
                        if undoing {
                            self.redo.push(current);
                        } else {
                            self.undo.push(current);
                        }
                        self.cursor = Some(cursor);
                    }
                }
                KeyCode::Char('u') => {
                    self.remember(text);
                    text.clear();
                    self.cursor = Some(0);
                }
                _ => return false,
            }
            return true;
        }
        match key.code {
            KeyCode::Left => {
                self.cursor = Some(text[..pos].char_indices().next_back().map_or(0, |(i, _)| i))
            }
            KeyCode::Right => {
                self.cursor = Some(pos + text[pos..].chars().next().map_or(0, char::len_utf8))
            }
            KeyCode::Home => self.cursor = Some(text[..pos].rfind('\n').map_or(0, |i| i + 1)),
            KeyCode::End => {
                self.cursor = Some(text[pos..].find('\n').map_or(text.len(), |i| pos + i))
            }
            KeyCode::Backspace if pos > 0 => {
                self.remember(text);
                let start = text[..pos].char_indices().next_back().unwrap().0;
                text.replace_range(start..pos, "");
                self.cursor = Some(start);
            }
            KeyCode::Delete if pos < text.len() => {
                self.remember(text);
                text.remove(pos);
                self.cursor = Some(pos);
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::ALT) && !c.is_control() => {
                let _ = self.insert(text, &c.to_string(), false);
            }
            _ => return false,
        }
        true
    }

    pub fn display(&self, text: &str, masked: bool) -> String {
        let pos = self.position(text);
        let convert = |value: &str| {
            if masked {
                value
                    .chars()
                    .map(|c| if c == '\n' { '\n' } else { '•' })
                    .collect()
            } else {
                value.to_string()
            }
        };
        format!("{}█{}", convert(&text[..pos]), convert(&text[pos..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_cursor_delete_and_undo_redo() {
        let mut input = TextInput::default();
        let mut text = "a🔑é".to_string();
        for code in [KeyCode::Left, KeyCode::Backspace] {
            input.handle(&mut text, KeyEvent::new(code, KeyModifiers::NONE));
        }
        assert_eq!(text, "aé");
        input.handle(
            &mut text,
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        );
        assert_eq!(text, "a🔑é");
        input.handle(
            &mut text,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
        );
        assert_eq!(text, "aé");
        input.handle(
            &mut text,
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
        );
        assert_eq!(text, "a");
    }
    #[test]
    fn paste_is_atomic_and_rejects_multiline_secrets() {
        let mut input = TextInput::default();
        let mut text = String::new();
        assert!(
            input
                .insert(&mut text, "password\nusername: other", false)
                .is_err()
        );
        assert!(text.is_empty());
        input.insert(&mut text, "first\r\nsecond", true).unwrap();
        assert_eq!(text, "first\nsecond");
        input.handle(
            &mut text,
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        );
        assert!(text.is_empty());
    }
}
