//! The turret lot, DEVELOPER's COMBAT on TURRET: the lot with a street terminal in its
//! bottom-left corner, a turret in its top-left corner and a goon in its top-right. The
//! terminal jacks you into the lot's small Net, where the turret's node stands to be held,
//! blinded or taken over. The lot stays on alert all night, so the turret is awake and
//! sweeping from the start.

use super::{ARM_MID, Turret};
use crate::app::AppState;
use citygen::{
    BuildingId, City, Connector, ConnectorKind, Dir, Feature, Place, Pos, SiteNode, StratumKind,
};

/// The street terminal, in the sidewalk's bottom-left corner.
pub const TERMINAL: Pos = Pos::new(1, 17);
/// The turret, in the road's top-left corner, looking across the lot.
pub const TURRET: Pos = Pos::new(2, 2);
pub const TURRET_FACING: Dir = Dir::SE;
/// The goon, in the road's top-right corner.
pub const GOON: Pos = Pos::new(38, 2);
/// Where you stand: beside the terminal, out of the turret's reach.
pub const START: Pos = Pos::new(2, 16);
/// The building the lot's Net and its turret's node answer to: the lot's constructs'.
pub const LOT: BuildingId = BuildingId(0);
/// How long the lot stays on alert, in minutes: the night.
const ALERT: u64 = 24 * 60;

/// The lot of `seed` with its street terminal: a jack serving the street and the Net.
pub fn city(seed: u64) -> City {
    let mut city = City::lot(seed);
    if let Some(t) = city.tile_mut(StratumKind::Street, TERMINAL) {
        t.feature = Some(Feature::Terminal);
    }
    city.connectors.push(Connector {
        kind: ConnectorKind::Jack,
        pos: TERMINAL,
        serves: vec![StratumKind::Street, StratumKind::Net],
        public: true,
    });
    city
}

/// The turret's node on the lot's Net.
pub fn node() -> SiteNode {
    SiteNode {
        building: LOT,
        level: 0,
        at: TURRET,
    }
}

