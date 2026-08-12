//! The bash bashprof ships: what it injects into a subject's shells, and the
//! stub a script vendors so its call sites stay safe to ship without it.

/// `BASHPROF_TIME_CPS` and the three layers it expands, in every shell.
pub(crate) const BASH: &str = include_str!("bashprof.bash");

/// The no-op stub a script vendors, so instrumented call sites stay safe to
/// ship. Under the tool the real definition is already in place and its `if`
/// is false.
pub const POLYFILL: &str = include_str!("polyfill.bash");

/// The bash to put in a [`Startup`](crate::bash::rig::Startup), for any rig
/// that wants what bashprof measures.
///
/// The frame walk is [`bash::STACK`](crate::bash::STACK), which bashprof shares
/// with every other instrument that reports a stack.
pub fn instrument() -> String {
    format!("{}\n{BASH}", crate::bash::STACK)
}
