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

use super::{Event, Kind, REFUSE_AT, Say, handin_item};
use crate::screens::popup::{Choice, Header, Pick, Popup};
use crate::ui::hud::AMBER;
use citygen::BuildingId;
use game::ItemId;

/// Rolls an offer for every fixer beside the player, so their window has it the
/// first time you talk: on the street the city's walkers, indoors whoever `inside`
/// places on the tile. Nothing for a fixer who refuses you.
pub fn prepare_offers(
    st: &mut AppState,
    inside: &dyn Fn(citygen::Place, citygen::Pos) -> Option<PersonId>,
) {
    let Some(player) = st.player.as_ref() else {
        return;
    };
    let (place, pos) = (player.place, player.pos);
    let Some(world) = st.world.as_ref() else {
        return;
    };
    let fixers: Vec<PersonId> = pos
        .neighbors4()
        .filter_map(|n| {
            if place.is_street() {
                world.person_at(n)
            } else {
                inside(place, n)
            }
        })
        .filter(|id| world.people()[id.0 as usize].profession == "fixer")
        .collect();
    for f in fixers {
        if !refused(st, f) {
            let _ = crate::jobs::offer(st, f);
        }
    }
}

fn refused(st: &AppState, fixer: PersonId) -> bool {
    st.jobs.standing(fixer) <= REFUSE_AT || st.backdoors.fixer_quiet(st.net.now(st))
}

fn stands(st: &AppState) -> impl Fn(BuildingId) -> bool + '_ {
    move |b| st.backdoors.standing().any(|d| d.building == b)
}

fn carries(st: &AppState) -> impl Fn(&ItemId) -> bool + '_ {
    move |i| st.character.count(i) > 0
}

/// A fixer's rows after Look: Talk, then the offer, the hand-in or the drop.
pub fn rows(st: &AppState, fixer: PersonId) -> Vec<Choice> {
    let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_ref()) else {
        return Vec::new();
    };
    let card = world.card(fixer, city);
    if refused(st, fixer) {
        let line = st.flavor.line(Say::Refused, fixer.0 as u64).to_string();
        let said = format!("{}: \u{201c}{line}\u{201d}", card.name);
        return vec![Choice::new(
            't',
            "Talk",
            Pick::Talk {
                lines: vec![plain(line)],
                said,
                who: Some(fixer),
            },
        )];
    }
    let lines: Vec<Line<'static>> = crate::render::people::card_lines(&card)
        .into_iter()
        .skip(1)
        .collect();
    let said = format!("{}: \u{201c}{}\u{201d}", card.name, card.line);
    let mut out = vec![Choice::new(
        't',
        "Talk",
        Pick::Talk {
            lines,
            said,
            who: Some(fixer),
        },
    )];
    if let Some(j) = st.jobs.taken(fixer) {
        match st.jobs.ready(j.id, &stands(st), &carries(st)) {
            Ok(()) => out.push(
                Choice::on('h', "Hand in", j.title.clone(), CYAN, Pick::HandIn(j.id))
                    .tail(format!(" · ¤{}", j.pay)),
            ),
            Err(why) => out.push(Choice {
                enabled: false,
                ..Choice::on('h', "Hand in", j.title.clone(), CYAN, Pick::Close)
                    .tail(format!(" · {why}"))
            }),
        }
        out.push(Choice::on(
            'd',
            "Drop",
            j.title.clone(),
            TEXT,
            Pick::DropAsk(j.id),
        ));
    } else if let Some(j) = st.jobs.offered(fixer) {
        out.push(
            Choice::on('w', "Work", j.title.clone(), CYAN, Pick::Offer(j.id))
                .tail(format!(" · ¤{} · {}h", j.pay, j.hours)),
        );
    } else {
        let line = st.flavor.line(Say::Nothing, fixer.0 as u64);
        out.push(Choice::disabled('w', "Work").tail(format!(" · {line}")));
    }
    out
}

