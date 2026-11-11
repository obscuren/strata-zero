//! The HELIX/OS shell: a tokenizer, a pipeline runner and the builtins. Everything it
//! knows of the world comes through the `Io` trait.

use super::vfs::{Dev, Kind, ROOT, Vfs, VfsError};
use citygen::{BuildingId, DistrictId};
#[cfg(test)]
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HostId {
    Rig,
    Building(BuildingId),
    District(DistrictId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shell {
    pub host: HostId,
    pub origin: HostId,
    pub user: String,
    pub home: String,
    pub cwd: String,
    pub history: Vec<String>,
}

impl Shell {
    /// A fresh session on `host`, in the user's home.
    pub fn new(host: HostId, user: &str, home: &str) -> Shell {
        Shell {
            host,
            origin: host,
            user: user.to_string(),
            home: home.to_string(),
            cwd: home.to_string(),
            history: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub lines: Vec<String>,
    pub status: i32,
    pub clear: bool,
    pub exit: bool,
    pub denied: bool,
    pub open_nano: Option<String>,
    /// `mail` asks the screen for HELIX mail.
    pub open_mail: bool,
}

/// What the shell needs from the world; the app implements it, tests use `MemIo`.
pub trait Io {
    fn fs(&mut self, host: HostId) -> &mut Vfs;
    fn hostname(&self, host: HostId) -> String;
    fn os_version(&self, host: HostId) -> String;
    /// `Err` is a full message.
    fn read_dev(&mut self, host: HostId, dev: &Dev, user: &str) -> Result<Vec<String>, String>;
    fn write_dev(
        &mut self,
        host: HostId,
        dev: &Dev,
        user: &str,
        text: &str,
    ) -> Result<Vec<String>, String>;
    /// `None`: not a program.
    fn program(
        &mut self,
        shell: &mut Shell,
        name: &str,
        args: &[String],
        stdin: &[String],
    ) -> Option<Result<Output, String>>;
    /// For `/bin`, `help` and completion.
    fn programs(&self) -> Vec<&'static str>;
    fn who(&mut self, host: HostId) -> Vec<String>;
    /// Called by `cat` after it read a plain file: absolute path and text.
    fn on_read(&mut self, _host: HostId, _path: &str, _text: &str) {}
    /// Something at `path` was removed, moved, overwritten or written: absolute path.
    fn changed(&mut self, _host: HostId, _path: &str) {}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pipeline {
    pub stages: Vec<Vec<String>>,
    /// Target path and whether it appends.
    pub redirect: Option<(String, bool)>,
}

pub const BUILTINS: &[&str] = &[
    "ls", "cd", "pwd", "cat", "echo", "touch", "mkdir", "rm", "cp", "mv", "grep", "head", "tail",
    "wc", "man", "help", "whoami", "who", "uname", "hostname", "history", "clear", "exit", "nano",
];

/// The test double: one vfs per host, devices read `dev:<name>`, writes echo back.
#[cfg(test)]
pub struct MemIo {
    pub fs: HashMap<HostId, Vfs>,
}

#[cfg(test)]
impl Io for MemIo {
    fn fs(&mut self, host: HostId) -> &mut Vfs {
        self.fs.entry(host).or_default()
    }
    fn hostname(&self, _host: HostId) -> String {
        "lowtown".to_string()
    }
    fn os_version(&self, _host: HostId) -> String {
        "4.1".to_string()
    }
    fn read_dev(&mut self, _host: HostId, dev: &Dev, _user: &str) -> Result<Vec<String>, String> {
        Ok(vec![format!("dev:{dev:?}")])
    }
    fn write_dev(
        &mut self,
        _host: HostId,
        _dev: &Dev,
        _user: &str,
        text: &str,
    ) -> Result<Vec<String>, String> {
        Ok(vec![text.to_string()])
    }
    fn program(
        &mut self,
        _shell: &mut Shell,
        _name: &str,
        _args: &[String],
        _stdin: &[String],
    ) -> Option<Result<Output, String>> {
        None
    }
    fn programs(&self) -> Vec<&'static str> {
        Vec::new()
    }
    fn who(&mut self, _host: HostId) -> Vec<String> {
        Vec::new()
    }
}

// ---- the parser ----

enum Tok {
    Word(String),
    Semi,
    Pipe,
    Out(bool),
}

fn tokenize(line: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some(q) if q == c => break,
                        Some(q) => word.push(q),
                        None => return Err("helix: unterminated quote".to_string()),
                    }
                }
            }
            c if c.is_whitespace() || matches!(c, ';' | '|' | '>') => {
                if in_word {
                    toks.push(Tok::Word(std::mem::take(&mut word)));
                    in_word = false;
                }
                match c {
                    ';' => toks.push(Tok::Semi),
                    '|' => toks.push(Tok::Pipe),
                    '>' => {
                        let append = chars.peek() == Some(&'>');
                        if append {
                            chars.next();
                        }
                        toks.push(Tok::Out(append));
                    }
                    _ => {}
                }
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        toks.push(Tok::Word(word));
    }
    Ok(toks)
}

fn syntax(token: &str) -> String {
    format!("helix: syntax error near unexpected token `{token}`")
}

/// `;` separates pipelines, `|` the stages of one, a trailing `>` or `>>` its redirect.
pub fn parse(line: &str) -> Result<Vec<Pipeline>, String> {
    let mut out = Vec::new();
    let mut stages: Vec<Vec<String>> = Vec::new();
    let mut stage: Vec<String> = Vec::new();
    let mut redirect: Option<(String, bool)> = None;
    let mut pending: Option<bool> = None;
    for tok in tokenize(line)? {
        match tok {
            Tok::Word(wd) => {
                if let Some(append) = pending.take() {
                    redirect = Some((wd, append));
                } else if redirect.is_some() {
                    return Err(syntax(&wd));
                } else {
                    stage.push(wd);
                }
            }
            Tok::Pipe => {
                if pending.is_some() || redirect.is_some() || stage.is_empty() {
                    return Err(syntax("|"));
                }
                stages.push(std::mem::take(&mut stage));
            }
            Tok::Out(append) => {
                if pending.is_some() || redirect.is_some() || stage.is_empty() {
                    return Err(syntax(if append { ">>" } else { ">" }));
                }
                pending = Some(append);
            }
            Tok::Semi => {
                if pending.is_some() {
                    return Err(syntax(";"));
                }
                if stage.is_empty() {
                    if !stages.is_empty() {
                        return Err(syntax(";"));
                    }
                    continue;
                }
                stages.push(std::mem::take(&mut stage));
                out.push(Pipeline {
                    stages: std::mem::take(&mut stages),
                    redirect: redirect.take(),
                });
            }
        }
    }
    if pending.is_some() || (stage.is_empty() && !stages.is_empty()) {
        return Err(syntax("newline"));
    }
    if !stage.is_empty() {
        stages.push(stage);
        out.push(Pipeline { stages, redirect });
    }
    Ok(out)
}

// ---- the runner ----

/// The parts of the shell a builtin reads, copied so the io can be borrowed freely.
struct Me {
    host: HostId,
    user: String,
    home: String,
    cwd: String,
}

impl Me {
    fn of(shell: &Shell) -> Me {
        Me {
            host: shell.host,
            user: shell.user.clone(),
            home: shell.home.clone(),
            cwd: shell.cwd.clone(),
        }
    }

    fn abs(&self, path: &str) -> String {
        Vfs::normalize(&self.cwd, &self.home, path)
    }
}

fn is_denied(msg: &str) -> bool {
    msg.ends_with("Permission denied")
}

/// A stage's result: `out` is its stdout (and flags); `err` is what it complains about.
/// Only stdout flows down a pipe or into a redirect; errors go to the screen.
#[derive(Default)]
struct Res {
    out: Output,
    err: Vec<String>,
}

impl std::ops::Deref for Res {
    type Target = Output;
    fn deref(&self) -> &Output {
        &self.out
    }
}

impl std::ops::DerefMut for Res {
    fn deref_mut(&mut self) -> &mut Output {
        &mut self.out
    }
}

fn fail(out: &mut Res, cmd: &str, path: &str, e: &VfsError) {
    out.err.push(format!("{cmd}: {path}: {e}"));
    out.status = 1;
    if *e == VfsError::Denied {
        out.denied = true;
    }
}

fn fail_msg(out: &mut Res, msg: String) {
    out.denied |= is_denied(&msg);
    out.err.push(msg);
    out.status = 1;
}

fn text_lines(text: &str) -> Vec<String> {
    text.lines().map(String::from).collect()
}

/// Flags (`-l`, `-ai`) and operands, in order; a lone `-` is an operand.
fn split_args(args: &[String]) -> (String, Vec<&str>) {
    let mut flags = String::new();
    let mut operands = Vec::new();
    for a in args {
        match a.strip_prefix('-') {
            Some(f) if !f.is_empty() => flags.push_str(f),
            _ => operands.push(a.as_str()),
        }
    }
    (flags, operands)
}

enum ReadErr {
    Fs(VfsError),
    Msg(String),
}

fn report(out: &mut Res, cmd: &str, path: &str, e: ReadErr) {
    match e {
        ReadErr::Fs(e) => fail(out, cmd, path, &e),
        ReadErr::Msg(m) => fail_msg(out, m),
    }
}

/// A file's lines, or a device's through the world. A plain file read is told to the
/// world, whichever builtin reads it: what you saw, you learn.
fn read_text(io: &mut dyn Io, me: &Me, path: &str) -> Result<Vec<String>, ReadErr> {
    let kind = io
        .fs(me.host)
        .read(&me.user, &me.cwd, &me.home, path)
        .map_err(ReadErr::Fs)?
        .clone();
    match kind {
        Kind::File(text) => {
            io.on_read(me.host, &me.abs(path), &text);
            Ok(text_lines(&text))
        }
        Kind::Dev(dev) => io.read_dev(me.host, &dev, &me.user).map_err(ReadErr::Msg),
        Kind::Dir(_) => Err(ReadErr::Fs(VfsError::IsADirectory)),
    }
}

/// The input of a filter: the named files, or stdin when there are none.
fn inputs(
    io: &mut dyn Io,
    me: &Me,
    cmd: &str,
    files: &[&str],
    stdin: &[String],
    out: &mut Res,
) -> Vec<(Option<String>, Vec<String>)> {
    if files.is_empty() {
        return vec![(None, stdin.to_vec())];
    }
    let mut got = Vec::new();
    for f in files {
        match read_text(io, me, f) {
            Ok(lines) => got.push((Some((*f).to_string()), lines)),
            Err(e) => report(out, cmd, f, e),
        }
    }
    got
}

pub fn run(io: &mut dyn Io, shell: &mut Shell, line: &str) -> Output {
    let line = line.trim();
    if line.is_empty() {
        return Output::default();
    }
    shell.history.push(line.to_string());
    let pipelines = match parse(line) {
        Ok(p) => p,
        Err(e) => {
            return Output {
                lines: vec![e],
                status: 2,
                ..Output::default()
            };
        }
    };
    let mut total = Output::default();
    for p in &pipelines {
        let o = run_pipeline(io, shell, p);
        total.lines.extend(o.lines);
        total.status = o.status;
        total.clear |= o.clear;
        total.exit |= o.exit;
        total.denied |= o.denied;
        if o.open_nano.is_some() {
            total.open_nano = o.open_nano;
        }
        total.open_mail |= o.open_mail;
        if total.exit || total.open_nano.is_some() || total.open_mail {
            break;
        }
    }
    total
}

fn run_pipeline(io: &mut dyn Io, shell: &mut Shell, p: &Pipeline) -> Output {
    let mut acc = Output::default();
    let mut errs: Vec<String> = Vec::new();
    let mut stdin: Vec<String> = Vec::new();
    for stage in &p.stages {
        let r = exec(io, shell, stage, &stdin);
        acc.status = r.out.status;
        acc.clear |= r.out.clear;
        acc.exit |= r.out.exit;
        acc.denied |= r.out.denied;
        if r.out.open_nano.is_some() {
            acc.open_nano = r.out.open_nano;
        }
        acc.open_mail |= r.out.open_mail;
        errs.extend(r.err);
        stdin = r.out.lines;
    }
    acc.lines = stdin;
    if let Some((path, append)) = &p.redirect {
        let me = Me::of(shell);
        let r = redirect(io, &me, path, *append, &acc.lines);
        acc.lines = r.out.lines;
        acc.denied |= r.out.denied;
        if r.out.status != 0 {
            acc.status = r.out.status;
        }
        errs.extend(r.err);
    }
    acc.lines.extend(errs);
    acc
}

/// Write a pipeline's lines onto a file, or hand them to a device after the check.
fn redirect(io: &mut dyn Io, me: &Me, path: &str, append: bool, lines: &[String]) -> Res {
    let mut out = Res::default();
    let node = {
        let fs = io.fs(me.host);
        fs.resolve_as(&me.user, &me.cwd, &me.home, path)
            .ok()
            .map(|id| (fs.can_write(id, &me.user), fs.node(id).kind.clone()))
    };
    if let Some((writable, Kind::Dev(dev))) = node {
        if !writable {
            fail(&mut out, "helix", path, &VfsError::Denied);
        } else {
            match io.write_dev(me.host, &dev, &me.user, &lines.join("\n")) {
                Ok(reply) => out.lines = reply,
                Err(m) => fail_msg(&mut out, m),
            }
        }
        return out;
    }
    let mut text = lines.join("\n");
    if !lines.is_empty() {
        text.push('\n');
    }
    if let Err(e) = io
        .fs(me.host)
        .write(&me.user, &me.cwd, &me.home, path, &text, append)
    {
        fail(&mut out, "helix", path, &e);
    } else {
        io.changed(me.host, &me.abs(path));
    }
    out
}

fn exec(io: &mut dyn Io, shell: &mut Shell, argv: &[String], stdin: &[String]) -> Res {
    let name = argv[0].as_str();
    let args = &argv[1..];
    if BUILTINS.contains(&name) {
        return builtin(io, shell, name, args, stdin);
    }
    match io.program(shell, name, args, stdin) {
        Some(Ok(out)) => Res {
            out,
            err: Vec::new(),
        },
        Some(Err(m)) => {
            let mut out = Res::default();
            fail_msg(&mut out, m);
            out
        }
        None => Res {
            out: Output {
                status: 127,
                ..Output::default()
            },
            err: vec![format!("helix: {name}: command not found")],
        },
    }
}

// ---- the builtins ----

fn builtin(
    io: &mut dyn Io,
    shell: &mut Shell,
    name: &str,
    args: &[String],
    stdin: &[String],
) -> Res {
    let me = Me::of(shell);
    let mut out = Res::default();
    match name {
        "ls" => ls(io, &me, args, &mut out),
        "cd" => cd(io, shell, &me, args, &mut out),
        "pwd" => out.lines.push(shell.cwd.clone()),
        "cat" => cat(io, &me, args, stdin, &mut out),
        "echo" => out.lines.push(args.join(" ")),
        "touch" => touch(io, &me, args, &mut out),
        "mkdir" => mkdir(io, &me, args, &mut out),
        "rm" => rm(io, &me, args, &mut out),
        "cp" | "mv" => cp_mv(io, &me, name, args, &mut out),
        "grep" => grep(io, &me, args, stdin, &mut out),
        "head" | "tail" => head_tail(io, &me, name, args, stdin, &mut out),
        "wc" => wc(io, &me, args, stdin, &mut out),
        "man" => match args.first() {
            None => fail_msg(&mut out, "What manual page do you want?".to_string()),
            Some(n) => match PAGES.iter().find(|p| p.0 == n) {
                Some((n, gloss, body)) => {
                    out.lines = vec![format!("{n} - {gloss}"), String::new(), (*body).to_string()]
                }
                None => fail_msg(&mut out, format!("No manual entry for {n}")),
            },
        },
        "help" => out.lines = help(io),
        "whoami" => out.lines.push(shell.user.clone()),
        "who" => out.lines = io.who(shell.host),
        "uname" => {
            let all = args.iter().any(|a| a == "-a");
            out.lines.push(if all {
                format!(
                    "HELIX/OS {} {}",
                    io.os_version(shell.host),
                    io.hostname(shell.host)
                )
            } else {
                "HELIX/OS".to_string()
            });
        }
        "hostname" => out.lines.push(io.hostname(shell.host)),
        "history" => {
            out.lines = shell
                .history
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{}  {l}", i + 1))
                .collect()
        }
        "clear" => out.clear = true,
        // The world may take `exit` first (leaving a remote host); only when it does not
        // does the shell itself end.
        "exit" => match io.program(shell, "exit", args, stdin) {
            Some(Ok(o)) => out.out = o,
            Some(Err(m)) => fail_msg(&mut out, m),
            None => out.exit = true,
        },
        "nano" => match args.first() {
            Some(p) => out.open_nano = Some(me.abs(p)),
            None => fail_msg(&mut out, "usage: nano file".to_string()),
        },
        _ => {}
    }
    out
}

fn ls(io: &mut dyn Io, me: &Me, args: &[String], out: &mut Res) {
    let (flags, mut paths) = split_args(args);
    let long = flags.contains('l');
    let all = flags.contains('a');
    if paths.is_empty() {
        paths.push(".");
    }
    for path in paths {
        let fs = io.fs(me.host);
        let ids = match fs.list(&me.user, &me.cwd, &me.home, path) {
            Ok(ids) => ids,
            Err(e) => {
                fail(out, "ls", path, &e);
                continue;
            }
        };
        if me.abs(path) == "/bin" {
            let mut names: Vec<String> = BUILTINS.iter().map(|s| s.to_string()).collect();
            names.extend(io.programs().iter().map(|s| s.to_string()));
            names.sort();
            names.dedup();
            out.lines.extend(names.into_iter().map(|n| {
                if long {
                    format!("-rw-r-- {:<10}{:>6} /bin/{n}", ROOT, 0)
                } else {
                    n
                }
            }));
            continue;
        }
        let is_dir = fs
            .resolve(&me.cwd, &me.home, path)
            .is_ok_and(|id| matches!(fs.node(id).kind, Kind::Dir(_)));
        for id in ids {
            let node = fs.node(id);
            if is_dir && !all && node.name.starts_with('.') {
                continue;
            }
            out.lines.push(if long {
                fs.long_row(id)
            } else {
                node.name.clone()
            });
        }
    }
}

fn cd(io: &mut dyn Io, shell: &mut Shell, me: &Me, args: &[String], out: &mut Res) {
    if args.len() > 1 {
        fail_msg(out, "cd: too many arguments".to_string());
        return;
    }
    let path = args.first().map_or("~", |s| s.as_str());
    let fs = io.fs(me.host);
    match fs.resolve_as(&me.user, &me.cwd, &me.home, path) {
        Err(e) => fail(out, "cd", path, &e),
        Ok(id) if !matches!(fs.node(id).kind, Kind::Dir(_)) => {
            fail(out, "cd", path, &VfsError::NotADirectory)
        }
        Ok(id) if !fs.can_read(id, &me.user) => fail(out, "cd", path, &VfsError::Denied),
        Ok(_) => shell.cwd = me.abs(path),
    }
}

fn cat(io: &mut dyn Io, me: &Me, args: &[String], stdin: &[String], out: &mut Res) {
    if args.is_empty() {
        out.lines = stdin.to_vec();
        return;
    }
    for path in args {
        match read_text(io, me, path) {
            Ok(lines) => out.lines.extend(lines),
            Err(e) => report(out, "cat", path, e),
        }
    }
}

fn touch(io: &mut dyn Io, me: &Me, args: &[String], out: &mut Res) {
    if args.is_empty() {
        fail_msg(out, "touch: missing file operand".to_string());
    }
    for path in args {
        let fs = io.fs(me.host);
        let r = match fs.resolve_as(&me.user, &me.cwd, &me.home, path) {
            Ok(_) => Ok(()),
            Err(VfsError::NotFound) => fs.write(&me.user, &me.cwd, &me.home, path, "", false),
            Err(e) => Err(e),
        };
        if let Err(e) = r {
            fail(out, "touch", path, &e);
        }
    }
}

fn mkdir(io: &mut dyn Io, me: &Me, args: &[String], out: &mut Res) {
    let (flags, paths) = split_args(args);
    if paths.is_empty() {
        fail_msg(out, "mkdir: missing operand".to_string());
    }
    for path in paths {
        let r = io
            .fs(me.host)
            .mkdir(&me.user, &me.cwd, &me.home, path, flags.contains('p'));
        if let Err(e) = r {
            fail(out, "mkdir", path, &e);
        }
    }
}

fn rm(io: &mut dyn Io, me: &Me, args: &[String], out: &mut Res) {
    let (flags, paths) = split_args(args);
    let recursive = flags.contains('r') || flags.contains('R');
    if paths.is_empty() {
        fail_msg(out, "rm: missing operand".to_string());
    }
    for path in paths {
        let abs = me.abs(path);
        let fs = io.fs(me.host);
        let r = match fs.resolve_as(&me.user, &me.cwd, &me.home, path) {
            Ok(id) if !recursive && matches!(fs.node(id).kind, Kind::Dir(_)) => {
                Err(VfsError::IsADirectory)
            }
            Ok(_) => fs.remove(&me.user, &me.cwd, &me.home, path, recursive),
            Err(e) => Err(e),
        };
        match r {
            Err(VfsError::NotFound) if flags.contains('f') => {}
            Err(e) => fail(out, "rm", path, &e),
            Ok(()) => io.changed(me.host, &abs),
        }
    }
}

/// Whether a failed `cp` or `mv` is the destination's fault: the source is fine and the
/// destination (or the directory it would land in) is not writable.
fn dst_to_blame(fs: &Vfs, me: &Me, src: &str, dst: &str) -> bool {
    match fs.resolve_as(&me.user, &me.cwd, &me.home, src) {
        Ok(id) if fs.can_read(id, &me.user) => {}
        _ => return false,
    }
    match fs.resolve_as(&me.user, &me.cwd, &me.home, dst) {
        Ok(id) => !fs.can_write(id, &me.user),
        Err(VfsError::NotFound) => {
            let parent = format!("{}/..", me.abs(dst));
            match fs.resolve_as(&me.user, "/", "/", &parent) {
                Ok(id) => !fs.can_write(id, &me.user),
                Err(_) => true,
            }
        }
        Err(_) => true,
    }
}

fn cp_mv(io: &mut dyn Io, me: &Me, cmd: &str, args: &[String], out: &mut Res) {
    let (_, ops) = split_args(args);
    let [src, dst] = ops[..] else {
        let msg = if ops.len() < 2 {
            format!("{cmd}: missing file operand")
        } else {
            format!("{cmd}: too many arguments")
        };
        fail_msg(out, msg);
        return;
    };
    let fs = io.fs(me.host);
    let r = if cmd == "cp" {
        fs.copy(&me.user, &me.cwd, &me.home, src, dst)
    } else {
        fs.rename(&me.user, &me.cwd, &me.home, src, dst)
    };
    if let Err(e) = r {
        let at = if dst_to_blame(fs, me, src, dst) {
            dst
        } else {
            src
        };
        fail(out, cmd, at, &e);
        return;
    }
    if cmd == "mv" {
        io.changed(me.host, &me.abs(src));
    }
    let dst_abs = me.abs(dst);
    io.changed(me.host, &dst_abs);
    // a destination that was a directory received the file under its own name
    if let Some(name) = me.abs(src).rsplit('/').next() {
        io.changed(
            me.host,
            &format!("{}/{name}", dst_abs.trim_end_matches('/')),
        );
    }
}

fn grep(io: &mut dyn Io, me: &Me, args: &[String], stdin: &[String], out: &mut Res) {
    let (flags, ops) = split_args(args);
    let Some((pat, files)) = ops.split_first() else {
        fail_msg(out, "usage: grep [-i] pattern [file..]".to_string());
        out.status = 2;
        return;
    };
    let fold = flags.contains('i');
    let pat = if fold {
        pat.to_lowercase()
    } else {
        pat.to_string()
    };
    let many = files.len() > 1;
    let mut hit = false;
    for (file, lines) in inputs(io, me, "grep", files, stdin, out) {
        for l in lines {
            let found = if fold {
                l.to_lowercase().contains(&pat)
            } else {
                l.contains(&pat)
            };
            if found {
                hit = true;
                out.lines.push(match &file {
                    Some(f) if many => format!("{f}:{l}"),
                    _ => l,
                });
            }
        }
    }
    if !hit && out.status == 0 {
        out.status = 1;
    }
}

fn head_tail(
    io: &mut dyn Io,
    me: &Me,
    cmd: &str,
    args: &[String],
    stdin: &[String],
    out: &mut Res,
) {
    let mut n = 10usize;
    let mut files: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let num = if a == "-n" {
            it.next().map(|s| s.as_str())
        } else if let Some(r) = a.strip_prefix("-n") {
            Some(r)
        } else if a.len() > 1 && a.starts_with('-') && a[1..].chars().all(|c| c.is_ascii_digit()) {
            Some(&a[1..])
        } else {
            files.push(a);
            continue;
        };
        match num.and_then(|s| s.parse().ok()) {
            Some(v) => n = v,
            None => {
                fail_msg(
                    out,
                    format!("{cmd}: {}: invalid number of lines", num.unwrap_or("")),
                );
                return;
            }
        }
    }
    let mut lines = Vec::new();
    for (_, l) in inputs(io, me, cmd, &files, stdin, out) {
        lines.extend(l);
    }
    out.lines = if cmd == "head" {
        lines.into_iter().take(n).collect()
    } else {
        let skip = lines.len().saturating_sub(n);
        lines.into_iter().skip(skip).collect()
    };
}

