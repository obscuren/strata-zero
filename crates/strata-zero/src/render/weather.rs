//! Rain and fog overlays, pure functions of tick and screen position.

use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, screen_to_tile};
use citygen::rng::hash64;
use citygen::{City, StratumKind, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Rain on the current stratum and fog where the style asks for it.
pub struct Weather;

/// Whether a raindrop occupies screen cell `(sx, sy)` at `tick`. `div` is the density
/// divisor (0 = no rain). Drops fall one cell per tick and drift right every four ticks.
pub fn drop_at(sx: u16, sy: u16, tick: u64, div: u64) -> bool {
    if div == 0 {
        return false;
    }
    let drift = (tick / 4) as i64;
    let column = sx as i64 - drift;
    let seed_row = sy as i64 - tick as i64;
    hash64(column as u64, seed_row as u64).is_multiple_of(div)
}

fn density_div(level: u8) -> u64 {
    match level {
        0 => 0,
        1 => 40,
        2 => 20,
        _ => 10,
    }
}

impl Overlay for Weather {
    fn draw(
        &self,
        city: &City,
        stratum: StratumKind,
        origin: citygen::Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        style: &StylePack,
    ) {
        let ss = style.stratum(stratum);
        let div = density_div(ss.rain);
        let Some(st) = city.stratum(stratum) else {
            return;
        };
        let rain = &style.glyphs.rain;
        for sy in area.y..area.bottom() {
            for sx in area.x..area.right() {
                let p = screen_to_tile(area, origin, (sx, sy));
                let Some(t) = st.tiles.get(p) else { continue };
                let outdoors = !matches!(
                    t.terrain,
                    Terrain::Wall
                        | Terrain::Floor
                        | Terrain::Door
                        | Terrain::Window
                        | Terrain::Stair
                        | Terrain::Lift
                );
                if div > 0 && outdoors && drop_at(sx, sy, tick, div) {
                    let g =
                        rain[(hash64(sx as u64, sy as u64 + tick) % rain.len() as u64) as usize];
                    buf[(sx, sy)].set_char(g).set_fg(Color::Rgb(90, 115, 160));
                    continue;
                }
                if ss.fog {
                    let light = st.light.get(p).copied().unwrap_or(0);
                    if light < 40 && hash64(sx as u64 * 31 + sy as u64, tick / 8).is_multiple_of(6)
                    {
                        buf[(sx, sy)].set_char('░').set_fg(Color::Rgb(40, 62, 46));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_falls_one_cell_per_tick() {
        // A drop at (x, y) on tick t is at (x + drift, y + 1) on tick t + 1 when drift is 0.
        for t in 0..50u64 {
            // Drift happens when (t + 1) crosses a multiple of four.
            if !(t + 1).is_multiple_of(4) {
                assert_eq!(drop_at(10, 5, t, 10), drop_at(10, 6, t + 1, 10));
            }
        }
        assert!((0..400).any(|t| drop_at(3, 3, t, 10)));
        assert!(!(0..400).any(|t| drop_at(3, 3, t, 0)));
    }
}
