//! The splash at launch: a memory dump races past a read head, the scanner cracks a key
//! that turns out to be the game's name, and its bytes fly up into the title's logo.
//!
//! Every frame is drawn from the ticks since the splash opened. It hands over to the
//! title at [`HAND_OVER`], with the logo already where the title draws it, or at once on
//! any key.

use crate::anim::effects::GLITCH_CHARS;
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::screens::title::{self, LOGO_ACCENT, TITLE, Title};
use crate::ui::logo::{self, CELL_W};
use citygen::rng::hash64;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// The tick the splash hands over to the title: five seconds in.
pub const HAND_OVER: u64 = 150;
/// The tick the scan stops on the key.
const FOUND: u64 = 70;
/// How many rows scroll past the read head before it stops.
const SPAN: f32 = 560.0;
/// The dump's row the key sits in: far enough down that the first row on screen is never
/// below zero, however tall the terminal.
const KEY_ROW: i64 = 1000;
/// The key: the game's name, read as bytes.
const KEY: [u8; 11] = *b"STRATA ZERO";
/// Where the key starts in its row.
const KEY_AT: usize = 3;
/// A dump row: address, sixteen bytes, their ASCII between bars.
const DUMP_W: u16 = 79;
/// The scan, the dump's dissolve, the bytes' flight and the key's own dissolve begin.
const DISSOLVE: f32 = 84.0;
const FLY: f32 = 92.0;
const KEY_DISSOLVE: f32 = 124.0;
/// After this nothing of the dump is left to draw.
const DUMP_GONE: u64 = 140;

const CYAN: Rgb = Rgb(0, 229, 255);
const MAGENTA: Rgb = Rgb(255, 0, 200);
const WHITE: Rgb = Rgb(255, 255, 255);
const DIM: Rgb = Rgb(160, 170, 185);
const FAINT: Rgb = Rgb(120, 130, 143);
const FAINTER: Rgb = Rgb(92, 102, 115);
const HEAD: Rgb = Rgb(0, 28, 38);
const HEAD_FLASH: Rgb = Rgb(120, 60, 110);
const HEAD_FOUND: Rgb = Rgb(46, 0, 38);
const BAR_OFF: Rgb = Rgb(30, 40, 50);

pub struct Splash {
    opened: u64,
    /// The dump's bytes; the key row and the flight don't depend on it.
    seed: u64,
}

impl Splash {
    pub fn new(opened: u64, seed: u64) -> Self {
        Splash { opened, seed }
    }

    /// The splash the game opens on, its dump different every launch.
    pub fn at_launch(opened: u64) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Splash::new(opened, nanos)
    }

    /// The sixteen bytes of dump row `n`: the key's row, or a quarter zeros and noise.
    fn row_bytes(&self, n: i64) -> [u8; 16] {
        if n == KEY_ROW {
            let mut b = [0u8; 16];
            b[1] = 0x1C;
            b[2] = 0x7F;
            b[KEY_AT..KEY_AT + KEY.len()].copy_from_slice(&KEY);
            return b;
        }
        std::array::from_fn(|j| {
            let h = hash64(hash64(self.seed, n as u64), j as u64);
            if h % 100 < 28 { 0 } else { (h >> 32) as u8 }
        })
    }
}

/// `hash64` of `a` and `b` as a number in 0..1.
fn unit(a: u64, b: u64) -> f32 {
    (hash64(a, b) >> 40) as f32 / (1u64 << 24) as f32
}

fn out_cubic(p: f32) -> f32 {
    1.0 - (1.0 - p.clamp(0.0, 1.0)).powi(3)
}

fn fg(c: Rgb) -> Style {
    Style::default().fg(c.to_color())
}

fn bold(c: Rgb) -> Style {
    fg(c).add_modifier(Modifier::BOLD)
}

/// The column of byte `j` in a dump row: two groups of eight, a space between.
fn byte_x(j: usize) -> i32 {
    12 + 3 * j as i32 + i32::from(j >= 8)
}

fn address(n: i64) -> String {
    format!("0x{:08X}", 0x00A1_F000 + n * 16)
}

/// Where everything sits on `area`.
struct Layout {
    area: Rect,
    /// The dump's left edge.
    x0: i32,
    /// The rows the dump fills, and the read head among them.
    top: i32,
    bottom: i32,
    head: i32,
    status: i32,
}

impl Layout {
    fn new(area: Rect) -> Self {
        let (x, y, h) = (area.x as i32, area.y as i32, area.height as i32);
        Layout {
            area,
            x0: x + area.width.saturating_sub(DUMP_W) as i32 / 2,
            top: y + 2,
            bottom: y + h - 4,
            head: y + h / 2 + 4,
            status: y + h - 2,
        }
    }

