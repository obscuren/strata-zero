//! The bubbles look (scenes spec §6): the game's frame stays, the HUD dims, and the
//! line floats in a rounded bubble beside whoever speaks. The map, the doors and the
//! marker are drawn by the explore screen; this is the rest.

use super::film::cut;
use super::overlay;
use super::scene;
use crate::app::AppState;
use crate::render::viewport::tile_to_screen;
use crate::screens::explore::Regions;
use crate::ui::chrome::{DIM, PANEL_BG, TEXT};
use crate::ui::dim::wash_rect;
use crate::ui::sidebar::wrap_text;
use crate::walker::Visibility;
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The widest a bubble gets.
const WIDTH: u16 = 34;
/// The title in the footer: magenta at 80 %.
const TITLE: Color = Color::Rgb(204, 0, 160);
const KEYS: &str = "space next   esc skip";

/// Dims the top band and the side panel, clears the four footer rows to the panel and
/// writes `cinematic · <TITLE>` and the keys on them, and the stage line in the map's
/// top-left corner. With the console open its row is left alone.
pub fn chrome(buf: &mut Buffer, r: &Regions, st: &AppState, console: bool) {
    let Some(run) = scene::running(st) else {
        return;
    };
    for rect in r
        .band_l
        .iter()
        .chain(&r.band_r)
        .chain([&r.top_rule, &r.side])
    {
        wash_rect(buf, *rect);
    }
    if let Some(route) = r.route {
        wash_rect(buf, route);
    }
    for (i, row) in r.footer.iter().enumerate() {
        if console && i == 3 {
            continue;
        }
        for x in row.x..row.right() {
            let cell = &mut buf[(x, row.y)];
            cell.reset();
            cell.set_bg(PANEL_BG);
        }
    }
    let title = format!("cinematic · {}", run.title().to_uppercase());
    let bold = Style::default()
        .fg(TITLE)
        .bg(PANEL_BG)
        .add_modifier(Modifier::BOLD);
    let top = r.footer[0];
    put(buf, top.x + 1, top.y, &title, bold, top.right());
    if !console {
        let keys = Style::default().fg(DIM).bg(PANEL_BG);
        let last = r.footer[3];
        put(buf, last.x + 1, last.y, KEYS, keys, last.right());
    }
    if let Some(stage) = run.stage() {
        let stage = cut(&format!(" ▸ {stage} "), r.map.width as usize);
        let style = Style::default().fg(DIM).bg(PANEL_BG);
        put(buf, r.map.x + 1, r.map.y + 1, &stage, style, r.map.right());
    }
}

