//! Cloak tests' set-ups: the lot, the Stealth skill, a gun, frames on the map.

use crate::app::AppState;
use citygen::{Place, Pos};
use game::{AbilityId, Action, BodyPart, Charge, CopyId, ItemId, SkillId};

/// The lot with you at (6,8) and a goon at (20,8), out of its sight; the veil off.
pub(crate) fn street() -> AppState {
    let mut st = crate::foes::testutil::lot(Pos::new(20, 8));
    st.veil.off = true;
    st
}

/// You own the Stealth skill, so its Cloak.
pub(crate) fn stealthy(st: &mut AppState) {
    st.character.skills.insert(SkillId::new("stealth"));
}

/// A Ratchet SMG copy worn in the right hand.
pub(crate) fn armed(st: &mut AppState) {
    let gun = ItemId::new("ratchet_smg");
    let copy = st.character.new_copy(&st.content, &gun);
    st.character.wear(BodyPart::RightHand, gun, Some(copy));
}

/// A field scrambler in the pack, its cell full; its copy.
pub(crate) fn scrambler(st: &mut AppState) -> CopyId {
    let item = ItemId::new("field_scrambler");
    st.character.give_item(&st.content, item.clone(), 1);
    // The pack takes a new copy last: the newest of the item's is this one.
    let new = (st.character.inventory.iter().rev()).find(|s| s.item == item);
    new.and_then(|s| s.copy_id()).expect("a copy in the pack")
}

/// Writes a carried copy's charge.
pub(crate) fn set_charge(st: &mut AppState, id: CopyId, charge: Charge) {
    let s = (st.character.inventory.iter_mut()).find(|s| s.copy_id() == Some(id));
    s.and_then(|s| s.copy.as_mut()).expect("carried").charge = Some(charge);
}

/// `n` frames on the map: the tick moves on and the cloak takes its frame.
pub(crate) fn frames(st: &mut AppState, n: u64) {
    for _ in 0..n {
        st.tick += 1;
        super::frame(st);
    }
}

/// Fires an ability as its slot does.
pub(crate) fn fire(st: &mut AppState, id: &str) -> Result<String, String> {
    crate::actions::perform(st, Action::Fire(AbilityId::new(id)))
}

/// You on a lit walkable tile of `b`'s floor 4 to 6 tiles from a standing guard you
/// see, every other guard of `b` gone: the guard's tile.
pub(crate) fn a_guard_in_sight(st: &mut AppState, b: citygen::BuildingId) -> Pos {
    use crate::sitenet::testutil::stand_on_floor;
    let posted: Vec<(usize, i32, Pos)> = (st.foes.iter().enumerate())
        .filter(|(_, f)| f.standing() && f.post.as_ref().is_some_and(|p| p.building == b))
        .filter_map(|(i, f)| match f.place {
            Place::Floor { building, level } if building == b => Some((i, level, f.pos)),
            _ => None,
        })
        .collect();
    for (i, level, at) in posted {
        let city = st.city.clone().expect("a city");
        let Some(floor) = st.interiors.ensure(&city, b, level).cloned() else {
            continue;
        };
        let place = Place::Floor { building: b, level };
        for dy in -6..=6 {
            for dx in -6..=6 {
                let spot = at.offset(dx, dy);
                let far = spot.chebyshev(at);
                let open = floor.tiles.get(spot).is_some_and(|t| t.is_walkable())
                    && floor.light.get(spot).is_some_and(|l| *l > 0)
                    && crate::foes::foe_at(st, place, spot).is_none();
                if !(4..=6).contains(&far) || !open {
                    continue;
                }
                stand_on_floor(st, b, level, spot);
                if crate::foes::in_sight(st).contains(&i) {
                    let guard = st.foes[i].clone();
                    crate::guards::testutil::no_guards(st, b);
                    st.foes.push(guard);
                    assert_eq!(crate::foes::in_sight(st).len(), 1, "that guard only");
                    return at;
                }
            }
        }
    }
    panic!("no guard of {b:?}'s copy stands in sight of a lit tile 4 to 6 off");
}
