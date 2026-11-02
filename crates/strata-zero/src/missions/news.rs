//! The news (morning news spec §2): the item the story put out last, its texts filled.
//! One is current at a time, for its `fresh` game minutes; every outlet shows it. The
//! frame's news tick has the holo set read it out (§5) and the city's outlets show it
//! (§6), watches you walk away from the story's bodies (§7) and ends the item's step.

use super::asides::{self, READ_GAP, Speaker};
use super::{Left, Mark, Where, fill, person_of};
use crate::app::AppState;
use crate::ui::chrome::{DIM, MAGENTA};
use crate::walker::Visibility;
use citygen::{BuildingId, BuildingKind, Feature, Place, Pos, Rect, StratumKind};
use game::{Attribute, ItemId, Slot};
use sim::{MoverKind, PersonId};
use std::collections::BTreeSet;

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
/// Tiles from you (Chebyshev) someone you overhear stands at most.
pub const HEARD_WITHIN: i32 = 6;
/// Game minutes from one overheard line to the next, at least.
pub const HEARD_GAP: u64 = 12;
/// Past that, the chance each game minute someone is close: 1 in this.
pub const HEARD_ONE_IN: u64 = 4;
/// Tiles from you (Chebyshev) a holo ad that cuts in is at most.
pub const AD_WITHIN: i32 = 10;
/// Game minutes from one cut to the next, at least.
pub const AD_GAP: u64 = 8;
/// Past that, the chance each game minute an ad is in sight: 1 in this.
pub const AD_ONE_IN: u64 = 3;
/// Clock ticks a cut lasts: 3 seconds.
pub const CUT_TICKS: u64 = 3 * super::call::TICKS_PER_SECOND;
/// Each outlet's roll, apart from the other's.
const HEARD_SALT: u64 = 0x4EA2D;
const AD_SALT: u64 = 0xAD5;
/// The overheard lines' label, and a cut's.
const HEARD_LABEL: &str = "overheard";
const CUT_LABEL: &str = "NEWSLINE";
/// The kinds of building with a screen over the counter.
const SCREENED: [BuildingKind; 3] = [
    BuildingKind::Bar,
    BuildingKind::Club,
    BuildingKind::NoodleBar,
];

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
    /// The next of `overheard` the city says.
    pub heard: usize,
    /// The game minute of the last overheard line, and of the last cut.
    pub heard_at: Option<u64>,
    pub cut_at: Option<u64>,
    /// The holo ad cutting in, and the clock tick the cut ends.
    pub cut: Option<(Pos, u64)>,
    /// The last game minute the outlets rolled.
    pub rolled: Option<u64>,
    /// The bars, clubs and noodle bars whose screen has shown it.
    pub screened: BTreeSet<BuildingId>,
    /// The building you were in last frame.
    pub inside: Option<BuildingId>,
}

