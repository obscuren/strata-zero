//! Running programs against nodes: who wins a turn, and the trace that answers.

use crate::character::Character;
use crate::content::Content;
use crate::model::{Attribute, ItemId, Op, Program, SkillId};
use citygen::{BuildingId, IceGrade, NodeId, NodeKind};
use serde::{Deserialize, Serialize};

/// Heat at which a construct raises the alarm.
pub const ALARM_HEAT: u8 = 60;
/// Heat at which a construct dumps you.
pub const DUMP_HEAT: u8 = 100;
/// Game minutes an alarm keeps a building locked down.
pub const ALERT_MINUTES: u64 = 60;
/// Game minutes a landed hack lasts.
pub const HACK_MINUTES: u64 = 60;
/// Game minutes a dumped deck refuses to jack in.
pub const COOLING_MINUTES: u64 = 10;

/// What a program is pointed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Node(NodeKind),
    Guard(IceGrade),
    /// The runner's own deck: a program on yourself.
    Deck,
}

/// The exact thing a run works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HackTarget {
    Node(NodeId),
    Guard { building: BuildingId, index: usize },
    Deck,
}

/// Whether an op does anything to a target.
pub fn applies(op: Op, target: Target) -> bool {
    matches!(
        (op, target),
        (Op::Crack, Target::Node(_) | Target::Guard(_))
            | (Op::Attack, Target::Guard(_))
            | (Op::Shield | Op::Mask, Target::Deck)
            | (
                Op::Spoof,
                Target::Node(
                    NodeKind::DoorLock
                        | NodeKind::LiftCtrl
                        | NodeKind::Camera
                        | NodeKind::AdBoard
                        | NodeKind::LampGrid
                        | NodeKind::Terminal
                )
            )
            | (Op::Blind, Target::Node(NodeKind::Camera))
            | (
                Op::Copy,
                Target::Node(NodeKind::Server | NodeKind::DataVault)
            )
            | (Op::Backdoor, Target::Node(NodeKind::Server))
    )
}

/// The skill a run leans on.
pub fn skill_for(op: Op, target: Target) -> SkillId {
    match (op, target) {
        (Op::Crack, Target::Guard(_)) | (Op::Attack | Op::Shield, _) => SkillId::new("ice_breaker"),
        (Op::Mask, _) => SkillId::new("stealth"),
        (Op::Copy, _) => SkillId::new("vault_run"),
        _ => SkillId::new("hacking"),
    }
}

/// What the character brings to a turn: twice Net, three with the skill, twice the
/// program's rating, two with the talent flag that fits the op.
pub fn power(ch: &Character, c: &Content, p: &Program, target: Target) -> i32 {
    let mut out = 2 * ch.attribute(c, Attribute::Net) as i32 + 2 * p.rating as i32;
    if ch.has_skill(&skill_for(p.op, target)) {
        out += 3;
    }
    let flag = match p.op {
        Op::Crack => Some("ice_pick"),
        Op::Copy => Some("deep_dive"),
        _ => None,
    };
    if flag.is_some_and(|f| ch.flags(c).contains(f)) {
        out += 2;
    }
    out + ch.deck_power(c)
}

/// What a subnet's security puts against a turn.
pub fn difficulty(security: u8) -> i32 {
    security as i32 / 8
}

/// A turn succeeds when the roll (1..=10) and the power beat the difficulty.
pub fn succeeds(roll: u8, power: i32, difficulty: i32) -> bool {
    roll as i32 + power > difficulty
}

/// The chance one round lands, in percent: reflex and the skill carry it, every
/// tile past half range costs five, never below 5 nor above 95.
pub fn hit_chance(reflex: u8, skilled: bool, dist: i32, range: i32) -> u8 {
    hit_chance_with(reflex, skilled, dist, range, 0)
}

/// `hit_chance` with a bonus added before the clamp (a mark's).
pub fn hit_chance_with(reflex: u8, skilled: bool, dist: i32, range: i32, bonus: u8) -> u8 {
    let base = 50 + 4 * reflex as i32 + if skilled { 10 } else { 0 } + bonus as i32;
    let falloff = 5 * (dist - range / 2).max(0);
    (base - falloff).clamp(5, 95) as u8
}

