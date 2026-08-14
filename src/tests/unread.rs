//! A message this instrument wrote and cannot read back.
//!
//! Both scripts say something the reading has to refuse, beside calls it does
//! not. Nothing here is reachable from the word a client writes — a shell says
//! it directly, which is the only way a fault in the instrument can be staged.

use super::*;

/// Everything the run recorded, and why the rest did not.
fn read(script: &str) -> Result<Vec<Recorded>, Unread> {
    let scripts = Scripts::of(&[("subject.bash", script)]);
    let ran = BashProf.run(&bash(scripts.at("subject.bash"))).unwrap().whole().unwrap();

    assert_eq!(ran.subject, ExitStatus::Code(0), "the subject itself is fine");

    recorded(&heard(&ran.shells))
}

/// A BEGIN with a field missing is set aside, and the calls around it are no
/// less true for it.
#[test]
fn a_message_that_will_not_read_leaves_the_rest_standing() {
    let unread = read(
        r#"
        BASHPROF_TIME_CPS before true
        BC_INSTR say TIME_CPS BEGIN id 1.99 inside "" label mangled
        BASHPROF_TIME_CPS after true
        "#,
    )
    .expect_err("the mangled BEGIN");

    let labels: Vec<&str> =
        unread.resolved.iter().map(|node| node.record.call().label.as_str()).collect();

    assert_eq!(labels, ["before", "after"], "{unread}");
    assert_eq!(unread.unreadable().len(), 1);
    assert!(unread.unreadable()[0].to_string().contains("argv"), "{unread}");
}

/// A call made inside one that never began is unreachable from any root, so
/// the tree drops it. That the tree is shorter than what was read is the only
/// record of it, and it is the forest's own shape.
#[test]
fn a_call_whose_enclosing_one_never_began_is_dropped_and_counted() {
    let unread = read(
        r#"
        GHOST() {
            local -a __w=()
            __bc_stack __w 2
            BC_INSTR say TIME_CPS BEGIN id 1.99 inside nobody label ghost argv "()" "${__w[@]}"
        }

        BASHPROF_TIME_CPS kept true
        GHOST
        "#,
    )
    .expect_err("the orphaned call");

    let labels: Vec<&str> =
        unread.resolved.iter().map(|node| node.record.call().label.as_str()).collect();

    assert_eq!(labels, ["kept"], "{unread}");
    assert_eq!(unread.unreadable().len(), 1);
    assert!(unread.unreadable()[0].to_string().contains("1 calls"), "{unread}");
}

/// A run with nothing wrong in it says so by reading whole, which is what
/// every other test in this module relies on.
#[test]
fn a_sound_run_reads_without_a_word() {
    assert!(read("BASHPROF_TIME_CPS only true\n").is_ok());
}
