# A call tree that travels

`src/bashprof/`, `src/bin/bashprof.rs`

A worked rig that times a tree of CPS calls. The instrument measures nothing
and infers nothing: the wire stamps every message with the sending shell's
`$EPOCHREALTIME` and pid, and the instrument adds a walk, a name, and the name
it was handed — which is what makes the tree an observation rather than a
reconstruction.

## The tool

```
bashprof [--as-recorded] -- <command…>
```

One thing, so no subcommand. It prints the tree and exits with the
**subject's** own status, so a profiled script is indistinguishable from an
unprofiled one. Where the shell died inside a measured call the two halves of
`Profile::of`'s result go to the two streams: the measurements that completed
to stdout, what prevented a whole profile to stderr. `--as-recorded` prints the
tree as recorded instead, unended calls included.

Keeping a script's call sites runnable without the tool is the client's own,
and `assets/bashprof_polyfill.bash` is what it vendors to do it — see
[vendoring.md](vendoring.md).

## The layers are aliases

A call carries three declarations, and each is a layer of its own. They are
**aliases, not functions**: a function would be a frame — one the walk has to
skip, one every call measured below it carries in its own payload, and one more
call per measurement. An alias is the same text in the same frame, so the
layers separate for a reader and cost nothing. What that saves is measured in
[measurements.md](measurements.md#what-a-function-layer-costs-an-instrument).

| layer | declares | read by |
|---|---|---|
| `__BASHPROF_TAKE_STACK` | `__BP_stack` — the call site's walk | the BEGIN |
| `__BASHPROF_TAKE_NAME` | `__BP_id` — this call's name | the BEGIN, and the END |
| `__BASHPROF_HAND_ON` | `__BP_inside` — its own name, for what it runs | every call made inside it |

Every one of them declares in the frame of the word the subject called, which
is what puts it where the rest of that word and everything it runs will read
it — and what a fork inherits. See [scoping.md](scoping.md).

```bash
alias __BASHPROF_TAKE_STACK='
    local -a __BP_stack=()
    __bc_stack __BP_stack $(( 2 + ${__BASHPROF_STACK_SHIFT:-0} ))'
```

The 2 is `__bc_stack`'s own frame and the frame the alias expands into, so the
walk points at the subject rather than at the instrument. It holds wherever the
alias is used, as long as it is used in the body of the word the subject calls.

## `$__BASHPROF_STACK_SHIFT`

Code that wrapped the public word in a word of its own needs the walk to reach
past that too. It says so in the frame it is wrapping from, so the value dies
with that frame:

```bash
measure_step() {
    local __BASHPROF_STACK_SHIFT=1
    BASHPROF_TIME_CPS "$@"
}
```

Read through `:-0` because an **unset** name inside `(( ))` is an error under
`set -u` while an **empty** one is zero — so a subject that never heard of it
pays one parameter expansion and adds nothing.

`__BASHPROF_HAND_ON` clears it, in the same breath as handing the name down:

```bash
local __BP_inside="$__BP_id"
declare -- __BASHPROF_STACK_SHIFT=
```

A shift was for reaching *this* call site. Measured calls made inside this one
have their own, and would otherwise inherit a shift meant for someone else.

Unmeasured code between two measured calls is transparent to all of this: it
neither sets nor shadows either name, so a call made ten frames below the last
measured one still names that one, and still reports its own call site.

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

## The reading

| pass | | |
|---|---|---|
| `recording` | messages → flat records | one pass, one map from name to call |
| `nesting` | records → a forest | one index, then `Treeish` + `vec_fold` |
| `profile` | that forest → timings | one `vec_fold` |

The session is `Vec<Line>`; `hear` keeps. Nothing is grouped by shell, nothing
is paired by position, and nothing depends on the order messages arrived in.

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
subject's whole stack from its own call site upward, so every node has one
definite site, with one frame of the instrument's per enclosing measurement —
where that measurement is executing, and nothing else:

```
c   at    f__B
    outer BASHPROF_TIME_CPS, f__A, BASHPROF_TIME_CPS, main
```

`Span::at` reports the innermost, which is what tells two calls made from one
function apart. At one frame per level a walk 100 deep is a 17 kB payload in
five frames — see
[measurements.md](measurements.md#what-a-function-layer-costs-an-instrument)
for what the same instrument costs when its layers are functions instead.

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
