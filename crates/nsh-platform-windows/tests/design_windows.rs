//! A design tool's view of an app on Windows (`nsh/design`,
//! `docs/dev/design-mode.md`): the test plays the tool. It listens on the
//! loopback address and starts this binary again as the app
//! (`DESIGN_WINDOWS_APP`), a small JS app run by the real Windows host in
//! design mode, with real Win32 windows. The tool asks the app for what each
//! case checks (`{"do":{…}}`, answered `{"done":{…}}`), sends it input as a
//! tool does, and reads the windows from its own process (styles, places,
//! the foreground window). Two components of the test's own run in the
//! app's UI thread: a probe that watches every `WM_WINDOWPOSCHANGING` and
//! `WM_WINDOWPOSCHANGED` of the app's windows (`WH_CALLWNDPROCRET`), stands a
//! virtual screen in and reads what a window presents, and a composed
//! component that notes where it is placed.
//!
//! - `hello`: with `NSH_DESIGN_TOKEN` the hello carries the token, without
//!   it the hello is empty; the app ran at NSH's most verbose log level and
//!   its log holds no trace of the token.
//! - `pictures`, `settle`, `loops`, `shadows`, `codes`: `design.picture`
//!   and `design.settled` as on macOS: a coloured box on a transparent
//!   background at scale 1 and 2, a `rect`, a `hide`; settling after a
//!   timer's batch, during a 300 ms animation, and given up at 1 s; an
//!   endless loop drawn twice at its first frame; an outer shadow; the codes.
//! - `out-of-sight`, `foreground`, `display-change`, `large-window`: no
//!   design window on a display at any moment (the place checked in every
//!   message that changed a size), tool windows that never activate, the
//!   main window hidden; a window opening leaves the foreground window as it
//!   is; `WM_DISPLAYCHANGE` places them again; a 16,384 x 1,000-point window
//!   at 150 % (a stood-in DPI, which sizes the window and its swap chain)
//!   lays out, presents at most the largest texture and gives a picture.
//! - `keys-typed`, `keys-control`, `keys-named`: every printable ASCII key
//!   and a non-ASCII character typed into a field, Control and Alt chords
//!   typing nothing, every named key at a key-logging view, an unlisted name
//!   dropped.
//! - `composed`: a composed component whose box a transform takes out of its
//!   clip stays placed; one whose view clips its content does not.
//!
//! Without libtest's harness: arguments pick cases (none runs all), each
//! prints `design_windows: <case>: ok`, and the last line says how many ran.

#[cfg(windows)]
fn main() {
    if std::env::var_os(app::ROLE).is_some() {
        app::run();
    } else {
        tool::run();
    }
}

#[cfg(not(windows))]
fn main() {}

/// The app's script.
#[cfg(windows)]
const APP: &str = r##"import { Animated, createAnimatedValue, mount, nativeComponent, signal, Text, View, type KeyEvent, type NativeComponentRef, type ViewRef } from "nsh";
import { Input } from "nsh/base";
import { Window } from "nsh/desktop";
import { design } from "nsh/design";

const WINDOW = { frame: "none", taskbar: false, activate: false, shadow: false, transparent: true, sizeToContent: true } as const;

const Probe = nativeComponent<{}, { report: Record<string, unknown> }>("test.probe", { props: [], events: ["report"] });
const Composed = nativeComponent<{ name: string }, {}>("test.composed", { props: ["name"], events: [] });

const tick = signal(0);
const faded = signal(1);
const value = signal("");
const shift = signal(0);
const log: string[] = [];
let probe: NativeComponentRef | undefined;
let logView: ViewRef | undefined;
let reported: ((r: unknown) => void) | undefined;

