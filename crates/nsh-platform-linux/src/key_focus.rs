//! The keyboard focus on views: Tab moves it between the stops
//! ([`nsh_core::ViewProps::focus_stop`]), fields and the native components
//! that take it, in tree order ([`tab`], through
//! [`nsh_core::input::tab_stop`]), as the other desktops do: GTK's own focus
//! chain would go by the widgets' places on screen. The arrow keys move it
//! to the nearest stop in their direction, as on GPUI and the other
//! desktops ([`arrow`], through `nsh_core::focus`). `focus()` moves it to a
//! stop. A focused stop reports `FOCUSED`, `onFocus` and `onBlur`, Return
//! and Space press it, and Shift+F10 or the Menu key ask it for its context
//! menu.
//!
//! One key controller per window, in GTK's capture phase, decides a press
//! in this order: the app's shortcuts, the focused view's `KeyDown`
//! (reported to the engine, never waited for) with its `capture_keys` rules
//! ([`crate::keyboard`]), the application menu's shortcuts
//! ([`crate::chrome::menu_shortcut`]), then the defaults of this module. A
//! view that hears raw keys runs the app's bindings itself, once
//! ([`crate::keymap::raw`]), and its shortcut's action skips that press;
//! other views leave the shortcuts to the app's `ShortcutController`, as
//! before. A captured key is reported (`KeyEvent::captured`) and runs none
//! of the menu's shortcuts or defaults: no traversal, activation or context
//! menu. A key that only listens runs them as before.
//!
//! A native popup (a context menu, the menu bar's menus, their nested
//! submenus) grabs the keyboard: its releases never reach the window's
//! controller. [`observe_popovers`] watches them where they are delivered,
//! so a release reaches the view focused now, as a browser reports a key
//! up on the focused element whichever view heard its press.
//!
//! Views stay unfocusable until Tab or `focus()` picks them: GTK also
//! hands the focus around by itself (to the first focusable widget as a
//! window maps, into a parent's children when a focused widget goes, and
//! through a native component's widgets), and a click or a new window must
//! not land on a button.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gtk4::{self as gtk, gdk, glib, graphene, prelude::*};
use nsh_core::{
    AccessibilityRole, Bounds, EventKind, EventPayload, NodeId, ViewKind,
    focus::{self as core_focus, Candidate, Layer, Order},
    geometry::{Points, point, size},
    input as core_input,
    keymap::Key,
    tailwind::state,
};

use crate::{surface::Surface, view::NshView};

thread_local! {
    /// Whether a Tab key is moving the focus right now.
    static TABBING: Cell<bool> = const { Cell::new(false) };
}

/// Lets a view take part in the Tab GTK is moving the focus for
/// (`GtkWidget::focus`): only stops do, only then.
pub(crate) fn offer(view: &NshView) {
    if TABBING.get() {
        view.set_focusable(is_stop(view));
    }
}

/// After [`offer`]: a stop Tab passed over goes back to unfocusable.
pub(crate) fn offered(view: &NshView) {
    if TABBING.get() && !view.is_focus() {
        view.set_focusable(false);
    }
}

/// Whether Tab stops at `view`: it takes presses or is `focusable`, and
/// the pointer could reach it.
pub(crate) fn is_stop(view: &NshView) -> bool {
    {
        let state = view.state();
        if state.kind == ViewKind::Input
            || !state
                .props
                .focus_stop(state.events.contains(EventKind::Press))
        {
            return false;
        }
    }
    view.can_target() && view.is_visible()
}

/// `focus()` on a view that is not an input: the focus moves to it as Tab
/// moves it there, if it is a stop.
pub(crate) fn focus(view: &NshView) {
    if !is_stop(view) || view.is_focus() {
        return;
    }
    view.set_focusable(true);
    if !view.grab_focus() {
        view.set_focusable(false);
    }
}

/// `blur()` on a view that is not an input, when it has the focus.
pub(crate) fn blur(view: &NshView) -> bool {
    if !view.is_focus() {
        return false;
    }
    if let Some(root) = view.root() {
        root.set_focus(None::<&gtk::Widget>);
    }
    true
}

/// Follows the focus of the surface's window, and takes the keys of a
/// focused stop: the app's shortcuts first, then the view's raw keys and
/// capture rules, then the application menu's shortcuts and the defaults
/// of this module (see [`press`]).
pub(crate) fn install(surface: &Rc<Surface>) {
    let weak = Rc::downgrade(surface);
    let installed = RefCell::new(glib::WeakRef::<gtk::Window>::new());
    surface.root.connect_realize(move |root| {
        let Some(window) = root.root().and_downcast::<gtk::Window>() else {
            return;
        };
        if installed.borrow().upgrade().as_ref() == Some(&window) {
            return;
        }
        installed.borrow().set(Some(&window));
        if let Some(surface) = weak.upgrade() {
            watch(&surface, &window);
        }
    });
}

/// Follows the focus in `window` (the main one or one of the app's own,
/// each with focus chains of its own) and takes a focused stop's keys.
pub(crate) fn watch(surface: &Rc<Surface>, window: &gtk::Window) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let pressed = Rc::downgrade(surface);
    keys.connect_key_pressed(
        move |keys, keyval, keycode, modifiers| match pressed.upgrade() {
            Some(surface)
                if press(
                    &surface,
                    keys.widget().as_ref(),
                    keys.current_event().as_ref(),
                    keyval,
                    keycode,
                    modifiers,
                    crate::keyboard::layout(keys.current_event().as_ref()),
                ) =>
            {
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        },
    );
    let released = Rc::downgrade(surface);
    keys.connect_key_released(move |keys, keyval, keycode, modifiers| {
        if let Some(surface) = released.upgrade() {
            let window = keys
                .widget()
                .and_then(|widget| widget.root())
                .and_then(|root| root.downcast::<gtk::Window>().ok());
            let modifiers =
                crate::keyboard::modifier_state(keys.current_event().as_ref(), modifiers);
            let focused = window
                .as_ref()
                .and_then(|window| focused_view(&surface, window));
            crate::keyboard::released(
                &surface,
                window.as_ref(),
                focused.as_ref(),
                keyval,
                keycode,
                modifiers,
                crate::keyboard::layout(keys.current_event().as_ref()),
            );
        }
    });
    window.add_controller(keys);
    // A window losing the OS focus loses the releases of the keys held in
    // it (a focus move inside it does not).
    let inactive = Rc::downgrade(surface);
    window.connect_is_active_notify(move |window| {
        if window.is_active() {
            return;
        }
        // GTK's native popovers can briefly deactivate and reactivate the
        // host while taking their grab. Clear only a settled loss of focus,
        // after the pending focus events, not that in-window round trip.
        let inactive = inactive.clone();
        let window = window.downgrade();
        glib::idle_add_local_once(move || {
            if let (Some(surface), Some(window)) = (inactive.upgrade(), window.upgrade())
                && !window.is_active()
            {
                crate::keyboard::window_deactivated(&surface, &window);
            }
        });
    });
    // Tab after the app's bindings and the focused widget's (a native
    // component's own): the window's `move-focus` binding.
    let tabbed = Rc::downgrade(surface);
    window.connect_move_focus(move |window, direction| {
        let back = match direction {
            gtk::DirectionType::TabForward => false,
            gtk::DirectionType::TabBackward => true,
            _ => return,
        };
        if let Some(surface) = tabbed.upgrade()
            && tab(&surface, window, back)
        {
            window.stop_signal_emission_by_name("move-focus");
        }
    });
    // The stop the focus left reports its blur in the window's first
    // handlers, the one it reached its focus after all of them (`after`):
    // an input's own handler, connected later, reports between them, so
    // `onBlur` always comes before `onFocus`.
    let current = Rc::new(RefCell::new(None::<NshView>));
    let weak = Rc::downgrade(surface);
    let leaving = current.clone();
    window.connect_focus_widget_notify(move |window| {
        if let Some(surface) = weak.upgrade() {
            // The screen reader follows the keyboard focus again.
            crate::access::clear_reader_focus(&surface);
            left(&surface, window, &leaving);
        }
    });
    let weak = Rc::downgrade(surface);
    let watched = window.downgrade();
    window.connect_local("notify::focus-widget", true, move |_| {
        if let (Some(surface), Some(window)) = (weak.upgrade(), watched.upgrade()) {
            reached(&surface, &window, &current);
        }
        None
    });
}

