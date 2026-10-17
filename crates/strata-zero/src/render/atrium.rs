//! The look-down: from an upper level of an atrium megablock, the ground plaza seen
//! through the open drop, dimmer the higher you stand. Terrain and furniture only.

use crate::render::Visibility;
use crate::render::lighting::apply_vis;
use crate::render::map::{MapRef, TileMap};
use crate::render::style::StylePack;
use crate::render::tileset::{MASK_E, MASK_N, MASK_S, MASK_W, cell, connects};
use crate::render::viewport::{Overlay, screen_to_tile};
use crate::render::{Cell, RenderCtx, Rgb};
use citygen::{Floor, Grid, StratumKind, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// How bright the plaza reads from `level`: the remembered-tile dim at the first
/// floor, a little darker each floor up, never quite black.
pub fn look_down_scale(level: i32) -> f32 {
    (0.35 - 0.02 * (level - 1).max(0) as f32).max(0.12)
}

/// The faintest a plaza glyph seen right now is drawn (luminance out of 255): the
/// bare floor's dots still read from the top of the tallest megablock.
const MIN_LUMA: f32 = 22.0;

/// How far under the walkway at the same spot a plaza glyph may reach, as a share
/// of the walkway's luminance.
const UNDER_WALKWAY: f32 = 0.85;

fn luma(c: Rgb) -> f32 {
    0.2126 * c.0 as f32 + 0.7152 * c.1 as f32 + 0.0722 * c.2 as f32
}

/// A plaza glyph's colour from `level`, seen now: scaled down by height, but never
/// under [`MIN_LUMA`] (nor brighter than it was), so the floor stays faint, not black.
pub fn look_down_fg(fg: Rgb, level: i32) -> Rgb {
    let l = luma(fg);
    if l <= 0.0 {
        return fg;
    }
    let f = look_down_scale(level).max(MIN_LUMA / l).min(1.0);
    fg.scale(f)
}

/// `fg` darkened, if need be, to sit clearly under `walkway`.
fn under(fg: Rgb, walkway: Rgb) -> Rgb {
    let (l, cap) = (luma(fg), luma(walkway) * UNDER_WALKWAY);
    if l <= cap || l <= 0.0 {
        fg
    } else {
        fg.scale(cap / l)
    }
}

pub struct AtriumOverlay<'a> {
    /// Level 0 of the same building.
    pub ground: &'a Floor,
    /// The level the player stands on.
    pub level: i32,
    pub vis: &'a Grid<Visibility>,
}

