//! The aim: the path you mean to walk, lit as far as your moves carry.

use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::CYAN;
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The cross of an aim no path reaches.
pub const NO_WAY: Color = Color::Rgb(255, 40, 40);
/// Under the tiles your moves reach, so the path stands off the floor.
pub const LIT_BG: Color = Color::Rgb(0, 70, 90);
/// Under the things Space could open when it asks which way.
pub const WAY_BG: Color = Color::Rgb(0, 45, 55);

/// A background block on tiles, their glyphs kept: the look cursor, the which-way
/// candidates.
pub struct BlockOverlay<'a> {
    pub tiles: &'a [Pos],
    pub bg: Color,
}

impl Overlay for BlockOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for t in self.tiles {
            if let Some((sx, sy)) = tile_to_screen(area, origin, *t) {
                buf[(sx, sy)].set_bg(self.bg);
            }
        }
    }
}

pub struct PathOverlay<'a> {
    /// The tiles after your own, in walking order, ending on the aim.
    pub path: &'a [Pos],
    /// How many of them your moves cover.
    pub lit: usize,
    /// Where the cursor points; drawn red when no path reaches it.
    pub aim: Pos,
    pub reachable: bool,
}

impl Overlay for PathOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        if !self.reachable {
            if let Some((sx, sy)) = tile_to_screen(area, origin, self.aim) {
                buf[(sx, sy)]
                    .set_char('×')
                    .set_style(Style::default().fg(NO_WAY).add_modifier(Modifier::BOLD));
            }
            return;
        }
        let last = self.path.len().saturating_sub(1);
        for (i, p) in self.path.iter().enumerate() {
            let Some((sx, sy)) = tile_to_screen(area, origin, *p) else {
                continue;
            };
            let lit = i < self.lit;
            let style = if lit {
                Style::default()
                    .fg(CYAN)
                    .bg(LIT_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(CYAN).add_modifier(Modifier::DIM)
            };
            buf[(sx, sy)]
                .set_char(if i == last { '×' } else { '•' })
                .set_style(style);
        }
    }
}

/// The fire cursor on your act half: the line of fire in dim dots, the foe under
/// the cursor inverted (else the look cursor's block), the readout over it.
pub struct AimOverlay<'a> {
    /// The tiles between you and the target, both excluded.
    pub line: &'a [Pos],
    pub target: Pos,
    pub on_foe: bool,
    /// ` goon · Scrap nine · 9 tiles · 80% `, drawn two rows above the target.
    pub readout: Option<String>,
    /// The armed item's blast around the target with each tile's ring; empty when
    /// nothing is armed or it cannot land there.
    #[allow(dead_code)]
    pub footprint: &'a [(Pos, i32)],
    /// Your tile, when it is in the footprint.
    #[allow(dead_code)]
    pub you: Option<Pos>,
    /// The readout draws red: you stand in the blast.
    pub warn: bool,
}

/// The line of fire's dots.
pub const FIRE_LINE: Color = Color::Rgb(90, 90, 90);
/// Under the readout.
pub const READOUT_BG: Color = Color::Rgb(0, 20, 26);
/// The readout when you stand in the blast.
const WARN_FG: Color = Color::Rgb(255, 80, 80);
const WARN_BG: Color = Color::Rgb(26, 0, 0);

impl Overlay for AimOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for p in self.line {
            if let Some((sx, sy)) = tile_to_screen(area, origin, *p) {
                buf[(sx, sy)].set_char('·').set_fg(FIRE_LINE);
            }
        }
        if let Some((sx, sy)) = tile_to_screen(area, origin, self.target) {
            let cell = &mut buf[(sx, sy)];
            if self.on_foe {
                cell.set_fg(Color::Rgb(0, 0, 0)).set_bg(CYAN);
                cell.modifier.insert(Modifier::BOLD);
            } else {
                cell.set_bg(crate::look::LOOK_BG.to_color());
            }
        }
        let Some(text) = self.readout.as_ref() else {
            return;
        };
        let Some((tx, ty)) = tile_to_screen(area, origin, self.target) else {
            return;
        };
        let width = text.chars().count() as i32;
        if area.width < width as u16 || ty < area.y + 2 {
            return;
        }
        let x0 = (tx as i32 - width / 2)
            .max(area.x as i32)
            .min(area.right() as i32 - width);
        let y = ty - 2;
        // The name (the first word after the margin) in bold.
        let name_end = text
            .trim_start()
            .split('·')
            .next()
            .map_or(0, |n| n.len() + 1);
        for (i, ch) in text.chars().enumerate() {
            let x = (x0 + i as i32) as u16;
            let cell = &mut buf[(x, y)];
            let (fg, bg) = if self.warn {
                (WARN_FG, WARN_BG)
            } else {
                (CYAN, READOUT_BG)
            };
            cell.set_char(ch).set_fg(fg).set_bg(bg);
            cell.modifier.remove(Modifier::DIM);
            if i > 0 && i < name_end {
                cell.modifier.insert(Modifier::BOLD);
            } else {
                cell.modifier.remove(Modifier::BOLD);
            }
        }
    }
}

