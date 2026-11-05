//! The world beyond the city: the overworld, where you are on it, and what you have seen
//! of the open ground. The overworld comes from the city's seed and is rebuilt on load;
//! the home city sits on a block of its tiles (until the city grows into the world, the
//! whole block is the city). See `2026-11-04-overworld-screen-design.md`.

pub mod ground;
pub mod moves;

use crate::app::AppState;
use citygen::overworld::{Overworld, WorldParams};
use citygen::{City, Pos, Rect, StratumKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where you are: in the city, on an overworld tile, or on a detail cell of the open
/// ground (world-wide detail coordinates).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum At {
    #[default]
    City,
    Overworld(Pos),
    Ground(Pos),
}

/// The world beyond the city, as the run holds it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outside {
    pub params: WorldParams,
    pub at: At,
    /// What you have seen of the open ground: per chunk of 8×8 detail cells (not per
    /// overworld tile, whose size may change), a bit a cell (`y * 8 + x`).
    pub seen: BTreeMap<Pos, u64>,
    /// The overworld, made from `params`; rebuilt on load (`after_restore`).
    #[serde(skip)]
    pub world: Option<Overworld>,
    /// Jacked in on the open ground: where you are on its Net, your body on the ground
    /// cell `at` names. Not saved: a run reopened out here wakes on the body.
    #[serde(skip)]
    pub net: Option<Pos>,
    /// The city or town the run's city is; none for the home city (and every run from
    /// before other cities could be walked into).
    #[serde(default)]
    pub here: Option<Settlement>,
    /// The other cities and towns built this session, each the same from its world every
    /// time it's built: kept so walking back in is quick.
    #[serde(skip)]
    pub built: BTreeMap<Settlement, City>,
    /// The other cities' and towns' blocks, from the world (`others`).
    #[serde(skip)]
    blocks: std::cell::OnceCell<Vec<(Settlement, Rect)>>,
}

/// A place on the overworld with a map of its own: a world city, or one of its towns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Settlement {
    pub city: u8,
    pub town: Option<u8>,
}

/// Everything a run holds of the city it is in: set aside, for the home city, while
/// you're in another (and saved with the run).
#[derive(Serialize, Deserialize)]
pub struct Stay {
    pub city: City,
    pub player: Option<crate::walker::Player>,
    pub world: Option<sim::World>,
    pub interiors: crate::interiors::Interiors,
    pub constructs: crate::constructs::Constructs,
    pub net: crate::net::NetState,
    pub backdoors: crate::backdoors::Backdoors,
    pub ground: game::Ground,
    pub foes: Vec<crate::foes::Foe>,
    pub bots: Vec<crate::bots::BotOut>,
    pub live: Vec<crate::blast::Live>,
    pub journal: crate::credentials::Journal,
    pub jobs: crate::jobs::Jobs,
    pub missions: crate::missions::Missions,
    pub track: Option<crate::track::Track>,
    /// HELIX's hosts there (its buildings', its districts', the rig's).
    pub helix: crate::helix::Helix,
    /// How far through the journal's mails the log has said, and the mail's pill.
    pub mail_seen: usize,
    pub mail_pill: Option<crate::mail_mark::MailPill>,
    /// The keycards to its doors: their catalogue entries and what's in the pack.
    pub cards: Vec<game::Item>,
    pub card_stacks: Vec<game::Stack>,
}

impl Stay {
    /// The city the run is in, taken out of it.
    fn take(st: &mut AppState) -> Option<Stay> {
        use std::mem::take;
        let cards = crate::credentials::run_items(&st.content);
        let card_stacks = st
            .character
            .inventory
            .iter()
            .filter(|s| crate::credentials::run_made(&s.item))
            .cloned()
            .collect();
        crate::credentials::purge(&mut st.content, &mut st.character);
        Some(Stay {
            city: st.city.take()?,
            player: st.player.take(),
            world: st.world.take(),
            interiors: take(&mut st.interiors),
            constructs: take(&mut st.constructs),
            net: take(&mut st.net),
            backdoors: take(&mut st.backdoors),
            ground: take(&mut st.ground),
            foes: take(&mut st.foes),
            bots: take(&mut st.bots),
            live: take(&mut st.live),
            journal: take(&mut st.journal),
            jobs: take(&mut st.jobs),
            missions: take(&mut st.missions),
            track: st.track.take(),
            helix: take(&mut st.helix),
            mail_seen: take(&mut st.mail_seen),
            mail_pill: st.mail_pill.take(),
            cards,
            card_stacks,
        })
    }

