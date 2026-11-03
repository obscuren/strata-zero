//! Readings (doctor spec §3): what who you are finds in an item at the rig, at the
//! bench or by its Look, and the HOLO card the rig and the bench show it on.

use super::{ReadAt, Reading, Row, face, fill, holds};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::chrome::{CYAN, DIM, FAINT, MAGENTA, TEXT, put};
use crate::ui::hud::AMBER;
use citygen::rng::hash64;
use crossterm::event::{KeyCode, KeyEvent};
use game::ItemId;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// The widest the card gets.
const MAX_W: u16 = 66;
/// Rows of the card that are not its rows: the frame, the header and the gap under it,
/// the two blank rows and the key row at the foot.
const CHROME_H: u16 = 7;
/// Columns a field's label takes, its gap included.
const GUTTER: usize = 8;
/// Columns a ledger entry's first bar takes, its gap included.
const BAR_W: usize = 9;

/// The first reading of the item at `at` whose need holds now.
pub fn reading<'a>(st: &'a AppState, item: &str, at: ReadAt) -> Option<&'a Reading> {
    st.book
        .readings
        .get(item)?
        .iter()
        .find(|r| r.at == at && holds(st, None, &r.need))
}

/// What the item's Look and its card say: the first Look reading that holds, filled,
/// else the item's own description.
pub fn item_desc(st: &AppState, id: &ItemId) -> String {
    match reading(st, &id.0, ReadAt::Look) {
        Some(r) => fill(st, &r.text),
        None => st
            .content
            .item(id)
            .map(|i| i.desc.clone())
            .unwrap_or_default(),
    }
}

/// The pack shows the item's card: a Look reading that tells, when it holds, records the
/// item read.
pub fn look_tells(st: &mut AppState, item: &ItemId) {
    if reading(st, &item.0, ReadAt::Look).is_some_and(|r| r.tells) {
        st.missions.read.insert(item.0.clone());
    }
}

/// A reading on the screen, over the map at the rig or the deck at the bench: the HOLO
/// frame of the feed, its rows filled when it opened.
pub struct ReadCard {
    /// `HOLO` at the rig, `BENCH` at the bench.
    label: &'static str,
    head: String,
    lines: Vec<Line<'static>>,
    close: String,
}

impl ReadCard {
    /// The card of the item's reading at `at`; showing one that tells records the item
    /// read. `None` when no reading there holds, and at a Look, which has no card.
    pub fn open(st: &mut AppState, item: &ItemId, at: ReadAt) -> Option<ReadCard> {
        let label = match at {
            ReadAt::Rig => "HOLO",
            ReadAt::Bench => "BENCH",
            ReadAt::Look => return None,
        };
        let r = reading(st, &item.0, at)?;
        let tells = r.tells;
        let card = ReadCard {
            label,
            head: fill(st, &r.head),
            lines: r.rows.iter().flat_map(|row| lines(st, item, row)).collect(),
            close: fill(st, &r.close),
        };
        if tells {
            st.missions.read.insert(item.0.clone());
        }
        Some(card)
    }

    /// The card's box in `frame`: sized to its rows, at most `MAX_W` wide, centred two
    /// rows above the middle.
    fn area(&self, frame: Rect) -> Rect {
        let w = MAX_W.min(frame.width.saturating_sub(4));
        let h = (self.lines.len() as u16)
            .saturating_add(CHROME_H)
            .min(frame.height.saturating_sub(2));
        let x = frame.x + (frame.width - w) / 2;
        let y = frame.y + ((frame.height - h) / 2).saturating_sub(2);
        Rect::new(x, y, w, h)
    }
}

fn plain(c: Color) -> Style {
    Style::default().fg(c)
}

fn bold(c: Color) -> Style {
    plain(c).add_modifier(Modifier::BOLD)
}

