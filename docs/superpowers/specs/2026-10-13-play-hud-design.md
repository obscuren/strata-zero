# Play HUD design

Roadmap step 1 of `2026-10-13-gameplay-roadmap.md`. The map screen gets a
persistent frame around the map: a header, a resource row, a sidebar of
toggleable sections and an action bar. The fading status line becomes a
message log. The dev inspector becomes one sidebar section among several.

## Goal

What the player looks at every second of play tells them, without opening
the deck: level, credits, where they are, the time, HP and the two other
bars, what lies under them, what they are meant to be doing, what just
happened, and which abilities are ready.

## The frame

The map screen (`crates/cyberpunk/src/screens/explore.rs`) draws the same
frame in both of its modes, Explore (the dev camera) and Walk, at every size
from the minimum of 100 by 30 up. Smaller terminals keep showing the
too-small notice.

Rows, top to bottom:

1. **Header**, one row.
2. **Bars**, one row.
3. **Main**, everything between: the map on the left with no border, and the
   sidebar on the right as a bordered panel exactly 30 columns wide (28
   inside). The map keeps all it has today: overlays, the ping marker, ground
   items, the help overlay, and the dev console line when the console is open.
4. **Action bar**, the last row, full width.

A wider terminal gives the map more columns, a taller one gives the map and
the log more rows. The sidebar never changes width.

## Header

Three zones on one row. Each zone is cut with `…` when it would overlap the
next.

- **Left**: in dev mode the amber ` DEV ` badge first, then `LVL {level}   ¤ {credits}`.
- **Centre**: the location of the focus tile (the player in Walk mode, the
  cursor in Explore mode). In the Net it is the owner of the subnet of the
  node under the focus tile if there is one, else the city name, followed by
  ` · Net`. Elsewhere it is the building's name when the tile is inside a
  named building, else the district's name, else `the outskirts`, followed by
  ` · ` and the stratum label. Examples: `Kowloon Noodle · Street`,
  `Night Market · Skyways`, `Helix Biotek · Net`.
- **Right**: the clock as `HH:MM`, 24-hour, zero padded. In dev mode
  `seed {seed}` and, when the fps setting is on, `{fps} fps` are drawn dimmed
  in front of it, separated by ` · `.

## Bars

One row, three bars, each `LABEL` + space + a twelve-cell bar + space +
`cur/max`, separated by four spaces:

```
 HP ████████░░░░ 38/45    FOCUS ██████░░░░░░ 12/30    XP ████░░░░░░░░ 240/600
```

- Filled cells are `█`, empty `░`; filled count is `round(12 * cur / max)`,
  with a zero maximum drawing an empty bar.
- **HP** is green above half, amber above a quarter, magenta at a quarter or
  below and pulsing with `pulse(tick, 20)`. The numbers are `hp/hp_max`.
- **FOCUS** is cyan. The numbers are `focus/focus_max`.
- **XP** is dim amber. The numbers are `xp/xp_to_next(level)`.

## Sidebar

The first row of the panel is the section list: each section's checkbox and
name, `▣` on, `▢` off, in a fixed order:

```
 ▣ here  ▣ quest  ▣ map  ▣ log
 ▢ inspect  ▢ legend              (dev mode only, second row)
```

Below the list, sections draw in that same order. Each starts with a header
line: `── NAME ───…` with the name in the accent colour and the rule in the
panel rule colour, padded to the panel width.

- **HERE** describes the focus tile. First the feature, if any: a terminal as
  `╥ {port name}` (the same name `walker::port_name` gives), a connector as
  `{glyph} {kind label}` with `>` for one that leads down and `<` for one that
  leads up, a Net node as `◆ {node kind label}`. Then each item stack as
  `{kind icon} {name}` with ` ×{qty}` appended when qty > 1, at most five
  stacks, then `… and {n} more` when there are more. A tile with nothing says
  `nothing here`, dimmed. The section takes one header row plus its lines,
  capped at eight rows in total.
- **QUEST** shows the first quest in the character's log that is not done.
  The section header is the quest title in upper case, `── THE DROP ──…`; the
  body is its first unfinished objective, word-wrapped to the panel width,
  at most three lines. With no unfinished quest the header is `── QUEST ──…`
  and the body `no active quest`, dimmed.
- **MAP** is today's minimap (`render::minimap::Minimap`) with the stratum in
  the header: `── MAP · street ──…`. It takes one header row plus the
  minimap's fixed height (`MINIMAP_H`, 13 rows).
- **LOG** is the message log, newest at the bottom, each entry wrapped to the
  panel width and styled by kind (see Message log). As many whole entries as
  fit, counted from the newest. The newest entry reveals over `REVEAL_TICKS`
  from its own tick, using `chrome::reveal`.
- **INSPECT** (dev mode) is today's `inspector_lines` for the focus tile,
  capped at fifteen rows in total.
- **LEGEND** (dev mode) is today's `legend` for the stratum.