    /// A city as you first come into it at clock tick `now`, minute `minute` of the
    /// day: its people at their day, nothing else yet.
    fn fresh(st: &AppState, city: City, now: u64, minute: u16) -> Stay {
        let day = 24 * 60;
        let spent = (now / sim::TICKS_PER_MINUTE) % day;
        let start = (minute as u64 + day - spent) % day;
        let world = sim::World::seed(
            &city,
            &st.people,
            &sim::SimParams::default(),
            start as u16,
            now,
        );
        Stay {
            city,
            player: None,
            world: Some(world),
            interiors: Default::default(),
            constructs: Default::default(),
            net: Default::default(),
            backdoors: Default::default(),
            ground: Default::default(),
            foes: Vec::new(),
            bots: Vec::new(),
            live: Vec::new(),
            journal: Default::default(),
            jobs: Default::default(),
            missions: Default::default(),
            track: None,
            helix: Default::default(),
            mail_seen: 0,
            mail_pill: None,
            cards: Vec::new(),
            card_stacks: Vec::new(),
        }
    }

    /// Puts this city in place as the one the run is in at clock tick `now` (its people
    /// run on to it, as asleep), its people's places rebuilt and its home (the flat, in
    /// the home city only) found again.
    fn put(self, st: &mut AppState, home: bool, now: u64) {
        let Stay {
            city,
            player,
            mut world,
            interiors,
            constructs,
            net,
            backdoors,
            ground,
            foes,
            bots,
            live,
            journal,
            jobs,
            missions,
            track,
            helix,
            mail_seen,
            mail_pill,
            cards,
            card_stacks,
        } = self;
        if let Some(w) = world.as_mut() {
            w.rebuild(&city, &st.people);
            if w.tick() < now {
                w.advance(&city, now);
            }
        }
        st.home = if home {
            crate::home::HomeRig::find(&city)
        } else {
            None
        };
        st.city = Some(city);
        st.player = player;
        st.world = world;
        st.interiors = interiors;
        st.constructs = constructs;
        st.net = net;
        st.backdoors = backdoors;
        st.ground = ground;
        st.foes = foes;
        st.bots = bots;
        st.live = live;
        st.journal = journal;
        st.jobs = jobs;
        st.missions = missions;
        st.track = track;
        st.helix = helix;
        st.mail_seen = mail_seen;
        st.mail_pill = mail_pill;
        crate::credentials::put_back(&mut st.content, cards);
        st.character.inventory.extend(card_stacks);
        st.fx.clear();
        // the deck's and the MAP's picks were the city you left's
        st.deck_jobs_open.clear();
        st.deck_jobs_sel = None;
        st.deck_map_to = None;
        st.atlas = Default::default();
        crate::holds::relight(st);
    }

    /// After a run is reopened: the set-aside city's people's places rebuilt, and what
    /// it holds that this build's content lacks dropped (its lines in `lost`).
    pub fn reopened(&mut self, people: &sim::People, content: &game::Content) -> Vec<String> {
        if let Some(w) = self.world.as_mut() {
            w.rebuild(&self.city, people);
        }
        let mut lost = self.ground.forget_unknown(content);
        lost.extend(self.missions.forget_unknown(content));
        lost
    }
}

impl Outside {
    /// The world round `city`, made from its seed, with you in the city.
    pub fn new(city: &City) -> anyhow::Result<Outside> {
        // a city on its site stands in that world; one from before, in its seed's
        let params = match &city.params.site {
            Some(site) => site.world.clone(),
            None => world_params(city.seed),
        };
        let world = Overworld::generate(params.clone())?;
        Ok(Outside {
            params,
            at: At::City,
            seen: BTreeMap::new(),
            world: Some(world),
            net: None,
            here: None,
            built: BTreeMap::new(),
            blocks: Default::default(),
        })
    }

    pub fn world(&self) -> &Overworld {
        self.world
            .as_ref()
            .expect("the overworld is made with the run and on load")
    }

    /// Makes the overworld again from its params (after a load).
    pub fn rebuild(&mut self) {
        if self.world.is_none() {
            self.world = Overworld::generate(self.params.clone()).ok();
        }
    }

    /// The overworld tiles the city's map covers: for a city on its site, the block it
    /// was built on; for one from before, the city's size in tiles, centred on the home
    /// city's own tiles, inside the world.
    pub fn block(&self, city: &City) -> Rect {
        if let Some(site) = &city.params.site {
            return Rect::new(
                site.origin.x.div_euclid(TILE),
                site.origin.y.div_euclid(TILE),
                city.size.w as i32 / TILE,
                city.size.h as i32 / TILE,
            );
        }
        let w = self.world();
        let (bw, bh) = (
            city.size.w.div_ceil(TILE as u32) as i32,
            city.size.h.div_ceil(TILE as u32) as i32,
        );
        let home = w.cities.iter().position(|c| c.home).unwrap_or(0) as u8;
        let mine: Vec<Pos> = w
            .tiles
            .iter()
            .filter(|(_, t)| t.urban.is_some_and(|u| u.city == home))
            .map(|(p, _)| p)
            .collect();
        let centre = match (
            mine.iter().map(|p| p.x).min(),
            mine.iter().map(|p| p.x).max(),
            mine.iter().map(|p| p.y).min(),
            mine.iter().map(|p| p.y).max(),
        ) {
            (Some(x0), Some(x1), Some(y0), Some(y1)) => Pos::new((x0 + x1) / 2, (y0 + y1) / 2),
            _ => w.home().centre,
        };
        let fit =
            |c: i32, len: i32, world: u32| (c - len / 2).clamp(0, (world as i32 - len).max(0));
        Rect::new(
            fit(centre.x, bw, w.size.w),
            fit(centre.y, bh, w.size.h),
            bw,
            bh,
        )
    }

