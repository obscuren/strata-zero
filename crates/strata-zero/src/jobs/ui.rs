//! What the player sees of the jobs: the QUEST pane, the deck's tab, the names.

use super::{JobId, Jobs, State, WARN_MINUTES, clock_text};
use crate::app::AppState;
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
        .and_then(|pos| city.district_at(pos))
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

use super::{COPY_AT, Event, Kind, PLANT_AT, REFUSE_AT, Say, VAULT_AT, handin_item, handin_sifted};
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

pub(crate) fn refused(st: &AppState, fixer: PersonId) -> bool {
    st.jobs.standing(fixer) <= REFUSE_AT || st.backdoors.fixer_quiet(st.net.now(st))
}

fn stands(st: &AppState) -> impl Fn(BuildingId) -> bool + '_ {
    move |b| st.backdoors.standing().any(|d| d.building == b)
}

fn carries(st: &AppState) -> impl Fn(&ItemId) -> bool + '_ {
    move |i| st.character.count(i) > 0
}

/// Whether the fixer would take the hand-in if you found them now, else why not: their
/// refusal first, then the job's own reason.
pub(crate) fn hand_in_now(st: &AppState, id: JobId) -> Result<(), String> {
    let j = st.jobs.get(id).ok_or_else(|| "no such job".to_string())?;
    if refused(st, j.fixer) {
        return Err(format!(
            "{} won't deal with you now",
            fixer_name(st, j.fixer)
        ));
    }
    st.jobs.ready(id, &stands(st), &carries(st))
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
            Ok(()) => {
                let tail = format!(" · ¤{}{}", j.pay, bonus(&j.kind).map_or("", |b| b.1));
                out.push(
                    Choice::on('h', "Hand in", j.title.clone(), CYAN, Pick::HandIn(j.id))
                        .tail(tail),
                )
            }
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

/// A job's own mails, newest first, by their place in the inbox.
pub fn job_mails(st: &AppState, job: JobId) -> Vec<usize> {
    let mut v: Vec<usize> = st
        .journal
        .mails
        .iter()
        .enumerate()
        .filter(|(_, m)| m.job == Some(job))
        .map(|(i, _)| i)
        .collect();
    v.reverse();
    v
}

/// The mails about a fixer's jobs, newest first; never the story's, even from a fixer
/// the story also casts.
pub fn fixer_mails(st: &AppState, fixer: PersonId) -> Vec<usize> {
    let mut v: Vec<usize> = st
        .journal
        .mails
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            m.job
                .and_then(|j| st.jobs.get(j))
                .is_some_and(|j| j.fixer == fixer)
        })
        .map(|(i, _)| i)
        .collect();
    v.reverse();
    v
}

/// The mail of `mails` (newest first) to read next: the newest unread, else the newest.
pub fn next_mail(st: &AppState, mails: &[usize]) -> Option<usize> {
    mails
        .iter()
        .copied()
        .find(|i| st.journal.mails.get(*i).is_some_and(|m| !m.read))
        .or_else(|| mails.first().copied())
}

