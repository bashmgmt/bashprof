//! The bash bashprof injects into a subject's shells.

/// `BASHPROF_TIME_CPS` and the three layers it expands, in every shell.
pub(crate) const BASH: &str = include_str!("bashprof.bash");

/// The bash to put in a [`Startup`](crate::bash::rig::Startup), for any rig
/// that wants what bashprof measures.
///
/// The frame walk is [`bash::STACK`](crate::bash::STACK), which bashprof shares
/// with every other instrument that reports a stack.
pub fn instrument() -> String {
    format!("{}\n{BASH}", crate::bash::STACK)
}
