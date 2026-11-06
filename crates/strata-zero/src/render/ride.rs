//! A ride on the open Net, drawn over the lane: the streak behind you fading, the
//! stretch ahead lit and its stop marked.

use crate::render::Rgb;
use crate::render::map::MapRef;
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ride::Ride;
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

const WHITE: Rgb = Rgb(255, 255, 255);
/// How bright the streak is, newest first.
const STREAK: [f32; 4] = [0.5, 0.35, 0.2, 0.1];

pub struct RideOverlay<'a> {
    pub ride: &'a Ride,
    /// Where you are now.
    pub me: Pos,
}

fn rgb(c: Color) -> Rgb {
    match c {
        Color::Rgb(r, g, b) => Rgb(r, g, b),
        _ => Rgb(0, 0, 0),
    }
}

/// Tints a cell's background toward `colour`; with `fg`, sets its glyph's colour bold.
fn tint(buf: &mut Buffer, at: (u16, u16), colour: Rgb, f: f32, fg: Option<Rgb>) {
    let cell = &mut buf[at];
    let bg = rgb(cell.bg).mix(colour, f);
    cell.set_bg(bg.to_color());
    if let Some(fg) = fg {
        cell.set_fg(fg.to_color());
        cell.modifier.insert(Modifier::BOLD);
    }
}

impl Overlay for RideOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        // Behind: each tile toward the one after it, newest first.
        let mut toward = self.me;
        for (p, f) in self.ride.done.iter().rev().zip(STREAK) {
            if let Some(at) = tile_to_screen(area, origin, *p) {
                let ch = if p.y == toward.y { '━' } else { '┃' };
                buf[at].set_char(ch);
                tint(buf, at, WHITE, f, Some(SIGNAL.mix(WHITE, 0.4 + f)));
            }
            toward = *p;
        }
        // Ahead: the stretch lit, the stop marked.
        let last = self.ride.path.len().saturating_sub(1);
        for (i, p) in self.ride.path.iter().enumerate() {
            if let Some(at) = tile_to_screen(area, origin, *p) {
                let f = if i == last { 0.55 } else { 0.18 };
                tint(buf, at, SIGNAL, f, Some(WHITE));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset, StratumKind};

    #[test]
    fn a_ride_draws_its_streak_and_its_stop() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let map = MapRef::stratum_of(&c, StratumKind::Net);
        let ride = Ride {
            path: vec![Pos::new(6, 2), Pos::new(7, 2)],
            done: vec![Pos::new(3, 2), Pos::new(4, 2)],
            next_at: 0,
            runs_on: false,
        };
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = Buffer::empty(area);
        let style = StylePack::load().unwrap();
        RideOverlay {
            ride: &ride,
            me: Pos::new(5, 2),
        }
        .draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        assert_eq!(buf[(4, 2)].symbol(), "━");
        assert_eq!(buf[(3, 2)].symbol(), "━");
        let stop = rgb(buf[(7, 2)].bg);
        let ahead = rgb(buf[(6, 2)].bg);
        assert!(stop.1 > ahead.1, "the stop is brighter than the way to it");
    }
}
