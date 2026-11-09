//! The layout direction follows the language the app shows in: a TSX
//! app's `setLocale` and a Rust app's `set_locale` lay it out right to left
//! for a language written so, left to right for the others, and in the
//! platform's direction again when they follow the user's language; a
//! view's own `direction` keeps its subtree's.

use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, EventKind, EventPayload, MountBatch, MountItem,
    element::{Root, layout_rtl},
    platform::Locale,
    prelude::*,
};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::ChannelSink;

/// What the surface shows: its texts, its root's direction, and where the
/// boxes of [`APP`] are in their rows.
#[derive(Default)]
struct Surface {
    texts: Vec<(u32, String)>,
    rtl: Option<bool>,
    /// A box's x in its row, by its width (each box is 7 points high).
    boxes: Vec<(f32, f32)>,
    /// The texts of the batch that last turned the root.
    turned_with: Vec<String>,
}

impl Surface {
    fn take(&mut self, batch: &MountBatch) {
        let turns = |i: &MountItem| matches!(i, MountItem::LayoutDirection { .. });
        if batch.items.iter().any(turns) {
            self.turned_with.clear();
        }
        for item in &batch.items {
            match item {
                MountItem::Text { id, text, .. } => {
                    self.texts.retain(|(t, _)| t != id);
                    self.texts.push((*id, text.text.clone()));
                    if batch.items.iter().any(turns) {
                        self.turned_with.push(text.text.clone());
                    }
                }
                MountItem::LayoutDirection { rtl } => self.rtl = Some(*rtl),
                MountItem::Frame { frame, .. } if frame.size.height.0 == 7.0 => {
                    let width = frame.size.width.0;
                    self.boxes.retain(|(w, _)| *w != width);
                    self.boxes.push((width, frame.origin.x.0));
                }
                _ => {}
            }
        }
    }

    /// The batches until `done` holds (or 5 seconds passed).
    fn until(&mut self, rx: &Receiver<MountBatch>, done: impl Fn(&Self) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done(self) {
            let Ok(batch) = rx.recv_deadline(deadline) else {
                return false;
            };
            self.take(&batch);
        }
        true
    }

    fn shows(&self, text: &str) -> bool {
        self.texts.iter().any(|(_, t)| t == text)
    }

    fn press(&self, engine: &Engine, label: &str) {
        let id = self
            .texts
            .iter()
            .find(|(_, t)| t == label)
            .unwrap_or_else(|| panic!("no {label}: {:?}", self.texts))
            .0;
        engine
            .control()
            .dispatch_event(id, EventKind::Press, EventPayload::None);
    }

    /// The x of the box `width` points wide.
    fn x(&self, width: f32) -> Option<f32> {
        self.boxes.iter().find(|(w, _)| *w == width).map(|b| b.1)
    }
}

/// Two rows of two boxes in a 390-point surface, the second pinned left to
/// right, the languages to show and the one shown.
const APP: &str = r#"import { locale, mount, Text, View } from "nsh";
import { setLocale } from "nsh/i18n";

mount(() => (
  <View style={{ flex: 1 }}>
    <View style={{ flexDirection: "row" }}>
      <View style={{ width: 123, height: 7 }} />
      <View style={{ width: 61, height: 7 }} />
    </View>
    <View style={{ flexDirection: "row", direction: "ltr" }}>
      <View style={{ width: 37, height: 7 }} />
      <View style={{ width: 19, height: 7 }} />
    </View>
    <Text onPress={() => setLocale("ar-EG")}>Arabic</Text>
    <Text onPress={() => setLocale("en-GB")}>English</Text>
    <Text onPress={() => setLocale(null)}>Device</Text>
    <Text>{`shown in ${locale()}`}</Text>
  </View>
));
"#;

/// The first row left to right: its first box at the left.
fn left_to_right(s: &Surface) -> bool {
    s.rtl == Some(false) && s.x(123.0) == Some(0.0) && s.x(61.0) == Some(123.0)
}

/// The first row right to left: its first box at the right.
fn right_to_left(s: &Surface) -> bool {
    s.rtl == Some(true) && s.x(123.0) == Some(390.0 - 123.0) && s.x(61.0) == Some(390.0 - 184.0)
}

/// The pinned row stays left to right.
fn pinned(s: &Surface) -> bool {
    s.x(37.0) == Some(0.0) && s.x(19.0) == Some(37.0)
}

#[test]
fn a_tsx_apps_layout_follows_the_language_it_sets() {
    let (engine, rx) = crate::components::start_with(APP, Environment::default(), None, None);
    let mut s = Surface::default();
    assert!(s.until(&rx, |s| s.shows("Device") && left_to_right(s)));
    assert!(pinned(&s));
    // Arabic is written right to left: the surface turns with the texts
    // that show it, the row pinned left to right keeps its direction.
    s.press(&engine, "Arabic");
    assert!(s.until(&rx, right_to_left), "{:?} {:?}", s.rtl, s.boxes);
    assert!(
        s.turned_with.iter().any(|t| t == "shown in ar-EG"),
        "{:?}",
        s.turned_with
    );
    assert!(pinned(&s), "{:?}", s.boxes);
    s.press(&engine, "English");
    assert!(s.until(&rx, left_to_right), "{:?} {:?}", s.rtl, s.boxes);
    // Following the user's language again is the platform's direction.
    s.press(&engine, "Arabic");
    assert!(s.until(&rx, right_to_left));
    s.press(&engine, "Device");
    assert!(s.until(&rx, left_to_right), "{:?} {:?}", s.rtl, s.boxes);
    assert!(pinned(&s));
}

#[test]
fn an_app_on_a_right_to_left_platform_turns_and_comes_back() {
    let arabic = Environment {
        rtl: true,
        locale: Locale::parse("ar-EG").unwrap(),
        ..Environment::default()
    };
    let (engine, rx) = crate::components::start_with(APP, arabic, None, None);
    let mut s = Surface::default();
    assert!(s.until(&rx, |s| s.shows("Device") && right_to_left(s)));
    s.press(&engine, "English");
    assert!(s.until(&rx, left_to_right), "{:?} {:?}", s.rtl, s.boxes);
    assert!(pinned(&s));
    s.press(&engine, "Device");
    assert!(s.until(&rx, right_to_left), "{:?} {:?}", s.rtl, s.boxes);
}

/// A Rust app showing whether it lays out right to left, and the languages
/// to show.
#[component]
fn Languages() -> impl IntoElement {
    view()
        .flex_1()
        .child(text(|| format!("right to left: {}", layout_rtl())))
        .child(text("Hebrew").on_press(|_: &()| set_locale(Locale::parse("he"))))
        .child(text("Device").on_press(|_: &()| set_locale(None)))
}

#[test]
fn a_rust_apps_layout_follows_the_language_it_sets() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig::new(App::Rust(Root::of::<Languages>()));
    let engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
    let mut s = Surface::default();
    // The component reads the direction the root has.
    let laid_out = |rtl: bool| {
        move |s: &Surface| s.rtl == Some(rtl) && s.shows(&format!("right to left: {rtl}"))
    };
    assert!(s.until(&rx, laid_out(false)));
    s.press(&engine, "Hebrew");
    assert!(s.until(&rx, laid_out(true)), "{:?} {:?}", s.rtl, s.texts);
    s.press(&engine, "Device");
    assert!(s.until(&rx, laid_out(false)), "{:?} {:?}", s.rtl, s.texts);
}
