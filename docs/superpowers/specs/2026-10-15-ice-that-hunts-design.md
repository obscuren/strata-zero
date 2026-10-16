# ICE that hunts — design (netrunning, spec 2 of 3)

Date: 2026-10-15. Status: approved in chat.

## Goal

ICE stops being a door. Grey and black ICE patrol the construct's spine and
see along it; seeing you wakes a piece, and an awake piece hunts you by the
shortest path. Beside you it hits every turn: white adds heat, grey drains
focus and dumps you at zero, black takes HP through the jack. You fight back
with programs (attack, shield), hide in rooms, mask yourself, run for the
gate or jack out. Black ICE is the first thing in the game that can kill
you, behind a failsafe you can turn off.

## Decisions taken with Jeffrey

- **Patrols the spine, hunts on sight, follows the trace.** White keeps its
  post. Rooms off the spine are where you hide.
- **Integrity versus your deck, in turns.** Integrity by grade; attack and
  shield programs; crack still works, slowly; the deck's rating is armour.
- **Black ICE hurts the meat.** HP through the jack; a failsafe dumps you hard
  at a quarter of your HP; off, you can die. Death is new to the game and
  gets a FLATLINED screen.
- **Stealth is sight cones, rooms and a mask program.**

Assumptions carried in: spec 1's "awake ICE beside you dumps you" becomes a
hit by grade (a grey hit drains focus; the dump comes at zero focus); ICE
state lives with the Net's session state, nothing saved; one construct's ICE
acts only while the player is inside it.

## ICE (`citygen`)

- `IceGrade::{White, Grey, Black}`; `label()`; `integrity()` 3 / 6 / 10;
  `sight()` 4 / 6 / 8 tiles.
- `Guard { post: Pos, pos: Pos, grade: IceGrade, beat: Vec<Pos> }`. `pos` is
  where the piece stands now (the tile that carries `Feature::Ice`); `post`
  where it was generated; `beat` the spine tiles it walks when asleep, in
  order, empty for white.
