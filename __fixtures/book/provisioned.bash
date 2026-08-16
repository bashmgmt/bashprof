#!/usr/bin/env bash
# ANCHOR: script
# Started as:  bashprof run --into build.times -- bash provisioned.bash
#
# The provisioned bash_env.bash defined the words and said the join in
# every shell of this tree as it started. Nothing of the protocol appears
# here — this is the way in for programs that never heard of the session.
set -euo pipefail

build() { sleep 0.1; }
BASHPROF_TIMETHIS build build
# ANCHOR_END: script
