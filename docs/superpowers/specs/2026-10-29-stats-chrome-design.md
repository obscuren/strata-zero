# STATS and chrome: the ledger

Approved 2026-10-29. The mockups Jeffrey approved are beside this file in
`2026-10-29-stats-chrome-mock/`: `ledger.html` is the screen to build (concept B, every
state, 100×30 and 120×40), `concepts.html` is the first round with the three concepts
and the full chrome catalogue. `gen.py` draws them.

## Intent

The deck's STATS tab is five rows of stat bars and a text card. Jeffrey wants it to be
the character sheet, laid out like Caves of Qud's ATTRIBUTES & POWERS screen: on the
left the numbers that make you (the stats you buy, the secondary numbers gear and chrome
move, your resistances), on the right your chrome. Chrome is real cyberpunk body
modification: jaw replacements, arms that replace your arms and take smaller mods of
their own, extra limbs off the back, eyes, spines, organs, skin. He asked for lots of it
and for creative pieces.

Chrome stops being gear. Today a cyberware item is worn in a GEAR slot and fights
armour for it (mirrorshades and optics share EYES, a jacket and the adrenal pump share
TORSO). After this, chrome is installed in body sites, shown and managed on STATS, and
the GEAR rack only holds what you wear and wield.

## Decided with Jeffrey

- **Concept B, the ledger.** Sites as a list in three groups (head, body, limbs), a
  chrome limb's sockets folded under it, and a card for the selected piece that leads
  with its drawing, the way SKILLS cards do. ("This is all GREAT and B - The Ledger.")
- **Left pane:** STATS (the five you buy), SECONDARY (only gear and chrome move them:
  HP, FOC, ARM, SGT, CAR, MOV) and RESISTANCES (KIN, THM, SHK, EMP, TOX, FBK), as
  rack-style tiles, with a readout of the selected tile under them.
- **The catalogue** in `concepts.html` (54 pieces over eleven sites and two arm
  sockets) is approved as the direction.
- **Equipment:** "We can safely remove the slots." NEURAL and ARMS leave the GEAR rack
  and cyberware leaves EYES and TORSO. Every other change to equipment is a later
  session's, which starts when this one is done.
- **No ripperdoc yet.** You install and pull chrome yourself, from this screen. A clinic
  can take that over later.
- **What ships:** pieces whose effects are numbers, or abilities the game already
  knows how to fire, come in now. The seven existing pieces move to their sites. Pieces
  that need a new system are listed below as later.
- **New numbers count.** ARM, MOV and the six resistances do something in play. EMP
  costs −2 for every piece of chrome installed.
- From this spec straight to the plan and the build, no stops.

## Goals

- STATS is the two-pane sheet in `ledger.html`, at 100×30 and up.
- Eleven body sites hold chrome: NEURAL, EYES, EARS, JAW, SPINE, BACK, ORGANS, SKIN,
  L ARM, R ARM, LEGS. A chrome arm has two sockets, HAND and FOREARM, that take smaller
  chrome.
- A LOAD budget, from base BODY and COOL, caps how much chrome you carry.
- Install from the pack onto an empty site or socket; pull out to the pack.
- ARM, MOV and the six resistances exist as numbers with sources (gear, chrome,
  talents), are shown, and act in play.
- Thirty-one chrome pieces ship, each with a 30×12 pixel drawing.

## Non-goals

- No ripperdoc, clinic, surgery cost or humanity loss.
- No shops or loot: chrome comes from kits, recipes and the debug runners, as items do
  today.
- No new systems for the later pieces (grapple, drones that fly, x-ray sight, voice
  locks, dual wielding, reaction attacks, falling, jumping, noise, skillchips).
