//! What the player sees of the story: its lines in the JOBS pane, its rows and dossier
//! in the deck's JOBS tab.

use super::{RunState, role_name};
use crate::app::AppState;
use crate::screens::deck::{Detail, Row, Tone};
use crate::ui::chrome::{DIM, MAGENTA, TEXT};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

fn dim(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(DIM)))
}

fn shown(state: RunState) -> bool {
    matches!(state, RunState::Open | RunState::Done { .. })
}

/// Who the mission is from, by the name their role plays.
fn from_name(st: &AppState, mission: usize) -> String {
    st.book
        .missions
        .get(mission)
        .map_or_else(String::new, |m| role_name(st, &m.from))
}

/// Each open mission: its title and who it is from, then its current objective.
pub fn pane_lines(st: &AppState) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for (i, run) in st.missions.runs.iter().enumerate() {
        let (RunState::Open, Some(m)) = (run.state, st.book.missions.get(i)) else {
            continue;
        };
        out.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                m.title.to_uppercase(),
                Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" · {} · story", from_name(st, i)),
                Style::default().fg(DIM),
            ),
        ]));
        if let Some(now) = run.objectives.last() {
            out.push(Line::from(Span::styled(
                format!(" » {now}"),
                Style::default().fg(TEXT),
            )));
        }
    }
    out
}

/// The lines the pane never caps: the track's, then the story's.
pub fn story_line_count(st: &AppState) -> usize {
    usize::from(st.track.is_some()) + pane_lines(st).len()
}

/// The whole JOBS pane: the track, the story's lines, then the job in hand.
pub fn jobs_pane(st: &AppState) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = crate::track::ui::pane_line(st).into_iter().collect();
    out.extend(pane_lines(st));
    out.extend(crate::jobs::ui::pane_lines(st));
    out
}

/// The STORY header, the missions open or done, then the JOBS header; nothing at all
/// while no mission has started.
pub fn deck_rows(st: &AppState) -> Vec<Row> {
    let header = |text: &str| Row {
        text: text.into(),
        tone: Tone::Dim,
        detail: None,
        spans: None,
    };
    let mut out = Vec::new();
    for (i, run) in st.missions.runs.iter().enumerate() {
        let Some(m) = st.book.missions.get(i) else {
            continue;
        };
        let (text, tone) = match run.state {
            RunState::Open => (format!("  {}", m.title), Tone::Normal),
            RunState::Done { .. } => (format!("✓ {}", m.title), Tone::Done),
            _ => continue,
        };
        out.push(Row {
            text,
            tone,
            detail: Some(Detail::Mission(i)),
            spans: None,
        });
    }
    if out.is_empty() {
        return out;
    }
    out.insert(0, header("STORY"));
    out.push(header("JOBS"));
    out
}

/// The dev console's `story`: each role with the person cast in it, then each mission
/// with its state and the step it waits on.
pub fn console_lines(st: &AppState) -> Vec<String> {
    if !st.missions.begun {
        return vec!["the story has not begun in this city".to_string()];
    }
    let mut out = Vec::new();
    for role in st.book.cast.keys() {
        out.push(match super::person_of(st, role) {
            Some(id) => {
                let name = st
                    .world
                    .as_ref()
                    .and_then(|w| w.people().iter().find(|p| p.id == id))
                    .map_or("?", |p| p.name.as_str());
                format!("{role} → {} {name}", id.0)
            }
            None => format!("{role} → nobody"),
        });
    }
    for (i, m) in st.book.missions.iter().enumerate() {
        let Some(run) = st.missions.runs.get(i) else {
            continue;
        };
        let state = match run.state {
            RunState::Waiting => "waiting",
            RunState::Open => "open",
            RunState::Done { .. } => "done",
            RunState::Uncast => "uncast",
        };
        let waits = match (run.state, m.steps.get(run.at)) {
            (RunState::Open | RunState::Waiting, Some(step)) => {
                format!(" · waits on {}: {:?}", step.id, step.when)
            }
            _ => String::new(),
        };
        out.push(format!("{} · {state}{waits}", m.id));
    }
    out
}

