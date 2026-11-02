//! Look: what the player's eyes can tell them about one tile, in the house voice.

use crate::app::AppState;
use crate::render::Visibility;
use crate::render::map::{MapRef, TileMap};
use crate::render::mark::{mark_items, mark_person, mark_tile};
use crate::ui::chrome::{DIM, TEXT, frame, put, solid};
use crate::ui::hud::capitalised;
use citygen::{ConnectorKind, Feature, FloorPlan, Place, Pos, Rect as TileRect, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const DEFAULT_LOOKS: &str = include_str!("../../../flavor/looks.ron");

/// What the look window says about one tile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LookCard {
    pub glyph: Option<(char, Color)>,
    /// None on a tile the player cannot see.
    pub header: Option<String>,
    pub header_fg: Color,
    /// Dim: what the thing is and its state.
    pub second: Option<String>,
    pub body: Vec<String>,
    /// The other things on the tile, by name.
    pub also: Vec<String>,
    /// Space would open a window for it from where the player stands.
    pub hint: bool,
}

/// The flavour lines, by group and key.
#[derive(Debug, Deserialize)]
pub struct Looks {
    feature: BTreeMap<String, Vec<String>>,
    furniture: BTreeMap<String, Vec<String>>,
    building: BTreeMap<String, Vec<String>>,
    fixture: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Feature,
    Furniture,
    Building,
    Fixture,
}

impl Looks {
    /// The embedded `flavor/looks.ron`, parsed once.
    pub fn load() -> &'static Looks {
        static LOOKS: OnceLock<Looks> = OnceLock::new();
        LOOKS.get_or_init(|| ron::from_str(DEFAULT_LOOKS).expect("embedded looks are valid"))
    }

    fn group(&self, g: Group) -> &BTreeMap<String, Vec<String>> {
        match g {
            Group::Feature => &self.feature,
            Group::Furniture => &self.furniture,
            Group::Building => &self.building,
            Group::Fixture => &self.fixture,
        }
    }

    /// One of the key's lines, the same one every time for this tile.
    pub fn line(&self, g: Group, key: &str, seed: u64, pos: Pos) -> Option<&str> {
        let lines = self.group(g).get(key)?;
        if lines.is_empty() {
            return None;
        }
        let h = citygen::rng::hash64(seed, ((pos.x as u64) << 32) ^ (pos.y as u32 as u64));
        Some(lines[(h % lines.len() as u64) as usize].as_str())
    }

    #[cfg(test)]
    pub fn all_lines(&self) -> Vec<&str> {
        [
            Group::Feature,
            Group::Furniture,
            Group::Building,
            Group::Fixture,
        ]
        .into_iter()
        .flat_map(|g| self.group(g).values().flatten().map(|s| s.as_str()))
        .collect()
    }

    #[cfg(test)]
    pub fn has(&self, key: &str) -> bool {
        [
            Group::Feature,
            Group::Furniture,
            Group::Building,
            Group::Fixture,
        ]
        .into_iter()
        .any(|g| self.group(g).contains_key(key))
    }
}

/// A feature's name and its key in the flavour file.
fn feature_name(f: Feature) -> Option<(&'static str, &'static str)> {
    Some(match f {
        Feature::Lamp => ("Street lamp", "lamp"),
        Feature::Neon(_) => ("Neon sign", "neon"),
        Feature::HoloAd => ("Holo ad", "holo_ad"),
        Feature::Vent => ("Vent", "vent"),
        Feature::Puddle => ("Puddle", "puddle"),
        Feature::Trash => ("Trash", "trash"),
        Feature::Barrier => ("Barrier", "barrier"),
        Feature::Stall => ("Stall", "stall"),
        Feature::Sign(_) => ("Sign", "sign"),
        Feature::Camera => ("Camera", "camera"),
        Feature::AcUnit => ("AC unit", "ac_unit"),
        Feature::Helipad => ("Helipad", "helipad"),
        Feature::Pipe => ("Pipe", "pipe"),
        Feature::Cable => ("Cable", "cable"),
        Feature::Awning => ("Awning", "awning"),
        Feature::Fitting(x) => fitting_name(x),
        _ => return None,
    })
}

/// A light fitting's name and key, by its kind and, for the kinds that wear, its wear.
fn fitting_name(f: citygen::Fitting) -> (&'static str, &'static str) {
    use citygen::{FittingKind as K, Wear as W};
    match (f.kind, f.wear) {
        (K::Tube, W::Dead) => ("Dead fluorescent tube", "tube_dead"),
        (K::Tube, W::Stutter) => ("Flickering fluorescent tube", "tube_flicker"),
        (K::Tube, _) => ("Fluorescent tube", "tube"),
        (K::HighBay, W::Dead) => ("Dead high-bay lamp", "high_bay_dead"),
        (K::HighBay, W::Stutter) => ("Flickering high-bay lamp", "high_bay_flicker"),
        (K::HighBay, _) => ("High-bay lamp", "high_bay"),
        (K::Strip, W::Dead) => ("Dead light strip", "strip_dead"),
        (K::Strip, W::Stutter) => ("Flickering light strip", "strip_flicker"),
        (K::Strip, _) => ("Light strip", "strip"),
        (K::Panel, _) => ("LED panel", "panel"),
        (K::Bulb, _) => ("Bulb", "bulb"),
        (K::Sconce, _) => ("Sconce", "sconce"),
        (K::NeonStrip, _) => ("Neon strip", "neon_strip"),
        (K::Stage, _) => ("Stage lights", "stage_lights"),
    }
}

