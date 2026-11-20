//! A test file whose engine cannot start (the sets it runs with register
//! one component twice) fails with the reason, rather than leaving the run
//! waiting for results that never come.

use std::time::{Duration, Instant};

use nsh_core::{element::Registry, prelude::*};
use nsh_testing::{TestRun, run_file};

use super::{test_run, todo};

/// A component two sets register.
#[component]
fn Chip() -> impl IntoElement {
    view()
}

fn with_chip(specifier: &'static str) -> Registry {
    let mut set = Registry::ui_set(specifier);
    set.register::<Chip>();
    set
}

#[test]
fn a_test_file_whose_engine_cannot_start_fails_with_the_reason() {
    let run = TestRun {
        ui_sets: vec![with_chip("app/native"), with_chip("app/charts")],
        ..test_run(None)
    };
    let started = Instant::now();
    let error = run_file(&run, &todo().join("App.test.tsx"), &mut |_| {}).unwrap_err();
    assert!(
        error.starts_with("the app did not start: component `")
            && error.contains(
                "start_panic::Chip` is registered by both `app/native` and `app/charts`: register it in one set"
            ),
        "{error}"
    );
    assert!(started.elapsed() < Duration::from_secs(60));
}
