//! The deck's JOBS tab (spec 1, 2, 10): on the left the story, then each fixer who has
//! given you work with their jobs, a fixer's closed ones folded; on the right the
//! selected row's card, each of its lines backed. `t` tracks the row, `m` shows it on
//! the MAP, `← →` fold, space opens its verbs.

use crate::app::AppState;
use crate::jobs::card::{self, Way, fixer_source};
use crate::jobs::ui::{
    district_of_fixer, fixer_mails, fixer_name, hand_in_now, job_mails, next_mail,
};
use crate::jobs::{
    Job, JobId, Jobs, State, WARN_MINUTES, clock_text, day_minute, in_shift, shift_of,
};
use crate::missions::RunState;
use crate::missions::ui::{story_card, story_missions};
use crate::screens::deck::{REVEAL_TICKS, Tab, cut, lit};
use crate::screens::popup::{Choice, Pick, Popup};
use crate::track::ui::{job_source, story_source};
use crate::track::{self, ListingId, Source};
use crate::ui::chrome::{
    BAR_BG, CYAN, DIM, FAINT, FAINTER, MAGENTA, PANEL_BG, RULE, TEXT, put, reveal,
};
use crate::ui::hud::AMBER;
use citygen::{Place, Pos};
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use sim::PersonId;
use std::cell::{Cell, RefCell};

pub(crate) const HINT: &str =
    "Q/E tabs · ↑↓ select · ←→ fold · space verbs · t track · m map · tab close";

/// Under this many columns the list drops a job's pay and shortens the FIXERS header.
const NARROW: u16 = 44;

/// One row of the list.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Header(String),
    Mission(usize),
    Fixer(PersonId),
    Job(JobId),
    /// The next step under a live job or an open mission, and whether it is the track.
    Next(String, bool),
    /// A fixer's closed jobs, folded into one row.
    Folded {
        fixer: PersonId,
        n: usize,
    },
    Gap,
    Empty,
}

impl Entry {
    fn selectable(&self) -> bool {
        matches!(
            self,
            Entry::Mission(_) | Entry::Fixer(_) | Entry::Job(_) | Entry::Folded { .. }
        )
    }
}

fn closed(j: &Job) -> bool {
    matches!(j.state, State::Done | State::Failed | State::Dropped)
}

/// What the cached walks were made from: where you stand, each job's state and ticks,
/// the story's steps, the track. The world stands still while the deck is open, so
/// they last the visit unless one of these changes.
#[derive(Clone, Debug, PartialEq)]
struct Stamp {
    at: Option<(Place, Pos)>,
    jobs: Vec<(JobId, State, Vec<bool>)>,
    story: Vec<usize>,
    track: Option<Source>,
}

impl Stamp {
    fn now(st: &AppState) -> Stamp {
        Stamp {
            at: st.player.as_ref().map(|p| (p.place, p.pos)),
            jobs: st
                .jobs
                .list
                .iter()
                .map(|j| (j.id, j.state, j.objectives.iter().map(|o| o.done).collect()))
                .collect(),
            story: st.missions.runs.iter().map(|r| r.at).collect(),
            track: st.track.as_ref().map(|t| t.source),
        }
    }
}

#[derive(Default)]
struct Ways {
    stamp: Option<Stamp>,
    map: Vec<(Source, Way)>,
}

/// What a key did on the tab.
pub(crate) enum Key {
    /// Not the tab's: the deck's to handle.
    Pass,
    Done,
    Popup(Popup),
}

pub(crate) struct JobsTab {
    pub row: usize,
    /// The selected entry, so the cursor stays on it as rows come and go.
    pub(crate) sel: Option<Entry>,
    /// Tick of the last row change; drives the card's typewriter.
    pub changed: u64,
    ways: RefCell<Ways>,
    /// The list at the last draw: its rect and first row, for the popup's anchor and
    /// for whether the gaps fit.
    last_list: Cell<(Rect, usize)>,
}

impl JobsTab {
    pub fn new(tick: u64) -> JobsTab {
        JobsTab {
            row: 0,
            sel: None,
            changed: tick,
            ways: RefCell::default(),
            last_list: Cell::new((Rect::new(0, 0, u16::MAX, u16::MAX), 0)),
        }
    }

    /// The tab as the deck left it.
    pub fn restore(st: &AppState) -> JobsTab {
        let mut t = JobsTab::new(st.tick);
        t.row = st.deck_rows[Tab::Jobs.index()];
        t.sel = st.deck_jobs_sel.clone();
        t.reselect(st);
        t
    }

    /// The walk to `source`, from the cache while nothing it was made from has changed.
    fn way(&self, st: &AppState, source: &Source) -> Way {
        let stamp = Stamp::now(st);
        let mut ways = self.ways.borrow_mut();
        if ways.stamp.as_ref() != Some(&stamp) {
            ways.stamp = Some(stamp);
            ways.map.clear();
        }
        if let Some((_, w)) = ways.map.iter().find(|(s, _)| s == source) {
            return w.clone();
        }
        let w = card::way(st, source);
        ways.map.push((*source, w.clone()));
        w
    }

