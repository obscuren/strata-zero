//! How a turret reads: on the map, its field, its chip in the turn column, the look
//! card's line and the footer's word.

use super::{TurretState, field, state};
use crate::app::AppState;
use crate::cameras::ConeLook;
use crate::foes::Foe;
use citygen::{Dir, NetNode, Place, Pos};

/// Every turret on `place`, wrecked ones too: where it stands, where it faces, what it's
/// doing.
pub fn looks(st: &AppState, place: Place) -> Vec<(Pos, Dir, TurretState)> {
    (st.foes.iter())
        .filter(|f| f.place == place)
        .filter_map(|f| Some((f.pos, f.turret.as_ref()?.facing, state(f)?)))
        .collect()
}

/// The fields to draw on `place`: an awake or hunting turret's red where you know the
/// turret (an unseen one's would give it away), one that's yours green always; asleep,
/// blind or wrecked, none.
pub fn cones(st: &AppState, place: Place) -> Vec<(Vec<Pos>, ConeLook)> {
    let Some(map) = crate::foes::map_of(st, place) else {
        return Vec::new();
    };
    (st.foes.iter().enumerate())
        .filter(|(_, f)| f.place == place)
        .filter_map(|(i, f)| {
            let facing = f.turret.as_ref()?.facing;
            let look = match state(f)? {
                TurretState::Awake | TurretState::Hunting if !crate::foes::unseen(st, i) => {
                    ConeLook::Seeing
                }
                TurretState::Yours => ConeLook::Tapped,
                _ => return None,
            };
            Some((field::tiles(&map, f.pos, facing), look))
        })
        .collect()
}

/// The look card's line for turret `i`, after `Turret · `: its facing, plate, hit points
/// and rounds while it's awake, the minutes left on your hold while it's blind or yours.
pub fn look_line(st: &AppState, i: usize) -> String {
    let Some(f) = st.foes.get(i) else {
        return String::new();
    };
    let (Some(t), Some(s)) = (f.turret.as_ref(), state(f)) else {
        return String::new();
    };
    let left = || {
        let now = st.net.now(st);
        crate::holds::held(st, NetNode::Site(t.node), now)
            .map_or(String::new(), |h| format!(" · {} min", h.until - now))
    };
    match s {
        TurretState::Awake | TurretState::Hunting => format!(
            "facing {} · ARM {} · HP {} · {} rounds",
            field::dir_word(t.facing),
            t.arm,
            f.hp,
            t.rounds
        ),
        TurretState::Blind | TurretState::Yours => format!("{}{}", mode_word(f), left()),
        TurretState::Asleep | TurretState::Wrecked => mode_word(f).to_string(),
    }
}

/// The footer's word for a turret: `asleep`, `awake`, `hunting`, `blind`, `yours` (and
/// `wrecked` for a wreck, which the footer doesn't list). Empty for a foe that's none.
pub fn mode_word(f: &Foe) -> &'static str {
    match state(f) {
        Some(TurretState::Asleep) => "asleep",
        Some(TurretState::Awake) => "awake",
        Some(TurretState::Hunting) => "hunting",
        Some(TurretState::Blind) => "blind",
        Some(TurretState::Yours) => "yours",
        Some(TurretState::Wrecked) => "wrecked",
        None => "",
    }
}

