# The Net upgraded, part two: what a hacked node does, and danger outside

Date: 2026-11-04. Builds on part one (`2026-11-03-net-look-design.md`, merged). Ideas:
`docs/ideas/2026-11-03-net-nodes.md`. Mockups: `2026-11-04-net-nodes-mock/`
(`round1.html`: how a held node offers its verbs A/B/C, a tapped camera's feed A/B/C,
lights out; `round2.html`: danger outside A/B/C, and the nodes at work).

## Why

Netrunners "aren't that strong out in the open" in meat space, though they can hold
their own. The Net gets this much control over the street (lamps, cameras, holo ads,
terminals) so a netrunner can move about in meat space and complete jobs and missions
that way. A Net effect is judged by what it lets you do on foot: get past, go unseen,
pull eyes away, open a way.

Today half the nodes do nothing when hacked (a blinded camera changes nothing, an ad
board or a terminal only says "cracked"), and nothing can be hacked outside a
construct, so the open Net is safe and empty: "without ICE hunting or tracing kicking
you out it means that there's no danger in running there."

## What Jeffrey decided

- **Programs take the node; the node gives the verbs.** Crack (loud) or spoof (quiet)
  takes a node for an hour. While you hold it, its kind offers its verbs.
- **The dock (round 1, B).** When the run lands, its tether stays on the node as a dock
  reading the node, `HELD` with the time left, and the verbs with their keys.
- **The feed hangs off the node (round 1, A).** A tapped camera opens a small live
  window on a wire from its node: the street round the camera, walkers moving, in
  green phosphor.
- **Nodes on the open Net too.** A building's facade nodes (cameras, holo ads, door
  locks) sit on its construct's outer wall on the open Net; the district's lamp grid
  and street terminals sit out on the grid. All of them can be hacked from outside,
  with the same verbs, as well as from inside.
- **Danger outside: build A, B and C.** A, tracers, always. B, sweeps, and C, sentinels,
  can be swapped; C is on, B is the fallback.
- **The rest as drawn in round 2**: camera tap and loop, camera eyes on the street, ad
  hijack (smear, lure, tag) and decoy, lamp grid lights out, the building's shell from
  the Net. You are blue (done before this slice).

## Goals

- Every camera, ad board, lamp grid and terminal node does something when held, from
  outside or inside; each effect lands in the street (or, for a decoy, in the run).
- The open Net has teeth: hacking there heats a trace that, past its alarm, sends ICE
  after you, and at its dump jacks you out.
- A netrunner can, from the Net: see a street through a camera, keep a camera from
  seeing them, darken a district, pull walkers' eyes onto an ad, open a building's
  shell.

## Non-goals

- Hub (backbone) hacks, door seal, lift hold, server scrub, package interception: later.
- Jobs that ask for a hijack or a lights out: later (the smear line is there for them).
- Saving any of this: holds, hijacks, darkness, grid ICE and traces live in the session,
  as constructs and traces do today.

## Anti-goals

- No effect that only changes something in cyberspace when it could change the street.
- Nothing written into the saved city (`st.city`'s saved fields).
- The open Net must not become a slog: ICE that hunts, not ICE that blocks every lane.

## The rules

### Owners and traces

- Every node belongs to a subnet: a building's (its facade and inside nodes) or a
  district's (lamp grid, street terminals, hub). An **owner** is that building or
  district.
- Building traces stay as they are (keyed by building). Districts get traces of their
  own, the same `Trace` (heat, alert, cooling), in a second map. Every heat source goes
  through one owner-keyed path.
- A run's heat goes to its target's owner. A wall node hacked from outside heats the
  same building trace as the same node inside, so the construct is hotter when you go
  in.
- **Outside is louder**: each turn of a run on the open Net makes half again the noise
  it would inside (rounded up).
- Alarm (60) and dump (100) as today. A dump outside jacks you out as inside (and on a
  rig run starts the trace home). A building alarm reached from outside locks the
  building down as today; a district alarm lasts `ALERT_MINUTES` like a building's.
- Traces you are not in cool as today; a district's cools while you are not hacking it.
- The footer's TRACE on the open Net reads the owner of the run in flight, else the
  nearest construct (as today), and names it (`Stack 101 (district)` for a district).

### Runs outside a construct

- On the open Net, a program key offers the nodes on your tile and the eight around
  it: a construct's wall nodes (any kind) and district nodes (lamp grid, street
  terminal). Not hubs.