/// Picking a target for a program: every candidate marked, the picked one lit.
pub struct TargetOverlay<'a> {
    pub candidates: &'a [Pos],
    pub picked: Pos,
}

impl Overlay for TargetOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for c in self.candidates {
            if *c == self.picked {
                continue;
            }
            if let Some((sx, sy)) = tile_to_screen(area, origin, *c) {
                buf[(sx, sy)]
                    .set_char('◦')
                    .set_style(Style::default().fg(CYAN).bg(LIT_BG));
            }
        }
        if let Some((sx, sy)) = tile_to_screen(area, origin, self.picked) {
            buf[(sx, sy)].set_char('×').set_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(CYAN)
                    .add_modifier(Modifier::BOLD),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::map::MapRef;
    use citygen::{City, GenParams};

    fn draw(o: &PathOverlay) -> Buffer {
        let city = City::empty(GenParams::default());
        let map = MapRef::stratum_of(&city, citygen::StratumKind::Street);
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        let style = StylePack::load().unwrap();
        o.draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        buf
    }

    #[test]
    fn the_lit_prefix_is_bold_cyan_the_rest_dim_and_the_end_a_cross() {
        let path: Vec<Pos> = (1..=6).map(|x| Pos::new(x, 1)).collect();
        let buf = draw(&PathOverlay {
            path: &path,
            lit: 4,
            aim: Pos::new(6, 1),
            reachable: true,
        });
        for x in 1..=4 {
            let c = &buf[(x as u16, 1)];
            assert_eq!(c.symbol(), "•", "tile {x}");
            assert!(
                c.style().add_modifier.contains(Modifier::BOLD),
                "tile {x} lit"
            );
            assert_eq!(c.style().bg, Some(LIT_BG), "tile {x} stands off the floor");
        }
        let c = &buf[(5, 1)];
        assert_eq!(c.symbol(), "•");
        assert!(
            c.style().add_modifier.contains(Modifier::DIM),
            "beyond the moves: dim"
        );
        let end = &buf[(6, 1)];
        assert_eq!(end.symbol(), "×");
        assert!(
            end.style().add_modifier.contains(Modifier::DIM),
            "an end out of reach is dim"
        );
        assert_eq!(
            buf[(0, 1)].symbol(),
            " ",
            "your own tile is not on the path"
        );
    }

    #[test]
    fn block_overlay_keeps_the_glyph_and_sets_the_background() {
        let city = City::empty(GenParams::default());
        let map = MapRef::stratum_of(&city, citygen::StratumKind::Street);
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        buf[(4, 1)].set_char('@').set_fg(Color::White);
        let style = StylePack::load().unwrap();
        BlockOverlay {
            tiles: &[Pos::new(4, 1)],
            bg: WAY_BG,
        }
        .draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        let c = &buf[(4, 1)];
        assert_eq!(c.symbol(), "@");
        assert_eq!(c.fg, Color::White);
        assert_eq!(c.bg, WAY_BG);
        assert_ne!(buf[(5, 1)].bg, WAY_BG);
    }

    #[test]
    fn targets_are_marked_and_the_picked_one_lit() {
        let city = City::empty(GenParams::default());
        let map = MapRef::stratum_of(&city, citygen::StratumKind::Street);
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        let style = StylePack::load().unwrap();
        let cands = [Pos::new(2, 1), Pos::new(5, 1), Pos::new(8, 2)];
        TargetOverlay {
            candidates: &cands,
            picked: Pos::new(5, 1),
        }
        .draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        assert_eq!(buf[(2, 1)].symbol(), "◦");
        assert_eq!(buf[(8, 2)].symbol(), "◦");
        let picked = &buf[(5, 1)];
        assert_eq!(picked.symbol(), "×");
        assert_eq!(picked.style().bg, Some(CYAN));
    }

    #[test]
    fn an_end_within_the_moves_is_lit_and_no_path_draws_a_red_cross() {
        let path: Vec<Pos> = (1..=3).map(|x| Pos::new(x, 1)).collect();
        let buf = draw(&PathOverlay {
            path: &path,
            lit: 4,
            aim: Pos::new(3, 1),
            reachable: true,
        });
        let end = &buf[(3, 1)];
        assert_eq!(end.symbol(), "×");
        assert!(end.style().add_modifier.contains(Modifier::BOLD));
        assert!(!end.style().add_modifier.contains(Modifier::DIM));
        let buf = draw(&PathOverlay {
            path: &[],
            lit: 4,
            aim: Pos::new(7, 2),
            reachable: false,
        });
        let c = &buf[(7, 2)];
        assert_eq!(c.symbol(), "×");
        assert_eq!(
            c.style().fg,
            Some(Color::Rgb(255, 40, 40)),
            "no way there: red"
        );
    }
}
