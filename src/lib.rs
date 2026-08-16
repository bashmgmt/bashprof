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
//! use bash_interop::rig::{heard, Driving};
//! use bashprof::{recorded, BashProf, Profile};
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), bash_interop::rig::Failure> {
//! let ran = BashProf
//!     .run(&["bash", "build.bash"], |at| vec![at.bc_session(), at.bash_env()])
//!     .await?;
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

use std::sync::Arc;

use bash_interop::rig::{Driving, Failure, Layout, Message, Rig, Serving, Shell};
use bash_interop::stack;

/// `BASHPROF_TIMETHIS`, the word a call site says. Shipped as an asset so a
/// client's copy and the injected one are the same bytes, and naming nothing
/// of the protocol.
pub(crate) const WORDS: &str = include_str!("../assets/bashprof.bash");

/// `__bp_begin` and `__bp_end`, which are what make that word measure.
pub(crate) const EFFECT: &str = include_str!("effect.bash");

/// The join: the words speak under `BASHPROF`, and `$1` is the workspace the
/// invocation hands the rig's bash.
const JOIN: &str = "BC_JOIN BASHPROF \"$1\"\n";

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
pub struct BashProf;

impl Rig for BashProf {
    type Reaction = Vec<Message>;

    fn bash(&self) -> String {
        instrument()
    }

    async fn joined(&self, _at: &Layout, _shell: Arc<Shell>) -> Result<Vec<Message>, Failure> {
        Ok(Vec::new())
    }
}

/// A measurement is an interval between two messages, so nothing in the
/// reading depends on who started the shells that sent them.
impl Driving for BashProf {}

impl Serving for BashProf {}
