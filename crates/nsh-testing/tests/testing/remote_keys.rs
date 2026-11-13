//! `app.keyboard.remote`: a TV remote's keys by name, headless
//! (`remote/App.test.tsx`).

use std::path::Path;

use nsh_testing::{TestRun, compiler, run_file};

use super::test_run;

#[test]
fn a_test_presses_a_tv_remote_s_keys() {
    let run = TestRun {
        ui_sets: vec![nsh_base::registry()],
        compile: compiler(vec![nsh_base::registry()]),
        ..test_run(None)
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testing/remote");
    let results = run_file(&run, &dir.join("App.test.tsx"), &mut |_| {}).unwrap();
    let failed: Vec<_> = results.iter().filter(|r| r.failure.is_some()).collect();
    assert!(failed.is_empty(), "{failed:#?}");
    assert_eq!(results.len(), 5);
}
