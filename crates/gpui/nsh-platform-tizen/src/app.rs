//! The app's one window: a full-screen GPUI window holding [`TizenApp`], NSH's
//! `AppView` with the question whether to leave the app over it
//! ([`crate::back`]). It joins `AppView` to the Tizen platform:
//!
//! - the TV's settings (`dxn_gpui_tizen::settings`) go to `AppView`'s
//!   setters before the app loads, so its engine starts with them, and
//!   again as they change ([`crate::settings`], [`opening`]);
//! - the window's activation, which the platform turns off while the app
//!   framework has the app paused, is the app's state ([`crate::lifecycle`]);
//! - a back press nothing in the app took asks before leaving, and the
//!   question answered `Exit` (or the app's `exitApp`, which closes the
//!   window) quits GPUI, which ends `ui_app_main`.
//!
//! It is built and checked on the desktops too; only Tizen opens it.

use dxn_gpui_tizen::settings::{self as tv_settings, SubscriptionId, SystemSettings};
use futures_util::StreamExt as _;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render,
    SharedString, Styled as _, Window, WindowBounds, WindowOptions, div,
    prelude::FluentBuilder as _, px, rgb, rgba,
};
use nsh_platform_gpui::{AppOptions, AppView};

use crate::{
    back::{Choice, Effect, ExitPrompt},
    settings::{self, HostValues, Setter},
};

/// What the window runs.
#[cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]
// Made once, when the app starts: the size of a bundle is no cost.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Source {
    /// The packaged app: its bundle (or a Rust app's root).
    #[cfg_attr(feature = "dev", allow(dead_code))]
    App(nsh_platform_gpui::App),
    /// The development runtime: the dev server at the URL, connected
    /// through the TV's port mappings (`crate::dev`).
    #[cfg(feature = "dev")]
    Dev(String),
}

/// Opens the app's full-screen window running `source` (in `ui_app_main`'s
/// `create`, from `dxn_gpui_tizen::run`). `name` is what the exit
/// question names.
#[cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]
pub(crate) fn launch(source: Source, options: AppOptions, name: Option<String>, cx: &mut App) {
    // The app's `exitApp` (and `Application.quit()`) closes its window:
    // the app leaves with it.
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
    let window = WindowOptions {
        window_bounds: Some(WindowBounds::Fullscreen(gpui::Bounds::maximized(None, cx))),
        app_id: Some(options.app_id.clone()),
        // Animations run at the display's rate, as on the other backends.
        inactive_frame_interval: None,
        ..Default::default()
    };
    let opened = cx.open_window(window, move |window, cx| {
        cx.new(|cx| TizenApp::new(source, options, name, window, cx))
    });
    if let Err(err) = opened {
        log::error!("cannot open the app's window: {err:#}");
        cx.quit();
    }
}

/// What the window does to `AppView` as it opens, in order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Opening {
    /// One of the TV's settings, through `AppView`'s setter.
    Set(Setter),
    /// The app (or the dev server's): its engine starts with what was set.
    Load,
}

/// The steps for the TV's settings `tv` as the window opens, and the values
/// they give `AppView` (what later changes are compared with): every
/// setter, then the app.
pub(crate) fn opening(tv: &SystemSettings) -> (HostValues, Vec<Opening>) {
    let values = HostValues::of(tv);
    let mut steps: Vec<Opening> = settings::changes(None, &values)
        .into_iter()
        .map(Opening::Set)
        .collect();
    steps.push(Opening::Load);
    (values, steps)
}

/// `AppView` filling the window, and the exit question over it.
pub(crate) struct TizenApp {
    view: Entity<AppView>,
    prompt: ExitPrompt,
    /// The question's keys while it is shown.
    prompt_focus: FocusHandle,
    /// The app's name in the question.
    name: Option<SharedString>,
    /// The values last given to `AppView`'s setters.
    applied: Option<HostValues>,
    settings: Option<SubscriptionId>,
}

