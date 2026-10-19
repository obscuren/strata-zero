//! Screenshots for a look at the devices: ignored tests that draw a screen over the
//! washed map and write it as an SVG. Run with `SHOT_DIR=... cargo test -p strata-zero
//! shots:: -- --ignored`.

use crate::app::{AppState, Screen};
use crate::helix::shell::HostId;
use crate::screens::blueprint::Blueprint;
use crate::screens::deck::Deck;
use crate::screens::explore::MapScreen;
use crate::screens::keypad::Keypad;
use crate::screens::terminal::Terminal;
use crate::screens::testutil::key;
use citygen::{GenParams, Generator, Place, Pos, SizePreset, StratumKind};
use crossterm::event::KeyCode;
use game::ItemId;
use ratatui::Terminal as Term;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use std::fmt::Write as _;

const CELL_W: f32 = 9.6;
const CELL_H: f32 = 20.0;

fn hex(c: Color, fallback: &str) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => fallback.to_string(),
    }
}

/// The buffer as an SVG: a rect per cell background, a text run per cell.
fn svg(buf: &Buffer, bg: &str) -> String {
    let (w, h) = (
        buf.area.width as f32 * CELL_W,
        buf.area.height as f32 * CELL_H,
    );
    // A square canvas: the thumbnailer crops anything else.
    let side = w.max(h);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{side}\" height=\"{side}\" viewBox=\"0 0 {side} {side}\">"
    );
    let _ = writeln!(
        out,
        "<rect width=\"{side}\" height=\"{side}\" fill=\"{bg}\"/>"
    );
    let _ = writeln!(
        out,
        "<style>text{{font-family:'JetBrains Mono','Menlo','DejaVu Sans Mono',monospace;font-size:16px;dominant-baseline:central}}</style>"
    );
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            let px = x as f32 * CELL_W;
            let py = y as f32 * CELL_H;
            let fill = hex(cell.bg, bg);
            if fill != bg {
                let _ = writeln!(
                    out,
                    "<rect x=\"{px:.1}\" y=\"{py:.1}\" width=\"{CELL_W}\" height=\"{CELL_H}\" fill=\"{fill}\"/>"
                );
            }
            let sym = cell.symbol();
            if sym.trim().is_empty() {
                continue;
            }
            let fg = hex(cell.fg, "#cdd4de");
            let bold = cell.modifier.contains(Modifier::BOLD);
            let dim = cell.modifier.contains(Modifier::DIM);
            let text = sym
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let _ = writeln!(
                out,
                "<text text-anchor=\"middle\" x=\"{:.1}\" y=\"{:.1}\" fill=\"{fg}\"{}{}>{text}</text>",
                px + CELL_W / 2.0,
                py + CELL_H / 2.0,
                if bold { " font-weight=\"bold\"" } else { "" },
                if dim { " opacity=\"0.6\"" } else { "" },
            );
        }
    }
    out.push_str("</svg>\n");
    out
}

fn state_home() -> AppState {
    let mut st = AppState::new();
    st.enter_city(
        Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap(),
    );
    st
}

fn shot(name: &str, st: &AppState, map: &MapScreen, top: &dyn Screen) {
    shot_at(name, st, map, top, (120, 40));
}

/// `shot` on a terminal of `w` by `h` cells.
fn shot_at(name: &str, st: &AppState, map: &MapScreen, top: &dyn Screen, size: (u16, u16)) {
    shot_stack(name, st, &[map, top], size);
}

/// A stack of screens, each over the one below it washed, as the app draws it.
fn shot_stack(name: &str, st: &AppState, stack: &[&dyn Screen], (w, h): (u16, u16)) {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let mut t = Term::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| {
        for (i, screen) in stack.iter().enumerate() {
            if i > 0 {
                crate::ui::dim::wash(f.buffer_mut());
            }
            screen.draw(st, f);
        }
    })
    .unwrap();
    let bg = hex(st.style.background.to_color(), "#04060a");
    let out = svg(t.backend().buffer(), &bg);
    std::fs::write(format!("{dir}/{name}.svg"), out).unwrap();
}

#[test]
#[ignore]
fn keypad_over_the_home_floor() {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    st.tick += 200;
    let (b, level, number, _) = crate::jobs::first_fetch(&st).expect("the favour");
    let mut pad = Keypad::open(&st, b, level, number);
    pad.update(&mut st, &[key(KeyCode::Char('4')), key(KeyCode::Char('4'))]);
    shot("keypad", &st, &map, &pad);
}

