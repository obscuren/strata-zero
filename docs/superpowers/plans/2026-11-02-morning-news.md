# The Morning News Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The first slice of the morning after: the waking scene and daze, a news item the story puts out, the holo set that reads it in bubbles and opens the broadcast, and the city's subtle outlets (overheard, holo ads, screens).

**Architecture:** The news is a book item (`Book.news`) put out by `Effect::News`; the current one lives in `st.missions.news` and every outlet reads it. A per-frame `missions::news::tick` (called from the explore update) runs the set's reads, the outlets, the walk-away record and the step's ending. Bubbles that keep you in control are a new piece, `missions::asides`, drawn by the explore screen. The broadcast is a pushed `Screen` (`missions::broadcast`), the daze a pass over the map (`missions::daze`), the set a new `citygen::Furniture::Holo`.

**Tech Stack:** Rust, ratatui cells, crossterm keys, RON content (`flavor/*.ron`, compiled in), two front ends (terminal and `--features strata-zero/window`).

**Spec:** `docs/superpowers/specs/2026-11-02-morning-news-design.md` (mock: `docs/superpowers/specs/2026-11-02-morning-news-mock/news_mock.rs`, `broadcast_120x40.png`).

## Global Constraints

- Build and test into `--target-dir target/smoke`, never the default `target/`.
- Check both front ends (CLAUDE.md): `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`; the same with `--features strata-zero/window`; `cargo test --workspace --target-dir target/smoke`; the same with `--features strata-zero/window`.
- Draw through ratatui only; exact `Color::Rgb`; read crossterm `KeyEvent`s. `space` confirms, `esc` backs out, `enter` is not a window's key.
- No mission, step, role or news id, and no "Marlo", "courier", "package" in Rust outside tests: the book names them.
- New glyphs must pass `every_drawn_character_has_a_glyph` (`◫`, `●`, `▸`, `▀`, `╭╮╰╯│─`, `▾▴` are all in use already or in Fira Code).
- `citygen::Furniture::Holo` is appended after `Chair` (saved cities keep their bytes); `FORMAT_VERSION` stays.
- State is session-only: new fields go on `Missions` (derives `Default`, reset with the city, not saved).
- Clock ticks (`st.clock_tick()`, 30 a second, `call::TICKS_PER_SECOND`; 90 a game minute, `sim::TICKS_PER_MINUTE`) for asides and story timing, so they pause in menus; frame ticks (`st.tick`) for pure animation (the set's glyph, the crawl, the daze).
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Never bare `git stash`.

## Review Focus

- **A scene, a rest or a fight starting while asides are up**: nothing of the asides draws under a scene, asleep or in a fight, and a stale aside never pops back after (asides are cleared when any of those starts). Test in Task 5.
- **The player off the street** (inside a building's floor, a construct, the Net, your flat) when the outlets roll: overheard and holo ads do nothing and never panic; screens still work on a floor. Test in Task 6.
- **No home or no city** (a lot city, `story news` from the console before a city): putting out an item, the tick and the broadcast never panic; no Watch without a home. Test in Tasks 1 and 4.
- **The broadcast opened with the item gone** (stale or replaced while the screen is up): the screen pops on its next update and draws nothing. Test in Task 4.
- **The 100×30 frame**: the broadcast box, a 28-wide aside and the daze stay inside the screen and the map area; nothing indexes outside the buffer. Test in Tasks 4, 5, 7.

---

### Task 1: The news item in the book, its effects, and putting it out

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (types, `Effect` arms, `texts`/`named_roles`/`presents`, `Book.news`, `needs_cast`, validation, `BookError` variants)
- Create: `crates/strata-zero/src/missions/news.rs` (state, `put_out`, `channel`, `fresh`, `current`, `switch_off`)
- Create: `crates/strata-zero/src/missions/daze.rs` (the state and `start` only; drawing is Task 7)
- Modify: `crates/strata-zero/src/missions/mod.rs` (`pub mod news; pub mod daze;`, fields on `Missions`, module doc's list)
- Modify: `crates/strata-zero/src/missions/effects.rs` (`Effect::News`, `Effect::Daze`)
- Modify: `crates/strata-zero/src/screens/console.rs` (`story news <id>`, usage and completion)
- Modify: `flavor/names.ron` ("Odile" out)

**Interfaces:**
- Produces:
  - `content::News { about: String, headline: String, report: Vec<String>, reads: Vec<String>, crawl: Vec<String>, overheard: Vec<String>, sought: String, carrying: String, camera: String, still: Vec<String>, close: String, fresh: u64 }` (serde `Deserialize`, RON name `News`).
  - `Book.news: BTreeMap<String, News>` (`#[serde(default)]`).
  - `Effect::News(String)`, `Effect::Daze(f32)`.
  - `BookError::UnknownNews { place: String, news: String }`, `BookError::BadStill { news: String, why: String }`; an empty `headline`/`report`/`reads`/`overheard` and `fresh: 0` reuse `BookError::Empty(format!("… in news `{id}`"))`; `about` not in the cast is `BookError::UnknownRole`.
  - `news::NewsRun { pub id: String, pub mission: usize, pub step: usize, pub started: u64 /* game minute */, pub fresh: u64, pub channel: String, pub headline: String, pub report: Vec<String>, pub reads: Vec<String>, pub crawl: Vec<String>, pub overheard: Vec<String>, pub about: String, pub on: bool, pub ended: bool }` plus private bookkeeping fields the later tasks add (`#[derive(Clone, Debug, PartialEq)]`).
  - `news::put_out(st: &mut AppState, id: &str, mission: usize, step: usize)`; `news::current(st: &AppState) -> Option<&NewsRun>` (the item while fresh); `news::fresh(st: &AppState) -> bool`; `news::switch_off(st: &mut AppState)` (set off, ends the step once); `news::end_step(st: &mut AppState)` (ends the step once, the set left as it is); `news::channel(city: &citygen::City) -> String`.
  - `daze::Daze { pub start: u64 /* frame tick */, pub ticks: u64 }`; `daze::start(st: &mut AppState, seconds: f32)`.
  - `Missions.news: Option<news::NewsRun>`, `Missions.daze: Option<daze::Daze>`.

- [ ] **Step 1: Write the failing tests** (in `content.rs` tests, `news.rs` tests, `console.rs` tests)

```rust
// content.rs — with the module's small-book helper (follow `a_feed_from_nobody_is_refused`)
#[test] fn the_shipped_book_has_its_news() {
    let b = Book::load().unwrap();
    let n = &b.news["marlo"];
    assert_eq!((n.still.len(), n.still[0].chars().count()), (18, 18));
    assert_eq!(n.fresh, 1440);
}
#[test] fn an_unknown_news_is_refused()        { /* Effect::News("nope") → BookError::UnknownNews */ }
#[test] fn a_news_about_nobody_is_refused()    { /* about: "nobody" → UnknownRole */ }
#[test] fn an_empty_headline_is_refused()      { /* headline: "" → Empty */ }
#[test] fn an_empty_overheard_is_refused()     { /* overheard: [] → Empty */ }
#[test] fn a_bad_still_is_refused() {
    // 17 rows → BadStill; a row of 19 → BadStill; an 'x' in a row → BadStill
}
#[test] fn a_stale_at_once_news_is_refused()   { /* fresh: 0 → Empty */ }
#[test] fn news_is_a_presentation() {
    // a step `Ended("n")` after a step whose `then` is [News("x")] validates
}
#[test] fn the_news_roles_must_be_cast() {
    // a mission whose only effect is News("x"), x naming {marlo} in a text:
    // book.needs_cast(m) contains "marlo"
}

// news.rs — use crate::jobs::testutil::state_home() and missions::begin
#[test] fn putting_out_fills_the_texts_and_names_the_channel() {
    let mut st = state_home(); crate::missions::begin(&mut st);
    news::put_out(&mut st, "marlo", usize::MAX, usize::MAX);
    let n = news::current(&st).unwrap();
    assert!(n.on && !n.ended);
    for t in n.report.iter().chain(&n.reads).chain(&n.crawl).chain(&n.overheard).chain([&n.headline]) {
        assert!(!t.contains('{'), "{t}");
    }
    assert!(n.channel.ends_with(" NEWSLINE") && n.channel == n.channel.to_uppercase());
}
#[test] fn the_channel_is_a_corps_first_word() {
    // a city whose first named corp building is "Velform Group Plaza" → "VELFORM NEWSLINE";
    // a city with no named corp building → "CITY NEWSLINE"
}
#[test] fn news_goes_stale() {
    // put out at minute m; advance the world `fresh` minutes: current() is None, fresh() false
}
#[test] fn a_new_item_replaces_the_old() {}
#[test] fn switching_off_ends_the_step_once() {
    // a small book: step "n" [News], step "after" Ended("n") [Log("after")];
    // switch_off twice → one "after" log line, the mission done, n.on false
}
#[test] fn putting_out_without_a_home_or_city_does_not_panic() {
    let mut st = AppState::new(); news::put_out(&mut st, "marlo", usize::MAX, usize::MAX);
}
#[test] fn the_daze_effect_starts_a_daze() {
    // effects::apply(.., &Effect::Daze(4.0)) → st.missions.daze == Some(Daze { start: st.tick, ticks: 120 })
}

// console.rs
#[test] fn story_news_puts_out_an_item() {
    // run(&mut st, at, "story news marlo") → Ok("news marlo"); current(&st) is Some;
    // "story news nope" → Err("unknown news nope"); complete(&st, "story ", "n") offers "news"
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke missions:: console::`
Expected: FAIL (types and functions missing).

- [ ] **Step 3: Implement**

- The types and checks as in Interfaces. `News` texts take role slots: add them to the mission's roles through `needs_cast` (for each `Effect::News(id)` in the mission, the news's `about` and the roles of `slots(t)` over its texts, minus `you` and visitors). Validate texts' roles with `check_role` as feeds do.
- `presents()` includes `Effect::News(_)`. `texts()` and `named_roles()` return nothing for `News` and `Daze` (the news's own roles come through `needs_cast` and the news check).
- `put_out`: no book item → nothing (dev: `st.warn`). Fill every text with `missions::fill` (headline too); `channel(city)`; `started = st.net.now(st)`; `on: true`, `ended: false`; replaces `st.missions.news`. No city: the channel is `CITY NEWSLINE`.
- `channel`: the first building by id with `kind.is_corp()` and a name; its first word (split on whitespace) upper-cased; `"{WORD} NEWSLINE"`; else `"CITY NEWSLINE"`.
- `current`/`fresh`: fresh while `now - started < fresh`.
- `switch_off`: `on = false`, then `end_step`. `end_step`: once (`ended`), `missions::end(st, mission, step)` (a console item's `usize::MAX` ends nothing).
- `daze::start`: `ticks = (seconds * 30.0).round() as u64`, `start = st.tick`.
- `effects::apply`: `Effect::News(id) => news::put_out(st, mission, step...)`, `Effect::Daze(s) => daze::start(st, *s)`.
- Console: `["news", id]` → unknown → `Err(format!("unknown news {id}"))`, else `put_out(st, id, usize::MAX, usize::MAX)` and `Ok(format!("news {id}"))`; usage gains `news <id>`; completion list gains `"news"`.
- `flavor/names.ron`: replace `"Odile"` in `people` with `"Orla"` (check `grep -rn '"Orla"\|Orla' flavor crates` finds nothing first; if it does, pick another O-name the list and the story don't use). Same list length.
- `flavor/missions.ron`: add the `news: { "marlo": News(...) }` block exactly as the spec's §2 (texts and still rows verbatim). The story steps come in Task 2.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke missions:: console::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/strata-zero/src/missions crates/strata-zero/src/screens/console.rs flavor/missions.ron flavor/names.ron
git commit -m "news: a news item in the book, put out by the story"
```

---

### Task 2: The story's morning, the news tick's ending rules, the walk away and SOUGHT

**Files:**
- Modify: `flavor/missions.ron` (house_call's `up` step, the `morning_after` mission, the `waking` scene: spec §1, verbatim)
- Modify: `crates/strata-zero/src/missions/news.rs` (`tick`, `sought`, the walk away)
- Modify: `crates/strata-zero/src/missions/mod.rs` (`Body` gains the walk away)
- Modify: `crates/strata-zero/src/missions/effects.rs` (`Body` construction)
- Modify: `crates/strata-zero/src/screens/explore.rs` (call `news::tick` each update in walk mode, beside `crate::track::tick(st)`)

**Interfaces:**
- Consumes: Task 1's `NewsRun`, `put_out`, `end_step`, `current`, `fresh`.
- Produces:
  - `missions::Left { pub at: (u8, u8) /* clock */, pub heading: &'static str }`; `Body` gains `pub near: bool, pub left: Option<Left>`.
  - `news::tick(st: &mut AppState)`: per frame; this task adds the walk away and the step's ending; Tasks 5–6 add the set's reads and the outlets to it.
  - `news::sought(st: &AppState) -> Option<[String; 2]>`: the two SOUGHT lines for the current item.
  - `news::heading(from: Pos, to: Pos) -> &'static str` (eight points, north is `-y`).
  - `news::build_word(body: u8) -> &'static str`.

- [ ] **Step 1: Write the failing tests** (`news.rs` tests; story tests in `missions/mod.rs` beside `castellan_wakes_you_in_the_morning`, reusing `after_the_mail`, `answer_through`)

```rust
#[test] fn headings_have_north_up() {
    let o = Pos::new(10, 10);
    assert_eq!(news::heading(o, Pos::new(10, 0)), "north");
    assert_eq!(news::heading(o, Pos::new(20, 0)), "north-east");
    assert_eq!(news::heading(o, Pos::new(20, 10)), "east");
    assert_eq!(news::heading(o, Pos::new(10, 20)), "south");
    assert_eq!(news::heading(o, Pos::new(0, 20)), "south-west");
}
#[test] fn build_words() {
    assert_eq!([3, 4, 6, 7].map(news::build_word), ["slight", "medium", "medium", "heavy"]);
}
#[test] fn walking_away_from_a_body_is_recorded_once() {
    // a Body at p; put the player 5 tiles off (near), then 13 off to the north at 22:41:
    // left == Some(Left { at: (22, 41), heading: "north" }); walk on: unchanged
}
#[test] fn far_without_near_records_nothing() {}
#[test] fn sought_reads_you() {
    // salvage jacket worn, BODY 5, walked away at 22:41 north:
    // ["a courier · medium build · salvage jacket", "carrying a taped package · 22:41, heading north"]
    // nothing worn: "dark clothes"; no walk away: "carrying a taped package · late last night"
}
// missions/mod.rs
#[test] fn the_morning_after_wakes_dizzy_and_puts_out_the_news() {
    // after_the_mail → run to the call → answer_through: house_call done (runs[1]) only now;
    // tick: the waking scene plays (scene::running is "waking"); run it out (scene::skip or ticks);
    // then st.missions.daze is Some, news::current(&st) is Some with on, and the
    // morning_after objective is "Catch the news on the holo"
}
#[test] fn switching_the_set_off_sends_mezzs_mail() {
    // … then news::switch_off: a mail from Mezz, subject "the news", body ["Saw the news.",
    // "Don't go out looking like you."]; morning_after done
}
#[test] fn leaving_the_flat_sends_it_too() {
    // … move the player out of the home rect; news::tick → the mail; the set still on
}
#[test] fn out_of_the_flat_when_it_comes_sends_it_at_once() {}
#[test] fn staying_in_until_it_is_stale_sends_it_then() {}
#[test] fn the_house_call_is_not_done_while_the_call_rings() {
    // the morning call ringing: runs[1] not Done, objective still "Sleep · Castellan will be in touch"
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke missions::`
Expected: FAIL.

- [ ] **Step 3: Implement**

- `flavor/missions.ron`: spec §1 exactly (the `up` step's log text is ``"`House call` · morning"``).
- Walk away, in `news::tick` (every frame, any mode the explore update runs it in): for each body without `left`, on the body's place: within 6 tiles (Manhattan) sets `near`; `near` and more than 12 tiles away records `Left { at: st.clock(), heading: heading(body.pos, you) }`.
- `heading`: the angle of `(dx, -dy)` in eight 45° sectors centred on the points: `north`, `north-east`, `east`, `south-east`, `south`, `south-west`, `west`, `north-west`.
- `build_word`: 0..=3 `slight`, 4..=6 `medium`, 7.. `heavy`.
- `sought`: from the book item's `sought`/`carrying`, `character.attribute(&content, Attribute::Body)`, `character.equipped(Slot::Torso)` → `content.item(id).name.to_lowercase()` or `dark clothes`, and the body of the item's `about` role (`person_of`) → its `left`.
- Ending (spec builder rulings): if the current item has a mission step and is not `ended`: end it when you are not in your flat (`Where::Home`'s test: `st.home` and `rig.home.rect.contains(p.pos)` on its place; no home counts as out) or when the item is stale (`now - started >= fresh`; check the stored item, not `current`).
- `explore.rs`: `crate::missions::news::tick(st)` right after `crate::track::tick(st)`, in every update the world runs in (it gates itself on what it needs).
- Watch the `Body` constructors in tests (`near: false, left: None`).

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke missions::`
Expected: PASS (and the existing `castellan_*` tests still pass).

- [ ] **Step 5: Commit**

```bash
git commit -am "story: the morning after wakes you dizzy and the news is on"
```

---

### Task 3: The holo set

**Files:**
- Modify: `crates/citygen/src/tile.rs` (`Furniture::Holo` after `Chair`, `label` → `"holo set"`, the labels test)
- Modify: `crates/citygen/src/interior/layouts/home.rs` (`V` at row 4, column 16; the legend comment; `'V' => Furniture::Holo`)
- Modify: `crates/citygen/src/interior/home.rs` (the home test's furniture list gains `Holo`; assert its tile)
- Modify: `crates/strata-zero/src/render/tileset.rs` (glyph `◫`; legend `('◫', "holo set")`; the glyph table test)
- Modify: `flavor/looks.ron` (`furniture` `"holo set"` lines, spec §3)
- Modify: `crates/strata-zero/src/screens/explore.rs` (while the set is on, its cell in the cycling holo glyphs, MAGENTA; drawn after the visitors, before your `@`)
- Modify: `crates/strata-zero/src/missions/news.rs` (`set_tile(st) -> Option<(Place, Pos)>`)

**Interfaces:**
- Consumes: Task 1's `current`.
- Produces: `news::set_tile(st: &AppState) -> Option<(citygen::Place, citygen::Pos)>` (your flat's `Holo` tile, scanning `rig.home.rect` as `marks::resolve` does for `HomeFurniture`); `Furniture::Holo`.

- [ ] **Step 1: Write the failing tests**

```rust
// citygen home.rs test: Holo in the `has` list, and
assert_eq!(f.tiles[Pos::new(h.rect.x + 16, h.rect.y + 4)].feature, Some(Feature::Furniture(Furniture::Holo)));
// tileset.rs table: (Furniture::Holo, '◫'); legend has "holo set"
// look.rs: looking at the set in the home reads head "Holo set" and one of the two lines
// news.rs: set_tile(&state_home()) is Some, on the home's place, two rows above a couch tile
// explore.rs: with the news on and you home, the set's screen cell is one of the holo glyphs in MAGENTA;
//             switched off, it is '◫'
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --workspace --target-dir target/smoke holo`
Expected: FAIL.

- [ ] **Step 3: Implement** as the spec's §3 and the Files list. The set's cell: `pick(&style.glyphs.holo, st.tick / 8)` (reuse `render::tileset`'s glyph set; expose what is needed), fg `MAGENTA`, only when `news::current(st).is_some_and(|n| n.on)` and the set's place is the map's.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --workspace --target-dir target/smoke`
Expected: PASS (citygen's seed sweeps included).

- [ ] **Step 5: Commit**

```bash
git commit -am "home: a holo set in front of the couch"
```

---

### Task 4: The broadcast and Watch

**Files:**
- Create: `crates/strata-zero/src/missions/broadcast.rs`
- Modify: `crates/strata-zero/src/missions/mod.rs` (`pub mod broadcast;`)
- Modify: `crates/strata-zero/src/interact.rs` (the `Holo set` subject after the bed's)
- Modify: `crates/strata-zero/src/screens/popup.rs` (`Pick::Watch`)
- Modify: `crates/strata-zero/src/screens/shots.rs` (ignored `news_shots`, its first shots)

**Interfaces:**
- Consumes: `news::current`, `news::switch_off`, `news::sought`, `news::set_tile`, the book item's `still`, `camera`, `close`, `about`; `face::bracket_frame`; `ui::sidebar::wrap_text`; `ui::chrome::{put, DIM, FAINT, MAGENTA, PANEL_BG, TEXT}`; `cast::district_of`.
- Produces: `broadcast::Broadcast` (`impl Screen`), `Broadcast::new() -> Broadcast`; `broadcast::layout(frame: Rect, report: &[String]) -> (Rect, Vec<String>)`; `broadcast::still_colour(level: char, row: usize) -> Color`; `Pick::Watch`.

- [ ] **Step 1: Write the failing tests** (`broadcast.rs`, `interact.rs`)

```rust
#[test] fn the_box_fits_both_sizes() {
    for (w, h) in [(120, 40), (100, 30)] { let (r, _) = broadcast::layout(Rect::new(0, 0, w, h), &report);
        assert!(r.width <= 90 && r.width + 4 <= w && r.height >= 17 && r.height + 2 <= h); }
}
#[test] fn it_draws_what_the_mock_drew() {
    // state_home, begin, put_out "marlo", drawn at 120x40: rows contain "HOLO", "▸ news",
    // the channel, "LIVE", the clock; the still's first cell is '▀' at (r.x+3, r.y+3);
    // "CAM 14"; the report's first words at column r.x+22; "SOUGHT"; "space turn it off";
    // " NEWSLINE " on row r.bottom()-2
}
#[test] fn the_crawl_moves() { /* the crawl row differs between st.tick = 0 and st.tick = 30 */ }
#[test] fn space_switches_off_and_ends_the_step() {}
#[test] fn esc_closes_with_the_set_on() {}
#[test] fn enter_does_nothing() {}
#[test] fn with_the_item_gone_it_pops() { /* stale or None: update → Pop; draw writes nothing */ }
// interact.rs
#[test] fn the_set_offers_watch_while_the_news_is_fresh() {
    // beside the set at home: subject "Holo set", choice Interact: Watch, Pick::Watch;
    // no news → no subject; a stale item → none
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke broadcast interact`
Expected: FAIL.

- [ ] **Step 3: Implement** the spec's §4 from `news_mock.rs`: `MAX_W = 90`, `MIN_H = 17`, `TEXT_AT = 22`, `TEXT_LESS = 26`; red `Color::Rgb(255, 48, 72)`; band `(26, 8, 14)`; crawl band `(40, 0, 32)`; greens `[(4,8,5),(11,23,16),(21,41,27),(36,67,44),(63,112,73),(106,168,119),(182,240,194)]`, odd pixel rows ×0.8; the grain from the mock's LCG (seed `0x5eed_1234`, 5 % up, 5 % down) over the content rows; the crawl `" ▸ " + lines.upper().join("   ▸ ")`, offset `st.tick / 3` wrapping; height `report_rows + 11` at least `MIN_H`, at most frame height − 2. Caption `● <camera> · HH:MM` from the about-body's `left` (the `· HH:MM` left out when none); the district row from `cast::district_of`. The SOUGHT card's two lines from `news::sought`; when it gives none, the card and its blank row are left out.
- `Pick::Watch`: `news` on (`n.on = true`), `ScreenCmd::Replace { to: Box::new(Broadcast::new()), fx: TransitionKind::Glitch }`.
- The subject: as the bed's, a 4-neighbour inside the home rect with `Furniture::Holo`, header `"Holo set"`, `Choice::on('i', "Interact", "Watch", CYAN, Pick::Watch)`, only while `news::current(st).is_some()`.
- `news_shots` (ignored, `shot_stack`): `news_broadcast_{w}x{h}` for `HOLO_SIZES` at 07:14 (advance the world like the mock's test).

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke broadcast interact` then `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored news_shots` and look at the PNGs against `broadcast_120x40.png`.
Expected: PASS; the shot matches the mock but for the live text.

- [ ] **Step 5: Commit**

```bash
git add -A crates/strata-zero/src && git commit -m "news: the broadcast, and Watch on the holo set"
```

---

### Task 5: Asides, and the set reading the news

**Files:**
- Create: `crates/strata-zero/src/missions/asides.rs`
- Modify: `crates/strata-zero/src/missions/mod.rs` (`pub mod asides;`, `Missions.asides: Vec<asides::Aside>`)
- Modify: `crates/strata-zero/src/missions/bubbles.rs` (share the frame drawing: factor the box, label, typed text and tail of `say` into a `pub(super) fn frame(...)` both call; `say` unchanged in what it draws)
- Modify: `crates/strata-zero/src/missions/news.rs` (the set's reads in `tick`)
- Modify: `crates/strata-zero/src/screens/explore.rs` (draw the asides after the rest window, before the call's card; clear them when a scene, rest or fight starts)
- Modify: `crates/strata-zero/src/screens/shots.rs` (`news_shots`: `news_set_reading`)

**Interfaces:**
- Consumes: `bubbles::place`, `overlay::{typed_rows, ink}`, `call::{TYPE_PER_SECOND, TICKS_PER_SECOND}`, `news::{current, set_tile}`.
- Produces:
  - `asides::Speaker { Tile(Place, Pos), Person(sim::PersonId) }`.
  - `asides::Aside { pub speaker: Speaker, pub label: String, pub colour: Color, pub text: String, pub start: u64, pub until: u64 }`.
  - `asides::say(st: &mut AppState, speaker: Speaker, label: String, colour: Color, text: String)` (starts now, `until = start + lasts(&text)`; a third replaces the oldest).
  - `asides::lasts(text: &str) -> u64` = `75 + chars * 3 / 2` clock ticks (2.5 s + 1 s per 20 characters).
  - `asides::live(st: &AppState) -> bool` (walk mode is the caller's; this is: no scene running or queued, no rest, no fight).
  - `asides::prune(st: &mut AppState)` (drops ended ones; all of them when not `live`).
  - `asides::draw(buf: &mut Buffer, map: Rect, origin: Pos, st: &AppState)`.
  - `ASIDE_W = 28`, `MAX_ASIDES = 2`, `READ_GAP = 45` (clock ticks).

- [ ] **Step 1: Write the failing tests** (`asides.rs`, `news.rs`)

```rust
#[test] fn an_aside_lasts_by_its_length() { assert_eq!(asides::lasts(&"x".repeat(40)), 75 + 60); }
#[test] fn at_most_two() { /* three says → the first gone */ }
#[test] fn it_pauses_with_the_clock() { /* frame ticks pass, clock ticks don't: still up */ }
#[test] fn none_in_a_scene_asleep_or_a_fight() { /* each: prune empties, draw draws nothing */ }
#[test] fn it_keeps_off_your_at() { /* drawn at 100x30 and 120x40: your @ cell untouched, inside the map */ }
#[test] fn a_speaker_off_screen_is_not_drawn() {}
#[test] fn the_set_reads_in_turn_round_and_round() {
    // home, news on: tick through: reads[0], then after it ends + READ_GAP reads[1] … back to reads[0];
    // label is the channel, colour MAGENTA, speaker the set's tile
}
#[test] fn the_set_stops_when_off_or_you_leave() {}
#[test] fn the_scene_bubble_is_unchanged() { /* the existing bubbles tests pass unchanged */ }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke asides news`
Expected: FAIL.

- [ ] **Step 3: Implement** the spec's §5. The set's reads: `NewsRun` keeps the next read index and the clock tick the last one ended; when the set is on, you are in your flat on its place, no aside from the set is up and `READ_GAP` has passed, `say` the next. A person speaker is drawn where `world.pos_of(id)` is, on the street only. The avoid list: your `@`, the visible visitors, the visible walkers' cells in the map. Draw order in `explore.rs`: after `rest::draw_window`, before `call::draw`, only when `self.mode == Mode::Walk` and `asides::live(st)`.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/strata-zero/src && git commit -m "news: bubbles that leave you the keys, and the set reads it out"
```

---

### Task 6: The city's outlets: overheard, holo ads, screens

**Files:**
- Modify: `crates/strata-zero/src/missions/news.rs` (the three outlets in `tick`; `cut_in(st) -> Option<Pos>`; the roll)
- Modify: `crates/strata-zero/src/screens/explore.rs` (a cut-in's cell in MAGENTA, drawn after the visitors, before your `@`)
- Modify: `crates/strata-zero/src/look.rs` (the cut-in's body line)
- Modify: `crates/strata-zero/src/screens/shots.rs` (`news_shots`: `news_overheard`, `news_holo_ad`)

**Interfaces:**
- Consumes: `asides::say`, `news::current`, `world.visible(rect)`, `player.vis()`, `world.is_dead`, `missions.cast`, `citygen::{Feature::HoloAd, BuildingKind::{Bar, Club, NoodleBar}}`.
- Produces: `news::cut_in(st: &AppState) -> Option<Pos>` (the street tile cutting in now); `news::roll(seed: u64, minute: u64, salt: u64, one_in: u64) -> bool` (a splitmix-style hash of the three, `% one_in == 0`); constants `HEARD_WITHIN = 6`, `HEARD_GAP = 12`, `HEARD_ONE_IN = 4`, `AD_WITHIN = 10`, `AD_GAP = 8`, `AD_ONE_IN = 3`, `CUT_TICKS = 90`; `NewsRun` bookkeeping: next overheard index, last overheard minute, last cut minute, the cut (`Pos`, until tick), the last minute rolled, `screened: BTreeSet<BuildingId>`, and `inside: Option<BuildingId>` (the building you were in last frame).

- [ ] **Step 1: Write the failing tests** (`news.rs`, `look.rs`)

```rust
#[test] fn the_roll_is_steady() { assert_eq!(news::roll(1, 2, 3, 4), news::roll(1, 2, 3, 4)); }
#[test] fn overheard_from_someone_close_and_seen() {
    // street, a visible walker 3 tiles off, minute rolls forced (pick a minute whose roll is true):
    // an aside on Speaker::Person(id), label "overheard", the first overheard line; the next one, the second
}
#[test] fn never_within_twelve_minutes() {}
#[test] fn never_a_story_role_or_the_dead() {}
#[test] fn nothing_off_the_street() { /* on a floor, in the flat: no overheard, no cut; no panic */ }
#[test] fn a_holo_ad_cuts_in() {
    // a visible HoloAd within 10: cut_in == Some(ad); an aside labelled "NEWSLINE", text "▸ " + headline upper;
    // after CUT_TICKS: None
}
#[test] fn look_reads_the_headline_during_the_cut() {
    // look card body at the ad during: "The ad stutters. For a second it's a headline: COURIER SOUGHT IN …."
    // after: the ad's own line
}
#[test] fn screens_log_once_per_building() {
    // entering a Bar's floor: "Over the counter, a screen: <HEADLINE>." once; out and in again: nothing new;
    // a NoodleBar footprint on the street: once; a Shop: nothing
}
#[test] fn stale_news_is_quiet() {}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke news look`
Expected: FAIL.

- [ ] **Step 3: Implement** the spec's §6. Rolls happen once per game minute (when `st.net.now(st)` passes the last minute rolled), only in walk mode with `asides::live`. Overheard: candidates from `world.visible(Rect around you, 6)` that are walkers, visible in your `vis()`, Chebyshev ≤ 6, not in `st.missions.cast` values, not dead; the nearest, ties to the lowest id. Holo ads: scan the street tiles within 10 (Chebyshev) for `Feature::HoloAd` visible in `vis()`; nearest. The cut's map cell: fg `MAGENTA` (keep the glyph). Look: in the feature arm, `Feature::HoloAd` at `news::cut_in(st) == Some(pos)` → the stutter line in place of the flavour line. Screens: the building you are in (`Place::Floor { building, .. }` → building; on the street, the tile's `building`); a change into a `Bar`, `Club` or `NoodleBar` not in `screened` → `st.say(format!("Over the counter, a screen: {}.", headline.to_uppercase()))` and remember it.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/strata-zero/src && git commit -m "news: the city mutters it, holo ads cut in, screens over counters"
```

---

### Task 7: The daze

**Files:**
- Modify: `crates/strata-zero/src/missions/daze.rs` (`draw`, `strength`)
- Modify: `crates/strata-zero/src/screens/explore.rs` (call it after your `@` is drawn and before `drawn_you`/film/bubbles/asides/call)
- Modify: `crates/strata-zero/src/screens/shots.rs` (`news_shots`: `news_daze`)

**Interfaces:**
- Consumes: Task 1's `Daze`.
- Produces: `daze::strength(st: &AppState) -> f32` (1.0 at the start, 0.0 at and after the end, eased out: `(1 - t)^2` with `t` the share passed); `daze::draw(buf: &mut Buffer, map: Rect, st: &AppState)`; `DRIFT = 2` (cells).

- [ ] **Step 1: Write the failing tests** (`daze.rs`)

```rust
#[test] fn it_fades_out() { /* strength 1.0 at start, between at half, 0.0 at end and after */ }
#[test] fn rows_slide_and_colours_split_early() {
    // a map buffer of known cells; at strength 1 some row is shifted by 1 or 2 and some fg moved
    // toward magenta or cyan; never more than DRIFT
}
#[test] fn nothing_changes_after() { /* at the end the buffer is untouched */ }
#[test] fn only_the_map() { /* cells outside `map` untouched; stays inside the buffer at 100x30 */ }
#[test] fn the_daze_ends_and_is_cleared() { /* the explore update drops it once over */ }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke daze`
Expected: FAIL.

- [ ] **Step 3: Implement** the spec's §8 and the builder ruling. Copy the map rect's cells; row `y` takes its cells from `x - shift(y)` with `shift = round(DRIFT · s · sin(0.7·y + 0.25·tick))` (cells shifted in from outside the map get the panel colour); the bleed: on cells whose `(x + y + tick / 4) % 3` is 0, the fg mixes toward `(255, 0, 200)` by `0.5·s`, on 1 toward `(0, 229, 255)`. Frame ticks.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/strata-zero/src && git commit -m "news: the morning's daze"
```

---

### Task 8: All checks, both front ends, and the shots

**Files:**
- Modify: whatever the checks find.

- [ ] **Step 1:** Run the five Global Constraints checks. Expected: all green, no warnings.
- [ ] **Step 2:** `cargo check -p strata-zero --features window --target x86_64-unknown-linux-gnu --target-dir target/smoke` (if the target is installed; skip and say so if not).
- [ ] **Step 3:** `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored news_shots`; look at every PNG (the set reading in the flat, the broadcast at both sizes, an overheard bubble, a holo ad cutting in, the daze half way).
- [ ] **Step 4: Commit** any fixes: `git commit -am "news: <what>"`.
