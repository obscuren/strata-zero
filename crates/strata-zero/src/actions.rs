//! Item actions on the app: `game::apply` plus its effects on the walker and the world.

use crate::app::{AppState, FxInstance};
use crate::clinic::Operating;
use crate::render::map::MapRef;
use citygen::{City, Place, Pos, StratumKind};
use game::{AbilityKind, Action, Active, At, Effect};

/// What an item action is refused with while jacked in.
pub const JACKED_IN: &str = "jacked in · gear and pack locked until you jack out";
/// What dropping an item is refused with off the cells the city owns.
pub const OUT_HERE: &str = "nowhere to leave it out here";

/// Jacked in: the body sits still and its gear and pack can't be touched; only the
/// Net acts.
pub fn jacked_in(st: &AppState) -> bool {
    st.player.as_ref().is_some_and(|p| p.jacked_in())
}

/// Whether `action` changes what is worn, carried or loaded.
pub fn handles_items(action: &Action) -> bool {
    matches!(
        action,
        Action::Equip(_)
            | Action::EquipCopy(_)
            | Action::Unequip(_)
            | Action::Install { .. }
            | Action::InstallCopy { .. }
            | Action::Pull(_)
            | Action::Drop { .. }
            | Action::DropCopy(_)
            | Action::Use(_)
            | Action::Open(_)
            | Action::Take { .. }
            | Action::TakeCopy(_)
            | Action::TakeAll
            | Action::Load(_)
            | Action::Unload(_)
            | Action::Slot { .. }
            | Action::Fit { .. }
            | Action::Unfit { .. }
            | Action::Build(_)
            | Action::Stick { .. }
            | Action::Peel { .. }
    )
}

/// A fee the doc won't take: `the doc's fee is ¤370 · you have ¤320`.
pub fn fee_line(fee: u32, credits: u32) -> String {
    format!("the doc's fee is ¤{fee} · you have ¤{credits}")
}

/// The piece a chrome action puts in, from the pack: at a site or in a socket.
fn put_in(st: &AppState, action: &Action) -> Option<game::ItemId> {
    match action {
        Action::Install { item, .. } => Some(item.clone()),
        Action::InstallCopy { copy, .. } | Action::Fit { piece: copy, .. } => {
            st.character.copy_in_pack(*copy).map(|s| s.item.clone())
        }
        _ => None,
    }
}

