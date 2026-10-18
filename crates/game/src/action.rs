//! Item actions: everything a menu can do, applied by one function.

use crate::build::Build;
use crate::character::Character;
use crate::content::Content;
use crate::ground::Ground;
use crate::model::*;
use citygen::{Place, Pos};

/// Everything a menu can ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Equip(ItemId),
    Unequip(Slot),
    Drop {
        item: ItemId,
        qty: u32,
    },
    Use(ItemId),
    Take {
        item: ItemId,
        qty: u32,
    },
    TakeAll,
    Fire(AbilityId),
    /// Spends a creation point on a stat.
    BuyStat(Stat),
    /// Spends SP on a category; its free skills come along.
    Unlock(CategoryId),
    /// Spends SP on a skill of an owned category.
    UnlockSkill(SkillId),
    /// Spends a talent point on a rank.
    Learn(TalentId),
    /// Loads a program from the pack into the jacked deck.
    Load(ItemId),
    Unload(ItemId),
    /// Slots a program at a position in the deck, sending back what it covers.
    Slot {
        id: ItemId,
        at: usize,
    },
}

/// Where the character stands: where drops land and takes come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct At {
    pub place: Place,
    pub pos: Pos,
}

/// A consequence the app must carry out on the world.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Reveal { radius: i32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub message: String,
    pub effects: Vec<Effect>,
}

fn done(message: impl Into<String>) -> Result<Outcome, String> {
    Ok(Outcome {
        message: message.into(),
        effects: Vec::new(),
    })
}

fn name(c: &Content, id: &ItemId) -> String {
    c.item(id)
        .map(|i| i.name.clone())
        .unwrap_or_else(|| id.to_string())
}

/// Removes up to `qty` of an item from the pack; returns how many came out.
pub fn remove_from_pack(ch: &mut Character, item: &ItemId, qty: u32) -> u32 {
    // A program that leaves the pack leaves the deck.
    if ch.count(item) <= qty {
        ch.loaded.retain(|l| l != item);
    }
    let mut left = qty;
    for s in ch.inventory.iter_mut().filter(|s| &s.item == item) {
        let n = left.min(s.qty);
        s.qty -= n;
        left -= n;
        if left == 0 {
            break;
        }
    }
    ch.inventory.retain(|s| s.qty > 0);
    qty - left
}

fn clamp_hp(ch: &mut Character, c: &Content) {
    let max = ch.hp_max(c);
    if ch.hp > max {
        ch.hp = max;
    }
}

fn over_capacity(ch: &Character, c: &Content) -> String {
    format!(
        "over capacity · {:.1}/{:.1} kg",
        ch.weight(c),
        ch.carry_cap(c)
    )
}

