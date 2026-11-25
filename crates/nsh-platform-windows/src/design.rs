//! Design mode on Windows (dev hosts): the platform's part of `nsh/design`
//! (`nsh_core::runtime::design`, which keeps the connection to the tool
//! and its protocol, `docs/dev/design-mode.md`). A tool draws the app's
//! windows in its own (DirectComposition or DWM thumbnails), so each window's
//! `surface` line carries its `HWND` (`"hwnd"`) and, once its swap chain is
//! made, its shared DirectComposition surface handle (`"surface"`) and owning
//! process id (`"pid"`). The tool listens on the
//! loopback address (`NSH_DESIGN_SOCKET=tcp:127.0.0.1:<port>`) and gives
//! the run a token the hello carries (`NSH_DESIGN_TOKEN`).
//!
//! **Out of sight.** The app's main window never shows, and its own windows
//! show where no display is: the window procedure keeps them there
//! ([`window_message`]), not a timer. Each gets the tool-window and
//! no-activate styles before it first shows (no taskbar button, no place in
//! the window switcher, never the foreground window); every
//! `WM_WINDOWPOSCHANGING` of one, the one that changes its size included,
//! puts its whole rectangle beside the virtual screen (the union of every
//! display's), left of it where its origin stays in the range Windows keeps,
//! else right of it ([`out_of_sight`]); `WM_DISPLAYCHANGE` places them all
//! again. DWM
//! still composes them, so their thumbnails stay live and their frames
//! keep coming. A window lays out at its content's size, which may be far
//! larger than a display; what its swap chain presents is cut to the
//! device's largest texture ([`crate::gfx::presentable`]), its pictures
//! being drawn apart. The 50 ms tick only reports the windows to the tool.
//!
//! **Pictures** (`design.settled`, `design.picture`): [`crate::picture`].
//!
//! **Input** from the tool goes through the handlers the windows' messages
//! go through ([`crate::input`], as the test agent's does), in the window
//! it names: a key as the same key event a real press gives, then its text
//! as the characters `TranslateMessage` would make ([`key`]). The
//! connection's thread hands it to the UI thread
//! ([`crate::window::UiThread`]). When the tool's connection closes, the
//! host ends; the dev session ends with it.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

use nsh_core::{
    NodeId,
    runtime::design::{self as core, DesignWindow},
};
use serde_json::Value;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    UI::{
        HiDpi::AdjustWindowRectExForDpi,
        WindowsAndMessaging::{
            GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetSystemMetrics, GetWindowLongPtrW,
            GetWindowLongW, GetWindowRect, IsWindow, IsWindowVisible, MINMAXINFO,
            SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos,
            WINDOW_EX_STYLE, WINDOW_STYLE, WINDOWPOS, WM_DISPLAYCHANGE, WM_GETMINMAXINFO,
            WM_STYLECHANGING, WM_WINDOWPOSCHANGING, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW,
        },
    },
};

use crate::{
    input::{KeyEvent, Modifiers, PointerKind, WheelEvent},
    surface::Surface,
    view::NshView,
    window::UiThread,
};

/// How far left of the virtual screen a design window is kept, in pixels.
const MARGIN: i32 = 200;

/// The largest a design window may be sized, a side in pixels: past any
/// content laid out at the natural-size limit at any scale. The system's
/// own ceiling (about the virtual screen's size) would cut it.
const LARGEST: i32 = 1 << 20;

thread_local! {
    /// Whether the host is in design mode (the UI thread's).
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    /// The app's own windows by the id they opened with: their root node.
    static NAMED: RefCell<HashMap<String, NodeId>> = RefCell::new(HashMap::new());
    /// The native windows kept out of sight ([`window_message`]), the main
    /// one aside.
    static KEPT: RefCell<Vec<isize>> = const { RefCell::new(Vec::new()) };
    /// The virtual screen a test stands in, in place of the system's
    /// ([`stand_in_screen`]).
    static SCREEN: Cell<Option<RECT>> = const { Cell::new(None) };
    /// The pointer buttons the tool holds down.
    static HELD: Cell<u8> = const { Cell::new(0) };
}

/// Whether the app runs in a design tool ([`start`] connected).
pub(crate) fn active() -> bool {
    ACTIVE.get()
}

/// Window `root` opened as `id`, by which the tool names it, in native
/// window `hwnd`, which has not shown yet: kept out of sight from now on.
pub(crate) fn name_window(id: &str, root: NodeId, hwnd: HWND) {
    NAMED.with_borrow_mut(|named| named.insert(id.to_owned(), root));
    keep(hwnd);
}