mount(() => (
  <View>
    <Text>{`tick ${tick()}`}</Text>
    <Probe ref={(r) => (probe = r)} style={{ width: 1, height: 1 }} onReport={(e) => reported?.(e)} />
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

function note(e: KeyEvent, d: string) {
  const m = e.modifiers;
  log.push(`${d} ${e.key} ${m.shift ? "s" : ""}${m.control ? "c" : ""}${m.alt ? "a" : ""} ${e.repeat ? "r" : ""}`);
}

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
  composed: () => (
    <View style={{ width: 200, height: 100, flexDirection: "row" }}>
      <View style={{ width: 100, height: 100, overflow: "hidden" }}>
        <View style={{ transform: [{ translateX: shift() }] }}>
          <Composed name="open" style={{ width: 1, height: 1 }} />
        </View>
      </View>
      <View style={{ width: 100, height: 100, overflow: "hidden" }}>
        <View style={{ transform: [{ translateX: shift() }] }}>
          <Composed name="clipped" style={{ width: 1, height: 1, overflow: "hidden" }} />
        </View>
      </View>
    </View>
  ),
  type: () => (
    <View style={{ width: 300, height: 40 }}>
      <Input autoFocus value={value()} onChange={({ value: v }) => value.set(v)} style={{ width: 300, height: 40 }} />
    </View>
  ),
  log: () => (
    <View
      ref={(r) => (logView = r)}
      focusable
      captureKeys="all"
      onKeyDown={(e) => note(e, "d")}
      onKeyUp={(e) => note(e, "u")}
      style={{ width: 100, height: 100, backgroundColor: "#eeeeee" }}
    />
  ),
  late: () => <View style={{ width: 50, height: 50, backgroundColor: "#888888" }} />,
  big: () => (
    <View style={{ width: 16384, height: 1000, backgroundColor: "#202020" }}>
      <View style={{ marginLeft: 16300, marginTop: 10, width: 50, height: 50, backgroundColor: "#ff0000" }} />
    </View>
  ),
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
    for (const id of m.ids) windows[id] = Window.open(contents[id] as any, { id, title: id, ...WINDOW });
    for (const id of m.ids) await opened(id);
    return {};
  },
  close: async (m) => {
    windows[m.id]?.close();
    delete windows[m.id];
    await wait(100);
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
  value: async () => ({ value: value() }),
  focusLog: async () => {
    logView?.focus();
    await wait(100);
    return {};
  },
  log: async () => ({ log: [...log] }),
  pan: async (m) => {
    shift.set(m.x);
    return {};
  },
  probe: (m) =>
    new Promise((resolve) => {
      reported = resolve;
      probe?.command(m.command, m.args ?? {});
    }),
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

/// The app: the Windows host running [`APP`] in design mode, with the
/// test's probe and composed component.
#[cfg(windows)]
mod app {
    use std::{cell::RefCell, collections::BTreeMap, path::PathBuf, rc::Rc};

    use nsh_platform_windows::{
        AppOptions, AppSource, Bundle, ComponentContext, Components, NativeComponent, Placement,
        dev, run_app, set_ui_sets,
        windows::{
            Win32::{
                Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
                Graphics::{
                    DirectComposition::{IDCompositionDesktopDevice, IDCompositionVisual2},
                    Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect},
                },
                System::Threading::GetCurrentThreadId,
                UI::WindowsAndMessaging::{
                    CWPRETSTRUCT, CallNextHookEx, GWL_STYLE, GetClassNameW, GetWindowLongPtrW,
                    GetWindowRect, HHOOK, IsWindowVisible, SWP_NOMOVE, SWP_NOSIZE,
                    SetWindowsHookExW, WH_CALLWNDPROCRET, WINDOWPOS, WM_WINDOWPOSCHANGED,
                    WM_WINDOWPOSCHANGING, WS_CHILD,
                },
            },
            core::Interface,
        },
    };
    use serde_json::{Value, json};

    /// Set in the app's environment: this process is the app.
    pub(crate) const ROLE: &str = "DESIGN_WINDOWS_APP";
    /// The folder the app's files go to.
    pub(crate) const DIR: &str = "DESIGN_WINDOWS_DIR";
    /// The DPI stood in for every window's, if any.
    pub(crate) const DPI: &str = "DESIGN_WINDOWS_DPI";

    /// What the probe saw of the app's windows.
    #[derive(Default)]
    struct Watch {
        /// `WM_WINDOWPOSCHANGING`s that changed a window's size, checked.
        sized: u32,
        /// Places of a window on a display: what must never be.
        seen: Vec<String>,
        /// The virtual screen stood in, if any.
        screen: Option<RECT>,
        /// The composed components: whether each is placed now, and ever was.
        composed: BTreeMap<String, (bool, bool)>,
    }

    thread_local! {
        static WATCH: RefCell<Watch> = RefCell::default();
        static HOOK: std::cell::Cell<Option<HHOOK>> = const { std::cell::Cell::new(None) };
    }

    /// Whether `rect` (screen pixels) lies on a display: on one of the
    /// system's, or in the virtual screen stood in.
    fn on_a_display(rect: &RECT) -> bool {
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return false;
        }
        if let Some(s) = WATCH.with_borrow(|w| w.screen) {
            return rect.left < s.right
                && s.left < rect.right
                && rect.top < s.bottom
                && s.top < rect.bottom;
        }
        // SAFETY: a plain query.
        !unsafe { MonitorFromRect(rect, MONITOR_DEFAULTTONULL) }.is_invalid()
    }

    /// A top-level window of the host's (its class's).
    fn hosts(hwnd: HWND) -> bool {
        let mut name = [0u16; 64];
        // SAFETY: plain queries into a local buffer.
        let n = unsafe { GetClassNameW(hwnd, &mut name) };
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
        String::from_utf16_lossy(&name[..n.max(0) as usize]) == "NshWindow"
            && style & WS_CHILD.0 == 0
    }

    /// After each message a window of the UI thread handled: the places a
    /// change of a window's place or size left it in.
    unsafe extern "system" fn after(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            // SAFETY: the hook's `lparam` is the message's CWPRETSTRUCT.
            let ret = unsafe { &*(lparam.0 as *const CWPRETSTRUCT) };
            if (ret.message == WM_WINDOWPOSCHANGING || ret.message == WM_WINDOWPOSCHANGED)
                && hosts(ret.hwnd)
            {
                // SAFETY: the message's WINDOWPOS, as the window procedure
                // left it.
                let pos = unsafe { &*(ret.lParam.0 as *const WINDOWPOS) };
                let mut now = RECT::default();
                // SAFETY: a query with a local out value.
                let _ = unsafe { GetWindowRect(ret.hwnd, &mut now) };
                if ret.message == WM_WINDOWPOSCHANGING {
                    // Windows keeps an origin within 16 bits.
                    let (x, y) = if pos.flags.contains(SWP_NOMOVE) {
                        (now.left, now.top)
                    } else {
                        (pos.x.clamp(-32_768, 32_767), pos.y.clamp(-32_768, 32_767))
                    };
                    let (cx, cy) = if pos.flags.contains(SWP_NOSIZE) {
                        (now.right - now.left, now.bottom - now.top)
                    } else {
                        (pos.cx, pos.cy)
                    };
                    let rect = RECT {
                        left: x,
                        top: y,
                        right: x + cx,
                        bottom: y + cy,
                    };
                    let sized = !pos.flags.contains(SWP_NOSIZE);
                    WATCH.with_borrow_mut(|w| w.sized += u32::from(sized));
                    if on_a_display(&rect) {
                        WATCH.with_borrow_mut(|w| {
                            w.seen
                                .push(format!("changing to {rect:?} (sized: {sized})"))
                        });
                    }
                } else if unsafe { IsWindowVisible(ret.hwnd) }.as_bool() && on_a_display(&now) {
                    WATCH.with_borrow_mut(|w| w.seen.push(format!("shown at {now:?}")));
                }
            }
        }
        // SAFETY: passes the message on.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    /// The probe: watches the windows of the UI thread from when it is
    /// made, and answers the tool's questions through its app.
    struct Probe(ComponentContext);

    impl Probe {
        fn new(cx: ComponentContext) -> Probe {
            if HOOK.get().is_none() {
                // SAFETY: a hook of this thread, with a procedure of this
                // process, for the process's life.
                let hook = unsafe {
                    SetWindowsHookExW(WH_CALLWNDPROCRET, Some(after), None, GetCurrentThreadId())
                };
                HOOK.set(hook.ok());
            }
            Probe(cx)
        }
    }

    impl NativeComponent for Probe {
        fn set_prop(&mut self, _name: &str, _value: Option<&Value>) {}

        fn command(&mut self, name: &str, args: &Value) {
            match name {
                "screen" => {
                    let n = |k: &str| args[k].as_i64().map(|v| v as i32);
                    let rect = match (n("left"), n("top"), n("right"), n("bottom")) {
                        (Some(left), Some(top), Some(right), Some(bottom)) => Some(RECT {
                            left,
                            top,
                            right,
                            bottom,
                        }),
                        _ => None,
                    };
                    WATCH.with_borrow_mut(|w| w.screen = rect);
                    dev::design::stand_in_screen(rect);
                    self.0.emit("report", json!({}));
                }
                _ => {
                    let presented: Vec<Value> = args["hwnds"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|h| {
                            let hwnd = HWND(h.as_i64().unwrap_or(0) as isize as *mut _);
                            json!(dev::design::presented_size(hwnd))
                        })
                        .collect();
                    let report = WATCH.with_borrow(|w| {
                        json!({
                            "hooked": HOOK.get().is_some(),
                            "sized": w.sized,
                            "seen": w.seen,
                            "composed": w.composed.iter().map(|(name, (now, ever))| {
                                (name.clone(), json!({ "placed": now, "ever": ever }))
                            }).collect::<serde_json::Map<_, _>>(),
                            "presented": presented,
                            "largest": dev::design::largest_texture(),
                        })
                    });
                    self.0.emit("report", report);
                }
            }
        }
    }

    /// A composed component with a visual of its own: notes where it is
    /// placed.
    #[derive(Default)]
    struct Composed {
        name: String,
        visual: Option<IDCompositionVisual2>,
    }

    impl NativeComponent for Composed {
        fn composed(&self) -> bool {
            true
        }

        fn visual(&mut self, device: &IDCompositionDesktopDevice) -> Option<IDCompositionVisual2> {
            // SAFETY: a COM call on the window's live device.
            let visual = unsafe { device.CreateVisual() }.ok()?;
            let visual: IDCompositionVisual2 = visual.cast().ok()?;
            self.visual = Some(visual.clone());
            Some(visual)
        }

        fn placed(&mut self, at: Option<Placement>) {
            let name = self.name.clone();
            WATCH.with_borrow_mut(|w| {
                let entry = w.composed.entry(name).or_default();
                entry.0 = at.is_some();
                entry.1 |= at.is_some();
            });
        }

        fn set_prop(&mut self, name: &str, value: Option<&Value>) {
            if name == "name" {
                self.name = value.and_then(Value::as_str).unwrap_or("").to_owned();
            }
        }
    }

    pub(crate) fn run() {
        dev::init_logging();
        // NSH's most verbose: nothing it logs may hold the run's token.
        log::set_max_level(log::LevelFilter::Trace);
        if let Some(dpi) = std::env::var(DPI).ok().and_then(|d| d.parse().ok()) {
            dev::design::stand_in_dpi(Some(dpi));
        }
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
        let components = Components::default();
        components.register("test.probe", Rc::new(|cx| Box::new(Probe::new(cx))));
        components.register("test.composed", Rc::new(|_| Box::<Composed>::default()));
        let options = AppOptions {
            app_id: "dev.nsh.test.design-windows".into(),
            title: "design windows".into(),
            size: (200, 100),
            components,
            ..AppOptions::default()
        };
        let code = run_app(AppSource::Bundle(bundle), options);
        std::process::exit(if code == std::process::ExitCode::SUCCESS {
            0
        } else {
            1
        });
    }
}

