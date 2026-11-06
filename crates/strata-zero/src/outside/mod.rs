//! The world beyond the city: the overworld, where you are on it, and what you have seen
//! of the open ground. The overworld comes from the city's seed and is rebuilt on load;
//! the home city sits on a block of its tiles (until the city grows into the world, the
//! whole block is the city). See `2026-11-04-overworld-screen-design.md`.

#[cfg(test)]
mod commuters;
pub mod ground;
pub mod here;
pub mod kept;
pub mod moves;
pub mod stream;

use crate::app::AppState;
use citygen::overworld::{Overworld, WorldParams};
use citygen::{City, Pos, Rect, StratumKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where you are: in the city (its street carries you off its map, onto the open ground,
/// walked on the city screen), or on an overworld tile. `Ground`, a detail cell of the
/// open ground (world-wide detail coordinates), is only read from a run saved before the
/// ground was the city screen's: it is put on the street there (`reopened`).
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
    /// What you have seen of the world off the live city's map (the open ground, and
    /// each city's street as you left it, remembered again as you walk in): per chunk of
    /// 8×8 detail cells (not per overworld tile, whose size may change), a bit a cell
    /// (`y * 8 + x`).
    pub seen: BTreeMap<Pos, u64>,
    /// The overworld, made from `params`; rebuilt on load (`after_restore`).
    #[serde(skip)]
    pub world: Option<Overworld>,
    /// The city or town the run's city is; none for the home city (and every run from
    /// before other cities could be walked into).
    #[serde(default)]
    pub here: Option<Settlement>,
    /// The other cities and towns built this session and not walked into yet, each the
    /// same from its world every time it's built (once walked into, one is `kept`).
    #[serde(skip)]
    pub built: BTreeMap<Settlement, City>,
    /// The other cities and towns you've been in, each kept whole: saved as all each
    /// holds but its map, made again from the world on reopening (`kept::remake`).
    /// The first saves have none.
    #[serde(default)]
    kept: kept::Kept,
    /// The other cities' and towns' blocks, from the world (`others`).
    #[serde(skip)]
    blocks: std::cell::OnceCell<Vec<(Settlement, Rect)>>,
    /// A place of the world marked on the deck MAP's world zoom: its way shows until
    /// you stand on it.
    #[serde(default)]
    pub mark: Option<Mark>,
    /// The cities and towns you've been into (their maps are made again from the world
    /// when you go back, the same every time).
    #[serde(default)]
    pub visited: std::collections::BTreeSet<Settlement>,
    /// Who owns each overworld tile, made with the world (`new`, and `rebuild` on
    /// load).
    #[serde(skip)]
    owners: Option<stream::Owners>,
    /// The open ground round the view, street and Net: never saved, made again from the
    /// world as it's needed.
    #[serde(skip)]
    pub stream: stream::Stream,
}

/// A marked place of the world: its tile and its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub tile: Pos,
    pub name: String,
}

/// A place on the overworld with a map of its own: a world city, or one of its towns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Settlement {
    pub city: u8,
    pub town: Option<u8>,
}

/// Everything a run holds of the city it is in: set aside while you're in another,
/// the home city's whole (`st.away`), any other's kept (`Outside.kept`), both saved with
/// the run (a kept one but its map, made again from the world).
#[derive(Serialize, Deserialize)]
pub struct Stay {
    pub city: City,
    /// You as this city last had you: what you'd seen of it, and, left on the Net, your
    /// body as it stands in it (inside at the rig too), its frame's.
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

    /// A city as you first come into it, with its people `world` (`seed_people`):
    /// nothing else yet.
    fn fresh(city: City, world: sim::World) -> Stay {
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
            // who its story cast stays in town: the sim's mark isn't saved
            crate::missions::cast::keep_in_town(w, &self.missions);
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
        let mut o = Outside {
            params,
            at: At::City,
            seen: BTreeMap::new(),
            world: Some(world),
            here: None,
            built: BTreeMap::new(),
            kept: Default::default(),
            blocks: Default::default(),
            mark: None,
            visited: Default::default(),
            owners: None,
            stream: Default::default(),
        };
        o.owners = Some(stream::Owners::of(&o, city));
        Ok(o)
    }

    pub fn world(&self) -> &Overworld {
        self.world
            .as_ref()
            .expect("the overworld is made with the run and on load")
    }

    /// Makes the overworld again from its params, and who owns its tiles with it, the
    /// home city `home` (after a load).
    pub fn rebuild(&mut self, home: Option<&City>) {
        if self.world.is_none() {
            self.world = Overworld::generate(self.params.clone()).ok();
        }
        if self.owners.is_none()
            && self.world.is_some()
            && let Some(home) = home
        {
            self.owners = Some(stream::Owners::of(self, home));
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

    /// Where the city's map sits in the world: the world cell of its cell `(0, 0)`, the
    /// corner of its block.
    pub fn origin(&self, city: &City) -> Pos {
        let b = self.block(city);
        Pos::new(TILE * b.x, TILE * b.y)
    }

    /// The overworld tile a cell of the city's street lies in, on its map or past its
    /// edge.
    pub fn tile_of(&self, city: &City, p: Pos) -> Pos {
        let at = self.origin(city).offset(p.x, p.y);
        Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE))
    }