/// The deck's dossier on one mission.
pub fn detail(st: &AppState, mission: usize) -> Vec<Line<'static>> {
    let (Some(m), Some(run)) = (st.book.missions.get(mission), st.missions.runs.get(mission))
    else {
        return Vec::new();
    };
    if !shown(run.state) {
        return Vec::new();
    }
    let done = matches!(run.state, RunState::Done { .. });
    let mut out = vec![
        Line::from(Span::styled(
            m.title.clone(),
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
        )),
        dim(format!(
            "from {} · story · {}",
            from_name(st, mission),
            if done { "done" } else { "open" }
        )),
        Line::from(""),
    ];
    let last = run.objectives.len().saturating_sub(1);
    for (i, o) in run.objectives.iter().enumerate() {
        out.push(if i < last || done {
            dim(format!("☑ {o}"))
        } else {
            Line::from(Span::styled(format!("☐ {o}"), Style::default().fg(TEXT)))
        });
    }
    if let Some(src) = crate::track::ui::story_source(st, mission) {
        out.push(Line::from(""));
        out.push(crate::track::ui::detail_line(st, &src));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;

    fn text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// The shipped story begun, the welcome closed: the drop is open with its objective.
    fn open_drop() -> AppState {
        let mut st = state_home();
        super::super::begin(&mut st);
        super::super::end(&mut st, 0, 0);
        st
    }

    #[test]
    fn the_pane_shows_the_drop_above_the_job() {
        let st = open_drop();
        let story = pane_lines(&st);
        assert_eq!(text(&story[0]), " THE DROP · Dez · story");
        assert_eq!(story[0].spans[1].style.fg, Some(MAGENTA));
        assert_eq!(story[0].spans[2].style.fg, Some(DIM));
        assert!(text(&story[1]).starts_with(" » Hand the package to "));
        assert_eq!(story[1].spans[0].style.fg, Some(TEXT));
        // The sidebar puts them before the jobs' own lines.
        let all = jobs_pane(&st);
        assert_eq!(text(&all[0]), text(&story[0]));
        assert_eq!(
            text(&all[story.len()]),
            text(&crate::jobs::ui::pane_lines(&st)[0])
        );
    }

    #[test]
    fn the_tab_has_story_then_jobs() {
        let st = open_drop();
        let rows = deck_rows(&st);
        assert_eq!(rows[0].text, "STORY");
        assert!(rows[0].detail.is_none());
        assert_eq!(rows[1].text, "  The drop");
        assert_eq!(rows[1].detail, Some(Detail::Mission(0)));
        assert_eq!(rows[2].text, "JOBS");
        assert!(rows[2].detail.is_none());
        assert_eq!(rows.len(), 3);
        let mut deck = crate::screens::deck::Deck::new(st.tick);
        deck.tab = crate::screens::deck::Tab::Jobs;
        let tab = deck.rows(&st);
        let jobs = crate::jobs::ui::deck_rows(&st);
        assert!(!jobs.is_empty());
        assert_eq!(tab.len(), rows.len() + jobs.len());
        assert!(matches!(tab[3].detail, Some(Detail::Job(_))));
    }

    #[test]
    fn the_tab_is_unchanged_without_a_story() {
        let mut st = state_home();
        st.missions.runs.clear();
        assert!(deck_rows(&st).is_empty());
        assert!(pane_lines(&st).is_empty());
        let mut deck = crate::screens::deck::Deck::new(st.tick);
        deck.tab = crate::screens::deck::Tab::Jobs;
        let plain: Vec<String> = crate::jobs::ui::deck_rows(&st)
            .into_iter()
            .map(|r| r.text)
            .collect();
        let tab: Vec<String> = deck.rows(&st).into_iter().map(|r| r.text).collect();
        assert_eq!(tab, plain);
    }

    #[test]
    fn the_detail_ticks_earlier_objectives() {
        let mut st = open_drop();
        st.missions.runs[0]
            .objectives
            .insert(0, "Walk to the corner".into());
        let d = detail(&st, 0);
        assert_eq!(text(&d[0]), "The drop");
        assert_eq!(text(&d[1]), "from Dez · story · open");
        assert_eq!(text(&d[3]), "☑ Walk to the corner");
        assert_eq!(d[3].spans[0].style.fg, Some(DIM));
        assert!(text(&d[4]).starts_with("☐ Hand the package"));
        st.missions.runs[0].state = RunState::Done { at: 5 };
        let d = detail(&st, 0);
        assert_eq!(text(&d[1]), "from Dez · story · done");
        assert!(text(&d[4]).starts_with("☑ "));
    }
}
