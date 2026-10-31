//! The places pane beside the map, the strip that marks it folded, and the card beside
//! the listing under the cursor (spec 7, `map2.html`).

use super::map::{self, MapLayout};
use super::{Cache, pane_width, spot_under_cursor, under};
use crate::app::AppState;
use crate::render::Visibility;
use crate::track::is_tracked;
use crate::track::listings::{Kind, Listing, source_of};
use crate::track::overlay::mix;
use crate::ui::chrome::{CYAN, DIM, FAINTER, PANEL_BG, TEXT};
use citygen::{Place, StratumKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The cells the folded pane leaves: its strip of three, then the deck's margin.
pub(super) const STRIP: u16 = 4;
/// The pane's border, the strip's edge.
pub(super) const BORDER: Color = Color::Rgb(26, 34, 48);
/// The selected row, and the chosen kind.
pub(super) const SEL: Color = Color::Rgb(20, 32, 44);
/// The folded strip's ground.
pub(super) const STRIP_BG: Color = Color::Rgb(10, 16, 24);
const CARD_W: u16 = 30;
const CARD_H: u16 = 6;

/// Writes `text` from `x` on row `y`, short of `right`, over what is there.
pub(super) fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style, right: u16) {
    if !(buf.area.y..buf.area.bottom()).contains(&y) {
        return;
    }
    let right = right.min(buf.area.right());
    for (cx, ch) in (x.max(buf.area.x)..right).zip(text.chars()) {
        buf[(cx, y)].set_char(ch).set_style(style);
    }
}

/// Blanks `r` onto `bg`.
fn fill(buf: &mut Buffer, r: Rect, bg: Color) {
    let r = r.intersection(buf.area);
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_bg(bg);
        }
    }
}

