//! Actual-Wasm ownership fixtures, excluded from published artifacts.
use std::cell::RefCell;

use nsh_core::prelude::*;

thread_local! {
    static KEPT: RefCell<Vec<Callback<(), i32>>> = const { RefCell::new(Vec::new()) };
    static LISTENING: RefCell<Vec<(u32, Callback<(), i32>)>> = const { RefCell::new(Vec::new()) };
}

#[class]
pub(crate) struct CallbackBox {
    callback: Callback<(), i32>,
}

#[class]
impl CallbackBox {
    pub(crate) fn new(callback: Callback<(), i32>) -> Self {
        Self { callback }
    }
    pub(crate) fn call(&self) -> i32 {
        self.callback.call(()).unwrap_or(-1)
    }
    pub(crate) fn escape(&self) {
        KEPT.with(|kept| kept.borrow_mut().push(self.callback.clone()));
    }
    pub(crate) fn call_kept() -> i32 {
        let callback = KEPT.with(|kept| kept.borrow().last().cloned());
        callback
            .and_then(|callback| callback.call(()).ok())
            .unwrap_or(-1)
    }
    pub(crate) fn clear_kept() {
        let callbacks = KEPT.with(|kept| kept.take());
        drop(callbacks);
    }
    pub(crate) fn listen(callback: Callback<(), i32>) -> Stop {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        LISTENING.with(|l| l.borrow_mut().push((id, callback)));
        Stop::new(move || {
            let gone = LISTENING.with(|l| {
                let mut l = l.borrow_mut();
                let i = l.iter().position(|(i, _)| *i == id);
                i.map(|i| l.remove(i))
            });
            drop(gone);
        })
    }
    pub(crate) fn listening() -> u32 {
        LISTENING.with(|l| l.borrow().len() as u32)
    }
}

pub(super) fn registry() -> nsh_core::element::Registry {
    let mut registry = nsh_core::element::Registry::ui_set("test/browser");
    registry.class::<CallbackBox>();
    registry
}

/// Methods another set adds to `CallbackBox` (`class-extensions.test.mjs`).
pub(crate) struct BoxExtras;

#[class_extension(CallbackBox)]
impl BoxExtras {
    /// The box itself, to chain on.
    pub(crate) fn chain(this: Instance<CallbackBox>) -> Instance<CallbackBox> {
        this
    }
    /// What `f` makes of what the box's own function returns, called on
    /// the spot.
    pub(crate) fn call_with(this: Instance<CallbackBox>, f: Callback<i32, i32>) -> i32 {
        let own = this.with(|b| b.callback.clone());
        let value = own.call(()).unwrap_or(-1);
        f.call(value).unwrap_or(-1)
    }
    /// Refuses with `why`.
    pub(crate) fn refuse(this: Instance<CallbackBox>, why: String) -> Result<(), String> {
        let _ = this;
        Err(why)
    }
    /// A task of the core.
    pub(crate) async fn later(this: Instance<CallbackBox>) -> i32 {
        let _ = this;
        7
    }
}

/// The set registering `BoxExtras`, beside the class's.
pub(super) fn extension_registry() -> nsh_core::element::Registry {
    let mut registry = nsh_core::element::Registry::ui_set("test/browser-extension");
    registry.class_extension::<BoxExtras>();
    registry
}

/// Exercise the Rust-to-browser event conversion with the actual value bridge.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nsh_browser_test_mouse_events() -> u32 {
    use nsh_core::{EventPayload, MouseButton, MouseEvent, keymap::Modifiers};

    use super::{bindings::Bindings, events, values::Value};
    let host = Bindings::new();
    let buttons = [
        None,
        Some(MouseButton::Left),
        Some(MouseButton::Right),
        Some(MouseButton::Middle),
        Some(MouseButton::Back),
        Some(MouseButton::Forward),
    ];
    let events = buttons.into_iter().map(|button| {
        events::payload(
            &host,
            &EventPayload::Mouse(MouseEvent::new(
                Point::new(Points(3.0), Points(4.0)),
                Point::new(Points(30.0), Points(40.0)),
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
                button,
            )),
        )
        .unwrap()
    });
    Value::array(events).unwrap().into_raw()
}

/// Both file-drop variants keep the same public shape as the native JS host.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nsh_browser_test_file_drop_events() -> u32 {
    use nsh_core::EventPayload;

    use super::{bindings::Bindings, events, values::Value};
    let host = Bindings::new();
    let uris = vec![
        "file:///tmp/hello.txt".to_owned(),
        "file:///tmp/world.txt".to_owned(),
    ];
    let payloads = [
        EventPayload::FileDrop(uris.clone()),
        EventPayload::FileDropAt {
            uris,
            page_position: Point::new(Points(30.0), Points(40.0)),
        },
    ];
    Value::array(payloads.iter().map(|p| events::payload(&host, p).unwrap()))
        .unwrap()
        .into_raw()
}

