//! The places' beacons on the overworld, round 2's: the relay mast blinks green-white
//! every 2 s, with a halo after dark; after dark the checkpoint's cross flashes red and
//! pale and its road glows red two tiles each way (by day only its pulse); the truck
//! stop's sign dips now and then, and after dark lights its tile and a faint ring; the
//! water plant pumps; a camp's fire flickers, glowing after dark; the wreck sparks.
//! Nothing lights the roads but a checkpoint its own.

use super::live::{Hour, hash2, hash3, key, quant};
use crate::anim::effects::pulse;
use crate::render::{Cell, Rgb};
use citygen::overworld::{LINK_E, LINK_N, LINK_S, LINK_W, Overworld, PoiKind};
use citygen::{Grid, Pos};
use std::collections::HashMap;

const SIGNAL: Rgb = Rgb(0, 255, 200);
const RED: Rgb = Rgb(255, 40, 72);
const AMBER: Rgb = Rgb(255, 176, 0);
const GLOW: Rgb = Rgb(255, 100, 30);
const FIRE: [Rgb; 4] = [
    Rgb(255, 128, 64),
    Rgb(255, 96, 40),
    Rgb(255, 160, 80),
    Rgb(230, 80, 30),
];
/// The relay's blink: 6 ticks in 60.
const RELAY_EVERY: u64 = 60;
const RELAY_ON: u64 = 6;
/// The checkpoint's cross flashes every 12 ticks; its pulse is 40.
const CROSS_TICKS: u64 = 12;
const CHECK_PULSE: u64 = 40;
/// The truck stop's sign dips 10 ticks in 200.
const SIGN_EVERY: u64 = 200;
const SIGN_DIP: u64 = 10;
/// The water plant's pump, 90 ticks.
const PUMP: u64 = 90;
/// The camp's fire changes every 4 ticks.
const FIRE_SHIFT: u32 = 2;
/// The wreck's spark: 2 ticks in 210.
const SPARK_EVERY: u64 = 210;
const SPARK_ON: u64 = 2;

/// How a place's light falls on a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Touch {
    /// The place itself.
    Own,
    /// One of the eight tiles round it, off the roads.
    Ring,
    /// Its road, this many tiles along.
    Road(u8),
}

pub(super) struct Beacons {
    /// For each tile a place's light falls on, 1 + its index into `lists`; 0 for none.
    at: Grid<u16>,
    /// The places whose light falls on a tile, in the world's order, and how.
    lists: Vec<Vec<(u16, Touch)>>,
    /// Hash keys off the world's seed: each place's phase, a camp's fire.
    phase: u64,
    fire: u64,
}

impl Beacons {
    pub(super) fn new(world: &Overworld) -> Beacons {
        let mut touches: HashMap<Pos, Vec<(u16, Touch)>> = HashMap::new();
        for (i, poi) in world.pois.iter().enumerate() {
            let i = i as u16;
            touches.entry(poi.at).or_default().push((i, Touch::Own));
            match poi.kind {
                PoiKind::Relay | PoiKind::TruckStop | PoiKind::Camp => {
                    for q in poi.at.neighbors8() {
                        if world.tile(q).is_some_and(|t| t.road == 0) {
                            touches.entry(q).or_default().push((i, Touch::Ring));
                        }
                    }
                }
                PoiKind::Checkpoint => {
                    for (q, d) in along_road(world, poi.at) {
                        touches.entry(q).or_default().push((i, Touch::Road(d)));
                    }
                }
                _ => {}
            }
        }
        let mut at = Grid::filled(world.size, 0u16);
        let mut lists = Vec::new();
        for (p, list) in touches {
            lists.push(list);
            at[p] = lists.len() as u16;
        }
        Beacons {
            at,
            lists,
            phase: key(world, 0xBE_12),
            fire: key(world, 0xBE_F1),
        }
    }

