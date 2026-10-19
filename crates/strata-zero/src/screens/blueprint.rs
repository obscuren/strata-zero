//! The blueprint window: Inspect on an item draws it as what it is, over the deck
//! screen. Every item opens the same window; only the cyberdeck has a drawing so far,
//! and anything else reads MISSING BLUEPRINT.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::screens::cyberdeck::DeckTab;
use crate::screens::deck::status_strip;
use crate::ui::chrome::{CYAN, DIM, PANEL_BG, put, solid};
use crossterm::event::{KeyCode, KeyEvent};
use game::{CopyId, ItemId, Slot};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// The window's size, the same for every item: this big where the terminal has room
/// for it and a margin of the screen under it, else the terminal less that margin.
const W: u16 = 100;
const H: u16 = 32;
/// Never smaller than this; the smallest terminal holds it with the margin to spare.
const MIN_W: u16 = 88;
const MIN_H: u16 = 28;
/// The screen under the window that shows round it: columns, rows.
const MARGIN: (u16, u16) = (2, 1);
/// What an item with no drawing yet shows.
const MISSING: &str = "MISSING BLUEPRINT";

pub struct Blueprint {
    /// ` BLUEPRINT · <NAME> `, on the top edge.
    title: String,
    /// The deck on its blueprint, with the copy it was opened on; `None` for an item
    /// that has none yet.
    deck: Option<DeckTab>,
}

impl Blueprint {
    /// The window on `item`, `equipped` when Inspect was on the item worn (GEAR) rather
    /// than one in the pack, `copy` the copy it was on when the item is held per copy.
    /// A deck the content knows draws as its look: the jacked one works, any other copy
    /// only reads. Anything else has no blueprint yet.
    pub fn open(st: &AppState, item: ItemId, equipped: bool, copy: Option<CopyId>) -> Blueprint {
        let known = st.content.item(&item);
        let name = known.map_or_else(|| item.to_string(), |i| i.name.clone());
        let title = format!(" BLUEPRINT · {} ", name.to_uppercase());
        let deck = known.and_then(|i| i.deck).map(|_| {
            let jacked = equipped && st.character.equipped(Slot::Deck) == Some(&item);
            DeckTab::for_deck(st.tick, item, jacked, copy, st)
        });
        Blueprint { title, deck }
    }

    /// In the middle of the screen, always inside the area.
    fn rect(area: Rect) -> Rect {
        let fit = |most: u16, least: u16, room: u16, margin: u16| {
            most.min(room.saturating_sub(2 * margin))
                .max(least)
                .min(room)
        };
        let w = fit(W, MIN_W, area.width, MARGIN.0);
        let h = fit(H, MIN_H, area.height, MARGIN.1);
        Rect::new(
            area.x + area.width.saturating_sub(w) / 2,
            area.y + area.height.saturating_sub(h) / 2,
            w,
            h,
        )
    }
}

/// The thin bezel of half blocks round the window, in the chrome's cyan.
fn bezel(buf: &mut Buffer, r: Rect) {
    let style = Style::default().fg(CYAN).bg(PANEL_BG);
    let (right, bottom) = (r.right() - 1, r.bottom() - 1);
    for x in r.x..=right {
        buf[(x, r.y)].set_char('▄').set_style(style);
        buf[(x, bottom)].set_char('▀').set_style(style);
    }
    for y in r.y..=bottom {
        buf[(r.x, y)].set_char('▐').set_style(style);
        buf[(right, y)].set_char('▌').set_style(style);
    }
    for (x, y, corner) in [
        (r.x, r.y, '▗'),
        (right, r.y, '▖'),
        (r.x, bottom, '▝'),
        (right, bottom, '▘'),
    ] {
        buf[(x, y)].set_char(corner).set_style(style);
    }
}

