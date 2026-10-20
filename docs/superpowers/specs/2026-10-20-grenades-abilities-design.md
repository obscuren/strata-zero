# Grenades and abilities — design

Date: 2026-10-20. Status: built (branch `worktree-grenades-abilities`): the blast
engine lives in `blast.rs`, logs name the whole item, the live blink is 4 ticks,
the act half's digits are the `ACT` list only, prompt headings are upper-case and
the nine-entry row needs 140 columns. Approved in chat ("ALL OF THEM", "Yeah
great!"); spec, plan and build ran on without check-ins.
Designed with Jeffrey in the visual companion: terminal-cell mockups of the area
cursor, two throw arcs, three blast looks, four ability looks and a fused throw.
Jeffrey took every look and asked for one change: the landing cursor is the
net's pick, a cyan block with an `×` in it, not a reticle.
Builds on: `2026-10-19-ranged-combat-design.md` (the engagement on the street,
`Weapon`, the shot shapes, `Active::Strike`, `combat.rs`, `foes.rs`, the fire
cursor, the lot and the COMBAT title entry) and `2026-10-17-engagement-design.md`
(turns, pools, the footer's four rows).
Art reference: `2026-10-20-grenades-abilities-mock.html` beside this file, the
page Jeffrey approved. It is the truth for every glyph, colour and tick below;
where this text and the mock disagree, the mock wins.

## Goal

On the act half you press a number and the cursor carries a blast: a red
footprint shades under it, darker toward the centre, cut by walls, and the
readout says who is in it and for how much. Space throws. The grenade lobs or
rolls across the line, bounces, and the lot blooms, whites out or shocks. Numbers
lift off everyone in it, you included if you stood too close. A fused thing lies
blinking for a turn and the goon steps out of its footprint.

Every stat tree gets an ability with its own look: Shove (MEAT), Redline (EDGE),
Jolt (WRENCH), Deadeye (STREET) beside Static (ICE), and Static starts to work on
ICE. On screen abilities and throwables are one numbered list, `ACT`; in data
they are two sources (an ability's `Active`, an item's `Use::Throw`) cast by one
engine.

## Decisions taken with Jeffrey

- **Every look has an owner.** Lob: thrown grenades (frag, flash). Shadow: rolled
  things (EMP puck, satchel). Bloom: frag and satchel. Whiteout: the flashbang.
  Shock: the EMP, and the look a net blast would wear later. Streak + tear:
  Static and Jolt. Hit flash + `»` trail: Shove. Kick + pulse + pools flash:
  Redline. Mark + reticle: Deadeye.
- **One list on screen, two sources in data.** The `ACT` row numbers the fight's
  abilities first, then the throwables with their counts. An ability carries an
  `Active`; a throwable carries `Use::Throw`; both hand the engine a `Blast`.
- **Chebyshev radius, falloff by ring**: full at the centre, then two thirds,
  then a third. The blast reaches only tiles the landing tile can see.
- **No scatter.** The grenade lands where you aim, every time.
- **Friendly fire on.** You take the blast if you stand in it; the preview warns.
- **Fuse on content.** Frag, flash and EMP go off on landing; the satchel lies a
  turn. A foe standing in a live footprint spends its moves leaving it.
- **Each ability is granted by a talent in its tree and by one piece of chrome.**
- **Grenades are meat.** The net gets the Shock look as a vocabulary only, and
  Static on ICE.
- **The landing cursor** is the net's pick: the tile's glyph replaced by `×`, black
  on cyan.

Calls made by me (the builder may not revisit them; Jeffrey asked for no stops):
the `ACT` row drops entries off its end with `…` when they do not fit (they all
fit at 120 columns); a stun on you empties your next turn's action points rather
than skipping the turn; the satchel's rings read 16/12/8/4 by the formula below
(the chat said 16/10/5; the formula wins); Redline sends you back to the move half.

Calls after the builder check (eight questions, none Jeffrey's to answer):

- **The net keeps its keys.** In a construct row 4 stays `PROGRAMS`: the deck's
  programs on `1`–`5` as today, then the net-capable abilities (Static, Jolt,
  Redline) follow on `6`–`8` in the `ACT` style. No throwables in a construct;
  the `ACT` list is the street's. §4 reads so.
- **A throwable is thrown, never used.** The deck's `u` on one is refused: `throw
  it from the map`. `game::Action::Throw(ItemId)` consumes one from the pack and
  does nothing else; `Action::Use` on a `Throw` item refuses.
- **A live blast counts calm beats too.** Engaged it counts down at the start of
  your turn; calm, every `BEAT_TICKS` beat counts as a turn, so a satchel left
  by a fight that ended still goes off, and the console's `throw` works calm.
- **No short names.** The row spells the item's name in lower case; the `…` cut
  handles 100 columns. The mock's `flash×1` / `emp×1` were abbreviations.
- **The mock's `stun 2` caption is a flourish**, not a shape: the stun shows as
  the dim foe, the footer's `· stunned 2` and the log; the `Number` reads `stun`
  only when the damage is 0.
- **A strike on ICE is an attack.** It adds the attack program's heat
  (`noise_on`), wakes the piece, and a break runs the attack's break path
  (`land`, `sync_guards`, `kick`, `alarm_clears`); the builder routes it through
  the function `attack_ice`'s success already calls.
- **Blasts wake foes.** Any standing foe hit by a blast, or standing in a
  footprint when a live blast lands, goes `Hunting` toward you as a shot's
  target does, so the step-out rule has a `seen` to work from.

## 1. Content and model (`game`)

### A blast

```rust
/// What a blast does where it lands, from a grenade or an ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blast {
    pub radius: i32,       // tiles, Chebyshev; ≥ 1
    pub damage: u8,        // at the centre; rings take less (below); 0 for stun-only
    pub stun: u8,          // turns, every ring the same
    #[serde(default)] pub fuse: u8,      // turns it lies live; 0 goes off on landing
    #[serde(default)] pub chrome: bool,  // true: only things with chrome are hurt
}

/// How a thrown thing crosses the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arc { Lob, Shadow }

pub enum Use {
    Heal(u32),
    /// Thrown at a tile within `range`; the blast goes off there.
    Throw { range: i32, arc: Arc, blast: Blast },
}
```

Damage by ring, in `game::run`: `blast_damage(damage, radius, ring) = damage ·
(radius + 1 − ring) / (radius + 1)`, rounded down, never below 1 when `damage > 0`.
Radius 2 at 12 reads 12 / 8 / 4; radius 3 at 16 reads 16 / 12 / 8 / 4. A blast
never rolls: everything in the footprint is hit.

```ron
Item(id: "frag", name: "Frag", kind: Consumable, weight: 0.4, value: 180,
     use: Some(Throw(range: 8, arc: Lob, blast: Blast(radius: 2, damage: 12, stun: 0))),
     fx: [Throw, Bloom, Chips, Number(tag: true)],
     desc: "A tin of nails with an opinion. Count to nothing."),
Item(id: "flashbang", name: "Flashbang", kind: Consumable, weight: 0.3, value: 140,
     use: Some(Throw(range: 8, arc: Lob, blast: Blast(radius: 2, damage: 0, stun: 2))),
     fx: [Throw, Whiteout, Number(tag: true)],
     desc: "Everything goes white, then everyone asks what happened."),
Item(id: "emp_puck", name: "EMP puck", kind: Consumable, weight: 0.5, value: 260,
     use: Some(Throw(range: 7, arc: Shadow, blast: Blast(radius: 2, damage: 3, stun: 2, chrome: true))),
     fx: [Throw, Shock, Tear, Spark, Number(tag: true)],
     desc: "Slides under things and takes their chrome offline for a breath."),
Item(id: "satchel", name: "Satchel charge", kind: Consumable, weight: 1.2, value: 400,
     use: Some(Throw(range: 6, arc: Shadow, blast: Blast(radius: 3, damage: 16, stun: 0, fuse: 1))),
     fx: [Throw, Bloom, Chips, Number(tag: true)],
     desc: "A turn to think about it. Theirs, mostly."),
```

Items gain `#[serde(default)] pub fx: Vec<Fx>` (today only abilities and weapons
carry one). Validation: `Throw` only on a `Consumable`; `range ≥ 1`, `radius ≥ 1`,
`damage + stun ≥ 1`; an item's `fx` holds at most one `Throw` and at most one
blast shape (`Bloom`, `Whiteout`, `Shock`), and only on a `Throw` item.

### The `Fx` vocabulary grows

```rust
pub enum Fx {
    ... as today ...
    /// A thrown thing crossing the line two tiles a tick, a bounce, then it settles.
    Throw,
    /// A blast: white core, an amber ring to the edge, smoke that thins, a scorch.
    Bloom,
    /// A blast: the footprint inverts white, then orange, debris out, a black scorch.
    Whiteout,
    /// A blast: cyan rings of `≋` out from the centre; the net's blue.
    Shock,
    /// Where a shoved thing was: an arrow trail along the push, fading.
    Trail,
    /// A ring of `·` pulsing out from the firer to three tiles.
    Pulse { colour: (u8, u8, u8) },
}
```

The `Throw` shape takes its arc from the item (`FxInstance` carries it). The blast
shapes are **area shapes**: the engine plays them at the landing tile with the
footprint in `targets` and the radius in `radius`. `Chips`, `Tear`, `Spark` and
`Number` listed on a throw item play per victim, at their tiles, when the blast
goes off. `Fx::is_blast()` names the three; `is_shot()` grows to cover the new
shapes a strike may list (`Trail`, `Pulse`).

### Abilities

```rust
pub enum Active {
    Scan {..}, Ping {..}, Trace {..},
    /// Hits one target in sight within `range`; the roll uses `stat` and `skill`.
    Strike {
        range: i32, damage: u8, stun: u8, skill: SkillId, ap: u8,
        #[serde(default = "net")] stat: Stat,   // the roll's stat; Static keeps Net
        #[serde(default)] push: u8,              // tiles the target is thrown back on a hit
        #[serde(default)] chain: u8,             // more targets it jumps to on a hit
    },
    /// No target: your pools refill this turn; `once` a fight.
    Boost { once: bool },
    /// Marks one foe in sight within `range` for the fight: `bonus` on your chance.
    Mark { range: i32, bonus: u8, ap: u8 },
}
```

```ron
Ability(id: "static", ... as today, stat Net by the default ...),
Ability(id: "jolt", name: "Jolt", cooldown: 180,
        kind: Active(Strike(range: 3, damage: 5, stun: 1, skill: "electronics", stat: Tech, ap: 1, chain: 1)),
        fx: [Streak(colour: (0, 229, 255)), Tear, Spark, Number(tag: true)],
        desc: "An arc off the hand. It likes company: whoever stands next to them gets the rest."),
Ability(id: "shove", name: "Shove", cooldown: 60,
        kind: Active(Strike(range: 1, damage: 3, stun: 0, skill: "athletics", stat: Body, ap: 1, push: 2)),
        fx: [Chips, Trail, Number(tag: false)],
        desc: "Both hands, all of you. They end up somewhere else."),
Ability(id: "redline", name: "Redline", cooldown: 900,
        kind: Active(Boost(once: true)),
        fx: [Kick(4), Pulse(colour: (0, 229, 255)), Number(tag: true)],
        desc: "The half second you did not have. Once; the body keeps the bill."),
Ability(id: "deadeye", name: "Deadeye", cooldown: 120,
        kind: Active(Mark(range: 12, bonus: 20, ap: 1)),
        fx: [Mark(colour: (255, 0, 200), ticks: 12)],
        desc: "You read them, and after that every round knows the way."),
```

Validation: `push`, `chain` only with `range ≥ 1`; a `Boost` or `Mark` ability's
`fx` may list `Kick`, `Pulse`, `Number`, `Mark`; a `Strike`'s may add `Trail`.
The `stat` default is `Net` so the existing Static entry parses unchanged.

- **Jolt's chain**: after a hit on the target, the nearest other standing foe
  within 2 tiles of it with a clear line from it takes half the damage (rounded
  down, at least 1) and the stun, no roll; one hop per `chain`. The hop's shapes
  play from the first foe's tile to the second, started two ticks after the first
  impact.
- **Shove's push**: on a hit the foe goes `push` tiles straight away from you
  (the sign of each axis of foe − you), stopping before a tile that is not
  walkable or holds you or another standing foe; a `Trail` plays on the tiles it
  left. The log reads `shove · goon 3 · back 2`. Shove is the first melee: its
  roll uses Body and `athletics`.
- **Redline**: refused calm (`not in a fight`) and when `Engagement.boosted` is
  set (`once a fight`); costs no AP; refills `moves` and `ap` to the pools' full
  and sets the phase back to `Move` (space to stay as always); the log reads
  `redline · your turn again`; the `Number` lifts off you reading `+ turn` on
  the cyan field.
- **Deadeye**: one AP; needs a standing foe in sight under the cursor within
  `range` (`pick a target` / `out of range`); sets `Foe.marked = true` on it and
  clears it on every other foe (one mark at a time); the mark lasts until the
  engagement ends or the foe goes down. `your_chance` adds `bonus` for a marked
  foe before the clamp. The log reads `deadeye · goon marked`.
- **Static on ICE** (§3).

### Grants

- Talents (a `Grant` boon added to an existing rank, by fit, not by tier):
  Shove ← `iron_grip` rank 1 (MEAT), Redline ← `bullet_time` rank 1 (EDGE),
  Jolt ← `solder_hand` rank 1 (WRENCH), Deadeye ← `cold_read` rank 1 (STREET).
  Static stays the neural port's.
- Chrome, one each, `kind: Cyberware`: `piston_arm` (Arms, grants shove, Stat(Body,
  1), 5.0 kg, 2800), `adrenal_pump` (Torso, grants redline, 0.6 kg, 3400),
  `arc_hand` (Arms, grants jolt, 2.5 kg, 2600), `target_lens` (Eyes, grants
  deadeye, Sight(1), 0.2 kg, 2200). Only one Arms piece is worn at a time; the
  talents cover the rest.
- `Character::combat_runner` adds: frag ×3, flashbang ×2, EMP puck ×2, satchel
  ×1, and wears an `adrenal_pump` and a `target_lens` (the debug runner's maxed
  talents grant all five abilities already; the chrome is there to see the
  items). The `jolt`, `shove`, `redline`, `deadeye` abilities are learned from
  the talents; `Character::abilities` already lists grants from both sources.

### The roll

Unchanged: `hit_chance(stat, skilled, dist, range)` with the strike's `stat`. A
marked foe adds the mark's `bonus` inside `your_chance` (street) before the
clamp at 95. Blasts do not roll.

## 2. The blast engine (`combat.rs`)

```rust
/// Every tile within `radius` of `centre` that the centre can see, with its ring.
pub fn footprint(city: &City, place: Place, centre: Pos, radius: i32) -> Vec<(Pos, i32)>;
```

A tile is in the footprint when it lies within `radius` (Chebyshev), does not
block sight (`foes::blocks_sight`), and the Bresenham line from the centre to it
crosses no blocking tile (the centre's own tile always counts). Order: by ring
then by y then x.

```rust
/// A blast goes off at `centre`: everyone in the footprint is hit by ring.
pub fn detonate(st: &mut AppState, place: Place, centre: Pos, blast: Blast,
                fx: &[Fx], source: &str, at: u64, tint: Option<(u8, u8, u8)>);
```

1. Compute the footprint. Push the blast shape (the one in `fx` with
   `is_blast()`) with `origin = centre`, `radius`, `targets = footprint tiles`,
   `started = at`.
2. For each standing foe in the footprint (its ring), `Hunting` toward you if it
   was idle, damage
   `blast_damage(damage, radius, ring)` (0 when `damage == 0`; 0 for a foe
   without `chrome` when `blast.chrome`), stun `max(stunned, stun)`; an inert
   dummy takes neither but gets the shapes. Per victim push `fx`'s impact shapes
   (`Chips`, `Tear`, `Spark`) at its tile at `at + ring` (the ring's tick: the
   bloom reaches ring one a tick later) and a `Number` whose label reads `-N`,
   or `stun` when the damage is 0 and the stun is not, columns offset by the
   victim's index. `hit` is true on every victim.
3. You in the footprint take the same by your ring (`chrome` counts you when any
   cyberware is worn); stun on you sets `net.stunned_you = max(.., stun)`; your
   death goes through the existing door (`net.death = Some("your own <name>")`).
   The `Number` lifts off you too.
4. The log, one line: `frag · goon 8 · dummy 4` / `flashbang · goon stun 2` /
   `satchel · nothing in it`; `· you 4` appended in the warn colour when you were
   in it. `source` is the item's or ability's name.
5. A scorch: `Bloom` and `Whiteout` leave their stain for `SCORCH_TICKS = 3600`;
   the renderer draws it from the same instance after the blast is over (below).
   `Shock` leaves nothing.

```rust
/// You throw a consumable at a tile: its item leaves the pack, the flight plays,
/// the blast goes off when it lands or lies live for its fuse.
pub fn throw(st: &mut AppState, id: &ItemId, at: Pos) -> Result<(), String>;
```

Refusals, in order: not engaged (`not in a fight`); not your turn; the move half
(`move first · space to stay`); no AP (`no AP left`); not in the pack (`none
left`); `out of range` (beyond the throw's `range` from you); `no line` (the tile
not in your sight, `citygen::sight::sees` as the fire cursor uses); `no landing`
(the tile blocks sight or is not walkable; a tile holding a foe or you is fine).
Then: one AP through `spend_ap`; `game::apply(Action::Throw(id))` consumes one
from the pack and nothing more (`Action::Use` on a `Throw` item refuses: `throw
it from the map`, and so does the deck's `u`); push `Fx::Throw` with `origin = you`, `targets = [at]`, `started =
now`, the arc on the instance (`FxInstance.arc: Option<Arc>`); the landing tick
is `now + THROW_TICKS(dist) = ceil(dist / 2) + 2` (two for the bounce and the
settle). With `fuse == 0` call `detonate` at the landing tick (the instances are
pushed now with `started` there; the damage lands now, as a shot's does). With a
fuse, push a `Live { place, pos, blast, fx, name, turns: fuse }` on
`st.live: Vec<Live>`; the log reads `satchel · 1 turn`.

**Live blasts** count down at the start of your turn (`net::begin_your_turn`,
the spot where `spent_at` clears) and, calm, on every `BEAT_TICKS` beat: each
`Live` on your place loses a turn and at zero detonates at once (`at = now`).
Leaving the place detonates nothing: the list is cleared with the foes on
`enter_city`. When a live blast lands, every standing foe in its footprint goes
`Hunting` toward you.
A `Live` is drawn by an overlay (`render/path.rs`'s family): its footprint shaded
in the dim rings `(60, 18, 18) (44, 14, 14) (30, 10, 10)`, the glyph `o` on its
tile in `(190, 190, 140)` on even ticks and `(255, 80, 80)` on odd ones, bold,
and ` fuse N ` in cyan on the readout field two rows above it.

## 3. Abilities in the engine (`combat.rs`, `net.rs`)

- `strike` grows: the roll's stat from the variant; after a hit, `push` moves the
  foe (its `Trail` instances along the tiles left, `started` at the impact tick);
  `chain` hops as §1 says. The stun line stays `goon stunned`.
- `boost(st, id)` and `mark(st, id, foe)` beside `strike`; `game::apply(Action::
  Fire(id))` still books the cooldown for all three, and the refusals above come
  first so a refused cast books nothing (today's order in `strike`: reach, turn,
  phase, AP, then `apply`; keep it).
- **Static on ICE**: in a construct, a `Strike` under the cursor on a piece: the
  roll with the strike's stat and skill against `ATTACK`'s reach rule
  (`piece_in_reach`), damage off the piece's `hp` as the attack program's turn
  does, `stunned = max(.., stun)`; it adds the attack's heat through `noise_on`,
  wakes the piece, and a break runs the attack's break path (what `attack_ice`'s
  success calls); the shot plays with `ATTACK_TINT` and no `Flash`; the number
  as the net's. Refused on the dummy ICE as any attack is (the number still
  lifts). Jolt in the net chains between pieces by the same
  rule. Shove and Deadeye are refused in a construct (`not in here`); Redline
  works in both.
- `Foe` gains `pub chrome: bool` (the goon and the dummy `true`) and `pub marked:
  bool`. `net.stunned_you: u8`: at the start of your turn, while it is above 0,
  `ap = 0`, the log reads `you're stunned`, and it counts down one.
- `Engagement.boosted: bool`, cleared with the engagement.
- **A foe in a live footprint** (`foes::step`, before the step toward `seen`):
  when any `Live` footprint on its place holds its tile and it has a move, it
  steps to the neighbouring walkable tile not held by you or a foe that is
  outside every live footprint, nearest to `seen` on a tie; with none outside,
  to the neighbour with the greatest ring; the turn line reads `goon moves` as
  it does. It looks and fires only after it is out, or when it cannot move.
- `enemies_in_sight`, `t`, the auto end, `.`: unchanged.

## 4. The cursor, the keys and the footer (`explore.rs`, `hud.rs`, `path.rs`)

### The `ACT` list

On the act half, engaged, on foot or jacked in: the footer's fourth row reads
`ACT` and numbers, `1`..`9`: first the fight's abilities you have (every active
whose kind is `Strike`, `Boost` or `Mark`, in content order), then the
throwables in your pack (consumables with `Use::Throw`, in pack order) with
their count: ` ACT  1 static 2 jolt 3 shove 4 redline 5 deadeye 6 frag×2 7
flashbang×1 8 emp puck×1 9 satchel×1`. Keys dim `(90, 90, 90)`, names in the
text colour, throwables in `(190, 190, 140)`, a cooling ability dim with ` ◌`
after its name, an ability refused where you are (Shove in a construct) dim. The
right side keeps `AP ●● 2/2  MOVE 4/4` (the pools, AP first on the act half).
Entries that do not fit the width drop off the end with `…`. Calm, and on the
move half, the row stays what it is today (ABILITIES / PROGRAMS). In a construct
the row keeps `PROGRAMS` and the programs' keys `1`–`5`; the net-capable
abilities (Static, Jolt, Redline) follow on `6`–`8` in the `ACT` style, and no
throwable is listed there.

### Keys on the act half

- A number picks that entry: a `Strike` or `Mark` fires at the foe (or piece)
  under the cursor as today (`pick a target` without one); a `Boost` fires at
  once; a throwable **arms** the cursor (`MapScreen.armed: Option<ItemId>`): the
  readout and the footprint change (below) and space throws it at the cursor's
  tile; `esc` disarms (a second `esc` drops the cursor as today); picking a
  strike while armed disarms first.
- `space`: armed → `combat::throw` at the cursor's tile, then disarm; not armed →
  fire as today.
- The cursor is free to sit on any tile in the map while armed (it already is).

### The cursor's look

- Not on a foe: the tile's glyph replaced by `×`, black on cyan `(0, 229, 255)`,
  bold (the net's pick). On a foe: the foe inverted, as today.
- Armed: the footprint of the item's blast around the cursor's tile shades the
  background by ring: `(120, 30, 30)`, `(80, 22, 22)`, `(48, 16, 16)` (ring three
  of a satchel takes the last); the line from you to the cursor in the fire
  line's dots; the cells' glyphs kept. Your own tile in the footprint shades
  `(170, 30, 30)`.
- Armed readout, two rows above the cursor, cyan on the readout field: ` frag ·
  10 tiles · blast 2 · goon 8 · dummy 4 `: the item's name (bold), the distance,
  `blast r`, then every standing foe in the footprint with the damage it would
  take (`goon stun` when the damage is 0), nearest first; ` · you're in it `
  appended, the whole readout in red `(255, 80, 80)` on `(26, 0, 0)`, when your
  tile is in it. A refused landing reads ` frag · 14 tiles · out of range ` / `no
  line` / `no landing` in place of the blast part. The readout is clamped inside
  the map area as today's is.
- The marked foe draws with a magenta background pulsing `(140, 0, 105)` /
  `(70, 0, 52)` every 8 ticks and `⌖` in magenta, bold on the bright phase, in
  the cell above it (`render/people.rs`). The fire readout's chance on a marked
  foe is magenta bold.

### Prompts

Row 2 on the act half: `aim  hjkl target · t next · space fire · 1-9 act · . end
turn`; armed: `throw  hjkl aim · space throw · esc back`.

## 5. Drawing the shapes (`render/fx.rs`)

Ticks and colours from the mock. `T` the start, `e` the ticks since, `L` the
landing tile, `fp` the footprint with rings. Every shape clears DIM on the cells
it touches. Constants: `THROW_SPEED = 2`, `BLOOM_TICKS = 13`, `WHITEOUT_TICKS =
8`, `SHOCK_TICKS = 4`, `TRAIL_TICKS = 4`, `PULSE_TICKS = 7`, `SCORCH_TICKS = 3600`.

- **Throw** (`ceil(dist / 2) + 2` ticks): along the Bresenham line from the firer
  to `L`, the glyph at cell `1 + 2e` while in flight, the three cells behind it
  `·` fading `(120, 120, 120)` → `(50, 50, 50)`. Lob: the glyph `o` in
  `(190, 190, 140)` bold, drawn `round(2.2 · 4s(1 − s))` rows above the line
  where `s` is the fraction of the way (always up on screen, whatever the
  direction). Shadow: on the line, the glyph by the height `h = 4s(1 − s)`: `·`
  under 0.3, `o` under 0.7, else `O`, its colour `(190, 190, 140)` mixed toward
  white by `h`, bold above 0.6. Landing: tick `n` the glyph `●` `(255, 240, 180)`
  bold on `L`; `n + 1` `o` one cell past `L` along the line and a row up (the
  bounce; on `L` when that cell blocks); `n + 2` `o` on `L`. For a live blast the
  instance ends there and the overlay takes over.
- **Bloom** (13 ticks, then the scorch): e0 `✺` black on `(255, 240, 180)` at `L`;
  e1 ring 0 `✺` white on `(255, 120, 40)`, ring 1 `*` `(255, 230, 120)` on
  `(200, 60, 20)`, bold; e2 ring 0 `▒` `(255, 200, 90)` on `(120, 40, 10)`, ring
  1 `░` amber on the same, ring 2+ `*` `(255, 200, 90)` on `(90, 30, 10)` bold;
  e3 ring 0 `▓` `(90, 90, 90)` on `(50, 40, 36)`, ring 1 `▒` `(120, 120, 120)` on
  `(60, 40, 30)`, ring 2+ `·` amber; e4–12 smoke: each footprint cell whose
  `hash(x, y, e / 2) % 3 == 0` shows `░` `(e − 4) / 9` of the way from
  `(110, 110, 110)` to `(45, 45, 45)`, risen `(e − 4) / 4` rows; cells with `hash
  % 7 == 0` and `e < 8` an ember `·` `(200, 110, 20)`. From e3 the scorch under
  it all: `L` `▒` `(45, 40, 36)` on `(18, 16, 15)`, ring 1 `░` `(38, 34, 30)` on
  `(20, 17, 16)`; actors draw over it. A radius-3 blast uses "ring 2+" for its
  outer rings.
- **Whiteout** (8 ticks, then the scorch): e0 every footprint cell inverted,
  black glyphs on white, bold; e1 glyphs `(60, 0, 0)` on `(255, 160, 60)` inside
  ring 2 and `(255, 110, 50)` beyond; from e2 the scorch: ring 0 `▓`, ring 1 `▒`,
  further `░`, all `(30, 27, 25)` on `(16, 14, 13)`; e2–7 debris: in the eight
  directions a piece (`·` on the axes, `,` `'` `` ` `` on the diagonals as the
  mock) at distance `3 + (e − 2) / 2` from `L` (halved on the vertical axis of a
  diagonal), only on a tile the centre sees, colour `(e − 2) / 5` of the way from
  `(230, 230, 230)` to `(90, 90, 90)`. A flashbang (damage 0) draws no scorch.
- **Shock** (4 ticks): e0 `◉` cyan bold on `(0, 60, 75)` at `L`; e1 `○` cyan at
  `L`, ring 1 `≋` cyan bold on `(0, 40, 50)`; e2 ring 2 `≋` `(0, 180, 200)` on
  `(0, 30, 38)`, ring 1 `·` `(0, 120, 140)`; e3 ring 2 `·` `(0, 120, 140)`. Its
  victims' `Tear` and `Spark` play tinted as the net's do (cyan and the deep
  blue `(40, 60, 200)`), the `Number` tag on `(0, 90, 110)`; a stunned victim
  draws dim as a stunned foe does.
- **Trail** (4 ticks): on each tile the shoved foe left, the arrow of the push
  (`»` east, `«` west, `∨` south, `∧` north, diagonals by the greater axis,
  horizontal on a tie) in `(255, 120, 120)` for two ticks then `(180, 60, 60)`,
  the tiles nearer you starting first.
- **Pulse** (7 ticks): e0 the firer's cell black on the colour, bold; e1–2 the
  eight cells at distance 1 `·` in the colour; e2–4 distance 2 in the colour at
  two thirds; e4–6 distance 3 at a third. Redline's `Kick(4)` thins over its
  four ticks as netscan's does; its `Number` reads `+ turn` on the cyan field
  `(0, 90, 110)`.
- **Number** with a cyan field: a tag whose instance carries a tint sits on
  `(0, 90, 110)` (the net's today), fading as the red one does.
- Off-place, future and ended instances draw nothing. The scorch is the blast
  instance after its blast ticks and before `SCORCH_TICKS`.
- **The pools flash**: for `PULSE_TICKS` after a Boost the footer's right side
  draws white bold on `(0, 90, 110)` (`hud::Footer.flash_pools: bool`).

## 6. The lot and the title (`title.rs`, `app.rs`)

Unchanged but for the runner's kit (§1). The dummy on the lot and the dummy ICE
in the net take every blast and strike without harm, the numbers lifting as
today. Console: `throw <item> [n]` throws the named item `n` tiles east of you
(default 4), engaged or not, for testing the looks; `live` lists live blasts.

## Out of scope

Foes throwing grenades, blasts on ICE or a net blast program, cover for shots,
armour, a grenade's loot or shop source, the scorch outlasting the session,
sound, screen shake, abilities beyond these five, a second ability per tree.

## Tests

game: `Use::Throw` parses for the four items; `Blast` validation (on armour, zero
radius, nothing to do, two blast shapes, a blast shape on a heal); `blast_damage`
at every ring for radius 2 and 3 and the floor of 1; the new `Active` variants
parse and Static parses unchanged with `stat` defaulting to Net; the four
talents grant and the four chrome pieces grant; `combat_runner` carries the
kit. combat: `footprint` on open ground, behind the kiosk, at a wall, the ring
order; `detonate` hits every standing foe by ring, skips the inert dummy's
damage, `chrome` skips a foe without chrome, you in it (damage, stun, death), the
log lines, the instances with their starts per ring and their labels (`-8`,
`stun`); `throw` refusals in order, the AP, the pack count, the landing tick, a
fuse goes live and counts down at your turn's start and detonates; Jolt's hop
(nearest within 2, half damage, no roll, its start), Shove's push (two tiles,
stopped by a wall, by a foe, the trail), Redline (refill, phase back to Move,
once, calm refused), Deadeye (marked, one at a time, the bonus in `your_chance`,
cleared on calm and on down), Static on a piece (damage, stun, tint, the dummy
ICE). foes: a foe in a live footprint steps out, prefers out over toward, takes
the greatest ring with no way out, fires only once out. explore: the `ACT`
list's order and keys, arming, the armed readout (each shape of it), `esc`, space
throws then disarms, the cursor's `×`, the footprint shading with your tile, the
marked foe's draw. hud: the `ACT` row at 100 and 120 columns, the `…`, the
pools flash. render/fx: each new shape at its first, middle and last tick against
the mock's glyphs and colours; the scorch after the blast; the Throw's lob rows
and shadow glyphs; `until` per shape. Smoke: the COMBAT start, engage, arm the
frag, the footprint and readout, throw, the bloom, the numbers, the satchel
going live, the goon stepping out, the fuse; screenshots of the armed cursor and
of each blast mid-way.
