# Ripperdoc clinics: chrome goes in and out in a ripperdoc's chair

Date: 2026-10-29. Builds on the ATTRIBUTES sheet and body-site chrome
(`2026-10-29-stats-chrome-design.md`, merged to main as 4c328e3), whose non-goals left
the ripperdoc for later. Mockups: `2026-10-29-ripperdoc-clinics-mock/` (round 1 the
concepts, round 2 the chair on the ledger, round 3 doors, the chair's window and fees;
all three approved).

## Why

Chrome is the body's hardware, and today it goes in and out anywhere, from the deck,
free. In the genre it goes in at a ripperdoc's: a back room, off the books, a doc with
bad bedside manner and a price. Gating chrome behind clinics makes the city matter for
your build, gives credits something to buy, and gives the Night Market and the Combat
Zone a reason to visit.

## What Jeffrey decided

- **Chrome changes only in a ripperdoc's chair.** Installing, pulling, swapping, and
  fitting or taking out a piece in a chrome limb's socket. Everything else stays as it
  is: the workbench, fitting into guns and decks, the pack.
- **The chair unlocks the ledger (concept A).** No new screen: sitting down opens the
  deck on ATTRIBUTES with the chrome ledger focused, in a chair mode. Consistency won
  over a dedicated chair screen.
- **One line, not two.** The chair's mark goes on the existing CHROME header line,
  after `CHROME`, in amber: `CHROME · CHROMING UP ──── LOAD ▰▰▰▰▱ 19/24`.
- **The LOAD gauge shows the change**, as it does today: what a pick adds in amber, its
  total in amber. **New:** when the pick won't fit, the added segments and the total go
  magenta, as the picker's load tag and the card's load line already do.
- **Space interacts with the chair like everything else:** its own window, with a row to
  sit in it. Sitting down opens the deck by itself.
- **The chair needs the doc there.** No doc, the clinic's door is shut. Space beside the
  shut door gives a window: CLOSED, with the opening hours.
- **Fees are in, now.** Per piece, with prices that fit the economy (see Fees).
- **Every clinic gets a hand-laid interior, picked at random:** all of the round-1
  concepts (behind a noodle bar, a laundromat's cellar, a shuttered dentist), and more
  layouts are better.
- **Dev:** a console command toggles free swapping anywhere.

## Goals

- Every `BuildingKind::Clinic` in the city is a ripperdoc's clinic behind a front
  business, on its own hand-laid map, its doc in the back by the chair.
- The ledger works in the chair, charges the doc's fee, and only looks anywhere else.
- Door hours follow the doc: open while the doc is in, CLOSED otherwise. The doc keeps
  night hours, 18:00 to 04:00.
- `ripper` in the dev console turns free chrome on and off.

## Non-goals

- No humanity, cyberpsychosis, surgery failure, recovery time or healing at the clinic.
- No buying chrome from the doc; chrome still comes from kits, recipes and the debug
  runners. The doc only puts in and takes out what you bring.
- No front staff: the noodle cook, the laundromat attendant and the receptionist are
  furniture and empty stations for now (follow-up).
- No cold surgical light (the mock's blue-tinted surgery floor): the chair is drawn in
  cyan, and the surgery has its own lamp. A tinted light is a follow-up.
- No haggling, discounts by standing or skill, or per-doc rates.

## Anti-goals

- Chrome must never be changeable outside the chair by some other path (GEAR, a popup,
  a console command other than `ripper`). The lock lives where every chrome change goes
  through: `actions::perform`.
- Kits, runners and `Character::install` stay as they are: setting a character up in
  code is not surgery and charges nothing.
- The chair mode must not be cases sprinkled through the ledger: it is a mode with its
  own state on the app (`AppState::chair`), and the ledger asks one question of it.
- No second header line, ever.

## The city: clinics

### Which buildings

Every `BuildingKind::Clinic` becomes a ripperdoc's clinic. The city's mix already places
them (Night Market, Combat Zone and Old Town, weight 1 each): 0 to 19 in a Medium city
of seven districts, and about one such city in 200 has none (a guarantee is a
follow-up; `ripper` covers it in dev).
Clinics are always `HeightClass::Low`: one street floor, and a basement when the lot
rolled one (`span.lo == Undercity`, 10–15%). None has a lift.

### Fronts and layouts

A clinic hides behind a **front**: what the street sees. Each front has one hand-laid
layout. Five ship:

| Front | Label | Size | Floors |
|---|---|---|---|
| `NoodleBar` | noodle bar | 31×10 | street |
| `Laundromat` | laundromat | 24×9 | street, and the clinic in its cellar (B1) |
| `Dentist` | dentist | 34×7 | street |
| `Tattoo` | tattoo parlour | 26×10 | street |
| `Butcher` | butcher | 26×10 | street |

The front is a pure function of the city's seed and the building's id, so naming and the
interior agree without storing anything:

- a clinic with a basement is always the `Laundromat` (its cellar is the basement);
- every other clinic picks one of the five uniformly, by
  `hash64(city.seed, 0xC1_1C ^ id)`. Picking the `Laundromat` gives the clinic a
  basement level (`FloorPlan::lowest = -1`) that has no door of its own.

Layouts are drawn with their street side at the bottom. The clinic's floor **is** the
layout: the clinic's `FloorPlan::size` is the layout's size, turned so its street side
faces the building's storefront door (the one `generate::exterior::main_door` dresses
as the main one; else the first Street portal), and mirrored left to right on a second
hash bit for variety. Turning means a quarter, half or three-quarter
turn of the grid; furniture glyphs don't turn (a counter is `═` either way).

The layout's front door `E` is the storefront door's exit tile. Nearly every closed
building also has a back door (city generation cuts one on another side). Any door
other than the storefront maps onto the frame as today, and when that tile is taken,
or its doorstep isn't plain floor of the front room, it takes the nearest frame tile
whose doorstep is: so a back door opens on the front room, never straight into the
surgery, a back room or a locked room. On the cellar (B1) a tunnel door steps onto any
plain floor outside the locked room. The basement floor's stairwell `^` sits under the
street floor's `v` (the plan's `stair`).

