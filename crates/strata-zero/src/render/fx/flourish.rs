//! Flourishes round a tile: the upload a level plays on you and the lock-on Deadeye
//! plays on its foe. Spec `docs/superpowers/specs/2026-11-08-level-up-design.md`; the
//! looks are mock B and mock C of `2026-11-08-level-up-mock/`.

use crate::app::FxInstance;
use crate::render::Rgb;
use crate::track::overlay::mix;
use citygen::Pos;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Frames a flourish plays, two ticks each: the mock's 15 a second at the app's 30.
pub const FRAMES: u64 = 18;
/// Frames the last one holds, so its words can be read.
pub const HOLD: u64 = 8;
/// Ticks a flourish lasts.
pub const FLOURISH_TICKS: u64 = (FRAMES + HOLD) * 2;
/// The frame Upload's beam lands on you.
const FALL: usize = 6;
/// The frame its beam starts to drain from the top.
const DRAIN: usize = 10;
/// The frame Lock's brackets bite.
const LOCK: usize = 8;
/// What runs down Upload's beam.
const BITS: [char; 5] = ['0', '1', '┃', '┃', '┃'];

const CYAN: Rgb = Rgb(0, 229, 255);
const MAGENTA: Rgb = Rgb(255, 0, 200);
const AMBER: Rgb = Rgb(255, 190, 60);
const WHITE: Rgb = Rgb(255, 255, 255);
const INK: Rgb = Rgb(4, 6, 10);

/// The frame at tick `e`, held at the last.
fn frame(e: u64) -> usize {
    (e / 2).min(FRAMES - 1) as usize
}

/// The view's cells, drawn on by screen cell; anything off the view is dropped. Words
/// keep left of `right` (short of the turn column, on turns).
struct Canvas<'a> {
    buf: &'a mut Buffer,
    area: Rect,
    right: i32,
}

impl Canvas<'_> {
    fn cell(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        let a = self.area;
        let inside = (a.x as i32..a.right() as i32).contains(&x)
            && (a.y as i32..a.bottom() as i32).contains(&y);
        inside.then(|| &mut self.buf[(x as u16, y as u16)])
    }

    fn tint(&mut self, x: i32, y: i32, c: Rgb, f: f32) {
        if let Some(cell) = self.cell(x, y) {
            let bg = cell.bg;
            cell.set_bg(mix(bg, c.to_color(), f.clamp(0.0, 1.0)));
        }
    }

    fn put(&mut self, x: i32, y: i32, ch: char, fg: Rgb, bold: bool) {
        if let Some(cell) = self.cell(x, y) {
            cell.set_char(ch).set_fg(fg.to_color());
            cell.modifier.remove(Modifier::DIM);
            cell.modifier.set(Modifier::BOLD, bold);
        }
    }

    /// The first `n` chars of `s` from `x`, on cells pressed dark so they read over
    /// the street.
    fn typed(&mut self, x: i32, y: i32, s: &str, n: usize, fg: Rgb, bold: bool) {
        for (i, ch) in s.chars().take(n).enumerate() {
            self.tint(x + i as i32, y, INK, 0.7);
            self.put(x + i as i32, y, ch, fg, bold);
        }
    }

    /// A flash on `(cx, cy)` and the eight cells round it, gone after `n` frames.
    fn flash(&mut self, (cx, cy): (i32, i32), t: usize, n: usize, c: Rgb) {
        if t >= n {
            return;
        }
        let f = 1.0 - t as f32 / n as f32;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let k = if (dx, dy) == (0, 0) { 0.9 } else { 0.45 };
                self.tint(cx + dx, cy + dy, c, f * k);
            }
        }
    }

    /// Where words `len` long start, `gap` cells off the cell at `cx`: on its right,
    /// or on its left when they would run into `right`.
    fn beside(&self, cx: i32, gap: i32, len: usize) -> i32 {
        let len = len as i32;
        if cx + gap + len <= self.right {
            cx + gap
        } else {
            cx - gap - len + 1
        }
    }

    /// The row words go on over the cell at `cy`: the one above it, or below it when
    /// that is off the view's top.
    fn over(&self, cy: i32) -> i32 {
        if cy - 1 < self.area.y as i32 {
            cy + 1
        } else {
            cy - 1
        }
    }
}

