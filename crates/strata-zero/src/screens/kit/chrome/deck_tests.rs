//! CHROME driven through the deck: the rules the ATTRIBUTES ledger had (the chair, the
//! bay anywhere, the tab's free piece, the fee line, over budget, `x` asks first, the
//! picker), the tiles, LOAD over the divider, and `c`.

use super::rules::{Pick, fitting, load_after};
use super::{Stop, chrome_layout, stops};
use crate::actions::{JACKED_IN, perform};
use crate::app::{AppState, Screen, ScreenCmd};
use crate::clinic::{OUTSIDE, Operating, operating};
use crate::messages::Kind;
use crate::screens::deck::{Deck, REVEAL_TICKS, Tab};
use crate::screens::kit::{Kit, Pane, View};
use crate::screens::testutil::key;
use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT};
use crate::ui::hud::{AMBER, BADGE_FG, GREEN};
use citygen::{GenParams, Generator, Place, Pos, SizePreset, StratumKind};
use crossterm::event::KeyCode;
use game::{Action, BodyPart, CopyId, Harm, ItemId, Site, SocketKind, Stack};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// The deck's body at 100×30.
const BODY: Rect = Rect::new(1, 5, 98, 22);
/// At 100×30: the divider right of the compact body, the right pane's first column and
/// the column past its last, and the card's rows under LOAD and the divider.
const DIV: u16 = 49;
const P0: u16 = 51;
const P1: u16 = 98;
const CARD: (u16, u16) = (8, 27);
/// The same at 120×40.
const DIV_W: u16 = 63;
const P0_W: u16 = 65;
const P1_W: u16 = 118;
const CARD_W: (u16, u16) = (13, 37);

fn id(s: &str) -> ItemId {
    ItemId::new(s)
}

fn last(st: &AppState) -> Option<&str> {
    st.log.latest().map(|e| e.text.as_str())
}

/// One of `item` into the pack; its copy when it is held per copy.
fn give(st: &mut AppState, item: &str) -> Option<CopyId> {
    st.character.give_item(&st.content, id(item), 1);
    st.character
        .inventory
        .iter()
        .rev()
        .find(|s| s.item == id(item))
        .and_then(|s| s.copy_id())
}

/// Installs `item` at `site`, as a copy when it is held per copy, with `fitted` in
/// its sockets.
fn install(st: &mut AppState, site: Site, item: &str, fitted: &[(usize, &str)]) {
    let per_copy = st.content.item(&id(item)).unwrap().per_copy();
    let mut copy = None;
    if per_copy {
        let mut k = st.character.new_copy(&st.content, &id(item));
        for &(n, piece) in fitted {
            let pc = st.character.new_copy(&st.content, &id(piece));
            k.fitted[n] = Some(Stack {
                item: id(piece),
                qty: 1,
                copy: Some(pc),
            });
        }
        copy = Some(k);
    }
    st.character.install(site, id(item), copy);
}

/// The spec's loadout: a piece at every site but EARS, R ARM and LEGS, the Scout
/// drone in the cradle's bay, and the Piston arm at L ARM with the Arc hand in its
/// hand.
fn loadout(st: &mut AppState) {
    for (site, item) in [
        (Site::Neural, "neural_port"),
        (Site::Eyes, "target_lens"),
        (Site::Jaw, "bulldog_jaw"),
        (Site::Spine, "redline_spine"),
        (Site::Organs, "adrenal_pump"),
        (Site::Skin, "subdermal_mesh"),
    ] {
        install(st, site, item, &[]);
    }
    install(st, Site::Back, "drone_cradle", &[(0, "scout_drone")]);
    install(st, Site::LeftArm, "piston_arm", &[(0, "arc_hand")]);
}

/// Jacked in: the avatar in a construct, the body on the street.
fn jacked() -> AppState {
    let mut st = AppState::new();
    let city = Generator::run(GenParams {
        seed: 6,
        size: SizePreset::Custom { w: 120, h: 60 },
        districts: 5,
        ..Default::default()
    })
    .unwrap();
    let b = city.buildings[0].id;
    let mut p = crate::walker::Player::at(Place::Construct(b), Pos::new(1, 1));
    p.body = Some((Place::Stratum(StratumKind::Street), Pos::new(1, 1)));
    st.player = Some(p);
    st.city = Some(city);
    st
}

/// The deck on GEAR & INVENTORY, switched to CHROME with `c`, the cursor on `stop`, free
/// chrome on: these test CHROME's rules, the chair has its own.
fn on_chrome(st: &mut AppState, stop: Stop) -> Deck {
    st.free_chrome = true;
    let mut d = Deck::new(st.tick);
    d.update(st, &[key(KeyCode::Char('3')), key(KeyCode::Char('c'))]);
    assert_eq!((d.tab, d.kit.state.view), (Tab::Kit, View::Chrome));
    d.kit.state.stop = stop;
    d
}

/// Sat in a doc's chair with the spec's loadout and `credits`, the deck opened by the
/// chair, the cursor moved to `stop`.
fn in_chair(credits: u32, stop: Stop) -> (AppState, Deck) {
    let mut st = crate::clinic::testutil::seated();
    loadout(&mut st);
    st.character.credits = credits;
    let mut d = Deck::chair(&st);
    d.kit.state.stop = stop;
    (st, d)
}

/// Outside any chair (inside a clinic, not seated), the spec's loadout with the Scout
/// drone docked and a Mule drone in the bag, the deck on CHROME's `stop`.
fn locked_on(stop: Stop) -> (AppState, Deck) {
    let (mut st, _, _, _) = crate::clinic::testutil::inside();
    loadout(&mut st);
    give(&mut st, "mule_drone");
    let mut d = Deck::new(st.tick);
    d.update(&mut st, &[key(KeyCode::Char('3')), key(KeyCode::Char('c'))]);
    d.kit.state.stop = stop;
    (st, d)
}

const BAY: Stop = Stop::Socket(Site::Back, 0);

fn hints(d: &Deck, st: &AppState) -> String {
    d.kit.hint_line(st).to_string()
}

fn hint_line(d: &Deck, st: &mut AppState) -> String {
    cells(&screen(d, st, (100, 30)), 2, 28, 96)
        .trim_end()
        .to_string()
}

fn press(d: &mut Deck, st: &mut AppState, codes: &[KeyCode]) {
    let keys: Vec<_> = codes.iter().map(|&c| key(c)).collect();
    d.update(st, &keys);
}

/// The deck at `w`×`h` once its card and the strip have typed out, clear of the
/// deck's signal jitter.
fn screen(d: &Deck, st: &mut AppState, (w, h): (u16, u16)) -> Buffer {
    let typed = d.kit.changed + REVEAL_TICKS + 1;
    st.tick = (st.tick + 25).max(typed);
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| d.draw(st, f)).unwrap();
    t.backend().buffer().clone()
}

/// `n` cells of row `y` from column `x`.
fn cells(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
    (x..x + n).map(|cx| buf[(cx, y)].symbol()).collect()
}

/// Columns `x0..x1` of rows `y0..y1`, a line each.
fn region(buf: &Buffer, (x0, x1): (u16, u16), (y0, y1): (u16, u16)) -> String {
    (y0..y1)
        .map(|y| cells(buf, x0, y, x1 - x0).trim_end().to_string() + "\n")
        .collect()
}

/// The card's text at 100×30.
fn card(buf: &Buffer) -> String {
    region(buf, (P0, P1), CARD)
}

/// The column in row `y`, from `x0`, where `needle` starts.
fn col_of(buf: &Buffer, y: u16, x0: u16, needle: &str) -> Option<u16> {
    let want: Vec<String> = needle.chars().map(String::from).collect();
    let n = want.len() as u16;
    (x0..buf.area.right().saturating_sub(n))
        .find(|&x| (0..n).all(|k| buf[(x + k, y)].symbol() == want[k as usize]))
}

/// The row, `y0..y1`, whose cells from `x0` hold `needle`, and where it starts.
fn find(buf: &Buffer, x0: u16, (y0, y1): (u16, u16), needle: &str) -> Option<(u16, u16)> {
    (y0..y1).find_map(|y| col_of(buf, y, x0, needle).map(|x| (x, y)))
}

/// Where CHROME draws `stop`'s tile in the deck at `w`×`h`.
fn tile(st: &AppState, stop: Stop, (w, h): (u16, u16)) -> Rect {
    let top = if h >= 36 { 10 } else { 5 };
    let body = Rect::new(1, top, w - 2, h - top - 3);
    chrome_layout(body, st).tile(stop).expect("a tile")
}

/// The text inside a tile, a line each.
fn inside(buf: &Buffer, r: Rect) -> String {
    region(buf, (r.x + 1, r.right() - 1), (r.y + 1, r.bottom() - 1))
}

