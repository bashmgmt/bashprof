//! Flat records read as a tree.
//!
//! Each record names the call it was made inside of, so the edges are given
//! and the shape follows from one index over them: which records name each
//! call. A [`Treeish`](hylic::graph::Treeish) over that index is the tree, and
//! a fold materialises it.
//!
//! Nothing here asks whether a call ended. Where a call sits does not depend
//! on how it went.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use hylic::prelude::{treeish, vec_fold, VecHeap, FUSED};
use serde::{Deserialize, Serialize};

use crate::record::{Call, Id, Record};
use super::records::Placed;
use super::show;

/// A call, and everything called inside it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recorded {
    pub record: Record,
    pub children: Arc<[Recorded]>,
}

impl Recorded {
    /// The calls in this forest the shell died inside, outermost first.
    pub fn unended(forest: &[Recorded]) -> Vec<&Call> {
        forest.iter().flat_map(Recorded::unended_here).collect()
    }

    fn unended_here(&self) -> Vec<&Call> {
        let own = match &self.record {
            Record::Unended(call) => Some(call),
            Record::Ended(_) => None,
        };

        own.into_iter().chain(self.children.iter().flat_map(Recorded::unended_here)).collect()
    }

    /// Every source path a frame in this forest names and does not have,
    /// once each, in the order first met.
    ///
    /// Bash keeps a source path as it was written, so a shell that changed
    /// directory after sourcing leaves a relative one pointing nowhere, and a
    /// workspace the run threw away takes the instrument's own with it.
    pub fn missing(forest: &[Recorded]) -> Vec<&Path> {
        let mut seen: Vec<&Path> = Vec::new();

        for node in forest {
            let own =
                node.record.call().stack.frames().filter_map(|frame| frame.source.missing());

            for path in own.chain(Recorded::missing(&node.children)) {
                if !seen.contains(&path) {
                    seen.push(path);
                }
            }
        }
        seen
    }

    /// The forest as it stands, ended and unended alike.
    pub fn render(forest: &[Recorded]) -> String {
        show::tree(forest, |node: &Recorded| node.children.to_vec(), |node: &Recorded| {
            let call = node.record.call();
            let took = match &node.record {
                Record::Ended(complete) => format!("{} µs", complete.took()),
                Record::Unended(_) => "NEVER ENDED".to_string(),
            };

            format!("{} {took} at {} in pid {}", call.label, call.stack.top(), call.shell.pid)
        })
    }
}

/// The records, the index the shape asks of them — which of them each call was
/// told it holds — and the ones nothing was told to hold. Records come in the
/// order they began, so every list built from them is in that order too.
struct Nesting {
    records: Vec<Record>,
    inside: HashMap<Id, Vec<usize>>,
    roots: Vec<usize>,
}

impl Nesting {
    fn of(read: Vec<Placed>) -> Self {
        let mut inside: HashMap<Id, Vec<usize>> = HashMap::new();
        let mut roots = Vec::new();
        let mut records = Vec::with_capacity(read.len());

        for (index, Placed { record, inside: outer }) in read.into_iter().enumerate() {
            match outer {
                Some(outer) => inside.entry(outer).or_default().push(index),
                None => roots.push(index),
            }

            records.push(record);
        }

        Self { records, inside, roots }
    }

    fn children(&self, of: &Id) -> &[usize] {
        self.inside.get(of).map_or(&[], Vec::as_slice)
    }
}

/// One record, and the neighbourhood it takes to ask for its children.
#[derive(Clone)]
struct Node {
    index: usize,
    nesting: Arc<Nesting>,
}

impl Node {
    fn record(&self) -> &Record {
        &self.nesting.records[self.index]
    }

    fn children(&self) -> Vec<Node> {
        self.nesting
            .children(&self.record().call().id)
            .iter()
            .map(|&index| Node { index, nesting: self.nesting.clone() })
            .collect()
    }
}

/// Placed flat records as the forest their names describe. Past this point what
/// a node is inside of is where it sits.
pub(super) fn nest(read: Vec<Placed>) -> Vec<Recorded> {
    let nesting = Arc::new(Nesting::of(read));
    let shape = treeish(Node::children);
    let build = vec_fold(|heap: &VecHeap<Node, Recorded>| Recorded {
        record: heap.node.record().clone(),
        children: Arc::from(heap.childresults.as_slice()),
    });

    nesting
        .roots
        .iter()
        .map(|&index| Node { index, nesting: nesting.clone() })
        .map(|root| FUSED.run(&build, &shape, &root))
        .collect()
}
