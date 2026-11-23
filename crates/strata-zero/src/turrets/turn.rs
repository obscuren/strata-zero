//! A turret's turn: it sees you, takes a seat, turns and fires. One you hold does so
//! at the guards, in a fight on its floor; and the guards fire back (`guard_mark`).
//!
//! Its whole turn is one action (`Step::Nothing`, so the turn passes after it): one
//! eighth round toward its target when its field doesn't hold it, then its gun when it
//! has the target in its field. `plan` works that out once, for the turn and for the
//! turn column's word.

use super::field::{aim, dir_word, in_field, turn_toward};
use super::{Hacked, clock, has_you};
use crate::app::AppState;
use crate::foes::{self, FoeMode};
use crate::net::Step;
use citygen::{Dir, Pos};

/// What turret `i` does with its turn: the facing it turns to, if it turns, and whether
/// it then has its target to fire on.
struct Plan {
    target: Pos,
    /// The guard a turret you hold fires on; None for a hostile one, whose target is you.
    mark: Option<usize>,
    turn: Option<Dir>,
    fires: bool,
}

/// Whether turret `i`, facing `facing`, has `p` to fire on: seen in its field by the
/// floor's light, with a clear line. You, never while cloaked.
pub(super) fn has(st: &AppState, i: usize, facing: Dir, p: Pos) -> bool {
    let f = &st.foes[i];
    let you = (st.player.as_ref()).is_some_and(|me| me.place == f.place && me.pos == p);
    if you && crate::cloak::hidden(st) {
        return false;
    }
    foes::map_of(st, f.place)
        .is_some_and(|m| super::field::sees(&m, f.pos, facing, p) && foes::line_clear(&m, f.pos, p))
}

/// Turret `i`'s turn, worked out: None with no target, and for one you hold with no
/// fight on its place.
fn plan(st: &AppState, i: usize) -> Option<Plan> {
    let f = st.foes.get(i)?;
    let facing = f.turret.as_ref()?.facing;
    let mark = if super::is_yours(f) {
        if !in_its_fight(st, i) {
            return None;
        }
        Some(mark(st, i)?)
    } else {
        None
    };
    let target = match mark {
        Some(g) => st.foes[g].pos,
        None => target(st, i)?,
    };
    let turn = (!in_field(f.pos, facing, target))
        .then(|| turn_toward(facing, aim(f.pos, facing, target)))
        .filter(|d| *d != facing);
    let facing = turn.unwrap_or(facing);
    let fires = match mark {
        // The guard's in its field and the gun's range; `mark` saw to the line.
        Some(_) => {
            in_field(f.pos, facing, target) && f.pos.chebyshev(target) <= foes::weapon_range(st, f)
        }
        None => has(st, i, facing, target),
    };
    Some(Plan {
        target,
        mark,
        turn,
        fires,
    })
}

/// A fight is on turret `i`'s place.
fn in_its_fight(st: &AppState, i: usize) -> bool {
    let place = st.foes.get(i).map(|f| f.place);
    (st.net.engaged.as_ref()).is_some_and(|e| Some(e.place) == place)
}

/// What turret `i` shoots at: a hostile one, you, when you're on its place; one you
/// hold, its `mark`, never you. Nothing for one that is blind or wrecked.
pub fn target(st: &AppState, i: usize) -> Option<Pos> {
    let f = st.foes.get(i)?;
    if super::is_yours(f) {
        return mark(st, i).map(|g| st.foes[g].pos);
    }
    if !super::hostile(f) {
        return None;
    }
    (st.player.as_ref())
        .filter(|me| me.place == f.place)
        .map(|me| me.pos)
}

/// The foe turret `i`, yours, fires on: the nearest fighter (`Foe::fighter`) on its
/// place it has a clear line to, by distance then index.
fn mark(st: &AppState, i: usize) -> Option<usize> {
    let f = st.foes.get(i).filter(|f| super::is_yours(f))?;
    let map = foes::map_of(st, f.place)?;
    (foes::standing(st, f.place).into_iter())
        .filter(|j| st.foes[*j].fighter())
        .filter(|j| foes::line_clear(&map, f.pos, st.foes[*j].pos))
        .min_by_key(|j| (f.pos.chebyshev(st.foes[*j].pos), *j))
}

/// The turret you hold that foe `i` fires on: the nearest standing one on its place
/// within its weapon's range, with a clear line, that it sees. None for a foe that
/// doesn't fight (`Foe::fighter`).
pub fn guard_mark(st: &AppState, i: usize) -> Option<usize> {
    let g = st.foes.get(i).filter(|g| g.fighter())?;
    let map = foes::map_of(st, g.place)?;
    let reach = foes::weapon_range(st, g);
    (super::allies(st, g.place).into_iter())
        .filter(|j| {
            let p = st.foes[*j].pos;
            g.pos.chebyshev(p) <= reach
                && foes::line_clear(&map, g.pos, p)
                && foes::sees(&map, g, p)
        })
        .min_by_key(|j| (g.pos.chebyshev(st.foes[*j].pos), *j))
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
        Some(Hacked::Yours) => in_its_fight(st, i),
        _ => false,
    }
}

