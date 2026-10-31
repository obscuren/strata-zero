# Drone chrome: a bay on the cradle, a drone at your shoulder

Date: 2026-10-30. Builds on body-site chrome (`2026-10-29-stats-chrome-design.md`) and
ripperdoc clinics (`2026-10-29-ripperdoc-clinics-design.md`). Mockups:
`2026-10-30-drone-chrome-mock/`: `round1.html` (the ledger's three states, approved, and
four glyphs), `round2.html` (four ideas built on the rotor; A3, the rotor with a wake, is
the one to build). `gen.py` and `gen2.py` draw them.

## Why

The Drone cradle is chrome on the BACK that grants Scout and nothing else: no drone ever
shows. Jeffrey wants the cradle to fold out a bay in the ledger, the way a chrome arm
folds out its hand and forearm, and a drone docked in that bay to come out with you:
moving on its own, staying near you, following you about, its glyph always animating.
Combat comes later, and drones will fight in it, taking turns of their own; other small
vehicles, a rover first, come later too. The code is built for both now.

## What Jeffrey decided

- **The drone you dock decides what you get.** The cradle is the mount; each drone
  carries its own abilities and bonuses. Scout moves off the cradle onto a Scout drone.
- **One drone at a time for now**, with room to expand: the cradle has one bay, and the
  model takes any number of bays on any piece.
- **Drones swap anywhere.** The cradle is chrome and still goes in and out in a
  ripperdoc's chair; docking, swapping and stowing a drone in its bay needs no chair and
  costs no fee.
- **The ledger (round 1, all three states):** the bay is a row under the cradle,
  `└ Scout drone`, or `└ □ bay` when empty; the cradle's card shows the bay and what it
  holds; the bay row's card is the drone's own, with its drawing; space on the bay opens
  what docks from the bag.
- **The glyph is A3:** a rotor, `┼` and `╳` swapped, in the drone's own light, spinning
  twice as fast while it flies; flying leaves a wake of dots that fade over three beats;
  standing still leaves none.
- **The behaviour, as played back and approved:** standing, it drifts a tile at a time
  within two tiles of you; walking, it trails and catches up. It never sits on your tile,
  flies over floor and never through walls, and blocks no one. It runs on the world's
  clock, so it freezes with menus and the overview. It comes with you through doors,
  stairs and lifts, appearing beside you on the new map; while you are jacked in it
  hovers by your body. A docked drone takes no LOAD, costs no EMP and weighs nothing,
  like installed chrome; it weighs what it weighs in the bag.
- **It is an entity that takes turns.** On turns on foot it has its own seat in the
  order; for now it spends its turn staying near you. Combat fills that turn later.
- **Built for other vehicles.** A rover is coming. What a vehicle is and how it moves is
  content and a kind, not a special case for drones.

## Goals

- A drone bay: a new socket kind, on the Drone cradle, that takes drones, folded under
  the cradle in the ledger by the ledger's existing socket rows.
- Bots: a new kind of item, `Bot`, whose content says its body (air now, ground later),
  its look and its turn. Drones are bots with an air body.
- A docked bot is out with you on the map as an entity: drawn, moving on the world's
  clock, following, crossing places with you, taking turns on foot.
- Three drones ship: Scout, Mule and Lamp (below), each with its card drawing; the cradle
  gets a new drawing of the mount alone.

## Non-goals

- No drone combat: no attacking, no hp, no foes noticing, shooting or blocking on drones.
- No rover, no ground body in content. The ground body exists in code as a kind with its
  own movement rule, unused by content.
- No ability cast from the drone's tile: Scout still waves out from you.
- No buying drones: they come from the debug runner's pack, as chrome does.
- No drone in the Net: jacked in, your drone is with your body, not in the construct.
- No saving where the drone was: a load puts it beside you.

## Anti-goals

- Drone-specific code where a kind would do. Wording comes from the bot's body and the
  socket's kind (`drone`, `bay`), never from "Scout drone"; the ledger, picker, card and
  hint row read bays and bots like any socket and piece.
