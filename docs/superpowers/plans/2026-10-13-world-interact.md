# World Interact Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Items lie on the map and can be picked up; Space in walk mode opens an interact popup; `a` fires active abilities with visible effects; dev mode gets a console to spawn and drop items.

**Architecture:** Two overlays (`GroundOverlay`, `PingOverlay`) draw piles and pings inside `MapView`. The popup gains `Pick::World` for jack and stair acts it applies to the player itself. `MapScreen` builds the interact and abilities popups and follows the player's stratum. The console is pure state in `ui/console.rs` plus a command runner in `screens/console.rs`, drawn in the status row.

**Tech Stack:** Rust 2024, ratatui, crossterm. Tests: `cargo test -p cyberpunk`.

**Spec:** `docs/superpowers/specs/2026-10-13-world-interact-design.md` (builds on `2026-10-13-items-design.md`)

## Global Constraints

- Space is the one interact key; Enter and `<` `>` stay as shortcuts.
- Hint rows must fit 120 columns in dev mode with the fps readout on: Walk `hjkl move · <> stairs · space act · a abilities · tab deck{cam} · ? keys`, Jacked `⚡ jacked in · space act · a abilities · tab deck{cam} · ? keys`.
- Amber for ground items `Rgb(255, 200, 80)`, scaled 0.45 when remembered; SIGNAL colour for pings.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` clean at every commit; the three targeted `#[allow(dead_code)]` in popup.rs come out in the task that uses each item.
- Commit trailer: `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01XUU3ja2Kw6dEz92VZnXrMb`.

## Review Focus

1. A pile under the player on a tile the player cannot see (jacked in, body elsewhere): the overlay must draw by the drawn stratum's visibility, never panic on a stratum with no grid (Task 1 test draws the Net with piles on the Street).
2. Jack in from the popup while the terminal is dead or out of reach must refuse through the status strip, not panic (Task 3 test).
3. Console `drop` on a tile outside the map or not walkable is refused (Task 6 test).
4. `take all` when the first stack already pushes the character over capacity must still finish the stacks already listed (ruled in spec A: capacity is checked once at the start) — Task 3 asserts both stacks arrive.
5. The abilities popup with every active cooling: `selected` lands on a disabled row and Enter does nothing (Task 4 test).

---

### Task 1: Ground and ping overlays

**Files:**
- Create: `crates/cyberpunk/src/render/glyphs.rs` (`pub fn kind_icon(kind: ItemKind) -> char`, moved from deck.rs; deck imports it)
- Create: `crates/cyberpunk/src/render/items.rs` (`GroundOverlay`, `PingOverlay`, `AMBER`)
- Modify: `crates/cyberpunk/src/render/mod.rs`, `crates/cyberpunk/src/screens/explore.rs` (push both overlays; the `@` is already drawn after `MapView`, so piles sit under it)

**Interfaces:**
- `pub struct GroundOverlay<'a> { pub ground: &'a Ground, pub content: &'a Content, pub vis: Option<&'a Grid<Visibility>> }` implements `Overlay`.
- `pub struct PingOverlay<'a> { pub ping: Option<&'a Ping> }` implements `Overlay`: while `tick < until` and `stratum == ping.stratum`, every Jack connector and every physical connector within `radius` (Chebyshev) gets `fg = SIGNAL.scale(0.5 + 0.5 * pulse(tick, 20))`, glyph unchanged.

- [ ] **Step 1: Write the failing tests** (`render/items.rs`)

