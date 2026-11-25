# Site objectives (#98)

A site job asks for one thing, deep in a guarded building: take something, copy or wipe
its data, plant something, or get into a room. The mission layer already marks where (the
`◎` spot). This spec says what each kind of objective is, how it is met, how it fails, and
gives fixers one job kind that sends you into a site, so the guards, alarm, containers,
site Net and turrets finally have a reason to be walked past.

Approved by Jeff on 2026-11-25. Every rule he didn't fix is marked **(call)**, my call.

## Why

Everything a site job needs is built: the generator and its copies (#34), guards and the
alarm (#33, #102), the cloak (#36), the site's Net with its data vault (#95), containers
(#96) and turrets (#97). But the `◎` does nothing, and nothing outside the dev console
calls `venues::begin`. A player can't come across a site job yet.

## Today

- A copy's layer holds `objective: Placed`, picked from the type's `objectives` in
  `sites.ron`. Each is a room (`safe`, `server`, `executive`, `cash`, `signal`, …),
  optionally on a level role. The spot is drawn as `Feature::Objective` (`◎`, magenta);
  its Look says "Objective" and nothing more.
- On the site's Net, a copy whose objective stands in a server room puts its **data
  vault** on the spot, and at security 160+ the objective's level is air-gapped.
- Containers take a fixed `Loot::Item`, "so #98 can place one"; nothing does.
- Jobs (`jobs/mod.rs`) are Fetch, Copy, Plant and Deliver, none at a site. A job's last
  objective is the hand-in at the fixer's corner. Fixers gate kinds by standing
  (`allowed_for`), and Copy/Plant only go to a build with a deck.
- SITENET's panel already shows "the job's line": the objective's name and level.

## Goals

1. **Five objective kinds**, each met in play: Take, Copy, Wipe, Plant, Reach.
2. **Each site objective says which kinds it offers**, in `sites.ron`.
3. **A site job is done when the objective is met and you're out**, then handed in as
   any job is. It can fail.
4. **A site job on the board**: fixers offer one, of a size that fits your standing.
5. **It shows** on JOBS, on the deck MAP and in tracking like any target, and its Look
   says what it is.

## Non-goals

- People as objectives (eliminate, extract, escort): #40, which needs people on site
  floors that can be followed. The kinds here leave room for them.
- Story missions on sites: #30. The copy's inputs already take a fixed type and
  objective, so a `missions.ron` effect to start one is a few lines when the first
  act-two mission needs it **(call)**.
- New site types (#99), new Net verbs beyond Wipe (#73), talk (#42).
- Replacing today's Fetch, Copy and Plant jobs. They stay as the small, cheap work.

## The kinds

| Kind | The spot holds | Met when | Every build's way |
|---|---|---|---|
| **Take** | a container holding the job's item | the item is in your pack | the container's ways in: card, Keyjack, Net, force |
| **Copy** | the data vault (site Net) | Copy on the vault, as today | deck only; offered only to a build with a deck |
| **Wipe** | the data vault | Wipe on the vault, or smash the rack by hand | deck, or force (Body), which sounds the alarm |
| **Plant** | an empty mount | you use the job's device on the spot | anyone: carry it there |
| **Reach** | nothing but the mark | you step on the spot | anyone |

- **Take.** The layer's container on the spot (`spot_kind` already makes it a safe,
  crate or locker) gets `Loot::Item` of the job's item and nothing else. Its lock grade
  comes from the security, as any container's does. The item is one of a few new
  objective items **(call: `ledger`, `prototype`, `sealed case`, named by the objective
  in `sites.ron`)**; the hand-in takes it.
- **Copy.** Only where the spot is in a server room, which is where the vault lands.
  Copying gives `vault_data`, which the hand-in takes, as a vault Copy job does today.
- **Wipe.** A new verb on the data vault only: needs the same access Copy does, gives
  nothing to carry. Without a deck, standing at the rack, **force** it the way a
  container is forced (time, Body, noise); a forced wipe raises the alarm at once
  **(call)**. That gives the street builds a server job.
- **Plant.** Taking the job hands you the device (`tap`, a new pack item, like
  Deliver's package). On the spot, use it: a few seconds, and it's planted. A planted tap
  stays for the job **(call: no guard finds or pulls it)**.
- **Reach.** You step onto the spot. The brief calls it "see the room", "photograph the
  board", whatever the objective's words make of it.

`ObjectiveDef` gains `kinds: [..]` (and `item` for Take). The current defs map like this
**(call)**:

- hotel `safe` Take · `guest` Take, Plant, Reach · `server` Copy, Wipe, Plant
- office `server` Copy, Wipe, Plant · `executive` Take, Plant · `desk` Take, Plant, Reach
- station `core` Copy, Wipe, Plant · `cash` Take · `signal` Plant, Reach

`MissionInputs` gains `kind: ObjectiveKind` (`#[serde(default)]`, Reach), so the layer
knows what to lay on the spot. Copy and Wipe refuse to lay where the spot isn't a server
room, as Take does where no container fits.

## Done, out and failed

A site job's objectives are three **(call)**:

1. the deed ("Take the ledger from the penthouse safe · Pale Orchid · 6F");
2. **Get out**: ticks when you leave the building (`venues::inside` false) with the deed
   done;
3. the hand-in at the fixer's corner, as every job.

When a site job closes (done, failed, dropped), it calls `venues::end` on its building,
which already waits until you're out and the fight is over.

It fails when:

- **Time runs out**, as any job (`Late`). Windows by size **(call: small 12 h, medium
  18 h, large 24 h)**.
- **The data is wiped in an alarm.** A Copy objective whose building goes on alert
  starts a purge: the vault wipes itself `PURGE_MINUTES` later **(call: 5 Net
  minutes)**, and an uncopied Copy job fails. The footer counts it down. This makes the
  alarm (#102) cost a decker the job, not only a fight.
- **The item is lost**: a Take job's item leaves your pack other than at the hand-in
  (dropped and left behind when the copy ends, sold, destroyed) **(call)**.
- Dying ends the run, as now.

A Wipe job is *done* by the purge, but pays half **(call)**: the fixer wanted it quiet.

## The job on the board

- A new `Kind::Site { building, objective: usize, kind: ObjectiveKind, size }`, added at
  the end of the enum, with its own `KindTag`.
- **Which building:** a hotel or office (a venue whose type has the kind) in the city, not
  your home building, not one already holding a copy, that can hold the size. Stations
  wait for #101 to place them.
- **Size by standing (call):** Small from the standing Plant needs today; Medium a step
  up; Large at the top. Security is the building's subnet's.
- **Pay** scales with size and security, above a Copy of the same security
  **(call: tune by playing)**.
- **Taking it** calls `venues::begin`; the building's doors flag. If begin refuses (it
  can't hold the size), the offer isn't made, as a Fetch with no flat isn't.
- Copy is offered only to a build with a deck, as today; Wipe, Take, Plant and Reach to
  everyone.

## Showing it

- **JOBS:** the card's stops are the site's flagged door, the deed and the hand-in. The
  deed's line names the objective, the building and the level.
- **Tracking and the deck MAP:** on the street the target is the building
  (`Target::Building`); inside, the `◎` on its level (`Target::Spot`); on another level,
  the way to it as tracking does for a flat today.
- **Look** on the `◎`: "Objective · the penthouse safe · take the ledger", and "done"
  once met.
- **SITENET**'s job line gains the kind: "copy · the server room · B1".
- The `◎` dims once the deed is done **(call)**.

## Saving

- `Kind::Site` is a new variant at the end; `ObjectiveKind` and `MissionInputs.kind` are
  `#[serde(default)]`. A copy laid before this reads as Reach.
- The purge's start minute saves on the copy (`#[serde(default)]`).
- The planted tap and the wipe save on the copy, so a reopened run keeps them.
- Fixtures keep opening, unregenerated.

## The manual

- **11 Fixers, jobs and the story:** site jobs, the five kinds, get out, how they fail.
- **08 The Net:** Wipe; the purge after an alarm.
- **07 Turns and fights:** forcing a rack sounds the alarm.

## Checking

- The venue checker lays every type's objective at every kind it offers, at every size,
  over its seeds, and fails if a kind can't lay.
- Each kind met end to end in a test copy: take, copy, wipe (deck and force), plant,
  reach; then out, then hand-in.
- The purge fails a Copy job; the item dropped fails a Take job.
- Shots: a JOBS card for a site job, the `◎` before and after, the purge countdown.

## Calls, in short

1. Five kinds now; people wait for #40, story missions for #30.
2. Each objective def lists its kinds; the mapping above.
3. Take holds a named objective item in the spot's container.
4. Wipe is a new vault verb, or forcing the rack by hand, which sounds the alarm.
5. Plant uses a tap handed over with the job; nobody pulls it.
6. A site job is deed, get out, hand-in.
7. An alarm purges the vault after 5 Net minutes: an uncopied Copy fails, a Wipe pays
   half.
8. Losing a Take's item fails the job.
9. A new `Kind::Site` on the board, hotels and offices only, size by standing.
10. Windows 12/18/24 h by size; pay above a Copy of the same security.
