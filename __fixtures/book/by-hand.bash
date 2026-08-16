#!/usr/bin/env bash
# Started as:  bashprof run --reach by-hand --into build.times -- bash by-hand.bash
set -euo pipefail
declare -- workspace="${BASHPROF_SESSION:?the workspace, from the tool}"

# fetch-deps.bash is an ordinary helper of this build — not part of the
# protocol. Like every shell in the tree it wakes up with the words
# defined; nobody initiates in it, so it stays outside the session: it
# runs exactly as it would unwrapped, and nothing it does is heard.
bash "${BASH_SOURCE[0]%/*}/fetch-deps.bash"

# From here on, THIS shell is part of the run.
BASHPROF_INIT "$workspace"

build() { sleep 0.1; }
BASHPROF_TIMETHIS build build