/// Starts design mode when a tool started the app (`NSH_DESIGN_SOCKET`):
/// the connection, the pictures and the windows' report. Called once on the
/// UI thread before the app runs.
pub(crate) fn start() {
    let Some(endpoint) = core::endpoint() else {
        return;
    };
    let ui = UiThread::current();
    let hooks = core::DesignHooks {
        input: Box::new(move |message| ui.invoke(move || input(&message))),
        // The tool went: nothing shows the app any more.
        closed: Box::new(|| std::process::exit(0)),
    };
    if !core::connect(endpoint, hooks) {
        return;
    }
    ACTIVE.set(true);
    // Nothing of the app activates a window or takes the foreground: its
    // windows show without activating, and the one that would have the
    // keyboard is kept by the app ([`crate::window::run_in_background`]).
    crate::window::run_in_background();
    crate::picture::install(ui);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(50));
            ui.invoke(tick);
        }
    });
}

/// The window the tool calls `id`: the surface showing it, its root view
/// and its native window.
pub(crate) fn window(id: &str) -> Option<(Rc<Surface>, NshView, HWND)> {
    let root = NAMED.with_borrow(|named| named.get(id).copied())?;
    Surface::all().into_iter().find_map(|surface| {
        let hwnd = crate::app_window::hwnd_of(&surface, root)?;
        let view = crate::app_window::root_view(&surface, root)?;
        Some((surface, view, hwnd))
    })
}

/// The app's surface (one per process for a standalone app) and the root
/// of window `id`.
fn find(id: &str) -> Option<(Rc<Surface>, NodeId)> {
    let root = NAMED.with_borrow(|named| named.get(id).copied())?;
    Surface::all()
        .into_iter()
        .find(|s| crate::app_window::hwnd_of(s, root).is_some())
        .map(|s| (s, root))
}

/// Points per pixel of `hwnd`'s display.
fn scale(hwnd: HWND) -> f64 {
    f64::from(crate::window::dpi(hwnd)) / 96.0
}

