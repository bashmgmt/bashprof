//! One call, how it went, and the call it was made inside of.

use std::fmt;

use serde::Serialize;

use crate::bash::rig::{Micros, Pid};
use crate::bash::stack::Stack;

/// The name a shell gave one of its calls: `$BASHPID` and a count only that
/// shell advances, which is what makes it unique across a run's process tree.
///
/// Opaque here — it is the shell's word, and nothing reads into it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Id(pub String);

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A measured call, as its BEGIN reported it.
#[derive(Debug, Clone, Serialize)]
pub struct Call {
    pub id: Id,
    pub label: String,
    pub pid: Pid,
    pub began: Micros,

    /// The subject's walk at the moment of the call, from the call site
    /// outward.
    pub stack: Stack,
}

/// A call, and how it went.
///
/// Which of the two a record is says nothing about whether it has children: a
/// call the shell died inside has knowable insides, and the calls made in it
/// named it themselves. Both variants carry the call under one name, so the
/// tag is the only thing that differs between them in Rust and in JSON alike.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Record {
    /// The shell died inside this call.
    Unended { call: Call },

    Ended { call: Call, ended: Micros },
}

impl Record {
    pub fn call(&self) -> &Call {
        match self {
            Self::Unended { call } | Self::Ended { call, .. } => call,
        }
    }
}
