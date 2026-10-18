# Backdoors: lifetimes, chaining and the trace home — design

Date: 2026-10-18. Status: approved in chat by Jeffrey, every question answered
"A" (the `backdoor` program instead of `ssh-copy-id`; a way to list the chain
added at his ask). Spec, plan and build run without check-ins; merging is his call.
Builds on: `2026-10-18-helix-os-design.md` (hosts, the shell, programs, the
rig's `/net`), `2026-10-15-constructs-and-programs-design.md` (the Backdoor
program, the trace, the dump), `2026-10-17-credentials-journal-design.md` (the
journal's entries), `2026-10-17-megablock-home-design.md` (the rig, jacking in
from home), `2026-10-17-access-terminal-design.md` (tiers, the first mail).
Riff notes: `docs/ideas/2026-10-17-terminals-access-apartment.md` §6 and §7.

## Intent

A backdoor stops being a permanent flag that makes you root. It becomes a
**foothold with a lifetime** that use wears down, that a sysadmin finds sooner
or later, and that sometimes gets left in place as bait. There are two kinds:
a **key** you plant from a shell on a building host in a few seconds, good for
hours, and the **Net backdoor** you cut into the server from inside the
construct, good for days. Both are places your rig can put you on the Net
without walking there, and hopping between them is lateral movement. The cost
of running from home is the **trace**: a dump during a run that began at the
rig walks your route back, burning each backdoor it passes, and if it reaches
your door the corp kicks it in. You can cut the trace by burning the backdoor it
sits on, at the cost of the asset. The rig holds three backdoors at a time.

## Words

- **Backdoor**: the thing you planted, either kind. A **key** is the shell
  kind, a **Net backdoor** the program kind.
- **Relay**: a role, not a thing. A backdoor you landed at or hopped through on
  this run, so it sits on your **route** between the target and home.
- **Burn**: a backdoor stops existing. By decay, by the trace, by you, by a raid.
- **Health**: what the journal says from the nominal clock: STABLE, DEGRADING,
  LIKELY BURNED, BURNED.

## Goals

- Every backdoor has a nominal life, a hidden real burn moment, and use costs.
- The journal and HELIX show health in words, never a countdown.
- A key is planted from a shell with `backdoor`; the artefact is a line in
  `~/.ssh/authorized_keys` on that host, which `cat` shows and `rm` removes.
- A key gives login as its user from the rig and from any district terminal;
  the Net backdoor keeps root, the open gate and the 2x cooling.
- Jacking in at the rig offers every live backdoor as a landing, at its
  construct's gate on the Net.
- Inside a construct you hold a backdoor on, at its server, you can hop to any
  other backdoor's gate.
- The route of a rig run is shown on the HUD, by `route` on the rig, in the
  rig's `net.log` and in the journal.
- A dump on a rig run starts a home trace that walks the route back, burning
  relays, and ends in a raid: the rig seized a day, every backdoor burned, the
  fixer gone quiet.
- Burning the relay the trace sits on cuts the trace, puts that building on
  alert and patches the host for a day.
- The rig holds `SLOTS = 3` backdoors; planting a fourth is refused.
- Honeypots: a backdoor that burns on a grey-or-blacker host has a one in three
  chance of staying usable as bait; its first use trips the alarm and, from the
  rig, starts a trace with that relay already gone. The host's `auth.log` gives
  it away to anyone who reads it first.

## Non-goals

- Saving backdoors, routes or traces to disk. Net state stays session-only.
- Combat or anyone appearing in the flat during the raid; damage; a wanted
  level or bounty on the street (nothing reads one yet).
- Rig upgrades (more slots, longer holds). `SLOTS` and the hold times are
  constants shaped for it.
- Selling backdoors to fixers; corp counter-ops as jobs.
- Landing choices at street ports: they land where they are, as today.
- Changing the Space window or its wording beyond the rows this slice adds
  (the interaction redesign is its own spec and may be built in parallel).
- Talents that touch backdoors. Quality comes from the program's rating and
  the host's security only.

## 1. The model: `backdoors.rs`

