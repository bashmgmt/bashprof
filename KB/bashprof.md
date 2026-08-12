# A call tree that travels

`tests/examples/bashprof/`, `tests/examples/bash/bashprof.bash`

A worked rig that times a tree of CPS calls. The instrument measures nothing
and infers nothing: the wire stamps every message with the sending shell's
`$EPOCHREALTIME` and pid, `__bc_stack` appends the frame walk, and the wrapper
adds the two words that make the tree an observation rather than a
reconstruction.

```bash
BASHPROF_TIME_CPS() {
    local __BP_label="${1-}"
    shift || __BC_THROW

    __BP_made=$(( __BP_made + 1 ))
    local __BP_id="$BASHPID.$__BP_made"

    local -a __BP_begin=(BEGIN id "$__BP_id" inside "${__BP_inside-}" label "$__BP_label")
    __bc_stack __BP_begin 2

    BC_INSTR say TIME_CPS "${__BP_begin[@]}" || __BC_BAIL

    local __BP_inside="$__BP_id"

    "$@"
    local __BP_rc=$?

    BC_INSTR say TIME_CPS END id "$__BP_id" || __BC_BAIL

    return "$__BP_rc"
}
```

| pass | | |
|---|---|---|
| `recording` | messages → flat records | one pass, one map from name to call |
| `nesting` | records → a forest | one index, then `Treeish` + `vec_fold` |
| `profile` | that forest → timings | one `vec_fold` |

The session is `Vec<Line>`; `hear` keeps. Nothing is grouped by shell, nothing
is paired by position, and nothing depends on the order messages arrived in.

## The hand-off is the whole mechanism

`local __BP_inside="$__BP_id"` sits **after** the BEGIN is sent and **before**
the call is run. That ordering is the design:

- reading `${__BP_inside-}` for the payload happens while the name in scope is
  still the caller's, so a call reports the call it was made inside of;
- declaring it `local` puts the new name in this frame, so everything `"$@"`
  reaches reads it by dynamic scoping — see [scoping.md](scoping.md);
- **a fork inherits it** along with the rest of the shell image, so the edge
  crosses a process boundary with nothing to reconstruct.

Unmeasured code between two measured calls is transparent to this: it neither
sets nor shadows the name, so a call made ten frames below the last measured
one still names that one.

## Why the name is `$BASHPID` and a count

Two calls under one name would close each other's spans and claim each other's
children. The name has to be unique across a run's whole process tree, and the
two halves cover the two ways it could fail:

- **`$BASHPID`** is the one value that differs in every process, by definition.
- **`__BP_made`**, deliberately not `local`, is one count per shell spanning
  every call that shell makes. A fork inherits the count and advances its own
  copy — under its own pid, so the two never meet.

`$RANDOM` cannot do either job, and drawing more of it does not help. The
failure mode is not a birthday collision but **determinism after fork**: a
subshell inherits the generator's state, so two forks made from one point draw
the same sequence, and `${RANDOM}${RANDOM}${RANDOM}` appends the same digits in
both. Bash 5 happens to reseed in a subshell — measured on 5.3.9 — but the
manual documents only "seeding with the same constant value produces the same
sequence", and bash 4 does not reseed.

What remains is pid reuse inside one run, which would take the OS cycling the
whole pid space in the seconds a profiled run lasts. That is not guarded
against; it is **detected**, because a second call under one name is refused
where the names are read.

## What the reading owes

Three lookups, and every one of them the instrument's fault if it fails:

| | |
|---|---|
| a name is given once | a second BEGIN under one name is refused |
| a name is ended once | a second END for one name is refused |
| every name pointed at was given | a call made inside a name that never began is refused |

Nothing else can go wrong in the placement, because nothing else is decided
there. In particular there is no ambiguity to report: the shell said where the
call belongs.

## The frames, then

Nothing places a call by them any more, and they stay. Each record carries the
subject's whole stack with exactly two frames dropped — `__bc_stack`'s own and
the wrapper's — so every node has one definite site, and the wrapper's *other*
frames remain because that is where the calls above it are executing:

```
c   at    f__B
    outer BASHPROF_TIME_CPS, f__A, BASHPROF_TIME_CPS, main
```

`Span::at` reports the innermost of these, which is what tells two calls made
from one function apart.

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

- [scoping.md](scoping.md) — the frame `__BP_inside` is declared in, and why
- [stack.md](stack.md) — the frame walk both instruments share
- [wire.md](wire.md#the-message) — the provenance every message carries already
- [rig.md](rig.md#what-a-session-is-for) — the session this one keeps
