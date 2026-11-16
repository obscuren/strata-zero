# Stealth: the cloak (#36)

A cloak hides you from guards, cameras and the street while you line up the first hit.
It drains a pool while it's on; the hit that ends it lands as an ambush. It comes from the
Stealth skill, as an ability that drains FOCUS, or from a field scrambler, a device with a
cell of its own. Vanish, from the talent tree, breaks a fight and cloaks you to get away.
While cloaked, the map drifts: the picture splits into pink and cyan channels, the way the
scrambler's interference looks from inside it.

Direction from Jeff; every rule he didn't fix is marked **(call)**, my call, confirmed with
him in outline before building.

## Why

The game draws sight roughly: ASCII has no true line of sight. So stealth isn't a
sightline system, it's a tool with a cost: switch it on, get where you want unseen, and
strike first. Guards on floors (#33) made the first hit matter: a guard dropped before
its turn never radios. The site jobs that follow (#98 and on) need a way through a
guarded building that isn't a gunfight. This is it.

## Today

- Guards see by a floor's light (#33); street foes by a clear line; floor and street
  cameras warm a building's trace once a game minute (`cameras.rs`); walkers' sight is a
  readout only (`exposure.rs`).
- No ability is toggled or sustained; abilities are `Passive` or `Active(…)` and firing
  one stamps a cooldown (`Character.fired`).
- A skill grants nothing; abilities come from innate, categories, worn items and chrome,
  and talents (`Character::abilities`).
- FOCUS only comes back by sleeping.
- No item holds a charge.
- The STREET talents Ghost, Shadow step and Blind spot set flags nothing reads. Vanish is
  a skill (Cool 7) that does nothing.
- The mission "daze" (`missions/daze.rs`) and the HOLO call's glitch
  (`missions/face.rs`: `fringe`, `slip`, `burst`) already distort the map as cells; the
  veil is built the same way.

## Goals

1. **The cloak.** While it's on, nobody and nothing on foot sees you: guards, street
   foes, floor and street cameras, walkers. No spotting, no radio, no camera heat.
2. **It's for the first hit only.** Your first attack from the cloak is an **ambush**: it
   can't miss and does double damage. The cloak ends on that attack. It plays no part in
   a fight once one is on.
3. **Two sources, two pools.** The Stealth skill gives the Cloak ability, which drains
   FOCUS. The field scrambler, carried, gives the same cloak from its own cell, which
   recharges.
4. **Vanish.** A talent ability: in a fight, on your turn, it ends the fight, every foe
   that was after you loses you, and you're cloaked for a few seconds, free. Long
   cooldown.
5. **The veil.** The agreed look (`2026-11-15-stealth-mock.html`, the drift): going in,
   holding, burning out, switching off, all one effect turned up or down.
6. **The STREET talents read.** Ghost, Shadow step and Blind spot do what their words say.

## Non-goals

- Suspicion and alert states, noise beyond gunshots, a site locking down floor by floor,
  guards closing in: a separate ticket (filed with this branch, see below).
- The Net: the cloak is on foot only. The Mask program and #55 (Stealth's bonus on Mask
  doing nothing) are untouched.
- Shops or buying the scrambler.
- Smoke, and the rest of the STREET tree.

## Anti-goals

- A cloak that keeps working once a fight is on.
- A cloaked runner seen, radioed or caught on camera.
- A veil that moves your `@` or your row, or hides a guard that's really there.
- A veil only the window draws: it's cells, the same in the terminal.

## The rules

### Switching it on and off

- **On:** from the ability bar (`[1]`–`[5]`) or the scrambler's own key **(call: the
  ability slot it grants)**, on foot, calm. Refused with a line in a fight ("not in a
  fight"), jacked in, on the Net, or with an empty pool.
- **Off:** press it again. Or it ends by itself when:
  - you attack (the ambush);
  - the pool runs dry (the burnout);
  - a fight starts by any other way, you jack in, or you go onto the Net (switched off).
- Moving, opening doors, taking stairs and lifts, crossing into a building: it holds.
- Only one cloak at a time. If you have both sources, the ability bar's Cloak uses
  FOCUS and the scrambler's uses its cell **(call)**.

### What it hides you from

- `foes::sees` and `foes::look` don't see you: guards don't spot you or radio, street
  foes don't start a fight. A guard already hunting you keeps its last `seen` and loses
  you as it would losing sight.
- Floor and street cameras don't see you (`cameras::seeing`): no heat.
- Walkers don't see you (`exposure::of` lists no eyes).
- Your body left jacked in is not cloaked (you can't jack in cloaked anyway).
- You can still bump into people, and a gunshot is still heard (`guards::hear`).

### The ambush

- The first attack you make while cloaked, a shot (`combat::fire`) or a strike
  (`combat::strike`), is an ambush: every round of it hits and does double damage
  (**call: in `combat::volley`, the one place your rounds roll**). The log says
  "AMBUSH". The cloak ends with it.
- A thrown grenade from the cloak ends it with no ambush bonus **(call)**.
- A guard downed by the ambush never radios: that's #33's rule already.
- A fight starts as it would after that attack (the target hunts, others hear a shot).
- **The footer** shows "AMBUSH · your first hit lands, ×2" while you're cloaked with a
  foe in your weapon's reach.

### The pools

- **The ability (FOCUS):** 1 FOCUS a second while on **(call)**. With FOCUS at 30–51,
  that's a long sneak or several short ones a night; FOCUS still only comes back by
  sleeping, so it's scarce on purpose.
- **The scrambler (its cell):** a full cell lasts 20 seconds of cloak, 30 if you have the
  Stealth skill (its proficiency) **(call)**. It recharges while off, on the world clock:
  empty to full in 30 game minutes (about a minute and a half on foot, since a game
  minute on foot is 3 seconds), and a night's sleep fills it **(call)**. The charge lives on the item copy (`ItemCopy`, a new
  `#[serde(default)]` field), so two scramblers have two cells.
- **Blind spot** halves the drain on either pool while you stand still.
- The drain counts real time on foot (frame ticks while the map runs), not turns
  **(call)**: there are no turns while cloaked.

### Vanish

- Granted by **Shadow step** rank 1, in place of its unread flag. It's an `Active`
  ability like the others, with a 120-second cooldown **(call)**; Shadow step rank 2
  sharpens the cooldown by 30 seconds instead of its Cool point **(call)**.
- Used in a fight, on your turn: the fight ends (`net::disengage`), and every foe hunting
  you loses you: every foe on your place, and every guard of the building you're in, on
  any floor. Guards go back to their rounds; others calm. Vanish is a clean getaway: it
  also clears a guard's `spotted` (the call it hadn't made yet) and its `follow`. A guard
  that already radioed has radioed; the alarm stays on **(call)**.