/// What one ring of a blast does: the centre's damage falls by a step a ring, never
/// below 1 while the blast hurts at all.
pub fn blast_damage(damage: u8, radius: i32, ring: i32) -> u8 {
    if damage == 0 {
        return 0;
    }
    let left = (radius + 1 - ring).max(0);
    (damage as i32 * left / (radius + 1)).max(1) as u8
}

/// Harm after a resistance: the percent (clamped to -100..=90) turned aside, rounded to
/// nearest and never below 1 while the harm was more than none.
pub fn resisted(amount: u32, resist: i32) -> u32 {
    if amount == 0 {
        return 0;
    }
    resisted_stun(amount, resist).max(1)
}

/// `resisted` for a stun, which may be turned aside entirely.
pub fn resisted_stun(stun: u32, resist: i32) -> u32 {
    let r = resist.clamp(-100, 90) as i64;
    ((stun as i64 * (100 - r) + 50) / 100) as u32
}

/// The chance armour turns a round that hit, in percent: ten a point, at most 60.
pub fn turn_chance(armor: i32) -> u32 {
    armor.saturating_mul(10).clamp(0, 60) as u32
}

/// A round lands when the roll (1..=100) is within the chance.
pub fn round_hits(roll: u8, chance: u8) -> bool {
    roll <= chance
}

/// The chance a turn succeeds, in percent: how many of the ten rolls beat it.
pub fn chance(power: i32, difficulty: i32) -> u8 {
    (1..=10u8)
        .filter(|r| succeeds(*r, power, difficulty))
        .count() as u8
        * 10
}

/// A program in flight: how many successful turns it has, of how many it needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub program: ItemId,
    pub target: HackTarget,
    pub done: u8,
    pub work: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceEvent {
    None,
    Alarm,
    Dump,
}

/// A construct's trace: heat from noise, and the alert the alarm starts. Times are
/// game minutes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub heat: u8,
    pub alert_until: Option<u64>,
    /// Tenths of a heat point not yet paid: a cut trace is fractional, the meter is not.
    #[serde(default)]
    pub carry: u8,
}

impl Trace {
    /// Adds noise. Crossing the alarm line while not alert raises the alarm; reaching
    /// the dump line dumps.
    pub fn add(&mut self, noise: u8, now: u64) -> TraceEvent {
        self.add_cut(noise, 0, now)
    }

    /// `add` with a percent cut off the noise. The cut leaves tenths, which wait in
    /// `carry` until they make a whole point.
    pub fn add_cut(&mut self, noise: u8, cut: u8, now: u64) -> TraceEvent {
        let was_alert = self.alert(now);
        let tenths = noise as u32 * 100u32.saturating_sub(cut as u32) / 10 + self.carry as u32;
        self.carry = (tenths % 10) as u8;
        self.heat = self
            .heat
            .saturating_add((tenths / 10).min(255) as u8)
            .min(DUMP_HEAT);
        // A jump straight past the alarm line still starts the alert.
        let fresh_alarm = self.heat >= ALARM_HEAT && !was_alert;
        if fresh_alarm {
            self.alert_until = Some(now + ALERT_MINUTES);
        }
        if self.heat >= DUMP_HEAT {
            return TraceEvent::Dump;
        }
        if fresh_alarm {
            return TraceEvent::Alarm;
        }
        TraceEvent::None
    }

    pub fn alert(&self, now: u64) -> bool {
        self.alert_until.is_some_and(|until| now < until)
    }

