//! The window's fonts: a monospace face and its bold (Fira Code, built in, unless the
//! machine has a preferred one), fallbacks on the machine for characters it lacks, the
//! cell size, and a cache of rasterized glyphs.

use super::platform::{Env, Platform, process_env, set};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthChar;

/// Fira Code 6.2 (SIL Open Font License, `fonts/OFL.txt`), built in so the window draws
/// the same on every machine.
const FIRA_REGULAR: &[u8] = include_bytes!("../../fonts/FiraCode-Regular.ttf");
const FIRA_BOLD: &[u8] = include_bytes!("../../fonts/FiraCode-Bold.ttf");

/// Font files Linux fallbacks are looked for by, in order: where they live differs by
/// distribution.
const LINUX_FALLBACKS: &[&str] = &[
    "DejaVuSans.ttf",
    "DejaVuSansMono.ttf",
    "NotoSansSymbols2-Regular.ttf",
    "NotoSansMath-Regular.ttf",
    "NotoSans-Regular.ttf",
    "NotoSansLisu-Regular.ttf",
    "NotoSansCanadianAboriginal-Regular.ttf",
    "NotoSansCJK-Regular.ttc",
    "NotoSansCJKjp-Regular.otf",
];

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

/// A face: a file on disk (with, for a collection, the face's index in it), or bytes
/// built into the binary.
#[derive(Clone)]
enum Face {
    File { path: PathBuf, index: u32 },
    Builtin(&'static [u8]),
}

impl Face {
    fn file(path: impl Into<PathBuf>, index: u32) -> Self {
        Face::File {
            path: path.into(),
            index,
        }
    }

    /// The file, for a face on disk.
    #[cfg(test)]
    fn path(&self) -> Option<&Path> {
        match self {
            Face::File { path, .. } => Some(path),
            Face::Builtin(_) => None,
        }
    }