- You're then cloaked for 4 seconds that cost no pool, drawn at a calm drift (heat 0.2),
  then the cloak switches off **(call)**.
- Usable only in a fight **(call)**; out of one, the cloak is the tool.
- The Vanish *skill* stays as it is: its words go with Shadow step.

### The other talents

- **Ghost** rank 1: cameras see a smear. A camera that sees you uncloaked warms the trace
  at half rate (1 a minute, not 2). Rank 2 keeps its +1 sight.
- **Blind spot** rank 1: the drain halves while you stand still (above). Rank 2 keeps
  its +1 sight.

### The Stealth skill grants Cloak (call)

- `Skill` gains `grants: Vec<AbilityId>` (`#[serde(default)]` in content), folded into
  `Character::abilities` as `Source::Skill`. Stealth grants `cloak`. So whoever owns
  Stealth (the GHOST category's free skill) has the ability.

### The field scrambler (call)

- A new item, carried in the pack like the Keyjack (no slot), granting the cloak from its
  cell while carried. Made at the workbench (a recipe) and given by the console; kits and
  fixers can hand it out later.
- Its look card says its charge.

## The veil

The look agreed with Jeff, mock `2026-11-15-stealth-mock.html` (keep it beside this
spec), drawn as cells after the map's layers and your `@`, before the HUD, where
`daze::draw` runs:

- **The drift.** Every wall, furniture and foe glyph throws a pink copy `split` cells to
  its left and a cyan copy to its right, drawn only on empty floor; where both land on one
  cell it shows near-white. A band of rows shifts sideways, rolling down the map; now and
  then the whole map slips one cell for a frame. A faint cyan wash and scanline over the
  map. Your `@` and your row never move, and your `@` never throws or takes a copy; your row's empty floor can take one, as the mock draws it.
- **Strength** (`heat`, 0–1, past 1 out of control) is how empty the pool is: at full
  the split is 1 cell and faint, one slow row in the wave; near empty, 3 cells,
  shivering and bright, a thick fast band. Everything grows smoothly with it.
  - Scrambler: how empty its cell is.
  - Ability: how much of the FOCUS you had when you switched it on is spent, so a cloak
    started on little FOCUS still starts calm **(call)**.
- **Foes stay where they are.** The drift smears copies of a guard's glyph onto empty
  floor and shifts rows, but after it, every standing foe you can see is drawn again on
  its true cell, so the veil never hides or moves one (the anti-goal). This makes the pass
  tile-aware, not buffer-only like `daze::draw` **(call)**.
- **Going in** (about 1 s): the cloak sputters in, frames of the drift over the plain
  map, wide at first, then settles to its resting drift.
- **Burning out** (about 2.5 s): the drift keeps going past empty (split wider, wave over
  the map, slips often), then sputters, plain frames more and more often, until it's
  gone. The HUD flashes `FAILING` and the footer `SIGNAL LOST` through it. You're in the
  open from the moment it starts. The FOCUS cloak burns out the same way when FOCUS hits
  0 **(call)**.
- **Switching off** (half a second): the drift eased out, nothing else. Also how an
  ambush, a fight starting, or Vanish's cloak running out ends it.
- Randomness comes from `hash64` of the tick, as `face::burst` does, so shots are fixed
  frames.

### The HUD

- **Scrambler:** the top-right line reads `◌ FIELD SCRAMBLER 62%` (pink below a quarter);
  the XP row shows a `CELL` bar and the seconds left while cloaked; after a burnout,
  `◌ field scrambler 0% · burned out` until it recharges past a tenth.
- **Ability:** the FOCUS bar drains as you watch; no CELL row.
- **Footer:** `▒▒ SCRAMBLED ▒▒` (scrambler) or `▒▒ CLOAKED ▒▒` (ability) where
  `ENGAGED` would be; the AMBUSH line when a foe is in reach.
- **Ability bar:** Cloak's slot shows on while it's on.

## Saving

- The cloak state (on, its source, when it started, Vanish's free seconds) is play
  state and saves (`#[serde(default)]`); a reopened run comes back cloaked, without the
  going-in animation.
- The scrambler's cell saves on its item copy.
- The veil's animation phase is UI and doesn't save.
- Fixtures keep opening.

## The manual

- 07 (turns and fights): the cloak, the ambush, Vanish.
- 06 (the city): cloaked, cameras and guards don't see you.
- 02 (your runner) or wherever skills and talents are described: Stealth grants Cloak;
  the STREET talents.
- The item and the workbench recipe where items are described.

## Follow-up ticket

File with this branch: "Suspicion, noise and lockdown floor by floor" — the alert
states, noise beyond gunshots, a site locking down floor by floor and guards closing
in on an alarm, from #36's original list, set aside for the simple cloak.

## Left to the builder

- Module layout (a `cloak` module), the exact HUD placement within the rows named, the
  ability's content entry and its key.
- Constant values within the calls above.
- Shots: holding at full and low, going in, burning out, from an office copy.

## Calls made

1. 1 FOCUS a second for the ability; the scrambler's cell lasts 20 s (30 with Stealth)
   and recharges in 30 game minutes.
2. The drain runs on real time on foot.
3. The ambush is every round of the first shot or strike: sure hits, double damage.
4. A thrown grenade ends the cloak without an ambush.
5. Vanish from Shadow step rank 1; 120 s cooldown, rank 2 −30 s; 4 free seconds of
   cloak; fights only.
6. Ghost halves camera heat uncloaked; Blind spot halves the drain standing still.
7. Skills can grant abilities; Stealth grants Cloak.
8. The scrambler is a carried pack item, made at the workbench.
9. With both sources, the ability uses FOCUS and the scrambler its cell.
10. The scrambler recharges in 30 game minutes on the world clock; sleep fills it.
11. Vanish shakes every foe hunting you on your place and every guard of the building,
    and clears their unmade radio calls; a call already made stands.
12. The veil redraws every standing foe on its true cell after the drift.
13. The FOCUS cloak's heat is measured from the FOCUS you started with, and it burns
    out like the scrambler.
