# A site's own Net (#95)

A hotel's or an office's devices (its locks, lifts, cameras, terminals, turrets and safes)
become nodes on a Net of the building's own. That Net is a construct of several levels
that follows the building, one level for each floor. A netrunner gets onto it by jacking in
at a device on a floor, or by breaking the building's firewall from the city's Net. A node
on it works its own device: crack a door's node and that door is open on the floor.

The mock is `2026-11-11-site-net-mock/site-net.html`. Look D, the open tower, is the one
agreed. Looks A, B and C are kept below it as the round it came from.

## Why

Netrunners are weak in the open. In the street, their power over lamps, cameras, ads and
doors is how they move and how they do a job on foot. Now that sites have floors, zones,
locks and devices (#34), a netrunner needs the same power inside a building. Today they
have none of it:

- one crack opens every lock in a building at once;
- the construct has nothing to do with the floors;
- nothing on a floor is on the Net.

The aim is a way of playing where the Net and the street take turns. A netrunner opens a
door from the Net, then walks through it. They loop a camera, then slip past it. They
call a lift, then ride it down to a floor they can only reach in person.

## Decided

These are Jeff's rulings from the brainstorm and the mock rounds.

1. **A way in on a floor.** A device on a site's floor is a way onto its Net: a terminal,
   or the switch box of a line of turnstiles. You jack in there, and your body stays
   standing beside it.
2. **A firewall around it.** From the city's Net, a site is behind a firewall with
   several locks.
   - Each lock can be cracked from outside, on the grid: standing at it, from your rig or
     through a backdoor. It can also be cracked from inside the site's Net.
   - Every lock you break adds trace.
   - Break them all and the way through is open.
3. **A construct of several levels.** Inside, the building's Net has one level for each
   floor of the site. Each level is that floor's own plan.
4. **A node works its own device.** Cracking a door's node opens that door on the floor.
   It does not open every door in the building. Cameras, lifts, turrets and safes each
   have their own node.
5. **ICE patrols the levels.**
6. **The site makes the netrunner walk.**
   - Parts of a building's Net can be cut off from the rest: the vault's floor, or the
     security office. The only way in is to jack in on that floor.
   - From the Net you call a lift and loop the cameras on the way, then walk.
   - A safe cracked from the Net still has to be emptied by hand.
   - Some jobs are pure data, copied out from the chair.
7. **The look is D, the open tower.**
   - The building is drawn in section, as in look C, with every level stacked.
   - The level you're on is opened out to its full floor plan, as in look A.
   - The other levels are folded to three rows each.
   - Every level is drawn at the same width, so the lifts and the riser line up through
     the tower.
8. **Counts on screens are digits**: "FIREWALL · 3 locks", "0/3".

## What happens today

- **A building's Net.**
  - It is one subnet with one security number, made with the city
    (`citygen/src/generate/net.rs`, `building()`).
  - Its nodes stand on street tiles of its footprint:
    - a server, and a data vault in a corp tower or a clinic;
    - terminals and cameras on its face;
    - a door lock at each door to the street;
    - a lift control at each lift.
  - A node's id is its index in `city.net.nodes`. That id is stored in hacks, holds, the
    camera feed and the construct.
- **The construct.**
  - It is made from the building's footprint alone (`citygen/src/construct.rs`): a spine
    of rooms, with the server before the vault.
  - It is entered through the building's gate tile on the Net, cached per building, and
    saved.
  - ICE stands by security:
    - white at the gate from 160;
    - grey patrolling from 200;
    - a guard on the vault, black from 230.
- **What a hack does.**
  - `NetState::unlocked(b)` is any live crack or spoof on any of the building's door
    locks. It opens every Net lock, flat and lift stop in the building.
  - `lifts_answer(b)` makes the lifts answer in an alarm.
  - An alarm drops every hack on the building except a backdoor.
  - A door held by an alarm says "locked down" (`venues::locked_down`).
- **Cameras** are street cameras only (`cameras.rs`). Each one:
  - sees a cone eight tiles deep;
  - warms its building's trace by 2 a minute, but never past the alarm line;
  - can be looped or tapped once you hold it.
- **Jacking in** works only at the street's jack connectors and at the home rig. A floor
  offers no port (`walker::terminal_in_reach`).
- **Sites** (hotels and offices; the metro type isn't placed in the city yet):
  - Locks are `DoorLock`s with `Ways`.
  - A mission copy's layer places cameras, turrets, containers and guards. They are drawn,
    but none of them act.
  - No floor has a terminal.

## Goals

- Every site has its own Net, made from its plan. A mission copy also gets the devices its
  layer places.
- A node works only its own device.
- A netrunner can:
  - jack in on a floor;
  - break the firewall from outside;
  - land through a backdoor.
- The construct follows the building level by level, and is drawn as look D.
- The site sends the netrunner on foot:
  - parts of its Net are cut off from the riser (air gaps);
  - lifts and doors are opened from the Net;
  - cameras on floors watch, and looping them matters.
- Old saves keep opening.

## Not in this spec

| What | Where it goes |
|---|---|
| Guards who see, walk and shoot on floors, or find a body left jacked in | #33 |
| Alert states, noise, a site locking down floor by floor, the guards closing in | #36 |
| New verbs: seal a door, hold a lift, hacks on hubs and the backbone, scrub a server | #73 |
| A turret firing, being shut off or turned on the guards | #97 |
| What a container holds, and opening it with a key, the Keyjack or force | #96 |
| Objective kinds: an item to carry out, data to copy, someone to meet | #98 |
| Placing the metro in the city | the metro project |

So at the end of this spec, a site's Net can be broken into, walked and used to open the
building. The guards still don't move, and the turrets and safes are nodes whose
effects come later.

## What would make it a failure

- The site's Net looks nothing like the building, so a run teaches you nothing about the
  floors you'll walk.
- A netrunner can do everything from the chair, and never has to set foot on a floor.
- One crack still opens the whole building.
- A save from before this change won't open, or opens with a site's doors stuck.
- The tower can't be read at 100×30.

## The design

### The site's Net

Every site has a Net of its own, made from its plan:

- the plain building's plan;
- or, for a copy, the copy's plan and its mission layer.

It is worked out again whenever it is needed, never generated by chance. So the same plan
always gives the same Net, with the same nodes in the same order.

**Its nodes** come from:

| Node | Comes from | In the plain building |
|---|---|---|
| Door lock | every lock the Net can open today: `DoorLock`s with `Ways::NET` (card doors, keypads, locked street doors) and private rooms' keypads (the venue graph's `KEYPAD` way) | yes |
| Gate switch | one for each line of turnstiles, standing at one end of the line | yes |
| Lift | one for each lift, at its landing on the lowest level it serves that is on the riser | yes |
| Terminal | new, placed with the plan (below) | yes |
| Hub | one on each level, beside the riser | yes |
| Server | new, the building's host, beside G's hub (below) | yes |
| Data vault | a copy whose objective is in a server room: on the objective's spot; the plain building at security 230 or more: in its first server room | copies, or 230+ |
| Camera | the mission layer's cameras | copies |
| Turret | the mission layer's turrets | copies |
| Container lock | the mission layer's containers | copies |

A hotel floor of guest rooms has a node for every room's keypad: the count is honest,
and you crack only the one you need.

A node is known by its building and its place in the site's Net, never by an index into
`city.net.nodes`. Hacks, holds and the camera feed name it that way.

**Its links:**
- each device hangs off its level's hub;
- each hub hangs off the riser;
- the riser runs to the server, and from the server out through the firewall.

The **riser** follows the lift shaft that serves the most levels the site keeps. Each
hub stands on a floor tile beside that shaft. So the hubs are one above another, and in
the section they make one vertical line.

**An air gap** cuts off a whole level. A copy at security 160 or more cuts off its
objective's level, unless that level is G. That level's devices hang off its own hub, and
that hub isn't on the riser. Nothing else on the building's Net can reach those nodes,
and they can't reach anything else. The way in is a terminal on that level, in its
outermost zone near where its lifts and stairs arrive. So you can reach it on foot, then
work the level's own doors from its own Net. The plain building has no air gap.

**Terminals.** Each type's data names the rooms that get a terminal:
- every site has a public terminal at G (a lobby kiosk, or the station's ticket machine);
- some staff rooms have desk terminals;
- a gapped level always has one, as above.

Terminals are placed after the layout is laid, by a pass of their own with its own seed.
So placing them changes nothing the layout already laid: doors, locks, lock numbers and
codes all stay the same.

**The server** is the building's host: it stands beside G's hub, at the foot of the
riser where the building meets the street. G is never gapped, so the server is always on
the riser. The data a job is after sits in the data vault, which is where an air gap can
put it.

**Which security.** A site's security comes from a live copy's own `inputs.security`, or
from the building's subnet when no copy holds it. It sets the firewall's locks, the ICE,
the gap, and the odds and trace of runs on the site, through `owners::security`. When a
copy starts or ends, the site's Net is worked out again. The firewall's broken locks and
the hacks and holds on the site's nodes are dropped.

### The firewall

From the city's Net, a site is a building behind a firewall.

- **How many locks** goes by the site's security:

  | Security | Locks |
  |---|---|
  | under 100 | 1 |
  | 100–159 | 2 |
  | 160–229 | 3 |
  | 230 and up | 4 |

- **Where they are.** The locks stand on the building's edge on the Net, each on a tile
  of the open grid against the edge, spread around the building as far as they'll go. The
  building's gate tile is always one of them. Only the gate has a lane today; no new
  lanes are laid. They are worked out from the Net map as it is, not added when the city
  is made. So a city in an old save gets them too.
- **Cracking one.**
  - A lock is a target for a run, cracked standing beside it on the grid, as you crack a
    node beside you.
  - "From your rig or a backdoor" means where your body is. You jack in at home, or land
    at a backdoor nearby, and go to the lock on the grid: your body stays safe, but
    every break is heard. There is no new way to target a lock from afar.
  - Inside the site's Net, the locks stand on G's outer wall, and can be cracked from
    there.
  - Each landed crack adds trace to the building, as a crack does today. A miss adds
    more.
  - A broken lock stays broken for the hack's hour, or until an alarm. An alarm closes
    them all.
- **Through it.** While every lock is broken, the gate lets you through, both ways:
  - into the site's Net, landing on G's hub, beside the server;
  - and out again onto the grid.

  With a lock still whole, the gate says "FIREWALL · 3 locks · 1/3 broken".

Jacking out always works, wherever you are. The firewall only blocks crossing between the
site's Net and the city's.

### Ways in

1. **Jack in on a floor.** Stand beside a terminal or a gate switch, and jack in with your
   deck as you do at a street port. You start on that level, on that device's node.
   - Your body stays standing on the floor beside it.
   - The run's cord runs from the node back to it.
   - The device's zone doesn't matter: a public kiosk, a staff desk and the terminal in an
     air-gapped zone all work.
   - An alarm locks the building's ports, as it does today.
2. **Through the firewall**, from the grid, as above.
3. **A backdoor on the server.** A backdoor planted on a site's server lands you on G's
   hub, past the firewall, as a backdoor lands you in a construct today.

A camera is not a way in: it is out of reach on the ceiling. A device you already hold
is not a way in either. Both are left for later.

### The construct

The construct has one level for each floor the site keeps. Each level is that floor's
plan, tile for tile:

- walls are Net walls, and the floor is Net floor;
- a door with no lock is an opening;
- a door with a lock node is that node.
  - It stops you on the Net until it's cracked, except from its free side, as on the
    floor.
  - So the zones on the Net are the zones on the floor.
- a door whose lock has no Net way is a wall on the Net;
- the floor is drawn tile for tile, at the floor's own size (floors are their
  footprint × 3). The level scrolls with you as a floor does;
- each device stands as a node on its own tile. A device on a wall (a gate switch, a
  camera) stands on the floor tile in front of it;
- the links from devices to their hub are drawn as thin traces. They are only drawn, and
  you don't walk them.
- the hub is the way between levels:
  - stand on it and press `<` or `>` to go down or up the riser to the next kept level's
    hub;
  - an air-gapped hub has no riser, so `<` and `>` say so.

**ICE, by security** (my call, close to today's rules):

| Security | ICE |
|---|---|
| under 100 | none |
| 100–159 | 1 white at G's hub |
| 160–199 | 1 white at G's hub; 1 white patrolling every other kept level |
| 200–229 | grey patrolling every kept level, as well as the white at G's hub |
| 230 and up | as at 200, and black on the data vault |

A patrol walks a beat through the level's rooms. Each level's ICE is the engine's ICE as
it works today: seeing, waking, turns and engagement. It only works on the level it's on.

**A new place.** The player on a site's Net is at `Place::SiteNet { building, level }`.
Every other building keeps its old construct and `Place::Construct`.

### What a node does

| Node | Crack or spoof lands | What changes |
|---|---|---|
| Door lock | held for the hack's hour | that door opens on the floor and on the Net; "cracked" in its look |
| Gate switch | held | the whole line of turnstiles lets you through |
| Lift | spoof held | the lift stops at every floor it serves with no card, and answers in an alarm |
| Camera | held, with Tap and Loop | as a street camera: its feed, or blind while looped |
| Terminal | held, with Shell | HELIX on the site's host, as at a street terminal |
| Server | Crack: root on the host. Backdoor and Copy as today | as today, for this building |
| Data vault | Copy | as today |
| Turret | Crack lands and holds | nothing yet (#97) |
| Container lock | Crack lands and holds | its lock is open, for #96 to use |
| Hub | none of today's programs apply | (#73) |

**Per device, not per building.** For a site, `unlocked(b)` stops being a test of the
whole building:
- a lock asks for a hack on its own node;
- a lift stop asks for a spoof on its own lift.

A street door's lock on the city's Net (today's door-lock node at a door to the street)
opens only its own door.

**What's inside the firewall.** For a site, the city's server, vault and lift-control
nodes are inside the firewall now, so you can't target them from the grid. Its cameras
and terminals on its face stay on the city's Net, as part of the street.

**HELIX on a site.** Today, `ssh` from any street terminal into a building's host with
User standing runs building-wide commands (`door release`, `lift answer`, `cam loop`,
`alarm off` in `access.rs` and `helix/bin.rs`). On a site, that would get around the
firewall and open everything from the chair. So on a site:
- `ssh` into its host from outside the site's Net is refused while any firewall lock is
  whole: "FIREWALL · 3 LOCKS". From a terminal on the site's own Net, or with the
  firewall broken, it connects as today.
- The four building-wide commands need root on a site, which means a cracked server.
  With root they act on every device of their kind on the riser's levels, but not across
  an air gap.
- The rest of HELIX (mail, files, plans, the user commands) works as today.

**An alarm** works as today, on the site's trace:
- it drops every hack on the site's nodes and every broken firewall lock;
- a backdoor holds;
- doors relock, and say "locked down".

### Cameras on floors

A copy's cameras watch their floors the way street cameras watch the street:
- each sees a cone, `CONE_DEPTH` deep, from the wall it's on, and needs light as a street
  camera does;
- each warms the site's trace by `EYE_HEAT` a minute, up to the alarm line;
- a looped or blinded camera sees nothing.

This is the one thing on a floor that reacts to being seen before #36.

### The body while jacked in

While jacked in on a floor, your body stands at the device, and the panel says where it
is and who is near it. Nothing happens to the body yet. A guard who finds it comes with
#33 and #36.

### The look: the open tower

These are drawn as mock D, on the map of a site's Net:

- **The tower.** Every level the site keeps is stacked top to bottom, at the same width
  and the same x. The level you're on is open at its full plan. Every other level is
  folded to three rows:
  - its back rooms;
  - the wall with its doors;
  - its front.

  A folded level still shows its nodes, its ICE and what you hold there.
- **Between levels.**
  - A row of slab sits between each pair of levels.
  - Floors the lift passes are folded into that row with a label: "8F–11F passed".
  - The lift shafts and the riser run through the folds.
- **The ground.**
  - The street line runs across at G.
  - Levels under it sit in the earth.
  - The riser is cut, `╳`, where an air gap leaves a level off it.
- **The firewall** is a red double line around every level on the riser, above and
  under the street. A gapped level sits outside it, in the earth or above.
  - Its locks are `▮` on G's outer wall, and green `▯` when broken.
  - "FIREWALL · 3 locks" is written on its top edge.
- **A level you can't reach** from where you stand (across an air gap) is dimmed. Its
  label on the right says "out of reach".
- **On the left of each level**: its name. For the open level also:
  - its node count ("11 ▪");
  - how many you hold ("4 held").
- **The panel.**
  - **The building**: its name, its security, and "FIREWALL ▮▮▮ 0/3".
  - **The job's line.**
  - **LEVELS**: one row a level, with its node count, its ICE, and marks for the server,
    the vault and you. An unreachable level says "out of reach", or "AIR GAP".
  - **HELD**: what you hold and what each does.
  - **BODY**: where your body stands, and who is near it.
- **Colours** are the Net's own:
  - green for what you hold;
  - ICE's colours by grade;
  - the firewall red;
  - an air-gapped level's floor tinted red.
- **Scrolling.** Levels are drawn tile for tile, so a tower is usually bigger than the
  map. It scrolls with you both ways, as the street does, and the folded levels and the
  firewall scroll with it. Don't promise that a whole tower fits on the map; the mock's
  floors are smaller than a real site's.
- **On the city's Net**, a site's building carries its locks on its edge, `▮`, or `▯` once
  broken. Its card says "FIREWALL ▯▮▮ · 1/3 broken".

### The plain building and the copy

Both have a site Net. The plain building's has no cameras, turrets or containers, and no
air gap. Its ICE still comes from its security.

A copy's Net is made from the copy. When the copy ends, its hacks and holds on the
copy's devices go with it.

### Saving

- **The site's Net** is worked out from what's saved (the plan, and the copy's layer), so
  it isn't saved itself. If it's cached, `after_restore` remakes the cache (saving.md
  rule #4).
- **The site's construct is saved**, as constructs are today: its ICE, where it stands,
  and what's lifted. It saves per level.
- **New fields**, each `#[serde(default)]`:
  - the firewall's broken locks, in `NetState`;
  - hacks and holds that name a site node. The new `HackTarget` variant is added at the
    end of the enum.
  - `Place::SiteNet`, added at the end of `Place`.
- **Old saves.**
  - A hack or a hold from before names a city node, and keeps working as it did.
  - A save whose player stands in a site's old construct still opens. The builder picks
    between two ways:
    - let the old construct carry on until the player leaves it;
    - put the player out on the grid at the building's gate.
- **Fixtures.** Saves in `runs/fixtures/` must keep opening.
- **`docs/saving.md`** gets a section on the site's Net.

### The manual

These chapters change in the same branch:
- **The Net**:
  - jacking in on a floor;
  - the firewall and its locks;
  - the building's Net and its look;
  - the riser and air gaps;
  - nodes that work one device each.
- **The city and the world**, under "Hotels and offices": the Net opens the door whose
  lock you cracked, not every door in the building.

### Checking

Tests, beside the code they test:

- **The Net from the plan.**
  - The same plan gives the same Net, every time.
  - Every Net lock, lift and terminal has a node.
  - Every hub is beside the riser's shaft.
  - An air gap's devices can't reach the riser.
- **Terminals.**
  - Placing them changes no door, lock or code of an existing plan.
  - Every site has a public terminal at G.
  - Every air gap has a terminal inside it.
- **Per device.**
  - Cracking one door's node opens that door and no other.
  - A lift's spoof frees that lift's stops and no other lift's.
- **The firewall.**
  - The lock count by security, as in the table.
  - The gate refuses while any lock is whole, and lets you through once all are broken.
  - An alarm closes them all.
  - A crack from inside counts the same as one from outside.
- **Ways in.**
  - Jacking in at a kiosk lands on its node.
  - A backdoor lands on G's hub.
  - In an air gap, the levels across it can't be reached.
- **Cameras on floors** warm the trace, and stop when looped.
- **Saves.**
  - A round trip keeps the construct's ICE and the firewall's state.
  - The fixtures open.
- **Shots** of the open tower at 120×40 and 100×30, from a DEVELOPER start in an office
  copy whose objective is the server room:
  - on L;
  - on the vault's level across an air gap;
  - one through the firewall;
  - the building on the city's Net with a lock broken.
- **HELIX.** `ssh` to a site's host from the street is refused while its firewall
  stands, and a building-wide command needs root.

## Calls I made

1. Terminals and gate switches are the ways in on a floor. Cameras and devices already held
   are not.
2. The lock count by security: 1, 2, 3 and 4 locks at 0, 100, 160 and 230.
3. The locks are worked out from the city's Net map, not made with the city, so old
   cities get them too. They stand on open-grid tiles against the building's edge, and no
   new lanes are laid.
4. A lock is cracked standing beside it. "From your rig or a backdoor" means your body is
   safe there while you walk the grid; there's no remote targeting.
5. The server, the building's host, stands beside G's hub. Through the firewall, or by a
   backdoor, you land on G's hub. From inside, the locks stand on G's outer wall.
6. The riser follows the lift shaft that serves the most kept levels, and each hub stands
   beside it.
7. An air gap is a whole level: a copy's objective level, at security 160 or more, unless
   it is G. Its terminal is in its outermost zone, near where its lifts and stairs arrive.
   The plain building has none.
8. The data vault is on a copy's objective when that's a server room, and in the plain
   building's first server room at security 230 or more.
9. Every lock the Net can open today is a node, private rooms' keypads included.
10. ICE by security, as in the table, close to today's thresholds. Patrols walk the
    level's rooms.
11. Locked doors stop you on the Net, except from their free side, as on the floor.
12. Levels are drawn tile for tile, and the tower scrolls both ways.
13. A live copy's security governs its site's Net; otherwise the subnet's does. When a
    copy starts or ends, the site's hacks, holds and broken locks are dropped.
14. A site's city-Net server, vault and lift nodes are inside the firewall now. Its face's
    cameras and terminals stay on the street's Net.
15. HELIX on a site: `ssh` from outside is refused while the firewall stands, and the four
    building-wide commands need root.
16. Floor cameras watch and warm the trace now, ahead of #36.
17. A turret, a container lock and a hub are nodes now. What they do comes with #97, #96
    and #73.

## Left to the builder

- How the site's Net is cached, and how its nodes are numbered, as long as the same plan
  gives the same order.
- Which tiles the firewall's locks take on the Net map, within the rule above.
- The ICE's beats on each level.
- How the open tower is drawn under the hood. The renderer can make the stack up from
  levels and folds.
- What a save inside a site's old construct does (above).
- The types' data for terminal rooms (`flavor/sites.ron`).
- Where exactly the hub and the server stand beside the shaft, as long as the hubs line
  up level to level.

## What done looks like

In a DEVELOPER start in an office copy at security 190, whose objective is the server
room on the server basement:

1. Jack in at the lobby kiosk. The open tower shows with L open, and you're on the
   kiosk's node.
2. Crack the turnstiles' switch, then jack out: the turnstiles let you through on foot.
3. From the grid, crack the 3 firewall locks one by one. Each adds trace, and once all 3
   are broken the gate lets you onto L's hub, beside the server.
4. The server basement is across an air gap, with the data vault on it. From the riser
   it's dimmed and out of reach. Spoof the goods lift from L, ride it down, and jack in
   at the basement's terminal: that level opens, and you can work its doors and copy the
   vault.
5. On a floor, a camera that sees you warms the trace. Looped, it doesn't.

The checks pass on both front ends, the shots are written at 120×40 and 100×30, and the
manual says all of the above.

## After this spec

- #33 guards on floors: they can find the body you left.
- #36 the alert: a site locking down floor by floor.
- #73 verbs on site devices: seal, hold, hubs.
- #96 containers and #97 turrets.
- #98 objectives: data to copy out from behind an air gap.
