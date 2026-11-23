//! The ways in and out of your rig's room: jacking in at home lands you in it, the LINE
//! trunk at the foot of the hall takes you down into your block's construct, and your
//! line's door there takes you back up.

use crate::app::AppState;
use citygen::{Construct, Place, Pos, StratumKind};

/// Your line's door in your block's construct: the wall under the lane, a step in from
/// the gate. A generated construct's side rooms start two columns in, so it's always
/// wall, and only your own block's construct has the door.
pub fn your_line(k: &Construct) -> Pos {
    k.entry.offset(1, 1)
}

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

/// Out of the room down the LINE trunk, still jacked in: into your block's construct on
/// the lane over your line's door, its gate a step west. Your block on alert closes its
/// gate, not your line. With no construct for your block, out onto the grid beside its
/// gate (`HomeRig.landing`).
pub fn down_the_line(st: &mut AppState) -> Result<String, String> {
    let player = st.player.as_ref().ok_or("no body")?;
    if !player.jacked_in() || player.place != Place::Rig {
        return Err("the line starts in your rig's room".to_string());
    }
    let home = st.home.ok_or("no rig here")?;
    let block = home.home.building;
    let site = crate::sitenet::is_site(st, block);
    let city = st.city.as_ref().ok_or("no city")?;
    let lane = (!site)
        .then(|| st.constructs.ensure(city, block))
        .flatten()
        .map(|k| your_line(k).offset(0, -1));
    if let Some(lane) = lane {
        // The first beat falls a full beat after you come in, as through the gate.
        st.net.beat = st.clock_tick();
        crate::constructs::land(st, Place::Construct(block), lane);
        return Ok("down the line · your block".to_string());
    }
    if !city.walkable(StratumKind::Net, home.landing) {
        return Err("the line is dead".to_string());
    }
    crate::constructs::land(st, Place::Stratum(StratumKind::Net), home.landing);
    Ok("down the line · your block".to_string())
}

/// Up your line from its door into your rig's room, onto the hall's floor over the
/// trunk's end: the room you left, laid out again only if it's gone (a run reopened
/// outside it). Only while you're jacked in at your rig. You're home, so the route home
/// empties.
pub fn up_the_line(st: &mut AppState) -> Result<String, String> {
    let home = st.home.ok_or("no rig here")?;
    let player = st.player.as_ref().ok_or("no body")?;
    if !player.jacked_in() || player.body.map(|(place, _)| place) != Some(home.place()) {
        return Err("your line · your rig isn't jacked in".to_string());
    }
    if super::room(st).is_none() {
        super::build(st);
    }
    let trunk = super::room(st).ok_or("no rig here")?.trunk;
    crate::constructs::land(st, Place::Rig, trunk.offset(0, -1));
    super::refresh(st);
    st.backdoors.start_route(None);
    Ok("up the line · your rig".to_string())
}

/// A step in your block's construct onto your line's door: up into your rig's room.
/// None for any other step, and in any other construct, where that tile is wall.
pub fn door_step(st: &mut AppState, to: Pos) -> Option<Result<String, String>> {
    let block = st.home?.home.building;
    if st.player.as_ref()?.place != Place::Construct(block) {
        return None;
    }
    if to != your_line(st.constructs.get(block)?) {
        return None;
    }
    Some(up_the_line(st))
}