    /// The overworld tile a cell of the city's street lies in.
    pub fn tile_of(&self, city: &City, p: Pos) -> Pos {
        let b = self.block(city);
        Pos::new(
            b.x + p.x.div_euclid(TILE).clamp(0, b.w - 1),
            b.y + p.y.div_euclid(TILE).clamp(0, b.h - 1),
        )
    }

    /// Where you stand in the city coming in on block tile `tile`: the outdoor street
    /// cell nearest the tile's middle.
    pub fn street_of(&self, city: &City, tile: Pos) -> Pos {
        let b = self.block(city);
        street_near(
            city,
            Pos::new(
                TILE * (tile.x - b.x) + TILE / 2,
                TILE * (tile.y - b.y) + TILE / 2,
            ),
        )
    }

    /// Where you stand in the city walking in at world-wide cell `cell` of its block:
    /// the outdoor street cell nearest the city's cell there.
    pub fn street_at(&self, city: &City, cell: Pos) -> Pos {
        let b = self.block(city);
        street_near(city, Pos::new(cell.x - TILE * b.x, cell.y - TILE * b.y))
    }

    /// The home city: the world's.
    pub fn home(&self) -> Settlement {
        let city = self.world().cities.iter().position(|c| c.home).unwrap_or(0) as u8;
        Settlement { city, town: None }
    }

    /// The city or town the run is in.
    pub fn here(&self) -> Settlement {
        self.here.unwrap_or_else(|| self.home())
    }

    /// Every city and town of the world but the home city, each with its block.
    pub fn others(&self) -> Vec<(Settlement, Rect)> {
        self.blocks.get_or_init(|| self.blocks_of_others()).clone()
    }

    fn blocks_of_others(&self) -> Vec<(Settlement, Rect)> {
        let w = self.world();
        let home = self.home();
        let mut out = Vec::new();
        for (i, c) in w.cities.iter().enumerate() {
            let i = i as u8;
            let s = Settlement {
                city: i,
                town: None,
            };
            if s != home {
                out.push((s, w.city_block(i)));
            }
            for t in 0..c.towns.len() as u8 {
                if let Some(b) = w.town_block(i, t) {
                    out.push((
                        Settlement {
                            city: i,
                            town: Some(t),
                        },
                        b,
                    ));
                }
            }
        }
        out
    }

    /// What a city or town is called.
    pub fn name(&self, s: Settlement) -> String {
        let city = self
            .world()
            .cities
            .get(s.city as usize)
            .map_or("the city", |c| c.name.as_str());
        match s.town {
            None => city.to_string(),
            Some(_) => format!("a town off {city}"),
        }
    }
}

/// The outdoor street cell of `city` nearest `cell` (clamped into the city).
fn street_near(city: &City, cell: Pos) -> Pos {
    let (cw, ch) = (city.size.w as i32, city.size.h as i32);
    let from = Pos::new(cell.x.clamp(0, cw - 1), cell.y.clamp(0, ch - 1));
    nearest(from, cw.max(ch), |p| {
        city.tile(StratumKind::Street, p)
            .is_some_and(|t| t.is_walkable() && t.building.is_none())
    })
    .unwrap_or(from)
}

/// A run's world for seed `seed`: the default world, its size kept in tiles rather than
/// by the preset's name, so a run keeps its world when the presets are retuned.
pub fn world_params(seed: u64) -> WorldParams {
    let d = WorldParams::default();
    let citygen::Size { w, h } = d.size.size();
    WorldParams {
        seed,
        size: citygen::overworld::WorldSize::Custom { w, h },
        ..d
    }
}

/// The city or town whose map covers overworld tile `tile`: the one you're in first,
/// then the home city (set aside), then the rest.
pub fn settlement_at(st: &AppState, tile: Pos) -> Option<Settlement> {
    let o = st.outside.as_ref()?;
    let city = st.city.as_ref()?;
    if o.block(city).contains(tile) {
        return Some(o.here());
    }
    let home = o.home();
    if let Some(stay) = st.away.as_ref()
        && o.block(&stay.city).contains(tile)
    {
        return Some(home);
    }
    o.others()
        .into_iter()
        .find(|(s, b)| *s != o.here() && b.contains(tile))
        .map(|(s, _)| s)
}

/// Whether overworld tile `tile` is on any city's or town's block (tests pick open
/// ground off them all).
#[cfg(test)]
pub(crate) fn in_any_block(st: &AppState, tile: Pos) -> bool {
    settlement_at(st, tile).is_some()
}

