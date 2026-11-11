//! The HOLO call (spec 8): a card in the map's corner that rings while you walk, then
//! shows the caller's face and types their lines. Not a screen: the explore screen
//! hands it `space` and `esc` and draws it over the map.

use super::{Call, cast, face, person_of, role_name};
use crate::app::AppState;
use crate::ui::chrome::{DIM, MAGENTA, TEXT, put};
use crate::ui::sidebar::wrap_text;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};

/// Clock ticks a call rings before it goes to voicemail: 30 seconds.
pub const RING_TICKS: u64 = 900;
/// Characters a line types out in a second.
pub const TYPE_PER_SECOND: u64 = 60;
/// The clock's ticks in a second.
pub const TICKS_PER_SECOND: u64 = 30;
/// Frame ticks the ringing dot stays lit, then dark.
const BLINK_TICKS: u64 = 15;
const RING_W: u16 = 36;
const RING_H: u16 = 5;
const TALK_W: u16 = 62;
const TALK_H: u16 = 13;
/// The talking card without its face, on a map shorter than `COMPACT_BELOW`.
const COMPACT_H: u16 = 7;
const COMPACT_BELOW: u16 = 28;
/// Columns from the card's left edge to the words: beside the face, or without it.
const TEXT_AT: u16 = 20;
const COMPACT_AT: u16 = 4;
/// The narrowest card worth drawing.
const MIN_W: u16 = 20;

/// Puts a call up, or queues it behind the one that is up. Never ends a call, so it is
/// safe from inside a step's effects.
pub fn ring(st: &mut AppState, call: Call) {
    st.missions.calls.push_back(call);
    next(st);
}

/// The call's beat: voicemail after `RING_TICKS` of ringing, or at once when a fight
/// or a run starts; then the next queued call rings, unless one keeps it quiet.
pub fn tick(st: &mut AppState) {
    if quiet(st) {
        if st.missions.call.is_some() {
            voicemail(st);
        }
        return;
    }
    let now = st.clock_tick();
    if st
        .missions
        .call
        .as_ref()
        .is_some_and(|c| c.answered.is_none() && now.saturating_sub(c.rang) >= RING_TICKS)
    {
        voicemail(st);
    }
    next(st);
}

/// `space` answers, finishes typing the line, or moves on (after the last line it ends
/// the call); `esc` sends a ringing call to voicemail or hangs up a talking one. True
/// when the card took the key; it takes none while the call is quiet.
pub fn key(st: &mut AppState, k: &KeyEvent) -> bool {
    if quiet(st) {
        return false;
    }
    let now = st.clock_tick();
    let Some(call) = st.missions.call.as_mut() else {
        return false;
    };
    match k.code {
        KeyCode::Char(' ') => {
            if call.answered.is_none() {
                call.answered = Some(now);
                call.line = 0;
                call.shown = now;
                call.full = false;
                say(st);
            } else if typed(call, now) < length(call) {
                call.full = true;
            } else if call.line + 1 < call.lines.len() {
                call.line += 1;
                call.shown = now;
                call.full = false;
                say(st);
            } else {
                hang_up(st);
            }
            true
        }
        KeyCode::Esc => {
            if call.answered.is_none() {
                voicemail(st);
            } else {
                hang_up(st);
            }
            true
        }
        _ => false,
    }
}

/// No call rings or talks during a fight, a run or a scene, or while a scene waits to
/// play (a call up as one starts would go to voicemail): `space` and `esc` are theirs.
pub fn quiet(st: &AppState) -> bool {
    busy(st) || st.missions.scene.is_some() || !st.missions.scenes.is_empty()
}

/// A fight is on or the player is in a construct: no call, and no scene either.
pub fn busy(st: &AppState) -> bool {
    st.net.engaged.is_some() || st.player.as_ref().is_some_and(|p| p.place.in_construct())
}

/// The next queued call comes up, ringing from now, when none is up and nothing keeps
/// it quiet.
fn next(st: &mut AppState) {
    if st.missions.call.is_some() || quiet(st) {
        return;
    }
    if let Some(mut call) = st.missions.calls.pop_front() {
        call.rang = st.clock_tick();
        st.missions.call = Some(call);
    }
}

/// The line being said goes in the log as it starts.
fn say(st: &mut AppState) {
    let Some(call) = st.missions.call.as_ref() else {
        return;
    };
    let Some(line) = call.lines.get(call.line) else {
        return;
    };
    let text = format!("`{}` · \u{201c}{line}\u{201d}", role_name(st, &call.from));
    st.story(text);
}

