//! The map viewport: camera, screen/tile mapping and the MapView widget with overlays.

use crate::render::lighting::{apply_vis, sky_of};
use crate::render::map::{MapRef, TileMap};
use crate::render::style::StylePack;
use crate::render::tileset::{MASK_E, MASK_N, MASK_S, MASK_W, cell, connects};
use crate::render::{Cell, RenderCtx, Visibility};
use citygen::{Grid, Pos, Size, StratumKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

/// Eased camera centred on a tile.
#[derive(Clone, Copy, Debug, Default)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub tx: i32,
    pub ty: i32,
}

impl Camera {
    pub fn snap(&mut self, p: Pos) {
        self.tx = p.x;
        self.ty = p.y;
        self.x = p.x as f32;
        self.y = p.y as f32;
    }

    /// Lands on the target at once: no easing across a map change.
    pub fn settle(&mut self) {
        self.x = self.tx as f32;
        self.y = self.ty as f32;
    }

    pub fn target(&mut self, p: Pos) {
        self.tx = p.x;
        self.ty = p.y;
    }

    /// Moves the target only when `p` leaves the middle third of the view.
    pub fn follow(&mut self, p: Pos, view_w: u16, view_h: u16) {
        let zx = (view_w as i32 / 6).max(1);
        let zy = (view_h as i32 / 6).max(1);
        if p.x < self.tx - zx {
            self.tx = p.x + zx;
        } else if p.x > self.tx + zx {
            self.tx = p.x - zx;
        }
        if p.y < self.ty - zy {
            self.ty = p.y + zy;
        } else if p.y > self.ty + zy {
            self.ty = p.y - zy;
        }
    }

    /// Eases a quarter of the way toward the target each tick.
    pub fn update(&mut self) {
        self.x += (self.tx as f32 - self.x) * 0.25;
        self.y += (self.ty as f32 - self.y) * 0.25;
        if (self.tx as f32 - self.x).abs() < 0.05 {
            self.x = self.tx as f32;
        }
        if (self.ty as f32 - self.y).abs() < 0.05 {
            self.y = self.ty as f32;
        }
    }

    /// Keeps the target inside the map, with the view never scrolling past the edge.
    pub fn clamp(&mut self, size: Size, view_w: u16, view_h: u16) {
        let half_w = view_w as i32 / 2;
        let half_h = view_h as i32 / 2;
        let w = size.w as i32;
        let h = size.h as i32;
        // The view spans origin..origin+view, so the far bound is the view's other half.
        self.tx = if w > view_w as i32 {
            self.tx.clamp(half_w, w - (view_w as i32 - half_w))
        } else {
            w / 2
        };
        self.ty = if h > view_h as i32 {
            self.ty.clamp(half_h, h - (view_h as i32 - half_h))
        } else {
            h / 2
        };
    }

    /// Top-left tile shown in a view of the given size.
    pub fn origin(&self, view_w: u16, view_h: u16) -> Pos {
        Pos::new(
            self.x.round() as i32 - view_w as i32 / 2,
            self.y.round() as i32 - view_h as i32 / 2,
        )
    }
}

pub fn screen_to_tile(area: Rect, origin: Pos, (sx, sy): (u16, u16)) -> Pos {
    Pos::new(
        origin.x + (sx as i32 - area.x as i32),
        origin.y + (sy as i32 - area.y as i32),
    )
}

pub fn tile_to_screen(area: Rect, origin: Pos, p: Pos) -> Option<(u16, u16)> {
    let sx = p.x - origin.x + area.x as i32;
    let sy = p.y - origin.y + area.y as i32;
    let inside = sx >= area.x as i32
        && sy >= area.y as i32
        && sx < area.right() as i32
        && sy < area.bottom() as i32;
    inside.then_some((sx as u16, sy as u16))
}

/// Something drawn over the base tiles.
pub trait Overlay {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        style: &StylePack,
    );
}

/// Bits N=1, E=2, S=4, W=8 for the neighbours of `p` whose terrain joins `terrain`.
pub(crate) fn wall_mask(map: &MapRef, p: Pos, terrain: citygen::Terrain) -> u8 {
    let mut mask = 0u8;
    for (d, bit) in [
        ((0, -1), MASK_N),
        ((1, 0), MASK_E),
        ((0, 1), MASK_S),
        ((-1, 0), MASK_W),
    ] {
        if let Some(n) = map.tile(p.offset(d.0, d.1))
            && connects(terrain, n.terrain)
        {
            mask |= bit;
        }
    }
    mask
}

/// Draws one map into an area, then the overlays, then the cursor.
pub struct MapView<'a> {
    pub map: &'a MapRef<'a>,
    pub origin: Pos,
    pub tick: u64,
    pub style: &'a StylePack,
    pub vis: Option<&'a Grid<Visibility>>,
    pub overlays: &'a [&'a dyn Overlay],
    pub cursor: Option<Pos>,
}

