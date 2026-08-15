//! bashprof: time a tree of calls in a bash program, wherever the program
//! wraps one in `BASHPROF_TIMETHIS`.
//!
//! Nothing is timed in bash. The wire stamps every message with the sending
//! shell's `$EPOCHREALTIME`, so a span is the interval between two of them.
//! Nothing is inferred either: each call is given a name, hands that name to
//! everything it runs, and reports the name it was handed, so the tree travels
//! on the wire rather than being reconstructed from it.
//!
//! ```no_run
//! use mb_resolver::bash::rig::{heard, Driving, Reaching};
//! use mb_resolver::bashprof::{recorded, BashProf, Profile};
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), mb_resolver::bash::rig::Failure> {
//! let ran = BashProf { reaching: Reaching::BashEnv }.run(&["bash", "build.bash"]).await?;
//!
//! // Two readings, and each hands back what it did read when it refuses.
//! let forest = recorded(&heard(&ran.shells)).unwrap_or_else(|unread| unread.resolved);
//!
//! match Profile::of(&forest) {
//!     Ok(profile) => println!("{profile}"),
//!     Err(unfinished) => println!("{}", unfinished.resolved),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! The modules are private, and [`recorded`] is the whole reading. Each module
//! is one step of it, and what passes between them is nobody else's:
//!
//! | | |
//! |---|---|
//! | `record` | one call and how it went — the vocabulary |
//! | `reading` | the three passes from messages to measurements |

pub(crate) mod reading;
pub(crate) mod record;

use std::ffi::OsString;
use std::sync::Arc;

use crate::bash::rig::{
    Driving, Failure, Layout, Message, Reaching, Rig, Serving, Setup, Shell, Workspace,
};
use crate::bash::stack;

/// `BASHPROF_TIMETHIS`, the word a call site says. Shipped as an asset so a
/// client's copy and the injected one are the same bytes, and naming nothing
/// of the protocol.
pub(crate) const WORDS: &str = include_str!("../../assets/bashprof.bash");

/// `__bp_begin` and `__bp_end`, which are what make that word measure.
pub(crate) const EFFECT: &str = include_str!("effect.bash");

/// The label bashprof's word speaks under.
const JOIN: &str = "BC_JOIN BASHPROF\n";

/// The bash a rig hands the subject, for any rig that wants what bashprof
/// measures. The frame walk comes with it, since a measurement reports one.
pub fn instrument() -> String {
    stack::with_walk(&[WORDS, EFFECT, JOIN])
}

pub use reading::{recorded, Profile, Recorded, Span, Unfinished, Unread};
pub use record::{Call, Complete, Id, Record};

#[cfg(test)]
mod tests;

/// The rig: inject the instrument, keep what each shell says.
///
/// Every message carries the name of the call it belongs to, so the tree is on
/// the wire and there is nothing to keep up as they arrive — the reaction is
/// the one that keeps every message.
pub struct BashProf {
    /// How a driven subject's shells find the instrument.
    pub reaching: Reaching,
}

impl Rig for BashProf {
    type Reaction = Vec<Message>;

    fn setup(&self) -> Setup {
        Setup { bash: instrument(), workspace: Workspace::Temporary }
    }

    async fn joined(&self, _at: &Layout, _shell: Arc<Shell>) -> Result<Vec<Message>, Failure> {
        Ok(Vec::new())
    }
}

/// Either orchestration. A measurement is an interval between two messages, so
/// nothing in the reading depends on who started the shells that sent them.
impl Driving for BashProf {
    fn environment(&self, at: &Layout) -> Vec<(OsString, OsString)> {
        self.reaching.environment(at)
    }
}

impl Serving for BashProf {}
