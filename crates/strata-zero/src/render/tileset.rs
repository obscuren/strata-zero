//! The tileset: one pure function from a tile and its context to a styled cell.

use crate::anim::effects::{flicker, pulse};
use crate::render::lighting::{lit, lit_by};
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use crate::render::{Cell, ConnDir, RenderCtx, Rgb, Visibility};
use citygen::lighting::tuning::tone_rgb;
use citygen::{
    BuildingKind, ConnectorKind, Feature, Fitting, FittingKind, Furniture, IceGrade, NodeKind,
    StratumKind, Terrain, Tile, Tone, Wear,
};

pub(crate) const LIGHT: [char; 16] = [
    '─', '│', '─', '└', '│', '│', '┌', '├', '─', '┘', '─', '┴', '┐', '┤', '┬', '┼',
];
pub(crate) const HEAVY: [char; 16] = [
    '━', '┃', '━', '┗', '┃', '┃', '┏', '┣', '━', '┛', '━', '┻', '┓', '┫', '┳', '╋',
];
pub(crate) const DOUBLE: [char; 16] = [
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

/// The holo-ad glyphs in turn: a holo set's picture at `tick`.
pub fn holo_glyph(style: &StylePack, tick: u64) -> char {
    let set = &style.glyphs.holo;
    set[(tick / 8 % set.len() as u64) as usize]
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
    // A wall's fitting and a stage's lights draw over the ground's own glyph.
    let feature = feature.filter(|f| !keeps_ground(*f));
    if let Some(f) = feature {
        return match f {
            Feature::Ice(_) => '▣',
            Feature::Neon(_) => '▪',
            Feature::HoloAd => pick(&g.holo, tick / 8 + variant as u64),
            Feature::Lamp => '●',
            Feature::Fitting(x) => match x.kind {
                FittingKind::Bulb => '•',
                _ => '▬',
            },
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
            Feature::Pylon => '╫',
            Feature::PowerLine(false) => '┄',
            Feature::PowerLine(true) => '┆',
            Feature::FireBarrel => '•',
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
                Furniture::Vending => '◘',
                Furniture::Bike => 'ꝺ',
                Furniture::Chair => '╦',
                Furniture::Holo => '◫',
            },
        };
    }
    let horizontal = mask & (MASK_E | MASK_W) != 0;
    // A deck or a rail runs the way it goes on: east-west where it goes on both ways, or
    // one way with nothing north or south (a deck two wide has its twin beside it).
    let runs_across =
        mask & (MASK_E | MASK_W) == MASK_E | MASK_W || horizontal && mask & (MASK_N | MASK_S) == 0;
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
            if runs_across {
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
            if runs_across {
                '─'
            } else {
                '│'
            }
        }
        // Open ground: sparse, so the screen reads quiet and what's built carries it.
        Terrain::Dirt => one_in(variant, 16, '·'),
        Terrain::Scrub => ['"', ',', ' '][(variant % 3) as usize],
        Terrain::Crops => one_in(variant, 2, '▬'),
        Terrain::Salt => ['░', '·'][(variant % 2) as usize],
        Terrain::Spill => ['≈', '·', '·', ' ', ' '][(variant % 5) as usize],
        Terrain::Gravel => one_in(variant, 2, '∙'),
        Terrain::Sand => ['░', '·', ' '][(variant % 3) as usize],
        Terrain::Reeds => ['"', '·', ' '][(variant % 3) as usize],
        Terrain::Rock => ['■', '▪', '▪'][(variant % 3) as usize],
        Terrain::Cliff => ['█', '▓', '▓'][(variant % 3) as usize],
        Terrain::Shallows => ['~', '≈', ' '][((tick / 6 + variant as u64) % 3) as usize],
        Terrain::Ford => one_in(variant, 2, '·'),
        Terrain::Rise => ['·', '░', '░', '░'][(variant % 4) as usize],
        Terrain::Wash => ['░', '·', ' '][(variant % 3) as usize],
        // The way it runs from here.
        Terrain::Escalator => match connector {
            Some(ConnDir {
                up: true,
                down: true,
                ..
            }) => '⇅',
            Some(ConnDir { down: true, .. }) => '↓',
            _ => '↑',
        },
    }
}

