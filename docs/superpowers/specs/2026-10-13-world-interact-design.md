# World interact: ground items, Space, abilities and the dev console — design

**Status:** approved in chat on 2026-10-13 (sub-project B of two). Builds on
`2026-10-13-items-design.md` (A): `Action`, `apply`, `perform`, `Ground`,
`AppState::ping` and the popup screen exist before this starts.

## Goal

Items lie in the city and can be picked up. Space in walk mode is the one
interact key: it opens a popup of what you can do here (take, jack in or out,
take the stairs, look). `a` opens your active abilities and fires one. Dev mode
gets a console to spawn items and drop them on the ground, so everything above
can be exercised before loot exists.

## Scope

In: the ground overlay on the map; the walk-mode interact popup; the abilities
popup and the two active effects (scan, ping) visibly working; the map following
the player's stratum after a popup moves them; the inspector listing items on
the focus tile; the dev console; hint rows, help and README.

Out: loot generation, saving the ground, containers, vendors, any console
command beyond the six below.

## Ground overlay (`crates/cyberpunk/src/render/items.rs`)

`GroundOverlay<'a> { ground: &'a Ground, content: &'a Content, vis: Option<&'a Grid<Visibility>> }`
implements `Overlay`. For each pile on the drawn stratum inside the viewport it
draws one glyph: the kind icon when every stack shares a kind, `≡` for a mixed
pile. `kind_icon` moves from the deck to `render::glyphs` so both use it. The
colour is amber `Rgb(255, 200, 80)` on a `Visible` tile, scaled to 0.45 on a
`Remembered` one, and nothing on an `Unseen` tile; without a visibility grid
(free camera) every pile shows. The player's `@` is drawn after overlays, so it
sits on top of the pile it stands on.

## Interact (`Space` in walk mode)

`MapScreen` builds the popup from the player's surroundings, in this order:

1. one row per stack in the pile under the player: `take 3× Copper coil`,
   letters from `a`; when the pile has more than one stack, `t take all`;
2. `j jack in` when a Jack connector is within reach (the same test as
   `Player::jack_in`), or `j jack out` when jacked in;
3. `> go up to Skyways` and `< go down to Undercity` for connectors under the
   player that serve that way;
4. `l look`: a card of the inspector lines for the tile plus its items.

With nothing but `look` on offer the popup still opens. The popup's `Pick`
gains `World(WorldAct)` with `WorldAct::{JackIn, JackOut, Up, Down}`; the
popup applies these to `st.player` itself (`jack_in`, `jack_out`,
`use_connector` + `update_fov`), sets the status, and pops with
`TransitionKind::Glitch` for jacks and `None` for stairs. Take rows are
`Act(Take { .. })` and `Act(TakeAll)` through `perform`.

`MapScreen::update` gains one rule for walk mode: when `player.stratum !=
self.stratum`, adopt it and snap the camera to the player. That is how the map
follows a jack or a stair taken from the popup. `Enter` and `<` `>` keep
working as shortcuts.

## Abilities (`a` in walk mode)

A popup titled ABILITIES with one row per active ability from
`character.abilities()`: `Netscan · ready` (enabled, `Act(Fire(id))`) or
`Ping · cooling 7s` (disabled). No actives: status `no active abilities`, no
popup. Effects, run by `perform` (spec A): Scan reveals within the radius;
Ping stores `AppState::ping`. The `PingOverlay` (in `render/items.rs`) draws,
while `tick < until` and on the ping's stratum, a pulsing cyan highlight on
every Jack and physical connector within the radius: the glyph keeps its
character and takes the SIGNAL colour at `0.5 + 0.5 × pulse(tick, 20)`.

## Inspector

`inspector_lines` gains an `items: &[Stack]` argument and appends `items ·
3× Copper coil, Dead phone` when the pile is not empty.

## Dev console

Opened with `:` in dev mode, in either map mode. It is state on `MapScreen`
(`console: Option<Console>`), not a pushed screen, so the map stays unwashed
and keeps animating. While open every key goes to the console.

`ui/console.rs` holds the pure part:

```rust
pub struct Console { pub input: String, history: Vec<String>, cursor: Option<usize>, pub hint: Option<String> }
pub enum ConsoleEvent { None, Run(String), Close }
impl Console { pub fn key(&mut self, k: KeyEvent, complete: &dyn Fn(&str) -> Vec<String>) -> ConsoleEvent }
```

Keys: printable characters insert, `Backspace` deletes, `↑`/`↓` walk the
history, `Tab` completes the current word from `complete(word)` (unique match
fills it in; several set `hint` to the candidates joined by spaces, up to
eight), `Enter` runs a non-empty line and clears it, `Esc` closes.

`screens/console.rs` runs a line against the app: `run(st: &mut AppState,
focus: At, line: &str) -> Result<String, String>`, where `focus` is the map's
focus tile (the player when walking, the cursor under the camera) on the map's
stratum.

| Command | Does |
|---|---|
| `spawn <item> [qty]` | adds to the inventory, no capacity check: `spawned 3× Copper coil` |
| `drop <item> [qty]` | adds to the ground at the focus tile; refused when the tile is not walkable |
| `give <credits>` | adds credits |
| `heal` | hp to max |
| `clear` | empties the pile at the focus tile |
| `help` | lists the commands |

Unknown commands and ids are refusals (`unknown item cop`). Completion offers
command names for the first word and item ids after `spawn` or `drop`. Output
goes to the status strip; refusals as warnings.

Drawing: the console takes the status row: `: ` prompt in cyan, the input in
TEXT, a `▮` cursor pulsing in magenta, the hint in DIM after a gap.

## Keys and hints

| Mode | Key | Does |
|---|---|---|
| Walk | `Space` | interact popup |
| Walk | `a` | abilities popup |
| Explore and Walk, dev | `:` | dev console |

Hint rows (both fit 120 columns in dev mode with the fps readout):

- Walk: `hjkl move · <> stairs · space act · a abilities · tab deck{cam} · ? keys`
- Jacked: `⚡ jacked in · space act · a abilities · tab deck{cam} · ? keys`

Help gains `space` (interact: take, jack, stairs, look), `a` (abilities) and
`:` (dev console, dev mode) in the walk table, `:` in the explore table. The
README's walk line and key table follow.

## Testing

Overlay: a pile draws its icon on a visible tile, dimmed on a remembered one,
nothing on an unseen one, `≡` for a mixed pile, `@` on top. Interact: at a
pile the popup lists take rows and take all; next to a terminal it offers jack
in, and jacked in it offers jack out; on a stair tile it offers the direction
that exists; taking moves the stack into the inventory and the glyph goes; jack
in from the popup moves the map to the Net. Abilities: `a` lists Netscan ready;
firing marks tiles Remembered around the player and the second press shows it
cooling; Ping lights a terminal cell within range and not one outside. Console:
unit tests for keys, history, completion and the `ConsoleEvent`s; `spawn`,
`drop`, `give`, `heal`, `clear`, unknown command and unknown item through
`run`; `:` opens it only in dev mode and `Esc` closes; the status row shows
the prompt. Hint rows render without truncation at 120 columns in dev mode
with fps on. The pty smoke adds: console spawn, drop, Space take, `a` scan.
