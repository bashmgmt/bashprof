//! What a client ships: the word `BASHPROF_TIMETHIS`, and the guard that
//! decides whether it does anything.

use std::process::Command;

use crate::bash::rig::{heard, Driving, ExitStatus};
use crate::bash;
use crate::bashprof::{recorded, BashProf, Profile, WORDS};
use crate::tests::scripts::{bash, Scripts};

/// The one line a client writes. It names the hooks rather than the word, so a
/// client cannot displace the real one whichever order the two arrive in.
const GUARD: &str =
    "declare -F __bp_begin >/dev/null || { __bp_begin() { :; }; __bp_end() { :; }; }";

/// A script whose call sites are guarded the way a shipped one's would be.
/// What it sources is the file the tool injects, byte for byte.
///
/// `set -euo pipefail` because that is what a shipped script has, and because
/// it is the option that reaches furthest into the tool: every name the hooks
/// read has to be one they set, an unset one being an error rather than empty.
/// Under a driven run `BASH_ENV` is read before the script sets it, so it is
/// the hooks' own bodies that run under it — and a client that joins a session
/// of its own sets it before anything of the tool's is sourced at all.
fn vendoring() -> Scripts {
    Scripts::of(&[
        ("bashprof.bash", WORDS),
        (
            "build.bash",
            &format!(
                "set -euo pipefail\n\
                 source \"$(dirname \"${{BASH_SOURCE[0]}}\")/bashprof.bash\"\n{GUARD}\n\
                 step() {{ echo \"ran $1\"; }}\n\
                 BASHPROF_TIMETHIS build step target\n"
            ),
        ),
    ])
}

/// Without the tool the guard installs the empty hooks, so the call sites stay
/// where they are and the calls they wrap still happen.
#[test]
fn the_vendored_word_runs_an_instrumented_script_unprofiled() {
    let scripts = vendoring();
    let ran = Command::new("bash").arg(scripts.at("build.bash")).output().expect("bash");

    assert_eq!(String::from_utf8(ran.stdout).unwrap(), "ran target\n");
    assert_eq!(ran.status.code(), Some(0));
}

/// The same script under the tool. `BASH_ENV` runs before the script's first
/// line, so the client's `source` comes second and redefines the word with the
/// same bytes — and the guard, naming the hook rather than the word, finds the
/// real one and leaves it.
#[test]
fn the_guard_leaves_the_real_hooks_in_place_under_the_tool() {
    let scripts = vendoring();
    let ran = BashProf.run(&bash(scripts.at("build.bash"))).unwrap().whole().unwrap();

    assert_eq!(ran.subject, ExitStatus::Code(0));

    let forest = recorded(&heard(&ran.shells)).unwrap();
    let profile = Profile::of(&forest).expect("the call was measured");

    assert_eq!(profile.roots.len(), 1, "the client's copy did not displace the effect");
    assert_eq!(profile.roots[0].complete.call.label, "build");
    assert_eq!(profile.roots[0].complete.call.argv, ["step", "target"]);
}

/// The word is one file, shipped both ways, so a client's copy cannot drift
/// from the injected one. What makes that possible is that it names nothing
/// that only exists once the tool has been sourced.
#[test]
fn the_word_names_nothing_a_client_would_not_have() {
    for line in WORDS.lines().filter(|line| !line.trim_start().starts_with('#')) {
        for name in bash::INJECTED_NAMES {
            assert!(!line.contains(name), "{name} in a file a client vendors: {line}");
        }
    }
}
