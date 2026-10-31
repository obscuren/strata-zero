//! How a track shows in the JOBS pane and the deck's JOBS tab (spec 5, 6).

use super::route::NoLeg;
use super::{Source, distance, is_tracked, leg_now, next_now};
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

/// The card's last line: whether `source` is the track, and what `t` does.
pub fn detail_line(st: &AppState, source: &Source) -> Line<'static> {
    if is_tracked(st, source) {
        Line::from(Span::styled(
            "⌖ tracking · t stops",
            Style::default().fg(CYAN),
        ))
    } else {
        Line::from(Span::styled("t track", Style::default().fg(DIM)))
    }
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
    (run.state == RunState::Open).then_some(Source::Story {
        mission,
        step: run.at,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{outside, steps_off};
    use super::*;
    use crate::app::Screen;
    use crate::jobs::testutil::a_fixer;
    use crate::jobs::{KindTag, generate};
    use crate::missions::ui::{jobs_pane, story_line_count};
    use crate::screens::deck::{Deck, Detail, Tab};
    use crate::screens::testutil::key;
    use citygen::{Place, StratumKind};
    use crossterm::event::KeyCode;

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

    /// A fresh job on offer from a fixer.
    fn offered(st: &mut AppState) -> JobId {
        let fixer = a_fixer(st);
        st.jobs.fixer_mut(fixer).standing = 10;
        let job = (0..40)
            .find_map(|n| generate::generate(st, fixer, KindTag::Fetch, n))
            .unwrap();
        st.jobs.push(job)
    }

    /// The deck on JOBS with the row of `want` selected.
    fn deck_on(st: &AppState, want: Detail) -> Deck {
        let mut d = Deck::new(st.tick);
        d.tab = Tab::Jobs;
        d.row = d
            .rows(st)
            .iter()
            .position(|r| r.detail.as_ref() == Some(&want))
            .expect("the row is on the tab");
        d
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

    #[test]
    fn t_on_a_taken_job_tracks_and_stops() {
        let mut st = outside();
        let id = st.jobs.active().unwrap().id;
        let mut d = deck_on(&st, Detail::Job(id));
        d.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert!(matches!(
            st.track.as_ref().map(|t| t.source),
            Some(Source::Job { job, .. }) if job == id
        ));
        d.update(&mut st, &[key(KeyCode::Char('T'))]);
        assert!(st.track.is_none());
    }

    #[test]
    fn t_on_an_offered_job_says_take_it_first() {
        let mut st = outside();
        let id = offered(&mut st);
        let mut d = deck_on(&st, Detail::Job(id));
        d.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert!(st.track.is_none());
        let latest = st.log.latest().unwrap();
        assert_eq!(latest.text, "take it first");
        assert_eq!(latest.kind, crate::messages::Kind::Warn);
    }

    #[test]
    fn t_on_the_story_tracks_its_step() {
        let mut st = outside();
        open_drop(&mut st);
        let mut d = deck_on(&st, Detail::Mission(0));
        d.update(&mut st, &[key(KeyCode::Char('t'))]);
        let at = st.missions.runs[0].at;
        assert_eq!(
            st.track.as_ref().map(|t| t.source),
            Some(Source::Story {
                mission: 0,
                step: at
            })
        );
    }

    #[test]
    fn the_detail_says_the_track_state() {
        let mut st = outside();
        let id = st.jobs.active().unwrap().id;
        let last = |st: &AppState| text(crate::jobs::ui::detail(st, id).last().unwrap());
        assert_eq!(last(&st), "t track");
        let src = job_source(&st, id).unwrap();
        crate::track::set(&mut st, src).unwrap();
        assert_eq!(last(&st), "⌖ tracking · t stops");
        // Not taken: no line.
        let offer = offered(&mut st);
        let card = crate::jobs::ui::detail(&st, offer);
        assert!(!text(card.last().unwrap()).contains("track"));
        // A story card ends the same way.
        open_drop(&mut st);
        let card = crate::missions::ui::detail(&st, 0);
        assert_eq!(text(card.last().unwrap()), "t track");
    }

    #[test]
    fn dropping_a_tracked_job_clears_the_line_before_the_deck_closes() {
        let mut st = outside();
        let id = st.jobs.active().unwrap().id;
        let mut d = deck_on(&st, Detail::Job(id));
        d.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert!(st.track.is_some());
        // The drop popup acts outside the deck; the deck regains control.
        crate::jobs::ui::drop(&mut st, id).unwrap();
        d.update(&mut st, &[]);
        assert!(st.track.is_none());
        assert!(jobs_pane(&st).iter().all(|l| !text(l).contains('⌖')));
    }
}