    /// Where you stand in the city coming in on block tile `tile`: the outdoor street
    /// cell of its own nearest the tile's middle.
    pub fn street_of(&self, city: &City, tile: Pos) -> Pos {
        let b = self.block(city);
        street_near(
            city,
            Pos::new(
                TILE * (tile.x - b.x) + TILE / 2,
                TILE * (tile.y - b.y) + TILE / 2,
            ),
            |p| self.owns(city, p),
        )
    }

    /// The city or town `city` is: the one its site names, the home city for one from
    /// before the world.
    pub fn settlement_of(&self, city: &City) -> Settlement {
        match &city.params.site {
            Some(site) => Settlement {
                city: site.city,
                town: site.town,
            },
            None => self.home(),
        }
    }

    /// Whether `city` owns its cell `p`: the overworld tile it lies in is its own.
    pub fn owns(&self, city: &City, p: Pos) -> bool {
        self.owner(self.tile_of(city, p)) == Some(self.settlement_of(city))
    }

    /// The map of city or town `s` held this session (kept whole, or built and not
    /// walked into yet), if it is.
    pub fn held_city(&self, s: Settlement) -> Option<&City> {
        self.kept
            .stays
            .get(&s)
            .map(|k| &k.city)
            .or_else(|| self.built.get(&s))
    }

    /// Every city and town held this session but the one you're in and the home city
    /// set aside, each once: those kept whole, and those built and not walked into yet.
    pub fn held(&self) -> impl Iterator<Item = (Settlement, &City)> {
        self.kept
            .stays
            .iter()
            .map(|(s, k)| (*s, &k.city))
            .chain(self.built.iter().map(|(s, c)| (*s, c)))
    }

    /// The cities and towns kept, as a save read them, until their maps are made again
    /// (`kept::remake`).
    pub(crate) fn kept_saved_mut(&mut self) -> impl Iterator<Item = &mut kept::StayParts> {
        self.kept.saved_mut()
    }

    /// City or town `s` no longer held (a test's way to see the stream answer for it).
    #[cfg(test)]
    pub fn let_go(&mut self, s: Settlement) {
        self.built.remove(&s);
        self.kept.take(s);
        self.kept.built.remove(&s);
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

    /// What a city or town is called: the world's name for it, which a city or town
    /// built on its site carries as its own (`Overworld::settlement_name`).
    pub fn name(&self, s: Settlement) -> String {
        self.world().settlement_name(s.city, s.town)
    }
}

/// The outdoor street cell of `city` nearest `cell` (clamped into the city) of those
/// `own` keeps (the cells the city owns), else of any.
fn street_near(city: &City, cell: Pos, own: impl Fn(Pos) -> bool) -> Pos {
    let (cw, ch) = (city.size.w as i32, city.size.h as i32);
    let from = Pos::new(cell.x.clamp(0, cw - 1), cell.y.clamp(0, ch - 1));
    let open = |p: Pos| {
        city.tile(StratumKind::Street, p)
            .is_some_and(|t| t.is_walkable() && t.building.is_none())
    };
    nearest(from, cw.max(ch), |p| open(p) && own(p))
        .or_else(|| nearest(from, cw.max(ch), open))
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

/// The city or town whose overworld tile `tile` is: its owner, not whichever map covers
/// it (a town's own tiles inside its city's block are the town's); the map covering it
/// before who owns the world's tiles is known.
pub fn tile_owner(st: &AppState, tile: Pos) -> Option<Settlement> {
    st.outside
        .as_ref()?
        .owner(tile)
        .or_else(|| settlement_at(st, tile))
}

/// Whether overworld tile `tile` is on any city's or town's block (tests pick open
/// ground off them all).
#[cfg(test)]
pub(crate) fn in_any_block(st: &AppState, tile: Pos) -> bool {
    settlement_at(st, tile).is_some()
}

/// The overworld tile you're on: out there, the one you stand on; in a city, the one
/// its street there lies in.
pub fn your_tile(st: &AppState) -> Option<Pos> {
    let o = st.outside.as_ref()?;
    Some(match o.at {
        At::Overworld(t) => t,
        At::Ground(c) => Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)),
        At::City => {
            // indoors (or jacked in) the street cell you'd come out on
            let city = st.city.as_ref()?;
            let pos = crate::track::listings::walk_from(st, StratumKind::Street)
                .or_else(|| st.player.as_ref().map(|p| p.pos))?;
            o.tile_of(city, pos)
        }
    })
}

/// The mark's way from where you are: its name, how many tiles and which way.
pub fn mark_way(st: &AppState) -> Option<(String, i32, &'static str)> {
    mark_way_from(st, your_tile(st)?)
}

/// The mark's way from overworld tile `you`: its name, how many tiles and which way.
pub fn mark_way_from(st: &AppState, you: Pos) -> Option<(String, i32, &'static str)> {
    let m = st.outside.as_ref()?.mark.as_ref()?;
    let (dx, dy) = (m.tile.x - you.x, m.tile.y - you.y);
    let d = ((dx * dx + dy * dy) as f32).sqrt().round() as i32;
    Some((m.name.clone(), d, here::bearing(dx, dy)))
}

