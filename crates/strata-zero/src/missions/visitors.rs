//! Visitors: authored people the story puts on the map. Never in the sim, so jobs, talk
//! and the inspector don't see them; they are drawn, looked at, and they block the way
//! (scenes spec §4). Also the flat's lights (§8), and a held clinic after hours.

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

/// How much of the flat's light is left while you sleep.
const ASLEEP_LIGHTS: f32 = 0.4;

/// A held clinic after hours (doctor spec §5): its chair's floor at this much of its
/// colour, but within `POOL` tiles (Manhattan) of the chair, which keeps it, shifted
/// `COLD`.
const AFTER_HOURS: f32 = 0.35;
const POOL: i32 = 3;
/// The pool's shift: red down by this, blue up by it.
const COLD: f32 = 0.2;

/// A colour in the chair's cold pool; named colours are left as they are.
fn cold(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            (f32::from(r) * (1.0 - COLD)).round() as u8,
            g,
            (f32::from(b) * (1.0 + COLD)).round().min(255.0) as u8,
        ),
        other => other,
    }
}

/// While you stand on a held clinic's chair floor, the floor after hours: dim, but for
/// a cold pool around the chair.
fn after_hours(buf: &mut Buffer, area: Rect, origin: Pos, st: &AppState, place: Place) {
    let Place::Floor { building, level } = place else {
        return;
    };
    let Some((_, floor)) = st.interiors.floor(building, level) else {
        return;
    };
    let Some(chair) = floor.chair() else {
        return;
    };
    if !crate::clinic::held(st, building) {
        return;
    }
    for (p, _) in floor.tiles.iter() {
        if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
            let cell = &mut buf[(sx, sy)];
            if p.manhattan(chair) <= POOL {
                cell.fg = cold(cell.fg);
                cell.bg = cold(cell.bg);
            } else {
                cell.fg = scale(cell.fg, AFTER_HOURS);
                cell.bg = scale(cell.bg, AFTER_HOURS);
            }
        }
    }
}

/// Draws the flat's lights and a held clinic after hours, then the visitors, over the
/// map view in `area`. The lights scale every cell of the flat (walls too); the
/// visitors come after, bright.
pub fn draw(buf: &mut Buffer, area: Rect, origin: Pos, st: &AppState) {
    let Some(me) = st.player.as_ref() else {
        return;
    };
    // Asleep the flat is dark whatever the story left on.
    let lights = match &st.rest {
        Some(rest) if rest.kind == crate::rest::Kind::Sleep => {
            Some(st.missions.lights.unwrap_or(1.0).min(ASLEEP_LIGHTS))
        }
        _ => st.missions.lights,
    };
    if let (Some(level), Some((place, rect))) = (lights, marks::flat_rect(st))
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
    after_hours(buf, area, origin, st, me.place);
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
    fn a_held_clinic_draws_dim_with_a_cold_pool() {
        let (mut st, b, level, chair) = crate::clinic::testutil::inside();
        super::super::tests::cast_doc(&mut st, b);
        let size = st.interiors.floor(b, level).unwrap().1.tiles.size();
        let inside = |p: &Pos| p.x >= 0 && p.y >= 0 && p.x < size.w as i32 && p.y < size.h as i32;
        let far = [
            (6, 0),
            (-6, 0),
            (0, 6),
            (0, -6),
            (3, 3),
            (-3, -3),
            (3, -3),
            (-3, 3),
        ]
        .into_iter()
        .map(|(dx, dy)| chair.offset(dx, dy))
        .find(inside)
        .expect("a tile six from the chair");
        let beside = chair
            .neighbors4()
            .find(inside)
            .expect("a tile beside the chair");
        let grey = Color::Rgb(120, 120, 120);
        // The floor drawn flat grey around the chair: (beside, far).
        let drawn = |st: &AppState| {
            let origin = chair.offset(-10, -10);
            let area = Rect::new(0, 0, 21, 21);
            let mut buf = Buffer::empty(area);
            buf.set_style(area, ratatui::style::Style::default().fg(grey).bg(grey));
            draw(&mut buf, area, origin, st);
            let at = |p: Pos| buf[((p.x - origin.x) as u16, (p.y - origin.y) as u16)].fg;
            (at(beside), at(far))
        };
        assert_eq!(drawn(&st), (grey, grey), "open, as it is");
        crate::missions::effects::apply(
            &mut st,
            0,
            0,
            &crate::missions::Effect::Hold("doc".into()),
        );
        let (near, away) = drawn(&st);
        assert_eq!(away, scale(grey, 0.35), "dim away from the chair");
        let Color::Rgb(r, g, bl) = near else {
            panic!("{near:?}")
        };
        assert_eq!(g, 120, "full colour beside the chair");
        assert!(bl > r, "cold: {near:?}");
        assert_eq!((r, bl), (96, 144));
        // Off that floor, nothing.
        let out = super::super::tests::outside_door(&st, b);
        super::super::tests::on_street(&mut st, out);
        assert_eq!(drawn(&st), (grey, grey));
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