A new module `crates/strata-zero/src/backdoors.rs`, `st.backdoors: Backdoors`
on the app state, session-only like `NetState`, reset by `enter_city`. It
replaces `NetState.backdoors: BTreeSet<BuildingId>`; the six readers of that
set (`tier_on`, `user_for`, `sync_guards`, `minute`'s cooling, the journal's
entries, the hosts test) ask this module.

```rust
pub enum Kind { Key { user: String }, Net { rating: u8 } }
pub enum Why { Decay, Trace, You, Raid, Honeypot }
pub enum State { Live, Honeypot, Burned { at: u64, why: Why } }
pub struct Backdoor {
    pub building: BuildingId,
    pub kind: Kind,
    pub planted: u64,          // game minute
    pub nominal: u64,          // the life it was planted with, in minutes
    pub spent: u64,            // minutes taken off by use
    pub factor: u8,            // hidden: 70..=130, by hash of (seed, building, planted)
    pub state: State,
}
pub enum Health { Stable, Degrading, LikelyBurned, Burned }
pub struct Route { pub relays: Vec<BuildingId> }        // the one you landed at first
pub struct HomeTrace { pub relays: Vec<BuildingId>, pub at: usize, pub moves_at: u64 }
pub struct Raid { pub at: u64 }
#[derive(Default)]
pub struct Backdoors {
    pub list: Vec<Backdoor>,
    pub route: Option<Route>,                // this run's, only when it began at the rig
    pub trace: Option<HomeTrace>,
    pub patched: BTreeMap<BuildingId, u64>,  // the hole is patched until this minute
    pub rig_seized_until: Option<u64>,
    pub fixer_quiet_until: Option<u64>,
    pub raided: Vec<Raid>,                   // the mail is sent when the rig comes back
    last_minute: u64,
}
pub const SLOTS: usize = 3;
pub const KEY_LIFE: u64 = 120;               // minutes; halved on a host with grey ICE or worse
pub const NET_LIFE: u64 = 3 * 24 * 60;       // doubled at rating 2 and up
pub const COST_SSH: u64 = 10;                // a login through a key or root
pub const COST_LAND: u64 = 20;               // a landing or a hop, keys
pub const COST_LAND_NET: u64 = 60;           // a landing or a hop, Net backdoors
pub const COST_WRITE: u64 = 5;               // a device write from a remote session
pub const HOLD_KEY: u64 = 30;                // the trace sits on a key this long
pub const HOLD_NET: u64 = 60;                // on a Net backdoor; doubled at rating 2
pub const LAST_LEG: u64 = 20;                // the nearest relay (or the rig) to your door
pub const PATCH_MINUTES: u64 = 24 * 60;
pub const SEIZED_MINUTES: u64 = 24 * 60;
pub const QUIET_MINUTES: u64 = 24 * 60;
pub const PLANT_NOISE: u8 = 15;
pub const HONEYPOT_NOISE: u8 = 60;
```

**Life.** `nominal` is `KEY_LIFE` (halved when the host's security is at or
above `GREY_THRESHOLD`) or `NET_LIFE` (doubled when the program's rating is 2
or more). The real burn moment is `planted + (nominal × factor / 100) − spent`;
`factor` is `70 + hash64(seed, building ^ planted) % 61`. Nothing on screen
ever uses `factor`.

**Health** from the nominal clock, with `left = nominal − spent − (now − planted)`
(saturating): `Burned` when the state says so; `LikelyBurned` when `left == 0`;
`Degrading` when `left ≤ nominal / 2`; else `Stable`. Labels upper case:
`STABLE`, `DEGRADING`, `LIKELY BURNED`, `BURNED`.

**API** (pure over game minutes; `seed` and security come from the caller):

- `live(b) -> Option<&Backdoor>`: a `Live` or `Honeypot` one on `b`. `root_on(b)`:
  a live Net backdoor on `b`. `held(b)`: any not-burned backdoor on `b`.
- `slots_used()`: not-burned backdoors. `can_plant(b, now) -> Result<(), Refusal>`
  with `Refusal::{Seized, Patched(until), Full, Already}`, checked in that order.
