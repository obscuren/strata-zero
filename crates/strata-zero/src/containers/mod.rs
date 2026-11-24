//! Containers: what the lockers, safes, crates, cabinets and drawers hold, which of them
//! are locked and how, what became of each, and taking what is in them. The pure roll is
//! `loot`, the saved shape `state`; spec `docs/superpowers/specs/2026-11-18-containers-design.md`.

use crate::app::AppState;
use citygen::rng::hash64;
use citygen::tile::{ContainerKind, wall_front};
use citygen::venue::VenueLayout;
use citygen::{BuildingId, Pos, SiteNode, Terrain};
use game::ItemId;

pub mod force;
pub mod loot;
pub mod state;
pub mod ways;

use loot::{TableKey, Tables};
use state::{ContainerState, Loot, Opened};

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) mod testutil;

/// Salt for a container's roll seed.
pub const CONTAINER_SALT: u64 = 0xC047_A1E5;
/// Salt for the seed that decides whether a locker or a drawer is locked.
pub const LOCK_SALT: u64 = 0x10C4_ED00;

/// Where a container stands: its building, level and tile (a wall-set one's own wall tile).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerAt {
    pub building: BuildingId,
    pub level: i32,
    pub at: Pos,
}

/// A container on a floor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Container {
    pub at: ContainerAt,
    pub kind: ContainerKind,
    /// The floor tile in front of a wall-set one.
    pub front: Option<Pos>,
    /// A live copy's layer spot: the job's guarded container.
    pub guarded: bool,
    /// The zone of the venue room holding it.
    pub zone: Option<u8>,
    /// The venue's `type_id`, else `"plain"`.
    pub site: String,
}

/// A container's lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lock {
    /// 1 to 3.
    pub grade: u8,
    /// The lock maker of a maker-graded lock (a safe, a layer spot).
    pub maker: Option<String>,
    /// Its node on its site's Net, when it has one.
    pub node: Option<SiteNode>,
}

/// Where a container stands with whoever is at it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Locked(Lock),
    /// No lock, never opened.
    Shut,
    Open(Vec<Loot>),
    Empty,
}

fn layout_of(st: &AppState, b: BuildingId) -> Option<&VenueLayout> {
    st.interiors.plan_of(b)?.venue.as_deref()
}

/// The zone of the room of level `level` holding `p`.
fn zone_at(layout: &VenueLayout, level: i32, p: Pos) -> Option<u8> {
    let x = layout.level(level)?;
    (x.rooms.iter().find(|r| r.spots.contains(&p)))
        .or_else(|| x.rooms.iter().find(|r| r.rect.contains(p)))
        .map(|r| r.zone)
}

/// The container on tile `at.at` of the session's floor, never one in the player's flat.
pub fn find(st: &AppState, at: ContainerAt) -> Option<Container> {
    let (_, floor) = st.interiors.floor(at.building, at.level)?;
    let tile = floor.tiles.get(at.at)?;
    let kind = tile.feature?.container()?;
    if let Some(h) = st.home.as_ref().map(|h| h.home)
        && h.building == at.building
        && h.level == at.level
        && h.rect.contains(at.at)
    {
        return None;
    }
    let front = if tile.terrain == Terrain::Wall {
        wall_front(&floor.tiles, at.at)
    } else {
        None
    };
    let guarded = crate::sitenet::layer_of(st, at.building)
        .is_some_and(|l| (l.containers.iter()).any(|c| c.level == at.level && c.at == at.at));
    let layout = layout_of(st, at.building);
    let zone = layout.and_then(|v| zone_at(v, at.level, front.unwrap_or(at.at)));
    let site = layout.map_or("plain", |v| v.type_id.as_str()).to_string();
    Some(Container {
        at,
        kind,
        front,
        guarded,
        zone,
        site,
    })
}

