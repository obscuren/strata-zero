# Ranged combat on the street — design

Date: 2026-10-19. Status: built (branch `worktree-ranged-combat`; after the review
`t` cycles foes so `tab` keeps the deck, a miss always clears its target by a tile,
the lot's clock reads 18:15 whatever the session's ticks, and the `s` key refuses
the lot as the pause menu does). Designed with Jeffrey in the visual companion (terminal
cell mockups of the burst, animated), approved in chat ("it all looks sick", the
three looks assigned to pistol, SMG and abilities, the ability and the lot "perfect"
/ "great yes"). Spec, plan and build run on without check-ins.
Builds on: `2026-10-17-engagement-design.md` (turns, pools, the aimed walk, the
footer's four rows, `e`), `2026-10-15-ice-that-hunts-design.md` (sight, hits,
stun, FLATLINED), `2026-10-14-ability-fx-design.md` (the `Fx` vocabulary on
content), `2026-10-19-smg-sockets-design.md` (the Ratchet, its hardware).
Art reference: `2026-10-19-ranged-combat-mock.html` beside this file, the mockup
Jeffrey approved. It is the truth for every glyph, colour and tick below; where
this text and the mock disagree, the mock wins.

## Goal

The street gets the net's fight. Walk into a goon's sight, or press `e`, and the
board goes to turns with the same two pools, the same initiative order, the same
aimed walk and paced enemy turns the net already has. Then aim a cursor at the
goon, press space, and watch three rounds leave the Ratchet: the muzzle flash, the
tracers, the goon flashing red, the number lifting off them, a miss sparking off
the wall behind. The goon walks in and shoots back, one tracer at a time. A new
title entry drops you on an empty lot with one goon in sight to try it.

The looks are not combat code: they are `fx` lists on the weapon or ability that
fires, like netscan's wave is today, so a pistol, an SMG, a rifle and an ability
each read differently from the same engine.

## Decisions taken with Jeffrey

- **One rulebook.** The net's engagement carries over whole: real time until
  engaged, then `MOVE` and `AP`, move half then act half, `.` ends the turn, `e`
  engages by hand and calms when nothing hunts you. The street and the net feel
  the same.
- **Each round rolls on its own.** A burst is three rolls, three rounds drawn;
  a hit lands on the target, a miss runs on and sparks where it stops.
