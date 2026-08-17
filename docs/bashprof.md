# The call tree

`src/bashprof/`, `src/bin/bashprof.rs`

A rig that times a tree of calls. The instrument measures nothing and infers
nothing. The wire stamps every message with the sending shell's
`$EPOCHREALTIME` and pid, and the instrument adds a walk, a name, and the name
it was handed, so the tree is assembled from what the shells reported.

## The tool

Two verbs, differing in who started the shells. Both take the same options,
from the same `Reading` struct:

```
bashprof run   [--reach bash-env|by-hand] --into FILE [--output …] -- <command…>
bashprof serve --at DIR --into FILE [--output …]
```

| | who starts the shells | how they are reached | its exit code |
|---|---|---|---|
| `run` | the tool, from the command line it was given | `BASHPROF_SESSION` in the environment always; `--reach bash-env` (the default) also `BASH_ENV`, so the whole process tree joins; `--reach by-hand` leaves it to the scripts | whatever the subject exited with |
| `serve` | a bash script, which named and made the workspace (`--at`, required, existing) and started this process as a coprocess | its own choice — the workspace is the address; the join fifo in it says the session is up, and the script sources the laid files and initiates by the same dir (`BASHPROF_INIT`) | its own: 0, or 1 if the reading did not come out |

`serve` is the shipped half of a client's own coprocess —
`coproc SERVER { bashprof serve --at "$PWD/prof.d" --into build.times; }`.
Nothing about the reading changes
between the two, and `__fixtures/joined/build.bash` is the client story in
one file ([tests/cli.rs](../tests/cli.rs)). `run --help` and `serve --help` end
with every way a script joins, in this tool's words.

```rust
pub struct BashProf;   // drive it: BashProf.run(argv, |at| …); or serve it
```

`--output` chooses how far the run is read, and in what form:

| `--output` | the file holds | refuses |
|---|---|---|
| `human` (default) | the measured tree indented, one call per line | a run with a call the shell died inside |
| `tree` | `Profile` — the same reading, as JSON | the same |
| `tree-with-err` | `Vec<Recorded>` — every call that began, each under `"ended"` or `"unended"` | nothing the reading can express |
| `raw` | every message the run heard, with the shell that sent it, one JSON object per line | nothing |

`human` and `tree` are one reading in two forms, and the only one that can
refuse, since every entry of it claims a duration and a call that never ended
has none. What the four share, a message the recording refuses or a transport
that broke, fails in all of them.

Each JSON shape is a derived `Serialize` over the type in its row, with no
`flatten`, no tag attribute and nothing that buffers on the way back in. The
JSON is the struct, so `Span::inclusive` and `Span::exclusive` are absent from
it; `human` is the same reading through `Profile`'s `Display`, which prints
both.