#[test]
fn the_mark_is_on_the_header_line() {
    let (mut st, d) = in_chair(1000, Stop::Site(Site::LeftArm));
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(cells(&buf, 2, 5, 27), "MEAT ▌CHROME▐ · CHROMING UP");
    assert_eq!(buf[(16, 5)].fg, AMBER);
    assert!(buf[(16, 5)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(DIV, 12)].fg, AMBER, "the divider");
    assert!(region(&buf, (0, 100), (6, 30)).matches("CHROMING").count() == 0);
    // Free chrome: the DEV badge's colours.
    st.free_chrome = true;
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(cells(&buf, 2, 5, 20), "MEAT ▌CHROME▐  FREE ");
    assert_eq!((buf[(17, 5)].fg, buf[(17, 5)].bg), (BADGE_FG, AMBER));
    assert_eq!(buf[(DIV, 12)].fg, AMBER);
    // Anywhere else: no mark, the divider as it was.
    st.free_chrome = false;
    st.chair = None;
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(cells(&buf, 2, 5, 23), "MEAT ▌CHROME▐  c switch");
    assert_eq!(buf[(DIV, 12)].fg, RULE);
}

#[test]
fn hints_in_the_chair() {
    let (mut st, d) = in_chair(1000, Stop::Site(Site::LeftArm));
    let line = "←↑↓→ select · space what fits · x pull it out ¤110 · c meat/chrome · esc get out of the chair";
    assert_eq!(hints(&d, &st), line);
    assert_eq!(hint_line(&d, &mut st), line);
    let buf = screen(&d, &mut st, (100, 30));
    let x = col_of(&buf, 28, 2, "¤110").unwrap();
    assert_eq!(buf[(x, 28)].fg, AMBER);
}

#[test]
fn hints_anywhere_else() {
    let (mut st, _, _, _) = crate::clinic::testutil::inside();
    loadout(&mut st);
    give(&mut st, "grip_arm");
    let mut d = Deck::new(st.tick);
    d.update(&mut st, &[key(KeyCode::Char('3')), key(KeyCode::Char('c'))]);
    d.kit.state.stop = Stop::Site(Site::LeftArm);
    assert_eq!(
        hints(&d, &st),
        "←↑↓→ select · c meat/chrome · Q/E tabs · tab close   chrome goes in and out in a ripperdoc's chair"
    );
    let buf = screen(&d, &mut st, (100, 30));
    let x = col_of(&buf, 28, 2, "chrome goes").unwrap();
    assert_eq!(buf[(x, 28)].fg, FAINTER);
    let before = st.character.clone();
    for code in [KeyCode::Char(' '), KeyCode::Char('x')] {
        press(&mut d, &mut st, &[code]);
        assert_eq!(last(&st), Some(OUTSIDE), "{code:?}");
        assert_eq!(d.kit.state.picker, None);
    }
    assert_eq!(st.character, before);
}

#[test]
fn a_bay_docks_and_stows_anywhere() {
    let (mut st, mut d) = locked_on(BAY);
    assert_eq!(operating(&st), Operating::Locked);
    let credits = st.character.credits;
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(
        last(&st),
        Some("Mule drone fitted · socket bay · Scout drone to the pack")
    );
    assert_eq!(
        st.character.docked(&st.content),
        vec![(Site::Back, 0, id("mule_drone"))]
    );
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(last(&st), Some("Mule drone out · to the pack"));
    assert!(st.character.docked(&st.content).is_empty());
    assert_eq!(st.character.credits, credits, "no fee");
}

#[test]
fn a_bay_stays_locked_jacked_in() {
    let mut st = jacked();
    loadout(&mut st);
    give(&mut st, "mule_drone");
    let mut d = on_chrome(&mut st, BAY);
    let before = st.character.clone();
    for code in [KeyCode::Char(' '), KeyCode::Char('x')] {
        press(&mut d, &mut st, &[code]);
        assert_eq!(last(&st), Some(JACKED_IN), "{code:?}");
    }
    assert_eq!(st.character, before);
}

#[test]
fn the_cradle_itself_still_needs_the_chair() {
    let (mut st, mut d) = locked_on(Stop::Site(Site::Back));
    let before = st.character.clone();
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(last(&st), Some(OUTSIDE));
    assert_eq!(st.character, before);
}

#[test]
fn hints_on_a_bay() {
    let (mut st, d) = locked_on(BAY);
    assert_eq!(
        hints(&d, &st),
        "←↑↓→ select · space swap · x stow in the bag · c meat/chrome · Q/E tabs   drones dock anywhere"
    );
    let buf = screen(&d, &mut st, (100, 30));
    let x = col_of(&buf, 28, 2, "drones dock").unwrap();
    assert_eq!(buf[(x, 28)].fg, FAINTER);
    st.character
        .chrome_copies
        .get_mut(&Site::Back)
        .unwrap()
        .fitted[0] = None;
    assert_eq!(
        hints(&d, &st),
        "←↑↓→ select · space dock · c meat/chrome · Q/E tabs · tab close   drones dock anywhere"
    );
}

#[test]
fn the_bay_card_is_the_drones() {
    let mut st = AppState::new();
    loadout(&mut st);
    let d = on_chrome(&mut st, BAY);
    let buf = screen(&d, &mut st, (100, 30));
    let card = card(&buf);
    for want in [
        "SCOUT DRONE",
        "docked in the Drone cradle",
        "grants Scout",
        "out with you · follows on foot",
        "when it's bored.",
    ] {
        assert!(card.contains(want), "{want}:\n{card}");
    }
    for not in ["load", "EMP"] {
        assert!(!card.contains(not), "{not}:\n{card}");
    }
    // Its drawing leads the card, in half blocks.
    let art = region(&buf, (P0, P1), (CARD.0, CARD.0 + 6));
    assert!(art.contains('▀') || art.contains('▄'), "{art}");
}

#[test]
fn the_cradles_card_shows_its_bay() {
    let mut st = AppState::new();
    loadout(&mut st);
    let d = on_chrome(&mut st, Stop::Site(Site::Back));
    let text = card(&screen(&d, &mut st, (100, 30)));
    assert!(text.contains("■ bay     Scout drone"), "{text}");
    st.character
        .chrome_copies
        .get_mut(&Site::Back)
        .unwrap()
        .fitted[0] = None;
    let text = card(&screen(&d, &mut st, (100, 30)));
    assert!(text.contains("□ bay     empty"), "{text}");
    assert!(
        text.contains("Scout drone, Mule"),
        "what docks there:\n{text}"
    );
}

#[test]
fn a_cradle_carrying_a_drone_costs_one_piece_of_emp() {
    let mut st = AppState::new();
    loadout(&mut st);
    let mut d = on_chrome(&mut st, Stop::Site(Site::Back));
    // Out with its drone in it, then picked to go back in.
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert!(st.character.chrome_at(Site::Back).is_none());
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    let text = card(&screen(&d, &mut st, (100, 30)));
    assert!(text.contains("EMP −2"), "{text}");
    assert!(!text.contains("EMP −4"), "{text}");
}

#[test]
fn the_bay_picker_shows_what_a_drone_does() {
    let (mut st, mut d) = locked_on(BAY);
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    let (_, y) = find(&buf, P0, CARD, "Mule drone").unwrap();
    let row = cells(&buf, P0, y, P1 - P0);
    assert!(row.trim_end().ends_with("carry +8"), "{row}");
    let text = card(&buf);
    assert!(!text.contains("L0"), "{text}");
    assert!(!text.contains("LOAD"), "{text}");
}

/// The card's fee line at 100×30: its row and text.
fn fee_row(buf: &Buffer) -> Option<(u16, String)> {
    (CARD.0..CARD.1)
        .map(|y| (y, cells(buf, P0, y, P1 - P0)))
        .find(|(_, t)| t.starts_with("fee "))
}

/// `item` on Castellan's tab, its doc `doc`.
fn on_the_tab(st: &mut AppState, doc: sim::PersonId, item: &str) {
    st.missions.tab = Some(crate::missions::Tab {
        doc,
        item: id(item),
        payer: "Castellan".into(),
        spent: false,
    });
}

fn spent(st: &AppState) -> bool {
    st.missions.tab.as_ref().is_some_and(|t| t.spent)
}

#[test]
fn the_tab_piece_goes_in_free_once() {
    // Short of the fee, the grip arm on the tab of the doc you sit with.
    let (mut st, mut d) = in_chair(50, Stop::Site(Site::RightArm));
    let doc = st.chair.unwrap().doc;
    on_the_tab(&mut st, doc, "grip_arm");
    give(&mut st, "grip_arm");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    // The card is 47 wide at 100×30: the line fits.
    let (y, text) = fee_row(&buf).expect("a fee line");
    assert_eq!(text.trim_end(), "fee TAB · CASTELLAN · ¤370 on the tab");
    assert_eq!(buf[(P0, y)].fg, DIM);
    let tab = buf[(P0 + 4, y)].clone();
    assert_eq!(tab.fg, AMBER);
    assert!(tab.modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(P0 + 18, y)].fg, AMBER, "the name");
    assert_eq!(buf[(P0 + 22, y)].fg, DIM, "the amount");
    // At 120×40, as the mock: one row.
    let wide = screen(&d, &mut st, (120, 40));
    let text = region(&wide, (P0_W, P1_W), CARD_W);
    assert!(
        text.contains("fee TAB · CASTELLAN · ¤370 on the tab"),
        "{text}"
    );
    // Space isn't dimmed: the tab pays.
    let x = col_of(&buf, 28, 2, "space install").unwrap();
    assert_eq!(buf[(x, 28)].fg, DIM);
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(
        st.character.chrome_at(Site::RightArm),
        Some(&id("grip_arm"))
    );
    assert_eq!(st.character.credits, 50);
    assert!(spent(&st));
    let said = last(&st).unwrap();
    assert!(said.ends_with(" · ¤370 on the tab"), "{said}");
    // Once: a second one pays the doc.
    st.character.credits = 1200;
    give(&mut st, "grip_arm");
    d.kit.state.stop = Stop::Site(Site::LeftArm);
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(
        fee_row(&buf).unwrap().1.trim_end(),
        "fee ¤480 · in ¤370 · out ¤110"
    );
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(st.character.chrome_at(Site::LeftArm), Some(&id("grip_arm")));
    assert_eq!(st.character.credits, 720);
}

