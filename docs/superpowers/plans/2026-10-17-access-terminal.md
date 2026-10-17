# The Access Terminal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A typed MUTHUR-style session in a CRT overlay at street terminals (the district's buildings: scan, connect, who, plan, log, and door/lift/camera control by tier) and on the home rig (mail, journal, your block), with the game's first mail and job.

**Architecture:** `access.rs` holds the sessions and commands as plain functions over the app state, returning text replies. `screens/terminal.rs` is the one screen that hosts either session: the CRT from the device kit, a scrollback, typed-out replies and the dev console widget for input. Tiers come from credentials already held; world commands land hacks through the Net state so the Net and the terminal share one truth. The first mail lives in the journal and seeds a quest.

**Tech Stack:** Rust 2024, ratatui, the workspace crates. `cargo test -p strata-zero`, `cargo clippy --workspace --all-targets -- -D warnings`.

**Spec:** `docs/superpowers/specs/2026-10-17-access-terminal-design.md`

## Global Constraints

- Never rebuild `target/release/strata`.
- Commit messages in the repo's voice.
- Log lines mark names with backticks; terminal replies are plain upper-case period text, no backticks.
- CRT 72×24 centred; `TYPE_CPS` 60; ICE grades: none < 160, white < 200, grey < 230, black ≥ 230 (`citygen::construct::{ICE_THRESHOLD, GREY_THRESHOLD, BLACK_THRESHOLD}`).
- Denial wording exactly `ACCESS DENIED · CLEARANCE: <tier> · REQUIRED: <tier>`.

## Review Focus

1. A district with no constructs (all open buildings) must give SCAN an honest "NO CONSTRUCTS ON THIS SUBNET", never an empty prompt. Pinned in Task 2.
2. `CONNECT` with a name prefix matching several buildings must list them, not pick the first silently. Pinned in Task 2.
3. `PLAN` on a building whose ground floor is the player's current place must not wipe what is visible: only `Unseen` becomes `Remembered`. Pinned in Task 2.
4. A terminal keyed by tile must boot once per tile, so two street terminals each boot once. Pinned in Task 4.
5. `move_in` at a load (player respawned) must not add a second mail, a second shard or a second quest. Pinned in Task 3.

---

### Task 1: The CRT and scanlines in the device kit

**Files:** Modify `crates/strata-zero/src/ui/device.rs`.

