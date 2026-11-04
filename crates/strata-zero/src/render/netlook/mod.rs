//! The Net's look, "relief" (spec `docs/superpowers/specs/2026-11-03-net-look-design.md`):
//! the void dark, lanes as beams, constructs stepped by their security; inside a
//! construct its body in its security's colour, rooms lit by what they hold and the
//! spine each piece of ICE watches tinted. Drawn over the whole map area as the first
//! overlay on the Net; everything else draws on top as before.

pub mod grid;
pub mod inside;
pub mod mini;
pub mod packets;
pub mod plate;

use crate::anim::effects::pulse;
use crate::render::map::MapRef;
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use crate::render::tileset::{DOUBLE, HEAVY, LIGHT};
use crate::render::viewport::Overlay;
use crate::render::{Cell, Rgb};
use citygen::{BuildingId, City, IceGrade, NodeKind, Pos, StratumKind, SubnetOwner};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::HashMap;

pub const VOID: Rgb = Rgb(1, 2, 4);
pub const WHITE: Rgb = Rgb(230, 240, 245);
pub const OPEN: Rgb = Rgb(63, 168, 143);
pub const ICE_BLUE: Rgb = Rgb(159, 230, 255);
pub const BLACK_RED: Rgb = Rgb(255, 40, 72);
/// The tracked building, and a vault's loot.
pub const AMBER: Rgb = Rgb(255, 176, 0);
/// Terminals, wherever the Net draws them.
pub const TERMINAL: Rgb = Rgb(140, 150, 255);
/// Rooms inside a construct.
pub const ROOM: Rgb = Rgb(0, 229, 255);
/// You, on the Net and in a construct: blue against the Net's greens and cyans.
pub const YOU: Rgb = Rgb(70, 140, 255);
/// The glow behind you there.
pub const YOU_GLOW: Rgb = Rgb(14, 34, 84);

/// Light box drawing with dashed straights: the open tier's wall.
const DASHED: [char; 16] = [
    '┄', '┆', '┄', '└', '┆', '┆', '┌', '├', '┄', '┘', '┄', '┴', '┐', '┤', '┬', '┼',
];

/// How hard a construct is, read from its walls: one tier per band of security.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Open,
    Guarded,
    Ice,
    Black,
}

impl Tier {
    /// Below 100 open, to 159 guarded, to 229 ICE (white and grey), then black.
    pub fn of(security: u8) -> Tier {
        match security {
            0..=99 => Tier::Open,
            100..=159 => Tier::Guarded,
            160..=229 => Tier::Ice,
            _ => Tier::Black,
        }
    }

    pub fn colour(self) -> Rgb {
        match self {
            Tier::Open => OPEN,
            Tier::Guarded => SIGNAL,
            Tier::Ice => ICE_BLUE,
            Tier::Black => BLACK_RED,
        }
    }

    pub fn walls(self) -> &'static [char; 16] {
        match self {
            Tier::Open => &DASHED,
            Tier::Guarded => &LIGHT,
            Tier::Ice => &HEAVY,
            Tier::Black => &DOUBLE,
        }
    }

    /// The steps a construct of this tier rises, seen from the open Net.
    pub fn steps(self) -> i32 {
        match self {
            Tier::Open => 1,
            Tier::Guarded => 2,
            Tier::Ice => 3,
            Tier::Black => 4,
        }
    }
}

/// The security of a building's subnet; none for a building without one.
pub fn security_of(city: &City, b: BuildingId) -> Option<u8> {
    city.net
        .subnets
        .iter()
        .find(|s| s.owner == SubnetOwner::Building(b))
        .map(|s| s.security)
}

/// What a construct's plate says: your own block, a block of flats, or its name.
pub fn label_for(city: &City, b: BuildingId, home: Option<BuildingId>) -> String {
    if home == Some(b) {
        return "YOUR BLOCK".to_string();
    }
    match city.building_opt(b).and_then(|x| x.name.as_deref()) {
        Some("apartment") | None => "FLATS".to_string(),
        Some(name) => name.to_uppercase(),
    }
}

