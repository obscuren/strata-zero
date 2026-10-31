//! What a step does to the game (spec §4), its text filled as it runs.

use super::{Body, Call, Effect, FeedView, cast, fill, person_of, role_name};
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
    if let Some(pos) = cast::work_tile(st, person).or_else(|| world.pos_of(person)) {
        let body = Body {
            person,
            name: role_name(st, role),
            place: Place::Stratum(StratumKind::Street),
            pos,
            look: fill(st, look),
        };
        st.missions.bodies.push(body);
    }
    if let Some(w) = st.world.as_mut() {
        w.kill(person);
    }
}
