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
pub(crate) fn draw_stack(st: &AppState, stack: &[&dyn Screen], (w, h): (u16, u16)) -> Buffer {
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

/// `buf` as `<dir>/<name>.svg` on the city's background; with the window feature, also
/// as `<name>.png` painted the way the window paints it.
fn write_svg(dir: &str, name: &str, st: &AppState, buf: &Buffer, square: bool) {
    let bg = hex(st.style.background.to_color(), "#04060a");
    std::fs::write(format!("{dir}/{name}.svg"), svg(buf, &bg, square)).unwrap();
    #[cfg(feature = "window")]
    crate::window::png::write_buffer(dir, name, buf).unwrap();
}

/// `draw_stack` written to `SHOT_DIR`, square; nothing without it.
fn shot_stack(name: &str, st: &AppState, stack: &[&dyn Screen], size: (u16, u16)) {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    write_svg(&dir, name, st, &draw_stack(st, stack, size), true);
}

/// The lot from the title's COMBAT entry (dev mode), walked four tiles into the goon's
/// sight, on your act half with the cursor on the goon.
fn combat_aim() -> (AppState, MapScreen) {
    let mut st = AppState::new();
    st.unlock_dev();
    let mut title = crate::screens::title::Title::new(&st, 0);
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

/// The launch splash: the scan, the key found, the dump dissolving and the bytes in
/// flight to the logo, at the usual size and the smallest.
#[test]
#[ignore]
fn splash() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let splash = crate::screens::splash::Splash::new(0, 7);
    let mut st = AppState::new();
    for (name, tick) in [
        ("scan", 30),
        ("found", 76),
        ("dissolve", 96),
        ("flight", 118),
    ] {
        st.tick = tick;
        for (w, h) in [(120, 40), (100, 30)] {
            let buf = draw_stack(&st, &[&splash], (w, h));
            write_svg(&dir, &format!("splash_{name}_{w}x{h}"), &st, &buf, false);
        }
    }
}

/// The COMBAT lot (dev mode) with the Scout drone out: three steps east and a few ticks,
/// so it trails you with its wake behind it.
#[test]
#[ignore]
fn drone_on_the_lot() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let mut st = AppState::new();
    st.unlock_dev();
    let mut title = crate::screens::title::Title::new(&st, 0);
    title.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Down)]);
    let _ = title.update(&mut st, &[key(KeyCode::Enter)]);
    let mut map = MapScreen::walking(&mut st);
    for _ in 0..60 {
        st.tick += 1;
        map.update(&mut st, &[]);
    }
    for _ in 0..3 {
        map.update(&mut st, &[key(KeyCode::Char('l'))]);
        st.tick += 2;
        map.update(&mut st, &[]);
    }
    for _ in 0..4 {
        st.tick += 1;
        map.update(&mut st, &[]);
    }
    assert!(!st.bots.is_empty(), "the drone is out");
    for (w, h) in [(100, 30), (120, 40)] {
        let buf = draw_stack(&st, &[&map], (w, h));
        write_svg(&dir, &format!("drone_on_the_lot_{w}x{h}"), &st, &buf, false);
    }
}

/// The COMBAT start on the Net (dev mode): jacked into a construct, a beat in.
#[test]
#[ignore]
fn combat_net_start() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let mut st = AppState::new();
    st.unlock_dev();
    let mut title = crate::screens::title::Title::new(&st, 0);
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
fn combat_haze_mid() {
    // The choke can is packed last: `0`, the tenth act key.
    let (mut st, mut map) = runner_aim(Pos::new(12, 8), None);
    throw_digit(&mut st, &mut map, '0');
    at_shape(&mut st, |f| matches!(f, game::Fx::Haze), 1);
    combat_shot("combat_haze_mid", &st, &map);
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
    crate::jobs::first_favour(&mut st);
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
    // A new game mails nothing now; the favour's mail is the one the rig reads.
    crate::jobs::first_favour(&mut st);
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

/// D'SHAWN's rebuilt deck in the sticker mode: the tray risen, ORODYNE placed level
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

/// ATTRIBUTES at each size: the spec's loadout on the Piston arm; the picker on R ARM with
/// a Grip arm in the pack; the picker on the forearm with a Monowire spool; the swap
/// picker on the Piston arm at L ARM, the Grip arm picked; the left pane on EMP, two
/// off for each piece; and on REFLEX, its readout crowded with sources and its cap.
#[test]
#[ignore]
fn attributes_sheet() {
    use crate::screens::sheet::{LedgerRow, SheetPane};
    use game::{Num, Site, Stack};
    let sizes = [(100, 30), (120, 40)];
    let fresh = || {
        let mut st = state_home();
        st.character = game::Character::debug_runner(&st.content);
        for (site, item) in [
            (Site::Neural, "neural_port"),
            (Site::Eyes, "target_lens"),
            (Site::Jaw, "bulldog_jaw"),
            (Site::Spine, "redline_spine"),
            (Site::Back, "drone_cradle"),
            (Site::Organs, "adrenal_pump"),
            (Site::Skin, "subdermal_mesh"),
            (Site::LeftArm, "piston_arm"),
        ] {
            let id = ItemId::new(item);
            let copy = st.content.item(&id).unwrap().per_copy().then(|| {
                let mut k = st.character.new_copy(&st.content, &id);
                // The arm with its Arc hand, the cradle with its Scout drone.
                let fitted = match item {
                    "piston_arm" => Some("arc_hand"),
                    "drone_cradle" => Some("scout_drone"),
                    _ => None,
                };
                if let Some(piece) = fitted {
                    let piece = ItemId::new(piece);
                    let pc = st.character.new_copy(&st.content, &piece);
                    k.fitted[0] = Some(Stack {
                        item: piece,
                        qty: 1,
                        copy: Some(pc),
                    });
                }
                k
            });
            st.character.install(site, id, copy);
        }
        st
    };
    let mut st = fresh();
    sheet_shots(&mut st, "attributes_ledger_limb", &sizes, |d, _| {
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Site(Site::LeftArm);
    });

    // The cradle, its bay with the Scout drone docked, and what docks there.
    let mut st = fresh();
    sheet_shots(&mut st, "attributes_ledger_cradle", &sizes, |d, _| {
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Site(Site::Back);
    });
    let mut st = fresh();
    sheet_shots(&mut st, "attributes_ledger_bay", &sizes, |d, _| {
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Socket(Site::Back, 0);
    });
    let mut st = fresh();
    sheet_shots(&mut st, "attributes_picker_bay", &sizes, |d, st| {
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Socket(Site::Back, 0);
        d.update(st, &[key(KeyCode::Char(' '))]);
    });

    let mut st = fresh();
    st.character
        .give_item(&st.content, ItemId::new("grip_arm"), 1);
    sheet_shots(&mut st, "attributes_picker_site", &sizes, |d, st| {
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Site(Site::RightArm);
        d.update(st, &[key(KeyCode::Char(' '))]);
    });

    let mut st = fresh();
    st.character
        .give_item(&st.content, ItemId::new("monowire_spool"), 1);
    sheet_shots(&mut st, "attributes_picker_socket", &sizes, |d, st| {
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Socket(Site::LeftArm, 1);
        d.update(st, &[key(KeyCode::Char(' '))]);
    });

    let mut st = fresh();
    st.character
        .give_item(&st.content, ItemId::new("grip_arm"), 1);
    sheet_shots(&mut st, "attributes_picker_swap", &sizes, |d, st| {
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Site(Site::LeftArm);
        d.update(st, &[key(KeyCode::Char(' '))]);
    });

    let mut st = fresh();
    let emp = Num::ALL
        .iter()
        .position(|&n| n == Num::Resist(game::Harm::Emp))
        .unwrap();
    sheet_shots(&mut st, "attributes_left_emp", &sizes, |d, _| {
        d.sheet.state.tile = emp;
    });

    // REFLEX, crowded: the Monowire shiv worn as well, so gear, chrome, talents and the
    // cap want more rows than the readout has at 100×30.
    let mut st = fresh();
    st.character
        .wear(game::Slot::Secondary, ItemId::new("shiv_blade"), None);
    let reflex = Num::ALL
        .iter()
        .position(|&n| n == Num::Attribute(game::Attribute::Reflex))
        .unwrap();
    sheet_shots(&mut st, "attributes_left_reflex", &sizes, |d, _| {
        d.sheet.state.tile = reflex;
    });
}

/// The deck on `2` (ATTRIBUTES), `set` applied, then drawn at each size once the card and
/// the readout have typed out, clear of the deck's signal jitter.
fn sheet_shots(
    st: &mut AppState,
    name: &str,
    sizes: &[(u16, u16)],
    set: impl FnOnce(&mut Deck, &mut AppState),
) {
    let map = MapScreen::walking(st);
    let mut deck = Deck::new(st.tick);
    deck.update(st, &[key(KeyCode::Char('2'))]);
    set(&mut deck, st);
    deck.sheet.changed = st.tick;
    deck.sheet.card = st.tick;
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

/// A fixer's offer over the map, nothing in hand.
fn offer_stack(st: &mut AppState) -> (MapScreen, Popup) {
    let map = MapScreen::walking(st);
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

/// The SKILLS board with IRON unfolded on →, then its buy popup open on space.
#[test]
#[ignore]
fn the_skills_board_asks_before_it_buys() {
    let mut st = state_home();
    st.character.sp = 200;
    let map = MapScreen::walking(&mut st);
    let mut deck = Deck::new(st.tick);
    deck.update(&mut st, &[key(KeyCode::Char('1'))]);
    deck.row = deck
        .rows(&st)
        .iter()
        .position(|r| r.text.contains("IRON"))
        .unwrap();
    deck.update(&mut st, &[key(KeyCode::Right)]);
    st.tick += 200;
    if st.tick % 90 < 3 {
        st.tick += 3;
    }
    shot_stack("skills_board_120x40", &st, &[&map, &deck], (120, 40));
    // The popup anchors to the list the deck last drew.
    draw_stack(&st, &[&map, &deck], (120, 40));
    let crate::app::ScreenCmd::Push { to: popup, .. } =
        deck.update(&mut st, &[key(KeyCode::Char(' '))])
    else {
        panic!("space opens the buy popup")
    };
    st.tick += 40;
    let stack: [&dyn Screen; 3] = [&map, &deck, &*popup];
    shot_stack("skills_board_buy_120x40", &st, &stack, (120, 40));
}

/// Skill cards at the minimum terminal: hardware learned and not, a sign lit and cold, a
/// drawing with a label, and three categories' cards; then ICE breaker at 120x40 and
/// scanning in.
#[test]
#[ignore]
fn skill_cards() {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    let mut deck = Deck::new(st.tick);
    deck.update(&mut st, &[key(KeyCode::Char('1'))]);
    let on = |deck: &mut Deck, st: &mut AppState, name: &str| {
        deck.row = deck
            .rows(st)
            .iter()
            .position(|r| r.text.contains(name))
            .unwrap();
        deck.update(st, &[]);
        deck.changed = st.tick;
    };
    for name in [
        "ICE breaker",
        "Vault run",
        "Streetwise",
        "Haggle",
        "Fixer's ear",
        "NETRUNNER",
        "IRON",
        "GHOST",
    ] {
        on(&mut deck, &mut st, name);
        st.tick += 200;
        if st.tick % 90 < 3 {
            st.tick += 3;
        }
        let file = name.replace([' ', '\''], "_").to_lowercase();
        shot_stack(
            &format!("skill_card_{file}_100x30"),
            &st,
            &[&map, &deck],
            (100, 30),
        );
    }
    on(&mut deck, &mut st, "ICE breaker");
    st.tick += 200;
    shot_stack(
        "skill_card_ice_breaker_120x40",
        &st,
        &[&map, &deck],
        (120, 40),
    );
    on(&mut deck, &mut st, "ICE breaker");
    st.tick += 3;
    shot_stack("skill_card_scanning_100x30", &st, &[&map, &deck], (100, 30));
}

/// The creation wizard's SKILLS step with VEX's build: NETRUNNER's card, Hacking's and
/// IRON's at the minimum terminal, then NETRUNNER's at 120x40.
#[test]
#[ignore]
fn creation_skill_cards() {
    let mut st = AppState::new();
    let mut create = crate::screens::create::chooser::Chooser::new(&st, 0).choose(&mut st);
    // A step a frame: back from SUMMARY past NAME to SKILLS.
    create.update(&mut st, &[key(KeyCode::Esc)]);
    create.update(&mut st, &[key(KeyCode::Esc)]);
    // Down from NETRUNNER: Hacking is one row on, IRON eight.
    for (name, downs) in [("netrunner", 0), ("hacking", 1), ("iron", 7)] {
        create.update(&mut st, &vec![key(KeyCode::Down); downs]);
        st.tick += 200;
        shot_stack(
            &format!("creation_{name}_100x30"),
            &st,
            &[&create],
            (100, 30),
        );
    }
    let mut create = crate::screens::create::chooser::Chooser::new(&st, 0).choose(&mut st);
    // A step a frame: back from SUMMARY past NAME to SKILLS.
    create.update(&mut st, &[key(KeyCode::Esc)]);
    create.update(&mut st, &[key(KeyCode::Esc)]);
    st.tick += 200;
    shot_stack("creation_netrunner_120x40", &st, &[&create], (120, 40));
}

/// The way in: NAME with perma death on (both sizes) and turned off on its field; each
/// shipped broadcast at 120x40 in a full-size city, STATIC 66 at 100x30, and STATIC 66
/// with perma death off and the tutorial ticked.
#[test]
#[ignore]
fn broadcast_and_perma_death() {
    use crate::screens::broadcast::Broadcast;
    let mut st = AppState::new();
    let mut create = crate::screens::create::chooser::Chooser::new(&st, 0).choose(&mut st);
    // From SUMMARY back to NAME.
    create.update(&mut st, &[key(KeyCode::Esc)]);
    st.tick += 200;
    for (w, h) in [(120, 40), (100, 30)] {
        shot_stack(
            &format!("name_perma_death_{w}x{h}"),
            &st,
            &[&create],
            (w, h),
        );
    }
    create.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Right)]);
    shot_stack("name_perma_death_off_120x40", &st, &[&create], (120, 40));

    let city = Generator::run(GenParams::default().with_seed(77)).unwrap();
    let space = key(KeyCode::Char(' '));
    for b in st.broadcasts.0.clone() {
        let mut on_air = Broadcast::with(&st, city.clone(), &b);
        on_air.update(&mut st, &[space]);
        shot_stack(
            &format!("broadcast_{}_120x40", b.id),
            &st,
            &[&on_air],
            (120, 40),
        );
        if b.id == "static66" {
            shot_stack("broadcast_static66_100x30", &st, &[&on_air], (100, 30));
            on_air.update(&mut st, &[key(KeyCode::Char('p')), key(KeyCode::Char('t'))]);
            shot_stack("broadcast_static66_off_120x40", &st, &[&on_air], (120, 40));
        }
    }
}

