use std::{
    cell::Cell,
    collections::HashSet,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use anyhow::Context as _;
use notify::{RecursiveMode, Watcher};
use nsh_dev::{
    ClientEvent, DevServer,
    protocol::{HostPlatform, WS_PATH},
};

use crate::{build, logger, ui};

/// File events arriving within this window are coalesced into one rebuild,
/// or into a restart when one of them changes the app's Rust code.
const DEBOUNCE: Duration = Duration::from_millis(30);

/// A running dev server plus the source watcher feeding it.
pub(crate) struct DevSession {
    entry: PathBuf,
    src_dir: PathBuf,
    /// Directories `nsh.json`'s `"watch"` skips: events inside them are
    /// not the app's.
    watch_ignored: Vec<PathBuf>,
    release: bool,
    server: DevServer,
    /// Whether other machines may connect.
    devices: Arc<Devices>,
    /// The modules of the last good development build: updates are
    /// computed against it.
    hot: std::cell::RefCell<Option<nsh_bundler::HotBundle>>,
    /// Files read by the last good build (imports may be outside the app).
    watch_files: std::cell::RefCell<Vec<PathBuf>>,
    /// The directories outside the app the last good build's
    /// `import.meta.glob` patterns search, with their patterns: a watcher
    /// of their own reports the files they match ([`glob_watcher`]).
    glob_dirs: Arc<Mutex<Vec<nsh_bundler::GlobDir>>>,
    /// Every `import.meta.glob` pattern the last good build searched, in
    /// the app or outside it: a JSON file one matches is one the app takes
    /// ([`uses_json`]).
    glob_patterns: std::cell::RefCell<Vec<nsh_bundler::GlobDir>>,
    /// The last build failed: which files it would read is not known.
    last_failed: Cell<bool>,
    /// Good builds so far: the first one is "built", the next "rebuilt".
    builds: Cell<u32>,
    /// A Rust app (`nsh.json` `"root"`): nothing to bundle, the hosts run
    /// its root; the session serves its assets, its web page and its logs.
    rust_root: bool,
    /// Active build context for asset filtering, aliases, and compile-time defines.
    context: nsh_core::element::BuildContext,
    /// Scratch directory for generated pruned manifests served during `nsh run`.
    context_assets_dir: PathBuf,
    /// The token `nsh test --attach` drives the session's hosts with, kept
    /// where that command finds it while the session runs.
    _attach: Option<crate::dev_port::AttachFile>,
}

/// Where a dev server listens.
#[derive(Clone, Copy)]
pub(crate) struct Listen {
    /// Its port, zero for one the OS allocates.
    pub(crate) port: u16,
    pub(crate) host: Ipv4Addr,
}

impl Listen {
    pub(crate) fn new(port: u16, network: bool) -> Self {
        let host = if network {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        Self { port, host }
    }
}

impl DevSession {
    /// Starts the server and publishes the first build (or its error).
    /// Hosts run the app with `limits` as an embedding app that sets them
    /// would (`--limits`), or with none, as an app on its own; the web page
    /// it serves runs it in `idiom` (`--idiom`, `nsh.json` `dev.idiom`), or
    /// in the one the browser measures.
    #[cfg(test)]
    pub(crate) fn start(
        entry: &Path,
        listen: Listen,
        release: bool,
        limits: Option<nsh_core::js::HostLimits>,
        idiom: Option<crate::run::DevIdiom>,
    ) -> anyhow::Result<DevSession> {
        Self::start_with_context(
            entry,
            listen,
            release,
            limits,
            idiom,
            nsh_core::element::BuildContext::new(),
        )
    }

    /// Starts the server with an explicit [`nsh_core::element::BuildContext`].
    pub(crate) fn start_with_context(
        entry: &Path,
        listen: Listen,
        release: bool,
        limits: Option<nsh_core::js::HostLimits>,
        idiom: Option<crate::run::DevIdiom>,
        context: nsh_core::element::BuildContext,
    ) -> anyhow::Result<DevSession> {
        let entry = dunce::canonicalize(entry)
            .with_context(|| format!("entry not found: {}", entry.display()))?;
        // The app's directory: the entry's, or `--app`'s for an entry kept
        // outside the app, whose own directory `watch_imports` watches.
        let src_dir = dunce::canonicalize(crate::entry::app_dir(&entry))?;
        let devices = Arc::<Devices>::default();
        // The app's directory: its `nsh.json`'s, which may be above the
        // entry (`"entry"`); its assets are named from it.
        let app_dir = nsh_bundler::app_dir::app_dir_of(&entry);
        let context_assets_dir = app_dir.join(".nsh").join("context-assets");
        // What its `"watch"` leaves out of the watcher.
        let watch_ignored = crate::package::read_config(&entry)
            .ok()
            .map(|config| ignored_dirs(&config.watch.ignore, &app_dir))
            .unwrap_or_default();
        let server = start_server(listen, &app_dir, devices.clone())?;
        let crate_assets: Vec<build::Asset> = crate::build::crate_assets().collect();
        let initial_served = build::resolve_assets_for_context(&crate_assets, &context)?;
        server.serve_files(materialize_dev_assets(
            &initial_served,
            &context_assets_dir,
        )?);
        server.install_commands(install_commands(&app_dir));
        server.set_limits(limits.map(|l| l.to_string()));
        // `nsh test --attach` drives the hosts that take its steps, from this
        // machine, with the token only this user reads.
        let attach = match crate::dev_port::AttachFile::new(server.local_addr().port()) {
            Ok(attach) => {
                server.set_attach_token(Some(attach.token().to_owned()));
                Some(attach)
            }
            Err(err) => {
                ui::warn(format!(
                    "nsh test --attach cannot drive this session: {err:#}"
                ));
                None
            }
        };
        // Browsers too, when there is a web runtime to serve (`nsh run
        // --target web` builds it in a checkout).
        let rust_root = crate::rust_app::is_root(&entry);
        if let Ok(Some(runtime)) = crate::runtime::web(crate::runtime::WebKind::Dev, false) {
            let (page, files) = crate::web::with_page_host(
                crate::web::dev_host(&runtime, rust_root)?,
                &entry,
                idiom,
            )?;
            server.serve_web(page, files);
        }
        let session = DevSession {
            entry,
            src_dir,
            watch_ignored,
            release,
            server,
            devices,
            hot: Default::default(),
            watch_files: Default::default(),
            glob_dirs: Default::default(),
            glob_patterns: Default::default(),
            last_failed: Cell::new(false),
            builds: Cell::new(0),
            rust_root,
            context,
            context_assets_dir,
            _attach: attach,
        };
        session.rebuild();
        // A Rust app's hosts that connect (the terminal's) run the root
        // they were built with: the server tells them so.
        if session.rust_root {
            session.server.publish_root();
        }
        Ok(session)
    }

    /// The port the dev server listens on.
    pub(crate) fn port(&self) -> u16 {
        self.server.local_addr().port()
    }

    /// WebSocket URL for hosts on this machine.
    pub(crate) fn local_url(&self) -> String {
        format!("ws://{}{WS_PATH}", self.local_endpoint())
    }

    /// WebSocket URL for a host reaching the server through `127.0.0.1`
    /// (such as an Android device over `adb reverse`).
    pub(crate) fn loopback_url(&self) -> String {
        format!("ws://{}:{}{WS_PATH}", Ipv4Addr::LOCALHOST, self.port())
    }

    /// The web host's page for browsers on this machine.
    pub(crate) fn web_url(&self) -> String {
        format!("http://{}/", self.local_endpoint())
    }

    fn local_endpoint(&self) -> SocketAddr {
        let addr = self.server.local_addr();
        if addr.ip().is_unspecified() {
            SocketAddr::from((Ipv4Addr::LOCALHOST, addr.port()))
        } else {
            addr
        }
    }

    /// The web host's page for a device on this network (a TV): at this
    /// machine's first LAN address, else (no network) at loopback.
    pub(crate) fn device_web_url(&self) -> String {
        let port = self.server.local_addr().port();
        match self.network_ips().first() {
            Some(ip) => format!("http://{ip}:{port}/"),
            None => self.web_url(),
        }
    }

    /// Serves the web host's page of a Tizen web app instead of the web's:
    /// with the TV shim, in the TV idiom (`tizen_web::dev_page`).
    pub(crate) fn serve_tv_page(&self) -> anyhow::Result<()> {
        let runtime =
            crate::runtime::web(crate::runtime::WebKind::Dev, false)?.context("no web runtime")?;
        let page = crate::tizen_web::dev_page(crate::web::dev_host(&runtime, self.rust_root)?);
        let (page, files) = crate::web::with_page_host(page, &self.entry, None)?;
        self.server.serve_web(page, files);
        Ok(())
    }

    /// This machine's addresses other machines reach the server on.
    fn network_ips(&self) -> Vec<Ipv4Addr> {
        match self.server.local_addr().ip() {
            std::net::IpAddr::V4(ip) if ip.is_unspecified() => lan_ips(),
            std::net::IpAddr::V4(ip) if !ip.is_loopback() => vec![ip],
            _ => Vec::new(),
        }
    }

    /// Prints where hosts can connect.
    pub(crate) fn print_urls(&self) {
        let port = self.server.local_addr().port();
        ui::note(format!("dev server {}", self.local_url()));
        let web = self.server.serves_web();
        if web {
            ui::detail(format!("browsers: {}", self.web_url()));
        }
        for ip in self.network_ips() {
            ui::detail(format!(
                "devices on this network: ws://{ip}:{port}{WS_PATH}"
            ));
            if web {
                ui::detail(format!("browsers on this network: http://{ip}:{port}/"));
            }
        }
    }

    /// Prints the page browsers open (`nsh run --target web`): the one
    /// address to open here, and dimmed, the same on this network (phones).
    pub(crate) fn print_web_url(&self) {
        let port = self.server.local_addr().port();
        ui::note(format!("open in a browser: {}", ui::link(&self.web_url())));
        if let Some(ip) = self.network_ips().first() {
            ui::detail(ui::dim(format!("on this network: http://{ip}:{port}/")));
        }
    }

    /// Rebuilds on source changes until `stop` returns true.
    pub(crate) fn watch(&self, stop: impl FnMut() -> anyhow::Result<bool>) -> anyhow::Result<()> {
        let result = self.watch_until(stop);
        // The session is over: its hosts going away (stopped by the
        // command, or with the server) is no news.
        self.devices.closing.store(true, Ordering::Relaxed);
        result
    }

    fn watch_until(&self, mut stop: impl FnMut() -> anyhow::Result<bool>) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel();
        let mut globbed = glob_watcher(self.glob_dirs.clone(), tx.clone())?;
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if let Ok(event) = event {
                    let _ = tx.send(event);
                }
            })?;
        watcher.watch(&self.src_dir, RecursiveMode::Recursive)?;
        ui::debug(format!("watching {}", ui::path(&self.src_dir)));
        // The app's Rust code: a change builds the app's hosts again.
        let rust = crate::rust_app::current();
        for dir in rust.iter().flat_map(|app| app.watched()) {
            if !dir.starts_with(&self.src_dir) {
                watcher.watch(dir, RecursiveMode::Recursive)?;
            }
        }
        let mut imported_dirs = HashSet::new();
        self.watch_imports(&mut watcher, &mut imported_dirs)?;
        let mut glob_dirs = HashSet::new();
        watch_globs(&mut globbed, &self.glob_dirs, &mut glob_dirs);
        loop {
            if ui::stop_requested() || stop()? {
                return Ok(());
            }
            let Ok(event) = rx.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            // inotify reports reads too (FSEvents does not): the build's
            // own would start the next one, forever.
            if matches!(event.kind, notify::EventKind::Access(_)) {
                continue;
            }
            if let Some(app) = &rust
                && event
                    .paths
                    .iter()
                    .any(|p| app.is_rust(p) && !app.in_watch_ignore(p))
            {
                return Err(crate::rust_app::Restart.into());
            }
            if !is_source(
                &event,
                &self.src_dir,
                &|path| self.takes_json(path),
                &self.watch_ignored,
            ) {
                continue;
            }
            // A Rust change saved with this one (a checkout, a formatter)
            // starts the session again, which builds the TSX too: built
            // now, it would check its imports against the old UI sets.
            let deadline = Instant::now() + DEBOUNCE;
            let is_rust = |p: &Path| {
                rust.as_ref()
                    .is_some_and(|app| app.is_rust(p) && !app.in_watch_ignore(p))
            };
            if rust_changed_until(&rx, deadline, is_rust) {
                return Err(crate::rust_app::Restart.into());
            }
            self.rebuild();
            self.watch_imports(&mut watcher, &mut imported_dirs)?;
            watch_globs(&mut globbed, &self.glob_dirs, &mut glob_dirs);
        }
    }

    /// Watch the directories of shared imports, so atomic saves and new
    /// modules work as they do inside the app. Its own tree is watched above.
    fn watch_imports(
        &self,
        watcher: &mut impl Watcher,
        watched: &mut HashSet<PathBuf>,
    ) -> anyhow::Result<()> {
        for dir in self
            .watch_files
            .borrow()
            .iter()
            .filter_map(|path| path.parent())
        {
            if !dir.starts_with(&self.src_dir) && watched.insert(dir.to_path_buf()) {
                watcher.watch(dir, RecursiveMode::NonRecursive)?;
                ui::debug(format!("watching shared source {}", ui::path(dir)));
            }
        }
        Ok(())
    }

    /// Whether the app takes the JSON file `path` ([`uses_json`]), as its
    /// last good build says; every one, before a good build or after a
    /// failed one.
    fn takes_json(&self, path: &Path) -> bool {
        uses_json(
            path,
            &self.src_dir,
            &self.watch_files.borrow(),
            &self.glob_patterns.borrow(),
            self.builds.get() == 0 || self.last_failed.get(),
        )
    }

    /// Serves the assets `built` imports by name, whatever their type: the
    /// app's directory itself serves only its media files. It is the one
    /// the build named them from (an entry below its app names them from
    /// the app's directory), so a literal `asset:/` URL finds what a
    /// package of the app holds.
    fn serve_assets(&self, built: &build::Built) {
        if let Some(root) = &built.asset_root {
            self.server.set_assets_root(Some(root.clone()));
        }
        match built
            .assets_for_context(&self.context)
            .and_then(|resolved| materialize_dev_assets(&resolved, &self.context_assets_dir))
        {
            Ok(files) => self.server.serve_files(files),
            Err(err) => ui::warn(format!("could not resolve context assets: {err:#}")),
        }
    }

    fn rebuild(&self) {
        // A Rust app has no JS: its Rust code's changes restart the session.
        if self.rust_root {
            return;
        }
        self.server.building();
        let active_ctx = (!self.context.is_empty()).then_some(&self.context);
        if self.release {
            match build::compile_with_context(&self.entry, true, false, active_ctx) {
                Ok(built) => {
                    self.note_web_view(built.native_use.as_ref().is_none_or(|u| u.web_view));
                    self.watch_files.replace(built.watch_files.clone());
                    *lock(&self.glob_dirs) = built.glob_dirs.clone();
                    self.glob_patterns.replace(built.glob_patterns.clone());
                    self.serve_assets(&built);
                    let capabilities = built.capabilities_json();
                    let summary = self.summary(&built);
                    self.server.publish(
                        built.bytecode,
                        built.code,
                        built.source_map,
                        nsh_dev::AppInfo {
                            capabilities,
                            url_schemes: built.url_schemes,
                            id: crate::package::app_id(&self.entry).ok(),
                            window: crate::package::window_json(&self.entry),
                            application: crate::package::application_json(&self.entry),
                        },
                        None,
                        built.ui_sets,
                    );
                    self.published(summary);
                }
                Err(err) => self.failed(err),
            }
            return;
        }
        match build::compile_hot_with_context(&self.entry, active_ctx) {
            Ok((built, hot)) => {
                // The compiled `<WebView>` passes the tag `webview` to
                // `element` (`nsh_bundler::NativeUse::web_view`).
                self.note_web_view(built.code.contains("webview"));
                self.watch_files.replace(built.watch_files.clone());
                *lock(&self.glob_dirs) = built.glob_dirs.clone();
                self.glob_patterns.replace(built.glob_patterns.clone());
                self.serve_assets(&built);
                let summary = self.summary(&built);
                // Other capabilities: the app starts again under them.
                let patch = self
                    .hot
                    .borrow()
                    .as_ref()
                    .filter(|previous| previous.capabilities == hot.capabilities)
                    .map(|previous| hot.patch(previous));
                let patch = match patch {
                    Some(nsh_bundler::HotPatch::Update { code, source_map }) => {
                        Some((code, source_map))
                    }
                    // Nothing to replace; hosts still clear a build error.
                    Some(nsh_bundler::HotPatch::Unchanged) => Some(("true".to_owned(), None)),
                    Some(nsh_bundler::HotPatch::Reload) | None => None,
                };
                let capabilities = built.capabilities_json();
                self.server.publish(
                    built.bytecode,
                    built.code,
                    built.source_map,
                    nsh_dev::AppInfo {
                        capabilities,
                        url_schemes: built.url_schemes,
                        id: crate::package::app_id(&self.entry).ok(),
                        window: crate::package::window_json(&self.entry),
                        application: crate::package::application_json(&self.entry),
                    },
                    patch,
                    built.ui_sets,
                );
                *self.hot.borrow_mut() = Some(hot);
                self.published(summary);
            }
            Err(err) => self.failed(err),
        }
    }

    /// Tells a Rust app's Linux development host whether the app may make
    /// a web view: by its bundle (`bundle`), or as `nsh.json` keeps
    /// `webview` ([`crate::rust_app::set_dev_bundle_web_view`]).
    fn note_web_view(&self, bundle: bool) {
        let kept = || {
            crate::package::read_config(&self.entry)
                .is_ok_and(|c| c.features.iter().any(|f| f == "webview"))
        };
        crate::rust_app::set_dev_bundle_web_view(bundle || kept());
    }

    /// What the line of a good build says after its path.
    fn summary(&self, built: &build::Built) -> String {
        if self.builds.get() == 0 {
            built.summary("")
        } else {
            ui::dim(format!("({})", ui::duration(built.elapsed)))
        }
    }

    /// `✓ built App.tsx (...)`, then `✓ rebuilt App.tsx (18ms), 2 devices`.
    fn published(&self, summary: String) {
        self.last_failed.set(false);
        let entry = ui::path(&self.entry);
        let builds = self.builds.get();
        self.builds.set(builds + 1);
        if builds == 0 {
            return ui::done(format!("built {entry} {summary}"));
        }
        let devices = match self.server.clients() {
            0 => String::new(),
            1 => ", 1 device".to_owned(),
            n => format!(", {n} devices"),
        };
        ui::done(format!("rebuilt {entry} {summary}{}", ui::dim(devices)));
    }

    fn failed(&self, err: build::BuildError) {
        self.last_failed.set(true);
        let plain = err.plain.clone();
        ui::report(&err.into_error(&self.entry));
        self.server.publish_error(plain);
    }
}