- `plant_key(b, user, security, seed, now)`, `plant_net(b, rating, seed, now)`:
  push a `Live` backdoor.
- `spend(b, minutes)`: adds to `spent`.
- `check(b, security, seed, now) -> Use`: what using it now does. `Use::Works`
  when the real moment has not passed; else, the first time, the roll:
  `hash64(seed, building ^ 0xB00B) % 3 == 0` on a grey-or-worse host makes it
  `Honeypot` (state set, `Use::Trap`), otherwise `Burned { why: Decay }`
  (`Use::Refused`). A `Honeypot` state answers `Trap` and becomes
  `Burned { why: Honeypot }`: bait springs once. A `Burned` one answers
  `Refused`.
- `burn(b, why, now) -> Burnt` with `Burnt { cut_trace: bool }`: marks the
  backdoor; `You` and `Trace` also patch the host for `PATCH_MINUTES`; `You`
  on the relay the trace sits on removes the trace and reports `cut_trace`.
- `start_route(b)`, `hop(b)`, `end_route()`; `route_line(names)` builds
  `zenbo ▸ hollis-court ▸ meridian`.
- `start_trace(now)`: from `route`'s relays that are not burned, nearest-the-target
  first (so the reverse of the route's order), `at = 0`, `moves_at = now +
  hold(relays[0])`; with no relays left, `moves_at = now + LAST_LEG`. The trace's current relay is
  `relays[at]`; past the last relay it is on the last leg.
- `trace_line(now, names) -> Option<String>`: `TRACE ▸ hollis-court · 23 min`,
  or `TRACE ▸ your door · 7 min` on the last leg.
- `minute(now) -> Vec<Event>` steps once per game minute from `last_minute`:
  a trace whose `moves_at` passed burns its relay (`Why::Trace`, patched) and
  moves on (`Event::RelayBurned(b)`); past the last relay it starts the last
  leg; at the end of the last leg `Event::Raid`: the trace is gone, every
  not-burned backdoor burns with `Why::Raid` (no patch), `rig_seized_until`
  and `fixer_quiet_until` are set, a `Raid` is pushed. When a seizure ends and a
  `Raid` has no mail yet, `Event::RigReturned`. Idempotent for the same minute.

## 2. Planting

**Keys**: the `backdoor` program on a building host (never the rig or a district
host: `backdoor: no server here`). As `guest`: `backdoor: Permission denied`.
Refusals in order, each `status 1`:

```
backdoor: the rig is seized · no line home
backdoor: hole patched · try again tomorrow
backdoor: the rig holds 3 lines · burn one first
backdoor: already planted here
```

Success: `~/.ssh/` and `~/.ssh/authorized_keys` of the shell's user are created
if missing (owner that user, `rw----`), and the line
`helix-ed25519 <16 hex from hash64(seed, "key")> <you>@zenbo` is appended;
`plant_key` runs; `PLANT_NOISE` goes on the building's trace through
`NetState::add_heat`, raising the alarm on `Alarm` and never dumping (nobody is
jacked in); the reply is
`KEY PLANTED · mara-voss@hollis-court · the rig holds 2 of 3`. The journal
learns nothing else: the backdoor is the entry. `ls -l /net` on the rig lists it
the next time the rig's disk is touched (see §5).

**Net backdoors**: the Backdoor program lands as today but through
`plant_net(b, spec.rating, …)`. `start_run` refuses with the refusal's words
when `can_plant` fails, and the program's row in the construct's list is greyed
with the same words after the odds: `backdoor · server · the rig holds 3 lines`.
The log line stays `backdoor planted · `Hollis Court`'s gate is yours`, now
with ` · 1 of 3` after it.

**What they unlock**

| | Key | Net backdoor |
|---|---|---|
| Shell | login as its user, from the rig and any district terminal | root, likewise |
| Net | a landing at that construct's gate; the gate ICE stands | a landing; gate ICE down; heat cools 2x (as today) |
| Life | `KEY_LIFE` | `NET_LIFE` |

`tier_on`: `Admin` when `root_on(b)`; a live key makes the tier at least `User`.
`user_for`: root for a Net backdoor; a key's user for a key (over card or code);
else as today. A `Trap` or `Refused` on `ssh` is handled in §5.

## 3. Landing and hopping

**The landing list.** `WorldAct::JackIn` at the rig, when `list` has any
not-burned backdoor, pushes a popup titled `JACK IN` instead of jacking in:
`Choice('a', "your block", Pick::World(WorldAct::JackIn))` then one row per
not-burned backdoor in planted order, letters from the free pool:
`Hollis Court · root · STABLE`, `Meridian Towers · mara-voss · DEGRADING`,
picking `Pick::World(WorldAct::Land(b))`. Without backdoors the act jacks in
at once as today. The popup is the plain `Popup::verbs`; the interaction
redesign may restyle it later.

**`WorldAct::Land(b)`** (`net::land_at(st, b)`):
1. `jack_in_refusal` as today (cooling; the rig ignores lockdowns).
2. `check(b)`: `Refused` → `connection refused · key rejected`, the journal shows
   BURNED, no jack-in. `Trap` → jack in at the gate, then the alarm on `b`
   (`raise_alarm`), `HONEYPOT_NOISE` heat, `start_route(b)` then
   `start_trace` with that relay already burned (`Why::Honeypot`), the warning
   `honeypot · they were waiting · trace running`. `Works` → jack in.
3. Jacking in: `Player::jack_in` as from the rig (body stays in the flat), then
   the player is moved to `city_gate(b)` with the Net FOV refreshed, the log
   `jacked in · `Hollis Court`'s gate · via your backdoor`, `spend(b,
   COST_LAND or COST_LAND_NET)`, `start_route(b)`.
   A gate that is not walkable (never happens for a generated gate) falls back
   to the landing beside your block.

**Jacking in at your block from the rig** starts an empty `Route` (`relays`
empty): it is a rig run with nothing between you and home. A street-port
jack-in leaves `route` `None`.

**Hopping.** In `run_choices`, when the player stands in `Place::Construct(b)`
with `held(b)` live, on or 8-adjacent to the server node, one row per other
not-burned backdoor: `hop · Meridian Towers · root · STABLE` → `Pick::Hop(b2)`.
Firing it (`net::hop(st, b2)`): `check(b2)` as above (a `Trap` lands you and
springs); `spend`; the player leaves the construct onto `city_gate(b2)` via the
`land` helper, `hop(b2)` on the route, the log `hop · `Meridian Towers`'s gate`;
inside an engagement it costs one AP like a program turn; `run` is cleared
(`run aborted` when one was up). Only on a rig run; from a street port the rows
read `hop · … · no route home` greyed.

**Ending.** `jack_out` and `dump_for` call `end_route()`; `dump_for` on a
rig run first calls `start_trace(now)` and warns `trace running · it starts at
`Hollis Court``, or `trace running · straight to your door` with no relays.

## 4. The trace and the raid

`Backdoors::minute(now)` runs beside `st.net.minute` in the map screen's
update and in the terminal screen's update (the rig's shell is where you watch
it). Events become log lines:

- `RelayBurned(b)`: `trace passes `Hollis Court` · the backdoor is gone` (warn).
- `LastLeg`: `trace · nothing left between them and your door` (warn).
- `Raid`: `st.story("they kicked your door in · the rig is evidence · every backdoor is gone")`,
  `Fx::Kick(20)`, repaint. If the player is jacked in at that moment the run is
  ended with `dump_for(…, "the line went dead", COOLING_MINUTES)` first.
- `RigReturned`: a mail from the fixer (`journal.favour`'s name, else `Dez`),
  subject `lose this number`, body: "They came to your door. I don't know you
  for a while. Don't call." The favour quest is untouched.

**Burning by hand.** `burn <host>` on the rig (`host` as `/net` names it, a
prefix allowed, number allowed as `ssh` does): refuses `burn: <x>: no such
backdoor`; else `burn(b, You, now)`; reply `BURNED · hollis-court`, plus
` · TRACE CUT` when it cut the trace, and then `raise_alarm(b)` on that
building (`ALARM · … locks down` in the log). On a key host, `rm
~/.ssh/authorized_keys` (or `rm` of the `.ssh` directory) by any user burns the
key the same way (`You`); the vfs needs a hook: `Io::removed(host, path)` called
after a successful `rm`, which the app's `Io` answers.

**The seized rig.** While `rig_seized_until` is in the future: the rig's Space
rows still open, but `Terminal::open` on the rig shows a boot of two lines,
`EVIDENCE · THIS UNIT IS HELD UNDER WARRANT`, `CONTACT THE DISTRICT OFFICE`, no
prompt, no commands, Esc leaves; `jack_in_refusal` at the rig answers `the rig
is gone · evidence`; the landing list does not open. The journal's rig group
(your block) gets a dim line `rig seized · back day 3 04:12`.

**The fixer quiet.** `fixer_quiet_until` is read by nothing in this slice but
the journal line `fixers quiet until day 4 04:12` under the same group, and by
`credentials::first_mail`, which does not send while it is set (a new game sets
nothing, so this only matters for a later slice).

## 5. Showing it

**Journal** (`credentials::entries`), per building after codes and cards, one
line per backdoor, burned ones kept but dim:

```
  backdoor · root on `Hollis Court` · planted day 2 03:12 · STABLE
  key · `Mara Voss`'s login on `Hollis Court` · planted day 2 04:40 · DEGRADING
  key · `Dev Okafor`'s login on `Hollis Court` · BURNED day 2 09:00 · by you
```

`by you`, `found`, `the trace took it`, `the raid`, `a honeypot` by `Why`.
A `Honeypot` state shows as whatever the nominal clock says: you are not told.
While a route exists, a dim line above the first group: `route · zenbo ▸
hollis-court ▸ meridian`; while a trace runs, `trace ▸ hollis-court · 23 min`.
`EntryKind::Backdoor` becomes `Backdoor(usize)`, the index into `list`; the detail
card gives kind, user, host, planted, health, and the state's words.

**HUD.** `Footer.trace` while jacked in from a rig run gains ` · ROUTE zenbo ▸
hollis-court` (cut as the footer cuts). A running trace puts `TRACE ▸
hollis-court · 23 min` in magenta on the footer's first row whether or not you
are jacked in, before the construct's trace when both show. A seized rig puts
nothing on the HUD.

**HELIX on the rig.**
- `/net/<host>` exists for every not-burned backdoor's building (beside your
  block's entry), as `Dev::NetHost(b)`. `ensure_host` adds and removes these
  entries to match on every touch, as it does for mails (a `nets_placed` set).
  `cat /net/hollis-court` prints the scan row then `LINE  root  STABLE
  planted day 2 03:12` (or `LINE  mara-voss  …`); `ls -l /net` lists them.
- `ssh <host>` from the rig or a district host to a backdoored building:
  `check(b)`: `Works` → login as the key's user or root, `spend(COST_SSH)`,
  the reply `CONNECTED TO HOLLIS COURT · CLEARANCE ADMIN · via your backdoor`;
  `Refused` → `ssh: hollis-court: connection refused · key rejected` (status
  1, journal BURNED); `Trap` → login succeeds, then `raise_alarm(b)` and
  `HONEYPOT_NOISE`, the reply's second line `AUDIT · THIS SESSION IS BEING
  RECORDED`, and when the shell's origin is the rig a trace starts with an
  empty route (`LAST_LEG` to your door). A device write from a session that
  logged in through a backdoor spends `COST_WRITE`.
- `route`: on the rig only. Jacked in on a rig run: `ROUTE  zenbo ▸ hollis-court
  ▸ meridian` then one line per relay `hollis-court   root   STABLE`. Not jacked
  in: `NO ROUTE · you are home`, then `TRACE ▸ hollis-court · 23 min` when a
  trace runs, else `TRACE  none`. Also `LINES  2 of 3` as a last line always.
- `burn <host>`: §4.
- `/var/log/net.log` on the rig (new: the rig gets `/var/log/`, owned by you,
  with `Dev::NetLog`; on a host with no building `read_dev` answers with the
  home side): the route line, the trace line, and `LINES 2 of 3`, the same words.
- A key host's `/var/log/auth.log` (`access::log_lines`) adds, for a `Honeypot`
  on that building, a first line `HH:MM  audit  key flagged · monitoring`, and
  for every login through a backdoor `HH:MM  login  <user> via key`.

**Refusal voice** on the Net side: `connection refused · key rejected`,
`honeypot · they were waiting · trace running`, in the log's warn colour.

## 6. Where the code goes

- `backdoors.rs`: everything in §1, pure, with its own tests.
- `net.rs`: `land_at`, `hop`, `start_route` calls in the rig jack-in path,
  `end_route`/`start_trace` in `jack_out` and `dump_for`, `sync_guards` and
  `minute` reading `root_on`, `land`'s Backdoor arm calling `plant_net`,
  `start_run` refusing on `can_plant`, `jack_in_refusal` for the seized rig.
- `access.rs`: `tier_on` through `backdoors`, `log_lines` with the audit and
  login lines, `trace_lines` on the rig.
- `helix/bin.rs`: `backdoor`, `burn`, `route` programs; `ssh` through `check`;
  `Io::removed`; `PROGRAMS` grows by three; `man` pages for them.
- `helix/hosts.rs`: `user_for` through `backdoors`; the rig's `/net` entries
  kept in step; the key file is ordinary vfs content.
- `credentials.rs`: entries, the fixer's mail, `first_mail` quiet.
- `screens/popup.rs`: `WorldAct::Land(BuildingId)`, `Pick::Hop(BuildingId)`.
- `screens/explore.rs`: the landing list at the rig, hop rows in
  `run_choices`, `backdoors.minute` beside `net.minute`, the HUD strings.
- `screens/terminal.rs`: the seized boot; `backdoors.minute` in update.
- `ui/hud.rs`: the footer's trace row taking the home trace.
- `app.rs`: `backdoors` on the state, reset in `enter_city`.
- Console: `trace` (start a home trace from the current route for tests and
  play), `seize` (seize the rig), `backdoor <host>` (plant a key as root).

## Testing

- `backdoors.rs`: health at each boundary from the nominal clock; the factor
  never reaches the screen (health ignores it); `check` works before the real
  moment and refuses after, honeypot by hash on grey and never on white;
  every use cost; `can_plant` refusals in order; `burn` by each `Why`, the
  patch, the cut; `start_trace` ordering for a two-relay route; `minute`
  through two relays, the last leg and the raid's three costs; `RigReturned`
  once; idempotence for a repeated minute; `route_line` and `trace_line`.
- Shell: `backdoor` as guest, resident and root; each refusal's wording in
  order; the key file's owner, mode and line; `rm` of the file burns it;
  `burn` by name, prefix and number, `TRACE CUT` and the alarm; `route` jacked
  in and not; `ssh` through a key from the rig and from another district's
  terminal, refused after a burn with the words and the journal BURNED, a
  honeypot's audit line and alarm; `cat /net/<host>`'s LINE row; `man` for the
  three.
- App: the landing list's rows and letters; a landing at the gate with the
  route started and life spent; a refused landing; a honeypot landing's alarm
  and trace; a hop from the server with the route grown and `run` cleared;
  hop rows absent away from the server and greyed on a street run; a dump on a
  rig run starting the trace and the HUD line; a dump on a street run
  starting nothing; the trace burning a relay on the minute; the raid's story
  line, the seized boot, the jack-in refusal, the mail a day later; the
  Backdoor program greyed at the cap; the journal rows and the detail card;
  `tier_on` and `user_for` with a key, a Net backdoor, and after a burn.
- One ignored fuzz run: across three cities, plant a key on every scannable
  building of one district, land at each, hop between two, burn them all, and
  check `slots_used` never exceeds `SLOTS` and nothing panics.
