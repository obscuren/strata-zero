//! Screenshots for a look at the devices: ignored tests that draw a screen over the
//! washed map and write it as an SVG. Run with `SHOT_DIR=... cargo test -p strata-zero
//! shots:: -- --ignored`.

use crate::access::Session;
use crate::app::{AppState, Screen};
use crate::screens::explore::MapScreen;
use crate::screens::keypad::Keypad;
use crate::screens::terminal::Terminal;
use crate::screens::testutil::key;
use citygen::{GenParams, Generator, Place, Pos, SizePreset, StratumKind};
use crossterm::event::KeyCode;
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
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    let mut t = Term::new(TestBackend::new(120, 40)).unwrap();
    t.draw(|f| {
        map.draw(st, f);
        crate::ui::dim::wash(f.buffer_mut());
        top.draw(st, f);
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
    let (b, level, number) = st.journal.favour.clone().expect("the favour").flat;
    let mut pad = Keypad::open(&st, b, level, number);
    pad.update(&mut st, &[key(KeyCode::Char('4')), key(KeyCode::Char('4'))]);
    shot("keypad", &st, &map, &pad);
}

/// A terminal session in a district, after SCAN, CONNECT 1 and a denied DOOR UNLOCK.
fn terminal_shot(name: &str, d: citygen::DistrictId) {
    let mut st = state_home();
    let map = MapScreen::walking(&mut st);
    let mut term = Terminal::open(
        &mut st,
        Session::District {
            district: d,
            connected: None,
        },
        (Place::Stratum(StratumKind::Street), Pos::new(1, 1)),
    );
    for line in ["scan", "connect 1", "door unlock"] {
        st.tick += 10_000;
        term.update(&mut st, &[]);
        let keys: Vec<_> = line
            .chars()
            .map(|c| key(KeyCode::Char(c)))
            .chain(std::iter::once(key(KeyCode::Enter)))
            .collect();
        term.update(&mut st, &keys);
    }
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
    let mut term = Terminal::open(
        &mut st,
        Session::Rig {
            building: home.building,
        },
        (map.place, home.rig),
    );
    st.tick += 10_000;
    term.update(&mut st, &[]);
    let keys: Vec<_> = "mail 1"
        .chars()
        .map(|c| key(KeyCode::Char(c)))
        .chain(std::iter::once(key(KeyCode::Enter)))
        .collect();
    term.update(&mut st, &keys);
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
        Session::District {
            district: d,
            connected: None,
        },
        (Place::Stratum(StratumKind::Street), Pos::new(1, 1)),
    );
    st.tick += 10_000;
    let keys: Vec<_> = "scan"
        .chars()
        .map(|c| key(KeyCode::Char(c)))
        .chain(std::iter::once(key(KeyCode::Enter)))
        .collect();
    term.update(&mut st, &keys);
    st.tick += 10_000;
    term.update(&mut st, &[]);
    let keys: Vec<_> = "connect 1"
        .chars()
        .map(|c| key(KeyCode::Char(c)))
        .chain(std::iter::once(key(KeyCode::Enter)))
        .collect();
    term.update(&mut st, &keys);
    st.tick += 10_000;
    term.update(&mut st, &[]);
    let keys: Vec<_> = "door unlock"
        .chars()
        .map(|c| key(KeyCode::Char(c)))
        .chain(std::iter::once(key(KeyCode::Enter)))
        .collect();
    term.update(&mut st, &keys);
    st.tick += 200;
    term.update(&mut st, &[]);
    shot("terminal", &st, &map, &term);
}
