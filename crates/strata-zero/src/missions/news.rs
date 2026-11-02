//! The news (morning news spec §2): the item the story put out last, its texts filled.
//! One is current at a time, for its `fresh` game minutes; every outlet shows it. The
//! frame's news tick has the holo set read it out (§5), watches you walk away from the
//! story's bodies (§7) and ends the item's step.

use super::asides::{self, READ_GAP, Speaker};
use super::{Left, Mark, Where, fill, person_of};
use crate::app::AppState;
use crate::ui::chrome::MAGENTA;
use citygen::Pos;
use game::{Attribute, Slot};

/// The channel's name in a city with no named corp.
const NO_CORP: &str = "CITY";
/// Tiles from a body, on its place, that count as having been at it.
const NEAR: i32 = 6;
/// Tiles from it past which, having been near, you have walked away.
const AWAY: i32 = 12;
/// The eight points, anticlockwise from east as the angle of `(dx, -dy)` turns.
const POINTS: [&str; 8] = [
    "east",
    "north-east",
    "north",
    "north-west",
    "west",
    "south-west",
    "south",
    "south-east",
];
/// SOUGHT's clothes with nothing on your torso.
const NO_CLOTHES: &str = "dark clothes";
/// SOUGHT's time when nobody saw you walk away.
const NO_TIME: &str = "late last night";

/// The news item put out last, its texts filled.
#[derive(Clone, Debug, PartialEq)]
pub struct NewsRun {
    /// The book's id for it.
    pub id: String,
    /// The mission and step that put it out; `usize::MAX` from the console.
    pub mission: usize,
    pub step: usize,
    /// The game minute it was put out.
    pub started: u64,
    /// Game minutes it stays current.
    pub fresh: u64,
    /// `VELFORM NEWSLINE`: one of the city's corps.
    pub channel: String,
    pub headline: String,
    pub report: Vec<String>,
    pub reads: Vec<String>,
    pub crawl: Vec<String>,
    pub overheard: Vec<String>,
    /// The role whose body the camera saw.
    pub about: String,
    /// The holo set in your flat is on.
    pub on: bool,
    /// The step that put it out has ended.
    pub ended: bool,
    /// The next of `reads` the set says.
    pub read: usize,
    /// The clock tick the set's last line ends, or ended if it went early; none before
    /// the first.
    pub read_end: Option<u64>,
}

/// Puts out the book's news item `id` for step `step` of mission `mission`: its texts
/// filled, the set switched on, current in place of any earlier one.
pub fn put_out(st: &mut AppState, id: &str, mission: usize, step: usize) {
    let Some(item) = st.book.news.get(id) else {
        if st.dev {
            st.warn(format!("story · no news {id}"));
        }
        return;
    };
    let all = |texts: &[String]| texts.iter().map(|t| fill(st, t)).collect();
    let run = NewsRun {
        id: id.to_string(),
        mission,
        step,
        started: st.net.now(st),
        fresh: item.fresh,
        channel: st
            .city
            .as_ref()
            .map_or_else(|| format!("{NO_CORP} NEWSLINE"), channel),
        headline: fill(st, &item.headline),
        report: all(&item.report),
        reads: all(&item.reads),
        crawl: all(&item.crawl),
        overheard: all(&item.overheard),
        about: item.about.clone(),
        on: true,
        ended: false,
        read: 0,
        read_end: None,
    };
    // A step still waiting on the item this replaces must not wait for ever: the next
    // tick ends it (ending it here would run the story inside the step putting this out).
    if let Some(old) = st
        .missions
        .news
        .as_ref()
        .filter(|n| !n.ended && n.mission != usize::MAX)
    {
        st.missions.news_ends.push((old.mission, old.step));
    }
    st.missions.news = Some(run);
}