/// A piece of ICE's colour, pulsing as the tileset pulses it.
pub fn ice_colour(grade: IceGrade, tick: u64) -> Rgb {
    match grade {
        IceGrade::White => Rgb(235, 240, 250).scale(0.7 + 0.3 * pulse(tick, 20)),
        IceGrade::Grey => Rgb(255, 0, 200).scale(0.7 + 0.3 * pulse(tick, 20)),
        IceGrade::Black => Rgb(255, 40, 40).scale(0.7 + 0.3 * pulse(tick, 12)),
    }
}

/// The ICE at a construct's gate, by its security: none below 160, white to 199, then
/// grey (a gate never holds black).
pub fn gate_ice(security: u8) -> Option<IceGrade> {
    use citygen::construct::{GREY_THRESHOLD, ICE_THRESHOLD};
    match security {
        s if s >= GREY_THRESHOLD => Some(IceGrade::Grey),
        s if s >= ICE_THRESHOLD => Some(IceGrade::White),
        _ => None,
    }
}

/// The colour a node draws in: terminals their own, servers, vaults and hubs white,
/// ICE as its gate's piece, the rest halfway from their wall to white.
pub fn node_colour(kind: NodeKind, wall: Rgb, security: Option<u8>, tick: u64) -> Rgb {
    match kind {
        NodeKind::Terminal => TERMINAL,
        NodeKind::Server | NodeKind::DataVault | NodeKind::Backbone => WHITE,
        NodeKind::Ice => ice_colour(security.and_then(gate_ice).unwrap_or(IceGrade::White), tick),
        _ => wall.mix(WHITE, 0.5),
    }
}

/// What the look needs besides the map: the clock, what you track, whose home it is,
/// and the spine the ICE watches.
#[derive(Clone, Debug, Default)]
pub struct NetLookView {
    pub tick: u64,
    /// The lane tiles the traffic lights this frame (`packets::Traffic::glows`).
    pub packets: HashMap<Pos, (f32, bool)>,
    /// The nodes soaking a packet up this frame, with how bright (`Traffic::soaks`).
    pub soaks: HashMap<Pos, f32>,
    pub tracked: Option<BuildingId>,
    pub home: Option<BuildingId>,
    pub watch: HashMap<Pos, IceGrade>,
}

pub struct NetLook<'a> {
    pub view: &'a NetLookView,
}

impl Overlay for NetLook<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        style: &StylePack,
    ) {
        match map {
            MapRef::Stratum(m) if m.stratum == StratumKind::Net => {
                grid::draw(map, self.view, origin, area, buf, style)
            }
            MapRef::Construct(m) => inside::draw(m, self.view, origin, area, buf, style),
            _ => {}
        }
    }
}

/// One cell of the look.
fn put(buf: &mut Buffer, sx: u16, sy: u16, ch: char, fg: Rgb, bg: Rgb, bold: bool) {
    Cell {
        ch,
        fg,
        bg,
        bold,
        dim: false,
    }
    .write(buf, sx, sy);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_follow_security() {
        for (s, t) in [
            (0, Tier::Open),
            (99, Tier::Open),
            (100, Tier::Guarded),
            (159, Tier::Guarded),
            (160, Tier::Ice),
            (229, Tier::Ice),
            (230, Tier::Black),
            (255, Tier::Black),
        ] {
            assert_eq!(Tier::of(s), t, "{s}");
        }
        let steps: Vec<i32> = [Tier::Open, Tier::Guarded, Tier::Ice, Tier::Black]
            .iter()
            .map(|t| t.steps())
            .collect();
        assert_eq!(steps, [1, 2, 3, 4]);
        assert_eq!(Tier::Open.walls()[1], '┆');
        assert_eq!(Tier::Open.walls()[0], '┄');
        assert_eq!(Tier::Black.walls()[0], '═');
        assert_eq!(gate_ice(159), None);
        assert_eq!(gate_ice(160), Some(IceGrade::White));
        assert_eq!(gate_ice(255), Some(IceGrade::Grey));
    }
}