Bashprof prints one line on stderr per source path a frame names and no longer
has, which [bash-interop: stack](https://bashmgmt.github.io/bash-interop/stack.html#where-a-source-path-lands) explains. A
run whose reading is meant to resolve those paths names a workspace it keeps.

## The chain

Each type wraps the one before it and adds what its own message carried.
Nothing is restated, so no two of them can disagree.

```rust
/// When one message was written and when it was read.
pub struct Stamp { sent_at, heard_at }

/// A call that began: everything its BEGIN reported.
pub struct Call {
    pub id: Id,
    pub label: String,
    pub argv: Vec<String>,      // the command being measured
    pub stack: Stack,
    pub shell: Arc<Shell>,      // the shell the walk above was taken in
    pub stamp: Stamp,
}

/// A call that also ended: what its END carried back.
pub struct Complete { pub call: Call, pub ended_at: Micros, pub status: u8 }

/// A measurement: a completed call, and the calls made inside it.
pub struct Span { pub complete: Complete, pub children: Vec<Span> }

/// The tree that can hold either.
pub enum Record { Unended(Call), Ended(Complete) }
pub struct Recorded { pub record: Record, pub children: Arc<[Recorded]> }
```

The END carries the id, a timestamp and a status, with no walk of its own,
because it is the same shell in the same place. `status` is what the measured
command returned.

### The subject owns both streams

Nothing of bashprof's reaches stdout or stderr but its own failures, so a
profiled run pipes exactly as an unprofiled one does. `--into` is the only way
out, and `bashcap run --into` follows the same rule.

The file is truncated before the subject starts, so an unwritable path is known
straight away and a run that reads as nothing leaves nothing earlier standing in
for its reading.

### The exit code

Wherever the subject failed, its own exit code is passed through, so a
profiled script is indistinguishable from an unprofiled one. Where the subject
succeeded and bashprof could not write what was asked for, the failure belongs
to bashprof and so does the status: 1, with the reason on stderr.

The text renderings that `Profile` and `Recorded::render` produce stay in the
library, for a caller assembling a report of its own.

A call site makes bashprof a dependency of the script that says it:
outside a session the word is a missing command, loudly. A script that
must also run without the tool defines the word itself —

```bash
declare -F BASHPROF_TIMETHIS >/dev/null || BASHPROF_TIMETHIS() { shift; "$@"; }
```

— one line of its own, unshipped.

## The word, and the frame the subject reads

One function carries the call-site choreography, and its frame is where
everything the measured call reads is declared. The word, whole, from
`src/words.bash`:

```bash
BASHPROF_TIMETHIS() {
    declare __BP_label="${1-}"
    shift || return 125

    declare __BP_id=
    __bp_begin "$@" || return $?

    declare __BP_inside="$__BP_id"
    declare -- __BASHPROF_STACK_SHIFT=

    "$@"
    declare __BP_rc=$?

    declare -- BC_SAY__ARG_LABEL=BASHPROF
    BC_SAY TIMETHIS END id "$__BP_id" status "$__BP_rc" || return $?

    return "$__BP_rc"
}
```

Three declarations in that frame do the work. `__BP_id` is declared
empty and filled by `__bp_begin`, so the END can name the same call.
`__BP_inside` takes this call's name only after the BEGIN went out, the BEGIN
having read the name of the enclosing call, which is what places this one in
the tree.
`__BASHPROF_STACK_SHIFT` is reset beside it, covered in the next section. All
three reach everything inside `"$@"`, because a caller's locals are the
callee's environment ([bash-interop: scoping](https://bashmgmt.github.io/bash-interop/scoping.html)).

The BEGIN itself is assembled in a frame of its own — the rest of the
same file:

```bash
__bp_begin() {
    declare IFS=' '

    declare -a __BP_stack=()
    __bc_stack __BP_stack $(( 3 + ${__BASHPROF_STACK_SHIFT:-0} ))

    __BP_made=$(( ${__BP_made:-0} + 1 ))
    __BP_id="$BASHPID.$__BP_made"

    declare -- BC_SAY__ARG_LABEL=BASHPROF
    BC_SAY TIMETHIS BEGIN id "$__BP_id" inside "${__BP_inside-}" \
        label "$__BP_label" argv "(${*@Q})" "${__BP_stack[@]}"
}
```

The separate frame is what makes the `IFS` handling work. Every join in it is
`[*]@Q` under a `declare IFS=' '`, and that binding has to die before `"$@"`
runs, so the subject has its `IFS` back, unset included, before anything measured
runs. The 3 counts `__bc_stack`'s frame, `__bp_begin`'s and the word's, so the
walk points at the subject, and a wrapper's declared shift adds to it. What a
stack of function layers would cost instead is measured in
[bash-interop: measurements](https://bashmgmt.github.io/bash-interop/measurements.html#what-a-function-layer-costs-an-instrument).

## `$__BASHPROF_STACK_SHIFT`

Code that wrapped the public word in a word of its own needs the walk to reach
past that too. It says so in the frame it is wrapping from, so the value dies
with that frame:

```bash
measure_step() {
    declare __BASHPROF_STACK_SHIFT=1
    BASHPROF_TIMETHIS "$@"
}
```

It is read through `:-0` because an unset name inside `(( ))` is an error
under `set -u` while an empty one is zero, so a subject that never heard of it
pays one parameter expansion.

The word clears it, in the same breath as handing the name down:

```bash
declare __BP_inside="$__BP_id"
declare -- __BASHPROF_STACK_SHIFT=
```

A shift is for reaching this call site. Measured calls made inside this one
have their own, and would otherwise inherit a shift meant for another.

Unmeasured code between two measured calls is transparent to all of this: it
neither sets nor shadows either name, so a call made ten frames below the last
measured one still names that one, and still reports its own call site.

## Why the name is `$BASHPID` and a count

Two calls under one name would close each other's spans and claim each other's
children, so the name has to be unique across a run's whole process tree. The
two halves cover the two ways it could fail.

`$BASHPID` differs in every process, by definition. `__BP_made`, which no
frame declares, is one count per shell spanning every call that shell makes; a fork
inherits the count and advances its own copy, under its own pid, so the two
never meet.

`$RANDOM` cannot do either job, and drawing more of it does not help. The
failure mode is determinism after fork rather than a birthday collision: a
subshell inherits the generator's state, so two forks made from one point draw
the same sequence, and `${RANDOM}${RANDOM}${RANDOM}` appends the same digits in
both. Bash 5 reseeds in a subshell, measured on 5.3.9, but the manual documents
only that seeding with the same constant value produces the same sequence, and
bash 4 does not reseed.

What remains is pid reuse inside one run, which would take the OS cycling the
whole pid space in the seconds a profiled run lasts. It is detected rather than
guarded against, since a second call under one name is refused where the names
are read.

## The reading

| pass | | |
|---|---|---|
| `recording` | messages → flat records, each with the name it was told encloses it | one pass, one map from name to call |
| `nesting` | those → a forest, the names spent | one index, then `Treeish` + `vec_fold` |
| `profile` | that forest → timings | one `vec_fold` |

The reaction is the shipped `Vec<Message>`, and bashprof's rig is `bash` and
`joined` alone. `recorded()` takes `&[Said]`, which is what
[bash-interop: rigs](https://bashmgmt.github.io/bash-interop/rigs.html#what-a-run-hands-back) makes of the per-shell
foldings: a message and the shell that sent it, since a walk cannot be read
without one. Nothing is paired by position and nothing depends on the order
messages arrived in.

`recorded()` is the whole reading and the only entry point, and what passes
between the passes stays private.

The enclosing name does not reach the tree. `nest` reads it, and where a node
sits is what the tree then holds; a node carrying the name as well would be one
fact with two sources, free to disagree with the shape it was used to build. It
rides on `recording::Read` and stops there, so `Call` holds the call rather
than its wiring.

## What the reading owes

Three lookups, each of them the instrument's fault if it fails:

| | |
|---|---|
| a name is given once | a second BEGIN under one name is set aside |
| a name is ended once | a second END for one name is set aside |
| every name pointed at was given | a call made inside a name that never began falls out of the tree |

Nothing else is decided in the placement, so nothing else can go wrong there.
There is no ambiguity to report, because the shell said where the call belongs.

A message set aside does not end the reading. The pass carries on and collects
what it could not read, since what the other messages said is no less true. A
call whose enclosing one was set aside is then unreachable from any root, and
nesting drops it. The tree being shorter than what was read is the record of
that, held in the forest's own shape rather than in a count kept alongside
it.

`recorded()` yields `Result<Vec<Recorded>, Unread>`, and `Unread` carries the
forest. It takes the same shape as `Unfinished` below and reports something
different: `Unread` means the instrument or the wire faulted, `Unfinished` that
a shell died inside a call it had begun.

## The stack a node carries

The shell says where a call belongs, so nothing places a call by its frames.
Every node carries them anyway, because a stack is not the tree and cannot be
read off it: an unmeasured function between two measured calls is a frame and
not a node.

`Call` and `Span` each hold one [bash-interop: stack](https://bashmgmt.github.io/bash-interop/stack.html), from the call site
outward, with one frame of the instrument's per enclosing measurement:

```
inner   mid@build.bash:2  between@build.bash:3  BASHPROF_TIMETHIS  main@build.bash:5
```

`between` is on that walk and nowhere in the tree. `Stack::top` is the call
site, which tells two calls made from one function apart and is what `human`
prints. At one frame per level a walk 100 deep is a 17 kB payload in five
frames;
[bash-interop: measurements](https://bashmgmt.github.io/bash-interop/measurements.html#what-a-function-layer-costs-an-instrument)
has what the same instrument costs when its layers are functions instead.

## Time a span had to itself

A span's children do not partition its window. Concurrent forks overlap each
other, and a backgrounded one can outlive the call that made it, so summing
their durations and subtracting would count the overlap twice and count the
part past the window at all, claiming more time than the span has — an unsigned
subtraction that goes negative.

The children's windows are therefore clipped to the parent's and merged, and a
span's own time is what none of them covered. For sequential children that
comes to the plain sum; for concurrent ones it is what stays true.

## What is an error, and whose

`Profile::of` yields `Result<Profile, Unfinished>`. A subtree reads as a
measurement when its call ended and every call inside it did, which is a
traverse, so nothing tracks whether something went wrong. A call that ended
around one that did not knows its own duration but cannot account for it, and
is not a measurement either.

`Unfinished` carries the measurements that did complete, which are no less true
for the run having ended badly. A test bails on it and a tool reporting what it
has need not, and the tool makes that choice twice over: `--output
tree-with-err` reports what a partial reading holds, while `--output human` and
`--output tree` refuse, every entry of those claiming a duration.

## See also

- [bash-interop: scoping](https://bashmgmt.github.io/bash-interop/scoping.html) — the frame `__BP_inside` is declared in, and why
- [bash-interop: stack](https://bashmgmt.github.io/bash-interop/stack.html) — the frame walk both instruments share
- [bash-interop: wire](https://bashmgmt.github.io/bash-interop/wire.html#what-a-line-is) — the provenance every message carries already
- [bash-interop: rigs](https://bashmgmt.github.io/bash-interop/rigs.html) — the reaction this one keeps
