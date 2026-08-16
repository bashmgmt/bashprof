# bashprof

Time a tree of calls in a bash program, wherever the program wraps one in
`BASHPROF_TIMETHIS label command…`. Nothing is timed in bash and nothing is
inferred: every message carries the sending shell's clock and the name of the
call it belongs to, so the tree travels on the wire.

```
bashprof run   [--reach bash-env|by-hand] --into build.times [--output human|tree|tree-with-err|raw] -- make test
bashprof serve --at session.d --into build.times   # started BY a script: BC_START, BC_UP, BC_ATTACH
```

Built on [`bash-interop`](../bash-interop); the word a client vendors is
`assets/bashprof.bash`, and the reference is `KB/bashprof.md`.
`__fixtures/joined/build.bash` is the whole client story in one file.
