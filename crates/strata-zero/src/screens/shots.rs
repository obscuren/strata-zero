//! Screenshots for a look at the devices: ignored tests that draw a screen over the
//! washed map and write it as an SVG. Run with `SHOT_DIR=... cargo test -p strata-zero
//! shots:: -- --ignored`.

use crate::app::{AppState, Screen};
use crate::helix::shell::HostId;
use crate::screens::blueprint::Blueprint;
use crate::screens::deck::Deck;
use crate::screens::explore::MapScreen;
use crate::screens::keypad::Keypad;
use crate::screens::kit::{Kit, Pane};
use crate::screens::popup::Popup;
use crate::screens::terminal::Terminal;
use crate::screens::testutil::key;
use citygen::{GenParams, Generator, Place, Pos, SizePreset, StratumKind};
use crossterm::event::KeyCode;
use game::{ItemId, ItemKind, Slot};
use ratatui::Terminal as Term;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
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

/// The buffer as an SVG: a rect per cell background, a text run per cell. `square`
/// pads the canvas to a square for the thumbnailer, which crops anything else; the
/// site takes the terminal as it is.
fn svg(buf: &Buffer, bg: &str, square: bool) -> String {
    let (w, h) = (
        buf.area.width as f32 * CELL_W,
        buf.area.height as f32 * CELL_H,
    );
    let (w, h) = if square { (w.max(h), w.max(h)) } else { (w, h) };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">"
    );
    let _ = writeln!(out, "<rect width=\"{w}\" height=\"{h}\" fill=\"{bg}\"/>");
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
fn draw_stack(st: &AppState, stack: &[&dyn Screen], (w, h): (u16, u16)) -> Buffer {
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
    t.backend().buffer().clone()
}

/// `buf` as `<dir>/<name>.svg` on the city's background.
fn write_svg(dir: &str, name: &str, st: &AppState, buf: &Buffer, square: bool) {
    let bg = hex(st.style.background.to_color(), "#04060a");
    std::fs::write(format!("{dir}/{name}.svg"), svg(buf, &bg, square)).unwrap();
}

/// `draw_stack` written to `SHOT_DIR`, square; nothing without it.
fn shot_stack(name: &str, st: &AppState, stack: &[&dyn Screen], size: (u16, u16)) {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    write_svg(&dir, name, st, &draw_stack(st, stack, size), true);
}

/// The lot from the title's COMBAT entry, walked four tiles into the goon's sight,
/// on your act half with the cursor on the goon.
fn combat_aim() -> (AppState, MapScreen) {
    let mut st = AppState::new();
    let mut title = crate::screens::title::Title::new(0);
    title.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Down)]);
    let _ = title.update(&mut st, &[key(KeyCode::Enter)]);
    let mut map = MapScreen::walking(&mut st);
    st.tick += 100;
    // East along the road until the goon in the far corner sees you.
    for _ in 0..40 {
        if st.net.engaged.is_some() {
            break;
        }
        map.update(&mut st, &[key(KeyCode::Char('l'))]);
    }
    map.update(&mut st, &[key(KeyCode::Char(' '))]);
    map.update(&mut st, &[]);
    (st, map)
}

/// The COMBAT start on the Net: jacked into a construct, a beat in.
#[test]
#[ignore]
fn combat_net_start() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let mut st = AppState::new();
    let mut title = crate::screens::title::Title::new(0);
    title.update(
        &mut st,
        &[key(KeyCode::Down), key(KeyCode::Down), key(KeyCode::Left)],
    );
    let crate::app::ScreenCmd::Switch { to: mut map, .. } =
        title.update(&mut st, &[key(KeyCode::Enter)])
    else {
        panic!("straight to the lot's construct")
    };
    for _ in 0..60 {
        st.tick += 1;
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + 1);
        }
        map.update(&mut st, &[]);
    }
    let buf = draw_stack(&st, &[&*map], (100, 30));
    write_svg(&dir, "combat_net", &st, &buf, false);
    // Then an attack on the dummy from two tiles off: the streak, then the tear
    // and the tag where it lands.
    st.player.as_mut().unwrap().pos = citygen::Pos::new(4, 4);
    crate::net::engage(
        &mut st,
        citygen::Place::Construct(citygen::BuildingId(0)),
        true,
    );
    crate::net::stay(&mut st);
    st.net.force_roll = Some(10);
    map.update(&mut st, &[]);
    map.update(&mut st, &[key(KeyCode::Char(' '))]);
    for (name, ticks) in [("combat_net_attack_1", 1), ("combat_net_attack_4", 3)] {
        st.tick += ticks;
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + ticks);
        }
        map.update(&mut st, &[]);
        write_svg(
            &dir,
            name,
            &st,
            &draw_stack(&st, &[&*map], (100, 30)),
            false,
        );
    }
}