/// A ripperdoc's: the shut door by day, the chair's window by night, and the ledger in
/// the chair on a piece, in FITS paying and short, on a swap, asking before a pull, and
/// with free chrome, at both sizes.
#[test]
#[ignore]
fn ripperdoc_clinic() {
    use crate::clinic::testutil::{DAY, inside, outside, seated};
    use crate::screens::sheet::LedgerRow;
    use game::{Site, Stack};
    let sizes = [(100, 30), (120, 40)];

    let (mut st, _, _, _) = outside(DAY);
    let map = MapScreen::walking(&mut st);
    let (_, b, door, out) = outside(DAY);
    let _ = b;
    st.player.as_mut().unwrap().pos = out;
    st.tick += 200;
    let subjects = crate::interact::gather(&st, &|_, _| None);
    let shut = subjects.iter().find(|s| s.pos == door).unwrap();
    let pop = shut.window(st.tick.saturating_sub(200));
    for (w, h) in sizes {
        shot_stack(
            &format!("clinic_shut_door_{w}x{h}"),
            &st,
            &[&map, &pop],
            (w, h),
        );
    }

    let (mut st, _, _, chair) = inside();
    let map = MapScreen::walking(&mut st);
    st.tick += 200;
    let subjects = crate::interact::gather(&st, &|_, _| None);
    let s = subjects.iter().find(|s| s.pos == chair).unwrap();
    let pop = s.window(st.tick.saturating_sub(200));
    for (w, h) in sizes {
        shot_stack(
            &format!("clinic_chair_window_{w}x{h}"),
            &st,
            &[&map, &pop],
            (w, h),
        );
    }

    let loadout = |st: &mut AppState| {
        st.character = game::Character::debug_runner(&st.content);
        for (site, item) in [
            (Site::Neural, "neural_port"),
            (Site::Eyes, "target_lens"),
            (Site::Jaw, "bulldog_jaw"),
            (Site::Spine, "redline_spine"),
            (Site::Back, "drone_cradle"),
            (Site::Organs, "adrenal_pump"),
            (Site::Skin, "subdermal_mesh"),
            (Site::LeftArm, "piston_arm"),
        ] {
            let id = ItemId::new(item);
            let copy = st.content.item(&id).unwrap().per_copy().then(|| {
                let mut k = st.character.new_copy(&st.content, &id);
                if item == "piston_arm" {
                    let hand = ItemId::new("arc_hand");
                    let pc = st.character.new_copy(&st.content, &hand);
                    k.fitted[0] = Some(Stack {
                        item: hand,
                        qty: 1,
                        copy: Some(pc),
                    });
                }
                k
            });
            st.character.install(site, id, copy);
        }
    };
    let chair_shots = |name: &str, credits: u32, free: bool, row: LedgerRow, keys: &[KeyCode]| {
        let mut st = seated();
        loadout(&mut st);
        st.character.credits = credits;
        st.free_chrome = free;
        st.character
            .give_item(&st.content, ItemId::new("grip_arm"), 1);
        let map = MapScreen::walking(&mut st);
        let mut deck = Deck::chair(&st);
        deck.sheet.state.row = row;
        for k in keys {
            deck.update(&mut st, &[key(*k)]);
        }
        deck.sheet.changed = st.tick;
        deck.sheet.card = st.tick;
        st.tick += 200;
        if st.tick % 90 < 3 {
            st.tick += 3;
        }
        for (w, h) in sizes {
            shot_stack(&format!("{name}_{w}x{h}"), &st, &[&map, &deck], (w, h));
        }
    };
    let arm = LedgerRow::Site(Site::LeftArm);
    let right = LedgerRow::Site(Site::RightArm);
    chair_shots("clinic_ledger_piece", 1200, false, arm, &[]);
    chair_shots(
        "clinic_fits_paying",
        1200,
        false,
        right,
        &[KeyCode::Char(' ')],
    );
    chair_shots(
        "clinic_fits_short",
        320,
        false,
        right,
        &[KeyCode::Char(' ')],
    );
    chair_shots("clinic_fits_swap", 1200, false, arm, &[KeyCode::Char(' ')]);
    chair_shots("clinic_x_asks", 1200, false, arm, &[KeyCode::Char('x')]);
    chair_shots("clinic_free", 1200, true, arm, &[]);
}

/// The two sizes the HOLO shots are drawn at.
const HOLO_SIZES: [(u16, u16); 2] = [(120, 40), (100, 30)];

/// Seed 8 with the story begun, the welcome closed (the package given, Marlo dead at his
/// station) and the player on the street beside the body, the map's camera there.
fn street_by_the_body() -> (AppState, MapScreen) {
    let mut st = state_home();
    let _ = MapScreen::walking(&mut st);
    crate::missions::begin(&mut st);
    st.missions.feeds.clear();
    crate::missions::end(&mut st, 0, 0);
    let body = st.missions.bodies[0].pos;
    let street = Place::Stratum(StratumKind::Street);
    let beside = {
        let city = st.city.as_ref().unwrap();
        let world = st.world.as_ref().unwrap();
        body.neighbors4()
            .find(|&n| city.walkable(StratumKind::Street, n) && world.person_at(n).is_none())
            .expect("a free tile beside the body")
    };
    {
        let p = st.player.as_mut().unwrap();
        p.place = street;
        p.pos = beside;
    }
    let bonus = st.sight_bonus();
    let mut p = st.player.take().unwrap();
    {
        let city = st.city.as_ref().unwrap();
        let map = crate::render::map::MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            street,
            st.minute_of_day(),
        )
        .unwrap();
        p.update_fov(&map, bonus);
    }
    st.player = Some(p);
    let map = MapScreen::walking(&mut st);
    (st, map)
}

/// A minute on from the welcome: Mezz's call rings.
fn ringing_call() -> (AppState, MapScreen) {
    let (mut st, map) = street_by_the_body();
    crate::missions::tick(&mut st);
    if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
        world.advance(city, world.tick() + sim::TICKS_PER_MINUTE);
    }
    crate::missions::tick(&mut st);
    assert!(st.missions.call.is_some(), "Mezz rings");
    (st, map)
}

fn holo_space(st: &mut AppState) {
    assert!(crate::missions::call::key(st, &key(KeyCode::Char(' '))));
}

#[test]
#[ignore]
fn holo_feed() {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    crate::missions::begin(&mut st);
    let view = st.missions.feeds.pop_front().unwrap();
    let feed = crate::missions::feed::Feed::new(view, st.tick);
    for (w, h) in HOLO_SIZES {
        shot_stack(&format!("holo_feed_{w}x{h}"), &st, &[&map, &feed], (w, h));
    }
}

/// The morning news: the broadcast at 07:14 over the map, Marlo dead and you seen
/// walking away north at 22:41, at both sizes.
#[test]
#[ignore]
fn news_shots() {
    use crate::missions::{Left, news, person_of};
    let mut st = state_home();
    let _ = MapScreen::walking(&mut st);
    crate::missions::begin(&mut st);
    st.missions.feeds.clear();
    crate::missions::end(&mut st, 0, 0);
    let want = 7 * 60 + 14;
    let ahead = (want + 1440 - st.minute_of_day() as u64) % 1440;
    {
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(city, world.tick() + ahead * sim::TICKS_PER_MINUTE);
    }
    news::put_out(&mut st, "marlo", None);
    let about = person_of(&st, &news::current(&st).unwrap().about).unwrap();
    // You walk away in what you wear.
    let worn = st.character.equipped(Slot::Torso).cloned();
    let body = st.missions.bodies.iter_mut().find(|b| b.person == about);
    body.expect("the item's body").left = Some(Left {
        at: (22, 41),
        heading: "north".to_string(),
        worn,
    });
    let map = MapScreen::walking(&mut st);
    let broadcast = crate::missions::broadcast::Broadcast::new();
    for (w, h) in HOLO_SIZES {
        let name = format!("news_broadcast_{w}x{h}");
        shot_stack(&name, &st, &[&map, &broadcast], (w, h));
    }
    // The set reading its first line to you by the couch, two seconds in.
    let (place, set) = news::set_tile(&st).unwrap();
    let mut me = crate::walker::Player::at(place, set.offset(-2, 1));
    {
        let city = st.city.as_ref().unwrap();
        let flat = crate::render::map::MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            place,
            st.minute_of_day(),
        )
        .unwrap();
        me.update_fov(&flat, st.sight_bonus());
    }
    st.player = Some(me);
    news::tick(&mut st, true);
    assert_eq!(st.missions.asides.len(), 1, "the set reads");
    {
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(
            city,
            world.tick() + 2 * crate::missions::call::TICKS_PER_SECOND,
        );
    }
    let map = MapScreen::walking(&mut st);
    for (w, h) in HOLO_SIZES {
        shot_stack(&format!("news_set_reading_{w}x{h}"), &st, &[&map], (w, h));
    }
    // Out on the street: someone three tiles off says the first overheard line, a second
    // in, you still seeing them.
    use crate::missions::news::tests as at;
    st.missions.asides.clear();
    at::to_a_heard_minute(&mut st, 0);
    at::by_someone(&mut st, 3);
    let spot = st.player.as_ref().unwrap().pos;
    news::tick(&mut st, true);
    assert_eq!(st.missions.asides.len(), 1, "overheard");
    let speaker = st.missions.asides[0].speaker;
    {
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(city, world.tick() + crate::missions::call::TICKS_PER_SECOND);
    }
    at::look_from(&mut st, Place::Stratum(StratumKind::Street), spot);
    if let crate::missions::asides::Speaker::Person(id) = speaker {
        let at = st.world.as_ref().unwrap().pos_of(id).unwrap();
        let seen = st.player.as_ref().unwrap().vis().get(at).copied();
        assert_eq!(seen, Some(crate::walker::Visibility::Visible), "still seen");
    }
    let map = MapScreen::walking(&mut st);
    for (w, h) in HOLO_SIZES {
        let drawn = crate::screens::testutil::buffer_text(&draw_stack(&st, &[&map], (w, h)));
        assert!(drawn.contains("overheard"), "a bubble over them at {w}x{h}");
        shot_stack(&format!("news_overheard_{w}x{h}"), &st, &[&map], (w, h));
    }
    // By a holo ad as it cuts in, a second in: its glow MAGENTA and its NEWSLINE aside.
    st.missions.asides.clear();
    at::an_ad_cutting_in(&mut st);
    {
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(city, world.tick() + crate::missions::call::TICKS_PER_SECOND);
    }
    assert!(news::cut_in(&st).is_some(), "still cutting in");
    let map = MapScreen::walking(&mut st);
    for (w, h) in HOLO_SIZES {
        let drawn = crate::screens::testutil::buffer_text(&draw_stack(&st, &[&map], (w, h)));
        assert!(drawn.contains("NEWSLINE"), "a bubble over it at {w}x{h}");
        shot_stack(&format!("news_holo_ad_{w}x{h}"), &st, &[&map], (w, h));
    }
    // The daze half way: rows adrift, a magenta and cyan fringe, the HUD still.
    st.missions.asides.clear();
    crate::missions::daze::start(&mut st, 4.0);
    st.tick += 20;
    assert!(crate::missions::daze::strength(&st) > 0.0, "still dizzy");
    let map = MapScreen::walking(&mut st);
    for (w, h) in HOLO_SIZES {
        shot_stack(&format!("news_daze_{w}x{h}"), &st, &[&map], (w, h));
    }
}

#[test]
#[ignore]
fn holo_call_ringing() {
    let (st, map) = ringing_call();
    for (w, h) in HOLO_SIZES {
        shot_stack(&format!("holo_call_ringing_{w}x{h}"), &st, &[&map], (w, h));
    }
}