impl Screen for Blueprint {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        let mut cmd = ScreenCmd::Stay;
        for k in keys {
            cmd = match &mut self.deck {
                Some(tab) => tab.key(st, k.code),
                // Nothing to work: esc closes and no other key does anything.
                None if matches!(k.code, KeyCode::Esc | KeyCode::Char('q')) => ScreenCmd::Pop {
                    fx: TransitionKind::None,
                },
                None => ScreenCmd::Stay,
            };
            if !matches!(cmd, ScreenCmd::Stay) {
                break;
            }
        }
        // The jacked deck reopens on the part it was left on.
        if let Some(tab) = self.deck.as_ref().filter(|t| t.equipped) {
            st.deck_part = tab.pack();
        }
        cmd
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let r = Self::rect(f.area());
        if r.width < MIN_W || r.height < MIN_H {
            return;
        }
        let buf = f.buffer_mut();
        solid(buf, r);
        bezel(buf, r);
        let title = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
        put(buf, r.x + 3, r.y, &self.title, title, r.right() - 2);
        // Inside the frame: the blueprint, then the status row and the hint row.
        let inside = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2);
        let hint_y = inside.bottom() - 1;
        let strip_y = hint_y - 1;
        let hint = match &self.deck {
            Some(tab) => {
                let body = Rect::new(inside.x, inside.y, inside.width, strip_y - inside.y);
                crate::ui::cyberdeck::draw(&tab.view(st), body, buf);
                tab.hint()
            }
            None => {
                let x = r.x + (r.width - MISSING.chars().count() as u16) / 2;
                let style = Style::default().fg(DIM).add_modifier(Modifier::BOLD);
                put(buf, x, r.y + r.height / 2, MISSING, style, inside.right());
                "esc close"
            }
        };
        status_strip(buf, st, inside.x + 1, strip_y, inside.right());
        let dim = Style::default().fg(DIM);
        put(buf, inside.x + 1, hint_y, hint, dim, inside.right());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::Kind;
    use crate::screens::deck::Deck;
    use crate::screens::testutil::{buffer_text, key};
    use crate::ui::chrome::MAGENTA;
    use crate::ui::cyberdeck::Part;
    use citygen::{GenParams, Generator, Place, Pos, SizePreset, StratumKind};
    use game::{AbilityKind, Character};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    fn id(s: &str) -> ItemId {
        ItemId::new(s)
    }

    fn last(st: &AppState) -> Option<&str> {
        st.log.latest().map(|e| e.text.as_str())
    }

    fn last_is_warning(st: &AppState) -> bool {
        st.log.latest().is_some_and(|e| e.kind == Kind::Warn)
    }

    fn press(bp: &mut Blueprint, st: &mut AppState, codes: &[KeyCode]) -> ScreenCmd {
        let keys: Vec<_> = codes.iter().map(|c| key(*c)).collect();
        bp.update(st, &keys)
    }

    fn deck(bp: &Blueprint) -> &DeckTab {
        bp.deck.as_ref().expect("a deck's blueprint")
    }

    /// The window over the washed deck screen, as the app draws the stack.
    fn screen(bp: &Blueprint, st: &AppState, w: u16, h: u16) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| {
            Deck::new(0).draw(st, f);
            crate::ui::dim::wash(f.buffer_mut());
            bp.draw(st, f);
        })
        .unwrap();
        t.backend().buffer().clone()
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// The window on a terminal of `w` by `h`.
    fn window(w: u16, h: u16) -> Rect {
        Blueprint::rect(Rect::new(0, 0, w, h))
    }

    /// The window's corner on a terminal of `w` by `h`.
    fn corner(w: u16, h: u16) -> (u16, u16) {
        let r = window(w, h);
        (r.x, r.y)
    }

    /// The status row inside the window, at the smallest terminal, once it has typed.
    fn status(bp: &Blueprint, st: &mut AppState) -> String {
        st.tick += 30;
        let r = window(100, 30);
        let buf = screen(bp, st, 100, 30);
        let inside: String = (r.x + 1..r.right() - 1)
            .map(|cx| buf[(cx, r.bottom() - 3)].symbol())
            .collect();
        inside.trim().to_string()
    }

    /// Jacks the player into the construct of the first building, body on the street.
    fn jack_in(st: &mut AppState) {
        let b = st.city.as_ref().unwrap().buildings[0].id;
        let mut p = crate::walker::Player::at(Place::Construct(b), Pos::new(1, 1));
        p.body = Some((Place::Stratum(StratumKind::Street), Pos::new(1, 1)));
        st.player = Some(p);
    }

    #[test]
    fn the_window_frames_the_deck() {
        let mut st = state();
        st.tick = 40;
        let bp = Blueprint::open(&st, id("pawnshop_deck"), true, None);
        st.tick += 40;
        let buf = screen(&bp, &st, 100, 30);
        // The smallest terminal: as wide as leaves two columns of the screen each side.
        let r = window(100, 30);
        assert_eq!(r, Rect::new(2, 1, 96, 28));
        let (x, y) = (r.x, r.y);
        let (right, bottom) = (r.right() - 1, r.bottom() - 1);
        let edge = |y: u16| -> String { (x..=right).map(|cx| buf[(cx, y)].symbol()).collect() };
        let top = edge(y);
        assert!(top.starts_with("▗▄") && top.ends_with("▄▖"), "{top}");
        assert_eq!(edge(bottom), format!("▝{}▘", "▀".repeat(94)));
        for cy in y + 1..bottom {
            for (cx, side) in [(x, "▐"), (right, "▌")] {
                assert_eq!(buf[(cx, cy)].symbol(), side, "({cx},{cy})");
                assert_eq!(buf[(cx, cy)].fg, CYAN, "({cx},{cy})");
            }
        }
        for cx in [x, x + 40, right] {
            assert_eq!(buf[(cx, y)].fg, CYAN);
            assert_eq!(buf[(cx, bottom)].fg, CYAN);
        }
        // The title on the top edge from column 3, bold cyan.
        let title = " BLUEPRINT · PAWNSHOP DECK ";
        let at: String = top.chars().skip(3).take(title.chars().count()).collect();
        assert_eq!(at, title);
        let cell = &buf[(x + 4, y)];
        assert_eq!(cell.symbol(), "B");
        assert_eq!(cell.fg, CYAN);
        assert!(cell.modifier.contains(Modifier::BOLD));
        // Outside the window the deck screen shows, washed.
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| {
            Deck::new(0).draw(&st, f);
            crate::ui::dim::wash(f.buffer_mut());
        })
        .unwrap();
        let under = t.backend().buffer();
        for cy in 0..30 {
            for cx in 0..100 {
                if !r.contains((cx, cy).into()) {
                    assert_eq!(buf[(cx, cy)], under[(cx, cy)], "({cx},{cy})");
                }
            }
        }
        // The deck is drawn in the middle of the inside, five columns to spare on each
        // side: the slab's corners sit at the drawing's x 1 and 82, its rows 4 and 17.
        let text = buffer_text(&buf);
        for want in ["K▒ROK▒WA KX-2", "CRACK", "WAREZ", "FAILSAFE"] {
            assert!(text.contains(want), "{want}: {text}");
        }
        let (ox, oy) = (x + 6, y + 2);
        assert_eq!(buf[(ox + 1, oy + 4)].symbol(), "╭");
        assert_eq!(buf[(ox + 82, oy + 17)].symbol(), "╯");
        assert_eq!(buf[(ox + 5, oy + 5)].symbol(), "K");
        assert!(row(&buf, bottom - 1).contains("↑↓←→ point · space act · esc close"));
        // Worn, not jacked: the player's cable snakes out under the frame on the left;
        // the terminal's jack stays empty.
        assert_eq!(buf[(ox, oy + 16)].symbol(), "▐");
        assert_eq!(buf[(x + 1, oy + 20)].symbol(), "▌");
        assert_eq!(buf[(x, oy + 20)].symbol(), "▐");
        assert_eq!(buf[(ox + 82, oy + 16)].symbol(), "█");
        assert_eq!(buf[(ox + 83, oy + 16)].symbol(), " ");
        assert_eq!(buf[(right - 1, oy + 20)].symbol(), " ");

        // A bigger terminal: the full size, centred, with more room round the deck.
        let buf = screen(&bp, &st, 120, 40);
        let r = window(120, 40);
        assert_eq!(r, Rect::new(10, 4, 100, 32));
        let (x, y) = (r.x, r.y);
        assert_eq!(buf[(x, y)].symbol(), "▗");
        assert_eq!(buf[(r.right() - 1, r.bottom() - 1)].symbol(), "▘");
        assert_eq!(buf[(x + 8 + 1, y + 4 + 4)].symbol(), "╭");
        // Never smaller than the smallest terminal holds, and never past the screen.
        assert_eq!(window(92, 29), Rect::new(2, 0, 88, 28));
        assert_eq!(window(200, 60).width, 100);
        assert_eq!(window(200, 60).height, 32);
    }

    #[test]
    fn the_window_works_the_deck() {
        let mut st = state();
        let mut bp = Blueprint::open(&st, id("pawnshop_deck"), true, None);
        // Down from slot A: the plate, the first ROM strip, then the attack in WAREZ.
        press(
            &mut bp,
            &mut st,
            &[KeyCode::Down, KeyCode::Down, KeyCode::Down],
        );
        assert_eq!(deck(&bp).point, Part::Inv(0));
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(deck(&bp).held, Some(id("attack")));
        let (x, y) = corner(100, 30);
        let hint = row(&screen(&bp, &st, 100, 30), y + 26);
        assert!(
            hint.contains("↑↓←→ carry · space drop · esc put back"),
            "{hint}"
        );
        press(&mut bp, &mut st, &[KeyCode::Right, KeyCode::Char(' ')]);
        assert_eq!(st.character.loaded, vec![id("crack"), id("attack")]);
        assert_eq!(status(&bp, &mut st), "Attack slotted · Copy to inventory");

        // Space on the cart just slotted ejects it.
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(st.character.loaded, vec![id("crack")]);
        assert_eq!(status(&bp, &mut st), "Attack unloaded");

        // Right of the empty slot B is the failsafe: two presses turn it off.
        press(&mut bp, &mut st, &[KeyCode::Right]);
        assert_eq!(deck(&bp).point, Part::Failsafe);
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert!(st.net.failsafe && deck(&bp).cover_up);
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert!(!st.net.failsafe);
        assert!(last_is_warning(&st));
        assert_eq!(
            status(&bp, &mut st),
            "! failsafe off · black ICE can kill you"
        );
        let buf = screen(&bp, &st, 100, 30);
        assert_eq!(buf[(x + 2, y + 25)].fg, MAGENTA);
        press(&mut bp, &mut st, &[KeyCode::Enter]);
        assert!(st.net.failsafe);
        assert_eq!(status(&bp, &mut st), "failsafe on");

        // Jacked in, the deck is locked: nothing changes and the strip says so.
        jack_in(&mut st);
        let loaded = st.character.loaded.clone();
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert!(st.net.failsafe && !deck(&bp).cover_up);
        assert_eq!(st.character.loaded, loaded);
        assert_eq!(
            status(&bp, &mut st),
            "! the deck is locked while you're jacked in"
        );
        assert!(buffer_text(&screen(&bp, &st, 100, 30)).contains("locked · jacked in"));
        // ... and the terminal's cyan cable is in the right jack, out under the frame.
        let buf = screen(&bp, &st, 100, 30);
        let cell = &buf[(x + 94, y + 22)];
        assert_eq!((cell.symbol(), cell.fg), ("▌", CYAN));
    }

    #[test]
    fn esc_unwinds_then_closes() {
        for close in [KeyCode::Esc, KeyCode::Char('q')] {
            let mut st = state();
            let mut bp = Blueprint::open(&st, id("pawnshop_deck"), true, None);
            // Carrying: esc puts the cart back.
            press(
                &mut bp,
                &mut st,
                &[KeyCode::Down, KeyCode::Down, KeyCode::Down],
            );
            press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
            assert!(deck(&bp).held.is_some());
            assert!(matches!(press(&mut bp, &mut st, &[close]), ScreenCmd::Stay));
            assert_eq!(deck(&bp).held, None);
            assert_eq!(st.character.loaded, vec![id("crack"), id("copy")]);
            // Cover up: esc shuts it.
            press(&mut bp, &mut st, &[KeyCode::Right, KeyCode::Right]);
            assert_eq!(deck(&bp).point, Part::Failsafe);
            press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
            assert!(deck(&bp).cover_up);
            // The tab keys and the digits do nothing here.
            let before = (deck(&bp).clone(), st.character.clone(), st.net.failsafe);
            for code in [KeyCode::Tab, KeyCode::BackTab, KeyCode::Char('5')] {
                assert!(matches!(press(&mut bp, &mut st, &[code]), ScreenCmd::Stay));
            }
            assert_eq!(
                (deck(&bp).clone(), st.character.clone(), st.net.failsafe),
                before
            );
            assert!(matches!(press(&mut bp, &mut st, &[close]), ScreenCmd::Stay));
            assert!(!deck(&bp).cover_up && st.net.failsafe);
            // Nothing left to undo: it closes.
            assert!(matches!(
                press(&mut bp, &mut st, &[close]),
                ScreenCmd::Pop { .. }
            ));
        }
    }

    #[test]
    fn a_carried_deck_is_read_only() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        st.character.give(id("spoof"), 1);
        st.tick = 40;
        let mut bp = Blueprint::open(&st, id("pawnshop_deck"), false, None);
        assert!(!deck(&bp).equipped);
        // Opened as the jacked deck, a deck not in the DECK slot still only reads.
        let guarded = Blueprint::open(&st, id("pawnshop_deck"), true, None);
        assert!(!deck(&guarded).equipped);
        st.tick += 40;
        let buf = screen(&bp, &st, 100, 30);
        let (x, y) = corner(100, 30);
        // In the pack: both jacks empty, no cables.
        for (jack, plug) in [(x + 7, x + 6), (x + 88, x + 89)] {
            assert_eq!(buf[(jack, y + 18)].symbol(), "█");
            assert_eq!(buf[(plug, y + 18)].symbol(), " ");
        }
        for cx in [x + 1, x + 94] {
            assert_eq!(buf[(cx, y + 22)].symbol(), " ");
        }
        let text = buffer_text(&buf);
        assert!(text.contains("2 SLOT · R1"), "{text}");
        assert!(text.contains("not jacked · equip it first"), "{text}");
        assert!(
            text.contains("SPOOF"),
            "WAREZ as for the jacked deck: {text}"
        );
        // An empty bay: no carts, both slots say so.
        let bay: Vec<String> = (y + 3..=y + 5).map(|r| row(&buf, r)).collect();
        assert_eq!(bay[2].matches("empty").count(), 2, "{bay:?}");
        for name in ["CRACK", "COPY", "ATTACK", "BACKDOOR"] {
            assert!(!bay.iter().any(|r| r.contains(name)), "{name}: {bay:?}");
        }
        // Nothing on it can be changed, wherever the brackets are.
        let spoof = crate::screens::cyberdeck::inventory(&st)
            .iter()
            .position(|i| *i == id("spoof"))
            .unwrap();
        let before = (st.character.clone(), st.net.failsafe);
        for point in [Part::Slot(0), Part::Inv(spoof), Part::Failsafe] {
            bp.deck.as_mut().unwrap().point = point;
            for code in [KeyCode::Char(' '), KeyCode::Enter] {
                assert!(matches!(press(&mut bp, &mut st, &[code]), ScreenCmd::Stay));
                assert_eq!(last(&st), Some("equip the deck to work on it"));
                assert!(last_is_warning(&st));
            }
            assert_eq!((st.character.clone(), st.net.failsafe), before, "{point:?}");
            assert_eq!((deck(&bp).held.clone(), deck(&bp).cover_up), (None, false));
            assert_eq!(
                deck(&bp).view(&st).pinned.0,
                "not jacked · equip it first",
                "{point:?}"
            );
        }
        // Pointing and reading work: the plate, and the ROM burned into this deck.
        bp.deck.as_mut().unwrap().point = Part::Slot(0);
        press(&mut bp, &mut st, &[KeyCode::Down]);
        assert_eq!(deck(&bp).point, Part::Plate);
        st.tick += 40;
        assert!(buffer_text(&screen(&bp, &st, 100, 30)).contains("PAWNSHOP DECK"));
        press(&mut bp, &mut st, &[KeyCode::Down]);
        assert_eq!(deck(&bp).point, Part::Rom(0));
        st.tick += 40;
        let text = buffer_text(&screen(&bp, &st, 100, 30));
        assert!(text.contains("from Pawnshop deck"), "{text}");
    }

    #[test]
    fn the_worn_deck_shows_its_cradles() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        st.tick = 40;
        let copy = st.character.worn_copy(Slot::Deck).map(|k| k.id);
        let mut bp = Blueprint::open(&st, id("rebuilt_deck"), true, copy);
        assert!(deck(&bp).equipped);
        st.tick += 40;
        let buf = screen(&bp, &st, 100, 30);
        // The window at x 2, its inside from 3; the drawing's origin at x 8.
        let (x, y) = corner(100, 30);
        let (ox, oy) = (x + 6, y + 2);
        assert_eq!(ox, 8);
        assert_eq!(buf[(ox + 1, oy + 4)].symbol(), "╭");
        // Each socket's outer rail end: L at drawing x -4, one column in from the
        // inside's first; R at drawing x 87.
        assert_eq!(buf[(4, oy + 6)].symbol(), "▄");
        assert_eq!(buf[(95, oy + 6)].symbol(), "▄");
        assert_eq!(buf[(4, oy + 12)].symbol(), "▀");
        assert_eq!(buf[(95, oy + 12)].symbol(), "▀");
        // The maker plate's card lists both sockets, empty.
        press(&mut bp, &mut st, &[KeyCode::Down]);
        assert_eq!(deck(&bp).point, Part::Plate);
        let (body, _) = crate::screens::cyberdeck::glass(&st, deck(&bp));
        let empty = body
            .iter()
            .filter(|(t, _)| t == "□ empty socket · deck hardware")
            .count();
        assert_eq!(empty, 2, "{body:?}");
    }

    #[test]
    fn anything_else_is_a_missing_blueprint() {
        let mut st = state();
        st.tick = 40;
        let mut bp = Blueprint::open(&st, id("copper_coil"), false, None);
        assert!(bp.deck.is_none());
        let buf = screen(&bp, &st, 100, 30);
        let r = window(100, 30);
        let (x, y) = (r.x, r.y);
        let line = row(&buf, y + r.height / 2);
        let at = line
            .find("MISSING BLUEPRINT")
            .map(|b| line[..b].chars().count() as u16);
        assert_eq!(at, Some(x + (r.width - 17) / 2), "{line}");
        let cell = &buf[(at.unwrap(), y + 14)];
        assert_eq!(cell.fg, DIM);
        assert!(cell.modifier.contains(Modifier::BOLD));
        let text = buffer_text(&buf);
        assert!(text.contains(" BLUEPRINT · COPPER COIL "), "{text}");
        assert!(!text.contains("FAILSAFE"), "{text}");
        let hint = row(&buf, y + 26);
        assert!(
            hint.contains("esc close") && !hint.contains("space"),
            "{hint}"
        );
        // Nothing but esc does anything.
        let before = (st.character.clone(), st.net.failsafe, st.log.len());
        for code in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Char(' '),
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::Char('5'),
            KeyCode::PageDown,
        ] {
            assert!(matches!(press(&mut bp, &mut st, &[code]), ScreenCmd::Stay));
        }
        assert_eq!(
            (st.character.clone(), st.net.failsafe, st.log.len()),
            before
        );
        for close in [KeyCode::Esc, KeyCode::Char('q')] {
            assert!(matches!(
                press(&mut bp, &mut st, &[close]),
                ScreenCmd::Pop { .. }
            ));
        }
    }

    #[test]
    fn odd_decks_read_as_they_are() {
        let mut st = state();
        // A deck the content does not know has no blueprint; nor has one without a spec.
        st.character.equipment.insert(Slot::Deck, id("ghost_deck"));
        st.tick = 40;
        let bp = Blueprint::open(&st, id("ghost_deck"), true, None);
        let text = buffer_text(&screen(&bp, &st, 100, 30));
        assert!(
            text.contains(" BLUEPRINT · GHOST_DECK ") && text.contains("MISSING BLUEPRINT"),
            "{text}"
        );
        let mut specless = st.content.item(&id("pawnshop_deck")).unwrap().clone();
        specless.id = id("specless_deck");
        specless.deck = None;
        st.content.items.push(specless);
        assert!(
            Blueprint::open(&st, id("specless_deck"), false, None)
                .deck
                .is_none()
        );

        // No grants: the ROM row holds its label alone; the plate reads the deck's name.
        let mut bare = st.content.item(&id("pawnshop_deck")).unwrap().clone();
        bare.id = id("bare_deck");
        bare.name = "Bare deck".into();
        bare.grants.clear();
        st.content.items.push(bare);
        st.character.equipment.insert(Slot::Deck, id("bare_deck"));
        let mut bp = Blueprint::open(&st, id("bare_deck"), true, None);
        assert!(deck(&bp).equipped);
        st.tick += 40;
        let text = buffer_text(&screen(&bp, &st, 100, 30));
        assert!(text.contains("ROM▸") && !text.contains("NETSCAN"), "{text}");
        assert!(text.contains(" BLUEPRINT · BARE DECK "), "{text}");
        press(&mut bp, &mut st, &[KeyCode::Down]);
        assert_eq!(deck(&bp).point, Part::Plate);
        st.tick += 40;
        let (_, y) = corner(100, 30);
        let glass = row(&screen(&bp, &st, 100, 30), y + 2 + 7);
        assert!(glass.contains("BARE DECK"), "{glass}");

        // A passive grant reads as passive, as on SKILLS.
        let passive = game::AbilityId::new("street_sense");
        assert!(matches!(
            st.content.ability(&passive).unwrap().kind,
            AbilityKind::Passive
        ));
        let mut quiet = st.content.item(&id("pawnshop_deck")).unwrap().clone();
        quiet.id = id("quiet_deck");
        quiet.grants = vec![passive];
        st.content.items.push(quiet);
        st.character.equipment.insert(Slot::Deck, id("quiet_deck"));
        // The jacked deck's part is remembered across decks: it opens on the plate.
        let mut bp = Blueprint::open(&st, id("quiet_deck"), true, None);
        assert_eq!(deck(&bp).point, Part::Plate);
        press(&mut bp, &mut st, &[KeyCode::Down]);
        assert_eq!(deck(&bp).point, Part::Rom(0));
        st.tick += 40;
        let text = buffer_text(&screen(&bp, &st, 100, 30));
        assert!(
            text.contains("ROM ▸ STREET SENSE") && text.contains("passive"),
            "{text}"
        );
    }

    #[test]
    fn the_rom_reads_the_sharpened_ability() {
        let mut st = state();
        for t in ["wide_band", "ice_pick"] {
            st.character.talents.insert(game::TalentId::new(t), 2);
        }
        let mut bp = Blueprint::open(&st, id("pawnshop_deck"), true, None);
        press(&mut bp, &mut st, &[KeyCode::Down, KeyCode::Down]);
        assert_eq!(deck(&bp).point, Part::Rom(0));
        st.tick += 60;
        let text = buffer_text(&screen(&bp, &st, 100, 30));
        assert!(text.contains("cooldown 8s"), "{text}");
        assert!(text.contains("18 tiles"), "{text}");
    }

    #[test]
    fn the_equipped_deck_remembers_its_part() {
        let mut st = state();
        let mut bp = Blueprint::open(&st, id("pawnshop_deck"), true, None);
        press(&mut bp, &mut st, &[KeyCode::Right, KeyCode::Right]);
        assert_eq!(deck(&bp).point, Part::Failsafe);
        assert!(matches!(
            press(&mut bp, &mut st, &[KeyCode::Esc]),
            ScreenCmd::Pop { .. }
        ));
        let again = Blueprint::open(&st, id("pawnshop_deck"), true, None);
        assert_eq!(deck(&again).point, Part::Failsafe);

        // A carried deck opens on slot A and leaves the jacked deck's part alone.
        st.character.give(id("rebuilt_deck"), 1);
        let mut carried = Blueprint::open(&st, id("rebuilt_deck"), false, None);
        assert_eq!(deck(&carried).point, Part::Slot(0));
        press(&mut carried, &mut st, &[KeyCode::Down, KeyCode::Esc]);
        let again = Blueprint::open(&st, id("pawnshop_deck"), true, None);
        assert_eq!(deck(&again).point, Part::Failsafe);
    }
}