#[test]
#[ignore]
fn combat_aim_on_the_lot() {
    let (st, map) = combat_aim();
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    write_svg(
        &dir,
        "combat_aim",
        &st,
        &draw_stack(&st, &[&map], (100, 30)),
        false,
    );
}

#[test]
#[ignore]
fn combat_burst_in_flight() {
    let (mut st, mut map) = combat_aim();
    st.force_hit = Some(true);
    map.update(&mut st, &[key(KeyCode::Char(' '))]);
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    // One, five and nine ticks into the burst: the flash, the second round's
    // streak with the first number up, the third round landing.
    for (name, ticks) in [
        ("combat_burst_1", 1),
        ("combat_burst_5", 4),
        ("combat_burst_9", 4),
    ] {
        st.tick += ticks;
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + ticks);
        }
        write_svg(&dir, name, &st, &draw_stack(&st, &[&map], (100, 30)), false);
    }
}

/// The lot with the combat runner's pack, the goon at `goon` (and a second one
/// at `other`), on the act half with the cursor on the first goon.
fn runner_aim(goon: Pos, other: Option<Pos>) -> (AppState, MapScreen) {
    let mut st = crate::foes::testutil::lot(goon);
    if let Some(p) = other {
        st.foes.push(crate::foes::Foe::goon(
            Place::Stratum(StratumKind::Street),
            p,
        ));
    }
    st.character = game::Character::combat_runner(&st.content);
    let mut map = MapScreen::walking(&mut st);
    st.tick += 100;
    map.update(&mut st, &[]);
    map.update(&mut st, &[key(KeyCode::Char(' '))]);
    map.update(&mut st, &[]);
    (st, map)
}

/// Sets the clock `e` ticks into the first of `st`'s shapes that `pick` takes.
fn at_shape(st: &mut AppState, pick: impl Fn(&game::Fx) -> bool, e: u64) {
    let started = st
        .fx
        .iter()
        .find(|i| pick(&i.fx))
        .expect("the shape is up")
        .started;
    st.tick = started + e;
    if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
        world.advance(city, world.tick().max(st.tick));
    }
}

fn combat_shot(name: &str, st: &AppState, map: &MapScreen) {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    write_svg(&dir, name, st, &draw_stack(st, &[map], (140, 40)), false);
}

/// Arms `digit` on the cursor and throws.
fn throw_digit(st: &mut AppState, map: &mut MapScreen, digit: char) {
    map.update(st, &[key(KeyCode::Char(digit))]);
    st.force_hit = Some(true);
    map.update(st, &[key(KeyCode::Char(' '))]);
}

#[test]
#[ignore]
fn combat_armed_frag() {
    let (mut st, mut map) = runner_aim(Pos::new(12, 8), None);
    map.update(&mut st, &[key(KeyCode::Char('6'))]);
    combat_shot("combat_armed_frag", &st, &map);
}

#[test]
#[ignore]
fn combat_bloom_mid() {
    let (mut st, mut map) = runner_aim(Pos::new(12, 8), None);
    throw_digit(&mut st, &mut map, '6');
    at_shape(&mut st, |f| matches!(f, game::Fx::Bloom), 2);
    combat_shot("combat_bloom_mid", &st, &map);
}

