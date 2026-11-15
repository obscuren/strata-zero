//! Cloak tests' set-ups: the lot, the Stealth skill, a gun, frames on the map.

use crate::app::AppState;
use citygen::Pos;
use game::{AbilityId, Action, Charge, CopyId, ItemId, SkillId, Slot};

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

/// A Ratchet SMG copy worn in the primary slot.
#[expect(dead_code, reason = "for the ambush's tests")]
pub(crate) fn armed(st: &mut AppState) {
    let gun = ItemId::new("ratchet_smg");
    let copy = st.character.new_copy(&st.content, &gun);
    st.character.wear(Slot::Primary, gun, Some(copy));
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
