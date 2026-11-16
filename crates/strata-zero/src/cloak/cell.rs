//! The field scrambler's cell (#36): the cloak it runs drains the copy's own charge,
//! faster without the Stealth skill; off, the cell fills back on the world clock, and a
//! night's sleep fills it.

use super::Pool;
use crate::app::AppState;
use game::{Charge, CopyId, FULL_CELL, ItemId, SkillId};

/// What a drain unit costs a cell: a full one lasts 600 frames moving, 20 s.
pub const CELL_RATE: u32 = 9;
/// With the Stealth skill: 900 frames, 30 s.
pub const CELL_RATE_SKILLED: u32 = 6;
/// What a cell gains a clock tick while off: empty to full in 2700 ticks, 30 game
/// minutes.
pub const CELL_RECHARGE: u32 = 4;

/// What a drain unit costs the cell: less with the Stealth skill.
pub fn rate(st: &AppState) -> u32 {
    if st.character.skills.contains(&SkillId::new("stealth")) {
        CELL_RATE_SKILLED
    } else {
        CELL_RATE
    }
}

/// Whether a cell can run a cloak: charge left, and not burned out (one that burned out
/// runs again once it recharges past a tenth).
pub fn runs(c: Charge) -> bool {
    c.left > 0 && !c.burned
}

/// The carried copy of `item` a cloak runs on: the one with the most left of those
/// that can run, else (none can) the one with the most left. Ties by pack order.
pub fn best(st: &AppState, item: &ItemId) -> Option<(CopyId, Charge)> {
    let mut best: Option<(CopyId, Charge)> = None;
    for s in (st.character.inventory.iter()).filter(|s| s.item == *item) {
        let Some(k) = s.copy.as_ref() else { continue };
        let c = k.cell();
        if best.is_none_or(|(_, b)| (runs(c), c.left) > (runs(b), b.left)) {
            best = Some((k.id, c));
        }
    }
    best
}

/// The charge on a carried copy's cell; None when the pack holds no such copy.
pub fn charge(st: &AppState, id: CopyId) -> Option<Charge> {
    Some(st.character.copy_in_pack(id)?.copy.as_ref()?.cell())
}

/// The charge on a carried copy's cell, to write: one with none recorded is full.
fn charge_mut(st: &mut AppState, id: CopyId) -> Option<&mut Charge> {
    let s = (st.character.inventory.iter_mut()).find(|s| s.copy_id() == Some(id))?;
    let k = s.copy.as_mut()?;
    let now = k.cell();
    Some(k.charge.get_or_insert(now))
}

/// Takes `cost` off a carried copy's cell; true when that empties it, burned out.
pub(crate) fn drain(st: &mut AppState, id: CopyId, cost: u32) -> bool {
    let Some(c) = charge_mut(st, id) else {
        return false;
    };
    c.left = c.left.saturating_sub(cost);
    c.burned |= c.left == 0;
    c.left == 0
}

/// `clock_ticks` of the world clock: every carried cell but the one a cloak runs on
/// gains `CELL_RECHARGE` a tick, up to full, and a burned-out one is clear past a tenth.
pub fn recharge(st: &mut AppState, clock_ticks: u64) {
    if clock_ticks == 0 {
        return;
    }
    let running = match st.cloak.as_ref().map(|c| &c.pool) {
        Some(Pool::Cell(id)) => Some(*id),
        _ => None,
    };
    let gain = clock_ticks.saturating_mul(CELL_RECHARGE as u64);
    for s in &mut st.character.inventory {
        let Some(k) = s.copy.as_mut().filter(|k| Some(k.id) != running) else {
            continue;
        };
        // A copy with no charge recorded reads full.
        let Some(c) = k.charge.as_mut() else {
            continue;
        };
        c.left = (c.left as u64).saturating_add(gain).min(FULL_CELL as u64) as u32;
        if c.left > FULL_CELL / 10 {
            c.burned = false;
        }
    }
}

