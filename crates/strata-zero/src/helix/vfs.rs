//! The virtual filesystem: a tree of directories, files and device nodes with owners and
//! modes. Pure: no app state. Device nodes are only stored here; the shell renders them.

use citygen::BuildingId;
use std::collections::BTreeMap;
use std::fmt;

pub const ROOT: &str = "root";
pub const GUEST: &str = "guest";
/// The pseudo-owner whose owner bits belong to anyone but `guest`.
pub const USERS: &str = "users";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mode {
    pub owner_r: bool,
    pub owner_w: bool,
    pub other_r: bool,
    pub other_w: bool,
}

impl Mode {
    /// `rw-r--`
    pub const RW_R: Mode = Mode::new(true, true, true, false);
    /// `rw-rw-`
    pub const RW_RW: Mode = Mode::new(true, true, true, true);
    /// `rw----`
    pub const RW_NONE: Mode = Mode::new(true, true, false, false);
    /// `r--r--`
    pub const R_R: Mode = Mode::new(true, false, true, false);

    const fn new(owner_r: bool, owner_w: bool, other_r: bool, other_w: bool) -> Mode {
        Mode {
            owner_r,
            owner_w,
            other_r,
            other_w,
        }
    }

    fn bits(self) -> String {
        let b = |on: bool, c: char| if on { c } else { '-' };
        format!(
            "{}{}-{}{}-",
            b(self.owner_r, 'r'),
            b(self.owner_w, 'w'),
            b(self.other_r, 'r'),
            b(self.other_w, 'w')
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Dev {
    Door,
    Lift,
    Cam(usize),
    Alarm,
    Plan,
    AuthLog,
    NetLog,
    Journal,
    Mail(usize),
    NetHost(BuildingId),
    Passwd,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Dir(BTreeMap<String, usize>),
    File(String),
    Dev(Dev),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub name: String,
    pub kind: Kind,
    pub owner: String,
    pub mode: Mode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VfsError {
    NotFound,
    NotADirectory,
    IsADirectory,
    Denied,
    Exists,
    NotEmpty,
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            VfsError::NotFound => "No such file or directory",
            VfsError::NotADirectory => "Not a directory",
            VfsError::IsADirectory => "Is a directory",
            VfsError::Denied => "Permission denied",
            VfsError::Exists => "File exists",
            VfsError::NotEmpty => "Directory not empty",
        })
    }
}

/// `nodes[0]` is `/`. Removed nodes are unlinked from their parent and left in the vec,
/// so every id stays stable.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Vfs {
    nodes: Vec<Node>,
}

impl Default for Vfs {
    fn default() -> Self {
        Vfs::new()
    }
}

/// Split an absolute normalized path into its components.
fn parts(abs: &str) -> Vec<&str> {
    abs.split('/').filter(|s| !s.is_empty()).collect()
}

/// An absolute normalized path split into (parent path, final name); `None` for `/`.
fn split_last(abs: &str) -> Option<(String, String)> {
    let p = parts(abs);
    let (name, dir) = p.split_last()?;
    Some((format!("/{}", dir.join("/")), (*name).to_string()))
}

impl Vfs {
    pub fn new() -> Vfs {
        Vfs {
            nodes: vec![Node {
                name: "/".to_string(),
                kind: Kind::Dir(BTreeMap::new()),
                owner: ROOT.to_string(),
                mode: Mode::RW_R,
            }],
        }
    }

