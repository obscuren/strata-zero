//! The city's maps as half-block pixels, for the deck's MAP and the minimap: a pixel's
//! colour from the tiles it covers, and pixels painted two to a cell.

use crate::render::Rgb;
use crate::render::style::{StylePack, ThemeStyle};
use crate::ui::chrome::PANEL_BG_RGB;
use citygen::{City, DistrictId, Grid, Pos, RoomKind, StratumKind, Terrain, Tile};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// The ground of a pixel no district colours.
pub const NEUTRAL: Rgb = Rgb(40, 44, 52);
pub const WATER: Rgb = Rgb(14, 40, 72);

/// Walkable ground under whatever stands on it: a stall on a pavement is pavement.
fn ground(t: &Tile) -> bool {
    Tile {
        feature: None,
        ..*t
    }
    .is_walkable()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Building,
    Road,
    Walk,
    Water,
}

/// The colour of the `s` × `s` pixel at tile (`x0`, `y0`) of stratum `k`: its class in
/// its commonest district's theme; none where there is nothing (off the stratum, open
/// air). On the Net, walls, gates and the floors inside them are its blocks, lanes its
/// roads, and the open lattice is nothing.
pub fn stratum_px(
    city: &City,
    style: &StylePack,
    k: StratumKind,
    x0: i32,
    y0: i32,
    s: i32,
) -> Option<Rgb> {
    let stratum = city.stratum(k)?;
    let (mut n, mut solid, mut water, mut road, mut walk) = (0, 0, 0, false, false);
    // The districts met, and how often; a pixel meets few.
    let mut districts: [(Option<DistrictId>, u32); 8] = [(None, 0); 8];
    for y in y0..y0 + s {
        for x in x0..x0 + s {
            let p = Pos::new(x, y);
            let Some(t) = stratum.tiles.get(p) else {
                continue;
            };
            n += 1;
            match t.terrain {
                Terrain::Wall
                | Terrain::Mass
                | Terrain::Window
                | Terrain::Rooftop
                | Terrain::Door
                | Terrain::NetWall
                | Terrain::NetGate
                | Terrain::NetFloor => solid += 1,
                Terrain::Water | Terrain::Sewage => water += 1,
                Terrain::Road | Terrain::NetLane => road = true,
                _ => {}
            }
            walk |= ground(t) && t.terrain != Terrain::NetGrid;
            if let Some(d) = t.district
                && let Some(slot) = districts
                    .iter_mut()
                    .find(|(id, _)| id.is_none_or(|id| id == d))
            {
                *slot = (Some(d), slot.1 + 1);
            }
        }
    }
    let class = if solid * 2 > n {
        Class::Building
    } else if road {
        Class::Road
    } else if walk {
        Class::Walk
    } else if n > 0 && water * 2 >= n {
        Class::Water
    } else {
        return None;
    };
    // The commonest, the first met on a tie.
    let district = districts
        .iter()
        .fold(
            (None, 0),
            |best, (d, c)| if *c > best.1 { (*d, *c) } else { best },
        )
        .0;
    let theme = district
        .and_then(|d| city.district_opt(d))
        .map(|d| style.theme_or_neutral(Some(d.theme)));
    Some(match (class, theme) {
        (Class::Water, _) => WATER,
        (_, None) => NEUTRAL,
        (Class::Building, Some(t)) => t.roof,
        (Class::Road, Some(t)) => t.road.scale(1.9),
        (Class::Walk, Some(t)) => t.sidewalk.scale(1.15),
    })
}

/// What a floor's or a construct's plan is drawn in, from its building's district theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanColours {
    pub wall: Rgb,
    /// Floor in no room, or in a corridor.
    pub corridor: Rgb,
    pub room: Rgb,
    pub open_plan: Rgb,
    /// Your flat.
    pub home: Rgb,
    /// An atrium's railing.
    pub rail: Rgb,
    /// The atrium's drop.
    pub pit: Rgb,
}

impl PlanColours {
    pub fn of(t: &ThemeStyle) -> PlanColours {
        let room = t.roof.scale(1.1);
        PlanColours {
            wall: PANEL_BG_RGB.mix(t.roof, 0.3),
            corridor: t.road.scale(2.1),
            room,
            open_plan: t.roof.scale(1.25),
            home: room.mix(Rgb(124, 255, 154), 0.22),
            rail: t.roof.mix(Rgb(230, 230, 240), 0.25),
            pit: Rgb(5, 7, 11),
        }
    }
}