/// Applies an action at the player's feet and runs its effects on the world.
/// The message is for the status strip; a refusal is the reason.
pub fn perform(st: &mut AppState, action: Action) -> Result<String, String> {
    if handles_items(&action) && jacked_in(st) {
        return Err(JACKED_IN.to_string());
    }
    // Items lie only on a city's map: off the cells the city owns there's nothing to
    // leave one on.
    if matches!(action, Action::Drop { .. } | Action::DropCopy(_))
        && st.player.as_ref().is_some_and(|p| {
            p.place.stratum().is_some() && !crate::outside::stream::live_owns(st, p.pos)
        })
    {
        return Err(OUT_HERE.to_string());
    }
    // The story keeps some things shut until it says otherwise.
    if let Action::Open(id) = &action
        && !crate::missions::unsealed(st, id)
    {
        return Err("not yours to open".to_string());
    }
    // Chrome goes in and out in a ripperdoc's chair, for the doc's fee; free with dev
    // free chrome; a piece on someone's tab goes in on the tab, once, and free chrome
    // spends that tab too. Every refusal comes before any change.
    let mut charge = None;
    let mut tab = None;
    let mut spend = false;
    if let Some(fee) = game::surgery_fee(&st.character, &st.content, &action) {
        match crate::clinic::operating(st) {
            Operating::Free => {
                spend = put_in(st, &action)
                    .is_some_and(|item| crate::missions::free_tab_for(st, &item).is_some());
            }
            Operating::Locked => return Err(crate::clinic::OUTSIDE.to_string()),
            Operating::Gone { doc } => {
                st.chair = None;
                return Err(crate::clinic::gone_line(&doc));
            }
            Operating::Chair { .. }
                if put_in(st, &action)
                    .is_some_and(|item| crate::missions::tab_for(st, &item).is_some()) =>
            {
                tab = Some(fee);
                spend = true;
            }
            Operating::Chair { doc } => {
                if fee > st.character.credits {
                    return Err(fee_line(fee, st.character.credits));
                }
                charge = Some((fee, doc));
            }
        }
    }
    // Pack-only actions work anywhere; the ground and the world need a body.
    let needs_body = matches!(
        action,
        Action::Drop { .. }
            | Action::DropCopy(_)
            | Action::Take { .. }
            | Action::TakeCopy(_)
            | Action::TakeAll
            | Action::Fire(_)
    );
    let (place, pos) = match st.player.as_ref().map(|p| (p.place, p.pos)) {
        Some(body) => body,
        None if needs_body => return Err("no body in the city".to_string()),
        None => (Place::Stratum(StratumKind::Street), Pos::new(0, 0)),
    };
    let at = At { place, pos };
    let fired = match &action {
        Action::Fire(id) => Some(id.clone()),
        _ => None,
    };
    let refresh_sight = matches!(
        action,
        Action::Equip(_)
            | Action::EquipCopy(_)
            | Action::Unequip(_)
            | Action::Install { .. }
            | Action::InstallCopy { .. }
            | Action::Pull(_)
            | Action::Fit { .. }
            | Action::Unfit { .. }
    );
    if let Some(id) = &fired
        && st
            .character
            .ability(&st.content, id)
            .is_some_and(|a| matches!(a.kind, AbilityKind::Active(Active::Strike { .. })))
    {
        // A strike wants a target: the map's cursor fires it, not the slot alone.
        return Err("needs a target · aim at a foe".to_string());
    }
    if let Some(id) = &fired
        && st.character.ability(&st.content, id).is_some_and(|a| {
            matches!(
                a.kind,
                AbilityKind::Active(Active::Boost { .. } | Active::Mark { .. })
            )
        })
    {
        // A boost or a mark is cast from the map, where the cursor and the turn are.
        return Err("use it from the map".to_string());
    }
    if let Action::Use(id) = &action
        && st.content.item(id).is_some_and(|i| i.throw().is_some())
    {
        // A throwable is thrown, never used: refused before an action point goes.
        return Err("throw it from the map".to_string());
    }
    // Firing an ability, or using an item on turns, is an action point.
    if fired.is_some() || (matches!(action, Action::Use(_)) && st.net.engaged.is_some()) {
        crate::net::spend_ap(st)?;
    }
    let took = matches!(
        action,
        Action::Take { .. } | Action::TakeCopy(_) | Action::TakeAll
    );
    let outcome = game::apply(
        &mut st.character,
        &mut st.ground,
        &st.content,
        at,
        st.tick,
        action,
    )?;
    if took {
        let left = st
            .ground
            .at(place, pos)
            .iter()
            .any(|s| s.item == game::ItemId::new("paydata"));
        if !left {
            crate::jobs::send(st, crate::jobs::Event::Taken { place, pos });
        }
    }
    let minute = st.minute_of_day();
    let interiors = &st.interiors;
    let constructs = &st.constructs;
    if refresh_sight {
        let bonus = st.sight_bonus();
        if let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut())
            && let Some(map) = MapRef::for_place(city, interiors, constructs, player.place, minute)
        {
            player.update_fov(&map, bonus);
        }
    }
    let mut revealed = Vec::new();
    for effect in outcome.effects {
        match effect {
            Effect::Reveal { radius } => {
                if let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut())
                    && let Some(map) =
                        MapRef::for_place(city, interiors, constructs, player.place, minute)
                {
                    revealed = player.reveal(&map, radius);
                }
            }
        }
    }
    // The ability's effects start now, at the firer's feet.
    let mut message = outcome.message.clone();
    if let Some((fee, doc)) = charge.filter(|(fee, _)| *fee > 0) {
        st.character.credits -= fee;
        message = format!("{message} · ¤{fee} to {doc}");
    }
    if spend && let Some(t) = st.missions.tab.as_mut() {
        t.spent = true;
    }
    if let Some(fee) = tab {
        message = format!("{message} · ¤{fee} on the tab");
    }
    if let Some(id) = fired
        && let Some(ability) = st.character.ability(&st.content, &id)
    {
        let radius = match ability.kind {
            AbilityKind::Active(
                Active::Scan { radius } | Active::Ping { radius } | Active::Trace { radius },
            ) => radius,
            AbilityKind::Active(
                Active::Strike { .. } | Active::Boost { .. } | Active::Mark { .. },
            )
            | AbilityKind::Passive => 0,
        };
        let mut targets = match (&ability.kind, st.city.as_ref(), place.stratum()) {
            (AbilityKind::Active(Active::Trace { radius }), Some(city), Some(stratum)) => {
                trace_targets(city, stratum, pos, *radius)
            }
            _ => Vec::new(),
        };
        // Inside a construct the deck's pulses have teeth: a ping lights every node
        // and an arc trace stuns the ICE it reaches; both warm the trace.
        if let Place::Construct(b) = place {
            match ability.kind {
                AbilityKind::Active(Active::Ping { .. }) => {
                    targets = crate::net::construct_nodes(st, b);
                    message = format!("Ping: {} nodes answer", targets.len());
                    crate::net::noise_on(st, b, crate::net::PING_NOISE);
                }
                AbilityKind::Active(Active::Trace { radius }) => {
                    targets = crate::net::stun_pieces(st, b, pos, radius);
                    message = match targets.len() {
                        0 => "Arc Trace: nothing to arc to".to_string(),
                        1 => "Arc Trace: 1 piece stunned".to_string(),
                        n => format!("Arc Trace: {n} pieces stunned"),
                    };
                    crate::net::noise_on(st, b, crate::net::TRACE_NOISE);
                }
                _ => {}
            }
        }
        // A mark lights when the arc lands: it waits out any bolt fired with it.
        let landing = ability
            .fx
            .iter()
            .find_map(|f| match f {
                game::Fx::Bolt { ticks, .. } => Some(*ticks),
                _ => None,
            })
            .unwrap_or(0);
        for fx in &ability.fx {
            let started = match fx {
                game::Fx::Mark { .. } => st.tick + landing,
                _ => st.tick,
            };
            st.fx.push(FxInstance {
                fx: fx.clone(),
                place,
                origin: pos,
                radius,
                started,
                revealed: revealed.clone(),
                targets: targets.clone(),
                label: String::new(),
                hit: false,
                tint: None,
                arc: None,
            });
        }
    }
    Ok(message)
}