/// Mails as the rig reads them, one after another, each marked read.
pub fn mail_card(st: &mut AppState, mails: &[usize]) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for (k, i) in mails.iter().enumerate() {
        let Some(m) = st.journal.mails.get(*i) else {
            continue;
        };
        if k > 0 {
            out.push(plain(""));
        }
        out.push(dim(format!("From: {}", m.from)));
        out.push(dim(format!("Subject: {}", m.subject)));
        out.extend(m.body.iter().map(|l| plain(l.clone())));
        crate::credentials::mail_read(st, *i);
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

/// What a job pays on top of its credits, and the words the hand-in row and line
/// carry after the pay: a fetch pays a stock fuse for the Keyjack.
fn bonus(kind: &Kind) -> Option<(ItemId, &'static str)> {
    matches!(kind, Kind::Fetch { .. })
        .then(|| (ItemId::new(crate::keyjack::STOCK_FUSE), " · a stock fuse"))
}

/// The sleds a fixer pays for knowing you, in the order they come.
const KNOW_SLEDS: [&str; 4] = [
    "sled_tinman",
    "sled_switchback",
    "sled_longwire",
    "sled_sandbag",
];
/// The racks trust sends, in the order they come.
const TRUST_RACKS: [&str; 2] = ["rack_civitas", "rack_lotus"];

/// What a hand-in that moved a fixer's standing from `before` to `after` brings in rig
/// parts, into the pack: a sled for each fixer who comes to know you, a rack (and a mail
/// about it) for the first two who trust you, the Halcyon for the first inner circle,
/// and the program shards from the first Plant and the first Copy. Each item with the
/// words the line adds.
pub fn rig_rewards(
    st: &mut AppState,
    fixer: PersonId,
    before: i32,
    after: i32,
    kind: &Kind,
) -> Vec<(ItemId, String)> {
    let name = fixer_name(st, fixer);
    let crossed = |at: i32| before < at && after >= at;
    let mut got: Vec<ItemId> = Vec::new();
    let mut racks: Vec<ItemId> = Vec::new();
    let r = &mut st.character.rig_rewards;
    if crossed(COPY_AT)
        && let Some(sled) = KNOW_SLEDS.get(r.sleds as usize)
        && r.paid.insert(format!("know:{name}"))
    {
        r.sleds += 1;
        got.push(ItemId::new(sled));
    }
    if crossed(PLANT_AT)
        && let Some(rack) = TRUST_RACKS.get(r.racks as usize)
        && r.paid.insert(format!("trust:{name}"))
    {
        r.racks += 1;
        racks.push(ItemId::new(rack));
    }
    if crossed(VAULT_AT) && !r.halcyon && r.paid.insert(format!("inner:{name}")) {
        r.halcyon = true;
        racks.push(ItemId::new("rack_halcyon"));
    }
    let program = match kind {
        Kind::Plant { .. } => Some(("first_plant", "prog_sniff")),
        Kind::Copy { .. } => Some(("first_copy", "prog_keepalive")),
        _ => None,
    };
    if let Some((key, item)) = program
        && r.paid.insert(key.to_string())
    {
        got.push(ItemId::new(item));
    }
    let named = |st: &AppState, i: &ItemId| {
        let n = st.content.item(i).map_or(i.0.clone(), |i| i.name.clone());
        format!("a {n}")
    };
    let mut out = Vec::new();
    for item in got {
        // Hardware gets a copy of its own, as the Keyjack's fuse does; a shard just stacks.
        st.character.give_item(&st.content, item.clone(), 1);
        // a rig program comes on a shard, and is named as `run` takes it
        let words = match program {
            Some((_, p)) if item.0 == p => {
                let n = st.content.item(&item).map_or(p, |i| i.name.as_str());
                format!("a {} shard", n.to_lowercase())
            }
            _ => named(st, &item),
        };
        out.push((item, words));
    }
    for rack in racks {
        st.character.give_item(&st.content, rack.clone(), 1);
        let roll = st.character.rig_rewards.paid.len() as u64;
        let said = st.flavor.line(Say::Rack, roll).to_string();
        let words = named(st, &rack);
        let now = st.net.now(st);
        let mail = crate::credentials::Mail {
            from: name.clone(),
            subject: words.clone(),
            body: vec![said],
            read: false,
            at: now,
            job: None,
            story: None,
        };
        st.journal.mails.push(mail);
        out.push((rack, words));
    }
    out
}

/// Ends the job at its fixer: the item out of the pack, the pay in, the line logged.
pub fn hand_in(st: &mut AppState, id: JobId) -> Result<String, String> {
    let j = st.jobs.get(id).cloned().ok_or("no such job")?;
    st.jobs.ready(id, &stands(st), &carries(st))?;
    if let Some(item) = handin_item(&j.kind) {
        // The plain shard first; a rig's sifted one takes its place when that's all there is.
        let sifted = handin_sifted(&j.kind).filter(|_| st.character.count(&item) == 0);
        game::remove_from_pack(&mut st.character, &sifted.unwrap_or(item), 1);
    }
    let now = st.net.now(st);
    let before = st.jobs.standing(j.fixer);
    let pay = st.jobs.hand_in(id, now).ok_or("not in hand")?;
    let after = st.jobs.standing(j.fixer);
    st.character.credits = st.character.credits.saturating_add(pay);
    let bonus = bonus(&j.kind);
    if let Some((item, _)) = &bonus {
        // A copy of its own, as hardware is held, so it can be fitted.
        st.character.give_item(&st.content, item.clone(), 1);
    }
    let fuse = bonus.map_or("", |b| b.1);
    let parts: String = rig_rewards(st, j.fixer, before, after, &j.kind)
        .into_iter()
        .map(|(_, words)| format!(" · {words}"))
        .collect();
    let fuse = format!("{fuse}{parts}");
    let fixer = fixer_name(st, j.fixer);
    let said = st.flavor.line(Say::Handed, id.0 as u64).to_string();
    let line = format!(
        "`{}` · handed in · ¤{pay}{fuse} · {fixer}: \u{201c}{said}\u{201d}",
        j.title
    );
    st.story(line.clone());
    Ok(line)
}

/// Gives the job up; the package, if still carried, stays in the pack.
pub fn drop(st: &mut AppState, id: JobId) -> Result<String, String> {
    let j = st.jobs.get(id).cloned().ok_or("no such job")?;
    let now = st.net.now(st);
    if !st.jobs.drop_job(id, now) {
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
    use crate::jobs::{HOURS_FETCH, Job, Kind, Objective, PAY_FETCH};
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
            closed: None,
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

    fn fetch() -> Kind {
        Kind::Fetch {
            building: BuildingId(0),
            level: 1,
            number: 101,
            tile: citygen::Pos::new(1, 1),
        }
    }

    fn two_fixers(st: &AppState) -> (PersonId, PersonId) {
        let mut it = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .filter(|p| p.profession == "fixer")
            .map(|p| p.id);
        (it.next().unwrap(), it.next().expect("two fixers"))
    }

    fn sleds_in_pack(st: &AppState, id: &str) -> u32 {
        st.character.count(&ItemId::new(id))
    }

    /// The racks in the pack, in pack order.
    fn racks_in_pack(st: &AppState) -> Vec<String> {
        st.character
            .inventory
            .iter()
            .filter(|s| st.content.item(&s.item).unwrap().kind == game::ItemKind::Frame)
            .map(|s| s.item.0.clone())
            .collect()
    }

    #[test]
    fn knowing_you_pays_a_sled_once() {
        use crate::jobs::COPY_AT;
        let mut st = state_home();
        let (a, b) = two_fixers(&st);
        let got = rig_rewards(&mut st, a, COPY_AT - 1, COPY_AT, &fetch());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, ItemId::new("sled_tinman"));
        assert_eq!(sleds_in_pack(&st, "sled_tinman"), 1);
        let got = rig_rewards(&mut st, b, COPY_AT - 1, COPY_AT, &fetch());
        assert_eq!(got[0].0, ItemId::new("sled_switchback"));
        // Same fixer again, or a drop and a climb back, pays nothing.
        assert!(rig_rewards(&mut st, a, COPY_AT - 1, COPY_AT, &fetch()).is_empty());
        assert!(rig_rewards(&mut st, a, COPY_AT, COPY_AT + 1, &fetch()).is_empty());
        assert_eq!(st.character.rig_rewards.sleds, 2);
    }

    #[test]
    fn the_first_trust_sends_the_civitas() {
        use crate::jobs::PLANT_AT;
        let mut st = state_home();
        let (a, b) = two_fixers(&st);
        let got = rig_rewards(&mut st, a, PLANT_AT - 1, PLANT_AT, &fetch());
        assert!(got.iter().any(|(i, _)| *i == ItemId::new("rack_civitas")));
        assert_eq!(racks_in_pack(&st), vec!["rack_civitas"]);
        assert!(
            st.character.inventory.iter().all(|s| s.copy.is_some()
                || st.content.item(&s.item).unwrap().kind != game::ItemKind::Frame),
            "a rack in the pack can be fitted"
        );
        let mail = st.journal.mails.last().unwrap();
        assert_eq!(mail.from, fixer_name(&st, a));
        assert_eq!(mail.subject, "a Civitas cabinet");
        assert!(
            !mail
                .body
                .iter()
                .any(|l| l.contains("door") || l.contains("flat")),
            "{:?}",
            mail.body
        );
        assert!(!mail.read);
        rig_rewards(&mut st, b, PLANT_AT - 1, PLANT_AT, &fetch());
        assert_eq!(racks_in_pack(&st), vec!["rack_civitas", "rack_lotus"]);
        // A third fixer, or the same one again, sends nothing more.
        let mails = st.journal.mails.len();
        rig_rewards(&mut st, a, PLANT_AT - 1, PLANT_AT, &fetch());
        assert_eq!(racks_in_pack(&st).len(), 2);
        assert_eq!(st.journal.mails.len(), mails);
    }

    #[test]
    fn a_rack_earned_while_away_comes_with_you() {
        use crate::jobs::PLANT_AT;
        let mut st = state_home();
        let (a, _) = two_fixers(&st);
        // Home set aside, as in another city: its journal goes with it.
        let mut home = state_home();
        st.away = crate::outside::set_aside(&mut home).map(Box::new);
        let mails = st.journal.mails.len();
        let home_mails = st.away.as_ref().unwrap().journal.mails.len();
        rig_rewards(&mut st, a, PLANT_AT - 1, PLANT_AT, &fetch());
        assert_eq!(racks_in_pack(&st), vec!["rack_civitas"], "in the pack");
        assert_eq!(st.journal.mails.len(), mails + 1, "the mail here");
        let journal = &st.away.as_ref().unwrap().journal;
        assert_eq!(journal.mails.len(), home_mails, "none waiting at home");
    }

    #[test]
    fn inner_circle_sends_the_halcyon_once() {
        use crate::jobs::VAULT_AT;
        let mut st = state_home();
        let (a, b) = two_fixers(&st);
        rig_rewards(&mut st, a, VAULT_AT - 1, VAULT_AT, &fetch());
        assert!(racks_in_pack(&st).contains(&"rack_halcyon".to_string()));
        assert!(st.character.rig_rewards.halcyon);
        let n = racks_in_pack(&st).len();
        rig_rewards(&mut st, b, VAULT_AT - 1, VAULT_AT, &fetch());
        assert_eq!(sleds_in_pack(&st, "rack_halcyon"), 1);
        assert!(racks_in_pack(&st).len() <= n + 1);
    }

    #[test]
    fn first_plant_pays_sniff_first_copy_pays_keepalive() {
        let mut st = state_home();
        let (a, _) = two_fixers(&st);
        let plant = Kind::Plant {
            building: BuildingId(1),
        };
        let copy = Kind::Copy {
            building: BuildingId(1),
            vault: false,
        };
        let got = rig_rewards(&mut st, a, 0, 1, &plant);
        assert_eq!(got[0].0, ItemId::new("prog_sniff"));
        assert_eq!(got[0].1, "a sniff shard");
        assert_eq!(sleds_in_pack(&st, "prog_sniff"), 1);
        assert!(rig_rewards(&mut st, a, 2, 3, &plant).is_empty());
        let got = rig_rewards(&mut st, a, 2, 3, &copy);
        assert!(
            got.iter()
                .any(|(i, w)| *i == ItemId::new("prog_keepalive") && w == "a keepalive shard"),
            "{got:?}"
        );
        assert_eq!(sleds_in_pack(&st, "prog_keepalive"), 1);
        assert!(
            rig_rewards(&mut st, a, 3, 4, &copy)
                .iter()
                .all(|(i, _)| *i != ItemId::new("prog_keepalive"))
        );
    }

    #[test]
    fn a_sifted_shard_hands_in() {
        use crate::jobs::handin_sifted;
        let copy = Kind::Copy {
            building: BuildingId(1),
            vault: true,
        };
        assert_eq!(handin_sifted(&fetch()), Some(ItemId::new("paydata_sifted")));
        assert_eq!(handin_sifted(&copy), Some(ItemId::new("vault_data_sifted")));
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
        st.character.give(ItemId::new("paydata_sifted"), 1);
        assert!(hand_in(&mut st, id).is_ok());
        assert_eq!(st.character.count(&ItemId::new("paydata_sifted")), 0);
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
        st.jobs.drop_job(welcome, 0);
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
        st.jobs.drop_job(welcome, 0);
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

    #[test]
    fn a_fetch_pays_a_fuse() {
        let mut st = state_home();
        let id = st.jobs.active().unwrap().id;
        assert!(matches!(st.jobs.get(id).unwrap().kind, Kind::Fetch { .. }));
        st.character.give(ItemId::new("paydata"), 1);
        let (b, level, _, tile) = first_fetch(&st).unwrap();
        crate::jobs::send(
            &mut st,
            Event::Taken {
                place: Place::Floor { building: b, level },
                pos: tile,
            },
        );
        let fuse = ItemId::new(crate::keyjack::STOCK_FUSE);
        let copies = |st: &AppState| -> Vec<Option<game::CopyId>> {
            let pack = st.character.inventory.iter();
            pack.filter(|s| s.item == fuse)
                .map(|s| s.copy_id())
                .collect()
        };
        let (fuses, before) = (st.character.count(&fuse), copies(&st));
        let line = hand_in(&mut st, id).unwrap();
        assert!(line.contains("¤200 · a stock fuse"), "{line}");
        assert_eq!(st.character.count(&fuse), fuses + 1);
        // The fuse is its own copy, as hardware is, so it goes in a Keyjack's socket.
        let after = copies(&st);
        let new: Vec<_> = after.iter().filter(|k| !before.contains(k)).collect();
        assert!(
            matches!(new[..], [Some(_)]),
            "one new copy: {before:?} -> {after:?}"
        );
        let paid = new[0].unwrap();
        let keyjack = ItemId::new(crate::keyjack::ITEM);
        st.character.give_item(&st.content, keyjack.clone(), 1);
        let empty = st.character.inventory.iter().rev();
        let host = empty
            .filter(|s| s.item == keyjack)
            .find_map(|s| s.copy_id())
            .map(game::Host::Pack)
            .unwrap();
        let fit = game::Action::Fit {
            host,
            socket: 0,
            piece: paid,
        };
        crate::actions::perform(&mut st, fit).unwrap();
        assert_eq!(st.character.count(&fuse), fuses, "the fuse left the pack");
    }
}