/// At the mark: you're there, and the mark goes. There is a tile from it, in the city
/// or town it marks, or, for a place you can't stand on (a rig at sea), beside the
/// nearest ground you can.
pub fn reached_mark(st: &mut AppState) {
    let there = (|| {
        let o = st.outside.as_ref()?;
        let m = o.mark.as_ref()?;
        let you = your_tile(st)?;
        let near = |a: Pos, b: Pos| (a.x - b.x).abs().max((a.y - b.y).abs()) <= 1;
        if near(you, m.tile) {
            return Some(true);
        }
        // in the city it marks, on its own cells
        if o.at == At::City && !away(st) && settlement_at(st, m.tile) == Some(o.here()) {
            return Some(true);
        }
        let w = o.world();
        if moves::step_cost(w, m.tile).is_none() {
            let shore = nearest(m.tile, 12, |p| moves::step_cost(w, p).is_some())?;
            return Some(near(you, shore));
        }
        Some(false)
    })()
    .unwrap_or(false);
    if there {
        let name = st
            .outside
            .as_mut()
            .and_then(|o| o.mark.take())
            .map(|m| m.name)
            .unwrap_or_default();
        st.say(format!("⌖ you're at {name}"));
    }
}

/// What city or town `s` of the run's world is built from, on `base` (the run's own
/// params): its seed from the world's and its place in it, the same every time, set on
/// its site. None when it has none (a town grown into its city, or no world).
pub fn build_params(
    o: &Outside,
    s: Settlement,
    base: &citygen::GenParams,
) -> Option<citygen::GenParams> {
    let w = o.world.as_ref()?;
    let base = citygen::GenParams {
        seed: citygen::rng::hash64(
            w.seed ^ 0xC17E,
            ((s.city as u64) << 8) | s.town.map_or(0, |t| t as u64 + 1),
        ) % 1_000_000_007,
        ..base.clone()
    };
    match s.town {
        None => Some(citygen::sited(w, s.city, base)),
        Some(t) => citygen::sited_town(w, s.city, t, base),
    }
}

/// The longest a city's people seeded by its build in the background are run on to the
/// hour you walk in (a day): older, they're seeded again for the hour, cheaper than
/// stepping every minute between on the frame.
pub const RUN_ON: u64 = 24 * 60 * sim::TICKS_PER_MINUTE;

/// `city`'s people as you first come into it at clock tick `now`, minute `minute` of
/// the day: each at their day.
pub fn seed_people(city: &City, people: &sim::People, now: u64, minute: u16) -> sim::World {
    let day = 24 * 60;
    let spent = (now / sim::TICKS_PER_MINUTE) % day;
    let start = (minute as u64 + day - spent) % day;
    sim::World::seed(city, people, &sim::SimParams::default(), start as u16, now)
}

/// Whether the run is in a city or town other than its home: the story waits there.
pub fn away_from_home(st: &AppState) -> bool {
    st.outside
        .as_ref()
        .is_some_and(|o| o.here.is_some_and(|h| h != o.home()))
}

/// Where `enter` takes you: home, set aside; a city kept whole from before; or one
/// built (in the background or on the spot) and walked into the first time.
enum Next {
    Home,
    Kept(Box<Stay>),
    Fresh(Box<City>),
}

/// Into city or town `s`: the one you're in set aside (with the run when it's the home
/// city, kept whole when not: `kept`); `s` as it was kept, or as its build in
/// the background left it (its people seeded there run on to now), else built and its
/// people seeded on the spot the first time this session. One that won't build leaves
/// you where you were.
pub fn enter(st: &mut AppState, s: Settlement) -> Result<(), String> {
    let Some(o) = st.outside.as_mut() else {
        return Err("there is no world out here".into());
    };
    let (here, home) = (o.here(), o.home());
    if s == here {
        return Ok(());
    }
    let name = o.name(s);
    // the home city only from where it's set aside; one that wouldn't build when
    // walked into isn't tried again this session
    if (s == home && st.away.is_none()) || st.city.is_none() || o.stream.refused(s) {
        return Err(format!("no way into {name}"));
    }
    // one building in the background is waited for: no slower than building it here
    o.stream.wait(s, &mut o.built);
    let next = if s == home {
        Next::Home
    } else if let Some(stay) = o.kept.take(s) {
        Next::Kept(Box::new(stay))
    } else if let Some(c) = o.built.remove(&s) {
        // built in the background as you neared it
        Next::Fresh(Box::new(c))
    } else {
        let built = build_params(o, s, &st.params).and_then(|p| citygen::Generator::run(p).ok());
        match built {
            Some(c) => Next::Fresh(Box::new(c)),
            None => {
                o.stream.refuse(s);
                return Err(format!("no way into {name}"));
            }
        }
    };
    // the hour carries over: the run's clock is the city's you're in; and what of the
    // Net's state is the deck's and your Net self's goes where you go
    let (now, minute) = (st.clock_tick(), st.minute_of_day());
    let deck = st.net.carried();
    // where you stand in the world (your Net self and your body, jacked in), and what
    // you've seen of the city you leave, kept as its frame goes; your bots come with you
    let (was, you) = leave(st);
    let bots = std::mem::take(&mut st.bots);
    let Some(current) = Stay::take(st) else {
        st.bots = bots;
        return Err("nowhere to go".into());
    };
    if here == home {
        st.away = Some(Box::new(current));
    } else if let Some(o) = st.outside.as_mut() {
        o.kept.keep(here, current);
    }
    match next {
        Next::Home => {
            if let Some(stay) = st.away.take() {
                stay.put(st, true, now);
            }
        }
        // as you left it, its people run on to now
        Next::Kept(stay) => stay.put(st, false, now),
        Next::Fresh(city) => {
            // its people as its build seeded them (run on to now as it's put in place)
            // if that was no more than `RUN_ON` ago, else seeded now
            let people = st
                .outside
                .as_mut()
                .and_then(|o| o.stream.take_people(s))
                .filter(|w| now.saturating_sub(w.tick()) <= RUN_ON);
            let world = people.unwrap_or_else(|| seed_people(&city, &st.people, now, minute));
            // its map as built: what a save measures its changes against
            let built = kept::Built::of(&city);
            if let Some(o) = st.outside.as_mut() {
                o.kept.built.insert(s, built);
            }
            Stay::fresh(*city, world).put(st, false, now);
        }
    }
    if let Some(o) = st.outside.as_mut() {
        o.here = (s != home).then_some(s);
        if s != home {
            o.visited.insert(s);
        }
    }
    st.net.carry(deck);
    arrive(st, was, you, bots);
    st.say(format!("into {name}"));
    Ok(())
}