/// The seed a container's lock and contents roll from: the site's, the level and the tile.
pub fn seed(st: &AppState, c: &Container) -> u64 {
    let b = c.at.building;
    let site_seed = match (layout_of(st, b), st.interiors.copy(b).filter(|k| k.live)) {
        (Some(v), Some(copy)) => hash64(v.seed, copy.inputs.seed),
        (Some(v), None) => v.seed,
        (None, _) => hash64(st.city.as_ref().map_or(0, |c| c.seed), b.index() as u64),
    };
    let at = c.at.at;
    hash64(
        hash64(site_seed, CONTAINER_SALT),
        ((c.at.level as i64 as u64) << 32) ^ ((at.x as u16 as u64) << 16) ^ (at.y as u16 as u64),
    )
}

/// A container's lock: a layer spot's always, at the building's lock maker's grade; a
/// safe's the same; a locker or drawer's by the seed; a cabinet's in a room of the
/// `secure` zone or deeper; none on a crate.
pub fn lock(st: &AppState, c: &Container) -> Option<Lock> {
    let b = c.at.building;
    let by_seed = |percent: u64| hash64(seed(st, c), LOCK_SALT) % 100 < percent;
    let (grade, maker) = if c.guarded || c.kind == ContainerKind::Safe {
        let look = crate::ui::device::Look::pick(st, crate::ui::device::Device::Keypad(b));
        (crate::keyjack::rating(st, b), Some(look.name))
    } else {
        let locked = match c.kind {
            ContainerKind::Locker => by_seed(50),
            ContainerKind::Drawer => by_seed(25),
            ContainerKind::Cabinet => {
                let secure = layout_of(st, b)
                    .and_then(|v| v.zones.iter().position(|z| z == "secure"))
                    .map(|z| z as u8);
                matches!((secure, c.zone), (Some(s), Some(z)) if z >= s)
            }
            ContainerKind::Crate | ContainerKind::Safe => false,
        };
        if !locked {
            return None;
        }
        (1, None)
    };
    let at = SiteNode {
        building: b,
        level: c.at.level,
        at: c.at.at,
    };
    let node = crate::sitenet::net_of(st, b)
        .and_then(|n| n.node(at).map(|i| i.kind))
        .filter(|k| *k == citygen::NodeKind::Container)
        .map(|_| at);
    Some(Lock { grade, maker, node })
}

/// What became of a container, if anything has.
pub fn state(st: &AppState, at: ContainerAt) -> Option<&ContainerState> {
    if crate::venues::copy_live(st, at.building) {
        st.interiors
            .copy(at.building)?
            .containers
            .get(&(at.level, at.at))
    } else {
        st.interiors.container(at.building, at.level, at.at)
    }
}

/// The same, to change; a default one is put there if there was none.
pub fn state_mut(st: &mut AppState, at: ContainerAt) -> &mut ContainerState {
    if crate::venues::copy_live(st, at.building) {
        let map = st.interiors.copy_containers_mut(at.building);
        return map
            .expect("a live copy")
            .entry((at.level, at.at))
            .or_default();
    }
    st.interiors.container_mut(at.building, at.level, at.at)
}

/// Where it stands: opened or not, locked or not.
pub fn status(st: &AppState, c: &Container) -> Status {
    if state(st, c.at).is_some_and(|s| s.opened.is_some()) {
        let left = contents(st, c);
        return if left.is_empty() {
            Status::Empty
        } else {
            Status::Open(left)
        };
    }
    match lock(st, c) {
        Some(l) => Status::Locked(l),
        None => Status::Shut,
    }
}

/// How a container with something to show looks on the map beyond its closed body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerLook {
    /// Opened, with something left.
    Open,
    /// Opened, with nothing left.
    Empty,
    /// Locked, with a node on the site's Net, not yet opened.
    Net,
}