/// Whether the run is in a city or town other than its home: the story waits there.
pub fn away_from_home(st: &AppState) -> bool {
    st.outside
        .as_ref()
        .is_some_and(|o| o.here.is_some_and(|h| h != o.home()))
}

/// Into city or town `s`: the one you're in set aside (kept with the run when it's the
/// home city), `s` built on its site the first time this session (else as it was kept).
/// One that won't build leaves you where you were.
pub fn enter(st: &mut AppState, s: Settlement) -> Result<(), String> {
    let Some(o) = st.outside.as_ref() else {
        return Err("there is no world out here".into());
    };
    let (here, home) = (o.here(), o.home());
    if s == here {
        return Ok(());
    }
    let name = o.name(s);
    let next = if s == home {
        None
    } else if let Some(c) = o.built.get(&s) {
        Some(c.clone())
    } else {
        let w = o.world();
        let base = citygen::GenParams {
            seed: citygen::rng::hash64(
                w.seed ^ 0xC17E,
                ((s.city as u64) << 8) | s.town.map_or(0, |t| t as u64 + 1),
            ) % 1_000_000_007,
            ..st.params.clone()
        };
        let params = match s.town {
            None => Some(citygen::sited(w, s.city, base)),
            Some(t) => citygen::sited_town(w, s.city, t, base),
        };
        let built = params.and_then(|p| citygen::Generator::run(p).ok());
        match built {
            Some(c) => Some(c),
            None => return Err(format!("no way into {name}")),
        }
    };
    if s == home && st.away.is_none() {
        return Err(format!("no way into {name}"));
    }
    // the hour carries over: the run's clock is the city's you're in; and the deck's
    // own switch and its cooling go where you go
    let (now, minute) = (st.clock_tick(), st.minute_of_day());
    let (failsafe, cooling) = (st.net.failsafe, st.net.cooling_until);
    let Some(current) = Stay::take(st) else {
        return Err("nowhere to go".into());
    };
    if here == home {
        st.away = Some(Box::new(current));
    } else if let Some(o) = st.outside.as_mut() {
        o.built.insert(here, current.city);
    }
    match next {
        None => {
            if let Some(stay) = st.away.take() {
                stay.put(st, true, now);
            }
        }
        Some(city) => {
            let stay = Stay::fresh(st, city, now, minute);
            stay.put(st, false, now);
        }
    }
    if let Some(o) = st.outside.as_mut() {
        o.here = (s != home).then_some(s);
    }
    st.net.failsafe = failsafe;
    st.net.cooling_until = cooling;
    st.say(format!("into {name}"));
    Ok(())
}

/// Detail cells to an overworld tile, each way.
pub const TILE: i32 = citygen::overworld::detail::TILE;

/// The cell nearest `from` (by rings, then row order) where `ok` holds, out to `reach`.
pub fn nearest(from: Pos, reach: i32, ok: impl Fn(Pos) -> bool) -> Option<Pos> {
    (0..=reach).find_map(|r| {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| dx.abs().max(dy.abs()) == r)
            .map(|(dx, dy)| from.offset(dx, dy))
            .find(|p| ok(*p))
    })
}

/// You are out of the city, on the overworld or the open ground: the city holds still
/// for you (its story waits, its cameras look at no one), your body isn't on its street.
pub fn away(st: &AppState) -> bool {
    st.outside.as_ref().is_some_and(|o| o.at != At::City)
}

/// After a run is opened: the overworld made again, and you back in the city if what
/// you stood on is no longer there (the world under a saved run can change between
/// builds until seeds are fixed for 1.0).
pub fn reopened(st: &mut AppState) {
    let Some(o) = st.outside.as_mut() else {
        return;
    };
    o.rebuild();
    let Some(w) = o.world.as_ref() else {
        st.outside = None;
        st.warn("the world out there couldn't be made again · back in the city");
        return;
    };
    let stands = match o.at {
        At::City => true,
        At::Overworld(t) => moves::step_cost(w, t).is_some(),
        At::Ground(c) => {
            let (cw, ch) = (TILE * w.size.w as i32, TILE * w.size.h as i32);
            c.x >= 0
                && c.y >= 0
                && c.x < cw
                && c.y < ch
                && citygen::overworld::detail::region(w, c.x, c.y, 1, 1)[Pos::new(0, 0)]
                    .is_walkable()
        }
    };
    if !stands {
        o.at = At::City;
        st.warn("the world out there has changed since this run was saved · back in the city");
    }
}

