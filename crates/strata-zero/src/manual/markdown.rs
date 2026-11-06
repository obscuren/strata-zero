//! The manual's Markdown (`manual/README.md`): a `#` title on the first line, `##` and
//! `###` headings, paragraphs, one-level lists and numbered steps, tables of two or
//! three columns, `>` tips, and `code` and **bold** inside them. Anything else is an
//! error naming its line, so a chapter one reader can't draw never lands.

/// A run of text inside a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Span {
    Text(String),
    /// A key, a command, a word the game shows.
    Code(String),
    /// A term where it is defined.
    Bold(String),
}

/// A block of a chapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// The chapter's title, on its first line.
    Title(Vec<Span>),
    /// `##` (2) or `###` (3).
    Heading(u8, Vec<Span>),
    Para(Vec<Span>),
    /// `- ` items.
    List(Vec<Vec<Span>>),
    /// `1. ` items.
    Steps(Vec<Vec<Span>>),
    Table {
        head: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
    },
    /// `> ` lines, set apart from the text.
    Tip(Vec<Span>),
}

/// What went wrong and on which line (from 1).
pub type Error = (usize, String);

/// `src`'s blocks, or the first thing in it the manual's Markdown doesn't have.
pub fn parse(src: &str) -> Result<Vec<Block>, Error> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let n = i + 1;
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        refuse_block(line, n)?;
        if let Some(rest) = line.strip_prefix("# ") {
            if i != 0 {
                return Err((n, "the # title goes on the first line, once".into()));
            }
            out.push(Block::Title(inline(rest, n)?));
            i += 1;
        } else if i == 0 {
            return Err((n, "a chapter opens with its # title".into()));
        } else if let Some(rest) = line.strip_prefix("## ") {
            out.push(Block::Heading(2, inline(rest, n)?));
            i += 1;
        } else if let Some(rest) = line.strip_prefix("### ") {
            out.push(Block::Heading(3, inline(rest, n)?));
            i += 1;
        } else if line.starts_with('#') {
            return Err((n, "headings are # (the title), ## and ###".into()));
        } else if line.starts_with("- ") {
            let (items, next) = items(&lines, i, |l| l.strip_prefix("- ").map(|r| (r, 0)))?;
            out.push(Block::List(items));
            i = next;
        } else if step(line).is_some() {
            let (items, next) = items(&lines, i, step)?;
            for (k, at) in (1..).zip(i..next) {
                if let Some((_, num)) = step(lines[at])
                    && num != k
                {
                    return Err((at + 1, format!("step {num} should be {k}")));
                }
            }
            out.push(Block::Steps(items));
            i = next;
        } else if line.starts_with('|') {
            let (block, next) = table(&lines, i)?;
            out.push(block);
            i = next;
        } else if line.starts_with('>') {
            let mut spans = Vec::new();
            while i < lines.len() && !lines[i].trim().is_empty() {
                let Some(rest) = lines[i].strip_prefix('>') else {
                    return Err((i + 1, "a tip's lines all start with >".into()));
                };
                join(&mut spans, inline(rest.trim(), i + 1)?);
                i += 1;
            }
            out.push(Block::Tip(spans));
        } else {
            let mut spans = Vec::new();
            while i < lines.len() && !lines[i].trim().is_empty() {
                let l = lines[i];
                if starts_block(l) {
                    return Err((i + 1, "leave a blank line before a new block".into()));
                }
                refuse_block(l, i + 1)?;
                join(&mut spans, inline(l.trim(), i + 1)?);
                i += 1;
            }
            out.push(Block::Para(spans));
        }
    }
    if !matches!(out.first(), Some(Block::Title(_))) {
        return Err((1, "a chapter opens with its # title".into()));
    }
    Ok(out)
}

/// The Markdown this manual leaves out, at the start of a line.
fn refuse_block(line: &str, n: usize) -> Result<(), Error> {
    let t = line.trim_start();
    if t.starts_with("```") || t.starts_with("~~~") {
        return Err((n, "no fenced code: put a command in `code`".into()));
    }
    // two spaces continue a list item; four, or a tab, would be code to other readers
    if line.starts_with("    ") || line.starts_with('\t') {
        return Err((
            n,
            "no indented code: a list item's next line is indented two".into(),
        ));
    }
    if t.starts_with("* ") || t.starts_with("+ ") {
        return Err((n, "list items start with - ".into()));
    }
    if t.starts_with("---") || t.starts_with("***") || t.starts_with("===") {
        return Err((n, "no rules or underlined headings".into()));
    }
    Ok(())
}

/// A line that opens a block of its own kind.
fn starts_block(line: &str) -> bool {
    line.starts_with('#')
        || line.starts_with("- ")
        || line.starts_with('|')
        || line.starts_with('>')
        || step(line).is_some()
}

/// A numbered step's text and number: `12. text`.
fn step(line: &str) -> Option<(&str, usize)> {
    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 || digits > 2 {
        return None;
    }
    let rest = line[digits..].strip_prefix(". ")?;
    Some((rest, line[..digits].parse().ok()?))
}