/// A fixture on the tile: header, second line and its flavour key.
struct Fixture {
    name: String,
    second: Option<String>,
    key: &'static str,
}

/// "up to Skyways · down to Undercity", leaving out a missing side.
fn leads(up: Option<String>, down: Option<String>) -> Option<String> {
    let parts: Vec<String> = up
        .map(|u| format!("up to {u}"))
        .into_iter()
        .chain(down.map(|d| format!("down to {d}")))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn fixture_at(st: &AppState, map: &MapRef, place: Place, pos: Pos) -> Option<Fixture> {
    let city = st.city.as_ref()?;
    let tile = map.tile(pos)?;
    match place {
        Place::Floor { building, level } => {
            if let Some(rig) = st.home
                && rig.place() == place
                && rig.home.rig == pos
            {
                return Some(Fixture {
                    name: "Access rig".into(),
                    second: Some("your rig".into()),
                    key: "rig",
                });
            }
            let (plan, floor) = st.interiors.floor(building, level)?;
            if let Some(r) = floor
                .door_of(pos)
                .filter(|r| r.private && r.kind == citygen::RoomKind::Flat)
            {
                let now = st.net.now(st);
                let state = match st
                    .journal
                    .keypad_locked_until(building, level, r.number, now)
                {
                    Some(until) => format!("keypad locked {}m", until.saturating_sub(now).max(1)),
                    None if crate::credentials::opens(st, building, level, r.number).is_some() => {
                        "unlocked".to_string()
                    }
                    None => "locked, keypad".to_string(),
                };
                return Some(Fixture {
                    name: capitalised(&crate::credentials::flat_label(
                        st, building, level, r.number,
                    )),
                    second: Some(format!("front door · {state}")),
                    key: "flat_door",
                });
            }
            match tile.terrain {
                Terrain::Stair => Some(Fixture {
                    name: "Stairwell".into(),
                    second: leads(
                        (level < plan.top).then(|| FloorPlan::label(level + 1)),
                        (level > plan.lowest).then(|| FloorPlan::label(level - 1)),
                    ),
                    key: "stairwell",
                }),
                Terrain::Lift => Some(Fixture {
                    name: "Lift".into(),
                    second: Some(format!(
                        "{} to {}",
                        FloorPlan::label(plan.lowest),
                        FloorPlan::label(plan.top)
                    )),
                    key: "lift",
                }),
                _ => None,
            }
        }
        Place::Stratum(k) => {
            if city
                .connectors_at(pos)
                .any(|c| c.kind == ConnectorKind::Jack && c.serves.contains(&k))
            {
                let district = tile
                    .district
                    .and_then(|d| city.district_opt(d))
                    .map(|d| d.name.clone());
                return Some(Fixture {
                    name: "Access terminal".into(),
                    second: Some(match district {
                        Some(d) => format!("public · {d}"),
                        None => "public".into(),
                    }),
                    key: "terminal",
                });
            }
            let c = city
                .connectors_at(pos)
                .find(|c| c.kind != ConnectorKind::Jack)?;
            let (below, above) = c.serves_from(k)?;
            let (name, key) = match c.kind {
                ConnectorKind::Stair => ("Stairs", "stairs"),
                ConnectorKind::Lift => ("Lift", "lift"),
                ConnectorKind::Ladder => ("Ladder", "ladder"),
                _ => ("Manhole", "manhole"),
            };
            Some(Fixture {
                name: name.into(),
                second: leads(
                    above.map(|s| s.label().to_string()),
                    below.map(|s| s.label().to_string()),
                ),
                key,
            })
        }
        Place::Construct(_) => None,
    }
}

/// The info window's width.
pub const LOOK_W: u16 = 48;
/// Under the look cursor: a dark amber block, the glyph kept.
pub const LOOK_BG: crate::render::Rgb = crate::render::Rgb(90, 70, 0);

/// Greedy word wrap at `width`; a word longer than a row is cut.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut row = String::new();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        let used = row.chars().count();
        let w = word.chars().count();
        if used > 0 && used + 1 + w > width {
            out.push(std::mem::take(&mut row));
        }
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str(word);
        while row.chars().count() > width {
            let head: String = row.chars().take(width).collect();
            row = row.chars().skip(width).collect();
            out.push(head);
        }
    }
    if !row.is_empty() {
        out.push(row);
    }
    out
}