/// A step in the room onto `to` that leaves it: the trunk's end down into your block,
/// or a backdoor's door out through it. A free line's door (or one whose backdoor burned)
/// is a wall that says so. None for any other tile, which the room walks as a map.
pub fn step(st: &mut AppState, to: Pos) -> Option<Result<String, String>> {
    super::refresh(st);
    let room = super::room(st)?;
    if to == room.trunk {
        return Some(down_the_line(st));
    }
    let i = room.doors.iter().position(|d| *d == to)?;
    Some(match super::door_lines(st).get(i).copied().flatten() {
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

    /// Your block's building and the lane tile over your line's door in its construct.
    fn your_block(st: &AppState) -> (citygen::BuildingId, Pos) {
        let block = st.home.unwrap().home.building;
        let k = st.constructs.get(block).expect("your block's construct");
        (block, your_line(k).offset(0, -1))
    }

    #[test]
    fn the_trunk_lands_in_your_block_over_your_line() {
        let mut st = in_the_room();
        let trunk = room(&st).unwrap().trunk;
        // the room's own floor is walked as any map
        assert!(step(&mut st, Pos::new(trunk.x, trunk.y - 2)).is_none());
        assert_eq!(st.player.as_ref().unwrap().place, Place::Rig);
        let line = step(&mut st, trunk).expect("the trunk leads out").unwrap();
        assert_eq!(line, "down the line · your block");
        let (block, lane) = your_block(&st);
        let k = st.constructs.get(block).unwrap();
        assert_eq!(lane, k.entry.offset(1, 0), "a step in from the gate");
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (Place::Construct(block), lane));
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
        let (block, lane) = your_block(&st);
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (Place::Construct(block), lane));
    }

    #[test]
    fn your_lines_door_takes_you_back_up() {
        let mut st = in_the_room();
        let mut map = MapScreen::walking(&mut st);
        let trunk = room(&st).unwrap().trunk;
        down_the_line(&mut st).unwrap();
        // out by a backdoor and round to your block: the route home has a relay
        let b = hold_a_backdoor(&mut st, 0);
        st.backdoors.start_route(Some(b));
        map.update(&mut st, &[key(KeyCode::Down)]);
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (Place::Rig, trunk.offset(0, -1)));
        assert!(p.jacked_in());
        assert!(
            st.log.iter().any(|e| e.text == "up the line · your rig"),
            "{:?}",
            st.log.iter().last().map(|e| &e.text)
        );
        assert_eq!(
            st.backdoors.route,
            Some(crate::backdoors::Route::default()),
            "home: the route empties"
        );
        // and down again: the trunk still leads out
        map.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(
            st.player.as_ref().unwrap().place,
            Place::Construct(your_block(&st).0)
        );
    }

    #[test]
    fn your_line_lays_the_room_out_again_when_its_gone() {
        let mut st = in_the_room();
        down_the_line(&mut st).unwrap();
        st.constructs.rig = None;
        let (block, lane) = your_block(&st);
        let door = lane.offset(0, 1);
        assert_eq!(st.player.as_ref().unwrap().place, Place::Construct(block));
        let line = door_step(&mut st, door).expect("your line's door").unwrap();
        assert_eq!(line, "up the line · your rig");
        let trunk = room(&st).expect("laid out again").trunk;
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (Place::Rig, trunk.offset(0, -1)));
    }

    #[test]
    fn your_line_is_shut_unless_you_jacked_in_at_your_rig() {
        let mut st = in_the_room();
        down_the_line(&mut st).unwrap();
        let (_, lane) = your_block(&st);
        let here = st.player.as_ref().map(|p| (p.place, p.pos));
        // the body jacked in somewhere else: on the street by your block
        let street = (
            Place::Stratum(StratumKind::Street),
            st.home.unwrap().landing,
        );
        st.player.as_mut().unwrap().body = Some(street);
        let refused = door_step(&mut st, lane.offset(0, 1)).expect("your line's door");
        assert_eq!(
            refused,
            Err("your line · your rig isn't jacked in".to_string())
        );
        assert_eq!(st.player.as_ref().map(|p| (p.place, p.pos)), here);
    }

    #[test]
    fn only_your_block_has_your_line() {
        let mut st = in_the_room();
        // in your own, only the door tile is it
        down_the_line(&mut st).unwrap();
        let (_, lane) = your_block(&st);
        assert!(door_step(&mut st, lane.offset(1, 0)).is_none());
        // in another block's, that tile is wall
        let b = hold_a_backdoor(&mut st, 0);
        let city = st.city.clone().unwrap();
        let k = st.constructs.ensure(&city, b).unwrap();
        let (lane, door) = (k.entry.offset(1, 0), your_line(k));
        crate::constructs::land(&mut st, Place::Construct(b), lane);
        assert!(door_step(&mut st, door).is_none(), "another block's wall");
    }

    #[test]
    fn every_constructs_line_tile_is_wall_under_its_lane() {
        let st = at_the_rig();
        let city = st.city.as_ref().unwrap();
        let mut seen = 0;
        for b in &city.buildings {
            let Some(k) = citygen::construct::generate_construct(city, b.id) else {
                continue;
            };
            let door = your_line(&k);
            let lane = k.tiles[k.entry.offset(1, 0)].terrain;
            assert_eq!(lane, citygen::Terrain::NetLane, "{:?}", b.id);
            let wall = k.tiles.get(door).map(|t| t.terrain);
            assert_eq!(wall, Some(citygen::Terrain::NetWall), "{:?}", b.id);
            assert!(
                door.y < k.size.h as i32 - 1,
                "inside the border: {:?}",
                b.id
            );
            assert!(k.node_at(door).is_none() && k.guard_at(door).is_none());
            seen += 1;
        }
        assert!(seen > 10, "{seen}");
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
        // down in your block the run's route is traced as ever
        st.net.cooling_until = None;
        crate::net::jack_in_here(&mut st).unwrap();
        down_the_line(&mut st).unwrap();
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
        assert!(down_the_line(&mut st).is_err(), "not jacked in");
        let b = hold_a_backdoor(&mut st, 0);
        crate::net::jack_in_here(&mut st).unwrap();
        crate::net::leave_by_backdoor(&mut st, b).unwrap();
        let spent = st.backdoors.held(b).unwrap().spent;
        assert!(down_the_line(&mut st).is_err(), "out on the grid already");
        assert!(crate::net::leave_by_backdoor(&mut st, b).is_err());
        assert_eq!(st.backdoors.held(b).unwrap().spent, spent);
    }

    #[test]
    fn a_backdoor_planted_in_the_room_gets_a_free_door() {
        let mut st = in_the_room();
        let before = crate::rigroom::door_lines(&st);
        assert!(before.iter().all(|l| l.is_none()), "every line free");
        let b = hold_a_backdoor(&mut st, 0);
        let free = room(&st).unwrap().doors[0];
        let line = step(&mut st, free).expect("a door").unwrap();
        assert!(line.contains("via your backdoor"), "{line}");
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![b]);
    }

    #[test]
    fn a_door_given_stays_with_its_backdoor() {
        let mut st = in_the_room();
        let a = hold_a_backdoor(&mut st, 0);
        crate::rigroom::refresh(&mut st);
        let b = hold_a_backdoor(&mut st, 1);
        crate::rigroom::refresh(&mut st);
        assert_eq!(&crate::rigroom::door_lines(&st)[..2], &[Some(a), Some(b)]);
        st.backdoors
            .list
            .iter_mut()
            .find(|d| d.building == a)
            .unwrap()
            .state = crate::backdoors::State::Burned {
            at: 0,
            why: crate::backdoors::Why::Decay,
        };
        crate::rigroom::refresh(&mut st);
        assert_eq!(&crate::rigroom::door_lines(&st)[..2], &[None, Some(b)]);
    }

    #[test]
    fn a_backdoor_past_the_last_door_grows_the_room() {
        let mut st = at_the_rig();
        let lines = crate::rig::of(&st).lines;
        let held: Vec<_> = (0..lines).map(|n| hold_a_backdoor(&mut st, n)).collect();
        crate::net::jack_in_here(&mut st).unwrap();
        assert_eq!(room(&st).unwrap().doors.len(), lines);
        let extra = hold_a_backdoor(&mut st, lines);
        let first = room(&st).unwrap().doors[0];
        step(&mut st, Pos::new(first.x + 1, first.y)); // not a door: just refreshes
        let r = room(&st).unwrap();
        assert_eq!(r.doors.len(), lines + 1, "one more door");
        let lines_now = crate::rigroom::door_lines(&st);
        let held: Vec<_> = held.into_iter().map(Some).collect();
        assert_eq!(&lines_now[..lines], &held[..], "the old doors keep theirs");
        assert_eq!(lines_now[lines], Some(extra));
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Rig);
        assert!(r.construct.tiles[p.pos].is_walkable());
    }
}
