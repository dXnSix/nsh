//! `nsh test --target tizen`: the apps tests open run in the development
//! runtime on a Samsung TV or the Tizen emulator (`--device`, else the only
//! device `sdb devices` lists), installed as `nsh run --target tizen`
//! installs it (`tizen::run_dev`'s package, signed with `nsh.json`
//! `tizen.signing`). Each `open` builds the entry and serves it in a dev
//! server's test session of its own, and starts the runtime on it as `nsh
//! run` does (`tizen::open`: what ran before stopped, the dev port mapped
//! with `sdb reverse`, else `sdb forward` and the relay, the runtime
//! launched), whose test agent (GPUI's, `nsh-platform-gpui`'s `agent.rs`)
//! takes the test's steps; the runtime is stopped and the mapping removed
//! when the test closes the app. The runtime's dev port is fixed, so the
//! apps run one at a time.

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use nsh_dev::{DevServer, protocol::TestStep};
use nsh_testing::{Host, HostOptions, StartHost};

use crate::tizen::{Live, Opened, Sdb};

/// How long the runtime may take to start and connect: a TV starts .NET
/// from scratch.
const CONNECT: Duration = Duration::from_secs(120);
/// An app runs on the device: they share the runtime's dev port, so one
/// at a time ([`Turn`]).
static BUSY: AtomicBool = AtomicBool::new(false);

/// The device's turn of one app, waited for and given back when dropped.
struct Turn;

impl Turn {
    fn take() -> Turn {
        while BUSY
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        Turn
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

/// Starts the apps tests open on the device (`device`: its serial; by
/// default the only one), installing the development runtime there first.
pub(crate) fn starter(work: &Path, device: Option<&str>) -> anyhow::Result<StartHost> {
    // `work` is the app's `.nsh/test`: the package goes beside `nsh run`'s.
    let nsh = work.parent().unwrap_or(work);
    let app = nsh.parent().unwrap_or(Path::new("."));
    let config = crate::package::read_config(&app.join("App.tsx"))?;
    let sdb = Arc::new(crate::tizen::install_dev(
        &nsh.join("tizen"),
        config.tizen.signing.as_deref(),
        device,
    )?);
    Ok(Arc::new(move |entry: &Path, options: &HostOptions| {
        start(&sdb, entry, options).map(|h| Box::new(h) as Box<dyn Host>)
    }))
}

/// The development runtime running one app for a test.
struct DeviceHost {
    server: DevServer,
    sdb: Arc<Sdb>,
    /// What `tizen::open` mapped, removed when the test closes the app.
    opened: Opened,
    _turn: Turn,
}

impl Host for DeviceHost {
    fn step(&self, step: TestStep, timeout: Duration) -> Result<serde_json::Value, String> {
        self.server.step(step, timeout)
    }
}

impl Drop for DeviceHost {
    fn drop(&mut self) {
        for command in crate::tizen::close_sequence(&self.sdb, &self.opened) {
            let _ = self.sdb.run(&command);
        }
    }
}

/// Builds `entry`, serves it in a test session and starts the runtime on it.
fn start(sdb: &Arc<Sdb>, entry: &Path, options: &HostOptions) -> Result<DeviceHost, String> {
    let turn = Turn::take();
    let server = crate::test_host::serve(entry, options)?;
    let port = server.local_addr().port();
    let mut host = DeviceHost {
        server,
        sdb: sdb.clone(),
        opened: Opened::default(),
        _turn: turn,
    };
    crate::tizen::open(sdb, port, &mut Live { sdb, server: port }, &mut host.opened)
        .map_err(|e| format!("{e:#}"))?;
    let deadline = Instant::now() + CONNECT;
    while !host.server.has_tester() {
        if Instant::now() >= deadline {
            return Err(format!(
                "the development runtime did not connect in {} s",
                CONNECT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(host)
}
