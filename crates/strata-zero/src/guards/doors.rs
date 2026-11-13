//! Which doors a foe's card opens.

use crate::foes::Post;
use citygen::venue::Ways;
use citygen::{Floor, Pos, RoomKind};

/// Whether a foe with `post` (a guard's card; None for any other foe) gets from `from`
/// onto `to` on `floor`: a lock opens from its free side, or to a card of its zone or
/// a deeper one; a private room's door opens from inside the room, or to a card of
/// its zone or a deeper one; any other tile opens. An alarm shuts no door to a card.
pub fn opens(post: Option<&Post>, floor: &Floor, from: Pos, to: Pos) -> bool {
    let card = |zone: u8| post.is_some_and(|p| zone <= p.zone);
    if let Some(lock) = floor.lock_at(to) {
        return lock.free_from == Some(from) || (lock.ways.has(Ways::CARD) && card(lock.zone));
    }
    let shut = (floor.door_of(to))
        .filter(|r| r.private && r.kind != RoomKind::Home && !r.rect.contains(from));
    shut.is_none_or(|r| card(r.zone))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::testutil::*;
    use citygen::Pos;

    #[test]
    fn a_guards_card_opens_its_zones_doors() {
        let st = ward();
        let (_, floor) = st.interiors.floor(WARD, 0).unwrap();
        let z = |zone| post(zone, &[Pos::new(2, 3)]);
        let (west, lock, east) = (Pos::new(14, 3), Pos::new(15, 3), Pos::new(16, 3));
        assert!(
            !opens(Some(&z(0)), floor, west, lock),
            "zone 0 has no card for zone 1"
        );
        assert!(opens(Some(&z(1)), floor, west, lock));
        assert!(opens(Some(&z(0)), floor, east, lock), "its free side");
        assert!(!opens(None, floor, west, lock) && opens(None, floor, east, lock));
        let (outside, door, inside) = (Pos::new(23, 3), Pos::new(24, 3), Pos::new(25, 3));
        assert!(!opens(Some(&z(1)), floor, outside, door));
        assert!(opens(Some(&z(2)), floor, outside, door));
        assert!(
            opens(Some(&z(0)), floor, inside, door),
            "out of a room is free"
        );
        assert!(
            opens(None, floor, Pos::new(5, 3), Pos::new(6, 3)),
            "no door"
        );
    }
}