/// Where you stand as the run's city changes, in world cells.
#[derive(Clone, Copy, Debug)]
enum Standing {
    /// On no stratum (no player, or inside with no body out).
    Nowhere,
    /// On foot on the street at this cell (jacked in off the Net's stratum, your body's).
    Foot(Pos),
    /// Your Net self on the Net at `at`, your body standing at `body` (as `stand_in`
    /// gives it, its cell a world cell).
    Net {
        at: Pos,
        body: (citygen::Place, Pos),
    },
}

/// Your body as another city's frame can hold it: on a stratum, as it stands; inside at
/// the flat's rig (`home`), by the rig's landing on the street, the port the cord runs
/// to. None for a body anywhere else.
fn stand_in(
    home: Option<crate::home::HomeRig>,
    body: (citygen::Place, Pos),
) -> Option<(citygen::Place, Pos)> {
    if body.0.stratum().is_some() {
        return Some(body);
    }
    let h = home.filter(|h| h.place() == body.0)?;
    Some((citygen::Place::Stratum(StratumKind::Street), h.landing))
}

/// The cities and towns held as you left them: the home city set aside, and every other
/// kept whole.
pub fn held_stays(st: &AppState) -> impl Iterator<Item = &Stay> {
    let kept = st.outside.iter().flat_map(|o| o.kept.stays.values());
    st.away.as_deref().into_iter().chain(kept)
}

/// `held_stays`, to change (their clock: `holds::tick`).
pub(crate) fn held_stays_mut(st: &mut AppState) -> impl Iterator<Item = &mut Stay> {
    let kept = st
        .outside
        .iter_mut()
        .flat_map(|o| o.kept.stays.values_mut());
    st.away.as_deref_mut().into_iter().chain(kept)
}

/// The people of city or town `s` held but not live: the home city's set aside, one
/// kept whole's, or those its build in the background seeded. None for the live city
/// (`st.world`) and one not held, or built with none.
pub fn people_of(st: &AppState, s: Settlement) -> Option<&sim::World> {
    let o = st.outside.as_ref()?;
    if s == o.here() {
        return None;
    }
    match stay_of(st, s) {
        Some(stay) => stay.world.as_ref(),
        None => o.stream.people(s),
    }
}

/// Jacked in, your body as another city's frame can hold it (`stand_in`), in the live
/// city's frame: none when you're not jacked in or it can't be held there.
pub(crate) fn body_stand_in(st: &AppState) -> Option<(citygen::Place, Pos)> {
    stand_in(st.home, st.player.as_ref()?.body?)
}

/// What city or town `s` holds as you left it, if it's held: the home city set aside,
/// or another kept whole.
fn stay_of(st: &AppState, s: Settlement) -> Option<&Stay> {
    let o = st.outside.as_ref()?;
    if s == o.home() {
        return st.away.as_deref().filter(|_| o.here() != s);
    }
    o.kept.stays.get(&s)
}

/// Jacked in with your body in a city or town your Net self has walked out of (handed
/// over on the Net, the body's cell off the live city's own): what that city holds as
/// you left it (its map, its backdoors, your body as it keeps it, inside at the rig
/// too) and the shift from its frame to the live city's. None while the body is in the
/// live city or on the open ground, or its city isn't held as you left it (one a
/// reopen couldn't make again: `held_city` has its map once it's built).
pub fn body_stay(st: &AppState) -> Option<(&Stay, Pos)> {
    let (place, body) = st.player.as_ref()?.body?;
    place.stratum()?;
    let (city, o) = (st.city.as_ref()?, st.outside.as_ref()?);
    let s = stream::owner_of(st, body).filter(|s| *s != o.here())?;
    let stay = stay_of(st, s)?;
    let (live, theirs) = (o.origin(city), o.origin(&stay.city));
    Some((stay, Pos::new(theirs.x - live.x, theirs.y - live.y)))
}

