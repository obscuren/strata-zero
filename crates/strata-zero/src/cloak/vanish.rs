//! Vanish (#36): Shadow step's way out of a fight. The fight ends, everyone on your map
//! and every guard of the building loses you, and a free cloak holds for 4 s to get
//! away under.

use super::{Pool, VANISH_TICKS, on_the_net, put_on};
use crate::app::AppState;
use citygen::Place;
use game::AbilityId;

/// Vanish from the act list: the fight ends, every standing foe on your map turns calm,
/// and every guard of the building you stand in (any floor) and on your map loses you,
/// its unmade call and its trail forgotten (a call made and the alarm stand). Then a
/// free cloak for `VANISH_TICKS`. The refusal says why not.
pub fn vanish(st: &mut AppState, id: &AbilityId) -> Result<(), String> {
    if st.net.engaged.is_none() {
        return Err("only in a fight".to_string());
    }
    let Some((place, at)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return Err("no body in the city".to_string());
    };
    if on_the_net(place) {
        return Err("not in here".to_string());
    }
    crate::casts::can_act(st, 0)?;
    crate::casts::book(st, id)?;
    crate::net::disengage(st);
    let building = match place {
        Place::Floor { building, .. } => Some(building),
        _ => None,
    };
    let now = st.clock_tick();
    for i in 0..st.foes.len() {
        let f = &st.foes[i];
        let here = f.standing() && f.place == place;
        let guard = f
            .post
            .as_ref()
            .is_some_and(|p| f.place == place || Some(p.building) == building);
        if here {
            st.foes[i].mode = crate::foes::FoeMode::Idle;
        }
        if guard {
            crate::guards::lose_you(st, i, now);
            if let Some(p) = st.foes[i].post.as_mut() {
                p.spotted = false;
            }
        }
    }
    put_on(
        st,
        Pool::Free {
            until: st.tick + VANISH_TICKS,
        },
        at,
    );
    st.say("vanish · they lose you");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::super::*;
    use citygen::Pos;

    fn shadow_step(st: &mut AppState) {
        st.character
            .talents
            .insert(game::TalentId::new("shadow_step"), 1);
    }

    fn vanish_out() -> AbilityId {
        AbilityId::new("vanish_out")
    }

    /// The lot with the goon at (10,8) hunting you, on turns on your act half, and
    /// Vanish used.
    fn vanished_on_the_street() -> AppState {
        let mut st = street();
        shadow_step(&mut st);
        st.foes[0].pos = Pos::new(10, 8);
        st.foes[0].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 8),
            lost: 0,
        };
        let here = st.player.as_ref().unwrap().place;
        crate::net::engage(&mut st, here, false);
        crate::guards::testutil::settle(&mut st);
        crate::net::stay(&mut st);
        vanish(&mut st, &vanish_out()).unwrap();
        st
    }

    #[test]
    fn vanish_ends_the_fight_and_every_guard_of_the_building_loses_you() {
        use crate::guards::testutil::*;
        let mut st = ward();
        shadow_step(&mut st);
        let here = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        let upstairs = guard(&mut st, 1, 0, &[Pos::new(10, 3)]);
        let called = guard(&mut st, 0, 0, &[Pos::new(12, 5)]);
        for i in [here, upstairs, called] {
            st.foes[i].mode = crate::foes::FoeMode::Hunting {
                seen: Pos::new(6, 3),
                lost: 0,
            };
            st.foes[i].post.as_mut().unwrap().spotted = true;
        }
        st.foes[upstairs].post.as_mut().unwrap().follow = Some((0, 0));
        st.foes[called].post.as_mut().unwrap().radioed = true;
        crate::net::engage(&mut st, on(0), false);
        settle(&mut st);
        crate::net::stay(&mut st);
        vanish(&mut st, &vanish_out()).unwrap();
        assert!(st.net.engaged.is_none());
        for i in [here, upstairs, called] {
            let p = st.foes[i].post.as_ref().unwrap();
            assert!(
                !st.foes[i].hunting() && !p.spotted && p.follow.is_none(),
                "{i}"
            );
        }
        assert!(
            st.foes[called].post.as_ref().unwrap().radioed,
            "a call made stands"
        );
        assert!(matches!(st.cloak.as_ref().unwrap().pool, Pool::Free { .. }));
        assert_eq!(heat(&st), Some(VANISH_HEAT));
        assert!(
            (st.character)
                .cooling(&st.content, &vanish_out(), st.tick)
                .is_some()
        );
        assert!(logged(&st, "vanish · they lose you"));
    }

    #[test]
    fn vanishs_cloak_lasts_four_seconds_and_costs_nothing() {
        let mut st = vanished_on_the_street();
        let focus = st.character.focus;
        frames(&mut st, VANISH_TICKS - 1);
        assert!(hidden(&st));
        frames(&mut st, 1);
        assert!(!hidden(&st));
        assert_eq!(st.character.focus, focus);
        assert_eq!(st.veil.fade.unwrap().phase, Phase::SwitchingOff);
        assert!(!st.foes[0].hunting(), "the goon calmed");
    }

    #[test]
    fn a_guard_of_another_building_keeps_hunting() {
        use crate::guards::testutil::*;
        let mut st = ward();
        shadow_step(&mut st);
        let here = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        let next_door = citygen::BuildingId(1);
        let there = Place::Floor {
            building: next_door,
            level: 0,
        };
        st.foes.push(crate::foes::Foe {
            name: "guard".to_string(),
            glyph: '@',
            post: Some(crate::foes::Post::new(next_door, 0, vec![Pos::new(4, 4)])),
            ..crate::foes::Foe::goon(there, Pos::new(4, 4))
        });
        let other = st.foes.len() - 1;
        for i in [here, other] {
            st.foes[i].mode = crate::foes::FoeMode::Hunting {
                seen: Pos::new(6, 3),
                lost: 0,
            };
            st.foes[i].post.as_mut().unwrap().spotted = true;
        }
        crate::net::engage(&mut st, on(0), false);
        settle(&mut st);
        crate::net::stay(&mut st);
        vanish(&mut st, &vanish_out()).unwrap();
        assert!(!st.foes[here].hunting(), "the ward's guard loses you");
        assert!(st.foes[other].hunting(), "next door's keeps hunting");
        assert!(st.foes[other].post.as_ref().unwrap().spotted);
    }

    #[test]
    fn vanish_is_refused_in_a_construct() {
        let mut st = street();
        shadow_step(&mut st);
        let here = st.player.as_ref().unwrap().place;
        crate::net::engage(&mut st, here, false);
        st.player.as_mut().unwrap().place = Place::Construct(citygen::BuildingId(0));
        assert_eq!(
            vanish(&mut st, &vanish_out()),
            Err("not in here".to_string())
        );
        assert!(st.net.engaged.is_some(), "the fight stands");
        assert!(st.cloak.is_none());
        assert!(
            (st.character)
                .cooling(&st.content, &vanish_out(), st.tick)
                .is_none(),
            "no cooldown booked"
        );
    }

    #[test]
    fn vanish_is_for_fights_only() {
        let mut st = street();
        shadow_step(&mut st);
        assert_eq!(
            vanish(&mut st, &vanish_out()),
            Err("only in a fight".to_string())
        );
    }
}