fn wc(io: &mut dyn Io, me: &Me, args: &[String], stdin: &[String], out: &mut Res) {
    let (flags, files) = split_args(args);
    let any = flags.contains(['l', 'w', 'c']);
    let want = |c: char| !any || flags.contains(c);
    let mut rows = Vec::new();
    let mut errs = std::mem::take(out);
    for (file, lines) in inputs(io, me, "wc", &files, stdin, &mut errs) {
        let mut cols = Vec::new();
        if want('l') {
            cols.push(lines.len());
        }
        if want('w') {
            cols.push(lines.iter().map(|l| l.split_whitespace().count()).sum());
        }
        if want('c') {
            cols.push(lines.iter().map(|l| l.len() + 1).sum());
        }
        let mut row = cols
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(f) = file {
            row = format!("{row} {f}");
        }
        rows.push(row);
    }
    *out = errs;
    out.lines.extend(rows);
}

// ---- manual, help, completion, prompt ----

/// (name, one-line gloss, paragraph)
const PAGES: &[(&str, &str, &str)] = &[
    (
        "ls",
        "list a directory",
        "ls [-l] [-a] [path..] lists what a directory holds. -l shows the type, the owner and the mode of each entry, -a shows the names that start with a dot. /bin lists every command.",
    ),
    (
        "cd",
        "change directory",
        "cd [path] moves you to a directory you may read. Without a path it takes you home. ~ is your home, .. the directory above.",
    ),
    (
        "pwd",
        "print the working directory",
        "pwd prints the absolute path of the directory you are in.",
    ),
    (
        "cat",
        "print files",
        "cat [file..] prints files, one after the other. On a /dev node it asks the machine for its state. With no file it copies its input.",
    ),
    (
        "echo",
        "print words",
        "echo [words..] prints its words. Send it to a file with > to write, or >> to append; send it to a /dev node to give the machine an order.",
    ),
    (
        "touch",
        "make an empty file",
        "touch file.. makes the files that do not exist yet. You need write on the directory.",
    ),
    (
        "mkdir",
        "make directories",
        "mkdir [-p] dir.. makes directories. With -p it makes the missing parents too.",
    ),
    (
        "rm",
        "remove files",
        "rm [-r] path.. removes files. A directory needs -r. You need write on the directory that holds it.",
    ),
    (
        "cp",
        "copy a file",
        "cp src dst copies a file. If dst is a directory the copy lands inside it under the same name.",
    ),
    (
        "mv",
        "move or rename",
        "mv src dst moves or renames. If dst is a directory the file lands inside it. The owner stays.",
    ),
    (
        "grep",
        "find lines",
        "grep [-i] pattern [file..] prints the lines that hold the pattern, from the files or from its input. -i ignores case. It reads only what you may read.",
    ),
    (
        "head",
        "the first lines",
        "head [-n N] [file..] prints the first N lines, ten by default, from the files or from its input.",
    ),
    (
        "tail",
        "the last lines",
        "tail [-n N] [file..] prints the last N lines, ten by default, from the files or from its input.",
    ),
    (
        "wc",
        "count lines, words, bytes",
        "wc [-l] [-w] [-c] [file..] counts the lines, words and bytes of the files or of its input.",
    ),
    (
        "man",
        "the manual",
        "man cmd prints the page for a command. help lists them all.",
    ),
    (
        "help",
        "list the commands",
        "help prints every command on this machine with a line about it.",
    ),
    (
        "whoami",
        "who you are here",
        "whoami prints your user on this machine. Your user decides what you may read and write.",
    ),
    (
        "who",
        "who is here",
        "who prints the people on this machine now.",
    ),
    (
        "uname",
        "the system",
        "uname prints the system name. uname -a adds the version and the hostname.",
    ),
    (
        "hostname",
        "this machine's name",
        "hostname prints the name of the machine you are on.",
    ),
    (
        "history",
        "what you typed",
        "history prints the commands you have run, numbered from 1.",
    ),
    ("clear", "clear the screen", "clear wipes the terminal."),
    (
        "exit",
        "leave",
        "exit ends the session. On a machine you reached with ssh it takes you back to where you came from.",
    ),
    (
        "nano",
        "edit a file",
        "nano file opens the editor on a file, new or old. Ctrl-O saves, Ctrl-X leaves, Ctrl-G helps.",
    ),
    (
        "scan",
        "list the machines near you",
        "scan lists the machines you can reach from here, one a line, with a number to give to ssh.",
    ),
    (
        "ssh",
        "log in to another machine",
        "ssh host moves your shell to another machine, by name, by the start of its name or by its scan number. exit brings you back. A hotel's or an office's host turns you away while its firewall stands.",
    ),
    ("connect", "ssh by another name", "connect host is ssh."),
    (
        "mail",
        "read your mail",
        "mail opens your inbox: space reads, n the next new, esc leaves. mail n prints message n.",
    ),
    (
        "journal",
        "read the journal",
        "journal prints your journal: what you have done and what it has cost.",
    ),
    (
        "door",
        "work the door locks",
        "door release asks the door locks to let go for a while. It writes /dev/door, so you need the right to. On a hotel's or an office's host it wants root (crack its server) and lets go every lock on the floors its riser joins, never across an air gap.",
    ),
    (
        "lift",
        "call the lifts",
        "lift answer makes the lifts answer you. It writes /dev/lift. On a hotel's or an office's host it wants root (crack its server) and calls every lift on its riser.",
    ),
    (
        "cam",
        "blind a camera",
        "cam loop [n] loops a camera feed for a while. It writes /dev/camN. On a hotel's or an office's host it wants root (crack its server), and its cameras are the ones on the floors its riser joins.",
    ),
    (
        "backdoor",
        "plant a key on this host",
        "backdoor writes your rig's key into this user's ~/.ssh/authorized_keys, so you can ssh in as them from home or any terminal for a while, and your rig can put you on the Net at this building's gate. The rig keeps three lines at most. rm the file to burn it.",
    ),
    (
        "burn",
        "burn one of your backdoors",
        "burn <host> destroys a backdoor of yours by its /net name, a prefix or its number. The hole is patched for a day. If the trace is sitting on it, the trace dies there and the building goes on alert.",
    ),
    (
        "route",
        "your route and the trace",
        "route prints the chain of backdoors between the rig and you while you are jacked in from home, each with its health, and the trace walking it back when there is one. Rig only.",
    ),
    (
        "alarm",
        "silence the alarm",
        "alarm off stops the alarm. It writes /dev/alarm, which only root may. On a hotel's or an office's host it wants root too (crack its server).",
    ),
    (
        "plan",
        "read the floor plan",
        "plan prints the plan of the ground floor and loads it into your map.",
    ),
];

