# Skill cards: art, requirements and the card's layout

Approved 2026-10-23. The drawings Jeffrey approved are the code in
`2026-10-23-skill-cards-art/` beside this file.

## Intent

The deck's SKILLS tab shows a plain text card for the selected skill. Professions get a
small emblem when you choose one at creation. Skills deserve better: each gets a drawing as
good as the Ratchet's and the deck's, and the card is laid out around it. At the same time
the card shows what a skill asks for, which the game does not have yet. Each skill gets
example stat requirements to tune later, plus a list of skills it needs first (Hacking
I/II/III one day), empty for now.

## Goals

- Every one of the 24 skills has a drawing: the same size for all, half-block pixels,
  colour allowed.
- The drawings mix two kinds. Physical skills are **hardware**, drawn in the Ratchet's
  materials (gunmetal, polymer, red tape, gold, brass, steel). Abstract skills are **neon**
  signs on a screwed plate, lit in the skill's stat colour with one accent tube.
- A skill you have not learned looks it: hardware goes grey, neon goes cold.
- The card, top to bottom: the drawing centred, `[learned]` or `[unlearned]`, the SP cost,
  one line per stat requirement, `[REQUIRED SKILLS]` and the list, then the description
  left-aligned.
- Requirements live in content as data, shown on the card and not enforced.

## Non-goals

- No enforcement of stat requirements or required skills. Buying works as it does now.
- The character creation wizard's SKILLS step keeps its card as it is.
- Category rows on the board keep today's category card.
- No new skills. Skills that need others (I/II/III) come later.

## Anti-goals

- A drawing that only works for one skill, or a card special-cased per skill. The card
  reads whatever art and requirements the content gives it.
- Art that drifts from what was approved. The drawings in the game are the same pixels the
  browser showed.

## The card

The deck's detail pane, for a row of `Detail::Skill`:

```
                 ┌──────── 34×10 cells ────────┐
                 │          the drawing         │
                 └──────────────────────────────┘
                            [learned]
                          :: 100 SP ::
                          :: NET 5 ::

                        [REQUIRED SKILLS]
                             [none]

You find the seam in the ICE before it finds you.
```

- The drawing is 34 cells wide and 10 tall (34×20 pixels, two to a cell), centred in the
  pane's text area. At 100×30 the pane is 56 by 21, and the whole card takes 20 rows.
  At 120×40 there are rows to spare.
- `[learned]` is cyan and bold; `[unlearned]` is dim. Learned means the character has the
  skill.
- `:: 100 SP ::` is the skill's cost in its category: the `::` faint, the value in text
  colour. A first skill reads `:: free ::`.
- One `:: NET 5 ::` line per requirement, the stat's name upper case, bold, in the stat's
  colour (the wizard's `stat_rgb`). A skill with no requirements has no such line.
- `[REQUIRED SKILLS]` is dim and bold; under it `[none]` faint, or the names joined with
  ` · ` in text colour.
- The description wraps to the pane, left-aligned.
- The skill's name is not on the card: the list row is lit. (Our call; easy to add.)
- Every line but the description is centred.

The text types itself out as every card does. The drawing scans in from the top during the
first half of that reveal, a row at a time. Buying the selected skill through its popup
replays the card, so the drawing scans in again, now lit. (Our call.)

The deck's pane is glass over the washed map, and only cells with text get a backing. The
drawing's 34×10 block is cleared to the panel colour (`PANEL_BG`, `#04060a`) before it draws,
so holes and half-empty cells show the panel, exactly as the approved mockups did.

## Requirements in content

Two fields on `Skill`, both defaulting to empty:

- `needs: [(Stat, u8)]`: stat requirements, shown on the card, not enforced. Each value is
  1 to 10.
- `after: [SkillId]`: skills wanted first, shown on the card, not enforced. Each is a known
  skill.

The example values use the skill's own stat and its place in its category: 4 for the
first skill, 5 for the second, 7 for the third. `after` is empty everywhere.

| category | first | second | third |
|---|---|---|---|
| NETRUNNER | Hacking NET 4 | ICE breaker NET 5 | Vault run NET 7 |
| WRENCH | Electronics TECH 4 | Scrapcraft TECH 5 | Overclock TECH 7 |
| IRON | Handguns REFLEX 4 | Longarm REFLEX 5 | Snapshot REFLEX 7 |
| EDGE | Blades REFLEX 4 | Close work REFLEX 5 | Riposte REFLEX 7 |
| GHOST | Stealth COOL 4 | Lockpick COOL 5 | Vanish COOL 7 |
| STREET | Streetwise COOL 4 | Haggle COOL 5 | Fixer's ear COOL 7 |
| RIPPER | Medtech TECH 4 | Field patch TECH 5 | Stim cocktail TECH 7 |
| MEAT | Athletics BODY 4 | Freerun BODY 5 | Second wind BODY 7 |

## The art in content

A new file, `flavor/skill_art.ron`, maps each skill id to its art. It is generated by the
art tool (below) and is not hand-edited. `Content` loads it beside `content.ron` and holds
it as `skill_art: BTreeMap<SkillId, SkillArt>`. Every content parse, including the tests'
edited copies of `content.ron`, takes its art from this file.

```rust
pub enum SkillArt {
    /// Hardware in its own colours: one char per pixel, `.` empty, as a sticker picture.
    Hardware {
        rows: Vec<String>,
        colours: BTreeMap<char, (u8, u8, u8)>,
        /// Text over the folded cells: cell x, cell y, text, ink, ground.
        #[serde(default)]
        labels: Vec<(u16, u16, String, (u8, u8, u8), (u8, u8, u8))>,
    },
    /// A neon sign: `#` a tube in the skill's stat colour, `+` a tube in the accent,
    /// `.` the plate.
    Neon { rows: Vec<String>, accent: (u8, u8, u8) },
}
```

Validation: every skill has art and every art names a skill; rows are exactly 20, each 34
chars; a hardware pixel's char has a colour; a neon row holds only `.`, `#` and `+`, with at
least one tube; a label stays inside the 34×10 cells. A missing drawing is a content error,
so the card never draws without one.

