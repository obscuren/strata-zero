//! The cloak (#36): on foot, nobody and nothing sees you until your first hit. It runs
//! on FOCUS (the Stealth skill's Cloak) or on a field scrambler's cell, draining by the
//! map's frames, and ends by itself on a fight, the Net or an empty pool. The veil is its
//! look; its fade is UI and isn't saved.

use crate::app::AppState;
use citygen::{Place, Pos, StratumKind};
use game::{AbilityId, CopyId, FULL_CELL, Source};
use serde::{Deserialize, Serialize};

pub mod cell;
#[cfg(test)]
pub(crate) mod testutil;

/// Frame ticks on one tile before you count as standing still.
pub const STILL_TICKS: u64 = 15;
/// A frame's drain moving.
pub const DRAIN_MOVING: u32 = 2;
/// A frame's drain standing still with Blind spot.
pub const DRAIN_STILL: u32 = 1;
/// The drain one FOCUS pays for: 30 frames moving, a second.
pub const FOCUS_UNITS: u32 = 60;
/// The veil's phase lengths in frame ticks: going in, burning out, switching off.
pub const START_TICKS: u64 = 33;
pub const BREAK_TICKS: u64 = 72;
pub const SNAP_TICKS: u64 = 15;
/// How hot Vanish's free cloak runs: a calm drift.
pub const VANISH_HEAT: f32 = 0.2;

/// The cloak that's on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cloak {
    pub pool: Pool,
    /// The frame tick it went on.
    pub since: u64,
    /// The frame tick it last drained.
    #[serde(default)]
    pub drained_to: u64,
    /// Your tile at the last frame.
    #[serde(default)]
    pub at: Pos,
    /// The frame tick you last changed tile.
    #[serde(default)]
    pub moved: u64,
    /// On turns you started by hand from it: it holds until your attack.
    #[serde(default)]
    pub waiting: bool,
}

/// What a cloak runs on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pool {
    /// FOCUS: what you had when it went on, and the drain owed toward the next point.
    Focus { start: u32, owed: u32 },
    /// A carried scrambler copy's cell.
    Cell(CopyId),
    /// Vanish's: free until the frame tick.
    Free { until: u64 },
}

/// The veil's state: UI, never saved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VeilState {
    pub fade: Option<Fade>,
    /// Tests and shots keep the map plain.
    pub off: bool,
}

/// A phase of the veil playing out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    pub phase: Phase,
    /// The frame tick it began.
    pub since: u64,
    /// The heat it plays from.
    pub hot: f32,
    /// The cloak that made it ran on a scrambler's cell.
    pub cell: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    #[expect(
        dead_code,
        reason = "the veil holds between its fades, once it's drawn"
    )]
    Holding,
    GoingIn,
    BurningOut,
    SwitchingOff,
}

/// How a cloak ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Switched off: by its key, the Net, a jack-in, its time up.
    Off,
    /// The pool ran dry.
    Burnout,
    /// Your first hit from it.
    #[expect(dead_code, reason = "the ambush ends it, once your rounds take it")]
    Ambush,
    /// A fight started.
    Fight,
}

/// A phase's length in frame ticks; holding has none.
pub fn fade_ticks(phase: Phase) -> u64 {
    match phase {
        Phase::Holding => 0,
        Phase::GoingIn => START_TICKS,
        Phase::BurningOut => BREAK_TICKS,
        Phase::SwitchingOff => SNAP_TICKS,
    }
}

/// Whether you're cloaked. A burnout's fade isn't: you're in the open from its start.
pub fn hidden(st: &AppState) -> bool {
    st.cloak.is_some()
}

/// How empty the cloak's pool is, 0 to 1; None with no cloak on.
pub fn heat(st: &AppState) -> Option<f32> {
    let c = st.cloak.as_ref()?;
    Some(match c.pool {
        Pool::Focus { start: 0, .. } => 0.0,
        Pool::Focus { start, .. } => start.saturating_sub(st.character.focus) as f32 / start as f32,
        Pool::Cell(id) => {
            1.0 - cell::charge(st, id).map_or(0, |c| c.left) as f32 / FULL_CELL as f32
        }
        Pool::Free { .. } => VANISH_HEAT,
    })
}

/// A cell's charge as a percentage: 0 only when empty.
pub fn pct(left: u32) -> u32 {
    (left.min(FULL_CELL) * 100).div_ceil(FULL_CELL)
}

/// Where no cloak works: inside a construct or on the Net.
fn on_the_net(place: Place) -> bool {
    place.in_construct() || place == Place::Stratum(StratumKind::Net)
}

