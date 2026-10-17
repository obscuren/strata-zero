//! The tileset: one pure function from a tile and its context to a styled cell.

use crate::anim::effects::{flicker, pulse};
use crate::render::lighting::lit;
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use crate::render::{Cell, ConnDir, RenderCtx, Rgb, Visibility};
use citygen::{
    BuildingKind, ConnectorKind, Feature, Furniture, IceGrade, NodeKind, StratumKind, Terrain, Tile,
};

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

/// Whether two adjacent terrains draw as one continuous structure (walls, walkways, lanes).
pub fn connects(a: Terrain, b: Terrain) -> bool {
    use Terrain::*;
    let wallish = |t| matches!(t, Wall | Door | Window);
    let spanish = |t| matches!(t, Walkway | Bridge);
    let railing = |t| matches!(t, Railing);
    let netwall = |t| matches!(t, NetWall | NetGate);
    let lane = |t| matches!(t, NetLane | NetGate);
    (wallish(a) && wallish(b))
        || (spanish(a) && (spanish(b) || matches!(b, Door | Floor | Stair | Lift | Ladder)))
        || (railing(a) && railing(b))
        || (netwall(a) && netwall(b))
        || (lane(a) && lane(b))
}

/// Glyph for a Net node kind from the style's `net` set.
pub fn node_glyph(kind: NodeKind, style: &StylePack) -> char {
    let g = &style.glyphs.net;
    let i = match kind {
        NodeKind::Terminal => 0,
        NodeKind::Server => 1,
        NodeKind::Backbone => 2,
        NodeKind::DataVault => 3,
        NodeKind::Ice => 5,
        _ => 4,
    };
    g[i % g.len()]
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
            Feature::Ice(_) => '▣',
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
            Feature::Awning => '▔',
            Feature::Node(k) => node_glyph(k, style),
            Feature::Furniture(piece) => match piece {
                Furniture::Desk => '▭',
                Furniture::Counter => '═',
                Furniture::Table => '○',
                Furniture::Stool => '·',
                Furniture::Shelf => '≣',
                Furniture::Machine => '⚙',
                Furniture::Bed => '▤',
                Furniture::Shrine => '♦',
                Furniture::Locker => '▮',
                Furniture::Couch => '▬',
                Furniture::Shower => '≋',
                Furniture::Bench => '╥',
                Furniture::Vending => '▣',
                Furniture::Bike => 'ꝺ',
            },
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
        Terrain::Mass => pick(&g.mass, variant as u64),
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
        Terrain::NetGrid => {
            if variant & 1 == 1 {
                '·'
            } else {
                ' '
            }
        }
        Terrain::NetLane => DOUBLE[(mask & 15) as usize],
        Terrain::NetWall => {
            let set = if kind.is_some_and(|k| k.is_corp()) {
                &DOUBLE
            } else {
                &HEAVY
            };
            set[(mask & 15) as usize]
        }
        Terrain::NetFloor => ' ',
        Terrain::NetGate => '▯',
        Terrain::Atrium => ' ',
        Terrain::Railing => {
            if horizontal {
                '─'
            } else {
                '│'
            }
        }
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
            ('─', "railing"),
            ('▬', "couch"),
            ('≋', "shower"),
            ('▔', "awning"),
            ('╥', "bench"),
            ('▣', "vending machine"),
            ('ꝺ', "bike"),
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
            ('·', "grid"),
            ('═', "lane"),
            ('┃', "construct wall"),
            ('▯', "gate"),
            ('◇', "terminal / port"),
            ('◆', "server"),
            ('⬡', "backbone"),
            ('⌗', "data vault"),
            ('◈', "ice"),
            ('▪', "sensor"),
        ],
    };
    if k != StratumKind::Net {
        out.extend(common);
    }
    out
}

/// Base colours for a terrain within a theme: (fg, bg).
fn terrain_colors(terrain: Terrain, tile: &Tile, ctx: &RenderCtx, tick: u64) -> (Rgb, Rgb) {
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
        Terrain::Mass => (ts.wall.scale(0.45), bg0),
        Terrain::Rooftop => (ts.roof, ts.roof.scale(0.4)),
        Terrain::Walkway => (ts.accent.scale(0.85), Rgb(18, 22, 30)),
        Terrain::Bridge => (ts.wall.scale(1.3), Rgb(18, 22, 30)),
        Terrain::Tunnel => (Rgb(48, 58, 48), Rgb(13, 16, 14)),
        Terrain::Rail => (Rgb(140, 125, 95), Rgb(13, 16, 14)),
        Terrain::Stair | Terrain::Lift | Terrain::Ladder => (ts.accent2, ts.floor),
        Terrain::Grate => (Rgb(100, 112, 100), Rgb(13, 16, 14)),
        Terrain::NetGrid => (ts.accent.mix(SIGNAL, 0.5).scale(0.45), Rgb(3, 6, 10)),
        Terrain::NetLane => (
            SIGNAL.scale(0.6 + 0.4 * pulse(tick + tile.variant as u64, 20)),
            Rgb(3, 6, 10),
        ),
        Terrain::NetWall => (SIGNAL, Rgb(3, 6, 10)),
        Terrain::NetFloor => (Rgb(10, 40, 44), Rgb(6, 18, 22)),
        Terrain::NetGate => (Rgb(200, 255, 245), Rgb(6, 18, 22)),
        Terrain::Atrium => (bg0, bg0),
        Terrain::Railing => (ts.wall, ts.floor),
    }
}

