//! A design tool's view of an app on Linux under Wayland (`nsh/design`,
//! `docs/dev/design-mode.md`): the test plays the tool. It runs a minimal
//! Smithay Wayland compositor (`wl_compositor`, `wl_shm`, `xdg_wm_base`,
//! `wl_seat`, `wl_output`, `wl_data_device_manager`) and starts this binary
//! again as the app (`DESIGN_WAYLAND_APP`), a small JS app run by the real
//! Linux GTK 4 host in design mode (`NSH_DESIGN_WAYLAND=1`,
//! `GDK_BACKEND=wayland`, `WAYLAND_DISPLAY=<compositor>`).
//!
//! - `hello`: with `NSH_DESIGN_TOKEN` the hello carries the token, without it
//!   the hello is empty; at trace log level the log holds no trace of the token.
//! - `no_wayland_env`: without `NSH_DESIGN_WAYLAND=1`, design mode reports no
//!   surfaces and logs once.
//! - `surfaces`: each opened window reports `"wayland": <n>` matching a mapped
//!   `xdg_toplevel` titled `"nsh-design:<n>"` with real `wl_shm` buffers; the
//!   main window never maps a toplevel; closing emits `"gone": true`; `resize`
//!   changes its size; `viewport` does not resize it to 1x1.
//! - `pictures`, `settle`, `loops`, `keyframes`, `shadows`, `codes`:
//!   `design.picture` and `design.settled` at scale 1 and 2, `rect`, `hide`,
//!   settling, endless loops/keyframes at first frame, outer shadows, and all
//!   error codes.
//! - `wayland_input`: real Wayland pointer motion and button press/release
//!   from the Smithay seat reach a pressable view in the app.
//!
//! Without libtest's harness: arguments pick cases (none runs all), each
//! prints `design_wayland: <case>: ok`, and the last line says how many ran.

