//! The tileset: one pure function from a tile and its context to a styled cell.

use crate::anim::effects::{flicker, pulse};
use crate::render::lighting::lit;
use crate::render::style::StylePack;
use crate::render::{Cell, ConnDir, RenderCtx, Rgb, Visibility};
use citygen::{BuildingKind, ConnectorKind, Feature, StratumKind, Terrain, Tile};

const LIGHT: [char; 16] = [
    '─', '│', '─', '└', '│', '│', '┌', '├', '─', '┘', '─', '┴', '┐', '┤', '┬', '┼',
];
const HEAVY: [char; 16] = [
    '━', '┃', '━', '┗', '┃', '┃', '┏', '┣', '━', '┛', '━', '┻', '┓', '┫', '┳', '╋',
];
const DOUBLE: [char; 16] = [
    '═', '║', '═', '╚', '║', '║', '╔', '╠', '═', '╝', '═', '╩', '╗', '╣', '╦', '╬',
];

pub const MASK_N: u8 = 1;
pub const MASK_E: u8 = 2;
pub const MASK_S: u8 = 4;
pub const MASK_W: u8 = 8;

/// Whether two adjacent terrains draw as one continuous structure (walls, walkways).
pub fn connects(a: Terrain, b: Terrain) -> bool {
    use Terrain::*;
    let wallish = |t| matches!(t, Wall | Door | Window);
    let spanish = |t| matches!(t, Walkway | Bridge);
    (wallish(a) && wallish(b))
        || (spanish(a) && (spanish(b) || matches!(b, Door | Floor | Stair | Lift | Ladder)))
}

fn wall_set(kind: Option<BuildingKind>) -> &'static [char; 16] {
    match kind {
        Some(k) if k.is_corp() => &DOUBLE,
        Some(
            BuildingKind::Apartment
            | BuildingKind::Hotel
            | BuildingKind::Factory
            | BuildingKind::Warehouse,
        ) => &HEAVY,
        _ => &LIGHT,
    }
}

/// The glyph for a tile, before colour. Pure in all of its inputs.
#[allow(clippy::too_many_arguments)]
pub fn glyph_for(
    terrain: Terrain,
    feature: Option<Feature>,
    connector: Option<ConnDir>,
    kind: Option<BuildingKind>,
    variant: u8,
    tick: u64,
    mask: u8,
    style: &StylePack,
) -> char {
    let g = &style.glyphs;
    let pick = |set: &[char], i: u64| set[(i % set.len() as u64) as usize];
    if let Some(f) = feature {
        return match f {
            Feature::Neon(_) => '▪',
            Feature::HoloAd => pick(&g.holo, tick / 8 + variant as u64),
            Feature::Lamp => '●',
            Feature::Vent => {
                if (tick / 4 + variant as u64) % 10 < 3 {
                    pick(&g.steam, tick / 4 + variant as u64)
                } else {
                    '☰'
                }
            }
            Feature::Puddle => '~',
            Feature::Trash => '%',
            Feature::Barrier => '▬',
            Feature::Stall => '▤',
            Feature::Sign(c) => c,
            Feature::Terminal => '▣',
            Feature::Camera => {
                if (tick / 15).is_multiple_of(4) {
                    '○'
                } else {
                    '◉'
                }
            }
            Feature::AcUnit => '▩',
            Feature::Helipad => 'H',
            Feature::Pipe => '╌',
            Feature::Cable => '╌',
        };
    }
    let horizontal = mask & (MASK_E | MASK_W) != 0;
    match terrain {
        Terrain::Void => ' ',
        Terrain::Water => pick(&g.water, tick / 6 + variant as u64),
        Terrain::Sewage => '≈',
        Terrain::Road => {
            if variant.is_multiple_of(16) {
                '·'
            } else {
                ' '
            }
        }
        Terrain::Sidewalk => '░',
        Terrain::Alley => ' ',
        Terrain::Plaza => {
            if variant.is_multiple_of(3) {
                '▫'
            } else {
                ' '
            }
        }
        Terrain::Floor => {
            if variant.is_multiple_of(23) {
                '·'
            } else {
                ' '
            }
        }
        Terrain::Wall => wall_set(kind)[(mask & 15) as usize],
        Terrain::Door => '▯',
        Terrain::Window => '▫',
        Terrain::Rooftop => pick(&g.mass, variant as u64),
        Terrain::Walkway => {
            if horizontal {
                '═'
            } else {
                '║'
            }
        }
        Terrain::Bridge => {
            if horizontal {
                '╍'
            } else {
                '╏'
            }
        }
        Terrain::Tunnel => {
            if variant.is_multiple_of(19) {
                '.'
            } else {
                ' '
            }
        }
        Terrain::Rail => {
            if (tick / 20).is_multiple_of(2) {
                '╬'
            } else {
                '╫'
            }
        }
        Terrain::Stair => match connector {
            Some(ConnDir {
                up: true,
                down: true,
                ..
            }) => '⇅',
            Some(ConnDir { down: true, .. }) => '▼',
            _ => '▲',
        },
        Terrain::Lift => {
            if (tick / 10).is_multiple_of(2) {
                '≡'
            } else {
                '⇅'
            }
        }
        Terrain::Ladder => match connector {
            Some(ConnDir {
                kind: ConnectorKind::Manhole,
                ..
            }) => '◍',
            _ => '╫',
        },
        Terrain::Grate => '▦',
    }
}

