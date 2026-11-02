# Doctor's in by ten: the shard, Seren's word and Odile Vance

The second slice of the morning after. When Mezz's mail has come, Seren breaks into
your holo set with one line and mails the rest: the doctor is in by ten, the shutter is
down, here is the code. The package is yours to open now; what the shard inside tells
you depends on who you are. Across town the clinic's front is shut for the day, but the
code lifts the shutter, and in the dark one light burns over the chair: Odile Vance,
who takes Castellan's strays, gives you one piece of chrome on his tab and a question
to carry back to him. You leave knowing whose ledger you hold: a fixer called Coz.

Every piece is built as a general thing the story drives from data: conditions on who
you are, readings of an item, a held doc, a tray on someone's tab.

Mockup: `docs/superpowers/specs/2026-11-02-doctor-in-by-ten-mock/` (`slice2_mock.diff`,
the throwaway shot test and its two throwaway patches; the pictures `s2_*.png`, in
story order: `seren_set`, `track`, `read_rig`, `read_bench`, `odile`, `tray`,
`chair_tab`). The clinic pictures are at night (the test fixture's hour); in the game it
is morning, shut and dim.

## What Jeffrey decided

- **The pitch, "Doctor's in by ten"** ("Slice 2 is great!", 2026-11-02), and the mocks
  ("Yes, build it"):
  1. Seren's word comes through your holo set, glitching the channel's label for her
     line, *"Doctor's in by ten. Eat first."*; her mail carries the way in.
  2. The shard is read by build: NETRUNNER at the rig (its ICE already open, her tag
     *"too slow. —S"*), WRENCH at the bench (the fixer's mark burned into the
     contacts), STREET knows the tape; anyone else finds it locked. The owner is the
     fixer **Coz** (from the name bank).
  3. Clinics keep night hours, so at ten the shutter is down; Seren's code gets you in.
  4. **Odile Vance**: *"He sends me strays."* She notices the jacket the news
     described, names Coz to whoever couldn't read the shard, offers three pieces fit
     to your build of which one goes in on Castellan's tab (`fee TAB · CASTELLAN`),
     and sends you off with *"Three sat in that chair on his money before you. Ask him
     where they are."*
  5. The hook: "Sell the ledger back to Coz".
- From the pitch, as my calls he saw and did not change: late after ten is soft (a line,
  not a fail); a Look in her surgery finds three scratched tags.

## Calls I made

- **A new mission, `doctor`** ("Doctor's in by ten", from Castellan), started by
  `MissionDone(morning_after)`; `morning_after` is untouched (§1).
- **The front door, not a back door.** Not every clinic has a second street door, so
  Seren's code opens the shutter at any of the clinic's street doors (§5). The pitch
  said "back door"; the mail says "Shutter's down till dark."
- **The doc is held, not a visitor.** Odile is the sim's ripperdoc of the clinic nearest
  your home, cast and renamed like Marlo. The story holds her at work from Seren's word
  until you leave after your piece (§4); a held doc's clinic is shut to the street but
  works for whoever gets in.
- **Who you are is one condition type, `Need`** (§2), used by scene lines, readings and
  the tray. A skill stands for the build: NETRUNNER is `hacking`, WRENCH `electronics`,
  STREET `streetwise` (each category's free skill).
- **Readings are a book section keyed by item** (§3), so any item can be read later:
  at the rig, at the bench (from the item in your bag, with a bench), or in its Look
  text. Rig and bench readings draw the HOLO card of the mock; Look readings replace
  the item's description.
- **The package opens** with a new item field and action, offered only once the story
  unseals it (§6). Seren's word unseals it: Castellan told you what's inside.
- **The jacket the camera saw is kept** when you walk away from the body (§7). SOUGHT
  shows that jacket from then on, not whatever you wear when you watch, and Odile
  compares it with what you wear.
- **The tray is a choice window** before you sit (the chair's FITS list shows one body
  site at a time, so three pieces on three sites can't be picked there). The piece you
  take goes in your bag; at her chair its whole fee (in and any out) is on the tab, once.
- **Tray sets per build** are data, three pieces each, picked to suit the category
  (§8); every piece fits a site on a body with no chrome.
- **The clinic after hours looks it**: while a doc is held, their chair's floor is drawn
  dim with a cold pool around the chair (§5). Cosmetic, like the flat's `Lights`.
- **The slice ends on an open objective**, "Sell the ledger back to Coz", in a mission of
  its own (`ledger`) waiting on talking to him; talking logs his line and closes it. The
  next slice replaces that last step.
- **Casting puts nearest-picks first** (§9): a new fixer role must not take Mezz's
  person, and today roles are cast in alphabetical id order.

## Builder rulings

A fresh builder read this spec against the code. Its four questions, settled as my
calls:

- **A city with one fixer.** Fixers come only from NeonStrip and NightMarket districts,
  so a small city can have just Mezz. The hook is its own mission, `ledger`, and a role's
  name fills from the book when nobody plays it (§9): `doctor` always plays (given a
  clinic), `ledger` stays `Uncast` there.
- **Odile's scene starts beside her**, not at the street door: `At(By(role, within: 4))`
  (§4). Clinic surgeries are a room or a floor away from the front door.
- **Readings wait for the story.** A new need, `Unsealed(item)`; the package's STREET
  reading needs it, so Mezz's "don't open it" description stays until Seren's word. The
  shard exists only after opening. The tape is black, as the package's description says
  (the pitch said blue).
- **An unknown jacket says nothing**: `Changed` and `Unchanged` (§2, §7), both false when
  the jacket isn't known, read from the last news item's body even once it is stale.

## Goals

- The morning has somewhere to go: a place, a time, a person, a reward.
- Every build learns the shard's owner, each through its own system.
- The city stays consistent: a shut clinic is shut, a camera remembers a jacket.

## Non-goals

- Selling the ledger, meeting Coz, Castellan answering for the three (next slices).
- Fail states: never going, refusing a piece, selling the package. The objective just
  waits.
- Real light (the light grid, stealth exposure) in the clinic; sound.
- A city with no clinic (about 1 in 200 Medium cities): Odile isn't cast and the step
  waits. Noted, not handled.

## Anti-goals

- Writing the slice in Rust: no "Odile", "Coz", "shard" or "Castellan" in code. Rust
  knows needs, readings, holds, trays and tabs.
- A build seeing another build's reading, or a reading telling a NETRUNNER they aren't
  one.
- Blocking screens nobody chose: the tray is the only window the story opens, after a
  scene the player walked into.

## §1 The mission

`flavor/missions.ron`, a new mission after `morning_after`:

```ron
Mission(id: "doctor", title: "Doctor's in by ten", from: "castellan", steps: [
    Step(id: "word", when: MissionDone(mission: "morning_after", minutes: 0), then: [
        Hold("odile"),
        Unseal("taped_package"),
        Holo(from: "seren", text: "Doctor's in by ten. Eat first."),
        Mail(from: "seren", subject: "ten", code: Some(Clinic("odile")), body: [
            "Doctor's in by ten. Eat first.",
            "Shutter's down till dark. Code's {code}. She knows you're coming.",
            "The package is yours now. Open it.",
            "— S",
        ]),
        Objective("See the doctor · {odile.place} · {odile.district} · by ten"),
    ]),
    Step(id: "in", when: At(By(role: "odile", within: 4)), then: [Scene("odile")]),
    Step(id: "tray", when: Ended("in"), then: [
        Tray(from: "odile", tab: "castellan", title: "her tray",
            line: "One piece, on his tab. The rest go back in the drawer.", sets: [ /* §8 */ ]),
        Objective("Sit in {odile}'s chair · your piece is on {castellan}'s tab"),
    ]),
    Step(id: "out", when: Chromed("odile"), then: [Scene("odile_out")]),
    Step(id: "free", when: Ended("out"), then: [
        Release("odile"),
        Log("`Doctor's in by ten` · on {castellan}'s tab"),
    ]),
]),
// The hook, its own mission so a city with one fixer (Mezz) still plays the doctor.
Mission(id: "ledger", title: "The ledger", from: "castellan", steps: [
    Step(id: "go", when: MissionDone(mission: "doctor", minutes: 0), then: [
        Objective("Sell the ledger back to {coz} · {coz.place} · {coz.district}"),
    ]),
    Step(id: "sell", when: Talk("coz"), then: [
        Log("`{coz}` · \"The ledger? Not here. I'll find you.\""),
    ]),
]),
```

Cast, beside the others:

```ron
"odile": Role(name: "Odile Vance", title: "ripperdoc", pick: Nearest("ripperdoc"),
    face: None, tint: Some((160, 220, 255))),
