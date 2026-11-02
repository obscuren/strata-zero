//! The broadcast (morning news spec §4): the current news item on your holo set, a
//! screen over the map laid out as the HOLO feed is: the camera's still at the left
//! where the feed has the face, the report beside it, the SOUGHT card, the crawl along
//! the foot. `space` switches the set off; `esc` leaves it on.

use super::film::cut;
use super::{cast, face, fill, news, person_of};
use crate::app::{AppState, Screen, ScreenCmd};
use crate::ui::chrome::{DIM, FAINT, MAGENTA, PANEL_BG, TEXT, put};
use crate::ui::transition::TransitionKind;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The widest the broadcast gets.
const MAX_W: u16 = 90;
/// The fewest rows the box has: the still and its caption above the crawl.
const MIN_H: u16 = 17;
/// Columns from the box's left edge to the report.
const TEXT_AT: u16 = 22;
/// Columns the report gives up beside its column: the still's side and the margins.
const TEXT_LESS: u16 = 26;
/// Rows of the box that are not report or card: the edges, the header and the gap under
/// it, the gap under the report, the key row and the gap under it, the crawl.
const CHROME_H: u16 = 8;
/// The SOUGHT card's two rows and the gap under it.
const CARD_H: u16 = 3;
/// Columns from the card's left edge to its lines: the bar, `SOUGHT` and a gap.
const CARD_TEXT: u16 = 10;
/// The still's cells: 18 pixels wide, 18 high, two pixel rows to a cell.
const STILL_W: u16 = 18;
const STILL_ROWS: u16 = 9;
/// The live mark, the bracket on the still and the SOUGHT card.
const RED: Color = Color::Rgb(255, 48, 72);
/// Under the SOUGHT card.
const CARD_BG: Color = Color::Rgb(26, 8, 14);
/// Under the crawl.
const CRAWL_BG: Color = Color::Rgb(40, 0, 32);
/// The crawl's tag at its left.
const CRAWL_TAG: &str = " NEWSLINE ";
/// Frame ticks per column the crawl moves: ten columns a second.
const CRAWL_TICKS: u64 = 3;
/// The camera's greens, dark to light: a still pixel's `0` to `6`.
const GREENS: [(u8, u8, u8); 7] = [
    (4, 8, 5),
    (11, 23, 16),
    (21, 41, 27),
    (36, 67, 44),
    (63, 112, 73),
    (106, 168, 119),
    (182, 240, 194),
];
/// The odd pixel rows' brightness: the scanlines.
const SCANLINE: f32 = 0.8;
/// The grain's seed, so the still holds still.
const GRAIN_SEED: u32 = 0x5eed_1234;

/// The broadcast on the screen: whatever item is current.
#[derive(Default)]
pub struct Broadcast;

impl Broadcast {
    pub fn new() -> Broadcast {
        Broadcast
    }
}

/// The broadcast's box in `frame` and the report's rows: the paragraphs wrapped with a
/// blank row between them, cut with `…` where the box has no more room. `sought`: the
/// SOUGHT card shows, and takes its rows.
pub fn layout(frame: Rect, report: &[String], sought: bool) -> (Rect, Vec<String>) {
    let w = MAX_W.min(frame.width.saturating_sub(4));
    let room = usize::from(w.saturating_sub(TEXT_LESS)).max(1);
    let mut lines: Vec<String> = Vec::new();
    for (i, para) in report.iter().enumerate() {
        if i > 0 {
            lines.push(String::new());
        }
        lines.extend(crate::ui::sidebar::wrap_text(para, room));
    }
    let chrome = CHROME_H + if sought { CARD_H } else { 0 };
    let wanted = (lines.len() as u16).saturating_add(chrome).max(MIN_H);
    let h = wanted.min(frame.height.saturating_sub(2));
    let rows = usize::from(h.saturating_sub(chrome));
    if lines.len() > rows {
        lines.truncate(rows);
        if let Some(last) = lines.last_mut() {
            let mut cut: String = last.chars().take(room.saturating_sub(1)).collect();
            cut.push('…');
            *last = cut;
        }
    }
    let x = frame.x + frame.width.saturating_sub(w) / 2;
    let y = frame.y + frame.height.saturating_sub(h) / 2;
    (Rect::new(x, y, w, h), lines)
}

