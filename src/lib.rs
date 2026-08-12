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
//! The modules are private and this list of exports is the API; each is one
//! step of the reading.
//!
//! | | |
//! |---|---|
//! | `record` | one call, how it went, and the call it was made inside of |
//! | `recording` | the wire read as flat records — one pass and a map |
//! | `nesting` | those records read as a tree — one hylic unfold |
//! | `profile` | that tree read as timings — one hylic fold |
//! | `render` | hylic's tree formatter, for either tree |

mod instrument;
mod nesting;
mod profile;
mod record;
mod recording;
mod render;

use crate::bash::rig::{Failure, Line, Rig, Startup};

pub use instrument::{instrument, POLYFILL};
pub use nesting::{nest, Recorded};
pub use profile::{Profile, Span, Unfinished};
pub use record::{Call, Id, Record};
pub use recording::records;

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
