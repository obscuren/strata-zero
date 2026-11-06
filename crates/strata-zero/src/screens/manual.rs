//! MANUAL: the booklet from the box, read at your desk. It opens with you at the desk
//! (`manual::desk`), the cover on the screen; space leans in until the screen is the
//! booklet: two pages at a time on the game's own dark, stapled at the spine, the
//! contents on page 1, every chapter opening on a page of its own (`manual::book`).

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::manual::book::{Book, Page, book};
use crate::manual::chapters;
use crate::manual::desk::Desk;
use crate::ui::chrome::{CYAN, DIM, FAINT, MAGENTA, PANEL_BG, RULE, TEXT};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

/// A page, its frame and margins included.
const PAGE_W: u16 = 46;
const PAGE_H: u16 = 28;
/// The spine between the pages.
const SPINE_W: u16 = 2;
/// Two pages and the spine.
const SPREAD_W: u16 = 2 * PAGE_W + SPINE_W;
/// Where a page's text starts, from its top left.
const TEXT_X: u16 = 3;
const TEXT_Y: u16 = 4;
/// How long leaning in or out takes: about 0.7 s.
pub const LEAN_TICKS: u64 = 22;

/// The paper, the spine and its staples.
const SHEET: Color = Color::Rgb(10, 14, 21);
const SPINE: Color = Color::Rgb(21, 28, 39);
const STAPLE: Color = Color::Rgb(140, 150, 164);
/// A contents line's dots.
const LEADER: Color = Color::Rgb(58, 70, 86);

/// Where you are with the booklet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    /// Sat back at the desk, the cover on the screen.
    Desk,
    /// Leaned in, open on spread `n`: pages 2n-1 and 2n.
    Spread(usize),
}

/// A lean in flight: from which tick, and which way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Lean {
    start: u64,
    inward: bool,
}

pub struct Manual {
    at: At,
    /// The chapter the contents' cursor is on.
    pick: usize,
    lean: Option<Lean>,
}

impl Manual {
    /// You at your desk, the cover on the screen.
    pub fn new() -> Self {
        Manual {
            at: At::Desk,
            pick: 0,
            lean: None,
        }
    }

    /// Leaned in already, the booklet open where chapter `slug` begins; the desk for a
    /// chapter it doesn't have.
    pub fn at_chapter(slug: &str) -> Self {
        match crate::manual::find(slug) {
            Some(c) => Manual {
                at: At::Spread(Book::spread_of(book().starts[c])),
                pick: c,
                lean: None,
            },
            None => Manual::new(),
        }
    }

    /// Opens on the spread where chapter `c` begins, its line picked in the contents.
    fn turn_to(&mut self, c: usize) {
        self.pick = c;
        self.at = At::Spread(Book::spread_of(book().starts[c]));
    }

    /// How far leaned in at `tick`: 0 at the desk, 1 in the booklet, eased between.
    fn leaned(&self, tick: u64) -> f32 {
        let Some(l) = self.lean else {
            return if self.at == At::Desk { 0.0 } else { 1.0 };
        };
        let p = (tick.saturating_sub(l.start) as f32 / LEAN_TICKS as f32).min(1.0);
        let p = if l.inward { p } else { 1.0 - p };
        if p < 0.5 {
            4.0 * p * p * p
        } else {
            1.0 - (-2.0 * p + 2.0).powi(3) / 2.0
        }
    }

    fn keys_line(&self) -> &'static str {
        match (self.lean, self.at) {
            (Some(_), _) => "",
            (None, At::Desk) => "space read the manual · esc close",
            (None, At::Spread(1)) => {
                "↑↓ choose · space turn to it · → turn the page · esc lean back · q close"
            }
            (None, At::Spread(_)) => {
                "← → turn the page · [ ] chapters · c contents · esc lean back · q close"
            }
        }
    }
}