**Budget.** Sections are laid out top to bottom. HERE, QUEST, INSPECT, LEGEND
and MAP each take the smaller of their need and what remains. LOG takes
everything left. If LOG is on and would get fewer than three rows while MAP
is on, MAP is dropped and the layout is recomputed. A section that ends with
zero rows is not drawn. Nothing ever draws outside the panel.

**Toggles.** The sidebar's six flags live in the app state as
`AppState::sidebar` so popups and settings can reach them. Defaults: HERE,
QUEST, MAP and LOG on, INSPECT and LEGEND off; they last for the session.
Turning dev mode off also turns INSPECT and LEGEND off.

**VIEW popup.** The `v` key, in both modes, pushes a `Popup::verbs("VIEW", …)`
with one row per section, letters `a` to `f` in list order, dev sections only
in dev mode. A row reads `▣ here` or `▢ here` from the current flag. Choosing
a row toggles that flag through a new `Pick::View(Section)` and pops the
popup. `esc` and `q` close it unchanged.

The `i`, `m` and `e` toggles of today are removed; `m` is reused for the
message history.

## Message log

`crates/cyberpunk/src/messages.rs` holds the data:

```rust
pub enum Kind { Info, Warn, Story }
pub struct Entry { pub text: String, pub kind: Kind, pub tick: u64 }
pub struct MessageLog { /* VecDeque<Entry>, oldest first */ }
impl MessageLog {
    pub const CAP: usize = 200;
    pub fn push(&mut self, kind: Kind, text: impl Into<String>, tick: u64);
    /// Newest entry while it is younger than RECENT_TICKS (90).
    pub fn recent(&self, tick: u64) -> Option<&Entry>;
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Entry>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

Pushing past the cap drops the oldest entry.

`AppState` gains `log: MessageLog` and three methods that push with the
current tick: `say(msg)` for `Info`, `warn(msg)` for `Warn`, `story(msg)` for
`Story`. `Status`, `set_status`, `set_warning`, `status_text`,
`status_since` and `status_is_warning` are removed; every caller writes a log
line instead, with the same text it has today. `actions::report` maps `Ok` to
`say` and `Err` to `warn`.

Styling, wherever an entry is drawn: `Info` in the text colour; `Warn` in
magenta with `! ` in front; `Story` in the accent colour with `▸ ` in front.

**Where the newest entry shows.**

- The LOG section, when on.
- The console line (`screens::console::line`) and the deck's status strip
  show `log.recent(tick)` where they showed the status before.
- When the LOG section is off and `recent` is `Some`, the map screen draws
  the entry as a toast on the bottom row of the map area, left aligned with
  one cell of padding, over the map, so a refusal is never silent.

**MESSAGES popup.** The `m` key, in both modes, pushes
`screens::messages::Messages`, a glass panel centred over the screen, 70% of
the width and height and at least 40 by 10, titled `MESSAGES` with
`{n} entries` dimmed at the right of the title row. It lists every entry,
oldest first, wrapped to the panel width, styled by kind, scrolled to the
bottom when opened. `j`, `k`, `↑`, `↓` scroll one line; `PgUp`, `PgDn` scroll
a page; `esc`, `q`, `m` close it. An empty log shows `nothing yet`, dimmed.

## Action bar

The last row, full width, dimmed hints on the right and the mode's content
on the left.

**Walk mode**: five numbered slots, two spaces apart:

```
 1 netscan ●   2 ping 12s   3 ──   4 ──   5 ──                 tab deck   ? keys
