//! The HOLO feed (spec 7): a screen over the map where someone speaks at length, the
//! face at the left and the words beside it, closed with `space`.

use super::{FeedView, face};
use crate::app::{AppState, Screen, ScreenCmd};
use crate::ui::chrome::{DIM, MAGENTA, PANEL_BG, TEXT, put};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// The widest the feed gets.
const MAX_W: u16 = 90;
/// Columns from the box's left edge to the text.
const TEXT_AT: u16 = 22;
/// Columns the text gives up beside its column: the face's side and the margins.
const TEXT_LESS: u16 = 26;
/// The fewest rows the box has.
const MIN_H: u16 = 16;
/// Rows of the box that are not text: the header, the gaps (one blank row between the
/// last paragraph and the key row) and the key row.
const CHROME_H: u16 = 7;

/// A feed on the screen.
pub struct Feed {
    view: FeedView,
    /// The tick it opened on; the face's animation counts from it.
    opened: u64,
}

impl Feed {
    pub fn new(view: FeedView, tick: u64) -> Feed {
        Feed { view, opened: tick }
    }
}

/// The feed's box in `frame` and the text's rows: the paragraphs wrapped with a blank row
/// between them, cut with `…` where the box has no more room.
pub fn layout(frame: Rect, text: &[String]) -> (Rect, Vec<String>) {
    let w = MAX_W.min(frame.width.saturating_sub(4));
    let room = usize::from(w.saturating_sub(TEXT_LESS)).max(1);
    let mut lines: Vec<String> = Vec::new();
    for (i, para) in text.iter().enumerate() {
        if i > 0 {
            lines.push(String::new());
        }
        lines.extend(crate::ui::sidebar::wrap_text(para, room));
    }
    let wanted = (lines.len() as u16).saturating_add(CHROME_H).max(MIN_H);
    let h = wanted.min(frame.height.saturating_sub(2));
    let rows = usize::from(h.saturating_sub(CHROME_H));
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

impl Screen for Feed {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if keys
            .iter()
            .any(|k| matches!(k.code, KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Esc))
        {
            super::end(st, self.view.mission, self.view.step);
            return ScreenCmd::Pop {
                fx: crate::ui::transition::TransitionKind::None,
            };
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let (r, lines) = layout(f.area(), &self.view.text);
        if r.width < TEXT_LESS + 8 || r.height < MIN_H.min(8) {
            return;
        }
        let buf = f.buffer_mut();
        face::bracket_frame(buf, r);
        let bold = |c| Style::default().fg(c).add_modifier(Modifier::BOLD);
        let plain = |c| Style::default().fg(c).bg(PANEL_BG);
        let right = r.right();
        put(buf, r.x + 3, r.y + 1, "HOLO", bold(MAGENTA), right);
        put(buf, r.x + 8, r.y + 1, "▸ incoming", plain(DIM), right);
        put(
            buf,
            r.right().saturating_sub(16),
            r.y + 1,
            "▂▄▆█",
            plain(MAGENTA),
            right,
        );
        let (h, m) = st.clock();
        put(
            buf,
            r.right().saturating_sub(11),
            r.y + 1,
            &format!("{h:02}:{m:02}"),
            plain(DIM),
            right,
        );
        let who = &self.view.from;
        let face = st.book.face_of(who);
        let tick = st.tick.saturating_sub(self.opened);
        face::draw(buf, r.x + 3, r.y + 3, face, tick, face::seed_of(st, who));
        let name_y = r.y + 4 + face::rows(face);
        let name = super::role_name(st, who).to_uppercase();
        put(buf, r.x + 3, name_y, &name, bold(MAGENTA), right);
        if let Some(role) = st.book.cast.get(who) {
            let at = r.x + 3 + name.chars().count() as u16 + 1;
            put(
                buf,
                at,
                name_y,
                &format!("· {}", role.title),
                plain(DIM),
                right,
            );
        }
        let tx = r.x + TEXT_AT;
        for (j, line) in lines.iter().enumerate() {
            put(buf, tx, r.y + 3 + j as u16, line, plain(TEXT), right);
        }
        let key_y = r.bottom().saturating_sub(3);
        put(buf, tx, key_y, "space", bold(MAGENTA), right);
        put(buf, tx + 6, key_y, &self.view.close, plain(DIM), right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::screens::explore::MapScreen;
    use crate::screens::shots::draw_stack;
    use crate::screens::testutil::{buffer_text, key};
    use game::ItemId;

    #[test]
    fn the_feed_keeps_every_word_at_100x30_and_120x40() {
        for size in [(100, 30), (120, 40)] {
            let mut st = state_home();
            let map = MapScreen::walking(&mut st);
            super::super::begin(&mut st);
            let view = st.missions.feeds.pop_front().unwrap();
            let words: Vec<String> = view
                .text
                .iter()
                .flat_map(|p| p.split_whitespace().map(str::to_string))
                .collect();
            assert!(view.text.len() == 3 && words.len() > 30);
            let feed = Feed::new(view, st.tick);
            let shown = buffer_text(&draw_stack(&st, &[&map, &feed], size));
            for w in &words {
                assert!(shown.contains(w.as_str()), "{size:?}: {w:?} missing");
            }
            for w in ["space", "take the package", "HOLO", "MEZZ"] {
                assert!(shown.contains(w), "{size:?}: {w:?} missing");
            }
            assert!(!shown.contains('…'), "{size:?}");
        }
    }

    #[test]
    fn layout_cuts_long_text_with_an_ellipsis() {
        let frame = Rect::new(0, 0, 100, 20);
        let text = vec!["word ".repeat(300)];
        let (r, lines) = layout(frame, &text);
        assert_eq!((r.width, r.height), (90, 18));
        assert_eq!(lines.len(), 11);
        assert!(lines[10].ends_with('…'));
        let (r, _) = layout(Rect::new(0, 0, 60, 30), &["hi".to_string()]);
        assert_eq!((r.width, r.height, r.x), (56, 16, 2));
    }

    #[test]
    fn a_blank_row_separates_the_text_from_the_key_row() {
        for size in [(100, 30), (120, 40)] {
            let mut st = state_home();
            let map = MapScreen::walking(&mut st);
            super::super::begin(&mut st);
            let view = st.missions.feeds.pop_front().unwrap();
            let feed = Feed::new(view, st.tick);
            let (r, lines) = layout(Rect::new(0, 0, size.0, size.1), &feed.view.text);
            let buf = draw_stack(&st, &[&map, &feed], size);
            let row = |y: u16| -> String {
                (r.x + super::TEXT_AT..r.right() - 2)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            };
            let key_y = r.bottom() - 3;
            assert!(row(key_y).starts_with("space"), "{size:?}");
            assert!(row(key_y - 1).trim().is_empty(), "{size:?}");
            assert!(!row(key_y - 2).trim().is_empty(), "{size:?}");
            assert_eq!(lines.len() as u16, key_y - 2 - (r.y + 3) + 1, "{size:?}");
        }
    }

    #[test]
    fn space_closes_the_feed_and_ends_its_step() {
        let mut st = state_home();
        super::super::begin(&mut st);
        let view = st.missions.feeds.pop_front().unwrap();
        let mut feed = Feed::new(view, st.tick);
        assert!(matches!(feed.update(&mut st, &[]), ScreenCmd::Stay));
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 0);
        let cmd = feed.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(matches!(cmd, ScreenCmd::Pop { .. }));
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 1);
    }

    #[test]
    fn the_explore_screen_opens_a_queued_feed() {
        let mut st = state_home();
        let mut map = MapScreen::walking(&mut st);
        super::super::begin(&mut st);
        assert_eq!(st.missions.feeds.len(), 1);
        let cmd = map.update(&mut st, &[]);
        assert!(matches!(cmd, ScreenCmd::Push { .. }));
        assert!(st.missions.feeds.is_empty());
    }
}
