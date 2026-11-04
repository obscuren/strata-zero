# Mail in the terminal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mail at the rig reads like a mail program: the boot says what's new, `mail`
opens HELIX mail on the glass, `mail N` prints a readable mail.

**Architecture:** A new `helix/mailer.rs` holds the program (inbox and mail views, keys,
drawing on the glass), modelled on `helix/nano.rs`; the terminal screen opens and routes
keys to it as it does nano, through a new `Output.open_mail` flag from the shell's
`mail`. `access::mail_open` gains the readable print; the terminal's greeting gains the
new-mail line.

**Tech Stack:** Rust, ratatui cells, crossterm keys.

**Spec:** `docs/superpowers/specs/2026-11-04-mail-in-the-terminal-design.md`

## Global Constraints

- Build only into `--target-dir target/smoke`; never the default `target/`.
- The five checks, both front ends, pass at the end of every task:
  `cargo fmt --all --check`;
  `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`;
  `cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings`;
  `cargo test --workspace --target-dir target/smoke`;
  `cargo test --workspace --features strata-zero/window --target-dir target/smoke`.
- Draw through ratatui only, in the terminal look's own colours (`look.text()`,
  `look.text_dim()`, `look.glass()`), as nano does; `▸` and `─` are in the glyph set.
- Every read goes through `credentials::mail_read` (it ends a story step).
- Space confirms, esc backs out; enter does nothing new.
- The worktree sandbox refuses compound shell commands: plain separate commands.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Never bare
  `git stash`. No authored names in non-test Rust.

## Review Focus

1. A story mail read in the program ends its story step, and the mail that step sends
   lands in the open inbox (Task 2 test).
2. ↑↓ past either end of the inbox, and an inbox longer than the glass (Task 2 test).
3. `n` with nothing new left: no read, the status says so (Task 2 test).
4. `mail` with an empty inbox prints `NO MAIL` and opens nothing (Task 2 test).
5. The 100×30 glass: rows and title cut, nothing drawn outside the glass (Task 2 test).

---

### Task 1: The boot line and `mail N`, readable

**Files:**
- Modify: `crates/strata-zero/src/screens/terminal.rs` (`motd`, ~305: on `HostId::Rig`
  with unread mail, append a blank line and `YOU HAVE {n} NEW MAIL · TYPE mail`)