/// A cloak key: switches the cloak that's on off, or this key's on. The line for the
/// status strip, or the refusal.
pub fn toggle(st: &mut AppState, id: &AbilityId) -> Result<String, String> {
    let source = (st.character.abilities(&st.content).into_iter())
        .find(|(have, _)| have == id)
        .map(|(_, source)| source);
    let Some(source) = source else {
        let name = (st.character.ability(&st.content, id))
            .map_or_else(|| id.to_string(), |a| a.name.clone());
        return Err(format!("you do not have {name}"));
    };
    if let Some(c) = st.cloak.as_ref() {
        let line = match c.pool {
            Pool::Cell(copy) => format!(
                "scrambler off · {}% left",
                pct(cell::charge(st, copy).map_or(0, |c| c.left))
            ),
            Pool::Focus { .. } | Pool::Free { .. } => "cloak off".to_string(),
        };
        end(st, End::Off);
        return Ok(line);
    }
    let Some((place, at)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return Err("no body in the city".to_string());
    };
    if crate::actions::jacked_in(st) {
        return Err("not while jacked in".to_string());
    }
    if on_the_net(place) {
        return Err("not on the Net".to_string());
    }
    if st.net.engaged.is_some() {
        return Err("not in a fight".to_string());
    }
    // A scrambler's key runs on the fullest carried copy of its item, any other on FOCUS.
    let pool = if let Source::Item(item) = &source
        && st.content.item(item).is_some_and(|i| i.cell)
    {
        match cell::best(st, item) {
            Some((copy, c)) if cell::runs(c) => Pool::Cell(copy),
            Some((_, c)) if c.left > 0 => return Err("the cell is burned out".to_string()),
            _ => return Err("the cell is empty".to_string()),
        }
    } else if st.character.focus == 0 {
        return Err("no FOCUS left".to_string());
    } else {
        Pool::Focus {
            start: st.character.focus,
            owed: 0,
        }
    };
    let on_cell = matches!(pool, Pool::Cell(_));
    st.cloak = Some(Cloak {
        pool,
        since: st.tick,
        drained_to: st.tick,
        at,
        moved: st.tick,
        waiting: false,
    });
    st.veil.fade = Some(Fade {
        phase: Phase::GoingIn,
        since: st.tick,
        hot: heat(st).unwrap_or(0.0),
        cell: on_cell,
    });
    Ok(if on_cell { "SCRAMBLED" } else { "CLOAKED" }.to_string())
}

/// This frame's drain: moving, or standing still with Blind spot.
pub(crate) fn units(st: &AppState) -> u32 {
    let still = st
        .cloak
        .as_ref()
        .is_some_and(|c| st.tick.saturating_sub(c.moved) >= STILL_TICKS);
    if still && st.character.flags(&st.content).contains("blind_spot") {
        DRAIN_STILL
    } else {
        DRAIN_MOVING
    }
}

/// The cloak's frame on the map: a finished fade dropped, the cloak ended where it
/// can't be, else one frame drained. Time off the map (the deck, a menu) costs nothing:
/// however far the tick moved, a frame drains one frame.
pub fn frame(st: &mut AppState) {
    if let Some(f) = st.veil.fade
        && st.tick.saturating_sub(f.since) >= fade_ticks(f.phase)
    {
        st.veil.fade = None;
        st.repaint = true;
    }
    let Some(c) = st.cloak.as_ref() else {
        return;
    };
    let body = st.player.as_ref().map(|p| (p.place, p.pos));
    let gone = match c.pool {
        Pool::Cell(id) => st.character.copy_in_pack(id).is_none(),
        Pool::Free { until } => st.tick >= until,
        Pool::Focus { .. } => false,
    };
    let Some((place, pos)) = body else {
        end(st, End::Off);
        return;
    };
    if gone || crate::actions::jacked_in(st) || on_the_net(place) {
        end(st, End::Off);
        return;
    }
    let tick = st.tick;
    let c = st.cloak.as_mut().expect("on");
    if c.at != pos {
        c.at = pos;
        c.moved = tick;
    }
    if tick <= c.drained_to {
        return;
    }
    c.drained_to = tick;
    let units = units(st);
    let rate = cell::rate(st);
    let c = st.cloak.as_mut().expect("on");
    match &mut c.pool {
        Pool::Focus { owed, .. } => {
            *owed += units;
            while *owed >= FOCUS_UNITS && st.character.focus > 0 {
                *owed -= FOCUS_UNITS;
                st.character.focus -= 1;
            }
            if st.character.focus == 0 {
                end(st, End::Burnout);
            }
        }
        // The copy is in the pack: a gone one ended it above.
        Pool::Cell(id) => {
            let id = *id;
            if cell::drain(st, id, units * rate) {
                end(st, End::Burnout);
            }
        }
        // Vanish's costs nothing: it runs out on its tick, above.
        Pool::Free { .. } => {}
    }
}

