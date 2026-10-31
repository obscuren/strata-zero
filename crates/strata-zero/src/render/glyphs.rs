//! Glyphs shared between the deck and the map.

use game::ItemKind;

/// One character per item kind, for list rows and ground piles.
pub fn kind_icon(kind: ItemKind) -> char {
    match kind {
        ItemKind::Weapon => '/',
        ItemKind::Armor => '▣',
        ItemKind::Cyberware => '◈',
        ItemKind::Deck => '⌗',
        ItemKind::Consumable => '◍',
        ItemKind::Component => '▪',
        ItemKind::Junk => '·',
        ItemKind::Program => '⌁',
        ItemKind::Hardware => '≡',
        // A square with a corner lifting, as a sticker peels.
        ItemKind::Sticker => '◪',
        // A rotor seen from above, as a drone draws on the map.
        ItemKind::Bot => '┼',
    }
}