```rust
    fn city() -> City { Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 5, ..Default::default() }).unwrap() }

    #[test]
    fn ground_overlay_draws_piles_by_sight() {
        let c = city();
        let content = Content::load().unwrap();
        let s = StylePack::load().unwrap();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&c, 0);
        let here = p.pos;
        let near = here.neighbors8().find(|q| c.walkable(p.stratum, *q)).unwrap();
        let far = (0..60).flat_map(|y| (0..120).map(move |x| Pos::new(x, y)))
            .find(|q| p.vis()[*q] == Visibility::Unseen && c.walkable(p.stratum, *q)).unwrap();
        let mut g = Ground::default();
        g.add(p.stratum, here, ItemId::new("copper_coil"), 2);
        g.add(p.stratum, near, ItemId::new("copper_coil"), 1);
        g.add(p.stratum, near, ItemId::new("dead_phone"), 1);
        g.add(p.stratum, far, ItemId::new("dead_phone"), 1);
        let draw = |vis: Option<&Grid<Visibility>>| {
            let mut t = Terminal::new(TestBackend::new(120, 60)).unwrap();
            t.draw(|f| MapView { city: &c, stratum: p.stratum, origin: Pos::new(0, 0), tick: 0, style: &s, vis,
                overlays: &[&GroundOverlay { ground: &g, content: &content, vis }], cursor: None }.render(f.area(), f.buffer_mut())).unwrap();
            t.backend().buffer().clone()
        };
        let buf = draw(Some(p.vis()));
        let cell = |q: Pos| buf[(q.x as u16, q.y as u16)].clone();
        assert_eq!(cell(here).symbol(), "▪");
        assert_eq!(cell(here).fg, AMBER.to_color());
        assert_eq!(cell(near).symbol(), "≡");
        assert_ne!(cell(far).fg, AMBER.to_color(), "unseen piles stay hidden");
        // walk away: the pile is remembered and dimmed
        let mut p2 = p.clone();
        p2.pos = far; p2.update_fov(&c, 0);
        let buf = draw(Some(p2.vis()));
        assert_eq!(buf[(here.x as u16, here.y as u16)].fg, AMBER.scale(0.45).to_color());
        // free camera: everything shows
        let buf = draw(None);
        assert_eq!(buf[(far.x as u16, far.y as u16)].fg, AMBER.to_color());
        // a stratum without piles never panics
        let mut t = Terminal::new(TestBackend::new(40, 20)).unwrap();
        t.draw(|f| MapView { city: &c, stratum: StratumKind::Net, origin: Pos::new(0, 0), tick: 0, style: &s, vis: None,
            overlays: &[&GroundOverlay { ground: &g, content: &content, vis: None }], cursor: None }.render(f.area(), f.buffer_mut())).unwrap();
    }

    #[test]
    fn ping_overlay_lights_connectors_in_range() {
        let c = city(); let s = StylePack::load().unwrap();
        let jack = c.connectors.iter().find(|k| k.kind == ConnectorKind::Jack && k.serves.contains(&StratumKind::Street)).unwrap().pos;
        let ping = Ping { stratum: StratumKind::Street, pos: jack, radius: 3, until: 100 };
        let other = c.connectors.iter().find(|k| k.serves.contains(&StratumKind::Street) && k.pos.chebyshev(jack) > 10).unwrap().pos;
        let mut t = Terminal::new(TestBackend::new(120, 60)).unwrap();
        t.draw(|f| MapView { city: &c, stratum: StratumKind::Street, origin: Pos::new(0, 0), tick: 10, style: &s, vis: None,
            overlays: &[&PingOverlay { ping: Some(&ping) }], cursor: None }.render(f.area(), f.buffer_mut())).unwrap();
        let buf = t.backend().buffer();
        let lit = |q: Pos| buf[(q.x as u16, q.y as u16)].fg == SIGNAL.scale(0.5 + 0.5 * pulse(10, 20)).to_color();
        assert!(lit(jack)); assert!(!lit(other));
        let mut t = Terminal::new(TestBackend::new(120, 60)).unwrap();
        t.draw(|f| MapView { city: &c, stratum: StratumKind::Street, origin: Pos::new(0, 0), tick: 100, style: &s, vis: None,
            overlays: &[&PingOverlay { ping: Some(&ping) }], cursor: None }.render(f.area(), f.buffer_mut())).unwrap();
        assert!(!(t.backend().buffer()[(jack.x as u16, jack.y as u16)].fg == SIGNAL.scale(0.5 + 0.5 * pulse(100, 20)).to_color()), "expired");
    }
```
  Explore test: `walk_draws_piles_under_the_player` — after `w`, add a coil pile at the player's feet and a phone pile two tiles away on a walkable tile, draw at 120x40, assert `@` is present and `▪`/`·`? Use a Component (`▪`) at the neighbour: assert the buffer has a `▪` cell with amber fg, and the player's own cell is `@`.
- [ ] **Step 2: Run** `cargo test -p cyberpunk items` — Expected: compile errors.
- [ ] **Step 3: Implement.** `SIGNAL` is `render::overlay::SIGNAL` (make `pub(crate)` if needed); `pulse` from `anim::effects`. Explore pushes `GroundOverlay { ground: &st.ground, content: &st.content, vis }` and `PingOverlay { ping: st.ping.as_ref() }` after `digital`.
- [ ] **Step 4: Run** `cargo test -p cyberpunk` — Expected: pass.
- [ ] **Step 5: Commit** `feat(render): ground and ping overlays`

### Task 2: Inspector lists items on the tile

