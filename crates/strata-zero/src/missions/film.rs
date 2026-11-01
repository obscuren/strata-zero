//! The film look (scenes spec §5): the frame black, the map in the middle at its usual
//! size, what happens dim in the top bar, the line as a subtitle under the map. The
//! map and the overlays on it are drawn by the explore screen; this is the rest.

use super::overlay::{self, WHITE};
use super::scene::{self, Line};
use crate::app::AppState;
use crate::ui::chrome::{DIM, FAINT};
use crate::ui::sidebar::wrap_text;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

const BLACK: Color = Color::Rgb(0, 0, 0);
/// The title: magenta at 60 %.
const TITLE: Color = Color::Rgb(153, 0, 120);
const HINT: &str = "space next · esc skip";
/// The most rows a subtitle takes.
const SUBTITLE_ROWS: u16 = 3;

/// `map` moved to the middle of `frame`; only its x changes.
pub fn map_rect(frame: Rect, map: Rect) -> Rect {
    let slack = frame.width.saturating_sub(map.width);
    Rect::new(frame.x + slack / 2, map.y, map.width, map.height)
}

/// Blacks out everything outside `map`, then draws the top bar, the subtitle and the
/// hint.
pub fn draw(buf: &mut Buffer, frame: Rect, map: Rect, st: &AppState) {
    let Some(run) = scene::running(st) else {
        return;
    };
    for y in frame.y..frame.bottom() {
        for x in frame.x..frame.right() {
            if !map.contains((x, y).into()) {
                let cell = &mut buf[(x, y)];
                cell.reset();
                cell.set_bg(BLACK);
            }
        }
    }
    let title = cut(&run.title().to_uppercase(), map.width as usize);
    text(buf, frame, map.x, frame.y + 1, &title, TITLE, true);
    if let Some(stage) = run.stage() {
        let stage = cut(stage, map.width as usize);
        let w = stage.chars().count() as u16;
        let x = frame.x + frame.width.saturating_sub(w) / 2;
        text(buf, frame, x, map.y.saturating_sub(2), &stage, DIM, false);
    }
    let hint_y = frame.bottom().saturating_sub(1);
    let hint_x = map.right().saturating_sub(HINT.chars().count() as u16);
    text(buf, frame, hint_x.max(frame.x), hint_y, HINT, FAINT, false);
    if let Some(line) = run.line() {
        let top = map.bottom() + 1;
        let rows = hint_y.saturating_sub(top).min(SUBTITLE_ROWS);
        subtitle(buf, frame, map, st, line, top, rows);
    }
}

/// `s` cut to `w` cells, with `…` when it was longer.
pub(super) fn cut(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        return s.to_string();
    }
    s.chars().take(w.saturating_sub(1)).chain(['…']).collect()
}

/// The subtitle: the label, then the line wrapped under it, as much as has been typed,
/// the block centred in the frame from row `top`, up to `rows` rows.
fn subtitle(
    buf: &mut Buffer,
    frame: Rect,
    map: Rect,
    st: &AppState,
    line: &Line,
    top: u16,
    rows: u16,
) {
    if rows == 0 {
        return;
    }
    let (label, colour) = overlay::speaker(st, line);
    let label_w = label.chars().count();
    let text_w = (map.width as usize).saturating_sub(label_w + 2).max(1);
    // Wrapped whole, so the words don't jump about while they type.
    let mut wrapped = wrap_text(&line.text, text_w);
    let now = st.clock_tick();
    let typed = overlay::typed_rows(&line.text, &wrapped, line.shown(now).chars().count());
    if wrapped.len() > rows as usize {
        wrapped.truncate(rows as usize);
        if let Some(last) = wrapped.last_mut() {
            *last = cut(&format!("{last}…"), text_w);
        }
    }
    let width = label_w + 2 + wrapped.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    let x = frame.x + frame.width.saturating_sub(width as u16) / 2;
    text(buf, frame, x, top, &label, colour, true);
    let tx = x + label_w as u16 + 2;
    let mut caret = (tx, top);
    for (i, (row, &take)) in wrapped.iter().zip(&typed).enumerate() {
        let y = top + i as u16;
        let shown: String = row.chars().take(take).collect();
        text(buf, frame, tx, y, &shown, WHITE, false);
        if take > 0 {
            caret = (tx + shown.chars().count() as u16, y);
        }
    }
    if line.typing(now) {
        text(buf, frame, caret.0, caret.1, "▌", colour, false);
    }
}