```

- Slots hold the character's active abilities (`Character::abilities`
  filtered to `Kind::Active`) in that order, by id, lower-cased ability name
  cut to ten characters with `…`.
- A ready slot shows ` ●` in the accent colour; a cooling slot shows
  ` {secs}s` dimmed, with `secs = left.div_ceil(30)` like the deck; an empty
  slot shows `──` dimmed with its number.
- Keys `1` to `5` fire the slot through `actions::perform(Fire(id))` and
  `report`, so the log lines and refusals match the abilities popup. An empty
  slot warns `nothing in slot {n}`.
- The `a` popup stays as the full list with descriptions.

**Explore mode**: `w walk   r regen   g configure` on the left.

**Right end**, both modes: `tab deck   ? keys`, with `: console` added in
front in dev mode.

## Character data

`game::Character` gains four fields, all serialised:

- `level: u8`, starter value from the content file, at least 1.
- `xp: u32`, starts at 0.
- `focus: u32` and `focus_max: u32`, both the starter's `focus`.

`game::Starter` gains `level: u8` (serde default 1) and `focus: u32` (serde
default 0). `Content::validate` rejects a starter level of 0.
`flavor/content.ron` sets `level: 1` and `focus: 30`.

```rust
impl Character {
    pub fn xp_to_next(level: u8) -> u32 { 200 * level as u32 }
    /// Adds points and levels up while the threshold is crossed. Returns the
    /// levels gained. Level 255 never gains another; the points still add.
    pub fn gain_xp(&mut self, points: u32) -> u8;
}
```

Nothing in the game grants experience or spends focus yet. The dev console
gains two commands: `xp N` calls `gain_xp(N)`, logs `+N xp` and a story line
`level {L}` for each level gained; `focus N` sets focus to `min(N, focus_max)`
and logs `focus {focus}/{focus_max}`. Both appear in `help` and Tab
completion.

## Clock

Real time, derived from the tick with no state of its own:

```rust
impl AppState {
    pub const TICKS_PER_MINUTE: u64 = 90;
    pub const START_MINUTE: u64 = 22 * 60;
    /// (hour, minute), wrapping at midnight.
    pub fn clock(&self) -> (u8, u8);
}
```

The session starts at 22:00 and one game minute passes every ninety ticks,
three seconds. Only the header reads it.

## Dev mode and the camera

Both modes share the frame. In Explore mode the header's location, HERE and
INSPECT follow the cursor. `c` (back to the camera), `:` (console), the
backtick, `w`, `r`, `g`, `[`, `]`, `s` and `d` keep today's meaning. Dev mode
off hides the DEV badge, the seed, the fps readout and the two dev sections,
and clears their flags.

## Help overlay

Both key lists drop `i m e`. Walk gains `1-5 fire an ability`, `v view
sections` and `m messages`; Explore gains `v view sections` and `m messages`.
The overlay box grows from 16 to 18 rows so the walk list fits with one row
to spare.

## Edges

- Terminals under 100 by 30 show the too-small notice; above it the frame
  always fits because the sidebar has a fixed width and every section
  degrades by dropping rows, never by drawing outside its area.
- Text longer than its slot is cut with `…`: ability names at ten
  characters, header zones at their boundaries, sidebar lines at 28 cells.
- HP at zero draws an empty magenta bar; nothing else happens.
- `gain_xp` on a level-255 character adds the points and returns 0.
- Firing an empty slot or a cooling ability logs a warning; the cooling text
  is whatever `game::apply` returns today.
- The log holds 200 entries and drops the oldest.
- A `recent` entry is one younger than 90 ticks; a log line never fades from
  the LOG section itself.

## Modules

- `crates/cyberpunk/src/messages.rs` (new): `MessageLog`, `Entry`, `Kind`.
- `crates/cyberpunk/src/ui/hud.rs` (new): `header`, `bars`, `action_bar`,
  `location`, `bar_cells`; pure functions from plain values to lines.
- `crates/cyberpunk/src/ui/sidebar.rs` (new): `Section`, `Sidebar` (the
  flags), the section builders, the budget (`fn budget(needs, height)`), and
  `draw(buf, area, …)`.
- `crates/cyberpunk/src/screens/messages.rs` (new): the MESSAGES popup.
- `crates/cyberpunk/src/app.rs`: `log`, `sidebar`, `say`/`warn`/`story`,
  `clock`; the `Status` type and its methods removed.
- `crates/cyberpunk/src/screens/explore.rs`: new layout, keys `v`, `m`, `1`
  to `5`, the toast, the old `show_*` flags removed.
- `crates/cyberpunk/src/screens/popup.rs`: `Pick::View(Section)`.
- `crates/cyberpunk/src/screens/{deck,console,help}.rs`,
  `crates/cyberpunk/src/{actions,ui/console}.rs` and the other status callers:
  log calls instead of status calls.
- `crates/game/src/{character,model,content}.rs`, `flavor/content.ron`: the
  new fields.
- `README.md`: the key section and a paragraph on the HUD.

## Testing

- **Game crate**: the starter loads `level` and `focus`; validation rejects
  level 0; `xp_to_next`; `gain_xp` gains one level, two levels in one call,
  and none at 255.
- **messages.rs**: cap at 200, `recent` while young and `None` after, the
  three kinds, `iter` order.
- **hud.rs**: header zones at 100 columns in dev and non-dev mode; `location`
  for a building tile, a district tile and a Net node; the bar cell count at
  0, half and full and with a zero maximum; HP colour at the three
  thresholds; the action bar for ready, cooling and empty slots and for
  Explore mode.
- **sidebar.rs**: the budget gives LOG the remainder, drops MAP when LOG would
  be under three rows, and never exceeds the height; HERE lists a feature and
  caps items at five; QUEST picks the first unfinished objective and says
  `no active quest` when none.
- **Screen tests** (`explore.rs`, `popup.rs`, `messages.rs`): the frame draws
  at 100 by 30 in both modes with the header, bars and action bar; `1` fires
  netscan and the LOG section shows the result; `v` then `a` turns HERE off
  and the next frame lacks it; with LOG off a refusal appears as a toast on
  the map; `m` opens MESSAGES with the entries and `esc` closes it; turning
  dev mode off clears INSPECT.
- **Smoke** (`drive5.py`): checks for `LVL 1`, the `HP` bar, `1 netscan` in
  walk mode, the VIEW popup, and the MESSAGES popup replace the old status
  bar checks.

## Out of scope

Focus mechanics, experience sources, rest and wait, saving the character and
the log, sidebar toggles surviving a restart, and anything the clock drives.
Those belong to roadmap steps 2 and 3.
