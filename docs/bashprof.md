# A call tree that travels

`src/bashprof/`, `src/bin/bashprof.rs`

A worked rig that times a tree of CPS calls. The instrument measures nothing
and infers nothing: the wire stamps every message with the sending shell's
`$EPOCHREALTIME` and pid, and the instrument adds a walk, a name, and the name
it was handed — which is what makes the tree an observation rather than a
reconstruction.

## The tool

Two verbs, and they differ only in who started the shells. Both take the same
options, from the same `Reading` struct — the symmetry is the code, not a
convention:

```
bashprof run   [--reach bash-env|by-hand] --into FILE [--output …] -- <command…>
bashprof serve --at DIR --into FILE [--output …]
```

| | who starts the shells | how they are reached | its exit code |
|---|---|---|---|
| `run` | the tool, from the command line it was given | `BC_SESSION` in the environment always; `--reach bash-env` (the default) also `BASH_ENV`, so the whole process tree joins; `--reach by-hand` leaves it to the scripts | the subject's |
| `serve` | a bash script, which named and made the workspace (`--at`, required, existing) and started this process as a coprocess | its own choice — the workspace is the address; the join fifo in it says the session is up (`BC_UP`), and the script loads and initiates by the same dir (`BC_LOAD`, `BASHPROF_INIT`) | its own: 0, or 1 if the reading did not come out |

`serve` is the shipped half of what `assets/joining.bash` starts —
`BC_START bashprof serve --at "$PWD/prof.d" --into build.times`. Nothing about the reading changes
between the two, and `__fixtures/joined/build.bash` runs under both plus under
no server at all, which is the vendoring contract end to end
([tests/cli.rs](../../../tests/cli.rs)). `run --help` and `serve --help` end
with `JOINING`, every way a script joins.

```rust
pub struct BashProf;   // drive it: BashProf.run(argv, |at| …); or serve it
```

`--output` chooses **how far the run is read**, and in what:

| `--output` | the file holds | refuses |
|---|---|---|
| `human` (default) | the measured tree indented, one call per line | a run with a call the shell died inside |
| `tree` | `Profile` — the same reading, as JSON | the same |
| `tree-with-err` | `Vec<Recorded>` — every call that began, each under `"ended"` or `"unended"` | nothing the reading can express |
| `raw` | every message the run heard, with the shell that sent it, one JSON object per line | nothing |

`human` and `tree` are one reading in two hands, and the only one that can
refuse: every entry of it claims a duration, and a call that never ended has
none. What the four share — a message the recording refuses, a transport that
broke — fails in all of them.

Each JSON shape is a plain derived `Serialize` over the type in its row — no
`flatten`, no tag attribute, nothing that buffers on the way back in. The JSON
is the struct, so `Span::inclusive` and `Span::exclusive` are not in it; `human`
is the same reading through `Profile`'s `Display`, which does print both.

Bashprof also prints one line on stderr per source path a frame names and does
not have. Not a failure — see
`bash-interop/docs/stack.md#where-a-source-path-lands` — but a run whose reading is
meant to resolve names a workspace it keeps.

## The chain

Each type wraps the one before it and adds what its own message carried.
Nothing is restated, so nothing can disagree.

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

The END carries the id, a timestamp and a status and nothing else — no walk of
its own, because it is the same shell in the same place. `status` is what the
measured command returned, which the wrapper knew all along and used to say
nothing about.

### The subject owns both streams

Nothing of bashprof's is written to stdout or stderr but its own failures, so a
profiled run pipes exactly as an unprofiled one does. That is what `--into`
is for, and it is the only way out. `bashcap run --into` is the same rule.

The file is truncated before the subject starts, so an unwritable path is known
straight away and a run that reads as nothing leaves nothing earlier standing in
for its reading.

### The exit code

The **subject's own** wherever the subject failed, so a profiled script is
indistinguishable from an unprofiled one. Where the subject succeeded and
bashprof could not write what was asked for, the failure is bashprof's and so
is the status: 1, with the reason on stderr.

The text renderings that `Profile` and `Recorded::render` produce stay in the
library, for a caller assembling a report of its own.

Keeping a script's call sites runnable without the tool is the client's own,
and `assets/bashprof.bash` is what it vendors to do it — see
`bash-interop/docs/vendoring.md`.

## The layers are aliases

