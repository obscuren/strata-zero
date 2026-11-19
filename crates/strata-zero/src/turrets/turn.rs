//! A turret's turn: it sees you, takes a seat, turns and fires.
//!
//! Its whole turn is one action (`Step::Nothing`, so the turn passes after it): one
//! eighth round toward its target when its field doesn't hold it, then its gun when it
//! has the target in its field. `plan` works that out once, for the turn and for the
//! turn column's word.

use super::field::{aim, dir_word, in_field, turn_toward};
use super::{Hacked, clock};
use crate::app::AppState;
use crate::foes::{self, FoeMode};
use crate::net::Step;
use citygen::{Dir, Pos};

/// What turret `i` does with its turn: the facing it turns to, if it turns, and whether
/// it then has its target to fire on.
struct Plan {
    target: Pos,
    turn: Option<Dir>,
    fires: bool,
}

/// Whether turret `i`, facing `facing`, has `p` to fire on: seen in its field by the
/// floor's light, with a clear line. You, never while cloaked.
fn has(st: &AppState, i: usize, facing: Dir, p: Pos) -> bool {
    let f = &st.foes[i];
    let you = (st.player.as_ref()).is_some_and(|me| me.place == f.place && me.pos == p);
    if you && crate::cloak::hidden(st) {
        return false;
    }
    foes::map_of(st, f.place)
        .is_some_and(|m| super::field::sees(&m, f.pos, facing, p) && foes::line_clear(&m, f.pos, p))
}

/// Turret `i`'s turn, worked out: None with no target.
fn plan(st: &AppState, i: usize) -> Option<Plan> {
    let target = target(st, i)?;
    let f = st.foes.get(i)?;
    let facing = f.turret.as_ref()?.facing;
    let turn = (!in_field(f.pos, facing, target))
        .then(|| turn_toward(facing, aim(f.pos, facing, target)))
        .filter(|d| *d != facing);
    let fires = has(st, i, turn.unwrap_or(facing), target);
    Some(Plan {
        target,
        turn,
        fires,
    })
}

/// What turret `i` shoots at: a hostile one, you, when you're on its place. Nothing for
/// one that is blind, wrecked, or yours.
pub fn target(st: &AppState, i: usize) -> Option<Pos> {
    let f = st.foes.get(i)?;
    if !super::hostile(f) {
        return None;
    }
    (st.player.as_ref())
        .filter(|me| me.place == f.place)
        .map(|me| me.pos)
}

/// Turret `i` looks: a hostile, standing, unstunned one that sees you in its own field
/// hunts you (waking, if it slept). Returns whether it hunts after looking; a blind one,
/// a wrecked one or yours never does.
pub fn look(st: &mut AppState, i: usize) -> bool {
    let Some(f) = st.foes.get(i) else {
        return false;
    };
    if !super::hostile(f) {
        return false;
    }
    let hunting = f.hunting();
    let Some(t) = f.turret.as_ref() else {
        return false;
    };
    let (facing, asleep) = (t.facing, !t.awake);
    if f.stunned > 0 {
        return hunting;
    }
    let Some(you) = target(st, i) else {
        return hunting;
    };
    if !has(st, i, facing, you) {
        return hunting;
    }
    if asleep {
        clock::wake(st, i, Some(you));
    }
    has_you(st, i, you);
    true
}

/// Turret `i` has you at `you`: it hunts, awake, its minute renewed.
fn has_you(st: &mut AppState, i: usize, you: Pos) {
    let minute = st.net.now(st);
    let f = &mut st.foes[i];
    f.mode = FoeMode::Hunting { seen: you, lost: 0 };
    if let Some(t) = f.turret.as_mut() {
        t.awake = true;
        t.since = minute;
    }
}

