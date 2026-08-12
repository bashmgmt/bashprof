//! What the reading does with a tree, over trees built by hand.
//!
//! Everything that needs a real shell is `tests/examples/bashprof.rs`, over
//! this module's public surface. What is left here is arithmetic and shape.

use std::sync::Arc;

use crate::bash::rig::{Micros, Pid};
use crate::bash::stack::Frame;

use super::nesting::Recorded;
use super::profile::{Profile, Span};
use super::record::{Call, Id, Record};

fn call(label: &str, began: u64) -> Call {
    Call {
        id: Id(format!("1.{began}")),
        inside: None,
        label: label.into(),
        pid: Pid(1),
        began: Micros(began),
        at: Frame { funcname: "f".into(), source: "/x.bash".into(), lineno: 1, args: None },
        outer: Vec::new(),
    }
}

fn node(record: Record, children: Vec<Recorded>) -> Recorded {
    Recorded { record, children: Arc::from(children) }
}

fn span(label: &str, began: u64, ended: u64, children: Vec<Span>) -> Span {
    Span::of(&call(label, began), Micros(ended), children)
}

/// Two children overlapping each other and a third outliving its parent: the
/// time a span had to itself is what none of them covered. Subtracting their
/// durations would count the overlap twice and the part beyond the window at
/// all, and claim more time than the span has.
#[test]
fn a_spans_own_time_is_what_no_child_was_running_for() {
    let a = span(
        "a",
        0,
        100,
        vec![
            span("x", 10, 60, Vec::new()),
            span("y", 40, 90, Vec::new()),
            span("z", 95, 200, Vec::new()),
        ],
    );

    assert_eq!(a.inclusive(), 100);
    assert_eq!(a.exclusive(), 15, "0..10 and 90..95, and nothing else");
}

/// Nesting cannot produce this — a shell that dies inside a call leaves every
/// call it was made from open too — but the reading takes a tree, not that
/// builder's output, and is total over one.
///
/// The unended child has nothing under it, so it reads as `Err(vec![])`.
/// Asking whether anything survived would lose it; asking whether every child
/// is a measurement does not.
#[test]
fn a_call_that_ended_around_one_that_did_not_is_no_measurement_either() {
    let forest = [node(
        Record::Ended { call: call("outer", 0), ended: Micros(100) },
        vec![
            node(Record::Ended { call: call("done", 10), ended: Micros(20) }, Vec::new()),
            node(Record::Unended(call("open", 30)), Vec::new()),
        ],
    )];

    let unfinished = Profile::of(&forest).expect_err("something under it never ended");

    assert_eq!(unfinished.unended().len(), 1);
    assert_eq!(
        unfinished.resolved.roots.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(),
        ["done"],
        "`outer` knows its own duration but cannot account for it, so it is not a span"
    );
}
