//! The Keyjack's run: the game's numbers in `flavor/keyjack.ron`, and what a door, a
//! Keyjack copy with its fuse and the runner's skills make of them.

use crate::app::AppState;
use citygen::BuildingId;
use game::{CopyId, ItemCopy, Mod, SkillId};
use serde::Deserialize;
use std::sync::OnceLock;

const DEFAULT: &str = include_str!("../../../flavor/keyjack.ron");

/// The Keyjack's item id.
pub const ITEM: &str = "keyjack";
/// The stock fuse's item id: what a fixer pays on top of a fetch.
pub const STOCK_FUSE: &str = "fuse_stock";

/// The skill whose rank widens every zone.
const LOCKPICK: &str = "lockpick";
/// The talent flag that allows one more fault.
const SKELETON_KEY: &str = "skeleton_key";

/// The file's numbers.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Numbers {
    /// The bar's width in cells.
    pub bar: u16,
    /// Zone widths of the four beats, for each rating (index 0 is rating 1).
    pub zones: Vec<[u16; 4]>,
    /// The marker's cells per second on each beat.
    pub speeds: [f32; 4],
    /// Faults a run allows.
    pub faults: u8,
    /// Cells Lockpick adds to every zone.
    pub lockpick_zone: u16,
    /// Faults Skeleton key adds.
    pub skeleton_key_fault: u8,
    /// Each lock maker (a device theme) and its rating.
    pub makers: Vec<(String, u8)>,
}

/// Why the file is no good.
#[derive(Debug, thiserror::Error)]
pub enum NumbersError {
    #[error("keyjack parse error: {0}")]
    Parse(String),
    #[error("keyjack: {0}")]
    Bad(&'static str),
}

impl Numbers {
    /// Any text in the file's shape, checked: a bar, ratings, makers that name one.
    pub fn parse(src: &str) -> Result<Numbers, NumbersError> {
        let n: Numbers = ron::from_str(src).map_err(|e| NumbersError::Parse(e.to_string()))?;
        if n.bar == 0 {
            return Err(NumbersError::Bad("the bar has no cells"));
        }
        if n.zones.is_empty() {
            return Err(NumbersError::Bad("no ratings"));
        }
        if n.speeds.iter().any(|s| *s <= 0.0) {
            return Err(NumbersError::Bad("a beat doesn't move"));
        }
        if n.makers
            .iter()
            .any(|(_, r)| *r == 0 || *r as usize > n.zones.len())
        {
            return Err(NumbersError::Bad(
                "a maker rates a lock there is no row for",
            ));
        }
        Ok(n)
    }

    /// The shipped file.
    pub fn shipped() -> &'static Numbers {
        static N: OnceLock<Numbers> = OnceLock::new();
        N.get_or_init(|| Numbers::parse(DEFAULT).expect("the shipped keyjack.ron is valid"))
    }

    /// A maker's rating; a name the file doesn't list rates 1.
    pub fn rating_of(&self, maker: &str) -> u8 {
        self.makers
            .iter()
            .find(|(m, _)| m == maker)
            .map_or(1, |(_, r)| *r)
    }
}

/// What one run asks of the player.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The zone's width in cells on each beat.
    pub zones: [u16; 4],
    /// The marker's cells per second on each beat.
    pub speed: [f32; 4],
    /// Faults that lock the pad.
    pub faults: u8,
    /// The lock's rating, 1 to 3.
    pub rating: u8,
    /// The bar's width in cells.
    pub bar: u16,
}

/// The rating of the pad on a building's door, by its maker's theme.
pub fn rating(st: &AppState, building: BuildingId) -> u8 {
    let look = crate::ui::device::Look::pick(st, crate::ui::device::Device::Keypad(building));
    Numbers::shipped().rating_of(&look.name)
}