#[test]
#[ignore]
fn holo_call_talking() {
    let (mut st, map) = ringing_call();
    holo_space(&mut st);
    holo_space(&mut st);
    for (w, h) in HOLO_SIZES {
        shot_stack(&format!("holo_call_talking_{w}x{h}"), &st, &[&map], (w, h));
    }
}

#[test]
#[ignore]
fn holo_call_glitch() {
    let (mut st, map) = ringing_call();
    holo_space(&mut st);
    holo_space(&mut st);
    let seed = crate::missions::face::seed_of(&st, "mezz");
    st.tick = (0..600)
        .find(|&t| crate::missions::face::burst(t, seed).is_some())
        .expect("a burst frame");
    for (w, h) in HOLO_SIZES {
        shot_stack(&format!("holo_call_glitch_{w}x{h}"), &st, &[&map], (w, h));
    }
}

/// TALENTS, the board, as the mockups drew it: EDGE with Quick draw 3, Double tap 2, Light
/// step 2 and Steady hand 1, Ice pick 2 in ICE, two points to burn; the cursor on Hair
/// trigger (open) and on Bullet time (out of reach); and MEAT with nothing spent and no
/// points, the board between levels.
#[test]
#[ignore]
fn talents_board() {
    use crate::ui::talents::TalentsState;
    let sizes = [(100u16, 30u16), (120, 40)];
    let runner = || {
        let mut st = state_home();
        st.character = game::Character::debug_runner(&st.content);
        st.character.talents.clear();
        for (id, r) in [
            ("quick_draw", 3u8),
            ("double_tap", 2),
            ("light_step", 2),
            ("steady_hand", 1),
            ("ice_pick", 2),
        ] {
            st.character.talents.insert(game::TalentId::new(id), r);
        }
        st.character.talent_points = 2;
        st
    };
    let edge = |spot: (usize, usize)| TalentsState {
        tree: 1,
        spots: vec![(0, 0), spot],
    };
    for (name, talents, points) in [
        ("talents_board_open", edge((2, 0)), 2),
        ("talents_board_locked", edge((2, 1)), 2),
        ("talents_board_meat_no_points", TalentsState::default(), 0),
    ] {
        let mut st = runner();
        st.character.talent_points = points;
        let map = MapScreen::walking(&mut st);
        let mut deck = Deck::new(st.tick);
        deck.update(&mut st, &[key(KeyCode::Char('0'))]);
        deck.talents = talents;
        deck.changed = st.tick;
        st.tick += 200;
        if st.tick % 90 < 3 {
            st.tick += 3;
        }
        for (w, h) in sizes {
            shot_stack(&format!("{name}_{w}x{h}"), &st, &[&map, &deck], (w, h));
        }
    }
    // ICE on the debug runner, every talent at its best: the cursor on Data surfer, and
    // on Slipstream, which scrolls the board at 100x30.
    for (name, spot) in [
        ("talents_board_ice_surfer", (4, 3)),
        ("talents_board_ice_slipstream", (5, 3)),
    ] {
        let mut st = state_home();
        st.character = game::Character::debug_runner(&st.content);
        let map = MapScreen::walking(&mut st);
        let mut deck = Deck::new(st.tick);
        deck.update(&mut st, &[key(KeyCode::Char('0'))]);
        deck.talents = TalentsState {
            tree: 4,
            spots: vec![(0, 0), (0, 0), (0, 0), (0, 0), spot],
        };
        deck.changed = st.tick;
        st.tick += 200;
        if st.tick % 90 < 3 {
            st.tick += 3;
        }
        for (w, h) in sizes {
            shot_stack(&format!("{name}_{w}x{h}"), &st, &[&map, &deck], (w, h));
        }
    }
}

/// Ticks pass on the clock and the world alike.
fn pass(st: &mut AppState, ticks: u64) {
    st.tick += ticks;
    if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
        world.advance(city, world.tick() + ticks);
    }
}

/// A calm crack on a door lock in a corp tower's construct, every tile remembered: the
/// run's tether half a beat on, at the smallest terminal too, on a hit and landed.
#[test]
#[ignore]
fn run_tether() {
    use citygen::{BuildingKind, NodeKind, SubnetOwner};
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let mut st = state_home();
    let mut map = MapScreen::walking(&mut st);
    let city = st.city.clone().unwrap();
    let b = city
        .buildings
        .iter()
        .filter(|b| {
            b.kind == BuildingKind::CorpTower
                && city
                    .net
                    .nodes
                    .iter()
                    .any(|n| n.building == Some(b.id) && n.kind == NodeKind::DoorLock)
        })
        .filter_map(|b| citygen::construct::generate_construct(&city, b.id).map(|k| (k, b.id)))
        .filter(|(k, _)| k.size.w <= 34 && k.size.h <= 18)
        .max_by_key(|(k, _)| (k.rooms.len(), k.size.w * k.size.h))
        .map(|(_, b)| b)
        .expect("a tower with a lock");
    st.city
        .as_mut()
        .unwrap()
        .net
        .subnets
        .iter_mut()
        .find(|s| s.owner == SubnetOwner::Building(b))
        .unwrap()
        .security = 120;
    {
        let p = st.player.as_mut().unwrap();
        p.body = Some((Place::Stratum(StratumKind::Street), p.pos));
        p.place = Place::Stratum(StratumKind::Net);
        p.pos = crate::constructs::city_gate(&city, b).unwrap();
    }
    crate::constructs::enter_construct(&mut st, b).unwrap();
    map.place = Place::Construct(b);
    let k = st.constructs.get(b).unwrap().clone();
    let (lock, lock_pos) = k
        .nodes
        .iter()
        .find(|(id, _)| city.net.node(*id).kind == NodeKind::DoorLock)
        .copied()
        .unwrap();
    let here = lock_pos
        .neighbors4()
        .find(|n| k.tiles.get(*n).is_some_and(|t| t.is_walkable()))
        .unwrap();
    st.player.as_mut().unwrap().pos = here;
    map.cam.snap(here);
    map.update(&mut st, &[key(KeyCode::Char('.'))]);
    for y in 0..k.size.h as i32 {
        for x in 0..k.size.w as i32 {
            let at = Pos::new(x, y);
            let p = st.player.as_mut().unwrap();
            if p.vis().get(at) == Some(&crate::render::Visibility::Unseen) {
                p.set_vis(at, crate::render::Visibility::Remembered);
            }
        }
    }
    st.net.force_roll = Some(10);
    crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock)).unwrap();
    map.update(&mut st, &[key(KeyCode::Char('.'))]);
    pass(&mut st, crate::net::BEAT_TICKS / 2);
    let shot = |st: &AppState, map: &MapScreen, name: &str, size| {
        write_svg(&dir, name, st, &draw_stack(st, &[map], size), false);
    };
    shot(&st, &map, "run_tether", (120, 36));
    shot(&st, &map, "run_tether_small", (100, 30));
    map.update(&mut st, &[key(KeyCode::Char(' '))]);
    shot(&st, &map, "run_tether_hit", (120, 36));
    map.update(&mut st, &[key(KeyCode::Char(' '))]);
    // Past the landing's kick.
    pass(&mut st, 8);
    shot(&st, &map, "run_tether_landed", (120, 36));
}

/// The walk from the home megablock's street door to Marlo's body, tile by tile.
fn home_to_body(st: &AppState) -> Vec<Pos> {
    let city = st.city.as_ref().unwrap();
    let body = st.missions.bodies[0].pos;
    let home = st.home.unwrap().home.building;
    let door = city
        .portals(home)
        .into_iter()
        .find(|(k, _)| *k == StratumKind::Street)
        .map(|(_, p)| p)
        .unwrap();
    let start = door
        .neighbors4()
        .find(|&n| city.walkable(StratumKind::Street, n))
        .unwrap();
    citygen::pathing::astar(city.size, start, body, |_, s| {
        city.walkable(StratumKind::Street, s).then_some(1)
    })
    .unwrap()
}

/// Puts the player on `at` of the street and sees from there.
fn stand_on(st: &mut AppState, at: Pos) {
    let street = Place::Stratum(StratumKind::Street);
    let bonus = st.sight_bonus();
    let mut p = st.player.take().unwrap();
    p.place = street;
    p.pos = at;
    {
        let city = st.city.as_ref().unwrap();
        let map = crate::render::map::MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            street,
            st.minute_of_day(),
        )
        .unwrap();
        p.update_fov(&map, bonus);
    }
    st.player = Some(p);
}

/// The street by the body, with the walk from home remembered (seen every other step)
/// and the player back outside the home megablock's door.
pub(crate) fn walked_city() -> (AppState, Vec<Pos>) {
    let (mut st, _) = street_by_the_body();
    let route = home_to_body(&st);
    for at in route.iter().step_by(2) {
        stand_on(&mut st, *at);
    }
    stand_on(&mut st, route[0]);
    (st, route)
}

/// The track of the story's open mission, if one waits.
fn story_track(st: &AppState) -> Option<crate::track::Source> {
    (0..st.book.missions.len()).find_map(|m| crate::track::ui::story_source(st, m))
}

/// `walked_city` with the clinic named Miracorp Tattoo tracked.
pub(crate) fn tracked_city() -> (AppState, MapScreen) {
    let (mut st, _) = walked_city();
    let l = crate::track::listings::listings(&st, StratumKind::Street)
        .into_iter()
        .find(|l| l.name == "Miracorp Tattoo")
        .expect("a clinic called Miracorp Tattoo");
    st.atlas.cursor = Some(l.tile);
    st.atlas.picked = Some((l.id, l.kind, l.tile));
    crate::track::set(&mut st, crate::track::Source::Listing(l.id)).unwrap();
    let map = MapScreen::walking(&mut st);
    (st, map)
}

/// `tracked_city`, back in your flat on 18F where the game starts.
pub(crate) fn tracked_from_the_flat() -> (AppState, MapScreen) {
    let (mut st, _) = tracked_city();
    let rig = st.home.unwrap();
    let p = st.player.as_mut().unwrap();
    p.place = rig.place();
    p.pos = rig.home.bed_side;
    st.atlas.cursor = None;
    st.atlas.picked = None;
    crate::track::tick(&mut st);
    let map = MapScreen::walking(&mut st);
    (st, map)
}

const BOTH: [(u16, u16); 2] = [(120, 40), (100, 30)];

/// `n` frames of the track, as the deck (or the street) runs them, so its pulses are
/// in flight when the shot is taken.
fn fly(st: &mut AppState, mut deck: Option<&mut Deck>, n: u64) {
    for _ in 0..n {
        st.tick += 1;
        match deck.as_deref_mut() {
            Some(d) => {
                d.update(st, &[]);
            }
            None => crate::track::tick(st),
        }
    }
}

/// The deck on MAP, drawn once so its keys have a map to work on; `keys` then press,
/// and the deck is written at `size` as `name`.
fn atlas_shot(name: &str, keys: &[KeyCode], ground: bool) {
    atlas_shot_from(name, keys, ground, tracked_city);
}

/// `atlas_shot` from the state `start` makes.
fn atlas_shot_from(
    name: &str,
    keys: &[KeyCode],
    ground: bool,
    start: fn() -> (AppState, MapScreen),
) {
    for size in BOTH {
        let (mut st, map) = start();
        let plaza = ground.then(|| {
            let city = st.city.as_ref().unwrap();
            let tiles = &city.stratum(StratumKind::Street).unwrap().tiles;
            let size = tiles.size();
            (0..size.h as i32)
                .flat_map(|y| (0..size.w as i32).map(move |x| Pos::new(x, y)))
                .find(|p| {
                    tiles.get(*p).is_some_and(|t| {
                        t.terrain == citygen::Terrain::Plaza
                            && t.district
                                .and_then(|d| city.districts.get(d.0 as usize))
                                .is_some_and(|d| d.name.starts_with("Crown Circle"))
                    })
                })
                .expect("a Crown Circle plaza")
        });
        let mut deck = Deck::new(st.tick);
        deck.update(&mut st, &[key(KeyCode::Char('7'))]);
        st.tick += 200;
        let _ = draw_stack(&st, &[&map, &deck], size);
        for k in keys {
            deck.update(&mut st, &[key(*k)]);
        }
        if let Some(p) = plaza {
            st.atlas.cursor = Some(p);
            st.atlas.picked = None;
        }
        fly(&mut st, Some(&mut deck), 40);
        let (w, h) = size;
        shot_stack(&format!("{name}_{w}x{h}"), &st, &[&map, &deck], size);
    }
}

/// The MAP opened in your flat on 18F, Miracorp Tattoo tracked: the walk from the
/// megablock's door, the distances from there.
#[test]
#[ignore]
fn atlas_from_the_flat() {
    atlas_shot_from("atlas_from_the_flat", &[], false, tracked_from_the_flat);
}

/// Frame 1: every kind, the cursor on the tracked clinic.
#[test]
#[ignore]
fn atlas_listing() {
    atlas_shot("atlas_listing", &[], false);
}

/// Frame 2: ripperdocs only, the cursor on a Crown Circle plaza.
#[test]
#[ignore]
fn atlas_ground() {
    let at = crate::track::listings::Kind::ALL
        .iter()
        .position(|k| *k == crate::track::listings::Kind::Docs)
        .unwrap();
    let keys = vec![KeyCode::Char('>'); at + 1];
    atlas_shot("atlas_ground", &keys, true);
}

