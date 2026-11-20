//! Design mode on Windows (dev hosts): the platform's part of `nsh/design`
//! (`nsh_core::runtime::design`, which keeps the connection to the tool
//! and its protocol, `docs/dev/design-mode.md`). A tool draws the app's
//! windows in its own with DWM thumbnails (`DwmRegisterThumbnail`), so
//! each window's surface handle is its `HWND` (`"hwnd"`).
//!
//! The app's main window stays hidden. Its own windows are moved off every
//! display and out of the taskbar and the window switcher: DWM still
//! composes them, so their thumbnails stay live and their frames keep
//! coming. Input from the tool goes through the handlers the windows'
//! messages go through ([`crate::input`], as the test agent's does), in
//! the window it names. The connection's thread hands it to the UI thread
//! ([`crate::window::UiThread`]). When the tool's connection closes, the
//! host ends; the dev session ends with it.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use nsh_core::{
    NodeId,
    runtime::design::{self as core, DesignWindow},
};
use serde_json::Value;
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::{
        HiDpi::AdjustWindowRectExForDpi,
        WindowsAndMessaging::{
            GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetSystemMetrics, GetWindowLongPtrW,
            GetWindowLongW, IsWindowVisible, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_HIDE,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
            WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        },
    },
};

use crate::{
    input::{KeyEvent, Modifiers, PointerKind, WheelEvent},
    surface::Surface,
    window::UiThread,
};

thread_local! {
    /// Whether the host is in design mode (the UI thread's).
    static ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The app's own windows by the id they opened with: their root node.
    static NAMED: RefCell<HashMap<String, NodeId>> = RefCell::new(HashMap::new());
    /// The windows already moved out of sight.
    static TREATED: RefCell<Vec<isize>> = const { RefCell::new(Vec::new()) };
    /// The pointer buttons the tool holds down.
    static HELD: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// Whether the app runs in a design tool ([`start`] connected).
pub(crate) fn active() -> bool {
    ACTIVE.get()
}

/// Window `root` opened as `id`, by which the tool names it.
pub(crate) fn name_window(id: &str, root: NodeId) {
    NAMED.with_borrow_mut(|named| named.insert(id.to_owned(), root));
}

/// Starts design mode when a tool started the app (`NSH_DESIGN_SOCKET`):
/// the connection and the windows' watch. Called once on the UI thread
/// before the app runs.
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
    let ui = UiThread::current();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(50));
            ui.invoke(tick);
        }
    });
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

/// Moves `hwnd` off every display, out of the taskbar and the switcher.
fn unseen(hwnd: HWND) {
    // SAFETY: plain calls on a window of this thread.
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = (ex | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) & !WS_EX_APPWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN) - (rect.right - rect.left) - 200;
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Keeps the main window hidden and the others unseen, and tells the tool
/// what it draws.
fn tick() {
    let mut shown = Vec::new();
    let named: Vec<(String, NodeId)> =
        NAMED.with_borrow(|named| named.iter().map(|(k, v)| (k.clone(), *v)).collect());
    for surface in Surface::all() {
        let main = surface.hwnd();
        // SAFETY: plain calls on the surface's window.
        if unsafe { IsWindowVisible(main) }.as_bool() {
            let _ = unsafe { ShowWindow(main, SW_HIDE) };
        }
        for (id, root) in &named {
            let Some(hwnd) = crate::app_window::hwnd_of(&surface, *root) else {
                continue;
            };
            // SAFETY: plain call.
            if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                continue;
            }
            let key = hwnd.0 as isize;
            if TREATED.with_borrow_mut(|t| {
                let new = !t.contains(&key);
                if new {
                    t.push(key);
                }
                new
            }) {
                unseen(hwnd);
            }
            let mut rect = RECT::default();
            // SAFETY: plain call.
            let _ = unsafe { GetClientRect(hwnd, &mut rect) };
            let s = scale(hwnd);
            shown.push(DesignWindow {
                id: id.clone(),
                handle: ("hwnd", key as u64),
                size: nsh_core::geometry::size(
                    nsh_core::geometry::Points((f64::from(rect.right - rect.left) / s) as f32),
                    nsh_core::geometry::Points((f64::from(rect.bottom - rect.top) / s) as f32),
                ),
            });
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
    core::report(&shown);
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

/// The virtual key of a key as the DOM names it, and its text.
fn virtual_key(name: &str) -> Option<(u16, Option<String>)> {
    Some(match name {
        "Enter" => (0x0D, Some("\r".into())),
        "Backspace" => (0x08, None),
        "Tab" => (0x09, Some("\t".into())),
        "Escape" => (0x1B, None),
        "Delete" => (0x2E, None),
        "ArrowUp" => (0x26, None),
        "ArrowDown" => (0x28, None),
        "ArrowLeft" => (0x25, None),
        "ArrowRight" => (0x27, None),
        "Home" => (0x24, None),
        "End" => (0x23, None),
        " " => (0x20, Some(" ".into())),
        other if other.chars().count() == 1 => {
            let c = other.chars().next()?;
            let vk = match c.to_ascii_uppercase() {
                u @ ('A'..='Z' | '0'..='9') => u as u16,
                _ => 0,
            };
            (vk, Some(other.to_owned()))
        }
        _ => return None,
    })
}

/// A key going down or up, with its text typed as it goes down.
fn key(surface: &Rc<Surface>, k: &Value) {
    let Some((vk, text)) = virtual_key(k["key"].as_str().unwrap_or("")) else {
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
                sys: false,
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