impl Drop for DevSession {
    fn drop(&mut self) {
        // The server stops next.
        self.devices.closing.store(true, Ordering::Relaxed);
    }
}

/// Converts resolved packaged assets into `(logical_path, disk_file)` pairs
/// for [`DevServer::serve_files`], writing any generated bytes (such as pruned
/// `assets.toml`) under `scratch_dir`.
fn materialize_dev_assets(
    assets: &[build::PackagedAsset],
    scratch_dir: &Path,
) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::with_capacity(assets.len());
    for asset in assets {
        match &asset.content {
            build::PackagedAssetContent::File(source) => {
                out.push((asset.path.clone(), source.clone()));
            }
            build::PackagedAssetContent::Bytes(bytes) => {
                let dest = scratch_dir.join(&asset.path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&dest, bytes)
                    .with_context(|| format!("write pruned {}", asset.path))?;
                out.push((asset.path.clone(), dest));
            }
        }
    }
    Ok(out)
}

/// The hosts connected to the dev server: what they do is shown as `•`
/// lines, their log lines with their names.
#[derive(Default)]
struct Devices {
    connected: Mutex<Vec<String>>,
    /// The session ends.
    closing: AtomicBool,
}

impl Devices {
    fn connected(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
        self.connected.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn on_event(&self, event: ClientEvent) {
        match event {
            ClientEvent::Connected { device, build } => {
                let build = build.map_or(String::new(), |b| format!(", build {b}"));
                ui::event(format!("{device} connected{build}"));
                self.connected().push(device);
            }
            ClientEvent::CannotRun {
                device,
                missing,
                install,
            } => {
                let label = short_name(&self.connected(), &device);
                let install =
                    install.map_or(String::new(), |c| format!(": {c} installs this app's host"));
                ui::warn(format!(
                    "{label}: its host has no {} (it was built for another app){install}",
                    missing.join(", ")
                ));
            }
            ClientEvent::Disconnected { device } => {
                {
                    let mut connected = self.connected();
                    if let Some(i) = connected.iter().position(|d| *d == device) {
                        connected.remove(i);
                    }
                }
                if !self.closing.load(Ordering::Relaxed) {
                    ui::event(ui::dim(format!("{device} disconnected")));
                }
            }
            ClientEvent::Log(log) => {
                let (label, several) = {
                    let connected = self.connected();
                    (short_name(&connected, &log.device), connected.len() > 1)
                };
                let device = logger::Device {
                    name: &log.device,
                    label: &label,
                    several,
                };
                logger::device(&device, log.level, &log.target, &log.message);
            }
            // Only a test session's traced app sends its batches.
            ClientEvent::Trace { .. } => {}
        }
    }
}

/// `device` without its model ("iOS Simulator (iPhone 16 Plus)": "iOS
/// Simulator"), unless another connected device has that name too.
fn short_name(connected: &[String], device: &str) -> String {
    let short = |name: &str| name.split_once(" (").map_or(name, |(s, _)| s).to_owned();
    let name = short(device);
    let shared = connected
        .iter()
        .any(|other| other != device && short(other) == name);
    if shared { device.to_owned() } else { name }
}

/// The commands that install the app in `dir`'s host, for the platforms
/// `nsh run` has a target for.
fn install_commands(dir: &Path) -> Vec<(HostPlatform, String)> {
    let app = match ui::path(dir) {
        dot if dot == "." => String::new(),
        path if path.contains(' ') => format!(" '{path}'"),
        path => format!(" {path}"),
    };
    use crate::run::Target;
    // The target this machine's `nsh run` defaults to goes without a flag.
    let run = |target: Target, name: &str| {
        if target == Target::native() {
            format!("nsh run{app}")
        } else {
            format!("nsh run{app} --target {name}")
        }
    };
    vec![
        (HostPlatform::MacCatalyst, run(Target::Catalyst, "catalyst")),
        (HostPlatform::MacOs, run(Target::Macos, "macos")),
        (HostPlatform::Linux, run(Target::Linux, "linux")),
        (HostPlatform::Windows, run(Target::Windows, "windows")),
        (HostPlatform::Gpui, run(Target::Gpui, "gpui")),
        (HostPlatform::Tui, run(Target::Tui, "tui")),
        (HostPlatform::IosSimulator, run(Target::Ios, "ios")),
        (HostPlatform::Android, run(Target::Android, "android")),
        (HostPlatform::Tizen, run(Target::Tizen, "tizen")),
        (HostPlatform::Web, run(Target::Web, "web")),
    ]
}

fn start_server(
    listen: Listen,
    assets_root: &Path,
    devices: Arc<Devices>,
) -> anyhow::Result<DevServer> {
    let Listen { port, host } = listen;
    if ui::stop_requested() {
        anyhow::bail!("development session is stopping");
    }
    let addr = SocketAddr::from((host, port));
    DevServer::start(
        addr,
        nsh_js::BYTECODE_VERSION,
        Some(assets_root.to_path_buf()),
        move |event| devices.on_event(event),
    )
    .map_err(|error| {
        let message = if error.kind() == std::io::ErrorKind::AddrInUse {
            format!(
                "start dev server: port {port} is taken; stop what uses it or choose another --port"
            )
        } else {
            format!("start dev server on port {port}")
        };
        anyhow::Error::new(error).context(message)
    })
}

/// Waits out the debounce of a source change: whether one of the events
/// arriving until `deadline` changes the app's Rust code (`is_rust`; the
/// first that does ends the wait). Reads change nothing, as in the watch
/// loop.
fn rust_changed_until(
    rx: &mpsc::Receiver<notify::Event>,
    deadline: Instant,
    is_rust: impl Fn(&Path) -> bool,
) -> bool {
    while let Ok(event) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        if !matches!(event.kind, notify::EventKind::Access(_))
            && event.paths.iter().any(|p| is_rust(p))
        {
            return true;
        }
    }
    false
}