/// Types each line into the terminal, letting the one before settle first.
fn type_lines(st: &mut AppState, term: &mut Terminal, lines: &[&str]) {
    for line in lines {
        st.tick += 10_000;
        term.update(st, &[]);
        let keys: Vec<_> = line
            .chars()
            .map(|c| key(KeyCode::Char(c)))
            .chain(std::iter::once(key(KeyCode::Enter)))
            .collect();
        term.update(st, &keys);
    }
}

/// A shell on a district terminal, after `ls /net`, `ssh 1` and a denied `door release`.
fn terminal_shot(name: &str, d: citygen::DistrictId) {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    let mut term = Terminal::open(
        &mut st,
        HostId::District(d),
        (Place::Stratum(StratumKind::Street), Pos::new(1, 1)),
    );
    type_lines(&mut st, &mut term, &["ls /net", "ssh 1", "door release"]);
    st.tick += 400;
    term.update(&mut st, &[]);
    shot(name, &st, &map, &term);
}

#[test]
#[ignore]
fn terminals_by_theme() {
    let st = state_home();
    let city = st.city.as_ref().unwrap();
    for (name, theme) in [
        ("terminal_megacorp", citygen::Theme::CorpCore),
        ("terminal_street", citygen::Theme::CombatZone),
        ("terminal_neon", citygen::Theme::NightMarket),
    ] {
        if let Some(d) = city.districts.iter().find(|d| d.theme == theme) {
            terminal_shot(name, d.id);
        }
    }
}

#[test]
#[ignore]
fn the_rig_at_home() {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    let home = st.home.unwrap().home;
    let mut term = Terminal::open(&mut st, HostId::Rig, (map.place, home.rig));
    type_lines(&mut st, &mut term, &["cat ~/mail/001-a-favour.txt"]);
    st.tick += 10_000;
    term.update(&mut st, &[]);
    shot("rig", &st, &map, &term);
}

#[test]
#[ignore]
fn terminal_on_the_street() {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    let d = {
        let city = st.city.as_ref().unwrap();
        city.building(st.home.unwrap().home.building).district
    };
    let mut term = Terminal::open(
        &mut st,
        HostId::District(d),
        (Place::Stratum(StratumKind::Street), Pos::new(1, 1)),
    );
    type_lines(
        &mut st,
        &mut term,
        &["ls /net", "ssh 1", "echo release > /dev/door"],
    );
    st.tick += 200;
    term.update(&mut st, &[]);
    shot("terminal", &st, &map, &term);
}

/// The blueprint window on `item`, opened from the deck screen's tab `tab` (GEAR, `3`,
/// opens what is worn), over the map, at each size.
fn blueprint_shots(st: &mut AppState, tab: char, item: &str, name: &str, sizes: &[(u16, u16)]) {
    let map = MapScreen::walking(st);
    let mut deck = Deck::new(st.tick);
    deck.update(st, &[key(KeyCode::Char(tab))]);
    // The copy the deck screen would pass: the one worn from GEAR, else the first carried.
    let id = ItemId::new(item);
    let copy = if tab == '3' {
        let slot = st.content.item(&id).and_then(|i| i.slot);
        slot.and_then(|s| st.character.worn_copy(s)).map(|k| k.id)
    } else {
        let carried = st.character.inventory.iter().find(|s| s.item == id);
        carried.and_then(|s| s.copy_id())
    };
    let window = Blueprint::open(st, id, tab == '3', copy);
    // The glass typed out, clear of the deck's signal jitter (three ticks in ninety).
    st.tick += 200;
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    for (w, h) in sizes {
        let name = format!("{name}_{w}x{h}");
        shot_stack(&name, st, &[&map, &deck, &window], (*w, *h));
    }
}

#[test]
#[ignore]
fn the_deck_on_its_blueprint() {
    let mut st = state_home();
    let sizes = [(100, 30), (120, 40)];
    blueprint_shots(&mut st, '3', "pawnshop_deck", "blueprint_deck", &sizes);
}

#[test]
#[ignore]
fn dshawns_decks_on_their_blueprints() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    let at = [(120, 40)];
    blueprint_shots(&mut st, '3', "rebuilt_deck", "blueprint_dshawn", &at);
    // The pawnshop deck he carries: its own blueprint, read only.
    blueprint_shots(
        &mut st,
        '4',
        "pawnshop_deck",
        "blueprint_dshawn_carried",
        &at,
    );
}