impl Widget for MapView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let map = self.map;
        let background = Cell {
            ch: ' ',
            fg: self.style.background,
            bg: self.style.background,
            bold: false,
            dim: false,
        };
        // A floor takes the street's palette tint.
        let stratum = map.stratum().unwrap_or(StratumKind::Street);
        for sy in area.y..area.bottom() {
            for sx in area.x..area.right() {
                let p = screen_to_tile(area, self.origin, (sx, sy));
                let Some(tile) = map.tile(p) else {
                    // Beyond the edge: the map's own backdrop, remembered-dim, or nothing.
                    match map.backdrop(p) {
                        Some(back) => {
                            let ctx = RenderCtx {
                                style: self.style,
                                theme: map.theme(p),
                                stratum,
                                light: 150,
                                tick: self.tick,
                                vis: Visibility::Remembered,
                                connector: None,
                                kind: None,
                                wall_mask: 0,
                                lamps_on: map.lamps_on(),
                                sky: sky_of(map),
                                glow: None,
                            };
                            apply_vis(cell(&back, &ctx), Visibility::Remembered).write(buf, sx, sy);
                        }
                        None => background.write(buf, sx, sy),
                    }
                    continue;
                };
                let mask = wall_mask(map, p, tile.terrain);
                let vis = self
                    .vis
                    .and_then(|g| g.get(p).copied())
                    .unwrap_or(Visibility::Visible);
                let ctx = RenderCtx {
                    style: self.style,
                    theme: map.theme(p),
                    stratum,
                    light: map.light(p),
                    tick: self.tick,
                    vis,
                    connector: map.connector(p),
                    kind: map.kind(p),
                    wall_mask: mask,
                    lamps_on: map.lamps_on_at(p),
                    sky: sky_of(map),
                    glow: map.glow(p),
                };
                apply_vis(cell(tile, &ctx), vis).write(buf, sx, sy);
            }
        }
        for o in self.overlays {
            o.draw(map, self.origin, area, self.tick, buf, self.style);
        }
        if let Some(c) = self.cursor
            && let Some((sx, sy)) = tile_to_screen(area, self.origin, c)
        {
            let cur = &mut buf[(sx, sy)];
            let fg = cur.fg;
            let bg = cur.bg;
            cur.set_fg(bg).set_bg(fg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::StylePack;
    use crate::render::weather::Weather;
    use citygen::{GenParams, Generator, Pos, SizePreset, StratumKind};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Widget;

    #[test]
    fn mapview_clips_at_all_edges_and_tiny_areas() {
        let c = Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let s = StylePack::load().unwrap();
        for origin in [
            Pos::new(-50, -50),
            Pos::new(70, 30),
            Pos::new(0, 0),
            Pos::new(200, 200),
        ] {
            for (w, h) in [(1u16, 1u16), (120, 40), (10, 3)] {
                let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                t.draw(|f| {
                    MapView {
                        map: &MapRef::stratum_of(&c, StratumKind::Street),
                        origin,
                        tick: 5,
                        style: &s,
                        vis: None,
                        overlays: &[&Weather],
                        cursor: Some(origin),
                    }
                    .render(f.area(), f.buffer_mut())
                })
                .unwrap();
            }
        }
        let area = ratatui::layout::Rect::new(2, 3, 50, 20);
        assert_eq!(
            screen_to_tile(area, Pos::new(10, 10), (2, 3)),
            Pos::new(10, 10)
        );
        assert_eq!(
            tile_to_screen(area, Pos::new(10, 10), Pos::new(12, 11)),
            Some((4, 4))
        );
        assert_eq!(
            tile_to_screen(area, Pos::new(10, 10), Pos::new(9, 11)),
            None
        );
    }

    #[test]
    fn mapview_renders_every_partial_generation_state() {
        use citygen::{Generator, Progress};
        let s = StylePack::load().unwrap();
        let mut g = Generator::new(GenParams {
            seed: 11,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let mut t = Terminal::new(TestBackend::new(90, 45)).unwrap();
        let mut steps = 0u32;
        loop {
            let progress = g.step();
            steps += 1;
            if steps.is_multiple_of(7) || matches!(progress, Progress::Done) {
                for k in StratumKind::PHYSICAL {
                    t.draw(|f| {
                        MapView {
                            map: &MapRef::stratum_of(g.city(), k),
                            origin: Pos::new(-2, -2),
                            tick: steps as u64,
                            style: &s,
                            vis: None,
                            overlays: &[&Weather],
                            cursor: None,
                        }
                        .render(f.area(), f.buffer_mut())
                    })
                    .unwrap();
                }
            }
            if matches!(progress, Progress::Done) {
                break;
            }
        }
    }

    #[test]
    fn camera_follow_has_deadzone_and_eases() {
        let mut cam = Camera::default();
        cam.snap(Pos::new(50, 50));
        cam.follow(Pos::new(52, 50), 60, 30);
        assert_eq!((cam.tx, cam.ty), (50, 50));
        cam.follow(Pos::new(90, 50), 60, 30);
        assert!(cam.tx > 50);
        let before = cam.x;
        cam.update();
        assert!(cam.x > before && cam.x < cam.tx as f32);
        cam.clamp(citygen::Size { w: 100, h: 40 }, 60, 30);
        assert!(cam.tx <= 70 && cam.ty <= 25);
        let o = cam.origin(60, 30);
        assert_eq!(o.x, cam.x.round() as i32 - 30);
    }
}
