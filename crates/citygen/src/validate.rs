//! City invariants.

use crate::generate::connectivity::reachable_from_street;
use crate::geom::Pos;
use crate::ids::{BuildingId, DistrictId, NodeId};
use crate::net::NodeKind;
use crate::tile::{Feature, Terrain};
use crate::world::{City, StratumKind};
use std::fmt;

/// Share of walkable tiles per stratum that must be reachable from the street.
pub const REACH_TARGET: f32 = 0.95;

/// A broken invariant found by [`City::validate`].
#[derive(Clone, Debug, PartialEq)]
pub enum Violation {
    ConnectorNotWalkable {
        pos: Pos,
        stratum: StratumKind,
    },
    BuildingWithoutDoor(BuildingId),
    StratumUnreachable {
        stratum: StratumKind,
        reached: usize,
        walkable: usize,
    },
    DanglingId {
        pos: Pos,
        stratum: StratumKind,
    },
    FootprintOverlap(BuildingId, BuildingId),
    NetAnchor(NodeId),
    DistrictEmpty(DistrictId),
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::ConnectorNotWalkable { pos, stratum } => {
                write!(f, "connector at {pos:?} is not walkable on {stratum:?}")
            }
            Violation::BuildingWithoutDoor(b) => write!(f, "building {b:?} has no street door"),
            Violation::StratumUnreachable {
                stratum,
                reached,
                walkable,
            } => write!(
                f,
                "{stratum:?}: only {reached}/{walkable} walkable tiles reachable from the street"
            ),
            Violation::DanglingId { pos, stratum } => {
                write!(
                    f,
                    "tile {pos:?} on {stratum:?} references a missing district or building"
                )
            }
            Violation::FootprintOverlap(a, b) => write!(f, "buildings {a:?} and {b:?} overlap"),
            Violation::NetAnchor(n) => write!(f, "net node {n:?} is anchored to the wrong tile"),
            Violation::DistrictEmpty(d) => {
                write!(f, "district {d:?} lacks a building or a public connector")
            }
        }
    }
}

