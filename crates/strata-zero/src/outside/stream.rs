//! The open ground as one stream round the view: its street and its Net, built tile by
//! tile from the world, lit by the street's rule, and who owns each overworld tile.
//! See `2026-11-05-streamed-world-design.md`, "Areas and the stream".

use super::ground::GroundView;
use super::{Outside, Settlement, TILE};
use crate::app::AppState;
use crate::render::ConnDir;
use crate::render::map::TileMap;
use citygen::overworld::Overworld;
use citygen::{BuildingKind, City, Grid, Pos, Rect, Size, StratumKind, Terrain, Theme, Tile};
use std::collections::{BTreeMap, BTreeSet};

/// The open ground round the view, both its layers: the street's and the Net's, and the
/// cities and towns building as the view nears them. Not saved: everything in it is made
/// again from the world.
pub struct Stream {
    /// The street's layer, with its light (its windows' and its lamps').
    pub street: GroundView,
    /// The Net's layer.
    pub net: GroundView,
    /// The world's size in cells, from the last cover.
    cells: Size,
    /// The cities and towns building off the frame, and what came of them.
    builds: Builds,
}

impl Default for Stream {
    fn default() -> Stream {
        Stream {
            street: GroundView::default(),
            net: GroundView::net(),
            cells: Size { w: 0, h: 0 },
            builds: Builds::default(),
        }
    }
}

/// A clone starts empty: the stream is made again from the world as it's needed, and
/// the builds in flight stay with the one that started them.
impl Clone for Stream {
    fn clone(&self) -> Stream {
        Stream::default()
    }
}

impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("tiles", &(self.street.held() + self.net.held()))
            .field("building", &self.builds.flight.keys().collect::<Vec<_>>())
            .field("failed", &self.builds.failed)
            .finish()
    }
}

/// Cells past the view's rectangle a city or town starts building at: two overworld
/// tiles.
pub const REACH: i32 = 2 * TILE;

/// What a build makes: the city, and its people seeded; none when it wouldn't build.
type Made = Option<(City, sim::World)>;

/// What a build thread hands back: what it made, or what its panic said.
type Landed = Result<Made, String>;

thread_local! {
    /// On a build's thread, what its panic said once it has (kept by the front ends'
    /// panic hooks); none on any other thread.
    static BUILD: std::cell::RefCell<Option<Option<String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Whether this thread is a build's.
#[cfg(test)]
pub fn on_build_thread() -> bool {
    BUILD.with(|b| b.borrow().is_some())
}

/// For a front end's panic hook, first: on a build's thread, what the panic says is kept
/// for the main thread to say again when it collects the build, and true (the hook does
/// nothing more: no restoring the terminal or showing a box while the game goes on);
/// on any other thread, false.
pub fn held_by_build(info: &std::panic::PanicHookInfo<'_>) -> bool {
    BUILD.with(|b| match b.borrow_mut().as_mut() {
        Some(said) => {
            *said = Some(info.to_string());
            true
        }
        None => false,
    })
}

/// Runs `job` as a build's thread: a panic in it caught, and handed back as what it said
/// (the hook's words, where it broke among them, else the panic's own).
pub(super) fn build_thread<T>(job: impl FnOnce() -> T) -> Result<T, String> {
    BUILD.with(|b| *b.borrow_mut() = Some(None));
    let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
    let kept = BUILD.with(|b| b.borrow_mut().take().flatten());
    made.map_err(|panic| kept.unwrap_or_else(|| said(&*panic)))
}

/// The name of settlement `s`'s build's thread: `build 2`, `build 2.1` for its town 1.
pub(super) fn build_name(s: Settlement) -> String {
    match s.town {
        None => format!("build {}", s.city),
        Some(t) => format!("build {}.{t}", s.city),
    }
}

/// What a panic's payload says.
pub(super) fn said(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a build broke".into())
}

/// The cities and towns building off the frame, each on a thread of its own, and what
/// came of them.
#[derive(Default)]
struct Builds {
    /// Building.
    flight: BTreeMap<Settlement, std::thread::JoinHandle<Landed>>,
    /// The people each city built here was seeded with, until you walk in.
    people: BTreeMap<Settlement, sim::World>,
    /// Those that wouldn't build: not tried again this session.
    failed: BTreeSet<Settlement>,
    /// Those that wouldn't build when walked into either: not walked into again this
    /// session.
    refused: BTreeSet<Settlement>,
    /// Builds started this session.
    started: usize,
}

/// In a test no build outlives the run that started it: dropped, it waits for them.
/// In play they're let go, to finish unwanted.
#[cfg(test)]
impl Drop for Builds {
    fn drop(&mut self) {
        for (_, build) in std::mem::take(&mut self.flight) {
            let _ = build.join();
        }
    }
}

impl Stream {
    /// Starts a build of each of `wanted` not built (in `built`), building or failed:
    /// a settlement with its params (made from the run; none: it has no map), on a
    /// thread of its own that then seeds its people with `people` for clock tick `now`,
    /// minute `minute` of the day, as walking in seeds them. The builds finished are
    /// collected: each city into `built`, its people kept here. Whether a city came
    /// in.
    pub fn tend(
        &mut self,
        built: &mut BTreeMap<Settlement, City>,
        wanted: Vec<(Settlement, Option<citygen::GenParams>)>,
        people: &sim::People,
        now: u64,
        minute: u16,
    ) -> bool {
        for (s, params) in wanted {
            if !built.contains_key(&s) && !self.asked(s) {
                self.start(s, params, people, now, minute);
            }
        }
        self.collect(built, false)
    }

    /// Whether settlement `s` has been asked for and isn't to be again: building, or
    /// it wouldn't build.
    pub fn asked(&self, s: Settlement) -> bool {
        self.builds.flight.contains_key(&s) || self.builds.failed.contains(&s)
    }

    /// Settlement `s` building on a thread of its own, its people seeded after.
    fn start(
        &mut self,
        s: Settlement,
        params: Option<citygen::GenParams>,
        people: &sim::People,
        now: u64,
        minute: u16,
    ) {
        let Some(params) = params else {
            self.builds.failed.insert(s);
            return;
        };
        let people = people.clone();
        self.start_with(s, move || {
            let city = citygen::Generator::run(params).ok()?;
            let world = super::seed_people(&city, &people, now, minute);
            Some((city, world))
        });
    }

    /// Settlement `s` made by `job` on a thread of its own, named for it (`build 2`,
    /// `build 2.1` for its town 1).
    fn start_with(&mut self, s: Settlement, job: impl FnOnce() -> Made + Send + 'static) {
        let build = std::thread::Builder::new()
            .name(build_name(s))
            .spawn(move || build_thread(job));
        match build {
            Ok(build) => {
                self.builds.flight.insert(s, build);
                self.builds.started += 1;
            }
            // no thread to be had: walking in builds it on the spot
            Err(_) => {
                self.builds.failed.insert(s);
            }
        }
    }

    /// Settlement `s`'s build, if it is in flight, waited for and collected into
    /// `built`.
    pub fn wait(&mut self, s: Settlement, built: &mut BTreeMap<Settlement, City>) {
        if let Some(build) = self.builds.flight.remove(&s) {
            self.land(s, build.join(), built);
        }
    }

    /// Every build in flight waited for and collected into `built`. Whether a city came
    /// in.
    #[cfg(test)]
    pub fn finish(&mut self, built: &mut BTreeMap<Settlement, City>) -> bool {
        self.collect(built, true)
    }

    /// The builds finished (every one in flight, if `wait`) collected into `built`.
    /// Whether a city came in.
    fn collect(&mut self, built: &mut BTreeMap<Settlement, City>, wait: bool) -> bool {
        let done: Vec<Settlement> = self
            .builds
            .flight
            .iter()
            .filter(|(_, build)| wait || build.is_finished())
            .map(|(s, _)| *s)
            .collect();
        let mut came = false;
        for s in done {
            if let Some(build) = self.builds.flight.remove(&s) {
                came |= self.land(s, build.join(), built);
            }
        }
        came
    }

    /// What settlement `s`'s build came to: its city into `built` and its people kept,
    /// else remembered as one that won't build. Whether the city came in. A build that
    /// panicked panics here, on the main thread, with what it said, as building it on
    /// the spot would have: the front ends' panic hooks (which left the build's thread
    /// be) take it for the end of the game.
    fn land(
        &mut self,
        s: Settlement,
        landed: std::thread::Result<Landed>,
        built: &mut BTreeMap<Settlement, City>,
    ) -> bool {
        let made = landed.unwrap_or_else(|panic| Err(said(&*panic)));
        match made {
            Ok(Some((city, world))) => {
                // one held already keeps its own
                if built.contains_key(&s) {
                    return false;
                }
                built.insert(s, city);
                self.builds.people.insert(s, world);
                true
            }
            Ok(None) => {
                self.builds.failed.insert(s);
                false
            }
            Err(said) => panic!("building {s:?} broke: {said}"),
        }
    }

    /// The people seeded with settlement `s` when it was built here, taken to walk in
    /// with.
    pub fn take_people(&mut self, s: Settlement) -> Option<sim::World> {
        self.builds.people.remove(&s)
    }

    /// Settlement `s` wouldn't build when walked into: it isn't tried again this
    /// session, and its ground is walked as the open ground's.
    pub fn refuse(&mut self, s: Settlement) {
        self.builds.refused.insert(s);
    }

    /// Whether settlement `s` wouldn't build when walked into.
    pub fn refused(&self, s: Settlement) -> bool {
        self.builds.refused.contains(&s)
    }

    /// Whether settlement `s` is building.
    #[cfg(test)]
    pub fn building(&self, s: Settlement) -> bool {
        self.builds.flight.contains_key(&s)
    }

    /// How many builds were started.
    #[cfg(test)]
    pub fn started(&self) -> usize {
        self.builds.started
    }

    /// The people seeded with settlement `s` when it was built here, until you walk
    /// in.
    pub fn people(&self, s: Settlement) -> Option<&sim::World> {
        self.builds.people.get(&s)
    }

    /// `people`, to run on.
    fn people_mut(&mut self, s: Settlement) -> Option<&mut sim::World> {
        self.builds.people.get_mut(&s)
    }

    /// Both layers built over `area` (world cells) and a ring past it; far tiles
    /// forgotten.
    pub fn cover(&mut self, world: &Overworld, area: Rect) {
        self.cells = Size {
            w: TILE as u32 * world.size.w,
            h: TILE as u32 * world.size.h,
        };
        self.street.cover(world, area);
        self.net.cover(world, area);
    }
}

/// One layer of the stream as a map in world cells, seen at `minute` of the day: the
/// street's for any stratum but the Net.
#[derive(Clone, Copy)]
pub struct GroundMap<'a> {
    pub stream: &'a Stream,
    pub layer: StratumKind,
    pub minute: u16,
}

impl<'a> GroundMap<'a> {
    fn view(&self) -> &'a GroundView {
        match self.layer {
            StratumKind::Net => &self.stream.net,
            _ => &self.stream.street,
        }
    }

    /// The tile at world cell `p`, if it is built: held as long as the stream is.
    pub fn cell(&self, p: Pos) -> Option<&'a Tile> {
        self.view().tile(p)
    }
}

impl TileMap for GroundMap<'_> {
    fn size(&self) -> Size {
        self.stream.cells
    }

    fn tile(&self, p: Pos) -> Option<&Tile> {
        self.cell(p)
    }

    /// Ambient for the hour plus what the cell is lit by, as a city's stratum is lit.
    fn light(&self, p: Pos) -> u8 {
        citygen::lighting::ambient(self.layer, self.minute).saturating_add(self.emitted(p))
    }

    /// On the street, what burns all day and the lamps while they burn; on the Net, its
    /// grid's and its trunks' own.
    fn emitted(&self, p: Pos) -> u8 {
        match self.layer {
            StratumKind::Net => self.tile(p).map_or(0, |t| net_light(t.terrain)),
            _ => self.stream.street.emitted(p, self.lamps_on()),
        }
    }

    fn emits(&self, p: Pos) -> bool {
        self.tile(p).is_some_and(|t| {
            citygen::lighting::emits(t.terrain, t.feature, self.layer, self.lamps_on())
        })
    }

    fn daylight(&self) -> f32 {
        if citygen::lighting::sunlit(self.layer) {
            citygen::lighting::daylight(self.minute)
        } else {
            0.0
        }
    }

    fn lamps_on(&self) -> bool {
        citygen::lighting::lamps_on_at(self.layer, self.minute)
    }

    fn lit_sight(&self) -> bool {
        self.layer != StratumKind::Net
    }

    fn sky(&self) -> Option<((u8, u8, u8), f32)> {
        citygen::lighting::sky(self.layer, self.minute)
    }

    fn theme(&self, _p: Pos) -> Option<Theme> {
        None
    }

    fn kind(&self, _p: Pos) -> Option<BuildingKind> {
        None
    }

    fn connector(&self, _p: Pos) -> Option<ConnDir> {
        None
    }
}

/// The light the open Net's cells give, as a city's Net gives them past its outline
/// (citygen's `CyberspaceStage::on_its_site`): trunks and gates brightest, the grid's
/// dots, a construct's walls and floor dim, the void none.
fn net_light(terrain: Terrain) -> u8 {
    use citygen::lighting::tuning::{NET_GRID_LIGHT, NET_LANE_LIGHT};
    match terrain {
        Terrain::NetLane | Terrain::NetGate => NET_LANE_LIGHT,
        Terrain::NetGrid => NET_GRID_LIGHT,
        Terrain::NetWall | Terrain::NetFloor => 96,
        _ => 0,
    }
}

/// Who owns each overworld tile: the settlement whose map holds it, or none for the
/// open ground. Made from the world alone (and the home city's block), with the world.
#[derive(Clone)]
pub(super) struct Owners(Grid<Option<Settlement>>);

impl std::fmt::Debug for Owners {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let owned = self.0.count(|s| s.is_some());
        write!(f, "Owners({owned} tiles)")
    }
}