/// Frame 3: the places pane folded to its strip and the card.
#[test]
#[ignore]
fn atlas_folded() {
    atlas_shot("atlas_folded", &[KeyCode::Char('f')], false);
}

/// The minimap on the street, Miracorp Tattoo tracked: the close up, the walk, the
/// clinic's `+`, its distance and bearing.
#[test]
#[ignore]
fn minimap_street() {
    for (w, h) in BOTH {
        let (mut st, map) = tracked_city();
        fly(&mut st, None, 14);
        shot_stack(&format!("minimap_street_{w}x{h}"), &st, &[&map], (w, h));
    }
}

/// The minimap in your flat on 18F, the same track: the floor plan, the walk to the
/// lift, the shaft lit down to L.
#[test]
#[ignore]
fn minimap_flat() {
    for (w, h) in BOTH {
        let (mut st, map) = tracked_from_the_flat();
        fly(&mut st, None, 14);
        shot_stack(&format!("minimap_flat_{w}x{h}"), &st, &[&map], (w, h));
    }
}

/// The minimap inside a building of one floor: the plan alone, no shaft.
#[test]
#[ignore]
fn minimap_one_floor() {
    for (w, h) in BOTH {
        let (mut st, _) = tracked_city();
        let city = st.city.clone().unwrap();
        let b = city
            .buildings
            .iter()
            .filter(|b| b.kind.is_closed())
            .find(|b| {
                st.interiors.ensure(&city, b.id, 0).is_some()
                    && st
                        .interiors
                        .plan_of(b.id)
                        .is_some_and(|p| p.lowest == p.top)
            })
            .expect("a one-floor building")
            .id;
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        let door = floor.exits.first().map(|e| floor.doorstep(e)).unwrap();
        let p = st.player.as_mut().unwrap();
        p.place = Place::Floor {
            building: b,
            level: 0,
        };
        p.pos = door;
        crate::track::tick(&mut st);
        let map = MapScreen::walking(&mut st);
        fly(&mut st, None, 14);
        shot_stack(&format!("minimap_one_floor_{w}x{h}"), &st, &[&map], (w, h));
    }
}

/// The street with the walk to Marlo's body, `back` tiles of it still ahead, `flying`
/// ticks after it was tracked.
fn street_walk(name: &str, back: usize, flying: u64) {
    let (mut st, route) = walked_city();
    stand_on(&mut st, route[route.len().saturating_sub(back + 1)]);
    let source = story_track(&st).unwrap_or(crate::track::Source::Spot {
        place: Place::Stratum(StratumKind::Street),
        pos: st.missions.bodies[0].pos,
        radius: 1,
    });
    crate::track::set(&mut st, source).unwrap();
    let map = MapScreen::walking(&mut st);
    fly(&mut st, None, flying);
    for (w, h) in BOTH {
        shot_stack(&format!("{name}_{w}x{h}"), &st, &[&map], (w, h));
    }
}

#[test]
#[ignore]
fn track_street_near() {
    street_walk("track_street_near", 30, 14);
}

#[test]
#[ignore]
fn track_street_far() {
    street_walk("track_street_far", 95, 14);
}

/// The shipped story at your door after the drop, the house call queued, and the map
/// that will draw it.
fn at_the_door() -> (AppState, MapScreen) {
    let mut st = crate::missions::tests::at_your_door();
    let map = MapScreen::walking(&mut st);
    (st, map)
}

/// One tick on, the story's own.
fn story_tick(st: &mut AppState) {
    if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
        world.advance(city, world.tick() + 1);
    }
    crate::missions::tick(st);
}

/// Ticks the story until the running scene stands on `beat` with its line fully typed
/// (or, on a keypad beat, its verdict showing).
fn scene_at(st: &mut AppState, beat: usize) {
    use crate::missions::scene;
    for _ in 0..20_000 {
        if let Some(run) = scene::running(st) {
            let line_done = run.line().is_none_or(|l| !l.typing(st.clock_tick()));
            let pad_done = run.pad().is_none() || run.verdict_shown();
            if run.beat() == beat && line_done && pad_done {
                return;
            }
        }
        story_tick(st);
    }
    panic!("the scene never reached beat {beat}");
}

/// Ticks on until the scene `id` is the one running.
fn scene_is(st: &mut AppState, id: &str) {
    use crate::missions::scene;
    for _ in 0..20_000 {
        if scene::running(st).map(|r| r.id()) == Some(id) {
            return;
        }
        story_tick(st);
    }
    panic!("{id} never played");
}

/// The house call's film at the beats that matter: the pad's DENIED, the voice at the
/// open door, Castellan on the couch, Seren out of the bedroom; then the couch small.
#[test]
#[ignore]
fn scene_film_shots() {
    for (name, beat, size) in [
        ("scene_film_keypad", 5, (120, 40)),
        ("scene_film_door", 10, (120, 40)),
        ("scene_film_couch", 15, (120, 40)),
        ("scene_film_seren", 21, (120, 40)),
        ("scene_film_couch_100x30", 15, (100, 30)),
    ] {
        let (mut st, map) = at_the_door();
        scene_at(&mut st, beat);
        shot_stack(name, &st, &[&map], size);
    }
}

/// Seren's aside as bubbles over the dimmed flat, at both sizes.
#[test]
#[ignore]
fn scene_bubbles_shots() {
    for (name, size) in [
        ("scene_bubbles_seren", (120, 40)),
        ("scene_bubbles_seren_100x30", (100, 30)),
    ] {
        let (mut st, map) = at_the_door();
        scene_is(&mut st, "seren_aside");
        scene_at(&mut st, 2);
        shot_stack(name, &st, &[&map], size);
    }
}

/// Interiors under their own light, map only, as the indoor lighting mock drew them
/// (`docs/superpowers/specs/2026-11-02-indoor-lighting-mock/`): the start flat, a
/// ripperdoc's at night, the first bar and club at street level, an office and a hotel
/// floor, with the player on the floor's middle walkable tile.
#[test]
#[ignore]
fn indoor_lighting() {
    use citygen::BuildingKind as K;
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let shoot = |name: &str, st: &mut AppState| {
        let map = MapScreen::walking(st);
        write_svg(&dir, name, st, &draw_stack(st, &[&map], (120, 40)), false);
    };
    shoot("indoor_flat", &mut state_home());
    let (mut st, ..) = crate::clinic::testutil::inside();
    shoot("indoor_clinic", &mut st);
    for (kind, level, name) in [
        (K::Bar, 0, "indoor_bar"),
        (K::Club, 0, "indoor_club"),
        (K::Office, 2, "indoor_office"),
        (K::Hotel, 2, "indoor_hotel"),
    ] {
        let mut st = state_home();
        let city = st.city.clone().unwrap();
        let found = city
            .buildings
            .iter()
            .filter(|b| b.kind == kind)
            .find_map(|b| {
                let floor = st.interiors.ensure(&city, b.id, level)?.clone();
                let size = floor.tiles.size();
                let mid = Pos::new(size.w as i32 / 2, size.h as i32 / 2);
                let stand = floor
                    .tiles
                    .iter()
                    .filter(|(_, t)| t.is_walkable())
                    .min_by_key(|(p, _)| p.dist2(mid))
                    .map(|(p, _)| p)?;
                Some((b.id, stand))
            });
        let Some((building, stand)) = found else {
            continue;
        };
        let place = Place::Floor { building, level };
        st.player = Some(crate::walker::Player::at(place, stand));
        shoot(name, &mut st);
    }
}

/// The flat after the house call, both scenes skipped, beside the bed at 22:01, the map
/// run a few frames at `size` as play does: the camera clamped to the floor, which puts
/// the flat (on the floor's bottom edge) low on the map.
pub(crate) fn beside_the_bed(size: (u16, u16)) -> (AppState, MapScreen) {
    use crate::missions::scene::tests::press;
    let mut st = crate::missions::tests::at_your_door();
    for _ in 0..2 {
        crate::missions::tick(&mut st);
        press(&mut st, KeyCode::Esc);
    }
    assert!(crate::missions::scene::running(&st).is_none());
    // The HOLO feed the story queued is read; it would wake you.
    st.missions.feeds.clear();
    let (place, pos) =
        crate::missions::marks::resolve(&st, &crate::missions::Mark::HomeBed, None).unwrap();
    st.player = Some(crate::walker::Player::at(place, pos));
    let ahead = (22 * 60 + 1 + 1440 - st.minute_of_day() as u64) % 1440;
    let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
    world.advance(city, world.tick() + ahead * sim::TICKS_PER_MINUTE);
    let mut map = MapScreen::walking(&mut st);
    map.settle(&mut st, size);
    (st, map)
}

/// The Rest window over the map beside the bed, and the same night asleep at 03:12, at
/// both sizes.
#[test]
#[ignore]
fn rest_shots() {
    use crate::rest::{self, Kind};
    use crate::screens::rest::RestScreen;
    for (suffix, size) in [("", (120, 40)), ("_100x30", (100, 30))] {
        let (mut st, map) = beside_the_bed(size);
        let choose = RestScreen::open(&st, Kind::Sleep);
        shot_stack(&format!("rest_choose{suffix}"), &st, &[&map, &choose], size);

        let until = rest::default_wake(&st, Kind::Sleep);
        rest::start(&mut st, Kind::Sleep, until);
        assert!(
            st.rest.is_some(),
            "the rest began: {:?}",
            st.log.iter().last().map(|e| &e.text)
        );
        // A minute a frame, so the clock stops on 03:12.
        while st.rest.is_some() && st.minute_of_day() != 3 * 60 + 12 {
            rest::frame_within(&mut st, std::time::Duration::ZERO);
        }
        assert_eq!(st.clock(), (3, 12));
        shot_stack(&format!("rest_asleep{suffix}"), &st, &[&map], size);
    }
}

/// The JOBS tab (spec 2026-11-02), as the mock drew it: the drop open, the mock's six
/// jobs taken at 22:00, the clock on 01:20, the fetch tracked; a job in hand, one ready,
/// an offer, a fixer, a closed job, the story, and space on the fixer, at both sizes.
#[test]
#[ignore]
fn jobs_tab() {
    use crate::screens::jobs_tab::Entry;
    let (mut st, route) = walked_city();
    stand_on(&mut st, route[route.len().saturating_sub(31)]);
    let ids = crate::jobs::testutil::six_jobs(&mut st);
    let p = st.player.as_ref().unwrap();
    let at = game::At {
        place: p.place,
        pos: p.pos,
    };
    crate::screens::console::run(&mut st, at, "clock 01:20").unwrap();
    let src = crate::track::ui::job_source(&st, ids[0]).unwrap();
    crate::track::set(&mut st, src).unwrap();
    let map = MapScreen::walking(&mut st);
    let fixer = st.jobs.get(ids[0]).unwrap().fixer;
    st.deck_jobs_open.insert(st.jobs.get(ids[3]).unwrap().fixer);
    let deck_on = |st: &mut AppState, want: &Entry| {
        let mut deck = Deck::new(st.tick);
        deck.update(st, &[key(KeyCode::Char('5'))]);
        deck.jobs.select(st, want);
        // Off the deck's signal jitter, the card typed out.
        st.tick += 200;
        while st.tick % 90 < 3 {
            st.tick += 1;
        }
        deck
    };
    for (name, want) in [
        ("t", Entry::Job(ids[0])),
        ("t_ready", Entry::Job(ids[1])),
        ("offer", Entry::Job(ids[2])),
        ("fixer", Entry::Fixer(fixer)),
        ("closed", Entry::Job(ids[3])),
        ("story", Entry::Mission(0)),
    ] {
        let deck = deck_on(&mut st, &want);
        for (w, h) in BOTH {
            shot_stack(
                &format!("jobs_tab_{name}_{w}x{h}"),
                &st,
                &[&map, &deck],
                (w, h),
            );
        }
    }
    for (w, h) in BOTH {
        let mut deck = deck_on(&mut st, &Entry::Fixer(fixer));
        // A draw at this size first, so the popup anchors on the row as drawn.
        shot_stack(
            &format!("jobs_tab_verbs_{w}x{h}"),
            &st,
            &[&map, &deck],
            (w, h),
        );
        let crate::app::ScreenCmd::Push { to, .. } =
            deck.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the fixer's verbs")
        };
        st.tick += 40;
        while st.tick % 90 < 3 {
            st.tick += 1;
        }
        shot_stack(
            &format!("jobs_tab_verbs_{w}x{h}"),
            &st,
            &[&map, &deck, to.as_ref()],
            (w, h),
        );
    }
}

/// The morning after, Mezz's mail read and Seren's word come: in the flat, the package
/// unsealed, the doctor's mission open.
pub(crate) fn seren_has_written() -> AppState {
    let mut st = crate::missions::tests::the_morning_call();
    crate::missions::tests::to_the_word(&mut st);
    st
}

/// Walks the player to a free tile by the news set, the camera there.
fn by_the_set(st: &mut AppState) -> MapScreen {
    let (place, set) = crate::missions::news::set_tile(st).expect("the set");
    let mut me = crate::walker::Player::at(place, set.offset(-2, 1));
    {
        let city = st.city.as_ref().unwrap();
        let flat = crate::render::map::MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            place,
            st.minute_of_day(),
        )
        .unwrap();
        me.update_fov(&flat, st.sight_bonus());
    }
    st.player = Some(me);
    MapScreen::walking(st)
}

