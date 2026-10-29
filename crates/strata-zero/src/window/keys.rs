//! Window key presses as the crossterm events every screen already reads, and the Cmd
//! chords that belong to the window.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// What one key press does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Game(KeyEvent),
    Window(Shortcut),
    Nothing,
}

/// The window's own chords, never passed to the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    Quit,
    Fullscreen,
    Bigger,
    Smaller,
    ResetSize,
}

/// A key event: `pressed` is false for a release (a held key's repeats are presses, as a
/// terminal's are); `logical` is the key with every modifier applied, `bare` the key
/// without them (Ctrl and Alt chords read the bare key, so Alt+A is `a`, not `å`).
pub fn translate(pressed: bool, logical: &Key, bare: &Key, mods: ModifiersState) -> Input {
    if !pressed {
        return Input::Nothing;
    }
    if mods.super_key() {
        return cmd_chord(bare, mods);
    }
    let mut m = KeyModifiers::NONE;
    if mods.shift_key() {
        m |= KeyModifiers::SHIFT;
    }
    if mods.control_key() {
        m |= KeyModifiers::CONTROL;
    }
    if mods.alt_key() {
        m |= KeyModifiers::ALT;
    }
    let code = match logical {
        Key::Named(named) => match named_code(*named, mods.shift_key()) {
            Some(code) => code,
            None => return Input::Nothing,
        },
        Key::Character(_) => {
            let key = if mods.control_key() || mods.alt_key() {
                bare
            } else {
                logical
            };
            match key {
                Key::Character(s) => match s.chars().next() {
                    Some(c) => KeyCode::Char(c),
                    None => return Input::Nothing,
                },
                _ => return Input::Nothing,
            }
        }
        _ => return Input::Nothing,
    };
    Input::Game(KeyEvent::new(code, m))
}

fn cmd_chord(bare: &Key, mods: ModifiersState) -> Input {
    let Key::Character(s) = bare else {
        return Input::Nothing;
    };
    let shortcut = match s.as_str() {
        "q" => Shortcut::Quit,
        "f" if mods.control_key() => Shortcut::Fullscreen,
        "=" | "+" => Shortcut::Bigger,
        "-" => Shortcut::Smaller,
        "0" => Shortcut::ResetSize,
        _ => return Input::Nothing,
    };
    Input::Window(shortcut)
}

