//! A chapter's blocks laid out as rows for a page `width` cells wide: words wrapped,
//! list items hanging under their marks, tables in columns whose last one wraps.

use super::markdown::{Block, Span};
use crate::ui::chrome::{CYAN, DIM, FAINT, RULE, TEXT};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span as Run};

/// A space inside `code`, kept from breaking the line until it has to.
const NBSP: char = '\u{a0}';

/// The gap between a table's columns.
const GAP: usize = 2;

/// A styled character.
type Cell = (char, Style);

/// A laid-out row of a page: its line, and whether it must not end a page (a heading,
/// a table's head) because what follows belongs to it.
pub struct Row {
    pub line: Line<'static>,
    pub keep: bool,
}

/// `blocks` as rows no wider than `width`, a blank row between blocks (none under a
/// heading).
pub fn rows(blocks: &[Block], width: usize) -> Vec<Row> {
    let width = width.max(8);
    let text = Style::default().fg(TEXT);
    let head = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
    let mut out: Vec<Row> = Vec::new();
    let mut after_heading = true;
    for block in blocks {
        if !out.is_empty() && !after_heading {
            out.push(Row {
                line: Line::default(),
                keep: false,
            });
        }
        after_heading = false;
        match block {
            Block::Title(spans) => {
                push(&mut out, wrap(&upper(spans), width, head, &[], 0), true);
                after_heading = true;
            }
            Block::Heading(level, spans) => {
                let lines = if *level == 2 {
                    wrap(&upper(spans), width, head, &[], 0)
                } else {
                    wrap(spans, width, text.add_modifier(Modifier::BOLD), &[], 0)
                };
                push(&mut out, lines, true);
                after_heading = true;
            }
            Block::Para(spans) => push(&mut out, wrap(spans, width, text, &[], 0), false),
            Block::List(items) => {
                for item in items {
                    let mark = [Run::styled(" · ", Style::default().fg(CYAN))];
                    push(&mut out, wrap(item, width, text, &mark, 3), false);
                }
            }
            Block::Steps(items) => {
                for (n, item) in (1..).zip(items) {
                    let mark = [Run::styled(format!("{n:>2}. "), Style::default().fg(CYAN))];
                    push(&mut out, wrap(item, width, text, &mark, 4), false);
                }
            }
            Block::Tip(spans) => {
                // the bar runs down the whole tip
                let bar = || Run::styled(" ▌ ", Style::default().fg(CYAN));
                let lines = wrap(spans, width, Style::default().fg(DIM), &[bar()], 3)
                    .into_iter()
                    .enumerate()
                    .map(|(i, line)| {
                        if i == 0 {
                            return line;
                        }
                        let mut runs = vec![bar()];
                        runs.extend(line.spans.into_iter().skip(1));
                        Line::from(runs)
                    })
                    .collect();
                push(&mut out, lines, false);
            }
            Block::Table { head, rows } => {
                let (top, body) = table(head, rows, width);
                push(&mut out, top, true);
                // a row's lines stay on one page: all but its last keep
                for lines in body {
                    let last = lines.len().saturating_sub(1);
                    out.extend(lines.into_iter().enumerate().map(|(i, line)| Row {
                        line,
                        keep: i < last,
                    }));
                }
            }
        }
    }
    out
}

fn push(out: &mut Vec<Row>, lines: Vec<Line<'static>>, keep: bool) {
    out.extend(lines.into_iter().map(|line| Row { line, keep }));
}

/// `spans` in capitals, as the game writes its headings.
fn upper(spans: &[Span]) -> Vec<Span> {
    spans
        .iter()
        .map(|s| match s {
            Span::Text(t) => Span::Text(t.to_uppercase()),
            Span::Bold(t) => Span::Bold(t.to_uppercase()),
            Span::Code(t) => Span::Code(t.clone()),
        })
        .collect()
}

fn plain_width(spans: &[Span]) -> usize {
    super::markdown::plain(spans).chars().count()
}