/// A still pixel's colour: `0` to `6` the camera's greens, the odd pixel rows dimmer;
/// `r` the bracket's red.
pub fn still_colour(level: char, row: usize) -> Color {
    if level == 'r' {
        return RED;
    }
    let v = level.to_digit(10).map_or(0, |v| v as usize);
    let (r, g, b) = GREENS[v.min(GREENS.len() - 1)];
    let k = if row % 2 == 1 { SCANLINE } else { 1.0 };
    let f = |c: u8| (f32::from(c) * k) as u8;
    Color::Rgb(f(r), f(g), f(b))
}

/// The still's pixels with a light grain over them, as the mock drew it: of every
/// hundred, five a level lighter and four a level darker, from a fixed seed so the
/// picture holds still. The bracket keeps its red.
fn grained(still: &[String]) -> Vec<Vec<char>> {
    let mut seed = GRAIN_SEED;
    let mut out = Vec::with_capacity(still.len());
    for row in still {
        let mut px = Vec::with_capacity(row.len());
        for c in row.chars() {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let roll = (seed >> 16) % 100;
            px.push(match c.to_digit(10) {
                Some(v) if roll < 5 && v < 6 => char::from_digit(v + 1, 10).unwrap_or(c),
                Some(v) if roll > 95 && v > 0 => char::from_digit(v - 1, 10).unwrap_or(c),
                _ => c,
            });
        }
        out.push(px);
    }
    out
}

/// Draws the still from (`x`, `y`) in `▀`s: each cell's top pixel as its foreground,
/// the one under it as its background.
fn draw_still(buf: &mut Buffer, x: u16, y: u16, still: &[String]) {
    let px = grained(still);
    for (j, pair) in px.chunks(2).take(usize::from(STILL_ROWS)).enumerate() {
        let bottom = pair.get(1);
        for (i, &top) in pair[0].iter().take(usize::from(STILL_W)).enumerate() {
            let under = bottom.and_then(|b| b.get(i)).copied().unwrap_or('0');
            buf[(x + i as u16, y + j as u16)]
                .set_symbol("▀")
                .set_fg(still_colour(top, 2 * j))
                .set_bg(still_colour(under, 2 * j + 1));
        }
    }
}

/// The crawl's text once round: each line in capitals after a `▸`, and a gap at the end
/// as wide as those between, so it runs on into its start.
fn crawl_text(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let upper: Vec<String> = lines.iter().map(|l| l.to_uppercase()).collect();
    format!(" ▸ {}  ", upper.join("   ▸ "))
}

fn close() -> ScreenCmd {
    ScreenCmd::Pop {
        fx: TransitionKind::None,
    }
}

