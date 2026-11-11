//! The marks on venues' doors on the city map: a mission's flagged doors in its colour,
//! and the barred ones drawn shut; and on a site's floor its terminals and gate switches.

use crate::render::map::MapRef;
use crate::render::netlook::{ICE_BLUE, TERMINAL};
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use crate::ui::chrome::MAGENTA_RGB;
use crate::venues::DoorMark;
use citygen::{Grid, NodeKind, Pos, StratumKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// A barred door: shut, and grey.
pub const BARRED: Rgb = Rgb(90, 90, 96);
pub const BARRED_GLYPH: char = '▮';

/// Each marked door on the drawn stratum: a flagged one keeps its glyph in the mission's
/// colour, a barred one is drawn shut. Hidden on unseen tiles, dimmed on remembered ones;
/// with no visibility grid (the free camera) every one shows.
pub struct DoorMarks<'a> {
    pub marks: &'a [(StratumKind, Pos, DoorMark)],
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for DoorMarks<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        let Some(k) = map.stratum() else {
            return;
        };
        for &(_, pos, mark) in self.marks.iter().filter(|m| m.0 == k) {
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
            let cell = &mut buf[(sx, sy)];
            match mark {
                DoorMark::Flagged => {
                    cell.set_fg(MAGENTA_RGB.scale(strength).to_color());
                }
                DoorMark::Barred => {
                    cell.set_char(BARRED_GLYPH)
                        .set_fg(BARRED.scale(strength).to_color());
                }
            }
        }
    }
}

/// How a way in on a site's floor is drawn on its wall tile: a terminal `◇`, a gate switch
/// `▪`, in their colours on the Net.
pub fn device_mark(kind: NodeKind) -> (char, Rgb) {
    match kind {
        NodeKind::GateSwitch => ('▪', ICE_BLUE),
        _ => ('◇', TERMINAL),
    }
}

/// A site's floor's terminals and gate switches, each on its wall tile (`device_mark`).
/// Hidden on unseen tiles, dimmed on remembered ones; with no visibility grid (the free
/// camera) every one shows.
pub struct FloorDevices<'a> {
    pub devices: &'a [(Pos, NodeKind)],
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for FloorDevices<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for &(pos, kind) in self.devices {
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
            let (glyph, colour) = device_mark(kind);
            buf[(sx, sy)]
                .set_char(glyph)
                .set_fg(colour.scale(strength).to_color());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};
    use ratatui::style::Color;

    #[test]
    fn a_flagged_door_takes_the_mark_and_a_barred_one_is_shut() {
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let map = MapRef::stratum_of(&city, StratumKind::Street);
        let (a, b) = (Pos::new(3, 2), Pos::new(5, 4));
        let marks = [
            (StratumKind::Street, a, DoorMark::Flagged),
            (StratumKind::Street, b, DoorMark::Barred),
            (StratumKind::Skyways, Pos::new(1, 1), DoorMark::Barred),
        ];
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        buf[(3, 2)].set_char('x');
        let overlay = DoorMarks {
            marks: &marks,
            vis: None,
        };
        let style = StylePack::load().unwrap();
        overlay.draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        assert_eq!(buf[(3, 2)].symbol(), "x");
        assert_eq!(buf[(3, 2)].fg, MAGENTA_RGB.to_color());
        assert_eq!(buf[(5, 4)].symbol(), "▮");
        assert_eq!(buf[(5, 4)].fg, Color::Rgb(90, 90, 96));
        // Only the drawn stratum's.
        assert_eq!(buf[(1, 1)].symbol(), " ");
    }

    #[test]
    fn a_floors_terminal_and_switch_show_on_their_wall_tiles_as_they_are_seen() {
        use citygen::Size;
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let map = MapRef::stratum_of(&city, StratumKind::Street);
        let (term, switch, hidden) = (Pos::new(2, 1), Pos::new(6, 3), Pos::new(8, 5));
        let devices = [
            (term, NodeKind::Terminal),
            (switch, NodeKind::GateSwitch),
            (hidden, NodeKind::Terminal),
        ];
        let mut vis = Grid::filled(Size { w: 20, h: 10 }, Visibility::Unseen);
        vis.set(term, Visibility::Visible);
        vis.set(switch, Visibility::Remembered);
        let area = Rect::new(0, 0, 20, 10);
        let style = StylePack::load().unwrap();
        let draw = |vis: Option<&Grid<Visibility>>| {
            let mut buf = Buffer::empty(area);
            let overlay = FloorDevices {
                devices: &devices,
                vis,
            };
            overlay.draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
            buf
        };
        let buf = draw(Some(&vis));
        assert_eq!(buf[(2, 1)].symbol(), "◇");
        assert_eq!(buf[(2, 1)].fg, TERMINAL.to_color());
        assert_eq!(buf[(6, 3)].symbol(), "▪");
        assert_eq!(buf[(6, 3)].fg, ICE_BLUE.scale(0.45).to_color(), "dimmed");
        assert_eq!(buf[(8, 5)].symbol(), " ", "unseen: hidden");
        // The free camera sees them all.
        let buf = draw(None);
        assert_eq!(buf[(8, 5)].symbol(), "◇");
        assert_eq!(buf[(6, 3)].fg, ICE_BLUE.to_color());
    }
}