#[test]
fn another_piece_pays_and_the_tab_waits() {
    let (mut st, mut d) = in_chair(1200, Stop::Site(Site::RightArm));
    let doc = st.chair.unwrap().doc;
    on_the_tab(&mut st, doc, "grip_arm");
    give(&mut st, "surgeon_arm");
    let fee = game::put_in_fee(&st.content, &id("surgeon_arm"));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(
        fee_row(&buf).unwrap().1.trim_end(),
        format!("fee ¤{fee} · leaves ¤{}", 1200 - fee)
    );
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(
        st.character.chrome_at(Site::RightArm),
        Some(&id("surgeon_arm"))
    );
    assert_eq!(st.character.credits, 1200 - fee);
    assert!(!spent(&st), "the tab waits");
    // Its own piece, a swap at L ARM: in and out, both on the tab.
    give(&mut st, "grip_arm");
    d.kit.state.stop = Stop::Site(Site::LeftArm);
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(
        fee_row(&buf).unwrap().1.trim_end(),
        "fee TAB · CASTELLAN · ¤480 on the tab"
    );
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(st.character.credits, 1200 - fee);
    assert!(spent(&st));
}

#[test]
fn the_tab_piece_pays_at_another_doc() {
    let (mut st, mut d) = in_chair(1200, Stop::Site(Site::RightArm));
    let doc = st.chair.unwrap().doc;
    on_the_tab(&mut st, sim::PersonId(doc.0 + 1), "grip_arm");
    give(&mut st, "grip_arm");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(
        fee_row(&buf).unwrap().1.trim_end(),
        "fee ¤370 · leaves ¤830"
    );
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(
        st.character.chrome_at(Site::RightArm),
        Some(&id("grip_arm"))
    );
    assert_eq!(st.character.credits, 830);
    assert!(!spent(&st), "not her chair");
}

#[test]
fn the_fee_line() {
    // Into an empty place: what it leaves you.
    let (mut st, mut d) = in_chair(1200, Stop::Site(Site::RightArm));
    give(&mut st, "grip_arm");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    let (y, text) = fee_row(&buf).expect("a fee line");
    assert_eq!(text.trim_end(), "fee ¤370 · leaves ¤830");
    assert!(cells(&buf, P0, y - 1, 30).starts_with("load 19 → 22/24"));
    assert_eq!(buf[(P0 + 4, y)].fg, AMBER);
    // A swap: in and out.
    let (mut st, mut d) = in_chair(1200, Stop::Site(Site::LeftArm));
    give(&mut st, "grip_arm");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(
        fee_row(&buf).unwrap().1.trim_end(),
        "fee ¤480 · in ¤370 · out ¤110"
    );
    // Short: magenta, space dimmed and refused, the picker still open.
    let (mut st, mut d) = in_chair(320, Stop::Site(Site::RightArm));
    give(&mut st, "grip_arm");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    let (y, text) = fee_row(&buf).unwrap();
    assert_eq!(text.trim_end(), "fee ¤370 · you have ¤320");
    assert_eq!(buf[(P0 + 4, y)].fg, MAGENTA);
    assert_eq!(buf[(P0 + 12, y)].fg, MAGENTA);
    let x = col_of(&buf, 28, 2, "space install").unwrap();
    assert_eq!(buf[(x, 28)].fg, FAINTER);
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    assert_eq!(last(&st), Some(crate::actions::fee_line(370, 320).as_str()));
    assert_eq!(st.character.chrome_at(Site::RightArm), None);
}

#[test]
fn over_budget_goes_magenta() {
    let mut st = AppState::new();
    loadout(&mut st);
    st.character.attributes.cool = 4;
    assert_eq!(st.character.load_max(), 22);
    give(&mut st, "wrecker_arm");
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    let load = cells(&buf, P0, 5, P1 - P0);
    assert!(load.trim_end().ends_with("23/22"), "{load}");
    let total = col_of(&buf, 5, P0, "23/22").unwrap();
    assert_eq!(buf[(total, 5)].fg, MAGENTA);
    assert!((P0..P1).any(|x| buf[(x, 5)].symbol() == "▰" && buf[(x, 5)].fg == MAGENTA));
    assert!((P0..P1).all(|x| buf[(x, 5)].fg != AMBER), "{load}");
    // One that fits stays amber.
    let mut st = AppState::new();
    loadout(&mut st);
    st.character.attributes.cool = 4;
    give(&mut st, "grip_arm");
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    let total = col_of(&buf, 5, P0, "22/22").unwrap();
    assert_eq!(buf[(total, 5)].fg, AMBER);
    assert!((P0..P1).all(|x| buf[(x, 5)].fg != MAGENTA));
}

#[test]
fn x_asks_first() {
    let arm = Stop::Site(Site::LeftArm);
    let (mut st, mut d) = in_chair(1000, arm);
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert!(st.character.chrome_at(Site::LeftArm).is_some());
    assert_eq!(
        hint_line(&d, &mut st),
        "x again to pull Piston arm for ¤110 · it comes out with its Arc hand · esc keep it"
    );
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(st.character.chrome_at(Site::LeftArm), None);
    assert_eq!(st.character.credits, 890);
    // Esc keeps it, and the deck stays open.
    let (mut st, mut d) = in_chair(1000, arm);
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert!(matches!(
        d.update(&mut st, &[key(KeyCode::Esc)]),
        ScreenCmd::Stay
    ));
    assert_eq!(d.kit.pulling, None);
    assert!(st.character.chrome_at(Site::LeftArm).is_some() && st.chair.is_some());
    // Moving on lets it go: the next x asks about the new stop.
    press(
        &mut d,
        &mut st,
        &[KeyCode::Char('x'), KeyCode::Up, KeyCode::Char('x')],
    );
    assert!(st.character.chrome_at(Site::LeftArm).is_some());
    assert_ne!(d.kit.pulling, Some(arm));
    assert!(d.kit.pulling.is_some());
    // So does another tab.
    d.update(&mut st, &[key(KeyCode::Char('e'))]);
    assert_eq!(d.kit.pulling, None);
    assert_eq!(st.character.credits, 1000);
    // Short of the fee, the ask says so in magenta, and the second x is refused.
    let (mut st, mut d) = in_chair(50, arm);
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(
        hint_line(&d, &mut st),
        "x again to pull Piston arm for ¤110 · you have ¤50 · esc keep it"
    );
    let buf = screen(&d, &mut st, (100, 30));
    let x = col_of(&buf, 28, 2, "¤110").unwrap();
    assert_eq!(buf[(x, 28)].fg, MAGENTA);
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert!(st.character.chrome_at(Site::LeftArm).is_some());
    assert_eq!(last(&st), Some(crate::actions::fee_line(110, 50).as_str()));
}

#[test]
fn free_x_pulls_at_once() {
    let mut st = AppState::new();
    loadout(&mut st);
    let mut d = on_chrome(&mut st, Stop::Site(Site::Eyes));
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(st.character.chrome_at(Site::Eyes), None);
}

#[test]
fn the_picker_moves_without_retyping_the_card() {
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "grip_arm");
    give(&mut st, "surgeon_arm");
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    // The place's card types out once.
    st.tick = d.kit.changed + REVEAL_TICKS + 1;
    let typed = d.kit.changed;
    let now = |d: &Deck, st: &AppState| {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        card(t.backend().buffer())
    };
    let line = |st: &AppState, item: &str| {
        let desc = &st.content.item(&id(item)).unwrap().desc;
        desc.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    };
    let (grip, surgeon) = (line(&st, "grip_arm"), line(&st, "surgeon_arm"));
    // Opening the picker, moving the pick and closing it keep the card as typed:
    // the pick's lines change in place, whole, at once.
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    assert!(now(&d, &st).contains(&grip), "opened:\n{}", now(&d, &st));
    press(&mut d, &mut st, &[KeyCode::Down]);
    assert_eq!(d.kit.state.picker, Some(1));
    assert!(now(&d, &st).contains(&surgeon), "moved:\n{}", now(&d, &st));
    press(&mut d, &mut st, &[KeyCode::Up, KeyCode::Esc]);
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(d.kit.changed, typed, "the card never started over");
}