/// A stop of Tab: the widget whose subtree has the focus while the stop
/// does (an input's view, a native component's widget), and the widget
/// that takes it.
struct TabStop {
    owner: gtk::Widget,
    target: gtk::Widget,
}

/// Tab (`back`: Shift+Tab) in `window`: the focus moves to the next stop
/// of the window's root view, in tree order (a scroll view's content as it
/// is on screen, [`NshView::reading_children`]), around from the last;
/// the newest modal keeps it (the views under it cannot take the focus:
/// `can-focus`). False when there is no stop: GTK moves the focus then.
fn tab(surface: &Surface, window: &gtk::Window, back: bool) -> bool {
    let Some(root) = window
        .surface()
        .and_then(|s| crate::window::root_at(surface, &s))
    else {
        return false;
    };
    let stops = tab_stops(&root);
    // The innermost stop the focus is in.
    let current = GtkWindowExt::focus(window).and_then(|at| {
        stops
            .iter()
            .rposition(|stop| at == stop.owner || at.is_ancestor(&stop.owner))
    });
    let Some(next) = core_input::tab_stop(current, stops.len(), back, true) else {
        return false;
    };
    if Some(next) != current {
        let target = &stops[next].target;
        match target.downcast_ref::<NshView>() {
            Some(view) => focus(view),
            None => {
                target.grab_focus();
            }
        }
    }
    true
}

/// The stops of Tab under `root`, in tree order.
fn tab_stops(root: &NshView) -> Vec<TabStop> {
    let mut stops = Vec::new();
    for child in children(root) {
        collect_tab_stops(&child, &mut stops);
    }
    stops
}

/// `view`'s child widgets in the order Tab goes through them.
fn children(view: &NshView) -> Vec<gtk::Widget> {
    view.reading_children().unwrap_or_else(|| {
        let mut out = Vec::new();
        let mut child = view.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            out.push(widget);
        }
        out
    })
}

/// Whether `widget` and its subtree may take the focus: shown, sensitive
/// and not kept out (`can-focus`: the views under the newest modal).
fn reachable(widget: &gtk::Widget) -> bool {
    widget.is_visible() && widget.is_child_visible() && widget.is_sensitive() && widget.can_focus()
}

fn collect_tab_stops(widget: &gtk::Widget, out: &mut Vec<TabStop>) {
    if !reachable(widget) {
        return;
    }
    let Some(view) = widget.downcast_ref::<NshView>() else {
        // A native component's widget: one stop, its first focusable one.
        if let Some(target) = first_focusable(widget, 0) {
            out.push(TabStop {
                owner: widget.clone(),
                target,
            });
        }
        return;
    };
    if !view.can_target() {
        return;
    }
    if view.kind() == ViewKind::Input {
        if let Some(editor) = crate::text_input::key_widget(view)
            && reachable(&editor)
            && editor.is_focusable()
        {
            out.push(TabStop {
                owner: widget.clone(),
                target: editor,
            });
        }
        return;
    }
    if is_stop(view) {
        out.push(TabStop {
            owner: widget.clone(),
            target: widget.clone(),
        });
    }
    for child in children(view) {
        collect_tab_stops(&child, out);
    }
}