/// What one row draws, its texts filled: one line, or a ledger's entries and how many
/// more it holds.
fn lines(st: &AppState, item: &ItemId, row: &Row) -> Vec<Line<'static>> {
    let line = match row {
        Row::Field(label, text) => {
            let label = fill(st, label);
            let w = GUTTER.max(label.chars().count() + 1);
            let mut spans = vec![Span::styled(format!("{label:<w$}"), plain(DIM))];
            spans.extend(marked(&fill(st, text), plain(TEXT)));
            Line::from(spans)
        }
        Row::Hot(text) => Line::from(Span::styled(fill(st, text), plain(MAGENTA))),
        Row::Tag(text) => Line::from(Span::styled(
            format!("▚▞ {}", fill(st, text)),
            bold(MAGENTA),
        )),
        Row::Art(text) => Line::from(marked(&fill(st, text), plain(CYAN))),
        Row::You(text) => Line::from(vec![
            Span::styled("you  ", plain(DIM)),
            Span::styled(fill(st, text), plain(TEXT)),
        ]),
        Row::Ledger(shown, total) => return ledger(st, item, *shown, *total),
        Row::Blank => Line::default(),
    };
    vec![line]
}

/// `text` in `base`, its backticked spans AMBER bold and the backticks gone.
fn marked(text: &str, base: Style) -> Vec<Span<'static>> {
    text.split('`')
        .enumerate()
        .filter(|(_, s)| !s.is_empty())
        .map(|(i, s)| {
            let style = if i % 2 == 1 { bold(AMBER) } else { base };
            Span::styled(s.to_string(), style)
        })
        .collect()
}

/// `shown` entries of a ledger of `total`, the same each time for the city and the
/// item, in date order: the date, the amount, a name blacked out, then a district of the
/// city on every other entry and a second bar on the rest; then how many more it holds.
fn ledger(st: &AppState, item: &ItemId, shown: u32, total: u32) -> Vec<Line<'static>> {
    let city = st.city.as_ref();
    let seed = item
        .0
        .bytes()
        .fold(city.map_or(0, |c| c.seed), |h, b| hash64(h, u64::from(b)));
    let mut entries: Vec<(u64, u64, u64)> = (0..u64::from(shown))
        .map(|i| {
            let h = hash64(seed, i);
            (1 + h % 12, 1 + (h >> 8) % 28, h)
        })
        .collect();
    entries.sort_unstable();
    let districts = city.map_or(&[][..], |c| c.districts.as_slice());
    let mut out: Vec<Line<'static>> = entries
        .iter()
        .enumerate()
        .map(|(j, &(month, day, h))| {
            let amount = [10, 100, 1000][((h >> 16) % 3) as usize] * (1 + (h >> 24) % 99);
            let bar = "▓".repeat(4 + ((h >> 32) % 5) as usize);
            let place = districts
                .get(((h >> 40) % districts.len().max(1) as u64) as usize)
                .filter(|_| j % 2 == 0);
            let tail = match place {
                Some(d) => Span::styled(d.name.clone(), plain(DIM)),
                None => Span::styled("▓".repeat(5 + ((h >> 48) % 6) as usize), plain(FAINT)),
            };
            Line::from(vec![
                Span::styled(
                    format!("{month:02}.{day:02}  ¤{:>6}  ", grouped(amount)),
                    plain(TEXT),
                ),
                Span::styled(format!("{bar:<BAR_W$}"), plain(FAINT)),
                tail,
            ])
        })
        .collect();
    if total > shown {
        out.push(Line::from(Span::styled(
            format!("  …{} more", total - shown),
            plain(FAINT),
        )));
    }
    out
}

/// An amount with a space before each three digits from the right: `12 500`.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

