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
use crate::bash::value::parse_array;
use crate::bash::stack::Columns;

use super::record::{Call, Complete, Id, Record};

/// The word this instrument's messages begin with.
const TAG: &str = "TIME_CPS";

/// One call the run made, and the name its shell said it was made inside of.
///
/// That name is what [`nest`](super::nest) reads, and the tree it builds holds
/// the same fact in its shape, so it stops here.
pub(super) struct Read {
    pub record: Record,
    pub inside: Option<Id>,
}

/// Every call the run made, in the order they began.
pub(super) fn records(heard: &[Line]) -> Result<Vec<Read>, Failure> {
    let mut reading = Reading::default();

    for line in heard {
        reading.hear(line)?;
    }

    reading.settled()
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
    fn begin(&mut self, line: &Line, rest: &[String]) -> Result<(), Failure> {
        let (call, inside) = began(line, rest)?;

        if self.at.insert(call.id.clone(), self.opened.len()).is_some() {
            return Err(reading(format!("a second call named {}", call.id)));
        }

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

    /// The calls as records, oldest first — once every name they point at is
    /// one that was given.
    fn settled(self) -> Result<Vec<Read>, Failure> {
        let Reading { opened, at } = self;

        let named_but_absent = opened
            .iter()
            .filter_map(|open| open.inside.as_ref().map(|inside| (&open.call.id, inside)))
            .find(|(_, inside)| !at.contains_key(inside));

        if let Some((call, missing)) = named_but_absent {
            return Err(reading(format!("{call} was made inside {missing}, which never began")));
        }

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
        Ok(records)
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
