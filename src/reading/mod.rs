//! What a run's messages read as, in three passes.
//!
//! | | | |
//! |---|---|---|
//! | [`flat`] | messages → records, each with the name it was told encloses it | one pass, one map from name to call |
//! | [`tree`] | those → a forest, the names spent | one index, then `Treeish` + `vec_fold` |
//! | [`timings`] | that forest → measurements | one `vec_fold` |
//!
//! [`recorded`] is the whole of it and the only way in; what passes between
//! the passes is nobody else's. `show` is hylic's tree formatter, which the
//! last two passes both render through.

mod flat;
mod show;
mod timings;
mod tree;

pub use timings::{Profile, Span, Unfinished};
pub use tree::Recorded;

use crate::bash::rig::{Failure, Line};

/// What a run recorded, as the tree its calls made: every call that began,
/// whether or not it ended.
///
/// Reading that as measurements is [`Profile::of`], and the caller's — a test
/// bails on a run that died mid-call, a tool reporting what it has need not.
pub fn recorded(heard: &[Line]) -> Result<Vec<Recorded>, Failure> {
    flat::records(heard).map(tree::nest)
}
