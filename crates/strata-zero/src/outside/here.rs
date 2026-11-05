//! Where you are in the world, in words: HERE's place and road, the ground and the
//! hour, and NEAR's places with how far and which way. The overworld's panel says them,
//! and so does the city screen's sidebar off the live city's cells.

use super::{At, Outside, Settlement, TILE};
use crate::app::AppState;
use crate::ui::chrome::{CYAN, DIM, TEXT};
use citygen::Pos;
use citygen::overworld::{Ground, Water};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// The overworld tile you're on: on the overworld, the one you stand on; in the city,
/// the one your world cell lies in (your Net self's, jacked in), on its map or past it.
pub fn tile(st: &AppState) -> Option<Pos> {
    let o = st.outside.as_ref()?;
    Some(match o.at {
        At::Overworld(t) => t,
        At::Ground(c) => Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)),
        At::City => o.tile_of(st.city.as_ref()?, st.player.as_ref()?.pos),
    })
}

/// The place word for where you stand: the overworld, the street of a city or town
/// (its cells, though it isn't the live one), or the open ground.
fn where_label(st: &AppState) -> &'static str {
    let Some(o) = st.outside.as_ref() else {
        return "open ground";
    };
    match (o.at, tile(st)) {
        (At::Overworld(_), _) => "overworld",
        (_, Some(t)) if super::tile_owner(st, t).is_some() => "street",
        _ => "open ground",
    }
}

/// What the street of overworld tile `t` is part of, `owner` the city or town that owns
/// it: that one, else the city whose outskirts it is; none on the open ground. HERE,
/// the top band and the minimap name the street by it.
pub fn street_name(o: &Outside, owner: Option<Settlement>, t: Pos) -> Option<String> {
    let w = o.world();
    match (owner, w.tile(t)?.urban) {
        (Some(s), _) => Some(o.name(s)),
        (_, Some(u)) => Some(
            w.cities
                .get(u.city as usize)
                .map_or("the edge of town".to_string(), |c| c.name.clone()),
        ),
        _ => None,
    }
}

/// What the Net over overworld tile `t` is called, `owner` the city or town that owns
/// it: that one, else the trunk over the tile, else the dead grid. HERE, the top band,
/// the minimap and the status bar name a cell of the Net by it.
pub fn net_name(o: &Outside, owner: Option<Settlement>, t: Pos) -> String {
    match (owner, o.world().routes.iter().find(|r| r.path.contains(&t))) {
        (Some(s), _) => o.name(s),
        (None, Some(r)) => format!("{} trunk", r.name),
        (None, None) => "dead grid".to_string(),
    }
}

/// HERE's first line: the region (or the city) you're in and the road you're on.
fn place_and_road(st: &AppState) -> Option<String> {
    let (o, t) = (st.outside.as_ref()?, tile(st)?);
    let w = o.world();
    w.tile(t)?;
    let place = street_name(o, super::tile_owner(st, t), t).unwrap_or_else(|| {
        w.regions
            .iter()
            .min_by_key(|r| (r.at.x - t.x).pow(2) + (4 * (r.at.y - t.y).pow(2)))
            .map_or("the open ground".to_string(), |r| r.name.clone())
    });
    let road = w
        .routes
        .iter()
        .find(|r| r.path.contains(&t))
        .map(|r| format!(" · {}", r.name))
        .unwrap_or_default();
    Some(format!("{place}{road}"))
}

