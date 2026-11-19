//! A turret on the clock: asleep until a camera's word or an alarm wakes it, then
//! turning toward where you were, and asleep again once nothing has been seen.

use super::Hacked;
use super::field::{aim, in_field, turn_toward};
use crate::app::AppState;
use crate::foes::FoeMode;
use crate::guards::GUARD_STEP_TICKS;
use citygen::{NetNode, Place, Pos};

/// The holds read back onto turret `i`. Blind or yours, while the hold on its node lasts,
/// it hunts nothing and holds no fight open. Once the hold has run out it is its own
/// again: hunting you if it has you in its field with a fight on its place, awake while
/// its building is on alert, else asleep. It says so where you remember its tile, or in
/// its building or on its Net, where you'd have held it from; a wreck goes back without a
/// word.
pub(super) fn sync(st: &mut AppState, i: usize) {
    let Some(t) = st.foes.get(i).and_then(|f| f.turret.as_ref()) else {
        return;
    };
    if t.hacked == Hacked::None {
        return;
    }
    let (node, facing) = (t.node, t.facing);
    let now = st.net.now(st);
    let held = crate::holds::held(st, NetNode::Site(node), now).is_some();
    let f = &mut st.foes[i];
    f.mode = FoeMode::Idle;
    if held {
        return;
    }
    let (place, down) = (f.place, f.down);
    if let Some(t) = f.turret.as_mut() {
        t.hacked = Hacked::None;
        t.awake = false;
        t.told = None;
    }
    if down {
        return;
    }
    let b = node.building;
    let near = st.player.as_ref().is_some_and(|p| {
        matches!(p.place, Place::Floor { building, .. } if building == b)
            || p.place.construct() == Some(b)
    });
    if near || !crate::foes::unseen(st, i) {
        st.say("turret's back on its own net");
    }
    let fight = st.net.engaged.as_ref().is_some_and(|e| e.place == place);
    let you = (st.player.as_ref())
        .filter(|me| me.place == place)
        .map(|me| me.pos)
        .filter(|you| fight && super::turn::has(st, i, facing, *you));
    if let Some(you) = you {
        super::has_you(st, i, you);
    } else if st.net.alert(node.building, now) {
        wake(st, i, None);
    }
}

/// Wakes turret `i` toward `told`, logging it when you remember its tile.
pub(super) fn wake(st: &mut AppState, i: usize, told: Option<Pos>) {
    let minute = st.net.now(st);
    let tick = st.clock_tick();
    let seen = !crate::foes::unseen(st, i);
    if let Some(t) = st.foes[i].turret.as_mut() {
        t.awake = true;
        t.told = told;
        t.since = minute;
        t.next = tick + GUARD_STEP_TICKS;
    }
    if seen {
        st.say("turret wakes");
    }
}

/// Each frame: the turrets of the city asleep, awake and sweeping.
pub fn tick(st: &mut AppState) {
    let minute = st.net.now(st);
    let tick = st.clock_tick();
    let you = st.player.as_ref().map(|p| p.place);
    for i in 0..st.foes.len() {
        if st.foes[i].turret.is_none() {
            continue;
        }
        sync(st, i);
        let f = &st.foes[i];
        if !super::hostile(f) {
            continue;
        }
        let (place, at) = (f.place, f.pos);
        let t = f.turret.as_ref().expect("a turret");
        let (building, awake) = (t.node.building, t.awake);
        let alert = st.net.alert(building, minute);
        let fight = st.net.engaged.as_ref().is_some_and(|e| e.place == place);
        // Hunting is awake, whatever set it (a look, a blast's noise, an arc): off a fight
        // with you gone it loses you, stunned or not, and turns toward where it had you.
        if let FoeMode::Hunting { seen, .. } = f.mode {
            let lost = !fight && you != Some(place);
            let f = &mut st.foes[i];
            if lost {
                f.mode = FoeMode::Idle;
            }
            if let Some(t) = f.turret.as_mut() {
                t.awake = true;
                if lost {
                    t.told = Some(seen);
                    t.since = minute;
                    t.next = tick + GUARD_STEP_TICKS;
                }
            }
            continue;
        }
        if f.stunned > 0 {
            continue;
        }
        if !awake {
            if alert {
                wake(st, i, None);
            }
            continue;
        }
        let (facing, told, next, since) = (t.facing, t.told, t.next, t.since);
        if tick >= next && !fight {
            let told = told.filter(|p| !in_field(at, facing, *p));
            let goal = told.map(|p| aim(at, facing, p));
            let turned = goal.map_or_else(|| clockwise(facing), |g| turn_toward(facing, g));
            let f = &mut st.foes[i];
            f.glyph = super::field::glyph(turned);
            if let Some(t) = f.turret.as_mut() {
                t.told = told;
                t.facing = turned;
                t.next = tick + GUARD_STEP_TICKS;
            }
        }
        // sleep
        if minute >= since + crate::net::LOST_TURNS as u64
            && !alert
            && !fight
            && let Some(t) = st.foes[i].turret.as_mut()
        {
            t.awake = false;
            t.told = None;
        }
    }
}

