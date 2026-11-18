# Containers: pick, jack, hack or force (#96)

Lockers, safes, crates, medicine cabinets and desk drawers that hold loot, and every
build has a way into them. Loot tables come with this spec, so a container is worth
opening the day it ships. The mock and its generator are in
`2026-11-18-containers-mock/` (round 2 for the look on the map, round 4 for the panel).

## Why

A break-in pays only in story today: the rooms of a hotel or an office are scenery, and a
locker is a `▮` that does nothing. #96 makes the rooms worth searching and gives every
build its own way in: a netrunner jacks in or cracks the lock from the Net, a solo or a
fixer clips on the Keyjack, anyone with Body forces it, anyone with the card walks in.
Loot drops and XP for kills (#39) and site objectives (#98) build on what this spec
lays down: #39 adds foe drops to the same loot tables, and #98 puts an objective's item
in a container.

## Decided

- **Look: solid bodies (mock option B).** A container is a filled cell, its body colour
  by kind, with a detail on it. Each piece is either set into the wall or standing on the
  floor, and a kind can be both: a safe can be in the wall or on the floor.
- **Panel: the deck's popup (mock option A, round 4).** Locked, it names the lock and
  lists the ways in; open, it lists what's inside.
- **The Net shows only when the lock is on it:** a `● NET` mark at the right end of the
  panel's top rule, and a `j jack in` row above the others. Off the Net, neither shows.
  The panel never names the node.
- **Credchips and locked credchips.** A credchip carries a sum; a locked one can't be
  cashed until it's hacked, and the hack comes later (#104: your rig's own Net). In this
  spec a locked credchip is carried and sells for nothing.
- **Loot tables live here,** in a new `flavor/loot.ron`. #39 becomes foe drops from the
  same tables, the stash at home, and XP for kills.
- **Force has no noise.** Noise was dropped with #102. Forcing takes time, leans on
  Body, and a guard who sees you sees you; a lock on the Net sets off the alarm when it's
  forced.

## Goals

- Five kinds of container placed by room and site type, each with a lock grade.
- Every way in: the card, the Keyjack, the Net (a crack from anywhere, or jacking in at
  the container), and force.
- Contents rolled from loot tables by kind, site type and security; fixed by the seed;
  what's left saved with the run.
- The look from the mock on the map, wall-set and floor-standing, closed, open, empty and
  on the Net.
- The take panel and the locked panel.

## Not in this spec