/// The containers of floor `level` of `b` that are open, empty or on the Net (closed ones are
/// the tileset's). Visits only the tiles with a state or a node, not the floor.
pub fn looks(st: &AppState, b: BuildingId, level: i32) -> Vec<(Pos, ContainerKind, ContainerLook)> {
    let mut tiles: std::collections::BTreeSet<Pos> = if crate::venues::copy_live(st, b) {
        st.interiors.copy_container_tiles(b, level)
    } else {
        st.interiors.container_tiles(b, level)
    }
    .into_iter()
    .collect();
    if let Some(net) = crate::sitenet::net_of(st, b) {
        tiles.extend(
            (net.nodes.iter())
                .filter(|n| n.node.level == level && n.kind == citygen::NodeKind::Container)
                .map(|n| n.node.at),
        );
    }
    tiles
        .into_iter()
        .filter_map(|at| {
            let c = find(
                st,
                ContainerAt {
                    building: b,
                    level,
                    at,
                },
            )?;
            let look = match status(st, &c) {
                Status::Open(_) => ContainerLook::Open,
                Status::Empty => ContainerLook::Empty,
                Status::Locked(l) if l.node.is_some() => ContainerLook::Net,
                _ => return None,
            };
            Some((at, c.kind, look))
        })
        .collect()
}

/// The look line: the word, the lock, and where it stands.
pub fn look_line(st: &AppState, c: &Container) -> String {
    let status = status(st, c);
    let mut s = c.kind.word().to_string();
    if let Some(l) = lock(st, c) {
        match &l.maker {
            Some(m) => s.push_str(&format!(" · {m} grade {}", l.grade)),
            None => s.push_str(&format!(" · grade {}", l.grade)),
        }
    }
    s.push_str(match &status {
        Status::Locked(_) => " · locked",
        Status::Shut => " · closed",
        Status::Open(_) => " · open",
        Status::Empty => " · empty",
    });
    if matches!(&status, Status::Locked(l) if l.node.is_some()) {
        s.push_str(" · on the Net");
    }
    s
}

/// The roll a container makes when it is opened; pure.
fn rolled(st: &AppState, c: &Container) -> Vec<Loot> {
    let b = c.at.building;
    let deeper = c.zone.and_then(|z| {
        let zones = layout_of(st, b)?.zones.len();
        (usize::from(z) + 1 < zones).then_some(z + 1)
    });
    let key = TableKey {
        kind: c.kind,
        site: &c.site,
        guarded: c.guarded,
        deeper,
    };
    loot::roll(
        Tables::shipped(),
        &key,
        crate::sitenet::security(st, b),
        seed(st, c),
    )
}

/// Whether this build has what a line names.
fn known(st: &AppState, l: &Loot) -> bool {
    match l {
        Loot::Item { item, .. } => st.content.item(item).is_some(),
        _ => true,
    }
}

/// What is in it: what is left once opened, else the roll. Lines naming an item this
/// build lacks are left out.
pub fn contents(st: &AppState, c: &Container) -> Vec<Loot> {
    let all = match state(st, c.at).and_then(|s| s.left.clone()) {
        Some(left) => left,
        None => rolled(st, c),
    };
    all.into_iter().filter(|l| known(st, l)).collect()
}

/// Opens it, once: how, and the roll it holds. Forcing a lock on the Net sets off the
/// building's alarm.
pub fn open(st: &mut AppState, c: &Container, how: Opened) {
    if state(st, c.at).is_some_and(|s| s.opened.is_some()) {
        return;
    }
    let on_net = lock(st, c).is_some_and(|l| l.node.is_some());
    let roll = rolled(st, c);
    let s = state_mut(st, c.at);
    s.opened = Some(how);
    s.left = Some(roll);
    if how == Opened::Forced && on_net {
        crate::net::lock_down(st, c.at.building);
    }
}

/// `1250` as `"1,250 cr"`.
pub fn cr(n: u32) -> String {
    format!("{} cr", crate::ui::commas(n))
}

/// The black credchip: its sum opens only at a rig.
const BLACK_CHIP: &str = "credchip_black";