/// The run a Keyjack copy makes of this door with this runner: the rating's zones plus
/// the fuse's and Lockpick's, the faults plus the fuse's and Skeleton key's, and the
/// speeds at the fuse's sweep.
pub fn run_for(st: &AppState, building: BuildingId, keyjack: &ItemCopy) -> Run {
    let n = Numbers::shipped();
    let rating = rating(st, building).clamp(1, n.zones.len() as u8);
    let mods: Vec<Mod> = keyjack
        .fitted
        .first()
        .and_then(|s| s.as_ref())
        .and_then(|s| st.content.item(&s.item))
        .and_then(|i| i.hardware.as_ref())
        .map(|h| h.mods.clone())
        .unwrap_or_default();
    let mut zone_bonus = 0i32;
    let mut fault_bonus = 0i32;
    let mut sweep = 100u32;
    for m in &mods {
        match m {
            Mod::Zone(z) => zone_bonus += *z as i32,
            Mod::Faults(f) => fault_bonus += *f as i32,
            Mod::Sweep(p) => sweep = *p as u32,
            _ => {}
        }
    }
    if st.character.has_skill(&SkillId::new(LOCKPICK)) {
        zone_bonus += n.lockpick_zone as i32;
    }
    if st.character.flags(&st.content).contains(SKELETON_KEY) {
        fault_bonus += n.skeleton_key_fault as i32;
    }
    let zones =
        n.zones[rating as usize - 1].map(|w| (w as i32 + zone_bonus).clamp(1, n.bar as i32) as u16);
    let speed = n.speeds.map(|s| s * sweep as f32 / 100.0);
    Run {
        zones,
        speed,
        faults: (n.faults as i32 + fault_bonus).clamp(1, u8::MAX as i32) as u8,
        rating,
        bar: n.bar,
    }
}

/// The Keyjacks a runner carries in the pack, as a pad sees them.
#[derive(Clone, Copy, Debug)]
pub enum Carried<'a> {
    None,
    /// Every one has an empty socket.
    NoFuse,
    /// The first with a fuse in: the one that runs.
    Fused(&'a ItemCopy),
}

/// The Keyjacks in the pack (it has no slot to be worn in).
pub fn carried(st: &AppState) -> Carried<'_> {
    let mut jacks = st
        .character
        .inventory
        .iter()
        .filter(|s| s.item.0 == ITEM)
        .peekable();
    if jacks.peek().is_none() {
        return Carried::None;
    }
    jacks
        .filter_map(|s| s.copy.as_ref())
        .find(|c| c.fitted.first().is_some_and(Option::is_some))
        .map_or(Carried::NoFuse, Carried::Fused)
}

/// The last fault: the fuse in that Keyjack's socket burns out and is gone.
pub fn blow_fuse(st: &mut AppState, copy: CopyId) {
    let jack = st
        .character
        .inventory
        .iter_mut()
        .filter_map(|s| s.copy.as_mut())
        .find(|c| c.id == copy);
    if let Some(slot) = jack.and_then(|c| c.fitted.first_mut()) {
        *slot = None;
    }
}

/// A flat's door, as a seed for where its zones sit.
pub fn door_seed(st: &AppState, building: BuildingId, level: i32, number: u16) -> u64 {
    let city = st.city.as_ref().map_or(0, |c| c.seed);
    citygen::rng::hash64(
        citygen::rng::hash64(city, 0x4B1A_C000 + building.index() as u64),
        ((level as i64 as u64) << 16) ^ number as u64,
    )
}