- The run works as inside: beats roll it, a move aborts it, `.` hurries the beat, the
  tether shows it, odds from the owner's security.
- What lands is what lands inside (a door lock opens the building's doors, and so on),
  plus the holds below.

### Held nodes and their verbs

- A crack or a spoof that lands on a camera, an ad board, a lamp grid or a terminal
  **holds** it for `HACK_MINUTES` (an hour). Spoof now applies to those four kinds as
  well as locks and lifts. Blind still loops a camera directly.
- Holding is by node: a node held from outside is held inside too.
- Verbs, by kind (keys are free in walk mode; letters may repeat across kinds):

  | Kind | Verb | Key | Where |
  |---|---|---|---|
  | camera | tap | `f` | anywhere |
  | camera | loop | `o` | anywhere |
  | ad board | hijack | `i` | anywhere |
  | ad board | decoy | `z` | inside its construct, with ICE in it |
  | lamp grid | lights out | `o` | anywhere |
  | terminal | shell | `r` | anywhere |

- A verb key acts on the held node **nearest you** on your map (the open Net or the
  construct holding it), within 12 tiles; out of reach it says so.
- Each verb works once per hold except tap (toggles the feed) and shell (opens again).

### The dock

- The held node nearest you on screen carries a dock: the run's tether that stayed. Its
  label reads `CAMERA` and `HELD 59m`, its second row the verbs (`[f] tap  [o] loop`),
  its wire lit whole to the node. Other held nodes on screen glow on their tile, no
  label.
- A spent verb drops off the dock (a camera looped reads `[f] tap · looped`).
- When the hold runs out the dock goes.

### Camera: tap, loop, eyes

- **Tap** opens the feed: a window 32 by 12 (at 100 columns, 28 by 10) hanging off the
  node on a wire, as the tether's label does, on the side with room. It draws the
  street around the camera's anchor live, walkers included, in green phosphor (each
  cell by its light; walkers white; cameras red), with the header
  `CAM · <BUILDING>` and `● REC`. One feed at a time; `f` again, Esc or a move (a step or a ride) closes it; it closes
  when the hold ends, when you leave the Net, or when you tap another camera.
- **Loop** blinds the camera (as Blind does) until the hold ends.
- **Eyes in the meat**: a street camera faces out from its building's facade and sees a
  cone 8 tiles deep and as wide as it is deep (each step out, one more each side), with
  a clear line. Standing in a live (not looped) camera's cone on the street:
  - you count as an eye: the header reads `bright · 1 eye`, cameras added to walkers;
  - while it sees you, its building's trace gains 2 heat a game minute (alarm and
    lockdown follow as from any heat);
  - its cone draws on the street tinted red while it sees you; a looped camera's cone
    draws dotted grey; a tapped camera's cone draws faint green while held.

### Ad board: hijack, decoy

- **Hijack** opens a picker: what the building's ads say, three lines:
  - smear: `<BUILDING> SELLS YOUR FACE`
  - lure: `FREE NOODLES · <BUILDING> · NOW`
  - tag: `<HANDLE> WAS HERE`