/// The first `n` characters of `text`.
fn cut(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

/// Whether a listing is what is tracked.
fn tracked(st: &AppState, x: &Listing) -> bool {
    is_tracked(st, &source_of(st, x))
}

/// The places list as the pane lays it out.
pub(super) struct List {
    /// The listings shown, by their index, nearest first.
    pub rows: Vec<usize>,
    /// The selected row: the listing under the cursor.
    pub sel: Option<usize>,
    /// The first row in view.
    pub first: usize,
}

/// The listings shown under the chosen kind, the one under the cursor among them, and
/// the first row in view: the one kept, moved to show the selected row whole.
pub(super) fn list(st: &AppState, l: &MapLayout, all: &[Listing]) -> List {
    let rows: Vec<usize> = (0..all.len())
        .filter(|i| map::shows(st, &all[*i]))
        .collect();
    let sel = under(st, l, all).and_then(|i| rows.iter().position(|r| *r == i));
    let (start, last) = lines(l, sel.is_none());
    let room = last.saturating_sub(start) as usize;
    let first = first_row(rows.len(), sel, st.atlas.scroll, room);
    List { rows, sel, first }
}

/// Where the kinds' glyphs go in a pane `w` wide, each as a column from the pane's first
/// cell inside and a row under its header, then `all`'s: a glyph every two cells, onto
/// a second row when the next does not fit.
fn chips(w: u16) -> (Vec<(u16, u16)>, (u16, u16)) {
    let room = w.saturating_sub(2);
    let (mut x, mut y) = (0, 0);
    let mut at = |cells: u16| {
        if x > 0 && x + cells > room {
            (x, y) = (0, y + 1);
        }
        let p = (x, y);
        x += cells + 1;
        p
    };
    let glyphs = Kind::ALL.iter().map(|_| at(1)).collect();
    (glyphs, at(3))
}

/// The rows the kinds take in a pane `w` wide.
fn chip_rows(w: u16) -> u16 {
    chips(w).1.1 + 1
}

/// The list's first line and its `▼` line, below the header, the kinds and the rule;
/// above the cursor's lines when no row is selected (`foot`).
fn lines(l: &MapLayout, foot: bool) -> (u16, u16) {
    let top = l.area.y.saturating_sub(1);
    let first = top + 2 + chip_rows(l.pane);
    let last = l.area.bottom().saturating_sub(if foot { 5 } else { 2 });
    (first, last.max(first))
}

/// The first row in view: `kept`, moved just enough that the selected row shows whole
/// (it opens to three lines) in `room` lines, one of them `▲` once scrolled.
fn first_row(n: usize, sel: Option<usize>, kept: usize, room: usize) -> usize {
    let most = if n <= room { 0 } else { n + 1 - room.max(1) };
    let mut first = kept.min(most);
    if let Some(s) = sel {
        first = first.min(s);
        while first < s && s - first + 3 + usize::from(first > 0) > room {
            first += 1;
        }
    }
    first
}

/// The places pane on the body's right (spec 7, frames 1 and 2): `PLACES ‹ kind ›`, the
/// kinds, the list nearest first with the row under the cursor opened, and, when the
/// cursor is on open ground, what is there.
pub(super) fn pane(st: &AppState, l: &MapLayout, body: Rect, cache: &Cache, buf: &mut Buffer) {
    let all = &cache.listings;
    let w = pane_width(body);
    let edge = body.right().saturating_sub(w);
    let (px, right, top, bottom) = (edge + 1, body.right(), body.y, body.bottom());
    for y in top..bottom {
        put(buf, edge, y, "│", fg(BORDER), right);
    }
    let bold = |c: Color| fg(c).add_modifier(Modifier::BOLD);
    put(buf, px + 1, top, "PLACES", bold(CYAN), right);
    let label = format!("‹ {} ›", st.atlas.kind.map_or("all", Kind::label));
    put(buf, px + 8, top, &label, fg(TEXT), right);
    let keys = right.saturating_sub(5);
    if px + 8 + label.chars().count() as u16 + 1 < keys {
        put(buf, keys, top, "< >", fg(FAINTER), right);
    }
    let (glyphs, all_at) = chips(w);
    for (k, (dx, dy)) in Kind::ALL.into_iter().zip(glyphs) {
        let on = st.atlas.kind.is_none_or(|c| c == k) || k.pinned();
        let mut style = if on {
            bold(k.color())
        } else {
            fg(mix(Color::Rgb(0, 0, 0), k.color(), 0.3))
        };
        if st.atlas.kind == Some(k) {
            style = style.bg(SEL);
        }
        let glyph = k.glyph().to_string();
        put(buf, px + 1 + dx, top + 1 + dy, &glyph, style, right);
    }
    let every = if st.atlas.kind.is_none() {
        fg(CYAN).bg(SEL)
    } else {
        fg(FAINTER)
    };
    put(
        buf,
        px + 1 + all_at.0,
        top + 1 + all_at.1,
        "all",
        every,
        right,
    );
    let rule = "─".repeat(w.saturating_sub(2) as usize);
    put(buf, px, top + 1 + chip_rows(w), &rule, fg(BORDER), right);

    let v = list(st, l, all);
    let foot = v.sel.is_none();
    let (start, last) = lines(l, foot);
    let (names, whats) = (w.saturating_sub(13) as usize, w.saturating_sub(6) as usize);
    let at = right.saturating_sub(7);
    let mut y = start;
    if v.first > 0 {
        put(
            buf,
            px + 1,
            y,
            &format!("▲ {} more", v.first),
            fg(FAINTER),
            right,
        );
        y += 1;
    }
    let mut shown = 0;
    for (n, &i) in v.rows.iter().enumerate().skip(v.first) {
        let x = &all[i];
        let sel = v.sel == Some(n);
        let h = if sel { 3 } else { 1 };
        if y + h > last {
            break;
        }
        if sel {
            fill(buf, Rect::new(px, y, w.saturating_sub(2), 3), SEL);
        }
        put(
            buf,
            px + 1,
            y,
            &x.kind.glyph().to_string(),
            bold(x.kind.color()),
            right,
        );
        let name = if sel { bold(TEXT) } else { fg(DIM) };
        put(buf, px + 3, y, &cut(&x.name, names), name, right);
        let walk = match x.walk {
            Some(m) => format!("{m:>4} m"),
            None => format!("{:>6}", "—"),
        };
        put(
            buf,
            at,
            y,
            &walk,
            fg(if sel { DIM } else { FAINTER }),
            right,
        );
        if sel {
            put(buf, px + 3, y + 1, &cut(&x.what, whats), fg(DIM), right);
            if tracked(st, x) {
                put(buf, px + 3, y + 2, "⌖ tracking", bold(CYAN), right);
                put(buf, px + 14, y + 2, "· t stops", fg(FAINTER), right);
            } else {
                put(buf, px + 3, y + 2, "t track", fg(DIM), right);
            }
        }
        y += h;
        shown += 1;
    }
    let more = v.rows.len() - v.first - shown;
    if more > 0 {
        put(
            buf,
            px + 1,
            last,
            &format!("▼ {more} more"),
            fg(FAINTER),
            right,
        );
    }
    if foot && bottom >= start + 4 {
        put(buf, px, bottom - 4, &rule, fg(BORDER), right);
        for (k, (text, style)) in ground(st, l, cache).into_iter().enumerate() {
            let text = cut(&text, w.saturating_sub(3) as usize);
            put(buf, px + 1, bottom - 3 + k as u16, &text, style, right);
        }
    }
}

/// What the cursor is on, off the listings (spec 7, frame 2): the district and the
/// ground, the walk to its spot and whether you have been there, and what `t` does.
fn ground(st: &AppState, l: &MapLayout, cache: &Cache) -> Vec<(String, Style)> {
    let (Some(city), Some(k), Some(c)) = (st.city.as_ref(), l.stratum, l.cursor) else {
        return Vec::new();
    };
    let tile = city.tile(k, c);
    let terrain = tile.map_or("nothing", |t| t.terrain.label());
    let district = tile
        .and_then(|t| city.district_opt(t.district?))
        .map(|d| d.name.as_str());
    let head = match district {
        Some(d) => format!("CURSOR · {d} · {terrain}"),
        None => format!("CURSOR · {terrain}"),
    };
    let mut out = vec![(head, fg(CYAN).add_modifier(Modifier::BOLD))];
    let spot = spot_under_cursor(st, l);
    let walk = spot.and_then(|p| {
        let n = *cache.field.as_ref()?.get(p)?;
        (n != u32::MAX).then_some(n)
    });
    match (spot, walk) {
        (None, _) => out.push(("nothing to walk to there".to_string(), fg(DIM))),
        (Some(_), None) => out.push(("no way there".to_string(), fg(DIM))),
        (Some(p), Some(m)) => {
            let vis = st.player.as_ref().and_then(|x| x.vis_at(Place::Stratum(k)));
            let been = vis
                .and_then(|g| g.get(p))
                .is_some_and(|v| *v != Visibility::Unseen);
            let seen = if been {
                "you've been here"
            } else {
                "never been"
            };
            out.push((format!("walk {m} m · {seen}"), fg(DIM)));
            let t = match st.track {
                Some(_) => "t tracks this spot instead",
                None => "t tracks this spot",
            };
            out.push((t.to_string(), fg(DIM)));
        }
    }
    out
}

/// The folded pane (spec 7, frame 3): a strip at the body's right edge, `◂f`, `PLACES`
/// down it, the chosen kind's glyph and how many are listed.
pub(super) fn strip(st: &AppState, body: Rect, all: &[Listing], buf: &mut Buffer) {
    let x = body.right().saturating_sub(STRIP);
    let (right, top, bottom) = (body.right(), body.y, body.bottom());
    let on = Style::default().bg(STRIP_BG);
    for y in top..bottom {
        put(buf, x, y, "▕", fg(BORDER).bg(PANEL_BG), right);
        put(buf, x + 1, y, "  ", on, right);
    }
    let mut at = |dx: u16, dy: u16, text: &str, style: Style| {
        if top + dy < bottom {
            put(buf, x + 1 + dx, top + dy, text, on.patch(style), right);
        }
    };
    at(0, 0, "◂f", fg(CYAN).add_modifier(Modifier::BOLD));
    for (k, ch) in "PLACES".chars().enumerate() {
        at(k as u16 % 2, 2 + k as u16, &ch.to_string(), fg(DIM));
    }
    if let Some(k) = st.atlas.kind {
        let glyph = k.glyph().to_string();
        at(0, 9, &glyph, fg(k.color()).add_modifier(Modifier::BOLD));
    }
    let n = all.iter().filter(|l| map::shows(st, l)).count();
    at(0, 10, &n.to_string(), fg(DIM));
}

/// Where the card goes: beside the cursor's cell on the side with room, inside the map.
pub(super) fn card_box(l: &MapLayout) -> Option<Rect> {
    let (cx, cy) = l.cell_of(l.cursor?)?;
    let a = l.area;
    let (w, h) = (CARD_W.min(a.width), CARD_H.min(a.height));
    if w < 3 || h < 3 {
        return None;
    }
    let x = if cx + 3 + CARD_W <= a.right() {
        cx + 3
    } else {
        cx.saturating_sub(3 + CARD_W)
    };
    let x = x.clamp(a.x, a.right() - w);
    let y = cy
        .saturating_sub(2)
        .min(a.bottom().saturating_sub(8))
        .max(a.y)
        .min(a.bottom() - h);
    Some(Rect::new(x, y, w, h))
}

/// The card for the listing under the cursor, the pane open or folded: its glyph and
/// name, what it is, where and how far, and what `t` does.
pub(super) fn card(st: &AppState, l: &MapLayout, at: Rect, x: &Listing, buf: &mut Buffer) {
    let walk = x.walk.map_or("—".to_string(), |m| format!("{m} m"));
    let place = match l.stratum.and_then(|k| district_of(st, k, x)) {
        Some(d) => format!("{d} · {walk}"),
        None => walk,
    };
    let track = if tracked(st, x) {
        ("⌖ tracking · t stops", CYAN)
    } else {
        ("t track", DIM)
    };
    let rows = [
        (format!("{} {}", x.kind.glyph(), x.name), x.kind.color()),
        (x.what.clone(), DIM),
        (place, DIM),
        (track.0.to_string(), track.1),
    ];
    let border = fg(CYAN).remove_modifier(Modifier::BOLD);
    let inner = at.width.saturating_sub(2);
    let right = at.right();
    let line = "─".repeat(inner as usize);
    put(buf, at.x, at.y, &format!("┌{line}┐"), border, right);
    for (k, (text, colour)) in rows.iter().enumerate() {
        let y = at.y + 1 + k as u16;
        if y + 1 >= at.bottom() {
            break;
        }
        put(buf, at.x, y, "│", border, right);
        fill(buf, Rect::new(at.x + 1, y, inner, 1), PANEL_BG);
        put(buf, at.x + 1, y, text, fg(*colour), right - 1);
        put(buf, right - 1, y, "│", border, right);
    }
    put(
        buf,
        at.x,
        at.bottom() - 1,
        &format!("└{line}┘"),
        border,
        right,
    );
}

/// The name of the district a listing stands in.
fn district_of(st: &AppState, k: StratumKind, x: &Listing) -> Option<String> {
    let city = st.city.as_ref()?;
    let id = x
        .building
        .and_then(|b| city.building_opt(b).map(|b| b.district))
        .or_else(|| city.tile(k, x.tile)?.district)?;
    city.district_opt(id).map(|d| d.name.clone())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        BODY_100, BODY_120, STREET, clinic, frame, laid, open_ground, outside, press, text,
    };
    use super::super::{hint, listed, map, pane_width};
    use super::*;
    use crate::track::listings::Kind;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;

    fn sym(buf: &Buffer, x: u16, y: u16) -> &str {
        buf[(x, y)].symbol()
    }

    /// The pane's row `y`, inside its border.
    fn row(buf: &Buffer, body: Rect, y: u16) -> String {
        let w = pane_width(body);
        text(buf, y, body.right() - w + 1..body.right())
    }

    #[test]
    fn the_pane_is_narrower_below_110_columns() {
        assert_eq!(pane_width(BODY_100), 30);
        assert_eq!(pane_width(BODY_120), 36);
        assert_eq!(pane_width(Rect::new(1, 5, 107, 22)), 30);
        assert_eq!(pane_width(Rect::new(1, 5, 108, 22)), 36);
        let mut st = outside();
        for (body, w) in [(BODY_100, 30), (BODY_120, 36)] {
            let buf = frame(&mut st, body);
            for y in body.y..body.bottom() {
                assert_eq!(sym(&buf, body.right() - w, y), "│", "{w} at row {y}");
                assert_eq!(buf[(body.right() - w, y)].fg, BORDER);
            }
            let l = laid(&st);
            assert_eq!(l.area.right(), body.right() - w - 1, "a spare column");
            assert_eq!((l.area.y, l.area.bottom()), (body.y + 1, body.bottom()));
            assert!(row(&buf, body, body.y).starts_with(" PLACES ‹ all ›"));
        }
    }

    #[test]
    fn the_kinds_wrap_at_30_cells_and_not_at_36() {
        let glyphs: String = Kind::ALL.iter().map(|k| k.glyph()).collect();
        let first: String = glyphs.chars().take(14).collect();
        let last = glyphs.chars().last().unwrap();
        for (b, rows) in [(BODY_120, 1), (BODY_100, 2)] {
            let mut st = outside();
            let buf = frame(&mut st, b);
            let chips =
                |y: u16| -> String { row(&buf, b, y).chars().filter(|c| *c != ' ').collect() };
            if rows == 1 {
                assert_eq!(chips(b.y + 1), format!("{glyphs}all"));
            } else {
                assert_eq!(chips(b.y + 1), first);
                assert_eq!(chips(b.y + 2), format!("{last}all"));
                assert_eq!(sym(&buf, b.right() - 30 + 2, b.y + 2), last.to_string());
            }
            // The rule under them, and the list a row lower.
            let rule = b.y + 1 + rows;
            assert!(
                row(&buf, b, rule).contains("──────"),
                "{}",
                row(&buf, b, rule)
            );
            let l = laid(&st);
            let v = list(&st, &l, &listed(&st));
            let x = &listed(&st)[v.rows[v.first]];
            let name: String = x.name.chars().take(15).collect();
            assert!(
                row(&buf, b, rule + 1).contains(&name),
                "{}",
                row(&buf, b, rule + 1)
            );
        }
    }

    #[test]
    fn the_row_under_the_cursor_opens() {
        let mut st = outside();
        let doc = clinic(&st);
        st.atlas.stratum = Some(STREET);
        st.atlas.cursor = Some(doc.tile);
        let b = BODY_120;
        let buf = frame(&mut st, b);
        let px = b.right() - 36 + 1;
        let name: String = doc.name.chars().take(23).collect();
        let y = (b.y + 3..b.bottom())
            .find(|y| row(&buf, b, *y).contains(&name))
            .expect("the clinic's row");
        for dy in 0..3 {
            for x in px..px + 34 {
                assert_eq!(buf[(x, y + dy)].bg, SEL, "{x},{}", y + dy);
            }
        }
        assert!(buf[(px + 3, y)].modifier.contains(Modifier::BOLD));
        let what: String = doc.what.chars().take(30).collect();
        assert!(row(&buf, b, y + 1).contains(&what));
        assert!(row(&buf, b, y + 2).contains("t track"));
        let lit = |buf: &Buffer| {
            (b.y + 3..b.bottom())
                .filter(|y| buf[(px, *y)].bg == SEL)
                .count()
        };
        assert_eq!(lit(&buf), 3, "one row, opened");
        // Tracked, it says so.
        press(&mut st, 't');
        let buf = frame(&mut st, b);
        let y = (b.y + 3..b.bottom())
            .find(|y| row(&buf, b, *y).contains(&name))
            .unwrap();
        assert!(row(&buf, b, y + 2).contains("⌖ tracking · t stops"));
        assert_eq!(buf[(px + 3, y + 2)].fg, crate::ui::chrome::CYAN);

        // Off a listing: no row lit, and the foot of the pane says what is there.
        open_ground(&mut st, b);
        let buf = frame(&mut st, b);
        assert_eq!(lit(&buf), 0);
        let foot = |buf: &Buffer, k: u16| row(buf, b, b.bottom() - k);
        assert!(foot(&buf, 3).starts_with(" CURSOR · "), "{}", foot(&buf, 3));
        assert!(foot(&buf, 2).starts_with(" walk "), "{}", foot(&buf, 2));
        assert!(foot(&buf, 2).contains(" m · "));
        assert!(foot(&buf, 1).contains("t tracks this spot instead"));
        assert!(foot(&buf, 4).contains("──────"));
        st.track = None;
        let buf = frame(&mut st, b);
        assert!(foot(&buf, 1).trim_end().ends_with("t tracks this spot"));
    }

    /// The pane's line `k` rows from the body's bottom.
    fn foot(buf: &Buffer, b: Rect, k: u16) -> String {
        row(buf, b, b.bottom() - k)
    }

    #[test]
    fn from_the_flat_the_foot_says_the_walk() {
        let mut st = crate::track::listings::tests::in_the_flat();
        let b = BODY_120;
        open_ground(&mut st, b);
        let buf = frame(&mut st, b);
        assert!(foot(&buf, b, 3).starts_with(" CURSOR · "));
        let walk = foot(&buf, b, 2);
        assert!(
            walk.starts_with(" walk ") && walk.contains(" m · "),
            "{walk}"
        );
    }

    #[test]
    fn a_spot_walled_off_has_no_way_there() {
        let mut st = outside();
        let b = BODY_120;
        open_ground(&mut st, b);
        let l = laid(&st);
        let spot = spot_under_cursor(&st, &l).unwrap();
        let city = st.city.as_mut().unwrap();
        for p in spot.neighbors8() {
            city.tile_mut(STREET, p).unwrap().terrain = citygen::Terrain::Wall;
        }
        st.tick += 30;
        let buf = frame(&mut st, b);
        assert_eq!(spot_under_cursor(&st, &laid(&st)), Some(spot));
        assert_eq!(foot(&buf, b, 2).trim(), "no way there");
    }

    #[test]
    fn f_folds_and_the_strip_marks_it() {
        let mut st = outside();
        st.atlas.kind = Some(Kind::Docs);
        let b = BODY_120;
        frame(&mut st, b);
        assert!(hint(&st).contains("f fold ·"));
        press(&mut st, 'f');
        assert!(st.atlas.folded);
        let buf = frame(&mut st, b);
        let x = b.right() - 4;
        for y in b.y..b.bottom() {
            assert_eq!(sym(&buf, x, y), "▕");
            assert_eq!(buf[(x, y)].fg, BORDER);
            assert_eq!(buf[(x + 1, y)].bg, STRIP_BG);
            assert_eq!(buf[(x + 2, y)].bg, STRIP_BG);
        }
        assert_eq!(text(&buf, b.y, x + 1..x + 3), "◂f");
        assert_eq!(buf[(x + 1, b.y)].fg, crate::ui::chrome::CYAN);
        let word: String = (0..6u16)
            .map(|k| sym(&buf, x + 1 + k % 2, b.y + 2 + k).to_string())
            .collect();
        assert_eq!(word, "PLACES");
        assert_eq!(sym(&buf, x + 1, b.y + 9), "+");
        let n = listed(&st).iter().filter(|l| map::shows(&st, l)).count();
        assert_eq!(text(&buf, b.y + 10, x + 1..x + 3).trim(), n.to_string());
        // The map takes the pane's cells.
        let l = laid(&st);
        assert_eq!((l.area.x, l.area.right()), (b.x, x));
        assert!(!text(&buf, b.y, b.x..b.right()).contains("PLACES"));
        assert!(hint(&st).contains("f unfold ·"));
        press(&mut st, 'f');
        let buf = frame(&mut st, b);
        assert!(row(&buf, b, b.y).starts_with(" PLACES ‹ ripperdocs ›"));
        assert_eq!(sym(&buf, b.right() - 36, b.y + 3), "│");
    }

    #[test]
    fn the_card_shows_with_the_pane_open() {
        for b in [BODY_120, BODY_100] {
            let mut st = outside();
            let doc = clinic(&st);
            st.atlas.stratum = Some(STREET);
            st.atlas.cursor = Some(doc.tile);
            frame(&mut st, b);
            press(&mut st, 't');
            let buf = frame(&mut st, b);
            let l = laid(&st);
            assert!(!st.atlas.folded);
            let card = card_box(&l).expect("a card");
            let a = l.area;
            assert!(
                card.x >= a.x && card.right() <= a.right(),
                "{card:?} in {a:?}"
            );
            assert!(
                card.y >= a.y && card.bottom() <= a.bottom(),
                "{card:?} in {a:?}"
            );
            assert_eq!(sym(&buf, card.x, card.y), "┌");
            assert_eq!(sym(&buf, card.right() - 1, card.bottom() - 1), "┘");
            let inner = |y: u16| text(&buf, y, card.x + 1..card.right() - 1);
            let name: String = doc.name.chars().take(20).collect();
            assert!(inner(card.y + 1).starts_with("+ "), "{}", inner(card.y + 1));
            assert!(inner(card.y + 1).contains(&name));
            assert!(inner(card.y + 4).starts_with("⌖ tracking · t stops"));
            // The row still opens in the list.
            let y = (b.y + 3..b.bottom())
                .find(|y| row(&buf, b, *y).contains(&name))
                .expect("the clinic's row");
            assert!(row(&buf, b, y + 2).contains("⌖ tracking · t stops"));
            // Nothing animates under it.
            let cells = |buf: &Buffer| -> Vec<ratatui::buffer::Cell> {
                card.positions().map(|p| buf[(p.x, p.y)].clone()).collect()
            };
            st.tick += 11;
            let later = frame(&mut st, b);
            assert_eq!(cells(&buf), cells(&later));
            // Off the listings, no card.
            open_ground(&mut st, b);
            let buf = frame(&mut st, b);
            let l = laid(&st);
            let map: String = (l.area.y..l.area.bottom())
                .map(|y| text(&buf, y, l.area.x..l.area.right()))
                .collect();
            assert!(!map.contains(&name), "{b:?}");
        }
    }

    #[test]
    fn the_card_stays_inside() {
        let b = BODY_100;
        for east in [true, false] {
            let mut st = outside();
            st.atlas.folded = true;
            let all: Vec<_> = listed(&st)
                .iter()
                .filter(|l| l.walk.is_some() && !l.kind.pinned())
                .cloned()
                .collect();
            let far = if east {
                all.iter().max_by_key(|l| l.tile.x)
            } else {
                all.iter().min_by_key(|l| l.tile.x)
            }
            .unwrap()
            .clone();
            st.atlas.stratum = Some(STREET);
            st.atlas.cursor = Some(far.tile);
            crate::track::set(&mut st, crate::track::Source::Listing(far.id)).unwrap();
            let buf = frame(&mut st, b);
            let l = laid(&st);
            let (cx, cy) = l.cell_of(far.tile).unwrap();
            let card = card_box(&l).expect("a card");
            let a = l.area;
            assert_eq!(card.width, 30);
            assert!(
                card.x >= a.x && card.right() <= a.right(),
                "{card:?} in {a:?}"
            );
            assert!(
                card.y >= a.y && card.bottom() <= a.bottom(),
                "{card:?} in {a:?}"
            );
            if east {
                assert!(cx + 3 + 30 > a.right(), "near the right edge");
                assert!(card.right() < cx, "on the left, where there is room");
            } else {
                assert!(card.x > cx, "on the right");
            }
            // Two rows above the cursor, or as low as leaves two rows under it.
            let want = (cy.saturating_sub(2)).min(a.bottom() - 8).max(a.y);
            assert_eq!(card.y, want);
            assert_eq!(sym(&buf, card.x, card.y), "┌");
            assert_eq!(sym(&buf, card.right() - 1, card.y), "┐");
            assert_eq!(sym(&buf, card.x, card.bottom() - 1), "└");
            assert_eq!(sym(&buf, card.right() - 1, card.bottom() - 1), "┘");
            let first = text(&buf, card.y + 1, card.x + 1..card.right() - 1);
            assert!(
                first.starts_with(&format!("{} ", far.kind.glyph())),
                "{first}"
            );
            assert!(first.contains(&far.name.chars().take(20).collect::<String>()));
            let last = text(&buf, card.y + 4, card.x + 1..card.right() - 1);
            assert!(last.starts_with("⌖ tracking · t stops"), "{last}");
            // Nothing animates under it.
            let cells = |buf: &Buffer| -> Vec<ratatui::buffer::Cell> {
                card.positions().map(|p| buf[(p.x, p.y)].clone()).collect()
            };
            st.tick += 11;
            let later = frame(&mut st, b);
            assert_eq!(cells(&buf), cells(&later));
        }
    }
}
