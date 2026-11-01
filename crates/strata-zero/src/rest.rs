//! Resting: the city's clock run fast while you lie down or wait, and what wakes you
//! (wait-and-sleep spec §3 to §5). The window that picks the time and the frame that
//! draws the rest live elsewhere; this is the state and the running of time.

use crate::app::AppState;
use crate::ui::chrome::{self, CYAN, DIM, FAINT, TEXT};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::time::{Duration, Instant};

/// Sleeping heals and darkens the flat; waiting only lets the time go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sleep,
    Wait,
}

/// A rest under way, in game minutes.
#[derive(Clone, Debug, PartialEq)]
pub struct Rest {
    pub kind: Kind,
    /// The minute you lay down.
    pub from: u64,
    /// The minute you meant to wake.
    pub until: u64,
    /// Your HP then: less than this wakes you.
    hp: u32,
    /// The roles standing in your flat then: a new one wakes you.
    visitors: Vec<String>,
}

/// What ended the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Time,
    Esc,
    Fight,
    Hurt,
    Call,
    Scene,
    Feed,
    Visitor,
}

/// Game minutes run in one frame, at most.
pub const MINUTES_PER_FRAME: u64 = 4;
/// A frame stops after the minute that finds this much time spent since it began.
pub const FRAME_BUDGET: Duration = Duration::from_millis(16);
/// Minutes of sleep that heal in full.
pub const NIGHT: u64 = 360;

/// Path requests a minute asleep turns into trips: what a minute awake allows.
const PATHS_PER_MINUTE: usize = crate::clock::PATHS_PER_FRAME * sim::TICKS_PER_MINUTE as usize;

/// The game minute now.
pub fn now(st: &AppState) -> u64 {
    st.net.now(st)
}

/// The time the window offers: a sleep in the lamps' hours lasts to 07:00, by day eight
/// hours; a wait an hour; either rounded up to a ten of the clock.
pub fn default_wake(st: &AppState, kind: Kind) -> u64 {
    let of_day = st.minute_of_day() as u64;
    let ahead = match kind {
        Kind::Sleep if citygen::lighting::lamps_on(st.minute_of_day()) => {
            (7 * 60 + 1440 - of_day) % 1440
        }
        Kind::Sleep => 480,
        Kind::Wait => 60,
    };
    let ten = (10 - (of_day + ahead) % 10) % 10;
    now(st) + ahead + ten
}

/// A wake minute held to the least (ten minutes on) and the most (twenty hours) a rest
/// may last.
pub fn clamp(st: &AppState, wake: u64) -> u64 {
    let now = now(st);
    wake.clamp(now + 10, now + 1200)
}

/// Lies you down or sits you out until `until`, unless something already wakes you.
pub fn start(st: &mut AppState, kind: Kind, until: u64) {
    st.rest = Some(Rest {
        kind,
        from: now(st),
        until,
        hp: st.character.hp,
        visitors: in_the_flat(st),
    });
    // What is there as you lie down is what you lie down to; only a fight, a call, a
    // scene or a feed wake you at once.
    if let Some(why) = cause(st, false) {
        wake(st, why);
    }
}

/// One explore frame of resting.
pub fn frame(st: &mut AppState) {
    frame_within(st, FRAME_BUDGET);
}

/// Runs up to `MINUTES_PER_FRAME` whole minutes, stopping after the minute that finds
/// `budget` spent (always one), or at the first thing that wakes you.
pub fn frame_within(st: &mut AppState, budget: Duration) {
    let began = Instant::now();
    for _ in 0..MINUTES_PER_FRAME {
        if st.rest.is_none() {
            return;
        }
        crate::clock::run(st, Some(sim::TICKS_PER_MINUTE), PATHS_PER_MINUTE);
        if let Some(why) = cause(st, true) {
            wake(st, why);
            return;
        }
        if began.elapsed() >= budget {
            return;
        }
    }
}

/// While resting `esc` wakes you and every other key does nothing. False when not
/// resting.
pub fn key(st: &mut AppState, k: &KeyEvent) -> bool {
    if st.rest.is_none() {
        return false;
    }
    if k.code == KeyCode::Esc {
        wake(st, Why::Esc);
    }
    true
}

