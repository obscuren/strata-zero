# Look with a cursor: riff notes

Date: 2026-10-17. Status: **ideas, not a spec.** A todo from Jeffrey: the
look action needs a target the player can move freely across the screen,
identifying whatever is under the cursor. Nothing below is decided unless it
says so.

## The idea

Today "look" only describes the tile the player stands on. It should instead
put a cursor on the map that the player moves with the walk keys, and
describe whatever sits under it: a person, a lamp, a door, a terminal, a lit
window across the plaza. Esc or the look key again puts the cursor away.

## What already exists

Checked while riffing, so the next session doesn't have to.

- **The look verb** lives in the Space popup
  (`MapScreen::here_choices` region, `crates/strata-zero/src/screens/explore.rs`
  around line 860). It pushes `Pick::Card(inspector_lines(...))` for the
  player's own tile: the tile, the node under it, item stacks, and the person
  there if any.
- **There is already a free cursor**, but it is dev-only. `Mode::Explore` is
  entered with `c` in dev mode, `MapScreen::cursor` is the tile under
  inspection, the camera follows it, and `handle_explore` moves it with the
  direction keys, eight tiles at a time with Shift. Leaving dev mode forces
  the screen back to `Mode::Walk` ("without dev mode there is no free camera").
  `focus_pos` already picks the cursor over the player when not walking.
- **The cursor is drawn** by `MapView` when `cursor: Some(pos)` is passed
  (`render/viewport.rs`), so the glyph and highlight exist.
- **The sidebar's Inspect section** (`ui/sidebar.rs`, `Section::Inspect`) is
  dev-only and shows `inspector_lines` for the focus tile.
- **`inspector_lines`** (`crates/strata-zero/src/ui/panel.rs`) is a dev dump:
  position, terrain, feature, room, station, building kind and footprint size,
  district wealth and decay percentages, connectors, net node and subnet owner,
  and the person card (name, profession, doing, next, lives, works, car).
- **Targeting** (`Targeting` struct, `handle_targeting`) is the other "pick a
  tile" flow: a program key lists candidates nearest first and Tab cycles
  them. It is the closest thing to what look needs, but it is tied to a
  program and a list, not a free cursor.
- **Visibility** is a per-place `Grid<Visibility>` on the player
  (`Player::vis`), with `Visible`, `Remembered` and `Unseen`.

## 1. Look mode

- A player-facing mode next to Walk, entered with `l` straight from walking
  (the Space popup keeps its `l look` row as a second way in, or drops it).
  Either a third `Mode::Look`, or a `looking: Option<Pos>` field in the style
  of `targeting`, so Walk stays the mode and look is a layer on it. The
  second is less churn: the camera, the HUD and the walker keep behaving as
  in Walk.
- **The cursor starts on the player** and moves with the same keys as
  walking, including diagonals, with Shift for eight tiles. It clamps to the
  map. The camera follows softly as in Explore, or not at all and the cursor
  is clamped to the viewport instead; the second keeps the player's bearings.
- **Esc, `l`, or Space** leaves look mode. Enter on something with verbs
  (a person, a terminal, a door, an item stack) could open the verb popup for
  that target later; today the popup is only for the player's own tile.
- **Tab cycles points of interest** in view, nearest first: people,
  terminals, doors, connectors, item stacks, lamps and signs. This reuses the
  candidate idea from `Targeting` and spares the player walking the cursor
  across the whole screen. Shift+Tab goes back.
- While looking, the world's clock should keep running (it runs in Walk) but
  no key moves the player.

## 2. What the cursor says

`inspector_lines` is the dev inspector and should stay that. Look wants a
short, in-fiction description.

- **A one-liner in the HUD** as the cursor moves: "a street lamp", "a locked
  door · Flat 12", "a sidewalk under the Night Market", "Mara Chen, door
  muscle, watching the street". The person's doing-line from the sim card
  already reads like this.
- **A longer card on Enter**: the same facts in prose, plus what the player
  knows about it (later: journal entries, codes, who lives there).
- A `look_line(map, pos, person, items) -> String` next to `inspector_lines`
  in `ui/panel.rs`, built from the same inputs, is the natural split. The dev
  inspector keeps wealth, decay, footprint and subnet; the look line never
  shows numbers.
- In dev mode the Inspect sidebar section can follow the look cursor too,
  which `focus_pos` already arranges once the cursor is the focus.

## 3. What the cursor may see

The cursor must not tell the player more than their eyes do.

- **`Visible`**: everything, including people and items.
- **`Remembered`**: terrain, features and buildings as last seen; no people,
  no items ("you remember a sidewalk here").
- **`Unseen`**: nothing ("you can't see that far"). The cursor can still move
  there, so the player can aim at darkness, but it says nothing.
- This is where the lit-sight idea (`2026-10-17-lit-sight.md`) pays off: a
  lit window across the dark plaza is `Visible`, so the cursor can name it,
  and the dark ground between it and the player says nothing.
- In a construct, pieces and nodes follow the same rule through the construct
  visibility grid.

## 4. Mouse, later

crossterm reports mouse movement, so hovering could drive the same cursor
and a click could open the card. It is an addition on top of the keyboard
flow, not a replacement, since the whole game is keyboard-first. Not for the
first pass.

## Order of work

1. Look mode as a layer on Walk with a free cursor, Esc to leave, and the
   HUD one-liner from a new `look_line`. Visibility gating from the start.
2. Tab to cycle points of interest.
3. Enter on a target to open its verbs; the longer card.
4. Mouse hover, if ever.

## Open questions

- Does the look key also exist inside the Net? Nodes and pieces have labels,
  so probably yes, with the same gating.
- Should the camera follow the cursor past the viewport edge, or is the
  viewport the limit of what you can look at?
- Does the one-liner replace the location line in the HUD while looking, or
  sit under it?