/// The card's rows inside the frame, each with its style.
fn rows(card: &LookCard, width: u16) -> Vec<(String, Style)> {
    let inner = width.saturating_sub(4) as usize;
    let dim = Style::default().fg(DIM);
    let text = Style::default().fg(TEXT);
    let mut out: Vec<(String, Style)> = Vec::new();
    if let Some(s) = &card.second {
        out.extend(wrap(s, inner).into_iter().map(|l| (l, dim)));
    }
    if !card.body.is_empty() {
        if !out.is_empty() {
            out.push((String::new(), text));
        }
        for b in &card.body {
            out.extend(wrap(b, inner).into_iter().map(|l| (l, text)));
        }
    }
    let mut foot: Vec<(String, Style)> = Vec::new();
    if !card.also.is_empty() {
        let also = format!("also here: {}", card.also.join(", "));
        foot.extend(wrap(&also, inner).into_iter().map(|l| (l, dim)));
    }
    if card.hint {
        foot.push(("space · interact".to_string(), dim));
    }
    if !foot.is_empty() {
        if !out.is_empty() {
            out.push((String::new(), text));
        }
        out.extend(foot);
    }
    out
}

/// Rows the box takes at `width`, its frame included.
pub fn box_height(card: &LookCard, width: u16) -> u16 {
    rows(card, width).len().max(1) as u16 + 2
}

/// Where the box goes in the map area: a top corner on the half away from the
/// cursor, or with a narrow map the top or bottom half away from it. Never over
/// the cursor.
pub fn place_box(map: Rect, cursor: (u16, u16), h: u16) -> Rect {
    if map.width >= 2 * LOOK_W + 2 {
        let h = h.min(map.height);
        let x = if cursor.0 < map.x + map.width / 2 {
            map.right() - LOOK_W - 1
        } else {
            map.x + 1
        };
        return Rect::new(x, map.y, LOOK_W, h);
    }
    let h = h.min(map.height / 2).max(1);
    let w = LOOK_W.min(map.width);
    let x = map.right() - w;
    let y = if cursor.1 >= map.y + map.height / 2 {
        map.y
    } else {
        map.bottom() - h
    };
    Rect::new(x, y, w, h)
}

/// Draws the card into `r`: the header on the top border, then its rows.
pub fn draw_box(card: &LookCard, r: Rect, buf: &mut Buffer) {
    if r.width < 6 || r.height < 2 {
        return;
    }
    solid(buf, r);
    frame(buf, r);
    let max = r.right() - 1;
    if let Some(name) = &card.header {
        let style = Style::default()
            .fg(card.header_fg)
            .add_modifier(Modifier::BOLD);
        let mut x = r.x + 2;
        put(buf, x, r.y, " ", style, max);
        x += 1;
        if let Some((g, colour)) = card.glyph {
            put(
                buf,
                x,
                r.y,
                &format!("{g} "),
                Style::default().fg(colour).add_modifier(Modifier::BOLD),
                max,
            );
            x += 2;
        }
        put(buf, x, r.y, &format!("{name} "), style, max);
    }
    for (i, (line, style)) in rows(card, r.width).into_iter().enumerate() {
        let y = r.y + 1 + i as u16;
        if y >= r.bottom() - 1 {
            break;
        }
        put(buf, r.x + 2, y, &line, style, max);
    }
}

/// The things worth a look in `view`, nearest to `from` first: people, fixtures,
/// piles, and lamps, neon, signs and holo ads. Only what is in sight.
pub fn points_of_interest(
    st: &AppState,
    map: &MapRef,
    place: Place,
    view: TileRect,
    from: Pos,
    inside_person: &dyn Fn(Place, Pos) -> Option<sim::PersonId>,
) -> Vec<Pos> {
    let player = st.player.as_ref().filter(|p| p.place == place);
    let mut out: Vec<Pos> = Vec::new();
    for y in view.y..view.y + view.h {
        for x in view.x..view.x + view.w {
            let p = Pos::new(x, y);
            let Some(tile) = map.tile(p) else { continue };
            let seen = player
                .map(|pl| pl.vis().get(p).copied().unwrap_or(Visibility::Unseen))
                .unwrap_or(Visibility::Visible);
            if seen != Visibility::Visible {
                continue;
            }
            let person = if place.is_street() {
                st.world.as_ref().and_then(|w| w.person_at(p)).is_some()
            } else {
                inside_person(place, p).is_some()
            } || crate::missions::visitors::at(st, place, p).is_some();
            let lit = matches!(
                tile.feature,
                Some(Feature::Lamp | Feature::Neon(_) | Feature::Sign(_) | Feature::HoloAd)
            );
            if person
                || lit
                || !st.ground.at(place, p).is_empty()
                || fixture_at(st, map, place, p).is_some()
            {
                out.push(p);
            }
        }
    }
    out.sort_by_key(|p| (p.chebyshev(from), p.y, p.x));
    out
}

