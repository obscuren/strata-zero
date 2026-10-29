# Window on the GPU, and screen effects

2026-10-29. Follows `2026-10-29-window-prototype-design.md`; Jeffrey asked for "level 1
with the effects pass".

## Why

The prototype presents its frame through softbuffer, which hands the OS a CPU image.
The Steam overlay hooks a GPU swapchain, so a Steam build needs the frame on the GPU.
Once the frame is a GPU texture, screen effects are one shader pass, and a soft neon
glow or a CRT look is an easy visual win on a store page for a cyberpunk game.

## Goals

- The window presents through wgpu (Metal on the Mac; Direct3D 12 or Vulkan on Windows;
  Vulkan on Linux), one code path for every Steam platform.
- The CPU rasterizer stays as it is. Each present uploads only the rectangle of pixels
  that changed since the last.
- A screen-effects pass on the GPU with three settings:
  - **Off**: the frame as painted.
  - **Soft** (the default): a glow around bright colours and a gentle vignette.
  - **CRT**: a stronger glow, scanlines, a deeper vignette and a slight curve.
- Settings shows a **Screen effects** row only when the game draws in a window through
  the GPU (not in a terminal, not in the softbuffer fallback); ←→, enter or space cycles
  Off → Soft → CRT. Like the other settings it is not saved between runs.
  `STRATA_SCREEN_FX=off|soft|crt` picks the setting at launch.
- Without a usable GPU the window falls back to the softbuffer path, with no effects.

## Non-goals

Glyphs drawn on the GPU; animated effects (the effects are still images, so a frame
that doesn't change isn't redrawn); HDR; saving settings; the Steamworks SDK itself.

## Design

- `window/gpu.rs`: the device, the frame texture, the effects pipelines and a capture to
  PNG. `window/effects.wgsl` holds the shaders. `window/present.rs` chooses wgpu or
  softbuffer and hides which one the loop is using.
- The backend reports a damage rectangle in place of its dirty flag. The loop gathers
  damage between presents and uploads that rectangle with `write_texture`, reading
  straight out of the frame (row stride = frame width), with no copy.
- The frame's `0x00RRGGBB` pixels are BGRA bytes in memory, so the texture is
  `Bgra8Unorm(Srgb)` and nothing is converted.
- Glow: a bright pass downsampled into a quarter-size texture (half size at scale 1),
  then the composite pass blurs it with 9 taps and adds it. The bright pass keys on a
  pixel's brightest channel, so magenta and cyan neon glow as much as white text does.
- Curve: barrel distortion normalised so the middle of each edge stays on screen (no
  column is cut off); only the corners round off.
- Scanlines follow the curved rows, one dark line every 2 logical pixels.
- `STRATA_WINDOW_SHOT` captures the GPU output, effects included. An ignored test renders
  the shot harness's screens through the GPU pipeline with no window, one PNG per
  setting.

## Calls made (Claude's)

1. wgpu directly rather than the `pixels` crate: the effects need their own passes, and
   `pixels` would only wrap the part that is easy anyway.
2. Soft is the default, and CRT is opt-in: the curve and scanlines cost some legibility.
3. Effect strengths are tuned by eye on the PNGs: glow and vignette for Soft; glow,
   scanlines, vignette and curve for CRT.
4. Linear light: when the surface is sRGB the texture is too, so the glow adds light, not
   gamma values.
5. softbuffer stays as the fallback, so a machine without a usable GPU (a VM, an odd
   driver) still plays.
6. A `bytemuck` dependency (already in the tree through wgpu) gives the frame's bytes to
   wgpu without unsafe code.

## After review

- A hidden window (minimised, covered, on another Space) presents nothing until winit
  says it can be seen again, instead of retrying in a tight loop.
- A lost surface is made again (three times in a row at most, then an error).
- The device asks for the adapter's own limits, and a frame larger than the GPU's
  texture limit keeps the last picture instead of panicking.
- A failed `STRATA_WINDOW_SHOT` is a note on stderr, not the end of the game; captures
  read 8-bit RGBA or BGRA targets only.

## Done when

- Tests pass with and without the `window` feature; clippy `-D warnings` both ways;
  `cargo fmt --check`.
- Settings tests cover the row appearing only in a window, and the cycle.
- The headless GPU PNGs for Off, Soft and CRT, looked at: Off matches the CPU PNG pixel
  for pixel, Soft glows without smearing text, CRT reads at 100×30.
- The `.app` launched with each `STRATA_SCREEN_FX`, with its live capture looked at.
