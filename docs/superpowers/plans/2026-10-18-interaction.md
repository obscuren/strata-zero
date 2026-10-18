# Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Space window names its one subject and offers fixed verbs with fixed letters. Several things in reach ask "which way". Look is a free cursor with an info window. Space acts, Enter only confirms. `J` jacks out after a confirm.

**Architecture:** `Popup` gains a header, a subtitle and two-part rows (`Verb: Thing`), and its cards close the whole popup. A new `interact.rs` gathers the subjects around the player and builds each one's rows. A new `look.rs` writes the look card from the map, the visibility grid and a new `flavor/looks.ron`, and places its box. `MapScreen` gets two layers on Walk in the style of `targeting`: `which_way` and `looking`. Popups ask the map for look mode through `AppState::look_at`.

**Tech Stack:** Rust 2024, ratatui, crossterm, ron. Tests are `cargo test -p strata-zero` with `TestBackend`.

**Spec:** `docs/superpowers/specs/2026-10-18-interaction-design.md`

## Global Constraints

- Verb letters: `x` Look, `t` Talk, `r` Trade (reserved, never shown), `i` Interact, `j` Jack in/out, `g` Take (one stack) or Take all (several), `>` `<` Go up/down.
- Free letters for take, ride and program rows: `abcdefmnopsuvwyz` (no verb letter, no `h j k l q`).
- Rows read `Verb` or `Verb: Thing` with the verb capitalised. The thing is coloured by kind: terminals `CYAN`, keypads `hud::AMBER`, items in their kind colour, places (floors, strata) in `TEXT` and bold, nodes `CYAN`, ICE `MAGENTA`, yourself `TEXT`.
- The Space window and every `Popup` are at least 40 columns wide, up to `MAX_W` 60. The look box is 48 wide.
- A header names the window's single subject, or is blank. Menus keep a title naming the list: "View", "Abilities", "Floors". No header is upper-cased. A confirm's header is its question: "Jack out?".
- Esc from any window, card, the which-way pick or look returns to the game. It never returns to the rows.
- On the map, Enter never starts an action. It still confirms targeting, walks the construct aim, fires the selected row, and closes cards.
- Look text never contains digits, except `×n` counts and floor labels. It never says "You see". Remembered tiles show no people or items. Unseen tiles say exactly "You can't see there."
- Generated text about people uses they/them.

## Review Focus

1. **The look box covering the cursor** at the map's edges, on a narrow map (`cargo run` at 100×30), and with a cursor on the middle column. The box must never overlap the cursor's screen cell. Pinned in Task 6 by `look_box_never_covers_the_cursor`, which sweeps every cursor cell of a 100×30 and a 60×20 map.
2. **Mid-fight escape.** `J` on your act half with a run armed opens the confirm, and confirming jacks out; on the ICE's turn it warns "not your turn". Pinned in Task 4 by `j_jacks_out_mid_fight_with_a_run_armed`.
3. **Two subjects in the same direction, or none** (a person standing beside a terminal; a terminal reached diagonally). Each direction shows one subject; a direction with nothing does nothing. Pinned in Task 3 by `which_way_opens_the_direction_and_ignores_empty_ones`.
4. **Look on a remembered tile with a person or an item on it**, which must name neither. Pinned in Task 5 by `remembered_tiles_hide_people_and_items`.
5. **Letters running out** with many stacks under you (more than the free pool). Rows beyond the pool are dropped and the rest keep their letters; nothing panics. Pinned in Task 2 by `take_rows_stop_when_the_letters_run_out`.

---

## File Structure

- `crates/strata-zero/src/screens/popup.rs`: the window. It gains `Header`, the subtitle, `Choice::on`, `tail` and `alias`, cards that close it, `Popup::about`, `Popup::jack_out_confirm`, and the new `Pick` variants.
- `crates/strata-zero/src/interact.rs` (new): `Subject`, `gather`, the verb rows, the construct's program rows, the free-letter pool.
- `crates/strata-zero/src/render/mark.rs` (new): the glyph and colour a tile shows (person, item pile, or tile cell). Window headers and look cards both use it.
- `crates/strata-zero/src/look.rs` (new): `LookCard`, `look_card`, `points_of_interest`, `Looks` (the flavour file), and `place_box` and `draw_box` for the info window.
- `flavor/looks.ron` (new): body lines per kind.
- `crates/strata-zero/src/render/path.rs`: `BlockOverlay`, a background block on tiles, for the look cursor and the which-way candidates.
- `crates/strata-zero/src/screens/explore.rs`: `interact` delegates to `interact::gather`, plus `which_way`, `looking`, `J`, Enter's removal, `x`, the footer prompts, and `focus_pos`.
- `crates/strata-zero/src/screens/deck.rs`: popup titles as names, capitalised verbs.
- `crates/strata-zero/src/net.rs`: `jack_in_here(st) -> Result<String, String>`, shared by the popup and tests.
- `crates/strata-zero/src/app.rs`: `AppState::look_at: Option<Pos>`.
- `crates/strata-zero/src/screens/help.rs`, `README.md`: keys.

---

### Task 1: The window's new look