#[test]
fn every_site_and_socket_has_a_tile_and_load_its_numbers() {
    let mut st = AppState::new();
    loadout(&mut st);
    let s = Stop::Site;
    assert_eq!(
        stops(&st),
        [
            s(Site::Neural),
            s(Site::Eyes),
            s(Site::Ears),
            s(Site::Jaw),
            s(Site::Spine),
            s(Site::Back),
            Stop::Socket(Site::Back, 0),
            s(Site::Organs),
            s(Site::Skin),
            s(Site::LeftArm),
            Stop::Socket(Site::LeftArm, 0),
            Stop::Socket(Site::LeftArm, 1),
            s(Site::RightArm),
            s(Site::Legs),
        ]
    );
    let d = on_chrome(&mut st, s(Site::LeftArm));
    let buf = screen(&d, &mut st, (100, 30));
    // Each tile wears its label in its top border: the site's, or its socket's kind.
    for (stop, label) in [
        (s(Site::Neural), "NEURAL"),
        (s(Site::Ears), "EARS"),
        (s(Site::LeftArm), "L ARM"),
        (Stop::Socket(Site::LeftArm, 0), "HAND"),
        (Stop::Socket(Site::LeftArm, 1), "FOREARM"),
        (Stop::Socket(Site::Back, 0), "BAY"),
        (s(Site::Legs), "LEGS"),
    ] {
        let r = tile(&st, stop, (100, 30));
        let top = cells(&buf, r.x, r.y, r.width);
        assert!(top.contains(label), "{stop:?}: {top}");
    }
    // The cursor's tile is lit; the others dark.
    let arm = tile(&st, s(Site::LeftArm), (100, 30));
    assert_eq!(buf[(arm.x, arm.y)].fg, CYAN);
    assert_eq!(buf[(arm.x + 1, arm.y + 1)].bg, BAR_BG);
    let ears = tile(&st, s(Site::Ears), (100, 30));
    assert_eq!(buf[(ears.x, ears.y)].fg, RULE);
    // LOAD, the numbers from content, the gauge's share rounded up.
    let (load, max) = (
        st.character.chrome_load(&st.content),
        st.character.load_max(),
    );
    assert_eq!(cells(&buf, P0, 5, 4), "LOAD");
    let row = cells(&buf, P0, 5, P1 - P0);
    assert!(row.ends_with(&format!("{load}/{max}")), "{row}");
    let segments = row.matches(['▰', '▱']).count() as u32;
    let filled = (load * segments).div_ceil(max) as usize;
    assert_eq!(row.matches('▰').count(), filled, "{row}");
}

#[test]
fn every_stop_draws_at_100x30_and_nothing_spills() {
    let mut st = AppState::new();
    for (site, item) in [
        (Site::Neural, "neural_port"),
        (Site::Eyes, "target_lens"),
        (Site::Ears, "damper_cochlea"),
        (Site::Jaw, "bulldog_jaw"),
        (Site::Spine, "redline_spine"),
        (Site::Organs, "adrenal_pump"),
        (Site::Skin, "subdermal_mesh"),
        (Site::Legs, "dog_legs"),
    ] {
        install(&mut st, site, item, &[]);
    }
    install(&mut st, Site::Back, "drone_cradle", &[(0, "scout_drone")]);
    install(
        &mut st,
        Site::LeftArm,
        "piston_arm",
        &[(0, "arc_hand"), (1, "monowire_spool")],
    );
    install(
        &mut st,
        Site::RightArm,
        "grip_arm",
        &[(0, "toolfingers"), (1, "launcher_sleeve")],
    );
    let all = stops(&st);
    assert_eq!(
        all.len(),
        11 + 5,
        "eleven sites, four arm sockets and the bay"
    );
    for &stop in &all {
        let d = on_chrome(&mut st, stop);
        let buf = screen(&d, &mut st, (100, 30));
        let r = tile(&st, stop, (100, 30));
        assert_eq!(buf[(r.x, r.y)].fg, CYAN, "{stop:?} lit");
        let name = match stop {
            Stop::Site(s) => st.character.chrome_at(s).cloned(),
            Stop::Socket(s, n) => st
                .character
                .chrome_copy(s)
                .and_then(|k| k.fitted[n].as_ref().map(|f| f.item.clone())),
        };
        let name = st.content.item(&name.unwrap()).unwrap().name.clone();
        let lead: String = name.chars().take(4).collect();
        let shown = inside(&buf, r);
        assert!(shown.contains(&lead), "{stop:?}: {shown}");
        // The frame's edge right of the body is the deck's.
        for y in BODY.y..BODY.bottom() {
            assert_eq!(buf[(BODY.right(), y)].symbol(), "│", "{stop:?} y {y}");
        }
    }
    // Nothing of the tab outside its body, whichever stop.
    for &stop in &all {
        let mut k = Kit::new(0);
        k.state.view = View::Chrome;
        k.state.stop = stop;
        let mut st2 = AppState::new();
        st2.character = st.character.clone();
        st2.tick = REVEAL_TICKS + 1;
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        for c in &mut buf.content {
            c.set_char('X');
        }
        k.draw(&st2, BODY, &mut buf);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if !BODY.contains((x, y).into()) {
                    assert_eq!(buf[(x, y)].symbol(), "X", "{stop:?} ({x},{y})");
                }
            }
        }
    }
}

#[test]
fn card_on_a_limb() {
    let mut st = AppState::new();
    loadout(&mut st);
    let d = on_chrome(&mut st, Stop::Site(Site::LeftArm));
    let buf = screen(&d, &mut st, (100, 30));
    let text = card(&buf);
    for want in [
        "PISTON ARM",
        "L ARM · load 3 · EMP −2",
        "BODY +1",
        "Shove",
        "■ hand",
        "Arc hand",
        "□ forearm",
    ] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    // The drawing at the top, a blank row, then the name.
    assert_eq!(cells(&buf, P0, CARD.0 + 7, 10), "PISTON ARM");
    assert_eq!(buf[(P0, CARD.0 + 7)].fg, CYAN);
    let elbow = Color::Rgb(0, 229, 255);
    let lit = (CARD.0..CARD.0 + 6)
        .any(|y| (P0..P1).any(|x| buf[(x, y)].fg == elbow || buf[(x, y)].bg == elbow));
    assert!(lit, "the elbow's light in the art's rows");
    let (x, y) = find(&buf, P0, CARD, "EMP −2").unwrap();
    assert_eq!(buf[(x, y)].fg, MAGENTA);
    let (x, y) = find(&buf, P0, CARD, "BODY +1").unwrap();
    assert_eq!(buf[(x, y)].fg, GREEN);
    let (x, y) = find(&buf, P0, CARD, "Shove").unwrap();
    assert_eq!(buf[(x, y)].fg, CYAN);
    // Under the empty forearm, in FAINT, the pieces in content that fit it.
    let forearm: Vec<String> = st
        .content
        .items
        .iter()
        .filter(|i| {
            i.implant
                .is_some_and(|m| m.fits == game::Fits::Socket(SocketKind::Forearm))
        })
        .map(|i| i.name.clone())
        .collect();
    assert!(!forearm.is_empty());
    let (x, y) = forearm
        .iter()
        .find_map(|n| find(&buf, P0, CARD, n))
        .unwrap_or_else(|| panic!("a forearm piece named:\n{text}"));
    assert_eq!(buf[(x, y)].fg, FAINTER);
}

#[test]
fn picker_installs() {
    let mut st = AppState::new();
    loadout(&mut st);
    let grip = give(&mut st, "grip_arm");
    let (load, max) = (
        st.character.chrome_load(&st.content),
        st.character.load_max(),
    );
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    let buf = screen(&d, &mut st, (100, 30));
    let text = card(&buf);
    for want in ["RIGHT ARM", "FITS", "from your bag", "Grip arm", "L3"] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    // The pick on the bar; the cursor's tile dim.
    let (x, y) = find(&buf, P0, CARD, "Grip arm").unwrap();
    assert_eq!(buf[(x, y)].bg, BAR_BG);
    let arm = tile(&st, Stop::Site(Site::RightArm), (100, 30));
    assert_eq!(buf[(arm.x, arm.y)].fg, DIM);
    assert_ne!(buf[(arm.x + 1, arm.y + 1)].bg, BAR_BG);
    // LOAD shows what it takes, in amber.
    let after = format!(" {}/{max}", load + 3);
    let x =
        col_of(&buf, 5, P0, &after).unwrap_or_else(|| panic!("{}", cells(&buf, P0, 5, P1 - P0)));
    assert_eq!(buf[(x + 1, 5)].fg, AMBER);
    assert!((P0..P1).any(|x| buf[(x, 5)].symbol() == "▰" && buf[(x, 5)].fg == AMBER));
    // Space installs it, and the picker closes on the arm.
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(
        st.character.chrome_at(Site::RightArm),
        Some(&id("grip_arm"))
    );
    assert_eq!(st.character.chrome_copy(Site::RightArm).map(|k| k.id), grip);
    assert_eq!(last(&st), Some("installed Grip arm · R ARM"));
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::RightArm));

    // Esc closes the picker and changes nothing; esc again closes the deck.
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "grip_arm");
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    let before = st.character.clone();
    assert!(matches!(
        d.update(&mut st, &[key(KeyCode::Esc)]),
        ScreenCmd::Stay
    ));
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(st.character, before);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::RightArm));
    assert!(matches!(
        d.update(&mut st, &[key(KeyCode::Esc)]),
        ScreenCmd::Pop { .. }
    ));
}

