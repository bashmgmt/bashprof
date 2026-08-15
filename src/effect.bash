# What a measured call does when the tool is there.
#
# The work happens in a frame of its own, so `IFS` is taken here and given back
# — including an `IFS` that was unset — before anything of the subject's runs.
# Every join below is `[*]@Q` and uses the caller's, and a subject with one of
# its own would corrupt them.
#
# 3 is `__bc_stack`'s own frame, this one, and the word's; the shift a caller
# asked for adds to it. Read through `:-0` because an unset name inside `(( ))`
# is an error under `set -u`, and an empty one is zero.
#
# `__BP_made` is not local — one count per shell, which a fork inherits and
# then advances its own copy of, under its own pid. $BASHPID differs in every
# process, so the two together name a call across the run's process tree. It is
# unset until the first measured call, and read through `:-0` for the same
# reason as the shift above.
#
# `__BP_id` is the word's local, declared empty there and filled here.
# `__BP_inside` is not the word's yet: it resolves to the enclosing call's,
# which is what makes this BEGIN name the caller's call rather than its own.
__bp_begin() {
    local IFS=' '

    local -a __BP_stack=()
    __bc_stack __BP_stack $(( 3 + ${__BASHPROF_STACK_SHIFT:-0} ))

    __BP_made=$(( ${__BP_made:-0} + 1 ))
    __BP_id="$BASHPID.$__BP_made"

    BC_INSTR BASHPROF say TIMETHIS BEGIN id "$__BP_id" inside "${__BP_inside-}" \
        label "$__BP_label" argv "(${*@Q})" "${__BP_stack[@]}"
}

__bp_end() {
    BC_INSTR BASHPROF say TIMETHIS END id "$__BP_id" status "$1"
}
