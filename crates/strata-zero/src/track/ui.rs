//! How a track shows in the JOBS pane, and what the deck's JOBS tab tracks (spec 5, 6).

use super::route::NoLeg;
use super::{Source, distance, leg_now, next_now};
use crate::app::AppState;
use crate::jobs::{JobId, State};
use crate::missions::RunState;
use crate::ui::chrome::{CYAN, DIM};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// The pane's first line while tracking: ` ⌖ {label} · {m} m`, then `· {next}`; or why
/// there is no distance.
pub fn pane_line(st: &AppState) -> Option<Line<'static>> {
    let t = st.track.as_ref()?;
    let tail = match leg_now(st) {
        Some(Err(NoLeg::NoWay)) => " · no way from here".to_string(),
        Some(Err(NoLeg::JackedIn)) => " · jacked in".to_string(),
        _ => {
            let m = distance(st).unwrap_or(0);
            match next_now(st) {
                Some(next) => format!(" · {m} m · {next}"),
                None => format!(" · {m} m"),
            }
        }
    };
    Some(Line::from(vec![
        Span::raw(" "),
        Span::styled("⌖", Style::default().fg(CYAN)),
        Span::styled(format!(" {}{tail}", t.label), Style::default().fg(DIM)),
    ]))
}

/// What `t` tracks on a taken job: its first objective not yet done.
pub fn job_source(st: &AppState, id: JobId) -> Option<Source> {
    let j = st.jobs.get(id).filter(|j| j.state == State::Taken)?;
    let objective = j.objectives.iter().position(|o| !o.done)?;
    Some(Source::Job { job: id, objective })
}

/// What `t` tracks on an open story mission: the step it waits on.
pub fn story_source(st: &AppState, mission: usize) -> Option<Source> {
    let run = st.missions.runs.get(mission)?;
    (run.state == RunState::Open).then_some(Source::Story(crate::runs::ids::StepRef::new(
        mission, run.at,
    )))
}

#[cfg(test)]
mod tests {
    use super::super::tests::{outside, steps_off};
    use super::*;
    use crate::missions::ui::{jobs_pane, story_line_count};
    use citygen::{Place, StratumKind};

    fn text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn street_spot(st: &AppState, n: u32) -> Source {
        Source::Spot {
            place: Place::Stratum(StratumKind::Street),
            pos: steps_off(st, n),
            radius: 1,
        }
    }

    fn open_drop(st: &mut AppState) {
        crate::missions::begin(st);
        crate::missions::end(st, 0, 0);
    }

    #[test]
    fn the_pane_leads_with_the_track() {
        let mut st = outside();
        let before = jobs_pane(&st).len();
        assert!(pane_line(&st).is_none());
        let spot = street_spot(&st, 12);
        crate::track::set(&mut st, spot).unwrap();
        let label = crate::track::label(&st, &spot);
        let pane = jobs_pane(&st);
        assert_eq!(text(&pane[0]), format!(" ⌖ {label} · 12 m"));
        assert_eq!(pane[0].spans[1].style.fg, Some(CYAN));
        assert_eq!(pane[0].spans[2].style.fg, Some(DIM));
        assert_eq!(pane.len(), before + 1);
        st.track = None;
        assert_eq!(jobs_pane(&st).len(), before);
    }

    #[test]
    fn the_pane_names_the_next_leg() {
        let mut st = outside();
        let b = st.home.unwrap().home.building;
        let lift = st.interiors.floor(b, 0).unwrap().0.lift.unwrap();
        let p = st.player.as_mut().unwrap();
        p.place = Place::Floor {
            building: b,
            level: 0,
        };
        p.pos = lift;
        let ids: Vec<_> = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .map(|x| x.id)
            .filter(|id| *id != b)
            .collect();
        let found = ids.into_iter().any(|id| {
            let source = Source::Listing(crate::track::ListingId::Building(id));
            crate::track::set(&mut st, source).is_ok()
                && pane_line(&st).is_some_and(|l| text(&l).contains("· out to the street"))
        });
        assert!(found, "some building is reached by way of the street");
    }

    #[test]
    fn no_way_and_jacked_in_say_so() {
        let mut st = outside();
        let spot = street_spot(&st, 12);
        crate::track::set(&mut st, spot).unwrap();
        let label = st.track.as_ref().unwrap().label.clone();
        st.track.as_mut().unwrap().walk.leg = Err(NoLeg::NoWay);
        assert_eq!(
            text(&pane_line(&st).unwrap()),
            format!(" ⌖ {label} · no way from here")
        );
        st.track.as_mut().unwrap().walk.leg = Err(NoLeg::JackedIn);
        assert_eq!(
            text(&pane_line(&st).unwrap()),
            format!(" ⌖ {label} · jacked in")
        );
    }

    #[test]
    fn the_story_count_includes_the_track_line() {
        let mut st = outside();
        open_drop(&mut st);
        let story = crate::missions::ui::pane_lines(&st).len();
        assert!(story > 0);
        assert_eq!(story_line_count(&st), story);
        let spot = street_spot(&st, 12);
        crate::track::set(&mut st, spot).unwrap();
        assert_eq!(story_line_count(&st), story + 1);
    }
}
