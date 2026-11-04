//! The mail mark: a pill on the map as a mail lands, then a mark on the status line
//! until the inbox is read. Both are the main map's alone; it notices the inbox on its
//! own frames, so a mail that lands under the deck or a terminal shows once it is back.

use crate::app::AppState;
use crate::ui::chrome::MAGENTA;
use crate::ui::hud::{BADGE_FG, cut_spans};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Frames the pill stays up: six seconds of the map's frames on top, its corner clear.
pub const MAIL_PILL_TICKS: u64 = 180;
/// Cells between the pill's end and the map's right edge.
pub const PILL_INSET: u16 = 8;
/// The pill's sender and subject: light on a deep magenta.
const PILL_FG: Color = Color::Rgb(255, 225, 245);
const PILL_BG: Color = Color::Rgb(120, 20, 90);

/// The pill over the map: the mail it announces, and the clear map frames it has been
/// up on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MailPill {
    pub mail: usize,
    pub frames: u64,
}

/// The map's top rows are the pill's: no film scene framing the map, no call card and
/// no rest window over its top.
fn clear(st: &AppState) -> bool {
    let film = crate::missions::scene::running(st)
        .is_some_and(|s| s.look() == crate::missions::Look::Film);
    !film && st.missions.call.is_none() && st.rest.is_none()
}

/// A map frame on top: each mail landed since the last is logged, and the newest puts
/// the pill up. The pill counts the frames its corner is clear and comes down after
/// `MAIL_PILL_TICKS` of them, or as soon as its mail is read.
pub fn notice(st: &mut AppState) {
    let landed = st.mail_seen.min(st.journal.mails.len())..st.journal.mails.len();
    st.mail_seen = landed.end;
    for i in landed.clone() {
        let m = &st.journal.mails[i];
        let line = format!("`{}` · mail · {}", m.from, m.subject);
        st.story(line);
    }
    if let Some(mail) = landed.last() {
        st.mail_pill = Some(MailPill { mail, frames: 0 });
    }
    let Some(mut pill) = st.mail_pill else {
        return;
    };
    if clear(st) {
        pill.frames += 1;
    }
    let read = st.journal.mails.get(pill.mail).is_none_or(|m| m.read);
    st.mail_pill = (!read && pill.frames <= MAIL_PILL_TICKS).then_some(pill);
}

/// The pill as the map draws it, cut to `width` cells: `▣ MAIL` lit, then who wrote and
/// about what, and `+N` for the other mails unread. `None` before its first clear frame,
/// and while its corner is covered.
pub fn pill_line(st: &AppState, width: u16) -> Option<Line<'static>> {
    let pill = st.mail_pill.filter(|p| p.frames > 0 && clear(st))?;
    let m = st.journal.mails.get(pill.mail).filter(|m| !m.read)?;
    let others = unread(st) - 1;
    let more = if others > 0 {
        format!("  +{others}")
    } else {
        String::new()
    };
    let lit = Style::default()
        .fg(BADGE_FG)
        .bg(MAGENTA)
        .add_modifier(Modifier::BOLD);
    let soft = Style::default().fg(PILL_FG).bg(PILL_BG);
    let spans = vec![
        Span::styled(" ▣ MAIL ", lit),
        Span::styled(
            format!(" {} · {}{more} ", m.from.to_uppercase(), m.subject),
            soft,
        ),
    ];
    Some(Line::from(cut_spans(spans, width as usize)))
}

/// The unread mails the map has noticed; one landed since its last frame is not yet
/// among them.
pub fn unread(st: &AppState) -> usize {
    let seen = st.mail_seen.min(st.journal.mails.len());
    st.journal.mails[..seen].iter().filter(|m| !m.read).count()
}

#[cfg(test)]
pub(crate) mod testutil {
    use crate::app::AppState;

    /// A mail lands in the inbox, unread; its index.
    pub fn land(st: &mut AppState, from: &str, subject: &str) -> usize {
        st.journal.mails.push(crate::credentials::Mail {
            from: from.to_string(),
            subject: subject.to_string(),
            body: vec!["Saw the news.".to_string()],
            read: false,
            at: 0,
            job: None,
            story: None,
        });
        st.journal.mails.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::land;
    use super::*;

    /// The log's lines that announce a mail.
    fn announced(st: &AppState) -> usize {
        st.log
            .iter()
            .filter(|e| e.text.contains(" · mail · "))
            .count()
    }

    #[test]
    fn entering_a_city_announces_nothing() {
        let mut st = AppState::new();
        st.enter_city(citygen::City::lot(1));
        land(&mut st, "Mezz", "the news");
        land(&mut st, "Seren", "ten");
        notice(&mut st);
        assert_eq!(announced(&st), 2);
        assert!(st.mail_pill.is_some());
        // A new city, a new game or a load: the inbox, the count and the pill start over.
        st.enter_city(citygen::City::lot(2));
        assert_eq!((st.mail_seen, st.mail_pill), (0, None));
        for _ in 0..10 {
            st.tick += 1;
            notice(&mut st);
        }
        assert_eq!(announced(&st), 2, "nothing more is announced");
        assert!(st.mail_pill.is_none() && pill_line(&st, 80).is_none());
        assert_eq!(unread(&st), 0);
        // The first mail to land in the new city is new.
        let i = land(&mut st, "Mezz", "lay low");
        st.tick += 1;
        notice(&mut st);
        assert_eq!(announced(&st), 3);
        assert_eq!(st.mail_pill, Some(MailPill { mail: i, frames: 1 }));
    }

    #[test]
    fn the_pill_names_the_newest_and_counts_the_rest() {
        let mut st = AppState::new();
        land(&mut st, "Mezz", "the news");
        let ten = land(&mut st, "Seren", "ten");
        notice(&mut st);
        assert_eq!(st.mail_pill.map(|p| p.mail), Some(ten));
        let line = pill_line(&st, 80).expect("the pill is up");
        assert_eq!(line.to_string(), " ▣ MAIL  SEREN · ten  +1 ");
        // Cut to fit, never wider than it is given.
        let cut = pill_line(&st, 12).unwrap();
        assert_eq!(
            (cut.width(), cut.to_string()),
            (12, " ▣ MAIL  SE…".to_string())
        );
        // Read before it is down: it goes.
        crate::credentials::mail_read(&mut st, ten);
        st.tick += 1;
        notice(&mut st);
        assert!(st.mail_pill.is_none() && pill_line(&st, 80).is_none());
        assert_eq!(unread(&st), 1);
    }
}