/// `ch` on one tile in `n`, picked by its variant; blank on the rest.
fn one_in(variant: u8, n: u8, ch: char) -> char {
    if variant.is_multiple_of(n) { ch } else { ' ' }
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
            ('◘', "vending machine"),
            ('ꝺ', "bike"),
            ('╦', "operating chair"),
            ('◫', "holo set"),
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
        Terrain::Stair | Terrain::Lift | Terrain::Ladder | Terrain::Escalator => {
            (ts.accent2, ts.floor)
        }
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
        Terrain::Dirt => (Rgb(67, 60, 48), Rgb(13, 11, 9)),
        Terrain::Scrub => (Rgb(74, 90, 48), Rgb(13, 12, 9)),
        Terrain::Crops => (Rgb(58, 106, 48), Rgb(11, 12, 8)),
        Terrain::Salt => (Rgb(90, 88, 82), Rgb(18, 18, 16)),
        Terrain::Spill => (Rgb(74, 138, 34), Rgb(6, 12, 4)),
        Terrain::Gravel => (Rgb(74, 69, 60), Rgb(20, 19, 16)),
        Terrain::Sand => (Rgb(138, 122, 88), Rgb(28, 24, 18)),
        Terrain::Reeds => (Rgb(94, 112, 64), Rgb(15, 18, 11)),
        Terrain::Rock => (Rgb(94, 88, 80), Rgb(13, 11, 9)),
        Terrain::Cliff => (Rgb(52, 49, 44), Rgb(12, 11, 10)),
        Terrain::Shallows => (Rgb(106, 166, 216), Rgb(18, 60, 94)),
        Terrain::Ford => (Rgb(90, 140, 190), Rgb(14, 40, 64)),
        Terrain::Rise => (Rgb(70, 60, 44), Rgb(20, 17, 13)),
        Terrain::Wash => (Rgb(96, 84, 60), Rgb(22, 19, 14)),
    }
}

/// The operating chair's cyan.
const CHAIR: Rgb = Rgb(0, 229, 255);

/// A fire burning in a drum in the outskirts (round 2's).
const FIRE: Rgb = Rgb(255, 140, 58);

/// The caps either end of a roadside sign (round 2's Kilo-9).
const SIGN_CAP: Rgb = Rgb(60, 140, 255);
/// The orange of a roadside sign's letters (round 2's Kilo-9).
const ROADSIDE_SIGN: Rgb = Rgb(255, 140, 0);

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
        Feature::Fitting(x) => match x.wear {
            Wear::Ok => fitting_colour(x),
            Wear::Dead => DEAD_FITTING,
            Wear::Stutter => fitting_colour(x).scale(flicker(tick, tile.variant, true)),
        },
        Feature::Vent => Rgb(170, 175, 185),
        Feature::Puddle => Rgb(80, 115, 165),
        Feature::Trash => Rgb(115, 105, 85),
        Feature::Barrier => Rgb(255, 60, 60),
        Feature::Stall => ts.accent,
        // a roadside sign's caps in blue, its letters in the sign's orange
        Feature::Sign('▣') => SIGN_CAP,
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
        Feature::Furniture(Furniture::Chair) => CHAIR,
        Feature::Furniture(_) => ts.wall.scale(1.2),
        Feature::Pylon => Rgb(106, 110, 118),
        // round 2's fire in a drum, guttering
        Feature::FireBarrel => FIRE.scale(flicker(tick, tile.variant, true)),
        Feature::PowerLine(_) => Rgb(42, 46, 54),
    }
}

/// A fitting that leaves the ground's glyph showing: set in a wall, or a stage's lights.
fn keeps_ground(f: Feature) -> bool {
    matches!(f, Feature::Fitting(x) if x.kind.on_wall() || x.kind == FittingKind::Stage)
}

/// A dead fitting's grey.
const DEAD_FITTING: Rgb = Rgb(0x3e, 0x47, 0x50);
/// A bare bulb's warm white.
const BULB: Rgb = Rgb(0xff, 0xd9, 0xa0);
/// Furniture whose light has a channel this bright takes the light's colour.
const LIT_FURNITURE: u8 = 180;

