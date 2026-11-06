//! The MAP's keys (spec 7): the cursor, the places list, the kinds, the track, the
//! zoom and the fold. Everything else is the deck's.

use super::Zoom;
use super::map::MapLayout;
use super::{cached, pane, refresh, relayout, spot_source, under_cursor};
use crate::app::AppState;
use crate::track;
use crate::track::listings::{Kind, source_of};
use citygen::Pos;
use crossterm::event::KeyCode;

#[derive(Clone, Copy)]
enum Act {
    /// The cursor, by cells.
    Move(i32, i32),
    /// The places list.
    List(i32),
    Kind(i32),
    Track,
    Zoom {
        closer: bool,
    },
    Fold,
}

/// Handles a key on the MAP tab; false for the keys the deck keeps (`q e`, the digits,
/// tab, esc and the rest).
pub fn key(st: &mut AppState, code: KeyCode) -> bool {
    if st.atlas.zoom == Zoom::World && super::world::available(st) {
        return super::world::key(st, code);
    }
    let world = super::world::available(st);
    let act = match code {
        KeyCode::Left | KeyCode::Char('h') => Act::Move(-1, 0),
        KeyCode::Right | KeyCode::Char('l') => Act::Move(1, 0),
        KeyCode::Up | KeyCode::Char('k') => Act::Move(0, -1),
        KeyCode::Down | KeyCode::Char('j') => Act::Move(0, 1),
        KeyCode::Char('H') => Act::Move(-8, 0),
        KeyCode::Char('L') => Act::Move(8, 0),
        KeyCode::Char('K') => Act::Move(0, -8),
        KeyCode::Char('J') => Act::Move(0, 8),
        KeyCode::Char('[') => Act::List(-1),
        KeyCode::Char(']') => Act::List(1),
        KeyCode::Char('<') => Act::Kind(-1),
        KeyCode::Char('>') => Act::Kind(1),
        KeyCode::Char('t' | 'T' | ' ') => Act::Track,
        KeyCode::Char('+' | '=') => Act::Zoom { closer: true },
        KeyCode::Char('-') => Act::Zoom { closer: false },
        KeyCode::Char('f' | 'F') => Act::Fold,
        _ => return false,
    };
    let atlas = &mut st.atlas;
    match act {
        Act::Kind(d) => {
            // All, then each kind, and round again.
            let n = Kind::ALL.len() as i32 + 1;
            let at = atlas
                .kind
                .and_then(|k| Kind::ALL.iter().position(|x| *x == k));
            let next = (at.map_or(0, |i| i as i32 + 1) + d).rem_euclid(n);
            atlas.kind = (next > 0).then(|| Kind::ALL[next as usize - 1]);
            atlas.scroll = 0;
        }
        Act::Zoom { closer } => {
            atlas.zoom = if closer {
                atlas.zoom.closer()
            } else if atlas.zoom == Zoom::Fit && world {
                // past the whole city: the world, on the city's tile you're on
                Zoom::World
            } else {
                atlas.zoom.further()
            };
            if atlas.zoom == Zoom::World {
                atlas.world.cursor = None;
            }
            atlas.recentre = true;
        }
        Act::Fold => atlas.folded = !atlas.folded,
        _ => {}
    }
    // The rest work on the map as it would draw now; before its first draw there is none.
    let Some(last) = st.atlas.layout else {
        return true;
    };
    refresh(st);
    let now = relayout(st, &last);
    st.atlas.settle(&now);
    match act {
        Act::Move(dx, dy) => step(st, &now, dx, dy),
        Act::List(d) => step_list(st, &now, d),
        Act::Track => track_here(st, &now),
        _ => {}
    }
    refresh(st);
    let after = relayout(st, &last);
    st.atlas.settle(&after);
    st.atlas.scroll = pane::list(st, &after, &cached(st).listings).first;
    true
}

/// Moves the cursor `dx` cells across and `dy` down, a cell `s` tiles wide and `2s`
/// tall, a tile at least closer in; it stops at the stratum's edge.
fn step(st: &mut AppState, l: &MapLayout, dx: i32, dy: i32) {
    let size = l
        .stratum
        .and_then(|k| st.city.as_ref()?.stratum(k))
        .map(|s| s.tiles.size());
    let (Some(c), Some(size)) = (l.cursor, size) else {
        return;
    };
    let (w, h) = (size.w as i32, size.h as i32);
    let m = l.m.max(1);
    let (across, down) = ((l.s / m).max(1), (2 * l.s / m).max(1));
    let x = (c.x + dx * across).clamp(0, w - 1);
    let y = (c.y + dy * down).clamp(0, h - 1);
    st.atlas.cursor = Some(Pos::new(x, y));
}