/// The call ends, the rest unsaid; its step moves the story on.
fn hang_up(st: &mut AppState) {
    if let Some(call) = st.missions.call.take() {
        super::end_of(st, call.story);
        next(st);
    }
}

/// The call that is up ends with every line left on the rig.
pub(super) fn voicemail(st: &mut AppState) {
    let Some(call) = st.missions.call.take() else {
        return;
    };
    let mail = crate::credentials::Mail {
        from: role_name(st, &call.from),
        subject: "voicemail".to_string(),
        body: call.lines.clone(),
        read: false,
        at: st.net.now(st),
        job: None,
        story: None,
    };
    // The map's mail mark logs it as it lands.
    st.journal.mails.push(mail);
    super::end_of(st, call.story);
    next(st);
}

/// A line as the card shows it, in quotes.
fn quoted(line: &str) -> String {
    format!("\u{201c}{line}\u{201d}")
}

/// Characters in the line being said, quotes and all.
fn length(call: &Call) -> usize {
    call.lines
        .get(call.line)
        .map_or(0, |l| quoted(l).chars().count())
}

/// Characters of the line typed by clock tick `now`.
fn typed(call: &Call, now: u64) -> usize {
    if call.full {
        return length(call);
    }
    let n = now.saturating_sub(call.shown) * TYPE_PER_SECOND / TICKS_PER_SECOND;
    usize::try_from(n).unwrap_or(usize::MAX).min(length(call))
}

/// The first `typed` characters of a wrapped line, row by row (a wrap ate the space at
/// each break), and where the cursor stands after them as (row, column).
fn reveal(rows: &[String], typed: usize) -> (Vec<String>, (usize, usize)) {
    let mut out = Vec::with_capacity(rows.len());
    let mut left = typed;
    let mut cursor = None;
    for (j, row) in rows.iter().enumerate() {
        let n = row.chars().count();
        let k = left.min(n);
        out.push(row.chars().take(k).collect());
        if cursor.is_none() && (left <= n || j + 1 == rows.len()) {
            cursor = Some((j, k));
        }
        left = left.saturating_sub(n + 1);
    }
    (out, cursor.unwrap_or((0, 0)))
}

/// Where the card sits on `map`: the top-left corner, or the bottom-left when the
/// player's cell would be under it. Ringing it is 36×5; talking `min(62, map width −
/// 2)` wide and 13 rows, or 7 without the face on a map under 28 rows.
pub fn card_rect(map: Rect, talking: bool, player: Option<(u16, u16)>) -> Rect {
    let room = map.width.saturating_sub(2);
    let (w, h) = match (talking, map.height < COMPACT_BELOW) {
        (false, _) => (RING_W.min(room), RING_H),
        (true, true) => (TALK_W.min(room), COMPACT_H),
        (true, false) => (TALK_W.min(room), TALK_H),
    };
    let top = Rect::new(map.x + 1, map.y + 1, w, h);
    match player {
        Some((x, y)) if top.contains(Position::new(x, y)) => Rect::new(
            map.x + 1,
            map.bottom().saturating_sub(1 + h).max(map.y),
            w,
            h,
        ),
        _ => top,
    }
}

/// The call that is up, over the map's corner and off `player`, the `@`'s screen cell.
pub fn draw(buf: &mut Buffer, map: Rect, player: Option<(u16, u16)>, st: &AppState) {
    let Some(call) = st.missions.call.as_ref() else {
        return;
    };
    if quiet(st) {
        return;
    }
    let talking = call.answered.is_some();
    let r = card_rect(map, talking, player);
    if r.width < MIN_W || r.bottom() > map.bottom() || buf.area.intersection(r) != r {
        return;
    }
    face::bracket_frame(buf, r);
    header(buf, r, st, call);
    if talking {
        draw_talking(buf, r, st, call);
    } else {
        draw_ringing(buf, r, st, call);
    }
}

fn bold(c: Color) -> Style {
    Style::default().fg(c).add_modifier(Modifier::BOLD)
}

fn plain(c: Color) -> Style {
    Style::default().fg(c)
}