/// The node tiles a trace from `origin` reaches: cyber tiles on the Net, anchors
/// elsewhere, within `radius`.
pub fn trace_targets(city: &City, stratum: StratumKind, origin: Pos, radius: i32) -> Vec<Pos> {
    if stratum == StratumKind::Net {
        city.net
            .nodes
            .iter()
            .map(|n| n.cyber)
            .filter(|p| p.chebyshev(origin) <= radius)
            .collect()
    } else {
        city.net
            .nodes_on(stratum)
            .map(|n| n.anchor.1)
            .filter(|p| p.chebyshev(origin) <= radius)
            .collect()
    }
}

/// Puts a result where the player sees it: the status strip, warnings in magenta.
pub fn report(st: &mut AppState, result: Result<String, String>) {
    match result {
        Ok(msg) => st.say(msg),
        Err(msg) => st.warn(msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Visibility;
    use crate::walker::Player;
    use citygen::{City, GenParams, Generator, SizePreset};
    use game::{AbilityId, CopyId, Host, ItemId, RecipeId, Site, Slot};

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn chrome_is_locked_jacked_in() {
        let mut st = AppState::new();
        st.city = Some(city());
        let b = st.city.as_ref().unwrap().buildings[0].id;
        let mut p = Player::at(Place::Construct(b), Pos::new(1, 1));
        p.body = Some((Place::Stratum(StratumKind::Street), Pos::new(1, 1)));
        st.player = Some(p);
        st.character
            .give_item(&st.content, ItemId::new("grip_arm"), 1);
        let before = st.character.clone();
        for action in [
            Action::Install {
                item: ItemId::new("grip_arm"),
                site: Site::RightArm,
            },
            Action::InstallCopy {
                copy: CopyId(1),
                site: Site::RightArm,
            },
            Action::Pull(Site::Eyes),
        ] {
            let refused = perform(&mut st, action.clone());
            assert_eq!(refused, Err(JACKED_IN.to_string()), "{action:?}");
        }
        assert_eq!(st.character, before);
        // Back in the body, the arm goes in (free chrome: this tests the jack, not the
        // chair).
        st.free_chrome = true;
        st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
        let install = Action::Install {
            item: ItemId::new("grip_arm"),
            site: Site::RightArm,
        };
        perform(&mut st, install).unwrap();
        assert_eq!(
            st.character.chrome_at(Site::RightArm),
            Some(&ItemId::new("grip_arm"))
        );
    }

    #[test]
    fn opening_a_sealed_item_is_refused() {
        let mut st = AppState::new();
        st.character.give(ItemId::new("taped_package"), 1);
        let before = st.character.clone();
        let open = Action::Open(ItemId::new("taped_package"));
        assert_eq!(perform(&mut st, open), Err("not yours to open".to_string()));
        assert_eq!(st.character, before);
    }

    #[test]
    fn opening_gives_the_shard() {
        let mut st = AppState::new();
        st.character.give(ItemId::new("taped_package"), 1);
        crate::missions::effects::apply(
            &mut st,
            0,
            0,
            &crate::missions::Effect::Unseal("taped_package".into()),
        );
        let open = Action::Open(ItemId::new("taped_package"));
        assert_eq!(
            perform(&mut st, open),
            Ok("You open the Taped package: Ledger shard.".to_string())
        );
        assert_eq!(st.character.count(&ItemId::new("ledger_shard")), 1);
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 0);
    }

    fn grip(st: &mut AppState) -> Action {
        st.character
            .give_item(&st.content, ItemId::new("grip_arm"), 1);
        Action::Install {
            item: ItemId::new("grip_arm"),
            site: Site::RightArm,
        }
    }

    #[test]
    fn chrome_is_locked_outside_a_chair() {
        let (mut st, _, _, _) = crate::clinic::testutil::outside(crate::clinic::testutil::NIGHT);
        let install = grip(&mut st);
        st.character
            .install(Site::LeftArm, ItemId::new("piston_arm"), None);
        st.character
            .give_item(&st.content, ItemId::new("arc_hand"), 1);
        let hand = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == ItemId::new("arc_hand"))
            .and_then(|s| s.copy_id())
            .unwrap();
        let before = st.character.clone();
        for action in [
            install,
            Action::Pull(Site::LeftArm),
            Action::Fit {
                host: Host::Site(Site::LeftArm),
                socket: 0,
                piece: hand,
            },
            Action::Unfit {
                host: Host::Site(Site::LeftArm),
                socket: 0,
            },
        ] {
            assert_eq!(
                perform(&mut st, action.clone()),
                Err(crate::clinic::OUTSIDE.to_string()),
                "{action:?}"
            );
        }
        assert_eq!(st.character, before);
        // Fitting a deck or a gun is no surgery: whatever else refuses it, the chair doesn't.
        let deck = Action::Fit {
            host: Host::Worn(Slot::Deck),
            socket: 0,
            piece: hand,
        };
        assert_ne!(
            perform(&mut st, deck),
            Err(crate::clinic::OUTSIDE.to_string())
        );
    }

    #[test]
    fn the_chair_charges() {
        let mut st = crate::clinic::testutil::seated();
        st.character.credits = 1000;
        let install = grip(&mut st);
        let Operating::Chair { doc } = crate::clinic::operating(&st) else {
            panic!()
        };
        let msg = perform(&mut st, install).unwrap();
        assert_eq!(st.character.credits, 630);
        assert!(msg.ends_with(&format!(" · ¤370 to {doc}")), "{msg}");
        assert_eq!(
            st.character.chrome_at(Site::RightArm),
            Some(&ItemId::new("grip_arm"))
        );
    }

    #[test]
    fn short_is_refused_first() {
        let mut st = crate::clinic::testutil::seated();
        st.character.credits = 320;
        let install = grip(&mut st);
        let before = st.character.clone();
        assert_eq!(
            perform(&mut st, install),
            Err("the doc's fee is ¤370 · you have ¤320".to_string())
        );
        assert_eq!(st.character, before);
    }

    #[test]
    fn a_short_swap_is_refused() {
        let mut st = crate::clinic::testutil::seated();
        st.character
            .install(Site::RightArm, ItemId::new("piston_arm"), None);
        st.character.credits = 400;
        let swap = grip(&mut st);
        let before = st.character.clone();
        assert_eq!(perform(&mut st, swap), Err(fee_line(480, 400)));
        assert_eq!(st.character, before);
    }

    #[test]
    fn the_doc_gone_ends_the_chair() {
        let mut st = crate::clinic::testutil::seated();
        st.character
            .install(Site::Eyes, ItemId::new("target_lens"), None);
        st.player.as_mut().unwrap().place = Place::Stratum(StratumKind::Street);
        let Err(why) = perform(&mut st, Action::Pull(Site::Eyes)) else {
            panic!("pulled without a doc")
        };
        assert!(why.ends_with(" has gone for the night"), "{why}");
        assert_eq!(st.chair, None);
        assert!(st.character.chrome_at(Site::Eyes).is_some());
    }

    #[test]
    fn free_chrome_is_free() {
        let (mut st, _, _, _) = crate::clinic::testutil::outside(crate::clinic::testutil::DAY);
        st.free_chrome = true;
        st.character.credits = 5;
        let install = grip(&mut st);
        let msg = perform(&mut st, install).unwrap();
        assert_eq!(st.character.credits, 5);
        assert!(!msg.contains('¤'), "{msg}");
    }

    #[test]
    fn jacked_in_still_wins_in_the_chair() {
        let mut st = crate::clinic::testutil::seated();
        let install = grip(&mut st);
        let p = st.player.as_mut().unwrap();
        p.body = Some((p.place, p.pos));
        p.place = Place::Stratum(StratumKind::Net);
        assert_eq!(perform(&mut st, install), Err(JACKED_IN.to_string()));
    }

    #[test]
    fn jacked_in_the_gear_and_the_pack_stay_put() {
        let mut st = AppState::new();
        st.city = Some(city());
        let b = st.city.as_ref().unwrap().buildings[0].id;
        let mut p = Player::at(Place::Construct(b), Pos::new(1, 1));
        p.body = Some((Place::Stratum(StratumKind::Street), Pos::new(1, 1)));
        st.player = Some(p);
        st.character.give(ItemId::new("stim_patch"), 1);
        let before = st.character.clone();
        let loaded = st.character.loaded[0].clone();
        let stim = ItemId::new("stim_patch");
        for action in [
            Action::Equip(ItemId::new("copper_coil")),
            Action::Unequip(Slot::Deck),
            Action::Use(stim.clone()),
            Action::Drop {
                item: stim.clone(),
                qty: 1,
            },
            Action::Take {
                item: stim.clone(),
                qty: 1,
            },
            Action::TakeAll,
            Action::Load(ItemId::new("attack")),
            Action::Unload(loaded),
            Action::Slot {
                id: ItemId::new("attack"),
                at: 0,
            },
            Action::Build(RecipeId::new("wire_shiv")),
            Action::EquipCopy(CopyId(1)),
            Action::DropCopy(CopyId(1)),
            Action::TakeCopy(CopyId(1)),
            Action::Fit {
                host: Host::Worn(Slot::Deck),
                socket: 0,
                piece: CopyId(1),
            },
            Action::Unfit {
                host: Host::Worn(Slot::Deck),
                socket: 0,
            },
            Action::Stick {
                host: Host::Worn(Slot::Deck),
                sticker: ItemId::new("sticker_neko"),
                x: 54,
                y: 12,
                turn: game::Turn::Level,
            },
            Action::Peel {
                host: Host::Worn(Slot::Deck),
                index: 0,
            },
        ] {
            let refused = perform(&mut st, action.clone());
            assert_eq!(refused, Err(JACKED_IN.to_string()), "{action:?}");
        }
        assert_eq!(st.character, before);
        // Back in the body, its hands are its own again.
        st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
        perform(&mut st, Action::Unequip(Slot::Deck)).unwrap();
        assert_eq!(st.character.equipped(Slot::Deck), None);
    }

    #[test]
    fn a_stim_from_the_deck_costs_an_ap() {
        use citygen::Pos;
        let mut st = crate::foes::testutil::lot(Pos::new(14, 8));
        st.character.hp = 1;
        let stim = ItemId::new("stim_patch");
        crate::net::engage(&mut st, Place::Stratum(StratumKind::Street), true);
        assert_eq!(
            perform(&mut st, Action::Use(stim.clone())).unwrap_err(),
            "move first · space to stay"
        );
        assert!(crate::net::stay(&mut st));
        perform(&mut st, Action::Use(stim.clone())).unwrap();
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 1);
        assert_eq!(st.character.hp, 16);
        // Out of a fight it is free.
        crate::net::disengage(&mut st);
        st.character.give(stim.clone(), 1);
        perform(&mut st, Action::Use(stim)).unwrap();
    }

    #[test]
    fn perform_runs_effects_on_the_world() {
        let mut st = AppState::new();
        st.city = Some(city());
        assert_eq!(
            perform(&mut st, Action::Fire(AbilityId::new("netscan"))).unwrap_err(),
            "no body in the city"
        );
        st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
        st.tick = 100;
        let seen = |st: &AppState| {
            st.player
                .as_ref()
                .unwrap()
                .vis()
                .count(|v| *v != Visibility::Unseen)
        };
        let before = seen(&st);
        let msg = perform(&mut st, Action::Fire(AbilityId::new("netscan"))).unwrap();
        assert_eq!(msg, "Netscan: pulse sent");
        let after = seen(&st);
        assert!(after > before, "{after} vs {before}");
        let kinds: Vec<&game::Fx> = st.fx.iter().map(|i| &i.fx).collect();
        assert!(matches!(kinds[..], [game::Fx::Wave { .. }]), "{kinds:?}");
        assert!(
            !st.fx[0].revealed.is_empty(),
            "the wave hides what the scan revealed"
        );
        assert_eq!((st.fx[0].radius, st.fx[0].started), (12, st.tick));
        // Cooling: refused, and nothing plays.
        assert!(perform(&mut st, Action::Fire(AbilityId::new("netscan"))).is_err());
        assert_eq!(st.fx.len(), 1);
        st.character
            .chrome
            .insert(Site::Neural, ItemId::new("neural_port"));
        perform(&mut st, Action::Fire(AbilityId::new("ping"))).unwrap();
        let kinds: Vec<&game::Fx> = st.fx.iter().map(|i| &i.fx).collect();
        assert!(
            matches!(
                kinds[1..],
                [game::Fx::Wave { .. }, game::Fx::Glow { ticks: 180, .. }]
            ),
            "{kinds:?}"
        );
        assert_eq!(st.fx[2].until(), st.tick + 180);
        assert!(st.fx[1].revealed.is_empty() && st.fx[1].radius == 24);
        assert!(st.fx.iter().all(|i| i.targets.is_empty()));
        perform(&mut st, Action::Fire(AbilityId::new("arc_trace"))).unwrap();
        let kinds: Vec<&game::Fx> = st.fx.iter().map(|i| &i.fx).collect();
        assert!(
            matches!(kinds[3..], [game::Fx::Bolt { .. }, game::Fx::Mark { .. }]),
            "{kinds:?}"
        );
        assert!(
            !st.fx[3].targets.is_empty(),
            "nodes within 20 tiles of the spawn"
        );
        assert_eq!(st.fx[3].targets, st.fx[4].targets);
        assert_eq!(
            (st.fx[3].started, st.fx[4].started),
            (st.tick, st.tick + 36),
            "the marks light when the arc lands"
        );
        let (k, pos) = (st.fx[3].place.stratum().unwrap(), st.fx[3].origin);
        let city = st.city.as_ref().unwrap();
        assert!(
            st.fx[3]
                .targets
                .iter()
                .all(|t| t.chebyshev(pos) <= 20 && city.net.node_at(k, *t).is_some())
        );
        // On the Net the targets are the nodes' cyber tiles.
        let node = &city.net.nodes[0];
        let net = trace_targets(city, StratumKind::Net, node.cyber, 3);
        assert!(net.contains(&node.cyber));
        assert!(net.iter().all(|t| t.chebyshev(node.cyber) <= 3));
        report(&mut st, Err("nope".into()));
        assert_eq!(
            st.log.latest().map(|e| e.marked()),
            Some("! nope".to_string())
        );
        report(&mut st, Ok("fine".into()));
        assert_eq!(
            st.log.latest().map(|e| e.marked()),
            Some("fine".to_string())
        );
    }

    #[test]
    fn perform_refreshes_sight_after_equipment_changes() {
        let mut st = AppState::new();
        st.city = Some(city());
        st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
        let visible = |st: &AppState| {
            st.player
                .as_ref()
                .unwrap()
                .vis()
                .count(|v| *v == Visibility::Visible)
        };
        // Shades that see further, so wearing them shows.
        let shades = ItemId::new("mirrorshades");
        let item = st.content.items.iter_mut().find(|i| i.id == shades);
        item.unwrap().perks.push(game::Perk::Sight(4));
        st.character.give(shades.clone(), 1);
        perform(&mut st, Action::Equip(shades.clone())).unwrap();
        let worn = visible(&st);
        perform(&mut st, Action::Unequip(Slot::Eyes)).unwrap();
        let bare = visible(&st);
        assert!(worn > bare, "shades on: {worn} vs {bare}");
        perform(&mut st, Action::Equip(shades)).unwrap();
        assert_eq!(visible(&st), worn, "shades on again");
    }

    #[test]
    fn perform_refreshes_sight_after_fitting_chrome() {
        let mut st = AppState::new();
        // Free chrome: this is about sight, not the chair.
        st.free_chrome = true;
        st.city = Some(city());
        st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
        let visible = |st: &AppState| {
            st.player
                .as_ref()
                .unwrap()
                .vis()
                .count(|v| *v == Visibility::Visible)
        };
        // A hand with an eye in its palm, so fitting it shows.
        let hand = ItemId::new("arc_hand");
        let item = st.content.items.iter_mut().find(|i| i.id == hand);
        item.unwrap().perks.push(game::Perk::Sight(4));
        st.character
            .chrome
            .insert(Site::LeftArm, ItemId::new("piston_arm"));
        st.character.give_item(&st.content, hand.clone(), 1);
        let host = Host::Site(Site::LeftArm);
        let piece = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == hand)
            .and_then(|s| s.copy_id())
            .unwrap();
        let fit = Action::Fit {
            host,
            socket: 0,
            piece,
        };
        perform(&mut st, fit.clone()).unwrap();
        let fitted = visible(&st);
        perform(&mut st, Action::Unfit { host, socket: 0 }).unwrap();
        let bare = visible(&st);
        assert!(fitted > bare, "hand fitted: {fitted} vs {bare}");
        perform(&mut st, fit).unwrap();
        assert_eq!(visible(&st), fitted, "hand fitted again");
    }

    #[test]
    fn pack_actions_work_without_a_body() {
        let mut st = AppState::new();
        st.character.give(ItemId::new("mirrorshades"), 1);
        assert!(perform(&mut st, Action::Equip(ItemId::new("mirrorshades"))).is_ok());
        assert!(perform(&mut st, Action::Unequip(Slot::Eyes)).is_ok());
        st.character.hp = 1;
        assert!(perform(&mut st, Action::Use(ItemId::new("stim_patch"))).is_ok());
        let no_body = "no body in the city";
        assert_eq!(
            perform(
                &mut st,
                Action::Drop {
                    item: ItemId::new("copper_coil"),
                    qty: 1
                }
            )
            .unwrap_err(),
            no_body
        );
        assert_eq!(
            perform(&mut st, Action::Fire(AbilityId::new("netscan"))).unwrap_err(),
            no_body
        );
    }

    #[test]
    fn firing_uses_the_sharpened_ability() {
        let mut st = AppState::new();
        st.city = Some(city());
        st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
        st.tick = 100;
        // Handshake at its best grants ping and sharpens its reach by four.
        st.character
            .talents
            .insert(game::TalentId::new("handshake"), 2);
        let base = match st.content.ability(&AbilityId::new("ping")).unwrap().kind {
            AbilityKind::Active(Active::Ping { radius }) => radius,
            _ => unreachable!(),
        };
        perform(&mut st, Action::Fire(AbilityId::new("ping"))).unwrap();
        let radii: Vec<i32> = st.fx.iter().map(|i| i.radius).collect();
        assert!(radii.iter().all(|r| *r == base + 4), "{radii:?}");
    }
}
