//! Design mode on Linux (dev hosts): the platform's part of `nsh/design`
//! (`nsh_core::runtime::design`, which keeps the connection to the tool
//! and its protocol, `docs/dev/design-mode.md`). Under X11 a tool draws the
//! app's windows in its own with the Composite extension, so each window's
//! surface handle is its X11 window (`"xid"`). Wayland lets no other
//! process draw a window: there the windows are not reported, and the host
//! says so once.
//!
//! The app's main window stays hidden. Its own windows are moved off every
//! display and out of the taskbar and pager: still mapped, so the
//! compositor keeps their pictures and their frames keep coming. Input from
//! the tool goes through the handlers real input goes through
//! ([`crate::input::simulate`], as the test agent's does), in the window it
//! names. GDK's X11 functions and libX11's are looked up at run time
//! (`dlsym`, `dlopen`), as `crate::xtest` does, so the crate links no X11
//! library. When the tool's connection closes, the app stops with the dev
//! session that runs it (its process group).

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CStr, c_char, c_int, c_ulong, c_void},
    rc::Rc,
    time::Duration,
};

use gtk4::{self as gtk, glib, prelude::*};
use nsh_core::{
    NodeId,
    runtime::design::{self as core, DesignWindow},
};
use serde_json::Value;

use crate::{input::Simulated, surface::Surface};

const RTLD_NOW: c_int = 2;

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