/// Runs the world on `seconds` of game time.
fn world_seconds(st: &mut AppState, seconds: u64) {
    let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
    world.advance(
        city,
        world.tick() + seconds * crate::missions::call::TICKS_PER_SECOND,
    );
}

/// In at the doc's street door on the mailed code and up beside her, her scene (the
/// doctor's `in` step) starting.
fn at_the_doctor(st: &mut AppState) {
    use crate::missions::tests as t;
    let doc = crate::missions::person_of(st, "odile").expect("the doc is cast");
    let b = crate::missions::cast::work_building(st, doc).expect("she works in her clinic");
    let code = crate::credentials::street_code(st, b);
    let out = t::outside_door(st, b);
    t::on_street(st, out);
    let (_, door) = st
        .city
        .as_ref()
        .unwrap()
        .portals(b)
        .into_iter()
        .find(|(k, _)| *k == StratumKind::Street)
        .unwrap();
    let keyed = crate::interiors::enter(st, b, (StratumKind::Street, door));
    assert_eq!(keyed, Ok(format!("you key in {code:04}")));
    let (place, at) = crate::interiors::drawn_at(st, doc).expect("she is drawn inside");
    let by = t::beside_on(st, place, at);
    t::stand_seeing(st, place, by);
    crate::missions::tick(st);
}

/// Doctor's in by ten, drawn through the real screens: Seren on the set, the shard's
/// cards at the rig and the bench, the doc and her scene, her tray, the tab in the chair.
#[test]
#[ignore]
fn doctor_shots() {
    use crate::missions::ReadAt;
    use crate::missions::reading::ReadCard;
    use crate::screens::sheet::LedgerRow;
    let shard = ItemId::new("ledger_shard");
    for (w, h) in [(120u16, 40u16), (100, 30)] {
        let size = (w, h);
        let name = |what: &str| format!("doctor_{what}_{w}x{h}");

        // Seren's line through the set.
        let mut st = seren_has_written();
        st.missions.daze = None;
        let ahead = (7 * 60 + 52 + 1440 - u64::from(st.minute_of_day())) % 1440;
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(city, world.tick() + ahead * sim::TICKS_PER_MINUTE);
        // Her line went up at 06:04 and is long gone: put it up again at 07:52.
        let line = st.missions.asides.pop().expect("Seren on the set");
        st.missions.asides.clear();
        crate::missions::asides::say(&mut st, line.speaker, line.label, line.colour, line.text);
        let map = by_the_set(&mut st);
        world_seconds(&mut st, 3);
        shot_stack(&name("seren_set"), &st, &[&map], size);

        // The shard read at the rig by a hacker, over the flat.
        let mut st = seren_has_written();
        st.character.skills.insert(game::SkillId::new("hacking"));
        let map = by_the_set(&mut st);
        let card = ReadCard::open(&mut st, &shard, ReadAt::Rig).expect("the rig reads it");
        shot_stack(&name("read_rig"), &st, &[&map, &card], size);

        // The casing opened at the bench by a wrench, over the deck's pack.
        let mut st = seren_has_written();
        st.character = game::Character::debug_runner(&st.content);
        st.character
            .skills
            .insert(game::SkillId::new("electronics"));
        st.character.give_item(&st.content, shard.clone(), 1);
        let map = by_the_set(&mut st);
        st.tick += 200;
        let mut deck = Deck::new(st.tick);
        deck.update(&mut st, &[key(KeyCode::Char('4'))]);
        let card = ReadCard::open(&mut st, &shard, ReadAt::Bench).expect("the bench opens it");
        shot_stack(&name("read_bench"), &st, &[&map, &deck, &card], size);

        // The held clinic at ten, the first line up beside her.
        let mut st = seren_has_written();
        let (hh, mm) = st.clock();
        let now = u64::from(hh) * 60 + u64::from(mm);
        let ahead = (10 * 60 + 1440 - now) % 1440;
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(city, world.tick() + ahead * sim::TICKS_PER_MINUTE);
        at_the_doctor(&mut st);
        scene_is(&mut st, "odile");
        scene_at(&mut st, 1);
        st.missions.daze = None;
        let mut map = MapScreen::walking(&mut st);
        map.settle(&mut st, size);
        // Her bubble is anchored: she is drawn where the map places her, and the tail
        // sits in the cell just above or below hers.
        let doc = crate::missions::person_of(&st, "odile").unwrap();
        let her = crate::interiors::drawn_at(&st, doc).expect("she is drawn");
        let anchor = crate::missions::scene::running(&st)
            .and_then(|s| s.line().and_then(|l| l.anchor))
            .expect("her line has an anchor");
        assert_eq!(anchor, her, "the line is anchored on her drawn tile");
        let buf = draw_stack(&st, &[&map], size);
        assert_eq!(
            map.placed_tile(doc),
            Some(her.1),
            "the map places her there"
        );
        let rect = map.last_map_rect();
        let origin = map.cam.origin(rect.width, rect.height);
        let (sx, sy) = crate::render::viewport::tile_to_screen(rect, origin, her.1)
            .expect("her cell is on screen");
        let tail = [(sy.wrapping_sub(1), '▾'), (sy + 1, '▴')]
            .into_iter()
            .find(|&(y, glyph)| buf[(sx, y)].symbol() == glyph.to_string());
        assert!(tail.is_some(), "no tail beside her cell ({sx}, {sy})");
        shot_stack(&name("odile"), &st, &[&map], size);

        // Her tray, once the scene is out.
        crate::missions::scene::skip(&mut st);
        let tray = Popup::tray(&st).expect("her tray is held out");
        st.tick += 200;
        let map = MapScreen::walking(&mut st);
        shot_stack(&name("tray"), &st, &[&map, &tray], size);

        // The chair on the tab: a piece from the tray, the fee his.
        crate::missions::take_from_tray(&mut st, 0);
        let tab = st.missions.tab.clone().expect("the tab");
        let site = crate::missions::tests::site_for(&st, &tab.item);
        let mut st = crate::clinic::testutil::seated();
        st.character.give_item(&st.content, tab.item.clone(), 1);
        st.missions.tab = Some(tab);
        let map = MapScreen::walking(&mut st);
        let mut deck = Deck::chair(&st);
        deck.sheet.state.row = LedgerRow::Site(site);
        deck.update(&mut st, &[key(KeyCode::Char(' '))]);
        deck.sheet.changed = st.tick;
        deck.sheet.card = st.tick;
        st.tick += 200;
        if st.tick % 90 < 3 {
            st.tick += 3;
        }
        shot_stack(&name("chair_tab"), &st, &[&map, &deck], size);
    }
}

/// Out of the rig on the open Net (seed 8): on the lane under Kiroflux Mart's gate,
/// tracking it. Spec `2026-11-03-net-look-design.md`.
pub(crate) fn on_the_open_net() -> (AppState, MapScreen) {
    let mut st = state_home();
    let _ = MapScreen::walking(&mut st);
    let city = st.city.clone().unwrap();
    let kiroflux = city
        .buildings
        .iter()
        .find(|b| b.name.as_deref() == Some("Kiroflux Mart"))
        .map(|b| b.id)
        .expect("Kiroflux Mart");
    // Tracked from the street: the Net refuses a new track.
    crate::track::set(
        &mut st,
        crate::track::Source::Listing(crate::track::ListingId::Building(kiroflux)),
    )
    .unwrap();
    {
        let p = st.player.as_mut().unwrap();
        p.body = Some((p.place, p.pos));
        p.place = Place::Stratum(StratumKind::Net);
        p.pos = Pos::new(201, 62);
    }
    let bonus = st.sight_bonus();
    let mut p = st.player.take().unwrap();
    p.update_fov(
        &crate::render::map::MapRef::stratum_of(&city, StratumKind::Net),
        bonus,
    );
    st.player = Some(p);
    let mut map = MapScreen::walking(&mut st);
    map.update(&mut st, &[]);
    (st, map)
}

#[test]
#[ignore]
fn net_open() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    for (name, size) in [("net_open", (120, 40)), ("net_open_small", (100, 30))] {
        write_svg(&dir, name, &st, &draw_stack(&st, &[&map], size), false);
    }
    // The traffic a few tiles on: packets hop node to node along the lanes.
    pass(&mut st, crate::render::netlook::packets::PACKET_TICKS * 5);
    map.update(&mut st, &[]);
    write_svg(
        &dir,
        "net_open_later",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
}

/// Inside the first black construct with a vault (seed 8), standing in its first room
/// beside the door.
#[test]
#[ignore]
fn net_inside() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let mut st = state_home();
    let mut map = MapScreen::walking(&mut st);
    let city = st.city.clone().unwrap();
    let (k, b) = city
        .buildings
        .iter()
        .filter_map(|b| citygen::construct::generate_construct(&city, b.id).map(|k| (k, b.id)))
        .find(|(k, _)| k.vault_room().is_some() && k.security >= 230 && k.guards.len() == 2)
        .expect("a black construct with a vault");
    {
        let p = st.player.as_mut().unwrap();
        p.body = Some((p.place, p.pos));
        p.place = Place::Stratum(StratumKind::Net);
        p.pos = crate::constructs::city_gate(&city, b).unwrap();
    }
    crate::constructs::enter_construct(&mut st, b).unwrap();
    map.place = Place::Construct(b);
    // Inside the first room, on the tile in from its door.
    let room = k.rooms[0];
    let door = room
        .grow(1)
        .border()
        .find(|p| k.tiles[*p].terrain == citygen::Terrain::NetGate)
        .expect("the first room's door");
    let inside = door
        .neighbors4()
        .find(|p| room.contains(*p))
        .expect("the tile in from the door");
    let here = if k.tiles[inside].feature.is_none() {
        inside
    } else {
        room.iter().find(|p| k.tiles[*p].feature.is_none()).unwrap()
    };
    stand_in(&mut st, Place::Construct(b), here);
    map.cam.snap(here);
    map.update(&mut st, &[]);
    for (name, size) in [("net_inside", (120, 40)), ("net_inside_small", (100, 30))] {
        write_svg(&dir, name, &st, &draw_stack(&st, &[&map], size), false);
    }
}

/// Moves the player within a place and refreshes their sight.
fn stand_in(st: &mut AppState, place: Place, at: Pos) {
    let bonus = st.sight_bonus();
    let minute = st.minute_of_day();
    let mut p = st.player.take().unwrap();
    p.place = place;
    p.pos = at;
    if let Some(city) = st.city.as_ref()
        && let Some(map) = crate::render::map::MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            place,
            minute,
        )
    {
        p.update_fov(&map, bonus);
    }
    st.player = Some(p);
}

/// Half way along a ride north up the lane by Kiroflux Mart.
#[test]
#[ignore]
fn net_ride() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    // Riding is the Data surfer's.
    st.character
        .talents
        .insert(game::TalentId::new("data_surfer"), 1);
    stand_in(&mut st, Place::Stratum(StratumKind::Net), Pos::new(199, 61));
    map.cam.snap(Pos::new(199, 61));
    map.update(&mut st, &[key(KeyCode::Char('k'))]);
    for _ in 0..3 {
        pass(&mut st, crate::ride::RIDE_TICKS);
        map.update(&mut st, &[]);
    }
    for (name, size) in [("net_ride", (120, 40)), ("net_ride_small", (100, 30))] {
        write_svg(&dir, name, &st, &draw_stack(&st, &[&map], size), false);
    }
}

/// A rig run that landed at a backdoor on Stack 101 Terminal and walked to Kiroflux
/// Mart: the cord back to the relay and the hop home, then a trace on the relay.
#[test]
#[ignore]
fn net_cord() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    let city = st.city.clone().unwrap();
    let relay = city
        .buildings
        .iter()
        .find(|b| b.name.as_deref() == Some("Stack 101 Terminal"))
        .map(|b| b.id)
        .expect("Stack 101 Terminal");
    let home = st.home.unwrap();
    st.player.as_mut().unwrap().body = Some((home.place(), home.home.rig));
    let now = st.net.now(&st);
    st.backdoors.plant_net(relay, 50, city.seed, now);
    st.backdoors.start_route(Some(relay));
    map.update(&mut st, &[]);
    for (name, size) in [("net_cord", (120, 40)), ("net_cord_small", (100, 30))] {
        write_svg(&dir, name, &st, &draw_stack(&st, &[&map], size), false);
    }
    st.backdoors.start_trace(now);
    map.update(&mut st, &[]);
    write_svg(
        &dir,
        "net_cord_traced",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
}