/// HERE: the region (or the city) and the road, then where you are, the ground and the
/// hour.
pub fn here_lines(st: &AppState) -> Vec<Line<'static>> {
    let (Some(o), Some(t)) = (st.outside.as_ref(), tile(st)) else {
        return Vec::new();
    };
    let (Some(tile), Some(place)) = (o.world().tile(t), place_and_road(st)) else {
        return Vec::new();
    };
    let ground = match (tile.water, tile.ground) {
        (Water::River, _) if tile.road != 0 => "a bridge",
        (Water::River, _) => "the river",
        (_, Ground::Flats) => "flats",
        (_, Ground::Scrub) => "scrub",
        (_, Ground::Fields) => "fields",
        (_, Ground::Salt) => "salt pan",
        (_, Ground::Spill) => "the spill",
        (_, Ground::Hills) => "hills",
        (_, Ground::Mountains | Ground::Peak) => "mountains",
    };
    let hour = if citygen::lighting::daylight(st.minute_of_day()) > 0.25 {
        "day"
    } else {
        "night"
    };
    vec![
        Line::from(Span::styled(
            format!(" {place}"),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(" {} · {ground} · {hour}", where_label(st)),
            Style::default().fg(DIM),
        )),
    ]
}

/// HERE's line jacked in: the Net, and the city or town whose tile your Net self is
/// over, or the trunk, or the dead grid.
fn net_over(st: &AppState) -> Option<String> {
    net_over_at(st, tile(st)?)
}

/// HERE's line jacked in over overworld tile `t`: the Net, and the city or town that
/// owns the tile, or the trunk over it off every settlement's, or the dead grid.
fn net_over_at(st: &AppState, t: Pos) -> Option<String> {
    let o = st.outside.as_ref()?;
    Some(on_the_net(&net_name(o, super::tile_owner(st, t), t)))
}

/// The words for being jacked in over `over` (a city, a trunk, the dead grid), as HERE
/// and the minimap say them.
pub fn on_the_net(over: &str) -> String {
    format!("the Net · {over}")
}

/// HERE jacked in: the city or town whose tile your Net self is over, or the trunk, or
/// the dead grid.
pub fn net_here_lines(st: &AppState) -> Vec<Line<'static>> {
    tile(st)
        .map(|t| net_here_lines_at(st, t))
        .unwrap_or_default()
}

/// HERE jacked in over overworld tile `t` (the Net overworld's): the city or town that
/// owns it, or its trunk, or the dead grid.
pub fn net_here_lines_at(st: &AppState, t: Pos) -> Vec<Line<'static>> {
    net_over_at(st, t)
        .map(|over| {
            vec![Line::from(Span::styled(
                format!(" {over}"),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ))]
        })
        .unwrap_or_default()
}

/// Where you are off the live city's cells, for the top band, as HERE names it: the
/// place and the road on foot, the Net and what it runs over on the Net.
pub fn band(st: &AppState, net: bool) -> Option<String> {
    if net {
        net_over(st)
    } else {
        place_and_road(st)
    }
}

/// NEAR: the mark from the MAP first, then the nearest cities and places out here, with
/// how far and which way; `n` lines at most.
pub fn near_lines(st: &AppState, n: usize) -> Vec<Line<'static>> {
    let Some(t) = tile(st) else {
        return Vec::new();
    };
    near_from(st, super::your_tile(st), t, n)
}

/// NEAR from overworld tile `t` (the Net overworld's): the mark, then the nearest
/// places, each from there.
pub fn near_lines_at(st: &AppState, t: Pos, n: usize) -> Vec<Line<'static>> {
    near_from(st, Some(t), t, n)
}

/// NEAR with the mark's way from tile `you` and the places' from tile `t`.
fn near_from(st: &AppState, you: Option<Pos>, t: Pos, n: usize) -> Vec<Line<'static>> {
    let mark = you
        .and_then(|you| super::mark_way_from(st, you))
        .map(|(name, d, dir)| {
            Line::from(vec![
                Span::styled(" ⌖ ", Style::default().fg(CYAN)),
                Span::styled(name, Style::default().fg(TEXT)),
                Span::styled(format!("  {d} {dir}"), Style::default().fg(DIM)),
            ])
        });
    let n = n.saturating_sub(mark.is_some() as usize);
    let mut out: Vec<Line<'static>> = mark.into_iter().collect();
    out.extend(nearest_lines(st, t, n));
    out
}

