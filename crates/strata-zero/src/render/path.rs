//! The aim: the path you mean to walk, lit as far as your moves carry.

use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::CYAN;
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::ops::Range;

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
    pub footprint: &'a [(Pos, i32)],
    /// Your tile, when it is in the footprint.
    pub you: Option<Pos>,
    /// The readout draws red: you stand in the blast.
    pub warn: bool,
    /// The readout's characters (by index) drawn in the mark's magenta, bold: the
    /// chance on a marked foe.
    pub lit: Option<Range<usize>>,
}

/// The line of fire's dots.
pub const FIRE_LINE: Color = Color::Rgb(90, 90, 90);
/// Under the readout.
pub const READOUT_BG: Color = Color::Rgb(0, 20, 26);
/// The chance on a marked foe, in the readout.
pub const MARK_LIT: Color = Color::Rgb(255, 0, 200);
/// The readout when you stand in the blast.
pub const WARN_FG: Color = Color::Rgb(255, 80, 80);
pub const WARN_BG: Color = Color::Rgb(26, 0, 0);
/// Under the footprint's rings, the centre out; ring 3 and beyond take the last.
pub const RING_BG: [Color; 3] = [
    Color::Rgb(120, 30, 30),
    Color::Rgb(80, 22, 22),
    Color::Rgb(48, 16, 16),
];
/// Under your own tile when you stand in the blast.
pub const YOU_IN_IT_BG: Color = Color::Rgb(170, 30, 30);
/// Ticks in each half of a live blast's blink (30 ticks a second).
pub const LIVE_BLINK: u64 = 4;
/// Under a live blast's footprint, dimmer than the aim's.
pub const LIVE_RING_BG: [Color; 3] = [
    Color::Rgb(60, 18, 18),
    Color::Rgb(44, 14, 14),
    Color::Rgb(30, 10, 10),
];

/// ` text ` on the readout ground, bold over the first word, two rows above `at`,
/// clamped inside the area; the screen cell of its first character. Nothing when
/// it does not fit.
fn readout(
    buf: &mut Buffer,
    area: Rect,
    at: (u16, u16),
    text: &str,
    fg: Color,
    bg: Color,
    bold_name: bool,
) -> Option<(u16, u16)> {
    let width = text.chars().count() as i32;
    if area.width < width as u16 || at.1 < area.y + 2 {
        return None;
    }
    let x0 = (at.0 as i32 - width / 2)
        .max(area.x as i32)
        .min(area.right() as i32 - width);
    let y = at.1 - 2;
    // The name (the first word after the margin) in bold.
    let name_end = if bold_name {
        text.trim_start()
            .split('·')
            .next()
            .map_or(0, |n| n.len() + 1)
    } else {
        0
    };
    for (i, ch) in text.chars().enumerate() {
        let x = (x0 + i as i32) as u16;
        let cell = &mut buf[(x, y)];
        cell.set_char(ch).set_fg(fg).set_bg(bg);
        cell.modifier.remove(Modifier::DIM);
        if i > 0 && i < name_end {
            cell.modifier.insert(Modifier::BOLD);
        } else {
            cell.modifier.remove(Modifier::BOLD);
        }
    }
    Some((x0 as u16, y))
}

/// Shade each footprint tile by its ring, glyphs kept.
fn shade(buf: &mut Buffer, area: Rect, origin: Pos, tiles: &[(Pos, i32)], rings: &[Color; 3]) {
    for (p, ring) in tiles {
        if let Some((sx, sy)) = tile_to_screen(area, origin, *p) {
            buf[(sx, sy)].set_bg(rings[(*ring).clamp(0, 2) as usize]);
        }
    }
}

/// The ground under blasts lying live: their footprints shaded, glyphs kept. Drawn
/// before the people, so whoever stands in one is drawn over the shading.
pub struct LiveGround<'a> {
    /// `blast::live_footprints` for this place.
    pub footprints: &'a [(Pos, i32)],
}

impl Overlay for LiveGround<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        shade(buf, area, origin, self.footprints, &LIVE_RING_BG);
    }
}

/// Blasts lying live on the map: the thing blinking where no one stands on it, its
/// fuse over it. The shading under them is `LiveGround`'s.
pub struct LiveOverlay<'a> {
    pub live: &'a [crate::blast::Live],
    /// Tiles a standing body is drawn on: its glyph is kept over the thing.
    pub standing: &'a [Pos],
}