/// The first focusable widget of a native component's subtree, depth
/// first from `widget`.
fn first_focusable(widget: &gtk::Widget, depth: usize) -> Option<gtk::Widget> {
    if depth > 16 || !reachable(widget) {
        return None;
    }
    if widget.is_focusable() {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = first_focusable(&c, depth + 1) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

/// The focused stop of `surface`, in whichever of its windows.
fn focused_stop(surface: &Surface, focus: Option<gtk::Widget>) -> Option<NshView> {
    focus
        .and_downcast::<NshView>()
        .filter(|view| crate::window::holds(surface, view))
        .filter(is_stop)
}

/// The view a release is reported to: the stop focused now in `window`
/// (the main window, or one of the app's own). Nothing when the window has
/// no focused view any more.
pub(crate) fn focused_view(surface: &Surface, window: &gtk::Window) -> Option<NshView> {
    focused_stop(surface, GtkWindowExt::focus(window))
}

/// Watches the keys a native popup receives while it owns the keyboard
/// grab: a context menu, the menu bar's menus and their nested submenus
/// (`GtkPopoverMenu`s), all under `widget`.
///
/// A popover's grab delivers keys to its own native surface, and GTK stops
/// their propagation at the grab (`gtk_propagate_event_internal`), so the
/// window's key controller never sees them: a key held when a menu opens
/// would never release. Here a press only tracks a modifier (no view hears
/// the popup's keys); a release ends the hold on the view the key went down
/// on ([`crate::keyboard::released`]), in the window the popover belongs
/// to. Nothing is reported for a key that went down in the popup, and
/// opening a popup emits nothing by itself.
pub(crate) fn observe_popovers(surface: &Rc<Surface>, widget: &gtk::Widget) {
    if let Some(popover) = widget.downcast_ref::<gtk::Popover>()
        && !popover_watched(surface, popover)
    {
        // The default `SameNative` propagation limit is what this needs:
        // the popover's own native surface delivers the keys, and its
        // widget tree is the one they propagate through.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let pressed = Rc::downgrade(surface);
        keys.connect_key_pressed(move |_, keyval, keycode, _| {
            if let Some(surface) = pressed.upgrade() {
                crate::keyboard::track_modifiers(&surface, keyval, keycode, true);
            }
            glib::Propagation::Proceed
        });
        let released = Rc::downgrade(surface);
        keys.connect_key_released(move |keys, keyval, keycode, modifiers| {
            let window = host(keys.widget().as_ref());
            if let (Some(surface), Some(window)) = (released.upgrade(), window) {
                let modifiers =
                    crate::keyboard::modifier_state(keys.current_event().as_ref(), modifiers);
                let focused = focused_view(&surface, &window);
                crate::keyboard::released(
                    &surface,
                    Some(&window),
                    focused.as_ref(),
                    keyval,
                    keycode,
                    modifiers,
                    crate::keyboard::layout(keys.current_event().as_ref()),
                );
            }
        });
        popover.add_controller(keys);
    }
    // Nested menus are popovers of their own (children of their items),
    // each with its own native surface and grab.
    let mut child = widget.first_child();
    while let Some(current) = child {
        observe_popovers(surface, &current);
        child = current.next_sibling();
    }
}

/// Whether `popover` already has a release watcher (marking it when not):
/// walking a menu again, or a popover another menu adopted, adds none.
fn popover_watched(surface: &Surface, popover: &gtk::Popover) -> bool {
    let mut popovers = surface.chrome.popovers.borrow_mut();
    popovers.retain(|watched| watched.upgrade().is_some());
    if popovers
        .iter()
        .any(|watched| watched.upgrade().as_ref() == Some(popover))
    {
        return true;
    }
    popovers.push(popover.downgrade());
    false
}

/// The window a popover belongs to: a popover's own root is the popover,
/// so its host's parents lead to the window it pops over.
fn host(widget: Option<&gtk::Widget>) -> Option<gtk::Window> {
    let mut widget = widget?.parent();
    while let Some(current) = widget {
        if let Some(window) = current.downcast_ref::<gtk::Window>() {
            return Some(window.clone());
        }
        widget = current.parent();
    }
    None
}

/// The window's focus moved, first round: the stop it left says so.
fn left(surface: &Surface, window: &gtk::Window, current: &RefCell<Option<NshView>>) {
    let next = focused_stop(surface, GtkWindowExt::focus(window));
    if *current.borrow() == next {
        return;
    }
    if let Some(view) = current.take() {
        blurred(surface, &view);
    }
}

/// The window's focus moved, last round (after every other handler, an
/// input's own blur among them): the stop it reached says so.
fn reached(surface: &Surface, window: &gtk::Window, current: &RefCell<Option<NshView>>) {
    let next = focused_stop(surface, GtkWindowExt::focus(window));
    if *current.borrow() == next {
        return;
    }
    if let Some(view) = current.replace(next.clone()) {
        blurred(surface, &view);
    }
    if let Some(view) = next {
        crate::input::set_state(surface, &view, state::FOCUSED, true);
        surface.event(view.id(), EventKind::Focus, EventPayload::None);
    }
}

/// Stop `view` lost the focus.
fn blurred(surface: &Surface, view: &NshView) {
    view.set_focusable(false);
    crate::input::set_state(surface, view, state::FOCUSED, false);
    surface.event(view.id(), EventKind::Blur, EventPayload::None);
}

/// A key on its way to the focus: the app's shortcuts first (the app's
/// `ShortcutController` runs right after this, when the key is the app's),
/// then the focused view's `KeyDown` and capture rules, then the
/// application menu's shortcuts ([`crate::chrome::menu_shortcut`]), then
/// the defaults (Tab's traversal, Return and Space pressing, a context
/// menu). True when the key is consumed.
///
/// A focused view whose ancestry hears none of the raw keys and captures
/// none builds no raw [`KeyEvent`]; the app's bindings, the menu and
/// [`defaults`] decide. A native input or native content focus owns its
/// keys the same way, capture rules around it aside. Modifier transitions
/// stay tracked.
pub(crate) fn press(
    surface: &Rc<Surface>,
    widget: Option<&gtk::Widget>,
    event: Option<&gdk::Event>,
    keyval: gdk::Key,
    keycode: u32,
    modifiers: gdk::ModifierType,
    group: i32,
) -> bool {
    // GDK's typed state drops private modifiers (NumLock's Mod2): take the
    // raw state so a keypad key keeps typing its digit.
    let modifiers = crate::keyboard::modifier_state(event, modifiers);
    // Every key the window gets passes here first (the capture phase),
    // before it acts: the focus it moves shows (`focus-visible:`). A
    // modifier alone does not count.
    let modifier = event.is_some_and(|event| {
        event
            .downcast_ref::<gdk::KeyEvent>()
            .is_some_and(gdk::KeyEvent::is_modifier)
    });
    if !modifier {
        surface.control(|c| c.set_input_modality(nsh_core::platform::InputModality::Keyboard));
    }
    // A binding a previous press's raw dispatch reported must not leave its
    // shortcut's action silent (`keymap::raw`).
    crate::keymap::forget(surface);
    // A modifier's press is tracked even when nothing raw has the focus (a
    // native field, no focus): the next key event carries it.
    crate::keyboard::track_modifiers(surface, keyval, keycode, true);
    // The window the key is going to, and its focus.
    let root = widget.and_then(|widget| widget.root());
    let window = root
        .as_ref()
        .and_then(|root| root.clone().downcast::<gtk::Window>().ok());
    let focus = root.and_then(|root| root.focus());
    // The application menu's shortcut, where a view leaves the key to it.
    let menu_shortcut = || match (event, widget) {
        (Some(event), Some(widget)) => crate::chrome::menu_shortcut(surface, widget, event),
        _ => false,
    };
    let Some(view) = focused_stop(surface, focus) else {
        // A native field (or no focus) has no view to notify: the app's
        // bindings first, then the menu's shortcuts, then Tab's traversal.
        if event.is_some_and(|event| crate::keymap::claims(surface, event)) {
            return false;
        }
        if menu_shortcut() {
            return true;
        }
        if is_tab(keyval) {
            arm_tab();
        }
        return false;
    };
    if !crate::keyboard::listens(surface, &view) || crate::keyboard::native_owns(&view) {
        // A view with no raw keys (or a native input/content that owns its
        // keys) keeps the original path: the app's bindings (GTK's
        // controller) first, then the menu's shortcut, then the defaults.
        // Return and Space on a control fire only the focus' own (a menu
        // button's opening its menu, `keymap::fires`).
        if event.is_some_and(|event| crate::keymap::claims(surface, event)) {
            return false;
        }
        if menu_shortcut() {
            return true;
        }
        return defaults(surface, &view, keyval, modifiers);
    }
    // A raw view runs the app's bindings itself, once, before it is
    // notified: a pass-through binding reports and the key goes on, a
    // consuming one reports and takes it, and the shortcut controller does
    // not report either again.
    let repeat = window
        .as_ref()
        .is_some_and(|window| crate::keyboard::held(surface, window, keycode));
    if let Some(event) = event {
        match crate::keymap::raw(surface, event, repeat) {
            crate::keymap::RawFire::Consumed => return true,
            crate::keymap::RawFire::None | crate::keymap::RawFire::Passed => {}
        }
    }
    // A key the layout cannot name (a dead key) is still a `KeyDown`
    // (`Dead`, or `Unidentified`), so a capture that takes every key gets
    // it. The capture decides over the canonical shortcut keystroke.
    let mut key_event = crate::keyboard::key_event(
        surface,
        keyval,
        keycode,
        modifiers,
        crate::keyboard::KeyPhase::Down { repeat },
    );
    let captured = crate::keyboard::captures(surface, &view, &key_event, keycode, modifiers, group);
    key_event.default_prevented = captured;
    crate::keyboard::notify(surface, &view, EventKind::KeyDown, key_event);
    if captured {
        // The view took the key: no menu shortcut, traversal, activation or
        // context menu.
        return true;
    }
    // The application menu's shortcuts come before the defaults: a menu's
    // key is the app's, not a traversal or a context menu.
    if menu_shortcut() {
        return true;
    }
    // Tab's traversal is a default too: it is armed here, then GTK moves the
    // focus right after this, in the same dispatch.
    defaults(surface, &view, keyval, modifiers)
}

/// Whether `keyval` is a Tab (the app's traversal).
fn is_tab(keyval: gdk::Key) -> bool {
    matches!(
        keyval,
        gdk::Key::Tab | gdk::Key::ISO_Left_Tab | gdk::Key::KP_Tab
    )
}

fn arm_tab() {
    TABBING.set(true);
    glib::idle_add_local_once(|| TABBING.set(false));
}

/// The defaults of a focused stop: Tab's traversal, a context menu on
/// Shift+F10 or the Menu key, and Return and Space pressing a view that
/// takes presses. True when one consumed the key.
fn defaults(
    surface: &Rc<Surface>,
    view: &NshView,
    keyval: gdk::Key,
    modifiers: gdk::ModifierType,
) -> bool {
    if is_tab(keyval) {
        // Only a Tab left to GTK may offer the stops for traversal.
        arm_tab();
        return false;
    }
    let held = crate::input::modifiers(modifiers);
    let function = (keyval == gdk::Key::F10).then_some(Key::F(10));
    if core_input::opens_context_menu(keyval == gdk::Key::Menu, function, held) {
        return show_context_menu(surface, view);
    }
    let pressing = match keyval {
        gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => Key::Enter,
        gdk::Key::space => Key::Char(' '),
        _ => return false,
    };
    presses(surface, view, pressing, held)
}

/// Whether `focus` (a window's focus widget) is a stop that Return and
/// Space press: one of the surface's views that takes presses. The app's
/// bindings of those keys then give way to it, but for the focus' own
/// (`Keymap::find_at`).
pub(crate) fn is_control(surface: &Surface, focus: &gtk::Widget) -> bool {
    focused_stop(surface, Some(focus.clone()))
        .is_some_and(|view| view.state().events.contains(EventKind::Press))
}

/// Return and Space press the focused stop `view` when it takes presses
/// ([`core_input::presses_focused_control`]), once the app's bindings of
/// the focus passed on them. A stop that takes no presses (a `focusable`
/// chart) hears them through the shortcuts only. True when it was
/// pressed.
fn presses(surface: &Surface, view: &NshView, key: Key, held: nsh_core::keymap::Modifiers) -> bool {
    if !core_input::presses_focused_control(key, held)
        || !view.state().events.contains(EventKind::Press)
    {
        return false;
    }
    crate::input::activate(surface, view);
    true
}

/// A key pressed without a GDK event (`nsh test --target linux` without
/// XTest) that no binding took: Return or Space press the focused stop of
/// the key window, as [`press`] has them do. True when it was pressed.
#[cfg(feature = "dev")]
pub(crate) fn press_key(surface: &Surface, stroke: nsh_core::keymap::Keystroke) -> bool {
    let focus = crate::window::key_window(surface).and_then(|w| GtkWindowExt::focus(&w));
    focused_stop(surface, focus)
        .is_some_and(|view| presses(surface, &view, stroke.key, stroke.modifiers))
}

/// A context menu asked for on `view` without a pointer: the nearest view
/// from it outwards that listens to `onContextMenu` hears it, with no
/// pointer, at its bottom-start corner. True when one heard it.
fn show_context_menu(surface: &Surface, view: &NshView) -> bool {
    let rtl = surface.mounter.try_borrow().is_ok_and(|m| m.rtl);
    let mut at = Some(view.clone());
    while let Some(v) = at {
        let (id, listens, size) = {
            let s = v.state();
            (
                s.id,
                s.events.contains(EventKind::ContextMenu),
                s.frame.size,
            )
        };
        if listens {
            let position = core_input::context_menu_anchor(size, rtl);
            let (x, y) = v
                .compute_point(
                    &crate::window::root_of(surface, &v),
                    &graphene::Point::new(position.x.0, position.y.0),
                )
                .map_or((position.x.0, position.y.0), |p| (p.x(), p.y()));
            let event = nsh_core::ContextMenuEvent::new(
                position,
                point(Points(x), Points(y)),
                Default::default(),
                None,
            );
            surface.event(id, EventKind::ContextMenu, EventPayload::ContextMenu(event));
            return true;
        }
        at = v.parent_view();
    }
    false
}

// --- The arrows -------------------------------------------------------------------

/// An arrow key, the way it moves the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

impl Arrow {
    /// The arrow of `key` (the keypad's too) held with no modifier: with
    /// Shift it selects, with Ctrl, Alt or Super it is a shortcut (Alt+Left
    /// goes back).
    pub(crate) fn of(key: gdk::Key, modifiers: gdk::ModifierType) -> Option<Arrow> {
        use gdk::{Key, ModifierType as M};
        let held = M::SHIFT_MASK
            | M::CONTROL_MASK
            | M::ALT_MASK
            | M::SUPER_MASK
            | M::META_MASK
            | M::HYPER_MASK;
        if modifiers.intersects(held) {
            return None;
        }
        Some(match key {
            Key::Left | Key::KP_Left => Arrow::Left,
            Key::Right | Key::KP_Right => Arrow::Right,
            Key::Up | Key::KP_Up => Arrow::Up,
            Key::Down | Key::KP_Down => Arrow::Down,
            _ => return None,
        })
    }

    fn horizontal(self) -> bool {
        matches!(self, Arrow::Left | Arrow::Right)
    }
}

/// A stop of the arrows: the core's candidate, the widget whose subtree
/// has the focus while the stop does (an input's view) and its view.
struct ArrowStop {
    node: Candidate,
    owner: gtk::Widget,
    view: NshView,
}

impl AsRef<Candidate> for ArrowStop {
    fn as_ref(&self) -> &Candidate {
        &self.node
    }
}

/// An axis-aligned box in the window's points: left, top, right, bottom.
type Edges = (f32, f32, f32, f32);

fn intersect(a: Edges, b: Edges) -> Edges {
    (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
}

fn shows(e: Edges) -> bool {
    e.2 > e.0 && e.3 > e.1
}

fn bounds(e: Edges) -> Bounds<Points> {
    Bounds::new(
        point(Points(e.0), Points(e.1)),
        size(Points(e.2 - e.0), Points(e.3 - e.1)),
    )
}

/// The box `widget` covers in `root`'s points as it shows (scroll offsets
/// and transforms applied: GTK's `compute_bounds`, the box around a
/// transformed one).
fn edges(widget: &gtk::Widget, root: &NshView) -> Option<Edges> {
    let r = widget.compute_bounds(root)?;
    Some((r.x(), r.y(), r.x() + r.width(), r.y() + r.height()))
}

/// The box of `root`'s window, in its points.
fn window_edges(root: &NshView) -> Edges {
    (0.0, 0.0, root.width() as f32, root.height() as f32)
}

/// The views of a focus scope as the core's candidates, in the order Tab
/// goes through them.
struct Walk<'a> {
    root: &'a NshView,
    scope: NodeId,
    stops: Vec<ArrowStop>,
    order: usize,
}

impl Walk<'_> {
    fn widget(&mut self, widget: &gtk::Widget, clip: Edges, layer: NodeId, popup: bool) {
        // A native component keeps the keys once it has the focus: it is
        // no stop of the arrows. Another modal is a scope of its own.
        let Some(view) = widget.downcast_ref::<NshView>() else {
            return;
        };
        if !reachable(widget) || !view.can_target() || view.kind() == ViewKind::Modal {
            return;
        }
        let order = self.order;
        self.order += 1;
        let popup = popup || view.state().props.overlay_anchor.is_some();
        let own = edges(widget, self.root);
        let candidate = |editable: bool, frame: Edges| {
            let mut node = Candidate::default();
            node.id = view.id();
            node.frame = bounds(frame);
            node.visible = shows(intersect(frame, clip));
            node.stop = true;
            node.editable = editable;
            node.scope = self.scope;
            node.layer = Some(layer);
            node.popup = popup;
            node.order = Order::view(order);
            node
        };
        if view.kind() == ViewKind::Input {
            if let (Some(editor), Some(own)) = (crate::text_input::key_widget(view), own)
                && reachable(&editor)
                && editor.is_focusable()
            {
                let editable = crate::text_input::editable(view);
                self.stops.push(ArrowStop {
                    node: candidate(editable, own),
                    owner: widget.clone(),
                    view: view.clone(),
                });
            }
            return;
        }
        if let Some(own) = own
            && is_stop(view)
        {
            self.stops.push(ArrowStop {
                node: candidate(false, own),
                owner: widget.clone(),
                view: view.clone(),
            });
        }
        let clip = match own {
            Some(own) if view.state().clips() => intersect(own, clip),
            _ => clip,
        };
        for child in children(view) {
            self.widget(&child, clip, layer, popup);
        }
    }
}

/// The stops of `scope` (the newest modal of `root`'s window, or `root`)
/// in focus order (`focus::candidates`).
fn arrow_stops(root: &NshView, scope: &NshView) -> Vec<ArrowStop> {
    let mut walk = Walk {
        root,
        scope: scope.id(),
        stops: Vec::new(),
        order: 0,
    };
    let window = window_edges(root);
    let clip = edges(scope.upcast_ref(), root).map_or(window, |own| intersect(own, window));
    for child in children(scope) {
        let layer = child
            .downcast_ref::<NshView>()
            .map_or(scope.id(), NshView::id);
        walk.widget(&child, clip, layer, false);
    }
    core_focus::candidates(walk.stops, scope.id(), &layers(root, scope))
}

/// The children of `scope` as the core's layers, the page first.
fn layers(root: &NshView, scope: &NshView) -> Vec<Layer> {
    let window = window_edges(root);
    scope
        .child_views()
        .iter()
        .filter(|child| child.kind() != ViewKind::Modal)
        .enumerate()
        .map(|(order, child)| {
            let mut layer = Layer::default();
            layer.id = child.id();
            layer.order = order;
            if reachable(child.upcast_ref()) && child.can_target() {
                layer.frame = edges(child.upcast_ref(), root)
                    .map(|e| intersect(e, window))
                    .filter(|&e| shows(e))
                    .map(bounds);
                layer.takes_presses = core_input::has_press(child.state().events);
            }
            layer
        })
        .collect()
}

/// `ancestor` is `view` or one of its ancestors.
fn contains(ancestor: &NshView, view: &NshView) -> bool {
    view == ancestor || view.is_ancestor(ancestor)
}

/// The overlay anchored to another view that `view` is in (a menu's, a
/// select's or a popover's popup), if any.
fn anchored_overlay(view: &NshView) -> Option<NshView> {
    let mut at = Some(view.clone());
    while let Some(v) = at {
        if v.state().props.overlay_anchor.is_some() {
            return Some(v);
        }
        at = v.parent_view();
    }
    None
}

/// Anchored overlay `popup` hangs from a trigger: its anchor is one of
/// `stops`, holds one or is in one.
fn triggered(surface: &Surface, popup: &NshView, stops: &[ArrowStop]) -> bool {
    let anchor = popup
        .state()
        .props
        .overlay_anchor
        .as_ref()
        .and_then(|anchor| anchor.node);
    let Some(anchor) = anchor.and_then(|id| {
        let mounter = surface.mounter.try_borrow().ok()?;
        mounter.views.get(&id).cloned()
    }) else {
        return false;
    };
    stops
        .iter()
        .any(|stop| contains(&anchor, &stop.view) || contains(&stop.view, &anchor))
}

/// `view` is a menu's item.
fn in_menu(view: &NshView) -> bool {
    matches!(
        view.state().props.accessibility.role,
        AccessibilityRole::Menuitem
            | AccessibilityRole::MenuItemCheckbox
            | AccessibilityRole::MenuItemRadio
    )
}

/// Whether the focused widget `focus` keeps `arrow` to itself: a native
/// component's widget keeps them all, an input's editor Left and Right,
/// and a multiline one's text view Up and Down too.
fn keeps(focus: &gtk::Widget, arrow: Arrow) -> bool {
    if focus.is::<NshView>() {
        return false;
    }
    let field = focus
        .ancestor(NshView::static_type())
        .and_downcast::<NshView>()
        .filter(|view| view.kind() == ViewKind::Input)
        .and_then(|view| crate::text_input::key_widget(&view))
        .is_some_and(|editor| editor == *focus);
    !field || arrow.horizontal() || focus.is::<gtk::TextView>()
}

/// `arrow` in the window `root` is the root view of: the focus moves to
/// the stop that way among the stops of the focus scope (the newest modal,
/// else `root`), if any; the window then scrolls it into view
/// ([`crate::scroll`]). With nothing focused Left / Right take the first
/// stop, Up / Down the first / last in sight. False when it moved none, or
/// the focused widget keeps the arrow ([`keeps`]).
pub(crate) fn arrow(surface: &Surface, root: &NshView, arrow: Arrow) -> bool {
    let focused = root.root().and_then(|r| r.focus());
    if focused.as_ref().is_some_and(|at| keeps(at, arrow)) {
        return false;
    }
    let scope = crate::navigation::focus_scope(surface, root);
    let mut stops = arrow_stops(root, &scope);
    // The innermost stop the focus is in.
    let at = |stops: &[ArrowStop]| {
        focused.as_ref().and_then(|at| {
            stops
                .iter()
                .rposition(|stop| *at == stop.owner || at.is_ancestor(&stop.owner))
        })
    };
    // Left and Right on a menu's item would leave its menu open behind
    // the focus: they stay in it.
    if arrow.horizontal()
        && let Some(popup) = at(&stops)
            .map(|i| stops[i].view.clone())
            .filter(in_menu)
            .and_then(|view| anchored_overlay(&view))
            .filter(|popup| triggered(surface, popup, &stops))
    {
        stops.retain(|stop| contains(&popup, &stop.view));
    }
    let current = at(&stops);
    let next = match arrow {
        Arrow::Left | Arrow::Right => {
            core_focus::horizontal(&stops, current, arrow == Arrow::Right)
        }
        Arrow::Up | Arrow::Down => core_focus::vertical(&stops, current, arrow == Arrow::Down),
    };
    let Some(next) = next.filter(|&next| Some(next) != current) else {
        return false;
    };
    let view = &stops[next].view;
    if view.kind() == ViewKind::Input {
        crate::text_input::focus(view);
    } else {
        focus(view);
    }
    true
}

#[cfg(test)]
mod tests {
    use nsh_core::{
        EventMask, KeyCapture, MountItem, NodeId, ROOT_ID, ViewProps, keymap::Keystroke,
        platform::Os,
    };

    use super::*;
    use crate::gtk_test::{apply, frame, run, show, surface, view, wait_until};

    /// A stop (2, it takes presses), a plain view (3) and a stop (4).
    fn views(surface: &Rc<Surface>) {
        let mut items = Vec::new();
        for (id, y) in [(2, 0.0), (3, 40.0), (4, 80.0)] {
            items.push(MountItem::Create {
                id,
                kind: ViewKind::View,
            });
            items.push(MountItem::Insert {
                parent: ROOT_ID,
                child: id,
                before: None,
            });
            items.push(MountItem::Frame {
                id,
                frame: frame(0.0, y, 100.0, 30.0),
            });
            if id != 3 {
                let mut events = EventMask::default();
                events.set(EventKind::Press, true);
                items.push(MountItem::Events { id, events });
            }
        }
        apply(surface, &items);
    }

    fn tab(window: &gtk::Window) {
        TABBING.set(true);
        window.child_focus(gtk::DirectionType::TabForward);
        TABBING.set(false);
    }

    /// `props` of `id`, changed as `edit` says.
    fn props(surface: &Rc<Surface>, id: NodeId, edit: impl FnOnce(&mut ViewProps)) {
        let mut props = view(surface, id).state().props.clone();
        edit(&mut props);
        apply(surface, &[MountItem::ViewProps { id, props }]);
    }

    /// The display's keycode for `key`.
    fn keycode(key: gdk::Key) -> u32 {
        gdk::Display::default()
            .and_then(|display| display.map_keyval(key))
            .and_then(|keys| keys.first().map(|key| key.keycode()))
            .expect("a keycode")
    }

    /// Whether a press of `key` is captured by `view`'s ancestry, from the
    /// display's keycode for it.
    fn captured(surface: &Surface, view: &NshView, key: gdk::Key) -> bool {
        let keycode = keycode(key);
        let event = crate::keyboard::key_event(
            surface,
            key,
            keycode,
            gdk::ModifierType::empty(),
            crate::keyboard::KeyPhase::Down { repeat: false },
        );
        crate::keyboard::captures(
            surface,
            view,
            &event,
            keycode,
            gdk::ModifierType::empty(),
            0,
        )
    }

    #[test]
    fn a_capture_takes_the_key_before_the_defaults() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let (first, plain, last) = (view(&surface, 2), view(&surface, 3), view(&surface, 4));
            assert!(wait_until(|| first.is_mapped()));
            // The plain view becomes a stop that only listens to keys.
            props(&surface, 3, |props| props.focusable = Some(true));
            focus(&plain);
            assert!(plain.is_focus());
            // Listening does not consume: Tab is still GTK's traversal and
            // Space is still the platform's.
            assert!(!press(
                &surface,
                Some(window.upcast_ref()),
                None,
                gdk::Key::space,
                keycode(gdk::Key::space),
                gdk::ModifierType::empty(),
                0,
            ));
            assert!(!press(
                &surface,
                Some(window.upcast_ref()),
                None,
                gdk::Key::Tab,
                keycode(gdk::Key::Tab),
                gdk::ModifierType::empty(),
                0,
            ));
            tab(&window);
            assert!(last.is_focus(), "an uncaptured Tab still moves the focus");
            // A capture consumes the key here, before GTK's traversal.
            focus(&first);
            assert!(first.is_focus());
            props(&surface, 2, |props| props.key_capture = KeyCapture::All);
            assert!(press(
                &surface,
                Some(window.upcast_ref()),
                None,
                gdk::Key::Tab,
                keycode(gdk::Key::Tab),
                gdk::ModifierType::empty(),
                0,
            ));
            assert!(first.is_focus(), "a captured Tab is no traversal");
            assert!(
                !TABBING.get(),
                "a captured Tab must not arm another window's focus"
            );
            // The capture follows the mounted props.
            props(&surface, 2, |props| props.key_capture = KeyCapture::None);
            assert!(!press(
                &surface,
                Some(window.upcast_ref()),
                None,
                gdk::Key::Tab,
                keycode(gdk::Key::Tab),
                gdk::ModifierType::empty(),
                0,
            ));
            window.child_focus(gtk::DirectionType::TabForward);
            assert!(plain.is_focus());
            assert!(wait_until(|| !TABBING.get()));
            window.destroy();
        });
    }

    #[test]
    fn capture_walks_enabled_ancestors_only() {
        run(|| {
            let surface = surface();
            apply(
                &surface,
                &[
                    MountItem::Create {
                        id: 2,
                        kind: ViewKind::View,
                    },
                    MountItem::Insert {
                        parent: ROOT_ID,
                        child: 2,
                        before: None,
                    },
                    MountItem::Create {
                        id: 3,
                        kind: ViewKind::View,
                    },
                    MountItem::Insert {
                        parent: 2,
                        child: 3,
                        before: None,
                    },
                    MountItem::Frame {
                        id: 2,
                        frame: frame(0.0, 0.0, 100.0, 80.0),
                    },
                    MountItem::Frame {
                        id: 3,
                        frame: frame(0.0, 0.0, 100.0, 40.0),
                    },
                ],
            );
            let escape = Keystroke::parse("escape", Os::Linux).expect("a stroke");
            props(&surface, 2, |props| {
                props.key_capture = KeyCapture::keys([escape]);
            });
            let child = view(&surface, 3);
            let mut events = EventMask::default();
            events.set(EventKind::KeyDown, true);
            apply(&surface, &[MountItem::Events { id: 2, events }]);
            assert!(
                crate::keyboard::listens(&surface, &child),
                "an ancestor's raw listener reaches the child"
            );
            assert!(captured(&surface, &child, gdk::Key::Escape));
            assert!(!captured(&surface, &child, gdk::Key::Return));
            // A disabled view captures nothing.
            props(&surface, 2, |props| {
                props.accessibility.state.disabled = true;
            });
            assert!(!captured(&surface, &child, gdk::Key::Escape));
        });
    }

    /// Two portaled children share a native overlay host: one's logical
    /// parent listens and captures, the other's does not. Keyboard ancestry
    /// follows `key_parent`, not the native Insert parent.
    #[test]
    fn a_portals_logical_ancestor_routes_interest_and_capture() {
        run(|| {
            let surface = surface();
            let mut items = Vec::new();
            for id in [2, 5, 7] {
                items.push(MountItem::Create {
                    id,
                    kind: ViewKind::View,
                });
                items.push(MountItem::Insert {
                    parent: ROOT_ID,
                    child: id,
                    before: None,
                });
            }
            for id in [3, 4] {
                items.push(MountItem::Create {
                    id,
                    kind: ViewKind::View,
                });
                items.push(MountItem::Insert {
                    parent: 5,
                    child: id,
                    before: None,
                });
            }
            apply(&surface, &items);
            // The logical ancestor hears raw keys and captures them; the
            // unrelated logical parent (7) does neither.
            let mut events = EventMask::default();
            events.set(EventKind::KeyDown, true);
            apply(&surface, &[MountItem::Events { id: 2, events }]);
            props(&surface, 2, |props| props.key_capture = KeyCapture::All);
            props(&surface, 3, |props| {
                props.key_parent = Some(2);
                props.focusable = Some(true);
            });
            props(&surface, 4, |props| {
                props.key_parent = Some(7);
                props.focusable = Some(true);
            });
            let child = view(&surface, 3);
            let sibling = view(&surface, 4);
            assert!(
                crate::keyboard::listens(&surface, &child),
                "the portal follows its logical ancestor"
            );
            assert!(captured(&surface, &child, gdk::Key::Escape));
            assert!(
                !crate::keyboard::listens(&surface, &sibling),
                "an unrelated portal is not under the listener"
            );
            assert!(!captured(&surface, &sibling, gdk::Key::Escape));
        });
    }

    #[test]
    fn every_key_capture_takes_an_unidentified_key_and_none_lets_it_go() {
        run(|| {
            let surface = surface();
            views(&surface);
            // The view stays a stop (it takes presses) and hears the raw
            // keys, so a press takes the raw path.
            let mut events = EventMask::default();
            events.set(EventKind::Press, true);
            events.set(EventKind::KeyDown, true);
            apply(&surface, &[MountItem::Events { id: 2, events }]);
            let window = show(&surface);
            let first = view(&surface, 2);
            assert!(wait_until(|| first.is_mapped()));
            focus(&first);
            assert!(first.is_focus());
            // An unnamed key without a hardware code reports neither an
            // invented identity nor a code.
            let event = crate::keyboard::key_event(
                &surface,
                gdk::Key::VoidSymbol,
                0,
                gdk::ModifierType::empty(),
                crate::keyboard::KeyPhase::Down { repeat: false },
            );
            assert_eq!(event.key, "Unidentified");
            assert_eq!(event.code, "");
            assert!(!event.default_prevented);
            // `All` takes it before the defaults.
            props(&surface, 2, |props| props.key_capture = KeyCapture::All);
            assert!(press(
                &surface,
                Some(window.upcast_ref()),
                None,
                gdk::Key::VoidSymbol,
                0,
                gdk::ModifierType::empty(),
                0,
            ));
            // `None` leaves it to the platform.
            props(&surface, 2, |props| props.key_capture = KeyCapture::None);
            assert!(!press(
                &surface,
                Some(window.upcast_ref()),
                None,
                gdk::Key::VoidSymbol,
                0,
                gdk::ModifierType::empty(),
                0,
            ));
            window.destroy();
        });
    }

    #[test]
    fn a_tab_press_arms_the_traversal_without_focus() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let first = view(&surface, 2);
            assert!(wait_until(|| first.is_mapped()));
            assert!(GtkWindowExt::focus(&window).is_none());
            // The press itself arms GTK's traversal (the stops are
            // focusable only while it moves the focus): no test helper.
            assert!(!press(
                &surface,
                Some(window.upcast_ref()),
                None,
                gdk::Key::Tab,
                keycode(gdk::Key::Tab),
                gdk::ModifierType::empty(),
                0,
            ));
            window.child_focus(gtk::DirectionType::TabForward);
            assert!(first.is_focus(), "Tab from nothing enters the first stop");
            assert!(wait_until(|| !TABBING.get()));
            window.destroy();
        });
    }

    #[test]
    fn a_release_is_reported_to_the_view_focused_then() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let (first, last) = (view(&surface, 2), view(&surface, 4));
            assert!(wait_until(|| first.is_mapped()));
            focus(&first);
            assert!(first.is_focus());
            assert!(!crate::keyboard::held(&surface, &window, 9));
            crate::keyboard::notify(
                &surface,
                &first,
                EventKind::KeyDown,
                crate::keyboard::key_event(
                    &surface,
                    gdk::Key::Escape,
                    9,
                    gdk::ModifierType::empty(),
                    crate::keyboard::KeyPhase::Down { repeat: false },
                ),
            );
            assert!(crate::keyboard::held_key(&surface, 9));
            // A focus move keeps the key held, and the view focused at the
            // source's release is the one the key up goes to.
            focus(&last);
            assert!(last.is_focus());
            assert!(crate::keyboard::held_key(&surface, 9));
            assert_eq!(focused_view(&surface, &window).map(|v| v.id()), Some(4));
            // The release ends the hold; there is no original down target
            // to send it back to.
            crate::keyboard::released(
                &surface,
                Some(&window),
                focused_view(&surface, &window).as_ref(),
                gdk::Key::Escape,
                9,
                gdk::ModifierType::empty(),
                0,
            );
            assert!(!crate::keyboard::held_key(&surface, 9));
            window.destroy();
        });
    }

    #[test]
    fn the_fast_path_follows_the_mounted_events_and_capture() {
        run(|| {
            let surface = surface();
            views(&surface);
            let plain = view(&surface, 3);
            assert!(!crate::keyboard::listens(&surface, &plain));
            let mut events = EventMask::default();
            events.set(EventKind::KeyDown, true);
            apply(&surface, &[MountItem::Events { id: 3, events }]);
            assert!(
                crate::keyboard::listens(&surface, &plain),
                "a raw key listener"
            );
            events.set(EventKind::KeyDown, false);
            apply(&surface, &[MountItem::Events { id: 3, events }]);
            assert!(!crate::keyboard::listens(&surface, &plain));
            props(&surface, 3, |props| {
                props.key_capture = KeyCapture::keys([]);
            });
            assert!(
                !crate::keyboard::listens(&surface, &plain),
                "an empty capture takes no key"
            );
            props(&surface, 3, |props| props.key_capture = KeyCapture::All);
            assert!(
                crate::keyboard::listens(&surface, &plain),
                "a capture policy"
            );
        });
    }

    #[test]
    fn tab_stops_at_stops_and_a_window_opening_focuses_none() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let (first, plain, last) = (view(&surface, 2), view(&surface, 3), view(&surface, 4));
            assert!(wait_until(|| first.is_mapped()));
            assert!(
                GtkWindowExt::focus(&window).is_none(),
                "nothing focused on opening"
            );
            tab(&window);
            assert!(first.is_focus());
            tab(&window);
            assert!(last.is_focus(), "the plain view is no stop");
            assert!(!plain.is_focusable());
            assert!(!first.is_focusable(), "unfocusable again once left");
            window.destroy();
        });
    }

    #[test]
    fn focus_moves_to_stops_only_and_blur_lets_go() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let (first, plain) = (view(&surface, 2), view(&surface, 3));
            assert!(wait_until(|| first.is_mapped()));
            focus(&plain);
            assert!(!plain.is_focus());
            focus(&first);
            assert!(first.is_focus());
            assert!(blur(&first));
            assert!(!first.is_focus());
            window.destroy();
        });
    }

    #[cfg(feature = "dev")]
    mod menus {
        use nsh_core::{
            desktop::{DesktopCommand, MenuAction, MenuItem, MenuRole, Shortcut},
            keymap::When,
        };

        use super::*;

        /// Whether the tests' display is X11 (real keys need XTest).
        fn x11() -> bool {
            gdk::Display::default().is_some_and(|display| display.type_().name() == "GdkX11Display")
        }

        /// A menu item with Ctrl+N that does `role`.
        fn menu_item(role: MenuRole) -> MenuItem {
            MenuItem::Action(MenuAction {
                id: 1,
                label: "Test".into(),
                shortcut: Some(Shortcut::parse("mod+n", false).expect("a shortcut")),
                enabled: true,
                checked: None,
                role: Some(role),
                when: When::Auto,
                icon: None,
                symbol: None,
                destructive: false,
            })
        }

        /// The application menu of `surface`: one Ctrl+N item with `role`.
        fn set_menu(surface: &Rc<Surface>, role: MenuRole) {
            crate::chrome::command(
                surface,
                &DesktopCommand::SetMenu(Some(vec![menu_item(role)].into())),
            );
        }

        #[test]
        fn a_capture_takes_an_application_menu_key() {
            run(|| {
                if !x11() {
                    return;
                }
                let surface = surface();
                views(&surface);
                let window = show(&surface);
                let first = view(&surface, 2);
                assert!(wait_until(|| first.is_mapped()));
                focus(&first);
                assert!(first.is_focus());
                let mut events = EventMask::default();
                events.set(EventKind::Press, true);
                events.set(EventKind::KeyDown, true);
                apply(&surface, &[MountItem::Events { id: 2, events }]);
                props(&surface, 2, |props| props.key_capture = KeyCapture::All);
                set_menu(&surface, MenuRole::Close);
                let n = keycode(gdk::Key::n);
                crate::xtest::Xtest::with(|x| {
                    x.focus(&window);
                    assert!(wait_until(|| window.is_active()));
                    assert!(x.key(crate::xtest::CONTROL_L, true));
                    assert!(x.key(crate::xtest::char_keysym('n'), true));
                    assert!(
                        wait_until(|| crate::keyboard::held_key(&surface, n)),
                        "the capture hears the key the application menu also binds"
                    );
                    assert!(window.is_visible(), "the menu action did not run");
                    assert!(x.key(crate::xtest::char_keysym('n'), false));
                    assert!(x.key(crate::xtest::CONTROL_L, false));
                })
                .expect("real XTest is required");
                assert!(wait_until(|| !crate::keyboard::held_key(&surface, n)));
                window.destroy();
            });
        }

        #[test]
        fn the_application_menu_runs_without_a_focused_view() {
            run(|| {
                if !x11() {
                    return;
                }
                let surface = surface();
                views(&surface);
                let window = show(&surface);
                let first = view(&surface, 2);
                assert!(wait_until(|| first.is_mapped()));
                assert!(GtkWindowExt::focus(&window).is_none());
                set_menu(&surface, MenuRole::Close);
                crate::xtest::Xtest::with(|x| {
                    x.focus(&window);
                    assert!(wait_until(|| window.is_active()));
                    assert!(x.stroke(&[crate::xtest::CONTROL_L], crate::xtest::char_keysym('n')));
                })
                .expect("real XTest is required");
                assert!(
                    wait_until(|| !window.is_visible()),
                    "a menu shortcut still works where no view hears keys"
                );
            });
        }

        #[test]
        fn a_popover_grab_releases_the_key_it_held() {
            run(|| {
                if !x11() {
                    return;
                }
                let surface = surface();
                views(&surface);
                let window = show(&surface);
                let first = view(&surface, 2);
                assert!(wait_until(|| first.is_mapped()));
                let group = gtk::gio::SimpleActionGroup::new();
                group.add_action(&gtk::gio::SimpleAction::new("item", None));
                surface.root.insert_action_group("popup", Some(&group));
                let model = gtk::gio::Menu::new();
                model.append(Some("Item"), Some("popup.item"));
                let menu = gtk::PopoverMenu::from_model(Some(&model));
                menu.set_parent(&surface.root);
                observe_popovers(&surface, menu.upcast_ref());
                let k = keycode(gdk::Key::k);
                let mut events = nsh_core::EventMask::default();
                events.set(nsh_core::EventKind::KeyDown, true);
                events.set(nsh_core::EventKind::KeyUp, true);
                apply(&surface, &[MountItem::Events { id: 2, events }]);
                props(&surface, 2, |props| props.focusable = Some(true));
                crate::xtest::Xtest::with(|x| {
                    x.focus(&window);
                    assert!(wait_until(|| window.is_active()));
                    focus(&first);
                    assert!(wait_until(|| first.is_focus()));
                    assert!(x.key(crate::xtest::char_keysym('k'), true));
                    assert!(wait_until(|| crate::keyboard::held_key(&surface, k)));
                    menu.popup();
                    assert!(wait_until(|| menu.is_mapped()));
                    assert!(
                        crate::keyboard::held_key(&surface, k),
                        "opening the popup releases nothing"
                    );
                    // A key pressed in the popup is the popup's, not the view's.
                    let seen = Rc::new(std::cell::Cell::new(false));
                    let probe = gtk::EventControllerKey::new();
                    probe.set_propagation_phase(gtk::PropagationPhase::Capture);
                    let flag = seen.clone();
                    probe.connect_key_pressed(move |_, key, _, _| {
                        if key == gdk::Key::j {
                            flag.set(true);
                        }
                        glib::Propagation::Proceed
                    });
                    menu.add_controller(probe);
                    let j = keycode(gdk::Key::j);
                    assert!(x.key(crate::xtest::char_keysym('j'), true));
                    assert!(wait_until(|| seen.get()), "the popup hears its key");
                    assert!(
                        !crate::keyboard::held_key(&surface, j),
                        "a popup's key is not reported to a view"
                    );
                    assert!(x.key(crate::xtest::char_keysym('j'), false));
                    assert!(x.key(crate::xtest::char_keysym('k'), false));
                    assert!(
                        wait_until(|| !crate::keyboard::held_key(&surface, k)),
                        "the popup's grab reports the release"
                    );
                    menu.popdown();
                })
                .expect("real XTest is required");
                assert!(wait_until(|| !menu.is_visible()));
                window.destroy();
            });
        }
    }
}

