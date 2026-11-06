//! MANUAL: the booklet from the box. The cover alone first, the closed booklet; then two
//! pages at a time on the game's own dark, stapled at the spine: the contents on page
//! 1, every chapter opening on a page of its own (`manual::book`).

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::manual::book::{Book, Page, book};
use crate::manual::chapters;
use crate::render::Rgb;
use crate::ui::chrome::{CYAN, DIM, FAINT, MAGENTA, MAGENTA_RGB, PANEL_BG, RULE, TEXT};
use crate::ui::cyberdeck::Pen;
use crate::ui::logo::{self, Logo};
use crate::ui::pixels::Pixels;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

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

/// The paper, the spine and its staples.
const SHEET: Color = Color::Rgb(10, 14, 21);
const SPINE: Color = Color::Rgb(21, 28, 39);
const STAPLE: Color = Color::Rgb(140, 150, 164);
/// The edge the closed booklet casts on what it lies on.
const EDGE: Color = Color::Rgb(31, 40, 54);
/// A contents line's dots.
const LEADER: Color = Color::Rgb(58, 70, 86);

/// Where the booklet is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    /// Closed, the cover up.
    Cover,
    /// Open on spread `n`: pages 2n-1 and 2n.
    Spread(usize),
}

pub struct Manual {
    at: At,
    /// The chapter the contents' cursor is on.
    pick: usize,
}

impl Manual {
    /// The booklet closed, the cover up.
    pub fn new() -> Self {
        Manual {
            at: At::Cover,
            pick: 0,
        }
    }

    /// The booklet open where chapter `slug` begins; the cover for one it doesn't have.
    pub fn at_chapter(slug: &str) -> Self {
        match crate::manual::find(slug) {
            Some(c) => Manual {
                at: At::Spread(Book::spread_of(book().starts[c])),
                pick: c,
            },
            None => Manual::new(),
        }
    }

    /// Opens on the spread where chapter `c` begins, its line picked in the contents.
    fn turn_to(&mut self, c: usize) {
        self.pick = c;
        self.at = At::Spread(Book::spread_of(book().starts[c]));
    }

    fn keys_line(&self) -> &'static str {
        match self.at {
            At::Cover => "space open · esc close",
            At::Spread(1) => "↑↓ choose · space turn to it · → turn the page · esc close",
            At::Spread(_) => "← → turn the page · [ ] chapters · c contents · esc close",
        }
    }
}