**Files:**
- Modify: `crates/strata-zero/src/screens/popup.rs`
- Modify: `crates/strata-zero/src/screens/deck.rs:196-320` (titles and row labels)
- Modify: `crates/strata-zero/src/screens/explore.rs` (`view_popup`, `abilities`, `floors_popup` titles; their row labels capitalised)
- Test: `crates/strata-zero/src/screens/popup.rs` (tests module)

**Interfaces:**
- Produces:
  - `pub struct Header { pub glyph: Option<(char, Color)>, pub name: String, pub fg: Color }` and `Header::title(name: &str) -> Header` (no glyph, `CYAN`).
  - `Popup::verbs(title: &str, choices, tick)` keeps its signature. An empty title draws a blank top border; otherwise the title is drawn as written.
  - `Popup::about(header: Option<Header>, subtitle: Option<String>, choices: Vec<Choice>, tick: u64) -> Popup`.
  - `Choice` gains `thing: Option<(String, Color)>`, `tail: String` and `alias: Option<char>`. Add `Choice::on(key, verb, thing: impl Into<String>, colour: Color, pick) -> Choice`, `.tail(s)` and `.alias(c)`. The row draws `verb`, then `": "` and `thing` in `colour` when present, then `tail` in the label style. The width counts all three.
  - Any close key on a card (`Esc q Enter Space`) returns `ScreenCmd::Pop`.
  - `pub(crate) const MIN_W: u16 = 40`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_header_names_the_subject_and_a_blank_one_draws_nothing() {
    let mut st = state();
    st.tick = 40;
    let p = Popup::about(
        Some(Header { glyph: Some(('@', CYAN)), name: "Mara Chen".into(), fg: CYAN }),
        Some("door muscle · on the door".into()),
        vec![Choice::new('x', "Look", Pick::Close), Choice::new('t', "Talk", Pick::Close)],
        0,
    );
    let text = draw(&p, &st, 120, 35);
    assert!(text.contains("@ Mara Chen"), "{text}");
    assert!(text.contains("door muscle · on the door"), "{text}");
    assert!(text.contains("x  Look") && text.contains("t  Talk"), "{text}");
    let blank = Popup::verbs("", vec![Choice::new('x', "Look", Pick::Close)], 0);
    let r = blank.rect(Rect::new(0, 0, 120, 35));
    assert!(r.width >= 40, "{r:?}");
    let text = draw(&blank, &st, 120, 35);
    let top = text.lines().nth(r.y as usize).unwrap();
    assert!(!top.chars().any(|c| c.is_alphabetic()), "blank header: {top}");
}

#[test]
fn a_row_draws_verb_colon_thing_and_tail() {
    let mut st = state();
    st.tick = 40;
    let p = Popup::verbs(
        "",
        vec![Choice::on('i', "Interact", "Keypad", hud::AMBER, Pick::Close).tail(" · locked 4m")],
        0,
    );
    let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
    t.draw(|f| p.draw(&st, f)).unwrap();
    let text = buffer_text(t.backend().buffer());
    assert!(text.contains("i  Interact: Keypad · locked 4m"), "{text}");
    let (x, y) = find(t.backend().buffer(), "Keypad");
    assert_eq!(t.backend().buffer()[(x, y)].fg, hud::AMBER);
}

#[test]
fn a_card_closes_the_whole_popup() {
    let mut st = state();
    let mut p = Popup::verbs("", vec![Choice::new('i', "Inspect", Pick::Card(vec![Line::from("a spool")]))], 0);
    assert!(matches!(p.update(&mut st, &[key(KeyCode::Char('i'))]), ScreenCmd::Stay));
    assert!(matches!(p.update(&mut st, &[key(KeyCode::Esc)]), ScreenCmd::Pop { .. }));
}

#[test]
fn an_alias_fires_its_row() {
    let mut st = state();
    let mut p = Popup::verbs("Jack out?", vec![
        Choice::new('y', "Jack out", Pick::Close).alias('J'),
        Choice::new('n', "Stay in", Pick::Close),
    ], 0);
    assert!(matches!(p.update(&mut st, &[key(KeyCode::Char('J'))]), ScreenCmd::Pop { .. }));
}
```

`find(buf, needle) -> (u16, u16)` is a small test helper: it returns the cell of the needle's first character. Update `popup_refusal_warns_and_cards_open_and_close` so its last Esc expects `Pop` straight from the card.

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p strata-zero popup::tests`
Expected: compile errors for `Header`, `Popup::about`, `Choice::on`, `tail` and `alias`.

- [ ] **Step 3: Implement the window changes in `popup.rs`**

`Popup` stores `header: Option<Header>` and `subtitle: Option<String>`. `verbs` maps a non-empty title to `Header::title`. `rect` adds one row for the subtitle plus one blank row under it, and counts the header width as `name + 4` (+2 with a glyph). The draw puts ` <glyph> <name> ` on the top border at `x + 2`, the glyph in its colour and the name in `fg`, bold. The subtitle goes on the first inner row in `DIM`. The card branch returns `Pop` on its close keys. Letter matching checks `ch.key == c || ch.alias == Some(c)`.

- [ ] **Step 4: Retitle and recapitalise the other popups**