/// Draws the current line as a bubble in `map`, around the speaker's cell when it is on
/// screen, never over the player or anyone else standing there.
pub fn say(buf: &mut Buffer, map: Rect, origin: Pos, st: &AppState) {
    let Some(line) = scene::running(st).and_then(|s| s.line()) else {
        return;
    };
    let Some(me) = st.player.as_ref() else {
        return;
    };
    let w = WIDTH.min(map.width.saturating_sub(2));
    let inner = w.saturating_sub(4).max(1) as usize;
    // Wrapped whole, so the words don't jump about while they type.
    let mut wrapped = wrap_text(&line.text, inner);
    let now = st.clock_tick();
    let typed = overlay::typed_rows(&line.text, &wrapped, line.shown(now).chars().count());
    let rows = map.height.saturating_sub(2).max(1) as usize;
    if wrapped.len() > rows {
        wrapped.truncate(rows);
        if let Some(last) = wrapped.last_mut() {
            *last = cut(&format!("{last}…"), inner);
        }
    }
    let h = wrapped.len() as u16 + 2;
    let anchor = line
        .anchor
        .filter(|(place, _)| *place == me.place)
        .and_then(|(_, at)| tile_to_screen(map, origin, at));
    let vis = me.vis();
    let avoid: Vec<(u16, u16)> = std::iter::once(me.pos)
        .chain(
            st.missions
                .visitors
                .iter()
                .filter(|v| v.place == me.place && vis.get(v.pos) == Some(&Visibility::Visible))
                .map(|v| v.pos),
        )
        .filter_map(|p| tile_to_screen(map, origin, p))
        .collect();
    let (rect, tail) = place(anchor, map, w, h, &avoid);

    let (label, colour) = overlay::speaker(st, line);
    let edge = Style::default().fg(colour).bg(PANEL_BG);
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            let cell = &mut buf[(x, y)];
            cell.reset();
            cell.set_bg(PANEL_BG);
        }
    }
    let wide = rect.width as usize;
    let label = cut(&label, wide.saturating_sub(5));
    let dashes = wide.saturating_sub(label.chars().count() + 5);
    let top = format!("╭─ {label} {}╮", "─".repeat(dashes));
    put(buf, rect.x, rect.y, &top, edge, rect.right());
    let bold = edge.add_modifier(Modifier::BOLD);
    put(buf, rect.x + 3, rect.y, &label, bold, rect.right());
    let text = Style::default().fg(TEXT).bg(PANEL_BG);
    let mut caret = (rect.x + 2, rect.y + 1);
    for (i, (row, &take)) in wrapped.iter().zip(&typed).enumerate() {
        let y = rect.y + 1 + i as u16;
        put(buf, rect.x, y, "│", edge, rect.right());
        put(buf, rect.right() - 1, y, "│", edge, rect.right());
        let shown: String = row.chars().take(take).collect();
        put(buf, rect.x + 2, y, &shown, text, rect.right() - 1);
        if take > 0 {
            caret = (rect.x + 2 + shown.chars().count() as u16, y);
        }
    }
    let bottom = format!("╰{}╯", "─".repeat(wide.saturating_sub(2)));
    put(buf, rect.x, rect.bottom() - 1, &bottom, edge, rect.right());
    if line.typing(now) {
        put(buf, caret.0, caret.1, "▌", edge, rect.right() - 1);
    }
    if let Some((x, y, ch)) = tail {
        let style = Style::default().fg(colour).add_modifier(Modifier::BOLD);
        overlay::ink(&mut buf[(x, y)], ch, style);
    }
}

/// Where a `w` by `h` bubble goes in `map` for a speaker on the screen cell `anchor`,
/// and the tail's cell and glyph. Above (the bottom border two rows up, a `▾` between)
/// if it fits in the map and neither it nor its tail covers any of `avoid`, else below
/// (`▴`); centred on the
/// speaker, clamped into the map. With no anchor or no spot: the map's bottom-left
/// corner, no tail.
pub fn place(
    anchor: Option<(u16, u16)>,
    map: Rect,
    w: u16,
    h: u16,
    avoid: &[(u16, u16)],
) -> (Rect, Option<(u16, u16, char)>) {
    let w = w.min(map.width);
    let h = h.min(map.height);
    if let Some((sx, sy)) = anchor {
        let x = (i32::from(sx) - i32::from(w / 2))
            .clamp(i32::from(map.x), i32::from(map.right() - w)) as u16;
        let spots = [
            (i32::from(sy) - 1 - i32::from(h), i32::from(sy) - 1, '▾'),
            (i32::from(sy) + 2, i32::from(sy) + 1, '▴'),
        ];
        for (top, tail_y, glyph) in spots {
            if top < i32::from(map.y) || top + i32::from(h) > i32::from(map.bottom()) {
                continue;
            }
            let rect = Rect::new(x, top as u16, w, h);
            let tail = (sx, tail_y as u16);
            if !avoid.iter().any(|&c| rect.contains(c.into()) || c == tail) {
                return (rect, Some((tail.0, tail.1, glyph)));
            }
        }
    }
    (Rect::new(map.x, map.bottom() - h, w, h), None)
}

/// Writes `s` from `x` in its whole `style`, stopping before column `right`.
fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, style: Style, right: u16) {
    if y >= buf.area.bottom() {
        return;
    }
    for (cx, ch) in (x..right.min(buf.area.right())).zip(s.chars()) {
        overlay::ink(&mut buf[(cx, y)], ch, style);
    }
}