fn gloss(name: &str) -> &'static str {
    PAGES.iter().find(|p| p.0 == name).map_or("", |p| p.1)
}

fn help(io: &dyn Io) -> Vec<String> {
    let mut names: Vec<String> = BUILTINS.iter().map(|s| s.to_string()).collect();
    names.extend(io.programs().iter().map(|s| s.to_string()));
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|n| format!("{n:<9}{}", gloss(&n)))
        .collect()
}

pub fn complete(io: &mut dyn Io, shell: &Shell, head: &str, word: &str) -> Vec<String> {
    let before = head.trim_end();
    if before.is_empty() || before.ends_with([';', '|']) {
        let mut names: Vec<String> = BUILTINS.iter().map(|s| s.to_string()).collect();
        names.extend(io.programs().iter().map(|s| s.to_string()));
        names.retain(|n| n.starts_with(word));
        names.sort();
        names.dedup();
        return names;
    }
    let (dir, prefix) = match word.rfind('/') {
        Some(i) => (&word[..=i], &word[i + 1..]),
        None => ("", word),
    };
    let target = if dir.is_empty() { "." } else { dir };
    let fs = io.fs(shell.host);
    let Ok(ids) = fs.list(&shell.user, &shell.cwd, &shell.home, target) else {
        return Vec::new();
    };
    let is_dir = fs
        .resolve(&shell.cwd, &shell.home, target)
        .is_ok_and(|id| matches!(fs.node(id).kind, Kind::Dir(_)));
    if !is_dir {
        return Vec::new();
    }
    ids.into_iter()
        .map(|id| fs.node(id))
        .filter(|n| n.name.starts_with(prefix))
        .filter(|n| prefix.starts_with('.') || !n.name.starts_with('.'))
        .map(|n| {
            let slash = if matches!(n.kind, Kind::Dir(_)) {
                "/"
            } else {
                ""
            };
            format!("{dir}{}{slash}", n.name)
        })
        .collect()
}

