//! What a step does to the game (spec §4), its text filled as it runs.

use super::{
    Body, Call, CodeOf, Effect, FeedView, Mark, YOU, cast, code_slot, daze, fill, marks, news,
    person_of, role_name, scene, visitors,
};
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
            code,
        } => {
            // The code is learned as the mail lands; `{code}` says it, or nothing.
            let digits = code
                .as_ref()
                .and_then(|c| learn_code(st, c))
                .map(|c| format!("{c:04}"))
                .unwrap_or_default();
            let text = |st: &AppState, t: &str| fill(st, &t.replace(&code_slot(), &digits));
            let mail = crate::credentials::Mail {
                from: role_name(st, from),
                subject: text(st, subject),
                body: body.iter().map(|b| text(st, b)).collect(),
                read: false,
                at: st.net.now(st),
                job: None,
            };
            st.journal.mails.push(mail);
        }
        Effect::Log(text) => {
            let text = fill(st, text);
            st.story(text);
        }
        Effect::Scene(id) => st.missions.scenes.push_back((mission, step, id.clone())),
        Effect::Arrive { role, at } => arrive(st, role, at),
        Effect::Leave(role) => visitors::leave(st, role),
        Effect::Lights(level) => lights(st, *level),
        Effect::News(id) => news::put_out(st, id, mission, step),
        Effect::Daze(seconds) => daze::start(st, *seconds),
        Effect::Hold(role) => {
            if let (Some(id), Some(w)) = (person_of(st, role), st.world.as_mut()) {
                w.keep_at_work(id);
            }
        }
        Effect::Release(role) => {
            st.missions.releasing.insert(role.clone());
        }
    }
}

/// A mail's code into the journal (`Source::Mail`): the street code of the role's work
/// building. None when nobody plays the role or they work in no building.
fn learn_code(st: &mut AppState, code: &CodeOf) -> Option<u16> {
    let CodeOf::Clinic(role) = code;
    let b = cast::work_building(st, person_of(st, role)?)?;
    let code = crate::credentials::street_code(st, b);
    let at = st.net.now(st);
    st.journal.learn(crate::credentials::Code {
        building: b,
        level: 0,
        number: crate::credentials::STREET_DOOR,
        household: None,
        code,
        source: crate::credentials::Source::Mail,
        noted_by: None,
        at,
    });
    Some(code)
}

/// Lets go each role `Release` named once you are not inside their work building, so
/// they never walk out past you. A role nobody plays is dropped.
pub(super) fn release_waiting(st: &mut AppState) {
    if st.missions.releasing.is_empty() {
        return;
    }
    let inside = match st.player.as_ref().map(|p| p.place) {
        Some(Place::Floor { building, .. }) => Some(building),
        _ => None,
    };
    let roles: Vec<String> = st.missions.releasing.iter().cloned().collect();
    for role in roles {
        let who = person_of(st, &role);
        if inside.is_some() && who.and_then(|id| cast::work_building(st, id)) == inside {
            continue;
        }
        if let (Some(id), Some(w)) = (who, st.world.as_mut()) {
            w.release(id);
        }
        st.missions.releasing.remove(&role);
    }
}

/// The role's visitor stands on the mark, or you do, at once: a cut, you seeing from
/// there. A tile another actor holds gives the free one beside it; a mark that
/// resolves to nothing (or has no room beside it) leaves things as they are. As the
/// effect and as a scene's beat.
pub(super) fn arrive(st: &mut AppState, role: &str, at: &Mark) {
    let walker = (role != YOU).then_some(role);
    let spot = marks::resolve(st, at, walker).and_then(|(place, pos)| {
        if scene::taken(st, role, place, pos) {
            marks::beside_tile(st, place, pos, walker)
        } else {
            Some((place, pos))
        }
    });
    match spot {
        Some((place, pos)) if role == YOU => scene::put(st, YOU, place, pos),
        Some(spot) => visitors::arrive(st, role, spot),
        None if st.dev => st.warn(format!("scene · {at:?} not found")),
        None => {}
    }
}

