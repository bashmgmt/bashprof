#!/usr/bin/env bash
# A build script that measures itself, with or without anything listening.
#
#     build.bash                                     # just build
#     build.bash bashprof serve --into build.times   # build, and keep the timings
#
# A shipped client vendors copies of the two files below; this one sources them
# where they live, so the test and the tool cannot drift apart.
set -euo pipefail

__root="${BASH_SOURCE[0]%/*}/../.."

# The word, and empty hooks for when nothing is there to measure. Joining
# replaces them with the ones that do — which is why the guard comes first and
# `BC_START` after it.
source "$__root/assets/bashprof.bash"
declare -F __bp_begin >/dev/null || { __bp_begin() { :; }; __bp_end() { :; }; }

if (( $# > 0 )); then
    source "$__root/assets/joining.bash"
    __workspace="$(mktemp -d)"
    BC_START "$@" --at "$__workspace"
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
    BC_LEAVE
    rm -rf "$__workspace"
fi