/// Hunting turret `i` didn't have you this turn: after `net::LOST_TURNS` of them it
/// lets you go, awake, turning toward where it last had you.
fn missed_you(st: &mut AppState, i: usize) {
    let minute = st.net.now(st);
    let f = &mut st.foes[i];
    let FoeMode::Hunting { seen, lost } = f.mode else {
        return;
    };
    let lost = lost + 1;
    if lost < crate::net::LOST_TURNS {
        f.mode = FoeMode::Hunting { seen, lost };
        return;
    }
    f.mode = FoeMode::Idle;
    if let Some(t) = f.turret.as_mut() {
        t.awake = true;
        t.told = Some(seen);
        t.since = minute;
    }
}

/// Turret `i`'s turn begins: its holds read back onto it, then a blind one passes, one
/// that's yours acts in a fight on its place, a hostile one looks and acts when it hunts.
pub fn turn_start(st: &mut AppState, i: usize) -> bool {
    clock::sync(st, i);
    let Some(f) = st.foes.get(i).filter(|f| f.standing()) else {
        return false;
    };
    match f.turret.as_ref().map(|t| t.hacked) {
        Some(Hacked::None) => look(st, i),
        Some(Hacked::Yours) => (st.net.engaged.as_ref()).is_some_and(|e| e.place == f.place),
        _ => false,
    }
}

/// Turret `i`'s whole turn, one action: it turns one eighth toward its target when its
/// field doesn't hold it, then fires when it has it (clicks with its magazine empty).
/// A hostile one keeps its hunt: had you, it hunts on; else it counts a turn toward
/// losing you.
pub fn step(st: &mut AppState, i: usize) -> Step {
    let Some(f) = st.foes.get(i) else {
        return Step::Nothing;
    };
    let (hostile, name) = (super::hostile(f), f.name.clone());
    let plan = plan(st, i);
    let mut had = None;
    if let Some(p) = &plan {
        let lead = p.turn.map(|d| format!("turns {}", dir_word(d)));
        if let Some(d) = p.turn {
            let f = &mut st.foes[i];
            f.glyph = super::field::glyph(d);
            if let Some(t) = f.turret.as_mut() {
                t.facing = d;
            }
        }
        let dry = (st.foes[i].turret.as_ref()).is_some_and(|t| t.rounds == 0);
        if p.fires {
            had = Some(p.target);
            if dry {
                st.warn(match &lead {
                    Some(lead) => format!("{name} {lead} › clicks dry"),
                    None => format!("{name} clicks dry"),
                });
            } else {
                crate::combat::fire_at_you_with(st, i, lead.as_deref());
            }
        } else if let Some(lead) = lead {
            st.warn(format!("{name} {lead}"));
        }
    }
    if hostile && st.foes.get(i).is_some_and(|f| f.standing()) {
        match had {
            Some(you) => has_you(st, i, you),
            None => missed_you(st, i),
        }
    }
    Step::Nothing
}

