use super::*;
use crate::actions::perform;
use crate::screens::popup::Pick;
use crate::ui::chrome::{BAR_BG, CYAN, PANEL_BG, TEXT};
use game::{Action, BodyNode, Character, Host, ItemId, PartKind};
use ratatui::style::Color;

fn state() -> AppState {
    let mut st = AppState::new();
    st.character = Character::debug_runner(&st.content);
    st
}

/// The tick at which the card has typed out.
fn settle(k: &Kit, st: &mut AppState) {
    st.tick = k.changed + REVEAL_TICKS + 40;
}

/// The tab's body in a window of `w` by `h`, as the deck sets it.
fn tab(w: u16, h: u16) -> Rect {
    let top = if h >= 36 { 10 } else { 5 };
    Rect::new(1, top, w - 2, h - top - 3)
}

fn drawn(k: &Kit, st: &AppState) -> Buffer {
    drawn_at(k, st, 100, 30)
}

fn drawn_at(k: &Kit, st: &AppState, w: u16, h: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    k.draw(st, tab(w, h), &mut buf);
    buf
}

/// `n` cells of row `y` from column `x`.
fn cells(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
    (x..x + n).map(|cx| buf[(cx, y)].symbol()).collect()
}

/// Row `y` from column `x` to the buffer's right edge.
fn from(buf: &Buffer, x: u16, y: u16) -> String {
    cells(buf, x, y, buf.area.right() - x)
}