A call carries three declarations, and each is a layer of its own. They are
**aliases, not functions**: a function would be a frame — one the walk has to
skip, one every call measured below it carries in its own payload, and one more
call per measurement. An alias is the same text in the same frame, so the
layers separate for a reader and cost nothing. What that saves is measured in
`bash-interop/docs/measurements.md#what-a-function-layer-costs-an-instrument`.

| layer | declares | read by |
|---|---|---|
| `__BASHPROF_TAKE_STACK` | `__BP_stack` — the call site's walk | the BEGIN |
| `__BASHPROF_TAKE_NAME` | `__BP_id` — this call's name | the BEGIN, and the END |
| `__BASHPROF_HAND_ON` | `__BP_inside` — its own name, for what it runs | every call made inside it |

Every one of them declares in the frame of the word the subject called, which
is what puts it where the rest of that word and everything it runs will read
it — and what a fork inherits. See `bash-interop/docs/scoping.md`.

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
    BASHPROF_TIMETHIS "$@"
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
| `recording` | messages → flat records, each with the name it was told encloses it | one pass, one map from name to call |
| `nesting` | those → a forest, the names spent | one index, then `Treeish` + `vec_fold` |
| `profile` | that forest → timings | one `vec_fold` |

The reaction is `Vec<Message>`, the shipped one — bashprof adds nothing to it, and
its rig is `bash` and `joined` alone. `recorded()` takes `&[Said]`, which is
what `bash-interop/docs/rigs.md#what-a-run-hands-back` makes of the per-shell foldings: a
message and the shell that sent it, since a walk cannot be read without one.
Nothing is paired by position and nothing depends on the order they arrived in.

`recorded()` is the whole reading and the only entry point; what passes between
the passes is nobody else's. **The enclosing name does not reach the tree.** It
is what `nest` reads, and where a node sits is what the tree then holds — a
node carrying it as well would be one fact with two sources, free to disagree
with the shape it was used to build. So it rides on `recording::Read` and
stops there; `Call` is the call, not its wiring.

## What the reading owes

Three lookups, and every one of them the instrument's fault if it fails:

| | |
|---|---|
| a name is given once | a second BEGIN under one name is set aside |
| a name is ended once | a second END for one name is set aside |
| every name pointed at was given | a call made inside a name that never began falls out of the tree |

Nothing else can go wrong in the placement, because nothing else is decided
there. In particular there is no ambiguity to report: the shell said where the
call belongs.

A message set aside does not end the reading. The pass carries on and collects
what it could not read, because what the other messages said is no less true —
and a call whose enclosing one was set aside is then unreachable from any root,
so nesting drops it. That the tree is shorter than what was read is the only
record of that, and it is the forest's own shape rather than a count kept
alongside it.

`recorded()` yields `Result<Vec<Recorded>, Unread>`, and `Unread` carries the
forest. It is the same shape as `Unfinished` below and not the same news: this
one means the instrument or the wire faulted, that one means a shell died
inside a call it had begun.

## The stack a node carries

Nothing places a call by the frames — the shell says where it belongs — and
every node carries them anyway, because **a stack is not the tree and cannot be
read off it**: an unmeasured function between two measured calls is a frame and
not a node.

`Call` and `Span` each hold one `bash-interop/docs/stack.md`, from the call site
outward, with one frame of the instrument's per enclosing measurement:

```
inner   mid@build.bash:2  between@build.bash:3  BASHPROF_TIMETHIS  main@build.bash:5
```

`between` is on that walk and nowhere in the tree. `Stack::top` is the call
site, which is what tells two calls made from one function apart, and what
`human` prints. At one frame per level a walk 100 deep is a 17 kB payload in
five frames — see
`bash-interop/docs/measurements.md#what-a-function-layer-costs-an-instrument`
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
need not. That choice is made once, at the top — and the tool makes it twice
over, `--output tree-with-err` reporting what a partial reading holds while
`--output human` and `--output tree` refuse, every entry of those claiming a
duration.

## See also

- `bash-interop/docs/scoping.md` — the frame `__BP_inside` is declared in, and why
- `bash-interop/docs/stack.md` — the frame walk both instruments share
- `bash-interop/docs/wire.md#messages` — the provenance every message carries already
- `bash-interop/docs/rigs.md#what-a-reaction-is-for` — the reaction this one keeps