#[test]
#[ignore]
fn combat_scorch_under_the_aim() {
    let (mut st, mut map) = runner_aim(Pos::new(12, 8), None);
    throw_digit(&mut st, &mut map, '6');
    at_shape(
        &mut st,
        |f| matches!(f, game::Fx::Bloom),
        crate::render::fx::BLOOM_TICKS + 2,
    );
    // Armed again on the goon standing on the scorch: the rings over it.
    map.update(&mut st, &[key(KeyCode::Char('6'))]);
    combat_shot("combat_scorch_under_the_aim", &st, &map);
}

#[test]
#[ignore]
fn combat_whiteout_mid() {
    let (mut st, mut map) = runner_aim(Pos::new(12, 8), None);
    throw_digit(&mut st, &mut map, '7');
    at_shape(&mut st, |f| matches!(f, game::Fx::Whiteout), 1);
    combat_shot("combat_whiteout_mid", &st, &map);
}

#[test]
#[ignore]
fn combat_shock_mid() {
    let (mut st, mut map) = runner_aim(Pos::new(12, 8), None);
    throw_digit(&mut st, &mut map, '8');
    at_shape(&mut st, |f| matches!(f, game::Fx::Shock), 1);
    combat_shot("combat_shock_mid", &st, &map);
}

#[test]
#[ignore]
fn combat_live_satchel() {
    // Two tiles short of the goon, the cursor put away after: the satchel lies live
    // with its fuse, the goon standing in its footprint.
    let (mut st, mut map) = runner_aim(Pos::new(12, 8), None);
    map.update(&mut st, &[key(KeyCode::Char('9'))]);
    map.update(&mut st, &[key(KeyCode::Char('h')), key(KeyCode::Char('h'))]);
    map.update(&mut st, &[key(KeyCode::Char(' '))]);
    map.update(&mut st, &[key(KeyCode::Esc)]);
    st.tick += 20;
    map.update(&mut st, &[]);
    combat_shot("combat_live_satchel", &st, &map);
}

#[test]
#[ignore]
fn combat_jolt_chain() {
    let (mut st, mut map) = runner_aim(Pos::new(8, 8), Some(Pos::new(10, 8)));
    st.force_hit = Some(true);
    map.update(&mut st, &[key(KeyCode::Char('2'))]);
    // Three ticks after the first arc lands the second one is in the air.
    let first = st.fx.iter().map(|i| i.started).min().unwrap_or_else(|| {
        let log: Vec<_> = st.log.iter().map(|e| e.text.clone()).collect();
        panic!("no arc: {log:?}")
    });
    st.tick = first + 3;
    combat_shot("combat_jolt_chain", &st, &map);
}

