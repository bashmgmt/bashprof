if ! declare -F BASHPROF_TIME_CPS >/dev/null; then
    BASHPROF_TIME_CPS() { shift || return 2; "$@"; }
fi
