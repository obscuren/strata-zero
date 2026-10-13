//! The message log: what happened, newest last.

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A plain result.
    Info,
    /// A refusal; magenta with a leading `!`.
    Warn,
    /// Quest and level lines; accent with a leading `▸`.
    Story,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub text: String,
    pub kind: Kind,
    /// The tick the entry arrived.
    pub tick: u64,
}

impl Entry {
    /// The text with its kind's marker in front.
    pub fn marked(&self) -> String {
        match self.kind {
            Kind::Info => self.text.clone(),
            Kind::Warn => format!("! {}", self.text),
            Kind::Story => format!("▸ {}", self.text),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MessageLog {
    entries: VecDeque<Entry>,
}

impl MessageLog {
    pub const CAP: usize = 200;
    /// How long the newest entry counts as recent.
    pub const RECENT_TICKS: u64 = 90;

    pub fn push(&mut self, kind: Kind, text: impl Into<String>, tick: u64) {
        if self.entries.len() == Self::CAP {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry {
            text: text.into(),
            kind,
            tick,
        });
    }

    /// The newest entry, however old.
    pub fn latest(&self) -> Option<&Entry> {
        self.entries.back()
    }

    /// The newest entry while it is younger than `RECENT_TICKS`.
    pub fn recent(&self, tick: u64) -> Option<&Entry> {
        self.latest()
            .filter(|e| tick.saturating_sub(e.tick) < Self::RECENT_TICKS)
    }

    /// Oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_caps_and_reports_recent() {
        let mut log = MessageLog::default();
        assert!(log.is_empty() && log.latest().is_none() && log.recent(0).is_none());
        log.push(Kind::Info, "took it", 10);
        log.push(Kind::Warn, "no", 20);
        assert_eq!(log.len(), 2);
        assert_eq!(log.recent(20).map(|e| e.text.as_str()), Some("no"));
        assert_eq!(log.recent(109).map(|e| e.text.as_str()), Some("no"));
        assert!(log.recent(110).is_none());
        assert_eq!(log.latest().map(|e| e.marked()), Some("! no".to_string()));
        log.push(Kind::Story, "level 2", 30);
        assert_eq!(log.latest().unwrap().marked(), "▸ level 2");
        let texts: Vec<&str> = log.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, vec!["took it", "no", "level 2"]);
        for i in 0..MessageLog::CAP {
            log.push(Kind::Info, format!("m{i}"), 100 + i as u64);
        }
        // 203 pushes into a 200-cap log: the three first entries are gone.
        assert_eq!(log.len(), MessageLog::CAP);
        assert_eq!(log.iter().next().unwrap().text, "m0");
        assert_eq!(log.latest().unwrap().text, "m199");
    }
}
