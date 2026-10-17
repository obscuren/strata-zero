# Real time until engaged, then turns — design

Date: 2026-10-17. Status: approved in chat.

## Goal

Inside a construct nothing waits for a key: programs run on a beat with a
visible bar. The moment ICE wakes, the board goes to turns with two budgets, an
initiative order, a path you aim and watch yourself walk, and a bolt for every
roll and hit. The footer grows to four fixed rows that carry the fight. One
rulebook for the Net now and the street's combat later.

## Decisions taken with Jeffrey

- Real-time movement, turn-based engagements (Fallout, Shadowrun Returns).
- Two pools a turn: `MOVE` (tiles) and `AP` (actions). `.` ends the turn.
- Moving costs; aim a path first; movement and actions animate; nothing is
  instant.
- The street layout stays. The footer loses its hints but `[?]`, grows to
  four rows that never resize, and carries the combat readouts.
- `e` switches between calm and engaged by hand, refused while hunted.
- Parked for later: speed from talents and chrome, street combat and
  projectiles, the Net's look.

## 1. The footer (`ui/hud.rs`, `screens/explore.rs`)

Four rows under the heavy rule, always, on the street and in the Net:

```
 on foot · Night Market                                                                     [?]
(blank)
(blank)
 ABILITIES  [1] netscan ●    [2] ping 12s    [3] ──    [4] ──    [5] ──
```

```
 ⚡ Hextek Holdings · Crown Circle      TRACE ▰▰▰▱▱▱▱▱▱▱ 34 · ALERT 58 min           HUNTED    [?]
(blank)
 TURN 3  YOU › grey ICE › black ICE   RUN  crack · door lock  ▰▰▱ 2/3 · 40%   ▣ grey ICE · hunting · 2 tiles
 PROGRAMS   [1] CRACK ●    [2] COPY ●    [3] ──    [4] ──    [5] ──          MOVE ▰▰▰▱ 3/4    AP ●● 2/2
```

- Row 1: on foot, `on foot · <place>` (the camera reads `camera · <place>`,
  overburdened in magenta as before); jacked in, `⚡ <building> · <district>`
  or `⚡ <district> · grid`, then `TRACE` with the heat (the nearest gate's,
  named, on the grid), `· ALERT n min` when alert, `· SHIELD ×n`, `· MASK n`;
  `HUNTED` in magenta bold at the right while an awake piece is in sight
  memory; `[?]` at the far right, the only hint left. No `HERE`.
- Row 2: blank.
- Row 3: in calm with a run, `RUN  <op> · <target>  <bar> done/work · pct%`
  then the beat bar `▓▓▓▓░░░░  next roll`; engaged, `TURN n  <order>` first,
  the actor whose turn it is leading and `›` between, then the run readout; at
  the right the ICE in sight, nearest first, `▣ grey ICE · hunting · 2 tiles`
  (`· stunned n` kept). During an ICE turn the order reads `grey ICE moves 2`
  or `grey ICE hits`. Empty otherwise.
- Row 4: the slots as today (`ABILITIES` on foot, `PROGRAMS` jacked in, the
  flash kept); engaged, `MOVE ▰▰▰▱ 3/4   AP ●● 2/2` at the right; during an
  ICE turn the program keys draw hollow `○`.
- The dev console line replaces row 4 while open. The toast over the map for a
  hidden log stays. The help overlay lists every key that left the footer.
- `layout`: `bottom_rule` at `h-5`, `footer: [Rect; 4]` at `h-4..h-1`, the map
  and side get `h-10`. The 100×30 minimum leaves 20 map rows.

## 2. Calm: the beat (`net.rs`, `explore.rs`)

- `BEAT_TICKS = 45`. While the player stands in a construct and no engagement
  is on, `MapScreen::update` fires a beat every `BEAT_TICKS` of the world
  clock: `step_run` (one roll, with its bolt) then `ice_turn` (asleep pieces
  patrol a tile; sight is checked). The beat phase lives on `NetState.beat`
  (the tick of the last beat); popups freeze it like everything else.
- `.` in calm hurries the next beat: it fires at once. Nothing else waits.
- A move is instant as now; a move out of the target's reach aborts the run.
- If `ice_turn` leaves any piece awake, or the alarm trips, or the player
  attacks a piece, the construct **engages**.

## 3. Engaged: turns (`net.rs`)

```rust
pub enum Actor { You, Ice(usize) }
pub struct Engagement {
    pub round: u32,
    pub order: Vec<Actor>,          // rolled once at the start
    pub turn: usize,                // index into order
    pub moves: u8, pub ap: u8,      // the current actor's pools left
    pub manual: bool,               // switched on by `e`, not by a wake
    pub acting_since: u64,          // tick the current ICE action started, for the pace
}
pub const BASE_MOVE: u8 = 4; pub const BASE_AP: u8 = 2; pub const PACE_TICKS: u64 = 8;
```