- No rework of the GEAR rack beyond losing two slots; no new armour numbers on gear
  (the next session's).
- No leg sockets yet: the socket model is generic, but no shipped leg piece has one.

## Anti-goals

- A piece whose line promises what the game doesn't do. A shipped piece's effects are
  all real; anything else waits in "Later".
- UI or rules special-cased per piece or per site. The ledger, the card and the picker
  read sites, sockets and pieces from content; wording comes from kinds, not examples.
- A number the sheet shows that nothing uses, or a number that changes in play without
  the sheet showing where it came from.

## The screen

STATS draws in the deck's body like GEAR & INVENTORY and WORKBENCH do: its own module
with its own cursor, keys and drawing, its state kept on `AppState` across deck opens
(as `deck_kit` is). The deck's tab bar, header, Q/E, digits and `tab` are unchanged.

### Layout at 100×30

The deck's body is 98×22 here. Left pane 44 columns, a `║` rule, the right pane in the
rest; the right pane splits into the ledger (18 columns), a `│` rule and the card.

```
STATS ──────────────────────────── 2 points ║ CHROME ─────────────────── LOAD ▰▰▰▰▰▰▰▰▰▰▱▱ 20/24
┌ BOD ─┐┌ REF ─┐┌ TEC ─┐┌ COL ─┐┌ NET ─┐    ║  ── head ──       │ (drawing, 30×6 cells)
│  5   ││  7   ││  5   ││  5   ││  8   │    ║   Neural port     │
└─ +1 ─┘└─ +1 ─┘└──────┘└──────┘└─ +2 ─┘    ║   Target lens     │
                                            ║   · ears          │
SECONDARY ────────────── from gear & chrome ║   Bulldog jaw     │
┌ HP ─┐┌ FOC ┐┌ ARM ┐┌ SGT ┐┌ CAR ┐┌ MOV ┐  ║  ── body ──       │
│ 80  ││ 51  ││  3  ││ 13  ││ 28  ││  5  │  ║   Redline spine   │ PISTON ARM
└─────┘└─────┘└─ +3 ┘└─ +1 ┘└─ +4 ┘└─ +1 ┘  ║   Drone cradle    │ L ARM · load 3 · EMP −2
                                            ║   Adrenal pump    │ BODY +1 · grants Shove
RESISTANCES ─────────────────────────────── ║   Subdermal mesh  │
┌ KIN ┐┌ THM ┐┌ SHK ┐┌ EMP ┐┌ TOX ┐┌ FBK ┐  ║  ── limbs ──      │ ■ hand    Arc hand
│ 15  ││  0  ││ 10  ││ -18 ││  0  ││  0  │  ║ ▸ Piston arm      │           Jolt · SHK +10
└─────┘└─────┘└─────┘└─────┘└─────┘└─────┘  ║    ├ Arc hand     │ □ forearm empty
                                            ║    └ □ forearm    │           Monowire spool, …
─────────────────────────────────────────── ║   · r arm         │
BODY 4 · 5 with chrome        next 3 pts    ║   · legs          │ A piston in a sleeve, bolted
What the meat can take, and how much of it  ║                   │ on at the elbow. Good for
there is. Raises HP, carry and your load.   ║                   │ arguments that have gone
                                            ║                   │ physical.
feeds ◇ Athletics ◇ Freerun ◇ Second wind   ║                   │
chrome +1 Piston arm                        ║                   │
```

Hints row: `↑↓ select · space pull it out · ← stats · Q/E tabs · tab close` on chrome;
`↑↓ select · space what fits · ← stats · …` on an empty site or socket;
`↑↓ choose · space install · esc back · Q/E tabs · tab close` in the picker;
`←↑↓→ select · space buy a point · → over to the chrome · Q/E tabs · tab close` on the
left. The colours are the mock's: the deck's CYAN, LIT, MAGENTA, DIM, FAINT, TEXT,
PANEL, RULE and BAR; GOOD green `#50e678` for a bonus, AMBER `#ffc850` for what a
choice would cost, the stats in their `stat_rgb` colours.

### The left pane

Three blocks of tiles, rack style: the label set into the top border, the value in the
middle, a modifier set into the bottom border. A selected tile on the focused pane has a
CYAN border and a BAR fill; selected on the other pane, a DIM border.

- **STATS**, header tail `N points` while creation points are unspent. Five tiles 8 wide:
  the label in the stat's colour, the value the stat with everything
  (`Character::stat`), the bottom edge what gear, chrome and talents add (GOOD when
  positive, MAGENTA when negative, nothing at zero). Space on a tile buys a point under
  today's rules (`Action::BuyStat`).
- **SECONDARY**, header tail `from gear & chrome` in FAINT. Six tiles 7 wide: HP
  (`hp_max`), FOC (`focus_max`), ARM, SGT (sight radius in tiles), CAR (carry capacity,
  whole kilograms), MOV. The bottom edge is what gear, chrome and talents add; a value
  with a bonus is GOOD.
- **RESISTANCES**. Six tiles 7 wide: KIN, THM, SHK, EMP, TOX, FBK. The value is GOOD
  above zero, TEXT at zero, MAGENTA below.

Under the tiles a rule, then the **readout** of the selected tile, typed out like every
deck card:

- line 1: the full name in its colour, the value, and what it is with everything when
  that differs (`BODY 4 · 5 with chrome`); for a stat, right-aligned in AMBER, the next
  point's cost while points are unspent and the stat is under the cap;
- the blurb, wrapped, from content (stats use `stat_blurbs`; the twelve new numbers get
  blurbs beside them);
- for a stat, `feeds` and its skills (◆ owned, ◇ not), wrapped, as the card does today;
- the sources, one line each kind: `chrome +1 Piston arm`, `gear +1 Mirrorshades`,
  `talents +2`. EMP's chrome line reads `chrome -18 · nine pieces`.

What each new number's blurb says is the builder's to write in the house register; what
the numbers do is under "In play".

### The ledger

A list of rows, one per site, in three groups with a FAINT header each:
`── head ──` NEURAL, EYES, EARS, JAW; `── body ──` SPINE, BACK, ORGANS, SKIN;
`── limbs ──` L ARM, R ARM, LEGS.

- A site with chrome shows the piece's name, TEXT.
- An empty site shows `· ears` (the site's label, lower case) in FAINT.
- Under an installed piece with sockets, one row per socket: `├ ` or `└ ` in RULE, then
  the fitted piece's name in DIM, or `□ forearm` in FAINT when empty.
- The cursor row: `▸` and a BAR fill when the ledger has focus, a DIM `▸` alone when not.
  The name goes CYAN and bold.
- At 120 columns and wider the ledger is 27 columns and every site row names its site
  in FAINT, 8 wide, before what's in it (`NEURAL  Neural port`, `EARS    · meat`);
  socket rows sit under the piece's name.

The rows are built from the site list and what's installed, so a new site or socket
kind needs no new drawing code. If the rows ever outgrow the pane they scroll with the
cursor.

### The card

Beside the ledger, typed out like every card.

**On chrome** (a site's piece or a socket's): the piece's drawing, 30×12 pixels in
half-blocks (30×6 cells), at the top. Under it, a blank row, then:

- the name in caps, CYAN, bold;
- where it sits and what it costs: `L ARM · load 3 · EMP −2` (a socket piece:
  `hand · on the Piston arm · load 1 · EMP −2`);
- what it does: its perks and grants, `BODY +1 · grants Shove`, wrapped, GOOD numbers and
  CYAN ability names;
- for a limb, a blank row and each socket: `■ hand    Arc hand` with the fitted
  piece's effects on the line under it, or `□ forearm empty` with the names of the
  pieces in content that fit it under it in FAINT, cut with `…`;
- a blank row and the description, wrapped.

**On an empty site or socket:** the place's name in caps (`RIGHT ARM`, `FOREARM`), a
line under it (`meat · nothing fitted`, or `a socket on the Piston arm`), a blank row,
and a `FITS ───── from your bag` section listing every piece in the pack that fits,
name on the left and `L3` on the right. Nothing fits: `nothing in your bag fits` in
FAINT. Under the list, while the picker is open, the picked piece's effects, then
`load 20 → 23/24 · EMP −2`, a blank row and its description.

When the drawing and text are taller than the pane, blank rows give way first, then
the description is cut, as the skill card does.

### Focus and keys

Two panes. The tab opens on the left pane with the last tile it had (BODY the first
time).

Left pane: `←→` along a block's row, `↑↓` between blocks keeping the column (clamped to
the shorter row), `→` past a row's last tile crosses to the ledger, `space` on a stat
buys a point (nothing on the other tiles).

Ledger: `↑↓` walks the site and socket rows (group headers are skipped), `←` crosses back
to the tile the left pane had. `space`:

- on chrome: pulls it out to the pack. A limb comes out with what's in its sockets,
  the pieces still fitted in it, as hardware stays in a gun.
- on an empty site or socket: opens the picker if anything fits; otherwise the
  message line says nothing in the bag fits.

Picker: the FITS list takes the cursor (BAR fill on the pick), the ledger's cursor goes
DIM, LOAD shows the pick's load in AMBER on the gauge and the total turns AMBER. `↑↓`
choose, `space` installs and closes the picker, `esc` closes it. A pick that would go
over the budget shows its `L5` in MAGENTA; space on it refuses with a message.

Jacked in, chrome is locked as gear is: install and pull refuse with the gear lock's
wording, and the hints dim those keys.

### The LOAD gauge

`LOAD ` then 12 segments, `▰` filled CYAN (DIM when the ledger isn't focused), `▱`
FAINT, then `20` and `/24`. Filled segments are the load's share of the budget,
rounded up. In the picker the pick's extra segments are AMBER and the total is AMBER.

## The model (`game`)

### Sites and what fits them

```rust
/// A place on the body that holds one piece of chrome.
pub enum Site { Neural, Eyes, Ears, Jaw, Spine, Back, Organs, Skin, LeftArm, RightArm, Legs }
```

`Site::ALL` in ledger order, `label()` (`NEURAL` … `L ARM`, `R ARM`, `LEGS`), a lower
case form for the ledger, a long name for the card (`RIGHT ARM`), and its group (head,
body, limbs).

What a piece fits is a kind of site (both arms are one kind) or a kind of socket:

```rust
pub enum Fits { Neural, Eyes, Ears, Jaw, Spine, Back, Organs, Skin, Arm, Legs, Socket(SocketKind) }
```

`SocketKind` gains `Hand` and `Forearm` (and its `takes`/`fits` wording). A chrome limb
lists its sockets in `Item.sockets` as guns and decks do.

### The implant on an item

```rust
pub struct Implant {
    pub fits: Fits,
    /// What it takes of your load.
    pub load: u8,
}
```

`Item` gains `implant: Option<Implant>` (`#[serde(default)]`). Every `Cyberware` item has
one and no `slot`; no other kind has one (content validation enforces both). A piece
that fits a socket is held per copy, as hardware is, because `Fit` moves copies.

### Perks grow, and harm gets kinds

The game has no damage kinds today: a hit is a number. One enum names both the kind of
harm and the resistance to it:

```rust
pub enum Harm { Kinetic, Thermal, Shock, Emp, Toxin, Feedback }
```

with `label()` (`kinetic` …), `name()` (`KINETIC` …) and `short()` (`KIN` `THM` `SHK`
`EMP` `TOX` `FBK`).

`Perk` gains `Armor(i8)`, `Move(i8)`, `Resist(Harm, i8)` and `Throw(i8)` (tiles added to
every throw's range). Gear and talents can carry them too; this session's content only
puts them on chrome. The two exhaustive matches on `Perk` (the deck's perk lines,
content validation) learn them in their own styles: the item card's `+1 armour`, `+1
move`, `+15 kinetic`, `+3 throw`; a boon's `+1 armour`. The ledger's card has its own
short form (`BODY +1`, `ARM +1`, `KIN +15`, `sight +4`, `carry +12`, `throw +3`).

`Blast` gains `harm: Harm` (`#[serde(default)]`, default `Kinetic`): `frag` Kinetic,
`flashbang` Shock, `emp_puck` Emp, `satchel` Thermal.

### Where installed chrome lives

`Character` gains, mirroring `equipment` and `worn`:

```rust
/// The chrome installed at each site.
pub chrome: BTreeMap<Site, ItemId>,
/// The copy of each installed piece held per copy (a limb with sockets).
pub chrome_copies: BTreeMap<Site, ItemCopy>,
```

`Host` gains `Site(Site)`, so `Fit` and `Unfit` work on an installed limb's sockets
exactly as on a worn gun's: `host_item`, `host_copy`, `host_copy_mut` learn the new
variant.

Derived numbers stay computed on read:

- `perks()` chains, after equipment, every installed piece's perks in site order, then
  every piece fitted in an installed piece's sockets, then talents.
- `abilities()` adds installed chrome's grants (and fitted socket chrome's), in the same
  order, as `Source::Item`.
- `worn_mods()` also reads hardware fitted in installed chrome (none ships, but the rule
  is the same).
- `weight()` counts installed chrome. (It is in your body; it still weighs.)
- `installed()` lists every installed piece, sockets included, as `(where, ItemId)`;
  `pieces()` is its length; `load()` sums their `implant.load`.
- `load_max()` is `2 × (base BODY + base COOL) + 6` (24 for BODY 4, COOL 5). Base
  stats, so chrome that raises BODY never raises its own budget.
- `armor()`, `move_bonus()`, `throw_bonus()` and `resist(h)` sum their perks;
  `resist(Emp)` also takes 2 for every installed piece.
- `game` also gets the one rule every resistance uses, so the app never does the sum
  itself: `resisted(amount, resist)` is `amount × (100 − r) / 100` rounded to nearest,
  with `r` clamped to −100..=90; damage that was above zero stays at least 1, a stun may
  fall to 0.

### Actions

- `Install { item: ItemId, site: Site }` and `InstallCopy { copy: CopyId, site: Site }`
  take a piece from the pack and install it. Refusals, before any change: not in the
  pack; not chrome; doesn't fit the site; the site holds something (`pull the X out
  first`); over the budget (`too much chrome: load 25 of 24`).
- `Pull(Site)` puts the piece in the pack with its copy (and what's fitted in it).
- `Fit` accepts, besides hardware for its socket kind, a piece whose implant fits
  `Socket(kind)`. Fitting into an installed limb (`Host::Site`) is refused over the
  budget. `Unfit` is unchanged.
- Messages in the house style: `installed \`Grip arm\` · R ARM`, `pulled \`Piston arm\`
  · to the pack`.
- HP is clamped after a pull (as `Unequip` does) since HP max can drop.

### GEAR loses two slots

`Slot::Neural` and `Slot::Arms` go; `Slot::ALL` is seven. Cyberware items lose their
`slot`. The rack draws what `Slot::ALL` holds, so it shows seven tiles in its 3-wide
grid; anything that assumed nine is fixed to read the length. `Equip` on chrome
refuses (`chrome is installed from STATS`) and the pack's verbs offer no Equip on it.
Where an item card reads `kind · slot`, chrome reads `cyberware · arm · load 3` (the
fits kind, lower case; a socket piece `cyberware · hand socket · load 1`).
The README's "nine slots" becomes seven.

### Kits and runners

- `from_build`: kit chrome is installed at its site when the site is free and the
  budget allows (an `Arm` piece takes L ARM, then R ARM); socket chrome and anything
  that doesn't go in is packed. Vex's Nightglass optics go in at EYES.
- `combat_runner` installs the Neural port, the Adrenal pump and the Target lens (the
  optics it had go to the pack), as it wears them today.
- `debug_runner` gets nothing new: thirty pieces would put D'SHAWN at about 68 of
  28 kg and every take would refuse. The dev console's `spawn <item>` hands out any
  piece to try, and the shots install their own loadout.

### Content and validation

Chrome lives in `flavor/content.ron` with the other items. Validation: every cyberware
item has an implant and no slot; an implant's `Fits::Socket` piece is per copy; a piece
with sockets fits an `Arm` (the only limb with sockets this session); load is 1 to 5;
every chrome piece has a drawing; every `grants` names an ability.

## In play

What each number does. The code survey found no armour, resistance or damage kind
anywhere; a hit is `wound_you(st, source, damage)` and a fight's pools are the fixed
`BASE_MOVE`/`BASE_AP` (`net.rs`). Foes have no resistances and stay that way.

| Tile | What it does | Where |
|---|---|---|
| HP | max HP, as today | `hp_max` |
| FOC | max focus, as today | `focus_max` |
| ARM | each point turns 10% of the rounds that hit you, up to 60%: the round sparks off you and does nothing | `fire_at_you` (`combat.rs`), after the hit roll, its own roll |
| SGT | the field-of-view radius where you stand right now, with your bonus | the walker's radius (`update_fov`); the tile reads the same number the map uses |
| CAR | carry capacity, as today | `carry_cap` |
| MOV | tiles a turn in a fight on foot: `BASE_MOVE` plus `move_bonus()`, at least 1. In a construct your avatar isn't your meat: it moves `BASE_MOVE` | every place that gives *you* `BASE_MOVE`: `pools_for`, Redline's refill (`casts::boost`), `aim_cap`, `turn_view`, the footer's budgets, the clamps in `explore.rs` and `combat.rs`, each by where the fight is. Foes keep theirs. |
| KIN | `resisted` on every kinetic hit you take: foes' rounds, frag blasts | `fire_at_you` (after ARM), `blast.rs`'s `detonate` |
| THM | the same for thermal: satchel blasts | `detonate` |
| SHK | the same for shock: the flashbang's stun | `detonate` |
| EMP | the same for EMP: the puck's damage and stun. Still only chrome feels it (`Blast.chrome`); `you_are_chrome` becomes "anything installed" | `detonate`, `you_are_chrome` |
| TOX | the same for toxin: the choke can (new, below) | `detonate` |
| FBK | the same for feedback: black ICE's HP damage and grey ICE's focus drain, after the deck's rating and before `deep_end` halves it | `net.rs`'s `hit` |

A blast's resistance applies to its damage and its stun alike. The log keeps its
wording; a turned round reads as a miss that sparked off you (`turned` in the round
outcomes, the Spark shape at your tile).

**The choke can.** Nothing deals toxin today, and TOX must do something, so one
grenade comes in: `choke_can`, Consumable, `Throw(range: 7, arc: Lob, blast:
Blast(radius: 2, damage: 8, stun: 0, harm: Toxin))`, fx `[Throw, Haze, Number(tag:
true)]`, weight 0.4, value 200, "A soup can of something the chemical plant denies
making. Hold your breath; it won't help." `Fx::Haze` is a new blast shape (`is_blast`):
the footprint fills with sickly green `░▒` (about `(150, 220, 60)`) that thickens over a
few ticks, then thins away, leaving no scorch. The combat runner packs two.

**Throws** reach `throw_bonus()` tiles further (Launcher sleeve), wherever a throw's
range is read for you.

**New abilities**, all content using kinds the engine already fires:

| Ability | Kind | Fx | Cooldown | Line |
|---|---|---|---|---|
| `bite` Bite | `Strike(range: 1, damage: 4, stun: 0, skill: "athletics", stat: Body, ap: 1)` | `[Chips, Number(tag: false)]` | 60 | "Steel teeth on a hinge rated for car doors." |
| `spit` Spit | `Strike(range: 4, damage: 3, stun: 1, skill: "athletics", stat: Reflex, ap: 1)` | `[Streak(colour: (180, 255, 120)), Chips, Number(tag: false)]` | 300 | "A dart off the tongue. They notice the sting before the dart." |
| `whip` Whip | `Strike(range: 2, damage: 6, stun: 0, skill: "blades", stat: Reflex, ap: 1)` | `[Streak(colour: (230, 240, 255)), Chips, Number(tag: false)]` | 90 | "A wire too thin to see, on a spool in the wrist." |
| `scout` Scout | `Scan(radius: 18)` | `[Wave(colour: (120, 255, 120), ticks: 20)]` | 600 | "The drone goes up, the drone comes back, the floor is on your map." |

## The chrome that ships

Thirty-one pieces. ★ marks the seven in the game today (moved to their sites, numbers
kept, sockets added to the two arms, Arc hand to the hand socket). Lines for new pieces
are the catalogue's, trimmed of any promise this table doesn't keep.

| id | Name | Fits | Load | Does | Weight, value |
|---|---|---|---|---|---|
| `neural_port` ★ | Neural port | Neural | 1 | grants Ping, Static | as today |
| `limbic_damper` | Limbic damper | Neural | 2 | COOL +1 | 0.1, 1800 |
| `feedback_fuse` | Feedback fuse | Neural | 1 | FBK +25 | 0.1, 1200 |
| `nightglass_optics` ★ | Nightglass optics | Eyes | 2 | sight +4, grants Low-light | as today |
| `target_lens` ★ | Target lens | Eyes | 2 | sight +1, grants Deadeye | as today |
| `secondhand_eyes` | Secondhand eyes | Eyes | 1 | sight +1 | 0.2, 600 |
| `damper_cochlea` | Damper cochlea | Ears | 1 | SHK +25 | 0.1, 900 |
| `bulldog_jaw` | Bulldog jaw | Jaw | 2 | grants Bite | 0.8, 1900 |
| `rebreather` | Rebreather | Jaw | 1 | TOX +40 | 0.3, 1100 |
| `spit_dart` | Spit dart | Jaw | 2 | grants Spit | 0.2, 1600 |
| `redline_spine` | Redline spine | Spine | 3 | REFLEX +1, MOV +1 | 1.5, 5200 |
| `hardback_brace` | Hardback brace | Spine | 2 | carry +10, ARM +1 | 2.0, 2400 |
| `drone_cradle` | Drone cradle | Back | 3 | grants Scout | 1.8, 4100 |
| `adrenal_pump` ★ | Adrenal pump | Organs | 2 | grants Redline | as today |
| `second_heart` | Second heart | Organs | 3 | HP +20 | 0.5, 4800 |
| `filter_liver` | Filter liver | Organs | 1 | TOX +50 | 0.4, 1400 |
| `capacitor_ribs` | Capacitor ribs | Organs | 2 | focus +10 | 0.9, 2600 |
| `subdermal_mesh` | Subdermal mesh | Skin | 2 | ARM +1, KIN +15 | 1.0, 2900 |
| `ceramic_plating` | Ceramic plating | Skin | 4 | ARM +3, KIN +30, THM +15, MOV −1 | 4.5, 6800 |
| `grounding_weave` | Grounding weave | Skin | 1 | SHK +40 | 0.3, 1300 |
| `smart_ink` | Smart ink | Skin | 1 | COOL +1 | 0.0, 700 |
| `grip_arm` ★ | Grip arm | Arm; hand, forearm | 3 | BODY +1, carry +12 | as today |
| `piston_arm` ★ | Piston arm | Arm; hand, forearm | 3 | BODY +1, grants Shove | as today |
| `surgeon_arm` | Surgeon arm | Arm; hand, forearm | 3 | TECH +2 | 3.5, 4400 |
| `wrecker_arm` | Wrecker arm | Arm; hand, forearm | 4 | BODY +2 | 6.0, 4900 |
| `arc_hand` ★ | Arc hand | hand socket | 1 | grants Jolt, SHK +10 | as today |
| `toolfingers` | Toolfingers | hand socket | 1 | TECH +1 | 0.4, 1500 |
| `monowire_spool` | Monowire spool | forearm socket | 2 | grants Whip | 0.3, 3600 |
| `launcher_sleeve` | Launcher sleeve | forearm socket | 2 | throw +3 | 0.8, 2200 |
| `sprinter_shanks` | Sprinter shanks | Legs | 3 | MOV +1 | 5.0, 4600 |
| `dog_legs` | Dog legs | Legs | 4 | MOV +2 | 6.5, 6200 |

Descriptions:
the catalogue's line for each (in `page.py`'s `CHROME`), with the cut promises gone:
Bulldog jaw "A steel jaw on a hinge rated for car doors. Smile carefully."; Spit dart "A
dart under the tongue. Kissing is off the table."; Drone cradle "A cradle for a drone
the size of a sparrow. It comes back when it's bored."; Wrecker arm "Demolition arm,
filed down to street legal. Mostly."; Dog legs "Bent-back racing legs. Shoes are a
memory."; Damper cochlea "Clamps down on anything loud. Conversations included.";
Monowire spool "A wire too thin to see. Count your fingers after."; Launcher sleeve "A
spring tube for grenades along the forearm."; Capacitor ribs "Ribs that hold a charge.
Hugs are a liability."; Second heart "A spare heart behind the real one. Twice the
pressure, half the warranty."; the rest verbatim.

## Later

The rest of the approved catalogue, each waiting on a system:

- **NEURAL:** Chipsocket (skillchips), Pain editor (wound penalties), Ghost cache (map
  memory that outlives a floor).
- **EYES:** Flicker iris (blinding), Heatsight (seeing through walls), Zoom barrels
  (range falloff).
- **EARS:** Bonephone (hearing through walls), Tripwire ear (initiative from chrome).
- **JAW:** Voicebox (voice locks), Grinder jaw (eating junk).
- **SPINE:** Overclock bus (a first-turn AP and a focus bill).
- **BACK:** Spider rig (a second weapon, reloads), Shrike rig (reaction strikes),
  Counterweight tail (falling, climbing).
- **ORGANS:** Smuggler's gut (scanners).
- **SKIN:** Chameleon skin (stealth).
- **ARM:** Hollow arm (scanners).
- **hand:** Data spike (jacking by touch), Razor nails (bleeding).
- **forearm:** Holdout pistol (a weapon in a socket), Grapple line (pulling), Fold
  shield (standing still).
- **LEGS:** Hydraulic knees, Spring heels, Silent pads (falling, jumping, noise; they
  bring the knee and foot sockets).
- Requirements (Ceramic plating's BODY 6), Dog legs barring boots, the Second heart
  getting you back up: when chrome gets requirements and drawbacks.

## Art

Each shipped piece has a drawing, 30 pixels wide and 12 tall (30×6 cells in half
blocks), drawn side-on on the panel in the Piston arm's register (`piston_arm_art` in
`gen.py` is the approved one): gunmetal and steel greys with a highlight edge, and one
light in the piece's own colour (CYAN by default; the Target lens red). Drawings live
in a new `flavor/implant_art.ron` keyed by item id, in the skill art's `Hardware` shape
(one char per pixel, `.` empty, a colour per char), drawn by a script in
`2026-10-29-stats-chrome-art/` that exports the RON, as the skill cards' art is. The
game draws them with the skill art's pixel code. An item with no drawing (only possible
in a broken content file) draws nothing above its name.