#[cfg(target_os = "linux")]
fn main() {
    if std::env::var_os(app::ROLE).is_some() {
        app::run();
    } else {
        tool::run();
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}

/// The app's script.
#[cfg(target_os = "linux")]
const APP: &str = r##"import { Animated, createAnimatedValue, mount, signal, Text, View } from "nsh";
import { Window } from "nsh/desktop";
import { design } from "nsh/design";

const WINDOW = { frame: "none", taskbar: false, activate: false, shadow: false, transparent: true, sizeToContent: true } as const;

const tick = signal(0);
const faded = signal(1);
const clicks = signal(0);

mount(() => (
  <View>
    <Text>{`tick ${tick()}`}</Text>
  </View>
));

function HiddenPart() {
  return <View testID="hidden-part" style={{ marginLeft: 60, width: 10, height: 10, backgroundColor: "#00ff00" }} />;
}

function Box() {
  return (
    <View testID="box" style={{ width: 100, height: 80 }}>
      <View style={{ marginLeft: 10, marginTop: 10, width: 40, height: 30, backgroundColor: "#ff0000" }} />
      <HiddenPart />
    </View>
  );
}

const spin = createAnimatedValue(0.2);
const fade = createAnimatedValue(0);
Animated.loop(Animated.timing(spin, { toValue: 1, duration: 1000 }), { resetBeforeIteration: true }).start();

const contents: Record<string, () => unknown> = {
  box: () => <Box />,
  spin: () => <View style={{ width: 60, height: 60, backgroundColor: "#0000ff", opacity: spin }} />,
  fade: () => <View style={{ width: 60, height: 60, backgroundColor: "#0000ff", opacity: fade }} />,
  keyframes: () => <View class="animate-pulse" style={{ width: 60, height: 60, backgroundColor: "#0000ff" }} />,
  transit: () => (
    <View style={{ width: 60, height: 60, backgroundColor: "#0000ff", opacity: faded(), transitionProperty: "opacity", transitionDuration: 300 }} />
  ),
  shadow: () => (
    <View style={{ width: 120, height: 120 }}>
      <View style={{ marginLeft: 30, marginTop: 30, width: 60, height: 60, backgroundColor: "#ffffff", boxShadow: "0 8px 12px rgba(0,0,0,0.6)" }} />
    </View>
  ),
  click: () => (
    <View
      testID="click-btn"
      onPress={() => clicks.set(clicks() + 1)}
      style={{ width: 120, height: 80, backgroundColor: "#336699" }}
    >
      <Text>{`clicks ${clicks()}`}</Text>
    </View>
  ),
  late: () => <View style={{ width: 50, height: 50, backgroundColor: "#888888" }} />,
};

const windows: Record<string, { close(): void }> = {};
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const code = async (p: Promise<unknown>) => {
  try {
    await p;
    return "resolved";
  } catch (e: any) {
    return e.code ?? String(e.message);
  }
};
const timed = async (p: Promise<{ timedOut: boolean }>) => {
  const t = Date.now();
  const r = await p;
  return { timedOut: r.timedOut, ms: Date.now() - t };
};

/** NSH knows a window once its first commit is in. */
async function opened(id: string) {
  for (let i = 0; i < 200; i++) {
    try {
      return await design.settled(id);
    } catch (e: any) {
      if (e.code !== "ERR_NSH_DESIGN_NO_WINDOW") throw e;
      await wait(20);
    }
  }
  throw new Error(`window ${id} never opened`);
}

const steps: Record<string, (m: any) => Promise<unknown>> = {
  open: async (m) => {
    for (const id of m.ids) windows[id] = Window.open(contents[id] as any, { id, title: `custom-${id}`, ...WINDOW });
    for (const id of m.ids) await opened(id);
    return {};
  },
  close: async (m) => {
    windows[m.id]?.close();
    delete windows[m.id];
    await wait(120);
    return {};
  },
  picture: async (m) => {
    try {
      return { picture: await design.picture(m.window, m.options) };
    } catch (e: any) {
      return { code: e.code ?? String(e.message) };
    }
  },
  settleQuiet: async () => {
    await wait(200);
    return timed(design.settled("box"));
  },
  settleTimer: async () => {
    await wait(200);
    setTimeout(() => tick.set(tick() + 1), 30);
    return timed(design.settled("box"));
  },
  settleAnimation: async () => {
    await wait(200);
    Animated.timing(fade, { toValue: 1, duration: 300 }).start();
    return timed(design.settled("fade"));
  },
  settleKeyframes: async () => {
    await wait(200);
    return timed(design.settled("keyframes"));
  },
  settleTransition: async () => {
    await wait(200);
    faded.set(faded() === 1 ? 0.2 : 1);
    return timed(design.settled("transit"));
  },
  settleBusy: async () => {
    const busy = setInterval(() => tick.set(tick() + 1), 10);
    try {
      return await timed(design.settled("box"));
    } finally {
      clearInterval(busy);
    }
  },
  codes: async (m) => ({
    pictureWindow: await code(design.picture("nope", { scale: 1, path: m.dir + "/x.raw" })),
    settledWindow: await code(design.settled("nope")),
    outside: await code(design.picture("box", { scale: 1, path: m.dir + "/x.raw", rect: { x: 90, y: 0, width: 20, height: 10 } })),
    path: await code(design.picture("box", { scale: 1, path: m.dir + "/missing/x.raw" })),
    scale: await code(design.picture("box", { scale: 0, path: m.dir + "/x.raw" })),
    relative: await code(design.picture("box", { scale: 1, path: "x.raw" })),
  }),
  clicks: async () => ({ clicks: clicks() }),
};

(async () => {
  for (;;) {
    const m: any = await design.next();
    const ask = m.do;
    if (!ask) continue;
    let out: unknown;
    try {
      const step = steps[ask.step];
      out = step ? await step(ask) : { error: `no step ${ask.step}` };
    } catch (e: any) {
      out = { error: String(e?.message ?? e), code: e?.code };
    }
    design.send({ done: { n: ask.n, ...(out as object) } });
  }
})();
"##;

/// The app: the Linux GTK 4 host running [`APP`] in design mode.
#[cfg(target_os = "linux")]
mod app {
    use std::path::PathBuf;

    use nsh_platform_linux::{AppOptions, AppSource, Bundle, dev, run_app, set_ui_sets};

    /// Set in the app's environment: this process is the app.
    pub(crate) const ROLE: &str = "DESIGN_WAYLAND_APP";
    /// The folder the app's files go to.
    pub(crate) const DIR: &str = "DESIGN_WAYLAND_DIR";

    pub(crate) fn run() {
        dev::init_logging();
        // NSH's most verbose: nothing it logs may hold the run's token.
        log::set_max_level(log::LevelFilter::Trace);
        let dir = PathBuf::from(std::env::var(DIR).expect("the app's folder"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("App.tsx"), super::APP).unwrap();
        let sets = vec![nsh_base::registry()];
        let described: Vec<_> = nsh_core::element::Registry::served_sets(&sets)
            .into_iter()
            .map(Into::into)
            .collect();
        let bundle = nsh_bundler::bundle_hot(&dir.join("App.tsx"), &described)
            .unwrap_or_else(|e| panic!("the app does not bundle: {}", e.plain));
        let (code, map) = bundle.script();
        let mut bundle = Bundle::source(code)
            .with_capabilities_json(Some(r#"{ "windows": true }"#))
            .unwrap();
        if let Some(map) = map {
            bundle = bundle.with_source_map(map);
        }
        set_ui_sets(sets);
        let options = AppOptions {
            app_id: "dev.nsh.test.design-wayland".into(),
            title: "design wayland".into(),
            size: (200, 100),
            ..AppOptions::default()
        };
        let code = run_app(AppSource::Bundle(bundle), options);
        std::process::exit(i32::from(code));
    }
}

/// Minimal Smithay Wayland compositor for the test tool.
#[cfg(target_os = "linux")]
mod compositor {
    use std::{
        collections::HashMap,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Sender},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    use smithay::{
        delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
        delegate_xdg_shell,
        input::{
            Seat, SeatHandler, SeatState,
            keyboard::XkbConfig,
            pointer::{ButtonEvent, MotionEvent},
        },
        output::{Mode, Output, PhysicalProperties, Subpixel},
        reexports::{
            calloop::{EventLoop, Interest, Mode as LoopMode, PostAction, generic::Generic},
            wayland_protocols::xdg::shell::server::xdg_toplevel,
            wayland_server::{
                Client, Display, DisplayHandle, Resource,
                backend::{ClientData, ClientId, DisconnectReason},
                protocol::{wl_buffer::WlBuffer, wl_seat::WlSeat, wl_surface::WlSurface},
            },
        },
        utils::{Point, SERIAL_COUNTER, Serial},
        wayland::{
            buffer::BufferHandler,
            compositor::{
                BufferAssignment, CompositorClientState, CompositorHandler, CompositorState,
                SurfaceAttributes, with_states,
            },
            output::OutputManagerState,
            selection::{
                SelectionHandler,
                data_device::{
                    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
                },
            },
            shell::xdg::{
                PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
                XdgToplevelSurfaceData,
            },
            shm::{ShmHandler, ShmState, with_buffer_contents},
            socket::ListeningSocketSource,
        },
    };

    /// Linux `BTN_LEFT`.
    const BTN_LEFT: u32 = 0x110;

    /// Observable state of one mapped toplevel.
    #[derive(Debug, Clone)]
    pub(crate) struct ToplevelInfo {
        pub(crate) title: Option<String>,
        pub(crate) buffer_size: Option<(i32, i32)>,
        pub(crate) commits: u64,
    }

    enum Cmd {
        Click { title: String, x: f64, y: f64 },
        Stop,
    }

    struct ClientState {
        compositor_state: CompositorClientState,
    }

    impl ClientData for ClientState {
        fn initialized(&self, _client_id: ClientId) {}
        fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
    }

    struct State {
        display_handle: DisplayHandle,
        compositor_state: CompositorState,
        xdg_shell_state: XdgShellState,
        shm_state: ShmState,
        _output_manager_state: OutputManagerState,
        seat_state: SeatState<Self>,
        data_device_state: DataDeviceState,
        seat: Seat<Self>,
        toplevels: Vec<ToplevelSurface>,
        snapshot: Arc<Mutex<HashMap<u32, ToplevelInfo>>>,
        started: Instant,
    }

    impl State {
        fn update_snapshot(&self, surface: &WlSurface, buffer_size: Option<(i32, i32)>) {
            let key = surface.id().protocol_id();
            let title = with_states(surface, |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .and_then(|d| d.lock().ok().and_then(|g| g.title.clone()))
            });
            let mut map = self.snapshot.lock().unwrap();
            let entry = map.entry(key).or_insert(ToplevelInfo {
                title: None,
                buffer_size: None,
                commits: 0,
            });
            if title.is_some() {
                entry.title = title;
            }
            if buffer_size.is_some() {
                entry.buffer_size = buffer_size;
            }
            entry.commits += 1;
        }

        fn send_click(&mut self, title: &str, x: f64, y: f64) {
            let target = self
                .toplevels
                .iter()
                .find(|t| {
                    with_states(t.wl_surface(), |states| {
                        states
                            .data_map
                            .get::<XdgToplevelSurfaceData>()
                            .and_then(|d| d.lock().ok().and_then(|g| g.title.clone()))
                            .as_deref()
                            == Some(title)
                    })
                })
                .map(|t| t.wl_surface().clone());
            let Some(surface) = target else {
                return;
            };
            let Some(pointer) = self.seat.get_pointer() else {
                return;
            };
            let time = self.started.elapsed().as_millis() as u32;
            let loc = Point::from((x, y));
            pointer.motion(
                self,
                Some((surface.clone(), Point::from((0.0, 0.0)))),
                &MotionEvent {
                    location: loc,
                    serial: SERIAL_COUNTER.next_serial(),
                    time,
                },
            );
            pointer.frame(self);
            pointer.button(
                self,
                &ButtonEvent {
                    serial: SERIAL_COUNTER.next_serial(),
                    time: time + 5,
                    button: BTN_LEFT,
                    state: smithay::backend::input::ButtonState::Pressed,
                },
            );
            pointer.frame(self);
            pointer.button(
                self,
                &ButtonEvent {
                    serial: SERIAL_COUNTER.next_serial(),
                    time: time + 15,
                    button: BTN_LEFT,
                    state: smithay::backend::input::ButtonState::Released,
                },
            );
            pointer.frame(self);
        }
    }

    impl BufferHandler for State {
        fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
    }

    impl CompositorHandler for State {
        fn compositor_state(&mut self) -> &mut CompositorState {
            &mut self.compositor_state
        }

        fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
            &client.get_data::<ClientState>().unwrap().compositor_state
        }

        fn commit(&mut self, surface: &WlSurface) {
            let mut buffer_size = None;
            let time = self.started.elapsed().as_millis() as u32;
            with_states(surface, |states| {
                let mut attrs = states.cached_state.get::<SurfaceAttributes>();
                let current = attrs.current();
                if let Some(BufferAssignment::NewBuffer(buf)) = current.buffer.take() {
                    let _ = with_buffer_contents(&buf, |_ptr, _len, data| {
                        buffer_size = Some((data.width, data.height));
                    });
                    buf.release();
                }
                for cb in current.frame_callbacks.drain(..) {
                    cb.done(time);
                }
            });
            self.update_snapshot(surface, buffer_size);
        }
    }

    impl XdgShellHandler for State {
        fn xdg_shell_state(&mut self) -> &mut XdgShellState {
            &mut self.xdg_shell_state
        }

        fn new_toplevel(&mut self, surface: ToplevelSurface) {
            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Activated);
            });
            surface.send_configure();
            self.toplevels.push(surface);
        }

        fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
            let key = surface.wl_surface().id().protocol_id();
            self.toplevels
                .retain(|t| t.wl_surface() != surface.wl_surface());
            self.snapshot.lock().unwrap().remove(&key);
        }

        fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
            let _ = surface.send_configure();
        }

        fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

        fn reposition_request(
            &mut self,
            surface: PopupSurface,
            positioner: PositionerState,
            token: u32,
        ) {
            surface.with_pending_state(|state| {
                state.geometry = positioner.get_geometry();
                state.positioner = positioner;
            });
            surface.send_repositioned(token);
        }
    }

    impl ShmHandler for State {
        fn shm_state(&self) -> &ShmState {
            &self.shm_state
        }
    }

    impl SeatHandler for State {
        type KeyboardFocus = WlSurface;
        type PointerFocus = WlSurface;
        type TouchFocus = WlSurface;

        fn seat_state(&mut self) -> &mut SeatState<Self> {
            &mut self.seat_state
        }

        fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}
        fn cursor_image(
            &mut self,
            _seat: &Seat<Self>,
            _image: smithay::input::pointer::CursorImageStatus,
        ) {
        }
    }

    impl SelectionHandler for State {
        type SelectionUserData = ();
    }

    impl DataDeviceHandler for State {
        fn data_device_state(&self) -> &DataDeviceState {
            &self.data_device_state
        }
    }

    impl ClientDndGrabHandler for State {}
    impl ServerDndGrabHandler for State {}
    impl smithay::wayland::output::OutputHandler for State {}

    delegate_compositor!(State);
    delegate_xdg_shell!(State);
    delegate_shm!(State);
    delegate_seat!(State);
    delegate_data_device!(State);
    delegate_output!(State);

    /// Running minimal Smithay compositor handle.
    pub(crate) struct Running {
        pub(crate) socket_name: String,
        snapshot: Arc<Mutex<HashMap<u32, ToplevelInfo>>>,
        cmd: Sender<Cmd>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl Running {
        pub(crate) fn start() -> Self {
            let snapshot = Arc::new(Mutex::new(HashMap::new()));
            let stop = Arc::new(AtomicBool::new(false));
            let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
            let (ready_tx, ready_rx) = mpsc::channel::<String>();

            let snap_thread = snapshot.clone();
            let stop_thread = stop.clone();

            let thread = thread::spawn(move || {
                let mut event_loop: EventLoop<State> = EventLoop::try_new().unwrap();
                let mut display: Display<State> = Display::new().unwrap();
                let dh = display.handle();

                let compositor_state = CompositorState::new::<State>(&dh);
                let xdg_shell_state = XdgShellState::new::<State>(&dh);
                let shm_state = ShmState::new::<State>(&dh, vec![]);
                let output_manager_state = OutputManagerState::new_with_xdg_output::<State>(&dh);
                let mut seat_state = SeatState::new();
                let mut seat = seat_state.new_wl_seat(&dh, "seat0");
                let _ = seat.add_keyboard(XkbConfig::default(), 200, 25);
                let _ = seat.add_pointer();
                let data_device_state = DataDeviceState::new::<State>(&dh);

                let output = Output::new(
                    "headless-0".into(),
                    PhysicalProperties {
                        size: (0, 0).into(),
                        subpixel: Subpixel::Unknown,
                        make: "test".into(),
                        model: "headless".into(),
                    },
                );
                let mode = Mode {
                    size: (1920, 1080).into(),
                    refresh: 60_000,
                };
                output.change_current_state(Some(mode), None, None, Some((0, 0).into()));
                output.set_preferred(mode);
                let _output_global = output.create_global::<State>(&dh);

                let listener = ListeningSocketSource::new_auto().unwrap();
                let socket_name = listener.socket_name().to_string_lossy().into_owned();
                event_loop
                    .handle()
                    .insert_source(listener, |stream, _, state: &mut State| {
                        let _ = state.display_handle.insert_client(
                            stream,
                            Arc::new(ClientState {
                                compositor_state: CompositorClientState::default(),
                            }),
                        );
                    })
                    .unwrap();

                let display_fd = display.backend().poll_fd();
                // Keep display alive in the loop; dispatch on readability.
                let mut state = State {
                    display_handle: dh,
                    compositor_state,
                    xdg_shell_state,
                    shm_state,
                    _output_manager_state: output_manager_state,
                    seat_state,
                    data_device_state,
                    seat,
                    toplevels: Vec::new(),
                    snapshot: snap_thread,
                    started: Instant::now(),
                };

                let _ = ready_tx.send(socket_name);

                let fd_raw = std::os::fd::AsRawFd::as_raw_fd(&display_fd);
                let _ = event_loop.handle().insert_source(
                    Generic::new(
                        unsafe { std::os::fd::BorrowedFd::borrow_raw(fd_raw) },
                        Interest::READ,
                        LoopMode::Level,
                    ),
                    |_, _, _| Ok(PostAction::Continue),
                );

                while !stop_thread.load(Ordering::Relaxed) {
                    let _ = display.dispatch_clients(&mut state);
                    let _ = display.flush_clients();
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            Cmd::Click { title, x, y } => {
                                state.send_click(&title, x, y);
                                let _ = display.flush_clients();
                            }
                            Cmd::Stop => {
                                stop_thread.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    let _ = event_loop.dispatch(Some(Duration::from_millis(5)), &mut state);
                }
            });

            let socket_name = ready_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("compositor started");
            Self {
                socket_name,
                snapshot,
                cmd: cmd_tx,
                stop,
                thread: Some(thread),
            }
        }

        pub(crate) fn toplevels(&self) -> Vec<ToplevelInfo> {
            self.snapshot.lock().unwrap().values().cloned().collect()
        }

        pub(crate) fn wait_for_titled(
            &self,
            title: &str,
            require_buffer: bool,
            timeout: Duration,
        ) -> Option<ToplevelInfo> {
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                if let Some(info) = self.toplevels().into_iter().find(|t| {
                    t.title.as_deref() == Some(title)
                        && (!require_buffer || t.buffer_size.is_some())
                }) {
                    return Some(info);
                }
                thread::sleep(Duration::from_millis(20));
            }
            None
        }

        pub(crate) fn click(&self, title: &str, x: f64, y: f64) {
            let _ = self.cmd.send(Cmd::Click {
                title: title.to_owned(),
                x,
                y,
            });
        }
    }

    impl Drop for Running {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = self.cmd.send(Cmd::Stop);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }
}