/// Its chip's figure in the turn column: plate, then hit points (`4·40`).
pub fn chip_figure(f: &Foe) -> String {
    let arm = f.turret.as_ref().map_or(0, |t| t.arm);
    format!("{arm}·{}", f.hp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::testutil::*;
    use crate::holds::Hold;
    use crate::owners::Owner;
    use crate::turrets::Hacked;
    use crate::turrets::testutil::turret_in_ward;

    /// A hold on turret `i`'s node, by hand, `minutes` from its end.
    fn held_for(st: &mut AppState, i: usize, minutes: u64) {
        let node = st.foes[i].turret.as_ref().unwrap().node;
        let now = st.net.now(st);
        st.net.holds.clear();
        st.net.holds.push(Hold {
            node: NetNode::Site(node),
            owner: Owner::Building(WARD),
            until: now + minutes,
            used: Vec::new(),
        });
    }

    #[test]
    fn the_look_line_reads_its_state() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::SE);
        assert_eq!(look_line(&st, i), "asleep");
        st.foes[i].turret.as_mut().unwrap().awake = true;
        st.foes[i].turret.as_mut().unwrap().rounds = 22;
        assert_eq!(look_line(&st, i), "facing SE · ARM 4 · HP 40 · 22 rounds");
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Blind;
        held_for(&mut st, i, 42);
        assert_eq!(look_line(&st, i), "blind · 42 min");
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Yours;
        held_for(&mut st, i, 12);
        assert_eq!(look_line(&st, i), "yours · 12 min");
        st.foes[i].down = true;
        assert_eq!(look_line(&st, i), "wrecked");
    }

    #[test]
    fn the_chip_reads_arm_then_hp() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::SE);
        assert_eq!(chip_figure(&st.foes[i]), "4·40");
        st.foes[i].hp = 31;
        st.foes[i].turret.as_mut().unwrap().arm = 3;
        assert_eq!(chip_figure(&st.foes[i]), "3·31");
    }

    #[test]
    fn the_footer_word_follows_its_state() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::SE);
        assert_eq!(mode_word(&st.foes[i]), "asleep");
        st.foes[i].turret.as_mut().unwrap().awake = true;
        assert_eq!(mode_word(&st.foes[i]), "awake");
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        assert_eq!(mode_word(&st.foes[i]), "hunting");
        st.foes[i].mode = crate::foes::FoeMode::Idle;
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Blind;
        assert_eq!(mode_word(&st.foes[i]), "blind");
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Yours;
        assert_eq!(mode_word(&st.foes[i]), "yours");
    }

    #[test]
    fn looks_lists_every_turret_on_the_place_wrecked_too() {
        let mut st = ward();
        let a = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::SE);
        turret_in_ward(&mut st, 1, Pos::new(10, 3), Dir::N);
        goon(&mut st, 0, Pos::new(4, 4));
        st.foes[a].down = true;
        assert_eq!(
            looks(&st, on(0)),
            vec![(Pos::new(10, 3), Dir::SE, TurretState::Wrecked)]
        );
        assert_eq!(
            looks(&st, on(1)),
            vec![(Pos::new(10, 3), Dir::N, TurretState::Asleep)]
        );
    }

    #[test]
    fn its_field_draws_red_awake_green_yours_and_not_at_all_asleep() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
        assert!(cones(&st, on(0)).is_empty());
        st.foes[i].turret.as_mut().unwrap().awake = true;
        let c = cones(&st, on(0));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].1, crate::cameras::ConeLook::Seeing);
        assert!(c[0].0.contains(&Pos::new(6, 3)));
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Yours;
        assert_eq!(cones(&st, on(0))[0].1, crate::cameras::ConeLook::Tapped);
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Blind;
        assert!(cones(&st, on(0)).is_empty());
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::None;
        st.foes[i].down = true;
        assert!(cones(&st, on(0)).is_empty(), "a wreck has no field");
    }

    #[test]
    fn an_unseen_hostile_turrets_field_isnt_drawn_but_yours_is() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
        st.foes[i].turret.as_mut().unwrap().awake = true;
        // Upstairs, its tile isn't in your sight memory: red floor would give it away.
        stand(&mut st, 1, Pos::new(6, 3));
        assert!(crate::foes::unseen(&st, i));
        assert!(cones(&st, on(0)).is_empty());
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Yours;
        let c = cones(&st, on(0));
        assert_eq!(c.len(), 1, "yours, you know where it is");
        assert_eq!(c[0].1, crate::cameras::ConeLook::Tapped);
    }

    #[test]
    fn the_cameras_cones_carry_the_turrets_fields() {
        let mut st = ward();
        let i = turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
        st.foes[i].turret.as_mut().unwrap().awake = true;
        assert_eq!(crate::cameras::cones(&st, on(0)), cones(&st, on(0)));
    }
}
