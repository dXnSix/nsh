//! The media and TV remote keys by the keymap's names (`red`,
//! `mediaplaypause`), as the test agent hands them to the app
//! (`app.keyboard.press`): no terminal names them, so the agent does.

use nsh_core::prelude::*;
use nsh_platform_tui::{App, Handled, Input, Root};
use ratatui_core::{backend::TestBackend, terminal::Terminal};

use super::app::{draw_until, surface};

/// The remote's keys the test presses.
const KEYS: [&str; 5] = ["red", "mediaplaypause", "channelup", "info", "medianext"];

#[component]
fn RemotePage() -> impl IntoElement {
    let heard = signal(String::new());
    for name in KEYS {
        on_key(name, move |_| {
            heard.update(|h| h.push_str(&format!(" {name}")))
        });
    }
    view()
        .flex_1()
        .child(text(move || format!("heard{}", heard.get())))
}

#[test]
fn the_media_and_tv_keys_fire_their_bindings() {
    let (mut view, rx) = surface();
    let mut terminal = Terminal::new(TestBackend::new(60, 4)).unwrap();
    view.load(App::Rust(Root::new::<RemotePage>(|| ())))
        .unwrap();
    draw_until(&mut view, &mut terminal, &rx, "heard");
    for name in KEYS {
        let pressed = view.handle(Input::Key {
            key: name.into(),
            modifiers: Default::default(),
            release: false,
        });
        assert_eq!(pressed, Handled::Yes, "{name}");
    }
    draw_until(
        &mut view,
        &mut terminal,
        &rx,
        "heard red mediaplaypause channelup info medianext",
    );
}