## How it draws

`ui/skill_art.rs` in `strata-zero` draws a skill's art at a cell position, learned or not.
It folds pixels into cells exactly as the `Pixels` canvas does.

- **Hardware, learned:** the colours as given; labels over the folded cells.
- **Hardware, unlearned:** every colour, ink and ground included, through `grey`. With
  luma `l = 0.3r + 0.59g + 0.11b`, each channel becomes `(0.25v + 0.75l) × 0.6`.
- **Neon:** the plate is `#101217` over all 34×20, its top row `#1b1e25`, a screw
  `#4a4f58` in each corner. Lit: every non-tube pixel next to a tube (eight ways) takes
  that tube's halo, `mix(plate, tube colour, 0.28)`. Halos go in two passes, the main tubes'
  first and the accent's after, so the accent's halo wins where they meet. Then the tubes:
  `#` in the skill's stat colour, `+` in the accent. Cold: no halo, every tube `#363b44`.

The deck draws the art itself at the top of the pane, then puts the text lines under it
through the usual reveal. `put_paragraph` backs only text and paints every cell's
background, so the art cannot go through it. `reveal` keeps a line's alignment on the line
it is typing, so centred lines type in centred.

## The art tool

`docs/superpowers/specs/2026-10-23-skill-cards-art/` holds the Python that drew the approved
art. It is the way to draw a new skill:

- `skillart.py`: the pixel canvas, the palette, the 13 hardware drawings, the 11 neon signs,
  and the learned and unlearned renders, each matching the game's rules above.
- `export.py`: writes `flavor/skill_art.ron`. Hardware colours get one char each. The
  panel colour `#04060a`, used as a hole, becomes `.`.
- `board.py`: an HTML board of every drawing, learned and unlearned, to check by eye.

The split, with what each drawing is:

| | hardware | neon |
|---|---|---|
| NETRUNNER | Hacking (jack plug in a port), ICE breaker (ice axe in ICE), Vault run (vault door, gold bar) | |
| WRENCH | Electronics (iron on a board), Scrapcraft (taped pipe, wrench) | Overclock (dial pinned in the red) |
| IRON | Handguns (pistol), Longarm (scoped rifle) | Snapshot (crosshair, speed lines) |
| EDGE | Blades (knife), Close work (brass knuckles) | Riposte (crossed blades, a spark) |
| GHOST | Lockpick (picked padlock) | Stealth (closed eye), Vanish (a figure fading) |
| STREET | Fixer's ear (burner phone, `JOB?`) | Streetwise (arrow sign), Haggle (price tag going down) |
| RIPPER | Medtech (medkit), Field patch (tape roll, gel patch) | Stim cocktail (glass, pill on the pick) |
| MEAT | | Athletics (runner), Freerun (vaulting a wall), Second wind (heart, a pulse) |

## Left to the builder

- Module and function names, how the deck passes the pane rect to the art.
- The scan-in's exact timing, within "the first half of the reveal".
- Tests: content validation for each rule, the renders (a learned colour, a grey, a lit
  tube and its halo, a cold tube, a label), the card's lines for a learned and an
  unlearned skill, and the card fitting the 100×30 minimum with its description showing.
- A screenshot test for a few cards, beside the deck's others.
- The README's deck paragraph mentions the card.
