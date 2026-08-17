use super::*;

#[tokio::test]
async fn a_call_the_shell_died_inside_is_an_error_carrying_the_rest() {
    let (recorded, status) = profiled(
        r#"
        set -e

        f__ok()   { :; }
        f__dies() { false; echo "RAN PAST ITS OWN FAILURE"; }

        BASHPROF_TIMETHIS ok f__ok
        BASHPROF_TIMETHIS doomed f__dies
        echo "REACHED THE END"
        "#,
    )
    .await;

    assert_eq!(
        status,
        ExitStatus::Code(1),
        "the subject's own status, not the wrapper's"
    );
    assert_eq!(
        unended(&recorded),
        ["doomed"],
        "the forest says so on its own"
    );

    let unfinished = Profile::of(&recorded).expect_err("the shell died inside a call");
    let resolved = &unfinished.resolved;

    assert_eq!(resolved.roots.len(), 1);
    assert_eq!(
        resolved.roots[0].complete.call.label, "ok",
        "no less true for the run ending badly"
    );
}

#[tokio::test]
async fn a_completed_call_survives_an_enclosing_one_that_did_not() {
    let recorded = profiled(NESTED).await.0;
    assert_eq!(unended(&recorded), ["outer"]);

    let unfinished = Profile::of(&recorded).expect_err("the outer call never ended");

    assert_eq!(
        unfinished
            .resolved
            .roots
            .iter()
            .map(|span| span.complete.call.label.as_str())
            .collect::<Vec<_>>(),
        ["inner"]
    );
}

#[tokio::test]
async fn the_error_renders_the_tree_it_was_recorded_as() {
    let recorded = profiled(NESTED).await.0;
    let shown = Profile::of(&recorded)
        .expect_err("the outer call never ended")
        .to_string();
    println!("{shown}");

    assert!(
        shown.contains("outer NEVER ENDED at main@"),
        "{shown}"
    );
    assert!(
        shown.contains("µs at f__outer@"),
        "the completed one, with its duration: {shown}"
    );
    assert!(
        !shown.contains("inner NEVER"),
        "{shown}"
    );
}