/// A fitting's own colour, by its tone (the spec's): a pale tint of the light it throws,
/// neon still saturated, a bulb its warm white.
pub fn fitting_colour(f: Fitting) -> Rgb {
    if f.kind == FittingKind::Bulb {
        return BULB;
    }
    match f.tone {
        Tone::Fluorescent => Rgb(0xea, 0xff, 0xfa),
        Tone::Led => Rgb(0xe8, 0xf0, 0xff),
        Tone::Tungsten => BULB,
        Tone::Surgical => Rgb(0xff, 0xff, 0xff),
        Tone::Sodium => Rgb(0xff, 0xc0, 0x70),
        Tone::Blacklight => Rgb(0xd9, 0xa8, 0xff),
        Tone::Neon(_) => {
            let (r, g, b) = tone_rgb(f.tone);
            Rgb(r, g, b).mix(Rgb(255, 255, 255), 0.2)
        }
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
    let emits = match tile.feature {
        Some(Feature::Lamp) => ctx.lamps_on,
        Some(
            Feature::Neon(_)
            | Feature::HoloAd
            | Feature::Terminal
            | Feature::Camera
            | Feature::Node(_)
            | Feature::Fitting(_)
            | Feature::FireBarrel,
        ) => true,
        _ => false,
    };
    let (mut fg, mut bg) = match ctx.glow {
        Some(l) => (
            if emits { fg } else { lit_by(fg, l, true) },
            lit_by(bg, l, false),
        ),
        None => (
            if emits { fg } else { lit(fg, ctx.light) },
            lit(bg, ctx.light),
        ),
    };
    if let (Some(l), Some(Feature::Furniture(_))) = (ctx.glow, tile.feature) {
        let top = l.0.max(l.1).max(l.2);
        if top >= LIT_FURNITURE {
            fg = fg.mix(l.scale(255.0 / top as f32), 0.7);
        }
    }
    let fitting = match tile.feature {
        Some(Feature::Fitting(f)) if f.wear != Wear::Dead => Some(f),
        _ => None,
    };
    if let Some(f) = fitting {
        if f.kind.on_wall() {
            bg = fitting_colour(f).scale(0.22);
        } else if f.kind == FittingKind::Stage {
            bg = bg.mix(fitting_colour(f).scale(0.35), 0.45);
        }
    }
    if let Some((sky, w)) = ctx.sky {
        if !emits {
            fg = fg.mix(sky, w);
        }
        bg = bg.mix(sky, w);
    }
    // a roadside sign's letters in the sign's orange, where no district colours them:
    // on the open ground and on a city's own ground past its outline alike
    if let Some(Feature::Sign(c)) = tile.feature
        && c != '▣'
        && c != '!'
        && tile.district.is_none()
    {
        fg = ROADSIDE_SIGN;
    }
    let jack = ctx.connector.is_some_and(|c| c.kind == ConnectorKind::Jack);
    let bold = tile.is_connector()
        || matches!(tile.feature, Some(Feature::Sign(_) | Feature::Neon(_)))
        || fitting.is_some_and(|f| f.kind.on_wall() || f.kind == FittingKind::Bulb)
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
        ALL_FEATURES, ALL_TERRAIN, Feature, Fitting, FittingKind, Furniture, Hue, NodeKind,
        StratumKind, Terrain, Theme, Tile, Tone, Wear,
    };
    use unicode_width::UnicodeWidthChar;

    #[test]
    fn a_roadside_sign_is_orange_between_blue_caps() {
        let style = StylePack::load().unwrap();
        let ctx = RenderCtx {
            style: &style,
            theme: None,
            stratum: StratumKind::Street,
            light: 110,
            tick: 0,
            vis: Visibility::Visible,
            connector: None,
            kind: None,
            wall_mask: 0,
            lamps_on: true,
            sky: None,
            glow: None,
        };
        let sign = |c: char, district: Option<citygen::DistrictId>| {
            let mut t = Tile::new(Terrain::Gravel);
            t.feature = Some(Feature::Sign(c));
            t.district = district;
            cell(&t, &ctx)
        };
        assert_eq!(sign('K', None).fg, ROADSIDE_SIGN);
        assert_eq!(sign('▣', None).ch, '▣');
        assert!(sign('▣', None).fg.2 > sign('▣', None).fg.0, "a blue cap");
        // a district's own sign keeps its colour
        assert_ne!(sign('K', Some(citygen::DistrictId(0))).fg, ROADSIDE_SIGN);
    }

    #[test]
    fn a_fire_barrel_burns_bright_at_night() {
        let style = crate::render::style::StylePack::load().unwrap();
        let mut t = Tile::new(Terrain::Dirt);
        t.feature = Some(Feature::FireBarrel);
        let at = |light: u8| {
            let ctx = RenderCtx {
                style: &style,
                theme: None,
                stratum: StratumKind::Street,
                light,
                tick: 0,
                vis: Visibility::Visible,
                connector: None,
                kind: None,
                wall_mask: 0,
                lamps_on: true,
                sky: None,
                glow: None,
            };
            cell(&t, &ctx).fg
        };
        assert_eq!(at(20), at(255), "a fire lights itself");
    }

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
    fn a_wide_bridge_runs_the_way_it_goes() {
        let s = StylePack::load().unwrap();
        let g = |t, mask| glyph_for(t, None, None, None, 1, 0, mask, &s);
        // a deck two wide running north-south: each cell has its twin beside it
        assert_eq!(g(Terrain::Bridge, MASK_N | MASK_S | MASK_E), '╏');
        assert_eq!(g(Terrain::Bridge, MASK_N | MASK_S | MASK_W), '╏');
        // and east-west, its twin above or below
        assert_eq!(g(Terrain::Bridge, MASK_E | MASK_W | MASK_S), '╍');
        assert_eq!(g(Terrain::Bridge, MASK_E | MASK_W), '╍');
        assert_eq!(g(Terrain::Bridge, MASK_E), '╍');
        assert_eq!(g(Terrain::Railing, MASK_N | MASK_S), '│');
        assert_eq!(g(Terrain::Railing, MASK_W | MASK_E), '─');
    }

    #[test]
    fn an_escalator_points_the_way_it_runs() {
        let s = StylePack::load().unwrap();
        let g = |up, down| {
            let way = ConnDir {
                kind: ConnectorKind::Stair,
                up,
                down,
            };
            glyph_for(Terrain::Escalator, None, Some(way), None, 1, 0, 0, &s)
        };
        assert_eq!(g(true, false), '↑');
        assert_eq!(g(false, true), '↓');
        assert_eq!(g(true, true), '⇅');
    }

    #[test]
    fn open_ground_draws_in_one_cell() {
        let s = StylePack::load().unwrap();
        let ground = [
            Terrain::Dirt,
            Terrain::Scrub,
            Terrain::Crops,
            Terrain::Salt,
            Terrain::Spill,
            Terrain::Gravel,
            Terrain::Sand,
            Terrain::Reeds,
            Terrain::Rock,
            Terrain::Cliff,
            Terrain::Shallows,
            Terrain::Ford,
            Terrain::Rise,
            Terrain::Wash,
        ];
        for t in ground {
            let mut drawn = std::collections::BTreeSet::new();
            for variant in 0..=255u8 {
                let c = glyph_for(t, None, None, None, variant, 0, 0, &s);
                assert_eq!(c.width(), Some(1), "{t:?} {variant}");
                drawn.insert(c);
            }
            assert!(drawn.len() >= 2, "{t:?} draws only {drawn:?}");
        }
        assert_eq!(
            glyph_for(Terrain::Dirt, Some(Feature::Pylon), None, None, 0, 0, 0, &s),
            '╫'
        );
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
            lamps_on: true,
            sky: None,
            glow: None,
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
    fn the_chair_is_cyan_and_in_the_legend() {
        let s = StylePack::load().unwrap();
        let ctx = RenderCtx {
            style: &s,
            theme: Some(Theme::NightMarket),
            stratum: StratumKind::Street,
            light: 255,
            tick: 0,
            vis: Visibility::Visible,
            connector: None,
            kind: None,
            wall_mask: 0,
            lamps_on: true,
            sky: None,
            glow: None,
        };
        let mut t = Tile::new(Terrain::Floor);
        t.feature = Some(Feature::Furniture(Furniture::Chair));
        assert_eq!(feature_color(t.feature.unwrap(), &t, &ctx, 0), CHAIR);
        assert_eq!(cell(&t, &ctx).ch, '╦');
        assert!(legend_entries(StratumKind::Street).contains(&('╦', "operating chair")));
    }

    #[test]
    fn home_furniture_and_railings_have_glyphs() {
        let s = StylePack::load().unwrap();
        for (piece, ch) in [
            (Furniture::Couch, '▬'),
            (Furniture::Shower, '≋'),
            (Furniture::Bench, '╥'),
            (Furniture::Vending, '◘'),
            (Furniture::Bike, 'ꝺ'),
            (Furniture::Chair, '╦'),
            (Furniture::Holo, '◫'),
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
    fn a_lamp_glows_at_night_and_is_furniture_by_day_and_the_sky_tints() {
        let s = StylePack::load().unwrap();
        let ctx = |lamps_on, sky| RenderCtx {
            style: &s,
            theme: Some(Theme::CorpCore),
            stratum: StratumKind::Street,
            light: 40,
            tick: 0,
            vis: Visibility::Visible,
            connector: None,
            kind: None,
            wall_mask: 0,
            lamps_on,
            sky,
            glow: None,
        };
        let mut lamp = Tile::new(Terrain::Sidewalk);
        lamp.feature = Some(Feature::Lamp);
        let night = cell(&lamp, &ctx(true, None));
        let day = cell(&lamp, &ctx(false, None));
        assert_ne!(
            night.fg, day.fg,
            "a lit lamp keeps its colour; an unlit one is scaled"
        );
        let floor = Tile::new(Terrain::Sidewalk);
        let plain = cell(&floor, &ctx(true, None));
        let tinted = cell(&floor, &ctx(true, Some((Rgb(255, 0, 0), 0.5))));
        assert!(tinted.bg.0 > plain.bg.0, "the sky pulls toward red");
    }

    fn glow_ctx(s: &StylePack, glow: Option<Rgb>) -> RenderCtx<'_> {
        RenderCtx {
            style: s,
            theme: Some(Theme::CorpCore),
            stratum: StratumKind::Street,
            light: 120,
            tick: 0,
            vis: Visibility::Visible,
            connector: None,
            kind: None,
            wall_mask: 0,
            lamps_on: true,
            sky: None,
            glow,
        }
    }

    fn fit(kind: FittingKind, tone: Tone, wear: Wear) -> Feature {
        Feature::Fitting(Fitting { kind, tone, wear })
    }

    fn street_tint(s: &StylePack, c: Rgb) -> Rgb {
        let (dr, dg, db) = s.stratum(StratumKind::Street).tint;
        c.tint(dr, dg, db)
    }

    #[test]
    fn a_warm_glow_warms_the_floor_and_a_cold_one_cools_it() {
        let s = StylePack::load().unwrap();
        let floor = Tile::new(Terrain::Floor);
        let warm = cell(&floor, &glow_ctx(&s, Some(Rgb(200, 130, 70))));
        assert!(warm.bg.0 > warm.bg.2, "{:?}", warm.bg);
        let cold = cell(&floor, &glow_ctx(&s, Some(Rgb(140, 180, 170))));
        assert!(cold.bg.2 >= cold.bg.0, "{:?}", cold.bg);
    }

    #[test]
    fn strata_cells_are_unchanged_without_glow() {
        let s = StylePack::load().unwrap();
        let ctx = glow_ctx(&s, None);
        let wall = Tile::new(Terrain::Wall);
        let (fg, bg) = terrain_colors(Terrain::Wall, &wall, &ctx, 0);
        let c = cell(&wall, &ctx);
        assert_eq!(c.fg, lit(street_tint(&s, fg), 120));
        assert_eq!(c.bg, lit(street_tint(&s, bg), 120));
    }

    #[test]
    fn fittings_draw_as_tubes_bulbs_and_lit_walls() {
        let s = StylePack::load().unwrap();
        let ctx = glow_ctx(&s, Some(Rgb(120, 120, 120)));
        let mut t = Tile::new(Terrain::Floor);
        t.feature = Some(fit(FittingKind::Tube, Tone::Fluorescent, Wear::Ok));
        assert_eq!(cell(&t, &ctx).ch, '▬');
        t.feature = Some(fit(FittingKind::Bulb, Tone::Tungsten, Wear::Ok));
        let bulb = cell(&t, &ctx);
        assert_eq!(bulb.ch, '•');
        assert!(bulb.bold);
        let strip = Fitting {
            kind: FittingKind::Strip,
            tone: Tone::Fluorescent,
            wear: Wear::Ok,
        };
        let mut w = Tile::new(Terrain::Wall);
        w.feature = Some(Feature::Fitting(strip));
        let mask = MASK_E | MASK_W;
        let wctx = RenderCtx {
            wall_mask: mask,
            ..glow_ctx(&s, Some(Rgb(120, 120, 120)))
        };
        let c = cell(&w, &wctx);
        assert_eq!(
            c.ch,
            glyph_for(Terrain::Wall, None, None, None, w.variant, 0, mask, &s)
        );
        assert_eq!(c.fg, street_tint(&s, fitting_colour(strip)));
        t.feature = Some(fit(FittingKind::Tube, Tone::Fluorescent, Wear::Dead));
        let dead = cell(&t, &ctx);
        assert_eq!(dead.fg, street_tint(&s, Rgb(0x3e, 0x47, 0x50)));
        assert_eq!(dead.bg, cell(&Tile::new(Terrain::Floor), &ctx).bg);
    }

    #[test]
    fn fittings_take_the_specs_colours() {
        let c = |kind, tone| {
            fitting_colour(Fitting {
                kind,
                tone,
                wear: Wear::Ok,
            })
        };
        assert_eq!(
            c(FittingKind::Tube, Tone::Fluorescent),
            Rgb(0xea, 0xff, 0xfa)
        );
        assert_eq!(c(FittingKind::Panel, Tone::Led), Rgb(0xe8, 0xf0, 0xff));
        assert_eq!(c(FittingKind::HighBay, Tone::Sodium), Rgb(0xff, 0xc0, 0x70));
        assert_eq!(c(FittingKind::Bulb, Tone::Tungsten), Rgb(0xff, 0xd9, 0xa0));
        assert_eq!(
            c(FittingKind::Sconce, Tone::Tungsten),
            Rgb(0xff, 0xd9, 0xa0)
        );
        assert_eq!(c(FittingKind::Strip, Tone::Surgical), Rgb(0xff, 0xff, 0xff));
        assert_eq!(
            c(FittingKind::Sconce, Tone::Blacklight),
            Rgb(0xd9, 0xa8, 0xff)
        );
    }

    #[test]
    fn furniture_in_strong_light_takes_its_colour() {
        let s = StylePack::load().unwrap();
        let mut t = Tile::new(Terrain::Floor);
        t.feature = Some(Feature::Furniture(Furniture::Table));
        let lit_table = cell(&t, &glow_ctx(&s, Some(Rgb(230, 160, 90))));
        assert!(
            lit_table.fg.0 as i32 > lit_table.fg.2 as i32 + 60,
            "{:?}",
            lit_table.fg
        );
        let dim = cell(&t, &glow_ctx(&s, Some(Rgb(90, 60, 40))));
        assert!((dim.fg.0 as i32 - dim.fg.2 as i32) < 40, "{:?}", dim.fg);
    }

    #[test]
    fn remembered_glow_tiles_still_dim() {
        let s = StylePack::load().unwrap();
        let c = cell(
            &Tile::new(Terrain::Floor),
            &glow_ctx(&s, Some(Rgb(200, 130, 70))),
        );
        let r = apply_vis(c, Visibility::Remembered);
        assert_eq!(r.bg, c.bg.scale(0.35));
        assert!(r.dim);
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
            lamps_on: true,
            sky: None,
            glow: None,
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