impl Owners {
    /// The owners of `o`'s world, `home` its home city. Every map is a settlement's
    /// block: the home city's, and every other's. Where several cover a tile it goes
    /// to the one whose own tiles lie nearest, the city before a town, then the lower
    /// city, then the lower town. A home city from before the world owns its whole
    /// block.
    pub(super) fn of(o: &Outside, home: &City) -> Owners {
        let w = o.world();
        let maps: Vec<(Settlement, Rect)> = std::iter::once((o.home(), o.block(home)))
            .chain(o.others())
            .collect();
        let mut covers = Grid::filled(w.size, 0u8);
        for (_, b) in &maps {
            for t in b.iter() {
                if let Some(n) = covers.get_mut(t) {
                    *n = n.saturating_add(1);
                }
            }
        }
        type Rank = (i64, bool, u8, Option<u8>);
        let mut best: Grid<Option<(Rank, Settlement)>> = Grid::filled(w.size, None);
        for &(s, b) in &maps {
            let own = match s.town {
                None => w.city_tiles(s.city),
                Some(t) => w.town_tiles(s.city, t),
            };
            for t in b.iter() {
                let Some(&n) = covers.get(t) else {
                    continue;
                };
                // alone on a tile it needs no rank
                let rank = if n > 1 {
                    let d = own.iter().map(|p| p.dist2(t)).min().unwrap_or(i64::MAX);
                    (d, s.town.is_some(), s.city, s.town)
                } else {
                    (0, false, 0, None)
                };
                if best[t].is_none_or(|(r, _)| rank < r) {
                    best[t] = Some((rank, s));
                }
            }
        }
        let mut owners = best.map(|b| b.as_ref().map(|(_, s)| *s));
        if home.params.site.is_none() {
            owners.fill_rect(o.block(home), Some(o.home()));
        }
        Owners(owners)
    }
}

impl Outside {
    /// Who owns overworld tile `tile`: the settlement whose map holds it, none on the
    /// open ground (or before the world is made).
    pub fn owner(&self, tile: Pos) -> Option<Settlement> {
        self.owners.as_ref()?.0.get(tile).copied().flatten()
    }
}

/// The street and the Net past the live city's edge, and the cells of its map other
/// areas own: for each cell of the live city's frame, the area that owns it answers.
/// Made from the run each time it's wanted; holds nothing of its own.
pub struct Beyond<'a> {
    /// Who owns each overworld tile.
    outside: &'a Outside,
    /// The live city and the world cell of its `(0, 0)`.
    live: (Settlement, &'a City, Pos),
    /// The other cities and towns held this session.
    held: Vec<Held<'a>>,
    stream: &'a Stream,
}

/// A city or town held this session but not live, as `Beyond` sees it.
#[derive(Clone, Copy)]
pub struct Held<'a> {
    pub settlement: Settlement,
    pub city: &'a City,
    /// The world cell of its `(0, 0)`.
    pub origin: Pos,
    /// Its people (`outside::people_of`): none for one built with none.
    pub people: Option<&'a sim::World>,
}

/// Who answers for a cell of the live city's frame that the live city doesn't own.
pub enum Owner<'a> {
    /// Another city or town held this session: its map, at its own cell.
    City(&'a City, Pos),
    /// The stream, at the world cell: the open ground, or a settlement not built yet.
    Ground(&'a Stream, Pos),
}

impl<'a> Beyond<'a> {
    /// What lies past the run's city: none with no world out there (the lot, a run
    /// before the world).
    pub fn of(st: &'a AppState) -> Option<Beyond<'a>> {
        let city = st.city.as_ref()?;
        let o = st.outside.as_ref()?;
        if city.is_lot() || o.world.is_none() || o.owners.is_none() {
            return None;
        }
        let (here, home) = (o.here(), o.home());
        let away = st
            .away
            .as_ref()
            .filter(|_| here != home)
            .map(|stay| (home, &stay.city));
        let held = away
            .into_iter()
            .chain(o.held())
            .filter(|(s, _)| *s != here)
            .map(|(s, city)| Held {
                settlement: s,
                city,
                origin: o.origin(city),
                people: super::people_of(st, s),
            })
            .collect();
        Some(Beyond {
            outside: o,
            live: (here, city, o.origin(city)),
            held,
            stream: &o.stream,
        })
    }

    /// Whether the live city owns local cell `p` (and its map has it).
    pub fn live_owns(&self, p: Pos) -> bool {
        self.elsewhere(p).is_none()
    }

    /// The world cell of the live city's `(0, 0)`.
    pub fn origin(&self) -> Pos {
        self.live.2
    }

    /// The other cities and towns held this session.
    pub fn held(&self) -> impl Iterator<Item = Held<'a>> + '_ {
        self.held.iter().copied()
    }

    /// The open ground round the view.
    pub fn stream(&self) -> &'a Stream {
        self.stream
    }

    /// The overworld tile local cell `p` lies in, and who owns it.
    fn tile_and_owner(&self, p: Pos) -> (Pos, Option<Settlement>) {
        let at = self.live.2.offset(p.x, p.y);
        let t = Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE));
        (t, self.outside.owner(t))
    }

    /// What the Net at local cell `p` is called, as HERE calls it (`here::net_name`):
    /// the city or town owning its tile, else the trunk, else the dead grid.
    pub fn net_name(&self, p: Pos) -> String {
        let (t, owner) = self.tile_and_owner(p);
        super::here::net_name(self.outside, owner, t)
    }

    /// What the street at local cell `p` is part of, as HERE names it
    /// (`here::street_name`): none on the open ground.
    pub fn street_name(&self, p: Pos) -> Option<String> {
        let (t, owner) = self.tile_and_owner(p);
        super::here::street_name(self.outside, owner, t)
    }

    /// Who answers for local cell `p` when the live city doesn't: the area owning its
    /// overworld tile, held, with the cell on its map; else the stream. None for the
    /// live city's own cells.
    pub fn elsewhere(&self, p: Pos) -> Option<Owner<'a>> {
        let (live, city, origin) = self.live;
        let at = origin.offset(p.x, p.y);
        let owner = self
            .outside
            .owner(Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE)));
        if owner == Some(live) && city.size.contains(p) {
            return None;
        }
        let theirs = owner.and_then(|s| {
            let h = self.held.iter().find(|h| h.settlement == s)?;
            let q = Pos::new(at.x - h.origin.x, at.y - h.origin.y);
            h.city.size.contains(q).then_some(Owner::City(h.city, q))
        });
        Some(theirs.unwrap_or(Owner::Ground(self.stream, at)))
    }
}

/// Whether the live city owns local cell `p`, as `Beyond::live_owns` says, without
/// making a `Beyond`: every cell of its map with no world past it (the lot, a run
/// before the world).
pub fn live_owns(st: &AppState, p: Pos) -> bool {
    let (Some(city), Some(o)) = (st.city.as_ref(), st.outside.as_ref()) else {
        return true;
    };
    if city.is_lot() || o.world.is_none() || o.owners.is_none() {
        return true;
    }
    if !city.size.contains(p) {
        return false;
    }
    // a city from before the world is the home city, and owns its whole block
    if city.params.site.is_none() {
        return true;
    }
    let at = o.origin(city).offset(p.x, p.y);
    o.owner(Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE))) == Some(o.here())
}

/// The city or town that owns the overworld tile the live city's cell `p` lies in, on
/// its map or past it: none on the open ground, and with no world out there.
pub fn owner_of(st: &AppState, p: Pos) -> Option<Settlement> {
    let (city, o) = (st.city.as_ref()?, st.outside.as_ref()?);
    if city.is_lot() {
        return None;
    }
    let at = o.origin(city).offset(p.x, p.y);
    o.owner(Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE)))
}

/// The stream built under `area` (cells of the live city's frame) wherever the live
/// city doesn't own a cell of it: the open ground, and every settlement's but the
/// live one's, built or not; and every city and town but the live one whose map comes
/// within `REACH` cells of `area` building in the background, those finished collected.
/// Whether it built anything new, or a city came in.
pub fn tend(st: &mut AppState, area: Rect) -> bool {
    let came = tend_builds(st, area);
    let Some(foreign) = Beyond::of(st).and_then(|b| b.foreign(area)) else {
        return came;
    };
    let Some(o) = st.outside.as_mut() else {
        return came;
    };
    let Some(world) = o.world.as_ref() else {
        return came;
    };
    let before = (o.stream.street.held(), o.stream.net.held());
    o.stream.cover(world, foreign);
    came || before != (o.stream.street.held(), o.stream.net.held())
}

/// Every city and town but the live one (and the home city, always held) whose map
/// comes within `REACH` cells of `area` (the live city's frame) building in the
/// background, its params made here, its people seeded for this hour; those finished
/// collected. Whether a city came in.
fn tend_builds(st: &mut AppState, area: Rect) -> bool {
    let wanted: Vec<_> = {
        let (Some(city), Some(o)) = (st.city.as_ref(), st.outside.as_ref()) else {
            return false;
        };
        if city.is_lot() || o.world.is_none() {
            return false;
        }
        let at = o.origin(city);
        let near = Rect::new(
            at.x + area.x - REACH,
            at.y + area.y - REACH,
            area.w + 2 * REACH,
            area.h + 2 * REACH,
        );
        let here = o.here();
        o.others()
            .into_iter()
            .filter(|(s, b)| {
                *s != here
                    && o.held_city(*s).is_none()
                    && !o.stream.asked(*s)
                    && near.intersects(Rect::new(TILE * b.x, TILE * b.y, TILE * b.w, TILE * b.h))
            })
            .map(|(s, _)| (s, super::build_params(o, s, &st.params)))
            .collect()
    };
    let (now, minute) = (st.clock_tick(), st.minute_of_day());
    let Some(o) = st.outside.as_mut() else {
        return false;
    };
    o.stream.tend(&mut o.built, wanted, &st.people, now, minute)
}

/// Cells past the view a held city's or town's map may lie and its people still live:
/// the people overlay's bubble.
pub const ALIVE: i32 = crate::render::people::MARGIN;

/// The most game minutes a held city's or town's people catch up on in a frame, come
/// back into view after a while out of it: a day spread over 24 frames.
pub const CATCH_UP: u64 = 60;

/// Further behind the clock than this (a game minute), or than the frame's own step if
/// that's longer, a held city's or town's people are catching up: a frame pays for one
/// of those at most.
const BEHIND: u64 = sim::TICKS_PER_MINUTE;

/// Every city and town held but the live one whose map comes within `ALIVE` cells of
/// `view` (cells of the live city's frame): its people's day run on to the live city's
/// clock and up to `paths` of the walks they've asked for found, as the live city's
/// are, nobody held back for you (their day only: no story, no cameras, no foes). Back
/// in view after a while, one catches up `CATCH_UP` minutes a frame; one built here and
/// not walked into whose people are more than `RUN_ON` behind has them seeded afresh,
/// as walking in would. Of those behind (by more than `step`, the ticks this frame's
/// clock ran, and `BEHIND`), only the one furthest behind catches up (or is seeded) in a
/// frame and the rest wait their turn, so the work never adds up across neighbours;
/// those that kept time step with the frame however long it is. Those out of view hold
/// still. How many ran.
pub fn run_neighbours(st: &mut AppState, view: Rect, paths: usize, step: u64) -> usize {
    let now = st.clock_tick();
    let near: Vec<(Settlement, u64)> = {
        let Some(b) = Beyond::of(st) else {
            return 0;
        };
        let live = b.origin();
        let wide = view.grow(ALIVE);
        b.held()
            .filter_map(|h| {
                let (w, hh) = (h.city.size.w as i32, h.city.size.h as i32);
                let map = Rect::new(h.origin.x - live.x, h.origin.y - live.y, w, hh);
                let people = h.people.filter(|_| wide.intersects(map))?;
                Some((h.settlement, now.saturating_sub(people.tick())))
            })
            .collect()
    };
    if near.is_empty() {
        return 0;
    }
    // whose turn it is to catch up: the furthest behind, the first of a tie
    let slack = step.max(BEHIND);
    let turn = near
        .iter()
        .filter(|(_, behind)| *behind > slack)
        .max_by_key(|(s, behind)| (*behind, std::cmp::Reverse(*s)))
        .map(|(s, _)| *s);
    let minute = st.minute_of_day();
    let people = &st.people;
    let mut away = st.away.as_deref_mut();
    let Some(o) = st.outside.as_mut() else {
        return 0;
    };
    let home = o.home();
    let mut ran = 0;
    for (s, behind) in near {
        if behind > slack && Some(s) != turn {
            continue;
        }
        let (city, world, built) = if s == home {
            let Some(stay) = away.as_deref_mut() else {
                continue;
            };
            (&stay.city, stay.world.as_mut(), false)
        } else if let Some(stay) = o.kept.stays.get_mut(&s) {
            (&stay.city, stay.world.as_mut(), false)
        } else {
            let Some(city) = o.built.get(&s) else {
                continue;
            };
            (city, o.stream.people_mut(s), true)
        };
        let Some(world) = world else {
            continue;
        };
        if built && behind > super::RUN_ON {
            *world = super::seed_people(city, people, now, minute);
        }
        world.set_blocker(None);
        if world.tick() < now {
            let most = world.tick() + CATCH_UP * sim::TICKS_PER_MINUTE;
            world.advance(city, now.min(most));
        }
        world.path_some(city, paths);
        ran += 1;
    }
    ran
}

/// The builds finished collected, wherever you are: indoors too, so one that broke
/// says so at once. Whether a city came in.
pub fn collect(st: &mut AppState) -> bool {
    let Some(o) = st.outside.as_mut() else {
        return false;
    };
    o.stream.collect(&mut o.built, false)
}

/// Every city and town building waited for and collected, so a test or a shot sees
/// what play shows once they're done. Whether a city came in.
#[cfg(test)]
pub fn finish(st: &mut AppState) -> bool {
    let Some(o) = st.outside.as_mut() else {
        return false;
    };
    o.stream.finish(&mut o.built)
}