/// Applies one action to the character and the ground. Refusals come back as a
/// plain reason and change nothing.
pub fn apply(
    ch: &mut Character,
    ground: &mut Ground,
    c: &Content,
    at: At,
    tick: u64,
    action: Action,
) -> Result<Outcome, String> {
    match action {
        Action::Equip(id) => {
            let item = c.item(&id).ok_or_else(|| format!("unknown item {id}"))?;
            if ch.count(&id) == 0 {
                return Err(format!("{} is not in your pack", item.name));
            }
            let slot = item
                .slot
                .ok_or_else(|| format!("{} has no slot", item.name))?;
            remove_from_pack(ch, &id, 1);
            let old = ch.equipment.insert(slot, id.clone());
            clamp_hp(ch, c);
            match old {
                Some(old) => {
                    let old_name = name(c, &old);
                    ch.give(old, 1);
                    done(format!("swapped `{old_name}` for `{}`", item.name))
                }
                None => done(format!("equipped `{}`", item.name)),
            }
        }
        Action::Unequip(slot) => {
            let id = ch
                .equipment
                .remove(&slot)
                .ok_or_else(|| format!("nothing in the {} slot", slot.label()))?;
            ch.give(id.clone(), 1);
            clamp_hp(ch, c);
            done(format!("unequipped `{}`", name(c, &id)))
        }
        Action::Drop { item, qty } => {
            let n = qty.min(ch.count(&item));
            if n == 0 {
                return Err(format!("no {} to drop", name(c, &item)));
            }
            remove_from_pack(ch, &item, n);
            ground.add(at.place, at.pos, item.clone(), n);
            done(format!("dropped {n}× `{}`", name(c, &item)))
        }
        Action::Use(id) => {
            let item = c.item(&id).ok_or_else(|| format!("unknown item {id}"))?;
            if ch.count(&id) == 0 {
                return Err(format!("{} is not in your pack", item.name));
            }
            let on_use = item
                .on_use
                .ok_or_else(|| format!("{} cannot be used", item.name))?;
            remove_from_pack(ch, &id, 1);
            match on_use {
                Use::Heal(n) => {
                    let healed = ch.hp_max(c).saturating_sub(ch.hp).min(n);
                    ch.hp += healed;
                    done(format!("used `{}` · +{healed} hp", item.name))
                }
            }
        }
        Action::Take { item, qty } => {
            if ch.overburdened(c) {
                return Err(over_capacity(ch, c));
            }
            let have = ground
                .at(at.place, at.pos)
                .iter()
                .find(|s| s.item == item)
                .map(|s| s.qty)
                .unwrap_or(0);
            let n = qty.min(have);
            if n == 0 {
                return Err(format!("no {} here", name(c, &item)));
            }
            ground.take(at.place, at.pos, &item, n);
            ch.give(item.clone(), n);
            done(format!("took {n}× `{}`", name(c, &item)))
        }
        Action::TakeAll => {
            if ch.overburdened(c) {
                return Err(over_capacity(ch, c));
            }
            let stacks = ground.at(at.place, at.pos).to_vec();
            if stacks.is_empty() {
                return Err("nothing here".to_string());
            }
            let n: u32 = stacks.iter().map(|s| s.qty).sum();
            for s in stacks {
                ground.take(at.place, at.pos, &s.item, s.qty);
                ch.give(s.item, s.qty);
            }
            done(format!("took {n} items"))
        }
        Action::BuyStat(stat) => {
            let mut b = Build::from_character(c, ch);
            b.buy_stat(c, stat).map_err(|e| e.to_string())?;
            ch.stats = b.stats.clone();
            ch.stat_points = b.points_left(c).max(0) as u32;
            done(format!("{} {}", stat.name(), ch.stats.get(stat)))
        }
        Action::Unlock(id) => {
            let cat = c
                .category(&id)
                .ok_or_else(|| format!("unknown category {id}"))?;
            if ch.has_category(&id) {
                return Err(format!("{} is yours already", cat.name));
            }
            let mut b = Build::from_character(c, ch);
            b.toggle_category(c, &id).map_err(|e| e.to_string())?;
            ch.categories = b.categories.clone();
            ch.skills = b.skills.clone();
            ch.sp = b.sp_left(c).max(0) as u32;
            done(format!("`{}` is yours", cat.name))
        }
        Action::UnlockSkill(id) => {
            let skill = c.skill(&id).ok_or_else(|| format!("unknown skill {id}"))?;
            if ch.has_skill(&id) {
                return Err(format!("{} is yours already", skill.name));
            }
            let mut b = Build::from_character(c, ch);
            b.toggle_skill(c, &id).map_err(|e| e.to_string())?;
            ch.skills = b.skills.clone();
            ch.sp = b.sp_left(c).max(0) as u32;
            done(format!("`{}` is yours", skill.name))
        }
        Action::Learn(id) => {
            let t = c
                .talent(&id)
                .ok_or_else(|| format!("unknown talent {id}"))?;
            let rank = ch.learn(c, &id)?;
            done(format!("`{}` {}/{}", t.name, rank, t.ranks))
        }
        Action::Load(id) => ch.load(c, &id).and_then(done),
        Action::Unload(id) => ch.unload(c, &id).and_then(done),
        Action::Slot { id, at } => ch.slot_at(c, &id, at).and_then(done),
        Action::Fire(id) => {
            let ability = ch
                .ability(c, &id)
                .ok_or_else(|| format!("unknown ability {id}"))?;
            if !ch.abilities(c).iter().any(|(a, _)| a == &id) {
                return Err(format!("you do not have {}", ability.name));
            }
            let AbilityKind::Active(active) = ability.kind else {
                return Err(format!("{} is passive", ability.name));
            };
            if let Some(left) = ch.cooling(c, &id, tick) {
                return Err(format!(
                    "{} cooling down · {}s",
                    ability.name,
                    left.div_ceil(30)
                ));
            }
            ch.fired.insert(id.clone(), tick);
            let effects = match active {
                Active::Scan { radius } => vec![Effect::Reveal { radius }],
                Active::Ping { .. } | Active::Trace { .. } => Vec::new(),
            };
            Ok(Outcome {
                message: format!("{}: pulse sent", ability.name),
                effects,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::Stack;

    fn setup() -> (Content, Character, Ground, At) {
        let c = Content::load().unwrap();
        let ch = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        let at = At {
            place: Place::Stratum(citygen::StratumKind::Street),
            pos: Pos::new(5, 5),
        };
        (c, ch, Ground::default(), at)
    }

    fn id(s: &str) -> ItemId {
        ItemId::new(s)
    }

    #[test]
    fn equip_swaps_and_unequip_returns() {
        let (c, mut ch, mut g, at) = setup();
        assert!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Equip(id("hardhat"))).is_err(),
            "not carried"
        );
        assert!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Equip(id("copper_coil"))).is_err(),
            "no slot"
        );
        ch.inventory.push(Stack {
            item: id("hardhat"),
            qty: 1,
        });
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Equip(id("hardhat"))).unwrap();
        assert_eq!(out.message, "equipped `Hardhat with visor`");
        assert_eq!(ch.equipped(Slot::Head), Some(&id("hardhat")));
        assert_eq!(ch.count(&id("hardhat")), 0);
        assert!(!ch.inventory.iter().any(|s| s.qty == 0));
        ch.inventory.push(Stack {
            item: id("mirrorshades"),
            qty: 1,
        });
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Equip(id("mirrorshades")),
        )
        .unwrap();
        assert_eq!(
            out.message,
            "swapped `Nightglass optics` for `Mirrorshades`"
        );
        assert_eq!(ch.count(&id("nightglass_optics")), 1);
        assert_eq!(ch.sight_bonus(&c), 0);
        assert!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Unequip(Slot::Legs)).is_err(),
            "empty slot"
        );
        ch.hp = 47;
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Unequip(Slot::Torso)).unwrap();
        assert_eq!(out.message, "unequipped `Salvage jacket`");
        assert_eq!(ch.count(&id("salvage_jacket")), 1);
        assert_eq!((ch.hp, ch.hp_max(&c)), (42, 42), "hp clamps to the new max");
    }

    #[test]
    fn use_heals_and_consumes() {
        let (c, mut ch, mut g, at) = setup();
        ch.hp = 20;
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Use(id("stim_patch"))).unwrap();
        assert_eq!(out.message, "used `Stim patch` · +15 hp");
        assert_eq!((ch.hp, ch.count(&id("stim_patch"))), (35, 0));
        assert!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Use(id("stim_patch"))).is_err(),
            "none left"
        );
        assert!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Use(id("copper_coil"))).is_err(),
            "no use"
        );
        ch.hp = 44;
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Use(id("soy_noodle_cup")),
        )
        .unwrap();
        assert_eq!(out.message, "used `Soy-noodle cup` · +1 hp");
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Use(id("soy_noodle_cup")),
        )
        .unwrap();
        assert_eq!(out.message, "used `Soy-noodle cup` · +0 hp");
        assert_eq!(ch.count(&id("soy_noodle_cup")), 0);
        // Above the max (a console heal, a later buff): nothing happens, nothing panics.
        ch.hp = 60;
        ch.inventory.push(Stack {
            item: id("stim_patch"),
            qty: 1,
        });
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Use(id("stim_patch"))).unwrap();
        assert_eq!(out.message, "used `Stim patch` · +0 hp");
        assert_eq!(ch.hp, 60);
    }

    #[test]
    fn drop_and_take_move_stacks_through_the_ground() {
        let (c, mut ch, mut g, at) = setup();
        let drop = |n| Action::Drop {
            item: id("copper_coil"),
            qty: n,
        };
        let take = |n| Action::Take {
            item: id("copper_coil"),
            qty: n,
        };
        let out = apply(&mut ch, &mut g, &c, at, 0, drop(5)).unwrap();
        assert_eq!(out.message, "dropped 3× `Copper coil`");
        assert_eq!(ch.count(&id("copper_coil")), 0);
        assert_eq!(g.at(at.place, at.pos)[0].qty, 3);
        assert!(apply(&mut ch, &mut g, &c, at, 0, drop(1)).is_err());
        let out = apply(&mut ch, &mut g, &c, at, 0, take(2)).unwrap();
        assert_eq!(out.message, "took 2× `Copper coil`");
        assert_eq!(ch.count(&id("copper_coil")), 2);
        let elsewhere = At {
            pos: Pos::new(9, 9),
            ..at
        };
        assert!(apply(&mut ch, &mut g, &c, elsewhere, 0, take(1)).is_err());
        g.add(at.place, at.pos, id("dead_phone"), 2);
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::TakeAll).unwrap();
        assert_eq!(out.message, "took 3 items", "units, not stacks");
        assert!(g.at(at.place, at.pos).is_empty());
        assert_eq!(ch.count(&id("copper_coil")), 3);
        ch.inventory.push(Stack {
            item: id("scrap_servo"),
            qty: 40,
        });
        g.add(at.place, at.pos, id("bottle_caps"), 1);
        let err = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Take {
                item: id("bottle_caps"),
                qty: 1,
            },
        )
        .unwrap_err();
        assert!(err.starts_with("over capacity · "), "{err}");
        assert!(apply(&mut ch, &mut g, &c, at, 0, Action::TakeAll).is_err());
    }

    #[test]
    fn fire_respects_grants_kinds_and_cooldowns() {
        let (c, mut ch, mut g, at) = setup();
        let scan = || Action::Fire(AbilityId::new("netscan"));
        let out = apply(&mut ch, &mut g, &c, at, 100, scan()).unwrap();
        assert_eq!(out.effects, vec![Effect::Reveal { radius: 12 }]);
        assert_eq!(out.message, "Netscan: pulse sent");
        let err = apply(&mut ch, &mut g, &c, at, 110, scan()).unwrap_err();
        assert_eq!(err, "Netscan cooling down · 10s");
        assert!(apply(&mut ch, &mut g, &c, at, 500, scan()).is_ok());
        assert!(
            apply(
                &mut ch,
                &mut g,
                &c,
                at,
                0,
                Action::Fire(AbilityId::new("ping"))
            )
            .is_err(),
            "not granted"
        );
        assert!(
            apply(
                &mut ch,
                &mut g,
                &c,
                at,
                0,
                Action::Fire(AbilityId::new("lowlight"))
            )
            .is_err(),
            "passive"
        );
        ch.equipment.insert(Slot::Neural, id("neural_port"));
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Fire(AbilityId::new("ping")),
        )
        .unwrap();
        assert!(
            out.effects.is_empty(),
            "ping is visual only: {:?}",
            out.effects
        );
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Fire(AbilityId::new("arc_trace")),
        )
        .unwrap();
        assert_eq!(out.message, "Arc Trace: pulse sent");
        assert!(out.effects.is_empty(), "trace is visual only");
    }

    #[test]
    fn spending_from_the_pools() {
        let (c, mut ch, mut g, at) = setup();
        assert!(
            apply(&mut ch, &mut g, &c, at, 0, Action::BuyStat(Stat::Body)).is_err(),
            "no points"
        );
        ch.stat_points = 3;
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::BuyStat(Stat::Body)).unwrap();
        assert_eq!((ch.stats.body, ch.stat_points), (5, 0));
        assert_eq!(out.message, "BODY 5");
        ch.sp = 2;
        assert!(
            apply(
                &mut ch,
                &mut g,
                &c,
                at,
                0,
                Action::UnlockSkill(SkillId::new("handguns"))
            )
            .is_err(),
            "IRON first"
        );
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Unlock(CategoryId::new("iron")),
        )
        .unwrap();
        assert!(
            ch.has_category(&CategoryId::new("iron")) && ch.has_skill(&SkillId::new("handguns"))
        );
        assert_eq!((ch.sp, out.message.as_str()), (0, "`IRON` is yours"));
        assert!(
            apply(
                &mut ch,
                &mut g,
                &c,
                at,
                0,
                Action::UnlockSkill(SkillId::new("longarm"))
            )
            .is_err(),
            "no SP"
        );
        ch.sp = 1;
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::UnlockSkill(SkillId::new("longarm")),
        )
        .unwrap();
        assert_eq!((ch.sp, out.message.as_str()), (0, "`Longarm` is yours"));
        assert!(
            apply(
                &mut ch,
                &mut g,
                &c,
                at,
                0,
                Action::Unlock(CategoryId::new("iron"))
            )
            .is_err(),
            "already yours"
        );
    }

    #[test]
    fn learn_spends_the_point_and_reports_the_rank() {
        let (c, mut ch, mut g, at) = setup();
        let first = c.tree(Stat::Net).unwrap().tiers[0][0].clone();
        let name = c.talent(&first).unwrap().name.clone();
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Learn(first.clone())).unwrap();
        assert!(
            out.message.starts_with(&format!("`{name}` 1/")),
            "{}",
            out.message
        );
        assert_eq!(ch.talent_rank(&first), 1);
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Learn(first)).unwrap_err(),
            "no points left"
        );
    }

    #[test]
    fn fire_uses_the_sharpened_reach_and_cooldown() {
        let (c, mut ch, mut g, at) = setup();
        // Wide band at its best reaches six further; ice pick cools sixty ticks sooner.
        ch.talents.insert(TalentId::new("wide_band"), 2);
        ch.talents.insert(TalentId::new("ice_pick"), 2);
        let scan = || Action::Fire(AbilityId::new("netscan"));
        let out = apply(&mut ch, &mut g, &c, at, 100, scan()).unwrap();
        assert_eq!(out.effects, vec![Effect::Reveal { radius: 18 }]);
        assert!(apply(&mut ch, &mut g, &c, at, 339, scan()).is_err());
        assert!(apply(&mut ch, &mut g, &c, at, 340, scan()).is_ok());
    }

    #[test]
    fn load_and_unload_actions_and_a_drop_unloads() {
        let (c, mut ch, mut g, at) = setup();
        let copy = ItemId::new("copy");
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Unload(copy.clone())).unwrap();
        assert_eq!(out.message, "`Copy` unloaded");
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Load(copy.clone())).unwrap();
        assert_eq!(out.message, "`Copy` loaded · 2/2 slots");
        apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Drop {
                item: copy.clone(),
                qty: 1,
            },
        )
        .unwrap();
        assert!(!ch.loaded.contains(&copy), "{:?}", ch.loaded);
        assert_eq!(ch.loaded_size(&c), 1);
    }

    #[test]
    fn apply_slot_action() {
        let (c, mut ch, mut g, at) = setup();
        let spoof = ItemId::new("spoof");
        ch.give(spoof.clone(), 1);
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Slot { id: spoof, at: 0 },
        )
        .unwrap();
        assert_eq!(out.message, "`Spoof` slotted · `Crack` to inventory");
    }
}