"coz": Role(name: "Coz", title: "fixer",
    pick: Worker(professions: ["fixer"], at: [], range: (40, 200)), face: None),
```

Scenes, both `Bubbles`, pace as `waking`:

```ron
"odile": Scene(title: "Doctor's in by ten", look: Bubbles, beats: [
    Stage("One light on, over the chair."),
    Say(who: "odile", text: "He sends me strays."),
    Say(who: "odile", need: Some(Late(600)), text: "Ten, he said. He said you'd be late, too."),
    Say(who: "odile", need: Some(Changed), text: "New jacket. Good. The last one didn't think of that."),
    Say(who: "odile", need: Some(Unchanged), text: "Lose that jacket before you go. It's on every screen in {marlo.district}."),
    Say(who: "odile", need: Some(All([Not(Read("ledger_shard")), Carries("taped_package")])),
        text: "That tape's {coz}'s. You're walking a fixer's ledger round in daylight."),
    Say(who: "odile", need: Some(All([Not(Read("ledger_shard")), Carries("ledger_shard")])),
        text: "That shard's {coz}'s. You're walking a fixer's ledger round in daylight."),
    Say(who: "odile", text: "{castellan} pays for one piece. Pick."),
]),
"odile_out": Scene(title: "Doctor's in by ten", look: Bubbles, beats: [
    Say(who: "odile", text: "Three sat in that chair on his money before you."),
    Say(who: "odile", hold: Some(4.0), text: "Ask him where they are."),
    Note(at: Chair("odile"), text: "Three clinic tags hang off the lamp arm. The names are scratched off."),
]),
```

`Note` is a beat here and an effect too (§10). Lines are the in-world voice: Odile's
and Seren's own words, no narrator.

## §2 Needs

`missions::content::Need`, deserialised from the book:

| Need | Holds when |
|---|---|
| `Skill(id)` | `st.character.has_skill(id)` |
| `Read(item)` | a telling reading (§3) of the item has been shown this story |
| `Carries(item)` | `st.character.count(item) > 0` |
| `Unsealed(item)` | the story has unsealed the item (§6) |
| `Changed` | the jacket kept at the walk-away (§7) is known and isn't what your Torso wears now |
| `Unchanged` | that jacket is known and is what your Torso wears now |
| `Late(minute)` | now is past the first time the clock showed `minute` after the mission began |
| `Not(Need)`, `All([Need])` | as named |

One function, `missions::holds(st, mission, &Need) -> bool`. `Late` needs the mission's
start tick: the run records the tick its first step fired.

`Say` gains `need: Option<Need>` (serde default `None`); a beat whose need fails is
skipped at once (no time passes). No other beat takes a need.

## §3 Readings

A book section, `readings: { "<item id>": [Reading, …] }`; the first reading whose
`need` holds at a place is the one used there:

```ron
Reading(at: Rig | Bench | Look, need: Need, verb: "Read", head: "shard · read at the rig",
    rows: [Row], close: "pull it", tells: true),
