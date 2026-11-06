//! Asides (morning news spec §5): bubbles over the map that leave you the keys. The
//! holo set's lines are asides, and so are the city's outlets: who says it, a label,
//! the text typed out, gone on the clock. The explore screen draws them while you walk.

use super::call::{self, TICKS_PER_SECOND, TYPE_PER_SECOND};
use super::film::cut;
use super::{bubbles, overlay, scene};
use crate::app::AppState;
use crate::render::viewport::tile_to_screen;
use crate::ui::sidebar::wrap_text;
use crate::walker::{Player, Visibility};
use citygen::{Place, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use sim::MoverKind;

/// The widest an aside gets.
pub const ASIDE_W: u16 = 28;
/// The most up at once.
pub const MAX_ASIDES: usize = 2;
/// Clock ticks from one of the set's lines ending to the next: 1.5 seconds.
pub const READ_GAP: u64 = 45;

/// Who says an aside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speaker {
    /// A tile on a place: the holo set, an ad.
    Tile(Place, Pos),
    /// Someone on the street, followed as they walk.
    Person(sim::PersonId),
}

/// A bubble up over the map.
#[derive(Clone, Debug, PartialEq)]
pub struct Aside {
    pub speaker: Speaker,
    pub label: String,
    pub colour: Color,
    pub text: String,
    /// The clock tick it went up, and the one it is gone at.
    pub start: u64,
    pub until: u64,
}

/// Puts an aside up from now, for as long as its text `lasts`; past `MAX_ASIDES` the
/// oldest goes.
pub fn say(st: &mut AppState, speaker: Speaker, label: String, colour: Color, text: String) {
    let start = st.clock_tick();
    let until = start + lasts(&text);
    let up = &mut st.missions.asides;
    while up.len() >= MAX_ASIDES {
        up.remove(0);
    }
    up.push(Aside {
        speaker,
        label,
        colour,
        text,
        start,
        until,
    });
}

/// Clock ticks an aside stays up: 2.5 seconds, and a second for every 20 characters.
/// On the clock, so it waits under a menu.
pub fn lasts(text: &str) -> u64 {
    TICKS_PER_SECOND * 5 / 2 + text.chars().count() as u64 * TICKS_PER_SECOND / 20
}

/// Asides may be up: no scene playing or waiting, no rest, no fight or run. Walk mode is
/// the explore screen's to see to.
pub fn live(st: &AppState) -> bool {
    scene::running(st).is_none()
        && st.missions.scenes.is_empty()
        && st.rest.is_none()
        && !call::busy(st)
}

/// The asides whose time is up go; all of them while they are not `live`.
pub fn prune(st: &mut AppState) {
    if !live(st) {
        st.missions.asides.clear();
        return;
    }
    let now = st.clock_tick();
    st.missions.asides.retain(|a| now < a.until);
}

/// Each aside up, oldest first, as a bubble in `map` around its speaker's cell, off your
/// `@` and the people drawn there; none while they are not `live`. One whose speaker is
/// off the map's screen area or (a person) out of your sight, or with no room beside it,
/// is not drawn.
pub fn draw(buf: &mut Buffer, map: Rect, origin: Pos, st: &AppState) {
    let Some(me) = st.player.as_ref().filter(|_| live(st)) else {
        return;
    };
    if st.missions.asides.is_empty() {
        return;
    }
    let now = st.clock_tick();
    let avoid = people(map, origin, st, me);
    let most = ASIDE_W.min(map.width.saturating_sub(2));
    let inner = most.saturating_sub(4).max(1) as usize;
    for a in st.missions.asides.iter().filter(|a| now < a.until) {
        let Some(anchor) = anchor(a.speaker, map, origin, st, me) else {
            continue;
        };
        // Wrapped whole, so the words don't jump about while they type.
        let mut rows = wrap_text(&a.text, inner);
        let typed = now.saturating_sub(a.start) * TYPE_PER_SECOND / TICKS_PER_SECOND;
        let typed = usize::try_from(typed).unwrap_or(usize::MAX);
        let shown = overlay::typed_rows(&a.text, &rows, typed);
        let fit = map.height.saturating_sub(2).max(1) as usize;
        if rows.len() > fit {
            rows.truncate(fit);
            if let Some(last) = rows.last_mut() {
                *last = cut(&format!("{last}…"), inner);
            }
        }
        // As wide as its text and its label need.
        let longest = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let w = (longest + 4).max(a.label.chars().count() + 6) as u16;
        let h = rows.len() as u16 + 2;
        let at = bubbles::place(Some(anchor), map, w.min(most), h, &avoid);
        if at.1.is_none() {
            // `place`'s corner is for a scene; an aside never sits over you.
            continue;
        }
        let typing = typed < a.text.chars().count();
        bubbles::frame(buf, at, &a.label, a.colour, &rows, &shown, typing);
    }
}

