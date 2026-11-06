//! The booklet: the chapters laid out on pages of a fixed size, so a page number means
//! the same page in every terminal and window. Page 1 is the contents; every chapter
//! opens on a new page; spreads pair pages 1-2, 3-4 and on.

use super::layout::{self, Row};
use super::{Chapter, chapters};
use ratatui::text::Line;
use std::sync::OnceLock;

/// A page's text: columns and rows.
pub const TEXT_W: usize = 40;
pub const TEXT_H: usize = 22;

/// A page of a chapter.
pub struct Page {
    pub chapter: usize,
    /// The chapter's first page, which carries its number and title.
    pub opens: bool,
    pub rows: Vec<Line<'static>>,
}

pub struct Book {
    /// Page `n` is `pages[n - 2]`: page 1 is the contents.
    pages: Vec<Page>,
    /// The page each chapter opens on.
    pub starts: Vec<usize>,
}

impl Book {
    pub fn new(chapters: &[Chapter]) -> Book {
        let mut pages = Vec::new();
        let mut starts = Vec::new();
        for (c, ch) in chapters.iter().enumerate() {
            starts.push(pages.len() + 2);
            let laid = paginate(layout::rows(&ch.blocks, TEXT_W));
            for (k, rows) in laid.into_iter().enumerate() {
                pages.push(Page {
                    chapter: c,
                    opens: k == 0,
                    rows,
                });
            }
        }
        Book { pages, starts }
    }

    /// The last page with words on it.
    pub fn last(&self) -> usize {
        self.pages.len() + 1
    }

    /// Page `n` of a chapter: none for the contents or past the end.
    pub fn page(&self, n: usize) -> Option<&Page> {
        n.checked_sub(2).and_then(|i| self.pages.get(i))
    }

    /// The spread showing page `n`: pages 1 and 2 are spread 1.
    pub fn spread_of(n: usize) -> usize {
        n.div_ceil(2).max(1)
    }

    /// The last spread.
    pub fn spreads(&self) -> usize {
        Self::spread_of(self.last())
    }
}

/// The manual's booklet, laid out once.
pub fn book() -> &'static Book {
    static BOOK: OnceLock<Book> = OnceLock::new();
    BOOK.get_or_init(|| Book::new(chapters()))
}

/// A chapter's rows cut into pages: no page starts on a blank row, and a run of rows
/// that keep (a heading, a table's head) goes to the next page with the row after it
/// rather than end a page.
fn paginate(rows: Vec<Row>) -> Vec<Vec<Line<'static>>> {
    let mut pages = Vec::new();
    let mut cur: Vec<Line<'static>> = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let row = &rows[i];
        if cur.is_empty() && row.line.width() == 0 {
            i += 1;
            continue;
        }
        let run = rows[i..].iter().take_while(|r| r.keep).count();
        let need = if run > 0 {
            (run + 1).min(rows.len() - i)
        } else {
            1
        };
        if !cur.is_empty() && cur.len() + need > TEXT_H {
            pages.push(std::mem::take(&mut cur));
            continue;
        }
        cur.push(row.line.clone());
        i += 1;
        if cur.len() == TEXT_H {
            pages.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() || pages.is_empty() {
        pages.push(cur);
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manual::markdown::parse;

    fn chapter(src: &str) -> Chapter {
        let mut blocks = parse(src).unwrap();
        blocks.remove(0);
        Chapter {
            slug: "t",
            title: "T".into(),
            blocks,
        }
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn chapters_open_on_new_pages_numbered_after_the_contents() {
        let long: String = (0..60)
            .map(|i| format!("Line {i} of the first.\n\n"))
            .collect();
        let a = chapter(&format!("# A\n\n{long}"));
        let b = chapter("# B\n\nShort.\n");
        let book = Book::new(&[a, b]);
        assert_eq!(book.starts[0], 2, "the first chapter opens on page 2");
        assert!(
            book.starts[1] > 3,
            "the long one takes pages: {:?}",
            book.starts
        );
        let opens = book.page(book.starts[1]).unwrap();
        assert!(opens.opens && opens.chapter == 1);
        assert!(!book.page(3).unwrap().opens);
        assert!(book.page(1).is_none() && book.page(book.last() + 1).is_none());
        for n in 2..=book.last() {
            let p = book.page(n).unwrap();
            assert!(p.rows.len() <= TEXT_H, "page {n} has {} rows", p.rows.len());
            assert!(
                p.rows.first().is_none_or(|l| l.width() > 0),
                "page {n} starts blank"
            );
            assert!(
                p.rows.iter().all(|l| l.width() <= TEXT_W),
                "page {n} too wide"
            );
        }
    }

    #[test]
    fn a_heading_never_ends_a_page() {
        // 20 rows of text, then a heading: it would land on the last row or two
        let text_rows: String = (0..10).map(|i| format!("Row {i}.\n\n")).collect();
        let c = chapter(&format!(
            "# T\n\n{text_rows}## Next\n\nUnder it.\n\n| Key | What |\n|---|---|\n| `a` | b |\n"
        ));
        let book = Book::new(&[c]);
        for n in 2..=book.last() {
            let rows = &book.page(n).unwrap().rows;
            let last = rows.last().map(text).unwrap_or_default();
            assert!(
                !last.contains("NEXT") && !last.contains("KEY"),
                "page {n} ends {last:?}"
            );
        }
        assert!(
            (2..=book.last()).any(|n| book.page(n).unwrap().rows.iter().any(|l| text(l) == "NEXT")),
            "the heading is somewhere"
        );
    }

    #[test]
    fn spreads_pair_the_pages() {
        assert_eq!(Book::spread_of(1), 1);
        assert_eq!(Book::spread_of(2), 1);
        assert_eq!(Book::spread_of(3), 2);
        assert_eq!(Book::spread_of(4), 2);
        assert_eq!(Book::spread_of(5), 3);
    }

    #[test]
    fn the_manual_lays_out() {
        let book = book();
        assert_eq!(book.starts.len(), chapters().len());
        assert!(book.last() > chapters().len(), "{} pages", book.last());
        // the contents fit on page 1 with room for its head
        assert!(chapters().len() + 4 <= TEXT_H);
    }
}
