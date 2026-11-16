//! CHROME on GEAR & INVENTORY: the body's chrome as a body of its own, a tile per site
//! and one per socket of the piece installed there, placed by a table and joined the
//! way the sites join.

// GEAR draws CHROME from these; until then only the tests and the ledger read them.
#![cfg_attr(not(test), allow(dead_code))]

use super::body::{BodyLayout, FULL_W, Place, at, place};
use crate::app::AppState;
use game::{Site, SocketKind};
use ratatui::layout::Rect;

/// Where the cursor stands on CHROME: a site, or a socket of the piece a site holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Site(Site),
    Socket(Site, usize),
}

impl Default for Stop {
    fn default() -> Stop {
        Stop::Site(Site::Neural)
    }
}

/// CHROME's stops: each site in `Site::ALL` order, each followed by the sockets of the
/// piece installed there.
pub fn stops(st: &AppState) -> Vec<Stop> {
    let mut out = Vec::new();
    for site in Site::ALL {
        out.push(Stop::Site(site));
        let piece = st
            .character
            .chrome_at(site)
            .and_then(|id| st.content.item(id));
        let n = piece.map_or(0, |i| i.sockets.len());
        out.extend((0..n).map(|k| Stop::Socket(site, k)));
    }
    out
}

/// The kind of a socket of what a site holds.
pub(crate) fn socket_kind(st: &AppState, site: Site, n: usize) -> Option<SocketKind> {
    let item = st
        .character
        .chrome_at(site)
        .and_then(|id| st.content.item(id))?;
    item.sockets.get(n).copied()
}

/// A tile's cell in the graph: its column and row.
type Cell = (u16, u16);

/// Each site's cell and the site it joins; the trunk runs down the middle column from
/// NEURAL to LEGS and branches under the SPINE to BACK and SKIN.
const SITES: [(Site, Cell, Option<Site>); 11] = {
    use Site::*;
    [
        (Neural, (2, 0), None),
        (Eyes, (1, 0), Some(Neural)),
        (Ears, (3, 0), Some(Neural)),
        (Jaw, (4, 0), Some(Ears)),
        (Spine, (2, 2), Some(Organs)),
        (Back, (1, 3), Some(Spine)),
        (Organs, (2, 1), Some(Neural)),
        (Skin, (3, 3), Some(Spine)),
        (LeftArm, (1, 1), Some(Organs)),
        (RightArm, (3, 1), Some(Organs)),
        (Legs, (2, 3), Some(Spine)),
    ]
};

/// A socket's cell by its host and its kind: an arm's hand on its outer side, its
/// forearm under it, a cradle's bay left of the back.
const SOCKETS: [(Site, SocketKind, Cell); 5] = {
    use Site::*;
    use SocketKind::*;
    [
        (LeftArm, Hand, (0, 1)),
        (RightArm, Hand, (4, 1)),
        (LeftArm, Forearm, (1, 2)),
        (RightArm, Forearm, (3, 2)),
        (Back, Bay, (0, 3)),
    ]
};

/// Cells no tile takes, in the order a socket the table has no cell for takes them: a
/// second socket of one kind on a piece, or a kind on a host the table doesn't name.
const FREE: [Cell; 4] = [(4, 2), (4, 3), (0, 2), (0, 0)];

/// The full body's columns (first column, width) and rows (first row, height): tiles
/// 11×5, as MEAT's.
const FULL_COLS: [(u16, u16); 5] = [(0, 11), (12, 11), (24, 11), (36, 11), (48, 11)];
const FULL_ROWS: [(u16, u16); 4] = [(0, 5), (6, 5), (12, 5), (18, 5)];
/// The compact body's, in the left half of a 100-column tab (48 columns, 21 rows under
/// the header): the same shape, tiles 4 tall for a name on one line, the trunk's column
/// widest and the outer ones, which hold the hands, the bay and the jaw, narrowest.
const COMPACT_COLS: [(u16, u16); 5] = [(0, 8), (9, 9), (19, 10), (30, 9), (40, 8)];
const COMPACT_ROWS: [(u16, u16); 4] = [(0, 4), (5, 4), (10, 4), (15, 4)];