/// The frame's news, in every explore update: the steps a newer item replaced end, the
/// set reads (only with the map in walk mode, `walking`), the walk away from each body
/// is watched, and the item's step ends the first frame you are out of your flat (no
/// home counts as out) or once the item is stale (spec builder rulings).
pub fn tick(st: &mut AppState, walking: bool) {
    for (mission, step) in std::mem::take(&mut st.missions.news_ends) {
        super::end(st, mission, step);
    }
    if walking {
        read(st);
    }
    walk_away(st);
    // The stored item, fresh or not; a console item has no step to end.
    let Some(n) = st.missions.news.as_ref() else {
        return;
    };
    if n.ended || n.mission == usize::MAX {
        return;
    }
    let stale = st.net.now(st).saturating_sub(n.started) >= n.fresh;
    if stale || !super::at(st, &Where::Home) {
        end_step(st);
    }
}

/// The set's lines (spec §5): while the item is fresh, the set on and you in your flat,
/// its `reads` in turn, round and round, each an aside on the set labelled with the
/// channel, `READ_GAP` after the last one ended.
fn read(st: &mut AppState) {
    if !asides::live(st) || !super::at(st, &Where::Home) {
        return;
    }
    let Some((place, pos)) = set_tile(st) else {
        return;
    };
    let speaker = Speaker::Tile(place, pos);
    let now = st.clock_tick();
    let up = st
        .missions
        .asides
        .iter()
        .any(|a| a.speaker == speaker && now < a.until);
    if up || !current(st).is_some_and(|n| n.on && !n.reads.is_empty()) {
        return;
    }
    let Some(n) = st.missions.news.as_mut() else {
        return;
    };
    if let Some(end) = n.read_end.as_mut() {
        // Taken down before its time (a scene, the broadcast): it ended now.
        *end = (*end).min(now);
        if now < *end + READ_GAP {
            return;
        }
    }
    let text = n.reads[n.read % n.reads.len()].clone();
    n.read = (n.read + 1) % n.reads.len();
    n.read_end = Some(now + asides::lasts(&text));
    let channel = n.channel.clone();
    asides::say(st, speaker, channel, MAGENTA, text);
}

/// Each body you have not yet walked away from, on the place you are on: near once you
/// are within `NEAR` tiles of it, in sight or not; after that, the first frame you are
/// past `AWAY` the clock and your heading from it are kept.
fn walk_away(st: &mut AppState) {
    let Some((place, you)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return;
    };
    let at = st.clock();
    for b in st
        .missions
        .bodies
        .iter_mut()
        .filter(|b| b.left.is_none() && b.place == place)
    {
        let d = b.pos.manhattan(you);
        b.near |= d <= NEAR;
        if b.near && d > AWAY {
            b.left = Some(Left {
                at,
                heading: heading(b.pos, you),
            });
        }
    }
}

/// The eight-point heading from `from` to `to`, north up the map (`-y`): each point
/// holds the 45 degrees centred on it.
pub fn heading(from: Pos, to: Pos) -> &'static str {
    let (dx, dy) = (f64::from(to.x - from.x), f64::from(from.y - to.y));
    let sector = (dy.atan2(dx) / std::f64::consts::FRAC_PI_4).round() as i32;
    POINTS[sector.rem_euclid(8) as usize]
}

/// SOUGHT's build from your BODY.
pub fn build_word(body: u8) -> &'static str {
    match body {
        0..=3 => "slight",
        4..=6 => "medium",
        _ => "heavy",
    }
}

/// The SOUGHT card's two lines for the current item, read from you as you are now: who
/// is sought, your build and what you wear; what they carried, and the minute you walked
/// away from the item's body and which way (or `late last night`).
pub fn sought(st: &AppState) -> Option<[String; 2]> {
    let n = current(st)?;
    let item = st.book.news.get(&n.id)?;
    let build = build_word(st.character.attribute(&st.content, Attribute::Body));
    let clothes = st
        .character
        .equipped(Slot::Torso)
        .and_then(|id| st.content.item(id))
        .map_or_else(|| NO_CLOTHES.to_string(), |i| i.name.to_lowercase());
    let when = walked_away(st, &n.about).map_or_else(
        || NO_TIME.to_string(),
        |l| format!("{:02}:{:02}, heading {}", l.at.0, l.at.1, l.heading),
    );
    Some([
        format!("{} · {build} build · {clothes}", fill(st, &item.sought)),
        format!("carrying {} · {when}", fill(st, &item.carrying)),
    ])
}