    fn load(&self) -> Option<Font> {
        match self {
            Face::File { path, index } => {
                let bytes = std::fs::read(path).ok()?;
                let settings = FontSettings {
                    collection_index: *index,
                    ..FontSettings::default()
                };
                Font::from_bytes(bytes, settings).ok()
            }
            Face::Builtin(bytes) => Font::from_bytes(*bytes, FontSettings::default()).ok(),
        }
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

/// The monospace faces tried in order, each with its bold: `STRATA_FONT`, the
/// platform's preference, and the built-in Fira Code, which always loads.
fn families(p: Platform, env: Env) -> Vec<(Face, Face)> {
    let mut out = Vec::new();
    if let Some(path) = set(env, "STRATA_FONT") {
        out.push((Face::file(&path, 0), Face::file(&path, 0)));
    }
    if p == Platform::Mac {
        // The Nerd Font build of Fira Code, where it is installed: the game's own face,
        // as the maintainer's Mac has always drawn it.
        let fonts = PathBuf::from(env("HOME").unwrap_or_default()).join("Library/Fonts");
        out.push((
            Face::file(fonts.join("FiraCodeNerdFontMono-Regular.ttf"), 0),
            Face::file(fonts.join("FiraCodeNerdFontMono-Bold.ttf"), 0),
        ));
    }
    out.push((Face::Builtin(FIRA_REGULAR), Face::Builtin(FIRA_BOLD)));
    out
}

/// Faces for characters the monospace face lacks, tried in order.
fn fallbacks(p: Platform, env: Env) -> Vec<Face> {
    match p {
        Platform::Mac => vec![
            Face::file("/System/Library/Fonts/Apple Symbols.ttf", 0),
            Face::file("/System/Library/Fonts/Menlo.ttc", 0),
            Face::file("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0),
            // ꝺ ꓘ ꓤ ꓭ Ԁ, and ᗡ: none of the above has them.
            Face::file("/System/Library/Fonts/Geneva.ttf", 0),
            Face::file("/System/Library/Fonts/Supplemental/EuphemiaCAS.ttc", 0),
        ],
        Platform::Windows => {
            let dir = PathBuf::from(set(env, "WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
                .join("Fonts");
            [
                ("seguisym.ttf", 0), // Segoe UI Symbol: shapes, arrows, dingbats, braille
                ("segoeui.ttf", 0),  // Segoe UI: IPA, Latin extensions, Lisu
                ("consola.ttf", 0),
                ("cambria.ttc", 1), // Cambria Math
                ("gadugi.ttf", 0),  // Canadian syllabics
                ("YuGothR.ttc", 0), // halfwidth katakana, from whichever Japanese face is there
                ("msgothic.ttc", 0),
                ("meiryo.ttc", 0),
                ("arial.ttf", 0),
            ]
            .into_iter()
            .map(|(f, index)| Face::file(dir.join(f), index))
            .collect()
        }
        Platform::Linux => find_font_files(&linux_font_dirs(env), LINUX_FALLBACKS)
            .into_iter()
            .map(|path| Face::file(path, 0))
            .collect(),
    }
}

/// The folders fonts are installed in on Linux.
fn linux_font_dirs(env: Env) -> Vec<PathBuf> {
    let home = PathBuf::from(env("HOME").unwrap_or_default());
    let data = set(env, "XDG_DATA_HOME").map_or_else(|| home.join(".local/share"), PathBuf::from);
    vec![
        "/usr/share/fonts".into(),
        "/usr/local/share/fonts".into(),
        data.join("fonts"),
        home.join(".fonts"),
    ]
}

/// The first file under `dirs` (four folders deep at most) named each of `names`, in the
/// order of `names`; names no file has are left out.
fn find_font_files(dirs: &[PathBuf], names: &[&str]) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, found: &mut HashMap<String, PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            // Linked folders aren't followed, so a loop of links can't trap the walk.
            if kind.is_dir() {
                if depth > 0 {
                    walk(&entry.path(), depth - 1, found);
                }
            } else if let Some(name) = entry.file_name().to_str() {
                found
                    .entry(name.to_string())
                    .or_insert_with(|| entry.path());
            }
        }
    }
    let mut found = HashMap::new();
    for dir in dirs {
        walk(dir, 4, &mut found);
    }
    names.iter().filter_map(|n| found.remove(*n)).collect()
}

impl Fonts {
    /// The fonts for this machine.
    pub fn load() -> Fonts {
        Fonts::load_for(Platform::NATIVE, &process_env)
    }

    /// The fonts `p` draws with, its files found through `env`: the first monospace face
    /// that loads, with its bold when that loads too.
    pub fn load_for(p: Platform, env: Env) -> Fonts {
        let (regular, bold) = families(p, env)
            .into_iter()
            .find_map(|(regular, bold)| Some((regular.load()?, bold.load())))
            .expect("the built-in Fira Code loads");
        Fonts {
            regular,
            bold,
            fallbacks: fallbacks(p, env)
                .into_iter()
                .map(Fallback::Unread)
                .collect(),
            cache: HashMap::new(),
        }
    }

    /// Whether some face draws `c`: the regular, the bold or a fallback.
    #[cfg(test)]
    pub fn has(&mut self, c: char) -> bool {
        self.regular.has_glyph(c)
            || self.bold.as_ref().is_some_and(|b| b.has_glyph(c))
            || self.fallback_for(c).is_some()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::platform::env_of;
    use std::collections::BTreeSet;

    fn paths(faces: &[Face]) -> Vec<Option<&Path>> {
        faces.iter().map(Face::path).collect()
    }

    #[test]
    fn builtin_face_loads() {
        let mut fonts = Fonts::load_for(Platform::Windows, &env_of(&[]));
        assert!(fonts.has('M'));
        assert!(!fonts.regular.has_glyph('▸'), "Fira Code has no ▸");
        let m = fonts.metrics(15.0);
        assert!(m.w > 0 && m.h > m.w, "{m:?}");
    }

    #[test]
    fn mac_families() {
        let env = env_of(&[("HOME", "/Users/x")]);
        let mac = families(Platform::Mac, &env);
        let regulars: Vec<Face> = mac.iter().map(|(r, _)| r.clone()).collect();
        let fonts = Path::new("/Users/x/Library/Fonts");
        let nerd = fonts.join("FiraCodeNerdFontMono-Regular.ttf");
        assert_eq!(paths(&regulars), [Some(nerd.as_path()), None]);
        let bold = fonts.join("FiraCodeNerdFontMono-Bold.ttf");
        assert_eq!(mac[0].1.path(), Some(bold.as_path()));

        let env = env_of(&[("HOME", "/Users/x"), ("STRATA_FONT", "/f.ttf")]);
        assert_eq!(
            families(Platform::Mac, &env)[0].0.path(),
            Some(Path::new("/f.ttf"))
        );
    }

    #[test]
    fn pc_families_are_the_builtin_face() {
        for p in [Platform::Windows, Platform::Linux] {
            let families = families(p, &env_of(&[("HOME", "/h")]));
            assert_eq!(families.len(), 1);
            assert!(matches!(families[0], (Face::Builtin(_), Face::Builtin(_))));
        }
    }

    #[test]
    fn mac_fallbacks_add_geneva_and_euphemia() {
        let faces = fallbacks(Platform::Mac, &env_of(&[]));
        let tail: Vec<_> = paths(&faces).into_iter().rev().take(2).collect();
        assert_eq!(
            tail,
            [
                Some(Path::new(
                    "/System/Library/Fonts/Supplemental/EuphemiaCAS.ttc"
                )),
                Some(Path::new("/System/Library/Fonts/Geneva.ttf")),
            ]
        );
    }

    #[test]
    fn windows_fallbacks_read_the_windows_font_folder() {
        for (env, root) in [
            (env_of(&[("WINDIR", "D:\\Win")]), "D:\\Win"),
            (env_of(&[]), "C:\\Windows"),
        ] {
            let dir = PathBuf::from(root).join("Fonts");
            let faces = fallbacks(Platform::Windows, &env);
            assert!(faces.len() > 3);
            for p in paths(&faces) {
                assert_eq!(p.and_then(Path::parent), Some(dir.as_path()));
            }
            assert_eq!(
                faces[0].path().and_then(Path::file_name),
                Some("seguisym.ttf".as_ref())
            );
        }
    }

    #[test]
    fn linux_font_index_finds_by_name() {
        let root = std::env::temp_dir().join(format!("strata-fonts-{}", std::process::id()));
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(a.join("x")).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        for f in [
            a.join("x/DejaVuSans.ttf"),
            b.join("NotoSansCJK-Regular.ttc"),
            b.join("other.ttf"),
        ] {
            std::fs::write(f, b"").unwrap();
        }
        let found = find_font_files(
            &[a.clone(), b.clone(), root.join("missing")],
            &["NotoSansCJK-Regular.ttc", "DejaVuSans.ttf", "Nope.ttf"],
        );
        assert_eq!(
            found,
            [
                b.join("NotoSansCJK-Regular.ttc"),
                a.join("x/DejaVuSans.ttf")
            ]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn linux_font_dirs_follow_xdg() {
        let dirs = linux_font_dirs(&env_of(&[("HOME", "/h"), ("XDG_DATA_HOME", "/d")]));
        assert!(dirs.contains(&PathBuf::from("/d/fonts")));
        let dirs = linux_font_dirs(&env_of(&[("HOME", "/h"), ("XDG_DATA_HOME", "")]));
        assert!(dirs.contains(&PathBuf::from("/h/.local/share/fonts")));
        assert!(dirs.contains(&PathBuf::from("/usr/share/fonts")));
    }

    #[test]
    fn missing_fallbacks_are_skipped() {
        let mut fonts =
            Fonts::load_for(Platform::Windows, &env_of(&[("WINDIR", "/nowhere/strata")]));
        assert!(fonts.has('M'));
        assert!(!fonts.has('\u{10FFFD}'));
        assert!(fonts.glyph('▸', false, 15.0).is_none());
    }

    #[test]
    fn metrics_at_fractional_sizes() {
        let fonts = Fonts::load_for(Platform::Linux, &env_of(&[]));
        for px in [18.75, 22.5, 11.25] {
            let m = fonts.metrics(px);
            assert!(m.w >= 1 && m.h >= 1 && m.baseline < m.h, "{px}: {m:?}");
        }
    }

    /// Every character the game can draw (from the content and the strings in the
    /// sources, less ASCII and the box and block characters drawn as shapes) has a glyph
    /// in this machine's fonts. Asserted on macOS and Windows, and on Linux with
    /// `STRATA_FONT_CHECK=1` (where the common desktop fonts are installed); elsewhere
    /// it lists what is missing.
    #[test]
    fn every_drawn_character_has_a_glyph() {
        fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files(&path, ext, out);
                } else if path.extension().is_some_and(|e| e == ext) {
                    out.push(path);
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut found = Vec::new();
        files(&root.join("src"), "rs", &mut found);
        files(&root.join("../../flavor"), "ron", &mut found);
        let shape = |c: char| super::super::shapes::shape(c, 8, 16).is_some();
        let mut chars = BTreeSet::new();
        for file in found {
            let text = std::fs::read_to_string(&file).unwrap();
            for line in text.lines() {
                if !line.trim_start().starts_with("//") {
                    chars.extend(line.chars().filter(|&c| !c.is_ascii() && !shape(c)));
                }
            }
        }
        assert!(
            chars.len() > 100,
            "the scan found {} characters",
            chars.len()
        );
        let mut fonts = Fonts::load();
        let missing: String = chars.into_iter().filter(|&c| !fonts.has(c)).collect();
        let strict = cfg!(any(target_os = "macos", windows))
            || std::env::var_os("STRATA_FONT_CHECK").is_some_and(|v| v == "1");
        if strict {
            assert!(missing.is_empty(), "no font draws: {missing}");
        } else if !missing.is_empty() {
            eprintln!("no font draws: {missing}");
        }
    }
}
