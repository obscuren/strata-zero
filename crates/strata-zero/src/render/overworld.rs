//! The overworld's atlas look: one cell a tile, the ground as glyphs, highways as double
//! lines, rivers as thin blue ones and dry washes as faint sandy ones, cities shaded by
//! density in their districts' colours.
//! `atlas_cell` is what the overworld screen and `strata world`'s sheet both draw.

use crate::render::style::StylePack;
use crate::render::{Cell, Rgb};
use citygen::Pos;
use citygen::overworld::{Ground, Overworld, PoiKind, Water};
use citygen::rng::hash64;

/// The atlas's background.
pub const ATLAS_BG: Rgb = Rgb(4, 6, 10);

/// Box glyphs by link mask (N 1, E 2, S 4, W 8).
const DOUBLE: [char; 16] = [
    '═', '║', '═', '╚', '║', '║', '╔', '╠', '═', '╝', '═', '╩', '╗', '╣', '╦', '╬',
];
const ROUND: [char; 16] = [
    '─', '│', '─', '╰', '│', '│', '╭', '├', '─', '╯', '─', '┴', '╮', '┤', '┬', '┼',
];

const DASHED: [char; 16] = [
    '┄', '┆', '┄', '╰', '┆', '┆', '╭', '├', '┄', '╯', '┄', '┴', '╮', '┤', '┬', '┼',
];

/// A POI's glyph and colour.
pub fn poi_glyph(kind: PoiKind) -> (char, Rgb) {
    match kind {
        PoiKind::TruckStop => ('⌂', Rgb(255, 176, 0)),
        PoiKind::Checkpoint => ('╪', Rgb(255, 40, 72)),
        PoiKind::Relay => ('⬡', Rgb(0, 255, 200)),
        PoiKind::Camp => ('△', Rgb(255, 128, 64)),
        PoiKind::Wreck => ('×', Rgb(255, 40, 72)),
        PoiKind::Arcology => ('▣', Rgb(168, 144, 216)),
        PoiKind::Solar => ('▦', Rgb(122, 176, 224)),
        PoiKind::WaterPlant => ('◎', Rgb(0, 229, 255)),
        PoiKind::TestRange => ('◈', Rgb(255, 0, 200)),
        PoiKind::Rig => ('◆', Rgb(200, 160, 80)),
    }
}

fn cell(ch: char, fg: Rgb, bg: Rgb) -> Cell {
    Cell {
        ch,
        fg,
        bg,
        bold: false,
        dim: false,
    }
}

/// The tile's stable roll in 0..1.
fn roll(world: &Overworld, p: Pos) -> f32 {
    let packed = ((p.x as u32 as u64) << 32) | p.y as u32 as u64;
    (hash64(hash64(world.seed, 0xA71A5), packed) >> 40) as f32 / (1u64 << 24) as f32
}

/// Tile `p` of `world` in the atlas look.
pub fn atlas_cell(world: &Overworld, p: Pos, style: &StylePack) -> Cell {
    let Some(t) = world.tile(p) else {
        return cell(' ', ATLAS_BG, ATLAS_BG);
    };
    let mut c = ground_cell(world, p, style);
    if let Some(poi) = world.poi_at(p) {
        let (ch, fg) = poi_glyph(poi.kind);
        c.ch = ch;
        c.fg = fg;
        c.bold = true;
        return c;
    }
    if t.road != 0 {
        let in_city = t.urban.is_some();
        let bg = if in_city {
            Rgb(26, 20, 8)
        } else if t.water != Water::None {
            Rgb(10, 26, 46)
        } else {
            ATLAS_BG
        };
        let fg = if in_city {
            Rgb(240, 200, 112)
        } else {
            Rgb(200, 160, 80)
        };
        let mut c = cell(DOUBLE[(t.road & 15) as usize], fg, bg);
        c.bold = in_city;
        return c;
    }
    c
}

