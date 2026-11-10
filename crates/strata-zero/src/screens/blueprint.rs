//! The blueprint window: Inspect on an item draws it as what it is, over the deck
//! screen. Every item opens the same window, in one of three modes: inspect, the
//! item's own view; modding, where any item with sockets and a drawing has them
//! worked on; and stickers, where anything worn with a drawing has stickers stuck on. Decks, hardware and items with a look have drawings so far; anything else reads
//! MISSING BLUEPRINT. A drawing with a back (the Keyjack's) shows its back in modding,
//! opened up on its sockets, and its front in the other modes.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::screens::bench::Bench;
use crate::screens::cyberdeck::DeckTab;
use crate::screens::deck::status_strip;
use crate::screens::stickers::StickerBench;
use crate::ui::art::Art;
use crate::ui::chrome::{CYAN, DIM, PANEL_BG, put, solid};
use crate::ui::hardware::HardwareArt;
use crate::ui::looks::ItemArt;
use crate::ui::picture::PictureArt;
use crate::ui::stickers::{Loose, Placed, placed};
use crossterm::event::{KeyCode, KeyEvent};
use game::{CopyId, HardwareLook, Host, Item, ItemId, ItemLook, Slot};
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
pub(crate) const MISSING: &str = "MISSING BLUEPRINT";

pub struct Blueprint {
    /// The item's name, for the title on the top edge.
    title_name: String,
    /// The item's own view.
    inspect: Inspect,
    /// Modding's state, kept while inspect is up.
    bench: Bench,
    /// The sticker mode's state, kept likewise.
    pub(crate) stickers: StickerBench,
    pub(crate) mode: Mode,
    /// Where the item's sockets are: the copy worn, or the one in the pack it was
    /// opened on. `None` for an item held without a copy.
    host: Option<Host>,
    item: ItemId,
}

/// The looks of the pieces fitted in the host's copy of `item`, one per socket it
/// has. An item held without a copy has its sockets, all empty.
fn fitted_looks(st: &AppState, host: Option<Host>, item: &Item) -> Vec<Option<HardwareLook>> {
    let copy = host.and_then(|h| st.character.host_copy(h));
    (0..item.sockets.len())
        .map(|i| {
            let piece = copy?.fitted.get(i)?.as_ref()?;
            Some(st.content.item(&piece.item)?.hardware.as_ref()?.look)
        })
        .collect()
}

/// The stickers on the host's copy, with their art; none for an item held without one.
fn stuck_on(st: &AppState, host: Option<Host>) -> Vec<Placed> {
    host.and_then(|h| st.character.host_copy(h))
        .map(|k| placed(&st.content, &k.stickers))
        .unwrap_or_default()
}

/// What inspect shows.
enum Inspect {
    /// The deck as hardware: the jacked one works, any other copy only reads.
    Deck(DeckTab),
    /// A piece of hardware, drawn large to look at.
    Hardware(HardwareArt),
    /// An item drawn by its look, with its fitted pieces.
    Drawn(ItemLook),
    /// A sticker, drawn as it is.
    Sticker(Loose),
    /// An item drawn from its picture, to look at only.
    Pictured(PictureArt),
    /// No drawing yet.
    Missing,
}

/// The mode the window is in. Each has its own pointer, and switching resets none:
/// inspect's lives in `Inspect`, modding's in the bench, which starts on the first
/// socket, and the sticker mode's in its own bench, which starts on the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Inspect,
    Mod,
    Stickers,
}

impl Inspect {
    /// The drawing modding works on; none when there is no drawing.
    fn art(&self, st: &AppState, host: Option<Host>, item: &ItemId) -> Option<Box<dyn Art>> {
        match self {
            Inspect::Deck(tab) => Some(Box::new(tab.art(st))),
            Inspect::Hardware(art) => Some(Box::new(art.clone())),
            Inspect::Drawn(look) => Some(Box::new(ItemArt {
                look: *look,
                fitted: fitted_looks(st, host, st.content.item(item)?),
                stickers: stuck_on(st, host),
            })),
            Inspect::Sticker(art) => Some(Box::new(art.clone())),
            Inspect::Pictured(art) => Some(Box::new(art.clone())),
            Inspect::Missing => None,
        }
    }
}

/// What the window shows for an item: a deck the content knows as its look, hardware
/// as itself, any other item with a look as that, a sticker as it is, an item with a
/// picture as that, anything else nothing.
fn inspect_for(st: &AppState, item: &ItemId, equipped: bool, copy: Option<CopyId>) -> Inspect {
    match st.content.item(item) {
        Some(i) if i.deck.is_some() => {
            let jacked = equipped && st.character.equipped(Slot::Deck) == Some(item);
            Inspect::Deck(DeckTab::for_deck(st.tick, item.clone(), jacked, copy, st))
        }
        Some(i) => match (&i.hardware, i.look, &i.sticker) {
            (Some(h), _, _) => Inspect::Hardware(HardwareArt {
                look: h.look,
                mods: h.mods.clone(),
            }),
            (None, Some(look), _) => Inspect::Drawn(look),
            (None, None, Some(art)) => Inspect::Sticker(Loose { art: art.clone() }),
            (None, None, None) => match st.content.picture(item) {
                Some(p) => Inspect::Pictured(PictureArt { picture: p.clone() }),
                None => Inspect::Missing,
            },
        },
        None => Inspect::Missing,
    }
}

/// Where the item is fitted from: worn only when the slot still wears it, else the
/// copy in the pack.
fn host_for(st: &AppState, item: &ItemId, equipped: bool, copy: Option<CopyId>) -> Option<Host> {
    let worn = st
        .content
        .item(item)
        .and_then(|i| i.slot)
        .filter(|s| equipped && st.character.equipped(*s) == Some(item));
    worn.map(Host::Worn).or(copy.map(Host::Pack))
}

/// The drawing the window would show for the same arguments, with the deck unplugged
/// so no cable runs out of it. `None` when the item has no drawing.
pub fn preview_art(
    st: &AppState,
    item: &ItemId,
    equipped: bool,
    copy: Option<CopyId>,
) -> Option<Box<dyn Art>> {
    let inspect = inspect_for(st, item, equipped, copy);
    let host = host_for(st, item, equipped, copy);
    match &inspect {
        Inspect::Deck(tab) => Some(Box::new(tab.art(st).unplugged())),
        _ => inspect.art(st, host, item),
    }
}

impl Blueprint {
    /// The window on `item`, `equipped` when Inspect was on the item worn (the rack)
    /// rather than one in the pack, `copy` the copy it was on when the item is held per
    /// copy. A deck the content knows draws as its look, hardware as itself, any other
    /// item with a look as that, an item with a picture as its picture; anything else
    /// has no blueprint yet. It opens in inspect.
    pub fn open(st: &AppState, item: ItemId, equipped: bool, copy: Option<CopyId>) -> Blueprint {
        let known = st.content.item(&item);
        let title_name = known.map_or_else(|| item.to_string(), |i| i.name.clone());
        let inspect = inspect_for(st, &item, equipped, copy);
        let host = host_for(st, &item, equipped, copy);
        Blueprint {
            title_name,
            inspect,
            bench: Bench::default(),
            stickers: StickerBench::default(),
            mode: Mode::Inspect,
            host,
            item,
        }
    }

    /// `m` opens modding on an item with a drawing and sockets (its copy's, else the
    /// content's) and a host to fit into, unless a cart is being carried.
    fn moddable(&self, st: &AppState) -> bool {
        let carrying = matches!(&self.inspect, Inspect::Deck(tab) if tab.holding());
        let drawn = !matches!(self.inspect, Inspect::Missing);
        let sockets = match self.host.and_then(|h| st.character.host_copy(h)) {
            Some(copy) => copy.fitted.len(),
            None => st.content.item(&self.item).map_or(0, |i| i.sockets.len()),
        };
        drawn && !carrying && self.host.is_some() && sockets > 0
    }

    /// `s` opens the sticker mode on an item whose drawing takes stickers and that
    /// has a copy to stick them on, unless a cart is being carried.
    fn stickerable(&self, st: &AppState) -> bool {
        let carrying = matches!(&self.inspect, Inspect::Deck(tab) if tab.holding());
        let area = self.drawing(st).is_some_and(|a| a.sticker_area().is_some());
        area && !carrying && self.host.is_some()
    }

    /// The item's drawing as the modes work on it; none when there is no drawing.
    pub(crate) fn drawing(&self, st: &AppState) -> Option<Box<dyn Art>> {
        self.inspect.art(st, self.host, &self.item)
    }