/// A night's sleep: every carried cell full.
pub fn fill_cells(st: &mut AppState) {
    for s in &mut st.character.inventory {
        if let Some(c) = s.copy.as_mut().and_then(|k| k.charge.as_mut()) {
            *c = Charge {
                left: FULL_CELL,
                burned: false,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::hidden;
    use super::super::testutil::*;
    use super::*;
    use std::time::Duration;

    /// Sleeps `minutes` from now, frames until you wake, as the rest's tests do.
    fn sleep(st: &mut AppState, minutes: u64) {
        crate::rest::start(st, crate::rest::Kind::Sleep, crate::rest::now(st) + minutes);
        for _ in 0..400 {
            if st.rest.is_none() {
                return;
            }
            crate::rest::frame_within(st, Duration::from_secs(60));
        }
        panic!("still resting");
    }

    #[test]
    fn a_full_cell_lasts_twenty_seconds_or_thirty_with_stealth() {
        let mut st = street();
        let k = scrambler(&mut st);
        assert_eq!(fire(&mut st, "scramble"), Ok("SCRAMBLED".to_string()));
        assert_eq!(st.cloak.as_ref().unwrap().pool, Pool::Cell(k));
        frames(&mut st, 599);
        assert!(hidden(&st));
        frames(&mut st, 1);
        assert!(!hidden(&st));
        assert_eq!(
            charge(&st, k),
            Some(Charge {
                left: 0,
                burned: true
            })
        );
        assert!(crate::guards::testutil::logged(
            &st,
            "the scrambler burns out · you're in the open"
        ));
        fill_cells(&mut st);
        stealthy(&mut st);
        fire(&mut st, "scramble").unwrap();
        frames(&mut st, 899);
        assert!(hidden(&st));
        frames(&mut st, 1);
        assert!(!hidden(&st));
    }

    #[test]
    fn an_empty_cell_refuses_and_recharges_in_thirty_game_minutes() {
        let mut st = street();
        let k = scrambler(&mut st);
        set_charge(
            &mut st,
            k,
            Charge {
                left: 0,
                burned: true,
            },
        );
        assert_eq!(
            fire(&mut st, "scramble"),
            Err("the cell is burned out".to_string()),
            "as the HUD reads it"
        );
        recharge(&mut st, 270);
        assert_eq!(
            charge(&st, k),
            Some(Charge {
                left: 1080,
                burned: true
            }),
            "a tenth, not past it"
        );
        assert_eq!(
            fire(&mut st, "scramble"),
            Err("the cell is burned out".to_string())
        );
        recharge(&mut st, 1);
        assert!(!charge(&st, k).unwrap().burned);
        recharge(&mut st, 30 * sim::TICKS_PER_MINUTE);
        assert_eq!(charge(&st, k).unwrap().left, FULL_CELL);
        // An empty cell that never burned out reads empty.
        set_charge(
            &mut st,
            k,
            Charge {
                left: 0,
                burned: false,
            },
        );
        assert_eq!(
            fire(&mut st, "scramble"),
            Err("the cell is empty".to_string())
        );
    }

    #[test]
    fn a_burned_cell_waits_for_a_tenth_and_a_clean_one_runs_first() {
        let mut st = street();
        let burned = scrambler(&mut st);
        let clean = scrambler(&mut st);
        let at = |pct: u32, burned: bool| Charge {
            left: FULL_CELL * pct / 100,
            burned,
        };
        set_charge(&mut st, burned, at(8, true));
        set_charge(&mut st, clean, at(4, false));
        fire(&mut st, "scramble").unwrap();
        assert_eq!(
            st.cloak.as_ref().unwrap().pool,
            Pool::Cell(clean),
            "not the burned one"
        );
        fire(&mut st, "scramble").unwrap();
        st.character
            .inventory
            .retain(|s| s.copy_id() != Some(clean));
        assert_eq!(
            fire(&mut st, "scramble"),
            Err("the cell is burned out".to_string())
        );
        recharge(&mut st, (FULL_CELL / 50 / CELL_RECHARGE + 1) as u64); // past a tenth
        assert!(!charge(&st, burned).unwrap().burned);
        assert_eq!(fire(&mut st, "scramble"), Ok("SCRAMBLED".to_string()));
        assert_eq!(st.cloak.as_ref().unwrap().pool, Pool::Cell(burned));
    }

    #[test]
    fn the_key_runs_on_a_copy_of_its_own_item() {
        let mut st = street();
        // Another cell item that grants the same key, carried after the scrambler, so
        // the key's source is the scrambler.
        let mut other = st
            .content
            .item(&ItemId::new("field_scrambler"))
            .unwrap()
            .clone();
        other.id = ItemId::new("other_scrambler");
        st.content.items.push(other);
        let own = scrambler(&mut st);
        set_charge(
            &mut st,
            own,
            Charge {
                left: FULL_CELL / 2,
                burned: false,
            },
        );
        st.character
            .give_item(&st.content, ItemId::new("other_scrambler"), 1);
        fire(&mut st, "scramble").unwrap();
        assert_eq!(
            st.cloak.as_ref().unwrap().pool,
            Pool::Cell(own),
            "not the fuller copy of another item"
        );
    }

    #[test]
    fn the_cell_in_use_doesnt_recharge_and_off_says_whats_left() {
        let mut st = street();
        let k = scrambler(&mut st);
        fire(&mut st, "scramble").unwrap();
        frames(&mut st, 300);
        recharge(&mut st, 900);
        assert_eq!(charge(&st, k).unwrap().left, FULL_CELL / 2);
        assert_eq!(
            fire(&mut st, "scramble"),
            Ok("scrambler off · 50% left".to_string())
        );
    }

    #[test]
    fn the_other_key_switches_the_cloak_off() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 10;
        scrambler(&mut st);
        fire(&mut st, "cloak").unwrap();
        assert_eq!(fire(&mut st, "scramble"), Ok("cloak off".to_string()));
        assert!(st.cloak.is_none());
        fire(&mut st, "scramble").unwrap();
        assert!(
            matches!(st.cloak.as_ref().unwrap().pool, Pool::Cell(_)),
            "the scrambler's key, its cell"
        );
    }

    #[test]
    fn dropping_the_scrambler_in_use_switches_it_off() {
        let mut st = street();
        let k = scrambler(&mut st);
        fire(&mut st, "scramble").unwrap();
        st.character.inventory.retain(|s| s.copy_id() != Some(k)); // dropped, stashed or taken apart
        frames(&mut st, 1);
        assert!(st.cloak.is_none());
    }

    #[test]
    fn a_nights_sleep_fills_the_cell() {
        let mut st = crate::jobs::testutil::state_home();
        let k = scrambler(&mut st);
        set_charge(
            &mut st,
            k,
            Charge {
                left: 100,
                burned: true,
            },
        );
        sleep(&mut st, crate::rest::NIGHT);
        assert_eq!(
            charge(&st, k),
            Some(Charge {
                left: FULL_CELL,
                burned: false
            })
        );
    }

    #[test]
    fn waking_from_a_night_fills_the_cell_not_the_clock_alone() {
        use crate::rest::{Kind, NIGHT, Why, now, start, wake};
        let low = Charge {
            left: 100,
            burned: true,
        };
        // A night lain down and woken with no clock run: only the wake can fill it.
        let full = Charge {
            left: FULL_CELL,
            burned: false,
        };
        for (kind, after) in [(Kind::Wait, low), (Kind::Sleep, full)] {
            let mut st = crate::jobs::testutil::state_home();
            // a night on, so there's one to have lain down at
            crate::clock::run(&mut st, Some(NIGHT * sim::TICKS_PER_MINUTE), 0);
            let k = scrambler(&mut st);
            set_charge(&mut st, k, low);
            let at = now(&st);
            start(&mut st, kind, at + NIGHT);
            st.rest.as_mut().unwrap().from = at - NIGHT;
            wake(&mut st, Why::Esc);
            assert_eq!(charge(&st, k), Some(after), "{kind:?}");
        }
    }

    #[test]
    fn a_nap_leaves_the_cell_to_the_clock() {
        let mut st = crate::jobs::testutil::state_home();
        let k = scrambler(&mut st);
        set_charge(
            &mut st,
            k,
            Charge {
                left: 100,
                burned: true,
            },
        );
        sleep(&mut st, 10);
        assert!(
            charge(&st, k).unwrap().left < FULL_CELL,
            "ten minutes isn't a night"
        );
    }
}
