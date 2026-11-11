//! The deck's JOBS cards (spec 3–8): a job in hand as a route from you to its fixer's
//! corner, with its window and the track line; the offer, the fixer and the closed job.
//! Each is lines for a width, none wider; the tab backs and types them.

use super::ui::{district_of_fixer, fixer_name, hand_in_now};
use super::{
    COPY_AT, Corner, GAIN_DONE, Job, JobId, Jobs, Kind, LOSS_DROP, LOSS_LATE, PLANT_AT, REFUSE_AT,
    Say, State, VAULT_AT, clock_text, corner, day_minute, hours_text, in_shift, opens_at, shift_of,
    shift_turn, standing_word,
};
use crate::app::AppState;
use crate::backdoors::{Health, State as DoorState};
use crate::credentials::How;
use crate::track::route::NoLeg;
use crate::track::{self, ListingId, Resolved, Source};
use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINT, FAINTER, MAGENTA, PANEL_BG, RULE, TEXT};
use crate::ui::hud::{AMBER, DIM_AMBER};
use citygen::{BuildingId, FloorPlan, Place};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use sim::{PersonId, Work};

/// The deck's done tone.
pub const DONE: Color = Color::Rgb(90, 200, 140);
pub const WHITE: Color = Color::Rgb(255, 255, 255);

/// The walk to a stop, as the tab caches it.
#[derive(Clone, Debug, PartialEq)]
pub enum Way {
    Walk {
        m: usize,
        next: Option<String>,
    },
    /// No walk gets there from here.
    Unreachable,
    JackedIn,
    /// Nothing to walk to: done with, or a step with no place.
    Nowhere,
}

/// The walk from where you stand to what `source` points at now, as the track would
/// make it.
pub fn way(st: &AppState, source: &Source) -> Way {
    let Resolved::Target(target) = track::resolve(st, source) else {
        return Way::Nowhere;
    };
    let Resolved::Target(located) = track::locate(st, target) else {
        return Way::Nowhere;
    };
    match track::route::leg(st, located) {
        Ok(leg) => Way::Walk {
            m: leg.path.len(),
            next: track::route::next_text(st, &leg, located),
        },
        Err(NoLeg::NoWay) => Way::Unreachable,
        Err(NoLeg::JackedIn) => Way::JackedIn,
    }
}

/// `51 m · building, then 1F`, or why there is no distance; nothing for nowhere.
pub fn way_text(way: &Way) -> String {
    match way {
        Way::Walk { m, next: Some(n) } => format!("{m} m · {n}"),
        Way::Walk { m, next: None } => format!("{m} m"),
        Way::Unreachable => "no way from here".to_string(),
        Way::JackedIn => "jacked in".to_string(),
        Way::Nowhere => String::new(),
    }
}

/// Tracking a fixer: the MAP's fixer listing.
pub fn fixer_source(fixer: PersonId) -> Source {
    Source::Listing(ListingId::Person(fixer))
}

fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

fn bold(c: Color) -> Style {
    fg(c).add_modifier(Modifier::BOLD)
}

fn span(text: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(text.into(), style)
}

fn cells(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
}

fn hm(m: u16) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// `left`, then `right` against the width's end; the left is cut with `…` to leave
/// `right` and a cell before it.
pub(crate) fn row(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: usize,
) -> Line<'static> {
    let rw = cells(&right);
    let room = if rw > 0 {
        width.saturating_sub(rw + 1)
    } else {
        width
    };
    let mut out = Vec::new();
    let mut used = 0;
    for s in left {
        let n = s.content.chars().count();
        if used + n <= room {
            used += n;
            out.push(s);
            continue;
        }
        let kept = crate::screens::deck::cut(&s.content, room - used);
        used += kept.chars().count();
        out.push(Span::styled(kept, s.style));
        break;
    }
    if rw > 0 && used + rw <= width {
        out.push(Span::raw(" ".repeat(width - used - rw)));
        out.extend(right);
    }
    Line::from(out)
}

fn plain(text: impl Into<String>, c: Color, width: usize) -> Line<'static> {
    row(vec![span(text, fg(c))], Vec::new(), width)
}

fn blank() -> Line<'static> {
    Line::from("")
}

/// `first`, then `tail` when the two fit the width, else `first` alone.
fn with_tail(first: Span<'static>, tail: Span<'static>, width: usize) -> Line<'static> {
    if cells(&[first.clone(), tail.clone()]) <= width {
        Line::from(vec![first, tail])
    } else {
        row(vec![first], Vec::new(), width)
    }
}

/// `FETCH ━ THE APARTMENT SHARD ━━━━━━ ¤200`: the tag, the title cut to fit, a rule to
/// the right part.
pub(crate) fn title_line(
    tag: &str,
    tag_col: Color,
    title: &str,
    right: &str,
    width: usize,
) -> Line<'static> {
    let fixed = tag.chars().count() + 3 + 1 + right.chars().count();
    let t = crate::screens::deck::cut(&title.to_uppercase(), width.saturating_sub(fixed + 1));
    let fill = width.saturating_sub(fixed + t.chars().count());
    Line::from(vec![
        span(tag, bold(tag_col)),
        span(" ━ ", fg(RULE)),
        span(t, bold(CYAN)),
        span(" ", fg(RULE)),
        span("━".repeat(fill), fg(RULE)),
        span(right, fg(TEXT)),
    ])
}

/// Words wrapped to `width`, each line after the first led by `indent`; a word longer
/// than a line is cut.
pub(crate) fn wrap(text: &str, width: usize, indent: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let lead = if out.is_empty() { "" } else { indent };
        let room = width.saturating_sub(lead.chars().count()).max(1);
        let len = line.chars().count();
        if len > 0 && len + 1 + word.chars().count() > room {
            out.push(format!("{lead}{line}"));
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    let lead = if out.is_empty() { "" } else { indent };
    out.push(format!("{lead}{line}"));
    out.into_iter()
        .map(|l| crate::screens::deck::cut(&l, width))
        .collect()
}

/// The fixer's words, quoted, each line after the first one cell in.
pub(crate) fn brief_lines(brief: &[String], width: usize) -> Vec<Line<'static>> {
    let n = brief.len();
    let mut out = Vec::new();
    for (i, l) in brief.iter().enumerate() {
        let q = format!(
            "{}{l}{}",
            if i == 0 { "\u{201c}" } else { " " },
            if i + 1 == n { "\u{201d}" } else { "" }
        );
        out.extend(
            wrap(&q, width, " ")
                .into_iter()
                .map(|l| plain(l, DIM, width)),
        );
    }
    out
}