    /// One key: `m` and `s` switch modes (the mode's own key back to inspect), the
    /// rest go to the mode that is up. While a piece is being chosen, a sticker held
    /// or a peel armed, they go to the mode instead: esc goes back first.
    fn key(&mut self, st: &mut AppState, code: KeyCode) -> ScreenCmd {
        let busy = self.bench.choosing.is_some() || self.stickers.busy();
        if code == KeyCode::Char('m') && !busy {
            if self.mode == Mode::Mod {
                self.mode = Mode::Inspect;
            } else if self.moddable(st) {
                self.mode = Mode::Mod;
            }
            return ScreenCmd::Stay;
        }
        if code == KeyCode::Char('s') && !busy {
            if self.mode == Mode::Stickers {
                self.mode = Mode::Inspect;
            } else if self.stickerable(st) {
                self.stickers.enter(st, self.host);
                self.mode = Mode::Stickers;
            }
            return ScreenCmd::Stay;
        }
        if self.mode != Mode::Inspect
            && let Some(art) = self.drawing(st)
        {
            return match self.mode {
                Mode::Mod => self.bench.key(st, art.as_ref(), self.host, code),
                _ => self.stickers.key(st, art.as_ref(), self.host, code),
            };
        }
        match &mut self.inspect {
            Inspect::Deck(tab) => tab.key(st, code),
            // Nothing to work: esc closes and no other key does anything.
            _ if matches!(code, KeyCode::Esc | KeyCode::Char('q')) => ScreenCmd::Pop {
                fx: TransitionKind::None,
            },
            _ => ScreenCmd::Stay,
        }
    }

    /// Inspect's view of the item in `body`; the hint row: the item's keys, `m mod`
    /// and `s stickers` when those modes are open to it, then the esc part.
    fn draw_inspect(&self, st: &AppState, r: Rect, body: Rect, buf: &mut Buffer) -> String {
        let (keys, esc) = match &self.inspect {
            Inspect::Deck(tab) => {
                crate::ui::cyberdeck::draw(&tab.view(st), body, buf);
                let (keys, esc) = tab.hint();
                (Some(keys), esc)
            }
            Inspect::Hardware(art) => {
                art.draw(art.origin(body), body, buf);
                (None, "esc close")
            }
            Inspect::Sticker(art) => {
                art.draw(art.origin(body), body, buf);
                (None, "esc close")
            }
            Inspect::Pictured(art) => {
                art.draw(art.origin(body), body, buf);
                (None, "esc close")
            }
            Inspect::Drawn(_) => {
                if let Some(art) = self.inspect.art(st, self.host, &self.item) {
                    art.draw(art.origin(body), body, buf);
                }
                (None, "esc close")
            }
            Inspect::Missing => {
                let x = r.x + (r.width - MISSING.chars().count() as u16) / 2;
                let style = Style::default().fg(DIM).add_modifier(Modifier::BOLD);
                put(buf, x, r.y + r.height / 2, MISSING, style, body.right());
                (None, "esc close")
            }
        };
        let m = self.moddable(st).then_some("m mod");
        let s = self.stickerable(st).then_some("s stickers");
        let parts: Vec<&str> = keys.into_iter().chain(m).chain(s).chain([esc]).collect();
        parts.join(" · ")
    }