/// The items of a list from line `i`: each opens on a line `opens` takes and goes on
/// over lines indented two spaces. Returns the items and the line after the list.
fn items<'a>(
    lines: &[&'a str],
    mut i: usize,
    opens: impl Fn(&'a str) -> Option<(&'a str, usize)>,
) -> Result<(Vec<Vec<Span>>, usize), Error> {
    let mut items: Vec<Vec<Span>> = Vec::new();
    while i < lines.len() && !lines[i].trim().is_empty() {
        let line = lines[i];
        if let Some((rest, _)) = opens(line) {
            items.push(inline(rest.trim(), i + 1)?);
        } else if let Some(rest) = line.strip_prefix("  ")
            && !rest.starts_with(' ')
            && let Some(last) = items.last_mut()
        {
            join(last, inline(rest.trim(), i + 1)?);
        } else {
            return Err((
                i + 1,
                "a list's next line is an item or indented two spaces; leave a blank line after it"
                    .into(),
            ));
        }
        i += 1;
    }
    Ok((items, i))
}

/// A table from line `i`: its head, its rule, its rows. Returns it and the line after.
fn table(lines: &[&str], mut i: usize) -> Result<(Block, usize), Error> {
    let head = cells(lines[i], i + 1)?;
    if !(2..=3).contains(&head.len()) {
        return Err((i + 1, "a table has two or three columns".into()));
    }
    let rule = lines.get(i + 1).copied().unwrap_or("");
    let rule_cells = split_cells(rule);
    let is_rule = rule_cells.as_ref().is_some_and(|c| {
        c.iter().all(|cell| {
            let t = cell.trim().trim_start_matches(':').trim_end_matches(':');
            !t.is_empty() && t.chars().all(|ch| ch == '-')
        })
    });
    if !is_rule {
        return Err((i + 2, "a table's second line is its rule: |---|---|".into()));
    }
    if rule_cells.map(|c| c.len()) != Some(head.len()) {
        return Err((i + 2, "the rule has as many cells as the head".into()));
    }
    let head = head
        .iter()
        .map(|c| inline(c.trim(), i + 1))
        .collect::<Result<Vec<_>, _>>()?;
    i += 2;
    let mut rows = Vec::new();
    while i < lines.len() && !lines[i].trim().is_empty() {
        let row = cells(lines[i], i + 1)?;
        if row.len() != head.len() {
            return Err((
                i + 1,
                format!("this row has {} cells, the head {}", row.len(), head.len()),
            ));
        }
        rows.push(
            row.iter()
                .map(|c| inline(c.trim(), i + 1))
                .collect::<Result<Vec<_>, _>>()?,
        );
        i += 1;
    }
    Ok((Block::Table { head, rows }, i))
}

/// A table line's cells, or why it isn't one.
fn cells(line: &str, n: usize) -> Result<Vec<String>, Error> {
    split_cells(line).ok_or((n, "a table's lines start and end with |".into()))
}

/// The cells between `|`s, a `|` inside `code` kept; None unless the line is fenced.
fn split_cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if !(t.starts_with('|') && t.ends_with('|') && t.len() >= 2) {
        return None;
    }
    let inner = &t[1..t.len() - 1];
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_code = false;
    for c in inner.chars() {
        match c {
            '`' => {
                in_code = !in_code;
                cur.push(c);
            }
            '|' if !in_code => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    Some(out)
}

/// One line's text as spans: `code` first, so nothing in it is read as markup.
fn inline(text: &str, n: usize) -> Result<Vec<Span>, Error> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('`') {
            let Some(end) = after.find('`') else {
                return Err((n, "a `code` span is never closed".into()));
            };
            if end == 0 {
                return Err((n, "an empty `code` span".into()));
            }
            out.push(Span::Code(after[..end].to_string()));
            rest = &after[end + 1..];
        } else if let Some(after) = rest.strip_prefix("**") {
            let Some(end) = after.find("**") else {
                return Err((n, "a **bold** span is never closed".into()));
            };
            let inner = &after[..end];
            if inner.is_empty() || inner.contains('`') || inner.contains('*') {
                return Err((n, "**bold** holds plain words".into()));
            }
            out.push(Span::Bold(inner.to_string()));
            rest = &after[end + 2..];
        } else if rest.starts_with('*') {
            return Err((n, "no italics: **bold** marks a term".into()));
        } else {
            let end = rest.find(['`', '*']).unwrap_or(rest.len());
            let plain = &rest[..end];
            refuse_inline(plain, n)?;
            push_text(&mut out, plain);
            rest = &rest[end..];
        }
    }
    Ok(out)
}

