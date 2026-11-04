//! Cameras as eyes in the meat. A building's street camera faces out from its facade
//! and sees a cone eight tiles deep; standing in a live camera's cone, near it or lit,
//! you count as an eye and its building's trace warms, up to the alarm line. A camera
//! held from the Net can be looped (blind until the hold ends) or tapped (its feed on
//! the Net). Spec `docs/superpowers/specs/2026-11-04-net-nodes-design.md`, "Camera".

use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use citygen::{BuildingId, City, Dir, NodeId, NodeKind, Place, Pos, StratumKind, sight};

/// How deep a camera's cone reaches.
pub const CONE_DEPTH: i32 = 8;
/// How near a camera sees you in the dark.
pub const DARK_REACH: i32 = 3;
/// Heat a camera that sees you puts on its building's trace each game minute.
pub const EYE_HEAT: u8 = 2;

/// A street camera: its node, its building, its tile on the street and which way it
/// faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camera {
    pub node: NodeId,
    pub building: BuildingId,
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
                node: n.id,
                building: b,
                at: n.anchor.1,
                facing: facing(city, b, n.anchor.1)?,
            })
        })
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
    let blocks = |p: Pos| {
        city.tile(StratumKind::Street, p)
            .is_none_or(|t| t.blocks_sight())
    };
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

/// The live cameras that see you on foot on the street now; none jacked in (your
/// body at a port is not watched) or anywhere else.
pub fn seeing(st: &AppState) -> Vec<Camera> {
    let (Some(city), Some(p)) = (st.city.as_ref(), st.player.as_ref()) else {
        return Vec::new();
    };
    if p.place != Place::Stratum(StratumKind::Street) {
        return Vec::new();
    }
    let now = st.net.now(st);
    let map = MapRef::stratum_at(city, StratumKind::Street, st.minute_of_day());
    street_cameras(city)
        .filter(|c| c.at.chebyshev(p.pos) <= CONE_DEPTH)
        .filter(|c| !st.net.blinded(c.node, now))
        .filter(|c| sees(&map, c, p.pos))
        .collect()
}

/// A game minute on the street: each camera that sees you warms its building's trace,
/// never past the alarm line (a camera can lock a building down, not dump you).
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

/// How a camera's cone draws on the street.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConeLook {
    /// It sees you: red.
    Seeing,
    /// Looped: dotted grey.
    Looped,
    /// Held and tapped: faint green.
    Tapped,
}

/// The cones to draw on the street now, with how each looks.
pub fn cones(st: &AppState) -> Vec<(Vec<Pos>, ConeLook)> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    let now = st.net.now(st);
    let seeing = seeing(st);
    street_cameras(city)
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
            Some((cone(city, &c), look))
        })
        .collect()
}

/// The cones as backgrounds on the street map.
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
    use game::HackTarget;

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
}