```

- `Look` readings use only `text: String` (a description); `Rig`/`Bench` use `verb`,
  `head`, `rows`, `close`. Unused fields default.
- Rows: `Field(label, text)` (label DIM in a 8-column gutter, text TEXT, backticked
  spans AMBER bold), `Hot(text)` (MAGENTA), `Tag(text)` (MAGENTA bold, led by `▚▞ `),
  `Art(text)` (CYAN, backticked spans AMBER bold), `You(text)` (`you` DIM, text TEXT),
  `Ledger(shown, total)` (`shown` seeded rows `MM.DD  ¤amount  ▓▓▓  place`, the bars
  FAINT, a place every other row from the city's districts and landmarks, then
  `…N more` FAINT), `Blank`. Every text takes the story's slots.
- **The card** (`missions/reading.rs`, a `Screen`): the HOLO bracket frame of the feed,
  `HOLO ▸ <head>` (or `BENCH ▸` at the bench), the signal and the clock on the top row as
  the feed has them, the rows from row 3, `space <close>` two rows from the bottom.
  Sized to its rows, at most 66 wide, centred; space or esc closes. Showing a `tells`
  reading records `Read(item)`.
- **At the rig:** the rig's subject (`interact.rs`) gains `r Read: <item name>` for each
  carried item with a Rig reading whose need holds; it opens the card.
- **At the bench:** the pack popup (`deck.rs item_popup`) gains a row with the reading's
  `verb` for a carried item with a Bench reading whose need holds, while
  `has_workbench`; the card opens over the deck.
- **Look:** one accessor `missions::item_desc(st, id) -> String` returns the first Look
  reading whose need holds, else `item.desc`; the pack card and the map's Look use it.

The shard's readings (`flavor/missions.ron`):

```ron
readings: {
    "ledger_shard": [
        Reading(at: Rig, need: Skill("hacking"), verb: "Read", head: "shard · read at the rig",
            close: "pull it", tells: true, rows: [
            Field("LEDGER", "214 entries · 2068–2071"),
            Field("owner", "`{coz}` · fixer"),
            Hot("ICE     open · cracked 02:13 last night"),
            Blank,
            Ledger(4, 214),
            Blank,
            Tag("too slow. —S"),
        ]),
        Reading(at: Bench, need: Skill("electronics"), verb: "Open the casing",
            head: "shard · casing open", close: "close the casing", tells: true, rows: [
            Art("┌─────────────────────┐   housing  matte, no maker"),
            Art("│ ░░░░░░░░░░░░░░░░░░░ │   data     locked · ICE 3"),
            Art("└┬┬┬┬┬┬┬┬┬┬┬┬┬┬┬┬┬┬┬┬┬┘   contacts gold, worn bright"),
            Art("  ╰── `C O Z` ──╯         mark     `{coz}` · burned in"),
            Blank,
            You("{coz}. Fixers sign their shards."),
        ]),
        Reading(at: Look, need: Skill("streetwise"),
            text: "Black fixer's tape still round the casing, wound three times. {coz} wraps his like that."),
    ],
    "taped_package": [
        Reading(at: Look, need: All([Skill("streetwise"), Unsealed("taped_package")]),
            text: "Black fixer's tape, wound three times. {coz} wraps his like that."),
    ],
},
```

(`C O Z` in the art is the role's name spaced: the art row takes a slot
`{coz.spaced}`, the name's letters upper-cased with spaces, so the art follows the
cast.) Anyone else reads the shard's own description: *"A ledger shard. Locked.
Somebody will know whose."*

## §4 Holding a doc

- **Sim** (`crates/sim/src/world.rs`): `World` gains `held: BTreeSet<PersonId>` and
  `keep_at_work(id)`, `release(id)`, `is_held(id)`. Keeping sets the person's goal to
  Work, puts them inside their work building (or at their spot), drops a pending trip
  and skips them in the per-minute departure loop; releasing lets the next minute send
  them where their schedule says. A world's held set starts empty; `advance` keeps it.
- **Effects:** `Hold(role)` keeps the role's person at work. `Release(role)` lets them
  go the first minute you are not inside their work building (a pending release in
  `Missions`), so she never walks out past you.
- **Where:** `At(By { role, within })` holds while you and the role's person are on the
  same place (the street, or one floor of a building) and you are within `within`
  tiles of them (Manhattan): on the street where the sim has them, indoors the tile the
  floor draws them on. So the scene starts beside her in the surgery, on whatever floor
  the layout puts the chair (a Laundromat's is the cellar). Tracking resolves it to a
  `Spot` on her floor at her tile with radius `within` (indoor spots already route, as
  jobs do), or to `Target::Person` while she is on the street.

## §5 A held clinic

- `clinic::is_open(st, b)` is false while its doc is held: shut to the street though
  the doc is in. `doc_at_work`, the chair and `operating` are unchanged, so the chair
  works for whoever is inside.
- **The street code.** A clinic's street doors share one code,
  `door_code(seed, b, 0, STREET_DOOR)` where `STREET_DOOR` is a room number no room
  uses (a `credentials` const). `interiors::enter` on a shut clinic lets you in when
  the journal holds that code, with the status line `you key in 4417` as a locked
  inner door does; otherwise it refuses as today. The shut door's window line gains
  ` · code 4417` when you have it.
- **The journal** words a street code `door code 4417 · Miracorp Tattoo, street door ·
  <source>`: the code's entry carries what it opens (flat, back room or street door),
  so non-flat codes stop reading "flat 0".
- **The look:** while a clinic's doc is held, the floor with its chair is drawn at 0.35
  of its colour except within 3 tiles of the chair, which keeps full colour shifted cold
  (blue up, red down, a fifth each). Drawn where the flat's `Lights` is drawn.

## §6 Opening an item

- `game::Item` gains `opens: Vec<(ItemId, u32)>` (serde default empty); `Action::Open(id)`
  takes one and gives each output; the outcome message is
  `You open the <name>: <output names>.` Refused for an item with no `opens`.
- The app offers **Open** in the pack popup (`item_popup`, and `copy_popup` for a
  per-copy item) only when the item opens and `Missions.unsealed` holds its id, and
  `actions::perform` refuses it otherwise. `Effect::Unseal(item)` adds the id;
  `Missions::default` empties it.
- `flavor/content.ron`: `taped_package` gains `opens: [("ledger_shard", 1)]`; a new
  `ledger_shard` (`Component`, weight 0.1, value 0, desc *"A ledger shard. Locked.
  Somebody will know whose."*).
- `Book::gives` counts what a given item opens into, so a story restart takes the shard
  back too.

## §7 The jacket the camera saw

`Left` gains `worn: Option<ItemId>`, the Torso item at the walk-away. `news::sought`
names that jacket when a `Left` is known (else today's live read). The jacket is the
`worn` of the body of the news item put out last (`Missions.news`, fresh or stale), so
the check outlives the news. `Changed` holds when it is `Some` and differs from your
Torso now; `Unchanged` when it is `Some` and the same; neither when no `Left`, no news
item or no jacket was worn then, and Odile says neither jacket line.

## §8 The tray and the tab

- `Effect::Tray { from, tab, title, line, sets: [TraySet(need: Need, items: [String; 3])] }`:
  the first set whose need holds (the last set's need is `All([])`, always true) is put
  up as a choice window titled `<from's name> · <title>` (`Odile Vance · her tray`),
  its body the `line` (*"One piece, on his tab. The rest go back in the drawer."*), one
  row per piece: `Take: <name> · <site> · <grants, or nothing>` (the mock's rows).
  Space takes the piece into your bag and opens the tab; esc closes the window and
  leaves the tray pending: talking to the `from` role opens it again.
- **The tab:** `Missions.tab = Some(Tab { doc: PersonId, item: ItemId, payer: String,
  spent: bool })`. In the chair of that doc, installing that item costs nothing: the
  fee line reads `fee TAB · CASTELLAN · ¤N on the tab` (payer's name upper-cased, N the
  fee that would have been charged, AMBER bold as the mock; the mock's "on him" would
  need a pronoun the code doesn't have). The install marks it
  spent; any other piece pays as usual.
- **`Chromed(role)`** fires once the tab with that role's person as doc is spent and
  you are out of the chair.

The sets (`items` in this order; ids from `flavor/content.ron`):

| need | pieces |
|---|---|
| `Skill("hacking")` | neural_port, feedback_fuse, limbic_damper |
| `Skill("electronics")` | grip_arm, capacitor_ribs, damper_cochlea |
| `Skill("handguns")` | target_lens, adrenal_pump, hardback_brace |
| `Skill("blades")` | piston_arm, subdermal_mesh, adrenal_pump |
| `Skill("stealth")` | nightglass_optics, damper_cochlea, smart_ink |
| `Skill("streetwise")` | secondhand_eyes, bulldog_jaw, filter_liver |
| `Skill("medtech")` | surgeon_arm, filter_liver, rebreather |
| `Skill("athletics")` | adrenal_pump, hardback_brace, subdermal_mesh |
| `All([])` | subdermal_mesh, target_lens, neural_port |

A test asserts every piece of every set installs on a body with no chrome.

## §9 Casting

- `Pick::Nearest(profession)`: the person of that profession whose work tile is nearest
  the home door (spot or building), never the dead or the taken.
- `cast` casts every `NearestFixer` and `Nearest` role before any `Worker` role, each
  group in id order, so adding `coz` (a fixer `Worker`) never takes Mezz's person and
  the current roles cast as they do today.
- **Spaced names:** a slot `{role.spaced}` gives the role's name upper-cased with a
  space between letters (`C O Z`).
- **A name needs no person.** `{role}`, `{role.spaced}` and `{role.title}` fill from the
  book when the role has nobody (as a visitor's do), so a line can name someone the city
  can't cast. `needs_cast` counts only the roles a mission needs a person for: those in
  its triggers, its marks, its effects that act on a person (`Dead`, `Hold`, `Release`,
  `Tray`'s `from`, a mail's `Clinic` code) and its `.place`, `.district` and `.hours`
  slots, in its steps and in the scenes it plays. So `doctor` plays in a city whose only
  fixer is Mezz, and `ledger` (which tracks and talks to Coz) is the one left `Uncast`.

## §10 The rest of the engine

- **Marks:** `Chair(role)` resolves to the chair on the role's work building, on its
  floor. `spot(who)` for a cast person on your floor is the tile the floor draws them
  on, so a doc's `Say` anchors beside her.
- **`Holo { from, text }`:** while you are in your flat and the set is there, one aside
  from the set: label `<channel minus its last three letters>▚▞▚ <first letter of
  from's name>`, the role's tint, the filled text. Otherwise nothing.
- **`Mail.code: Option<CodeOf>`**, `CodeOf::Clinic(role)`: the street code of the
  role's work building is learned into the journal when the mail lands
  (`Source::Mail`), and `{code}` in the body is filled with it.
- **`Note { at: Mark, text }`** (beat and effect): a story Look line for that tile,
  added under its Look card while the story lasts (`Missions.notes`, reset with it).
- **Console:** `story` needs nothing new (`story fire doctor` plays the slice).

## Testing

- Unit tests beside each piece: needs, readings picked by need and place, the card's
  rows, opening gated by unseal, the hold through `advance`, a held clinic shut with the
  chair working, entry with and without the code, the journal wording, the tab's fee
  and spend, `Chromed`, casting order (Mezz unchanged on the seeds the story tests use),
  the jacket kept at the walk-away.
- A story test plays the slice end to end on a seed: word → code learned → in → scene
  lines by need → tray → install on the tab → out → release after you leave → Coz.
- Shots (`doctor_shots`, ignored): Seren on the set, the rig card, the bench card over
  the deck, Odile's bubble in the dim clinic at 10:00, the tray, the chair with the tab;
  both sizes.

## Files

- `flavor/missions.ron` (§1, §3, §8), `flavor/content.ron` (§6).
- `crates/sim/src/world.rs` (§4).
- `crates/game/src/{model,action}.rs` (§6).
- `crates/strata-zero/src/missions/`: `content.rs` (needs, readings, effects, triggers,
  marks, picks), `mod.rs` (holds, tab, unsealed, notes, pending release, `Late`'s start
  tick), `effects.rs`, `cast.rs`, `marks.rs`, `scene.rs` (`Say.need`), `news.rs` (§7),
  new `reading.rs`.
- `crates/strata-zero/src/{clinic,interiors,credentials,interact,look,actions}.rs`,
  `screens/{deck,popup}.rs`, `screens/sheet/ledger.rs` (the tab), `track/mod.rs`,
  `screens/shots.rs`.
