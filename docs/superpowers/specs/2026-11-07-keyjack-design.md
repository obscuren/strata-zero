# The Keyjack: a way through the door for every build

Issue #6, every build can finish a fixer job. A Fetch job's flat opens only through the
Net, a journal code or that flat's own keycard, and only a NETRUNNER starts with a deck.
So a Solo, a Fixer or a Techie can take a Fetch it can't finish.

The answer is a device anyone can carry: the **Keyjack**, a street-made slab on a lead.
You clip it onto a door's keypad and it plays a short timing game for each of the code's
four digits. A marker sweeps a bar; you press space when it sits in the lit zone. Every
hit keys one digit into the pad and narrows the zone for the next beat. The third fault
locks the door and burns out the Keyjack's fuse. The fuse sits in a socket on the board
inside the case; without one the Keyjack won't run. Better fuses are its mods, a Techie
builds Keyjacks and fuses at the bench, and every build starts with one. A lock's rating
sets how narrow the zones get, and every number of the game is data.

Mockup: `docs/superpowers/specs/2026-11-07-lock-cracker-mock/` (`gen.py` builds the page
from the throwaway `cracker_mock` and `keyjack_art_mock` shot tests in
`cracker_mock.diff`). Round 2 is the Keyjack as built here; round 1 holds the three
devices first drawn.

## What Jeff decided

- **A clip-on device with a timing game**, not a build skill: a small screen with a bar,
  a marker going left and right, a coloured zone; space lands the marker in the zone;
  each hit makes the zone smaller; a miss is a fault, and you get three.
- **Device A, the KEYJACK** (a black slab in a magenta case on a lead, a green LCD,
  fault lights), over B (a clamp that takes the pad's glass) and C (an amber pin
  scope).
- **The third fault locks the door and destroys the fuse.** The fuse is a socket on the
  Keyjack (M from Inspect opens the back), replaceable and upgradeable as a mod. With no
  fuse in it the Keyjack doesn't work.
- **The Keyjack gets artwork**: its Inspect drawing and its opened back, as drawn.
- **Every build starts with one** for now. **A Techie can build Keyjacks and new fuses.**
- **Difficulty is easy to tweak overall**, and **door locks carry a rating**: a higher
  rating means smaller beat zones.
- **A key on the keypad** chooses to clip the Keyjack on; the pad works as now
  otherwise.
- The drawings and the fuse tiers in round 2 were approved as drawn.

## Goals