    pub fn normalize(cwd: &str, home: &str, path: &str) -> String {
        let full = if path == "~" {
            home.to_string()
        } else if let Some(rest) = path.strip_prefix("~/") {
            format!("{home}/{rest}")
        } else if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{cwd}/{path}")
        };
        let mut out: Vec<&str> = Vec::new();
        for part in full.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    out.pop();
                }
                p => out.push(p),
            }
        }
        format!("/{}", out.join("/"))
    }

    /// Walk to `abs`. With a user, every directory passed through must be readable by
    /// them (the final node's own permission is the caller's to check).
    fn walk(&self, user: Option<&str>, abs: &str) -> Result<usize, VfsError> {
        let mut at = 0;
        for part in parts(abs) {
            match &self.nodes[at].kind {
                Kind::Dir(children) => {
                    if user.is_some_and(|u| !self.can_read(at, u)) {
                        return Err(VfsError::Denied);
                    }
                    at = *children.get(part).ok_or(VfsError::NotFound)?
                }
                _ => return Err(VfsError::NotADirectory),
            }
        }
        Ok(at)
    }

    fn lookup(&self, abs: &str) -> Result<usize, VfsError> {
        self.walk(None, abs)
    }

    fn lookup_as(&self, user: &str, abs: &str) -> Result<usize, VfsError> {
        self.walk(Some(user), abs)
    }

    /// Like `resolve`, but denied when a directory on the way is unreadable by `user`.
    pub fn resolve_as(
        &self,
        user: &str,
        cwd: &str,
        home: &str,
        path: &str,
    ) -> Result<usize, VfsError> {
        self.lookup_as(user, &Vfs::normalize(cwd, home, path))
    }

    pub fn resolve(&self, cwd: &str, home: &str, path: &str) -> Result<usize, VfsError> {
        self.lookup(&Vfs::normalize(cwd, home, path))
    }

    pub fn node(&self, id: usize) -> &Node {
        &self.nodes[id]
    }

    pub fn path_of(&self, id: usize) -> String {
        fn walk(v: &Vfs, at: usize, target: usize, path: &mut Vec<String>) -> bool {
            if at == target {
                return true;
            }
            if let Kind::Dir(children) = &v.nodes[at].kind {
                for (name, &child) in children {
                    path.push(name.clone());
                    if walk(v, child, target, path) {
                        return true;
                    }
                    path.pop();
                }
            }
            false
        }
        let mut path = Vec::new();
        walk(self, 0, id, &mut path);
        format!("/{}", path.join("/"))
    }

    fn allowed(&self, id: usize, user: &str, read: bool) -> bool {
        if user == ROOT {
            return true;
        }
        let n = &self.nodes[id];
        let owner_bits = n.owner == user || (n.owner == USERS && user != GUEST);
        match (owner_bits, read) {
            (true, true) => n.mode.owner_r,
            (true, false) => n.mode.owner_w,
            (false, true) => n.mode.other_r,
            (false, false) => n.mode.other_w,
        }
    }

    pub fn can_read(&self, id: usize, user: &str) -> bool {
        self.allowed(id, user, true)
    }

    pub fn can_write(&self, id: usize, user: &str) -> bool {
        self.allowed(id, user, false)
    }

    fn add(&mut self, parent: usize, name: &str, kind: Kind, owner: &str, mode: Mode) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Node {
            name: name.to_string(),
            kind,
            owner: owner.to_string(),
            mode,
        });
        if let Kind::Dir(children) = &mut self.nodes[parent].kind {
            children.insert(name.to_string(), id);
        }
        id
    }

    fn children(&self, id: usize) -> Option<&BTreeMap<String, usize>> {
        match &self.nodes[id].kind {
            Kind::Dir(c) => Some(c),
            _ => None,
        }
    }

    /// The directory that holds `abs` and the final name, with the directory checked.
    fn parent_of(&self, user: &str, abs: &str) -> Result<(usize, String), VfsError> {
        let (dir, name) = split_last(abs).ok_or(VfsError::Denied)?;
        let pid = self.lookup_as(user, &dir)?;
        if self.children(pid).is_some() && !self.can_read(pid, user) {
            return Err(VfsError::Denied);
        }
        if self.children(pid).is_none() {
            return Err(VfsError::NotADirectory);
        }
        Ok((pid, name))
    }

    pub fn mkdir(
        &mut self,
        user: &str,
        cwd: &str,
        home: &str,
        path: &str,
        parents: bool,
    ) -> Result<usize, VfsError> {
        let abs = Vfs::normalize(cwd, home, path);
        let mut at = 0;
        let comps = parts(&abs);
        if comps.is_empty() {
            return if parents {
                Ok(0)
            } else {
                Err(VfsError::Exists)
            };
        }
        for (i, name) in comps.iter().enumerate() {
            let last = i + 1 == comps.len();
            if !self.can_read(at, user) {
                return Err(VfsError::Denied);
            }
            let next = self
                .children(at)
                .ok_or(VfsError::NotADirectory)?
                .get(*name)
                .copied();
            at = match next {
                Some(id) => {
                    if last {
                        if !parents {
                            return Err(VfsError::Exists);
                        }
                        if self.children(id).is_none() {
                            return Err(VfsError::Exists);
                        }
                    }
                    id
                }
                None => {
                    if !last && !parents {
                        return Err(VfsError::NotFound);
                    }
                    if !self.can_write(at, user) {
                        return Err(VfsError::Denied);
                    }
                    self.add(at, name, Kind::Dir(BTreeMap::new()), user, Mode::RW_R)
                }
            };
        }
        Ok(at)
    }

    /// Generator use: no checks, creates parents (root-owned `rw-r--`). An existing
    /// directory keeps its children and takes the new owner and mode.
    pub fn mkdir_as(&mut self, path: &str, owner: &str, mode: Mode) -> usize {
        let abs = Vfs::normalize("/", "/", path);
        let mut at = 0;
        let comps = parts(&abs);
        for (i, name) in comps.iter().enumerate() {
            let last = i + 1 == comps.len();
            let existing = self.children(at).and_then(|c| c.get(*name).copied());
            at = match existing {
                Some(id) if self.children(id).is_some() => id,
                Some(id) => {
                    self.nodes[id].kind = Kind::Dir(BTreeMap::new());
                    id
                }
                None => {
                    if last {
                        self.add(at, name, Kind::Dir(BTreeMap::new()), owner, mode)
                    } else {
                        self.add(at, name, Kind::Dir(BTreeMap::new()), ROOT, Mode::RW_R)
                    }
                }
            };
            if last {
                self.nodes[at].owner = owner.to_string();
                self.nodes[at].mode = mode;
            }
        }
        at
    }

    pub fn write(
        &mut self,
        user: &str,
        cwd: &str,
        home: &str,
        path: &str,
        text: &str,
        append: bool,
    ) -> Result<(), VfsError> {
        let abs = Vfs::normalize(cwd, home, path);
        match self.lookup_as(user, &abs) {
            Ok(id) => {
                if self.children(id).is_some() {
                    return Err(VfsError::IsADirectory);
                }
                if !self.can_write(id, user) {
                    return Err(VfsError::Denied);
                }
                if let Kind::File(body) = &mut self.nodes[id].kind {
                    if !append {
                        body.clear();
                    }
                    body.push_str(text);
                }
                Ok(())
            }
            Err(VfsError::NotFound) => {
                let (pid, name) = self.parent_of(user, &abs)?;
                if !self.can_write(pid, user) {
                    return Err(VfsError::Denied);
                }
                self.add(pid, &name, Kind::File(text.to_string()), user, Mode::RW_R);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn put_node(&mut self, path: &str, kind: Kind, owner: &str, mode: Mode) {
        let abs = Vfs::normalize("/", "/", path);
        let Some((dir, name)) = split_last(&abs) else {
            return;
        };
        let pid = self.mkdir_parents(&dir);
        let existing = self.children(pid).and_then(|c| c.get(&name).copied());
        match existing {
            Some(id) => {
                let n = &mut self.nodes[id];
                n.kind = kind;
                n.owner = owner.to_string();
                n.mode = mode;
            }
            None => {
                self.add(pid, &name, kind, owner, mode);
            }
        }
    }

    /// Make a directory path as root without touching the owner of ones that exist.
    fn mkdir_parents(&mut self, abs: &str) -> usize {
        let mut at = 0;
        for name in parts(abs) {
            let existing = self.children(at).and_then(|c| c.get(name).copied());
            at = existing.unwrap_or_else(|| {
                self.add(at, name, Kind::Dir(BTreeMap::new()), ROOT, Mode::RW_R)
            });
        }
        at
    }

    /// Generator use, no checks: a file, replacing whatever is there.
    pub fn put(&mut self, path: &str, text: &str, owner: &str, mode: Mode) {
        self.put_node(path, Kind::File(text.to_string()), owner, mode);
    }

    pub fn put_dev(&mut self, path: &str, dev: Dev, owner: &str, mode: Mode) {
        self.put_node(path, Kind::Dev(dev), owner, mode);
    }

    pub fn read(&self, user: &str, cwd: &str, home: &str, path: &str) -> Result<&Kind, VfsError> {
        let id = self.resolve_as(user, cwd, home, path)?;
        if !self.can_read(id, user) {
            return Err(VfsError::Denied);
        }
        match &self.nodes[id].kind {
            Kind::Dir(_) => Err(VfsError::IsADirectory),
            k => Ok(k),
        }
    }

    /// The children of a directory by name; a file lists as itself.
    pub fn list(
        &self,
        user: &str,
        cwd: &str,
        home: &str,
        path: &str,
    ) -> Result<Vec<usize>, VfsError> {
        let id = self.resolve_as(user, cwd, home, path)?;
        if !self.can_read(id, user) {
            return Err(VfsError::Denied);
        }
        match &self.nodes[id].kind {
            Kind::Dir(children) => Ok(children.values().copied().collect()),
            _ => Ok(vec![id]),
        }
    }

    pub fn remove(
        &mut self,
        user: &str,
        cwd: &str,
        home: &str,
        path: &str,
        recursive: bool,
    ) -> Result<(), VfsError> {
        let abs = Vfs::normalize(cwd, home, path);
        let id = self.lookup_as(user, &abs)?;
        let (pid, name) = self.parent_of(user, &abs)?;
        if !self.can_write(pid, user) {
            return Err(VfsError::Denied);
        }
        if let Some(c) = self.children(id)
            && !c.is_empty()
            && !recursive
        {
            return Err(VfsError::NotEmpty);
        }
        if let Kind::Dir(c) = &mut self.nodes[pid].kind {
            c.remove(&name);
        }
        Ok(())
    }

    /// The file a copy reads, and the parent and name it lands under.
    pub fn copy(
        &mut self,
        user: &str,
        cwd: &str,
        home: &str,
        src: &str,
        dst: &str,
    ) -> Result<(), VfsError> {
        let sid = self.resolve_as(user, cwd, home, src)?;
        let text = match &self.nodes[sid].kind {
            Kind::File(t) => t.clone(),
            Kind::Dir(_) => return Err(VfsError::IsADirectory),
            Kind::Dev(_) => return Err(VfsError::Denied),
        };
        if !self.can_read(sid, user) {
            return Err(VfsError::Denied);
        }
        let name = self.nodes[sid].name.clone();
        let abs = self.landing(user, cwd, home, dst, &name)?;
        match self.lookup_as(user, &abs) {
            Ok(id) => match &mut self.nodes[id].kind {
                Kind::File(body) => *body = text,
                Kind::Dir(_) => return Err(VfsError::IsADirectory),
                Kind::Dev(_) => return Err(VfsError::Denied),
            },
            Err(VfsError::NotFound) => {
                let (pid, n) = self.parent_of(user, &abs)?;
                self.add(pid, &n, Kind::File(text), user, Mode::RW_R);
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Where a copy or move to `dst` lands (into `dst` if it is a directory) and that the
    /// user may write there.
    fn landing(
        &self,
        user: &str,
        cwd: &str,
        home: &str,
        dst: &str,
        name: &str,
    ) -> Result<String, VfsError> {
        let mut abs = Vfs::normalize(cwd, home, dst);
        if let Ok(id) = self.lookup_as(user, &abs)
            && self.children(id).is_some()
        {
            if !self.can_write(id, user) {
                return Err(VfsError::Denied);
            }
            abs = format!("{}/{}", abs.trim_end_matches('/'), name);
        }
        match self.lookup_as(user, &abs) {
            Ok(id) if !self.can_write(id, user) => Err(VfsError::Denied),
            Ok(_) => Ok(abs),
            Err(VfsError::NotFound) => {
                let (pid, _) = self.parent_of(user, &abs)?;
                if self.can_write(pid, user) {
                    Ok(abs)
                } else {
                    Err(VfsError::Denied)
                }
            }
            Err(e) => Err(e),
        }
    }

    pub fn rename(
        &mut self,
        user: &str,
        cwd: &str,
        home: &str,
        src: &str,
        dst: &str,
    ) -> Result<(), VfsError> {
        let sabs = Vfs::normalize(cwd, home, src);
        let sid = self.lookup_as(user, &sabs)?;
        let (spid, sname) = self.parent_of(user, &sabs)?;
        if !self.can_write(spid, user) {
            return Err(VfsError::Denied);
        }
        let dabs = self.landing(user, cwd, home, dst, &sname)?;
        if dabs == sabs {
            return Ok(());
        }
        if dabs.starts_with(&format!("{sabs}/")) {
            return Err(VfsError::Denied);
        }
        let (dpid, dname) = self.parent_of(user, &dabs)?;
        if let Some(&old) = self.children(dpid).and_then(|c| c.get(&dname))
            && self.children(old).is_some()
        {
            return Err(VfsError::IsADirectory);
        }
        if let Kind::Dir(c) = &mut self.nodes[spid].kind {
            c.remove(&sname);
        }
        if let Kind::Dir(c) = &mut self.nodes[dpid].kind {
            c.insert(dname.clone(), sid);
        }
        self.nodes[sid].name = dname;
        Ok(())
    }

    /// `drw-r-- root        /etc`: type, owner bits, other bits, owner padded to ten, then
    /// the size for a file and the path.
    pub fn long_row(&self, id: usize) -> String {
        let n = &self.nodes[id];
        let ty = match n.kind {
            Kind::Dir(_) => 'd',
            Kind::File(_) => '-',
            Kind::Dev(_) => 'c',
        };
        let path = self.path_of(id);
        let tail = match n.kind {
            Kind::File(_) => format!(" {:>5} {path}", self.size(id)),
            _ => format!("  {path}"),
        };
        format!("{ty}{} {:<10}{tail}", n.mode.bits(), n.owner)
    }

    pub fn size(&self, id: usize) -> usize {
        match &self.nodes[id].kind {
            Kind::File(t) => t.len(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> Vfs {
        let mut v = Vfs::new();
        v.mkdir_as("/etc", ROOT, Mode::RW_R);
        v.mkdir_as("/tmp", USERS, Mode::RW_RW);
        v.mkdir_as("/home/mara", "mara", Mode::RW_NONE);
        v.mkdir_as("/dev", ROOT, Mode::RW_R);
        v.put("/home/mara/notes", "hello", "mara", Mode::RW_NONE);
        v.put_dev("/dev/door", Dev::Door, USERS, Mode::RW_R);
        v.put_dev("/dev/alarm", Dev::Alarm, ROOT, Mode::RW_R);
        v
    }

    #[test]
    fn paths_resolve_dots_home_and_trailing_slashes() {
        let v = world();
        let id = v.resolve("/", "/home/mara", "~/notes").unwrap();
        assert_eq!(v.path_of(id), "/home/mara/notes");
        assert_eq!(
            v.resolve("/etc", "/", "../home/mara/").unwrap(),
            v.resolve("/", "/", "/home/mara").unwrap()
        );
        assert_eq!(
            v.resolve("/home", "/home/mara", "mara/./notes").unwrap(),
            id
        );
        assert_eq!(
            v.resolve("/", "/home/mara", "~").unwrap(),
            v.resolve("/", "/", "/home/mara/").unwrap()
        );
        assert_eq!(v.resolve("/", "/", "..").unwrap(), 0);
        assert_eq!(
            Vfs::normalize("/tmp", "/home/mara", "a//b/../c/"),
            "/tmp/a/c"
        );
        assert_eq!(Vfs::normalize("/tmp", "/home/mara", "~/x"), "/home/mara/x");
        assert_eq!(v.resolve("/", "/", "/nope"), Err(VfsError::NotFound));
        assert_eq!(
            v.resolve("/", "/", "/dev/door/x"),
            Err(VfsError::NotADirectory)
        );
    }

    #[test]
    fn errors_use_unix_words() {
        assert_eq!(VfsError::NotFound.to_string(), "No such file or directory");
        assert_eq!(VfsError::NotADirectory.to_string(), "Not a directory");
        assert_eq!(VfsError::IsADirectory.to_string(), "Is a directory");
        assert_eq!(VfsError::Denied.to_string(), "Permission denied");
        assert_eq!(VfsError::Exists.to_string(), "File exists");
        assert_eq!(VfsError::NotEmpty.to_string(), "Directory not empty");
    }

    #[test]
    fn permissions_follow_owner_users_and_root() {
        let mut v = world();
        let home = v.resolve("/", "/", "/home/mara").unwrap();
        assert!(!v.can_read(home, GUEST));
        assert!(v.can_read(home, "mara"));
        assert!(v.can_read(home, ROOT));
        assert_eq!(v.list(GUEST, "/", "/", "/home/mara"), Err(VfsError::Denied));
        let door = v.resolve("/", "/", "/dev/door").unwrap();
        assert!(v.can_read(door, GUEST));
        assert!(!v.can_write(door, GUEST));
        assert!(v.can_write(door, "user"));
        assert!(v.can_write(door, "mara"));
        assert_eq!(
            v.write(GUEST, "/", "/", "/dev/door", "x", false),
            Err(VfsError::Denied)
        );
        assert_eq!(v.write("user", "/", "/", "/dev/door", "x", false), Ok(()));
        assert_eq!(v.write(ROOT, "/", "/", "/dev/alarm", "x", false), Ok(()));
        assert_eq!(
            v.write("mara", "/", "/", "/dev/alarm", "x", false),
            Err(VfsError::Denied)
        );
        assert!(matches!(
            v.read(GUEST, "/", "/", "/dev/door"),
            Ok(Kind::Dev(Dev::Door))
        ));
        assert_eq!(
            v.read(GUEST, "/", "/", "/home/mara/notes"),
            Err(VfsError::Denied)
        );
        assert_eq!(v.read(GUEST, "/", "/", "/etc"), Err(VfsError::IsADirectory));
    }

    #[test]
    fn write_creates_and_appends_and_needs_the_parent() {
        let mut v = world();
        assert_eq!(
            v.write(GUEST, "/etc", "/tmp", "motd", "x", false),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.write(GUEST, "/tmp", "/tmp", "a.txt", "one", false),
            Ok(())
        );
        assert_eq!(
            v.write(GUEST, "/tmp", "/tmp", "a.txt", "-two", true),
            Ok(())
        );
        let id = v.resolve("/", "/", "/tmp/a.txt").unwrap();
        assert_eq!(v.node(id).kind, Kind::File("one-two".into()));
        assert_eq!(v.node(id).owner, GUEST);
        assert_eq!(v.node(id).mode, Mode::RW_R);
        assert_eq!(
            v.write(GUEST, "/tmp", "/tmp", "a.txt", "new", false),
            Ok(())
        );
        assert_eq!(v.node(id).kind, Kind::File("new".into()));
        assert_eq!(
            v.write(GUEST, "/", "/tmp", "/nope/x", "x", false),
            Err(VfsError::NotFound)
        );
        assert_eq!(
            v.write(GUEST, "/", "/tmp", "/tmp", "x", false),
            Err(VfsError::IsADirectory)
        );
        assert_eq!(
            v.mkdir(GUEST, "/tmp", "/tmp", "d/e", false),
            Err(VfsError::NotFound)
        );
        assert!(v.mkdir(GUEST, "/tmp", "/tmp", "d/e", true).is_ok());
        assert_eq!(
            v.mkdir(GUEST, "/tmp", "/tmp", "d", false),
            Err(VfsError::Exists)
        );
        assert_eq!(
            v.mkdir(GUEST, "/", "/tmp", "/etc/x", false),
            Err(VfsError::Denied)
        );
    }

    #[test]
    fn remove_refuses_root_and_non_empty_without_r() {
        let mut v = world();
        assert_eq!(v.remove(ROOT, "/", "/", "/", true), Err(VfsError::Denied));
        assert_eq!(
            v.remove(ROOT, "/", "/", "/home", false),
            Err(VfsError::NotEmpty)
        );
        assert_eq!(
            v.remove(GUEST, "/", "/", "/etc", true),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.remove(ROOT, "/", "/", "/nope", false),
            Err(VfsError::NotFound)
        );
        assert_eq!(v.remove(ROOT, "/", "/", "/home", true), Ok(()));
        assert_eq!(v.resolve("/", "/", "/home"), Err(VfsError::NotFound));
        v.write(GUEST, "/tmp", "/tmp", "f", "x", false).unwrap();
        assert_eq!(v.remove(GUEST, "/tmp", "/tmp", "f", false), Ok(()));
    }

    #[test]
    fn copy_and_rename_move_files_and_keep_owner() {
        let mut v = world();
        assert_eq!(v.copy(ROOT, "/", "/", "/home/mara/notes", "/tmp"), Ok(()));
        let copy = v.resolve("/", "/", "/tmp/notes").unwrap();
        assert_eq!(v.node(copy).kind, Kind::File("hello".into()));
        assert_eq!(
            v.copy(GUEST, "/", "/", "/home/mara/notes", "/tmp/x"),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.copy(ROOT, "/", "/", "/etc", "/tmp/e"),
            Err(VfsError::IsADirectory)
        );
        let orig = v.resolve("/", "/", "/home/mara/notes").unwrap();
        assert_eq!(
            v.rename("mara", "/", "/", "/home/mara/notes", "/home/mara/log"),
            Ok(())
        );
        assert_eq!(
            v.resolve("/", "/", "/home/mara/notes"),
            Err(VfsError::NotFound)
        );
        let moved = v.resolve("/", "/", "/home/mara/log").unwrap();
        assert_eq!(moved, orig);
        assert_eq!(v.node(moved).owner, "mara");
        assert_eq!(v.node(moved).name, "log");
        assert_eq!(v.rename("mara", "/", "/", "/home/mara/log", "/tmp"), Ok(()));
        assert_eq!(
            v.node(v.resolve("/", "/", "/tmp/log").unwrap()).owner,
            "mara"
        );
        assert_eq!(
            v.rename(GUEST, "/", "/", "/etc", "/tmp/etc"),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.rename(ROOT, "/", "/", "/home", "/home/mara/h"),
            Err(VfsError::Denied)
        );
    }

    #[test]
    fn a_private_directory_guards_the_path() {
        let mut v = world();
        assert_eq!(
            v.write("mara", "/home/mara", "/home/mara", "new", "secret", false),
            Ok(())
        );
        for p in ["/home/mara/new", "/home/mara/notes"] {
            assert_eq!(v.read(GUEST, "/", "/", p), Err(VfsError::Denied));
            assert_eq!(v.list(GUEST, "/", "/", p), Err(VfsError::Denied));
            assert!(v.read("mara", "/", "/", p).is_ok());
            assert!(v.read(ROOT, "/", "/", p).is_ok());
        }
        assert_eq!(v.list(GUEST, "/", "/", "/home/mara"), Err(VfsError::Denied));
        assert_eq!(
            v.copy(GUEST, "/", "/", "/home/mara/new", "/tmp"),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.remove(GUEST, "/", "/", "/home/mara/new", false),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.write(GUEST, "/", "/", "/home/mara/x", "x", false),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.mkdir(GUEST, "/", "/", "/home/mara/d", false),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.mkdir(GUEST, "/", "/tmp", "/home/mara/notes", false),
            Err(VfsError::Denied)
        );
        v.mkdir("mara", "/", "/home/mara", "/home/mara/pub", false)
            .unwrap();
        v.mkdir_as("/home/mara/pub", "mara", Mode::RW_RW);
        assert_eq!(
            v.mkdir(GUEST, "/", "/tmp", "/home/mara/pub/z", true),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.mkdir(GUEST, "/", "/tmp", "/home/mara/pub", true),
            Err(VfsError::Denied)
        );
        assert_eq!(
            v.rename(GUEST, "/", "/", "/home/mara/new", "/tmp/n"),
            Err(VfsError::Denied)
        );
    }

    #[test]
    fn long_rows_read_like_ls_l() {
        let v = world();
        let etc = v.resolve("/", "/", "/etc").unwrap();
        assert_eq!(v.long_row(etc), "drw-r-- root        /etc");
        let notes = v.resolve("/", "/", "/home/mara/notes").unwrap();
        assert_eq!(
            v.long_row(notes),
            "-rw---- mara           5 /home/mara/notes"
        );
        let door = v.resolve("/", "/", "/dev/door").unwrap();
        assert_eq!(v.long_row(door), "crw-r-- users       /dev/door");
        assert_eq!(v.size(notes), 5);
        assert_eq!(v.size(etc), 0);
    }
}
