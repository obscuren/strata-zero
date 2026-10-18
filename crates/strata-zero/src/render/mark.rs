//! The glyph a tile shows on the map, in its colour: for window headers and look cards.

use crate::app::AppState;
use crate::render::glyphs::kind_icon;
use crate::render::items::AMBER;
use crate::render::lighting::sky_of;
use crate::render::map::{MapRef, TileMap};
use crate::render::people::tint_colour;
use crate::render::tileset::cell;
use crate::render::viewport::wall_mask;
use crate::render::{RenderCtx, Visibility};
use crate::ui::chrome::DIM;
use citygen::{Place, Pos, StratumKind};
use game::ItemKind;
use ratatui::style::Color;

/// A person: `@` in their tint.
pub fn mark_person(st: &AppState, id: sim::PersonId) -> (char, Color) {
    let tint = st
        .world
        .as_ref()
        .and_then(|w| w.people().get(id.0 as usize))
        .and_then(|p| p.tint);
    ('@', tint.map(tint_colour).unwrap_or(DIM))
}

/// A pile on the ground, as the ground overlay draws it.
pub fn mark_items(st: &AppState, place: Place, pos: Pos) -> Option<(char, Color)> {
    let kinds: Vec<ItemKind> = st
        .ground
        .at(place, pos)
        .iter()
        .filter_map(|s| st.content.item(&s.item).map(|i| i.kind))
        .collect();
    let first = *kinds.first()?;
    let glyph = if kinds.iter().all(|k| *k == first) {
        kind_icon(first)
    } else {
        '≡'
    };
    Some((glyph, AMBER.to_color()))
}

/// The tile's own cell, fully lit.
pub fn mark_tile(st: &AppState, map: &MapRef, pos: Pos) -> Option<(char, Color)> {
    let tile = map.tile(pos)?;
    let ctx = RenderCtx {
        style: &st.style,
        theme: map.theme(pos),
        stratum: map.stratum().unwrap_or(StratumKind::Street),
        light: map.light(pos),
        tick: st.tick,
        vis: Visibility::Visible,
        connector: map.connector(pos),
        kind: map.kind(pos),
        wall_mask: wall_mask(map, pos, tile.terrain),
        lamps_on: map.lamps_on(),
        sky: sky_of(map),
    };
    let c = cell(tile, &ctx);
    Some((c.ch, c.fg.to_color()))
}
