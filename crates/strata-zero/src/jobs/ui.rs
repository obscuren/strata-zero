//! What the player sees of the jobs: the QUEST pane, the deck's tab, the names.

use super::{Job, JobId, Jobs, State, WARN_MINUTES, clock_text};
use crate::app::AppState;
use crate::screens::deck::{Detail, Row, Tone};
use crate::ui::chrome::{CYAN, DIM, MAGENTA, TEXT};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use sim::PersonId;

/// The fixer's first name, as their mails sign.
pub fn fixer_name(st: &AppState, id: PersonId) -> String {
    st.world
        .as_ref()
        .and_then(|w| w.people().get(id.0 as usize))
        .map(|p| {
            p.name
                .split_whitespace()
                .next()
                .unwrap_or(&p.name)
                .to_string()
        })
        .unwrap_or_else(|| "the fixer".to_string())
}

/// The district a fixer holds a corner on.
pub fn district_of_fixer(st: &AppState, id: PersonId) -> String {
    let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_ref()) else {
        return "the street".to_string();
    };
    world
        .spot_of(id)
        .and_then(|pos| city.districts.iter().find(|d| d.bounds.contains(pos)))
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "the street".to_string())
}

fn plain(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(TEXT)))
}

fn dim(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(DIM)))
}

/// The first job in hand: title, fixer and clock; its first open objective; pips.
pub fn pane_lines(st: &AppState) -> Vec<Line<'static>> {
    let Some(job) = st.jobs.active() else {
        return vec![dim(" no job on · find a fixer")];
    };
    let now = st.net.now(st);
    let left = Jobs::left(job, now).unwrap_or(0);
    let clock_style = if left < WARN_MINUTES {
        Style::default().fg(MAGENTA)
    } else {
        Style::default().fg(DIM)
    };
    let done = job.objectives.iter().filter(|o| o.done).count();
    let total = job.objectives.len();
    let next = job
        .objectives
        .iter()
        .find(|o| !o.done)
        .map(|o| o.text.clone())
        .unwrap_or_default();
    vec![
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                job.title.to_uppercase(),
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" · {}", fixer_name(st, job.fixer)),
                Style::default().fg(TEXT),
            ),
            Span::styled(format!(" · {}", clock_text(left)), clock_style),
        ]),
        plain(format!(" » {next}")),
        Line::from(vec![
            Span::raw(" "),
            Span::styled("▰".repeat(done), Style::default().fg(CYAN)),
            Span::styled("▱".repeat(total - done), Style::default().fg(DIM)),
            Span::styled(format!(" {done}/{total}"), Style::default().fg(DIM)),
        ]),
    ]
}

fn mark(state: State) -> &'static str {
    match state {
        State::Done => "✓",
        State::Failed => "✗",
        State::Dropped => "–",
        State::Taken | State::Offered => " ",
    }
}

/// Taken first, then offered (dim), then the finished ones.
pub fn deck_rows(st: &AppState) -> Vec<Row> {
    let order = |s: State| match s {
        State::Taken => 0,
        State::Offered => 1,
        _ => 2,
    };
    let mut jobs: Vec<&Job> = st.jobs.list.iter().collect();
    jobs.sort_by_key(|j| (order(j.state), j.id));
    jobs.into_iter()
        .map(|j| Row {
            text: format!("{} {}", mark(j.state), j.title),
            tone: match j.state {
                State::Taken => Tone::Normal,
                State::Offered | State::Dropped => Tone::Dim,
                State::Done => Tone::Done,
                State::Failed => Tone::Warn,
            },
            detail: Some(Detail::Job(j.id)),
            spans: None,
        })
        .collect()
}

/// The deck's dossier on one job.
pub fn detail(st: &AppState, id: JobId) -> Vec<Line<'static>> {
    let Some(j) = st.jobs.get(id) else {
        return Vec::new();
    };
    let now = st.net.now(st);
    let outcome = match j.state {
        State::Offered => "on offer".to_string(),
        State::Taken => format!("{} left", clock_text(Jobs::left(j, now).unwrap_or(0))),
        State::Done => "done".to_string(),
        State::Failed => "failed".to_string(),
        State::Dropped => "dropped".to_string(),
    };
    let mut out = vec![Line::from(Span::styled(
        j.title.clone(),
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    ))];
    out.push(dim(format!(
        "from {} · {} · ¤{} · {outcome}",
        fixer_name(st, j.fixer),
        district_of_fixer(st, j.fixer),
        j.pay
    )));
    out.push(plain(""));
    out.extend(j.brief.iter().map(|l| plain(l.clone())));
    out.push(plain(""));
    for o in &j.objectives {
        out.push(if o.done {
            Line::from(Span::styled(
                format!("☑ {}", o.text),
                Style::default().fg(DIM),
            ))
        } else {
            plain(format!("☐ {}", o.text))
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{HOURS_FETCH, Kind, Objective, PAY_FETCH};
    use citygen::{BuildingId, Pos};

    fn text(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect()
    }

    #[test]
    fn the_pane_reads_the_job_in_hand_and_its_clock() {
        let mut st = AppState::new();
        assert_eq!(text(&pane_lines(&st)), vec![" no job on · find a fixer"]);
        let id = st.jobs.push(Job {
            id: JobId(0),
            fixer: PersonId(0),
            kind: Kind::Fetch {
                building: BuildingId(0),
                level: 1,
                number: 101,
                tile: Pos::new(1, 1),
            },
            title: "The drop".into(),
            brief: vec![],
            pay: PAY_FETCH,
            hours: HOURS_FETCH,
            objectives: vec![
                Objective {
                    text: "Take the shard".into(),
                    done: false,
                },
                Objective {
                    text: "Bring it".into(),
                    done: false,
                },
            ],
            offered: 0,
            taken: None,
            deadline: None,
            state: State::Offered,
        });
        assert_eq!(
            text(&pane_lines(&st)),
            vec![" no job on · find a fixer"],
            "an offer is not in hand"
        );
        st.jobs.take(id, 0);
        let lines = text(&pane_lines(&st));
        assert!(
            lines[0].starts_with(" THE DROP · the fixer · 12h 0m"),
            "{lines:?}"
        );
        assert_eq!(lines[1], " » Take the shard");
        assert_eq!(lines[2], " ▱▱ 0/2");
        let rows = deck_rows(&st);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "  The drop");
        let d = text(&detail(&st, id));
        assert!(
            d[1].contains("¤200") && d[1].contains("12h 0m left"),
            "{d:?}"
        );
        assert!(d.iter().any(|l| l == "☐ Take the shard"), "{d:?}");
    }
}
