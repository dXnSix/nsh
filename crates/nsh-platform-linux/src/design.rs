//! Design mode on Linux (dev hosts): the platform's part of `nsh/design`
//! (`nsh_core::runtime::design`, which keeps the connection to the tool
//! and its protocol, `docs/dev/design-mode.md`).
//!
//! Under Wayland a design tool runs its own compositor and starts the app
//! against it with `NSH_DESIGN_WAYLAND=1`, `GDK_BACKEND=wayland` and
//! `WAYLAND_DISPLAY` set. Each window of the app's own (`Window.open`) gets a
//! monotonic number `n` (`1, 2, …`), sets its toplevel title to
//! `"nsh-design:<n>"` (kept across attribute updates), stays undecorated and
//! shadowless so its toplevel geometry matches its content size, and is
//! reported with handle `("wayland", n)`. The app's windows are neither moved
//! nor hidden (the tool's compositor owns their display), and pointer, wheel
//! and keyboard input arrive over Wayland directly rather than through the
//! design socket. The main window stays hidden.
//!
//! Without `NSH_DESIGN_WAYLAND=1` (or without a Wayland display), Linux design
//! mode logs once and reports no windows.
//!
//! Pictures (`design.settled`, `design.picture`): [`crate::picture`]. When
//! the tool's connection closes, the app stops with the dev session that runs
//! it (its process group).

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

use gtk4::{self as gtk, glib, prelude::*};
use nsh_core::{
    NodeId,
    runtime::design::{self as core, DesignWindow},
};
use serde_json::Value;

use crate::{surface::Surface, view::NshView};

/// The environment variable enabling Wayland design mode inside a tool's
/// compositor (`"1"`).
const WAYLAND: &str = "NSH_DESIGN_WAYLAND";

/// The prefix on each design window's Wayland toplevel title
/// (`"nsh-design:<n>"`).
const TITLE_PREFIX: &str = "nsh-design:";

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

thread_local! {
    /// Whether the host is in design mode (the main thread's).
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    /// Next monotonic window number for Wayland design mode (`1, 2, …`).
    static NEXT_TAG: Cell<u64> = const { Cell::new(1) };
    /// The app's own windows by the id they opened with: their root node and
    /// Wayland window number.
    static NAMED: RefCell<HashMap<String, (NodeId, u64)>> = RefCell::new(HashMap::new());
    /// Missing `NSH_DESIGN_WAYLAND=1` or non-Wayland display: logged once.
    static NOT_WAYLAND: Cell<bool> = const { Cell::new(false) };
}

/// Whether the app runs in a design tool ([`start`] connected).
pub(crate) fn active() -> bool {
    ACTIVE.get()
}

/// The toplevel title identifying design window `tag`.
fn title_for(tag: u64) -> String {
    format!("{TITLE_PREFIX}{tag}")
}

/// Whether Wayland design mode is enabled by the environment
/// (`NSH_DESIGN_WAYLAND=1` and a non-empty `WAYLAND_DISPLAY`).
fn wayland_env(flag: Option<&str>, display: Option<&str>) -> bool {
    flag == Some("1") && display.is_some_and(|d| !d.is_empty())
}

fn wayland_enabled() -> bool {
    let flag = std::env::var(WAYLAND).ok();
    let display = std::env::var("WAYLAND_DISPLAY").ok();
    wayland_env(flag.as_deref(), display.as_deref())
}

fn warn_not_wayland_once(reason: &str) {
    if !NOT_WAYLAND.replace(true) {
        log::warn!(target: "nsh::design", "{reason}");
    }
}

/// Window `root` opened as `id`, by which the tool names it: assigns its
/// Wayland tag, sets its toplevel title to `"nsh-design:<n>"`, and strips
/// client-side decorations and shadow margins so toplevel pixels match the
/// window's content.
pub(crate) fn name_window(id: &str, root: NodeId, window: &gtk::Window) {
    let tag = NEXT_TAG.with(|n| {
        let current = n.get();
        n.set(current.saturating_add(1));
        current
    });
    NAMED.with_borrow_mut(|named| named.insert(id.to_owned(), (root, tag)));
    window.set_title(Some(&title_for(tag)));
    window.set_decorated(false);
    window.add_css_class("nsh-no-shadow");
}