/// The offer in place of the window: title, pay and hours, the brief, take or not.
pub fn offer_popup(st: &AppState, id: JobId, tick: u64) -> Popup {
    let Some(j) = st.jobs.get(id) else {
        return Popup::verbs("", vec![], tick);
    };
    let subtitle = format!(
        "pay ¤{} · {} hours · {}",
        j.pay,
        j.hours,
        j.kind.tag().label()
    );
    let mut body = vec![format!(
        "\u{201c}{}\u{201d}",
        st.flavor.line(Say::Offer, id.0 as u64)
    )];
    body.extend(j.brief.iter().cloned());
    Popup::about(
        Some(Header::title(&j.title)),
        Some(subtitle),
        vec![
            Choice::new('y', "Take the job", Pick::Take(id)),
            Choice::new('n', "Not now", Pick::Close),
        ],
        tick,
    )
    .with_body(body)
}

pub fn drop_confirm(id: JobId, title: &str, tick: u64) -> Popup {
    Popup::verbs(
        &format!("Drop {title}?"),
        vec![
            Choice::new('y', "Drop it", Pick::Drop(id)),
            Choice::new('n', "Keep it", Pick::Close),
        ],
        tick,
    )
}

/// Ends the job at its fixer: the item out of the pack, the pay in, the line logged.
pub fn hand_in(st: &mut AppState, id: JobId) -> Result<String, String> {
    let j = st.jobs.get(id).cloned().ok_or("no such job")?;
    st.jobs.ready(id, &stands(st), &carries(st))?;
    if let Some(item) = handin_item(&j.kind) {
        game::remove_from_pack(&mut st.character, &item, 1);
    }
    let pay = st.jobs.hand_in(id).ok_or("not in hand")?;
    st.character.credits = st.character.credits.saturating_add(pay);
    let fixer = fixer_name(st, j.fixer);
    let said = st.flavor.line(Say::Handed, id.0 as u64).to_string();
    let line = format!(
        "`{}` · handed in · ¤{pay} · {fixer}: \u{201c}{said}\u{201d}",
        j.title
    );
    st.story(line.clone());
    Ok(line)
}

/// Gives the job up; the package, if still carried, stays in the pack.
pub fn drop(st: &mut AppState, id: JobId) -> Result<String, String> {
    let j = st.jobs.get(id).cloned().ok_or("no such job")?;
    if !st.jobs.drop_job(id) {
        return Err("not in hand".into());
    }
    let fixer = fixer_name(st, j.fixer);
    let said = st.flavor.line(Say::Dropped, id.0 as u64).to_string();
    let line = format!("`{}` · dropped · {fixer}: \u{201c}{said}\u{201d}", j.title);
    st.warn(line.clone());
    Ok(line)
}

/// The hand-over row on the person a Taken Deliver names, while it is undelivered.
pub fn receiver_rows(st: &AppState, person: PersonId) -> Vec<Choice> {
    let Some(j) = st.jobs.list.iter().find(|j| {
        j.state == State::Taken
            && matches!(j.kind, Kind::Deliver { to, .. } if to == person)
            && !j.objectives[0].done
    }) else {
        return Vec::new();
    };
    let Kind::Deliver { item, .. } = &j.kind else {
        return Vec::new();
    };
    let name = st
        .content
        .item(item)
        .map(|i| i.name.clone())
        .unwrap_or_else(|| "Package".into());
    if st.character.count(item) > 0 {
        vec![Choice::on('h', "Hand over", name, AMBER, Pick::Hand(j.id))]
    } else {
        vec![Choice {
            enabled: false,
            ..Choice::on('h', "Hand over", name, AMBER, Pick::Close).tail(" · not on you")
        }]
    }
}