impl Beyond<'_> {
    /// The overworld tiles `area` (the live city's frame) touches that hold a cell the
    /// live city doesn't own, as one rectangle of world cells; none when it owns all.
    fn foreign(&self, area: Rect) -> Option<Rect> {
        let (live, city, origin) = self.live;
        let at = Rect::new(origin.x + area.x, origin.y + area.y, area.w, area.h);
        let map = Rect::new(origin.x, origin.y, city.size.w as i32, city.size.h as i32);
        let (t0, t1) = (
            Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE)),
            Pos::new(
                (at.right() - 1).div_euclid(TILE),
                (at.bottom() - 1).div_euclid(TILE),
            ),
        );
        let mut bounds: Option<(Pos, Pos)> = None;
        for y in t0.y..=t1.y {
            for x in t0.x..=t1.x {
                let t = Pos::new(x, y);
                let own = self.outside.owner(t) == Some(live)
                    && map.contains(Pos::new(TILE * x, TILE * y))
                    && map.contains(Pos::new(TILE * x + TILE - 1, TILE * y + TILE - 1));
                if !own {
                    bounds = Some(match bounds {
                        None => (t, t),
                        Some((lo, hi)) => (
                            Pos::new(lo.x.min(x), lo.y.min(y)),
                            Pos::new(hi.x.max(x), hi.y.max(y)),
                        ),
                    });
                }
            }
        }
        let (lo, hi) = bounds?;
        Some(Rect::new(
            TILE * lo.x,
            TILE * lo.y,
            TILE * (hi.x - lo.x + 1),
            TILE * (hi.y - lo.y + 1),
        ))
    }
}

/// Cells round both ends a way over a map that carries on past its edge is looked for in,
/// when an end lies off the map.
pub const PATH_MARGIN: i32 = 24;