/// Every row of `buf` as one string, to look for the words a shot is for.
fn buffer_text(buf: &Buffer) -> String {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Clear objectives (spec 2026-11-03), through the real chain: the JOBS card of the
/// morning after waiting on its package and of the ledger for a hacker, then the map as
/// Mezz's mail lands (the pill, the log line typed out) and 180 map frames on (the mark).
#[test]
#[ignore]
fn objectives_shots() {
    use crate::screens::jobs_tab::Entry;
    let mission = |st: &AppState, id: &str| st.book.missions.iter().position(|m| m.id == id);
    let jobs_on = |st: &mut AppState, m: usize| {
        let mut deck = Deck::new(st.tick);
        deck.update(st, &[key(KeyCode::Char('5'))]);
        deck.jobs.select(st, &Entry::Mission(m));
        // Off the deck's signal jitter, the card typed out.
        st.tick += 200;
        while st.tick % 90 < 3 {
            st.tick += 1;
        }
        deck
    };
    for (w, h) in [(120u16, 40u16), (100, 30)] {
        let size = (w, h);
        let name = |what: &str| format!("objectives_{what}_{w}x{h}");

        // The morning after, Seren's word in: the package in the pack, unopened.
        let mut st = seren_has_written();
        st.missions.daze = None;
        let map = MapScreen::walking(&mut st);
        let m = mission(&st, "morning_after").expect("the morning after");
        let deck = jobs_on(&mut st, m);
        let buf = draw_stack(&st, &[&map, &deck], size);
        assert!(
            buffer_text(&buf).contains("Open the package"),
            "the card says what is next"
        );
        shot_stack(&name("jobs_morning"), &st, &[&map, &deck], size);

        // A hacker with the package opened: the ledger's whose-shard objective.
        let mut st = crate::missions::tests::the_morning_call();
        st.character.skills.insert(game::SkillId::new("hacking"));
        crate::missions::tests::to_the_word(&mut st);
        crate::missions::tests::open_the_package(&mut st);
        st.missions.daze = None;
        let map = MapScreen::walking(&mut st);
        let m = mission(&st, "ledger").expect("the ledger");
        let deck = jobs_on(&mut st, m);
        let buf = draw_stack(&st, &[&map, &deck], size);
        assert!(
            buffer_text(&buf).contains("read it at your rig"),
            "a hacker reads it at the rig"
        );
        shot_stack(&name("jobs_ledger"), &st, &[&map, &deck], size);

        // Mezz's mail lands on a map frame: the pill up, its log line typed out.
        let mut st = crate::missions::tests::the_morning_call();
        crate::missions::tests::the_news_is_on(&mut st);
        st.missions.daze = None;
        // The inbox read up to here, so the mark counts Mezz's mail alone.
        st.journal.mails.iter_mut().for_each(|m| m.read = true);
        st.mail_seen = st.journal.mails.len();
        let mut map = MapScreen::walking(&mut st);
        map.update(&mut st, &[]);
        crate::missions::news::switch_off(&mut st);
        crate::missions::tick(&mut st);
        assert!(st.journal.mails.len() > st.mail_seen, "Mezz's mail landed");
        st.tick += 1;
        map.update(&mut st, &[]);
        // The newest line types out against the frame clock.
        st.tick += 30;
        let text = buffer_text(&draw_stack(&st, &[&map], size));
        assert!(text.contains("▣ MAIL"), "the pill is up");
        assert!(
            text.contains("mail · the news"),
            "the log line is typed out"
        );
        assert!(
            !text.contains("NEW MAIL"),
            "the pill holds the status line back"
        );
        shot_stack(&name("mail_pill"), &st, &[&map], size);

        // 180 map frames on: the pill is down, the mark on the status line.
        for _ in 0..crate::mail_mark::MAIL_PILL_TICKS {
            st.tick += 1;
            map.update(&mut st, &[]);
        }
        let text = buffer_text(&draw_stack(&st, &[&map], size));
        assert!(!text.contains("▣ MAIL"), "the pill is down");
        assert!(
            text.contains("▣ NEW MAIL"),
            "the mark is on the status line\n{text}"
        );
        shot_stack(&name("mail_mark"), &st, &[&map], size);
    }
}

/// HELIX mail at the rig, through the real story: Mezz's mail just in (the older mail
/// read), the rig's boot saying so, the inbox, the mail read with Seren's landing, and
/// `mail 4` printed.
#[test]
#[ignore]
fn mail_shots() {
    for (w, h) in [(120u16, 40u16), (100, 30)] {
        let size = (w, h);
        let name = |what: &str| format!("mail_{what}_{w}x{h}");

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
        // The map's mail pill stays off the shot.
        st.missions.daze = None;
        st.missions.asides.clear();
        st.mail_seen = st.journal.mails.len();
        st.mail_pill = None;
        let map = MapScreen::walking(&mut st);
        let home = st.home.unwrap().home;
        let mut term = Terminal::open(&mut st, HostId::Rig, (map.place, home.rig));
        st.tick += 10_000;
        term.update(&mut st, &[]);

        let text = buffer_text(&draw_stack(&st, &[&map, &term], size));
        assert!(
            text.contains("YOU HAVE 1 NEW MAIL"),
            "the boot says what is new\n{text}"
        );
        shot_stack(&name("boot"), &st, &[&map, &term], size);

        type_lines(&mut st, &mut term, &["mail"]);
        st.tick += 10_000;
        term.update(&mut st, &[]);
        let text = buffer_text(&draw_stack(&st, &[&map, &term], size));
        assert!(text.contains("HELIX mail 2.2"), "the program is up\n{text}");
        assert!(text.contains("INBOX"), "on the inbox\n{text}");
        assert!(text.contains("the news"), "Mezz's mail is listed\n{text}");
        assert!(text.contains("NEW"), "marked new\n{text}");
        shot_stack(&name("inbox"), &st, &[&map, &term], size);

        term.update(&mut st, &[key(KeyCode::Char(' '))]);
        st.tick += 10_000;
        term.update(&mut st, &[]);
        let text = buffer_text(&draw_stack(&st, &[&map, &term], size));
        assert!(
            text.contains("Saw the news."),
            "Mezz's mail is read\n{text}"
        );
        assert!(text.contains("[ 1 new left ]"), "Seren's came\n{text}");
        shot_stack(&name("read"), &st, &[&map, &term], size);

        // Out of the program, `mail 4` printed on the rig's own glass.
        term.update(&mut st, &[key(KeyCode::Esc), key(KeyCode::Esc)]);
        type_lines(&mut st, &mut term, &["mail 4"]);
        st.tick += 10_000;
        term.update(&mut st, &[]);
        let text = buffer_text(&draw_stack(&st, &[&map, &term], size));
        assert!(
            !text.contains("HELIX mail 2.2"),
            "back at the prompt\n{text}"
        );
        assert!(
            text.contains("MAIL 4 OF 5") && text.contains("FROM     MEZZ"),
            "mail 4 is read out on the glass\n{text}"
        );
        shot_stack(&name("n"), &st, &[&map, &term], size);
    }
}

/// The clock on to `minute` of the day, the city with it.
fn on_to(st: &mut AppState, minute: u16) {
    let ahead = (minute as u64 + 1440 - st.minute_of_day() as u64) % 1440;
    let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
    for _ in 0..ahead {
        let t = world.tick() + sim::TICKS_PER_MINUTE;
        world.advance(city, t);
        world.path_some(city, 64);
    }
}

/// Kiroflux Mart's node of a kind: its id and its tile on the Net.
fn kiroflux_node(st: &AppState, kind: citygen::NodeKind) -> (citygen::NodeId, Pos) {
    let city = st.city.as_ref().unwrap();
    let b = city
        .buildings
        .iter()
        .find(|b| b.name.as_deref() == Some("Kiroflux Mart"))
        .unwrap()
        .id;
    let n = city
        .net
        .nodes
        .iter()
        .find(|n| n.building == Some(b) && n.kind == kind)
        .unwrap();
    (n.id, n.cyber)
}

fn kiroflux(st: &AppState) -> citygen::BuildingId {
    st.city
        .as_ref()
        .unwrap()
        .buildings
        .iter()
        .find(|b| b.name.as_deref() == Some("Kiroflux Mart"))
        .unwrap()
        .id
}

/// On the lane under Kiroflux Mart's camera, the camera and an ad board held; with
/// `tap`, the camera's feed open. Spec `2026-11-04-net-nodes-design.md`.
fn held_camera(tap: bool) -> (AppState, MapScreen) {
    use citygen::NodeKind;
    let (mut st, mut map) = on_the_open_net();
    on_to(&mut st, 21 * 60 + 30);
    let b = kiroflux(&st);
    let (cam, at) = kiroflux_node(&st, NodeKind::Camera);
    let (ad, _) = kiroflux_node(&st, NodeKind::AdBoard);
    let here = Pos::new(at.x - 2, at.y + 1);
    stand_in(&mut st, Place::Stratum(StratumKind::Net), here);
    map.cam.snap(here);
    let now = st.net.now(&st);
    crate::holds::take(&mut st, cam, crate::owners::Owner::Building(b), now);
    crate::holds::take(&mut st, ad, crate::owners::Owner::Building(b), now);
    if tap {
        crate::holds::use_verb(&mut st, 'f').unwrap();
    }
    map.update(&mut st, &[]);
    (st, map)
}

#[test]
#[ignore]
fn net_held() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = held_camera(false);
    for (name, size) in [("net_held", (120, 40)), ("net_held_small", (100, 30))] {
        write_svg(&dir, name, &st, &draw_stack(&st, &[&map], size), false);
    }
    // Out of reach: the held nodes are only green, no dock.
    let city = st.city.clone().unwrap();
    let me = st.player.as_ref().unwrap().pos;
    let far = (14..30)
        .flat_map(|d| [Pos::new(me.x - d, me.y), Pos::new(me.x, me.y + d)])
        .find(|q| city.walkable(StratumKind::Net, *q))
        .expect("somewhere out of reach");
    stand_in(&mut st, Place::Stratum(StratumKind::Net), far);
    map.cam
        .snap(Pos::new((far.x + me.x) / 2, (far.y + me.y) / 2));
    map.update(&mut st, &[]);
    write_svg(
        &dir,
        "net_held_far",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
}

#[test]
#[ignore]
fn net_feed() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (st, map) = held_camera(true);
    for (name, size) in [("net_feed", (120, 40)), ("net_feed_small", (100, 30))] {
        let buf = draw_stack(&st, &[&map], size);
        assert!(
            buffer_text(&buf).contains("CAM · KIROFLUX"),
            "feed {:?} held {:?} place {:?}\n{}",
            st.net.feed,
            st.net.holds,
            st.player.as_ref().map(|p| (p.place, p.pos, p.body)),
            buffer_text(&buf)
        );
        write_svg(&dir, name, &st, &buf, false);
    }
}

/// A crack in flight from the lane on Kiroflux Mart's wall camera: the tether out on
/// the open Net.
#[test]
#[ignore]
fn net_outside_run() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    let (cam, at) = kiroflux_node(&st, citygen::NodeKind::Camera);
    let here = Pos::new(at.x, at.y + 1);
    stand_in(&mut st, Place::Stratum(StratumKind::Net), here);
    map.cam.snap(here);
    crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(cam)).unwrap();
    crate::net::step_run(&mut st);
    st.net.beat = st.clock_tick();
    pass(&mut st, crate::net::BEAT_TICKS / 2);
    map.update(&mut st, &[]);
    for (name, size) in [
        ("net_outside_run", (120, 40)),
        ("net_outside_run_small", (100, 30)),
    ] {
        write_svg(&dir, name, &st, &draw_stack(&st, &[&map], size), false);
    }
}

/// On foot at night in front of Kiroflux Mart, in its camera's cone.
fn on_foot_by_kiroflux(minute: u16) -> (AppState, MapScreen, Pos) {
    let mut st = state_home();
    on_to(&mut st, minute);
    let city = st.city.clone().unwrap();
    let cam = crate::cameras::street_cameras(&city)
        .find(|c| c.building == kiroflux(&st))
        .unwrap();
    // The walkable tile of its cone furthest out within its dark reach.
    let here = crate::cameras::cone(&city, &cam)
        .into_iter()
        .filter(|q| city.walkable(StratumKind::Street, *q))
        .filter(|q| q.chebyshev(cam.at) <= crate::cameras::DARK_REACH)
        .max_by_key(|q| (q.chebyshev(cam.at), -(q.x - cam.at.x).abs()))
        .expect("somewhere to stand in the cone");
    st.player = Some(crate::walker::Player::at(
        Place::Stratum(StratumKind::Street),
        here,
    ));
    stand_in(&mut st, Place::Stratum(StratumKind::Street), here);
    let mut map = MapScreen::walking(&mut st);
    map.cam.snap(here);
    map.update(&mut st, &[]);
    (st, map, cam.at)
}

#[test]
#[ignore]
fn street_eyes() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (st, map, cam) = on_foot_by_kiroflux(22 * 60);
    assert!(
        !crate::cameras::seeing(&st).is_empty(),
        "the camera at {cam:?} sees you at {:?}",
        st.player.as_ref().map(|p| (p.place, p.pos))
    );
    let buf = draw_stack(&st, &[&map], (120, 40));
    write_svg(&dir, "street_eyes", &st, &buf, false);
}

#[test]
#[ignore]
fn street_hijack() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map, _) = on_foot_by_kiroflux(22 * 60);
    let b = kiroflux(&st);
    let (ad, _) = kiroflux_node(&st, citygen::NodeKind::AdBoard);
    let now = st.net.now(&st);
    crate::holds::take(&mut st, ad, crate::owners::Owner::Building(b), now);
    crate::holds::hijack(&mut st, ad, crate::holds::Line::Smear);
    map.update(&mut st, &[]);
    pass(&mut st, 60);
    map.update(&mut st, &[]);
    assert!(!st.missions.asides.is_empty(), "the ad says its line");
    write_svg(
        &dir,
        "street_hijack",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
}

