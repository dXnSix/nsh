//! Design mode on macOS (dev hosts): the platform's part of `nsh/design`
//! (`nsh_core::runtime::design`, which keeps the connection to the tool
//! and its protocol). A tool draws the app's windows in its own through
//! Core Animation's `CALayerHost`, so each window's surface handle is its
//! layer's `CAContext` id (`"context"`).
//!
//! The app's main window stays hidden, and its own windows unseen:
//! transparent, click-through and still on screen, so that their display
//! links tick and animations run. The app has no Dock icon or menu bar and
//! no App Nap. Input from the tool is given to the views straight, as
//! AppKit gives it. When the tool's connection closes the app stops, with
//! the dev session that runs it (its process group). `NSH_DESIGN_ALPHA` (0
//! to 1) shows the windows, for looking at what the tool draws.

use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
    ffi::c_void,
    ptr::null_mut,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicPtr, Ordering},
    },
    time::Duration,
};

use nsh_core::runtime::design::{self as core, DesignWindow};
use objc2::{
    MainThreadMarker,
    encode::{Encoding, RefEncode},
    msg_send,
    rc::Retained,
    runtime::AnyObject,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventType, NSView, NSWindow,
};
use objc2_foundation::{NSPoint, NSProcessInfo, NSRect, NSSize, NSString};
use serde_json::Value;

/// Whether this host is in design mode (set once, at start).
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Whether the app runs in a design tool ([`start`] connected).
pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// A `CGEventRef`'s target.
#[repr(C)]
struct CgEvent {
    _private: [u8; 0],
}
// SAFETY: an opaque CoreGraphics type, passed by pointer.
unsafe impl RefEncode for CgEvent {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
}

/// A `CFRunLoopSourceContext` (version 0): only `perform` is used.
#[repr(C)]
struct SourceContext {
    version: isize,
    info: *mut c_void,
    unused: [*const c_void; 7],
    perform: extern "C" fn(*mut c_void),
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateScrollWheelEvent2(
        source: *const c_void,
        units: u32,
        count: u32,
        wheel1: i32,
        wheel2: i32,
        wheel3: i32,
    ) -> *mut CgEvent;
    fn CGEventSetLocation(event: *mut CgEvent, location: NSPoint);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: *const c_void);
    fn CFRunLoopGetMain() -> *mut c_void;
    fn CFRunLoopSourceCreate(
        allocator: *const c_void,
        order: isize,
        context: *mut SourceContext,
    ) -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopSourceSignal(source: *mut c_void);
    fn CFRunLoopWakeUp(run_loop: *mut c_void);
    static kCFRunLoopCommonModes: *const c_void;
}

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

/// The tool's input waiting for the main thread, and the run-loop source
/// that hands it over. A source in the common modes, not the main dispatch
/// queue: an AppKit control follows the mouse in a run loop of its own
/// inside `mouseDown:`, and that loop serves sources but not the dispatch
/// queue, so the drags and the release still get in.
static INBOX: Mutex<VecDeque<Value>> = Mutex::new(VecDeque::new());
static SOURCE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

/// Starts design mode when a tool started the app (`NSH_DESIGN_SOCKET`):
/// the connection, the input source and the windows' watch. Called once by
/// the host before the app runs.
pub(crate) fn start(mtm: MainThreadMarker) {
    let Some(endpoint) = core::endpoint() else {
        return;
    };
    // SAFETY: a source with a static perform function and no info, added
    // to the main run loop for the process's life, before the connection
    // can signal it.
    unsafe {
        let mut context = SourceContext {
            version: 0,
            info: null_mut(),
            unused: [std::ptr::null(); 7],
            perform: deliver,
        };
        let source = CFRunLoopSourceCreate(std::ptr::null(), 0, &raw mut context);
        CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
        SOURCE.store(source, Ordering::SeqCst);
    }
    let hooks = core::DesignHooks {
        input: Box::new(|message| {
            INBOX
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push_back(message);
            // SAFETY: the source made above, never released.
            unsafe {
                CFRunLoopSourceSignal(SOURCE.load(Ordering::SeqCst));
                CFRunLoopWakeUp(CFRunLoopGetMain());
            }
        }),
        // The tool went (it quit, or closed the project): nothing shows the
        // app any more, so it stops with the dev session that runs it.
        closed: Box::new(|| {
            // SAFETY: SIGTERM to this process's own group.
            unsafe { kill(0, 15) };
        }),
    };
    if !core::connect(endpoint, hooks) {
        return;
    }
    ACTIVE.store(true, Ordering::Relaxed);
    treat_app(mtm);
    // The windows: found, made unseen and reported, from the main thread.
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(Duration::from_millis(50));
            dispatch2::DispatchQueue::main().exec_async(tick);
        }
    });
}