impl Default for Manual {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for Manual {
    fn update(&mut self, _st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        let book = book();
        let last = book.spreads();
        let count = chapters().len();
        for k in keys {
            match (self.at, k.code) {
                (_, KeyCode::Esc | KeyCode::Char('q')) => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                (
                    At::Cover,
                    KeyCode::Char(' ')
                    | KeyCode::Enter
                    | KeyCode::Right
                    | KeyCode::Char('l')
                    | KeyCode::PageDown,
                ) => self.at = At::Spread(1),
                (At::Spread(s), KeyCode::Left | KeyCode::Char('h') | KeyCode::PageUp) => {
                    self.at = if s <= 1 { At::Cover } else { At::Spread(s - 1) };
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
        let y = area.y + (area.height - PAGE_H - 1) / 2;
        let buf = f.buffer_mut();
        match self.at {
            At::Cover => cover(buf, area.x + (area.width - PAGE_W) / 2, y, st.tick),
            At::Spread(s) => {
                let x = area.x + (area.width - SPREAD_W) / 2;
                spread(buf, x, y, s, self.pick);
            }
        }
        // the keys line on a strip of its own, so nothing beneath reads through it
        let strip = Rect::new(
            area.x + (area.width - SPREAD_W) / 2,
            y + PAGE_H,
            SPREAD_W,
            1,
        );
        for x in strip.x..strip.right() {
            buf[(x, strip.y)].reset();
            buf[(x, strip.y)].set_bg(PANEL_BG);
        }
        let keys = self.keys_line();
        let w = keys.chars().count() as u16;
        put(
            buf,
            area.x + area.width.saturating_sub(w) / 2,
            y + PAGE_H,
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

/// The closed booklet: its frame, the logo in neon, the city drawn under it, its name.
fn cover(buf: &mut Buffer, x: u16, y: u16, tick: u64) {
    sheet(buf, Rect::new(x, y, PAGE_W, PAGE_H));
    if x > buf.area.x {
        for j in 0..PAGE_H {
            let c = &mut buf[(x - 1, y + j)];
            c.set_char('▐').set_fg(EDGE);
        }
    }
    // the frame: heavy rules top and bottom, light sides
    let edge = Style::default().fg(CYAN);
    let rule = "━".repeat(PAGE_W as usize - 2);
    put(buf, x, y, &format!("┍{rule}┑"), edge);
    put(buf, x, y + PAGE_H - 1, &format!("┕{rule}┙"), edge);
    for j in 1..PAGE_H - 1 {
        put(buf, x, y + j, "│", edge);
        put(buf, x + PAGE_W - 1, y + j, "│", edge);
    }
    // STRATA over ZERO, their right edges lined up
    let lx = x + 5;
    let gap = logo::width("STRATA") - logo::width("ZERO");
    for (word, wx, wy) in [("STRATA", lx, y + 2), ("ZERO", lx + gap, y + 8)] {
        Logo {
            text: word,
            tick,
            accent: MAGENTA_RGB,
            steady: true,
        }
        .render(Rect::new(wx, wy, logo::width(word), logo::ROWS), buf);
    }
    let art = city(PAGE_W - 4, 9);
    let mut pen = Pen {
        buf,
        ox: x + 2,
        oy: y + 14,
        clip: Rect::new(x + 2, y + 14, PAGE_W - 4, 9),
    };
    art.fold(&mut pen, 0, 0);
    let bold = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
    put(buf, x + 3, y + 24, "PLAYER'S MANUAL", bold);
    put(
        buf,
        x + 3,
        y + 25,
        "high tech · low life",
        Style::default().fg(DIM),
    );
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let vx = x + PAGE_W - 3 - version.chars().count() as u16;
    put(buf, vx, y + 25, &version, Style::default().fg(FAINT));
}

/// The cover's drawing, `w` cells by `rows`: the Net's grid running back to a horizon
/// high in the sky, the towers with their windows lit, the street's neon, the
/// undercity's pipes and lamps under it. The same city every time.
fn city(w: u16, rows: u16) -> Pixels {
    let mut px = Pixels::new(w, rows);
    let (w, h) = (i32::from(w), i32::from(rows) * 2);
    let street = h * 3 / 4;
    let col = |c: Rgb| c.to_color();
    for y in 0..h {
        let c = if y < street {
            Rgb(20, 10, 42).mix(Rgb(42, 15, 58), y as f32 / street as f32)
        } else {
            Rgb(11, 7, 16).mix(Rgb(2, 2, 4), (y - street) as f32 / (h - street) as f32)
        };
        px.hline(0, w, y, col(c));
    }
    // the Net: rungs across the sky, rails running back to its horizon
    let hz = (h / 6).max(3);
    for y in [1, hz / 2] {
        px.hline(0, w, y, col(Rgb(10, 61, 74)));
    }
    let cx = w / 2;
    for k in -10..=10 {
        for y in 0..hz {
            let t = (hz - y) as f32 / hz as f32;
            let x = cx + k * 3 + (k as f32 * 9.0 * t) as i32;
            px.px(x, y, Some(col(Rgb(11, 90, 106))));
        }
    }
    px.hline(0, w, hz, col(Rgb(10, 109, 128)));
    // towers: a dim row behind, a darker one in front with its windows lit
    let mut dice = 0x5eed_u64;
    let mut roll = |n: i32| -> i32 {
        dice = citygen::rng::hash64(dice, 7);
        (dice % n.max(1) as u64) as i32
    };
    let lights = [
        Rgb(255, 209, 102),
        Rgb(0, 229, 255),
        Rgb(255, 79, 216),
        Rgb(255, 233, 168),
    ];
    for (front, shade, lit) in [(false, Rgb(34, 24, 58), 10), (true, Rgb(18, 13, 30), 22)] {
        let mut x = -roll(3);
        while x < w {
            let tw = 4 + roll(6);
            let top = hz + 2 + roll((street - 5 - hz - 2).max(1)) + if front { 2 } else { 0 };
            px.rect(x, top, x + tw, street, col(shade));
            if roll(100) < 45 {
                px.vline(x + tw / 2, top - 3, top, col(shade));
            }
            let mut wy = top + 2;
            while wy < street - 1 {
                let mut wx = x + 1;
                while wx < x + tw - 1 {
                    if roll(100) < lit {
                        px.px(wx, wy, Some(col(lights[roll(4) as usize])));
                    }
                    wx += 2;
                }
                wy += 2;
            }
            if front && roll(100) < 40 && street - 4 >= top + 2 {
                let sy = top + 2 + roll(street - 4 - top - 1);
                let sign = if roll(2) == 0 {
                    MAGENTA_RGB
                } else {
                    Rgb(0, 229, 255)
                };
                px.hline(x + 1, x + (tw - 1).max(3), sy, col(sign));
            }
            x += tw + roll(3);
        }
    }
    // the street's neon and its glow, the undercity's pipes and lamps
    px.hline(0, w, street, col(MAGENTA_RGB));
    px.hline(0, w, street + 1, col(Rgb(90, 11, 72)));
    for y in [street + 4, street + 7] {
        for x in 0..w {
            if (x / 7 + y) % 3 != 0 {
                px.px(x, y, Some(col(Rgb(29, 36, 48))));
            }
        }
    }
    for x in (3..w).step_by(11) {
        px.px(x, street + 5, Some(col(Rgb(255, 179, 71))));
    }
    px
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
        let st = AppState::new();
        let mut m = Manual::new();
        let mut stx = AppState::new();
        for at in 0..4 {
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

    #[test]
    fn the_cover_opens_on_the_contents() {
        let mut st = AppState::new();
        let mut m = Manual::new();
        let cover = draw(&m, &st, 100, 30);
        assert!(
            cover.contains("PLAYER'S MANUAL") && cover.contains("space open"),
            "{cover}"
        );
        assert!(matches!(
            press(&mut m, &mut st, KeyCode::Char(' ')),
            ScreenCmd::Stay
        ));
        let open = draw(&m, &st, 100, 30);
        assert!(open.contains("CONTENTS"), "{open}");
        for ch in chapters() {
            assert!(open.contains(&ch.title), "{} missing:\n{open}", ch.title);
        }
        // back past the contents closes it again
        press(&mut m, &mut st, KeyCode::Left);
        assert!(draw(&m, &st, 100, 30).contains("PLAYER'S MANUAL"));
    }

    #[test]
    fn the_contents_turn_to_the_picked_chapter() {
        let mut st = AppState::new();
        let mut m = Manual::new();
        press(&mut m, &mut st, KeyCode::Char(' '));
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
        assert_eq!(m.at, At::Spread(1));
        let last = book().spreads();
        for _ in 0..last + 5 {
            press(&mut m, &mut st, KeyCode::Right);
        }
        assert_eq!(m.at, At::Spread(last));
        for _ in 0..last + 5 {
            press(&mut m, &mut st, KeyCode::Left);
        }
        assert_eq!(m.at, At::Cover);
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
    fn controls_opens_on_its_chapter_and_esc_closes() {
        let mut st = AppState::new();
        let mut m = Manual::at_chapter("controls");
        let text = draw(&m, &st, 100, 30);
        assert!(text.contains("CONTROLS"), "{text}");
        assert!(matches!(
            press(&mut m, &mut st, KeyCode::Esc),
            ScreenCmd::Pop { .. }
        ));
        let mut m = Manual::new();
        assert!(matches!(
            press(&mut m, &mut st, KeyCode::Char('q')),
            ScreenCmd::Pop { .. }
        ));
        assert_eq!(Manual::at_chapter("no-such-chapter").at, At::Cover);
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