    /// In the middle of the screen, always inside the area.
    pub(crate) fn rect(area: Rect) -> Rect {
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
            cmd = self.key(st, k.code);
            if !matches!(cmd, ScreenCmd::Stay) {
                break;
            }
        }
        // The jacked deck reopens on the part it was left on.
        if let Inspect::Deck(tab) = &self.inspect
            && tab.equipped
        {
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
        let art = match self.mode {
            Mode::Inspect => None,
            Mode::Mod | Mode::Stickers => self.drawing(st),
        };
        let mode = match (self.mode, &art) {
            (Mode::Mod, Some(_)) => "MODDING",
            (Mode::Stickers, Some(_)) => "STICKERS",
            _ => "BLUEPRINT",
        };
        let title = format!(" {mode} · {} ", self.title_name.to_uppercase());
        let style = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
        put(buf, r.x + 3, r.y, &title, style, r.right() - 2);
        // Inside the frame: the item, then the status row and the hint row.
        let inside = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2);
        let hint_y = inside.bottom() - 1;
        let strip_y = hint_y - 1;
        let body = Rect::new(inside.x, inside.y, inside.width, strip_y - inside.y);
        let hint = match art {
            Some(art) if self.mode == Mode::Mod => {
                self.bench.draw(st, art.as_ref(), self.host, body, buf);
                self.bench.hint().to_string()
            }
            Some(art) => {
                self.stickers.draw(st, art.as_ref(), self.host, body, buf);
                self.stickers.hint(st, self.host)
            }
            None => self.draw_inspect(st, r, body, buf),
        };
        status_strip(buf, st, inside.x + 1, strip_y, inside.right());
        let dim = Style::default().fg(DIM);
        put(buf, inside.x + 1, hint_y, &hint, dim, inside.right());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::perform;
    use crate::messages::Kind;
    use crate::screens::deck::Deck;
    use crate::screens::testutil::{buffer_text, key};
    use crate::ui::art::grey;
    use crate::ui::chrome::MAGENTA;
    use crate::ui::cyberdeck::Part;
    use crate::ui::looks::ItemArt;
    use citygen::{GenParams, Generator, Place, Pos, SizePreset, StratumKind};
    use game::{AbilityKind, Action, Character, DeckLook, HardwareLook, Host, ItemLook};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Cell;
    use ratatui::style::Color;

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
        match &bp.inspect {
            Inspect::Deck(tab) => tab,
            _ => panic!("a deck's blueprint"),
        }
    }

    fn deck_mut(bp: &mut Blueprint) -> &mut DeckTab {
        match &mut bp.inspect {
            Inspect::Deck(tab) => tab,
            _ => panic!("a deck's blueprint"),
        }
    }

    fn modding(bp: &Blueprint) -> bool {
        bp.mode == Mode::Mod
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
        assert!(row(&buf, bottom - 1).contains("↑↓←→ point · space act · s stickers · esc close"));
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
            deck_mut(&mut bp).point = point;
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
        deck_mut(&mut bp).point = Part::Slot(0);
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
    fn the_worn_deck_shows_its_connectors() {
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
        // Both sockets are empty: no rail where each outer end would be (L at drawing
        // x -4, R at 87), and the gold connectors in the edges.
        for (x, y) in [(4, oy + 6), (95, oy + 6), (4, oy + 12), (95, oy + 12)] {
            assert_eq!(buf[(x, y)].symbol(), " ", "({x},{y})");
        }
        assert_eq!(buf[(ox + 1, oy + 9)].symbol(), "▌");
        assert_eq!(buf[(ox + 82, oy + 9)].symbol(), "▐");
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
        assert!(matches!(bp.inspect, Inspect::Missing));
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
    fn armor_is_drawn_from_its_picture() {
        let mut st = state();
        st.tick = 40;
        let armor = [
            "salvage_jacket",
            "ballistic_weave",
            "cargo_pants",
            "hardhat",
            "mirrorshades",
        ];
        for item in armor {
            let mut bp = Blueprint::open(&st, id(item), false, None);
            assert!(matches!(bp.inspect, Inspect::Pictured(_)), "{item}");
            let picture = st.content.picture(&id(item)).unwrap();
            let colours: Vec<Color> = picture
                .colours
                .values()
                .map(|&(r, g, b)| Color::Rgb(r, g, b))
                .collect();
            for (w, h) in [(100, 30), (120, 40)] {
                let buf = screen(&bp, &st, w, h);
                let text = buffer_text(&buf);
                assert!(!text.contains("MISSING BLUEPRINT"), "{item} {w}x{h}");
                // The whole picture fits the body, and is what is drawn there.
                let body = body_of(w, h);
                let art = bp.drawing(&st).unwrap();
                let (aw, ah) = art.size();
                assert!(aw <= body.width && ah <= body.height, "{item} {w}x{h}");
                let (ox, oy) = art.origin(body);
                let painted = (oy..oy + ah)
                    .flat_map(|y| (ox..ox + aw).map(move |x| (x, y)))
                    .filter(|&p| colours.contains(&buf[p].bg) || colours.contains(&buf[p].fg))
                    .count();
                let room = usize::from(aw) * usize::from(ah);
                assert!(painted * 3 > room, "{item} {w}x{h}: {painted} of {room}");
                // To look at only: no modding, no stickers.
                let r = window(w, h);
                let hint = row(&buf, r.y + r.height - 2);
                assert!(hint.contains("esc close"), "{hint}");
                assert!(!hint.contains("m mod") && !hint.contains("s stickers"), "{hint}");
            }
            for code in [KeyCode::Char('m'), KeyCode::Char('s')] {
                assert!(matches!(press(&mut bp, &mut st, &[code]), ScreenCmd::Stay));
                assert_eq!(bp.mode, Mode::Inspect, "{item}");
            }
        }
        // The hardhat's stencil is over its shell.
        let bp = Blueprint::open(&st, id("hardhat"), false, None);
        let text = buffer_text(&screen(&bp, &st, 100, 30));
        assert!(text.contains("DOCK 9"), "{text}");
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
        let specless = Blueprint::open(&st, id("specless_deck"), false, None);
        assert!(matches!(specless.inspect, Inspect::Missing));

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

    /// The text inside the window's frame on row `dy` from its top edge, trimmed.
    fn inside(buf: &Buffer, w: u16, h: u16, dy: u16) -> String {
        let r = window(w, h);
        let line: String = (r.x + 1..r.right() - 1)
            .map(|x| buf[(x, r.y + dy)].symbol())
            .collect();
        line.trim().to_string()
    }

    /// The rows of the window's title, readout and hint, from its top edge.
    fn rows_of(w: u16, h: u16) -> (u16, u16, u16) {
        let r = window(w, h);
        (0, r.height - 4, r.height - 2)
    }

    /// Where `text` starts on the screen.
    fn find(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
        (0..buf.area.height).find_map(|y| {
            let line = row(buf, y);
            let at = line.find(text)?;
            Some((line[..at].chars().count() as u16, y))
        })
    }

    /// D'SHAWN's jacked deck, inspected from the rack: the rebuilt one, with two sockets.
    fn dshawns_deck(st: &mut AppState) -> Blueprint {
        st.character = Character::debug_runner(&st.content);
        st.tick = 40;
        let copy = st.character.worn_copy(Slot::Deck).map(|k| k.id);
        Blueprint::open(st, id("rebuilt_deck"), true, copy)
    }

    /// The copy in the pack of `item`.
    fn pack_copy(st: &AppState, item: &str) -> CopyId {
        let stack = st.character.inventory.iter().find(|s| s.item == id(item));
        stack.and_then(|s| s.copy_id()).expect("a copy in the pack")
    }

    const LABEL: Color = Color::Rgb(0x8b, 0x85, 0x96);
    const STEEL: Color = Color::Rgb(0x8b, 0x85, 0x96);
    const EDGE: Color = Color::Rgb(0xa0, 0x58, 0x3a);
    const GOLD: Color = Color::Rgb(0xc9, 0xa2, 0x27);
    const WHITE: Color = Color::Rgb(0xe8, 0xe8, 0xec);

    #[test]
    fn hardware_has_its_own_blueprint() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        st.tick = 40;
        let copy = pack_copy(&st, "coldfin_2");
        let mut bp = Blueprint::open(&st, id("coldfin_2"), false, Some(copy));
        let (title, _, hint) = rows_of(100, 30);
        let buf = screen(&bp, &st, 100, 30);
        let line = inside(&buf, 100, 30, title);
        assert!(line.contains(" BLUEPRINT · COLDFIN VII "), "{line}");
        let text = buffer_text(&buf);
        assert!(text.contains("TRACE −20%"), "{text}");
        assert!(!text.contains("MISSING BLUEPRINT"), "{text}");
        // Drawn large in the middle of the body: the clamp's gold contacts on its left.
        let r = window(100, 30);
        let body = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 4);
        let (ox, oy) = (
            body.x + (body.width - 52) / 2,
            body.y + (body.height - 18) / 2,
        );
        let cell = &buf[(ox, oy + 6)];
        assert_eq!((cell.symbol(), cell.fg), ("▌", GOLD));
        assert_eq!(inside(&buf, 100, 30, hint), "esc close");
        // No sockets: m does nothing.
        assert!(matches!(
            press(&mut bp, &mut st, &[KeyCode::Char('m')]),
            ScreenCmd::Stay
        ));
        assert!(!modding(&bp));
        assert_eq!(buffer_text(&screen(&bp, &st, 100, 30)), text);
        for code in [KeyCode::Char(' '), KeyCode::Right, KeyCode::Enter] {
            assert!(matches!(press(&mut bp, &mut st, &[code]), ScreenCmd::Stay));
        }
        assert!(matches!(
            press(&mut bp, &mut st, &[KeyCode::Esc]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn a_sticker_has_its_own_blueprint() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        st.tick = 40;
        let mut bp = Blueprint::open(&st, id("sticker_no_future"), false, None);
        let (title, _, hint) = rows_of(100, 30);
        let buf = screen(&bp, &st, 100, 30);
        let line = inside(&buf, 100, 30, title);
        assert!(line.contains(" BLUEPRINT · NO FUTURE "), "{line}");
        let text = buffer_text(&buf);
        assert!(!text.contains("MISSING BLUEPRINT"), "{text}");
        // Level, at its size, centred in the body.
        let r = window(100, 30);
        let body = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 4);
        let (ox, oy) = (
            body.x + (body.width - 11) / 2,
            body.y + (body.height - 1) / 2,
        );
        let at: String = (ox..ox + 11).map(|x| buf[(x, oy)].symbol()).collect();
        assert_eq!(at, " NO FUTURE ");
        assert_eq!(buf[(ox, oy)].bg, MAGENTA);
        assert_eq!(inside(&buf, 100, 30, hint), "esc close");
        for code in [KeyCode::Char('m'), KeyCode::Char('s'), KeyCode::Char(' ')] {
            assert!(matches!(press(&mut bp, &mut st, &[code]), ScreenCmd::Stay));
        }
        assert_eq!(buffer_text(&screen(&bp, &st, 100, 30)), text);
    }

    #[test]
    fn m_switches_modes_on_an_item_with_sockets() {
        let mut st = state();
        let mut bp = dshawns_deck(&mut st);
        let (w, h) = (100, 30);
        let (title, readout, hint) = rows_of(w, h);
        let buf = screen(&bp, &st, w, h);
        let line = inside(&buf, w, h, hint);
        assert!(
            line.contains("space act · m mod · s stickers · esc close"),
            "{line}"
        );
        // The inspect pointer to the plate; modding has a pointer of its own.
        press(&mut bp, &mut st, &[KeyCode::Down]);
        assert_eq!(deck(&bp).point, Part::Plate);
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        assert!(modding(&bp));
        let buf = screen(&bp, &st, w, h);
        let line = inside(&buf, w, h, title);
        assert!(line.contains(" MODDING · REBUILT PAWN DECK "), "{line}");
        assert_eq!(
            inside(&buf, w, h, hint),
            "↑↓←→ socket · space choose · m inspect · esc close"
        );
        assert_eq!(
            inside(&buf, w, h, readout),
            "SOCKET L ▸ empty · takes deck hardware"
        );
        // The readout's runs from the inside's second column: bold cyan, white, dim.
        let r = window(w, h);
        let at = |x: u16| &buf[(r.x + x, r.y + readout)];
        assert_eq!((at(2).symbol(), at(2).fg), ("S", CYAN));
        assert!(at(2).modifier.contains(Modifier::BOLD));
        assert_eq!((at(11).symbol(), at(11).fg), ("▸", WHITE));
        assert!(!at(11).modifier.contains(Modifier::BOLD));
        assert_eq!((at(19).symbol(), at(19).fg), ("·", DIM));

        press(&mut bp, &mut st, &[KeyCode::Right]);
        let buf = screen(&bp, &st, w, h);
        assert_eq!(
            inside(&buf, w, h, readout),
            "SOCKET R ▸ empty · takes deck hardware"
        );
        press(&mut bp, &mut st, &[KeyCode::Char('h')]);
        let buf = screen(&bp, &st, w, h);
        assert!(inside(&buf, w, h, readout).starts_with("SOCKET L "));
        // Back to inspect: its pointer where it was.
        assert_eq!(deck(&bp).point, Part::Plate);
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        assert!(!modding(&bp));
        assert_eq!(deck(&bp).point, Part::Plate);
        let buf = screen(&bp, &st, w, h);
        let line = inside(&buf, w, h, title);
        assert!(line.contains(" BLUEPRINT · REBUILT PAWN DECK "), "{line}");

        // The pawnshop deck he carries has no sockets: no m.
        let mut pawn = Blueprint::open(&st, id("pawnshop_deck"), false, None);
        let buf = screen(&pawn, &st, w, h);
        let line = inside(&buf, w, h, hint);
        assert_eq!(line, "↑↓←→ point · space act · esc close");
        let before = (deck(&pawn).clone(), buffer_text(&buf));
        assert!(matches!(
            press(&mut pawn, &mut st, &[KeyCode::Char('m')]),
            ScreenCmd::Stay
        ));
        assert!(!modding(&pawn));
        let after = (deck(&pawn).clone(), buffer_text(&screen(&pawn, &st, w, h)));
        assert_eq!(after, before);
    }

    #[test]
    fn the_readout_names_what_is_fitted() {
        let mut st = state();
        let mut bp = dshawns_deck(&mut st);
        let fit = |st: &mut AppState, host, socket, piece: &str| {
            let piece = pack_copy(st, piece);
            let fit = Action::Fit {
                host,
                socket,
                piece,
            };
            perform(st, fit).unwrap();
        };
        fit(&mut st, Host::Worn(Slot::Deck), 1, "coldfin_2");
        press(&mut bp, &mut st, &[KeyCode::Char('m'), KeyCode::Char('l')]);
        let (_, readout, _) = rows_of(100, 30);
        let buf = screen(&bp, &st, 100, 30);
        assert_eq!(
            inside(&buf, 100, 30, readout),
            "SOCKET R ▸ Coldfin vII · trace per cast −20%"
        );
        // Esc closes the window.
        assert!(matches!(
            press(&mut bp, &mut st, &[KeyCode::Esc]),
            ScreenCmd::Pop { .. }
        ));

        // A deck carried in the pack mods too, and reads its own copy's sockets.
        st.character.give_item(&st.content, id("rebuilt_deck"), 1);
        let copy = pack_copy(&st, "rebuilt_deck");
        fit(&mut st, Host::Pack(copy), 0, "sidecar");
        let mut carried = Blueprint::open(&st, id("rebuilt_deck"), false, Some(copy));
        press(&mut carried, &mut st, &[KeyCode::Char('m')]);
        assert!(modding(&carried));
        let buf = screen(&carried, &st, 100, 30);
        assert_eq!(
            inside(&buf, 100, 30, readout),
            "SOCKET L ▸ Sidecar co-processor · program power +1"
        );
        press(&mut carried, &mut st, &[KeyCode::Right]);
        let buf = screen(&carried, &st, 100, 30);
        assert_eq!(
            inside(&buf, 100, 30, readout),
            "SOCKET R ▸ empty · takes deck hardware"
        );
    }

    #[test]
    fn modding_greys_the_item_but_not_its_sockets() {
        let mut st = state();
        let mut bp = dshawns_deck(&mut st);
        // Rails draw round fitted pieces only.
        for (socket, piece) in [(0, "coldfin_1"), (1, "sidecar")] {
            let piece = pack_copy(&st, piece);
            let fit = Action::Fit {
                host: Host::Worn(Slot::Deck),
                socket,
                piece,
            };
            perform(&mut st, fit).unwrap();
        }
        st.tick += 40;
        let (x, y) = corner(100, 30);
        let (ox, oy) = (x + 6, y + 2);
        // Inspect: the maker plate bold in its own colour, the glass on slot A.
        let buf = screen(&bp, &st, 100, 30);
        let k = &buf[(ox + 5, oy + 5)];
        assert_eq!((k.symbol(), k.fg), ("K", LABEL));
        assert!(k.modifier.contains(Modifier::BOLD));
        assert!(buffer_text(&buf).contains("SLOT A"));
        assert_eq!(buf[(ox + 7, oy)].symbol(), "┏");

        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        let buf = screen(&bp, &st, 100, 30);
        let k = &buf[(ox + 5, oy + 5)];
        assert_eq!(k.symbol(), "K");
        assert_eq!(k.fg, grey(LABEL, PANEL_BG));
        assert!(!k.modifier.contains(Modifier::BOLD));
        // The rust edge greys too, and the player's cable; the window's panel stays.
        let edge = &buf[(ox + 1, oy + 4)];
        assert_eq!((edge.symbol(), edge.fg), ("╭", grey(EDGE, PANEL_BG)));
        let cable = &buf[(ox, oy + 16)];
        assert_eq!(cable.symbol(), "▐");
        assert!(
            matches!(cable.fg, Color::Rgb(r, g, _) if r == g),
            "{:?}",
            cable.fg
        );
        assert_eq!(buf[(ox, oy)].bg, PANEL_BG);
        // Each socket keeps its colours: its rails and the gold contacts in the edge.
        let rail = &buf[(ox - 3, oy + 6)];
        assert_eq!((rail.symbol(), rail.fg), ("▄", STEEL));
        let pin = &buf[(ox + 1, oy + 9)];
        assert_eq!((pin.symbol(), pin.fg), ("▌", GOLD));
        let rail = &buf[(ox + 86, oy + 12)];
        assert_eq!((rail.symbol(), rail.fg), ("▀", STEEL));
        // The brackets on socket L, none on slot A; the glass blank, no WAREZ.
        assert_eq!(buf[(ox - 5, oy + 5)].symbol(), "┏");
        assert_eq!(buf[(ox + 2, oy + 13)].symbol(), "┛");
        assert_ne!(buf[(ox + 7, oy)].symbol(), "┏");
        let text = buffer_text(&buf);
        assert!(!text.contains("SLOT A"), "{text}");
        assert!(!text.contains("WAREZ"), "{text}");
    }

    #[test]
    fn modding_greys_the_terminal_cable_too() {
        let net = [CYAN, Color::Rgb(0x00, 0x3c, 0x46)];
        for (w, h) in [(100, 30), (120, 40)] {
            let mut st = state();
            let mut bp = dshawns_deck(&mut st);
            jack_in(&mut st);
            // The terminal's cable, right of the drawing on rows 16-20, out to the
            // body's edge; the bottom rows fall where the tray will go.
            let r = window(w, h);
            let body = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 4);
            let (ox, oy) = crate::ui::cyberdeck::origin(DeckLook::Pawn, body);
            let cable = |buf: &Buffer| -> Vec<((u16, u16), Cell)> {
                let cells =
                    (oy + 16..=oy + 20).flat_map(|y| (ox + 85..body.right()).map(move |x| (x, y)));
                cells.map(|at| (at, buf[at].clone())).collect()
            };
            let before = cable(&screen(&bp, &st, w, h));
            let cyan = before.iter().filter(|(_, c)| c.fg == CYAN).count();
            assert!(cyan > 4, "{w}x{h}: the cable is drawn, {cyan} cyan cells");
            press(&mut bp, &mut st, &[KeyCode::Char('m')]);
            assert!(modding(&bp));
            let after = cable(&screen(&bp, &st, w, h));
            for ((at, was), (_, now)) in before.iter().zip(&after) {
                assert_eq!(now.symbol(), was.symbol(), "{w}x{h} {at:?}");
                assert_eq!(now.fg, grey(was.fg, Color::Reset), "{w}x{h} {at:?}");
                assert_eq!(now.bg, grey(was.bg, PANEL_BG), "{w}x{h} {at:?}");
                for c in [now.fg, now.bg] {
                    assert!(!net.contains(&c), "{w}x{h} {at:?}: {c:?}");
                }
            }
        }
    }

    #[test]
    fn modding_keeps_the_drawing_in_place() {
        for (w, h) in [(120, 40), (100, 30)] {
            let mut st = state();
            let mut bp = dshawns_deck(&mut st);
            let at = find(&screen(&bp, &st, w, h), "K▒ROK▒WA");
            assert!(at.is_some(), "{w}x{h}");
            press(&mut bp, &mut st, &[KeyCode::Char('m')]);
            assert!(modding(&bp));
            assert_eq!(find(&screen(&bp, &st, w, h), "K▒ROK▒WA"), at, "{w}x{h}");
        }
    }

    #[test]
    fn holding_a_cart_blocks_m() {
        let mut st = state();
        let mut bp = dshawns_deck(&mut st);
        st.character.give(id("spoof"), 1);
        let spoof = crate::screens::cyberdeck::inventory(&st)
            .iter()
            .position(|i| *i == id("spoof"))
            .unwrap();
        deck_mut(&mut bp).point = Part::Inv(spoof);
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(deck(&bp).held, Some(id("spoof")));
        let (_, _, hint) = rows_of(100, 30);
        let buf = screen(&bp, &st, 100, 30);
        assert_eq!(
            inside(&buf, 100, 30, hint),
            "↑↓←→ carry · space drop · esc put back"
        );
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        assert!(!modding(&bp));
        assert_eq!(deck(&bp).held, Some(id("spoof")));
        // Put back, m works again.
        press(&mut bp, &mut st, &[KeyCode::Esc, KeyCode::Char('m')]);
        assert!(modding(&bp));
    }

    fn choosing(bp: &Blueprint) -> bool {
        bp.bench.choosing.is_some()
    }

    /// The tray's two tile rows, from the window's top edge.
    fn tile_rows(w: u16, h: u16) -> (u16, u16) {
        let r = window(w, h);
        (r.height - 7, r.height - 6)
    }

    /// The deck's drawing origin in the window on a terminal of `w` by `h`.
    fn deck_origin(w: u16, h: u16) -> (u16, u16) {
        let r = window(w, h);
        let body = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 4);
        crate::ui::cyberdeck::origin(DeckLook::Pawn, body)
    }

    /// The text across the deck drawing's own columns on row `dy` from the window's
    /// top edge, trimmed: the tray's rows, without the cables either side.
    fn across(buf: &Buffer, w: u16, h: u16, dy: u16) -> String {
        let (ox, _) = deck_origin(w, h);
        let width = crate::ui::cyberdeck::size(DeckLook::Pawn).0;
        let y = window(w, h).y + dy;
        let line: String = (ox..ox + width).map(|x| buf[(x, y)].symbol()).collect();
        line.trim().to_string()
    }

    /// What each socket of a host holds.
    fn fitted(st: &AppState, host: Host) -> Vec<Option<ItemId>> {
        let copy = st.character.host_copy(host).expect("a copy");
        copy.fitted
            .iter()
            .map(|s| s.as_ref().map(|s| s.item.clone()))
            .collect()
    }

    fn in_pack(st: &AppState, item: &str) -> usize {
        let item = id(item);
        st.character
            .inventory
            .iter()
            .filter(|s| s.item == item)
            .count()
    }

    const SOCKET_HINT: &str = "↑↓←→ socket · space choose · m inspect · esc close";
    const TRAY_HINT: &str = "←→ hardware · space fit · esc back";
    const ALU: Color = Color::Rgb(0xb4, 0xbe, 0xc8);
    const ALU_DK: Color = Color::Rgb(0x2b, 0x31, 0x38);
    const TRAY_INK: Color = Color::Rgb(0x1a, 0x1e, 0x24);
    const TRAY_LABEL: Color = Color::Rgb(0x6b, 0x65, 0x75);

    #[test]
    fn choosing_fits_the_pointed_piece() {
        let mut st = state();
        let mut bp = dshawns_deck(&mut st);
        let (w, h) = (100, 30);
        let (_, readout, hint) = rows_of(w, h);
        let (names, lines) = tile_rows(w, h);
        let (ox, oy) = deck_origin(w, h);
        let y = window(w, h).y + names;
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        // Not choosing yet: the tray lies there, no brackets in it.
        let buf = screen(&bp, &st, w, h);
        assert!(across(&buf, w, h, names).starts_with("HARDWARE"));
        assert_ne!(buf[(ox + 12, y - 1)].symbol(), "┏");
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert!(choosing(&bp));
        let buf = screen(&bp, &st, w, h);
        let row = across(&buf, w, h, names);
        assert!(row.starts_with("HARDWARE"), "{row}");
        for want in ["COLDFIN VI ", "COLDFIN VII"] {
            assert!(row.contains(want), "{want}: {row}");
        }
        // The third tile would end past the drawing: the row is cut, `›` at its end.
        assert!(row.ends_with('›') && !row.contains("SIDECAR"), "{row}");
        let second = across(&buf, w, h, lines);
        assert!(second.contains("trace per cast −10%"), "{second}");
        assert!(second.contains("trace per cast −20%"), "{second}");
        assert_eq!(inside(&buf, w, h, hint), TRAY_HINT);
        assert_eq!(
            inside(&buf, w, h, readout),
            "COLDFIN VI ▸ socket L · trace per cast −10%"
        );
        // The label at the drawing's column 2; the first tile 11 later: its swatch,
        // then its name bold on the tile's colours, its mod under it.
        let label = &buf[(ox + 2, y)];
        assert_eq!((label.symbol(), label.fg), ("H", TRAY_LABEL));
        assert!(label.modifier.contains(Modifier::BOLD));
        for (x, dy) in [(ox + 13, 0), (ox + 14, 1)] {
            let cell = &buf[(x, y + dy)];
            assert_eq!((cell.symbol(), cell.fg, cell.bg), ("▀", ALU, ALU_DK));
        }
        let c = &buf[(ox + 16, y)];
        assert_eq!((c.symbol(), c.fg, c.bg), ("C", TRAY_INK, ALU));
        assert!(c.modifier.contains(Modifier::BOLD));
        let t = &buf[(ox + 16, y + 1)];
        assert_eq!((t.symbol(), t.fg, t.bg), ("t", TRAY_INK, ALU));
        assert!(!t.modifier.contains(Modifier::BOLD));
        // 23 wide: the swatch, a space, the mod line, a space.
        assert_eq!(buf[(ox + 35, y + 1)].bg, ALU);
        assert_eq!(buf[(ox + 36, y + 1)].bg, PANEL_BG);
        // The brackets drop onto that tile; the socket gets a dashed outline.
        for (x, cy, glyph) in [
            (ox + 12, y - 1, "┏"),
            (ox + 36, y - 1, "┓"),
            (ox + 12, y + 2, "┗"),
            (ox + 36, y + 2, "┛"),
        ] {
            assert_eq!(buf[(x, cy)].symbol(), glyph, "({x},{cy})");
        }
        let (left, right, top, bottom) = (ox - 5, ox + 2, oy + 5, oy + 13);
        for (x, cy, glyph) in [
            (left, top, "┌"),
            (right, top, "┐"),
            (left, bottom, "└"),
            (right, bottom, "┘"),
            (ox - 2, top, "╌"),
            (ox, bottom, "╌"),
            (left, oy + 9, "╎"),
            (right, oy + 6, "╎"),
        ] {
            let cell = &buf[(x, cy)];
            assert_eq!((cell.symbol(), cell.fg), (glyph, CYAN), "({x},{cy})");
        }
        assert_ne!(buf[(ox - 5, oy + 5)].symbol(), "┏");

        press(&mut bp, &mut st, &[KeyCode::Right]);
        let buf = screen(&bp, &st, w, h);
        assert_eq!(
            inside(&buf, w, h, readout),
            "COLDFIN VII ▸ socket L · trace per cast −20%"
        );
        // On to the third: the row scrolls to show it, `‹` at its start.
        press(&mut bp, &mut st, &[KeyCode::Char('l')]);
        let buf = screen(&bp, &st, w, h);
        let row = across(&buf, w, h, names);
        assert!(row.contains("SIDECAR CO-PROCESSOR"), "{row}");
        assert!(row.contains("‹") && !row.contains("COLDFIN VI "), "{row}");
        assert_eq!(buf[(ox + 11, y)].symbol(), "‹");
        assert_eq!(
            inside(&buf, w, h, readout),
            "SIDECAR CO-PROCESSOR ▸ socket L · program power +1"
        );
        // The end of the row: no further.
        press(&mut bp, &mut st, &[KeyCode::Right]);
        assert!(inside(&screen(&bp, &st, w, h), w, h, readout).starts_with("SIDECAR"));
        press(&mut bp, &mut st, &[KeyCode::Left]);
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert!(!choosing(&bp));
        assert_eq!(status(&bp, &mut st), "Coldfin vII fitted · socket L");
        assert_eq!(
            fitted(&st, Host::Worn(Slot::Deck)),
            vec![Some(id("coldfin_2")), None]
        );
        let buf = screen(&bp, &st, w, h);
        assert_eq!(
            inside(&buf, w, h, readout),
            "SOCKET L ▸ Coldfin vII · trace per cast −20%"
        );
        assert_eq!(inside(&buf, w, h, hint), SOCKET_HINT);
        let row = across(&buf, w, h, names);
        assert!(
            row.contains("COLDFIN VI ") && row.contains("SIDECAR"),
            "{row}"
        );
        assert!(!row.contains("COLDFIN VII"), "{row}");
        // The brackets are back on the socket.
        assert_eq!(buf[(ox - 5, oy + 5)].symbol(), "┏");
    }

    #[test]
    fn a_filled_socket_offers_empty_and_swaps() {
        let mut st = state();
        let mut bp = dshawns_deck(&mut st);
        let (w, h) = (100, 30);
        let (_, readout, _) = rows_of(w, h);
        let (names, lines) = tile_rows(w, h);
        let (ox, _) = deck_origin(w, h);
        let y = window(w, h).y + names;
        let space = KeyCode::Char(' ');
        press(
            &mut bp,
            &mut st,
            &[KeyCode::Char('m'), space, KeyCode::Right, space],
        );
        assert_eq!(
            fitted(&st, Host::Worn(Slot::Deck)),
            vec![Some(id("coldfin_2")), None]
        );
        press(&mut bp, &mut st, &[space]);
        assert!(choosing(&bp));
        let buf = screen(&bp, &st, w, h);
        let row = across(&buf, w, h, names);
        assert!(row.starts_with("HARDWARE   ▀▀ EMPTY"), "{row}");
        assert!(across(&buf, w, h, lines).contains("take it out"));
        // EMPTY on its own tile colours.
        let e = &buf[(ox + 16, y)];
        assert_eq!(e.symbol(), "E");
        assert_eq!(e.bg, Color::Rgb(0x2a, 0x28, 0x30));
        assert_eq!(e.fg, DIM);
        assert_eq!(
            inside(&buf, w, h, readout),
            "EMPTY ▸ socket L · Coldfin vII back to the pack"
        );
        press(&mut bp, &mut st, &[KeyCode::Right]);
        let buf = screen(&bp, &st, w, h);
        assert_eq!(
            inside(&buf, w, h, readout),
            "COLDFIN VI ▸ socket L · trace per cast −10% · Coldfin vII back to the pack"
        );
        // Space swaps: vI in, vII back to the pack.
        press(&mut bp, &mut st, &[space]);
        assert_eq!(
            status(&bp, &mut st),
            "Coldfin vI fitted · socket L · Coldfin vII to the pack"
        );
        assert_eq!(
            fitted(&st, Host::Worn(Slot::Deck)),
            vec![Some(id("coldfin_1")), None]
        );
        assert_eq!(
            (in_pack(&st, "coldfin_1"), in_pack(&st, "coldfin_2")),
            (0, 1)
        );
        // EMPTY takes it out.
        press(&mut bp, &mut st, &[space]);
        let buf = screen(&bp, &st, w, h);
        assert!(inside(&buf, w, h, readout).starts_with("EMPTY ▸ socket L"));
        press(&mut bp, &mut st, &[space]);
        assert!(!choosing(&bp));
        assert_eq!(status(&bp, &mut st), "Coldfin vI out · to the pack");
        assert_eq!(fitted(&st, Host::Worn(Slot::Deck)), vec![None, None]);
        assert_eq!(
            (in_pack(&st, "coldfin_1"), in_pack(&st, "coldfin_2")),
            (1, 1)
        );
        let buf = screen(&bp, &st, w, h);
        assert_eq!(
            inside(&buf, w, h, readout),
            "SOCKET L ▸ empty · takes deck hardware"
        );
    }

    #[test]
    fn esc_while_choosing_changes_nothing() {
        for back in [KeyCode::Esc, KeyCode::Char('q')] {
            let mut st = state();
            let mut bp = dshawns_deck(&mut st);
            let (w, h) = (100, 30);
            let (_, readout, hint) = rows_of(w, h);
            let before = st.character.clone();
            press(&mut bp, &mut st, &[KeyCode::Char('m'), KeyCode::Char(' ')]);
            press(&mut bp, &mut st, &[KeyCode::Right]);
            // Up, down, j, k and m do nothing while choosing.
            let buf = screen(&bp, &st, w, h);
            for code in [
                KeyCode::Up,
                KeyCode::Down,
                KeyCode::Char('j'),
                KeyCode::Char('k'),
                KeyCode::Char('m'),
                KeyCode::Enter,
            ] {
                assert!(matches!(press(&mut bp, &mut st, &[code]), ScreenCmd::Stay));
                assert!(modding(&bp) && choosing(&bp), "{code:?}");
                assert_eq!(buffer_text(&screen(&bp, &st, w, h)), buffer_text(&buf));
            }
            assert!(matches!(press(&mut bp, &mut st, &[back]), ScreenCmd::Stay));
            assert!(modding(&bp) && !choosing(&bp));
            assert_eq!(st.character, before);
            assert_eq!(bp.bench.point, 0);
            let buf = screen(&bp, &st, w, h);
            assert_eq!(inside(&buf, w, h, hint), SOCKET_HINT);
            assert_eq!(
                inside(&buf, w, h, readout),
                "SOCKET L ▸ empty · takes deck hardware"
            );
            // Not choosing, esc closes.
            assert!(matches!(
                press(&mut bp, &mut st, &[back]),
                ScreenCmd::Pop { .. }
            ));
        }
    }

    #[test]
    fn jacked_in_modding_only_looks() {
        let mut st = state();
        let mut bp = dshawns_deck(&mut st);
        jack_in(&mut st);
        let before = st.character.clone();
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        assert!(modding(&bp));
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert!(!choosing(&bp));
        assert!(last_is_warning(&st));
        assert_eq!(last(&st), Some(crate::actions::JACKED_IN));
        assert_eq!(
            status(&bp, &mut st),
            format!("! {}", crate::actions::JACKED_IN)
        );
        assert_eq!(st.character, before);
        let (_, _, hint) = rows_of(100, 30);
        assert_eq!(
            inside(&screen(&bp, &st, 100, 30), 100, 30, hint),
            SOCKET_HINT
        );
    }

    #[test]
    fn an_empty_tray_says_so() {
        let mut st = state();
        st.character.give_item(&st.content, id("rebuilt_deck"), 1);
        let copy = pack_copy(&st, "rebuilt_deck");
        perform(&mut st, Action::EquipCopy(copy)).unwrap();
        let mut bp = Blueprint::open(&st, id("rebuilt_deck"), true, Some(copy));
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        assert!(modding(&bp));
        let (names, _) = tile_rows(100, 30);
        let (_, _, hint) = rows_of(100, 30);
        let buf = screen(&bp, &st, 100, 30);
        let row = across(&buf, 100, 30, names);
        assert!(row.starts_with("HARDWARE"), "{row}");
        assert!(row.contains("nothing in your pack fits"), "{row}");
        let (ox, _) = deck_origin(100, 30);
        let y = window(100, 30).y + names;
        assert_eq!(buf[(ox + 13, y)].symbol(), "n");
        assert_eq!(buf[(ox + 13, y)].fg, DIM);
        let (log, before) = (st.log.len(), st.character.clone());
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert!(!choosing(&bp));
        assert_eq!(st.character, before);
        assert_eq!(st.log.len(), log + 1);
        assert_eq!(last(&st), Some("no deck hardware in your pack"));
        let buf = screen(&bp, &st, 100, 30);
        assert!(across(&buf, 100, 30, names).contains("nothing in your pack fits"));
        assert_eq!(inside(&buf, 100, 30, hint), SOCKET_HINT);
    }

    #[test]
    fn a_carried_deck_can_be_modded() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        st.character.give_item(&st.content, id("rebuilt_deck"), 1);
        let copy = pack_copy(&st, "rebuilt_deck");
        let mut bp = Blueprint::open(&st, id("rebuilt_deck"), false, Some(copy));
        let space = KeyCode::Char(' ');
        press(&mut bp, &mut st, &[KeyCode::Char('m'), space]);
        press(&mut bp, &mut st, &[KeyCode::Right, KeyCode::Right, space]);
        assert_eq!(
            status(&bp, &mut st),
            "Sidecar co-processor fitted · socket L"
        );
        assert_eq!(
            fitted(&st, Host::Pack(copy)),
            vec![Some(id("sidecar")), None]
        );
        assert_eq!(fitted(&st, Host::Worn(Slot::Deck)), vec![None, None]);
        assert_eq!(in_pack(&st, "sidecar"), 0);
    }

    #[test]
    fn the_modding_pointer_is_kept_across_m() {
        let mut st = state();
        let mut bp = dshawns_deck(&mut st);
        let (_, readout, _) = rows_of(100, 30);
        press(&mut bp, &mut st, &[KeyCode::Char('m'), KeyCode::Right]);
        assert_eq!(bp.bench.point, 1);
        press(&mut bp, &mut st, &[KeyCode::Char('m'), KeyCode::Char('m')]);
        assert!(modding(&bp));
        assert_eq!(bp.bench.point, 1);
        let buf = screen(&bp, &st, 100, 30);
        assert!(inside(&buf, 100, 30, readout).starts_with("SOCKET R "));
    }

    const RED_LENS: Color = Color::Rgb(0xff, 0x2a, 0x3c);
    const GOLD_BRIGHT: Color = Color::Rgb(0xe8, 0xc5, 0x4a);
    const ALU_MID: Color = Color::Rgb(0x7d, 0x88, 0x93);
    const WEAR: Color = Color::Rgb(0x8d, 0x93, 0x9c);
    const GUNMETAL: Color = Color::Rgb(0x2e, 0x31, 0x38);

    /// The window's body on a terminal of `w` by `h`: inside the frame, above the
    /// status and hint rows.
    fn body_of(w: u16, h: u16) -> Rect {
        let r = window(w, h);
        Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 4)
    }

    /// The gun's drawing origin in the window on a terminal of `w` by `h`.
    fn gun_origin(w: u16, h: u16) -> (u16, u16) {
        let art = ItemArt {
            stickers: Vec::new(),
            look: ItemLook::Ratchet,
            fitted: Vec::new(),
        };
        art.origin(body_of(w, h))
    }

    /// Vex's Ratchet, inspected from the rack.
    fn vexs_gun(st: &AppState) -> Blueprint {
        let copy = st.character.worn_copy(Slot::Primary).map(|k| k.id);
        Blueprint::open(st, id("ratchet_smg"), true, copy)
    }

    #[test]
    fn ratchet_inspects_its_drawing() {
        let mut st = state();
        let mut bp = vexs_gun(&st);
        let (w, h) = (100, 30);
        let (title, _, hint) = rows_of(w, h);
        let buf = screen(&bp, &st, w, h);
        let line = inside(&buf, w, h, title);
        assert!(line.contains(" BLUEPRINT · RATCHET SMG "), "{line}");
        assert_eq!(inside(&buf, w, h, hint), "m mod · s stickers · esc close");
        assert!(!buffer_text(&buf).contains("MISSING BLUEPRINT"));
        let (ox, oy) = gun_origin(w, h);
        assert_eq!((ox, oy), (6, 6));
        let at = |dx: u16, dy: u16| &buf[(ox + dx, oy + dy)];
        let thread = at(66, 5);
        assert_eq!((thread.symbol(), thread.fg), ("▄", GOLD_BRIGHT));
        let rail = at(34, 3);
        assert_eq!((rail.symbol(), rail.fg, rail.bg), ("▀", ALU, ALU_MID));
        let stamp = at(28, 6);
        assert_eq!((stamp.symbol(), stamp.fg, stamp.bg), ("R", WEAR, GUNMETAL));
        // Nothing fitted: the muzzle is blank.
        assert_eq!(at(74, 6).symbol(), " ");
        assert!(matches!(
            press(&mut bp, &mut st, &[KeyCode::Esc]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn ratchet_draws_its_fitted_pieces() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        let bp = vexs_gun(&st);
        for (socket, piece) in [(0, "hushcan"), (1, "redeye")] {
            let piece = pack_copy(&st, piece);
            let fit = Action::Fit {
                host: Host::Worn(Slot::Primary),
                socket,
                piece,
            };
            perform(&mut st, fit).unwrap();
        }
        let buf = screen(&bp, &st, 100, 30);
        let (ox, oy) = gun_origin(100, 30);
        assert_eq!(buf[(ox + 74, oy + 6)].symbol(), "H");
        let lens = &buf[(ox + 49, oy + 1)];
        assert_eq!((lens.symbol(), lens.fg), ("●", RED_LENS));
    }

    #[test]
    fn ratchet_modding() {
        let mut st = state();
        let mut bp = vexs_gun(&st);
        let (w, h) = (100, 30);
        let (title, readout, hint) = rows_of(w, h);
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        assert!(modding(&bp));
        let buf = screen(&bp, &st, w, h);
        let line = inside(&buf, w, h, title);
        assert!(line.contains(" MODDING · RATCHET SMG "), "{line}");
        assert_eq!(
            inside(&buf, w, h, hint),
            "↑↓←→ socket · space choose · m inspect · esc close"
        );
        assert_eq!(
            inside(&buf, w, h, readout),
            "SOCKET MUZZLE ▸ empty · takes muzzle hardware"
        );
        // The receiver greys; the thread, in the muzzle's socket, keeps its colour.
        let (ox, oy) = gun_origin(w, h);
        let stamp = &buf[(ox + 28, oy + 6)];
        assert_eq!(stamp.fg, grey(WEAR, PANEL_BG));
        assert_eq!(buf[(ox + 68, oy + 6)].bg, GOLD_BRIGHT);
        // The brackets sit one cell outside the pointed socket, inside the frame.
        let r = window(w, h);
        let frame = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2);
        let corner = &buf[(ox + 66 + 22, oy + 4 - 1)];
        assert_eq!(corner.symbol(), "┓");
        assert!(frame.contains((ox + 88, oy + 3).into()));

        press(&mut bp, &mut st, &[KeyCode::Up]);
        let buf = screen(&bp, &st, w, h);
        assert!(inside(&buf, w, h, readout).starts_with("SOCKET MUZZLE"));
        press(&mut bp, &mut st, &[KeyCode::Left]);
        let buf = screen(&bp, &st, w, h);
        assert!(inside(&buf, w, h, readout).starts_with("SOCKET RAIL"));
        let corner = &buf[(ox + 34 - 1, oy - 1)];
        assert_eq!(corner.symbol(), "┏");
        assert!(frame.contains((ox + 33, oy - 1).into()));
    }

    #[test]
    fn ratchet_fits_the_window_and_clears_the_band() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        for (w, h) in [(100, 30), (120, 40)] {
            let r = window(w, h);
            let inner = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2);
            let body = body_of(w, h);
            let above = Rect::new(inner.x, inner.y, inner.width, body.bottom() - 5 - inner.y);
            let (ox, oy) = gun_origin(w, h);
            let fitted = [Some(HardwareLook::Hushcan), Some(HardwareLook::Redeye)];
            for fitted in [Vec::new(), fitted.to_vec()] {
                // Drawn on the whole terminal, nothing may land outside the room.
                let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
                let art = ItemArt {
                    stickers: Vec::new(),
                    look: ItemLook::Ratchet,
                    fitted,
                };
                art.draw((ox, oy), buf.area, &mut buf);
                let blank = Buffer::empty(buf.area);
                for y in 0..h {
                    for x in 0..w {
                        if buf[(x, y)] != blank[(x, y)] {
                            assert!(above.contains((x, y).into()), "{w}x{h} ({x},{y})");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ratchet_with_one_socket_offers_one_place() {
        let mut st = state();
        for item in &mut st.content.items {
            if item.id == id("ratchet_smg") {
                item.sockets = vec![game::SocketKind::Muzzle];
            }
        }
        let mut bp = vexs_gun(&st);
        let (w, h) = (100, 30);
        let (_, readout, _) = rows_of(w, h);
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        assert!(modding(&bp));
        for code in [KeyCode::Left, KeyCode::Up, KeyCode::Right, KeyCode::Down] {
            press(&mut bp, &mut st, &[code]);
            let buf = screen(&bp, &st, w, h);
            let line = inside(&buf, w, h, readout);
            assert!(line.starts_with("SOCKET MUZZLE"), "{line}");
        }
    }

    #[test]
    fn ratchet_without_a_copy() {
        let mut st = state();
        st.character.give(id("ratchet_smg"), 1);
        let mut bp = Blueprint::open(&st, id("ratchet_smg"), false, None);
        let (w, h) = (100, 30);
        let (_, _, hint) = rows_of(w, h);
        let buf = screen(&bp, &st, w, h);
        assert_eq!(inside(&buf, w, h, hint), "esc close");
        let (ox, oy) = gun_origin(w, h);
        assert_eq!(buf[(ox + 28, oy + 6)].symbol(), "R");
        assert!(matches!(
            press(&mut bp, &mut st, &[KeyCode::Char('m')]),
            ScreenCmd::Stay
        ));
        assert!(!modding(&bp));
    }

    /// The Keyjack's drawing origin in the window on a terminal of `w` by `h`.
    fn keyjack_origin(w: u16, h: u16) -> (u16, u16) {
        let art = ItemArt {
            stickers: Vec::new(),
            look: ItemLook::Keyjack,
            fitted: Vec::new(),
        };
        art.origin(body_of(w, h))
    }

    const KJ_LCD: Color = Color::Rgb(0x06, 0x18, 0x0e);
    const KJ_PCB: Color = Color::Rgb(0x0f, 0x3a, 0x24);
    const KJ_GLASS: Color = Color::Rgb(0x9a, 0xd4, 0xdc);
    const KJ_LIT: Color = Color::Rgb(0x5a, 0xff, 0x96);

    #[test]
    fn modding_the_keyjack_draws_its_back() {
        let mut st = state();
        let copy = pack_copy(&st, "keyjack");
        let mut bp = Blueprint::open(&st, id("keyjack"), false, Some(copy));
        let (w, h) = (100, 30);
        let (title, readout, hint) = rows_of(w, h);
        let (ox, oy) = keyjack_origin(w, h);
        let at = |buf: &Buffer, dx: u16, dy: u16| buf[(ox + dx, oy + dy)].clone();
        // Inspect draws the front: the LCD, its strike key, the fuse light lit.
        let buf = screen(&bp, &st, w, h);
        assert!(inside(&buf, w, h, title).contains(" BLUEPRINT · KEYJACK "));
        assert_eq!(inside(&buf, w, h, hint), "m mod · esc close", "no stickers");
        let text = buffer_text(&buf);
        assert!(text.contains("KEYJACK") && text.contains("READY"), "{text}");
        assert!(text.contains("STRIKE") && !text.contains("KJ4"), "{text}");
        assert_eq!(at(&buf, 30, 6).bg, KJ_LCD);
        assert_eq!(at(&buf, 43, 3).fg, KJ_LIT);
        let front = buf;
        // Modding draws the back, opened and in its own colours, the fuse in its holder
        // and the holder boxed.
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        assert!(modding(&bp));
        let buf = screen(&bp, &st, w, h);
        assert!(inside(&buf, w, h, title).contains(" MODDING · KEYJACK "));
        assert_eq!(
            inside(&buf, w, h, readout),
            "SOCKET FUSE ▸ Stock fuse · no bonus"
        );
        let text = buffer_text(&buf);
        assert!(text.contains("KJ4") && text.contains("3V"), "{text}");
        assert!(!text.contains("READY"), "{text}");
        assert!(
            text.contains("STOCK FUSE"),
            "the spares in the tray: {text}"
        );
        assert_eq!(at(&buf, 17, 4).bg, KJ_PCB, "the board is not greyed");
        assert_eq!(at(&buf, 28, 10).bg, KJ_GLASS, "the fuse in its holder");
        assert_eq!(at(&buf, 28, 9).symbol(), "╌");
        assert_eq!(at(&buf, 20, 11).symbol(), "╎");
        // The front never draws the fitted fuse: where it sits on the back, the front
        // has its own drawing.
        assert_ne!(at(&front, 28, 10).bg, KJ_GLASS);
        // Out comes the fuse: the holder draws empty and the fuse light goes dark.
        let out = Action::Unfit {
            host: Host::Pack(copy),
            socket: 0,
        };
        perform(&mut st, out).unwrap();
        let buf = screen(&bp, &st, w, h);
        assert_eq!(at(&buf, 28, 10).bg, KJ_PCB);
        assert_eq!(
            inside(&buf, w, h, readout),
            "SOCKET FUSE ▸ empty · takes fuses"
        );
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        let buf = screen(&bp, &st, w, h);
        assert!(buffer_text(&buf).contains("READY"));
        assert_ne!(at(&buf, 43, 3).fg, KJ_LIT);
    }

    #[test]
    fn the_fuse_has_a_drawing() {
        let mut st = state();
        for (fuse, name) in [
            ("fuse_stock", "STOCK FUSE"),
            ("fuse_ceramic", "CERAMIC FUSE"),
            ("fuse_slowblow", "SLOW-BLOW FUSE"),
            ("fuse_wideband", "WIDEBAND FUSE"),
        ] {
            st.character.give_item(&st.content, id(fuse), 1);
            let copy = pack_copy(&st, fuse);
            let bp = Blueprint::open(&st, id(fuse), false, Some(copy));
            let buf = screen(&bp, &st, 100, 30);
            let line = inside(&buf, 100, 30, 0);
            assert!(line.contains(&format!(" BLUEPRINT · {name} ")), "{line}");
            assert!(!buffer_text(&buf).contains(MISSING), "{fuse}");
        }
    }

    /// The names on the tray's tiles, left to right.
    fn tray_names(bp: &Blueprint, st: &AppState) -> Vec<String> {
        let (names, _) = tile_rows(100, 30);
        let row = across(&screen(bp, st, 100, 30), 100, 30, names);
        row.split("  ")
            .map(|t| t.trim().trim_start_matches(|c: char| !c.is_alphanumeric()))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty() && t != "HARDWARE")
            .collect()
    }

    fn dshawns_gun(st: &mut AppState) -> Blueprint {
        st.character = Character::debug_runner(&st.content);
        st.tick = 40;
        vexs_gun(st)
    }

    #[test]
    fn the_tray_offers_what_fits_the_chosen_socket() {
        let mut st = state();
        let mut bp = dshawns_gun(&mut st);
        let (w, h) = (100, 30);
        let (_, readout, _) = rows_of(w, h);
        let space = KeyCode::Char(' ');
        press(&mut bp, &mut st, &[KeyCode::Char('m'), space]);
        assert!(choosing(&bp));
        let names = tray_names(&bp, &st);
        assert!(names.iter().any(|n| n.starts_with("HUSHCAN")), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("REDEYE")), "{names:?}");
        press(&mut bp, &mut st, &[space]);
        assert_eq!(status(&bp, &mut st), "Hushcan fitted · socket MUZZLE");
        let buf = screen(&bp, &st, w, h);
        assert_eq!(
            inside(&buf, w, h, readout),
            "SOCKET MUZZLE ▸ Hushcan · cool +1"
        );
        press(&mut bp, &mut st, &[KeyCode::Left, space]);
        let names = tray_names(&bp, &st);
        assert!(names.iter().any(|n| n.starts_with("REDEYE")), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("HUSHCAN")), "{names:?}");
        press(&mut bp, &mut st, &[space]);
        assert_eq!(status(&bp, &mut st), "Redeye sight fitted · socket RAIL");
        // The filled muzzle offers EMPTY first.
        press(&mut bp, &mut st, &[KeyCode::Right, space]);
        let names = tray_names(&bp, &st);
        assert!(names[0].starts_with("EMPTY"), "{names:?}");
        press(&mut bp, &mut st, &[space]);
        assert_eq!(status(&bp, &mut st), "Hushcan out · to the pack");
        assert_eq!(in_pack(&st, "hushcan"), 1);
    }

    #[test]
    fn an_empty_socket_with_nothing_to_fit_says_so_in_the_log() {
        let mut st = state();
        let mut bp = dshawns_gun(&mut st);
        st.character.inventory.retain(|s| s.item != id("hushcan"));
        press(&mut bp, &mut st, &[KeyCode::Char('m'), KeyCode::Char(' ')]);
        assert!(!choosing(&bp));
        assert_eq!(last(&st), Some("no muzzle hardware in your pack"));
        assert!(!last_is_warning(&st));
    }

    #[test]
    fn the_tray_not_choosing_shows_what_fits_the_item() {
        let mut st = state();
        let mut bp = dshawns_gun(&mut st);
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        let names = tray_names(&bp, &st);
        assert!(names.iter().any(|n| n.starts_with("HUSHCAN")), "{names:?}");
        assert!(names.iter().any(|n| n.starts_with("REDEYE")), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("COLDFIN")), "{names:?}");
        let mut bp = dshawns_deck(&mut st);
        press(&mut bp, &mut st, &[KeyCode::Char('m')]);
        // Not choosing the tray shows the first tiles; choosing, it scrolls to them all.
        let names = tray_names(&bp, &st);
        assert_eq!(names, ["COLDFIN VI", "COLDFIN VII"]);
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        let mut all = tray_names(&bp, &st);
        for _ in 0..2 {
            press(&mut bp, &mut st, &[KeyCode::Right]);
            all.extend(tray_names(&bp, &st));
        }
        for want in ["COLDFIN VI", "COLDFIN VII", "SIDECAR CO-PROCESSOR"] {
            assert!(all.iter().any(|n| n == want), "{want} in {all:?}");
        }
        let gun = |n: &String| n.contains("HUSHCAN") || n.contains("REDEYE");
        assert!(!all.iter().any(gun), "{all:?}");
    }

    #[test]
    fn a_jacked_in_gun_refuses_a_fit() {
        let mut st = state();
        let mut bp = dshawns_gun(&mut st);
        jack_in(&mut st);
        let before = st.character.clone();
        press(&mut bp, &mut st, &[KeyCode::Char('m'), KeyCode::Char(' ')]);
        assert!(!choosing(&bp));
        assert_eq!(last(&st), Some(crate::actions::JACKED_IN));
        assert!(last_is_warning(&st));
        assert_eq!(st.character, before);
    }

    #[test]
    fn the_preview_draws_what_the_window_draws() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        for (socket, piece) in [(0, "hushcan"), (1, "redeye")] {
            let piece = pack_copy(&st, piece);
            let host = Host::Worn(Slot::Primary);
            perform(
                &mut st,
                Action::Fit {
                    host,
                    socket,
                    piece,
                },
            )
            .unwrap();
        }
        let shape = |art: Option<Box<dyn Art>>| {
            let art = art.expect("a drawing");
            (art.extent(), crate::ui::preview::half(art.as_ref()).cells())
        };
        let gun = st.character.worn_copy(Slot::Primary).map(|k| k.id);
        let got = shape(preview_art(&st, &id("ratchet_smg"), true, gun));
        assert_eq!(got, ((0, 0, 88, 15), (44, 8)));
        let cold = Some(pack_copy(&st, "coldfin_1"));
        let got = shape(preview_art(&st, &id("coldfin_1"), false, cold));
        assert_eq!(got, ((0, 0, 52, 18), (26, 9)));
        let got = shape(preview_art(&st, &id("pawnshop_deck"), false, None));
        assert_eq!(got, ((0, 0, 84, 22), (42, 11)));
        assert!(preview_art(&st, &id("scrap_servo"), false, None).is_none());
    }

    #[test]
    fn the_decks_preview_takes_its_cradles_and_not_its_cables() {
        let mut st = state();
        st.character = Character::debug_runner(&st.content);
        let copy = st.character.worn_copy(Slot::Deck).map(|k| k.id);
        let art = preview_art(&st, &id("rebuilt_deck"), true, copy).unwrap();
        assert_eq!(art.extent(), (-4, 0, 92, 22));
        assert_eq!(crate::ui::preview::half(art.as_ref()).cells(), (46, 11));
        let whole = Rect::new(0, 0, 140, 30);
        let mut buf = Buffer::empty(whole);
        art.draw((30, 4), whole, &mut buf);
        let outside = |buf: &Buffer| {
            (0..30).any(|y| {
                (0..140)
                    .filter(|x| *x < 26 || *x > 117)
                    .any(|x| buf[(x, y)] != Cell::default())
            })
        };
        assert!(
            !outside(&buf),
            "the preview reaches only as far as its cradles"
        );
        // The window's own drawing runs the player's cable out to the left.
        let tab = DeckTab::for_deck(st.tick, id("rebuilt_deck"), true, copy, &st);
        let mut buf = Buffer::empty(whole);
        tab.art(&st).draw((30, 4), whole, &mut buf);
        assert!(outside(&buf), "the worn deck's cable is drawn");
    }
}