/// The lock of `globs`, whatever a thread that held it did.
fn lock(
    globs: &Mutex<Vec<nsh_bundler::GlobDir>>,
) -> std::sync::MutexGuard<'_, Vec<nsh_bundler::GlobDir>> {
    globs.lock().unwrap_or_else(|e| e.into_inner())
}

/// A watcher of the directories outside the app that `import.meta.glob`
/// patterns search ([`watch_globs`]): it sends `tx` the events of the
/// files `globs` match, in directories new too, and none of the rest of
/// what it watches (a library's other files, its `target` or
/// `node_modules`, which the patterns' search skips unless they name them).
fn glob_watcher(
    globs: Arc<Mutex<Vec<nsh_bundler::GlobDir>>>,
    tx: mpsc::Sender<notify::Event>,
) -> notify::Result<notify::RecommendedWatcher> {
    notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            let globs = lock(&globs);
            if event
                .paths
                .iter()
                .any(|p| globs.iter().any(|g| g.matches(p)))
            {
                let _ = tx.send(event);
                return;
            }
            // A directory made under a glob's: inotify watches it only
            // once it reports it, so the files already written in it come
            // without events of their own. They are looked for here.
            if matches!(event.kind, notify::EventKind::Create(_)) {
                let mut found = Vec::new();
                for path in &event.paths {
                    if path.is_dir() && globs.iter().any(|g| path.starts_with(&g.dir)) {
                        globbed_files(path, &globs, &mut found);
                    }
                }
                if !found.is_empty() {
                    let mut files = notify::Event::new(notify::EventKind::Create(
                        notify::event::CreateKind::File,
                    ));
                    files.paths = found;
                    let _ = tx.send(files);
                }
            }
        }
    })
}