In `deck.rs`, titles are `slot.label()` with its first letter capitalised, and `item.name`, `cat.name` and `skill.name` as written. Rows become "Inspect", "Equip", "Unequip", "Use", "Drop one", "Drop all", "Unlock · N SP" and "Unlock · free". In `explore.rs`: `view_popup` is titled "View", `abilities` "Abilities", `floors_popup` "Floors". Rows start with a capital letter.

- [ ] **Step 5: Run the whole crate's tests**

Run: `cargo test -p strata-zero`
Expected: the new tests pass. Fix any existing test that asserted an upper-cased title or a lowercase deck row so it expects the new text. Don't touch the Space window tests yet.

- [ ] **Step 6: Commit**

```bash
git add -A crates/strata-zero/src/screens/popup.rs crates/strata-zero/src/screens/deck.rs crates/strata-zero/src/screens/explore.rs
git commit -m "feat(app): the window names its subject, rows read verb: thing, cards close it"
```

---

### Task 2: Subjects and the verb table

**Files:**
- Create: `crates/strata-zero/src/interact.rs`, `crates/strata-zero/src/render/mark.rs`
- Modify: `crates/strata-zero/src/main.rs` (or `lib.rs`: wherever modules are declared), `crates/strata-zero/src/render/mod.rs`
- Modify: `crates/strata-zero/src/screens/popup.rs` (new `Pick` variants)
- Modify: `crates/strata-zero/src/app.rs` (`look_at`)
- Modify: `crates/strata-zero/src/screens/explore.rs` (`interact`, `run_choices`, `floor_rows`, `WORLD_LETTERS`)
- Modify: `crates/strata-zero/src/render/people.rs` (`tint_colour` becomes `pub(crate)`)
- Test: `crates/strata-zero/src/interact.rs`, and the Space-window tests in `explore.rs`

**Interfaces:**
- Consumes: `Header`, `Choice::on`, `.tail`, `Popup::about` (Task 1).
- Produces:
  - `pub const FREE_LETTERS: &str = "abcdefmnopsuvwyz";` replaces `WORLD_LETTERS`.
  - `pub fn mark(st: &AppState, map: &MapRef, place: Place, pos: Pos, person: Option<sim::PersonId>) -> Option<(char, Color)>` in `render/mark.rs`. It gives `'@'` in the person's tint, else the item pile's glyph and colour as `GroundOverlay` draws it, else `tileset::cell` for the tile, lit as `Visible`.
  - `pub struct Subject { pub pos: Pos, pub dir: Option<Dir>, pub header: Option<Header>, pub subtitle: Option<String>, pub choices: Vec<Choice> }`. `dir` is `None` for the player's own tile. `Subject::name(&self) -> String` is the header's name, or "here" when the subject has none.
  - `pub fn gather(st: &AppState, inside_person: &dyn Fn(Place, Pos) -> Option<sim::PersonId>) -> Vec<Subject>` returns the subjects in reach, here first, then by `Dir::ALL` order. Look is not among a subject's choices; `Subject::window(&self, tick) -> Popup` prepends `Choice::new('x', "Look", Pick::Look(self.pos))`.
  - `pub fn run_rows(st, b, pos, only: Option<&ItemId>, letters: &mut Chars) -> Vec<Choice>` moves here from `MapScreen::run_choices`, with rows `Choice::on(key, Op label capitalised, what, colour, Pick::Run(..)).tail(" · 64% · 3 turns · noise 4")`.
  - New `Pick` variants:
    - `Look(Pos)` sets `st.look_at = Some(pos)` and pops.
    - `Talk { lines: Vec<Line<'static>>, said: String }` shows `lines` as the card and logs `said` with `st.say`.
    - `JackOutAsk` replaces the popup with `Popup::jack_out_confirm(tick)`.
  - `Popup::jack_out_confirm(tick) -> Popup` builds the "Jack out?" header with rows `Choice::new('y', "Jack out", Pick::World(WorldAct::JackOut)).alias('J')` (selected) and `Choice::new('n', "Stay in", Pick::Close)`.
  - `AppState::look_at: Option<Pos>`, initialised to `None` and not saved.

Verb rows per subject, in this order. Look comes first, added by `window`.

| Subject | Header | Subtitle | Rows |
|---|---|---|---|
| Person (4-neighbour: `world.person_at` on the street, `inside_person` indoors) | card name, `mark` glyph and tint | `"{profession} · {doing}"` | `t` Talk |
| Locked flat door (4-neighbour, rule as today) | `flat_label`, first letter capitalised | none | `Choice::on('i', "Interact", "Keypad", hud::AMBER, Pick::Keypad{..})`, with `.tail(" · locked Nm")` while locked |
| Terminal in reach (`terminal_in_reach`, 8-neighbour) | "Access terminal" / "Access rig" | none | `Choice::on('i', "Interact", "Terminal", CYAN, Pick::Terminal{..})`, `j` Jack in |
| Here (own tile) | connector, stairs or lift name first, else the top stack "Name ×n" (×n only above 1) | none | take rows, Go up/down, Ride rows, and Jack out (`Pick::JackOutAsk`) while jacked in |