**Interfaces:** Produces `pub const CRT_W: u16 = 72; pub const CRT_H: u16 = 24;` `pub fn crt(buf, area: Rect, badge: &str, led: Led) -> Rect` (draws the bezel and a dark screen inset by 2 columns and 2 rows, returns the screen rect), `pub fn scanlines(buf, area: Rect)` (alternate rows' fg scaled to 0.78 via `crate::render::Rgb` when the fg is an Rgb).

- [ ] Test `the_crt_has_a_dark_screen_inside_the_bezel_and_scanlines_dim_alternate_rows`: `crt` on a 72×24 area returns `Rect::new(x+2, y+2, 68, 20)`, the screen cells have bg `LCD_BG`, the badge and LED are there; after writing a phosphor char on rows 3 and 4 of the screen and calling `scanlines`, row 3 keeps `PHOSPHOR` and row 4 is darker.
- [ ] Run → compile error. Implement. Run → pass. Commit `feat(app): a crt and scanlines in the device kit`.

---

### Task 2: Sessions and commands

**Files:** Create `crates/strata-zero/src/access.rs`; register `mod access;` in `main.rs`; modify `credentials.rs` (`Source::Mail`, `Mail`, `Journal::mails`, `Journal::booted`); modify `walker/mod.rs` (`Player::remember_place(place, size, tiles: impl Iterator<Item = Pos>)`).

**Interfaces:**
```rust
pub enum Tier { Guest, User, Admin }            // Ord; label() -> "GUEST" | "USER" | "ADMIN"
pub fn tier_on(st: &AppState, b: BuildingId) -> Tier
pub enum Session {
    District { district: DistrictId, connected: Option<BuildingId> },
    Rig { building: BuildingId },
}
pub struct Reply { pub lines: Vec<String>, pub denied: bool }
pub fn boot_lines(st: &AppState, s: &Session) -> Vec<String>
pub fn prompt(st: &AppState, s: &Session) -> String
pub fn verbs(st: &AppState, s: &Session) -> Vec<&'static str>   // allowed now, for HELP and Tab
pub fn run(st: &mut AppState, s: &mut Session, line: &str) -> Reply
pub fn ice_grade(security: u8) -> &'static str                  // "none" | "white" | "grey" | "black"
pub fn district_of_terminal(city: &City, pos: Pos) -> Option<DistrictId>  // the street tile's district
```
`Journal` gains `pub mails: Vec<credentials::Mail>` (`Mail { from: String, subject: String, body: Vec<String>, read: bool, at: u64 }`) and `pub booted: BTreeSet<(Place, Pos)>`. `Source::Mail` at the end of `Source` ("in a mail from `Dez`" in the journal line; `whose` as ServerCopy).

Commands (`run` upper-cases the line, splits on whitespace, dispatches on the first word):
- `HELP`: one line per verb from `verbs`, with a gloss from a static table.
- `EXIT`: `Reply { lines: vec!["BYE".into()], denied: false }` and the screen pops (the screen checks for the verb).
- District: `SCAN`, `CONNECT [n|prefix]`, `DISCONNECT`; connected or Rig: `WHO`, `PLAN`, `LOG`, `DOOR UNLOCK`, `LIFT`, `CAM LOOP`, `ALARM OFF`; Rig only: `MAIL [n]`, `JOURNAL`, `SCAN`.
- Tier gate: `DOOR`, `LIFT`, `CAM` need `User`; `ALARM` needs `Admin`; denial reply `denied: true` with the two lines.
- `SCAN` lists `city.buildings` with `district == d` and `kind.is_closed()` and a server node (`city.net.nodes.iter().any(|n| n.building == Some(b) && n.kind == Server)`), numbered from 1 in building order; security from `citygen::construct::security_of` (make it `pub` if it is not) or `st.constructs.ensure`'s security: use `citygen::Construct::security_of(city, b)` if exposed, else generate via `st.constructs.ensure(city, b).map(|c| c.security)`. Prefer the pure citygen function; expose it as `pub fn security_of` if needed.
- `WHO`: `st.world.inside(b)` → `NAME  profession  at work|at home|a guest|visiting`; empty → `NOBODY INSIDE`.
- `PLAN`: `st.interiors.ensure(city, b, 0)` then `player.remember_place(Place::Floor{b,0}, floor.tiles.size(), floor.tiles.iter().map(|(p,_)| p))` → `GROUND FLOOR PLAN LOADED`.
- `LOG`: `st.net.hacks` for `b`, newest first, `HH:MM  <target kind>  <OP>  (EXPIRES HH:MM)`; plus `TRACE ALERT` if `st.net.alert(b, now)`; empty → `NO ENTRIES`. Times via `crate::ui::hud::clock_at(minute * TICKS_PER_MINUTE)`.
- `DOOR UNLOCK`: find the building's DoorLock node; `st.net.land(b, HackTarget::Node(id), Target::Node(NodeKind::DoorLock), Op::Spoof, now)`; log "`<name>`'s locked doors open for an hour"; reply `DOOR LOCKS RELEASED · 60 MIN`. No lock node → `NO DOOR CONTROLLER`.
- `LIFT`: same with LiftCtrl → `LIFTS ANSWERING`. `CAM LOOP`: every Camera node → Blind → `N CAMERAS LOOPED`; none → `NO CAMERAS`.
- `ALARM OFF`: `st.net.trace(b).alert_until = Some(now)` (or whatever field ends the alert; read `Trace`) → `ALARM CLEARED`, or `NO ALARM` when not alert.
- Rig `MAIL`: list `n  [*] FROM  subject`; `MAIL n`: body lines, `read = true`, and `credentials::mail_read(st, n)` ticks the quest's first objective. `JOURNAL`: `credentials::entries` groups → header then lines with backticks stripped (`messages::parse_marks`). `SCAN` on the rig: the one building.

- [ ] Tests in `access.rs` (one `state()` helper like keypad's; a district with a residential building): `tiers_come_from_what_you_hold` (guest; card → user; code → user; `st.net.land(.. Op::Backdoor ..)` → admin; rig session prompt says HOME), `scan_lists_constructs_with_grades_and_connect_picks_one` (numbered rows, `CONNECT 1`, `CONNECT <prefix>`, ambiguous prefix lists candidates and stays unconnected, `CONNECT ZZZ` → `NO SUCH BUILDING`), `who_plan_and_log_read_the_building` (WHO names someone inside at 03:00 using `seed_world`-like advancing or accept `NOBODY INSIDE` and assert the format; PLAN marks the ground floor `Remembered` and leaves `Visible` tiles alone when standing there; LOG `NO ENTRIES` then a landed hack line), `world_commands_need_user_and_land_hacks` (DOOR UNLOCK at guest → denied wording; give a card → `st.net.unlocked(b, now)`; LIFT → `lifts_answer` during an alert; CAM LOOP → every camera `blinded`; ALARM OFF needs admin, then clears `alert`), `syntax_help_and_not_connected`, `ice_grade_thresholds`.
- [ ] Run → compile error. Implement. Run → pass. Commit `feat(app): the access terminal's sessions and commands`.

---

### Task 3: The first mail, the shard and the quest

**Files:** Modify `credentials.rs` (`first_mail`, `mail_read`, `shard_taken`, `move_in` calls `first_mail`), `actions.rs` (after a successful `Take`/`TakeAll`, call `credentials::shard_taken(st, at.place, at.pos)`), `flavor/content.ron` (quest `first_favour`), `game` if quest logs need a push helper (`Character::add_quest(id, objectives: usize)`).

**Interfaces:** `pub fn first_mail(st: &mut AppState)` (idempotent: returns if a mail from the fixer exists), `pub fn mail_read(st: &mut AppState, index: usize)`, `pub fn shard_taken(st: &mut AppState, place: Place, pos: Pos)`; `Journal::favour: Option<Favour { flat: (BuildingId, i32, u16), tile: Pos, fixer: String }>`.

- [ ] Tests in `credentials.rs`: `the_first_mail_names_a_neighbour_and_drops_the_shard` (state_home-like: `enter_city` on seed 8 Medium 7 districts, then `move_in`; one mail, unread, body mentions the code; the journal has the code with `Source::Mail`; the shard lies on a walkable tile inside that flat on the home megablock; the quest `first_favour` is in `st.character.quests` with two objectives undone; a second `move_in` adds nothing), `reading_and_taking_tick_the_favour` (`mail_read(0)` ticks objective 0; `shard_taken` at the tile with the shard gone from the ground ticks objective 1 and logs "the shard is yours").
- [ ] Run → fail. Implement. Run → pass. Commit `feat(app): the first mail: a neighbour's code and a shard to pick up`.

---

### Task 4: The terminal screen

**Files:** Create `crates/strata-zero/src/screens/terminal.rs`; register in `screens/mod.rs`.

**Interfaces:** `pub struct Terminal`, `Terminal::open(st: &mut AppState, session: Session, key: (Place, Pos)) -> Terminal` (queues the boot lines if `key` is not in `journal.booted`, then inserts it), `pub const TYPE_CPS: u64 = 60;`, `pub(crate) fn text(&self) -> Vec<String>` (test aid: the scrollback plus the typing reply so far).

Behaviour: keys go to the `ui::console::Console` widget with `complete = |head, word| verbs starting with word when head is empty`; `Run(line)` → echo `<prompt><line>` to the scrollback, `access::run`, queue the reply's lines as the typing reply with its start tick; while a reply types, any key finishes it (and is not otherwise handled); `EXIT` pops after the reply is shown; `Close` (Esc) pops. Draw: `crt` with the badge by session and the LED (green while typing, red for `DENIED_TICKS` after a denied reply, amber otherwise), the last `screen.height - 1` scrollback rows, the prompt row `prompt + input + '▮'` (and the hint row above it when `console.hint` is set), then `scanlines`.

- [ ] Tests: `boots_once_per_terminal_and_types_the_reply` (open twice with the same key: the second has no boot lines; a different key boots; after `Run("HELP")` the scrollback has the echo and the reply types: at tick+1 fewer chars than at tick+1000; a key press finishes it), `the_prompt_shows_the_tier_and_esc_leaves` (draw contains `LOWTOWN NET >`-style prompt from `access::prompt`; Esc → Pop; `EXIT` → Pop after the reply), `a_session_walks_scan_connect_who_on_screen` (typed through the console: `SCAN`, `CONNECT 1`, `WHO`, the draw shows the building name in the prompt).
- [ ] Run → compile error. Implement. Run → pass. Commit `feat(app): the terminal screen: a crt, a prompt and typed replies`.

---

### Task 5: Reaching it from Space

**Files:** Modify `screens/popup.rs` (`Pick::Terminal { session: Session, key: (Place, Pos) }` → `Replace` with `Terminal::open`), `screens/explore.rs` (`interact`: beside a street terminal add `access terminal` with `Session::District { district: district_of_terminal(city, jack_pos), connected: None }`, key `(place, jack_pos)`; beside the rig add `access rig` with `Session::Rig { building: home.building }`, key `(place, home.rig)`).

- [ ] Tests in `explore.rs`: `space_at_a_street_terminal_offers_access_terminal` (the existing jack-in test setup; the popup shows `access terminal`; firing it yields `Replace`; drawing the new screen shows `MUNI-OS`), `space_at_the_rig_offers_access_rig` (state_home; stand beside the rig; `access rig` row; `Replace`; the screen shows `HOME RIG` and `MAIL` lists the fixer's mail).
- [ ] Run → fail. Implement. Run → pass. Commit `feat(app): access terminal and access rig from the space menu`.

---

### Task 6: Docs and the whole-workspace check

- [ ] `cargo test --workspace`, clippy `-D warnings`, the ignored fuzz.
- [ ] README: a paragraph on the access terminal and the rig's mail. Spec status "built on `feat/access-terminal`". Commit `docs: the access terminal`.
