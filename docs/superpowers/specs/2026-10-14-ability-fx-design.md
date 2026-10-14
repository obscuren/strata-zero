# Ability effects design

Every active ability plays a visual effect when it fires. The effects come
from a small vocabulary that abilities pick from in the content file, so a
future ability gets its visuals by naming shapes and colours, not by code.
Netscan is the first: a cyan ripple that paints the revealed map outward. Ping keeps its glowing markers and gains a magenta
wavefront.

## Vocabulary

`game::model::Fx`, serialised in RON as part of `Ability`:

```rust
pub enum Fx {
    /// A glitch burst over the map for this many ticks, thinning as it ends.
    Kick(u64),
    /// A ring that expands from the firer to the ability's radius over `ticks`.
    Wave { colour: (u8, u8, u8), ticks: u64 },
    /// A pulsing marker on terminals and stairs within the radius for `ticks`.
    Glow { colour: (u8, u8, u8), ticks: u64 },
}
```

`Ability` gains `#[serde(default)] pub fx: Vec<Fx>`. `Active::Ping` loses
its `ticks` field (the Glow carries the duration now) and reads
`Ping { radius }`. `Effect::Ping` is removed; `Effect::Reveal` stays.
`Content::validate` rejects `fx` on a passive ability with
`ContentError::FxOnPassive { ability }`.

Content:

```ron
Ability(id: "netscan", ..., kind: Active(Scan(radius: 12)),
        fx: [Wave(colour: (0, 229, 255), ticks: 15)]),
Ability(id: "ping", ..., kind: Active(Ping(radius: 24)),
        fx: [Wave(colour: (255, 0, 200), ticks: 20), Glow(colour: (255, 0, 200), ticks: 180)]),
```

## Firing

`Player::reveal` returns the tiles it newly revealed (`Vec<Pos>`, those
that went from Unseen to Remembered). `actions::perform`, after a
successful `Fire(id)`, pushes one `FxInstance` per entry of the ability's
`fx` into `AppState::fx`:

```rust
pub struct FxInstance {
    pub fx: Fx,
    pub stratum: StratumKind,
    pub origin: Pos,
    /// The ability's radius: Scan or Ping.
    pub radius: i32,
    pub started: u64,
    /// For a scan, the tiles it revealed; the wave hides them until it passes.
    pub revealed: Vec<Pos>,
}
impl FxInstance {
    /// The tick from which the instance draws nothing.
    pub fn until(&self) -> u64;   // Kick: started + ticks; Wave: started + ticks + tail; Glow: started + ticks
    // tail = ceil(9 * ticks / reach): the ticks the front needs to carry its ripple
    // band past the reach, so the ring fades out instead of popping.
}
```

`AppState::prune_fx(tick)` drops ended instances; the map screen calls it
every update. A kick that ends sets `AppState::repaint`, which the runner
turns into a full repaint, as after a transition. Entering a city (new or
loaded) clears the effects along with the body. The old `Ping` struct, `AppState::ping` and `PingOverlay` go
away.

## Rendering

`render/fx.rs` holds `FxOverlay { fx: &[FxInstance], vis }`, an `Overlay`
drawn after the ground items. Only instances on the drawn stratum draw.
With `elapsed = tick - started`:

- **Kick(ticks)**: while `elapsed < ticks`, `glitch_cells(area, tick, p)`
  with `p = elapsed / ticks`, each cell's glyph replaced and tinted cyan or
  magenta by cell hash, as the jack-in flash does.
- **Wave**: distances are measured on screen, a row counting as two cell
  widths, so the ring is round in the terminal; it travels to the corner of
  the ability's square radius (`reach = radius * sqrt 5` cell widths) so every
  revealed tile gets painted. The front is at `r = reach * elapsed / ticks`,
  running on for `ceil(9 * ticks / reach)` ticks so the band clears. For every
  screen cell within reach:
  - hidden, `p` in `revealed` and `d > r`: an unseen tile (space on black).
  - front, `r - 1 < d <= r`: foreground the colour, background the colour at
    35%; one cell in ten gets a spark glyph from `ｦ ｱ ﾂ ﾈ ╳ ≋`.
  - ripple, `r - 9 < d <= r - 1`: with `u = r - d`, brightness
    `b = (1 - u/9) * (0.5 + 0.5 cos(2π u / 3))`, so crests trail the front
    every three cells and fade; alternate crests lean toward a deeper blue.
    Foreground the crest colour at `0.25 + 0.75 b`, background at `0.3 b`. Near
    a crest (`b > 0.55`) the cell shows the glyph one step further out from the
    origin, a lens-like shimmer. The DIM modifier is cleared throughout.
- **Glow**: what Ping draws today: every connector serving the stratum
  within the radius gets a pulsing foreground in the colour, and a `◆`
  marker when out of sight.

The action bar slot of an ability fired fewer than six ticks ago draws
inverted (background cyan, foreground the badge black).

## Edges

- An instance on another stratum, or ended, draws nothing; pruning keeps
  the list short.
- A Wave on an ability with no radius (none exist) would use radius 0 and
  end at once.
- `revealed` may hold hundreds of tiles for a scan; the overlay checks
  membership with a set built once per draw.
- Firing twice before the first wave ends shows both; each instance is
  independent.

## Testing

- game: `fx` parses from the content file for netscan and ping; a passive
  ability with `fx` fails validation; `Fire(ping)` yields no effects and
  `Fire(netscan)` still yields `Reveal`.
- walker: `reveal` returns only newly revealed tiles, none on a second call.
- app/actions: firing netscan pushes a Kick and a Wave with the revealed
  tiles; firing ping pushes a Wave and a Glow; `until` per shape;
  `prune_fx` drops ended instances only.
- render/fx: at a mid tick the front cells carry the colour and a hidden
  revealed tile beyond the front is blank while one behind it is not; a
  Kick draws glitch glyphs on its first tick and none after; a Glow marks
  a connector out of sight; an instance on another stratum draws nothing.
- hud: a fired slot draws inverted.
- explore: pressing `1` leaves two instances in the state and the frame
  draws; after enough ticks they are pruned.
- smoke: fire netscan and snapshot two ticks later.

## Out of scope

Sound, screen shake, effects for passives, effects tied to anything but
abilities.
