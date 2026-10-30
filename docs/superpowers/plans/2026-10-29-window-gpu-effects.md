# Window GPU and Effects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Present the window's frame through wgpu with a screen-effects pass (Off/Soft/CRT) chosen in Settings.

**Architecture:** The CPU rasterizer and `PixelBackend` stay. The backend reports damage rectangles; a `Presenter` uploads them into a GPU texture and draws it through a glow pass and a composite pass to the surface, or falls back to softbuffer.

**Tech Stack:** wgpu 30, pollster 1, bytemuck 1, winit 0.30, WGSL.

**Spec:** `docs/superpowers/specs/2026-10-29-window-gpu-effects-design.md`

## Global Constraints

- New dependencies are optional and enabled only by the `window` feature.
- Builds and runs use `--target-dir target/smoke`; never `target/release`.
- `ScreenFx` defaults to `Soft`; `STRATA_SCREEN_FX=off|soft|crt` overrides it at launch.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A zero-size window (minimised) or a resize mid-frame: no surface configure at 0, no panic, the next present reconfigures. Test: `Presenter`-independent `Gpu::resize` to 0 is a no-op (Task 2).
2. `Outdated`/`Lost`/`Timeout` from `get_current_texture`: reconfigure or skip, never panic.
3. Damage that spans the whole frame after a regrid uploads the whole frame (Task 1 test `regrid_damages_everything`).
4. Off is the frame exactly: the headless Off capture equals the CPU frame (Task 2 test `off_is_the_frame`).
5. No GPU: the window still opens through softbuffer (manual check with `WGPU_BACKEND` pointed at a backend that isn't there).

---

### Task 1: ScreenFx in state and Settings; damage rectangles in the backend

**Files:** `crates/strata-zero/src/app.rs`, `crates/strata-zero/src/screens/settings.rs`, `crates/strata-zero/src/window/backend.rs`, `crates/strata-zero/src/window/mod.rs`

**Interfaces:**
- Produces: `pub enum ScreenFx { Off, Soft, Crt }` (`Default` = `Soft`, `next()`, `label()`, `from_env(&str) -> Option<ScreenFx>`); `AppState { pub windowed: bool, pub screen_fx: ScreenFx }`; `Runner::state(&self) -> &AppState`; `pub struct Damage { pub x0, pub y0, pub x1, pub y1: usize }` with `union`; `PixelBackend::take_damage(&mut self) -> Option<Damage>` (replaces `take_dirty`).

- [ ] Tests: `screen_effects_row_only_in_a_window`, `screen_effects_cycle` (settings); `damage_is_the_painted_cells`, `regrid_damages_everything` (backend).
- [ ] Implement; run `cargo test -p strata-zero --features window --target-dir target/smoke settings window::backend`; commit.

### Task 2: The GPU pipeline, headless

**Files:** Create `crates/strata-zero/src/window/gpu.rs`, `crates/strata-zero/src/window/effects.wgsl`; Cargo deps `wgpu`, `pollster`, `bytemuck`.

**Interfaces:**
- Produces: `pub struct Gpu`; `Gpu::new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Gpu`; `Gpu::headless() -> Option<Gpu>`; `upload(&mut self, frame: &Frame, damage: Option<Damage>)`; `render(&mut self, target: &wgpu::TextureView, fx: Params)`; `capture(&mut self, fx: Params) -> anyhow::Result<(u32, u32, Vec<u8>)>` (RGBA); `pub struct Params { glow, scan, vignette, curve: f32 }` with `Params::of(ScreenFx)`; `scale` passed to `render`/`capture`.

- [ ] Tests (skip without an adapter): `off_is_the_frame` (capture with Off equals the frame's RGBA); `soft_brightens_around_neon` (a magenta block on BG: a BG pixel 4 px off the block gets brighter with Soft than Off); `crt_keeps_the_edge_middles` (curve maps the middle of each edge onto the frame's edge pixel).
- [ ] Ignored `gpu_shots`: title, ATTRIBUTES sheet, street terminal at 120×40 through Off, Soft, CRT into `SHOT_DIR`.
- [ ] Look at the PNGs; tune strengths; commit.

### Task 3: The Presenter and the loop

**Files:** Create `crates/strata-zero/src/window/present.rs`; modify `crates/strata-zero/src/window/mod.rs`.

**Interfaces:**
- Produces: `pub enum Presenter`; `Presenter::new(window: Arc<Window>) -> Presenter` (wgpu, else softbuffer with a note on stderr); `resize(&mut self, w: u32, h: u32)`; `present(&mut self, frame: &Frame, damage: Option<Damage>, fx: ScreenFx, scale: f32) -> Result<()>`; `capture(&mut self, frame: &Frame, fx: ScreenFx, scale: f32) -> Result<(u32, u32, Vec<u8>)>`.

The loop sets `st.windowed = true` and `STRATA_SCREEN_FX`, gathers damage from `take_damage` into a pending union, requests a redraw on damage or on a `screen_fx` change, and `STRATA_WINDOW_SHOT` writes `capture`.

- [ ] Run the window with each `STRATA_SCREEN_FX`; look at the captures; commit.

### Task 4: Bundle, README, whole check, review

- [ ] `scripts/macos-app.sh`; open the `.app` with `STRATA_SCREEN_FX=crt`; check the capture.
- [ ] README: the effects and Settings row.
- [ ] Tests, clippy both ways, fmt; one fresh reviewer; fix what holds up.