    /// The dump row the top row shows when the scan stops: the key's on the read head.
    fn last_top(&self) -> f32 {
        (KEY_ROW - (self.head - self.top) as i64) as f32
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        let a = self.area;
        x >= a.x as i32 && y >= a.y as i32 && x < a.right() as i32 && y < a.bottom() as i32
    }

    fn set(&self, buf: &mut Buffer, x: i32, y: i32, ch: char, style: Style) {
        if self.inside(x, y) {
            buf[(x as u16, y as u16)].set_char(ch).set_style(style);
        }
    }

    /// `s` from (x, y); returns the column after it.
    fn put(&self, buf: &mut Buffer, x: i32, y: i32, s: &str, style: Style) -> i32 {
        let mut x = x;
        for ch in s.chars() {
            self.set(buf, x, y, ch, style);
            x += 1;
        }
        x
    }
}

/// A cell of the logo: letter `i` of the title, its glyph row `r`, where the title draws
/// it.
struct LogoCell {
    i: usize,
    r: u16,
    ch: char,
    x: i32,
    y: i32,
}

fn logo_cells(area: Rect) -> Vec<LogoCell> {
    let (ox, oy) = title::logo_origin(area);
    let mut out = Vec::new();
    for (i, c) in TITLE.chars().enumerate() {
        for (r, row) in logo::glyph(c).iter().enumerate() {
            for (col, ch) in row.chars().enumerate() {
                if ch != ' ' {
                    out.push(LogoCell {
                        i,
                        r: r as u16,
                        ch,
                        x: ox as i32 + (i as u16 * CELL_W) as i32 + col as i32,
                        y: oy as i32 + r as i32,
                    });
                }
            }
        }
    }
    out
}