/// NEAR's places from overworld tile `t`: the nearest cities and places out here, with
/// how far and which way.
fn nearest_lines(st: &AppState, t: Pos, n: usize) -> Vec<Line<'static>> {
    let Some(o) = st.outside.as_ref() else {
        return Vec::new();
    };
    let w = o.world();
    // the city whose tiles you stand on is here, whatever its middle's distance
    let in_city = |i: usize| {
        w.tile(t)
            .and_then(|x| x.urban)
            .is_some_and(|u| u.city as usize == i)
            || super::settlement_at(st, t).is_some_and(|s| s.town.is_none() && s.city as usize == i)
    };
    let mut places: Vec<(i32, String, Pos)> = w
        .cities
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.clone(), c.centre, in_city(i)))
        .chain(w.pois.iter().map(|p| (p.name.clone(), p.at, p.at == t)))
        .map(|(name, at, here)| {
            let (dx, dy) = (at.x - t.x, at.y - t.y);
            let d = if here {
                0
            } else {
                ((dx * dx + dy * dy) as f32).sqrt().round().max(1.0) as i32
            };
            (d, name, at)
        })
        .collect();
    places.sort();
    places
        .into_iter()
        .take(n)
        .map(|(d, name, at)| {
            let way = if d == 0 {
                "here".to_string()
            } else {
                format!("{d} {}", bearing(at.x - t.x, at.y - t.y))
            };
            Line::from(vec![
                Span::styled(format!(" {name}"), Style::default().fg(TEXT)),
                Span::styled(format!("  {way}"), Style::default().fg(DIM)),
            ])
        })
        .collect()
}