- **Take rows.** With one stack: `Choice::on('g', "Take", "Name ×n", kind colour, Pick::Act(Take))`. With several: `g` "Take all" first, then one `Take: Name ×n` row per stack, lettered from `FREE_LETTERS`.
- **Go up and down.** `Choice::on('>', "Go up", dest.label(), TEXT, ..)` with bold places, and the same with `<` for down. Stairs indoors use `FloorPlan::label(level ± 1)`.
- **Ride rows.** `Choice::on(letter, "Ride", label, TEXT, Pick::World(Lift(l)))`, with the current floor disabled.
- **Here names.** Connector kinds give "Stairs", "Lift", "Ladder" or "Manhole"; indoor stairs give "Stairwell" and a lift bank "Lift".
- **Which subjects exist.** A here subject exists only when it has at least one row. In a construct, `gather` returns no subjects; `interact` builds the program window instead (see Step 4).

- [ ] **Step 1: Write the failing tests in `interact.rs`**

Use the `explore.rs` test fixtures (`state`, `walking`, `stand`, `seed_world`, and the terminal and keypad setups in `the_access_terminal_row_*` and `a_locked_flat_names_its_resident`). Move the fixtures into `screens/testutil.rs` as `pub(crate)` when both test modules need them.

```rust
#[test]
fn a_terminal_beside_you_is_one_subject_with_interact_and_jack() {
    // Stand beside a district terminal (as in the access terminal tests).
    let subs = gather(&st, &|_, _| None);
    let t = subs.iter().find(|s| s.header.as_ref().is_some_and(|h| h.name == "Access terminal")).unwrap();
    let rows: Vec<String> = t.choices.iter().map(row_text).collect();
    assert_eq!(rows, ["i Interact: Terminal", "j Jack in"]);
}

#[test]
fn a_person_beside_you_offers_talk_under_their_name() { /* header == card.name, subtitle == "{profession} · {doing}", rows == ["t Talk"] */ }

#[test]
fn a_locked_flat_offers_interact_keypad() { /* rows == ["i Interact: Keypad"]; header == flat_label capitalised */ }

#[test]
fn nothing_in_reach_gathers_no_subjects() { /* on an empty sidewalk: gather(...).is_empty() */ }

#[test]
fn stacks_under_you_take_g_and_take_all() {
    // Two stacks dropped on the player's tile.
    let here = &gather(&st, &|_, _| None)[0];
    assert_eq!(here.dir, None);
    let rows: Vec<String> = here.choices.iter().map(row_text).collect();
    assert_eq!(rows[0], "g Take all");
    assert!(rows[1].starts_with("a Take: ") && rows[2].starts_with("b Take: "), "{rows:?}");
}

#[test]
fn take_rows_stop_when_the_letters_run_out() {
    // FREE_LETTERS.len() + 3 distinct stacks on the tile: no panic, every row has a distinct key.
}

#[test]
fn the_window_puts_look_first_on_x() {
    let w = subject.window(0);
    assert_eq!(w.choices()[0].key, 'x');
    assert!(matches!(w.choices()[0].pick, Pick::Look(p) if p == subject.pos));
}
```

`row_text(c: &Choice) -> String` formats `"{key} {label}{: thing}{tail}"`. Add `Popup::choices(&self) -> &[Choice]` for tests.

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p strata-zero interact::`
Expected: compile errors because `gather` and `Subject` are missing.

- [ ] **Step 3: Implement `mark`, `Subject`, `gather`, `run_rows`, the `Pick` variants, `jack_out_confirm` and `look_at`**

Move the body of `MapScreen::interact`'s row building into `gather`, keeping every existing rule: refusals are never offered, a connector direction only counts when it leads somewhere walkable, the keypad row only appears outside the room, and the terminal origin comes from `access::district_of_terminal` or the rig. `Pick::Look` and `Pick::Talk` are handled in `Popup::fire`. Talk's lines are today's `card_lines` without the name line (the header names them). `said` is `format!("{name}: \u{201c}{line}\u{201d}")`.

- [ ] **Step 4: Rewire `MapScreen::interact`**

```text
construct with a deck  → Popup::verbs("", [Look(pos)] + run_rows(..)); header only when every Run row has the same target
no subjects            → Popup::verbs("", [Look(player pos)])
one subject            → subject.window(tick)
several                → Task 3 (until then, open the first; Task 3 replaces this arm)
```

`MapScreen::update` starts by taking `st.look_at`. Until Task 6 lands, it drops the value.

- [ ] **Step 5: Update the Space-window tests in `explore.rs` to the new wording**

These are the tests at the old lines 2852 (take all), 2946 (go down), 3569, 3573, 3657, 3662 and 4464-4470 (talk), 3906-3945 (terminal and rig), 4304-4308, 4667-4672 (keypad), 4806-4811 (stairs), 5022 and 5333 (RUN). Assert the new row text: `"g  Take all"`, `"<  Go down: Undercity"`, `"t  Talk"` with the name in the header, `"i  Interact: Terminal"`, `"j  Jack in"`, `"i  Interact: Keypad"`, `"Go up: Level 1"`, `"Crack: "`. HERE and RUN must no longer appear. Firing Talk then Esc returns `Pop`.

- [ ] **Step 6: Run the crate's tests**

Run: `cargo test -p strata-zero`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add -A crates/strata-zero
git commit -m "feat(app): the space window is about one subject, with fixed verbs and letters"
```

