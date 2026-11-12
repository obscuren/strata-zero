//! The Net: subnets, nodes and links anchored to physical tiles.

use crate::geom::Pos;
use crate::ids::{BuildingId, DistrictId, NodeId, SubnetId};
use crate::world::StratumKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Net {
    pub subnets: Vec<Subnet>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
}

/// Undirected link between two nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Link(pub NodeId, pub NodeId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubnetOwner {
    Building(BuildingId),
    District(DistrictId),
    City,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subnet {
    pub id: SubnetId,
    pub owner: SubnetOwner,
    pub security: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    Terminal,
    Camera,
    DoorLock,
    LiftCtrl,
    AdBoard,
    LampGrid,
    Server,
    DataVault,
    Ice,
    Backbone,
    GateSwitch,
    Turret,
    Container,
    Hub,
    Firewall,
}

impl NodeKind {
    pub fn label(self) -> &'static str {
        match self {
            NodeKind::Terminal => "terminal",
            NodeKind::Camera => "camera",
            NodeKind::DoorLock => "door lock",
            NodeKind::LiftCtrl => "lift control",
            NodeKind::AdBoard => "ad board",
            NodeKind::LampGrid => "lamp grid",
            NodeKind::Server => "server",
            NodeKind::DataVault => "data vault",
            NodeKind::Ice => "ICE",
            NodeKind::Backbone => "backbone",
            NodeKind::GateSwitch => "gate switch",
            NodeKind::Turret => "turret",
            NodeKind::Container => "container lock",
            NodeKind::Hub => "hub",
            NodeKind::Firewall => "firewall lock",
        }
    }
}

/// A node of a site's own Net, by place: its building, its level and its device's tile
/// on that floor. A firewall lock stands on no floor: level `FIREWALL`, at `(lock, 0)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SiteNode {
    pub building: BuildingId,
    pub level: i32,
    pub at: Pos,
}

impl SiteNode {
    pub const FIREWALL: i32 = i32::MIN;

    pub fn firewall(building: BuildingId, lock: u8) -> SiteNode {
        SiteNode {
            building,
            level: Self::FIREWALL,
            at: Pos::new(lock as i32, 0),
        }
    }

    /// Which of the building's firewall locks this is, if it is one.
    pub fn firewall_lock(self) -> Option<u8> {
        (self.level == Self::FIREWALL).then_some(self.at.x as u8)
    }
}

/// A node of the city's Net or of a site's. Reads a bare `NodeId` (what saves held
/// before) as `City`; writes the tagged shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(from = "NetNodeRead")]
pub enum NetNode {
    City(NodeId),
    Site(SiteNode),
}

impl NetNode {
    pub fn city(self) -> Option<NodeId> {
        match self {
            NetNode::City(id) => Some(id),
            NetNode::Site(_) => None,
        }
    }

    pub fn site(self) -> Option<SiteNode> {
        match self {
            NetNode::Site(n) => Some(n),
            NetNode::City(_) => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NetNodeRead {
    Bare(NodeId),
    Tagged(NetNodeShape),
}

#[derive(Deserialize)]
enum NetNodeShape {
    City(NodeId),
    Site(SiteNode),
}

impl From<NetNodeRead> for NetNode {
    fn from(r: NetNodeRead) -> NetNode {
        match r {
            NetNodeRead::Bare(id) | NetNodeRead::Tagged(NetNodeShape::City(id)) => {
                NetNode::City(id)
            }
            NetNodeRead::Tagged(NetNodeShape::Site(n)) => NetNode::Site(n),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub anchor: (StratumKind, Pos),
    /// The node's tile on the Net stratum.
    pub cyber: Pos,
    pub subnet: SubnetId,
    pub building: Option<BuildingId>,
}

impl Net {
    pub fn nodes_on(&self, k: StratumKind) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.anchor.0 == k)
    }

    pub fn node_at(&self, k: StratumKind, p: Pos) -> Option<&Node> {
        self.nodes.iter().find(|n| n.anchor == (k, p))
    }

    /// The node standing on a cyberspace tile.
    pub fn node_in_cyber(&self, p: Pos) -> Option<&Node> {
        self.nodes.iter().find(|n| n.cyber == p)
    }

    pub fn subnet(&self, id: SubnetId) -> &Subnet {
        &self.subnets[id.index()]
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_are_found_by_cyber_tile() {
        let mut net = Net::default();
        net.subnets.push(Subnet {
            id: SubnetId(0),
            owner: SubnetOwner::City,
            security: 10,
        });
        net.nodes.push(Node {
            id: NodeId(0),
            kind: NodeKind::Server,
            anchor: (StratumKind::Street, Pos::new(5, 5)),
            cyber: Pos::new(7, 5),
            subnet: SubnetId(0),
            building: None,
        });
        assert_eq!(
            net.node_in_cyber(Pos::new(7, 5)).map(|n| n.id),
            Some(NodeId(0))
        );
        assert_eq!(net.node_in_cyber(Pos::new(5, 5)), None);
        assert!(net.node_at(StratumKind::Street, Pos::new(5, 5)).is_some());
    }

    #[test]
    fn a_net_node_reads_a_bare_node_id_and_round_trips() {
        let bare = rmp_serde::to_vec_named(&NodeId(42)).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<NetNode>(&bare).unwrap(),
            NetNode::City(NodeId(42))
        );
        let site = SiteNode {
            building: BuildingId(5),
            level: -1,
            at: Pos::new(4, 7),
        };
        for n in [
            NetNode::City(NodeId(3)),
            NetNode::Site(site),
            NetNode::Site(SiteNode::firewall(BuildingId(5), 2)),
        ] {
            let bytes = rmp_serde::to_vec_named(&n).unwrap();
            assert_eq!(rmp_serde::from_slice::<NetNode>(&bytes).unwrap(), n);
        }
    }

    #[test]
    fn a_firewall_lock_is_a_site_node_of_its_own() {
        let n = SiteNode::firewall(BuildingId(5), 2);
        assert_eq!((n.level, n.firewall_lock()), (SiteNode::FIREWALL, Some(2)));
        let g = SiteNode {
            building: BuildingId(5),
            level: 0,
            at: Pos::new(2, 0),
        };
        assert_eq!(g.firewall_lock(), None);
        assert_eq!(NodeKind::Hub.label(), "hub");
        assert_eq!(NodeKind::Firewall.label(), "firewall lock");
    }
}
