//! CHROME's card: an installed piece's card under its drawing, and the FITS card of
//! what in the pack fits the cursor's place, with the pick's load, EMP and fee.

use super::rules::{Here, held_pieces, host_name, load_after, pick_fee, pick_pieces, socket_word};
use crate::app::AppState;
use crate::clinic::{Operating, operating};
use crate::screens::create::wrap;
use crate::screens::deck::{cut, lit};
use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT, put};
use crate::ui::hud::{AMBER, GREEN};
use game::{Content, EMP_PER_PIECE, Item, ItemId, Perk, Site, SocketKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// The card's socket column: `■ hand    Arc hand`.
const SOCKET_W: usize = 10;

/// A card's lines, a row each, which of them sit on the bar, and where its description
/// starts, if it has one.
#[derive(Default)]
pub(crate) struct CardText {
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) bars: Vec<bool>,
    pub(crate) desc: Option<usize>,
}

impl CardText {
    pub(crate) fn push(&mut self, line: Line<'static>) {
        self.lines.push(line);
        self.bars.push(false);
    }

    fn extend(&mut self, lines: Vec<Line<'static>>) {
        for line in lines {
            self.push(line);
        }
    }

    fn blank(&mut self) {
        self.push(Line::default());
    }

    /// The description, wrapped, after a blank row.
    fn description(&mut self, text: &str, w: usize) {
        self.blank();
        self.desc = Some(self.lines.len());
        for line in wrap(text, w) {
            self.push(Line::from(Span::styled(line, Style::default().fg(TEXT))));
        }
    }

    /// Fits `rows`: blank rows give way first, top down, then the description from its
    /// end, its last line kept marked cut; then the bottom.
    pub(crate) fn fit(mut self, rows: usize, w: usize) -> CardText {
        let mut cut_desc = false;
        while self.lines.len() > rows {
            if let Some(i) = self.lines.iter().position(|l| l.width() == 0) {
                self.lines.remove(i);
                self.bars.remove(i);
                self.desc = self.desc.map(|d| if i < d { d - 1 } else { d });
            } else if self.desc.is_some_and(|d| self.lines.len() > d) {
                self.lines.pop();
                self.bars.pop();
                cut_desc = true;
            } else {
                self.lines.truncate(rows);
                self.bars.truncate(rows);
                cut_desc = false;
            }
        }
        if cut_desc
            && self.desc.is_some_and(|d| self.lines.len() > d)
            && let Some(last) = self.lines.last_mut()
        {
            let text = format!("{last}…");
            *last = Line::from(Span::styled(cut(&text, w), Style::default().fg(TEXT)));
        }
        self
    }
}

/// An installed piece's card, under its drawing: the name, where it sits and what it
/// costs, what it does; a limb's sockets and what is in each; the description.
pub(crate) fn piece_card(
    st: &AppState,
    (site, socket): (Site, Option<usize>),
    id: &ItemId,
    w: usize,
) -> CardText {
    let c = &st.content;
    let mut card = CardText::default();
    let fg = |c: Color| Style::default().fg(c);
    let Some(item) = c.item(id) else {
        card.push(Line::from(Span::styled(id.to_string(), fg(TEXT))));
        return card;
    };
    card.push(Line::from(Span::styled(
        cut(&item.name.to_uppercase(), w),
        fg(CYAN).add_modifier(Modifier::BOLD),
    )));
    let place = match socket {
        None => site.label().to_string(),
        Some(n) => format!(
            "{} · on the {}",
            socket_word(st, site, n),
            host_name(st, site)
        ),
    };
    if item.bot.is_some() {
        // Docked, not installed: no load, no EMP, and out with you.
        card.extend(wrap_spans(
            vec![
                Span::styled("docked in the ", fg(DIM)),
                Span::styled(host_name(st, site), fg(TEXT)),
            ],
            w,
        ));
        card.extend(wrap_spans(effects(c, item, false), w));
        card.push(Line::from(Span::styled(
            cut("out with you · follows on foot", w),
            fg(FAINTER),
        )));
        card.description(&item.desc, w);
        return card;
    }
    let load = item.implant.map_or(0, |i| i.load);
    card.extend(wrap_spans(
        vec![
            Span::styled(format!("{place} · load {load} · "), fg(DIM)),
            Span::styled(format!("EMP {}", signed(-EMP_PER_PIECE)), fg(MAGENTA)),
        ],
        w,
    ));
    card.extend(wrap_spans(effects(c, item, false), w));
    if socket.is_none() && !item.sockets.is_empty() {
        card.blank();
        let copy = st.character.chrome_copy(site);
        for (n, &kind) in item.sockets.iter().enumerate() {
            let fitted = copy
                .and_then(|k| k.fitted.get(n)?.as_ref())
                .and_then(|s| c.item(&s.item));
            card.extend(socket_lines(c, kind, fitted, w));
        }
    }
    card.description(&item.desc, w);
    card
}

