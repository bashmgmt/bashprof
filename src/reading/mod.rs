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
//!
//! Two of them can refuse, and each hands back what it did read: [`Unread`]
//! where a message this instrument wrote would not read back, [`Unfinished`]
//! where a shell died inside a call it had begun.

mod flat;
mod show;
mod timings;
mod tree;

use std::fmt;

use crate::bash::rig::{Failure, Said};

pub use timings::{Profile, Span, Unfinished};
pub use tree::Recorded;

/// What a run recorded, as the tree its calls made: every call that began,
/// whether or not it ended.
///
/// Reading that as measurements is [`Profile::of`], and the caller's — a test
/// bails on a run that died mid-call, a tool reporting what it has need not.
pub fn recorded(heard: &[Said<'_>]) -> Result<Vec<Recorded>, Unread> {
    let (records, mut unreadable) = flat::records(heard);
    let read = records.len();
    let resolved = tree::nest(records);

    // A call whose enclosing one was set aside is unreachable from any root, so
    // nesting drops it. The forest's own size is where that shows, and nothing
    // beside it could disagree.
    let dropped = read - nested(&resolved);
    if dropped > 0 {
        unreadable.push(Failure::new(
            "reading the run as a tree",
            format!("{dropped} calls were made inside one that never began"),
        ));
    }

    match unreadable.is_empty() {
        true => Ok(resolved),
        false => Err(Unread { resolved, unreadable }),
    }
}

fn nested(forest: &[Recorded]) -> usize {
    forest.iter().map(|node| 1 + nested(&node.children)).sum()
}

/// The run held messages this instrument wrote and could not read back, so
/// what came out is not the whole of what it recorded.
///
/// Reaching this means a fault in the instrument or on the wire. A run that
/// merely died mid-call reads cleanly and refuses one pass later, at
/// [`Profile::of`] — that is [`Unfinished`], and the two are not the same news.
///
/// What did read comes with it, so a caller that can proceed does.
#[derive(Debug)]
pub struct Unread {
    pub resolved: Vec<Recorded>,
    unreadable: Vec<Failure>,
}

impl Unread {
    pub fn unreadable(&self) -> &[Failure] {
        &self.unreadable
    }
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for why in &self.unreadable {
            writeln!(f, "{why}")?;
        }

        write!(f, "{} calls read", nested(&self.resolved))
    }
}

impl std::error::Error for Unread {}