/// The canvas on `area`, its words kept `clear_right` columns short of its right edge.
fn canvas(buf: &mut Buffer, area: Rect, clear_right: u16) -> Canvas<'_> {
    let right = area.right().saturating_sub(clear_right).max(area.x) as i32;
    Canvas { buf, area, right }
}

/// The instance's tile as a screen cell, on the view or off it.
fn centre(inst: &FxInstance, origin: Pos, area: Rect) -> (i32, i32) {
    (
        area.x as i32 + inst.origin.x - origin.x,
        area.y as i32 + inst.origin.y - origin.y,
    )
}

/// Upload: a beam of data falls on you from the top of the view; as it lands, a
/// scanline runs out along your row and your cell flashes; the beam drains from the
/// top, and `▲` and the label type out beside you.
pub(super) fn upload(
    inst: &FxInstance,
    e: u64,
    origin: Pos,
    area: Rect,
    clear_right: u16,
    buf: &mut Buffer,
) {
    let t = frame(e);
    let (cx, cy) = centre(inst, origin, area);
    let mut c = canvas(buf, area, clear_right);
    let top = area.y as i32;
    let span = (cy - top).max(0) as f32;
    let head = if t < FALL {
        top + (span * (t as f32 + 1.0) / FALL as f32) as i32
    } else {
        cy
    };
    let tail = if t >= DRAIN {
        top + (span * ((t - DRAIN) as f32 / 6.0)).min(span) as i32
    } else {
        top
    };
    for y in tail..head.min(cy) {
        let near = (head - y) as f32;
        let glow = (1.0 - near / 12.0).max(0.3);
        let ch = BITS[(y.unsigned_abs() as usize * 7 + t * 3) % BITS.len()];
        c.put(cx, y, ch, CYAN.mix(WHITE, glow * 0.7), near < 2.0);
        c.tint(cx, y, CYAN, 0.45 * glow);
        c.tint(cx - 1, y, CYAN, 0.18 * glow);
        c.tint(cx + 1, y, CYAN, 0.18 * glow);
    }
    if t + 1 >= FALL {
        let k = (t + 1 - FALL) as i32;
        let fade = (1.0 - k as f32 / 10.0).max(0.0);
        let reach = k * 4;
        for x in (cx - reach)..=(cx + reach) {
            let off = (x - cx).abs();
            let w = fade * (1.0 - off as f32 / 40.0).max(0.0);
            if w > 0.0 {
                c.tint(x, cy, CYAN, 0.5 * w);
            }
            if fade > 0.0 && off > 0 && off >= reach - 1 {
                c.put(x, cy, '━', CYAN.mix(WHITE, 0.5), true);
            }
        }
        c.flash((cx, cy), k as usize, 5, CYAN);
    }
    if t >= 8 {
        let words = format!("▲ {}", inst.label);
        let x = c.beside(cx, 2, words.chars().count());
        c.typed(x, c.over(cy), &words, (t - 8) * 2 + 1, CYAN, true);
    }
}