## Docs and shots

- README: the deck paragraph's STATS description becomes the sheet and the ledger, a
  short chrome section (sites, sockets, load, EMP, install and pull, locked jacked in),
  and the rack's "nine slots" becomes seven.
- Shots at 100×30 and 120×40, by the shot harness: the ledger on a limb with a full
  loadout, the picker on an empty site, the picker on an empty socket, the left pane on
  EMP.

## Tests

- `game`: load and its budget, `load_max`, EMP's −2 per piece (sockets count), perks
  and grants from installed and socket chrome reaching `stat`, `hp_max`, `abilities` and
  the new numbers; every Install/Pull/Fit refusal changes nothing; a pulled limb keeps
  its fitted pieces; kits install chrome; content validation catches each broken rule;
  `Equip` refuses chrome.
- `strata-zero`: the tab's focus and keys (crossing panes, the picker, esc, buying a
  point), the lock while jacked in, the sheet draws inside 100×30 and 120×40 with no
  cell outside its pane, the in-play effects below.

## My calls (Jeffrey to check)

- Names in code: `Site`, `Fits`, `Implant`, `Resist`, `chrome`/`chrome_copies` on the
  character. `ui::chrome` already means the deck's look, so the new UI modules avoid the
  word (`screens/sheet.rs`, `ui/implant_art.rs`).
