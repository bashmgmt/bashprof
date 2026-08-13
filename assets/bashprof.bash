# The word bashprof gives a script. This file is what the tool injects and what
# a client vendors, so the word has one definition and only its effect exists
# twice:
#
#     source lib/bashprof.bash
#     declare -F __bp_begin >/dev/null || { __bp_begin() { :; }; __bp_end() { :; }; }
#
#     BASHPROF_TIME_CPS <label> <command> [args…]
#
# Nothing here names the protocol. Nothing is timed here either: the wire
# stamps every message with the sending shell's $EPOCHREALTIME, so a span is
# the interval between two of them. Nothing is inferred either — a call is
# given a name, hands that name to everything it runs, and reports the name it
# was handed.
#
# $__BASHPROF_STACK_SHIFT adds to the walk's depth, for a caller that wrapped
# this word in one of its own:
#
#     measure_step() {
#         local __BASHPROF_STACK_SHIFT=1
#         BASHPROF_TIME_CPS "$@"
#     }
#
# 125 is what the protocol's own words return when they could not do their job.
# Without the guard, a call with no arguments would shift nothing, run nothing
# and report success.
#
# The three declarations below stand in this frame because what reads them runs
# in it: `"$@"` sees __BP_inside and the reset shift, and the END is sent after
# `"$@"` has returned. __BP_id is declared here and filled by the hook, which
# is where the rest of the work happens — and what that takes for itself, bash
# gives back when it returns.
#
# The measured call is run unguarded: a `||` list would suppress errexit for
# everything it reaches, so a measured function would run past its own first
# failure and the run's status would change. Under `set -e` a failure exits at
# `"$@"`, no END is sent, and the call stays open.
#
# `$?` is read as the first command after the call, which is the only place it
# survives, and returned after the END has clobbered it.
BASHPROF_TIME_CPS() {
    local __BP_label="${1-}"
    shift || return 125

    local __BP_id=
    __bp_begin "$@" || return $?

    local __BP_inside="$__BP_id"
    declare -- __BASHPROF_STACK_SHIFT=

    "$@"
    local __BP_rc=$?

    __bp_end "$__BP_rc" || return $?

    return "$__BP_rc"
}