#[test]
fn picker_over_budget() {
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "grip_arm");
    st.character.attributes.cool = 3;
    let (load, max) = (
        st.character.chrome_load(&st.content),
        st.character.load_max(),
    );
    assert!(load <= max && load + 3 > max, "{load} of {max}");
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    let buf = screen(&d, &mut st, (100, 30));
    let (_, y) = find(&buf, P0, CARD, "Grip arm").unwrap();
    let x = col_of(&buf, y, P0, "L3").unwrap();
    assert_eq!(buf[(x, y)].fg, MAGENTA);
    let before = st.character.clone();
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(st.character, before);
    let why = format!("too much chrome: load {} of {max}", load + 3);
    assert_eq!(last(&st), Some(why.as_str()));
    assert_eq!(st.log.latest().map(|e| e.kind), Some(Kind::Warn));
    assert_eq!(d.kit.state.picker, Some(0), "still choosing");
}

#[test]
fn picker_closes_when_nothing_fits() {
    let mut st = AppState::new();
    loadout(&mut st);
    st.character.chrome.remove(&Site::LeftArm);
    st.character.chrome_copies.remove(&Site::LeftArm);
    let grip = give(&mut st, "grip_arm").unwrap();
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    // The arm goes in at L ARM by another way: nothing is left to fit R ARM.
    perform(
        &mut st,
        Action::InstallCopy {
            copy: grip,
            site: Site::LeftArm,
        },
    )
    .unwrap();
    d.update(&mut st, &[]);
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::RightArm));
    let buf = screen(&d, &mut st, (100, 30));
    let arm = tile(&st, Stop::Site(Site::RightArm), (100, 30));
    assert_eq!(buf[(arm.x, arm.y)].fg, CYAN, "the cursor stays on R ARM");
    let dot = (arm.x + arm.width / 2, arm.y + arm.height / 2);
    assert_eq!(buf[dot].symbol(), "·", "empty");
    assert!(hints(&d, &st).contains("space what fits"));
}

#[test]
fn nothing_fits_says_so() {
    let mut st = AppState::new();
    loadout(&mut st);
    let mut d = on_chrome(&mut st, Stop::Site(Site::Ears));
    let before = st.character.clone();
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(st.character, before);
    assert_eq!(last(&st), Some("nothing in your bag fits"));
    let buf = screen(&d, &mut st, (100, 30));
    assert!(cells(&buf, 2, 27, 60).contains("nothing in your bag fits"));
    // The card says so too, faint, under its FITS.
    let (x, y) = find(&buf, P0, CARD, "nothing in your bag fits").unwrap();
    assert_eq!(buf[(x, y)].fg, FAINTER);
    assert!(find(&buf, P0, CARD, "FITS").is_some());
}

#[test]
fn pull_and_reinstall_keeps_pieces() {
    let mut st = AppState::new();
    loadout(&mut st);
    let c = st.content.clone();
    let (load, emp) = (
        st.character.chrome_load(&c),
        st.character.resist(&c, Harm::Emp),
    );
    let mut d = on_chrome(&mut st, Stop::Site(Site::LeftArm));
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(st.character.chrome_at(Site::LeftArm), None);
    assert_eq!(last(&st), Some("pulled Piston arm · to the pack"));
    let arm = st
        .character
        .inventory
        .iter()
        .find(|s| s.item == id("piston_arm"))
        .unwrap();
    let hand = arm.copy.as_ref().unwrap().fitted[0].as_ref().unwrap();
    assert_eq!(hand.item, id("arc_hand"), "the hand comes out with the arm");
    // Across the ORGANS to R ARM: what fits is the arm, with its hand's load.
    press(&mut d, &mut st, &[KeyCode::Right, KeyCode::Right]);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::RightArm));
    let picks = fitting(&st, (Site::RightArm, None));
    assert_eq!(picks.len(), 1);
    assert_eq!(
        (picks[0].item.clone(), picks[0].load),
        (id("piston_arm"), 4)
    );
    press(&mut d, &mut st, &[KeyCode::Char(' '), KeyCode::Char(' ')]);
    assert_eq!(
        st.character.chrome_at(Site::RightArm),
        Some(&id("piston_arm"))
    );
    assert!(
        st.character
            .installed()
            .contains(&(Site::RightArm, Some(0), id("arc_hand")))
    );
    assert_eq!(
        (
            st.character.chrome_load(&c),
            st.character.resist(&c, Harm::Emp)
        ),
        (load, emp)
    );
    // The tiles follow: the arm and its hand at R ARM, L ARM empty and alone.
    let buf = screen(&d, &mut st, (120, 40));
    let r_arm = inside(&buf, tile(&st, Stop::Site(Site::RightArm), (120, 40)));
    assert!(r_arm.starts_with("Piston\narm\n"), "{r_arm}");
    let r_hand = inside(&buf, tile(&st, Stop::Socket(Site::RightArm, 0), (120, 40)));
    assert!(r_hand.starts_with("Arc hand\n"), "{r_hand}");
    let l_arm = tile(&st, Stop::Site(Site::LeftArm), (120, 40));
    assert_eq!(buf[(l_arm.x + 5, l_arm.y + 2)].symbol(), "·");
    assert!(
        chrome_layout(Rect::new(1, 10, 118, 27), &st)
            .tile(Stop::Socket(Site::LeftArm, 0))
            .is_none()
    );
}

#[test]
fn socket_picker() {
    let mut st = AppState::new();
    loadout(&mut st);
    let spool = give(&mut st, "monowire_spool").unwrap();
    let mut d = on_chrome(&mut st, Stop::Socket(Site::LeftArm, 1));
    assert_eq!(
        fitting(&st, (Site::LeftArm, Some(1))),
        [Pick {
            item: id("monowire_spool"),
            copy: Some(spool),
            load: 2,
        }]
    );
    assert!(fitting(&st, (Site::LeftArm, Some(0))).is_empty());
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    let buf = screen(&d, &mut st, (100, 30));
    let text = card(&buf);
    for want in [
        "FOREARM",
        "a socket on the Piston arm",
        "Monowire spool",
        "L2",
    ] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let fitted = st
        .character
        .chrome_copy(Site::LeftArm)
        .and_then(|k| k.fitted[1].clone())
        .unwrap();
    assert_eq!(
        (fitted.item.clone(), fitted.copy_id()),
        (id("monowire_spool"), Some(spool))
    );
    assert!(
        last(&st).is_some_and(|m| m.contains("fitted")),
        "{:?}",
        last(&st)
    );
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(d.kit.state.stop, Stop::Socket(Site::LeftArm, 1));
    // `x` on a socket's piece takes it out to the pack.
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert!(
        st.character
            .chrome_copy(Site::LeftArm)
            .is_some_and(|k| k.fitted[1].is_none())
    );
    assert!(st.character.copy_in_pack(spool).is_some());
}

#[test]
fn locked_jacked_in() {
    let mut st = jacked();
    loadout(&mut st);
    give(&mut st, "grip_arm");
    let before = st.character.clone();
    let mut d = on_chrome(&mut st, Stop::Site(Site::LeftArm));
    // Space on chrome: no picker, the lock's words; `x` the same.
    for code in [KeyCode::Char(' '), KeyCode::Char('x')] {
        st.say("before");
        press(&mut d, &mut st, &[code]);
        assert_eq!(d.kit.state.picker, None, "{code:?}");
        assert_eq!(st.character, before, "{code:?}");
        assert_eq!(last(&st), Some(JACKED_IN), "{code:?}");
    }
    let buf = screen(&d, &mut st, (100, 30));
    assert!(cells(&buf, 2, 27, 90).contains(JACKED_IN), "the strip");
    let hint = cells(&buf, 2, 28, 70);
    assert!(
        hint.starts_with("←↑↓→ select · space what fits · x pull it out · c meat/chrome"),
        "{hint}"
    );
    assert_eq!(buf[(2, 28)].fg, DIM);
    for keys in ["space what fits", "x pull it out"] {
        let x = col_of(&buf, 28, 2, keys).unwrap();
        let n = keys.chars().count() as u16;
        for k in 0..n {
            assert_eq!(buf[(x + k, 28)].fg, FAINTER, "{keys} at {k}");
        }
        assert_eq!(buf[(x + n + 1, 28)].fg, DIM, "after {keys}");
    }
    // An empty site: no picker, the lock's words.
    press(&mut d, &mut st, &[KeyCode::Right, KeyCode::Right]);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::RightArm));
    st.say("before");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(st.character, before);
    assert_eq!(last(&st), Some(JACKED_IN));
    let buf = screen(&d, &mut st, (100, 30));
    let x = col_of(&buf, 28, 2, "space what fits").unwrap();
    assert_eq!(buf[(x, 28)].fg, FAINTER);
}