impl Overlay for AtriumOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: citygen::Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        style: &StylePack,
    ) {
        let f = look_down_scale(self.level);
        let ground = |p| self.ground.tiles.get(p);
        for sy in area.y..area.bottom() {
            for sx in area.x..area.right() {
                let p = screen_to_tile(area, origin, (sx, sy));
                if map.tile(p).is_none_or(|t| t.terrain != Terrain::Atrium)
                    || self.vis.get(p).is_none_or(|v| *v == Visibility::Unseen)
                {
                    continue;
                }
                let Some(tile) = ground(p) else { continue };
                let mut mask = 0u8;
                for (d, bit) in [
                    ((0, -1), MASK_N),
                    ((1, 0), MASK_E),
                    ((0, 1), MASK_S),
                    ((-1, 0), MASK_W),
                ] {
                    if let Some(n) = ground(p.offset(d.0, d.1))
                        && connects(tile.terrain, n.terrain)
                    {
                        mask |= bit;
                    }
                }
                let ctx = RenderCtx {
                    style,
                    theme: map.theme(p),
                    stratum: StratumKind::Street,
                    light: self.ground.light.get(p).copied().unwrap_or(0),
                    tick: 0,
                    vis: Visibility::Visible,
                    connector: None,
                    kind: map.kind(p),
                    wall_mask: mask,
                };
                let c = cell(tile, &ctx);
                // The walkway this level would show here, at this tile's light: the
                // look-down always sits under it.
                let walkway = map.tile(p).map(|t| {
                    let bare = citygen::Tile {
                        terrain: Terrain::Floor,
                        feature: None,
                        ..*t
                    };
                    let wctx = RenderCtx {
                        style,
                        theme: map.theme(p),
                        stratum: map.stratum().unwrap_or(StratumKind::Street),
                        light: map.light(p),
                        tick: 0,
                        vis: Visibility::Visible,
                        connector: None,
                        kind: map.kind(p),
                        wall_mask: 0,
                    };
                    cell(&bare, &wctx).fg
                });
                let vis = self.vis.get(p).copied().unwrap_or(Visibility::Visible);
                // Only what is seen right now keeps the legible minimum.
                let fg = if vis == Visibility::Visible {
                    look_down_fg(c.fg, self.level)
                } else {
                    c.fg.scale(f)
                };
                let fg = walkway.map_or(fg, |w| under(fg, w));
                // Scaled once here, then remembered the way the rest of the level is.
                apply_vis(
                    Cell {
                        fg,
                        bg: c.bg.scale(f),
                        bold: false,
                        dim: false,
                        ..c
                    },
                    vis,
                )
                .write(buf, sx, sy);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interiors::Interiors;
    use crate::render::map::FloorMap;
    use crate::render::viewport::{MapView, tile_to_screen};
    use citygen::{Feature, Furniture, GenParams, Generator, Pos, SizePreset};
    use ratatui::style::Color;
    use ratatui::widgets::Widget;

    #[test]
    fn the_plaza_dims_with_height_to_a_floor() {
        assert_eq!(look_down_scale(1), 0.35);
        assert!(look_down_scale(5) < look_down_scale(2));
        assert_eq!(look_down_scale(60), 0.12);
    }

    fn rgb(c: Color) -> u32 {
        match c {
            Color::Rgb(r, g, b) => r as u32 + g as u32 + b as u32,
            other => panic!("not truecolor: {other:?}"),
        }
    }

    #[test]
    fn the_atrium_shows_the_plaza_table_dimmed_and_unseen_stays_black() {
        let city = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let home = citygen::home(&city).expect("seed 8 has a home");
        let mut interiors = Interiors::default();
        interiors
            .ensure(&city, home.building, home.level)
            .expect("the home level");
        let (plan, upper) = interiors.floor(home.building, home.level).unwrap();
        let (_, ground) = interiors.floor(home.building, 0).unwrap();
        let atrium: Vec<Pos> = upper
            .tiles
            .iter()
            .filter(|(_, t)| t.terrain == Terrain::Atrium)
            .map(|(p, _)| p)
            .collect();
        let table = *atrium
            .iter()
            .find(|p| ground.tiles[**p].feature == Some(Feature::Furniture(Furniture::Table)))
            .expect("a plaza table under the atrium");
        let unseen = *atrium
            .iter()
            .find(|p| (2..10).contains(&p.chebyshev(table)))
            .expect("another atrium cell");
        let mut vis = Grid::filled(upper.tiles.size(), Visibility::Visible);
        vis[unseen] = Visibility::Unseen;
        let s = StylePack::load().unwrap();
        let map = MapRef::Floor(FloorMap {
            city: &city,
            plan,
            floor: upper,
        });
        let overlay = AtriumOverlay {
            ground,
            level: home.level,
            vis: &vis,
        };
        let area = Rect::new(0, 0, 60, 30);
        let origin = Pos::new(table.x - 30, table.y - 15);
        let mut buf = Buffer::empty(area);
        MapView {
            map: &map,
            origin,
            tick: 0,
            style: &s,
            vis: Some(&vis),
            overlays: &[&overlay],
            cursor: None,
        }
        .render(area, &mut buf);
        let (tx, ty) = tile_to_screen(area, origin, table).unwrap();
        let drawn = &buf[(tx, ty)];
        assert_eq!(drawn.symbol(), "○");
        let plain = cell(
            &ground.tiles[table],
            &RenderCtx {
                style: &s,
                theme: Some(plan.theme),
                stratum: StratumKind::Street,
                light: ground.light[table],
                tick: 0,
                vis: Visibility::Visible,
                connector: None,
                kind: Some(plan.kind),
                wall_mask: 0,
            },
        );
        assert!(
            rgb(drawn.fg) < rgb(plain.fg.to_color()),
            "{:?} vs level 0's {:?}",
            drawn.fg,
            plain.fg
        );
        let (ux, uy) = tile_to_screen(area, origin, unseen).unwrap();
        let u = &buf[(ux, uy)];
        assert_eq!(u.symbol(), " ");
        assert_eq!(u.fg, Color::Rgb(0, 0, 0));
        assert_eq!(u.bg, Color::Rgb(0, 0, 0));
    }

    fn luma_of(c: Color) -> f32 {
        match c {
            Color::Rgb(r, g, b) => 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32,
            other => panic!("not truecolor: {other:?}"),
        }
    }

    /// The bare plaza floor and the walkway beside the railing, drawn from `level`
    /// with both at `vis` on `floor` (the upper level): the two fg colours and
    /// whether the plaza cell carries DIM.
    fn plaza_and_walkway(
        city: &citygen::City,
        plan: &citygen::FloorPlan,
        upper: &Floor,
        ground: &Floor,
        level: i32,
        vis: Visibility,
    ) -> (Color, Color, bool) {
        let plaza = upper
            .tiles
            .iter()
            .find(|(p, t)| {
                t.terrain == Terrain::Atrium
                    && ground.tiles[*p].terrain == Terrain::Floor
                    && ground.tiles[*p].feature.is_none()
            })
            .map(|(p, _)| p)
            .expect("bare plaza floor under the atrium");
        let walkway = upper
            .tiles
            .iter()
            .find(|(p, t)| {
                t.terrain == Terrain::Floor
                    && t.feature.is_none()
                    && p.neighbors4().any(|n| {
                        upper
                            .tiles
                            .get(n)
                            .is_some_and(|u| u.terrain == Terrain::Railing)
                    })
                    && p.chebyshev(plaza) < 25
            })
            .map(|(p, _)| p)
            .expect("a walkway tile by the railing");
        let vis = Grid::filled(upper.tiles.size(), vis);
        let s = StylePack::load().unwrap();
        let map = MapRef::Floor(FloorMap {
            city,
            plan,
            floor: upper,
        });
        let overlay = AtriumOverlay {
            ground,
            level,
            vis: &vis,
        };
        let area = Rect::new(0, 0, 60, 60);
        let origin = Pos::new(plaza.x - 30, plaza.y - 30);
        let mut buf = Buffer::empty(area);
        MapView {
            map: &map,
            origin,
            tick: 0,
            style: &s,
            vis: Some(&vis),
            overlays: &[&overlay],
            cursor: None,
        }
        .render(area, &mut buf);
        let at = |p: Pos| {
            let (x, y) = tile_to_screen(area, origin, p).unwrap();
            buf[(x, y)].clone()
        };
        let (f, w) = (at(plaza), at(walkway));
        (
            f.fg,
            w.fg,
            f.modifier.contains(ratatui::style::Modifier::DIM),
        )
    }

    #[test]
    fn from_the_top_the_plaza_floor_is_faint_but_under_the_walkway() {
        let city = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let home = citygen::home(&city).expect("seed 8 has a home");
        let mut interiors = Interiors::default();
        interiors.ensure(&city, home.building, home.level).unwrap();
        interiors.ensure(&city, home.building, 0).unwrap();
        let (plan, upper) = interiors.floor(home.building, home.level).unwrap();
        let (_, ground) = interiors.floor(home.building, 0).unwrap();
        // From the top of the building, the darkest the look-down gets here: seen
        // now, the floor is faint but legible, scaled once, under the walkway.
        let (f, w, dim) =
            plaza_and_walkway(&city, plan, upper, ground, plan.top, Visibility::Visible);
        assert!(!dim, "scaled once, not dimmed again");
        let (lf, lw) = (luma_of(f), luma_of(w));
        assert!(
            lf >= MIN_LUMA - 0.5,
            "plaza floor {f:?} reads as black ({lf})"
        );
        assert!(lf < lw, "plaza floor {lf} not under the walkway {lw}");
        // Remembered, from any height, it stays under the remembered walkway.
        for level in [1, home.level, plan.top] {
            let (f, w, _) =
                plaza_and_walkway(&city, plan, upper, ground, level, Visibility::Remembered);
            let (lf, lw) = (luma_of(f), luma_of(w));
            assert!(
                lf < lw,
                "remembered from {level}: plaza {lf} over walkway {lw}"
            );
        }
        // And over an unlit walkway, seen or remembered.
        let mut dark = upper.clone();
        dark.light = Grid::filled(dark.light.size(), 0);
        for vis in [Visibility::Visible, Visibility::Remembered] {
            for level in [1, plan.top] {
                let (f, w, _) = plaza_and_walkway(&city, plan, &dark, ground, level, vis);
                let (lf, lw) = (luma_of(f), luma_of(w));
                assert!(
                    lf < lw,
                    "unlit {vis:?} from {level}: plaza {lf} over walkway {lw}"
                );
            }
        }
    }
}