/// Ends the rest. A sleep heals HP and FOCUS by the part of a night slept; one line
/// says what happened.
pub fn wake(st: &mut AppState, why: Why) {
    let Some(rest) = st.rest.take() else {
        return;
    };
    let slept = now(st).saturating_sub(rest.from);
    if rest.kind == Kind::Sleep {
        let part = slept.min(NIGHT);
        let rise = |max: u32| (max as u64 * part).div_ceil(NIGHT) as u32;
        let hp_max = st.character.hp_max(&st.content);
        let focus_max = st.character.focus_max(&st.content);
        st.character.hp = (st.character.hp + rise(hp_max)).min(hp_max);
        st.character.focus = (st.character.focus + rise(focus_max)).min(focus_max);
    }
    let (h, m) = st.clock();
    let at = format!("{h:02}:{m:02}");
    let line = match (rest.kind, why) {
        (Kind::Sleep, Why::Time) if slept >= NIGHT => format!("You wake at {at}, rested."),
        (Kind::Sleep, Why::Time | Why::Esc) => format!("You wake at {at}."),
        (Kind::Sleep, Why::Fight) => "A fight wakes you.".to_string(),
        (Kind::Sleep, Why::Hurt) => format!("Pain wakes you at {at}."),
        (Kind::Sleep, Why::Call) => format!("A call wakes you at {at}."),
        (Kind::Sleep, Why::Scene) => format!("Something wakes you at {at}."),
        (Kind::Sleep, Why::Feed) => format!("The HOLO wakes you at {at}."),
        (Kind::Sleep, Why::Visitor) => "Someone in the flat wakes you.".to_string(),
        (Kind::Wait, Why::Time) => format!("You wait until {at}."),
        (Kind::Wait, Why::Esc) => format!("You stop waiting at {at}."),
        (Kind::Wait, Why::Fight) => "A fight ends your wait.".to_string(),
        (Kind::Wait, Why::Hurt) => format!("Pain ends your wait at {at}."),
        (Kind::Wait, Why::Call) => format!("A call ends your wait at {at}."),
        (Kind::Wait, Why::Scene) => format!("Something ends your wait at {at}."),
        (Kind::Wait, Why::Feed) => format!("The HOLO ends your wait at {at}."),
        (Kind::Wait, Why::Visitor) => "Someone in the flat ends your wait.".to_string(),
    };
    st.say(line);
}

/// The roles of the visitors standing in your flat.
fn in_the_flat(st: &AppState) -> Vec<String> {
    let Some((place, rect)) = crate::missions::marks::flat_rect(st) else {
        return Vec::new();
    };
    st.missions
        .visitors
        .iter()
        .filter(|v| v.place == place && rect.contains(v.pos))
        .map(|v| v.role.clone())
        .collect()
}

/// What wakes you, in the order of the spec. As you lie down (`running` false) only a
/// fight, a call, a scene or a feed count.
fn cause(st: &AppState, running: bool) -> Option<Why> {
    let rest = st.rest.as_ref()?;
    let m = &st.missions;
    if st.net.engaged.is_some() {
        Some(Why::Fight)
    } else if running && st.character.hp < rest.hp {
        Some(Why::Hurt)
    } else if m.call.is_some() {
        Some(Why::Call)
    } else if m.scene.is_some() || !m.scenes.is_empty() {
        Some(Why::Scene)
    } else if !m.feeds.is_empty() {
        Some(Why::Feed)
    } else if running && in_the_flat(st).iter().any(|r| !rest.visitors.contains(r)) {
        Some(Why::Visitor)
    } else if running && now(st) >= rest.until {
        Some(Why::Time)
    } else {
        None
    }
}

/// The window's width and height, borders included.
pub const WINDOW_W: u16 = 57;
pub const WINDOW_H: u16 = 12;
/// The strip: 24 hours in half-hour cells.
const CELLS: i64 = 48;
const CELL_MINUTES: i64 = 30;