/// The virtual screen: the bounds of every display, or what a test stands
/// in for it ([`stand_in_screen`]).
fn screen() -> RECT {
    if let Some(rect) = SCREEN.get() {
        return rect;
    }
    use windows::Win32::UI::WindowsAndMessaging::{SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN};
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

/// Stands `rect` in for the virtual screen (`None`: the system's again),
/// as a display change would make it, until `WM_DISPLAYCHANGE` places the
/// windows for it: a test's ([`crate::dev`]).
pub(crate) fn stand_in_screen(rect: Option<RECT>) {
    SCREEN.set(rect);
}

/// The range a window's origin may lie in, a coordinate in pixels:
/// Windows puts an origin outside it at its end (a window asked at x =
/// -52,000 lands at -32,768), as the 16 bits of `WM_MOVE` hold it.
const ORIGINS: std::ops::RangeInclusive<i32> = -32_768..=32_767;

/// Where a window `size` pixels large (outer) is out of sight: its top
/// left corner, by [`MARGIN`] left of the virtual screen, else right of it,
/// below it or above it, the first of them whose origin Windows keeps
/// ([`ORIGINS`]). A window may be far larger than the displays (16,384
/// points at 300 % are 49,152 pixels), and left of them its origin would
/// be cut to -32,768, bringing it back onto them.
fn out_of_sight(size: (i32, i32)) -> (i32, i32) {
    place_beside(screen(), size)
}

/// [`out_of_sight`] for virtual screen `screen`.
fn place_beside(screen: RECT, size: (i32, i32)) -> (i32, i32) {
    let (w, h) = (size.0.max(0), size.1.max(0));
    [
        (screen.left - w - MARGIN, screen.top),
        (screen.right + MARGIN, screen.top),
        (screen.left, screen.bottom + MARGIN),
        (screen.left, screen.top - h - MARGIN),
    ]
    .into_iter()
    .find(|(x, y)| ORIGINS.contains(x) && ORIGINS.contains(y))
    // Displays reaching both ends of the coordinates: right of them, as
    // far as an origin goes.
    .unwrap_or((
        *ORIGINS.end(),
        screen.top.clamp(*ORIGINS.start(), *ORIGINS.end()),
    ))
}

/// Keeps `hwnd`, a window of the app's own not shown yet, out of sight: its
/// styles now, and its place from its next position change on.
fn keep(hwnd: HWND) {
    let key = hwnd.0 as isize;
    KEPT.with_borrow_mut(|kept| {
        if !kept.contains(&key) {
            kept.push(key);
        }
    });
    // SAFETY: plain calls on a window of this thread. Setting the styles
    // sends `WM_STYLECHANGING`, which keeps them ([`window_message`]), and
    // the move sends `WM_WINDOWPOSCHANGING`, which places it.
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, unseen_style(ex) as isize);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Extended style `ex` of a window kept out of sight: a tool window (no
/// taskbar button, not in the window switcher) that never activates.
fn unseen_style(ex: u32) -> u32 {
    (ex | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) & !WS_EX_APPWINDOW.0
}

/// Whether `hwnd` is one of the windows kept out of sight.
fn kept(hwnd: HWND) -> bool {
    let key = hwnd.0 as isize;
    KEPT.with_borrow(|kept| kept.contains(&key))
}

/// Places every window kept out of sight again (the displays changed).
fn place_all() {
    let kept: Vec<isize> = KEPT.with_borrow_mut(|kept| {
        // SAFETY: a plain query; a handle that is no window answers false.
        kept.retain(|&h| unsafe { IsWindow(Some(HWND(h as *mut _))) }.as_bool());
        kept.clone()
    });
    let main = Surface::all().into_iter().map(|s| s.hwnd().0 as isize);
    for key in kept.into_iter().chain(main) {
        // SAFETY: plain calls on windows of this thread; the move sends
        // `WM_WINDOWPOSCHANGING`, which places it.
        let _ = unsafe {
            SetWindowPos(
                HWND(key as *mut _),
                None,
                0,
                0,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }
}

/// Design mode's part of the window procedure of `hwnd` (`main`: the
/// surface's own window) before the rest of it: `Some` when it handled
/// `msg` itself. A window kept out of sight keeps its styles
/// (`WM_STYLECHANGING`) and its place (`WM_WINDOWPOSCHANGING`, the one that
/// changes its size included: its rectangle never lies on a display), may be
/// as large as its content (`WM_GETMINMAXINFO`), and the main window never
/// shows. `WM_DISPLAYCHANGE` places them all again.
pub(crate) fn window_message(
    hwnd: HWND,
    main: bool,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if !active() {
        return None;
    }
    if msg == WM_DISPLAYCHANGE {
        place_all();
        return None;
    }
    let own = kept(hwnd);
    if !own && !main {
        return None;
    }
    match msg {
        WM_WINDOWPOSCHANGING => {
            // SAFETY: `lparam` is the change's WINDOWPOS, which the window
            // procedure may change.
            let pos = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
            if main {
                // The main window shows nothing a tool draws.
                pos.flags &= !SWP_SHOWWINDOW;
            }
            pos.flags |= SWP_NOACTIVATE;
            let size = if pos.flags.contains(SWP_NOSIZE) {
                let mut rect = RECT::default();
                // SAFETY: a query with a local out value.
                let _ = unsafe { GetWindowRect(hwnd, &mut rect) };
                (rect.right - rect.left, rect.bottom - rect.top)
            } else {
                (pos.cx, pos.cy)
            };
            (pos.x, pos.y) = out_of_sight(size);
            pos.flags &= !SWP_NOMOVE;
            None
        }
        WM_STYLECHANGING if wparam.0 as i32 == GWL_EXSTYLE.0 => {
            // SAFETY: `lparam` is the change's STYLESTRUCT, which the window
            // procedure may change.
            let style = unsafe { &mut *(lparam.0 as *mut STYLESTRUCT) };
            style.styleNew = unseen_style(style.styleNew);
            None
        }
        WM_GETMINMAXINFO if own => {
            // SAFETY: `lparam` is the window's MINMAXINFO.
            let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            // As small and as large as its content: neither the system's
            // smallest window nor the size of the displays.
            info.ptMinTrackSize = POINT { x: 1, y: 1 };
            info.ptMaxTrackSize = POINT {
                x: LARGEST,
                y: LARGEST,
            };
            Some(LRESULT(0))
        }
        _ => None,
    }
}

/// Tells the tool what the app's windows show: those visible, with their
/// handles (including their shared DirectComposition surface handle and
/// owning process id once their swap chain exists) and content sizes.
fn tick() {
    let mut shown: Vec<(DesignWindow, Vec<(&'static str, u64)>)> = Vec::new();
    let named: Vec<(String, NodeId)> =
        NAMED.with_borrow(|named| named.iter().map(|(k, v)| (k.clone(), *v)).collect());
    let pid = u64::from(std::process::id());
    for surface in Surface::all() {
        for (id, root) in &named {
            let Some(hwnd) = crate::app_window::hwnd_of(&surface, *root) else {
                continue;
            };
            // SAFETY: plain call.
            if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                continue;
            }
            let mut rect = RECT::default();
            // SAFETY: plain call.
            let _ = unsafe { GetClientRect(hwnd, &mut rect) };
            let s = scale(hwnd);
            let extra = match crate::app_window::shared_surface_of(&surface, *root) {
                Some(h) => vec![("surface", h.0 as usize as u64), ("pid", pid)],
                None => Vec::new(),
            };
            shown.push((
                DesignWindow {
                    id: id.clone(),
                    handle: ("hwnd", hwnd.0 as isize as u64),
                    size: nsh_core::geometry::size(
                        nsh_core::geometry::Points((f64::from(rect.right - rect.left) / s) as f32),
                        nsh_core::geometry::Points((f64::from(rect.bottom - rect.top) / s) as f32),
                    ),
                },
                extra,
            ));
        }
    }
    // Windows that went: forgotten.
    let surfaces = Surface::all();
    NAMED.with_borrow_mut(|named| {
        named.retain(|_, root| {
            surfaces
                .iter()
                .any(|s| crate::app_window::hwnd_of(s, *root).is_some())
        })
    });
    KEPT.with_borrow_mut(|kept| {
        // SAFETY: a plain query; a handle that is no window answers false.
        kept.retain(|&h| unsafe { IsWindow(Some(HWND(h as *mut _))) }.as_bool())
    });
    let entries: Vec<(&DesignWindow, &[(&'static str, u64)])> = shown
        .iter()
        .map(|(w, extra)| (w, extra.as_slice()))
        .collect();
    core::report_extra(&entries);
}

fn input(message: &Value) {
    let Some((kind, body)) = core::INPUT
        .into_iter()
        .find_map(|kind| message.get(kind).map(|body| (kind, body)))
    else {
        return;
    };
    let Some((surface, root)) = find(body["id"].as_str().unwrap_or("")) else {
        return;
    };
    let n = |name: &str| body[name].as_f64().unwrap_or(0.0) as f32;
    // The surface's input picks views in that window's root.
    let _root = crate::app_window::enter(&surface, root);
    match kind {
        "pointer" => {
            let (kind, held) = match body["kind"].as_str().unwrap_or("") {
                "down" => (PointerKind::Down, 1),
                "up" => (PointerKind::Up, 0),
                "drag" => (PointerKind::Move, 1),
                _ => (PointerKind::Move, HELD.get()),
            };
            HELD.set(held);
            crate::synth::pointer(&surface, kind, n("x"), n("y"), 0, held);
        }
        "wheel" => crate::input::wheel(
            &surface,
            WheelEvent {
                x: n("x"),
                y: n("y"),
                dx: n("dx"),
                dy: n("dy"),
                precise: true,
                modifiers: Modifiers::default(),
                time: surface.clock(),
            },
        ),
        "key" => key(&surface, body),
        _ => {
            if let Some(hwnd) = crate::app_window::hwnd_of(&surface, root) {
                resize(hwnd, n("width").max(1.0), n("height").max(1.0));
            }
        }
    }
}

/// Resizes `hwnd` so that its client area is `w` x `h` points.
fn resize(hwnd: HWND, w: f32, h: f32) {
    let dpi = crate::window::dpi(hwnd);
    let s = dpi as f32 / 96.0;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: (w * s).round() as i32,
        bottom: (h * s).round() as i32,
    };
    // SAFETY: plain calls on a window of this thread.
    unsafe {
        let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
        let ex = WINDOW_EX_STYLE(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32);
        let _ = AdjustWindowRectExForDpi(&mut rect, style, false, ex, dpi);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// A key as the DOM names it (`"Enter"`, `"a"`, `"!"`), as a real press
/// gives it: its virtual key on the US layout (none for a character no key
/// of it types), and the text `TranslateMessage` would make of it. `None`
/// for a name it does not give.
fn virtual_key(name: &str) -> Option<(u16, Option<String>)> {
    let named = |vk: u16| Some((vk, None));
    match name {
        "Enter" => Some((0x0D, Some("\r".into()))),
        "Tab" => Some((0x09, Some("\t".into()))),
        " " => Some((0x20, Some(" ".into()))),
        "Backspace" => named(0x08),
        "Escape" => named(0x1B),
        "PageUp" => named(0x21),
        "PageDown" => named(0x22),
        "End" => named(0x23),
        "Home" => named(0x24),
        "ArrowLeft" => named(0x25),
        "ArrowUp" => named(0x26),
        "ArrowRight" => named(0x27),
        "ArrowDown" => named(0x28),
        "Delete" => named(0x2E),
        _ => {
            if let Some(n) = name
                .strip_prefix('F')
                .and_then(|n| n.parse::<u16>().ok())
                .filter(|n| (1..=12).contains(n) && name.len() <= 3)
            {
                return named(0x6F + n);
            }
            let mut chars = name.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Some((us_key(c).unwrap_or(0), Some(name.to_owned())))
        }
    }
}

/// The US layout's key that types `c`, with Shift or without (`a` and `A`
/// the A key, `!` the 1 key): `None` for a character none types.
fn us_key(c: char) -> Option<u16> {
    use nsh_core::keymap::{Key, shifted};
    if !c.is_ascii_graphic() {
        return None;
    }
    Key::Char(c.to_ascii_lowercase()).virtual_key().or_else(|| {
        (b'!'..=b'~')
            .map(char::from)
            .find(|&base| base != c && shifted(base) == c)
            .and_then(|base| Key::Char(base).virtual_key())
    })
}

/// A key going down or up, as a real press: the key event, then, going
/// down, the text it types where nothing took the key and neither Control
/// nor Alt is held.
fn key(surface: &Rc<Surface>, k: &Value) {
    let name = k["key"].as_str().unwrap_or("");
    let Some((vk, text)) = virtual_key(name) else {
        log::debug!(target: "nsh::design", "a design tool's key {name:?} is none a keyboard gives; dropped");
        return;
    };
    let held = |name: &str| k[name].as_bool().unwrap_or(false);
    let modifiers = Modifiers {
        shift: held("shift"),
        ctrl: held("control"),
        alt: held("alt"),
        meta: held("meta"),
    };
    let down = k["down"].as_bool().unwrap_or(true);
    let handled = vk != 0
        && crate::input::key(
            surface,
            KeyEvent {
                vk,
                down,
                repeat: held("repeat"),
                modifiers,
                scan: 0,
                extended: false,
                sys: modifiers.alt,
                translated: false,
            },
        );
    if down
        && !handled
        && !modifiers.ctrl
        && !modifiers.alt
        && let Some(text) = text
    {
        crate::text_input::char_input(surface, &text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The keys a design tool sends map to the US layout's virtual keys, a
    /// character no key types to none, and other names to nothing.
    #[test]
    fn a_design_tools_keys_map_to_virtual_keys() {
        assert_eq!(virtual_key("a"), Some((0x41, Some("a".into()))));
        assert_eq!(virtual_key("A"), Some((0x41, Some("A".into()))));
        assert_eq!(virtual_key("!"), Some((0x31, Some("!".into()))));
        assert_eq!(virtual_key("?"), Some((0xBF, Some("?".into()))));
        assert_eq!(virtual_key("~"), Some((0xC0, Some("~".into()))));
        assert_eq!(virtual_key("F1"), Some((0x70, None)));
        assert_eq!(virtual_key("F12"), Some((0x7B, None)));
        assert_eq!(virtual_key("F13"), None);
        assert_eq!(virtual_key("PageDown"), Some((0x22, None)));
        assert_eq!(virtual_key("é"), Some((0, Some("é".into()))));
        assert_eq!(virtual_key("Hyper"), None);
        assert_eq!(virtual_key(""), None);
        for c in (b'!'..=b'~').map(char::from) {
            assert!(us_key(c).is_some(), "{c:?} has no key");
        }
    }

    /// A window is placed beside the displays where its origin stays in
    /// the range Windows keeps: left of them while it fits there, else
    /// right of them, and never on them.
    #[test]
    fn a_window_is_placed_where_its_origin_is_kept() {
        let screen = |left, right| RECT {
            left,
            top: 0,
            right,
            bottom: 1080,
        };
        let main = screen(0, 1920);
        assert_eq!(place_beside(main, (480, 360)), (-680, 0));
        // 16,384 points at 150 %, 200 %, 300 %.
        assert_eq!(place_beside(main, (24_576, 1500)), (-24_776, 0));
        assert_eq!(place_beside(main, (32_768, 2000)), (2120, 0));
        assert_eq!(place_beside(main, (49_152, 3000)), (2120, 0));
        // A display left of the main one.
        let wide = screen(-2560, 1920);
        assert_eq!(place_beside(wide, (32_768, 2000)), (2120, 0));
        assert_eq!(place_beside(wide, (480, 360)), (-3240, 0));
        for (screen, size) in [(main, (49_152, 3000)), (wide, (24_576, 1500))] {
            let (x, y) = place_beside(screen, size);
            assert!(ORIGINS.contains(&x) && ORIGINS.contains(&y));
            assert!(x + size.0 <= screen.left || x >= screen.right || y >= screen.bottom);
        }
    }
}
