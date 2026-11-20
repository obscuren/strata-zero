//! Cracking a locked credchip in the reader, one lock at a time: your loaded Crack at the
//! chip's locks, a minute and a roll a try. It is inside your own firewall, so no trace
//! rises and no ICE answers. The last lock turns the chip into an open credchip.

pub use super::reader::locks;
use crate::app::AppState;
use citygen::NodeKind;
use game::{ItemId, Op, Target};

/// The security behind a chip's locks, by how many it has (one, two, three): the
/// difficulty a try is rolled against.
pub const LOCK_SECURITY: [u8; 3] = [100, 140, 180];

/// The Crack loaded in your deck, if any.
fn loaded_crack(st: &AppState) -> Option<game::Program> {
    (st.character.loaded.iter())
        .filter_map(|id| st.content.item(id).and_then(|i| i.program))
        .find(|p| p.op == Op::Crack)
}

/// The chip in `slot`: its sum and how many locks it has and has had cracked.
fn chip(st: &AppState, slot: usize) -> Result<(u32, u8, u8), String> {
    let stack = (st.character.reader.get(slot))
        .and_then(|s| s.as_ref())
        .ok_or_else(|| "that slot is empty".to_string())?;
    if stack.item.0 != "credchip_locked" {
        return Err("nothing there to crack".to_string());
    }
    let copy = stack.copy.as_ref();
    let credits = copy.map_or(0, |k| k.credits);
    Ok((credits, locks(credits), copy.map_or(0, |k| k.cracked)))
}

/// Your chance a try cracks the next lock of the chip in `slot`, in percent; refused as
/// a run is on the Net: no deck jacked, no Crack loaded.
pub fn crack_chance(st: &AppState, slot: usize) -> Result<u8, String> {
    let (_, locks, _) = chip(st, slot)?;
    if st.character.deck(&st.content).is_none() {
        return Err("no deck jacked".to_string());
    }
    let spec = loaded_crack(st).ok_or_else(|| "no Crack loaded".to_string())?;
    let security = LOCK_SECURITY[usize::from(locks.clamp(1, 3)) - 1];
    Ok(game::chance(
        game::power(
            &st.character,
            &st.content,
            &spec,
            Target::Node(NodeKind::Container),
        ),
        game::difficulty(security),
    ))
}