The legend (as `home.rs`'s, plus the clinic's own):

| | | | |
|---|---|---|---|
| `#` wall | `+` door | `E` front door | `w` window |
| `%` locked door | | | |
| `.` floor | `*` lamp | `g` drain (grate) | `v` / `^` stairs |
| `X` operating chair | `d` the doc's station | `K` counter | `S` stool |
| `H` shelf | `M` machine | `B` bed | `L` locker |
| `C` couch | `O` table | `D` desk | `N` bench |
| `V` vending | | | |

`d` is a `Role::Surgery` station beside `X`; the ripperdoc stands there. Every layout
has one locked room (`%` its door): the pantry, the cellar's stockroom, the dentist's
recovery room, the tattoo parlour's stock cupboard, the butcher's meat locker, so a
cracked lock always opens something, as in every other closed building. The layout's
frame is walls, windows (`w`) and the one `E`; the generic window pass, lamp grid and
furniture passes don't run on clinic floors. Every layout is validated by a test: one
`E` on the bottom row, one `X` with a `d` beside it, the frame closed, every walkable
tile reachable from `E` (on B1, from `^`).

**Behind the noodle bar** — the street sees a counter and stools; past the kitchen's
stoves a door opens on the back room.

```
###############################
#L..D.....L..#H.M.M.M..#LL.HH.#
#.....Xd.....#.........#......#
#.*...g......+.......*.###%####
#L...........#................#
##############KKKKKKKKKKKKKKK.#
#.......*.....S.S.S.S.S.S.S...#
#.............................#
#..O.S....O.S.....O.S.........#
#w###w####w####E####w####w#####
```

**Down the laundromat's cellar** — washers on the street floor, a stair at the back;
below, a stockroom of racked salvage and the surgery with a generator and two cots.