/// `spans` as words of styled cells; a space in `code` doesn't end its word.
fn words(spans: &[Span], base: Style) -> Vec<Vec<Cell>> {
    let code = Style::default().fg(CYAN);
    let mut out = Vec::new();
    let mut cur: Vec<Cell> = Vec::new();
    for span in spans {
        let (text, style, keep) = match span {
            Span::Text(t) => (t, base, false),
            Span::Bold(t) => (t, base.add_modifier(Modifier::BOLD), false),
            Span::Code(t) => (t, code, true),
        };
        for ch in text.chars() {
            if ch == ' ' && !keep {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            } else {
                cur.push((if ch == ' ' { NBSP } else { ch }, style));
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `spans` wrapped to `width`: the first line after `mark`, the rest indented `hang`.
fn wrap(
    spans: &[Span],
    width: usize,
    base: Style,
    mark: &[Run<'static>],
    hang: usize,
) -> Vec<Line<'static>> {
    let rows = wrap_cells(spans, width.saturating_sub(hang).max(1), base);
    rows.into_iter()
        .enumerate()
        .map(|(i, row)| {
            let mut runs: Vec<Run<'static>> = if i == 0 {
                mark.to_vec()
            } else if hang > 0 {
                vec![Run::raw(" ".repeat(hang))]
            } else {
                Vec::new()
            };
            runs.extend(runs_of(&row));
            Line::from(runs)
        })
        .collect()
}

/// `spans` as rows of cells no wider than `width`; a word longer than a row is broken,
/// at a space inside it if it has one.
fn wrap_cells(spans: &[Span], width: usize, base: Style) -> Vec<Vec<Cell>> {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut row: Vec<Cell> = Vec::new();
    for mut word in words(spans, base) {
        loop {
            let sep = usize::from(!row.is_empty());
            if row.len() + sep + word.len() <= width {
                if sep == 1 {
                    row.push((' ', base));
                }
                row.append(&mut word);
                break;
            }
            if !row.is_empty() {
                rows.push(std::mem::take(&mut row));
                continue;
            }
            // longer than a whole row: break it at its last space that fits, else hard
            let cut = word[..width]
                .iter()
                .rposition(|(c, _)| *c == NBSP)
                .filter(|&k| k > 0)
                .unwrap_or(width);
            let mut rest = word.split_off(cut);
            if rest.first().is_some_and(|(c, _)| *c == NBSP) {
                rest.remove(0);
            }
            rows.push(word);
            word = rest;
            if word.is_empty() {
                break;
            }
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows
}

/// A row of cells as runs of one style each, code's kept spaces as spaces.
fn runs_of(row: &[Cell]) -> Vec<Run<'static>> {
    let mut runs: Vec<Run<'static>> = Vec::new();
    let mut text = String::new();
    let mut style: Option<Style> = None;
    for &(ch, st) in row {
        if let Some(s) = style
            && s != st
        {
            runs.push(Run::styled(std::mem::take(&mut text), s));
        }
        style = Some(st);
        text.push(if ch == NBSP { ' ' } else { ch });
    }
    if let Some(s) = style {
        runs.push(Run::styled(text, s));
    }
    runs
}

/// A table as its head (faint capitals over a rule) and its body; every column but the
/// last as wide as its widest cell (within a share of the page), the last wrapping.
fn table(
    head: &[Vec<Span>],
    rows: &[Vec<Vec<Span>>],
    width: usize,
) -> (Vec<Line<'static>>, Vec<Vec<Line<'static>>>) {
    let cols = head.len();
    let text = Style::default().fg(TEXT);
    let natural = |c: usize| {
        rows.iter()
            .map(|r| plain_width(&r[c]))
            .chain([plain_width(&head[c])])
            .max()
            .unwrap_or(1)
    };
    // the columns before the last share at most half the page between them
    let cap = (width / 2).saturating_sub(GAP * (cols - 1)) / (cols - 1).max(1);
    let mut widths: Vec<usize> = (0..cols - 1)
        .map(|c| natural(c).clamp(1, cap.max(4)))
        .collect();
    let used: usize = widths.iter().sum::<usize>() + GAP * (cols - 1) + 1;
    widths.push(width.saturating_sub(used).max(4));
    let line_of = |cells: Vec<Vec<Vec<Cell>>>| -> Vec<Line<'static>> {
        let height = cells.iter().map(Vec::len).max().unwrap_or(1);
        (0..height)
            .map(|y| {
                let mut runs = vec![Run::raw(" ")];
                for (c, cell) in cells.iter().enumerate() {
                    let row = cell.get(y).cloned().unwrap_or_default();
                    let pad = widths[c].saturating_sub(row.len());
                    runs.extend(runs_of(&row));
                    if c + 1 < cols {
                        runs.push(Run::raw(" ".repeat(pad + GAP)));
                    }
                }
                Line::from(runs)
            })
            .collect()
    };
    let faint = Style::default().fg(FAINT).add_modifier(Modifier::BOLD);
    let mut top = line_of(
        head.iter()
            .enumerate()
            .map(|(c, h)| wrap_cells(&upper(h), widths[c], faint))
            .collect(),
    );
    let ruled = (widths.iter().sum::<usize>() + GAP * (cols - 1)).min(width.saturating_sub(1));
    top.push(Line::from(vec![
        Run::raw(" "),
        Run::styled("─".repeat(ruled), Style::default().fg(RULE)),
    ]));
    let body = rows
        .iter()
        .map(|row| {
            line_of(
                row.iter()
                    .enumerate()
                    .map(|(c, cell)| wrap_cells(cell, widths[c], text))
                    .collect(),
            )
        })
        .collect();
    (top, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manual::markdown::parse;

    fn text(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|r| r.line.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn long_lines_and_wide_tables_wrap() {
        let src = "# A title that is long\n\n\
                   A paragraph with a good many words that cannot fit on one row of thirty cells.\n\n\
                   - a list item that also runs on for longer than the page is wide\n\n\
                   | Key | What it does |\n|---|---|\n\
                   | `hjkl yubn ↑↓←→` | move one step, or attack whoever stands there |\n\n\
                   > A tip that wraps under its own bar all the way down.\n";
        let rows = text(&rows(&parse(src).unwrap(), 30));
        for row in &rows {
            assert!(row.chars().count() <= 30, "{row:?} is wider than 30");
        }
        let all = rows.join(" ");
        for word in [
            "paragraph",
            "cells.",
            "wide",
            "attack",
            "whoever",
            "there",
            "down.",
        ] {
            assert!(all.contains(word), "{word} lost: {rows:#?}");
        }
        // the item's next row hangs under its text, the tip's under its bar
        let item = rows
            .iter()
            .position(|r| r.starts_with(" · a list"))
            .unwrap();
        assert!(rows[item + 1].starts_with("   ") && !rows[item + 1].starts_with("    "));
        assert!(rows.iter().filter(|r| r.starts_with(" ▌ ")).count() >= 2);
    }

    #[test]
    fn a_table_keeps_its_columns_and_its_head_with_its_rows() {
        let src = "# T\n\n| Key | What |\n|---|---|\n| `tab` | the deck |\n| `esc` | the menu |\n";
        let laid = rows(&parse(src).unwrap(), 60);
        let rows = text(&laid);
        let deck = rows.iter().find(|r| r.contains("the deck")).unwrap();
        let menu = rows.iter().find(|r| r.contains("the menu")).unwrap();
        assert_eq!(deck.find("the deck"), menu.find("the menu"), "{rows:#?}");
        let head = rows
            .iter()
            .position(|r| r.contains("KEY") && r.contains("WHAT"))
            .unwrap();
        assert!(
            laid[head].keep && laid[head + 1].keep,
            "the head and its rule keep"
        );
        assert!(!laid[head + 2].keep, "the rows may end a page");
        // a row that wraps keeps its lines together
        let src = "# T\n\n| Key | What |\n|---|---|\n| `a` | a description long enough to wrap |\n";
        let laid = super::rows(&parse(src).unwrap(), 24);
        let first = text(&laid).iter().position(|r| r.contains('a')).unwrap();
        let row: Vec<bool> = laid[first..].iter().map(|r| r.keep).collect();
        assert!(row.len() > 1, "it wraps");
        assert!(row[..row.len() - 1].iter().all(|k| *k) && !row[row.len() - 1]);
    }

    #[test]
    fn code_keeps_its_spaces_until_it_must_break() {
        let src = "# T\n\nPress `hjkl yubn` to move.\n";
        let rows_at = |w| text(&rows(&parse(src).unwrap(), w));
        assert!(rows_at(12).iter().any(|r| r.contains("hjkl yubn")));
        assert!(rows_at(8).iter().all(|r| r.chars().count() <= 8));
    }
}