- Every build can get into a Fetch job's flat without the Net.
- No build is offered a job it has no way to finish (#6's done-when).
- Nothing a player can spend SP or a talent on does nothing without saying so, for the
  pieces #6 names: Lockpick and Skeleton key now tune the Keyjack.
- The game's numbers live in one data file; changing them needs no code.

## Non-goals

- People calling the police when they see you clip a Keyjack on (later).
- Plugging a cyberdeck into a keypad (later).
- A non-netrunner way into a server (Copy, Plant and Vault jobs); fixers stop offering
  those to a runner with no deck instead (§9).
- The lobby-card hint that can point at the wrong flat, and taking a resident's lanyard
  keycard (#6's other loose ends); and the Smoke talent, which also sets a flag nothing
  reads. Each stays as it is and goes to its own issue.

## Anti-goals

- A game that can't be won by a careful player at the hardest rating with the stock fuse.
- A silent refusal: no fuse, a locked pad and a blown fuse each say so where you look.
- Numbers in code that a designer would want to tune.

## 1. The items

- **`keyjack`**: "Keyjack", a moddable item with a drawing (`look: Some(Keyjack)`) and one
  socket of a new kind, `Fuse`. Weight 0.3 kg. Its description is the Inspect text
  drawn: *"A slab on a lead that clips onto a door's keypad and works its four digits out
  with you, a beat at a time."*
- **Fuses**, hardware fitting `Fuse`, each carrying its bonus as a hardware mod in
  `content.ron` (that is where hardware mods live):
  - `fuse_stock`, "Stock fuse": no mod at all; its card reads "no bonus". Content
    validation allows hardware with no mods for the `Fuse` socket only.
  - `fuse_ceramic`, "Ceramic fuse": `Faults(1)`, read "faults +1".
  - `fuse_slowblow`, "Slow-blow fuse": `Sweep(75)` (the marker at 75% of its speed),
    read "marker slower".
  - `fuse_wideband`, "Wideband fuse": `Zone(1)`, read "zones +1".
  `Faults`, `Sweep` and `Zone` are new `Mod` variants; the card and ledger text comes from
  their numbers, as the other mods' does.
- **A fuse's drawing**: one new hardware look, a glass cartridge fuse with metal caps and
  a filament, tinted per tier (stock: clear glass, silver caps; ceramic: white body;
  slow-blow: a coiled amber filament; wideband: cyan glass). It has the large Inspect
  picture, the tray swatch and the fitted drawing every hardware look has; fitted, it
  sits in the Keyjack's holder on the opened back. (Not in the approved mock: my
  drawing.)
- Names, weights and values are content; my calls for values: the Keyjack 400, the stock
  fuse 40, the others 150.

## 2. Getting one

- **Every build's kit** gains a Keyjack with a stock fuse fitted and **two spare stock
  fuses** in the pack. The mechanism is generic: a piece of hardware in a kit is fitted
  into the first kit item with a free socket of its kind, and the rest go in the pack, so
  the kit lists `("keyjack", 1)` and `("fuse_stock", 3)`.
- **A finished Fetch pays a stock fuse** with its credits, so a runner who can't build
  fuses keeps a way in. The hand-in line says so.
- **Recipes** at the workbench, skill `electronics` (the Techie's), from parts already in
  the game:
  - Keyjack: `dead_phone` 1, `copper_coil` 2, `scrap_servo` 1. It comes with no fuse.
  - Stock fuse: `copper_coil` 1, two at a time.
  - Ceramic fuse: `copper_coil` 1, `scrap_servo` 1.
  - Slow-blow fuse: `copper_coil` 2.
  - Wideband fuse: `copper_coil` 1, `dead_phone` 1.
  Each recipe has the two-step text recipes carry, in the item descriptions' voice.

## 3. On the keypad

- When you carry a Keyjack, the pad shows one line under itself: `k  clip the keyjack`.
  If no Keyjack you carry has a fuse in it, the line is greyed: `k  keyjack · no fuse`,
  and `k` logs *"No fuse in the Keyjack."* With several, the first with a fuse is used.
- Typing digits still works as now. `k` is refused while the pad is locked (its lockout
  shows as now).
- `k` clips it on: the Keyjack appears to the pad's right on its lead (mock frame
  `a_beat1`), and the run begins. Space strikes; esc pulls it off.
- The run plays four beats, one a digit, left to right. A hit keys that digit into the
  pad's glass (as typing does) and lights it on the Keyjack's LCD; the next beat starts
  with a narrower zone.
- A miss is a **fault**: the bar flashes red with ERR, a fault light comes on, and the
  beat starts again (same zone width, a new position).
- **Faults stick to the door.** Pulling the Keyjack off and clipping it on again starts
  the beats over but keeps the faults. They clear when the door opens or when its lockout
  ends. They are kept with the pad's state in the journal.
- **The last fault** (3, or more with a ceramic fuse or Skeleton key) locks the pad for
  its lockout, as three wrong codes do (`credentials::LOCKOUT_MINUTES`), and destroys the
  fitted fuse: the fuse leaves the socket and is gone. The Keyjack's LCD shows FUSE BLOWN
  and the pad DENIED; the log says *"The Keyjack's fuse blew. The pad locked for 30m."*
- **The fourth hit** opens the door as the right code would: the pad says OPEN and the
  journal learns the code, with a new source, `Keyjack` ("cracked with the Keyjack"). The
  faults clear.
- The Keyjack's LCD shows the lock's rating (`R1`–`R3`) beside BEAT.
- The panel draws **one fault light per fault the run allows** (3 to 5 with today's
  fuses and skills), lit as they are spent; the panel's layout has room for up to 6.
- Success and failure each get a path in `credentials` that takes what opened or locked
  it: opening with a given `Source`, and a lockout without a typed code. `try_code`
  keeps its own behaviour for typed codes.

## 4. The game's numbers (`flavor/keyjack.ron`)

The game's own numbers in one file, compiled in like the rest of `flavor/` (a fuse's
bonus is on the fuse, in `content.ron`):

