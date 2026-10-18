//! HELIX/OS: the small Unix every machine in the city runs.

pub mod bin;
pub mod hosts;
pub mod nano;
pub mod shell;
pub mod vfs;

use shell::HostId;
use std::collections::HashMap;
use vfs::Vfs;

/// One account on a host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub name: String,
    pub home: String,
    pub display: String,
    pub person: Option<sim::PersonId>,
}

/// One machine: its name, its disk and who has an account on it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Host {
    pub name: String,
    pub fs: Vfs,
    pub users: Vec<User>,
    /// The device theme the machine wears: municipal, megacorp, neon or street.
    pub theme: String,
    /// The door codes the generator wrote into residents' notes: only these teach.
    pub planted: Vec<Planted>,
    /// On the rig: how many of the journal's mails have had a file put down for them,
    /// so a mail you delete stays deleted.
    pub mails_placed: usize,
}

/// A door code written into a notes file: the flat it opens, the code, the file and
/// the resident whose notes they are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planted {
    pub flat: (i32, u16),
    pub code: u16,
    pub path: String,
    pub by: String,
}

/// Every host touched this run, kept like the journal.
#[derive(Default)]
pub struct Helix {
    pub hosts: HashMap<HostId, Host>,
}