/// The inline Markdown this manual leaves out, in plain text.
fn refuse_inline(text: &str, n: usize) -> Result<(), Error> {
    if text.contains("](") || text.contains("![") {
        return Err((n, "no links or images: name the chapter instead".into()));
    }
    let bytes = text.as_bytes();
    for (k, &b) in bytes.iter().enumerate() {
        let next = bytes.get(k + 1).copied().unwrap_or(b' ');
        if b == b'<' && (next.is_ascii_alphabetic() || next == b'/' || next == b'!') {
            return Err((n, "no HTML".into()));
        }
        if b == b'\\' {
            return Err((n, "no escapes: put the character in `code`".into()));
        }
        if b == b'_' {
            let before = if k == 0 { b' ' } else { bytes[k - 1] };
            if !before.is_ascii_alphanumeric() || !next.is_ascii_alphanumeric() {
                return Err((n, "no _italics_: an _ goes in `code`".into()));
            }
        }
    }
    Ok(())
}

fn push_text(out: &mut Vec<Span>, text: &str) {
    if let Some(Span::Text(last)) = out.last_mut() {
        last.push_str(text);
    } else {
        out.push(Span::Text(text.to_string()));
    }
}

/// Appends a line's spans to a block's, a space between.
fn join(spans: &mut Vec<Span>, more: Vec<Span>) {
    if !spans.is_empty() {
        push_text(spans, " ");
    }
    for s in more {
        match s {
            Span::Text(t) => push_text(spans, &t),
            other => spans.push(other),
        }
    }
}

/// The plain text of `spans`, markup gone.
pub fn plain(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|s| match s {
            Span::Text(t) | Span::Code(t) | Span::Bold(t) => t.as_str(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Span {
        Span::Text(s.into())
    }
    fn c(s: &str) -> Span {
        Span::Code(s.into())
    }

    #[test]
    fn parses_every_block_kind() {
        let src = "# The **deck**\n\
                   \n\
                   Press `tab` to open it\n\
                   and `esc` to close.\n\
                   \n\
                   ## Tabs\n\
                   ### One\n\
                   \n\
                   - first\n\
                   \x20 goes on\n\
                   - second\n\
                   \n\
                   1. step\n\
                   2. step two\n\
                   \n\
                   | Key | What |\n\
                   |-----|:----:|\n\
                   | `a|b` | both |\n\
                   \n\
                   > A tip\n\
                   > on two lines.\n";
        let blocks = parse(src).unwrap();
        assert_eq!(
            blocks,
            vec![
                Block::Title(vec![t("The "), Span::Bold("deck".into())]),
                Block::Para(vec![
                    t("Press "),
                    c("tab"),
                    t(" to open it and "),
                    c("esc"),
                    t(" to close."),
                ]),
                Block::Heading(2, vec![t("Tabs")]),
                Block::Heading(3, vec![t("One")]),
                Block::List(vec![vec![t("first goes on")], vec![t("second")]]),
                Block::Steps(vec![vec![t("step")], vec![t("step two")]]),
                Block::Table {
                    head: vec![vec![t("Key")], vec![t("What")]],
                    rows: vec![vec![vec![c("a|b")], vec![t("both")]]],
                },
                Block::Tip(vec![t("A tip on two lines.")]),
            ]
        );
    }

    #[test]
    fn rejects_links_images_html_italics_and_fences() {
        for (body, line) in [
            ("see [the Net](net.md)", 3),
            ("![a shot](shot.png)", 3),
            ("a <b>bold</b> word", 3),
            ("an *italic* word", 3),
            ("an _italic_ word", 3),
            ("```\ncode\n```", 3),
            ("a \\* star", 3),
            ("#### deep", 3),
            ("* item", 3),
            ("text\n---", 4),
            ("a `code span", 3),
            ("a **bold span", 3),
        ] {
            let src = format!("# Title\n\n{body}\n");
            let err = parse(&src).expect_err(body);
            assert_eq!(err.0, line, "{body}: {err:?}");
        }
        // an underscore inside a word, and < > as keys, are fine
        assert!(parse("# T\n\nsnake_case and `<` `>` keys, 3 < 4\n").is_ok());
    }

    #[test]
    fn a_title_only_on_the_first_line() {
        assert_eq!(parse("text\n").unwrap_err().0, 1);
        assert_eq!(parse("\n# Late\n").unwrap_err().0, 2);
        assert_eq!(parse("# One\n\n# Two\n").unwrap_err().0, 3);
    }

    #[test]
    fn tables_need_a_rule_row_and_two_or_three_columns() {
        assert_eq!(parse("# T\n\n| a | b |\n| c | d |\n").unwrap_err().0, 4);
        assert_eq!(parse("# T\n\n| a |\n|---|\n").unwrap_err().0, 3);
        assert_eq!(
            parse("# T\n\n| a | b | c | d |\n|---|---|---|---|\n")
                .unwrap_err()
                .0,
            3
        );
        assert_eq!(
            parse("# T\n\n| a | b |\n|---|---|\n| c |\n").unwrap_err().0,
            5
        );
        assert!(parse("# T\n\n| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |\n").is_ok());
    }

    #[test]
    fn blocks_need_a_blank_line_between_them() {
        assert_eq!(parse("# T\n\ntext\n- item\n").unwrap_err().0, 4);
        assert_eq!(parse("# T\n\n- item\ntext\n").unwrap_err().0, 4);
        assert_eq!(parse("# T\n\n1. one\n3. three\n").unwrap_err().0, 4);
    }
}