/// `body_stay`'s city, to change (a dump on a rig run starts its trace home there).
pub(crate) fn body_stay_mut(st: &mut AppState) -> Option<&mut Stay> {
    let s = {
        let (stay, _) = body_stay(st)?;
        st.outside.as_ref()?.settlement_of(&stay.city)
    };
    let o = st.outside.as_mut()?;
    if s == o.home() {
        return st.away.as_deref_mut();
    }
    o.kept.stays.get_mut(&s)
}

/// Jacked in with your body in a city or town your Net self has walked out of: the Net
/// tile you jacked in at there, in the live city's frame (the rig's landing for a body
/// at the flat's rig; else the terminal beside the body on that city's map). None while
/// the body is in the live city or on the open ground, or no map of its city is held.
pub fn held_port(st: &AppState) -> Option<Pos> {
    let (place, body) = st.player.as_ref()?.body?;
    let k = place.stratum()?;
    let (city, o) = (st.city.as_ref()?, st.outside.as_ref()?);
    let s = stream::owner_of(st, body).filter(|s| *s != o.here())?;
    let (map, shift) = match body_stay(st) {
        Some((stay, shift)) => {
            // a body at the rig stands in by the rig's landing: that's the port
            let inside = stay
                .player
                .as_ref()
                .and_then(|p| p.body)
                .is_some_and(|(pl, _)| pl.stratum().is_none());
            if inside {
                return Some(body);
            }
            (&stay.city, shift)
        }
        None => {
            let map = o.held_city(s)?;
            let (live, theirs) = (o.origin(city), o.origin(map));
            (map, Pos::new(theirs.x - live.x, theirs.y - live.y))
        }
    };
    let q = Pos::new(body.x - shift.x, body.y - shift.y);
    std::iter::once(q).chain(q.neighbors8()).find_map(|c| {
        map.connectors_at(c)
            .find(|j| j.kind == citygen::ConnectorKind::Jack && j.serves.contains(&k))
            .map(|j| j.pos.offset(shift.x, shift.y))
    })
}

/// Before the run's city changes: what was in sight past its edge and what you
/// remember of its street kept in the world's record. Where its map sits (the world cell
/// of its `(0, 0)`), and where you stand in the world: on foot (your body, jacked in off
/// the Net's stratum), or your Net self on the Net with your body where it stands.
fn leave(st: &mut AppState) -> (Pos, Standing) {
    stream::remember_sight(st);
    let home = st.home;
    let (Some(city), Some(o)) = (st.city.as_ref(), st.outside.as_mut()) else {
        return (Pos::new(0, 0), Standing::Nowhere);
    };
    let origin = o.origin(city);
    let Some(p) = st.player.as_ref() else {
        return (origin, Standing::Nowhere);
    };
    if let Some(vis) = p.vis_at(citygen::Place::Stratum(StratumKind::Street)) {
        ground::remember_map(o, origin, vis);
    }
    let world = |q: Pos| origin.offset(q.x, q.y);
    let net = p
        .body
        .filter(|_| p.place == citygen::Place::Stratum(StratumKind::Net));
    if let Some((place, at)) = net.and_then(|b| stand_in(home, b)) {
        return (
            origin,
            Standing::Net {
                at: world(p.pos),
                body: (place, world(at)),
            },
        );
    }
    let (place, pos) = p.body.unwrap_or((p.place, p.pos));
    let you = match place.stratum() {
        Some(_) => Standing::Foot(world(pos)),
        None => Standing::Nowhere,
    };
    (origin, you)
}

