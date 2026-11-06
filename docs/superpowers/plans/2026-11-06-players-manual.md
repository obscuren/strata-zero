# The player's manual · implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a manual written once in `manual/*.md`, read in the game on a MANUAL screen and
published from the same files on stratazero.io.

**Architecture:** the chapters are compiled in with `include_str!`; a small parser turns
the manual's Markdown into blocks; the MANUAL screen lays the blocks out for its width.
The site's side is the website repo's work and is not in this plan.

**Tech stack:** Rust, ratatui, crossterm. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-11-06-players-manual-design.md`

## Global constraints

- Colours are `Color::Rgb` from `ui::chrome` (CYAN, TEXT, DIM, FAINT, MAGENTA, PANEL_BG).
- Fits 100×30 (`app::MIN_W`, `app::MIN_H`); draws nothing outside its frame at any size.
- Space confirms in windows; menus keep enter as they do today.
- Builds into `--target-dir target/smoke` of this worktree; both front ends checked.
- No dev-mode keys or screens in the chapters.

## Review focus

- A chapter line longer than the page: wraps, never cut off.
- A table wider than the page: the last column wraps under itself; a too-wide first
  column is capped so the last keeps room.
- A terminal at exactly 100×30 and a frame smaller than that: nothing panics.
- Scrolling past the end and before the start: clamped; changing chapter starts at the top.
- Unsupported Markdown in a chapter (a link, `*italic*`, a fenced block): a test names
  the file and line.

---

### Task 1: The manual's Markdown

**Files:** create `crates/strata-zero/src/manual/mod.rs`, `crates/strata-zero/src/manual/markdown.rs`; add `mod manual;` to the crate root.

**Produces:**
- `pub enum Block { Title(Vec<Span>), Heading(u8, Vec<Span>), Para(Vec<Span>), List(Vec<Vec<Span>>), Steps(Vec<Vec<Span>>), Table { head: Vec<Vec<Span>>, rows: Vec<Vec<Vec<Span>>> }, Tip(Vec<Span>) }`
- `pub enum Span { Text(String), Code(String), Bold(String) }`
- `pub fn parse(src: &str) -> Result<Vec<Block>, (usize, String)>` (1-based line, why)
- `pub struct Chapter { pub slug: &'static str, pub title: String, pub blocks: Vec<Block> }`
- `pub fn chapters() -> &'static [Chapter]` (parsed once), `pub fn find(slug) -> usize`

- [ ] Tests: `parses_every_block_kind`, `rejects_links_images_html_italics_and_fences` (each names its line), `a_title_only_on_the_first_line`, `tables_need_a_rule_row_and_two_or_three_columns`, `every_chapter_parses`, `the_list_in_code_is_the_folder` (reads `manual/` from `CARGO_MANIFEST_DIR`).
- [ ] Implement; run `cargo test -p strata-zero --target-dir target/smoke manual::`.
- [ ] Commit.

### Task 2: The MANUAL booklet

Revised after the mock: look B, the open spread (spec §3).

**Files:** create `crates/strata-zero/src/manual/layout.rs` (blocks → rows at a width, each marked whether it may end a page), `crates/strata-zero/src/manual/book.rs` (fixed pages, 40×22 of text; page 1 the contents; chapters open on new pages) and `crates/strata-zero/src/screens/manual.rs` (the cover, the spreads, the keys); register in `screens/mod.rs`.

**Produces:** `pub struct Manual`, `Manual::new() -> Manual` (the cover), `Manual::at_chapter(slug) -> Manual`; `impl Screen`; `book::book() -> &'static Book` with `starts`, `page(n)`, `spread_of(n)`, `spreads()`.

- [ ] Tests: `the_booklet_fits_every_size_without_panicking`, `the_cover_opens_on_the_contents`, `the_contents_turn_to_the_picked_chapter`, `pages_turn_and_stop_at_the_ends`, `brackets_go_chapter_to_chapter`, `every_page_draws_inside_its_sheet`; in `book`: `chapters_open_on_new_pages_numbered_after_the_contents`, `a_heading_never_ends_a_page`; in `layout`: `long_lines_and_wide_tables_wrap`.
- [ ] Implement, with a `manual_pages` shot test in `screens/shots.rs` to look at every chapter's opening spread.
- [ ] Commit.

### Task 3: Ways in

**Files:** `screens/title.rs` (Entry::Manual before QUIT), `screens/pause.rs` (Row::Manual after SETTINGS), `screens/explore.rs` and `screens/help.rs` (space while the key list is up opens the Controls chapter; the key list's last line says so).

- [ ] Tests: the title and pause rows open a `Manual`; space with help up pushes the manual and closes help; existing row lists updated.
- [ ] Commit.

### Task 4: Keys in step

**Files:** `screens/help.rs` (fix what the crawl found apart from the handlers), `manual/mod.rs` test.

- [ ] Test `the_on_foot_table_is_the_key_list`: the key cells of the Controls chapter's "On foot" table equal `help::WALK`'s keys with the dev keys left out.
- [ ] Commit.

### Task 5: The chapters

**Files:** `manual/README.md` and `manual/NN-*.md`; `window/font.rs` glyph scan reads `manual/*.md`.

- [ ] Write the thirteen chapters from the crawl's facts, second person, plain words, leaving out what a player can't reach yet.
- [ ] Run every test and both clippy builds; look at the `manual_pages` shots.
- [ ] Commit with `Closes #7`.