/// `[` `]`: the cursor to the previous or next listing in the list, or, with none
/// selected, to the one nearest it.
fn step_list(st: &mut AppState, l: &MapLayout, d: i32) {
    let Some(c) = l.cursor else {
        return;
    };
    let picked = {
        let cache = cached(st);
        let all = &cache.listings;
        let v = pane::list(st, l, all);
        let n = v.rows.len() as i32;
        let next = match v.sel {
            Some(at) => Some((at as i32 + d).rem_euclid(n.max(1)) as usize),
            None => (0..v.rows.len()).min_by_key(|r| (all[v.rows[*r]].tile.dist2(c), *r)),
        };
        next.map(|r| {
            let x = &all[v.rows[r]];
            (x.id, x.kind, x.tile)
        })
    };
    if let Some(p @ (_, _, tile)) = picked {
        st.atlas.cursor = Some(tile);
        st.atlas.picked = Some(p);
    }
}

/// `t`: tracks the listing under the cursor (a `◆` as its job or story), else the spot
/// under it; on the tracked one, stops.
fn track_here(st: &mut AppState, l: &MapLayout) {
    let listing = under_cursor(st, l).map(|x| source_of(st, &x));
    let result = match listing.or_else(|| spot_source(st, l)) {
        Some(source) => track::toggle(st, source),
        None => Err("nothing to walk to there".to_string()),
    };
    if let Err(e) = result {
        st.warn(e);
    }
    track::tick(st);
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        BODY_120, ON_STREET, STREET, clinic, frame, laid, open_ground, outside, press, text, you,
    };
    use super::super::{Zoom, listed, map, spot_under_cursor, under_cursor};
    use super::*;
    use crate::render::map::MapRef;
    use crate::track::listings::{Kind, Listing};
    use crate::track::ui::story_source;
    use crate::track::{ListingId, Source};
    use citygen::Pos;

    fn cursor(st: &AppState) -> Pos {
        st.atlas.cursor.expect("a cursor")
    }

    #[test]
    fn arrows_move_and_shift_moves_eight() {
        let mut st = outside();
        frame(&mut st, BODY_120);
        let at = cursor(&st);
        assert_eq!(at, you(&st));
        let s = laid(&st).s;
        assert_eq!(s, 2);
        // A cell is s tiles across and 2s down.
        for (code, to) in [
            (KeyCode::Char('l'), at.offset(s, 0)),
            (KeyCode::Right, at.offset(2 * s, 0)),
            (KeyCode::Char('h'), at.offset(s, 0)),
            (KeyCode::Left, at),
            (KeyCode::Char('j'), at.offset(0, 2 * s)),
            (KeyCode::Down, at.offset(0, 4 * s)),
            (KeyCode::Char('k'), at.offset(0, 2 * s)),
            (KeyCode::Up, at),
            (KeyCode::Char('L'), at.offset(8 * s, 0)),
            (KeyCode::Char('J'), at.offset(8 * s, 16 * s)),
            (KeyCode::Char('H'), at.offset(0, 16 * s)),
            (KeyCode::Char('K'), at),
        ] {
            assert!(key(&mut st, code), "{code:?}");
            frame(&mut st, BODY_120);
            assert_eq!(cursor(&st), to, "{code:?}");
            assert_eq!(laid(&st).cursor, Some(to), "drawn there");
        }
        // It stops at the stratum's edge.
        let size = st.city.as_ref().unwrap().size;
        let (w, h) = (size.w as i32, size.h as i32);
        st.atlas.cursor = Some(Pos::new(w - 3, 5));
        frame(&mut st, BODY_120);
        press(&mut st, 'L');
        press(&mut st, 'K');
        assert_eq!(cursor(&st), Pos::new(w - 1, 0));
        press(&mut st, 'l');
        press(&mut st, 'k');
        assert_eq!(cursor(&st), Pos::new(w - 1, 0));
        st.atlas.cursor = Some(Pos::new(2, h - 2));
        frame(&mut st, BODY_120);
        press(&mut st, 'H');
        press(&mut st, 'J');
        assert_eq!(cursor(&st), Pos::new(0, h - 1));
        let l = laid(&st);
        assert!(l.cell_of(Pos::new(0, h - 1)).is_some(), "still in view");
    }

    /// The listings the pane shows, in its order.
    fn rows(st: &AppState) -> Vec<Listing> {
        listed(st)
            .iter()
            .filter(|l| map::shows(st, l))
            .cloned()
            .collect()
    }

    /// Where the selected listing stands in the list.
    fn selected(st: &AppState) -> Option<usize> {
        let x = under_cursor(st, &laid(st))?;
        rows(st)
            .iter()
            .position(|l| l.id == x.id && l.kind == x.kind)
    }

    #[test]
    fn brackets_step_the_list_and_jump() {
        let mut st = outside();
        frame(&mut st, BODY_120);
        let ground = open_ground(&mut st, BODY_120);
        assert_eq!(selected(&st), None);
        let all = rows(&st);
        let near = all
            .iter()
            .min_by_key(|l| l.tile.dist2(ground))
            .unwrap()
            .clone();
        press(&mut st, ']');
        frame(&mut st, BODY_120);
        assert_eq!(cursor(&st), near.tile, "the nearest to the cursor");
        let l = laid(&st);
        assert_eq!(under_cursor(&st, &l).map(|x| x.id), Some(near.id));
        assert!(l.cell_of(near.tile).is_some(), "the view pans to it");
        // Then the list's order, either way.
        let at = selected(&st).unwrap();
        press(&mut st, ']');
        frame(&mut st, BODY_120);
        let next = &all[(at + 1) % all.len()];
        assert_eq!(
            (selected(&st), cursor(&st)),
            (Some((at + 1) % all.len()), next.tile)
        );
        press(&mut st, '[');
        press(&mut st, '[');
        frame(&mut st, BODY_120);
        let back = (at + all.len() - 1) % all.len();
        assert_eq!(selected(&st), Some(back));
        // From the first, `[` wraps to the last, and the list scrolls to show it.
        for _ in 0..all.len() {
            if selected(&st) == Some(0) {
                break;
            }
            press(&mut st, '[');
        }
        assert_eq!(selected(&st), Some(0));
        assert_eq!(st.atlas.scroll, 0);
        press(&mut st, '[');
        let buf = frame(&mut st, BODY_120);
        assert_eq!(selected(&st), Some(all.len() - 1));
        assert!(st.atlas.scroll > 0, "scrolled");
        let b = BODY_120;
        let pane: Vec<String> = (b.y..b.bottom())
            .map(|y| text(&buf, y, b.right() - 35..b.right()))
            .collect();
        let last: String = all[all.len() - 1].name.chars().take(23).collect();
        assert!(pane.iter().any(|r| r.contains(&last)), "{pane:#?}");
        assert!(pane.iter().any(|r| r.contains("▲") && r.contains("more")));
    }

    #[test]
    fn angles_step_the_kind_and_pinned_stay() {
        let mut st = outside();
        frame(&mut st, BODY_120);
        assert_eq!(st.atlas.kind, None, "all");
        assert_eq!(Kind::ALL.len(), 15);
        for k in Kind::ALL {
            press(&mut st, '>');
            assert_eq!(st.atlas.kind, Some(k));
        }
        assert_eq!(st.atlas.kind, Some(Kind::Blocks));
        press(&mut st, '>');
        assert_eq!(st.atlas.kind, None, "round to all");
        press(&mut st, '<');
        assert_eq!(st.atlas.kind, Some(Kind::Blocks));
        for k in Kind::ALL.iter().rev().skip(1) {
            press(&mut st, '<');
            assert_eq!(st.atlas.kind, Some(*k));
        }
        press(&mut st, '<');
        assert_eq!(st.atlas.kind, None);
        // Ripperdocs: the clinics, and the pinned rows with them.
        st.atlas.kind = Some(Kind::Docs);
        let buf = frame(&mut st, BODY_120);
        let shown = rows(&st);
        let want = listed(&st)
            .iter()
            .filter(|l| matches!(l.kind, Kind::Docs | Kind::Jobs | Kind::Home))
            .count();
        assert_eq!(shown.len(), want);
        assert!(shown.iter().any(|l| l.kind == Kind::Docs));
        assert!(shown.iter().any(|l| l.kind == Kind::Home));
        assert!(shown.iter().any(|l| l.kind == Kind::Jobs));
        let b = BODY_120;
        let pane: String = (b.y..b.bottom())
            .map(|y| text(&buf, y, b.right() - 35..b.right()) + "\n")
            .collect();
        assert!(pane.contains("PLACES ‹ ripperdocs ›"), "{pane}");
        for l in &shown {
            let name: String = l.name.chars().take(23).collect();
            assert!(pane.contains(&name), "{name} in\n{pane}");
        }
    }

    #[test]
    fn t_tracks_the_listing_or_the_spot() {
        let mut st = outside();
        let doc = clinic(&st);
        st.atlas.stratum = Some(STREET);
        st.atlas.cursor = Some(doc.tile);
        frame(&mut st, BODY_120);
        press(&mut st, 't');
        let tracked = |st: &AppState| st.track.as_ref().map(|t| t.source);
        let b = doc.building.unwrap();
        assert_eq!(tracked(&st), Some(Source::Listing(ListingId::Building(b))));
        frame(&mut st, BODY_120);
        press(&mut st, 'T');
        assert_eq!(tracked(&st), None, "t on the tracked one stops it");

        // Open ground: a spot of the tiles the cursor's cell covers.
        let ground = open_ground(&mut st, BODY_120);
        let spot = spot_under_cursor(&st, &laid(&st)).unwrap();
        assert!(spot.chebyshev(ground) <= 8);
        press(&mut st, ' ');
        let spot_source = Source::Spot {
            place: ON_STREET,
            pos: spot,
            radius: 2,
        };
        assert_eq!(tracked(&st), Some(spot_source));

        // Nothing walkable within 8 tiles: refused, the old track kept.
        st.atlas.zoom = Zoom::One;
        let solid = solid_ground(&st);
        st.atlas.cursor = Some(solid);
        frame(&mut st, BODY_120);
        assert_eq!(laid(&st).s, 1);
        assert_eq!(spot_under_cursor(&st, &laid(&st)), None);
        assert_eq!(under_cursor(&st, &laid(&st)), None);
        press(&mut st, 't');
        let last = st.log.latest().unwrap();
        assert_eq!(last.text, "nothing to walk to there");
        assert_eq!(last.kind, crate::messages::Kind::Warn);
        assert_eq!(tracked(&st), Some(spot_source));
    }

    /// A street tile with nothing walkable within 9 tiles of it.
    fn solid_ground(st: &AppState) -> Pos {
        let city = st.city.as_ref().unwrap();
        let ok = crate::track::route::open(MapRef::stratum_of(city, STREET), true, &[], &[]);
        let size = city.size;
        let open: Vec<bool> = size.rect().iter().map(&ok).collect();
        size.rect()
            .iter()
            .find(|p| {
                (-9..=9).all(|dy| {
                    (-9..=9).all(|dx| {
                        let q = p.offset(dx, dy);
                        !size.contains(q) || !open[size.index(q)]
                    })
                })
            })
            .expect("solid ground somewhere")
    }

    #[test]
    fn t_on_a_diamond_is_the_jobs_tab_t() {
        let mut st = outside();
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::end(&mut st, 0, 0);
        let story = listed(&st)
            .iter()
            .find(|l| l.id == ListingId::Story(0))
            .cloned()
            .expect("the story's ◆");
        st.atlas.stratum = Some(STREET);
        st.atlas.cursor = Some(story.tile);
        frame(&mut st, BODY_120);
        assert_eq!(
            under_cursor(&st, &laid(&st)).map(|l| l.id),
            Some(ListingId::Story(0))
        );
        press(&mut st, 't');
        let step = st.missions.runs[0].at;
        let source = st.track.as_ref().map(|t| t.source);
        assert_eq!(
            source,
            Some(Source::Story(crate::runs::ids::StepRef::new(0, step)))
        );
        assert_eq!(source, story_source(&st, 0), "as the JOBS tab's t");
    }

    #[test]
    fn zoom_steps_and_clamps() {
        let mut st = outside();
        let title = |buf: &ratatui::buffer::Buffer| text(buf, BODY_120.y, 2..80);
        let buf = frame(&mut st, BODY_120);
        assert_eq!(st.atlas.zoom, Zoom::Two);
        assert!(title(&buf).contains("zoom 2"));
        let at = cursor(&st);
        for (c, z) in [
            ('-', Zoom::Fit),
            ('-', Zoom::Fit),
            ('+', Zoom::Two),
            ('+', Zoom::One),
            ('+', Zoom::One),
            ('-', Zoom::Two),
            ('-', Zoom::Fit),
            ('=', Zoom::Two),
            ('=', Zoom::One),
        ] {
            press(&mut st, c);
            assert_eq!(st.atlas.zoom, z, "{c}");
            let buf = frame(&mut st, BODY_120);
            let n = z.number();
            assert!(
                title(&buf).contains(&format!("zoom {n}")),
                "{}",
                title(&buf)
            );
            assert_eq!(cursor(&st), at, "a zoom keeps the cursor");
        }
        // At fit the whole street shows.
        st.atlas.zoom = Zoom::Fit;
        frame(&mut st, BODY_120);
        let size = st.city.as_ref().unwrap().size;
        let l = laid(&st);
        assert_eq!(l.s, 3);
        assert!(l.cell_of(Pos::new(0, 0)).is_some());
        assert!(
            l.cell_of(Pos::new(size.w as i32 - 1, size.h as i32 - 1))
                .is_some()
        );
        // Zoomed in, the view centres on the cursor.
        press(&mut st, '+');
        frame(&mut st, BODY_120);
        let l = laid(&st);
        let (x, y) = l.cell_of(at).unwrap();
        let mid = (l.area.x + l.area.width / 2, l.area.y + l.area.height / 2);
        let (cols, rows) = (l.area.width as i32, l.area.height as i32);
        let edge_x = l.origin.x == 0 || l.origin.x + cols * l.s >= size.w as i32;
        let edge_y = l.origin.y == 0 || l.origin.y + rows * 2 * l.s >= size.h as i32;
        assert!(x.abs_diff(mid.0) <= 1 || edge_x, "{x} {mid:?}");
        assert!(y.abs_diff(mid.1) <= 1 || edge_y, "{y} {mid:?}");
    }

    /// Seed 3's run walked into the world's first other city (not a town), on its
    /// street nearest its block's middle: a city much smaller than the home city.
    fn in_another_city() -> AppState {
        let mut st = crate::outside::tests::home_run();
        let o = st.outside.as_ref().unwrap();
        let (s, b) = o
            .others()
            .into_iter()
            .find(|(s, _)| s.town.is_none())
            .unwrap();
        crate::outside::enter(&mut st, s).unwrap();
        let o = st.outside.as_ref().unwrap();
        let middle = Pos::new(b.x + b.w / 2, b.y + b.h / 2);
        let cell = o.street_of(st.city.as_ref().unwrap(), middle);
        crate::outside::moves::into_city(&mut st, cell);
        st
    }

    #[test]
    fn every_zoom_step_is_closer_in_every_city() {
        // Zoom 3 is the whole city, 2 closer and 1 closer still, in the home city and
        // in a smaller one, on a small deck and a wide one: a city that fits at a tile a
        // pixel spreads a tile over more pixels closer in.
        const BODY_200: ratatui::layout::Rect = ratatui::layout::Rect::new(1, 10, 198, 47);
        let cities = [
            ("another", in_another_city()),
            ("home", crate::outside::tests::home_run()),
        ];
        for (name, mut st) in cities {
            for body in [BODY_120, BODY_200] {
                let mid = st.city.as_ref().unwrap().size.center();
                st.atlas.zoom = Zoom::Fit;
                st.atlas.stratum = Some(STREET);
                st.atlas.cursor = Some(mid);
                st.atlas.recentre = true;
                let mut was = 0;
                let mut titles = Vec::new();
                for (z, n) in [(Zoom::Fit, 3), (Zoom::Two, 2), (Zoom::One, 1)] {
                    if z != Zoom::Fit {
                        press(&mut st, '+');
                    }
                    let buf = frame(&mut st, body);
                    assert_eq!(st.atlas.zoom, z);
                    let l = laid(&st);
                    let at = l.cell_of(mid).expect("the cursor in view");
                    assert!(l.tiles_of(at).contains(mid), "{name} zoom {n}");
                    let east = l.cell_of(mid.offset(8, 0)).expect("8 tiles on in view");
                    let across = (east.0 - at.0) as i32;
                    assert!(
                        across > was,
                        "{name} at {body:?}, zoom {n}: 8 tiles over {across} cells, {was} before"
                    );
                    was = across;
                    titles.push((n, text(&buf, body.y, 2..body.right())));
                }
                // the title names the step, not the tiles a pixel covers
                for (n, title) in titles {
                    assert!(title.contains(&format!(" · zoom {n}")), "{name}: {title}");
                }
                // closest in, a step is the tiles a cell covers, a tile at least
                for (code, east) in [('l', true), ('j', false)] {
                    let from = cursor(&st);
                    let l = laid(&st);
                    let r = l.tiles_of(l.cell_of(from).unwrap());
                    press(&mut st, code);
                    frame(&mut st, body);
                    let to = if east {
                        from.offset(r.w, 0)
                    } else {
                        from.offset(0, r.h)
                    };
                    assert_eq!(cursor(&st), to, "{name}: {code}");
                }
            }
        }
    }

    #[test]
    fn the_deck_keeps_its_own_keys() {
        let mut st = outside();
        frame(&mut st, BODY_120);
        for code in [
            KeyCode::Char('q'),
            KeyCode::Char('e'),
            KeyCode::Char('Q'),
            KeyCode::Char('1'),
            KeyCode::Char('0'),
            KeyCode::Tab,
            KeyCode::Esc,
            KeyCode::Enter,
        ] {
            assert!(!key(&mut st, code), "{code:?} is the deck's");
        }
    }
}
