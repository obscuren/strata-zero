//! The tests' rig at home: seed 8's flat, you beside the rig, and the room jacked into.

use crate::app::AppState;
use citygen::{BuildingId, GenParams, Generator, SizePreset};

/// Seed 8, medium, seven districts: you woken in the flat and stepped beside the rig.
pub(crate) fn at_the_rig() -> AppState {
    let mut st = AppState::new();
    st.enter_city(
        Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap(),
    );
    let rig = st.home.expect("a home");
    let city = st.city.as_ref().unwrap();
    let mut p = (rig.wake(city, &mut st.interiors, &st.constructs, 0)).expect("the flat");
    let (_, floor) = (st.interiors.floor(rig.home.building, rig.home.level)).unwrap();
    p.pos = (rig.home.rig.neighbors8())
        .filter(|q| rig.home.rect.contains(*q) && floor.tiles[*q].is_walkable())
        .min_by_key(|q| q.chebyshev(rig.home.bed_side))
        .expect("a tile beside the rig");
    st.player = Some(p);
    st
}

/// A Net backdoor held on the `n`th plain building (not a site) with a gate on the
/// grid, not the home's.
pub(crate) fn hold_a_backdoor(st: &mut AppState, n: usize) -> BuildingId {
    let city = st.city.as_ref().unwrap();
    let home = st.home.unwrap().home.building;
    let b = (city.buildings.iter().map(|b| b.id))
        .filter(|b| *b != home && crate::constructs::city_gate(city, *b).is_some())
        .filter(|b| !crate::sitenet::is_site(st, *b))
        .nth(n)
        .expect("a building with a gate");
    let seed = city.seed;
    st.backdoors.plant_net(b, 1, seed, 0, 3);
    b
}

/// At the rig and jacked in: in the room, on the tile you land on.
pub(crate) fn in_the_room() -> AppState {
    let mut st = at_the_rig();
    crate::net::jack_in_here(&mut st).expect("jacked in");
    st
}

/// The mock's moment (round 5, frame 1), jacked into the room: a Civitas cabinet with a
/// Tinman 2X core, a Switchback relay, a Longwire line and a Sandbag firewall; three
/// backdoors held of its five lines (two Net, then a key); `sift` on a paydata shard in
/// the first seat and `keepalive` holding the first backdoor in the second; a credchip
/// of ¤340 in the reader's first slot. The backdoors' buildings, in the doors' order.
pub(crate) fn the_mocks_rig() -> (AppState, [BuildingId; 3]) {
    use crate::rig::jobs::{JobKind, start};
    use game::{ItemId, Stack};
    let mut st = at_the_rig();
    let c = st.content.clone();
    crate::rig::fit_new_rack(&mut st.character, &c, "rack_civitas");
    let sleds = [
        "sled_tinman",
        "sled_switchback",
        "sled_longwire",
        "sled_sandbag",
    ];
    for (bay, sled) in sleds.into_iter().enumerate() {
        let item = ItemId::new(sled);
        let copy = Some(st.character.new_copy(&c, &item));
        st.character.rig_copy_mut(&c).fitted[bay] = Some(Stack { item, qty: 1, copy });
    }
    let kiroflux = hold_a_backdoor(&mut st, 0);
    let hextek = hold_a_backdoor(&mut st, 1);
    let stack = {
        let city = st.city.as_ref().unwrap();
        let home = st.home.unwrap().home.building;
        let b = (city.buildings.iter().map(|b| b.id))
            .filter(|b| *b != home && crate::constructs::city_gate(city, *b).is_some())
            .filter(|b| !crate::sitenet::is_site(&st, *b))
            .nth(2)
            .expect("a third building with a gate");
        let seed = city.seed;
        st.backdoors.plant_key(b, "ops", 100, seed, 0);
        b
    };
    st.backdoors.jobs.programs.insert("keepalive".to_string());
    st.character.give_item(&c, ItemId::new("paydata"), 1);
    start(
        &mut st,
        JobKind::Sift {
            shard: ItemId::new("paydata"),
        },
    )
    .expect("sift");
    start(&mut st, JobKind::Keepalive(kiroflux)).expect("keepalive");
    st.character.give_item(&c, ItemId::new("credchip"), 1);
    let chip = (st.character.inventory.iter_mut().rev())
        .find(|s| s.item.0 == "credchip")
        .expect("the chip");
    chip.copy.as_mut().expect("a chip's copy").credits = 340;
    let copy = chip.copy_id().expect("a chip's copy");
    st.character
        .put_in_reader(&c, 0, copy)
        .expect("in the reader");
    crate::net::jack_in_here(&mut st).expect("jacked in");
    (st, [kiroflux, hextek, stack])
}

