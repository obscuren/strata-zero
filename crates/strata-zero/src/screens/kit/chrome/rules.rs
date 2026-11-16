//! CHROME's rules, as the ATTRIBUTES ledger had them: what in the pack fits a site or a
//! socket, what a pick does to the load, EMP and the doc's fee, and what `space` and
//! `x` do there, in a doc's chair, with free chrome, locked or jacked in.

use super::socket_kind;
use crate::actions::{JACKED_IN, jacked_in, perform, report};
use crate::app::AppState;
use crate::clinic::{OUTSIDE, Operating, gone_line, operating};
use crate::ui::chrome::{CYAN, DIM, FAINTER, MAGENTA, TEXT};
use crate::ui::hud::{AMBER, BADGE_FG};
use game::{Action, CopyId, Host, ItemId, Site, SocketKind, put_in_fee, stack_load, take_out_fee};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// A piece in the pack that fits where the cursor is: its copy when it is held per copy,
/// and what installing it adds to the load, with what is fitted in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub item: ItemId,
    pub copy: Option<CopyId>,
    pub load: u32,
}

/// The cursor's place as it stands: a site (`None`) or a socket of what a site holds,
/// what it holds, what in the pack fits there, and the pick while FITS is open.
pub(crate) struct Here {
    pub at: (Site, Option<usize>),
    pub held: Option<ItemId>,
    pub picks: Vec<Pick>,
    pub pick: Option<usize>,
}

impl Here {
    /// The place `at`, the pick `picker` kept on one that exists; none when nothing
    /// fits.
    pub(crate) fn of(st: &AppState, at: (Site, Option<usize>), picker: Option<usize>) -> Here {
        let picks = fitting(st, at);
        let pick = picker
            .filter(|_| !picks.is_empty())
            .map(|p| p.min(picks.len() - 1));
        Here {
            at,
            held: held(st, at).cloned(),
            picks,
            pick,
        }
    }
}

/// What in the pack fits `at`, a site or socket, empty or not, in pack order: a copy
/// each, a stack without one once. A socket takes copies only, as `Fit` moves them.
/// Another of the piece a site holds without a copy is that same piece: no swap.
pub fn fitting(st: &AppState, at: (Site, Option<usize>)) -> Vec<Pick> {
    let (c, ch) = (&st.content, &st.character);
    let same = match at {
        (site, None) if ch.chrome_copy(site).is_none() => ch.chrome_at(site),
        _ => None,
    };
    let mut out: Vec<Pick> = Vec::new();
    for s in &ch.inventory {
        if s.qty == 0 || !ch.fits_at(c, &s.item, at) || (at.1.is_some() && s.copy.is_none()) {
            continue;
        }
        if s.copy.is_none() && same == Some(&s.item) {
            continue;
        }
        if s.copy.is_none() && out.iter().any(|p| p.copy.is_none() && p.item == s.item) {
            continue;
        }
        out.push(Pick {
            item: s.item.clone(),
            copy: s.copy_id(),
            load: stack_load(c, s),
        });
    }
    out
}

/// The load once `pick` is in at `at`: what the place holds leaves first, with what is
/// fitted in it.
pub(crate) fn load_after(st: &AppState, at: (Site, Option<usize>), pick: &Pick) -> u32 {
    let (c, ch) = (&st.content, &st.character);
    ch.chrome_load(c).saturating_sub(ch.load_at(c, at)) + pick.load
}

/// The pieces at `at` that a pick swaps out: what the place holds, with what is fitted
/// in it.
pub(crate) fn held_pieces(st: &AppState, (site, socket): (Site, Option<usize>)) -> usize {
    st.character
        .installed()
        .iter()
        .filter(|&&(s, n, _)| s == site && (socket.is_none() || n == socket))
        .filter(|(_, _, id)| !is_bot(st, id))
        .count()
}

/// A bot is docked, not in the body: no piece of EMP.
fn is_bot(st: &AppState, id: &ItemId) -> bool {
    st.content.item(id).is_some_and(|i| i.bot.is_some())
}

