//! HELIX mail on the rig: the inbox and its mails, drawn on the CRT's glass. It reads
//! the journal's mails each time, so one that lands while it is open shows at once.

use crate::access::mail_time;
use crate::app::AppState;
use crate::credentials::{self, Mail};
use crate::ui::chrome;
use crate::ui::device::{CRT_H, CRT_W, Look};
use crate::ui::sidebar::wrap_text;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// What the glass shows: the inbox under its bar, or one mail open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// `sel` is the journal index under the bar, `top` the first row shown.
    Inbox { sel: usize, top: usize },
    /// `i` is the open mail's journal index, `scroll` the body rows scrolled past, `new`
    /// whether it was unread as it opened, `top` the inbox's, for esc to come back to.
    Reading {
        i: usize,
        scroll: usize,
        new: bool,
        top: usize,
    },
}

pub struct Mailer {
    pub view: View,
}

pub enum MailerEvent {
    Stay,
    Closed,
}

/// The real glass, as the terminal gives it: the CRT less its bezel.
const GLASS_W: usize = (CRT_W - 4) as usize;
const GLASS_H: usize = (CRT_H - 4) as usize;
/// Rows of mail a glass holds: less the title, a gap, the status and two key rows.
fn inbox_rows(h: usize) -> usize {
    h.saturating_sub(5)
}
/// Rows of body a glass holds: less the head block (three rows and a gap) too.
fn body_rows(h: usize) -> usize {
    h.saturating_sub(9)
}
/// The columns before `From`: the NEW mark and the time, with their gaps.
const HEAD: usize = 17;
const FROM: usize = 12;

/// `s` in at most `n` cells, ending `…` when cut.
fn fit(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut cut: String = s.chars().take(n.saturating_sub(1)).collect();
    if n > 0 {
        cut.push('…');
    }
    cut
}

/// An inbox row `w` cells wide: the subject is cut first, then `From`.
fn row(m: &Mail, w: usize) -> String {
    let mark = if m.read { "          " } else { "  ▸ NEW   " };
    let room = w.saturating_sub(HEAD);
    let from = fit(&m.from, room.min(FROM));
    let subject = room.saturating_sub(FROM + 2);
    if subject == 0 {
        return format!("{mark}{}  {from}", mail_time(m));
    }
    format!(
        "{mark}{}  {from:<FROM$}  {}",
        mail_time(m),
        fit(&m.subject, subject)
    )
}

/// A mail's body as the glass shows it: each line wrapped on its own, two in.
fn body(m: &Mail, w: usize) -> Vec<String> {
    m.body
        .iter()
        .flat_map(|l| wrap_text(l, w.saturating_sub(2)))
        .map(|l| format!("  {l}"))
        .collect()
}

/// The first row shown so that `row` is on a page of `page` rows from `top`.
fn shown_from(row: usize, top: usize, page: usize) -> usize {
    if row < top {
        row
    } else if row >= top + page {
        row + 1 - page
    } else {
        top
    }
}

/// The newest mail not yet read, by journal index.
fn newest_new(st: &AppState) -> Option<usize> {
    st.journal.mails.iter().rposition(|m| !m.read)
}

impl Mailer {
    /// The inbox, the bar on the newest new mail, else the newest; none without mail.
    pub fn open(st: &AppState) -> Option<Mailer> {
        let n = st.journal.mails.len();
        let sel = newest_new(st).or(n.checked_sub(1))?;
        let mut mailer = Mailer {
            view: View::Inbox { sel, top: 0 },
        };
        mailer.clamp(n);
        Some(mailer)
    }

    /// Keeps the bar's row on the inbox's page.
    fn clamp(&mut self, n: usize) {
        if let View::Inbox { sel, top } = &mut self.view {
            *top = shown_from(n.saturating_sub(*sel + 1), *top, inbox_rows(GLASS_H));
        }
    }

    /// Opens mail `i`, reading it: a story step it ends fires, and may land another.
    /// `top` is the inbox's, kept for esc.
    fn read(&mut self, st: &mut AppState, i: usize, top: usize) {
        let Some(m) = st.journal.mails.get(i) else {
            return;
        };
        let new = !m.read;
        credentials::mail_read(st, i);
        self.view = View::Reading {
            i,
            scroll: 0,
            new,
            top,
        };
    }