/// The black chip's reader slot in `a_black_chip`, and its sum.
pub(crate) const BLACK_SLOT: usize = 2;
pub(crate) const BLACK_SUM: u32 = 4800;

/// A chip of `id` holding `credits` from the pack into reader slot `slot`.
pub(crate) fn chip_in_reader(st: &mut AppState, slot: usize, id: &str, credits: u32) {
    use game::{ItemId, Stack};
    let c = st.content.clone();
    let id = ItemId::new(id);
    let mut chip = Stack::new(id.clone(), 1);
    let mut copy = st.character.new_copy(&c, &id);
    copy.credits = credits;
    let copy_id = copy.id;
    chip.copy = Some(copy);
    st.character.inventory.push(chip);
    st.character
        .put_in_reader(&c, slot, copy_id)
        .expect("in the reader");
}

/// `id` loaded in the deck, after what is loaded already.
pub(crate) fn load(st: &mut AppState, id: &str) {
    let id = game::ItemId::new(id);
    if !st.character.loaded.contains(&id) {
        st.character.give(id.clone(), 1);
        st.character.loaded.push(id);
    }
}

/// A starting runner with a deck and Attack loaded, a Civitas cabinet (three reader
/// slots) with a Tinman 2X core (two seats), a black chip of ¤4,800 in slot 2, jacked
/// into the room and standing at the front of slot 2.
pub(crate) fn a_black_chip() -> AppState {
    use game::{ItemId, Stack};
    let mut st = at_the_rig();
    let c = st.content.clone();
    if st.character.deck(&c).is_none() {
        let deck = ItemId::new("pawnshop_deck");
        let copy = st.character.new_copy(&c, &deck);
        st.character.wear(game::BodyPart::Waist, deck, Some(copy));
    }
    load(&mut st, "attack");
    crate::rig::fit_new_rack(&mut st.character, &c, "rack_civitas");
    let core = ItemId::new("sled_tinman");
    let copy = Some(st.character.new_copy(&c, &core));
    st.character.rig_copy_mut(&c).fitted[0] = Some(Stack {
        item: core,
        qty: 1,
        copy,
    });
    chip_in_reader(&mut st, BLACK_SLOT, "credchip_black", BLACK_SUM);
    crate::net::jack_in_here(&mut st).expect("jacked in");
    let at = super::layout::front(super::room(&st).unwrap().slots[BLACK_SLOT]);
    st.player.as_mut().unwrap().pos = at;
    st
}

/// The mock's round 3 (`rig-construct-round3.html`, frame 1), jacked into the room: the
/// round 5 rig with `sift` alone in the seats, a deck with Crack, Attack and Shield
/// loaded, and in the reader a credchip of ¤340, a locked one of ¤1,200 (2 locks) and a
/// black one of ¤4,800 with MASTIFF coiled round it; you at the locked chip.
pub(crate) fn the_round_three_rig() -> AppState {
    use game::ItemId;
    let (mut st, _) = the_mocks_rig();
    let c = st.content.clone();
    if st.character.deck(&c).is_none() {
        let deck = ItemId::new("pawnshop_deck");
        let copy = st.character.new_copy(&c, &deck);
        st.character.wear(game::BodyPart::Waist, deck, Some(copy));
    }
    st.character.loaded.clear();
    for id in ["crack", "attack", "shield"] {
        load(&mut st, id);
    }
    let keepalive = (st.backdoors.jobs.running.iter())
        .find(|j| j.kind.name() == "keepalive")
        .map(|j| j.n)
        .expect("keepalive in the second seat");
    crate::rig::jobs::kill(&mut st, keepalive).expect("killed");
    chip_in_reader(&mut st, 1, "credchip_locked", 1200);
    chip_in_reader(&mut st, BLACK_SLOT, "credchip_black", BLACK_SUM);
    let at = super::layout::front(super::room(&st).unwrap().slots[1]);
    st.player.as_mut().unwrap().pos = at;
    st
}
