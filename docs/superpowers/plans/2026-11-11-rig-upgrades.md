# Rig Upgrades Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The home rig becomes a rack of sleds fitted at a desk screen, the sleds set its backdoor lines, hold and firewall, and (slice two) HELIX jobs run on the rig on game time and report by a 06:00 mail.

**Architecture:** The rack is an item of a new `ItemKind::Frame` whose sockets are bays; sleds are `Hardware` with new mods; the fitted rack lives on `Character` as `rig: Option<ItemCopy>` and is reached as `Host::Rig` through the existing `Fit`/`Unfit` actions. `rig::spec` reads what the sleds give; `Backdoors` takes it as arguments. Jobs live in `Backdoors.jobs` (per city, travels in `outside::Stay`) and are advanced from `net::tick_backdoors` with catch-up by elapsed minutes.

**Tech Stack:** Rust 2024 workspace (`game`, `strata-zero`), ratatui cells, RON content in `flavor/`.

**Spec:** `docs/superpowers/specs/2026-11-11-rig-upgrades-design.md` (read it first; every number and line of copy below comes from it).

## Global Constraints

- Build into `--target-dir target/smoke` only. Never touch `target/release`.
- Every task ends green on: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`, `cargo test --workspace --target-dir target/smoke`. Tasks touching drawing or screens also run the window build: `cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings` and `cargo test --workspace --features strata-zero/window --target-dir target/smoke`.
- New enum variants join their enums **at the end** (`SocketKind`, `ItemKind`, `Mod`, `HardwareLook`, `Host`, `Why`, `Source`). New fields in saved types are `#[serde(default)]`. No `HashSet`, `Instant`, `&'static str` in saved types.
- Never regenerate `crates/strata-zero/src/runs/fixtures/`.
- Colours are exact `Color::Rgb`. Kind colours: CORE `#00e5ff`, RELAY `#50e678`, LINE `#ffb000`, FIREWALL `#ff3a5a`.
- The rig's hostname stays `zenbo`. On-screen word is "rack"; code word is "frame".
- Copy in the game's terse voice; counts in digits (`3 lines`, `1 of 2 seats`).
- Commit at the end of each task; messages `area: what` with the trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

- An old save (the fixtures) opens and the rig reads as the stock Zenbo with ZB-8/R-3/CU-1: Task 2 test `a_save_without_a_rig_has_the_stock_one`.
- Fewer lines than backdoors held (a relay pulled): nothing burns, planting is refused until under: Task 4 test `fewer_lines_than_held_burns_nothing`.
- A long absence (days away, asleep) gives the same finds as ticking hour by hour: Task 10 test `catch_up_matches_ticking_through`.
- Swapping to a frame with fewer bays puts the extra sleds in the pack and kills jobs over the new seat count: Task 7 test `a_smaller_frame_sends_extras_to_the_pack` and Task 11 test `losing_seats_kills_the_newest_jobs`.
- The desk at 100×30 still shows every bay: Task 7 shot plus test `the_desk_fits_100x30`.

---

## Slice one: the rack and its sleds

### Task 1: Content model for frames and sleds

**Files:**
- Modify: `crates/game/src/model.rs` (`SocketKind` :395 + its `label`/`takes`/`fits`; `ItemKind` :217 + `ALL`; `Mod` :494 + `line`/`sticker`; `HardwareLook` :556 + `kind`; `Item` :853)
- Modify: `crates/game/src/content.rs` (validation :533–586)
- Modify: `flavor/content.ron` (items, recipes)
- Test: `crates/game/src/content.rs` tests

**Interfaces:**
- Produces: `SocketKind::{Core, Relay, Line, Firewall}` (labels `core`/`relay`/`line`/`firewall`; `needs_a_doc` false); `ItemKind::Frame`; `Mod::{Seats(u8), Speed(u16), Lines(u8), Hold(u8), Firewall(u8)}`; `HardwareLook::{SledCore, SledRelay, SledLine, SledFirewall}`; `Item.frame: Option<FrameSpec>` with `pub struct FrameSpec { pub theme: String, pub head: String, pub base: FrameBase }` and `pub enum FrameBase { Crate, Plinth, Foot, Rails }`; `SocketKind::is_rig(self) -> bool`.

