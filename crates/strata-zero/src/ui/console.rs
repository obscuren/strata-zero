//! The dev console's state: a line of input, history and completion. Pure: no commands here.

use crossterm::event::{KeyCode, KeyEvent};

/// What a key did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsoleEvent {
    None,
    /// Enter on a non-empty line: run it.
    Run(String),
    /// Esc: close the console.
    Close,
}

#[derive(Clone, Debug, Default)]
pub struct Console {
    pub input: String,
    history: Vec<String>,
    /// Where ↑↓ stand in the history; `None` is the live line.
    cursor: Option<usize>,
    /// Completion candidates to show when Tab found several.
    pub hint: Option<String>,
}

impl Console {
    /// Feeds one key. `complete(head, word)` returns candidates for the word being
    /// typed, given the line before it; a single candidate is filled in.
    pub fn key(
        &mut self,
        k: KeyEvent,
        complete: &dyn Fn(&str, &str) -> Vec<String>,
    ) -> ConsoleEvent {
        match k.code {
            KeyCode::Esc => return ConsoleEvent::Close,
            KeyCode::Enter => {
                let line = self.input.trim().to_string();
                if line.is_empty() {
                    return ConsoleEvent::None;
                }
                self.history.push(line.clone());
                self.input.clear();
                self.cursor = None;
                self.hint = None;
                return ConsoleEvent::Run(line);
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.hint = None;
            }
            KeyCode::Up => {
                if self.history.is_empty() {
                    return ConsoleEvent::None;
                }
                let i = match self.cursor {
                    None => self.history.len() - 1,
                    Some(i) => i.saturating_sub(1),
                };
                self.cursor = Some(i);
                self.input = self.history[i].clone();
            }
            KeyCode::Down => match self.cursor {
                None => {}
                Some(i) if i + 1 < self.history.len() => {
                    self.cursor = Some(i + 1);
                    self.input = self.history[i + 1].clone();
                }
                Some(_) => {
                    self.cursor = None;
                    self.input.clear();
                }
            },
            KeyCode::Tab => {
                let (head, word) = match self.input.rfind(' ') {
                    Some(i) => (
                        self.input[..=i].to_string(),
                        self.input[i + 1..].to_string(),
                    ),
                    None => (String::new(), self.input.clone()),
                };
                let candidates = complete(head.trim_end(), &word);
                match candidates.as_slice() {
                    [] => {}
                    [one] => {
                        // a directory stays open for the next part of the path
                        let gap = if one.ends_with('/') { "" } else { " " };
                        self.input = format!("{head}{one}{gap}");
                        self.hint = None;
                    }
                    many => {
                        self.hint =
                            Some(many.iter().take(8).cloned().collect::<Vec<_>>().join(" "));
                    }
                }
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                self.hint = None;
                self.cursor = None;
            }
            _ => {}
        }
        ConsoleEvent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ch(c: char) -> KeyEvent {
        k(KeyCode::Char(c))
    }

    #[test]
    fn console_types_completes_runs_and_recalls() {
        let complete = |_line: &str, word: &str| -> Vec<String> {
            ["spawn", "drop", "copper_coil", "clear"]
                .iter()
                .filter(|c| c.starts_with(word))
                .map(|c| c.to_string())
                .collect()
        };
        let mut c = Console::default();
        for x in "spa".chars() {
            assert!(matches!(c.key(ch(x), &complete), ConsoleEvent::None));
        }
        c.key(k(KeyCode::Tab), &complete);
        assert_eq!(c.input, "spawn ");
        for x in "co".chars() {
            c.key(ch(x), &complete);
        }
        c.key(k(KeyCode::Tab), &complete);
        assert_eq!(c.input, "spawn copper_coil ");
        assert!(matches!(
            c.key(k(KeyCode::Enter), &complete),
            ConsoleEvent::Run(line) if line == "spawn copper_coil"
        ));
        assert_eq!(c.input, "");
        assert!(
            matches!(c.key(k(KeyCode::Enter), &complete), ConsoleEvent::None),
            "an empty line runs nothing"
        );
        c.key(ch('c'), &complete);
        c.key(k(KeyCode::Tab), &complete);
        assert_eq!(c.input, "c", "ambiguous: nothing filled in");
        assert_eq!(c.hint.as_deref(), Some("copper_coil clear"));
        c.key(k(KeyCode::Backspace), &complete);
        assert_eq!(c.input, "");
        assert_eq!(c.hint, None, "typing clears the hint");
        c.key(k(KeyCode::Up), &complete);
        assert_eq!(c.input, "spawn copper_coil");
        c.key(k(KeyCode::Down), &complete);
        assert_eq!(c.input, "");
        for x in "drop x".chars() {
            c.key(ch(x), &complete);
        }
        c.key(k(KeyCode::Enter), &complete);
        c.key(k(KeyCode::Up), &complete);
        assert_eq!(c.input, "drop x");
        c.key(k(KeyCode::Up), &complete);
        assert_eq!(c.input, "spawn copper_coil");
        c.key(k(KeyCode::Up), &complete);
        assert_eq!(c.input, "spawn copper_coil", "stops at the oldest");
        assert!(matches!(
            c.key(k(KeyCode::Esc), &complete),
            ConsoleEvent::Close
        ));
    }
}
