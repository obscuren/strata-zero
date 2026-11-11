# Rig upgrades: the rack, its sleds, and jobs while you sleep

Issue #74, its first part (rig upgrades). Selling backdoors to fixers and corp
counter-ops stay on the issue.

## Intent

The rig at home has been the same battered box since the first game: it logs you in,
holds three backdoor lines, keeps a backdoor three days and gets seized when a trace
follows you home. Since the device-themes design (2026-10-17) it has been "the frame the
player gets to upgrade". This design makes it that.

The rig becomes **a rack of sleds**. Each sled is a part you find, earn or build, and
fitting a better one changes what the rig does for you: how many backdoors it keeps, how
long they hold, how a trace home goes, and how many **jobs** it can run while you're out.
The jobs are HELIX programs on the rig that work on game time: they sniff logins off a
backdoor, sift a paydata shard for leads, keep a backdoor alive, or rent out the rig's
spare cycles. In the morning a mail from `cron` says what they caught.

Mockups: `2026-11-11-rig-upgrades-mock/` (`gen.py`; `python3 -I gen.py 1` and
`gen.py 2` write `round1.html` and `round2.html`, drawn over the shot frames in
`shots/`). Round 1 holds the three ways to fit parts, the jobs in HELIX and the overnight
mail; round 2 the three drawings of the rig.

## What Jeff decided

- From three pitches (parts, jobs while you sleep, defending your door) Jeff took the
  first two, **the parts including the firewall**. Defending the door proper (corp
  counter-ops) stays for later; the firewall sled brings in part of it.
- **Parts are fitted at the desk (round 1, B)**: sitting at the rig turns the map into
  the desk, as the ripperdoc's chair does for your body. The ports on the left, the
  machine in the middle, the part and what could replace it on the right.
- **The rig is drawn as the rack (round 2, F)**: blade sleds in a bent half-rack on a
  milk crate, a head unit with a little green LCD, each sled with a pull handle and
  lamps in its kind's colour, the loom running off to the wall. Not the round-one wedge
  ("too much C64"), not the bare board or the slab.
- **The rack is the chassis.** Its frame decides which bays you have; a bay with nothing
  in it shows a blanking plate stamped with its kind; the head unit gives the look and
  voice. Better racks: a Civitas cabinet (adds a firewall bay), a Lotus tower (a second
  core), a Halcyon server frame (everything). Swapping the frame moves the sleds over; a
  sled with no bay on the new frame goes to your pack.
- **The jobs** as played back and agreed: `run`, `ps`, `kill`, rig only; seats from the
  core; `sniff`, `sift`, `keepalive`, `rent`; noise on the target until its admins patch
  the hole; `sift` and `rent` come with the rig, `sniff` and `keepalive` are found;
  jobs run on game time wherever you are; a 06:00 mail from `cron`; the boot says how
  many ran; a seized rig kills them.
- **The parts** as played back and agreed: sleds are pack items (found, built, paid by
  fixers), racks arrive at your door; the tiers and the firewall ladder below.
- Two slices, both built on one branch: (1) the rack and sleds, (2) the jobs.

## Goals

- The rig visibly grows: a better rack looks different and has more bays, a better sled
  says what it changes before you fit it.
- Every number a sled changes is data on the sled.
- The rig does work while the player is away or asleep, and says so when they're back.
- A runner who isn't a Techie still gets parts (from fixers); a Techie can build them.

## Non-goals