- A number shown that nothing uses. Bots carry no hp or attack until combat does.
- A drone that ever covers you, stands in a wall, or draws on a tile you can't see.
- Breaking the chair rule for chrome: only a bay's contents skip the chair.

## The model (game crate)

### The bay

- `SocketKind::Bay`: label `bay`, takes `drones`, fits `bay`.
- `SocketKind::needs_a_doc()`: true for `Hand` and `Forearm`, false for `Bay` (and for
  the deck and gun sockets, which never reach surgery). `surgery_fee` returns `None` for
  `Fit` and `Unfit` on a socket that doesn't need a doc, so `perform` lets it through
  anywhere, free. Pulling or installing the cradle itself is surgery as before; what is
  docked in it rides along, as fitted pieces do.
- The Drone cradle: `sockets: [Bay]`, no grants, its line changes (below).
- Content validation: chrome sockets are hand, forearm or bay; a bay is on chrome only;
  a bot fits a bay and nothing else fits one.

### Bots

A new item kind, `ItemKind::Bot` (label `bot`), and a field `bot: Option<Bot>` on
`Item`:

```rust
pub struct Bot {
    /// How it gets about: in the air over any floor, or on the ground.
    pub body: BotBody,
    /// The socket it docks in.
    pub docks: SocketKind,
    /// Its look on the map: the frames it cycles, in its light.
    pub frames: Vec<char>,
    pub light: (u8, u8, u8),
    /// Whether moving leaves a wake.
    pub wake: bool,
    /// Its seat in a fight and how far it moves in a turn.
    pub reflex: u8,
    pub moves: u8,
}

pub enum BotBody { Air, Ground }
```

- `BotBody::word()`: `drone` for Air, `rover` for Ground: what the UI calls it.
- A bot is held per copy (`per_copy` true), as socket chrome is, since `Fit` moves
  copies.
- `Fit` accepts a piece whose `bot.docks` is the socket's kind, next to hardware and
  socket chrome; one helper, `Item::fits_socket() -> Option<SocketKind>`, answers for all
  three and is used by `Fit`, the ledger's picker and validation.
- A docked bot's `perks` and `grants` count as chrome's do (they already reach `stat`,
  `abilities` and the rest through `installed()`), with two exceptions: `pieces()` leaves
  bots out, so they cost no EMP, and they take no LOAD (they have no `implant`). A
  number's sources name it by its kind: `drone +3 Lamp drone` where chrome reads
  `chrome +1 Piston arm`.
- Validation: a bot has a `bot` and no `implant`, `hardware` or `sockets`; at least one
  frame; every frame and the wake's dots render in the window (box-drawing or a symbol
  the map already uses); `moves` at least 1.

### The drones that ship

| id | Name | Does | Weight, value |
|---|---|---|---|
| `scout_drone` | Scout drone | grants Scout | 0.6, 2400 |
| `mule_drone` | Mule drone | carry +8 | 1.4, 1800 |
| `lamp_drone` | Lamp drone | sight +3 | 0.5, 1500 |

All three: `body: Air`, `docks: Bay`, `frames: ['┼', '╳']`, `wake: true`, `reflex: 6`,
`moves: 5`. Lights: Scout `(120, 255, 120)` (its Wave's green), Mule `(255, 200, 80)`,
Lamp `(255, 240, 200)`. Lines: Scout "A drone the size of a sparrow. It comes back when
it's bored." (the cradle's old line); Mule "Four rotors and a cargo net. It carries what
you can't, and it complains."; Lamp "A drone with a lamp and nothing to say. You see
further with it up." The Drone cradle's new line: "A cradle bolted between the shoulder
blades and wired to the spine. Whatever docks in it flies for you."

Each has a 30×12 drawing in `flavor/implant_art.ron`, drawn in
`2026-10-29-stats-chrome-art/implantart.py` in that file's register; the Scout drone and
the empty cradle in `gen.py` are the approved starting points.

The debug runner starts with the Drone cradle installed at BACK and the Scout drone
docked, so a debug game has its drone out at once; the Mule and Lamp drones are in its
pack (no spare Scout). The cradle leaves the pack of chrome to try on.