- Until the hold ends, every holo ad of the building (its ad board nodes' street tiles)
  draws magenta on the street, and when you are on the street within 12 tiles of one,
  it carries an aside labelled with your handle in magenta: `▸ <LINE>`.
- Walkers within 6 tiles of a hijacked ad stare at it: they don't count as eyes.
- The lure also sends walkers within 12 tiles of the ads, once when you choose it, to
  the pavement in front of the nearest ad.
- **Decoy** (inside a construct with standing ICE): for 4 beats the ad board's room
  floods (tinted magenta) and every standing piece goes to the ad board's tile, as a
  piece walks to where it last saw you, without hunting you: no `HUNTED`, no turns.
  When the beats run out they go back to their beats. Signal ghost marks their steps.

### Lamp grid: lights out

- **Lights out** darkens the district's street lamps for 20 game minutes: the lamps' light
  is dropped for every street tile of that district, so the picture, your exposure and
  everyone's sight fall with it. Neon, holo ads and windows stay lit. The lamps draw
  unlit. The edge is the district's border.
- It is one use per hold.

### Terminal: shell

- **Shell** opens the terminal screen on the node's host: a building's for a building's
  terminal, the district's for a street terminal. The same screen and login rules as on
  the street.

### Danger outside

#### Grid ICE

- ICE on the open Net is a new kind of piece, built on the street foes (they already
  see, move, fight on turns and draw on any stratum). A grid piece has a grade (white,
  grey, black), integrity as its hit points (by grade, as construct ICE), and a role:
  sentinel or tracer.
- It sees along lanes, as ICE sees along a spine: you on a lane or gate in its row or
  column, within its grade's sight, no wall between. Net walls block its sight.
- Awake, it hunts you: on turns (as a fight on foot), initiative by your Net, it steps
  toward you and hits you beside it by its grade, as construct ICE does (white: heat on
  its owner's trace; grey: focus; black: HP and the failsafe).
- Programs reach it as they reach construct ICE: attack takes integrity, crack lifts it;
  `t` cycles it as a target; the fire cursor picks it.
- On the open Net only pieces within 20 tiles count for a fight, so ICE across the city
  never joins.
- The header reads `grid · hunted` while one hunts you.

#### A · Tracers (always)

- When an owner's trace crosses its alarm line from a run outside, a **tracer** leaves
  the owner's source (the building's gate, the district's hub): a grid piece of the
  owner's grade (white below 200 security, grey below 230, black above).
- It follows your **trail**: the tiles you have stood on on the open Net since you came
  onto it, in order. It walks to the trail, then along it toward you, a tile every 4
  ticks (faster than walking, slower than riding).
- Beside you it latches: you are dumped (jacked out, cooling) and, on a rig run, the
  trace home starts. If it sees you along a lane on the way it hunts you as any grid
  ICE.
- Ways out: jack out first; outrun it (ride); **a hub** loses it (a trail that passes
  through a hub's ring stops there: the tracer reaches the hub and is gone); **mask**:
  while masked your steps are not added to the trail, so it runs out of trail and is
  gone; fight it.
- Leaving the open Net (into a construct or jacking out) clears your trail; a tracer
  waiting at a construct's gate is gone when its owner's alarm clears.

#### C · Sentinels (on)

- Each district keeps sentinels on its lanes, by its security: below 100 none; to 159
  one white; to 199 a white and a grey; to 229 two grey; from 230 two grey and a black.
- Each walks a beat: a straight lane run near its hub, there and back, asleep, seeing
  as above. Seeing you wakes it to hunt only while its district has you on trace (heat on
  the district's trace or on any building's in it, or an alarm there); a clean runner
  passes unbothered. (Jeffrey, after the first build: "ICE shouldn't see me as a threat
  on the open net" when there's no trace and nothing hunts you.)
- A district alarm, or an alarm of a building in it, reached from outside, wakes them
  all toward where you were.
- They appear when you first come onto the open Net in a session and stay for it.

#### B · Sweeps (the fallback)

- No sentinels. While a district's alert is up (its own, or a building in it, from
  outside), its hub pulses a ring every 2 beats that expands across the district at a
  tile every 2 ticks to 30 tiles (measured with a cell twice as tall as wide).
- A ring that crosses your tile adds 12 heat to the district's trace (so a dump follows
  in a few rings). Inside a construct the rings pass you by.
- Rings draw magenta, faint behind their edge.

#### The switch

- `Grid patrols` in Settings, a dev-only row (shown once dev is unlocked):
  `sentinels` (default) or `sweeps`. Tracers stay on either way.

## Calls made

- **Grid ICE is built on the street foes**, not on construct guards: guards need a
  construct's own tile grid and would leak into the saved city.
- **District traces sit beside building traces** rather than re-keying all heat by a
  subnet owner (about 145 call sites); one owner-keyed path feeds both.
- **Holds are separate from the existing hacks**: door locks and lifts keep their
  effects as today; holds are a new list keyed by node.
- **Tracers can be fought** like any grid ICE, as round 2 said.
- **Verb keys** `f o i z r` were free in walk mode; a key acts on the nearest held node.
- **Lights out lasts 20 game minutes**; the darkness is a session-only set of dark
  districts on the street stratum, never saved.
- **Lines for the hijack are in code** with the building's name and your handle; a
  flavor file can take them later.
- **Hubs are not hackable yet**; the lamp grid and street terminals are the district
  nodes you can hit.
- **Camera eyes** give 2 heat a game minute; the cone is 8 deep.

- **Tracers latch, whatever else**: beside you a tracer dumps you. Calm, it moves in real
  time; in a fight it takes turns as other grid ICE, and its "hit" is the latch, so you
  stop it at range.
- **Holds outlast alarms**: an alarm doesn't take back what you hold, nor does an alert
  ending; a loop is part of its hold (not a hack an alarm wipes).
- **Camera heat stops at the alarm line**: cameras can lock a building down, never dump
  you; and only you on foot are seen (a body jacked in at a port is not).
- **Verbs are calm-only**: not in a fight. A decoy draws asleep pieces off their beats
  while you go unseen; it never ends a fight.
- **A district alarm locks the district's street ports** for its alert, as a building's
  alarm locks the ports at its walls today.
- **Lights out is refused by day** (the lamps are off), not spent.
- **Cameras need light**: a camera sees you in its cone within 3 tiles, or further where
  your tile is lit, as walkers see (`sight::sees`); lights out blinds them at range.

### Calls made while building

- **On the grid even white ICE walks** (two tiles a turn; grey two, black three), unlike a
  construct's gate piece, so a white sentinel can chase.
- **A tracer through a gate waits there**: entering a construct clears your trail and
  every tracer makes for the gate you went in by; it latches if you come back out there,
  and goes when its owner's alarm clears. Jacking out sends tracers home.
- **The trail keeps 600 tiles**; a tile within 2 of a hub is marked, and a tracer that
  reaches a marked tile is lost there.
- **Sentinels' beats** are straight lane runs of 5 to 12 tiles within 10 of the hub,
  longest first, no two sharing a tile; a district with fewer runs than its grades
  keeps fewer sentinels. Woken by an alarm, a far sentinel walks toward where it had
  you in calm too, and loses you there.
- **Lights out lives in the Net's session state** (`NetState.lights_out`) and drives the
  street's `Stratum.dark`; the lamps' light drops by each tile's district.
- **Hijack picker keys** `s` smear, `l` lure, `t` tag. The lure asks every free walker
  within 12 (not one held by a story, not a spot worker) for a trip to the pavement in
  front of the nearest ad, keeping their goal. The line aside says again a while after
  it goes down.
- **The feed** sits off the node on the side the dock doesn't take, or the top right of
  the map when its node isn't on your map; it draws your own body white when it's in
  frame.
- **Grid pieces glow** in their grade's colour on their tile; the HERE panel and the
  fight only count pieces within 20 tiles.

## What's left to the builder

- How the owner-keyed heat path is shaped (an enum on new paths, the building map
  untouched).
- How grid pieces sit in `Foe` (a kind, a role) and how their hits reuse construct ICE's
  hit by grade.
- Where the trail is kept and how long (bounded; the tracer only needs it from its
  start).
- The feed's exact placement, within the tether's side rules.
- Tests for every rule above; shots: `net_held` (a dock), `net_feed`, `net_outside_run`,
  `street_eyes`, `street_hijack`, `street_dark`, `net_decoy`, `net_tracer`,
  `net_sentinels`, `net_sweep`.