thread_local! {
    /// The windows already made unseen, by number.
    static TREATED: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
    /// The view that took the press: drags and the release are its.
    static PRESSED: RefCell<Option<Retained<NSView>>> = const { RefCell::new(None) };
    /// The press has not returned: a control is following the mouse in a
    /// loop of its own, which takes the drags and the release from the
    /// app's event queue.
    static TRACKING: Cell<bool> = const { Cell::new(false) };
}

/// No Dock icon, no menu bar of its own, and no App Nap, which would slow a
/// background app's timers and frames.
fn treat_app(mtm: MainThreadMarker) {
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let reason = NSString::from_str("Shown live in a design tool");
    // NSActivityUserInitiated without idle system sleep prevention
    // (`NSActivityIdleSystemSleepDisabled`, bit 20): timers and frames run
    // at full rate in the background.
    let options = 0x00FF_FFFFu64 & !(1u64 << 20);
    // SAFETY: `beginActivityWithOptions:reason:` returns an autoreleased
    // token, retained for the process's life.
    unsafe {
        let activity: *mut AnyObject = msg_send![&*NSProcessInfo::processInfo(), beginActivityWithOptions: options, reason: &*reason];
        let _: *mut AnyObject = msg_send![activity, retain];
    }
}

/// The identifier design mode gives a window of the app's own: its `id`
/// ([`crate::window`] sets it as it opens one).
fn id_of(window: &NSWindow) -> Option<String> {
    // SAFETY: `-[NSWindow identifier]` (NSUserInterfaceItemIdentification)
    // returns an NSString or nil.
    let id: Option<Retained<NSString>> = unsafe { msg_send![window, identifier] };
    id.map(|id| id.to_string()).filter(|id| !id.is_empty())
}

/// Gives `window`, a window of the app's own opened as `id`, that id, by
/// which the tool names it.
pub(crate) fn name_window(window: &NSWindow, id: &str) {
    let id = NSString::from_str(id);
    // SAFETY: `-[NSWindow setIdentifier:]` (NSUserInterfaceItemIdentification).
    let _: () = unsafe { msg_send![window, setIdentifier: &*id] };
}

/// The app's window `id`, its content view and the content's frame.
fn window(
    mtm: MainThreadMarker,
    id: &str,
) -> Option<(Retained<NSWindow>, Retained<NSView>, NSRect)> {
    let windows = NSApplication::sharedApplication(mtm).windows();
    let window = windows.iter().find(|w| id_of(w).as_deref() == Some(id))?;
    let content = window.contentView()?;
    let frame = content.frame();
    Some((window, content, frame))
}

/// A window of the app's own stays on screen, unseen: macOS gives a
/// window's display link (the clock of the app's animations) frames only
/// while some of it is on a screen and not covered. So it is transparent,
/// lets every click through, floats above the other windows (nothing
/// covers it), is on every Space, and is left out of the window cycle and
/// Mission Control. No title bar: its context, which the tool draws, is
/// its content alone.
fn unseen(window: &NSWindow) {
    let size = window
        .contentView()
        .map_or(NSSize::new(1.0, 1.0), |c| c.frame().size);
    window.setStyleMask(objc2_app_kit::NSWindowStyleMask::Borderless);
    window.setContentSize(size);
    let alpha = std::env::var("NSH_DESIGN_ALPHA")
        .ok()
        .and_then(|a| a.parse::<f64>().ok())
        .unwrap_or(0.0);
    window.setAlphaValue(alpha);
    window.setIgnoresMouseEvents(true);
    window.setHasShadow(false);
    // NSStatusWindowLevel.
    window.setLevel(25);
    window.setCollectionBehavior(
        objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
            | objc2_app_kit::NSWindowCollectionBehavior::Transient
            | objc2_app_kit::NSWindowCollectionBehavior::IgnoresCycle
            | objc2_app_kit::NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    window.orderFrontRegardless();
}

/// Finds the app's windows, keeps the main one hidden and the others
/// unseen, and tells the tool what it draws.
fn tick() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let mut shown = Vec::new();
    for window in NSApplication::sharedApplication(mtm).windows().iter() {
        let number = window.windowNumber();
        let Some(id) = id_of(&window) else {
            // The main window, and the panels AppKit makes: nothing the
            // tool shows. NSH shows the main window once its first frame is
            // in, so it is hidden whenever it shows.
            let big = window.contentView().is_some_and(|c| {
                let size = c.frame().size;
                size.width > 200.0 && size.height > 200.0
            });
            if window.isVisible() && big {
                if TREATED.with_borrow_mut(|t| t.insert(number)) {
                    window.setAlphaValue(0.0);
                    window.setIgnoresMouseEvents(true);
                }
                window.orderOut(None);
            }
            continue;
        };
        if !window.isVisible() {
            continue;
        }
        if TREATED.with_borrow_mut(|t| t.insert(number)) {
            unseen(&window);
        }
        let Some(content) = window.contentView() else {
            continue;
        };
        let Some(context) = context_id(&content).filter(|c| *c != 0) else {
            continue;
        };
        let size = content.frame().size;
        shown.push(DesignWindow {
            id,
            handle: ("context", u64::from(context)),
            width: size.width,
            height: size.height,
        });
    }
    core::report(&shown);
}