impl Screen for ReadCard {
    fn update(&mut self, _st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if keys
            .iter()
            .any(|k| matches!(k.code, KeyCode::Char(' ') | KeyCode::Esc))
        {
            return ScreenCmd::Pop {
                fx: TransitionKind::None,
            };
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let r = self.area(f.area());
        if r.width < 24 || r.height < CHROME_H {
            return;
        }
        let buf = f.buffer_mut();
        face::bracket_frame(buf, r);
        // Text stops short of the frame; the header's title short of the signal.
        let (right, inner) = (r.right(), r.right() - 2);
        put(buf, r.x + 3, r.y + 1, self.label, bold(MAGENTA), right);
        let head_x = r.x + 4 + self.label.len() as u16;
        let head = format!("▸ {}", self.head);
        put(buf, head_x, r.y + 1, &head, plain(DIM), right - 17);
        put(buf, right - 16, r.y + 1, "▂▄▆█", plain(MAGENTA), right);
        let (h, m) = st.clock();
        let clock = format!("{h:02}:{m:02}");
        put(buf, right - 11, r.y + 1, &clock, plain(DIM), right);
        let room = usize::from(r.height - CHROME_H);
        for (j, line) in self.lines.iter().take(room).enumerate() {
            let y = r.y + 3 + j as u16;
            let mut x = r.x + 3;
            for span in &line.spans {
                put(buf, x, y, &span.content, span.style, inner);
                x = x.saturating_add(span.content.chars().count() as u16);
            }
        }
        let key_y = r.bottom() - 2;
        put(buf, r.x + 3, key_y, "space", bold(MAGENTA), right);
        put(buf, r.x + 9, key_y, &self.close, plain(DIM), inner);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::missions::Book;
    use crate::screens::shots::draw_stack;
    use crate::screens::testutil::{buffer_text, key};
    use crate::ui::chrome::FAINT;
    use crate::ui::hud::AMBER;
    use crossterm::event::KeyCode;
    use game::SkillId;
    use ratatui::style::Modifier;

    /// The shard's readings: its card at the rig and at the bench, two Looks.
    const READINGS: &str = r#""ledger_shard": [
        Reading(at: Rig, need: Skill("hacking"), verb: "Read", head: "shard · read at the rig",
            close: "pull it", tells: true, rows: [
            Field("LEDGER", "214 entries"),
            Field("owner", "`{fence}` · fixer"),
            Hot("ICE     open"),
            Blank,
            Ledger(4, 214),
            Blank,
            Tag("too slow. —S"),
        ]),
        Reading(at: Bench, need: Skill("electronics"), verb: "Open the casing",
            head: "shard · casing open", close: "close the casing", rows: [
            Art("  ╰── `{fence.spaced}` ──╯"),
            Blank,
            You("{fence}. Fixers sign their shards."),
        ]),
        Reading(at: Look, need: Skill("streetwise"),
            text: "Tape round it. {fence} wraps his like that."),
        Reading(at: Look, need: Skill("hacking"), text: "Its ICE is cracked."),
    ]"#;

    /// A book whose one role, `fence`, nobody plays, with the shard's readings.
    pub(crate) fn book() -> Book {
        Book::parse(&format!(
            r#"Story(
                cast: {{
                    "fence": Role(name: "Ruko", title: "fixer", pick: NearestFixer, face: None),
                }},
                faces: {{
                    "silhouette": Face(rows: ["................", "................"], eye: (0, 0, 0)),
                }},
                missions: [Mission(id: "m", title: "M", from: "fence", steps: [
                    Step(id: "a", when: NewGame, then: [Log("hi")]),
                ])],
                readings: {{ {READINGS} }},
            )"#
        ))
        .unwrap()
    }

    /// `st` reading from the test book with these skills and no others, one shard carried.
    pub(crate) fn reader(mut st: AppState, skills: &[&str]) -> AppState {
        st.book = book();
        st.character.skills = skills.iter().map(|s| SkillId::new(s)).collect();
        st.character.give(ItemId::new("ledger_shard"), 1);
        st
    }

    fn shard() -> ItemId {
        ItemId::new("ledger_shard")
    }

    #[test]
    fn the_first_reading_whose_need_holds_is_used() {
        let mut st = reader(AppState::new(), &["streetwise", "hacking"]);
        let look =
            |st: &AppState| reading(st, "ledger_shard", ReadAt::Look).map(|r| r.text.clone());
        assert_eq!(
            look(&st).as_deref(),
            Some("Tape round it. {fence} wraps his like that.")
        );
        st.character.skills.remove(&SkillId::new("streetwise"));
        assert_eq!(look(&st).as_deref(), Some("Its ICE is cracked."));
        st.character.skills.clear();
        assert_eq!(look(&st), None);
        // Each place has its own: hacking reads at the rig, not at the bench.
        st.character.skills.insert(SkillId::new("hacking"));
        let rig = reading(&st, "ledger_shard", ReadAt::Rig).unwrap();
        assert_eq!(rig.verb, "Read");
        assert!(reading(&st, "ledger_shard", ReadAt::Bench).is_none());
        assert!(reading(&st, "taped_package", ReadAt::Look).is_none());
    }