/// Tab goes through the stops in tree order, not by their places on
/// screen: `cargo test -p nsh-platform-linux --lib key_focus::tab_order`.
#[cfg(test)]
mod tab_order {
    use nsh_core::{EventMask, MountItem, ROOT_ID};

    use super::*;
    use crate::gtk_test::{apply, frame, run, show, surface, view, wait_until};

    /// Stops 2, 3 and 4 in tree order, 3 above 2 on screen, and a plain
    /// view (5) between them.
    fn views(surface: &Rc<Surface>) {
        let mut items = Vec::new();
        for (id, y) in [(2, 40.0), (5, 80.0), (3, 0.0), (4, 120.0)] {
            items.push(MountItem::Create {
                id,
                kind: ViewKind::View,
            });
            items.push(MountItem::Insert {
                parent: ROOT_ID,
                child: id,
                before: None,
            });
            items.push(MountItem::Frame {
                id,
                frame: frame(0.0, y, 100.0, 30.0),
            });
            if id != 5 {
                let mut events = EventMask::default();
                events.set(EventKind::Press, true);
                items.push(MountItem::Events { id, events });
            }
        }
        apply(surface, &items);
    }

    fn tab(window: &gtk::Window, back: bool) {
        window.emit_move_focus(if back {
            gtk::DirectionType::TabBackward
        } else {
            gtk::DirectionType::TabForward
        });
    }