impl Default for Manual {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for Manual {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // a lean done: in, you're in the booklet; out, you're back at the desk
        if let Some(l) = self.lean
            && st.tick >= l.start + LEAN_TICKS
        {
            if !l.inward {
                self.at = At::Desk;
            }
            self.lean = None;
        }
        let book = book();
        let last = book.spreads();
        let count = chapters().len();
        for k in keys {
            if k.code == KeyCode::Char('q') {
                return ScreenCmd::Pop {
                    fx: TransitionKind::None,
                };
            }
            if self.lean.is_some() {
                continue;
            }
            let lean_out = Some(Lean {
                start: st.tick,
                inward: false,
            });
            match (self.at, k.code) {
                (At::Desk, KeyCode::Esc) => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                (
                    At::Desk,
                    KeyCode::Char(' ')
                    | KeyCode::Enter
                    | KeyCode::Right
                    | KeyCode::Char('l')
                    | KeyCode::PageDown,
                ) => {
                    self.at = At::Spread(1);
                    self.lean = Some(Lean {
                        start: st.tick,
                        inward: true,
                    });
                }
                (At::Spread(_), KeyCode::Esc) => self.lean = lean_out,
                (At::Spread(1), KeyCode::Left | KeyCode::Char('h') | KeyCode::PageUp) => {
                    self.lean = lean_out;
                }
                (At::Spread(s), KeyCode::Left | KeyCode::Char('h') | KeyCode::PageUp) => {
                    self.at = At::Spread(s - 1);
                }
                (At::Spread(s), KeyCode::Right | KeyCode::Char('l') | KeyCode::PageDown) => {
                    self.at = At::Spread((s + 1).min(last));
                }
                (At::Spread(_), KeyCode::Char('c')) => self.at = At::Spread(1),
                (At::Spread(s), KeyCode::Char(']')) => {
                    match (0..count).find(|&c| Book::spread_of(book.starts[c]) > s) {
                        Some(c) => self.turn_to(c),
                        None => self.at = At::Spread(last),
                    }
                }
                (At::Spread(s), KeyCode::Char('[')) => {
                    match (0..count)
                        .rev()
                        .find(|&c| Book::spread_of(book.starts[c]) < s)
                    {
                        Some(c) => self.turn_to(c),
                        None => self.at = At::Spread(1),
                    }
                }
                (At::Spread(1), KeyCode::Up | KeyCode::Char('k')) => {
                    self.pick = self.pick.saturating_sub(1);
                }
                (At::Spread(1), KeyCode::Down | KeyCode::Char('j')) => {
                    self.pick = (self.pick + 1).min(count.saturating_sub(1));
                }
                (At::Spread(1), KeyCode::Char(' ') | KeyCode::Enter) => self.turn_to(self.pick),
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        if area.width < SPREAD_W || area.height < PAGE_H + 1 {
            return;
        }
        let x = area.x + (area.width - SPREAD_W) / 2;
        let y = area.y + (area.height - PAGE_H - 1) / 2;
        let buf = f.buffer_mut();
        let desk = Desk::new(area.width, area.height);
        let booklet = Rect::new(x, y, SPREAD_W, PAGE_H);
        match (self.lean, self.at) {
            (None, At::Desk) => desk.draw(buf, area, st.tick),
            (None, At::Spread(s)) => {
                desk.draw_leaned(buf, area, st.tick, 1.0, booklet);
                spread(buf, x, y, s, self.pick);
            }
            (Some(_), _) => {
                desk.draw_leaned(buf, area, st.tick, self.leaned(st.tick), booklet);
            }
        }
        let keys = self.keys_line();
        if keys.is_empty() {
            return;
        }
        // the keys line on a strip of its own, so nothing beneath reads through it: under
        // the booklet, or along the bottom at the desk
        let w = keys.chars().count() as u16;
        let strip = if self.at == At::Desk {
            Rect::new(
                area.x + area.width.saturating_sub(w + 2) / 2,
                area.bottom() - 1,
                (w + 2).min(area.width),
                1,
            )
        } else {
            Rect::new(x, y + PAGE_H, SPREAD_W, 1)
        };
        for cx in strip.x..strip.right() {
            buf[(cx, strip.y)].reset();
            buf[(cx, strip.y)].set_bg(PANEL_BG);
        }
        put(
            buf,
            area.x + area.width.saturating_sub(w) / 2,
            strip.y,
            keys,
            Style::default().fg(DIM).bg(PANEL_BG),
        );
    }
}

/// Writes `text` from (x, y) over a solid backing, clipped to the buffer.
fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style) {
    let area = buf.area;
    if y < area.y || y >= area.bottom() {
        return;
    }
    let style = if style.bg.is_none() {
        style.bg(SHEET)
    } else {
        style
    };
    for (cx, ch) in (x..area.right()).zip(text.chars()) {
        if cx >= area.x {
            buf[(cx, y)].set_char(ch).set_style(style);
        }
    }
}

/// Writes `line`'s spans from (x, y) on the page.
fn put_line(buf: &mut Buffer, x: u16, y: u16, line: &Line) {
    let mut cx = x;
    for span in &line.spans {
        put(buf, cx, y, &span.content, span.style.bg(SHEET));
        cx += span.content.chars().count() as u16;
    }
}

/// A blank sheet: every cell reset onto the paper.
fn sheet(buf: &mut Buffer, r: Rect) {
    let r = r.intersection(buf.area);
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_bg(SHEET);
        }
    }
}