/// The pieces a pick installs: itself and what is fitted in it.
pub(crate) fn pick_pieces(st: &AppState, pick: &Pick) -> usize {
    let stack = st.character.inventory.iter().find(|s| match pick.copy {
        Some(k) => s.copy_id() == Some(k),
        None => s.copy.is_none() && s.item == pick.item,
    });
    let fitted = stack.and_then(|s| s.copy.as_ref()).map_or(0, |k| {
        k.fitted
            .iter()
            .flatten()
            .filter(|s| !is_bot(st, &s.item))
            .count()
    });
    1 + fitted
}

/// What a site or socket holds.
pub(crate) fn held(st: &AppState, (site, socket): (Site, Option<usize>)) -> Option<&ItemId> {
    let ch = &st.character;
    match socket {
        None => ch.chrome_at(site),
        Some(n) => ch
            .chrome_copy(site)?
            .fitted
            .get(n)?
            .as_ref()
            .map(|s| &s.item),
    }
}

/// A place whose contents change anywhere: a socket whose kind needs no doc, a drone's
/// bay. What it holds docks and stows with no chair and no fee.
pub(crate) fn chair_free(st: &AppState, (site, socket): (Site, Option<usize>)) -> bool {
    socket.is_some_and(|n| socket_kind(st, site, n).is_some_and(|k| !k.needs_a_doc()))
}

/// A socket's own word, from its kind.
pub(crate) fn socket_word(st: &AppState, site: Site, n: usize) -> &'static str {
    socket_kind(st, site, n).map_or("socket", SocketKind::label)
}

/// The name of what a site holds.
pub(crate) fn host_name(st: &AppState, site: Site) -> String {
    let id = st.character.chrome_at(site);
    id.and_then(|id| st.content.item(id)).map_or_else(
        || id.map_or_else(String::new, |id| id.to_string()),
        |i| i.name.clone(),
    )
}

/// What the doc charges for a pick at a place: the piece in, and what the place holds
/// out. A tab (`missions::tab_for`) pays both for its piece.
pub(crate) fn pick_fee(st: &AppState, v: &Here, pick: &Pick) -> (u32, u32) {
    let c = &st.content;
    (
        put_in_fee(c, &pick.item),
        v.held.as_ref().map_or(0, |h| take_out_fee(c, h)),
    )
}

/// Whether chrome can change here: in a doc's chair, or with free chrome. CHROME's
/// divider (and the ledger's rule) go amber.
pub(crate) fn changing(st: &AppState) -> bool {
    matches!(operating(st), Operating::Chair { .. } | Operating::Free)
}

