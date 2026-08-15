use super::*;

#[test]
fn a_call_carries_the_whole_stack_it_was_made_on() {
    let recorded = profiled(TREE).0;
    let c = calls(&recorded).into_iter().find(|call| call.label == "c").expect("c was measured");

    assert_eq!(c.stack.top().site.to_string(), "f__B", "where the call was made");
    assert_eq!(
        c.stack.below().iter().map(|frame| frame.site.to_string()).collect::<Vec<_>>(),
        ["BASHPROF_TIMETHIS", "f__A", "BASHPROF_TIMETHIS", "main"],
        "and everything above it"
    );
}

#[test]
fn a_wrapper_can_move_the_walk_past_itself() {
    let recorded = profiled(
        r#"
        f__leaf() { :; }

        f__measured() {
            local __BASHPROF_STACK_SHIFT=1
            BASHPROF_TIMETHIS "$@"
        }

        f__A() { f__measured leaf f__leaf; }

        BASHPROF_TIMETHIS a f__A
        "#,
    )
    .0;

    let profile = Profile::of(&recorded).expect("every call ended");
    let a = &profile.roots[0];

    assert_eq!(at(a, &["leaf"]).complete.call.stack.top().site.to_string(), "f__A", "the subject's site, not the wrapper's");
    assert_eq!(a.complete.call.stack.top().site.to_string(), "main", "and the unwrapped call is unaffected\n{profile}");
}

#[test]
fn a_span_says_where_its_call_was_made() {
    let recorded = profiled(TREE).0;
    let profile = Profile::of(&recorded).expect("a complete profile");
    let a = &profile.roots[0];

    assert_eq!(a.complete.call.stack.top().site.to_string(), "main", "the outermost call is in the script's own body");
    assert_eq!(at(a, &["b"]).complete.call.stack.top().site.to_string(), "f__A");
    assert_eq!(at(a, &["e"]).complete.call.stack.top().site.to_string(), "f__A");
    assert_eq!(at(a, &["b", "c"]).complete.call.stack.top().site.to_string(), "f__B");
    assert_eq!(at(a, &["b", "d"]).complete.call.stack.top().site.to_string(), "f__B");
    assert_eq!(at(a, &["e", "f"]).complete.call.stack.top().site.to_string(), "f__E");

    assert_ne!(at(a, &["b", "c"]).complete.call.stack.top().lineno, at(a, &["b", "d"]).complete.call.stack.top().lineno);
}
