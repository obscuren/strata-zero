//! The walk on the street: the leg's tiles glow under their glyphs, the goal ripples,
//! and a tag on the edge names the way when the goal is out of view. Spec
//! `docs/superpowers/specs/2026-10-31-tracking-and-map-design.md`, 4.

use super::Pulse;
use super::route::{Leg, Through};
use crate::render::Rgb;
use crate::render::map::{MapRef, TileMap};
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::{CYAN, CYAN_RGB, MAGENTA_RGB};
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::f32::consts::{PI, TAU};

/// The tag's text and the ground it sits on.
const TAG_FG: Rgb = Rgb(205, 212, 222);
const TAG_BG: Rgb = Rgb(4, 6, 10);

/// How much of the cyan a tile `g` tiles from the goal along the walk takes at tick
/// `t`: a slow breath, and each pulse's head with its trail behind it, the brightest.
pub fn glow(g: usize, pulses: &[Pulse], t: u64) -> f32 {
    let breathe = 0.22 + 0.06 * (TAU * t as f32 / 60.0).sin();
    pulses.iter().fold(breathe, |f, p| {
        let d = g as f32 - p.head(t);
        if (0.0..12.0).contains(&d) {
            f.max(0.85 * (1.0 - d / 12.0))
        } else {
            f
        }
    })
}

/// How far from the goal the `i`th tile of an `n`-tile walk is.
pub fn from_goal(i: usize, n: usize) -> usize {
    n.saturating_sub(i + 1)
}

/// How much of the colour a cell `dx`, `dy` tiles from the goal takes at tick `t`: a
/// ring going out from a pulsing core.
pub fn ripple(dx: i32, dy: i32, t: u64) -> f32 {
    let t = t as f32;
    let r = (t / 14.0) % 4.0;
    let d = dx.abs() as f32 + 1.6 * dy.abs() as f32;
    let ring = (1.0 - (d - r * 1.4).abs() / 1.1).max(0.0) * (1.0 - r / 4.0);
    let core = if d < 0.5 {
        0.35 + 0.2 * (PI * t / 15.0).sin()
    } else {
        0.0
    };
    (ring * 0.55).max(core)
}

/// The map's ripple on its pixels: how much of the colour a pixel `d` pixels from the
/// target takes at tick `t`, rings 1.6 pixels apart going out from a pulsing core.
pub fn ripple_px(d: f32, t: u64) -> f32 {
    let t = t as f32;
    let r = (t / 14.0) % 4.0;
    let ring = (1.0 - (d - 1.6 * r).abs() / 1.2).max(0.0) * (1.0 - r / 4.0);
    let core = if d < 0.8 {
        0.4 + 0.25 * (PI * t / 15.0).sin()
    } else {
        0.0
    };
    (0.7 * ring).max(core)
}

fn rgb(c: Color) -> Rgb {
    match c {
        Color::Rgb(r, g, b) => Rgb(r, g, b),
        _ => Rgb(0, 0, 0),
    }
}

/// `a` blended toward `b` by `f`.
pub fn mix(a: Color, b: Color, f: f32) -> Color {
    rgb(a).mix(rgb(b), f).to_color()
}

/// The leg's glow, under the glyphs: backgrounds only.
pub struct TrackOverlay<'a> {
    pub leg: &'a Leg,
    pub pulses: &'a [Pulse],
    /// The goal is the target itself (a magenta ripple), not a way on (a cyan one).
    pub arrive: bool,
}

