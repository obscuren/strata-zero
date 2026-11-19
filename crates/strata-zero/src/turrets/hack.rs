//! A turret from the Net: a crack on its node holds it for an hour, and while you hold
//! it you blind it or, with Turncoat, take it over. When the hold ends it is its own
//! again (`clock::sync`).

use super::Hacked;
use crate::app::AppState;
use crate::foes::FoeMode;
use citygen::SiteNode;

/// The turret of `node` goes blind for the hold: it sees nothing, wakes to nothing, and
/// holds no fight open.
pub fn blind(st: &mut AppState, node: SiteNode) -> Result<String, String> {
    hack(st, node, Hacked::Blind)?;
    Ok("turret blind".to_string())
}

/// The turret of `node` takes your side for the hold. Takes the Turncoat talent.
pub fn take_over(st: &mut AppState, node: SiteNode) -> Result<String, String> {
    if !st.character.flags(&st.content).contains("turncoat") {
        return Err("takes Turncoat".to_string());
    }
    hack(st, node, Hacked::Yours)?;
    Ok("turret's yours".to_string())
}

/// The turret of `node` on your net as `how`: calm and asleep to its own, so it neither
/// hunts nor holds a fight open.
fn hack(st: &mut AppState, node: SiteNode, how: Hacked) -> Result<(), String> {
    let i = super::of_node(st, node).ok_or("nothing answers there")?;
    let f = &mut st.foes[i];
    f.mode = FoeMode::Idle;
    if let Some(t) = f.turret.as_mut() {
        t.hacked = how;
        t.awake = false;
        t.told = None;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::guards::testutil::*;
    use crate::holds::{Hold, VerbDone};
    use crate::net::Actor;
    use crate::owners::Owner;
    use crate::turrets::testutil::{hold_from_the_net, turret_copy, turret_in_ward};
    use crate::turrets::{Hacked, TurretState};
    use citygen::{Dir, NetNode, Pos};

    #[test]
    fn a_crack_on_its_node_holds_it_with_two_verbs() {
        let (mut st, _, i) = turret_copy();
        hold_from_the_net(&mut st, i);
        assert!(
            st.log
                .iter()
                .any(|e| e.text.contains("held · an hour · [o] blind [i] take over"))
        );
    }

    #[test]
    fn blind_it_goes_dark_for_the_hold() {
        let (mut st, _, i) = turret_copy();
        hold_from_the_net(&mut st, i);
        assert_eq!(
            crate::holds::use_verb(&mut st, 'o'),
            Ok(VerbDone::Said("turret blind".into()))
        );
        assert_eq!(crate::turrets::state(&st.foes[i]), Some(TurretState::Blind));
        // Dark: an alarm doesn't wake it, nor does its own field.
        let b = st.foes[i].turret.as_ref().unwrap().node.building;
        crate::net::raise_alarm(&mut st, b);
        crate::turrets::tick(&mut st);
        assert_eq!(crate::turrets::state(&st.foes[i]), Some(TurretState::Blind));
    }

    #[test]
    fn take_over_needs_turncoat() {
        let (mut st, _, i) = turret_copy();
        // The copy's runner knows every talent: this one forgets Turncoat.
        st.character
            .talents
            .remove(&game::TalentId::new("turncoat"));
        hold_from_the_net(&mut st, i);
        assert_eq!(
            crate::holds::use_verb(&mut st, 'i'),
            Err("takes Turncoat".to_string())
        );
        st.character
            .talents
            .insert(game::TalentId::new("turncoat"), 1);
        assert_eq!(
            crate::holds::use_verb(&mut st, 'i'),
            Ok(VerbDone::Said("turret's yours".into()))
        );
        assert_eq!(crate::turrets::state(&st.foes[i]), Some(TurretState::Yours));
        // And back to blind on the same hold.
        crate::holds::use_verb(&mut st, 'o').unwrap();
        assert_eq!(crate::turrets::state(&st.foes[i]), Some(TurretState::Blind));
    }

    #[test]
    fn when_the_hold_ends_it_is_its_own_again() {
        let (mut st, _, i) = turret_copy();
        hold_from_the_net(&mut st, i);
        crate::holds::use_verb(&mut st, 'o').unwrap();
        crate::sitenet::testutil::pass_minutes(&mut st, game::HACK_MINUTES + 1);
        crate::holds::tick(&mut st);
        crate::turrets::tick(&mut st);
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(TurretState::Asleep)
        );
        assert!(
            st.log
                .iter()
                .any(|e| e.text.contains("turret's back on its own net"))
        );
    }

    /// A hold on turret `i`'s node, pushed by hand, until game minute `until`.
    fn held_by_hand(st: &mut crate::app::AppState, i: usize, until: u64) -> citygen::SiteNode {
        let node = st.foes[i].turret.as_ref().unwrap().node;
        st.net.holds.push(Hold {
            node: NetNode::Site(node),
            owner: Owner::Building(WARD),
            until,
            used: Vec::new(),
        });
        node
    }

    /// How many seats turret `i` has in the fight's order.
    fn seats(st: &crate::app::AppState, i: usize) -> usize {
        let e = st.net.engaged.as_ref().expect("a fight");
        e.order.iter().filter(|a| **a == Actor::Foe(i)).count()
    }

    /// Ends turns round the whole order, or until the fight is over.
    fn round_the_order(st: &mut crate::app::AppState) {
        let len = st.net.engaged.as_ref().map_or(0, |e| e.order.len());
        for _ in 0..=len {
            if st.net.engaged.is_none() {
                return;
            }
            crate::net::end_turn(st);
        }
    }

    #[test]
    fn a_hold_ending_mid_fight_turns_your_turret_on_you_in_its_seat() {
        // Yours, you in its field, a guard hunting you: the fight is on.
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Yours;
        let now = st.net.now(&st);
        held_by_hand(&mut st, i, now + 1);
        let g = guard(&mut st, 0, 0, &[Pos::new(3, 3)]);
        st.foes[g].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        crate::net::engage(&mut st, on(0), false);
        // The turret that's yours takes its seat as the fight starts, on your side.
        assert_eq!(seats(&st, i), 1, "yours, it sits");
        let chip = |st: &crate::app::AppState| {
            let view = crate::net::turn_view(st).expect("the turn column");
            view.chips.into_iter().find(|c| c.name == "turret")
        };
        assert!(chip(&st).expect("its chip").ours);
        // The hold runs out.
        crate::sitenet::testutil::pass_minutes(&mut st, 2);
        crate::holds::tick(&mut st);
        assert!(crate::turrets::turn_start(&mut st, i), "its turn: it acts");
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(TurretState::Hunting)
        );
        assert_eq!(
            st.foes[i].mode,
            crate::foes::FoeMode::Hunting {
                seen: Pos::new(6, 3),
                lost: 0
            }
        );
        assert!(logged(&st, "turret's back on its own net"));
        crate::net::seat_foes(&mut st);
        assert_eq!(seats(&st, i), 1, "the seat it had, no second");
        let view = crate::net::turn_view(&st).expect("the turn column");
        assert_eq!(view.chips.iter().filter(|c| c.name == "turret").count(), 1);
        assert!(!chip(&st).expect("its chip").ours);
        assert!(crate::net::turn_readout(&st).is_some());
    }

    #[test]
    fn the_hold_ending_on_alert_leaves_it_awake() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::E);
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Blind;
        let now = st.net.now(&st);
        held_by_hand(&mut st, i, now + 1);
        crate::net::lock_down(&mut st, WARD);
        crate::sitenet::testutil::pass_minutes(&mut st, 2);
        assert_eq!(crate::turrets::state(&st.foes[i]), Some(TurretState::Awake));
        assert!(logged(&st, "turret's back on its own net"));
    }

    #[test]
    fn blinded_or_wrecked_mid_fight_it_passes_and_the_fight_still_ends() {
        for wrecked in [false, true] {
            // Hostile and hunting you in a fight it sits in.
            let mut st = ward();
            let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
            assert!(crate::foes::look_all(&mut st));
            crate::net::engage(&mut st, on(0), false);
            assert_eq!(seats(&st, i), 1);
            if wrecked {
                st.foes[i].down = true;
            } else {
                let now = st.net.now(&st);
                held_by_hand(&mut st, i, now + game::HACK_MINUTES);
                st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Blind;
            }
            assert!(!crate::turrets::turn_start(&mut st, i), "wrecked {wrecked}");
            assert_eq!(seats(&st, i), 1, "it keeps its place in the order");
            let view = crate::net::turn_view(&st).expect("the turn column");
            assert!(
                view.chips.iter().all(|c| c.name != "turret"),
                "wrecked {wrecked}"
            );
            round_the_order(&mut st);
            assert!(
                st.net.engaged.is_none(),
                "wrecked {wrecked}: the fight ends"
            );
        }
    }

    #[test]
    fn blinding_a_hunting_turret_mid_fight_lets_the_fight_end() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
        assert!(crate::foes::look_all(&mut st));
        crate::net::engage(&mut st, on(0), false);
        let now = st.net.now(&st);
        let node = held_by_hand(&mut st, i, now + game::HACK_MINUTES);
        assert_eq!(
            crate::turrets::blind(&mut st, node),
            Ok("turret blind".to_string())
        );
        assert!(!crate::foes::any_hunting(&st, on(0)));
        round_the_order(&mut st);
        assert!(st.net.engaged.is_none());
    }

    #[test]
    fn nothing_answers_a_node_with_no_turret() {
        let mut st = ward();
        let node = citygen::SiteNode {
            building: WARD,
            level: 0,
            at: Pos::new(9, 9),
        };
        assert_eq!(
            crate::turrets::blind(&mut st, node),
            Err("nothing answers there".to_string())
        );
        st.character
            .talents
            .insert(game::TalentId::new("turncoat"), 1);
        assert_eq!(
            crate::turrets::take_over(&mut st, node),
            Err("nothing answers there".to_string())
        );
    }
}