/// A limb's socket on its card: `■ hand    Arc hand` with the piece's effects under
/// it, or `□ forearm empty` with what in content fits it, faint and cut.
fn socket_lines(
    c: &Content,
    kind: SocketKind,
    fitted: Option<&Item>,
    w: usize,
) -> Vec<Line<'static>> {
    let fg = |c: Color| Style::default().fg(c);
    let label = |mark: &str| format!("{mark} {:<width$} ", kind.label(), width = SOCKET_W - 3);
    let indent = label("■").chars().count();
    let room = w.saturating_sub(indent);
    let pad = Span::raw(" ".repeat(indent));
    let mut out = Vec::new();
    match fitted {
        Some(piece) => {
            out.push(Line::from(vec![
                Span::styled(label("■"), fg(CYAN)),
                Span::styled(
                    cut(&piece.name, room),
                    fg(TEXT).add_modifier(Modifier::BOLD),
                ),
            ]));
            for line in wrap_spans(effects(c, piece, true), room) {
                let mut spans = vec![pad.clone()];
                spans.extend(line.spans);
                out.push(Line::from(spans));
            }
        }
        None => {
            out.push(Line::from(vec![
                Span::styled(label("□"), fg(DIM)),
                Span::styled("empty", fg(FAINTER)),
            ]));
            let takes: Vec<&str> = c
                .items
                .iter()
                .filter(|i| i.fits_socket() == Some(kind))
                .map(|i| i.name.as_str())
                .collect();
            if !takes.is_empty() {
                out.push(Line::from(vec![
                    pad,
                    Span::styled(cut(&takes.join(", "), room), fg(FAINTER)),
                ]));
            }
        }
    }
    out
}