/// Turret `i`'s plan as the turn column reads it: `clicks dry`, `fires`,
/// `turns › fires`, `turns`, or `watches` when its turn does nothing.
pub fn plan_word(st: &AppState, i: usize) -> String {
    let dry = (st.foes.get(i))
        .and_then(|f| f.turret.as_ref())
        .is_some_and(|t| t.rounds == 0);
    match plan(st, i) {
        Some(Plan { fires: true, .. }) if dry => "clicks dry".to_string(),
        Some(Plan {
            fires: true,
            turn: Some(_),
            ..
        }) => "turns › fires".to_string(),
        Some(Plan { fires: true, .. }) => "fires".to_string(),
        Some(Plan { turn: Some(_), .. }) => "turns".to_string(),
        _ => "watches".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::guards::testutil::*;
    use crate::turrets::TurretState;
    use crate::turrets::testutil::turret_in_ward;
    use citygen::{Dir, Place, Pos};

    fn in_ward(you: Pos, at: Pos, facing: Dir) -> (AppState, usize) {
        let mut st = ward();
        stand(&mut st, 0, you);
        let i = turret_in_ward(&mut st, 0, at, facing);
        (st, i)
    }

    #[test]
    fn its_own_field_sees_you_and_the_fight_starts() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        assert!(crate::foes::look_all(&mut st));
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(TurretState::Hunting)
        );
        assert!(st.foes[i].turret.as_ref().unwrap().awake);
        assert!(logged(&st, "turret wakes"));
        crate::net::engage(&mut st, on(0), false);
        assert!(
            st.net
                .engaged
                .as_ref()
                .unwrap()
                .order
                .contains(&crate::net::Actor::Foe(i))
        );
    }

    #[test]
    fn behind_it_you_are_not_seen() {
        let (mut st, i) = in_ward(Pos::new(12, 3), Pos::new(10, 3), Dir::W);
        assert!(!crate::foes::look_all(&mut st));
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(TurretState::Asleep)
        );
    }

    #[test]
    fn in_its_field_it_fires_and_the_magazine_pays() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        st.force_hit = Some(false);
        crate::foes::look_all(&mut st);
        assert_eq!(crate::turrets::plan_word(&st, i), "fires");
        step(&mut st, i);
        assert!(logged(&st, "turret fires · miss · miss · miss"));
        assert_eq!(st.foes[i].turret.as_ref().unwrap().rounds, 27);
    }

    #[test]
    fn out_of_its_field_it_turns_one_eighth_and_fires_if_that_finds_you() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::N);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        st.foes[i].turret.as_mut().unwrap().awake = true;
        st.force_hit = Some(false);
        assert_eq!(crate::turrets::plan_word(&st, i), "turns › fires");
        step(&mut st, i);
        assert_eq!(st.foes[i].turret.as_ref().unwrap().facing, Dir::NW);
        assert_eq!(st.foes[i].glyph, crate::turrets::field::glyph(Dir::NW));
        assert!(logged(&st, "turret turns NW › fires · miss · miss · miss"));
    }

    #[test]
    fn a_turn_that_doesnt_find_you_only_turns() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::E);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        st.foes[i].turret.as_mut().unwrap().awake = true;
        assert_eq!(crate::turrets::plan_word(&st, i), "turns");
        step(&mut st, i);
        // aim: NW and SW both hold you three eighths off E; clockwise first, so SW, via SE.
        assert!(logged(&st, "turret turns SE"));
        assert_eq!(st.foes[i].turret.as_ref().unwrap().facing, Dir::SE);
        assert_eq!(st.foes[i].turret.as_ref().unwrap().rounds, 30);
    }

    #[test]
    fn dry_it_turns_and_watches_and_clicks() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        crate::foes::look_all(&mut st);
        st.foes[i].turret.as_mut().unwrap().rounds = 0;
        let hp = st.character.hp;
        assert_eq!(crate::turrets::plan_word(&st, i), "clicks dry");
        step(&mut st, i);
        assert!(logged(&st, "turret clicks dry"));
        assert_eq!(st.character.hp, hp);
    }

    #[test]
    fn the_last_rounds_are_all_it_fires() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        crate::foes::look_all(&mut st);
        st.force_hit = Some(false);
        st.foes[i].turret.as_mut().unwrap().rounds = 2;
        step(&mut st, i);
        assert!(logged(&st, "turret fires · miss · miss") && !logged(&st, "miss · miss · miss"));
        assert_eq!(st.foes[i].turret.as_ref().unwrap().rounds, 0);
    }

    #[test]
    fn out_of_its_field_three_turns_it_loses_you_and_stays_awake() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        crate::foes::look_all(&mut st);
        stand(&mut st, 1, Pos::new(5, 3)); // gone upstairs
        for n in 1..crate::net::LOST_TURNS {
            step(&mut st, i);
            assert_eq!(
                crate::turrets::state(&st.foes[i]),
                Some(TurretState::Hunting),
                "still hunting after {n}"
            );
        }
        step(&mut st, i);
        let f = &st.foes[i];
        assert_eq!(crate::turrets::state(f), Some(TurretState::Awake));
        assert_eq!(f.turret.as_ref().unwrap().told, Some(Pos::new(6, 3)));
    }

    #[test]
    fn cloaked_or_jacked_in_it_never_sees_you() {
        // Cloaked in its field: no hunt, no fight.
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        crate::cloak::testutil::stealthy(&mut st);
        st.character.focus = 10;
        crate::cloak::testutil::fire(&mut st, "cloak").unwrap();
        assert!(!crate::foes::look_all(&mut st));
        assert!(!crate::foes::look(&mut st, i));
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(TurretState::Asleep)
        );
        // Hunting already, it doesn't fire on you cloaked in its field.
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        st.foes[i].turret.as_mut().unwrap().awake = true;
        step(&mut st, i);
        assert!(!logged(&st, "turret fires"));
        assert_eq!(st.foes[i].turret.as_ref().unwrap().rounds, 30);

        // Your body on its floor in its field while you're on the building's Net.
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        let p = st.player.as_mut().unwrap();
        p.body = Some((on(0), Pos::new(6, 3)));
        p.place = Place::SiteNet {
            building: WARD,
            level: 0,
        };
        p.pos = Pos::new(2, 2);
        assert!(!crate::foes::look_all(&mut st));
        assert!(!crate::foes::look(&mut st, i));
        assert!(!turn_start(&mut st, i));
        assert_eq!(target(&st, i), None);
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(TurretState::Asleep)
        );
    }

    #[test]
    fn in_a_fight_its_turn_comes_in_the_order_with_one_move_and_one_ap() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        crate::foes::look_all(&mut st);
        crate::net::engage(&mut st, on(0), false);
        if crate::net::your_turn(&st) {
            crate::net::end_turn(&mut st);
        }
        let e = st.net.engaged.as_ref().unwrap();
        assert_eq!(e.actor(), crate::net::Actor::Foe(i));
        assert_eq!((e.moves, e.ap), (1, 1));
        let view = crate::net::turn_view(&st).unwrap();
        assert_eq!(view.theirs.as_deref(), Some("fires"));
        assert!(
            crate::net::turn_readout(&st)
                .unwrap()
                .contains("turret fires")
        );
        settle(&mut st);
        assert!(logged(&st, "turret fires"));
    }

    #[test]
    fn asleep_or_awake_it_takes_no_seat_in_a_fight() {
        let (mut st, i) = in_ward(Pos::new(12, 3), Pos::new(10, 3), Dir::W);
        assert!(!crate::turrets::seated(&st, i));
        crate::net::engage(&mut st, on(0), true);
        let e = st.net.engaged.as_ref().unwrap();
        assert!(!e.order.contains(&crate::net::Actor::Foe(i)));
        let view = crate::net::turn_view(&st).unwrap();
        assert!(view.chips.iter().all(|c| c.name != "turret"));
        assert!(!crate::net::turn_readout(&st).unwrap().contains("turret"));
    }

    #[test]
    fn hunting_it_counts_on_its_floor() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        assert!(!crate::foes::hunting_on(&st, on(0)).contains(&i));
        crate::foes::look_all(&mut st);
        assert!(crate::foes::hunting_on(&st, on(0)).contains(&i));
        assert!(crate::turrets::seated(&st, i));
    }

    #[test]
    fn a_calm_turret_looks_as_a_round_of_the_fight_ends() {
        // A fight by hand, you behind it; you step into its field and the round ends.
        let (mut st, i) = in_ward(Pos::new(12, 3), Pos::new(10, 3), Dir::W);
        crate::net::engage(&mut st, on(0), true);
        stand(&mut st, 0, Pos::new(6, 3));
        crate::net::seat_foes(&mut st);
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(TurretState::Hunting)
        );
        assert!(
            st.net
                .engaged
                .as_ref()
                .unwrap()
                .order
                .contains(&crate::net::Actor::Foe(i))
        );
    }
}
