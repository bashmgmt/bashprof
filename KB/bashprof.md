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
| `recording` | a shell's messages → records, already placed | BEGIN opens, END closes the one it opened |
| `nesting` | placed records → a forest | one `Treeish` + one `vec_fold` |
| `profile` | that forest → timings | one `vec_fold` |

The session is `Vec<Line>`; `hear` keeps. Nothing is accumulated as messages
arrive, because nothing has to be — provenance rides along.

## The wrapper places the call

`BASHPROF_TIME_CPS` sends BEGIN, runs the call, sends END. So within one shell
the calls **are** a stack, and a BEGIN belongs to whatever that shell had open
— the same stack the pairing already keeps:

```rust
let inside = match stack.last() {
    Some(&enclosing) => Some(enclosing),
    None => forked_into(&call, index, forked_from, opened),
};
```

Nothing is searched for and nothing can be ambiguous: a shell's open calls are
a chain, never a set. A shell either returns from a call or dies inside it, so
what is left open when its messages run out is the latter — and calls made
inside it are already placed there, which is why an error node has children
here where the resolver's cannot.

Reconstructing this by comparing frame stacks across every record in the run is
the mistake to avoid. It widens the candidate set to things the wrapper had
already excluded, and then needs machinery to exclude them again.

## Where a fork attaches

A fork is the one thing the stack does not cover: it inherits the frames but
begins a stack of its own. A call that opens a fork's stack therefore attaches
into the shell it was forked from — the innermost call still running there,
walking up `rig::forked_from` until a shell has one.

The frames pick it, and this is the only place they are compared:

```rust
call.made_inside(&open.call)     // open's site is a strict suffix of call's
```

A shell blocked on a fork has the same call open throughout, so the choice is
forced. A shell that forked in the *background* may have begun another call
since — and one begun after the fork is not one the inherited site was ever
made under, so it does not match and the walk passes it. That is the whole of
what `&` costs, and it costs it in one `find`.

Shells that never spoke are transparent to all of this: they are not in
`shells()`, and `__BC__owner` is inherited past them, so the fork chain links
the shells that have calls to place.

## Time a span had to itself

Placement is settled; the overlap is not. A span's children do **not** partition
its window: concurrent forks overlap each other, and a backgrounded one can
outlive the call that made it. Summing their durations and subtracting would
count the overlap twice and the part past the window at all, and claim more
time than the span has — an unsigned subtraction that goes negative.

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
