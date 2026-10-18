//! A deck's blueprint: the deck as hardware, what it is made of, where its parts sit
//! and how the pointer moves between them. It works on a `DeckView` snapshot, never
//! on the game state.

pub mod pawn;

use crate::anim::effects::pulse;
use crate::ui::chrome::CYAN_RGB;
use game::DeckLook;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// Characters a line of the glass holds.
pub const GLASS_TEXT_W: usize = 38;
/// Rows the glass shows.
pub const GLASS_ROWS: usize = 5;

/// Something the pointer can rest on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// Bay stop k: loaded program k, or the first empty slot at `k == bay.len()`.
    Slot(usize),
    Plate,
    Rom(usize),
    Failsafe,
    Inv(usize),
}

impl Part {
    /// The part as a number, for the one cursor field the game keeps.
    pub fn pack(self) -> usize {
        match self {
            Part::Slot(k) => k,
            Part::Plate => 100,
            Part::Rom(k) => 200 + k,
            Part::Failsafe => 300,
            Part::Inv(k) => 1000 + k,
        }
    }

    /// The part a number stands for.
    pub fn unpack(n: usize) -> Part {
        match n {
            0..=99 => Part::Slot(n),
            100..=199 => Part::Plate,
            200..=299 => Part::Rom(n - 200),
            300..=999 => Part::Failsafe,
            _ => Part::Inv(n - 1000),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// The failsafe switch's position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    On,
    Lifted,
    Off,
}

/// How a glass line is lit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassTone {
    Head,
    Body,
    Dim,
    Warn,
}

/// A program cart as the deck shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Cart {
    pub name: String,
    pub rating: u8,
    pub size: u8,
}

/// A cart lifted from the inventory, waiting for a slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    /// Its inventory index.
    pub from: usize,
    pub cart: Cart,
    /// The physical slot (0-based) it lands at once packed.
    pub start: u8,
    pub letter: char,
}

/// Everything the deck draws, taken from the game once a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct DeckView {
    pub look: DeckLook,
    pub slots: u8,
    pub rating: u8,
    pub bay: Vec<Cart>,
    pub roms: Vec<String>,
    pub inventory: Vec<Cart>,
    pub inv_offset: usize,
    pub point: Part,
    pub held: Option<Held>,
    pub switch: Switch,
    pub heat: Option<u8>,
    pub link: bool,
    pub handle: String,
    /// The glass rows to show, already wrapped and scrolled, at most `GLASS_ROWS`.
    pub glass: Vec<(String, GlassTone)>,
    pub pinned: (String, GlassTone),
    pub more_above: bool,
    pub more_below: bool,
    /// Glass characters typed so far; `usize::MAX` is all of them.
    pub typed: usize,
    pub tick: u64,
}

/// The drawing's size for a look.
pub fn size(look: DeckLook) -> (u16, u16) {
    match look {
        DeckLook::Pawn => (pawn::W, pawn::H),
    }
}

/// The parts of the deck and where they sit, relative to the drawing's origin.
pub fn parts(view: &DeckView) -> Vec<(Part, Rect)> {
    match view.look {
        DeckLook::Pawn => pawn::parts(view),
    }
}

/// Draws the deck centred in `area`, then the pointer's corners round the pointed
/// part. Nothing is written outside `area`.
pub fn draw(view: &DeckView, area: Rect, buf: &mut Buffer) {
    let (w, h) = size(view.look);
    let ox = area.x + area.width.saturating_sub(w) / 2;
    let oy = area.y + area.height.saturating_sub(h) / 2;
    let mut pen = Pen {
        clip: area.intersection(buf.area),
        buf,
        ox,
        oy,
    };
    match view.look {
        DeckLook::Pawn => pawn::draw_with(view, &mut pen),
    }
    if let Some((_, r)) = parts(view).into_iter().find(|(p, _)| *p == view.point) {
        let r = Rect::new(ox + r.x, oy + r.y, r.width, r.height);
        brackets(pen.buf, r, view.tick, pen.clip);
    }
}

/// Four cyan heavy corners one cell outside `r`, pulsing like the selected rows on
/// the other tabs. Corners that fall outside `clip` are skipped; the cells keep
/// whatever they sat on.
fn brackets(buf: &mut Buffer, r: Rect, tick: u64, clip: Rect) {
    let fg = CYAN_RGB.scale(0.7 + 0.3 * pulse(tick, 30)).to_color();
    let (x1, y1) = (r.x.checked_sub(1), r.y.checked_sub(1));
    let (x2, y2) = (r.x + r.width, r.y + r.height);
    for (x, y, glyph) in [
        (x1, y1, '┏'),
        (Some(x2), y1, '┓'),
        (x1, Some(y2), '┗'),
        (Some(x2), Some(y2), '┛'),
    ] {
        if let (Some(x), Some(y)) = (x, y)
            && clip.contains((x, y).into())
        {
            buf[(x, y)].set_char(glyph).set_fg(fg);
        }
    }
}

