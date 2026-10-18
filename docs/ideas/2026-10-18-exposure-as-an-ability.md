# Exposure as an ability: riff note

Date: 2026-10-18. Status: **idea, not a spec.** From Jeffrey, on seeing the
`eyes` and `light` dev overlays land with the lighting work.

## The idea

The dev overlays are already a player power in disguise. `eyes` paints every
walker who has you in sight; `light` paints how lit each tile is. Both read
`exposure::of`, the same `Exposure` the HUD row shows. An ability or an optics
upgrade could switch one of them on in play:

- **Threat sense** (a talent, or a Perk on optics): walkers who can see you
  are tinted, for a few turns or while focus holds. The stealth loop then has
  its feedback on the map, not only in the HUD count.
- **Lowlight or thermal optics**: the `light` overlay with a palette instead of
  grey, so the dark reads as a heat map of where you can stand unseen.

## What exists

- `crates/strata-zero/src/exposure.rs`: `of(st)` gives `{ light, eyes }`.
- `render/people.rs`: `PeopleOverlay { eyes }` tints the eyes red.
- `render/lighting.rs`: `LightOverlay` paints light as grey.
- `screens/console.rs`: `eyes` and `light` toggle `AppState::show_eyes` and
  `show_light`; the abilities would set the same flags with a duration.
- The spec at `docs/superpowers/specs/2026-10-18-lighting-and-time-of-day-design.md`
  leaves behaviour on being seen to the stealth spec; this would be its first
  player-facing consumer.