/// Where a way from `from` to `to` over a map of `size` is looked for: the map's own
/// cells when both ends are on it, as ever; else the box round both ends, `PATH_MARGIN`
/// cells wider all round, on the map or past its edge.
pub fn path_area(size: Size, from: Pos, to: Pos) -> Rect {
    if size.contains(from) && size.contains(to) {
        return Rect::new(0, 0, size.w as i32, size.h as i32);
    }
    let m = PATH_MARGIN;
    let (x0, y0) = (from.x.min(to.x) - m, from.y.min(to.y) - m);
    let (x1, y1) = (from.x.max(to.x) + m, from.y.max(to.y) + m);
    Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// The shortest way from `start` to `goal` four ways within `area` (cells of the live
/// city's frame, on its map or past it), as `citygen::pathing::astar` finds it on a map
/// of its own; `cost` is asked in the same frame.
pub fn path_within(
    area: Rect,
    start: Pos,
    goal: Pos,
    cost: impl Fn(Pos, Pos) -> Option<u32>,
) -> Option<Vec<Pos>> {
    let o = Pos::new(area.x, area.y);
    let size = Size {
        w: area.w.max(0) as u32,
        h: area.h.max(0) as u32,
    };
    let inner = |p: Pos| Pos::new(p.x - o.x, p.y - o.y);
    let outer = |p: Pos| Pos::new(p.x + o.x, p.y + o.y);
    let path = citygen::pathing::astar(size, inner(start), inner(goal), |a, b| {
        cost(outer(a), outer(b))
    })?;
    Some(path.into_iter().map(outer).collect())
}

/// On foot on the street, or your Net self on the Net, on a cell another city or town
/// owns: the run handed over to it in place (`enter`), you on the same world cell in its
/// frame, its street remembering what the world's record has seen of it, your bots
/// beside you, and your sight looked again. Jacked in, your body stays where it stands,
/// in the city you left (held now, not live), on its own world cell in the new frame;
/// jacked out onto it again, the step on foot hands that city back. The shift from the
/// old frame's cells to the new's (the old origin less the new), for the screen to move
/// what it holds by; none when nothing was handed over (in a construct or indoors, on
/// the live city's own cells or the open ground, a city that wouldn't build, or held in
/// the live city by a fight, a call, a scene or a run in flight).
pub fn hand_over(st: &mut AppState) -> Option<Pos> {
    let (s, was) = {
        let (Some(city), Some(o), Some(p)) =
            (st.city.as_ref(), st.outside.as_ref(), st.player.as_ref())
        else {
            return None;
        };
        let foot = p.place.is_street() && !p.jacked_in();
        // your Net self, with a body the next city's frame can hold
        let net = p.place == citygen::Place::Stratum(StratumKind::Net)
            && super::body_stand_in(st).is_some();
        if !(foot || net) || city.is_lot() || o.world.is_none() {
            return None;
        }
        let s = owner_of(st, p.pos)?;
        // the home city comes back only from where it's set aside
        let gone = s == o.home() && st.away.is_none();
        if s == o.here() || o.stream.refused(s) || gone {
            return None;
        }
        // what holds you in the city (a fight, a call, a scene, a run in flight) holds
        // you out of the next: the step onto it says so, a run put there waits
        if super::can_cross(st).is_err() {
            return None;
        }
        (s, o.origin(city))
    };
    if let Err(why) = super::enter(st, s) {
        st.warn(why);
        return None;
    }
    let now = st
        .city
        .as_ref()
        .zip(st.outside.as_ref())
        .map(|(city, o)| o.origin(city))?;
    super::footing(st);
    crate::screens::explore::look_round(st);
    Some(Pos::new(was.x - now.x, was.y - now.y))
}

/// Marks what was in sight past the live city's edge at the last look as seen, in the
/// world's record.
pub fn remember_sight(st: &mut AppState) {
    let (Some(city), Some(p), Some(o)) =
        (st.city.as_ref(), st.player.as_ref(), st.outside.as_mut())
    else {
        return;
    };
    let origin = o.origin(city);
    for c in p.beyond_sight() {
        super::ground::remember_cell(o, origin.offset(c.x, c.y));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::outside::ground::CAP;
    use crate::outside::tests::sited;
    use crate::render::map::StratumMap;
    use citygen::overworld::detail;
    use citygen::{Feature, GenParams, Generator, SizePreset};
    use std::sync::OnceLock;

    /// The night: the lamps burn.
    const NIGHT: u16 = 22 * 60;
    /// Noon: they don't.
    const NOON: u16 = 12 * 60;

    /// Seed 3's home city on its site, and its world.
    fn seed3() -> (&'static City, Outside) {
        let city = sited();
        (city, Outside::new(city).unwrap())
    }

    /// Every city and town of seed 3's world built on its site as the game builds it,
    /// the home city first: built once for every test.
    pub(crate) fn settlements() -> &'static [(Settlement, City)] {
        static ALL: OnceLock<Vec<(Settlement, City)>> = OnceLock::new();
        ALL.get_or_init(|| {
            let (home, o) = seed3();
            let base = AppState::new().params;
            let others = o.others().into_iter().map(|(s, _)| {
                let params = crate::outside::build_params(&o, s, &base);
                (s, Generator::run(params.unwrap()).unwrap())
            });
            std::iter::once((o.home(), home.clone()))
                .chain(others)
                .collect()
        })
    }

    /// The edges of `city`'s map with world past them, each as the strip of its cells
    /// within 2 of it.
    fn edges_to_the_world(o: &Outside, city: &City) -> Vec<Rect> {
        let origin = o.origin(city);
        let (w, h) = (city.size.w as i32, city.size.h as i32);
        let world = o.world().size;
        let (ww, wh) = (TILE * world.w as i32, TILE * world.h as i32);
        [
            (origin.y > 0, Rect::new(0, 0, w, 3)),
            (origin.y + h < wh, Rect::new(0, h - 3, w, 3)),
            (origin.x > 0, Rect::new(0, 0, 3, h)),
            (origin.x + w < ww, Rect::new(w - 3, 0, 3, h)),
        ]
        .into_iter()
        .filter_map(|(past, strip)| past.then_some(strip))
        .collect()
    }

    /// Every map of the world: the home city's block and every other settlement's.
    fn maps(o: &Outside, city: &City) -> Vec<(Settlement, Rect)> {
        std::iter::once((o.home(), o.block(city)))
            .chain(o.others())
            .collect()
    }

    /// A settlement's own tiles.
    fn own(o: &Outside, s: Settlement) -> Vec<Pos> {
        match s.town {
            None => o.world().city_tiles(s.city),
            Some(t) => o.world().town_tiles(s.city, t),
        }
    }

    /// A city from before the world, its world seed 4's: a town of its own inside
    /// another city's block, and a map reaching into its own.
    fn before_the_world() -> (City, Outside) {
        let city = Generator::run(GenParams {
            seed: 4,
            size: SizePreset::Medium,
            ..Default::default()
        })
        .unwrap();
        let o = Outside::new(&city).unwrap();
        (city, o)
    }

    #[test]
    fn every_tile_has_one_owner() {
        let (city, o) = seed3();
        let maps = maps(&o, city);
        let own: Vec<Vec<Pos>> = maps.iter().map(|(s, _)| own(&o, *s)).collect();
        let w = o.world();
        let (mut open, mut shared) = (0, 0);
        for (t, _) in w.tiles.iter() {
            let over: Vec<usize> = (0..maps.len()).filter(|&i| maps[i].1.contains(t)).collect();
            let want = match over.len() {
                0 => {
                    open += 1;
                    None
                }
                1 => Some(maps[over[0]].0),
                _ => {
                    shared += 1;
                    // the nearest own tiles; the city before a town, then the lower index
                    over.iter()
                        .min_by_key(|&&i| {
                            let s = maps[i].0;
                            let d = own[i].iter().map(|p| p.dist2(t)).min();
                            (d.unwrap_or(i64::MAX), s.town.is_some(), s.city, s.town)
                        })
                        .map(|&i| maps[i].0)
                }
            };
            assert_eq!(o.owner(t), want, "{t:?} under {over:?}");
        }
        assert!(open > 0 && shared > 0, "{open} open, {shared} shared");
        for (i, (s, _)) in maps.iter().enumerate() {
            for t in &own[i] {
                assert_eq!(o.owner(*t), Some(*s), "{s:?}'s own tile {t:?}");
            }
        }
    }

    #[test]
    fn a_town_owns_its_own_tiles_inside_its_citys_block() {
        // seed 3's towns all lie off their cities' blocks; seed 4's world has one inside
        let (_, o) = before_the_world();
        let w = o.world();
        let (town, inside) = o
            .others()
            .into_iter()
            .filter(|(s, _)| s.city != o.home().city)
            .filter_map(|(s, _)| {
                let block = w.city_block(s.city);
                let inside: Vec<Pos> = w
                    .town_tiles(s.city, s.town?)
                    .into_iter()
                    .filter(|p| block.contains(*p))
                    .collect();
                (!inside.is_empty()).then_some((s, inside))
            })
            .next()
            .expect("a town inside its city's block in seed 4's world");
        for t in inside {
            assert_eq!(o.owner(t), Some(town), "{t:?}");
        }
    }

    #[test]
    fn a_city_from_before_the_world_owns_its_whole_block() {
        let (city, o) = before_the_world();
        let block = o.block(&city);
        let others = o.others();
        let contested = block
            .iter()
            .filter(|t| others.iter().any(|(_, b)| b.contains(*t)))
            .count();
        assert!(contested > 0, "another map reaches into the block");
        for t in block.iter() {
            assert_eq!(o.owner(t), Some(o.home()), "{t:?}");
        }
    }

    #[test]
    fn the_ground_past_a_city_is_the_citys_own_ground() {
        // every city and town of seed 3's world: along each edge of its map with world
        // past it, its cells with no district are the stream's, street and Net
        let o = seed3().1;
        let mut checked = BTreeMap::new();
        let mut off = Vec::new();
        for (s, city) in settlements() {
            let origin = o.origin(city);
            let mut stream = Stream::default();
            // each strip covered and checked in turn (the stream forgets what's far)
            for strip in edges_to_the_world(&o, city) {
                stream.cover(
                    o.world(),
                    Rect::new(origin.x + strip.x, origin.y + strip.y, strip.w, strip.h),
                );
                for p in strip.iter() {
                    for layer in [StratumKind::Street, StratumKind::Net] {
                        let mine = city.tile(layer, p).unwrap();
                        if mine.district.is_some() {
                            continue;
                        }
                        let ground = GroundMap {
                            stream: &stream,
                            layer,
                            minute: NIGHT,
                        };
                        let at = origin.offset(p.x, p.y);
                        let theirs = ground.tile(at).expect("the stream covers the edge");
                        *checked.entry(layer.label()).or_insert(0) += 1;
                        if (mine.terrain, mine.feature) != (theirs.terrain, theirs.feature) {
                            off.push((*s, p, layer, mine.terrain, mine.feature, theirs.terrain));
                        }
                        // and the open Net lights as the city's Net does past its outline
                        let lit = StratumMap {
                            city,
                            stratum: layer,
                            minute: NIGHT,
                            beyond: None,
                        };
                        if layer == StratumKind::Net && mine.terrain == theirs.terrain {
                            assert_eq!(ground.light(at), lit.light(p), "{s:?} {p:?} {mine:?}");
                        }
                    }
                }
            }
        }
        assert!(
            checked.len() == 2 && checked.values().all(|n| *n > 10_000),
            "{checked:?}"
        );
        assert!(
            off.is_empty(),
            "{} of {checked:?} cells differ, e.g. {:?}",
            off.len(),
            &off[..off.len().min(12)]
        );
    }

    #[test]
    fn a_ground_lamp_lights_as_the_citys_lamps_do() {
        // every city and town of seed 3's world: off its map's edge and out of reach of
        // its own lamps (no district cell within 8), the open ground's lamps and windows
        // light as the city lights them, by night and by day
        assert!(citygen::lighting::lamps_on(NIGHT) && !citygen::lighting::lamps_on(NOON));
        let o = seed3().1;
        let (mut cells, mut lit, mut off) = (0, 0, Vec::new());
        for (s, city) in settlements() {
            let origin = o.origin(city);
            let (w, h) = (city.size.w as i32, city.size.h as i32);
            // the district cells round a cell, from a summed-area table
            let mut sum = vec![0i32; ((w + 1) * (h + 1)) as usize];
            let at = |x: i32, y: i32| (y * (w + 1) + x) as usize;
            for y in 0..h {
                for x in 0..w {
                    let d = city
                        .tile(StratumKind::Street, Pos::new(x, y))
                        .is_some_and(|t| t.district.is_some()) as i32;
                    sum[at(x + 1, y + 1)] =
                        d + sum[at(x, y + 1)] + sum[at(x + 1, y)] - sum[at(x, y)];
                }
            }
            let districts_near = |p: Pos, r: i32| {
                let (x0, y0) = ((p.x - r).max(0), (p.y - r).max(0));
                let (x1, y1) = ((p.x + r + 1).min(w), (p.y + r + 1).min(h));
                sum[at(x1, y1)] - sum[at(x0, y1)] - sum[at(x1, y0)] + sum[at(x0, y0)] > 0
            };
            // checked a chunk at a time (the stream forgets what's far)
            let chunk = 8 * TILE;
            let mut chunks: BTreeMap<(i32, i32), Vec<Pos>> = BTreeMap::new();
            for p in Rect::new(10, 10, w - 20, h - 20).iter() {
                if !districts_near(p, 8) {
                    chunks
                        .entry((p.x.div_euclid(chunk), p.y.div_euclid(chunk)))
                        .or_default()
                        .push(p);
                }
            }
            let mut stream = Stream::default();
            for ((cx, cy), ps) in chunks {
                stream.cover(
                    o.world(),
                    Rect::new(origin.x + cx * chunk, origin.y + cy * chunk, chunk, chunk),
                );
                for minute in [NIGHT, NOON] {
                    let mine = StratumMap {
                        city,
                        stratum: StratumKind::Street,
                        minute,
                        beyond: None,
                    };
                    let ground = GroundMap {
                        stream: &stream,
                        layer: StratumKind::Street,
                        minute,
                    };
                    for &p in &ps {
                        let at = origin.offset(p.x, p.y);
                        let (a, b) = (mine.emitted(p), ground.emitted(at));
                        cells += usize::from(minute == NIGHT);
                        lit += usize::from(minute == NIGHT && a > 0);
                        if a.abs_diff(b) > 1 || mine.emits(p) != ground.emits(at) {
                            off.push((*s, p, minute, a, b));
                        }
                    }
                }
            }
        }
        assert!(
            off.is_empty(),
            "{} of {cells} cells differ (minute, city, ground), e.g. {:?}",
            off.len(),
            &off[..off.len().min(12)]
        );
        assert!(lit > 100, "{lit} of {cells} cells lit by the cities' lamps");
    }

    /// The lamp light at `p` a stream holds, at night.
    fn lamp_at(s: &Stream, p: Pos) -> u8 {
        GroundMap {
            stream: s,
            layer: StratumKind::Street,
            minute: NIGHT,
        }
        .emitted(p)
    }

    #[test]
    fn a_tile_lights_the_same_whatever_built_it() {
        let (_, o) = seed3();
        let w = o.world();
        // a tile of the open ground with a lamp on it
        let t = w
            .tiles
            .iter()
            .map(|(p, _)| p)
            .filter(|p| p.x > 8 && p.y > 8 && o.owner(*p).is_none())
            .find(|p| {
                detail::ground(w, p.x, p.y)
                    .iter()
                    .any(|(_, c)| c.feature == Some(Feature::Lamp))
            })
            .expect("a lamp on the open ground");
        let round = |r: i32| {
            Rect::new(
                TILE * (t.x - r),
                TILE * (t.y - r),
                TILE * (2 * r + 1),
                TILE * (2 * r + 1),
            )
        };
        let mut small = Stream::default();
        small.cover(w, round(1));
        let mut big = Stream::default();
        big.cover(w, round(3));
        // and one walked in from the west, each step building a column of its own
        let mut walked = Stream::default();
        for k in -4..=1 {
            walked.cover(w, Rect::new(TILE * (t.x + k), TILE * t.y, TILE, TILE));
        }
        let middle = Rect::new(TILE * t.x, TILE * t.y, TILE, TILE);
        assert!(
            middle.iter().any(|p| lamp_at(&small, p) > 0),
            "the lamp lights its tile"
        );
        for p in middle.iter() {
            assert_eq!(lamp_at(&small, p), lamp_at(&big, p), "{p:?}");
        }
        // every tile either built, to its edges, lights as the big one's
        for s in [&small, &walked] {
            for p in round(3).iter().filter(|p| s.street.tile(*p).is_some()) {
                assert_eq!(lamp_at(s, p), lamp_at(&big, p), "{p:?}");
            }
        }
    }

    #[test]
    fn a_copy_of_the_world_streams_afresh() {
        let (city, mut o) = seed3();
        let origin = o.origin(city);
        let area = Rect::new(origin.x, origin.y, TILE, TILE);
        o.stream.cover(o.world.as_ref().unwrap(), area);
        let held = o.stream.street.held() + o.stream.net.held();
        assert!(held > 0);
        assert!(format!("{o:?}").contains(&format!("Stream {{ tiles: {held}, ")));
        let copy = o.clone();
        assert_eq!(copy.stream.street.held() + copy.stream.net.held(), 0);
        let mine = o.world().city_tiles(o.home().city)[0];
        assert_eq!(
            copy.owner(mine),
            Some(o.home()),
            "who owns what comes along"
        );
    }

    #[test]
    fn the_stream_forgets_far_tiles() {
        let (_, o) = seed3();
        let w = o.world();
        let mut stream = Stream::default();
        // east along a row, then south: 400 tiles, four at a stride
        let path = (2..=312)
            .step_by(4)
            .map(|x| Pos::new(x, 10))
            .chain((10..=100).step_by(4).map(|y| Pos::new(312, y)));
        let start = Pos::new(TILE * 2, TILE * 10);
        for t in path {
            let at = Pos::new(TILE * t.x + TILE / 2, TILE * t.y + TILE / 2);
            stream.cover(w, Rect::new(at.x - 40, at.y - 40, 81, 81));
            assert!(
                stream.street.held() <= CAP && stream.net.held() <= CAP,
                "{stream:?} at {t:?}"
            );
            assert!(stream.street.tile(at).is_some() && stream.net.tile(at).is_some());
        }
        assert!(
            stream.street.tile(start).is_none(),
            "the start is forgotten"
        );
    }

    /// The street and the Net carried on past the live city's edge.
    pub(crate) mod beyond {
        use super::*;
        use crate::app::AppState;
        use crate::outside::enter;
        use crate::outside::ground::remembered;
        use crate::outside::tests::home_run;
        use crate::render::map::MapRef;
        use crate::walker::Player;
        use citygen::{Dir, Place};

        /// The world cell of the live city's cell `p`.
        fn world_cell(st: &AppState, p: Pos) -> Pos {
            let o = st.outside.as_ref().unwrap();
            o.origin(st.city.as_ref().unwrap()).offset(p.x, p.y)
        }

        /// The overworld tile of world cell `c`.
        fn tile_of(c: Pos) -> Pos {
            Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE))
        }

        /// The stream built `r` cells round the live city's cell `p`.
        fn cover_round(st: &mut AppState, p: Pos, r: i32) {
            let at = world_cell(st, p);
            let o = st.outside.as_mut().unwrap();
            o.stream.cover(
                o.world.as_ref().unwrap(),
                Rect::new(at.x - r, at.y - r, 2 * r + 1, 2 * r + 1),
            );
        }

        /// A street cell on the live city's edge with open ground to walk on past it,
        /// and the way off.
        fn edge(st: &AppState) -> (Pos, Dir) {
            edge_out(st, 1)
        }

        /// A street cell of the live city's own on its edge with `len` cells of open
        /// ground to walk on straight out past it, and the way off.
        pub(crate) fn edge_out(st: &AppState, len: i32) -> (Pos, Dir) {
            let city = st.city.as_ref().unwrap();
            let o = st.outside.as_ref().unwrap();
            let w = o.world();
            let (cw, ch) = (city.size.w as i32, city.size.h as i32);
            let sides = (0..ch)
                .flat_map(|y| [(Pos::new(0, y), Dir::W), (Pos::new(cw - 1, y), Dir::E)])
                .chain(
                    (0..cw).flat_map(|x| [(Pos::new(x, 0), Dir::N), (Pos::new(x, ch - 1), Dir::S)]),
                );
            sides
                .filter(|(p, _)| {
                    city.walkable(StratumKind::Street, *p)
                        && o.owner(tile_of(world_cell(st, *p))) == Some(o.here())
                })
                .find(|(p, d)| {
                    let (dx, dy) = d.delta();
                    (1..=len).all(|i| {
                        let off = world_cell(st, p.offset(dx * i, dy * i));
                        w.tile(tile_of(off)).is_some()
                            && o.owner(tile_of(off)).is_none()
                            && detail::region(w, off.x, off.y, 1, 1)[Pos::new(0, 0)].is_walkable()
                    })
                })
                .expect("a way off the city onto open ground")
        }

        /// Settlement `s` built and held this session, as walking in and back out
        /// leaves it, the home city live again.
        fn hold(st: &mut AppState, s: Settlement) {
            let home = st.outside.as_ref().unwrap().home();
            enter(st, s).unwrap();
            enter(st, home).unwrap();
            assert!(st.outside.as_ref().unwrap().held_city(s).is_some());
        }

        #[test]
        fn the_street_walks_off_the_city_and_back() {
            let mut st = home_run();
            let (e, d) = edge(&st);
            cover_round(&mut st, e, 16);
            let city = st.city.as_ref().unwrap();
            let b = Beyond::of(&st).unwrap();
            let bare = MapRef::stratum_at(city, StratumKind::Street, NIGHT);
            let map = bare.beyond(&b);
            let off = e.step(d);
            assert!(!city.size.contains(off));
            let mut p = Player::at(Place::Stratum(StratumKind::Street), e);
            assert!(
                !p.try_move(&bare, d),
                "without the beyond the edge stops you"
            );
            assert_eq!(p.pos, e);
            assert!(p.try_move(&map, d), "off the city onto the ground");
            assert_eq!(p.pos, off);
            assert!(b.live_owns(e) && !b.live_owns(off));
            let ground = GroundMap {
                stream: o_stream(&st),
                layer: StratumKind::Street,
                minute: NIGHT,
            };
            assert_eq!(map.tile(off), ground.tile(world_cell(&st, off)));
            assert!(p.try_move(&map, d.opposite()), "and back");
            assert_eq!(p.pos, e);
            // the undercity keeps to the city's own map
            let under = MapRef::stratum_at(city, StratumKind::Undercity, NIGHT).beyond(&b);
            assert!(matches!(under, MapRef::Stratum(m) if m.beyond.is_none()));
            assert!(!under.reaches(off));
        }

        fn o_stream(st: &AppState) -> &Stream {
            &st.outside.as_ref().unwrap().stream
        }

        #[test]
        fn sight_crosses_the_edge() {
            let mut st = home_run();
            let (e, _) = edge(&st);
            cover_round(&mut st, e, 72);
            let mut p = Player::at(Place::Stratum(StratumKind::Street), e);
            let city = st.city.as_ref().unwrap();
            let bare = MapRef::stratum_at(city, StratumKind::Street, NOON);
            {
                let b = Beyond::of(&st).unwrap();
                let map = bare.beyond(&b);
                p.update_fov(&map, 0);
                let off = p.beyond_sight();
                assert!(off.len() > 100, "{} cells past the edge seen", off.len());
                for c in off {
                    assert!(!city.size.contains(*c), "{c:?} is off the map");
                    assert!(map.reaches(*c) && !b.live_owns(*c), "{c:?}");
                    assert!(
                        o_stream(&st).street.tile(world_cell(&st, *c)).is_some(),
                        "{c:?} is the ground's"
                    );
                }
            }
            let seen = p.beyond_sight().to_vec();
            // a look without the beyond sees nothing past the edge
            let mut q = p.clone();
            q.update_fov(&bare, 0);
            assert!(q.beyond_sight().is_empty());
            // a cell past the edge out of sight
            let unseen = Rect::new(e.x - 100, e.y - 100, 201, 201)
                .iter()
                .find(|c| !city.size.contains(*c) && !seen.contains(c))
                .unwrap();
            st.player = Some(p);
            remember_sight(&mut st);
            let o = st.outside.as_ref().unwrap();
            for c in &seen {
                assert!(remembered(o, world_cell(&st, *c)), "{c:?} remembered");
            }
            assert!(
                !remembered(o, world_cell(&st, unseen)),
                "{unseen:?} wasn't in sight"
            );
        }

        #[test]
        fn the_nets_void_is_walked_and_walls_are_not() {
            let mut st = home_run();
            let (e, d) = edge(&st);
            cover_round(&mut st, e.step(d), 24);
            let city = st.city.as_ref().unwrap();
            let b = Beyond::of(&st).unwrap();
            let bare = MapRef::stratum_at(city, StratumKind::Net, NIGHT);
            let map = bare.beyond(&b);
            let net = &city.stratum(StratumKind::Net).unwrap().tiles;
            // the city's own Net past its outline
            let void = net
                .iter()
                .find(|(p, t)| t.terrain == Terrain::Void && b.live_owns(*p))
                .map(|(p, _)| p)
                .expect("a void cell of the city's Net");
            assert!(
                !bare.walkable(void),
                "the city's Net alone keeps to its grid"
            );
            assert!(map.walkable(void), "{void:?}");
            // the open Net past the edge
            let open = Rect::new(e.x - 24, e.y - 24, 49, 49)
                .iter()
                .filter(|p| !city.size.contains(*p))
                .find(|p| map.tile(*p).is_some_and(|t| t.terrain == Terrain::Void))
                .expect("a void cell of the open Net past the edge");
            assert!(
                std::ptr::eq(
                    map.tile(open).unwrap(),
                    o_stream(&st).net.tile(world_cell(&st, open)).unwrap()
                ),
                "the stream's"
            );
            assert!(map.walkable(open) && map.reaches(open), "{open:?}");
            let wall = net
                .iter()
                .find(|(_, t)| t.terrain == Terrain::NetWall)
                .map(|(p, _)| p)
                .expect("a construct's wall");
            assert!(!map.walkable(wall), "{wall:?}");
        }

        /// Whether `p` saw the live city's cell `c` at its last look: on the map or past
        /// its edge.
        fn saw(p: &Player, c: Pos) -> bool {
            p.vis().get(c) == Some(&crate::render::Visibility::Visible)
                || p.beyond_sight().contains(&c)
        }

        #[test]
        fn the_night_sight_is_round() {
            // at 22:00 on the open ground, nothing in the way and nothing lit near: a
            // cell the dark radius straight off is seen, one a cell short of it both ways
            // (past the circle, inside the square round it) isn't
            use citygen::lighting::tuning::LIT_SIGHT;
            let mut st = home_run();
            let (e, d) = edge(&st);
            let (dx, dy) = d.delta();
            cover_round(&mut st, e.offset(40 * dx, 40 * dy), 64);
            let city = st.city.as_ref().unwrap();
            let b = Beyond::of(&st).unwrap();
            let map = MapRef::stratum_at(city, StratumKind::Street, NIGHT).beyond(&b);
            let mut p = Player::at(Place::Stratum(StratumKind::Street), e);
            let r = p.radius_at(map.daylight());
            assert_eq!(r, crate::walker::RADIUS_STREET, "the night's radius");
            let quiet = |c: Pos| {
                matches!(b.elsewhere(c), Some(Owner::Ground(..)))
                    && Rect::new(c.x - r - 1, c.y - r - 1, 2 * r + 3, 2 * r + 3)
                        .iter()
                        .all(|q| {
                            map.tile(q).is_some_and(|t| !t.blocks_sight())
                                && map.emitted(q) < LIT_SIGHT
                                && !map.emits(q)
                        })
            };
            let stand = (r + 2..72)
                .map(|k| e.offset(k * dx, k * dy))
                .find(|c| quiet(*c))
                .expect("open ground, dark and clear");
            p.pos = stand;
            p.update_fov(&map, 0);
            for (x, y) in [(r, 0), (-r, 0), (0, r), (0, -r)] {
                assert!(saw(&p, stand.offset(x, y)), "({x}, {y}) off is seen");
            }
            let c = r - 1;
            for (x, y) in [(c, c), (-c, c), (c, -c), (-c, -c)] {
                assert!(
                    !saw(&p, stand.offset(x, y)),
                    "({x}, {y}) off is past the circle"
                );
            }
        }

        #[test]
        fn a_city_glows_from_afar() {
            // at night on the open ground 60 to 90 cells from the next city, straight
            // back off its edge: its lit cells are in sight, and none of its dark ones
            use citygen::lighting::tuning::LIT_SIGHT;
            let mut st = home_run();
            let (s, x, d) = super::handover::beside_a_city(&mut st);
            let (dx, dy) = d.opposite().delta();
            let mut lit_seen = Vec::new();
            for k in [60, 70, 80, 90] {
                let stand = x.offset(k * dx, k * dy);
                if !matches!(
                    Beyond::of(&st).unwrap().elsewhere(stand),
                    Some(Owner::Ground(..))
                ) {
                    continue;
                }
                cover_round(&mut st, stand, 100);
                let city = st.city.as_ref().unwrap();
                let b = Beyond::of(&st).unwrap();
                let map = MapRef::stratum_at(city, StratumKind::Street, NIGHT).beyond(&b);
                let mut p = Player::at(Place::Stratum(StratumKind::Street), stand);
                p.update_fov(&map, 0);
                let theirs = |c: Pos| {
                    matches!(b.elsewhere(c), Some(Owner::City(..)))
                        && st
                            .outside
                            .as_ref()
                            .unwrap()
                            .owner(tile_of(world_cell(&st, c)))
                            == Some(s)
                };
                let seen: Vec<Pos> = p
                    .beyond_sight()
                    .iter()
                    .copied()
                    .chain(
                        p.vis()
                            .iter()
                            .filter(|(_, v)| **v == crate::render::Visibility::Visible)
                            .map(|(c, _)| c),
                    )
                    .filter(|c| theirs(*c))
                    .collect();
                let lit = |c: Pos| map.emitted(c) >= LIT_SIGHT || map.emits(c);
                let dark: Vec<&Pos> = seen.iter().filter(|c| !lit(**c)).collect();
                assert!(dark.is_empty(), "{k} out: their dark cells seen: {dark:?}");
                let far = seen.iter().filter(|c| c.dist2(stand) >= 60 * 60).count();
                lit_seen.push((k, far));
            }
            assert!(
                lit_seen.iter().any(|(_, n)| *n > 0),
                "their lit cells from 60 to 90 out: {lit_seen:?}"
            );
        }

        #[test]
        fn an_unbuilt_cell_cannot_be_walked() {
            let st = home_run();
            let (e, d) = edge(&st);
            let city = st.city.as_ref().unwrap();
            let b = Beyond::of(&st).unwrap();
            let off = e.step(d);
            for k in [StratumKind::Street, StratumKind::Net] {
                let map = MapRef::stratum_at(city, k, NIGHT).beyond(&b);
                assert!(map.tile(off).is_none(), "{k:?}");
                assert!(!map.walkable(off) && !map.reaches(off), "{k:?}");
            }
            let map = MapRef::stratum_at(city, StratumKind::Street, NIGHT).beyond(&b);
            let mut p = Player::at(Place::Stratum(StratumKind::Street), e);
            assert!(!p.try_move(&map, d));
            assert_eq!(p.pos, e);
        }

        #[test]
        fn another_citys_cells_answer_from_that_city() {
            let mut st = home_run();
            let s = st
                .outside
                .as_ref()
                .unwrap()
                .others()
                .into_iter()
                .map(|(s, _)| s)
                .find(|s| s.town.is_none())
                .unwrap();
            hold(&mut st, s);
            let city = st.city.as_ref().unwrap();
            let o = st.outside.as_ref().unwrap();
            let other = o.held_city(s).unwrap();
            let (live, theirs) = (o.origin(city), o.origin(other));
            // a building of theirs on a tile they own, whose ids read otherwise here
            let (q, t) = other
                .stratum(StratumKind::Street)
                .unwrap()
                .tiles
                .iter()
                .find(|(q, t)| {
                    let (Some(id), Some(d)) = (t.building, t.district) else {
                        return false;
                    };
                    let at = theirs.offset(q.x, q.y);
                    o.owner(tile_of(at)) == Some(s)
                        && city.building_opt(id).map(|b| b.kind) != Some(other.building(id).kind)
                        && city.district_opt(d).map(|d| d.theme) != Some(other.district(d).theme)
                })
                .expect("a building of the other city");
            let at = theirs.offset(q.x, q.y);
            let p = Pos::new(at.x - live.x, at.y - live.y);
            let b = Beyond::of(&st).unwrap();
            assert!(!b.live_owns(p));
            for minute in [NIGHT, NOON] {
                let map = MapRef::stratum_at(city, StratumKind::Street, minute).beyond(&b);
                assert!(std::ptr::eq(map.tile(p).unwrap(), t), "their tile");
                let id = t.building.unwrap();
                assert_eq!(map.kind(p), Some(other.building(id).kind));
                assert_eq!(
                    map.theme(p),
                    Some(other.district(t.district.unwrap()).theme)
                );
                let them = StratumMap {
                    city: other,
                    stratum: StratumKind::Street,
                    minute,
                    beyond: None,
                };
                assert_eq!(map.light(p), them.light(q));
                assert_eq!(map.emitted(p), them.emitted(q));
                assert!(map.reaches(p));
            }
            // and one of their connectors is theirs
            let c = other
                .connectors
                .iter()
                .find(|c| {
                    c.serves.contains(&StratumKind::Street)
                        && o.owner(tile_of(theirs.offset(c.pos.x, c.pos.y))) == Some(s)
                })
                .expect("a connector of theirs");
            let map = MapRef::stratum_at(city, StratumKind::Street, NIGHT).beyond(&b);
            let them = MapRef::stratum_at(other, StratumKind::Street, NIGHT);
            let at = theirs.offset(c.pos.x, c.pos.y);
            let p = Pos::new(at.x - live.x, at.y - live.y);
            assert_eq!(
                map.connector(p).map(|d| (d.kind, d.up, d.down)),
                them.connector(c.pos).map(|d| (d.kind, d.up, d.down))
            );
        }

        /// A city of the world whose block holds tiles one of its towns owns, and the
        /// town.
        pub(crate) fn city_with_a_town(st: &AppState) -> (Settlement, Settlement) {
            let o = st.outside.as_ref().unwrap();
            o.others()
                .into_iter()
                .filter(|(s, _)| s.town.is_none())
                .find_map(|(s, block)| {
                    block
                        .iter()
                        .find_map(|t| o.owner(t).filter(|w| w.town.is_some()).map(|w| (s, w)))
                })
                .expect("a town owning tiles of its city's map")
        }

        #[test]
        fn a_cell_of_the_live_map_another_settlement_owns_answers_from_it() {
            let mut st = home_run();
            // a city whose block holds tiles one of its towns owns
            let (c, town) = city_with_a_town(&st);
            enter(&mut st, c).unwrap();
            enter(&mut st, town).unwrap();
            enter(&mut st, c).unwrap();
            let p = {
                let city = st.city.as_ref().unwrap();
                let o = st.outside.as_ref().unwrap();
                assert_eq!(o.here(), c);
                let them = o.held_city(town).unwrap();
                let (live, theirs) = (o.origin(city), o.origin(them));
                // a cell of the live map the town owns, built otherwise in its map
                let owned: Vec<Pos> = Rect::new(0, 0, city.size.w as i32, city.size.h as i32)
                    .iter()
                    .filter(|p| o.owner(tile_of(live.offset(p.x, p.y))) == Some(town))
                    .collect();
                assert!(!owned.is_empty());
                let theirs_at = |p: Pos| {
                    let at = live.offset(p.x, p.y);
                    them.tile(
                        StratumKind::Street,
                        Pos::new(at.x - theirs.x, at.y - theirs.y),
                    )
                };
                owned
                    .iter()
                    .copied()
                    .find(|p| {
                        theirs_at(*p).map(|t| t.terrain)
                            != city.tile(StratumKind::Street, *p).map(|t| t.terrain)
                    })
                    .expect("a cell the town built otherwise than the live map has it")
            };
            let city = st.city.as_ref().unwrap();
            {
                let o = st.outside.as_ref().unwrap();
                let them = o.held_city(town).unwrap();
                let at = world_cell(&st, p);
                let q = {
                    let theirs = o.origin(them);
                    Pos::new(at.x - theirs.x, at.y - theirs.y)
                };
                let b = Beyond::of(&st).unwrap();
                assert!(!b.live_owns(p) && city.size.contains(p));
                let map = MapRef::stratum_at(city, StratumKind::Street, NIGHT).beyond(&b);
                let want = them.tile(StratumKind::Street, q).unwrap();
                assert!(std::ptr::eq(map.tile(p).unwrap(), want), "the town's tile");
                assert_ne!(
                    Some(want.terrain),
                    city.tile(StratumKind::Street, p).map(|t| t.terrain)
                );
                assert_eq!(
                    map.kind(p),
                    MapRef::stratum_of(them, StratumKind::Street).kind(q)
                );
            }
            // the town not held: the stream answers (nothing yet, then the ground)
            st.outside.as_mut().unwrap().let_go(town);
            {
                let b = Beyond::of(&st).unwrap();
                let map = MapRef::stratum_at(city, StratumKind::Street, NIGHT).beyond(&b);
                assert!(map.tile(p).is_none() && !map.reaches(p) && !map.walkable(p));
            }
            cover_round(&mut st, p, 8);
            let city = st.city.as_ref().unwrap();
            let b = Beyond::of(&st).unwrap();
            let map = MapRef::stratum_at(city, StratumKind::Street, NIGHT).beyond(&b);
            assert!(std::ptr::eq(
                map.tile(p).unwrap(),
                o_stream(&st).street.tile(world_cell(&st, p)).unwrap()
            ));
            assert_eq!(map.kind(p), None);
        }
    }

    /// Cities and towns built off the frame as the view nears them.
    mod builds {
        use super::*;
        use crate::app::AppState;
        use crate::outside::tests::home_run;
        use crate::outside::{RUN_ON, build_params, enter, seed_people};

        fn o(st: &AppState) -> &Outside {
            st.outside.as_ref().unwrap()
        }

        /// A block's cells: the map built on it, in world cells.
        fn cells(b: Rect) -> Rect {
            Rect::new(TILE * b.x, TILE * b.y, TILE * b.w, TILE * b.h)
        }

        /// Another city of the world (not a town) with room west of its map, and its map
        /// in world cells.
        fn a_city(st: &AppState) -> (Settlement, Rect) {
            o(st)
                .others()
                .into_iter()
                .find(|(s, b)| s.town.is_none() && b.x > 4)
                .map(|(s, b)| (s, cells(b)))
                .expect("a city with room west of it")
        }

        /// A view one cell across at world cell `c`, in the live city's frame.
        fn view_at(st: &AppState, c: Pos) -> Rect {
            let at = o(st).origin(st.city.as_ref().unwrap());
            Rect::new(c.x - at.x, c.y - at.y, 1, 1)
        }

        /// The view `REACH` cells west of map `map`, level with its middle.
        fn reach_of(st: &AppState, map: Rect) -> Rect {
            view_at(st, Pos::new(map.x - REACH, map.y + map.h / 2))
        }

        /// The stream tended for a view one cell across at world cell `c`.
        fn tend_at(st: &mut AppState, c: Pos) {
            let view = view_at(st, c);
            tend(st, view);
        }

        /// The stream tended for the view `REACH` cells west of map `map`.
        fn tend_near(st: &mut AppState, map: Rect) {
            let view = reach_of(st, map);
            tend(st, view);
        }

        /// The cells between `c` and rectangle `r`, the longer way.
        fn gap(c: Pos, r: Rect) -> i32 {
            let dx = (r.x - c.x).max(c.x - (r.right() - 1)).max(0);
            let dy = (r.y - c.y).max(c.y - (r.bottom() - 1)).max(0);
            dx.max(dy)
        }

        #[test]
        fn a_city_near_the_view_builds_in_the_background() {
            let mut st = home_run();
            let (s, map) = a_city(&st);
            tend_near(&mut st, map);
            // started, and the frame didn't wait for it
            assert!(o(&st).stream.building(s), "building");
            assert!(!o(&st).built.contains_key(&s), "not built on the frame");
            finish(&mut st);
            assert!(!o(&st).stream.building(s));
            assert!(o(&st).built.contains_key(&s), "built");
            assert!(o(&st).stream.people(s).is_some(), "its people seeded");
            assert_eq!(o(&st).here(), o(&st).home(), "you're where you were");
        }

        #[test]
        fn far_cities_are_not_built() {
            // one cell past `REACH` of a city's map: not that city
            let mut st = home_run();
            let (s, map) = a_city(&st);
            let past = Pos::new(map.x - REACH - 1, map.y + map.h / 2);
            tend_at(&mut st, past);
            finish(&mut st);
            assert!(!o(&st).built.contains_key(&s));
            // and more than `REACH` from every other map: nothing at all
            let mut st = home_run();
            let maps: Vec<Rect> = o(&st).others().iter().map(|(_, b)| cells(*b)).collect();
            let w = o(&st).world().size;
            let far = Rect::new(0, 0, w.w as i32, w.h as i32)
                .iter()
                .map(|t| Pos::new(TILE * t.x + TILE / 2, TILE * t.y + TILE / 2))
                .find(|c| maps.iter().all(|m| gap(*c, *m) > REACH))
                .expect("a cell far from every other map");
            tend_at(&mut st, far);
            finish(&mut st);
            assert_eq!(o(&st).stream.started(), 0);
            assert!(o(&st).built.is_empty());
        }

        #[test]
        fn a_streamed_city_is_the_city_built_on_the_spot() {
            let mut st = home_run();
            let (city, map) = a_city(&st);
            // and a town of the world
            let (town, by) = o(&st)
                .others()
                .into_iter()
                .find(|(s, b)| s.town.is_some() && b.x > 4)
                .map(|(s, b)| (s, cells(b)))
                .expect("a town with room west of it");
            tend_near(&mut st, map);
            tend_near(&mut st, by);
            finish(&mut st);
            for s in [city, town] {
                let streamed = &o(&st).built[&s];
                let params = build_params(o(&st), s, &st.params).unwrap();
                let spot = citygen::Generator::run(params).unwrap();
                assert_eq!((streamed.seed, streamed.size), (spot.seed, spot.size));
                for k in [StratumKind::Street, StratumKind::Net] {
                    assert!(
                        streamed.stratum(k).unwrap().tiles == spot.stratum(k).unwrap().tiles,
                        "{s:?} {k:?}"
                    );
                }
            }
        }

        #[test]
        fn a_streamed_citys_people_are_ready_when_you_walk_in() {
            let mut st = home_run();
            let (s, map) = a_city(&st);
            let (asked, minute) = (st.clock_tick(), st.minute_of_day());
            tend_near(&mut st, map);
            finish(&mut st);
            // the hour runs on before you walk in
            crate::clock::run(
                &mut st,
                Some(40 * sim::TICKS_PER_MINUTE),
                crate::clock::PATHS_PER_FRAME,
            );
            let now = st.clock_tick();
            assert!(now > asked);
            let city = o(&st).built[&s].clone();
            enter(&mut st, s).unwrap();
            assert!(o(&st).stream.people(s).is_none(), "taken in with you");
            let w = st.world.as_ref().unwrap();
            assert_eq!(w.tick(), now);
            // the people seeded for the hour the build was asked for, run on to now
            let mut want = seed_people(&city, &st.people, asked, minute);
            want.rebuild(&city, &st.people);
            want.advance(&city, now);
            assert!(w.people() == want.people(), "run on from when it was asked");
            // not seeded again
            let again = seed_people(&city, &st.people, now, st.minute_of_day());
            assert!(w.people() != again.people(), "seeded again on the frame");
        }

        #[test]
        fn the_same_city_is_never_built_twice() {
            let mut st = home_run();
            let (s, map) = a_city(&st);
            let view = reach_of(&st, map);
            tend(&mut st, view);
            let started = o(&st).stream.started();
            assert!(started >= 1 && o(&st).stream.building(s));
            for _ in 0..20 {
                tend(&mut st, view);
            }
            assert_eq!(o(&st).stream.started(), started, "while it builds");
            finish(&mut st);
            for _ in 0..3 {
                tend(&mut st, view);
            }
            assert_eq!(o(&st).stream.started(), started, "once it's built");
            // walked into and out of: kept, not built again
            enter(&mut st, s).unwrap();
            let home = o(&st).home();
            enter(&mut st, home).unwrap();
            tend(&mut st, view);
            finish(&mut st);
            assert_eq!(o(&st).stream.started(), started, "once walked into");
        }

        #[test]
        fn a_city_that_wont_build_is_not_tried_again() {
            let st = home_run();
            let (s, _) = a_city(&st);
            let params = build_params(o(&st), s, &st.params);
            let mut stream = Stream::default();
            let mut built = BTreeMap::new();
            stream.tend(&mut built, vec![(s, None)], &st.people, 0, 0);
            stream.finish(&mut built);
            assert!(built.is_empty() && !stream.building(s));
            stream.tend(&mut built, vec![(s, params)], &st.people, 0, 0);
            stream.finish(&mut built);
            assert!(built.is_empty(), "not tried again");
            assert_eq!(stream.started(), 0);
        }

        #[test]
        fn a_streamed_city_entered_a_day_on_is_seeded_afresh() {
            // people frozen at the ask for more than a day aren't run on across it on
            // the frame: they're seeded for now, as on the spot
            let mut st = home_run();
            let (s, map) = a_city(&st);
            let (asked, then) = (st.clock_tick(), st.minute_of_day());
            tend_near(&mut st, map);
            finish(&mut st);
            crate::clock::run(
                &mut st,
                Some(RUN_ON + 60 * sim::TICKS_PER_MINUTE),
                crate::clock::PATHS_PER_FRAME,
            );
            let (now, minute) = (st.clock_tick(), st.minute_of_day());
            assert!(now - asked > RUN_ON);
            let city = o(&st).built[&s].clone();
            enter(&mut st, s).unwrap();
            assert!(o(&st).stream.people(s).is_none(), "dropped");
            let w = st.world.as_ref().unwrap();
            assert_eq!(w.tick(), now);
            let fresh = seed_people(&city, &st.people, now, minute);
            assert!(w.people() == fresh.people(), "seeded for now");
            let mut old = seed_people(&city, &st.people, asked, then);
            old.rebuild(&city, &st.people);
            old.advance(&city, now);
            assert!(w.people() != old.people(), "not run on across the day");
        }

        #[test]
        fn a_build_that_panics_is_said_on_the_main_thread() {
            let mut stream = Stream::default();
            let mut built = BTreeMap::new();
            let s = Settlement {
                city: 9,
                town: Some(1),
            };
            assert!(!on_build_thread());
            stream.start_with(s, || {
                assert!(on_build_thread(), "a build's own thread");
                assert_eq!(std::thread::current().name(), Some("build 9.1"));
                panic!("the generator broke");
            });
            // the frame goes on: nothing is said until it's collected
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                stream.finish(&mut built)
            }));
            let said = said(&*caught.expect_err("the main thread panics on collecting it"));
            assert!(said.contains("the generator broke"), "{said}");
            assert!(built.is_empty() && !stream.building(s));
        }

        #[test]
        fn a_front_ends_hook_leaves_a_builds_panic_to_the_main_thread() {
            // a hook as the front ends' (term.rs, window/fatal.rs) start: what a build's
            // panic said kept for the main thread, nothing done on the build's. The hook
            // is the process's, so only this test's build's thread is counted.
            let held = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let previous = std::sync::Arc::new(std::panic::take_hook());
            std::panic::set_hook(Box::new({
                let (previous, held) = (previous.clone(), held.clone());
                move |info| {
                    if held_by_build(info) {
                        if std::thread::current().name() == Some("build 7") {
                            held.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        return;
                    }
                    previous(info);
                }
            }));
            let mut stream = Stream::default();
            let mut built = BTreeMap::new();
            let s = Settlement {
                city: 7,
                town: None,
            };
            stream.start_with(s, || panic!("the generator broke"));
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                stream.finish(&mut built)
            }));
            let _ = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| previous(info)));
            let said = said(&*caught.expect_err("the main thread panics on collecting it"));
            assert_eq!(held.load(std::sync::atomic::Ordering::SeqCst), 1);
            // the hook's own words, where it broke among them
            assert!(
                said.contains("the generator broke") && said.contains("stream.rs"),
                "{said}"
            );
        }

        #[test]
        fn a_copy_starts_with_nothing_in_flight() {
            let mut st = home_run();
            let (s, map) = a_city(&st);
            tend_near(&mut st, map);
            let copy = o(&st).stream.clone();
            assert!(!copy.building(s) && copy.started() == 0);
            finish(&mut st);
            assert!(o(&st).built.contains_key(&s));
        }
    }

    /// Walking onto a cell another city or town owns: the run handed over to it in
    /// place.
    pub(crate) mod handover {
        use super::*;
        use crate::app::AppState;
        use crate::outside::tests::home_run;
        use crate::render::Visibility;
        use citygen::overworld::detail;
        use citygen::{Dir, Place};

        fn o(st: &AppState) -> &Outside {
            st.outside.as_ref().unwrap()
        }

        fn tile_of(c: Pos) -> Pos {
            Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE))
        }

        /// The world cell of the live city's cell `p`.
        pub(crate) fn world_of(st: &AppState, p: Pos) -> Pos {
            o(st).origin(st.city.as_ref().unwrap()).offset(p.x, p.y)
        }

        /// The live city's cell at world cell `c`.
        pub(crate) fn local(st: &AppState, c: Pos) -> Pos {
            let at = o(st).origin(st.city.as_ref().unwrap());
            Pos::new(c.x - at.x, c.y - at.y)
        }

        /// A city of the world held beside the live one (built as the game builds it),
        /// a cell of the open ground beside its map, and the step from there onto a
        /// cell it owns that its street lets you onto: the city, the ground cell (in the
        /// live city's frame) and the step.
        pub(crate) fn beside_a_city(st: &mut AppState) -> (Settlement, Pos, Dir) {
            let (s, city, x, d) = {
                let o = o(st);
                let w = o.world();
                let here = o.here();
                settlements()
                    .iter()
                    .filter(|(s, _)| s.town.is_none() && *s != here && *s != o.home())
                    .find_map(|(s, city)| {
                        let origin = o.origin(city);
                        let (cw, ch) = (city.size.w as i32, city.size.h as i32);
                        // the edge cells of its map, each with the step onto it from
                        // outside
                        (0..ch)
                            .flat_map(|y| [(Pos::new(0, y), Dir::E), (Pos::new(cw - 1, y), Dir::W)])
                            .chain((0..cw).flat_map(|x| {
                                [(Pos::new(x, 0), Dir::S), (Pos::new(x, ch - 1), Dir::N)]
                            }))
                            .find_map(|(q, d)| {
                                let y = origin.offset(q.x, q.y);
                                let x = y.step(d.opposite());
                                let open = w.tile(tile_of(x)).is_some()
                                    && detail::region(w, x.x, x.y, 1, 1)[Pos::new(0, 0)]
                                        .is_walkable();
                                (o.owner(tile_of(y)) == Some(*s)
                                    && o.owner(tile_of(x)).is_none()
                                    && city.walkable(StratumKind::Street, q)
                                    && open)
                                    .then_some((*s, city.clone(), x, d))
                            })
                    })
                    .expect("a city with open ground beside its map")
            };
            st.outside.as_mut().unwrap().built.insert(s, city);
            (s, local(st, x), d)
        }

        /// The people of settlement `s` (held, built and not walked into) seeded for the
        /// hour, as its build in the background seeds them.
        pub(crate) fn seed_its_people(st: &mut AppState, s: Settlement) {
            let (now, minute) = (st.clock_tick(), st.minute_of_day());
            let o = st.outside.as_mut().unwrap();
            let world = crate::outside::seed_people(&o.built[&s], &st.people, now, minute);
            o.stream.builds.people.insert(s, world);
        }

        /// Settlement `s` held, built as the game builds it.
        pub(crate) fn hold(st: &mut AppState, s: Settlement) {
            let city = settlements()
                .iter()
                .find(|(t, _)| *t == s)
                .unwrap()
                .1
                .clone();
            st.outside.as_mut().unwrap().built.insert(s, city);
        }

        /// A city with a town owning cells of its map, the city live and the town held,
        /// and a street cell of the city's own beside one of the town's: the city, the
        /// town, the cell (in the city's frame) and the step onto the town's.
        pub(crate) fn beside_a_town(st: &mut AppState) -> (Settlement, Settlement, Pos, Dir) {
            let (c, town) = super::beyond::city_with_a_town(st);
            crate::outside::enter(st, c).unwrap();
            hold(st, town);
            let city = st.city.as_ref().unwrap();
            let bey = Beyond::of(st).unwrap();
            let map =
                crate::render::map::MapRef::stratum_of(city, StratumKind::Street).beyond(&bey);
            let (a, d) = Rect::new(0, 0, city.size.w as i32, city.size.h as i32)
                .iter()
                .filter(|p| bey.live_owns(*p) && city.walkable(StratumKind::Street, *p))
                .find_map(|p| {
                    Dir::CARDINAL
                        .into_iter()
                        .find(|d| {
                            let q = p.step(*d);
                            city.size.contains(q)
                                && o(st).owner(tile_of(world_of(st, q))) == Some(town)
                                && map.walkable(q)
                        })
                        .map(|d| (p, d))
                })
                .expect("the city's street beside the town's");
            (c, town, a, d)
        }

        /// You on foot on the live city's street at `p`.
        pub(crate) fn on_foot(st: &mut AppState, p: Pos) {
            let pl = st.player.as_mut().unwrap();
            pl.place = Place::Stratum(StratumKind::Street);
            pl.pos = p;
            pl.body = None;
        }

        /// The live city's street cells you remember (in sight or not), as world cells.
        fn remembered_street(st: &AppState) -> Vec<Pos> {
            let p = st.player.as_ref().unwrap();
            p.vis_at(Place::Stratum(StratumKind::Street))
                .map(|g| {
                    g.iter()
                        .filter(|(_, v)| **v != Visibility::Unseen)
                        .map(|(q, _)| world_of(st, q))
                        .collect()
                })
                .unwrap_or_default()
        }

        #[test]
        fn a_cell_another_city_owns_hands_over_on_the_same_cell() {
            let mut st = home_run();
            let (s, x, d) = beside_a_city(&mut st);
            on_foot(&mut st, x);
            assert_eq!(hand_over(&mut st), None, "the open ground is nobody's");
            let y = x.step(d);
            let at = world_of(&st, y);
            let before = o(&st).origin(st.city.as_ref().unwrap());
            on_foot(&mut st, y);
            let shift = hand_over(&mut st).expect("handed over");
            assert_eq!(o(&st).here(), s);
            let after = o(&st).origin(st.city.as_ref().unwrap());
            assert_eq!(shift, Pos::new(before.x - after.x, before.y - after.y));
            let p = st.player.as_ref().unwrap();
            assert_eq!(world_of(&st, p.pos), at, "the same world cell");
            assert_eq!(p.pos, y.offset(shift.x, shift.y));
            assert!(live_owns(&st, p.pos), "on its own cells");
            assert!(
                st.log
                    .iter()
                    .any(|e| e.text == format!("into {}", o(&st).name(s)))
            );
            assert_eq!(hand_over(&mut st), None, "once is enough");
        }

        /// Jacked in with the body at `body` (the live city's frame), your Net self on
        /// its Net at `at`.
        pub(crate) fn jacked(st: &mut AppState, body: (Place, Pos), at: Pos) {
            let p = st.player.as_mut().unwrap();
            p.body = Some(body);
            p.place = Place::Stratum(StratumKind::Net);
            p.pos = at;
        }

        /// Seed 3's home run jacked in from its street where you stood, your Net self on
        /// the open Net beside the next city (`beside_a_city`): the city, the cell (in
        /// home's frame), the step onto its cells, and the body's cell.
        fn jacked_beside_a_city(st: &mut AppState) -> (Settlement, Pos, Dir, Pos) {
            let body = st.player.as_ref().unwrap().pos;
            let (s, x, d) = beside_a_city(st);
            jacked(st, (Place::Stratum(StratumKind::Street), body), x);
            (s, x, d, body)
        }

        #[test]
        fn walking_the_net_into_another_city_hands_it_over() {
            let mut st = home_run();
            let (s, x, d, _) = jacked_beside_a_city(&mut st);
            assert_eq!(hand_over(&mut st), None, "the open Net is nobody's");
            let y = x.step(d);
            let at = world_of(&st, y);
            let before = o(&st).origin(st.city.as_ref().unwrap());
            st.player.as_mut().unwrap().pos = y;
            let shift = hand_over(&mut st).expect("handed over");
            assert_eq!(o(&st).here(), s);
            let after = o(&st).origin(st.city.as_ref().unwrap());
            assert_eq!(shift, Pos::new(before.x - after.x, before.y - after.y));
            let p = st.player.as_ref().unwrap();
            assert_eq!(
                p.place,
                Place::Stratum(StratumKind::Net),
                "still on the Net"
            );
            assert!(p.jacked_in(), "still jacked in");
            assert_eq!(world_of(&st, p.pos), at, "the same world cell");
            assert_eq!(p.pos, y.offset(shift.x, shift.y));
            assert!(live_owns(&st, p.pos), "on its own cells");
            assert!(
                st.log
                    .iter()
                    .any(|e| e.text == format!("into {}", o(&st).name(s)))
            );
            assert_eq!(hand_over(&mut st), None, "once is enough");
        }

        #[test]
        fn the_body_stays_in_the_city_you_left() {
            let mut st = home_run();
            let home = o(&st).home();
            let (s, x, d, body) = jacked_beside_a_city(&mut st);
            let was = world_of(&st, body);
            st.player.as_mut().unwrap().pos = x.step(d);
            hand_over(&mut st).expect("handed over");
            assert_eq!(o(&st).here(), s);
            let (place, b) = st.player.as_ref().unwrap().body.expect("jacked in");
            assert_eq!(place, Place::Stratum(StratumKind::Street));
            assert_eq!(world_of(&st, b), was, "on its own world cell");
            assert!(!live_owns(&st, b), "off the live city's cells");
            assert_eq!(owner_of(&st, b), Some(home), "in the city you left");
            assert!(st.away.is_some(), "home set aside");
            assert!(crate::outside::away(&st), "not on the live city's street");
            // the city you left holds still while you're away from it
            let held = |st: &AppState| st.away.as_ref().unwrap().world.as_ref().unwrap().tick();
            let then = held(&st);
            crate::clock::run(
                &mut st,
                Some(5 * sim::TICKS_PER_MINUTE),
                crate::clock::PATHS_PER_FRAME,
            );
            assert_eq!(held(&st), then, "its people hold still");
            // the Net walked back into it: the body is in the live city again, as it was
            st.player.as_mut().unwrap().pos = local(&st, was);
            hand_over(&mut st).expect("home on the Net");
            assert_eq!(o(&st).here(), home);
            let p = st.player.as_ref().unwrap();
            assert_eq!(p.body, Some((Place::Stratum(StratumKind::Street), body)));
            assert!(!crate::outside::away(&st), "the body is home");
        }

        #[test]
        fn jacking_out_hands_the_bodys_city_back() {
            // jacked in from the street: back in the body there, its city live again
            let mut st = home_run();
            let home = o(&st).home();
            let (s, x, d, body) = jacked_beside_a_city(&mut st);
            st.player.as_mut().unwrap().pos = x.step(d);
            hand_over(&mut st).expect("handed over");
            assert_eq!(o(&st).here(), s);
            crate::net::jack_out(&mut st).unwrap();
            hand_over(&mut st).expect("the body's city back");
            assert_eq!(o(&st).here(), home);
            let p = st.player.as_ref().unwrap();
            assert_eq!(
                (p.place, p.pos, p.body),
                (Place::Stratum(StratumKind::Street), body, None)
            );
            // jacked in from the flat's rig: the cord ends at the rig's landing, and
            // jacking out wakes you in the chair
            let mut st = home_run();
            let rig = st.home.expect("the flat's rig");
            let landing = world_of(&st, rig.landing);
            let (s, x, d) = beside_a_city(&mut st);
            jacked(&mut st, (rig.place(), rig.home.rig), x.step(d));
            hand_over(&mut st).expect("handed over from the rig");
            assert_eq!(o(&st).here(), s);
            assert!(crate::outside::away(&st), "the body isn't here");
            assert_eq!(
                crate::net::port_of(&st).map(|q| world_of(&st, q)),
                Some(landing),
                "the cord's end"
            );
            crate::net::jack_out(&mut st).unwrap();
            hand_over(&mut st).expect("home again");
            assert_eq!(o(&st).here(), home);
            let p = st.player.as_ref().unwrap();
            assert_eq!(
                (p.place, p.pos, p.body),
                (rig.place(), rig.home.rig, None),
                "at the rig"
            );
        }

        #[test]
        fn a_dump_on_a_rig_run_in_another_city_starts_the_trace_home() {
            // landed at a relay of home's, then walked the Net into the next city
            let mut st = home_run();
            let rig = st.home.expect("the flat's rig");
            let (relay, gate) = {
                let city = st.city.as_ref().unwrap();
                city.buildings
                    .iter()
                    .find_map(|b| Some((b.id, crate::constructs::city_gate(city, b.id)?)))
                    .expect("a construct of home's")
            };
            let gate = world_of(&st, gate);
            let (s, x, d) = beside_a_city(&mut st);
            jacked(&mut st, (rig.place(), rig.home.rig), x.step(d));
            st.backdoors.start_route(Some(relay));
            hand_over(&mut st).expect("handed over from the rig");
            assert_eq!(o(&st).here(), s);
            assert!(st.backdoors.route.is_none(), "the rig's route is home's");
            // the cord runs back through home's relay, where it stands
            let v = crate::net::cord_view(&st).expect("the cord");
            assert_eq!(world_of(&st, v.entry), gate, "from the relay's gate");
            assert_eq!(
                v.relays
                    .iter()
                    .map(|(g, _)| world_of(&st, *g))
                    .collect::<Vec<_>>(),
                vec![gate]
            );
            crate::net::dump(&mut st, "test");
            assert!(
                st.away.as_ref().unwrap().backdoors.trace.is_some(),
                "the trace starts at home"
            );
            assert!(st.log.iter().any(|e| e.text.starts_with("trace running")));
            hand_over(&mut st).expect("home again");
            assert_eq!(o(&st).here(), o(&st).home());
            assert!(st.backdoors.trace.is_some(), "and walks there");
            assert!(st.backdoors.route.is_none(), "the run's route over");
            let p = st.player.as_ref().unwrap();
            assert_eq!((p.place, p.body), (rig.place(), None), "in the chair");
        }

        #[test]
        fn a_run_in_flight_holds_your_net_self_out_of_a_handover() {
            let mut st = home_run();
            let (_, x, d, _) = jacked_beside_a_city(&mut st);
            st.net.run = Some(game::Run {
                program: game::ItemId::new("crack"),
                target: game::HackTarget::Deck,
                done: 0,
                work: 3,
            });
            st.player.as_mut().unwrap().pos = x.step(d);
            let here = o(&st).here();
            assert_eq!(hand_over(&mut st), None);
            assert_eq!(o(&st).here(), here, "a run put there waits");
            assert_eq!(crate::outside::can_cross(&st), Err("finish the run first"));
            st.net.run = None;
            assert!(hand_over(&mut st).is_some(), "the run done, in");
        }

        #[test]
        fn walking_home_hands_back() {
            let mut st = home_run();
            crate::missions::begin(&mut st);
            let story = format!("{:?}", st.missions);
            let spawn = st.player.as_ref().unwrap().pos;
            crate::screens::explore::look_round(&mut st);
            let known = remembered_street(&st);
            assert!(!known.is_empty());
            let home_cell = world_of(&st, spawn);
            let (s, x, d) = beside_a_city(&mut st);
            on_foot(&mut st, x.step(d));
            hand_over(&mut st).expect("into the next city");
            assert_eq!(o(&st).here(), s);
            assert!(st.missions.runs.is_empty(), "the story waits at home");
            // back onto a cell of home's own, from wherever
            let back = local(&st, home_cell);
            on_foot(&mut st, back);
            hand_over(&mut st).expect("home again");
            assert_eq!(o(&st).here(), o(&st).home());
            assert!(st.away.is_none() && st.home.is_some());
            assert_eq!(format!("{:?}", st.missions), story);
            let p = st.player.as_ref().unwrap();
            assert_eq!(world_of(&st, p.pos), home_cell);
            let now = remembered_street(&st);
            let lost: Vec<_> = known.iter().filter(|c| !now.contains(c)).collect();
            assert!(lost.is_empty(), "home's streets forgotten: {lost:?}");
        }

        /// Seed 3's home run, the next city held with its people seeded now, the clock
        /// then run `minutes` on with it out of view: the city, and a 200x60 view (the
        /// live city's frame) beside it.
        fn back_beside_it(minutes: u64) -> (AppState, Settlement, Rect) {
            let mut st = home_run();
            let (s, x, _) = beside_a_city(&mut st);
            seed_its_people(&mut st, s);
            crate::clock::run(
                &mut st,
                Some(minutes * sim::TICKS_PER_MINUTE),
                crate::clock::PATHS_PER_FRAME,
            );
            (st, s, Rect::new(x.x - 100, x.y - 30, 200, 60))
        }

        #[test]
        fn a_neighbour_back_in_view_catches_up_a_while_a_frame() {
            let (mut st, s, view) = back_beside_it(150);
            let t0 = crate::outside::people_of(&st, s).unwrap().tick();
            let now = st.clock_tick();
            assert_eq!(now - t0, 150 * sim::TICKS_PER_MINUTE);
            let ticks = |st: &AppState| crate::outside::people_of(st, s).unwrap().tick();
            assert_eq!(run_neighbours(&mut st, view, 4, 1), 1);
            assert_eq!(ticks(&st), t0 + CATCH_UP * sim::TICKS_PER_MINUTE);
            run_neighbours(&mut st, view, 4, 1);
            assert_eq!(ticks(&st), t0 + 2 * CATCH_UP * sim::TICKS_PER_MINUTE);
            run_neighbours(&mut st, view, 4, 1);
            assert_eq!(ticks(&st), now, "caught up");
            // out of view it holds still
            let far = Rect::new(view.x - 2000, view.y, 200, 60);
            crate::clock::run(&mut st, Some(90), crate::clock::PATHS_PER_FRAME);
            assert_eq!(run_neighbours(&mut st, far, 4, 90), 0);
            assert_eq!(ticks(&st), now);
        }

        #[test]
        fn a_neighbours_people_a_day_behind_are_seeded_afresh() {
            let (mut st, s, view) = back_beside_it(25 * 60);
            let count = crate::outside::people_of(&st, s).unwrap().people().len();
            run_neighbours(&mut st, view, 4, 1);
            let w = crate::outside::people_of(&st, s).unwrap();
            assert_eq!(
                w.tick(),
                st.clock_tick(),
                "seeded for now, as walking in would"
            );
            assert_eq!(w.minute_of_day(), st.minute_of_day());
            assert_eq!(w.people().len(), count);
        }

        #[test]
        fn neighbours_behind_catch_up_one_a_frame() {
            // two held cities in view, both behind: a frame pays for one catch-up, the one
            // furthest behind, and the other waits its turn
            let mut st = home_run();
            let (a, _, _) = beside_a_city(&mut st);
            let home = o(&st).home();
            let b = settlements()
                .iter()
                .map(|(s, _)| *s)
                .find(|s| s.town.is_none() && *s != a && *s != home)
                .unwrap();
            hold(&mut st, b);
            let run_on = |st: &mut AppState, minutes: u64| {
                crate::clock::run(
                    st,
                    Some(minutes * sim::TICKS_PER_MINUTE),
                    crate::clock::PATHS_PER_FRAME,
                )
            };
            seed_its_people(&mut st, a);
            run_on(&mut st, 50);
            seed_its_people(&mut st, b);
            run_on(&mut st, 100);
            let now = st.clock_tick();
            let behind = |st: &AppState, s: Settlement| {
                let tick = crate::outside::people_of(st, s).unwrap().tick();
                (now - tick) / sim::TICKS_PER_MINUTE
            };
            assert_eq!((behind(&st, a), behind(&st, b)), (150, 100));
            // the whole world in view
            let all = Rect::new(-100_000, -100_000, 200_000, 200_000);
            let frames: Vec<(u64, u64)> = (0..5)
                .map(|_| {
                    run_neighbours(&mut st, all, 4, 1);
                    (behind(&st, a), behind(&st, b))
                })
                .collect();
            assert_eq!(frames, [(90, 100), (90, 40), (30, 40), (30, 0), (0, 0)]);
            // caught up, both run every frame
            crate::clock::run(&mut st, Some(1), crate::clock::PATHS_PER_FRAME);
            assert_eq!(run_neighbours(&mut st, all, 4, 1), 2);
            for s in [a, b] {
                let w = crate::outside::people_of(&st, s).unwrap();
                assert_eq!(w.tick(), st.clock_tick());
            }
        }

        #[test]
        fn a_long_frame_runs_every_neighbour_that_kept_time() {
            // a frame whose clock steps three game minutes: two neighbours that kept time
            // both step with it, neither waits its turn
            let mut st = home_run();
            let (a, _, _) = beside_a_city(&mut st);
            let home = o(&st).home();
            let b = settlements()
                .iter()
                .map(|(s, _)| *s)
                .find(|s| s.town.is_none() && *s != a && *s != home)
                .unwrap();
            hold(&mut st, b);
            seed_its_people(&mut st, a);
            seed_its_people(&mut st, b);
            let all = Rect::new(-100_000, -100_000, 200_000, 200_000);
            crate::clock::run(&mut st, Some(1), crate::clock::PATHS_PER_FRAME);
            assert_eq!(run_neighbours(&mut st, all, 4, 1), 2, "both keep time");
            let step = 3 * sim::TICKS_PER_MINUTE;
            crate::clock::run(&mut st, Some(step), crate::clock::PATHS_PER_FRAME);
            assert_eq!(run_neighbours(&mut st, all, 4, step), 2);
            for s in [a, b] {
                let w = crate::outside::people_of(&st, s).unwrap();
                assert_eq!(w.tick(), st.clock_tick(), "{s:?} stepped with the frame");
            }
        }

        #[test]
        fn your_bots_come_into_the_next_city() {
            let mut st = home_run();
            st.character = game::Character::debug_runner(&st.content);
            let (s, x, d) = beside_a_city(&mut st);
            on_foot(&mut st, x);
            crate::screens::explore::look_round(&mut st);
            crate::bots::sync(&mut st);
            let out = |st: &AppState| {
                st.bots
                    .iter()
                    .map(|b| (b.dock, b.item.clone(), b.pos.map(|p| world_of(st, p))))
                    .collect::<Vec<_>>()
            };
            let before = out(&st);
            assert!(
                !before.is_empty() && before.iter().all(|b| b.2.is_some()),
                "a bot out beside you"
            );
            on_foot(&mut st, x.step(d));
            hand_over(&mut st).expect("handed over");
            assert_eq!(o(&st).here(), s);
            assert_eq!(out(&st), before, "where they were, in the new city's frame");
            assert!(
                st.away.as_ref().unwrap().bots.is_empty(),
                "none left behind at home"
            );
        }

        #[test]
        fn what_a_city_saw_of_its_town_is_remembered_in_the_town() {
            // a town's cells inside its city's map, seen while in the city, are kept in
            // the city's memory: walking into the town, it remembers them
            let mut st = home_run();
            let (_, town, a, d) = beside_a_town(&mut st);
            on_foot(&mut st, a);
            crate::screens::explore::look_round(&mut st);
            let theirs: Vec<Pos> = remembered_street(&st)
                .into_iter()
                .filter(|c| o(&st).owner(tile_of(*c)) == Some(town))
                .collect();
            assert!(!theirs.is_empty(), "the town's cells seen from the city");
            on_foot(&mut st, a.step(d));
            hand_over(&mut st).expect("into the town");
            assert_eq!(o(&st).here(), town);
            let now = remembered_street(&st);
            let lost: Vec<_> = theirs.iter().filter(|c| !now.contains(c)).collect();
            assert!(lost.is_empty(), "forgotten in the town: {lost:?}");
        }

        #[test]
        fn what_you_drop_in_a_town_is_there_when_you_come_back() {
            // over the line into the town, an item down, back over into the city and
            // into the town again: the town kept it
            let mut st = home_run();
            let (c, town, a, d) = beside_a_town(&mut st);
            on_foot(&mut st, a.step(d));
            hand_over(&mut st).expect("into the town");
            let stim = game::ItemId::new("stim_patch");
            st.character.give_item(&st.content, stim.clone(), 2);
            let drop = game::Action::Drop {
                item: stim.clone(),
                qty: 1,
            };
            assert!(crate::actions::perform(&mut st, drop).is_ok());
            let street = Place::Stratum(StratumKind::Street);
            let lies = |st: &AppState| {
                let p = st.player.as_ref().unwrap().pos;
                st.ground.at(street, p).iter().any(|s| s.item == stim)
            };
            assert!(lies(&st));
            let back = st.player.as_ref().unwrap().pos.step(d.opposite());
            on_foot(&mut st, back);
            hand_over(&mut st).expect("back into the city");
            assert_eq!(o(&st).here(), c);
            let again = st.player.as_ref().unwrap().pos.step(d);
            on_foot(&mut st, again);
            hand_over(&mut st).expect("into the town again");
            assert_eq!(o(&st).here(), town);
            assert!(lies(&st), "the town kept what you left in it");
        }

        #[test]
        fn what_you_leave_in_other_cities_is_there_after_a_reopen() {
            // a job taken from a city's fixer and a card to one of its doors in the
            // pack, an item left in its town; home, saved there and reopened: walked
            // back, the town keeps the item, the job is handed in, the cards come back
            let mut st = home_run();
            let home = o(&st).home();
            let (c, town, a, d) = beside_a_town(&mut st);
            // a delivery from a fixer of the city
            let fixers: Vec<sim::PersonId> = st
                .world
                .as_ref()
                .unwrap()
                .people()
                .iter()
                .filter(|p| p.profession == "fixer")
                .map(|p| p.id)
                .collect();
            let job = fixers
                .iter()
                .find_map(|f| {
                    crate::jobs::generate::generate(
                        &mut st,
                        *f,
                        crate::jobs::flavor::KindTag::Deliver,
                        0,
                    )
                })
                .expect("a delivery from a fixer of the city");
            let id = st.jobs.push(job);
            crate::jobs::take_job(&mut st, id).unwrap();
            // a card to a door of the city, in the pack
            let flats: Vec<citygen::BuildingId> = st
                .city
                .as_ref()
                .unwrap()
                .buildings
                .iter()
                .map(|b| b.id)
                .collect();
            let card = flats
                .into_iter()
                .find_map(|b| {
                    crate::credentials::prepare(&mut st, b);
                    crate::credentials::run_items(&st.content)
                        .first()
                        .map(|i| i.id.clone())
                })
                .expect("a building of the city with flats");
            st.character.give(card.clone(), 1);
            let cards = |st: &AppState| {
                (
                    crate::credentials::run_items(&st.content).len(),
                    st.character.count(&card),
                )
            };
            let held = cards(&st);
            // over into the town, an item down there
            on_foot(&mut st, a.step(d));
            hand_over(&mut st).expect("into the town");
            assert_eq!(o(&st).here(), town);
            let stim = game::ItemId::new("stim_patch");
            st.character.give_item(&st.content, stim.clone(), 1);
            let drop = game::Action::Drop {
                item: stim.clone(),
                qty: 1,
            };
            assert!(crate::actions::perform(&mut st, drop).is_ok());
            let left = world_of(&st, st.player.as_ref().unwrap().pos);
            // home, and saved there
            crate::outside::enter(&mut st, home).unwrap();
            assert_eq!(cards(&st), (0, 0), "the city's cards stay with it");
            let mut back = crate::outside::tests::reopen(st, "kept-cities");
            assert!(o(&back).held_city(c).is_some(), "the city kept");
            assert!(o(&back).held_city(town).is_some(), "the town kept");
            // the city: the cards back, the job still in hand, and handed in
            crate::outside::enter(&mut back, c).unwrap();
            assert_eq!(cards(&back), held, "the cards back as within a session");
            let taken = back.jobs.get(id).map(|j| j.state);
            assert_eq!(taken, Some(crate::jobs::State::Taken), "the job in hand");
            crate::jobs::ui::hand_over(&mut back, id).unwrap();
            crate::jobs::ui::hand_in(&mut back, id).unwrap();
            // the town: the item where it was left
            crate::outside::enter(&mut back, town).unwrap();
            let street = Place::Stratum(StratumKind::Street);
            let at = local(&back, left);
            assert!(
                back.ground.at(street, at).iter().any(|s| s.item == stim),
                "the town kept what you left in it"
            );
        }

        #[test]
        fn crossing_back_and_forth_neither_copies_nor_reseeds() {
            // the city and the town each kept whole as you step between them: the same
            // map and the same people every time, never a copy, never seeded again
            let mut st = home_run();
            let (c, town, a, d) = beside_a_town(&mut st);
            let map = |st: &AppState| {
                st.city
                    .as_ref()
                    .unwrap()
                    .stratum(StratumKind::Street)
                    .unwrap()
                    .tiles
                    .as_slice()
                    .as_ptr()
            };
            let people = |st: &AppState| st.world.as_ref().unwrap().people().as_ptr();
            let theirs = o(&st)
                .held_city(town)
                .unwrap()
                .stratum(StratumKind::Street)
                .unwrap()
                .tiles
                .as_slice()
                .as_ptr();
            let (city_map, city_people) = (map(&st), people(&st));
            on_foot(&mut st, a.step(d));
            hand_over(&mut st).expect("into the town");
            assert_eq!(map(&st), theirs, "the town as it was held, not a copy");
            let town_people = people(&st);
            for _ in 0..2 {
                let back = st.player.as_ref().unwrap().pos.step(d.opposite());
                on_foot(&mut st, back);
                hand_over(&mut st).expect("into the city");
                assert_eq!(o(&st).here(), c);
                assert_eq!(
                    (map(&st), people(&st)),
                    (city_map, city_people),
                    "the city kept"
                );
                let over = st.player.as_ref().unwrap().pos.step(d);
                on_foot(&mut st, over);
                hand_over(&mut st).expect("into the town");
                assert_eq!(o(&st).here(), town);
                assert_eq!(
                    (map(&st), people(&st)),
                    (theirs, town_people),
                    "the town kept"
                );
            }
        }

        #[test]
        fn a_call_holds_you_out_of_a_handover() {
            // on another city's cell with a call on the line (a run opened there): the
            // call stays with the home story, nothing handed over
            let mut st = home_run();
            let (_, x, d) = beside_a_city(&mut st);
            st.missions.call = Some(crate::missions::Call {
                story: None,
                from: "mezz".into(),
                lines: vec!["you there?".into()],
                rang: st.clock_tick(),
                answered: None,
                line: 0,
                shown: 0,
                full: false,
            });
            on_foot(&mut st, x.step(d));
            let home = o(&st).home();
            let log = st.log.len();
            assert_eq!(hand_over(&mut st), None);
            assert_eq!(o(&st).here(), home);
            assert!(st.missions.call.is_some(), "the call still on the line");
            assert_eq!(st.log.len(), log, "nothing said: the step says why");
        }

        #[test]
        fn a_run_saved_right_after_a_handover_reopens_there() {
            let mut st = home_run();
            let (s, x, d) = beside_a_city(&mut st);
            on_foot(&mut st, x.step(d));
            hand_over(&mut st).expect("handed over");
            let (seed, at) = (
                st.city.as_ref().unwrap().seed,
                world_of(&st, st.player.as_ref().unwrap().pos),
            );
            let back = crate::outside::tests::reopen(st, "handed-over");
            assert_eq!(o(&back).here(), s);
            assert_eq!(back.city.as_ref().unwrap().seed, seed);
            assert!(back.away.is_some(), "home set aside");
            let p = back.player.as_ref().unwrap();
            assert_eq!(p.place, Place::Stratum(StratumKind::Street));
            assert_eq!(world_of(&back, p.pos), at, "at your cell");
        }

        #[test]
        fn a_run_saved_after_a_net_handover_reopens_there() {
            // the body at home, the Net self in the next city: reopened there, and
            // jacking out goes home to the body
            let mut st = home_run();
            let home = o(&st).home();
            let (s, x, d, _) = jacked_beside_a_city(&mut st);
            st.player.as_mut().unwrap().pos = x.step(d);
            hand_over(&mut st).expect("handed over");
            let ends = |st: &AppState| {
                let p = st.player.as_ref().unwrap();
                let (place, b) = p.body.expect("jacked in");
                (p.place, world_of(st, p.pos), place, world_of(st, b))
            };
            let was = ends(&st);
            let mut back = crate::outside::tests::reopen(st, "net-handover");
            assert_eq!(o(&back).here(), s);
            assert!(back.away.is_some(), "home set aside, the body in it");
            assert_eq!(ends(&back), was);
            crate::net::jack_out(&mut back).unwrap();
            hand_over(&mut back).expect("home again");
            assert_eq!(o(&back).here(), home);
            let p = back.player.as_ref().unwrap();
            assert_eq!(
                (p.place, world_of(&back, p.pos), p.body),
                (Place::Stratum(StratumKind::Street), was.3, None)
            );
            // the body in the next city, the Net self walked home on the Net: the city
            // the body stands in is kept over a reopen, its map made again from the
            // world, to jack out into
            let mut st = home_run();
            let spawn = world_of(&st, st.player.as_ref().unwrap().pos);
            let (s, x, d) = beside_a_city(&mut st);
            on_foot(&mut st, x.step(d));
            hand_over(&mut st).expect("into the next city on foot");
            let body = st.player.as_ref().unwrap().pos;
            let body_at = world_of(&st, body);
            let net_at = local(&st, spawn);
            jacked(&mut st, (Place::Stratum(StratumKind::Street), body), net_at);
            hand_over(&mut st).expect("home on the Net");
            assert_eq!(o(&st).here(), home);
            let mut back = crate::outside::tests::reopen(st, "net-handover-home");
            assert_eq!(o(&back).here(), home);
            assert!(back.away.is_none(), "home is live");
            let p = back.player.as_ref().unwrap();
            assert_eq!(world_of(&back, p.pos), spawn, "the Net self where it stood");
            let (_, b) = p.body.expect("still jacked in");
            assert_eq!(world_of(&back, b), body_at, "the body where it stands");
            assert!(o(&back).held_city(s).is_some(), "kept over a reopen");
            assert!(
                crate::outside::body_stay(&back).is_some(),
                "the body's city held as you left it"
            );
            crate::net::jack_out(&mut back).unwrap();
            hand_over(&mut back).expect("into the city the body stands in");
            assert_eq!(o(&back).here(), s);
            let p = back.player.as_ref().unwrap();
            assert_eq!(
                (p.place, world_of(&back, p.pos), p.body),
                (Place::Stratum(StratumKind::Street), body_at, None)
            );
        }
    }
}
