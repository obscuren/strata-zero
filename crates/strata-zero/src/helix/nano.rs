//! nano on HELIX: a small editor over the vfs, drawn on the CRT's glass.
// hosted by the terminal screen in Task 6
#![allow(dead_code)]

use super::vfs::{Kind, Vfs, VfsError};
use crate::ui::chrome;
use crate::ui::device::Look;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Edit,
    ExitPrompt,
    SavePrompt(String),
    Search(String),
    Help,
}

pub struct Nano {
    pub path: String,
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
    pub top: usize,
    pub modified: bool,
    pub readonly: bool,
    pub cut: Vec<String>,
    pub mode: Mode,
    pub status: String,
    last_cut_row: Option<usize>,
}

pub enum NanoEvent {
    Stay,
    Closed(Vec<String>),
}

/// Rows of text a screen of the real glass holds (20 rows less title, status, 2 shortcuts).
const PAGE: usize = 16;

fn byte_at(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map_or(s.len(), |(i, _)| i)
}

fn len(s: &str) -> usize {
    s.chars().count()
}

fn is_ctrl(k: &KeyEvent, c: char) -> bool {
    k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char(c)
}

impl Nano {
    pub fn open(fs: &Vfs, user: &str, cwd: &str, home: &str, path: &str) -> Result<Nano, String> {
        let abs = Vfs::normalize(cwd, home, path);
        let mut nano = Nano {
            path: abs.clone(),
            lines: vec![String::new()],
            row: 0,
            col: 0,
            top: 0,
            modified: false,
            readonly: false,
            cut: Vec::new(),
            mode: Mode::Edit,
            status: String::new(),
            last_cut_row: None,
        };
        match fs.read(user, "/", "/", &abs) {
            Ok(Kind::File(text)) => {
                nano.lines = text.lines().map(str::to_string).collect();
                if nano.lines.is_empty() {
                    nano.lines.push(String::new());
                }
                let writable = fs
                    .resolve_as(user, "/", "/", &abs)
                    .is_ok_and(|id| fs.can_write(id, user));
                if !writable {
                    nano.readonly = true;
                    nano.status = format!("[ File '{path}' is unwritable ]");
                }
            }
            Ok(_) => {
                nano.readonly = true;
                nano.status = format!("[ File '{path}' is unwritable ]");
            }
            Err(VfsError::IsADirectory) => return Err(format!("[ '{path}' is a directory ]")),
            Err(VfsError::NotFound) => {
                let dir = abs.rsplit_once('/').map_or("/", |(d, _)| d);
                let dir = if dir.is_empty() { "/" } else { dir };
                match fs.resolve_as(user, "/", "/", dir) {
                    Ok(id) if matches!(fs.node(id).kind, Kind::Dir(_)) => {
                        if !fs.can_write(id, user) {
                            nano.readonly = true;
                            nano.status = format!("[ Directory '{dir}' is not writable ]");
                        }
                    }
                    Ok(_) => return Err(format!("[ Error reading {path}: Not a directory ]")),
                    Err(e) => return Err(format!("[ Error reading {path}: {e} ]")),
                }
            }
            Err(e) => return Err(format!("[ Error reading {path}: {e} ]")),
        }
        Ok(nano)
    }

    fn line_len(&self) -> usize {
        len(&self.lines[self.row])
    }

    fn clamp(&mut self) {
        self.row = self.row.min(self.lines.len() - 1);
        self.col = self.col.min(self.line_len());
        if self.row < self.top {
            self.top = self.row;
        } else if self.row >= self.top + PAGE {
            self.top = self.row + 1 - PAGE;
        }
    }

    fn text(&self) -> String {
        if self.lines.len() == 1 && self.lines[0].is_empty() {
            return String::new();
        }
        let mut t = self.lines.join("\n");
        t.push('\n');
        t
    }

    /// Write the buffer to `path`; the status says how it went.
    fn save(&mut self, fs: &mut Vfs, user: &str, path: &str) -> bool {
        if matches!(fs.read(user, "/", "/", path), Ok(Kind::Dev(_))) {
            self.status = format!("[ Error writing {path}: File is unwritable ]");
            return false;
        }
        match fs.write(user, "/", "/", path, &self.text(), false) {
            Ok(()) => {
                let n = if self.text().is_empty() {
                    0
                } else {
                    self.lines.len()
                };
                self.path = Vfs::normalize("/", "/", path);
                self.modified = false;
                self.readonly = false;
                self.status = format!("[ Wrote {n} lines ]");
                true
            }
            Err(e) => {
                self.status = format!("[ Error writing {path}: {e} ]");
                false
            }
        }
    }

