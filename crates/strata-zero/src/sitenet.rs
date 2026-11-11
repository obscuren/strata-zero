//! A site's own Net at play: which buildings are sites, the security that governs one, its
//! construct made once and kept, and what its Net held dropped when a mission's copy
//! begins or ends. The Net itself is worked out from the plan (and a live copy's layer)
//! whenever it's needed (`citygen::sitenet`), never kept; the construct is kept and saved
//! with the rest. Spec `docs/superpowers/specs/2026-11-11-site-net-design.md`.

use crate::app::AppState;
use citygen::sitenet::{SiteNet, site_construct};
use citygen::venue::{MissionLayer, VenuePack};
use citygen::{BuildingId, FloorPlan, NetNode};
use game::HackTarget;
use std::borrow::Cow;

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
fn layer_of(st: &AppState, b: BuildingId) -> Option<&MissionLayer> {
    (st.interiors.copy(b))
        .filter(|c| c.live)
        .and_then(|c| c.layer.as_ref())
}

/// Site `b`'s plan as the session has it and its Net worked out from it.
fn worked_out(st: &AppState, b: BuildingId) -> Option<(Cow<'_, FloorPlan>, SiteNet)> {
    let city = st.city.as_ref()?;
    let plan = crate::interiors::plan_now(st, b)?;
    let layout = plan.venue.as_deref()?;
    let t = VenuePack::builtin().by_id(&layout.type_id);
    let net = SiteNet::of(city, &plan, t, layer_of(st, b), security(st, b))?;
    Some((plan, net))
}

/// Site `b`'s Net, worked out now from its plan, a live copy's layer and its security;
/// never kept. None for a building that is no site.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "HELIX and the panel read it from Task 13 on")
)]
pub fn net_of(st: &AppState, b: BuildingId) -> Option<SiteNet> {
    worked_out(st, b).map(|(_, net)| net)
}

/// Makes site `b`'s construct the first time it's needed and keeps it. Made over a
/// construct from before (an old save's): that one's ICE and the hacks on its guards go
/// with it (Ruling 13).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the ways in (the firewall, a floor's terminal) make it from Task 11 on"
    )
)]
pub fn ensure_construct(st: &mut AppState, b: BuildingId) -> Result<(), String> {
    if st.constructs.site(b).is_some() {
        return Ok(());
    }
    let (plan, net) = worked_out(st, b).ok_or("that building has no Net of its own")?;
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
/// holds, its feed, its broken locks, its construct and its ICE. A copy beginning or
/// ending calls it, never while you're in the building.
pub fn forget(st: &mut AppState, b: BuildingId) {
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
    drop_ice(st, b);
    if st.constructs.site(b).is_some() {
        st.constructs.drop(b);
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) mod testutil;