/// `user@host:cwd$ `, with `~` for the home and `#` for root.
pub fn prompt(io: &dyn Io, shell: &Shell) -> String {
    let shown = if shell.cwd == shell.home {
        "~".to_string()
    } else if shell.home != "/"
        && let Some(rest) = shell.cwd.strip_prefix(&format!("{}/", shell.home))
    {
        format!("~/{rest}")
    } else {
        shell.cwd.clone()
    };
    let sigil = if shell.user == ROOT { '#' } else { '$' };
    format!("{}@{}:{shown}{sigil} ", shell.user, io.hostname(shell.host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helix::vfs::{GUEST, Mode, USERS};

    const H: HostId = HostId::Rig;

    fn io() -> MemIo {
        let mut v = Vfs::new();
        v.mkdir_as("/bin", ROOT, Mode::RW_R);
        v.mkdir_as("/etc", ROOT, Mode::RW_R);
        v.mkdir_as("/tmp", USERS, Mode::RW_RW);
        v.mkdir_as("/dev", ROOT, Mode::RW_R);
        v.mkdir_as("/home/mara", "mara", Mode::RW_NONE);
        v.mkdir_as("/home/mira", "mira", Mode::RW_NONE);
        v.put("/etc/motd", "welcome\nto the block\n", ROOT, Mode::RW_R);
        v.put("/etc/.hidden", "x\n", ROOT, Mode::RW_R);
        v.put(
            "/home/mara/notes.txt",
            "door is 4471\nbuy milk\n",
            "mara",
            Mode::RW_NONE,
        );
        v.put_dev("/dev/alarm", Dev::Alarm, ROOT, Mode::RW_R);
        v.put_dev("/dev/door", Dev::Door, USERS, Mode::RW_R);
        let mut fs = HashMap::new();
        fs.insert(H, v);
        MemIo { fs }
    }

    fn guest() -> Shell {
        let mut s = Shell::new(H, GUEST, "/tmp");
        s.cwd = "/".to_string();
        s
    }

    fn mara() -> Shell {
        Shell::new(H, "mara", "/home/mara")
    }

    fn sh(io: &mut MemIo, s: &mut Shell, line: &str) -> Vec<String> {
        run(io, s, line).lines
    }

    fn w(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn parse_handles_quotes_semicolons_pipes_and_redirects() {
        let p = parse(r#"echo "a b" 'c;d'; ls -l | grep x >> out"#).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].stages, vec![vec!["echo", "a b", "c;d"]]);
        assert_eq!(p[0].redirect, None);
        assert_eq!(p[1].stages, vec![w("ls -l"), w("grep x")]);
        assert_eq!(p[1].redirect, Some(("out".to_string(), true)));
        let p = parse("echo hi > f;;").unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].redirect, Some(("f".to_string(), false)));
        assert_eq!(parse("   ").unwrap(), vec![]);
        assert!(parse("echo 'oops").unwrap_err().contains("quote"));
        assert!(parse("echo >").is_err());
        assert!(parse("| ls").is_err());
        assert!(parse("ls |").is_err());
        assert!(parse("echo a > f b").is_err());
        // an operator inside quotes is a word
        assert_eq!(
            parse("echo '>' \"|\"").unwrap()[0].stages,
            vec![vec!["echo", ">", "|"]]
        );
    }

    #[test]
    fn ls_cd_pwd_cat_walk_the_tree() {
        let mut io = io();
        let mut s = guest();
        assert_eq!(sh(&mut io, &mut s, "ls"), w("bin dev etc home tmp"));
        assert_eq!(sh(&mut io, &mut s, "cd etc"), Vec::<String>::new());
        assert_eq!(sh(&mut io, &mut s, "pwd"), vec!["/etc"]);
        assert_eq!(sh(&mut io, &mut s, "ls"), vec!["motd"]);
        assert_eq!(sh(&mut io, &mut s, "ls -a"), w(".hidden motd"));
        assert_eq!(
            sh(&mut io, &mut s, "ls -l"),
            vec!["-rw-r-- root          21 /etc/motd"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "cat motd"),
            vec!["welcome", "to the block"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "cat /etc/shadow"),
            vec!["cat: /etc/shadow: No such file or directory"]
        );
        assert_eq!(sh(&mut io, &mut s, "cd .."), Vec::<String>::new());
        assert_eq!(sh(&mut io, &mut s, "pwd"), vec!["/"]);
        assert_eq!(sh(&mut io, &mut s, "cd ~"), Vec::<String>::new());
        assert_eq!(sh(&mut io, &mut s, "pwd"), vec!["/tmp"]);
        assert_eq!(
            sh(&mut io, &mut s, "cd /etc/motd"),
            vec!["cd: /etc/motd: Not a directory"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "cat /etc"),
            vec!["cat: /etc: Is a directory"]
        );
        // a device is rendered by the world
        assert_eq!(sh(&mut io, &mut s, "cat /dev/door"), vec!["dev:Door"]);
        // cd with no argument goes home
        sh(&mut io, &mut s, "cd /etc");
        sh(&mut io, &mut s, "cd");
        assert_eq!(s.cwd, "/tmp");
    }

    #[test]
    fn guest_is_denied_a_home_and_grep_leaks_nothing() {
        let mut io = io();
        let mut s = guest();
        let out = run(&mut io, &mut s, "ls /home/mara");
        assert_eq!(out.lines, vec!["ls: /home/mara: Permission denied"]);
        assert!(out.denied);
        assert_ne!(out.status, 0);
        let out = run(&mut io, &mut s, "cat /home/mara/notes.txt");
        assert_eq!(
            out.lines,
            vec!["cat: /home/mara/notes.txt: Permission denied"]
        );
        assert!(out.denied);
        let out = run(&mut io, &mut s, "grep door /home/mara/notes.txt");
        assert_eq!(
            out.lines,
            vec!["grep: /home/mara/notes.txt: Permission denied"]
        );
        assert!(!out.lines.iter().any(|l| l.contains("4471")));
        assert_eq!(
            sh(&mut io, &mut s, "cd /home/mara"),
            vec!["cd: /home/mara: Permission denied"]
        );
        assert_eq!(s.cwd, "/");
        let out = run(&mut io, &mut s, "echo x > /dev/alarm");
        assert_eq!(out.lines, vec!["helix: /dev/alarm: Permission denied"]);
        assert!(out.denied);
        let out = run(&mut io, &mut s, "cat /dev/alarm");
        assert!(!out.denied, "a guest may read the alarm");
        // the owner reads it
        let mut m = mara();
        assert_eq!(
            sh(&mut io, &mut m, "cat notes.txt"),
            vec!["door is 4471", "buy milk"]
        );
    }

    #[test]
    fn cat_reports_what_it_read_to_the_world() {
        struct Spy(MemIo, Vec<(String, String)>);
        impl Io for Spy {
            fn fs(&mut self, h: HostId) -> &mut Vfs {
                self.0.fs(h)
            }
            fn hostname(&self, h: HostId) -> String {
                self.0.hostname(h)
            }
            fn os_version(&self, h: HostId) -> String {
                self.0.os_version(h)
            }
            fn read_dev(&mut self, h: HostId, d: &Dev, u: &str) -> Result<Vec<String>, String> {
                self.0.read_dev(h, d, u)
            }
            fn write_dev(
                &mut self,
                h: HostId,
                d: &Dev,
                u: &str,
                t: &str,
            ) -> Result<Vec<String>, String> {
                self.0.write_dev(h, d, u, t)
            }
            fn program(
                &mut self,
                _: &mut Shell,
                _: &str,
                _: &[String],
                _: &[String],
            ) -> Option<Result<Output, String>> {
                None
            }
            fn programs(&self) -> Vec<&'static str> {
                Vec::new()
            }
            fn who(&mut self, _: HostId) -> Vec<String> {
                Vec::new()
            }
            fn on_read(&mut self, _: HostId, path: &str, text: &str) {
                self.1.push((path.to_string(), text.to_string()));
            }
        }
        let mut spy = Spy(io(), Vec::new());
        let mut m = mara();
        run(&mut spy, &mut m, "cat notes.txt /dev/door /nope");
        assert_eq!(
            spy.1,
            vec![(
                "/home/mara/notes.txt".to_string(),
                "door is 4471\nbuy milk\n".to_string()
            )]
        );
    }

    #[test]
    fn echo_redirects_create_and_append_files() {
        let mut io = io();
        let mut s = guest();
        s.cwd = "/tmp".to_string();
        assert!(sh(&mut io, &mut s, "echo hello world > a.txt").is_empty());
        assert_eq!(sh(&mut io, &mut s, "cat a.txt"), vec!["hello world"]);
        sh(&mut io, &mut s, "echo more >> a.txt");
        assert_eq!(
            sh(&mut io, &mut s, "cat a.txt"),
            vec!["hello world", "more"]
        );
        sh(&mut io, &mut s, "echo fresh > a.txt");
        assert_eq!(sh(&mut io, &mut s, "cat a.txt"), vec!["fresh"]);
        let id = io.fs(H).resolve("/", "/", "/tmp/a.txt").unwrap();
        assert_eq!(io.fs(H).node(id).owner, GUEST);
        assert_eq!(
            sh(&mut io, &mut s, "echo x > /etc/motd"),
            vec!["helix: /etc/motd: Permission denied"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "echo x > /nope/f"),
            vec!["helix: /nope/f: No such file or directory"]
        );
        // a device takes the joined lines; the world answers
        let mut m = mara();
        assert_eq!(
            sh(&mut io, &mut m, "echo release > /dev/door"),
            vec!["release"]
        );
        // a pipeline redirected: the file gets the lines
        sh(&mut io, &mut s, "cat /etc/motd | grep block > b.txt");
        assert_eq!(sh(&mut io, &mut s, "cat b.txt"), vec!["to the block"]);
    }

    #[test]
    fn errors_go_to_the_screen_never_into_a_pipe_or_a_file() {
        let mut io = io();
        let mut s = guest();
        s.cwd = "/tmp".to_string();
        // a failed stage still creates the (empty) target, as Unix does
        let out = run(&mut io, &mut s, "cat /nope > out");
        assert_eq!(out.lines, vec!["cat: /nope: No such file or directory"]);
        assert_ne!(out.status, 0);
        let id = io.fs(H).resolve("/", "/", "/tmp/out").unwrap();
        assert_eq!(io.fs(H).node(id).kind, Kind::File(String::new()));
        // a denied read appends nothing and shows the player the refusal
        let out = run(&mut io, &mut s, "cat /home/mara/notes.txt >> /tmp/log");
        assert_eq!(
            out.lines,
            vec!["cat: /home/mara/notes.txt: Permission denied"]
        );
        assert!(out.denied);
        let id = io.fs(H).resolve("/", "/", "/tmp/log").unwrap();
        assert_eq!(io.fs(H).node(id).kind, Kind::File(String::new()));
        // errors are not data for the next stage
        let out = run(&mut io, &mut s, "ls /nope | wc -l");
        assert_eq!(out.lines, vec!["0", "ls: /nope: No such file or directory"]);
        // good output still reaches the file while the error shows
        let out = run(&mut io, &mut s, "cat /etc/motd /nope > m");
        assert_eq!(out.lines, vec!["cat: /nope: No such file or directory"]);
        assert_eq!(
            sh(&mut io, &mut s, "cat m"),
            vec!["welcome", "to the block"]
        );
    }

    #[test]
    fn pipes_feed_grep_head_tail_wc() {
        let mut io = io();
        let mut s = mara();
        assert_eq!(
            sh(&mut io, &mut s, "cat /etc/motd | grep BLOCK"),
            Vec::<String>::new()
        );
        assert_eq!(
            sh(&mut io, &mut s, "cat /etc/motd | grep -i BLOCK"),
            vec!["to the block"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "grep -i milk notes.txt /etc/motd"),
            vec!["notes.txt:buy milk"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "cat notes.txt /etc/motd | head -n 2"),
            vec!["door is 4471", "buy milk"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "cat notes.txt /etc/motd | tail -n 1"),
            vec!["to the block"]
        );
        assert_eq!(sh(&mut io, &mut s, "cat /etc/motd | wc"), vec!["2 4 21"]);
        assert_eq!(sh(&mut io, &mut s, "cat /etc/motd | wc -l"), vec!["2"]);
        assert_eq!(sh(&mut io, &mut s, "head -n 1 /etc/motd"), vec!["welcome"]);
        let out = run(&mut io, &mut s, "grep zzz /etc/motd");
        assert!(out.lines.is_empty());
        assert_eq!(out.status, 1);
        assert_eq!(
            sh(&mut io, &mut s, "echo a; echo b | grep b"),
            vec!["a", "b"]
        );
    }

    #[test]
    fn rm_cp_mv_touch_mkdir_behave() {
        let mut io = io();
        let mut s = guest();
        s.cwd = "/tmp".to_string();
        assert!(sh(&mut io, &mut s, "touch a; mkdir d; mkdir -p x/y/z").is_empty());
        assert_eq!(sh(&mut io, &mut s, "ls"), w("a d x"));
        assert_eq!(
            sh(&mut io, &mut s, "mkdir d"),
            vec!["mkdir: d: File exists"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "mkdir p/q"),
            vec!["mkdir: p/q: No such file or directory"]
        );
        sh(&mut io, &mut s, "echo hi > a");
        sh(&mut io, &mut s, "cp a d");
        assert_eq!(sh(&mut io, &mut s, "cat d/a"), vec!["hi"]);
        sh(&mut io, &mut s, "mv a b");
        assert_eq!(sh(&mut io, &mut s, "ls"), w("b d x"));
        assert_eq!(sh(&mut io, &mut s, "rm x"), vec!["rm: x: Is a directory"]);
        sh(&mut io, &mut s, "rm -r x");
        sh(&mut io, &mut s, "rm b");
        assert_eq!(sh(&mut io, &mut s, "ls"), vec!["d"]);
        assert_eq!(sh(&mut io, &mut s, "rm d"), vec!["rm: d: Is a directory"]);
        assert_eq!(
            sh(&mut io, &mut s, "rm nope"),
            vec!["rm: nope: No such file or directory"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "rm /etc/motd"),
            vec!["rm: /etc/motd: Permission denied"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "cp /etc/motd /etc/x"),
            vec!["cp: /etc/x: Permission denied"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "mv /etc/motd /tmp"),
            vec!["mv: /etc/motd: Permission denied"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "touch /etc/z"),
            vec!["touch: /etc/z: Permission denied"]
        );
        assert_eq!(
            sh(&mut io, &mut s, "cp a"),
            vec!["cp: missing file operand"]
        );
    }

    #[test]
    fn command_not_found_and_man_and_help() {
        let mut io = io();
        let mut s = guest();
        let out = run(&mut io, &mut s, "frob now");
        assert_eq!(out.lines, vec!["helix: frob: command not found"]);
        assert_eq!(out.status, 127);
        assert_eq!(
            sh(&mut io, &mut s, "man frob"),
            vec!["No manual entry for frob"]
        );
        for name in BUILTINS
            .iter()
            .chain(["scan", "ssh", "connect", "mail", "journal"].iter())
            .chain(["door", "lift", "cam", "alarm", "plan", "who"].iter())
        {
            let page = sh(&mut io, &mut s, &format!("man {name}"));
            assert!(page.len() >= 2, "{name}: {page:?}");
            assert!(page[0].starts_with(name), "{name}: {page:?}");
        }
        let help = sh(&mut io, &mut s, "help");
        assert_eq!(help.len(), BUILTINS.len());
        assert!(help.iter().any(|l| l.starts_with("cat ")));
        sh(&mut io, &mut s, "echo hi");
        assert_eq!(sh(&mut io, &mut s, "whoami"), vec!["guest"]);
        assert_eq!(sh(&mut io, &mut s, "hostname"), vec!["lowtown"]);
        assert_eq!(sh(&mut io, &mut s, "uname"), vec!["HELIX/OS"]);
        assert_eq!(
            sh(&mut io, &mut s, "uname -a"),
            vec!["HELIX/OS 4.1 lowtown"]
        );
        assert!(sh(&mut io, &mut s, "who").is_empty());
    }

    #[test]
    fn history_numbers_from_one_and_includes_itself() {
        let mut io = io();
        let mut s = guest();
        sh(&mut io, &mut s, "pwd");
        sh(&mut io, &mut s, "");
        assert_eq!(sh(&mut io, &mut s, "history"), vec!["1  pwd", "2  history"]);
    }

    #[test]
    fn prompt_and_completion() {
        let mut io = io();
        let mut s = guest();
        assert_eq!(prompt(&io, &s), "guest@lowtown:/$ ");
        s.cwd = "/tmp".to_string();
        assert_eq!(prompt(&io, &s), "guest@lowtown:~$ ");
        let mut m = mara();
        assert_eq!(prompt(&io, &m), "mara@lowtown:~$ ");
        m.cwd = "/home/mara/mail".to_string();
        assert_eq!(prompt(&io, &m), "mara@lowtown:~/mail$ ");
        let mut r = Shell::new(H, ROOT, "/root");
        r.cwd = "/var/log".to_string();
        assert_eq!(prompt(&io, &r), "root@lowtown:/var/log# ");

        let mut s = guest();
        s.cwd = "/home".to_string();
        assert_eq!(complete(&mut io, &s, "ls ", "mar"), vec!["mara/"]);
        assert_eq!(complete(&mut io, &s, "ls ", "m"), vec!["mara/", "mira/"]);
        assert_eq!(complete(&mut io, &s, "", "ca"), vec!["cat"]);
        assert_eq!(complete(&mut io, &s, "ls | ", "wh"), vec!["who", "whoami"]);
        assert_eq!(complete(&mut io, &s, "echo hi; ", "ec"), vec!["echo"]);
        s.cwd = "/".to_string();
        assert_eq!(complete(&mut io, &s, "cat ", "etc/m"), vec!["etc/motd"]);
        assert_eq!(complete(&mut io, &s, "cat ", "/etc/"), vec!["/etc/motd"]);
        assert_eq!(
            complete(&mut io, &s, "cat ", "/etc/."),
            vec!["/etc/.hidden"]
        );
        // a directory the guest cannot read completes to nothing
        assert!(complete(&mut io, &s, "ls ", "/home/mara/n").is_empty());
        assert!(complete(&mut io, &s, "cat ", "nope/x").is_empty());
    }

    #[test]
    fn nano_asks_the_screen_to_open() {
        let mut io = io();
        let mut s = mara();
        let out = run(&mut io, &mut s, "nano notes.txt");
        assert_eq!(out.open_nano, Some("/home/mara/notes.txt".to_string()));
        assert!(out.lines.is_empty());
        assert_eq!(out.status, 0);
        let out = run(&mut io, &mut s, "nano ~/new.txt");
        assert_eq!(out.open_nano, Some("/home/mara/new.txt".to_string()));
        // nothing was created
        assert!(io.fs(H).resolve("/", "/", "/home/mara/new.txt").is_err());
    }

    #[test]
    fn clear_and_exit_flags() {
        let mut io = io();
        let mut s = guest();
        let out = run(&mut io, &mut s, "clear");
        assert!(out.clear && !out.exit && out.lines.is_empty());
        let out = run(&mut io, &mut s, "exit");
        assert!(out.exit && !out.clear);
        let out = run(&mut io, &mut s, "echo a; clear; echo b");
        assert!(out.clear);
        assert_eq!(out.lines, vec!["a", "b"]);
    }

    #[test]
    fn bin_lists_builtins_and_programs_sorted() {
        struct P(MemIo);
        impl Io for P {
            fn fs(&mut self, h: HostId) -> &mut Vfs {
                self.0.fs(h)
            }
            fn hostname(&self, h: HostId) -> String {
                self.0.hostname(h)
            }
            fn os_version(&self, h: HostId) -> String {
                self.0.os_version(h)
            }
            fn read_dev(&mut self, h: HostId, d: &Dev, u: &str) -> Result<Vec<String>, String> {
                self.0.read_dev(h, d, u)
            }
            fn write_dev(
                &mut self,
                h: HostId,
                d: &Dev,
                u: &str,
                t: &str,
            ) -> Result<Vec<String>, String> {
                self.0.write_dev(h, d, u, t)
            }
            fn program(
                &mut self,
                _shell: &mut Shell,
                name: &str,
                args: &[String],
                _: &[String],
            ) -> Option<Result<Output, String>> {
                match name {
                    "scan" => Some(Ok(Output {
                        lines: vec![format!("scanned {}", args.len())],
                        ..Output::default()
                    })),
                    "door" => Some(Err("door: /dev/door: Permission denied".to_string())),
                    _ => None,
                }
            }
            fn programs(&self) -> Vec<&'static str> {
                vec!["scan", "door"]
            }
            fn who(&mut self, _: HostId) -> Vec<String> {
                vec!["mara   at home".to_string()]
            }
        }
        let mut io = P(io());
        let mut s = guest();
        let bin = run(&mut io, &mut s, "ls /bin").lines;
        let mut want: Vec<String> = BUILTINS.iter().map(|s| s.to_string()).collect();
        want.extend(["scan".to_string(), "door".to_string()]);
        want.sort();
        assert_eq!(bin, want);
        assert_eq!(run(&mut io, &mut s, "scan a b").lines, vec!["scanned 2"]);
        let out = run(&mut io, &mut s, "door release");
        assert_eq!(out.lines, vec!["door: /dev/door: Permission denied"]);
        assert!(out.denied);
        assert_ne!(out.status, 0);
        assert_eq!(complete(&mut io, &s, "", "sc"), vec!["scan"]);
        assert_eq!(run(&mut io, &mut s, "who").lines, vec!["mara   at home"]);
        let help = run(&mut io, &mut s, "help").lines;
        assert_eq!(help.len(), BUILTINS.len() + 2);
        assert!(help.iter().any(|l| l.starts_with("scan ")));
    }
}
