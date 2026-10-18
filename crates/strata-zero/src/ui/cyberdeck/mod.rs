//! The CYBERDECK tab's deck as hardware: what it is made of, where its parts sit
//! and how the pointer moves between them. It works on a `DeckView` snapshot, never
//! on the game state.

pub mod pawn;

use game::DeckLook;
use ratatui::layout::Rect;

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
    /// `None`: no deck equipped.
    pub look: Option<DeckLook>,
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

/// The drawing's size for a look; no deck is just the glass and the inventory.
pub fn size(look: Option<DeckLook>) -> (u16, u16) {
    match look {
        Some(DeckLook::Pawn) => (pawn::W, pawn::H),
        None => (pawn::W, 12),
    }
}

/// The parts of the deck and where they sit, relative to the drawing's origin.
pub fn parts(view: &DeckView) -> Vec<(Part, Rect)> {
    match view.look {
        Some(DeckLook::Pawn) => pawn::parts(view),
        None => pawn::inventory(view, 9),
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
            look: Some(DeckLook::Pawn),
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
