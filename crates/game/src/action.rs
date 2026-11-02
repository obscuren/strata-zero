//! Item actions: everything a menu can do, applied by one function.

use crate::build::Build;
use crate::character::{Character, CopyId, Stack, over_budget};
use crate::content::Content;
use crate::ground::Ground;
use crate::model::*;
use citygen::{Place, Pos};

/// What a piece is fitted into: what a slot wears, the chrome a site holds, or a
/// copy in the pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Worn(Slot),
    Site(Site),
    Pack(CopyId),
}

/// Everything a menu can ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Wears the first of an item in pack order, a copy when it is one.
    Equip(ItemId),
    /// Wears one copy from the pack.
    EquipCopy(CopyId),
    Unequip(Slot),
    /// Installs the first of a piece of chrome in pack order at a site.
    Install {
        item: ItemId,
        site: Site,
    },
    /// Installs one copy of chrome from the pack at a site, with what is fitted in it.
    InstallCopy {
        copy: CopyId,
        site: Site,
    },
    /// Pulls the chrome out of a site, back to the pack with what is fitted in it.
    Pull(Site),
    /// Opens one of an item into what it holds.
    Open(ItemId),
    Drop {
        item: ItemId,
        qty: u32,
    },
    /// Drops one copy from the pack, with what is fitted in it.
    DropCopy(CopyId),
    Use(ItemId),
    /// Spends one thrown item; the throw itself is the app's to play.
    Throw(ItemId),
    /// Takes from the stacks without a copy; copies are taken by id.
    Take {
        item: ItemId,
        qty: u32,
    },
    /// Takes one copy from the ground, with what is fitted in it.
    TakeCopy(CopyId),
    TakeAll,
    Fire(AbilityId),
    /// Spends attribute points on an attribute, at creation's price.
    BuyAttribute(Attribute),
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
    /// Fits a piece of hardware from the pack into a socket.
    Fit {
        host: Host,
        socket: usize,
        piece: CopyId,
    },
    /// Takes the piece out of a socket, back to the pack.
    Unfit {
        host: Host,
        socket: usize,
    },
    /// Builds a recipe at the bench: its inputs leave the pack, its output arrives.
    Build(RecipeId),
    /// Sticks one of a sticker from the pack on the host's drawing, at its top-left
    /// cell as turned. Where it may go is the drawing's business, not this.
    Stick {
        host: Host,
        sticker: ItemId,
        x: u16,
        y: u16,
        turn: Turn,
    },
    /// Peels a stuck sticker off the host; it tears.
    Peel {
        host: Host,
        index: usize,
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

/// A socket's name: the item's drawing's own where it has one, else its kind's
/// (`hand`, `forearm`), else its number.
pub fn socket_name(c: &Content, item: &ItemId, socket: usize) -> String {
    let Some(item) = c.item(item) else {
        return (socket + 1).to_string();
    };
    match (item.drawn_sockets().get(socket), item.sockets.get(socket)) {
        (Some(&(name, _)), _) => name.to_string(),
        (None, Some(kind)) => kind.label().to_string(),
        (None, None) => (socket + 1).to_string(),
    }
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
            ch.stow_item(c, old);
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

/// Why chrome can't be worn.
const CHROME_FROM_ATTRIBUTES: &str = "chrome is installed from ATTRIBUTES";

/// Installs a stack taken from the pack at a site, checked by `can_install`; what the
/// site held goes to the pack, with its copy and what is fitted in it.
fn install_stack(ch: &mut Character, c: &Content, site: Site, s: Stack) -> String {
    let mut message = format!("installed `{}` · {}", name(c, &s.item), site.label());
    if let Some(old) = ch.install(site, s.item, s.copy) {
        message.push_str(&format!(" · `{}` to the pack", name(c, &old.item)));
        ch.stow_item(c, old);
    }
    // What left may have held max hp.
    clamp_hp(ch, c);
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
            if item.implant.is_some() {
                return Err(CHROME_FROM_ATTRIBUTES.to_string());
            }
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
            if item.implant.is_some() {
                return Err(CHROME_FROM_ATTRIBUTES.to_string());
            }
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
            ch.stow_item(c, s);
            clamp_hp(ch, c);
            done(message)
        }
        Action::Install { item, site } => {
            ch.can_install(c, &item, site)?;
            let Some(s) = take_from_pack(ch, &item, 1).pop() else {
                return Err(format!("{} is not in your pack", name(c, &item)));
            };
            done(install_stack(ch, c, site, s))
        }
        Action::InstallCopy { copy, site } => {
            let i = pack_copy(ch, copy)?;
            ch.can_install_stack(c, &ch.inventory[i], site)?;
            let s = ch.inventory.remove(i);
            done(install_stack(ch, c, site, s))
        }
        Action::Pull(site) => {
            let (item, copy) = (ch.chrome_at(site).cloned(), ch.chrome_copy(site).cloned());
            let item = item.ok_or_else(|| format!("nothing installed at {}", site.label()))?;
            ch.chrome.remove(&site);
            ch.chrome_copies.remove(&site);
            let message = format!("pulled `{}` · to the pack", name(c, &item));
            ch.stow_item(c, Stack { item, qty: 1, copy });
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
            if item.throw().is_some() {
                return Err("throw it from the map".to_string());
            }
            remove_from_pack(ch, &id, 1);
            match on_use {
                Use::Heal(n) => {
                    let healed = ch.hp_max(c).saturating_sub(ch.hp).min(n);
                    ch.hp += healed;
                    done(format!("used `{}` · +{healed} hp", item.name))
                }
                Use::Throw { .. } => unreachable!("refused above"),
            }
        }
        Action::Open(id) => {
            let item = c.item(&id).ok_or_else(|| format!("unknown item {id}"))?;
            if ch.count(&id) == 0 {
                return Err(format!("{} is not in your pack", item.name));
            }
            if item.opens.is_empty() {
                return Err(format!("{} doesn't open", item.name));
            }
            remove_from_pack(ch, &id, 1);
            let mut parts = Vec::new();
            for (out, n) in &item.opens {
                ch.give_item(c, out.clone(), *n);
                let out_name = name(c, out);
                parts.push(if *n > 1 {
                    format!("{n}× {out_name}")
                } else {
                    out_name
                });
            }
            done(format!("You open the {}: {}.", item.name, parts.join(", ")))
        }
        Action::Throw(id) => {
            let item = c.item(&id).ok_or_else(|| format!("unknown item {id}"))?;
            if item.throw().is_none() {
                return Err(format!("{} is not thrown", item.name));
            }
            if ch.count(&id) == 0 {
                return Err("none left".to_string());
            }
            remove_from_pack(ch, &id, 1);
            done(format!("threw `{}`", item.name))
        }
        Action::Take { item, qty } => {
            if ch.overburdened(c) {
                return Err(over_capacity(ch, c));
            }
            let n = ground.take(at.place, at.pos, &item, qty);
            if n == 0 {
                return Err(format!("no {} here", name(c, &item)));
            }
            ch.give_item(c, item.clone(), n);
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
                ch.stow_item(c, s);
            }
            done(format!("took {n} items"))
        }
        Action::BuyAttribute(attribute) => {
            let mut b = Build::from_character(c, ch);
            b.buy_attribute(c, attribute).map_err(|e| e.to_string())?;
            ch.attributes = b.attributes.clone();
            ch.attribute_points = b.points_left(c).max(0) as u32;
            done(format!(
                "{} {}",
                attribute.name(),
                ch.attributes.get(attribute)
            ))
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
        Action::Fit {
            host,
            socket,
            piece,
        } => {
            // Every refusal comes before the first change.
            let host_item = ch
                .host_item(host)
                .and_then(|h| c.item(h))
                .ok_or_else(|| "no such socket".to_string())?;
            let kind = *host_item
                .sockets
                .get(socket)
                .ok_or_else(|| "no such socket".to_string())?;
            let i = pack_copy(ch, piece)?;
            let piece_item = c
                .item(&ch.inventory[i].item)
                .ok_or_else(|| "not in your pack".to_string())?;
            let Some(fits) = piece_item.fits_socket() else {
                return Err(format!("`{}` is not hardware", piece_item.name));
            };
            // Socket chrome takes load; hardware and bots take none.
            let load = piece_item.implant.map_or(0, |i| u32::from(i.load));
            if fits != kind {
                return Err(format!(
                    "`{}` does not fit a {}",
                    piece_item.name,
                    kind.fits()
                ));
            }
            if matches!(host, Host::Site(_)) {
                // What the socket holds now leaves as the piece goes in.
                let out = ch
                    .host_copy(host)
                    .and_then(|k| k.fitted.get(socket)?.as_ref())
                    .and_then(|s| c.item(&s.item)?.implant)
                    .map_or(0, |i| i.load as u32);
                let after = ch.chrome_load(c) - out + load;
                if after > ch.load_max() {
                    return Err(over_budget(after, ch.load_max()));
                }
            }
            let sockets = host_item.sockets.len();
            let host_id = host_item.id.clone();
            // The host copy is found before the piece leaves the pack, and a piece is
            // never its own host.
            if host == Host::Pack(piece) || ch.host_copy_mut(c, host).is_none() {
                return Err("no such socket".to_string());
            }
            let stack = ch.inventory.remove(i);
            let copy = ch
                .host_copy_mut(c, host)
                .ok_or_else(|| "no such socket".to_string())?;
            // A copy made before its item gained sockets still has room for them.
            if copy.fitted.len() < sockets {
                copy.fitted.resize(sockets, None);
            }
            let old = copy.fitted[socket].replace(stack);
            let mut message = format!(
                "`{}` fitted · socket {}",
                piece_item.name,
                socket_name(c, &host_id, socket)
            );
            if let Some(old) = old {
                message.push_str(&format!(" · `{}` to the pack", name(c, &old.item)));
                ch.stow(old);
            }
            // A piece can carry any perk: what left may have held max hp.
            clamp_hp(ch, c);
            done(message)
        }
        Action::Build(id) => {
            // Every refusal comes before the first change.
            let r = c
                .recipe(&id)
                .ok_or_else(|| format!("unknown recipe {id}"))?;
            if !ch.has_workbench(c) {
                return Err("no workbench".to_string());
            }
            if !ch.has_skill(&r.skill) {
                let skill = c
                    .skill(&r.skill)
                    .map_or_else(|| r.skill.to_string(), |s| s.name.clone());
                return Err(format!("needs {skill}"));
            }
            if let Some((item, need)) = r.inputs.iter().find(|(i, n)| ch.count(i) < *n) {
                return Err(format!(
                    "needs {} more {}",
                    need - ch.count(item),
                    name(c, item)
                ));
            }
            for (item, n) in &r.inputs {
                take_from_pack(ch, item, *n);
            }
            let (out, n) = &r.output;
            ch.give_item(c, out.clone(), *n);
            if *n == 1 {
                done(format!("built `{}`", name(c, out)))
            } else {
                done(format!("built {n}× `{}`", name(c, out)))
            }
        }
        Action::Unfit { host, socket } => {
            let host_id = ch
                .host_item(host)
                .cloned()
                .ok_or_else(|| "no such socket".to_string())?;
            let sockets = c.item(&host_id).map_or(0, |i| i.sockets.len());
            if socket >= sockets {
                return Err("no such socket".to_string());
            }
            let label = socket_name(c, &host_id, socket);
            let filled = ch
                .host_copy(host)
                .is_some_and(|k| k.fitted.get(socket).is_some_and(Option::is_some));
            if !filled {
                return Err(format!("socket {label} is empty"));
            }
            let out = ch
                .host_copy_mut(c, host)
                .and_then(|k| k.fitted[socket].take())
                .ok_or_else(|| format!("socket {label} is empty"))?;
            let message = format!("`{}` out · to the pack", name(c, &out.item));
            ch.stow(out);
            clamp_hp(ch, c);
            done(message)
        }
        Action::Stick {
            host,
            sticker,
            x,
            y,
            turn,
        } => {
            // Every refusal comes before the first change.
            let takes = ch
                .host_item(host)
                .and_then(|h| c.item(h))
                .is_some_and(Item::takes_stickers);
            if !takes {
                return Err("takes no stickers".to_string());
            }
            let item = c
                .item(&sticker)
                .ok_or_else(|| format!("unknown item {sticker}"))?;
            if item.kind != ItemKind::Sticker {
                return Err(format!("`{}` is not a sticker", item.name));
            }
            let loose = ch
                .inventory
                .iter()
                .any(|s| s.item == sticker && s.copy.is_none() && s.qty > 0);
            if !loose {
                return Err("not in your pack".to_string());
            }
            if ch.host_copy_mut(c, host).is_none() {
                return Err("takes no stickers".to_string());
            }
            remove_from_pack(ch, &sticker, 1);
            let message = format!("`{}` stuck", item.name);
            let copy = ch
                .host_copy_mut(c, host)
                .ok_or_else(|| "takes no stickers".to_string())?;
            copy.stickers.push(Stuck {
                sticker,
                x,
                y,
                turn,
            });
            done(message)
        }
        Action::Peel { host, index } => {
            let in_range = ch.host_copy(host).is_some_and(|k| index < k.stickers.len());
            if !in_range {
                return Err("no such sticker".to_string());
            }
            let copy = ch
                .host_copy_mut(c, host)
                .ok_or_else(|| "no such sticker".to_string())?;
            let torn = copy.stickers.remove(index);
            done(format!("`{}` peeled · it tore", name(c, &torn.sticker)))
        }
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
                Active::Ping { .. }
                | Active::Trace { .. }
                | Active::Strike { .. }
                | Active::Boost { .. }
                | Active::Mark { .. } => Vec::new(),
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
    use crate::character::{CopyId, ItemCopy, Stack};

    #[test]
    fn sockets_are_named_by_their_drawing() {
        let c = Content::load().unwrap();
        assert_eq!(socket_name(&c, &ItemId::new("ratchet_smg"), 1), "RAIL");
        assert_eq!(socket_name(&c, &ItemId::new("rebuilt_deck"), 0), "L");
        // A host without a drawing names its sockets by their kind.
        assert_eq!(socket_name(&c, &ItemId::new("piston_arm"), 0), "hand");
        assert_eq!(socket_name(&c, &ItemId::new("piston_arm"), 1), "forearm");
        // No socket there at all: its number.
        assert_eq!(socket_name(&c, &ItemId::new("salvage_jacket"), 0), "1");
    }

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

    fn build(c: &Content, ch: &mut Character, recipe: &str) -> Result<Outcome, String> {
        let at = At {
            place: Place::Stratum(citygen::StratumKind::Street),
            pos: Pos::new(5, 5),
        };
        apply(
            ch,
            &mut Ground::default(),
            c,
            at,
            0,
            Action::Build(RecipeId::new(recipe)),
        )
    }

    #[test]
    fn build_spends_the_inputs_and_gives_the_output() {
        let (c, mut ch, _, _) = setup();
        ch.inventory.retain(|s| s.item != id("copper_coil"));
        ch.give(id("copper_coil"), 1);
        let out = build(&c, &mut ch, "wire_shiv").unwrap();
        assert_eq!(
            out.message,
            format!("built `{}`", name(&c, &id("shiv_blade")))
        );
        assert_eq!(ch.count(&id("copper_coil")), 0);
        assert_eq!(ch.count(&id("scrap_servo")), 1);
        assert_eq!(ch.count(&id("shiv_blade")), 1);
    }

    #[test]
    fn build_of_a_batch_names_its_count() {
        let (c, mut ch, _, _) = setup();
        ch.skills.insert(SkillId::new("medtech"));
        ch.give(id("copper_coil"), 1);
        let out = build(&c, &mut ch, "stim_batch").unwrap();
        assert_eq!(out.message, "built 2× `Stim patch`");
    }

    #[test]
    fn build_refuses_before_changing_anything() {
        let (c, mut ch, _, _) = setup();
        assert_eq!(
            build(&c, &mut ch, "ratchet_build").unwrap_err(),
            "needs Scrapcraft"
        );
        ch.skills.insert(SkillId::new("scrapcraft"));
        for part in [
            "ratchet_receiver",
            "ratchet_stock",
            "ratchet_mag",
            "ratchet_trigger",
        ] {
            ch.give_item(&c, id(part), 1);
        }
        let before = ch.clone();
        assert_eq!(
            build(&c, &mut ch, "ratchet_build").unwrap_err(),
            "needs 1 more Barrel 9×19"
        );
        assert_eq!(ch, before);
        assert_eq!(
            build(&c, &mut ch, "nope").unwrap_err(),
            "unknown recipe nope"
        );
        let mut kessler = Character::from_template(&c, &TemplateId::new("kessler")).unwrap();
        assert_eq!(
            build(&c, &mut kessler, "wire_shiv").unwrap_err(),
            "no workbench"
        );
    }

    #[test]
    fn build_of_an_item_with_sockets_makes_an_empty_copy() {
        let (c, mut ch, _, _) = setup();
        ch.skills.insert(SkillId::new("scrapcraft"));
        let parts = [
            "ratchet_receiver",
            "ratchet_stock",
            "ratchet_barrel",
            "ratchet_mag",
            "ratchet_trigger",
        ];
        for part in parts {
            ch.give_item(&c, id(part), 1);
        }
        let had = ch.count(&id("ratchet_smg"));
        let out = build(&c, &mut ch, "ratchet_build").unwrap();
        assert_eq!(out.message, "built `Ratchet SMG`");
        assert_eq!(ch.count(&id("ratchet_smg")), had + 1);
        let sockets = c.item(&id("ratchet_smg")).unwrap().sockets.len();
        let copy = ch
            .inventory
            .iter()
            .rev()
            .find(|s| s.item == id("ratchet_smg"))
            .and_then(|s| s.copy.as_ref())
            .expect("a copy");
        assert_eq!(copy.fitted.len(), sockets);
        assert!(sockets > 0 && copy.fitted.iter().all(Option::is_none));
        for part in parts {
            assert_eq!(ch.count(&id(part)), 0, "{part}");
        }
    }

    #[test]
    fn equip_wears_and_unequip_returns() {
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
        assert_eq!(out.message, "equipped `Mirrorshades`");
        assert_eq!(ch.equipped(Slot::Eyes), Some(&id("mirrorshades")));
        assert_eq!(ch.sight_bonus(&c), 4, "the optics are installed, not worn");
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

    fn box_content() -> Content {
        let mut c = Content::load().unwrap();
        let mut b = c.item(&id("copper_coil")).unwrap().clone();
        b.id = id("box");
        b.name = "Box".to_string();
        b.opens = vec![(id("bolt"), 2), (id("nut"), 1)];
        let mut bolt = b.clone();
        (bolt.id, bolt.name, bolt.opens) = (id("bolt"), "Bolt".to_string(), Vec::new());
        let mut nut = bolt.clone();
        (nut.id, nut.name) = (id("nut"), "Nut".to_string());
        c.items.extend([b, bolt, nut]);
        c
    }

    #[test]
    fn opening_takes_one_and_gives_what_it_holds() {
        let (_, mut ch, mut g, at) = setup();
        let c = box_content();
        ch.give(id("box"), 1);
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Open(id("box"))).unwrap();
        assert_eq!(out.message, "You open the Box: 2× Bolt, Nut.");
        assert_eq!(
            (
                ch.count(&id("box")),
                ch.count(&id("bolt")),
                ch.count(&id("nut"))
            ),
            (0, 2, 1)
        );
        assert!(apply(&mut ch, &mut g, &c, at, 0, Action::Open(id("box"))).is_err());
    }

    #[test]
    fn an_item_that_does_not_open_is_refused() {
        let (c, mut ch, mut g, at) = setup();
        let before = ch.count(&id("copper_coil"));
        ch.give(id("copper_coil"), 1);
        let held = ch.count(&id("copper_coil"));
        assert!(held > before);
        let err = apply(&mut ch, &mut g, &c, at, 0, Action::Open(id("copper_coil"))).unwrap_err();
        assert!(err.contains("doesn't open"), "{err}");
        assert_eq!(ch.count(&id("copper_coil")), held);
    }

    #[test]
    fn the_shipped_package_opens_into_the_shard() {
        let c = Content::load().unwrap();
        assert_eq!(
            c.item(&id("taped_package")).unwrap().opens,
            vec![(id("ledger_shard"), 1)]
        );
        let s = c.item(&id("ledger_shard")).unwrap();
        assert_eq!((s.kind, s.value), (ItemKind::Component, 0));
        assert_eq!(s.desc, "A ledger shard. Locked. Somebody will know whose.");
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
        ch.chrome.insert(Site::Neural, id("neural_port"));
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
            apply(
                &mut ch,
                &mut g,
                &c,
                at,
                0,
                Action::BuyAttribute(Attribute::Body)
            )
            .is_err(),
            "no points"
        );
        ch.attribute_points = 3;
        let out = apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::BuyAttribute(Attribute::Body),
        )
        .unwrap();
        assert_eq!((ch.attributes.body, ch.attribute_points), (5, 0));
        assert_eq!(out.message, "BODY 5");
        ch.sp = 200;
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
        ch.sp = 100;
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
        let first = c.tree(Attribute::Net).unwrap().slot(0, 0).unwrap().clone();
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
        // Back to the pawnshop deck: it is worn with its own copy, stickers and all.
        apply(
            &mut ch,
            &mut g,
            &c,
            at,
            0,
            Action::Equip(id("pawnshop_deck")),
        )
        .unwrap();
        let worn = ch.worn_copy(Slot::Deck).map(|k| k.id);
        assert!(worn.is_some() && worn != Some(ids[0]) && worn != Some(ids[1]));
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

    fn runner() -> (Content, Character, Ground, At) {
        let (c, _, g, at) = setup();
        (c.clone(), Character::debug_runner(&c), g, at)
    }

    fn pack_id(ch: &Character, item: &str) -> CopyId {
        ch.inventory
            .iter()
            .find(|s| s.item == id(item))
            .and_then(Stack::copy_id)
            .unwrap()
    }

    fn fit(socket: usize, piece: CopyId) -> Action {
        Action::Fit {
            host: Host::Worn(Slot::Deck),
            socket,
            piece,
        }
    }

    #[test]
    fn fit_moves_the_piece_into_the_socket() {
        let (c, mut ch, mut g, at) = runner();
        let piece = pack_id(&ch, "coldfin_2");
        let out = apply(&mut ch, &mut g, &c, at, 0, fit(0, piece)).unwrap();
        assert_eq!(out.message, "`Coldfin vII` fitted · socket L");
        let held = ch.worn_copy(Slot::Deck).unwrap().fitted[0]
            .as_ref()
            .unwrap();
        assert_eq!(held.copy_id(), Some(piece));
        assert_eq!(ch.count(&id("coldfin_2")), 0, "fitted is not in the pack");
    }

    #[test]
    fn fitting_a_filled_socket_swaps() {
        let (c, mut ch, mut g, at) = runner();
        let (two, one) = (pack_id(&ch, "coldfin_2"), pack_id(&ch, "coldfin_1"));
        apply(&mut ch, &mut g, &c, at, 0, fit(0, two)).unwrap();
        let out = apply(&mut ch, &mut g, &c, at, 0, fit(0, one)).unwrap();
        assert_eq!(
            out.message,
            "`Coldfin vI` fitted · socket L · `Coldfin vII` to the pack"
        );
        assert_eq!(ch.count(&id("coldfin_2")), 1);
        assert_eq!(pack_id(&ch, "coldfin_2"), two, "its own copy came back");
    }

    #[test]
    fn unfit_returns_the_piece() {
        let (c, mut ch, mut g, at) = runner();
        let one = pack_id(&ch, "coldfin_1");
        apply(&mut ch, &mut g, &c, at, 0, fit(0, one)).unwrap();
        let unfit = Action::Unfit {
            host: Host::Worn(Slot::Deck),
            socket: 0,
        };
        let out = apply(&mut ch, &mut g, &c, at, 0, unfit.clone()).unwrap();
        assert_eq!(out.message, "`Coldfin vI` out · to the pack");
        assert!(ch.worn_copy(Slot::Deck).unwrap().fitted[0].is_none());
        assert_eq!(pack_id(&ch, "coldfin_1"), one);
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, unfit).unwrap_err(),
            "socket L is empty"
        );
    }

    #[test]
    fn unfit_takes_a_piece_out_of_an_installed_arm() {
        let (c, mut ch, mut g, at) = setup();
        let (arm, hand) = (id("piston_arm"), id("arc_hand"));
        // Installed without a copy, the arm gets one on the way in.
        ch.chrome.insert(Site::LeftArm, arm.clone());
        let host = Host::Site(Site::LeftArm);
        assert_eq!(ch.host_item(host), Some(&arm));
        assert!(ch.host_copy(host).is_none());
        let piece = Stack {
            item: hand.clone(),
            qty: 1,
            copy: Some(ch.new_copy(&c, &hand)),
        };
        ch.host_copy_mut(&c, host).unwrap().fitted[0] = Some(piece);
        let fitted = (Site::LeftArm, Some(0), hand.clone());
        assert!(ch.installed().contains(&fitted));
        let unfit = Action::Unfit { host, socket: 0 };
        let out = apply(&mut ch, &mut g, &c, at, 0, unfit).unwrap();
        assert_eq!(out.message, "`Arc hand` out · to the pack");
        assert_eq!(ch.count(&hand), 1);
        assert!(!ch.installed().contains(&fitted));
    }

    #[test]
    fn hp_follows_socket_chrome_out() {
        let (mut c, mut ch, mut g, at) = setup();
        let (arm, hand) = (id("piston_arm"), id("arc_hand"));
        // A hand that holds blood.
        let item = c.items.iter_mut().find(|i| i.id == hand).unwrap();
        item.perks.push(Perk::HpMax(20));
        ch.chrome.insert(Site::LeftArm, arm);
        let host = Host::Site(Site::LeftArm);
        let piece = Stack {
            item: hand.clone(),
            qty: 1,
            copy: Some(ch.new_copy(&c, &hand)),
        };
        ch.host_copy_mut(&c, host).unwrap().fitted[0] = Some(piece);
        let bare = ch.hp_max(&c) - 20;
        ch.hp = ch.hp_max(&c);
        act(&mut ch, &mut g, &c, at, Action::Unfit { host, socket: 0 }).unwrap();
        assert_eq!(
            (ch.hp, ch.hp_max(&c)),
            (bare, bare),
            "out, the hp it held goes"
        );
        // Swapped out for a piece without it, the same.
        let back = Action::Fit {
            host,
            socket: 0,
            piece: pack_id(&ch, "arc_hand"),
        };
        act(&mut ch, &mut g, &c, at, back).unwrap();
        ch.hp = ch.hp_max(&c);
        assert_eq!(ch.hp, bare + 20);
        ch.give_item(&c, id("toolfingers"), 1);
        let swap = Action::Fit {
            host,
            socket: 0,
            piece: pack_id(&ch, "toolfingers"),
        };
        act(&mut ch, &mut g, &c, at, swap).unwrap();
        assert_eq!(
            (ch.hp, ch.hp_max(&c)),
            (bare, bare),
            "swapped out, the same"
        );
    }

    #[test]
    fn fit_refusals_change_nothing() {
        let (c, mut ch, mut g, at) = runner();
        let one = pack_id(&ch, "coldfin_1");
        ch.give_item(&c, id("rebuilt_deck"), 1);
        let deck = pack_id(&ch, "rebuilt_deck");
        ch.wear(Slot::Torso, id("salvage_jacket"), None);
        let jacket = Action::Fit {
            host: Host::Worn(Slot::Torso),
            socket: 0,
            piece: one,
        };
        for action in [
            fit(2, one),
            fit(0, CopyId(999)),
            fit(0, deck),
            jacket,
            Action::Fit {
                host: Host::Worn(Slot::Legs),
                socket: 0,
                piece: one,
            },
            Action::Fit {
                host: Host::Pack(CopyId(999)),
                socket: 0,
                piece: one,
            },
            Action::Unfit {
                host: Host::Worn(Slot::Torso),
                socket: 0,
            },
        ] {
            let before = ch.clone();
            assert!(
                apply(&mut ch, &mut g, &c, at, 0, action.clone()).is_err(),
                "{action:?}"
            );
            assert_eq!(ch, before, "{action:?}");
        }
    }

    #[test]
    fn fit_refusal_messages() {
        let (c, mut ch, mut g, at) = runner();
        ch.give_item(&c, id("rebuilt_deck"), 1);
        let deck = pack_id(&ch, "rebuilt_deck");
        let refused =
            |ch: &mut Character, g: &mut Ground, a| apply(ch, g, &c, at, 0, a).unwrap_err();
        assert_eq!(refused(&mut ch, &mut g, fit(2, deck)), "no such socket");
        assert_eq!(
            refused(&mut ch, &mut g, fit(0, deck)),
            "`Rebuilt pawn deck` is not hardware"
        );
    }

    #[test]
    fn fit_into_a_carried_deck() {
        let (c, mut ch, mut g, at) = runner();
        ch.give_item(&c, id("rebuilt_deck"), 1);
        let deck = pack_id(&ch, "rebuilt_deck");
        let piece = pack_id(&ch, "coldfin_1");
        let action = Action::Fit {
            host: Host::Pack(deck),
            socket: 1,
            piece,
        };
        let out = apply(&mut ch, &mut g, &c, at, 0, action).unwrap();
        assert_eq!(out.message, "`Coldfin vI` fitted · socket R");
        let held = ch.copy_in_pack(deck).unwrap().copy.as_ref().unwrap();
        assert_eq!(
            held.fitted[1].as_ref().and_then(Stack::copy_id),
            Some(piece)
        );
    }

    #[test]
    fn fitting_the_gun_checks_the_socket_kind() {
        let (c, mut ch, mut g, at) = setup();
        ch.give_item(&c, id("hushcan"), 1);
        ch.give_item(&c, id("redeye"), 1);
        let hushcan = pack_id(&ch, "hushcan");
        let redeye = pack_id(&ch, "redeye");
        let on_gun = |socket, piece| Action::Fit {
            host: Host::Worn(Slot::Primary),
            socket,
            piece,
        };
        for (socket, piece, message) in [
            (1, hushcan, "does not fit a rail socket"),
            (0, redeye, "does not fit a muzzle socket"),
        ] {
            let before = ch.clone();
            let err = apply(&mut ch, &mut g, &c, at, 0, on_gun(socket, piece)).unwrap_err();
            assert!(err.contains(message), "{err}");
            assert_eq!(ch, before);
        }
    }

    #[test]
    fn fit_into_a_carried_gun() {
        let (c, mut ch, mut g, at) = setup();
        ch.give_item(&c, id("ratchet_smg"), 1);
        ch.give_item(&c, id("hushcan"), 1);
        let gun = pack_id(&ch, "ratchet_smg");
        let piece = pack_id(&ch, "hushcan");
        let action = Action::Fit {
            host: Host::Pack(gun),
            socket: 0,
            piece,
        };
        apply(&mut ch, &mut g, &c, at, 0, action).unwrap();
        let held = ch.copy_in_pack(gun).unwrap().copy.as_ref().unwrap();
        assert_eq!(
            held.fitted[0].as_ref().and_then(Stack::copy_id),
            Some(piece)
        );
        assert_eq!(ch.count(&id("hushcan")), 0, "fitted is not in the pack");
    }

    #[test]
    fn a_deck_worn_without_a_copy_comes_back_as_one() {
        let (c, mut ch, mut g, at) = setup();
        ch.equipment.insert(Slot::Deck, id("rebuilt_deck"));
        ch.worn.clear();
        apply(&mut ch, &mut g, &c, at, 0, Action::Unequip(Slot::Deck)).unwrap();
        let back = pack_id(&ch, "rebuilt_deck");
        let held = ch.copy_in_pack(back).unwrap().copy.as_ref().unwrap();
        assert_eq!(held.fitted, vec![None, None]);
    }

    #[test]
    fn a_deck_worn_without_a_copy_takes_hardware() {
        let (c, mut ch, mut g, at) = setup();
        ch.equipment.insert(Slot::Deck, id("rebuilt_deck"));
        ch.worn.clear();
        ch.give_item(&c, id("coldfin_1"), 1);
        let piece = pack_id(&ch, "coldfin_1");
        apply(&mut ch, &mut g, &c, at, 0, fit(1, piece)).unwrap();
        let worn = ch.worn_copy(Slot::Deck).unwrap();
        assert_eq!(worn.fitted.len(), 2);
        assert_eq!(
            worn.fitted[1].as_ref().and_then(Stack::copy_id),
            Some(piece)
        );
    }

    #[test]
    fn a_stale_worn_entry_is_no_worn_copy() {
        let (_, mut ch, _, _) = runner();
        let worn = ch.worn_copy(Slot::Deck).unwrap().id;
        assert!(ch.is_worn_copy(worn));
        ch.equipment.remove(&Slot::Deck);
        assert!(ch.worn_copy(Slot::Deck).is_none());
        assert!(!ch.is_worn_copy(worn));
    }

    #[test]
    fn taking_a_copyless_stack_of_a_per_copy_item_makes_copies() {
        let (c, mut ch, mut g, at) = setup();
        g.add(at.place, at.pos, id("coldfin_1"), 1);
        let take = Action::Take {
            item: id("coldfin_1"),
            qty: 1,
        };
        apply(&mut ch, &mut g, &c, at, 0, take).unwrap();
        assert!(
            ch.inventory
                .iter()
                .any(|s| s.item == id("coldfin_1") && s.copy.is_some())
        );
        g.add(at.place, at.pos, id("coldfin_2"), 2);
        apply(&mut ch, &mut g, &c, at, 0, Action::TakeAll).unwrap();
        let copies = ch
            .inventory
            .iter()
            .filter(|s| s.item == id("coldfin_2"))
            .map(|s| (s.qty, s.copy.is_some()))
            .collect::<Vec<_>>();
        assert_eq!(copies, vec![(1, true), (1, true)]);
    }

    fn stick(host: Host, sticker: &str, x: u16, y: u16, turn: Turn) -> Action {
        Action::Stick {
            host,
            sticker: id(sticker),
            x,
            y,
            turn,
        }
    }

    #[test]
    fn stick_takes_one_from_the_pack_onto_the_copy() {
        let (c, mut ch, mut g, at) = runner();
        let before = ch.count(&id("sticker_neko"));
        let action = stick(
            Host::Worn(Slot::Deck),
            "sticker_neko",
            54,
            12,
            Turn::Quarter,
        );
        let out = apply(&mut ch, &mut g, &c, at, 0, action).unwrap();
        assert_eq!(out.message, "`Neko` stuck");
        assert_eq!(ch.count(&id("sticker_neko")), before - 1);
        let last = ch.worn_copy(Slot::Deck).unwrap().stickers.last().cloned();
        assert_eq!(
            last,
            Some(Stuck {
                sticker: id("sticker_neko"),
                x: 54,
                y: 12,
                turn: Turn::Quarter
            })
        );
    }

    #[test]
    fn stick_refuses_and_changes_nothing() {
        let (c, mut ch, mut g, at) = runner();
        let hushcan = pack_id(&ch, "hushcan");
        remove_from_pack(&mut ch, &id("sticker_flatline"), 99);
        let deck = Host::Worn(Slot::Deck);
        for (action, why) in [
            (
                stick(Host::Pack(hushcan), "sticker_neko", 0, 0, Turn::Level),
                "takes no stickers",
            ),
            (
                stick(deck, "crack", 50, 10, Turn::Level),
                "`Crack` is not a sticker",
            ),
            (
                stick(deck, "nope", 50, 10, Turn::Level),
                "unknown item nope",
            ),
            (
                stick(deck, "sticker_flatline", 50, 10, Turn::Level),
                "not in your pack",
            ),
        ] {
            let before = ch.clone();
            assert_eq!(
                apply(&mut ch, &mut g, &c, at, 0, action),
                Err(why.to_string())
            );
            assert_eq!(ch, before, "{why}");
        }
    }

    #[test]
    fn peel_destroys_the_sticker() {
        let (c, mut ch, mut g, at) = runner();
        let n = ch.count(&id("sticker_no_future"));
        let deck = Host::Worn(Slot::Deck);
        let peel = |index| Action::Peel { host: deck, index };
        let out = apply(&mut ch, &mut g, &c, at, 0, peel(0)).unwrap();
        assert_eq!(out.message, "`No Future` peeled · it tore");
        let left: Vec<ItemId> = ch
            .worn_copy(Slot::Deck)
            .unwrap()
            .stickers
            .iter()
            .map(|s| s.sticker.clone())
            .collect();
        assert_eq!(left, [id("sticker_dead_face")]);
        assert_eq!(
            ch.count(&id("sticker_no_future")),
            n,
            "not back in the pack"
        );
        let before = ch.clone();
        let err = apply(&mut ch, &mut g, &c, at, 0, peel(5));
        assert_eq!(err, Err("no such sticker".to_string()));
        assert_eq!(ch, before);
    }

    #[test]
    fn stickers_follow_their_copy() {
        let (c, mut ch, mut g, at) = runner();
        let pawn = pack_id(&ch, "pawnshop_deck");
        let action = stick(Host::Pack(pawn), "sticker_oni", 54, 9, Turn::Level);
        apply(&mut ch, &mut g, &c, at, 0, action).unwrap();
        let oni = |k: &ItemCopy| k.stickers.last().map(|s| s.sticker.clone());
        apply(&mut ch, &mut g, &c, at, 0, Action::EquipCopy(pawn)).unwrap();
        assert_eq!(ch.worn_copy(Slot::Deck).map(|k| k.id), Some(pawn));
        assert_eq!(
            ch.worn_copy(Slot::Deck).and_then(oni),
            Some(id("sticker_oni"))
        );
        apply(&mut ch, &mut g, &c, at, 0, Action::Unequip(Slot::Deck)).unwrap();
        let back = ch.copy_in_pack(pawn).and_then(|s| s.copy.as_ref());
        assert_eq!(back.and_then(oni), Some(id("sticker_oni")));
    }

    fn frag() -> Item {
        ron::from_str(
            r#"Item(id: "frag", name: "Frag", kind: Consumable, weight: 0.4, value: 1,
            use: Some(Throw(range: 8, arc: Lob, blast: Blast(radius: 2, damage: 12, stun: 0))),
            fx: [Throw, Bloom], desc: "")"#,
        )
        .unwrap()
    }

    #[test]
    fn throw_consumes_one_and_use_refuses_a_throwable() {
        let (mut c, mut ch, mut g, at) = setup();
        c.items.push(frag());
        ch.give_item(&c, id("frag"), 2);
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Throw(id("frag"))).unwrap();
        assert_eq!(out.message, "threw `Frag`");
        assert!(out.effects.is_empty());
        assert_eq!(ch.count(&id("frag")), 1);
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Use(id("frag"))).unwrap_err(),
            "throw it from the map"
        );
        assert_eq!(ch.count(&id("frag")), 1);
        ch.give_item(&c, id("stim_patch"), 1);
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Throw(id("stim_patch"))).unwrap_err(),
            "Stim patch is not thrown"
        );
        apply(&mut ch, &mut g, &c, at, 0, Action::Throw(id("frag"))).unwrap();
        assert_eq!(
            apply(&mut ch, &mut g, &c, at, 0, Action::Throw(id("frag"))).unwrap_err(),
            "none left"
        );
    }

    #[test]
    fn fire_books_boost_and_mark_cooldowns() {
        let (mut c, mut ch, mut g, at) = setup();
        for (name, kind) in [
            ("redline", "Active(Boost(once: true))"),
            ("deadeye", "Active(Mark(range: 12, bonus: 20, ap: 1))"),
        ] {
            c.abilities.push(
                ron::from_str(&format!(
                    r#"Ability(id: "{name}", name: "X", cooldown: 60, kind: {kind}, desc: "")"#
                ))
                .unwrap(),
            );
            let a = AbilityId::new(name);
            ch.abilities.push(a.clone());
            let out = apply(&mut ch, &mut g, &c, at, 10, Action::Fire(a.clone())).unwrap();
            assert!(out.effects.is_empty());
            assert_eq!(ch.fired.get(&a), Some(&10));
        }
    }

    fn act(
        ch: &mut Character,
        g: &mut Ground,
        c: &Content,
        at: At,
        a: Action,
    ) -> Result<Outcome, String> {
        apply(ch, g, c, at, 0, a)
    }

    #[test]
    fn install_and_pull() {
        let (c, mut ch, mut g, at) = setup();
        ch.give_item(&c, id("grip_arm"), 1);
        let copy = pack_id(&ch, "grip_arm");
        let packed = ch.weight(&c);
        let a = Action::Install {
            item: id("grip_arm"),
            site: Site::RightArm,
        };
        let out = act(&mut ch, &mut g, &c, at, a).unwrap();
        assert_eq!(out.message, "installed `Grip arm` · R ARM");
        assert!(
            near(ch.weight(&c), packed - 4.0),
            "installed, the arm weighs nothing: {}",
            ch.weight(&c)
        );
        assert_eq!(ch.chrome_at(Site::RightArm), Some(&id("grip_arm")));
        assert_eq!(ch.count(&id("grip_arm")), 0);
        assert_eq!(ch.chrome_copy(Site::RightArm).map(|k| k.id), Some(copy));
        let out = act(&mut ch, &mut g, &c, at, Action::Pull(Site::RightArm)).unwrap();
        assert_eq!(out.message, "pulled `Grip arm` · to the pack");
        assert_eq!(ch.chrome_at(Site::RightArm), None);
        assert_eq!(pack_id(&ch, "grip_arm"), copy);
        assert!(
            near(ch.weight(&c), packed),
            "back in the pack, it weighs again"
        );
        assert_eq!(
            act(&mut ch, &mut g, &c, at, Action::Pull(Site::RightArm)).unwrap_err(),
            "nothing installed at R ARM"
        );
    }

    #[test]
    fn install_refusals_change_nothing() {
        let (c, mut ch, mut g, at) = setup();
        for item in [
            "grip_arm",
            "neural_port",
            "hardhat",
            "arc_hand",
            "target_lens",
        ] {
            ch.give_item(&c, id(item), 1);
        }
        let install = |item: &str, site| Action::Install {
            item: id(item),
            site,
        };
        let mut refuses = |ch: &mut Character, a: Action, why: &str| {
            let before = ch.clone();
            assert_eq!(act(ch, &mut g, &c, at, a).unwrap_err(), why);
            assert_eq!(*ch, before);
        };
        refuses(
            &mut ch,
            install("piston_arm", Site::LeftArm),
            "`Piston arm` is not in your pack",
        );
        refuses(
            &mut ch,
            install("hardhat", Site::Eyes),
            "`Hardhat with visor` is not chrome",
        );
        refuses(
            &mut ch,
            install("neural_port", Site::Eyes),
            "`Neural port` doesn't fit EYES",
        );
        refuses(
            &mut ch,
            install("arc_hand", Site::RightArm),
            "`Arc hand` doesn't fit R ARM",
        );
        // A budget of 6 with a piston arm and the rest installed: no room for another arm.
        ch.chrome.insert(Site::RightArm, id("piston_arm"));
        ch.chrome.insert(Site::Neural, id("neural_port"));
        ch.attributes.body = 0;
        ch.attributes.cool = 0;
        assert_eq!((ch.chrome_load(&c), ch.load_max()), (6, 6));
        refuses(
            &mut ch,
            install("grip_arm", Site::LeftArm),
            "too much chrome: load 9 of 6",
        );
        let copy = pack_id(&ch, "grip_arm");
        refuses(
            &mut ch,
            Action::InstallCopy {
                copy: CopyId(copy.0 + 99),
                site: Site::RightArm,
            },
            "not in your pack",
        );
    }

    #[test]
    fn install_on_a_held_site_swaps() {
        let (c, mut ch, mut g, at) = setup();
        // A piece held without a copy: the optics go back to the pack.
        ch.give_item(&c, id("target_lens"), 1);
        let a = Action::Install {
            item: id("target_lens"),
            site: Site::Eyes,
        };
        let out = act(&mut ch, &mut g, &c, at, a).unwrap();
        assert_eq!(
            out.message,
            "installed `Target lens` · EYES · `Nightglass optics` to the pack"
        );
        assert_eq!(ch.chrome_at(Site::Eyes), Some(&id("target_lens")));
        assert_eq!(ch.count(&id("nightglass_optics")), 1);
        assert_eq!(ch.count(&id("target_lens")), 0);
        // A limb goes to the pack with its copy and what is fitted in it.
        ch.give_item(&c, id("piston_arm"), 1);
        ch.give_item(&c, id("arc_hand"), 1);
        ch.give_item(&c, id("grip_arm"), 1);
        let (piston, hand, grip) = (
            pack_id(&ch, "piston_arm"),
            pack_id(&ch, "arc_hand"),
            pack_id(&ch, "grip_arm"),
        );
        let site = |copy| Action::InstallCopy {
            copy,
            site: Site::LeftArm,
        };
        act(&mut ch, &mut g, &c, at, site(piston)).unwrap();
        let in_hand = Action::Fit {
            host: Host::Site(Site::LeftArm),
            socket: 0,
            piece: hand,
        };
        act(&mut ch, &mut g, &c, at, in_hand).unwrap();
        let out = act(&mut ch, &mut g, &c, at, site(grip)).unwrap();
        assert_eq!(
            out.message,
            "installed `Grip arm` · L ARM · `Piston arm` to the pack"
        );
        assert_eq!(ch.chrome_copy(Site::LeftArm).map(|k| k.id), Some(grip));
        let arm = ch
            .copy_in_pack(piston)
            .expect("the Piston arm, its copy kept");
        let fitted = arm.copy.as_ref().unwrap().fitted[0].as_ref().unwrap();
        assert_eq!(
            (fitted.item.clone(), fitted.copy_id()),
            (id("arc_hand"), Some(hand))
        );
        assert!(!ch.installed().iter().any(|(_, _, i)| *i == id("arc_hand")));
        // And back: the arm comes in again with its hand.
        act(&mut ch, &mut g, &c, at, site(piston)).unwrap();
        assert!(
            ch.installed()
                .contains(&(Site::LeftArm, Some(0), id("arc_hand")))
        );
        assert!(ch.copy_in_pack(grip).is_some());
    }

    #[test]
    fn a_swap_counts_what_leaves_against_the_budget() {
        let (c, mut ch, mut g, at) = setup();
        // A budget of 6, full: optics 2, a piston arm 3, the neural port 1.
        ch.chrome.insert(Site::RightArm, id("piston_arm"));
        ch.chrome.insert(Site::Neural, id("neural_port"));
        ch.attributes.body = 0;
        ch.attributes.cool = 0;
        assert_eq!((ch.chrome_load(&c), ch.load_max()), (6, 6));
        ch.give_item(&c, id("wrecker_arm"), 1);
        ch.give_item(&c, id("grip_arm"), 1);
        let (wrecker, grip) = (id("wrecker_arm"), id("grip_arm"));
        // The Wrecker arm's 4 for the Piston arm's 3: one over, and nothing moves.
        let why = "too much chrome: load 7 of 6";
        assert_eq!(
            ch.can_install(&c, &wrecker, Site::RightArm),
            Err(why.to_string())
        );
        let before = ch.clone();
        let a = Action::Install {
            item: wrecker,
            site: Site::RightArm,
        };
        assert_eq!(act(&mut ch, &mut g, &c, at, a).unwrap_err(), why);
        assert_eq!(ch, before);
        // The Grip arm's 3 fits only because the Piston arm's 3 leaves.
        assert_eq!(ch.can_install(&c, &grip, Site::RightArm), Ok(()));
        let copy = pack_id(&ch, "grip_arm");
        let a = Action::InstallCopy {
            copy,
            site: Site::RightArm,
        };
        act(&mut ch, &mut g, &c, at, a).unwrap();
        assert_eq!(ch.chrome_at(Site::RightArm), Some(&grip));
        assert_eq!(ch.chrome_load(&c), 6);
        assert_eq!(ch.count(&id("piston_arm")), 1);
    }

    #[test]
    fn a_swap_counts_what_is_fitted_on_both_sides() {
        let (c, mut ch, mut g, at) = setup();
        ch.attributes.body = 0;
        ch.attributes.cool = 0;
        ch.give_item(&c, id("piston_arm"), 1);
        ch.give_item(&c, id("arc_hand"), 1);
        let (piston, hand) = (pack_id(&ch, "piston_arm"), pack_id(&ch, "arc_hand"));
        let at_arm = |copy| Action::InstallCopy {
            copy,
            site: Site::LeftArm,
        };
        act(&mut ch, &mut g, &c, at, at_arm(piston)).unwrap();
        let in_hand = Action::Fit {
            host: Host::Site(Site::LeftArm),
            socket: 0,
            piece: hand,
        };
        act(&mut ch, &mut g, &c, at, in_hand).unwrap();
        // Optics 2 and the arm 3 with its hand 1: 6 of 6.
        assert_eq!((ch.chrome_load(&c), ch.load_max()), (6, 6));
        // A bare Wrecker arm (4) for the arm and its hand (4) fits.
        ch.give_item(&c, id("wrecker_arm"), 1);
        let wrecker = id("wrecker_arm");
        assert_eq!(ch.can_install(&c, &wrecker, Site::LeftArm), Ok(()));
        let a = Action::Install {
            item: wrecker.clone(),
            site: Site::LeftArm,
        };
        act(&mut ch, &mut g, &c, at, a).unwrap();
        assert_eq!(ch.chrome_load(&c), 6);
        // The Piston arm and its hand (4) back for the bare Wrecker arm (4): fits too;
        // with a Grip arm carrying a hand (4) it would too, but the Grip arm
        // carrying two pieces (6) would not.
        assert_eq!(
            ch.can_install_stack(&c, ch.copy_in_pack(piston).unwrap(), Site::LeftArm),
            Ok(())
        );
        let mut heavy = ch.copy_in_pack(piston).unwrap().clone();
        let spool = ch.new_copy(&c, &id("monowire_spool"));
        heavy.copy.as_mut().unwrap().fitted[1] = Some(Stack {
            item: id("monowire_spool"),
            qty: 1,
            copy: Some(spool),
        });
        assert_eq!(
            ch.can_install_stack(&c, &heavy, Site::LeftArm),
            Err("too much chrome: load 8 of 6".to_string())
        );
    }

    #[test]
    fn a_swap_clamps_hp() {
        let (c, mut ch, mut g, at) = setup();
        ch.give_item(&c, id("second_heart"), 1);
        ch.give_item(&c, id("filter_liver"), 1);
        let install = |item: &str| Action::Install {
            item: id(item),
            site: Site::Organs,
        };
        act(&mut ch, &mut g, &c, at, install("second_heart")).unwrap();
        ch.hp = ch.hp_max(&c);
        let full = ch.hp;
        let out = act(&mut ch, &mut g, &c, at, install("filter_liver")).unwrap();
        assert_eq!(
            out.message,
            "installed `Filter liver` · ORGANS · `Second heart` to the pack"
        );
        assert_eq!(ch.hp, ch.hp_max(&c));
        assert_eq!(ch.hp + 20, full);
    }

    #[test]
    fn can_install_is_what_install_refuses() {
        let (c, mut ch, _, _) = setup();
        ch.give_item(&c, id("adrenal_pump"), 1);
        let pump = id("adrenal_pump");
        assert_eq!(ch.can_install(&c, &pump, Site::Organs), Ok(()));
        assert!(ch.can_install(&c, &pump, Site::Jaw).is_err());
        assert!(ch.fits_at(&c, &pump, (Site::Organs, None)));
        assert!(!ch.fits_at(&c, &pump, (Site::Jaw, None)));
        assert!(!ch.fits_at(&c, &pump, (Site::Organs, Some(0))));
        ch.chrome.insert(Site::LeftArm, id("piston_arm"));
        assert!(ch.fits_at(&c, &id("arc_hand"), (Site::LeftArm, Some(0))));
        assert!(!ch.fits_at(&c, &id("arc_hand"), (Site::LeftArm, Some(1))));
        assert!(!ch.fits_at(&c, &id("arc_hand"), (Site::RightArm, Some(0))));
        assert!(!ch.fits_at(&c, &id("hardhat"), (Site::Jaw, None)));
    }

    #[test]
    fn a_pulled_limb_keeps_its_pieces() {
        let (c, mut ch, mut g, at) = setup();
        let (arm, hand) = (id("piston_arm"), id("arc_hand"));
        ch.give_item(&c, arm.clone(), 1);
        ch.give_item(&c, hand.clone(), 1);
        let arm_copy = pack_id(&ch, "piston_arm");
        let hand_copy = pack_id(&ch, "arc_hand");
        let at_arm = Action::InstallCopy {
            copy: arm_copy,
            site: Site::LeftArm,
        };
        act(&mut ch, &mut g, &c, at, at_arm.clone()).unwrap();
        let fit = Action::Fit {
            host: Host::Site(Site::LeftArm),
            socket: 0,
            piece: hand_copy,
        };
        act(&mut ch, &mut g, &c, at, fit).unwrap();
        let load = ch.chrome_load(&c);
        act(&mut ch, &mut g, &c, at, Action::Pull(Site::LeftArm)).unwrap();
        assert!(ch.chrome_load(&c) < load);
        act(&mut ch, &mut g, &c, at, at_arm).unwrap();
        assert!(ch.installed().contains(&(Site::LeftArm, Some(0), hand)));
        assert_eq!(ch.chrome_load(&c), load);
    }

    #[test]
    fn fit_takes_socket_chrome() {
        let (c, mut ch, mut g, at) = setup();
        ch.give_item(&c, id("piston_arm"), 1);
        ch.give_item(&c, id("arc_hand"), 1);
        let (arm, hand) = (pack_id(&ch, "piston_arm"), pack_id(&ch, "arc_hand"));
        let site = Action::InstallCopy {
            copy: arm,
            site: Site::LeftArm,
        };
        act(&mut ch, &mut g, &c, at, site).unwrap();
        let fit = |socket| Action::Fit {
            host: Host::Site(Site::LeftArm),
            socket,
            piece: hand,
        };
        let before = ch.clone();
        assert_eq!(
            act(&mut ch, &mut g, &c, at, fit(1)).unwrap_err(),
            "`Arc hand` does not fit a forearm socket"
        );
        assert_eq!(ch, before);
        // Over budget: arm 3 and neural 1 and eyes 2 fill a budget of 6.
        ch.attributes.cool = 0;
        ch.attributes.body = 0;
        ch.chrome.insert(Site::Neural, id("neural_port"));
        let before = ch.clone();
        assert_eq!((ch.chrome_load(&c), ch.load_max()), (6, 6));
        assert_eq!(
            act(&mut ch, &mut g, &c, at, fit(0)).unwrap_err(),
            "too much chrome: load 7 of 6"
        );
        assert_eq!(ch, before);
        ch.attributes.cool = 5;
        let out = act(&mut ch, &mut g, &c, at, fit(0)).unwrap();
        assert_eq!(out.message, "`Arc hand` fitted · socket hand");
        assert!(
            ch.installed()
                .contains(&(Site::LeftArm, Some(0), id("arc_hand")))
        );
        let unfit = Action::Unfit {
            host: Host::Site(Site::LeftArm),
            socket: 1,
        };
        assert_eq!(
            act(&mut ch, &mut g, &c, at, unfit).unwrap_err(),
            "socket forearm is empty"
        );
    }

    /// The Drone cradle installed at the back, a Scout and a Mule drone and an Arc hand
    /// in the pack.
    fn cradled() -> (Content, Character, Ground, At) {
        let (c, mut ch, mut g, at) = setup();
        for item in [
            "drone_cradle",
            "scout_drone",
            "mule_drone",
            "arc_hand",
            "piston_arm",
        ] {
            ch.give_item(&c, id(item), 1);
        }
        let cradle = pack_id(&ch, "drone_cradle");
        let install = Action::InstallCopy {
            copy: cradle,
            site: Site::Back,
        };
        act(&mut ch, &mut g, &c, at, install).unwrap();
        (c, ch, g, at)
    }

    fn dock(piece: CopyId) -> Action {
        Action::Fit {
            host: Host::Site(Site::Back),
            socket: 0,
            piece,
        }
    }

    #[test]
    fn a_drone_docks_and_swaps_in_its_bay() {
        let (c, mut ch, mut g, at) = cradled();
        let scout = pack_id(&ch, "scout_drone");
        let out = act(&mut ch, &mut g, &c, at, dock(scout)).unwrap();
        assert_eq!(out.message, "`Scout drone` fitted · socket bay");
        let mule = pack_id(&ch, "mule_drone");
        let out = act(&mut ch, &mut g, &c, at, dock(mule)).unwrap();
        assert_eq!(
            out.message,
            "`Mule drone` fitted · socket bay · `Scout drone` to the pack"
        );
        assert!(
            ch.installed()
                .contains(&(Site::Back, Some(0), id("mule_drone")))
        );
        assert_eq!(pack_id(&ch, "scout_drone"), scout);
    }

    #[test]
    fn bays_take_bots_and_nothing_else() {
        let (c, mut ch, mut g, at) = cradled();
        let before = ch.clone();
        let hand = pack_id(&ch, "arc_hand");
        assert_eq!(
            act(&mut ch, &mut g, &c, at, dock(hand)).unwrap_err(),
            "`Arc hand` does not fit a bay"
        );
        assert_eq!(ch, before);
        let arm = pack_id(&ch, "piston_arm");
        let install = Action::InstallCopy {
            copy: arm,
            site: Site::LeftArm,
        };
        act(&mut ch, &mut g, &c, at, install).unwrap();
        let scout = pack_id(&ch, "scout_drone");
        let in_the_hand = Action::Fit {
            host: Host::Site(Site::LeftArm),
            socket: 0,
            piece: scout,
        };
        assert_eq!(
            act(&mut ch, &mut g, &c, at, in_the_hand).unwrap_err(),
            "`Scout drone` does not fit a hand socket"
        );
    }

    #[test]
    fn a_cradle_from_an_old_save_takes_a_drone() {
        let (c, mut ch, mut g, at) = cradled();
        // A copy made before the cradle had a bay has no room for one.
        ch.chrome_copies
            .get_mut(&Site::Back)
            .unwrap()
            .fitted
            .clear();
        let scout = pack_id(&ch, "scout_drone");
        act(&mut ch, &mut g, &c, at, dock(scout)).unwrap();
        assert_eq!(ch.chrome_copy(Site::Back).unwrap().fitted.len(), 1);
        assert!(
            ch.installed()
                .contains(&(Site::Back, Some(0), id("scout_drone")))
        );
    }

    #[test]
    fn pulling_the_cradle_takes_its_drone_along() {
        let (c, mut ch, mut g, at) = cradled();
        let scout = pack_id(&ch, "scout_drone");
        act(&mut ch, &mut g, &c, at, dock(scout)).unwrap();
        act(&mut ch, &mut g, &c, at, Action::Pull(Site::Back)).unwrap();
        let cradle = ch
            .inventory
            .iter()
            .find(|s| s.item == id("drone_cradle"))
            .and_then(|s| s.copy.as_ref())
            .unwrap();
        assert_eq!(
            cradle.fitted[0].as_ref().map(|s| s.item.clone()),
            Some(id("scout_drone"))
        );
    }

    #[test]
    fn equip_refuses_chrome() {
        let (c, mut ch, mut g, at) = setup();
        ch.give_item(&c, id("grip_arm"), 1);
        let copy = pack_id(&ch, "grip_arm");
        for a in [
            Action::Equip(id("neural_port")),
            Action::Equip(id("grip_arm")),
            Action::EquipCopy(copy),
        ] {
            assert_eq!(
                act(&mut ch, &mut g, &c, at, a).unwrap_err(),
                "chrome is installed from ATTRIBUTES"
            );
        }
    }
}
