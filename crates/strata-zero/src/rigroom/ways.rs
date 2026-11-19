//! The ways in and out of your rig's room: jacking in at home lands you in it, and the
//! LINE trunk at the foot of the hall takes you out onto your block on the grid.

use crate::app::AppState;
use citygen::{Place, Pos, StratumKind};

/// Jacks in at the rig: the body stays at the desk, and you're in the room, laid out
/// afresh, on the hall's floor under the reader. A rig run starts: its route home is
/// empty until you leave by a backdoor.
pub fn enter(st: &mut AppState) -> Result<String, String> {
    if let Some(why) = crate::net::jack_in_refusal(st) {
        return Err(why);
    }
    let home = st.home.ok_or("no rig here")?;
    let bonus = st.sight_bonus();
    let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut()) else {
        return Err("no body in the city".to_string());
    };
    if player.place != home.place() {
        return Err("no rig here".to_string());
    }
    let from = player.place;
    // The port's own checks (at the rig, not already in, the line alive) set the body.
    player
        .jack_in(city, Some(&home), bonus)
        .map_err(|e| e.to_string())?;
    super::build(st);
    let land = super::room(st).ok_or("no rig here")?.land;
    crate::constructs::land(st, Place::Rig, land);
    crate::net::after_jack_in(st, from, None);
    Ok("jacked in · your rig".to_string())
}

/// Out of the room along the LINE trunk, onto your block on the grid (`HomeRig.landing`),
/// still jacked in.
pub fn to_the_grid(st: &mut AppState) -> Result<String, String> {
    let landing = st.home.ok_or("no rig here")?.landing;
    let city = st.city.as_ref().ok_or("no city")?;
    if !city.walkable(StratumKind::Net, landing) {
        return Err("the line is dead".to_string());
    }
    crate::constructs::land(st, Place::Stratum(StratumKind::Net), landing);
    Ok("the grid · your block".to_string())
}

