//! What a client ships: the stub that stands in for `BASHPROF_TIME_CPS`, and
//! the guard that decides whether it is installed.

use std::process::Command;

use crate::bash::rig::{run, ExitStatus};
use crate::bashprof::{recorded, BashProf, Profile};
use crate::tests::scripts::{bash, Scripts};

/// bashprof ships the stub as an asset, so these read the same file a client
/// would copy.
const POLYFILL: &str = include_str!("../../../assets/bashprof_polyfill.bash");

const GUARD: &str = "declare -F BASHPROF_TIME_CPS >/dev/null || __define_bashprof_polyfill";

/// A script whose call sites are guarded the way a shipped one's would be.
fn vendoring() -> Scripts {
    Scripts::of(&[
        ("polyfill.bash", POLYFILL),
        (
            "build.bash",
            &format!(
                "source \"$(dirname \"${{BASH_SOURCE[0]}}\")/polyfill.bash\"\n{GUARD}\n\
                 step() {{ echo \"ran $1\"; }}\n\
                 BASHPROF_TIME_CPS build step target\n"
            ),
        ),
    ])
}

/// Without the tool the guard installs the stub, so the call sites stay where
/// they are and the calls they wrap still happen.
#[test]
fn the_vendored_stub_runs_an_instrumented_script_unprofiled() {
    let scripts = vendoring();
    let ran = Command::new("bash").arg(scripts.at("build.bash")).output().expect("bash");

    assert_eq!(String::from_utf8(ran.stdout).unwrap(), "ran target\n");
    assert_eq!(ran.status.code(), Some(0));
}

/// The same script under the tool. `BASH_ENV` defines the real word before the
/// script's first line, so the guard must find it and leave it alone — a stub
/// that installed itself unconditionally would measure nothing, silently.
#[test]
fn the_guard_leaves_the_real_word_in_place_under_the_tool() {
    let scripts = vendoring();
    let (heard, status) =
        run(&BashProf, &bash(scripts.at("build.bash"))).unwrap().whole().unwrap();

    assert_eq!(status, ExitStatus::Code(0));

    let profile = Profile::of(&recorded(&heard).unwrap()).expect("the call was measured");

    assert_eq!(profile.roots.len(), 1, "the stub did not displace the real word");
    assert_eq!(profile.roots[0].complete.call.label, "build");
    assert_eq!(profile.roots[0].complete.call.argv, ["step", "target"]);
}