/// The tool: starts the compositor and app, asks it, and checks.
#[cfg(target_os = "linux")]
mod tool {
    use std::{
        collections::HashMap,
        io::{BufRead, BufReader, Write},
        os::unix::{
            net::{UnixListener, UnixStream},
            process::CommandExt,
        },
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::mpsc::{Receiver, RecvTimeoutError, channel},
        time::{Duration, Instant},
    };

    use serde_json::{Value, json};

    use super::{
        app::{DIR, ROLE},
        compositor::Running,
    };

    const ANSWER: Duration = Duration::from_secs(30);

    const FIRST: [&str; 7] = [
        "box",
        "spin",
        "fade",
        "keyframes",
        "transit",
        "shadow",
        "click",
    ];

    struct App {
        child: Child,
        writer: UnixStream,
        lines: Receiver<Value>,
        surfaces: HashMap<String, Value>,
        n: u64,
        hello: Value,
        dir: PathBuf,
        sock: PathBuf,
    }

    impl App {
        fn start(
            dir: &Path,
            wayland_display: &str,
            wayland_flag: bool,
            token: Option<&str>,
        ) -> App {
            let _ = std::fs::remove_dir_all(dir);
            std::fs::create_dir_all(dir).unwrap();
            let sock = dir.join("design.sock");
            let _ = std::fs::remove_file(&sock);
            let listener = UnixListener::bind(&sock).unwrap();

            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                // Own process group so `kill(0, SIGTERM)` on design disconnect
                // stops only the child app, not this test runner.
                .process_group(0)
                .env(ROLE, "1")
                .env(DIR, dir)
                .env("NSH_DESIGN_SOCKET", &sock)
                .env("WAYLAND_DISPLAY", wayland_display)
                .env("GDK_BACKEND", "wayland")
                .env_remove("DISPLAY")
                .env_remove("NSH_DESIGN_TOKEN")
                .env_remove("NSH_DESIGN_WAYLAND")
                .stdin(Stdio::null())
                .stdout(std::fs::File::create(dir.join("stdout.log")).unwrap())
                .stderr(std::fs::File::create(dir.join("stderr.log")).unwrap());
            if wayland_flag {
                command.env("NSH_DESIGN_WAYLAND", "1");
            }
            if let Some(token) = token {
                command.env("NSH_DESIGN_TOKEN", token);
            }

            let mut child = command.spawn().unwrap();
            listener.set_nonblocking(true).unwrap();
            let deadline = Instant::now() + Duration::from_secs(120);
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if let Ok(Some(status)) = child.try_wait() {
                            panic!("the app ended before it connected ({status}): {}", log(dir));
                        }
                        assert!(Instant::now() < deadline, "the app never connected");
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(e) => panic!("accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            let writer = stream.try_clone().unwrap();
            let (tx, lines) = channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    if let Ok(value) = serde_json::from_str::<Value>(&line)
                        && tx.send(value).is_err()
                    {
                        break;
                    }
                }
            });
            let hello = lines.recv_timeout(ANSWER).expect("a hello");
            App {
                child,
                writer,
                lines,
                surfaces: HashMap::new(),
                n: 0,
                hello,
                dir: dir.to_owned(),
                sock,
            }
        }

        fn send(&mut self, message: Value) {
            writeln!(self.writer, "{message}").unwrap();
        }

        fn take(&mut self, until: Instant) -> Option<Value> {
            let left = until.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    if let Some(surface) = line.get("surface")
                        && let Some(id) = surface["id"].as_str()
                    {
                        self.surfaces.insert(id.to_owned(), surface.clone());
                    }
                    Some(line)
                }
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => {
                    panic!("the app's connection closed: {}", log(&self.dir))
                }
            }
        }

        fn ask(&mut self, step: &str, args: Value) -> Value {
            self.n += 1;
            let n = self.n;
            let mut ask = json!({ "n": n, "step": step });
            if let Value::Object(args) = args {
                ask.as_object_mut().unwrap().extend(args);
            }
            self.send(json!({ "do": ask }));
            let until = Instant::now() + ANSWER;
            loop {
                let Some(line) = self.take(until) else {
                    panic!("no answer to {step}: {}", log(&self.dir));
                };
                if let Some(done) = line.get("done")
                    && done["n"] == n
                {
                    assert!(done.get("error").is_none(), "{step}: {done}");
                    return done.clone();
                }
            }
        }

        fn drain_surfaces(&mut self) {
            while let Ok(line) = self.lines.try_recv() {
                if let Some(surface) = line.get("surface")
                    && let Some(sid) = surface["id"].as_str()
                {
                    self.surfaces.insert(sid.to_owned(), surface.clone());
                }
            }
        }

        fn surface(&mut self, id: &str) -> Value {
            let until = Instant::now() + ANSWER;
            while !self.surfaces.contains_key(id) {
                assert!(self.take(until).is_some(), "no surface {id}");
            }
            self.drain_surfaces();
            self.surfaces[id].clone()
        }

        fn picture(&mut self, window: &str, name: &str, mut options: Value) -> (Value, Vec<u8>) {
            let path = self.dir.join(format!("{name}.raw"));
            options["path"] = json!(path.to_str().unwrap());
            let answer = self.ask("picture", json!({ "window": window, "options": options }));
            let picture = answer["picture"].clone();
            assert!(picture.is_object(), "{window}: {answer}");
            let pixels = std::fs::read(&path).unwrap();
            let (w, h) = (
                picture["width"].as_u64().unwrap(),
                picture["height"].as_u64().unwrap(),
            );
            assert_eq!(pixels.len() as u64, w * h * 4, "{name}: the file's size");
            (picture, pixels)
        }

        fn finish(mut self) -> String {
            let _ = self.writer.shutdown(std::net::Shutdown::Both);
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                if let Ok(Some(_)) = self.child.try_wait() {
                    break;
                }
                if Instant::now() > deadline {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = std::fs::remove_file(&self.sock);
            log(&self.dir)
        }
    }

    fn log(dir: &Path) -> String {
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        format!("{}{}", read("stdout.log"), read("stderr.log"))
    }

    fn pixel(pixels: &[u8], width: u64, x: u64, y: u64) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        pixels[i..i + 4].try_into().unwrap()
    }

    fn surfaces(app: &mut App, comp: &Running) {
        let mut tags = Vec::new();
        for id in FIRST {
            let s = app.surface(id);
            let tag = s["wayland"]
                .as_u64()
                .unwrap_or_else(|| panic!("{id}: missing wayland handle: {s}"));
            assert!(tag >= 1, "{id}: tag must start at 1: {s}");
            assert!(!tags.contains(&tag), "duplicate wayland tag {tag}");
            tags.push(tag);

            let title = format!("nsh-design:{tag}");
            // `fade` starts at opacity 0, where GTK may commit without a buffer.
            let require_buffer = id != "fade";
            let info = comp
                .wait_for_titled(&title, require_buffer, Duration::from_secs(10))
                .unwrap_or_else(|| {
                    panic!(
                        "no mapped toplevel with title {title} (have {:?})",
                        comp.toplevels()
                    )
                });
            if require_buffer {
                let (bw, bh) = info.buffer_size.expect("shm buffer committed");
                assert!(bw > 0 && bh > 0, "{id}: empty buffer ({bw}x{bh})");
                if id == "box" {
                    assert_eq!(
                        (bw, bh),
                        (100, 80),
                        "box toplevel must be borderless 100x80"
                    );
                }
            }
        }

        // Every mapped toplevel in the compositor belongs to a reported design
        // surface (the hidden host window never maps a toplevel).
        app.drain_surfaces();
        let reported_titles: Vec<String> = app
            .surfaces
            .values()
            .filter_map(|s| s["wayland"].as_u64().map(|n| format!("nsh-design:{n}")))
            .collect();
        let all = comp.toplevels();
        assert_eq!(
            all.len(),
            reported_titles.len(),
            "mapped toplevels ({all:?}) != reported surfaces ({reported_titles:?})"
        );
        for info in &all {
            let title = info.title.as_deref().unwrap_or("");
            assert!(
                reported_titles.iter().any(|t| t == title),
                "unexpected un-reported toplevel: {info:?}"
            );
        }

        // Open and close a late window: reported with wayland tag, then gone.
        app.ask("open", json!({ "ids": ["late"] }));
        let late = app.surface("late");
        let late_tag = late["wayland"].as_u64().expect("late wayland tag");
        assert!(
            comp.wait_for_titled(
                &format!("nsh-design:{late_tag}"),
                true,
                Duration::from_secs(5),
            )
            .is_some(),
            "late toplevel never mapped"
        );
        app.ask("close", json!({ "id": "late" }));
        let until = Instant::now() + Duration::from_secs(5);
        while app.surfaces.get("late").and_then(|s| s["gone"].as_bool()) != Some(true) {
            assert!(
                app.take(until).is_some(),
                "late window never reported gone: {:?}",
                app.surfaces.get("late")
            );
        }

        // Sending `viewport` must NOT fall through to resize (regression check).
        app.send(json!({
            "viewport": { "id": "box", "x": 0, "y": 0, "width": 20, "height": 20, "scale": 4 }
        }));
        std::thread::sleep(Duration::from_millis(150));
        app.drain_surfaces();
        let box_surface = app.surface("box");
        assert_eq!(
            (
                box_surface["width"].as_f64().unwrap().round() as i32,
                box_surface["height"].as_f64().unwrap().round() as i32
            ),
            (100, 80),
            "viewport resized box: {box_surface}"
        );

        // Explicit resize updates the window size.
        app.send(json!({ "resize": { "id": "click", "width": 160, "height": 100 } }));
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let s = app.surface("click");
            let w = s["width"].as_f64().unwrap_or(0.0).round() as i32;
            let h = s["height"].as_f64().unwrap_or(0.0).round() as i32;
            if (w, h) == (160, 100) {
                break;
            }
            assert!(
                app.take(until).is_some(),
                "click window never resized to 160x100: {s}"
            );
        }
    }

    fn pictures(app: &mut App) {
        let red = [255, 0, 0, 255];
        let clear = [0, 0, 0, 0];
        let green = [0, 255, 0, 255];

        let (one, pixels) = app.picture("box", "box-1", json!({ "scale": 1 }));
        assert_eq!(
            (one["width"].as_u64(), one["height"].as_u64()),
            (Some(100), Some(80))
        );
        assert_eq!(pixel(&pixels, 100, 20, 20), red);
        assert_eq!(
            pixel(&pixels, 100, 90, 70),
            clear,
            "the background is transparent"
        );
        assert_eq!(pixel(&pixels, 100, 65, 45), green);
        let layers = one["layers"].as_array().unwrap();
        let hidden_instance = |layers: &[Value]| {
            layers.iter().any(|l| {
                l["kind"] == "component"
                    && l["key"].as_str().is_some_and(|k| k.contains("HiddenPart"))
            })
        };
        assert!(hidden_instance(layers), "{layers:?}");
        assert!(layers.iter().any(|l| l["testId"] == "hidden-part"));
        assert!(!one["modules"].as_array().unwrap().is_empty());

        let (two, pixels) = app.picture("box", "box-2", json!({ "scale": 2 }));
        assert_eq!(
            (two["width"].as_u64(), two["height"].as_u64()),
            (Some(200), Some(160))
        );
        assert_eq!(pixel(&pixels, 200, 40, 40), red);
        assert_eq!(pixel(&pixels, 200, 180, 140), clear);
        assert_eq!(pixel(&pixels, 200, 130, 90), green);

        let (part, pixels) = app.picture(
            "box",
            "box-rect",
            json!({ "scale": 1, "rect": { "x": 10, "y": 10, "width": 40, "height": 30 } }),
        );
        assert_eq!(
            (part["width"].as_u64(), part["height"].as_u64()),
            (Some(40), Some(30))
        );
        assert!(
            pixels.as_chunks::<4>().0.iter().all(|p| *p == red),
            "the rect is the red box"
        );

        let (hidden, _) = app.picture(
            "box",
            "box-hide",
            json!({ "scale": 1, "hide": "HiddenPart" }),
        );
        assert!(!hidden_instance(hidden["layers"].as_array().unwrap()));
    }

    fn settle(app: &mut App) {
        let ms = |v: &Value| v["ms"].as_f64().unwrap();
        let quiet = app.ask("settleQuiet", json!({}));
        assert_eq!(quiet["timedOut"], false, "{quiet}");
        assert!((45.0..500.0).contains(&ms(&quiet)), "{quiet}");

        let timer = app.ask("settleTimer", json!({}));
        assert_eq!(timer["timedOut"], false, "{timer}");
        assert!(
            ms(&timer) >= 75.0,
            "settled before the timer's batch: {timer}"
        );

        let animation = app.ask("settleAnimation", json!({}));
        assert_eq!(animation["timedOut"], false, "{animation}");
        assert!(
            (300.0..1000.0).contains(&ms(&animation)),
            "settled during the animation: {animation}"
        );

        let transition = app.ask("settleTransition", json!({}));
        assert_eq!(transition["timedOut"], false, "{transition}");
        assert!(
            (300.0..1000.0).contains(&ms(&transition)),
            "settled during the style transition: {transition}"
        );

        let busy = app.ask("settleBusy", json!({}));
        assert_eq!(busy["timedOut"], true, "{busy}");
        assert!((990.0..1600.0).contains(&ms(&busy)), "{busy}");
    }

    fn loops(app: &mut App) {
        let (_, first) = app.picture("spin", "spin-1", json!({ "scale": 1 }));
        std::thread::sleep(Duration::from_millis(330));
        let (_, second) = app.picture("spin", "spin-2", json!({ "scale": 1 }));
        assert!(first == second, "an endless loop drawn twice differs");
        let [r, g, b, a] = pixel(&first, 60, 30, 30);
        assert_eq!((r, g), (0, 0));
        assert!((40..=65).contains(&a) && b == a, "{:?}", [r, g, b, a]);
    }

    fn keyframes(app: &mut App) {
        let settled = app.ask("settleKeyframes", json!({}));
        assert_eq!(settled["timedOut"], false, "{settled}");
        assert!(
            settled["ms"].as_f64().unwrap() < 300.0,
            "endless keyframes kept the window busy: {settled}"
        );
        let (_, first) = app.picture("keyframes", "keyframes-1", json!({ "scale": 1 }));
        std::thread::sleep(Duration::from_millis(530));
        let (_, second) = app.picture("keyframes", "keyframes-2", json!({ "scale": 1 }));
        assert!(first == second, "endless keyframes drawn twice differ");
        assert_eq!(pixel(&first, 60, 30, 30), [0, 0, 255, 255]);
    }

    fn shadows(app: &mut App) {
        let (_, pixels) = app.picture("shadow", "shadow", json!({ "scale": 1 }));
        assert_eq!(pixel(&pixels, 120, 60, 60), [255, 255, 255, 255]);
        assert_eq!(pixel(&pixels, 120, 2, 2), [0, 0, 0, 0]);
        let below = pixel(&pixels, 120, 60, 96);
        assert!(below[3] > 0, "no shadow below the box: {below:?}");
    }

    fn codes(app: &mut App) {
        let dir = app.dir.to_str().unwrap().to_owned();
        let codes = app.ask("codes", json!({ "dir": dir }));
        assert_eq!(
            codes["pictureWindow"], "ERR_NSH_DESIGN_NO_WINDOW",
            "{codes}"
        );
        assert_eq!(
            codes["settledWindow"], "ERR_NSH_DESIGN_NO_WINDOW",
            "{codes}"
        );
        assert_eq!(codes["outside"], "ERR_NSH_DESIGN_RECT_OUTSIDE", "{codes}");
        assert_eq!(codes["path"], "ERR_NSH_DESIGN_PATH", "{codes}");
        assert_eq!(codes["scale"], "ERR_INVALID_ARG_VALUE", "{codes}");
        assert_eq!(codes["relative"], "ERR_INVALID_ARG_VALUE", "{codes}");
    }

    fn wayland_input(app: &mut App, comp: &Running) {
        let s = app.surface("click");
        let tag = s["wayland"].as_u64().expect("click wayland tag");
        let title = format!("nsh-design:{tag}");
        assert!(
            comp.wait_for_titled(&title, true, Duration::from_secs(5))
                .is_some(),
            "click toplevel not mapped"
        );
        let before = app.ask("clicks", json!({}))["clicks"].as_u64().unwrap();
        comp.click(&title, 40.0, 40.0);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let now = app.ask("clicks", json!({}))["clicks"].as_u64().unwrap();
            if now > before {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Wayland click never reached pressable view (clicks={now})"
            );
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    pub(crate) fn run() {
        // Ensure XDG_RUNTIME_DIR exists for Smithay's listening socket.
        let base = std::env::temp_dir().join(format!("nsh-design-wayland-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        if std::env::var_os("XDG_RUNTIME_DIR").is_none() {
            // SAFETY: single-threaded test startup before spawning the compositor.
            unsafe { std::env::set_var("XDG_RUNTIME_DIR", &base) };
        }

        let filter: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with('-'))
            .collect();
        let wanted = |name: &str| filter.is_empty() || filter.iter().any(|f| name.contains(f));
        let mut ran = 0usize;

        if wanted("hello") {
            let comp = Running::start();
            let token = "secret-design-wayland-token-918273645";
            let app = App::start(
                &base.join("hello-tok"),
                &comp.socket_name,
                true,
                Some(token),
            );
            assert_eq!(app.hello, json!({ "hello": { "token": token } }));
            let log_text = app.finish();
            assert!(
                !log_text.contains(token),
                "the design token leaked into logs"
            );

            let app = App::start(&base.join("hello-plain"), &comp.socket_name, true, None);
            assert_eq!(app.hello, json!({ "hello": {} }));
            let _ = app.finish();
            println!("design_wayland: hello: ok");
            ran += 1;
        }

        if wanted("no_wayland_env") {
            let comp = Running::start();
            let mut app = App::start(&base.join("no-wl"), &comp.socket_name, false, None);
            app.ask("open", json!({ "ids": ["box"] }));
            std::thread::sleep(Duration::from_millis(250));
            app.drain_surfaces();
            assert!(
                app.surfaces.is_empty(),
                "surfaces reported without NSH_DESIGN_WAYLAND=1: {:?}",
                app.surfaces
            );
            let log_text = app.finish();
            assert!(
                log_text.contains("NSH_DESIGN_WAYLAND"),
                "expected one warning about NSH_DESIGN_WAYLAND, got:\n{log_text}"
            );
            println!("design_wayland: no_wayland_env: ok");
            ran += 1;
        }

        let main_cases = [
            "surfaces",
            "pictures",
            "settle",
            "loops",
            "keyframes",
            "shadows",
            "codes",
            "wayland_input",
        ];
        if main_cases.iter().any(|c| wanted(c)) {
            let comp = Running::start();
            let mut app = App::start(&base.join("main"), &comp.socket_name, true, None);
            app.ask("open", json!({ "ids": FIRST }));

            if wanted("surfaces") {
                surfaces(&mut app, &comp);
                println!("design_wayland: surfaces: ok");
                ran += 1;
            }
            if wanted("pictures") {
                pictures(&mut app);
                println!("design_wayland: pictures: ok");
                ran += 1;
            }
            if wanted("settle") {
                settle(&mut app);
                println!("design_wayland: settle: ok");
                ran += 1;
            }
            if wanted("loops") {
                loops(&mut app);
                println!("design_wayland: loops: ok");
                ran += 1;
            }
            if wanted("keyframes") {
                keyframes(&mut app);
                println!("design_wayland: keyframes: ok");
                ran += 1;
            }
            if wanted("shadows") {
                shadows(&mut app);
                println!("design_wayland: shadows: ok");
                ran += 1;
            }
            if wanted("codes") {
                codes(&mut app);
                println!("design_wayland: codes: ok");
                ran += 1;
            }
            if wanted("wayland_input") {
                wayland_input(&mut app, &comp);
                println!("design_wayland: wayland_input: ok");
                ran += 1;
            }
            let _ = app.finish();
        }

        let _ = std::fs::remove_dir_all(&base);
        println!("design_wayland: {ran} cases ok");
    }
}
