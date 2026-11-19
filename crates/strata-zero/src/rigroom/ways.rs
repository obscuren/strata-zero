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
    let player = st.player.as_ref().ok_or("no body")?;
    if !player.jacked_in() || player.place != Place::Rig {
        return Err("the line starts in your rig's room".to_string());
    }
    let landing = st.home.ok_or("no rig here")?.landing;
    let city = st.city.as_ref().ok_or("no city")?;
    if !city.walkable(StratumKind::Net, landing) {
        return Err("the line is dead".to_string());
    }
    crate::constructs::land(st, Place::Stratum(StratumKind::Net), landing);
    Ok("the grid · your block".to_string())
}

/// A step in the room onto `to` that leaves it: the trunk's end out to the grid, or a
/// backdoor's door out through it. A free line's door (or one whose backdoor burned) is
/// a wall that says so. None for any other tile, which the room walks as a map.
pub fn step(st: &mut AppState, to: Pos) -> Option<Result<String, String>> {
    let room = super::room(st)?;
    if to == room.trunk {
        return Some(to_the_grid(st));
    }
    let i = room.doors.iter().position(|d| *d == to)?;
    Some(match super::door_lines(st)[i] {
        Some(b) => crate::net::leave_by_backdoor(st, b),
        None => Err("a free line · nothing on it".to_string()),
    })
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
        let line = crate::net::jack_in_here(&mut st).unwrap();
        assert_eq!(line, "jacked in · your rig");
        let land = room(&st).expect("the room is laid out").land;
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Rig);
        assert_eq!(p.pos, land);
        assert_eq!(p.body, Some((flat, desk)), "the body at the rig");
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

    /// The door of the `i`th line, and the room's standing tile before it.
    fn door(st: &AppState, b: citygen::BuildingId) -> Pos {
        let i = crate::rigroom::door_lines(st)
            .iter()
            .position(|l| *l == Some(b))
            .expect("a door for it");
        room(st).unwrap().doors[i]
    }

    #[test]
    fn a_door_leaves_for_its_buildings_gate() {
        let mut st = at_the_rig();
        let a = hold_a_backdoor(&mut st, 0);
        let b = hold_a_backdoor(&mut st, 1);
        crate::net::jack_in_here(&mut st).unwrap();
        let lines = crate::rigroom::door_lines(&st);
        assert_eq!(&lines[..2], &[Some(a), Some(b)], "standing ones, in order");
        assert!(lines[2..].iter().all(|l| l.is_none()), "free lines after");
        let to = door(&st, b);
        let line = step(&mut st, to).expect("a door").unwrap();
        assert!(line.contains("via your backdoor"), "{line}");
        let p = st.player.as_ref().unwrap();
        let gate = crate::constructs::city_gate(st.city.as_ref().unwrap(), b).unwrap();
        assert_eq!((p.place, p.pos), (Place::Stratum(StratumKind::Net), gate));
        assert!(p.jacked_in());
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![b]);
        assert_eq!(
            st.backdoors.held(b).unwrap().spent,
            crate::backdoors::COST_LAND_NET
        );
        assert_eq!(st.backdoors.held(a).unwrap().spent, 0, "only the one used");
    }

    #[test]
    fn a_key_door_costs_a_keys_landing() {
        let mut st = at_the_rig();
        let b = hold_a_backdoor(&mut st, 0);
        st.backdoors.list.clear();
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_key(b, "user", 100, seed, 0);
        crate::net::jack_in_here(&mut st).unwrap();
        let to = door(&st, b);
        step(&mut st, to).unwrap().unwrap();
        assert_eq!(
            st.backdoors.held(b).unwrap().spent,
            crate::backdoors::COST_LAND
        );
    }

    #[test]
    fn a_sites_door_lands_on_its_l_hub() {
        use crate::sitenet::testutil::{office_copy, stand_at_the_rig};
        let (mut st, b) = office_copy(190);
        let now = st.net.now(&st);
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors
            .plant_net(b, 99, seed, now, crate::rig::STOCK.hold_days);
        stand_at_the_rig(&mut st);
        crate::net::jack_in_here(&mut st).unwrap();
        let to = door(&st, b);
        step(&mut st, to).unwrap().unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            p.place,
            Place::SiteNet {
                building: b,
                level: 0
            }
        );
        let k = st.constructs.site(b).unwrap();
        assert_eq!(
            k.device_at(p.pos).map(|d| d.kind),
            Some(citygen::NodeKind::Hub)
        );
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![b]);
    }

    #[test]
    fn a_burned_backdoor_door_refuses() {
        let mut st = at_the_rig();
        let a = hold_a_backdoor(&mut st, 0);
        let b = hold_a_backdoor(&mut st, 1);
        crate::net::jack_in_here(&mut st).unwrap();
        let to = door(&st, a);
        st.backdoors
            .list
            .iter_mut()
            .find(|d| d.building == a)
            .unwrap()
            .state = crate::backdoors::State::Burned {
            at: 0,
            why: crate::backdoors::Why::Decay,
        };
        let lines = crate::rigroom::door_lines(&st);
        assert_eq!(lines[0], None, "its door is dark");
        assert_eq!(lines[1], Some(b), "the others keep theirs");
        let before = st.player.as_ref().unwrap().pos;
        assert!(step(&mut st, to).expect("a door").is_err());
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (Place::Rig, before));
        assert_eq!(st.backdoors.route, Some(crate::backdoors::Route::default()));
        // the door that stands still leaves
        let to = door(&st, b);
        step(&mut st, to).unwrap().unwrap();
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![b]);
    }

    #[test]
    fn a_honeypot_door_springs() {
        let mut st = at_the_rig();
        let b = hold_a_backdoor(&mut st, 0);
        let d = st.backdoors.list.iter_mut().find(|d| d.building == b);
        d.unwrap().state = crate::backdoors::State::Honeypot;
        crate::net::jack_in_here(&mut st).unwrap();
        let to = door(&st, b);
        step(&mut st, to).unwrap().unwrap();
        assert!(st.backdoors.trace.is_some(), "the trace is running");
    }

    #[test]
    fn a_free_line_is_a_wall() {
        let mut st = in_the_room();
        let free = room(&st).unwrap().doors[0];
        let before = st.player.as_ref().unwrap().pos;
        assert!(step(&mut st, free).expect("a door").is_err());
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (Place::Rig, before));
        // walking at it stays put too
        let mut map = MapScreen::walking(&mut st);
        st.player.as_mut().unwrap().pos = Pos::new(free.x + 1, free.y);
        map.update(&mut st, &[key(KeyCode::Left)]);
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Rig);
        assert_eq!(p.pos, Pos::new(free.x + 1, free.y));
    }

    #[test]
    fn the_line_is_the_rooms_alone() {
        let mut st = at_the_rig();
        assert!(to_the_grid(&mut st).is_err(), "not jacked in");
        let b = hold_a_backdoor(&mut st, 0);
        crate::net::jack_in_here(&mut st).unwrap();
        crate::net::leave_by_backdoor(&mut st, b).unwrap();
        let spent = st.backdoors.held(b).unwrap().spent;
        assert!(to_the_grid(&mut st).is_err(), "out on the grid already");
        assert!(crate::net::leave_by_backdoor(&mut st, b).is_err());
        assert_eq!(st.backdoors.held(b).unwrap().spent, spent);
    }
}