impl City {
    /// Checks the invariants every finished city must satisfy.
    pub fn validate(&self) -> Result<(), Vec<Violation>> {
        let mut out = Vec::new();

        for c in &self.connectors {
            for k in &c.serves {
                if *k != StratumKind::Net && !self.walkable(*k, c.pos) {
                    out.push(Violation::ConnectorNotWalkable {
                        pos: c.pos,
                        stratum: *k,
                    });
                }
            }
        }

        for b in &self.buildings {
            let has_door = b.footprint.border().any(|p| {
                self.tile(StratumKind::Street, p)
                    .is_some_and(|t| t.terrain == Terrain::Door)
            });
            if !has_door {
                out.push(Violation::BuildingWithoutDoor(b.id));
            }
        }

        let reach = reachable_from_street(self);
        for k in StratumKind::PHYSICAL {
            let Some(st) = self.stratum(k) else { continue };
            let walkable = st.tiles.count(|t| t.is_walkable());
            if walkable == 0 {
                continue;
            }
            let reached = reach[k.index()].count(|&b| b);
            if (reached as f32) < REACH_TARGET * walkable as f32 {
                out.push(Violation::StratumUnreachable {
                    stratum: k,
                    reached,
                    walkable,
                });
            }
        }

        for st in &self.strata {
            for (p, t) in st.tiles.iter() {
                let bad_d = t
                    .district
                    .is_some_and(|d| d.index() >= self.districts.len());
                let bad_b = t
                    .building
                    .is_some_and(|b| b.index() >= self.buildings.len());
                if bad_d || bad_b {
                    out.push(Violation::DanglingId {
                        pos: p,
                        stratum: st.kind,
                    });
                }
            }
        }

        for (i, a) in self.buildings.iter().enumerate() {
            for b in &self.buildings[i + 1..] {
                if a.footprint.intersects(b.footprint) {
                    out.push(Violation::FootprintOverlap(a.id, b.id));
                }
            }
        }

        for n in &self.net.nodes {
            let ok = match self.tile(n.anchor.0, n.anchor.1) {
                None => false,
                Some(t) => match n.kind {
                    NodeKind::Terminal => t.feature == Some(Feature::Terminal),
                    NodeKind::Camera => t.feature == Some(Feature::Camera),
                    NodeKind::AdBoard => t.feature == Some(Feature::HoloAd),
                    NodeKind::LampGrid => t.feature == Some(Feature::Lamp),
                    NodeKind::DoorLock => t.terrain == Terrain::Door,
                    NodeKind::LiftCtrl => t.terrain == Terrain::Lift,
                    NodeKind::Server | NodeKind::DataVault | NodeKind::Ice => {
                        matches!(t.terrain, Terrain::Floor | Terrain::Lift | Terrain::Stair)
                    }
                    NodeKind::Backbone => true,
                },
            };
            if !ok || n.subnet.index() >= self.net.subnets.len() {
                out.push(Violation::NetAnchor(n.id));
            }
        }

        for link in &self.net.links {
            for id in [link.0, link.1] {
                if id.index() >= self.net.nodes.len() {
                    out.push(Violation::NetAnchor(id));
                }
            }
        }

        for d in &self.districts {
            let has_building = self.buildings.iter().any(|b| b.district == d.id);
            let has_public = self.connectors.iter().any(|c| {
                c.public
                    && self
                        .tile(StratumKind::Street, c.pos)
                        .is_some_and(|t| t.district == Some(d.id))
            });
            if !has_building || !has_public {
                out.push(Violation::DistrictEmpty(d.id));
            }
        }

        if out.is_empty() { Ok(()) } else { Err(out) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Pos, Rect};
    use crate::ids::{BuildingId, DistrictId};
    use crate::params::{GenParams, SizePreset};
    use crate::tile::{Terrain, Tile};
    use crate::world::{
        Building, BuildingKind, Connector, ConnectorKind, District, HeightClass, StrataSpan,
        StratumKind, Theme,
    };

    fn bare_city() -> City {
        let mut c = City::empty(GenParams {
            size: SizePreset::Custom { w: 80, h: 40 },
            ..Default::default()
        });
        c.districts.push(District {
            id: DistrictId(0),
            name: "x".into(),
            theme: Theme::OldTown,
            center: Pos::new(10, 10),
            wealth: 0.5,
            decay: 0.5,
            bounds: Rect::new(0, 0, 80, 40),
        });
        let st = c.stratum_mut(StratumKind::Street).unwrap();
        st.tiles
            .fill_rect(Rect::new(0, 0, 80, 40), Tile::new(Terrain::Sidewalk));
        c
    }

    #[test]
    fn detects_building_without_door_and_blocked_connector() {
        let mut c = bare_city();
        let fp = Rect::new(5, 5, 6, 5);
        let st = c.stratum_mut(StratumKind::Street).unwrap();
        for p in fp.border() {
            st.tiles.set(p, Tile::new(Terrain::Wall));
        }
        c.buildings.push(Building {
            id: BuildingId(0),
            kind: BuildingKind::Shop,
            district: DistrictId(0),
            footprint: fp,
            span: StrataSpan::from_class(HeightClass::Low, false),
            name: None,
        });
        c.connectors.push(Connector {
            kind: ConnectorKind::Manhole,
            pos: Pos::new(20, 20),
            serves: vec![StratumKind::Undercity, StratumKind::Street],
            public: true,
        });
        let errs = c.validate().unwrap_err();
        assert!(
            errs.iter()
                .any(|v| matches!(v, Violation::BuildingWithoutDoor(BuildingId(0))))
        );
        assert!(errs.iter().any(|v| matches!(
            v,
            Violation::ConnectorNotWalkable {
                stratum: StratumKind::Undercity,
                ..
            }
        )));
    }

    #[test]
    fn detects_overlap_and_empty_district() {
        let mut c = bare_city();
        for (i, fp) in [Rect::new(5, 5, 6, 5), Rect::new(8, 7, 6, 5)]
            .iter()
            .enumerate()
        {
            let st = c.stratum_mut(StratumKind::Street).unwrap();
            for p in fp.border() {
                st.tiles.set(p, Tile::new(Terrain::Wall));
            }
            st.tiles
                .set(Pos::new(fp.x + 2, fp.y), Tile::new(Terrain::Door));
            c.buildings.push(Building {
                id: BuildingId(i as u32),
                kind: BuildingKind::Shop,
                district: DistrictId(0),
                footprint: *fp,
                span: StrataSpan::from_class(HeightClass::Low, false),
                name: None,
            });
        }
        let errs = c.validate().unwrap_err();
        assert!(
            errs.iter()
                .any(|v| matches!(v, Violation::FootprintOverlap(..)))
        );
        assert!(
            errs.iter()
                .any(|v| matches!(v, Violation::DistrictEmpty(DistrictId(0))))
        );
        assert!(format!("{}", errs[0]).len() > 5);
    }
}