/// One stop on a route: its glyph, its text, its verb at the right, and the lines
/// under it.
pub(crate) struct Stop {
    pub glyph: &'static str,
    pub glyph_col: Color,
    pub text: String,
    pub text_col: Color,
    pub verb: String,
    pub verb_style: Style,
    pub under: Vec<(String, Color)>,
}

/// A route's parts: a stop, or you and the walk to the stop after.
pub(crate) enum Item {
    Stop(Stop),
    You { walk: Option<(String, Color)> },
}

/// `you · Stack 101 · Street`: where you stand, as the deck's link line says it.
fn you_text(st: &AppState) -> String {
    let Some(p) = st.player.as_ref() else {
        return "you".to_string();
    };
    match p.place {
        Place::Stratum(k) => match track::district_at(st, p.place, p.pos) {
            Some(d) => format!("you · {d} · {}", k.label()),
            None => format!("you · {}", k.label()),
        },
        Place::Floor { building, level } => format!(
            "you · {} · {}",
            crate::net::building_name(st, building),
            FloorPlan::label(level)
        ),
        Place::Construct(_) | Place::SiteNet { .. } => "you · jacked in".to_string(),
    }
}

/// The walk line under you: cyan with a distance, dim with a reason, none for nowhere.
pub(crate) fn walk_of(way: &Way) -> Option<(String, Color)> {
    match way {
        Way::Walk { .. } => Some((way_text(way), CYAN)),
        Way::Unreachable | Way::JackedIn => Some((way_text(way), DIM)),
        Way::Nowhere => None,
    }
}

/// The route: each stop, you and the walk before the one you are on, a `┊` between
/// stops and down the side of the lines under a stop with more after it.
pub(crate) fn route_lines(st: &AppState, items: Vec<Item>, width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let n = items.len();
    let pad = || span("  ", fg(TEXT));
    for (i, item) in items.iter().enumerate() {
        let more = i + 1 < n;
        match item {
            Item::You { walk } => {
                out.push(row(
                    vec![
                        pad(),
                        span("@", bold(WHITE)),
                        pad(),
                        span(you_text(st), fg(TEXT)),
                    ],
                    Vec::new(),
                    width,
                ));
                if let Some((text, c)) = walk {
                    out.push(row(
                        vec![
                            pad(),
                            span("┃", bold(CYAN)),
                            pad(),
                            span(text.clone(), fg(*c)),
                        ],
                        Vec::new(),
                        width,
                    ));
                }
            }
            Item::Stop(stop) => {
                out.push(row(
                    vec![
                        pad(),
                        span(stop.glyph, bold(stop.glyph_col)),
                        pad(),
                        span(stop.text.clone(), fg(stop.text_col)),
                    ],
                    vec![span(stop.verb.clone(), stop.verb_style)],
                    width,
                ));
                let side = if more { "┊" } else { " " };
                for (text, c) in &stop.under {
                    out.push(row(
                        vec![
                            pad(),
                            span(side, fg(FAINT)),
                            pad(),
                            span(text.clone(), fg(*c)),
                        ],
                        Vec::new(),
                        width,
                    ));
                }
                if more {
                    out.push(row(vec![pad(), span("┊", fg(FAINT))], Vec::new(), width));
                }
            }
        }
    }
    out
}

/// Whether the fixer is on their corner now, with their shift.
fn on_corner(st: &AppState, fixer: PersonId) -> Option<(bool, (u16, u16))> {
    let shift = shift_of(st, fixer)?;
    Some((in_shift(day_minute(st.net.now(st)), shift), shift))
}

/// `on the corner till 04:00`, or `off the corner · back at 18:00` in amber.
fn corner_line(st: &AppState, fixer: PersonId) -> Option<(String, Color)> {
    Some(match on_corner(st, fixer)? {
        (true, (_, to)) => (format!("on the corner till {}", hm(to)), DIM),
        (false, (from, _)) => (format!("off the corner · back at {}", hm(from)), AMBER),
    })
}

/// Where the track line says it lights the way to for a fixer: their corner while they
/// hold it, else them.
pub fn corner_place(st: &AppState, fixer: PersonId) -> String {
    let name = fixer_name(st, fixer);
    match on_corner(st, fixer) {
        Some((true, _)) => format!("{name}'s corner"),
        _ => name,
    }
}

/// Where the track line lights the way to for a job: its stop not yet done.
pub fn stop_place(st: &AppState, job: &Job) -> String {
    let at_target = job.state == State::Taken && job.objectives.first().is_some_and(|o| !o.done);
    if !at_target {
        return corner_place(st, job.fixer);
    }
    match &job.kind {
        Kind::Fetch { number, .. } => format!("flat {number}"),
        Kind::Copy { building, .. } | Kind::Plant { building } => {
            crate::helix::hosts::building_hostname(st, *building)
        }
        Kind::Deliver { to, .. } => person_called(st, *to),
    }
}

/// What a source points at, as the track line names it: a person, a building, your
/// home, or the district of a spot.
pub fn target_place(st: &AppState, source: &Source) -> String {
    match track::resolve(st, source) {
        Resolved::Target(track::Target::Person(p)) => person_called(st, p),
        Resolved::Target(track::Target::Building(b)) => crate::net::building_name(st, b),
        Resolved::Target(track::Target::Spot { place, pos, .. }) => {
            let home = st.home.map(|r| r.home.building);
            match place {
                Place::Floor { building, .. } if Some(building) == home => "home".to_string(),
                _ => track::district_at(st, place, pos).unwrap_or_else(|| "the spot".into()),
            }
        }
        _ => "there".to_string(),
    }
}

/// A person as the game calls them: by the story's name for the role they are cast
/// in, else their own.
fn person_called(st: &AppState, p: PersonId) -> String {
    st.missions
        .cast
        .iter()
        .find(|(_, who)| **who == p)
        .map(|(role, _)| crate::missions::role_name(st, role))
        .unwrap_or_else(|| track::person_name(st, p))
}

/// A delivery's person and their workplace, then their trade and hours.
fn deliver_place(st: &AppState, to: PersonId) -> (String, String) {
    let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_ref()) else {
        return ("someone".to_string(), String::new());
    };
    let card = world.card(to, city);
    let place = match world.people().get(to.0 as usize).map(|p| p.work) {
        Some(Work::Building(b)) => crate::net::building_name(st, b),
        _ => "the corner".to_string(),
    };
    let hours = world.work_hours(to).map(hours_text).unwrap_or_default();
    (
        format!("{} · {place}", card.name),
        format!("{} · {hours}", card.profession),
    )
}

