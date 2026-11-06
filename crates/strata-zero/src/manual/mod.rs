//! The player's manual: the chapters in `manual/` at the repo's root, compiled in,
//! written in the manual's own Markdown (`manual/README.md`), and the booklet they make.

pub mod book;
pub mod desk;
pub mod layout;
pub mod markdown;

use markdown::{Block, Span};
use std::sync::OnceLock;

macro_rules! chapter {
    ($name:literal) => {
        ($name, include_str!(concat!("../../../../manual/", $name)))
    };
}

/// The chapters' files, in order: `manual/NN-slug.md`.
const FILES: &[(&str, &str)] = &[
    chapter!("01-getting-started.md"),
    chapter!("02-your-runner.md"),
    chapter!("03-the-screen.md"),
    chapter!("04-controls.md"),
    chapter!("05-the-deck.md"),
    chapter!("06-the-city-and-the-world.md"),
    chapter!("07-turns-and-fights.md"),
    chapter!("08-the-net.md"),
    chapter!("09-the-rig-and-helix.md"),
    chapter!("10-chrome-and-drones.md"),
    chapter!("11-fixers-jobs-and-the-story.md"),
    chapter!("12-time-and-rest.md"),
    chapter!("13-saving-and-dying.md"),
];

/// A chapter: its slug (the file's name after the number), its title, its blocks after
/// the title.
pub struct Chapter {
    pub slug: &'static str,
    pub title: String,
    pub blocks: Vec<Block>,
}

/// Every chapter, in order, parsed once.
pub fn chapters() -> &'static [Chapter] {
    static CHAPTERS: OnceLock<Vec<Chapter>> = OnceLock::new();
    CHAPTERS.get_or_init(|| {
        FILES
            .iter()
            .map(|&(name, src)| chapter(name, src))
            .collect()
    })
}

/// The chapter with `slug`.
pub fn find(slug: &str) -> Option<usize> {
    chapters().iter().position(|c| c.slug == slug)
}

/// The chapter in file `name`. One that doesn't parse (the tests see that none ships)
/// says why on its page rather than taking the game down.
fn chapter(name: &'static str, src: &str) -> Chapter {
    let slug = name
        .trim_end_matches(".md")
        .split_once('-')
        .map_or(name, |(_, s)| s);
    match markdown::parse(src) {
        Ok(mut blocks) => {
            let title = match blocks.remove(0) {
                Block::Title(spans) => markdown::plain(&spans),
                _ => slug.to_string(),
            };
            Chapter {
                slug,
                title,
                blocks,
            }
        }
        Err((line, why)) => Chapter {
            slug,
            title: slug.to_string(),
            blocks: vec![Block::Para(vec![Span::Text(format!(
                "{name} line {line}: {why}"
            ))])],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn folder() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../manual")
    }

    #[test]
    fn every_chapter_parses() {
        for (name, src) in FILES {
            if let Err((line, why)) = markdown::parse(src) {
                panic!("manual/{name} line {line}: {why}");
            }
        }
        assert!(chapters().iter().all(|c| !c.title.is_empty()));
    }

    #[test]
    fn the_list_in_code_is_the_folder() {
        let on_disk: Vec<String> = std::fs::read_dir(folder())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".md") && n != "README.md")
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let listed: Vec<String> = FILES.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(
            listed, on_disk,
            "FILES in manual/mod.rs and the manual/ folder"
        );
    }

    #[test]
    fn chapter_files_are_named_by_number_and_slug() {
        for (k, (name, _)) in (1..).zip(FILES) {
            let (num, rest) = name.split_once('-').unwrap();
            assert_eq!(num, format!("{k:02}"), "{name}: numbered in order");
            let slug = rest.strip_suffix(".md").unwrap();
            assert!(
                !slug.is_empty()
                    && slug.split('-').all(|w| !w.is_empty()
                        && w.chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())),
                "{name}: the slug is lower-case words joined by hyphens"
            );
        }
        // the README describes the format and is no chapter
        assert!(folder().join("README.md").exists());
        assert!(
            markdown::parse(&std::fs::read_to_string(folder().join("README.md")).unwrap()).is_ok()
        );
    }

    /// The key cells of the table under `heading` in chapter `slug`.
    fn table_keys(slug: &str, heading: &str) -> BTreeSet<String> {
        let ch = &chapters()[find(slug).unwrap()];
        let at = ch
            .blocks
            .iter()
            .position(|b| matches!(b, Block::Heading(_, s) if markdown::plain(s) == heading))
            .unwrap_or_else(|| panic!("no {heading} in {slug}"));
        let rows = ch.blocks[at..]
            .iter()
            .find_map(|b| match b {
                Block::Table { rows, .. } => Some(rows),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no table under {heading}"));
        rows.iter().map(|r| markdown::plain(&r[0])).collect()
    }

    #[test]
    fn the_on_foot_table_is_the_key_list() {
        let help: BTreeSet<String> = crate::screens::help::WALK
            .iter()
            .filter(|(k, _)| !crate::screens::help::DEV_KEYS.contains(k))
            .map(|(k, _)| k.to_string())
            .collect();
        assert_eq!(
            table_keys("controls", "On foot"),
            help,
            "the Controls chapter's On foot table and the ? key list on foot"
        );
    }
}
