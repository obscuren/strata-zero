//! The marks on venues' doors on the city map: a mission's flagged doors in its colour,
//! and the barred ones drawn shut.

use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use crate::ui::chrome::MAGENTA_RGB;
use crate::venues::DoorMark;
use citygen::{Grid, Pos, StratumKind};
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
}