/// The booklet open on spread `s`: two pages and the spine, staples in it.
fn spread(buf: &mut Buffer, x: u16, y: u16, s: usize, pick: usize) {
    let left = Rect::new(x, y, PAGE_W, PAGE_H);
    let right = Rect::new(x + PAGE_W + SPINE_W, y, PAGE_W, PAGE_H);
    sheet(buf, left);
    sheet(buf, right);
    let sx = x + PAGE_W;
    for j in 0..PAGE_H {
        buf[(sx, y + j)].set_char('▐').set_fg(SPINE).set_bg(SHEET);
        buf[(sx + 1, y + j)]
            .set_char('▌')
            .set_fg(SPINE)
            .set_bg(SHEET);
    }
    for j in [6, PAGE_H - 7] {
        put(buf, sx, y + j, "▐▌", Style::default().fg(STAPLE).bg(SPINE));
    }
    page(buf, left, 2 * s - 1, pick, true);
    page(buf, right, 2 * s, pick, false);
}

/// Page `n` in `r`: the contents, a chapter's page, or NOTES past the last.
fn page(buf: &mut Buffer, r: Rect, n: usize, pick: usize, left: bool) {
    let book = book();
    if n == 1 {
        contents(buf, r, pick);
    } else if let Some(p) = book.page(n) {
        chapter_page(buf, r, p);
    } else {
        notes(buf, r);
    }
    // the page number in the outer bottom corner
    let num = n.to_string();
    let nx = if left {
        r.x + 2
    } else {
        r.right() - 2 - num.chars().count() as u16
    };
    put(buf, nx, r.y + PAGE_H - 1, &num, Style::default().fg(FAINT));
}

/// A heading at the top of a page: `title` in cyan over a short magenta rule, `num`
/// before it in magenta.
fn heading(buf: &mut Buffer, r: Rect, num: Option<usize>, title: &str) {
    let mut x = r.x + TEXT_X;
    if let Some(n) = num {
        let n = n.to_string();
        put(
            buf,
            x,
            r.y + 2,
            &n,
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
        );
        x += n.chars().count() as u16 + 1;
    }
    let title = title.to_uppercase();
    put(
        buf,
        x,
        r.y + 2,
        &title,
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    );
    let len = (x - r.x - TEXT_X) as usize + title.chars().count();
    put(
        buf,
        r.x + TEXT_X,
        r.y + 3,
        &"━".repeat(len.min(8)),
        Style::default().fg(MAGENTA),
    );
}

/// Page 1: every chapter, the page it opens on, the picked one lit.
fn contents(buf: &mut Buffer, r: Rect, pick: usize) {
    heading(buf, r, None, "Contents");
    let book = book();
    let w = (PAGE_W - 4) as usize;
    for (i, ch) in chapters().iter().enumerate() {
        let y = r.y + TEXT_Y + 1 + i as u16;
        let x = r.x + 2;
        let num = format!("{:>2}", i + 1);
        let page = book.starts[i].to_string();
        let dots = w.saturating_sub(4 + ch.title.chars().count() + page.len() + 3);
        if i == pick {
            let line = format!(
                "▸{num}  {} {} {page}",
                ch.title,
                ".".repeat(dots.saturating_sub(1))
            );
            let lit = Style::default()
                .fg(PANEL_BG)
                .bg(CYAN)
                .add_modifier(Modifier::BOLD);
            put(buf, x, y, &format!("{line:<w$}"), lit);
        } else {
            put(buf, x + 1, y, &num, Style::default().fg(CYAN));
            put(buf, x + 5, y, &ch.title, Style::default().fg(TEXT));
            let dx = x + 5 + ch.title.chars().count() as u16 + 1;
            put(buf, dx, y, &".".repeat(dots), Style::default().fg(LEADER));
            put(
                buf,
                x + w as u16 - page.len() as u16,
                y,
                &page,
                Style::default().fg(TEXT),
            );
        }
    }
}