/// The top row: the dot, who, and the signal at the right.
fn header(buf: &mut Buffer, r: Rect, st: &AppState, call: &Call) {
    let (x, y) = (r.x, r.y + 1);
    let signal = r.right().saturating_sub(7);
    let lit = call.answered.is_some() || (st.tick / BLINK_TICKS).is_multiple_of(2);
    put(
        buf,
        x + 2,
        y,
        if lit { "●" } else { "○" },
        bold(MAGENTA),
        signal,
    );
    let name = role_name(st, &call.from).to_uppercase();
    match call.answered {
        None => put(
            buf,
            x + 4,
            y,
            &format!("INCOMING · {name}"),
            bold(MAGENTA),
            signal.saturating_sub(1),
        ),
        Some(at) => {
            put(
                buf,
                x + 4,
                y,
                &name,
                bold(MAGENTA),
                signal.saturating_sub(1),
            );
            let secs = st.clock_tick().saturating_sub(at) / TICKS_PER_SECOND;
            let after = x + 5 + name.chars().count() as u16;
            let time = format!("· {:02}:{:02}", secs / 60, secs % 60);
            put(buf, after, y, &time, plain(DIM), signal.saturating_sub(1));
        }
    }
    put(buf, signal, y, "▂▄▆█", plain(MAGENTA), r.right() - 2);
}

/// Keys and their labels along a row: `space answer  esc let it ring`.
fn keys(buf: &mut Buffer, x: u16, y: u16, pairs: &[(&str, &str)], right: u16) {
    let mut at = x;
    for (k, label) in pairs {
        put(buf, at, y, k, bold(MAGENTA), right);
        at += k.chars().count() as u16 + 1;
        put(buf, at, y, label, plain(DIM), right);
        at += label.chars().count() as u16 + 2;
    }
}

/// Ringing: who it is and where they work, and the two keys.
fn draw_ringing(buf: &mut Buffer, r: Rect, st: &AppState, call: &Call) {
    let right = r.right() - 2;
    let title = st
        .book
        .cast
        .get(&call.from)
        .map_or_else(String::new, |role| role.title.clone());
    let place = match person_of(st, &call.from) {
        Some(id) => format!("{title} · {}", cast::district_of(st, id)),
        None => title,
    };
    put(buf, r.x + 4, r.y + 2, &place, plain(DIM), right);
    keys(
        buf,
        r.x + 4,
        r.y + 3,
        &[("space", "answer"), ("esc", "let it ring")],
        right,
    );
}

/// Talking: the face at the left (not on the compact card), the lines said so far,
/// the one being said typing with the cursor after it, and the keys.
fn draw_talking(buf: &mut Buffer, r: Rect, st: &AppState, call: &Call) {
    let now = st.clock_tick();
    let compact = r.height < TALK_H;
    let face = st.book.face_of(&call.from);
    let with_face = !compact && face::rows(face) + 3 <= r.height;
    if with_face {
        let seed = face::seed_of(st, &call.from);
        face::draw(buf, r.x + 1, r.y + 2, face, st.tick, seed);
    }
    let (tx, ty) = if compact {
        (r.x + COMPACT_AT, r.y + 2)
    } else {
        (r.x + TEXT_AT, r.y + 3)
    };
    let right = r.right() - 2;
    let width = usize::from(right.saturating_sub(tx + 1)).max(1);
    let n = usize::from(r.height.saturating_sub(if compact { 4 } else { 6 }));
    let mut rows: Vec<String> = Vec::new();
    for said in call.lines.iter().take(call.line) {
        rows.extend(wrap_text(&quoted(said), width));
        rows.push(String::new());
    }
    let first = rows.len();
    let current = call
        .lines
        .get(call.line)
        .map_or_else(String::new, |l| quoted(l));
    let (shown, (cr, cc)) = reveal(&wrap_text(&current, width), typed(call, now));
    rows.extend(shown);
    // Bottom-aligned, older lines scrolled off the top; a line longer than the card
    // shows from its start until the typing runs past the last row. The gap between
    // two lines never opens the card.
    let cursor = first + cr;
    let bottom = rows.len().saturating_sub(n);
    let mut start = if cursor >= bottom {
        bottom
    } else {
        first.max((cursor + 1).saturating_sub(n))
    };
    if start < cursor && rows.get(start).is_some_and(String::is_empty) {
        start += 1;
    }
    for (j, row) in rows.iter().skip(start).take(n).enumerate() {
        put(buf, tx, ty + j as u16, row, plain(TEXT), right);
    }
    if (start..start + n).contains(&cursor) {
        let cx = tx + cc as u16;
        put(
            buf,
            cx,
            ty + (cursor - start) as u16,
            "▌",
            plain(MAGENTA),
            right,
        );
    }
    let last = call.line + 1 >= call.lines.len() && typed(call, now) >= length(call);
    let space = if last { "hang up" } else { "next" };
    keys(
        buf,
        tx,
        r.bottom() - 2,
        &[("space", space), ("esc", "hang up")],
        right,
    );
}