fn named_code(key: NamedKey, shift: bool) -> Option<KeyCode> {
    Some(match key {
        NamedKey::Space => KeyCode::Char(' '),
        NamedKey::Enter => KeyCode::Enter,
        NamedKey::Escape => KeyCode::Esc,
        NamedKey::Tab if shift => KeyCode::BackTab,
        NamedKey::Tab => KeyCode::Tab,
        NamedKey::Backspace => KeyCode::Backspace,
        NamedKey::Delete => KeyCode::Delete,
        NamedKey::Insert => KeyCode::Insert,
        NamedKey::ArrowLeft => KeyCode::Left,
        NamedKey::ArrowRight => KeyCode::Right,
        NamedKey::ArrowUp => KeyCode::Up,
        NamedKey::ArrowDown => KeyCode::Down,
        NamedKey::Home => KeyCode::Home,
        NamedKey::End => KeyCode::End,
        NamedKey::PageUp => KeyCode::PageUp,
        NamedKey::PageDown => KeyCode::PageDown,
        NamedKey::F1 => KeyCode::F(1),
        NamedKey::F2 => KeyCode::F(2),
        NamedKey::F3 => KeyCode::F(3),
        NamedKey::F4 => KeyCode::F(4),
        NamedKey::F5 => KeyCode::F(5),
        NamedKey::F6 => KeyCode::F(6),
        NamedKey::F7 => KeyCode::F(7),
        NamedKey::F8 => KeyCode::F(8),
        NamedKey::F9 => KeyCode::F(9),
        NamedKey::F10 => KeyCode::F(10),
        NamedKey::F11 => KeyCode::F(11),
        NamedKey::F12 => KeyCode::F(12),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn ch(s: &str) -> Key {
        Key::Character(s.into())
    }

    fn game(code: KeyCode, m: KeyModifiers) -> Input {
        Input::Game(KeyEvent::new(code, m))
    }

    const NONE: ModifiersState = ModifiersState::empty();

    #[test]
    fn letters_and_shift() {
        assert_eq!(
            translate(true, &ch("a"), &ch("a"), NONE),
            game(KeyCode::Char('a'), KeyModifiers::NONE)
        );
        assert_eq!(
            translate(true, &ch("A"), &ch("a"), ModifiersState::SHIFT),
            game(KeyCode::Char('A'), KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn named_keys() {
        let cases = [
            (NamedKey::Space, KeyCode::Char(' ')),
            (NamedKey::Enter, KeyCode::Enter),
            (NamedKey::Escape, KeyCode::Esc),
            (NamedKey::Tab, KeyCode::Tab),
            (NamedKey::Backspace, KeyCode::Backspace),
            (NamedKey::Delete, KeyCode::Delete),
            (NamedKey::ArrowLeft, KeyCode::Left),
            (NamedKey::ArrowRight, KeyCode::Right),
            (NamedKey::ArrowUp, KeyCode::Up),
            (NamedKey::ArrowDown, KeyCode::Down),
            (NamedKey::Home, KeyCode::Home),
            (NamedKey::End, KeyCode::End),
            (NamedKey::PageUp, KeyCode::PageUp),
            (NamedKey::PageDown, KeyCode::PageDown),
            (NamedKey::F5, KeyCode::F(5)),
        ];
        for (named, code) in cases {
            let k = Key::Named(named);
            assert_eq!(
                translate(true, &k, &k, NONE),
                game(code, KeyModifiers::NONE),
                "{named:?}"
            );
        }
        let tab = Key::Named(NamedKey::Tab);
        assert_eq!(
            translate(true, &tab, &tab, ModifiersState::SHIFT),
            game(KeyCode::BackTab, KeyModifiers::SHIFT)
        );
        let up = Key::Named(NamedKey::ArrowUp);
        assert_eq!(
            translate(true, &up, &up, ModifiersState::SHIFT),
            game(KeyCode::Up, KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn ctrl_and_alt_use_the_bare_key() {
        assert_eq!(
            translate(true, &ch("\u{3}"), &ch("c"), ModifiersState::CONTROL),
            game(KeyCode::Char('c'), KeyModifiers::CONTROL)
        );
        assert_eq!(
            translate(true, &ch("å"), &ch("a"), ModifiersState::ALT),
            game(KeyCode::Char('a'), KeyModifiers::ALT)
        );
    }

    #[test]
    fn cmd_chords_are_the_window_s() {
        let cmd = ModifiersState::SUPER;
        let w = |s: &str, m| translate(true, &ch(s), &ch(s), m);
        assert_eq!(w("q", cmd), Input::Window(Shortcut::Quit));
        assert_eq!(
            w("f", cmd | ModifiersState::CONTROL),
            Input::Window(Shortcut::Fullscreen)
        );
        assert_eq!(w("f", cmd), Input::Nothing);
        assert_eq!(w("=", cmd), Input::Window(Shortcut::Bigger));
        assert_eq!(w("+", cmd), Input::Window(Shortcut::Bigger));
        assert_eq!(w("-", cmd), Input::Window(Shortcut::Smaller));
        assert_eq!(w("0", cmd), Input::Window(Shortcut::ResetSize));
        assert_eq!(w("k", cmd), Input::Nothing);
        let left = Key::Named(NamedKey::ArrowLeft);
        assert_eq!(translate(true, &left, &left, cmd), Input::Nothing);
    }

    #[test]
    fn repeat_is_press_release_is_nothing() {
        assert_eq!(translate(false, &ch("a"), &ch("a"), NONE), Input::Nothing);
        let Input::Game(k) = translate(true, &ch("a"), &ch("a"), NONE) else {
            panic!("a game key");
        };
        assert_eq!(k.kind, KeyEventKind::Press);
    }
}
