# Device themes: design

Date: 2026-10-17. Status: built on `feat/corp-gloss` right after the corp-gloss
restyle (approved in chat: "Go theme and megacorp"). Built as specified, plus a
maker and models per theme and dark ink on light bodies. Named on 2026-10-18 with
Jeffrey: one OS for every device, **HELIX/OS**, at a version per maker; makers
and models Civitas K-40 / Kiosk 9 (HELIX/OS 4.1), Halcyon SG-7 / Node 9 (9.4),
Lotus Luxpad / Marquee (8.0, the badge alternates the district's two accents),
Zenbo ZK-2 / 64 (2.2, also your rig). Boots use `{OS}`.
Builds on: `2026-10-17-keypads-design.md` and `2026-10-17-access-terminal-design.md`
(the device kit in `ui/device.rs`).

## Intent

One look for every machine in the city was wrong: a Corp Core terminal and a
Combat Zone one should not share a bezel, and your second-hand rig should not
look like a municipal keypad. Devices now wear a **theme**: a named look and
voice, kept as data in the style file, picked by what the machine is and where
it stands. The rig's theme is the hook a later upgrade changes.

## The themes

Four, in `flavor/style.ron` under `devices`:

| name | body | edge | glass / text | keys | voice |
|---|---|---|---|---|---|
| `municipal` | gunmetal | district accent, heavy hairline | teal-black, cyan | backlit cells | `MUNI-OS 4.1 · {PLACE} SUBNET`, `MEMORY CHECK OK`, `READY`; 60 cps |
| `megacorp` | brushed white | chrome, unbroken | black, ice-blue | dark cells, white labels | `{PLACE} SECURE NODE · CORP/OS 9`, `INTEGRITY VERIFIED`, `WELCOME`; 80 cps; denials open with `YOUR CLEARANCE DOES NOT PERMIT THIS ACTION`; no scanlines, no flicker |
| `neon` | aubergine | the district's second accent | plum, pink-white | backlit in the accent | `{PLACE} NET ~ STAY A WHILE ~`, `ALL SYSTEMS LIT`, `READY`; 60 cps |
| `street` | cracked black plastic | rust, with gaps | amber on near-black, flickers | unlit, worn labels, dead cells | `SYS BOOT.. OK?`, `MEM CHK 64K`, `REDY`; 40 cps; a few dead cells; the glass dims every few seconds |

Colours are data; the accent and second accent come from the district's
`ThemeStyle` so a theme reads differently per district.

## Who wears which

- **District terminal**: by the district's theme. CorpCore → `megacorp`;
  NightMarket, NeonStrip → `neon`; CombatZone, OldTown, Docks, Industrial →
  `street`; Megablocks and anything else → `municipal`.
- **Keypad** on a flat door: CorpTower and Office buildings → `megacorp`;
  else the district's pick as above.
- **The rig**: `street`, the battered second-hand box, with the badge
  `HOME RIG` and the boot `STRATA/OS`. An upgrade changes it later.

## Rendering

- `Theme` replaces `Skin` in `ui/device.rs`: the style's fields plus the
  accents, with `Theme::pick(st, Device)` where
  `Device { Keypad(BuildingId), Terminal(DistrictId), Rig }`.
- **Cracked edge**: a themed `cracked: true` skips every seventh edge cell by
  a hash of its position, so the hairline has gaps.
- **Dead cells**: `dead: n` draws `n` cells of the unit as `▒` in the edge
  colour, placed by hash, never over the glass.
- **Flicker**: `flicker: p` (ticks) dims the glass text to half for three
  ticks every `p` ticks, offset by the unit's own hash so two units never
  blink together.
- **Keys**: `keys_lit: false` draws labels on the body with no backlight.
- **Scanlines**: a flag.
- **Voice**: `boot: [..]` with `{PLACE}` for the district or building,
  `denied: Option<String>` for a first line on refusals, `cps` for the
  typing speed.

## Testing

- The style file parses with four devices; a missing `devices` section
  defaults to `municipal`.
- `Theme::pick`: the four district rules, corp buildings, the rig.
- Rendering: a cracked edge has gaps, a dead count leaves `▒` cells, unlit
  keys have no backlight, flicker dims at its tick and not between, no
  scanlines when off.
- Voice: `boot_lines` follows the theme and fills `{PLACE}`; the terminal
  types at the theme's speed; a megacorp denial opens with its line.