#[cfg(test)]
mod tests {
    use super::super::tests::{begun, on_street, to_the_call};
    use super::super::{FeedView, end};
    use super::*;
    use crate::app::{Screen, ScreenCmd};
    use crate::screens::explore::MapScreen;
    use crate::screens::shots::draw_stack;
    use citygen::{BuildingId, Place, StratumKind};
    use crossterm::event::KeyCode;
    use ratatui::style::Modifier;

    const SPACE: KeyCode = KeyCode::Char(' ');
    const STREET: Place = Place::Stratum(StratumKind::Street);

    /// Seed 8 on the street by Marlo's body, Mezz's call ringing, the welcome's feed
    /// already gone.
    fn up() -> AppState {
        let mut st = begun();
        st.missions.feeds.clear();
        to_the_call(&mut st);
        assert!(st.missions.call.is_some(), "Mezz rings");
        st
    }

    /// Runs the clock on `n` ticks.
    fn ticks(st: &mut AppState, n: u64) {
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + n);
        }
    }

    fn press(st: &mut AppState, code: KeyCode) -> bool {
        key(st, &crate::screens::testutil::key(code))
    }

    /// The map of a 120×40 frame.
    fn map() -> Rect {
        Rect::new(0, 5, 79, 30)
    }

    fn region(buf: &Buffer, r: Rect) -> String {
        let mut s = String::new();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    /// The card drawn on a 120×40 frame's map, its cells as text.
    fn card(st: &AppState) -> String {
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 40));
        draw(&mut buf, map(), None, st);
        let talking = st
            .missions
            .call
            .as_ref()
            .is_some_and(|c| c.answered.is_some());
        region(&buf, card_rect(map(), talking, None))
    }

    fn lines(st: &AppState) -> Vec<String> {
        st.missions.call.as_ref().unwrap().lines.clone()
    }

    fn missing<'a>(line: &'a str, shown: &str) -> Vec<&'a str> {
        line.split_whitespace()
            .filter(|w| !shown.contains(w))
            .collect()
    }

    fn said(st: &AppState, line: &str) -> bool {
        st.log
            .iter()
            .any(|e| e.text == format!("Mezz · \u{201c}{line}\u{201d}"))
    }

    fn voicemail(st: &AppState) -> Option<&crate::credentials::Mail> {
        st.journal.mails.iter().find(|m| m.subject == "voicemail")
    }

    /// The log's lines about the voicemail once a map frame has noticed it land.
    fn voicemail_logged(st: &mut AppState) -> Vec<String> {
        crate::mail_mark::notice(st);
        let lines = st.log.iter().filter(|e| e.text.contains("voicemail"));
        lines.map(|e| e.text.clone()).collect()
    }

    fn engage(st: &mut AppState) {
        crate::net::engage(st, STREET, true);
        assert!(st.net.engaged.is_some());
    }

    #[test]
    fn a_ringing_card_names_the_caller() {
        let st = up();
        let shown = card(&st);
        for w in [
            "INCOMING · MEZZ",
            "▂▄▆█",
            "space",
            "answer",
            "esc",
            "let it ring",
        ] {
            assert!(shown.contains(w), "{w:?} missing:\n{shown}");
        }
        let district = super::super::fill(&st, "{mezz.district}");
        let place: String = format!("fixer · {district}").chars().take(30).collect();
        assert!(shown.contains(&place), "{place:?} missing:\n{shown}");
        assert!(shown.contains('●') || shown.contains('○'), "{shown}");
        assert_eq!(card_rect(map(), false, None), Rect::new(1, 6, 36, 5));
        // Every other key goes to the map.
        let mut st = st;
        assert!(!press(&mut st, KeyCode::Char('h')));
        assert!(!press(&mut st, KeyCode::Enter));
        assert!(st.missions.call.as_ref().unwrap().answered.is_none());
    }

    #[test]
    fn space_answers_and_types_the_first_line() {
        let mut st = up();
        let first = lines(&st)[0].clone();
        assert!(press(&mut st, SPACE));
        let now = st.clock_tick();
        let c = st.missions.call.as_ref().unwrap();
        assert_eq!((c.answered, c.line, c.shown), (Some(now), 0, now));
        assert!(said(&st, &first), "the line is logged as it starts");
        let shown = card(&st);
        assert!(!shown.contains("Choom,"), "nothing typed yet:\n{shown}");
        // Half a second: 30 characters.
        ticks(&mut st, 15);
        let shown = card(&st);
        assert!(
            shown.contains("Choom,") && !shown.contains("looking"),
            "{shown}"
        );
        ticks(&mut st, 60);
        let shown = card(&st);
        assert_eq!(missing(&first, &shown), Vec::<&str>::new(), "{shown}");
        for w in ["MEZZ · 00:02", "▂▄▆█", "space next", "esc hang up", "▌"] {
            assert!(shown.contains(w), "{w:?} missing:\n{shown}");
        }
        // The face, at the left.
        assert!(shown.contains('▀'), "{shown}");
    }

    #[test]
    fn space_steps_through_and_the_last_ends_the_call() {
        let mut st = up();
        let all = lines(&st);
        assert!(press(&mut st, SPACE));
        for (i, line) in all.iter().enumerate() {
            assert_eq!(st.missions.call.as_ref().unwrap().line, i);
            assert!(said(&st, line), "line {i} logged as it starts");
            // The first space finishes the typing.
            assert!(press(&mut st, SPACE));
            assert_eq!(st.missions.call.as_ref().unwrap().line, i);
            let shown = card(&st);
            assert_eq!(missing(line, &shown), Vec::<&str>::new(), "{shown}");
            if i + 1 == all.len() {
                assert!(shown.contains("space hang up"), "{shown}");
            } else {
                assert!(shown.contains("space next"), "{shown}");
            }
            // The second moves on, or ends the call after the last.
            assert!(press(&mut st, SPACE));
        }
        assert!(st.missions.call.is_none());
        assert!(st.missions.runs[0].ended.contains(&3));
        assert_eq!(
            st.missions.runs[0].objectives.last().map(String::as_str),
            Some("Get home · keep the package")
        );
        assert!(voicemail(&st).is_none());
        assert!(!press(&mut st, SPACE), "no call, no key taken");
    }

    #[test]
    fn esc_while_ringing_is_voicemail() {
        let mut st = up();
        let all = lines(&st);
        assert!(press(&mut st, KeyCode::Esc));
        assert!(st.missions.call.is_none());
        let mail = voicemail(&st).expect("a voicemail");
        assert_eq!(mail.from, "Mezz");
        assert_eq!(mail.body, all);
        assert!(!mail.read);
        assert!(all.iter().all(|l| !said(&st, l)), "nothing was said");
        assert_eq!(voicemail_logged(&mut st), ["Mezz · mail · voicemail"]);
        assert!(st.missions.runs[0].ended.contains(&3));
    }

    #[test]
    fn a_call_waits_while_you_are_out_of_the_city() {
        use crate::outside::At;
        let mut st = up();
        crate::outside::ensure(&mut st).unwrap().at = At::Overworld(citygen::Pos::new(1, 1));
        ticks(&mut st, 2 * RING_TICKS);
        crate::clock::run(&mut st, Some(1), crate::clock::PATHS_PER_FRAME);
        assert!(st.missions.call.is_some(), "it waits for you");
        st.outside.as_mut().unwrap().at = At::City;
        crate::clock::run(&mut st, Some(1), crate::clock::PATHS_PER_FRAME);
        assert!(
            st.missions.call.is_some(),
            "and rings in full once you're back"
        );
        ticks(&mut st, RING_TICKS);
        crate::clock::run(&mut st, Some(1), crate::clock::PATHS_PER_FRAME);
        assert!(st.missions.call.is_none(), "then goes to voicemail as ever");
    }

    #[test]
    fn ringing_out_is_voicemail() {
        let mut st = up();
        let all = lines(&st);
        ticks(&mut st, RING_TICKS - 1);
        super::super::tick(&mut st);
        assert!(st.missions.call.is_some(), "still ringing");
        ticks(&mut st, 1);
        super::super::tick(&mut st);
        assert!(st.missions.call.is_none());
        assert_eq!(voicemail(&st).map(|m| m.body.clone()), Some(all));
        assert!(st.missions.runs[0].ended.contains(&3));
    }

    /// `st` saved and opened again in a new state, as LOAD GAME does.
    fn reopened(st: &AppState) -> AppState {
        use crate::runs::snapshot::{after_restore, restore};
        let dir = crate::runs::testutil::Dir::new("call");
        let path = crate::runs::file::write(&dir.0, st).expect("the run writes");
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        assert!(lost.is_empty(), "{lost:?}");
        restore(&mut back, snap);
        after_restore(&mut back, lost);
        back
    }

    #[test]
    fn a_call_saved_ringing_rings_on_reopened() {
        let st = crate::runs::testutil::running(up());
        let mut back = reopened(&st);
        assert_eq!(
            back.missions.call, st.missions.call,
            "the same call, ringing"
        );
        // It rings out from when it first rang, not from the load.
        ticks(&mut back, RING_TICKS - 1);
        super::super::tick(&mut back);
        assert!(back.missions.call.is_some(), "still ringing");
        // Answered, it talks.
        let first = lines(&back)[0].clone();
        assert!(press(&mut back, SPACE));
        assert!(said(&back, &first));
    }

    #[test]
    fn a_call_saved_mid_line_carries_on_reopened() {
        let mut st = crate::runs::testutil::running(up());
        let all = lines(&st);
        assert!(all.len() >= 2, "{all:?}");
        // Answered, the first line typed out, on to the second, its first letters typed.
        assert!(press(&mut st, SPACE));
        assert!(press(&mut st, SPACE));
        assert!(press(&mut st, SPACE));
        ticks(&mut st, 2);
        let up = st.missions.call.clone().unwrap();
        assert_eq!((up.line, up.full), (1, false));
        let mut back = reopened(&st);
        assert_eq!(back.missions.call, Some(up), "the call as it was");
        assert_eq!(card(&back), card(&st), "the card as it was, mid-line");
        assert!(said(&back, &all[1]), "the log has the line it is on");
        // It carries on: the rest of the lines, then the call ends its step.
        for i in 1..all.len() {
            assert_eq!(back.missions.call.as_ref().unwrap().line, i);
            assert!(press(&mut back, SPACE), "line {i} typed out");
            assert!(press(&mut back, SPACE), "line {i} done");
            if i + 1 < all.len() {
                assert!(said(&back, &all[i + 1]));
            }
        }
        assert!(back.missions.call.is_none());
        assert!(back.missions.runs[0].ended.contains(&3));
        assert!(voicemail(&back).is_none());
    }

    #[test]
    fn esc_while_talking_hangs_up() {
        let mut st = up();
        let all = lines(&st);
        assert!(press(&mut st, SPACE));
        ticks(&mut st, 10);
        assert!(press(&mut st, KeyCode::Esc));
        assert!(st.missions.call.is_none());
        assert!(voicemail(&st).is_none(), "the rest is lost");
        assert!(said(&st, &all[0]) && !said(&st, &all[1]));
        assert!(st.missions.runs[0].ended.contains(&3));
        // Answered, it no longer rings out.
        super::super::tick(&mut st);
        assert!(st.missions.call.is_none());
    }

    #[test]
    fn a_second_call_waits_for_the_first() {
        let mut st = up();
        let first = st.missions.call.clone().unwrap();
        let second = Call {
            story: Some(crate::runs::ids::StepRef::new(0, 1)),
            from: "mezz".into(),
            lines: vec!["Second.".into()],
            rang: 0,
            answered: None,
            line: 0,
            shown: 0,
            full: false,
        };
        ring(&mut st, second);
        assert_eq!(st.missions.call.as_ref(), Some(&first));
        assert_eq!(st.missions.calls.len(), 1);
        // A feed opened while it rings shows, closes, and leaves the call up.
        let mut map = MapScreen::walking(&mut st);
        st.missions.feeds.push_back(FeedView {
            story: Some(crate::runs::ids::StepRef::new(0, 0)),
            from: "mezz".into(),
            text: vec!["Feed.".into()],
            close: "ok".into(),
        });
        let ScreenCmd::Push { to: mut feed, .. } = map.update(&mut st, &[]) else {
            panic!("the feed opens");
        };
        assert_eq!(st.missions.call.as_ref(), Some(&first));
        let cmd = feed.update(&mut st, &[crate::screens::testutil::key(SPACE)]);
        assert!(matches!(cmd, ScreenCmd::Pop { .. }));
        assert_eq!(st.missions.call.as_ref(), Some(&first));
        // The first ends; the second rings from now.
        ticks(&mut st, 10);
        assert!(press(&mut st, KeyCode::Esc));
        let up = st.missions.call.as_ref().expect("the second rings");
        assert_eq!(up.lines, vec!["Second.".to_string()]);
        assert_eq!((up.rang, up.answered), (st.clock_tick(), None));
        assert!(st.missions.calls.is_empty());
        assert!(st.missions.runs[0].ended.contains(&3));
        // Ending a step twice changes nothing.
        end(&mut st, 0, 3);
        assert!(st.missions.call.is_some());
    }

    #[test]
    fn the_card_fits_a_100x30_map() {
        let mut st = up();
        let map = MapScreen::walking(&mut st);
        let all = lines(&st);
        assert!(press(&mut st, SPACE));
        for (i, line) in all.iter().enumerate() {
            if i > 0 {
                assert!(press(&mut st, SPACE), "next");
            }
            assert!(press(&mut st, SPACE), "typed");
            let buf = draw_stack(&st, &[&map], (100, 30));
            let m = map.map_area();
            assert_eq!(m.width, 59);
            assert!(m.height < 28, "{m:?}");
            let me = (m.y..m.bottom())
                .flat_map(|y| (m.x..m.right()).map(move |x| (x, y)))
                .find(|&c| buf[c].symbol() == "@" && buf[c].modifier.contains(Modifier::BOLD));
            let r = card_rect(m, true, me);
            assert_eq!((r.width, r.height), (57, 7), "compact");
            assert!(
                r.x > m.x && r.right() < m.right() && r.y > m.y && r.bottom() < m.bottom(),
                "{r:?} inside {m:?}"
            );
            assert_eq!(buf[(r.x, r.y)].symbol(), "▛");
            assert_eq!(buf[(r.right() - 1, r.bottom() - 1)].symbol(), "▟");
            let shown = region(&buf, r);
            assert_eq!(
                missing(line, &shown),
                Vec::<&str>::new(),
                "line {i}:\n{shown}"
            );
            assert!(
                shown.contains("MEZZ") && shown.contains("esc hang up"),
                "{shown}"
            );
            // No face: the rows under the header hold text and keys only.
            for y in r.y + 2..r.bottom() - 1 {
                for x in r.x + 1..r.right() - 1 {
                    let s = buf[(x, y)].symbol();
                    assert!(s != "▀" && s != "▄", "a face cell at ({x}, {y}):\n{shown}");
                }
            }
        }
    }

    #[test]
    fn the_card_moves_off_the_player() {
        let m = map();
        let top = Rect::new(1, 6, 36, 5);
        assert_eq!(card_rect(m, false, None), top);
        assert_eq!(card_rect(m, false, Some((40, 20))), top);
        assert_eq!(card_rect(m, false, Some((3, 8))), Rect::new(1, 29, 36, 5));
        assert_eq!(card_rect(m, true, None), Rect::new(1, 6, 62, 13));
        assert_eq!(card_rect(m, true, Some((62, 18))), Rect::new(1, 21, 62, 13));
        assert_eq!(card_rect(m, true, Some((63, 18))), Rect::new(1, 6, 62, 13));
        let small = Rect::new(0, 5, 59, 20);
        assert_eq!(card_rect(small, true, None), Rect::new(1, 6, 57, 7));
        assert_eq!(
            card_rect(small, true, Some((5, 7))),
            Rect::new(1, 17, 57, 7)
        );
        // Drawn there, and not in the corner.
        let st = up();
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 40));
        draw(&mut buf, m, Some((3, 8)), &st);
        assert_eq!(buf[(1, 29)].symbol(), "▛");
        assert_ne!(buf[(1, 6)].symbol(), "▛");
    }

    #[test]
    fn no_call_rings_in_a_fight() {
        let mut st = up();
        let call = st.missions.call.take().unwrap();
        engage(&mut st);
        assert!(quiet(&st));
        ring(&mut st, call);
        assert!(st.missions.call.is_none(), "it waits");
        assert_eq!(st.missions.calls.len(), 1);
        ticks(&mut st, RING_TICKS + 30);
        super::super::tick(&mut st);
        assert!(st.missions.call.is_none());
        assert_eq!(st.missions.calls.len(), 1);
        assert!(voicemail(&st).is_none(), "its ring timer has not run");
        assert!(!press(&mut st, SPACE));
        // The fight over, it rings from now.
        st.net.engaged = None;
        super::super::tick(&mut st);
        let up = st.missions.call.as_ref().expect("it rings");
        assert_eq!((up.rang, up.answered), (st.clock_tick(), None));
        assert!(st.missions.calls.is_empty());
        // A run is quiet too.
        st.player.as_mut().unwrap().place = Place::Construct(BuildingId(0));
        assert!(quiet(&st));
    }

    #[test]
    fn a_fight_sends_a_live_call_to_voicemail() {
        let mut st = up();
        let all = lines(&st);
        assert!(press(&mut st, SPACE));
        engage(&mut st);
        assert!(!press(&mut st, SPACE), "space is the fight's");
        assert!(!press(&mut st, KeyCode::Esc), "so is esc");
        super::super::tick(&mut st);
        assert!(st.missions.call.is_none());
        assert_eq!(voicemail(&st).map(|m| m.body.clone()), Some(all));
        assert_eq!(voicemail_logged(&mut st), ["Mezz · mail · voicemail"]);
        assert!(st.missions.runs[0].ended.contains(&3));
    }

    #[test]
    fn space_with_a_call_up_does_not_open_the_space_window() {
        let mut st = up();
        // Beside someone, alone: space would open their window.
        let people: Vec<_> = {
            let world = st.world.as_ref().unwrap();
            world
                .people()
                .iter()
                .filter_map(|p| world.pos_of(p.id))
                .collect()
        };
        let stood = people.iter().any(|&pos| {
            pos.neighbors4().any(|n| {
                let city = st.city.as_ref().unwrap();
                let world = st.world.as_ref().unwrap();
                if !city.walkable(StratumKind::Street, n) || world.person_at(n).is_some() {
                    return false;
                }
                on_street(&mut st, n);
                crate::interact::gather(&st, &|_, _| None).len() == 1
            })
        });
        assert!(stood, "someone to stand beside");
        let mut map = MapScreen::walking(&mut st);
        let call = st.missions.call.take();
        let cmd = map.update(&mut st, &[crate::screens::testutil::key(SPACE)]);
        assert!(
            matches!(cmd, ScreenCmd::Push { .. }),
            "space opens the window"
        );
        st.missions.call = call;
        let cmd = map.update(&mut st, &[crate::screens::testutil::key(SPACE)]);
        assert!(matches!(cmd, ScreenCmd::Stay), "the call takes space");
        assert!(st.missions.call.as_ref().unwrap().answered.is_some());
    }

    /// Up and still ringing from `rang`, nothing in the mail.
    fn still_ringing(st: &AppState, rang: u64) {
        let c = st.missions.call.as_ref().expect("the call is up");
        assert_eq!((c.rang, c.answered), (rang, None));
        assert!(voicemail(st).is_none());
    }

    #[test]
    fn look_mode_keeps_its_keys_with_a_call_up() {
        let mut st = up();
        let mut map = MapScreen::walking(&mut st);
        let rang = st.missions.call.as_ref().unwrap().rang;
        let k = crate::screens::testutil::key;
        map.update(&mut st, &[k(KeyCode::Char('x'))]);
        assert!(map.is_looking());
        let cmd = map.update(&mut st, &[k(KeyCode::Esc)]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert!(!map.is_looking(), "esc closes look mode");
        still_ringing(&st, rang);
        // Space in look mode is look's too.
        map.update(&mut st, &[k(KeyCode::Char('x'))]);
        assert!(map.is_looking());
        let _ = map.update(&mut st, &[k(SPACE)]);
        still_ringing(&st, rang);
    }

    #[test]
    fn help_keeps_esc_and_q_with_a_call_up() {
        let mut st = up();
        let mut map = MapScreen::walking(&mut st);
        let rang = st.missions.call.as_ref().unwrap().rang;
        let k = crate::screens::testutil::key;
        for close in [KeyCode::Esc, KeyCode::Char('q')] {
            map.update(&mut st, &[k(KeyCode::Char('?'))]);
            assert!(map.help);
            let cmd = map.update(&mut st, &[k(close)]);
            assert!(matches!(cmd, ScreenCmd::Stay), "{close:?}");
            assert!(!map.help, "{close:?} closes help");
            still_ringing(&st, rang);
        }
        // With nothing of the map's open, esc is the call's.
        let cmd = map.update(&mut st, &[k(KeyCode::Esc)]);
        assert!(matches!(cmd, ScreenCmd::Stay), "no pause menu");
        assert!(st.missions.call.is_none());
        assert!(voicemail(&st).is_some());
    }
}