```
########################     ########################
#M.M.M.M.M.M..#H.......#     #HHHH.....#M...M.......#
#.............#....v...#     #.........#........^...#
#.N...N...*...+........#     #..*......%.....X......#
#.............#H.......#     #HHHH.....#.....dg...B.#
#M.M.M.M.M.M..#H.......#     #.........#..D.......B.#
#.............##########     #HHHH.....#....*.......#
#....O....N............#     #.........#.........L..#
#w#w####E###w###########     ########################
        street (L)                  cellar (B1)
```

**The shuttered dentist** — couches and a dead fish tank in the waiting room, the
surgery through the middle door, a recovery room at the back.

```
##################################
#C.C.......D...#.LL.L..#B........#
#..............#.......#.......*.#
#..V.....*.....+...X...%.........#
#C.C...........#...dgD.#B........#
#..............#...*...#.........#
#w####w####E######################
```

**The tattoo parlour** — flash on the walls, two benches, the till; the back room
behind a locker wall.

```
##########################
#H.H.H.H.#L..M....D.#.LL.#
#........#..........#....#
#.N.S..N.#.*....X...%....#
#.S....S.+......dg..#....#
#........#....L.L.L.#....#
#..KKK...#################
#..S.....*...............#
#........................#
#w###w####E####w####w#####
```

**The butcher** — a counter and hooks out front; the chair in the cold room among the
freezers, over a drain.

```
##########################
#L.L.L.L.L.#H.H.H.H.#.LL.#
#..........#........#....#
#....X.....+...*....#....#
#....dg..D.#........##%###
#..*.......#.KKKKKKKKK...#
#L.L.L.L.L.#.............#
############.....V....*..#
#........................#
#w###w####w####E####w#####
```

Rooms: the walled region holding `X` is `RoomKind::Surgery` (listed first), the one the
front door steps into `RoomKind::Waiting`, the rest `RoomKind::BackRoom`; the region
behind a `%` door that holds no chair, stairs or front door is private, the `%` its
door. The generic clinic
layout (`Builder::clinic`, the stool rows and the bed) goes.

### The chair

`Furniture::Chair`, appended last to `Furniture` so saved cities keep their indices.
Glyph `╦`, drawn in cyan (`Rgb(0, 229, 255)` scaled as furniture is by light), label
"operating chair", in the legend.

### Names and labels

The front names the building. `flavor/names.ron` drops the `(Clinic, …)` shop patterns
("Doc {syl}" gives the game away) for a `fronts:` table, one list per front, picked with
the naming stage's rng exactly as `shop_name` picks (so the names of the buildings after
it don't change):

- NoodleBar: "Hot Pot {syl}", "{syl} Noodles", "Ramen {syl}", "{syl} Bowl"
- Laundromat: "{syl} Wash", "Suds {syl}", "{syl} Coin-Op", "Laundromat {syl}"
- Dentist: "Saint {syl} Dental", "{syl} Dental", "Dr {syl}, Dentist"
- Tattoo: "{syl} Ink", "Needle {syl}", "{syl} Tattoo"
- Butcher: "{syl} Meats", "{syl} & Son, Butchers", "Cold Cuts {syl}"

Wherever the game names a building's kind to the player (Look, the HUD and log
fallbacks, the Net's host list), a clinic names its front: one function,
`citygen::kind_label(city, id)`, which every such site calls instead of
`kind.label()`. The dev inspector keeps the true kind. Look gives each front its own
flavour line with a tell in it (key `"{front} front"` in `looks.ron`: "a door at the
back the cook never looks at", "a cold room that hums louder than it should"), so the
attentive can tell a clinic's noodle bar from a real one; the old "red cross" clinic
line goes. The sim's card for the doc at work reads "in the back room" instead of
"in the clinic". Old saved cities keep their old clinic names (names are saved, floors
are not); nothing else about them changes, and `FORMAT_VERSION` stays 4.

### The doc

The `ripperdoc` profession already works at `Clinic` and already wants a
`Role::Surgery` station. Its staff becomes `(1, 1)`: one doc per clinic, so "the doc"
is always one person. Its hours move from 10:00–22:00 to **18:00–04:00**: back-room
docs keep night hours, and the city's clock starts at 22:00 (with the world frozen in
menus, a 22:00 closing would shut every clinic for the first half hour of a session).
Placement puts them on `d`. The doc's lines stay the
profession's.