    #[test]
    fn tab_goes_through_the_stops_in_tree_order_and_around() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let stops = [view(&surface, 2), view(&surface, 3), view(&surface, 4)];
            assert!(wait_until(|| stops.iter().all(|v| v.is_mapped())));
            for stop in &stops {
                tab(&window, false);
                assert!(stop.is_focus(), "{} next in tree order", stop.id());
            }
            tab(&window, false);
            assert!(stops[0].is_focus(), "around from the last");
            tab(&window, true);
            assert!(stops[2].is_focus(), "Shift+Tab back around");
            tab(&window, true);
            assert!(stops[1].is_focus());
            assert!(
                !view(&surface, 5).is_focusable(),
                "the plain view is no stop"
            );
            window.destroy();
        });
    }
}

/// The view the focus leaves reports `onBlur` before the one it reaches
/// reports `onFocus`, an input among them (whose own handler on the
/// window's focus is connected after the stops'):
/// `cargo test -p nsh-platform-linux --lib key_focus::blur_first`.
#[cfg(test)]
mod blur_first {
    use std::sync::{Arc, Mutex};

    use nsh_core::element::IntoElement;

    use super::*;
    use crate::gtk_test::{run, show, surface, wait_until};

    type Heard = Arc<Mutex<Vec<(&'static str, EventKind)>>>;

    struct Recorder(Heard);

    /// A field (`name`) and a stop (`first`) after it.
    #[nsh_core::element::component]
    fn FieldAndStop() -> impl IntoElement {
        use nsh_core::prelude::*;

        let heard = engine_context::<Recorder>().unwrap().0.clone();
        let hear = |name: &'static str, kind: EventKind| {
            let heard = heard.clone();
            move |_: &EventPayload| heard.lock().unwrap().push((name, kind))
        };
        view()
            .child(
                nsh_core::element::input()
                    .test_id("name")
                    .w(px(100.0))
                    .h(px(30.0))
                    .on(EventKind::Focus, hear("name", EventKind::Focus))
                    .on(EventKind::Blur, hear("name", EventKind::Blur)),
            )
            .child(
                view()
                    .test_id("first")
                    .w(px(100.0))
                    .h(px(30.0))
                    .on(EventKind::Press, |_: &EventPayload| {})
                    .on(EventKind::Focus, hear("first", EventKind::Focus))
                    .on(EventKind::Blur, hear("first", EventKind::Blur)),
            )
    }

