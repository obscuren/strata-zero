//! Where you are in the world, in words: HERE's place and road, the ground and the
//! hour, and NEAR's places with how far and which way. The overworld's panel says them,
//! and so does the city screen's sidebar off the live city's cells.

use super::{At, TILE};
use crate::app::AppState;
use crate::ui::chrome::CYAN;
use citygen::Pos;
use citygen::overworld::{Ground, Water};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const TEXT: Color = Color::Rgb(205, 212, 222);
const DIM: Color = Color::Rgb(110, 118, 130);

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

/// The place word for where you stand.
fn where_label(st: &AppState) -> &'static str {
    match st.outside.as_ref().map(|o| o.at) {
        Some(At::Overworld(_)) => "overworld",
        _ => "open ground",
    }
}

/// HERE: the region (or the city) and the road, then where you are, the ground and the
/// hour.
pub fn here_lines(st: &AppState) -> Vec<Line<'static>> {
    let (Some(o), Some(t)) = (st.outside.as_ref(), tile(st)) else {
        return Vec::new();
    };
    let w = o.world();
    let Some(tile) = w.tile(t) else {
        return Vec::new();
    };
    let place = match (super::settlement_at(st, t), tile.urban) {
        (Some(s), _) => o.name(s),
        (_, Some(u)) => w
            .cities
            .get(u.city as usize)
            .map_or("the edge of town".to_string(), |c| c.name.clone()),
        _ => w
            .regions
            .iter()
            .min_by_key(|r| (r.at.x - t.x).pow(2) + (4 * (r.at.y - t.y).pow(2)))
            .map_or("the open ground".to_string(), |r| r.name.clone()),
    };
    let road = w
        .routes
        .iter()
        .find(|r| r.path.contains(&t))
        .map(|r| format!(" · {}", r.name))
        .unwrap_or_default();
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
            format!(" {place}{road}"),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(" {} · {ground} · {hour}", where_label(st)),
            Style::default().fg(DIM),
        )),
    ]
}

/// HERE jacked in: the trunk your Net self is over, or the city's node field, or the
/// dead grid.
pub fn net_here_lines(st: &AppState) -> Vec<Line<'static>> {
    let (Some(o), Some(t)) = (st.outside.as_ref(), tile(st)) else {
        return Vec::new();
    };
    let w = o.world();
    let over = match (
        w.routes.iter().find(|r| r.path.contains(&t)),
        super::settlement_at(st, t),
    ) {
        (Some(r), _) => format!("{} trunk", r.name),
        (None, Some(s)) => o.name(s),
        (None, None) => "dead grid".to_string(),
    };
    vec![Line::from(Span::styled(
        format!(" the Net · {over}"),
        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
    ))]
}

/// NEAR: the mark from the MAP first, then the nearest cities and places out here, with
/// how far and which way; `n` lines at most.
pub fn near_lines(st: &AppState, n: usize) -> Vec<Line<'static>> {
    let mark = super::mark_way(st).map(|(name, d, dir)| {
        Line::from(vec![
            Span::styled(" ⌖ ", Style::default().fg(CYAN)),
            Span::styled(name, Style::default().fg(TEXT)),
            Span::styled(format!("  {d} {dir}"), Style::default().fg(DIM)),
        ])
    });
    let n = n.saturating_sub(mark.is_some() as usize);
    let mut out: Vec<Line<'static>> = mark.into_iter().collect();
    out.extend(nearest_lines(st, n));
    out
}

/// NEAR's places: the nearest cities and places out here, with how far and which way.
fn nearest_lines(st: &AppState, n: usize) -> Vec<Line<'static>> {
    let (Some(o), Some(t)) = (st.outside.as_ref(), tile(st)) else {
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
}
