#!/usr/bin/env bash
# A build script that measures itself, with or without anything listening.
#
#     build.bash                                     # just build
#     build.bash bashprof serve --into build.times   # build, and keep the timings
#
# A shipped client vendors a copy of the words file below; this fixture,
# living beside the tool, sources the asset itself.
set -euo pipefail

__root="${BASH_SOURCE[0]%/*}/../.."

# The word, and empty hooks for when nothing is there to measure. Loading
# replaces them with the ones that do — which is why the guard comes first
# and the load after it.
source "$__root/assets/bashprof.bash"
declare -F __bp_begin >/dev/null || { __bp_begin() { :; }; __bp_end() { :; }; }

if (( $# > 0 )); then
    declare -- workspace="$(mktemp -d)"
    coproc SERVER { "$@" --at "$workspace"; }
    until [[ -p "$workspace/join" ]]; do sleep 0.01; done
    source "$workspace/prelude.bash"
    source "$workspace/rig.bash"
    BASHPROF_INIT "$workspace"
fi

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
if (( $# > 0 )); then
    declare -- handle="${SERVER[1]}"
    exec {handle}>&-
    wait "$SERVER_PID"
    rm -rf "$workspace"
fi