fn chip_item(locked: bool) -> ItemId {
    ItemId::new(if locked {
        "credchip_locked"
    } else {
        "credchip"
    })
}

/// What a line weighs in the pack.
pub fn line_weight(st: &AppState, l: &Loot) -> f32 {
    let weight = |id: &ItemId| st.content.item(id).map_or(0.0, |i| i.weight);
    match l {
        Loot::Cash(_) | Loot::Card(_) => 0.0,
        Loot::Chip { locked, .. } => weight(&chip_item(*locked)),
        Loot::BlackChip(_) => weight(&ItemId::new(BLACK_CHIP)),
        Loot::Item { item, qty } => weight(item) * *qty as f32,
    }
}

/// What a line is called.
fn name_of(st: &AppState, l: &Loot) -> String {
    match l {
        Loot::Cash(n) => cr(*n),
        Loot::Chip { credits, .. } => format!("credchip, {}", cr(*credits)),
        Loot::BlackChip(credits) => format!("black credchip, {}", cr(*credits)),
        Loot::Item { item, qty } => {
            let name = st.content.item(item).map_or(item.0.as_str(), |i| &i.name);
            if *qty > 1 {
                format!("{name} ×{qty}")
            } else {
                name.to_string()
            }
        }
        Loot::Card(_) => "a keycard".to_string(),
    }
}

/// Moves line `raw` of what is left to the character: Err and nothing changed when it
/// won't fit.
fn take_raw(st: &mut AppState, c: &Container, raw: usize) -> Result<String, String> {
    let line = (state(st, c.at))
        .and_then(|s| s.left.as_ref())
        .and_then(|l| l.get(raw).cloned())
        .ok_or("empty")?;
    if !st.character.dev
        && st.character.weight(&st.content) + line_weight(st, &line)
            > st.character.carry_cap(&st.content)
    {
        return Err(game::over_capacity(&st.character, &st.content));
    }
    let said = name_of(st, &line);
    match line {
        Loot::Cash(n) => st.character.credits = st.character.credits.saturating_add(n),
        Loot::Chip { credits, locked } => {
            st.character
                .give_credchip(&st.content, &chip_item(locked), credits);
        }
        Loot::BlackChip(credits) => {
            st.character
                .give_credchip(&st.content, &ItemId::new(BLACK_CHIP), credits);
        }
        Loot::Item { item, qty } => st.character.give_item(&st.content, item, qty),
        Loot::Card(z) => {
            crate::credentials::stock_zone_cards(st, c.at.building);
            let card = crate::credentials::zone_card_id(c.at.building, z);
            st.character.give_item(&st.content, card, 1);
        }
    }
    if let Some(left) = state_mut(st, c.at).left.as_mut() {
        left.remove(raw);
    }
    Ok(said)
}

/// The index in what is left of line `line` of `contents`.
fn raw_index(st: &AppState, c: &Container, line: usize) -> Option<usize> {
    let left = state(st, c.at)?.left.as_ref()?;
    (0..left.len()).filter(|i| known(st, &left[*i])).nth(line)
}

/// Takes line `line` of what is in an open container.
pub fn take(st: &mut AppState, c: &Container, line: usize) -> Result<String, String> {
    let raw = raw_index(st, c, line).ok_or("empty")?;
    take_raw(st, c, raw).map(|said| format!("took {said}"))
}

/// Takes every line it can, stopping at the first that won't fit.
pub fn take_all(st: &mut AppState, c: &Container) -> Result<String, String> {
    let mut taken = Vec::new();
    while let Some(raw) = raw_index(st, c, 0) {
        match take_raw(st, c, raw) {
            Ok(said) => taken.push(said),
            Err(why) if taken.is_empty() => return Err(why),
            Err(why) => return Ok(format!("took {} · {why}", taken.join(", "))),
        }
    }
    if taken.is_empty() {
        Err("empty".to_string())
    } else {
        Ok(format!("took {}", taken.join(", ")))
    }
}