impl TizenApp {
    fn new(
        source: Source,
        options: AppOptions,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TizenApp {
        let view = cx.new(|cx| AppView::new(options, window, cx));
        let this = cx.entity().downgrade();
        view.read(cx).on_unhandled_back(move |window, cx| {
            let _ = this.update(cx, |app, cx| app.unhandled_back(window, cx));
        });
        // The app framework's pause and resume, as the platform's window
        // activation.
        cx.observe_window_activation(window, |app, window, cx| {
            app.window_activation(window.is_window_active(), window, cx);
        })
        .detach();
        let active = window.is_window_active();
        view.update(cx, |view, cx| {
            view.set_lifecycle(Some(crate::lifecycle::of_window(active)), cx)
        });
        // The TV's settings whenever they change (the platform calls
        // subscribers on the main thread, outside GPUI's updates: they are
        // applied from a task), subscribed before they are read below so
        // that no change goes missing.
        let (tx, mut rx) = futures_channel::mpsc::unbounded::<SystemSettings>();
        let subscription = tv_settings::subscribe(move |settings| {
            let _ = tx.unbounded_send(settings.clone());
        });
        cx.spawn(async move |this, cx| {
            while let Some(settings) = rx.next().await {
                if this
                    .update(cx, |app, cx| app.settings_changed(&settings, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        // The TV's settings now, then the app: its engine starts with them
        // (`load` reads the environment once, for the app's first code).
        let (applied, steps) = opening(&tv_settings::current());
        let mut source = Some(source);
        for step in steps {
            match step {
                Opening::Set(setter) => {
                    view.update(cx, |view, cx| settings::apply(view, setter, cx))
                }
                Opening::Load => match source.take() {
                    Some(Source::App(app)) => view.read(cx).load(app),
                    #[cfg(feature = "dev")]
                    Some(Source::Dev(url)) => {
                        view.update(cx, |view, cx| crate::dev::connect(view, url, cx))
                    }
                    None => {}
                },
            }
        }
        let focus = view.focus_handle(cx);
        window.focus(&focus, cx);
        TizenApp {
            view,
            prompt: ExitPrompt::default(),
            prompt_focus: cx.focus_handle(),
            name: name.map(SharedString::from),
            applied: Some(applied),
            settings: Some(subscription),
        }
    }

    /// The TV's settings, read again: `AppView` gets what changed.
    fn settings_changed(&mut self, tv: &SystemSettings, cx: &mut Context<Self>) {
        let values = HostValues::of(tv);
        let changes = settings::changes(self.applied.as_ref(), &values);
        self.applied = Some(values);
        if changes.is_empty() {
            return;
        }
        self.view.update(cx, |view, cx| {
            for setter in changes {
                settings::apply(view, setter, cx);
            }
        });
    }

    /// The window became the active one (the app resumed) or stopped
    /// being it (paused).
    fn window_activation(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.view.update(cx, |view, cx| {
            view.set_lifecycle(Some(crate::lifecycle::of_window(active)), cx)
        });
        if !active {
            let effect = self.prompt.paused();
            self.perform(effect, window, cx);
        }
    }

    /// A back press nothing in the app took.
    fn unhandled_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.prompt.unhandled_back();
        self.perform(effect, window, cx);
    }

    /// A key while the question has the keys.
    fn prompt_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        let Some(key) = crate::keys::remote(&event.keystroke.key) else {
            return;
        };
        let effect = self.prompt.key(key);
        self.perform(effect, window, cx);
    }

    /// An answer pointed at.
    fn answer(&mut self, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.prompt.answer(choice);
        self.perform(effect, window, cx);
    }

    fn perform(&mut self, effect: Effect, window: &mut Window, cx: &mut Context<Self>) {
        match effect {
            Effect::None => return,
            Effect::Show => window.focus(&self.prompt_focus, cx),
            Effect::Hide => {
                let focus = self.view.focus_handle(cx);
                window.focus(&focus, cx);
            }
            Effect::Exit => {
                log::info!("leaving the app (Back on its first screen)");
                cx.quit();
            }
        }
        cx.notify();
    }

    /// The question: a card over the dimmed app, its two answers in a row.
    fn render_prompt(&self, chosen: Choice, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let title = match &self.name {
            Some(name) => SharedString::from(format!("Exit {name}?")),
            None => SharedString::new_static("Exit the app?"),
        };
        let button = |choice: Choice, label: &'static str, cx: &mut Context<Self>| {
            let chosen = choice == chosen;
            div()
                .id(label)
                .px(px(28.0))
                .py(px(10.0))
                .rounded(px(10.0))
                .text_size(px(18.0))
                .font_weight(FontWeight::MEDIUM)
                .border_2()
                .when(chosen, |b| {
                    b.bg(rgb(0xf5f5f5))
                        .text_color(rgb(0x0a0a0a))
                        .border_color(rgb(0xffffff))
                })
                .when(!chosen, |b| {
                    b.bg(rgb(0x262626))
                        .text_color(rgb(0xe5e5e5))
                        .border_color(rgb(0x404040))
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |app, _, window, cx| app.answer(choice, window, cx)),
                )
                .child(label)
        };
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x0000_00b3))
            .track_focus(&self.prompt_focus)
            .on_key_down(cx.listener(Self::prompt_key))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(28.0))
                    .px(px(48.0))
                    .py(px(36.0))
                    .rounded(px(20.0))
                    .bg(rgb(0x171717))
                    .border_1()
                    .border_color(rgb(0x2e2e2e))
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xfafafa))
                            .child(title),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(20.0))
                            .child(button(Choice::Stay, "Stay", cx))
                            .child(button(Choice::Exit, "Exit", cx)),
                    ),
            )
    }
}

