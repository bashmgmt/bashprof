# The word bashprof gives a script:
#
#     BASHPROF_TIMETHIS <label> <command> [args…]
#
# Nothing is timed here: the wire
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
#         BASHPROF_TIMETHIS "$@"
#     }
#
# 125 is what the protocol's own words return when they could not do their job.
# Without the guard, a call with no arguments would shift nothing, run nothing
# and report success.
#
# The declarations below stand in this frame because what reads them runs in
# it: `"$@"` sees __BP_inside and the reset shift, and the END is sent after
# `"$@"` has returned. __BP_id is declared here and filled by __bp_begin.
#
# The measured call is run unguarded: a `||` list would suppress errexit for
# everything it reaches, so a measured function would run past its own first
# failure and the run's status would change. Under `set -e` a failure exits at
# `"$@"`, no END is sent, and the call stays open.
#
# `$?` is read as the first command after the call, which is the only place it
# survives, and returned after the END has clobbered it.
# ANCHOR: word
BASHPROF_TIMETHIS() {
    local __BP_label="${1-}"
    shift || return 125

    local __BP_id=
    __bp_begin "$@" || return $?

    local __BP_inside="$__BP_id"
    declare -- __BASHPROF_STACK_SHIFT=

    "$@"
    local __BP_rc=$?

    BC_INSTR BASHPROF say TIMETHIS END id "$__BP_id" status "$__BP_rc" || return $?

    return "$__BP_rc"
}
# ANCHOR_END: word

# The BEGIN, in a frame of its own so `IFS` is taken here and given back —
# including an `IFS` that was unset — before anything of the subject's runs.
# Every join below is `[*]@Q` and uses the caller's, and a subject with one
# of its own would corrupt them.
#
# 3 is `__bc_stack`'s own frame, this one, and the word's; the shift a caller
# asked for adds to it. Read through `:-0` because an unset name inside `(( ))`
# is an error under `set -u`, and an empty one is zero.
#
# `__BP_made` is not local — one count per shell, which a fork inherits and
# then advances its own copy of, under its own pid. $BASHPID differs in every
# process, so the two together name a call across the run's process tree. It
# is unset until the first measured call, and read through `:-0` for the same
# reason as the shift above.
#
# `__BP_inside` is not the word's yet: it resolves to the enclosing call's,
# which is what makes this BEGIN name the caller's call rather than its own.
# ANCHOR: begin
__bp_begin() {
    local IFS=' '

    local -a __BP_stack=()
    __bc_stack __BP_stack $(( 3 + ${__BASHPROF_STACK_SHIFT:-0} ))

    __BP_made=$(( ${__BP_made:-0} + 1 ))
    __BP_id="$BASHPID.$__BP_made"

    BC_INSTR BASHPROF say TIMETHIS BEGIN id "$__BP_id" inside "${__BP_inside-}" \
        label "$__BP_label" argv "(${*@Q})" "${__BP_stack[@]}"
}
# ANCHOR_END: begin
