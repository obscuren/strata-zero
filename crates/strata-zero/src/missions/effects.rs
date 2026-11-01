//! What a step does to the game (spec §4), its text filled as it runs.

use super::{Body, Call, Effect, FeedView, cast, fill, marks, person_of, role_name, visitors};
use crate::app::AppState;
use citygen::{Place, StratumKind};
use game::ItemId;

/// Runs one effect of step `step` of mission `mission`.
pub fn apply(st: &mut AppState, mission: usize, step: usize, e: &Effect) {
    match e {
        Effect::Feed { from, text, close } => {
            let view = FeedView {
                mission,
                step,
                from: from.clone(),
                text: text.iter().map(|t| fill(st, t)).collect(),
                close: fill(st, close),
            };
            st.missions.feeds.push_back(view);
        }
        Effect::Call { from, lines } => {
            let call = Call {
                mission,
                step,
                from: from.clone(),
                lines: lines.iter().map(|l| fill(st, l)).collect(),
                rang: 0,
                answered: None,
                line: 0,
                shown: 0,
                full: false,
            };
            super::call::ring(st, call);
        }
        Effect::Give { item, count } => {
            let id = ItemId::new(item);
            let name = st
                .content
                .item(&id)
                .map_or_else(|| item.clone(), |i| i.name.clone());
            st.character.give_item(&st.content, id, *count);
            st.story(format!("`{name}` ×{count} · in your pack"));
        }
        Effect::Take { item, count } => {
            game::remove_from_pack(&mut st.character, &ItemId::new(item), *count);
        }
        Effect::Dead { role, look } => dead(st, role, look),
        Effect::Objective(text) => {
            let text = fill(st, text);
            if let Some(run) = st.missions.runs.get_mut(mission) {
                run.objectives.push(text);
            }
        }
        Effect::Mail {
            from,
            subject,
            body,
        } => {
            let mail = crate::credentials::Mail {
                from: role_name(st, from),
                subject: fill(st, subject),
                body: body.iter().map(|b| fill(st, b)).collect(),
                read: false,
                at: st.net.now(st),
            };
            st.journal.mails.push(mail);
        }
        Effect::Log(text) => {
            let text = fill(st, text);
            st.story(text);
        }
        Effect::Scene(id) => st.missions.scenes.push_back((mission, step, id.clone())),
        Effect::Arrive { role, at } => match marks::resolve(st, at, Some(role)) {
            Some(spot) => visitors::arrive(st, role, spot),
            None if st.dev => st.warn(format!("scene · {at:?} not found")),
            None => {}
        },
        Effect::Leave(role) => visitors::leave(st, role),
        Effect::Lights(level) => st.missions.lights = (*level < 1.0).then_some(*level),
    }
}

/// The role's person dies: a body at their work tile, the sim done with them. Once.
fn dead(st: &mut AppState, role: &str, look: &str) {
    let Some(person) = person_of(st, role) else {
        return;
    };
    let Some(world) = st.world.as_ref() else {
        return;
    };
    if world.is_dead(person) {
        return;
    }
    if let Some(pos) = cast::body_tile(st, person) {
        let body = Body {
            person,
            name: role_name(st, role),
            place: Place::Stratum(StratumKind::Street),
            pos,
            look: fill(st, look),
        };
        st.missions.bodies.push(body);
    } else if st.dev {
        st.warn(format!("story · no place for {role}'s body"));
    }
    if let Some(w) = st.world.as_mut() {
        w.kill(person);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::missions::{Mark, marks, visitors};
    use crate::walker::Player;
    use citygen::Pos;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};

    fn in_the_flat() -> AppState {
        let mut st = state_home();
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
        st
    }

    /// The colours of a flat tile and of one far outside it, drawn over a flat grey map.
    fn drawn(st: &AppState) -> [(Color, Color); 2] {
        let (_, flat) = marks::flat_rect(st).unwrap();
        let origin = Pos::new(flat.x - 10, flat.y - 10);
        let area = Rect::new(0, 0, 60, 40);
        let mut buf = Buffer::empty(area);
        buf.set_style(
            area,
            Style::default()
                .fg(Color::Rgb(200, 100, 50))
                .bg(Color::Rgb(100, 50, 20)),
        );
        visitors::draw(&mut buf, area, origin, st);
        let at = |p: Pos| {
            let c = &buf[((p.x - origin.x) as u16, (p.y - origin.y) as u16)];
            (c.fg, c.bg)
        };
        [at(flat.center()), at(origin)]
    }

    #[test]
    fn lights_dim_the_flat() {
        let mut st = in_the_flat();
        let full = drawn(&st);
        assert_eq!(full[0], (Color::Rgb(200, 100, 50), Color::Rgb(100, 50, 20)));
        apply(&mut st, 0, 0, &Effect::Lights(0.5));
        assert_eq!(st.missions.lights, Some(0.5));
        let dim = drawn(&st);
        assert_eq!(dim[0], (Color::Rgb(100, 50, 25), Color::Rgb(50, 25, 10)));
        assert_eq!(dim[1], full[1], "outside the flat is unchanged");
        apply(&mut st, 0, 0, &Effect::Lights(1.0));
        assert_eq!(st.missions.lights, None);
        assert_eq!(drawn(&st), full);
    }

    #[test]
    fn arrive_leave_and_scene() {
        let mut st = in_the_flat();
        let want = marks::resolve(&st, &Mark::HomeInside(2), None).unwrap();
        let arrive = Effect::Arrive {
            role: "seren".into(),
            at: Mark::HomeInside(2),
        };
        apply(&mut st, 0, 0, &arrive);
        assert_eq!(st.missions.visitors.len(), 1);
        assert_eq!(visitors::at(&st, want.0, want.1).unwrap().role, "seren");
        // A mark that resolves to nothing leaves things as they are.
        st.home = None;
        apply(&mut st, 0, 0, &arrive);
        assert_eq!(st.missions.visitors.len(), 1);
        apply(&mut st, 0, 0, &Effect::Leave("seren".into()));
        assert!(st.missions.visitors.is_empty());
        apply(&mut st, 3, 4, &Effect::Scene("film".into()));
        assert_eq!(
            st.missions.scenes,
            [(3, 4, "film".to_string())]
                .into_iter()
                .collect::<std::collections::VecDeque<_>>()
        );
    }
}
