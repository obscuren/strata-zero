# Run tether: a calm run hangs off its target on the map

Date: 2026-10-31. Builds on the engagement spec (`2026-10-17-engagement-design.md`),
which put the run in the footer's third row. Mockups:
`2026-10-31-run-tether-mock/options.html` (round one A to E, round two B, C and the
blends F and G; C was chosen).

## Why

In a calm construct a program in flight shows only in the footer:
`RUN  crack · door lock  ▰▱▱ 1/3 · 40%   ▓▓▓▓░░░░  next roll`. Nothing on the map says
which node it is working on (a room can hold six small nodes beside you, all drawn
`▪`), and the eye has to leave the map to follow it. Jeffrey wants the run off the
bottom bar and tethered to the thing it runs on, with its progress in a small window.

## What Jeffrey decided

- **Calm only.** Out of a fight, the run leaves the footer. In a fight (the construct
  on turns) the footer keeps its RUN row as it is.
- **The live wire (option C).** A small label and a wire from it to the target. The
  wire is the progress: each turn that lands lights another stretch of it, from the
  label's end toward the target. Each beat a pulse rides the wire from the label to the
  target, and the roll happens when it arrives: the pulse replaces the footer's beat
  bar and `next roll`.
- **The label** is two rows on the wire's far end: the program and its odds
  (`CRACK  60%`), and under it, on the wire's row, what it runs on (`door lock`).
- **A slip** shows at the target (the existing magenta slip mark) and turns the unlit
  wire magenta while the mark shows.
- **Landing** turns the whole wire and the target green, the label reads what happened
  (`CRACKED`), then the wire reels into the label and both are gone.
- **The mockups are kept** in the repo for later inspiration, with their generator.

## Goals

- The RUN row is gone from the footer whenever the run's tether is drawn; nothing else
  in the four-row footer moves.
- A calm run always shows its tether while the player is in that construct and the
  target is on screen.
- The tether works for every program and every target a calm run can have, with words
  from the program's op and the target's kind, never from the one example.
- Both front ends show it with no window code: it is ratatui cells, box-drawing and
  block characters only.

## Non-goals

- The tether in a fight. The footer's RUN row stays there; a later change may move it.
- New feedback on a roll beyond the wire: the bolt from you to the target and the
  slip's mark stay as they are.
- Programs aimed at ICE. Pointing a program at a piece starts a fight, so a calm run
  never has one; the tether does not draw for a guard target.
- Bends in the wire. It runs straight along the target's row.

## Anti-goals

- The tether must never hide you or a piece of ICE in sight: the wire and label leave
  those cells alone.
- It must not jump sides while nothing around it changes. The label keeps one width
  through the landing, so only a piece walking into its cells or the view moving can
  move it.

## What a calm run can target

A program on or beside you (a node), or on yourself (shield and mask: the target is
your own tile, named `deck` as the footer names it today). Work runs from 1 (attack,
shield, mask) to 5 (backdoor).

## The look

All on the target's row, in the map area, over whatever the map draws there.

```
 CRACK  60%
 door lock ╺━━━━━━━━╌╌╌█╌▪
```

- **Wire:** `cells = clamp(4 × work, 8, 16)`, shortened to fit (never below one cell a
  turn). Cell `i` (0 at the label's end) is lit when `i < round(done × cells / work)`.
  Lit cells are `━` in the net's cyan; unlit cells `╌` in a faint teal. The cap at the
  label's end is `╺` (`╸` on the right side) in cyan. The wire keeps the map cell's
  background.
- **Pulse:** a near-white `█` at cell `round(phase × (cells − 1))`, where phase is how
  far the clock is from the last beat toward the next (`BEAT_TICKS`), with the cell
  behind it tinted a lighter cyan. Hidden while a hit, slip or landing shows.
- **Label:** two rows, as wide as its longer line, on the map's background colour, one
  blank cell between it and the cap. Row one: the op uppercased in cyan bold, two
  spaces, the odds (`60%`) dim. Row two (the wire's row): the target's name, dim.
  Row one goes above the wire, or below it when the wire is on the map's top row. Both
  rows are left-aligned, as in the mockup, and the label keeps a cell clear of the
  map's edge.
- **Target:** its glyph stays and goes cyan and bold. Not on yourself: your `@` is left
  alone.

### Rolls

- **Hit** (for `ROLL_TICKS`): the stretch it lit draws white, and the target draws
  dark on cyan.
- **Slip** (for `MARK_TICKS`): the unlit wire draws magenta. The existing slip mark
  sits on the target.
- **Landing** (one beat, `BEAT_TICKS`): the run is over, but the tether stays. The
  wire, cap and target draw green; the label's row one reads the op's past tense
  uppercased (`CRACKED`, `SPOOFED`, `BLINDED`, `COPIED`, `PLANTED`, `RAISED` for
  shield and mask); row two keeps the target's name. For the first half the whole wire
  shows; over the second half it shortens from the target end back to the cap. Then
  nothing.

### Where it goes

The wire leaves the target to the left or the right. For each side, count the cells
the wire, cap, gap and label would cover that lie on the construct (inside its map),
and add a large cost for any wire or label cell over you or a standing piece of ICE in
sight, or that falls outside the map area. Pick the cheaper side; on a tie, the side
with more room to the map's edge. So a tether near the construct's edge reaches out
over the empty backdrop, as in the mockup.

When neither side fits at full length, the wire shortens (down to a cell a turn) on
the side with more room. When it still does not fit, nothing is drawn.

## Behaviour

- The tether draws on the map the player stands in, while jacked into a calm
  construct, with a run whose target is a node or the deck, or during a landing's beat.
- Moving still aborts the run (unchanged): the tether goes with it. A landing's beat
  plays out even if you step away.
- Starting another run during a landing's beat shows the new run.
- A calm run with no tether keeps its RUN row in the footer: a program left on ICE
  when a fight ends (pointing one at a dummy, or `e` out of a fight), or no room on
  either side.
- The roll outcome (hit, slip, landing), when it happened and the run as it stood after
  it are kept on the net's state for the tether to read; nothing is saved.

## Where things live

- `game::Op` gains the past-tense word.
- `net.rs`: the last roll's echo (set where a run rolls), the tether's view built from
  the state (like the turn column's `turn_view`), and the footer's run readout trimmed
  to the fight (its beat bar goes).
- `render/tether.rs`: the view type, the placement and the overlay that draws it.
- `screens/explore.rs`: the overlay pushed before the shapes (so a roll's bolt and the
  slip mark draw over it), and the footer's run only in a fight.
- `screens/shots.rs`: a shot of a calm run on a node, for the window shots.

## Calls made after the builder check

- **A calm run with no tether stays in the footer.** The check found calm runs on ICE
  (a fight that ends with a program on a piece) and runs with no room to draw; rather
  than hide them, the footer keeps RUN whenever no tether is drawn. The map screen
  places the tether once a frame and tells the footer.
- **The side is worked out every frame**, not held. With the label's width fixed
  across the landing, the side changes only when a piece walks into the tether or the
  view moves, and then it should.

## Builder's calls

- Wire length per turn, colours of the faint and lit wire, and the exact tick counts
  for the flashes, within the values above.
- Tests: the footer has no RUN while calm and keeps it in a fight; the tether's
  placement (sides, shortening, keeping cells); the drawn label and wire for a run
  mid-way, a hit, a slip and a landing.