---

### Task 3: Which way

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs`
- Modify: `crates/strata-zero/src/render/path.rs` (`BlockOverlay`)
- Test: `crates/strata-zero/src/screens/explore.rs`, `crates/strata-zero/src/render/path.rs`

**Interfaces:**
- Consumes: `gather`, `Subject`, `Subject::window` (Task 2).
- Produces:
  - `pub struct WhichWay { pub subjects: Vec<Subject> }` and `MapScreen::which_way: Option<WhichWay>`.
  - `pub struct BlockOverlay<'a> { pub tiles: &'a [Pos], pub bg: Color }` keeps each cell's glyph and fg and sets bg. Which-way uses `WAY_BG = Color::Rgb(0, 45, 55)`; Task 6's cursor uses `LOOK_BG`.
  - `turn_prompt` returns `("WHICH WAY?", "↑ Mara Chen   → Access terminal   · here: 2 items")` while `which_way` is up. Arrows come from `Dir` (`↑ ↗ → ↘ ↓ ↙ ← ↖`); here reads "· here: {name}".

Keys while `which_way` is up (it takes every key, like `targeting`):
- A direction (`dir_for`) with a subject: `which_way = None` and push that subject's window.
- A direction without a subject: nothing happens.
- Space or `.`: the here subject's window, if there is one.
- Esc: `which_way = None`.
- Anything else is ignored.

It is dropped when the player's place changes. The world keeps running.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn several_subjects_ask_which_way_and_a_direction_opens_one() {
    // Beside a terminal (east) with a person to the north.
    assert!(matches!(s.update(&mut st, &[key(KeyCode::Char(' '))]), ScreenCmd::Stay));
    assert!(s.which_way.is_some());
    let text = draw_screen(&s, &st, 120, 40);
    assert!(text.contains("WHICH WAY?") && text.contains("Access terminal"), "{text}");
    let ScreenCmd::Push { to: w, .. } = s.update(&mut st, &[key(KeyCode::Up)]) else { panic!("up opens the person") };
    assert!(s.which_way.is_none());
    st.tick += 40;
    assert!(draw_screen(&*w, &st, 120, 40).contains(&person_name));
}

#[test]
fn which_way_opens_the_direction_and_ignores_empty_ones() {
    // Same setup: a direction with nothing stays in the pick; Esc leaves it.
    s.update(&mut st, &[key(KeyCode::Char(' '))]);
    assert!(matches!(s.update(&mut st, &[key(KeyCode::Char('b'))]), ScreenCmd::Stay));
    assert!(s.which_way.is_some());
    s.update(&mut st, &[key(KeyCode::Esc)]);
    assert!(s.which_way.is_none());
    assert!(!matches!(last(&st), Some(t) if t.contains("pause")));
}

#[test]
fn one_subject_opens_at_once_and_none_opens_look_only() { /* terminal alone → Push with "Access terminal"; empty sidewalk → Push whose rows are exactly ["x  Look"] and blank header */ }

#[test]
fn block_overlay_keeps_the_glyph_and_sets_the_background() { /* in path.rs tests */ }
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p strata-zero which_way block_overlay`
Expected: compile errors because `which_way` and `BlockOverlay` are missing.