/// The colour of the `s` × `s` pixel at tile (`x0`, `y0`) of a plan: a railing, the
/// drop, a wall (a wall a tile thick always shows), else the room most of its floor lies
/// in; none where the plan has no tile. `room` says which room a tile is in.
pub fn plan_px(
    tiles: &Grid<Tile>,
    room: impl Fn(Pos) -> Option<RoomKind>,
    c: &PlanColours,
    x0: i32,
    y0: i32,
    s: i32,
) -> Option<Rgb> {
    let (mut n, mut wall, mut pit, mut rail) = (0, 0, 0, false);
    // The rooms the floor meets, and how often; `None` is no room.
    let mut rooms: Vec<(Option<RoomKind>, u32)> = Vec::new();
    for y in y0..y0 + s {
        for x in x0..x0 + s {
            let p = Pos::new(x, y);
            let Some(t) = tiles.get(p).filter(|t| t.terrain != Terrain::Void) else {
                continue;
            };
            n += 1;
            match t.terrain {
                Terrain::Railing => rail = true,
                Terrain::Atrium => pit += 1,
                _ if ground(t) => {
                    let r = room(p);
                    match rooms.iter_mut().find(|(k, _)| *k == r) {
                        Some(e) => e.1 += 1,
                        None => rooms.push((r, 1)),
                    }
                }
                _ => wall += 1,
            }
        }
    }
    if n == 0 {
        return None;
    }
    if rail {
        return Some(c.rail);
    }
    if pit * 2 >= n {
        return Some(c.pit);
    }
    // The commonest, the first met on a tie.
    let most = rooms.iter().fold(
        None,
        |best: Option<(Option<RoomKind>, u32)>, e| match best {
            Some(b) if b.1 >= e.1 => Some(b),
            _ => Some(*e),
        },
    );
    let Some((kind, _)) = most.filter(|_| wall * s < n) else {
        return Some(c.wall);
    };
    Some(match kind {
        None | Some(RoomKind::Corridor) => c.corridor,
        Some(RoomKind::OpenPlan) => c.open_plan,
        Some(RoomKind::Home) => c.home,
        Some(_) => c.room,
    })
}

/// Pixels two to a cell, row by row.
pub struct Pixels {
    w: i32,
    h: i32,
    v: Vec<Rgb>,
}

impl Pixels {
    /// `w` × `h` pixels, each `f(x, y)` counted from the top-left one.
    pub fn new(w: i32, h: i32, mut f: impl FnMut(i32, i32) -> Rgb) -> Pixels {
        let (w, h) = (w.max(0), h.max(0));
        let v = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect();
        Pixels { w, h, v }
    }

    pub fn width(&self) -> i32 {
        self.w
    }

    pub fn height(&self) -> i32 {
        self.h
    }

    pub fn get(&self, (x, y): (i32, i32)) -> Option<Rgb> {
        ((0..self.w).contains(&x) && (0..self.h).contains(&y))
            .then(|| self.v[(y * self.w + x) as usize])
    }

    pub fn get_mut(&mut self, (x, y): (i32, i32)) -> Option<&mut Rgb> {
        ((0..self.w).contains(&x) && (0..self.h).contains(&y))
            .then(|| &mut self.v[(y * self.w + x) as usize])
    }

    /// The top and bottom pixels of the cell `x`, `y` cells in.
    pub fn cell(&self, x: u16, y: u16) -> (Rgb, Rgb) {
        let at = |py: i32| self.v[(py * self.w + x as i32) as usize];
        (at(2 * y as i32), at(2 * y as i32 + 1))
    }
}