## The ledger

Nothing new to build for the row: the bay is a socket of the cradle, and the ledger
already folds a piece's sockets under it (`ledger_rows`). What changes:

- **The cradle's card** shows its bay as limbs show theirs: `■ bay  Scout drone` and the
  docked bot's line under it, or `□ bay  empty` with what in the bag would dock.
- **The bay row's card** is the bot's own: its drawing, name, `docked in the Drone
  cradle`, its effects line (`grants Scout`, `carry +8`), then `out with you · follows on
  foot` in FAINT, then its line. An empty bay's card is a socket's, as today: `BAY`,
  `a bay on the Drone cradle`.
- **Keys on a bay row work anywhere**, chair or not: space opens the picker, a pick docks
  it (swapping what was there to the bag), `x` stows the docked bot in the bag. Jacked in
  it stays locked, as the whole sheet is.
- **The picker on a bay** lists what docks from the bag; its right column shows each
  bot's effects in GOOD (`carry +8`) where chrome shows its load.
- **The hint row on a bay:** `↑↓ select · space swap · x stow in the bag · ← attributes ·
  Q/E tabs` and, on the right, `drones dock anywhere` (from the bot body's word) in place
  of the chair line. On an empty bay, `space dock` instead of `space swap` and no `x`.
- The words come from the socket kind and the bot's body, so a second bay or a rover's
  dock reads right with no new code.

## Bots on the map (strata-zero)

### The entity

A new module, `bots.rs`, beside `foes.rs`. `AppState` gains `bots: Vec<BotOut>` (name
the builder's), one per docked bot:

- `item` (the bot's id), `dock` (`(Site, socket)` it came out of), `place`, `pos`;
- the pace: when it last moved, when it next drifts, whether it is catching up;
- the glyph's frame and the wake: up to three recent tiles with the tick each was left.

It is not saved; each load and each new game sync it fresh.

### Sync, every frame on the map

`bots::sync(st)` makes `bots` match what is docked: a bot docked and not out comes out on
a free floor tile next to you (the nearest one, behind you when you just moved); one no
longer docked (stowed, swapped, its cradle pulled) is gone at once. Whoever stands where
the bots follow is "the anchor": you on foot, your body while you are jacked in. A bot on
another place than the anchor's comes over to a free tile next to the anchor, so doors,
stairs, lifts and jacking in and out need no code of their own.

A free tile for a bot is one its body may enter (Air: any tile you could walk on, as
`walkable` has it for the place; Ground: the same for now, with the rule kept apart so a
rover can differ), not the anchor's, not a standing foe's, not another bot's.

The leash is distance and sight: a bot is "with you" when it is within `LEASH` tiles
(Chebyshev) and nothing that blocks sight (wall, door, window, mass; `foes::blocks_sight`)
stands on the line between it and the anchor. Drifting, it only picks free tiles that are
with you and are not doors, so it never slips through a doorway, locked or not, into the
next room; it crosses a door only while catching up, behind you.

### Moving, off turns

On the world's clock (`clock_tick`), so it stops when the clock stops:

- **Drift:** within `LEASH = 2` tiles (Chebyshev) of the anchor, every 15 to 40 ticks
  (rolled from a hash of the tick and the bot, so tests can pin it) it moves to a random
  free neighbouring tile still within the leash, or stays if none.
- **Catch up:** not with you (too far, or out of sight of you), or on a tile that isn't
  free (yours, a foe's), it steps one tile every 3 ticks along the shortest path of tiles
  its body may enter toward a free tile with you, behind the anchor, until it is with you
  again; then it drifts again. Foes' `step_toward` paths on strata only, so bots path
  over the place's own map, floors included.
- It never moves while the map is on turns (below).

### On turns

`Actor::Bot(usize)` joins `Actor`. When the map goes to turns on foot, every bot out on
that place gets a seat by its `reflex`, after foes of the same score; one that comes out
mid-fight takes a seat at the end, as foes do (`seat_foes`). Its pools are
`(moves, 0)`. Its turn: if it is not with you, or on a tile that isn't free (yours, a
foe's), it moves toward you, one tile a `PACE_TICKS`, until it is; then it ends its turn
(`Step::Nothing`), so a bot already near you passes at once. It is never a target, never in the fire cursor's list
and never in the way of a shot or a foe. The turn column shows its chip with its name and
no hp; the footer's turn line reads `Scout drone moves 2`. In a construct the order holds
no bots.

This is the seam combat fills later: an attack is a bot's AP and a `Step::Hit`, as foes
already have.

### Drawing

A `BotOverlay`, pushed after the people overlay, so a bot draws over walkers and under
the aim path, target marks and your `@`:

- **The bot:** on a Visible tile that no standing foe holds (a foe always draws over a
  drone), its frame `frames[(tick / pace) % len]` in its
  light, bold; `pace` 6 ticks while it drifts or stands, 3 while it catches up or moves
  on turns.
- **The wake:** each tile it left in the last 9 ticks, if Visible and holding no one, a
  dot over the tile's own background: `∙` for the first 3 ticks, then `·`, its colour
  going from the bot's light to the tile's background as it ages. Air bodies with
  `wake: true` leave one; standing still leaves none.
- Remembered tiles show nothing: a bot is not remembered where it was, as people aren't.

## Docs and shots

- README: the chrome section gets the cradle's bay and drones (dock anywhere, swap,
  stow, out with you).
- Shots at 100×30 and 120×40: the ledger on the cradle, on the bay with the Scout drone,
  the picker on the bay; and the street with the Scout drone out beside you.

## Tests

- `game`: `Bay` needs no doc, so `surgery_fee` is `None` for docking and stowing and
  `Some` for hand and forearm; a docked bot's grants reach `abilities` and its perks
  reach `carry` and `sight`; `pieces()`, `chrome_load` and EMP leave docked bots out;
  `Fit` refuses a bot in a hand socket and chrome in a bay; pulling the cradle takes the
  drone along; validation catches each broken bot rule; the shipped drones load.
- `strata-zero`: `perform` docks and stows a drone outside a chair; the ledger's keys on
  a bay work outside a chair and stay locked jacked in; the bay's hints; `sync` brings a
  bot out next to you, takes it away when stowed, and moves it with you to a new place
  and to your body when you jack in; drift stays within the leash and off your tile;
  catch-up comes back inside the leash; nothing moves while the clock stands; on turns a
  bot is seated, moves toward you on its turn and passes when near, and is never a
  target; the overlay draws the frame for the tick, the wake fading, nothing on a tile
  out of sight.
- Both front ends build and pass (`CLAUDE.md`).

## My calls (Jeffrey to check)

- Names: `SocketKind::Bay`, `ItemKind::Bot`, `game::Bot` and `BotBody`, `bots.rs`,
  `Actor::Bot`. "Bot" covers drones and rovers in code; players read `drone` or
  `rover`.
- The three drones, their numbers, lights and lines; Mule and Lamp are there so swapping
  means something on day one.
- `LEASH = 2`, drift every 15 to 40 ticks, catch-up a step every 3 ticks; glyph pace 6
  and 3 ticks (30 frames a second, so the mock's 200 ms and 100 ms).
- A docked bot's bonuses read `drone +3 Lamp drone` in a number's sources.
- On turns a bot seats by its own reflex (6), after foes of the same score.
- A bot draws under your `@` and over walkers; it is not remembered off sight.
- From the builder check: the leash needs a clear line to you and drifting never takes a
  door, so a drone can't wander into the next room; a foe draws over a drone and the
  drone steps off a foe's tile; the debug runner (and so the COMBAT runner) starts with
  the cradle in and the Scout drone docked.
- A bot's item kind lists with the icon `┼`, its rotor.

## Left to the builder

- The entity's field names, and how `sync`, drift and the turn hook split in `bots.rs`.
- The wake's exact fade, within "three beats, light to background".
- The drone drawings, in the chrome art's register.
- Message wording for docking and stowing, in the house style (`` `Scout drone` docked
  · bay``, `` `Scout drone` out · to the pack``).