/// One eighth clockwise.
fn clockwise(facing: citygen::Dir) -> citygen::Dir {
    let i = citygen::Dir::ALL
        .iter()
        .position(|d| *d == facing)
        .unwrap_or(0);
    citygen::Dir::ALL[(i + 1) % 8]
}

/// A word that you are at `at` on `place`: every asleep turret there wakes toward it, an
/// awake one has its word and its minute renewed.
pub fn told(st: &mut AppState, place: Place, at: Pos) {
    let minute = st.net.now(st);
    for i in 0..st.foes.len() {
        let f = &st.foes[i];
        if f.place != place || !super::hostile(f) {
            continue;
        }
        match f.turret.as_ref() {
            Some(t) if t.awake => {
                let t = st.foes[i].turret.as_mut().expect("a turret");
                t.told = Some(at);
                t.since = minute;
            }
            Some(_) => wake(st, i, Some(at)),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::testutil::*;
    use crate::turrets::testutil::turret_in_ward;
    use citygen::Dir;

    #[test]
    fn an_asleep_turret_neither_turns_nor_wakes_on_its_own() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(3, 1));
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::E);
        for _ in 0..200 {
            advance(&mut st, 1);
            tick(&mut st);
        }
        let t = st.foes[i].turret.as_ref().unwrap();
        assert_eq!((t.facing, t.awake), (Dir::E, false));
    }

    #[test]
    fn a_cameras_word_wakes_it_and_it_turns_toward_where_you_were() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::E);
        told(&mut st, on(0), Pos::new(4, 3));
        assert!(st.foes[i].turret.as_ref().unwrap().awake);
        // NW and SW both hold (4,3) three eighths off E; clockwise first: SW, via SE.
        for _ in 0..=crate::guards::GUARD_STEP_TICKS {
            advance(&mut st, 1);
            tick(&mut st);
        }
        assert_eq!(st.foes[i].turret.as_ref().unwrap().facing, Dir::SE);
    }

    #[test]
    fn facing_the_tile_it_was_told_of_it_sweeps_on_clockwise() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
        told(&mut st, on(0), Pos::new(6, 3)); // already in its field
        let steps = |st: &mut AppState, n: u64| {
            for _ in 0..n * crate::guards::GUARD_STEP_TICKS + 1 {
                advance(st, 1);
                tick(st);
            }
        };
        steps(&mut st, 1);
        assert_eq!(st.foes[i].turret.as_ref().unwrap().facing, Dir::NW);
        steps(&mut st, 1);
        assert_eq!(st.foes[i].turret.as_ref().unwrap().facing, Dir::N);
    }

    #[test]
    fn an_alarm_wakes_it_and_holds_it_awake() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::E);
        crate::net::lock_down(&mut st, crate::guards::testutil::WARD);
        advance(&mut st, 1);
        tick(&mut st);
        assert!(st.foes[i].turret.as_ref().unwrap().awake);
        crate::sitenet::testutil::pass_minutes(&mut st, crate::net::LOST_TURNS as u64 + 1);
        tick(&mut st);
        assert!(
            st.foes[i].turret.as_ref().unwrap().awake,
            "on alarm it stays awake"
        );
    }

    #[test]
    fn awake_with_nothing_seen_it_sleeps_after_three_minutes() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::E);
        told(&mut st, on(0), Pos::new(4, 3));
        crate::sitenet::testutil::pass_minutes(&mut st, crate::net::LOST_TURNS as u64);
        tick(&mut st);
        assert!(!st.foes[i].turret.as_ref().unwrap().awake);
    }

    #[test]
    fn hunting_off_a_fight_with_you_gone_it_turns_awake_toward_where_it_had_you() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
        assert!(crate::foes::look_all(&mut st), "it has you at (6,3)");
        stand(&mut st, 1, Pos::new(5, 3)); // gone upstairs, no fight
        advance(&mut st, 1);
        tick(&mut st);
        let f = &st.foes[i];
        assert_eq!(
            crate::turrets::state(f),
            Some(crate::turrets::TurretState::Awake)
        );
        assert_eq!(f.turret.as_ref().unwrap().told, Some(Pos::new(6, 3)));
        // Awake with nothing seen, it sleeps after `LOST_TURNS` minutes.
        crate::sitenet::testutil::pass_minutes(&mut st, crate::net::LOST_TURNS as u64);
        tick(&mut st);
        assert!(!st.foes[i].turret.as_ref().unwrap().awake);
    }

    #[test]
    fn a_blast_wakes_it_hunting_and_off_a_fight_it_loses_you_then_sleeps() {
        let mut st = ward();
        // In the dark east hall facing away from you: it never sees you.
        let i = turret_in_ward(&mut st, 0, Pos::new(20, 3), Dir::E);
        let frag = st.content.item(&game::ItemId::new("frag")).unwrap().clone();
        let (_, _, blast) = frag.throw().unwrap();
        let now = st.clock_tick();
        crate::blast::detonate(
            &mut st,
            on(0),
            Pos::new(20, 3),
            blast,
            &frag.fx,
            &frag.name,
            now,
            None,
        );
        assert_eq!(
            st.foes[i].mode,
            FoeMode::Hunting {
                seen: Pos::new(6, 3),
                lost: 0
            },
            "the bang turns it toward you"
        );
        advance(&mut st, 1);
        tick(&mut st);
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(crate::turrets::TurretState::Hunting)
        );
        assert!(
            st.foes[i].turret.as_ref().unwrap().awake,
            "hunting is awake"
        );
        stand(&mut st, 1, Pos::new(5, 3)); // gone upstairs, no fight
        advance(&mut st, 1);
        tick(&mut st);
        let f = &st.foes[i];
        assert_eq!(
            crate::turrets::state(f),
            Some(crate::turrets::TurretState::Awake)
        );
        assert_eq!(f.turret.as_ref().unwrap().told, Some(Pos::new(6, 3)));
        crate::sitenet::testutil::pass_minutes(&mut st, crate::net::LOST_TURNS as u64);
        tick(&mut st);
        assert_eq!(
            crate::turrets::state(&st.foes[i]),
            Some(crate::turrets::TurretState::Asleep)
        );
    }

    #[test]
    fn awake_it_doesnt_turn_while_a_fight_holds_its_floor() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::E);
        told(&mut st, on(0), Pos::new(4, 3));
        crate::net::engage(&mut st, on(0), true);
        for _ in 0..=3 * crate::guards::GUARD_STEP_TICKS {
            advance(&mut st, 1);
            tick(&mut st);
        }
        let t = st.foes[i].turret.as_ref().unwrap();
        assert_eq!((t.facing, t.awake), (Dir::E, true));
    }

    #[test]
    fn awake_it_takes_no_seat_and_holds_no_fight() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::E);
        told(&mut st, on(0), Pos::new(20, 3));
        assert!(!crate::foes::any_hunting(&st, on(0)));
        assert!(!crate::foes::hunting_on(&st, on(0)).contains(&i));
    }
}