- Initiative: you by your Net stat, each standing piece by grade (black 3,
  grey 2, white 1); higher first, ties to you, then by index. Broken or lifted
  pieces are skipped when their turn comes; a piece that stands again joins at
  the end of the order.
- Budgets: you `BASE_MOVE` tiles and `BASE_AP`; ICE by grade: white 0/1, grey
  2/1, black 3/2. A tile costs one move; a program turn, an attack turn, a
  shield, a mask, a pulse cost one AP; jacking out ends the engagement; `.`
  ends your turn. A run keeps its progress and odds across calm and turns and
  rolls per AP while engaged.
- Your turn: keys act as in calm but spend the pools; a key with nothing left
  says `no AP left` / `no moves left`. Programs on the number keys, space, the
  `a` menu all go through one `spend_ap` door.
- An ICE turn plays out on `PACE_TICKS`: at the start the piece looks (sight,
  then its mode); while it has moves and is awake and not beside you it steps
  one tile toward where it saw you; with moves and asleep it patrols one tile;
  then each AP beside you is a hit (through `hit`, with its bolt); the stun
  counts down one per turn and a stunned piece's turn is skipped. Between each
  step the engine waits `PACE_TICKS`, so you watch it come.
- `ice_turn` is refactored into `ice_look(st, b, i)` (sight and mode) and
  `ice_step(st, b, i) -> Step` (`Moved(to)`, `Hit`, `Nothing`), used by both
  the beat (one look, one step, one hit at most, as today) and the turn engine.
- End of a round: if no piece is awake and the engagement is not manual, it
  ends and calm resumes with the beat phase reset; if it is manual it goes on.
  Jacking out, a dump and FLATLINED end it. Leaving the construct ends it.
- `e`: in calm, engages by hand (`manual: true`); engaged, returns to calm only
  when no piece is awake, else `still hunted`. On the street `e` says
  `nothing to engage`. The console gets `engage` / `calm` aliases.

## 4. Aiming and moving while engaged (`explore.rs`, `render/path.rs` new)

- `hjkl` and the arrows move an aim cursor from your tile over walkable tiles;
  the shortest walkable path (A*, pieces and the player's own tile excluded)
  draws from you to it: the first `moves` tiles lit (`·` cyan bold on the
  tile, `×` on the end), the rest dim; a tile with no path shows the cursor
  red. `esc` drops the aim.
- `enter` on a lit tile executes: `MapScreen.walking: Option<Walk { path,
  next_at }>` steps you one tile per `WALK_TICKS = 4`, spending a move each,
  with input locked until it ends (`esc` still pauses). Each step refreshes
  sight and may abort a run out of reach, as a normal step does. `enter` on
  your own tile jacks out, as today.
- In calm the same aim works and the walk costs nothing; a direct `hjkl` step
  without aiming stays as it is in calm.

## 5. Bolts (`actions.rs`, `net.rs`, `render/fx.rs`)

- A program turn fires `Fx::Bolt` from you to the target tile, `ROLL_TICKS =
  12`; the roll itself resolves at once (the outcome is known), the log line
  and the kick follow the bolt's landing by scheduling the line's tick? No:
  the line is written at once; the bolt is the picture of it. A slip adds a
  `Fx::Mark` on the target for 20 ticks in magenta.
- An attack turn is the same bolt in white `(240, 240, 240)`; an ICE hit is a
  bolt from the piece to you, magenta for grey and white, `(255, 40, 40)` for
  black; a shield absorbing adds a cyan mark on you.
- Pulses keep their effects; in calm the beat's roll fires the same bolt.

## Out of scope

Street combat, guns and projectiles, speed from talents or chrome, the Net's
palette and frame, saving mid-engagement (a save ends it), the beat's length
in settings (console only).

## Tests

hud: the four rows on foot, in calm, engaged (your turn, an ICE turn), the
only hint `[?]`, no `HERE`, no resize across states, the console line in row
4; the layout's rows and the 20-row map at the minimum. net: the beat fires
every `BEAT_TICKS` and not in a popup; `.` hurries; a wake engages and the
order is by Net and grade with ties to you; budgets by grade; `spend_ap` and
the refusals; ICE turns step on the pace and hit per AP; the stun skips a
turn; a round with nothing awake ends the engagement unless manual; `e` both
ways and `still hunted`; jack out ends it; the run keeps progress across the
switch. explore: aiming draws the path with the lit prefix, `enter` walks on
`WALK_TICKS` with input locked and moves spent, a walk out of reach aborts
the run, `esc` drops the aim. fx: a bolt per roll, per hit, the slip mark.
Smoke: calm beat landing a crack, a wake engaging, a turn with a path walk, a
hit, `.` ending the turn, `e` refused while hunted and allowed after.