/// Lock: four brackets close in on the target from about the edges of the view and
/// lock round it, white as they bite, then magenta; the label's first line types out
/// beside them in magenta, its second under it in amber, done by the last frame.
pub(super) fn lock(
    inst: &FxInstance,
    e: u64,
    origin: Pos,
    area: Rect,
    clear_right: u16,
    buf: &mut Buffer,
) {
    let t = frame(e);
    let (cx, cy) = centre(inst, origin, area);
    let mut c = canvas(buf, area, clear_right);
    let reach = |t: usize| {
        let p = (t as f32 / LOCK as f32).min(1.0);
        let e = 1.0 - (1.0 - p).powi(3);
        (
            (18.0 + (2.0 - 18.0) * e).round() as i32,
            (8.0 + (1.0 - 8.0) * e).round() as i32,
        )
    };
    let corners = |(dx, dy): (i32, i32)| {
        [
            (cx - dx, cy - dy, '┏', 1),
            (cx + dx, cy - dy, '┓', -1),
            (cx - dx, cy + dy, '┗', 1),
            (cx + dx, cy + dy, '┛', -1),
        ]
    };
    if (1..=LOCK).contains(&t) {
        for (x, y, ch, _) in corners(reach(t - 1)) {
            c.put(x, y, ch, CYAN.mix(INK, 0.55), false);
        }
    }
    let colour = if (LOCK..LOCK + 2).contains(&t) {
        WHITE
    } else if t > LOCK {
        MAGENTA
    } else {
        CYAN
    };
    for (x, y, ch, inward) in corners(reach(t)) {
        c.put(x, y, ch, colour, true);
        if t >= LOCK {
            c.put(x + inward, y, '━', colour, true);
        }
    }
    if t < LOCK {
        return;
    }
    let k = t - LOCK;
    let f = (0.7 - k as f32 * 0.08).max(0.15);
    for dx in -1..=1 {
        c.tint(cx + dx, cy, MAGENTA, f);
    }
    let mut lines = inst.label.lines();
    let (first, second) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""));
    let len = first.chars().count().max(second.chars().count());
    let x = c.beside(cx, 5, len);
    let y = c.over(cy);
    c.typed(x, y, first, k * 2 + 1, MAGENTA, true);
    if k >= 2 {
        // four a frame, or faster for a long name, so it's done by the last frame
        let frames = FRAMES as usize - 1 - LOCK - 2;
        let pace = second.chars().count().div_ceil(frames).max(4);
        c.typed(x, y + 1, second, (k - 2) * pace, AMBER, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{Place, StratumKind};
    use game::Fx;
    use ratatui::style::Color;

    const AREA: Rect = Rect::new(0, 0, 80, 30);
    /// The view's top-left tile; the target sits at screen (40, 20).
    const ORIGIN: Pos = Pos::new(100, 100);
    const AT: Pos = Pos::new(140, 120);

    fn inst(fx: Fx, label: &str) -> FxInstance {
        FxInstance {
            fx,
            place: Place::Stratum(StratumKind::Street),
            origin: AT,
            radius: 0,
            started: 0,
            revealed: Vec::new(),
            targets: Vec::new(),
            label: label.to_string(),
            hit: false,
            tint: None,
            arc: None,
        }
    }

    type Draw = fn(&FxInstance, u64, Pos, Rect, u16, &mut Buffer);

    fn drawn(f: Draw, i: &FxInstance, e: u64) -> Buffer {
        drawn_clear(f, i, e, 0)
    }

    fn drawn_clear(f: Draw, i: &FxInstance, e: u64, clear: u16) -> Buffer {
        let mut buf = Buffer::empty(AREA);
        f(i, e, ORIGIN, AREA, clear, &mut buf);
        buf
    }

    fn sym(buf: &Buffer, x: i32, y: i32) -> String {
        buf[(x as u16, y as u16)].symbol().to_string()
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..AREA.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn a_flourish_holds_its_last_frame() {
        assert_eq!(FLOURISH_TICKS, 52);
        assert_eq!(frame(0), 0);
        assert_eq!(frame(35), 17);
        assert_eq!(frame(51), 17, "held");
        let i = inst(Fx::Lock, "MARKED\ngoon · +20% to hit");
        assert_eq!(i.until(), FLOURISH_TICKS);
    }

    #[test]
    fn lock_closes_in_and_bites() {
        let i = inst(Fx::Lock, "MARKED\ngoon · +20% to hit");
        // the first frame: brackets about the edges, cyan, no words
        let first = drawn(lock, &i, 0);
        assert_eq!(sym(&first, 40 - 18, 20 - 8), "┏");
        assert_eq!(sym(&first, 40 + 18, 20 + 8), "┛");
        assert_eq!(first[(22, 12)].fg, CYAN.to_color());
        assert!(!row(&first, 19).contains('M'));
        // locked: round the foe, white as they bite, then magenta, the words typed out
        let bite = drawn(lock, &i, (LOCK * 2) as u64);
        for (x, y, ch) in [(38, 19, "┏"), (42, 19, "┓"), (38, 21, "┗"), (42, 21, "┛")] {
            assert_eq!(sym(&bite, x, y), ch);
            assert_eq!(bite[(x as u16, y as u16)].fg, WHITE.to_color());
        }
        assert_eq!(sym(&bite, 39, 19), "━", "the arms in");
        let held = drawn(lock, &i, FLOURISH_TICKS - 1);
        assert_eq!(held[(38, 19)].fg, MAGENTA.to_color());
        assert!(row(&held, 19).contains("MARKED"), "{}", row(&held, 19));
        assert!(
            row(&held, 20).contains("goon · +20% to hit"),
            "{}",
            row(&held, 20)
        );
        assert_eq!(sym(&held, 45, 19), "M", "five cells off the foe");
        assert_eq!(held[(45, 20)].fg, AMBER.to_color());
    }

    #[test]
    fn upload_falls_on_you_and_says_the_level() {
        let i = inst(Fx::Upload, "LEVEL 2");
        // the beam's head on its way down, above you only
        let falling = drawn(upload, &i, 4);
        let lit = (0..20)
            .filter(|y| falling[(40, *y)].bg != Color::Reset)
            .count();
        assert!((1..20).contains(&lit), "part way down: {lit}");
        assert_eq!(falling[(40, 20)].bg, Color::Reset, "not on you yet");
        // landed: the column down to you, your cell flashed, the scanline out
        let landed = drawn(upload, &i, ((FALL - 1) * 2) as u64);
        assert!((0..20).all(|y| landed[(40, y)].bg != Color::Reset));
        assert_ne!(landed[(40, 20)].bg, Color::Reset);
        let later = drawn(upload, &i, ((FALL + 2) * 2) as u64);
        assert!(row(&later, 20).contains('━'), "{}", row(&later, 20));
        // held: the beam gone, the words beside you
        let held = drawn(upload, &i, FLOURISH_TICKS - 1);
        assert!((0..19).all(|y| sym(&held, 40, y) == " "), "drained");
        assert!(row(&held, 19).contains("▲ LEVEL 2"), "{}", row(&held, 19));
        assert_eq!(sym(&held, 42, 19), "▲", "two cells off you");
    }

    #[test]
    fn words_go_left_at_the_right_edge() {
        let edge = Pos::new(ORIGIN.x + 76, AT.y);
        let mut up = inst(Fx::Upload, "LEVEL 2");
        up.origin = edge;
        let held = drawn(upload, &up, FLOURISH_TICKS - 1);
        let words = "▲ LEVEL 2";
        let x = 76 - 2 - words.chars().count() as i32 + 1;
        assert_eq!(sym(&held, x, 19), "▲", "{}", row(&held, 19));
        let mut on = inst(Fx::Lock, "MARKED\ngoon · +20% to hit");
        on.origin = edge;
        let held = drawn(lock, &on, FLOURISH_TICKS - 1);
        let x = 76 - 5 - "goon · +20% to hit".chars().count() as i32 + 1;
        assert_eq!(sym(&held, x, 19), "M", "{}", row(&held, 19));
    }

    #[test]
    fn words_keep_out_of_the_turn_column() {
        // a foe 30 cells in from the right: room for its readout, but not with the
        // turn column's 18 kept clear
        let mut on = inst(Fx::Lock, "MARKED\ngoon · +20% to hit");
        on.origin = Pos::new(ORIGIN.x + 50, AT.y);
        let free = drawn(lock, &on, FLOURISH_TICKS - 1);
        assert_eq!(sym(&free, 55, 19), "M");
        let turns = drawn_clear(lock, &on, FLOURISH_TICKS - 1, 18);
        let x = 50 - 5 - "goon · +20% to hit".chars().count() as i32 + 1;
        assert_eq!(sym(&turns, x, 19), "M", "{}", row(&turns, 19));
        assert!(
            (62..80).all(|x| sym(&turns, x, 20) == " "),
            "{}",
            row(&turns, 20)
        );
    }

    #[test]
    fn words_go_below_at_the_top_edge() {
        let top = Pos::new(AT.x, ORIGIN.y);
        let mut up = inst(Fx::Upload, "LEVEL 2");
        up.origin = top;
        let held = drawn(upload, &up, FLOURISH_TICKS - 1);
        assert!(row(&held, 1).contains("▲ LEVEL 2"), "{}", row(&held, 1));
        let mut on = inst(Fx::Lock, "MARKED\ngoon · +20% to hit");
        on.origin = top;
        let held = drawn(lock, &on, FLOURISH_TICKS - 1);
        assert!(row(&held, 1).contains("MARKED"), "{}", row(&held, 1));
        assert!(
            row(&held, 2).contains("goon · +20% to hit"),
            "{}",
            row(&held, 2)
        );
    }

    #[test]
    fn a_long_name_still_finishes() {
        let name = "MARKED\nvery long street name · +20% to hit";
        let held = drawn(lock, &inst(Fx::Lock, name), FLOURISH_TICKS - 1);
        let whole = name.lines().nth(1).unwrap();
        assert!(row(&held, 20).contains(whole), "{}", row(&held, 20));
    }
}