#[test]
#[ignore]
fn street_dark() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map, _) = on_foot_by_kiroflux(22 * 60);
    let city = st.city.clone().unwrap();
    let d = city.buildings[kiroflux(&st).index()].district;
    // Out on the district's most lamp-lit pavement, lamps the only light.
    let street = city.stratum(StratumKind::Street).unwrap();
    let here = street
        .tiles
        .iter()
        .filter(|(p, t)| {
            t.district == Some(d)
                && t.building.is_none()
                && t.is_walkable()
                && street.steady[*p] == 0
        })
        .max_by_key(|(p, _)| street.lamps[*p])
        .map(|(p, _)| p)
        .unwrap();
    stand_in(&mut st, Place::Stratum(StratumKind::Street), here);
    map.cam.snap(here);
    map.update(&mut st, &[]);
    write_svg(
        &dir,
        "street_dark_before",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
    let now = st.net.now(&st);
    st.net
        .lights_out
        .insert(d, now + crate::holds::LIGHTS_OUT_MINUTES);
    crate::holds::tick(&mut st);
    let here = st.player.as_ref().unwrap().pos;
    stand_in(&mut st, Place::Stratum(StratumKind::Street), here);
    map.update(&mut st, &[]);
    write_svg(
        &dir,
        "street_dark",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
}

/// Inside a construct with a walking piece and an ad board, the board held and its
/// decoy up, two beats on.
#[test]
#[ignore]
fn net_decoy() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let mut st = state_home();
    let mut map = MapScreen::walking(&mut st);
    let city = st.city.clone().unwrap();
    let (b, ad, at) = city
        .buildings
        .iter()
        .find_map(|b| {
            let k = citygen::construct::generate_construct(&city, b.id)?;
            let g = k.guards.iter().find(|g| g.beat.len() >= 2 && !g.inert)?;
            let (ad, at) = k
                .nodes
                .iter()
                .find(|(n, _)| city.net.nodes[n.index()].kind == citygen::NodeKind::AdBoard)?;
            (g.pos.chebyshev(*at) > 4).then_some((b.id, *ad, *at))
        })
        .expect("a construct with a walking piece and an ad board");
    {
        let p = st.player.as_mut().unwrap();
        p.body = Some((p.place, p.pos));
        p.place = Place::Stratum(StratumKind::Net);
        p.pos = crate::constructs::city_gate(&city, b).unwrap();
    }
    crate::constructs::enter_construct(&mut st, b).unwrap();
    map.place = Place::Construct(b);
    let k = st.constructs.get(b).unwrap().clone();
    let here = at
        .neighbors8()
        .find(|q| k.tiles.get(*q).is_some_and(|t| t.is_walkable()))
        .unwrap_or(at);
    stand_in(&mut st, Place::Construct(b), here);
    map.cam.snap(here);
    let now = st.net.now(&st);
    crate::holds::take(&mut st, ad, crate::owners::Owner::Building(b), now);
    crate::holds::use_verb(&mut st, 'z').unwrap();
    for _ in 0..2 {
        st.net.beat = st.clock_tick();
        crate::net::beat(&mut st);
        pass(&mut st, 2);
    }
    map.update(&mut st, &[]);
    write_svg(
        &dir,
        "net_decoy",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
}

/// Up the lane from Kiroflux Mart, a tracer off its gate on the trail behind you.
#[test]
#[ignore]
fn net_tracer() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    st.grid_patrols = crate::grid::Patrols::Sweeps;
    let b = kiroflux(&st);
    // Walk west along the lane under the gate, then the tracer sets off.
    let start = st.player.as_ref().unwrap().pos;
    let city = st.city.clone().unwrap();
    let mut p = start;
    for _ in 0..10 {
        let q = Pos::new(p.x - 1, p.y);
        if !city.walkable(StratumKind::Net, q) {
            break;
        }
        p = q;
        stand_in(&mut st, Place::Stratum(StratumKind::Net), p);
        map.update(&mut st, &[]);
    }
    crate::grid::send_tracer(&mut st, crate::owners::Owner::Building(b));
    for _ in 0..6 {
        pass(&mut st, crate::grid::TRACER_TICKS);
        map.update(&mut st, &[]);
    }
    map.cam.snap(p);
    map.update(&mut st, &[]);
    let buf = draw_stack(&st, &[&map], (120, 40));
    assert!(
        buffer_text(&buf).contains('◈'),
        "me {:?} foes {:?}",
        st.player.as_ref().map(|p| (p.place, p.pos)),
        st.foes
            .iter()
            .map(|f| (f.place, f.pos, f.down))
            .collect::<Vec<_>>()
    );
    write_svg(&dir, "net_tracer", &st, &buf, false);
}

/// Beside the hub of Kiroflux Mart's district, its sentinels on their beats.
#[test]
#[ignore]
fn net_sentinels() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    let city = st.city.clone().unwrap();
    let d = city.buildings[kiroflux(&st).index()].district;
    let (_, hub) = crate::grid::hubs(&city)
        .into_iter()
        .find(|(x, _)| *x == d)
        .unwrap();
    let here = (0..12)
        .flat_map(|r| {
            [
                Pos::new(hub.x + 6 + r, hub.y + 4),
                Pos::new(hub.x - 6 - r, hub.y + 4),
            ]
        })
        .find(|q| city.walkable(StratumKind::Net, *q))
        .unwrap();
    stand_in(&mut st, Place::Stratum(StratumKind::Net), here);
    map.cam.snap(hub);
    map.update(&mut st, &[]);
    pass(&mut st, crate::grid::SENTINEL_TICKS * 2);
    map.update(&mut st, &[]);
    write_svg(
        &dir,
        "net_sentinels",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
}

/// Sweeps on, Kiroflux Mart's district on alert: a ring out of its hub.
#[test]
#[ignore]
fn net_sweep() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    st.grid_patrols = crate::grid::Patrols::Sweeps;
    let city = st.city.clone().unwrap();
    let d = city.buildings[kiroflux(&st).index()].district;
    let (_, hub) = crate::grid::hubs(&city)
        .into_iter()
        .find(|(x, _)| *x == d)
        .unwrap();
    crate::owners::heat_on(&mut st, crate::owners::Owner::District(d), game::ALARM_HEAT);
    map.cam.snap(hub);
    for _ in 0..(crate::grid::SWEEP_BEATS * crate::net::BEAT_TICKS + 24) {
        pass(&mut st, 1);
        map.update(&mut st, &[]);
    }
    write_svg(
        &dir,
        "net_sweep",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
}

/// Space beside Kiroflux Mart's camera on the Net: what the deck reaches; then, the
/// camera held, its verbs.
#[test]
#[ignore]
fn net_space_camera() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    let (cam, at) = kiroflux_node(&st, citygen::NodeKind::Camera);
    let here = Pos::new(at.x, at.y + 1);
    stand_in(&mut st, Place::Stratum(StratumKind::Net), here);
    map.cam.snap(here);
    map.update(&mut st, &[]);
    let crate::app::ScreenCmd::Push { to: w, .. } = map.update(&mut st, &[key(KeyCode::Char(' '))])
    else {
        panic!("the window");
    };
    st.tick += 120;
    write_svg(
        &dir,
        "net_space_camera",
        &st,
        &draw_stack(&st, &[&map, w.as_ref()], (120, 40)),
        false,
    );
    let b = kiroflux(&st);
    let now = st.net.now(&st);
    crate::holds::take(&mut st, cam, crate::owners::Owner::Building(b), now);
    map.update(&mut st, &[]);
    let crate::app::ScreenCmd::Push { to: w, .. } = map.update(&mut st, &[key(KeyCode::Char(' '))])
    else {
        panic!("the window");
    };
    st.tick += 120;
    write_svg(
        &dir,
        "net_space_held",
        &st,
        &draw_stack(&st, &[&map, w.as_ref()], (120, 40)),
        false,
    );
}

