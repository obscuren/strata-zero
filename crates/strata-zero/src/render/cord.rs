//! The cord on the open Net: a line from you back to where the run came onto the grid,
//! dotted hops between its relays and home, and a trace walking home in magenta.
//! Its own module with one call from the map draw and a Settings row, so it can be
//! switched off or taken out whole.

use crate::net::CordView;
use crate::render::Rgb;
use crate::render::map::{MapRef, TileMap};
use crate::render::style::StylePack;
use crate::render::tileset::LIGHT;
use crate::render::viewport::{Overlay, tile_to_screen};
use citygen::pathing::bresenham;
use citygen::{Pos, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::collections::HashSet;

const CORD: Rgb = Rgb(230, 240, 245);
const TRACED: Rgb = Rgb(255, 0, 200);
const HOP: Rgb = Rgb(0, 229, 255);
const PORT: Rgb = Rgb(124, 255, 154);
/// A relay whose backdoor is gone.
const DEAD: Rgb = Rgb(96, 104, 116);

pub struct CordOverlay<'a> {
    pub view: &'a CordView,
    /// The Net the cord is walked over, past the city's edge and on the cells other
    /// areas own too: on a lane of any of them the lane's own glyph stays, tinted.
    pub walked: &'a dyn TileMap,
}

fn rgb(c: Color) -> Rgb {
    match c {
        Color::Rgb(r, g, b) => Rgb(r, g, b),
        _ => Rgb(0, 0, 0),
    }
}

fn set(buf: &mut Buffer, at: (u16, u16), ch: Option<char>, fg: Rgb, glow: Option<(Rgb, f32)>) {
    let cell = &mut buf[at];
    if let Some(ch) = ch {
        cell.set_char(ch);
    }
    if let Some((c, f)) = glow {
        let bg = rgb(cell.bg).mix(c, f);
        cell.set_bg(bg.to_color());
    }
    cell.set_fg(fg.to_color());
    cell.modifier.insert(Modifier::BOLD);
}

impl Overlay for CordOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        let v = self.view;
        let (colour, glow) = if v.traced_walk {
            (TRACED, 0.22)
        } else {
            (CORD, 0.14)
        };
        let on: HashSet<Pos> = v.walk.iter().copied().collect();
        for p in &v.walk {
            let Some(at) = tile_to_screen(area, origin, *p) else {
                continue;
            };
            let lane = self
                .walked
                .tile(*p)
                .is_some_and(|t| matches!(t.terrain, Terrain::NetLane | Terrain::NetGate));
            let ch = (!lane).then(|| {
                let m = [(0, -1, 1), (1, 0, 2), (0, 1, 4), (-1, 0, 8)]
                    .into_iter()
                    .filter(|(dx, dy, _)| on.contains(&p.offset(*dx, *dy)))
                    .map(|(_, _, b)| b)
                    .sum::<usize>();
                LIGHT[m]
            });
            set(buf, at, ch, colour, Some((colour, glow)));
        }
        for (i, (a, b)) in v.hops.iter().enumerate() {
            let fg = if v.traced_hops.get(i).copied().unwrap_or(false) {
                TRACED
            } else {
                HOP
            };
            for p in bresenham(*a, *b).into_iter().filter(|p| p != a && p != b) {
                if let Some(at) = tile_to_screen(area, origin, p) {
                    set(buf, at, Some('·'), fg, None);
                }
            }
        }
        for (r, live) in &v.relays {
            if let Some(at) = tile_to_screen(area, origin, *r) {
                set(buf, at, Some('▣'), if *live { HOP } else { DEAD }, None);
            }
        }
        if let Some(at) = v.port.and_then(|p| tile_to_screen(area, origin, p)) {
            set(buf, at, Some('▣'), PORT, None);
        }
        if let Some(at) = v.at.and_then(|p| tile_to_screen(area, origin, p)) {
            set(buf, at, None, Rgb(255, 255, 255), Some((TRACED, 0.75)));
        }
    }
}