- [ ] **Step 1: Write the failing tests** in `content.rs` tests:
  - `every_frame_is_valid`: for each of `rack_zenbo`, `rack_civitas`, `rack_lotus`, `rack_halcyon`: kind `Frame`, `slot` None, `frame` Some, sockets exactly the spec's table (`[Core, Relay, Line]`, `[Core, Relay, Line, Firewall]`, `[Core, Core, Relay, Line]`, `[Core, Core, Relay, Line, Firewall]`), themes `street`/`municipal`/`neon`/`megacorp`.
  - `every_sled_has_its_mods`: the 12 sleds of the spec's table with their exact `fits`, `mods` and `value`; weight 0.4.
  - `rig_kinds_fit_only_frames`: content with a non-frame item socketed `Relay` fails validation; a frame with a `Deck` socket fails; a frame missing `Line` fails.
  - `mod_lines_read`: `Seats(2).line() == "seats 2"`, `Speed(150).line() == "jobs ×1.5"`, `Lines(5).line() == "lines 5"`, `Hold(5).line() == "holds 5 days"`, `Firewall(1|2|3).line()` == `"sandbag"`/`"tripwire"`/`"killswitch"`.
  - `the_sled_recipes_build`: the nine recipes of spec §4 exist with skill `electronics` and those inputs.
- [ ] **Step 2: Run** `cargo test -p game --target-dir target/smoke content::` — FAIL (variants missing).
- [ ] **Step 3: Implement** the variants and `FrameSpec`; validation: frames exempt from the drawing rule, frame sockets only rig kinds with ≥1 each of Core/Relay/Line, frame `slot` None; rig socket kinds only on frames; hardware with a rig `fits` needs ≥1 mod. Add to `content.ron`: 4 racks (`frame: Some(FrameSpec(theme: "street", head: "ZENBO 64  REDY", base: Crate))`, Civitas `"CIVITAS KIOSK  READY"`/`Plinth`, Lotus `"LOTUS  ALL LIT"`/`Foot`, Halcyon `"HALCYON NODE 9  OK"`/`Rails`), 12 sleds (names, values, one-line descs in item voice), items `prog_sniff`, `prog_keepalive` (kind `Program`-free: use `Component`, weight 0.05, value 400, desc "sniff · a rig program on a shard" / "keepalive · …"), `paydata_sifted` ("Paydata shard · sifted"), `vault_data_sifted` ("Vault dump · sifted") with their unsifted twin's value and weight, and the nine recipes with 2–4 `steps` lines ≤16 chars.
- [ ] **Step 4: Run** the tests — PASS; then the whole workspace check (Global Constraints).
- [ ] **Step 5: Commit** `content: racks, sleds and their recipes`.

### Task 2: The rig on the character, `Host::Rig`

**Files:**
- Modify: `crates/game/src/character.rs` (`Character` :272, `host_item` :1541, `host_copy` :1550, `host_copy_mut` :1560)
- Modify: `crates/game/src/action.rs` (`Host` :13, `socket_name` :138, Fit/Unfit arms :473–576)
- Test: `crates/game/src/character.rs` tests

