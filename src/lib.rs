//! bashprof: time a tree of calls in a bash program, wherever the program
//! wraps one in `BASHPROF_TIME_CPS`.
//!
//! Nothing is timed in bash. The wire stamps every message with the sending
//! shell's `$EPOCHREALTIME`, so a span is the interval between two of them.
//! Nothing is inferred either: each call is given a name, hands that name to
//! everything it runs, and reports the name it was handed, so the tree travels
//! on the wire rather than being reconstructed from it.
//!
//! ```no_run
//! use mb_resolver::bash::rig::Master;
//! use mb_resolver::bashprof::{recorded, BashProf, Profile};
//!
//! let ran = BashProf.run(&["bash", "build.bash"])?;
//!
//! // Two readings, and each hands back what it did read when it refuses.
//! let forest = recorded(&ran.session).unwrap_or_else(|unread| unread.resolved);
//!
//! match Profile::of(&forest) {
//!     Ok(profile) => println!("{profile}"),
//!     Err(unfinished) => println!("{}", unfinished.resolved),
//! }
//! # Ok::<(), mb_resolver::bash::rig::Failure>(())
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

use crate::bash::rig::{Failure, Line, Master, Rig, Slave};
use crate::bash::stack;

/// `BASHPROF_TIME_CPS`, the word a call site says. Shipped as an asset so a
/// client's copy and the injected one are the same bytes, and naming nothing
/// of the protocol.
pub(crate) const WORDS: &str = include_str!("../../assets/bashprof.bash");

/// `__bp_begin` and `__bp_end`, which are what make that word measure.
pub(crate) const EFFECT: &str = include_str!("effect.bash");

/// The bash a rig hands the subject, for any rig that wants what bashprof
/// measures. The frame walk comes with it, since a measurement reports one.
pub fn instrument() -> String {
    stack::with(&[WORDS, EFFECT])
}

pub use reading::{recorded, Profile, Recorded, Span, Unfinished, Unread};
pub use record::{Call, Complete, Id, Record};

#[cfg(test)]
mod tests;

/// The rig: inject the instrument, keep what the run says.
///
/// The session is every message heard. Which shell sent one and when is on the
/// message already, and so is the call it belongs to, so there is nothing to
/// keep up as they arrive.
pub struct BashProf;

impl Rig for BashProf {
    type Session = Vec<Line>;

    /// The words, their effect, and the frame walk a measurement reports.
    fn bash(&self) -> String {
        stack::with(&[WORDS, EFFECT])
    }

    fn open(&self) -> Result<Vec<Line>, Failure> {
        Ok(Vec::new())
    }

    fn hear(&self, heard: &mut Vec<Line>, said: Line) -> Result<(), Failure> {
        heard.push(said);
        Ok(())
    }
}

/// Either orchestration. A measurement is an interval between two messages, so
/// nothing in the reading depends on who started the shells that sent them.
impl Master for BashProf {}
impl Slave for BashProf {}
