# Sockets on the Ratchet SMG Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Ratchet SMG gets a blueprint drawing with a muzzle and a rail socket, two
pieces of weapon hardware (Hushcan, Redeye sight) fit in them through the existing
generic modding mode, and D'SHAWN carries both.

**Architecture:** The game crate learns two socket kinds, a stat mod, an item look
that places sockets, and counts hardware fitted in anything worn. The UI gets a pixel
canvas (half blocks), an `Art` for items drawn by an `ItemLook` (the Ratchet), the
weapon pieces' large and in-socket drawings, one generic arm in the blueprint window,
and a tray that offers only the chosen socket's kind.

**Tech Stack:** Rust 2024, ratatui (TestBackend for tests), serde + RON content,
`unicode-width` in tests.

**Spec:** `docs/superpowers/specs/2026-10-19-smg-sockets-design.md`. Art reference:
`docs/superpowers/specs/2026-10-19-smg-sockets-art.py` (run it with `python3` to print
every drawing's cells; it is the truth for pixels, colours and glyphs).

## Global Constraints

- Everything fits 100×30; every glyph single width (`unicode-width` 1); every colour
  `Color::Rgb` (or `Reset`), exact hex from the art reference.
- No code in `screens/bench.rs` or `screens/blueprint.rs` names the gun, a socket
  kind, or a piece; wording comes from `SocketKind::takes`/`fits`, `Mod::line`, item
  names.
- Minus signs in mod text are U+2212, as `Mod` already writes them.
- Comments and doc comments read like the surrounding code: plain sentences, no task
  numbers.
- Run tests with `cargo test -q` at the workspace root; `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo fmt --check` stay clean after each task.
- Never build `target/release/strata` (Jeffrey may be playing it). Shots and any
  release builds use `--target-dir target/smoke`.
- Commit after each task with a conventional message ending in:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_014zwgC815q3KaJsaiBuvToz
  ```

## Review Focus

- A Ratchet held without a copy (content `give` without `give_item`, old tests):
  Inspect draws the bare gun and never offers `m`. Test in Task 4.
- The deck's tray must never offer weapon hardware, choosing or not, and the gun's
  never deck hardware. Test in Task 5.
- A taller terminal (120×40): the gun stays centred and the tray band still sits
  wholly below it. Test in Task 4.
- A fitted Hushcan travels with the gun: its card's weight reads 3.7 kg and EMPTY on
  a filled MUZZLE takes it out to the pack. Test in Task 5.
- Jacked in, space on a gun socket refuses with the jacked-in line and nothing
  changes. Test in Task 5.

---

### Task 1: Socket kinds, stat mod, item looks, content and validation (`game`)

**Files:**
- Modify: `crates/game/src/model.rs` (SocketKind, Mod, HardwareLook, DeckLook, new ItemLook, Item)
- Modify: `crates/game/src/content.rs` (validation, `ContentError::LookInvalid`, tests)
- Modify: `crates/game/src/action.rs:102-107` (`socket_name`)
- Modify: `crates/game/src/lib.rs` (re-export `ItemLook` beside the other model types)
- Modify: `flavor/content.ron` (ratchet_smg, hushcan, redeye)
- Modify: `crates/strata-zero/src/ui/cyberdeck/pawn.rs:56-76` (names and kinds from `DeckLook::sockets`)
- Modify: `crates/strata-zero/src/ui/hardware.rs` (only what the new `HardwareLook` variants need to compile, see Step 5)

**Interfaces:**
- Produces:
  - `SocketKind::{Deck, Muzzle, Rail}`; `takes()` → `"muzzle hardware"` / `"rail hardware"`; `fits()` → `"muzzle socket"` / `"rail socket"`.
  - `Mod::Stat(Stat, i8)`; `line()` → `"cool +1"`; `sticker()` → `"COOL +1"`.
  - `HardwareLook::{ColdfinI, ColdfinII, Sidecar, Hushcan, Redeye}`; `HardwareLook::kind(self) -> SocketKind`.
  - `ItemLook::Ratchet` (derives as `DeckLook`); `ItemLook::sockets(self) -> &'static [(&'static str, SocketKind)]` → `[("MUZZLE", Muzzle), ("RAIL", Rail)]`.
  - `DeckLook::sockets(self) -> &'static [(&'static str, SocketKind)]` → Pawn `[("L", Deck), ("R", Deck)]`; `socket_names` is removed.
  - `Item.look: Option<ItemLook>` (`#[serde(default)]`).
  - `Item::drawn_sockets(&self) -> &'static [(&'static str, SocketKind)]`: the deck look's places when `deck` is `Some`, else `look`'s, else `&[]`.
  - `ContentError::LookInvalid { id: String, why: String }`.

