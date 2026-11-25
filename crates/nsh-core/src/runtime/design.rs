//! `nsh/design`: an app shown in a design tool, which draws the app's
//! windows in its own and drives them (`docs/dev/design-mode.md`). The
//! tool starts the app with `NSH_DESIGN_SOCKET` set to where it listens: a
//! Unix socket's path, or `tcp:<host>:<port>` (any platform). The
//! platform's design host calls [`connect`] at start; this module keeps the
//! connection, one JSON object a line both ways:
//!
//! - `{"hello":{}}` once connected (`{"hello":{"token":…}}` when the tool
//!   gave the run a token, [`TOKEN`]); then for each window of the app's own
//!   (`Window.open`, named by its `id`) a `{"surface":{"id":…,"width":…,
//!   "height":…, <handle>}}` line whenever it changes, where the handle is
//!   what the tool draws the window with, the platform's
//!   ([`DesignWindow::handle`]: `"context"`, the Core Animation context, on
//!   macOS; `"hwnd"` on Windows; `"wayland"`, the window's number, on Linux), and
//!   `{"surface":{"id":…,"gone":true}}` when it closed ([`report`]).
//! - The tool's `pointer`, `wheel`, `key` and `resize` messages are the
//!   platform's input, delivered to that window ([`INPUT`]); every other
//!   message is the app's, taken with `design.next()`, and what the app
//!   sends (`design.send()`) goes to the tool.
//! - When the connection closes, the tool has gone: the platform stops the
//!   app.
//!
//! In a dev build (feature `inspect`) the module also reads what the
//! windows show for the tool: `design.layers()`, the live tree under each
//! view whose test id starts with a prefix the tool chose, with the
//! component instances that rendered it, a view that hosts a Rust
//! component as that component and under it what the app gave it;
//! `design.detail()`, one layer in full, a host with what its component
//! takes; and `design.setOverrides()`, classes and states put on every
//! element written at a place in the source, again whenever it is made
//! anew. Without the hooks they answer nothing.
//!
//! For a tool that shows the app's windows as pictures, a dev build with
//! the hooks also has [`settled`] (`design.settled()`: a window has
//! been quiet for [`SETTLE_QUIET`], at most [`SETTLE_LIMIT`]) and
//! [`picture`] (`design.picture()`: the window's content drawn into a
//! file the tool names, with its full layer tree and the source modules it
//! uses), both the platform's to answer ([`PictureHooks`]), and tells the
//! tool which modules each hot update ran again ([`hot_updated`]:
//! `{"hotUpdate":{"modules":[…]}}`). Elsewhere they reject ([`codes`]).
//!
//! One design host per process: a design tool starts the app for itself.

use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    task::{Poll, Waker},
    time::{Duration, Instant},
};

use crate::{
    JsonValue,
    element::{JsError, module},
    geometry::{Bounds, Points},
};

/// The variable naming where the tool listens.
pub const SOCKET: &str = "NSH_DESIGN_SOCKET";

/// The variable holding the token the tool made for this run: when it is
/// set and not empty, the hello carries it (`{"hello":{"token":…}}`), so a
/// tool listening on a TCP port (any process of the machine can connect to
/// one) knows the connection is the app it started. It is never logged.
pub const TOKEN: &str = "NSH_DESIGN_TOKEN";

/// The tool's messages that are a window's input, which the platform
/// delivers: `{"pointer":{"id","kind":"down|drag|up|move","x","y"}}` (points
/// from the window's content's top left), `{"wheel":{"id","x","y","dx",
/// "dy"}}` (content moves up for a positive `dy`), `{"key":{"id","down",
/// "key","shift","control","alt","meta","repeat"}}` (a key as the DOM names
/// it), `{"resize":{"id","width","height"}}` and
/// `{"viewport":{"id","x","y","width","height","scale"}}` (a cropped,
/// high-DPI live slice in content points, or `{"viewport":{"id"}}` to reset
/// to the full window at display backing scale).
pub const INPUT: [&str; 5] = ["pointer", "wheel", "key", "resize", "viewport"];

/// What the platform does for the connection.
pub struct DesignHooks {
    /// Takes a message of [`INPUT`], from the connection's thread, to
    /// deliver on the platform's main thread in order.
    pub input: Box<dyn Fn(JsonValue) + Send + Sync>,
    /// The tool's connection closed: stop the app (and the dev session
    /// that runs it).
    pub closed: Box<dyn FnOnce() + Send>,
}

/// The connection's writing half.
trait Line: Write + Send {}
impl<T: Write + Send> Line for T {}

static LINE: Mutex<Option<Box<dyn Line>>> = Mutex::new(None);
static ATTACHED: OnceLock<()> = OnceLock::new();

/// The tool's messages the app has not taken yet, and the `next()` calls
/// waiting for one.
struct Inbox {
    messages: VecDeque<JsonValue>,
    waiting: Vec<Waker>,
}

static INBOX: Mutex<Inbox> = Mutex::new(Inbox {
    messages: VecDeque::new(),
    waiting: Vec::new(),
});

/// Where the tool listens ([`SOCKET`]), if this process was started as a
/// design host.
pub fn endpoint() -> Option<String> {
    std::env::var(SOCKET).ok().filter(|e| !e.is_empty())
}

/// Makes this process a design host connected to the tool at `endpoint`
/// ([`endpoint`]): a thread connects (the tool may start listening after
/// the app starts), says hello and hands on the tool's messages. Called
/// once, by the platform, before the app starts; `false` when it was
/// called before.
pub fn connect(endpoint: String, hooks: DesignHooks) -> bool {
    if ATTACHED.set(()).is_err() {
        return false;
    }
    std::thread::Builder::new()
        .name("nsh-design".to_owned())
        .spawn(move || {
            let reader = loop {
                match open(&endpoint) {
                    Ok((reader, writer)) => {
                        *LINE.lock().unwrap_or_else(|e| e.into_inner()) = Some(writer);
                        break reader;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(200)),
                }
            };
            log::info!(target: "nsh::design", "connected to the design tool");
            send_line(&hello(token().as_deref()));
            for line in reader.lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str::<JsonValue>(&line) {
                    Ok(message) if is_input(&message) => (hooks.input)(message),
                    Ok(message) => deliver(message),
                    Err(_) => {
                        log::warn!(target: "nsh::design", "the design tool sent a line that is not JSON")
                    }
                }
            }
            log::info!(target: "nsh::design", "the design tool closed its connection; stopping");
            *LINE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            (hooks.closed)();
        })
        .is_ok()
}

/// The token the tool gave for this run ([`TOKEN`]), if any: never logged.
fn token() -> Option<String> {
    std::env::var(TOKEN).ok().filter(|t| !t.is_empty())
}

/// The first line to the tool: `{"hello":{}}`, with the run's `token` when
/// the tool gave one (`{"hello":{"token":…}}`), so that a tool listening on
/// a TCP port takes only the app it started.
fn hello(token: Option<&str>) -> JsonValue {
    match token {
        Some(token) => serde_json::json!({ "hello": { "token": token } }),
        None => serde_json::json!({ "hello": {} }),
    }
}