/// Feature foreground colour, if the feature overrides it.
fn feature_color(f: Feature, tile: &Tile, ctx: &RenderCtx, tick: u64) -> Rgb {
    let ts = ctx.style.theme_or_neutral(ctx.theme);
    match f {
        Feature::Ice(IceGrade::White) => Rgb(235, 240, 250).scale(0.7 + 0.3 * pulse(tick, 20)),
        Feature::Ice(IceGrade::Grey) => Rgb(255, 0, 200).scale(0.7 + 0.3 * pulse(tick, 20)),
        Feature::Ice(IceGrade::Black) => Rgb(255, 40, 40).scale(0.7 + 0.3 * pulse(tick, 12)),
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
        Feature::Awning => ts.accent.scale(0.6),
        Feature::Node(_) => {
            Rgb(240, 255, 250).scale(0.7 + 0.3 * pulse(tick + tile.variant as u64, 30))
        }
        Feature::Furniture(Furniture::Counter | Furniture::Shrine) => ts.accent2,
        Feature::Furniture(_) => ts.wall.scale(1.2),
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
    let (mut fg, mut bg) = terrain_colors(tile.terrain, tile, ctx, tick);
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
                | Feature::Node(_)
        )
    );
    let fg = if emits { fg } else { lit(fg, ctx.light) };
    let bg = lit(bg, ctx.light);
    let jack = ctx.connector.is_some_and(|c| c.kind == ConnectorKind::Jack);
    let bold = tile.is_connector()
        || matches!(tile.feature, Some(Feature::Sign(_) | Feature::Neon(_)))
        || (matches!(tile.feature, Some(Feature::Node(_))) && jack);
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
    use citygen::{
        ALL_FEATURES, ALL_TERRAIN, Feature, Hue, NodeKind, StratumKind, Terrain, Theme, Tile,
    };
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

    #[test]
    fn home_furniture_and_railings_have_glyphs() {
        let s = StylePack::load().unwrap();
        for (piece, ch) in [
            (Furniture::Couch, '▬'),
            (Furniture::Shower, '≋'),
            (Furniture::Bench, '╥'),
            (Furniture::Vending, '▣'),
            (Furniture::Bike, 'ꝺ'),
        ] {
            let f = Some(Feature::Furniture(piece));
            assert_eq!(glyph_for(Terrain::Floor, f, None, None, 1, 0, 0, &s), ch);
        }
        assert!(connects(Terrain::Railing, Terrain::Railing));
        assert!(!connects(Terrain::Railing, Terrain::Wall));
        assert_eq!(
            glyph_for(
                Terrain::Railing,
                None,
                None,
                None,
                0,
                0,
                MASK_E | MASK_W,
                &s
            ),
            '─'
        );
        assert_eq!(
            glyph_for(Terrain::Atrium, None, None, None, 0, 0, 0, &s),
            ' '
        );
    }

    #[test]
    fn net_tiles_render_in_the_signal_palette() {
        let s = StylePack::load().unwrap();
        let ctx = |stratum| RenderCtx {
            style: &s,
            theme: Some(Theme::CorpCore),
            stratum,
            light: 255,
            tick: 0,
            vis: Visibility::Visible,
            connector: None,
            kind: None,
            wall_mask: MASK_E | MASK_W,
        };
        let wall = cell(&Tile::new(Terrain::NetWall), &ctx(StratumKind::Net));
        assert_eq!(wall.ch, '━');
        assert!(wall.fg.2 > wall.fg.0, "wall is cyan-ish: {:?}", wall.fg);
        let lane = cell(&Tile::new(Terrain::NetLane), &ctx(StratumKind::Net));
        assert_eq!(lane.ch, '═');
        let mut grid = Tile::new(Terrain::NetGrid);
        grid.variant = 1;
        assert_eq!(cell(&grid, &ctx(StratumKind::Net)).ch, '·');
        grid.variant = 2;
        assert_eq!(cell(&grid, &ctx(StratumKind::Net)).ch, ' ');
        assert_eq!(
            cell(&Tile::new(Terrain::NetGate), &ctx(StratumKind::Net)).ch,
            '▯'
        );
        let mut node = Tile::new(Terrain::NetGrid);
        node.feature = Some(Feature::Node(NodeKind::Server));
        assert_eq!(
            cell(&node, &ctx(StratumKind::Net)).ch,
            node_glyph(NodeKind::Server, &s)
        );
        assert_eq!(node_glyph(NodeKind::Server, &s), '◆');
        assert_eq!(node_glyph(NodeKind::Ice, &s), '◈');
        assert!(connects(Terrain::NetWall, Terrain::NetGate));
        assert!(connects(Terrain::NetLane, Terrain::NetGate));
        assert!(!connects(Terrain::NetLane, Terrain::NetWall));
        let legend = legend_entries(StratumKind::Net);
        assert!(legend.iter().any(|(_, l)| *l == "construct wall"));
        assert!(!legend.iter().any(|(_, l)| *l == "stairs"));
    }
}
