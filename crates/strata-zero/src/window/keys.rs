//! Window key presses as the crossterm events every screen already reads, and the chords
//! that belong to the window: Cmd chords on a Mac, Ctrl, F11 and Alt+Enter on a PC.

use super::platform::Platform;
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

/// A key event on `platform`: `pressed` is false for a release (a held key's repeats are
/// presses, as a terminal's are); `logical` is the key with every modifier applied,
/// `bare` the key without them. Ctrl chords read the bare key (Ctrl+C is `c`).
///
/// On a Mac, Cmd chords are the window's, and Option types what the layout puts there,
/// as Terminal.app does: `@` and `[` on German keyboards, `å` on a US one, with no ALT
/// modifier, since no screen reads Alt chords and nano drops them.
///
/// On a PC, Ctrl+= / Ctrl+- / Ctrl+0, F11 and Alt+Enter are the window's, and chords
/// with the Windows key do nothing. AltGr types its character: winit reports the right
/// Alt of an AltGr layout as no modifier, but Windows also takes left Ctrl+Alt for
/// AltGr, so Ctrl+Alt on a key whose character isn't the bare key's types that
/// character. Alt otherwise carries ALT, as a terminal reports it.
pub fn translate(
    pressed: bool,
    logical: &Key,
    bare: &Key,
    mods: ModifiersState,
    platform: Platform,
) -> Input {
    if !pressed {
        return Input::Nothing;
    }
    let pc = platform != Platform::Mac;
    if mods.super_key() {
        return if pc {
            Input::Nothing
        } else {
            cmd_chord(bare, mods)
        };
    }
    if pc && let Some(shortcut) = pc_chord(logical, bare, mods) {
        return Input::Window(shortcut);
    }
    let altgr = pc && mods.control_key() && mods.alt_key() && types_other(logical, bare);
    let mut m = KeyModifiers::NONE;
    if mods.shift_key() && !altgr {
        m |= KeyModifiers::SHIFT;
    }
    if mods.control_key() && !altgr {
        m |= KeyModifiers::CONTROL;
    }
    if mods.alt_key() && (mods.control_key() || pc) && !altgr {
        m |= KeyModifiers::ALT;
    }
    let code = match logical {
        Key::Named(named) => match named_code(*named, mods.shift_key()) {
            Some(code) => code,
            None => return Input::Nothing,
        },
        Key::Character(_) => {
            let key = if mods.control_key() && !altgr {
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

/// The window's chords on a PC.
fn pc_chord(logical: &Key, bare: &Key, mods: ModifiersState) -> Option<Shortcut> {
    match logical {
        Key::Named(NamedKey::F11) => return Some(Shortcut::Fullscreen),
        Key::Named(NamedKey::Enter) if mods.alt_key() && !mods.control_key() => {
            return Some(Shortcut::Fullscreen);
        }
        _ => {}
    }
    if !mods.control_key() || mods.alt_key() {
        return None;
    }
    let Key::Character(s) = bare else {
        return None;
    };
    Some(match s.as_str() {
        "=" | "+" => Shortcut::Bigger,
        "-" => Shortcut::Smaller,
        "0" => Shortcut::ResetSize,
        _ => return None,
    })
}

/// Whether `logical` types a character other than the bare key's (Shift aside): what
/// AltGr does.
fn types_other(logical: &Key, bare: &Key) -> bool {
    match (logical, bare) {
        (Key::Character(l), Key::Character(b)) => l.to_lowercase() != b.to_lowercase(),
        _ => false,
    }
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
    const CTRL: ModifiersState = ModifiersState::CONTROL;
    const ALT: ModifiersState = ModifiersState::ALT;
    const SHIFT: ModifiersState = ModifiersState::SHIFT;

    fn mac(pressed: bool, logical: &Key, bare: &Key, mods: ModifiersState) -> Input {
        translate(pressed, logical, bare, mods, Platform::Mac)
    }

    /// `check` for each PC platform, with a translate for it.
    fn on_pcs(check: impl Fn(&dyn Fn(&Key, &Key, ModifiersState) -> Input)) {
        for p in [Platform::Windows, Platform::Linux] {
            check(&|logical, bare, mods| translate(true, logical, bare, mods, p));
        }
    }

    #[test]
    fn pc_zoom_chords() {
        on_pcs(|t| {
            assert_eq!(t(&ch("="), &ch("="), CTRL), Input::Window(Shortcut::Bigger));
            assert_eq!(
                t(&ch("+"), &ch("="), CTRL | SHIFT),
                Input::Window(Shortcut::Bigger)
            );
            assert_eq!(
                t(&ch("-"), &ch("-"), CTRL),
                Input::Window(Shortcut::Smaller)
            );
            assert_eq!(
                t(&ch("0"), &ch("0"), CTRL),
                Input::Window(Shortcut::ResetSize)
            );
            assert_eq!(
                t(&ch("="), &ch("="), NONE),
                game(KeyCode::Char('='), KeyModifiers::NONE)
            );
        });
    }

    #[test]
    fn pc_fullscreen() {
        let f11 = Key::Named(NamedKey::F11);
        let enter = Key::Named(NamedKey::Enter);
        on_pcs(|t| {
            assert_eq!(t(&f11, &f11, NONE), Input::Window(Shortcut::Fullscreen));
            assert_eq!(t(&enter, &enter, ALT), Input::Window(Shortcut::Fullscreen));
            assert_eq!(
                t(&enter, &enter, NONE),
                game(KeyCode::Enter, KeyModifiers::NONE)
            );
        });
        // On a Mac F11 stays the game's.
        assert_eq!(
            mac(true, &f11, &f11, NONE),
            game(KeyCode::F(11), KeyModifiers::NONE)
        );
    }

    #[test]
    fn pc_altgr_types() {
        on_pcs(|t| {
            // German layout: AltGr+Q is `@`, AltGr+8 is `[`.
            assert_eq!(
                t(&ch("@"), &ch("q"), CTRL | ALT),
                game(KeyCode::Char('@'), KeyModifiers::NONE)
            );
            assert_eq!(
                t(&ch("["), &ch("8"), CTRL | ALT),
                game(KeyCode::Char('['), KeyModifiers::NONE)
            );
            // The right Alt of an AltGr layout comes with no modifier at all.
            assert_eq!(
                t(&ch("@"), &ch("q"), NONE),
                game(KeyCode::Char('@'), KeyModifiers::NONE)
            );
        });
    }

    #[test]
    fn pc_ctrl_alt_chord() {
        on_pcs(|t| {
            let both = KeyModifiers::CONTROL | KeyModifiers::ALT;
            assert_eq!(
                t(&ch("a"), &ch("a"), CTRL | ALT),
                game(KeyCode::Char('a'), both)
            );
            // Shift doesn't make a chord into AltGr.
            assert_eq!(
                t(&ch("A"), &ch("a"), CTRL | ALT | SHIFT),
                game(KeyCode::Char('a'), both | KeyModifiers::SHIFT)
            );
        });
    }

    #[test]
    fn pc_alt_carries_alt() {
        on_pcs(|t| {
            assert_eq!(
                t(&ch("x"), &ch("x"), ALT),
                game(KeyCode::Char('x'), KeyModifiers::ALT)
            );
        });
    }

    #[test]
    fn pc_game_ctrl_letters() {
        on_pcs(|t| {
            for c in ['c', 'x', 'o', 'k', 'u', 'w'] {
                let s = c.to_string();
                assert_eq!(
                    t(&ch(&s), &ch(&s), CTRL),
                    game(KeyCode::Char(c), KeyModifiers::CONTROL)
                );
            }
        });
    }

    #[test]
    fn pc_super_does_nothing() {
        on_pcs(|t| {
            assert_eq!(t(&ch("q"), &ch("q"), ModifiersState::SUPER), Input::Nothing);
            assert_eq!(t(&ch("="), &ch("="), ModifiersState::SUPER), Input::Nothing);
        });
    }

    #[test]
    fn shift_tab_is_backtab_everywhere() {
        let tab = Key::Named(NamedKey::Tab);
        let backtab = game(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(mac(true, &tab, &tab, SHIFT), backtab);
        on_pcs(|t| assert_eq!(t(&tab, &tab, SHIFT), backtab));
    }

    #[test]
    fn letters_and_shift() {
        assert_eq!(
            mac(true, &ch("a"), &ch("a"), NONE),
            game(KeyCode::Char('a'), KeyModifiers::NONE)
        );
        assert_eq!(
            mac(true, &ch("A"), &ch("a"), ModifiersState::SHIFT),
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
                mac(true, &k, &k, NONE),
                game(code, KeyModifiers::NONE),
                "{named:?}"
            );
        }
        let tab = Key::Named(NamedKey::Tab);
        assert_eq!(
            mac(true, &tab, &tab, ModifiersState::SHIFT),
            game(KeyCode::BackTab, KeyModifiers::SHIFT)
        );
        let up = Key::Named(NamedKey::ArrowUp);
        assert_eq!(
            mac(true, &up, &up, ModifiersState::SHIFT),
            game(KeyCode::Up, KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn ctrl_uses_the_bare_key() {
        assert_eq!(
            mac(true, &ch("\u{3}"), &ch("c"), ModifiersState::CONTROL),
            game(KeyCode::Char('c'), KeyModifiers::CONTROL)
        );
    }

    #[test]
    fn option_types_what_the_layout_puts_there() {
        // German layout: Option+L is `@`, Option+5 is `[`.
        assert_eq!(
            mac(true, &ch("@"), &ch("l"), ModifiersState::ALT),
            game(KeyCode::Char('@'), KeyModifiers::NONE)
        );
        assert_eq!(
            mac(true, &ch("["), &ch("5"), ModifiersState::ALT),
            game(KeyCode::Char('['), KeyModifiers::NONE)
        );
        assert_eq!(
            mac(true, &ch("å"), &ch("a"), ModifiersState::ALT),
            game(KeyCode::Char('å'), KeyModifiers::NONE)
        );
    }

    #[test]
    fn cmd_chords_are_the_window_s() {
        let cmd = ModifiersState::SUPER;
        let w = |s: &str, m| mac(true, &ch(s), &ch(s), m);
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
        assert_eq!(mac(true, &left, &left, cmd), Input::Nothing);
    }

    #[test]
    fn repeat_is_press_release_is_nothing() {
        assert_eq!(mac(false, &ch("a"), &ch("a"), NONE), Input::Nothing);
        let Input::Game(k) = mac(true, &ch("a"), &ch("a"), NONE) else {
            panic!("a game key");
        };
        assert_eq!(k.kind, KeyEventKind::Press);
    }
}
