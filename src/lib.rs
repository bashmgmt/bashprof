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
//! use bash_interop::rig::{heard, Driving, Provision, Rig};
//! use bashprof::{recorded, BashProf, Profile};
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), bash_interop::rig::Failure> {
//! let ran = BashProf
//!     .run(&["bash", "build.bash"], |at| {
//!         Ok(vec![at.bash_env(Provision::Joining(&BashProf.joining(at)))?])
//!     })
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

/// The definitions a rig hands the subject, for any rig that wants what
/// bashprof measures: the words speak under `BASHPROF`, the frame walk comes
/// with them since a measurement reports one, and `BASHPROF_INIT <dir>` is
/// the channel setup on offer — defined here, called by nothing here.
pub fn instrument() -> String {
    const INIT: &str = r#"
BASHPROF_INIT() {
    BC_JOIN BASHPROF "${1:?the session workspace}"
}
"#;
    stack::with_walk(&[WORDS, EFFECT, INIT])
}

/// The standard initiation: `BASHPROF_INIT '<dir>'`. Data — written into a
/// provisioned `bash_env.bash`, or said by a client's own line.
pub fn joining(at: &Layout) -> String {
    format!("BASHPROF_INIT {}\n", bash_strings::emit_scalar(at.text()))
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

    fn bash(&self, _at: &Layout) -> String {
        instrument()
    }

    fn joining(&self, at: &Layout) -> String {
        joining(at)
    }

    async fn joined(&self, _at: &Layout, _shell: Arc<Shell>) -> Result<Vec<Message>, Failure> {
        Ok(Vec::new())
    }
}

/// A measurement is an interval between two messages, so nothing in the
/// reading depends on who started the shells that sent them.
impl Driving for BashProf {}

impl Serving for BashProf {}
