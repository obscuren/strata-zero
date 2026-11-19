//! Turrets: a gun on a swivel, bolted to a floor, as a foe. This is its type and its
//! saved state; the field, the clock, the turn and the Net are built on it.

// Built on by the tasks after this one; drop this once the turn and the Net use it all.
#![allow(dead_code)]

use crate::app::AppState;
use crate::foes::{self, Foe, FoeMode};
use citygen::{Dir, Place, Pos, SiteNode};
use game::ItemId;
use serde::{Deserialize, Serialize};

#[cfg(test)]
pub(crate) mod testutil;

pub const TURRET_HP: u8 = 40;
/// Its plate at mid security, and at high.
pub const ARM_MID: u8 = 4;
pub const ARM_HIGH: u8 = 6;
pub const MAGAZINE: u8 = 30;
/// The item it shoots with.
pub const GUN: &str = "turret_gun";
pub const TURRET_MOVE: u8 = 1;
pub const TURRET_AP: u8 = 1;

/// Whose net the turret is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hacked {
    #[default]
    None,
    Blind,
    Yours,
}

/// What makes a foe a turret: its device, where it faces, its plate and magazine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turret {
    pub node: SiteNode,
    pub facing: Dir,
    pub arm: u8,
    pub arm_max: u8,
    pub rounds: u8,
    pub awake: bool,
    #[serde(default)]
    pub hacked: Hacked,
    /// The tile it turns toward while awake.
    #[serde(default)]
    pub told: Option<Pos>,
    /// The game minute it last woke or had you.
    #[serde(default)]
    pub since: u64,
    /// The clock tick it next turns, awake.
    #[serde(default)]
    pub next: u64,
}

impl Turret {
    /// A turret asleep, its magazine full.
    pub fn new(node: SiteNode, facing: Dir, arm: u8) -> Turret {
        Turret {
            node,
            facing,
            arm,
            arm_max: arm,
            rounds: MAGAZINE,
            awake: false,
            hacked: Hacked::None,
            told: None,
            since: 0,
            next: 0,
        }
    }
}

/// Its plate by the site's security band: high plates thicker; low lays no turrets and
/// takes the mid plate.
pub fn arm_for(security: u8) -> u8 {
    match crate::containers::loot::band(security) {
        crate::containers::loot::Band::High => ARM_HIGH,
        _ => ARM_MID,
    }
}

/// A turret foe on `place` at `pos`.
pub fn foe(place: Place, pos: Pos, t: Turret) -> Foe {
    Foe {
        name: "turret".to_string(),
        place,
        pos,
        hp: TURRET_HP,
        hp_max: TURRET_HP,
        reflex: foes::GOON_REFLEX,
        weapon: Some(ItemId::new(GUN)),
        sight: crate::cameras::CONE_DEPTH,
        mode: FoeMode::Idle,
        stunned: 0,
        down: false,
        inert: false,
        glyph: '▲',
        chrome: true,
        marked: false,
        ice: None,
        post: None,
        turret: Some(t),
    }
}

/// What a turret is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurretState {
    Asleep,
    Awake,
    Hunting,
    Blind,
    Yours,
    Wrecked,
}

/// None for a foe with no turret.
pub fn state(f: &Foe) -> Option<TurretState> {
    let t = f.turret.as_ref()?;
    Some(if f.down {
        TurretState::Wrecked
    } else {
        match t.hacked {
            Hacked::Blind => TurretState::Blind,
            Hacked::Yours => TurretState::Yours,
            Hacked::None if f.hunting() => TurretState::Hunting,
            Hacked::None if t.awake => TurretState::Awake,
            Hacked::None => TurretState::Asleep,
        }
    })
}

/// A standing turret on nobody's net but its own.
pub fn hostile(f: &Foe) -> bool {
    f.standing() && f.turret.as_ref().is_some_and(|t| t.hacked == Hacked::None)
}

/// A standing turret you hold.
pub fn is_yours(f: &Foe) -> bool {
    f.standing() && f.turret.as_ref().is_some_and(|t| t.hacked == Hacked::Yours)
}

/// The foe whose turret is `node`.
pub fn of_node(st: &AppState, node: SiteNode) -> Option<usize> {
    st.foes
        .iter()
        .position(|f| f.turret.as_ref().is_some_and(|t| t.node == node))
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::BuildingId;

    #[test]
    fn a_turret_is_a_foe_with_its_numbers() {
        let node = SiteNode {
            building: BuildingId(3),
            level: 1,
            at: Pos::new(5, 5),
        };
        let f = foe(
            Place::Floor {
                building: BuildingId(3),
                level: 1,
            },
            Pos::new(5, 5),
            Turret::new(node, Dir::SE, ARM_HIGH),
        );
        assert_eq!(
            (f.name.as_str(), f.hp, f.hp_max, f.reflex),
            ("turret", 40, 40, 5)
        );
        assert_eq!(f.weapon, Some(ItemId::new("turret_gun")));
        assert!(f.chrome && f.post.is_none() && f.ice.is_none());
        let t = f.turret.as_ref().unwrap();
        assert_eq!(
            (t.arm, t.arm_max, t.rounds, t.awake, t.hacked),
            (6, 6, 30, false, Hacked::None)
        );
        assert_eq!(state(&f), Some(TurretState::Asleep));
    }

    #[test]
    fn its_plate_goes_by_the_sites_security_band() {
        assert_eq!(arm_for(100), 4);
        assert_eq!(arm_for(169), 4);
        assert_eq!(arm_for(170), 6);
        assert_eq!(arm_for(40), 4);
    }

    #[test]
    fn the_mounted_gun_is_content() {
        let st = AppState::new();
        let w = st
            .content
            .item(&ItemId::new(GUN))
            .unwrap()
            .weapon
            .clone()
            .unwrap();
        assert_eq!(
            (w.damage, w.rounds, w.range, w.skill.0.as_str()),
            (5, 3, 8, "handguns")
        );
    }

    #[test]
    fn a_foe_saved_before_turrets_is_no_turret() {
        let goon = Foe::goon(Place::Stratum(citygen::StratumKind::Street), Pos::new(1, 1));
        let mut v = serde_json::to_value(&goon).unwrap();
        v.as_object_mut().unwrap().remove("turret");
        let back: Foe = serde_json::from_value(v).unwrap();
        assert_eq!(back.turret, None);
        assert_eq!(back, goon);
    }

    #[test]
    fn a_turret_comes_back_whole_from_a_save() {
        let mut st = crate::guards::testutil::ward();
        let i = testutil::turret_in_ward(&mut st, 0, Pos::new(10, 3), Dir::W);
        st.foes[i].turret.as_mut().unwrap().hacked = Hacked::Blind;
        let back = crate::runs::testutil::round(&st.foes);
        assert_eq!(back, st.foes);
    }
}