- [ ] **Step 3: Implement `WhichWay`, its keys in `update` (before `handle_common`, beside the `targeting` branch), the overlay in `draw` (the subjects' positions, here included), and the prompt in `turn_prompt`**

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p strata-zero`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A crates/strata-zero
git commit -m "feat(app): several things in reach ask which way, a direction opens one"
```

---

### Task 4: Space acts, Enter confirms; `J` jacks out

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs` (`handle_walk_inner`, the not-your-turn guard, tests)
- Modify: `crates/strata-zero/src/net.rs` (`jack_in_here`)
- Modify: `crates/strata-zero/src/screens/popup.rs` (`world` uses `jack_in_here`)
- Modify: `crates/strata-zero/src/screens/help.rs`, `README.md`
- Test: `crates/strata-zero/src/screens/explore.rs`

**Interfaces:**
- Consumes: `Popup::jack_out_confirm` and `Pick::JackOutAsk` (Task 2).
- Produces: `pub fn jack_in_here(st: &mut AppState) -> Result<String, String>` in `net.rs`. It runs the refusal check, `player.jack_in` and `jacked_in_line`; it is the body of `Popup::world`'s `JackIn` arm, which now calls it.

Behaviour:
- `handle_walk_inner` loses its three Enter-jack branches (the refusal warning, the jack out, and the `KeyCode::Enter =>` jack in).
- `J` while jacked in pushes `Popup::jack_out_confirm`. When not jacked in, it warns "not jacked in". It sits after the not-your-turn guard, so on the ICE's turn it warns "not your turn".
- The guard's free keys gain `'x'`, ready for Task 6.
- Enter keeps walking the construct aim and confirming targeting.
- Help: drop `("enter", "jack in at an adjacent terminal / jack out")` and add `("J", "jacked in: jack out (asks first)")`. Change the `g` line's "enter walks" to "space or enter walks".
- README: the "press `enter`" jack lines (88-90) say Space and `j` to jack in and `J` to jack out. Line 239's "`enter` jack shortcut" goes.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn enter_no_longer_jacks_in() {
    // Beside a terminal, walking: Enter changes nothing.
    s.update(&mut st, &[key(KeyCode::Enter)]);
    assert!(!st.player.as_ref().unwrap().jacked_in());
}

#[test]
fn j_asks_before_jacking_out_and_n_stays() {
    // Jacked in on the grid.
    let ScreenCmd::Push { to: mut ask, .. } = s.update(&mut st, &[key(KeyCode::Char('J'))]) else { panic!("J asks") };
    st.tick += 40;
    assert!(draw_screen(&*ask, &st, 120, 40).contains("Jack out?"));
    assert!(matches!(ask.update(&mut st, &[key(KeyCode::Char('n'))]), ScreenCmd::Pop { .. }));
    assert!(st.player.as_ref().unwrap().jacked_in());
    let ScreenCmd::Push { to: mut ask, .. } = s.update(&mut st, &[key(KeyCode::Char('J'))]) else { panic!() };
    ask.update(&mut st, &[key(KeyCode::Enter)]);
    assert!(!st.player.as_ref().unwrap().jacked_in());
    assert_eq!(last(&st), Some("jacked out"));
}

#[test]
fn j_jacks_out_mid_fight_with_a_run_armed() {
    // Engaged in a construct, your act half, a run armed (as in the engagement tests).
    let ScreenCmd::Push { to: mut ask, .. } = s.update(&mut st, &[key(KeyCode::Char('J'))]) else { panic!("J asks mid-fight") };
    ask.update(&mut st, &[key(KeyCode::Char('J'))]);
    assert!(!st.player.as_ref().unwrap().jacked_in());
}

#[test]
fn j_on_the_ice_turn_says_not_your_turn() { /* engaged, ICE actor: J → Stay, last warning "not your turn" */ }

#[test]
fn the_space_window_jack_out_row_asks_too() { /* jacked in on the grid, Space → window with "j  Jack out"; firing j → ScreenCmd::Replace to the confirm */ }
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p strata-zero jack`
Expected: `enter_no_longer_jacks_in` fails because Enter still jacks; the `J` tests fail because `J` does nothing.

- [ ] **Step 3: Implement the key changes, `jack_in_here`, help and README**

- [ ] **Step 4: Migrate the tests that pressed Enter on the map**

Go through the `KeyCode::Enter` presses in `explore.rs` tests (old lines 2520 to 8388, about 25 of them). A press that jacked in becomes `net::jack_in_here(&mut st).unwrap();` followed by `s.update(&mut st, &[])`, so the map follows. A press that jacked out becomes `crate::net::jack_out(&mut st).unwrap();` followed by `s.update(&mut st, &[])`. Presses that walk an aim or confirm a target stay. A test whose point was "Enter jacks" is rewritten to Space then `j` (in) or `J` then `y` (out).

- [ ] **Step 5: Run the whole workspace's tests and clippy**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 6: Commit**

```bash
git add -A crates/strata-zero README.md
git commit -m "feat(app): space acts and enter confirms; J jacks out, after asking"
```

---

### Task 5: The look card and its words

**Files:**
- Create: `crates/strata-zero/src/look.rs`, `flavor/looks.ron`
- Test: `crates/strata-zero/src/look.rs`

**Interfaces:**
- Consumes: `mark` (Task 2), `hud::location`, `World::card`, `credentials::{flat_label, carries}`, `Player::vis`.
- Produces:

```rust
pub struct LookCard {
    pub glyph: Option<(char, Color)>,
    pub header: Option<String>,
    pub header_fg: Color,
    pub second: Option<String>,
    pub body: Vec<String>,
    pub also: Vec<String>,
    /// Space would open a window for it from where the player stands.
    pub hint: bool,
}
pub fn look_card(st: &AppState, map: &MapRef, place: Place, pos: Pos,
                 inside_person: &dyn Fn(Place, Pos) -> Option<sim::PersonId>,
                 reachable: &[Pos]) -> LookCard;
pub struct Looks { /* terrain, feature, furniture, building, fixture, node: BTreeMap<String, Vec<String>> */ }
impl Looks { pub fn load() -> Looks; pub fn line(&self, group: Group, key: &str, seed: u64, pos: Pos) -> Option<&str>; }
pub enum Group { Feature, Furniture, Building, Fixture, Node }
```

`Looks::load` parses `include_str!("../../../flavor/looks.ron")` once, in a `std::sync::OnceLock`. `line` picks `lines[hash64(seed, pos) % len]`. Keys: `Furniture::label()`, `BuildingKind::label()`, node `kind.label()`, and for features these strings: `"lamp" "neon" "holo_ad" "vent" "puddle" "trash" "barrier" "stall" "sign" "camera" "ac_unit" "helipad" "pipe" "cable" "awning"`. Fixtures: `"terminal" "rig" "stairwell" "lift" "stairs" "ladder" "manhole" "flat_door"`. Write one to three lines per key in the voice of `flavor/content.ron`'s `desc`. The street lamp's first line is "Sodium light and a hum you stop hearing." and the terminal's is "A municipal box bolted to the wall, its screen scratched to fog." Others follow the spec's examples.

Rules, in this order:
1. **Visibility** (`Player::vis`, where a missing grid means `Visible`):
   - `Unseen`: header `None`, body `["You can't see there."]`, nothing else.
   - `Remembered`: the tile's header (rule 3, from fixtures down) and second `"as you last saw it"`, with no body, no people, no items and no hint.
2. **Ranking** on the tile: a person (never the player), then a fixture, then the top item stack, then a feature, then the ground or building. The header is the first. `also` names the rest of the people and items, plus the fixture when something outranks it.
3. **Per kind:**
   - person: name, `"{profession} · {doing}"`, body `["A keycard swings on a lanyard at their belt."]` when `carries` is non-empty.
   - item: `"Name ×n"`, `"{kind label lowercase} · on the ground"`, body the item's `desc`.
   - flat door: `flat_label` capitalised, `"front door · locked, keypad"`, or `"front door · unlocked"` when `credentials::opens` lets the player in, or `"front door · keypad locked Nm"`.
   - district terminal: "Access terminal", `"public · {district name}"`.
   - rig: "Access rig", `"your rig"`.
   - indoor stairs: "Stairwell", `"up to {L+1} · down to {L-1}"`, leaving out a missing side.
   - lift bank: "Lift", `"{lowest label} to {top label}"`.
   - strata connector: "Stairs", "Lift", "Ladder" or "Manhole", with `"up to {stratum} · down to {stratum}"`.
   - feature: its name ("Street lamp", "Neon sign", "Holo ad"), second none, body from `Looks`.
   - building tile on a stratum (Wall, Window or Mass with `tile.building`): the name, or the kind label capitalised; `"{kind} · {district}"`; body from `Looks`.
   - ground: `Terrain::label()` capitalised, second `hud::location(..)`, no body.
   - the player's own tile with nothing else: "You", second `hud::location(..)`.
   - node: its kind label capitalised, `"node"`.
   - ICE: `"{Grade} ICE"`, with second `"broken"` or `"standing"` from the guard.
4. **`hint`** is true when `pos` is in `reachable`.
5. **Colours:** `glyph` comes from `mark` for the ranked thing, and `header_fg` is its colour (`TEXT` for "You").

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_person_reads_name_job_doing_and_never_a_number() { /* header == card.name; second == format!("{} · {}", card.profession, card.doing); no digit anywhere in the card */ }

#[test]
fn an_item_reads_its_desc() {
    // A stim patch ×2 dropped on a visible tile.
    let c = look_card(&st, &map, place, pos, &|_, _| None, &[]);
    assert_eq!(c.header.as_deref(), Some("Stim patch ×2"));
    assert_eq!(c.second.as_deref(), Some("consumable · on the ground"));
    assert_eq!(c.body, ["Twelve minutes of clarity, then the bill."]);
}

#[test]
fn remembered_tiles_hide_people_and_items() { /* mark the tile Remembered with a person and a stack on it: header is the ground's, second "as you last saw it", also empty, body empty */ }

#[test]
fn unseen_tiles_say_one_line() { /* header None, body == ["You can't see there."] */ }

#[test]
fn your_own_tile_shows_what_else_is_there() { /* on stairs indoors: header "Stairwell"; on bare sidewalk: header "You" */ }

#[test]
fn every_flavour_line_is_house_voice() {
    let l = Looks::load();
    for line in l.all_lines() {
        assert!(!line.chars().any(|c| c.is_ascii_digit()), "{line}");
        assert!(!line.starts_with("You see"), "{line}");
        assert!(line.ends_with('.'), "{line}");
    }
    for k in ["lamp", "neon", "holo_ad", "terminal", "stairwell", "flat_door"] {
        assert!(l.has(k), "{k}");
    }
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p strata-zero look::`
Expected: compile errors because `look` is missing.

- [ ] **Step 3: Write `flavor/looks.ron`, then implement `Looks`, `LookCard` and `look_card`**

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero look::`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A crates/strata-zero/src/look.rs flavor/looks.ron crates/strata-zero/src
git commit -m "feat(app): the look card: what your eyes can tell you, in the house voice"
```

---

### Task 6: Look mode

**Files:**
- Modify: `crates/strata-zero/src/look.rs` (`place_box`, `draw_box`, `points_of_interest`)
- Modify: `crates/strata-zero/src/screens/explore.rs`
- Test: `crates/strata-zero/src/look.rs`, `crates/strata-zero/src/screens/explore.rs`

**Interfaces:**
- Consumes: `look_card` (Task 5), `BlockOverlay` (Task 3), `AppState::look_at` (Task 2), `gather` (Task 2), the `'x'` free key (Task 4).
- Produces:
  - `pub struct Looking { pub cursor: Pos, pub at: usize }` and `MapScreen::looking: Option<Looking>`. `at` is the Tab index.
  - `pub const LOOK_W: u16 = 48;` and `pub const LOOK_BG: Color = Color::Rgb(90, 70, 0);` (amber-dark, distinct from `WAY_BG`).
  - `pub fn place_box(map: Rect, cursor: (u16, u16), h: u16) -> Rect`:
    - if `map.width >= 2 * LOOK_W + 2`, the top-right corner when `cursor.0 < map.x + map.width / 2`, else top-left;
    - otherwise full width, at the top when the cursor is in the bottom half, else at the bottom;
    - clamped inside `map`.
  - `pub fn draw_box(card: &LookCard, r: Rect, tick: u64, buf: &mut Buffer)` uses the popup frame: header on the top border, second line `DIM`, a blank row, the body wrapped in `TEXT`, a blank row, then `also here: …` `DIM` and `space · interact` `DIM`. `pub fn box_height(card, width) -> u16`.
  - `pub fn points_of_interest(st, map, place, view: TileRect, from: Pos) -> Vec<Pos>` returns `Visible` tiles in `view` with a person, a fixture, an item pile, or a lamp, neon, sign or holo ad feature, sorted by Chebyshev distance from `from`, then y, then x.

Keys while `looking` (it takes every key, checked before `handle_common`):
- Walk keys move the cursor one tile; Shift or upper-case `H J K L Y U B N` move it 8. The cursor is clamped to the visible map rect (`cam.origin` plus `last_map`) and to the map's size.
- Tab and BackTab step through `points_of_interest` from the player's position.
- Space: when a subject from `gather` sits at the cursor, set `looking = None` and push its window. A construct target in reach opens the program window for it.
- Esc or `x`: `looking = None`.

Entering:
- `x` in `handle_walk_inner` puts the cursor on the player.
- `st.look_at.take()` at the top of `update` puts it on that tile.

`focus_pos` returns the look cursor while looking. `turn_prompt` returns `("LOOK", "arrows move · tab next · esc back")`. `draw` pushes `BlockOverlay { tiles: &[cursor], bg: LOOK_BG }`, pulsed with `pulse(st.tick, 30)` scaling the bg, and draws the box after the map and before the help overlay.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn look_box_never_covers_the_cursor() {
    for map in [Rect::new(0, 5, 100, 30), Rect::new(0, 5, 60, 20)] {
        for y in map.y..map.bottom() {
            for x in map.x..map.right() {
                let r = place_box(map, (x, y), 8);
                assert!(!(r.x <= x && x < r.right() && r.y <= y && y < r.bottom()), "{map:?} {x},{y} {r:?}");
                assert!(r.right() <= map.right() && r.bottom() <= map.bottom());
            }
        }
    }
}

#[test]
fn x_looks_and_esc_or_x_puts_it_away() {
    s.update(&mut st, &[key(KeyCode::Char('x'))]);
    assert_eq!(s.looking.as_ref().map(|l| l.cursor), Some(player_pos));
    s.update(&mut st, &[key(KeyCode::Char('l'))]);
    assert_eq!(s.looking.as_ref().unwrap().cursor, player_pos.offset(1, 0));
    assert_eq!(player(&st).1, player_pos, "looking never moves you");
    let text = draw_screen(&s, &st, 120, 40);
    assert!(text.contains("LOOK") && text.contains("esc back"), "{text}");
    s.update(&mut st, &[key(KeyCode::Esc)]);
    assert!(s.looking.is_none());
}

#[test]
fn look_from_a_window_starts_on_its_subject() { /* open the terminal window, fire x → Pop; next update: looking.cursor == terminal tile; the screen shows "Access terminal" and "space · interact" */ }

#[test]
fn space_in_look_opens_the_subject_window() { /* cursor on the person beside you → Space → Push whose header is their name; looking is None */ }

#[test]
fn tab_walks_the_points_of_interest_nearest_first() { /* two lamps in view at distance 3 and 6: Tab → first, Tab → second */ }

#[test]
fn look_is_free_on_the_ice_turn() { /* engaged, ICE actor: x enters look; no "not your turn" warning */ }
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p strata-zero look`
Expected: compile errors because `place_box` and `looking` are missing.

- [ ] **Step 3: Implement `place_box`, `box_height`, `draw_box`, `points_of_interest`, then `Looking` and its keys, entering it, `focus_pos`, the prompt and the drawing**

- [ ] **Step 4: Run the whole workspace's tests and clippy**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 5: See it in the real game**

Build to the smoke target (never `target/release`): `cargo build --release --target-dir target/smoke`. Drive it in tmux at 120×40: walk beside a terminal, press Space, then `x`, then Tab, and capture each screen. Check the box sits away from the cursor, the cursor block is visible, and the wording matches the spec.

- [ ] **Step 6: Commit**

```bash
git add -A crates/strata-zero
git commit -m "feat(app): look: a cursor, an info window that keeps out of its way"
```

---

## After the tasks

- Update the spec's Status line to "built on `feat/interaction`", plus any rulings made while building.
- Whole-branch review by a fresh reviewer, then present the finishing-branch options (integration is Jeffrey's call).