impl Screen for Broadcast {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if news::current(st).is_none() {
            return close();
        }
        for k in keys {
            match k.code {
                KeyCode::Char(' ') => {
                    news::switch_off(st);
                    return close();
                }
                KeyCode::Esc => return close(),
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let Some(n) = news::current(st) else {
            return;
        };
        // The book's item: the still, the camera and the key's label.
        let item = st.book.news.get(&n.id);
        let sought = news::sought(st);
        let (r, lines) = layout(f.area(), &n.report, sought.is_some());
        if r.width < TEXT_LESS + 8 || r.height < MIN_H {
            return;
        }
        let buf = f.buffer_mut();
        face::bracket_frame(buf, r);
        let bold = |c| Style::default().fg(c).add_modifier(Modifier::BOLD);
        let plain = |c| Style::default().fg(c).bg(PANEL_BG);
        let right = r.right();
        put(buf, r.x + 3, r.y + 1, "HOLO", bold(MAGENTA), right);
        put(buf, r.x + 8, r.y + 1, "▸ news", plain(DIM), right);
        let live = right.saturating_sub(18);
        let channel = cut(
            &format!("· {}", n.channel),
            usize::from(live.saturating_sub(r.x + 16)),
        );
        put(buf, r.x + 15, r.y + 1, &channel, plain(FAINT), live);
        put(buf, live, r.y + 1, "●", plain(RED), right);
        put(buf, live + 2, r.y + 1, "LIVE", bold(RED), right);
        let (h, m) = st.clock();
        put(
            buf,
            right.saturating_sub(11),
            r.y + 1,
            &format!("{h:02}:{m:02}"),
            plain(DIM),
            right,
        );
        // The still, then which camera took it, when you left, and where.
        let side = r.x + TEXT_AT - 1;
        let caption_y = r.y + 4 + STILL_ROWS;
        if let Some(item) = item {
            draw_still(buf, r.x + 3, r.y + 3, &item.still);
            put(buf, r.x + 3, caption_y, "●", plain(RED), side);
            let camera = cut(&fill(st, &item.camera), usize::from(side - (r.x + 5)));
            put(buf, r.x + 5, caption_y, &camera, bold(DIM), side);
            if let Some(left) = news::walked_away(st, &n.about) {
                let at = r.x + 6 + camera.chars().count() as u16;
                let when = format!("· {:02}:{:02}", left.at.0, left.at.1);
                put(buf, at, caption_y, &when, plain(FAINT), side);
            }
        }
        if let Some(who) = person_of(st, &n.about) {
            let district = cut(&cast::district_of(st, who), usize::from(side - (r.x + 3)));
            put(buf, r.x + 3, caption_y + 1, &district, plain(FAINT), side);
        }
        let tx = r.x + TEXT_AT;
        for (j, line) in lines.iter().enumerate() {
            put(buf, tx, r.y + 3 + j as u16, line, plain(TEXT), right);
        }
        // The description, as a lower third.
        let room = r.width - TEXT_LESS;
        if let Some(sought) = &sought {
            let card_y = r.y + 4 + lines.len() as u16;
            let end = tx + room;
            let on = |c| Style::default().fg(c).bg(CARD_BG);
            for (dy, text) in sought.iter().enumerate() {
                let y = card_y + dy as u16;
                put(buf, tx, y, &" ".repeat(usize::from(room)), on(TEXT), end);
                put(buf, tx, y, "▌", on(RED), end);
                let text = cut(text, usize::from(room.saturating_sub(CARD_TEXT)));
                put(buf, tx + CARD_TEXT, y, &text, on(TEXT), end);
            }
            let label = on(RED).add_modifier(Modifier::BOLD);
            put(buf, tx + 2, card_y, "SOUGHT", label, end);
        }
        let key_y = r.bottom() - 4;
        put(buf, tx, key_y, "space", bold(MAGENTA), right);
        if let Some(item) = item {
            let label = cut(&fill(st, &item.close), usize::from(room - 6));
            put(buf, tx + 6, key_y, &label, plain(DIM), right);
        }
        // The crawl along the foot, scrolling left and round.
        let (crawl_y, inner) = (r.bottom() - 2, right - 1);
        let band = Style::default().bg(CRAWL_BG);
        let blank = " ".repeat(usize::from(r.width - 2));
        put(buf, r.x + 1, crawl_y, &blank, band, inner);
        let tag = Style::default()
            .fg(PANEL_BG)
            .bg(MAGENTA)
            .add_modifier(Modifier::BOLD);
        put(buf, r.x + 1, crawl_y, CRAWL_TAG, tag, inner);
        let text: Vec<char> = crawl_text(&n.crawl).chars().collect();
        if !text.is_empty() {
            let from = (st.tick / CRAWL_TICKS % text.len() as u64) as usize;
            let at = r.x + 1 + CRAWL_TAG.chars().count() as u16;
            let shown: String = text
                .iter()
                .cycle()
                .skip(from)
                .take(usize::from(inner.saturating_sub(at)))
                .collect();
            put(buf, at, crawl_y, &shown, band.fg(DIM), inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::missions::news::tests::{afters, news_then_after};
    use crate::missions::{Left, cast, person_of};
    use crate::screens::explore::MapScreen;
    use crate::screens::shots::draw_stack;
    use crate::screens::testutil::{buffer_text, key};
    use crossterm::event::KeyCode;
    use ratatui::buffer::Buffer;

    const SIZES: [(u16, u16); 2] = [(120, 40), (100, 30)];

    /// Seed 8 with its home and the map, the story begun and the news put out from the
    /// console.
    fn on_air() -> (AppState, MapScreen) {
        let mut st = state_home();
        let map = MapScreen::walking(&mut st);
        crate::missions::begin(&mut st);
        news::put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        (st, map)
    }

    /// Seed 8 with a book of one mission whose first step puts the news out.
    fn on_air_for_a_step() -> AppState {
        let mut st = state_home();
        news_then_after(&mut st);
        crate::missions::begin(&mut st);
        assert!(news::current(&st).is_some_and(|n| n.mission == 0));
        st
    }

    /// The cells of row `y` from column `x0` up to `x1`.
    fn row(buf: &Buffer, y: u16, x0: u16, x1: u16) -> String {
        (x0..x1).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// The broadcast's box at `size` for the state's item.
    fn boxed(st: &AppState, (w, h): (u16, u16)) -> (Rect, Vec<String>) {
        let n = news::current(st).unwrap();
        layout(Rect::new(0, 0, w, h), &n.report, news::sought(st).is_some())
    }

    #[test]
    fn the_box_fits_both_sizes() {
        let (st, _) = on_air();
        let report = &news::current(&st).unwrap().report;
        for (w, h) in SIZES {
            for sought in [true, false] {
                let (r, lines) = layout(Rect::new(0, 0, w, h), report, sought);
                assert!(
                    r.width <= 90 && r.width + 4 <= w && r.height >= 17 && r.height + 2 <= h,
                    "{w}x{h}: {r:?}"
                );
                assert_eq!(r.x, (w - r.width) / 2);
                assert!(lines.len() > 4, "{lines:?}");
                assert!(
                    !lines.iter().any(|l| l.contains('…')),
                    "the whole report fits"
                );
            }
        }
        // The card and its gap: three rows, when the text sets the height.
        let tall = vec!["word ".repeat(120)];
        let frame = Rect::new(0, 0, 120, 40);
        let (with, lines) = layout(frame, &tall, true);
        let (without, _) = layout(frame, &tall, false);
        assert_eq!(with.height, lines.len() as u16 + 11);
        assert_eq!(with.height - without.height, 3);
        // Too long for the screen: cut with an ellipsis.
        let (r, lines) = layout(Rect::new(0, 0, 100, 30), &["word ".repeat(300)], true);
        assert_eq!((r.width, r.height), (90, 28));
        assert_eq!(lines.len(), 17);
        assert!(lines[16].ends_with('…'));
    }

    #[test]
    fn it_draws_what_the_mock_drew() {
        for size in SIZES {
            let (mut st, map) = on_air();
            // Marlo dead and you seen walking away north at 22:41.
            crate::missions::end(&mut st, 0, 0);
            let about = person_of(&st, &news::current(&st).unwrap().about).unwrap();
            let body = st.missions.bodies.iter_mut().find(|b| b.person == about);
            body.expect("the item's body").left = Some(Left {
                at: (22, 41),
                heading: "north",
                worn: None,
            });
            let n = news::current(&st).unwrap();
            let (r, lines) = boxed(&st, size);
            let buf = draw_stack(&st, &[&map, &Broadcast::new()], size);
            let line = |y: u16| row(&buf, y, r.x, r.right());
            let head = line(r.y + 1);
            let (h, m) = st.clock();
            let channel = format!("· {}", n.channel);
            let clock = format!("{h:02}:{m:02}");
            for want in ["HOLO", "▸ news", &channel, "● LIVE", &clock] {
                assert!(head.contains(want), "{size:?}: {want:?} not in {head:?}");
            }
            // The still: nine rows of half blocks, eighteen wide.
            for (x, y) in [(3, 3), (20, 3), (3, 11), (20, 11)] {
                assert_eq!(buf[(r.x + x, r.y + y)].symbol(), "▀", "{size:?} {x},{y}");
            }
            assert_ne!(buf[(r.x + 3, r.y + 12)].symbol(), "▀");
            assert!(
                row(&buf, r.y + 13, r.x + 3, r.x + 22).starts_with("● CAM 14 · 22:41"),
                "{size:?}: {}",
                line(r.y + 13)
            );
            let district = cast::district_of(&st, about);
            let shown = row(&buf, r.y + 14, r.x + 3, r.x + 22);
            assert!(shown.trim_end() == district, "{size:?}: {shown:?}");
            // The report from column 22.
            let first: Vec<&str> = n.report[0].split_whitespace().take(3).collect();
            let text = row(&buf, r.y + 3, r.x + 22, r.right());
            assert!(text.starts_with(&first.join(" ")), "{size:?}: {text:?}");
            for (j, l) in lines.iter().enumerate() {
                let y = r.y + 3 + j as u16;
                assert!(row(&buf, y, r.x + 22, r.right()).starts_with(l.as_str()));
            }
            // A blank row, then SOUGHT.
            let card = r.y + 3 + lines.len() as u16 + 1;
            assert!(
                row(&buf, card - 1, r.x + 22, r.right() - 1)
                    .trim()
                    .is_empty()
            );
            let sought = news::sought(&st).unwrap();
            let top = row(&buf, card, r.x + 22, r.right());
            assert!(top.starts_with("▌ SOUGHT"), "{size:?}: {top:?}");
            assert!(top.contains(&sought[0]), "{size:?}: {top:?}");
            let next = row(&buf, card + 1, r.x + 22, r.right());
            assert!(next.starts_with("▌"), "{size:?}: {next:?}");
            assert!(next.contains(&sought[1]), "{size:?}: {next:?}");
            assert!(sought[1].ends_with("22:41, heading north"));
            // The key row three above the bottom edge, the crawl on the row above it.
            let keys = row(&buf, r.bottom() - 4, r.x + 22, r.right());
            assert!(keys.starts_with("space turn it off"), "{size:?}: {keys:?}");
            assert_eq!(row(&buf, r.bottom() - 2, r.x + 1, r.x + 11), " NEWSLINE ");
            let boxed: String = (r.y..r.bottom()).map(&line).collect();
            assert!(
                !boxed.contains('{') && !boxed.contains('…'),
                "{size:?}: {boxed}"
            );
        }
    }

    #[test]
    fn the_caption_leaves_the_minute_out_when_nobody_saw_you_go() {
        let (st, map) = on_air();
        let (r, _) = boxed(&st, (120, 40));
        let buf = draw_stack(&st, &[&map, &Broadcast::new()], (120, 40));
        let caption = row(&buf, r.y + 13, r.x + 3, r.x + 22);
        assert_eq!(caption.trim_end(), "● CAM 14");
        let card = news::sought(&st).unwrap();
        assert!(card[1].ends_with("late last night"));
        assert!(buffer_text(&buf).contains(&card[1]));
    }

    #[test]
    fn without_sought_lines_the_card_and_its_gap_are_left_out() {
        let (mut st, map) = on_air();
        let id = news::current(&st).unwrap().id.clone();
        st.book.news.remove(&id);
        assert!(news::sought(&st).is_none());
        let (r, lines) = boxed(&st, (120, 40));
        assert_eq!(r.height, (lines.len() as u16 + 8).max(17));
        let buf = draw_stack(&st, &[&map, &Broadcast::new()], (120, 40));
        let shown = buffer_text(&buf);
        assert!(!shown.contains("▌ SOUGHT"), "{shown}");
        assert!(row(&buf, r.bottom() - 4, r.x + 22, r.right()).starts_with("space"));
        assert_eq!(row(&buf, r.bottom() - 2, r.x + 1, r.x + 11), " NEWSLINE ");
    }

    #[test]
    fn the_crawl_moves() {
        let (mut st, _) = on_air();
        let crawl = |st: &AppState| {
            let (r, _) = boxed(st, (120, 40));
            let buf = draw_stack(st, &[&Broadcast::new()], (120, 40));
            row(&buf, r.bottom() - 2, r.x + 11, r.right() - 1)
        };
        st.tick = 0;
        let at_0 = crawl(&st);
        let first = news::current(&st).unwrap().crawl[0].to_uppercase();
        assert!(at_0.starts_with(&format!(" ▸ {first}   ▸ ")), "{at_0:?}");
        st.tick = 30;
        let at_30 = crawl(&st);
        assert_ne!(at_0, at_30);
        // Ten columns a second, to the left.
        let (a, b): (Vec<char>, Vec<char>) = (at_0.chars().collect(), at_30.chars().collect());
        assert_eq!(b[..b.len() - 10], a[10..]);
        // Round and round: once its whole length has gone by it reads as it began.
        let once = crawl_text(&news::current(&st).unwrap().crawl);
        st.tick = CRAWL_TICKS * once.chars().count() as u64;
        assert_eq!(crawl(&st), at_0);
    }

    #[test]
    fn space_switches_off_and_ends_the_step() {
        let mut st = on_air_for_a_step();
        let mut b = Broadcast::new();
        assert!(matches!(b.update(&mut st, &[]), ScreenCmd::Stay));
        let cmd = b.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(matches!(cmd, ScreenCmd::Pop { .. }));
        let n = st.missions.news.as_ref().unwrap();
        assert!(!n.on && n.ended);
        assert_eq!(afters(&st), 1);
    }

    #[test]
    fn esc_closes_with_the_set_on() {
        let mut st = on_air_for_a_step();
        let cmd = Broadcast::new().update(&mut st, &[key(KeyCode::Esc)]);
        assert!(matches!(cmd, ScreenCmd::Pop { .. }));
        let n = st.missions.news.as_ref().unwrap();
        assert!(n.on && !n.ended);
        assert_eq!(afters(&st), 0);
    }

    #[test]
    fn enter_does_nothing() {
        let mut st = on_air_for_a_step();
        let mut b = Broadcast::new();
        for code in [KeyCode::Enter, KeyCode::Char('x'), KeyCode::Char('i')] {
            assert!(matches!(b.update(&mut st, &[key(code)]), ScreenCmd::Stay));
        }
        let n = st.missions.news.as_ref().unwrap();
        assert!(n.on && !n.ended);
        assert_eq!(afters(&st), 0);
    }

    #[test]
    fn with_the_item_gone_it_pops() {
        let drawn = |st: &AppState| {
            let buf = draw_stack(st, &[&Broadcast::new()], (120, 40));
            buffer_text(&buf)
        };
        let mut st = state_home();
        let mut b = Broadcast::new();
        assert!(matches!(b.update(&mut st, &[]), ScreenCmd::Pop { .. }));
        assert!(drawn(&st).trim().is_empty());
        // Stale.
        crate::missions::begin(&mut st);
        news::put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        assert!(matches!(b.update(&mut st, &[]), ScreenCmd::Stay));
        assert!(!drawn(&st).trim().is_empty());
        st.missions.news.as_mut().unwrap().fresh = 0;
        assert!(matches!(b.update(&mut st, &[]), ScreenCmd::Pop { .. }));
        assert!(drawn(&st).trim().is_empty());
    }

    #[test]
    fn the_still_is_green_with_a_red_bracket() {
        assert_eq!(still_colour('r', 0), Color::Rgb(255, 48, 72));
        assert_eq!(still_colour('r', 1), Color::Rgb(255, 48, 72));
        assert_eq!(still_colour('0', 0), Color::Rgb(4, 8, 5));
        assert_eq!(still_colour('6', 0), Color::Rgb(182, 240, 194));
        // Odd pixel rows at 80 %.
        assert_eq!(still_colour('6', 1), Color::Rgb(145, 192, 155));
        assert_eq!(still_colour('4', 3), Color::Rgb(50, 89, 58));
    }

    #[test]
    fn the_grain_holds_still_and_is_light() {
        let (st, _) = on_air();
        let still = &st.book.news["marlo"].still;
        let a = grained(still);
        assert_eq!(a, grained(still));
        let changed: usize = a
            .iter()
            .zip(still)
            .map(|(g, s)| g.iter().zip(s.chars()).filter(|(x, y)| *x != y).count())
            .sum();
        assert!((5..=40).contains(&changed), "{changed} of 324");
        // The bracket keeps its red.
        for (g, s) in a.iter().zip(still) {
            for (x, y) in g.iter().zip(s.chars()) {
                assert_eq!(*x == 'r', y == 'r');
            }
        }
    }
}