    /// Waits until the app has handled every event sent so far.
    fn flush(surface: &Surface) {
        let (sent, received) = std::sync::mpsc::channel();
        surface.control(|control| {
            control.flush(move |done| {
                let _ = sent.send(done);
            });
        });
        assert!(wait_until(|| received.try_recv() == Ok(true)));
    }

    fn tab(window: &gtk::Window) {
        window.emit_move_focus(gtk::DirectionType::TabForward);
    }

    #[test]
    fn blur_comes_before_focus_between_a_field_and_a_stop() {
        run(|| {
            let surface = surface();
            let heard = Heard::default();
            let mut config = nsh_engine::EngineConfig::new(nsh_engine::Root::of::<FieldAndStop>());
            config.context = config.context.with(Recorder(heard.clone()));
            surface.start(config);
            let window = show(&surface);
            let mapped = |id: &str| {
                surface
                    .mounter
                    .borrow()
                    .views
                    .values()
                    .any(|v| v.state().props.test_id.as_deref() == Some(id) && v.is_mapped())
            };
            assert!(wait_until(|| mapped("name") && mapped("first")));
            let take = || {
                flush(&surface);
                std::mem::take(&mut *heard.lock().unwrap())
            };
            tab(&window);
            assert_eq!(take(), [("name", EventKind::Focus)]);
            tab(&window);
            assert_eq!(
                take(),
                [("name", EventKind::Blur), ("first", EventKind::Focus)],
                "from the field to the stop"
            );
            tab(&window);
            assert_eq!(
                take(),
                [("first", EventKind::Blur), ("name", EventKind::Focus)],
                "around, from the stop to the field"
            );
            window.destroy();
        });
    }
}

