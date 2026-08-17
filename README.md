# bashprof

Time a tree of calls in a bash program.

Wrap what you want measured, giving it a label and the command to run:

```bash
build() {
    BASHPROF_TIMETHIS compile cc_all
    BASHPROF_TIMETHIS link    ld_all
}
BASHPROF_TIMETHIS build build
```

```
bashprof run --into build.times -- bash build.bash
```

```
build 76588 µs (621 µs of its own) at main@build.bash:6
  compile 53139 µs (53139 µs of its own) at build@build.bash:2
  link 22828 µs (22828 µs of its own) at build@build.bash:3
```

Each measurement carries the clock of the shell that sent it, the identity of
the call it belongs to, and the frame it was written from. The tree is
assembled from those, so calls made in a subshell nest under their parent, and
a call whose shell died before it ended is reported as unended rather than
given a duration. The figure in parentheses is the call's own time with its
children's subtracted.

`BASHPROF_TIMETHIS` is a shell function that does nothing when no session is
present, so a script carrying it runs the same way outside the profiler.

`--output` chooses how far the run is read: `human` for the tree above, `tree`
for the same as JSON, `tree-with-err` to include calls that never ended, and
`raw` for the messages themselves, one per line, before any of them are read
as calls.

## Reaching a session

`run` provisions `BASH_ENV`, and every non-interactive bash in the tree joins
as it starts. Under `--reach by-hand` the provisioned file only defines the
words, and a script joins where it says `BASHPROF_INIT "$BASHPROF_SESSION"`;
this is the one to use when the program starts shells whose startup you do not
control.

A script can also start the profiler itself as a coprocess and keep the
reading. `bashprof serve --help` prints that recipe, and
`__fixtures/joined/build.bash` is the whole of it in one file.

Built on [bash-interop](https://github.com/bashmgmt/bash-interop). Reference:
[`docs/`](docs/README.md).

Licensed under the MIT licence.
