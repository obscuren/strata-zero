//! A site's own Net at play: which buildings are sites, the security that governs one, its
//! construct made once and kept, and what its Net held dropped when a mission's copy
//! begins or ends. The Net itself is worked out from the plan (and a live copy's layer)
//! when first needed (`citygen::sitenet`) and kept beside the plan, never saved; the
//! construct is kept and saved with the rest. Spec
//! `docs/superpowers/specs/2026-11-11-site-net-design.md`.

use crate::app::AppState;
use citygen::sitenet::{SiteNet, site_construct};
use citygen::venue::{MissionLayer, VenuePack};
use citygen::{BuildingId, NetNode};
use game::HackTarget;
use std::sync::Arc;

mod firewall;
mod locks;
mod nodes;
mod panel;
mod walk;
mod ways;
pub use firewall::*;
pub use locks::*;
pub use nodes::*;
#[expect(
    unused_imports,
    reason = "the site's Net panel reads it from Task 15 on"
)]
pub use panel::*;
pub use walk::*;
pub use ways::*;

/// Whether building `b` is a site: its plan is a venue's (Ruling 14). The session's plan
/// when it holds one, so an old save's hotel kept on the classic builder is none; else a
/// kind a venue type builds whose layout lays.
pub fn is_site(st: &AppState, b: BuildingId) -> bool {
    st.city
        .as_ref()
        .is_some_and(|city| st.interiors.is_venue(city, b))
}

/// The security that governs site `b`: a live copy's own, else its subnet's. It sets the
/// firewall's locks, the ICE, the air gap and the odds and trace of runs on the site.
pub fn security(st: &AppState, b: BuildingId) -> u8 {
    match st.interiors.copy(b).filter(|c| c.live) {
        Some(copy) => copy.inputs.security,
        None => st
            .city
            .as_ref()
            .and_then(|city| crate::render::netlook::security_of(city, b))
            .unwrap_or(0),
    }
}

/// The live copy's layer on `b`, if a copy holds it.
pub(crate) fn layer_of(st: &AppState, b: BuildingId) -> Option<&MissionLayer> {
    (st.interiors.copy(b))
        .filter(|c| c.live)
        .and_then(|c| c.layer.as_ref())
}

/// Site `b`'s Net worked out from its plan as the session has it, a live copy's layer and
/// its security.
fn work_out(st: &AppState, b: BuildingId) -> Option<SiteNet> {
    let city = st.city.as_ref()?;
    let plan = crate::interiors::plan_now(st, b)?;
    let layout = plan.venue.as_deref()?;
    let t = VenuePack::builtin().by_id(&layout.type_id);
    SiteNet::of(city, &plan, t, layer_of(st, b), security(st, b))
}

/// Site `b`'s Net, worked out from its plan, a live copy's layer and its security the
/// first time it's asked and kept (`Interiors`, never saved) until its plan or its copy
/// changes or `forget` drops it. None for a building that is no site.
pub fn net_of(st: &AppState, b: BuildingId) -> Option<Arc<SiteNet>> {
    st.interiors.site_net(b, || work_out(st, b))
}

/// Makes site `b`'s construct the first time it's needed and keeps it. Made over a
/// construct from before (an old save's): that one's ICE and the hacks on its guards go
/// with it (Ruling 13).
pub fn ensure_construct(st: &mut AppState, b: BuildingId) -> Result<(), String> {
    if st.constructs.site(b).is_some() {
        return Ok(());
    }
    let net = net_of(st, b).ok_or("that building has no Net of its own")?;
    let plan = crate::interiors::plan_now(st, b).ok_or("that building has no Net of its own")?;
    let city = st.city.as_ref().ok_or("no city")?;
    let k = site_construct(city, &net, &plan, layer_of(st, b));
    drop(plan);
    drop_ice(st, b);
    st.constructs.put_site(k);
    Ok(())
}

/// Drops building `b`'s ICE states and every hack on a piece of its ICE.
fn drop_ice(st: &mut AppState, b: BuildingId) {
    st.net.ice.remove(&b);
    st.net
        .hacks
        .retain(|h| !matches!(h.target, HackTarget::Guard { building, .. } if building == b));
}

/// Drops what the site's Net held: its hacks, a run in flight on one of its nodes, its
/// holds, its feed, its broken locks, its construct, its ICE and the Net kept from its
/// plan. A copy beginning or ending calls it, never while you're in the building.
pub fn forget(st: &mut AppState, b: BuildingId) {
    st.interiors.drop_site_net(b);
    let ours = |n: NetNode| n.site().is_some_and(|s| s.building == b);
    st.net
        .hacks
        .retain(|h| !matches!(h.target, HackTarget::Site(s) if s.building == b));
    if (st.net.run.as_ref())
        .is_some_and(|r| matches!(r.target, HackTarget::Site(s) if s.building == b))
    {
        st.net.run = None;
    }
    st.net.holds.retain(|h| !ours(h.node));
    if st.net.feed.is_some_and(ours) {
        st.net.feed = None;
    }
    st.net.firewalls.remove(&b);
    drop_ice(st, b);
    if st.constructs.site(b).is_some() {
        st.constructs.drop(b);
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) mod testutil;