- `load_max = 2 × (base BODY + base COOL) + 6`.
- Installing only on an empty site; replacing means pulling first.
- The tab opens on the left pane.
- The debug runner is left as it is (the builder check found packing every piece
  would overburden it); the console's `spawn` is how a debug game gets chrome.
- MOV counts on foot only: legs don't run faster in cyberspace, and the netrunning
  specs to come are built around turns there.
- The shots' BACK piece is the Drone cradle, since the mock's Spider rig is a later
  piece.
- ARM turns rounds (10% a point, up to 60%) rather than taking a flat number off each:
  rounds do 4 to 5, so a flat cut would make ARM 3 near immunity, and a turned round is
  something you see happen.
- Resistances are percentages through one rule (`resisted`, capped at 90, damage never
  below 1), and a blast's resistance covers its stun too.
- Harm kinds: rounds and frag kinetic, satchel thermal, flashbang shock, the puck EMP,
  black and grey ICE feedback. White ICE's heat is the deck's, not yours, and stays
  outside FBK.
- The choke can and its Haze shape, so TOX does something. Without it nothing in the
  game deals toxin.
- Damper cochlea recast as SHK +25 (its catalogue effect, a shorter stun, is what SHK
  does now).
- MOV is `BASE_MOVE` plus the bonus wherever *you* get `BASE_MOVE` on foot, at least 1.
- SGT shows the radius where you stand right now, so the tile matches the map; the
  readout says it changes with place and daylight.
- Installed chrome still weighs, as worn gear does.

## Left to the builder

- Blurb wording for the twelve new numbers, and the house-style messages.
- How the sheet module splits its drawing and keys, and where the shared tile drawing
  lives (the GEAR rack's tile is the model).
- The exact pixel art for each piece, within the register above.