/// The tool: starts the app, asks it and checks.
#[cfg(windows)]
mod tool {
    use std::{
        collections::HashMap,
        io::{BufRead, BufReader, Write},
        net::{Shutdown, TcpListener, TcpStream},
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::mpsc::{Receiver, RecvTimeoutError, channel},
        time::{Duration, Instant},
    };

    use nsh_platform_windows::windows::Win32::{
        Foundation::{HWND, LPARAM, RECT, WPARAM},
        Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect},
        UI::WindowsAndMessaging::{
            EnumWindows, GWL_EXSTYLE, GetClassNameW, GetClientRect, GetForegroundWindow,
            GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
            IsWindowVisible, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN, SendMessageW, WM_DISPLAYCHANGE, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW,
        },
    };
    use serde_json::{Value, json};

    use super::app::{DIR, DPI, ROLE};

    /// How long an answer may take.
    const ANSWER: Duration = Duration::from_secs(30);

    /// The windows the app opens first.
    const FIRST: [&str; 8] = [
        "box",
        "spin",
        "fade",
        "keyframes",
        "transit",
        "shadow",
        "composed",
        "type",
    ];

    /// A running app, as its tool sees it.
    struct App {
        child: Child,
        writer: TcpStream,
        lines: Receiver<Value>,
        /// The last `surface` line of each window.
        surfaces: HashMap<String, Value>,
        n: u64,
        hello: Value,
        dir: PathBuf,
    }

    impl App {
        /// Starts the app in `dir` with `token` and a stood-in `dpi`.
        fn start(dir: &Path, token: Option<&str>, dpi: Option<u32>) -> App {
            let _ = std::fs::remove_dir_all(dir);
            std::fs::create_dir_all(dir).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .env(ROLE, "1")
                .env(DIR, dir)
                .env("NSH_DESIGN_SOCKET", format!("tcp:127.0.0.1:{port}"))
                .env_remove("NSH_DESIGN_TOKEN")
                .stdin(Stdio::null())
                .stdout(std::fs::File::create(dir.join("stdout.log")).unwrap())
                .stderr(std::fs::File::create(dir.join("stderr.log")).unwrap());
            if let Some(token) = token {
                command.env("NSH_DESIGN_TOKEN", token);
            }
            if let Some(dpi) = dpi {
                command.env(DPI, dpi.to_string());
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
            }
        }

        /// Sends `message` as the tool does.
        fn send(&mut self, message: Value) {
            writeln!(self.writer, "{message}").unwrap();
        }

        /// Takes a line of the app's, keeping what windows it tells of.
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

        /// Asks the app's script to run `step` with `args`, and its answer.
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

        /// Window `id` as the app last told of it, once it has.
        fn surface(&mut self, id: &str) -> Value {
            let until = Instant::now() + ANSWER;
            while !self.surfaces.contains_key(id) {
                assert!(self.take(until).is_some(), "no surface {id}");
            }
            // Lines told since.
            while let Ok(line) = self.lines.try_recv() {
                if let Some(surface) = line.get("surface")
                    && let Some(sid) = surface["id"].as_str()
                {
                    self.surfaces.insert(sid.to_owned(), surface.clone());
                }
            }
            self.surfaces[id].clone()
        }

        /// The native window of window `id`.
        fn hwnd(&mut self, id: &str) -> HWND {
            let surface = self.surface(id);
            HWND(surface["hwnd"].as_u64().expect("an hwnd") as isize as *mut _)
        }

        /// The probe's report, with the swap chain sizes of `hwnds`.
        fn report(&mut self, hwnds: &[HWND]) -> Value {
            let hwnds: Vec<i64> = hwnds.iter().map(|h| h.0 as isize as i64).collect();
            self.ask(
                "probe",
                json!({ "command": "report", "args": { "hwnds": hwnds } }),
            )
        }

        /// A picture of `window` with `options` (its `path` in the app's
        /// folder, named `name`): its answer and pixels.
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

        /// Closes the connection, as a tool that goes: the app ends. Its log.
        fn finish(mut self) -> String {
            let _ = self.writer.shutdown(Shutdown::Both);
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                if let Ok(Some(_)) = self.child.try_wait() {
                    break;
                }
                if Instant::now() > deadline {
                    let _ = self.child.kill();
                    panic!("the app did not end when its tool went");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            log(&self.dir)
        }
    }

    /// What the app wrote out and logged.
    fn log(dir: &Path) -> String {
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        format!("{}{}", read("stdout.log"), read("stderr.log"))
    }

    /// Pixel `(x, y)` of an RGBA picture `width` wide.
    fn pixel(pixels: &[u8], width: u64, x: u64, y: u64) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        pixels[i..i + 4].try_into().unwrap()
    }

    /// The virtual screen (every display's rectangle's union).
    fn virtual_screen() -> RECT {
        // SAFETY: plain queries.
        unsafe {
            let (x, y) = (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
            );
            RECT {
                left: x,
                top: y,
                right: x + GetSystemMetrics(SM_CXVIRTUALSCREEN),
                bottom: y + GetSystemMetrics(SM_CYVIRTUALSCREEN),
            }
        }
    }

    fn window_rect(hwnd: HWND) -> RECT {
        let mut rect = RECT::default();
        // SAFETY: a query of another process's window with a local out
        // value.
        unsafe { GetWindowRect(hwnd, &mut rect) }.unwrap();
        rect
    }

    /// Whether `hwnd`'s rectangle lies on no display.
    fn on_no_display(hwnd: HWND) -> bool {
        let rect = window_rect(hwnd);
        // SAFETY: a plain query.
        unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONULL) }.is_invalid()
    }

    /// The process of `hwnd`.
    fn process_of(hwnd: HWND) -> u32 {
        let mut pid = 0;
        // SAFETY: a plain query with a local out value.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        pid
    }

    /// The top-level windows of process `pid`.
    fn windows_of(pid: u32) -> Vec<HWND> {
        unsafe extern "system" fn each(
            hwnd: HWND,
            lparam: LPARAM,
        ) -> nsh_platform_windows::windows::core::BOOL {
            // SAFETY: `lparam` is the list `windows_of` passed.
            let found = unsafe { &mut *(lparam.0 as *mut (u32, Vec<HWND>)) };
            if process_of(hwnd) == found.0 {
                found.1.push(hwnd);
            }
            true.into()
        }
        let mut found = (pid, Vec::new());
        // SAFETY: the callback only touches `found`, alive for the call.
        let _ = unsafe { EnumWindows(Some(each), LPARAM(&raw mut found as isize)) };
        found.1
    }

    fn class_of(hwnd: HWND) -> String {
        let mut name = [0u16; 64];
        // SAFETY: a plain query into a local buffer.
        let n = unsafe { GetClassNameW(hwnd, &mut name) };
        String::from_utf16_lossy(&name[..n.max(0) as usize])
    }

    /// `hwnd` of the app kept out of sight: a tool window that never
    /// activates, shown on no display.
    fn kept_out_of_sight(id: &str, hwnd: HWND) {
        // SAFETY: a plain query of another process's window.
        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        assert!(ex & WS_EX_TOOLWINDOW.0 != 0, "{id}: not a tool window");
        assert!(ex & WS_EX_NOACTIVATE.0 != 0, "{id}: activates");
        assert!(ex & WS_EX_APPWINDOW.0 == 0, "{id}: has a taskbar button");
        // SAFETY: a plain query.
        assert!(
            unsafe { IsWindowVisible(hwnd) }.as_bool(),
            "{id}: not shown"
        );
        assert!(
            on_no_display(hwnd),
            "{id}: on a display at {:?}",
            window_rect(hwnd)
        );
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
        // At its first frame: an opacity of 0.2.
        let [r, g, b, a] = pixel(&first, 60, 30, 30);
        assert_eq!((r, g), (0, 0));
        assert!((40..=65).contains(&a) && b == a, "{:?}", [r, g, b, a]);
    }

    /// A view's keyframes repeating for good (`animate-pulse`): its window
    /// settles beside them, and its pictures show their first frame, alike.
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
        // A pulse's first frame: its full opacity.
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

    /// Every window of the app's own out of sight, the main one hidden, no
    /// window of the app on a display, and no place of one on a display in
    /// any message that changed its place or size.
    fn out_of_sight(app: &mut App) {
        let mut hwnds = Vec::new();
        for id in FIRST {
            let hwnd = app.hwnd(id);
            kept_out_of_sight(id, hwnd);
            hwnds.push(hwnd);
        }
        let pid = app.child.id();
        let mut hidden_host = false;
        for hwnd in windows_of(pid) {
            // SAFETY: a plain query.
            if unsafe { IsWindowVisible(hwnd) }.as_bool() {
                assert!(
                    app.surfaces
                        .values()
                        .any(|s| s["hwnd"].as_u64() == Some(hwnd.0 as u64)),
                    "a window of the app's shows that is not one of its own ({})",
                    class_of(hwnd)
                );
                assert!(on_no_display(hwnd), "{:?} on a display", window_rect(hwnd));
            } else if class_of(hwnd) == "NshWindow" {
                hidden_host = true;
            }
        }
        assert!(hidden_host, "the main window is not there, hidden");
        let report = app.report(&hwnds);
        assert_eq!(report["hooked"], true);
        assert!(
            report["sized"].as_u64().unwrap() >= FIRST.len() as u64,
            "{report}"
        );
        assert_eq!(
            report["seen"],
            json!([]),
            "a design window was on a display"
        );
    }

    fn foreground(app: &mut App) {
        // SAFETY: plain queries.
        let before = unsafe { GetForegroundWindow() };
        app.ask("open", json!({ "ids": ["late"] }));
        let late = app.hwnd("late");
        std::thread::sleep(Duration::from_millis(200));
        // SAFETY: as above.
        let after = unsafe { GetForegroundWindow() };
        assert_eq!(before, after, "a window opening took the foreground");
        if !after.is_invalid() {
            assert_ne!(process_of(after), app.child.id());
        }
        kept_out_of_sight("late", late);
        let report = app.report(&[]);
        assert_eq!(report["seen"], json!([]));
    }

    fn display_change(app: &mut App) {
        let hwnds: Vec<(&str, HWND)> = FIRST.iter().map(|id| (*id, app.hwnd(id))).collect();
        let real = virtual_screen();
        // A display to the left now covers where the windows are.
        let leftmost = hwnds
            .iter()
            .map(|(_, h)| window_rect(*h).left)
            .min()
            .unwrap();
        let screen = RECT {
            left: leftmost - 1000,
            top: real.top - 1000,
            right: real.right,
            bottom: real.bottom + 1000,
        };
        app.ask(
            "probe",
            json!({ "command": "screen", "args": {
                "left": screen.left, "top": screen.top, "right": screen.right, "bottom": screen.bottom,
            } }),
        );
        for (id, hwnd) in &hwnds {
            assert!(
                window_rect(*hwnd).left >= screen.left,
                "{id} moved before the display change"
            );
        }
        let notify = |hwnd: HWND| {
            // SAFETY: a message with no pointers to another process's
            // window, as the system broadcasts it.
            unsafe { SendMessageW(hwnd, WM_DISPLAYCHANGE, Some(WPARAM(32)), Some(LPARAM(0))) };
        };
        notify(hwnds[0].1);
        for (id, hwnd) in &hwnds {
            let rect = window_rect(*hwnd);
            assert!(
                rect.right <= screen.left,
                "{id} not placed again for the displays: {rect:?}"
            );
        }
        // Back to the system's displays.
        app.ask("probe", json!({ "command": "screen", "args": {} }));
        notify(hwnds[0].1);
        for (id, hwnd) in &hwnds {
            let rect = window_rect(*hwnd);
            assert!(rect.right <= real.left, "{id}: {rect:?}");
            assert!(rect.right > screen.left, "{id} not placed again: {rect:?}");
        }
        let report = app.report(&[]);
        assert_eq!(report["seen"], json!([]));
    }

    /// The key message a tool sends.
    fn key(app: &mut App, window: &str, name: &str, down: bool, modifiers: &str, repeat: bool) {
        app.send(json!({ "key": {
            "id": window,
            "down": down,
            "key": name,
            "shift": modifiers.contains('s'),
            "control": modifiers.contains('c'),
            "alt": modifiers.contains('a'),
            "meta": false,
            "repeat": repeat,
        } }));
    }

    /// Asks `step` until `done` says its answer is the one, at most 10 s.
    fn until(app: &mut App, step: &str, done: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let answer = app.ask(step, json!({}));
            if done(&answer) || Instant::now() > deadline {
                return answer;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Whether the US layout types `c` with Shift.
    fn shifted(c: char) -> bool {
        c.is_ascii_uppercase() || "~!@#$%^&*()_+{}|:\"<>?".contains(c)
    }

    fn keys_typed(app: &mut App) {
        assert_eq!(app.ask("value", json!({}))["value"], "");
        let mut typed = String::new();
        for c in (b' '..=b'~').map(char::from).chain(['é']) {
            let name = c.to_string();
            let modifiers = if shifted(c) { "s" } else { "" };
            key(app, "type", &name, true, modifiers, false);
            key(app, "type", &name, false, modifiers, false);
            typed.push(c);
        }
        let value = until(app, "value", |v| v["value"] == typed.as_str());
        assert_eq!(value["value"], typed.as_str());
    }

    fn keys_control(app: &mut App) {
        let before = app.ask("value", json!({}))["value"].clone();
        for modifiers in ["c", "a"] {
            key(app, "type", "j", true, modifiers, false);
            key(app, "type", "j", false, modifiers, false);
        }
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            app.ask("value", json!({}))["value"],
            before,
            "a chord typed"
        );
    }

    /// The named keys a design tool sends, with the modifiers and repeat
    /// some are sent with.
    const NAMED: [(&str, &str, bool); 26] = [
        (" ", "", false),
        ("Enter", "", false),
        ("Tab", "", false),
        ("Backspace", "", false),
        ("Delete", "", false),
        ("Escape", "", false),
        ("ArrowUp", "", false),
        ("ArrowDown", "", true),
        ("ArrowLeft", "s", false),
        ("ArrowRight", "", false),
        ("Home", "", false),
        ("End", "", false),
        ("PageUp", "", false),
        ("PageDown", "", false),
        ("F1", "", false),
        ("F2", "", false),
        ("F3", "", false),
        ("F4", "", false),
        ("F5", "", false),
        ("F6", "s", false),
        ("F7", "", false),
        ("F8", "", false),
        ("F9", "", false),
        ("F10", "", false),
        ("F11", "", false),
        ("F12", "", false),
    ];

    fn keys_named(app: &mut App) {
        app.ask("close", json!({ "id": "type" }));
        app.ask("open", json!({ "ids": ["log"] }));
        app.ask("focusLog", json!({}));
        let mut expected = Vec::new();
        for (name, modifiers, repeat) in NAMED {
            key(app, "log", name, true, modifiers, repeat);
            key(app, "log", name, false, modifiers, false);
            expected.push(format!(
                "d {name} {modifiers} {}",
                if repeat { "r" } else { "" }
            ));
            expected.push(format!("u {name} {modifiers} "));
        }
        // Not a key: dropped.
        key(app, "log", "Hyper", true, "", false);
        key(app, "log", "Hyper", false, "", false);
        // A chord arrives as a key.
        key(app, "log", "j", true, "c", false);
        key(app, "log", "j", false, "c", false);
        expected.push("d j c ".into());
        expected.push("u j c ".into());
        let log = until(app, "log", |v| {
            v["log"]
                .as_array()
                .is_some_and(|l| l.len() >= expected.len())
        });
        let log: Vec<&str> = log["log"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_str().unwrap())
            .collect();
        assert_eq!(log, expected);
    }

    fn composed(app: &mut App) {
        let placed = |report: &Value, name: &str| report["composed"][name]["placed"] == true;
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut report = app.report(&[]);
        while !(placed(&report, "open") && placed(&report, "clipped")) && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(50));
            report = app.report(&[]);
        }
        assert!(
            placed(&report, "open") && placed(&report, "clipped"),
            "{report}"
        );
        // A transform takes both boxes out of their clip.
        app.ask("pan", json!({ "x": -500 }));
        let deadline = Instant::now() + Duration::from_secs(10);
        while placed(&report, "clipped") && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
            report = app.report(&[]);
        }
        assert!(
            !placed(&report, "clipped"),
            "a component that clips stayed placed: {report}"
        );
        assert!(
            placed(&report, "open"),
            "a component that does not clip went: {report}"
        );
    }

    /// Whether `rect` lies wholly out of `screen`.
    fn outside(rect: &RECT, screen: &RECT) -> bool {
        rect.right <= screen.left
            || rect.left >= screen.right
            || rect.bottom <= screen.top
            || rect.top >= screen.bottom
    }

    /// A 16,384 x 1,000-point window at the stood-in `percent` %.
    fn large_window(app: &mut App, percent: u64) {
        app.ask("open", json!({ "ids": ["big"] }));
        let surface = app.surface("big");
        assert_eq!(surface["width"].as_f64(), Some(16384.0), "{surface}");
        assert_eq!(surface["height"].as_f64(), Some(1000.0), "{surface}");
        let hwnd = app.hwnd("big");
        kept_out_of_sight("big", hwnd);
        let mut client = RECT::default();
        // SAFETY: a query of another process's window.
        unsafe { GetClientRect(hwnd, &mut client) }.unwrap();
        let pixels = (client.right as u64, client.bottom as u64);
        assert_eq!(
            pixels,
            (16384 * percent / 100, 1000 * percent / 100),
            "the window at {percent} %"
        );
        assert!(
            outside(&window_rect(hwnd), &virtual_screen()),
            "on the displays at {:?}",
            window_rect(hwnd)
        );
        let report = app.report(&[hwnd]);
        let largest = report["largest"].as_u64().expect("the largest texture");
        assert!(pixels.0 > largest, "the window fits a texture: {largest}");
        let presented = &report["presented"][0];
        let (pw, ph) = (
            presented[0].as_u64().expect("the window presents nothing"),
            presented[1].as_u64().expect("the window presents nothing"),
        );
        assert!(pw <= largest && ph <= largest, "presented {pw} x {ph}");
        assert_eq!((pw, ph), (largest.min(pixels.0), pixels.1));
        let (part, pixels) = app.picture(
            "big",
            "big-rect",
            json!({ "scale": 1, "rect": { "x": 16290, "y": 0, "width": 94, "height": 80 } }),
        );
        assert_eq!(
            (part["width"].as_u64(), part["height"].as_u64()),
            (Some(94), Some(80))
        );
        assert_eq!(pixel(&pixels, 94, 30, 30), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 94, 5, 5)[3], 255);
        assert_eq!(report["seen"], json!([]));
    }

    /// A display left of the origin: a window that fitted left of the
    /// displays does no more (its origin would pass -32,768) and goes right
    /// of them, a small one stays left of them; none lies on one.
    fn display_left(app: &mut App) {
        app.ask("open", json!({ "ids": ["late"] }));
        let big = app.hwnd("big");
        let small = app.hwnd("late");
        let real = virtual_screen();
        assert!(
            window_rect(big).right <= real.left,
            "big starts left of the displays"
        );
        let screen = RECT {
            left: real.left - 20_000,
            ..real
        };
        app.ask(
            "probe",
            json!({ "command": "screen", "args": {
                "left": screen.left, "top": screen.top, "right": screen.right, "bottom": screen.bottom,
            } }),
        );
        // SAFETY: a message with no pointers to another process's window, as
        // the system broadcasts it.
        unsafe { SendMessageW(big, WM_DISPLAYCHANGE, Some(WPARAM(32)), Some(LPARAM(0))) };
        let (b, s) = (window_rect(big), window_rect(small));
        assert!(
            b.left >= screen.right,
            "big not right of the displays: {b:?}"
        );
        assert!(
            s.right <= screen.left,
            "small not left of the displays: {s:?}"
        );
        assert!(outside(&b, &screen) && outside(&s, &screen));
        let report = app.report(&[]);
        assert_eq!(report["seen"], json!([]), "on a display meanwhile");
        app.ask("probe", json!({ "command": "screen", "args": {} }));
        // SAFETY: as above.
        unsafe { SendMessageW(big, WM_DISPLAYCHANGE, Some(WPARAM(32)), Some(LPARAM(0))) };
        assert!(
            window_rect(big).right <= real.left,
            "big not back left of the displays"
        );
    }

    /// A case: its name, and what it runs.
    type Case = (&'static str, fn(&mut App));

    pub(crate) fn run() {
        let picked: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with('-'))
            .collect();
        let wanted = |name: &str| picked.is_empty() || picked.iter().any(|p| p == name);
        let root = std::env::temp_dir().join(format!("nsh-design-windows-{}", std::process::id()));
        let token = format!("tok-{:016x}", {
            use std::hash::{BuildHasher, RandomState};
            RandomState::new().hash_one(std::process::id())
        });
        let mut ran = 0;
        let cases: [Case; 13] = [
            ("pictures", pictures),
            ("settle", settle),
            ("loops", loops),
            ("keyframes", keyframes),
            ("shadows", shadows),
            ("codes", codes),
            ("out-of-sight", out_of_sight),
            ("foreground", foreground),
            ("display-change", display_change),
            ("composed", composed),
            ("keys-typed", keys_typed),
            ("keys-control", keys_control),
            ("keys-named", keys_named),
        ];
        let mut app = App::start(&root.join("app"), Some(&token), None);
        app.ask("open", json!({ "ids": FIRST }));
        for (name, case) in cases {
            if wanted(name) {
                case(&mut app);
                println!("design_windows: {name}: ok");
                ran += 1;
            }
        }
        let first_hello = app.hello.clone();
        let first_log = app.finish();
        if wanted("keys-named") {
            assert!(
                first_log.contains("a design tool's key \"Hyper\""),
                "the unlisted key was not logged at debug level"
            );
        }
        // No renderer error.
        let clean = |log: &str| {
            for line in log.lines() {
                assert!(!line.trim_start().starts_with("ERROR"), "{line}");
            }
        };
        let mut second_log = String::new();
        let mut second_hello = Value::Null;
        if wanted("large-window") || wanted("display-left") || wanted("hello") {
            // Without a token, and at 150 %.
            let mut large = App::start(&root.join("large"), None, Some(144));
            second_hello = large.hello.clone();
            if wanted("large-window") || wanted("display-left") {
                large_window(&mut large, 150);
            }
            if wanted("display-left") {
                display_left(&mut large);
            }
            second_log = large.finish();
            for name in ["large-window", "display-left"] {
                if wanted(name) {
                    clean(&second_log);
                    println!("design_windows: {name}: ok");
                    ran += 1;
                }
            }
        }
        if wanted("large-window-300") {
            // At 300 %: 49,152 pixels wide, which fits left of no display.
            let mut larger = App::start(&root.join("larger"), None, Some(288));
            large_window(&mut larger, 300);
            clean(&larger.finish());
            println!("design_windows: large-window-300: ok");
            ran += 1;
        }
        if wanted("hello") {
            assert_eq!(first_hello, json!({ "hello": { "token": token } }));
            assert_eq!(second_hello, json!({ "hello": {} }));
            assert!(
                first_log.contains("TRACE") || first_log.contains("DEBUG"),
                "the app did not log verbosely"
            );
            assert!(!first_log.contains(&token), "the token is in the app's log");
            assert!(!second_log.contains(&token));
            println!("design_windows: hello: ok");
            ran += 1;
        }
        let _ = std::fs::remove_dir_all(&root);
        println!("design_windows: {ran} cases ok");
    }
}