/// What the player's eyes can tell them about `pos` on the map they walk.
pub fn look_card(
    st: &AppState,
    map: &MapRef,
    place: Place,
    pos: Pos,
    inside_person: &dyn Fn(Place, Pos) -> Option<sim::PersonId>,
    reachable: &[Pos],
) -> LookCard {
    let Some(city) = st.city.as_ref() else {
        return LookCard::default();
    };
    let player = st.player.as_ref().filter(|p| p.place == place);
    let seen = player
        .map(|p| p.vis().get(pos).copied().unwrap_or(Visibility::Unseen))
        .unwrap_or(Visibility::Visible);
    if seen == Visibility::Unseen || map.tile(pos).is_none() {
        return LookCard {
            body: vec!["You can't see there.".to_string()],
            header_fg: TEXT,
            ..LookCard::default()
        };
    }
    let visible = seen == Visibility::Visible;
    let looks = Looks::load();
    let tile = map.tile(pos).expect("checked");
    let own = player.is_some_and(|p| p.pos == pos);
    let mut card = LookCard {
        header_fg: TEXT,
        hint: visible && reachable.contains(&pos),
        ..LookCard::default()
    };

    // What is there, best first.
    let person = visible
        .then(|| {
            if place.is_street() {
                st.world.as_ref().and_then(|w| w.person_at(pos))
            } else {
                inside_person(place, pos)
            }
        })
        .flatten();
    let fixture = fixture_at(st, map, place, pos);
    let stacks: Vec<String> = if visible {
        st.ground
            .at(place, pos)
            .iter()
            .filter_map(|s| {
                let item = st.content.item(&s.item)?;
                Some(if s.qty > 1 {
                    format!("{} ×{}", item.name, s.qty)
                } else {
                    item.name.clone()
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let set_head = |card: &mut LookCard, name: String, glyph: Option<(char, Color)>| {
        card.header_fg = glyph.map_or(TEXT, |(_, c)| c);
        card.glyph = glyph;
        card.header = Some(name);
    };

    // A foe on the tile, standing or down, before whatever it stands on.
    let foe = st
        .foes
        .iter()
        .find(|f| f.place == place && f.pos == pos && (visible || f.down));
    // A visitor the story put here, before people.
    let visitor = visible
        .then(|| crate::missions::visitors::at(st, place, pos))
        .flatten();
    // A body the story left, before the same.
    let body = st
        .missions
        .bodies
        .iter()
        .find(|b| b.place == place && b.pos == pos);
    if let (Some(b), None) = (body, foe) {
        set_head(
            &mut card,
            capitalised(&b.name),
            Some(('%', crate::render::people::FOE_DOWN)),
        );
        card.second = Some("dead".to_string());
        card.body.push(b.look.clone());
        card.also.extend(stacks);
    } else if let Some(f) = foe {
        let colour = if f.down {
            crate::render::people::FOE_DOWN
        } else {
            crate::render::people::FOE
        };
        set_head(
            &mut card,
            capitalised(&f.name),
            Some((if f.down { '%' } else { f.glyph }, colour)),
        );
        card.second = Some(if f.down {
            "down".to_string()
        } else {
            let mode = if f.hunting() { "hunting" } else { "idle" };
            format!(
                "{mode} · {}/{} · {}",
                f.hp,
                f.hp_max,
                crate::foes::weapon_name(st, f)
            )
        });
        if f.stunned > 0 {
            card.body.push(format!(
                "Stunned for {} turn{}.",
                f.stunned,
                if f.stunned == 1 { "" } else { "s" }
            ));
        }
        card.also.extend(stacks);
    } else if let Some(v) = visitor {
        let role = st.book.cast.get(&v.role);
        set_head(
            &mut card,
            role.map_or_else(|| v.role.clone(), |r| r.name.clone()),
            Some(('@', crate::missions::visitors::colour(&st.book, &v.role))),
        );
        card.second = role.map(|r| r.title.clone());
        card.body.extend(role.and_then(|r| r.look.clone()));
        card.also.extend(stacks);
    } else if let (Some(id), Some(world)) = (person, st.world.as_ref()) {
        let c = world.card(id, city);
        set_head(&mut card, c.name.clone(), Some(mark_person(st, id)));
        card.second = Some(format!("{} · {}", c.profession, c.doing));
        if !crate::credentials::carries(st, id).is_empty() {
            card.body
                .push("A keycard swings on a lanyard at their belt.".to_string());
        }
        card.also.extend(fixture.iter().map(|f| f.name.clone()));
        card.also.extend(stacks);
    } else if let Some(f) = fixture {
        set_head(&mut card, f.name, mark_tile(st, map, pos));
        card.second = f.second;
        if visible {
            card.body.extend(
                looks
                    .line(Group::Fixture, f.key, city.seed, pos)
                    .map(String::from),
            );
            card.also.extend(stacks);
        }
    } else if let Some((first, rest)) = stacks.split_first() {
        let s = &st.ground.at(place, pos)[0];
        let item = st.content.item(&s.item);
        set_head(&mut card, first.clone(), mark_items(st, place, pos));
        card.second = item.map(|i| format!("{} · on the ground", i.kind.label()));
        card.body.extend(item.map(|i| i.desc.clone()));
        card.also.extend(rest.iter().cloned());
    } else if let Some(f) = tile.feature {
        let glyph = mark_tile(st, map, pos);
        match f {
            Feature::Furniture(fu) => {
                set_head(&mut card, capitalised(fu.label()), glyph);
                if visible {
                    card.body.extend(
                        looks
                            .line(Group::Furniture, fu.label(), city.seed, pos)
                            .map(String::from),
                    );
                }
            }
            Feature::Node(k) => {
                set_head(&mut card, capitalised(k.label()), glyph);
                card.second = Some("node".to_string());
            }
            Feature::Ice(g) => {
                set_head(&mut card, format!("{} ICE", capitalised(g.label())), glyph);
            }
            f => match feature_name(f) {
                Some((name, key)) => {
                    set_head(&mut card, name.to_string(), glyph);
                    // A holo ad cutting into the news reads the headline while it lasts.
                    let cut = (visible
                        && f == Feature::HoloAd
                        && place.is_street()
                        && crate::missions::news::cut_in(st) == Some(pos))
                    .then(|| crate::missions::news::current(st))
                    .flatten();
                    if let Some(n) = cut {
                        card.body.push(format!(
                            "The ad stutters. For a second it's a headline: {}.",
                            n.headline_caps()
                        ));
                    } else if visible {
                        card.body.extend(
                            looks
                                .line(Group::Feature, key, city.seed, pos)
                                .map(String::from),
                        );
                    }
                }
                None => ground(st, map, place, pos, own, &mut card),
            },
        }
    } else if let Place::Construct(b) = place
        && let Some(i) = st.constructs.get(b).and_then(|k| k.guard_at(pos))
    {
        let g = &st.constructs.get(b).expect("checked").guards[i];
        set_head(
            &mut card,
            format!("{} ICE", capitalised(g.label())),
            mark_tile(st, map, pos),
        );
        card.second = Some(if g.standing { "standing" } else { "broken" }.to_string());
    } else if let Some(b) = tile.building.filter(|_| {
        place.stratum().is_some()
            && matches!(
                tile.terrain,
                Terrain::Wall | Terrain::Window | Terrain::Mass
            )
    }) {
        let bl = city.building(b);
        let kind = citygen::kind_label(city, b);
        let name = bl.name.clone().unwrap_or_else(|| capitalised(kind));
        set_head(&mut card, name, mark_tile(st, map, pos));
        let district = city.district(bl.district).name.clone();
        card.second = Some(format!("{kind} · {district}"));
        // A clinic's front has its own line, with a tell in it.
        let key = match citygen::clinic_front(city, b) {
            Some(_) => format!("{kind} front"),
            None => kind.to_string(),
        };
        if visible {
            card.body.extend(
                looks
                    .line(Group::Building, &key, city.seed, pos)
                    .map(String::from),
            );
        }
    } else {
        ground(st, map, place, pos, own, &mut card);
    }
    if !visible {
        card.second = Some("as you last saw it".to_string());
        card.body.clear();
        card.also.clear();
    }
    card
}

/// The bare ground: its name and where it is, or "You" on your own tile.
fn ground(st: &AppState, map: &MapRef, place: Place, pos: Pos, own: bool, card: &mut LookCard) {
    let Some(city) = st.city.as_ref() else { return };
    let location = crate::ui::hud::location(city, &st.interiors, place, pos);
    if own {
        card.glyph = Some(('@', Color::Rgb(255, 255, 255)));
        card.header_fg = TEXT;
        card.header = Some("You".to_string());
    } else if let Some(t) = map.tile(pos) {
        card.glyph = mark_tile(st, map, pos);
        card.header_fg = card.glyph.map_or(TEXT, |(_, c)| c);
        card.header = Some(capitalised(t.terrain.label()));
    }
    card.second = Some(location);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::map::MapRef;
    use crate::walker::{Player, Visibility};
    use citygen::{GenParams, Generator, SizePreset, StratumKind};

    const STREET: Place = Place::Stratum(StratumKind::Street);

    fn state(world: bool) -> AppState {
        let mut st = AppState::new();
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        if world {
            st.world = Some(sim::World::seed(
                &city,
                &st.people,
                &sim::SimParams::default(),
                22 * 60,
                st.tick,
            ));
        }
        st.player = Some(Player::spawn(&city, 1));
        st.city = Some(city);
        st
    }

    /// Puts the player on `pos` and lets them look around.
    fn stand(st: &mut AppState, pos: Pos) {
        let city = st.city.as_ref().unwrap();
        let p = st.player.as_mut().unwrap();
        p.place = STREET;
        p.pos = pos;
        p.update_fov(&MapRef::stratum_of(city, StratumKind::Street), 0);
    }

    fn card(st: &AppState, pos: Pos) -> LookCard {
        let city = st.city.as_ref().unwrap();
        let map = MapRef::stratum_of(city, StratumKind::Street);
        look_card(st, &map, STREET, pos, &|_, _| None, &[])
    }

    /// A walkable street tile with nothing on it: no connector, no feature, no building.
    fn bare(st: &AppState) -> Pos {
        let city = st.city.as_ref().unwrap();
        let map = MapRef::stratum_of(city, StratumKind::Street);
        (0..60)
            .flat_map(|y| (0..120).map(move |x| Pos::new(x, y)))
            .find(|p| {
                city.walkable(StratumKind::Street, *p)
                    && city.connectors_at(*p).next().is_none()
                    && map
                        .tile(*p)
                        .is_some_and(|t| t.feature.is_none() && t.building.is_none())
            })
            .unwrap()
    }

    /// Someone on the street and a free tile beside them.
    fn someone(st: &AppState) -> (sim::PersonId, Pos, Pos) {
        let city = st.city.as_ref().unwrap();
        let world = st.world.as_ref().unwrap();
        (0..60)
            .flat_map(|y| (0..120).map(move |x| Pos::new(x, y)))
            .find_map(|p| {
                let id = world.person_at(p)?;
                let beside = p.neighbors4().find(|n| {
                    city.walkable(StratumKind::Street, *n) && world.person_at(*n).is_none()
                })?;
                Some((id, p, beside))
            })
            .expect("someone out at ten")
    }

    fn all_text(c: &LookCard) -> String {
        let mut s = String::new();
        for t in c
            .header
            .iter()
            .chain(c.second.iter())
            .chain(&c.body)
            .chain(&c.also)
        {
            s.push_str(t);
            s.push('\n');
        }
        s
    }

    #[test]
    fn every_fitting_has_a_name_and_flavour() {
        use citygen::{Fitting, FittingKind as K, Tone, Wear};
        let looks = Looks::load();
        for kind in [
            K::Tube,
            K::Panel,
            K::Bulb,
            K::HighBay,
            K::Strip,
            K::Sconce,
            K::NeonStrip,
            K::Stage,
        ] {
            let wears: &[Wear] = if kind.wears() {
                &[Wear::Ok, Wear::Dead, Wear::Stutter]
            } else {
                &[Wear::Ok]
            };
            for &wear in wears {
                let f = Feature::Fitting(Fitting {
                    kind,
                    tone: Tone::Fluorescent,
                    wear,
                });
                let (name, key) =
                    feature_name(f).unwrap_or_else(|| panic!("{kind:?} {wear:?} has no name"));
                assert!(!name.is_empty() && !name.contains("Street"));
                assert!(
                    looks
                        .line(Group::Feature, key, 1, citygen::Pos::new(0, 0))
                        .is_some(),
                    "{key} has no flavour"
                );
            }
        }
    }

    #[test]
    fn the_holo_set_in_the_flat_reads_its_head_and_a_line() {
        let mut st = crate::jobs::testutil::state_home();
        let (place, pos) = crate::missions::news::set_tile(&st).unwrap();
        let map = MapRef::for_place(
            st.city.as_ref().unwrap(),
            &st.interiors,
            &st.constructs,
            place,
            0,
        )
        .unwrap();
        let mut me = Player::at(place, pos.offset(0, 1));
        me.update_fov(&map, 0);
        st.player = Some(me);
        let c = look_card(&st, &map, place, pos, &|_, _| None, &[]);
        assert_eq!(c.header.as_deref(), Some("Holo set"));
        assert!(
            [
                "A cheap holo set, the picture a hand's width off true.",
                "Older than the flat. It still gets every channel."
            ]
            .contains(&c.body[0].as_str()),
            "{:?}",
            c.body
        );
    }

    #[test]
    fn look_reads_the_headline_during_the_cut() {
        use crate::missions::news;
        let mut st = crate::jobs::testutil::state_home();
        crate::missions::begin(&mut st);
        news::put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        let headline = news::current(&st).unwrap().headline.to_uppercase();
        let ad = news::tests::an_ad_cutting_in(&mut st);
        let c = card(&st, ad);
        assert_eq!(c.header.as_deref(), Some("Holo ad"));
        assert_eq!(
            c.body,
            [format!(
                "The ad stutters. For a second it's a headline: {headline}."
            )]
        );
        assert!(headline.starts_with("COURIER SOUGHT IN "), "{headline}");
        // After the cut: the ad's own line.
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + news::CUT_TICKS);
        }
        assert_eq!(news::cut_in(&st), None);
        let seed = st.city.as_ref().unwrap().seed;
        let own = Looks::load()
            .line(Group::Feature, "holo_ad", seed, ad)
            .map(String::from);
        assert!(own.is_some());
        assert_eq!(card(&st, ad).body, Vec::from_iter(own));
    }

    #[test]
    fn a_person_reads_name_job_doing_and_never_a_number() {
        let mut st = state(true);
        let (id, at, beside) = someone(&st);
        stand(&mut st, beside);
        let c = card(&st, at);
        let person = st
            .world
            .as_ref()
            .unwrap()
            .card(id, st.city.as_ref().unwrap());
        assert_eq!(c.header.as_deref(), Some(person.name.as_str()));
        assert_eq!(
            c.second,
            Some(format!("{} · {}", person.profession, person.doing))
        );
        assert_eq!(c.glyph.map(|g| g.0), Some('@'));
        // Names are data (some carry a number); everything else has none.
        let rest = LookCard {
            header: None,
            ..c.clone()
        };
        let text = all_text(&rest);
        assert!(!text.chars().any(|ch| ch.is_ascii_digit()), "{text}");
    }

    #[test]
    fn look_reads_a_body() {
        let mut st = state(false);
        let at = bare(&st);
        stand(&mut st, at);
        st.missions.bodies.push(crate::missions::Body {
            person: sim::PersonId(0),
            name: "Marlo".into(),
            place: STREET,
            pos: at,
            look: "Marlo lies face down by the shutters.".into(),
            near: false,
            left: None,
        });
        let c = card(&st, at);
        assert_eq!(c.header.as_deref(), Some("Marlo"));
        assert_eq!(c.glyph.map(|g| g.0), Some('%'));
        assert_eq!(c.second.as_deref(), Some("dead"));
        assert_eq!(c.body, ["Marlo lies face down by the shutters."]);
    }

    #[test]
    fn an_item_reads_its_desc() {
        let mut st = state(false);
        let at = bare(&st);
        stand(&mut st, at);
        st.ground
            .add(STREET, at, game::ItemId::new("stim_patch"), 2);
        let c = card(&st, at);
        assert_eq!(c.header.as_deref(), Some("Stim patch ×2"));
        assert_eq!(c.second.as_deref(), Some("consumable · on the ground"));
        assert_eq!(c.body, ["Twelve minutes of clarity, then the bill."]);
        st.ground
            .add(STREET, at, game::ItemId::new("dead_phone"), 1);
        let c = card(&st, at);
        assert_eq!(c.also, ["Dead phone"]);
    }

    #[test]
    fn remembered_tiles_hide_people_and_items() {
        let mut st = state(true);
        let (_, at, beside) = someone(&st);
        stand(&mut st, beside);
        st.ground
            .add(STREET, at, game::ItemId::new("stim_patch"), 2);
        st.player
            .as_mut()
            .unwrap()
            .set_vis(at, Visibility::Remembered);
        let c = card(&st, at);
        let ground = st
            .city
            .as_ref()
            .unwrap()
            .tile(StratumKind::Street, at)
            .unwrap()
            .terrain;
        assert_eq!(
            c.header,
            Some(crate::ui::hud::capitalised(ground.label())),
            "the ground, as remembered"
        );
        assert_eq!(c.second.as_deref(), Some("as you last saw it"));
        assert!(c.body.is_empty() && c.also.is_empty(), "{c:?}");
        assert!(!c.hint);
    }

    #[test]
    fn unseen_tiles_say_one_line() {
        let mut st = state(false);
        let at = bare(&st);
        stand(&mut st, at);
        let far = Pos::new(at.x + 1, at.y);
        st.player.as_mut().unwrap().set_vis(far, Visibility::Unseen);
        let c = card(&st, far);
        assert_eq!(c.header, None);
        assert_eq!(c.second, None);
        assert_eq!(c.body, ["You can't see there."]);
    }

    #[test]
    fn your_own_tile_shows_what_else_is_there() {
        let mut st = state(false);
        let at = bare(&st);
        stand(&mut st, at);
        let c = card(&st, at);
        assert_eq!(c.header.as_deref(), Some("You"));
        let k = st
            .city
            .as_ref()
            .unwrap()
            .connectors
            .iter()
            .find(|k| {
                k.kind != citygen::ConnectorKind::Jack
                    && k.serves.contains(&StratumKind::Street)
                    && st
                        .city
                        .as_ref()
                        .unwrap()
                        .walkable(StratumKind::Street, k.pos)
            })
            .unwrap()
            .clone();
        stand(&mut st, k.pos);
        let c = card(&st, k.pos);
        assert!(
            matches!(
                c.header.as_deref(),
                Some("Stairs" | "Lift" | "Ladder" | "Manhole")
            ),
            "{c:?}"
        );
        let second = c.second.unwrap_or_default();
        assert!(
            second.contains("up to") || second.contains("down to"),
            "{second}"
        );
    }

    #[test]
    fn a_hint_shows_only_within_reach() {
        let mut st = state(false);
        let at = bare(&st);
        stand(&mut st, at);
        let city = st.city.as_ref().unwrap();
        let map = MapRef::stratum_of(city, StratumKind::Street);
        assert!(look_card(&st, &map, STREET, at, &|_, _| None, &[at]).hint);
        assert!(!look_card(&st, &map, STREET, at, &|_, _| None, &[]).hint);
    }

    #[test]
    fn look_box_never_covers_the_cursor() {
        for map in [
            Rect::new(0, 5, 100, 30),
            Rect::new(0, 5, 60, 20),
            Rect::new(3, 2, 97, 25),
        ] {
            for y in map.y..map.bottom() {
                for x in map.x..map.right() {
                    for h in [4, 8, 30] {
                        let r = place_box(map, (x, y), h);
                        assert!(
                            !(r.x <= x && x < r.right() && r.y <= y && y < r.bottom()),
                            "{map:?} {x},{y} h{h} {r:?}"
                        );
                        assert!(r.right() <= map.right() && r.bottom() <= map.bottom());
                        assert!(r.x >= map.x && r.y >= map.y);
                    }
                }
            }
        }
    }

    #[test]
    fn the_box_draws_header_second_body_and_hint() {
        let c = LookCard {
            glyph: Some(('▣', Color::Cyan)),
            header: Some("Access terminal".into()),
            header_fg: Color::Cyan,
            second: Some("public · Halvorsen".into()),
            body: vec!["A municipal box bolted to the wall, its screen scratched to fog.".into()],
            also: vec![],
            hint: true,
        };
        let h = box_height(&c, LOOK_W);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        draw_box(&c, Rect::new(0, 0, LOOK_W, h), &mut buf);
        let text = crate::screens::testutil::buffer_text(&buf);
        assert!(text.contains("▣ Access terminal"), "{text}");
        assert!(text.contains("public · Halvorsen"), "{text}");
        assert!(text.contains("scratched to fog."), "{text}");
        assert!(text.contains("space · interact"), "{text}");
        assert_eq!(
            h, 8,
            "border, second, blank, two body rows, blank, hint, border: {text}"
        );
    }

    #[test]
    fn a_clinic_looks_like_its_front() {
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 8,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        let city = st.city.as_ref().unwrap();
        let b = city
            .buildings
            .iter()
            .find(|b| b.kind == citygen::BuildingKind::Clinic)
            .unwrap();
        let front = citygen::clinic_front(city, b.id).unwrap();
        // A wall of it, seen from the pavement outside.
        let (wall, outside) = b
            .footprint
            .border()
            .filter(|p| {
                city.tile(StratumKind::Street, *p).is_some_and(|t| {
                    t.feature.is_none()
                        && matches!(t.terrain, citygen::Terrain::Wall | citygen::Terrain::Window)
                })
            })
            .find_map(|p| {
                p.neighbors4()
                    .find(|n| !b.footprint.contains(*n) && city.walkable(StratumKind::Street, *n))
                    .map(|n| (p, n))
            })
            .unwrap();
        let district = city.district(b.district).name.clone();
        st.player = Some(Player::spawn(city, 1));
        stand(&mut st, outside);
        let c = card(&st, wall);
        assert_eq!(c.second, Some(format!("{} · {district}", front.label())));
        let looks = Looks::load();
        let line = looks
            .line(
                Group::Building,
                &format!("{} front", front.label()),
                0,
                wall,
            )
            .unwrap();
        assert_eq!(c.body, vec![line.to_string()]);
        // Every front has its line.
        for f in citygen::Front::ALL {
            assert!(looks.has(&format!("{} front", f.label())), "{f:?}");
        }
    }

    #[test]
    fn every_flavour_line_is_house_voice() {
        let l = Looks::load();
        let lines = l.all_lines();
        assert!(lines.len() > 40, "{}", lines.len());
        for line in lines {
            assert!(!line.chars().any(|c| c.is_ascii_digit()), "{line}");
            assert!(!line.starts_with("You see"), "{line}");
            assert!(line.ends_with('.'), "{line}");
        }
        for k in [
            "lamp",
            "neon",
            "holo_ad",
            "terminal",
            "stairwell",
            "flat_door",
            "club",
        ] {
            assert!(l.has(k), "{k}");
        }
        assert!(l.line(Group::Feature, "lamp", 0, Pos::new(0, 0)).is_some());
    }

    #[test]
    fn look_reads_a_visitor() {
        use crate::jobs::testutil::state_home;
        use crate::missions::{Mark, Pick, Role, marks};
        let mut st = state_home();
        st.book.cast.insert(
            "seren".into(),
            Role {
                name: "Seren".into(),
                title: "ripperdoc".into(),
                pick: Pick::Visitor,
                face: None,
                tint: Some((255, 128, 0)),
                look: Some("Gloves, a long coat and a tired eye.".into()),
            },
        );
        let (place, me) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, me));
        let (_, beside) = marks::resolve(&st, &Mark::Beside(Box::new(Mark::You)), None).unwrap();
        crate::missions::visitors::arrive(&mut st, "seren", (place, beside));
        let mut p = Player::at(place, me);
        fn floor(st: &AppState, place: Place) -> MapRef<'_> {
            MapRef::for_place(
                st.city.as_ref().unwrap(),
                &st.interiors,
                &st.constructs,
                place,
                0,
            )
            .unwrap()
        }
        p.update_fov(&floor(&st, place), 0);
        st.player = Some(p);
        let map = floor(&st, place);
        let c = look_card(&st, &map, place, beside, &|_, _| None, &[]);
        assert_eq!(c.header.as_deref(), Some("Seren"));
        assert_eq!(c.glyph, Some(('@', Color::Rgb(255, 128, 0))));
        assert_eq!(c.second.as_deref(), Some("ripperdoc"));
        assert!(all_text(&c).contains("a tired eye"), "{}", all_text(&c));
        let near = points_of_interest(
            &st,
            &map,
            place,
            TileRect::new(me.x - 3, me.y - 3, 7, 7),
            me,
            &|_, _| None,
        );
        assert!(near.contains(&beside));
    }
}
