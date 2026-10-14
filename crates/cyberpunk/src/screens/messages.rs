//! The MESSAGES popup: the whole log, scrollable.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::messages::Kind;
use crate::ui::chrome::{CYAN, DIM, MAGENTA, TEXT, frame, put, put_paragraph, solid};
use crate::ui::sidebar::wrap_text;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::cell::Cell;

pub struct Messages {
    /// First visible wrapped line; `usize::MAX` means the bottom.
    scroll: usize,
    /// (visible rows, total wrapped lines) from the last draw, so keys can page and clamp.
    extent: Cell<(usize, usize)>,
}

impl Messages {
    pub fn new(_tick: u64) -> Self {
        Messages {
            scroll: usize::MAX,
            extent: Cell::new((1, 0)),
        }
    }

    fn rect(area: Rect) -> Rect {
        let w = (area.width * 7 / 10).max(40).min(area.width);
        let h = (area.height * 7 / 10).max(10).min(area.height);
        Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        )
    }

    fn lines(st: &AppState, width: usize) -> Vec<Line<'static>> {
        if st.log.is_empty() {
            return vec![Line::from(Span::styled(
                "nothing yet",
                Style::default().fg(DIM),
            ))];
        }
        let mut out = Vec::new();
        for e in st.log.iter() {
            let style = match e.kind {
                Kind::Info => Style::default().fg(TEXT),
                Kind::Warn => Style::default().fg(MAGENTA),
                Kind::Story => Style::default().fg(CYAN),
            };
            for row in wrap_text(&e.marked(), width) {
                out.push(Line::from(Span::styled(row, style)));
            }
        }
        out
    }

    fn scroll_by(&mut self, delta: i64) {
        let (rows, total) = self.extent.get();
        let max = total.saturating_sub(rows);
        let cur = self.scroll.min(max) as i64;
        self.scroll = (cur + delta).clamp(0, max as i64) as usize;
    }
}

impl Screen for Messages {
    fn update(&mut self, _st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            let rows = self.extent.get().0 as i64;
            match k.code {
                KeyCode::Down | KeyCode::Char('j') => self.scroll_by(1),
                KeyCode::Up | KeyCode::Char('k') => self.scroll_by(-1),
                KeyCode::PageDown => self.scroll_by(rows),
                KeyCode::PageUp => self.scroll_by(-rows),
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('m') => {
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
        let r = Self::rect(f.area());
        if r.width < 6 || r.height < 4 {
            return;
        }
        let buf = f.buffer_mut();
        solid(buf, r);
        frame(buf, r);
        put(
            buf,
            r.x + 2,
            r.y,
            " MESSAGES ",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            r.right() - 1,
        );
        let count = format!(" {} entries ", st.log.len());
        put(
            buf,
            r.right().saturating_sub(count.chars().count() as u16 + 2),
            r.y,
            &count,
            Style::default().fg(DIM),
            r.right() - 1,
        );
        let inner = Rect::new(r.x + 2, r.y + 2, r.width - 4, r.height - 4);
        let lines = Self::lines(st, inner.width as usize);
        let rows = inner.height as usize;
        self.extent.set((rows.max(1), lines.len()));
        let max = lines.len().saturating_sub(rows);
        let start = self.scroll.min(max);
        let shown: Vec<Line<'static>> = lines.into_iter().skip(start).take(rows).collect();
        put_paragraph(buf, inner, shown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn draw(m: &Messages, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| m.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn messages_popup_handles_an_empty_log() {
        let mut st = AppState::new();
        let mut m = Messages::new(0);
        let text = draw(&m, &st, 100, 30);
        assert!(
            text.contains("MESSAGES") && text.contains("0 entries") && text.contains("nothing yet"),
            "{text}"
        );
        for k in [
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::PageDown,
            KeyCode::PageUp,
        ] {
            assert!(matches!(m.update(&mut st, &[key(k)]), ScreenCmd::Stay));
        }
        assert!(matches!(
            m.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(matches!(
            m.update(&mut st, &[key(KeyCode::Char('m'))]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn messages_popup_lists_and_scrolls() {
        let mut st = AppState::new();
        for i in 0..40 {
            st.say(format!("line {i}"));
        }
        st.warn("no");
        let mut m = Messages::new(0);
        let text = draw(&m, &st, 100, 30);
        assert!(
            text.contains("41 entries") && text.contains("! no") && text.contains("line 39"),
            "{text}"
        );
        assert!(!text.contains("line 0 "), "opens at the bottom: {text}");
        for _ in 0..3 {
            m.update(&mut st, &[key(KeyCode::PageUp)]);
        }
        let text = draw(&m, &st, 100, 30);
        assert!(text.contains("line 0 ") && !text.contains("! no"), "{text}");
        for _ in 0..100 {
            m.update(&mut st, &[key(KeyCode::Char('j'))]);
        }
        assert!(draw(&m, &st, 100, 30).contains("! no"));
    }
}
