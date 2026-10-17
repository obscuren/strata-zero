# Terminals, access and the apartment: riff notes

Date: 2026-10-17. Status: **ideas, not a spec.** Written up from a riffing
session with Jeffrey so the next brainstorm starts from here. Nothing below is
decided unless it says so.

## Where we landed

- **The apartment comes first.** It is the player's own flat, and a new game
  starts there instead of on a random street connector.
- Terminals get two ways in: **[a] Jack in** (the Net, as today) and
  **[b] Access Terminal** (a typed, MUTHUR-style session on the machine
  itself).
- The terminal is for **scouting** and **operating the world**. Lore and mail
  matter mainly as a source of codes and credentials.
- **Typed commands first.** We can fall back to menus later if typing turns
  out to be too fiddly.
- Keypads on doors with `0–9`, `X` (clear) and `V` (verify).
- A proper **hardware screen**: a coloured device frame (beige plastic bezel,
  recessed CRT) around the terminal, and the same family for keypads.
- The **journal** (empty today) stores what the player knows and what they
  have planted.
- **Backdoors** come in two kinds: short-lived ones planted at a terminal and
  long-lived ones planted in the Net. Chaining backdoors is lateral movement,
  and Jeffrey loves it.

## What already exists

Checked while riffing, so the next session doesn't have to:

- **The Net is a city-wide stratum** (`2026-10-12-cyberspace-design.md`). Each
  building has a construct at the same spot. Jacking in at a street terminal
  drops the player's consciousness at that spot, and the body stays behind.
- **Interiors** (`2026-10-15-interiors-design.md`). Apartments and Megablocks
  are closed buildings with real floors. `RoomKind::Flat` rooms are locked,
  and bumping the door says whose it is. "Lockpicks and invitations come
  later."
- **City features** already include `Terminal`, `Camera`, `DoorLock`,
  `LiftCtrl` and Net nodes (terminal, server, backbone, data vault, ICE).
- **Spawn:** `Player::spawn` (`crates/strata-zero/src/walker/mod.rs`) drops
  the player on a random public street connector outside the corp core.
- **Journal:** the deck has a `JOURNAL` tab that only says "nothing written
  yet" (`crates/strata-zero/src/screens/deck.rs`).

## 1. The apartment (first)

The player's flat: a starting area and, later, a hub.

- A new game starts **inside the flat**, one of the locked `Flat` rooms in an
  Apartment or Megablock, which now belongs to the player. The door knows its
  owner, so this may be close to free.
- **The home rig:** the player's own computer, a hardware-framed terminal.
  - Access Terminal: review the journal on a big screen, maybe as a corkboard
    linking people, places and codes.
  - Jack in from home, through net backdoors (see §6).
  - Upgradeable: it starts as a battered second-hand beige box. Better cooling
    means longer runs, a better line means faster actions, and it can run its
    own ICE to defend against a corp hitting back.
- **The safe and dangerous room:** your body is safest here, but a completed
  trace from a home run leads to your door. That could mean a raid, a bounty,
  or fixers who stop calling.
- Natural home for later things too: stash, bed and clock, a door with your
  own keypad.

Open questions:
- What does the first minute look like? Wake up, rig boots, first message.
  The rig is a perfect tutorial device: an email from a fixer, a code to try.
- Which floor and building kind? Does the flat's location vary with the seed?
- Is the rig present from the start, or is it the first thing you earn?

## 2. Terminals: two ways in

Walking up to a terminal offers:

```
[a] Jack in
[b] Access Terminal
```

| | Jack in | Access Terminal |
|---|---|---|
| Speed | fast | slow (typing, turns pass) |
| Depth | deep (the whole Net) | shallow (this machine, its local subnet) |
| Risk | ICE, traces, body left behind | suspicion meter, lockout, someone walks by |
| Needs | a port | credentials for anything past GUEST |

### What an Access Terminal session can do

**Scouting**
- Floor plans: reveal the interior map.
- Camera feeds: a static, low-res look into rooms you haven't entered.
- Guard rosters and patrol schedules.
- System logs: who logged in, from where, whether someone got here first.
- Subnet scan: reveal the local part of the Net (constructs, nodes, which ICE
  sits where) before jacking in.
- Identify ICE: its grade and type, so you bring the right programs.

**Operating the world**
- Doors and shutters: lock or unlock. Call lifts (`LiftCtrl`).
- Lights off in a sector.
- Loop cameras for N turns.
- A fake alarm elsewhere to pull guards away.
- Turrets and drones to friendly (ADMIN only).
- Reroute power: kill security, or overload something on purpose.

**Net, the basic way**
- Plant a terminal backdoor (§5).
- Lower a node's security, softening its ICE.
- Download data in meatspace: a slow progress bar while patrols tick.

**Lore and mail** (supporting role): messages, personnel files. These are
mainly where codes and passwords come from.

### Friction

- Access tiers **GUEST → USER → ADMIN**. Credentials found in the world raise
  your tier.
- Three wrong passwords lock you out, or ping a sysadmin.
- Actions are logged and raise a local suspicion meter. Wiping the logs is an
  action of its own.

## 3. Access: credentials and locks

The thread that ties keycards, keypads, terminals and backdoors together.

A **credential** has:
- a **form:** physical (keycard, chip off a body) or known (code, password,
  backdoor)
- a **scope:** which locks it opens (one door, a sector, a terminal, a tier)
- maybe a **lifetime:** codes rotate at shift change, and a card gets revoked
  once its owner's body is found

A **lock** is anything that asks for a credential: door, keypad, terminal, ICE.
Some take two (card slot plus keypad is two-factor, which is very corp).