/// The compass point from you toward `(dx, dy)`.
pub fn bearing(dx: i32, dy: i32) -> &'static str {
    let (ax, ay) = (dx.abs(), dy.abs());
    match (dx.signum(), dy.signum()) {
        _ if ax > 2 * ay => {
            if dx > 0 {
                "E"
            } else {
                "W"
            }
        }
        _ if ay > 2 * ax => {
            if dy > 0 {
                "S"
            } else {
                "N"
            }
        }
        (1, 1) => "SE",
        (1, _) => "NE",
        (_, 1) => "SW",
        _ => "NW",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outside::moves::tests::on_the_world;

    fn text(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect()
    }

    #[test]
    fn here_and_near_are_dim_as_the_sidebar_is() {
        let mut st = on_the_world();
        crate::outside::moves::up(&mut st);
        st.outside.as_mut().unwrap().mark = Some(crate::outside::Mark {
            tile: Pos::new(0, 0),
            name: "the old relay".into(),
        });
        let dim = |lines: &[Line]| {
            lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .filter(|s| !s.content.trim().is_empty())
                .filter_map(|s| s.style.fg)
                .filter(|c| *c != CYAN && *c != crate::ui::chrome::TEXT)
                .collect::<Vec<_>>()
        };
        for lines in [here_lines(&st), near_lines(&st, 4)] {
            let quiet = dim(&lines);
            assert!(!quiet.is_empty(), "{:?}", text(&lines));
            assert!(
                quiet.iter().all(|c| *c == crate::ui::chrome::DIM),
                "{quiet:?}"
            );
        }
    }

    #[test]
    fn the_city_you_stand_in_is_here() {
        let mut st = on_the_world();
        crate::outside::moves::up(&mut st);
        let home = st.outside.as_ref().unwrap().world().home().name.clone();
        let lines = text(&near_lines(&st, 4));
        assert!(
            lines
                .iter()
                .any(|l| l.contains(&home) && l.contains("here")),
            "{lines:?}"
        );
    }

    #[test]
    fn here_names_the_town_whose_tile_it_is() {
        // a town's own tiles inside its city's map, the city live: on the overworld and
        // jacked in over them, HERE names the town, not the city whose map it is
        use crate::outside::stream::tests::beyond::city_with_a_town;
        use crate::outside::stream::tests::handover::{hold, local};
        let mut st = crate::outside::tests::home_run();
        let (c, town) = city_with_a_town(&st);
        crate::outside::enter(&mut st, c).unwrap();
        hold(&mut st, town);
        let (t, name) = {
            let o = st.outside.as_ref().unwrap();
            let city = st.city.as_ref().unwrap();
            let routes = &o.world().routes;
            let t = o
                .block(city)
                .iter()
                .find(|t| o.owner(*t) == Some(town) && !routes.iter().any(|r| r.path.contains(t)))
                .expect("a tile of the town's off the roads");
            (t, o.name(town))
        };
        st.outside.as_mut().unwrap().at = At::Overworld(t);
        assert_eq!(
            place_and_road(&st).unwrap().split(" · ").next(),
            Some(name.as_str())
        );
        st.outside.as_mut().unwrap().at = At::City;
        let cell = local(&st, Pos::new(TILE * t.x + TILE / 2, TILE * t.y + TILE / 2));
        let p = st.player.as_mut().unwrap();
        p.body = Some((p.place, p.pos));
        p.place = citygen::Place::Stratum(citygen::StratumKind::Net);
        p.pos = cell;
        assert_eq!(net_over(&st).unwrap(), format!("the Net · {name}"));
    }

    #[test]
    fn here_names_the_city_on_its_lanes() {
        // jacked in on a lane of another city's Net, on a tile of its own a highway's
        // trunk runs through: HERE names the city; off every settlement's cells, the
        // trunk
        use crate::outside::stream::tests::handover::{hold, local};
        let mut st = crate::outside::tests::home_run();
        let on_route = |st: &AppState, t: Pos| {
            let routes = &st.outside.as_ref().unwrap().world().routes;
            routes
                .iter()
                .find(|r| r.path.contains(&t))
                .map(|r| r.name.clone())
        };
        let cities: Vec<_> = {
            let o = st.outside.as_ref().unwrap();
            o.others()
                .into_iter()
                .filter(|(s, _)| s.town.is_none())
                .collect()
        };
        let found = cities.into_iter().find_map(|(s, block)| {
            hold(&mut st, s);
            let o = st.outside.as_ref().unwrap();
            let city = o.held_city(s).unwrap();
            let origin = o.origin(city);
            let net = &city.stratum(citygen::StratumKind::Net).unwrap().tiles;
            block
                .iter()
                .filter(|t| o.owner(*t) == Some(s) && on_route(&st, *t).is_some())
                .find_map(|t| {
                    citygen::Rect::new(TILE * t.x, TILE * t.y, TILE, TILE)
                        .iter()
                        .find(|c| {
                            net.get(Pos::new(c.x - origin.x, c.y - origin.y))
                                .is_some_and(|x| x.terrain == citygen::Terrain::NetLane)
                        })
                })
                .map(|c| (o.name(s), c))
        });
        let (name, at) = found.expect("a lane of another city's on a tile with a trunk");
        let cell = local(&st, at);
        let p = st.player.as_mut().unwrap();
        p.body = Some((p.place, p.pos));
        p.place = citygen::Place::Stratum(citygen::StratumKind::Net);
        p.pos = cell;
        assert_eq!(net_over(&st).unwrap(), format!("the Net · {name}"));
        // a trunk's tile no settlement owns
        let (t, route) = {
            let o = st.outside.as_ref().unwrap();
            o.world()
                .routes
                .iter()
                .flat_map(|r| r.path.iter().copied())
                .find(|t| o.owner(*t).is_none())
                .map(|t| (t, on_route(&st, t).unwrap()))
                .expect("a trunk on the open ground")
        };
        st.player.as_mut().unwrap().pos =
            local(&st, Pos::new(TILE * t.x + TILE / 2, TILE * t.y + TILE / 2));
        assert_eq!(net_over(&st).unwrap(), format!("the Net · {route} trunk"));
    }
}
