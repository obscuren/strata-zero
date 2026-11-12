//! The Net in the sidebar's minimap: the open Net close up in its tier colours, and a
//! construct's plan with its rooms, spine, watch, vault and ICE.

use super::{AMBER, ROOM, Tier, VOID, WHITE, inside, security_of};
use crate::render::Rgb;
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use citygen::{
    BuildingId, City, Construct, Feature, IceGrade, NodeKind, Pos, Rect, StratumKind, Terrain,
};
use std::collections::HashMap;

/// The `s` × `s` pixel at tile (`x0`, `y0`) of the open Net: a lane, a construct's
/// outline in its tier (amber when tracked), its inside dark in it, a hub, else the void
/// with its district's accent; none off the Net.
pub fn net_px(
    city: &City,
    style: &StylePack,
    x0: i32,
    y0: i32,
    s: i32,
    tracked: Option<BuildingId>,
) -> Option<Rgb> {
    let stratum = city.stratum(StratumKind::Net)?;
    net_tiles_px(
        &|p| stratum.tiles.get(p),
        Some(city),
        style,
        x0,
        y0,
        s,
        tracked,
    )
}

/// `net_px` over any of the Net's tiles: `tile` says what lies at each, `city` names
/// their buildings and districts (none on the open Net, which has neither).
pub fn net_tiles_px<'t>(
    tile: &dyn Fn(Pos) -> Option<&'t citygen::Tile>,
    city: Option<&City>,
    style: &StylePack,
    x0: i32,
    y0: i32,
    s: i32,
    tracked: Option<BuildingId>,
) -> Option<Rgb> {
    let colour = |b: BuildingId| {
        if tracked == Some(b) {
            AMBER
        } else {
            Tier::of(city.and_then(|c| security_of(c, b)).unwrap_or(0)).colour()
        }
    };
    let (mut wall, mut lane, mut hub, mut floor, mut district, mut any) =
        (None, false, false, None, None, false);
    for y in y0..y0 + s {
        for x in x0..x0 + s {
            let Some(t) = tile(Pos::new(x, y)) else {
                continue;
            };
            any = true;
            district = district.or(t.district);
            match (t.terrain, t.building) {
                (Terrain::NetWall | Terrain::NetGate, Some(b)) => wall = wall.or(Some(b)),
                (Terrain::NetWall | Terrain::NetGate, None) => hub = true,
                (Terrain::NetLane, _) => lane = true,
                (Terrain::NetFloor, Some(b)) => floor = floor.or(Some(b)),
                _ => {}
            }
        }
    }
    if !any {
        return None;
    }
    Some(if lane {
        SIGNAL.scale(0.6)
    } else if let (Some(b), None) = (wall, floor) {
        colour(b).scale(0.55)
    } else if let Some(b) = floor {
        colour(b).scale(0.2)
    } else if hub {
        WHITE
    } else {
        let theme = district
            .and_then(|d| city?.district_opt(d))
            .map(|d| d.theme);
        VOID.mix(style.theme_or_neutral(theme).accent, 0.12)
    })
}

/// A tile of a construct's plan, with how much it matters when several share a pixel:
/// ICE in its colour, the watched spine tinted, the spine, nodes white (the vault
/// amber), the vault room amber, the other rooms cyan, walls facing rooms brighter
/// than the body; none off the construct.
fn plan_tile(k: &Construct, watch: &HashMap<Pos, IceGrade>, p: Pos) -> Option<(u8, Rgb)> {
    let t = k.tiles.get(p)?;
    let body = Tier::of(k.security).colour();
    if let Some(Feature::Ice(g)) = t.feature {
        return Some((9, inside::watch_tint(g).0));
    }
    if let Some(g) = watch.get(&p) {
        let (tint, _) = inside::watch_tint(*g);
        return Some((8, SIGNAL.scale(0.5).mix(tint, 0.6)));
    }
    let in_vault = k.vault_room().is_some_and(|r| r.contains(p));
    Some(match (t.terrain, t.feature) {
        (Terrain::NetLane | Terrain::NetGate, _) => (7, SIGNAL.scale(0.8)),
        (_, Some(Feature::Node(NodeKind::DataVault))) => (6, AMBER),
        (_, Some(Feature::Node(_))) => (5, WHITE),
        (Terrain::NetWall, _) if inside::edge(k, p) => (2, body.scale(0.5)),
        (Terrain::NetWall, _) => (1, body.scale(0.25)),
        _ if in_vault => (4, AMBER.scale(0.45)),
        _ => (3, ROOM.scale(0.3)),
    })
}