/// The files below `dir` that `globs` match, into `found`. The directories
/// the globs' search skips (`node_modules`, `target`, a dot directory) are
/// not entered unless a pattern names them.
fn globbed_files(dir: &Path, globs: &[nsh_bundler::GlobDir], found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let skipped = name.starts_with('.') || name == "node_modules" || name == "target";
            if !skipped || globs.iter().any(|g| g.pattern.contains(name.as_str())) {
                globbed_files(&path, globs, found);
            }
        } else if globs.iter().any(|g| g.matches(&path)) {
            found.push(path);
        }
    }
}

/// Watches, recursively, the directories of `globs` not `watched` yet. One
/// that cannot be watched is reported and tried again after the next build.
fn watch_globs(
    watcher: &mut impl Watcher,
    globs: &Mutex<Vec<nsh_bundler::GlobDir>>,
    watched: &mut HashSet<PathBuf>,
) {
    let dirs: Vec<PathBuf> = lock(globs).iter().map(|g| g.dir.clone()).collect();
    for dir in dirs {
        if watched.contains(&dir) {
            continue;
        }
        match watcher.watch(&dir, RecursiveMode::Recursive) {
            Ok(()) => {
                ui::debug(format!("watching {} for import.meta.glob", ui::path(&dir)));
                watched.insert(dir);
            }
            Err(err) => ui::warn(format!(
                "cannot watch {} for import.meta.glob: {err}",
                ui::path(&dir)
            )),
        }
    }
}