    #[test]
    fn item_desc_falls_back_to_the_description() {
        let mut st = reader(AppState::new(), &[]);
        let own = st.content.item(&shard()).unwrap().desc.clone();
        assert_eq!(item_desc(&st, &shard()), own);
        st.character.skills.insert(SkillId::new("streetwise"));
        let read = "Tape round it. Ruko wraps his like that.";
        assert_eq!(item_desc(&st, &shard()), read);
        // The pack's card says it too.
        let card: String = crate::screens::deck::Deck::item_card(&st, &shard())
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(card.contains(read) && !card.contains(&own), "{card}");
        // An item with no readings, and one the content lacks.
        let coil = ItemId::new("copper_coil");
        let coil_desc = st.content.item(&coil).unwrap().desc.clone();
        assert_eq!(item_desc(&st, &coil), coil_desc);
        assert_eq!(item_desc(&st, &ItemId::new("no_such_item")), "");
    }

    #[test]
    fn a_telling_card_records_the_read() {
        let mut st = reader(AppState::new(), &["hacking", "electronics"]);
        // The bench's reading doesn't tell: nothing is recorded.
        assert!(ReadCard::open(&mut st, &shard(), ReadAt::Bench).is_some());
        assert!(st.missions.read.is_empty());
        assert!(ReadCard::open(&mut st, &shard(), ReadAt::Rig).is_some());
        assert!(st.missions.read.contains("ledger_shard"));
        assert!(super::super::holds(
            &st,
            None,
            &crate::missions::Need::Read("ledger_shard".into())
        ));
        // No reading there holds, or a Look has no card: nothing opens.
        st.character.skills.clear();
        st.missions.read.clear();
        assert!(ReadCard::open(&mut st, &shard(), ReadAt::Rig).is_none());
        assert!(ReadCard::open(&mut st, &shard(), ReadAt::Look).is_none());
        assert!(st.missions.read.is_empty());
    }

    #[test]
    fn a_telling_look_is_read_when_its_card_shows() {
        use crate::screens::{deck::Deck, testutil::key};
        let telling = |streetwise_tells: bool, skills: &[&str]| {
            let mut st = reader(AppState::new(), skills);
            if !streetwise_tells {
                for rs in st.book.readings.values_mut() {
                    rs.iter_mut().for_each(|r| r.tells = false);
                }
            }
            for rs in st.book.readings.values_mut() {
                for r in rs.iter_mut().filter(|r| r.at == ReadAt::Look) {
                    r.tells = r.tells || streetwise_tells;
                }
            }
            let mut d = Deck::new(st.tick);
            d.update(&mut st, &[key(KeyCode::Char('3'))]);
            // The pack's cursor on the shard.
            let rows = d.kit.pack_rows(&st);
            d.kit.state.pane = crate::screens::kit::Pane::Pack;
            d.kit.state.row = rows
                .iter()
                .position(|r| {
                    matches!(&r.detail, Some(crate::screens::deck::Detail::Item(i)) if *i == shard())
                })
                .unwrap();
            assert!(st.missions.read.is_empty(), "nothing before the update");
            d.update(&mut st, &[]);
            st.missions.read.contains("ledger_shard")
        };
        // A telling Look whose need holds.
        assert!(telling(true, &["streetwise"]));
        // One that doesn't tell.
        assert!(!telling(false, &["streetwise"]));
        // One whose need doesn't hold.
        assert!(!telling(true, &[]));
    }

    #[test]
    fn space_and_esc_close_the_card() {
        let mut st = reader(AppState::new(), &["hacking"]);
        let mut card = ReadCard::open(&mut st, &shard(), ReadAt::Rig).unwrap();
        assert!(matches!(card.update(&mut st, &[]), ScreenCmd::Stay));
        let enter = card.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(matches!(enter, ScreenCmd::Stay));
        for k in [KeyCode::Char(' '), KeyCode::Esc] {
            assert!(matches!(
                card.update(&mut st, &[key(k)]),
                ScreenCmd::Pop { .. }
            ));
        }
    }

    /// The screen's rows as text, top to bottom, trimmed of the card's sides.
    fn rows(buf: &ratatui::buffer::Buffer) -> Vec<String> {
        buffer_text(buf)
            .lines()
            .map(|l| l.trim().trim_matches(['▌', '▐']).trim().to_string())
            .collect()
    }