    pub fn key(&mut self, st: &mut AppState, k: KeyEvent) -> MailerEvent {
        let n = st.journal.mails.len();
        let next_new = matches!(k.code, KeyCode::Char('n') | KeyCode::Char('N'));
        match self.view {
            View::Inbox { sel, top } => match k.code {
                KeyCode::Esc => return MailerEvent::Closed,
                // the inbox lists newest first: up is the next mail in
                KeyCode::Up if sel + 1 < n => self.view = View::Inbox { sel: sel + 1, top },
                KeyCode::Down if sel > 0 => self.view = View::Inbox { sel: sel - 1, top },
                KeyCode::Char(' ') => self.read(st, sel, top),
                _ if next_new => {
                    if let Some(i) = newest_new(st) {
                        self.read(st, i, top);
                    }
                }
                _ => {}
            },
            View::Reading {
                i,
                scroll,
                new,
                top,
            } => match k.code {
                // back where the inbox was left, moved only as far as the bar needs
                KeyCode::Esc => self.view = View::Inbox { sel: i, top },
                KeyCode::Up if i + 1 < n => self.read(st, i + 1, top),
                KeyCode::Down if i > 0 => self.read(st, i - 1, top),
                KeyCode::PageUp | KeyCode::PageDown => {
                    let page = body_rows(GLASS_H);
                    let len = st
                        .journal
                        .mails
                        .get(i)
                        .map_or(0, |m| body(m, GLASS_W).len());
                    let scroll = if k.code == KeyCode::PageUp {
                        scroll.saturating_sub(page)
                    } else {
                        (scroll + page).min(len.saturating_sub(page))
                    };
                    self.view = View::Reading {
                        i,
                        scroll,
                        new,
                        top,
                    };
                }
                _ if next_new => {
                    if let Some(i) = newest_new(st) {
                        self.read(st, i, top);
                    }
                }
                _ => {}
            },
        }
        self.clamp(st.journal.mails.len());
        MailerEvent::Stay
    }