/// One try at the chip in `slot`: a minute passes and a roll at `crack_chance` either
/// cracks a lock or doesn't. Returns the log line. The last lock leaves a `credchip`
/// with the same copy in the same slot. Refused, nothing is spent.
pub fn crack(st: &mut AppState, slot: usize) -> Result<String, String> {
    let chance = crack_chance(st, slot)?;
    let (credits, locks, cracked) = chip(st, slot)?;
    crate::clock::run(
        st,
        Some(sim::TICKS_PER_MINUTE),
        crate::clock::PATHS_PER_FRAME,
    );
    let roll = crate::combat::roll(st);
    if !game::round_hits(roll, chance) {
        return Ok("the lock holds".to_string());
    }
    let cracked = cracked + 1;
    let stack = st.character.reader[slot]
        .as_mut()
        .expect("the chip is there");
    if let Some(k) = stack.copy.as_mut() {
        k.cracked = cracked;
    }
    if cracked < locks {
        return Ok(format!("lock {cracked} of {locks} cracked"));
    }
    stack.item = ItemId::new("credchip");
    Ok(format!("the chip opens · ¤{}", crate::ui::commas(credits)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rigroom::testutil::at_the_rig;
    use game::Stack;

    /// A rig with a Civitas cabinet and a runner with a deck and Crack loaded, jacked in,
    /// a locked chip of `credits` in slot 0.
    fn armed(credits: u32) -> AppState {
        let mut st = at_the_rig();
        let c = st.content.clone();
        st.character = game::Character::debug_runner(&c);
        let crack = ItemId::new("crack");
        st.character.give(crack.clone(), 1);
        st.character.loaded.push(crack);
        crate::rig::fit_new_rack(&mut st.character, &c, "rack_civitas");
        crate::net::jack_in_here(&mut st).expect("jacked in");
        let id = ItemId::new("credchip_locked");
        let mut chip = Stack::new(id.clone(), 1);
        let mut copy = st.character.new_copy(&c, &id);
        copy.credits = credits;
        let copy_id = copy.id;
        chip.copy = Some(copy);
        st.character.inventory.push(chip);
        st.character
            .put_in_reader(&c, 0, copy_id)
            .expect("in the reader");
        st
    }

    #[test]
    fn locks_come_from_the_sum() {
        assert_eq!(locks(499), 1);
        assert_eq!(locks(500), 2);
        assert_eq!(locks(1999), 2);
        assert_eq!(locks(2000), 3);
    }

    #[test]
    fn a_hit_cracks_a_lock_and_the_last_opens_it() {
        let mut st = armed(1200);
        assert!(crack_chance(&st, 0).unwrap() > 0);
        st.force_hit = Some(true);
        let t0 = st.clock_tick();
        assert_eq!(crack(&mut st, 0).unwrap(), "lock 1 of 2 cracked");
        assert_eq!(st.clock_tick(), t0 + sim::TICKS_PER_MINUTE);
        let held = st.character.reader[0].as_ref().unwrap();
        assert_eq!(held.item.0, "credchip_locked");
        assert_eq!(held.copy.as_ref().unwrap().cracked, 1);
        assert_eq!(crack(&mut st, 0).unwrap(), "the chip opens · ¤1,200");
        assert_eq!(st.clock_tick(), t0 + 2 * sim::TICKS_PER_MINUTE);
        let held = st.character.reader[0].as_ref().unwrap();
        assert_eq!(held.item.0, "credchip");
        assert_eq!(held.copy.as_ref().unwrap().credits, 1200);
    }

    #[test]
    fn a_miss_holds() {
        let mut st = armed(1200);
        st.force_hit = Some(false);
        // A runner's power would make it a sure thing; weaken the deck.
        st.character.equipment.remove(&game::BodyPart::Waist);
        let c = st.content.clone();
        let deck = ItemId::new("pawnshop_deck");
        let copy = st.character.new_copy(&c, &deck);
        st.character.wear(game::BodyPart::Waist, deck, Some(copy));
        st.character.attributes.net = 1;
        st.character.talents.clear();
        st.character.skills.clear();
        // A sure thing would still land on the roll's 100.
        let chance = crack_chance(&st, 0).unwrap();
        assert!(chance < 100, "{chance}% leaves a miss to force");
        assert_eq!(crack(&mut st, 0).unwrap(), "the lock holds");
        let held = st.character.reader[0].as_ref().unwrap();
        assert_eq!(held.copy.as_ref().unwrap().cracked, 0);
        assert_eq!(held.item.0, "credchip_locked");
    }

    #[test]
    fn crack_without_a_deck_refuses_and_keeps_the_chip() {
        let mut st = armed(1200);
        st.force_hit = Some(true);
        let t0 = st.clock_tick();
        st.character.loaded.clear();
        assert_eq!(crack(&mut st, 0), Err("no Crack loaded".to_string()));
        st.character.loaded.push(ItemId::new("crack"));
        st.character.equipment.remove(&game::BodyPart::Waist);
        assert_eq!(crack(&mut st, 0), Err("no deck jacked".to_string()));
        assert_eq!(st.clock_tick(), t0, "no time spent");
        let held = st.character.reader[0].as_ref().unwrap();
        assert_eq!(held.item.0, "credchip_locked");
        assert_eq!(held.copy.as_ref().unwrap().cracked, 0);
    }

    #[test]
    fn cracking_raises_no_trace() {
        let mut st = armed(1200);
        st.force_hit = Some(true);
        let traces = st.net.traces.clone();
        crack(&mut st, 0).unwrap();
        assert_eq!(st.net.traces, traces);
        assert!(st.net.run.is_none());
    }

    #[test]
    fn the_popup_and_the_panel_offer_the_crack() {
        let mut st = armed(1200);
        let at = crate::rigroom::layout::front(crate::rigroom::room(&st).unwrap().slots[0]);
        st.player.as_mut().unwrap().pos = at;
        let chance = crack_chance(&st, 0).unwrap();
        let popup = crate::rigroom::summon::interact(&mut st).expect("a popup");
        let c = &popup.choices()[0];
        assert_eq!(
            format!("{}{}", c.label, c.tail),
            format!("Crack a lock · {chance}%")
        );
        assert!(c.enabled);
        let lines: Vec<String> = crate::rigroom::panel::here_lines(&st)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(lines[0], " ▣ Locked credchip · ¤1,200 inside");
        assert_eq!(lines[1], format!("   2 locks · crack {chance}% · space"));
        st.character.loaded.clear();
        let popup = crate::rigroom::summon::interact(&mut st).unwrap();
        assert!(!popup.choices()[0].enabled);
    }
}