    #[test]
    fn the_card_draws_its_rows() {
        let mut st = reader(state_home(), &["hacking", "electronics"]);
        let districts: Vec<String> = st
            .city
            .as_ref()
            .unwrap()
            .districts
            .iter()
            .map(|d| d.name.clone())
            .collect();
        for size in [(120, 40), (100, 30)] {
            let rig = ReadCard::open(&mut st, &shard(), ReadAt::Rig).unwrap();
            let buf = draw_stack(&st, &[&rig], size);
            let text = buffer_text(&buf);
            for want in [
                "HOLO",
                "▸ shard · read at the rig",
                "▂▄▆█",
                "LEDGER  214 entries",
                "owner   Ruko · fixer",
                "▚▞ too slow. —S",
                "space pull it",
                "…210 more",
            ] {
                assert!(text.contains(want), "{size:?}: {want:?} missing\n{text}");
            }
            let (h, m) = st.clock();
            assert!(text.contains(&format!("{h:02}:{m:02}")), "{size:?}");
            assert!(!text.contains('`'), "{size:?}: a backtick drawn");
            // The role's name stands out.
            let name = (0..buf.area.height)
                .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
                .find(|&(x, y)| buf[(x, y)].symbol() == "R" && buf[(x + 1, y)].symbol() == "u")
                .unwrap();
            assert_eq!(buf[name].fg, AMBER);
            assert!(buf[name].modifier.contains(Modifier::BOLD));
            // The ledger: as many entries as it shows, dated, and every other one placed.
            let entries: Vec<String> = rows(&buf).into_iter().filter(|r| r.contains('¤')).collect();
            assert_eq!(entries.len(), 4, "{size:?}: {entries:?}");
            for (i, e) in entries.iter().enumerate() {
                let (month, day) = (&e[0..2], &e[3..5]);
                assert!(e.as_bytes()[2] == b'.', "{e}");
                assert!(
                    ("01"..="12").contains(&month) && ("01"..="28").contains(&day),
                    "{e}"
                );
                let placed = districts.iter().any(|d| e.ends_with(d.as_str()));
                assert_eq!(placed, i % 2 == 0, "{e}");
            }
            let mut dates: Vec<&str> = entries.iter().map(|e| &e[0..5]).collect();
            let shown = dates.clone();
            dates.sort();
            assert_eq!(dates, shown, "in date order");
            let bar = (0..buf.area.height)
                .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
                .find(|&p| buf[p].symbol() == "▓")
                .unwrap();
            assert_eq!(buf[bar].fg, FAINT);
            // The same city and item read the same ledger.
            let again = ReadCard::open(&mut st, &shard(), ReadAt::Rig).unwrap();
            assert_eq!(buffer_text(&draw_stack(&st, &[&again], size)), text);
            // The bench's card.
            let bench = ReadCard::open(&mut st, &shard(), ReadAt::Bench).unwrap();
            let text = buffer_text(&draw_stack(&st, &[&bench], size));
            for want in [
                "BENCH",
                "▸ shard · casing open",
                "╰── R U K O ──╯",
                "you  Ruko. Fixers sign their shards.",
                "space close the casing",
            ] {
                assert!(text.contains(want), "{size:?}: {want:?} missing\n{text}");
            }
            assert!(!text.contains("HOLO"), "{size:?}");
        }
    }

    #[test]
    fn the_card_sits_two_rows_above_the_middle_sized_to_its_rows() {
        let mut st = reader(state_home(), &["hacking"]);
        let rig = ReadCard::open(&mut st, &shard(), ReadAt::Rig).unwrap();
        let buf = draw_stack(&st, &[&rig], (120, 40));
        let rows = rows(&buf);
        let at = |s: &str| rows.iter().position(|r| r.contains(s)).unwrap();
        // 11 rows of reading and 7 of frame: 18 high, from row 9 (40 − 18 = 22, 11 − 2).
        assert_eq!(at("HOLO"), 10);
        assert_eq!(at("LEDGER"), 12);
        assert_eq!(at("too slow"), 22);
        assert_eq!(at("space"), 25);
        // 66 wide, centred: from column 27.
        let row = buffer_text(&buf).lines().nth(10).unwrap().to_string();
        assert_eq!(row.chars().position(|c| c == 'H'), Some(27 + 3));
    }
}