    pub fn draw(&self, st: &AppState, buf: &mut Buffer, glass: Rect, look: &Look) {
        if glass.width < 8 || glass.height < 6 {
            return;
        }
        let (x0, w, h) = (glass.x, glass.width as usize, glass.height as usize);
        let max_x = glass.right();
        let text = Style::default().fg(look.text()).bg(look.glass());
        let dim = Style::default().fg(look.text_dim()).bg(look.glass());
        let bar = Style::default()
            .fg(look.glass())
            .bg(look.text())
            .add_modifier(Modifier::BOLD);
        for y in glass.y..glass.bottom() {
            chrome::put(buf, x0, y, &" ".repeat(w), text, max_x);
        }
        let mails = &st.journal.mails;
        let n = mails.len();
        let new = mails.iter().filter(|m| !m.read).count();
        let first = glass.y + 2;
        let (title, status, keys) = match self.view {
            View::Inbox { sel, top } => {
                let page = inbox_rows(h);
                let at = n.checked_sub(sel + 1);
                let top = at.map_or(top, |r| shown_from(r, top, page));
                for (r, m) in mails.iter().rev().enumerate().skip(top).take(page) {
                    let y = first + (r - top) as u16;
                    let style = if at == Some(r) {
                        chrome::put(buf, x0, y, &" ".repeat(w), bar, max_x);
                        bar
                    } else if m.read {
                        text
                    } else {
                        text.add_modifier(Modifier::BOLD)
                    };
                    chrome::put(buf, x0, y, &row(m, w), style, max_x);
                }
                (
                    format!("  HELIX mail 2.2   INBOX · {n} mail · {new} new"),
                    if new == 0 {
                        "[ no new mail ]".to_string()
                    } else {
                        format!("[ {new} new ]")
                    },
                    ("space Read     n Next new    ↑↓ Move", "esc Exit"),
                )
            }
            View::Reading {
                i,
                scroll,
                new: was,
                ..
            } => {
                if let Some(m) = mails.get(i) {
                    let head = [
                        format!("  From:     {}", m.from),
                        format!("  Subject:  {}", m.subject),
                        format!("  Time:     {}", mail_time(m)),
                    ];
                    for (r, l) in head.iter().enumerate() {
                        chrome::put(buf, x0, first + r as u16, l, text, max_x);
                    }
                    let page = body_rows(h);
                    let lines = body(m, w);
                    let scroll = scroll.min(lines.len().saturating_sub(page));
                    for (r, l) in lines.iter().skip(scroll).take(page).enumerate() {
                        chrome::put(buf, x0, first + 4 + r as u16, l, text, max_x);
                    }
                }
                (
                    format!(
                        "  HELIX mail 2.2   {} of {n}{}",
                        i + 1,
                        if was { " · NEW" } else { "" }
                    ),
                    if new == 0 {
                        "[ no new mail ]".to_string()
                    } else {
                        format!("[ {new} new left ]")
                    },
                    ("n Next new     ↑↓ Other mail", "esc Inbox"),
                )
            }
        };
        chrome::put(buf, x0, glass.y, &" ".repeat(w), bar, max_x);
        chrome::put(buf, x0, glass.y, &title, bar, max_x);
        let status_y = glass.bottom() - 3;
        chrome::put(buf, x0, status_y, &format!("  {status}"), text, max_x);
        chrome::put(buf, x0, status_y + 1, keys.0, dim, max_x);
        chrome::put(buf, x0, status_y + 2, keys.1, dim, max_x);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::device::{self, Led};
    use crossterm::event::KeyModifiers;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn space() -> KeyEvent {
        k(KeyCode::Char(' '))
    }
    fn mail(from: &str, subject: &str, read: bool, at: u64) -> Mail {
        Mail {
            from: from.to_string(),
            subject: subject.to_string(),
            body: vec!["one line".to_string(), String::new(), "two".to_string()],
            read,
            at,
            job: None,
            story: None,
        }
    }
    /// The seed-8 home with these mails and no others.
    fn with(mails: Vec<Mail>) -> AppState {
        let mut st = crate::jobs::testutil::state_home();
        st.journal.mails = mails;
        st
    }
    /// The real glass: the CRT less its bezel.
    const GLASS: Rect = Rect::new(0, 0, 68, 20);
    fn drawn(m: &Mailer, st: &AppState) -> Buffer {
        let look = Look::named(st, "municipal", None, 1);
        let mut buf = Buffer::empty(GLASS);
        m.draw(st, &mut buf, GLASS, &look);
        buf
    }
    fn line_at(buf: &Buffer, y: u16) -> String {
        let a = buf.area;
        (a.x..a.right())
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
            .trim_end()
            .to_string()
    }
    fn rows(buf: &Buffer) -> Vec<String> {
        (buf.area.y..buf.area.bottom())
            .map(|y| line_at(buf, y))
            .collect()
    }
    /// The morning after: Mezz's news mail just in, every older mail read.
    fn mezz_just_in() -> (AppState, usize) {
        let mut st = crate::missions::tests::the_morning_call();
        crate::missions::tests::the_news_is_on(&mut st);
        crate::missions::news::switch_off(&mut st);
        let mezz = st
            .journal
            .mails
            .iter()
            .position(|m| m.subject == "the news")
            .expect("Mezz's mail");
        for (i, m) in st.journal.mails.iter_mut().enumerate() {
            m.read = i != mezz;
        }
        (st, mezz)
    }

    #[test]
    fn opens_on_the_newest_new_mail() {
        let st = with(vec![
            mail("a", "one", true, 600),
            mail("b", "two", false, 610),
            mail("c", "three", false, 620),
            mail("d", "four", true, 630),
        ]);
        let m = Mailer::open(&st).expect("an inbox");
        assert!(matches!(m.view, View::Inbox { sel: 2, .. }), "{:?}", m.view);
        let st = with(vec![
            mail("a", "one", true, 600),
            mail("b", "two", true, 610),
        ]);
        let m = Mailer::open(&st).expect("an inbox");
        assert!(matches!(m.view, View::Inbox { sel: 1, .. }), "the newest");
        assert!(Mailer::open(&with(Vec::new())).is_none());
    }

    #[test]
    fn space_reads_and_marks_read() {
        let (mut st, mezz) = mezz_just_in();
        let morning = 2;
        assert_eq!(
            st.missions.runs[morning]
                .objectives
                .last()
                .map(String::as_str),
            Some("Read Mezz's mail · on your rig")
        );
        let mut m = Mailer::open(&st).unwrap();
        assert!(matches!(m.view, View::Inbox { sel, .. } if sel == mezz));
        assert!(matches!(m.key(&mut st, space()), MailerEvent::Stay));
        assert_eq!(
            m.view,
            View::Reading {
                i: mezz,
                scroll: 0,
                new: true,
                top: 0
            }
        );
        assert!(st.journal.mails[mezz].read);
        // through `mail_read`: the step it was the mail of has ended
        assert_ne!(
            st.missions.runs[morning]
                .objectives
                .last()
                .map(String::as_str),
            Some("Read Mezz's mail · on your rig")
        );
        let n = st.journal.mails.len();
        let time = mail_time(&st.journal.mails[mezz]);
        let r = rows(&drawn(&m, &st));
        assert_eq!(
            r[0],
            format!("  HELIX mail 2.2   {} of {n} · NEW", mezz + 1)
        );
        assert_eq!(r[2], "  From:     Mezz");
        assert_eq!(r[3], "  Subject:  the news");
        assert_eq!(r[4], format!("  Time:     {time}"));
        assert_eq!(r[5], "");
        assert_eq!(r[6], "  Saw the news.");
        assert_eq!(r[7], "  Don't go out looking like you.");
        assert_eq!(r[18], "n Next new     ↑↓ Other mail");
        assert_eq!(r[19], "esc Inbox");
        // a mail read again opens without the NEW
        m.key(&mut st, k(KeyCode::Esc));
        m.key(&mut st, space());
        assert!(matches!(m.view, View::Reading { new: false, .. }));
        let r = rows(&drawn(&m, &st));
        assert_eq!(r[0], format!("  HELIX mail 2.2   {} of {n}", mezz + 1));
    }

    #[test]
    fn the_step_s_mail_lands_in_the_open_inbox() {
        let (mut st, mezz) = mezz_just_in();
        let n = st.journal.mails.len();
        let mut m = Mailer::open(&st).unwrap();
        m.key(&mut st, space());
        assert_eq!(st.journal.mails.len(), n + 1, "Seren's mail came");
        assert_eq!(st.journal.mails[n].subject, "ten");
        assert!(
            matches!(m.view, View::Reading { i, .. } if i == mezz),
            "pinned"
        );
        let r = rows(&drawn(&m, &st));
        assert_eq!(
            r[0],
            format!("  HELIX mail 2.2   {} of {} · NEW", mezz + 1, n + 1)
        );
        assert_eq!(r[17], "  [ 1 new left ]");
        // back in the inbox: hers on top, new, the bar still on Mezz's
        m.key(&mut st, k(KeyCode::Esc));
        assert!(matches!(m.view, View::Inbox { sel, .. } if sel == mezz));
        let buf = drawn(&m, &st);
        let r = rows(&buf);
        assert_eq!(
            r[0],
            format!("  HELIX mail 2.2   INBOX · {} mail · 1 new", n + 1)
        );
        let seren = mail_time(&st.journal.mails[n]);
        assert_eq!(r[2], format!("  ▸ NEW   {seren}  Seren         ten"));
        let time = mail_time(&st.journal.mails[mezz]);
        assert_eq!(r[3], format!("          {time}  Mezz          the news"));
        let look = Look::named(&st, "municipal", None, 1);
        assert_eq!(buf[(0, 3)].bg, look.text(), "the bar on Mezz's row");
        assert_eq!(buf[(0, 2)].bg, look.glass());
        assert_eq!(r[17], "  [ 1 new ]");
    }

    #[test]
    fn n_reads_the_newest_new_and_says_when_none() {
        let mut st = with(vec![
            mail("a", "one", false, 600),
            mail("b", "two", true, 610),
            mail("c", "three", false, 620),
            mail("d", "four", true, 630),
        ]);
        let mut m = Mailer::open(&st).unwrap();
        m.key(&mut st, k(KeyCode::Down));
        assert!(matches!(m.view, View::Inbox { sel: 1, .. }));
        m.key(&mut st, k(KeyCode::Char('n')));
        assert!(matches!(
            m.view,
            View::Reading {
                i: 2,
                new: true,
                ..
            }
        ));
        assert!(st.journal.mails[2].read);
        assert_eq!(rows(&drawn(&m, &st))[17], "  [ 1 new left ]");
        m.key(&mut st, k(KeyCode::Char('n')));
        assert!(matches!(
            m.view,
            View::Reading {
                i: 0,
                new: true,
                ..
            }
        ));
        assert_eq!(rows(&drawn(&m, &st))[17], "  [ no new mail ]");
        // none left: no read, the mail stays open, the status says so
        let before = m.view;
        m.key(&mut st, k(KeyCode::Char('n')));
        assert_eq!(m.view, before);
        assert_eq!(rows(&drawn(&m, &st))[17], "  [ no new mail ]");
        m.key(&mut st, k(KeyCode::Esc));
        m.key(&mut st, k(KeyCode::Char('n')));
        assert!(matches!(m.view, View::Inbox { sel: 0, .. }), "{:?}", m.view);
        assert_eq!(rows(&drawn(&m, &st))[17], "  [ no new mail ]");
    }

    #[test]
    fn arrows_stop_at_the_ends_and_scroll_a_long_inbox() {
        let mut st = with(
            (1..=30)
                .map(|i| mail("a", &format!("mail {i:02}"), true, 600 + i))
                .collect(),
        );
        let mut m = Mailer::open(&st).unwrap();
        assert!(matches!(m.view, View::Inbox { sel: 29, .. }));
        m.key(&mut st, k(KeyCode::Up));
        assert!(
            matches!(m.view, View::Inbox { sel: 29, top: 0 }),
            "the top stops"
        );
        let r = rows(&drawn(&m, &st));
        assert!(r[2].ends_with("mail 30"), "{r:?}");
        assert!(r[16].ends_with("mail 16"), "fifteen rows: {r:?}");
        assert_eq!(r[17], "  [ no new mail ]");
        for _ in 0..40 {
            m.key(&mut st, k(KeyCode::Down));
        }
        assert!(
            matches!(m.view, View::Inbox { sel: 0, top: 15 }),
            "{:?}",
            m.view
        );
        let buf = drawn(&m, &st);
        let r = rows(&buf);
        assert!(r[16].ends_with("mail 01"), "the bar's row shows: {r:?}");
        assert!(r[2].ends_with("mail 15"), "{r:?}");
        assert!(!r.iter().any(|l| l.ends_with("mail 30")), "{r:?}");
        let look = Look::named(&st, "municipal", None, 1);
        assert_eq!(buf[(0, 16)].bg, look.text(), "the bar on the last row");
        // up again: the bar climbs, the list stays until the bar leaves it
        m.key(&mut st, k(KeyCode::Up));
        assert!(matches!(m.view, View::Inbox { sel: 1, top: 15 }));
        // the open mail's arrows stop at the ends too
        m.key(&mut st, k(KeyCode::Down));
        m.key(&mut st, space());
        m.key(&mut st, k(KeyCode::Down));
        assert!(
            matches!(m.view, View::Reading { i: 0, .. }),
            "the oldest stops"
        );
        m.key(&mut st, k(KeyCode::Up));
        assert!(
            matches!(m.view, View::Reading { i: 1, .. }),
            "the next one up"
        );
        for _ in 0..40 {
            m.key(&mut st, k(KeyCode::Up));
        }
        assert!(
            matches!(m.view, View::Reading { i: 29, .. }),
            "the newest stops"
        );
    }

    #[test]
    fn esc_brings_the_inbox_back_where_it_was() {
        let mut st = with(
            (1..=30)
                .map(|i| mail("a", &format!("mail {i:02}"), true, 600 + i))
                .collect(),
        );
        let mut m = Mailer::open(&st).unwrap();
        for _ in 0..20 {
            m.key(&mut st, k(KeyCode::Down));
        }
        for _ in 0..3 {
            m.key(&mut st, k(KeyCode::Up));
        }
        assert!(
            matches!(m.view, View::Inbox { sel: 12, top: 6 }),
            "{:?}",
            m.view
        );
        m.key(&mut st, space());
        m.key(&mut st, k(KeyCode::Esc));
        assert!(
            matches!(m.view, View::Inbox { sel: 12, top: 6 }),
            "{:?}",
            m.view
        );
        // a mail left for a neighbour still on the page: the bar on it, the list as it was
        m.key(&mut st, space());
        m.key(&mut st, k(KeyCode::Down));
        m.key(&mut st, k(KeyCode::Esc));
        assert!(
            matches!(m.view, View::Inbox { sel: 11, top: 6 }),
            "{:?}",
            m.view
        );
        // one off the page: the list moves just enough to show the bar
        m.key(&mut st, space());
        for _ in 0..13 {
            m.key(&mut st, k(KeyCode::Up));
        }
        m.key(&mut st, k(KeyCode::Esc));
        assert!(
            matches!(m.view, View::Inbox { sel: 24, top: 5 }),
            "{:?}",
            m.view
        );
    }

    #[test]
    fn page_keys_scroll_a_long_body() {
        let mut long = mail("a", "long", false, 600);
        long.body = (1..=30).map(|i| format!("line {i:02}")).collect();
        let mut st = with(vec![long]);
        let mut m = Mailer::open(&st).unwrap();
        m.key(&mut st, space());
        let r = rows(&drawn(&m, &st));
        assert_eq!(r[6], "  line 01");
        assert_eq!(r[16], "  line 11", "eleven body rows: {r:?}");
        m.key(&mut st, k(KeyCode::PageDown));
        let r = rows(&drawn(&m, &st));
        assert_eq!(r[6], "  line 12", "{r:?}");
        assert_eq!(r[2], "  From:     a", "the head stays");
        for _ in 0..5 {
            m.key(&mut st, k(KeyCode::PageDown));
        }
        assert!(
            rows(&drawn(&m, &st))[16].ends_with("line 30"),
            "the end stops"
        );
        m.key(&mut st, k(KeyCode::PageUp));
        m.key(&mut st, k(KeyCode::PageUp));
        m.key(&mut st, k(KeyCode::PageUp));
        assert_eq!(rows(&drawn(&m, &st))[6], "  line 01");
    }

    #[test]
    fn esc_backs_out_to_the_inbox_then_closes() {
        let mut st = with(vec![
            mail("a", "one", false, 600),
            mail("b", "two", false, 610),
            mail("c", "three", false, 620),
        ]);
        let mut m = Mailer::open(&st).unwrap();
        m.key(&mut st, k(KeyCode::Down));
        // enter does nothing, here or in a mail
        assert!(matches!(
            m.key(&mut st, k(KeyCode::Enter)),
            MailerEvent::Stay
        ));
        assert!(matches!(m.view, View::Inbox { sel: 1, .. }));
        assert!(!st.journal.mails[1].read);
        m.key(&mut st, space());
        assert!(matches!(m.view, View::Reading { i: 1, .. }));
        m.key(&mut st, k(KeyCode::Enter));
        assert!(matches!(m.view, View::Reading { i: 1, .. }));
        assert!(matches!(m.key(&mut st, k(KeyCode::Esc)), MailerEvent::Stay));
        assert!(
            matches!(m.view, View::Inbox { sel: 1, .. }),
            "the bar on it"
        );
        assert!(matches!(
            m.key(&mut st, k(KeyCode::Esc)),
            MailerEvent::Closed
        ));
    }

    #[test]
    fn the_inbox_draws_its_rows_on_the_glass() {
        let mails = vec![
            mail("Teodora-11", "a favour", true, 1320),
            mail("Mezz", "voicemail", true, 1321),
            mail("Mezz", "lay low", true, 1323),
            mail("Mezz", "the news", false, 1803),
        ];
        let times: Vec<String> = mails.iter().map(mail_time).collect();
        let mut st = with(mails);
        let mut m = Mailer::open(&st).unwrap();
        let look = Look::named(&st, "municipal", None, 1);
        let buf = drawn(&m, &st);
        let r = rows(&buf);
        assert_eq!(r[0], "  HELIX mail 2.2   INBOX · 4 mail · 1 new");
        assert_eq!(r[1], "");
        assert_eq!(
            r[2],
            format!("  ▸ NEW   {}  Mezz          the news", times[3])
        );
        assert_eq!(
            r[3],
            format!("          {}  Mezz          lay low", times[2])
        );
        assert_eq!(
            r[4],
            format!("          {}  Mezz          voicemail", times[1])
        );
        assert_eq!(
            r[5],
            format!("          {}  Teodora-11    a favour", times[0])
        );
        assert_eq!(r[17], "  [ 1 new ]");
        assert_eq!(r[18], "space Read     n Next new    ↑↓ Move");
        assert_eq!(r[19], "esc Exit");
        // the title bar and the cursor bar are inverse, the whole glass wide
        for y in [0, 2] {
            for x in [0, 30, 67] {
                assert_eq!(buf[(x, y)].bg, look.text(), "({x},{y})");
                assert_eq!(buf[(x, y)].fg, look.glass(), "({x},{y})");
            }
        }
        assert_eq!(buf[(0, 3)].bg, look.glass());
        assert_eq!(buf[(0, 3)].fg, look.text());
        assert_eq!(buf[(0, 18)].fg, look.text_dim(), "the key rows are dim");
        // off the bar, a new row is bold and a read one is not
        m.key(&mut st, k(KeyCode::Down));
        let buf = drawn(&m, &st);
        assert_eq!(buf[(2, 2)].bg, look.glass());
        assert!(buf[(2, 2)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(2, 2)].symbol(), "▸");
        assert!(!buf[(12, 4)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn it_fits_the_100x30_glass() {
        let mut st = with(vec![
            mail("Mezz", "short", true, 600),
            mail(
                "Teodora-11-of-the-long-handle",
                &"a very long subject ".repeat(5),
                false,
                610,
            ),
        ]);
        st.journal.mails[1].body = vec!["word ".repeat(40)];
        let look = Look::named(&st, "municipal", None, 1);
        let frame = Rect::new(0, 0, 100, 30);
        let r = crate::screens::terminal::Terminal::rect(frame);
        let mut buf = Buffer::empty(frame);
        let glass = device::crt(&mut buf, r, "TEST", Led::Idle, &look);
        let bare = buf.clone();
        let mut m = Mailer::open(&st).unwrap();
        let outside = |a: &Buffer, b: &Buffer| {
            for y in 0..frame.height {
                for x in 0..frame.width {
                    let inside =
                        x >= glass.x && x < glass.right() && y >= glass.y && y < glass.bottom();
                    if !inside {
                        assert_eq!(a[(x, y)], b[(x, y)], "({x},{y}) is not the glass");
                    }
                }
            }
        };
        let in_glass = |buf: &Buffer, y: u16| -> String {
            (glass.x..glass.right())
                .map(|x| buf[(x, glass.y + y)].symbol().to_string())
                .collect()
        };
        m.draw(&st, &mut buf, glass, &look);
        outside(&buf, &bare);
        assert!(in_glass(&buf, 0).starts_with("  HELIX mail 2.2   INBOX · 2 mail · 1 new"));
        // the From column cut to its twelve, the subject cut to the glass
        let top = in_glass(&buf, 2);
        assert!(top.contains("  Teodora-11-…  a very long"), "{top}");
        assert!(top.ends_with('…'), "{top}");
        // the open mail wraps its body inside the glass at word breaks, two in
        let mut buf = bare.clone();
        m.key(&mut st, space());
        m.draw(&st, &mut buf, glass, &look);
        outside(&buf, &bare);
        let body: Vec<String> = (6..10).map(|y| in_glass(&buf, y)).collect();
        assert!(body.iter().all(|l| l.starts_with("  word")), "{body:?}");
        let words: Vec<&str> = body.iter().flat_map(|l| l.split_whitespace()).collect();
        assert_eq!(words.len(), 40, "{body:?}");
        assert!(words.iter().all(|w| *w == "word"), "{body:?}");
        // a glass narrower than the columns cuts the subject first, then From (the
        // older mail's row, under the bar's)
        let narrow = |st: &AppState, w: u16| {
            let g = Rect::new(0, 0, w, 20);
            let mut buf = Buffer::empty(g);
            let m = Mailer::open(st).unwrap();
            m.draw(st, &mut buf, g, &look);
            line_at(&buf, 3)
        };
        st.journal.mails[0].subject = "a long subject".to_string();
        let time = mail_time(&st.journal.mails[0]);
        assert_eq!(
            narrow(&st, 36),
            format!("          {time}  Mezz          a lo…")
        );
        assert_eq!(narrow(&st, 31), format!("          {time}  Mezz"));
        st.journal.mails[0].from = "Teodora-11".to_string();
        assert_eq!(narrow(&st, 24), format!("          {time}  Teodor…"));
    }
}