#[test]
fn hints_follow_the_cursor() {
    let mut st = AppState::new();
    // Free chrome: the hints out of the chair, the chair's have their own test.
    st.free_chrome = true;
    loadout(&mut st);
    give(&mut st, "grip_arm");
    let hint_row = |d: &Deck, st: &mut AppState| cells(&screen(d, st, (100, 30)), 2, 28, 90);
    let mut d = Deck::new(0);
    d.update(&mut st, &[key(KeyCode::Char('3'))]);
    let meat = d.kit.hints();
    assert_eq!(hints(&d, &st), meat, "MEAT's own");
    assert!(hint_row(&d, &mut st).starts_with(meat));
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    d.kit.state.stop = Stop::Site(Site::LeftArm);
    let chrome =
        "←↑↓→ select · space what fits · x pull it out · c meat/chrome · Q/E tabs · tab close";
    assert_eq!(hints(&d, &st), chrome);
    assert!(hint_row(&d, &mut st).starts_with(chrome));
    d.kit.state.stop = Stop::Socket(Site::LeftArm, 0);
    assert_eq!(hints(&d, &st), chrome, "a socket's piece is chrome too");
    d.kit.state.stop = Stop::Site(Site::RightArm);
    let empty = "←↑↓→ select · space what fits · c meat/chrome · Q/E tabs · tab close";
    assert_eq!(hints(&d, &st), empty);
    assert!(hint_row(&d, &mut st).starts_with(empty));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let picker = "↑↓ choose · space install · esc back · Q/E tabs · tab close";
    assert_eq!(hints(&d, &st), picker);
    let row = hint_row(&d, &mut st);
    assert!(row.starts_with(picker), "{row}");
    let x = col_of(&screen(&d, &mut st, (100, 30)), 28, 2, "space install").unwrap();
    assert_eq!(screen(&d, &mut st, (100, 30))[(x, 28)].fg, DIM);
    // On a place that holds a piece, space swaps, and the hint says so.
    press(&mut d, &mut st, &[KeyCode::Esc]);
    d.kit.state.stop = Stop::Site(Site::LeftArm);
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert!(d.kit.state.picker.is_some());
    let swap = "↑↓ choose · space swap · esc back · Q/E tabs · tab close";
    assert_eq!(hints(&d, &st), swap);
    let row = hint_row(&d, &mut st);
    assert!(row.starts_with(swap), "{row}");
}

#[test]
fn space_on_chrome_opens_what_fits_and_swaps() {
    let mut st = AppState::new();
    loadout(&mut st);
    let grip = give(&mut st, "grip_arm");
    give(&mut st, "surgeon_arm");
    give(&mut st, "ceramic_plating");
    let (load, max) = (
        st.character.chrome_load(&st.content),
        st.character.load_max(),
    );
    assert_eq!((load, max), (19, 24));
    let mut d = on_chrome(&mut st, Stop::Site(Site::LeftArm));
    let picks: Vec<ItemId> = fitting(&st, (Site::LeftArm, None))
        .into_iter()
        .map(|p| p.item)
        .collect();
    assert_eq!(picks, [id("grip_arm"), id("surgeon_arm")], "the arms only");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    assert_eq!(
        st.character.chrome_at(Site::LeftArm),
        Some(&id("piston_arm"))
    );
    let buf = screen(&d, &mut st, (100, 30));
    let text = card(&buf);
    for want in [
        "LEFT ARM",
        "replaces Piston arm",
        "FITS",
        "Grip arm",
        "Surgeon arm",
        "L3",
        "load 19 → 18/24",
    ] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    assert!(
        !text.contains("PISTON ARM"),
        "the piece's card gave way:\n{text}"
    );
    assert!(!text.contains("nothing fitted"), "{text}");
    let (x, y) = find(&buf, P0, CARD, "replaces Piston arm").unwrap();
    assert_eq!(buf[(x, y)].fg, DIM);
    assert_eq!(buf[(x + 9, y)].fg, TEXT);
    // Two pieces out, one in: EMP comes back by two.
    let (x, y) = find(&buf, P0, CARD, "EMP +2").unwrap();
    assert_eq!(buf[(x, y)].fg, GREEN);
    // Lighter: LOAD shows no amber and the lower total.
    let row = cells(&buf, P0, 5, P1 - P0);
    assert!(row.trim_end().ends_with("18/24"), "{row}");
    assert!((P0..P1).all(|x| buf[(x, 5)].fg != AMBER), "{row}");
    let segments = row.matches(['▰', '▱']).count();
    assert_eq!(row.matches('▰').count(), (18 * segments).div_ceil(24));
    // Space swaps: the Grip arm in, the Piston arm to the pack with its hand.
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(st.character.chrome_at(Site::LeftArm), Some(&id("grip_arm")));
    assert_eq!(st.character.chrome_copy(Site::LeftArm).map(|k| k.id), grip);
    assert_eq!(
        last(&st),
        Some("installed Grip arm · L ARM · Piston arm to the pack")
    );
    let arm = st
        .character
        .inventory
        .iter()
        .find(|s| s.item == id("piston_arm"))
        .expect("the Piston arm in the pack");
    let hand = arm.copy.as_ref().unwrap().fitted[0].as_ref().unwrap();
    assert_eq!(hand.item, id("arc_hand"), "its hand still in it");
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::LeftArm));
    assert_eq!(st.character.chrome_load(&st.content), 18);
}

#[test]
fn the_same_piece_is_no_swap() {
    // The Target lens at EYES, held without a copy: another one is the same piece.
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "target_lens");
    let eyes = (Site::Eyes, None);
    assert!(fitting(&st, eyes).is_empty(), "{:?}", fitting(&st, eyes));
    let mut d = on_chrome(&mut st, Stop::Site(Site::Eyes));
    let before = st.character.clone();
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(st.character, before);
    assert_eq!(last(&st), Some("nothing in your bag fits"));
    // Other eyes still swap in.
    give(&mut st, "nightglass_optics");
    let picks: Vec<ItemId> = fitting(&st, eyes).into_iter().map(|p| p.item).collect();
    assert_eq!(picks, [id("nightglass_optics")]);
    // A limb is held per copy: another Piston arm is another arm, and swaps.
    let other = give(&mut st, "piston_arm");
    assert!(other.is_some());
    let arms: Vec<Option<CopyId>> = fitting(&st, (Site::LeftArm, None))
        .into_iter()
        .map(|p| p.copy)
        .collect();
    assert_eq!(arms, [other]);
}

#[test]
fn the_picker_counts_load_as_the_rule_does() {
    // A budget two short of the loadout's: some picks fit by what leaves, some not.
    let mut st = AppState::new();
    loadout(&mut st);
    st.character.attributes.cool = 3;
    for item in [
        "grip_arm",
        "wrecker_arm",
        "ceramic_plating",
        "grounding_weave",
        "second_heart",
        "filter_liver",
        "secondhand_eyes",
        "nightglass_optics",
        "neural_port",
        "limbic_damper",
        "damper_cochlea",
        "dog_legs",
    ] {
        give(&mut st, item);
    }
    let surgeon = give(&mut st, "surgeon_arm").unwrap();
    let tools = st.character.new_copy(&st.content, &id("toolfingers"));
    let k = st
        .character
        .inventory
        .iter_mut()
        .find(|s| s.copy_id() == Some(surgeon))
        .and_then(|s| s.copy.as_mut())
        .unwrap();
    k.fitted[0] = Some(Stack {
        item: id("toolfingers"),
        qty: 1,
        copy: Some(tools),
    });
    let max = st.character.load_max();
    let mut seen = (0, 0);
    for site in Site::ALL {
        for pick in fitting(&st, (site, None)) {
            let stack = st
                .character
                .inventory
                .iter()
                .find(|s| match pick.copy {
                    Some(k) => s.copy_id() == Some(k),
                    None => s.copy.is_none() && s.item == pick.item,
                })
                .unwrap();
            let after = load_after(&st, (site, None), &pick);
            let rule = st.character.can_install_stack(&st.content, stack, site);
            let over = format!("too much chrome: load {after} of {max}");
            match rule {
                Ok(()) => assert!(after <= max, "{site:?} {:?}", pick.item),
                Err(why) => assert_eq!(why, over, "{site:?} {:?}", pick.item),
            }
            if after > max {
                seen.1 += 1;
            } else {
                seen.0 += 1;
            }
        }
    }
    assert!(seen.0 > 0 && seen.1 > 0, "fits and refusals both: {seen:?}");
}