/// Touch points keep the pointer fields the native JS host gives them
/// (`pointerType`, `pressure`, `tiltX`, `tiltY`): a mouse, a pen and a finger.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nsh_browser_test_touch_events() -> u32 {
    use nsh_core::{
        EventPayload, PointerType, TouchEvent, TouchPhase, TouchPoint, keymap::Modifiers,
    };

    use super::{bindings::Bindings, events, values::Value};
    let host = Bindings::new();
    let pointers = [
        (PointerType::Mouse, 0.5, 0.0, 0.0),
        (PointerType::Pen, 0.25, 30.0, -15.0),
        (PointerType::Touch, 0.5, 0.0, 0.0),
    ];
    let events = pointers
        .into_iter()
        .map(|(pointer_type, pressure, tilt_x, tilt_y)| {
            events::payload(
                &host,
                &EventPayload::Touch(Box::new(TouchEvent {
                    phase: TouchPhase::Move,
                    timestamp: 10.0,
                    points: vec![TouchPoint {
                        id: 1,
                        target: 2,
                        path: vec![2, 1],
                        page: Point::new(Points(30.0), Points(40.0)),
                        location: Point::new(Points(3.0), Points(4.0)),
                        changed: true,
                        pointer_type,
                        pressure,
                        tilt_x,
                        tilt_y,
                    }],
                    activated: None,
                    modifiers: Modifiers::default(),
                })),
            )
            .unwrap()
        });
    Value::array(events).unwrap().into_raw()
}

/// A video's `onLoad` keeps the `{ duration, size }` of the native JS host.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nsh_browser_test_video_load_event() -> u32 {
    use nsh_core::PayloadValue;

    use super::events;
    events::video_load(&[
        ("duration", PayloadValue::Number(12.5)),
        ("width", PayloadValue::Number(1920.0)),
        ("height", PayloadValue::Number(1080.0)),
    ])
    .unwrap()
    .into_raw()
}

/// The environment the engine sends the page's runtime when an app starts
/// (`Host::set_environment`) in a 1920 × 1080 dark window, from what the
/// page last told the module (`nsh_environment`: its `idiom`, ...).
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nsh_browser_test_environment() -> u32 {
    use nsh_core::element::JsHost;

    use super::{bindings::Bindings, engine};
    let environment = crate::environment(1920.0, 1080.0, true, 1.0);
    let data = engine::environment_data(&environment);
    Bindings::new().json(&data).unwrap().0[0] as u32
}

thread_local! {
    /// The core [`nsh_browser_test_turn`] applies the page's theme writes
    /// in, with `nsh/base`'s sets (their roles), as the engine's.
    static THEME_CORE: RefCell<Option<nsh_core::Core<nsh_core::ApproxTextEngine>>> =
        const { RefCell::new(None) };
}

/// Ends the page's turn as the engine ends one for the theme
/// (`theme-format.test.mjs`): the theme writes the runtime collected
/// (`Prop::UiAppTheme`, `UiThemeDocument`, `UiThemeUpdate`, `UiThemeReset`,
/// `UiModifier` on the root) reach a core, which resolves them once in its
/// commit, and the tokens it resolved reach the runtime
/// (`Host::set_tokens`) when they are new. The first call makes the core
/// with the runtime's sets and `nsh/base`'s, and hands its tokens first
/// (`Host::initialize`'s). The other ops are dropped.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nsh_browser_test_turn() {
    use nsh_core::{Op, Prop};
    use nsh_engine::browser::Host;

    let Some(runtime) = super::REACTIVE.with(|slot| slot.borrow().clone()) else {
        return;
    };
    let ops = std::mem::take(&mut *runtime.bindings.ops.borrow_mut());
    let tokens = THEME_CORE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let core = slot.get_or_insert_with(|| {
            let viewport = nsh_core::Size::new(nsh_core::Points(800.0), nsh_core::Points(600.0));
            let mut core = nsh_core::Core::new(nsh_core::ApproxTextEngine, viewport);
            let sets: Vec<_> = nsh_core::runtime::registries()
                .iter()
                .cloned()
                .chain([nsh_base::registry()])
                .collect();
            core.set_components(nsh_core::element::Registry::merged(&sets));
            core
        });
        let held = runtime.tokens.borrow().clone();
        let first = held.is_none();
        for op in ops {
            let theme = matches!(
                &op,
                Op::SetProp {
                    id: nsh_core::ROOT_ID,
                    prop: Prop::UiAppTheme(..)
                        | Prop::UiThemeDocument(_)
                        | Prop::UiThemeUpdate(_)
                        | Prop::UiThemeReset(_)
                        | Prop::UiModifier(..),
                }
            );
            if theme {
                let _ = core.apply(op);
            }
        }
        let _ = core.commit();
        let tokens = core.tokens().clone();
        let new = first || !held.is_some_and(|held| std::sync::Arc::ptr_eq(&held, &tokens));
        new.then_some(tokens)
    });
    if let Some(tokens) = tokens {
        runtime.set_tokens(tokens);
    }
}