- [ ] **Step 1: Write the failing tests** in `crates/game/src/content.rs` tests:

```rust
#[test]
fn weapon_sockets_and_hardware_load() {
    let c = Content::load().unwrap();
    let get = |id: &str| c.item(&ItemId::new(id)).unwrap();
    let smg = get("ratchet_smg");
    assert_eq!(smg.look, Some(ItemLook::Ratchet));
    assert_eq!(smg.sockets, vec![SocketKind::Muzzle, SocketKind::Rail]);
    assert!(smg.per_copy());
    assert_eq!(smg.drawn_sockets()[0], ("MUZZLE", SocketKind::Muzzle));
    assert_eq!(get("rebuilt_deck").drawn_sockets()[1], ("R", SocketKind::Deck));
    assert!(get("salvage_jacket").drawn_sockets().is_empty());
    for (id, name, weight, value, m, fits, look) in [
        ("hushcan", "Hushcan", 0.5, 700, Mod::Stat(Stat::Cool, 1), SocketKind::Muzzle, HardwareLook::Hushcan),
        ("redeye", "Redeye sight", 0.2, 550, Mod::Stat(Stat::Reflex, 1), SocketKind::Rail, HardwareLook::Redeye),
    ] {
        let i = get(id);
        assert_eq!((i.kind, i.name.as_str(), i.weight, i.value), (ItemKind::Hardware, name, weight, value));
        assert_eq!(i.hardware, Some(Hardware { fits, mods: vec![m], look }), "{id}");
        assert_eq!(look.kind(), fits);
    }
}
```

Extend `mod_lines_read_as_the_card_does` with: `Mod::Stat(Stat::Cool, 1).line() == "cool +1"`, `.sticker() == "COOL +1"`, `Mod::Stat(Stat::Reflex, -1).line() == "reflex \u{2212}1"`, `.sticker() == "REFLEX \u{2212}1"`, `SocketKind::Muzzle.takes() == "muzzle hardware"`, `SocketKind::Rail.fits() == "rail socket"`.

Extend `validation_refuses_bad_hardware_and_sockets` with, each on a fresh clone:
- `ratchet_smg.sockets = vec![Rail, Muzzle]` → `SocketsInvalid` (kind out of place);
- `ratchet_smg.sockets = vec![Muzzle, Rail, Rail]` → `SocketsInvalid`;
- `ratchet_smg.look = None` (sockets kept) → `SocketsInvalid`;
- `hushcan.hardware.fits = Rail` → `HardwareInvalid` (look drawn for another kind);
- `rebuilt_deck.look = Some(Ratchet)` → `LookInvalid`; `coldfin_1.look = Some(Ratchet)` → `LookInvalid`.
And one that passes: `ratchet_smg.sockets = vec![Muzzle]` validates (fewer than the places, in kind).