#[test]
fn a_heavier_swap_counts_what_leaves() {
    // SKIN: Ceramic plating (4) for the Subdermal mesh (2).
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "ceramic_plating");
    let mut d = on_chrome(&mut st, Stop::Site(Site::Skin));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    let buf = screen(&d, &mut st, (100, 30));
    let text = card(&buf);
    for want in ["SKIN", "replaces Subdermal mesh", "Ceramic plating", "L4"] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    let (x, y) = find(&buf, P0, CARD, "load 19 → 21/24").unwrap();
    assert_eq!(buf[(x + 10, y)].fg, AMBER);
    assert!(
        !text.contains("EMP"),
        "one piece for one: EMP moves not:\n{text}"
    );
    // The gauge: the two more in amber, the total with them.
    let row = cells(&buf, P0, 5, P1 - P0);
    assert!(row.trim_end().ends_with("21/24"), "{row}");
    let x = col_of(&buf, 5, P0, " 21/24").unwrap();
    assert_eq!(buf[(x + 1, 5)].fg, AMBER);
    assert!((P0..P1).any(|x| buf[(x, 5)].symbol() == "▰" && buf[(x, 5)].fg == AMBER));

    // Over the budget even once the mesh's two leave: magenta, and it refuses.
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "ceramic_plating");
    st.character.attributes.cool = 3;
    assert_eq!(st.character.load_max(), 20);
    let mut d = on_chrome(&mut st, Stop::Site(Site::Skin));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (100, 30));
    let (_, y) = find(&buf, P0, CARD, "Ceramic plating").unwrap();
    let x = col_of(&buf, y, P0, "L4").unwrap();
    assert_eq!(buf[(x, y)].fg, MAGENTA);
    let (x, y) = find(&buf, P0, CARD, "load 19 → 21/20").unwrap();
    assert_eq!(buf[(x + 10, y)].fg, MAGENTA);
    let before = st.character.clone();
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(st.character, before);
    assert_eq!(last(&st), Some("too much chrome: load 21 of 20"));
    assert_eq!(d.kit.state.picker, Some(0), "still choosing");
}

#[test]
fn space_on_a_socket_piece_swaps_it() {
    let mut st = AppState::new();
    loadout(&mut st);
    let fingers = give(&mut st, "toolfingers");
    let hand = st
        .character
        .chrome_copy(Site::LeftArm)
        .and_then(|k| k.fitted[0].as_ref()?.copy_id());
    let mut d = on_chrome(&mut st, Stop::Socket(Site::LeftArm, 0));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    let buf = screen(&d, &mut st, (100, 30));
    let text = card(&buf);
    for want in [
        "HAND",
        "replaces Arc hand",
        "Toolfingers",
        "L1",
        "load 19 → 19/24",
    ] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    let (x, y) = find(&buf, P0, CARD, "load 19 → 19/24").unwrap();
    assert_eq!(buf[(x + 10, y)].fg, TEXT, "nothing more: plain");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let fitted = st
        .character
        .chrome_copy(Site::LeftArm)
        .and_then(|k| k.fitted[0].clone())
        .unwrap();
    assert_eq!(fitted.copy_id(), fingers);
    assert_eq!(
        last(&st),
        Some("Toolfingers fitted · socket hand · Arc hand to the pack")
    );
    assert!(hand.is_some_and(|k| st.character.copy_in_pack(k).is_some()));
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(d.kit.state.stop, Stop::Socket(Site::LeftArm, 0));
}

#[test]
fn the_swap_picker_types_its_card_once() {
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "grip_arm");
    give(&mut st, "surgeon_arm");
    let mut d = on_chrome(&mut st, Stop::Site(Site::LeftArm));
    st.tick = d.kit.changed + REVEAL_TICKS + 1;
    // Opening on a piece: its card gives way to the FITS card, typed out once.
    st.tick += 1;
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let opened = st.tick;
    assert_eq!(d.kit.changed, opened);
    // Moving the pick never types it again.
    st.tick += 5;
    press(&mut d, &mut st, &[KeyCode::Down]);
    assert_eq!(d.kit.state.picker, Some(1));
    st.tick += 5;
    press(&mut d, &mut st, &[KeyCode::Up]);
    assert_eq!(d.kit.changed, opened);
    // Esc: the piece's card again, typed out.
    st.tick += 5;
    press(&mut d, &mut st, &[KeyCode::Esc]);
    assert_eq!(d.kit.state.picker, None);
    assert_eq!(d.kit.changed, st.tick);
    let text = card(&screen(&d, &mut st, (100, 30)));
    assert!(text.contains("PISTON ARM"), "{text}");
}

#[test]
fn x_pulls_chrome_and_nothing_else() {
    let mut st = AppState::new();
    loadout(&mut st);
    // On an empty place `x` does nothing.
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    st.say("before");
    let before = st.character.clone();
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!((&st.character, last(&st)), (&before, Some("before")));
    let mut d = on_chrome(&mut st, Stop::Socket(Site::LeftArm, 1));
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!((&st.character, last(&st)), (&before, Some("before")));
    // On a socket's piece: out to the pack.
    let mut d = on_chrome(&mut st, Stop::Socket(Site::LeftArm, 0));
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(last(&st), Some("Arc hand out · to the pack"));
    assert_eq!(st.character.count(&id("arc_hand")), 1);
    // On a site's piece: out to the pack; the card types afresh.
    let mut d = on_chrome(&mut st, Stop::Site(Site::LeftArm));
    st.tick += 7;
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(st.character.chrome_at(Site::LeftArm), None);
    assert_eq!(last(&st), Some("pulled Piston arm · to the pack"));
    assert_eq!(d.kit.changed, st.tick);
    assert_eq!(d.kit.state.picker, None);
}

#[test]
fn the_arrows_walk_the_chrome() {
    let mut st = AppState::new();
    loadout(&mut st);
    let mut d = on_chrome(&mut st, Stop::Site(Site::Neural));
    let walk = |d: &mut Deck, st: &mut AppState, code: KeyCode| {
        st.tick += 5;
        press(d, st, &[code]);
        d.kit.state.stop
    };
    let s = Stop::Site;
    assert_eq!(
        walk(&mut d, &mut st, KeyCode::Up),
        s(Site::Neural),
        "the top"
    );
    assert_eq!(walk(&mut d, &mut st, KeyCode::Left), s(Site::Eyes));
    assert_eq!(walk(&mut d, &mut st, KeyCode::Char('l')), s(Site::Neural));
    assert_eq!(walk(&mut d, &mut st, KeyCode::Right), s(Site::Ears));
    assert_eq!(walk(&mut d, &mut st, KeyCode::Char('l')), s(Site::Jaw));
    assert_eq!(d.kit.changed, st.tick, "the card types out afresh");
    d.kit.state.stop = s(Site::Neural);
    assert_eq!(walk(&mut d, &mut st, KeyCode::Down), s(Site::Organs));
    assert_eq!(walk(&mut d, &mut st, KeyCode::Char('j')), s(Site::Spine));
    assert_eq!(walk(&mut d, &mut st, KeyCode::Down), s(Site::Legs));
    // The arm's sockets hang off it: its hand outside, its forearm under it.
    d.kit.state.stop = s(Site::LeftArm);
    assert_eq!(
        walk(&mut d, &mut st, KeyCode::Char('h')),
        Stop::Socket(Site::LeftArm, 0)
    );
    d.kit.state.stop = s(Site::LeftArm);
    assert_eq!(
        walk(&mut d, &mut st, KeyCode::Down),
        Stop::Socket(Site::LeftArm, 1)
    );
    // The picker holds the cursor while it is open.
    give(&mut st, "monowire_spool");
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert!(d.kit.state.picker.is_some());
    for code in [KeyCode::Left, KeyCode::Right] {
        assert_eq!(walk(&mut d, &mut st, code), Stop::Socket(Site::LeftArm, 1));
    }
}

#[test]
fn c_switches_meat_and_chrome_from_anywhere_on_the_tab() {
    let mut st = AppState::new();
    loadout(&mut st);
    let mut d = Deck::new(0);
    d.update(&mut st, &[key(KeyCode::Char('3'))]);
    assert_eq!(d.kit.state.view, View::Meat, "MEAT first");
    // From a MEAT tile: CHROME on the stop it had.
    d.kit.state.part = BodyPart::Torso;
    d.kit.state.stop = Stop::Site(Site::Spine);
    st.tick = 40;
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    assert_eq!(d.kit.state.view, View::Chrome);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::Spine));
    assert_eq!(d.kit.changed, 40, "the card types out afresh");
    let buf = screen(&d, &mut st, (120, 40));
    assert_eq!(cells(&buf, 2, 10, 23), "MEAT ▌CHROME▐  c switch");
    // And back: MEAT, the body, the part it had.
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    assert_eq!(
        (d.kit.state.view, d.kit.state.pane, d.kit.state.part),
        (View::Meat, Pane::Body, BodyPart::Torso)
    );
    let buf = screen(&d, &mut st, (120, 40));
    assert_eq!(cells(&buf, 2, 10, 23), "▌MEAT▐ CHROME  c switch");
    assert_eq!(buf[(17, 10)].fg, CYAN, "the c");
    // From the pack: CHROME, then back on the body.
    d.kit.state.pane = Pane::Pack;
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    assert_eq!(
        (d.kit.state.view, d.kit.state.stop),
        (View::Chrome, Stop::Site(Site::Spine))
    );
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    assert_eq!(
        (d.kit.state.view, d.kit.state.pane),
        (View::Meat, Pane::Body)
    );
    // The view stays while the game runs: away to another tab and back, and the deck
    // closed and opened again.
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    press(&mut d, &mut st, &[KeyCode::Char('1'), KeyCode::Char('3')]);
    assert_eq!(d.kit.state.view, View::Chrome);
    let again = Deck::new(st.tick).restore(&st);
    assert_eq!(
        (again.tab, again.kit.state.view, again.kit.state.stop),
        (Tab::Kit, View::Chrome, Stop::Site(Site::Spine))
    );
}