/// Starts design mode when a tool started the app (`NSH_DESIGN_SOCKET`):
/// the connection, pictures, and the windows' watch. Called once on the main
/// thread before the app runs.
pub(crate) fn start() {
    let Some(endpoint) = core::endpoint() else {
        return;
    };
    let hooks = core::DesignHooks {
        input: Box::new(|message| {
            glib::MainContext::default().invoke_with_priority(glib::Priority::DEFAULT, move || {
                input(&message);
            });
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
    crate::picture::install();
    glib::timeout_add_local(Duration::from_millis(50), || {
        tick();
        glib::ControlFlow::Continue
    });
}

/// The app's surfaces, by the windows it has.
fn surfaces() -> Vec<Rc<Surface>> {
    Surface::all()
}

/// The window the tool calls `id`: the surface showing it, its root view, and
/// its `gtk::Window`.
pub(crate) fn window(id: &str) -> Option<(Rc<Surface>, NshView, gtk::Window)> {
    let (root, _) = NAMED.with_borrow(|named| named.get(id).copied())?;
    surfaces().into_iter().find_map(|s| {
        let window = crate::window::window_of(&s, root)?;
        let view = crate::window::root_view(&s, root)?;
        Some((s, view, window))
    })
}

/// Keeps the main window hidden and tells the tool what visible Wayland
/// design windows exist.
fn tick() {
    let surfaces = surfaces();
    for surface in &surfaces {
        if let Some(main) = crate::window::window_of(surface, nsh_core::ROOT_ID)
            && main.is_visible()
        {
            main.set_visible(false);
        }
    }
    if !wayland_enabled() {
        warn_not_wayland_once(
            "design mode on Linux requires NSH_DESIGN_WAYLAND=1 and WAYLAND_DISPLAY; reporting no windows",
        );
        prune_named(&surfaces);
        return;
    }
    let mut shown = Vec::new();
    let named: Vec<(String, NodeId, u64)> =
        NAMED.with_borrow(|named| named.iter().map(|(k, &(r, t))| (k.clone(), r, t)).collect());
    for surface in &surfaces {
        for (id, root, tag) in &named {
            let Some(window) = crate::window::window_of(surface, *root) else {
                continue;
            };
            if !window.is_visible() {
                continue;
            }
            let Some(gdk_surface) = window.surface() else {
                continue;
            };
            if gdk_surface.display().type_().name() != "GdkWaylandDisplay" {
                warn_not_wayland_once(
                    "design mode on Linux requires a Wayland display (GdkWaylandDisplay); reporting no windows",
                );
                continue;
            }
            let (width, height) = if window.width() > 0 && window.height() > 0 {
                (window.width() as f32, window.height() as f32)
            } else if let Some(view) = crate::window::root_view(surface, *root) {
                let size = view.state().frame.size;
                (size.width.0, size.height.0)
            } else {
                continue;
            };
            if width <= 0.0 || height <= 0.0 {
                continue;
            }
            shown.push(DesignWindow {
                id: id.clone(),
                handle: ("wayland", *tag),
                size: nsh_core::geometry::size(
                    nsh_core::geometry::Points(width),
                    nsh_core::geometry::Points(height),
                ),
            });
        }
    }
    prune_named(&surfaces);
    core::report(&shown);
}

/// Forgets windows that have closed.
fn prune_named(surfaces: &[Rc<Surface>]) {
    NAMED.with_borrow_mut(|named| {
        named.retain(|_, (root, _)| {
            surfaces
                .iter()
                .any(|s| crate::window::window_of(s, *root).is_some())
        });
    });
}

fn input(message: &Value) {
    let Some((kind, body)) = core::INPUT
        .into_iter()
        .find_map(|kind| message.get(kind).map(|body| (kind, body)))
    else {
        return;
    };
    // Under Wayland design mode, pointer, wheel and keyboard input arrive
    // through the tool's Wayland seat; viewport sharpening is a no-op here.
    if kind != "resize" {
        return;
    }
    let Some((_, _, window)) = window(body["id"].as_str().unwrap_or("")) else {
        return;
    };
    let width = body["width"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or_else(|| f64::from(window.width().max(1)));
    let height = body["height"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or_else(|| f64::from(window.height().max(1)));
    window.set_default_size(
        width.round().max(1.0) as i32,
        height.round().max(1.0) as i32,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_env_requires_flag_and_display() {
        assert!(wayland_env(Some("1"), Some("wayland-0")));
        assert!(!wayland_env(None, Some("wayland-0")));
        assert!(!wayland_env(Some("0"), Some("wayland-0")));
        assert!(!wayland_env(Some("true"), Some("wayland-0")));
        assert!(!wayland_env(Some("1"), None));
        assert!(!wayland_env(Some("1"), Some("")));
    }

    #[test]
    fn title_format_matches_protocol() {
        assert_eq!(title_for(1), "nsh-design:1");
        assert_eq!(title_for(42), "nsh-design:42");
    }
}
