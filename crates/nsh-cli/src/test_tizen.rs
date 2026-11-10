//! `nsh test --target tizen`: the apps tests open run in the development
//! runtime on a Samsung TV or the Tizen emulator (the only device `sdb
//! devices` lists), installed as `nsh run --target tizen` installs it
//! (`tizen::run_dev`'s package, signed with `nsh.json` `tizen.signing`).
//! Each `open` builds the entry and serves it in a dev server's test session
//! of its own, maps the runtime's dev port to it with `sdb reverse` and
//! launches the runtime, whose test agent (GPUI's, `nsh-platform-gpui`'s
//! `agent.rs`) takes the test's steps; the runtime is stopped when the test
//! closes the app. The runtime's dev port is fixed, so the apps run one at
//! a time.

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

use crate::tizen::{DEV_PORT, Manifest, Sdb};

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

/// Starts the apps tests open on the device, installing the development
/// runtime there first.
pub(crate) fn starter(work: &Path) -> anyhow::Result<StartHost> {
    // `work` is the app's `.nsh/test`: the package goes beside `nsh run`'s.
    let nsh = work.parent().unwrap_or(work);
    let app = nsh.parent().unwrap_or(Path::new("."));
    let config = crate::package::read_config(&app.join("App.tsx"))?;
    let sdb = Arc::new(crate::tizen::install_dev(
        &nsh.join("tizen"),
        config.tizen.signing.as_deref(),
        None,
    )?);
    Ok(Arc::new(move |entry: &Path, options: &HostOptions| {
        start(&sdb, entry, options).map(|h| Box::new(h) as Box<dyn Host>)
    }))
}

/// The `sdb` commands that start the runtime on a test's dev server at
/// `port`: what an earlier app left running stopped, the dev port mapped,
/// the runtime launched.
pub(crate) fn open_sequence(sdb: &Sdb, port: u16) -> Vec<Vec<String>> {
    let app = Manifest::dev().app;
    vec![
        sdb.stop(&app),
        sdb.reverse(DEV_PORT, port),
        sdb.launch(&app),
    ]
}

/// The `sdb` commands that end a test's app: the runtime stopped, the
/// mapping removed.
pub(crate) fn close_sequence(sdb: &Sdb) -> Vec<Vec<String>> {
    vec![sdb.stop(&Manifest::dev().app), sdb.remove_reverse(DEV_PORT)]
}

/// The development runtime running one app for a test.
struct DeviceHost {
    server: DevServer,
    sdb: Arc<Sdb>,
    _turn: Turn,
}

impl Host for DeviceHost {
    fn step(&self, step: TestStep, timeout: Duration) -> Result<serde_json::Value, String> {
        self.server.step(step, timeout)
    }
}

impl Drop for DeviceHost {
    fn drop(&mut self) {
        for command in close_sequence(&self.sdb) {
            let _ = self.sdb.run(&command);
        }
    }
}

/// Builds `entry`, serves it in a test session and starts the runtime on it.
fn start(sdb: &Arc<Sdb>, entry: &Path, options: &HostOptions) -> Result<DeviceHost, String> {
    let turn = Turn::take();
    let server = crate::test_host::serve(entry, options)?;
    let host = DeviceHost {
        server,
        sdb: sdb.clone(),
        _turn: turn,
    };
    for command in open_sequence(sdb, host.server.local_addr().port()) {
        sdb.run(&command).map_err(|e| format!("{e:#}"))?;
    }
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn device(secure: bool) -> Sdb {
        Sdb {
            program: PathBuf::from("sdb"),
            serial: "192.168.1.20:26101".into(),
            arch: crate::tizen::Arch::Armv7l,
            secure,
        }
    }

    #[test]
    fn a_test_maps_the_dev_port_to_its_server_and_launches_the_runtime() {
        let sdb = device(true);
        let serial = ["-s", "192.168.1.20:26101"];
        let line = |rest: &[&str]| -> Vec<String> {
            serial.iter().chain(rest).map(|s| (*s).to_owned()).collect()
        };
        assert_eq!(
            open_sequence(&sdb, 50123),
            [
                line(&["shell", "0", "kill", "NshDevTizn.NshDev"]),
                line(&["reverse", "tcp:8790", "tcp:50123"]),
                line(&["shell", "0", "execute", "NshDevTizn.NshDev"]),
            ]
        );
        assert_eq!(
            close_sequence(&sdb),
            [
                line(&["shell", "0", "kill", "NshDevTizn.NshDev"]),
                line(&["reverse", "--remove", "tcp:8790"]),
            ]
        );
        // The emulator has a shell: `app_launcher`.
        assert_eq!(
            open_sequence(&device(false), 1)[2],
            line(&["shell", "app_launcher", "-s", "NshDevTizn.NshDev"])
        );
    }
}
