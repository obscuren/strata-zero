# Clear objectives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every step of the morning after is an objective you can see and follow, a new
mail can't be missed, and the JOBS list is titles only.

**Architecture:** Four small story-engine additions (`Trigger::Holds`, a mail step that
ends when read, `Effect::ObjectiveFit`, `Effect::Told`, plus a Look reading that tells),
the book rewritten on them, the tracker taught three new targets, the JOBS list's walk
lines dropped, a mail mark on the map screen (a pill, then a status-line mark), and the
log typing its newest line out on the frame clock.

**Tech Stack:** Rust workspace (crates `game`, `sim`, `strata-zero`), ratatui cells,
RON content in `flavor/*.ron` compiled into the binary.

**Spec:** `docs/superpowers/specs/2026-11-03-clear-objectives-design.md`

## Global Constraints

- Build only into `--target-dir target/smoke`; never the default `target/`.
- The five checks, both front ends, must pass at the end of every task:
  `cargo fmt --all --check`;
  `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`;
  `cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings`;
  `cargo test --workspace --target-dir target/smoke`;
  `cargo test --workspace --features strata-zero/window --target-dir target/smoke`.
- Draw through ratatui only; exact `Color::Rgb` colours; `▣` is already in the game's
  glyph set (no new symbols).
- No authored names (Mezz, Seren, Coz, Odile…) in non-test Rust; they live in
  `flavor/*.ron`.
- Copy is the game's: reuse its words ("on your rig", "Inspect", "hand it in").
- Match the surrounding code's comment density and idiom; doc comments in the files'
  own voice.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Never bare
  `git stash`.
- `AppState::new()` has dev mode off; a test needing dev calls `st.unlock_dev()`.

## Review Focus

1. A mail read through the terminal (`mail n`) rather than the JOBS card must end its
   story step too: both go through `credentials::mail_read` (Task 3 test).
2. The newest log line must type out after a sleep, when the city's tick is hours ahead
   of the frame tick (Task 1 test).
3. A mail landing while the deck or the terminal is up must still put the pill up when
   the map is back, for its full 6 seconds (Task 6 test).
4. A player who reaches Odile before opening the package: the ledger's `whose` and `know`
   both fire as soon as the package is opened (Task 5 test).
5. Entering a city (a new game, a load) must not announce mails or leave a pill (Task 6
   test).

---

### Task 1: The log types its newest line out on the frame clock

**Files:**
- Modify: `crates/strata-zero/src/messages.rs` (`Entry`, `MessageLog::push`)
- Modify: `crates/strata-zero/src/app.rs:466-482` (`say`, `warn`, `story`)
- Modify: `crates/strata-zero/src/ui/sidebar.rs:426-454` (`draw_log`)
- Test: `crates/strata-zero/src/ui/sidebar.rs` tests (or `app.rs` tests, where log
  drawing is already tested)

**Interfaces:**
- Produces: `Entry.shown: u64` — the frame tick (`AppState.tick`) the entry arrived;
  `MessageLog::push(kind, text, tick, shown)`. `Entry.tick` stays the city's tick and
  still drives the clock stamp and `recent`.

- [ ] **Step 1: Write the failing test** `the_newest_line_types_out_after_a_sleep`: a
  state with a world whose tick is 43 200 ahead of `st.tick` (advance the world 480
  minutes without touching `st.tick`), `st.story("`Mezz` · mail · the news")`, then
  `st.tick += 30`; draw the sidebar's log; assert the drawn text contains
  `"Mezz · mail · the news"`.
- [ ] **Step 2: Run it; it fails** (only the typing cursor shows).
- [ ] **Step 3: Implement.** `shown` on `Entry`, set from `self.tick` by the three
  `AppState` loggers; `draw_log` reveals against `e.shown`.
- [ ] **Step 4: Run the test and the messages/sidebar tests; they pass.**
- [ ] **Step 5: Commit** `log: the newest line types out on the frame clock`.

### Task 2: The JOBS list keeps no walk lines

**Files:**
- Modify: `crates/strata-zero/src/screens/jobs_tab.rs` (`Entry::Next` and its builders
  `build`, `next_of`, its draw arm; the `Entry::Mission` draw arm)
- Test: `crates/strata-zero/src/screens/jobs_tab.rs` tests

**Interfaces:**
- Produces: `Entry` without `Next`. Nothing outside the tab used it.

- [ ] **Step 1: Update the list test**
  `the_list_is_the_story_then_each_fixer_with_their_jobs`: `e[2]` is no longer a walk
  line; assert no entry is a walk line (the variant is gone, so assert
  `e[2] == Entry::Gap` or the next header, whichever the list builds) and that the row
  after `Entry::Job(ids[0])` is not under it. Add
  `a_tracked_story_marks_its_row`: track the open story; draw the tab; the row with the
  mission's title has `⌖` two columns before the title.