/// The package into its person's hands; the job ticks, the fixer still owes you.
pub fn hand_over(st: &mut AppState, id: JobId) -> Result<String, String> {
    let j = st.jobs.get(id).cloned().ok_or("no such job")?;
    let Kind::Deliver { to, item } = j.kind.clone() else {
        return Err("not a delivery".into());
    };
    if j.state != State::Taken || j.objectives[0].done {
        return Err("already delivered".into());
    }
    if st.character.count(&item) == 0 {
        return Err("the package is not on you".into());
    }
    game::remove_from_pack(&mut st.character, &item, 1);
    crate::jobs::send(st, Event::Handed { job: id });
    let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_ref()) else {
        return Err("no city".into());
    };
    let name = world.card(to, city).name;
    let said = st.flavor.line(Say::Receiver, id.0 as u64).to_string();
    let fixer = fixer_name(st, j.fixer);
    let line = format!("`{name}` takes the package · \u{201c}{said}\u{201d} · {fixer} owes you");
    st.story(line.clone());
    Ok(line)
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

    use crate::app::{Screen, ScreenCmd};
    use crate::jobs::testutil::{a_fixer, state_home};
    use crate::jobs::{Event, GAIN_DONE, KindTag, REFUSE_AT, first_fetch};
    use crate::screens::popup::Pick;
    use crate::screens::testutil::key;
    use citygen::Place;
    use crossterm::event::KeyCode;
    use game::ItemId;

    /// Puts the player on the street next to the person's spot and parks the person there.
    fn stand_beside(st: &mut AppState, who: PersonId) {
        let spot = st
            .world
            .as_ref()
            .unwrap()
            .spot_of(who)
            .expect("a spot worker");
        let city = st.city.clone().unwrap();
        let beside = spot
            .neighbors4()
            .find(|p| city.walkable(citygen::StratumKind::Street, *p))
            .unwrap();
        let mut p = st
            .player
            .take()
            .unwrap_or_else(|| crate::walker::Player::spawn(&city, 1));
        p.place = Place::Stratum(citygen::StratumKind::Street);
        p.pos = beside;
        st.player = Some(p);
        st.world.as_mut().unwrap().park(who, spot);
    }

    #[test]
    fn a_fixer_offers_work_and_the_welcome_hands_in_for_pay() {
        let mut st = state_home();
        let id = st.jobs.active().unwrap().id;
        let fx = st.jobs.get(id).unwrap().fixer;
        stand_beside(&mut st, fx);
        prepare_offers(&mut st, &|_, _| None);
        let rs = rows(&st, fx);
        let keys: Vec<char> = rs.iter().map(|c| c.key).collect();
        assert_eq!(
            keys,
            vec!['t', 'h', 'd'],
            "welcome in hand: no second offer"
        );
        assert!(
            !rs[1].enabled && rs[1].tail.contains("not yet yours"),
            "{:?}",
            rs[1].tail
        );
        st.character.give(ItemId::new("paydata"), 1);
        let (b, level, _, tile) = first_fetch(&st).unwrap();
        crate::jobs::send(
            &mut st,
            Event::Taken {
                place: Place::Floor { building: b, level },
                pos: tile,
            },
        );
        let rs = rows(&st, fx);
        assert!(rs[1].enabled);
        let credits = st.character.credits;
        let line = hand_in(&mut st, id).unwrap();
        assert!(
            line.contains("handed in") && line.contains("¤200"),
            "{line}"
        );
        assert_eq!(st.character.credits, credits + PAY_FETCH);
        assert_eq!(st.character.count(&ItemId::new("paydata")), 0);
        assert_eq!(st.jobs.standing(fx), GAIN_DONE);
        prepare_offers(&mut st, &|_, _| None);
        let rs = rows(&st, fx);
        assert_eq!(
            rs.iter().map(|c| c.key).collect::<Vec<_>>(),
            vec!['t', 'w'],
            "a new offer after the hand-in"
        );
        assert!(
            rs[1].tail.contains('¤') && rs[1].tail.contains('h'),
            "{}",
            rs[1].tail
        );
    }

    #[test]
    fn hand_in_refuses_without_the_shard() {
        let mut st = state_home();
        let id = st.jobs.active().unwrap().id;
        let (b, level, _, tile) = first_fetch(&st).unwrap();
        crate::jobs::send(
            &mut st,
            Event::Taken {
                place: Place::Floor { building: b, level },
                pos: tile,
            },
        );
        assert_eq!(
            hand_in(&mut st, id),
            Err("the shard is not on you".to_string())
        );
        assert_eq!(st.jobs.get(id).unwrap().state, State::Taken);
    }

    #[test]
    fn the_offer_window_takes_or_keeps_the_job() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        // Clear the welcome so this fixer can offer.
        let welcome = st.jobs.active().unwrap().id;
        st.jobs.drop_job(welcome);
        st.jobs.fixer_mut(fx).standing = 0;
        let id = crate::jobs::offer(&mut st, fx).unwrap();
        let mut p = offer_popup(&st, id, st.tick);
        let keys: Vec<char> = p.choices().iter().map(|c| c.key).collect();
        assert_eq!(keys, vec!['y', 'n']);
        let ScreenCmd::Pop { .. } = p.update(&mut st, &[key(KeyCode::Char('n'))]) else {
            panic!()
        };
        assert_eq!(st.jobs.get(id).unwrap().state, State::Offered);
        let mut p = offer_popup(&st, id, st.tick);
        p.update(&mut st, &[key(KeyCode::Char('y'))]);
        assert_eq!(st.jobs.get(id).unwrap().state, State::Taken);
        assert!(
            st.journal
                .mails
                .last()
                .unwrap()
                .body
                .iter()
                .any(|l| l.starts_with("By "))
        );
        assert!(st.log.latest().is_some_and(|e| e.text.contains("taken")));
    }

    #[test]
    fn hand_over_needs_the_package_in_the_pack() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        let job = crate::jobs::generate(&mut st, fx, KindTag::Deliver, 0).unwrap();
        let Kind::Deliver { to, .. } = job.kind.clone() else {
            panic!()
        };
        let id = st.jobs.push(job);
        crate::jobs::take_job(&mut st, id).unwrap();
        let rs = receiver_rows(&st, to);
        assert_eq!(rs.len(), 1);
        assert!(rs[0].enabled && rs[0].key == 'h');
        st.character.inventory.clear(); // dropped it somewhere
        let rs = receiver_rows(&st, to);
        assert!(!rs[0].enabled && rs[0].tail.contains("not on you"));
        st.character.give(ItemId::new("sealed_package"), 1);
        let line = hand_over(&mut st, id).unwrap();
        assert!(line.contains("takes the package"), "{line}");
        assert!(st.jobs.get(id).unwrap().objectives[0].done);
        assert_eq!(st.character.count(&ItemId::new("sealed_package")), 0);
        assert!(hand_over(&mut st, id).is_err(), "nothing doubles");
        assert!(receiver_rows(&st, to).is_empty());
    }

    #[test]
    fn a_refused_runner_gets_the_line_and_no_rows() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        st.jobs.fixer_mut(fx).standing = REFUSE_AT;
        prepare_offers(&mut st, &|_, _| None);
        let rs = rows(&st, fx);
        assert_eq!(rs.len(), 1);
        let Pick::Talk { said, .. } = &rs[0].pick else {
            panic!()
        };
        assert!(
            said.contains("Lose this number") || said.contains("I don't know you"),
            "{said}"
        );
        assert!(st.jobs.offered(fx).is_none());
    }

    #[test]
    fn a_fixer_met_indoors_offers_too() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        let welcome = st.jobs.active().unwrap().id;
        st.jobs.drop_job(welcome);
        st.jobs.fixer_mut(fx).standing = 0;
        let home = st.home.unwrap().home;
        let city = st.city.clone().unwrap();
        let mut p = crate::walker::Player::spawn(&city, 1);
        p.place = Place::Floor {
            building: home.building,
            level: home.level,
        };
        p.pos = home.bed_side;
        st.player = Some(p);
        prepare_offers(&mut st, &|_, _| Some(fx));
        assert!(
            st.jobs.offered(fx).is_some(),
            "the fixer at the bar has work too"
        );
    }
}
