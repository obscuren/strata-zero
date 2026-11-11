//! Cameras as eyes in the meat. A building's street camera faces out from its facade
//! and sees a cone eight tiles deep; standing in a live camera's cone, near it or lit,
//! you count as an eye and its building's trace warms, up to the alarm line. A camera
//! held from the Net can be looped (blind until the hold ends) or tapped (its feed on
//! the Net). Spec `docs/superpowers/specs/2026-11-04-net-nodes-design.md`, "Camera".
//!
//! A mission copy's cameras watch their floors the same way: each faces away from the
//! wall beside its spot and sees the same cone by the floor's own light, warming the
//! site's trace. Spec `docs/superpowers/specs/2026-11-11-site-net-design.md`, "Cameras on
//! floors".

use crate::app::AppState;
use crate::render::map::{FloorMap, MapRef, TileMap};
use citygen::venue::DeviceKind;
use citygen::{
    BuildingId, City, Dir, Floor, NetNode, NodeKind, Place, Pos, SiteNode, StratumKind, Terrain,
    sight,
};

/// How deep a camera's cone reaches.
pub const CONE_DEPTH: i32 = 8;
/// How near a camera sees you in the dark.
pub const DARK_REACH: i32 = 3;
/// Heat a camera that sees you puts on its building's trace each game minute.
pub const EYE_HEAT: u8 = 2;

/// A camera: its node, its building, where it watches (the street, or a site's floor),
/// its tile there and which way it faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camera {
    pub node: NetNode,
    pub building: BuildingId,
    pub place: Place,
    pub at: Pos,
    pub facing: Dir,
}

/// Which way a camera on a building's facade looks: out, toward the first side
/// whose tile is not the building's.
pub fn facing(city: &City, b: BuildingId, at: Pos) -> Option<Dir> {
    Dir::CARDINAL.into_iter().find(|d| {
        let q = at.step(*d);
        city.tile(StratumKind::Street, q)
            .is_some_and(|t| t.building != Some(b) && t.is_walkable())
    })
}

/// Every building camera on the street, facing out.
pub fn street_cameras(city: &City) -> impl Iterator<Item = Camera> + '_ {
    city.net
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Camera && n.anchor.0 == StratumKind::Street)
        .filter_map(|n| {
            let b = n.building?;
            Some(Camera {
                node: NetNode::City(n.id),
                building: b,
                place: Place::Stratum(StratumKind::Street),
                at: n.anchor.1,
                facing: facing(city, b, n.anchor.1)?,
            })
        })
}

/// Which way a camera on a floor looks (Ruling 16): away from the first wall beside its
/// spot, in `Dir::CARDINAL` order; with none, toward the nearest of `locks` (the Net's
/// door locks on its level; manhattan, then y, x) along the axis it lies further on (east
/// or west on a tie); with none, south.
pub fn floor_facing(floor: &Floor, at: Pos, locks: &[Pos]) -> Dir {
    let wall = |p: Pos| {
        floor
            .tiles
            .get(p)
            .is_some_and(|t| matches!(t.terrain, Terrain::Wall | Terrain::Window))
    };
    if let Some(d) = Dir::CARDINAL.into_iter().find(|d| wall(at.step(*d))) {
        return d.opposite();
    }
    let Some(lock) = (locks.iter()).min_by_key(|p| (p.manhattan(at), p.y, p.x)) else {
        return Dir::S;
    };
    let (dx, dy) = (lock.x - at.x, lock.y - at.y);
    match (dx.abs() >= dy.abs(), dx >= 0, dy >= 0) {
        (true, true, _) => Dir::E,
        (true, false, _) => Dir::W,
        (false, _, true) => Dir::S,
        (false, _, false) => Dir::N,
    }
}