    /// One game minute with nobody inside: heat falls by one, two with a backdoor;
    /// an alert that has ended resets the heat.
    pub fn cool(&mut self, now: u64, backdoor: bool) {
        if self.alert_until.is_some_and(|until| now >= until) {
            self.alert_until = None;
            self.heat = 0;
            return;
        }
        let by = if backdoor { 2 } else { 1 };
        self.heat = self.heat.saturating_sub(by);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::Character;
    use crate::content::Content;
    use crate::model::{ItemId, SkillId, TalentId, TemplateId};
    use citygen::{IceGrade, NodeKind};

    #[test]
    fn hit_chance_by_the_formula() {
        assert_eq!(hit_chance(10, true, 2, 10), 95);
        assert_eq!(hit_chance(10, true, 9, 10), 80);
        assert_eq!(hit_chance(4, true, 1, 8), 76);
        assert_eq!(hit_chance(0, false, 30, 8), 5);
        assert!(round_hits(80, 80) && !round_hits(81, 80));
    }

    #[test]
    fn the_run_maths() {
        let c = Content::load().unwrap();
        assert!(applies(Op::Crack, Target::Node(NodeKind::Camera)));
        assert!(applies(Op::Crack, Target::Guard(IceGrade::White)));
        assert!(!applies(Op::Blind, Target::Node(NodeKind::DoorLock)));
        assert!(applies(Op::Blind, Target::Node(NodeKind::Camera)));
        assert!(applies(Op::Spoof, Target::Node(NodeKind::LiftCtrl)));
        assert!(!applies(Op::Spoof, Target::Node(NodeKind::Server)));
        assert!(applies(Op::Spoof, Target::Node(NodeKind::Camera)));
        assert!(applies(Op::Spoof, Target::Node(NodeKind::LampGrid)));
        assert!(applies(Op::Copy, Target::Node(NodeKind::Server)));
        assert!(applies(Op::Copy, Target::Node(NodeKind::DataVault)));
        assert!(applies(Op::Backdoor, Target::Node(NodeKind::Server)));
        assert!(!applies(Op::Backdoor, Target::Node(NodeKind::DataVault)));
        assert!(!applies(Op::Copy, Target::Guard(IceGrade::Grey)));
        assert!(applies(Op::Attack, Target::Guard(IceGrade::Black)));
        assert!(!applies(Op::Attack, Target::Node(NodeKind::Server)));
        assert!(applies(Op::Shield, Target::Deck) && applies(Op::Mask, Target::Deck));
        assert!(!applies(Op::Shield, Target::Guard(IceGrade::Grey)));
        assert!(!applies(Op::Crack, Target::Deck));
        assert_eq!(
            skill_for(Op::Attack, Target::Guard(IceGrade::Grey)),
            SkillId::new("ice_breaker")
        );
        assert_eq!(
            skill_for(Op::Shield, Target::Deck),
            SkillId::new("ice_breaker")
        );
        assert_eq!(skill_for(Op::Mask, Target::Deck), SkillId::new("stealth"));
        assert_eq!(Op::ALL.len(), 8);
        for id in ["attack", "shield", "mask"] {
            assert!(
                c.item(&ItemId::new(id))
                    .is_some_and(|i| i.program.is_some()),
                "{id}"
            );
        }
        assert_eq!(
            skill_for(Op::Crack, Target::Guard(IceGrade::White)),
            SkillId::new("ice_breaker")
        );
        assert_eq!(
            skill_for(Op::Crack, Target::Node(NodeKind::DoorLock)),
            SkillId::new("hacking")
        );
        assert_eq!(
            skill_for(Op::Copy, Target::Node(NodeKind::Server)),
            SkillId::new("vault_run")
        );
        assert_eq!(
            skill_for(Op::Spoof, Target::Node(NodeKind::LiftCtrl)),
            SkillId::new("hacking")
        );
        let crack = c.item(&ItemId::new("crack")).unwrap().program.unwrap();
        let mut vex = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        // Net 6 plus the deck's perk: 2 × 7 + 3 (hacking) + 2 × 1.
        assert_eq!(
            power(&vex, &c, &crack, Target::Node(NodeKind::DoorLock)),
            19
        );
        vex.talents.insert(TalentId::new("ice_pick"), 1);
        assert_eq!(
            power(&vex, &c, &crack, Target::Node(NodeKind::DoorLock)),
            21
        );
        let kessler = Character::from_template(&c, &TemplateId::new("kessler")).unwrap();
        assert_eq!(
            power(&kessler, &c, &crack, Target::Node(NodeKind::DoorLock)),
            8
        );
        assert_eq!(difficulty(110), 13);
        assert_eq!(difficulty(200), 25);
        assert!(succeeds(4, 17, 13));
        assert!(!succeeds(1, 8, 13));
        assert!(succeeds(6, 8, 13));
        assert_eq!(chance(19, 14), 100);
        assert_eq!(chance(19, 25), 40);
        assert_eq!(chance(19, 31), 0);
        assert_eq!(chance(8, 13), 50);
    }

    #[test]
    fn the_trace_alarms_dumps_and_cools() {
        let mut t = Trace::default();
        assert_eq!(t.add(30, 0), TraceEvent::None);
        assert_eq!(t.heat, 30);
        assert_eq!(t.add(30, 0), TraceEvent::Alarm);
        assert_eq!(t.alert_until, Some(ALERT_MINUTES));
        assert!(t.alert(59) && !t.alert(60));
        assert_eq!(t.add(30, 1), TraceEvent::None, "already alert");
        assert_eq!(t.add(10, 1), TraceEvent::Dump);
        assert_eq!(t.heat, DUMP_HEAT);
        t.cool(2, false);
        assert_eq!(t.heat, 99);
        t.cool(3, true);
        assert_eq!(t.heat, 97);
        t.cool(60, false);
        assert_eq!(
            (t.heat, t.alert_until),
            (0, None),
            "the alert ending resets the heat"
        );
        let mut t = Trace::default();
        assert_eq!(t.add(100, 0), TraceEvent::Dump);
        assert!(t.alert(1), "a dump from cold starts the alert too");
        let mut t = Trace::default();
        t.add(5, 0);
        t.cool(1, true);
        t.cool(2, true);
        t.cool(3, true);
        assert_eq!(t.heat, 0, "never below zero");
    }

    #[test]
    fn trace_carries_tenths() {
        let mut t = Trace::default();
        t.add_cut(4, 10, 0);
        assert_eq!((t.heat, t.carry), (3, 6));
        t.add_cut(4, 10, 0);
        assert_eq!((t.heat, t.carry), (7, 2));
        let mut t = Trace::default();
        t.add_cut(4, 0, 0);
        assert_eq!((t.heat, t.carry), (4, 0));
        // A carry does not move the dump line.
        let mut t = Trace {
            heat: DUMP_HEAT - 4,
            carry: 7,
            ..Trace::default()
        };
        assert_eq!(t.add(4, 0), TraceEvent::Dump);
        assert_eq!(t.heat, DUMP_HEAT);
    }

    #[test]
    fn the_sidecar_adds_power() {
        let c = Content::load().unwrap();
        let mut ch = Character::debug_runner(&c);
        let p = c.item(&ItemId::new("crack")).unwrap().program.unwrap();
        let target = Target::Node(NodeKind::DoorLock);
        let bare = power(&ch, &c, &p, target);
        let at = ch
            .inventory
            .iter()
            .position(|s| s.item == ItemId::new("sidecar"))
            .unwrap();
        let piece = ch.inventory.remove(at);
        ch.worn.get_mut(&crate::model::Slot::Deck).unwrap().fitted[0] = Some(piece);
        assert_eq!(power(&ch, &c, &p, target), bare + 1);
        // Five over the bare power: mid-range, so the extra point shows.
        let hard = bare + 5;
        assert_eq!(chance(bare + 1, hard), chance(bare, hard) + 10);
    }

    #[test]
    fn resisted_rounds_and_clamps() {
        assert_eq!(resisted(10, 0), 10);
        assert_eq!(resisted(10, 15), 9, "8.5 rounds to 9");
        assert_eq!(resisted(4, 50), 2);
        assert_eq!(resisted(3, -18), 4);
        assert_eq!(resisted(5, 95), 1, "clamped to 90, floor 1");
        assert_eq!(resisted(0, -50), 0);
        assert_eq!(resisted(2, -300), 4, "clamped to -100");
        assert_eq!(resisted_stun(2, 40), 1);
        assert_eq!(resisted_stun(1, 60), 0, "a stun can be turned aside whole");
        assert_eq!(resisted_stun(2, -18), 2);
    }

    #[test]
    fn blast_damage_falls_off_by_ring() {
        assert_eq!(blast_damage(12, 2, 0), 12);
        assert_eq!(blast_damage(12, 2, 1), 8);
        assert_eq!(blast_damage(12, 2, 2), 4);
        assert_eq!(blast_damage(16, 3, 3), 4);
        assert_eq!(blast_damage(2, 3, 3), 1, "the floor");
        assert_eq!(blast_damage(0, 2, 0), 0, "stun-only");
    }
}
