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
//! use mb_resolver::bash::rig::run;
//! use mb_resolver::bashprof::{recorded, BashProf, Profile};
//!
//! let ran = run(&BashProf, &["bash", "build.bash"])?;
//! let forest = recorded(&ran.session)?;
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
//! | `show` | hylic's tree formatter, for either tree |

mod reading;
mod record;
mod show;

use crate::bash::rig::{Failure, Line, Rig, Startup};
use crate::bash::stack;

/// `BASHPROF_TIME_CPS` and the three layers it expands, in every shell.
const BASH: &str = include_str!("bashprof.bash");

/// The bash to put in a [`Startup`], for any rig that wants what bashprof
/// measures. The frame walk comes with it, since a measurement reports one.
pub fn instrument() -> String {
    stack::with(&[BASH])
}

pub use reading::{recorded, Profile, Recorded, Span, Unfinished};
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

    fn startup(&self) -> Startup {
        Startup { bash: instrument(), ..Default::default() }
    }

    fn open(&self) -> Result<Vec<Line>, Failure> {
        Ok(Vec::new())
    }

    fn hear(&self, heard: &mut Vec<Line>, said: Line) -> Result<(), Failure> {
        heard.push(said);
        Ok(())
    }
}