/// A live copy's cameras on `level` of `b`, each facing by Ruling 16; none for a building
/// no live copy holds, or a level its plan doesn't keep.
pub fn floor_cameras(st: &AppState, b: BuildingId, level: i32) -> Vec<Camera> {
    let Some(layer) = crate::sitenet::layer_of(st, b) else {
        return Vec::new();
    };
    let spots: Vec<Pos> = (layer.devices.iter())
        .filter(|d| d.level == level && d.kind == DeviceKind::Camera)
        .map(|d| d.at)
        .collect();
    if spots.is_empty() {
        return Vec::new();
    }
    let Some(plan) = crate::interiors::plan_now(st, b) else {
        return Vec::new();
    };
    let Some(floor) = crate::interiors::floor_now(st, &plan, level) else {
        return Vec::new();
    };
    let locks: Vec<Pos> = crate::sitenet::net_of(st, b).map_or_else(Vec::new, |net| {
        (net.nodes.iter())
            .filter(|n| n.kind == NodeKind::DoorLock && n.node.level == level)
            .map(|n| n.node.at)
            .collect()
    });
    let place = Place::Floor { building: b, level };
    (spots.into_iter())
        .map(|at| Camera {
            node: NetNode::Site(SiteNode {
                building: b,
                level,
                at,
            }),
            building: b,
            place,
            at,
            facing: floor_facing(&floor, at, &locks),
        })
        .collect()
}

/// The cameras watching `place`: the street's, or a site's floor's.
fn cameras_at(st: &AppState, place: Place) -> Vec<Camera> {
    match place {
        Place::Stratum(StratumKind::Street) => st
            .city
            .as_ref()
            .map_or_else(Vec::new, |city| street_cameras(city).collect()),
        Place::Floor { building, level } => floor_cameras(st, building, level),
        _ => Vec::new(),
    }
}

/// Whether a tile lies in a camera's cone: up to `CONE_DEPTH` out, and at each step
/// out one more tile to each side.
pub fn in_cone(cam: &Camera, p: Pos) -> bool {
    let (dx, dy) = cam.facing.delta();
    let (ox, oy) = (p.x - cam.at.x, p.y - cam.at.y);
    let depth = ox * dx + oy * dy;
    let side = (ox * dy - oy * dx).abs();
    (1..=CONE_DEPTH).contains(&depth) && side <= depth
}

/// The cone's tiles a camera has a clear line to, on the street.
pub fn cone(city: &City, cam: &Camera) -> Vec<Pos> {
    cone_by(cam, |p| {
        city.tile(StratumKind::Street, p)
            .is_none_or(|t| t.blocks_sight())
    })
}

/// The cone's tiles a camera has a clear line to, on its floor.
pub fn floor_cone(floor: &Floor, cam: &Camera) -> Vec<Pos> {
    cone_by(cam, |p| floor.tiles.get(p).is_none_or(|t| t.blocks_sight()))
}

/// The cone's tiles that `blocks` leaves open, with a clear line from the camera.
fn cone_by(cam: &Camera, blocks: impl Fn(Pos) -> bool) -> Vec<Pos> {
    let r = CONE_DEPTH;
    let mut out = Vec::new();
    for y in cam.at.y - r..=cam.at.y + r {
        for x in cam.at.x - r..=cam.at.x + r {
            let p = Pos::new(x, y);
            if in_cone(cam, p)
                && !blocks(p)
                && citygen::pathing::bresenham(cam.at, p)
                    .into_iter()
                    .filter(|q| *q != cam.at && *q != p)
                    .all(|q| !blocks(q))
            {
                out.push(p);
            }
        }
    }
    out
}

/// Whether a camera sees the tile: in its cone, a clear line, and near or lit.
pub fn sees(map: &MapRef, cam: &Camera, p: Pos) -> bool {
    in_cone(cam, p)
        && sight::sees(
            cam.at,
            p,
            DARK_REACH,
            |q| map.tile(q).is_none_or(|t| t.blocks_sight()),
            |q| map.emitted(q),
            |q| map.emits(q),
        )
}

/// The live cameras that see you on foot now: on the street its cameras, on a site's
/// floor its copy's, each by the light where you stand; none jacked in (your body at a
/// port is not watched) or anywhere else.
pub fn seeing(st: &AppState) -> Vec<Camera> {
    let (Some(city), Some(p)) = (st.city.as_ref(), st.player.as_ref()) else {
        return Vec::new();
    };
    let map = match p.place {
        Place::Stratum(StratumKind::Street) => {
            MapRef::stratum_at(city, StratumKind::Street, st.minute_of_day())
        }
        Place::Floor { building, level } => {
            let Some((plan, floor)) = st.interiors.floor(building, level) else {
                return Vec::new();
            };
            MapRef::Floor(FloorMap { city, plan, floor })
        }
        _ => return Vec::new(),
    };
    let now = st.net.now(st);
    cameras_at(st, p.place)
        .into_iter()
        .filter(|c| c.at.chebyshev(p.pos) <= CONE_DEPTH)
        .filter(|c| !st.net.blinded(c.node, now))
        .filter(|c| sees(&map, c, p.pos))
        .collect()
}

