//! bashprof against a real shell, over the module's own public surface.
//!
//! | | |
//! |---|---|
//! | [`arithmetic`] | what a span had to itself, over trees built by hand |
//! | [`nesting`] | where a call lands: subshells, concurrent forks, names across two |
//! | [`timing`] | a span covers its own work and everything it called |
//! | [`walks`] | the stack a measurement carries, and moving it past a wrapper |
//! | [`unfinished`] | a run the shell died inside, and what survives it |
//! | [`unread`] | a message the instrument mangled, and what survives it |
//! | [`vendoring`] | the word a client ships, and the guard that keeps its effect |

mod arithmetic;
mod nesting;
mod timing;
mod unfinished;
mod unread;
mod vendoring;
mod walks;

use std::collections::HashSet;

use crate::bash::rig::{heard, Driving, ExitStatus};
use crate::bashprof::{recorded, BashProf, Call, Profile, Recorded, Span, Unread};
use crate::tests::scripts::{bash, Scripts};

/// Run a script under the profiler. What comes back is the tree as recorded —
/// every call that began, ended or not. Reading it as timings is the caller's,
/// which is what each test below does next.
fn profiled(script: &str) -> (Vec<Recorded>, ExitStatus) {
    let scripts = Scripts::of(&[("subject.bash", script)]);
    let ran = BashProf.run(&bash(scripts.at("subject.bash"))).unwrap().whole().unwrap();

    (recorded(&heard(&ran.shells)).expect("the instrument's own messages"), ran.subject)
}

/// The labels of the calls that never ended.
fn unended(forest: &[Recorded]) -> Vec<&str> {
    Recorded::unended(forest).iter().map(|call| call.label.as_str()).collect()
}

/// Every call in a recorded forest, outermost first.
fn calls(forest: &[Recorded]) -> Vec<&Call> {
    forest
        .iter()
        .flat_map(|node| std::iter::once(node.record.call()).chain(calls(&node.children)))
        .collect()
}

/// Follow labels down from a root. The tree's shape is what is under test, so
/// a path that does not exist is a failed assertion rather than a `None`.
fn at<'a>(root: &'a Span, path: &[&str]) -> &'a Span {
    path.iter().fold(root, |span, label| {
        span.child(label).unwrap_or_else(|| panic!("no {label:?} under {:?}", span.complete.call.label))
    })
}

/// A → {B → {C, D}, E → F}, with unmeasured work between the measured calls
/// so that a span's own time is not just its children's.
const TREE: &str = r#"
    pause() { sleep "$1"; }

    f__A() {
        pause 0.02
        BASHPROF_TIMETHIS b f__B
        pause 0.02
        BASHPROF_TIMETHIS e f__E
    }

    f__B() {
        BASHPROF_TIMETHIS c f__C
        pause 0.01
        BASHPROF_TIMETHIS d f__D
    }

    f__C() { pause 0.03; }
    f__D() { pause 0.04; }

    f__E() {
        pause 0.01
        BASHPROF_TIMETHIS f f__F
    }

    f__F() { pause 0.05; }

    BASHPROF_TIMETHIS a f__A
    "#;

/// A call that completes inside one the shell then dies in.
const NESTED: &str = r#"
    set -e

    f__inner() { :; }
    f__outer() { BASHPROF_TIMETHIS inner f__inner; false; }

    BASHPROF_TIMETHIS outer f__outer
    "#;


/// A µs budget for scheduling and for the `sleep` each pause forks. Wide,
/// because the bound it guards only has to separate `a`'s own two pauses from
/// the whole tree's time — an order of magnitude apart.
const SLACK: u64 = 60_000;

