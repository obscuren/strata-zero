//! Visitors: authored people the story puts on the map. Never in the sim, so jobs, talk
//! and the inspector don't see them; they are drawn, looked at, and they block the way
//! (scenes spec §4). Also the flat's lights (§8).

use super::{Book, marks};
use crate::app::AppState;
use crate::render::viewport::tile_to_screen;
use crate::ui::chrome::DIM;
use crate::ui::dim::scale;
use crate::walker::Visibility;
use citygen::{Place, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// A visitor standing somewhere.
#[derive(Clone, Debug, PartialEq)]
pub struct Visitor {
    pub role: String,
    pub place: Place,
    pub pos: Pos,
}

/// Puts the role's visitor at `at`: a new one last in the order, or the one already
/// there moved.
pub fn arrive(st: &mut AppState, role: &str, at: (Place, Pos)) {
    let (place, pos) = at;
    match st.missions.visitors.iter_mut().find(|v| v.role == role) {
        Some(v) => (v.place, v.pos) = (place, pos),
        None => st.missions.visitors.push(Visitor {
            role: role.to_string(),
            place,
            pos,
        }),
    }
}

/// Takes the role's visitor off the map.
pub fn leave(st: &mut AppState, role: &str) {
    st.missions.visitors.retain(|v| v.role != role);
}

/// The visitor standing on a tile.
pub fn at(st: &AppState, place: Place, pos: Pos) -> Option<&Visitor> {
    st.missions
        .visitors
        .iter()
        .find(|v| v.place == place && v.pos == pos)
}

/// A role's colour: its tint, else `DIM`.
pub fn colour(book: &Book, role: &str) -> Color {
    book.cast
        .get(role)
        .and_then(|r| r.tint)
        .map_or(DIM, |(r, g, b)| Color::Rgb(r, g, b))
}

/// Draws the flat's lights, then the visitors, over the map view in `area`. The lights
/// scale every cell of the flat (walls too); the visitors come after, bright.
pub fn draw(buf: &mut Buffer, area: Rect, origin: Pos, st: &AppState) {
    let Some(me) = st.player.as_ref() else {
        return;
    };
    if let (Some(level), Some((place, rect))) = (st.missions.lights, marks::flat_rect(st))
        && place == me.place
    {
        for p in rect.iter() {
            if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
                let cell = &mut buf[(sx, sy)];
                cell.fg = scale(cell.fg, level);
                cell.bg = scale(cell.bg, level);
            }
        }
    }
    let vis = me.vis();
    for v in st.missions.visitors.iter().filter(|v| v.place == me.place) {
        if vis.get(v.pos) != Some(&Visibility::Visible) {
            continue;
        }
        if let Some((sx, sy)) = tile_to_screen(area, origin, v.pos) {
            let cell = &mut buf[(sx, sy)];
            cell.set_char('@').fg = colour(&st.book, &v.role);
            cell.modifier.insert(Modifier::BOLD);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::jobs::testutil::state_home;
    use crate::missions::{Mark, Pick, Role};
    use crate::screens::explore::MapScreen;
    use crate::walker::Player;
    use citygen::{GenParams, Generator, SizePreset};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn flat() -> AppState {
        let mut st = state_home();
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
        st
    }

    /// The book gains a visitor role.
    pub(crate) fn add_role(st: &mut AppState, id: &str, name: &str) {
        let role = Role {
            name: name.into(),
            title: "visitor".into(),
            pick: Pick::Visitor,
            face: None,
            tint: Some((255, 128, 0)),
            look: Some("A figure in the flat.".into()),
        };
        st.book.cast.insert(id.into(), role);
    }

    fn spot(st: &AppState, dx: i32) -> (Place, Pos) {
        let p = st.player.as_ref().unwrap();
        (p.place, p.pos.offset(dx, 0))
    }

    #[test]
    fn arrive_and_leave() {
        let mut st = flat();
        let (a, b) = (spot(&st, 1), spot(&st, 2));
        arrive(&mut st, "castellan", a);
        arrive(&mut st, "seren", b);
        assert_eq!(at(&st, a.0, a.1).unwrap().role, "castellan");
        assert_eq!(at(&st, b.0, b.1).unwrap().role, "seren");
        assert_eq!(at(&st, a.0, a.1.offset(0, 5)), None);
        leave(&mut st, "castellan");
        assert!(at(&st, a.0, a.1).is_none());
        assert_eq!(st.missions.visitors.len(), 1);
        leave(&mut st, "nobody");
        assert_eq!(st.missions.visitors.len(), 1);
    }

    #[test]
    fn arrive_twice_moves() {
        let mut st = flat();
        let (a, b) = (spot(&st, 1), spot(&st, 2));
        arrive(&mut st, "seren", a);
        arrive(&mut st, "seren", b);
        assert_eq!(st.missions.visitors.len(), 1);
        assert!(at(&st, a.0, a.1).is_none());
        assert!(at(&st, b.0, b.1).is_some());
    }

    #[test]
    fn a_visitor_blocks_the_way() {
        let mut st = state_home();
        let mut screen = MapScreen::walking(&mut st);
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
        add_role(&mut st, "castellan", "Castellan");
        arrive(&mut st, "castellan", (place, pos.offset(1, 0)));
        let east = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        screen.update(&mut st, &[east]);
        assert_eq!(st.player.as_ref().unwrap().pos, pos);
        let last = st.log.latest().unwrap();
        assert!(
            last.text.contains("Castellan") && last.text.contains("in the way"),
            "{}",
            last.text
        );
    }

    #[test]
    fn a_visitor_draws_as_a_bold_at_in_its_tint_where_you_can_see() {
        let mut st = flat();
        add_role(&mut st, "castellan", "Castellan");
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        let bonus = st.sight_bonus();
        let minute = st.minute_of_day();
        let map = crate::render::map::MapRef::for_place(
            st.city.as_ref().unwrap(),
            &st.interiors,
            &st.constructs,
            place,
            minute,
        )
        .unwrap();
        st.player.as_mut().unwrap().update_fov(&map, bonus);
        let vis = st.player.as_ref().unwrap().vis().clone();
        let area = Rect::new(0, 0, 20, 10);
        let seen = pos.offset(1, 0);
        assert_eq!(vis.get(seen), Some(&Visibility::Visible));
        arrive(&mut st, "castellan", (place, seen));
        let mut buf = Buffer::empty(area);
        draw(&mut buf, area, pos.offset(-5, -5), &st);
        let cell = &buf[(6, 5)];
        assert_eq!(cell.symbol(), "@");
        assert_eq!(cell.fg, Color::Rgb(255, 128, 0));
        assert!(cell.modifier.contains(Modifier::BOLD));
        // On a tile you can't see, nothing is drawn.
        let hidden = (-20..20)
            .flat_map(|d| [pos.offset(d, 6), pos.offset(d, -6)])
            .find(|p| vis.get(*p).is_some_and(|v| *v != Visibility::Visible))
            .expect("a tile out of sight");
        arrive(&mut st, "castellan", (place, hidden));
        let mut buf = Buffer::empty(area);
        draw(&mut buf, area, hidden.offset(-3, -3), &st);
        assert!(buf.content().iter().all(|c| c.symbol() != "@"));
    }

    #[test]
    fn a_new_city_forgets_visitors_and_lights() {
        let mut st = flat();
        let a = spot(&st, 1);
        arrive(&mut st, "seren", a);
        st.missions.lights = Some(0.2);
        st.enter_city(
            Generator::run(GenParams {
                seed: 9,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        assert!(st.missions.visitors.is_empty());
        assert_eq!(st.missions.lights, None);
    }
}