/// Ends the cloak, starting its fade from the heat it had; false with none on. A
/// burnout warns that you're in the open.
pub fn end(st: &mut AppState, why: End) -> bool {
    let hot = heat(st).unwrap_or(0.0);
    let Some(c) = st.cloak.take() else {
        return false;
    };
    let cell = matches!(c.pool, Pool::Cell(_));
    let (phase, hot) = match why {
        End::Off | End::Ambush | End::Fight => (Phase::SwitchingOff, hot),
        End::Burnout => (Phase::BurningOut, 1.0),
    };
    st.veil.fade = Some(Fade {
        phase,
        since: st.tick,
        hot,
        cell,
    });
    if why == End::Burnout {
        let what = if cell { "scrambler" } else { "cloak" };
        st.warn(format!("the {what} burns out · you're in the open"));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;
    use citygen::{Place, Pos, StratumKind};

    #[test]
    fn the_cloak_goes_on_and_off_from_its_slot() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 20;
        assert_eq!(fire(&mut st, "cloak"), Ok("CLOAKED".to_string()));
        assert!(hidden(&st));
        assert!(matches!(
            st.cloak.as_ref().unwrap().pool,
            Pool::Focus { start: 20, owed: 0 }
        ));
        assert_eq!(st.veil.fade.unwrap().phase, Phase::GoingIn);
        assert!(
            !st.character
                .fired
                .contains_key(&game::AbilityId::new("cloak")),
            "no cooldown stamped"
        );
        assert_eq!(fire(&mut st, "cloak"), Ok("cloak off".to_string()));
        assert!(!hidden(&st));
        assert_eq!(st.veil.fade.unwrap().phase, Phase::SwitchingOff);
    }

    #[test]
    fn the_cloak_refuses_a_fight_the_net_and_no_focus() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 0;
        assert_eq!(fire(&mut st, "cloak"), Err("no FOCUS left".to_string()));
        st.character.focus = 5;
        let here = st.player.as_ref().unwrap().place;
        crate::net::engage(&mut st, here, true);
        assert_eq!(fire(&mut st, "cloak"), Err("not in a fight".to_string()));
        crate::net::disengage(&mut st);
        st.player.as_mut().unwrap().place = Place::Stratum(StratumKind::Net);
        assert_eq!(fire(&mut st, "cloak"), Err("not on the Net".to_string()));
        assert!(st.cloak.is_none());
    }

    #[test]
    fn a_cloak_key_you_dont_own_refuses() {
        let mut st = street();
        st.character.focus = 20;
        assert_eq!(
            fire(&mut st, "cloak"),
            Err("you do not have Cloak".to_string())
        );
        assert_eq!(
            fire(&mut st, "scramble"),
            Err("you do not have Scramble".to_string())
        );
        assert!(st.cloak.is_none());
        assert_eq!(st.character.focus, 20);
    }

    #[test]
    fn focus_drains_a_point_a_second_and_burns_out_at_zero() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 2;
        fire(&mut st, "cloak").unwrap();
        frames(&mut st, 30);
        assert_eq!(st.character.focus, 1);
        frames(&mut st, 30);
        assert_eq!(st.character.focus, 0);
        assert!(st.cloak.is_none());
        assert_eq!(st.veil.fade.unwrap().phase, Phase::BurningOut);
        assert!(crate::guards::testutil::logged(
            &st,
            "the cloak burns out · you're in the open"
        ));
    }

    #[test]
    fn blind_spot_halves_the_drain_standing_still() {
        let mut st = street();
        stealthy(&mut st);
        st.character
            .talents
            .insert(game::TalentId::new("blind_spot"), 1);
        st.character.focus = 10;
        fire(&mut st, "cloak").unwrap();
        frames(&mut st, STILL_TICKS + 60);
        // 14 frames moving (28), then 61 standing still (61): 89 owed, one FOCUS gone.
        // Without Blind spot the same 75 frames owe 150: two.
        assert_eq!(st.character.focus, 9);
    }

    #[test]
    fn time_off_the_map_costs_nothing() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 10;
        fire(&mut st, "cloak").unwrap();
        st.tick += 3000; // the deck was open
        frame(&mut st);
        assert_eq!(st.character.focus, 10);
        assert!(matches!(
            st.cloak.as_ref().unwrap().pool,
            Pool::Focus { owed: 2, .. }
        ));
    }

    #[test]
    fn a_fight_starting_and_jacking_in_switch_it_off() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 10;
        fire(&mut st, "cloak").unwrap();
        let here = st.player.as_ref().unwrap().place;
        crate::net::engage(&mut st, here, false);
        assert!(st.cloak.is_none());
        assert_eq!(st.veil.fade.unwrap().phase, Phase::SwitchingOff);
        crate::net::disengage(&mut st);
        fire(&mut st, "cloak").unwrap();
        crate::net::after_jack_in(&mut st, here, None);
        assert!(st.cloak.is_none());
    }

    #[test]
    fn moving_keeps_it_on() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 10;
        fire(&mut st, "cloak").unwrap();
        st.player.as_mut().unwrap().pos = Pos::new(7, 8);
        frames(&mut st, 5);
        assert!(hidden(&st));
        assert_eq!(st.cloak.as_ref().unwrap().at, Pos::new(7, 8));
    }

    #[test]
    fn the_fade_ends_and_repaints() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 10;
        fire(&mut st, "cloak").unwrap();
        fire(&mut st, "cloak").unwrap();
        st.repaint = false;
        frames(&mut st, 15);
        assert!(st.veil.fade.is_none() && st.repaint);
    }
}