/// Legend rows for a stratum: glyph and label.
pub fn legend_entries(k: StratumKind) -> Vec<(char, &'static str)> {
    let common = [
        ('▲', "stairs"),
        ('≡', "lift"),
        ('╫', "ladder"),
        ('▣', "terminal"),
        ('●', "lamp"),
    ];
    let mut out: Vec<(char, &'static str)> = match k {
        StratumKind::Undercity => vec![
            (' ', "tunnel"),
            ('≈', "sewage"),
            ('▦', "grate"),
            ('╬', "rail"),
            ('◍', "manhole"),
            ('╌', "pipe"),
            ('☰', "vent"),
        ],
        StratumKind::Street => vec![
            (' ', "road"),
            ('░', "sidewalk"),
            ('▫', "plaza/window"),
            ('│', "wall"),
            ('▯', "door"),
            ('▪', "neon"),
            ('▣', "holo ad"),
            ('▬', "barrier"),
            ('▤', "stall"),
            ('◉', "camera"),
            ('~', "puddle/water"),
        ],
        StratumKind::Skyways => vec![
            ('═', "walkway"),
            ('╌', "cable"),
            ('│', "wall"),
            ('▯', "door"),
        ],
        StratumKind::Heights => vec![
            ('█', "rooftop"),
            ('╍', "bridge"),
            ('H', "helipad"),
            ('▩', "ac unit"),
        ],
        StratumKind::Net => vec![
            ('◇', "node"),
            ('◆', "server"),
            ('⬡', "backbone"),
            ('◈', "ice"),
        ],
    };
    out.extend(common);
    out
}

