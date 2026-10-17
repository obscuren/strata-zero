# Credentials and the journal: design

Date: 2026-10-17. Status: built on `feat/credentials-journal` (approach A: a small
store of learned codes, the rest derived; stable homes fixed as part of it).
Built as specified except: households spread round-robin over the levels with the
seed picking the first level, so neighbours are on every floor; the lost card's
drop generates the lobby itself.
Builds on: `2026-10-15-interiors-design.md` (floors, locked flats, placement),
`2026-10-15-constructs-and-programs-design.md` (the run, the copy op, the
lock crack) and `2026-10-17-megablock-home-design.md` (the player's flat,
still on its own branch).
Riff notes this came out of: `docs/ideas/2026-10-17-terminals-access-apartment.md`.

## Intent

The first step of the hacking and terminal line. Locked flat doors get two
new ways in beside the Net's lock crack: a **keycard** you hold and a **door
code** you know. Cards are items; codes are knowledge, and knowledge lives in
the **journal**, which stops being empty. Every later step (keypads, the
Access Terminal's tiers, backdoors with a lifetime) writes into the same two
places, so this slice is the foundation, and it ships with one lock to prove
it: a resident's flat.

Along the way a flaw in the interiors gets fixed: a flat's owner is worked out
from whoever is inside the building the moment a floor is first looked at, so
a resident who is out has no flat. Credentials need a flat that is always
somebody's. **Homes become stable.**

## Goals

- Every resident of a residential building (one whose floors have `Flat`
  rooms: Apartment, Megablock) has a flat, in or out, the same one every time. The door label
  "locked · Mara Voss's flat 1204" never changes and never goes blank.
- Every flat has a four-digit door code, fixed by the city seed.
- A keycard is an item that names its owner and the flat; it stacks, drops
  and is picked up like any item.
- A locked flat opens for the Net's lock crack (as today), for the right
  keycard in the pack, or for the flat's code in the journal. The log says
  which.
- Codes come from the Net: a server copy on a residential building writes
  every flat's code to the journal.
- Keycards come from the world: one household per residential building has
  left its card at the lobby desk; every other resident carries their own.
- The JOURNAL tab lists what you know and what you hold, grouped by building,
  each entry one informative line with its names lit.
- The player's own card and code are one call away, for the apartment branch
  to make at new game once it merges.

## Non-goals

- A lifetime on any credential: no rotation, no revocation. The journal entry
  keeps a `source` and a tick so a later slice can say what burned.
- Keypads and the keypad screen. A code opens the door on bump; keying it in
  by hand is the hardware-frames slice.
- Taking a card off a person: no pickpocketing, no bodies. The rule that a
  resident carries their card is data only.
- Terminal access tiers (GUEST, USER, ADMIN) and the Access Terminal.
- Backdoor lifetimes, chaining, remote jack-in. The journal only lists the
  Net backdoors that already exist.
- The corkboard view of the journal.
- Saving the journal to disk. The game saves only cities today; a load
  respawns and the journal starts empty, like the Net state.

## 1. Stable homes

A new `homes` map per building, computed once beside the floor plan and
cached with it in `Interiors`:

- **Households** of the building: every person with `home == Some(b)`,
  grouped by `HouseholdId`, households in id order.
- **Flats** of the building: every `RoomKind::Flat` room on every corridor
  floor, in level-then-number order. This generates the corridor floors through
  `Interiors::ensure`, which is cheap and deterministic.
- **Assignment:** household *i* takes flat `(start + i) mod flats.len()`,
  where `start = hash64(seed, building) mod flats.len()`. When there are more
  households than flats, flats are shared; `owner_of` names the first.
- A building with no flats (an office, a shop) has an empty map.

```rust
pub struct Homes {
    /// Flat (level, number) by household.
    by_household: BTreeMap<HouseholdId, (i32, u16)>,
    /// The households in a flat, first is the owner.
    residents: BTreeMap<(i32, u16), Vec<HouseholdId>>,
}
impl Homes {
    pub fn flat_of(&self, person: &Person) -> Option<(i32, u16)>;
    pub fn owner_of(&self, level: i32, number: u16, world: &World) -> Option<PersonId>;
    pub fn flats(&self) -> impl Iterator<Item = (i32, u16)>;
}
```

`Placement::new` stops rolling flats for residents. A resident inside the
building stands in the flat `Homes` gives them (the old hash picked a corridor
floor and a flat; now the level and room come from `Homes`). Hotel guests keep
the old roll: a hotel room is not a home. `Placement::owner_of(door)` is kept
for callers but reads `Homes` through the room behind the door, so the owner
is named with nobody inside.

**Door codes.** `code_of(seed, building, level, number) -> u16` in 0..=9999,
hashed from the seed, the building and the flat, shown zero-padded to four
digits. Pure, in `citygen` next to `hash64`, so the Net and the journal agree
without sharing state.

## 2. The credentials module

`crates/strata-zero/src/credentials.rs`, one question and one store.

```rust
/// How a locked door opened.
pub enum How { Cracked, Keycard, Code(u16) }

/// Whether the player gets through this flat's door, and how.
pub fn opens(st: &AppState, building: BuildingId, level: i32, number: u16) -> Option<How>;
```

Answered in order: the Net lock crack live on the building
(`st.net.unlocked`), a keycard for that flat in the pack
(`st.character.count(&keycard_id(building, household)) > 0`), the flat's code
in the journal. The door gate in `MapScreen::gate` (explore.rs) calls `opens`
instead of its inline check. The room behind the door, its owner and the
household come from the floor and `Homes`.

Log lines on passing a door, names in backticks so they light:

- cracked: unchanged, the door just opens (the crack's own line said so).
- keycard: "`Mara Voss`'s flat 1204 · the keycard opens it"
- code: "`Mara Voss`'s flat 1204 · you key in 4471"
- none: "locked · Mara Voss's flat 1204", unchanged.

**The journal store**, on `AppState` as `journal: Journal`, beside `net`:

```rust
pub struct Code {
    pub building: BuildingId,
    pub level: i32,
    pub number: u16,
    pub household: HouseholdId,
    pub code: u16,
    pub source: Source,
    pub at: u64,          // game minute
}
pub enum Source { ServerCopy, Home }
pub struct Journal { codes: Vec<Code> }
impl Journal {
    /// Records a code; false when it was already known.
    pub fn learn(&mut self, code: Code) -> bool;
    pub fn code_for(&self, building: BuildingId, level: i32, number: u16) -> Option<&Code>;
    pub fn codes(&self) -> &[Code];
}
```

A code is known once per flat; learning it again returns false and changes
nothing. `Source::Home` is for the player's own flat (section 3).

## 3. Keycards as items

**Catalogue entries.** When a city becomes the current one (new game, load),
`credentials::stock(content, city, world, interiors)` appends one item per
household that has a flat:

- id `keycard:<building index>:<household index>`
- name "Keycard · Mara Voss, Hollis Court 1204"; a nameless building uses its
  kind label, as `building_name` does
- kind `Component`, weight 0.01, value 40
- desc "A plastic rectangle with a chip and a lanyard. It opens one door in
  the city, and that door is 1204 at Hollis Court."

The entries are appended, never read from the content file, and are rebuilt
when the city changes (the previous city's cards are removed first, matched by
the `keycard:` prefix). `keycard_id(building, household)` and
`parse_keycard(&ItemId) -> Option<(BuildingId, HouseholdId)>` live in the
module so the journal can recognise a card in the pack.

**At the lobby desk.** When a residential building's ground floor is first
generated (`Interiors::ensure` returns a new level-0 floor), one household's
card lies on the floor: the household is
`households[hash64(seed, building ^ 0xCA2D) mod households.len()]`, the tile
is the floor's `Reception` station if it has one, else its first `Table`,
else the first free floor tile inside the lobby room. The drop goes through
`st.ground.add(Place::Floor { building, level: 0 }, tile, id, 1)` once; the
ground keeps it from then on, so re-entering does not duplicate it. This runs
in `interiors::enter` and `ride`/`climb` after `ensure`, from one helper
`credentials::drop_lost_card(st, building)`, which does nothing when the
floor was already there.

**Carried.** `credentials::carries(person, homes) -> Vec<ItemId>`: a resident
carries their household's keycard unless it is the lost one. Nothing in the
game takes it yet. The person's look card (`inspector_lines`) adds the line
"a keycard on a lanyard" when the list is not empty, so the data shows.

**Your own.** `credentials::move_in(st, building, household)`: gives the
keycard and learns the code with `Source::Home`. The apartment branch calls it
at new game; this branch only provides and tests it.

## 4. Codes from the Net

In `net::apply` (the match that turns a landed op into effects), the
`(Op::Copy, Target::Node(Server))` arm, after giving the paydata shard, calls
`credentials::copy_codes(st, building) -> usize`, the number of codes new to
the journal. For a residential building (one with flats) this learns every
flat's code with `Source::ServerCopy` at the current game minute. The line:

- new codes: "server copied · a `Paydata shard` in the pack · `Hollis Court`'s
  door codes are in the journal"
- nothing new: "server copied · a `Paydata shard` in the pack · the door codes
  were already yours"
- not residential: unchanged, "server copied · a `Paydata shard` in the pack"

A data vault copy is unchanged.

## 5. The JOURNAL tab

`Deck::journal_rows(st)` builds the tab from three sources, read fresh on
every draw so nothing drifts:

- codes: `st.journal.codes()`
- cards: inventory stacks whose id parses as a keycard
- backdoors: `st.net.backdoors`

Grouped under one dim header per building, "HOLLIS COURT · Lowtown"
(building name upper-cased, the district's name). Buildings with a code or a
backdoor come first, newest tick first; buildings known only by a card follow,
by name, since the pack does not record when a card was picked up. Within a
building: codes by flat, then cards by flat, then the backdoor.

Each entry is one line, written so it says what it is, whose it is and where
it belongs. Names are marked with backticks and parsed by `messages::parse_marks`,
then drawn as spans in the log's mark colour through the styled-row path the
deck already has (`b.line.spans`):

```
HOLLIS COURT · Lowtown
  door code 4471 · `Mara Voss`'s flat `1204`, level 12 · copied off the server, day 1 21:40
  keycard · `Dev Okafor`'s flat `1107`, level 11 · in your pack
  backdoor · `Hollis Court`'s gate is yours · planted day 2 03:12
MERIDIAN TOWERS · Corp Core
  keycard · `Ana Reyes`'s flat `0304`, level 3 · in your pack
```

The time is the game clock in the form the HUD uses (day and HH:MM). A code
with `Source::Home` reads "· your own door". The selected row's detail card
gives the full text: the owner's name and profession, the building and
district, the flat and level, the code or the card, and where the entry came
from. Headers are not selectable. An empty journal keeps one dim row,
"nothing known yet".

The deck's `Row` grows an optional `spans: Option<Line>` (or the journal rows
use the existing styled-row branch directly, whichever the code allows with
less change); `Detail` gains `Journal(usize)` indexing a flat list of entries
built alongside the rows.

## Testing

- `Homes`: every resident of a residential building gets a flat; the same
  city gives the same answer twice; no two households share while flats
  remain; a building without flats has an empty map; `owner_of` names the
  first household's first member with nobody inside.
- `code_of`: four digits, stable, differs between flats.
- `opens`: none without anything; `Cracked` with a live crack; `Keycard` with
  the card in the pack; `Code` with the code learned; the order when several
  apply.
- `Journal::learn` twice returns false the second time and keeps one entry.
- `stock`: one card per household with a flat, ids parse back, rebuilt on a
  new city without duplicates.
- `drop_lost_card`: the card lies at the reception on first generation and
  is not duplicated on re-entry; `carries` leaves it out for that household.
- `copy_codes`: a residential server copy learns every flat's code once and
  the log line names the building; a second copy says they were already
  yours; an office copy is unchanged.
- Screen test: a player with a resident's keycard walks through that flat's
  door and the log says the keycard opened it; without it the door says
  locked; with the code learned instead, the log says the code.
- Deck test: the journal rows show the header, the three entry kinds with
  their marks, and the detail card for a selected code.
- The ignored city fuzz runs stay green (placement with `Homes` on three
  cities).

## Out of scope but shaped for

- A lifetime: `Code` gets `until: Option<u64>` and `opens` checks it.
- Keypads: `How::Code` becomes the keypad's success path.
- Terminal tiers: a `Credential` enum over cards, codes and logins with a
  scope, built on top of `Journal` and `keycard_id`.
