//! The saved shape of a container's contents.

use game::ItemId;
use serde::{Deserialize, Serialize};

/// One thing in a container.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Loot {
    /// Loose cash.
    Cash(u32),
    /// A credchip with a sum on it, and whether it is locked.
    Chip { credits: u32, locked: bool },
    /// Items; also a fixed item a job names.
    Item { item: ItemId, qty: u32 },
    /// The building's card for zone `z`.
    Card(u8),
}