/// A site's or socket's FITS card: its name, what it is or what it holds, and what in
/// the pack fits it; while the picker is open, the pick on the bar, what it does, what
/// it does to the load and EMP (what it swaps out counted), and its description.
pub(crate) fn fit_card(st: &AppState, v: &Here, focused: bool, w: usize) -> CardText {
    let (c, ch) = (&st.content, &st.character);
    let (site, socket) = v.at;
    let mut card = CardText::default();
    let fg = |c: Color| Style::default().fg(c);
    let bold = |c: Color| fg(c).add_modifier(Modifier::BOLD);
    let title = match socket {
        None => site.long().to_string(),
        Some(n) => socket_word(st, site, n).to_uppercase(),
    };
    card.push(Line::from(Span::styled(title, bold(CYAN))));
    match (socket, &v.held) {
        (_, Some(id)) => {
            let name = c
                .item(id)
                .map_or_else(|| id.to_string(), |i| i.name.clone());
            card.extend(wrap_spans(
                vec![
                    Span::styled("replaces ", fg(DIM)),
                    Span::styled(name, fg(TEXT)),
                ],
                w,
            ));
        }
        (None, None) => card.push(Line::from(vec![
            Span::styled("meat", fg(DIM)),
            Span::styled(" · nothing fitted", fg(FAINTER)),
        ])),
        (Some(_), None) => card.extend(wrap_spans(
            vec![
                Span::styled("a socket on the ", fg(DIM)),
                Span::styled(host_name(st, site), fg(TEXT)),
            ],
            w,
        )),
    }
    card.blank();
    // FITS ──── from your bag
    let tail = "from your bag";
    let run = w.saturating_sub("FITS".len() + tail.len() + 2);
    card.push(Line::from(vec![
        Span::styled("FITS", bold(if focused { CYAN } else { DIM })),
        Span::raw(" "),
        Span::styled("─".repeat(run), fg(RULE)),
        Span::raw(" "),
        Span::styled(tail, fg(FAINTER)),
    ]));
    if v.picks.is_empty() {
        card.push(Line::from(Span::styled(
            "nothing in your bag fits",
            fg(FAINTER),
        )));
        return card;
    }
    let (load, max) = (ch.chrome_load(c), ch.load_max());
    for (i, pick) in v.picks.iter().enumerate() {
        let on = v.pick == Some(i);
        let over = load_after(st, v.at, pick) > max;
        let bar = |s: Style| if on { s.bg(BAR_BG) } else { s };
        let name = c
            .item(&pick.item)
            .map_or_else(|| pick.item.to_string(), |i| i.name.clone());
        // A bot takes no load: its column says what it does.
        let bot = c.item(&pick.item).filter(|i| i.bot.is_some());
        let tag = match bot {
            Some(i) => effects(c, i, true)
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>(),
            None => format!("L{}", pick.load),
        };
        // `▸ Name ········· L3 `: the tag ends a column short of the edge.
        let name = cut(&name, w.saturating_sub(tag.len() + 4));
        let pad = w.saturating_sub(2 + name.chars().count() + tag.len() + 1);
        let tag_style = match (over, on) {
            _ if bot.is_some() => fg(GREEN),
            (true, _) => fg(MAGENTA),
            (false, true) => fg(lit(st.tick)),
            (false, false) => fg(DIM),
        };
        let lift = |s: Style| {
            if on {
                s.add_modifier(Modifier::BOLD)
            } else {
                s
            }
        };
        card.lines.push(Line::from(vec![
            Span::styled(if on { "▸ " } else { "  " }, bar(bold(lit(st.tick)))),
            Span::styled(name, bar(if on { bold(CYAN) } else { fg(TEXT) })),
            Span::styled(" ".repeat(pad), bar(Style::default())),
            Span::styled(tag, bar(lift(tag_style))),
            Span::styled(" ", bar(Style::default())),
        ]));
        card.bars.push(on);
    }
    let Some(pick) = v.pick.map(|i| &v.picks[i]) else {
        return card;
    };
    let Some(item) = c.item(&pick.item) else {
        return card;
    };
    card.blank();
    card.extend(wrap_spans(effects(c, item, false), w));
    if item.bot.is_some() {
        card.description(&item.desc, w);
        return card;
    }
    // `load 19 → 21/24 · EMP −2`: what leaves counted; a lighter swap's total plain,
    // and EMP only when the count of pieces moves.
    let after = load_after(st, v.at, pick);
    let pieces = pick_pieces(st, pick) as i32 - held_pieces(st, v.at) as i32;
    let total = match after {
        a if a > max => MAGENTA,
        a if a > load => AMBER,
        _ => TEXT,
    };
    let mut spans = vec![
        Span::styled(format!("load {load} → "), fg(DIM)),
        Span::styled(after.to_string(), bold(total)),
        Span::styled(format!("/{max}"), fg(DIM)),
    ];
    if pieces != 0 {
        let emp = -EMP_PER_PIECE * pieces;
        spans.push(Span::styled(" · ", fg(DIM)));
        spans.push(Span::styled(
            format!("EMP {}", signed(emp)),
            fg(if emp < 0 { MAGENTA } else { GREEN }),
        ));
    }
    card.extend(wrap_spans(spans, w));
    // In the doc's chair, `fee ¤370 · leaves ¤830`: a swap's both parts, a fee you
    // can't pay in magenta, a piece on someone's tab `fee TAB · NAME · ¤370 on the tab`.
    if matches!(operating(st), Operating::Chair { .. }) {
        let (put, out) = pick_fee(st, v, pick);
        let (fee, credits) = (put + out, ch.credits);
        let amount = format!("¤{fee}");
        let line = if let Some(tab) = crate::missions::tab_for(st, &pick.item) {
            vec![
                Span::styled("fee ", fg(DIM)),
                Span::styled(format!("TAB · {}", tab.payer.to_uppercase()), bold(AMBER)),
                Span::styled(format!(" · {amount} on the tab"), fg(DIM)),
            ]
        } else if fee > credits {
            vec![
                Span::styled("fee ", fg(DIM)),
                Span::styled(amount, bold(MAGENTA)),
                Span::styled(format!(" · you have ¤{credits}"), fg(MAGENTA)),
            ]
        } else if out > 0 {
            vec![
                Span::styled("fee ", fg(DIM)),
                Span::styled(amount, bold(AMBER)),
                Span::styled(format!(" · in ¤{put} · out ¤{out}"), fg(DIM)),
            ]
        } else {
            vec![
                Span::styled("fee ", fg(DIM)),
                Span::styled(amount, bold(AMBER)),
                Span::styled(format!(" · leaves ¤{}", credits - fee), fg(DIM)),
            ]
        };
        card.extend(wrap_spans(line, w));
    }
    card.description(&item.desc, w);
    card
}

/// `+3`, or `−2` with a true minus.
pub(crate) fn signed(n: i32) -> String {
    if n < 0 {
        format!("−{}", -n)
    } else {
        format!("+{n}")
    }
}

/// A perk in the card's short form, `BODY +1`, `sight +4`, `KIN +15`, and which way it
/// goes.
fn perk_text(p: &Perk) -> (String, i32) {
    let (word, n): (&str, i32) = match *p {
        Perk::Attribute(s, n) => (s.name(), n.into()),
        Perk::Sight(n) => ("sight", n.into()),
        Perk::Carry(kg) => ("carry", kg.round() as i32),
        Perk::HpMax(n) => ("HP", n),
        Perk::Focus(n) => ("focus", n),
        Perk::Armor(n) => ("ARM", n.into()),
        Perk::Move(n) => ("MOV", n.into()),
        Perk::Resist(h, n) => (h.short(), n.into()),
        Perk::Throw(n) => ("throw", n.into()),
    };
    (format!("{word} {}", signed(n)), n)
}

