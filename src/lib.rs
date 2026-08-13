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
//! | `record` | one call and how it went |
//! | `recording` | the wire read as flat records, each with the name it was told encloses it — one pass and a map |
//! | `nesting` | those records read as a tree, which spends the names — one hylic unfold |
//! | `profile` | that tree read as timings — one hylic fold |
//! | `render` | hylic's tree formatter, for either tree |

mod nesting;
mod profile;
mod record;
mod recording;
mod render;

use crate::bash::rig::{Failure, Line, Rig, Startup};
use crate::bash::stack;

/// `BASHPROF_TIME_CPS` and the three layers it expands, in every shell.
const BASH: &str = include_str!("bashprof.bash");

/// The bash to put in a [`Startup`], for any rig that wants what bashprof
/// measures. The frame walk comes with it, since a measurement reports one.
pub fn instrument() -> String {
    stack::with(&[BASH])
}

pub use nesting::Recorded;
pub use profile::{Profile, Span, Unfinished};
pub use record::{Call, Complete, Id, Record};

use nesting::nest;
use recording::records;

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

/// What a run recorded, as the tree its calls made: every call that began,
/// whether or not it ended.
///
/// Reading that as timings is [`Profile::of`], and the caller's — a test bails
/// on a run that died mid-call, a tool reporting what it has need not.
pub fn recorded(heard: &[Line]) -> Result<Vec<Recorded>, Failure> {
    records(heard).map(nest)
}