/// The cell of each stop of `stops`, and the stop it joins.
fn cells(st: &AppState, stops: &[Stop]) -> Vec<(Stop, Cell, Option<Stop>)> {
    let mut taken = Vec::new();
    let mut out = Vec::new();
    for &stop in stops {
        let (cell, joins) = match stop {
            Stop::Site(site) => {
                let Some(&(_, cell, joins)) = SITES.iter().find(|s| s.0 == site) else {
                    continue;
                };
                (Some(cell), joins.map(Stop::Site))
            }
            Stop::Socket(site, n) => {
                let kind = socket_kind(st, site, n);
                let own = SOCKETS
                    .iter()
                    .find(|s| s.0 == site && Some(s.1) == kind)
                    .map(|s| s.2)
                    .filter(|c| !taken.contains(c));
                let cell = own.or_else(|| FREE.into_iter().find(|c| !taken.contains(c)));
                (cell, Some(Stop::Site(site)))
            }
        };
        if let Some(cell) = cell {
            taken.push(cell);
            out.push((stop, cell, joins));
        }
    }
    out
}

/// The places of `cells` on the grid of `cols` and `rows`; a line meets a tile at its
/// middle column.
fn places(
    cells: &[(Stop, Cell, Option<Stop>)],
    cols: &[(u16, u16)],
    rows: &[(u16, u16)],
) -> Vec<Place<Stop>> {
    cells
        .iter()
        .map(|&(stop, (c, r), joins)| {
            let ((x, w), (y, h)) = (cols[usize::from(c)], rows[usize::from(r)]);
            let p = at(stop, (x, y), (w, h), w / 2);
            match joins {
                Some(to) => p.to(to),
                None => p,
            }
        })
        .collect()
}

/// CHROME's side of the tab `area`: the full body when the tab has room for it, else
/// the compact one in the left half, with a tile for every stop.
pub(crate) fn chrome_layout(area: Rect, st: &AppState) -> BodyLayout<Stop> {
    let stops = stops(st);
    let cells = cells(st, &stops);
    let full = places(&cells, &FULL_COLS, &FULL_ROWS);
    let compact = places(&cells, &COMPACT_COLS, &COMPACT_ROWS);
    place(area, &full, &compact, &stops, FULL_W)
}

#[cfg(test)]
mod tests {
    use super::super::body::{Dir, joints, step};
    use super::*;
    use game::{ItemId, Stack};

    /// The tab's body for a window of `w` by `h`, as the deck sets it.
    fn tab(w: u16, h: u16) -> Rect {
        let top = if h >= 36 { 10 } else { 5 };
        Rect::new(1, top, w - 2, h - top - 3)
    }

    /// Installs `item` at `site`, as a copy when it is held per copy, with `fitted` in
    /// its sockets.
    fn install(st: &mut AppState, site: Site, item: &str, fitted: &[(usize, &str)]) {
        let id = ItemId::new(item);
        let copy = st.content.item(&id).unwrap().per_copy().then(|| {
            let mut k = st.character.new_copy(&st.content, &id);
            for &(n, piece) in fitted {
                let piece = ItemId::new(piece);
                let pc = st.character.new_copy(&st.content, &piece);
                k.fitted[n] = Some(Stack {
                    item: piece,
                    qty: 1,
                    copy: Some(pc),
                });
            }
            k
        });
        st.character.install(site, id, copy);
    }

    /// The attributes shot's loadout: a piece at every site but EARS, R ARM and LEGS,
    /// the Scout drone in the cradle's bay, the Piston arm at L ARM with the Arc hand.
    fn loadout() -> AppState {
        let mut st = AppState::new();
        st.character = game::Character::debug_runner(&st.content);
        for (site, item) in [
            (Site::Neural, "neural_port"),
            (Site::Eyes, "target_lens"),
            (Site::Jaw, "bulldog_jaw"),
            (Site::Spine, "redline_spine"),
            (Site::Organs, "adrenal_pump"),
            (Site::Skin, "subdermal_mesh"),
        ] {
            install(&mut st, site, item, &[]);
        }
        install(&mut st, Site::Back, "drone_cradle", &[(0, "scout_drone")]);
        install(&mut st, Site::LeftArm, "piston_arm", &[(0, "arc_hand")]);
        st
    }

    /// Every socket there is: both chrome arms with a hand and a forearm piece each, a
    /// cradle with a drone docked.
    fn fullest() -> AppState {
        let mut st = loadout();
        install(
            &mut st,
            Site::LeftArm,
            "piston_arm",
            &[(0, "arc_hand"), (1, "monowire_spool")],
        );
        install(
            &mut st,
            Site::RightArm,
            "grip_arm",
            &[(0, "toolfingers"), (1, "launcher_sleeve")],
        );
        st
    }