/// Whether you can leave the city for the world now: on foot on its open street, in a
/// city (not the lot), not jacked in, not in a fight, with no call or scene under way.
pub fn can_leave(st: &AppState) -> Result<(), &'static str> {
    let (Some(city), Some(p)) = (st.city.as_ref(), st.player.as_ref()) else {
        return Err("nowhere to go");
    };
    if city.is_lot() {
        return Err("there is no world past the lot");
    }
    if p.jacked_in() {
        return Err("jack out first");
    }
    if st.net.engaged.is_some() {
        return Err("not in a fight");
    }
    let m = &st.missions;
    if m.call.is_some() || !m.calls.is_empty() {
        return Err("not with a call on the line");
    }
    if m.scene.is_some() || !m.scenes.is_empty() {
        return Err("not now");
    }
    let outdoors = p.place.is_street()
        && city
            .tile(StratumKind::Street, p.pos)
            .is_some_and(|t| t.building.is_none());
    if !outdoors {
        return Err("only from the open street");
    }
    Ok(())
}

/// Out of the city by its edge: from street cell `from` a step `d` off the map, onto
/// the open ground beside the block, where there is ground to stand on.
pub fn leave_by(st: &mut AppState, from: Pos, d: citygen::Dir) -> Result<(), &'static str> {
    can_leave(st)?;
    let city = st.city.clone().ok_or("nowhere to go")?;
    let o = ensure(st).ok_or("there is no world out here")?;
    let b = o.block(&city);
    let to = from.step(d);
    let cell = Pos::new(TILE * b.x + to.x, TILE * b.y + to.y);
    let w = o.world();
    if cell.x < 0
        || cell.y < 0
        || cell.x >= TILE * w.size.w as i32
        || cell.y >= TILE * w.size.h as i32
    {
        return Err("the world ends here");
    }
    let ground = citygen::overworld::detail::region(w, cell.x, cell.y, 1, 1)[Pos::new(0, 0)];
    if !ground.is_walkable() {
        return Err("no way out there");
    }
    o.at = At::Ground(cell);
    Ok(())
}