/// Writes `s` from `x` on black in its whole style, stopping at the frame's edge.
fn text(buf: &mut Buffer, frame: Rect, x: u16, y: u16, s: &str, fg: Color, bold: bool) {
    if y >= frame.bottom() {
        return;
    }
    let mut style = Style::default().fg(fg).bg(BLACK);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    for (cx, ch) in (x..frame.right()).zip(s.chars()) {
        overlay::ink(&mut buf[(cx, y)], ch, style);
    }
}

#[cfg(test)]
mod tests {
    use super::super::scene::tests::{SLOW, SPACE, playing, press, queued, run_ticks, see};
    use super::super::{Mark, marks, overlay, visitors};
    use crate::app::AppState;
    use crate::screens::explore::MapScreen;
    use crate::screens::shots::draw_stack;
    use crate::screens::testutil::buffer_text;
    use crate::ui::chrome::{DIM, FAINT};
    use ratatui::buffer::{Buffer, Cell};
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};

    const AMBER: Color = Color::Rgb(255, 200, 80);

    /// `st` drawn as the map at `size`, with the map's rect.
    fn drawn(st: &mut AppState, size: (u16, u16)) -> (Buffer, Rect) {
        let map = MapScreen::walking(st);
        let buf = draw_stack(st, &[&map], size);
        (buf, map.map_area())
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    fn cells(buf: &Buffer, f: impl Fn(&Cell) -> bool) -> Vec<(u16, u16)> {
        let mut out = Vec::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if f(&buf[(x, y)]) {
                    out.push((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn film_hides_the_hud() {
        let mut st = playing(
            SLOW,
            r#"Stage("The lock clicks."), Say(who: "castellan", text: "Hello there.")"#,
        );
        let (buf, map) = drawn(&mut st, (120, 40));
        assert_eq!((map.x, map.y, map.width, map.height), (20, 5, 79, 30));
        let top: String = (0..5).map(|y| row(&buf, y)).collect();
        assert!(!top.contains("HP") && !top.contains("FOCUS"), "{top}");
        assert!(row(&buf, 1)[map.x as usize..].starts_with("TEST"));
        let stage = "The lock clicks.";
        let at = (120 - stage.len()) / 2;
        assert_eq!(row(&buf, 3).trim(), stage);
        assert!(row(&buf, 3).starts_with(&format!("{}{stage}", " ".repeat(at))));
        assert_eq!(buf[(at as u16, 3)].fg, DIM);
        let sub = row(&buf, 36);
        let label_at = sub.find("CASTELLAN").expect("the label");
        assert_eq!(sub[..label_at].trim(), "");
        let hint = row(&buf, 39);
        assert!(hint.trim_end().ends_with("esc skip"), "{hint}");
        assert_eq!(hint.trim_end().chars().count() as u16, map.right());
        assert_eq!(buf[(map.right() - 1, 39)].fg, FAINT);
        // Outside the map is black; the HUD's rules are gone.
        assert!(!buffer_text(&buf).contains('┿'));
        assert_eq!(buf[(5, 20)].bg, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn film_fits_100x30() {
        let text = ["abcdef"; 20].join(" ");
        let mut st = playing(SLOW, &format!(r#"Say(who: "castellan", text: "{text}")"#));
        assert!(press(&mut st, SPACE), "space shows the whole line");
        let (buf, map) = drawn(&mut st, (100, 30));
        assert_eq!((map.width, map.height), (59, 20));
        let rows: Vec<String> = (26..29).map(|y| row(&buf, y)).collect();
        assert!(rows.iter().all(|r| !r.trim().is_empty()), "{rows:?}");
        assert!(rows[0].contains("CASTELLAN"));
        let words = rows.join(" ").replace("CASTELLAN", "");
        assert_eq!(words.split_whitespace().count(), 20, "{rows:?}");
        assert!(row(&buf, 29).trim_end().ends_with("esc skip"));
        assert!(row(&buf, 25).trim().is_empty());
    }

    #[test]
    fn typing_keeps_pace_past_a_long_word() {
        // A word wider than the subtitle splits with no space dropped at the break.
        let long = "x".repeat(100);
        let mut st = playing(
            SLOW,
            &format!(r#"Say(who: "castellan", text: "{long} end")"#),
        );
        // A character a tick.
        run_ticks(&mut st, 70);
        let (buf, _) = drawn(&mut st, (120, 40));
        let typed = |y| row(&buf, y).chars().filter(|c| *c == 'x').count();
        assert_eq!((typed(36), typed(37)), (68, 2));
    }

    #[test]
    fn the_marker_sits_over_the_speaker() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Hi.")"#);
        let (place, pos) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        visitors::arrive(&mut st, "castellan", (place, pos.offset(2, 0)));
        super::super::tick(&mut st);
        see(&mut st);
        let (buf, _) = drawn(&mut st, (120, 40));
        let who = cells(&buf, |c| {
            c.symbol() == "@" && c.fg == AMBER && c.modifier.contains(Modifier::BOLD)
        });
        assert_eq!(who.len(), 1, "the visitor is in view");
        let (x, y) = who[0];
        assert_eq!(buf[(x, y - 1)].symbol(), "▾");
        assert_eq!(buf[(x, y - 1)].fg, AMBER);
        assert!(buf[(x, y - 1)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn a_hidden_speaker_reads_dim() {
        let mut st = playing(SLOW, VOICE);
        // You inside, off the walkway the marker hangs over.
        let (place, inside) = marks::resolve(&st, &Mark::HomeInside(2), None).unwrap();
        st.player = Some(crate::walker::Player::at(place, inside));
        see(&mut st);
        let (buf, _) = drawn(&mut st, (120, 40));
        let sub = row(&buf, 36);
        assert!(
            sub.contains("A VOICE") && !sub.contains("CASTELLAN"),
            "{sub}"
        );
        let x = sub.find("A VOICE").unwrap() as u16;
        assert_eq!(buf[(x, 36)].fg, DIM);
        let marker = cells(&buf, |c| c.symbol() == "▾");
        assert_eq!(marker.len(), 1);
        assert_eq!(buf[marker[0]].fg, DIM);
        let line = super::super::scene::running(&st).unwrap().line().unwrap();
        assert_eq!(overlay::speaker(&st, line), ("A VOICE".to_string(), DIM));
    }

    /// The voice behind your door, as the house call has it.
    const VOICE: &str =
        r#"Say(who: "castellan", as: Some("A voice"), at: Some(HomeDoor), text: "Come in.")"#;

    #[test]
    fn the_voice_at_the_door_never_covers_you() {
        let mut st = playing(SLOW, VOICE);
        // You on the cell the marker would take, over the door.
        let (place, door) = marks::resolve(&st, &Mark::HomeDoor, None).unwrap();
        st.player = Some(crate::walker::Player::at(place, door.offset(0, -1)));
        see(&mut st);
        let (buf, _) = drawn(&mut st, (120, 40));
        let me = cells(&buf, |c| {
            c.symbol() == "@" && c.fg == Color::Rgb(255, 255, 255)
        });
        assert_eq!(me.len(), 1, "your @ shows");
        assert!(cells(&buf, |c| c.symbol() == "▾").is_empty());
    }

    #[test]
    fn an_open_door_lets_light_in() {
        let mut st = playing(
            SLOW,
            r#"Door(at: HomeDoor, open: true), Say(who: "you", text: "Open.")"#,
        );
        see(&mut st);
        let (buf, _) = drawn(&mut st, (120, 40));
        let door = cells(&buf, |c| c.symbol() == "·" && c.fg == AMBER);
        assert_eq!(door.len(), 1, "{door:?}");
        let (x, y) = door[0];
        assert_eq!(buf[(x, y)].bg, Color::Rgb(64, 50, 20));
        let (_, d) = marks::resolve(&st, &Mark::HomeDoor, None).unwrap();
        let (_, out) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        let (dx, dy) = (d.x - out.x, d.y - out.y);
        for (k, bg) in [(1, (31, 24, 10)), (2, (23, 18, 7)), (3, (15, 12, 5))] {
            let c = &buf[((x as i32 + dx * k) as u16, (y as i32 + dy * k) as u16)];
            assert_eq!(c.bg, Color::Rgb(bg.0, bg.1, bg.2), "{k} deep");
        }
    }

    #[test]
    fn film_draws_no_track() {
        use super::super::Look;
        // The map's cells drawn in this look, untracked and then tracked to a tile two
        // in from your door.
        let frames = |look: Look| {
            let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Hello.")"#);
            st.book.scenes.get_mut("s").unwrap().look = look;
            super::super::tick(&mut st);
            see(&mut st);
            let (plain, map) = drawn(&mut st, (120, 40));
            let (place, pos) = marks::resolve(&st, &Mark::HomeInside(2), None).unwrap();
            let spot = crate::track::Source::Spot {
                place,
                pos,
                radius: 0,
            };
            crate::track::set(&mut st, spot).unwrap();
            let leg = crate::track::leg_now(&st).unwrap().unwrap();
            assert!(!leg.path.is_empty() && leg.place == place);
            let (tracked, _) = drawn(&mut st, (120, 40));
            let cells = |b: &Buffer| -> Vec<Cell> {
                (map.y..map.bottom())
                    .flat_map(|y| (map.x..map.right()).map(move |x| (x, y)))
                    .map(|p| b[p].clone())
                    .collect()
            };
            (cells(&plain), cells(&tracked))
        };
        let (plain, tracked) = frames(Look::Film);
        assert!(plain == tracked, "the film shows no walk");
        let (plain, tracked) = frames(Look::Bubbles);
        assert!(plain != tracked, "the bubbles keep the HUD's walk");
    }

    const PAD: &str =
        "Keypad(door: HomeDoor, code: Code(1234), result: Denied, keys: 0.1, hold: 0.5)";

    #[test]
    fn a_keypad_washes_the_map_and_draws_over_it() {
        use crate::app::Screen;
        use ratatui::{Terminal, backend::TestBackend};
        let mut st = playing(SLOW, PAD);
        let (buf, map) = drawn(&mut st, (120, 40));
        let mut plain = playing(SLOW, r#"Say(who: "castellan", text: "Hi.")"#);
        let (mut reference, _) = drawn(&mut plain, (120, 40));
        crate::ui::dim::wash_rect(&mut reference, map);
        let (x, y) = (map.x, map.y + 3);
        assert_eq!(buf[(x, y)].fg, reference[(x, y)].fg);
        assert_eq!(buf[(x, y)].bg, reference[(x, y)].bg);
        // The pad is where it draws alone.
        let pad = super::super::scene::running(&st).unwrap().pad().unwrap();
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| pad.draw(&st, f)).unwrap();
        let alone = term.backend().buffer().clone();
        let mut inked = 0;
        for py in 0..40 {
            for px in 0..120 {
                if alone[(px, py)].symbol() != " " {
                    inked += 1;
                    assert_eq!(buf[(px, py)].symbol(), alone[(px, py)].symbol());
                }
            }
        }
        assert!(inked > 20, "the pad has something on it");
        // The HUD rows stay black.
        assert_eq!(buf[(5, 20)].bg, Color::Rgb(0, 0, 0));
        assert!(!buffer_text(&buf).contains("FOCUS"));
    }
}
