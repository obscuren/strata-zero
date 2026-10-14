//! Item actions on the app: `game::apply` plus its effects on the walker and the world.

use crate::app::{AppState, FxInstance};
use citygen::{Pos, StratumKind};
use game::{AbilityKind, Action, Active, At, Effect};

/// Applies an action at the player's feet and runs its effects on the world.
/// The message is for the status strip; a refusal is the reason.
pub fn perform(st: &mut AppState, action: Action) -> Result<String, String> {
    // Pack-only actions work anywhere; the ground and the world need a body.
    let needs_body = matches!(
        action,
        Action::Drop { .. } | Action::Take { .. } | Action::TakeAll | Action::Fire(_)
    );
    let (stratum, pos) = match st.player.as_ref().map(|p| (p.stratum, p.pos)) {
        Some(body) => body,
        None if needs_body => return Err("no body in the city".to_string()),
        None => (StratumKind::Street, Pos::new(0, 0)),
    };
    let at = At { stratum, pos };
    let fired = match &action {
        Action::Fire(id) => Some(id.clone()),
        _ => None,
    };
    let refresh_sight = matches!(action, Action::Equip(_) | Action::Unequip(_));
    let outcome = game::apply(
        &mut st.character,
        &mut st.ground,
        &st.content,
        at,
        st.tick,
        action,
    )?;
    if refresh_sight {
        let bonus = st.character.sight_bonus(&st.content);
        if let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut()) {
            player.update_fov(city, bonus);
        }
    }
    let mut revealed = Vec::new();
    for effect in outcome.effects {
        match effect {
            Effect::Reveal { radius } => {
                if let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut()) {
                    revealed = player.reveal(city, radius);
                }
            }
        }
    }
    // The ability's effects start now, at the firer's feet.
    if let Some(id) = fired
        && let Some(ability) = st.content.ability(&id)
    {
        let radius = match ability.kind {
            AbilityKind::Active(
                Active::Scan { radius } | Active::Ping { radius } | Active::Trace { radius },
            ) => radius,
            AbilityKind::Passive => 0,
        };
        for fx in &ability.fx {
            st.fx.push(FxInstance {
                fx: fx.clone(),
                stratum,
                origin: pos,
                radius,
                started: st.tick,
                revealed: revealed.clone(),
            });
        }
    }
    Ok(outcome.message)
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
    use game::{AbilityId, ItemId, Slot};

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
            .equipment
            .insert(Slot::Neural, ItemId::new("neural_port"));
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
        perform(&mut st, Action::Unequip(Slot::Eyes)).unwrap();
        let bare = visible(&st);
        perform(&mut st, Action::Equip(ItemId::new("nightglass_optics"))).unwrap();
        let optics = visible(&st);
        assert!(optics > bare, "optics on: {optics} vs {bare}");
        perform(&mut st, Action::Unequip(Slot::Eyes)).unwrap();
        assert_eq!(visible(&st), bare, "optics off again");
    }

    #[test]
    fn pack_actions_work_without_a_body() {
        let mut st = AppState::new();
        assert!(perform(&mut st, Action::Unequip(Slot::Eyes)).is_ok());
        assert!(perform(&mut st, Action::Equip(ItemId::new("nightglass_optics"))).is_ok());
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
}