/// The door of a fetch's flat: whether it opens for you, and how.
fn door_line(st: &AppState, b: BuildingId, level: i32, number: u16) -> (String, Color) {
    match crate::credentials::opens(st, b, level, number) {
        Some(How::Code(c)) => (format!("door code {c:04}"), DONE),
        Some(How::Keycard) => ("you hold its keycard".to_string(), DONE),
        Some(How::Cracked) => ("the lock is cracked".to_string(), DONE),
        Some(How::Free) => ("it opens without a key".to_string(), DONE),
        None => (
            format!("door code not known · {}", super::generate::how_in(st, b)),
            MAGENTA,
        ),
    }
}

/// The newest backdoor on `b`, burned ones too: its health word and its colour.
fn backdoor_health(st: &AppState, b: BuildingId) -> Option<(String, Color)> {
    let d = st.backdoors.list.iter().rev().find(|d| d.building == b)?;
    Some(match (d.state, d.health(st.net.now(st))) {
        (DoorState::Burned { why, .. }, _) => (format!("BURNED · {}", why.label()), MAGENTA),
        (_, Health::Stable) => (Health::Stable.label().to_string(), DONE),
        (_, Health::Degrading) => (Health::Degrading.label().to_string(), AMBER),
        (_, h) => (h.label().to_string(), MAGENTA),
    })
}

/// The job's own stop, its objective 0: done, to do, or (on an offer) faint.
fn target_stop(st: &AppState, job: &Job, done: bool, faint: bool) -> Stop {
    let name = |b: BuildingId| crate::net::building_name(st, b);
    let host = |b: BuildingId| crate::helix::hosts::building_hostname(st, b);
    let security = |b: BuildingId| crate::access::security_of(st, b);
    let (text, todo, did, under) = match &job.kind {
        Kind::Fetch {
            building,
            level,
            number,
            ..
        } => (
            format!(
                "flat {number} · {} · {}",
                FloorPlan::label(*level),
                name(*building)
            ),
            "take the shard",
            "shard taken",
            vec![door_line(st, *building, *level, *number)],
        ),
        Kind::Copy { building, vault } => (
            format!("{} · security {}", host(*building), security(*building)),
            if *vault {
                "copy its vault"
            } else {
                "copy its server"
            },
            if *vault {
                "vault copied"
            } else {
                "server copied"
            },
            vec![("in the Net".to_string(), DIM)],
        ),
        Kind::Plant { building } => (
            format!("{} · security {}", host(*building), security(*building)),
            "plant a backdoor",
            "backdoor planted",
            if done {
                backdoor_health(st, *building)
                    .map(|(h, c)| (format!("backdoor {h}"), c))
                    .into_iter()
                    .collect()
            } else {
                vec![("hand it in before the backdoor burns".to_string(), DIM)]
            },
        ),
        Kind::Deliver { to, .. } => {
            let (text, under) = deliver_place(st, *to);
            (
                text,
                "deliver the package",
                "package delivered",
                vec![(under, DIM)],
            )
        }
    };
    let keep_under = match job.kind {
        Kind::Plant { .. } => true,
        _ => !done,
    };
    let under = if keep_under { under } else { Vec::new() };
    let under = if faint {
        under.into_iter().map(|(t, _)| (t, FAINT)).collect()
    } else {
        under
    };
    if done {
        Stop {
            glyph: "✓",
            glyph_col: DONE,
            text,
            text_col: FAINT,
            verb: did.to_string(),
            verb_style: fg(FAINT),
            under,
        }
    } else {
        Stop {
            glyph: if faint { "◇" } else { "◆" },
            glyph_col: if faint { FAINT } else { MAGENTA },
            text,
            text_col: if faint { FAINT } else { TEXT },
            verb: todo.to_string(),
            verb_style: fg(if faint { FAINT } else { TEXT }),
            under,
        }
    }
}

/// `$ Mezz · Kowloon Row corner`: the fixer's corner, where every job ends.
fn corner_text(st: &AppState, fixer: PersonId) -> String {
    format!(
        "{} · {} corner",
        fixer_name(st, fixer),
        district_of_fixer(st, fixer)
    )
}

/// The hand-in stop: handed in, READY, to do with the corner and any reason under it,
/// or (on an offer or a closed job) faint.
fn handin_stop(st: &AppState, job: &Job, done: bool, faint: bool) -> Stop {
    let text = corner_text(st, job.fixer);
    if done {
        return Stop {
            glyph: "✓",
            glyph_col: DONE,
            text,
            text_col: FAINT,
            verb: "handed in".to_string(),
            verb_style: fg(FAINT),
            under: Vec::new(),
        };
    }
    if faint {
        return Stop {
            glyph: "$",
            glyph_col: FAINT,
            text,
            text_col: FAINT,
            verb: "hand it in".to_string(),
            verb_style: fg(FAINT),
            under: Vec::new(),
        };
    }
    let now = hand_in_now(st, job.id);
    let mut under: Vec<(String, Color)> = corner_line(st, job.fixer).into_iter().collect();
    let target_done = job.objectives.first().is_some_and(|o| o.done);
    if let Err(why) = &now
        && target_done
    {
        under.push((why.clone(), MAGENTA));
    }
    let (verb, verb_style) = if now.is_ok() {
        ("READY".to_string(), bold(CYAN))
    } else {
        ("hand it in".to_string(), fg(TEXT))
    };
    Stop {
        glyph: "$",
        glyph_col: AMBER,
        text,
        text_col: TEXT,
        verb,
        verb_style,
        under,
    }
}

/// The card's last line: `t` as a key and what it does, or, tracked, a lit band with
/// the distance and the key that stops it.
pub fn track_line(st: &AppState, source: &Source, place: &str, width: usize) -> Line<'static> {
    if !track::is_tracked(st, source) {
        let key = span(" t ", bold(PANEL_BG).bg(CYAN));
        let what = span("  TRACK", bold(CYAN));
        let long = format!(" · the floor lights the way to {place}");
        let short = format!(" · lights the way to {place}");
        let tail = if 3 + 7 + long.chars().count() <= width {
            long
        } else {
            short
        };
        return row(vec![key, what, span(tail, fg(DIM))], Vec::new(), width);
    }
    let band = |c: Color| fg(c).bg(BAR_BG);
    let head = " ⌖ TRACKING";
    let (key, stop) = (" t ", " stop");
    let (long, short) = match track::leg_now(st) {
        Some(Err(NoLeg::NoWay)) => (format!("  {place} · no way from here"), None),
        Some(Err(NoLeg::JackedIn)) => (format!("  {place} · jacked in"), None),
        _ => {
            let m = track::distance(st).unwrap_or(0);
            (
                format!("  {place} · {m} m · the floor lights the way"),
                Some(format!("  {place} · {m} m")),
            )
        }
    };
    let room = width.saturating_sub(head.chars().count() + key.len() + stop.len() + 1);
    let tail = match short {
        Some(s) if long.chars().count() > room => s,
        _ => long,
    };
    let tail = crate::screens::deck::cut(&tail, room);
    let pad =
        width.saturating_sub(head.chars().count() + tail.chars().count() + key.len() + stop.len());
    Line::from(vec![
        span(head, band(CYAN).add_modifier(Modifier::BOLD)),
        span(tail, band(TEXT)),
        span(" ".repeat(pad), band(TEXT)),
        span(key, bold(PANEL_BG).bg(CYAN)),
        span(stop, band(DIM)),
    ])
}