- Selling backdoors to fixers; corp counter-ops (the rest of #74).
- Shops and fences (#45): no buying parts; racks and sleds come as rewards and builds.
- Loot in server rooms (containers are #96): later, sleds join that loot.
- Upgrading the flat itself, or the deck.
- Running jobs from anywhere but the rig.

## Anti-goals

- A sled whose effect the player can't see: every sled reads its change on the desk.
- A rig that only works in the window: the desk is ratatui cells like every screen.
- A job that silently does nothing: a job with nothing left to do ends and says why.
- Breaking old saves: a run saved before this reads as the stock rack with stock sleds.

## 1. The model

### The rack and its bays

- **Four new socket kinds** join `SocketKind` at its end: `Core`, `Relay`, `Line`,
  `Firewall`. Their labels: `CORE`, `RELAY`, `LINE`, `FIREWALL`. None needs a doc.
- **A rack is an item** of a new `ItemKind::Frame` (in code "frame", because "Rack"
  already names GEAR & INVENTORY's worn-gear pane; on screen it's "rack"). A frame's
  `sockets` are its bays, in the order the desk draws them top to bottom. Validation: a
  frame's bays are only the four rig kinds and hold at least one each of `Core`,
  `Relay` and `Line`; a frame has no `slot` and is never carried (it weighs nothing and
  never enters the pack); the rig kinds fit only frames.
- The racks (`flavor/content.ron`), each with its head unit's look (a device theme of
  `flavor/style.ron`) and my values:

| id | name | bays | head unit | value |
|---|---|---|---|---|
| `rack_zenbo` | Zenbo half-rack | Core, Relay, Line | street | 200 |
| `rack_civitas` | Civitas cabinet | Core, Relay, Line, Firewall | municipal | 1200 |
| `rack_lotus` | Lotus tower | Core, Core, Relay, Line | neon | 1800 |
| `rack_halcyon` | Halcyon server frame | Core, Core, Relay, Line, Firewall | megacorp | 3500 |

  A frame names its device theme in content (a new optional `theme` on the item, or on a
  small `FrameSpec`, the builder's choice). The rig's terminal wears its frame's theme
  from then on (today it is always `street`): a Halcyon's HELIX boots white and says
  `INTEGRITY VERIFIED`, and its badge reads the theme's maker and model as every
  themed terminal's does (`HALCYON NODE 9`). **The rig's name stays `zenbo`** whatever
  the rack: it's your machine's hostname, so the prompt (`you@zenbo`), `cron@zenbo`, the
  route line (`zenbo ▸ …`), the motd and every test that reads them are unchanged. (My
  call, from the builder check.)

### The sleds

- **A sled is `Hardware`** fitting one of the four kinds, so the existing `Fit`/`Unfit`
  actions, the per-copy machinery and the tray all work on it. Its effect is a hardware
  mod; new `Mod` variants, their card text from their numbers as the other mods':
  - `Seats(n)`: jobs at once, read `seats n`.
  - `Speed(pct)`: job speed, read `jobs ×1.5` (150).
  - `Lines(n)`: backdoor lines, read `lines n`.
  - `Hold(days)`: a planted Net backdoor's life, read `holds n days`.
  - `Firewall(tier)`: 1 to 3, read `sandbag`, `tripwire`, `killswitch` (§4).
- The sleds, my names and values:

| id | name | bay | mods | value |
|---|---|---|---|---|
| `sled_zb8` | ZB-8 core | Core | Seats(1) | 60 |
| `sled_tinman` | Tinman 2X core | Core | Seats(2) | 640 |
| `sled_qstack` | Q-Stack 4 core | Core | Seats(3), Speed(150) | 1600 |
| `sled_r3` | R-3 relay | Relay | Lines(3) | 60 |
| `sled_switchback` | Switchback relay | Relay | Lines(5) | 500 |
| `sled_hydra` | Hydra relay | Relay | Lines(8) | 1400 |
| `sled_cu1` | CU-1 copper line | Line | Hold(3) | 60 |
| `sled_longwire` | Longwire | Line | Hold(5) | 450 |
| `sled_darkfibre` | Darkfibre | Line | Hold(7) | 1300 |
| `sled_sandbag` | Sandbag firewall | Firewall | Firewall(1) | 500 |
| `sled_tripwire` | Tripwire firewall | Firewall | Firewall(2) | 1100 |
| `sled_killswitch` | Killswitch firewall | Firewall | Firewall(3) | 2000 |

  Weight 0.4 kg each. Descriptions in the item voice, one line each, e.g. the Tinman:
  *"A second-hand core sled with two seats; the fan rattles at night."*
- **The drawing.** One new hardware look, the sled's faceplate as the mock draws it (a
  dark plate, a pull handle, vents, a row of lamps), tinted per bay kind: CORE cyan
  (`#00e5ff`), RELAY green (`#50e678`), LINE amber (`#ffb000`), FIREWALL red
  (`#ff3a5a`), with the tier in the lamps (one, two or three lit pairs). It has the large
  Inspect picture, the tray swatch and the fitted drawing every hardware look has.
  `HardwareLook::kind()` is per bay kind, so this is four looks (`SledCore`, `SledRelay`,
  `SledLine`, `SledFirewall`) drawn by one function.
- **Racks get no blueprint drawing** of their own: a frame is never in the pack, so
  nothing opens its blueprint. The desk draws the rack (§2). Content validation's rule
  "an item with sockets has a drawing that places them" makes an exception for frames.

### Where the rig lives

- `Character` gains `rig: Option<ItemCopy>` (the fitted frame: its copy holds the sleds
  in `fitted`, as a worn deck's does) and `frames: Vec<ItemId>` (the racks standing in
  your flat, the fitted one among them). Both `#[serde(default)]`. The runner owns the
  rig, so it's in `Character`, not in a city's state; it follows no one into another
  city, but the desk only opens at home anyway.
- **`Host::Rig`** joins `action::Host`. `host_item(Host::Rig)` is the fitted frame,
  `host_copy`/`host_copy_mut` the rig's copy. `Fit`/`Unfit` on it work as on any host.
  Fitting or pulling a rig sled is refused anywhere but at the desk (the desk is the only
  screen that issues them).
- **The stock rig.** A character whose `rig` is `None` has the stock rig: the Zenbo
  half-rack with ZB-8, R-3 and CU-1 fitted. `Character::rig_copy_mut` makes it the first
  time it's needed (as `host_copy_mut` makes a worn copy), and `frames` gets
  `rack_zenbo`. So every new game and every old save starts with the stock rig, and no
  save code is needed beyond the defaults.
- **What the rig gives** is read from the fitted sleds, through one function
  `rig::spec(&Character, &Content) -> RigSpec { seats, speed, lines, hold_days,
  firewall }`, summing like kinds (a Lotus with two cores adds their seats; two speeds
  take the best). An empty bay gives nothing; with no relay you hold 0 lines and can't
  plant, and the desk says so. The stock rig's spec is exactly today's numbers (1 seat,
  3 lines, 3 days, no firewall), so nothing changes until you fit something.

## 2. The desk (round 1, B, with round 2's rack)

- **Getting there.** The rig's Space menu gains `o Open up the rig` (beside `i`
  Terminal and `j` Jack in; `o` leaves `FREE_LETTERS` so the rig's reads can't take it).
  It opens the desk. Refused, greyed with its reason, while the rig is seized
  (`· held as evidence`). It is a new `Pick::Desk` and a new screen
  `screens/desk.rs`, pushed over the map.
- **The screen**, laid out as `round2.html` F over the 120×40 frame:
  - The map pane becomes the desk: a dark wall, the rack drawn in the middle on its
    crate (or the frame's own base), the desk's edge under it. The top band and the
    side panel's frame stay.
  - The header row: the rack's name in amber bold, then `· your rig · n bays`.
  - Left, `BAYS`: a `FRAME` row first (the rack itself), then one row per bay, top to
    bottom: its kind and the fitted sled's short model (`ZB-8`), or `empty`. The picked
    row is highlighted, and its sled (or blanking plate, or for FRAME the whole rack) is
    outlined in the drawing with a dashed amber box.
  - Right, in the side panel: the picked part's card (name, what it gives, `came with
    the box` for a stock sled), then `FITS HERE · IN YOUR PACK` with every sled in the
    pack that fits this bay, each with the change it would make (`seats 1 ▸ 2`), the
    highlighted one first. For FRAME: `IN YOUR FLAT`, the racks in `frames`, each with
    its bays.
  - The bottom status row: `at your rig · ↑↓ bays · space what fits · x pull · esc
    stand up`.
- **The rack drawing** is made in code from the frame and its fitted sleds, as the mock
  draws F: rails, a head unit with the frame's LCD text (`ZENBO 64  REDY` for the
  Zenbo; each frame's head line is content), one sled per bay with its handle, vents and
  lamps in its kind's colour and its kind and model labelled on the plate, a blanking
  plate (`── CORE ──`, dim) for an empty bay, the frame's base (the Zenbo's milk crate;
  a cabinet plinth, a tower foot, a server sled's rails for the others: the builder
  draws them in the mock's manner) and the loom to the wall. It fits the map pane at
  100×30 as at 120×40 (fewer rows of art at 100×30; the sleds shrink to one cell row of
  plate before any bay is dropped).
- **Keys**, as the chair's:

| Key | What it does |
|---|---|
| `↑↓` | move between the frame and the bays |
| `space` | what fits here; in the list, fit it (a swap puts the old sled in your pack) |
| `x` | pull the sled into your pack (no asking: it's your own machine) |
| `esc` | in the list, back; else stand up |

- **Fitting takes time**: each fit or pull runs the clock ten minutes (my call),
  shown as the log line (`you seat the Tinman 2X · 10m`). It costs nothing.
- **Fitting stops jobs that lose their seat** (§5): pulling a core, or swapping to
  fewer seats, kills the newest jobs over the new seat count, and the log says which.
- **Changing the frame** (space on FRAME, then a rack from your flat): the sleds move to
  the new frame's bays of their kind, in order; a sled with no bay left goes to your
  pack, and the log lists them. The old frame stays in `frames`.
- **Weight.** A sled pulled into a full pack still goes in (as a swap's old piece does
  today); the carry bar shows the load as ever.

## 3. What the sleds change

All through `RigSpec`; the numbers in `backdoors.rs` become its defaults.

- **Lines.** `backdoors::SLOTS` goes; `can_plant` takes the rig's line count. The
  refusal reads `the rig holds {n} lines · burn one first`; with no relay fitted,
  `no relay in the rig · no lines`. Every place that prints `of SLOTS` prints the rig's
  count (`net.rs` landing line, `access.rs` `LINES n of m`, the `backdoor` man page,
  the manual). Lowering lines below the backdoors held burns nothing: you just can't
  plant until you're under.
- **Hold.** `plant_net` takes the rig's hold: a Net backdoor's nominal life is
  `hold_days` days (doubled at rating ≥ 2, as today). A key backdoor's life is
  unchanged. Backdoors already planted keep the life they were planted with.
- **Firewall**, each tier keeping what the one below does:
  1. **Sandbag**: the trace's last leg (`LAST_LEG`, the nearest relay or the rig to your
     door) takes twice as long.
  2. **Tripwire**: when a home trace starts, your phone rings: from `zenbo`, your rig
     (my call; the face falls back to the silhouette as an unknown caller's does), lines `trace on your line · {n} hops out` and `burn the route
     or ride it out`. It rings wherever you are, as a call does; let it ring and it's
     voicemail.
  3. **Killswitch**: when the trace reaches your door, the rig wipes itself: every
     backdoor burns as a raid burns them, but nothing is seized, the fixers don't go
     quiet, and the story line reads `the rig wiped itself before they got the door
     open · every backdoor is gone`. No `RigReturned` mail follows.
- `Backdoors` stays pure over minutes: the functions that need the rig take it as an
  argument (`can_plant(b, now, lines)`, `plant_net(…, hold_days)`, `start_trace(now,
  firewall)`, `minute(now, firewall)`), and the callers in `net.rs`, `helix/bin.rs` and
  `access.rs` pass `rig::spec`. The tripwire's call is made by the caller from the
  event `minute` returns (a new `Event::TraceStarted { hops }` or the caller of
  `start_trace`; the builder's choice).

## 4. Getting parts

- **The stock rig** comes with every game (§1).
- **The workbench** (skill `electronics`, as the Keyjack's), from parts in the game;
  the top tier eats a data vault's dump, so a netrunner's haul becomes hardware:

| sled | inputs (my calls) |
|---|---|
| Tinman 2X | `scrap_servo` 1, `copper_coil` 2 |
| Switchback | `copper_coil` 2, `dead_phone` 1 |
| Longwire | `copper_coil` 3 |
| Sandbag | `scrap_servo` 1, `dead_phone` 1 |
| Q-Stack 4 | `vault_data` 1, `scrap_servo` 2 |
| Hydra | `vault_data` 1, `copper_coil` 2 |
| Darkfibre | `vault_data` 1, `copper_coil` 3 |
| Tripwire | `vault_data` 1, `dead_phone` 2 |
| Killswitch | `vault_data` 2, `scrap_servo` 1 |

  Each recipe carries the two-to-four-line `steps` recipes have.
- **Fixers**, as standing rises (hand-in is where standing changes):
  - A fixer who comes to **know** you pays a tier-two sled with that hand-in, once per
    fixer: Tinman, Switchback, Longwire, Sandbag in turn by how many fixers have paid
    one before. The hand-in line says so (`· a Tinman 2X core`), as a Fetch's fuse.
  - **These rewards count across the run**, not per city (my call, from the builder
    check): `Character` gains `rig_rewards` (`#[serde(default)]`): the fixers who have
    paid a sled, by name, the sleds paid, the racks sent and whether the Halcyon has
    been. A fixer pays once ever: standing that drops and climbs back pays nothing
    again, and a new city's fixers carry on the same count.
  - The first fixer to **trust** you sends the **Civitas cabinet**; the second, the
    **Lotus tower**; the first to take you into the **inner circle**, the **Halcyon
    server frame**. A rack arrives at your flat: it joins `frames`, and a mail from the
    fixer says so (`it's at your door`, in their voice, from a short list in content).
    A runner away from home gets it all the same: it's in the flat when they're back.
  - The first finished **Plant** pays the `sniff` program shard; the first finished
    **Copy** (server or vault) pays `keepalive` (§5). The hand-in line says so.
- Tier-three sleds come only from the bench, so a non-Techie's ceiling is tier two plus
  the racks. My call; it keeps the bench worth having.

## 5. Jobs while you sleep (slice two)

### Commands (rig only)

| Command | What it does |
|---|---|
| `run <job> [target]` | start a job in a free seat |
| `ps` | the jobs running and done since you last looked, and the seats |
| `kill <n>` | stop job `n` |

Elsewhere each says `{cmd}: rig only`. They're HELIX programs on the rig (`bin.rs`
`program`, `programs()`), with man pages (the shell's test wants one per program) and
lines in `help`. `ps` lists `#  JOB  ON  UP  FOUND`, then `{core} · {busy} of {seats}
seats busy`, as the mock draws it.

### The jobs

Every job works in **hours of game time**, divided by the rig's speed. Each job keeps
the minute it started and the minute it was last advanced, so time it missed (asleep,
away in another city, the home city set aside) is made up the next time the clock runs
at home: the same pattern as the world while you're away. All outcomes are seeded from
the run, the job and the hour, so a reopened run plays the same.

- **`sniff <backdoor>`** (a host in the rig's `/net`, by name or number as `route`
  lists them): a **Net** backdoor you hold (a key's life is too short: `sniff: a key
  won't hold that long`). Every 3 hours it finds the door code of one more of that
  building's flats, into the journal with a new `Source::Sniffed` ("sniffed off the
  line"). With nothing left to find it ends: `sniff · {host} · nothing left to find`.
  **No logins** (my call, from the builder check): the game has no stored account a
  sniffed login could become, and a lasting admin login would undercut Copy and Plant
  jobs. The mock's `login` lines become door codes; a sniffed account can come with
  corp counter-ops.
- **`sift <shard>`**: `paydata` or `vault_data` from your pack. Takes 6 hours (a vault
  dump 10). Finds the door codes of two flats in a building the seed picks (a vault
  dump: four flats), and one time in three a
  **stash**: a component laid on the floor of one of those flats, its code known and the
  flat marked as a stash on the MAP (the Fetch placement and the map's marks do this
  today; the builder reuses them). The shard goes into the rig while it's sifted and is
  back in your pack when it's done, as a **sifted** shard: new items `paydata_sifted`
  ("Paydata shard · sifted") and `vault_data_sifted`, same value and weight, which hand
  in wherever their unsifted shard does and which `sift` refuses (`sift: already
  sifted`). So the shard keeps its value, and a shard is sifted once.
- **`keepalive <backdoor>`** (Net backdoors only, as `sniff`): while it runs, the
  backdoor's life doesn't run down (its remaining life is frozen at the minute it
  started and picks up again when it stops).
- **A job doesn't use its backdoor** (my call, from the builder check): no `check`, no
  `spend`, so it never springs a honeypot or costs life. But a backdoor whose hidden
  real end passes while a job is on it (and no keepalive holds it) burns there and then
  with `Why::Decay`, and the job ends: `the line went dead`. A job on a honeypot finds
  nothing, and ends at its first hour: `nothing on the line but bait`; the honeypot
  stays set.
- **`rent`**: ¤6 an hour, times the speed, paid into your credits by the hour.
- **Noise.** A job on someone else's server raises that building's **heat** on the rig,
  kept per backdoor: `sniff` 1 an hour, `keepalive` ¼ an hour. At 36 their admins
  patch the hole: the backdoor burns (a new `Why::Noticed`, read `the admins found
  it`), the jobs on it end, and the morning mail says so. At 18 the mail warns once:
  `their auth.log is getting long. somebody reads those.` Heat falls 1 an hour while no
  job runs on it. `sift` and `rent` make no noise.
- **Seats.** A job takes one seat; `run` with every seat busy says `run: every seat is
  busy · kill a job, or fit a bigger core`. Losing seats at the desk kills the newest
  jobs over the count.
- **Ends.** A job ends when it's done (`sift`), finds nothing left (`sniff`), its
  backdoor burns for any reason, or you `kill` it. A seized rig kills every job; a
  killswitch wipe kills those on backdoors (all of them: every backdoor burns).
- **Where `sniff` and `keepalive` come from.** Two new items, `prog_sniff` and
  `prog_keepalive` ("sniff · a rig program on a shard"): the next time you open the
  rig's terminal carrying one, it installs (`INSTALLED sniff FROM SHARD`) and the shard
  is gone; the rig keeps the program for good (`rig.programs`). `sift` and `rent` are
  installed from the start. `run` of a program not installed says `run: no such job ·
  ls /opt/jobs`, and `/opt/jobs` on the rig's disk lists what is.

### Saying what happened

- Every find, end, warning and payment is a line in the rig's **report**.
- **At 06:00** each game day, if the report has lines, a mail lands from `cron`,
  subject `overnight · {n} jobs`, its body the report grouped by job as the mock's
  mail draws it, and the report is cleared. (Jobs that ran in the home city while you
  were away land it in the home city's mail when its clock catches up, at the first
  06:00 after.)
- **The boot** adds `{n} JOBS RAN WHILE YOU WERE OUT · TYPE ps` when any job did work
  since you last sat at the rig.
- The finds themselves land where they belong at once (journal, logins, credits, the
  map); the mail is the summary.

### Where the jobs live

- `RigJobs` (the running jobs with their start, last-advanced minute and progress, the
  heat per backdoor, the report, the programs installed, and what the boot has said),
  saved with the rig's city: a field of `Backdoors` (`#[serde(default)]`), so it travels
  in `outside::Stay` with the home city as the backdoors it works on do. It's advanced
  from `net::tick_backdoors` (every step of the clock, and from the terminal's update),
  with the rig's spec.

## 6. Saving

- `Character.rig`, `Character.frames` and `Backdoors.jobs` are `#[serde(default)]`; a
  `None` rig is the stock rig. No new `AppState` field, so `snapshot`/`restore` don't
  change; the fixtures keep opening and play as the stock rig.
- New enum variants (`SocketKind`, `ItemKind`, `Mod`, `HardwareLook`, `Host`, `Why`,
  `Source`) join their enums at the end.
- New items and recipes are content; ids a build lacks are dropped by `forget_unknown`
  as ever (a fitted sled whose id is gone empties its bay).

## 7. The manual

- `09-the-rig-and-helix.md`: a new section **Working on the rig** (opening it with
  `o`, the desk's keys, bays and sleds, racks, what each kind does, where parts come
  from) and a section **Jobs** (the three commands, the four jobs, seats, noise, the
  morning mail), and `run`, `ps`, `kill` in "Only on your rig".
- `08-the-net.md` "Backdoors": "lasts three days" and "keeps up to three" become "as
  long as your rig's line sled holds (three days on the stock one)" and "as many as its
  relay sled holds (three on the stock one)"; the raid line names the killswitch.
- `11-fixers-jobs-and-the-story.md` "Standing": what a fixer gives as standing rises.

## 8. Tests and shots

- Content: every sled and frame is valid; every rig kind fits only frames; the stock
  rig's spec equals today's numbers.
- `rig::spec` sums cores, takes the best speed, reads an empty bay as nothing.
- Backdoors: planting refuses at the rig's line count; a Net backdoor's life follows the
  hold; sandbag doubles the last leg; tripwire rings once per trace; killswitch burns
  without seizing.
- The desk: fit, swap, pull, frame change (sleds move, extras to the pack); fitting is
  refused away from the desk; seats lost kill the newest jobs.
- Jobs: each job's outcome over hours; catch-up after a long gap gives the same as
  ticking through; noise to the patch; the 06:00 mail; the boot line; a seized rig kills
  jobs; the program shards install.
- A run saved before this opens with the stock rig (the fixtures).
- Shots (`screens/shots.rs`, 100×30 and 120×40): the desk on the stock rig, the desk
  with the fits-here list open on a Civitas cabinet's firewall bay, a Halcyon with every bay fitted, `ps`
  with jobs on the rig, the `cron` mail.
- Both front ends' checks as CLAUDE.md lists them; `every_drawn_character_has_a_glyph`
  covers any new glyph.

## 9. Left to the builder

- The rack's drawing per frame beyond the Zenbo (the mock's F), and the sled's hardware
  drawing, in the mock's manner and colours.
- Where `RigSpec` and the jobs' code live (`rig.rs`, `rig/jobs.rs` suggested).
- The exact wording of log lines not quoted here, in the game's terse voice.
- How the desk shrinks at 100×30.