/// Whether `event` changed what a build of the app in `root` takes: a module
/// (`.ts`, `.tsx`, `.js`, `.jsx`), or a `.json` file the app takes
/// (`takes_json`, [`uses_json`]), not in `.nsh/`, `node_modules/` or another
/// checkout kept in the app, nor in a directory `nsh.json`'s
/// `"watch.ignore"` names (`ignored`). A JSON file the app keeps beside its
/// code without importing it (data a tool writes there) starts no build.
fn is_source(
    event: &notify::Event,
    root: &Path,
    takes_json: &dyn Fn(&Path) -> bool,
    ignored: &[PathBuf],
) -> bool {
    event.paths.iter().any(|p| {
        !p.components()
            .any(|c| c.as_os_str() == ".nsh" || c.as_os_str() == "node_modules")
            && !in_nested_checkout(root, p)
            && !ignored.iter().any(|dir| p.starts_with(dir))
            && match p.extension().and_then(|e| e.to_str()) {
                Some("ts" | "tsx" | "js" | "jsx") => true,
                Some("json") => takes_json(p),
                _ => false,
            }
    })
}

/// Whether the app in `app` takes the JSON file `path`: the last good build
/// read it (`read`: an imported JSON module, the theme, its overrides, the
/// translations), it is the app's `nsh.json`, it is in the app's translations
/// folder (`locales/`, whose files the build reads all of: a new one too), or
/// one of the build's `import.meta.glob` patterns matches it (`globs`), a
/// file new too.
/// `unknown`: there is no good build to tell (none yet, or the last
/// failed), and every one is taken.
fn uses_json(
    path: &Path,
    app: &Path,
    read: &[PathBuf],
    globs: &[nsh_bundler::GlobDir],
    unknown: bool,
) -> bool {
    if unknown {
        return true;
    }
    // Events name a file as the system does; the build, as it resolved it.
    let canonical = dunce::canonicalize(path).ok();
    let named = |file: &Path| file == path || canonical.as_deref() == Some(file);
    let locales = app.join(nsh_i18n::LOCALES_DIR);
    named(&app.join("nsh.json"))
        || path.starts_with(&locales)
        || canonical
            .as_deref()
            .is_some_and(|c| c.starts_with(&locales))
        || read.iter().any(|file| named(file))
        || globs
            .iter()
            .any(|glob| glob.matches(path) || canonical.as_deref().is_some_and(|c| glob.matches(c)))
}