/// The Core Animation context of `content`'s layer, which another process
/// draws with `CALayerHost` (`CAContext`, private).
fn context_id(content: &NSView) -> Option<u32> {
    let layer = content.layer()?;
    // SAFETY: `-[CALayer context]` and `-[CAContext contextId]` are private
    // methods every layer-backed window's layer answers on macOS 10.5+.
    unsafe {
        let context: *mut AnyObject = msg_send![&*layer, context];
        if context.is_null() {
            return None;
        }
        let id: u32 = msg_send![context, contextId];
        Some(id)
    }
}

/// The waiting input, one message at a time: handling one may run a
/// control's mouse loop, inside which this is called again for the next.
extern "C" fn deliver(_: *mut c_void) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    loop {
        let next = INBOX.lock().unwrap_or_else(|e| e.into_inner()).pop_front();
        let Some(message) = next else { return };
        input(mtm, &message);
    }
}

fn input(mtm: MainThreadMarker, message: &Value) {
    let Some((kind, body)) = ["pointer", "wheel", "key", "resize"]
        .into_iter()
        .find_map(|kind| message.get(kind).map(|body| (kind, body)))
    else {
        return;
    };
    let Some((window, content, frame)) = window(mtm, body["id"].as_str().unwrap_or("")) else {
        return;
    };
    let n = |name: &str| body[name].as_f64().unwrap_or(0.0);
    match kind {
        "pointer" => pointer(
            mtm,
            &window,
            &content,
            frame,
            body["kind"].as_str().unwrap_or(""),
            n("x"),
            n("y"),
        ),
        "wheel" => wheel(mtm, &content, frame, n("x"), n("y"), n("dx"), n("dy")),
        "key" => key(&window, body),
        _ => {
            // The window's size is the content's: it lays out at it.
            let width = body["width"].as_f64().unwrap_or(frame.size.width).max(1.0);
            let height = body["height"]
                .as_f64()
                .unwrap_or(frame.size.height)
                .max(1.0);
            if (width, height) != (frame.size.width, frame.size.height) {
                window.setContentSize(NSSize::new(width, height));
            }
        }
    }
}

/// The window point and the view under (`x`, `y`), measured from the
/// content's top left.
fn locate(content: &NSView, frame: NSRect, x: f64, y: f64) -> (NSPoint, Option<Retained<NSView>>) {
    let local = NSPoint::new(
        x,
        if content.isFlipped() {
            y
        } else {
            frame.size.height - y
        },
    );
    let in_window = content.convertPoint_toView(local, None);
    // SAFETY: the view's superview, read on the main thread.
    let parent = unsafe { content.superview() };
    let in_parent = parent.map_or(in_window, |parent| {
        parent.convertPoint_fromView(in_window, None)
    });
    (in_window, content.hitTest(in_parent))
}

fn now() -> f64 {
    NSProcessInfo::processInfo().systemUptime()
}