/// A street terminal by Kiroflux Mart held from the Net and its shell opened there
/// (`r`), typed out; and the Net behind it alone.
#[test]
#[ignore]
fn net_shell() {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let (mut st, mut map) = on_the_open_net();
    let city = st.city.clone().unwrap();
    let d = city.buildings[kiroflux(&st).index()].district;
    let owner = crate::owners::Owner::District(d);
    let term = city
        .net
        .nodes
        .iter()
        .filter(|n| {
            n.kind == citygen::NodeKind::Terminal
                && crate::owners::owner_of(&city, n.id) == Some(owner)
        })
        .min_by_key(|n| n.cyber.chebyshev(Pos::new(201, 62)))
        .expect("a street terminal");
    let (id, at) = (term.id, term.cyber);
    let here = at
        .neighbors8()
        .find(|q| city.walkable(StratumKind::Net, *q))
        .unwrap_or(at);
    stand_in(&mut st, Place::Stratum(StratumKind::Net), here);
    map.cam.snap(here);
    let now = st.net.now(&st);
    crate::holds::take(&mut st, id, owner, now);
    map.update(&mut st, &[]);
    write_svg(
        &dir,
        "net_shell_bg",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
    let mut shell = crate::screens::popup::verb_opens(&mut st, 'r').expect("the shell");
    for _ in 0..40 {
        st.tick += 30;
        shell.update(&mut st, &[]);
    }
    write_svg(
        &dir,
        "net_shell",
        &st,
        &draw_stack(&st, &[&map, shell.as_ref()], (120, 40)),
        false,
    );
}

/// The save mock's three runs in `dir`, from VEX at home in seed 8 (the newest, saved
/// today at 10:44) and two copies of her file under other runners' headers.
fn the_mock_runs(dir: &crate::runs::testutil::Dir) {
    use crate::runs::{RunHeader, file};
    let mut st = state_home();
    let _ = MapScreen::walking(&mut st);
    let mut st = crate::runs::testutil::running(st);
    st.saves = Some(dir.0.clone());
    assert!(crate::runs::when::save_now(&mut st));
    let vex = dir.0.join(format!("{}.run", st.run.unwrap().id));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let today = now / 86_400 * 86_400;
    let edit = |path: &std::path::Path, f: &dyn Fn(&mut RunHeader)| {
        let mut h = file::header(path).unwrap();
        f(&mut h);
        file::rewrite_header(path, &h).unwrap();
    };
    edit(&vex, &|h| {
        h.level = 4;
        h.day = 2;
        h.minute = 7 * 60 + 40;
        h.mission = Some("The ledger".into());
        h.objective = Some("Bring it to Coz · the Brass Lamp · Ironside".into());
        h.credits = 1_240;
        h.chrome = vec!["Neural port".into(), "Target lens".into()];
        h.talents = 6;
        h.played = 3 * 3600 + 12 * 60;
        h.perma = true;
        h.saved = today + 10 * 3600 + 44 * 60;
    });
    for (name, build, level, city, (day, minute), story, perma, saved) in [
        (
            "KESTREL",
            "SOLO",
            2,
            "Lowport",
            (1, 23 * 60 + 10),
            "See the doctor",
            true,
            today - 86_400,
        ),
        (
            "BRAM",
            "TECHIE",
            1,
            "Chromespire",
            (1, 6 * 60 + 5),
            "Watch the news",
            false,
            today - 2 * 86_400,
        ),
    ] {
        let path = dir.0.join(format!("{}.run", name.to_lowercase()));
        std::fs::copy(&vex, &path).unwrap();
        edit(&path, &|h| {
            h.handle = name.into();
            h.profession = build.into();
            h.level = level;
            h.city = city.into();
            h.day = day;
            h.minute = minute;
            h.mission = Some("The morning after".into());
            h.objective = Some(story.into());
            h.perma = perma;
            h.saved = saved;
        });
    }
}

/// The title with a run to go on with, as players see it and in dev mode, and without one
/// (the save mock's `title()`), at 120x40.
#[test]
#[ignore]
fn save_title_shots() {
    let Ok(out) = std::env::var("SHOT_DIR") else {
        return;
    };
    let dir = crate::runs::testutil::Dir::new("title-shots");
    the_mock_runs(&dir);
    let empty = crate::runs::testutil::Dir::new("title-shots-empty");
    for (name, saves, dev) in [
        ("save_title", &dir, false),
        ("save_title_dev", &dir, true),
        ("save_title_none", &empty, false),
    ] {
        let mut st = AppState::new();
        if dev {
            st.unlock_dev();
        }
        st.saves = Some(saves.0.clone());
        let mut title = crate::screens::title::Title::new(&st, 0);
        for _ in 0..90 {
            st.tick += 1;
            title.update(&mut st, &[]);
        }
        let buf = draw_stack(&st, &[&title], (120, 40));
        write_svg(&out, name, &st, &buf, false);
    }
}

/// LOAD GAME with the save mock's runs, with a file it can't read among them, and
/// empty; and the broadcast a run opens with (the save mock's `load_game()`), at
/// 120x40.
#[test]
#[ignore]
fn save_load_shots() {
    use crate::screens::runs::RunsScreen;
    let Ok(out) = std::env::var("SHOT_DIR") else {
        return;
    };
    let dir = crate::runs::testutil::Dir::new("load-shots");
    the_mock_runs(&dir);
    let mut st = AppState::new();
    st.saves = Some(dir.0.clone());
    let list = RunsScreen::load(&st);
    write_svg(
        &out,
        "save_load",
        &st,
        &draw_stack(&st, &[&list], (120, 40)),
        false,
    );

    std::fs::write(dir.0.join("zz.run"), b"not a run").unwrap();
    let mut list = RunsScreen::load(&st);
    list.update(&mut st, &[key(KeyCode::Up)]);
    write_svg(
        &out,
        "save_load_unreadable",
        &st,
        &draw_stack(&st, &[&list], (120, 40)),
        false,
    );

    let empty = crate::runs::testutil::Dir::new("load-shots-empty");
    st.saves = Some(empty.0.clone());
    let list = RunsScreen::load(&st);
    write_svg(
        &out,
        "save_load_empty",
        &st,
        &draw_stack(&st, &[&list], (120, 40)),
        false,
    );

    // VEX's run opened: its broadcast, the run's own flags and no switches.
    st.saves = Some(dir.0.clone());
    let mut list = RunsScreen::load(&st);
    let crate::app::ScreenCmd::Switch { to: mut on_air, .. } =
        list.update(&mut st, &[key(KeyCode::Char(' '))])
    else {
        panic!("space opens VEX's run")
    };
    on_air.update(&mut st, &[key(KeyCode::Char(' '))]);
    write_svg(
        &out,
        "save_resume_broadcast",
        &st,
        &draw_stack(&st, &[on_air.as_ref()], (120, 40)),
        false,
    );
}

/// VEX at home in seed 8 with a run saved in `dir`: the state, the map and the file.
fn vex_saved_at_home(
    dir: &crate::runs::testutil::Dir,
) -> (AppState, MapScreen, std::path::PathBuf) {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    let mut st = crate::runs::testutil::running(st);
    st.saves = Some(dir.0.clone());
    assert!(crate::runs::when::save_now(&mut st));
    let path = dir.0.join(format!("{}.run", st.run.unwrap().id));
    (st, map, path)
}

/// The HALL OF FAME with the save mock's three dead, copies of VEX's file under their
/// headers, and before anyone has flatlined (the save mock's `hall()` and
/// `hall_empty()`), at 120x40.
#[test]
#[ignore]
fn save_hall_shots() {
    use crate::runs::{Death, file};
    use crate::screens::runs::RunsScreen;
    let Ok(out) = std::env::var("SHOT_DIR") else {
        return;
    };
    let dir = crate::runs::testutil::Dir::new("hall-shots");
    let (_, _, vex) = vex_saved_at_home(&dir);
    // 2026-11-02 10:44 UTC.
    let nov_2 = 1_793_616_240;
    for (i, (name, level, city, lasted, cause)) in [
        (
            "MOTH",
            6,
            "Saintburg",
            3 * 1440 + 4 * 60,
            "black ICE · Velform Group's vault",
        ),
        ("RIX", 3, "Lowport", 19 * 60 + 20, "black ICE · Zenbo"),
        ("NAIL", 1, "Chromespire", 2 * 60 + 5, "black ICE · Aoki"),
    ]
    .into_iter()
    .enumerate()
    {
        let path = dir.0.join(format!("{}.run", name.to_lowercase()));
        std::fs::copy(&vex, &path).unwrap();
        let mut h = file::header(&path).unwrap();
        h.handle = name.into();
        h.profession = "NETRUNNER".into();
        h.level = level;
        h.city = city.into();
        h.mission = Some("The ledger".into());
        h.objective = Some("Bring it to Coz · the Brass Lamp · Ironside".into());
        h.credits = 2_340;
        h.chrome = vec!["neural port".into(), "target lens".into()];
        h.talents = 9;
        h.played = 5 * 3600 + 40 * 60;
        h.death = Some(Death {
            cause: cause.into(),
            day: 4,
            minute: 3 * 60 + 12,
            at: nov_2 - i as u64 * 86_400,
            lasted,
        });
        file::rewrite_header(&path, &h).unwrap();
    }
    let mut st = AppState::new();
    st.saves = Some(dir.0.clone());
    let hall = RunsScreen::hall(&st);
    write_svg(
        &out,
        "save_hall",
        &st,
        &draw_stack(&st, &[&hall], (120, 40)),
        false,
    );
    let empty = crate::runs::testutil::Dir::new("hall-shots-empty");
    st.saves = Some(empty.0.clone());
    let hall = RunsScreen::hall(&st);
    write_svg(
        &out,
        "save_hall_empty",
        &st,
        &draw_stack(&st, &[&hall], (120, 40)),
        false,
    );
}

/// The flatline with perma death on (the first taunt, as the save mock draws it) and
/// off (VEX's own last save at home, the line the game makes of it): the save mock's
/// `flatline(True)` and `flatline(False)`, at 120x40.
#[test]
#[ignore]
fn save_flatline_shots() {
    use crate::screens::flatlined::{End, Flatlined, Taunts};
    let Ok(out) = std::env::var("SHOT_DIR") else {
        return;
    };
    let dir = crate::runs::testutil::Dir::new("flatline-shots");
    let (mut st, _, _) = vex_saved_at_home(&dir);
    let tower = {
        let city = st.city.as_ref().unwrap();
        city.buildings
            .iter()
            .find(|b| b.kind == citygen::BuildingKind::CorpTower)
            .unwrap()
            .id
    };
    let cause = format!("black ICE at {}", crate::net::building_name(&st, tower));
    st.perma_death = false;
    st.net.death = Some(cause.clone());
    let reload = crate::screens::flatlined::end(&mut st, &cause);
    assert!(
        matches!(reload, End::Reload { .. }),
        "the last save to go back to"
    );
    let perma = End::Perma {
        taunt: Taunts::load().all()[0].clone(),
    };
    for (name, end) in [
        ("save_flatline_perma", perma),
        ("save_flatline_reload", reload),
    ] {
        let screen = Flatlined::new(cause.clone(), &st.character.handle, 1, st.tick, end);
        st.tick += 40;
        write_svg(
            &out,
            name,
            &st,
            &draw_stack(&st, &[&screen], (120, 40)),
            false,
        );
    }
}

/// The pause menu two minutes after a save, its line under the rows, and the map just
/// saved, the SAVED mark after the place (the save mock's `pause()` and
/// `saved_mark()`), at 120x40.
#[test]
#[ignore]
fn save_pause_shots() {
    let Ok(out) = std::env::var("SHOT_DIR") else {
        return;
    };
    let dir = crate::runs::testutil::Dir::new("pause-shots");
    let (mut st, mut map, _) = vex_saved_at_home(&dir);
    map.settle(&mut st, (120, 40));
    assert!(crate::runs::when::save_now(&mut st));
    assert!(crate::runs::when::marked(&st));
    write_svg(
        &out,
        "save_map_mark",
        &st,
        &draw_stack(&st, &[&map], (120, 40)),
        false,
    );
    st.tick += crate::runs::when::MARK;
    st.last_save = st
        .last_save
        .map(|(t, at)| (t, at - std::time::Duration::from_secs(150)));
    let pause = crate::screens::pause::Pause::new(&st, st.tick);
    st.tick += 90;
    write_svg(
        &out,
        "save_pause",
        &st,
        &draw_stack(&st, &[&map, &pause], (120, 40)),
        false,
    );
}

/// The world beyond the city (#22): the overworld after going up from the large seed-8
/// city, and down on the open ground on a highway, beside a bridge, by day and night.
#[test]
#[ignore]
fn outside_shots() {
    use crate::outside::At;
    use crate::screens::outside::OutsideScreen;
    use citygen::overworld::Water;
    let city = Generator::run(GenParams {
        seed: 8,
        size: SizePreset::Large,
        districts: 7,
        ..Default::default()
    })
    .unwrap();
    let at = |minute: u16| {
        let mut st = AppState::new();
        st.enter_city_at(city.clone(), minute);
        st.character = game::Character::debug_runner(&st.content);
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        crate::outside::ensure(&mut st);
        st
    };
    // Up from the city: on its block on the overworld.
    let mut st = at(13 * 60);
    let s = OutsideScreen::new(&mut st);
    shot_stack("outside_overworld", &st, &[&s], (120, 40));
    shot_stack("outside_overworld_small", &st, &[&s], (100, 30));
    // Down on the open ground: a highway, then a bridge, outside the city's block.
    let pick = |st: &AppState, f: &dyn Fn(&citygen::overworld::WorldTile) -> bool| {
        let o = st.outside.as_ref().unwrap();
        let b = o.block(st.city.as_ref().unwrap());
        o.world()
            .tiles
            .iter()
            .find(|(p, t)| !b.contains(*p) && f(t))
            .map(|(p, _)| p)
            .unwrap()
    };
    use citygen::overworld::{LINK_E, LINK_W};
    for (name, minute, f) in [
        (
            "outside_ground_highway",
            13 * 60,
            &(|t: &citygen::overworld::WorldTile| {
                t.road == LINK_E | LINK_W && t.water == Water::None
            }) as &dyn Fn(&citygen::overworld::WorldTile) -> bool,
        ),
        (
            "outside_ground_bridge",
            13 * 60,
            &|t: &citygen::overworld::WorldTile| t.road != 0 && t.water == Water::River,
        ),
        (
            "outside_ground_night",
            22 * 60,
            &|t: &citygen::overworld::WorldTile| {
                t.road == LINK_E | LINK_W && t.water == Water::None
            },
        ),
    ] {
        let mut st = at(minute);
        let t = pick(&st, f);
        st.outside.as_mut().unwrap().at = At::Overworld(t);
        let mut s = OutsideScreen::new(&mut st);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        // a few steps along, so the memory shows behind you
        for _ in 0..6 {
            s.update(&mut st, &[key(KeyCode::Char('l'))]);
        }
        shot_stack(name, &st, &[&s], (120, 40));
    }
}

/// POIs one level down (#17): the truck stop by day and by night, a checkpoint, a camp
/// at night and a rig, on the seed-8 large city's world, standing on each.
#[test]
#[ignore]
fn poi_shots() {
    use crate::outside::At;
    use crate::screens::outside::OutsideScreen;
    use citygen::overworld::PoiKind;
    let city = Generator::run(GenParams {
        seed: 8,
        size: SizePreset::Large,
        districts: 7,
        ..Default::default()
    })
    .unwrap();
    for (name, kind, minute) in [
        ("poi_truck_stop_day", PoiKind::TruckStop, 13 * 60),
        ("poi_truck_stop_night", PoiKind::TruckStop, 22 * 60),
        ("poi_checkpoint", PoiKind::Checkpoint, 13 * 60),
        ("poi_camp_night", PoiKind::Camp, 22 * 60),
        ("poi_rig", PoiKind::Rig, 13 * 60),
        ("poi_solar", PoiKind::Solar, 13 * 60),
        ("poi_water_plant", PoiKind::WaterPlant, 13 * 60),
    ] {
        let mut st = AppState::new();
        st.enter_city_at(city.clone(), minute);
        st.character = game::Character::debug_runner(&st.content);
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        crate::outside::ensure(&mut st);
        let o = st.outside.as_ref().unwrap();
        let Some(poi) = o.world().pois.iter().find(|p| p.kind == kind).cloned() else {
            eprintln!("{name}: none in this world");
            continue;
        };
        st.outside.as_mut().unwrap().at = At::Overworld(poi.at);
        let mut s = OutsideScreen::new(&mut st);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        s.update(&mut st, &[]);
        shot_stack(name, &st, &[&s], (120, 40));
    }
}

/// The open Net (#18): jacked in beside the truck stop's pump, by day.
#[test]
#[ignore]
fn open_net_shots() {
    use crate::outside::{At, TILE};
    use crate::screens::outside::OutsideScreen;
    use citygen::overworld::PoiKind;
    let city = Generator::run(GenParams {
        seed: 8,
        size: SizePreset::Large,
        districts: 7,
        ..Default::default()
    })
    .unwrap();
    let mut st = AppState::new();
    st.enter_city_at(city, 13 * 60);
    st.character = game::Character::debug_runner(&st.content);
    st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
    crate::outside::ensure(&mut st);
    let w = st.outside.as_ref().unwrap().world().clone();
    let poi = w
        .pois
        .iter()
        .find(|p| p.kind == PoiKind::TruckStop)
        .unwrap()
        .clone();
    let o = Pos::new(TILE * (poi.at.x - 3), TILE * (poi.at.y - 3));
    let g = citygen::overworld::detail::region(&w, o.x, o.y, 7 * TILE as u32, 7 * TILE as u32);
    let pump = g
        .iter()
        .find(|(_, t)| t.feature == Some(citygen::Feature::Furniture(citygen::Furniture::Vending)))
        .map(|(p, _)| p)
        .unwrap();
    let beside = pump
        .neighbors4()
        .find(|q| g.get(*q).is_some_and(|t| t.is_walkable()))
        .unwrap();
    st.outside.as_mut().unwrap().at = At::Ground(Pos::new(o.x + beside.x, o.y + beside.y));
    let mut s = OutsideScreen::new(&mut st);
    s.update(&mut st, &[key(KeyCode::Char(' '))]);
    s.update(&mut st, &[]);
    shot_stack("open_net_truck_stop", &st, &[&s], (120, 40));
}