/// A connection to `endpoint`: its reading and writing halves.
fn open(endpoint: &str) -> std::io::Result<(Box<dyn BufRead + Send>, Box<dyn Line>)> {
    if let Some(address) = endpoint.strip_prefix("tcp:") {
        let stream = std::net::TcpStream::connect(address)?;
        let _ = stream.set_nodelay(true);
        let writer = stream.try_clone()?;
        return Ok((Box::new(BufReader::new(stream)), Box::new(writer)));
    }
    #[cfg(unix)]
    {
        let stream = std::os::unix::net::UnixStream::connect(endpoint)?;
        let writer = stream.try_clone()?;
        Ok((Box::new(BufReader::new(stream)), Box::new(writer)))
    }
    #[cfg(not(unix))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "NSH_DESIGN_SOCKET: a Unix socket's path; name tcp:<host>:<port> here",
        ))
    }
}

/// Whether this process is a design host ([`connect`]): what the app's
/// script reads as `design.active()` and a Rust view as
/// [`crate::element::design_mode`].
pub fn attached() -> bool {
    ATTACHED.get().is_some()
}

/// Whether `message` is a window's input ([`INPUT`]).
pub fn is_input(message: &JsonValue) -> bool {
    INPUT.iter().any(|kind| message.get(kind).is_some())
}

fn send_line(message: &JsonValue) {
    if let Some(line) = LINE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let _ = line.write_all(format!("{message}\n").as_bytes());
        let _ = line.flush();
    }
}

/// Hands the app a message of the tool's, from any thread.
pub fn deliver(message: JsonValue) {
    let waiting = {
        let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
        inbox.messages.push_back(message);
        std::mem::take(&mut inbox.waiting)
    };
    for waker in waiting {
        waker.wake();
    }
}

/// A window of the app's own as the tool sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignWindow {
    /// Its `id` (`Window.open`).
    pub id: String,
    /// What the tool draws it with: `("context", <CAContext id>)` on macOS,
    /// `("hwnd", <HWND>)` on Windows, `("wayland", <window number>)` on Linux.
    pub handle: (&'static str, u64),
    /// Its content's size.
    pub size: crate::geometry::Size<crate::geometry::Points>,
}

/// A cropped, scaled live view of a design window's content: what the
/// window's live surface actually presents when zoomed in past its
/// display's backing scale (in content points from the top left, at
/// `scale` pixels a point).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignViewport {
    /// Left edge of the presented slice in content points.
    pub x: f64,
    /// Top edge of the presented slice in content points.
    pub y: f64,
    /// Width of the presented slice in content points.
    pub width: f64,
    /// Height of the presented slice in content points.
    pub height: f64,
    /// Pixels per point the slice is rasterized at.
    pub scale: f64,
}