#[test]
#[ignore]
fn combat_marked() {
    let (mut st, mut map) = runner_aim(Pos::new(12, 8), None);
    st.force_hit = Some(true);
    map.update(&mut st, &[key(KeyCode::Char('5'))]);
    st.tick += 6;
    combat_shot("combat_marked", &st, &map);
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
fn district_terminal(st: &mut AppState, d: citygen::DistrictId) -> (MapScreen, Terminal) {
    let map = MapScreen::walking(st);
    let mut term = Terminal::open(
        st,
        HostId::District(d),
        (Place::Stratum(StratumKind::Street), Pos::new(1, 1)),
    );
    type_lines(st, &mut term, &["ls /net", "ssh 1", "door release"]);
    st.tick += 400;
    term.update(st, &[]);
    (map, term)
}

fn terminal_shot(name: &str, d: citygen::DistrictId) {
    let mut st = state_home();
    let (map, term) = district_terminal(&mut st, d);
    shot(name, &st, &map, &term);
}

/// The first district of `theme`, if the city has one.
fn district_of(st: &AppState, theme: citygen::Theme) -> Option<citygen::DistrictId> {
    let city = st.city.as_ref().unwrap();
    city.districts
        .iter()
        .find(|d| d.theme == theme)
        .map(|d| d.id)
}

#[test]
#[ignore]
fn terminals_by_theme() {
    let st = state_home();
    for (name, theme) in [
        ("terminal_megacorp", citygen::Theme::CorpCore),
        ("terminal_street", citygen::Theme::CombatZone),
        ("terminal_neon", citygen::Theme::NightMarket),
    ] {
        if let Some(d) = district_of(&st, theme) {
            terminal_shot(name, d);
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

/// The blueprint window on `item`, opened from GEAR & INVENTORY (`3`) over the map, at
/// each size: on the rack's worn copy when `worn`, else on the first copy in the pack.
fn blueprint_shots(st: &mut AppState, worn: bool, item: &str, name: &str, sizes: &[(u16, u16)]) {
    let (map, deck, window) = blueprint_stack(st, worn, item);
    for (w, h) in sizes {
        let name = format!("{name}_{w}x{h}");
        shot_stack(&name, st, &[&map, &deck, &window], (*w, *h));
    }
}

/// The screens `blueprint_shots` draws, settled: the map, GEAR & INVENTORY, the window.
fn blueprint_stack(st: &mut AppState, worn: bool, item: &str) -> (MapScreen, Deck, Blueprint) {
    let map = MapScreen::walking(st);
    let mut deck = Deck::new(st.tick);
    deck.update(st, &[key(KeyCode::Char('3'))]);
    deck.kit.state.pane = if worn { Pane::Rack } else { Pane::Pack };
    // The copy the deck screen would pass: the one worn, else the first carried.
    let id = ItemId::new(item);
    let copy = if worn {
        let slot = st.content.item(&id).and_then(|i| i.slot);
        slot.and_then(|s| st.character.worn_copy(s)).map(|k| k.id)
    } else {
        let carried = st.character.inventory.iter().find(|s| s.item == id);
        carried.and_then(|s| s.copy_id())
    };
    let window = Blueprint::open(st, id, worn, copy);
    // The glass typed out, clear of the deck's signal jitter (three ticks in ninety).
    st.tick += 200;
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    (map, deck, window)
}

#[test]
#[ignore]
fn the_deck_on_its_blueprint() {
    let mut st = state_home();
    let sizes = [(100, 30), (120, 40)];
    blueprint_shots(&mut st, true, "pawnshop_deck", "blueprint_deck", &sizes);
}

#[test]
#[ignore]
fn dshawns_decks_on_their_blueprints() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    let at = [(120, 40)];
    blueprint_shots(&mut st, true, "rebuilt_deck", "blueprint_dshawn", &at);
    // The pawnshop deck he carries: its own blueprint, read only.
    blueprint_shots(
        &mut st,
        false,
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
        blueprint_shots(&mut st, false, item, &name, &[(100, 30)]);
    }
}

/// The Whisper SMG D'SHAWN carries, on the Ratchet's drawing.
#[test]
#[ignore]
fn the_whisper_on_its_blueprint() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    let sizes = [(100, 30), (120, 40)];
    blueprint_shots(&mut st, false, "whisper_smg", "blueprint_whisper", &sizes);
}

/// D'SHAWN's worn Ratchet SMG on its blueprint, bare and with both pieces fitted.
#[test]
#[ignore]
fn the_ratchet_on_its_blueprint() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    let sizes = [(100, 30), (120, 40)];
    blueprint_shots(&mut st, true, "ratchet_smg", "blueprint_ratchet", &sizes);
    fit_on_the_ratchet(&mut st, &[(0, "hushcan"), (1, "redeye")]);
    blueprint_shots(
        &mut st,
        true,
        "ratchet_smg",
        "blueprint_ratchet_fitted",
        &sizes,
    );
}

/// Fits each `(socket, item)` piece from the pack into the worn Ratchet.
fn fit_on_the_ratchet(st: &mut AppState, pieces: &[(usize, &str)]) {
    for (socket, item) in pieces {
        let piece = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == ItemId::new(item))
            .and_then(|s| s.copy_id())
            .unwrap();
        let fit = game::Action::Fit {
            host: game::Host::Worn(game::Slot::Primary),
            socket: *socket,
            piece,
        };
        crate::actions::perform(st, fit).unwrap();
    }
}

