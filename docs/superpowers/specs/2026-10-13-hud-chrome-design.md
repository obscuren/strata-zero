# HUD chrome design

A restyle of the play HUD shipped in `2026-10-13-play-hud-design.md`. The
data, the log, the toggles, the keys and the popups stay; the frame changes so
the HUD reads as one instrument panel rather than widgets beside a map. The
layout follows the one agreed in chat (ASTRA-style grid), the look is the
deck's chrome.

## Goal

Everything sits on a shared grid: a top band whose bars run to their column's
edge, a right column of panes divided by thin rules with a tab row above them,
a footer band with three labelled zones and the ability row. Heavy horizontal
rules and a light vertical one, as the deck's frames draw them.

## The frame

At every size from 100 by 30 up, both modes:

```
 [DEV] VEX :: LVL 2 :: ¤ 320                                 │ Night Market · Street          ☾ 22:41
 HP     38/45 ████████████████████████████████████░░░░░░░░   │ CARRY 11.6/16.0 kg ██████████████░░░░░
 FOCUS  12/30 ████████████████████░░░░░░░░░░░░░░░░░░░░░░░░   │ [F5]Inspect [F6]Legend
 XP   240/600 ████████████████████░░░░░░░░░░░░░░░░░░░░░░░░   │ [F1]Here* [F2]Quest* [F3]Map* [F4]Log*
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┳━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  map, unboxed                                               │ panes, separated by ────
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┻━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 STATUS: on foot · Street         HERE: 3 items · terminal   [space]act [enter]jack [v]iew [m]sgs [?]keys
 ABILITIES: 1 netscan ●   2 ping 12s   3 ──   4 ──   5 ──                          [tab]deck [:]console
```

Columns: the right column is `R` = 40 cells wide at every terminal width (the
main tab row needs 38 cells). One cell of light vertical rule `│` separates it from the
map, which takes the rest (`L = W - R - 1`).

Rows: four band rows, one heavy rule, the main area, one heavy rule, two
footer rows. Heavy rules are `━` with `┳` and `┻` where the vertical rule
crosses them. The vertical rule runs through the band and the main area; the
footer rows span the full width. At 100 by 30 the map keeps 22 rows.

Rules are drawn in the chrome rule colour (`chrome::RULE`). The right column
and both bands sit on the panel background (`chrome::PANEL_BG`); the map
keeps its own background.

## Top band

**Row 1, left**: ` [DEV] ` badge in amber when dev mode is on (same badge as
today), then `{HANDLE} :: LVL {level} :: ¤ {credits}` with the handle in cyan
bold, labels and `::` dim, values in the text colour. In dev mode `:: seed
{seed}` and, when the fps setting is on, `:: {fps} fps` follow, dimmed.

**Row 1, right**: the location (same rule as today: building name, else
district, else `the outskirts`, then ` · ` and the stratum; subnet owner in
the Net) left-aligned in cyan; the clock right-aligned as `{glyph} HH:MM`,
where the glyph is `☾` from 18:00 to 05:59 and `☀` otherwise. The location is
cut with `…` so one cell always separates it from the clock.

**Rows 2 to 4, left**: the three bars, one per row, in the form
`{label:<5} {cur}/{max}` right-aligned to a 7-cell number field, a space,
then the bar filling every remaining cell of the column but the last:

```
 HP     38/45 ████…░░░
 FOCUS  12/30 ████…░░░
 XP   240/600 ████…░░░
```

Filled cells are `█`, empty `░`, count `round(cells * cur / max)`, 0 when
max is 0. Colours as today: HP green above half, amber above a quarter,
pulsing magenta at or below; FOCUS cyan; XP dim amber. Labels dim, numbers
text colour.

**Row 2, right**: ` CARRY {weight:.1}/{cap:.1} kg ` then a bar filling the
rest of the column but the last cell, cyan, magenta when over capacity.

**Row 3, right**: `[F5]Inspect [F6]Legend` in dev mode, empty otherwise.

**Row 4, right**: `[F1]Here [F2]Quest [F3]Map [F4]Log`. A tab that is on
reads `Here*` in cyan bold; one that is off reads `Here` dimmed. The key
inside the brackets is cyan, the brackets dim, in both states.

## Right column panes

The active sections, in the fixed order here, quest, map, log, inspect,
legend, drawn top to bottom from the row under the band rule. Every pane
after the first starts with one row of light rule `─` across the column.
There are no pane titles.

- **HERE**: as today (feature, connectors, up to five stacks, `… and n more`,
  `nothing here` dimmed). Capped at seven rows.
- **QUEST**: three rows: `{TITLE} · {giver}` with the title upper-cased in
  cyan bold and the giver in the text colour; `» {first open objective}`
  wrapped to at most two rows; then pips, one `▰` per objective done and one
  `▱` per open one, followed by ` {done}/{total}` dimmed. With no open quest a
  single dimmed row `no active quest`.
