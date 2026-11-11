//! A tapped camera's feed: a small window hanging off the camera's node on a wire, the
//! street around the camera's tile drawn live in green phosphor (each cell by its
//! light, walkers white, cameras red), headed `CAM · <BUILDING>` and `● REC`. Spec
//! `docs/superpowers/specs/2026-11-04-net-nodes-design.md`, "Camera: tap, loop, eyes".

use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use crate::render::people::PeopleOverlay;
use crate::render::viewport::{MapView, Overlay, tile_to_screen};
use citygen::{NodeKind, Place, Pos, StratumKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;

/// The feed's size, border included: smaller on a narrow screen.
pub fn size(wide: bool) -> (u16, u16) {
    if wide { (32, 12) } else { (28, 10) }
}

/// The border, the header and the wire.
pub const EDGE: Color = Color::Rgb(70, 210, 100);
const REC: Color = Color::Rgb(255, 50, 50);
const WALKER: Color = Color::Rgb(240, 255, 240);
const CAMERA: Color = Color::Rgb(255, 60, 60);
const BG: Color = Color::Rgb(3, 14, 6);
const SCAN_BG: Color = Color::Rgb(1, 8, 3);

/// A cell's green by the light on its tile.
pub fn phosphor(light: u8) -> Color {
    let l = (light as f32 / 255.0).clamp(0.18, 1.0);
    Color::Rgb((30.0 * l) as u8, (70.0 + 185.0 * l) as u8, (40.0 * l) as u8)
}

/// Where the feed goes in `area`: off the node's screen cell on the side with room
/// (not `avoid`, the dock's side, when the other fits), its row centred on the node's;
/// with no node on screen or no room, in the top right. The window's rect, and the
/// wire's cells on the node's row.
pub fn place(
    area: Rect,
    node: Option<(u16, u16)>,
    avoid_left: Option<bool>,
    wide: bool,
) -> (Rect, Vec<(u16, u16)>) {
    let (w, h) = size(wide);
    let (w, h) = (w.min(area.width), h.min(area.height));
    let corner = Rect::new(
        area.right().saturating_sub(w + 1).max(area.x),
        area.y + 1,
        w,
        h,
    );
    let Some((nx, ny)) = node else {
        return (corner, Vec::new());
    };
    let y = ny
        .saturating_sub(h / 2)
        .clamp(area.y, area.bottom().saturating_sub(h));
    let right = (nx + 3 + w <= area.right()).then(|| Rect::new(nx + 3, y, w, h));
    let left = (nx >= area.x + 2 + w).then(|| Rect::new(nx - 2 - w, y, w, h));
    let pick = match avoid_left {
        Some(true) => right.or(left),
        Some(false) => left.or(right),
        None => {
            let room_right = area.right().saturating_sub(nx);
            if room_right >= nx.saturating_sub(area.x) {
                right.or(left)
            } else {
                left.or(right)
            }
        }
    };
    let Some(win) = pick else {
        return (corner, Vec::new());
    };
    let wire = if win.x > nx {
        (nx + 1..win.x).map(|x| (x, ny)).collect()
    } else {
        (win.right()..nx).map(|x| (x, ny)).collect()
    };
    (win, wire)
}

/// Draws the open feed, if any, over the map: the camera's node on the map you are
/// on, or the corner when it isn't there.
pub fn draw(
    buf: &mut Buffer,
    area: Rect,
    origin: Pos,
    st: &AppState,
    avoid_left: Option<bool>,
    wide: bool,
) {
    let (Some(node), Some(city), Some(p)) = (st.net.feed, st.city.as_ref(), st.player.as_ref())
    else {
        return;
    };
    // A site node's feed is drawn by the site's own Net.
    let Some(n) = node.city().and_then(|id| city.net.nodes.get(id.index())) else {
        return;
    };
    let at =
        crate::holds::node_pos(st, p.place, node).and_then(|q| tile_to_screen(area, origin, q));
    let (win, wire) = place(area, at, avoid_left, wide);
    if win.width < 6 || win.height < 4 {
        return;
    }
    for (x, y) in wire {
        let cell = &mut buf[(x, y)];
        cell.set_char('━').set_fg(EDGE);
    }
    let title = crate::holds::feed_title(st, node);
    paint(buf, win, st, n.anchor, &title);
}

/// The window: border and header, and the street around `anchor` inside it.
pub fn paint(buf: &mut Buffer, win: Rect, st: &AppState, anchor: (StratumKind, Pos), title: &str) {
    let Some(city) = st.city.as_ref() else {
        return;
    };
    let inner = Rect::new(win.x + 1, win.y + 1, win.width - 2, win.height - 2);
    let origin = Pos::new(
        anchor.1.x - inner.width as i32 / 2,
        anchor.1.y - inner.height as i32 / 2,
    );
    // The street as the map draws it, into a scratch buffer, then recoloured.
    let minute = st.minute_of_day();
    let map = MapRef::stratum_at(city, anchor.0, minute);
    let scratch_area = Rect::new(0, 0, inner.width, inner.height);
    let mut scratch = Buffer::empty(scratch_area);
    let hour = minute / 60;
    let people = st
        .world
        .as_ref()
        .filter(|_| anchor.0 == StratumKind::Street)
        .map(|world| PeopleOverlay {
            world,
            night: hour >= 18 || hour <= 5,
            vis: None,
            eyes: &[],
            foes: &st.foes,
            bodies: &[],
            keep: None,
        });
    let overlays: Vec<&dyn Overlay> = people.iter().map(|o| o as &dyn Overlay).collect();
    MapView {
        map: &map,
        origin,
        tick: st.tick,
        style: &st.style,
        vis: None,
        keep: None,
        overlays: &overlays,
        cursor: None,
    }
    .render(scratch_area, &mut scratch);
    let movers: Vec<Pos> = st.world.as_ref().map_or_else(Vec::new, |w| {
        w.visible(citygen::Rect::new(
            origin.x,
            origin.y,
            inner.width as i32,
            inner.height as i32,
        ))
        .into_iter()
        .map(|m| m.pos)
        .collect()
    });
    let cameras: Vec<Pos> = city
        .net
        .nodes
        .iter()
        .filter(|c| c.kind == NodeKind::Camera && c.anchor.0 == anchor.0)
        .map(|c| c.anchor.1)
        .collect();
    let body = st
        .player
        .as_ref()
        .and_then(|p| p.body)
        .filter(|(place, _)| *place == Place::Stratum(anchor.0))
        .map(|(_, at)| at);
    for y in 0..inner.height {
        for x in 0..inner.width {
            let p = Pos::new(origin.x + x as i32, origin.y + y as i32);
            let src = scratch[(x, y)].clone();
            let fg = if movers.contains(&p) || body == Some(p) {
                WALKER
            } else if cameras.contains(&p) {
                CAMERA
            } else {
                phosphor(map.light(p))
            };
            let cell = &mut buf[(inner.x + x, inner.y + y)];
            cell.set_symbol(src.symbol());
            if body == Some(p) {
                cell.set_char('@');
            }
            cell.set_fg(fg)
                .set_bg(if y % 2 == 1 { SCAN_BG } else { BG });
            cell.modifier = Modifier::empty();
        }
    }
    // The border, the header on it, `● REC` at its right.
    let right = win.right() - 1;
    let bottom = win.bottom() - 1;
    for x in win.x..=right {
        for y in [win.y, bottom] {
            let ch = match (x == win.x, x == right, y == win.y) {
                (true, _, true) => '┌',
                (_, true, true) => '┐',
                (true, _, false) => '└',
                (_, true, false) => '┘',
                _ => '─',
            };
            buf[(x, y)].set_char(ch).set_fg(EDGE).set_bg(BG);
        }
    }
    for y in win.y + 1..bottom {
        for x in [win.x, right] {
            buf[(x, y)].set_char('│').set_fg(EDGE).set_bg(BG);
        }
    }
    let rec = " ● REC ";
    let room = (win.width as usize).saturating_sub(rec.chars().count() + 3);
    let head: String = format!(" {title} ").chars().take(room).collect();
    for (i, ch) in head.chars().enumerate() {
        let cell = &mut buf[(win.x + 1 + i as u16, win.y)];
        cell.set_char(ch).set_fg(EDGE);
        cell.modifier.insert(Modifier::BOLD);
    }
    let blink = (st.tick / 20).is_multiple_of(2);
    let rx = right - rec.chars().count() as u16;
    for (i, ch) in rec.chars().enumerate() {
        let fg = if ch == '●' {
            if blink { REC } else { BG }
        } else {
            EDGE
        };
        buf[(rx + i as u16, win.y)].set_char(ch).set_fg(fg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::tests::{crack_to_landing, node_cyber, under_kiroflux};
    use game::HackTarget;

    #[test]
    fn the_feed_hangs_off_the_node_on_the_side_with_room() {
        let area = Rect::new(0, 0, 80, 30);
        let (win, wire) = place(area, Some((20, 15)), None, true);
        assert_eq!((win.width, win.height), (32, 12));
        assert!(win.x > 20, "right has more room: {win:?}");
        assert_eq!(wire.first(), Some(&(21, 15)));
        assert!(win.y <= 15 && 15 < win.bottom());
        // Away from the dock's side when the other fits.
        let (win, _) = place(area, Some((40, 15)), Some(false), true);
        assert!(win.right() <= 40, "{win:?}");
        // No node on screen: the top right, no wire.
        let (win, wire) = place(area, None, None, false);
        assert_eq!((win.width, win.height), (28, 10));
        assert!(wire.is_empty() && win.right() < 80);
    }

    #[test]
    fn the_feed_draws_the_street_in_phosphor() {
        let (mut st, b) = under_kiroflux();
        let (cam, at) = node_cyber(&st, b, NodeKind::Camera);
        st.player.as_mut().unwrap().pos = Pos::new(at.x, at.y + 1);
        crack_to_landing(&mut st, HackTarget::Node(cam));
        crate::holds::use_verb(&mut st, 'f').unwrap();
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        let me = st.player.as_ref().unwrap().pos;
        let origin = Pos::new(me.x - 50, me.y - 20);
        draw(&mut buf, area, origin, &st, None, true);
        let text: String = (0..area.height)
            .flat_map(|y| (0..area.width).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol().to_string())
            .collect();
        assert!(text.contains("CAM · KIROFLUX MART"), "the header");
        assert!(text.contains("● REC"));
        // Every cell inside the frame is drawn green, white or red on the phosphor.
        let greens = (0..area.height)
            .flat_map(|y| (0..area.width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                matches!(buf[(x, y)].fg, Color::Rgb(r, g, b) if g > 60 && r < 40 && b < 45)
            })
            .count();
        assert!(greens > 100, "{greens} green cells");
    }
}
