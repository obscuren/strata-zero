//! HELIX/OS: the small Unix every machine in the city runs.

pub mod bin;
pub mod hosts;
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
}

/// Every host touched this run, kept like the journal.
#[derive(Default)]
pub struct Helix {
    pub hosts: HashMap<HostId, Host>,
}
