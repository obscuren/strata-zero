# TALENTS: the board

Approved 2026-10-31. The mockups are drawn by the generator beside this file in
`2026-10-31-talent-board-mock/` (`gen.py`, `page.py`; how to make the frame shots it
reads is in `gen.py`'s docstring). Concept B, "BOARD", is the one to build.

## Intent

The deck's TALENTS tab draws all five trees side by side, two boxes a tier, names cut to
fit. Jeffrey wants the trees to feel and look like the classic WoW talent trees: one tree
at a time, real path choices, a small picture for each talent, a way to move around and
read about the talent you are on.

## Decided with Jeffrey

- **One tree per view**, switched with `[` and `]` (the keys GEAR & INVENTORY already
  uses for its filters).
- **Concept B, the board** ("B is gorgeous!!"): the tree as a circuit board in the deck.
  Talents are chips with a 6×6-pixel icon, prerequisites are traces, rank is a column of
  LEDs beside each chip, a power rail down the left lights each tier as it opens, the
  five trees are slots down the left, and a readout on the right describes the chip under
  the cursor.
- **The content on the mock page** ("Love the extra talents"): every current talent
  kept, three new ones a tree (15 new, 60 in all), on four columns of five tiers with
  two or three lines a tree.
- **Spec, plan and build in one go.**

## Rules

Unchanged: one talent point at creation and one a level; tier `t` (from 0) opens at
`tier_gate * t` (3) points in the tree; a prerequisite must be at its last rank; the
reasons `can_learn` gives, in the same order; `Action::Learn`; the console's `talent`
and `tpts`.

Changed:

- **A tree is a grid.** `Tree.tiers: Vec<Vec<Option<TalentId>>>`: each tier is a row of
  exactly `TREE_COLS` (4) slots, `None` for an empty one; at most `TREE_TIERS` (5)
  tiers, so the board fits the deck at 100×30. A talent's column is its slot's index.
- **Prerequisites can come from the side.** `requires` names a talent in the same tree
  on a lower tier, in any column. The trace runs from the prerequisite: straight down
  when the columns match; otherwise sideways along the prerequisite's tier to the
  talent's column, then down. Every slot the trace passes through (sideways: the slots
  between the two columns on the prerequisite's tier, the talent's column included; down:
  the talent's column on every tier between the two) must be empty, and no two traces
  may pass through the same slot.
- **Validation** (as `TreeInvalid` / `TalentInvalid`): four slots a tier; one to five
  tiers; no tier empty; every talent in exactly one slot of one tree; the trace rules
  above; a talent in the last tier has one rank (the capstone rule, now per slot);
  every talent has art, 6×6 pixels, and no art names an unknown talent.
- **`Talent.icon` goes**: the picture is the icon now.
- **Sharpening reaches strikes and marks.** A `Sharpen { radius }` adds to the range of
  an `Active::Strike` and an `Active::Mark` as it already does to a scan, ping and trace
  (at least 1). `Boon::words` already says `jolt reaches 1 further`.

## The trees

Columns 0-3, tiers 1-5; `←` marks a prerequisite reached sideways, `↑` straight above,
`⇑` straight above over an empty slot. NEW talents are marked. A talent not listed under
"moved" keeps its ranks, boons, skill and description.

**MEAT** (Body, "the body is the first machine")

| tier | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| 1 | Thick skin 3 | Long haul 2 | Iron grip 2 | |
| 2 | Second lung 2 ↑ | | Haymaker 2 NEW ↑ | Wall runner 1 |
| 3 | Pain editor 2 ↑ | Bull rush 1 NEW ← Haymaker | | Cat fall 1 ↑ |
| 4 | Scar tissue 2 NEW ↑ | Juggernaut 2 ↑ Bull rush | | |
| 5 | | Meat machine 1 ↑ | | |

**EDGE** (Reflex, "the half second that decides it")

| tier | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| 1 | Quick draw 3 | Steady hand 2 | | Light step 2 NEW |
| 2 | Double tap 2 ↑ | Long shot 2 NEW ↑ | | Backstab 2 ↑ Light step |
| 3 | Hair trigger 2 ↑ | Bullet time 2 ↑ Long shot | Counter cut 2 ← Backstab | |
| 4 | Fan fire 2 NEW ↑ | | Razor wire 1 ↑ | |
| 5 | | The half second 1 ⇑ Bullet time | | |

**WRENCH** (Tech, "chrome does what you tell it")

| tier | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| 1 | Solder hand 2 | Scrap eye 2 | | Field medic 2 |
| 2 | Overclocker 2 ↑ | Jury rig 2 NEW ↑ | | Stim chemist 2 ↑ |
| 3 | Arc lash 2 NEW ↑ | Tinker 2 ↑ Jury rig | | Trauma kit 1 ↑ |
| 4 | Live wire 2 NEW ↑ | | Chrome whisperer 2 ← Trauma kit | |
| 5 | | Machine ghost 1 ⇑ Tinker | | |

**STREET** (Cool, "the city owes you")

| tier | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| 1 | Ghost 2 | | Silver tongue 2 | Cold read 2 |
| 2 | Shadow step 2 ↑ | Skeleton key 1 | Ear to the ground 2 ↑ | |
| 3 | | Smoke 1 ↑ | Fixer's favour 2 NEW ↑ | Ice cold 2 ⇑ Cold read |
| 4 | Blind spot 2 NEW ⇑ Shadow step | | | Stare down 1 NEW ↑ |
| 5 | | The name 1 ⇑ Smoke | | |

**ICE** (Net, "the deep end of the grid")

| tier | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| 1 | Ice pick 2 | Wide band 2 | | Cold boot 2 |
| 2 | Deep dive 2 ↑ | Handshake 2 ↑ | Arc welder 2 | Null route 1 ↑ |
| 3 | Vault rat 2 NEW ↑ | | Black ICE 2 ↑ | Trace eater 2 NEW ↑ |
| 4 | | Static bloom 2 NEW ⇑ Handshake | | |
| 5 | | | The deep end 1 ⇑ Black ICE | |

**Moved prerequisites:** Juggernaut now needs Bull rush (was Pain editor); Backstab needs
Light step (was none); Bullet time needs Long shot (was Hair trigger); Counter cut keeps
Backstab, from the side; Tinker needs Jury rig (was Scrap eye); Chrome whisperer keeps
Trauma kit, from the side; Machine ghost keeps Tinker. Smoke keeps Skeleton key.

**The new talents** (boons a rank, then the description):

| talent | ranks | rank 1 | rank 2 | desc |
|---|---|---|---|---|
| Haymaker (athletics) | 2 | `Sharpen(shove, cooldown -20)` | `Flag("haymaker")` | The shove that ends the conversation. |
| Bull rush | 1 | `Perk(Move(1))` | | Through, not around. |
| Scar tissue | 2 | `Perk(Armor(1))` | `Perk(Armor(1))` | Every old wound is a plate now. |
| Light step (freerun) | 2 | `Perk(Move(1))` | `Perk(Attribute(Reflex, 1))` | You are already somewhere else when they look. |
| Long shot (longarm) | 2 | `Perk(Sight(1))` | `Flag("long_shot")` | Past the second streetlight, still yours. |
| Fan fire (handguns) | 2 | `Flag("fan_fire")` | `Perk(Attribute(Reflex, 1))` | Six rounds, one breath, one direction. |
| Jury rig (scrapcraft) | 2 | `Flag("jury_rig")` | `Perk(Attribute(Tech, 1))` | Tape holds. Tape always holds. |
| Arc lash (electronics) | 2 | `Sharpen(jolt, radius 1)` | `Sharpen(jolt, cooldown -60)` | The arc goes a little further than it should. |
| Live wire | 2 | `Perk(Resist(Emp, 10))` | `Flag("live_wire")` | You have been shocked so often it tickles. |
| Fixer's favour (streetwise) | 2 | `Flag("fixers_favour")` | `Perk(Attribute(Cool, 1))` | Somebody owes somebody who owes you. |
| Blind spot (stealth) | 2 | `Flag("blind_spot")` | `Perk(Sight(1))` | Every camera has one. You live there. |
| Stare down | 1 | `Flag("stare_down")` | | They look away first. They always look away first. |
| Vault rat (hacking) | 2 | `Flag("vault_rat")` | `Perk(Attribute(Net, 1))` | Every vault has a back door if you are small enough. |
| Trace eater | 2 | `Perk(Resist(Feedback, 10))` | `Flag("trace_eater")` | The trace comes looking. You have it for lunch. |
| Static bloom (hacking) | 2 | `Sharpen(static, radius 2)` | `Sharpen(static, cooldown -60)` | One burst of noise, and every lens in the room forgets. |

A skill in brackets is the talent's flavour `skill`; my call, from the skills the game
has. The new flags are promises like the old ones and read `later: ...`.

## The icons

Every talent gets a 6×6-pixel icon, drawn three cells tall in stacked half blocks like
the skill and chrome art. They are drawn in Python
(`2026-10-31-talent-board-art/talentart.py`, one function or pixel map a talent, with
the tree's colour as the accent) and exported to `flavor/talent_art.ron` (rows of one char
a pixel, `.` empty, a colour a char), loaded beside the skill and chrome art. Never
hand-edit the `.ron`. Behind the pixels, a chip's own plate: the tree's colour at 16%
over the panel. A talent that is not *open* (its tier closed, or its prerequisite short
of its last rank) and holds no rank is drawn through `ui::art::grey`, plate included.

## The board

Drawn in the deck's body (98×22 at 100×30, 118×27 at 120×40), by offsets from the body's
top-left. The deck draws over the map as glass; the board paints its own ground, and so
do the slots and the readout, so nothing of the map shows inside.

**Tree slots**, columns 0-12. Row 0: `TREES` (dim, bold) and `[ ]` (fainter). From row
2, three rows a tree: the name in its colour (bold and white-numbered on the selected
one), its points right-aligned to column 11; under it a meter of six `▰`/`▱` (one per
two points, up to the capstone's 12) and `/12`. The selected tree's two rows sit on its
colour at 22% with `▶` in column 0. Under the five: `N to burn` in cyan, bold (the
points left).

**The board**, columns 14-64, on the tree's colour at 3.5% over the panel with a faint
`·` every fourth column on every other row (the tree's colour at 14%). Row 0: the tree's
name (its colour, bold) `· ` and its line (faint).

- **Rail**: `┃` in column 15 from the row above tier 1 to the bottom of the last tier;
  lit in the tree's colour down to the last open tier, dark (`#3a4250`) below. At each
  tier's middle row a tap `┣━` and the points it needs (`%2d`), lit when open (bold) and
  fainter when closed.
- **Chips**: tier `t`, column `c` has its icon at column `21 + 11c`, row `2 + 4t`, six
  cells by three. Pins `┤` in the cell left of it and `├` right of it, all three rows: in
  the tree's colour with a rank held, dim when open, dark otherwise. One LED a rank in
  the column after the right pins, bottom up: `●` lit (amber when maxed, else the tree's
  colour, bold), `○` unlit (green for the next rank when a point would go in now, else
  fainter).
- **Traces** under the chips, in the tree's colour once the prerequisite is maxed, dark
  otherwise: straight down `│` from the gap under the prerequisite in the icon's third
  column, ending in `▼` in the gap above the talent; from the side, on the prerequisite's
  middle row: `●` next to its pins, `─` to the talent's column, `┌` (or `┐`) and down.
- **Cursor**: `┏ ┓ ┗ ┛` in white, bold, one cell outside the pins and LEDs, in the gap
  rows above and below the chip.
- **Flash**: a rank just learned lights its chip's pins and LEDs magenta until the flash
  ends (the deck's `FLASH_TICKS`).

**Readout**, column 67 to the body's right edge, typing itself out from the tick the
cursor last moved (as the dossier does now):

```
> hair_trigger.tal            the id, the tree's colour, bold
  ○○ 0/2 · tier 3             LEDs, rank, tier (dim)
──────────────────────
HAIR TRIGGER                  white, bold
The round leaves before       the description, faint, wrapped
the thought does.

NEXT  later: hair trigger     what the next rank adds (absent when maxed)
NOW   ...                     what the ranks held add (absent at rank 0)
NEED  ✓ 6 points in EDGE      the tier's points (from tier 2), green or red
      ✓ Double tap 2/2        the prerequisite at its last rank, green or red

LINE    Quick draw            the chain: up through what it needs, down
        Double tap            through what needs it while there is one
      ▸ Hair trigger          the cursor's talent white and bold; maxed
        Fan fire              ones in the tree's colour, the rest faint
```

and on its last row `[space]` (white on the deck's bar colour) `burn a point` when a
point would go in; `at its best` (amber) when maxed; otherwise nothing, the NEED lines
say why. With no points left, the NEED lines add `✗ no points left`.

**Hints**: `←→↑↓ move · [ ] trees · space burn a point · Q/E tabs · tab close`.

## Keys and the cursor

- `←` `→` (`h` `l`): the nearest talent along the tier, skipping empty slots; wraps
  within the tier; never leaves the tree.
- `↑` `↓` (`k` `j`): the next tier up or down that has a talent, landing on the one
  nearest the current column (the left one on a tie); wraps.
- `[` `]`: the previous or next tree, wrapping; the cursor lands where it last was in that
  tree (its first talent the first time).
- `space` / `enter`: learn a rank (unchanged), flash, the deck's strip reports.
- `q` `e`, `tab`, the digits: the deck's, unchanged.

The talents tab's state moves out of `deck_rows` into `AppState.deck_talents:
TalentsState { tree, spots }` (the selected tree and a `(tier, col)` for each tree), the
way KIT and ATTRIBUTES keep theirs; it survives closing the deck and resets with a new
character.

## Elsewhere

- `debug_runner` and the dev console keep working by id.
- Tests and code that read `tiers[i][j]` read the grid (`Tree::slot`, `Tree::talents`).
- The window: every glyph used is already drawn or in the box and block ranges; the
  glyph test covers it.
- A shot test `talents_board` writes the board at 100×30 and 120×40.

## Tests

- game: the grid loads (five trees, four slots a tier, 60 talents, every one placed);
  validation refuses a three-slot tier, a sixth tier, an empty tier, a prerequisite on the
  same or a higher tier, in another tree, a trace through a chip (down and sideways), two
  traces through one slot, a capstone with two ranks, missing art, art of the wrong size,
  art for an unknown talent; the learning rules on a sideways prerequisite and across an
  empty tier; a sharpened strike's and mark's range.
- app: the cursor's moves (skips empty slots, nearest column, wraps, `[ ]` and the
  remembered spot); the board draws the tree name, the slots, the rail's numbers, chips
  and LEDs, the readout's lines, the locked reasons, `burn a point`; space learns and
  lights the LED and the flash; the state survives a reopen; the smallest screen draws
  the whole board.

## Colours

The deck's own: `DIM` (160, 170, 185), `FAINT` (120, 130, 145), `FAINTER` (92, 102, 115),
`TEXT`, `CYAN`, `MAGENTA`, `PANEL_BG`, `BAR_BG`, `RULE`. Added here: dark
(58, 66, 80) for unlit rail, traces and pins; amber (255, 200, 80) for a maxed LED and
`at its best`; green (80, 230, 120) for the next-rank LED and a met NEED; red
(255, 77, 106) for an unmet NEED; white (242, 248, 255) for the cursor and the readout's
name.

## My calls

From the builder check and the build:

- **Open is not the same as learnable.** Points are usually spent the moment they come,
  so the board at 0 points is the normal one. A chip's colour, its dim pins and its
  traces follow whether it is open (tier open, prerequisite maxed); only the green
  next-rank LED and `[space] burn a point` need a point to spend. With none, the NEED
  lines end in `✗ no points left`.
- **The icons were drawn in this build**, all 60 (the mock's twelve EDGE ones, refined,
  and 48 new), in `talentart.py`. They are a first set to react to: any one is redrawn
  there and re-exported.
- **The new talents' skills** (Haymaker athletics, Light step freerun, and so on in the
  table) are mine, from the skills the game has.
- **Sharpening reaches strikes and marks** so that Arc lash and Static bloom do what
  their words say.

## Out of scope

Respec; talents from items; anything behind the new flags; an emblem behind the board
(concept C's); mouse.