/// When you walked away from the body of the role `about`, if anyone saw you go.
pub fn walked_away(st: &AppState, about: &str) -> Option<Left> {
    person_of(st, about)
        .and_then(|who| st.missions.bodies.iter().find(|b| b.person == who))
        .and_then(|b| b.left)
}

/// The channel: the first word of the first named corp building's name, in capitals,
/// then `NEWSLINE`.
pub fn channel(city: &citygen::City) -> String {
    let corp = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_corp())
        .find_map(|b| b.name.as_deref()?.split_whitespace().next())
        .map_or_else(|| NO_CORP.to_string(), str::to_uppercase);
    format!("{corp} NEWSLINE")
}

/// Your flat's holo set: the place and tile, or None without a flat.
pub fn set_tile(st: &AppState) -> Option<(citygen::Place, Pos)> {
    super::marks::resolve(st, &Mark::HomeFurniture(citygen::Furniture::Holo), None)
}

/// The item while it is fresh.
pub fn current(st: &AppState) -> Option<&NewsRun> {
    let now = st.net.now(st);
    st.missions
        .news
        .as_ref()
        .filter(|n| now.saturating_sub(n.started) < n.fresh)
}

/// Whether an item is fresh.
pub fn fresh(st: &AppState) -> bool {
    current(st).is_some()
}

/// Switches the set on, while the item is fresh: Watch on the set. The broadcast opens
/// over the map, so the asides go: none is left frozen under it.
pub fn switch_on(st: &mut AppState) {
    st.missions.asides.clear();
    if fresh(st)
        && let Some(n) = st.missions.news.as_mut()
    {
        n.on = true;
    }
}

/// Switches the set off and ends the item's step, once.
pub fn switch_off(st: &mut AppState) {
    if let Some(n) = st.missions.news.as_mut() {
        n.on = false;
    }
    end_step(st);
}

