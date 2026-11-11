//! Sites to test the Net of a site on: an office laid on a building away from home, plain
//! or with the DEVELOPER start's mission copy, and D'SHAWN with the programs a run wants.

use crate::app::AppState;
use citygen::venue::{MissionInputs, MissionSize, VenuePack};
use citygen::{BuildingId, BuildingKind, NodeKind, SiteNode};
use game::{HackTarget, ItemId, Op, Target};

/// `jobs::testutil::state_home`'s city, D'SHAWN in dev mode with crack, spoof, blind, copy
/// and backdoor loaded.
fn runner_home() -> AppState {
    let mut st = crate::jobs::testutil::state_home();
    st.character = game::Character::debug_runner(&st.content);
    st.unlock_dev();
    for id in ["crack", "spoof", "blind", "copy", "backdoor"] {
        let id = ItemId::new(id);
        if st.character.count(&id) == 0 {
            st.character.give(id.clone(), 1);
        }
        if !st.character.loaded.contains(&id) {
            st.character.loaded.push(id);
        }
    }
    st
}

/// An office laid on a building away from home, no job on it, you on the street at its
/// main door.
pub(crate) fn office_plain() -> (AppState, BuildingId) {
    let mut st = runner_home();
    let t = VenuePack::builtin()
        .for_kind(BuildingKind::Office)
        .expect("a type builds offices");
    let inputs = MissionInputs {
        size: MissionSize::Medium,
        objective: 0,
        security: 128,
        seed: st.tick,
    };
    let b = crate::venues::dev_start_with(&mut st, &t.id, inputs, false).unwrap();
    (st, b)
}

/// The DEVELOPER start's office copy at `security`: its server room's job begun, you on
/// the street at its main door.
pub(crate) fn office_copy(security: u8) -> (AppState, BuildingId) {
    let mut st = runner_home();
    let b = crate::venues::dev_site_net(&mut st, security).unwrap();
    (st, b)
}

/// A hack of `op` landed on site node `node` (a `kind`), now.
pub(crate) fn hack(st: &mut AppState, node: SiteNode, kind: NodeKind, op: Op) {
    let now = st.net.now(st);
    st.net.land(
        node.building,
        HackTarget::Site(node),
        Target::Node(kind),
        op,
        now,
    );
}