/// The speaker's screen cell: a tile on your place, or a person where they are on the
/// street while you are on it and see them.
fn anchor(
    speaker: Speaker,
    map: Rect,
    origin: Pos,
    st: &AppState,
    me: &Player,
) -> Option<(u16, u16)> {
    let at = match speaker {
        Speaker::Tile(place, pos) => (place == me.place).then_some(pos)?,
        Speaker::Person(id) => {
            if !me.place.is_street() {
                return None;
            }
            let at = st.world.as_ref()?.pos_of(id)?;
            (me.vis().get(at) == Some(&Visibility::Visible)).then_some(at)?
        }
    };
    tile_to_screen(map, origin, at)
}

/// The screen cells someone is drawn on: you, the visitors in sight and, on the street,
/// the walkers in sight.
fn people(map: Rect, origin: Pos, st: &AppState, me: &Player) -> Vec<(u16, u16)> {
    let vis = me.vis();
    let seen = |p: Pos| vis.get(p) == Some(&Visibility::Visible);
    let visitors = st
        .missions
        .visitors
        .iter()
        .filter(|v| v.place == me.place)
        .map(|v| v.pos);
    let view = citygen::Rect::new(
        origin.x,
        origin.y,
        i32::from(map.width),
        i32::from(map.height),
    );
    let walkers = st
        .world
        .as_ref()
        .filter(|_| me.place.is_street())
        .map(|w| w.visible(view))
        .unwrap_or_default()
        .into_iter()
        .filter(|m| matches!(m.kind, MoverKind::Walker { .. }))
        .map(|m| m.pos);
    std::iter::once(me.pos)
        .chain(visitors.chain(walkers).filter(|&p| seen(p)))
        .filter_map(|p| tile_to_screen(map, origin, p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::missions::{news, scene};
    use crate::render::map::MapRef;
    use crate::screens::explore::MapScreen;
    use crate::screens::shots::draw_stack;
    use crate::ui::chrome::{DIM, MAGENTA};
    use crate::walker::{Player, Visibility};
    use citygen::StratumKind;

    const STREET: Place = Place::Stratum(StratumKind::Street);
    /// A blank map, the speaker at its middle.
    const AREA: Rect = Rect::new(0, 0, 60, 20);
    const LINE: &str = "This is Newsline. A cook found dead outside his stall in Kowloon Row.";

    /// Runs the clock on `n` ticks.
    fn run(st: &mut AppState, n: u64) {
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + n);
        }
    }

    /// You stand on `pos` of `place`, looking around.
    fn stand(st: &mut AppState, place: Place, pos: Pos) {
        let mut me = Player::at(place, pos);
        let map = MapRef::for_place(
            st.city.as_ref().unwrap(),
            &st.interiors,
            &st.constructs,
            place,
            st.minute_of_day(),
        )
        .unwrap();
        me.update_fov(&map, 0);
        st.player = Some(me);
    }

    /// Seed 8 moved in, you below the holo set seeing it, and the set saying `LINE`.
    fn by_the_set() -> (AppState, Pos) {
        let mut st = state_home();
        let (place, set) = news::set_tile(&st).unwrap();
        stand(&mut st, place, set.offset(0, 1));
        let speaker = Speaker::Tile(place, set);
        say(
            &mut st,
            speaker,
            "VELFORM NEWSLINE".into(),
            MAGENTA,
            LINE.into(),
        );
        assert!(live(&st));
        (st, set)
    }

    /// The asides drawn on a blank `AREA` with `at` in its middle.
    fn drawn(st: &AppState, at: Pos) -> Buffer {
        let mut buf = Buffer::empty(AREA);
        draw(&mut buf, AREA, at.offset(-30, -10), st);
        buf
    }

    fn cells(buf: &Buffer) -> impl Iterator<Item = (u16, u16)> + '_ {
        let a = buf.area;
        (a.y..a.bottom()).flat_map(move |y| (a.x..a.right()).map(move |x| (x, y)))
    }

    /// The bubble's rect, from the cells of its `╭` and `╯`.
    fn bubble(buf: &Buffer) -> Option<Rect> {
        let find = |s: &str| cells(buf).find(|&c| buf[c].symbol() == s);
        let (a, b) = (find("╭")?, find("╯")?);
        Some(Rect::new(a.0, a.1, b.0 - a.0 + 1, b.1 - a.1 + 1))
    }

    fn texts(st: &AppState) -> Vec<&str> {
        st.missions.asides.iter().map(|a| a.text.as_str()).collect()
    }

    #[test]
    fn an_aside_lasts_by_its_length() {
        assert_eq!(lasts(&"x".repeat(40)), 75 + 60);
        assert_eq!(lasts(""), 75);
        assert_eq!(lasts("…ok"), 75 + 4, "characters, not bytes");
    }

    #[test]
    fn at_most_two() {
        let mut st = state_home();
        let at = Speaker::Tile(STREET, Pos::new(0, 0));
        for t in ["one", "two", "three"] {
            say(&mut st, at, "overheard".into(), DIM, t.into());
        }
        assert_eq!(texts(&st), ["two", "three"], "the oldest goes");
        let a = &st.missions.asides[1];
        assert_eq!(
            (a.label.as_str(), a.colour, a.speaker),
            ("overheard", DIM, at)
        );
        assert_eq!(a.start, st.clock_tick());
        assert_eq!(a.until, a.start + lasts("three"));
    }

    #[test]
    fn it_pauses_with_the_clock() {
        let (mut st, _) = by_the_set();
        // Frames go by under a menu, the clock held: still up.
        st.tick += 10_000;
        prune(&mut st);
        assert_eq!(texts(&st), [LINE]);
        run(&mut st, lasts(LINE) - 1);
        prune(&mut st);
        assert_eq!(texts(&st), [LINE]);
        run(&mut st, 1);
        prune(&mut st);
        assert!(st.missions.asides.is_empty(), "its time is up");
    }

    #[test]
    fn none_in_a_scene_asleep_or_a_fight() {
        let (st, set) = by_the_set();
        assert!(bubble(&drawn(&st, set)).is_some(), "up while you walk");
        let queued = |st: &mut AppState| scene::play(st, "waking");
        let playing = |st: &mut AppState| {
            scene::play(st, "waking");
            scene::tick(st);
            assert!(scene::running(st).is_some());
        };
        let asleep = |st: &mut AppState| {
            let until = crate::rest::now(st) + 600;
            crate::rest::start(st, crate::rest::Kind::Sleep, until);
        };
        let fight = |st: &mut AppState| {
            let place = st.player.as_ref().unwrap().place;
            crate::net::engage(st, place, true);
            assert!(st.net.engaged.is_some());
        };
        type Start = fn(&mut AppState);
        let cases: [(&str, Start); 4] = [
            ("a scene waiting", queued),
            ("a scene", playing),
            ("asleep", asleep),
            ("a fight", fight),
        ];
        for (what, start) in cases {
            let (mut st, set) = by_the_set();
            start(&mut st);
            assert!(!live(&st), "{what}");
            assert_eq!(
                drawn(&st, set),
                Buffer::empty(AREA),
                "{what}: nothing drawn"
            );
            prune(&mut st);
            assert!(st.missions.asides.is_empty(), "{what}: all gone");
        }
    }

    #[test]
    fn it_keeps_off_your_at() {
        let mut st = state_home();
        let (place, set) = news::set_tile(&st).unwrap();
        // Below it, above it, where the bubble above would go, and off to the side.
        for (dx, dy) in [(0, 1), (0, -1), (0, -2), (0, -3), (-1, -2), (2, 1)] {
            stand(&mut st, place, set.offset(dx, dy));
            st.missions.asides.clear();
            say(
                &mut st,
                Speaker::Tile(place, set),
                "NEWS".into(),
                MAGENTA,
                LINE.into(),
            );
            for size in [(100, 30), (120, 40)] {
                let map = MapScreen::walking(&mut st);
                let buf = draw_stack(&st, &[&map], size);
                let area = map.map_area();
                let rect = bubble(&buf).unwrap_or_else(|| panic!("a bubble at {dx},{dy} {size:?}"));
                assert_eq!(buf[(rect.x + 3, rect.y)].symbol(), "N", "the aside's");
                assert!(rect.width <= ASIDE_W);
                assert!(
                    area.contains(rect.as_position())
                        && area.contains((rect.right() - 1, rect.bottom() - 1).into()),
                    "{rect:?} in {area:?}"
                );
                let me: Vec<(u16, u16)> = cells(&buf)
                    .filter(|&c| {
                        area.contains(c.into())
                            && buf[c].symbol() == "@"
                            && buf[c].fg == Color::Rgb(255, 255, 255)
                    })
                    .collect();
                assert_eq!(me.len(), 1, "your @ is drawn at {dx},{dy} {size:?}");
                assert!(!rect.contains(me[0].into()), "{rect:?} covers {me:?}");
            }
        }
    }

    #[test]
    fn a_bubble_is_the_scene_s_frame_typed_at_the_call_s_pace() {
        let (mut st, set) = by_the_set();
        // Two characters a clock tick.
        run(&mut st, 5);
        let buf = drawn(&st, set);
        let rect = bubble(&buf).expect("a bubble");
        assert_eq!(rect.width, ASIDE_W);
        let text: String = (rect.y + 1..rect.bottom() - 1)
            .flat_map(|y| (rect.x + 2..rect.right() - 2).map(move |x| (x, y)))
            .map(|c| buf[c].symbol().to_string())
            .collect();
        assert_eq!(text.replace('▌', "").trim_end(), "This is Ne");
        let top: String = (rect.x..rect.right())
            .map(|x| buf[(x, rect.y)].symbol())
            .collect();
        assert!(top.starts_with("╭─ VELFORM NEWSLINE ─"), "{top}");
        assert_eq!(buf[(rect.x, rect.y)].fg, MAGENTA);
        // The tail points at the set from above it.
        assert_eq!(buf[(30, 9)].symbol(), "▾");
        assert_eq!(rect.bottom(), 9);
        // A short line: a narrower bubble, its label still whole.
        st.missions.asides.clear();
        let (place, _) = news::set_tile(&st).unwrap();
        say(
            &mut st,
            Speaker::Tile(place, set),
            "NEWS".into(),
            MAGENTA,
            "Rain.".into(),
        );
        let rect = bubble(&drawn(&st, set)).expect("a bubble");
        assert_eq!((rect.width, rect.height), (10, 3));
    }

    #[test]
    fn a_speaker_off_screen_is_not_drawn() {
        let (mut st, set) = by_the_set();
        let (place, _) = news::set_tile(&st).unwrap();
        // Far off on your place, or on another place.
        for at in [
            Speaker::Tile(place, set.offset(200, 0)),
            Speaker::Tile(STREET, set),
        ] {
            st.missions.asides.clear();
            say(&mut st, at, "NEWS".into(), MAGENTA, LINE.into());
            assert_eq!(drawn(&st, set), Buffer::empty(AREA), "{at:?}");
            // Not drawn, it still runs out its time.
            let until = st.missions.asides[0].until;
            let left = until - 1 - st.clock_tick();
            run(&mut st, left);
            prune(&mut st);
            assert_eq!(st.missions.asides.len(), 1, "{at:?} up till its time");
            run(&mut st, 1);
            prune(&mut st);
            assert!(st.missions.asides.is_empty(), "{at:?} gone at its time");
        }
    }

    #[test]
    fn a_person_is_drawn_on_the_street_while_you_see_them() {
        let mut st = state_home();
        let (place, set) = news::set_tile(&st).unwrap();
        stand(&mut st, place, set.offset(0, 1));
        // Someone on foot you would see from a free tile to one side of them, off where
        // the bubble goes, nobody else on foot above them where it goes.
        let city = st.city.as_ref().unwrap();
        let world = st.world.as_ref().unwrap();
        let room_above = |id: sim::PersonId, at: Pos| {
            world
                .visible(citygen::Rect::new(at.x - 30, at.y - 10, 60, 10))
                .iter()
                .all(|m| !matches!(m.kind, sim::MoverKind::Walker { id: o, .. } if o != id))
        };
        let spots: Vec<(sim::PersonId, Pos, Pos)> = world
            .visible(citygen::Rect::new(0, 0, 100_000, 100_000))
            .into_iter()
            .filter_map(|m| match m.kind {
                sim::MoverKind::Walker { id, .. } => Some((id, m.pos)),
                _ => None,
            })
            .filter(|&(id, at)| room_above(id, at))
            .flat_map(|(id, at)| [2, -2, 3, -3].map(|dx| (id, at, at.offset(dx, 0))))
            .filter(|&(_, _, p)| {
                city.walkable(StratumKind::Street, p) && world.person_at(p).is_none()
            })
            .collect();
        let home = st.player.clone();
        let (id, at, beside) = spots
            .into_iter()
            .find(|&(_, at, beside)| {
                stand(&mut st, STREET, beside);
                st.player.as_ref().unwrap().vis().get(at) == Some(&Visibility::Visible)
            })
            .expect("someone out on the street");
        st.player = home;
        say(
            &mut st,
            Speaker::Person(id),
            "overheard".into(),
            DIM,
            LINE.into(),
        );
        assert_eq!(drawn(&st, at), Buffer::empty(AREA), "you are in your flat");
        // On the street beside them but not looking: not drawn.
        st.player = Some(Player::at(STREET, beside));
        assert_eq!(drawn(&st, at), Buffer::empty(AREA), "out of your sight");
        // Seeing them: over them.
        stand(&mut st, STREET, beside);
        let buf = drawn(&st, at);
        assert!(bubble(&buf).is_some());
        assert_eq!(buf[(30, 9)].symbol(), "▾", "over them");
        // Out of sight again: gone from the map, still up on the clock.
        st.player
            .as_mut()
            .unwrap()
            .set_vis(at, Visibility::Remembered);
        assert_eq!(drawn(&st, at), Buffer::empty(AREA), "out of sight again");
        prune(&mut st);
        assert_eq!(st.missions.asides.len(), 1);
    }
}