/// A game minute on the street or a site's floor: each camera that sees you warms its
/// building's trace, never past the alarm line (a camera can lock a building down, not
/// dump you).
pub fn tick(st: &mut AppState) {
    let now = st.net.now(st);
    if st.net.eyes_minute.is_some_and(|m| now <= m) {
        return;
    }
    st.net.eyes_minute = Some(now);
    let mut buildings: Vec<BuildingId> = seeing(st).iter().map(|c| c.building).collect();
    buildings.sort();
    buildings.dedup();
    for b in buildings {
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        let room = game::ALARM_HEAT.saturating_sub(heat);
        if room > 0 {
            crate::owners::heat_on(st, crate::owners::Owner::Building(b), EYE_HEAT.min(room));
        }
    }
}

/// How a camera's cone draws on the street or a floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConeLook {
    /// It sees you: red.
    Seeing,
    /// Looped: dotted grey.
    Looped,
    /// Held and tapped: faint green.
    Tapped,
}

/// The cones to draw on `place` (the street, or a site's floor) now, with how each looks.
pub fn cones(st: &AppState, place: Place) -> Vec<(Vec<Pos>, ConeLook)> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    let floor = match place {
        Place::Floor { building, level } => st.interiors.floor(building, level).map(|(_, f)| f),
        _ => None,
    };
    let now = st.net.now(st);
    let seeing = seeing(st);
    cameras_at(st, place)
        .into_iter()
        .filter_map(|c| {
            let look = if seeing.contains(&c) {
                ConeLook::Seeing
            } else if crate::holds::looped(st, c.node, now) {
                ConeLook::Looped
            } else if crate::holds::held(st, c.node, now).is_some() {
                ConeLook::Tapped
            } else {
                return None;
            };
            let tiles = match place {
                Place::Floor { .. } => floor_cone(floor?, &c),
                _ => cone(city, &c),
            };
            Some((tiles, look))
        })
        .collect()
}

/// The cones as backgrounds on the street's or a floor's map.
pub struct ConeOverlay<'a> {
    pub cones: &'a [(Vec<Pos>, ConeLook)],
    pub vis: Option<&'a citygen::Grid<crate::render::Visibility>>,
}

const SEEING_BG: ratatui::style::Color = ratatui::style::Color::Rgb(80, 12, 12);
const LOOPED_BG: ratatui::style::Color = ratatui::style::Color::Rgb(44, 46, 50);
const TAPPED_BG: ratatui::style::Color = ratatui::style::Color::Rgb(10, 40, 18);

