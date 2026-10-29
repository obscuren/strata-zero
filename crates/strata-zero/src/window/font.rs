//! The window's fonts: a monospace face (and its bold) found on this Mac, fallbacks for
//! characters it lacks, the cell size, and a cache of rasterized glyphs.

use anyhow::{Result, bail};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::path::PathBuf;
use unicode_width::UnicodeWidthChar;

/// The size of one cell in pixels, and where its baseline sits from the cell's top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellMetrics {
    pub w: usize,
    pub h: usize,
    pub baseline: usize,
}

/// A rasterized glyph: coverage `w*h`, placed at `left`, `top` from its cell's top-left.
#[derive(Debug, Clone)]
pub struct Glyph {
    pub left: i32,
    pub top: i32,
    pub w: usize,
    pub h: usize,
    pub alpha: Vec<u8>,
}

/// A face on disk: the file and, for a collection, the face's index in it.
#[derive(Clone)]
struct Face {
    path: PathBuf,
    index: u32,
}

impl Face {
    fn new(path: impl Into<PathBuf>, index: u32) -> Self {
        Face {
            path: path.into(),
            index,
        }
    }

    fn load(&self) -> Option<Font> {
        let bytes = std::fs::read(&self.path).ok()?;
        let settings = FontSettings {
            collection_index: self.index,
            ..FontSettings::default()
        };
        Font::from_bytes(bytes, settings).ok()
    }
}

/// A fallback face, read from disk the first time a character needs it.
enum Fallback {
    Unread(Face),
    Read(Option<Font>),
}

pub struct Fonts {
    regular: Font,
    bold: Option<Font>,
    fallbacks: Vec<Fallback>,
    cache: HashMap<(char, bool, u32), Option<Glyph>>,
}

/// The monospace faces tried in order, each with its bold.
fn families() -> Vec<(Face, Face)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let user = |f: &str| format!("{home}/Library/Fonts/{f}");
    let sf = |f: &str| {
        format!("/System/Applications/Utilities/Terminal.app/Contents/Resources/Fonts/{f}")
    };
    let mut out = Vec::new();
    if let Ok(path) = std::env::var("STRATA_FONT") {
        out.push((Face::new(&path, 0), Face::new(&path, 0)));
    }
    out.push((
        Face::new(user("FiraCodeNerdFontMono-Regular.ttf"), 0),
        Face::new(user("FiraCodeNerdFontMono-Bold.ttf"), 0),
    ));
    out.push((
        Face::new(sf("SF-Mono-Regular.otf"), 0),
        Face::new(sf("SF-Mono-Bold.otf"), 0),
    ));
    out.push((
        Face::new("/System/Library/Fonts/Menlo.ttc", 0),
        Face::new("/System/Library/Fonts/Menlo.ttc", 1),
    ));
    out
}

/// Faces for characters the monospace face lacks, tried in order.
fn fallbacks() -> Vec<Face> {
    vec![
        Face::new("/System/Library/Fonts/Apple Symbols.ttf", 0),
        Face::new("/System/Library/Fonts/Menlo.ttc", 0),
        Face::new("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0),
    ]
}

impl Fonts {
    /// The first monospace face that loads, with its bold when that loads too.
    pub fn load() -> Result<Fonts> {
        for (regular, bold) in families() {
            if let Some(font) = regular.load() {
                return Ok(Fonts {
                    regular: font,
                    bold: bold.load(),
                    fallbacks: fallbacks().into_iter().map(Fallback::Unread).collect(),
                    cache: HashMap::new(),
                });
            }
        }
        bail!("no monospace font found (set STRATA_FONT to a .ttf or .otf)")
    }

    /// The cell for text `px` pixels high: the face's advance by its line height.
    pub fn metrics(&self, px: f32) -> CellMetrics {
        let advance = self.regular.metrics('M', px).advance_width;
        let (ascent, descent, gap) = match self.regular.horizontal_line_metrics(px) {
            Some(l) => (l.ascent, l.descent, l.line_gap),
            None => (px * 0.8, -px * 0.2, 0.0),
        };
        let h = (ascent - descent + gap).ceil().max(1.0) as usize;
        CellMetrics {
            w: advance.round().max(1.0) as usize,
            h,
            baseline: (gap / 2.0 + ascent).round() as usize,
        }
    }

    /// `c` rasterized at `px`, from the bold face when `bold` and it has `c`, else the
    /// regular face, else the first fallback that has it; `None` when no face does. A
    /// fallback glyph wider than its cell (or two, for a wide character) is scaled to fit
    /// and centred.
    pub fn glyph(&mut self, c: char, bold: bool, px: f32) -> Option<&Glyph> {
        let key = (c, bold, px.to_bits());
        if !self.cache.contains_key(&key) {
            let glyph = self.rasterize(c, bold, px);
            self.cache.insert(key, glyph);
        }
        self.cache.get(&key).and_then(Option::as_ref)
    }

    /// Forgets every rasterized glyph (the cache keys on size, so this only saves memory).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    fn rasterize(&mut self, c: char, bold: bool, px: f32) -> Option<Glyph> {
        let cell = self.metrics(px);
        let span = c.width().unwrap_or(1).clamp(1, 2);
        let room = (cell.w * span) as f32;
        let font = match &self.bold {
            Some(b) if bold && b.has_glyph(c) => b,
            _ if self.regular.has_glyph(c) => &self.regular,
            _ => self.fallback_for(c)?,
        };
        let advance = font.metrics(c, px).advance_width;
        let fit = if advance > room {
            px * room / advance
        } else {
            px
        };
        let (m, alpha) = font.rasterize(c, fit);
        let advance = font.metrics(c, fit).advance_width;
        let centre = ((room - advance) / 2.0).max(0.0).round() as i32;
        Some(Glyph {
            left: m.xmin + centre,
            top: cell.baseline as i32 - (m.ymin + m.height as i32),
            w: m.width,
            h: m.height,
            alpha,
        })
    }

    fn fallback_for(&mut self, c: char) -> Option<&Font> {
        let i = (0..self.fallbacks.len()).find(|&i| {
            if let Fallback::Unread(face) = &self.fallbacks[i] {
                self.fallbacks[i] = Fallback::Read(face.load());
            }
            matches!(&self.fallbacks[i], Fallback::Read(Some(f)) if f.has_glyph(c))
        })?;
        match &self.fallbacks[i] {
            Fallback::Read(Some(f)) => Some(f),
            _ => None,
        }
    }
}