/// The absolute directories `ignore` names (`nsh.json`'s `"watch.ignore"`):
/// entries are relative to `dir`, absolute ones as they are.
pub(crate) fn ignored_dirs(ignore: &[String], dir: &Path) -> Vec<PathBuf> {
    ignore
        .iter()
        .map(|entry| {
            let path = Path::new(entry);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                dir.join(path)
            }
        })
        .collect()
}

/// Whether `path` is in another Git checkout kept below `root` (a
/// `git worktree` or a clone in `.worktrees/`): a directory between them
/// holds its own `.git`. Its edits are not the app's.
pub(crate) fn in_nested_checkout(root: &Path, path: &Path) -> bool {
    let Ok(rest) = path.strip_prefix(root) else {
        return false;
    };
    let mut dir = root.to_path_buf();
    let mut parts = rest.components().peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            break;
        }
        dir.push(part);
        if dir.join(".git").exists() {
            return true;
        }
    }
    false
}

/// Private IPv4 addresses of physical network interfaces (`en*`; on Linux
/// also `eth*` and Wi-Fi's `wl*`), where devices on the same Wi-Fi or
/// Ethernet can reach this machine. VPN and proxy tunnels are skipped.
#[cfg(unix)]
fn lan_ips() -> Vec<Ipv4Addr> {
    let mut ips = Vec::new();
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills a linked list that we walk read-only and free
    // with freeifaddrs.
    unsafe {
        if libc::getifaddrs(&mut list) != 0 {
            return ips;
        }
        let mut cur = list;
        while let Some(ifa) = cur.as_ref() {
            cur = ifa.ifa_next;
            if ifa.ifa_addr.is_null() || i32::from((*ifa.ifa_addr).sa_family) != libc::AF_INET {
                continue;
            }
            let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy();
            if !["en", "eth", "wl"].iter().any(|p| name.starts_with(p)) {
                continue;
            }
            let addr = &*(ifa.ifa_addr as *const libc::sockaddr_in);
            let ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
            if ip.is_private() && !ips.contains(&ip) {
                ips.push(ip);
            }
        }
        libc::freeifaddrs(list);
    }
    ips
}

