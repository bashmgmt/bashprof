# Placing a call in its tree

`tests/examples/bashprof/`, `tests/examples/bash/bashprof.bash`

A worked rig that times a tree of CPS calls. The instrument is nine lines of
bash and measures nothing:

```bash
BASHPROF_TIME_CPS() {
    local __BP_label="${1-}"
    shift || __BC_THROW

    local -a __BP_begin=(BEGIN label "$__BP_label")
    __bc_stack __BP_begin 2

    BC_INSTR say TIME_CPS "${__BP_begin[@]}" || __BC_BAIL

    "$@"
    local __BP_rc=$?

    BC_INSTR say TIME_CPS END || __BC_BAIL

    return "$__BP_rc"
}
```

The wire stamps every message with the sending shell's `$EPOCHREALTIME` and its
pid, and `__bc_stack` appends the frame walk — so a message already carries
when, where and by whom. **Everything downstream is a reading of that**, in
passes over what the run heard:

| pass | | |
|---|---|---|
| `rig::shells` | messages → shells | `seq == 0` opens one |
| `recording` | a shell's messages → flat records | BEGIN opens, END closes the innermost still open |
| `nesting` | flat records → a forest | one `Treeish` + one `vec_fold` |
| `profile` | that forest → timings | one `vec_fold` |

The session is `Vec<Line>`; `hear` keeps. Nothing is accumulated as messages
arrive, because nothing has to be — provenance rides along.

## What places a call

Pairing is per **shell**, not per pid, and never crosses one: within a single
shell the open calls are a stack, since a call either returns or takes the
shell down. What is left open when a shell's messages run out is a call it died
inside.

Where a call sits relative to the others is then a fact about two records, and
it takes both structures a call stands in:

```rust
pub fn encloses(&self, inner: &Call) -> bool {
    self.site_encloses(inner) && self.shell_encloses(inner)
}
```

**The site** — this call's stack is a *strict* suffix of `inner`'s. Strict, so
no call encloses itself and two made from one line enclose neither.

**The shell** — `inner` ran in this call's shell or one forked from it, which
is `rig::forked_from` walked upward and carried on the record.

Neither half is enough alone. A fork inherits the function stack, so a call
measured inside `( … )` reports the frames of the shell that made it — which is
what lets it nest under the call it belongs to, and why arrival order or a
per-pid lane would strand it as a second root. But two forks of *one line*
report the same site as each other, so the stack cannot tell them apart:

```bash
for delay in 0.05 0.01; do
    ( BASHPROF_TIME_CPS turn f__work "$delay" ) &
done
```

Both `turn`s are made at that line, both run at once, and a call inside either
encloses under both by site and by clock. The shell is the only thing left, and
it decides.

Where two calls share a site and overlap without being in separate shells —
two turns of a sequential loop — **when** each ran separates them, which is what
`Record::running_at` is for.

## A tie is a defect, not a choice

The parent is the deepest enclosing record that was running. Two at that depth
would be one call made inside two others, which one shell's stack discipline
rules out; `nest` therefore returns a `Failure` naming both rather than taking
the first. Nothing in a run can produce it, and a reading that says otherwise
is wrong about the run — which is worth hearing.

## Time a span had to itself

A span's children do **not** partition its window. Concurrent forks overlap
each other, and a backgrounded one can outlive the call that made it. Summing
their durations and subtracting would count the overlap twice and the part past
the window at all, and claim more time than the span has — an unsigned
subtraction that goes negative.

So the children's windows are clipped to the parent's and merged, and a span's
own time is what none of them covered. For sequential children that is exactly
the old sum; for concurrent ones it is the only statement that stays true.

## What is an error, and whose

`Profile::of` yields `Result<Profile, Unfinished>`. A subtree reads as a
measurement exactly when its call ended and every call inside it did — a
traverse, so nothing tracks whether something went wrong. A call that ended
*around* one that did not knows its own duration but cannot account for it, so
it is not a measurement either.

`Unfinished` carries the measurements that did complete, which are no less true
for the run having ended badly. A test bails on it; a tool reporting what it has
need not. That choice is made once, at the top.

## See also

- [tree.md](tree.md) — `shells`, `forked_from`, and why the relation is acyclic
- [stack.md](stack.md) — the frame walk both instruments share
- [scoping.md](scoping.md) — why the instrument's locals are declared where they are
- [rig.md](rig.md#what-a-session-is-for) — the session this one keeps