/// Where a beat's zone starts (its left cell): fixed by the door and the beat, and
/// always whole inside the bar.
pub fn zone_at(door_seed: u64, beat: usize, width: u16, bar: u16) -> u16 {
    let room = bar.saturating_sub(width);
    if room == 0 {
        return 0;
    }
    (citygen::rng::hash64(door_seed, 0x4B31 ^ beat as u64) % (room as u64 + 1)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::{CopyId, ItemId, Stack};

    fn copy_with(fuse: Option<&str>) -> ItemCopy {
        ItemCopy {
            id: CopyId(1),
            fitted: vec![fuse.map(|f| Stack {
                item: ItemId::new(f),
                qty: 1,
                copy: None,
            })],
            stickers: Vec::new(),
        }
    }

    fn state() -> AppState {
        let mut st = AppState::new();
        st.enter_city(
            citygen::Generator::run(citygen::GenParams {
                seed: 6,
                size: citygen::SizePreset::Custom { w: 120, h: 60 },
                districts: 8,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    /// A building of the city and the rating its pad has.
    fn a_door(st: &AppState) -> (BuildingId, u8) {
        let b = st.city.as_ref().unwrap().buildings[0].id;
        (b, rating(st, b))
    }

    #[test]
    fn the_shipped_file_is_valid() {
        let n = Numbers::shipped();
        assert_eq!((n.bar, n.faults, n.zones.len()), (18, 3, 3));
    }

    #[test]
    fn ratings_follow_the_pads_maker() {
        let n = Numbers::shipped();
        assert_eq!(n.rating_of("street"), 1);
        assert_eq!(n.rating_of("municipal"), 2);
        assert_eq!(n.rating_of("neon"), 2);
        assert_eq!(n.rating_of("megacorp"), 3);
        assert_eq!(n.rating_of("no such maker"), 1);
        let st = state();
        let (_, r) = a_door(&st);
        assert!((1..=3).contains(&r));
        // No city: never a panic.
        assert!(rating(&AppState::new(), BuildingId(0)) >= 1);
    }

    #[test]
    fn zones_by_rating_and_bonuses() {
        let mut st = state();
        let (b, r) = a_door(&st);
        let base = Numbers::shipped().zones[r as usize - 1];
        let stock = copy_with(Some("fuse_stock"));
        assert_eq!(run_for(&st, b, &stock).zones, base);
        let wide = copy_with(Some("fuse_wideband"));
        assert_eq!(run_for(&st, b, &wide).zones, base.map(|w| w + 1));
        st.character.skills.insert(SkillId::new("lockpick"));
        assert_eq!(run_for(&st, b, &wide).zones, base.map(|w| w + 2));
        assert_eq!(run_for(&st, b, &stock).zones, base.map(|w| w + 1));
        assert_eq!(run_for(&st, b, &stock).rating, r);
        assert_eq!(run_for(&st, b, &stock).bar, 18);
    }

    #[test]
    fn zones_are_clamped_to_one_and_the_bar() {
        let mut st = state();
        let (b, _) = a_door(&st);
        let n = Numbers::shipped();
        st.character.skills.insert(SkillId::new("lockpick"));
        for fuse in ["fuse_stock", "fuse_wideband", "fuse_slowblow"] {
            for w in run_for(&st, b, &copy_with(Some(fuse))).zones {
                assert!((1..=n.bar).contains(&w));
            }
        }
        // A zone wider than the bar is cut to it.
        let mut wide = Numbers::parse(DEFAULT).unwrap();
        wide.zones = vec![[30, 20, 10, 1]];
        let clamped = wide.zones[0].map(|w| (w as i32).clamp(1, wide.bar as i32) as u16);
        assert_eq!(clamped, [18, 18, 10, 1]);
    }

    #[test]
    fn faults_by_bonuses() {
        let mut st = state();
        let (b, _) = a_door(&st);
        assert_eq!(run_for(&st, b, &copy_with(Some("fuse_stock"))).faults, 3);
        assert_eq!(run_for(&st, b, &copy_with(None)).faults, 3);
        assert_eq!(run_for(&st, b, &copy_with(Some("fuse_ceramic"))).faults, 4);
        st.character
            .talents
            .insert(game::TalentId::new("skeleton_key"), 1);
        assert_eq!(run_for(&st, b, &copy_with(Some("fuse_ceramic"))).faults, 5);
        assert_eq!(run_for(&st, b, &copy_with(Some("fuse_stock"))).faults, 4);
    }

    #[test]
    fn slowblow_slows() {
        let st = state();
        let (b, _) = a_door(&st);
        let base = Numbers::shipped().speeds;
        assert_eq!(run_for(&st, b, &copy_with(Some("fuse_stock"))).speed, base);
        let slow = run_for(&st, b, &copy_with(Some("fuse_slowblow"))).speed;
        for (s, w) in slow.iter().zip(base) {
            assert!((s - w * 0.75).abs() < 1e-4, "{s} vs {w}");
        }
    }

    #[test]
    fn zone_positions_are_deterministic_and_fit() {
        for seed in [0u64, 7, 0xDEAD_BEEF] {
            for beat in 0..4 {
                for width in 1..=18u16 {
                    let a = zone_at(seed, beat, width, 18);
                    assert_eq!(a, zone_at(seed, beat, width, 18));
                    assert!(a + width <= 18, "{a}+{width}");
                }
            }
        }
        assert_eq!(zone_at(1, 0, 18, 18), 0);
        let spread: std::collections::BTreeSet<u16> =
            (0..50).map(|s| zone_at(s, 1, 3, 18)).collect();
        assert!(spread.len() > 3, "positions vary with the door: {spread:?}");
    }
}
