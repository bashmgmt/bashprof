//! One call, and how it went.
//!
//! Each type here wraps the one before it and adds what its own message
//! carried. A BEGIN is a [`Call`]; the END that closes it makes a
//! [`Complete`]; the calls made inside one make a
//! [`Span`](super::Span). Nothing is restated, so nothing can disagree.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use bash_interop::rig::{Micros, Shell, Stamp};
use bash_interop::stack::Stack;

/// The name a shell gave one of its calls: `$BASHPID` and a count only that
/// shell advances, which is what makes it unique across a run's process tree.
///
/// Opaque here — it is the shell's word, and nothing reads into it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id(pub String);

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A call that began: everything its BEGIN reported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    pub id: Id,

    /// What the call site named this measurement.
    pub label: String,

    /// The command being measured, as the call site wrote it — the words left
    /// after the label.
    pub argv: Vec<String>,

    /// The subject's walk at the moment of the call, from the call site
    /// outward.
    pub stack: Stack,

    /// Which shell said so — the one the walk above was taken in.
    pub shell: Arc<Shell>,

    /// When it said so.
    pub stamp: Stamp,
}

/// A call that also ended: what it began as, and the two things its END
/// carried back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Complete {
    pub call: Call,

    /// The sending shell's clock at the END.
    pub ended_at: Micros,

    /// What the measured command returned.
    pub status: u8,
}

impl Complete {
    /// BEGIN to END: this call's own work and everything inside it.
    pub fn took(&self) -> u64 {
        self.ended_at.0 - self.call.stamp.sent_at.0
    }
}

/// A call as the run left it. Which variant says nothing about whether it has
/// children: a call the shell died inside has knowable insides, and the calls
/// made in it named it themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Record {
    /// The shell died inside this call, so no END arrived.
    Unended(Call),

    Ended(Complete),
}

impl Record {
    pub fn call(&self) -> &Call {
        match self {
            Self::Unended(call) => call,
            Self::Ended(complete) => &complete.call,
        }
    }
}
