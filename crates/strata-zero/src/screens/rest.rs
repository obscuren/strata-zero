//! The Rest window: pick when to wake on a strip of the next day, then lie down (or
//! sit out the time). Opened by the bed's Sleep and the console's `wait`.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::rest::{self, Kind};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;

pub struct RestScreen {
    kind: Kind,
    /// The game minute you would wake.
    until: u64,
}

impl RestScreen {
    /// The window with the time it offers.
    pub fn open(st: &AppState, kind: Kind) -> RestScreen {
        RestScreen {
            kind,
            until: rest::default_wake(st, kind),
        }
    }

    /// Moves the wake time by `by` minutes, held to what a rest may be.
    fn shift(&mut self, st: &AppState, by: i64) {
        self.until = rest::clamp(st, self.until.saturating_add_signed(by));
    }
}

impl Screen for RestScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            match k.code {
                KeyCode::Char('h') | KeyCode::Left => self.shift(st, -60),
                KeyCode::Char('l') | KeyCode::Right => self.shift(st, 60),
                KeyCode::Char('H') => self.shift(st, -10),
                KeyCode::Char('L') => self.shift(st, 10),
                KeyCode::Enter => {
                    rest::start(st, self.kind, self.until);
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                KeyCode::Esc => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let map = super::explore::layout(f.area(), false).map;
        rest::draw_window(f.buffer_mut(), map, st, self.kind, self.until, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::screens::testutil::key;
    use crossterm::event::KeyCode;

    fn press(s: &mut RestScreen, st: &mut AppState, c: KeyCode) -> ScreenCmd {
        s.update(st, &[key(c)])
    }

    #[test]
    fn keys_move_the_wake_time() {
        let mut st = state_home();
        let mut s = RestScreen::open(&st, Kind::Sleep);
        let shown = s.until;
        assert_eq!(shown, rest::default_wake(&st, Kind::Sleep));
        press(&mut s, &mut st, KeyCode::Char('l'));
        assert_eq!(s.until, rest::clamp(&st, shown + 60));
        press(&mut s, &mut st, KeyCode::Char('h'));
        assert_eq!(s.until, shown);
        press(&mut s, &mut st, KeyCode::Char('L'));
        assert_eq!(s.until, shown + 10);
        press(&mut s, &mut st, KeyCode::Char('H'));
        assert_eq!(s.until, shown);
        press(&mut s, &mut st, KeyCode::Right);
        press(&mut s, &mut st, KeyCode::Left);
        press(&mut s, &mut st, KeyCode::Left);
        assert_eq!(s.until, shown - 60);
        let now = rest::now(&st);
        for _ in 0..30 {
            press(&mut s, &mut st, KeyCode::Char('h'));
        }
        assert_eq!(s.until, now + 10, "no earlier than ten minutes on");
        for _ in 0..30 {
            press(&mut s, &mut st, KeyCode::Char('l'));
        }
        assert_eq!(s.until, now + 1200, "no later than twenty hours on");
    }

    #[test]
    fn esc_goes_back_and_enter_lies_you_down() {
        let mut st = state_home();
        let mut s = RestScreen::open(&st, Kind::Sleep);
        assert!(matches!(
            press(&mut s, &mut st, KeyCode::Esc),
            ScreenCmd::Pop { .. }
        ));
        assert!(st.rest.is_none());
        press(&mut s, &mut st, KeyCode::Char('l'));
        assert!(matches!(
            press(&mut s, &mut st, KeyCode::Enter),
            ScreenCmd::Pop { .. }
        ));
        let rest = st.rest.as_ref().unwrap();
        assert_eq!((rest.kind, rest.until), (Kind::Sleep, s.until));
    }

    #[test]
    fn it_draws_over_the_map() {
        let st = state_home();
        let s = RestScreen::open(&st, Kind::Wait);
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
        term.draw(|f| s.draw(&st, f)).unwrap();
        let text = crate::screens::testutil::buffer_text(term.backend().buffer());
        assert!(
            text.contains("Wait") && text.contains("1h 0m waiting"),
            "{text}"
        );
    }
}