/// What a piece does: its perks, green up and magenta down, and the abilities it grants
/// in cyan, ` · ` between. In full the grants follow the perks after `grants`; compact,
/// under a socket, they lead, bare.
fn effects(c: &Content, item: &Item, compact: bool) -> Vec<Span<'static>> {
    let fg = |c: Color| Style::default().fg(c);
    let perks = item.perks.iter().map(|p| {
        let (text, n) = perk_text(p);
        vec![Span::styled(text, fg(if n < 0 { MAGENTA } else { GREEN }))]
    });
    let mut grants = Vec::new();
    for (i, a) in item.grants.iter().enumerate() {
        if i > 0 {
            grants.push(Span::styled(", ", fg(DIM)));
        }
        let name = c
            .ability(a)
            .map_or_else(|| a.to_string(), |a| a.name.clone());
        grants.push(Span::styled(name, fg(CYAN)));
    }
    let mut parts: Vec<Vec<Span<'static>>> = Vec::new();
    if compact && !grants.is_empty() {
        parts.push(grants.clone());
    }
    parts.extend(perks);
    if !compact && !grants.is_empty() {
        grants.insert(0, Span::styled("grants ", fg(DIM)));
        parts.push(grants);
    }
    let mut out = Vec::new();
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            out.push(Span::styled(" · ", fg(DIM)));
        }
        out.extend(part);
    }
    out
}

/// Spans wrapped at their spaces into lines at most `width` wide, each keeping its
/// colours; a word too long for a line is cut.
fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    let chars: Vec<(char, Style)> = spans
        .iter()
        .flat_map(|s| s.content.chars().map(move |ch| (ch, s.style)))
        .collect();
    let mut rows: Vec<Vec<(char, Style)>> = Vec::new();
    let mut row: Vec<(char, Style)> = Vec::new();
    for word in chars.split(|(ch, _)| *ch == ' ').filter(|w| !w.is_empty()) {
        if !row.is_empty() && row.len() + 1 + word.len() > width {
            rows.push(std::mem::take(&mut row));
        }
        if let Some(&(_, style)) = row.last() {
            row.push((' ', style));
        }
        let room = width.saturating_sub(row.len());
        row.extend(word.iter().take(room));
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows.into_iter()
        .map(|row| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut run = String::new();
            let mut style = None;
            for (ch, s) in row {
                if style.is_some_and(|st| st != s) {
                    spans.push(Span::styled(
                        std::mem::take(&mut run),
                        style.unwrap_or_default(),
                    ));
                }
                style = Some(s);
                run.push(ch);
            }
            if !run.is_empty() {
                spans.push(Span::styled(run, style.unwrap_or_default()));
            }
            Line::from(spans)
        })
        .collect()
}

/// Draws a card's typed lines into `area`, a row each, its bar rows lit across.
pub(crate) fn draw_lines(buf: &mut Buffer, area: Rect, lines: &[Line<'static>], bars: &[bool]) {
    for (k, line) in lines.iter().take(area.height as usize).enumerate() {
        let y = area.y + k as u16;
        if bars.get(k).copied().unwrap_or(false) {
            for x in area.x..area.right() {
                buf[(x, y)].set_bg(BAR_BG);
            }
        }
        let mut x = area.x;
        for span in &line.spans {
            put(buf, x, y, &span.content, span.style, area.right());
            x = x.saturating_add(span.width() as u16);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tall_card_gives_way_blanks_then_description() {
        let card = || {
            let mut card = CardText::default();
            for t in ["NAME", "", "a", "b", ""] {
                card.push(Line::from(t));
            }
            card.desc = Some(card.lines.len());
            for t in ["one two", "three", "four"] {
                card.push(Line::from(t));
            }
            card
        };
        let shown = |rows: usize| -> Vec<String> {
            let c = card().fit(rows, 20);
            assert_eq!(c.lines.len(), c.bars.len());
            c.lines.iter().map(|l| l.to_string()).collect()
        };
        assert_eq!(
            shown(8),
            ["NAME", "", "a", "b", "", "one two", "three", "four"]
        );
        assert_eq!(shown(7), ["NAME", "a", "b", "", "one two", "three", "four"]);
        assert_eq!(shown(6), ["NAME", "a", "b", "one two", "three", "four"]);
        assert_eq!(shown(5), ["NAME", "a", "b", "one two", "three…"]);
        assert_eq!(shown(3), ["NAME", "a", "b"]);
        assert_eq!(shown(2), ["NAME", "a"]);
    }
}
