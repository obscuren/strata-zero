//! Item actions on the app: `game::apply` plus its effects on the walker and the world.

use crate::app::{AppState, Ping};
use game::{Action, At, Effect};

/// Applies an action at the player's feet and runs its effects on the world.
/// The message is for the status strip; a refusal is the reason.
pub fn perform(st: &mut AppState, action: Action) -> Result<String, String> {
    let (stratum, pos) = st
        .player
        .as_ref()
        .map(|p| (p.stratum, p.pos))
        .ok_or_else(|| "no body in the city".to_string())?;
    let at = At { stratum, pos };
    let outcome = game::apply(
        &mut st.character,
        &mut st.ground,
        &st.content,
        at,
        st.tick,
        action,
    )?;
    for effect in outcome.effects {
        match effect {
            Effect::Reveal { radius } => {
                if let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut()) {
                    player.reveal(city, radius);
                }
            }
            Effect::Ping { radius, ticks } => {
                st.ping = Some(Ping {
                    stratum,
                    pos,
                    radius,
                    until: st.tick + ticks,
                });
            }
        }
    }
    Ok(outcome.message)
}

/// Puts a result where the player sees it: the status strip, warnings in magenta.
pub fn report(st: &mut AppState, result: Result<String, String>) {
    match result {
        Ok(msg) => st.set_status(msg, 90),
        Err(msg) => st.set_warning(msg, 90),
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
        assert_eq!(msg, "netscan: pulse sent");
        let after = seen(&st);
        assert!(after > before, "{after} vs {before}");
        st.character
            .equipment
            .insert(Slot::Neural, ItemId::new("neural_port"));
        perform(&mut st, Action::Fire(AbilityId::new("ping"))).unwrap();
        let ping = st.ping.as_ref().unwrap();
        assert_eq!((ping.radius, ping.until), (24, 280));
        report(&mut st, Err("nope".into()));
        assert_eq!(st.status_text(), Some("nope"));
        assert!(st.status_is_warning());
        report(&mut st, Ok("fine".into()));
        assert!(!st.status_is_warning());
    }
}