thread_local! {
    /// Whether the host is in design mode (the main thread's).
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    /// The app's own windows by the id they opened with: their root node.
    static NAMED: RefCell<HashMap<String, NodeId>> = RefCell::new(HashMap::new());
    /// The X11 windows already moved out of sight.
    static TREATED: RefCell<Vec<c_ulong>> = const { RefCell::new(Vec::new()) };
    /// The display is not X11's: said once.
    static NOT_X11: Cell<bool> = const { Cell::new(false) };
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
/// the connection and the windows' watch. Called once on the main thread
/// before the app runs.
pub(crate) fn start() {
    let Some(endpoint) = core::endpoint() else {
        return;
    };
    let hooks = core::DesignHooks {
        input: Box::new(|message| {
            glib::MainContext::default().invoke(move || input(&message));
        }),
        // The tool went: nothing shows the app any more, so it stops with
        // the dev session that runs it.
        closed: Box::new(|| {
            // SAFETY: SIGTERM to this process's own group.
            unsafe { kill(0, 15) };
        }),
    };
    if !core::connect(endpoint, hooks) {
        return;
    }
    ACTIVE.set(true);
    glib::timeout_add_local(Duration::from_millis(50), || {
        tick();
        glib::ControlFlow::Continue
    });
}

/// A symbol from `lib` (or the process' own, with a null `lib`).
///
/// # Safety
///
/// `T` must be the function type of `name`.
unsafe fn sym<T: Copy>(lib: *mut c_void, name: &CStr) -> Option<T> {
    // SAFETY: the caller names a function of type `T`.
    unsafe {
        let p = dlsym(lib, name.as_ptr());
        (!p.is_null()).then(|| std::mem::transmute_copy::<*mut c_void, T>(&p))
    }
}

/// libX11, loaded once.
fn x11() -> *mut c_void {
    thread_local! {
        static LIB: Cell<usize> = const { Cell::new(0) };
    }
    if LIB.get() == 0 {
        // SAFETY: a plain dlopen; the library stays loaded.
        let lib = unsafe { dlopen(c"libX11.so.6".as_ptr(), RTLD_NOW) };
        LIB.set(lib as usize);
    }
    LIB.get() as *mut c_void
}

/// The X11 window of `window`, under X11.
fn xid(window: &gtk::Window) -> Option<c_ulong> {
    let surface = window.surface()?;
    if surface.display().type_().name() != "GdkX11Display" {
        if !NOT_X11.replace(true) {
            log::warn!(target: "nsh::design", "design mode draws windows under X11 only; this display is not X11's");
        }
        return None;
    }
    // SAFETY: GDK's X11 function, looked up in the GTK already loaded, on
    // an X11 surface.
    unsafe {
        let get: unsafe extern "C" fn(*mut c_void) -> c_ulong =
            sym(std::ptr::null_mut(), c"gdk_x11_surface_get_xid")?;
        Some(get(surface.as_ptr().cast()))
    }
}

/// Moves `window` (X11 window `xid`) off every display, out of the taskbar
/// and the pager.
fn unseen(window: &gtk::Window, xid: c_ulong) {
    let Some(surface) = window.surface() else {
        return;
    };
    // SAFETY: GDK's and libX11's functions, looked up by name, called on
    // GDK's own connection on the main thread.
    unsafe {
        type Hint = unsafe extern "C" fn(*mut c_void, glib::ffi::gboolean);
        for name in [
            c"gdk_x11_surface_set_skip_taskbar_hint",
            c"gdk_x11_surface_set_skip_pager_hint",
        ] {
            if let Some(hint) = sym::<Hint>(std::ptr::null_mut(), name) {
                hint(surface.as_ptr().cast(), glib::ffi::GTRUE);
            }
        }
        let Some(get_display): Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void> =
            sym(std::ptr::null_mut(), c"gdk_x11_display_get_xdisplay")
        else {
            return;
        };
        let lib = x11();
        if lib.is_null() {
            return;
        }
        type MoveWindow = unsafe extern "C" fn(*mut c_void, c_ulong, c_int, c_int) -> c_int;
        type Flush = unsafe extern "C" fn(*mut c_void) -> c_int;
        let (Some(move_window), Some(flush)) = (
            sym::<MoveWindow>(lib, c"XMoveWindow"),
            sym::<Flush>(lib, c"XFlush"),
        ) else {
            return;
        };
        let display = get_display(surface.display().as_ptr().cast());
        let width = window.width().max(1);
        move_window(display, xid, -width - 200, 0);
        flush(display);
    }
}

/// The app's surfaces, by the windows it has.
fn surfaces() -> Vec<Rc<Surface>> {
    Surface::all()
}

/// The surface that has window `id`, and its root.
fn find(id: &str) -> Option<(Rc<Surface>, NodeId, gtk::Window)> {
    let root = NAMED.with_borrow(|named| named.get(id).copied())?;
    surfaces().into_iter().find_map(|s| {
        let window = crate::window::window_of(&s, root)?;
        Some((s, root, window))
    })
}

/// Keeps the main window hidden and the others unseen, and tells the tool
/// what it draws.
fn tick() {
    let mut shown = Vec::new();
    let named: Vec<(String, NodeId)> =
        NAMED.with_borrow(|named| named.iter().map(|(k, v)| (k.clone(), *v)).collect());
    for surface in surfaces() {
        if let Some(main) = crate::window::window_of(&surface, nsh_core::ROOT_ID)
            && main.is_visible()
        {
            main.set_visible(false);
        }
        for (id, root) in &named {
            let Some(window) = crate::window::window_of(&surface, *root) else {
                continue;
            };
            if !window.is_visible() {
                continue;
            }
            let Some(xid) = xid(&window) else {
                continue;
            };
            if TREATED.with_borrow_mut(|t| {
                let new = !t.contains(&xid);
                if new {
                    t.push(xid);
                }
                new
            }) {
                unseen(&window, xid);
            }
            // An X11 window id is 32 bits wide on some targets.
            #[allow(clippy::useless_conversion)]
            let xid = u64::from(xid);
            shown.push(DesignWindow {
                id: id.clone(),
                handle: ("xid", xid),
                size: nsh_core::geometry::size(
                    nsh_core::geometry::Points(window.width() as f32),
                    nsh_core::geometry::Points(window.height() as f32),
                ),
            });
        }
    }
    // Windows that went: forgotten.
    NAMED.with_borrow_mut(|named| named.retain(|_, root| find_root(*root)));
    core::report(&shown);
}

/// Whether a window with root `root` is still open (it may be hidden).
fn find_root(root: NodeId) -> bool {
    surfaces()
        .iter()
        .any(|s| crate::window::window_of(s, root).is_some())
}

fn input(message: &Value) {
    let Some((kind, body)) = core::INPUT
        .into_iter()
        .find_map(|kind| message.get(kind).map(|body| (kind, body)))
    else {
        return;
    };
    let Some((surface, root, window)) = find(body["id"].as_str().unwrap_or("")) else {
        return;
    };
    let n = |name: &str| body[name].as_f64().unwrap_or(0.0) as f32;
    // Pointer input aims at that window's root.
    crate::input::set_event_root(&surface, crate::window::root_view(&surface, root));
    match kind {
        "pointer" => {
            let (x, y) = (n("x"), n("y"));
            let action = match body["kind"].as_str().unwrap_or("") {
                "down" => Simulated::Down(x, y),
                "up" => Simulated::Up(x, y),
                "drag" => Simulated::Move(x, y),
                _ => Simulated::Hover(x, y),
            };
            crate::input::simulate(&surface, action);
        }
        "wheel" => {
            crate::input::simulate(&surface, Simulated::Wheel(n("x"), n("y"), n("dx"), n("dy")))
        }
        "key" => key(&window, body),
        _ => window.set_default_size(n("width").max(1.0) as i32, n("height").max(1.0) as i32),
    }
    crate::input::set_event_root(&surface, None);
}

/// A key as the DOM names it, typed into `window`'s focused widget when it
/// goes down: text, Enter and Backspace.
fn key(window: &gtk::Window, k: &Value) {
    if !k["down"].as_bool().unwrap_or(true) {
        return;
    }
    let held = |name: &str| k[name].as_bool().unwrap_or(false);
    if held("control") || held("alt") || held("meta") {
        return;
    }
    match k["key"].as_str().unwrap_or("") {
        "Enter" => crate::synth::enter(window),
        "Backspace" => crate::synth::backspace(window),
        text if text.chars().count() == 1 => crate::synth::type_text(window, text),
        _ => {}
    }
}