/// A mouse event given straight to a view: AppKit drops the mouse events
/// it is asked to route to a window that ignores the mouse. The view that
/// took the press gets the drags and the release, as AppKit would give
/// them.
fn pointer(
    mtm: MainThreadMarker,
    window: &NSWindow,
    content: &NSView,
    frame: NSRect,
    kind: &str,
    x: f64,
    y: f64,
) {
    let (in_window, hit) = locate(content, frame, x, y);
    let (event_type, pressure) = match kind {
        "down" => (NSEventType::LeftMouseDown, 1.0),
        "up" => (NSEventType::LeftMouseUp, 0.0),
        "drag" => (NSEventType::LeftMouseDragged, 1.0),
        _ => (NSEventType::MouseMoved, 0.0),
    };
    let Some(event) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        event_type,
        in_window,
        objc2_app_kit::NSEventModifierFlags::empty(),
        now(),
        window.windowNumber(),
        None,
        0,
        1,
        pressure,
    ) else {
        return;
    };
    match kind {
        "down" => {
            let Some(hit) = hit else { return };
            PRESSED.set(Some(hit.clone()));
            // An NSH view returns at once; an AppKit control (a text field,
            // its editor) stays in here, following the mouse, until its
            // release.
            TRACKING.set(true);
            hit.mouseDown(&event);
            let released_inside = !TRACKING.replace(false);
            if released_inside {
                PRESSED.set(None);
            }
        }
        "drag" | "up" if TRACKING.get() => {
            // Inside a control's own mouse loop: it reads the app's queue.
            NSApplication::sharedApplication(mtm).postEvent_atStart(&event, false);
            if kind == "up" {
                TRACKING.set(false);
            }
        }
        "drag" => {
            if let Some(view) = PRESSED.with_borrow(Clone::clone) {
                view.mouseDragged(&event);
            }
        }
        "up" => {
            if let Some(view) = PRESSED.take() {
                view.mouseUp(&event);
            }
        }
        _ => {
            if let Some(hit) = hit {
                hit.mouseMoved(&event);
            }
        }
    }
}

/// A scroll of (`dx`, `dy`) points (content moves up for a positive `dy`),
/// given to the view under the point.
fn wheel(mtm: MainThreadMarker, content: &NSView, frame: NSRect, x: f64, y: f64, dx: f64, dy: f64) {
    let (in_window, hit) = locate(content, frame, x, y);
    let Some(hit) = hit else { return };
    // SAFETY: a scroll event made and released here; its location is set
    // before AppKit wraps it.
    unsafe {
        let event = CGEventCreateScrollWheelEvent2(
            std::ptr::null(),
            0,
            2,
            (-dy).round() as i32,
            (-dx).round() as i32,
            0,
        );
        if event.is_null() {
            return;
        }
        // An event with no window reads its screen point as its window
        // point: place it so that the two are the same.
        let height = objc2_app_kit::NSScreen::screens(mtm)
            .firstObject()
            .map_or(frame.size.height, |s| s.frame().size.height);
        CGEventSetLocation(event, NSPoint::new(in_window.x, height - in_window.y));
        let ns: Option<Retained<NSEvent>> =
            msg_send![objc2::class!(NSEvent), eventWithCGEvent: event];
        if let Some(ns) = ns {
            hit.scrollWheel(&ns);
        }
        CFRelease(event as *const c_void);
    }
}

/// A key, as the DOM names it, given to the window, whose focused view
/// takes it.
fn key(window: &NSWindow, k: &Value) {
    let name = k["key"].as_str().unwrap_or("");
    let (characters, code): (String, u16) = match name {
        "Enter" => ("\r".into(), 36),
        "Backspace" => ("\u{7f}".into(), 51),
        "Tab" => ("\t".into(), 48),
        "Escape" => ("\u{1b}".into(), 53),
        "Delete" => ("\u{f728}".into(), 117),
        "ArrowUp" => ("\u{f700}".into(), 126),
        "ArrowDown" => ("\u{f701}".into(), 125),
        "ArrowLeft" => ("\u{f702}".into(), 123),
        "ArrowRight" => ("\u{f703}".into(), 124),
        "Home" => ("\u{f729}".into(), 115),
        "End" => ("\u{f72b}".into(), 119),
        " " => (" ".into(), 49),
        // A modifier alone, or a key with no text: nothing to give.
        other if other.chars().count() != 1 => return,
        other => (other.into(), 0),
    };
    use objc2_app_kit::NSEventModifierFlags as M;
    let mut flags = M::empty();
    for (name, flag) in [
        ("shift", M::Shift),
        ("control", M::Control),
        ("alt", M::Option),
        ("meta", M::Command),
    ] {
        if k[name].as_bool().unwrap_or(false) {
            flags |= flag;
        }
    }
    let down = k["down"].as_bool().unwrap_or(true);
    let text = NSString::from_str(&characters);
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        if down { NSEventType::KeyDown } else { NSEventType::KeyUp },
        NSPoint::new(0.0, 0.0),
        flags,
        now(),
        window.windowNumber(),
        None,
        &text,
        &text,
        k["repeat"].as_bool().unwrap_or(false),
        code,
    );
    if let Some(event) = event {
        window.sendEvent(&event);
    }
}