impl NewsRun {
    /// The headline in capitals, as the outlets show it.
    pub fn headline_caps(&self) -> String {
        self.headline.to_uppercase()
    }
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
        heard: 0,
        heard_at: None,
        cut_at: None,
        cut: None,
        rolled: None,
        screened: BTreeSet::new(),
        inside: None,
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
/// set reads and the city's outlets show it (only with the map in walk mode,
/// `walking`), the walk away from each body is watched, and the item's step ends the
/// first frame you are out of your flat (no home counts as out) or once the item is
/// stale (spec builder rulings).
pub fn tick(st: &mut AppState, walking: bool) {
    for (mission, step) in std::mem::take(&mut st.missions.news_ends) {
        super::end(st, mission, step);
    }
    if walking {
        read(st);
        outlets(st);
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

/// The city's outlets (spec §6), while the item is fresh and asides may be up: the
/// screen over a counter each frame; once a game minute, on the street, someone you
/// overhear and a holo ad cutting in.
fn outlets(st: &mut AppState) {
    if !asides::live(st) || !fresh(st) {
        return;
    }
    screens(st);
    let now = st.net.now(st);
    let Some(n) = st
        .missions
        .news
        .as_mut()
        .filter(|n| n.rolled.is_none_or(|m| m < now))
    else {
        return;
    };
    n.rolled = Some(now);
    let (Some(you), Some(seed)) = (
        st.player
            .as_ref()
            .filter(|p| p.place.is_street())
            .map(|p| p.pos),
        st.city.as_ref().map(|c| c.seed),
    ) else {
        return;
    };
    overhear(st, seed, now, you);
    cut(st, seed, now, you);
}

/// A 1 in `one_in` chance for an outlet (`salt`) in a game minute of a city (`seed`):
/// the same each time it is asked, so a test can pin it.
pub fn roll(seed: u64, minute: u64, salt: u64, one_in: u64) -> bool {
    use citygen::rng::hash64;
    hash64(hash64(seed, salt), minute).is_multiple_of(one_in.max(1))
}

/// Someone overheard: `HEARD_GAP` on from the last and the minute's roll up, the item's
/// next line, in order and round again, from whoever `overheard_by` finds.
fn overhear(st: &mut AppState, seed: u64, now: u64, you: Pos) {
    let Some(n) = st.missions.news.as_ref() else {
        return;
    };
    if n.overheard.is_empty()
        || n.heard_at.is_some_and(|m| now < m + HEARD_GAP)
        || !roll(seed, now, HEARD_SALT, HEARD_ONE_IN)
    {
        return;
    }
    let Some(who) = overheard_by(st, you) else {
        return;
    };
    let Some(n) = st.missions.news.as_mut() else {
        return;
    };
    let text = n.overheard[n.heard % n.overheard.len()].clone();
    n.heard = (n.heard + 1) % n.overheard.len();
    n.heard_at = Some(now);
    asides::say(st, Speaker::Person(who), HEARD_LABEL.into(), DIM, text);
}

/// Who you overhear: someone on foot (`visible` leaves out anyone in a car) you see
/// within `HEARD_WITHIN`, alive and cast in no story role; the nearest, ties to the
/// lowest id.
fn overheard_by(st: &AppState, you: Pos) -> Option<PersonId> {
    let world = st.world.as_ref()?;
    let vis = st.player.as_ref()?.vis();
    world
        .visible(Rect::new(you.x, you.y, 1, 1).grow(HEARD_WITHIN))
        .into_iter()
        .filter_map(|m| match m.kind {
            MoverKind::Walker { id, .. } => Some((id, m.pos)),
            _ => None,
        })
        .filter(|&(id, p)| {
            p.chebyshev(you) <= HEARD_WITHIN
                && vis.get(p) == Some(&Visibility::Visible)
                && !world.is_dead(id)
                && !st.missions.cast.values().any(|&c| c == id)
        })
        .min_by_key(|&(id, p)| (p.chebyshev(you), id))
        .map(|(id, _)| id)
}

/// A holo ad cutting in: `AD_GAP` on from the last cut and the minute's roll up, the
/// nearest ad in sight shows the headline for `CUT_TICKS`, an aside over it.
fn cut(st: &mut AppState, seed: u64, now: u64, you: Pos) {
    if st
        .missions
        .news
        .as_ref()
        .is_none_or(|n| n.cut_at.is_some_and(|m| now < m + AD_GAP))
        || !roll(seed, now, AD_SALT, AD_ONE_IN)
    {
        return;
    }
    let Some(ad) = ad_in_sight(st, you) else {
        return;
    };
    let until = st.clock_tick() + CUT_TICKS;
    let Some(n) = st.missions.news.as_mut() else {
        return;
    };
    n.cut_at = Some(now);
    n.cut = Some((ad, until));
    let text = format!("▸ {}", n.headline_caps());
    let speaker = Speaker::Tile(Place::Stratum(StratumKind::Street), ad);
    asides::say(st, speaker, CUT_LABEL.into(), MAGENTA, text);
    // The ad's aside ends with the cut.
    if let Some(a) = st.missions.asides.last_mut() {
        a.until = until;
    }
}

/// The holo ad on the street you see within `AD_WITHIN`, the nearest; ties to the top,
/// then the left.
fn ad_in_sight(st: &AppState, you: Pos) -> Option<Pos> {
    let city = st.city.as_ref()?;
    let vis = st.player.as_ref()?.vis();
    Rect::new(you.x, you.y, 1, 1)
        .grow(AD_WITHIN)
        .iter()
        .filter(|&p| {
            city.tile(StratumKind::Street, p)
                .is_some_and(|t| t.feature == Some(Feature::HoloAd))
                && vis.get(p) == Some(&Visibility::Visible)
        })
        .min_by_key(|p| (p.chebyshev(you), p.y, p.x))
}

/// The street tile of the holo ad cutting into the item now, while it is fresh.
pub fn cut_in(st: &AppState) -> Option<Pos> {
    let now = st.clock_tick();
    current(st)?
        .cut
        .filter(|&(_, until)| now < until)
        .map(|(ad, _)| ad)
}

/// The screens over counters: the frame you come into a bar, a club or a noodle bar
/// whose screen has not shown the item, the log reads its headline.
fn screens(st: &mut AppState) {
    let here = building_here(st);
    let screened = here.is_some_and(|b| {
        st.city
            .as_ref()
            .and_then(|c| c.building_opt(b))
            .is_some_and(|b| SCREENED.contains(&b.kind))
    });
    let Some(n) = st.missions.news.as_mut().filter(|n| n.inside != here) else {
        return;
    };
    n.inside = here;
    if let Some(b) = here.filter(|_| screened)
        && n.screened.insert(b)
    {
        let line = format!("Over the counter, a screen: {}.", n.headline_caps());
        st.say(line);
    }
}

/// The building you are in: a closed one's floor, or on a city map a walk-in's
/// footprint (a closed building's tiles out there are its walls and roof).
fn building_here(st: &AppState) -> Option<BuildingId> {
    let p = st.player.as_ref()?;
    match p.place {
        Place::Floor { building, .. } => Some(building),
        Place::Stratum(StratumKind::Street) => {
            let city = st.city.as_ref()?;
            city.tile(StratumKind::Street, p.pos)?
                .building
                .filter(|&b| city.building_opt(b).is_some_and(|b| !b.kind.is_closed()))
        }
        _ => None,
    }
}

/// Each body you have not yet walked away from, on the place you are on: near once you
/// are within `NEAR` tiles of it, in sight or not; after that, the first frame you are
/// past `AWAY` the clock, your heading from it and what you wear on your torso are kept.
fn walk_away(st: &mut AppState) {
    let Some((place, you)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return;
    };
    let at = st.clock();
    let worn = st.character.equipped(Slot::Torso).cloned();
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
                worn: worn.clone(),
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

/// The SOUGHT card's two lines for the current item: who is sought, your build as it is
/// now and the jacket the camera saw (what you wear now, until you walk away); what they
/// carried, and the minute you walked away from the item's body and which way (or
/// `late last night`).
pub fn sought(st: &AppState) -> Option<[String; 2]> {
    let n = current(st)?;
    let item = st.book.news.get(&n.id)?;
    let build = build_word(st.character.attribute(&st.content, Attribute::Body));
    let left = walked_away(st, &n.about);
    let worn = match &left {
        Some(l) => l.worn.as_ref(),
        None => st.character.equipped(Slot::Torso),
    };
    let clothes = worn
        .and_then(|id| st.content.item(id))
        .map_or_else(|| NO_CLOTHES.to_string(), |i| i.name.to_lowercase());
    let when = left.map_or_else(
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
        .and_then(|b| b.left.clone())
}

/// The jacket the camera saw (doctor spec §7): what you wore walking away from the body
/// of the news item put out last, fresh or stale. None with no item out, nobody having
/// seen you go, or nothing on your torso then.
pub fn jacket_seen(st: &AppState) -> Option<ItemId> {
    let n = st.missions.news.as_ref()?;
    walked_away(st, &n.about)?.worn
}

/// The channel: the first word of the first named corp building's name, in capitals,
/// then `NEWSLINE`. A building named after its district is a landmark, not a corp's, and
/// is skipped.
pub fn channel(city: &citygen::City) -> String {
    let corp = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_corp())
        .filter_map(|b| {
            let name = b.name.as_deref()?;
            let district = city.district_opt(b.district)?.name.to_lowercase();
            let landmark = !district.is_empty() && name.to_lowercase().contains(&district);
            (!landmark).then_some(name)
        })
        .find_map(|name| name.split_whitespace().next())
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
    use crate::render::map::MapRef;
    use crate::ui::chrome::{DIM, MAGENTA};
    use crate::walker::{Player, Visibility};
    use citygen::{BuildingKind, Feature, Place, Pos, StratumKind};
    use game::{ItemId, Slot};
    use sim::{MoverKind, PersonId};

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

    #[test]
    fn the_channel_is_not_a_landmark_word() {
        use citygen::{GenParams, Generator, SizePreset};
        for seed in [2, 7, 10] {
            let city = Generator::run(GenParams {
                seed,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap();
            let channel = channel(&city);
            let word = channel.strip_suffix(" NEWSLINE").unwrap();
            assert!(!matches!(word, "THE" | "OLD"), "seed {seed}: {channel}");
            for d in &city.districts {
                let name = d.name.to_uppercase();
                assert!(
                    !name.split_whitespace().any(|w| w == word),
                    "seed {seed}: {channel} is a word of {name}"
                );
            }
        }
        // Seed 8 stays as it was.
        let mut city = state_home().city.unwrap();
        assert_eq!(channel(&city), "HEXTEK NEWSLINE");
        // A corp building named after its district is skipped for the next.
        let corps: Vec<usize> = (0..city.buildings.len())
            .filter(|&i| city.buildings[i].kind.is_corp())
            .collect();
        for &i in &corps {
            city.buildings[i].name = None;
        }
        let district = city
            .district(city.buildings[corps[0]].district)
            .name
            .clone();
        city.buildings[corps[0]].name = Some(format!("The {district} Arcology"));
        city.buildings[corps[1]].name = Some("   ".into());
        city.buildings[corps[2]].name = Some("Velform Plaza".into());
        assert_eq!(channel(&city), "VELFORM NEWSLINE");
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

    /// Runs the clock to the start of game minute `m`, if it is not there yet.
    fn to_minute(st: &mut AppState, m: u64) {
        let at = (m * sim::TICKS_PER_MINUTE).max(st.clock_tick());
        run_to(st, at);
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
            worn: st.character.equipped(Slot::Torso).cloned(),
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
            worn: Some(ItemId::new("salvage_jacket")),
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

    #[test]
    fn walking_away_keeps_the_jacket_and_sought_names_it() {
        let mut st = begun();
        // Marlo dies: his body is the item's `about`.
        crate::missions::end(&mut st, 0, 0);
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        st.character.attributes.body = 5;
        let jacket = ItemId::new("salvage_jacket");
        st.character.wear(Slot::Torso, jacket.clone(), None);
        let (place, at) = (st.missions.bodies[0].place, st.missions.bodies[0].pos);
        stand(&mut st, place, at.offset(3, -3));
        stand(&mut st, place, at.offset(0, -13));
        st.character.take_off(Slot::Torso);
        let left = st.missions.bodies[0].left.clone().expect("walked away");
        assert_eq!(left.worn, Some(jacket.clone()));
        assert_eq!(jacket_seen(&st), Some(jacket));
        let seen = "a courier · medium build · salvage jacket";
        assert_eq!(sought(&st).unwrap()[0], seen);
        // What you wear now changes nothing the camera saw.
        st.character
            .wear(Slot::Torso, ItemId::new("ballistic_weave"), None);
        assert_eq!(sought(&st).unwrap()[0], seen);
    }

    #[test]
    fn walking_away_in_nothing_is_dark_clothes_from_then_on() {
        let mut st = begun();
        crate::missions::end(&mut st, 0, 0);
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        st.character.take_off(Slot::Torso);
        let (place, at) = (st.missions.bodies[0].place, st.missions.bodies[0].pos);
        stand(&mut st, place, at.offset(3, -3));
        stand(&mut st, place, at.offset(0, -13));
        assert!(st.missions.bodies[0].left.is_some());
        assert_eq!(jacket_seen(&st), None);
        st.character
            .wear(Slot::Torso, ItemId::new("salvage_jacket"), None);
        assert!(sought(&st).unwrap()[0].ends_with("· dark clothes"));
    }

    /// Runs the clock on to the first game minute from `from` (and from now, and past
    /// the last the outlets rolled) whose roll for `salt` comes up: that minute.
    fn to_a_minute(st: &mut AppState, from: u64, salt: u64, one_in: u64) -> u64 {
        let seed = st.city.as_ref().unwrap().seed;
        let rolled = st.missions.news.as_ref().and_then(|n| n.rolled);
        let from = from.max(st.net.now(st)).max(rolled.map_or(0, |r| r + 1));
        let m = (from..).find(|&m| roll(seed, m, salt, one_in)).unwrap();
        to_minute(st, m);
        m
    }

    /// `to_a_minute` for someone overheard.
    pub(crate) fn to_a_heard_minute(st: &mut AppState, from: u64) -> u64 {
        to_a_minute(st, from, HEARD_SALT, HEARD_ONE_IN)
    }

    /// `to_a_minute` for a holo ad cutting in.
    fn to_an_ad_minute(st: &mut AppState, from: u64) -> u64 {
        to_a_minute(st, from, AD_SALT, AD_ONE_IN)
    }

    /// You at `pos` on `place`, looking around at the hour.
    pub(crate) fn look_from(st: &mut AppState, place: Place, pos: Pos) {
        let mut me = Player::at(place, pos);
        {
            let map = MapRef::for_place(
                st.city.as_ref().unwrap(),
                &st.interiors,
                &st.constructs,
                place,
                st.minute_of_day(),
            )
            .unwrap();
            me.update_fov(&map, st.sight_bonus());
        }
        st.player = Some(me);
    }

    fn seen(st: &AppState, p: Pos) -> bool {
        st.player.as_ref().unwrap().vis().get(p) == Some(&Visibility::Visible)
    }

    /// Everyone on foot on the street now.
    fn on_foot(st: &AppState) -> Vec<(PersonId, Pos)> {
        st.world
            .as_ref()
            .unwrap()
            .visible(citygen::Rect::new(0, 0, 100_000, 100_000))
            .into_iter()
            .filter_map(|m| match m.kind {
                MoverKind::Walker { id, .. } => Some((id, m.pos)),
                _ => None,
            })
            .collect()
    }

    /// You stand `d` tiles off someone on foot in no story role, seeing them, with nobody
    /// else on foot nearer than `HEARD_WITHIN` or `d`: their id. Beside them on their row
    /// if anyone will do, so a bubble over them has room.
    pub(crate) fn by_someone(st: &mut AppState, d: i32) -> PersonId {
        let people = on_foot(st);
        let cast: Vec<PersonId> = st.missions.cast.values().copied().collect();
        let around = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
        ];
        for (dx, dy) in around {
            for &(id, at) in people.iter().filter(|(id, _)| !cast.contains(id)) {
                let spot = at.offset(dx * d, dy * d);
                let city = st.city.as_ref().unwrap();
                let alone = people
                    .iter()
                    .all(|&(o, p)| o == id || p.chebyshev(spot) > d.max(HEARD_WITHIN));
                if !alone || !city.walkable(StratumKind::Street, spot) {
                    continue;
                }
                look_from(st, STREET, spot);
                if seen(st, at) {
                    return id;
                }
            }
        }
        panic!("nobody alone on foot {d} tiles off");
    }

    /// Every holo ad on the street, the city's middle first.
    fn ads(st: &AppState) -> Vec<Pos> {
        let tiles = &st
            .city
            .as_ref()
            .unwrap()
            .stratum(StratumKind::Street)
            .unwrap()
            .tiles;
        let middle = tiles.size().center();
        let mut ads: Vec<Pos> = tiles
            .size()
            .rect()
            .iter()
            .filter(|&p| {
                tiles
                    .get(p)
                    .is_some_and(|t| t.feature == Some(Feature::HoloAd))
            })
            .collect();
        ads.sort_by_key(|p| (p.chebyshev(middle), p.y, p.x));
        ads
    }

    /// You stand on a free street tile a few tiles off a holo ad, seeing it and no other
    /// within `AD_WITHIN`: the ad, the nearest the city's middle that will do.
    fn by_an_ad(st: &mut AppState) -> Pos {
        let ads = ads(st);
        for &ad in &ads {
            for d in 2..=4 {
                for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
                    let spot = ad.offset(dx * d, dy * d);
                    let free = st
                        .city
                        .as_ref()
                        .unwrap()
                        .walkable(StratumKind::Street, spot)
                        && st.world.as_ref().unwrap().person_at(spot).is_none();
                    if !free {
                        continue;
                    }
                    look_from(st, STREET, spot);
                    let in_sight: Vec<Pos> = ads
                        .iter()
                        .copied()
                        .filter(|&p| p.chebyshev(spot) <= AD_WITHIN && seen(st, p))
                        .collect();
                    if in_sight == [ad] {
                        return ad;
                    }
                }
            }
        }
        panic!("no holo ad alone in sight");
    }

    /// The news out, a minute whose ad roll comes up, you by an ad and a frame run: the
    /// ad, cutting in.
    pub(crate) fn an_ad_cutting_in(st: &mut AppState) -> Pos {
        to_an_ad_minute(st, 0);
        let ad = by_an_ad(st);
        tick(st, true);
        assert_eq!(cut_in(st), Some(ad), "the ad cuts in");
        ad
    }

    /// The asides up with `label`: who says them and what.
    fn labelled<'a>(st: &'a AppState, label: &str) -> Vec<(Speaker, &'a str)> {
        st.missions
            .asides
            .iter()
            .filter(|a| a.label == label)
            .map(|a| (a.speaker, a.text.as_str()))
            .collect()
    }

    fn heard(st: &AppState) -> Vec<(Speaker, &str)> {
        labelled(st, "overheard")
    }

    fn cuts(st: &AppState) -> Vec<(Speaker, &str)> {
        labelled(st, "NEWSLINE")
    }

    /// The outlets roll again this minute, as if none had come up yet.
    fn again(st: &mut AppState) {
        let n = st.missions.news.as_mut().unwrap();
        (n.rolled, n.heard_at, n.cut_at) = (None, None, None);
    }

    /// The console's item out on seed 8, the story begun.
    fn out() -> AppState {
        let mut st = begun();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        assert!(asides::live(&st));
        st
    }

    #[test]
    fn the_roll_is_steady() {
        assert_eq!(roll(1, 2, 3, 4), roll(1, 2, 3, 4));
        // About one in four, minute by minute; another salt, other minutes.
        let up = |salt| {
            (0..1200)
                .filter(|&m| roll(8, m, salt, 4))
                .collect::<Vec<u64>>()
        };
        assert!((250..=350).contains(&up(1).len()), "{}", up(1).len());
        assert_ne!(up(1), up(2));
        assert!(
            (0..100).all(|m| roll(8, m, 1, 1)),
            "one in one always comes up"
        );
    }

    #[test]
    fn overheard_from_someone_close_and_seen() {
        let mut st = out();
        let lines = current(&st).unwrap().overheard.clone();
        assert!(lines.len() >= 3);
        let m1 = to_a_heard_minute(&mut st, 0);
        // Three tiles off but out of your sight: nothing.
        by_someone(&mut st, 3);
        let spot = st.player.as_ref().unwrap().pos;
        st.player = Some(Player::at(STREET, spot));
        tick(&mut st, true);
        assert_eq!(heard(&st), []);
        // Seen: the first line, on them, in DIM.
        again(&mut st);
        let id = by_someone(&mut st, 3);
        tick(&mut st, true);
        assert_eq!(heard(&st), [(Speaker::Person(id), lines[0].as_str())]);
        let a = st.missions.asides.iter().find(|a| a.label == "overheard");
        assert_eq!(a.unwrap().colour, DIM);
        // Past the gap, someone else maybe: the second.
        let m2 = to_a_heard_minute(&mut st, m1 + HEARD_GAP);
        let id = by_someone(&mut st, 3);
        frame(&mut st);
        assert_eq!(heard(&st), [(Speaker::Person(id), lines[1].as_str())]);
        // Seven tiles off: too far; six: the third.
        to_a_heard_minute(&mut st, m2 + HEARD_GAP);
        by_someone(&mut st, 7);
        frame(&mut st);
        assert_eq!(heard(&st), []);
        again(&mut st);
        let id = by_someone(&mut st, 6);
        frame(&mut st);
        assert_eq!(heard(&st), [(Speaker::Person(id), lines[2].as_str())]);
    }

    #[test]
    fn the_nearest_speaks_ties_to_the_lowest_id() {
        let mut st = out();
        to_a_heard_minute(&mut st, 0);
        let near = by_someone(&mut st, 2);
        let spot = st.player.as_ref().unwrap().pos;
        // Free tiles you see, `d` tiles off.
        let ring = |st: &AppState, d: i32| -> Vec<Pos> {
            let city = st.city.as_ref().unwrap();
            let world = st.world.as_ref().unwrap();
            citygen::Rect::new(spot.x, spot.y, 1, 1)
                .grow(d)
                .iter()
                .filter(|&p| {
                    p.chebyshev(spot) == d
                        && city.walkable(StratumKind::Street, p)
                        && world.person_at(p).is_none()
                        && seen(st, p)
                })
                .collect()
        };
        let (two, three) = (ring(&st, 2), ring(&st, 3));
        assert!(two.len() >= 2 && !three.is_empty(), "room around {spot:?}");
        let cast: Vec<PersonId> = st.missions.cast.values().copied().collect();
        let other = on_foot(&st)
            .into_iter()
            .map(|(id, _)| id)
            .find(|id| *id != near && !cast.contains(id))
            .unwrap();
        let (lo, hi) = (near.min(other), near.max(other));
        // The higher id nearer: they speak.
        let world = st.world.as_mut().unwrap();
        world.park(lo, three[0]);
        world.park(hi, two[0]);
        tick(&mut st, true);
        assert_eq!(heard(&st).first().map(|h| h.0), Some(Speaker::Person(hi)));
        // Both as near: the lower id.
        again(&mut st);
        st.missions.asides.clear();
        let free = two.iter().find(|&&p| p != two[0]).copied().unwrap();
        st.world.as_mut().unwrap().park(lo, free);
        tick(&mut st, true);
        assert_eq!(heard(&st).first().map(|h| h.0), Some(Speaker::Person(lo)));
    }

    #[test]
    fn never_within_twelve_minutes() {
        let mut st = out();
        let seed = st.city.as_ref().unwrap().seed;
        let m1 = to_a_heard_minute(&mut st, 0);
        by_someone(&mut st, 3);
        frame(&mut st);
        assert_eq!(heard(&st).len(), 1);
        let inside = m1 + 1..m1 + HEARD_GAP;
        assert!(
            inside
                .clone()
                .any(|m| roll(seed, m, HEARD_SALT, HEARD_ONE_IN)),
            "a roll comes up inside the gap"
        );
        for m in inside {
            to_minute(&mut st, m);
            st.missions.asides.clear();
            by_someone(&mut st, 3);
            frame(&mut st);
            assert_eq!(heard(&st), [], "minute {}", m - m1);
        }
        to_a_heard_minute(&mut st, m1 + HEARD_GAP);
        by_someone(&mut st, 3);
        frame(&mut st);
        assert_eq!(heard(&st).len(), 1);
    }

    #[test]
    fn never_a_story_role_or_the_dead() {
        let mut st = out();
        to_a_heard_minute(&mut st, 0);
        let id = by_someone(&mut st, 3);
        st.missions.cast.insert("passer-by".into(), id);
        tick(&mut st, true);
        assert_eq!(heard(&st), [], "cast in a role");
        again(&mut st);
        st.missions.cast.remove("passer-by");
        tick(&mut st, true);
        assert_eq!(heard(&st).len(), 1, "out of the role, they speak");
        again(&mut st);
        st.missions.asides.clear();
        st.world.as_mut().unwrap().kill(id);
        tick(&mut st, true);
        assert_eq!(heard(&st), [], "dead");
    }

    #[test]
    fn nothing_off_the_street() {
        let mut st = out();
        let seed = st.city.as_ref().unwrap().seed;
        let from = st.net.now(&st);
        let span = from..from + 30;
        assert!(
            span.clone()
                .any(|m| roll(seed, m, HEARD_SALT, HEARD_ONE_IN))
        );
        assert!(span.clone().any(|m| roll(seed, m, AD_SALT, AD_ONE_IN)));
        let (flat, inside) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        let (floor, outside) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        for m in span {
            to_minute(&mut st, m);
            if m % 2 == 0 {
                look_from(&mut st, flat, inside);
            } else {
                look_from(&mut st, floor, outside);
            }
            frame(&mut st);
            assert_eq!((heard(&st), cuts(&st), cut_in(&st)), (vec![], vec![], None));
        }
        // Below the street at an ad's own spot, that spot in sight: only the gate stops it.
        let ad = ads(&st)[0];
        let under = Place::Stratum(StratumKind::Undercity);
        look_from(&mut st, under, ad);
        assert!(seen(&st, ad), "the spot is marked visible below");
        again(&mut st);
        for m in from + 30..from + 90 {
            to_minute(&mut st, m);
            look_from(&mut st, under, ad);
            frame(&mut st);
            assert_eq!((heard(&st), cuts(&st), cut_in(&st)), (vec![], vec![], None));
        }
        // No city, no world, nobody: nothing, and no panic.
        let mut st = AppState::new();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        tick(&mut st, true);
        assert_eq!(cut_in(&st), None);
    }

    #[test]
    fn a_holo_ad_cuts_in() {
        let mut st = out();
        let headline = current(&st).unwrap().headline.to_uppercase();
        // In range but out of your sight: nothing.
        to_an_ad_minute(&mut st, 0);
        by_an_ad(&mut st);
        let spot = st.player.as_ref().unwrap().pos;
        st.player = Some(Player::at(STREET, spot));
        tick(&mut st, true);
        assert_eq!((cuts(&st), cut_in(&st)), (vec![], None));
        again(&mut st);
        let ad = an_ad_cutting_in(&mut st);
        let text = format!("▸ {headline}");
        assert_eq!(cuts(&st), [(Speaker::Tile(STREET, ad), text.as_str())]);
        let a = st.missions.asides.iter().find(|a| a.label == "NEWSLINE");
        assert_eq!(a.unwrap().colour, MAGENTA);
        // Three seconds on the clock.
        let start = st.clock_tick();
        run_to(&mut st, start + CUT_TICKS - 1);
        assert_eq!(cut_in(&st), Some(ad));
        run_to(&mut st, start + CUT_TICKS);
        assert_eq!(cut_in(&st), None);
        asides::prune(&mut st);
        assert_eq!(cuts(&st), [], "the aside ends with the cut");
        // Not again inside the gap, though a roll comes up there; then again.
        let seed = st.city.as_ref().unwrap().seed;
        let cut_at = current(&st).unwrap().cut_at.unwrap();
        let inside = st.net.now(&st)..cut_at + AD_GAP;
        assert!(inside.clone().any(|m| roll(seed, m, AD_SALT, AD_ONE_IN)));
        for m in inside {
            to_minute(&mut st, m);
            look_from(&mut st, STREET, spot);
            frame(&mut st);
            assert_eq!(cut_in(&st), None, "minute {}", m - cut_at);
        }
        to_an_ad_minute(&mut st, cut_at + AD_GAP);
        look_from(&mut st, STREET, spot);
        frame(&mut st);
        assert_eq!(cut_in(&st), Some(ad));
    }

    #[test]
    fn screens_log_once_per_building() {
        let mut st = out();
        let line = format!(
            "Over the counter, a screen: {}.",
            current(&st).unwrap().headline.to_uppercase()
        );
        let shown = |st: &AppState| st.log.iter().filter(|e| e.text == line).count();
        let city = st.city.as_ref().unwrap();
        let first = |kind: BuildingKind| {
            city.buildings
                .iter()
                .find(|b| b.kind == kind)
                .map(|b| b.id)
                .unwrap_or_else(|| panic!("seed 8 has a {kind:?}"))
        };
        let (bar, noodles, shop) = (
            first(BuildingKind::Bar),
            first(BuildingKind::NoodleBar),
            first(BuildingKind::Shop),
        );
        // A tile of a walk-in's footprint on the street, and a bare street tile.
        let on = |b| {
            city.building(b)
                .footprint
                .iter()
                .find(|&p| city.tile(StratumKind::Street, p).and_then(|t| t.building) == Some(b))
                .unwrap()
        };
        let (noodles_at, shop_at) = (on(noodles), on(shop));
        let tiles = &city.stratum(StratumKind::Street).unwrap().tiles;
        let bare = tiles
            .size()
            .rect()
            .iter()
            .find(|&p| {
                city.walkable(StratumKind::Street, p)
                    && tiles.get(p).is_some_and(|t| t.building.is_none())
            })
            .unwrap();
        let bar_floor = Place::Floor {
            building: bar,
            level: 0,
        };
        stand(&mut st, bar_floor, Pos::new(1, 1));
        assert_eq!(shown(&st), 1, "into the bar");
        stand(&mut st, bar_floor, Pos::new(2, 1));
        stand(&mut st, STREET, bare);
        stand(&mut st, bar_floor, Pos::new(1, 1));
        assert_eq!(shown(&st), 1, "out and in again: nothing new");
        stand(&mut st, STREET, noodles_at);
        assert_eq!(shown(&st), 2, "into the noodle bar's footprint");
        stand(&mut st, STREET, bare);
        stand(&mut st, STREET, noodles_at);
        stand(&mut st, STREET, shop_at);
        assert_eq!(shown(&st), 2, "the noodle bar again, and a shop");
        // A new item: the bar shows it too.
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        stand(&mut st, bar_floor, Pos::new(1, 1));
        assert_eq!(shown(&st), 3);
        // A closed bar's tile on a city map is not its inside.
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        let city = st.city.as_ref().unwrap();
        let wall = city
            .building(bar)
            .footprint
            .iter()
            .find(|&p| city.tile(StratumKind::Street, p).and_then(|t| t.building) == Some(bar))
            .unwrap();
        stand(&mut st, STREET, wall);
        assert_eq!(shown(&st), 3, "on the bar's tile on the street");
        // The map not on foot: not yet; on foot, once.
        stand(&mut st, STREET, bare);
        st.player = Some(Player::at(bar_floor, Pos::new(1, 1)));
        tick(&mut st, false);
        assert_eq!(shown(&st), 3, "the map not on foot");
        tick(&mut st, true);
        assert_eq!(shown(&st), 4);
        // A walk-in's basement carries its id on the Undercity: not the screen.
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        // (Citygen writes it under a noodle bar's basement; here the bar is given one.)
        let under = StratumKind::Undercity;
        let cellar = st
            .city
            .as_mut()
            .and_then(|c| c.tile_mut(under, noodles_at))
            .unwrap();
        cellar.building = Some(noodles);
        stand(&mut st, Place::Stratum(under), noodles_at);
        assert_eq!(shown(&st), 4, "under the building");
    }

    #[test]
    fn stale_news_is_quiet() {
        let mut st = out();
        let fresh_for = current(&st).unwrap().fresh;
        let stale = |st: &mut AppState, yes: bool| {
            st.missions.news.as_mut().unwrap().fresh = if yes { 0 } else { fresh_for };
        };
        // Someone close, seen, the roll up.
        to_a_heard_minute(&mut st, 0);
        by_someone(&mut st, 3);
        stale(&mut st, true);
        tick(&mut st, true);
        assert_eq!(heard(&st), []);
        stale(&mut st, false);
        tick(&mut st, true);
        assert_eq!(heard(&st).len(), 1, "fresh, they speak");
        // An ad in sight, the roll up.
        st.missions.asides.clear();
        to_an_ad_minute(&mut st, 0);
        by_an_ad(&mut st);
        stale(&mut st, true);
        tick(&mut st, true);
        assert_eq!((cuts(&st), cut_in(&st)), (vec![], None));
        stale(&mut st, false);
        tick(&mut st, true);
        assert!(cut_in(&st).is_some(), "fresh, it cuts in");
        stale(&mut st, true);
        assert_eq!(cut_in(&st), None, "gone stale under it");
        // A bar's floor.
        let bar = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::Bar)
            .unwrap()
            .id;
        let floor = Place::Floor {
            building: bar,
            level: 0,
        };
        let screens = |st: &AppState| {
            st.log
                .iter()
                .filter(|e| e.text.starts_with("Over the counter"))
                .count()
        };
        stand(&mut st, floor, Pos::new(1, 1));
        assert_eq!(screens(&st), 0);
        stale(&mut st, false);
        stand(&mut st, floor, Pos::new(1, 1));
        assert_eq!(screens(&st), 1, "fresh, the screen shows it");
    }
}