/// The arrows move the focus to the nearest stop in their direction, by
/// the stops' places on screen, not the tree's order, and a field keeps
/// Left and Right: `cargo test -p nsh-platform-linux --lib key_focus::arrows`.
#[cfg(test)]
mod arrows {
    use gdk::{Key, ModifierType as M};
    use nsh_core::{EventMask, MountItem, ROOT_ID, element::IntoElement};

    use super::*;
    use crate::gtk_test::{apply, frame, run, show, surface, view, wait_until};

    #[test]
    fn arrows_are_the_four_keys_without_modifiers() {
        assert_eq!(Arrow::of(Key::Left, M::empty()), Some(Arrow::Left));
        assert_eq!(Arrow::of(Key::KP_Right, M::empty()), Some(Arrow::Right));
        assert_eq!(Arrow::of(Key::Up, M::empty()), Some(Arrow::Up));
        assert_eq!(
            Arrow::of(Key::Down, M::LOCK_MASK),
            Some(Arrow::Down),
            "Caps Lock on"
        );
        for held in [M::SHIFT_MASK, M::CONTROL_MASK, M::ALT_MASK, M::SUPER_MASK] {
            assert_eq!(Arrow::of(Key::Left, held), None, "{held:?}");
        }
        assert_eq!(Arrow::of(Key::Tab, M::empty()), None);
    }