/// Private IPv4 addresses of the Ethernet and Wi-Fi adapters that are up
/// (by Windows' interface types: VPN, Hyper-V and WSL adapters are of
/// other types).
#[cfg(windows)]
fn lan_ips() -> Vec<Ipv4Addr> {
    use windows_sys::Win32::{
        Foundation::ERROR_BUFFER_OVERFLOW,
        NetworkManagement::{
            IpHelper::{
                GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
                GetAdaptersAddresses, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211,
                IP_ADAPTER_ADDRESSES_LH,
            },
            Ndis::IfOperStatusUp,
        },
        Networking::WinSock::{AF_INET, SOCKADDR_IN},
    };

    let mut ips = Vec::new();
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    // u64s keep the structures written into the buffer aligned.
    let mut buffer: Vec<u64> = vec![0; 2048];
    loop {
        let mut size = (buffer.len() * 8) as u32;
        // SAFETY: the buffer holds `size` writable bytes.
        let result = unsafe {
            GetAdaptersAddresses(
                u32::from(AF_INET),
                flags,
                std::ptr::null(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if result == ERROR_BUFFER_OVERFLOW {
            buffer = vec![0; (size as usize).div_ceil(8)];
            continue;
        }
        if result != 0 {
            return ips;
        }
        break;
    }
    let mut adapter = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    // SAFETY: the call filled the buffer with a linked list of adapters and
    // their addresses, all inside it, walked read-only while it lives.
    unsafe {
        while let Some(a) = adapter.as_ref() {
            adapter = a.Next;
            if a.OperStatus != IfOperStatusUp
                || !matches!(a.IfType, IF_TYPE_ETHERNET_CSMACD | IF_TYPE_IEEE80211)
            {
                continue;
            }
            let mut unicast = a.FirstUnicastAddress;
            while let Some(u) = unicast.as_ref() {
                unicast = u.Next;
                let addr = u.Address.lpSockaddr;
                if addr.is_null() || (*addr).sa_family != AF_INET {
                    continue;
                }
                let addr = &*(addr as *const SOCKADDR_IN);
                let ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.S_un.S_addr));
                if ip.is_private() && !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
        }
    }
    ips
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::Listen;

    fn on_network(port: u16) -> Listen {
        Listen::new(port, true)
    }

    #[test]
    fn only_a_network_listen_is_open_to_other_machines() {
        let dir = std::env::temp_dir();
        let local = Listen::new(0, false);
        let server = super::start_server(local, &dir, Default::default()).unwrap();
        assert!(server.local_addr().ip().is_loopback());
        let server = super::start_server(on_network(0), &dir, Default::default()).unwrap();
        assert!(server.local_addr().ip().is_unspecified());
    }

    #[test]
    fn explicit_host_is_the_bound_address() {
        let listen = Listen {
            port: 0,
            host: Ipv4Addr::LOCALHOST,
        };
        let server =
            super::start_server(listen, &std::env::temp_dir(), Default::default()).unwrap();
        assert_eq!(server.local_addr().ip(), listen.host);
    }

    #[test]
    fn loopback_url_targets_127_0_0_1_even_when_bound_to_all_interfaces() {
        let dir = std::env::temp_dir().join(format!("nsh-loopback-url-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let entry = dir.join("App.tsx");
        std::fs::write(&entry, "export default function App() { return null; }\n").unwrap();
        let session = super::DevSession::start(&entry, on_network(0), false, None, None).unwrap();
        let expected = format!("ws://127.0.0.1:{}{}", session.port(), super::WS_PATH);
        assert_eq!(session.loopback_url(), expected);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn occupied_port_fails_without_moving_or_stopping_the_listener() {
        let taken = std::net::TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = taken.local_addr().unwrap().port();
        let error =
            super::start_server(on_network(port), &std::env::temp_dir(), Default::default())
                .err()
                .expect("must not move to another port");
        assert!(error.to_string().contains(&format!("port {port}")));
        assert_eq!(
            error
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::AddrInUse
        );
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
    }

    #[test]
    fn zero_allocates_a_port_and_an_explicit_port_is_kept() {
        let dir = std::env::temp_dir();
        let server = super::start_server(on_network(0), &dir, Default::default()).unwrap();
        let port = server.local_addr().port();
        assert_ne!(port, 0);
        drop(server);
        let server = super::start_server(on_network(port), &dir, Default::default()).unwrap();
        assert_eq!(server.local_addr().port(), port);
    }

    #[test]
    fn devices_go_by_short_names_unless_two_share_one() {
        let connected = [
            "iOS Simulator (iPhone 16 Plus)".to_owned(),
            "Mac Catalyst".to_owned(),
        ];
        assert_eq!(
            super::short_name(&connected, &connected[0]),
            "iOS Simulator"
        );
        assert_eq!(
            super::short_name(&connected, "Mac Catalyst"),
            "Mac Catalyst"
        );
        let two = [
            "iOS Simulator (iPhone 16 Plus)".to_owned(),
            "iOS Simulator (iPad Air)".to_owned(),
        ];
        assert_eq!(super::short_name(&two, &two[1]), "iOS Simulator (iPad Air)");
    }

    /// A file a glob outside the app matches, added in a directory that is
    /// new, reaches the session; what the glob's search skips does not.
    #[test]
    fn glob_watch_sees_a_file_added_in_a_new_directory_outside_the_app() {
        use std::{
            collections::HashSet,
            path::Path,
            sync::{Arc, Mutex},
            time::{Duration, Instant},
        };

        let top = std::env::temp_dir().join(format!("nsh-glob-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        let write = |path: &Path, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(&top.join("lib/src/A.preview.tsx"), "export const a = 1;\n");
        write(
            &top.join("app/App.tsx"),
            "import { mount, Text } from \"nsh\";\n\
             const all = import.meta.glob(\"/../lib/**/*.preview.tsx\");\n\
             mount(() => <Text>{Object.keys(all).join()}</Text>);\n",
        );
        let top = dunce::canonicalize(&top).unwrap();
        let hot = nsh_bundler::bundle_hot(&top.join("app/App.tsx"), &[]).unwrap();
        let dirs: Vec<&Path> = hot.glob_dirs.iter().map(|g| g.dir.as_path()).collect();
        assert_eq!(dirs, [top.join("lib")]);

        let globs = Arc::new(Mutex::new(hot.glob_dirs.clone()));
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = super::glob_watcher(globs.clone(), tx).unwrap();
        let mut watched = HashSet::new();
        super::watch_globs(&mut watcher, &globs, &mut watched);
        assert!(watched.contains(&top.join("lib")));
        // The watcher's stream starts.
        std::thread::sleep(Duration::from_millis(300));
        write(&top.join("lib/node_modules/x/C.preview.tsx"), "");
        write(&top.join("lib/src/new/D.tsx"), "");
        let added = top.join("lib/src/new/deeper/B.preview.tsx");
        write(&added, "export const b = 1;\n");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = false;
        while let Ok(event) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            assert!(
                !event
                    .paths
                    .iter()
                    .all(|p| p.ends_with("C.preview.tsx") || p.ends_with("D.tsx")),
                "{event:?}"
            );
            if event.paths.contains(&added) {
                seen = true;
                break;
            }
        }
        assert!(seen, "no event for {}", added.display());
        drop(watcher);
        let _ = std::fs::remove_dir_all(top);
    }

    #[test]
    fn a_rust_change_right_after_a_tsx_change_is_not_lost() {
        use std::{path::Path, time::Instant};

        use notify::{
            Event, EventKind,
            event::{AccessKind, DataChange, ModifyKind},
        };

        let saved = |path: &str| {
            Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
                .add_path(path.into())
        };
        let is_rust = |path: &Path| path.extension().is_some_and(|e| e == "rs");
        let (tx, rx) = std::sync::mpsc::channel();
        // `App.tsx`, then `src/lib.rs` within the debounce (a checkout, a
        // formatter): the session starts again.
        tx.send(saved("/app/App.tsx")).unwrap();
        tx.send(saved("/app/src/lib.rs")).unwrap();
        let deadline = Instant::now() + super::DEBOUNCE;
        assert!(super::rust_changed_until(&rx, deadline, is_rust));
        // A read of the Rust code (inotify reports a build's own) is no
        // change: only the TSX is built again.
        tx.send(saved("/app/App.tsx")).unwrap();
        tx.send(Event::new(EventKind::Access(AccessKind::Read)).add_path("/app/src/lib.rs".into()))
            .unwrap();
        let deadline = Instant::now() + super::DEBOUNCE;
        assert!(!super::rust_changed_until(&rx, deadline, is_rust));
    }

    #[test]
    fn edits_in_a_checkout_kept_in_the_app_do_not_rebuild_it() {
        use notify::{
            Event, EventKind,
            event::{DataChange, ModifyKind},
        };

        let dir = std::env::temp_dir().join(format!("nsh-nested-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let nested = dir.join(".worktrees/feature");
        std::fs::create_dir_all(nested.join("src")).unwrap();
        std::fs::write(nested.join(".git"), "gitdir: elsewhere\n").unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let saved = |path: std::path::PathBuf| {
            Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content))).add_path(path)
        };
        let any_json = |_: &std::path::Path| true;
        let no_ignored: Vec<std::path::PathBuf> = Vec::new();
        assert!(super::is_source(
            &saved(dir.join("App.tsx")),
            &dir,
            &any_json,
            &no_ignored
        ));
        assert!(super::is_source(
            &saved(dir.join("src/view.tsx")),
            &dir,
            &any_json,
            &no_ignored
        ));
        assert!(!super::is_source(
            &saved(nested.join("App.tsx")),
            &dir,
            &any_json,
            &no_ignored
        ));
        assert!(!super::is_source(
            &saved(nested.join("src/view.tsx")),
            &dir,
            &any_json,
            &no_ignored
        ));
        // The app's own `.git` is not a checkout below it.
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        assert!(super::is_source(
            &saved(dir.join("App.tsx")),
            &dir,
            &any_json,
            &no_ignored
        ));
        // What `watch.ignore` names is skipped, however it is spelled.
        let ignored = super::ignored_dirs(
            &[
                "vendor".into(),
                "third/jpeg".into(),
                nested.to_string_lossy().into(),
            ],
            &dir,
        );
        assert_eq!(
            ignored,
            vec![
                dir.join("vendor"),
                dir.join("third/jpeg"),
                dir.join(".worktrees/feature")
            ]
        );
        assert!(!super::is_source(
            &saved(dir.join("vendor/lib.ts")),
            &dir,
            &any_json,
            &ignored
        ));
        assert!(!super::is_source(
            &saved(dir.join("third/jpeg/turbo.ts")),
            &dir,
            &any_json,
            &ignored
        ));
        assert!(super::is_source(
            &saved(dir.join("third/png.ts")),
            &dir,
            &any_json,
            &ignored
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_json_file_rebuilds_only_when_the_app_takes_it() {
        use notify::{
            Event, EventKind,
            event::{DataChange, ModifyKind},
        };
        use nsh_bundler::GlobDir;

        let made = std::env::temp_dir().join(format!("nsh-json-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&made);
        for dir in ["data", "content/posts", "design/pages", "tools", "locales"] {
            std::fs::create_dir_all(made.join(dir)).unwrap();
        }
        let dir = dunce::canonicalize(&made).unwrap();
        for file in [
            "nsh.json",
            "App.tsx",
            "data/strings.json",
            "content/posts/one.json",
            "design/pages/a.json",
            "tools/nsh.json",
        ] {
            std::fs::write(dir.join(file), "{}").unwrap();
        }
        let saved = |path: std::path::PathBuf| {
            Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content))).add_path(path)
        };
        // The last good build read the app and its strings, and searched
        // `content/` with a pattern.
        let read = [dir.join("App.tsx"), dir.join("data/strings.json")];
        let globs = [GlobDir::new(dir.join("content"), "**/*.json")];
        let takes = |path: &std::path::Path| super::uses_json(path, &dir, &read, &globs, false);
        let rebuilds = |file: &str| super::is_source(&saved(dir.join(file)), &dir, &takes, &[]);
        // A module, as before; the app's nsh.json, a JSON module it imports,
        // and one its pattern matches, a new file too.
        assert!(rebuilds("App.tsx"));
        assert!(rebuilds("src/New.tsx"));
        assert!(rebuilds("nsh.json"));
        assert!(rebuilds("data/strings.json"));
        assert!(rebuilds("content/posts/one.json"));
        assert!(rebuilds("content/posts/two.json"));
        // A translation added: the build reads every file of `locales/`.
        assert!(rebuilds("locales/new.json"));
        assert!(rebuilds("locales/fr/app.json"));
        // Data a tool keeps in the app, which nothing imports, and another
        // folder's nsh.json: nothing to build.
        assert!(!rebuilds("design/pages/a.json"));
        assert!(!rebuilds("design/pages/b.json"));
        assert!(!rebuilds("tools/nsh.json"));
        // Named as the system names it (a link to the app), it is the same.
        #[cfg(unix)]
        {
            let link = std::env::temp_dir().join(format!("nsh-json-link-{}", std::process::id()));
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(&dir, &link).unwrap();
            assert!(super::is_source(
                &saved(link.join("data/strings.json")),
                &dir,
                &takes,
                &[]
            ));
            assert!(!super::is_source(
                &saved(link.join("design/pages/a.json")),
                &dir,
                &takes,
                &[]
            ));
            std::fs::remove_file(&link).unwrap();
        }
        // No good build to tell (none yet, or the last failed): every JSON
        // file builds.
        let unknown = |path: &std::path::Path| super::uses_json(path, &dir, &read, &globs, true);
        assert!(super::is_source(
            &saved(dir.join("design/pages/a.json")),
            &dir,
            &unknown,
            &[]
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