/// A step in the room onto `to` that leaves it: the trunk's end out to the grid. None for
/// any other tile, which the room walks as a map.
pub fn step(st: &mut AppState, to: Pos) -> Option<Result<String, String>> {
    let room = super::room(st)?;
    (to == room.trunk).then(|| to_the_grid(st))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::rigroom::testutil::{at_the_rig, hold_a_backdoor, in_the_room};
    use crate::rigroom::{counts, room};
    use crate::screens::explore::MapScreen;
    use crate::screens::testutil::key;
    use crate::walker::Visibility;
    use crossterm::event::KeyCode;

    #[test]
    fn jacking_in_at_home_lands_in_the_room() {
        let mut st = at_the_rig();
        hold_a_backdoor(&mut st, 0);
        let flat = st.home.unwrap().place();
        let desk = st.player.as_ref().unwrap().pos;
        assert!(
            !crate::net::wants_landing_list(&st),
            "the room in place of the JACK IN list"
        );
        let line = crate::net::jack_in_here(&mut st).unwrap();
        assert_eq!(line, "jacked in · your rig");
        let land = room(&st).expect("the room is laid out").land;
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Rig);
        assert_eq!(p.pos, land);
        assert_eq!(p.body, Some((flat, desk)), "the body at the rig");
        assert!(!crate::net::wants_landing_list(&st));
        // a rig run: the route home starts at your block
        assert_eq!(st.backdoors.route, Some(crate::backdoors::Route::default()));
        let map = crate::render::map::MapRef::for_place(
            st.city.as_ref().unwrap(),
            &st.interiors,
            &st.constructs,
            Place::Rig,
            st.minute_of_day(),
        )
        .expect("the room is a map");
        assert_eq!(map.place(), Place::Rig);
    }

    #[test]
    fn every_standing_backdoor_gets_a_door() {
        let mut st = at_the_rig();
        let lines = crate::rig::of(&st).lines;
        assert_eq!(counts(&st).lines, lines);
        for n in 0..lines + 2 {
            hold_a_backdoor(&mut st, n);
        }
        assert_eq!(counts(&st).lines, lines + 2, "more backdoors than lines");
        crate::net::jack_in_here(&mut st).unwrap();
        assert_eq!(room(&st).unwrap().doors.len(), lines + 2);
    }

    #[test]
    fn the_trunk_lands_on_your_block() {
        let mut st = in_the_room();
        let trunk = room(&st).unwrap().trunk;
        let landing = st.home.unwrap().landing;
        // the room's own floor is walked as any map
        assert!(step(&mut st, Pos::new(trunk.x, trunk.y - 2)).is_none());
        assert_eq!(st.player.as_ref().unwrap().place, Place::Rig);
        let line = step(&mut st, trunk).expect("the trunk leads out").unwrap();
        assert_eq!(line, "the grid · your block");
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            (p.place, p.pos),
            (Place::Stratum(StratumKind::Net), landing)
        );
        assert!(p.jacked_in(), "still jacked in");
        assert_eq!(st.backdoors.route, Some(crate::backdoors::Route::default()));
    }

    #[test]
    fn walking_onto_the_trunk_takes_it() {
        let mut st = in_the_room();
        let mut map = MapScreen::walking(&mut st);
        let trunk = room(&st).unwrap().trunk;
        st.player.as_mut().unwrap().pos = Pos::new(trunk.x, trunk.y - 1);
        map.update(&mut st, &[key(KeyCode::Down)]);
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            (p.place, p.pos),
            (Place::Stratum(StratumKind::Net), st.home.unwrap().landing)
        );
    }

    #[test]
    fn jacking_out_of_the_room_is_home() {
        let mut st = at_the_rig();
        let (flat, desk) = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        crate::net::jack_in_here(&mut st).unwrap();
        crate::net::jack_out(&mut st).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (flat, desk));
        assert!(!p.jacked_in());
        assert!(st.backdoors.route.is_none(), "the rig run is over");
    }

    #[test]
    fn a_dump_in_the_room_starts_no_trace() {
        let mut st = at_the_rig();
        let flat = st.home.unwrap().place();
        crate::net::jack_in_here(&mut st).unwrap();
        crate::net::dump(&mut st, "the failsafe pulled you out");
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, flat, "back at the rig");
        assert!(st.backdoors.trace.is_none(), "inside your own firewall");
        // out on your block the run's route is traced as ever
        st.net.cooling_until = None;
        crate::net::jack_in_here(&mut st).unwrap();
        to_the_grid(&mut st).unwrap();
        crate::net::dump(&mut st, "the failsafe pulled you out");
        assert!(st.backdoors.trace.is_some());
    }

    #[test]
    fn the_rig_is_fully_seen() {
        let st = in_the_room();
        let p = st.player.as_ref().unwrap();
        assert!(p.sees_all());
        let size = room(&st).unwrap().construct.tiles.size();
        let vis = p.vis();
        assert_eq!(vis.size(), size);
        assert!(vis.iter().all(|(_, v)| *v == Visibility::Visible));
    }

    #[test]
    fn the_place_line_names_your_rig() {
        let st = in_the_room();
        let p = st.player.as_ref().unwrap();
        let city = st.city.as_ref().unwrap();
        assert_eq!(
            crate::ui::hud::location(city, &st.interiors, p.place, p.pos),
            "zenbo · your rig"
        );
        assert_eq!(
            crate::look::location(&st, p.place, p.pos),
            "zenbo · your rig"
        );
        // and the footer's first row
        let mut st = st;
        st.show_fps = false;
        let map = MapScreen::walking(&mut st);
        let text = crate::screens::testutil::buffer_text(&crate::screens::shots::draw_stack(
            &st,
            &[&map],
            (120, 40),
        ));
        let row = (text.lines().find(|l| l.contains('⚡'))).expect("the footer's first row");
        assert!(row.contains("zenbo · your rig"), "{text}");
        assert!(row.contains("TRACE ▱▱▱▱▱▱▱▱▱▱ 0"), "{row}");
    }
}