**Interfaces:**
- Consumes: Task 1 items.
- Produces: `Character.rig: Option<ItemCopy>`, `Character.frames: Vec<ItemId>`, `Character.rig_rewards: RigRewards` (all `#[serde(default)]`); `pub struct RigRewards { pub paid: BTreeSet<String>, pub sleds: u8, pub racks: u8, pub halcyon: bool }`; `Host::Rig`; `Character::rig_frame(&self) -> ItemId` (`rack_zenbo` when `rig` is None); `Character::rig_copy_mut(&mut self, c: &Content) -> &mut ItemCopy` (makes the stock rig: `rack_zenbo` with `sled_zb8`, `sled_r3`, `sled_cu1` fitted, and pushes `rack_zenbo` into `frames` if missing); `Character::rig_fitted(&self) -> Vec<(SocketKind, Option<ItemId>)>` (the stock list when None); `Character::set_frame(&mut self, c: &Content, frame: &ItemId) -> Vec<ItemId>` (moves sleds into the new frame's bays of their kind in order, returns the ids sent to the pack, which it stows).
- `pub const STOCK_FRAME: &str = "rack_zenbo"; pub const STOCK_SLEDS: [&str; 3] = ["sled_zb8", "sled_r3", "sled_cu1"];` in `character.rs`.

- [ ] **Step 1: Write failing tests**: `a_save_without_a_rig_has_the_stock_one` (deserialize a `Character` JSON/msgpack without the fields → `rig_fitted` is the stock list, `frames` gains `rack_zenbo` after `rig_copy_mut`); `fitting_a_sled_into_the_rig` (`apply(Fit{host: Host::Rig, socket: 0, piece})` with a Tinman copy in pack → bay 0 holds Tinman, ZB-8 in pack); `a_wrong_kind_is_refused` (a relay into the core bay → Err); `set_frame_moves_sleds` (Zenbo → Civitas keeps three sleds, firewall bay empty, returns []); `a_smaller_frame_sends_extras_to_the_pack` (Lotus with two cores → Zenbo: second core in pack, returned).
- [ ] **Step 2: Run** `cargo test -p game --target-dir target/smoke character::` — FAIL.
- [ ] **Step 3: Implement.** `host_item(Host::Rig)` returns the frame id; `host_copy_mut(Host::Rig)` goes through `rig_copy_mut`. `trace_cut`, `deck_power` and attribute sums must not read the rig (they walk worn/installed only; leave so).
- [ ] **Step 4: Run** tests — PASS; workspace check.
- [ ] **Step 5: Commit** `game: the rig on the character, Host::Rig`.

### Task 3: `rig::spec`

**Files:**
- Create: `crates/strata-zero/src/rig/mod.rs` (`pub mod jobs;` comes in Task 10)
- Modify: `crates/strata-zero/src/main.rs` (or wherever modules are declared: `mod rig;`)
- Test: in `rig/mod.rs`

**Interfaces:**
- Produces: `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct RigSpec { pub seats: u8, pub speed: u16, pub lines: usize, pub hold_days: u64, pub firewall: u8 }`; `pub fn spec(ch: &Character, c: &Content) -> RigSpec`; `pub fn of(st: &AppState) -> RigSpec`; `pub const STOCK: RigSpec = RigSpec { seats: 1, speed: 100, lines: 3, hold_days: 3, firewall: 0 }`.

- [ ] **Step 1: Failing tests:** `the_stock_rig_is_todays_numbers` (`spec(&Character::default-ish, c) == STOCK`); `two_cores_add_their_seats` (Lotus with Tinman + Q-Stack → seats 5, speed 150); `an_empty_bay_gives_nothing` (relay pulled → lines 0); `the_best_firewall_counts` (Civitas + tripwire → firewall 2).
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement** (sum `Seats`, `Lines`; max `Speed` default 100, max `Hold`, max `Firewall`; `hold_days` 0 with no line sled). **Step 4: Run** — PASS. **Step 5: Commit** `rig: what the sleds give`.

### Task 4: Backdoors take the rig's spec

**Files:**
- Modify: `crates/strata-zero/src/backdoors.rs` (drop `SLOTS`; `can_plant` :246, `plant_net` :291, `start_trace` :375, `minute` :425, `raid` :473, `Refusal::words` :147, `Event` :183)
- Modify: `crates/strata-zero/src/net.rs` (`plant_net_refusal` :824, `Op::Backdoor` :1199, `dump`/`dump_for` :1466, `tick_backdoors` :1507)
- Modify: `crates/strata-zero/src/helix/bin.rs` (:330), `crates/strata-zero/src/access.rs` (:190, :219), `crates/strata-zero/src/helix/shell.rs` (backdoor man page ~:1056)
- Test: `backdoors.rs` tests, `net.rs` tests

**Interfaces:**
- Consumes: `rig::of(st) -> RigSpec`.
- Produces: `can_plant(&self, b, now, lines: usize)`; `plant_net(&mut self, b, rating, seed, now, hold_days: u64)`; `start_trace(&mut self, now, firewall: u8)`; `minute(&mut self, now, firewall: u8) -> Vec<Event>`; new `Event::TraceStarted { hops: usize }` returned by the next `minute` after a trace starts (store a flag); `Refusal::NoRelay`; `Why::Noticed` (label `"the admins found it"`, used in Task 10).

- [ ] **Step 1: Failing tests** in `backdoors.rs`: `planting_refuses_at_the_rigs_lines` (lines 5 → 5 plant, the 6th `Full`; words `"the rig holds 5 lines · burn one first"`); `no_relay_no_lines` (lines 0 → `NoRelay`, words `"no relay in the rig · no lines"`); `fewer_lines_than_held_burns_nothing` (3 held, lines 1 → all live, `can_plant` Full); `a_net_backdoor_lives_the_hold` (hold 5 → nominal `5*24*60`, rating 2 → doubled); `sandbag_doubles_the_last_leg` (firewall 1 → last leg moves at `LAST_LEG*2`); `tripwire_event_once` (firewall 2 → one `TraceStarted{hops}` per trace; firewall 0 → none); `killswitch_burns_without_seizing` (firewall 3 → raid burns all, `rig_seized_until` None, `fixer_quiet_until` None, no `RigReturned` later, a `Raid` event carries `wiped: true` — make `Event::Raid { wiped: bool }`).
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke backdoors::` — FAIL.
- [ ] **Step 3: Implement**, then thread `rig::of(st)` through every caller. In `tick_backdoors`: `TraceStarted{hops}` → if firewall ≥ 2, `missions::call::ring(st, Call { story: None, from: "zenbo".into(), lines: vec![format!("trace on your line · {hops} hops out"), "burn the route or ride it out".into()], ..zeroed as console.rs:917 })`; `Raid{wiped: true}` → `st.story("the rig wiped itself before they got the door open · every backdoor is gone")` and no fixer quiet. Every `of SLOTS` print uses `rig::of(st).lines`; the man page says "as many lines as the rig's relay holds".
- [ ] **Step 4: Run** tests — PASS (fix any test that asserted `SLOTS`); workspace check.
- [ ] **Step 5: Commit** `backdoors: lines, hold and firewall from the rig`.

### Task 5: The sled drawing

**Files:**
- Create: `crates/strata-zero/src/ui/hardware/sled.rs` (model: `fuse.rs`)
- Modify: `crates/strata-zero/src/ui/hardware/mod.rs` (`large_size`, `draw_large`, swatch, fitted/native arms, tests)

**Interfaces:**
- Produces: `pub(super) fn draw_large(look, mods, pen)`, `swatch`, as fuse's; `pub fn kind_colour(k: SocketKind) -> Color` (the four kind colours) and `pub fn tier(mods: &[Mod]) -> u8` (1..=3 from the mod's size: Seats/Lines/Hold/Firewall tiers by the spec table's rows), both `pub(crate)` for the rack drawing in Task 6.

- [ ] **Step 1: Failing tests** in `mod.rs` tests: `a_sled_is_drawn_in_its_kind` (large drawing of `SledRelay` has a cell with fg `#50e678`); `tiers_light_more_lamps` (tier 3 has more lit lamp cells than tier 1); sizes match `(52, 18)`.
- [ ] **Step 2–4:** FAIL, implement the faceplate (dark plate, pull handle, vents, lamp row; one/two/three lit pairs by tier; the item name on a label), PASS. Run window build too.
- [ ] **Step 5: Commit** `ui: the sled drawn`.

### Task 6: The rack drawing and the rig's theme

**Files:**
- Create: `crates/strata-zero/src/ui/rack.rs`
- Modify: `crates/strata-zero/src/ui/mod.rs`, `crates/strata-zero/src/ui/device.rs` (`Device::Rig` :153)
- Reference: `docs/superpowers/specs/2026-11-11-rig-upgrades-mock/gen.py` `rack()` (round 2, F)

**Interfaces:**
- Consumes: `Character::rig_frame`, `rig_fitted`, `sled::kind_colour`, `sled::tier`, `FrameSpec`.
- Produces: `pub struct RackArt { pub frame: ItemId, pub bays: Vec<(SocketKind, Option<ItemId>)>, pub picked: Option<usize> /* None = FRAME */ }`; `pub fn size(bays: usize, rows: u16) -> (u16, u16)`; `pub fn draw(art: &RackArt, st: &AppState, area: Rect, buf: &mut Buffer) -> Vec<Rect>` (draws into `area`, returns each bay's cell rect, index 0 = the whole rack, then one per bay, for the dashed outline).

- [ ] **Step 1: Failing tests:** `each_bay_is_a_sled_or_a_plate` (stock rack: three plates labelled CORE/RELAY/LINE in kind colours; pull the relay → a dim `── RELAY ──` plate); `the_head_unit_reads_its_line` (`ZENBO 64  REDY` on the Zenbo); `a_halcyon_has_five_bays` (5 bay rects); `it_fits_100x30` (`draw` into the desk's art rect at 100×30 returns 1+bays rects inside the area); `the_rig_wears_its_racks_theme` (`Look::of(Device::Rig)` style maker `HALCYON` after `set_frame(rack_halcyon)`).
- [ ] **Step 2–4:** FAIL; implement procedurally in the mock's manner (rails, head unit LCD, sleds with handle/vents/lamps and kind+model text, blanking plates, the base per `FrameBase`: milk crate / plinth / foot / rails, the loom); `Device::Rig` uses the frame's `theme`; PASS.
- [ ] **Step 5: Commit** `ui: the rack drawn, the rig in its rack's theme`.

### Task 7: The desk screen

**Files:**
- Create: `crates/strata-zero/src/screens/desk.rs`
- Modify: `crates/strata-zero/src/screens/mod.rs`, `crates/strata-zero/src/screens/popup.rs` (`Pick::Desk` near :104, dispatch near :702), `crates/strata-zero/src/interact.rs` (`FREE_LETTERS` :18 drop `o`; rig rows :386–426 add `Choice::on('o', "Open up", "the rig", AMBER, Pick::Desk)`, disabled with ` · held as evidence` while seized)
- Modify: `crates/strata-zero/src/screens/shots.rs` (new shots)
- Test: `screens/desk.rs` tests

**Interfaces:**
- Consumes: Tasks 2, 3, 6; `game::Action::{Fit, Unfit}` with `Host::Rig`; `actions::perform`.
- Produces: `pub struct Desk { row: usize, picker: Option<usize>, card: u64 }`; `Desk::open(st: &AppState) -> Desk`; `impl Screen for Desk`; `pub fn fits_here(st: &AppState, row: usize) -> Vec<Pick>` (row 0 = frames in `frames` other than the fitted one; else sled copies in the pack whose `fits_socket()` is that bay's kind); `pub const FIT_MINUTES: u64 = 10`.
- `actions::perform` refuses `Fit/Unfit{host: Host::Rig}` unless a new `st.at_desk: bool` (UI flag, `_` in snapshot with "UI") is set; `Desk` sets it on open and clears on close.

- [ ] **Step 1: Failing tests:** `o_opens_the_desk_at_home` (gather at the rig has an `o` row, not when seized: disabled with `held as evidence`); `space_lists_what_fits` (Tinman in pack, row 1 → one pick, card text `seats 1 ▸ 2`); `fitting_swaps_and_runs_the_clock` (fit → bay holds Tinman, ZB-8 in pack, game minute +10, log `you seat the Tinman 2X core · 10m`); `x_pulls_without_asking`; `rig_fits_are_refused_off_the_desk` (perform Fit Host::Rig with `at_desk` false → Err); `changing_frame_sends_extras_to_the_pack` (via row 0); `the_desk_fits_100x30` (draw at 100×30: every bay name in the buffer text).
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement** per spec §2 (layout of `round2.html` F: header, BAYS list with FRAME first, rack art, dashed amber outline on the picked rect, side-panel card + `FITS HERE · IN YOUR PACK` / `IN YOUR FLAT`, bottom row `at your rig · ↑↓ bays · space what fits · x pull · esc stand up`). Advance time with the same helper rest/fitting uses to run the clock (`clock::run` for 10 minutes' ticks).
- [ ] **Step 4: Run** tests — PASS. Add shots `desk_stock_{100x30,120x40}`, `desk_civitas_firewall_list_*`, `desk_halcyon_full_*`; render with `SHOT_DIR=… cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored desk` and look at the PNGs.
- [ ] **Step 5: Commit** `screens: the desk`.

### Task 8: Getting parts

**Files:**
- Modify: `crates/strata-zero/src/jobs/ui.rs` (`bonus` :319, `hand_in` :324), `crates/strata-zero/src/jobs/mod.rs` (`handin_item` :182 accepts sifted shards), `flavor/` (a short list of rack-delivery mail lines; put them in `flavor/jobs.ron` beside the fixers' lines)
- Test: `jobs/` tests

**Interfaces:**
- Consumes: `Character.rig_rewards`, `frames`; `jobs::standing_word`/`Jobs::standing`.
- Produces: `fn rig_rewards(st: &mut AppState, fixer: FixerId, before: i32, after: i32, kind: &Kind) -> Vec<(ItemId, String)>` in `jobs/ui.rs`, called in `hand_in` after the standing changes; its strings join the hand-in line (`· a Tinman 2X core`).

- [ ] **Step 1: Failing tests:** `knowing_you_pays_a_sled_once` (standing crosses to known → Tinman given, second fixer → Switchback, same fixer again → nothing; after a drop and climb back → nothing); `the_first_trust_sends_the_civitas` (crosses to trusted → `frames` gains `rack_civitas`, a mail from that fixer containing `it's at your door`; second → `rack_lotus`; third → nothing); `inner_circle_sends_the_halcyon_once`; `first_plant_pays_sniff_first_copy_pays_keepalive`; `a_sifted_shard_hands_in` (`handin_item` accepts `paydata_sifted` for Fetch/Copy).
- [ ] **Step 2–4:** FAIL, implement, PASS. Rewards are stored in `st.character.rig_rewards` (keyed by fixer name).
- [ ] **Step 5: Commit** `jobs: fixers pay sleds, racks and rig programs`.

### Task 9: Manual, slice one

**Files:** `manual/09-the-rig-and-helix.md` (new "## Working on the rig"), `manual/08-the-net.md` "## Backdoors", `manual/11-fixers-jobs-and-the-story.md` "## Standing".

- [ ] **Step 1:** Write per spec §7 (the desk keys as a table matching Task 7, bays and sleds, the four racks, what each kind does with the stock numbers, where parts come from).
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke manual::` and the glyph test with `STRATA_FONT_CHECK=1 cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character_has_a_glyph` — PASS.
- [ ] **Step 3: Commit** `manual: working on the rig`.

## Slice two: jobs while you sleep

### Task 10: `RigJobs` and the four jobs

**Files:**
- Create: `crates/strata-zero/src/rig/jobs.rs`
- Modify: `crates/strata-zero/src/backdoors.rs` (`Backdoors.jobs: RigJobs`, `#[serde(default)]`), `crates/strata-zero/src/net.rs` (`tick_backdoors` calls `rig::jobs::advance`), `crates/strata-zero/src/credentials.rs` (`Source::Sniffed`, label `"sniffed off the line"`)
- Test: `rig/jobs.rs` tests

**Interfaces:**
- Consumes: `rig::of`, `Backdoors`, `credentials::Journal::learn`, `Why::{Decay, Noticed}`.
- Produces:
  - `pub enum JobKind { Sniff(BuildingId), Sift { shard: ItemId }, Keepalive(BuildingId), Rent }`
  - `pub struct Job { pub n: u32, pub kind: JobKind, pub started: u64, pub last: u64, pub progress: u64, pub found: Vec<String> }`
  - `pub struct RigJobs { pub running: Vec<Job>, pub heat: BTreeMap<BuildingId, u32> /* quarter-points */, pub report: Vec<String>, pub programs: BTreeSet<String>, pub worked_since_boot: u32, pub warned: BTreeSet<BuildingId>, pub next: u32, pub mailed_day: u64 }` (`Default` has `programs = {"sift", "rent"}` via a custom default fn)
  - `pub fn start(st: &mut AppState, kind: JobKind) -> Result<u32, String>` (seat check `"run: every seat is busy · kill a job, or fit a bigger core"`, Net-only check `"sniff: a key won't hold that long"`, sift checks `"sift: already sifted"`; sift removes the shard from the pack)
  - `pub fn kill(st: &mut AppState, n: u32) -> Result<String, String>`
  - `pub fn advance(st: &mut AppState, now: u64)` (hour steps from each job's `last`; outcomes seeded by `(run seed, job n, hour)`)
  - `pub fn trim_to_seats(st: &mut AppState) -> Vec<String>` (kills newest over `rig::of(st).seats`)
  - `pub fn kill_all(st: &mut AppState, why: &str)`
- Numbers (consts in `jobs.rs`): `SNIFF_HOURS = 3`, `SIFT_HOURS = 6`, `SIFT_VAULT_HOURS = 10`, `RENT_PER_HOUR = 6`, heat per hour sniff 4 / keepalive 1 (quarter-points), `HEAT_WARN = 18*4`, `HEAT_PATCH = 36*4`, cooling 4 per hour with no job on it; hours divided by `speed/100`.

- [ ] **Step 1: Failing tests:** `sniff_learns_a_flat_every_three_hours` (after 9h: 3 codes, `Source::Sniffed`); `sniff_ends_with_nothing_left`; `sift_gives_codes_and_returns_a_sifted_shard` (6h: 2 codes, `paydata_sifted` in pack; vault dump 10h: 4 codes); `a_stash_one_time_in_three` (over seeds, roughly; a stash puts a component on that flat's floor, the code known, and a stash mark); `keepalive_freezes_the_life` (health unchanged after 48h with it; resumes after kill); `rent_pays_by_the_hour` (6/h; ×1.5 with Q-Stack); `noise_patches_at_36` (sniff 36h → backdoor burned `Why::Noticed`, job ended, report has the warning at 18h once); `a_dead_line_ends_the_job` (past real end → `Why::Decay`, `the line went dead`); `bait_ends_at_the_first_hour` (honeypot → `nothing on the line but bait`, still honeypot); `catch_up_matches_ticking_through` (advance once by 30h == advance hourly 30 times: same codes, heat, report); `losing_seats_kills_the_newest_jobs`; `a_seized_rig_kills_every_job`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke rig::` — FAIL.
- [ ] **Step 3: Implement.** Stash placement reuses the Fetch placement (`jobs/generate.rs:256`) and the map's mark; the journal's `copy_codes`-style single-flat learn. `tick_backdoors` calls `advance` every minute (cheap when no hour has passed) and `kill_all` on a raid/seizure. The desk (Task 7) calls `trim_to_seats` after a fit/pull/frame change.
- [ ] **Step 4: Run** — PASS; workspace check; `cargo test -p strata-zero --target-dir target/smoke runs::` (fixtures still open).
- [ ] **Step 5: Commit** `rig: jobs while you sleep`.

### Task 11: HELIX `run`, `ps`, `kill`, programs, boot line

**Files:**
- Modify: `crates/strata-zero/src/helix/bin.rs` (`PROGRAMS` :20, `program` :525, `programs` :557), `crates/strata-zero/src/helix/shell.rs` (`PAGES` man entries for run, ps, kill), `crates/strata-zero/src/helix/hosts.rs` (`rig_disk` :321: `/opt/jobs/` listing installed programs), `crates/strata-zero/src/screens/terminal.rs` (`new_mail_line` :400 → also the jobs line; install shards on open)
- Test: `helix/bin.rs` tests, `screens/terminal.rs` tests

**Interfaces:**
- Consumes: Task 10's `start`, `kill`, `RigJobs`.
- Produces: `ps` output exactly the mock's columns: header `" #  JOB        ON                      UP      FOUND"`, one line per job, then `"{core} · {busy} of {seats} seats busy"`; boot line `"{n} JOBS RAN WHILE YOU WERE OUT · TYPE ps"`; install line `"INSTALLED {prog} FROM SHARD"`.

- [ ] **Step 1: Failing tests:** `run_ps_kill_on_the_rig` (type `run rent`, `ps` lists it, `kill 1` ends it); `rig_only_elsewhere` (`ps` on a district host → `ps: rig only`); `run_unknown_job` → `run: no such job · ls /opt/jobs`; `sniff_needs_its_shard` (not installed → that error; open terminal carrying `prog_sniff` → `INSTALLED sniff FROM SHARD`, shard gone, `programs` has it); `the_boot_says_jobs_ran` (after a job worked, reopening the rig shows the line; once); `man_pages_exist` (the shell's existing test passes with the new programs).
- [ ] **Step 2–4:** FAIL, implement, PASS.
- [ ] **Step 5: Commit** `helix: run, ps and kill on the rig`.

### Task 12: The 06:00 mail, manual, shots

**Files:**
- Modify: `crates/strata-zero/src/rig/jobs.rs` (`fn morning_mail(st: &mut AppState, now: u64)` called from `advance`), `manual/09-the-rig-and-helix.md` ("## Jobs", `run`/`ps`/`kill` rows in "Only on your rig"), `crates/strata-zero/src/screens/shots.rs` (`rig_ps_*`, `cron_mail_*`)

**Interfaces:**
- Consumes: `credentials::Mail`, `ui::hud::minute_at`/`day_at`.
- Produces: a `Mail { from: "cron", subject: format!("overnight · {n} jobs"), body: report grouped by job, at: now, read: false, job: None, story: None }` at the first minute ≥ 06:00 of each day with a non-empty report; `mailed_day` stops a second one.

- [ ] **Step 1: Failing tests:** `the_cron_mail_lands_at_six` (report lines at 02:00 → no mail until 06:00, then one mail, report cleared); `no_report_no_mail`; `away_lands_on_return` (home city set aside for two days, catch-up lands one mail).
- [ ] **Step 2–4:** FAIL, implement, PASS.
- [ ] **Step 5:** Manual "## Jobs" per spec §5/§7; run the manual tests and glyph test.
- [ ] **Step 6:** Shots, rendered and looked at (both 100×30 and 120×40).
- [ ] **Step 7: Commit** `rig: the morning mail; manual: jobs`.

### Task 13: Whole-branch check

- [ ] **Step 1:** All five Global Constraints commands, plus `cargo check -p strata-zero --features window --target x86_64-pc-windows-msvc --target-dir target/smoke` and the linux target.
- [ ] **Step 2:** Render every new shot with `SHOT_FX=1` off and look at them; compare the desk to `round2.html` F.
- [ ] **Step 3:** Fix anything found; commit `fix: …` per finding.
