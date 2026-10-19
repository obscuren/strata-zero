//! Item actions: everything a menu can do, applied by one function.

use crate::build::Build;
use crate::character::{Character, CopyId, Stack};
use crate::content::Content;
use crate::ground::Ground;
use crate::model::*;
use citygen::{Place, Pos};

/// Everything a menu can ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Wears the first of an item in pack order, a copy when it is one.
    Equip(ItemId),
    /// Wears one copy from the pack.
    EquipCopy(CopyId),
    Unequip(Slot),
    Drop {
        item: ItemId,
        qty: u32,
    },
    /// Drops one copy from the pack, with what is fitted in it.
    DropCopy(CopyId),
    Use(ItemId),
    /// Takes from the stacks without a copy; copies are taken by id.
    Take {
        item: ItemId,
        qty: u32,
    },
    /// Takes one copy from the ground, with what is fitted in it.
    TakeCopy(CopyId),
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

/// Takes up to `qty` of an item out of the pack and returns what came out, in pack
/// order: copies whole, stacks without a copy split.
pub fn take_from_pack(ch: &mut Character, item: &ItemId, qty: u32) -> Vec<Stack> {
    // A program that leaves the pack leaves the deck.
    if ch.count(item) <= qty {
        ch.loaded.retain(|l| l != item);
    }
    let mut out = Vec::new();
    let mut left = qty;
    let mut i = 0;
    while left > 0 && i < ch.inventory.len() {
        let s = &mut ch.inventory[i];
        if &s.item != item || s.qty == 0 {
            i += 1;
        } else if s.qty <= left {
            left -= s.qty;
            out.push(ch.inventory.remove(i));
        } else {
            s.qty -= left;
            out.push(Stack::new(item.clone(), left));
            left = 0;
        }
    }
    ch.inventory.retain(|s| s.qty > 0);
    out
}

/// Removes up to `qty` of an item from the pack; returns how many came out.
pub fn remove_from_pack(ch: &mut Character, item: &ItemId, qty: u32) -> u32 {
    take_from_pack(ch, item, qty).iter().map(|s| s.qty).sum()
}

/// Wears a stack of one taken from the pack; what the slot wore comes back to it.
fn equip_stack(ch: &mut Character, c: &Content, item: &Item, slot: Slot, s: Stack) -> String {
    let old = ch.wear(slot, s.item, s.copy);
    clamp_hp(ch, c);
    let mut message = match old {
        Some(old) => {
            let old_name = name(c, &old.item);
            ch.stow(old);
            format!("swapped `{old_name}` for `{}`", item.name)
        }
        None => format!("equipped `{}`", item.name),
    };
    // What is loaded always fits the jacked deck: a smaller one sends
    // the last loaded back to the pack.
    if let Some(deck) = item.deck {
        let mut unloaded = Vec::new();
        while ch.loaded_size(c) > deck.slots
            && let Some(last) = ch.loaded.pop()
        {
            unloaded.push(format!("`{}`", name(c, &last)));
        }
        if !unloaded.is_empty() {
            message.push_str(&format!(" · {} to inventory", unloaded.join(", ")));
        }
    }
    message
}