- Modify: `crates/strata-zero/src/access.rs` (`mail_open`: spec §3's format)
- Modify: `crates/strata-zero/src/helix/bin.rs` (`mail`: `mail N` no longer upper-cases
  the first two lines; it prints `mail_open`'s lines as they are; `cat` of a mail file,
  which reads through `mail_open`, prints the same — update its tests)
- Test: `terminal.rs`, `access.rs`, `bin.rs` tests

**Interfaces:**
- Produces: `pub fn mail_time(m: &credentials::Mail) -> String` (`hh:mm` from
  `jobs::day_minute(m.at)`) in `access.rs`, used by Task 2.

- [ ] **Step 1: Failing tests.** `the_rig_boots_with_the_new_mail` (one unread → the
  greeting's lines end `["", "YOU HAVE 1 NEW MAIL · TYPE mail"]`; all read → no such
  line; a district terminal → none). `mail_n_prints_a_head_block` (an unread mail:
  `MAIL 1 OF 1 · NEW`, `FROM     MEZZ`, `SUBJECT  THE NEWS`, `TIME     hh:mm`, a 44-`─`
  rule, the body, the rule, `NO NEW MAIL`; read again → no `· NEW`; with another unread
  → `1 NEW LEFT · mail`). Update the existing `mail` tests in bin.rs (~828-877) that
  assert the old format.
- [ ] **Step 2: Run; they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run; they pass. The five checks.**
- [ ] **Step 5: Commit** `mail: the rig says what's new, mail N reads like mail`.

### Task 2: HELIX mail on the glass

**Files:**
- Create: `crates/strata-zero/src/helix/mailer.rs` (`pub struct Mailer`, views
  `Inbox { sel, top }` / `Reading { i, scroll }`, `pub fn open(st) -> Option<Mailer>`,
  `pub fn key(&mut self, st: &mut AppState, k: KeyEvent) -> MailerEvent`,
  `pub fn draw(&self, st: &AppState, buf: &mut Buffer, glass: Rect, look: &Look)`;
  `pub enum MailerEvent { Stay, Closed }`)
- Modify: `crates/strata-zero/src/helix/mod.rs` (the module)
- Modify: `crates/strata-zero/src/helix/shell.rs` (`Output.open_mail: bool`, carried
  through pipelines and `;` as `open_nano` is)
- Modify: `crates/strata-zero/src/helix/bin.rs` (`mail` with no args: `NO MAIL` when the
  inbox is empty, else `open_mail`); delete `access::mail_inbox` and its tests (nothing
  else lists the inbox)
- The spec's "Builder rulings" bind this task: `{i}` is the journal index (1 = oldest),
  the bar and the open mail pinned by journal index, per-line body wrap, the narrow-glass
  cut
- Modify: `crates/strata-zero/src/screens/terminal.rs` (`mailer: Option<Mailer>`; open on
  `open_mail`; every key is its while open, as nano's; draw it on the glass, the
  scanlines and dead cells after, as nano's)
- Test: `mailer.rs` tests (model and drawing into a `Buffer`), `terminal.rs` tests (open,
  keys routed, close back to the prompt)

**Interfaces:**
- Consumes: Task 1's `access::mail_time`.
- Produces: `Mailer`, `MailerEvent`, `Output.open_mail`.

Exact copy (spec §2): title `"  HELIX mail 2.2   INBOX · {n} mail · {k} new"` and
`"  HELIX mail 2.2   {i} of {n}"` + `" · NEW"`; rows `"  ▸ NEW   "` / `"          "` then
`"{hh:mm}  {from:<12}  {subject}"`; status `"  [ {k} new ]"`, `"  [ {k} new left ]"`,
`"  [ no new mail ]"`; keys `"space Read     n Next new    ↑↓ Move"` / `"esc Exit"`
and `"n Next new     ↑↓ Other mail"` / `"esc Inbox"`; mail rows `"  From:     "`,
`"  Subject:  "`, `"  Time:     "`.

- [ ] **Step 1: Failing tests.** `opens_on_the_newest_new_mail`;
  `space_reads_and_marks_read` (through `mail_read`: a story mail's step ends);
  `the_step_s_mail_lands_in_the_open_inbox` (Review Focus 1);
  `n_reads_the_newest_new_and_says_when_none` (Review Focus 3);
  `arrows_stop_at_the_ends_and_scroll_a_long_inbox` (Review Focus 2);
  `esc_backs_out_to_the_inbox_then_closes`; `mail_with_no_mail_prints_no_mail`
  (Review Focus 4); `the_inbox_draws_its_rows_on_the_glass` (title, a NEW row, the
  cursor bar's inverse style, key rows); `it_fits_the_100x30_glass` (Review Focus 5);
  `the_terminal_routes_keys_to_the_mailer_until_it_closes`.
- [ ] **Step 2: Run; they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run; they pass. The five checks, `every_drawn_character_has_a_glyph`
  among them.**
- [ ] **Step 5: Commit** `mail: HELIX mail, a program on the glass`.

### Task 3: Shots

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (a new ignored `mail_shots`)

- [ ] **Step 1: Write `mail_shots`** (spec §5) through the real story with
  `missions::tests::the_morning_call`, `the_news_is_on`, `news::switch_off`, older mails
  read; at 120×40 and 100×30: `mail_boot`, `mail_inbox`, `mail_read` (space on Mezz's
  mail: Seren's lands, `[ 1 new left ]`), `mail_n` (`mail 4` printed). Assert in each
  what it shows.
- [ ] **Step 2: Run it with `SHOT_DIR`, look at every PNG, fix what reads wrong.**
- [ ] **Step 3: The five checks; commit** `shots: mail in the terminal`.
