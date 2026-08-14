//! The wire read as flat records.
//!
//! Every BEGIN carries the name its shell gave the call and the name of the
//! call it was made inside of, so the tree is already on the wire. Reading it
//! is one pass over everything the run heard, with a map from name to call —
//! no grouping by shell, no pairing by position, and nothing that depends on
//! the order messages arrived in.
//!
//! What this pass owes the rest of the module is that the names hold up: a
//! name is given once, ended once, and every name a call points at is one
//! that was given. Each is a lookup, and each failure is the instrument's.

use std::collections::HashMap;

use crate::bash::rig::{field, Doing, Failure, Line, Micros};
use crate::bash::stack::Columns;
use crate::bash::value::parse_array;

use crate::bashprof::record::{Call, Complete, Id, Record};

/// The word this instrument's messages begin with.
const TAG: &str = "TIME_CPS";

/// One call the run made, and the name its shell said it was made inside of.
///
/// That name is what [`nest`](super::tree::nest) reads, and the tree it builds holds
/// the same fact in its shape, so it stops here.
pub(super) struct Read {
    pub record: Record,
    pub inside: Option<Id>,
}

/// Every call the run made, in the order they began, and every message that
/// would not read back.
///
/// A message set aside is a fault in the instrument. What the others said is
/// no less true for it, so the pass carries on — and a call whose enclosing
/// one was set aside falls out of the tree by itself, which is what the next
/// pass counts.
pub(super) fn records(heard: &[Line]) -> (Vec<Read>, Vec<Failure>) {
    let mut reading = Reading::default();
    let mut unreadable = Vec::new();

    for line in heard {
        if let Err(why) = reading.hear(line) {
            unreadable.push(why);
        }
    }

    (reading.settled(), unreadable)
}

/// What an END carries back.
struct Closed {
    ended_at: Micros,
    status: u8,
}

/// A call while the run is still being read. It gains an end when its END
/// arrives, and a shell that dies inside it never sends one.
struct Open {
    call: Call,
    inside: Option<Id>,
    closed: Option<Closed>,
}

#[derive(Default)]
struct Reading {
    opened: Vec<Open>,

    /// Where each name's call is, which is what makes both an END and a
    /// parent a lookup rather than a search.
    at: HashMap<Id, usize>,
}

impl Reading {
    /// One message. Anything not this instrument's is someone else's.
    fn hear(&mut self, line: &Line) -> Result<(), Failure> {
        let Some(payload) = line.behind(TAG) else { return Ok(()) };
        let Some((kind, rest)) = payload.split_first() else {
            return Err(reading("an empty TIME_CPS message"));
        };

        match kind.as_str() {
            "BEGIN" => self.begin(line, rest),
            "END" => self.end(rest, line.sent.sent_at),
            other => Err(reading(format!("unknown kind {other:?}"))),
        }
    }

    /// A name is given once: two calls under one name would close each
    /// other's spans and claim each other's children.
    ///
    /// Nothing is written before every check has passed, so a message set
    /// aside leaves the reading as it found it.
    fn begin(&mut self, line: &Line, rest: &[String]) -> Result<(), Failure> {
        let (call, inside) = began(line, rest)?;

        if self.at.contains_key(&call.id) {
            return Err(reading(format!("a second call named {}", call.id)));
        }

        self.at.insert(call.id.clone(), self.opened.len());
        self.opened.push(Open { call, inside, closed: None });
        Ok(())
    }

    fn end(&mut self, rest: &[String], ended_at: Micros) -> Result<(), Failure> {
        let id = named(rest)?;
        let status = field(rest, "status")
            .ok_or_else(|| reading(format!("an END for {id} with no status")))?;
        let status = status
            .parse()
            .map_err(|_| reading(format!("an END for {id} with status {status:?}")))?;

        let unknown = || reading(format!("an END for {id}, which never began"));
        let open = &mut self.opened[*self.at.get(&id).ok_or_else(unknown)?];

        if open.closed.is_some() {
            return Err(reading(format!("a second END for {id}")));
        }

        open.closed = Some(Closed { ended_at, status });
        Ok(())
    }

    /// The calls as records, oldest first.
    fn settled(self) -> Vec<Read> {
        let Reading { opened, .. } = self;

        let mut records: Vec<Read> = opened
            .into_iter()
            .map(|Open { call, inside, closed }| Read {
                record: match closed {
                    Some(Closed { ended_at, status }) => {
                        Record::Ended(Complete { call, ended_at, status })
                    }
                    None => Record::Unended(call),
                },
                inside,
            })
            .collect();

        records
            .sort_by_key(|read| (read.record.call().sent.sent_at, read.record.call().sent.pid.0));
        records
    }
}

/// The `id` an END names.
fn named(rest: &[String]) -> Result<Id, Failure> {
    field(rest, "id").map(|id| Id(id.to_string())).ok_or_else(|| reading("a message with no id"))
}

/// The call one BEGIN reports, and the name it says encloses it. The outermost
/// call is made inside nothing, and says so with a name it leaves empty.
fn began(line: &Line, rest: &[String]) -> Result<(Call, Option<Id>), Failure> {
    let word = |key: &str| {
        field(rest, key).ok_or_else(|| reading(format!("a BEGIN with no {key:?}"))).map(str::to_string)
    };

    let call = Call {
        id: named(rest)?,
        label: word("label")?,
        argv: parse_array(&word("argv")?).doing(|| "reading a BEGIN's argv".to_string())?,
        stack: Columns::of(rest)?.frames()?,
        sent: line.sent.clone(),
    };

    Ok((call, Some(word("inside")?).filter(|inside| !inside.is_empty()).map(Id)))
}

fn reading(what: impl Into<String>) -> Failure {
    Failure::new("reading a span", what.into())
}