/// Where a copy sits in the pack.
fn pack_copy(ch: &Character, id: CopyId) -> Result<usize, String> {
    ch.inventory
        .iter()
        .position(|s| s.copy_id() == Some(id))
        .ok_or_else(|| "not in your pack".to_string())
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
            let Some(s) = take_from_pack(ch, &id, 1).pop() else {
                return Err(format!("{} is not in your pack", item.name));
            };
            done(equip_stack(ch, c, item, slot, s))
        }
        Action::EquipCopy(copy) => {
            let i = pack_copy(ch, copy)?;
            let id = &ch.inventory[i].item;
            let item = c.item(id).ok_or_else(|| format!("unknown item {id}"))?;
            let slot = item
                .slot
                .ok_or_else(|| format!("{} has no slot", item.name))?;
            let s = ch.inventory.remove(i);
            done(equip_stack(ch, c, item, slot, s))
        }
        Action::Unequip(slot) => {
            let s = ch
                .take_off(slot)
                .ok_or_else(|| format!("nothing in the {} slot", slot.label()))?;
            let message = format!("unequipped `{}`", name(c, &s.item));
            ch.stow(s);
            clamp_hp(ch, c);
            done(message)
        }
        Action::Drop { item, qty } => {
            let taken = take_from_pack(ch, &item, qty);
            if taken.is_empty() {
                return Err(format!("no {} to drop", name(c, &item)));
            }
            let n: u32 = taken.iter().map(|s| s.qty).sum();
            for s in taken {
                ground.put(at.place, at.pos, s);
            }
            done(format!("dropped {n}× `{}`", name(c, &item)))
        }
        Action::DropCopy(copy) => {
            let i = pack_copy(ch, copy)?;
            let s = ch.inventory.remove(i);
            let message = format!("dropped 1× `{}`", name(c, &s.item));
            ground.put(at.place, at.pos, s);
            done(message)
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
            let n = ground.take(at.place, at.pos, &item, qty);
            if n == 0 {
                return Err(format!("no {} here", name(c, &item)));
            }
            ch.give(item.clone(), n);
            done(format!("took {n}× `{}`", name(c, &item)))
        }
        Action::TakeCopy(copy) => {
            if ch.overburdened(c) {
                return Err(over_capacity(ch, c));
            }
            let s = ground
                .take_copy(at.place, at.pos, copy)
                .ok_or_else(|| "not here".to_string())?;
            let message = format!("took 1× `{}`", name(c, &s.item));
            ch.stow(s);
            done(message)
        }
        Action::TakeAll => {
            if ch.overburdened(c) {
                return Err(over_capacity(ch, c));
            }
            let stacks = ground.take_all(at.place, at.pos);
            if stacks.is_empty() {
                return Err("nothing here".to_string());
            }
            let n: u32 = stacks.iter().map(|s| s.qty).sum();
            for s in stacks {
                ch.stow(s);
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
    use crate::character::{CopyId, Stack};

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
        ch.inventory.push(Stack::new(id("hardhat"), 1));
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Equip(id("hardhat"))).unwrap();
        assert_eq!(out.message, "equipped `Hardhat with visor`");
        assert_eq!(ch.equipped(Slot::Head), Some(&id("hardhat")));
        assert_eq!(ch.count(&id("hardhat")), 0);
        assert!(!ch.inventory.iter().any(|s| s.qty == 0));
        ch.inventory.push(Stack::new(id("mirrorshades"), 1));
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
    fn equipping_a_smaller_deck_unloads_what_no_longer_fits() {
        let (c, _, mut g, at) = setup();
        let mut ch = Character::debug_runner(&c);
        assert_eq!(ch.loaded.len(), 4);
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Equip(id("pawnshop_deck")),
        )
        .unwrap();
        assert_eq!(ch.loaded, vec![id("crack"), id("copy")]);
        assert_eq!(
            out.message,
            "swapped `Rebuilt pawn deck` for `Pawnshop deck` · `Backdoor`, `Attack` to inventory"
        );
        assert!(ch.loaded_size(&c) <= 2);
    }

    #[test]
    fn equipping_a_deck_that_fits_keeps_what_is_loaded() {
        let (c, mut ch, mut g, at) = setup();
        let loaded = ch.loaded.clone();
        ch.give(id("rebuilt_deck"), 1);
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Equip(id("rebuilt_deck")),
        )
        .unwrap();
        assert_eq!(ch.loaded, loaded);
        assert_eq!(
            out.message,
            "swapped `Pawnshop deck` for `Rebuilt pawn deck`"
        );
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
        ch.inventory.push(Stack::new(id("stim_patch"), 1));
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
        ch.inventory.push(Stack::new(id("scrap_servo"), 40));
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

    /// Gives a rebuilt deck copy with `piece` fitted in `socket`; returns the
    /// deck's copy and the piece's.
    fn modded_deck(
        c: &Content,
        ch: &mut Character,
        piece: &str,
        socket: usize,
    ) -> (CopyId, CopyId) {
        ch.give_item(c, id("rebuilt_deck"), 1);
        let fitted = ch.new_copy(c, &id(piece));
        let piece_id = fitted.id;
        let deck = ch
            .inventory
            .iter_mut()
            .rev()
            .find(|s| s.item == id("rebuilt_deck"))
            .unwrap();
        let copy = deck.copy.as_mut().unwrap();
        copy.fitted[socket] = Some(Stack {
            item: id(piece),
            qty: 1,
            copy: Some(fitted),
        });
        (copy.id, piece_id)
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn wearing_a_copy_keeps_its_state() {
        let (c, mut ch, mut g, at) = setup();
        let (weight, value) = (ch.weight(&c), ch.value(&c));
        let (deck, piece) = modded_deck(&c, &mut ch, "coldfin_2", 0);
        let held = ch.copy_in_pack(deck).unwrap().clone();
        assert!(near(held.weight(&c), 1.8 + 0.6), "{}", held.weight(&c));
        assert_eq!(held.value(&c), 6000 + 1600);
        assert!(near(ch.weight(&c), weight + 1.8 + 0.6));
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::EquipCopy(deck)).unwrap();
        assert_eq!(
            out.message,
            "swapped `Pawnshop deck` for `Rebuilt pawn deck`"
        );
        let worn = ch.worn_copy(Slot::Deck).unwrap();
        assert_eq!(worn.id, deck);
        assert_eq!(
            worn.fitted[0].as_ref().and_then(|s| s.copy_id()),
            Some(piece)
        );
        assert!(ch.is_worn_copy(deck));
        assert_eq!(ch.count(&id("rebuilt_deck")), 0);
        assert!(ch.copy_in_pack(deck).is_none());
        assert_eq!(ch.count(&id("pawnshop_deck")), 1, "the old deck comes back");
        assert!(
            near(ch.weight(&c), weight + 1.8 + 0.6),
            "worn, the piece still weighs: {}",
            ch.weight(&c)
        );
        assert_eq!(ch.value(&c), value + 6000 + 1600);
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Unequip(Slot::Deck)).unwrap();
        assert_eq!(out.message, "unequipped `Rebuilt pawn deck`");
        assert!(ch.worn_copy(Slot::Deck).is_none() && !ch.is_worn_copy(deck));
        assert_eq!(ch.copy_in_pack(deck), Some(&held), "back with its piece");
        assert!(near(ch.weight(&c), weight + 1.8 + 0.6));
    }

    #[test]
    fn equip_by_id_takes_the_first_copy() {
        let (c, mut ch, mut g, at) = setup();
        ch.give_item(&c, id("rebuilt_deck"), 2);
        let ids: Vec<CopyId> = ch
            .inventory
            .iter()
            .filter(|s| s.item == id("rebuilt_deck"))
            .filter_map(|s| s.copy_id())
            .collect();
        assert_eq!(ids.len(), 2);
        let equip = || Action::Equip(id("rebuilt_deck"));
        apply(&mut ch, &mut g, &c, at, 0, equip()).unwrap();
        assert_eq!(ch.worn_copy(Slot::Deck).map(|k| k.id), Some(ids[0]));
        assert!(ch.copy_in_pack(ids[0]).is_none() && ch.copy_in_pack(ids[1]).is_some());
        assert_eq!(ch.count(&id("pawnshop_deck")), 1);
        let out = apply(&mut ch, &mut g, &c, at, 0, equip()).unwrap();
        assert_eq!(
            out.message,
            "swapped `Rebuilt pawn deck` for `Rebuilt pawn deck`"
        );
        assert_eq!(ch.worn_copy(Slot::Deck).map(|k| k.id), Some(ids[1]));
        let back = ch.copy_in_pack(ids[0]).expect("the old worn deck returns");
        assert_eq!((back.qty, back.item.clone()), (1, id("rebuilt_deck")));
        assert_eq!(ch.count(&id("rebuilt_deck")), 1);
        // Back to the plain deck: no copy is left worn.
        apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Equip(id("pawnshop_deck")),
        )
        .unwrap();
        assert!(ch.worn_copy(Slot::Deck).is_none());
        assert!(ch.copy_in_pack(ids[1]).is_some());
        // A copy without a slot is refused before anything moves.
        let piece = ch.new_copy(&c, &id("sidecar"));
        ch.inventory.push(Stack {
            item: id("sidecar"),
            qty: 1,
            copy: Some(piece.clone()),
        });
        let before = ch.clone();
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, Action::EquipCopy(piece.id)).unwrap_err(),
            "Sidecar co-processor has no slot"
        );
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, Action::EquipCopy(CopyId(999))).unwrap_err(),
            "not in your pack"
        );
        assert_eq!(ch, before);
    }

    #[test]
    fn drop_and_take_keep_copies() {
        let (c, mut ch, mut g, at) = setup();
        let (deck, _) = modded_deck(&c, &mut ch, "coldfin_1", 1);
        let held = ch.copy_in_pack(deck).unwrap().clone();
        let before = ch.clone();
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, Action::DropCopy(CopyId(999))).unwrap_err(),
            "not in your pack"
        );
        assert_eq!(ch, before);
        assert!(g.at(at.place, at.pos).is_empty());
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::DropCopy(deck)).unwrap();
        assert_eq!(out.message, "dropped 1× `Rebuilt pawn deck`");
        assert!(ch.copy_in_pack(deck).is_none());
        assert_eq!(
            g.at(at.place, at.pos),
            std::slice::from_ref(&held),
            "with its piece"
        );
        let take = Action::Take {
            item: id("rebuilt_deck"),
            qty: 1,
        };
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, take).unwrap_err(),
            "no Rebuilt pawn deck here"
        );
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::TakeAll).unwrap();
        assert_eq!(out.message, "took 1 items");
        assert_eq!(ch.copy_in_pack(deck), Some(&held), "intact");
        assert!(g.at(at.place, at.pos).is_empty());
        apply(&mut ch, &mut g, &c, at, 0, Action::DropCopy(deck)).unwrap();
        assert!(
            apply(&mut ch, &mut g, &c, at, 0, Action::TakeCopy(CopyId(999))).is_err(),
            "no such copy here"
        );
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::TakeCopy(deck)).unwrap();
        assert_eq!(out.message, "took 1× `Rebuilt pawn deck`");
        assert_eq!(ch.copy_in_pack(deck), Some(&held));
        assert!(g.at(at.place, at.pos).is_empty());
        // Dropped by id, each copy lies as its own stack.
        ch.give_item(&c, id("rebuilt_deck"), 1);
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Drop {
                item: id("rebuilt_deck"),
                qty: 5,
            },
        )
        .unwrap();
        assert_eq!(out.message, "dropped 2× `Rebuilt pawn deck`");
        let pile = g.at(at.place, at.pos);
        assert_eq!(pile.len(), 2);
        assert_eq!(pile[0], held, "pack order");
        assert!(pile.iter().all(|s| s.qty == 1 && s.copy.is_some()));
    }

    #[test]
    fn take_from_pack_splits_plain_stacks_and_unloads() {
        let (_, mut ch, _, _) = setup();
        let out = take_from_pack(&mut ch, &id("copper_coil"), 2);
        assert_eq!(out, vec![Stack::new(id("copper_coil"), 2)]);
        assert_eq!(ch.count(&id("copper_coil")), 1);
        assert!(take_from_pack(&mut ch, &id("hardhat"), 1).is_empty());
        assert_eq!(remove_from_pack(&mut ch, &id("copper_coil"), 5), 1);
        assert_eq!(ch.count(&id("copper_coil")), 0);
        assert!(ch.loaded.contains(&id("crack")));
        let out = take_from_pack(&mut ch, &id("crack"), 1);
        assert_eq!(out, vec![Stack::new(id("crack"), 1)]);
        assert!(
            !ch.loaded.contains(&id("crack")),
            "the last copy leaves the deck"
        );
    }
}