#[cfg(test)]
mod tests {
    use super::super::scene::tests::{SLOW, SPACE, playing, press, queued, run_ticks, see};
    use super::super::{Look, Mark, marks, scene, visitors};
    use super::*;
    use crate::screens::explore::MapScreen;
    use crate::screens::shots::draw_stack;
    use crate::screens::testutil::buffer_text;

    const MAP: Rect = Rect::new(2, 5, 59, 20);

    #[test]
    fn place_prefers_above() {
        let (rect, tail) = place(Some((30, 15)), MAP, 34, 4, &[]);
        assert_eq!(rect.bottom() - 1, 13, "the bottom border on sy - 2");
        assert_eq!(tail, Some((30, 14, '▾')));
        assert_eq!((rect.x, rect.width, rect.height), (13, 34, 4));
    }

    #[test]
    fn place_clamps_into_the_map() {
        let (rect, _) = place(Some((2, 15)), MAP, 34, 4, &[]);
        assert_eq!(rect.x, MAP.x);
        let (rect, _) = place(Some((60, 15)), MAP, 34, 4, &[]);
        assert_eq!(rect.right(), MAP.right());
    }

    #[test]
    fn place_goes_below_near_the_top() {
        let (rect, tail) = place(Some((30, MAP.y)), MAP, 34, 4, &[]);
        assert_eq!(rect.y, MAP.y + 2);
        assert_eq!(tail, Some((30, MAP.y + 1, '▴')));
    }

    #[test]
    fn place_never_covers_the_player() {
        let (above, _) = place(Some((30, 15)), MAP, 34, 4, &[]);
        let me = (above.x + 3, above.y + 1);
        let (rect, tail) = place(Some((30, 15)), MAP, 34, 4, &[me]);
        assert_eq!(rect.y, 17);
        assert_eq!(tail, Some((30, 16, '▴')));
        assert!(!rect.contains(me.into()));
    }

    #[test]
    fn place_never_puts_the_tail_on_an_actor() {
        // Someone right above the speaker: the tail above would land on them.
        let (rect, tail) = place(Some((30, 15)), MAP, 34, 4, &[(30, 14)]);
        assert_eq!(tail, Some((30, 16, '▴')));
        assert_eq!(rect.y, 17);
        // Above and below both: the corner, no tail.
        let (rect, tail) = place(Some((30, 15)), MAP, 34, 4, &[(30, 14), (30, 16)]);
        assert_eq!((rect.x, rect.bottom(), tail), (MAP.x, MAP.bottom(), None));
    }

    #[test]
    fn place_falls_back_to_the_corner() {
        let (rect, tail) = place(None, MAP, 34, 4, &[]);
        assert_eq!((rect.x, rect.bottom()), (MAP.x, MAP.bottom()));
        assert_eq!(tail, None);
        // Above covers the player and below runs off the map: the corner too.
        let (rect, tail) = place(Some((30, 22)), MAP, 34, 4, &[(30, 18)]);
        assert_eq!((rect.x, rect.bottom(), tail), (MAP.x, MAP.bottom(), None));
    }

    /// The test scene as bubbles, started.
    fn bubbling(beats: &str) -> AppState {
        let mut st = queued(SLOW, beats);
        st.book.scenes.get_mut("s").unwrap().look = Look::Bubbles;
        super::super::tick(&mut st);
        assert_eq!(scene::running(&st).unwrap().look(), Look::Bubbles);
        st
    }