- [ ] **Step 2: Run; the new test fails.**
- [ ] **Step 3: Implement.** Remove `Entry::Next`, its two pushes, `next_of` and its draw
  arm; the mission row's mark is `✓` when done, else `⌖` (CYAN) when
  `story_source(st, m)` is tracked.
- [ ] **Step 4: Run the jobs_tab tests and `cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored jobs_tab`
  with `SHOT_DIR` set; they pass and the shots show no walk line.**
- [ ] **Step 5: Commit** `jobs tab: titles only, the walk is the card's`.

### Task 3: A mail step ends when its mail is read

**Files:**
- Modify: `crates/strata-zero/src/credentials.rs` (`Mail.story`, `mail_read`)
- Modify: every `Mail { … }` literal (`credentials.rs`, `missions/effects.rs`,
  `missions/call.rs`, `helix/hosts.rs`, `jobs/mod.rs`, `jobs/generate.rs`, tests) to
  set `story`
- Modify: `crates/strata-zero/src/missions/effects.rs` (`Effect::Mail` sets
  `story: Some((mission, step))`)
- Modify: `crates/strata-zero/src/missions/content.rs` (`Effect::presents` includes
  `Mail`)
- Test: `crates/strata-zero/src/missions/mod.rs` tests

**Interfaces:**
- Produces: `Mail.story: Option<(usize, usize)>` (mission index, step index);
  `credentials::mail_read(st, i)` ends that step the first time an unread story mail is
  read.

- [ ] **Step 1: Write the failing tests** (a small book through the existing
  `doc_book`/`storied` helpers): steps `send` (NewGame → `Mail(from: "mezz", …)`) and
  `next` (`Ended("send")` → `Log("after")`).
  `a_story_mail_read_ends_its_step`: after `send`, `next` waits; `mail_read(st, i)`;
  `next` has run. `a_mail_read_at_the_terminal_ends_its_step`: same through
  `access::mail_open(st, i)`. `reading_it_again_ends_nothing_twice`: a second read runs
  nothing new. `ended_may_name_a_mail_step`: `Book::validate` accepts the book (today it
  refuses `NotAPresentation`).
- [ ] **Step 2: Run; they fail.**
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the missions, credentials and helix tests; they pass.**
- [ ] **Step 5: Commit** `story: a mail step ends when its mail is read`.