/// The job's window from taken to due, now on it, the fixer's time off the corner in
/// amber dashes, and where they will be at the end.
pub fn window_lines(st: &AppState, job: &Job, width: usize) -> Vec<Line<'static>> {
    let (Some(taken), Some(due)) = (job.taken, job.deadline) else {
        return Vec::new();
    };
    let real_now = st.net.now(st);
    let now = real_now.clamp(taken, due);
    let n = width.saturating_sub(8).max(1);
    let total = due.saturating_sub(taken).max(1);
    let cell = |m: u64| ((m.saturating_sub(taken) * n as u64) / total).min(n as u64 - 1) as usize;
    let middle = |i: usize| taken + ((2 * i as u64 + 1) * total) / (2 * n as u64);
    let shift = shift_of(st, job.fixer);
    let spent = cell(now);
    let mut bar = vec![span("window  ", fg(FAINT))];
    let mut run = String::new();
    let mut run_style = fg(FAINTER);
    for i in 0..n {
        let (ch, style) = if i < spent {
            ("━", fg(FAINTER))
        } else if i == spent {
            ("╋", bold(WHITE))
        } else if shift.is_some_and(|s| !in_shift(day_minute(middle(i)), s)) {
            ("┄", fg(DIM_AMBER))
        } else {
            ("━", fg(CYAN))
        };
        if style != run_style && !run.is_empty() {
            bar.push(span(std::mem::take(&mut run), run_style));
        }
        run_style = style;
        run.push_str(ch);
    }
    bar.push(span(run, run_style));
    let mut out = vec![Line::from(bar)];

    let at = shift.map(|s| (corner(real_now, due, s), s));
    let taken_l = format!("taken {}", hm(day_minute(taken)));
    let mut left = vec![span(format!("        {taken_l}"), fg(FAINT))];
    let used = 8 + taken_l.chars().count();
    let left_text = clock_text(Jobs::left(job, real_now).unwrap_or(0));
    let due_l = format!("due {}", hm(day_minute(due)));
    let rights = [
        vec![
            span(format!("now {} · ", hm(day_minute(real_now))), fg(DIM)),
            span(format!("{left_text} left"), fg(TEXT)),
            span(format!(" · {due_l}"), fg(DIM)),
        ],
        vec![
            span(format!("{left_text} left"), fg(TEXT)),
            span(format!(" · {due_l}"), fg(DIM)),
        ],
        vec![span(due_l.clone(), fg(DIM))],
    ];
    // Where the fixer leaves goes under the bar when it fits before the right part;
    // the due time comes first.
    let leaves = match at {
        Some((Corner::Leaves(h), _)) => {
            let x = (8 + cell(h)).saturating_sub(2).max(used + 1);
            Some((x, hm(day_minute(h))))
        }
        _ => None,
    };
    let with = leaves.as_ref().and_then(|(x, label)| {
        let after = x + label.len();
        rights
            .iter()
            .find(|r| after + 1 + cells(r) <= width)
            .map(|r| (r.clone(), *x, label.clone()))
    });
    let right = match with {
        Some((r, x, label)) => {
            left.push(span(" ".repeat(x - used), fg(FAINT)));
            left.push(span(label, fg(FAINT)));
            r
        }
        None => rights
            .into_iter()
            .find(|r| used + 1 + cells(r) <= width)
            .unwrap_or_default(),
    };
    out.push(row(left, right, width));

    let fixer = fixer_name(st, job.fixer);
    match at {
        Some((Corner::Leaves(h), (from, _))) => out.push(with_tail(
            span(
                format!("off the corner from {}", hm(day_minute(h))),
                fg(AMBER),
            ),
            span(format!(" · {fixer} is back at {}", hm(from)), fg(DIM)),
            width,
        )),
        Some((Corner::Off, (from, _))) => out.push(with_tail(
            span("off the corner now", fg(AMBER)),
            span(format!(" · back at {}, after it's due", hm(from)), fg(DIM)),
            width,
        )),
        _ => {}
    }
    out
}

/// What a job is called on its title line: its kind in capitals.
fn tag(job: &Job) -> String {
    job.kind.tag().label().to_uppercase()
}

/// The card for a job, by its state: in hand, on offer, or closed.
pub fn job_card(st: &AppState, id: JobId, width: usize, way: &Way) -> Vec<Line<'static>> {
    let Some(job) = st.jobs.get(id) else {
        return Vec::new();
    };
    match job.state {
        State::Taken => taken_card(st, job, width, way),
        State::Offered => offer_card(st, job, width, way),
        State::Done | State::Failed | State::Dropped => closed_card(st, job, width),
    }
}

fn taken_card(st: &AppState, job: &Job, width: usize, way: &Way) -> Vec<Line<'static>> {
    let standing = standing_word(st.jobs.standing(job.fixer));
    let mut out = vec![
        title_line(
            &tag(job),
            MAGENTA,
            &job.title,
            &format!(" ¤{}", job.pay),
            width,
        ),
        plain(
            format!(
                "for {} · {} · {standing}",
                fixer_name(st, job.fixer),
                district_of_fixer(st, job.fixer)
            ),
            DIM,
            width,
        ),
        blank(),
    ];
    let target_done = job.objectives.first().is_some_and(|o| o.done);
    let you = Item::You { walk: walk_of(way) };
    let items = if target_done {
        vec![
            Item::Stop(target_stop(st, job, true, false)),
            you,
            Item::Stop(handin_stop(st, job, false, false)),
        ]
    } else {
        vec![
            you,
            Item::Stop(target_stop(st, job, false, false)),
            Item::Stop(handin_stop(st, job, false, false)),
        ]
    };
    out.extend(route_lines(st, items, width));
    out.push(blank());
    out.extend(window_lines(st, job, width));
    out.push(blank());
    out.extend(brief_lines(&job.brief, width));
    out.push(blank());
    if let Some(src) = crate::track::ui::job_source(st, job.id) {
        out.push(track_line(st, &src, &stop_place(st, job), width));
    }
    out
}