/// The box-drawing cells of the full body's row `y`, from the graph's origin, the rest
/// blank.
fn boxes(buf: &Buffer, y: u16) -> String {
    let row: String = (2..63)
        .map(|x| {
            let c = buf[(x, y)].symbol().chars().next().unwrap_or(' ');
            if ('\u{2500}'..='\u{257f}').contains(&c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    row.trim_end().to_string()
}

/// Wears what the mock shows on the parts it names.
fn dress(st: &mut AppState) {
    for (part, item) in [
        (BodyPart::Face, "mirrorshades"),
        (BodyPart::LeftHand, "shiv_blade"),
        (BodyPart::Torso, "salvage_jacket"),
        (BodyPart::Legs, "cargo_pants"),
    ] {
        st.character.wear(part, ItemId::new(item), None);
    }
    fit(st, BodyPart::RightHand, 0, "hushcan");
    fit(st, BodyPart::RightHand, 1, "redeye");
}

/// A body with a third arm off the torso, and its hand.
fn third_arm(st: &mut AppState) {
    st.content.body.push(BodyNode {
        part: BodyPart::ThirdArm,
        kind: PartKind::Arm,
        name: "ARM 3".into(),
        joins: Some(BodyPart::Torso),
        main: false,
    });
    st.content.body.push(BodyNode {
        part: BodyPart::ThirdHand,
        kind: PartKind::Hand,
        name: "HAND 3".into(),
        joins: Some(BodyPart::ThirdArm),
        main: false,
    });
}

/// Fits the first `item` in the pack into a socket of what `part` wears.
fn fit(st: &mut AppState, part: BodyPart, socket: usize, item: &str) {
    let piece = st
        .character
        .inventory
        .iter()
        .find(|s| s.item == ItemId::new(item))
        .and_then(|s| s.copy_id())
        .unwrap_or_else(|| panic!("a copy of {item} in the pack"));
    perform(
        st,
        Action::Fit {
            host: Host::Worn(part),
            socket,
            piece,
        },
    )
    .unwrap();
}

fn corners_in(buf: &Buffer, view: Rect, fg: Color) {
    let (r, b) = (view.right() - 1, view.bottom() - 1);
    for (x, y, ch) in [
        (view.x, view.y, "┌"),
        (r, view.y, "┐"),
        (view.x, b, "└"),
        (r, b, "┘"),
    ] {
        assert_eq!(
            (buf[(x, y)].symbol(), buf[(x, y)].fg),
            (ch, fg),
            "({x},{y})"
        );
    }
}

fn press(k: &mut Kit, st: &AppState, code: KeyCode, tick: u64) -> bool {
    k.key(st, code, tick)
}

fn texts(k: &Kit, st: &AppState) -> Vec<String> {
    k.pack_rows(st).into_iter().map(|r| r.text).collect()
}

fn to_row(k: &mut Kit, st: &AppState, name: &str) {
    let i = k
        .pack_rows(st)
        .iter()
        .position(|r| r.text.contains(name))
        .expect("row");
    k.state.pane = Pane::Pack;
    k.state.row = i;
}

/// The mock's graph (`gen.py` `graph()`), its box-drawing cells only.
const MOCK: [&str; 25] = [
    "                        ┌      ───┐ ┌      ───┐",
    "                        │         │ │         │",
    "                        │         ├─┤         │",
    "                        │         │ │         │",
    "                        └────────┬┘ └─────────┘",
    "                                 │",
    "                        ┌       ─┴─┐",
    "┌        ─┐ ┌       ──┐ │          │ ┌       ──┐ ┌        ─┐",
    "│         │ │         │ │          │ │         │ │         │",
    "│         ├─┤         ├─┤          ├─┤         ├─┤         │",
    "│         │ │         │ │          │ │         │ │         │",
    "└─────────┘ └─────────┘ │          │ └─────────┘ └─────────┘",
    "                        └────────┬─┘",
    "                    ┌────────────┤",
    "            ┌      ─┴─┐ ┌       ─┴┐",
    "            │         │ │         │",
    "            │         │ │         │",
    "            │         │ │         │",
    "            └─────────┘ └────────┬┘",
    "                                 │",
    "                        ┌      ──┴┐ ┌      ───┐",
    "                        │         │ │         │",
    "                        │         ├─┤         │",
    "                        │         │ │         │",
    "                        └─────────┘ └─────────┘",
];

/// The mock's rows with a third arm that differ from `MOCK`, from its row 13.
const MOCK_THIRD: [&str; 6] = [
    "                    ┌────────────┼────────────┐",
    "            ┌      ─┴─┐ ┌       ─┴┐  ┌       ─┴┐ ┌        ─┐",
    "            │         │ │         │  │         │ │         │",
    "            │         │ │         │  │         ├─┤         │",
    "            │         │ │         │  │         │ │         │",
    "            └─────────┘ └────────┬┘  └─────────┘ └─────────┘",
];

#[test]
fn layout_at_the_smallest_and_a_common_terminal() {
    let st = state();
    let l = layout(tab(100, 30), &parts(&st));
    assert!(!l.body.full);
    assert_eq!(l.divider, 49);
    assert_eq!(l.pack, Rect::new(50, 5, 49, 9));
    assert_eq!(l.rule, 14);
    assert_eq!(l.card, Rect::new(51, 15, 47, 12));
    let l = layout(tab(120, 40), &parts(&st));
    assert!(l.body.full);
    assert_eq!(l.divider, 63, "the mock's separator");
    assert_eq!(l.pack, Rect::new(64, 10, 55, 11));
    assert_eq!(l.rule, 21);
    assert_eq!(l.card, Rect::new(65, 22, 53, 15));
}

#[test]
fn the_full_body_matches_the_mock_at_120_by_40() {
    let mut st = state();
    let k = Kit::new(0);
    settle(&k, &mut st);
    let buf = drawn_at(&k, &st, 120, 40);
    for (i, row) in MOCK.iter().enumerate() {
        assert_eq!(boxes(&buf, 12 + i as u16), *row, "graph row {i}");
    }
    assert_eq!(cells(&buf, 26, 12, 23), "┌ HEAD ───┐ ┌ FACE ───┐");
    assert_eq!(
        cells(&buf, 2, 19, 60),
        "┌ L HAND ─┐ ┌ L ARM ──┐ │          │ ┌ R ARM ──┐ ┌ R HAND ─┐"
    );
    assert_eq!(cells(&buf, 26, 18, 12), "┌ TORSO ─┴─┐");
    assert_eq!(cells(&buf, 14, 26, 23), "┌ BACK ─┴─┐ ┌ WAIST ─┴┐");
    assert_eq!(cells(&buf, 26, 32, 23), "┌ LEGS ──┴┐ ┌ FEET ───┐");
    // The header and its rule; the divider, its junction with the pack's rule.
    assert_eq!(cells(&buf, 2, 10, 6), "▌MEAT▐");
    assert_eq!(cells(&buf, 1, 11, 62), "─".repeat(62));
    for y in 10..37 {
        let want = if y == 21 { "╠" } else { "║" };
        assert_eq!(buf[(63, y)].symbol(), want, "y {y}");
    }
}

#[test]
fn a_third_arm_lands_in_its_free_places() {
    let mut st = state();
    third_arm(&mut st);
    st.character
        .wear(BodyPart::ThirdHand, ItemId::new("scrap_nine"), None);
    let k = Kit::new(0);
    settle(&k, &mut st);
    let buf = drawn_at(&k, &st, 120, 40);
    for (i, row) in MOCK.iter().enumerate() {
        let want = i
            .checked_sub(13)
            .and_then(|j| MOCK_THIRD.get(j))
            .unwrap_or(row);
        assert_eq!(boxes(&buf, 12 + i as u16), *want, "graph row {i}");
    }
    assert_eq!(cells(&buf, 39, 26, 23), "┌ ARM 3 ─┴┐ ┌ HAND 3 ─┐");
    assert_eq!(cells(&buf, 52, 27, 5), "Scrap");
    assert_eq!(cells(&buf, 52, 28, 4), "nine");
    assert_eq!(cells(&buf, 52, 29, 5), "○ 5×1");
    // Compact, its hand hangs off the torso under R HAND.
    let buf = drawn(&k, &st);
    assert_eq!(cells(&buf, 33, 18, 14), "┌ HAND 3 ─┴──┐");
}

#[test]
fn a_tile_says_the_name_on_two_lines_and_what_it_gives() {
    let mut st = state();
    dress(&mut st);
    let k = Kit::new(0);
    settle(&k, &mut st);
    let buf = drawn_at(&k, &st, 120, 40);
    let at = |x: u16, y: u16, text: &str| {
        let n = text.chars().count() as u16;
        assert_eq!(cells(&buf, x, y, n), text, "({x},{y})");
    };
    at(52, 20, "Ratchet");
    at(52, 21, "SMG");
    at(52, 22, "● 4×3 ■■");
    assert_eq!(buf[(52, 22)].fg, CYAN, "the main hand's dot");
    assert_eq!(buf[(54, 22)].fg, DIM);
    assert_eq!(
        buf[(58, 22)].fg,
        Color::Rgb(0x3f, 0x7c, 0x87),
        "the sockets"
    );
    at(3, 20, "Monowire");
    at(3, 21, "shiv");
    at(3, 22, "○ REF +1");
    at(27, 27, "Rebuilt");
    at(27, 28, "pawn deck");
    at(27, 29, "■■■■■ □□");
    at(27, 20, "Salvage");
    at(27, 21, "jacket");
    at(27, 22, "+5 HP");
    assert_eq!((buf[(27, 20)].fg, buf[(27, 22)].fg), (TEXT, DIM));
    at(27, 33, "Cargo");
    at(27, 34, "pants");
    at(27, 35, "+4 kg");
    at(39, 13, "Mirrorsh…");
    at(39, 15, "COOL +1");
    // Empty: the feet, and the arms, which take nothing yet.
    for (x, y) in [(43, 34), (19, 21), (44, 21)] {
        assert_eq!((buf[(x, y)].symbol(), buf[(x, y)].fg), ("·", FAINTER));
    }
    // Compact, the name takes one line.
    let buf = drawn(&k, &st);
    assert_eq!(cells(&buf, 33, 12, 14), "┌ R HAND ────┐");
    assert_eq!(cells(&buf, 34, 13, 11), "Ratchet SMG");
    assert_eq!(cells(&buf, 34, 14, 8), "● 4×3 ■■");
    assert_eq!(cells(&buf, 19, 13, 12), "Salvage jac…");
}

#[test]
fn the_cursors_tile_is_lit() {
    let mut st = state();
    let mut k = Kit::new(0);
    k.state.part = BodyPart::RightHand;
    settle(&k, &mut st);
    let buf = drawn_at(&k, &st, 120, 40);
    assert_eq!(buf[(51, 19)].fg, CYAN);
    assert!(buf[(53, 19)].modifier.contains(Modifier::BOLD), "its label");
    assert_eq!(buf[(55, 21)].bg, BAR_BG);
    assert!(buf[(52, 20)].modifier.contains(Modifier::BOLD), "its name");
    assert_eq!(buf[(58, 22)].fg, CYAN, "its sockets");
    assert_eq!(buf[(39, 19)].fg, RULE, "the rest stay dark");
    assert!(
        buf[(3, 10)].modifier.contains(Modifier::BOLD),
        "MEAT, focused"
    );

    k.state.pane = Pane::Pack;
    k.reselect(&st);
    let buf = drawn_at(&k, &st, 120, 40);
    assert_eq!(buf[(51, 19)].fg, DIM, "the tile keeps its place, dim");
    assert_ne!(buf[(55, 21)].bg, BAR_BG);
    assert_eq!(buf[(3, 10)].fg, DIM);
}

#[test]
fn the_arrows_walk_the_body_and_right_crosses_to_the_pack() {
    let st = state();
    let mut k = Kit::new(0);
    for (key, to) in [
        (KeyCode::Up, BodyPart::Head),
        (KeyCode::Char('h'), BodyPart::LeftArm),
        (KeyCode::Right, BodyPart::RightArm),
        (KeyCode::Char('j'), BodyPart::Waist),
    ] {
        k.state.part = BodyPart::Torso;
        assert!(press(&mut k, &st, key, 5));
        assert_eq!(k.state.part, to, "{key:?}");
    }
    k.changed = 0;
    k.state.part = BodyPart::Head;
    assert!(press(&mut k, &st, KeyCode::Up, 5));
    assert_eq!(
        (k.state.part, k.changed),
        (BodyPart::Head, 0),
        "a stop is no move"
    );

    k.state.part = BodyPart::RightHand;
    assert!(press(&mut k, &st, KeyCode::Right, 6));
    assert_eq!((k.state.pane, k.changed), (Pane::Pack, 6));
    assert!(press(&mut k, &st, KeyCode::Left, 7));
    assert_eq!(
        (k.state.pane, k.state.part),
        (Pane::Body, BodyPart::RightHand)
    );

    // Drawn compact, the hands sit on the torso; an arm kept from the full body
    // reads as its hand.
    drawn(&k, &st);
    k.state.part = BodyPart::Torso;
    press(&mut k, &st, KeyCode::Left, 8);
    assert_eq!(k.state.part, BodyPart::LeftHand);
    k.state.part = BodyPart::LeftArm;
    assert_eq!(k.selected(&st), Some(Detail::Part(BodyPart::LeftHand)));
}

#[test]
fn the_pack_wraps_and_left_returns_to_the_tile() {
    let st = state();
    let mut k = Kit::new(0);
    k.state.pane = Pane::Pack;
    k.reselect(&st);
    let rows = k.pack_rows(&st);
    let sel: Vec<usize> = (0..rows.len())
        .filter(|&i| rows[i].detail.is_some())
        .collect();
    assert_eq!(k.state.row, sel[0]);
    assert!(press(&mut k, &st, KeyCode::Up, 1));
    assert_eq!(k.state.row, *sel.last().unwrap());
    assert!(press(&mut k, &st, KeyCode::Down, 2));
    assert_eq!(k.state.row, sel[0]);
    assert!(press(&mut k, &st, KeyCode::Left, 3));
    assert_eq!(k.state.pane, Pane::Body);
}

#[test]
fn the_filter_steps_through_the_kinds_carried() {
    let mut st = state();
    st.character.inventory.clear();
    st.character
        .give_item(&st.content, ItemId::new("stim_patch"), 1);
    st.character
        .give_item(&st.content, ItemId::new("hushcan"), 1);
    let mut k = Kit::new(0);
    press(&mut k, &st, KeyCode::Char(']'), 1);
    assert_eq!(k.state.filter, Some(ItemKind::Consumable));
    press(&mut k, &st, KeyCode::Char(']'), 1);
    assert_eq!(k.state.filter, Some(ItemKind::Hardware));
    assert_eq!(texts(&k, &st), ["≡  1× Hushcan"]);
    press(&mut k, &st, KeyCode::Char(']'), 1);
    assert_eq!(k.state.filter, None);
    assert_eq!(
        texts(&k, &st),
        [
            "── consumable ──",
            "◍  1× Stim patch",
            "── hardware ──",
            "≡  1× Hushcan"
        ]
    );
    press(&mut k, &st, KeyCode::Char('['), 1);
    assert_eq!(k.state.filter, Some(ItemKind::Hardware));
    assert_eq!(k.state.pane, Pane::Body);
}

#[test]
fn selected_follows_the_pane() {
    let mut st = state();
    let mut k = Kit::new(0);
    k.state.part = BodyPart::RightHand;
    assert_eq!(k.selected(&st), Some(Detail::Part(BodyPart::RightHand)));
    st.character
        .give_item(&st.content, ItemId::new("hushcan"), 1);
    to_row(&mut k, &st, "Hushcan");
    assert!(matches!(k.selected(&st), Some(Detail::Copy(_))));
}

#[test]
fn space_on_a_tile_or_a_row_opens_its_popup() {
    let mut st = state();
    let mut k = Kit::new(0);
    k.state.part = BodyPart::RightHand;
    let p = k.interact(&mut st).expect("popup");
    let labels: Vec<_> = p.choices().iter().map(|c| c.label.clone()).collect();
    assert!(labels.iter().any(|l| l == "Unequip"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Inspect / Mod"), "{labels:?}");
    to_row(&mut k, &st, "Stim patch");
    let p = k.interact(&mut st).expect("popup");
    let labels: Vec<_> = p.choices().iter().map(|c| c.label.clone()).collect();
    assert_eq!(labels, ["Inspect / Mod", "Use", "Drop one"]);
}

#[test]
fn space_on_a_hand_offers_what_fits_a_hand_and_wears_it_there() {
    let mut st = state();
    for id in ["scrap_nine", "whisper_smg", "shiv_blade"] {
        st.character.give_item(&st.content, ItemId::new(id), 1);
    }
    let mut k = Kit::new(0);
    k.state.part = BodyPart::LeftHand;
    let p = k.interact(&mut st).expect("popup");
    let mut labels: Vec<_> = p.choices().iter().map(|c| c.label.clone()).collect();
    labels.sort();
    assert_eq!(labels, ["Monowire shiv", "Scrap nine", "Whisper SMG"]);
    let nine = p
        .choices()
        .iter()
        .find(|c| c.label == "Scrap nine")
        .unwrap();
    let Pick::Act(act) = nine.pick.clone() else {
        panic!("a wear")
    };
    assert!(
        matches!(&act, Action::EquipAt(id, BodyPart::LeftHand) if id.0 == "scrap_nine"),
        "{act:?}"
    );
    perform(&mut st, act).unwrap();
    assert_eq!(
        st.character.equipped(BodyPart::LeftHand),
        Some(&ItemId::new("scrap_nine"))
    );
    // The feet take boots, and the pack has none.
    k.state.part = BodyPart::Feet;
    assert!(k.interact(&mut st).is_none());
}

#[test]
fn an_empty_pack_selects_nothing() {
    let mut st = state();
    st.character.inventory.clear();
    let mut k = Kit::new(0);
    k.state.pane = Pane::Pack;
    k.reselect(&st);
    assert!(k.pack_rows(&st).is_empty());
    assert_eq!(k.selected(&st), None);
    assert!(k.interact(&mut st).is_none());
    for c in ['j', 'k', ']', '['] {
        press(&mut k, &st, KeyCode::Char(c), 1);
    }
    press(&mut k, &st, KeyCode::Down, 1);
    press(&mut k, &st, KeyCode::Up, 1);
}

#[test]
fn a_filter_left_empty_stays_until_stepped() {
    let mut st = state();
    let mut k = Kit::new(0);
    k.state.filter = Some(ItemKind::Hardware);
    let content = st.content.clone();
    st.character.inventory.retain(|s| {
        content
            .item(&s.item)
            .is_some_and(|i| i.kind != ItemKind::Hardware)
    });
    k.reselect(&st);
    assert_eq!(k.state.filter, Some(ItemKind::Hardware));
    assert!(k.pack_rows(&st).is_empty());
    // Stepped: on to the next kind the pack holds, his stickers, his drones, then
    // all.
    press(&mut k, &st, KeyCode::Char(']'), 1);
    assert_eq!(k.state.filter, Some(ItemKind::Sticker));
    press(&mut k, &st, KeyCode::Char(']'), 1);
    assert_eq!(k.state.filter, Some(ItemKind::Bot));
    press(&mut k, &st, KeyCode::Char(']'), 1);
    assert_eq!(k.state.filter, None);
}

#[test]
fn a_row_that_leaves_moves_the_cursor_to_a_row() {
    let mut st = state();
    let mut k = Kit::new(0);
    to_row(&mut k, &st, "Pawnshop deck");
    // Held per copy now: it carries stickers.
    let Some(Detail::Copy(copy)) = k.selected(&st) else {
        panic!("no pawnshop deck selected");
    };
    perform(&mut st, Action::EquipCopy(copy)).unwrap();
    k.reselect(&st);
    let rows = k.pack_rows(&st);
    assert!(rows[k.state.row].detail.is_some());
    assert!(k.selected(&st).is_some());
}

#[test]
fn the_pack_draws_its_chips_then_its_list() {
    let mut st = state();
    let mut k = Kit::new(0);
    k.state.pane = Pane::Pack;
    k.reselect(&st);
    settle(&k, &mut st);
    let buf = drawn(&k, &st);
    let chips = from(&buf, 51, 5);
    assert!(chips.starts_with("▌ALL▐ /  ▣  ◈  ⌗ "), "{chips:?}");
    assert_eq!(buf[(57, 5)].fg, FAINTER, "no weapon in the pack");
    assert_eq!(buf[(66, 5)].fg, DIM, "a deck in the pack");

    // Back from ALL: his drones, the last kind, his stickers, then the hardware.
    assert!(k.key(&st, KeyCode::Char('['), k.changed));
    assert_eq!(k.state.filter, Some(ItemKind::Bot));
    assert!(k.key(&st, KeyCode::Char('['), k.changed));
    assert_eq!(k.state.filter, Some(ItemKind::Sticker));
    assert!(k.key(&st, KeyCode::Char('['), k.changed));
    assert_eq!(k.state.filter, Some(ItemKind::Hardware));
    let buf = drawn(&k, &st);
    let chips = from(&buf, 51, 5);
    assert!(chips.starts_with(" ALL "), "{chips:?}");
    assert!(chips.contains("▌≡ HARDWARE▐"), "{chips:?}");
    let first = from(&buf, 50, 6);
    assert!(first.contains("▸ ≡  1× Stock fuse"), "{first:?}");
    assert_eq!(buf[(51, 6)].bg, BAR_BG);
}

#[test]
fn the_rule_carries_the_packs_weight() {
    let mut st = state();
    let k = Kit::new(0);
    settle(&k, &mut st);
    let buf = drawn(&k, &st);
    assert_eq!(buf[(49, 14)].symbol(), "╠");
    assert_eq!((buf[(49, 13)].symbol(), buf[(49, 26)].symbol()), ("║", "║"));
    for x in 50..60 {
        assert_eq!((buf[(x, 14)].symbol(), buf[(x, 14)].fg), ("═", RULE));
    }
    let total = format!(
        " {:.1} kg · ¤ {} ",
        st.character.weight(&st.content),
        st.character.value(&st.content)
    );
    let n = total.chars().count() as u16;
    assert_eq!(cells(&buf, 97 - n, 14, n), total, "ends at x 96");
}

#[test]
fn the_card_sits_under_the_pack_with_the_drawing_under_it() {
    let mut st = state();
    fit(&mut st, BodyPart::RightHand, 0, "hushcan");
    fit(&mut st, BodyPart::RightHand, 1, "redeye");
    let mut k = Kit::new(0);
    k.state.part = BodyPart::RightHand;
    settle(&k, &mut st);
    let buf = drawn_at(&k, &st, 120, 40);
    assert!(from(&buf, 65, 22).starts_with("RATCHET SMG"));
    let meta = from(&buf, 65, 23);
    assert!(
        meta.starts_with("weapon · right hand · main · 3.9 kg"),
        "{meta:?}"
    );
    corners_in(&buf, Rect::new(65, 29, 54, 8), RULE);
    assert_eq!(cells(&buf, 104, 36, 13), "space inspect");
    let bare = [PANEL_BG, Color::Reset];
    let drawing = (66..118).any(|x| {
        (30..36).any(|y| {
            let c = &buf[(x, y)];
            !bare.contains(&c.fg) || !bare.contains(&c.bg)
        })
    });
    assert!(drawing, "the Ratchet is drawn at half size");
    let card: String = (22..29).map(|y| from(&buf, 65, y)).collect();
    assert!(
        !card.contains("Stamped-receiver"),
        "the blurb waits: {card}"
    );

    // At 100×30 the card has the whole space, and the drawing waits for Inspect.
    let buf = drawn(&k, &st);
    assert!(from(&buf, 51, 15).starts_with("RATCHET SMG"));
    let card: String = (15..26).map(|y| from(&buf, 51, y)).collect();
    assert!(card.contains("Stamped-receiver"), "{card}");
    assert_eq!(cells(&buf, 84, 26, 13), "space inspect");
    assert_ne!(buf[(97, 26)].symbol(), "┘", "no viewer");
}

#[test]
fn an_item_without_a_drawing_shows_the_missing_box() {
    let mut st = state();
    st.character
        .wear(BodyPart::RightHand, ItemId::new("scrap_nine"), None);
    let mut k = Kit::new(0);
    k.state.part = BodyPart::RightHand;
    settle(&k, &mut st);
    let buf = drawn_at(&k, &st, 120, 40);
    corners_in(&buf, Rect::new(81, 30, 21, 5), FAINTER);
    assert_eq!((buf[(82, 30)].symbol(), buf[(82, 30)].fg), ("╌", FAINTER));
    assert_eq!((buf[(81, 31)].symbol(), buf[(81, 31)].fg), ("╎", FAINTER));
    let icon = kind_icon(game::ItemKind::Weapon).to_string();
    assert_eq!(
        (buf[(91, 31)].symbol(), buf[(91, 31)].fg),
        (icon.as_str(), DIM)
    );
    assert_eq!(cells(&buf, 83, 33, 17), "MISSING BLUEPRINT");
    assert_eq!(buf[(83, 33)].fg, FAINTER);
}

/// The viewer at 120×40.
const VIEWER: Rect = Rect::new(65, 29, 54, 8);

fn viewer_text(buf: &Buffer) -> String {
    (VIEWER.y..VIEWER.bottom())
        .map(|y| cells(buf, VIEWER.x, y, VIEWER.width))
        .collect()
}

#[test]
fn worn_armor_shows_its_picture() {
    for (part, item) in [
        (BodyPart::Head, "hardhat"),
        (BodyPart::Face, "mirrorshades"),
        (BodyPart::Torso, "salvage_jacket"),
        (BodyPart::Legs, "cargo_pants"),
    ] {
        let mut st = state();
        st.character.wear(part, ItemId::new(item), None);
        let mut k = Kit::new(0);
        k.state.part = part;
        settle(&k, &mut st);
        let buf = drawn_at(&k, &st, 120, 40);
        assert!(!viewer_text(&buf).contains("MISSING"), "{item}");
        let picture = st.content.picture(&ItemId::new(item)).unwrap();
        let colours: Vec<Color> = picture
            .colours
            .values()
            .map(|&(r, g, b)| Color::Rgb(r, g, b))
            .collect();
        let painted = (VIEWER.y..VIEWER.bottom())
            .flat_map(|y| (VIEWER.x..VIEWER.right()).map(move |x| (x, y)))
            .filter(|&p| colours.contains(&buf[p].bg) || colours.contains(&buf[p].fg))
            .count();
        assert!(painted > 40, "{item}: {painted} cells drawn");
    }
}

#[test]
fn the_tallest_picture_stays_in_the_viewer() {
    let mut st = state();
    st.character
        .wear(BodyPart::Torso, ItemId::new("salvage_jacket"), None);
    let tall = st.content.picture(&ItemId::new("salvage_jacket")).unwrap();
    assert_eq!(
        tall.rows.len(),
        game::PICTURE_MAX_H,
        "the jacket is the tallest"
    );
    let mut k = Kit::new(0);
    k.state.part = BodyPart::Torso;
    settle(&k, &mut st);
    let with = drawn_at(&k, &st, 120, 40);
    st.content.pictures.clear();
    let without = drawn_at(&k, &st, 120, 40);
    for y in 0..40 {
        for x in 0..120 {
            if !VIEWER.contains((x, y).into()) {
                assert_eq!(with[(x, y)], without[(x, y)], "({x}, {y})");
            }
        }
    }
}

#[test]
fn an_empty_part_shows_no_drawing() {
    let mut st = state();
    assert!(st.character.equipped(BodyPart::Head).is_none());
    let mut k = Kit::new(0);
    settle(&k, &mut st);
    let bare = |buf: &Buffer| {
        let card: String = (22..37).map(|y| from(buf, 65, y)).collect();
        assert!(!card.contains("space inspect"), "{card}");
        assert!(!card.contains('┘'), "{card}");
    };
    let buf = drawn_at(&k, &st, 120, 40);
    assert!(from(&buf, 65, 22).starts_with("— EMPTY —"));
    bare(&buf);

    st.character.inventory.clear();
    k.state.pane = Pane::Pack;
    k.reselect(&st);
    let buf = drawn_at(&k, &st, 120, 40);
    assert!(from(&buf, 65, 22).starts_with("nothing here"));
    bare(&buf);
}

#[test]
fn nothing_spills_outside_the_body() {
    let mut st = state();
    third_arm(&mut st);
    let area = Rect::new(0, 0, 160, 50);
    let last = *ItemKind::ALL.last().unwrap();
    for body in [
        Rect::new(1, 5, 98, 22),
        Rect::new(1, 5, 99, 23),
        Rect::new(1, 10, 118, 27),
        Rect::new(1, 10, 158, 37),
    ] {
        for pane in [Pane::Body, Pane::Pack] {
            let mut k = Kit::new(0);
            k.state.pane = pane;
            k.state.filter = Some(ItemKind::Hardware);
            k.reselect(&st);
            settle(&k, &mut st);
            let mut buf = Buffer::empty(area);
            for c in &mut buf.content {
                c.set_char('X');
            }
            k.draw(&st, body, &mut buf);
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    if !body.contains((x, y).into()) {
                        assert_eq!(buf[(x, y)].symbol(), "X", "{body:?} {pane:?} ({x},{y})");
                    }
                }
            }
        }
        // Every chip row fits the pack, the widest active chip included.
        let pack = layout(body, &parts(&st)).pack;
        let filters = std::iter::once(None).chain(ItemKind::ALL.into_iter().map(Some));
        for filter in filters {
            let mut k = Kit::new(0);
            k.state.filter = filter;
            let mut buf = Buffer::empty(area);
            k.draw(&st, body, &mut buf);
            let row = cells(&buf, pack.x, pack.y, pack.width);
            let active = filter.map_or("▌ALL▐".to_string(), |kind| {
                format!("▌{} {}▐", kind_icon(kind), kind.label().to_uppercase())
            });
            assert!(row.contains(&active), "{body:?} {row:?}");
            let end = if filter == Some(last) {
                '▐'
            } else {
                kind_icon(last)
            };
            assert!(row.trim_end().ends_with(end), "{body:?} {row:?}");
        }
    }
}

#[test]
fn filter_keys_retype_the_card_only_from_the_pack() {
    let st = state();
    let mut k = Kit::new(0);
    press(&mut k, &st, KeyCode::Char(']'), 7);
    assert_eq!(k.changed, 0, "the body's card stays as it was");
    k.state.pane = Pane::Pack;
    press(&mut k, &st, KeyCode::Char(']'), 9);
    assert_eq!(k.changed, 9);
}