- the bar's width in cells (18);
- for each rating, the zone width of each of the four beats, in cells. My calls:
  R1 `[8, 6, 5, 4]`, R2 `[7, 5, 3, 3]`, R3 `[6, 4, 3, 2]`;
- the marker's speed per beat, in cells per second, the same for every rating (my
  calls: `[9, 11, 13, 15]`), sweeping end to end and back;
- the faults a run allows (3);
- what each skill adds: Lockpick zones +1 cell, Skeleton key +1 fault. They add to the
  fitted fuse's mod (wideband zones +1, ceramic faults +1, slow-blow speed × 75%);
- the rating of each lock maker: `street` 1, `municipal` 2, `neon` 2, `megacorp` 3,
  keyed by the device themes `ui::device` already picks for a pad.

Zones never go below 1 cell or past the bar. A zone's position is picked per beat from
the door and the beat, so a test can drive a run.

## 5. Skills

- **Lockpick** (the GHOST skill): every zone one cell wider.
- **Skeleton key** (its talent, flag `skeleton_key`): one more fault.
- They stack with the fuse. Their cards in SKILLS and TALENTS keep their own text; the
  manual says what they do to the Keyjack.

## 6. Inspect and modding

- **BLUEPRINT · KEYJACK** draws the front as in the mock (`kj_front`): the slab, the
  LCD with its bar and READY, the fault lights, the STRIKE key, the grip ridges, the
  badge with a fuse light (green with a fuse in, dark without), and the lead ending in an
  alligator clip. The drawing goes in `ui/looks/keyjack.rs` beside `ratchet.rs`, ported
  from the mock's `kj::front`. Inspect shows the drawing only, as for other items with a
  drawing (the mock's text lines under it are not built).
- **MODDING · KEYJACK** draws the **back opened** instead of the front dimmed: the board,
  the KJ4 chip, the capacitors, the 3V cell and the fuse holder (mock `kj_back`), the
  socket boxed as sockets are. The fitted fuse draws in the holder with its own fitted
  drawing; an empty holder draws as empty. This is generic: an item look may give a back
  drawing; the window picks the back in modding mode and the front otherwise, and a
  look with a back places its sockets (and so its fitted pieces) on the back only.
  Looks without one keep today's dimmed front.
- The tray and the socket lines are the blueprint's own, unchanged: the tray labelled
  HARDWARE with `←→ hardware · space fit · esc back`, as for the Ratchet and decks.
- No stickers on the Keyjack: it is carried, not worn, and stickers go on worn items.

## 7. In the pack

- The Keyjack lists like any moddable item; its card says which fuse is in it or "no
  fuse".
- A blown fuse is gone; there is no burnt-fuse item.

## 8. Every drawn character

The Keyjack's panel and drawings use box-drawing, block and the symbols the game already
draws (`●`, `▸`); nothing that fails `every_drawn_character_has_a_glyph`.

## 9. Jobs a runner can finish

Fixers offer Copy, Plant and Vault (Net jobs) only to a runner with a cyberdeck in the
DECK slot, the deck the Net uses. Fetch and Deliver are offered to everyone: a Fetch's
flat opens with the Keyjack, a journal code, a keycard or the Net. A job already offered
or taken stays. The fixer card's standing ladder marks those rungs "needs a deck" for a
runner without one, so the gap is never silent.

## 10. Saves

- `KeypadState` gains `faults: u8` (`#[serde(default)]`).
- `credentials::Source` gains `Keyjack`; old saves never hold it.
- The new items, socket kind and mods are content; a Keyjack copy and its fitted fuse are
  saved as item copies already are.
- A run saved before this has no Keyjack; nothing gives it one (new games do).
- `runs/fixtures/` keep opening.

## 11. The manual

- `manual/` gains the Keyjack where doors and keypads are described, and in the deck
  chapter where Inspect, modding and the workbench are: the clip key, the game, faults,
  fuses, ratings, and what Lockpick and Skeleton key do to it.

## 12. Tests

- The run: hits key the code's digits in order; a miss is a fault and repeats the beat;
  the fourth hit opens the door and learns the code (source Keyjack).
- The last fault locks the pad for `LOCKOUT_MINUTES` and removes the fitted fuse.
- Faults survive pulling it off and clipping it on; they clear on opening and when the
  lockout ends.
- No fuse: the line greys, `k` logs the line and starts nothing. Locked pad: `k` refused.
- The tuning: rating zones per beat, Lockpick and wideband widen, Skeleton key and
  ceramic add faults, slow-blow slows; zones clamp to 1..bar.
- A lock maker's rating comes from the device theme.
- The kit: every build starts with a Keyjack with a stock fuse fitted and two spares.
- A finished Fetch pays a stock fuse.
- The recipes build a Keyjack (no fuse) and each fuse.
- Modding draws the back with the fuse socket and the fitted fuse; fitting and removing
  a fuse work; the front draws no fitted piece.
- The fuses' cards read their mods ("no bonus" for the stock fuse); content validation
  accepts a mod-less fuse and still rejects other mod-less hardware.
- The panel shows one fault light per allowed fault.
- Fixers don't offer Copy, Plant or Vault to a runner with no deck in the DECK slot;
  the ladder marks them "needs a deck".
- Saves: a pad with faults reopens with them.
- Shots: an ignored `keyjack_shots` test draws the clip line (fused and not), a beat, a
  miss, the fuse blowing, the door cracked, the Inspect front, the back with the tray,
  and the back after a blown fuse, at 120×40 and 100×30.

## Calls I made

- **Faults stick to the door** in the pad's journal state, so pulling the Keyjack off
  can't reset them.
- **The run opens and locks the door as the pad does**: success learns the code with
  source Keyjack; failure locks it for the pad's lockout. Both through new
  `credentials` paths beside `try_code`.
- **The kit fits hardware generically** into a kit item with a free socket; two spare
  stock fuses go in every kit, and a finished Fetch pays one more, so a runner who
  can't build fuses keeps a way in.
- **One fuse drawing tinted per tier**, mine, since the mock drew the fuse only inside
  the Keyjack.
- **Fuse bonuses live on the fuses' mods** in `content.ron`; `keyjack.ron` holds the
  game's base numbers, the skills and the ratings. The stock fuse has no mod and reads
  "no bonus"; validation allows that for fuses only.
- **No stickers on the Keyjack**, no text under its Inspect drawing, the shared HARDWARE
  tray wording.
- **One fault light per allowed fault** on the run's panel.
- **Lock ratings come from the pad's maker** (the device theme): street 1, municipal 2,
  neon 2, megacorp 3.
- **The numbers in §4** and the item values in §1.
- **Lockpick widens zones; Skeleton key adds a fault.**
- **Net jobs need a deck in the DECK slot** (§9), so #6's done-when holds while
  non-netrunners have no way into servers; the standing ladder says so.
- **A built Keyjack comes without a fuse**; fuses are their own recipes.
- **Modding shows a look's back** when the look has one; that is generic.
