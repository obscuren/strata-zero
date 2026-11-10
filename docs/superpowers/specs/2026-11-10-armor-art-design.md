# Armor art: drawings for the five armor pieces

## Intent

75 of the game's 118 items open on a dashed MISSING BLUEPRINT box, in the blueprint
window (`i`) and in GEAR & INVENTORY's viewer. This is the first slice of giving them
drawings: the five armor pieces. The look is punk: worn, mended, patched, pinned.

The drawings were agreed in the browser mock (`2026-11-10-armor-art-mock/`, round 3):

| Item | Drawing |
|---|---|
| Salvage jacket | D: worn brown leather courier jacket, brass snaps and zip, faded bolt patch, round pin, safety pins, a rip showing the aramid weave, no spikes |
| Ballistic weave | a quilted vest of layered weave, three buckled straps, studs, patches, button badges, two flattened slugs caught in it |
| Cargo pants | olive cargos, studded belt, eleven pockets with things in them, bondage strap knee to knee, tartan knee, wallet chain |
| Hardhat with visor | in profile, scuffed yellow, DOCK 9 stencilled, a dead phone hung under the brim on copper wire for a visor, the union sticker on it; no cable-tie mohawk |
| Mirrorshades | wraparound chrome, a neon street reflected, one lens cracked, taped bridge, safety-pin hinge |

Concepts A (studs and patches), B (the back piece) and C (hi-vis salvage) stay in the
mock's generator, kept for later pieces.

## Goals

- The five pieces show their drawing in the blueprint window and, at half size, in
  the GEAR & INVENTORY viewer, in the terminal and the window alike.
- A way in that works for any item: an item with no look, deck, hardware or sticker
  art can carry a plain picture, so the 70 still missing need only data.
- The pictures are generated, not hand-typed: the mock's generator is their source,
  and an exporter writes the game's file from it.

## Non-goals

- No stickers on armor (a picture takes none) and no sockets: inspect only.
- No drawings yet for the other 70 items.
- The chrome window's card art is untouched.

## Design

### Content (`game`)

- `Picture` (model.rs): `rows: Vec<String>` (one char a pixel, `.` empty),
  `colours: BTreeMap<char, (u8, u8, u8)>`, and `labels: Vec<PictureLabel>`
  (`#[serde(default)]`). A `PictureLabel` is a cell `x`, `y`, its `text` and `colour`,
  drawn bold over the folded cells, as the Ratchet's label tape is.
- `flavor/item_art.ron` maps item id to `Picture`. `Content` loads it beside
  `implant_art.ron` into `pictures: BTreeMap<ItemId, Picture>` (`#[serde(skip)]`);
  `Content::picture(id)` reads it.
- Validation (`ContentError::PictureInvalid { id, why }`):
  - every key names an item;
  - an item with a picture has no look, deck, hardware or sticker art (one drawing
    each), and no sockets: a picture places none, so modding would have nowhere to
    fit a piece (my call, from the builder check: five chrome pieces have sockets);
  - the rows are all one width, 1 to 88 pixels, and an even number of rows, 2 to 44:
    no more than the Ratchet's width and 22 cells, which fits the smallest blueprint
    window (100×30) and, halved, the GEAR & INVENTORY viewer's 12 rows;
  - every pixel char has a colour and `.` has none (the existing `pixel_art_ok`);
  - every label is non-empty printable ASCII lying wholly on the picture's cells.
- A content test: every armor piece has a picture.

### Drawing (`strata-zero`)

- `ui/picture.rs`: `PictureArt`, an `Art` over a `Picture`: its size is its width by
  half its rows; it draws by filling `Pixels` from the rows and folding them through
  a `Pen`, then putting the labels. No sockets, no back, no parts, no sticker area.
- `screens/blueprint.rs`: `Inspect` gains `Pictured(PictureArt)`. `inspect_for` gives
  it to an item with none of the other drawings and a picture in the content; only
  an item with neither stays `Missing`. The KIT viewer takes it through
  `preview_art` with no change of its own.
- The tests that used armor as the item with no drawing (KIT's
  `an_item_without_a_drawing_shows_the_missing_box` wears mirrorshades) move to an
  item still without one.

### The generator

- The mock folder keeps the generator (`gen.py`, `px.py`) as the pictures' source.
  `export.py` there writes `flavor/item_art.ron` from a table of item id to drawing,
  with a header saying to draw there, not here (as `implant_art.ron` does).
- Pixel chars are ASCII letters and digits, so the glyph test needs nothing: a
  picture's colours must number at most 62, and the exporter refuses one with more.
  The five drawn so far use at most 59 (the jacket), so their scuff mixes stay as
  agreed; a later drawing that runs over rounds its mixes (my call).
- The throwaway `armor_mock` shot test becomes a kept shot test,
  `armor_on_its_blueprints`, writing each piece's blueprint window and KIT view.

## Left to the builder

- Exact names of private helpers and test names.
- Whether `PictureArt` holds a clone of the `Picture` or a reference count; a clone is
  fine at this size.