    /// What a row tracks and walks to: a job's stop, an offer's fixer, a fixer, the
    /// story's step; nothing for a closed job.
    fn source_of(&self, st: &AppState, e: &Entry) -> Option<Source> {
        match e {
            Entry::Mission(m) => story_source(st, *m),
            Entry::Fixer(f) | Entry::Folded { fixer: f, .. } => Some(fixer_source(*f)),
            Entry::Job(id) => {
                let j = st.jobs.get(*id)?;
                match j.state {
                    State::Taken => job_source(st, *id),
                    State::Offered => Some(fixer_source(j.fixer)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The list as it would draw now: with a gap between sections and fixers while
    /// that fits the list's height, else without.
    pub(crate) fn entries(&self, st: &AppState) -> Vec<Entry> {
        let (list, _) = self.last_list.get();
        let full = self.build(st, true, list.width >= NARROW);
        if full.len() > list.height as usize {
            self.build(st, false, list.width >= NARROW)
        } else {
            full
        }
    }

    fn build(&self, st: &AppState, gaps: bool, wide: bool) -> Vec<Entry> {
        let mut out = Vec::new();
        let story = story_missions(st);
        if !story.is_empty() {
            out.push(Entry::Header("STORY".to_string()));
            for (m, done) in story {
                out.push(Entry::Mission(m));
                if done {
                    continue;
                }
                if let Some(src) = story_source(st, m) {
                    // The walk, and who or what is at its end.
                    let way = self.way(st, &src);
                    let t = match way {
                        Way::Walk { .. } => {
                            format!(
                                "{} · {}",
                                card::way_text(&way),
                                card::target_place(st, &src)
                            )
                        }
                        _ => card::way_text(&way),
                    };
                    if !t.is_empty() {
                        out.push(Entry::Next(t, track::is_tracked(st, &src)));
                    }
                }
            }
        }
        let fixers = fixers_in_order(st);
        if !fixers.is_empty() {
            if gaps && !out.is_empty() {
                out.push(Entry::Gap);
            }
            out.push(Entry::Header(fixers_header(st, &fixers, wide)));
            for (i, f) in fixers.iter().enumerate() {
                if gaps && i > 0 {
                    out.push(Entry::Gap);
                }
                out.push(Entry::Fixer(*f));
                for j in live_jobs(st, *f) {
                    out.push(Entry::Job(j.id));
                    let (t, tracked) = self.next_of(st, j);
                    if !t.is_empty() {
                        out.push(Entry::Next(t, tracked));
                    }
                }
                let shut = closed_jobs(st, *f);
                if shut.is_empty() {
                    continue;
                }
                if st.deck_jobs_open.contains(f) {
                    out.extend(shut.iter().map(|j| Entry::Job(j.id)));
                } else {
                    out.push(Entry::Folded {
                        fixer: *f,
                        n: shut.len(),
                    });
                }
            }
        }
        if out.is_empty() {
            out.push(Entry::Empty);
        }
        out
    }

    /// The line under a live job: the walk to its stop, or to its fixer for the
    /// hand-in or an offer; and whether the job is the track.
    fn next_of(&self, st: &AppState, j: &Job) -> (String, bool) {
        let fixer = fixer_name(st, j.fixer);
        match j.state {
            State::Taken => {
                let Some(src) = job_source(st, j.id) else {
                    return (String::new(), false);
                };
                let way = self.way(st, &src);
                let at_corner = j.objectives.first().is_some_and(|o| o.done);
                let text = match (&way, at_corner) {
                    (Way::Walk { m, .. }, true) => format!("{m} m · hand in to {fixer}"),
                    _ => card::way_text(&way),
                };
                (text, track::is_tracked(st, &src))
            }
            State::Offered => match self.way(st, &fixer_source(j.fixer)) {
                Way::Walk { m, .. } => (format!("take it from {fixer} · {m} m"), false),
                _ => (format!("take it from {fixer}"), false),
            },
            _ => (String::new(), false),
        }
    }

    /// Puts the cursor back on the entry it was on; one gone from the list (a job folded
    /// away as it closed) hands it to its fixer's fold, else to the fixer. Else the
    /// nearest selectable row after where it was; on a fresh tab, the first live job.
    /// Then makes the walk the selected card reads, so the draw finds it made.
    pub(crate) fn reselect(&mut self, st: &AppState) {
        let entries = self.entries(st);
        let at = |want: &Entry| entries.iter().position(|e| e == want);
        let found = self.sel.as_ref().and_then(|sel| {
            at(sel).or_else(|| {
                let fixer = match sel {
                    Entry::Job(id) => st.jobs.get(*id).map(|j| j.fixer),
                    Entry::Fixer(f) | Entry::Folded { fixer: f, .. } => Some(*f),
                    _ => None,
                }?;
                entries
                    .iter()
                    .position(|e| matches!(e, Entry::Folded { fixer: f, .. } if *f == fixer))
                    .or_else(|| at(&Entry::Fixer(fixer)))
            })
        });
        let ok = |i: usize| entries.get(i).is_some_and(Entry::selectable);
        self.row = match found {
            Some(i) => i,
            None if self.sel.is_none() && !ok(self.row) => entries
                .iter()
                .position(
                    |e| matches!(e, Entry::Job(id) if st.jobs.get(*id).is_some_and(|j| !closed(j))),
                )
                .or_else(|| entries.iter().position(Entry::selectable))
                .unwrap_or(0),
            None if !ok(self.row) => {
                let after = (self.row..entries.len()).find(|i| ok(*i));
                let before = (0..self.row.min(entries.len())).rev().find(|i| ok(*i));
                after.or(before).unwrap_or(0)
            }
            None => self.row,
        };
        self.sel = entries.get(self.row).filter(|e| e.selectable()).cloned();
        if let Some(e) = &self.sel
            && let Some(src) = self.source_of(st, e)
        {
            self.way(st, &src);
        }
    }

    /// Puts the cursor on `want`, for tests and shots.
    #[cfg(test)]
    pub(crate) fn select(&mut self, st: &AppState, want: &Entry) {
        self.row = self
            .entries(st)
            .iter()
            .position(|e| e == want)
            .expect("the row is listed");
        self.sel = Some(want.clone());
    }

    fn step(&mut self, entries: &[Entry], delta: i32, tick: u64) {
        let n = entries.len() as i32;
        let mut i = self.row as i32;
        for _ in 0..n {
            i = (i + delta).rem_euclid(n.max(1));
            if entries[i as usize].selectable() {
                self.row = i as usize;
                self.sel = Some(entries[self.row].clone());
                self.changed = tick;
                return;
            }
        }
    }

    /// The fixer whose closed jobs `← →` fold from this row.
    fn fold_owner(st: &AppState, e: &Entry) -> Option<PersonId> {
        match e {
            Entry::Fixer(f) | Entry::Folded { fixer: f, .. } => Some(*f),
            Entry::Job(id) => st.jobs.get(*id).filter(|j| closed(j)).map(|j| j.fixer),
            _ => None,
        }
    }

    fn fold(&mut self, st: &mut AppState, entries: &[Entry], open: bool) {
        let Some(f) = entries.get(self.row).and_then(|e| Self::fold_owner(st, e)) else {
            return;
        };
        let changed = if open {
            st.deck_jobs_open.insert(f)
        } else {
            st.deck_jobs_open.remove(&f)
        };
        if !changed {
            return;
        }
        // A closed row folded away hands the cursor to its fold.
        let after = self.entries(st);
        if self.sel.as_ref().is_some_and(|s| !after.contains(s)) {
            self.sel = after
                .iter()
                .find(|e| matches!(e, Entry::Folded { fixer, .. } if *fixer == f))
                .cloned();
        }
        self.reselect(st);
        self.changed = st.tick;
    }

    fn track(&self, st: &mut AppState, entries: &[Entry]) {
        let Some(src) = entries.get(self.row).and_then(|e| self.source_of(st, e)) else {
            return;
        };
        if let Err(e) = track::toggle(st, src) {
            st.warn(e);
        }
        track::tick(st);
    }

    /// The MAP listing a row stands for.
    fn listing_of(st: &AppState, e: &Entry) -> Option<ListingId> {
        match e {
            Entry::Mission(m) => Some(ListingId::Story(*m)),
            Entry::Fixer(f) | Entry::Folded { fixer: f, .. } => Some(ListingId::Person(*f)),
            Entry::Job(id) => {
                let j = st.jobs.get(*id)?;
                match j.state {
                    State::Taken => Some(ListingId::Job(*id)),
                    State::Offered => Some(ListingId::Person(j.fixer)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn map(&self, st: &mut AppState, entries: &[Entry]) {
        match entries.get(self.row).and_then(|e| Self::listing_of(st, e)) {
            Some(id) => st.deck_map_to = Some(id),
            None => st.warn("not on the map from here"),
        }
    }

    /// The row's verbs, as a popup beside it.
    fn verbs(&self, st: &AppState, entries: &[Entry]) -> Option<Popup> {
        let e = entries.get(self.row)?;
        let track_choice = |label: String| {
            let src = self.source_of(st, e)?;
            Some(if track::is_tracked(st, &src) {
                Choice::new('t', "Stop tracking", Pick::Track(src)).tail(" ⌖")
            } else {
                Choice::new('t', label, Pick::Track(src))
            })
        };
        let map_choice =
            || Self::listing_of(st, e).map(|id| Choice::new('m', "Show on the MAP", Pick::Map(id)));
        // One mail a card, so none is marked read unseen: the newest unread, else the
        // newest; the tail counts what is left unread.
        let mail_choice = |label: String, mails: Vec<usize>| match next_mail(st, &mails) {
            None => Choice::disabled('r', label).tail(" · no mail"),
            Some(i) => {
                let unread = mails
                    .iter()
                    .filter(|m| st.journal.mails.get(**m).is_some_and(|m| !m.read))
                    .count();
                let tail = if unread > 0 {
                    format!(" · {unread} unread")
                } else {
                    format!(" · {}", mails.len())
                };
                Choice::new('r', label, Pick::Mail(vec![i])).tail(tail)
            }
        };
        let (title, choices): (String, Vec<Option<Choice>>) = match e {
            Entry::Mission(m) => (
                st.book
                    .missions
                    .get(*m)
                    .map(|x| x.title.clone())
                    .unwrap_or_default(),
                vec![track_choice("Track".to_string()), map_choice()],
            ),
            Entry::Fixer(f) | Entry::Folded { fixer: f, .. } => {
                let name = fixer_name(st, *f);
                (
                    name.clone(),
                    vec![
                        track_choice(format!("Track {name}")),
                        map_choice(),
                        Some(mail_choice(
                            format!("Read {name}'s mail"),
                            fixer_mails(st, *f),
                        )),
                    ],
                )
            }
            Entry::Job(id) => {
                let j = st.jobs.get(*id)?;
                let mail = || Some(mail_choice("Read the mail".to_string(), job_mails(st, *id)));
                let choices = match j.state {
                    State::Taken => vec![
                        track_choice("Track".to_string()),
                        map_choice(),
                        mail(),
                        Some(Choice::new('d', "Drop", Pick::DropAsk(*id))),
                    ],
                    State::Offered => vec![
                        track_choice(format!("Track {}", fixer_name(st, j.fixer))),
                        map_choice(),
                    ],
                    _ => vec![mail()],
                };
                (j.title.clone(), choices)
            }
            _ => return None,
        };
        let (list, first) = self.last_list.get();
        let y = list.y + self.row.saturating_sub(first) as u16;
        Some(
            Popup::verbs(&title, choices.into_iter().flatten().collect(), st.tick)
                .anchored(list.right().saturating_sub(12), y),
        )
    }

    /// Handles a key; `Key::Pass` for the deck's own (`q e`, the digits, tab, esc).
    pub(crate) fn key(&mut self, st: &mut AppState, code: KeyCode) -> Key {
        self.reselect(st);
        let entries = self.entries(st);
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.step(&entries, -1, st.tick),
            KeyCode::Down | KeyCode::Char('j') => self.step(&entries, 1, st.tick),
            KeyCode::Right | KeyCode::Char('l') => self.fold(st, &entries, true),
            KeyCode::Left | KeyCode::Char('h') => self.fold(st, &entries, false),
            KeyCode::Char('t' | 'T') => self.track(st, &entries),
            KeyCode::Char('m' | 'M') => self.map(st, &entries),
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some(p) = self.verbs(st, &entries) {
                    return Key::Popup(p);
                }
            }
            _ => return Key::Pass,
        }
        Key::Done
    }

    /// The selected row's card, cut to `height` with its track line kept last; and
    /// whether it has one.
    fn card(
        &self,
        st: &AppState,
        e: Option<&Entry>,
        width: usize,
        height: usize,
    ) -> Vec<Line<'static>> {
        let Some(e) = e else {
            return Vec::new();
        };
        let way = self
            .source_of(st, e)
            .map_or(Way::Nowhere, |s| self.way(st, &s));
        let mut lines = match e {
            Entry::Mission(m) => story_card(st, *m, width, &way),
            Entry::Fixer(f) | Entry::Folded { fixer: f, .. } => {
                card::fixer_card(st, *f, width, &way)
            }
            Entry::Job(id) => card::job_card(st, *id, width, &way),
            _ => Vec::new(),
        };
        let tracked_card = match e {
            Entry::Job(id) => st.jobs.get(*id).is_some_and(|j| !closed(j)),
            Entry::Mission(_) => way != Way::Nowhere,
            _ => true,
        };
        if lines.len() > height && height > 0 {
            let last = lines.pop();
            lines.truncate(height - usize::from(tracked_card));
            if tracked_card && let Some(l) = last {
                lines.push(l);
            }
        }
        lines
    }

    pub(crate) fn draw(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        let list_w = (body.width * 2 / 5)
            .max(28)
            .min(body.width.saturating_sub(10));
        let list = Rect::new(body.x, body.y, list_w, body.height);
        let detail = Rect::new(
            body.x + list_w + 1,
            body.y,
            body.width.saturating_sub(list_w + 1),
            body.height,
        );
        for cy in body.y..body.bottom() {
            buf[(body.x + list_w, cy)].set_char('║').set_fg(RULE);
        }
        let (_, first_was) = self.last_list.get();
        self.last_list.set((list, first_was));
        let entries = self.entries(st);
        // The selected entry wherever this draw's list puts it.
        let row = self
            .sel
            .as_ref()
            .and_then(|s| entries.iter().position(|e| e == s))
            .unwrap_or_else(|| self.row.min(entries.len().saturating_sub(1)));
        let half = list.height.saturating_sub(1) as usize / 2;
        let max_first = entries.len().saturating_sub(list.height as usize);
        let first = row.saturating_sub(half).min(max_first);
        self.last_list.set((list, first));
        for (i, e) in entries
            .iter()
            .enumerate()
            .skip(first)
            .take(list.height as usize)
        {
            let y = list.y + (i - first) as u16;
            self.draw_entry(st, buf, list, y, e, i == row && e.selectable(), st.tick);
        }

        let text = Rect::new(
            detail.x + 1,
            detail.y,
            detail.width.saturating_sub(2),
            detail.height,
        );
        let lines = self.card(
            st,
            entries.get(row),
            text.width as usize,
            text.height as usize,
        );
        let p = st.tick.saturating_sub(self.changed) as f32 / REVEAL_TICKS as f32;
        let cursor_on = (st.tick / 4).is_multiple_of(2);
        for (k, line) in reveal(&lines, p, cursor_on)
            .iter()
            .enumerate()
            .take(text.height as usize)
        {
            let y = text.y + k as u16;
            for x in detail.x..detail.right() {
                buf[(x, y)].set_char(' ').set_bg(PANEL_BG);
            }
            put_spans(buf, text.x, y, &line.spans, text.right());
        }
    }
}

fn put_spans(buf: &mut Buffer, mut x: u16, y: u16, spans: &[Span], right: u16) {
    for s in spans {
        put(buf, x, y, &s.content, s.style, right);
        x = x.saturating_add(s.content.chars().count() as u16);
    }
}

fn hm(m: u16) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Every fixer who has given you a job: those with one in hand (soonest due first),
/// then those with an offer, then the rest, ties by their first job.
fn fixers_in_order(st: &AppState) -> Vec<PersonId> {
    let mut seen: Vec<PersonId> = Vec::new();
    for j in &st.jobs.list {
        if !seen.contains(&j.fixer) {
            seen.push(j.fixer);
        }
    }
    let key = |f: &PersonId| {
        let first = st
            .jobs
            .list
            .iter()
            .find(|j| j.fixer == *f)
            .map_or(0, |j| j.id.0);
        let due = st
            .jobs
            .list
            .iter()
            .filter(|j| j.fixer == *f && j.state == State::Taken)
            .filter_map(|j| j.deadline)
            .min();
        match due {
            Some(d) => (0, d, first),
            None if st.jobs.offered(*f).is_some() => (1, 0, first),
            None => (2, 0, first),
        }
    };
    seen.sort_by_key(key);
    seen
}

/// A fixer's jobs in hand, soonest due first, then their offers.
fn live_jobs(st: &AppState, f: PersonId) -> Vec<&Job> {
    let mut taken: Vec<&Job> = st
        .jobs
        .list
        .iter()
        .filter(|j| j.fixer == f && j.state == State::Taken)
        .collect();
    taken.sort_by_key(|j| (j.deadline, j.id));
    taken.extend(
        st.jobs
            .list
            .iter()
            .filter(|j| j.fixer == f && j.state == State::Offered),
    );
    taken
}

/// A fixer's closed jobs, newest first.
fn closed_jobs(st: &AppState, f: PersonId) -> Vec<&Job> {
    let mut v: Vec<&Job> = st
        .jobs
        .list
        .iter()
        .filter(|j| j.fixer == f && closed(j))
        .collect();
    v.sort_by_key(|j| std::cmp::Reverse((j.closed.map(|c| c.at), j.id)));
    v
}

/// `FIXERS · on their corners till 04:00` while every fixer listed keeps the same
/// hours; shorter on a narrow list; just `FIXERS` otherwise.
fn fixers_header(st: &AppState, fixers: &[PersonId], wide: bool) -> String {
    let shifts: Vec<Option<(u16, u16)>> = fixers.iter().map(|f| shift_of(st, *f)).collect();
    let Some(Some(s)) = shifts.first().copied() else {
        return "FIXERS".to_string();
    };
    if shifts.iter().any(|x| *x != Some(s)) {
        return "FIXERS".to_string();
    }
    let on = in_shift(day_minute(st.net.now(st)), s);
    match (on, wide) {
        (true, true) => format!("FIXERS · on their corners till {}", hm(s.1)),
        (true, false) => format!("FIXERS · till {}", hm(s.1)),
        (false, true) => format!("FIXERS · off their corners till {}", hm(s.0)),
        (false, false) => format!("FIXERS · off till {}", hm(s.0)),
    }
}

/// The parts a row puts against the list's right edge.
type Parts = Vec<(String, Color, bool)>;

fn parts_width(parts: &Parts) -> u16 {
    parts.iter().map(|p| p.0.chars().count() as u16).sum()
}

/// Draws one row: backed across the list, the cursor's bar and `▸` when selected, the
/// mark two cells before the text at `tx` cells in, the text cut to leave room for the
/// right parts.
#[allow(clippy::too_many_arguments)]
fn row_at(
    buf: &mut Buffer,
    list: Rect,
    y: u16,
    selected: bool,
    tick: u64,
    tx: u16,
    mark: Option<(&str, Color)>,
    text: (&str, Color, bool),
    right: Parts,
) {
    let (x0, edge) = (list.x, list.right());
    let bg = if selected { BAR_BG } else { PANEL_BG };
    for x in x0..edge {
        buf[(x, y)].set_char(' ').set_bg(bg);
    }
    if selected {
        put(
            buf,
            x0 + 1,
            y,
            "▸",
            Style::default()
                .fg(CYAN)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
            edge,
        );
    }
    let bold = |s: Style, b: bool| if b { s.add_modifier(Modifier::BOLD) } else { s };
    let tx = x0 + tx;
    if let Some((m, c)) = mark {
        put(
            buf,
            tx - 2,
            y,
            m,
            bold(Style::default().fg(c).bg(bg), true),
            edge,
        );
    }
    let rw = parts_width(&right);
    let room = edge
        .saturating_sub(1)
        .saturating_sub(tx + if rw > 0 { rw + 1 } else { 0 });
    let (t, c, b) = text;
    let fg = if selected { lit(tick) } else { c };
    put(
        buf,
        tx,
        y,
        &cut(t, room as usize),
        bold(Style::default().fg(fg).bg(bg), b || selected),
        edge,
    );
    let mut x = edge.saturating_sub(1 + rw);
    for (s, c, b) in right {
        put(
            buf,
            x,
            y,
            &s,
            bold(Style::default().fg(c).bg(bg), b || selected),
            edge,
        );
        x += s.chars().count() as u16;
    }
}

impl JobsTab {
    #[allow(clippy::too_many_arguments)]
    fn draw_entry(
        &self,
        st: &AppState,
        buf: &mut Buffer,
        list: Rect,
        y: u16,
        e: &Entry,
        selected: bool,
        tick: u64,
    ) {
        let (x0, edge) = (list.x, list.right());
        let narrow = list.width < NARROW;
        match e {
            Entry::Gap => {}
            Entry::Empty => row_at(
                buf,
                list,
                y,
                false,
                tick,
                3,
                None,
                ("no work yet · find a fixer", DIM, false),
                Vec::new(),
            ),
            Entry::Header(label) => {
                for x in x0..edge {
                    buf[(x, y)].set_char(' ').set_bg(PANEL_BG);
                }
                let label = cut(label, edge.saturating_sub(x0 + 5) as usize);
                put(buf, x0 + 3, y, &label, Style::default().fg(DIM), edge);
                let from = x0 + 3 + label.chars().count() as u16 + 1;
                for x in from..edge.saturating_sub(1) {
                    buf[(x, y)].set_char('─').set_fg(RULE);
                }
            }
            Entry::Mission(m) => {
                let done = st
                    .missions
                    .runs
                    .get(*m)
                    .is_some_and(|r| matches!(r.state, RunState::Done { .. }));
                let title = st
                    .book
                    .missions
                    .get(*m)
                    .map(|x| x.title.clone())
                    .unwrap_or_default();
                // The walk is on the next line, under it.
                let right = Vec::new();
                let mark = done.then_some(("✓", card::DONE));
                let tone = if done { card::DONE } else { TEXT };
                row_at(
                    buf,
                    list,
                    y,
                    selected,
                    tick,
                    5,
                    mark,
                    (&title, tone, false),
                    right,
                );
            }
            Entry::Fixer(f) => {
                let standing = st.jobs.standing(*f).clamp(0, 8) as usize;
                let meter = vec![
                    ("▮".repeat(standing), CYAN, false),
                    ("▯".repeat(8 - standing), FAINTER, false),
                ];
                let tracked = track::is_tracked(st, &fixer_source(*f));
                let name = fixer_name(st, *f).to_uppercase();
                let district = district_of_fixer(st, *f);
                let room = edge.saturating_sub(x0 + 3 + 1 + parts_width(&meter) + 1) as usize;
                let lead = if tracked { format!("{name} ⌖") } else { name };
                let full = format!("{lead} · {district}");
                let text = if full.chars().count() <= room {
                    full
                } else {
                    lead.clone()
                };
                row_at(
                    buf,
                    list,
                    y,
                    selected,
                    tick,
                    3,
                    None,
                    (&text, card::WHITE, true),
                    meter,
                );
                // The district after the name is dim, the mark cyan.
                if !selected {
                    let x = x0 + 3 + lead.chars().count() as u16;
                    if text.chars().count() > lead.chars().count() {
                        put(
                            buf,
                            x,
                            y,
                            &text[lead.len()..],
                            Style::default().fg(DIM).remove_modifier(Modifier::BOLD),
                            edge,
                        );
                    }
                    if tracked {
                        put(
                            buf,
                            x - 1,
                            y,
                            "⌖",
                            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                            edge,
                        );
                    }
                }
            }
            Entry::Job(id) => {
                let Some(j) = st.jobs.get(*id) else {
                    return;
                };
                let tracked = job_source(st, *id).is_some_and(|s| track::is_tracked(st, &s));
                let pay = (format!("  ¤{}", j.pay), DIM, false);
                let (mark, tone, right): (Option<(&str, Color)>, Color, Parts) = match j.state {
                    State::Taken => {
                        let left = Jobs::left(j, st.net.now(st)).unwrap_or(0);
                        let clock = if left < WARN_MINUTES { MAGENTA } else { TEXT };
                        let mut r = Vec::new();
                        if hand_in_now(st, *id).is_ok() {
                            r.push(("READY ".to_string(), CYAN, true));
                        }
                        r.push((clock_text(left), clock, false));
                        if !narrow {
                            r.push(pay);
                        }
                        (tracked.then_some(("⌖", CYAN)), TEXT, r)
                    }
                    State::Offered => (None, DIM, vec![("offer".to_string(), AMBER, false), pay]),
                    State::Done => (
                        Some(("✓", card::DONE)),
                        FAINT,
                        vec![("done".to_string(), card::DONE, false)],
                    ),
                    State::Failed => (
                        Some(("✗", MAGENTA)),
                        FAINT,
                        vec![("late".to_string(), MAGENTA, false)],
                    ),
                    State::Dropped => (
                        Some(("–", FAINT)),
                        FAINT,
                        vec![("dropped".to_string(), FAINT, false)],
                    ),
                };
                row_at(
                    buf,
                    list,
                    y,
                    selected,
                    tick,
                    7,
                    mark,
                    (&j.title, tone, false),
                    right,
                );
            }
            Entry::Next(text, tracked) => {
                for x in x0..edge {
                    buf[(x, y)].set_char(' ').set_bg(PANEL_BG);
                }
                let mut x = x0 + 9;
                if *tracked {
                    put(buf, x, y, "⌖ ", Style::default().fg(CYAN), edge);
                    x += 2;
                }
                let room = edge.saturating_sub(1).saturating_sub(x) as usize;
                put(buf, x, y, &cut(text, room), Style::default().fg(DIM), edge);
            }
            Entry::Folded { n, .. } => row_at(
                buf,
                list,
                y,
                selected,
                tick,
                7,
                None,
                (&format!("{n} closed"), FAINT, false),
                vec![("→ unfold".to_string(), FAINTER, false)],
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Screen, ScreenCmd};
    use crate::jobs::testutil::six_jobs;
    use crate::screens::deck::Deck;
    use crate::screens::testutil::{buffer_text, key};
    use crate::track::tests::outside;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// On the street, the drop open, the mock's six jobs, the deck on JOBS.
    fn jobs_deck() -> (AppState, Vec<JobId>, Deck) {
        let mut st = outside();
        crate::missions::begin(&mut st);
        crate::missions::end(&mut st, 0, 0);
        let ids = six_jobs(&mut st);
        let mut d = Deck::new(st.tick);
        d.update(&mut st, &[key(KeyCode::Char('5'))]);
        assert_eq!(d.tab, Tab::Jobs);
        (st, ids, d)
    }

    fn select(d: &mut Deck, st: &AppState, want: &Entry) {
        d.jobs.select(st, want);
    }

    fn selected(d: &Deck, st: &AppState) -> Entry {
        d.jobs.entries(st)[d.jobs.row].clone()
    }

    fn frame(d: &Deck, st: &AppState, w: u16, h: u16) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    #[test]
    fn the_list_is_the_story_then_each_fixer_with_their_jobs() {
        let (st, ids, d) = jobs_deck();
        let e = d.jobs.entries(&st);
        assert_eq!(e[0], Entry::Header("STORY".into()));
        assert_eq!(e[1], Entry::Mission(0));
        assert!(matches!(e[2], Entry::Next(..)), "{e:?}");
        assert!(
            e.iter()
                .any(|x| matches!(x, Entry::Header(h) if h.starts_with("FIXERS · "))),
            "{e:?}"
        );
        let at = |want: &Entry| e.iter().position(|x| x == want).unwrap();
        let fixer = |id: JobId| st.jobs.get(id).unwrap().fixer;
        // In hand first, the offer's fixer after; each live job under its fixer.
        assert!(at(&Entry::Fixer(fixer(ids[0]))) < at(&Entry::Fixer(fixer(ids[2]))));
        assert_eq!(
            at(&Entry::Job(ids[0])),
            at(&Entry::Fixer(fixer(ids[0]))) + 1
        );
        assert!(matches!(e[at(&Entry::Job(ids[0])) + 1], Entry::Next(..)));
        // Closed jobs fold, one row a fixer.
        for id in &ids[3..] {
            assert!(!e.contains(&Entry::Job(*id)), "{id:?} folded");
        }
        let folds = e
            .iter()
            .filter(|x| matches!(x, Entry::Folded { .. }))
            .count();
        assert_eq!(folds, 3, "{e:?}");
        // The soonest due leads: the delivery's 8 hours before the fetch's 12; the
        // cursor starts on it.
        assert!(at(&Entry::Fixer(fixer(ids[1]))) < at(&Entry::Fixer(fixer(ids[0]))));
        assert_eq!(selected(&d, &st), Entry::Job(ids[1]));
    }

    #[test]
    fn the_arrows_fold_a_fixers_closed_jobs() {
        let (mut st, ids, mut d) = jobs_deck();
        let f = st.jobs.get(ids[3]).unwrap().fixer;
        select(&mut d, &st, &Entry::Fixer(f));
        d.update(&mut st, &[key(KeyCode::Right)]);
        assert!(d.jobs.entries(&st).contains(&Entry::Job(ids[3])));
        select(&mut d, &st, &Entry::Job(ids[3]));
        d.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert!(!d.jobs.entries(&st).contains(&Entry::Job(ids[3])));
        assert!(
            matches!(selected(&d, &st), Entry::Folded { fixer, .. } if fixer == f),
            "the cursor lands on the fold"
        );
    }

    #[test]
    fn the_cursor_skips_headers_gaps_and_next_lines() {
        let (mut st, _, mut d) = jobs_deck();
        for _ in 0..30 {
            d.update(&mut st, &[key(KeyCode::Down)]);
            assert!(selected(&d, &st).selectable(), "{:?}", selected(&d, &st));
        }
    }

    #[test]
    fn t_tracks_each_kind_of_row() {
        let (mut st, ids, mut d) = jobs_deck();
        let t = |d: &mut Deck, st: &mut AppState| {
            d.update(st, &[key(KeyCode::Char('t'))]);
        };
        let source = |st: &AppState| st.track.as_ref().map(|x| x.source);
        // A job in hand: its stop; again, it stops.
        select(&mut d, &st, &Entry::Job(ids[0]));
        t(&mut d, &mut st);
        assert!(matches!(source(&st), Some(Source::Job { job, .. }) if job == ids[0]));
        t(&mut d, &mut st);
        assert!(st.track.is_none());
        // An offer: its fixer.
        let f = st.jobs.get(ids[2]).unwrap().fixer;
        select(&mut d, &st, &Entry::Job(ids[2]));
        t(&mut d, &mut st);
        assert!(
            track::is_tracked(&st, &fixer_source(f)),
            "{:?}",
            st.log.latest().map(|e| &e.text)
        );
        // The story: its step.
        select(&mut d, &st, &Entry::Mission(0));
        t(&mut d, &mut st);
        assert!(matches!(
            source(&st),
            Some(Source::Story { mission: 0, .. })
        ));
        // A closed job: nothing changes.
        st.deck_jobs_open.insert(st.jobs.get(ids[3]).unwrap().fixer);
        select(&mut d, &st, &Entry::Job(ids[3]));
        let before = source(&st);
        t(&mut d, &mut st);
        assert_eq!(source(&st), before);
    }

    #[test]
    fn the_tracked_job_is_marked_in_the_list_and_lit_on_its_card() {
        let (mut st, ids, mut d) = jobs_deck();
        select(&mut d, &st, &Entry::Job(ids[0]));
        d.update(&mut st, &[key(KeyCode::Char('t'))]);
        st.tick += 200;
        let text = buffer_text(&frame(&d, &st, 120, 40));
        let title = st.jobs.get(ids[0]).unwrap().title.clone();
        let row = text.lines().find(|l| l.contains(title.as_str())).unwrap();
        assert!(row.contains('⌖'), "{row}");
        assert!(text.contains("⌖ TRACKING"), "{text}");
        assert!(text.contains(" t  stop"), "{text}");
    }

    /// The card's text rect at the last draw.
    fn card_rect(d: &Deck, w: u16) -> (u16, u16, u16, usize) {
        let (list, _) = d.jobs.last_list.get();
        let x0 = list.right() + 2;
        let x1 = w - 2;
        (x0, x1, list.y, list.height as usize)
    }

    #[test]
    fn every_card_line_is_backed_across_the_card() {
        let (mut st, ids, mut d) = jobs_deck();
        st.tick += 200;
        for want in [Entry::Job(ids[0]), Entry::Job(ids[2]), Entry::Mission(0)] {
            select(&mut d, &st, &want);
            for (w, h) in [(120, 40), (100, 30)] {
                let b = frame(&d, &st, w, h);
                let (x0, x1, y0, height) = card_rect(&d, w);
                let card = d.jobs.card(&st, Some(&want), (x1 - x0) as usize, height);
                for k in 0..card.len() as u16 {
                    for x in x0..x1 {
                        let bg = b[(x, y0 + k)].bg;
                        assert!(
                            bg == PANEL_BG || bg == BAR_BG || bg == CYAN,
                            "{want:?} {w}x{h} ({x}, {k}): {bg:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_tab_fits_both_sizes() {
        let (mut st, ids, mut d) = jobs_deck();
        st.tick += 200;
        let f = st.jobs.get(ids[0]).unwrap().fixer;
        for want in [
            Entry::Job(ids[0]),
            Entry::Job(ids[1]),
            Entry::Job(ids[2]),
            Entry::Fixer(f),
            Entry::Mission(0),
        ] {
            select(&mut d, &st, &want);
            for (w, h) in [(120, 40), (100, 30)] {
                let text = buffer_text(&frame(&d, &st, w, h));
                assert!(text.contains(HINT), "{want:?} {w}x{h}\n{text}");
                let (x0, x1, _, height) = card_rect(&d, w);
                let card = d.jobs.card(&st, Some(&want), (x1 - x0) as usize, height);
                assert!(card.len() <= height);
                for l in &card {
                    let n: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
                    assert!(n <= (x1 - x0) as usize, "{want:?} {w}x{h}: {l:?}");
                }
                assert!(
                    text.contains("TRACK"),
                    "the track line kept: {want:?} {w}x{h}\n{text}"
                );
            }
        }
    }

    #[test]
    fn an_empty_tab_says_find_a_fixer() {
        let mut st = outside();
        st.jobs.list.clear();
        st.missions.runs.clear();
        let mut d = Deck::new(st.tick);
        d.update(&mut st, &[key(KeyCode::Char('5'))]);
        assert_eq!(d.jobs.entries(&st), vec![Entry::Empty]);
        let text = buffer_text(&frame(&d, &st, 120, 40));
        assert!(text.contains("no work yet · find a fixer"), "{text}");
    }

    #[test]
    fn m_opens_the_map_on_the_rows_listing() {
        let (mut st, ids, mut d) = jobs_deck();
        select(&mut d, &st, &Entry::Job(ids[0]));
        d.update(&mut st, &[key(KeyCode::Char('m'))]);
        assert_eq!(d.tab, Tab::Map);
        assert!(matches!(st.atlas.picked, Some((ListingId::Job(id), _, _)) if id == ids[0]));
        assert!(st.atlas.cursor.is_some());
        // A closed job is on no map.
        let (mut st, ids, mut d) = jobs_deck();
        st.deck_jobs_open.insert(st.jobs.get(ids[5]).unwrap().fixer);
        select(&mut d, &st, &Entry::Job(ids[5]));
        d.update(&mut st, &[key(KeyCode::Char('m'))]);
        assert_eq!(d.tab, Tab::Jobs);
        assert_eq!(st.log.latest().unwrap().text, "not on the map from here");
    }

    fn verbs_on(d: &mut Deck, st: &AppState, want: &Entry) -> Vec<String> {
        select(d, st, want);
        let entries = d.jobs.entries(st);
        d.jobs.verbs(st, &entries).expect("verbs").labels()
    }

    #[test]
    fn space_opens_each_rows_verbs() {
        let (mut st, ids, mut d) = jobs_deck();
        let name = |st: &AppState, id: JobId| fixer_name(st, st.jobs.get(id).unwrap().fixer);
        assert_eq!(
            verbs_on(&mut d, &st, &Entry::Job(ids[0])),
            ["Track", "Show on the MAP", "Read the mail", "Drop"]
        );
        let n = name(&st, ids[2]);
        assert_eq!(
            verbs_on(&mut d, &st, &Entry::Job(ids[2])),
            [format!("Track {n}"), "Show on the MAP".into()]
        );
        let f = st.jobs.get(ids[0]).unwrap().fixer;
        let n = name(&st, ids[0]);
        assert_eq!(
            verbs_on(&mut d, &st, &Entry::Fixer(f)),
            [
                format!("Track {n}"),
                "Show on the MAP".into(),
                format!("Read {n}'s mail")
            ]
        );
        assert_eq!(
            verbs_on(&mut d, &st, &Entry::Mission(0)),
            ["Track", "Show on the MAP"]
        );
        st.deck_jobs_open.insert(st.jobs.get(ids[3]).unwrap().fixer);
        assert_eq!(
            verbs_on(&mut d, &st, &Entry::Job(ids[3])),
            ["Read the mail"]
        );
        // Tracked, the verb says it stops.
        select(&mut d, &st, &Entry::Job(ids[0]));
        d.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(
            verbs_on(&mut d, &st, &Entry::Job(ids[0]))[0],
            "Stop tracking"
        );
    }

    #[test]
    fn drop_goes_through_the_ask() {
        let (mut st, ids, mut d) = jobs_deck();
        let f = st.jobs.get(ids[0]).unwrap().fixer;
        let before = st.jobs.standing(f);
        select(&mut d, &st, &Entry::Job(ids[0]));
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the verbs")
        };
        let ScreenCmd::Replace { to: mut ask, .. } = p.update(&mut st, &[key(KeyCode::Char('d'))])
        else {
            panic!("d asks first")
        };
        assert_eq!(
            st.jobs.get(ids[0]).unwrap().state,
            State::Taken,
            "not before the yes"
        );
        ask.update(&mut st, &[key(KeyCode::Char('y'))]);
        assert_eq!(st.jobs.get(ids[0]).unwrap().state, State::Dropped);
        assert_eq!(st.jobs.standing(f), before + crate::jobs::LOSS_DROP);
        d.update(&mut st, &[]);
        assert!(selected(&d, &st).selectable());
    }

    #[test]
    fn read_the_mail_shows_the_jobs_own_and_marks_them_read() {
        let (mut st, ids, _) = jobs_deck();
        let f = st.jobs.get(ids[0]).unwrap().fixer;
        let name = fixer_name(&st, f);
        // A story mail from the same name stays the rig's.
        st.journal.mails.push(crate::credentials::Mail {
            from: name,
            subject: "a favour".into(),
            body: vec!["Story.".into()],
            read: false,
            at: 0,
            job: None,
        });
        let story = st.journal.mails.len() - 1;
        let mails = fixer_mails(&st, f);
        assert!(!mails.is_empty() && !mails.contains(&story));
        let card = crate::jobs::ui::mail_card(&mut st, &mails);
        assert!(
            card.iter()
                .any(|l| l.spans.iter().any(|s| s.content.starts_with("From: ")))
        );
        assert!(mails.iter().all(|i| st.journal.mails[*i].read));
        assert!(!st.journal.mails[story].read);
        assert_eq!(job_mails(&st, ids[0]).len(), 1);
    }

    #[test]
    fn reopening_the_deck_finds_the_row_it_was_left_on() {
        let (mut st, ids, mut d) = jobs_deck();
        select(&mut d, &st, &Entry::Job(ids[2]));
        d.update(&mut st, &[]);
        // Whatever the saved index points at now, the entry wins.
        st.deck_rows[Tab::Jobs.index()] = 0;
        let back = Deck::new(st.tick).restore(&st);
        assert_eq!(back.jobs.sel, Some(Entry::Job(ids[2])));
    }

    #[test]
    fn a_dropped_job_leaves_the_cursor_with_its_fixer() {
        let (mut st, ids, mut d) = jobs_deck();
        let f = st.jobs.get(ids[1]).unwrap().fixer;
        select(&mut d, &st, &Entry::Job(ids[1]));
        crate::jobs::ui::drop(&mut st, ids[1]).unwrap();
        d.update(&mut st, &[]);
        assert!(
            matches!(selected(&d, &st), Entry::Folded { fixer, n: 2 } if fixer == f),
            "{:?}",
            selected(&d, &st)
        );
    }

    #[test]
    fn the_mail_is_read_one_at_a_time_newest_unread_first() {
        let (mut st, ids, _) = jobs_deck();
        let f = st.jobs.get(ids[1]).unwrap().fixer;
        let mails = fixer_mails(&st, f);
        assert!(mails.len() >= 2, "a take for each job: {mails:?}");
        let first = next_mail(&st, &mails).unwrap();
        assert_eq!(first, mails[0], "the newest");
        crate::jobs::ui::mail_card(&mut st, &[first]);
        assert_eq!(
            next_mail(&st, &mails),
            Some(mails[1]),
            "then the next unread"
        );
        assert!(!st.journal.mails[mails[1]].read, "unseen stays unread");
        for m in &mails {
            st.journal.mails[*m].read = true;
        }
        assert_eq!(
            next_mail(&st, &mails),
            Some(mails[0]),
            "all read: the newest"
        );
    }

    #[test]
    fn the_hint_names_the_keys() {
        let (st, _, d) = jobs_deck();
        let text = buffer_text(&frame(&d, &st, 100, 30));
        assert!(text.contains(
            "Q/E tabs · ↑↓ select · ←→ fold · space verbs · t track · m map · tab close"
        ));
    }
}