    /// The places' light on tile `p` at `tick`.
    pub(super) fn apply(&self, world: &Overworld, p: Pos, tick: u64, hour: Hour, c: &mut Cell) {
        let n = self.at.get(p).copied().unwrap_or(0);
        if n == 0 {
            return;
        }
        let dark = hour.dark();
        for &(i, touch) in &self.lists[n as usize - 1] {
            let poi = &world.pois[i as usize];
            let (x, y) = (poi.at.x as i64, poi.at.y as i64);
            let h = hash2(self.phase, x, y);
            match (poi.kind, touch) {
                (PoiKind::Relay, touch) => {
                    let on = (tick + h % RELAY_EVERY) % RELAY_EVERY < RELAY_ON;
                    match touch {
                        Touch::Own if on => {
                            c.fg = Rgb(210, 255, 245);
                            c.bg = c.bg.mix(SIGNAL, 0.28);
                        }
                        Touch::Own => c.fg = c.fg.scale(if dark { 0.55 } else { 0.8 }),
                        Touch::Ring if on && dark => c.bg = c.bg.mix(SIGNAL, 0.08),
                        _ => {}
                    }
                }
                (PoiKind::Checkpoint, Touch::Own) => {
                    let pulse = quant(pulse(tick, CHECK_PULSE), 4.0);
                    c.fg = match (dark, (tick / CROSS_TICKS) % 2 == 1) {
                        (true, true) => RED,
                        (true, false) => Rgb(255, 170, 185),
                        (false, _) => c.fg.scale(0.75 + 0.25 * pulse),
                    };
                }
                (PoiKind::Checkpoint, Touch::Road(d)) if dark => {
                    let g = 0.10 + 0.08 * quant(pulse(tick, CHECK_PULSE), 4.0);
                    c.bg = c.bg.mix(RED, g / d as f32);
                }
                (PoiKind::TruckStop, Touch::Own) => {
                    if (tick + h % SIGN_EVERY) % SIGN_EVERY < SIGN_DIP && tick % 2 == 1 {
                        c.fg = c.fg.scale(0.3);
                    }
                    if dark {
                        c.bg = Rgb(56, 38, 6);
                    }
                }
                (PoiKind::TruckStop, Touch::Ring) if dark => c.bg = c.bg.mix(AMBER, 0.07),
                (PoiKind::WaterPlant, Touch::Own) => {
                    c.fg = c.fg.scale(0.6 + 0.4 * quant(pulse(tick, PUMP), 4.0));
                }
                (PoiKind::Camp, Touch::Own) => {
                    let fire =
                        FIRE[(hash3(self.fire, (tick >> FIRE_SHIFT) as i64, x, y) % 4) as usize];
                    if dark {
                        c.fg = fire;
                        c.bg = c.bg.mix(GLOW, 0.16);
                    } else {
                        c.fg = fire.mix(FIRE[0], 0.6);
                    }
                }
                (PoiKind::Camp, Touch::Ring) if dark => c.bg = c.bg.mix(GLOW, 0.06),
                (PoiKind::Wreck, Touch::Own)
                    if (tick + h % SPARK_EVERY) % SPARK_EVERY < SPARK_ON =>
                {
                    c.fg = Rgb(255, 255, 255);
                }
                _ => {}
            }
        }
    }
}

/// The road through `at`, two tiles each way along its links: each tile and how far.
fn along_road(world: &Overworld, at: Pos) -> Vec<(Pos, u8)> {
    const WAYS: [(u8, u8, i32, i32); 4] = [
        (LINK_N, LINK_S, 0, -1),
        (LINK_E, LINK_W, 1, 0),
        (LINK_S, LINK_N, 0, 1),
        (LINK_W, LINK_E, -1, 0),
    ];
    let links = |p: Pos| world.tile(p).map_or(0, |t| t.road);
    let mut out = Vec::new();
    for &(first, ..) in &WAYS {
        if links(at) & first == 0 {
            continue;
        }
        let (mut p, mut way) = (at, first);
        for d in 1..=2 {
            let &(_, back, dx, dy) = WAYS.iter().find(|w| w.0 == way).unwrap();
            let q = p.offset(dx, dy);
            if links(q) == 0 {
                break;
            }
            out.push((q, d));
            let Some(&(next, ..)) = WAYS.iter().find(|w| links(q) & w.0 != 0 && w.0 != back) else {
                break;
            };
            (p, way) = (q, next);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::overworld::live::tests::world;

    #[test]
    fn a_checkpoint_lights_its_road_both_ways() {
        let w = world();
        let poi = w
            .pois
            .iter()
            .find(|p| p.kind == PoiKind::Checkpoint)
            .unwrap();
        let road = along_road(w, poi.at);
        assert_eq!(road.iter().filter(|(_, d)| *d == 1).count(), 2, "{road:?}");
        assert_eq!(road.iter().filter(|(_, d)| *d == 2).count(), 2, "{road:?}");
        for (q, d) in road {
            assert!(w.tile(q).unwrap().road != 0);
            assert!(q.manhattan(poi.at) <= d as i32);
        }
    }
}