/// The `s` × `s` pixel of a construct's plan at tile (`x0`, `y0`): the tile that
/// matters most in it, so a piece, a node or the spine never drops out when the plan
/// is drawn small; none where the block is off the construct, or off `within` (the tiles
/// drawn: a site's open level).
pub fn plan_px(
    k: &Construct,
    watch: &HashMap<Pos, IceGrade>,
    within: Rect,
    x0: i32,
    y0: i32,
    s: i32,
) -> Option<Rgb> {
    (y0..y0 + s)
        .flat_map(|y| (x0..x0 + s).map(move |x| Pos::new(x, y)))
        .filter(|p| within.contains(*p))
        .filter_map(|p| plan_tile(k, watch, p))
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};

    fn rgb(c: Rgb) -> (u8, u8, u8) {
        (c.0, c.1, c.2)
    }

    #[test]
    fn the_net_close_up_colours_by_tier() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let style = StylePack::load().unwrap();
        let s = c.stratum(StratumKind::Net).unwrap();
        let black_wall = s
            .tiles
            .iter()
            .find(|(_, t)| {
                t.terrain == Terrain::NetWall
                    && t.building
                        .is_some_and(|b| security_of(&c, b).is_some_and(|x| x >= 230))
            })
            .map(|(p, _)| p)
            .expect("a black construct's wall");
        let (r, g, _) = rgb(net_px(&c, &style, black_wall.x, black_wall.y, 1, None).unwrap());
        assert!(r > g, "red over a black wall");
        let lane = s
            .tiles
            .iter()
            .find(|(_, t)| t.terrain == Terrain::NetLane)
            .map(|(p, _)| p)
            .unwrap();
        assert_eq!(
            net_px(&c, &style, lane.x, lane.y, 1, None),
            Some(SIGNAL.scale(0.6))
        );
        let b = c.stratum(StratumKind::Net).unwrap().tiles[black_wall].building;
        assert_eq!(
            net_px(&c, &style, black_wall.x, black_wall.y, 1, b),
            Some(AMBER.scale(0.55)),
            "tracked"
        );
        assert_eq!(net_px(&c, &style, -10, -10, 3, None), None);
    }

    #[test]
    fn the_construct_plan_shows_ice_and_the_vault() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let k = c
            .buildings
            .iter()
            .filter_map(|b| citygen::construct::generate_construct(&c, b.id))
            .find(|k| k.vault_room().is_some() && k.guards.len() == 2)
            .expect("a construct with a vault guard");
        let watch = crate::net::watched(&k);
        let all = k.size.rect();
        let g = &k.guards[1];
        assert_eq!(
            plan_px(&k, &watch, all, g.pos.x, g.pos.y, 1),
            Some(inside::watch_tint(g.grade).0)
        );
        // Drawn small, the piece still shows in its block.
        assert_eq!(
            plan_px(&k, &watch, all, g.pos.x - 1, g.pos.y - 1, 3),
            Some(inside::watch_tint(g.grade).0)
        );
        let vault = k
            .nodes
            .iter()
            .find(|(_, p)| k.tiles[*p].feature == Some(Feature::Node(NodeKind::DataVault)))
            .map(|(_, p)| *p)
            .unwrap();
        assert_eq!(plan_px(&k, &watch, all, vault.x, vault.y, 1), Some(AMBER));
        assert_eq!(plan_px(&k, &watch, all, -3, 0, 2), None);
    }
}
