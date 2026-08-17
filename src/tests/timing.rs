use super::*;

#[tokio::test]
async fn a_spans_time_covers_its_own_work_and_everything_it_called() {
    let recorded = profiled(TREE).await.0;
    let profile = Profile::of(&recorded).expect("a complete profile");
    let a = &profile.roots[0];

    for (path, slept) in [
        (&["b", "c"][..], 30_000),
        (&["b", "d"][..], 40_000),
        (&["e", "f"][..], 50_000),
        (&["b"][..], 30_000 + 10_000 + 40_000),
        (&["e"][..], 10_000 + 50_000),
        (
            &[][..],
            20_000 + 80_000 + 20_000 + 60_000,
        ),
    ] {
        let span = at(a, path);
        assert!(
            span.complete.took() >= slept,
            "{} took {} µs, less than the {slept} µs it slept\n{profile}",
            span.complete.call.label,
            span.complete.took()
        );
    }

    assert!(
        (40_000..40_000 + SLACK).contains(&a.exclusive()),
        "a's own time is the two 20 ms pauses in f__A, got {} µs\n{profile}",
        a.exclusive()
    );

    let leaf = at(a, &["e", "f"]);
    assert_eq!(
        leaf.exclusive(),
        leaf.complete.took(),
        "nothing is measured inside f"
    );

    let total: u64 = a.all().iter().map(|span| span.exclusive()).sum();
    assert_eq!(
        total,
        a.complete.took(),
        "exclusive times partition the root's\n{profile}"
    );
}