    fn drawn(st: &mut AppState, size: (u16, u16)) -> (Buffer, Rect) {
        let map = MapScreen::walking(st);
        let buf = draw_stack(st, &[&map], size);
        (buf, map.map_area())
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// The bubble's rect, from the cells of its `╭` and `╯`.
    fn bubble(buf: &Buffer) -> Option<Rect> {
        let find = |s: &str| {
            (0..buf.area.height)
                .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
                .find(|&(x, y)| buf[(x, y)].symbol() == s)
        };
        let (a, b) = (find("╭")?, find("╯")?);
        Some(Rect::new(a.0, a.1, b.0 - a.0 + 1, b.1 - a.1 + 1))
    }

    #[test]
    fn bubbles_fit_100x30() {
        let text = ["abcdef"; 17].join(" ");
        assert_eq!(text.len(), 118);
        let mut st = bubbling(&format!(r#"Say(who: "castellan", text: "{text}")"#));
        assert!(press(&mut st, SPACE), "space shows the whole line");
        let (buf, map) = drawn(&mut st, (100, 30));
        assert_eq!((map.width, map.height), (59, 20));
        let rect = bubble(&buf).unwrap_or_else(|| panic!("a bubble\n{}", buffer_text(&buf)));
        assert!(
            map.contains(rect.as_position())
                && map.contains((rect.right() - 1, rect.bottom() - 1).into()),
            "{rect:?} in {map:?}"
        );
        let words: usize = (rect.y..rect.bottom())
            .map(|y| row(&buf, y).replace(['│', '╭', '╮', '╰', '╯', '─'], " "))
            .map(|r| r.split_whitespace().filter(|w| w.starts_with('a')).count())
            .sum();
        assert_eq!(words, 17);
    }

    #[test]
    fn bubbles_dim_the_hud_and_say_the_keys() {
        let mut st = bubbling(r#"Stage("The lock clicks."), Say(who: "castellan", text: "Hi.")"#);
        let (buf, map) = drawn(&mut st, (120, 40));
        assert!(
            row(&buf, 36).starts_with(" cinematic · TEST"),
            "{}",
            row(&buf, 36)
        );
        assert!(
            row(&buf, 39).starts_with(" space next"),
            "{}",
            row(&buf, 39)
        );
        assert_eq!(buf[(1, 36)].fg, TITLE);
        assert_eq!(buf[(1, 39)].fg, DIM);
        assert_eq!(buf[(1, 36)].bg, PANEL_BG);
        let stage: String = row(&buf, map.y + 1)
            .chars()
            .skip(map.x as usize + 1)
            .collect();
        assert!(stage.starts_with(" ▸ The lock clicks. "), "{stage}");
        // The band is still drawn, dimmed.
        let mut plain = playing(SLOW, r#"Say(who: "castellan", text: "Hi.")"#);
        let (film, _) = drawn(&mut plain, (120, 40));
        assert!(row(&buf, 1).contains("HP") || row(&buf, 2).contains("HP"));
        assert!(!row(&film, 1).contains("HP"));
    }

    #[test]
    fn the_bubble_sits_by_the_speaker_and_off_the_player() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Hi.")"#);
        st.book.scenes.get_mut("s").unwrap().look = Look::Bubbles;
        let (place, pos) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        visitors::arrive(&mut st, "castellan", (place, pos.offset(2, 0)));
        super::super::tick(&mut st);
        see(&mut st);
        let (buf, _) = drawn(&mut st, (120, 40));
        let rect = bubble(&buf).expect("a bubble");
        let me: Vec<(u16, u16)> = (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                buf[(x, y)].symbol() == "@" && buf[(x, y)].fg == Color::Rgb(255, 255, 255)
            })
            .collect();
        assert_eq!(me.len(), 1, "you are on the map");
        assert!(!rect.contains(me[0].into()), "{rect:?} covers {me:?}");
        // The tail points at the speaker from the side of the bubble.
        let tail = (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                matches!(buf[(x, y)].symbol(), "▾" | "▴")
                    && buf[(x, y)].fg == Color::Rgb(255, 200, 80)
            })
            .count();
        assert!(tail >= 1);
    }

    #[test]
    fn typing_keeps_pace_past_a_long_word() {
        // A word wider than the bubble splits with no space dropped at the break.
        let long = "y".repeat(40);
        let mut st = bubbling(&format!(r#"Say(who: "castellan", text: "{long} end")"#));
        // A character a tick.
        run_ticks(&mut st, 32);
        let (buf, _) = drawn(&mut st, (120, 40));
        let rect = bubble(&buf).expect("a bubble");
        let typed: Vec<usize> = (rect.y + 1..rect.bottom() - 1)
            .map(|y| {
                (rect.x..rect.right())
                    .filter(|&x| buf[(x, y)].symbol() == "y")
                    .count()
            })
            .collect();
        assert_eq!(typed, [30, 2]);
    }

    /// A map of grey cells, bold and dim, as a lit wall or a sign can be.
    fn bold_dim(area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        let style = Style::default()
            .fg(Color::Rgb(90, 90, 90))
            .bg(Color::Rgb(20, 20, 20))
            .add_modifier(Modifier::BOLD | Modifier::DIM);
        buf.set_style(area, style);
        buf
    }

    #[test]
    fn the_stage_line_sets_its_whole_style() {
        let st = bubbling(r#"Stage("The lock clicks."), Say(who: "castellan", text: "Hi.")"#);
        let area = Rect::new(0, 0, 120, 40);
        let r = crate::screens::explore::layout(area, false);
        let mut buf = bold_dim(area);
        chrome(&mut buf, &r, &st, false);
        let stage = " ▸ The lock clicks. ";
        let (x, y) = (r.map.x + 1, r.map.y + 1);
        let cells: Vec<&ratatui::buffer::Cell> = (x..x + stage.chars().count() as u16)
            .map(|cx| &buf[(cx, y)])
            .collect();
        let text: String = cells.iter().map(|c| c.symbol()).collect();
        assert_eq!(text, stage);
        assert!(
            cells.iter().all(|c| c.modifier.is_empty() && c.fg == DIM),
            "uniformly dim"
        );
        let title = &buf[(r.footer[0].x + 1, r.footer[0].y)];
        assert_eq!(title.modifier, Modifier::BOLD);
    }

    #[test]
    fn the_tail_sets_its_whole_style() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Hi.")"#);
        st.book.scenes.get_mut("s").unwrap().look = Look::Bubbles;
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(2), None).unwrap();
        visitors::arrive(&mut st, "castellan", (place, pos));
        super::super::tick(&mut st);
        // You well off the view; he is at (30, 10).
        st.player = Some(crate::walker::Player::at(place, pos.offset(60, 0)));
        let map = Rect::new(0, 0, 60, 20);
        let mut buf = bold_dim(map);
        say(&mut buf, map, pos.offset(-30, -10), &st);
        let tail = &buf[(30, 9)];
        assert_eq!(tail.symbol(), "▾");
        assert_eq!(tail.modifier, Modifier::BOLD);
        assert_eq!(tail.fg, Color::Rgb(255, 200, 80));
    }

    #[test]
    fn a_keypad_washes_the_map_and_the_bubble_comes_after() {
        use crate::app::Screen;
        use ratatui::{Terminal, backend::TestBackend};
        let pad = "Keypad(door: HomeDoor, code: Code(1234), result: Denied, keys: 0.1, hold: 0.5)";
        let mut st = bubbling(pad);
        let (buf, map) = drawn(&mut st, (120, 40));
        let run = scene::running(&st).unwrap();
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| run.pad().unwrap().draw(&st, f)).unwrap();
        let alone = term.backend().buffer().clone();
        let mut inked = 0;
        for y in 0..40 {
            for x in 0..120 {
                if alone[(x, y)].symbol() != " " {
                    inked += 1;
                    assert_eq!(buf[(x, y)].symbol(), alone[(x, y)].symbol());
                }
            }
        }
        assert!(inked > 20, "the pad is drawn over the map");
        // The map is washed; the footer still says the keys.
        let mut plain = bubbling(r#"Say(who: "castellan", text: "Hi.")"#);
        let (mut reference, _) = drawn(&mut plain, (120, 40));
        wash_rect(&mut reference, map);
        let at = (map.x, map.y + 3);
        assert_eq!(buf[at].fg, reference[at].fg);
        assert!(row(&buf, 39).starts_with(" space next"));
        // A line after the pad gets its bubble.
        let mut st = bubbling(&format!(r#"{pad}, Say(who: "castellan", text: "Hi.")"#));
        for _ in 0..2 {
            assert!(press(&mut st, SPACE));
        }
        let (buf, _) = drawn(&mut st, (120, 40));
        assert!(bubble(&buf).is_some());
    }
}