/// The mark beside CHROME's title: `· CHROMING UP` in the doc's chair, ` FREE ` in the
/// DEV badge's colours with free chrome, nothing anywhere else.
pub(crate) fn chrome_mark(st: &AppState) -> Vec<(String, Style)> {
    match operating(st) {
        Operating::Chair { .. } => vec![(
            " · CHROMING UP".to_string(),
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        )],
        Operating::Free => vec![
            (" ".to_string(), Style::default()),
            (
                " FREE ".to_string(),
                Style::default()
                    .fg(BADGE_FG)
                    .bg(AMBER)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        Operating::Locked | Operating::Gone { .. } => Vec::new(),
    }
}

/// The LOAD gauge in `segments`: the load's share of the budget, rounded up. With a
/// pick, the load `after` it goes in, what leaves counted: what it adds in amber (in
/// magenta when it won't fit) and the total with it; a lighter one, the lower share and
/// total. The segments, then ` 19` and `/24`.
pub(crate) fn gauge(
    st: &AppState,
    after: Option<u32>,
    segments: u32,
    focused: bool,
) -> Vec<(String, Style)> {
    let ch = &st.character;
    let (load, max) = (ch.chrome_load(&st.content), ch.load_max());
    let share = |n: u32| match max {
        0 => segments,
        m => (n * segments).div_ceil(m).min(segments),
    };
    let total = after.unwrap_or(load);
    let more = total > load;
    let change = if total > max { MAGENTA } else { AMBER };
    let (have, after) = (share(load.min(total)), share(total));
    let fg = |c: Color| Style::default().fg(c);
    vec![
        (
            "▰".repeat(have as usize),
            fg(if focused { CYAN } else { DIM }),
        ),
        ("▰".repeat((after - have) as usize), fg(change)),
        ("▱".repeat((segments - after) as usize), fg(FAINTER)),
        (
            format!(" {total}"),
            fg(if more { change } else { TEXT }).add_modifier(Modifier::BOLD),
        ),
        (format!("/{max}"), fg(DIM)),
    ]
}

/// `space` on a place: true when FITS opens on what in the pack fits there. Jacked
/// in, locked (outside a chair, but for a bay), or with nothing that fits, it says why
/// and stays shut.
pub(crate) fn act(st: &mut AppState, v: &Here) -> bool {
    if jacked_in(st) {
        st.warn(JACKED_IN);
        return false;
    }
    match operating(st) {
        _ if chair_free(st, v.at) => {}
        Operating::Locked => {
            st.warn(OUTSIDE);
            return false;
        }
        Operating::Gone { doc } => {
            st.chair = None;
            st.warn(gone_line(&doc));
            return false;
        }
        Operating::Chair { .. } | Operating::Free => {}
    }
    if v.picks.is_empty() {
        st.warn("nothing in your bag fits");
        return false;
    }
    true
}

/// What `x` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pulled {
    /// An empty place: nothing to pull.
    Nothing,
    /// In the doc's chair, a first `x`: the hint asks, for the fee.
    Asked,
    /// Tried; true when it came out.
    Tried(bool),
}

/// `x` on a place: chrome comes out to the pack, a limb with what is in it. In the
/// doc's chair the first `x` asks (`asked` false), for the fee, and a second one pulls
/// and pays.
pub(crate) fn pull(st: &mut AppState, v: &Here, asked: bool) -> Pulled {
    let Some(held) = v.held.as_ref() else {
        return Pulled::Nothing;
    };
    let fee = take_out_fee(&st.content, held);
    if matches!(operating(st), Operating::Chair { .. })
        && fee > 0
        && !jacked_in(st)
        && !chair_free(st, v.at)
        && !asked
    {
        return Pulled::Asked;
    }
    let (site, socket) = v.at;
    let action = match socket {
        None => Action::Pull(site),
        Some(socket) => Action::Unfit {
            host: Host::Site(site),
            socket,
        },
    };
    let result = perform(st, action);
    let ok = result.is_ok();
    report(st, result);
    Pulled::Tried(ok)
}

/// Installs the pick, or fits it in its socket, what was there going to the pack; true
/// when it went in. A refusal lands in the strip.
pub(crate) fn install(st: &mut AppState, v: &Here, pick: usize) -> bool {
    let p = &v.picks[pick];
    let (site, socket) = v.at;
    let action = match (socket, p.copy) {
        (None, Some(copy)) => Action::InstallCopy { copy, site },
        (None, None) => Action::Install {
            item: p.item.clone(),
            site,
        },
        (Some(socket), Some(piece)) => Action::Fit {
            host: Host::Site(site),
            socket,
            piece,
        },
        // `fitting` offers a socket copies only.
        (Some(_), None) => return false,
    };
    let result = perform(st, action);
    let ok = result.is_ok();
    report(st, result);
    ok
}

/// The words a hint row puts for walking and for leaving the place: CHROME's
/// `←↑↓→ select` and `c meat/chrome`, the ledger's `↑↓ select` and `← attributes`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Keys {
    pub select: &'static str,
    pub back: &'static str,
}

