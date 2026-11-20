//! An engine that panics as it starts (two UI sets registering one
//! component: `Registry::merged`) reports that the app did not start, with
//! the panic's message, and its thread ends: a host hears of it rather
//! than waiting for a first frame that never comes.

use std::{
    sync::{Mutex, mpsc},
    time::Duration,
};

use nsh_core::{ApproxTextEngine, MountBatch, element::Registry};
use nsh_engine::{AppError, AppErrorKind, Bundle, Engine, EngineConfig, MountSink};

use crate::test_ui;

/// Hears the errors with their parts.
struct Reports(Mutex<mpsc::Sender<AppError>>);

impl MountSink for Reports {
    fn submit(&self, _batch: MountBatch) {}

    fn report(&self, error: &AppError) {
        let _ = self.0.lock().unwrap().send(error.clone());
    }
}

#[test]
fn a_start_that_panics_is_the_apps_load_error() {
    // Another set registering a component the tests' set registers too.
    let mut other = Registry::ui_set("test/other");
    other.register::<test_ui::Card>();
    let (tx, rx) = mpsc::channel();
    let engine = Engine::start(
        EngineConfig {
            ui_sets: vec![test_ui::registry(), other],
            ..EngineConfig::new(Bundle::source("throw new Error('never runs');".to_owned()))
        },
        || ApproxTextEngine,
        Reports(Mutex::new(tx)),
    )
    .unwrap();
    let error = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(error.kind, AppErrorKind::Load);
    assert!(
        error
            .message
            .starts_with("the app did not start: component `")
            && error.message.contains(
                "test_ui::Card` is registered by both `test/ui` and `test/other`: register it in one set"
            ),
        "{error}"
    );
    // Nothing more: the app never ran, and the engine stops at once.
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    drop(engine);
}