In `crates/game/src/action.rs` tests add: `socket_name(&c, &ItemId::new("ratchet_smg"), 1) == "RAIL"`, `socket_name(&c, &ItemId::new("rebuilt_deck"), 0) == "L"`, `socket_name(&c, &ItemId::new("salvage_jacket"), 0) == "1"`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -q -p game`
Expected: compile errors (no `ItemLook`, `Mod::Stat`, `SocketKind::Muzzle`, ...).

- [ ] **Step 3: Implement the model and content**

In `model.rs` the types and functions of the Interfaces block. `Mod::line` for `Stat(s, n)` is `format!("{} {}", s.label(), signed(*n))`, `sticker` `format!("{} {}", s.name(), signed(*n))`. `HardwareLook::kind`: Coldfins and Sidecar `Deck`, Hushcan `Muzzle`, Redeye `Rail`.

In `content.rs` validation replace the count-only socket check with: the item's sockets must be no more than `drawn_sockets()` and kind for kind equal to its first places (`SocketsInvalid`, with a `why` naming the problem, e.g. `"its drawing places no socket 3"`, `"socket 1 is a rail socket on its drawing"`). Add: hardware whose `fits != look.kind()` → `HardwareInvalid` `"its look is drawn for a {} "` + `look.kind().fits()`; `look` on an item with `deck` or `hardware` → `LookInvalid` `"a deck or hardware wears its own look"`. Add `LookInvalid` to `ContentError` with its message in the same style as its neighbours.

In `action.rs` `socket_name` reads `drawn_sockets().get(socket)`'s name, falling back to `socket + 1`.

In `content.ron`: `ratchet_smg` gains `look: Some(Ratchet), sockets: [Muzzle, Rail]`; after `sidecar` add, with the texts verbatim from the spec:

```ron
Item(id: "hushcan", name: "Hushcan", kind: Hardware, weight: 0.5, value: 700,
     hardware: Some(Hardware(fits: Muzzle, mods: [Stat(Cool, 1)], look: Hushcan)),
     desc: "A baffle can that threads onto a muzzle. The shot still happens; the street just takes longer to turn round."),
Item(id: "redeye", name: "Redeye sight", kind: Hardware, weight: 0.2, value: 550,
     hardware: Some(Hardware(fits: Rail, mods: [Stat(Reflex, 1)], look: Redeye)),
     desc: "A red dot on coated glass. You stop aiming and start pointing."),
