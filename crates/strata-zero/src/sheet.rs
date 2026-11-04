//! `strata world`'s sheet: many overworlds on one HTML page, drawn in the atlas look,
//! to compare seeds and tune the generator in a browser.

use crate::render::overworld::atlas_cell;
use crate::render::style::StylePack;
use crate::render::{Cell, Rgb};
use citygen::Pos;
use citygen::overworld::{Overworld, PoiKind, RegionKind, Water};
use std::fmt::Write as _;
use std::time::Duration;

const FIRA_REGULAR: &[u8] = include_bytes!("../fonts/FiraCode-Regular.ttf");
const FIRA_BOLD: &[u8] = include_bytes!("../fonts/FiraCode-Bold.ttf");

/// Standard base64 with padding.
fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

fn escape(ch: char, out: &mut String) {
    match ch {
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '&' => out.push_str("&amp;"),
        '"' => out.push_str("&quot;"),
        c => out.push(c),
    }
}

/// One world as rows of spans, a run per style; a glyph outside ASCII sits in its own
/// fixed-width box so a fallback font can't push the row out of line.
fn grid(world: &Overworld, style: &StylePack) -> String {
    let mut out = String::from("<div class=tm>");
    for y in 0..world.size.h as i32 {
        out.push_str("<div class=r>");
        let cells: Vec<Cell> = (0..world.size.w as i32)
            .map(|x| atlas_cell(world, Pos::new(x, y), style))
            .collect();
        let mut i = 0;
        while i < cells.len() {
            let c = cells[i];
            let mut j = i;
            while j < cells.len()
                && cells[j].fg == c.fg
                && cells[j].bg == c.bg
                && cells[j].bold == c.bold
            {
                j += 1;
            }
            let _ = write!(
                out,
                "<span style=\"color:{};background:{}{}\">",
                hex(c.fg),
                hex(c.bg),
                if c.bold { ";font-weight:700" } else { "" }
            );
            for cell in &cells[i..j] {
                if cell.ch.is_ascii() {
                    escape(cell.ch, &mut out);
                } else {
                    out.push_str("<i>");
                    escape(cell.ch, &mut out);
                    out.push_str("</i>");
                }
            }
            out.push_str("</span>");
            i = j;
        }
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}

fn summary(world: &Overworld, took: Duration) -> String {
    let mut out = String::new();
    let land = world.tiles.count(|t| matches!(t.water, Water::None | Water::River)) as f32
        / world.size.area() as f32;
    let _ = write!(
        out,
        "<h2>seed {} · {}×{} · {} cities · {} ms</h2>",
        world.seed,
        world.size.w,
        world.size.h,
        world.cities.len(),
        took.as_millis()
    );
    let cities: Vec<String> = world
        .cities
        .iter()
        .map(|c| {
            let towns = if c.towns.is_empty() {
                String::new()
            } else {
                format!(", {} towns", c.towns.len())
            };
            let tiles = world.tiles.count(|t| {
                t.urban
                    .is_some_and(|u| world.cities.get(u.city as usize).is_some_and(|o| o.name == c.name))
            });
            format!(
                "<b>{}</b> ({}size {:.1}, {} tiles{})",
                c.name,
                if c.home { "home, " } else { "" },
                c.size,
                tiles,
                towns
            )
        })
        .collect();
    let _ = write!(out, "<p>{}</p>", cities.join(" · "));
    let mut routes: Vec<&str> = world.routes.iter().map(|r| r.name.as_str()).collect();
    routes.dedup();
    let mut seen = Vec::new();
    routes.retain(|r| {
        let fresh = !seen.contains(r);
        seen.push(*r);
        fresh
    });
    let pois: Vec<String> = PoiKind::ALL
        .iter()
        .filter_map(|k| {
            let n = world.pois.iter().filter(|p| p.kind == *k).count();
            (n > 0).then(|| format!("{} {}", n, k.label()))
        })
        .collect();
    let regions: Vec<String> = world
        .regions
        .iter()
        .map(|r| {
            let kind = match r.kind {
                RegionKind::Range => "range",
                RegionKind::Bay => "bay",
                RegionKind::Spill => "spill",
                RegionKind::Salt => "salt",
                RegionKind::Flats => "flats",
                RegionKind::Valley => "valley",
            };
            format!("{} ({kind})", r.name)
        })
        .collect();
    let _ = write!(
        out,
        "<p class=note>land {:.0}% · {} rivers · routes: {} · POIs: {} · regions: {}</p>",
        land * 100.0,
        world.rivers.len(),
        routes.join(", "),
        pois.join(", "),
        regions.join(", ")
    );
    out
}

/// The page for `worlds`, each with the time it took to generate.
pub fn world_sheet(worlds: &[(Overworld, Duration)], style: &StylePack) -> String {
    let mut out = String::from("<!doctype html><meta charset=utf-8><title>World sheet</title><style>");
    let _ = write!(
        out,
        "@font-face{{font-family:'StrataFira';font-weight:400;src:url(data:font/ttf;base64,{}) format('truetype')}}\
         @font-face{{font-family:'StrataFira';font-weight:700;src:url(data:font/ttf;base64,{}) format('truetype')}}",
        base64(FIRA_REGULAR),
        base64(FIRA_BOLD)
    );
    out.push_str(
        "body{background:#0d1117;color:#c9d1d9;font-family:-apple-system,system-ui,sans-serif;margin:24px 32px 60px}\
         h1{font-size:22px;margin:0 0 6px}h2{font-size:17px;margin:34px 0 4px}\
         p{max-width:1300px;line-height:1.5;margin:4px 0;font-size:14px}.note{color:#8b949e;font-size:13px}\
         .tm{font-family:'StrataFira',monospace;font-size:12px;line-height:1.25;display:inline-block;\
         background:#04060a;white-space:pre;margin-top:8px;box-shadow:0 0 0 1px #1a2230}\
         .tm .r{height:1.25em}.tm i{display:inline-block;width:.6em;font-style:normal;text-align:center}",
    );
    out.push_str("</style><h1>World sheet</h1>");
    out.push_str(
        "<p class=note>One cell a tile, the atlas look. ⌂ truck stop · ╪ checkpoint · ⬡ relay mast · \
         △ nomad camp · × wreck · ▣ dead arcology · ▦ solar array · ◎ water plant · ◈ test range · ◆ rig.</p>",
    );
    for (world, took) in worlds {
        out.push_str(&summary(world, *took));
        out.push_str(&grid(world, style));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::StylePack;
    use citygen::overworld::{Overworld, WorldParams};

    #[test]
    fn the_sheet_has_a_section_per_world() {
        let style = StylePack::load().unwrap();
        let worlds: Vec<_> = (0..2)
            .map(|seed| {
                (
                    Overworld::generate(WorldParams {
                        seed,
                        ..Default::default()
                    })
                    .unwrap(),
                    std::time::Duration::from_millis(5),
                )
            })
            .collect();
        let html = world_sheet(&worlds, &style);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("seed 0 ·") && html.contains("seed 1 ·"));
        assert!(html.contains(&worlds[1].0.home().name));
        assert!(html.contains("@font-face"));
    }

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