    #[test]
    fn the_full_chrome_body_sits_where_the_mock_puts_it() {
        let st = loadout();
        let l = chrome_layout(tab(120, 40), &st);
        assert!(l.full);
        assert_eq!(l.left, Rect::new(1, 10, 62, 27), "MEAT's columns");
        // The graph, 59×23, centred under the header and its rule.
        let (ox, oy) = (2, 13);
        let table = [
            (Site::Eyes, (12, 0)),
            (Site::Neural, (24, 0)),
            (Site::Ears, (36, 0)),
            (Site::Jaw, (48, 0)),
            (Site::LeftArm, (12, 6)),
            (Site::Organs, (24, 6)),
            (Site::RightArm, (36, 6)),
            (Site::Spine, (24, 12)),
            (Site::Back, (12, 18)),
            (Site::Legs, (24, 18)),
            (Site::Skin, (36, 18)),
        ];
        for (site, (x, y)) in table {
            assert_eq!(
                l.tile(Stop::Site(site)),
                Some(Rect::new(ox + x, oy + y, 11, 5)),
                "{site:?}"
            );
        }
        let sockets = [
            (Stop::Socket(Site::LeftArm, 0), (0, 6)),
            (Stop::Socket(Site::LeftArm, 1), (12, 12)),
            (Stop::Socket(Site::Back, 0), (0, 18)),
        ];
        for (stop, (x, y)) in sockets {
            assert_eq!(
                l.tile(stop),
                Some(Rect::new(ox + x, oy + y, 11, 5)),
                "{stop:?}"
            );
        }
        assert_eq!(l.tiles.len(), 14, "11 sites and 3 sockets");
    }

    #[test]
    fn a_socket_has_a_tile_only_while_its_piece_is_in() {
        let sockets = |st: &AppState| -> Vec<Stop> {
            stops(st)
                .into_iter()
                .filter(|s| matches!(s, Stop::Socket(..)))
                .collect()
        };
        let tiled = |st: &AppState| -> Vec<Stop> {
            chrome_layout(tab(120, 40), st)
                .tiles
                .iter()
                .map(|t| t.0)
                .filter(|s| matches!(s, Stop::Socket(..)))
                .collect()
        };
        // A runner fresh from the template: meat arms, no cradle.
        let mut st = AppState::new();
        assert!(sockets(&st).is_empty());
        assert!(tiled(&st).is_empty());
        let sites: Vec<Stop> = Site::ALL.into_iter().map(Stop::Site).collect();
        assert_eq!(stops(&st), sites);
        assert_eq!(chrome_layout(tab(120, 40), &st).tiles.len(), 11);

        install(&mut st, Site::LeftArm, "piston_arm", &[]);
        let arm = vec![
            Stop::Socket(Site::LeftArm, 0),
            Stop::Socket(Site::LeftArm, 1),
        ];
        assert_eq!(sockets(&st), arm);
        assert_eq!(tiled(&st), arm);
        let all = stops(&st);
        let at = all.iter().position(|s| *s == Stop::Site(Site::LeftArm));
        assert_eq!(
            all[at.unwrap() + 1..at.unwrap() + 3],
            arm[..],
            "under their site"
        );

        install(&mut st, Site::Back, "drone_cradle", &[]);
        assert!(sockets(&st).contains(&Stop::Socket(Site::Back, 0)));
        assert!(tiled(&st).contains(&Stop::Socket(Site::Back, 0)));
        assert_eq!(sockets(&st).len(), 3);
        assert_eq!(
            socket_kind(&st, Site::Back, 0),
            Some(SocketKind::Bay),
            "the cradle's"
        );
        assert_eq!(
            socket_kind(&st, Site::LeftArm, 1),
            Some(SocketKind::Forearm)
        );
        assert_eq!(socket_kind(&st, Site::RightArm, 0), None, "a meat arm");
    }