fn offer_card(st: &AppState, job: &Job, width: usize, way: &Way) -> Vec<Line<'static>> {
    let standing = st.jobs.standing(job.fixer);
    let mut out = vec![
        title_line(
            &tag(job),
            MAGENTA,
            &job.title,
            &format!(" ¤{}", job.pay),
            width,
        ),
        plain(
            format!(
                "on offer from {} · {} · {}",
                fixer_name(st, job.fixer),
                district_of_fixer(st, job.fixer),
                standing_word(standing)
            ),
            DIM,
            width,
        ),
        blank(),
    ];
    let take = Stop {
        glyph: "$",
        glyph_col: AMBER,
        text: corner_text(st, job.fixer),
        text_col: TEXT,
        verb: "take the job".to_string(),
        verb_style: fg(TEXT),
        under: corner_line(st, job.fixer).into_iter().collect(),
    };
    out.extend(route_lines(
        st,
        vec![
            Item::You { walk: walk_of(way) },
            Item::Stop(take),
            Item::Stop(target_stop(st, job, false, true)),
            Item::Stop(handin_stop(st, job, false, true)),
        ],
        width,
    ));
    out.push(blank());
    out.push(row(
        vec![
            span("window  ", fg(FAINT)),
            span(format!("{} hours", job.hours), fg(TEXT)),
            span(", from when you take it", fg(DIM)),
        ],
        Vec::new(),
        width,
    ));
    out.push(blank());
    out.extend(brief_lines(&job.brief, width));
    out.push(blank());
    let vault = matches!(job.kind, Kind::Copy { vault: true, .. });
    let work = if vault {
        "vault"
    } else {
        job.kind.tag().label()
    };
    let from = opens_at(job.kind.tag(), vault);
    let long = format!(
        "{work} work · {} offers it from standing {from} · you are at {standing}",
        fixer_name(st, job.fixer)
    );
    let short = format!("{work} work from standing {from} · you are at {standing}");
    let (long, short) = if super::is_net_work(job.kind.tag()) && !super::has_deck(st) {
        (
            format!("{long} · needs a deck"),
            format!("{short} · needs a deck"),
        )
    } else {
        (long, short)
    };
    out.push(plain(
        if long.chars().count() <= width {
            long
        } else {
            short
        },
        FAINT,
        width,
    ));
    out.push(track_line(
        st,
        &fixer_source(job.fixer),
        &corner_place(st, job.fixer),
        width,
    ));
    out
}

/// The standing ladder from REFUSE_AT to 8: the thresholds marked and named, you on
/// it, and what moves it. "refuses" goes left of the bar when it would run into "fetch".
fn ladder(standing: i32, deck: bool, width: usize) -> Vec<Line<'static>> {
    const HI: i32 = 8;
    let steps = (HI - REFUSE_AT) as usize;
    let marks = [
        (REFUSE_AT, "refuses"),
        (REFUSE_AT + 1, "fetch"),
        (COPY_AT, "copy"),
        (PLANT_AT, "plant"),
        (VAULT_AT, "vault"),
    ];
    let mut step = (width.saturating_sub(4) / steps).max(2);
    let aside = step < "refuses".len() + 1;
    let lead = if aside {
        step = (width.saturating_sub(4 + 8) / steps).max(2);
        9
    } else {
        1
    };
    let mut bar = vec![span(if aside { " refuses " } else { " " }, fg(FAINT))];
    for v in REFUSE_AT..=HI {
        let mark = if marks.iter().any(|(m, _)| *m == v) {
            "┿"
        } else {
            "━"
        };
        bar.push(span(mark, fg(if v <= standing { CYAN } else { FAINTER })));
        if v < HI {
            bar.push(span(
                "━".repeat(step - 1),
                fg(if v < standing { CYAN } else { FAINTER }),
            ));
        }
    }
    let len = lead + steps * step + 1;
    let mut labels = vec![' '; len];
    for (v, name) in marks {
        if aside && v == REFUSE_AT {
            continue;
        }
        let x = lead + (v - REFUSE_AT) as usize * step;
        let free = (x..x + name.len()).all(|i| labels.get(i) == Some(&' '))
            && labels.get(x.wrapping_sub(1)).is_none_or(|c| *c == ' ');
        if free {
            for (i, ch) in name.chars().enumerate() {
                labels[x + i] = ch;
            }
        }
    }
    let mut you = vec![' '; len];
    you[lead + (standing.clamp(REFUSE_AT, HI) - REFUSE_AT) as usize * step] = '▲';
    let mut out = vec![
        row(bar, Vec::new(), width),
        plain(labels.iter().collect::<String>().trim_end(), FAINT, width),
        plain(you.iter().collect::<String>().trim_end(), WHITE, width),
        plain(
            format!("  done {GAIN_DONE:+} · dropped {LOSS_DROP} · late {LOSS_LATE}"),
            FAINTER,
            width,
        ),
    ];
    if !deck {
        out.push(plain(
            "  copy, plant and vault: needs a deck in the slot",
            AMBER,
            width,
        ));
    }
    out
}

/// The last thing a fixer said to you: the line for the latest take or ending of their
/// jobs.
fn last_said(st: &AppState, fixer: PersonId) -> Option<String> {
    let (_, _, _, id, say) = st
        .jobs
        .list
        .iter()
        .filter(|j| j.fixer == fixer)
        .flat_map(|j| {
            let taken = j.taken.map(|t| (t, 0, j.id.0, j.id, Say::Taken));
            let closed = j.closed.map(|c| {
                let say = match j.state {
                    State::Done => Say::Handed,
                    State::Dropped => Say::Dropped,
                    _ => Say::Late,
                };
                (c.at, 1, j.id.0, j.id, say)
            });
            taken.into_iter().chain(closed)
        })
        .max_by_key(|(t, rank, n, _, _)| (*t, *rank, *n))?;
    Some(st.flavor.line(say, id.0 as u64).to_string())
}