type ToldEntry = (
    DesignWindow,
    Vec<(&'static str, u64)>,
    Option<DesignViewport>,
);

static TOLD: Mutex<Option<HashMap<String, ToldEntry>>> = Mutex::new(None);

fn surface_line(
    window: &DesignWindow,
    extra: &[(&'static str, u64)],
    viewport: Option<DesignViewport>,
) -> JsonValue {
    let mut surface = serde_json::json!({
        "id": window.id,
        "width": f64::from(window.size.width.0),
        "height": f64::from(window.size.height.0),
    });
    surface[window.handle.0] = window.handle.1.into();
    for &(key, value) in extra {
        surface[key] = value.into();
    }
    if let Some(vp) = viewport {
        surface["viewport"] = serde_json::json!({
            "x": vp.x,
            "y": vp.y,
            "width": vp.width,
            "height": vp.height,
            "scale": vp.scale,
        });
    }
    serde_json::json!({ "surface": surface })
}

/// The app's windows now (shown, with a handle the tool can draw): tells
/// the tool of those new or changed since the last call, and of those gone.
/// The platform calls it on a timer.
pub fn report(windows: &[DesignWindow]) {
    let entries: Vec<(&DesignWindow, &[(&'static str, u64)])> =
        windows.iter().map(|w| (w, &[][..])).collect();
    report_extra(&entries);
}

/// Like [`report`], with extra numeric fields added to each window's
/// `surface` line (such as a shared composition surface handle and owning
/// process id on Windows). A change in any extra field reports the window
/// again.
pub fn report_extra(windows: &[(&DesignWindow, &[(&'static str, u64)])]) {
    let entries: Vec<ReportedWindow<'_>> =
        windows.iter().map(|&(w, extra)| (w, extra, None)).collect();
    report_with_viewport(&entries);
}

/// A window [`report_with_viewport`] tells the tool of: the window, the
/// extra numeric fields of its `surface` line, and the cropped/scaled live
/// [`DesignViewport`] its surface presents, if any.
pub type ReportedWindow<'a> = (
    &'a DesignWindow,
    &'a [(&'static str, u64)],
    Option<DesignViewport>,
);

/// Like [`report_extra`], also carrying the optional cropped/scaled live
/// [`DesignViewport`] each window's surface currently presents.
pub fn report_with_viewport(windows: &[ReportedWindow<'_>]) {
    let mut lines = Vec::new();
    {
        let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
        let told = told.get_or_insert_with(HashMap::new);
        for &(window, extra, viewport) in windows {
            let changed = told
                .get(&window.id)
                .is_none_or(|(prev, prev_extra, prev_viewport)| {
                    prev != window || prev_extra.as_slice() != extra || *prev_viewport != viewport
                });
            if changed {
                told.insert(
                    window.id.clone(),
                    (window.clone(), extra.to_vec(), viewport),
                );
                lines.push(surface_line(window, extra, viewport));
            }
        }
        let gone: Vec<String> = told
            .keys()
            .filter(|id| !windows.iter().any(|(w, _, _)| &w.id == *id))
            .cloned()
            .collect();
        for id in gone {
            told.remove(&id);
            lines.push(serde_json::json!({ "surface": { "id": id, "gone": true } }));
        }
    }
    for line in &lines {
        send_line(line);
    }
}

/// The `code`s [`settled`] and [`picture`] reject with. Tools branch on
/// them; the messages are for people and may change.
pub mod codes {
    /// No design tool started the app, or the build has no dev hooks
    /// (`inspect`): there is nothing to answer.
    pub const NOT_ACTIVE: &str = "ERR_NSH_DESIGN_NOT_ACTIVE";
    /// The platform draws no pictures (Node's).
    pub const UNAVAILABLE: &str = crate::element::codes::UNAVAILABLE;
    /// No window of the app's own is open with that id.
    pub const NO_WINDOW: &str = "ERR_NSH_DESIGN_NO_WINDOW";
    /// The `rect` asked for is not within the window's content.
    pub const RECT_OUTSIDE: &str = "ERR_NSH_DESIGN_RECT_OUTSIDE";
    /// The picture's file cannot be written at the path the tool gave.
    pub const PATH: &str = "ERR_NSH_DESIGN_PATH";
    /// An argument is not what the call takes (Node's).
    pub const INVALID: &str = "ERR_INVALID_ARG_VALUE";
}

/// How long a window must have been quiet to be settled ([`Settling`]):
/// no batch applied and no animation that ends by itself running.
pub const SETTLE_QUIET: Duration = Duration::from_millis(50);
/// How long [`settled`] waits at most, then answers that it timed out.
pub const SETTLE_LIMIT: Duration = Duration::from_millis(1000);
/// How often a platform looks at its windows while one settles.
pub const SETTLE_POLL: Duration = Duration::from_millis(5);

/// The settle rule, which each platform applies to what it knows on its
/// main thread: a window is settled once, since the question, no batch has
/// been applied and no animation that ends by itself has run for
/// [`SETTLE_QUIET`]; animations that loop with no end (a spinner) do not
/// count, or nothing that shows one would settle. Past [`SETTLE_LIMIT`]
/// it is given up ([`Settled::timed_out`]).
#[derive(Debug, Clone, Copy)]
pub struct Settling {
    started: Instant,
    batches: u64,
    quiet_since: Instant,
}

/// How [`settled`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settled {
    /// It was still busy at [`SETTLE_LIMIT`].
    pub timed_out: bool,
}

impl Settling {
    /// Starts at `now`, with `batches` applied so far (a count the
    /// platform keeps).
    pub fn new(now: Instant, batches: u64) -> Self {
        Self {
            started: now,
            batches,
            quiet_since: now,
        }
    }

    /// Looks again at `now`: `batches` applied so far, and whether an
    /// animation that ends by itself runs or batches wait for a frame
    /// (`busy`). `Some` once it is settled or given up.
    pub fn step(&mut self, now: Instant, batches: u64, busy: bool) -> Option<Settled> {
        if batches != self.batches || busy {
            self.batches = batches;
            self.quiet_since = now;
        }
        if now.duration_since(self.quiet_since) >= SETTLE_QUIET {
            return Some(Settled { timed_out: false });
        }
        (now.duration_since(self.started) >= SETTLE_LIMIT).then_some(Settled { timed_out: true })
    }
}

/// Why the platform could not answer a question of [`PictureHooks`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// One of [`codes`].
    pub code: &'static str,
    /// For people.
    pub message: String,
}

impl Failure {
    /// A failure with `code` and `message`.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<Failure> for JsError {
    fn from(failure: Failure) -> Self {
        JsError::Error(format!("nsh/design: {}", failure.message)).with_code(failure.code)
    }
}

/// Where the platform's answer goes: [`Reply::send`] it from any thread.
/// Dropped unsent, the question fails as [`codes::UNAVAILABLE`].
pub struct Reply<T> {
    pending: Arc<Mutex<Pending<T>>>,
}

struct Pending<T> {
    value: Option<T>,
    /// Sent, or dropped unsent.
    done: bool,
    waker: Option<Waker>,
}

impl<T> Reply<T> {
    fn pair() -> (Self, impl Future<Output = Option<T>>) {
        let pending = Arc::new(Mutex::new(Pending {
            value: None,
            done: false,
            waker: None,
        }));
        let waiting = pending.clone();
        let answer = std::future::poll_fn(move |cx| {
            let mut p = waiting.lock().unwrap_or_else(|e| e.into_inner());
            if p.done {
                return Poll::Ready(p.value.take());
            }
            p.waker = Some(cx.waker().clone());
            Poll::Pending
        });
        (Self { pending }, answer)
    }

    /// Answers the question.
    pub fn send(self, value: T) {
        self.finish(Some(value));
    }

    fn finish(&self, value: Option<T>) {
        let waker = {
            let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            if p.done {
                return;
            }
            p.done = true;
            p.value = value;
            p.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Drop for Reply<T> {
    fn drop(&mut self) {
        self.finish(None);
    }
}

/// [`settled`]'s question to the platform.
pub struct SettleAsk {
    /// The window's id (`Window.open`'s).
    pub window: String,
    /// Where [`Settling`]'s answer goes.
    pub reply: Reply<Result<Settled, Failure>>,
}

/// [`picture`]'s question to the platform: once the batches the core has
/// sent are applied and a frame has followed them, draw window `window`'s
/// content (only `rect` of it, in the content's points from its top left,
/// when given) at `scale` pixels a point into the file at `path`: raw
/// pixels, 4 bytes each (red, green, blue and alpha, premultiplied, sRGB),
/// rows from the top, no padding and nothing else, which a tool maps.
/// Animations that loop with no end stand at their first frame.
pub struct PictureAsk {
    /// The window's id (`Window.open`'s).
    pub window: String,
    /// Pixels a point.
    pub scale: f64,
    /// The part drawn; `None`: all of it.
    pub rect: Option<Bounds<Points>>,
    /// The file written.
    pub path: PathBuf,
    /// The picture's size in pixels, or why there is none
    /// ([`codes::NO_WINDOW`], [`codes::RECT_OUTSIDE`], [`codes::PATH`]).
    pub reply: Reply<Result<Drawn, Failure>>,
}

/// What a platform drew ([`PictureAsk`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drawn {
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
}

/// The platform's part of [`settled`] and [`picture`]: each takes its
/// question on the engine's thread and answers it later, from its main
/// thread ([`Reply`]).
pub struct PictureHooks {
    /// Settles a window by [`Settling`]'s rule.
    pub settled: Box<dyn Fn(SettleAsk) + Send + Sync>,
    /// Draws a window ([`PictureAsk`]).
    pub picture: Box<dyn Fn(PictureAsk) + Send + Sync>,
}

static PICTURES: OnceLock<PictureHooks> = OnceLock::new();

/// Gives design mode the platform's pictures ([`PictureHooks`]); once, by
/// a platform's design host that draws them, after [`connect`]. A
/// platform that never calls it rejects [`picture`] and [`settled`] with
/// [`codes::UNAVAILABLE`].
pub fn set_picture_hooks(hooks: PictureHooks) -> bool {
    PICTURES.set(hooks).is_ok()
}

/// A picture of a window ([`picture`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
    /// The full tree of the window's root, in `design.layers`' node shape.
    pub layers: Vec<JsonValue>,
    /// The source modules the tree's layers were written in or rendered
    /// by, as stamps name them (`src/Card.tsx`, `@acme-ui/src/Card.tsx`).
    pub modules: Vec<String>,
}

/// How [`picture`] draws.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PictureOptions {
    /// Pixels a point, above 0 (at most 16).
    pub scale: f64,
    /// The file to write (absolute): a path the tool chose, outside the
    /// app.
    pub path: PathBuf,
    /// Only this part of the content, in its points from its top left.
    pub rect: Option<Bounds<Points>>,
    /// Instances whose key or call site contains it are left out of the
    /// layers, as `design.layers`' `hide`.
    pub hide: Option<String>,
}

impl PictureOptions {
    /// `{ scale, path, rect?: { x, y, width, height }, hide? }`, as the
    /// script gives them.
    fn from_json(options: &JsonValue) -> Result<Self, JsError> {
        let invalid = |what: &str| {
            JsError::Type(format!("nsh/design: picture takes {what}")).with_code(codes::INVALID)
        };
        let scale = options["scale"]
            .as_f64()
            .ok_or_else(|| invalid("a scale (a number)"))?;
        let path = options["path"]
            .as_str()
            .ok_or_else(|| invalid("a path (a string)"))?;
        let rect = match &options["rect"] {
            JsonValue::Null => None,
            rect => {
                let n = |key: &str| {
                    rect[key]
                        .as_f64()
                        .ok_or_else(|| invalid("a rect of numbers: { x, y, width, height }"))
                };
                Some(crate::geometry::bounds(
                    crate::geometry::point(Points(n("x")? as f32), Points(n("y")? as f32)),
                    crate::geometry::size(Points(n("width")? as f32), Points(n("height")? as f32)),
                ))
            }
        };
        let hide = match &options["hide"] {
            JsonValue::Null => None,
            hide => Some(
                hide.as_str()
                    .ok_or_else(|| invalid("a hide that is a string"))?
                    .to_owned(),
            ),
        };
        Ok(Self {
            scale,
            path: PathBuf::from(path),
            rect,
            hide,
        })
    }

    /// The scale, path and rect checked.
    fn check(&self) -> Result<(), JsError> {
        let invalid =
            |what: String| JsError::Range(format!("nsh/design: {what}")).with_code(codes::INVALID);
        if !(self.scale.is_finite() && self.scale > 0.0 && self.scale <= 16.0) {
            return Err(invalid(format!(
                "a picture's scale is above 0 and at most 16, not {}",
                self.scale
            )));
        }
        if !self.path.is_absolute() {
            return Err(invalid("a picture's path is absolute".to_owned()));
        }
        if let Some(rect) = self.rect {
            let numbers = [
                rect.origin.x.0,
                rect.origin.y.0,
                rect.size.width.0,
                rect.size.height.0,
            ];
            if numbers.iter().any(|n| !n.is_finite())
                || rect.size.width.0 <= 0.0
                || rect.size.height.0 <= 0.0
            {
                return Err(invalid(
                    "a picture's rect has a size above 0 in finite points".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

/// The platform's pictures, in a dev build a design tool started.
fn pictures() -> Result<&'static PictureHooks, JsError> {
    if !(cfg!(feature = "inspect") && attached()) {
        return Err(Failure::new(
            codes::NOT_ACTIVE,
            "pictures are a design tool's, in a dev build it started",
        )
        .into());
    }
    PICTURES
        .get()
        .ok_or_else(|| Failure::new(codes::UNAVAILABLE, "this platform draws no pictures").into())
}

#[cfg_attr(not(feature = "inspect"), allow(dead_code))]
fn no_window(window: &str) -> JsError {
    Failure::new(codes::NO_WINDOW, format!("no window {window:?} is open")).into()
}

fn unanswered() -> JsError {
    Failure::new(codes::UNAVAILABLE, "the platform did not answer").into()
}

/// Resolves once window `window` (`Window.open`'s `id`) has settled by
/// [`Settling`]'s rule, after what the app did before asking has been
/// sent: `timed_out` when it was still busy at [`SETTLE_LIMIT`]. Rejects
/// with [`codes`]: no tool started the app or no dev hooks, a platform
/// without pictures, no such window.
pub async fn settled(window: &str) -> Result<Settled, JsError> {
    let hooks = pictures()?;
    #[cfg(feature = "inspect")]
    if crate::element::dev_window(window).await.is_none() {
        return Err(no_window(window));
    }
    let (reply, answer) = Reply::pair();
    (hooks.settled)(SettleAsk {
        window: window.to_owned(),
        reply,
    });
    answer.await.ok_or_else(unanswered)?.map_err(Into::into)
}

/// Draws window `window` (`Window.open`'s `id`) as [`PictureAsk`] says,
/// after putting on its layers the overrides the tool asked for
/// (`design.setOverrides`) and laying them out, and answers with the
/// picture's size, the full tree of the window's root as `design.layers`
/// lists a root (taken apart from `layers`' "since the last call", which it
/// leaves as it is) and the modules that tree uses. Rejects with
/// [`codes`]: no tool started the app or no dev hooks, a platform without
/// pictures, options it does not take, no such window, a `rect` outside the
/// content, a path it cannot write.
pub async fn picture(window: &str, options: &PictureOptions) -> Result<Picture, JsError> {
    let hooks = pictures()?;
    options.check()?;
    #[cfg(feature = "inspect")]
    {
        let root = crate::element::dev_window(window)
            .await
            .ok_or_else(|| no_window(window))?;
        let (layers, modules) = layers::window(root, options.hide.as_deref())
            .await
            .ok_or_else(|| no_window(window))?;
        let (reply, answer) = Reply::pair();
        (hooks.picture)(PictureAsk {
            window: window.to_owned(),
            scale: options.scale,
            rect: options.rect,
            path: options.path.clone(),
            reply,
        });
        let drawn = answer.await.ok_or_else(unanswered)??;
        Ok(Picture {
            width: drawn.width,
            height: drawn.height,
            layers,
            modules,
        })
    }
    #[cfg(not(feature = "inspect"))]
    {
        let _ = (hooks, window);
        Err(Failure::new(codes::NOT_ACTIVE, "pictures need the dev hooks").into())
    }
}

/// A hot update ran `modules` again (by their hot keys, as stamps name
/// them: `src/Card.tsx`): a dev build tells the design tool,
/// `{"hotUpdate":{"modules":[…]}}`, so that it draws again what they
/// show. Nothing outside design mode.
pub fn hot_updated(modules: &[String]) {
    if cfg!(feature = "inspect") && attached() {
        send_line(&serde_json::json!({ "hotUpdate": { "modules": modules } }));
    }
}

/// The app's side of a design tool's connection.
#[module]
pub(crate) struct Design;

#[module(name = "design")]
impl Design {
    /// Whether a design tool shows this app: the app was started as its
    /// design host (`NSH_DESIGN_SOCKET`). Without one, `next()` never
    /// resolves and `send()` does nothing. Rust: `design_mode()`.
    pub(crate) fn active() -> bool {
        attached()
    }

    /// The tool's next message, an object: the first one not taken yet, or
    /// the next to come. Messages arrive in the order the tool sent them.
    pub(crate) async fn next() -> JsonValue {
        std::future::poll_fn(|cx| {
            let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
            match inbox.messages.pop_front() {
                Some(message) => Poll::Ready(message),
                None => {
                    if !inbox.waiting.iter().any(|w| w.will_wake(cx.waker())) {
                        inbox.waiting.push(cx.waker().clone());
                    }
                    Poll::Pending
                }
            }
        })
        .await
    }

    /// Sends `message`, an object, to the tool.
    pub(crate) fn send(message: JsonValue) -> Result<(), JsError> {
        if !message.is_object() {
            return Err(JsError::Type("nsh/design: send takes an object".to_owned()));
        }
        send_line(&message);
        Ok(())
    }

    /// What changed under each view whose test id starts with `prefix`
    /// (a root), since the last call: `{ roots: { <the rest of its test
    /// id>: { nodes: [...] } }, gone: [<roots no longer there>] }`, or `null`
    /// when nothing changed or the build has no dev hooks. All roots again
    /// after `setOverrides`. A node is `{ id, parent, kind, name, class,
    /// testId, text, x, y, width, height, source, hidden, overridden,
    /// slot, place }`; a component instance (`kind: "component"`, `{ id, parent,
    /// name, key, source }`) sits between the nodes it rendered and their
    /// parent, its id above every node's ([`layers::INSTANCE_IDS`]).
    /// Instances whose key contains `hide` (the tool's own components) are
    /// left out.
    ///
    /// A view that hosts a Rust component is `kind: "host"`, its `name`
    /// the component's JS export and its `source` where the app wrote it
    /// (a registered native view stays `kind: "native"`). What the
    /// component draws inside its host is not listed, the components its
    /// view nests neither; what the app gave it is, under the host in the
    /// order the view placed it: its JSX children, and for each prop the
    /// app fills with content the node holding it, whose `slot` is the
    /// prop's JS name (a named slot's holder, the list of a rows prop, what
    /// a render function rendered: the glue's nodes, with no `source`; the
    /// app's own are under them). A row a component's list wraps lists
    /// only the app's row.
    ///
    /// A slot marker (a component whose view calls
    /// [`crate::element::design_slot`]) has `place`, its slot's name (every
    /// other layer `place: null`). One the component's view draws is listed
    /// under that component's host, empty or not, with what it holds under
    /// it; one a component nested in that view draws for its own slot is
    /// not.
    pub(crate) async fn layers(prefix: String, hide: Option<String>) -> JsonValue {
        layers::layers(&prefix, hide.as_deref()).await
    }

    /// Layer `id` in full: a node's style, box, states, events, role,
    /// label, source and component, or a component instance's props (`{
    /// props: [{ name, value: { kind, value } }] }`); `null` when it is
    /// gone or the build has no dev hooks. A host's `component` is what
    /// its Rust component takes, from its spec: `{ name, props: [{ name,
    /// kind: "value" | "renderer" | "slot" | "children" | "items", type,
    /// optional, value }], events: [{ name, type, value }] }` (an event's
    /// `name` is its handler's; `type` is TypeScript, `null` in a build
    /// without the text of declarations, as apps' hosts are). `value` is
    /// what the app passes, read as an instance's props are (a getter is
    /// `{ kind: "getter" }`, never called; rows by their data, `each`),
    /// `null` for what it leaves out; hot bundles keep it, so without one
    /// every `value` is `null`. Other nodes have `component: null`.
    pub(crate) async fn detail(id: f64) -> JsonValue {
        layers::detail(id).await
    }

    /// The overrides: `{ "<module>:<line>:<column>": { class, replace,
    /// states } }`. Every element written at one of those places gets its
    /// classes (over its own, or in their place with `replace`) and states
    /// (a `DevOverride`'s bits), and gets them again when it is made anew;
    /// the others none. Applied by the next `layers()`.
    pub(crate) fn set_overrides(overrides: JsonValue) {
        layers::set_overrides(&overrides);
    }

    /// Resolves once window `id` (`Window.open`'s) has been quiet for 50 ms
    /// (no batch applied, no animation that ends by itself running;
    /// animations that loop forever do not count), at most 1 s: `{
    /// timedOut }`, true when it was still busy then. Dev builds a design
    /// tool started; otherwise, and for a window that is not open, it
    /// rejects (`code`: `ERR_NSH_DESIGN_NOT_ACTIVE`,
    /// `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`, `ERR_NSH_DESIGN_NO_WINDOW`).
    pub(crate) async fn settled(id: String) -> Result<JsonValue, JsError> {
        let settled = settled(&id).await?;
        Ok(serde_json::json!({ "timedOut": settled.timed_out }))
    }

    /// Draws window `id`'s content, or `rect` of it (its points from the
    /// content's top left), at `scale` pixels a point into the file at
    /// `path` (absolute; a path the tool chose): raw pixels, RGBA with
    /// alpha premultiplied, sRGB, rows from the top, no padding. The
    /// overrides of `setOverrides` are put on first and laid out, and the
    /// picture waits for a frame after them; animations that loop forever
    /// stand at their first frame. Resolves to `{ width, height, layers,
    /// modules }`: the pixel size, every layer under the window's root in
    /// `layers()`' node shape (instances whose key or call site contains
    /// `hide` left out; `layers()`' own "since the last call" is not
    /// touched) and the source modules those layers use. Rejects with a
    /// `code`: `ERR_NSH_DESIGN_NOT_ACTIVE`,
    /// `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`, `ERR_INVALID_ARG_VALUE`,
    /// `ERR_NSH_DESIGN_NO_WINDOW`, `ERR_NSH_DESIGN_RECT_OUTSIDE`,
    /// `ERR_NSH_DESIGN_PATH`.
    pub(crate) async fn picture(id: String, options: JsonValue) -> Result<JsonValue, JsError> {
        let options = PictureOptions::from_json(&options)?;
        let picture = picture(&id, &options).await?;
        Ok(serde_json::json!({
            "width": picture.width,
            "height": picture.height,
            "layers": picture.layers,
            "modules": picture.modules,
        }))
    }
}

pub(crate) mod layers {
    #[cfg(not(feature = "inspect"))]
    use crate::JsonValue;

    /// Where component instances' layer ids start: above every node id
    /// (`u32`).
    #[cfg(feature = "inspect")]
    pub(crate) const INSTANCE_IDS: u64 = 1 << 32;

    #[cfg(not(feature = "inspect"))]
    pub(crate) async fn layers(_prefix: &str, _hide: Option<&str>) -> JsonValue {
        JsonValue::Null
    }

    #[cfg(not(feature = "inspect"))]
    pub(crate) async fn detail(_id: f64) -> JsonValue {
        JsonValue::Null
    }

    #[cfg(not(feature = "inspect"))]
    pub(crate) fn set_overrides(_overrides: &JsonValue) {}

    #[cfg(feature = "inspect")]
    pub(crate) use shown::{detail, layers, set_overrides, window};

    #[cfg(feature = "inspect")]
    mod shown {
        use std::{
            cell::RefCell,
            collections::{HashMap, HashSet},
            hash::{Hash, Hasher},
        };

        use serde_json::{Map, Value, json};

        use super::INSTANCE_IDS;
        use crate::{
            NodeId,
            element::{
                PropKind, dev_host_props, dev_instance_props, dev_instances, dev_node,
                dev_override, dev_source, dev_tree,
            },
            inspect::{
                DevComponent, DevDetail, DevInstance, DevNode, DevOverride, DevProp, DevSource,
            },
        };

        /// An override as the tool asks for it.
        #[derive(Clone, Debug, PartialEq)]
        struct Layer {
            class: String,
            replace: bool,
            states: u16,
        }

        #[derive(Default)]
        struct State {
            /// Each root's fingerprint, as last reported.
            seen: HashMap<String, u64>,
            /// The overrides asked for, by where the element was written.
            wanted: HashMap<String, Layer>,
            /// The override on each node now.
            applied: HashMap<NodeId, Layer>,
            /// The overrides changed: every root is looked at again.
            dirty: bool,
        }

        thread_local! {
            static STATE: RefCell<State> = RefCell::default();
        }

        /// The layers of a root: its nodes in tree order, and the node
        /// each is listed under where that is not the root.
        struct Shown<'a> {
            nodes: Vec<&'a DevNode>,
            parents: HashMap<NodeId, NodeId>,
        }

        /// What the core says of a node besides the tree.
        struct Facts {
            source: Option<DevSource>,
            instances: Vec<DevInstance>,
        }

        pub(crate) fn set_overrides(overrides: &Value) {
            let wanted = overrides
                .as_object()
                .into_iter()
                .flatten()
                .map(|(key, value)| {
                    let layer = Layer {
                        class: value["class"].as_str().unwrap_or("").to_owned(),
                        replace: value["replace"].as_bool().unwrap_or(false),
                        states: value["states"].as_u64().unwrap_or(0) as u16,
                    };
                    (key.clone(), layer)
                })
                .collect();
            STATE.with_borrow_mut(|s| {
                s.wanted = wanted;
                s.dirty = true;
            });
        }

        pub(crate) async fn layers(prefix: &str, hide: Option<&str>) -> Value {
            let tree = dev_tree().await;
            let by_id: HashMap<NodeId, &DevNode> = tree.nodes.iter().map(|n| (n.id, n)).collect();
            let dirty = STATE.with_borrow_mut(|s| std::mem::take(&mut s.dirty));
            let mut roots = Map::new();
            let mut present = HashSet::new();
            for node in &tree.nodes {
                let Some(root) = node.test_id.as_deref().and_then(|t| t.strip_prefix(prefix))
                else {
                    continue;
                };
                present.insert(root.to_owned());
                let shown = below(node, &by_id);
                let nodes = &shown.nodes;
                let print = fingerprint(nodes);
                let unchanged = STATE.with_borrow(|s| s.seen.get(root) == Some(&print));
                if unchanged && !dirty {
                    continue;
                }
                STATE.with_borrow_mut(|s| s.seen.insert(root.to_owned(), print));
                let facts = facts(nodes).await;
                let _changed = apply(nodes, &facts).await;
                roots.insert(
                    root.to_owned(),
                    json!({ "nodes": list(&shown, &facts, hide) }),
                );
            }
            let gone: Vec<String> = STATE.with_borrow_mut(|s| {
                let gone: Vec<String> = s
                    .seen
                    .keys()
                    .filter(|k| !present.contains(*k))
                    .cloned()
                    .collect();
                for key in &gone {
                    s.seen.remove(key);
                }
                gone
            });
            if roots.is_empty() && gone.is_empty() {
                return Value::Null;
            }
            json!({ "roots": roots, "gone": gone })
        }

        /// Every layer under `root` (a window's root node), the overrides
        /// asked for put on first and laid out, listed as [`layers`] lists
        /// a root's, and the modules they use, as stamps name them: where
        /// each layer was written, and each component instance's module.
        /// The state of [`layers`]' "since the last call" stays as it is.
        /// `None` once the root is gone.
        pub(crate) async fn window(
            root: NodeId,
            hide: Option<&str>,
        ) -> Option<(Vec<Value>, Vec<String>)> {
            let tree = dev_tree().await;
            let changed = {
                let by_id: HashMap<NodeId, &DevNode> =
                    tree.nodes.iter().map(|n| (n.id, n)).collect();
                let shown = below(by_id.get(&root)?, &by_id);
                let facts = facts(&shown.nodes).await;
                apply(&shown.nodes, &facts).await
            };
            // The tree as the commit after the overrides lays it out.
            let tree = if changed { dev_tree().await } else { tree };
            let by_id: HashMap<NodeId, &DevNode> = tree.nodes.iter().map(|n| (n.id, n)).collect();
            let shown = below(by_id.get(&root)?, &by_id);
            let facts = facts(&shown.nodes).await;
            let layers = list(&shown, &facts, hide);
            Some((layers, modules(&shown, &facts, hide)))
        }

        /// The modules `shown`'s layers were written in, and those of the
        /// component instances that rendered them (where each was used, and
        /// where its component is), sorted.
        fn modules(
            shown: &Shown<'_>,
            facts: &HashMap<NodeId, Facts>,
            hide: Option<&str>,
        ) -> Vec<String> {
            let hidden = |key: &str| hide.is_some_and(|h| !h.is_empty() && key.contains(h));
            let mut modules = std::collections::BTreeSet::new();
            for node in &shown.nodes {
                let Some(f) = facts.get(&node.id) else {
                    continue;
                };
                if let Some(source) = &f.source {
                    modules.insert(source.module.to_string());
                }
                for instance in f.instances.iter().filter(|i| !hidden(&i.key)) {
                    if let Some((module, _)) = instance.key.rsplit_once(':') {
                        modules.insert(module.to_owned());
                    }
                    if let Some(site) = instance.call_site.as_ref().filter(|s| !hidden(&s.module)) {
                        modules.insert(site.module.to_string());
                    }
                }
            }
            modules.into_iter().collect()
        }

        pub(crate) async fn detail(id: f64) -> Value {
            if !(id.is_finite() && id >= 0.0) {
                return Value::Null;
            }
            let id = id as u64;
            if id >= INSTANCE_IDS {
                let Ok(instance) = u32::try_from(id - INSTANCE_IDS) else {
                    return Value::Null;
                };
                let props = dev_instance_props(instance).await;
                let props: Vec<Value> = props
                    .iter()
                    .map(|(name, value)| json!({ "name": name, "value": prop(value) }))
                    .collect();
                return json!({ "props": props });
            }
            let Ok(node) = NodeId::try_from(id) else {
                return Value::Null;
            };
            // All asked before any is awaited: one round trip.
            let (detail, source, passed) = (dev_node(node), dev_source(node), dev_host_props(node));
            let (detail, source, passed) = (detail.await, source.await, passed.await);
            let Some(detail) = detail else {
                return Value::Null;
            };
            let edges = |e: &crate::inspect::DevEdges| json!({ "top": finite(e.top), "right": finite(e.right), "bottom": finite(e.bottom), "left": finite(e.left) });
            json!({
                "style": style(&detail),
                "width": finite(detail.layout.size.width),
                "height": finite(detail.layout.size.height),
                "padding": edges(&detail.layout.padding),
                "border": edges(&detail.layout.border),
                "margin": edges(&detail.layout.margin),
                "states": detail.states,
                "events": events(&detail),
                "role": role(&detail),
                "label": detail.accessibility.as_ref().and_then(|a| a.label.as_deref()),
                "source": source.as_ref().map(stamp),
                "component": detail.component.as_ref().map(|c| component(c, &passed)),
            })
        }

        /// The nodes under `root`, in tree order, without `root`. Under a
        /// host only what the app gave its component is listed, wherever
        /// the component's view placed it, as the host's own, and the slot
        /// markers its view draws: what else the view draws is not.
        fn below<'a>(root: &'a DevNode, by_id: &HashMap<NodeId, &'a DevNode>) -> Shown<'a> {
            let mut shown = Shown {
                nodes: Vec::new(),
                parents: HashMap::new(),
            };
            // A node to look at: the layer it would be under, and whether
            // it is in that layer's component's view.
            let mut stack: Vec<(NodeId, Option<NodeId>, bool)> = Vec::new();
            let under = |stack: &mut Vec<_>, node: &DevNode, parent: Option<NodeId>| {
                let in_view = node.flags.host;
                stack.extend(
                    node.children
                        .iter()
                        .rev()
                        .map(|&child| (child, parent, in_view)),
                );
            };
            under(&mut stack, root, None);
            while let Some((id, parent, in_view)) = stack.pop() {
                let Some(node) = by_id.get(&id) else {
                    continue;
                };
                // A slot marker the host's own view drew (not one a component
                // it nests drew for its own slot) is the host's layer.
                let marks_slot = parent
                    .is_some_and(|host| node.place.as_ref().is_some_and(|p| p.of == Some(host)));
                if in_view && node.given.is_none() && !marks_slot {
                    // The view's own: looked through for what it places.
                    stack.extend(
                        node.children
                            .iter()
                            .rev()
                            .map(|&child| (child, parent, true)),
                    );
                    continue;
                }
                shown.nodes.push(*node);
                if let Some(parent) = parent {
                    shown.parents.insert(node.id, parent);
                }
                under(&mut stack, node, Some(node.id));
            }
            shown
        }

        /// What the nodes show: their shape, classes, text and frames.
        fn fingerprint(nodes: &[&DevNode]) -> u64 {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for node in nodes {
                node.id.hash(&mut hasher);
                node.kind.hash(&mut hasher);
                node.children.hash(&mut hasher);
                node.component.hash(&mut hasher);
                node.class.hash(&mut hasher);
                node.test_id.hash(&mut hasher);
                node.text.hash(&mut hasher);
                node.place.as_ref().map(|p| &p.name).hash(&mut hasher);
                let page = node.page.unwrap_or(node.frame.origin);
                for value in [
                    page.x,
                    page.y,
                    node.frame.size.width,
                    node.frame.size.height,
                ] {
                    finite(value).to_bits().hash(&mut hasher);
                }
                (node.flags.hidden || node.flags.culled).hash(&mut hasher);
            }
            hasher.finish()
        }

        /// Every node's source and instances, all asked before any is
        /// awaited: one round trip.
        async fn facts(nodes: &[&DevNode]) -> HashMap<NodeId, Facts> {
            let asked: Vec<_> = nodes
                .iter()
                .map(|node| (node.id, dev_source(node.id), dev_instances(node.id)))
                .collect();
            let mut out = HashMap::with_capacity(asked.len());
            for (id, source, instances) in asked {
                out.insert(
                    id,
                    Facts {
                        source: source.await,
                        instances: instances.await,
                    },
                );
            }
            out
        }

        /// Puts on each node the override its source asks for, and takes
        /// it off the others; whether any changed.
        async fn apply(nodes: &[&DevNode], facts: &HashMap<NodeId, Facts>) -> bool {
            let changes: Vec<(NodeId, Option<Layer>)> = STATE.with_borrow_mut(|s| {
                let mut changes = Vec::new();
                for node in nodes {
                    let wanted = facts
                        .get(&node.id)
                        .and_then(|f| f.source.as_ref())
                        .and_then(|source| s.wanted.get(&key(source)))
                        .cloned();
                    // A node made again has none, whatever was put on the
                    // node that had its id.
                    let on_node = if node.flags.overridden {
                        s.applied.get(&node.id).cloned()
                    } else {
                        None
                    };
                    if wanted != on_node {
                        match &wanted {
                            Some(layer) => s.applied.insert(node.id, layer.clone()),
                            None => s.applied.remove(&node.id),
                        };
                        changes.push((node.id, wanted));
                    }
                }
                changes
            });
            let asked: Vec<_> = changes
                .into_iter()
                .map(|(id, layer)| {
                    dev_override(
                        id,
                        layer.map(|l| DevOverride {
                            class: l.class,
                            replace_class: l.replace,
                            states: l.states,
                            ..DevOverride::default()
                        }),
                    )
                })
                .collect();
            let changed = !asked.is_empty();
            for answer in asked {
                answer.await;
            }
            changed
        }

        /// The layers, parents before children: each node, and each
        /// component instance before the first node it rendered.
        fn list(
            shown: &Shown<'_>,
            facts: &HashMap<NodeId, Facts>,
            hide: Option<&str>,
        ) -> Vec<Value> {
            let hidden = |key: &str| hide.is_some_and(|h| !h.is_empty() && key.contains(h));
            let chain = |id: NodeId| -> Vec<&DevInstance> {
                facts.get(&id).map_or_else(Vec::new, |f| {
                    f.instances
                        .iter()
                        .rev()
                        .filter(|i| !hidden(&i.key))
                        .collect()
                })
            };
            let wanted: HashSet<String> = STATE.with_borrow(|s| s.wanted.keys().cloned().collect());
            let mut out = Vec::new();
            let mut made = HashSet::new();
            for node in &shown.nodes {
                let parent = shown.parents.get(&node.id).copied();
                let mine = chain(node.id);
                // A node the core made (odd ids: a slot marker a view drew)
                // renders in no instance: the chain to compare with is that
                // of the nearest layer above it that the script made.
                let mut script_parent = parent;
                while let Some(p) = script_parent.filter(|p| p % 2 == 1) {
                    script_parent = shown.parents.get(&p).copied();
                }
                let theirs = script_parent.map(chain).unwrap_or_default();
                let shared = mine
                    .iter()
                    .zip(theirs.iter())
                    .take_while(|(a, b)| a.id == b.id)
                    .count();
                let mut layer_parent = parent.map(u64::from);
                for instance in &mine[shared..] {
                    let pseudo = INSTANCE_IDS + u64::from(instance.id);
                    if made.insert(instance.id) {
                        let source = instance.call_site.as_ref().filter(|s| !hidden(&s.module));
                        out.push(json!({
                            "id": pseudo,
                            "parent": layer_parent,
                            "kind": "component",
                            "name": instance.key.rsplit(':').next().unwrap_or(&instance.key),
                            "key": instance.key,
                            "source": source.map(stamp),
                        }));
                    }
                    layer_parent = Some(pseudo);
                }
                let source = facts.get(&node.id).and_then(|f| f.source.as_ref());
                let page = node.page.unwrap_or(node.frame.origin);
                out.push(json!({
                    "id": u64::from(node.id),
                    "parent": layer_parent,
                    "kind": match node.component {
                        Some(_) if node.flags.host => "host",
                        Some(_) => "native",
                        None => node.kind,
                    },
                    "name": node.component.as_deref().map_or_else(|| title(node.kind), str::to_owned),
                    "class": node.class,
                    "testId": node.test_id.as_deref(),
                    "text": node.text,
                    "x": finite(page.x),
                    "y": finite(page.y),
                    "width": finite(node.frame.size.width),
                    "height": finite(node.frame.size.height),
                    "source": source.map(stamp),
                    "hidden": node.flags.hidden || node.flags.culled,
                    "overridden": source.is_some_and(|s| wanted.contains(&key(s))),
                    "slot": node.given.as_ref().and_then(|g| g.slot.as_deref()),
                    "place": node.place.as_ref().map(|p| &*p.name),
                }));
            }
            out
        }

        fn key(source: &DevSource) -> String {
            format!("{}:{}:{}", source.module, source.line, source.column)
        }

        fn stamp(source: &DevSource) -> Value {
            json!({ "module": source.module, "line": source.line, "column": source.column })
        }

        /// `view` as `View`, `text-input` as `Text input`.
        fn title(kind: &str) -> String {
            let mut chars = kind.chars();
            chars.next().map_or_else(String::new, |first| {
                first
                    .to_uppercase()
                    .chain(chars)
                    .collect::<String>()
                    .replace('-', " ")
            })
        }

        fn lower_first(name: &str) -> String {
            let mut chars = name.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_lowercase().chain(chars).collect()
            })
        }

        /// A length as JSON carries it: a layout that has not run (a NaN)
        /// is 0.
        fn finite(points: crate::geometry::Points) -> f32 {
            let value = f32::from(points);
            if value.is_finite() { value } else { 0.0 }
        }

        fn style(detail: &DevDetail) -> Vec<Value> {
            detail
                .inline_style
                .iter()
                .map(|declaration| {
                    let text = format!("{declaration:?}");
                    let (name, value) = text.split_once('(').unwrap_or((&text, ""));
                    json!({ "name": lower_first(name), "value": value.trim_end_matches(')') })
                })
                .collect()
        }

        fn events(detail: &DevDetail) -> Vec<String> {
            crate::EventKind::ALL
                .iter()
                .filter(|event| detail.events.contains(**event))
                .map(|event| lower_first(&format!("{event:?}")))
                .collect()
        }

        fn role(detail: &DevDetail) -> Option<String> {
            let role = detail.accessibility.as_ref()?.role;
            let name = format!("{role:?}");
            (name != "None").then(|| lower_first(&name))
        }

        /// What a host's component takes, with what the app passes it.
        fn component(component: &DevComponent, passed: &[(String, DevProp)]) -> Value {
            let value = |name: &str| passed.iter().find(|(n, _)| n == name).map(|(_, v)| prop(v));
            let props: Vec<Value> = component
                .props
                .iter()
                .map(|p| {
                    let kind = match p.kind {
                        PropKind::Value => "value",
                        PropKind::Renderer => "renderer",
                        PropKind::Slot => "slot",
                        PropKind::Children => "children",
                        PropKind::Items => "items",
                    };
                    // Rows are passed as a list's: the data is `each`.
                    let passed = match p.kind {
                        PropKind::Items => value("each"),
                        _ => value(p.name),
                    };
                    json!({
                        "name": p.name,
                        "kind": kind,
                        "type": declared(&p.ts),
                        "optional": p.optional,
                        "value": passed,
                    })
                })
                .collect();
            let events: Vec<Value> = component
                .events
                .iter()
                .map(|e| {
                    json!({
                        "name": e.handler,
                        "type": declared(&e.data),
                        "value": value(&e.handler),
                    })
                })
                .collect();
            json!({ "name": component.name, "props": props, "events": events })
        }

        /// A spec's TypeScript, which only builds with declarations
        /// carry (`element::DECLARATIONS`).
        fn declared(ts: &str) -> Option<&str> {
            (!ts.is_empty()).then_some(ts)
        }

        fn prop(value: &DevProp) -> Value {
            match value {
                DevProp::Getter => json!({ "kind": "getter" }),
                DevProp::Function => json!({ "kind": "function" }),
                DevProp::Undefined => json!({ "kind": "undefined" }),
                DevProp::Null => json!({ "kind": "null" }),
                DevProp::Bool(b) => json!({ "kind": "boolean", "value": b }),
                DevProp::Number(n) => json!({ "kind": "number", "value": n }),
                DevProp::Text(t) => json!({ "kind": "string", "value": t }),
                DevProp::Object => json!({ "kind": "object" }),
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        sync::mpsc,
        time::Duration,
    };

    use serde_json::json;

    use super::*;

    /// One test: a process has one design host.
    #[test]
    fn a_design_host_talks_to_its_tool_one_line_at_a_time() {
        let path = std::env::temp_dir().join(format!("nsh-design-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let (input_tx, input) = mpsc::channel();
        let (closed_tx, closed) = mpsc::channel();
        let input_tx = Mutex::new(input_tx);
        assert!(connect(
            path.to_string_lossy().into_owned(),
            DesignHooks {
                input: Box::new(move |m| input_tx.lock().unwrap().send(m).unwrap()),
                closed: Box::new(move || closed_tx.send(()).unwrap()),
            },
        ));
        assert!(attached());
        assert!(!connect(
            String::new(),
            DesignHooks {
                input: Box::new(|_| {}),
                closed: Box::new(|| {}),
            },
        ));
        let (mut tool, _) = listener.accept().unwrap();
        tool.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut lines = BufReader::new(tool.try_clone().unwrap()).lines();
        let mut next_line =
            || -> JsonValue { serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap() };
        assert_eq!(next_line(), json!({ "hello": {} }));

        // Input is the platform's, the rest the app's, in order.
        writeln!(
            tool,
            "{}",
            json!({ "pointer": { "id": "a", "kind": "down", "x": 1, "y": 2 } })
        )
        .unwrap();
        writeln!(tool, "{}", json!({ "open": { "id": "a" } })).unwrap();
        writeln!(tool, "not json").unwrap();
        writeln!(tool, "{}", json!({ "close": { "id": "a" } })).unwrap();
        let got = input.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got["pointer"]["kind"], "down");
        let take = || {
            for _ in 0..500 {
                if let Some(m) = INBOX.lock().unwrap().messages.pop_front() {
                    return m;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("no message");
        };
        assert_eq!(take(), json!({ "open": { "id": "a" } }));
        assert_eq!(take(), json!({ "close": { "id": "a" } }));

        // Surfaces: new or changed ones, then the ones gone.
        let a = DesignWindow {
            id: "a".into(),
            handle: ("context", 7),
            size: crate::geometry::size(
                crate::geometry::Points(300.0),
                crate::geometry::Points(200.0),
            ),
        };
        report(std::slice::from_ref(&a));
        assert_eq!(
            next_line(),
            json!({ "surface": { "id": "a", "context": 7, "width": 300.0, "height": 200.0 } })
        );
        report(std::slice::from_ref(&a));
        let b = DesignWindow {
            size: crate::geometry::size(
                crate::geometry::Points(320.0),
                crate::geometry::Points(200.0),
            ),
            ..a
        };
        report(&[b]);
        assert_eq!(next_line()["surface"]["width"], 320.0);
        report(&[]);
        assert_eq!(
            next_line(),
            json!({ "surface": { "id": "a", "gone": true } })
        );

        // What the app sends goes to the tool.
        Design::send(json!({ "called": { "id": "a" } })).unwrap();
        assert_eq!(next_line(), json!({ "called": { "id": "a" } }));
        assert!(Design::send(json!("text")).is_err());

        // The tool goes: the platform stops the app.
        drop(lines);
        drop(tool);
        closed.recv_timeout(Duration::from_secs(5)).unwrap();
        let _ = std::fs::remove_file(path);
    }

    /// The hello carries the run's token when the tool gave one, and is as
    /// it always was without one.
    #[test]
    fn hello_carries_the_token() {
        assert_eq!(hello(None), json!({ "hello": {} }));
        assert_eq!(
            hello(Some("a-token")),
            json!({ "hello": { "token": "a-token" } })
        );
    }
}

#[cfg(test)]
mod format_tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn surface_line_includes_extra_fields() {
        let window = DesignWindow {
            id: "a".into(),
            handle: ("hwnd", 0x1234),
            size: crate::geometry::size(
                crate::geometry::Points(393.0),
                crate::geometry::Points(852.0),
            ),
        };
        assert_eq!(
            surface_line(&window, &[], None),
            json!({ "surface": { "id": "a", "hwnd": 0x1234, "width": 393.0, "height": 852.0 } })
        );
        assert_eq!(
            surface_line(&window, &[("surface", 0x99), ("pid", 4242)], None),
            json!({ "surface": { "id": "a", "hwnd": 0x1234, "width": 393.0, "height": 852.0, "surface": 0x99, "pid": 4242 } })
        );
        assert_eq!(
            surface_line(
                &window,
                &[("surface", 0x99), ("pid", 4242)],
                Some(DesignViewport {
                    x: 100.0,
                    y: 200.0,
                    width: 250.0,
                    height: 150.0,
                    scale: 4.0,
                }),
            ),
            json!({
                "surface": {
                    "id": "a",
                    "hwnd": 0x1234,
                    "width": 393.0,
                    "height": 852.0,
                    "surface": 0x99,
                    "pid": 4242,
                    "viewport": {
                        "x": 100.0,
                        "y": 200.0,
                        "width": 250.0,
                        "height": 150.0,
                        "scale": 4.0,
                    }
                }
            })
        );
        assert!(is_input(
            &json!({ "viewport": { "id": "mounted", "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0, "scale": 4.0 } })
        ));
    }
}