    #[test]
    fn the_trunk_branches_under_the_spine() {
        let st = loadout();
        let l = chrome_layout(tab(120, 40), &st);
        let glyphs = joints(&l);
        let at = |x: u16, y: u16| glyphs.iter().find(|g| g.0 == (x, y)).map(|g| g.1);
        let spine = l.tile(Stop::Site(Site::Spine)).unwrap();
        let legs = l.tile(Stop::Site(Site::Legs)).unwrap();
        let (back, skin) = (
            l.tile(Stop::Site(Site::Back)).unwrap(),
            l.tile(Stop::Site(Site::Skin)).unwrap(),
        );
        let trunk = spine.x + 5;
        let row = spine.bottom();
        assert_eq!(at(trunk, spine.bottom() - 1), Some('┬'));
        assert_eq!(at(trunk, row), Some('┼'));
        assert_eq!(at(trunk, legs.y), Some('┴'));
        assert_eq!(at(back.x + 5, row), Some('┌'));
        assert_eq!(at(skin.x + 5, row), Some('┐'));
        assert_eq!(at(back.x + 5, back.y), Some('┴'));
        assert_eq!(at(skin.x + 5, skin.y), Some('┴'));
        // The trunk runs straight down from NEURAL through ORGANS to the SPINE.
        let neural = l.tile(Stop::Site(Site::Neural)).unwrap();
        assert_eq!(neural.x + 5, trunk);
        assert_eq!(at(trunk, neural.bottom() - 1), Some('┬'));
        assert_eq!(at(trunk, neural.bottom()), Some('│'));
    }

    #[test]
    fn a_second_socket_of_a_kind_takes_a_free_place() {
        let mut st = fullest();
        let arm = st
            .content
            .items
            .iter_mut()
            .find(|i| i.id == ItemId::new("grip_arm"))
            .unwrap();
        arm.sockets.push(SocketKind::Hand);
        install(&mut st, Site::RightArm, "grip_arm", &[]);
        let second = Stop::Socket(Site::RightArm, 2);
        assert!(stops(&st).contains(&second));
        let l = chrome_layout(tab(120, 40), &st);
        let neural = l.tile(Stop::Site(Site::Neural)).unwrap();
        let (ox, oy) = (neural.x - 24, neural.y);
        assert_eq!(l.tile(second), Some(Rect::new(ox + 48, oy + 12, 11, 5)));
        assert_eq!(
            l.tile(Stop::Socket(Site::RightArm, 0)),
            Some(Rect::new(ox + 48, oy + 6, 11, 5)),
            "the first keeps the hand's place"
        );
        let l = chrome_layout(tab(100, 30), &st);
        assert!(l.tile(second).is_some(), "compact too");
    }

    #[test]
    fn every_stop_shows_and_walks_at_both_sizes() {
        let st = fullest();
        let all = stops(&st);
        assert_eq!(all.len(), 16, "11 sites, 2 sockets an arm, the bay");
        for (w, h) in [(100, 30), (120, 40)] {
            let area = tab(w, h);
            let l = chrome_layout(area, &st);
            assert_eq!(l.full, w == 120, "{w}x{h}");
            for stop in &all {
                let r = l.tile(*stop);
                assert!(r.is_some(), "{stop:?} has no tile at {w}x{h}");
                let r = r.unwrap();
                assert_eq!(
                    area.intersection(r),
                    r,
                    "{stop:?} {r:?} outside the tab at {w}x{h}"
                );
                assert_eq!(l.left.intersection(r), r, "{stop:?} past the divider");
                if !l.full {
                    assert_eq!(r.height, 4, "{stop:?} at {w}x{h}");
                }
            }
            for (i, (a, ra)) in l.tiles.iter().enumerate() {
                for (b, rb) in &l.tiles[i + 1..] {
                    assert!(!ra.intersects(*rb), "{a:?} on {b:?} at {w}x{h}");
                }
            }
            // Every line runs in the gaps: none crosses a tile but at its ends.
            for line in &l.lines {
                for pair in line.path.windows(2) {
                    let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
                    let cells: Vec<(u16, u16)> = if y0 == y1 {
                        (x0.min(x1) + 1..x0.max(x1)).map(|x| (x, y0)).collect()
                    } else {
                        (y0.min(y1) + 1..y0.max(y1)).map(|y| (x0, y)).collect()
                    };
                    for (x, y) in cells {
                        for (s, r) in &l.tiles {
                            assert!(
                                !r.contains((x, y).into()),
                                "{line:?} through {s:?} at {w}x{h}"
                            );
                        }
                    }
                }
            }
            assert_eq!(l.lines.len(), 15, "every stop but NEURAL joins one");
            // From NEURAL the arrows reach every stop.
            let mut reached = vec![Stop::Site(Site::Neural)];
            let mut i = 0;
            while let Some(&from) = reached.get(i) {
                for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
                    if let Some(to) = step(&l, from, dir)
                        && !reached.contains(&to)
                    {
                        reached.push(to);
                    }
                }
                i += 1;
            }
            for stop in &all {
                assert!(reached.contains(stop), "{stop:?} unreached at {w}x{h}");
            }
        }
    }
}
