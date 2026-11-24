//! A turret on the map: its arrowhead on its steel body, coloured by what it's doing.

use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use crate::turrets::TurretState;
use crate::turrets::field::glyph;
use citygen::{Dir, Grid, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// The steel it's bolted in.
pub const BODY: Rgb = Rgb(0x2b, 0x2f, 0x36);
pub const ASLEEP: Rgb = Rgb(0x8a, 0x4a, 0x4e);
/// Awake or hunting.
pub const AWAKE: Rgb = Rgb(0xff, 0x3c, 0x46);
pub const BLIND: Rgb = Rgb(0x5b, 0x6b, 0x7d);
pub const YOURS: Rgb = Rgb(0x5f, 0xd3, 0x8a);
pub const WRECKED: Rgb = Rgb(0x3a, 0x3f, 0x47);

/// Its arrowhead's colour by state, and whether it's bold.
pub fn colour(s: TurretState) -> (Rgb, bool) {
    match s {
        TurretState::Asleep => (ASLEEP, true),
        TurretState::Awake | TurretState::Hunting => (AWAKE, true),
        TurretState::Blind => (BLIND, false),
        TurretState::Yours => (YOURS, true),
        TurretState::Wrecked => (WRECKED, false),
    }
}

/// The turrets of a map: hidden on unseen tiles, dimmed on remembered ones; with no
/// visibility grid every one shows.
pub struct TurretLooks<'a> {
    pub looks: &'a [(Pos, Dir, TurretState)],
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for TurretLooks<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for &(pos, facing, state) in self.looks {
            let Some((sx, sy)) = tile_to_screen(area, origin, pos) else {
                continue;
            };
            let seen = match self.vis {
                None => Visibility::Visible,
                Some(v) => v.get(pos).copied().unwrap_or(Visibility::Unseen),
            };
            let strength = match seen {
                Visibility::Visible => 1.0,
                Visibility::Remembered => 0.45,
                Visibility::Unseen => continue,
            };
            let (fg, bold) = colour(state);
            let cell = &mut buf[(sx, sy)];
            cell.set_char(glyph(facing))
                .set_fg(fg.scale(strength).to_color())
                .set_bg(BODY.scale(strength).to_color());
            if bold {
                cell.modifier.insert(Modifier::BOLD);
            } else {
                cell.modifier.remove(Modifier::BOLD);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::testutil::{on, ward};
    use citygen::Size;
    use ratatui::style::Color;

    /// One turret at (1,1) facing SE in `state`, drawn over a blank 3x3 view at the origin
    /// with its tile `seen`: the cell it lands on.
    fn drawn(state: TurretState, seen: Visibility) -> ratatui::buffer::Cell {
        let st = ward();
        let map = crate::foes::map_of(&st, on(0)).unwrap();
        let looks = [(Pos::new(1, 1), Dir::SE, state)];
        let mut vis = Grid::filled(Size { w: 3, h: 3 }, Visibility::Visible);
        vis.set(Pos::new(1, 1), seen);
        let area = Rect::new(0, 0, 3, 3);
        let style = StylePack::load().unwrap();
        let mut buf = Buffer::empty(area);
        let overlay = TurretLooks {
            looks: &looks,
            vis: Some(&vis),
        };
        overlay.draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        buf[(1, 1)].clone()
    }

    #[test]
    fn each_state_draws_its_arrowhead_and_colour_on_the_steel_body() {
        use TurretState::*;
        for (state, fg, bold) in [
            (Asleep, ASLEEP, true),
            (Awake, AWAKE, true),
            (Hunting, AWAKE, true),
            (Blind, BLIND, false),
            (Yours, YOURS, true),
            (Wrecked, WRECKED, false),
        ] {
            let cell = drawn(state, Visibility::Visible);
            assert_eq!(cell.symbol(), "◢", "{state:?}");
            assert_eq!(cell.bg, Color::Rgb(0x2b, 0x2f, 0x36), "{state:?}");
            assert_eq!(cell.fg, fg.to_color(), "{state:?}");
            assert_eq!(
                cell.modifier.contains(Modifier::BOLD),
                bold,
                "{state:?} bold"
            );
            assert_eq!(colour(state), (fg, bold));
        }
    }

    #[test]
    fn remembered_it_is_dimmed_and_unseen_it_is_not_drawn() {
        let cell = drawn(TurretState::Awake, Visibility::Remembered);
        assert_eq!(cell.symbol(), "◢");
        assert_eq!(cell.fg, AWAKE.scale(0.45).to_color());
        assert_eq!(cell.bg, BODY.scale(0.45).to_color());
        let cell = drawn(TurretState::Awake, Visibility::Unseen);
        assert_eq!(cell, ratatui::buffer::Cell::default(), "unseen: untouched");
    }

    #[test]
    fn with_no_sight_grid_every_one_shows() {
        let st = ward();
        let map = crate::foes::map_of(&st, on(0)).unwrap();
        let looks = [
            (Pos::new(0, 0), Dir::N, TurretState::Blind),
            (Pos::new(2, 0), Dir::W, TurretState::Yours),
        ];
        let area = Rect::new(0, 0, 3, 1);
        let style = StylePack::load().unwrap();
        let mut buf = Buffer::empty(area);
        let overlay = TurretLooks {
            looks: &looks,
            vis: None,
        };
        overlay.draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        assert_eq!(buf[(0, 0)].symbol(), "▲");
        assert_eq!(buf[(2, 0)].symbol(), "◄");
        assert_eq!(buf[(2, 0)].fg, YOURS.to_color());
    }
}