```

- [ ] **Step 4: Run the game tests**

Run: `cargo test -q -p game`
Expected: all pass. Tests elsewhere that assumed `ratchet_smg` stacks (it is now held per copy) are fixed to the copy model, not by reverting content.

- [ ] **Step 5: Keep the UI compiling**

`pawn.rs::sockets` takes each place's name and kind from `DeckLook::Pawn.sockets()` (no hard-coded `SocketKind::Deck`). In `ui/hardware.rs`: `native` returns `Option<…>` and is `None` for `Hushcan` and `Redeye` (they have no deck side drawing; `draw_side` then draws nothing); `tile` gets the spec's tiles, Hushcan `Tile { bg: #3a3f47, ink: #e8ecf0, swatch: (#3a3f47, #1c1f24) }`, Redeye `Tile { bg: #5a0c14, ink: #ffd8dc, swatch: (#ff2a3c, #15171b) }`; `draw_large` gets an empty arm for `Hushcan | Redeye`, which Task 3 fills.

- [ ] **Step 6: Run everything**

Run: `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, no warnings.

- [ ] **Step 7: Commit** — `feat(game): muzzle and rail sockets, the stat mod, the Ratchet's look and two weapon pieces`

---

### Task 2: Hardware in anything worn counts; D'SHAWN's pieces (`game`)

**Files:**
- Modify: `crates/game/src/character.rs` (`deck_mods` → `worn_mods`, `stat`, `debug_runner`, tests)
- Modify: every caller of `deck_mods` (grep; today only `character.rs`)

**Interfaces:**
- Consumes: Task 1's `Mod::Stat`, content `hushcan`, `redeye`.
- Produces: `Character::worn_mods(&self, c: &Content) -> Vec<Mod>` (every worn copy's fitted pieces' mods, slots in `Slot::ALL` order, sockets in order); `trace_cut`, `deck_power` unchanged in signature; `stat` counts `Mod::Stat`.

- [ ] **Step 1: Write the failing tests** in `character.rs` tests. A helper `fit_worn(ch, slot, item, socket)` generalising `fit_direct` (which keeps working for the deck).

```rust
#[test]
fn hardware_in_any_worn_item_counts() {
    let c = Content::load().unwrap();
    let mut vex = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
    let (cool, reflex) = (vex.stat(&c, Stat::Cool), vex.stat(&c, Stat::Reflex));
    vex.give_item(&c, ItemId::new("hushcan"), 1);
    vex.give_item(&c, ItemId::new("redeye"), 1);
    assert_eq!(vex.stat(&c, Stat::Cool), cool, "in the pack it does nothing");
    fit_worn(&mut vex, Slot::Primary, "hushcan", 0);
    assert_eq!(vex.stat(&c, Stat::Cool), cool + 1);
    fit_worn(&mut vex, Slot::Primary, "redeye", 1);
    assert_eq!(vex.stat(&c, Stat::Reflex), reflex + 1);
    assert_eq!((vex.trace_cut(&c), vex.deck_power(&c)), (0, 0));
    // The gun off the body: its pieces count for nothing.
    let mut off = vex.clone();
    off.equipment.remove(&Slot::Primary);
    assert_eq!(off.stat(&c, Stat::Cool), cool);
}

#[test]
fn dshawn_carries_the_weapon_pieces_and_stays_capped() {
    let c = Content::load().unwrap();
    let mut d = Character::debug_runner(&c);
    let gun = d.worn_copy(Slot::Primary).expect("the Ratchet, worn as a copy");
    assert_eq!(gun.item, ItemId::new("ratchet_smg"));
    assert_eq!(gun.fitted, vec![None, None]);
    assert_eq!((d.count(&ItemId::new("hushcan")), d.count(&ItemId::new("redeye"))), (1, 1));
    assert_eq!(d.stat(&c, Stat::Cool), 10);
    fit_worn(&mut d, Slot::Primary, "hushcan", 0);
    assert_eq!(d.stat(&c, Stat::Cool), 10, "the clamp holds");
}
```

Also assert Vex's start wears the Ratchet as a copy with `fitted == vec![None, None]`. The existing `deck_mods_come_from_the_worn_deck` keeps passing (rename it if it names `deck_mods`).

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p game character::` → FAIL (no hushcan in D'SHAWN's pack; Cool unchanged when fitted).

- [ ] **Step 3: Implement** `worn_mods` (replaces `deck_mods`; the doc comment says hardware in anything worn counts, in the pack nothing does), `trace_cut`/`deck_power` read it, `stat` adds the `Mod::Stat` sum for that stat to the perks' bonus inside the existing clamp. `debug_runner` gives `hushcan` and `redeye` after the deck hardware; its doc comment mentions the weapon pieces.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** — `feat(game): hardware counts in anything worn; stat mods; D'SHAWN carries the Hushcan and the Redeye`

---

### Task 3: The pixel canvas and the weapon pieces' drawings (`strata-zero`)

**Files:**
- Create: `crates/strata-zero/src/ui/pixels.rs` (half-block pixel canvas), register in `ui/mod.rs`
- Move: `crates/strata-zero/src/ui/hardware.rs` → `crates/strata-zero/src/ui/hardware/mod.rs` (`git mv`, no other change in that step)
- Create: `crates/strata-zero/src/ui/hardware/weapon.rs` (Hushcan and Redeye: large, and in their sockets)
- Modify: `crates/strata-zero/src/ui/hardware/mod.rs` (sizes per look, `draw_large` arms, `footprint`, `draw_fitted`, tests)

**Interfaces:**
- Consumes: Task 1's `HardwareLook::{Hushcan, Redeye}`, `SocketKind`, `Mod::sticker`; `Pen` (`ui/cyberdeck/mod.rs`): `cell(x: i32, y: i32, ch, fg, bg: Option<Color>)`, `put(x: u16, y: u16, text, fg, bg: Option<Color>, bold)`; `SocketPlace` (`ui/art.rs`).
- Produces:
  - `pub(crate) struct Pixels` in `ui/pixels.rs`: `Pixels::new(w: u16, rows: u16)` (cells; twice `rows` pixels high), `px(&mut self, x: i32, y: i32, c: Option<Color>)`, `rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color)` (half-open), `hline(x0, x1, y, c)`, `vline(x, y0, y1, c)`, `fold(&self, p: &mut Pen, x: i32, y: i32)` (writes the cells with the canvas's top-left at pen cell `(x, y)`).
  - `pub fn large_size(look: HardwareLook) -> (u16, u16)` in `ui/hardware/mod.rs`: Coldfins and Sidecar `(52, 18)`, Hushcan `(72, 14)`, Redeye `(52, 18)`; `HardwareArt::size` returns it (the `W`/`H` constants go or become the deck pieces' size).
  - `pub fn footprint(kind: SocketKind) -> Option<(i32, i32, u16, u16)>`: where a piece of that kind draws inside its socket place, as offset and size: Muzzle `(4, 0, 18, 5)`, Rail `(0, 0, 24, 3)`, Deck `None` (the deck draws its own sides).
  - `pub fn draw_fitted(look: HardwareLook, place: &SocketPlace, p: &mut Pen)`: draws the piece in `place`'s footprint; nothing for a deck socket.

- [ ] **Step 1: Write the failing tests**

In `ui/pixels.rs`: folding follows the reference rule. A 2×1 canvas with pixel (0,0) red, (0,1) red, (1,0) red, (1,1) `None` folds to `(" ", red, red)` and `("▀", red, <the buffer's bg untouched>)`; a top `None` over blue gives `"▄"` fg blue with the bg untouched; red over blue gives `"▀"` fg red bg blue; an all-`None` cell is not written.

In `ui/hardware/mod.rs` tests (cells from running the art reference; `large(look, mods)` as today but sized by `large_size`):

```rust
#[test]
fn hushcan_large_matches_the_reference() {
    let b = large(HardwareLook::Hushcan, &[Mod::Stat(Stat::Cool, 1)]);
    assert_eq!((b[(17, 2)].symbol(), b[(17, 2)].fg, b[(17, 2)].bg), ("H", rgb(0x6b7480), rgb(0x1c1f24)));
    assert_eq!(text(&b, 52, 2, 9), " COOL +1 ");
    assert_eq!(b[(53, 2)].bg, rgb(0xe8e8ec));
    assert_eq!((b[(0, 7)].symbol(), b[(0, 7)].fg, b[(0, 7)].bg), ("▀", rgb(0x8d939c), rgb(0xe8c54a)));
    assert_eq!(b[(1, 8)].bg, rgb(0x8a6d14));
    assert_eq!(text(&b, 26, 13, 14), "▲ BAFFLE STACK");
    assert_eq!(text(&b, 0, 13, 8), "▲ THREAD");
}

#[test]
fn redeye_large_matches_the_reference() {
    let b = large(HardwareLook::Redeye, &[Mod::Stat(Stat::Reflex, 1)]);
    assert_eq!(text(&b, 23, 7, 6), "REDEYE");
    assert_eq!(text(&b, 37, 5, 11), " REFLEX +1 ");
    assert_eq!((b[(31, 10)].symbol(), b[(31, 10)].fg), ("●", rgb(0xff2a3c)));
    assert_eq!((b[(48, 6)].bg, b[(48, 12)].bg), (rgb(0xff2a3c), rgb(0xff2a3c)));
    assert_eq!((b[(2, 16)].symbol(), b[(2, 16)].fg, b[(2, 16)].bg), ("▀", rgb(0xb4bec8), rgb(0x7d8893)));
}

#[test]
fn weapon_pieces_draw_in_their_footprints() {
    // A muzzle place at (66, 4): the footprint starts 4 cells in, at (70, 4).
    let muzzle = place(66, 4, 22, 5, SocketKind::Muzzle);
    let b = drawn(100, 0, |p| draw_fitted(HardwareLook::Hushcan, &muzzle, p));
    assert_eq!((b[(74, 6)].symbol(), b[(74, 6)].bg), ("H", rgb(0x1c1f24)));
    assert_eq!((b[(70, 4)].symbol(), b[(70, 4)].fg, b[(70, 4)].bg), ("▀", rgb(0x3a3f47), rgb(0x5d6570)));
    assert_eq!((b[(87, 4)].symbol(), b[(87, 4)].fg), ("▄", rgb(0x0e1013)));
    let rail = place(34, 0, 24, 4, SocketKind::Rail);
    let b = drawn(100, 0, |p| draw_fitted(HardwareLook::Redeye, &rail, p));
    assert_eq!((b[(49, 1)].symbol(), b[(49, 1)].fg, b[(49, 1)].bg), ("●", rgb(0xff2a3c), rgb(0x15171b)));
    assert_eq!((b[(55, 0)].bg, b[(55, 1)].bg), (rgb(0xff2a3c), rgb(0xff2a3c)));
    assert_eq!((b[(41, 2)].fg, b[(41, 2)].bg), (rgb(0x7d8893), rgb(0xb4bec8)));
    assert_eq!(b[(34, 3)].symbol(), " ", "the rail row is the gun's, not the piece's");
    // A deck socket's piece is drawn by the deck, not here.
    let deck = place(-4, 6, 6, 7, SocketKind::Deck);
    let b = drawn(100, 10, |p| draw_fitted(HardwareLook::ColdfinI, &deck, p));
    assert!(b.content.iter().all(|c| c.symbol() == " "));
}
```

(`rgb` is `ui::hardware`'s const fn; `place(x, y, w, h, kind)` builds a `SocketPlace` with `index: 0, name: "X"`.) Extend `every_glyph_is_single_width_and_every_colour_exact` to cover all five looks' large drawings and the two weapon pieces in their footprints.

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p strata-zero ui::` → FAIL (no `Pixels`, no weapon drawings).

- [ ] **Step 3: Implement** `Pixels` (fold rule as the reference's `Pix.cells`: equal halves → `' '` fg and bg that colour; `None` top → `'▄'` fg bottom, bg untouched; `None` bottom → `'▀'` fg top, bg untouched; else `'▀'` top on bottom), then `weapon.rs`: transcribe `hushcan_large`, `redeye_large`, and the in-socket `hushcan` and `redeye` from the art reference (in-socket coordinates relative to the footprint: subtract pixels `(70, 8)` for the Hushcan and `(34, 0)` for the Redeye). Text and glyphs go on after the fold (`Pen::put` with `None` bg keeps the folded cell's bg; the reference's `None` bg means the same). The sticker is `mods.first().map(Mod::sticker)` padded with a space each side. Colours are named consts at the top of `weapon.rs` in the reference's names. `draw_large` dispatches the two new arms; `HardwareArt::size` uses `large_size`.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** — `feat(app): weapon hardware drawn large and in its sockets, on a half-block pixel canvas`

---

### Task 4: The Ratchet's drawing and the blueprint window (`strata-zero`)

**Files:**
- Create: `crates/strata-zero/src/ui/looks/mod.rs` (`ItemArt`: the `Art` for items drawn by an `ItemLook`), register in `ui/mod.rs`
- Create: `crates/strata-zero/src/ui/looks/ratchet.rs` (the gun)
- Modify: `crates/strata-zero/src/screens/blueprint.rs` (`Inspect::Drawn`, `open`, `art`, `draw_inspect`, tests)

**Interfaces:**
- Consumes: Task 3's `Pixels`, `hardware::draw_fitted`; Task 1's `ItemLook::sockets`, `Item.look`; `Art`, `SocketPlace` (`ui/art.rs`); `Character::host_copy(Host) -> Option<&ItemCopy>`.
- Produces:
  - `pub struct ItemArt { pub look: ItemLook, pub fitted: Vec<Option<HardwareLook>> }` implementing `Art`: `size` from the look (Ratchet `(88, 15)`), default `origin`, `draw` the look's drawing with `fitted`, `sockets` the look's places for the first `fitted.len()` sockets (index, rect, name and kind from `ItemLook::sockets`).
  - `ratchet::{W, H}` = 88, 15; `ratchet::PLACES: [(i32, i32, u16, u16); 2]` = MUZZLE `(66, 4, 22, 5)`, RAIL `(34, 0, 24, 4)`; `ratchet::draw(fitted: &[Option<HardwareLook>], p: &mut Pen)`.
  - `fn fitted_looks(st: &AppState, host: Option<Host>, item: &Item) -> Vec<Option<HardwareLook>>` in `blueprint.rs`: the host copy's fitted pieces' looks, else the item's sockets all empty.

- [ ] **Step 1: Write the failing tests** in `blueprint.rs` tests, on Vex's start (Ratchet worn) and D'SHAWN, opened from GEAR as the existing deck tests do, at 100×30 unless said:

- `ratchet_inspects_its_drawing`: title ` BLUEPRINT · RATCHET SMG `, hint row `m mod · esc close`, no `MISSING BLUEPRINT`; at the gun's origin (6, 6): `(6+66, 6+5)` is `▄` fg `#e8c54a`, `(6+34, 6+3)` is `▀` fg `#b4bec8` bg `#7d8893`, `(6+28, 6+6)` `R` fg `#8d939c` bg `#2e3138`; no piece: `(6+74, 6+6)` is blank; esc pops.
- `ratchet_draws_its_fitted_pieces`: after `perform(Fit { host: Worn(Primary), socket: 0, hushcan })` and socket 1 the redeye: `(6+74, 6+6)` is `H`, `(6+49, 6+1)` is `●` fg `#ff2a3c`.
- `ratchet_modding`: `m` → title ` MODDING · RATCHET SMG `, hint `↑↓←→ socket · space choose · m inspect · esc close`, readout `SOCKET MUZZLE ▸ empty · takes muzzle hardware`; `Left` → readout starts `SOCKET RAIL`; `Up` on MUZZLE leaves it on MUZZLE; a receiver cell `(6+28, 6+6)` has fg `grey(#8d939c)` while the gold thread `(6+68, 6+6)` keeps bg `#e8c54a`; the MUZZLE brackets' right column is 94 and the RAIL's top row 5, both inside the frame.
- `ratchet_fits_the_window_and_clears_the_band`: at 100×30 and at 120×40 every gun cell lies inside the window's inside and above the tray band's first row (the band starts at `body.bottom() - 5`).
- `ratchet_without_a_copy`: `Blueprint::open(st, ratchet_smg, false, None)` on a Ratchet given with `give` (a stack, no copy) draws the bare gun, the hint is `esc close`, `m` does nothing.

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p strata-zero blueprint::` → FAIL (MISSING BLUEPRINT).

- [ ] **Step 3: Implement** `ratchet.rs` by transcribing the reference's `gun()` (pixels via `Pixels`, then the text runs and glyphs, then each fitted piece through `hardware::draw_fitted` at its `SocketPlace`); `ItemArt` in `looks/mod.rs`; in `blueprint.rs` a generic `Inspect::Drawn(ItemLook)` arm (an item with a `look` that is neither deck nor hardware), `Inspect::art` taking what it needs to build `ItemArt { look, fitted: fitted_looks(..) }`, and `draw_inspect` drawing it at its origin with `(None, "esc close")` so `m mod` joins when moddable. No gun name or socket kind appears in `blueprint.rs`.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** — `feat(app): the Ratchet SMG on its blueprint, with its muzzle and rail sockets`

---

### Task 5: The tray offers the chosen socket's kind; the gun in the pane and on cards (`strata-zero`)

**Files:**
- Modify: `crates/strata-zero/src/screens/bench.rs` (`tray`, `choose`, tests)
- Modify: `crates/strata-zero/src/screens/deck.rs` (tests only)

**Interfaces:**
- Consumes: Task 4's Ratchet art; Task 2's D'SHAWN kit; `AppState::say`.
- Produces: no new names.

- [ ] **Step 1: Write the failing tests**

In `bench.rs` (or `blueprint.rs`, wherever the modding tests live) on D'SHAWN's worn Ratchet in modding:
- space on MUZZLE: the tray holds exactly `HUSHCAN`; space fits it; latest log `Hushcan fitted · socket MUZZLE`; the readout reads `SOCKET MUZZLE ▸ Hushcan · cool +1`.
- `Left`, space on RAIL: the tray holds exactly `REDEYE SIGHT`; fitting it logs `Redeye sight fitted · socket RAIL`.
- space on the filled MUZZLE: the first tile is `EMPTY`; choosing it logs `Hushcan out · to the pack` and the Hushcan is back in the pack.
- With the Hushcan dropped from the pack (remove its stack): space on the empty MUZZLE leaves `choosing` `None` and the latest log is `no muzzle hardware in your pack`, not a warning.
- Not choosing on the gun the tray shows HUSHCAN and REDEYE SIGHT, never a deck piece; on D'SHAWN's rebuilt deck, not choosing and choosing for L, the tray shows COLDFIN VI, COLDFIN VII, SIDECAR CO-PROCESSOR and never HUSHCAN or REDEYE SIGHT.
- Jacked in (as the existing jacked-in modding test sets it up), space on MUZZLE warns with `JACKED_IN` and the gun's copy is unchanged.

In `deck.rs` tests on D'SHAWN:
- GEAR's Ratchet row ends in `□□` with no ` · `; after fitting the Hushcan it ends in `■□`.
- The Ratchet's card lists `□ empty socket · muzzle hardware` and `□ empty socket · rail hardware`; after fitting, `■ Hushcan · cool +1` and its weight line reads `3.7 kg`.
- The Hushcan's card reads `hardware · muzzle socket` and `cool +1`.
- On Vex with a Hushcan fitted to the worn Ratchet, the STATS card for COOL reads `5 · 6 with gear`.

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p strata-zero` → the tray tests FAIL (REDEYE offered for MUZZLE; no "no muzzle hardware" line).

- [ ] **Step 3: Implement** in `tray`: while choosing, keep only pieces whose `fits == place.kind`; not choosing, unchanged. In `choose`: when the pointed socket is empty and the choosing tray is empty, `st.say(format!("no {} in your pack", place.kind.takes()))` and open nothing. The pane and card tests should pass without code changes; if one does not, fix the generic code, never by naming the gun.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** — `feat(app): the tray offers only what fits the chosen socket`

---

### Task 6: Shots and README

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`
- Modify: `README.md:368-392`

- [ ] **Step 1: Add ignored shot tests** beside `modding_the_rebuilt_deck`: `the_ratchet_on_its_blueprint` (D'SHAWN's worn Ratchet from GEAR, bare and with both pieces fitted, 100×30 and 120×40), `modding_the_ratchet` (pointer on MUZZLE, the tray open on MUZZLE, both fitted with the pointer on RAIL; 100×30 and 120×40), `weapon_hardware_blueprints` (`hushcan`, `redeye` from INVENTORY at 100×30).

- [ ] **Step 2: Render and look** — `SHOT_DIR=<scratch dir> cargo test -q --target-dir target/smoke -p strata-zero shots:: -- --ignored ratchet weapon_hardware`, convert with `qlmanage -t -s 1400 -o <dir> <file>.svg`, and compare by eye with the approved mockups (the art reference drawn at 100×30): the gun at rows 6-20, the tray below, the pieces where the mockups put them.

- [ ] **Step 3: README** — in the sockets paragraph: the Ratchet SMG has a muzzle and a rail socket; weapon hardware: the Hushcan (muzzle, Cool +1) and the Redeye sight (rail, Reflex +1); "Hardware only counts in the deck you wear" becomes "in what you wear"; an empty socket shows just its connector (the deck's gold contacts, the gun's gold threads or bare rail); the tray offers what fits the socket you chose.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** — `docs: the Ratchet's sockets in the README; shots`