/// The hint row on a place: what space does, and on chrome what `x` does, dimmer while
/// the lock is on; the picker's keys while it is open; the ask after a first `x`
/// (`asked`).
pub(crate) fn hint_line(st: &AppState, v: &Here, asked: bool, keys: Keys) -> Line<'static> {
    let Keys { select, back } = keys;
    let dim = Style::default().fg(DIM);
    let faint = Style::default().fg(FAINTER);
    let amber = Style::default().fg(AMBER);
    let mode = operating(st);
    if let Some(pick) = v.pick {
        // On a held place the pick swaps with what is there; a fee the doc won't
        // take dims it, unless a tab pays it.
        let space = match v.held {
            Some(_) => "space swap",
            None => "space install",
        };
        let (put, out) = pick_fee(st, v, &v.picks[pick]);
        let short = matches!(mode, Operating::Chair { .. })
            && !chair_free(st, v.at)
            && crate::missions::tab_for(st, &v.picks[pick].item).is_none()
            && put + out > st.character.credits;
        return Line::from(vec![
            Span::styled("↑↓ choose · ", dim),
            Span::styled(space, if short { faint } else { dim }),
            Span::styled(" · esc back · Q/E tabs · tab close", dim),
        ]);
    }
    // `x` asked: the second one pulls, for the fee.
    if let (true, Some(held)) = (asked, v.held.as_ref()) {
        let c = &st.content;
        let name = c
            .item(held)
            .map_or_else(|| held.to_string(), |i| i.name.clone());
        let (fee, credits) = (take_out_fee(c, held), st.character.credits);
        let mut spans = vec![
            Span::styled("x again to pull ", amber),
            Span::styled(name, Style::default().fg(TEXT)),
            Span::styled(" for ", amber),
            Span::styled(format!("¤{fee}"), amber),
        ];
        // A pull you can't pay says so, as FITS does.
        if fee > credits {
            let magenta = Style::default().fg(MAGENTA);
            spans[3] = Span::styled(format!("¤{fee}"), magenta);
            spans.push(Span::styled(format!(" · you have ¤{credits}"), magenta));
        }
        let fitted: Vec<&ItemId> = match v.at {
            (site, None) => st
                .character
                .chrome_copy(site)
                .map(|k| k.fitted.iter().flatten().map(|s| &s.item).collect())
                .unwrap_or_default(),
            (_, Some(_)) => Vec::new(),
        };
        // What rides out with it, when it can go at all.
        match fitted.as_slice() {
            _ if fee > credits => {}
            [] => {}
            [one] => spans.push(Span::styled(
                format!(
                    " · it comes out with its {}",
                    c.item(one)
                        .map_or_else(|| one.to_string(), |i| i.name.clone())
                ),
                dim,
            )),
            _ => spans.push(Span::styled(
                " · it comes out with what's fitted in it",
                dim,
            )),
        }
        spans.push(Span::styled(" · esc keep it", dim));
        return Line::from(spans);
    }
    if jacked_in(st) {
        let mut spans = vec![
            Span::styled(format!("{select} · "), dim),
            Span::styled("space what fits", faint),
        ];
        if v.held.is_some() {
            spans.push(Span::styled(" · ", dim));
            spans.push(Span::styled("x pull it out", faint));
        }
        spans.push(Span::styled(
            format!(" · {back} · Q/E tabs · tab close"),
            dim,
        ));
        return Line::from(spans);
    }
    // A bay works in any mode: `space swap · x stow in the bag`, and what docks
    // there docks anywhere.
    if chair_free(st, v.at) {
        let takes = socket_kind(st, v.at.0, v.at.1.unwrap_or(0)).map_or("they", |k| k.takes());
        let keys = match v.held {
            Some(_) => format!("{select} · space swap · x stow in the bag · {back} · Q/E tabs"),
            None => format!("{select} · space dock · {back} · Q/E tabs · tab close"),
        };
        return Line::from(vec![
            Span::styled(keys, dim),
            Span::styled(format!("   {takes} dock anywhere"), faint),
        ]);
    }
    match mode {
        Operating::Locked | Operating::Gone { .. } => Line::from(vec![
            Span::styled(format!("{select} · {back} · Q/E tabs · tab close"), dim),
            Span::styled(format!("   {OUTSIDE}"), faint),
        ]),
        Operating::Chair { .. } => {
            let mut spans = vec![Span::styled(format!("{select} · space what fits"), dim)];
            if let Some(held) = v.held.as_ref() {
                spans.push(Span::styled(" · x pull it out ", dim));
                spans.push(Span::styled(
                    format!("¤{}", take_out_fee(&st.content, held)),
                    amber,
                ));
            }
            spans.push(Span::styled(
                format!(" · {back} · esc get out of the chair"),
                dim,
            ));
            Line::from(spans)
        }
        Operating::Free => {
            let mut spans = vec![Span::styled(format!("{select} · space what fits"), dim)];
            if v.held.is_some() {
                spans.push(Span::styled(" · x pull it out", dim));
            }
            spans.push(Span::styled(
                format!(" · {back} · Q/E tabs · tab close"),
                dim,
            ));
            Line::from(spans)
        }
    }
}