/// Your flat's light level; 1.0 is normal.
pub(super) fn lights(st: &mut AppState, level: f32) {
    st.missions.lights = (level < 1.0).then_some(level);
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
            near: false,
            left: None,
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

    #[test]
    fn the_daze_effect_starts_a_daze() {
        let mut st = state_home();
        st.tick = 77;
        apply(&mut st, 0, 0, &Effect::Daze(4.0));
        assert_eq!(
            st.missions.daze,
            Some(crate::missions::daze::Daze {
                start: 77,
                ticks: 120
            })
        );
    }

    #[test]
    fn the_news_effect_puts_out_the_item() {
        let mut st = state_home();
        apply(&mut st, 1, 2, &Effect::News("marlo".into()));
        let n = crate::missions::news::current(&st).unwrap();
        assert_eq!((n.id.as_str(), n.mission, n.step), ("marlo", 1, 2));
    }

    /// At night beside the clinic's chair, its doc cast as `doc`: the state, the clinic
    /// and the doc.
    fn by_the_chair() -> (AppState, citygen::BuildingId, sim::PersonId) {
        let (mut st, b, _, _) = crate::clinic::testutil::inside();
        let doc = super::super::tests::cast_doc(&mut st, b);
        (st, b, doc)
    }

    fn held(st: &AppState, doc: sim::PersonId) -> bool {
        st.world.as_ref().unwrap().is_held(doc)
    }

    #[test]
    fn release_waits_until_you_are_out() {
        let (mut st, b, doc) = by_the_chair();
        apply(&mut st, 0, 0, &Effect::Hold("doc".into()));
        assert!(held(&st, doc));
        apply(&mut st, 0, 0, &Effect::Release("doc".into()));
        crate::missions::tick(&mut st);
        assert!(held(&st, doc), "not while you are inside");
        // Out on the street: the next tick lets her go.
        let out = super::super::tests::outside_door(&st, b);
        super::super::tests::on_street(&mut st, out);
        crate::missions::tick(&mut st);
        assert!(!held(&st, doc));
        assert!(st.missions.releasing.is_empty());
        // Nobody cast: the release is dropped, nothing else.
        apply(&mut st, 0, 0, &Effect::Release("nobody".into()));
        crate::missions::tick(&mut st);
        assert!(st.missions.releasing.is_empty());
    }

    fn mail(code: Option<&str>) -> Effect {
        Effect::Mail {
            from: "doc".into(),
            subject: "ten".into(),
            body: vec!["Shutter's down. Code's {code}.".into(), "— S".into()],
            code: code.map(|r| crate::missions::CodeOf::Clinic(r.into())),
        }
    }

    #[test]
    fn the_mail_learns_the_code_as_it_lands() {
        use crate::credentials::{STREET_DOOR, Source, street_code};
        let (mut st, b, _) = by_the_chair();
        let code = street_code(&st, b);
        apply(&mut st, 0, 0, &mail(Some("doc")));
        let learned = st.journal.code_for(b, 0, STREET_DOOR).expect("learned");
        assert_eq!((learned.code, learned.source), (code, Source::Mail));
        assert_eq!(learned.noted_by, None);
        let m = st.journal.mails.last().unwrap();
        assert_eq!(m.body[0], format!("Shutter's down. Code's {code:04}."));
        assert_eq!(m.body[1], "— S");
        let name = crate::net::building_name(&st, b);
        let lines: Vec<String> = crate::credentials::entries(&st)
            .into_iter()
            .flat_map(|(_, _, es)| es)
            .map(|e| e.text)
            .collect();
        assert!(
            lines.iter().any(|t| t.starts_with(&format!(
                "door code {code:04} · `{name}`, street door · from a mail on the rig"
            ))),
            "{lines:?}"
        );
    }

    #[test]
    fn a_code_for_nobody_learns_nothing() {
        let (mut st, _, _) = by_the_chair();
        let codes = st.journal.codes().len();
        apply(&mut st, 0, 0, &mail(Some("nobody")));
        apply(&mut st, 0, 0, &mail(None));
        assert_eq!(st.journal.codes().len(), codes);
        for m in &st.journal.mails {
            assert_eq!(m.body[0], "Shutter's down. Code's .");
        }
        assert_eq!(st.journal.mails.len(), 2);
    }

    fn arrive(role: &str, at: Mark) -> Effect {
        Effect::Arrive {
            role: role.into(),
            at,
        }
    }

    #[test]
    fn arrive_puts_you_there_at_once() {
        let mut st = state_home();
        let b = st.home.unwrap().home.building;
        let below = st.city.as_ref().unwrap().portals(b).first().unwrap().1;
        super::super::tests::on_street(&mut st, below);
        let want = marks::resolve(&st, &Mark::HomeInside(2), None).unwrap();
        apply(&mut st, 0, 0, &arrive("you", Mark::HomeInside(2)));
        let me = st.player.as_ref().unwrap();
        assert_eq!((me.place, me.pos), want);
        assert_eq!(
            me.vis().get(want.1),
            Some(&crate::render::Visibility::Visible),
            "you see from where you are put"
        );
        assert!(st.missions.visitors.is_empty(), "you are no visitor");
    }

    #[test]
    fn arrive_on_someone_lands_beside_them() {
        let mut st = in_the_flat();
        let me = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        let walkable = |st: &AppState, p: Pos| {
            crate::render::map::MapRef::for_place(
                st.city.as_ref().unwrap(),
                &st.interiors,
                &st.constructs,
                me.0,
                st.minute_of_day(),
            )
            .is_some_and(|m| crate::render::map::TileMap::walkable(&m, p))
        };
        // Onto you: the tile beside you.
        apply(&mut st, 0, 0, &arrive("seren", Mark::You));
        let seren = marks::resolve(&st, &Mark::Role("seren".into()), None).unwrap();
        assert_eq!(seren.0, me.0);
        assert_eq!(seren.1.chebyshev(me.1), 1);
        assert!(walkable(&st, seren.1));
        // Onto her: beside her, on neither of you.
        apply(
            &mut st,
            0,
            0,
            &arrive("castellan", Mark::Role("seren".into())),
        );
        let castellan = marks::resolve(&st, &Mark::Role("castellan".into()), None).unwrap();
        assert_eq!(castellan.1.chebyshev(seren.1), 1);
        assert!(castellan.1 != me.1 && walkable(&st, castellan.1));
        // You onto her: beside her, she stays.
        apply(&mut st, 0, 0, &arrive("you", Mark::Role("seren".into())));
        let now = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        assert_eq!(now.1.chebyshev(seren.1), 1);
        assert!(now.1 != castellan.1 && walkable(&st, now.1));
        assert_eq!(
            marks::resolve(&st, &Mark::Role("seren".into()), None),
            Some(seren)
        );
        // A visitor arriving where they already stand stays put.
        apply(&mut st, 0, 0, &arrive("seren", Mark::Role("seren".into())));
        assert_eq!(
            marks::resolve(&st, &Mark::Role("seren".into()), None),
            Some(seren)
        );
    }
}