/// Into the run's new city, its map's corner having been at world cell `was` in the
/// last's frame: you where you stood (`Standing`, its cells world cells), in its frame
/// now, its street remembered as far as the world's record has seen it, your `bots`
/// where they were.
///
/// On foot, you're on the street on your cell; but on the cell your body waits on as
/// this city kept it (you left it jacked in, and jacked out back to it), you're in your
/// body where it is, inside at the rig too, and the rig's route ends as a jack out ends
/// it. Your Net self, you're on the Net on your cell with your body as this city kept
/// it when it's the one it stands in, else on its own cell past this city's.
fn arrive(st: &mut AppState, was: Pos, you: Standing, mut bots: Vec<crate::bots::BotOut>) {
    let (Some(city), Some(o)) = (st.city.as_ref(), st.outside.as_ref()) else {
        st.bots = bots;
        return;
    };
    let origin = o.origin(city);
    let into = |c: Pos| Pos::new(c.x - origin.x, c.y - origin.y);
    let street = citygen::Place::Stratum(StratumKind::Street);
    let net = citygen::Place::Stratum(StratumKind::Net);
    // where a body this city kept stands in the world, as `stand_in` takes it
    let home = st.home;
    let anchor = |b: (citygen::Place, Pos)| stand_in(home, b).map(|(_, q)| origin.offset(q.x, q.y));
    let mut woke = false;
    match you {
        Standing::Foot(at) => {
            let p = st
                .player
                .get_or_insert_with(|| crate::walker::Player::at(street, into(at)));
            let body = p.body.filter(|b| anchor(*b) == Some(at));
            (p.place, p.pos) = body.unwrap_or((street, into(at)));
            p.body = None;
            woke = body.is_some();
        }
        Standing::Net { at, body } => {
            let p = st
                .player
                .get_or_insert_with(|| crate::walker::Player::at(net, into(at)));
            let kept = p.body.filter(|b| anchor(*b) == Some(body.1));
            p.place = net;
            p.pos = into(at);
            p.body = Some(kept.unwrap_or((body.0, into(body.1))));
        }
        Standing::Nowhere => {}
    }
    if woke {
        st.backdoors.end_route();
    }
    if let Some(p) = st.player.as_mut() {
        let map = Rect::new(origin.x, origin.y, city.size.w as i32, city.size.h as i32);
        let seen = ground::seen_in(o, map);
        p.remember_place(street, city.size, seen.into_iter().map(into));
    }
    // the bots by your body: on a stratum where they were; one inside (by the rig) has
    // no cell this frame holds, and comes over to you as `bots::sync` brings it
    let shift = into(was);
    let by = st
        .player
        .as_ref()
        .map(|p| p.body.unwrap_or((p.place, p.pos)).0);
    for b in &mut bots {
        if b.place.stratum().is_none() {
            b.place = by.unwrap_or(street);
            b.pos = None;
            b.wake.clear();
            continue;
        }
        b.pos = b.pos.map(|q| q.offset(shift.x, shift.y));
        for (q, _) in &mut b.wake {
            *q = q.offset(shift.x, shift.y);
        }
    }
    st.bots = bots;
}

/// You on the live city's own street where it can't be stood on (a city built on the
/// spot as you stepped onto its ground may have a wall there): moved to the nearest cell
/// of its own street that can be.
pub(crate) fn footing(st: &mut AppState) {
    let to = {
        let (Some(city), Some(o), Some(p)) =
            (st.city.as_ref(), st.outside.as_ref(), st.player.as_ref())
        else {
            return;
        };
        if !p.place.is_street()
            || p.jacked_in()
            || !stream::live_owns(st, p.pos)
            || city.walkable(StratumKind::Street, p.pos)
        {
            return;
        }
        street_near(city, p.pos, |q| o.owns(city, q))
    };
    if let Some(p) = st.player.as_mut() {
        p.pos = to;
    }
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

/// You are out of the city: on the overworld, or standing (your body, jacked in) off
/// the cells the city owns: on the open ground, or in the city your Net self walked out
/// of. The city holds still for you (its story waits, its cameras look at no one), your
/// body isn't on its street.
pub fn away(st: &AppState) -> bool {
    let Some(o) = st.outside.as_ref() else {
        return false;
    };
    if o.at != At::City {
        return true;
    }
    st.player.as_ref().is_some_and(|p| {
        let (place, pos) = p.body.unwrap_or((p.place, p.pos));
        place.stratum().is_some() && !stream::live_owns(st, pos)
    })
}

/// After a run is opened: the overworld made again, and you back in the city if what
/// you stood on is no longer there (the world under a saved run can change between
/// builds until seeds are fixed for 1.0).
pub fn reopened(st: &mut AppState) {
    let Some(o) = st.outside.as_mut() else {
        return;
    };
    // the home city is the one set aside while you're in another
    o.rebuild(st.away.as_ref().map(|s| &s.city).or(st.city.as_ref()));
    let Some(w) = o.world.as_ref() else {
        // away from home, the home city comes back first: it's the one the run keeps
        if let Some(stay) = st.away.take() {
            let now = st.clock_tick();
            // as coming home does: the other city's cards go, the deck's own state stays
            let deck = st.net.carried();
            crate::credentials::purge(&mut st.content, &mut st.character);
            stay.put(st, true, now);
            st.net.carry(deck);
        }
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
        return;
    }
    // a run saved on the open ground before the street carried on past the city's
    // edge: on the street there, off the city's map
    if let At::Ground(c) = o.at {
        o.at = At::City;
        let Some(city) = st.city.as_ref() else {
            return;
        };
        let origin = o.origin(city);
        let street = citygen::Place::Stratum(StratumKind::Street);
        let p = st
            .player
            .get_or_insert_with(|| crate::walker::Player::at(street, c));
        p.place = street;
        p.pos = Pos::new(c.x - origin.x, c.y - origin.y);
        p.body = None;
        return;
    }
    still_stands(st);
}

/// A run reopened on the open ground past its city's edge, on foot, or jacked in with the
/// body or the Net self out there: if the world has changed under it since (until seeds
/// are fixed for 1.0) and the cell can't be stood on, back on the city's own street
/// nearest it, or, for the Net self alone, woken on the body.
fn still_stands(st: &mut AppState) {
    use citygen::Place;
    use citygen::overworld::{detail, net_ground};
    let (Some(city), Some(o), Some(p)) =
        (st.city.as_ref(), st.outside.as_ref(), st.player.as_ref())
    else {
        return;
    };
    let Some(w) = o.world.as_ref() else {
        return;
    };
    let origin = o.origin(city);
    let (cw, ch) = (TILE * w.size.w as i32, TILE * w.size.h as i32);
    let stands = |place: Place, q: Pos| {
        if place.stratum().is_none() || stream::live_owns(st, q) {
            return true;
        }
        let c = origin.offset(q.x, q.y);
        if c.x < 0 || c.y < 0 || c.x >= cw || c.y >= ch {
            return false;
        }
        // another city's or town's cell: its own map answers for it, made again as ever
        if o.owner(Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)))
            .is_some()
        {
            return true;
        }
        match place {
            Place::Stratum(StratumKind::Net) => {
                let t = net_ground::region(w, c.x, c.y, 1, 1)[Pos::new(0, 0)];
                t.is_walkable() || t.terrain == citygen::Terrain::Void
            }
            _ => detail::region(w, c.x, c.y, 1, 1)[Pos::new(0, 0)].is_walkable(),
        }
    };
    let jacked = p.body.is_some();
    let body = p.body.unwrap_or((p.place, p.pos));
    let body_stands = stands(body.0, body.1);
    let net_stands = !jacked || stands(p.place, p.pos);
    let back = street_near(city, body.1, |q| stream::live_owns(st, q));
    if body_stands && net_stands {
        return;
    }
    // out of the Net as a jack out takes you: the run, the mask and the rig's route
    // go with it
    if jacked {
        let _ = crate::net::jack_out(st);
    }
    if body_stands {
        st.warn("the world out there has changed since this run was saved · jacked out");
        return;
    }
    if let Some(p) = st.player.as_mut() {
        p.body = None;
        p.place = Place::Stratum(StratumKind::Street);
        p.pos = back;
    }
    st.warn("the world out there has changed since this run was saved · back in the city");
}

