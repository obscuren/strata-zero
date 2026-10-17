//! The look-down: from an upper level of an atrium megablock, the ground plaza seen
//! through the open drop, dimmer the higher you stand. Terrain and furniture only.

use crate::render::Visibility;
use crate::render::map::{MapRef, TileMap};
use crate::render::style::StylePack;
use crate::render::tileset::{MASK_E, MASK_N, MASK_S, MASK_W, cell, connects};
use crate::render::viewport::{Overlay, screen_to_tile};
use crate::render::{Cell, RenderCtx};
use citygen::{Floor, Grid, StratumKind, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// How bright the plaza reads from `level`: the remembered-tile dim at the first
/// floor, a little darker each floor up, never quite black.
pub fn look_down_scale(level: i32) -> f32 {
    (0.35 - 0.02 * (level - 1).max(0) as f32).max(0.12)
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
                Cell {
                    fg: c.fg.scale(f),
                    bg: c.bg.scale(f),
                    bold: false,
                    dim: true,
                    ..c
                }
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
}