impl crate::render::viewport::Overlay for ConeOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: ratatui::layout::Rect,
        _tick: u64,
        buf: &mut ratatui::buffer::Buffer,
        _style: &crate::render::style::StylePack,
    ) {
        for (tiles, look) in self.cones {
            for p in tiles {
                if self
                    .vis
                    .is_some_and(|v| v.get(*p) != Some(&crate::render::Visibility::Visible))
                {
                    continue;
                }
                // A looped cone is dotted: every other tile.
                if *look == ConeLook::Looped && (p.x + p.y) % 2 != 0 {
                    continue;
                }
                if let Some((sx, sy)) = crate::render::viewport::tile_to_screen(area, origin, *p) {
                    buf[(sx, sy)].set_bg(match look {
                        ConeLook::Seeing => SEEING_BG,
                        ConeLook::Looped => LOOPED_BG,
                        ConeLook::Tapped => TAPPED_BG,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::tests::{crack_to_landing, node_cyber, under_kiroflux};
    use crate::sitenet::testutil::{in_a_floor_cameras_cone, next_minute, tiny_room};
    use game::{HackTarget, Op};

    /// Kiroflux Mart's street camera.
    fn kiroflux_cam(st: &AppState) -> Camera {
        let city = st.city.as_ref().unwrap();
        let b = city
            .buildings
            .iter()
            .find(|b| b.name.as_deref() == Some("Kiroflux Mart"))
            .unwrap()
            .id;
        street_cameras(city).find(|c| c.building == b).unwrap()
    }

    /// You on foot on the street at a tile, at night with the lamps on.
    fn on_foot(st: &mut AppState, at: Pos) {
        let p = st.player.as_mut().unwrap();
        p.place = Place::Stratum(StratumKind::Street);
        p.pos = at;
        p.body = None;
    }

    #[test]
    fn a_camera_faces_out_from_its_facade() {
        let (st, b) = under_kiroflux();
        let cam = kiroflux_cam(&st);
        let city = st.city.as_ref().unwrap();
        let out = cam.at.step(cam.facing);
        assert_ne!(
            city.tile(StratumKind::Street, out).unwrap().building,
            Some(b)
        );
        assert!(city.tile(StratumKind::Street, out).unwrap().is_walkable());
    }

    #[test]
    fn a_camera_sees_a_cone() {
        let (st, _) = under_kiroflux();
        let cam = kiroflux_cam(&st);
        let (dx, dy) = cam.facing.delta();
        let ahead = |k: i32| Pos::new(cam.at.x + dx * k, cam.at.y + dy * k);
        assert!(in_cone(&cam, ahead(1)));
        assert!(in_cone(&cam, ahead(CONE_DEPTH)));
        assert!(!in_cone(&cam, ahead(CONE_DEPTH + 1)));
        assert!(!in_cone(&cam, ahead(-1)), "not behind it");
        // Two out, two to the side: in; two out, three to the side: out.
        let side = |k: i32, s: i32| Pos::new(ahead(k).x + dy * s, ahead(k).y + dx * s);
        assert!(in_cone(&cam, side(2, 2)));
        assert!(!in_cone(&cam, side(2, 3)));
        assert!(!cone(st.city.as_ref().unwrap(), &cam).is_empty());
    }

    #[test]
    fn a_camera_sees_you_near_and_a_loop_blinds_it() {
        let (mut st, b) = under_kiroflux();
        let cam = kiroflux_cam(&st);
        let ahead = cam.at.step(cam.facing).step(cam.facing);
        on_foot(&mut st, ahead);
        assert_eq!(seeing(&st), vec![cam], "two tiles out, it sees you");
        assert!(!crate::exposure::of(&st).cameras.is_empty());
        // Hold it from the Net and loop it.
        let (node, at) = node_cyber(&st, b, NodeKind::Camera);
        let p = st.player.as_mut().unwrap();
        p.place = Place::Stratum(StratumKind::Net);
        p.pos = Pos::new(at.x, at.y + 1);
        p.body = Some((Place::Stratum(StratumKind::Street), ahead));
        crack_to_landing(&mut st, HackTarget::Node(node));
        crate::holds::use_verb(&mut st, 'o').unwrap();
        on_foot(&mut st, ahead);
        assert!(seeing(&st).is_empty(), "a looped camera sees nothing");
    }

    #[test]
    fn being_seen_heats_the_building() {
        let (mut st, b) = under_kiroflux();
        let cam = kiroflux_cam(&st);
        on_foot(&mut st, cam.at.step(cam.facing));
        tick(&mut st);
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        assert!(heat > 0 && heat <= EYE_HEAT, "{heat}");
        // At the alarm line it stops.
        st.net.trace(b).heat = game::ALARM_HEAT;
        st.net.eyes_minute = None;
        tick(&mut st);
        assert_eq!(st.net.traces[&b].heat, game::ALARM_HEAT);
    }

    #[test]
    fn jacked_in_your_body_is_not_seen() {
        let (st, _) = under_kiroflux();
        assert!(seeing(&st).is_empty());
    }

    #[test]
    fn a_floor_camera_that_sees_you_warms_the_trace() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let cam = in_a_floor_cameras_cone(&mut st, b); // on its floor, lit, in its cone with a clear line
        assert!(seeing(&st).contains(&cam));
        let before = st.net.traces.get(&b).map_or(0, |t| t.heat);
        next_minute(&mut st);
        tick(&mut st);
        assert_eq!(
            st.net.traces.get(&b).map_or(0, |t| t.heat),
            before + EYE_HEAT
        );
    }

    #[test]
    fn a_looped_or_blinded_floor_camera_sees_nothing() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let cam = in_a_floor_cameras_cone(&mut st, b);
        assert!(seeing(&st).contains(&cam), "it sees you before the blind");
        let n = cam.node.site().unwrap();
        crate::sitenet::testutil::hack(&mut st, n, NodeKind::Camera, Op::Blind);
        assert!(seeing(&st).is_empty());
        let before = st.net.traces.get(&b).map_or(0, |t| t.heat);
        next_minute(&mut st);
        tick(&mut st);
        assert_eq!(st.net.traces.get(&b).map_or(0, |t| t.heat), before);
    }

    #[test]
    fn a_held_floor_cameras_loop_blinds_it() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let cam = in_a_floor_cameras_cone(&mut st, b);
        let n = cam.node.site().unwrap();
        let (place, here) = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        assert!(seeing(&st).contains(&cam));
        // Jack in where you stand, crack the camera from beside its node on its level of
        // the site's Net and loop it.
        crate::sitenet::ensure_construct(&mut st, b).unwrap();
        let at = st.constructs.site(b).unwrap().device(n).unwrap().at;
        st.player.as_mut().unwrap().body = Some((place, here));
        let level = Place::SiteNet {
            building: b,
            level: n.level,
        };
        crate::constructs::land(&mut st, level, at);
        crate::sitenet::testutil::walk_beside(&mut st, at);
        st.net.beat = st.clock_tick();
        crack_to_landing(&mut st, HackTarget::Site(n));
        crate::holds::use_verb(&mut st, 'o').unwrap();
        // Back on foot where you stood: the camera sees nothing, and its cone draws looped.
        st.player.as_mut().unwrap().body = None;
        crate::constructs::land(&mut st, place, here);
        assert!(!seeing(&st).contains(&cam), "a looped camera sees nothing");
        let looped = (st.interiors.floor(b, n.level))
            .map(|(_, floor)| (floor_cone(floor, &cam), ConeLook::Looped));
        assert!(cones(&st, place).contains(&looped.unwrap()));
    }

    #[test]
    fn a_floor_camera_never_warms_past_the_alarm_line() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        in_a_floor_cameras_cone(&mut st, b);
        st.net.trace(b).heat = game::ALARM_HEAT - 1;
        for _ in 0..3 {
            next_minute(&mut st);
            tick(&mut st);
        }
        assert_eq!(st.net.traces[&b].heat, game::ALARM_HEAT);
    }

    #[test]
    fn a_floor_camera_faces_away_from_its_wall() {
        // A 5x5 room of floor walled round: a camera at (1, 2) faces east.
        let floor = tiny_room(5, 5);
        assert_eq!(floor_facing(&floor, Pos::new(1, 2), &[]), Dir::E);
        assert_eq!(floor_facing(&floor, Pos::new(2, 1), &[]), Dir::S);
    }

    #[test]
    fn a_floor_camera_with_no_wall_beside_it_faces_its_nearest_lock_else_south() {
        let floor = tiny_room(9, 9);
        let mid = Pos::new(4, 4);
        assert_eq!(floor_facing(&floor, mid, &[]), Dir::S);
        let locks = [Pos::new(4, 0), Pos::new(7, 4)];
        assert_eq!(floor_facing(&floor, mid, &locks), Dir::E);
        assert_eq!(floor_facing(&floor, mid, &[Pos::new(3, 0)]), Dir::N);
    }

    #[test]
    fn a_floor_cameras_cone_draws_on_its_floor_and_not_the_street() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let cam = in_a_floor_cameras_cone(&mut st, b);
        let here = st.player.as_ref().unwrap().pos;
        assert_eq!(cam.place, st.player.as_ref().unwrap().place);
        let drawn = cones(&st, cam.place);
        assert!(
            (drawn.iter()).any(|(tiles, look)| *look == ConeLook::Seeing && tiles.contains(&here)),
            "{drawn:?}"
        );
        assert!(cones(&st, Place::Stratum(StratumKind::Street)).is_empty());
    }
}