    fn find(&mut self, needle: &str) {
        if needle.is_empty() {
            return;
        }
        let n = self.lines.len();
        for i in 0..=n {
            let r = (self.row + i) % n;
            let line = &self.lines[r];
            // the first pass starts after the cursor, the wrap-around pass covers the rest
            let from = if i == 0 {
                byte_at(line, self.col + 1)
            } else {
                0
            };
            let limit = if i == n {
                byte_at(line, self.col + 1)
            } else {
                line.len()
            };
            if from > limit {
                continue;
            }
            if let Some(at) = line[from..limit].find(needle) {
                self.row = r;
                self.col = len(&line[..from + at]);
                self.status.clear();
                self.clamp();
                return;
            }
        }
        self.status = format!("[ \"{needle}\" not found ]");
    }

    pub fn key(&mut self, fs: &mut Vfs, user: &str, k: KeyEvent) -> NanoEvent {
        match self.mode.clone() {
            Mode::Help => {
                self.mode = Mode::Edit;
                NanoEvent::Stay
            }
            Mode::ExitPrompt => {
                match k.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        let p = self.path.clone();
                        if self.save(fs, user, &p) {
                            return NanoEvent::Closed(vec![self.status.clone()]);
                        }
                        self.mode = Mode::Edit;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        return NanoEvent::Closed(Vec::new());
                    }
                    _ if is_ctrl(&k, 'c') || k.code == KeyCode::Esc => self.mode = Mode::Edit,
                    _ => {}
                }
                NanoEvent::Stay
            }
            Mode::SavePrompt(mut name) => {
                match k.code {
                    KeyCode::Enter => {
                        self.mode = Mode::Edit;
                        if !name.is_empty() {
                            self.save(fs, user, &name);
                        }
                    }
                    KeyCode::Esc => {
                        self.mode = Mode::Edit;
                        self.status = "[ Cancelled ]".into();
                    }
                    _ if is_ctrl(&k, 'c') => {
                        self.mode = Mode::Edit;
                        self.status = "[ Cancelled ]".into();
                    }
                    KeyCode::Backspace => {
                        name.pop();
                        self.mode = Mode::SavePrompt(name);
                    }
                    KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                        name.push(c);
                        self.mode = Mode::SavePrompt(name);
                    }
                    _ => {}
                }
                NanoEvent::Stay
            }
            Mode::Search(mut term) => {
                match k.code {
                    KeyCode::Enter => {
                        self.mode = Mode::Edit;
                        self.find(&term);
                    }
                    KeyCode::Esc => self.mode = Mode::Edit,
                    _ if is_ctrl(&k, 'c') => self.mode = Mode::Edit,
                    KeyCode::Backspace => {
                        term.pop();
                        self.mode = Mode::Search(term);
                    }
                    KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                        term.push(c);
                        self.mode = Mode::Search(term);
                    }
                    _ => {}
                }
                NanoEvent::Stay
            }
            Mode::Edit => self.edit_key(k),
        }
    }

    fn edit_key(&mut self, k: KeyEvent) -> NanoEvent {
        let was_cut = self.last_cut_row.take();
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('x') => {
                    if self.modified {
                        self.mode = Mode::ExitPrompt;
                    } else {
                        return NanoEvent::Closed(Vec::new());
                    }
                }
                KeyCode::Char('o') => self.mode = Mode::SavePrompt(self.path.clone()),
                KeyCode::Char('w') => self.mode = Mode::Search(String::new()),
                KeyCode::Char('g') => self.mode = Mode::Help,
                KeyCode::Char('k') => {
                    let line = self.lines.remove(self.row);
                    if was_cut == Some(self.row) {
                        self.cut.push(line);
                    } else {
                        self.cut = vec![line];
                    }
                    if self.lines.is_empty() {
                        self.lines.push(String::new());
                    }
                    self.col = 0;
                    self.modified = true;
                    self.clamp();
                    self.last_cut_row = Some(self.row);
                }
                KeyCode::Char('u') => {
                    if self.cut.is_empty() {
                        self.status = "[ Cut buffer is empty ]".into();
                    } else {
                        let n = self.cut.len();
                        for (i, l) in self.cut.clone().into_iter().enumerate() {
                            self.lines.insert(self.row + i, l);
                        }
                        self.row += n;
                        self.col = 0;
                        self.modified = true;
                        self.clamp();
                    }
                }
                _ => {}
            }
            return NanoEvent::Stay;
        }
        match k.code {
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::ALT) => {
                let at = byte_at(&self.lines[self.row], self.col);
                self.lines[self.row].insert(at, c);
                self.col += 1;
                self.modified = true;
            }
            KeyCode::Tab => {
                let at = byte_at(&self.lines[self.row], self.col);
                self.lines[self.row].insert_str(at, "    ");
                self.col += 4;
                self.modified = true;
            }
            KeyCode::Enter => {
                let at = byte_at(&self.lines[self.row], self.col);
                let rest = self.lines[self.row].split_off(at);
                self.lines.insert(self.row + 1, rest);
                self.row += 1;
                self.col = 0;
                self.modified = true;
            }
            KeyCode::Backspace => {
                if self.col > 0 {
                    let at = byte_at(&self.lines[self.row], self.col - 1);
                    self.lines[self.row].remove(at);
                    self.col -= 1;
                    self.modified = true;
                } else if self.row > 0 {
                    let line = self.lines.remove(self.row);
                    self.row -= 1;
                    self.col = self.line_len();
                    self.lines[self.row].push_str(&line);
                    self.modified = true;
                }
            }
            KeyCode::Delete => {
                if self.col < self.line_len() {
                    let at = byte_at(&self.lines[self.row], self.col);
                    self.lines[self.row].remove(at);
                    self.modified = true;
                } else if self.row + 1 < self.lines.len() {
                    let next = self.lines.remove(self.row + 1);
                    self.lines[self.row].push_str(&next);
                    self.modified = true;
                }
            }
            KeyCode::Left => {
                if self.col > 0 {
                    self.col -= 1;
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.line_len();
                }
            }
            KeyCode::Right => {
                if self.col < self.line_len() {
                    self.col += 1;
                } else if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
            }
            KeyCode::Up => self.row = self.row.saturating_sub(1),
            KeyCode::Down => self.row = (self.row + 1).min(self.lines.len() - 1),
            KeyCode::Home => self.col = 0,
            KeyCode::End => self.col = self.line_len(),
            KeyCode::PageUp => self.row = self.row.saturating_sub(PAGE),
            KeyCode::PageDown => self.row = (self.row + PAGE).min(self.lines.len() - 1),
            _ => {}
        }
        self.clamp();
        NanoEvent::Stay
    }

    pub fn draw(&self, buf: &mut Buffer, glass: Rect, look: &Look) {
        if glass.width < 8 || glass.height < 6 {
            return;
        }
        let (x0, w) = (glass.x, glass.width as usize);
        let max_x = glass.x + glass.width;
        let body = Style::default().fg(look.text()).bg(look.glass());
        let dim = Style::default().fg(look.text_dim()).bg(look.glass());
        let bar = Style::default().fg(look.glass()).bg(look.text());
        for y in glass.y..glass.y + glass.height {
            chrome::put(buf, x0, y, &" ".repeat(w), body, max_x);
        }
        // title
        let name = self.path.rsplit('/').next().unwrap_or(&self.path);
        let flag = if self.modified { "  Modified" } else { "" };
        let title = format!("  HELIX nano 2.2   File: {name}{flag}");
        chrome::put(buf, x0, glass.y, &" ".repeat(w), bar, max_x);
        chrome::put(buf, x0, glass.y, &title, bar, max_x);

        let page = glass.height as usize - 4;
        let status_y = glass.y + glass.height - 3;
        if self.mode == Mode::Help {
            let help = [
                "  Main nano help text",
                "",
                "  ^G  This help          ^O  Write the buffer out",
                "  ^X  Exit               ^W  Where Is (search)",
                "  ^K  Cut the line       ^U  Paste the cut lines",
                "",
                "  Any key returns to the buffer.",
            ];
            for (i, l) in help.iter().take(page).enumerate() {
                chrome::put(buf, x0, glass.y + 1 + i as u16, l, body, max_x);
            }
        } else {
            let top = if self.row < self.top {
                self.row
            } else if self.row >= self.top + page {
                self.row + 1 - page
            } else {
                self.top
            };
            let left = if self.col + 1 < w {
                0
            } else {
                self.col + 2 - w
            };
            for i in 0..page {
                let Some(line) = self.lines.get(top + i) else {
                    break;
                };
                let n = len(line);
                let mut shown: String = line.chars().skip(left).take(w).collect();
                if n > left + w {
                    shown = shown.chars().take(w - 1).collect();
                    shown.push('$');
                }
                chrome::put(buf, x0, glass.y + 1 + i as u16, &shown, body, max_x);
            }
            let cx = x0 + (self.col - left) as u16;
            let cy = glass.y + 1 + (self.row - top) as u16;
            if cx < max_x && self.mode == Mode::Edit {
                let c = &mut buf[(cx, cy)];
                c.set_style(c.style().add_modifier(Modifier::REVERSED));
            }
        }
        let status = match &self.mode {
            Mode::ExitPrompt => "Save modified buffer? (Y/N)".to_string(),
            Mode::SavePrompt(n) => format!("File Name to Write: {n}"),
            Mode::Search(t) => format!("Search: {t}"),
            _ => self.status.clone(),
        };
        chrome::put(buf, x0, status_y, &format!("  {status}"), body, max_x);
        let (r1, r2) = match &self.mode {
            Mode::ExitPrompt => (" Y Yes   N No   ^C Cancel", ""),
            Mode::SavePrompt(_) | Mode::Search(_) => ("^C Cancel", ""),
            _ => (
                "^G Help    ^O Write Out  ^W Where Is  ^K Cut",
                "^X Exit    ^U Paste",
            ),
        };
        chrome::put(buf, x0, status_y + 1, r1, dim, max_x);
        chrome::put(buf, x0, status_y + 2, r2, dim, max_x);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::helix::vfs::{Dev, GUEST, Mode as FsMode, ROOT, USERS};

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }
    fn type_str(n: &mut Nano, fs: &mut Vfs, s: &str) {
        for c in s.chars() {
            n.key(fs, "mara", k(KeyCode::Char(c)));
        }
    }
    fn fixture() -> Vfs {
        let mut fs = Vfs::new();
        fs.mkdir_as("/home", ROOT, FsMode::RW_R);
        fs.mkdir_as("/home/mara", "mara", FsMode::RW_R);
        fs.mkdir_as("/etc", ROOT, FsMode::RW_R);
        fs.mkdir_as("/secret", ROOT, FsMode::RW_NONE);
        fs.put(
            "/home/mara/notes.txt",
            "one\ntwo\nthree\n",
            "mara",
            FsMode::RW_R,
        );
        fs.put("/etc/motd", "welcome\n", ROOT, FsMode::RW_R);
        fs.put("/secret/x", "x\n", ROOT, FsMode::RW_NONE);
        fs.put_dev("/dev/door", Dev::Door, USERS, FsMode::RW_R);
        fs.put_dev("/home/mara/journal.txt", Dev::Journal, "mara", FsMode::RW_R);
        fs
    }
    fn open(fs: &Vfs, path: &str) -> Nano {
        Nano::open(fs, "mara", "/home/mara", "/home/mara", path).unwrap()
    }
    fn buffer_text(buf: &Buffer) -> String {
        let a = buf.area;
        (a.y..a.y + a.height)
            .map(|y| {
                (a.x..a.x + a.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn opens_existing_and_new_files_and_refuses_directories() {
        let fs = fixture();
        let n = open(&fs, "notes.txt");
        assert_eq!(n.lines, vec!["one", "two", "three"]);
        assert!(!n.readonly && !n.modified);
        let n = open(&fs, "fresh.txt");
        assert_eq!(n.lines, vec![""]);
        assert!(!n.readonly);
        assert_eq!(
            Nano::open(&fs, "mara", "/home/mara", "/home/mara", "/etc").err(),
            Some("[ '/etc' is a directory ]".to_string())
        );
        assert_eq!(
            Nano::open(&fs, "mara", "/", "/home/mara", "nope/x").err(),
            Some("[ Error reading nope/x: No such file or directory ]".to_string())
        );
        assert_eq!(
            Nano::open(&fs, "mara", "/", "/home/mara", "/secret/x").err(),
            Some("[ Error reading /secret/x: Permission denied ]".to_string())
        );
        let n = Nano::open(&fs, "mara", "/", "/home/mara", "/etc/new").unwrap();
        assert!(n.readonly);
        assert_eq!(n.status, "[ Directory '/etc' is not writable ]");
    }

    #[test]
    fn typing_enter_backspace_and_arrows_edit_the_buffer() {
        let mut fs = fixture();
        let mut n = open(&fs, "fresh.txt");
        type_str(&mut n, &mut fs, "hello");
        assert!(n.modified);
        n.key(&mut fs, "mara", k(KeyCode::Left));
        n.key(&mut fs, "mara", k(KeyCode::Enter));
        assert_eq!(n.lines, vec!["hell", "o"]);
        assert_eq!((n.row, n.col), (1, 0));
        n.key(&mut fs, "mara", k(KeyCode::Backspace));
        assert_eq!(n.lines, vec!["hello"]);
        assert_eq!((n.row, n.col), (0, 4));
        n.key(&mut fs, "mara", k(KeyCode::Home));
        n.key(&mut fs, "mara", k(KeyCode::Delete));
        assert_eq!(n.lines, vec!["ello"]);
        n.key(&mut fs, "mara", k(KeyCode::End));
        assert_eq!(n.col, 4);
        n.key(&mut fs, "mara", k(KeyCode::Down));
        n.key(&mut fs, "mara", k(KeyCode::Up));
        assert_eq!(n.row, 0);
    }

    #[test]
    fn cut_accumulates_and_paste_inserts() {
        let mut fs = fixture();
        let mut n = open(&fs, "notes.txt");
        n.key(&mut fs, "mara", ctrl('k'));
        n.key(&mut fs, "mara", ctrl('k'));
        assert_eq!(n.cut, vec!["one", "two"]);
        assert_eq!(n.lines, vec!["three"]);
        n.key(&mut fs, "mara", k(KeyCode::Down));
        n.key(&mut fs, "mara", ctrl('u'));
        assert_eq!(n.lines, vec!["one", "two", "three"]);
        // a cut after another key starts a new buffer
        n.key(&mut fs, "mara", ctrl('k'));
        assert_eq!(n.cut, vec!["three"]);
    }

    #[test]
    fn search_wraps() {
        let mut fs = fixture();
        let mut n = open(&fs, "notes.txt");
        n.key(&mut fs, "mara", k(KeyCode::Down));
        n.key(&mut fs, "mara", k(KeyCode::Down));
        n.key(&mut fs, "mara", ctrl('w'));
        assert!(matches!(n.mode, Mode::Search(_)));
        for c in "one".chars() {
            n.key(&mut fs, "mara", k(KeyCode::Char(c)));
        }
        n.key(&mut fs, "mara", k(KeyCode::Enter));
        assert_eq!(n.mode, Mode::Edit);
        assert_eq!((n.row, n.col), (0, 0));
        n.key(&mut fs, "mara", ctrl('w'));
        for c in "zzz".chars() {
            n.key(&mut fs, "mara", k(KeyCode::Char(c)));
        }
        n.key(&mut fs, "mara", k(KeyCode::Enter));
        assert!(n.status.contains("not found"));
    }

    #[test]
    fn write_out_reports_lines_and_clears_modified() {
        let mut fs = fixture();
        let mut n = open(&fs, "fresh.txt");
        type_str(&mut n, &mut fs, "a");
        n.key(&mut fs, "mara", k(KeyCode::Enter));
        type_str(&mut n, &mut fs, "b");
        n.key(&mut fs, "mara", ctrl('o'));
        assert_eq!(n.mode, Mode::SavePrompt("/home/mara/fresh.txt".into()));
        n.key(&mut fs, "mara", k(KeyCode::Enter));
        assert_eq!(n.mode, Mode::Edit);
        assert_eq!(n.status, "[ Wrote 2 lines ]");
        assert!(!n.modified);
        match fs.read("mara", "/", "/", "/home/mara/fresh.txt").unwrap() {
            Kind::File(t) => assert_eq!(t, "a\nb\n"),
            _ => panic!("not a file"),
        }
    }

    #[test]
    fn exit_asks_when_modified_and_saves_on_y() {
        let mut fs = fixture();
        let mut n = open(&fs, "notes.txt");
        assert!(matches!(
            n.key(&mut fs, "mara", ctrl('x')),
            NanoEvent::Closed(l) if l.is_empty()
        ));
        let mut n = open(&fs, "notes.txt");
        type_str(&mut n, &mut fs, "X");
        assert!(matches!(n.key(&mut fs, "mara", ctrl('x')), NanoEvent::Stay));
        assert_eq!(n.mode, Mode::ExitPrompt);
        n.key(&mut fs, "mara", ctrl('c'));
        assert_eq!(n.mode, Mode::Edit);
        n.key(&mut fs, "mara", ctrl('x'));
        match n.key(&mut fs, "mara", k(KeyCode::Char('y'))) {
            NanoEvent::Closed(l) => assert_eq!(l, vec!["[ Wrote 3 lines ]"]),
            NanoEvent::Stay => panic!("should close"),
        }
        match fs.read("mara", "/", "/", "/home/mara/notes.txt").unwrap() {
            Kind::File(t) => assert_eq!(t, "Xone\ntwo\nthree\n"),
            _ => panic!(),
        }
        let mut n = open(&fs, "notes.txt");
        type_str(&mut n, &mut fs, "Q");
        n.key(&mut fs, "mara", ctrl('x'));
        assert!(matches!(
            n.key(&mut fs, "mara", k(KeyCode::Char('n'))),
            NanoEvent::Closed(_)
        ));
    }

    #[test]
    fn readonly_and_device_files_cannot_save() {
        let mut fs = fixture();
        for (user, path) in [("mara", "/dev/door"), ("mara", "journal.txt")] {
            let mut n = Nano::open(&fs, user, "/home/mara", "/home/mara", path).unwrap();
            assert!(n.readonly);
            assert!(n.status.contains("is unwritable"), "{}", n.status);
            type_str(&mut n, &mut fs, "x");
            n.key(&mut fs, user, ctrl('o'));
            n.key(&mut fs, user, k(KeyCode::Enter));
            assert_eq!(n.mode, Mode::Edit);
            assert!(n.modified, "the failed save keeps the buffer dirty");
            assert!(n.status.starts_with("[ "), "{}", n.status);
            assert!(!n.status.contains("Wrote"), "{}", n.status);
        }
        let mut g = Nano::open(&fs, GUEST, "/", "/", "/etc/motd").unwrap();
        assert!(g.readonly);
        assert_eq!(g.status, "[ File '/etc/motd' is unwritable ]");
        g.key(
            &mut fs,
            GUEST,
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
        );
        g.key(&mut fs, GUEST, ctrl('o'));
        g.key(&mut fs, GUEST, k(KeyCode::Enter));
        assert!(g.status.contains("Permission denied"), "{}", g.status);
        match fs.read(ROOT, "/", "/", "/etc/motd").unwrap() {
            Kind::File(t) => assert_eq!(t, "welcome\n"),
            _ => panic!(),
        }
    }

    #[test]
    fn the_glass_shows_title_status_and_shortcuts() {
        let st = AppState::new();
        let look = Look::named(&st, "municipal", None, 1);
        let fs = fixture();
        let n = open(&fs, "notes.txt");
        let glass = Rect::new(0, 0, 68, 20);
        let mut buf = Buffer::empty(glass);
        n.draw(&mut buf, glass, &look);
        let text = buffer_text(&buf);
        assert!(text.contains("HELIX nano"));
        assert!(text.contains("notes.txt"));
        assert!(text.contains("^X Exit"));
        assert!(text.contains("three"));
        assert!(buf[(0, 1)].modifier.contains(Modifier::REVERSED));
        assert_eq!(buf[(0, 1)].symbol(), "o");
        assert!(!buf[(1, 1)].modifier.contains(Modifier::REVERSED));
    }
}
