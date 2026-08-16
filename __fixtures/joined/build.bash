#!/usr/bin/env bash
# A build script that measures itself under a server it starts:
#
#     build.bash bashprof serve --into build.times
set -euo pipefail
: "${1:?the server command line}"

declare -- workspace="$(mktemp -d)"
coproc SERVER { "$@" --at "$workspace"; }
until [[ -p "$workspace/join" ]]; do sleep 0.01; done
source "$workspace/prelude.bash"
source "$workspace/rig.bash"
BASHPROF_INIT "$workspace"

compile() { sleep 0.01; BASHPROF_TIMETHIS link link; }
link()    { sleep 0.01; }
package() { sleep 0.01; }

build() {
    BASHPROF_TIMETHIS compile compile
    BASHPROF_TIMETHIS package package
}

BASHPROF_TIMETHIS build build
echo built

# `wait` is what says the reading is written: the server does its reading after
# the last shell has let go, and this is the client letting go. The workspace
# was this script's to name, so it is this script's to remove.
declare -- handle="${SERVER[1]}"
exec {handle}>&-
wait "$SERVER_PID"
rm -rf "$workspace"