/// The pixels as half blocks over `area`: `▀` in the top pixel's colour on the bottom's.
pub fn paint(buf: &mut Buffer, area: Rect, px: &Pixels) {
    let cols = area.width.min(px.width() as u16);
    let rows = area.height.min((px.height() / 2) as u16);
    for y in 0..rows {
        for x in 0..cols {
            let (top, bottom) = px.cell(x, y);
            let c = &mut buf[(area.x + x, area.y + y)];
            c.reset();
            c.set_char(if top == bottom { ' ' } else { '▀' })
                .set_fg(top.to_color())
                .set_bg(bottom.to_color());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};
    use ratatui::style::Color;

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    /// The first tile of stratum `k` that `f` accepts, with a district.
    fn find(city: &City, k: StratumKind, f: impl Fn(&Tile) -> bool) -> Option<Pos> {
        let tiles = &city.stratum(k)?.tiles;
        let size = tiles.size();
        (0..size.h as i32)
            .flat_map(|y| (0..size.w as i32).map(move |x| Pos::new(x, y)))
            .find(|p| tiles.get(*p).is_some_and(|t| f(t) && t.district.is_some()))
    }

    #[test]
    fn net_lanes_read_as_streets() {
        let city = city();
        let style = StylePack::load().unwrap();
        let net = StratumKind::Net;
        let theme = |p: Pos| {
            let d = city.tile(net, p).unwrap().district.unwrap();
            style.theme_or_neutral(Some(city.district(d).theme)).clone()
        };
        let lattice = find(&city, net, |t| t.terrain == Terrain::NetGrid).expect("a lattice");
        assert_eq!(
            stratum_px(&city, &style, net, lattice.x, lattice.y, 1),
            None
        );
        let lane = find(&city, net, |t| t.terrain == Terrain::NetLane).expect("a lane");
        assert_eq!(
            stratum_px(&city, &style, net, lane.x, lane.y, 1),
            Some(theme(lane).road.scale(1.9))
        );
        if let Some(wall) = find(&city, net, |t| t.terrain == Terrain::NetWall) {
            assert_eq!(
                stratum_px(&city, &style, net, wall.x, wall.y, 1),
                Some(theme(wall).roof)
            );
        }
    }

    #[test]
    fn plan_classes() {
        use citygen::Size;
        let style = StylePack::load().unwrap();
        let c = PlanColours::of(style.theme_or_neutral(None));
        let mut tiles = Grid::filled(Size { w: 12, h: 12 }, Tile::new(Terrain::Floor));
        // A wall a tile thick down column 5, a drop at 8..10 × 0..2, a railing at 11, 11.
        for y in 0..12 {
            tiles.set(Pos::new(5, y), Tile::new(Terrain::Wall));
        }
        for (x, y) in [(8, 0), (9, 0), (8, 1), (9, 1)] {
            tiles.set(Pos::new(x, y), Tile::new(Terrain::Atrium));
        }
        tiles.set(Pos::new(11, 11), Tile::new(Terrain::Railing));
        let home = |p: Pos| (p.y >= 6 && p.x < 5).then_some(RoomKind::Home);
        let px = |x, y, s| plan_px(&tiles, home, &c, x, y, s);
        assert_eq!(px(4, 0, 2), Some(c.wall), "a thin wall shows at 2");
        assert_eq!(px(3, 0, 3), Some(c.wall), "and at 3");
        assert_eq!(px(0, 0, 2), Some(c.corridor), "floor in no room");
        assert_eq!(px(0, 6, 2), Some(c.home));
        assert_eq!(px(8, 0, 2), Some(c.pit));
        assert_eq!(px(10, 10, 2), Some(c.rail), "any railing wins");
        assert_eq!(px(12, 0, 2), None, "off the plan");
        assert_eq!(
            plan_px(&tiles, |_| Some(RoomKind::OpenPlan), &c, 0, 0, 2),
            Some(c.open_plan)
        );
        assert_eq!(
            plan_px(&tiles, |_| Some(RoomKind::Office), &c, 0, 0, 2),
            Some(c.room)
        );
    }

    #[test]
    fn paint_folds_two_pixels() {
        let (red, blue) = (Rgb(200, 0, 0), Rgb(0, 0, 200));
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 2));
        let px = Pixels::new(2, 2, |x, y| match (x, y) {
            (0, 0) => red,
            (0, _) => blue,
            _ => red,
        });
        paint(&mut buf, Rect::new(1, 1, 2, 1), &px);
        assert_eq!(buf[(1, 1)].symbol(), "▀");
        assert_eq!(buf[(1, 1)].fg, Color::Rgb(200, 0, 0));
        assert_eq!(buf[(1, 1)].bg, Color::Rgb(0, 0, 200));
        assert_eq!(buf[(2, 1)].symbol(), " ");
        // A canvas smaller than its area paints what it has.
        paint(
            &mut buf,
            Rect::new(0, 0, 3, 2),
            &Pixels::new(1, 1, |_, _| red),
        );
    }
}
