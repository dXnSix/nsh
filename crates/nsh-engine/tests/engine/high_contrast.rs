//! The user's contrast setting: a TSX app's `highContrast()` and its
//! `contrast-more:` classes, and a Rust component's `high_contrast()`,
//! follow what the platform reports, while the app runs.

use crossbeam_channel::Receiver;
use nsh_core::{ApproxTextEngine, Color, MountBatch, MountItem, element::Root, prelude::*};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::{
    ChannelSink,
    components::{shows, start_with, until},
};

const APP: &str = r#"import { highContrast, mount, Text, View } from "nsh";

mount(() => (
  <View class="bg-white contrast-more:bg-black">
    <Text>{`contrast ${highContrast() ? "more" : "standard"}`}</Text>
  </View>
));
"#;

/// The batches until one shows `text`, and whether one did.
fn until_shown(rx: &Receiver<MountBatch>, text: &str) -> (Vec<MountBatch>, bool) {
    let batches = until(rx, |b| shows(b, text));
    let shown = batches.iter().any(|b| shows(b, text));
    (batches, shown)
}

/// The last opaque background `batches` give a view.
fn background(batches: &[MountBatch]) -> Option<Color> {
    batches
        .iter()
        .flat_map(|b| &b.items)
        .rev()
        .find_map(|i| match i {
            MountItem::ViewStyle { style, .. } if style.background_color.0 & 0xff == 0xff => {
                Some(style.background_color)
            }
            _ => None,
        })
}

const WHITE: Color = Color(0xffff_ffff);
const BLACK: Color = Color(0x0000_00ff);

#[test]
fn a_tsx_app_follows_the_users_contrast_setting() {
    let environment = Environment::default();
    let (engine, rx) = start_with(APP, environment, None, None);
    let (first, shown) = until_shown(&rx, "contrast standard");
    assert!(shown, "{first:?}");
    assert_eq!(background(&first), Some(WHITE));
    // The user turns Increase Contrast on: the accessor and the class follow.
    engine.control().set_environment(Environment {
        high_contrast: true,
        ..environment
    });
    let (more, shown) = until_shown(&rx, "contrast more");
    assert!(shown, "{more:?}");
    assert_eq!(background(&more), Some(BLACK));
    engine.control().set_environment(environment);
    let (standard, shown) = until_shown(&rx, "contrast standard");
    assert!(shown, "{standard:?}");
    assert_eq!(background(&standard), Some(WHITE));
}

#[test]
fn an_app_starts_with_the_contrast_the_platform_reports() {
    let environment = Environment {
        high_contrast: true,
        ..Environment::default()
    };
    let (_engine, rx) = start_with(APP, environment, None, None);
    let (first, shown) = until_shown(&rx, "contrast more");
    assert!(shown, "{first:?}");
    assert_eq!(background(&first), Some(BLACK));
}

/// A Rust component showing the user's contrast setting.
#[component]
fn Contrast() -> impl IntoElement {
    view().flex_1().child(text(|| {
        format!(
            "contrast {}",
            if high_contrast() { "more" } else { "standard" }
        )
    }))
}

#[test]
fn a_rust_component_follows_the_users_contrast_setting() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig::new(App::Rust(Root::of::<Contrast>()));
    let engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
    let environment = Environment::default();
    assert!(until_shown(&rx, "contrast standard").1);
    engine.control().set_environment(Environment {
        high_contrast: true,
        ..environment
    });
    assert!(until_shown(&rx, "contrast more").1);
    engine.control().set_environment(environment);
    assert!(until_shown(&rx, "contrast standard").1);
}