impl Splash {
    /// The header, the dump scrolling past the read head and the scanner's status line.
    fn draw_dump(&self, buf: &mut Buffer, lay: &Layout, t: u64) {
        let tf = t as f32;
        let found = t >= FOUND;
        let (top_row, speed) = if found {
            (lay.last_top(), 0.0)
        } else {
            let left = 1.0 - tf / FOUND as f32;
            (
                lay.last_top() - SPAN * left.powi(3),
                3.0 * SPAN * left * left / FOUND as f32,
            )
        };
        let first = top_row.floor() as i64;
        let x0 = lay.x0;
        lay.put(
            buf,
            x0,
            lay.area.y as i32,
            "ICEPICK 2.1  ·  memory read  ·  jack port 7",
            fg(FAINT),
        );
        // The read head: a band across the dump, a dark flash when the key lands on it.
        let band = if !found {
            HEAD
        } else if t < FOUND + 2 {
            HEAD_FLASH
        } else {
            HEAD_FOUND
        };
        for x in x0 - 2..x0 - 2 + DUMP_W as i32 + 4 {
            if lay.inside(x, lay.head) {
                buf[(x as u16, lay.head as u16)].set_bg(band.to_color());
            }
        }
        if !found {
            lay.set(buf, x0 - 2, lay.head, '▶', fg(CYAN));
        }
        let base = if speed > 6.0 {
            FAINTER
        } else if speed > 2.0 {
            FAINT
        } else {
            DIM
        };
        for y in lay.top..=lay.bottom {
            let n = first + (y - lay.top) as i64;
            let bytes = self.row_bytes(n);
            let is_key = n == KEY_ROW && found;
            let in_key = |j: usize| is_key && (KEY_AT..KEY_AT + KEY.len()).contains(&j);
            let addr = CYAN.scale(if speed > 6.0 { 0.35 } else { 0.55 });
            lay.put(buf, x0, y, &address(n), fg(addr));
            for (j, &b) in bytes.iter().enumerate() {
                let style = if in_key(j) {
                    bold(MAGENTA)
                } else if !found && unit(hash64(n as u64, 11), j as u64) < 0.025 {
                    // A near miss.
                    fg(MAGENTA)
                } else if b == 0 {
                    fg(base.scale(0.6))
                } else {
                    fg(base)
                };
                lay.put(buf, x0 + byte_x(j), y, &format!("{b:02X}"), style);
            }
            lay.set(buf, x0 + 61, y, '│', fg(FAINTER));
            for (j, &b) in bytes.iter().enumerate() {
                let ch = if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    '.'
                };
                let style = if in_key(j) { bold(WHITE) } else { fg(FAINTER) };
                lay.set(buf, x0 + 62 + j as i32, y, ch, style);
            }
            lay.set(buf, x0 + 78, y, '│', fg(FAINTER));
        }
        if found {
            lay.put(buf, x0 + 80, lay.head, "◀ KEY", bold(MAGENTA));
        }
        // The status line: the scan's progress and the key a byte at a time, then the
        // key found.
        let y = lay.status;
        if found {
            let x = lay.put(buf, x0, y, "KEY FOUND  ", bold(MAGENTA));
            let mut x = lay.put(
                buf,
                x,
                y,
                &format!("{}+{KEY_AT:02}   ", address(KEY_ROW)),
                fg(DIM),
            );
            for b in KEY {
                x = lay.put(buf, x, y, &format!("{b:02X} "), bold(MAGENTA));
            }
        } else {
            let p = tf / FOUND as f32;
            let mut x = lay.put(buf, x0, y, "SCAN ", fg(FAINT));
            let at_head = first + (lay.head - lay.top) as i64;
            x = lay.put(buf, x, y, &format!("{}  ", address(at_head)), fg(DIM));
            let done = (16.0 * p).round() as i32;
            for k in 0..16 {
                let (ch, c) = if k < done {
                    ('█', CYAN)
                } else {
                    ('░', BAR_OFF)
                };
                lay.set(buf, x + k, y, ch, fg(c));
            }
            x += 17;
            let pct = format!("{}%", (100 * t) / FOUND);
            x = lay.put(buf, x, y, &format!("{pct:>4}   KEY "), fg(FAINT));
            let known = (11.0 * ((tf - 6.0) / 60.0).clamp(0.0, 1.0)).floor() as usize;
            for (j, b) in KEY.iter().enumerate() {
                x = if j < known {
                    lay.put(buf, x, y, &format!("{b:02X} "), bold(MAGENTA))
                } else {
                    lay.put(buf, x, y, "?? ", fg(FAINTER))
                };
            }
        }
    }

    /// Everything but the key dissolves, each cell on its own tick with two ticks of a
    /// glitch first; the key's bytes and letters follow later.
    fn dissolve(&self, buf: &mut Buffer, lay: &Layout, t: u64) {
        let tf = t as f32;
        let ky = lay.head;
        let (kx0, kx1) = (
            lay.x0 + byte_x(KEY_AT),
            lay.x0 + byte_x(KEY_AT + KEY.len() - 1) + 1,
        );
        let (ax0, ax1) = (
            lay.x0 + 62 + KEY_AT as i32,
            lay.x0 + 62 + (KEY_AT + KEY.len()) as i32 - 1,
        );
        let a = lay.area;
        for y in a.y..a.bottom() {
            for x in a.x..a.right() {
                let (xi, yi) = (x as i32, y as i32);
                let key = yi == ky && ((kx0..=kx1).contains(&xi) || (ax0..=ax1).contains(&xi));
                let h = hash64((x - a.x) as u64, (y - a.y) as u64);
                let gone = if key {
                    KEY_DISSOLVE + 14.0 * unit(h, 22)
                } else {
                    DISSOLVE + 20.0 * unit(h, 21)
                };
                let cell = &mut buf[(x, y)];
                if tf >= gone {
                    cell.reset();
                } else if tf >= gone - 2.0 && cell.symbol() != " " {
                    let g = GLITCH_CHARS[(hash64(h, t) % GLITCH_CHARS.len() as u64) as usize];
                    cell.set_char(g).set_fg(CYAN.scale(0.6).to_color());
                }
            }
        }
    }

    /// Each cell of the logo flies out of its letter's byte and lands where the title
    /// draws it: white for two ticks, then the title logo's own colour.
    fn draw_bytes(&self, buf: &mut Buffer, lay: &Layout, t: u64, tick: u64) {
        let tf = t as f32;
        let mut flying = Vec::new();
        for (k, c) in logo_cells(lay.area).into_iter().enumerate() {
            let k = k as u64;
            let start = FLY + 2.2 * c.i as f32 + 8.0 * unit(k, 2);
            let dur = 14.0 + 10.0 * unit(k, 3);
            let q = (tf - start) / dur;
            if q < 0.0 {
                continue;
            }
            if q < 1.0 {
                flying.push((k, c, start, dur));
                continue;
            }
            let colour = if tf - (start + dur) < 2.0 {
                WHITE
            } else {
                logo::letter_colour(tick, c.i, c.r, LOGO_ACCENT, false)
            };
            if lay.inside(c.x, c.y) {
                let cell = &mut buf[(c.x as u16, c.y as u16)];
                cell.reset();
                cell.set_char(c.ch).set_fg(colour.to_color());
            }
        }
        for (k, c, start, dur) in flying {
            let side = i32::from(unit(k, 1) >= 0.5);
            let arc = 2.0 + 5.0 * unit(k, 4);
            let (sx, sy) = (
                (lay.x0 + byte_x(KEY_AT + c.i) + side) as f32,
                lay.head as f32,
            );
            let at = |e: f32| {
                let x = sx + (c.x as f32 - sx) * e;
                let y = sy + (c.y as f32 - sy) * e - (std::f32::consts::PI * e).sin() * arc;
                (x.round() as i32, y.round() as i32)
            };
            let (tx, ty) = at(out_cubic((tf - 1.0 - start) / dur));
            lay.set(buf, tx, ty, '·', fg(MAGENTA.scale(0.45)));
            let digit = format!("{:02X}", KEY[c.i])
                .chars()
                .nth(side as usize)
                .unwrap();
            let (x, y) = at(out_cubic((tf - start) / dur));
            lay.set(buf, x, y, digit, bold(MAGENTA.mix(WHITE, 0.35)));
        }
    }
}