Credentials **chain**, and the chain is the gameplay:

> email on a terminal → keypad code → office with a keycard on the desk →
> USER login → door controls and subnet scan → where to jack in → in the Net,
> change the codes or plant an ADMIN backdoor → more terminal options next time

**Physical credentials are items** (inventory, droppable, stealable).
**Known credentials live in the journal:** codes, passwords, backdoors, and
who they belong to.

## 4. Hardware: screens and keypads

Diegetic device frames drawn in truecolor cells, the "cassette futurism" look
from *Alien*'s MUTHUR.

**Terminal frame**
- A beige plastic bezel with rounded corners in block characters. Half-blocks
  (`▀▄█`) shade it so it looks moulded: a light top edge, a dark bottom edge.
- A recessed screen, phosphor green or amber on near-black.
- A manufacturer badge, power LED, vent slots.
- Inside: a scanline tint on alternate rows, text typed out at a fixed rate, a
  short boot sequence the first time you power a machine on.

**Keypad frame**
- A small LCD of digit slots, `[4][4][_][_]`, and a status LED.
- Keys `1–9`, `X`, `0`, `V`. A wrong code buzzes and flashes red. After three
  wrong codes it locks for N turns or pings security.
- **Worn keys:** the most-pressed digits look faded, a hint for observant
  players without the code.
- Optional card slot next to the keys for two-factor doors.

**Variants:** the frame tells you where you are before you read a word. Beige
for municipal or old hardware, gunmetal for corp, sun-yellowed and cracked for
a back-alley clinic. Each corp runs its own OS with its own boot screen. The
home rig is the frame the player gets to upgrade.

## 5. Typed commands

Old-OS flavour, but forgiving:

```
> HELP
> MAIL
> DOOR 3 UNLOCK
> CAM LOOP 2 30
> NET SCAN
> BACKDOOR PLANT
ACCESS DENIED - CLEARANCE: USER  REQUIRED: ADMIN
```

- `HELP` lists what *this* terminal at *your* tier can do.
- Tab completion: period-wrong, but merciful.
- Clear errors in the period voice.
- Strange terminals: a session left open mid-task with a half-written message
  on screen. An AI that answers back is the MUTHUR moment.

## 6. Backdoors

| | Terminal backdoor | Net backdoor |
|---|---|---|
| Planted | at an Access Terminal, a few turns at the machine | while jacked in, deep in a node, past the ICE |
| Lifespan | minutes to hours of game time | days, or permanent |
| Reach | local: this building's systems and local subnet | reachable from anywhere you can jack in |
| Role | **tactics:** this job | **strategy:** a foothold you own |

Expiry ideas:
- **Fuzzy timer.** The journal shows `STABLE → DEGRADING → LIKELY BURNED`, not
  a countdown.
- **Use costs lifespan.** Every use is noise, so a quiet backdoor lasts
  longer.
- **Honeypots.** Sometimes a found backdoor is left in place as bait, and
  using it gets you traced.
- **Quality** comes from skill or program: a longer life, a quieter footprint.
  This gives talents something to hook into.

Pressure on permanent net backdoors:
- A **slot cap** on your deck or rig.
- **Corp counter-ops:** heavy use draws a hunt, which could become a job in
  itself (defend it or lose it).
- **Fixers trade them:** sell a foothold instead of using it.

The intended loop: a quick terminal backdoor lets you jack in from a safe
corner, and that run earns a permanent net backdoor.

## 7. Jacking in from anywhere: origin, route, chaining

A remote jack-in needs an **origin** (where your body sits) and a **route**
(a backdoor into the target).

| Origin | Body risk | Trace risk | Link |
|---|---|---|---|
| Terminal in the building | high: slumped at a desk | low: the trace ends at their own terminal | strong, local |
| Public booth / street terminal (today's ports) | medium | low: an anonymous dead end | weak, laggy |
| Portable deck, wireless | high | medium | weakest, short range |
| **Home rig** | none | **high: leads to your door** | best, upgradeable |

**Chaining is lateral movement.** Route through backdoors to stretch the
trace: home → backdoor A (city grid) → backdoor B (shell company) → target.
Each hop slows or confuses the trace. Burning a relay on purpose cuts the
trace dead, at the cost of the asset.

Since the Net is already one city-wide map, a backdoor could simply mean
**jacking in at that node's tile instead of at your own port**. From there
you hop to the next backdoor, so the chain is a series of jumps across the
Net map.

Reach rules (proposal):
- From home: any **net** backdoor.
- **Terminal** backdoors stay local unless you chain into that building
  through a net backdoor. This keeps the job loop and the home loop distinct.

## Open questions

- Credentials as items vs journal entries: proposed both (cards are items,
  codes are knowledge). Confirm.
- Game-time scale for terminal backdoors: does "minutes to hours" fit how
  fast the clock runs today?
- How does a trace actually play out against the home rig? A meter during the
  run, consequences after?
- Does the portable deck exist, or do you always need a physical port?
- Typed parser scope: a fixed verb list per terminal, or a general grammar?

## Suggested order

1. **The apartment:** start a new game in your own flat. The rig exists as
   an object even if it does little yet.
2. **Credentials and the journal:** codes and keycards as data; the journal
   lists what you know.
3. **Hardware frames:** keypad on doors (`0–9 X V`), terminal frame.
4. **Access Terminal:** typed commands, tiers, scouting and operating the
   world.
5. **Backdoors:** terminal first, then net, then chaining and remote jack-in
   from the rig.

Each step uses the one before. Step 1 can stay small, but it gives every later
step a place to start: the home rig is where the player first meets a
terminal, a journal entry and, eventually, a backdoor.