/// D'SHAWN's rebuilt deck in the blueprint window's modding mode.
#[test]
#[ignore]
fn modding_the_rebuilt_deck() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    let map = MapScreen::walking(&mut st);
    let mut deck = Deck::new(st.tick);
    deck.update(&mut st, &[key(KeyCode::Char('3'))]);
    let copy = st.character.worn_copy(game::Slot::Deck).map(|k| k.id);
    let mut window = Blueprint::open(&st, ItemId::new("rebuilt_deck"), true, copy);
    st.tick += 200;
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    window.update(&mut st, &[key(KeyCode::Char('m'))]);
    for (w, h) in [(100, 30), (120, 40)] {
        let name = format!("modding_the_rebuilt_deck_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck, &window], (w, h));
    }
    // The HARDWARE tray open on the left socket.
    window.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char(' '))]);
    for (w, h) in [(100, 30), (120, 40)] {
        let name = format!("modding_the_rebuilt_deck_tray_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck, &window], (w, h));
    }
    // A Coldfin vII fitted on the left, a Sidecar on the right.
    for (socket, item) in [(0, "coldfin_2"), (1, "sidecar")] {
        let piece = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == ItemId::new(item))
            .and_then(|s| s.copy_id())
            .unwrap();
        let fit = game::Action::Fit {
            host: game::Host::Worn(game::Slot::Deck),
            socket,
            piece,
        };
        crate::actions::perform(&mut st, fit).unwrap();
    }
    window.update(&mut st, &[key(KeyCode::Esc)]);
    for (w, h) in [(100, 30), (120, 40)] {
        let name = format!("modding_the_rebuilt_deck_fitted_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck, &window], (w, h));
    }
}

/// Each piece of deck hardware on its blueprint.
#[test]
#[ignore]
fn hardware_blueprints() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    for item in ["coldfin_1", "coldfin_2", "sidecar"] {
        let name = format!("hardware_{item}");
        blueprint_shots(&mut st, '4', item, &name, &[(100, 30)]);
    }
}

/// GEAR and INVENTORY with a Coldfin vII fitted: the marks after the names.
#[test]
#[ignore]
fn gear_and_inventory_marks() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    let piece = st
        .character
        .inventory
        .iter()
        .find(|s| s.item == ItemId::new("coldfin_2"))
        .and_then(|s| s.copy_id())
        .unwrap();
    let fit = game::Action::Fit {
        host: game::Host::Worn(game::Slot::Deck),
        socket: 0,
        piece,
    };
    crate::actions::perform(&mut st, fit).unwrap();
    let map = MapScreen::walking(&mut st);
    st.tick += 200;
    for (tab, name) in [('3', "gear_marks"), ('4', "inventory_marks")] {
        let mut deck = Deck::new(st.tick);
        deck.update(&mut st, &[key(KeyCode::Char(tab))]);
        shot_stack(&format!("{name}_100x30"), &st, &[&map, &deck], (100, 30));
    }
}

#[test]
#[ignore]
fn a_missing_blueprint() {
    let mut st = state_home();
    blueprint_shots(
        &mut st,
        '4',
        "copper_coil",
        "blueprint_missing",
        &[(100, 30)],
    );
}

#[test]
#[ignore]
fn a_neon_keypad() {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    let b = {
        let city = st.city.as_ref().unwrap();
        city.buildings
            .iter()
            .find(|b| {
                b.kind == citygen::BuildingKind::Apartment
                    && matches!(
                        city.district(b.district).theme,
                        citygen::Theme::NightMarket | citygen::Theme::NeonStrip
                    )
            })
            .map(|b| b.id)
    };
    let Some(b) = b else { return };
    crate::credentials::prepare(&mut st, b);
    let Some((level, number)) = st.interiors.homes_of(b).and_then(|h| h.flats().next()) else {
        return;
    };
    st.tick += 200;
    let pad = Keypad::open(&st, b, level, number);
    shot("keypad_neon", &st, &map, &pad);
}

#[test]
#[ignore]
fn offer_window() {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    let welcome = st.jobs.active().unwrap().id;
    st.jobs.drop_job(welcome);
    let fx = st
        .world
        .as_ref()
        .unwrap()
        .people()
        .iter()
        .find(|p| p.profession == "fixer")
        .unwrap()
        .id;
    st.jobs.fixer_mut(fx).standing = 0;
    let id = crate::jobs::offer(&mut st, fx).expect("an offer");
    let popup = crate::jobs::ui::offer_popup(&st, id, st.tick);
    st.tick += 200;
    shot("offer", &st, &map, &popup);
}
