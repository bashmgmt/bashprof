use super::*;

#[test]
fn measurements_nest_the_way_the_calls_do() {
    let (recorded, status) = profiled(TREE);
    assert_eq!(status, ExitStatus::Code(0));
    println!("as recorded:\n{}\n", Recorded::render(&recorded));

    let profile = Profile::of(&recorded).expect("every call that began also ended");
    println!("as timings:\n{profile}");

    assert_eq!(profile.roots.len(), 1, "one outermost measurement");
    let a = &profile.roots[0];
    assert_eq!(a.complete.call.label, "a");

    let labels = |span: &Span| span.children.iter().map(|c| c.complete.call.label.clone()).collect::<Vec<_>>();
    assert_eq!(labels(a), ["b", "e"]);
    assert_eq!(labels(at(a, &["b"])), ["c", "d"]);
    assert_eq!(labels(at(a, &["e"])), ["f"]);
    assert!(labels(at(a, &["b", "c"])).is_empty());

    assert_eq!(a.all().len(), 6);
    assert!(a.all().iter().all(|span| span.complete.call.sent.pid == a.complete.call.sent.pid), "one shell produced all of it");
}

#[test]
fn a_call_measured_in_a_subshell_nests_where_it_was_made() {
    let (recorded, status) = profiled(
        r#"
        f__A() {
            BASHPROF_TIME_CPS plain true
            ( BASHPROF_TIME_CPS forked true )
        }
        BASHPROF_TIME_CPS a f__A
        "#,
    );

    assert_eq!(status, ExitStatus::Code(0));
    let profile = Profile::of(&recorded).expect("every call ended");

    assert_eq!(profile.roots.len(), 1, "one outermost measurement, not one per shell");
    let a = &profile.roots[0];

    let labels = a.children.iter().map(|span| span.complete.call.label.as_str()).collect::<Vec<_>>();
    assert_eq!(labels, ["plain", "forked"]);
    assert_ne!(at(a, &["forked"]).complete.call.sent.pid, a.complete.call.sent.pid, "and it did run in a shell of its own");
}

#[test]
fn concurrent_forks_of_one_line_keep_their_own_calls() {
    let (recorded, status) = profiled(
        r#"
        f__work() {
            sleep "$1"
            BASHPROF_TIME_CPS inner true
            sleep 0.1
        }

        f__A() {
            for delay in 0.05 0.01; do
                ( BASHPROF_TIME_CPS turn f__work "$delay" ) &
            done
            wait
        }

        BASHPROF_TIME_CPS a f__A
        "#,
    );

    assert_eq!(status, ExitStatus::Code(0));
    println!("as recorded:\n{}\n", Recorded::render(&recorded));

    let profile = Profile::of(&recorded).expect("every call ended");
    let a = &profile.roots[0];
    assert_eq!(a.children.len(), 2, "one measurement per fork\n{profile}");

    assert_ne!(a.children[0].complete.call.id, a.children[1].complete.call.id, "one line, two calls, two names");

    for turn in &a.children {
        assert_eq!(turn.complete.call.label, "turn");
        assert_eq!(turn.children.len(), 1, "the call made in its own shell\n{profile}");
        assert_eq!(turn.children[0].complete.call.sent.pid, turn.complete.call.sent.pid, "and no other's\n{profile}");
    }

    let together: u64 = a.children.iter().map(|span| span.complete.took()).sum();
    assert!(together > a.complete.took(), "the two ran at once, so their windows overlap\n{profile}");
    assert!(a.exclusive() < a.complete.took(), "and what neither covered is a's own\n{profile}");
}

#[test]
fn a_name_is_inherited_through_two_levels_of_forking() {
    let (recorded, status) = profiled(
        r#"
        f__A() {
            (
                BASHPROF_TIME_CPS middle true
                ( BASHPROF_TIME_CPS deep true )
            )
        }

        BASHPROF_TIME_CPS a f__A
        "#,
    );

    assert_eq!(status, ExitStatus::Code(0));
    let profile = Profile::of(&recorded).expect("every call ended");

    assert_eq!(profile.roots.len(), 1, "one outermost measurement\n{profile}");
    let a = &profile.roots[0];

    let labels = a.children.iter().map(|span| span.complete.call.label.as_str()).collect::<Vec<_>>();
    assert_eq!(labels, ["middle", "deep"], "both under a, neither under the other\n{profile}");

    let pids = [a.complete.call.sent.pid, at(a, &["middle"]).complete.call.sent.pid, at(a, &["deep"]).complete.call.sent.pid];
    assert_eq!(
        pids.iter().collect::<HashSet<_>>().len(),
        3,
        "three shells, so the name really crossed two forks\n{profile}"
    );
}

#[test]
fn every_measurement_has_a_name_of_its_own() {
    let recorded = profiled(TREE).0;
    let profile = Profile::of(&recorded).expect("a complete profile");
    let a = &profile.roots[0];

    let names: HashSet<&str> = a.all().iter().map(|span| span.complete.call.id.0.as_str()).collect();
    assert_eq!(names.len(), 6, "six calls, six names\n{profile}");
}