The doc is called **Doc {first name}** in every line the clinic writes (the window, the
log, the mark's card): the first word of their card name.

## Opening hours: the door

A clinic is **open while its doc is inside, working** (`world.inside(b)` has the
ripperdoc with `OccupantRole::Working`); otherwise it is CLOSED. The hours shown are the
ripperdoc profession's (`18:00` to `04:00`).

- **Walking into a closed clinic's door** (any of its doors, street or tunnel) is
  refused, nothing moves, and the log says `Hot Pot Lou is closed · opens at 18:00`.
  The refusal lives in `interiors::enter`, which every door goes through.
- **Space beside a closed clinic's door** (on the city map, the door tile next to you)
  gives the door its window, like a flat's keypad: the header is the building's name
  with the door glyph `▯` in amber; the subtitle `CLOSED · opens 18:00 · closes 04:00`;
  one row, `x Look`. An open clinic's door has no window: you walk in.
- **Inside at closing time** the doors still let you out. If the doc leaves while you
  are in the chair, the chair is over (below).

## The chair: sitting down

On a clinic floor, Space beside the chair gives the chair its window, like everything
else in reach (`interact::gather`):

```
┍━ ╦ Operating chair ━━━━━━━━━━━━━━━━━━━┑
│                                       │
│ Doc Lou · “Sit. Don't touch the tray.”│
│                                       │
│ ▸ x  Look                             │
│   i  Interact: Sit in the chair       │
└───────────────────────────────────────┘
```

- The subtitle is `Doc {name} · “{line}”`, the line one of the doc's, picked as their
  Talk line is.
- `i Interact: Sit in the chair` (a new `Pick::Chair`): sets `AppState::chair` and
  replaces the window with the deck, opened on ATTRIBUTES with the ledger focused on its
  first chrome row. The doc's line goes to the log (`Doc Lou: “…”`).
- With no doc at work in the building (they left at closing), the row is disabled with
  the tail ` · the doc is out`.

`AppState::chair: Option<Chair>`, where `Chair { building, level, doc: PersonId }`.
It holds only while the player is on that floor and that doc is working in the
building, checked whenever it is asked (`operating`), so nothing has to clear it when
the player moves. It ends (back to `None`) when:

- the deck closes (esc or tab, as today), which logs `you get out of the chair`;
- a chrome action finds it no longer holds: refused with `Doc Lou has gone for the
  night`, and the chair ends.

Q and E still change tabs in the chair; the chair holds until the deck closes.

## The ledger in the chair

The one question the ledger asks: `clinic::operating(st) -> Operating`, one of

- `Chair { doc: String }` — in a chair that holds: chrome changes, with fees;
- `Free` — dev free chrome (`ripper`): chrome changes anywhere, no fees;
- `Locked` — anywhere else: the ledger only looks;
- `Gone { doc: String }` — a chair that no longer holds: drawn as `Locked`, refused
  with the doc's farewell.

Jacked in, the existing lock stays first and unchanged (`JACKED_IN`).

### In the chair: CHROMING UP

- **The mark:** `CHROME · CHROMING UP` on the header line: `· CHROMING UP` in amber,
  bold, straight after `CHROME`, then the rule and the LOAD tail as today. At 100×30 the
  header fits it with room for the rule.
- **The divider** between the panes (`║`) is amber while the mark shows.
- **Hints, on a row:** `↑↓ select · space what fits · x pull it out ¤110 · ← attributes
  · esc get out of the chair`, the pull's fee in amber after `x pull it out` (on a piece;
  on an empty site no `x` part, as today). Q/E still work; the hint makes room by not
  naming them.
- **Hints, in FITS:** as today (`↑↓ choose · space install · esc back · Q/E tabs · tab
  close`, `space swap` on a held place).

### FITS: the fee line

The FITS card gets one line straight under its load line (the card's flavour text moves
down one; the card already folds to fit):