- **Damage numbers**, per round, lift off whoever was hit. The net gets them too.
- **Aim with a cursor, space fires.** No auto-pick.
- **The goon**: a pistol, closes in, shoots back.
- **Three looks, assigned** (the mock's A, B, C): the **tracer** is the pistol's
  (the goon's, and any single-shot gun); the **streak** is the Ratchet's, with the
  big muzzle flash Jeffrey called out; the **tear** (the net's glitch on the hit,
  the number as a tag) is for abilities. All three are shapes any weapon or ability
  may pick in content.
- **Static**, the first combat ability: noise pushed into the goon's chrome;
  damage and a turn of stun; the tear look.
- **NEW GAME (COMBAT)**: a title entry that puts D'SHAWN on a flat lot with a goon
  12 tiles off, outside the goon's sight of 8.

Calls made after the builder check (no round of questions; Jeffrey asked for no
stops): the Ratchet reaches 12 tiles (10 at first; Jeffrey asked for more after
playing) and the scrap nine 8, the goon has 30 HP and Reflex 5 (20 and 4 at
first, likewise), so you outrange the
goon by a step and the readout example below is what the formula gives; a stim on
your act half costs an AP; the lot starts at 18:15, the dusk cast's hour (19:30 is
plain night in the lighting code); saving does nothing to an engagement, in the net
or on the street; your death stays on `net.death`, one field for two causes.

## 1. Content and model (`game`)

### Weapons

```rust
pub struct Weapon {
    pub skill: SkillId,      // the roll's skill
    pub damage: u8,          // per round that lands
    pub rounds: u8,          // per fire action; 1 for a pistol, 3 for the Ratchet
    pub range: i32,          // tiles, Chebyshev; beyond it the shot is refused
    pub fx: Vec<Fx>,         // what one round looks like, in order
}
// Item gains:
#[serde(default)] pub weapon: Option<Weapon>,
```

```ron
Item(id: "ratchet_smg", ..., weapon: Some(Weapon(skill: "handguns", damage: 4, rounds: 3, range: 12,
     fx: [Flash(big: true), Tracer(colour: (255, 170, 40), speed: 3), Chips, Spark, Number(tag: false)])), ...),
// The Ratchet first carried the streak; after playing, Jeffrey wanted the pistol's
// tracers on it, three to a burst. The streak stays Static's.
Item(id: "scrap_nine", name: "Scrap nine", kind: Weapon, slot: Some(Primary), weight: 0.9, value: 320,
     weapon: Some(Weapon(skill: "handguns", damage: 5, rounds: 1, range: 8,
     fx: [Flash(big: false), Tracer(colour: (255, 170, 40), speed: 3), Chips, Spark, Number(tag: false)])),
     desc: "Nine millimetres of somebody else's problem. The serial was never there."),
```

- Validation: `weapon` only on `ItemKind::Weapon` with a `Primary` or `Secondary`
  slot; `rounds ≥ 1`, `range ≥ 1`, `damage ≥ 1`; the skill exists. A weapon
  without `weapon` (the shiv) is not shootable: `nothing to shoot with`.
- The Ratchet's weight, value, look, sockets and text stay.

### The `Fx` vocabulary grows

```rust
pub enum Fx {
    Kick(u64), Wave {..}, Glow {..}, Bolt {..}, Mark {..},      // as today
    /// A round in flight: a bright head and three trailing cells, `speed` tiles a tick.
    Tracer { colour: (u8, u8, u8), speed: u8 },
    /// A round drawn whole from muzzle to impact: white, then the colour, then ember, gone.
    Streak { colour: (u8, u8, u8) },
    /// The muzzle: the firer's tile lit and the two cells toward the target; `big` adds
    /// the cells above and below the muzzle.
    Flash { big: bool },
    /// A hit: the target lit red, chips flying off it.
    Chips,
    /// A miss: a spark where the round stopped.
    Spark,
    /// A hit: the cells around the target torn into the net's glitch set for a beat.
    Tear,
    /// The damage lifting off the target; `tag` draws it white on a red field.
    Number { tag: bool },
}
```

`Chips`, `Spark`, `Tear` and `Number` are **impact shapes**: the engine plays them
at the impact tile when the round lands there, `Spark` only on a miss, the others
only on a hit. `Flash` plays at the firer. `Tracer` and `Streak` are **flight
shapes**, one per list; the impact shapes start when the flight arrives. An
ability or weapon lists what it wants; order does not matter.

Validation: a weapon's `fx` has at most one flight shape; an ability's `fx` may hold
the new shapes only on a `Strike` ability (below).

### Abilities that strike

```rust
pub enum Active {
    Scan {..}, Ping {..}, Trace {..},
    /// Hits one target in sight within `range`: `damage` on a hit, `stun` turns of stun.
    Strike { range: i32, damage: u8, stun: u8, skill: SkillId, ap: u8 },
}
```

```ron
Ability(id: "static", name: "Static", cooldown: 240,
        kind: Active(Strike(range: 8, damage: 6, stun: 1, skill: "hacking", ap: 2)),
        fx: [Flash(big: false), Streak(colour: (0, 229, 255)), Tear, Number(tag: true)],
        desc: "A burst of noise shoved down whatever is looking at you. Their chrome forgets what it was doing."),
```

- `neural_port` grants `static` beside `ping`.
- `Fire(static)` outside an engagement, or with no target under the cursor, is
  refused: `nothing to hit` / `pick a target`. It costs `ap` action points (both
  of D'SHAWN's). The cooldown stays in ticks as every ability's does.

### The roll

In `game::run` (terminal-free), beside the net's `succeeds`:

```rust
/// The chance one round lands, in percent: reflex and the skill carry it, every
/// tile past half range costs five, never below 5 nor above 95.
pub fn hit_chance(reflex: u8, skilled: bool, dist: i32, range: i32) -> u8;
//  50 + 4·reflex + (skilled ? 10 : 0) − 5·max(0, dist − range/2), clamped 5..=95
pub fn round_hits(roll: u8 /* 1..=100 */, chance: u8) -> bool;   // roll <= chance
```

For you: reflex is `stat(Reflex)` (hardware mods counted, so the Redeye's +1
counts), skilled is `has_skill(weapon.skill)`. For a foe: its `reflex`, skilled
always. Static rolls the same way with `Net` for reflex and `hacking` for the
skill. Damage is the weapon's or the ability's number, flat; armour and cover are
not in this spec.

### Characters

- `Character::debug_runner` is unchanged. The COMBAT start (§5) adds to it.

## 2. Foes (`strata-zero`, `foes.rs` new)

```rust
pub struct Foe {
    pub name: String,            // "goon"
    pub place: Place, pub pos: Pos,
    pub hp: u8, pub hp_max: u8,
    pub reflex: u8,
    pub weapon: ItemId,          // "scrap_nine"
    pub sight: i32,
    pub mode: FoeMode,           // Idle | Hunting { seen: Pos, lost: u8 }
    pub stunned: u8,
    pub down: bool,
}
// AppState gains: pub foes: Vec<Foe>   (session-only; enter_city clears it)
pub const GOON_HP: u8 = 30; pub const GOON_REFLEX: u8 = 5; pub const GOON_SIGHT: i32 = 8;
pub const GOON_MOVE: u8 = 3; pub const GOON_AP: u8 = 1;
```

- **Sight**: a foe sees you when you are within `sight` (Chebyshev) and the
  Bresenham line between you crosses no wall, door or window of the place (the
  street's `Terrain::Wall | Door | Window`; interiors later). Seen → `Hunting {
  seen: your tile, lost: 0 }`. Nothing is halved by a mask on the street.
- **A foe that sees you engages** the place, as a waking piece engages a construct;
  the log reads `ENGAGED · the goon has you` in magenta.
- **Lost**: on reaching `seen` without sight, `lost += 1`; at `LOST_TURNS` (the
  net's 3) the foe goes `Idle` where it stands.
- **Down** at `hp == 0`: `down = true`, the tile is walkable again, the log reads
  `goon down`; the tileset draws a down foe `%` in dim red `(120, 30, 30)` and a
  standing one `@` in red `(255, 80, 80)`, bold while hunting. A stunned foe draws
  dim.
- A foe is drawn by `render/people.rs` beside the walkers, only in sight (a down
  one is remembered like a parked car).

### The foe's turn

On the engagement's pace (`PACE_TICKS`), through `foe_step`, the twin of
`ice_step`:

1. Stunned: the stun counts down one, the turn passes (`goon · stunned`).
2. Look. Idle and seeing you → Hunting.
3. Hunting, you in `range` of its weapon with a clear line of fire: **fire**
   (one round, the weapon's `rounds`), spend the AP; with no AP left it holds
   where it stands (Jeffrey, after playing: "if he's in range there's no point
   in moving in").
4. Else with moves: one A* step toward `seen` over walkable tiles not held by you
   or another foe; look again after the step.
5. Nothing to do: the turn ends. A foe with both pools spent ends its turn as a
   piece does.

The footer's turn line reads `goon moves 2` while it walks and `goon fires` while
it shoots, as the net reads `grey ICE hits`.

## 3. The engagement, loosened (`net.rs`)

```rust
pub enum Actor { You, Ice(usize), Foe(usize) }
pub struct Engagement { pub place: Place, /* was building: BuildingId */ ... }
```

- Every reader of `building` matches `place`: `Place::Construct(b)` keeps every
  ICE path as it is; `Place::Stratum(Street)` (and any floor later) is the street
  fight. `pools_for`: a foe gets `GOON_MOVE / GOON_AP`.
- **Initiative**: in a construct as today. On the street: you by `Reflex`, each
  standing foe in sight memory by its `reflex`; higher first, ties to you, then by
  index. Rolled once at the start; a foe that starts hunting mid-fight joins at the
  end (`seat_standing`'s twin).
- `end_turn`'s round end: on the street, when no foe is hunting and the fight is
  not manual, it ends (`calm`). A down foe never hunts.
- `e` on the street: calm → engages by hand when any foe stands in sight memory
  (`manual: true`), else `nothing to engage` as today; engaged → calm only when no
  foe hunts, else `still hunted`. The console's `engage` / `calm` work on the street.
- `spend_ap`, `spend_move`, `stay`, `phase`, `aim_cap`, `your_turn` are place-blind
  already and stay so.
- **Your turn ends on its own** `AUTO_END_TICKS = 20` ticks after your last action
  point goes (`Engagement.spent_at`), on the street and in the net alike, so the
  numbers can be read before the board moves on (Jeffrey, after playing). `.`
  still ends it at once.
- Jacking in while engaged on the street is refused: `not with a gun on you`.
  Entering a building door while engaged: refused, `not now`. Saving does not
  touch an engagement (it never did in the net either).
- **Using an item** (a stim from the deck) while engaged costs one AP through the
  same door as firing: refused on the move half (`move first · space to stay`),
  off your turn, or with no AP left.
- **Death**: your `hp` at 0 on the street sets `net.death = Some("a goon on the
  street")` and the FLATLINED screen takes over as it does for black ICE; the
  field keeps its home on `NetState`, one door for both causes; the failsafe stays
  the net's.

## 4. Firing (`combat.rs` new, `explore.rs`)

### The target cursor

On your act half, on the street:

- `hjkl` / arrows move a **target cursor** (`MapScreen.target: Option<Pos>`), the
  look cursor's glyphs, cyan; it starts on the nearest standing foe in sight when
  the act half opens and is dropped by `esc` or the end of the turn. `t` jumps to
  the next foe in sight, nearest first, wrapping.
- With the cursor on a standing foe in sight: the line of fire draws from you to
  it in dim cells (`·` at `(90, 90, 90)`), the foe's cell inverts (black on cyan),
  and a readout sits two rows above the foe, cyan on `(0, 20, 26)`: ` goon · pistol
  · 9 tiles · 85% ` (its name, its weapon, the distance, **your** chance per round
  with the worn weapon; `out of range` in place of the chance beyond `range`; `no
  line` when a wall is between). The mock's top frame shows it.
- `space` **fires** the worn `Primary` weapon at the foe under the cursor (one AP,
  through `spend_ap`); refused with `nothing to shoot with` (no `weapon`), `out of
  range`, `no line`, `pick a target` (cursor not on a foe). The number keys fire a
  `Strike` ability at the foe under the cursor the same way, spending its `ap`.
- In the move half the aimed walk works as in a construct (`hjkl` aims a path,
  `enter` walks it); `space` stays, as today.
- Row 2 of the footer, the prompt, reads `aim  hjkl target · t next · space fire
  · 1-5 ability · . end turn` on the act half and the net's move prompt on the move
  half.

### A fire action

`combat::fire(st, from: Pos, shooter: Shooter, target: usize /* foe */)` and
`combat::fire_at_you(st, foe: usize)` share one path:

1. For each of the weapon's `rounds` (k = 0..): roll `hit_chance`; on a hit, the
   impact is the target's tile and `damage` comes off at once (the outcome is
   known when the trigger is pulled; the picture follows, as the net's bolts do);
   on a miss the impact is the first wall or the last walkable tile along the line
   past the target, at most `range` tiles from the shooter.
2. Push the weapon's `fx` for that round with `started = now + k · ROUND_GAP`
   (`ROUND_GAP = 4` ticks), origin the shooter, `targets = [impact]`, the instance's
   new `label` the number (`-4`) and `hit` the roll's outcome. Flight shapes start at
   `started`; `Flash` at `started`; impact shapes at `started + flight` where
   flight is `ceil(dist / speed)` for a tracer, `1` for a streak, `0` without one.
3. The log, one line per fire action: `you fire · goon 4 · 3 · miss` / `goon fires
   · miss` / `goon fires · 5 · hp 23`.
4. A foe at 0 goes down; you at 0 die (§3). Static adds `stunned = stun` on a hit:
   `goon stunned`.

`FxInstance` gains `label: String` (empty for every shape but `Number`) and
`hit: bool`; `until()` covers the new shapes by their fixed lengths below.

### Numbers in the net

`net::hit` (an ICE hit on you) and the attack program's successful turn push
`Number { tag: false }` on the target with the damage (`-6`, `-10 focus` reads
`-10`, heat reads `+5`), in the bolt's colour, starting when the bolt lands
(`ROLL_TICKS`). Nothing else in the net changes.

## 5. The lot and the title (`citygen`, `title.rs`, `app.rs`)

- `City::lot(seed) -> City`: `City::empty` with the Street stratum painted: a
  36×14 rectangle of `Road` from `(2, 2)`, `Sidewalk` around it one tile wide, a
  `Wall` ring around that; the rest `Void`. No districts, buildings, connectors or
  net. `name: "the lot"`. The map screen reads `on foot · the lot` and the side
  panel's place line the same; nothing else of the HUD assumes a district (the
  builder checks the inspector, HERE and the minimap on a city with none).
- Lighting: the lot starts at **18:15** (`START_MINUTE_OF_DAY` is the city's
  22:00; `enter_city_at(city, minute)` takes the lot's own), inside the dusk cast,
  which peaks at 18:00 and is gone by 19:00, so the lot is amber and the tracers
  still read.
- `sim::World::seed` on a city with no buildings yields no people; the builder
  proves it rather than special-cases it.
- **Title**: `NEW GAME (COMBAT)` under `NEW GAME (DEBUG)`. It builds
  `debug_runner`, then: wears a `neural_port` (Static), learns `handguns`, gives
  three `stim_patch`, enters `City::lot`, spawns the player at `(6, 8)` and one
  goon in the lot's far corner at `(38, 16)`, `Idle` (it started twelve tiles
  off at `(18, 8)`; Jeffrey moved it after playing). A walk east along the road
  brings you into its sight at `(30, 8)`; `e` engages from anywhere it is in
  sight memory. Switches with the glitch, no generating screen.
- Save on the lot: `SAVE` refuses with `nothing to save on the lot` (a lot is a
  city with no districts). Load works as ever.
- Console: `foe [n]` spawns a goon `n` tiles east of you (default 6) on any
  street, `dummy [n]` a training dummy (`Foe::dummy`: `inert`, unarmed, sight 0,
  drawn `&`; it never hunts, moves or fires, and no round gets through, though
  the numbers still lift off it; added after Jeffrey asked for "a stationary
  dummy that is invulnerable so I can keep on testing"); `foes` lists them
  (`0 goon hunting 24/30 at 18,8`); `hp` as today. The lot starts with a dummy
  at `(6, 2)` beside the goon.

## 6. The footer and the sidebar (`hud.rs`)

- Row 1, right: `ENGAGED` in magenta bold while engaged on the street (the net
  keeps `HUNTED`; on the street the fight is the word).
- Row 3: `TURN 1  YOU › goon` as the net's; right, the foes in sight memory,
  nearest first: `▣ goon · hunting · 24/30 · 9 tiles` (`· stunned 1`, `· idle`,
  `· down` kept). In red `(255, 80, 80)`.
- Row 4: `MOVE`/`AP` as the net's; hollow keys on the foe's turn.
- HERE in the sidebar lists the same foe lines under the place.

## 7. Drawing the shapes (`render/fx.rs`)

Ticks and colours from the mock. `T` is the tick the instance started, `e` the
ticks since. Every shape clears DIM on the cells it touches.

- **Flash** (2 ticks): the firer's cell keeps its glyph on `(255, 240, 180)` then
  `(255, 170, 40)`, black foreground, bold; the next cell toward the target `✦`
  then `*` in `(255, 230, 120)` bold; the cell after `·` `(255, 200, 90)` on the
  first tick only. `big`: on the first tick `` ` `` above and `,` below the `✦` cell in
  `(255, 200, 90)`. Toward: the sign of the greater axis, horizontal on a tie.
- **Tracer**: along the Bresenham line from the firer to the impact, head at
  cell `1 + speed·e`; the head `•` in `(255, 240, 200)` bold; the three cells
  behind it `─` (`│` on a vertical step, `╲` or `╱` on a diagonal one) in
  `(255, 170, 40)` bold, `(200, 110, 20)`, `(110, 55, 10)`, never behind the
  firer. Ends when the head reaches the impact.
- **Streak** (3 ticks): every cell from the one past the firer to the one before
  the impact, `━` (`┃` vertical, `╲` `╱` diagonal) in `(240, 240, 240)` bold, then
  the colour, then `(120, 60, 10)`.
- **Chips** (3 ticks): the target cell inverted, white on `(255, 40, 40)` then
  `(160, 20, 20)`, bold, for two ticks; chips `'` `·` `,` `·` `` ` `` `·` at `(+1,
  −1) (+1, 0) (+1, +1) (+2, −1) (+2, +1) (0, −1)` relative to the target, the
  offsets' x flipped when the shot came from the right, each chip shown from
  tick `i/2`, drifting one cell outward on the third tick, `(255, 120, 120)` then
  `(180, 60, 60)`.
- **Spark** (3 ticks): at the impact `✶` `(255, 240, 160)` bold, `×` `(255,
  170, 40)`, `·` `(120, 60, 10)`; on the first tick `` ` `` above-left and `,`
  below-left in `(255, 220, 120)` (left meaning back toward the shooter); on the
  second a `·` `(200, 110, 20)` below-left two cells back.
- **Tear** (3 ticks): the 7×3 box around the target (the target's cell
  excluded); each cell whose hash of `(dx, dy, tick, instance)` is divisible by
  three shows a glyph from `ｦ ｱ ﾂ ﾈ ╳ ≋` by the hash, cyan `(0, 229, 255)` or
  magenta `(255, 0, 200)` by its parity.
- **Number** (14 ticks): the label, bold, one row above the target to start,
  rising a row every 4 ticks; plain: `(255, 235, 235)`, from tick 10 `(170, 60,
  60)`; tag: ` -4 ` white on `(170, 20, 20)`, from tick 10 `(200, 120, 120)` on
  `(80, 10, 10)`. Rounds of one burst offset one column each (k) so three numbers
  do not overprint. In the net the number takes the bolt's colour.
- Off-place, future (`started > tick`) and ended instances draw nothing.

## Out of scope

Melee, armour and cover, reloading and ammo, more foes than the goon, foes in
the real city (jobs, guards), foe loot, XP for a kill, Static on ICE, weapons in
the Secondary slot firing, saving the lot, sound, screen shake.

## Tests

game: `Weapon` parses for the Ratchet and the scrap nine; validation (weapon on
armour, zero rounds, unknown skill, two flight shapes, impact shapes on a scan);
`hit_chance` at point blank, at half range, at full range, the clamps; `round_hits`;
`Strike` parses with its fx; `neural_port` grants static. citygen: `City::lot`
validates, the rectangle's terrain, no districts. app/foes: a goon sees you at 8
and not at 9, not through a wall; seeing engages with the magenta line; a foe's
turn on the pace: stunned passes, fires in range with a clear line, steps toward
`seen` otherwise and looks again, ends with nothing left; lost after three; down at
zero and walkable; initiative by reflex with ties to you; a late foe seats at the
end; a round with no hunter calms unless manual; `e` both ways; jack in and doors
refused while engaged. combat: three rolls per burst, damage at once, impact tiles
on a hit and a miss (wall, range cap), the fx instances with their starts, labels
and flags; the log lines; Static's AP, stun, refusals; the foe's shot at you, your
death to FLATLINED. explore: the cursor opening on the nearest foe, `tab`, the line
of fire and the readout, `space` firing and the refusals, `esc`, the prompt
rows. hud: `ENGAGED`, the foe lines, the turn line's `goon fires`. net: a number
after an ICE hit and an attack. render/fx: each new shape at its first, middle and
last tick against the mock's glyphs and colours; a future instance draws nothing;
`until` per shape. title: the COMBAT entry builds the runner, the lot, the goon at
12 tiles, 18:15. Smoke: the COMBAT start, walk four tiles, engaged, aim, fire a
burst, the goon's turn, `.`, the goon down, calm; screenshots of the aim and of a
burst mid-flight.