**Files:** Modify `crates/cyberpunk/src/ui/panel.rs` (`inspector_lines(city, stratum, p, node, items: &[Stack], content: &Content)`), `crates/cyberpunk/src/screens/explore.rs` (pass `st.ground.at(self.stratum, cursor)`).

- [ ] **Step 1: Test** (panel.rs): with two stacks the lines contain `items` and `3× Copper coil, Dead phone`; with none, no `items` line.
- [ ] **Step 2: Run** — Expected: compile error. **Step 3: Implement.** Format: `kv("items", "3× Copper coil, Dead phone")` (qty omitted when 1). **Step 4: Run** `cargo test -p cyberpunk` — pass. **Step 5: Commit** `feat(explore): inspector lists items on the tile`

### Task 3: Space interacts with the world; the map follows the player

**Files:** Modify `crates/cyberpunk/src/screens/popup.rs` (`Pick::World(WorldAct)`, `pub enum WorldAct { JackIn, JackOut, Up, Down }`, applied in `fire`), `crates/cyberpunk/src/screens/explore.rs` (`fn interact_popup(&self, st: &AppState) -> Popup`, Space in `handle_walk`, follow rule in `update`).

**Interfaces:** `WorldAct` applied by the popup: `JackIn` → `player.jack_in(city, bonus)` → status `jacked in at {port_name}` (move `port_name` to `walker` or `pub(crate)` in explore) and `Pop { fx: Glitch }`, refusal → warning + `Pop { fx: None }`; `JackOut` likewise with `jacked out`; `Up`/`Down` → `use_connector` + `update_fov(city, bonus)` → `now on {stratum}` / refusal. Follow rule: in `update`, when `self.mode == Mode::Walk` and `player.stratum != self.stratum`: `self.stratum = player.stratum; self.cam.snap(player.pos)`.

- [ ] **Step 1: Write the failing tests** (explore.rs)

```rust
    fn walking(st: &mut AppState, s: &mut MapScreen) { s.update(st, &[]); s.update(st, &[key(KeyCode::Char('w'))]); assert_eq!(s.mode, Mode::Walk); }
    fn draw_screen(scr: &dyn Screen, st: &AppState, w: u16, h: u16) -> String { … }

    #[test]
    fn space_offers_take_rows_and_takes() {
        let mut st = state(); let mut s = MapScreen::explore(0); walking(&mut st, &mut s);
        let (k, pos) = { let p = st.player.as_ref().unwrap(); (p.stratum, p.pos) };
        st.ground.add(k, pos, ItemId::new("copper_coil"), 2);
        st.ground.add(k, pos, ItemId::new("dead_phone"), 1);
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else { panic!("space opens the interact popup") };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(text.contains("a  take 2× Copper coil") && text.contains("b  take 1× Dead phone") && text.contains("t  take all") && text.contains("l  look"), "{text}");
        p.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(st.character.count(&ItemId::new("copper_coil")), 5);
        assert_eq!(st.character.count(&ItemId::new("dead_phone")), 2);
        assert!(st.ground.at(k, pos).is_empty());
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else { panic!() };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(text.contains("l  look") && !text.contains("take"), "{text}");
        p.update(&mut st, &[key(KeyCode::Char('l'))]);
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("terrain"));
    }

    #[test]
    fn space_jacks_and_takes_stairs_and_the_map_follows() {
        let mut st = state(); let mut s = MapScreen::explore(0); walking(&mut st, &mut s);
        let city = st.city.clone().unwrap();
        let jack = city.connectors.iter().find(|k| k.kind == ConnectorKind::Jack && k.serves.contains(&StratumKind::Street) && city.walkable(StratumKind::Street, k.pos)).unwrap().pos;
        st.player.as_mut().unwrap().pos = jack;
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else { panic!() };
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("j  jack in"));
        assert!(matches!(p.update(&mut st, &[key(KeyCode::Char('j'))]), ScreenCmd::Pop { fx: TransitionKind::Glitch }));
        assert!(st.player.as_ref().unwrap().jacked_in());
        s.update(&mut st, &[]);
        assert_eq!(s.stratum, StratumKind::Net, "the map follows the player");
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else { panic!() };
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("j  jack out"));
        p.update(&mut st, &[key(KeyCode::Char('j'))]);
        s.update(&mut st, &[]);
        assert_eq!(s.stratum, StratumKind::Street);
        let stair = city.connectors.iter().find(|k| k.public && k.serves.contains(&StratumKind::Undercity) && k.serves.contains(&StratumKind::Street)).unwrap().pos;
        st.player.as_mut().unwrap().pos = stair;
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else { panic!() };
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("<  go down to Undercity"));
        p.update(&mut st, &[key(KeyCode::Char('<'))]);
        s.update(&mut st, &[]);
        assert_eq!(s.stratum, StratumKind::Undercity);
        assert_eq!(st.status_text(), Some("now on Undercity"));
    }
```
- [ ] **Step 2: Run** — Expected: failures (Space does nothing yet). **Step 3: Implement** per the spec's interact order. The popup title is `HERE`. **Step 4: Run** `cargo test -p cyberpunk` — pass. **Step 5: Commit** `feat(explore): space interacts with the world`