/// Whether you can leave the city for the world now: on foot on its open street (or on
/// the ground past it), in a city (not the lot), not jacked in, not in a fight, with
/// no call or scene under way.
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
    held_in(st)?;
    let outdoors = p.place.is_street()
        && (!stream::live_owns(st, p.pos)
            || city
                .tile(StratumKind::Street, p.pos)
                .is_some_and(|t| t.building.is_none()));
    if !outdoors {
        return Err("only from the open street");
    }
    Ok(())
}

/// Whether you can walk off the cells the city owns on foot now, or into another city's
/// or town's on foot or on the Net: in a city (not the lot), not in a fight, with no call
/// or scene under way; jacked in, with no run in flight (a run is its city's Net's, not
/// to be left hanging behind you).
pub fn can_cross(st: &AppState) -> Result<(), &'static str> {
    if st.city.as_ref().is_some_and(|c| c.is_lot()) {
        return Err("there is no world past the lot");
    }
    held_in(st)?;
    if st.net.run.is_some() && st.player.as_ref().is_some_and(|p| p.jacked_in()) {
        return Err("finish the run first");
    }
    Ok(())
}

/// What holds you in the city: a fight (on turns, or a foe hunting you where you
/// stand: over a line its hunters would hold still in the city left), a call, a scene.
fn held_in(st: &AppState) -> Result<(), &'static str> {
    if crate::runs::when::fighting(st) {
        return Err("not in a fight");
    }
    let m = &st.missions;
    if m.call.is_some() || !m.calls.is_empty() {
        return Err("not with a call on the line");
    }
    if m.scene.is_some() || !m.scenes.is_empty() {
        return Err("not now");
    }
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
pub(crate) mod tests {
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
    pub(crate) fn sited() -> &'static City {
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
    pub(crate) fn home_run() -> AppState {
        let mut st = AppState::new();
        st.enter_city(sited().clone());
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        ensure(&mut st);
        st
    }