/// A drawing's pen: cells addressed from the drawing's origin, clipped to a rect so a
/// drawing never spills past the area it was given.
pub(crate) struct Pen<'a> {
    pub buf: &'a mut Buffer,
    pub ox: u16,
    pub oy: u16,
    pub clip: Rect,
}

impl Pen<'_> {
    /// Writes `text` from (x, y); `bg` of `None` keeps the cell's own.
    pub fn put(&mut self, x: u16, y: u16, text: &str, fg: Color, bg: Option<Color>, bold: bool) {
        let y = self.oy + y;
        for (i, ch) in text.chars().enumerate() {
            let x = self.ox + x + i as u16;
            if !self.clip.contains((x, y).into()) {
                continue;
            }
            let cell = &mut self.buf[(x, y)];
            cell.set_char(ch).set_fg(fg);
            if let Some(bg) = bg {
                cell.set_bg(bg);
            }
            cell.modifier = if bold {
                Modifier::BOLD
            } else {
                Modifier::empty()
            };
        }
    }

    /// Paints a block's background, clearing what was written there. The blanks are
    /// inked in the same colour, so no cell is left to the terminal's own.
    pub fn fill(&mut self, x: u16, y: u16, w: u16, h: u16, bg: Color) {
        for yy in y..y + h {
            self.put(x, yy, &" ".repeat(usize::from(w)), bg, Some(bg), false);
        }
    }

    /// Changes a block's background and nothing else.
    pub fn tint(&mut self, x: u16, y: u16, w: u16, h: u16, bg: Color) {
        for yy in self.oy + y..self.oy + y + h {
            for xx in self.ox + x..self.ox + x + w {
                if self.clip.contains((xx, yy).into()) {
                    self.buf[(xx, yy)].set_bg(bg);
                }
            }
        }
    }

    /// A box outline from the given corner and edge glyphs, keeping the backgrounds.
    pub fn frame(&mut self, (x, y, w, h): (u16, u16, u16, u16), fg: Color, g: [char; 6]) {
        let [tl, tr, bl, br, hz, vt] = g;
        let run = |a: char, b: char| format!("{a}{}{b}", hz.to_string().repeat(usize::from(w) - 2));
        self.put(x, y, &run(tl, tr), fg, None, false);
        self.put(x, y + h - 1, &run(bl, br), fg, None, false);
        for yy in y + 1..y + h - 1 {
            self.put(x, yy, &vt.to_string(), fg, None, false);
            self.put(x + w - 1, yy, &vt.to_string(), fg, None, false);
        }
    }
}