- Generation, by the subnet's security: below 160 no ICE; 160–199 a white
  gate guard; 200–229 a grey gate guard and, with a vault, a grey vault
  guard; 230 and up a grey gate guard and, with a vault, a **black** vault
  guard. The gate guard's beat is the spine's near half (from its post to the
  spine's middle), the vault guard's the far half (from the spine's middle to
  the end room's door, its post). White has no beat.
- `Construct::move_guard(i, to)` moves the feature with the piece;
  `guard_at`, `set_guard` unchanged. `validate_construct` checks every beat
  tile is walkable spine (`NetLane`/`NetGate`).

## Programs (content)

```ron
Item(id: "attack", name: "Attack", kind: Program, value: 400,
     program: Some(Program(op: Attack, rating: 1, size: 1, noise: 3, work: 1)),
     desc: "A spike of bad arithmetic aimed at whatever is looking at you."),
Item(id: "shield", name: "Shield", kind: Program, value: 300,
     program: Some(Program(op: Shield, rating: 1, size: 1, noise: 1, work: 1)),
     desc: "Soaks the next hit. Then it is gone."),
Item(id: "mask", name: "Mask", kind: Program, value: 350,
     program: Some(Program(op: Mask, rating: 1, size: 1, noise: 1, work: 1)),
     desc: "You are still there. You are just harder to see."),
```

- `Op::{Attack, Shield, Mask}`; `Target::Deck` (a program on yourself);
  `HackTarget::Deck`. `applies`: Attack on any guard; Shield and Mask on
  `Deck` only; Crack on guards as before. `skill_for`: Attack and Shield use
  `ice_breaker`, Mask uses `stealth`.
- An attack turn that succeeds deals `2 × rating` integrity (plus one with
  the `black_ice` talent flag). A shield landing adds one charge (two at
  most); a hit consumes a charge before it lands. A mask landing holds for
  `MASK_TURNS = 6` turns; each held turn adds one heat; while masked every
  piece's sight is halved (rounded down).
- The netrunner's kit gains `attack`; the pawnshop deck stays at two slots
  (loading is the choice).

## ICE state and turns (app, `net.rs`)

```rust
pub enum IceMode { Asleep, Awake { seen: Pos, lost: u8 } }
pub struct Ice { pub mode: IceMode, pub integrity: u8, pub step: usize, pub forward: bool }
// NetState gains:
pub ice: BTreeMap<BuildingId, Vec<Ice>>,   // one per guard, made when the construct is first entered
pub shield: u8, pub mask: u8, pub failsafe: bool (true),
pub death: Option<String>,                 // set when black ICE kills; the screen reads it
pub const MASK_TURNS: u8 = 6; pub const LOST_TURNS: u8 = 3; pub const FAILSAFE_COOLING: u64 = 30;
```

- **A turn** is a wait, a `continue`, or a move. After the player's part
  (`step_run`), `ice_turn(st)` runs for the construct the player is in:
  1. The mask ticks down (heat +1 while it holds).
  2. For every standing piece: **sight**: the player is seen when they stand
     on spine (`NetLane`/`NetGate`), in the piece's row or column, within
     `sight` (halved under mask), with no wall between (Bresenham). Seen →
     `Awake { seen: player, lost: 0 }`.
  3. **Act**: Asleep grey/black steps along its beat (forward to the end,
     back to the start); white stands. Awake and adjacent (Chebyshev 1):
     **hit**. Awake and not adjacent: one step along the A* path to `seen`
     over walkable tiles not held by another piece or the player; on
     reaching `seen` without sight, `lost += 1`; at `LOST_TURNS` it falls
     asleep and walks back along the shortest path to its beat.
  4. The alarm (spec 1) wakes every piece with `seen` = the player's tile.
- **Hits** (the deck's rating is taken off grey and black hits, never below
  one): white `heat += 5` (through `Trace::add`, so it can alarm or dump);
  grey `focus -= 10`, at zero focus `dump("grey ICE drained you")`; black
  `hp -= 6 + (security − 230) / 5`; then the failsafe: with it on and
  `hp ≤ hp_max / 4`, a hard dump (`dump("the failsafe pulled you out")`,
  cooling `FAILSAFE_COOLING`); with it off and `hp == 0`,
  `death = Some("black ICE at <building>")`.
- A shield charge consumes a hit entirely (`shield absorbs the hit` in the
  log). Each hit logs `white ICE hits · heat 46`, `grey ICE hits · focus 12`,
  `black ICE hits · hp 20`.
- **Attack**: a run against a guard with `Op::Attack`; each successful turn
  takes integrity (`attack · grey ICE 4/6`); at zero the piece is broken:
  `land(b, Guard, kind, Op::Attack, now)` (lifted by `sync_guards` like a
  crack, for the hour or the alert) and `grey ICE broken`.
- Breaking or cracking a piece resets its integrity when it stands again.
- `turn_ends` of spec 1 (the instant dump beside awake ICE) is replaced by
  the hit.
- **FLATLINED**: when `net.death` is set, the explore screen switches to
  `screens::flatlined::Flatlined` (a block-font FLATLINED, the cause, the
  handle and level, `any key · title`), which switches to `Title`. The
  character is not saved; the city save is untouched.

## Reading it

- HERE in a construct: after the trace lines, one line per standing piece in
  sight memory: `▣ grey ICE · hunting · 4/6` / `· asleep` / `· awake`;
  `SHIELD ×1` and `MASK 4` while they hold.
- The tileset draws an awake piece brighter (full scale, no pulse) and black
  ICE in deep red `(255, 40, 40)`.
- The status strip's run verbs stay; `hp` and `focus` bars already show the
  damage.
- CYBERDECK tab: a row `failsafe · on` / `failsafe · off (black ICE can kill
  you)` after the deck row; `space` toggles; the detail explains it.
- Console: `wake` wakes every piece of the construct you are in; `ice` lists
  them (`0 grey asleep 6/6 at 2,5`); `failsafe on|off`.

## Out of scope

ICE outside constructs, NPC netrunners, ICE repairing itself, programs that
heal, selling paydata, saving the Net's state.

## Tests

citygen: grades by security including black at 230, beats on walkable spine
and halves, `move_guard` moving the feature, validation. game: `applies`
and `skill_for` for the three ops, `Target::Deck`. app: a grey piece asleep
walks its beat and turns; sees the player on the spine within range and not
in a room and not beyond range; halved under mask; wakes and closes by one
tile a turn; loses the player after three turns and goes back; the alarm
wakes all; hits by grade with the deck's rating off; the shield absorbing;
grey draining to a dump; black with the failsafe dumping at a quarter with
the long cooling; black with the failsafe off killing and the FLATLINED
screen reaching the title; attack breaking a piece and `sync_guards` lifting
it; integrity back when it stands again; the HERE lines; the cyberdeck
toggle; the console commands; a smoke run and screenshots of a hunt and the
FLATLINED screen.