/// Base colours for a terrain within a theme: (fg, bg).
fn terrain_colors(terrain: Terrain, ctx: &RenderCtx) -> (Rgb, Rgb) {
    let ts = ctx.style.theme_or_neutral(ctx.theme);
    let bg0 = ctx.style.background;
    match terrain {
        Terrain::Void => (bg0, bg0),
        Terrain::Water => (Rgb(50, 110, 170), Rgb(8, 22, 48)),
        Terrain::Sewage => (Rgb(70, 150, 70), Rgb(10, 28, 12)),
        Terrain::Road => (ts.road.scale(1.8), ts.road),
        Terrain::Sidewalk => (ts.sidewalk, ts.road.scale(1.1)),
        Terrain::Alley => (ts.road.scale(1.6), ts.road.scale(1.25)),
        Terrain::Plaza => (ts.accent2.scale(0.5), ts.sidewalk.scale(0.85)),
        Terrain::Floor => (ts.floor.scale(1.6), ts.floor),
        Terrain::Wall => (ts.wall, bg0),
        Terrain::Door => (ts.accent2, ts.floor),
        Terrain::Window => (ts.accent.scale(0.75), bg0),
        Terrain::Rooftop => (ts.roof, ts.roof.scale(0.4)),
        Terrain::Walkway => (ts.accent.scale(0.85), Rgb(18, 22, 30)),
        Terrain::Bridge => (ts.wall.scale(1.3), Rgb(18, 22, 30)),
        Terrain::Tunnel => (Rgb(48, 58, 48), Rgb(13, 16, 14)),
        Terrain::Rail => (Rgb(140, 125, 95), Rgb(13, 16, 14)),
        Terrain::Stair | Terrain::Lift | Terrain::Ladder => (ts.accent2, ts.floor),
        Terrain::Grate => (Rgb(100, 112, 100), Rgb(13, 16, 14)),
    }
}

/// Feature foreground colour, if the feature overrides it.
fn feature_color(f: Feature, tile: &Tile, ctx: &RenderCtx, tick: u64) -> Rgb {
    let ts = ctx.style.theme_or_neutral(ctx.theme);
    match f {
        Feature::Neon(h) => {
            ctx.style
                .hue(h)
                .scale(flicker(tick, tile.variant, tile.variant.is_multiple_of(5)))
        }
        Feature::HoloAd => ts
            .accent2
            .scale(0.7 + 0.3 * pulse(tick + tile.variant as u64, 24)),
        Feature::Lamp => Rgb(255, 240, 200),
        Feature::Vent => Rgb(170, 175, 185),
        Feature::Puddle => Rgb(80, 115, 165),
        Feature::Trash => Rgb(115, 105, 85),
        Feature::Barrier => Rgb(255, 60, 60),
        Feature::Stall => ts.accent,
        Feature::Sign(_) => ts.accent2,
        Feature::Terminal => Rgb(0, 230, 210),
        Feature::Camera => Rgb(255, 60, 70),
        Feature::AcUnit => Rgb(125, 135, 145),
        Feature::Helipad => Rgb(230, 230, 130),
        Feature::Pipe => Rgb(125, 105, 85),
        Feature::Cable => Rgb(85, 85, 95),
    }
}

