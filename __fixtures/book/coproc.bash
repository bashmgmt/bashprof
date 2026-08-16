#!/usr/bin/env bash
# ANCHOR: script
# Owns the session: names the workspace, starts the server, probes, loads,
# initiates — and leaves by closing the handle coproc left it.
set -euo pipefail

declare -- workspace="$PWD/prof.d"   # an address is absolute — initiation refuses else
mkdir -p "$workspace"

coproc SERVER { bashprof serve --at "$workspace" --into build.times; }
until [[ -p "$workspace/join" ]]; do sleep 0.01; done   # up exactly while serving

source "$workspace/prelude.bash"    # the protocol's words
source "$workspace/rig.bash"        # the rig's
BASHPROF_INIT "$workspace"

build() { sleep 0.1; }
BASHPROF_TIMETHIS build build

declare -- handle="${SERVER[1]}"
exec {handle}>&-    # let go: what was held is the server's standard input
wait "$SERVER_PID"  # it sees the session out; its status is this script's
# ANCHOR_END: script