    /// Another city of the world, and the overworld tile west of its block's middle
    /// row, a step east into it.
    pub(crate) fn beside_another(st: &AppState) -> (Settlement, Pos) {
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

    /// A run written and read back, after_restore and all.
    pub(crate) fn reopen(st: AppState, name: &str) -> AppState {
        let dir = crate::runs::testutil::Dir::new(name);
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        back
    }

    #[test]
    fn a_run_reopens_where_it_was_wherever_that_was() {
        // the overworld, the open ground, a town: back where it was saved
        let st = home_run();
        let (s, from) = beside_another(&st);
        let mut on_world = home_run();
        on_world.outside.as_mut().unwrap().at = At::Overworld(from);
        let back = reopen(on_world, "where-world");
        assert_eq!(back.outside.as_ref().unwrap().at, At::Overworld(from));
        let mut on_ground = home_run();
        let cell = Pos::new(TILE * from.x + 3, TILE * from.y + 3);
        on_ground.outside.as_mut().unwrap().at = At::Ground(cell);
        let ground_ok = citygen::overworld::detail::region(
            on_ground.outside.as_ref().unwrap().world(),
            cell.x,
            cell.y,
            1,
            1,
        )[Pos::new(0, 0)]
        .is_walkable();
        let back = reopen(on_ground, "where-ground");
        if ground_ok {
            // the ground is the street past the city's edge now
            let o = back.outside.as_ref().unwrap();
            assert_eq!(o.at, At::City);
            let origin = o.origin(back.city.as_ref().unwrap());
            assert_eq!(
                back.player.as_ref().unwrap().pos,
                Pos::new(cell.x - origin.x, cell.y - origin.y)
            );
        }
        let mut in_town = home_run();
        let town = in_town
            .outside
            .as_ref()
            .unwrap()
            .others()
            .into_iter()
            .find(|(t, _)| t.town.is_some())
            .unwrap()
            .0;
        enter(&mut in_town, town).unwrap();
        let back = reopen(in_town, "where-town");
        let site = back.city.as_ref().unwrap().params.site.clone().unwrap();
        assert_eq!((site.city, site.town), (town.city, town.town));
        assert!(back.away.is_some(), "home set aside");
        let _ = s;
    }

    #[test]
    fn an_old_save_on_the_ground_opens_on_the_street() {
        // a run saved out on the open ground before the street carried on past the
        // city's edge: back on the street there, off the city's map
        let mut st = home_run();
        let (e, d) = stream::tests::beyond::edge_out(&st, 1);
        let off = e.step(d);
        let city = st.city.clone().unwrap();
        let c = st
            .outside
            .as_ref()
            .unwrap()
            .origin(&city)
            .offset(off.x, off.y);
        st.outside.as_mut().unwrap().at = At::Ground(c);
        let back = reopen(st, "old-ground");
        assert_eq!(back.outside.as_ref().unwrap().at, At::City);
        let p = back.player.as_ref().unwrap();
        assert_eq!(
            (p.place, p.pos, p.body),
            (citygen::Place::Stratum(StratumKind::Street), off, None)
        );
        assert!(!city.size.contains(p.pos));
    }

    #[test]
    fn who_owns_the_world_comes_back_with_it() {
        // not saved: made again with the world, at home and away
        let st = home_run();
        let o = st.outside.as_ref().unwrap();
        let tiles: Vec<Pos> = o.world().tiles.iter().map(|(p, _)| p).collect();
        let owners = |st: &AppState| {
            let o = st.outside.as_ref().unwrap();
            tiles.iter().map(|t| o.owner(*t)).collect::<Vec<_>>()
        };
        let before = owners(&st);
        assert!(before.contains(&Some(o.home())));
        let back = reopen(home_run(), "owners-home");
        assert!(owners(&back) == before, "the owners come back");
        let mut away = home_run();
        let (s, _) = beside_another(&away);
        enter(&mut away, s).unwrap();
        let back = reopen(away, "owners-away");
        assert!(
            owners(&back) == before,
            "the owners come back away from home, the home city's block its own"
        );
    }

    #[test]
    fn the_cities_you_have_been_to_are_kept() {
        let mut st = home_run();
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        let home = st.outside.as_ref().unwrap().home();
        enter(&mut st, home).unwrap();
        assert!(st.outside.as_ref().unwrap().visited.contains(&s));
        let back = reopen(st, "visited");
        assert!(
            back.outside.as_ref().unwrap().visited.contains(&s),
            "kept with the run"
        );
    }

    #[test]
    fn a_world_that_wont_come_back_puts_you_home() {
        // away from home, a world that can't be made again on reopening: the home city
        // and its story back in place, not lost
        let mut st = home_run();
        crate::missions::begin(&mut st);
        let city = st.city.clone();
        let story = format!("{:?}", st.missions);
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        // a card to a door of that city, and the deck's own switch thrown there
        let card = crate::credentials::keycard_id(citygen::BuildingId(999), sim::HouseholdId(1));
        st.character.inventory.push(game::Stack {
            item: card.clone(),
            qty: 1,
            copy: None,
        });
        st.net.failsafe = false;
        st.outside.as_mut().unwrap().params.cities = 99;
        let back = reopen(st, "no-world");
        assert!(back.outside.is_none());
        assert!(back.away.is_none());
        assert_eq!(back.city, city);
        assert_eq!(format!("{:?}", back.missions), story);
        assert!(back.home.is_some(), "the flat back");
        assert!(
            back.character.inventory.iter().all(|s| s.item != card),
            "no card to the other city's doors"
        );
        assert!(!back.net.failsafe, "the deck's switch goes where you go");
    }

    #[test]
    fn the_runs_card_tells_the_home_story_while_away() {
        let mut st = home_run();
        crate::missions::begin(&mut st);
        let open = st
            .missions
            .runs
            .iter()
            .position(|r| r.state != crate::missions::RunState::Uncast)
            .unwrap();
        st.missions.runs[open].state = crate::missions::RunState::Open;
        st.missions.runs[open]
            .objectives
            .push("Find the doctor".into());
        let at_home = crate::runs::describe::story(&st);
        assert!(at_home.0.is_some());
        let (s, _) = beside_another(&st);
        enter(&mut st, s).unwrap();
        assert_eq!(crate::runs::describe::story(&st), at_home);
    }

    #[test]
    fn the_save_holds_the_worlds_params_not_its_tiles() {
        let st = home_run();
        let o = st.outside.as_ref().unwrap();
        let bytes = rmp_serde::to_vec_named(o).unwrap();
        let tiles = o.world().tiles.size().area();
        assert!(
            bytes.len() < tiles,
            "{} bytes for a world of {tiles} tiles",
            bytes.len()
        );
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
        assert!(back.home.is_none(), "no flat of yours in another city");
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