/// The run's world beyond the city, made from the city's seed the first time it is
/// needed (an old save, a test or a dev start has none yet).
pub fn ensure(st: &mut AppState) -> Option<&mut Outside> {
    if st.outside.is_none() {
        let city = st.city.as_ref()?;
        st.outside = Some(Outside::new(city).ok()?);
    }
    st.outside.as_mut()
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};

    fn medium() -> City {
        Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap()
    }

    /// Seed 3's home city built on its place in its world, once for every test.
    fn sited() -> &'static City {
        static CITY: std::sync::OnceLock<City> = std::sync::OnceLock::new();
        CITY.get_or_init(|| {
            crate::screens::build_home(GenParams {
                seed: 3,
                ..Default::default()
            })
            .unwrap()
        })
    }

    /// A run in seed 3's home city on its site, its world made, you on its street.
    fn home_run() -> AppState {
        let mut st = AppState::new();
        st.enter_city(sited().clone());
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        ensure(&mut st);
        st
    }

    /// Another city of the world, and the overworld tile west of its block's middle
    /// row, a step east into it.
    fn beside_another(st: &AppState) -> (Settlement, Pos) {
        let o = st.outside.as_ref().unwrap();
        let (s, b) = o
            .others()
            .into_iter()
            .find(|(s, b)| s.town.is_none() && b.x > 0)
            .unwrap();
        (s, Pos::new(b.x - 1, b.y + b.h / 2))
    }

    #[test]
    fn stepping_onto_another_city_goes_into_it() {
        let mut st = home_run();
        let home = st.city.as_ref().unwrap().seed;
        let (s, from) = beside_another(&st);
        st.outside.as_mut().unwrap().at = At::Overworld(from);
        let moved = moves::world_step(&mut st, citygen::Dir::E);
        assert!(matches!(moved, moves::Moved::City(_)));
        let site = st.city.as_ref().unwrap().params.site.clone().unwrap();
        assert_eq!((site.city, site.town), (s.city, s.town));
        assert_eq!(st.outside.as_ref().unwrap().here, Some(s));
        assert_eq!(st.away.as_ref().unwrap().city.seed, home, "home set aside");
        assert!(st.home.is_none(), "no flat in another city");
        assert!(away_from_home(&st));
    }

    #[test]
    fn coming_home_puts_the_home_city_back() {
        let mut st = home_run();
        crate::missions::begin(&mut st);
        let flat = st.home.unwrap();
        let door = crate::track::Source::Spot {
            place: flat.place(),
            pos: flat.home.door,
            radius: 0,
        };
        assert!(crate::track::set(&mut st, door).is_ok());
        let tracked = st.track.as_ref().map(|t| format!("{:?}", t.source));
        crate::clock::run(
            &mut st,
            Some(20 * sim::TICKS_PER_MINUTE),
            crate::clock::PATHS_PER_FRAME,
        );
        let jobs = |st: &AppState| {
            (
                st.jobs.list.len(),
                st.jobs.fixers.keys().copied().collect::<Vec<_>>(),
            )
        };
        let before = jobs(&st);
        let story = format!("{:?}", st.missions);
        let city = st.city.clone();
        let flat = st.home.as_ref().map(|h| h.home.building);
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        assert!(st.missions.runs.is_empty(), "the story isn't there");
        let home = st.outside.as_ref().unwrap().home();
        enter(&mut st, home).unwrap();
        assert_eq!(st.city, city);
        assert_eq!(format!("{:?}", st.missions), story);
        assert_eq!(
            st.track.as_ref().map(|t| format!("{:?}", t.source)),
            tracked
        );
        assert_eq!(jobs(&st), before);
        assert_eq!(st.home.as_ref().map(|h| h.home.building), flat);
        assert!(st.away.is_none() && st.outside.as_ref().unwrap().here.is_none());
        assert!(!away_from_home(&st));
    }

    #[test]
    fn a_citys_hosts_cards_and_mail_stay_with_it() {
        // HELIX's hosts, the keycards to its doors and the mail's marks are the home
        // city's: none of them in another city, all back as they were at home
        let mut st = home_run();
        let flat = st.home.unwrap().home.building;
        crate::credentials::prepare(&mut st, flat);
        crate::credentials::move_in(&mut st);
        crate::helix::hosts::ensure_host(&mut st, crate::helix::shell::HostId::Building(flat));
        st.mail_seen = 2;
        st.mail_pill = Some(crate::mail_mark::MailPill { mail: 1, frames: 9 });
        let cards = |st: &AppState| {
            st.character
                .inventory
                .iter()
                .filter(|s| crate::credentials::run_made(&s.item))
                .count()
        };
        let (hosts, held, known) = (
            st.helix.hosts.len(),
            cards(&st),
            crate::credentials::run_items(&st.content).len(),
        );
        assert!(hosts > 0 && held > 0 && known > 0);
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        assert!(st.helix.hosts.is_empty(), "no host of home's here");
        assert_eq!(cards(&st), 0, "no card to home's doors here");
        assert!(crate::credentials::run_items(&st.content).is_empty());
        assert_eq!((st.mail_seen, st.mail_pill), (0, None));
        let home = st.outside.as_ref().unwrap().home();
        enter(&mut st, home).unwrap();
        assert_eq!(st.helix.hosts.len(), hosts);
        assert_eq!(cards(&st), held);
        assert_eq!(crate::credentials::run_items(&st.content).len(), known);
        assert_eq!(st.mail_seen, 2);
        assert_eq!(
            st.mail_pill,
            Some(crate::mail_mark::MailPill { mail: 1, frames: 9 })
        );
    }

    #[test]
    fn the_deck_keeps_its_failsafe_and_its_cooling_everywhere() {
        let mut st = home_run();
        st.net.failsafe = false;
        st.net.cooling_until = Some(st.net.now(&st) + 60);
        let cooling = st.net.cooling_until;
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        assert!(!st.net.failsafe, "the deck's own switch");
        assert_eq!(
            st.net.cooling_until, cooling,
            "a dumped deck cools wherever you go"
        );
        let home = st.outside.as_ref().unwrap().home();
        enter(&mut st, home).unwrap();
        assert!(!st.net.failsafe);
        assert_eq!(st.net.cooling_until, cooling);
    }

    #[test]
    fn the_deck_forgets_the_city_you_left() {
        // the deck's and the MAP's picks point into the city you were in
        let mut st = home_run();
        st.deck_map_to = Some(crate::track::ListingId::Building(citygen::BuildingId(5)));
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        assert!(st.deck_map_to.is_none());
    }

    #[test]
    fn stepping_between_overlapping_blocks_keeps_you() {
        // where two blocks overlap, walking the overworld across them goes into one
        // and back out, never losing you
        let mut st = home_run();
        let o = st.outside.as_ref().unwrap();
        let blocks = o.others();
        let pair = blocks.iter().enumerate().find_map(|(i, (a, ab))| {
            blocks[i + 1..]
                .iter()
                .find(|(_, bb)| ab.intersects(*bb))
                .map(|(b, bb)| (*a, *ab, *b, *bb))
        });
        let Some((a, ab, _, bb)) = pair else {
            panic!("no overlapping blocks in seed 3's world");
        };
        // a tile in both, entered from the west outside both
        let both = ab.iter().find(|p| {
            bb.contains(*p) && !ab.contains(p.offset(-1, 0)) && !bb.contains(p.offset(-1, 0))
        });
        let Some(t) = both else {
            return;
        };
        st.outside.as_mut().unwrap().at = At::Overworld(t.offset(-1, 0));
        let moved = moves::world_step(&mut st, citygen::Dir::E);
        let into = st.outside.as_ref().unwrap().here;
        assert!(
            matches!(moved, moves::Moved::City(_)) || into.is_none(),
            "{moved:?}"
        );
        assert!(st.city.is_some() && st.player.is_some());
        let _ = a;
        // back out and home: the home city comes back
        let home = st.outside.as_ref().unwrap().home();
        enter(&mut st, home).unwrap();
        assert!(st.away.is_none() && st.home.is_some());
    }

    #[test]
    fn the_story_waits_in_another_city() {
        let mut st = home_run();
        crate::missions::begin(&mut st);
        let story = format!("{:?}", st.missions);
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        crate::clock::run(
            &mut st,
            Some(30 * sim::TICKS_PER_MINUTE),
            crate::clock::PATHS_PER_FRAME,
        );
        assert!(st.missions.runs.is_empty());
        assert_eq!(format!("{:?}", st.away.as_ref().unwrap().missions), story);
    }

    #[test]
    fn time_runs_on_through_another_city_and_home() {
        let mut st = home_run();
        crate::clock::run(
            &mut st,
            Some(10 * sim::TICKS_PER_MINUTE),
            crate::clock::PATHS_PER_FRAME,
        );
        let (tick, minute) = (st.clock_tick(), st.minute_of_day());
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        assert_eq!(
            (st.clock_tick(), st.minute_of_day()),
            (tick, minute),
            "the same hour"
        );
        crate::clock::run(
            &mut st,
            Some(30 * sim::TICKS_PER_MINUTE),
            crate::clock::PATHS_PER_FRAME,
        );
        let (later, at) = (st.clock_tick(), st.minute_of_day());
        let home = st.outside.as_ref().unwrap().home();
        enter(&mut st, home).unwrap();
        assert_eq!(st.clock_tick(), later, "home has gone on too");
        assert_eq!(st.minute_of_day(), at);
    }

    #[test]
    fn a_run_saved_in_another_city_reopens_there() {
        let mut st = home_run();
        let city = st.city.clone();
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        let there = st.city.as_ref().unwrap().seed;
        let dir = crate::runs::testutil::Dir::new("away");
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        assert_eq!(back.city.as_ref().unwrap().seed, there);
        assert_eq!(back.outside.as_ref().unwrap().here, Some(s));
        let home = back.outside.as_ref().unwrap().home();
        enter(&mut back, home).unwrap();
        assert_eq!(back.city, city, "the home city as it was");
    }

    #[test]
    fn a_settlement_that_wont_build_leaves_you_outside() {
        let mut st = home_run();
        let city = st.city.clone();
        let (s, _) = beside_another(&st);
        let nowhere = Settlement {
            city: s.city,
            town: Some(99),
        };
        assert!(enter(&mut st, nowhere).is_err());
        assert_eq!(st.city, city);
        assert!(st.away.is_none() && st.outside.as_ref().unwrap().here.is_none());
    }

    #[test]
    fn a_town_is_walked_into() {
        let mut st = home_run();
        let town = st
            .outside
            .as_ref()
            .unwrap()
            .others()
            .into_iter()
            .find(|(s, _)| s.town.is_some())
            .map(|(s, _)| s)
            .expect("a town of its own in seed 3's world");
        enter(&mut st, town).unwrap();
        let site = st.city.as_ref().unwrap().params.site.clone().unwrap();
        assert_eq!((site.city, site.town), (town.city, town.town));
    }

    #[test]
    fn the_block_is_the_generators() {
        let city = sited();
        let o = Outside::new(city).unwrap();
        let site = city.params.site.as_ref().unwrap();
        assert_eq!(o.params, site.world, "the city's own world");
        assert_eq!(o.block(city), o.world().city_block(site.city));
    }

    #[test]
    fn walking_off_the_city_the_ground_goes_on() {
        // the cells along the city's edges are the open ground's own: walk off its map
        // and the ground goes on as it was
        let city = sited();
        let o = Outside::new(city).unwrap();
        let b = o.block(city);
        let (w, h) = (city.size.w as i32, city.size.h as i32);
        let ground = citygen::overworld::detail::region(
            o.world(),
            TILE * b.x,
            TILE * b.y,
            w as u32,
            h as u32,
        );
        let edge = (0..h)
            .flat_map(|y| [Pos::new(0, y), Pos::new(w - 1, y)])
            .chain((0..w).flat_map(|x| [Pos::new(x, 0), Pos::new(x, h - 1)]))
            .collect::<Vec<_>>();
        let same = edge
            .iter()
            .filter(|p| city.tile(StratumKind::Street, **p).unwrap().terrain == ground[**p].terrain)
            .count();
        assert_eq!(same, edge.len(), "every edge cell is the ground's");
    }

    #[test]
    fn an_old_save_keeps_its_block() {
        // a city from before cities stood in the world has no site: its world comes
        // from its seed and its block is centred on the home city, as it was
        let city = medium();
        assert!(city.params.site.is_none());
        let o = Outside::new(&city).unwrap();
        assert_eq!(o.params, world_params(city.seed));
        let b = o.block(&city);
        assert_eq!((b.w, b.h), (30, 15));
    }

    #[test]
    fn the_world_comes_from_the_citys_seed() {
        let city = medium();
        let (a, b) = (Outside::new(&city).unwrap(), Outside::new(&city).unwrap());
        assert_eq!(a.params.seed, city.seed);
        assert_eq!(a.world().tiles, b.world().tiles);
        assert_eq!(a.at, At::City);
    }

    #[test]
    fn the_home_block_is_the_city_at_its_size() {
        let city = medium();
        let o = Outside::new(&city).unwrap();
        let b = o.block(&city);
        assert_eq!((b.w, b.h), (30, 15));
        let w = o.world();
        assert!(b.x >= 0 && b.y >= 0);
        assert!(b.x + b.w <= w.size.w as i32 && b.y + b.h <= w.size.h as i32);
        // over the home city's own tiles
        let home = w.cities.iter().position(|c| c.home).unwrap() as u8;
        let mine = w
            .tiles
            .iter()
            .filter(|(_, t)| t.urban.is_some_and(|u| u.city == home))
            .count();
        let inside = w
            .tiles
            .iter()
            .filter(|(p, t)| b.contains(*p) && t.urban.is_some_and(|u| u.city == home))
            .count();
        assert!(
            inside * 10 >= mine * 8,
            "{inside} of {mine} home tiles in the block"
        );
    }

    #[test]
    fn a_block_against_the_worlds_edge_stays_inside() {
        let city = medium();
        let mut o = Outside::new(&city).unwrap();
        let w = o.world.as_mut().unwrap();
        let all: Vec<Pos> = w.tiles.iter().map(|(p, _)| p).collect();
        for p in all {
            w.tiles[p].urban = None;
        }
        let home = w.cities.iter().position(|c| c.home).unwrap() as u8;
        w.tiles[Pos::new(1, 1)].urban = Some(citygen::overworld::Urban {
            city: home,
            density: 200,
            theme: citygen::Theme::CorpCore,
        });
        let b = o.block(&city);
        assert_eq!((b.x, b.y, b.w, b.h), (0, 0, 30, 15));
    }

    #[test]
    fn city_cells_and_tiles_map_both_ways() {
        let city = medium();
        let o = Outside::new(&city).unwrap();
        let b = o.block(&city);
        for t in [
            Pos::new(b.x, b.y),
            Pos::new(b.x + b.w - 1, b.y),
            Pos::new(b.x, b.y + b.h - 1),
            Pos::new(b.x + b.w - 1, b.y + b.h - 1),
            Pos::new(b.x + b.w / 2, b.y + b.h / 2),
        ] {
            let cell = o.street_of(&city, t);
            let tile = city.tile(StratumKind::Street, cell).unwrap();
            assert!(
                tile.is_walkable() && tile.building.is_none(),
                "{t:?} → {cell:?}"
            );
        }
        // in the middle of the city a street is never far; at a corner it may be
        // across the bay
        let mid = Pos::new(b.x + b.w / 2, b.y + b.h / 2);
        let back = o.tile_of(&city, o.street_of(&city, mid));
        assert!(
            (back.x - mid.x).abs() <= 1 && (back.y - mid.y).abs() <= 1,
            "{back:?}"
        );
        for p in [Pos::new(0, 0), Pos::new(239, 119), Pos::new(100, 50)] {
            let t = o.tile_of(&city, p);
            assert!(b.contains(t), "{p:?} → {t:?}");
            assert_eq!(t, Pos::new(b.x + p.x / 8, b.y + p.y / 8));
        }
    }

    #[test]
    fn ensure_builds_once() {
        let mut st = AppState::new();
        assert!(ensure(&mut st).is_none(), "no city, no world");
        st.enter_city(medium());
        ensure(&mut st).unwrap().at = At::Overworld(Pos::new(3, 4));
        let o = ensure(&mut st).unwrap();
        assert_eq!(o.at, At::Overworld(Pos::new(3, 4)));
    }

    #[test]
    fn the_world_comes_back_from_a_save() {
        let mut st = AppState::new();
        st.enter_city(medium());
        let o = ensure(&mut st).unwrap();
        o.at = At::Overworld(Pos::new(40, 20));
        o.seen.insert(Pos::new(41, 20), 0xF0F0);
        let tiles = o.world().tiles.clone();
        let dir = crate::runs::testutil::Dir::new("outside");
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        let o = back.outside.as_ref().unwrap();
        assert_eq!(o.at, At::Overworld(Pos::new(40, 20)));
        assert_eq!(o.seen.get(&Pos::new(41, 20)), Some(&0xF0F0));
        assert_eq!(o.world().tiles, tiles);
    }

    #[test]
    fn a_run_keeps_its_worlds_size_in_tiles() {
        let o = Outside::new(&medium()).unwrap();
        let citygen::Size { w, h } = WorldParams::default().size.size();
        assert_eq!(
            o.params.size,
            citygen::overworld::WorldSize::Custom { w, h }
        );
        assert_eq!((o.world().size.w, o.world().size.h), (w, h));
    }

    #[test]
    fn a_new_city_forgets_the_world() {
        let mut st = AppState::new();
        st.enter_city(medium());
        ensure(&mut st);
        st.enter_city(medium());
        assert!(st.outside.is_none());
    }

    #[test]
    fn a_new_game_builds_the_large_city() {
        assert_eq!(AppState::new().params.size, SizePreset::Large);
    }
}
