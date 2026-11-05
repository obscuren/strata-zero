//! What the player sees of the story: its lines in the JOBS pane, its rows and dossier
//! in the deck's JOBS tab.

use super::{RunState, role_name};
use crate::app::AppState;
use crate::jobs::card::{self, Item, Way};
use crate::ui::chrome::{DIM, FAINT, MAGENTA, TEXT};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

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
    usize::from(crate::track::ui::pane_line(st).is_some()) + pane_lines(st).len()
}

/// The whole JOBS pane: the track, the story's lines, then the job in hand.
pub fn jobs_pane(st: &AppState) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = crate::track::ui::pane_line(st).into_iter().collect();
    out.extend(pane_lines(st));
    out.extend(crate::jobs::ui::pane_lines(st));
    out
}

/// The missions the JOBS tab lists: the open ones, then the done ones, each with
/// whether it is done.
pub fn story_missions(st: &AppState) -> Vec<(usize, bool)> {
    let mut open = Vec::new();
    let mut done = Vec::new();
    for (i, run) in st.missions.runs.iter().enumerate() {
        if st.book.missions.get(i).is_none() {
            continue;
        }
        match run.state {
            RunState::Open => open.push((i, false)),
            RunState::Done { .. } => done.push((i, true)),
            _ => {}
        }
    }
    open.extend(done);
    open
}

/// The dev console's `story`: each role with the person cast in it, each mission with
/// its state and the step it waits on, then the book's scenes.
pub fn console_lines(st: &AppState) -> Vec<String> {
    let mut out = story_lines(st);
    for (id, scene) in &st.book.scenes {
        let look = match scene.look {
            super::Look::Film => "film",
            super::Look::Bubbles => "bubbles",
        };
        let n = scene.beats.len();
        let beats = if n == 1 { "beat" } else { "beats" };
        let then = scene
            .next
            .as_ref()
            .map_or_else(String::new, |next| format!(" · then {next}"));
        out.push(format!("scene {id} · {look} · {n} {beats}{then}"));
    }
    out
}

/// The cast and the missions, for `console_lines`.
fn story_lines(st: &AppState) -> Vec<String> {
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

/// The story's card (spec 9): the mission as a route, its earlier objectives ticked,
/// you and the walk, then the step it waits on in its own words; no window. A done one
/// is ticked through and has nothing to track.
pub fn story_card(st: &AppState, mission: usize, width: usize, way: &Way) -> Vec<Line<'static>> {
    let (Some(m), Some(run)) = (st.book.missions.get(mission), st.missions.runs.get(mission))
    else {
        return Vec::new();
    };
    if !shown(run.state) {
        return Vec::new();
    }
    let done = matches!(run.state, RunState::Done { .. });
    let mut out = vec![
        card::title_line("STORY", MAGENTA, &m.title, "", width),
        card::row(
            vec![Span::styled(
                format!(
                    "from {} · {}",
                    from_name(st, mission),
                    if done { "done" } else { "open" }
                ),
                Style::default().fg(DIM),
            )],
            Vec::new(),
            width,
        ),
        Line::from(""),
    ];
    let ticked = if done {
        run.objectives.len()
    } else {
        run.objectives.len().saturating_sub(1)
    };
    let stop = |glyph, glyph_col, text: String, text_col, under| card::Stop {
        glyph,
        glyph_col,
        text,
        text_col,
        verb: String::new(),
        verb_style: Style::default(),
        under,
    };
    let mut items: Vec<Item> = run.objectives[..ticked]
        .iter()
        .map(|o| Item::Stop(stop("✓", card::DONE, o.clone(), FAINT, Vec::new())))
        .collect();
    let mut tracks = None;
    if !done && let Some(now) = run.objectives.last() {
        if *way != Way::Nowhere {
            items.push(Item::You {
                walk: card::walk_of(way),
            });
            tracks = crate::track::ui::story_source(st, mission);
        }
        let mut lines = card::wrap(now, width.saturating_sub(5), "").into_iter();
        let first = lines.next().unwrap_or_default();
        let under = lines.map(|l| (l, TEXT)).collect();
        items.push(Item::Stop(stop("◆", MAGENTA, first, TEXT, under)));
    }
    out.extend(card::route_lines(st, items, width));
    if let Some(src) = tracks {
        out.push(Line::from(""));
        out.push(card::track_line(
            st,
            &src,
            &card::target_place(st, &src),
            width,
        ));
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
        assert_eq!(text(&story[0]), " THE DROP · Mezz · story");
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
    fn the_tab_lists_open_missions_then_done_ones() {
        let mut st = open_drop();
        assert_eq!(story_missions(&st), vec![(0, false)]);
        st.missions.runs[0].state = RunState::Done { at: 5 };
        assert_eq!(story_missions(&st), vec![(0, true)]);
        st.missions.runs.clear();
        assert!(story_missions(&st).is_empty());
        assert!(pane_lines(&st).is_empty());
    }

    fn card_text(st: &AppState, way: &Way) -> Vec<String> {
        story_card(st, 0, 68, way).iter().map(text).collect()
    }

    #[test]
    fn the_story_card_is_a_route_without_a_clock() {
        let mut st = open_drop();
        let walk = Way::Walk { m: 30, next: None };
        let c = card_text(&st, &walk);
        assert!(c[0].starts_with("STORY ━ THE DROP ━"), "{c:#?}");
        assert_eq!(c[1], "from Mezz · open");
        assert!(c.iter().any(|l| l.starts_with("  @  you")), "{c:#?}");
        assert!(c.iter().any(|l| l == "  ┃  30 m"), "{c:#?}");
        assert!(
            c.iter().any(|l| l.starts_with("  ◆  Hand the package to ")),
            "{c:#?}"
        );
        assert!(
            !c.iter()
                .any(|l| l.contains("clock") || l.contains("window") || l.contains("waits")),
            "{c:#?}"
        );
        assert!(c.last().unwrap().contains("TRACK"), "{c:#?}");
        // Earlier objectives are ticked stops above you.
        st.missions.runs[0]
            .objectives
            .insert(0, "Walk to the corner".into());
        let c = card_text(&st, &walk);
        let tick = c.iter().position(|l| l.contains("✓  Walk to the corner"));
        let you = c.iter().position(|l| l.starts_with("  @  you"));
        assert!(tick.is_some() && tick < you, "{c:#?}");
        // A step with nowhere to walk to: its line alone, nothing to track.
        let c = card_text(&st, &Way::Nowhere);
        assert!(
            !c.iter().any(|l| l.contains('@') || l.contains("TRACK")),
            "{c:#?}"
        );
    }

    #[test]
    fn a_done_story_is_ticked_through() {
        let mut st = open_drop();
        st.missions.runs[0].state = RunState::Done { at: 5 };
        let c = card_text(&st, &Way::Nowhere);
        assert_eq!(c[1], "from Mezz · done");
        assert!(
            c.iter().any(|l| l.contains("✓  Hand the package")),
            "{c:#?}"
        );
        assert!(!c.iter().any(|l| l.contains("TRACK") || l.contains('@')));
    }
}