/// The styled cell for a tile. Remembered tiles freeze at tick 0.
pub fn cell(tile: &Tile, ctx: &RenderCtx) -> Cell {
    let tick = if ctx.vis == Visibility::Remembered {
        0
    } else {
        ctx.tick
    };
    let ch = glyph_for(
        tile.terrain,
        tile.feature,
        ctx.connector,
        ctx.kind,
        tile.variant,
        tick,
        ctx.wall_mask,
        ctx.style,
    );
    let (mut fg, mut bg) = terrain_colors(tile.terrain, ctx);
    if let Some(f) = tile.feature {
        fg = feature_color(f, tile, ctx, tick);
    }
    let (dr, dg, db) = ctx.style.stratum(ctx.stratum).tint;
    fg = fg.tint(dr, dg, db);
    bg = bg.tint(dr, dg, db);
    let emits = matches!(
        tile.feature,
        Some(
            Feature::Neon(_)
                | Feature::HoloAd
                | Feature::Lamp
                | Feature::Terminal
                | Feature::Camera
        )
    );
    let fg = if emits { fg } else { lit(fg, ctx.light) };
    let bg = lit(bg, ctx.light);
    let bold =
        tile.is_connector() || matches!(tile.feature, Some(Feature::Sign(_) | Feature::Neon(_)));
    Cell {
        ch,
        fg,
        bg,
        bold,
        dim: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::lighting::apply_vis;
    use crate::render::style::StylePack;
    use crate::render::{RenderCtx, Visibility};
    use citygen::{ALL_FEATURES, ALL_TERRAIN, Feature, Hue, StratumKind, Terrain, Theme, Tile};
    use unicode_width::UnicodeWidthChar;

    #[test]
    fn style_glyphs_are_single_width() {
        let s = StylePack::load().unwrap();
        for set in [
            &s.glyphs.holo,
            &s.glyphs.rain,
            &s.glyphs.water,
            &s.glyphs.steam,
            &s.glyphs.mass,
            &s.glyphs.net,
        ] {
            for c in set {
                assert_eq!(c.width(), Some(1), "{c:?}");
            }
        }
        for terrain in ALL_TERRAIN {
            for v in [0u8, 77, 200] {
                for tick in [0u64, 7, 20] {
                    for mask in 0..16u8 {
                        let c = glyph_for(terrain, None, None, None, v, tick, mask, &s);
                        assert_eq!(c.width(), Some(1), "{terrain:?} mask {mask}");
                    }
                }
            }
        }
        for f in ALL_FEATURES {
            for tick in [0u64, 4, 15] {
                let c = glyph_for(Terrain::Floor, Some(f), None, None, 1, tick, 0, &s);
                assert_eq!(c.width(), Some(1), "{f:?}");
            }
        }
        for k in StratumKind::PHYSICAL {
            for (c, label) in legend_entries(k) {
                assert_eq!(c.width(), Some(1), "{label}");
            }
        }
    }

    #[test]
    fn cell_mapping_is_pure_and_lit() {
        let s = StylePack::load().unwrap();
        let ctx = |light, tick| RenderCtx {
            style: &s,
            theme: Some(Theme::CorpCore),
            stratum: StratumKind::Street,
            light,
            tick,
            vis: Visibility::Visible,
            connector: None,
            kind: None,
            wall_mask: 0,
        };
        let wall = Tile::new(Terrain::Wall);
        assert_eq!(cell(&wall, &ctx(255, 0)), cell(&wall, &ctx(255, 0)));
        assert!(cell(&wall, &ctx(255, 0)).fg.0 > cell(&wall, &ctx(0, 0)).fg.0);
        let mut neon = Tile::new(Terrain::Wall);
        neon.feature = Some(Feature::Neon(Hue::Magenta));
        neon.variant = 5; // broken sign: variant.is_multiple_of(5)
        let on = cell(&neon, &ctx(200, 0));
        let off = cell(&neon, &ctx(200, 117));
        assert!(off.fg.0 < on.fg.0 / 2, "on {:?} off {:?}", on.fg, off.fg);
        let water = Tile::new(Terrain::Water);
        assert_ne!(cell(&water, &ctx(100, 0)).ch, cell(&water, &ctx(100, 6)).ch);
        let unseen = apply_vis(cell(&wall, &ctx(255, 0)), Visibility::Unseen);
        assert_eq!(unseen.ch, ' ');
        let remembered = apply_vis(cell(&wall, &ctx(255, 0)), Visibility::Remembered);
        assert!(remembered.fg.0 < cell(&wall, &ctx(255, 0)).fg.0);
        assert_eq!(
            glyph_for(Terrain::Wall, None, None, None, 0, 0, 0b0101, &s),
            '│'
        );
        assert_eq!(
            glyph_for(Terrain::Wall, None, None, None, 0, 0, 0b0110, &s),
            '┌'
        );
        assert_eq!(
            glyph_for(Terrain::Wall, None, None, None, 0, 0, 0b1111, &s),
            '┼'
        );
        assert!(connects(Terrain::Wall, Terrain::Door));
        assert!(!connects(Terrain::Wall, Terrain::Floor));
        assert!(connects(Terrain::Walkway, Terrain::Bridge));
    }
}