/// Turret `i`'s whole turn, one action: it turns one eighth toward its target when its
/// field doesn't hold it, then fires when it has it (clicks with its magazine empty).
/// A hostile one keeps its hunt: had you, it hunts on; else it counts a turn toward
/// losing you. One you hold does the same at its guard (`mark`) in a fight on its place,
/// and nothing outside one.
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
        // A hostile turret's lines warn you; one you hold only reports.
        let tell = |st: &mut AppState, line: String| {
            if hostile {
                st.warn(line);
            } else {
                st.say(line);
            }
        };
        if p.fires {
            had = Some(p.target);
            if dry {
                let line = match &lead {
                    Some(lead) => format!("{name} {lead} › clicks dry"),
                    None => format!("{name} clicks dry"),
                };
                tell(st, line);
            } else if let Some(g) = p.mark {
                crate::combat::fire_on_foe(st, i, g, lead.as_deref());
            } else {
                crate::combat::fire_at_you_with(st, i, lead.as_deref());
            }
        } else if let Some(lead) = lead {
            tell(st, format!("{name} {lead}"));
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
        assert!(logged(&st, "turret fires · miss · miss"));
        assert!(!logged(&st, "miss · miss · miss"));
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
        for awake in [false, true] {
            let (mut st, i) = in_ward(Pos::new(12, 3), Pos::new(10, 3), Dir::W);
            if awake {
                crate::turrets::told(&mut st, on(0), Pos::new(4, 3));
            }
            let want = if awake {
                TurretState::Awake
            } else {
                TurretState::Asleep
            };
            assert_eq!(crate::turrets::state(&st.foes[i]), Some(want));
            assert!(!crate::turrets::seated(&st, i), "{want:?}");
            crate::net::engage(&mut st, on(0), true);
            crate::net::seat_foes(&mut st);
            let e = st.net.engaged.as_mut().unwrap();
            assert!(!e.order.contains(&crate::net::Actor::Foe(i)), "{want:?}");
            // One still in the order (it lost you) is passed over by the column and readout.
            e.order.push(crate::net::Actor::Foe(i));
            let view = crate::net::turn_view(&st).unwrap();
            assert!(view.chips.iter().all(|c| c.name != "turret"), "{want:?}");
            assert!(
                !crate::net::turn_readout(&st).unwrap().contains("turret"),
                "{want:?}"
            );
        }
    }

    #[test]
    fn hunting_it_counts_on_its_floor() {
        let (mut st, i) = in_ward(Pos::new(6, 3), Pos::new(10, 3), Dir::W);
        assert!(!crate::foes::hunting_on(&st, on(0)).contains(&i));
        crate::foes::look_all(&mut st);
        assert!(crate::foes::hunting_on(&st, on(0)).contains(&i));
        assert!(crate::turrets::seated(&st, i));
    }

    fn yours_in_ward(st: &mut AppState, at: Pos, facing: Dir) -> usize {
        let i = turret_in_ward(st, 0, at, facing);
        st.foes[i].turret.as_mut().unwrap().hacked = crate::turrets::Hacked::Yours;
        let node = st.foes[i].turret.as_ref().unwrap().node;
        let now = st.net.now(st);
        st.net.holds.push(crate::holds::Hold {
            node: citygen::NetNode::Site(node),
            owner: crate::owners::Owner::Building(WARD),
            until: now + game::HACK_MINUTES,
            used: Vec::new(),
        });
        i
    }

    #[test]
    fn outside_a_fight_yours_does_nothing() {
        let mut st = ward();
        // Past the card lock, out of the guard's sight: nothing hunts you.
        stand(&mut st, 0, Pos::new(20, 3));
        let t = yours_in_ward(&mut st, Pos::new(10, 3), Dir::W);
        let g = guard(&mut st, 0, 0, &[Pos::new(6, 3)]);
        st.foes[g].post.as_mut().unwrap().round.next = u64::MAX;
        for _ in 0..100 {
            advance(&mut st, 1);
            crate::turrets::tick(&mut st);
        }
        // Its own turn, with no fight on: no line, no shot.
        let lines = st.log.len();
        assert_eq!(step(&mut st, t), Step::Nothing);
        assert_eq!(st.log.len(), lines);
        assert_eq!(st.foes[t].turret.as_ref().unwrap().rounds, 30);
        assert_eq!(st.foes[g].hp, 30);
        // Nor does it start one.
        assert!(!crate::foes::look_all(&mut st));
        assert!(st.net.engaged.is_none());
    }

    #[test]
    fn in_a_fight_on_its_floor_it_sits_on_your_side_and_fires_on_the_guards() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(2, 5));
        let t = yours_in_ward(&mut st, Pos::new(10, 3), Dir::W);
        let g = guard(&mut st, 0, 0, &[Pos::new(6, 3)]);
        crate::net::engage(&mut st, on(0), true);
        let e = st.net.engaged.as_ref().unwrap();
        assert!(e.order.contains(&crate::net::Actor::Foe(t)));
        let chip = |st: &AppState| {
            crate::net::turn_view(st)
                .unwrap()
                .chips
                .into_iter()
                .find(|c| c.name == "turret")
                .unwrap()
        };
        assert!(chip(&st).ours);
        st.force_hit = Some(true);
        step(&mut st, t);
        assert!(logged(&st, "turret fires on guard"));
        assert!(st.foes[g].hp < 30);
    }

    #[test]
    fn it_never_fires_on_you() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(6, 3)); // in its field
        let t = yours_in_ward(&mut st, Pos::new(10, 3), Dir::W);
        crate::net::engage(&mut st, on(0), true);
        let hp = st.character.hp;
        step(&mut st, t);
        assert_eq!(
            (st.character.hp, st.foes[t].turret.as_ref().unwrap().rounds),
            (hp, 30)
        );
    }

    #[test]
    fn a_guard_fires_on_your_turret_when_it_is_nearer_than_you() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(2, 3));
        let t = yours_in_ward(&mut st, Pos::new(10, 3), Dir::W);
        let g = guard(&mut st, 0, 0, &[Pos::new(8, 3)]);
        st.foes[g].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(2, 3),
            lost: 0,
        };
        assert_eq!(guard_mark(&st, g), Some(t));
        st.force_hit = Some(true);
        crate::foes::step(&mut st, g, true, true);
        assert!(logged(&st, "guard fires on turret"));
    }

    #[test]
    fn a_guard_fires_on_you_when_you_are_nearer_and_on_the_turret_when_it_cant_reach_you() {
        for (you, on_turret) in [(Pos::new(7, 3), false), (Pos::new(20, 3), true)] {
            let mut st = ward();
            stand(&mut st, 0, you);
            let t = yours_in_ward(&mut st, Pos::new(10, 3), Dir::W);
            let g = guard(&mut st, 0, 0, &[Pos::new(8, 3)]);
            st.foes[g].mode = crate::foes::FoeMode::Hunting { seen: you, lost: 0 };
            st.force_hit = Some(true);
            crate::foes::step(&mut st, g, true, true);
            assert_eq!(logged(&st, "guard fires on turret"), on_turret, "{you:?}");
            assert_eq!(st.foes[t].turret.as_ref().unwrap().arm < 4, on_turret);
        }
    }

    #[test]
    fn a_guard_out_of_sight_that_hears_a_turret_fire_comes_toward_it() {
        // You in its field in the west hall; the guard in the dark east hall, the wall
        // between it and you both.
        let (mut st, t) = in_ward(Pos::new(6, 3), Pos::new(12, 3), Dir::W);
        let g = guard(&mut st, 0, 1, &[Pos::new(20, 1)]);
        let at = st.foes[t].pos;
        crate::combat::fire_at_you_with(&mut st, t, None);
        assert_eq!(
            st.foes[g].mode,
            crate::foes::FoeMode::Hunting { seen: at, lost: 0 }
        );
        assert!(st.net.engaged.is_none());
        let start = st.foes[g].pos.chebyshev(at);
        run_until(&mut st, 400, |st| st.foes[g].pos.chebyshev(at) <= start - 3);
    }

    #[test]
    fn a_guard_your_turret_hits_from_out_of_its_sight_closes_in_and_fires_on_it() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(2, 5));
        let t = yours_in_ward(&mut st, Pos::new(10, 3), Dir::E);
        // In the dark east hall, the wall between it and the turret.
        let g = guard(&mut st, 0, 1, &[Pos::new(20, 1)]);
        crate::net::engage(&mut st, on(0), true);
        assert!(guard_mark(&st, g).is_none());
        crate::combat::fire_on_foe(&mut st, t, g, None);
        let at = st.foes[t].pos;
        assert_eq!(
            st.foes[g].mode,
            crate::foes::FoeMode::Hunting { seen: at, lost: 0 }
        );
        for _ in 0..20 {
            if logged(&st, "guard fires on turret") {
                break;
            }
            crate::foes::step(&mut st, g, true, true);
        }
        assert!(logged(&st, "guard fires on turret"));
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
