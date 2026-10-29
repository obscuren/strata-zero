# STATS and Chrome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The deck's STATS tab becomes the character sheet: the stats, secondary numbers
and resistances as tiles on the left, and on the right the ledger of chrome installed
in eleven body sites, with a card, a picker and a LOAD budget. Chrome leaves the GEAR
rack, and the new numbers act in play.

**Architecture:** `game` gains the body: `Site`, `Fits`, `Implant`, `Harm`, new perks,
installed chrome on `Character` beside `equipment`, actions to install and pull, and the
rule resistances use. `strata-zero` reads the numbers in combat, blasts and the Net,
draws a new `screens/sheet.rs` for the STATS tab (as `kit.rs` draws GEAR), and draws
chrome art from `flavor/implant_art.ron`, made by a committed Python tool.

**Tech Stack:** Rust, ratatui, serde + RON; Python 3 for the art tool.

**Spec:** `docs/superpowers/specs/2026-10-29-stats-chrome-design.md` (the mockups:
`docs/superpowers/specs/2026-10-29-stats-chrome-mock/ledger.html`).

## Global Constraints

- Builds and test runs use `--target-dir target/smoke`; never rebuild `target/release`.
- Sites, in ledger order: NEURAL, EYES, EARS, JAW, SPINE, BACK, ORGANS, SKIN, L ARM, R ARM, LEGS; groups head (first four), body (next four), limbs.
- `load_max = 2 × (base BODY + base COOL) + 6`; EMP resist takes 2 per installed piece, socket pieces included.
- `resisted(amount, r)`: `amount × (100 − r) / 100` rounded to nearest, `r` clamped to −100..=90; damage above zero stays at least 1; stun may fall to 0.
- ARM turns `10 × ARM` percent of the rounds that hit you, at most 60.
- MOV in a fight on foot is `BASE_MOVE + move_bonus()`, at least 1, for you only; in a construct you keep `BASE_MOVE`.
- Chrome art is 30×12 pixels (30×6 cells).
- At 100×30: left pane 44 columns, `║` at body.x + 45, ledger 18 columns, `│`, card; at 120 columns and wider the ledger is 27 columns with site labels.
- Colours: the deck's (`ui::chrome`); GOOD `#50e678`, AMBER `#ffc850`.
- Wording comes from kinds and content, never from a particular piece.

## Review Focus

- A limb pulled with pieces in its sockets, then installed again in the other arm: the pieces come back with it, counted in load and EMP.
- The picker open when the pack changes under it (the last fitting piece installed): it closes, and the cursor lands on the site.
- Jacked in: no key on STATS changes chrome, and the hints say why.
- A narrow terminal at exactly 100×30 with every site and both arms' sockets showing: the ledger fits (18 rows) and nothing draws past the pane.
- A blast on you with EMP below −100 or a resistance above 90: clamped, no overflow or zero-damage hit from a positive blast.

Each is pinned by a test in the task that owns it.

---

### Task 1: Harm, the new perks and the resistance rule