/// The card for a fixer: the walk to their corner, the standing and what it opens,
/// the record, the last thing they said.
pub fn fixer_card(st: &AppState, fixer: PersonId, width: usize, way: &Way) -> Vec<Line<'static>> {
    let standing = st.jobs.standing(fixer);
    let name = fixer_name(st, fixer);
    let district = district_of_fixer(st, fixer);
    let word = standing_word(standing);
    let mut out = vec![title_line(
        "FIXER",
        AMBER,
        &name,
        &format!(" {word}"),
        width,
    )];
    let corner = on_corner(st, fixer);
    out.push(plain(
        match corner {
            Some((true, s)) => format!("{district} corner · {} · on it now", hours_text(s)),
            Some((false, s)) => format!(
                "{district} corner · {} · off it till {}",
                hours_text(s),
                hm(s.0)
            ),
            None => format!("{district} corner"),
        },
        DIM,
        width,
    ));
    out.push(blank());
    let now = st.net.now(st);
    let (verb, verb_style) = match corner {
        Some((true, s)) => (format!("{} left", clock_text(shift_turn(now, s))), fg(TEXT)),
        Some((false, s)) => (format!("back at {}", hm(s.0)), fg(AMBER)),
        None => (String::new(), fg(TEXT)),
    };
    out.extend(route_lines(
        st,
        vec![
            Item::You { walk: walk_of(way) },
            Item::Stop(Stop {
                glyph: "$",
                glyph_col: AMBER,
                text: corner_text(st, fixer),
                text_col: TEXT,
                verb,
                verb_style,
                under: Vec::new(),
            }),
        ],
        width,
    ));
    out.push(blank());
    out.push(row(
        vec![
            span("standing ", fg(DIM)),
            span(standing.to_string(), bold(WHITE)),
            span(format!(" · {word}"), fg(CYAN)),
        ],
        Vec::new(),
        width,
    ));
    out.extend(ladder(standing, super::has_deck(st), width));
    out.push(blank());
    let mine: Vec<&Job> = st.jobs.list.iter().filter(|j| j.fixer == fixer).collect();
    let count = |s: State| mine.iter().filter(|j| j.state == s).count();
    let paid: u32 = mine
        .iter()
        .filter(|j| j.state == State::Done)
        .map(|j| j.pay)
        .sum();
    out.push(row(
        vec![
            span("record  ", fg(FAINT)),
            span(format!("{} done", count(State::Done)), fg(DONE)),
            span(
                format!(
                    " · {} late · {} dropped · ",
                    count(State::Failed),
                    count(State::Dropped)
                ),
                fg(DIM),
            ),
            span(format!("¤{paid}"), fg(TEXT)),
            span(" paid", fg(DIM)),
        ],
        Vec::new(),
        width,
    ));
    if let Some(j) = st.jobs.taken(fixer) {
        out.push(row(
            vec![
                span("in hand ", fg(FAINT)),
                span(j.title.clone(), fg(TEXT)),
                span(
                    format!(" · {} left", clock_text(Jobs::left(j, now).unwrap_or(0))),
                    fg(DIM),
                ),
            ],
            Vec::new(),
            width,
        ));
    } else if let Some(j) = st.jobs.offered(fixer) {
        out.push(row(
            vec![
                span("on offer ", fg(FAINT)),
                span(j.title.clone(), fg(TEXT)),
                span(format!(" · ¤{}", j.pay), fg(DIM)),
            ],
            Vec::new(),
            width,
        ));
    }
    if let Some(said) = last_said(st, fixer) {
        let lines = wrap(
            &format!("\u{201c}{said}\u{201d}"),
            width.saturating_sub(8),
            " ",
        );
        for (i, l) in lines.into_iter().enumerate() {
            let lead = if i == 0 { "said    " } else { "        " };
            out.push(row(
                vec![span(lead, fg(FAINT)), span(l, fg(DIM))],
                Vec::new(),
                width,
            ));
        }
    }
    out.push(blank());
    out.push(track_line(
        st,
        &fixer_source(fixer),
        &corner_place(st, fixer),
        width,
    ));
    out
}