impl Overlay for LiveOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        // In the air it is the throw's picture; the overlay takes over at the landing.
        for l in self
            .live
            .iter()
            .filter(|l| l.place == map.place() && l.landed(tick))
        {
            let Some(at) = tile_to_screen(area, origin, l.pos) else {
                continue;
            };
            let fg = if (tick / LIVE_BLINK).is_multiple_of(2) {
                Color::Rgb(190, 190, 140)
            } else {
                Color::Rgb(255, 80, 80)
            };
            if !self.standing.contains(&l.pos) {
                buf[at]
                    .set_char('o')
                    .set_style(Style::default().fg(fg).add_modifier(Modifier::BOLD));
            }
            let text = format!(" fuse {} ", l.turns);
            readout(buf, area, at, &text, CYAN, READOUT_BG, false);
        }
    }
}

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
        shade(buf, area, origin, self.footprint, &RING_BG);
        if let Some((sx, sy)) = self.you.and_then(|y| tile_to_screen(area, origin, y)) {
            buf[(sx, sy)].set_bg(YOU_IN_IT_BG);
        }
        for p in self.line {
            if let Some((sx, sy)) = tile_to_screen(area, origin, *p) {
                buf[(sx, sy)].set_char('·').set_fg(FIRE_LINE);
            }
        }
        if let Some((sx, sy)) = tile_to_screen(area, origin, self.target) {
            let cell = &mut buf[(sx, sy)];
            if !self.on_foe {
                cell.set_char('×');
            }
            cell.set_fg(Color::Rgb(0, 0, 0)).set_bg(CYAN);
            cell.modifier.insert(Modifier::BOLD);
        }
        let Some(text) = self.readout.as_ref() else {
            return;
        };
        let Some(at) = tile_to_screen(area, origin, self.target) else {
            return;
        };
        let (fg, bg) = if self.warn {
            (WARN_FG, WARN_BG)
        } else {
            (CYAN, READOUT_BG)
        };
        let Some((x0, y)) = readout(buf, area, at, text, fg, bg, true) else {
            return;
        };
        let width = text.chars().count();
        for i in self.lit.clone().unwrap_or_default().filter(|i| *i < width) {
            let cell = &mut buf[(x0 + i as u16, y)];
            cell.set_fg(MARK_LIT);
            cell.modifier.insert(Modifier::BOLD);
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

    fn aim_buf(o: &AimOverlay, buf: &mut Buffer, area: Rect) {
        let city = City::empty(GenParams::default());
        let map = MapRef::stratum_of(&city, citygen::StratumKind::Street);
        let style = StylePack::load().unwrap();
        o.draw(&map, Pos::new(0, 0), area, 0, buf, &style);
    }

    fn aim<'a>(
        target: Pos,
        on_foe: bool,
        footprint: &'a [(Pos, i32)],
        you: Option<Pos>,
    ) -> AimOverlay<'a> {
        AimOverlay {
            line: &[],
            target,
            on_foe,
            readout: None,
            footprint,
            you,
            warn: false,
            lit: None,
        }
    }

    #[test]
    fn the_cursor_off_a_foe_is_a_cyan_block_with_a_cross() {
        let area = Rect::new(0, 0, 12, 6);
        let mut buf = Buffer::empty(area);
        buf[(5, 3)].set_char('.');
        aim_buf(&aim(Pos::new(5, 3), false, &[], None), &mut buf, area);
        let c = &buf[(5, 3)];
        assert_eq!(c.symbol(), "×");
        assert_eq!(c.fg, Color::Rgb(0, 0, 0));
        assert_eq!(c.bg, CYAN);
        assert!(c.modifier.contains(Modifier::BOLD));
        // On a foe: the glyph stays, inverted.
        let mut buf = Buffer::empty(area);
        buf[(5, 3)].set_char('g');
        aim_buf(&aim(Pos::new(5, 3), true, &[], None), &mut buf, area);
        let c = &buf[(5, 3)];
        assert_eq!(c.symbol(), "g");
        assert_eq!(c.bg, CYAN);
    }

    #[test]
    fn the_footprint_shades_by_ring_and_lights_your_tile() {
        let area = Rect::new(0, 0, 12, 6);
        let mut buf = Buffer::empty(area);
        buf[(4, 3)].set_char('#');
        let fp = [
            (Pos::new(5, 3), 0),
            (Pos::new(4, 3), 1),
            (Pos::new(3, 3), 2),
            (Pos::new(2, 3), 3),
            (Pos::new(8, 3), 1),
        ];
        let o = AimOverlay {
            line: &[Pos::new(4, 3)],
            ..aim(Pos::new(5, 3), false, &fp, Some(Pos::new(8, 3)))
        };
        aim_buf(&o, &mut buf, area);
        assert_eq!(buf[(5, 3)].bg, CYAN, "the cursor over the centre");
        let dots = &buf[(4, 3)];
        assert_eq!(dots.symbol(), "·", "the line's dots over the shading");
        assert_eq!(dots.bg, RING_BG[1]);
        assert_eq!(buf[(3, 3)].bg, RING_BG[2]);
        assert_eq!(buf[(2, 3)].bg, RING_BG[2], "ring 3 takes the last");
        assert_eq!(buf[(8, 3)].bg, YOU_IN_IT_BG);
    }

    #[test]
    fn a_warned_readout_is_red() {
        let area = Rect::new(0, 0, 20, 6);
        let mut buf = Buffer::empty(area);
        let o = AimOverlay {
            readout: Some(" frag ".to_string()),
            warn: true,
            ..aim(Pos::new(10, 4), false, &[], None)
        };
        aim_buf(&o, &mut buf, area);
        let c = &buf[(8, 2)];
        assert_eq!(c.symbol(), "f");
        assert_eq!(c.fg, WARN_FG);
        assert_eq!(c.bg, WARN_BG);
    }

    #[test]
    fn a_lit_span_of_the_readout_is_magenta_bold() {
        let area = Rect::new(0, 0, 30, 6);
        let mut buf = Buffer::empty(area);
        // ` goon · 80% ` is 12 wide: centred on x 10 it starts at x 4.
        let o = AimOverlay {
            readout: Some(" goon · 80% ".to_string()),
            lit: Some(8..11),
            ..aim(Pos::new(10, 4), true, &[], None)
        };
        aim_buf(&o, &mut buf, area);
        let row: String = (4..16).map(|x| buf[(x, 2)].symbol().to_string()).collect();
        assert_eq!(row, " goon · 80% ");
        for x in 12..15 {
            let c = &buf[(x, 2)];
            assert_eq!(c.fg, MARK_LIT, "{} at {x}", c.symbol());
            assert!(c.modifier.contains(Modifier::BOLD));
            assert_eq!(c.bg, READOUT_BG);
        }
        let space = &buf[(15, 2)];
        assert_eq!(space.fg, CYAN);
        assert!(!space.modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(10, 2)].fg, CYAN, "the dot stays the readout's");
    }

    #[test]
    fn a_live_blast_blinks_and_shades() {
        use crate::blast::Live;
        let city = City::empty(GenParams::default());
        let place = citygen::Place::Stratum(citygen::StratumKind::Street);
        let map = MapRef::stratum_of(&city, citygen::StratumKind::Street);
        let style = StylePack::load().unwrap();
        let area = Rect::new(0, 0, 20, 8);
        let live = [Live {
            place,
            pos: Pos::new(10, 5),
            blast: game::Blast {
                radius: 1,
                damage: 5,
                stun: 0,
                fuse: 1,
                chrome: false,
                harm: game::Harm::Kinetic,
            },
            fx: Vec::new(),
            name: "frag".to_string(),
            turns: 1,
            lands: 0,
            wakes: None,
        }];
        let fp = [
            (Pos::new(10, 5), 0),
            (Pos::new(11, 5), 1),
            (Pos::new(12, 5), 3),
        ];
        let ground = LiveGround { footprints: &fp };
        let o = LiveOverlay {
            live: &live,
            standing: &[],
        };
        let mut even = Buffer::empty(area);
        ground.draw(&map, Pos::new(0, 0), area, 0, &mut even, &style);
        o.draw(&map, Pos::new(0, 0), area, 0, &mut even, &style);
        let c = &even[(10, 5)];
        assert_eq!(c.symbol(), "o");
        assert_eq!(c.fg, Color::Rgb(190, 190, 140));
        assert!(c.modifier.contains(Modifier::BOLD));
        assert_eq!(c.bg, LIVE_RING_BG[0]);
        assert_eq!(even[(11, 5)].bg, LIVE_RING_BG[1]);
        assert_eq!(even[(12, 5)].bg, LIVE_RING_BG[2]);
        let f = &even[(7, 3)];
        assert_eq!(f.symbol(), "f");
        assert_eq!((f.fg, f.bg), (CYAN, READOUT_BG));
        let row: String = (6..14).map(|x| even[(x, 3)].symbol().to_string()).collect();
        assert_eq!(row, " fuse 1 ");
        let mut odd = Buffer::empty(area);
        o.draw(&map, Pos::new(0, 0), area, 4, &mut odd, &style);
        assert_eq!(odd[(10, 5)].fg, Color::Rgb(255, 80, 80));
        // Someone standing on it keeps their glyph; the fuse still reads over them.
        let mut held = Buffer::empty(area);
        held[(10, 5)].set_char('@').set_fg(Color::Rgb(255, 80, 80));
        LiveOverlay {
            live: &live,
            standing: &[Pos::new(10, 5)],
        }
        .draw(&map, Pos::new(0, 0), area, 0, &mut held, &style);
        assert_eq!(held[(10, 5)].symbol(), "@");
        assert_eq!(held[(7, 3)].symbol(), "f");
        // Still in the air: nothing of it yet.
        let flying = [Live {
            lands: 10,
            ..live[0].clone()
        }];
        let mut air = Buffer::empty(area);
        LiveOverlay {
            live: &flying,
            standing: &[],
        }
        .draw(&map, Pos::new(0, 0), area, 9, &mut air, &style);
        assert_eq!(air, Buffer::empty(area));
        LiveOverlay {
            live: &flying,
            standing: &[],
        }
        .draw(&map, Pos::new(0, 0), area, 10, &mut air, &style);
        assert_eq!(air[(10, 5)].symbol(), "o", "down on its landing tick");
    }
}
