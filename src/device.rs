//! Source-independent device model, and the tracker that merges scans over time.

use std::collections::HashSet;

/// Also the display order on the taskbar.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Kind {
    Headset,
    Earbuds,
    Mouse,
    Keyboard,
    Gamepad,
    Other,
}

impl Kind {
    pub fn from_name(name: &str) -> Kind {
        let n = name.to_ascii_lowercase();
        if ["headset", "headphone", "airpods max"].iter().any(|k| n.contains(k)) {
            Kind::Headset
        } else if ["earbud", "buds", "airpods", "earphone", "in-ear"].iter().any(|k| n.contains(k)) {
            Kind::Earbuds
        } else if n.contains("mouse") {
            Kind::Mouse
        } else if n.contains("keyboard") || n.contains("keys") {
            Kind::Keyboard
        } else if n.contains("controller") || n.contains("gamepad") {
            Kind::Gamepad
        } else {
            Kind::Other
        }
    }
}

/// What a source saw for one device during one scan.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Battery { percent: u8, charging: bool },
    /// Known to exist but didn't answer (asleep, or off; often indistinguishable).
    Unreachable,
    /// Definitely not connected (e.g. headset off while its dongle is plugged).
    Off,
}

#[derive(Clone)]
pub struct Observation {
    /// Stable per device across scans, e.g. "logi:c539:2:01" or "ble:f4ee25fbba46".
    pub id: String,
    /// None when the source can't identify the device right now (only updates known devices).
    pub name: Option<String>,
    pub kind: Option<Kind>,
    pub state: State,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status {
    /// `stale`: last known reading of a device that's asleep right now.
    Battery { percent: u8, charging: bool, stale: bool },
    NotConnected,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Battery { percent, charging, stale } => {
                write!(f, "{percent}%")?;
                if *charging {
                    write!(f, " (charging)")?;
                }
                if *stale {
                    write!(f, " (asleep)")?;
                }
                Ok(())
            }
            Status::NotConnected => write!(f, "not connected"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Reading {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub status: Status,
}

/// Remembers every device seen this session, so a device that disappears shows as
/// "not connected" and a sleeping one keeps its last battery reading.
#[derive(Default)]
pub struct Tracker {
    known: Vec<Reading>,
}

impl Tracker {
    pub fn update(&mut self, observations: Vec<Observation>) -> Vec<Reading> {
        let mut seen = HashSet::new();
        for o in observations {
            seen.insert(o.id.clone());
            let pos = self.known.iter().position(|r| r.id == o.id);
            let status = match (o.state, pos.map(|p| self.known[p].status)) {
                (State::Battery { percent, charging }, _) => Status::Battery { percent, charging, stale: false },
                // Asleep: battery hasn't changed, keep the last reading but mark it.
                (State::Unreachable, Some(Status::Battery { percent, charging, .. })) => {
                    Status::Battery { percent, charging, stale: true }
                }
                (State::Unreachable | State::Off, _) => Status::NotConnected,
            };
            match pos {
                Some(p) => {
                    let r = &mut self.known[p];
                    r.status = status;
                    if let Some(name) = o.name {
                        r.name = name;
                    }
                    if let Some(kind) = o.kind {
                        r.kind = kind;
                    }
                }
                None => {
                    let Some(name) = o.name else { continue };
                    let kind = o.kind.unwrap_or_else(|| Kind::from_name(&name));
                    self.known.push(Reading { id: o.id, name, kind, status });
                }
            }
        }
        for r in &mut self.known {
            if !seen.contains(&r.id) {
                r.status = Status::NotConnected;
            }
        }
        self.known.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.cmp(&b.name)));
        self.known.clone()
    }
}