/// The part nearest `from` in `dir` that `allow` lets the pointer rest on, or
/// `from` when there is none. Distances run in square units (a cell is twice as
/// tall as wide). A part in the 45 degree cone wins by `primary + perp`; only when
/// that cone holds none does a wider one (`perp <= 2 * primary`) count, by
/// `primary + 2 * perp`. Ties go to the earlier part.
pub fn step(parts: &[(Part, Rect)], from: Part, dir: Dir, allow: impl Fn(Part) -> bool) -> Part {
    let centre = |r: &Rect| {
        (
            f32::from(r.x) + f32::from(r.width) / 2.0,
            2.0 * (f32::from(r.y) + f32::from(r.height) / 2.0),
        )
    };
    let Some((_, here)) = parts.iter().find(|(p, _)| *p == from) else {
        return from;
    };
    let (fx, fy) = centre(here);
    let mut tight: Option<(f32, Part)> = None;
    let mut wide: Option<(f32, Part)> = None;
    for (p, r) in parts {
        if *p == from || !allow(*p) {
            continue;
        }
        let (cx, cy) = centre(r);
        let (dx, dy) = (cx - fx, cy - fy);
        let (primary, perp) = match dir {
            Dir::Right => (dx, dy.abs()),
            Dir::Left => (-dx, dy.abs()),
            Dir::Down => (dy, dx.abs()),
            Dir::Up => (-dy, dx.abs()),
        };
        if primary <= 0.0 {
            continue;
        }
        let (tier, score) = if perp <= primary {
            (&mut tight, primary + perp)
        } else if perp <= 2.0 * primary {
            (&mut wide, primary + 2.0 * perp)
        } else {
            continue;
        };
        if tier.is_none_or(|(best, _)| score < best) {
            *tier = Some((score, *p));
        }
    }
    tight.or(wide).map_or(from, |(_, p)| p)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn cart(name: &str, size: u8) -> Cart {
        Cart {
            name: name.into(),
            rating: 1,
            size,
        }
    }

    pub fn kit_view() -> DeckView {
        DeckView {
            look: DeckLook::Pawn,
            slots: 2,
            rating: 1,
            bay: vec![cart("CRACK", 1), cart("COPY", 1)],
            roms: vec!["NETSCAN".into(), "ARC TRACE".into()],
            inventory: vec![cart("ATTACK", 1)],
            inv_offset: 0,
            point: Part::Slot(0),
            held: None,
            switch: Switch::On,
            heat: None,
            link: false,
            handle: "ghost".into(),
            glass: Vec::new(),
            pinned: (String::new(), GlassTone::Dim),
            more_above: false,
            more_below: false,
            typed: usize::MAX,
            tick: 0,
        }
    }

    pub fn render(view: &DeckView) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 98, 22));
        draw(view, Rect::new(0, 0, 98, 22), &mut buf);
        buf
    }

    pub fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    pub fn text(buf: &Buffer) -> String {
        (0..buf.area.height)
            .map(|y| row(buf, y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn draw_centres_in_a_bigger_body() {
        let v = kit_view();
        let area = Rect::new(5, 3, 120, 30);
        let mut buf = Buffer::empty(Rect::new(0, 0, 140, 40));
        draw(&v, area, &mut buf);
        let (ox, oy) = (5 + (120 - 84) / 2, 3 + (30 - 22) / 2);
        assert_eq!(buf[(ox + 4, oy + 5)].symbol(), " ");
        assert_eq!(buf[(ox + 5, oy + 5)].symbol(), "K");
        let blank = Buffer::empty(Rect::new(0, 0, 140, 40));
        for y in 0..40 {
            for x in 0..140 {
                if !area.contains((x, y).into()) {
                    assert_eq!(buf[(x, y)], blank[(x, y)], "({x},{y})");
                }
            }
        }
        let buf = render(&v);
        assert_eq!(row(&buf, 5).chars().nth(7 + 5), Some('K'));
    }

    #[test]
    fn a_small_area_is_never_spilled_past() {
        // The failsafe's right corners and the inventory's bottom ones fall outside.
        let area = Rect::new(10, 6, 60, 12);
        let blank = Buffer::empty(Rect::new(0, 0, 98, 30));
        for point in [Part::Slot(0), Part::Plate, Part::Failsafe, Part::Inv(0)] {
            let mut v = kit_view();
            v.point = point;
            let mut buf = blank.clone();
            draw(&v, area, &mut buf);
            for y in 0..30 {
                for x in 0..98 {
                    if !area.contains((x, y).into()) {
                        assert_eq!(buf[(x, y)], blank[(x, y)], "{point:?} ({x},{y})");
                    }
                }
            }
        }
    }

    #[test]
    fn brackets_pulse_and_stay_on_the_buffer() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        let clip = buf.area;
        brackets(&mut buf, Rect::new(5, 3, 4, 2), 0, clip);
        assert_eq!(buf[(4, 2)].symbol(), "┏");
        assert_eq!(buf[(9, 2)].symbol(), "┓");
        assert_eq!(buf[(4, 5)].symbol(), "┗");
        assert_eq!(buf[(9, 5)].symbol(), "┛");
        let a = buf[(4, 2)].fg;
        brackets(&mut buf, Rect::new(5, 3, 4, 2), 8, clip);
        assert_ne!(buf[(4, 2)].fg, a);
        // At the buffer's edge the corners that fall outside are skipped.
        brackets(&mut buf, Rect::new(0, 0, 4, 2), 0, clip);
        assert_eq!(buf[(4, 2)].symbol(), "┛");
    }

    #[test]
    fn pointing_walks_the_pawn_layout() {
        let p = parts(&kit_view());
        let any = |_| true;
        assert_eq!(step(&p, Part::Slot(0), Dir::Right, any), Part::Slot(1));
        assert_eq!(step(&p, Part::Slot(0), Dir::Down, any), Part::Plate);
        assert_eq!(step(&p, Part::Plate, Dir::Down, any), Part::Rom(0));
        assert_eq!(step(&p, Part::Rom(0), Dir::Down, any), Part::Inv(0));
        assert_eq!(step(&p, Part::Rom(1), Dir::Right, any), Part::Failsafe);
        assert_eq!(step(&p, Part::Slot(1), Dir::Right, any), Part::Failsafe);
        assert_eq!(step(&p, Part::Slot(0), Dir::Left, any), Part::Slot(0));
        assert_eq!(step(&p, Part::Slot(0), Dir::Up, any), Part::Slot(0));
        assert_eq!(step(&p, Part::Inv(0), Dir::Up, any), Part::Rom(0));

        // With four carts in the row, the failsafe drops to the one beneath it.
        let mut v = kit_view();
        v.inventory = ["SPOOF", "BLIND", "MASK", "BACKDOOR"]
            .iter()
            .map(|n| cart(n, 1))
            .collect();
        let p = parts(&v);
        assert_eq!(step(&p, Part::Failsafe, Dir::Down, any), Part::Inv(3));
        let slots_only = |q| matches!(q, Part::Slot(_));
        assert_eq!(
            step(&p, Part::Slot(0), Dir::Down, slots_only),
            Part::Slot(0)
        );
    }

    #[test]
    fn parts_pack_and_unpack() {
        for p in [
            Part::Slot(0),
            Part::Slot(5),
            Part::Plate,
            Part::Rom(0),
            Part::Rom(3),
            Part::Failsafe,
            Part::Inv(0),
            Part::Inv(12),
        ] {
            assert_eq!(Part::unpack(p.pack()), p);
        }
    }
}