/// A chapter's page: its number and title on its first page, its title as a running
/// head on the rest, then its rows.
fn chapter_page(buf: &mut Buffer, r: Rect, p: &Page) {
    let title = &chapters()[p.chapter].title;
    if p.opens {
        heading(buf, r, Some(p.chapter + 1), title);
    } else {
        put(
            buf,
            r.x + 2,
            r.y + 1,
            &title.to_uppercase(),
            Style::default().fg(FAINT),
        );
        put(
            buf,
            r.x + 2,
            r.y + 2,
            &"─".repeat((PAGE_W - 4) as usize),
            Style::default().fg(RULE),
        );
    }
    for (i, line) in p.rows.iter().enumerate() {
        put_line(buf, r.x + TEXT_X, r.y + TEXT_Y + i as u16, line);
    }
}

/// The page facing an odd last one: NOTES, and dotted lines to write on.
fn notes(buf: &mut Buffer, r: Rect) {
    heading(buf, r, None, "Notes");
    let w = (PAGE_W - 2 * TEXT_X) as usize;
    let mut y = r.y + TEXT_Y + 1;
    while y < r.y + PAGE_H - 3 {
        put(
            buf,
            r.x + TEXT_X,
            y,
            &"·".repeat(w),
            Style::default().fg(LEADER),
        );
        y += 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn draw(m: &Manual, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| m.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    fn press(m: &mut Manual, st: &mut AppState, code: KeyCode) -> ScreenCmd {
        m.update(st, &[key(code)])
    }

    #[test]
    fn the_booklet_fits_every_size_without_panicking() {
        let mut st = AppState::new();
        let mut m = Manual::new();
        let mut stx = AppState::new();
        for at in 0..6 {
            st.tick += 7;
            for (w, h) in [
                (100u16, 30u16),
                (94, 29),
                (93, 28),
                (20, 5),
                (200, 60),
                (1, 1),
            ] {
                draw(&m, &st, w, h);
            }
            press(&mut m, &mut stx, KeyCode::Right);
            let _ = at;
        }
    }

    /// Lets a lean in flight finish: the ticks go by and a frame passes.
    fn settle(m: &mut Manual, st: &mut AppState) {
        st.tick += LEAN_TICKS;
        m.update(st, &[]);
    }

    #[test]
    fn the_desk_leans_in_to_the_contents() {
        let mut st = AppState::new();
        let mut m = Manual::new();
        let desk = draw(&m, &st, 100, 30);
        assert!(
            desk.contains("PLAYER'S MANUAL") && desk.contains("space read the manual"),
            "{desk}"
        );
        assert!(matches!(
            press(&mut m, &mut st, KeyCode::Char(' ')),
            ScreenCmd::Stay
        ));
        // on the way in: no words yet, keys wait
        st.tick += LEAN_TICKS / 2;
        let halfway = draw(&m, &st, 100, 30);
        assert!(!halfway.contains("CONTENTS"), "{halfway}");
        assert!(m.leaned(st.tick) > 0.0 && m.leaned(st.tick) < 1.0);
        press(&mut m, &mut st, KeyCode::Down);
        assert_eq!(m.pick, 0, "keys wait for the lean");
        settle(&mut m, &mut st);
        let open = draw(&m, &st, 100, 30);
        assert!(
            open.contains("CONTENTS") && open.contains("esc lean back"),
            "{open}"
        );
        for ch in chapters() {
            assert!(open.contains(&ch.title), "{} missing:\n{open}", ch.title);
        }
    }

    #[test]
    fn esc_leans_back_and_esc_at_the_desk_closes() {
        let mut st = AppState::new();
        let mut m = Manual::at_chapter("controls");
        assert!(matches!(
            press(&mut m, &mut st, KeyCode::Esc),
            ScreenCmd::Stay
        ));
        settle(&mut m, &mut st);
        assert_eq!((m.at, m.lean), (At::Desk, None));
        assert!(draw(&m, &st, 120, 40).contains("PLAYER'S MANUAL"));
        assert!(matches!(
            press(&mut m, &mut st, KeyCode::Esc),
            ScreenCmd::Pop { .. }
        ));
        // q closes from anywhere, mid-lean included
        let mut m = Manual::new();
        press(&mut m, &mut st, KeyCode::Char(' '));
        assert!(matches!(
            press(&mut m, &mut st, KeyCode::Char('q')),
            ScreenCmd::Pop { .. }
        ));
        // left past the contents leans back out too
        let mut m = Manual::at_chapter("getting-started");
        press(&mut m, &mut st, KeyCode::Left);
        settle(&mut m, &mut st);
        assert_eq!(m.at, At::Desk);
    }

    #[test]
    fn the_contents_turn_to_the_picked_chapter() {
        let mut st = AppState::new();
        let mut m = Manual::new();
        press(&mut m, &mut st, KeyCode::Char(' '));
        settle(&mut m, &mut st);
        let to = chapters().len() - 1;
        for _ in 0..to + 3 {
            press(&mut m, &mut st, KeyCode::Down);
        }
        assert_eq!(m.pick, to, "the cursor stops on the last chapter");
        press(&mut m, &mut st, KeyCode::Char(' '));
        let start = book().starts[to];
        assert_eq!(m.at, At::Spread(Book::spread_of(start)));
        let text = draw(&m, &st, 100, 30);
        assert!(
            text.contains(&chapters()[to].title.to_uppercase()),
            "{text}"
        );
        // c comes back to the contents, the chapter still picked
        press(&mut m, &mut st, KeyCode::Char('c'));
        assert_eq!((m.at, m.pick), (At::Spread(1), to));
    }

    #[test]
    fn pages_turn_and_stop_at_the_ends() {
        let mut st = AppState::new();
        let mut m = Manual::at_chapter("getting-started");
        assert_eq!((m.at, m.lean), (At::Spread(1), None), "already leaned in");
        let last = book().spreads();
        for _ in 0..last + 5 {
            press(&mut m, &mut st, KeyCode::Right);
        }
        assert_eq!(m.at, At::Spread(last));
        for _ in 0..last - 1 {
            press(&mut m, &mut st, KeyCode::Left);
        }
        assert_eq!(m.at, At::Spread(1));
        assert_eq!(Manual::at_chapter("no-such-chapter").at, At::Desk);
    }

    #[test]
    fn brackets_go_chapter_to_chapter() {
        let mut st = AppState::new();
        let mut m = Manual::at_chapter("getting-started");
        let starts = &book().starts;
        press(&mut m, &mut st, KeyCode::Char(']'));
        let next = (0..starts.len())
            .find(|&c| Book::spread_of(starts[c]) > 1)
            .unwrap();
        assert_eq!(
            (m.at, m.pick),
            (At::Spread(Book::spread_of(starts[next])), next)
        );
        press(&mut m, &mut st, KeyCode::Char('['));
        assert_eq!(m.at, At::Spread(1));
    }

    #[test]
    fn every_page_draws_inside_its_sheet() {
        // text never runs past a page's right margin onto the spine or off the sheet
        let st = AppState::new();
        let mut stx = AppState::new();
        let mut m = Manual::at_chapter("getting-started");
        for _ in 0..book().spreads() {
            let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
            t.draw(|f| m.draw(&st, f)).unwrap();
            let buf = t.backend().buffer();
            let x = (100 - SPREAD_W) / 2;
            let y = (30 - PAGE_H - 1) / 2;
            for j in 0..PAGE_H {
                for sx in [x + PAGE_W, x + PAGE_W + 1] {
                    let s = buf[(sx, y + j)].symbol();
                    assert!(s == "▐" || s == "▌", "spine at ({sx},{j}) is {s:?}");
                }
            }
            press(&mut m, &mut stx, KeyCode::Right);
        }
    }
}