impl Screen for Splash {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        let skipped = !keys.is_empty();
        if skipped || st.tick >= self.opened + HAND_OVER {
            return ScreenCmd::Switch {
                to: Box::new(Title::after_splash(st, st.tick)),
                fx: if skipped {
                    TransitionKind::Glitch
                } else {
                    TransitionKind::None
                },
            };
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let t = st.tick.saturating_sub(self.opened);
        let lay = Layout::new(f.area());
        let buf = f.buffer_mut();
        if t < DUMP_GONE {
            self.draw_dump(buf, &lay, t);
        }
        if t as f32 >= DISSOLVE {
            self.dissolve(buf, &lay, t);
        }
        if t as f32 >= FLY {
            self.draw_bytes(buf, &lay, t, st.tick);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn draw(screen: &dyn Screen, st: &AppState, (w, h): (u16, u16)) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| screen.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    fn switch_fx(cmd: ScreenCmd) -> Option<TransitionKind> {
        match cmd {
            ScreenCmd::Switch { fx, .. } => Some(fx),
            _ => None,
        }
    }

    #[test]
    fn it_hands_over_at_150() {
        let mut st = AppState::new();
        let mut splash = Splash::new(0, 7);
        for tick in 1..HAND_OVER {
            st.tick = tick;
            assert!(
                matches!(splash.update(&mut st, &[]), ScreenCmd::Stay),
                "tick {tick}"
            );
        }
        st.tick = HAND_OVER;
        assert_eq!(
            switch_fx(splash.update(&mut st, &[])),
            Some(TransitionKind::None)
        );
        // A slow frame that skips past the hand-over still hands over.
        st.tick = HAND_OVER + 2;
        assert!(switch_fx(Splash::new(0, 7).update(&mut st, &[])).is_some());
    }

    #[test]
    fn any_key_skips_to_the_title() {
        let mut st = AppState::new();
        st.tick = 10;
        let cmd = Splash::new(0, 7).update(&mut st, &[key(KeyCode::Char('x'))]);
        assert_eq!(switch_fx(cmd), Some(TransitionKind::Glitch));
    }

    #[test]
    fn a_key_mashed_through_the_skip_does_nothing() {
        let mut st = AppState::new();
        st.tick = 10;
        let ScreenCmd::Switch { to: mut title, .. } =
            Splash::new(0, 7).update(&mut st, &[key(KeyCode::Esc)])
        else {
            panic!("the key skips");
        };
        st.tick = 13;
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        st.tick = 22;
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Quit
        ));
    }

    #[test]
    fn the_key_is_found_at_75() {
        let mut st = AppState::new();
        st.tick = 75;
        let text = buffer_text(&draw(&Splash::new(0, 7), &st, (120, 40)));
        for want in [
            "53 54 52 41 54 41",
            "20 5A 45 52 4F",
            "STRATA ZERO",
            "KEY FOUND",
        ] {
            assert!(text.contains(want), "{want}\n{text}");
        }
    }

    #[test]
    fn the_hand_over_is_seamless() {
        for size in [(120, 40), (100, 30)] {
            for tick in HAND_OVER - 3..HAND_OVER {
                let mut st = AppState::new();
                st.tick = tick;
                let splash = draw(&Splash::new(0, 7), &st, size);
                let title = draw(&Title::new(&st, tick), &st, size);
                let cells = logo_cells(splash.area);
                for c in &cells {
                    let pos = (c.x as u16, c.y as u16);
                    assert_eq!(splash[pos].symbol(), title[pos].symbol(), "{size:?} {tick}");
                    assert_eq!(splash[pos].fg, title[pos].fg, "{size:?} {tick} {pos:?}");
                }
                let drawn = splash.content.iter().filter(|c| c.symbol() != " ").count();
                assert_eq!(drawn, cells.len(), "only the logo is left: {size:?} {tick}");
            }
        }
    }

    #[test]
    fn draws_at_every_tick_and_size() {
        let splash = Splash::new(0, 7);
        let mut st = AppState::new();
        for size in [(100, 30), (120, 40), (300, 80)] {
            for tick in 0..=HAND_OVER {
                st.tick = tick;
                draw(&splash, &st, size);
            }
        }
    }
}