/// The Ratchet SMG in the blueprint window's modding mode.
#[test]
#[ignore]
fn modding_the_ratchet() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    let map = MapScreen::walking(&mut st);
    let mut deck = Deck::new(st.tick);
    deck.update(&mut st, &[key(KeyCode::Char('3'))]);
    let copy = st.character.worn_copy(game::Slot::Primary).map(|k| k.id);
    let mut window = Blueprint::open(&st, ItemId::new("ratchet_smg"), true, copy);
    st.tick += 200;
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    let sizes = [(100, 30), (120, 40)];
    // The pointer on the muzzle.
    window.update(&mut st, &[key(KeyCode::Char('m'))]);
    for (w, h) in sizes {
        let name = format!("modding_the_ratchet_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck, &window], (w, h));
    }
    // The tray open on the muzzle.
    window.update(&mut st, &[key(KeyCode::Char(' '))]);
    for (w, h) in sizes {
        let name = format!("modding_the_ratchet_tray_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck, &window], (w, h));
    }
    // Both pieces fitted, the pointer on the rail.
    fit_on_the_ratchet(&mut st, &[(0, "hushcan"), (1, "redeye")]);
    window.update(&mut st, &[key(KeyCode::Esc), key(KeyCode::Left)]);
    for (w, h) in sizes {
        let name = format!("modding_the_ratchet_fitted_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck, &window], (w, h));
    }
}

/// The blueprint window on what D'SHAWN wears in `slot`, opened from GEAR, the
/// glass typed out.
fn worn_window(st: &mut AppState, item: &str, slot: game::Slot) -> (MapScreen, Deck, Blueprint) {
    st.character = game::Character::debug_runner(&st.content);
    let map = MapScreen::walking(st);
    let mut deck = Deck::new(st.tick);
    deck.update(st, &[key(KeyCode::Char('3'))]);
    let copy = st.character.worn_copy(slot).map(|k| k.id);
    let window = Blueprint::open(st, ItemId::new(item), true, copy);
    st.tick += 200;
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    (map, deck, window)
}

/// D'SHAWN's rebuilt deck in the sticker mode: the tray risen, ARASAKA placed level
/// and turned, a peel armed; and the deck inspected with what was stuck.
#[test]
#[ignore]
fn stickers_on_dshawns_deck() {
    let mut st = state_home();
    let (map, deck, mut window) = worn_window(&mut st, "rebuilt_deck", game::Slot::Deck);
    let sizes = [(100, 30), (120, 40)];
    let snap = |name: &str, st: &AppState, window: &Blueprint| {
        for (w, h) in sizes {
            shot_stack(
                &format!("stickers_deck_{name}_{w}x{h}"),
                st,
                &[&map, &deck, window],
                (w, h),
            );
        }
    };
    snap("inspect", &st, &window);
    window.update(&mut st, &[key(KeyCode::Char('s'))]);
    snap("tray", &st, &window);
    let right = key(KeyCode::Right);
    window.update(&mut st, &[right, right, key(KeyCode::Char(' '))]);
    snap("placing", &st, &window);
    window.update(&mut st, &[key(KeyCode::Char('r'))]);
    snap("turned", &st, &window);
    window.update(
        &mut st,
        &[
            key(KeyCode::Char(' ')),
            key(KeyCode::Up),
            key(KeyCode::Char(' ')),
        ],
    );
    snap("peel", &st, &window);
    window.update(&mut st, &[key(KeyCode::Esc), key(KeyCode::Char('s'))]);
    snap("stuck", &st, &window);
}

/// The Ratchet in the sticker mode: the tray, then FLATLINE placed on the receiver.
#[test]
#[ignore]
fn stickers_on_the_ratchet() {
    let mut st = state_home();
    let (map, deck, mut window) = worn_window(&mut st, "ratchet_smg", game::Slot::Primary);
    window.update(&mut st, &[key(KeyCode::Char('s'))]);
    for (w, h) in [(100, 30), (120, 40)] {
        let name = format!("stickers_ratchet_tray_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck, &window], (w, h));
    }
    let right = key(KeyCode::Right);
    window.update(
        &mut st,
        &[right, right, right, right, right, key(KeyCode::Char(' '))],
    );
    for (w, h) in [(100, 30), (120, 40)] {
        let name = format!("stickers_ratchet_placing_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck, &window], (w, h));
    }
}

/// Each piece of weapon hardware on its blueprint.
#[test]
#[ignore]
fn weapon_hardware_blueprints() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    for item in ["hushcan", "redeye"] {
        let name = format!("hardware_{item}");
        blueprint_shots(&mut st, false, item, &name, &[(100, 30), (120, 40)]);
    }
}

/// The deck's WORKBENCH on D'SHAWN's drawings: the Ratchet exploded with its barrel
/// selected, then whole with a barrel in the pack, and a process sheet.
#[test]
#[ignore]
fn workbench_sheets() {
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    let map = MapScreen::walking(&mut st);
    // Clear of the deck's signal jitter (three ticks in ninety).
    st.tick += 200;
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    let sheet = |st: &mut AppState, keys: &[KeyCode]| {
        let mut deck = Deck::new(st.tick);
        deck.update(st, &[key(KeyCode::Char('4'))]);
        for k in keys {
            deck.update(st, &[key(*k)]);
        }
        deck
    };
    let ratchet = [KeyCode::Left, KeyCode::Down, KeyCode::Down];
    let deck = sheet(&mut st, &ratchet);
    for (w, h) in [(100, 30), (120, 40)] {
        let name = format!("workbench_ratchet_{w}x{h}");
        shot_stack(&name, &st, &[&map, &deck], (w, h));
    }
    let barrel = game::ItemId::new("ratchet_barrel");
    st.character.give_item(&st.content, barrel, 1);
    let deck = sheet(&mut st, &ratchet);
    let name = "workbench_ratchet_whole_100x30";
    shot_stack(name, &st, &[&map, &deck], (100, 30));
    let stim = crate::screens::workbench::drawings(&st.content)
        .iter()
        .position(|&i| st.content.recipes[i].id.0 == "stim_batch")
        .expect("a Stim batch sheet");
    let deck = sheet(&mut st, &vec![KeyCode::Right; stim]);
    shot_stack("workbench_stim_100x30", &st, &[&map, &deck], (100, 30));
}

/// GEAR & INVENTORY at each size: D'SHAWN with the Hushcan and Redeye fitted on the
/// Ratchet and the rack on PRIMARY, then a fresh D'SHAWN with the pack filtered to
/// hardware and the cursor on the Hushcan.
#[test]
#[ignore]
fn gear_and_inventory() {
    let sizes = [(100, 30), (120, 40)];
    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    fit_on_the_ratchet(&mut st, &[(0, "hushcan"), (1, "redeye")]);
    let tile = Slot::ALL.iter().position(|s| *s == Slot::Primary).unwrap();
    kit_shots(&mut st, "gear_and_inventory_rack", &sizes, |kit, _| {
        kit.state.pane = Pane::Rack;
        kit.state.tile = tile;
    });

    let mut st = state_home();
    st.character = game::Character::debug_runner(&st.content);
    kit_shots(&mut st, "gear_and_inventory_pack", &sizes, |kit, st| {
        kit.state.pane = Pane::Pack;
        kit.state.filter = Some(ItemKind::Hardware);
        let rows = kit.pack_rows(st);
        kit.state.row = rows
            .iter()
            .position(|r| r.text.contains("Hushcan"))
            .unwrap();
    });
}

/// The deck on `3`, `set` applied to its kit, then drawn at each size once the card
/// has typed out, clear of the deck's signal jitter.
fn kit_shots(
    st: &mut AppState,
    name: &str,
    sizes: &[(u16, u16)],
    set: impl FnOnce(&mut Kit, &AppState),
) {
    let map = MapScreen::walking(st);
    let mut deck = Deck::new(st.tick);
    deck.update(st, &[key(KeyCode::Char('3'))]);
    set(&mut deck.kit, st);
    deck.kit.changed = st.tick;
    st.tick += 200;
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    for (w, h) in sizes {
        shot_stack(&format!("{name}_{w}x{h}"), st, &[&map, &deck], (*w, *h));
    }
}

#[test]
#[ignore]
fn a_missing_blueprint() {
    let mut st = state_home();
    blueprint_shots(
        &mut st,
        false,
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
    let (map, popup) = offer_stack(&mut st);
    shot("offer", &st, &map, &popup);
}

/// A fixer's offer over the map, the welcome job dropped first.
fn offer_stack(st: &mut AppState) -> (MapScreen, Popup) {
    let map = MapScreen::walking(st);
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
    let id = crate::jobs::offer(st, fx).expect("an offer");
    let popup = crate::jobs::ui::offer_popup(st, id, st.tick);
    st.tick += 200;
    (map, popup)
}

/// The city stratazero.io's screens are drawn in: the debug runner's kit as VEX,
/// without dev mode.
fn site_state() -> AppState {
    let mut st = state_home();
    st.dev = false;
    st.character = game::Character::debug_runner(&st.content);
    st.character.handle = "VEX".to_string();
    st
}

/// The rebuilt deck worn, on its blueprint, at 120×40.
fn cyberdeck_buffer(st: &mut AppState) -> Buffer {
    let (map, deck, window) = blueprint_stack(st, true, "rebuilt_deck");
    draw_stack(st, &[&map, &deck, &window], (120, 40))
}

/// The screens stratazero.io shows, at 120×40 and not padded. Run with `SHOT_DIR`
/// pointed at the site repo's `site/screens`.
#[test]
#[ignore]
fn for_the_site() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let size = (120, 40);

    let mut st = site_state();
    let d = district_of(&st, citygen::Theme::NightMarket).expect("a night market");
    let (map, term) = district_terminal(&mut st, d);
    let buf = draw_stack(&st, &[&map, &term], size);
    write_svg(&dir, "terminal", &st, &buf, false);

    let mut st = site_state();
    let buf = cyberdeck_buffer(&mut st);
    write_svg(&dir, "cyberdeck", &st, &buf, false);

    // Modding the Ratchet, both pieces fitted, the pointer on the rail.
    let mut st = site_state();
    let (map, deck, mut window) = blueprint_stack(&mut st, true, "ratchet_smg");
    window.update(&mut st, &[key(KeyCode::Char('m'))]);
    window.update(&mut st, &[key(KeyCode::Char(' '))]);
    fit_on_the_ratchet(&mut st, &[(0, "hushcan"), (1, "redeye")]);
    window.update(&mut st, &[key(KeyCode::Esc), key(KeyCode::Left)]);
    let buf = draw_stack(&st, &[&map, &deck, &window], size);
    write_svg(&dir, "ratchet", &st, &buf, false);

    let mut st = site_state();
    let (map, popup) = offer_stack(&mut st);
    let buf = draw_stack(&st, &[&map, &popup], size);
    write_svg(&dir, "offer", &st, &buf, false);
}

#[test]
fn an_exact_svg_is_the_terminal_not_a_square() {
    let buf = Buffer::empty(Rect::new(0, 0, 120, 40));
    let exact = svg(&buf, "#0a0b10", false);
    assert!(exact.starts_with(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1152" height="800" viewBox="0 0 1152 800">"#
    ));
    assert!(svg(&buf, "#0a0b10", true).contains(r#"height="1152""#));
}

#[test]
fn the_site_sees_vex_without_dev_mode() {
    let mut st = site_state();
    assert!(!st.dev);
    assert_eq!(st.character.handle, "VEX");
    let buf = cyberdeck_buffer(&mut st);
    let text: String = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_string())
        .collect();
    assert!(!text.contains("SHAWN"));
    assert!(text.contains("VEX"));
}
