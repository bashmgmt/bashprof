//! Reading a [`Recorded`] forest as timings — one hylic fold.
//!
//! A subtree reads as one measurement exactly when its call ended and every
//! call inside it did. That is a traverse: [`measured`] turns the children's
//! readings into their spans, or into `None` the moment one of them is not a
//! measurement. Nothing tracks whether something went wrong; the shape says.

use std::fmt;
use std::iter::once;

use either::Either::{Left, Right};
use hylic::prelude::{treeish, vec_fold, VecFold, VecHeap, FUSED};
use serde::Serialize;

use serde::Deserialize;

use super::nesting::Recorded;
use super::record::{Call, Complete, Record};
use super::render;

/// One measured call, and the ones made inside it. A call that had not ended
/// would not be here, so this is a [`Complete`] and everything under it.
///
/// Where the call sits among the others is the tree; the stack is not that and
/// cannot be read off it, since an unmeasured function between two measured
/// calls is a frame and not a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub complete: Complete,
    pub children: Vec<Span>,
}

impl Span {
    pub fn call(&self) -> &Call {
        &self.complete.call
    }

    /// BEGIN to END: this span's own work and everything inside it.
    pub fn inclusive(&self) -> u64 {
        self.complete.took()
    }

    /// How much of this window no measured child was running for.
    ///
    /// Children do not partition it: two forks of one line run at once, and a
    /// backgrounded one can outlive the call that made it. Their windows are
    /// clipped to this one and merged, so overlap counts once.
    pub fn exclusive(&self) -> u64 {
        self.inclusive() - self.covered()
    }

    /// How much of this window some child was running for, counted once.
    fn covered(&self) -> u64 {
        let (began, ended) = self.window();
        let mut windows: Vec<(u64, u64)> = self
            .children
            .iter()
            .map(|child| {
                let (from, upto) = child.window();
                (from.max(began), upto.min(ended))
            })
            .filter(|(from, upto)| from < upto)
            .collect();

        windows.sort();
        windows
            .iter()
            .fold((0, began), |(covered, filled), &(from, upto)| {
                (covered + upto.saturating_sub(from.max(filled)), filled.max(upto))
            })
            .0
    }

    fn window(&self) -> (u64, u64) {
        (self.complete.call.sent.sent_at.0, self.complete.ended_at.0)
    }

    pub fn child(&self, label: &str) -> Option<&Span> {
        self.children.iter().find(|span| span.call().label == label)
    }

    /// This span and everything under it, outermost first.
    pub fn all(&self) -> Vec<&Span> {
        once(self).chain(self.children.iter().flat_map(Span::all)).collect()
    }

    /// A span states what a run measured, and nothing outside a run has.
    pub(super) fn of(complete: Complete, children: Vec<Span>) -> Self {
        Self { complete, children }
    }
}

/// The measurements a recorded forest yields, outermost first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub roots: Vec<Span>,
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&render::forest(
            &self.roots,
            |span: &Span| span.children.clone(),
            |span: &Span| {
                format!(
                    "{} {} µs ({} µs of its own) at {} in pid {}",
                    span.call().label,
                    span.inclusive(),
                    span.exclusive(),
                    span.call().stack.at(),
                    span.call().sent.pid
                )
            },
        ))
    }
}

/// The forest held calls the shell died inside, so it is not a whole profile.
///
/// The measurements that did complete come with it; the forest they were read
/// from is the caller's already, and is borrowed.
#[derive(Debug)]
pub struct Unfinished<'a> {
    pub resolved: Profile,
    forest: &'a [Recorded],
}

impl Unfinished<'_> {
    pub fn unended(&self) -> Vec<&Call> {
        Recorded::unended(self.forest)
    }
}

impl fmt::Display for Unfinished<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let labels: Vec<&str> = self.unended().iter().map(|call| call.label.as_str()).collect();

        writeln!(f, "calls that never ended: {labels:?}")?;
        f.write_str(&Recorded::render(self.forest))
    }
}

impl std::error::Error for Unfinished<'_> {}

// ── the catamorphism ─────────────────────────────────────────────────

/// What one subtree reads as: a measurement, or — some call in it never having
/// ended — the measurements that survived it.
type Reading = Result<Span, Vec<Span>>;

/// These readings as spans, if every one of them is a measurement. `None` is
/// the only record that something below never ended, and it is the `collect`
/// that produces it.
fn measured(readings: &[Reading]) -> Option<Vec<Span>> {
    readings.iter().map(|reading| reading.as_ref().ok().cloned()).collect()
}

/// Every complete measurement in these readings, in the order they began.
fn salvage(readings: &[Reading]) -> Vec<Span> {
    let mut spans: Vec<Span> = readings
        .iter()
        .flat_map(|reading| match reading {
            Ok(span) => Left(once(span.clone())),
            Err(spans) => Right(spans.iter().cloned()),
        })
        .collect();

    spans.sort_by_key(|span| span.complete.call.sent.sent_at);
    spans
}

/// A call that ended *around* one that did not is not a measurement either:
/// its own duration is known, but its exclusive time would count work it
/// cannot account for. The rule is the whole subtree or none of it, which is
/// what pairing the node's own record with [`measured`] says.
fn reading() -> VecFold<Recorded, Reading> {
    vec_fold(|heap: &VecHeap<Recorded, Reading>| {
        match (&heap.node.record, measured(&heap.childresults)) {
            (Record::Ended(complete), Some(children)) => {
                Ok(Span::of(complete.clone(), children))
            }
            _ => Err(salvage(&heap.childresults)),
        }
    })
}

impl Profile {
    /// Read a recorded forest as measurements. Fused: the tree is small and
    /// every node is a handful of moves.
    pub fn of(forest: &[Recorded]) -> Result<Self, Unfinished<'_>> {
        let fold = reading();
        let shape = treeish(|node: &Recorded| node.children.to_vec());

        let readings: Vec<Reading> =
            forest.iter().map(|tree| FUSED.run(&fold, &shape, tree)).collect();
        let resolved = Profile { roots: salvage(&readings) };

        match measured(&readings) {
            Some(_) => Ok(resolved),
            None => Err(Unfinished { resolved, forest }),
        }
    }
}