- **MAP**: the minimap (13 rows) then one row with `{x},{y} · {stratum}`
  right-aligned and dimmed, where x, y is the focus tile.
- **LOG**: each entry starts with its clock stamp `HH:MM` dimmed (the clock at
  the entry's tick), a space, then the kind marker and text as today (`! ` in
  magenta, `▸ ` in cyan, plain in the text colour); wrapped rows hang under
  the text, indented seven cells. Whole entries from the newest, as today;
  the newest still reveals.
- **INSPECT**, **LEGEND**: as today, INSPECT capped at fourteen rows.

**Budget**: as today, with each pane's need including its separator row. LOG
takes the remainder, and its floor of three rows includes its separator. When
LOG would get fewer, MAP gives up rows first, down to seven (its separator, a
squeezed minimap and the coordinate row), and drops only when that is not
enough. A pane that would get only its separator row gets none; the first
pane has no separator and may be one row. When LOG gets no rows the newest
entry toasts on the map, as today. At 100 by 30 with the default panes the
minimap draws squeezed rather than vanishing.

## Footer band

**Row 1**, three zones on the panel background:

- `STATUS:` at the left: `on foot · {stratum}` in Walk mode, `⚡ jacked in ·
  Net` when jacked in (the bolt in cyan), `camera · {stratum}` in Explore
  mode; ` · overburdened` in magenta appended when over capacity.
- `HERE:` starting at 40% of the width: a summary of the focus tile. Parts
  joined by ` · `: `{n} item` or `{n} items` when stacks lie there,
  `terminal` when one is on the tile, each connector's kind label; `nothing`
  when there are no parts.
- The verbs, right-aligned, keys inside brackets in cyan and words dimmed.
  Walk: `[space]act [enter]jack [v]iew [m]sgs [?]keys`. Explore: `[w]alk
  [r]egen [g]configure [v]iew [m]sgs [?]keys`.

Zones are cut with `…` so they never overlap.

**Row 2**: ` ABILITIES: ` dimmed, then the five slots exactly as today
(`1 netscan ●`, `2 ping 12s`, `3 ──`), then right-aligned `[tab]deck`, with
`[:]console` in front of it in dev mode. When the dev console is open its
line replaces this row, as it replaces the action bar today.

Labels (`STATUS:`, `HERE:`, `ABILITIES:`) are dim.

## Keys

- `F1` to `F4` toggle here, quest, map, log; `F5` and `F6` toggle inspect
  and legend in dev mode only. All in both modes.
- The `v` VIEW popup stays as the fallback and gains the key in each row's
  label: `▣ here  F1`.
- Everything else is unchanged.

## Removed

The sidebar's bordered panel and handle title, the checkbox list row, the
pane title rules, the centred location in the header, the fixed twelve-cell
bars, the old action bar hints.

## Edges

- At 100 by 30: `R` is 40, the map is 59 wide, bars are 43 cells, the main
  tab row fits with one cell to spare, and the map keeps 22 rows.
- Any text that would cross a zone or column edge is cut with `…`.
- The CARRY bar with a zero capacity draws empty.
- A quest with zero objectives draws `no active quest`.
- Clock stamps use the same wrap at midnight as the header clock.

## Modules

- `ui/hud.rs`: `status_line` (row 1 left), `location_line` (row 1 right),
  `bar_row(label, cur, max, width, colour)`, `carry_row`, `tab_rows`,
  `footer_status`, `footer_abilities`, `rule_row(width, cross_at,
  heavy)`; `clock_glyph(hour)`.
- `ui/sidebar.rs`: `width(total)` (40), panes without titles, separators, quest
  pips, the map coordinate row, stamped log lines.
- `app.rs`: `clock_at(tick) -> (u8, u8)` shared by the header and the stamps.
- `screens/explore.rs`: the new layout, the F-keys, the vertical rule.
- `screens/popup.rs` or `explore.rs`: VIEW rows carry the F-key.
- `README.md`: keys and the HUD paragraph.

## Testing

- hud: each row builder at 100 and 120 columns (zones, cuts, the clock
  glyph at 17:59, 18:00, 05:59, 06:00, tab on/off text, carry over capacity,
  the bars' cell count from the width).
- sidebar: `width`, separators between panes and none before the first,
  quest pips `▰▱▱ 1/3`, the coordinate row, log stamps and hanging wraps,
  budget with separators.
- explore: the frame at 100 by 30 and 120 by 40 in both modes (rule rows
  with `┳`/`┻`, the vertical rule on every main row, no `╭` box glyphs),
  `F1` toggles HERE off and `F5` is ignored outside dev mode, STATUS reads
  `jacked in` after jacking in, the console line replaces the abilities row.
- Smoke: `drive7.py` checks `LVL 1`, `CARRY`, `[F1]Here*`, `STATUS:`,
  `ABILITIES:` and the F1 toggle.

## Out of scope

Any new data or mechanics; the toggles surviving a restart; the deck.