    /// Stops 2 (right of 3 on screen, first in the tree), 3 and 4 (below
    /// 3), and a plain view (5) right of 4.
    fn views(surface: &Rc<Surface>) {
        let mut items = Vec::new();
        for (id, x, y) in [
            (2, 120.0, 0.0),
            (3, 0.0, 0.0),
            (4, 0.0, 60.0),
            (5, 120.0, 60.0),
        ] {
            items.push(MountItem::Create {
                id,
                kind: ViewKind::View,
            });
            items.push(MountItem::Insert {
                parent: ROOT_ID,
                child: id,
                before: None,
            });
            items.push(MountItem::Frame {
                id,
                frame: frame(x, y, 70.0, 30.0),
            });
            if id != 5 {
                let mut events = EventMask::default();
                events.set(EventKind::Press, true);
                items.push(MountItem::Events { id, events });
            }
        }
        apply(surface, &items);
    }

    #[test]
    fn arrows_go_to_the_nearest_stop_that_way() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let [right, left, below] = [2, 3, 4].map(|id| view(&surface, id));
            // Laid out: the arrows go by the frames GTK allocated.
            assert!(wait_until(|| [&right, &left, &below]
                .iter()
                .all(|v| v.is_mapped() && v.width() > 0)));
            let root = surface.root.clone();
            let go = |to: Arrow| arrow(&surface, &root, to);
            assert!(go(Arrow::Right), "nothing focused: the first stop");
            assert!(right.is_focus(), "the first in tree order");
            assert!(!go(Arrow::Right), "nothing right of it");
            assert!(right.is_focus());
            assert!(go(Arrow::Left));
            assert!(left.is_focus());
            assert!(go(Arrow::Down));
            assert!(below.is_focus());
            assert!(!go(Arrow::Right), "the plain view is no stop");
            assert!(go(Arrow::Up));
            assert!(left.is_focus(), "the one in its column");
            assert!(!go(Arrow::Up), "nothing above");
            assert!(!right.is_focusable(), "unfocusable again once left");
            GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
            assert!(go(Arrow::Up), "nothing focused: the last in sight");
            assert!(below.is_focus());
            window.destroy();
        });
    }

    /// A field (`name`) and a stop (`below`) under it.
    #[nsh_core::element::component]
    fn FieldOverStop() -> impl IntoElement {
        use nsh_core::prelude::*;

        view()
            .child(
                nsh_core::element::input()
                    .test_id("name")
                    .w(px(100.0))
                    .h(px(30.0)),
            )
            .child(
                view()
                    .test_id("below")
                    .w(px(100.0))
                    .h(px(30.0))
                    .on(EventKind::Press, |_: &EventPayload| {}),
            )
    }

    #[test]
    fn a_field_keeps_left_and_right_and_leaves_with_down() {
        run(|| {
            let surface = surface();
            surface.start(nsh_engine::EngineConfig::new(nsh_engine::Root::of::<
                FieldOverStop,
            >()));
            let window = show(&surface);
            let find = |id: &str| {
                surface
                    .mounter
                    .borrow()
                    .views
                    .values()
                    .find(|v| {
                        v.state().props.test_id.as_deref() == Some(id)
                            && v.is_mapped()
                            && v.width() > 0
                    })
                    .cloned()
            };
            assert!(wait_until(
                || find("name").is_some() && find("below").is_some()
            ));
            let (name, below) = (find("name").unwrap(), find("below").unwrap());
            let editor = crate::text_input::key_widget(&name).unwrap();
            let root = surface.root.clone();
            crate::text_input::focus(&name);
            assert!(wait_until(|| editor.has_focus()));
            assert!(!arrow(&surface, &root, Arrow::Left), "the field's");
            assert!(!arrow(&surface, &root, Arrow::Right), "the field's");
            assert!(editor.has_focus());
            assert!(arrow(&surface, &root, Arrow::Down), "out of a single line");
            assert!(below.is_focus());
            assert!(arrow(&surface, &root, Arrow::Up));
            assert!(wait_until(|| editor.has_focus()), "back to the field");
            window.destroy();
        });
    }
}
