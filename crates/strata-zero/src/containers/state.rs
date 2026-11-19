//! The saved shape of a container's contents and what became of it.

use game::ItemId;
use serde::{Deserialize, Serialize};

/// One thing in a container.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Loot {
    /// Loose cash.
    Cash(u32),
    /// A credchip with a sum on it, and whether it is locked.
    Chip { credits: u32, locked: bool },
    /// A black credchip with a sum on it.
    BlackChip(u32),
    /// Items; also a fixed item a job names.
    Item { item: ItemId, qty: u32 },
    /// The building's card for zone `z`.
    Card(u8),
}

/// How a container was opened. Grows at its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Opened {
    /// It had no lock.
    Free,
    /// The keycard of its zone.
    Card,
    /// The Keyjack.
    Keyjack,
    /// A crack from the Net, or a jack-in.
    Net,
    /// Forced.
    Forced,
}

/// What became of one container. An untouched container has none: it rolls from its seed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerState {
    /// How it was opened, once it has been.
    #[serde(default)]
    pub opened: Option<Opened>,
    /// What is left in it: rolled at opening, less what has been taken.
    #[serde(default)]
    pub left: Option<Vec<Loot>>,
    /// The faults a Keyjack has run up on its lock.
    #[serde(default)]
    pub faults: u8,
    /// The game minute the Keyjack's lockout ends.
    #[serde(default)]
    pub locked_until: Option<u64>,
}