fn closed_card(st: &AppState, job: &Job, width: usize) -> Vec<Line<'static>> {
    let word = match job.state {
        State::Done => "done",
        State::Failed => "late",
        _ => "dropped",
    };
    let mut out = vec![
        title_line(
            &tag(job),
            MAGENTA,
            &job.title,
            &format!(" ¤{}", job.pay),
            width,
        ),
        plain(
            format!(
                "for {} · {} · {word}",
                fixer_name(st, job.fixer),
                district_of_fixer(st, job.fixer)
            ),
            DIM,
            width,
        ),
        blank(),
    ];
    let reached = job.objectives.first().is_some_and(|o| o.done);
    let mut target = target_stop(st, job, reached, !reached);
    target.under.clear();
    let handin = handin_stop(st, job, job.state == State::Done, job.state != State::Done);
    out.extend(route_lines(
        st,
        vec![Item::Stop(target), Item::Stop(handin)],
        width,
    ));
    out.push(blank());
    let (delta, head) = match job.state {
        State::Done => (
            GAIN_DONE,
            vec![
                span("PAID ", bold(DONE)),
                span(format!("¤{}", job.pay), bold(WHITE)),
            ],
        ),
        State::Failed => (LOSS_LATE, vec![span("LATE", bold(MAGENTA))]),
        _ => (LOSS_DROP, vec![span("DROPPED", bold(FAINT))]),
    };
    let after = job
        .closed
        .map_or_else(String::new, |c| format!(" → {}", c.standing));
    let mut outcome = head;
    outcome.push(span(format!(" · standing {delta:+}{after}"), fg(DIM)));
    out.push(row(outcome, Vec::new(), width));
    if job.state == State::Done
        && let Kind::Plant { building } = job.kind
        && let Some((health, c)) = backdoor_health(st, building)
    {
        out.push(row(
            vec![
                span("yours   ", fg(FAINT)),
                span(
                    format!(
                        "the backdoor on {} · health ",
                        crate::helix::hosts::building_hostname(st, building)
                    ),
                    fg(DIM),
                ),
                span(health, fg(if c == DONE { TEXT } else { c })),
            ],
            Vec::new(),
            width,
        ));
    }
    out.push(blank());
    out.extend(brief_lines(&job.brief, width));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn text(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect()
    }

    #[test]
    fn row_puts_the_right_part_at_the_end_and_cuts_the_left() {
        let l = row(
            vec![span("abcdefghij", fg(TEXT))],
            vec![span("xyz", fg(TEXT))],
            10,
        );
        let t: String = l.spans.iter().map(|s| s.content.to_string()).collect();
        assert_eq!(t, "abcdef xyz".replace("abcdef", "abcde…"));
        assert_eq!(t.chars().count(), 10);
    }

    #[test]
    fn wrap_indents_what_follows() {
        assert_eq!(
            wrap("one two three four", 9, " "),
            vec!["one two", " three", " four"]
        );
    }

    #[test]
    fn the_title_line_fills_the_width() {
        let l = title_line("FETCH", MAGENTA, "The apartment shard", " ¤200", 56);
        let t = text(&[l]).remove(0);
        assert_eq!(t.chars().count(), 56, "{t}");
        assert!(t.starts_with("FETCH ━ THE APARTMENT SHARD ━"), "{t}");
        assert!(t.ends_with("━ ¤200"), "{t}");
    }

    #[test]
    fn the_ladder_names_each_threshold_once_and_marks_you() {
        for width in [56, 68] {
            let t = text(&ladder(7, true, width));
            for name in ["refuses", "fetch", "copy", "plant", "vault"] {
                assert_eq!(
                    t.iter().map(|l| l.matches(name).count()).sum::<usize>(),
                    1,
                    "{name} at {width}: {t:?}"
                );
            }
            assert_eq!(t[2].matches('▲').count(), 1);
            assert!(t.iter().all(|l| l.chars().count() <= width), "{t:?}");
            assert_eq!(t[3], "  done +1 · dropped -1 · late -2");
        }
    }

    use crate::jobs::testutil::a_fixer;
    use crate::jobs::{Event, KindTag, generate};
    use crate::track::tests::outside as state_home;

    /// A job of kind `k` from the first fixer, taken now.
    fn taken(st: &mut AppState, k: KindTag) -> JobId {
        let id = offered(st, k);
        crate::jobs::take_job(st, id).unwrap();
        id
    }

    fn offered(st: &mut AppState, k: KindTag) -> JobId {
        let fx = a_fixer(st);
        st.jobs.fixer_mut(fx).standing = PLANT_AT;
        let job = (0..40)
            .find_map(|n| generate(st, fx, k, n))
            .expect("a job of the kind");
        st.jobs.push(job)
    }

    fn card(st: &AppState, id: JobId, width: usize) -> Vec<String> {
        let way = crate::track::ui::job_source(st, id).map_or(Way::Nowhere, |s| way(st, &s));
        let lines = text(&job_card(st, id, width, &way));
        for l in &lines {
            assert!(l.chars().count() <= width, "wider than {width}: {l:?}");
        }
        lines
    }

    fn has(lines: &[String], s: &str) -> bool {
        lines.iter().any(|l| l.contains(s))
    }

    fn tick_first(st: &mut AppState, id: JobId) {
        st.jobs
            .list
            .iter_mut()
            .find(|j| j.id == id)
            .unwrap()
            .objectives[0]
            .done = true;
    }

    #[test]
    fn each_kind_says_its_words_to_do_and_done() {
        let mut st = state_home();
        for (k, todo, did) in [
            (KindTag::Fetch, "take the shard", "shard taken"),
            (KindTag::Copy, "copy its server", "server copied"),
            (KindTag::Plant, "plant a backdoor", "backdoor planted"),
            (KindTag::Deliver, "deliver the package", "package delivered"),
        ] {
            let id = taken(&mut st, k);
            let c = card(&st, id, 68);
            assert!(has(&c, todo) && has(&c, "hand it in"), "{k:?}: {c:#?}");
            assert!(has(&c, &k.label().to_uppercase()), "{k:?}: {c:#?}");
            tick_first(&mut st, id);
            let c = card(&st, id, 68);
            assert!(
                c.iter().any(|l| l.contains('✓') && l.contains(did)),
                "{k:?}: {c:#?}"
            );
        }
    }

    #[test]
    fn the_door_line_follows_what_opens_it() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Fetch);
        let Kind::Fetch {
            building,
            level,
            number,
            ..
        } = st.jobs.get(id).unwrap().kind
        else {
            panic!()
        };
        assert!(has(&card(&st, id, 68), "door code not known · "));
        st.journal.learn(crate::credentials::Code {
            building,
            level,
            number,
            household: None,
            code: 4821,
            source: crate::credentials::Source::Keypad,
            noted_by: None,
            at: 0,
        });
        assert!(has(&card(&st, id, 68), "door code 4821"));
    }

    #[test]
    fn a_plant_says_when_to_hand_in_and_how_its_backdoor_holds() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Plant);
        let Kind::Plant { building } = st.jobs.get(id).unwrap().kind else {
            panic!()
        };
        let c = card(&st, id, 68);
        assert!(has(&c, "hand it in before the backdoor burns"), "{c:#?}");
        assert!(!has(&c, "READY"));
        let now = st.net.now(&st);
        st.backdoors.plant_net(building, 1, 7, now);
        crate::jobs::send(&mut st, Event::Planted { building });
        let c = card(&st, id, 68);
        assert!(has(&c, "backdoor STABLE"), "{c:#?}");
        assert!(has(&c, "READY"), "{c:#?}");
    }

    #[test]
    fn ready_or_the_reason_it_would_be_refused() {
        let mut st = state_home();
        let d = taken(&mut st, KindTag::Deliver);
        tick_first(&mut st, d);
        assert!(has(&card(&st, d, 68), "READY"));
        let f = taken(&mut st, KindTag::Fetch);
        tick_first(&mut st, f);
        let c = card(&st, f, 68);
        assert!(
            has(&c, "the shard is not on you") && !has(&c, "READY"),
            "{c:#?}"
        );
        let fixer = st.jobs.get(d).unwrap().fixer;
        st.jobs.fixer_mut(fixer).standing = REFUSE_AT;
        let c = card(&st, d, 68);
        let name = fixer_name(&st, fixer);
        assert!(
            has(&c, &format!("{name} won't deal with you now")),
            "{c:#?}"
        );
        assert!(!has(&c, "READY"));
    }

    #[test]
    fn the_window_says_when_the_fixer_leaves_the_corner() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Fetch);
        let j = st.jobs.get(id).unwrap().clone();
        let fixer = fixer_name(&st, j.fixer);
        // Taken at 22:00 for 12 hours: due 10:00, the corner held till 04:00.
        let w = text(&window_lines(&st, &j, 68));
        assert_eq!(day_minute(j.taken.unwrap()), 22 * 60);
        assert!(w[0].contains('┄') && w[0].contains('╋'), "{w:#?}");
        assert!(
            w[1].contains("taken 22:00") && w[1].contains("04:00"),
            "{w:#?}"
        );
        assert!(w[1].contains("due 10:00"), "{w:#?}");
        assert_eq!(
            w[2],
            format!("off the corner from 04:00 · {fixer} is back at 18:00")
        );
        assert!(!w.iter().any(|l| l.contains("hand in by")));
        // At 05:00 the fixer is off till after it's due.
        let ticks = 7 * 60 * sim::TICKS_PER_MINUTE;
        let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) else {
            panic!()
        };
        world.advance(city, ticks);
        let w = text(&window_lines(&st, &j, 68));
        assert_eq!(
            w[2], "off the corner now · back at 18:00, after it's due",
            "{w:#?}"
        );
        // The fixer still takes a ready job off the corner.
        let d = taken(&mut st, KindTag::Deliver);
        tick_first(&mut st, d);
        let c = card(&st, d, 68);
        assert!(
            has(&c, "READY") && has(&c, "off the corner · back at 18:00"),
            "{c:#?}"
        );
    }

    #[test]
    fn the_offer_starts_at_the_fixer() {
        let mut st = state_home();
        let id = offered(&mut st, KindTag::Copy);
        let j = st.jobs.get(id).unwrap().clone();
        let name = fixer_name(&st, j.fixer);
        let c = card(&st, id, 68);
        assert!(has(&c, "on offer from "), "{c:#?}");
        assert!(
            has(&c, "take the job") && has(&c, "copy its server"),
            "{c:#?}"
        );
        assert!(has(&c, "window  24 hours, from when you take it"), "{c:#?}");
        assert!(
            has(
                &c,
                &format!("copy work · {name} offers it from standing 2 · you are at 4")
            ),
            "{c:#?}"
        );
        assert!(
            has(&c, " t   TRACK · ") && has(&c, &format!("{name}'s corner")),
            "{c:#?}"
        );
    }

    #[test]
    fn the_fixer_card_reads_the_corner_the_standing_and_the_record() {
        let mut st = state_home();
        let d = taken(&mut st, KindTag::Deliver);
        let fx = st.jobs.get(d).unwrap().fixer;
        tick_first(&mut st, d);
        crate::jobs::ui::hand_in(&mut st, d).unwrap();
        let way = way(&st, &fixer_source(fx));
        let c = text(&fixer_card(&st, fx, 68, &way));
        for l in &c {
            assert!(l.chars().count() <= 68, "{l:?}");
        }
        assert!(has(&c, "corner · 18:00–04:00 · on it now"), "{c:#?}");
        assert!(has(&c, "6h 0m left"), "{c:#?}");
        assert!(has(&c, "standing 5 · trusted"), "{c:#?}");
        assert!(has(&c, "record  1 done · 0 late · 0 dropped · ¤"), "{c:#?}");
        let said = st.flavor.line(Say::Handed, d.0 as u64).to_string();
        assert!(has(&c, &said), "said {said:?}: {c:#?}");
    }

    #[test]
    fn a_closed_job_says_what_it_paid_or_cost() {
        let mut st = state_home();
        let d = taken(&mut st, KindTag::Deliver);
        tick_first(&mut st, d);
        crate::jobs::ui::hand_in(&mut st, d).unwrap();
        let after = st.jobs.standing(st.jobs.get(d).unwrap().fixer);
        let c = card(&st, d, 68);
        let pay = st.jobs.get(d).unwrap().pay;
        assert!(
            has(&c, &format!("PAID ¤{pay} · standing +1 → {after}")),
            "{c:#?}"
        );
        assert!(
            has(&c, "handed in") && has(&c, "package delivered"),
            "{c:#?}"
        );
        assert!(!has(&c, "TRACK"));
        let x = taken(&mut st, KindTag::Fetch);
        crate::jobs::ui::drop(&mut st, x).unwrap();
        let after = st.jobs.standing(st.jobs.get(x).unwrap().fixer);
        assert!(has(
            &card(&st, x, 56),
            &format!("DROPPED · standing -1 → {after}")
        ));
    }

    #[test]
    fn a_done_plant_says_its_backdoor_is_still_yours() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Plant);
        let Kind::Plant { building } = st.jobs.get(id).unwrap().kind else {
            panic!()
        };
        let now = st.net.now(&st);
        st.backdoors.plant_net(building, 1, 7, now);
        crate::jobs::send(&mut st, Event::Planted { building });
        crate::jobs::ui::hand_in(&mut st, id).unwrap();
        let host = crate::helix::hosts::building_hostname(&st, building);
        let c = card(&st, id, 68);
        assert!(
            has(
                &c,
                &format!("yours   the backdoor on {host} · health STABLE")
            ),
            "{c:#?}"
        );
    }

    #[test]
    fn the_way_is_the_walk_the_track_would_make() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Fetch);
        let src = crate::track::ui::job_source(&st, id).unwrap();
        let w = way(&st, &src);
        crate::track::set(&mut st, src).unwrap();
        assert_eq!(
            w,
            Way::Walk {
                m: crate::track::distance(&st).unwrap(),
                next: crate::track::next_now(&st).map(str::to_string),
            }
        );
        assert!(has(&card(&st, id, 68), &way_text(&w)));
        crate::jobs::ui::drop(&mut st, id).unwrap();
        assert_eq!(way(&st, &src), Way::Nowhere, "a dropped job is done with");
    }

    #[test]
    fn the_track_line_says_what_t_does_then_lights_up() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Fetch);
        let src = crate::track::ui::job_source(&st, id).unwrap();
        let place = stop_place(&st, st.jobs.get(id).unwrap());
        assert!(place.starts_with("flat "), "{place}");
        let l = text(&[track_line(&st, &src, &place, 68)]).remove(0);
        assert_eq!(
            l,
            format!(" t   TRACK · the floor lights the way to {place}")
        );
        crate::track::set(&mut st, src).unwrap();
        for width in [56, 68] {
            let line = track_line(&st, &src, &place, width);
            let l = text(std::slice::from_ref(&line)).remove(0);
            assert_eq!(l.chars().count(), width, "{l:?}");
            assert!(
                l.starts_with(" ⌖ TRACKING  flat ") && l.ends_with(" t  stop"),
                "{l:?}"
            );
            assert!(
                line.spans
                    .iter()
                    .all(|s| s.style.bg == Some(BAR_BG) || s.content == " t "),
                "the band is lit end to end"
            );
        }
    }

    #[test]
    fn the_ladder_says_needs_a_deck() {
        for width in [56, 68] {
            let without = text(&ladder(7, false, width));
            assert!(
                without.iter().any(|l| l.contains("needs a deck")),
                "{without:?}"
            );
            assert!(without.iter().all(|l| l.chars().count() <= width));
            let with = text(&ladder(7, true, width));
            assert!(with.iter().all(|l| !l.contains("needs a deck")));
        }
        let mut st = state_home();
        let id = offered(&mut st, KindTag::Copy);
        st.character.equipment.remove(&game::Slot::Deck);
        assert!(has(&card(&st, id, 68), "needs a deck"));
    }
}
