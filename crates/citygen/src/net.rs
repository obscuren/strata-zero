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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub anchor: (StratumKind, Pos),
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

    pub fn subnet(&self, id: SubnetId) -> &Subnet {
        &self.subnets[id.index()]
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }
}