impl Overlay for TrackOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        let n = self.leg.path.len();
        for (i, p) in self.leg.path.iter().enumerate() {
            if let Some((sx, sy)) = tile_to_screen(area, origin, *p) {
                let c = &mut buf[(sx, sy)];
                let f = glow(from_goal(i, n), self.pulses, tick);
                c.set_bg(mix(c.bg, CYAN, f));
            }
        }
        let (colour, strength) = if self.arrive {
            (MAGENTA_RGB.to_color(), 1.0)
        } else {
            (CYAN_RGB.to_color(), 0.5)
        };
        let g = self.leg.goal;
        for dy in -3..=3 {
            for dx in -4..=4 {
                let p = Pos::new(g.x + dx, g.y + dy);
                if map.tile(p).is_none() {
                    continue;
                }
                let f = ripple(dx, dy, tick) * strength;
                if f <= 0.02 {
                    continue;
                }
                if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
                    let c = &mut buf[(sx, sy)];
                    c.set_bg(mix(c.bg, colour, f));
                }
            }
        }
    }
}

/// Which edge of the view the walk leaves by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Edge {
    Up,
    Down,
    Left,
    Right,
}

impl Edge {
    fn arrow(self) -> char {
        match self {
            Edge::Up => '▴',
            Edge::Down => '▾',
            Edge::Left => '◂',
            Edge::Right => '▸',
        }
    }
}

/// The edge of the view that the tile `p`, outside it, lies beyond.
fn edge_of(area: Rect, origin: Pos, p: Pos) -> Edge {
    let (x, y) = (p.x - origin.x, p.y - origin.y);
    if y < 0 {
        Edge::Up
    } else if y >= area.height as i32 {
        Edge::Down
    } else if x < 0 {
        Edge::Left
    } else {
        Edge::Right
    }
}

/// When the goal is out of view, ` ▴ text ` one row (or column) inside the edge the
/// walk leaves by, the arrow pulsing magenta, clamped inside `area`. `text` is the
/// words and the distance, without the arrow.
pub fn draw_edge_tag(buf: &mut Buffer, area: Rect, origin: Pos, leg: &Leg, text: &str, t: u64) {
    if area.width < 8 || area.height < 4 || tile_to_screen(area, origin, leg.goal).is_some() {
        return;
    }
    // The last tile in view, and the way out of it.
    let seen = leg
        .path
        .iter()
        .rposition(|p| tile_to_screen(area, origin, *p).is_some());
    let (anchor, edge) = match seen {
        Some(i) => {
            let out = leg.path.get(i + 1).copied().unwrap_or(leg.goal);
            (leg.path[i], edge_of(area, origin, out))
        }
        // None of the walk is in view: point at the goal from the middle.
        None => {
            let mid = Pos::new(
                origin.x + area.width as i32 / 2,
                origin.y + area.height as i32 / 2,
            );
            (mid, edge_of(area, origin, leg.goal))
        }
    };
    let chars: Vec<char> = format!(" {} {} ", edge.arrow(), text)
        .chars()
        .take(area.width as usize)
        .collect();
    let len = chars.len() as i32;
    let ax = anchor.x - origin.x + area.x as i32;
    let ay = anchor.y - origin.y + area.y as i32;
    let (left, right) = (area.x as i32, area.right() as i32 - len);
    let (top, bottom) = (area.y as i32 + 1, area.bottom() as i32 - 2);
    let (x, y) = match edge {
        Edge::Up => (ax - len / 2, top),
        Edge::Down => (ax - len / 2, bottom),
        Edge::Left => (left + 1, ay),
        Edge::Right => (right - 1, ay),
    };
    let x = x.clamp(left, right);
    let y = y.clamp(top, bottom);
    let pulse = 0.65 + 0.35 * (TAU * t as f32 / 30.0).sin();
    for (k, ch) in chars.iter().enumerate() {
        let cell = &mut buf[((x + k as i32) as u16, y as u16)];
        let (fg, mods) = if k == 1 {
            (TAG_BG.mix(MAGENTA_RGB, pulse), Modifier::BOLD)
        } else {
            (TAG_BG.mix(TAG_FG, 0.9), Modifier::empty())
        };
        cell.set_char(*ch);
        cell.set_style(
            Style::default()
                .fg(fg.to_color())
                .bg(TAG_BG.to_color())
                .add_modifier(mods),
        );
        cell.modifier = mods;
    }
}

