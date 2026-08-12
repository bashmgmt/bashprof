# Time the call this wraps, and everything measured inside it.
#
#     BASHPROF_TIME_CPS <label> <command> [args…]
#
# Nothing is timed here: the wire stamps every message with the sending
# shell's $EPOCHREALTIME, so a span is the interval between two of them.
# Nothing is inferred either: a call is given a name, hands that name to
# everything it runs, and reports the name it was handed.
#
# The three layers are aliases rather than functions. A function would be a
# frame: one the walk has to skip, one every call measured below it carries in
# its own payload, and one more call per measurement. Each declares in the
# frame of the word the subject called, which is where the rest of that word
# and everything it runs will read it, and what a fork inherits.

# `__BP_stack`: the call site's walk.
#
# 2 is __bc_stack's own frame and the frame this expands into. It holds
# wherever this is used, as long as it is used in the body of the word the
# subject calls.
#
# $__BASHPROF_STACK_SHIFT adds to it, for a caller that wrapped that word in
# one of its own:
#
#     measure_step() {
#         local __BASHPROF_STACK_SHIFT=1
#         BASHPROF_TIME_CPS "$@"
#     }
#
# Read through `:-0` because an unset name inside `(( ))` is an error under
# `set -u`, and an empty one is zero.
alias __BASHPROF_TAKE_STACK='
    local -a __BP_stack=()
    __bc_stack __BP_stack $(( 2 + ${__BASHPROF_STACK_SHIFT:-0} ))'

# `__BP_id`: this call's name, unique across the run's process tree.
#
# $BASHPID differs in every process; the count keeps two calls in one shell
# apart. __BP_made is not local — one count per shell, which a fork inherits
# and then advances its own copy of, under its own pid.
alias __BASHPROF_TAKE_NAME='
    __BP_made=$(( __BP_made + 1 ))
    local __BP_id="$BASHPID.$__BP_made"'

# `__BP_inside`: what calls made inside this one report as their own parent.
#
# Declared after the BEGIN is sent, which is what makes that BEGIN name the
# caller's call rather than this one's. A shift a caller asked for was for
# reaching this call site, not the ones inside it, so it stops here as well.
alias __BASHPROF_HAND_ON='
    local __BP_inside="$__BP_id"
    declare -- __BASHPROF_STACK_SHIFT='

# The measured call is run unguarded: a `||` list would suppress errexit for
# everything it reaches, so a measured function would run past its own first
# failure and the run's status would change. Under `set -e` a failure exits at
# `"$@"`, no END is sent, and the call stays open.
#
# `$?` is read as the first command after the call, which is the only place it
# survives, and returned after the END has clobbered it.
BASHPROF_TIME_CPS() {
    local __BP_label="${1-}"
    shift || __BC_THROW

    __BASHPROF_TAKE_STACK
    __BASHPROF_TAKE_NAME

    BC_INSTR say TIME_CPS BEGIN id "$__BP_id" inside "${__BP_inside-}" \
        label "$__BP_label" "${__BP_stack[@]}" || __BC_BAIL

    __BASHPROF_HAND_ON

    "$@"
    local __BP_rc=$?

    BC_INSTR say TIME_CPS END id "$__BP_id" || __BC_BAIL

    return "$__BP_rc"
}