/// Ends the step that put the item out, once; the set stays as it is.
pub fn end_step(st: &mut AppState) {
    let Some(n) = st.missions.news.as_mut().filter(|n| !n.ended) else {
        return;
    };
    n.ended = true;
    let (mission, step) = (n.mission, n.step);
    super::end(st, mission, step);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::missions::asides::{self, READ_GAP, Speaker};
    use crate::missions::{
        Body, Effect, Left, Mark, Mission, RunState, Step, Trigger, marks, scene,
    };
    use crate::ui::chrome::{DIM, MAGENTA};
    use crate::walker::Player;
    use citygen::{Place, Pos, StratumKind};
    use game::{ItemId, Slot};

    /// Seed 8 with its home, the story begun.
    fn begun() -> AppState {
        let mut st = state_home();
        crate::missions::begin(&mut st);
        st
    }

    #[test]
    fn the_set_is_in_your_flat_two_rows_above_the_couch() {
        let st = state_home();
        let (place, pos) = set_tile(&st).expect("a set in the flat");
        assert_eq!(place, st.home.as_ref().unwrap().place());
        let map = crate::render::map::MapRef::for_place(
            st.city.as_ref().unwrap(),
            &st.interiors,
            &st.constructs,
            place,
            0,
        )
        .unwrap();
        use crate::render::map::TileMap;
        let below = map.tile(pos.offset(0, 2)).unwrap();
        assert_eq!(
            below.feature,
            Some(citygen::Feature::Furniture(citygen::Furniture::Couch))
        );
    }

    #[test]
    fn putting_out_fills_the_texts_and_names_the_channel() {
        let mut st = begun();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        let n = current(&st).unwrap();
        assert!(n.on && !n.ended);
        assert_eq!(n.id, "marlo");
        assert!(
            n.headline.starts_with("Courier sought in "),
            "{}",
            n.headline
        );
        for t in n
            .report
            .iter()
            .chain(&n.reads)
            .chain(&n.crawl)
            .chain(&n.overheard)
            .chain([&n.headline])
        {
            assert!(!t.contains('{'), "{t}");
        }
        assert!(n.channel.ends_with(" NEWSLINE") && n.channel == n.channel.to_uppercase());
        assert_eq!(n.channel, channel(st.city.as_ref().unwrap()));
        assert_eq!(n.started, st.net.now(&st));
        assert_eq!(n.fresh, 1440);
        assert!(fresh(&st));
    }

    #[test]
    fn the_channel_is_a_corps_first_word() {
        let mut city = state_home().city.unwrap();
        let corps: Vec<usize> = (0..city.buildings.len())
            .filter(|&i| city.buildings[i].kind.is_corp())
            .collect();
        assert!(corps.len() >= 2, "seed 8 has corps");
        for &i in &corps {
            city.buildings[i].name = None;
        }
        // The first corp with a name; an unnamed one before it and other kinds don't count.
        city.buildings[corps[1]].name = Some("Velform Group Plaza".into());
        city.buildings[corps[1] + 1..]
            .iter_mut()
            .filter(|b| b.kind.is_corp())
            .for_each(|b| b.name = Some("Miracorp Tower".into()));
        assert_eq!(channel(&city), "VELFORM NEWSLINE");
        for &i in &corps {
            city.buildings[i].name = None;
        }
        assert_eq!(channel(&city), "CITY NEWSLINE");
    }

    /// Runs the world on `minutes` game minutes.
    fn advance(st: &mut AppState, minutes: u64) {
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + minutes * sim::TICKS_PER_MINUTE);
        }
    }

    #[test]
    fn news_goes_stale() {
        let mut st = begun();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        let fresh_for = current(&st).unwrap().fresh;
        advance(&mut st, fresh_for - 1);
        assert!(fresh(&st) && current(&st).is_some());
        advance(&mut st, 1);
        assert!(!fresh(&st));
        assert!(current(&st).is_none());
        // Still kept, so its step can be ended.
        assert!(st.missions.news.is_some());
    }

    #[test]
    fn a_new_item_replaces_the_old() {
        let mut st = begun();
        let mut other = st.book.news["marlo"].clone();
        other.headline = "Lift nine out again".into();
        st.book.news.insert("lift".into(), other);
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        switch_off(&mut st);
        put_out(&mut st, "lift", 2, 3);
        let n = current(&st).unwrap();
        assert_eq!(
            (n.id.as_str(), n.headline.as_str(), n.mission, n.step),
            ("lift", "Lift nine out again", 2, 3)
        );
        assert!(n.on && !n.ended);
        // An id the book lacks changes nothing.
        put_out(&mut st, "nope", 0, 0);
        assert_eq!(current(&st).unwrap().id, "lift");
    }

    #[test]
    fn switching_off_ends_the_step_once() {
        let mut st = state_home();
        news_then_after(&mut st);
        crate::missions::begin(&mut st);
        let n = current(&st).expect("the step put it out");
        assert_eq!((n.mission, n.step), (0, 0));
        assert_eq!(st.missions.runs[0].state, RunState::Open);
        switch_off(&mut st);
        switch_off(&mut st);
        assert_eq!(afters(&st), 1);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        let n = st.missions.news.as_ref().unwrap();
        assert!(!n.on && n.ended);
    }

    #[test]
    fn switching_on_only_while_fresh() {
        let mut st = begun();
        switch_on(&mut st);
        assert!(st.missions.news.is_none());
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        switch_off(&mut st);
        switch_on(&mut st);
        let n = current(&st).unwrap();
        assert!(n.on && n.ended, "on again; the step stays ended");
        switch_off(&mut st);
        st.missions.news.as_mut().unwrap().fresh = 0;
        switch_on(&mut st);
        assert!(
            !st.missions.news.as_ref().unwrap().on,
            "stale: nothing to watch"
        );
    }

    #[test]
    fn ending_the_step_leaves_the_set_on() {
        let mut st = begun();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        end_step(&mut st);
        let n = current(&st).unwrap();
        assert!(n.on && n.ended);
    }

    #[test]
    fn putting_out_without_a_home_or_city_does_not_panic() {
        let mut st = AppState::new();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        assert_eq!(current(&st).unwrap().channel, "CITY NEWSLINE");
        switch_off(&mut st);
        assert!(!current(&st).unwrap().on);
        tick(&mut st, true);
    }

    /// A book of one mission: a step putting out the news, and one after it ends.
    pub(crate) fn news_then_after(st: &mut AppState) {
        let step = |id: &str, when: Trigger, then: Effect| Step {
            id: id.into(),
            when,
            then: vec![then],
        };
        st.book.missions = vec![Mission {
            id: "m".into(),
            title: "M".into(),
            from: "mezz".into(),
            steps: vec![
                step("n", Trigger::NewGame, Effect::News("marlo".into())),
                step(
                    "after",
                    Trigger::Ended("n".into()),
                    Effect::Log("after".into()),
                ),
            ],
        }];
        st.book.validate().unwrap();
    }

    /// How often the step after the news step has fired.
    pub(crate) fn afters(st: &AppState) -> usize {
        st.log.iter().filter(|e| e.text == "after").count()
    }

    #[test]
    fn a_replaced_item_s_step_ends_on_the_next_tick() {
        let mut st = state_home();
        news_then_after(&mut st);
        crate::missions::begin(&mut st);
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
        tick(&mut st, true);
        assert_eq!(afters(&st), 0, "in the flat, the step waits");
        // The console's item replaces it: the step is not ended there and then.
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        assert_eq!(afters(&st), 0);
        tick(&mut st, true);
        assert_eq!(afters(&st), 1);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        tick(&mut st, true);
        assert_eq!(afters(&st), 1);
        // A console item has nothing to end, in the flat or out of it.
        st.player = Some(Player::at(STREET, Pos::new(0, 0)));
        tick(&mut st, true);
        assert!(!current(&st).unwrap().ended);
    }

    #[test]
    fn the_explore_update_ends_the_step_out_of_the_flat() {
        let mut st = state_home();
        news_then_after(&mut st);
        crate::missions::begin(&mut st);
        let mut map = crate::screens::explore::MapScreen::walking(&mut st);
        let (place, pos) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        st.player = Some(Player::at(place, pos));
        crate::app::Screen::update(&mut map, &mut st, &[]);
        assert_eq!(afters(&st), 1);
        assert!(current(&st).unwrap().on, "the set stays on");
    }

    const STREET: Place = Place::Stratum(StratumKind::Street);

    #[test]
    fn headings_have_north_up() {
        let o = Pos::new(10, 10);
        assert_eq!(heading(o, Pos::new(10, 0)), "north");
        assert_eq!(heading(o, Pos::new(20, 0)), "north-east");
        assert_eq!(heading(o, Pos::new(20, 10)), "east");
        assert_eq!(heading(o, Pos::new(20, 20)), "south-east");
        assert_eq!(heading(o, Pos::new(10, 20)), "south");
        assert_eq!(heading(o, Pos::new(0, 20)), "south-west");
        assert_eq!(heading(o, Pos::new(0, 10)), "west");
        assert_eq!(heading(o, Pos::new(0, 0)), "north-west");
        // Each point holds the 45 degrees centred on it.
        assert_eq!(heading(o, Pos::new(14, 0)), "north");
        assert_eq!(heading(o, Pos::new(20, 6)), "east");
    }

    #[test]
    fn build_words() {
        assert_eq!(
            [0, 3, 4, 6, 7, 10].map(build_word),
            ["slight", "slight", "medium", "medium", "heavy", "heavy"]
        );
    }

    /// A body on the street at `at`, never walked away from.
    fn a_body(st: &mut AppState, at: Pos) {
        st.missions.bodies.push(Body {
            person: sim::PersonId(0),
            name: "Marlo".into(),
            place: STREET,
            pos: at,
            look: "Face down.".into(),
            near: false,
            left: None,
        });
    }

    /// You stand at `pos` on `place` for a frame.
    fn stand(st: &mut AppState, place: Place, pos: Pos) {
        st.player = Some(Player::at(place, pos));
        tick(st, true);
    }

    /// The console's item out, you in your flat, nothing keeping the asides down.
    fn home_with_the_news() -> AppState {
        let mut st = begun();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        in_the_flat(&mut st);
        assert!(asides::live(&st));
        st
    }

    fn in_the_flat(st: &mut AppState) {
        let (place, pos) = marks::resolve(st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
    }

    /// Runs the clock to tick `at`.
    fn run_to(st: &mut AppState, at: u64) {
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, at);
        }
    }

    /// A frame in walk mode: the asides whose time is up go, then the news tick.
    fn frame(st: &mut AppState) {
        asides::prune(st);
        tick(st, true);
    }

    fn texts(st: &AppState) -> Vec<&str> {
        st.missions.asides.iter().map(|a| a.text.as_str()).collect()
    }

    #[test]
    fn the_set_reads_in_turn_round_and_round() {
        let mut st = home_with_the_news();
        let n = current(&st).unwrap().clone();
        let (place, pos) = set_tile(&st).unwrap();
        assert_eq!(n.reads.len(), 4);
        for want in n.reads.iter().chain(&n.reads[..1]) {
            frame(&mut st);
            let [a] = st.missions.asides.as_slice() else {
                panic!("one line up: {:?}", texts(&st));
            };
            let a = a.clone();
            assert_eq!(a.text, *want);
            assert_eq!((a.label.as_str(), a.colour), (n.channel.as_str(), MAGENTA));
            assert_eq!(a.speaker, Speaker::Tile(place, pos));
            // Up for its time, then quiet for the gap.
            run_to(&mut st, a.until - 1);
            frame(&mut st);
            assert_eq!(st.missions.asides, std::slice::from_ref(&a));
            run_to(&mut st, a.until + READ_GAP - 1);
            frame(&mut st);
            assert!(st.missions.asides.is_empty(), "{:?}", texts(&st));
            run_to(&mut st, a.until + READ_GAP);
        }
    }

    #[test]
    fn the_set_stops_when_off_or_you_leave() {
        let mut st = home_with_the_news();
        let reads = current(&st).unwrap().reads.clone();
        frame(&mut st);
        let until = st.missions.asides[0].until;
        // Off: the line up runs out and none follows.
        switch_off(&mut st);
        run_to(&mut st, until + READ_GAP * 4);
        frame(&mut st);
        assert!(st.missions.asides.is_empty());
        // On again, but out of the flat on its floor, then on the street: nothing.
        switch_on(&mut st);
        let (place, pos) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        st.player = Some(Player::at(place, pos));
        frame(&mut st);
        st.player = Some(Player::at(STREET, Pos::new(0, 0)));
        frame(&mut st);
        assert!(st.missions.asides.is_empty());
        // Home, the map not on foot: nothing; on foot, the next line.
        in_the_flat(&mut st);
        tick(&mut st, false);
        assert!(st.missions.asides.is_empty());
        frame(&mut st);
        assert_eq!(texts(&st), [reads[1].as_str()]);
        // A scene waiting: the line goes and no other comes until it has played.
        let until = st.missions.asides[0].until;
        scene::play(&mut st, "waking");
        frame(&mut st);
        assert!(st.missions.asides.is_empty());
        run_to(&mut st, until + READ_GAP);
        frame(&mut st);
        assert!(st.missions.asides.is_empty());
        st.missions.scenes.clear();
        frame(&mut st);
        let now = st.clock_tick();
        run_to(&mut st, now + READ_GAP);
        frame(&mut st);
        assert_eq!(texts(&st), [reads[2].as_str()]);
    }

    #[test]
    fn watching_clears_the_asides() {
        let mut st = home_with_the_news();
        frame(&mut st);
        let there = Speaker::Tile(STREET, Pos::new(0, 0));
        asides::say(&mut st, there, "overheard".into(), DIM, "Hm.".into());
        assert_eq!(st.missions.asides.len(), 2);
        // The broadcast opens over the map: nothing left frozen around it.
        switch_on(&mut st);
        assert!(st.missions.asides.is_empty());
    }

    #[test]
    fn walking_away_from_a_body_is_recorded_once() {
        let mut st = state_home();
        let to = (22 * 60 + 41 - i64::from(st.minute_of_day())).rem_euclid(24 * 60);
        advance(&mut st, to as u64);
        assert_eq!(st.clock(), (22, 41));
        let at = Pos::new(60, 60);
        a_body(&mut st, at);
        stand(&mut st, STREET, at.offset(3, -3));
        assert!(st.missions.bodies[0].near);
        stand(&mut st, STREET, at.offset(0, -12));
        assert_eq!(st.missions.bodies[0].left, None, "12 tiles is not away yet");
        // Far off on another place: not on its place, so not away.
        let home = st.home.unwrap().place();
        assert_ne!(home, STREET);
        stand(&mut st, home, at.offset(0, -40));
        assert_eq!(st.missions.bodies[0].left, None);
        stand(&mut st, STREET, at.offset(0, -13));
        let left = Some(Left {
            at: (22, 41),
            heading: "north",
        });
        assert_eq!(st.missions.bodies[0].left, left);
        // Walk on, come back and leave the other way later: unchanged.
        advance(&mut st, 5);
        stand(&mut st, STREET, at.offset(1, 0));
        stand(&mut st, STREET, at.offset(30, 0));
        assert_eq!(st.missions.bodies[0].left, left);
    }

    #[test]
    fn far_without_near_records_nothing() {
        let mut st = state_home();
        let at = Pos::new(60, 60);
        a_body(&mut st, at);
        stand(&mut st, STREET, at.offset(7, 0));
        stand(&mut st, STREET, at.offset(40, 0));
        let body = &st.missions.bodies[0];
        assert!(!body.near && body.left.is_none());
        // Near in sight or not: six tiles, out of view, counts.
        stand(&mut st, STREET, at.offset(6, 0));
        assert!(st.missions.bodies[0].near);
    }

    #[test]
    fn sought_reads_you() {
        let mut st = begun();
        // Marlo dies: his body is the item's `about`.
        crate::missions::end(&mut st, 0, 0);
        assert_eq!(sought(&st), None, "no item out");
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        st.character.attributes.body = 5;
        st.character
            .wear(Slot::Torso, ItemId::new("salvage_jacket"), None);
        st.missions.bodies[0].left = Some(Left {
            at: (22, 41),
            heading: "north",
        });
        assert_eq!(
            sought(&st).unwrap(),
            [
                "a courier · medium build · salvage jacket",
                "carrying a taped package · 22:41, heading north",
            ]
        );
        // Nothing worn, never walked away: dark clothes, late last night.
        st.character.take_off(Slot::Torso);
        st.missions.bodies[0].left = None;
        assert_eq!(
            sought(&st).unwrap(),
            [
                "a courier · medium build · dark clothes",
                "carrying a taped package · late last night",
            ]
        );
        // What you wear now: BODY is read as it is.
        st.character.attributes.body = 8;
        assert!(sought(&st).unwrap()[0].contains("heavy build"));
    }
}