### Task 4: Abilities key

**Files:** Modify `crates/cyberpunk/src/screens/explore.rs` (`fn abilities_popup(&self, st) -> Option<Popup>`; `a` in `handle_walk`), `popup.rs` (remove the allows on `Choice::disabled`/`Pick::Close`).

- [ ] **Step 1: Test** `a_lists_actives_fires_and_cools`: walking; `a` → popup `ABILITIES` with `a  Netscan · ready`; fire → remembered count grows; `a` again → text contains `Netscan · cooling` and the row is disabled: `p.selected` row disabled, `Enter` → `ScreenCmd::Stay`, no status change. Clear equipment → `a` → `ScreenCmd::Stay` and status `no active abilities`.
- [ ] **Steps 2-5** as usual. Commit `feat(explore): abilities key`

### Task 5: Console state

**Files:** Create `crates/cyberpunk/src/ui/console.rs`; register in `ui/mod.rs`.

**Interfaces:** per the spec: `Console { pub input: String, history: Vec<String>, cursor: Option<usize>, pub hint: Option<String> }`, `ConsoleEvent { None, Run(String), Close }`, `key(&mut self, k: KeyEvent, complete: &dyn Fn(&str) -> Vec<String>) -> ConsoleEvent`.

- [ ] **Step 1: Tests:** typing `spa` then Tab with a completer returning `["spawn"]` fills `spawn `; `d` + Tab with `["drop", "dead_phone"]`? No: the completer receives the current word; for `drop co` the word is `co` → `["copper_coil"]` → `drop copper_coil `; two candidates set `hint` and leave the input; Enter on a non-empty line returns `Run(line)` and clears; Enter on empty → `None`; `↑` recalls the last line, `↑` again the one before, `↓` forward and back to empty; Backspace; Esc → `Close`.
- [ ] **Steps 2-5.** Commit `feat(ui): console state`

### Task 6: Console commands and the map integration

**Files:** Create `crates/cyberpunk/src/screens/console.rs` (`pub fn run(st: &mut AppState, focus: At, line: &str) -> Result<String, String>`, `pub fn complete(st: &AppState, line: &str, word: &str) -> Vec<String>`); Modify `explore.rs` (`console: Option<Console>` on `MapScreen`; `:` opens in dev mode in both modes; keys routed while open; drawn in the status row; `Run` → `run` → `report`).

- [ ] **Step 1: Tests** (console.rs): `spawn copper_coil 2` → `Ok("spawned 2× Copper coil")` and count +2; `spawn cop` → `Err("unknown item cop")`; `drop dead_phone` → pile at focus; `drop dead_phone` at a non-walkable focus → `Err("nothing can lie there")`; `give 100` → credits +100; `heal` → hp == hp_max; `clear` → pile empty; `help` → `Ok` containing `spawn`; `fly` → `Err("unknown command fly")`; `complete` of `sp` → `["spawn"]`, of `spawn co` → `["copper_coil"]`.
  (explore.rs): `:` with `st.dev = false` → no console; with dev → `s.console.is_some()`; typing `heal` + Enter → hp max and status; draw shows `: ` prompt in the bottom row; Esc closes.
- [ ] **Steps 2-5.** Commit `feat(explore): dev console`

### Task 7: Hints, help, README, smoke

**Files:** `explore.rs` hint rows (exact strings above; test asserts both at 120 columns with dev and fps on contain `? keys`), `help.rs` (walk: `space` interact, `a` abilities, `:` dev console; explore: `:`), `README.md` (walk line and a "Items in the world" paragraph), scratchpad pty smoke extended (console `spawn`/`drop`, Space take, `a`).

- [ ] **Step 1:** tests for the hint rows; **Step 2-5:** implement, run `cargo fmt`, clippy, `cargo test --workspace`, the pty smoke; commit `docs: world interact keys`.