/// Everything under the roads and POIs: river, city, water, ground.
fn ground_cell(world: &Overworld, p: Pos, style: &StylePack) -> Cell {
    let t = world.tile(p).expect("in bounds");
    let r = roll(world, p);
    if t.water == Water::River {
        return cell(
            ROUND[(t.river & 15) as usize],
            Rgb(58, 138, 216),
            Rgb(6, 18, 31),
        );
    }
    if t.wash != 0 && t.urban.is_none() {
        // A dry wash: a faint dashed sandy line, so it never reads as a road.
        return cell(DASHED[(t.wash & 15) as usize], Rgb(88, 76, 56), ATLAS_BG);
    }
    if let Some(u) = t.urban {
        let v = u.density as f32 / 255.0;
        let a = style.theme_or_neutral(Some(u.theme)).accent;
        let ch = if v > 0.75 {
            if r < 0.45 { '█' } else { '▓' }
        } else if v > 0.45 {
            if r < 0.4 { '▓' } else { '▒' }
        } else if v > 0.22 {
            if r < 0.35 { '▒' } else { '░' }
        } else if r < 0.45 {
            '░'
        } else if r < 0.75 {
            '▫'
        } else {
            '·'
        };
        let fg = Rgb(42, 46, 56)
            .mix(Rgb(154, 160, 174), v)
            .mix(a, 0.22 + 0.38 * v);
        let bg = ATLAS_BG.mix(a, 0.03 + 0.06 * v);
        if v > 0.45 && r > 0.92 {
            return cell('▪', a, bg);
        }
        return cell(ch, fg, bg);
    }
    match t.water {
        Water::Lake => return cell('≈', Rgb(42, 90, 138), Rgb(10, 26, 46)),
        Water::Sea => {
            let shore = p
                .neighbors8()
                .any(|q| world.tile(q).is_some_and(|n| n.water != Water::Sea));
            return if shore {
                cell('~', Rgb(42, 90, 138), Rgb(10, 26, 46))
            } else {
                cell(
                    if r < 0.12 { '~' } else { ' ' },
                    Rgb(26, 58, 92),
                    Rgb(5, 12, 24),
                )
            };
        }
        _ => {}
    }
    let bg = ATLAS_BG;
    match t.ground {
        Ground::Peak => cell('▲', Rgb(200, 204, 216), bg),
        Ground::Mountains => cell('▲', Rgb(120, 124, 138), bg),
        Ground::Hills => cell(if r < 0.7 { '∩' } else { '·' }, Rgb(122, 102, 72), bg),
        Ground::Spill => {
            let ch = if r < 0.18 {
                '≈'
            } else if r < 0.6 {
                '·'
            } else {
                ' '
            };
            let fg = if r > 0.9 {
                Rgb(154, 255, 90)
            } else {
                Rgb(74, 138, 34)
            };
            cell(ch, fg, Rgb(6, 12, 4))
        }
        Ground::Salt => cell(if r < 0.5 { '░' } else { '·' }, Rgb(90, 88, 82), bg),
        Ground::Fields => cell('▬', Rgb(58, 106, 48), bg),
        Ground::Scrub => {
            let ch = if r < 0.35 {
                '"'
            } else if r < 0.6 {
                ','
            } else {
                ' '
            };
            cell(ch, Rgb(74, 90, 52), bg)
        }
        Ground::Flats => {
            let ch = if r < 0.3 {
                '·'
            } else if r < 0.45 {
                '.'
            } else {
                ' '
            };
            cell(ch, Rgb(74, 66, 52), bg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::StylePack;
    use citygen::overworld::{Overworld, WorldParams};

    #[test]
    fn every_kind_of_tile_draws_one_cell() {
        let w = Overworld::generate(WorldParams {
            seed: 3,
            ..Default::default()
        })
        .unwrap();
        let style = StylePack::load().unwrap();
        let mut kinds = std::collections::BTreeSet::new();
        for (p, t) in w.tiles.iter() {
            let c = atlas_cell(&w, p, &style);
            assert_eq!(
                unicode_width::UnicodeWidthChar::width(c.ch),
                Some(1),
                "{:?} {p:?}",
                c.ch
            );
            kinds.insert(format!(
                "{:?}{:?}{}{}",
                t.water,
                t.ground,
                t.road != 0,
                t.urban.is_some()
            ));
        }
        assert!(
            kinds.len() > 12,
            "a world has many kinds of tile: {kinds:?}"
        );
        for poi in &w.pois {
            assert_eq!(atlas_cell(&w, poi.at, &style).ch, poi_glyph(poi.kind).0);
        }
    }
}