impl Drop for TizenApp {
    fn drop(&mut self) {
        if let Some(id) = self.settings.take() {
            tv_settings::unsubscribe(id);
        }
    }
}

impl Render for TizenApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prompt = self
            .prompt
            .shown()
            .map(|chosen| self.render_prompt(chosen, cx));
        div()
            .size_full()
            .relative()
            .child(self.view.clone())
            .children(prompt)
    }
}

#[cfg(test)]
mod tests {
    use dxn_gpui_tizen::settings::SystemSettings;
    use nsh_platform_gpui::{Appearance, Locale};

    use super::*;

    fn chinese_tv() -> SystemSettings {
        SystemSettings {
            locale: Some("zh-CN".into()),
            time_zone: Some("Asia/Shanghai".into()),
            font_scale: Some(1.5),
            screen_reader: Some(true),
            high_contrast: None,
            reduce_motion: Some(true),
        }
    }

    #[test]
    fn the_tvs_settings_are_set_before_the_app_loads() {
        let tv = chinese_tv();
        let (applied, steps) = opening(&tv);
        assert_eq!(applied, HostValues::of(&tv));
        // Every setter, then the app, once.
        assert_eq!(steps.last(), Some(&Opening::Load));
        assert_eq!(steps.iter().filter(|s| **s == Opening::Load).count(), 1);
        let set: Vec<Setter> = steps
            .iter()
            .filter_map(|step| match step {
                Opening::Set(setter) => Some(*setter),
                Opening::Load => None,
            })
            .collect();
        assert_eq!(set, settings::changes(None, &applied));
        for setter in [
            Setter::Locale(Locale::parse("zh-CN")),
            Setter::FontScale(Some(1.5)),
            Setter::ScreenReader(Some(true)),
            Setter::ReduceMotion(Some(true)),
            Setter::Appearance(Appearance::Dark),
        ] {
            assert!(set.contains(&setter), "{setter:?}");
        }
        // The same settings reported again change nothing.
        assert_eq!(
            settings::changes(Some(&applied), &HostValues::of(&tv)),
            Vec::new()
        );
    }

    #[test]
    fn a_tv_that_reports_nothing_still_loads_after_the_defaults() {
        let (applied, steps) = opening(&SystemSettings::default());
        assert_eq!(applied.font_scale, None);
        assert_eq!(steps.len(), 7);
        assert_eq!(steps[6], Opening::Load);
    }
}