### Task 4: `Holds`, `ObjectiveFit`, `Told`, and a Look that tells

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (`Trigger::Holds(Need)`,
  `Effect::ObjectiveFit(Vec<Fit>)`, `pub struct Fit { need: Need, text: String }`,
  `Effect::Told(String)`; parsing; `validate`: the need checked by `check_need`, its
  items known; `ObjectiveFit` non-empty and its last need `All([])`; `Told`'s item
  known; `Fit` text slots checked as `Objective`'s are)
- Modify: `crates/strata-zero/src/missions/mod.rs` (`Trigger::fires` for `Holds`:
  `holds(st, Some(m), need)`)
- Modify: `crates/strata-zero/src/missions/effects.rs` (`ObjectiveFit` pushes the first
  fit's filled text; `Told(item)` inserts into `st.missions.read`)
- Modify: `crates/strata-zero/src/screens/deck.rs` (the GEAR & INVENTORY tab: when the
  selected pack row is an item whose `Look` reading holds and `tells`, its card showing
  that text, record `Need::Read` — in `update`, as the selection lands on it; the pack's
  card is drawn by `Deck::item_card` → `missions::item_desc`) and
  `crates/strata-zero/src/missions/reading.rs` (the helper that does it)
- Modify: `crates/strata-zero/src/missions/content.rs` text scans: an `ObjectiveFit`'s
  texts count for slots and for the roles a mission needs exactly as `Objective`'s text
  does (`Effect::Objective(s) | Effect::Log(s) => vec![s]`), so a fit placing `{odile}`
  makes the mission need her cast
- Test: `missions/content.rs`, `missions/mod.rs`, `missions/reading.rs` tests

**Interfaces:**
- Produces: `Trigger::Holds(Need)`; `Effect::ObjectiveFit(Vec<Fit>)`; `Fit`;
  `Effect::Told(String)`; `pub fn missions::reading::look_tells(st: &mut AppState, item: &ItemId)`.

- [ ] **Step 1: Write the failing tests.**
  `holds_fires_once_the_need_holds` (a step on `Holds(Carries("x"))` waits until the
  item is given, then fires with no event); `holds_late_uses_its_mission`
  (`Holds(Late(m))` follows the mission's begin as `Say` does);
  `objective_fit_takes_the_first_that_holds` (hacking → the first text; none of the
  skills → the `All([])` text); `objective_fit_must_end_on_all` (validation refuses a
  last need other than `All([])`, and an empty list); `told_records_read`;
  `a_telling_look_is_read_when_its_card_shows` (a Look reading with `tells: true`: the
  deck's GEAR & INVENTORY tab with the item's row selected records `Read`; one without
  `tells`, or whose need doesn't hold, records nothing);
  `a_fit_placing_a_role_needs_it_cast` (a mission whose `ObjectiveFit` text has
  `{doc.place}` is Uncast when `doc` has nobody).
- [ ] **Step 2: Run; they fail.**
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the missions tests; they pass.**
- [ ] **Step 5: Commit** `story: Holds, ObjectiveFit, Told, a Look that tells`.

### Task 5: The book: the morning after as a chain

**Files:**
- Modify: `flavor/missions.ron` (spec §1: `morning_after` steps `saw`, `word`, `open`;
  `doctor.word` on `Holds(Unsealed("taped_package"))` without its `Holo` and `Unseal`;
  `doctor.tray` gains `Told("ledger_shard")`; `ledger` steps `whose`, `know`, `sell`;
  the streetwise Look of `ledger_shard` gains `tells: true`; the header comment, spec §8)
- Modify: the shipped-book story tests and helpers in
  `crates/strata-zero/src/missions/mod.rs` (`mezz_saw_the_news`, `to_the_word`, the
  doctor and ledger tests) and `screens/shots.rs` helpers (`seren_has_written`) to the
  new chain
- Test: `crates/strata-zero/src/missions/mod.rs` tests

**Interfaces:**
- Consumes: Tasks 3 and 4.
- Produces: `missions::tests::to_the_word(st)` now ends with Mezz's mail read and
  Seren's word in (the package unsealed, the doctor open); a new
  `missions::tests::open_the_package(st)` (Unsealed → `Action::Open` through
  `actions`).

Exact copy (spec §1):
`"Read {mezz}'s mail · on your rig"`, `"Open the package · it's in your pack"`,
`"`The morning after` · a ledger shard, in your pack"`,
`"Find out whose shard this is · read it at your rig"` (hacking),
`"Find out whose shard this is · open the casing at your bench"` (electronics),
`"Find out whose shard this is · look it over in your pack"` (streetwise),
`"Find out whose shard this is · ask the doctor at {odile.place}"` (`All([])`),
`"Bring it to {coz} · {coz.place} · {coz.district}"`;
Odile's third naming line in her `odile` scene, after the two that exist, with need
`All([Not(Read("ledger_shard")), Not(Carries("taped_package")), Not(Carries("ledger_shard"))])`:
`"He told me what you're carrying. Black tape, three turns? That's {coz}'s."`

- [ ] **Step 1: Write the failing shipped-book tests.**
  `the_morning_after_asks_for_mezzs_mail`: after the news is watched, the morning
  after's objective is "Read Mezz's mail · on your rig", the doctor waits, the package
  is sealed. `reading_mezzs_mail_lets_seren_in`: `mail_read` of Mezz's mail → the
  package is unsealed, Seren's holo line is up, Seren's mail has landed with the code,
  the doctor's objective is "See the doctor · …", the morning after's is "Open the
  package · it's in your pack". `opening_the_package_opens_the_ledger`: open it → the
  morning after is done; the ledger's objective for a hacker ends "read it at your rig",
  for nobody's skills "ask the doctor". `reading_the_shard_sends_you_to_coz`: the rig
  reading shown → "Bring it to Coz · …". `odile_tells_who_has_not_read`: through her
  scene without reading → the ledger at "Bring it to Coz"; and the Review Focus 4 case
  (her scene first, the package opened after → straight to "Bring it to Coz").
  `odile_names_him_to_the_empty_handed`: in her chair with neither item, her third line
  is said. `no_clinic_leaves_the_ledger_uncast`: a city whose `odile` has nobody leaves
  the ledger Uncast (the morning after still plays).
- [ ] **Step 2: Run; they fail.**
- [ ] **Step 3: Rewrite the book and the helpers** to spec §1; update the existing story
  tests that walked the old chain (they assert the new order; none is deleted unless its
  behaviour is gone, and then say so in the report).
- [ ] **Step 4: Run all missions tests and the ignored `doctor_shots` with `SHOT_DIR`;
  they pass.**
- [ ] **Step 5: Commit** `story: the morning after as a chain, the ledger with the package`.

### Task 6: Tracking the new steps

**Files:**
- Modify: `crates/strata-zero/src/track/mod.rs` (`resolve_story`)
- Test: `crates/strata-zero/src/track/mod.rs` tests

**Interfaces:**
- Consumes: Tasks 3–5 (the shipped book's steps).
- Produces: `resolve_story` targets: `Ended(step)` whose step has `Effect::News` → the
  holo set (`missions::news::set_tile`), radius 1; `Ended(step)` whose step has
  `Effect::Mail` → the home rig tile (`st.home` → `rig.place()`, `rig.home.rig`), radius
  1; `Holds(Need::Read(item))` when `missions::reading(st, item, ReadAt::Rig)` is `Some`
  → the home rig, radius 1; any other `Holds` → `Nowhere`.

- [ ] **Step 1: Write the failing tests** on the shipped chain: tracking the morning
  after while it waits on the news targets the set; while it waits on Mezz's mail, the
  rig; the ledger for a hacker, the rig; for nobody's skills, nowhere ("nothing to
  track" as today's no-place steps answer).
- [ ] **Step 2: Run; they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the track tests; they pass.**
- [ ] **Step 5: Commit** `track: the set, the rig, for the morning's steps`.

### Task 7: The mail mark: a pill, then the status line

**Files:**
- Modify: `crates/strata-zero/src/app.rs` (`AppState.mail_seen: usize`,
  `AppState.mail_pill: Option<MailPill>`; both reset in `enter_city_at`)
- Create: `crates/strata-zero/src/mail_mark.rs` (or a section of `credentials.rs`):
  `pub fn notice(st: &mut AppState)` (new mails since `mail_seen`: log each, put the
  pill up for the newest), `pub fn pill_line(st, width) -> Option<Line<'static>>`,
  `pub fn unread(st) -> usize`
- Modify: `crates/strata-zero/src/screens/explore.rs` (call `notice` each map frame
  after the clock step; draw the pill on the map's top edge, right, when no film scene
  plays; start its clock on its first drawn frame; pass the unread count to the status
  line when no pill is up)
- Modify: `crates/strata-zero/src/ui/hud.rs` (`Status.mail: usize`; `status_line` adds
  ` :: ▣ NEW MAIL` / ` :: ▣ N NEW MAIL` after the credits, MAGENTA, bold, when `mail > 0`)
- Test: the new module's tests, `hud.rs` tests, `explore.rs` tests

**Interfaces:**
- Produces: `pub struct MailPill { pub mail: usize, pub shown: Option<u64> }` (the mail's
  index, the frame its first draw happened); `MAIL_PILL_TICKS: u64 = 180`.

Exact look (spec §5): pill `"▣ MAIL "` dark text (`hud::BADGE_FG`) on MAGENTA, bold,
then `" {FROM} · {subject} "` (from upper-cased) light on `Rgb(120, 20, 90)`, then
`"  +N"` inside the second span when N other mails are unread; placed on the map's
first row, ending 8 cells in from the map's right edge, cut to fit. Status: `" :: "` dim,
then `"▣ NEW MAIL"` / `"▣ {n} NEW MAIL"` MAGENTA bold.

- [ ] **Step 1: Write the failing tests.** `a_landed_mail_logs_and_puts_the_pill_up`;
  `the_pill_stays_six_seconds_of_map_frames_then_the_status_line_marks`;
  `a_mail_behind_the_deck_shows_when_the_map_is_back` (Review Focus 3: push a mail,
  no map frames for 600 ticks, then map frames: the pill is up for 180 of them);
  `entering_a_city_announces_nothing` (Review Focus 5); `the_mark_goes_when_all_is_read`;
  `status_line_marks_unread_mail` (hud: 1 → "▣ NEW MAIL", 2 → "▣ 2 NEW MAIL", 0 →
  none; cut at 100×30's left band without breaking); `the_pill_fits_a_100x30_map`.
- [ ] **Step 2: Run; they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the new tests and `every_drawn_character_has_a_glyph`; they pass.**
- [ ] **Step 5: Commit** `mail: a pill as it lands, the status line till it's read`.

### Task 8: Shots

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (a new ignored `objectives_shots`)

**Interfaces:**
- Consumes: Tasks 2–7.

- [ ] **Step 1: Write `objectives_shots`** (spec §7), driven through the real chain with
  the Task 5 helpers, at 120×40 and 100×30: `objectives_jobs_morning` (JOBS tab, the
  morning after selected, waiting on "Open the package"), `objectives_jobs_ledger`
  (a hacker, the ledger selected), `objectives_mail_pill` (the map as Mezz's mail
  lands, the log line typed out), `objectives_mail_mark` (180 map frames on: the status
  line's mark). Assert in it what each shot is for (the objective text in the buffer,
  the pill's `▣ MAIL`, the status line's `▣ NEW MAIL`).
- [ ] **Step 2: Run it with `SHOT_DIR`** and look at every PNG; fix what reads wrong.
- [ ] **Step 3: Run the five checks.**
- [ ] **Step 4: Commit** `shots: clear objectives`.