/// The tag's words for this leg: the target's name, after the way on when the leg does
/// not arrive, then the distance.
pub fn tag_text(label: &str, next: Option<&str>, through: Through, metres: usize) -> String {
    match (through, next) {
        (Through::Arrive, _) | (_, None) => format!("{label} · {metres} m"),
        (_, Some(next)) => format!("{next} · {label} · {metres} m"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{here, outside, reach, steps_off};
    use super::super::{Source, set};
    use super::*;
    use crate::app::{AppState, Screen};
    use crate::screens::explore::MapScreen;
    use crate::screens::testutil::key;
    use citygen::{Place, StratumKind};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const STREET: Place = Place::Stratum(StratumKind::Street);

    #[test]
    fn glow_breathes_and_the_pulses_run_to_the_goal() {
        // No pulse: only the breath.
        assert!((glow(5, &[], 0) - 0.22).abs() < 1e-3);
        for t in 0..200 {
            let f = glow(7, &[], t);
            assert!((0.16..=0.28).contains(&f), "t {t} f {f}");
        }
        // Fired at 100 on a 40-tile walk: at 120 the head is 40 − 18 = 22 from the goal.
        let p = [Pulse { t0: 100, len: 40 }];
        assert_eq!(p[0].head(120), 22.0);
        assert!((glow(22, &p, 120) - 0.85).abs() < 1e-3);
        // The trail is behind it, away from the goal; ahead of it, only the breath.
        assert!((glow(28, &p, 120) - 0.425).abs() < 1e-3);
        assert_eq!(glow(21, &p, 120), glow(21, &[], 120));
        assert_eq!(glow(34, &p, 120), glow(34, &[], 120));
        // It runs towards the goal.
        let at = |t: u64| (0..40).max_by(|a, b| glow(*a, &p, t).total_cmp(&glow(*b, &p, t)));
        assert!(at(130) < at(120));
        // Two at once: the brightest wins.
        let two = [Pulse { t0: 100, len: 40 }, Pulse { t0: 110, len: 40 }];
        assert!((glow(31, &two, 120) - 0.85).abs() < 1e-3);
        assert!((glow(22, &two, 120) - 0.85).abs() < 1e-3);
        assert_eq!(from_goal(0, 40), 39);
        assert_eq!(from_goal(39, 40), 0);
    }

    #[test]
    fn the_tag_names_the_way_and_the_place() {
        let at = |next, through| tag_text("Velform Group Plaza", next, through, 12);
        assert_eq!(at(None, Through::Arrive), "Velform Group Plaza · 12 m");
        assert_eq!(
            at(Some("building"), Through::Door(citygen::BuildingId(3))),
            "building · Velform Group Plaza · 12 m"
        );
        assert_eq!(
            at(Some("lift to 6F"), Through::Lift(6)),
            "lift to 6F · Velform Group Plaza · 12 m"
        );
        assert_eq!(at(None, Through::Exit), "Velform Group Plaza · 12 m");
    }

    #[test]
    fn ripple_rings_out_and_fades() {
        for t in 0..120 {
            let c = ripple(0, 0, t);
            assert!((0.15..=0.55).contains(&c), "t {t} core {c}");
        }
        // t = 14: r = 1, so the ring is at d = 1.4.
        assert!(ripple(1, 0, 14) > 0.0);
        assert!(ripple(0, 1, 14) > ripple(2, 0, 14));
        let ring = ripple(1, 0, 14);
        assert!(ring > 0.2 && ring < ripple(0, 0, 14) + 0.3);
        for t in 0..120 {
            assert_eq!(ripple(4, 3, t), 0.0);
            assert_eq!(ripple(7, 0, t), 0.0);
        }
    }

    #[test]
    fn the_map_ripple_rings_out_in_pixels() {
        // The core pulses, and a ring leaves it at full strength.
        for t in 0..120 {
            let c = ripple_px(0.0, t);
            assert!((0.15..=0.7).contains(&c), "t {t} core {c}");
        }
        assert!((ripple_px(0.0, 0) - 0.7).abs() < 1e-3);
        // t = 14: r = 1, so the ring stands at d = 1.6, 0.7 · 0.75 strong.
        assert!((ripple_px(1.6, 14) - 0.525).abs() < 1e-3);
        assert!(ripple_px(1.6, 14) > ripple_px(3.0, 14));
        assert_eq!(ripple_px(2.9, 14), 0.0);
        // Never past the last ring.
        for t in 0..120 {
            assert_eq!(ripple_px(7.7, t), 0.0);
        }
    }

    fn draw(s: &MapScreen, st: &AppState, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| s.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    /// A tracked street spot `n` steps off, you on the street.
    fn tracked(n: u32) -> (AppState, MapScreen) {
        let mut st = outside();
        let s = MapScreen::walking(&mut st);
        let spot = Source::Spot {
            place: STREET,
            pos: steps_off(&st, n),
            radius: 1,
        };
        set(&mut st, spot).unwrap();
        (st, s)
    }

    #[test]
    fn the_overlay_only_changes_backgrounds() {
        // Near enough that the goal is in view, so no edge tag writes a glyph.
        let (st, s) = tracked(8);
        let with = draw(&s, &st, 60, 20);
        let mut off = st;
        off.track = None;
        let without = draw(&s, &off, 60, 20);
        // The map only: the sidebar's JOBS pane has the track's own line.
        let area = s.map_area();
        let mut bgs = 0;
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let (a, b) = (&with[(x, y)], &without[(x, y)]);
                assert_eq!(a.symbol(), b.symbol());
                assert_eq!(a.fg, b.fg);
                bgs += (a.bg != b.bg) as usize;
            }
        }
        assert!(bgs > 0, "the glow shows");
    }

    #[test]
    fn the_glow_draws_only_in_its_place() {
        let (mut st, _) = tracked(15);
        st.unlock_dev();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        assert_ne!(s.place, STREET, "the camera moved off the street");
        let with = draw(&s, &st, 60, 20);
        let mut off = st;
        off.track = None;
        let without = draw(&s, &off, 60, 20);
        // The sidebar's JOBS pane has the track's own line; the rest is the map's.
        let area = s.map_area();
        for y in area.y..area.bottom() {
            if (area.x..area.right()).any(|x| with[(x, y)].symbol() == "⌖") {
                continue;
            }
            for x in area.x..area.right() {
                assert_eq!(with[(x, y)], without[(x, y)], "at {x},{y}");
            }
        }
    }

    #[test]
    fn the_edge_tag_sits_where_the_walk_leaves() {
        // The farthest-north street tile within 60 steps.
        let mut st = outside();
        let s = MapScreen::walking(&mut st);
        let me = here(&st);
        let goal = reach(&st, me)
            .into_iter()
            .filter(|(p, k)| *k <= 60 && p.y < me.y - 40)
            .min_by_key(|(p, k)| (p.y, *k, *p))
            .map(|(p, _)| p)
            .expect("a street tile far north");
        let spot = Source::Spot {
            place: STREET,
            pos: goal,
            radius: 1,
        };
        set(&mut st, spot).unwrap();
        let leg = super::super::leg_now(&st).unwrap().unwrap();
        let buf = draw(&s, &st, 100, 30);
        let area = s.map_area();
        let want = format!("{} m", leg.path.len());
        let mut found = None;
        for y in area.y..area.bottom() {
            let row: String = (area.x..area.right())
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            if row.contains(&format!("· {want}")) {
                found = Some((y, row));
            }
        }
        let (y, row) = found.expect("a tag");
        assert!(row.contains('▴'), "{row}");
        assert_eq!(y, area.y + 1, "one row inside the top edge");
    }
}