#[test]
fn c_closes_fits_and_forgets_a_pending_pull() {
    // FITS open on R ARM: `c` shuts it and switches; back on CHROME it is still shut.
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "grip_arm");
    let mut d = on_chrome(&mut st, Stop::Site(Site::RightArm));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    let before = st.character.clone();
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    assert_eq!((d.kit.state.view, d.kit.state.picker), (View::Meat, None));
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    assert_eq!((d.kit.state.view, d.kit.state.picker), (View::Chrome, None));
    assert_eq!(st.character, before, "nothing went in");
    // A pull asked about in the chair: `c` forgets it, and the next `x` asks again.
    let arm = Stop::Site(Site::LeftArm);
    let (mut st, mut d) = in_chair(1000, arm);
    press(&mut d, &mut st, &[KeyCode::Char('x')]);
    assert_eq!(d.kit.pulling, Some(arm));
    press(&mut d, &mut st, &[KeyCode::Char('c')]);
    assert_eq!((d.kit.state.view, d.kit.pulling), (View::Meat, None));
    press(&mut d, &mut st, &[KeyCode::Char('c'), KeyCode::Char('x')]);
    assert_eq!(d.kit.pulling, Some(arm), "asked again");
    assert!(st.character.chrome_at(Site::LeftArm).is_some());
    assert_eq!(st.character.credits, 1000);
}

#[test]
fn a_socket_stop_whose_piece_goes_falls_back_to_its_site() {
    let mut st = AppState::new();
    loadout(&mut st);
    give(&mut st, "monowire_spool");
    let mut d = on_chrome(&mut st, Stop::Socket(Site::LeftArm, 1));
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    assert_eq!(d.kit.state.picker, Some(0));
    // The Piston arm comes out another way, its sockets with it.
    perform(&mut st, Action::Pull(Site::LeftArm)).unwrap();
    d.update(&mut st, &[]);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::LeftArm));
    assert_eq!(d.kit.state.picker, None);
    let buf = screen(&d, &mut st, (100, 30));
    let arm = tile(&st, Stop::Site(Site::LeftArm), (100, 30));
    assert_eq!(buf[(arm.x, arm.y)].fg, CYAN);
    // The keys go on from there.
    press(&mut d, &mut st, &[KeyCode::Right]);
    assert_eq!(d.kit.state.stop, Stop::Site(Site::Organs));
}

#[test]
fn the_chair_opens_gear_on_chrome() {
    // Left on MEAT's pack.
    let mut st = crate::clinic::testutil::seated();
    loadout(&mut st);
    st.deck_tab = Tab::Kit;
    st.deck_kit.pane = Pane::Pack;
    st.deck_kit.stop = Stop::Site(Site::Legs);
    let d = Deck::chair(&st);
    assert_eq!(
        (d.tab, d.kit.state.view, d.kit.state.stop),
        (Tab::Kit, View::Chrome, Stop::Site(Site::Neural))
    );
    assert_eq!(d.kit.state.picker, None);
    // Left on ATTRIBUTES, on its ledger.
    st.deck_tab = Tab::Attributes;
    st.deck_sheet.pane = crate::screens::sheet::SheetPane::Ledger;
    let mut d = Deck::chair(&st);
    assert_eq!(
        (d.tab, d.kit.state.view, d.kit.state.stop),
        (Tab::Kit, View::Chrome, Stop::Site(Site::Neural))
    );
    let buf = screen(&d, &mut st, (100, 30));
    assert_eq!(cells(&buf, 2, 5, 27), "MEAT ▌CHROME▐ · CHROMING UP");
    // Esc still gets you out of the chair.
    assert!(matches!(
        d.update(&mut st, &[key(KeyCode::Esc)]),
        ScreenCmd::Pop { .. }
    ));
    assert_eq!(st.chair, None);
}

#[test]
fn load_sits_over_the_divider() {
    let mut st = AppState::new();
    loadout(&mut st);
    let mut d = on_chrome(&mut st, Stop::Site(Site::LeftArm));
    let buf = screen(&d, &mut st, (120, 40));
    let (load, max) = (
        st.character.chrome_load(&st.content),
        st.character.load_max(),
    );
    let pieces = st.character.pieces(&st.content);
    let row = cells(&buf, P0_W, 10, P1_W - P0_W);
    assert!(row.starts_with("LOAD "), "{row}");
    assert!(row.ends_with(&format!("{load}/{max}")), "{row}");
    assert_eq!(buf[(P0_W, 10)].fg, DIM);
    let line = cells(&buf, P0_W, 11, P1_W - P0_W);
    assert_eq!(
        line.trim_end(),
        format!("{} left · {pieces} pieces in you", max - load)
    );
    assert_eq!(buf[(P0_W, 11)].fg, FAINTER);
    // The divider under them, joined to the one down the tab.
    assert_eq!(buf[(DIV_W, 12)].symbol(), "╠");
    assert!((DIV_W + 1..P1_W + 1).all(|x| buf[(x, 12)].symbol() == "═"));
    for y in 10..37 {
        if y != 12 {
            assert_eq!(buf[(DIV_W, y)].symbol(), "║", "y {y}");
        }
    }
    // Free chrome: the divider and its junction amber, as the ledger's rule was.
    assert_eq!(buf[(DIV_W, 20)].fg, AMBER);
    assert_eq!(buf[(DIV_W, 12)].fg, AMBER);
    assert_eq!(buf[(DIV_W + 1, 12)].fg, RULE);
    // The card under the divider.
    assert!(region(&buf, (P0_W, P1_W), CARD_W).contains("PISTON ARM"));
    // A pick: the segments it adds in amber.
    give(&mut st, "grip_arm");
    d.kit.state.stop = Stop::Site(Site::RightArm);
    press(&mut d, &mut st, &[KeyCode::Char(' ')]);
    let buf = screen(&d, &mut st, (120, 40));
    let amber = (P0_W..P1_W)
        .filter(|&x| buf[(x, 10)].symbol() == "▰" && buf[(x, 10)].fg == AMBER)
        .count();
    assert!(amber > 0, "{}", cells(&buf, P0_W, 10, P1_W - P0_W));
    // Over budget, the line says by how much.
    st.character.attributes.cool = 1;
    let over = st.character.chrome_load(&st.content) + 3 - st.character.load_max();
    let buf = screen(&d, &mut st, (120, 40));
    let line = cells(&buf, P0_W, 11, P1_W - P0_W);
    assert!(line.starts_with(&format!("over by {over} · ")), "{line}");
}

#[test]
fn the_tiles_say_what_each_piece_gives() {
    let mut st = AppState::new();
    loadout(&mut st);
    let d = on_chrome(&mut st, Stop::Site(Site::Neural));
    let buf = screen(&d, &mut st, (120, 40));
    let at = |stop: Stop| tile(&st, stop, (120, 40));
    // L ARM: the name on two lines, then its perk short and its sockets' pips (the
    // hand fitted, the forearm empty) in the pips' colour.
    let arm = at(Stop::Site(Site::LeftArm));
    assert_eq!(inside(&buf, arm), "Piston\narm\nBOD +1 ■□\n");
    assert_eq!(cells(&buf, arm.x, arm.y, 11), "┌ L ARM ──┐");
    let pips = col_of(&buf, arm.y + 3, arm.x, "■□").unwrap();
    assert_eq!(buf[(pips, arm.y + 3)].fg, Color::Rgb(0x3f, 0x7c, 0x87));
    assert_eq!(buf[(arm.x + 1, arm.y + 3)].fg, DIM);
    // The cradle: its bays and their pips; the drone in the bay, docked.
    let back = at(Stop::Site(Site::Back));
    assert_eq!(inside(&buf, back), "Drone\ncradle\n1 bay ■\n");
    let bay = at(Stop::Socket(Site::Back, 0));
    assert_eq!(cells(&buf, bay.x, bay.y, 11), "┌ BAY ────┐");
    assert_eq!(inside(&buf, bay), "Scout\ndrone\ndocked\n");
    // A piece with no perk: the first ability it grants.
    let jaw = at(Stop::Site(Site::Jaw));
    assert!(
        inside(&buf, jaw).ends_with("\nBite\n"),
        "{}",
        inside(&buf, jaw)
    );
    // An empty site: a dim dot.
    let ears = at(Stop::Site(Site::Ears));
    let dot = (ears.x + 5, ears.y + 2);
    assert_eq!((buf[dot].symbol(), buf[dot].fg), ("·", FAINTER));
    // Compact, a label too long for its border drops its spaces.
    let buf = screen(&d, &mut st, (100, 30));
    let fore = tile(&st, Stop::Socket(Site::LeftArm, 1), (100, 30));
    assert_eq!(fore.width, 9);
    assert_eq!(cells(&buf, fore.x, fore.y, 9), "┌FOREARM┐");
}
