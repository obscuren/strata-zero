//! Shooting: a fire action resolved round by round, the pictures it pushes, and
//! what the rounds do to whoever they hit.

use crate::app::AppState;

/// A foe fires its weapon at you.
pub fn fire_at_you(st: &mut AppState, foe: usize) {
    let Some(name) = st.foes.get(foe).map(|f| f.name.clone()) else {
        return;
    };
    st.say(format!("{name} fires"));
}