- a fee you can pay: `fee ¤370 · leaves ¤830`, the amount amber and bold, the rest dim;
- a swap: `fee ¤480 · in ¤370 · out ¤110`;
- a fee you can't pay: `fee ¤370 · you have ¤320`, all in magenta; `space install`
  (or `swap`) in the hint dims to FAINTER and space refuses, the picker staying open,
  with `the doc's fee is ¤370 · you have ¤320` in the strip.

### LOAD over budget

`chrome_header` with a pick whose total exceeds the budget draws the added segments and
the total in **magenta** instead of amber (and, as now, the picker's `L4` tag and the
card's `load 19 → 23/22` in magenta). A pick that fits stays amber; a lighter swap stays
as today (the lower total, no amber).

### `x` asks first

Pulling costs, so `x` on a held row (in the chair, with a fee above zero) arms the pull
instead of doing it. The ledger keeps `pulling: Option<LedgerRow>`; the hint row turns
into

`x again to pull Piston arm for ¤110 · it comes out with its Arc hand · esc keep it`

(`x again to pull` and the price amber, the piece's name TEXT, the rest dim; the
"comes out with" part only when something is fitted, naming the first fitted piece, or
`with what's fitted in it` for more than one). A second `x` on the same row pulls and
pays; `esc` disarms (and does not close the deck); any other key disarms and then does
what it does. With free chrome, `x` pulls at once, as today.

### Anywhere else

The ledger only looks:

- no mark on the header; the divider as today;
- hints: `↑↓ select · ← attributes · Q/E tabs · tab close`, then three spaces and
  `chrome goes in and out in a ripperdoc's chair` in FAINTER;
- space or `x` on a row refuses in the strip: `chrome goes in and out in a ripperdoc's
  chair`. Space does not open FITS.

### Dev: free chrome

- The console's `ripper` toggles `AppState::free_chrome` and answers `ripper on · chrome
  swaps anywhere, free` or `ripper off`. It is off by default: the app starts in dev
  mode, and the clinics are what you play.
- On, the ledger works anywhere with no fees and no `x` confirm; the mark reads
  ` FREE ` in the DEV badge's colours (black on amber), the divider amber.
- In a chair with free chrome on, free wins: no fees.
- `ripper` is in the console's command list, its completions and its help.

## Fees

The doc charges for the piece each action moves, by the piece's `value`:

- **putting a piece in** (install at a site, or fitting into a chrome limb's socket):
  **12%** of its value;
- **taking a piece out** (pulling from a site, or out of a socket): **4%**;
- each rounded to the nearest ¤10, at least ¤10;
- **a swap** is both: the new piece in, the old one out;
- what is fitted in a piece rides along free: pulling an arm with a hand in it costs the
  arm's take-out fee.

| piece | value | put in | take out |
|---|---|---|---|
| Smart ink | ¤700 | ¤80 | ¤30 |
| Neural port | ¤1500 | ¤180 | ¤60 |
| Target lens | ¤2200 | ¤260 | ¤90 |
| Arc hand | ¤2600 | ¤310 | ¤100 |
| Piston arm | ¤2800 | ¤340 | ¤110 |
| Grip arm | ¤3100 | ¤370 | ¤120 |
| Wrecker arm | ¤4900 | ¤590 | ¤200 |
| Redline spine | ¤5200 | ¤620 | ¤210 |
| Ceramic plating | ¤6800 | ¤820 | ¤270 |

Against the economy: a fetch pays ¤200, a vault job up to about ¤1000, and a runner
starts with ¤250–600, so a first piece costs a job or two.

The rates live in content: `surgery: (install: 12, pull: 4)` in `flavor/content.ron`, a
`Surgery` struct on `game::Content` (serde default 12 and 4). The rule lives in `game`:
`game::surgery_fee(&Character, &Content, &Action) -> Option<u32>` is the fee for a
chrome action (`Install`, `InstallCopy`, `Pull`, `Fit`/`Unfit` with `Host::Site`),
`None` for every other action; plus `game::put_in_fee` / `take_out_fee` for one item,
which the ledger uses to draw.

`actions::perform`, for an action `surgery_fee` prices:

1. jacked in: `JACKED_IN`, as today;
2. `Locked`: refused, `chrome goes in and out in a ripperdoc's chair`;
3. the chair's doc gone: refused, `Doc Lou has gone for the night`, the chair ends;
4. `Chair` and the fee above your credits: refused, `the doc's fee is ¤370 · you have
   ¤320`;
5. otherwise `game::apply`; on success the credits go down by the fee (none with
   `Free`), and the message gains ` · ¤370 to Doc Lou`.

Every refusal comes before any change.

## Where things live

- `citygen`: `Front` (the five), `clinic_front(city, id) -> Option<Front>`,
  `Front::label`, `kind_label(city, id)`, `Floor::chair()`; the layouts in
  `interior/layouts/clinics.rs` (the `home.rs` pattern: `const` grids and a stamp that
  turns and mirrors); `FloorPlan::new` sizing clinic floors from the layout, its stair,
  its forced basement and the front door's exit tile; `Furniture::Chair`; the names.
- `sim`: ripperdoc staff `(1, 1)`; the card's "in the back room".
- `game`: `Surgery` rates on `Content`, the fee functions.
- `strata-zero`: a `clinic` module (open/closed, the hours, the doc's name, the chair's
  state and `operating`); `actions::perform`'s lock and fee; `interact` (the chair's
  window, the shut door's window); `explore` (the refused door); the popup's
  `Pick::Chair`; the deck (opening in the chair, ending it on close); the ledger (mark,
  divider, hints, fee line, magenta over budget, `x` asks first); the console's
  `ripper`; the tileset's chair glyph and legend.

## Testing

- `citygen`: every layout validates (shape, one `E`, the chair and its `d`, closed
  frame, reachable); every clinic in seeds 1, 2, 3, 8, 11, 21 (Medium) gets a front,
  a floor of its turned layout's size that passes `validate_floor`, a chair, a surgery
  station, and an exit for every portal; a basement clinic is a laundromat; the front is
  stable across calls; clinic names come from their front's list.
- `game`: fees by the table above; a swap is in plus out; fitted pieces ride free;
  non-chrome actions have no fee.
- `strata-zero` (tests at 22:00 for open, 12:00 for shut): the lock refuses every
  chrome action outside a chair and allows it in one; the fee is charged and refused when short; the chair ends on close, on leaving
  the floor and when the doc goes; the door refuses and its window shows CLOSED with the
  hours; the chair's window offers Sit and opens the deck on the ledger; the ledger's
  mark, divider, hints, fee lines, magenta header and `x` confirm; `ripper` toggles and
  makes it free; jacked in still wins.
- Shots for the shot harness: the chair's window, the shut door, the ledger in the
  chair (on a piece, FITS paying, FITS short, a swap, `x` asking), free chrome.

## My calls (Jeffrey to check)

From the builder check and the build:

- **Night hours.** The doc works 18:00–04:00 instead of the content's 10:00–22:00: the
  clock starts at 22:00, so day hours would shut every clinic at the start of every
  session. It also fits a back-room doc.
- **The storefront door faces the street side**, and every other door opens on the
  front room, never the surgery (most clinics have a back door).
- **A locked room in every layout**, to keep "every closed building has a locked room".
- **Tells**: each front's Look line hints at what's behind it.
- **Cities with no clinic** (about 1 Medium city in 200) are left as they are for now.
- **The chair is checked when asked**, not cleared on every move: on the wrong floor or
  with the doc gone it simply doesn't hold.
- **Glyph `╦`** for the chair, in cyan, as in the mocks.
- **One doc per clinic**, no front staff.
- **Fees round to ¤10 with a floor of ¤10**; what is fitted in a moved piece rides free.

## Follow-ups

- Front staff: a noodle cook, an attendant, a receptionist at the fronts' stations.
- A cold, tinted surgical light.
- Per-doc rates, discounts by standing, buying chrome from the doc.
- More layouts.
- A clinic in every city (about 1 Medium city in 200 has none).
- Ways to learn where the clinics are: a fixer's word, a journal note once visited.