**Files:**
- Modify: `crates/game/src/model.rs` (`Perk`, `Blast`, new `Harm`)
- Modify: `crates/game/src/run.rs` (`resisted`)
- Modify: `crates/game/src/character.rs` (`armor`, `move_bonus`, `throw_bonus`, `resist`)
- Modify: `crates/game/src/content.rs` (the `Boon` text match; tests)
- Modify: `crates/strata-zero/src/screens/deck.rs` (`perk_lines`)
- Modify: `flavor/content.ron` (grenades' `harm`)

**Interfaces:**
- Produces: `game::Harm { Kinetic, Thermal, Shock, Emp, Toxin, Feedback }` (Copy, Eq, Ord, Hash, serde) with `ALL: [Harm; 6]`, `label()` (`kinetic`…`feedback`), `name()` (`KINETIC`…), `short()` (`KIN` `THM` `SHK` `EMP` `TOX` `FBK`); `Perk::Armor(i8)`, `Perk::Move(i8)`, `Perk::Resist(Harm, i8)`, `Perk::Throw(i8)`; `Blast.harm: Harm` (`#[serde(default)]`, `Default` = Kinetic); `game::resisted(amount: u32, resist: i32) -> u32` and `game::resisted_stun(stun: u32, resist: i32) -> u32` (the same sum, no floor of 1); `Character::armor(&self, c) -> i32`, `move_bonus(&self, c) -> i32`, `throw_bonus(&self, c) -> i32`, `resist(&self, c, h: Harm) -> i32` (perks only for now; Task 2 adds the EMP cost).

- [ ] **Step 1: Write the failing tests**
  - `run.rs` `resisted_rounds_and_clamps`: `resisted(10, 0) == 10`; `resisted(10, 15) == 9` (8.5 rounds to 9); `resisted(4, 50) == 2`; `resisted(3, -18) == 4`; `resisted(5, 95) == 1` (clamped to 90, floor 1); `resisted(0, -50) == 0`; `resisted(2, -300) == 4` (clamped to −100).
  - A stun goes through `resisted_stun(stun: u32, r: i32) -> u32` (same sum, no floor): `resisted_stun(2, 40) == 1`, `resisted_stun(1, 60) == 0`, `resisted_stun(2, -18) == 2`.
  - `character.rs` `new_perks_sum`: a character wearing a test item with `perks: [Armor(2), Move(1), Resist(Kinetic, 15), Throw(3)]` (built by `Content::parse` of a patched `content.ron`, as other tests do) reads `armor 2, move_bonus 1, resist(Kinetic) 15, resist(Thermal) 0, throw_bonus 3`.
  - `content.rs` `grenades_have_harm`: `frag` Kinetic, `flashbang` Shock, `emp_puck` Emp, `satchel` Thermal; a `Blast` written without `harm` parses as Kinetic.

- [ ] **Step 2: Run them to see them fail** — `cargo test -p game --target-dir target/smoke resisted new_perks grenades_have` — compile errors.

- [ ] **Step 3: Implement.** The enum and perks in `model.rs` beside `Perk`; `resisted`/`resisted_stun` in `run.rs` beside `blast_damage` (exported from `lib.rs` as the others are). The four getters follow `sight_bonus`'s shape. `perk_lines`: `+1 armour`, `+1 move`, `+15 kinetic`, `+3 throw`; the `Boon` text the same words. `harm:` on the four grenades in `content.ron`.

- [ ] **Step 4: Run** `cargo test --workspace --target-dir target/smoke` — all pass.

- [ ] **Step 5: Commit** — `feat(game): harm kinds, armour, move, resistances and throw perks`.

### Task 2: The body: sites, implants, installed chrome, two slots gone

**Files:**
- Modify: `crates/game/src/model.rs` (`Site`, `Fits`, `Implant`, `SocketKind::{Hand, Forearm}`, `Item.implant`, `Slot` loses `Neural` and `Arms`)
- Modify: `crates/game/src/character.rs` (`chrome`, `chrome_copies`, derived numbers)
- Modify: `crates/game/src/action.rs` (`Host::Site`)
- Modify: `crates/game/src/content.rs` (validation, errors, tests)
- Modify: `flavor/content.ron` (the seven pieces)
- Modify: every test and runner that used `Slot::Neural`/`Slot::Arms` or wore cyberware in a slot (game: `character.rs` 398, 408, 1302, 1603, 1840-1843; `action.rs` 927; `content.rs` 1306, 2266; strata-zero: `combat.rs` 1046/1074/1084, `actions.rs` 386/452-464, `screens/explore.rs` 3725/3745, `screens/kit.rs` and `screens/deck.rs` tests that index rack tiles: 6 → Primary is now 4, 8 → Deck is now 6, 1 → Eyes stays 1; the 3×3 navigation tests). `grep -rn "Slot::Neural\|Slot::Arms\|Cyberware" crates` must come back clean of the old uses.
- Modify: `crates/strata-zero/src/blast.rs` (`you_are_chrome` reads installed chrome)
- Modify: `crates/game/src/character.rs` `from_build`, `combat_runner` (kits and runners install chrome: without a slot, kit chrome would otherwise fall into the pack)

**Interfaces:**
- Consumes: Task 1's `Harm`, `Perk`.
- Produces:
  - `game::Site { Neural, Eyes, Ears, Jaw, Spine, Back, Organs, Skin, LeftArm, RightArm, Legs }` with `ALL: [Site; 11]` (ledger order), `label()` (`NEURAL` … `L ARM`, `R ARM`, `LEGS`), `lower()` (`neural` … `l arm`), `long()` (`NEURAL` … `LEFT ARM`, `RIGHT ARM`, `LEGS`), `group() -> &'static str` (`head` | `body` | `limbs`), `fits() -> Fits` (both arms `Fits::Arm`).
  - `game::Fits { Neural, Eyes, Ears, Jaw, Spine, Back, Organs, Skin, Arm, Legs, Socket(SocketKind) }` with `label()` (`neural` … `arm`, `legs`, `hand socket`, `forearm socket`).
  - `game::Implant { fits: Fits, load: u8 }`; `Item.implant: Option<Implant>`; `Item::per_copy` also true for an implant that fits a socket.
  - `SocketKind::Hand`, `SocketKind::Forearm` (`takes`: `hand chrome`, `forearm chrome`; `fits`: `hand socket`, `forearm socket`).
  - `Character.chrome: BTreeMap<Site, ItemId>`, `Character.chrome_copies: BTreeMap<Site, ItemCopy>`.
  - `Character::installed(&self) -> Vec<(Site, Option<usize>, ItemId)>` (site; `None` for the site's own piece, `Some(socket)` for a fitted socket piece; site order, sockets in order); `pieces(&self) -> usize`; `chrome_load(&self, c) -> u32`; `load_max(&self) -> u32`; `chrome_at(&self, site) -> Option<&ItemId>`; `chrome_copy(&self, site) -> Option<&ItemCopy>`.
  - `Host::Site(Site)`, handled by `host_item`, `host_copy`, `host_copy_mut`.
  - `perks`, `abilities` (as `Source::Item`), `worn_mods` and `weight` include installed chrome and what is fitted in it; `resist(c, Harm::Emp)` takes `2 × pieces()`.
  - `ContentError::ChromeInvalid { id: String, why: String }` (`"chrome `{id}`: {why}"`).

- [ ] **Step 1: Write the failing tests**
  - `model.rs` `sites_in_ledger_order`: `Site::ALL.len() == 11`; `Site::LeftArm.label() == "L ARM"`; `Site::RightArm.long() == "RIGHT ARM"`; groups 4/4/3; both arms' `fits()` is `Fits::Arm`; `Slot::ALL.len() == 7`.
  - `content.rs` `the_seven_pieces_have_sites`: `neural_port` fits Neural load 1; `nightglass_optics` Eyes 2; `target_lens` Eyes 2; `adrenal_pump` Organs 2; `grip_arm` and `piston_arm` Arm 3 with `sockets == [Hand, Forearm]`; `arc_hand` `Socket(Hand)` 1 with perks `[Resist(Shock, 10)]` and grants `[jolt]`; none has a slot.
  - `content.rs` `chrome_is_validated`: each is `Err(ChromeInvalid { .. })`: a cyberware item with no implant; a cyberware item with a slot; an implant on an Armor item; load 0; load 6; a socket piece with sockets of its own.
  - `character.rs` `installed_chrome_counts`: a character with `piston_arm` at LeftArm (a copy, `arc_hand` copy fitted at socket 0) and `neural_port` at Neural: `installed()` lists `(Neural, None, neural_port)`, `(LeftArm, None, piston_arm)`, `(LeftArm, Some(0), arc_hand)`; `pieces() == 3`; `load() == 5`; `stat(Body)` is base + 1; `abilities` holds ping, static, shove, jolt; `resist(Shock) == 10`; `resist(Emp) == -6`; `weight` includes 0.1 + 5.0 + 2.5.
  - `character.rs` `load_max_reads_base_stats`: BODY 4 COOL 5 → 24; with the Piston arm in, still 24.
  - `blast.rs` the existing EMP test (`net.rs` 2906-2921, combat runner stunned 2) still passes with the runner's chrome installed.
  - `kits_install_chrome`: Vex from the template has `nightglass_optics` at Eyes, none in the pack; a build kit with two `grip_arm` installs one at LeftArm and one at RightArm; a kit piece over the budget or with its site taken is packed.
  - `runners`: `combat_runner` has neural_port at Neural, adrenal_pump at Organs, target_lens at Eyes, nightglass_optics in the pack; `debug_runner` has nightglass_optics at Eyes and no cyberware in its pack.

- [ ] **Step 2: Run them to see them fail** — compile errors.

- [ ] **Step 3: Implement.** Types beside `Slot`/`SocketKind`; `Implant` validation in `validate` (grants and perks on an implant item are allowed: relax `PerkWithoutSlot` to "a slot or an implant"). The character fields mirror `equipment`/`worn` (`chrome_copy` follows `worn_copy`: counts only while `chrome` holds the site). Content: the seven pieces lose `slot`, gain `implant`, the two arms gain `sockets: [Hand, Forearm]`, `arc_hand` gains `perks: [Resist(Shock, 10)]`. `from_build`: after the slot step, kit cyberware whose site (first free of its kind, L before R) is free and whose load fits the budget is installed (a copy when per copy); anything else is packed. `combat_runner` installs its three pieces (the optics it had at Eyes go to the pack); `debug_runner` gets nothing new. Fix every old use listed above so the workspace builds; tests that wore a piece in `Slot::Neural` set `ch.chrome.insert(Site::Neural, …)` instead.

- [ ] **Step 4: Run** `cargo test --workspace --target-dir target/smoke` — all pass.

- [ ] **Step 5: Commit** — `feat(game): chrome lives in body sites; NEURAL and ARMS leave the rack`.

### Task 3: Install, pull and fit

**Files:**
- Modify: `crates/game/src/action.rs` (`Action::Install`, `InstallCopy`, `Pull`; `Fit`; `Equip`)
- Modify: `crates/strata-zero/src/actions.rs` (`handles_items`, `refresh_sight`)
- Modify: `crates/strata-zero/src/screens/deck.rs` (pack verbs offer no Equip on chrome; `item_card_of`'s kind line)
- Modify: `crates/strata-zero/src/screens/workbench.rs` (the assembly line's `kind · slot` for chrome)

**Interfaces:**
- Consumes: Task 2.
- Produces: `Action::Install { item: ItemId, site: Site }`, `Action::InstallCopy { copy: CopyId, site: Site }`, `Action::Pull(Site)`; `Character::can_install(&self, c, item: &ItemId, site: Site) -> Result<(), String>` (every refusal the action makes, for the picker to show before acting); `Character::fits_at(&self, c, item: &ItemId, at: (Site, Option<usize>)) -> bool`.

- [ ] **Step 1: Write the failing tests** (`action.rs`, `character.rs`)
  - `install_and_pull`: Install `grip_arm` (a copy) at RightArm → `chrome_at(RightArm) == grip_arm`, the pack lost it, message `installed \`Grip arm\` · R ARM`; Pull(RightArm) → back in the pack with its copy, message `pulled \`Grip arm\` · to the pack`.
  - `install_refusals_change_nothing` (character cloned before, equal after each): not in the pack; `hardhat` (not chrome); `neural_port` at Eyes (doesn't fit, `` `Neural port` doesn't fit EYES ``); RightArm already holding something (`pull the \`Grip arm\` out first`); over budget (a character with load 22 of 24 installing a load-3 piece: `too much chrome: load 25 of 24`); `arc_hand` at a site (it fits a socket).
  - `a_pulled_limb_keeps_its_pieces`: Piston arm with Arc hand fitted, Pull, InstallCopy at RightArm → `installed()` has the arc hand under RightArm, `chrome_load()` unchanged.
  - `fit_takes_socket_chrome`: Fit `{ host: Host::Site(LeftArm), socket: 0, piece: <arc_hand copy> }` works; socket 1 (Forearm) refuses it (`does not fit a forearm socket`); over budget refuses.
  - `equip_refuses_chrome`: `Equip(neural_port)` is `Err("chrome is installed from STATS")`.
  - `strata-zero` `actions.rs` `chrome_is_locked_jacked_in`: Install/Pull refused with `JACKED_IN` while jacked in.

- [ ] **Step 2: Run them to see them fail.**

- [ ] **Step 3: Implement.** Install mirrors Equip/EquipCopy (take from the pack, then place; every refusal before the first change); Pull mirrors Unequip and clamps HP. `Fit`'s piece check accepts `implant.fits == Fits::Socket(kind)`; with `Host::Site` it also checks the budget. In strata-zero the three new actions join `handles_items` and `refresh_sight`. The pack's verbs: no Equip on cyberware. `item_card_of`'s kind line for chrome, and the workbench's assembly line (`screens/workbench.rs` ~565, which builds `kind · slot`): `cyberware · arm · load 3` (`Fits::label`).

- [ ] **Step 4: Run** `cargo test --workspace --target-dir target/smoke` — all pass.

- [ ] **Step 5: Commit** — `feat(game): install and pull chrome; arms take hand and forearm pieces`.

### Task 4: The new chrome, its abilities and the choke can

**Files:**
- Modify: `flavor/content.ron` (24 new cyberware items, 4 abilities, `choke_can`)
- Modify: `crates/game/src/model.rs` (`Fx::Haze`, `is_blast`)
- Modify: `crates/game/src/content.rs` (tests)
- Modify: `crates/game/src/character.rs` (combat runner packs 2 `choke_can`; the runners test asserts them)

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: the item ids in the spec's table ("The chrome that ships"); abilities `bite`, `spit`, `whip`, `scout`; `choke_can`; `Fx::Haze` (a blast shape).

- [ ] **Step 1: Write the failing tests** (`content.rs`)
  - `every_shipped_piece`: the 31 ids of the spec's table exist, are Cyberware, with the table's fits, load, perks and grants (a table-driven test: `(id, Fits, load, perks, grants)`); weights and values as the table; `Ability` `bite` is `Strike { range: 1, damage: 4, stun: 0, skill: athletics, stat: Body, ap: 1 }` with cooldown 60, `spit` range 4 damage 3 stun 1 Reflex cooldown 300, `whip` range 2 damage 6 blades Reflex cooldown 90, `scout` `Scan { radius: 18 }` cooldown 600.
  - `choke_can_is_toxin`: `Throw { range: 7, arc: Lob, blast: Blast { radius: 2, damage: 8, stun: 0, harm: Toxin, .. } }`, fx `[Throw, Haze, Number { tag: true }]`; `Fx::Haze.is_blast()`.
  - `no_line_promises_more`: no new piece's `desc` contains `disarm`, `revive`, `get back up`, `through doors`, `boots`.

- [ ] **Step 2: Run to see them fail.**

- [ ] **Step 3: Implement.** Content per the spec's tables (descriptions per its "Descriptions" paragraph and `2026-10-29-stats-chrome-mock/page.py`'s `CHROME`). Abilities' fx and lines per the spec's ability table.

- [ ] **Step 4: Run** `cargo test --workspace --target-dir target/smoke` — all pass (Fx::Haze draws nothing yet; Task 6 draws it).

- [ ] **Step 5: Commit** — `feat(content): thirty-one pieces of chrome, four abilities, the choke can`.

### Task 5: Chrome art

**Files:**
- Create: `docs/superpowers/specs/2026-10-29-stats-chrome-art/implantart.py` (every drawing, 30×12, palette and helpers), `export.py` (writes the RON), `board.py` (an HTML board of every drawing at the card's size, for review)
- Create: `flavor/implant_art.ron` (generated)
- Modify: `crates/game/src/model.rs` (`IMPLANT_ART_W = 30`, `IMPLANT_ART_H = 12`, `ImplantArt { rows: Vec<String>, colours: BTreeMap<char, (u8, u8, u8)> }`)
- Modify: `crates/game/src/content.rs` (load, `implant_art(&ItemId)`, validation)
- Create: `crates/strata-zero/src/ui/implant_art.rs` (+ `pub mod implant_art;`)

**Interfaces:**
- Produces: `Content.implant_art: BTreeMap<ItemId, ImplantArt>` (serde skip, filled by `parse` from `include_str!("../../../flavor/implant_art.ron")`); `Content::implant_art(&self, &ItemId) -> Option<&ImplantArt>`; `ContentError::ChromeInvalid` for art problems; `ui::implant_art::{W: u16 = 30, ROWS: u16 = 6, draw(buf: &mut Buffer, x: u16, y: u16, clip: Rect, art: &ImplantArt, rows: u16)}` — clears its block to `PANEL_BG`, then the first `rows` cell rows, nothing outside `clip` (the skill art's drawing code, without the learned/neon rules).

- [ ] **Step 1: Draw.** `implantart.py` holds one function per piece returning 12 rows of 30 pixels (colour or empty), in the spec's register (side-on, gunmetal/steel greys with a highlight edge, one light in the piece's colour, CYAN by default). The Piston arm is `piston_arm_art` from `2026-10-29-stats-chrome-mock/gen.py`, verbatim. `board.py` renders every drawing into an HTML board (half-blocks, the mock's `cells.py`) for a look; open it with headless Chrome (`2026-10-29-stats-chrome-mock` has the pattern) and check each reads as what it is.
- [ ] **Step 2: Export** — `python3 docs/superpowers/specs/2026-10-29-stats-chrome-art/export.py flavor/implant_art.ron`; the file's header says it is generated and where to draw.
- [ ] **Step 3: Write the failing tests**
  - `content.rs` `implant_art_for_every_piece`: every cyberware item has art 30 wide and 12 tall; `piston_arm`'s row 3 (its cyan elbow) holds a char whose colour is `(0, 229, 255)`.
  - `chrome_art_is_validated`: art removed for one piece; a 29-wide row; 11 rows; a char with no colour; art for an unknown item: each `Err(ChromeInvalid { .. })`.
  - `ui/implant_art.rs` `draws_inside_its_clip`: drawing at (2, 1) into a 20×4 clip leaves every cell outside untouched and sets the half-block cells inside.
- [ ] **Step 4: Implement** loading, validation and the drawing; **run** `cargo test --workspace --target-dir target/smoke` — all pass.
- [ ] **Step 5: Commit** — `feat(chrome): every piece's drawing, its tool and its RON`.

### Task 6: The numbers in play

**Files:**
- Modify: `crates/strata-zero/src/combat.rs` (`fire_at_you`: ARM, KIN)
- Modify: `crates/strata-zero/src/blast.rs` (`effect` for you, `detonate`)
- Modify: `crates/strata-zero/src/net.rs` (`hit`: FBK; `pools_for`, `aim_cap`, `turn_view`: your MOV)
- Modify: `crates/strata-zero/src/casts.rs` (Redline refill: your MOV)
- Modify: `crates/strata-zero/src/screens/explore.rs` (footer budgets, clamps, throw range)
- Modify: `crates/strata-zero/src/render/fx.rs` (or wherever blast shapes draw: `Fx::Haze`)

**Interfaces:**
- Consumes: Tasks 1-4.
- Produces: `net::you_move(st: &AppState, place: Place) -> u8` (`BASE_MOVE + move_bonus`, at least 1, on foot; `BASE_MOVE` in a construct), used everywhere you got `BASE_MOVE`; `blast::effect_on_you(st: &AppState, blast: &Blast, ring: i32) -> (u8, u8)`.

- [ ] **Step 1: Write the failing tests**
  - `combat.rs` `armour_turns_rounds`: with ARM 6 and a roll forced to turn (the `roll` hash at a chosen tick; find one in the test), a hitting round does no damage, logs `turned`, plays the Spark at you; with ARM 0 the same tick wounds. ARM 9 still turns at most 60% (a test of the chance function: `turn_chance(9) == 60`, `turn_chance(3) == 30`).
  - `combat.rs` `kinetic_resist_cuts_rounds`: KIN 50, a round of 4 does 2.
  - `blast.rs` `resistances_cut_blasts`: SHK 40 → flashbang stun 2 becomes 1; THM 15 → satchel's centre 16 becomes 14; TOX 40 → choke can 8 becomes 5; EMP with nine pieces (−18) → puck 3 becomes 4, stun 2 stays 2; a chromeless you still takes nothing from the puck; resist 300 or −300 clamps (Review Focus).
  - `net.rs` `feedback_cuts_ice`: black ICE hit with FBK 25 does `resisted(base − armour, 25)` HP before `deep_end` halves it; grey drain likewise; white heat unchanged.
  - `net.rs` `move_reads_chrome`: with `redline_spine` installed, a fresh engagement gives you 5 moves; with `ceramic_plating` alone, 3; Redline's refill gives 5 with the spine; the footer shows `5`; engaged with ICE in a construct, the spine still leaves you 4.
  - `explore.rs` `throw_reach_grows`: with `launcher_sleeve` fitted in an installed arm's forearm socket, the frag's armed range is 11.
  - `render` (the blast shapes' tests' pattern) `haze_draws_green_and_thins`: mid-way the footprint holds `░` or `▒` in green; at the end nothing is left.

- [ ] **Step 2: Run to see them fail.**

- [ ] **Step 3: Implement** per the spec's "In play" table. `turn_chance(armor: i32) -> u32` lives in `game::run` (`(10 × armor).clamp(0, 60)`); the turn roll is a second `roll(st)` after a hit. `wound_you` stays the single HP path; the black ICE HP edit in `hit` keeps its own path but applies `resisted` first.

- [ ] **Step 4: Run** `cargo test --workspace --target-dir target/smoke` — all pass.

- [ ] **Step 5: Commit** — `feat(combat): armour turns rounds, resistances cut harm, chrome moves you`.

### Task 7: The sheet: state, the left pane and the readout

**Files:**
- Create: `crates/strata-zero/src/screens/sheet.rs` (+ `pub mod sheet;` in `screens/mod.rs`)
- Modify: `crates/strata-zero/src/screens/deck.rs` (Tab::Stats delegates draw, keys, hints, restore, as Tab::Kit does; `stat_rows` and `Detail::Stat` go)
- Modify: `crates/strata-zero/src/app.rs` (`deck_sheet: SheetState`, reset with the deck's other state)
- Modify: `crates/game/src/character.rs` (`Num`, `sources`)
- Modify: `crates/game/src/content.rs`, `flavor/content.ron` (`number_blurbs`)

**Interfaces:**
- Consumes: Tasks 1-3; Task 6's `net::you_move(st: &AppState, place: Place) -> u8` for MOV (with no body, `BASE_MOVE + move_bonus`, at least 1).
- Produces:
  - `game::Num { Stat(Stat), Hp, Focus, Armor, Sight, Carry, Move, Resist(Harm) }`, `Num::label()` (`BOD`… `HP` `FOC` `ARM` `SGT` `CAR` `MOV` `KIN`…), `Num::name()` (`BODY` … `HP`, `FOCUS`, `ARMOUR`, `SIGHT`, `CARRY`, `MOVE`, `KINETIC` …).
  - `game::Contrib { Gear(ItemId), Chrome(ItemId), Hardware(ItemId), Talents, Pieces(usize) }` (not `From`: that name is the prelude's) and `Character::sources(&self, c, n: Num) -> Vec<(Contrib, i32)>` (every non-zero contribution, gear in slot order, chrome in `installed()` order, talents summed into one, EMP's `Pieces(pieces)` at `-2 × pieces`).
  - `Content.number_blurbs: Vec<(String, String)>` keyed by `Num::label()` for the twelve non-stat numbers, validated to hold each once; `Content::number_blurb(&str) -> &str`.
  - `screens::sheet::{SheetState { pane: SheetPane, tile: usize, row: usize, picker: Option<usize> }, SheetPane { Left, Ledger }, Sheet }`; `Sheet::new(tick)`, `Sheet::restore(st)`, `Sheet::key(&mut self, st: &mut AppState, code: KeyCode) -> bool`, `Sheet::draw(&self, st, body: Rect, buf)`, `Sheet::hints(&self, st) -> String`. Tiles indexed 0..17 in reading order (5 stats, 6 secondary, 6 resistances).

- [ ] **Step 1: Write the failing tests**
  - `character.rs` `sources_name_each_contribution`: Piston arm installed and mirrorshades worn: `sources(Stat(Body)) == [(Contrib::Chrome(piston_arm), 1)]`, `sources(Stat(Cool)) == [(Contrib::Gear(mirrorshades), 1)]`, `sources(Resist(Emp)) == [(Contrib::Pieces(1), -2)]`.
  - `content.rs` `number_blurbs_cover_the_twelve`, and a missing one is an error.
  - `sheet.rs` `left_pane_draws_its_tiles` at 100×30 (the deck on `2`): the body text holds `STATS`, `2 points` (with 2 points unspent), `┌ BOD`, `SECONDARY`, `from gear & chrome`, `┌ ARM`, `RESISTANCES`, `┌ EMP`; nothing in the left pane's columns past body.x + 44.
  - `sheet.rs` `keys_walk_the_tiles`: from BOD, `→`×4 reaches NET, `→` again crosses to the ledger; `←` comes back to NET; `↓` from REF lands on FOC (column kept), `↓` again on THM; `↓` from NET (column 4) lands on CAR; `space` on BOD with points buys one (stat up, points down); `space` on ARM changes nothing.
  - `sheet.rs` `readout_names_sources`: on BOD with the Piston arm in, the readout holds `BODY 4 · 5 with chrome`, `next N pts` with N from `Build::from_character(..).stat_cost(Body)` (points unspent), `feeds`, `chrome +1 Piston arm`; on EMP with nine pieces, `chrome -18 · nine pieces`; on SGT, the value equals the map's radius where you stand.
  - `deck.rs` the STATS tab reopens where it was left (`st.deck_sheet` kept across a close and reopen).

- [ ] **Step 2: Run to see them fail.**

- [ ] **Step 3: Implement.** The tile is drawn as the GEAR rack's (`kit.rs` `draw_tile`): label in the top edge, value in the middle, the modifier set into the bottom edge; selected and focused CYAN with `BAR_BG`, selected unfocused DIM. Values: `stat`, `hp_max`, `focus_max`, `armor`, SGT `(player.radius_at(daylight) + st.sight_bonus()).max(1)` (0 with no body), `carry_cap` rounded, `you_move`, `resist`. Modifier: the sum of `sources` (GOOD positive, MAGENTA negative). The readout types out on each selection change (`reveal`, `REVEAL_TICKS`). Number words in the EMP count: one to twelve spelled, then digits.

- [ ] **Step 4: Run** `cargo test --workspace --target-dir target/smoke` — all pass.

- [ ] **Step 5: Commit** — `feat(deck): STATS is the sheet: stats, secondary numbers and resistances`.

### Task 8: The sheet: the ledger, the card and the picker

**Files:**
- Modify: `crates/strata-zero/src/screens/sheet.rs`

**Interfaces:**
- Consumes: Tasks 2, 3, 5, 7; `actions::perform` for `Install`/`InstallCopy`/`Pull`/`Fit`/`Unfit`.
- Produces: `sheet::ledger_rows(st: &AppState) -> Vec<LedgerRow>` where `LedgerRow { Group(&'static str), Site(Site), Socket(Site, usize) }`; `sheet::fitting(st: &AppState, at: (Site, Option<usize>)) -> Vec<Pick>` with `Pick { item: ItemId, copy: Option<CopyId>, load: u8 }` in pack order.

- [ ] **Step 1: Write the failing tests** (all on the deck at 100×30 unless named)
  - `ledger_lists_sites_and_sockets`: with the spec's loadout (Neural port, Target lens, Bulldog jaw, Redline spine, Drone cradle, Adrenal pump, Subdermal mesh, Piston arm at L ARM with the Arc hand): the ledger holds `── head ──`, `Neural port`, `· ears`, `── limbs ──`, `Piston arm`, `├ Arc hand`, `└ □ forearm`, `· r arm`, `· legs`; header `LOAD`, `20/24`... recomputed from content (assert `chrome_load()`/`load_max()` in the text, not a constant).
  - `full_body_fits_at_100x30` (Review Focus): both arms chrome with both sockets full, every site full: every ledger row is inside the body; nothing drawn right of the body.
  - `card_on_a_limb`: the card holds `PISTON ARM`, `L ARM · load 3 · EMP −2`, `BODY +1`, `Shove`, `■ hand`, `Arc hand`, `□ forearm`, a FAINT line naming a forearm piece from content; the art's cyan elbow cell is drawn at the card's top rows.
  - `picker_installs`: on `· r arm` with a Grip arm in the pack, `space` opens the picker (`FITS`, `from your bag`, `Grip arm`, `L3`), LOAD shows `23` in AMBER; `space` installs it (R ARM holds it, the picker is closed, the cursor on R ARM); `esc` instead closes it changing nothing.
  - `picker_over_budget`: a pick that would exceed the budget shows its load in MAGENTA and `space` refuses with the message, nothing changed.
  - `picker_closes_when_nothing_fits` (Review Focus): the picker open, the last fitting piece leaves the pack (installed elsewhere through the action in the test): on the next update the picker is closed and the cursor is on the site row.
  - `nothing_fits_says_so`: `space` on an empty site with nothing fitting opens no picker and the status line says `nothing in your bag fits`.
  - `pull_and_reinstall_keeps_pieces` (Review Focus): `space` on the Piston arm pulls it (with the Arc hand inside); installing it at R ARM through the picker brings the Arc hand back under R ARM, LOAD and EMP as before.
  - `socket_picker`: on `└ □ forearm` with a Monowire spool in the pack, `space`, `space` fits it (`Fit` with `Host::Site(LeftArm)`, socket 1).
  - `locked_jacked_in` (Review Focus): jacked in, `space` on chrome and on an empty site change nothing; the status line holds the gear lock's wording and the hints dim `space`.
  - `wide_ledger_names_sites` at 120×40: `NEURAL  Neural port`, `EARS    · meat`; the card art and text right of the ledger's `│`.
  - Hints: the four hint lines of the spec's "Layout at 100×30", by focus.

- [ ] **Step 2: Run to see them fail.**

- [ ] **Step 3: Implement** the ledger (`ledger_rows` from `Site::ALL`, `Site::group`, installed pieces' `sockets`), the card (art via `ui::implant_art::draw`, then the lines; blank lines give way first when tall, then the description is cut), the FITS list and picker, the LOAD gauge (12 segments, `ceil(load × 12 / max)` filled, the pick's extra in AMBER), and the keys of the spec's "Focus and keys". Installing and pulling go through `actions::perform` so the lock, the sight refresh and the status message are the app's.

- [ ] **Step 4: Run** `cargo test --workspace --target-dir target/smoke` — all pass; `cargo clippy --workspace --target-dir target/smoke -- -D warnings` clean.

- [ ] **Step 5: Commit** — `feat(deck): the chrome ledger, its card and the picker`.

### Task 9: Shots and the README

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (`stats_sheet` shots)
- Modify: `README.md`

- [ ] **Step 1: Shots.** A `#[test] #[ignore] fn stats_sheet()` in the GEAR shots' pattern (`kit_shots`), at `[(100, 30), (120, 40)]`: `stats_ledger_limb` (the loadout on the Piston arm), `stats_picker_site` (R ARM, a Grip arm picked), `stats_picker_socket` (the forearm, the Monowire spool picked), `stats_left_emp` (the left pane on EMP). Run with `SHOT_DIR=/tmp/... cargo test -p strata-zero --target-dir target/smoke stats_sheet -- --ignored` and look at each SVG against `ledger.html`; fix what differs.
- [ ] **Step 2: README.** The deck paragraph's STATS description becomes the sheet and the ledger; a short chrome section: the eleven sites and the arm sockets, LOAD from BODY and COOL, EMP's −2 a piece, install and pull from STATS, locked jacked in, what ARM, MOV and the resistances do; the rack's "nine slots" becomes seven; the choke can joins the grenades' line.
- [ ] **Step 3: Run** `cargo test --workspace --target-dir target/smoke` and clippy — clean.
- [ ] **Step 4: Commit** — `docs: STATS and chrome in the README; sheet shots`.