/// On the lot of `city` as it has just been entered: the turret and the goon stood up,
/// the lot's Net made, and the lot put on alert so the turret wakes.
pub fn lay(st: &mut AppState) {
    let place = Place::Stratum(StratumKind::Street);
    if let Some(t) = (st.city.as_mut()).and_then(|c| c.tile_mut(StratumKind::Street, TURRET)) {
        t.feature = Some(Feature::Turret);
    }
    st.foes.push(super::foe(
        place,
        TURRET,
        Turret::new(node(), TURRET_FACING, ARM_MID),
    ));
    st.foes.push(crate::foes::Foe::goon(place, GOON));
    st.constructs.insert(citygen::turret_lot_construct(node()));
    let now = st.net.now(st);
    st.net.trace(LOT).alert_until = Some(now + ALERT);
    super::tick(st);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holds::VerbDone;
    use crate::turrets::TurretState;

    /// The turret lot as DEVELOPER's COMBAT on TURRET opens it.
    fn turret_lot() -> AppState {
        let mut st = AppState::new();
        st.unlock_dev();
        let _ =
            crate::screens::title::combat_start(&mut st, crate::screens::title::CombatMap::Turret);
        st
    }

    fn turret(st: &AppState) -> usize {
        (st.foes.iter().position(|f| f.turret.is_some())).expect("a turret")
    }

    #[test]
    fn the_turret_lot_has_its_terminal_turret_and_goon_in_their_corners() {
        let st = turret_lot();
        let city = st.city.as_ref().unwrap();
        assert!(city.is_lot());
        let street = Place::Stratum(StratumKind::Street);
        let tile = |p| city.tile(StratumKind::Street, p).unwrap().feature;
        assert_eq!(tile(TERMINAL), Some(Feature::Terminal));
        assert_eq!(tile(TURRET), Some(Feature::Turret));
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (street, START));
        assert_eq!(p.terminal_in_reach(city, None), Some(TERMINAL));
        let i = turret(&st);
        assert_eq!((st.foes[i].place, st.foes[i].pos), (street, TURRET));
        assert_eq!(super::super::state(&st.foes[i]), Some(TurretState::Awake));
        let goons: Vec<_> = (st.foes.iter())
            .filter(|f| f.turret.is_none())
            .map(|f| (f.name.as_str(), f.pos))
            .collect();
        assert_eq!(goons, [("goon", GOON)]);
        assert!(st.net.engaged.is_none(), "no fight yet");
    }

    #[test]
    fn it_stays_awake_through_the_night() {
        let mut st = turret_lot();
        let i = turret(&st);
        st.tick += 300 * 60 * 5;
        crate::turrets::tick(&mut st);
        assert_eq!(crate::turrets::state(&st.foes[i]), Some(TurretState::Awake));
    }

    #[test]
    fn the_terminal_jacks_you_into_the_lots_net_with_the_turret_on_it() {
        let mut st = turret_lot();
        let line = crate::net::jack_in_here(&mut st).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Construct(LOT), "{line}");
        assert_eq!(p.body, Some((Place::Stratum(StratumKind::Street), START)));
        let k = st.constructs.get(LOT).unwrap();
        assert_eq!(p.pos, k.entry);
        assert_eq!(
            k.device(node()).map(|d| d.kind),
            Some(citygen::NodeKind::Turret)
        );
    }

    /// You on the lot's Net beside the turret's node, a crack landed on it.
    fn held(st: &mut AppState) {
        crate::net::jack_in_here(st).unwrap();
        let at = st.constructs.get(LOT).unwrap().device(node()).unwrap().at;
        let beside = citygen::Pos::new(at.x - 1, at.y);
        crate::constructs::land(st, Place::Construct(LOT), beside);
        st.net.beat = st.clock_tick();
        let lines = crate::net::tests::crack_to_landing(st, game::HackTarget::Site(node()));
        assert!(lines.iter().any(|l| l.contains("turret held")), "{lines:?}");
    }

    #[test]
    fn from_its_net_the_turret_goes_blind() {
        let mut st = turret_lot();
        held(&mut st);
        assert_eq!(
            crate::holds::use_verb(&mut st, 'o'),
            Ok(VerbDone::Said("turret blind".into()))
        );
        let i = turret(&st);
        assert_eq!(crate::turrets::state(&st.foes[i]), Some(TurretState::Blind));
    }

    #[test]
    fn from_its_net_the_turret_is_taken_over() {
        let mut st = turret_lot();
        assert!(st.character.flags(&st.content).contains("turncoat"));
        held(&mut st);
        assert_eq!(
            crate::holds::use_verb(&mut st, 'i'),
            Ok(VerbDone::Said("turret's yours".into()))
        );
        let i = turret(&st);
        assert_eq!(crate::turrets::state(&st.foes[i]), Some(TurretState::Yours));
    }

    /// The turret taken over from its Net, you back on the lot, the goon moved to `goon`
    /// and a fight on the street.
    fn yours_in_a_fight(goon: citygen::Pos) -> (AppState, usize, usize) {
        let mut st = turret_lot();
        held(&mut st);
        crate::holds::use_verb(&mut st, 'i').unwrap();
        crate::net::jack_out(&mut st).unwrap();
        let (t, g) = (
            turret(&st),
            (st.foes.iter().position(|f| f.name == "goon")).unwrap(),
        );
        st.foes[g].pos = goon;
        crate::net::engage(&mut st, Place::Stratum(StratumKind::Street), true);
        (st, t, g)
    }

    fn logged(st: &AppState, needle: &str) -> bool {
        st.log.iter().any(|l| l.text.contains(needle))
    }

    #[test]
    fn the_turret_you_hold_fires_on_the_goon_in_its_field() {
        // South-east of it, in the field it faces, three tiles off.
        let (mut st, t, g) = yours_in_a_fight(citygen::Pos::new(5, 5));
        assert!(crate::turrets::seated(&st, t));
        assert_eq!(crate::turrets::plan_word(&st, t), "fires");
        st.force_hit = Some(true);
        crate::turrets::step(&mut st, t);
        assert!(logged(&st, "turret fires on goon"));
        assert!(st.foes[g].hp < crate::foes::GOON_HP);
    }

    #[test]
    fn the_goon_fires_back_on_the_turret_when_it_is_nearer_than_you() {
        let (mut st, t, g) = yours_in_a_fight(citygen::Pos::new(5, 5));
        assert_eq!(crate::turrets::guard_mark(&st, g), Some(t));
        let you = st.player.as_ref().unwrap().pos;
        st.foes[g].mode = crate::foes::FoeMode::Hunting { seen: you, lost: 0 };
        st.force_hit = Some(true);
        crate::foes::step(&mut st, g, true, true);
        assert!(logged(&st, "goon fires on turret"));
    }
}
