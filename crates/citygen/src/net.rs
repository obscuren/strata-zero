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
}