- Hacking a locked credchip (#104).
- Shops, fences and selling (#45).
- Foe drops, the home stash, XP for kills (#39).
- Objectives in containers (#98). The contents model takes a fixed item so #98 can
  place one; nothing places one yet.
- Containers in the player's own flat: its lockers stay furniture until the home stash
  (#39).
- New items beyond the two credchips. A table hands out what exists.

## What would make it a failure

- A locked container with no way in for some build. Force is always there for anyone
  with the Body for its grade, and a lock that needs more Body than a build can reach
  has another way in (the Keyjack, the Net or the card).
- Reloading rerolls a container.
- A container that blocks the only way through a room, or stands on a doorstep.
- Loot that floods the economy: a run through a hotel shouldn't pay more than a job.

## Approaches

1. **A container is a piece of furniture on a tile, with its state in a saved map
   (chosen).** The tile carries the kind; a map keyed by building, level and tile holds
   what happened to it (opened, forced, what's left, the Keyjack's faults). Nothing about
   a container is stored until it's touched, so a site with forty lockers costs nothing
   until you open one.
2. **A container as an entity, like a guard.** It would give each one an id and a place
   in a list. It buys nothing a tile doesn't, and every floor would carry a list.
3. **Contents laid on the ground under a closed lid when the floor is drawn.** Piles
   already save, but every floor would roll every container when drawn, and the lock
   would have nowhere to live.

## The design

### Kinds

A new `ContainerKind` enum: `Locker`, `Safe`, `Crate`, `Cabinet` (a medicine cabinet),
`Drawer` (a desk with a drawer). Each is a `Furniture` piece; `Furniture::Locker` keeps
its place, the others join the enum at its end, and `Furniture::container()` maps a piece
to its kind. `Feature::Container` (the mission layer's kindless spot) stays for old
saves and draws as a safe.

| Kind | Body (B) | Detail | Lock | Grade |
| --- | --- | --- | --- | --- |
| Locker | steel blue `#3a4a5c` | seam `┃` `#9fb6cc` | locked by the seed, about half | 1 |
| Safe | gunmetal `#2b2f36` | dial `◉` `#d8c890` | always | the building's lock maker: 1 to 3 (`keyjack::rating`) |
| Crate | brown `#5a4a32` | bracing `╳` `#a89060` | none | — |
| Cabinet | white `#b9c3cc` | cross `╋` `#c0392b` | locked in a secure zone | 1 |
| Drawer | desk `#3e4c5c` | handle `╍` `#a0b8d0` | locked by the seed, about a quarter | 1 |

States, as in the mock's key:

- **Closed:** as above.
- **Open, something left:** the detail becomes its open shape (`▯ ○ □ ┼ ─`).
- **Empty:** the open shape in the dim grey `#5b6b7d` on `#20262e`.
- **On the Net and still locked:** the detail in the Net's cyan `#00e6d2`.

A container is never walkable. A wall-set one is drawn on its wall tile and opened from
the floor tile in front of it, the way wall fittings sit (`FittingKind::on_wall`).

### Where they go

- **Venue sites (hotel, office, metro), plain and copy.** A room in `flavor/sites.ron`
  names its containers as it names its furniture. `RoomDef` gains `wall` (pieces set into
  the room's bare wall tiles) beside `furniture` (pieces on the floor), with
  `#[serde(default)]`. The builder fills in each type's rooms. As a guide:
  - Hotel: the lockers room's lockers (in the wall), a guest room's drawer, the security
    office's safe on the floor and a locker, the linen room's crates, the penthouse's
    safe in the wall.
  - Office: open-plan desks with drawers, the executive floor's safe in the wall, a
    store room's crates, a first-aid cabinet in the wall.
  - Metro: the staff room's lockers, the cash room's safe, store rooms' crates.
- **The mission layer's spots (copies).** Each `Placed` in `layer.containers` gains a
  `kind` (`#[serde(default)]`, which reads an old save's as a safe), chosen by its room:
  a safe in a secure or executive room, a crate in a store or a plant room, a locker
  otherwise. They stand on the floor. A layer spot is the job's guarded container: it is
  always locked, whatever its kind, at the building's lock maker's grade, and always on
  the Net (a sealed security crate, a locked locker).
- **Classic buildings.** A `RoomKind::Lockers` room's lockers and a clinic's lockers
  become containers, rolled from the `plain` tables; the clinic layouts don't change.
  The player's own flat keeps its lockers as furniture.

### Locks and the Net

- A container is on the Net when its site's Net has a node for it. `SiteNet::of` adds a
  `NodeKind::Container` node for every safe in a site (venue rooms and the layer's
  spots) and every layer spot, named by kind ("penthouse safe", "store crate"). Lockers,
  drawers, cabinets and crates in rooms stay off the Net.
- **The card:** a locked container opens with the keycard of its room's zone. Carry it
  and space opens the container straight away.
- **The Keyjack (`k`):** the slab's timing game at the container's grade, with the fuse,
  Lockpick and Skeleton key as on a keypad. The fourth hit opens it; the last fault burns
  the fuse and locks the container out of the Keyjack for `LOCKOUT_MINUTES` (the other
  ways in stay open). The slab is hosted by a new
  screen for a lock with no keypad; the run's state is keyed by the container.
- **A crack from the Net:** a live crack on its node (`sitenet::nodes::container_open`)
  holds it open, and space opens it with no prompt. An alarm drops the crack, as it does
  a door's.
- **Jack in (`j`, on the Net only):** the container's lock is a port, like a terminal's.
  You jack in at its node through `sitenet::jack_in_on_floor`, your body standing beside
  it, and land on its node. Refused while the building is on alert ("port locked down"),
  as for any port.
- **Force (`f`):** see below.

### Force

- The row shows the Body the grade needs: 2 for grade 1, 4 for grade 2, 6 for grade 3.
  Below it, the row is greyed.
- Forcing takes time: a bar on the map over the container, 3, 6 and 10 seconds for grades
  1 to 3, cut by 5% for each point of Body over the need. A step or another key stops it,
  and the bar starts over next time.
- Guards keep walking and see you as they would anyone.
- Done, the lock is broken for good: the container is open. A forced container on the
  Net sets off the alarm (`net::lock_down`).
- Not in a fight: forcing and the Keyjack are refused while you're engaged ("not now").

### Contents and the loot tables

`flavor/loot.ron`, compiled in like the other flavor files, parsed and checked by a test.
The schema is the builder's; what it says:

- A **table** is picked by the container's kind and the site type (`hotel`, `office`,
  `metro`, or `plain` for a classic building), with a fallback per kind.
- **Security** sets a band: low below 86, mid below 170, high from there. A line can name
  the bands it rolls in, and a cash or credchip sum scales with the band.
- A table rolls **a count of lines** (a range), from **weighted lines**, after a chance
  of being **empty**.
- A **line** is one of:
  - **cash:** a range of credits, straight to your credits when taken;
  - **credchip:** a range, and a chance it's locked;
  - **item:** an item id and a quantity range;
  - **card:** the keycard of a deeper zone on the same site, when there is one.
- A test walks every table and checks each item id, range and band.

As a guide, by what a player can use now:

- Lockers: a little cash, soy-noodle cups, stim patches; a security locker adds grenades
  and a card.
- Drawers: a little cash, now and then a credchip.
- Cabinets: stim patches.
- Crates: fuses, components, grenades in a security store.
- Safes: cash and credchips, a locked one often; deeper in, a program or hardware.

The tables are tuned per container, not budgeted per site: each container rolls on its
own. Rooms that come in numbers (a hotel's guest-room drawers, a factory's wall of
lockers) lean empty and cheap, so a whole plain building pays less than a job does. A
job's layer spots are where the money is.

### Credchips

- Two items in `flavor/content.ron`: `credchip` and `credchip_locked`, per-copy
  (`Junk`, weight 0.05).
- The sum lives on the copy (`ItemCopy` gains `credits: u32` with `#[serde(default)]`),
  shown in its name: "credchip · 400 cr".
- A credchip is cashed by using it in the pack: its sum goes to your credits and the chip
  is gone. A locked one refuses: "locked · crack it at your rig" until #104 builds that.

### Rolled once, saved

- A container's contents are rolled the first time it's opened, from
  `hash64(hash64(site seed, CONTAINER_SALT), level and tile packed)`, where the site seed
  is the layout's seed and, in a copy, the layer's (`hash64(layout.seed, inputs.seed)`),
  so a new job's copy rolls fresh.
- A `ContainerState` keyed by building, level and tile holds: opened or forced, what's
  left, and the Keyjack's faults and lockout. Untouched containers have no entry.
- A copy's containers live in its `MissionCopy` (a new `#[serde(default)]` field) and go
  with the copy. A plain building's live in `Interiors`, carried by `Stay` with the city
  (docs/saving.md, rules 2, 3 and 15).
- The run fixtures keep opening.

### The panels

Both are the verb popup's frame (`screens/popup.rs`), opened by space beside a container.

- **Locked:**
  - The header: the container's glyph and kind (`◉ SAFE`), and `● NET` in cyan at the
    rule's right end when it's on the Net.
  - The lock: "megacorp lock · grade 3" (the maker for a safe, "lock · grade 1" for the
    rest).
  - Rows:
    - `j` jack in, on the Net only;
    - `k` Keyjack, with the fuses you carry, greyed with none;
    - `f` force it, with the Body it needs;
    - `c` card, with the zone, greyed when you don't carry it.
  - The first enabled row is selected. `esc` leaves it.
- **Open:**
  - The header: the open glyph and "SAFE · OPEN".
  - One row a line, lettered `a` on: cash and its sum; a credchip and its sum, or
    LOCKED in magenta; an item with its quantity and weight.
  - A letter or space takes that line; `t` takes all; the pack's load sits beside it.
    A take that would go over your carry cap is refused, as `Action::Take` does.
  - What's left stays. Empty, the container shows its empty look and space says "empty".

`look` names a container: "safe · megacorp grade 3 · locked", "· open", "· empty",
"· on the Net".

### The manual

Chapter 06 (the city and its buildings) gets containers: the kinds, the ways in, force
and Body, credchips and the locked ones. Chapter 07 says forcing and the Keyjack aren't
for a fight. The "On foot" keys don't change: space already reaches the thing in reach.

## Checking

- Tests per way in: card, Keyjack (open, and a burnt fuse locking it out), a crack, jack
  in, force (time by Body, alarm on the Net, refused below the Body).
- Contents: the same container rolls the same twice and across a save and reopen; a new
  copy rolls fresh; taking half leaves half.
- Placement: `validate_site` keeps passing on every type's sizes, no container on a
  doorstep or a spine, wall-set ones on a wall tile with floor in front.
- The loot table test.
- Shots of both panels and of the map with every kind and state, in the window too
  (`every_drawn_character_has_a_glyph`).
- The five checks on both front ends.

## Calls I made

- Lock grades and the Body needs (2, 4, 6) and force times, as above.
- Crates have no lock; lockers and drawers are locked by the seed.
- Only safes and the mission layer's spots are on the Net. A layer spot is always
  locked, at the maker's grade, whatever its kind.
- The tables are tuned per container, with no per-site budget; numerous rooms lean
  empty.
- Clinics keep their layouts; their lockers roll from the `plain` tables, and no cabinet
  goes in a clinic.
- A burnt fuse locks out only the Keyjack.
- Carrying the card opens a container with no panel.
- Force and the Keyjack are refused in a fight.
- Credchips are `Junk` and cash by use.
- The security bands' edges (86 and 170).

## Left to the builder

- The loot file's schema and its numbers, within the guide above.
- Each site type's room containers, within the guide.
- The new lock screen's shape around the slab.
- Where exactly `ContainerState` sits, within the copy and `Interiors` rule.