const TEAL: Color = Color::Rgb(0, 230, 210);
/// The sleep on the strip, and the wake time's own colour.
const SLEEP: Color = Color::Rgb(0, 165, 184);
const PAST: Color = Color::Rgb(35, 38, 45);
const NIGHT_CELL: Color = Color::Rgb(23, 33, 58);
const DAY_CELL: Color = Color::Rgb(61, 51, 24);

/// The window over the map: centred, its bottom row on the map's last, clipped to the
/// map where that is narrower.
pub fn window_rect(map: Rect) -> Rect {
    let (w, h) = (WINDOW_W.min(map.width), WINDOW_H.min(map.height));
    Rect::new(map.x + (map.width - w) / 2, map.bottom() - h, w, h)
}

/// `5h 11m`.
fn hours(minutes: u64) -> String {
    format!("{}h {}m", minutes / 60, minutes % 60)
}

/// A minute of the day as `hh:mm`, rounded to ten minutes when it is the wake time.
fn hhmm(minute_of_day: i64, rounded: bool) -> String {
    let m = if rounded {
        (minute_of_day + 5) / 10 * 10 % 1440
    } else {
        minute_of_day
    };
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Draws the Rest window over the map: choosing a wake time at `until` (`asleep`
/// `None`), or the rest under way since the minute `from` (`Some(from)`), the strip
/// filling up to the time now.
pub fn draw_window(
    buf: &mut Buffer,
    map: Rect,
    st: &AppState,
    kind: Kind,
    until: u64,
    asleep: Option<u64>,
) {
    let r = window_rect(map);
    if r.width < WINDOW_W || r.height < WINDOW_H {
        return;
    }
    chrome::solid(buf, r);
    chrome::frame(buf, r);
    let max = r.right() - 1;
    let title = Style::default().fg(TEAL).add_modifier(Modifier::BOLD);
    let (glyph, name) = match (kind, asleep.is_some()) {
        (Kind::Sleep, false) => ("▤", "Sleep"),
        (Kind::Wait, false) => ("☾", "Wait"),
        (Kind::Sleep, true) => ("▤", "Asleep"),
        (Kind::Wait, true) => ("☾", "Waiting"),
    };
    chrome::put(buf, r.x + 2, r.y, &format!(" {glyph} {name} "), title, max);

    // Game minutes as the strip counts them, and the minute of the day they fall on.
    let (now_m, until_m) = (now(st) as i64, until as i64);
    let from = asleep.map_or(now_m, |f| f as i64);
    let today = st.minute_of_day() as i64;
    let of_day = |m: i64| (today + m - now_m).rem_euclid(1440);
    // The strip starts on the hour two hours before you lay down.
    let start = from - of_day(from) % 60 - 120;
    let x0 = r.x + 4;
    let x_of = |m: i64| x0 + (m - start).div_euclid(CELL_MINUTES).clamp(0, CELLS - 1) as u16;

    for k in (0..24).step_by(2) {
        let hour = (of_day(start) / 60 + k) % 24;
        let style = Style::default().fg(FAINT);
        chrome::put(
            buf,
            x0 + 2 * k as u16,
            r.y + 2,
            &format!("{hour:02}"),
            style,
            max,
        );
    }
    for i in 0..CELLS {
        let m = start + i * CELL_MINUTES;
        let fg = if from < m + CELL_MINUTES && m < until_m {
            if asleep.is_some() && m < now_m {
                CYAN
            } else {
                SLEEP
            }
        } else if m + CELL_MINUTES <= from {
            PAST
        } else if citygen::lighting::lamps_on(of_day(m) as u16) {
            NIGHT_CELL
        } else {
            DAY_CELL
        };
        chrome::put(
            buf,
            x0 + i as u16,
            r.y + 3,
            "█",
            Style::default().fg(fg),
            max,
        );
    }

    // The markers, and under them the labels, a space apart and inside the window.
    let (now_colour, now_label) = if asleep.is_some() {
        (CYAN, hhmm(of_day(now_m), false))
    } else {
        (TEXT, format!("now {}", hhmm(of_day(now_m), false)))
    };
    let wake_label = format!("wake {}", hhmm(of_day(until_m), true));
    let (nl, wl) = (
        now_label.chars().count() as u16,
        wake_label.chars().count() as u16,
    );
    let right = r.right() - 2;
    let mut now_x = x_of(now_m).min(right - nl);
    let mut wake_x = x_of(until_m).max(now_x + nl + 1);
    if wake_x + wl > right {
        wake_x = right - wl;
        now_x = now_x.min(wake_x - nl - 1);
    }
    let bold = |c: Color| Style::default().fg(c).add_modifier(Modifier::BOLD);
    chrome::put(buf, x_of(now_m), r.y + 4, "▴", bold(now_colour), max);
    chrome::put(buf, x_of(until_m), r.y + 4, "▴", bold(SLEEP), max);
    chrome::put(
        buf,
        now_x,
        r.y + 5,
        &now_label,
        Style::default().fg(now_colour),
        max,
    );
    chrome::put(
        buf,
        wake_x,
        r.y + 5,
        &wake_label,
        Style::default().fg(SLEEP),
        max,
    );

    let (said, keys) = match asleep {
        Some(_) => {
            let slept = (now_m - from).max(0) as u64;
            let to_go = (until_m - now_m).max(0) as u64;
            let verb = if kind == Kind::Sleep {
                "slept"
            } else {
                "waited"
            };
            (
                format!("{} {verb} · {} to go", hours(slept), hours(to_go)),
                "esc wakes you".to_string(),
            )
        }
        None => {
            let length = (until_m - now_m).max(0) as u64;
            let said = match kind {
                Kind::Sleep if length >= NIGHT => {
                    format!("{} asleep · you wake rested", hours(length))
                }
                Kind::Sleep => format!("{} asleep · a nap, some HP and FOCUS back", hours(length)),
                Kind::Wait => format!("{} waiting", hours(length)),
            };
            let enter = if kind == Kind::Sleep { "sleep" } else { "wait" };
            (
                said,
                format!("h l an hour · H L ten minutes · enter {enter} · esc"),
            )
        }
    };
    chrome::put(buf, x0, r.y + 7, &said, Style::default().fg(DIM), max);
    chrome::put(buf, x0, r.y + 9, &keys, Style::default().fg(FAINT), max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::jobs::testutil::state_home;
    use crate::missions::marks;
    use crate::missions::{Book, Mark};
    use crate::screens::explore::MapScreen;
    use crate::screens::testutil::key;
    use crate::ui::chrome::CYAN;
    use crossterm::event::KeyCode;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    const LONG: Duration = Duration::from_secs(60);

    fn book(steps: &str) -> Book {
        Book::parse(&format!(
            r#"Story(
                cast: {{
                    "mezz": Role(name: "Mezz", title: "fixer", pick: NearestFixer, face: None),
                }},
                faces: {{
                    "silhouette": Face(rows: ["......dd........", "....dmmmmd......"], eye: (255, 255, 255)),
                }},
                missions: [Mission(id: "m", title: "M", from: "mezz", steps: [
                    Step(id: "start", when: NewGame, then: [Objective("M")]),
                    {steps}
                ])],
            )"#
        ))
        .unwrap()
    }

    /// A home state with a story that does `then` after `minutes`.
    fn storied(minutes: u64, then: &str) -> AppState {
        let mut st = state_home();
        st.book = book(&format!(
            r#"Step(id: "later", when: After(minutes: {minutes}), then: [{then}]),"#
        ));
        crate::missions::begin(&mut st);
        st
    }

    /// Moves the world on to the next `h:m` of the day, the story left alone.
    fn at(st: &mut AppState, h: u64, m: u64) {
        let want = h * 60 + m;
        let ahead = (want + 1440 - st.minute_of_day() as u64) % 1440;
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(city, world.tick() + ahead * sim::TICKS_PER_MINUTE);
        assert_eq!(st.minute_of_day() as u64, want);
    }

    /// The game minute of the next `h:m`.
    fn minutes(st: &AppState, h: u64, m: u64) -> u64 {
        now(st) + (h * 60 + m + 1440 - st.minute_of_day() as u64) % 1440
    }

    fn last(st: &AppState) -> String {
        st.log.iter().last().unwrap().text.clone()
    }

    /// Frames until you wake.
    fn sleep_through(st: &mut AppState) {
        for _ in 0..400 {
            if st.rest.is_none() {
                return;
            }
            frame_within(st, LONG);
        }
        panic!("still resting");
    }

    #[test]
    fn the_default_wake_time() {
        let mut st = state_home();
        at(&mut st, 22, 1);
        assert_eq!(default_wake(&st, Kind::Sleep), minutes(&st, 7, 0));
        assert_eq!(default_wake(&st, Kind::Wait), minutes(&st, 23, 10));
        at(&mut st, 23, 50);
        assert_eq!(default_wake(&st, Kind::Sleep), minutes(&st, 7, 0));
        at(&mut st, 5, 30);
        assert_eq!(default_wake(&st, Kind::Sleep), minutes(&st, 7, 0));
        at(&mut st, 6, 30);
        assert_eq!(default_wake(&st, Kind::Sleep), minutes(&st, 14, 30));
        at(&mut st, 13, 0);
        assert_eq!(default_wake(&st, Kind::Sleep), minutes(&st, 21, 0));
    }

    #[test]
    fn the_wake_time_clamps() {
        let st = state_home();
        let n = now(&st);
        assert_eq!(clamp(&st, n + 1), n + 10);
        assert_eq!(clamp(&st, n), n + 10);
        assert_eq!(clamp(&st, n + 300), n + 300);
        assert_eq!(clamp(&st, n + 2000), n + 1200);
    }

    #[test]
    fn time_passes_four_minutes_a_frame() {
        let mut st = state_home();
        let from = now(&st);
        start(&mut st, Kind::Sleep, from + 30);
        frame_within(&mut st, LONG);
        assert_eq!(now(&st), from + 4);
        for _ in 0..7 {
            frame_within(&mut st, LONG);
        }
        assert_eq!(now(&st), from + 30);
        assert!(st.rest.is_none());
        let (h, m) = st.clock();
        assert_eq!(last(&st), format!("You wake at {h:02}:{m:02}."));
    }

    #[test]
    fn a_spent_budget_still_runs_a_minute() {
        let mut st = state_home();
        let from = now(&st);
        start(&mut st, Kind::Sleep, from + 30);
        frame_within(&mut st, Duration::ZERO);
        assert_eq!(now(&st), from + 1);
        assert!(st.rest.is_some());
    }

    #[test]
    fn a_night_wakes_rested() {
        let mut st = state_home();
        at(&mut st, 22, 0);
        let until = default_wake(&st, Kind::Sleep);
        start(&mut st, Kind::Sleep, until);
        sleep_through(&mut st);
        assert_eq!(last(&st), "You wake at 07:00, rested.");
    }

    #[test]
    fn an_after_step_fires_on_the_same_minute_asleep() {
        // The game minute the step's line is written at, awake and asleep.
        let logged = |st: &AppState| st.log.iter().any(|e| e.text == "the step");
        let mut awake = storied(30, r#"Log("the step")"#);
        let from = now(&awake);
        while !logged(&awake) {
            crate::clock::run(
                &mut awake,
                Some(sim::TICKS_PER_MINUTE),
                crate::clock::PATHS_PER_FRAME,
            );
        }
        let mut asleep = storied(30, r#"Log("the step")"#);
        start(&mut asleep, Kind::Sleep, from + 120);
        let mut seen = None;
        while asleep.rest.is_some() {
            frame_within(&mut asleep, Duration::ZERO);
            if seen.is_none() && logged(&asleep) {
                seen = Some(now(&asleep));
            }
        }
        assert_eq!(seen, Some(now(&awake)));
        assert_eq!(seen, Some(from + 30));
    }

    #[test]
    fn a_call_wakes_you_ringing() {
        let mut st = storied(10, r#"Call(from: "mezz", lines: ["Pick up."])"#);
        let from = now(&st);
        start(&mut st, Kind::Sleep, from + 120);
        sleep_through(&mut st);
        assert_eq!(now(&st), from + 10);
        let call = st.missions.call.as_ref().expect("it is ringing");
        assert_eq!(call.answered, None);
        assert!(st.journal.mails.iter().all(|m| m.subject != "voicemail"));
        let (h, m) = st.clock();
        assert_eq!(last(&st), format!("A call wakes you at {h:02}:{m:02}."));
    }

    #[test]
    fn a_call_up_as_you_lie_down_wakes_you_at_once() {
        let mut st = storied(0, r#"Call(from: "mezz", lines: ["Pick up."])"#);
        crate::missions::tick(&mut st);
        assert!(st.missions.call.is_some());
        let from = st.clock_tick();
        let until = now(&st) + 120;
        start(&mut st, Kind::Sleep, until);
        assert!(st.rest.is_none());
        assert_eq!(st.clock_tick(), from);
        assert!(last(&st).starts_with("A call wakes you"));
    }

    #[test]
    fn each_cause_wakes_you() {
        let lie_down = || {
            let mut st = state_home();
            let until = now(&st) + 600;
            start(&mut st, Kind::Sleep, until);
            assert!(st.rest.is_some());
            st
        };
        // A fight.
        let mut st = lie_down();
        let street = citygen::Place::Stratum(citygen::StratumKind::Street);
        crate::net::engage(&mut st, street, true);
        frame_within(&mut st, LONG);
        assert!(st.rest.is_none());
        assert_eq!(last(&st), "A fight wakes you.");
        // Hurt.
        let mut st = lie_down();
        st.character.hp -= 1;
        frame_within(&mut st, LONG);
        assert!(st.rest.is_none());
        assert!(last(&st).starts_with("Pain wakes you at "));
        // A scene queued.
        let mut st = crate::missions::scene::tests::queued(
            crate::missions::scene::tests::SLOW,
            r#"Say(who: "castellan", text: "One.")"#,
        );
        let scene = st.missions.scenes.pop_front().unwrap();
        let until = now(&st) + 600;
        start(&mut st, Kind::Sleep, until);
        assert!(st.rest.is_some());
        st.missions.scenes.push_back(scene);
        frame_within(&mut st, LONG);
        assert!(st.rest.is_none());
        assert!(last(&st).starts_with("Something wakes you at "));
        // A feed queued.
        let mut st = lie_down();
        st.missions.feeds.push_back(crate::missions::FeedView {
            mission: 0,
            step: 0,
            from: "mezz".into(),
            text: vec!["hi".into()],
            close: "ok".into(),
        });
        frame_within(&mut st, LONG);
        assert!(st.rest.is_none());
        assert!(last(&st).starts_with("The HOLO wakes you at "));
        // A visitor in the flat.
        let mut st = lie_down();
        let there = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        crate::missions::visitors::arrive(&mut st, "seren", there);
        frame_within(&mut st, LONG);
        assert!(st.rest.is_none());
        assert_eq!(last(&st), "Someone in the flat wakes you.");
    }

    #[test]
    fn a_visitor_already_there_does_not_wake_you() {
        let mut st = state_home();
        let there = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        crate::missions::visitors::arrive(&mut st, "seren", there);
        let until = now(&st) + 600;
        start(&mut st, Kind::Sleep, until);
        frame_within(&mut st, LONG);
        assert!(st.rest.is_some());
    }

    #[test]
    fn keys_do_nothing_but_esc() {
        let mut st = state_home();
        let mut s = MapScreen::walking(&mut st);
        let until = now(&st) + 600;
        start(&mut st, Kind::Sleep, until);
        let spot = |st: &AppState| st.player.as_ref().map(|p| (p.place, p.pos));
        let at = spot(&st);
        for code in [
            KeyCode::Char('l'),
            KeyCode::Up,
            KeyCode::Tab,
            KeyCode::Char('1'),
            KeyCode::Char('q'),
            KeyCode::Char(' '),
        ] {
            s.update(&mut st, &[key(code)]);
            assert_eq!(spot(&st), at);
            assert_eq!(st.rest.as_ref().map(|r| r.until), Some(until));
        }
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(st.rest.is_none());
        assert!(last(&st).starts_with("You wake at "));
    }

    #[test]
    fn sleep_heals_and_wait_does_not() {
        let rested = |kind: Kind, hours: u64| {
            let mut st = state_home();
            st.character.hp = 10;
            st.character.focus = 10;
            let until = now(&st) + hours * 60;
            start(&mut st, kind, until);
            sleep_through(&mut st);
            st
        };
        let st = rested(Kind::Sleep, 6);
        assert_eq!(st.character.hp, st.character.hp_max(&st.content));
        assert_eq!(st.character.focus, st.character.focus_max(&st.content));
        let st = rested(Kind::Sleep, 3);
        let (hp_max, focus_max) = (
            st.character.hp_max(&st.content) as u64,
            st.character.focus_max(&st.content) as u64,
        );
        assert_eq!(st.character.hp as u64, 10 + (hp_max * 180).div_ceil(360));
        assert_eq!(
            st.character.focus as u64,
            10 + (focus_max * 180).div_ceil(360)
        );
        let st = rested(Kind::Wait, 3);
        assert_eq!((st.character.hp, st.character.focus), (10, 10));
    }

    #[test]
    fn a_new_city_forgets_the_rest() {
        let mut st = state_home();
        let until = now(&st) + 600;
        start(&mut st, Kind::Sleep, until);
        assert!(st.rest.is_some());
        st.enter_city(
            citygen::Generator::run(citygen::GenParams {
                seed: 9,
                size: citygen::SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        assert!(st.rest.is_none());
    }

    /// The window drawn at `until` over the map of a 120×40 screen; `asleep` is the
    /// minute you lay down, when it is drawn as the asleep frame.
    fn drawn(st: &AppState, kind: Kind, until: u64, asleep: Option<u64>) -> (Buffer, Rect) {
        let area = Rect::new(0, 0, 120, 40);
        let map = crate::screens::explore::layout(area, false).map;
        let mut buf = Buffer::empty(area);
        draw_window(&mut buf, map, st, kind, until, asleep);
        (buf, window_rect(map))
    }

    fn row(buf: &Buffer, r: Rect, y: u16) -> String {
        (r.x..r.right())
            .map(|x| buf[(x, r.y + y)].symbol())
            .collect()
    }

    /// The colour of strip cell `i`.
    fn cell(buf: &Buffer, r: Rect, i: u16) -> Color {
        buf[(r.x + 4 + i, r.y + 3)].fg
    }

    const PAST: Color = Color::Rgb(35, 38, 45);
    const SLEEPING: Color = Color::Rgb(0, 165, 184);

    #[test]
    fn the_strip_at_night() {
        let mut st = state_home();
        at(&mut st, 22, 1);
        let (buf, r) = drawn(&st, Kind::Sleep, minutes(&st, 7, 0), None);
        assert!(row(&buf, r, 0).contains("Sleep"));
        assert!(
            row(&buf, r, 2)
                .trim_start_matches(['│', ' '])
                .starts_with("20  22  00"),
            "{}",
            row(&buf, r, 2)
        );
        for i in 0..4 {
            assert_eq!(cell(&buf, r, i), PAST, "cell {i}");
        }
        for i in 4..22 {
            assert_eq!(cell(&buf, r, i), SLEEPING, "cell {i}");
        }
        assert_eq!(cell(&buf, r, 22), Color::Rgb(61, 51, 24), "07:00 is day");
        for i in 44..48 {
            assert_eq!(cell(&buf, r, i), Color::Rgb(23, 33, 58), "cell {i}");
        }
        for i in [4, 22] {
            assert_eq!(buf[(r.x + 4 + i, r.y + 4)].symbol(), "▴");
        }
        assert!(row(&buf, r, 5).contains("now 22:01"));
        assert!(row(&buf, r, 5).contains("wake 07:00"));
        assert_eq!(
            row(&buf, r, 7).trim_matches(|c| c == ' ' || c == '│'),
            "8h 59m asleep · you wake rested"
        );
        assert!(row(&buf, r, 9).contains("enter sleep"));
    }

    #[test]
    fn a_nap_and_a_wait_read_their_own_lines() {
        let mut st = state_home();
        at(&mut st, 22, 1);
        let (buf, r) = drawn(&st, Kind::Sleep, now(&st) + 120, None);
        assert!(row(&buf, r, 7).contains("2h 0m asleep · a nap, some HP and FOCUS back"));
        let (buf, r) = drawn(&st, Kind::Wait, now(&st) + 60, None);
        assert!(row(&buf, r, 0).contains("Wait"));
        assert!(row(&buf, r, 7).contains("1h 0m waiting"));
        assert!(row(&buf, r, 9).contains("enter wait"));
    }

    #[test]
    fn asleep_the_strip_fills() {
        let mut st = state_home();
        at(&mut st, 22, 1);
        let (from, until) = (now(&st), minutes(&st, 7, 0));
        at(&mut st, 3, 12);
        let (buf, r) = drawn(&st, Kind::Sleep, until, Some(from));
        assert!(row(&buf, r, 0).contains("Asleep"));
        for i in 4..=14 {
            assert_eq!(cell(&buf, r, i), CYAN, "cell {i}");
        }
        for i in 15..22 {
            assert_eq!(cell(&buf, r, i), SLEEPING, "cell {i}");
        }
        assert_eq!(buf[(r.x + 4 + 14, r.y + 4)].symbol(), "▴");
        assert_eq!(buf[(r.x + 4 + 14, r.y + 4)].fg, CYAN);
        assert_eq!(buf[(r.x + 4 + 14, r.y + 5)].fg, CYAN);
        assert!(row(&buf, r, 5).contains("03:12"));
        assert!(row(&buf, r, 7).contains("5h 11m slept · 3h 48m to go"));
        assert!(row(&buf, r, 9).contains("esc wakes you"));
        let (buf, r) = drawn(&st, Kind::Wait, until, Some(from));
        assert!(row(&buf, r, 0).contains("Waiting"));
        assert!(row(&buf, r, 7).contains("5h 11m waited · 3h 48m to go"));
    }

    #[test]
    fn close_labels_do_not_touch() {
        let mut st = state_home();
        at(&mut st, 22, 1);
        let (buf, r) = drawn(&st, Kind::Wait, minutes(&st, 22, 20), None);
        assert!(
            row(&buf, r, 5).contains("now 22:01 wake 22:20"),
            "{}",
            row(&buf, r, 5)
        );
    }

    #[test]
    fn labels_stay_in_the_window_at_the_strips_end() {
        let mut st = state_home();
        at(&mut st, 22, 1);
        let from = now(&st);
        let until = from + 1200;
        let (buf, r) = drawn(&st, Kind::Sleep, until, None);
        assert!(
            row(&buf, r, 5)
                .trim_end_matches([' ', '│'])
                .ends_with("wake 18:00")
        );
        assert_eq!(buf[(r.right() - 1, r.y + 5)].symbol(), "│");
        at(&mut st, 17, 59);
        let (buf, r) = drawn(&st, Kind::Sleep, until, Some(from));
        let text = row(&buf, r, 5);
        assert!(text.contains("17:59 wake 18:00"), "{text}");
    }

    #[test]
    fn the_window_fits_100x30_and_120x40() {
        for (w, h) in [(100, 30), (120, 40)] {
            let map = crate::screens::explore::layout(Rect::new(0, 0, w, h), false).map;
            let r = window_rect(map);
            assert_eq!((r.width, r.height), (57, 12), "{w}x{h}");
            assert!(r.x >= map.x && r.right() <= map.right(), "{w}x{h}");
            assert_eq!(r.bottom(), map.bottom(), "{w}x{h}");
        }
        let narrow = Rect::new(2, 5, 40, 20);
        let r = window_rect(narrow);
        assert!(r.x >= narrow.x && r.right() <= narrow.right());
    }
}
