//! The TV category in Rust apps: `is_tv()` holds in the TV idiom on any
//! OS and `is_desktop()` never does there, so props chosen with `is_tv()`
//! (Rust's `tv={{…}}`) apply on a TV only, and `nsh/base`'s components
//! behave on a TV as on mobile: a popover opens over a backdrop that takes
//! the presses outside it, as on a phone, not as a desktop's popup.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use nsh_base::popover;
use nsh_core::{
    MountItem, NodeId, Size,
    element::Root,
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

/// What the platform was sent: each view's parent and test id, and the
/// views put in each view in order (the last one on top).
#[derive(Default)]
struct Seen {
    parents: HashMap<NodeId, NodeId>,
    stacks: HashMap<NodeId, Vec<NodeId>>,
    test_ids: HashMap<String, NodeId>,
}

/// What the app reads of the platform, a card given a test id of its own
/// on a TV, and a popover.
#[component]
fn TvScreen() -> impl IntoElement {
    view()
        .pt(px(100.0))
        .child(text(format!(
            "TV: {} Desktop: {} Idiom: {:?}",
            is_tv(),
            is_desktop(),
            idiom()
        )))
        .child(
            view()
                .h(px(40.0))
                .when(is_tv(), |v| v.test_id("tv-card"))
                .child(text("Card")),
        )
        .child(
            popover()
                .trigger(
                    view()
                        .test_id("trigger")
                        .w(px(80.0))
                        .h(px(30.0))
                        .child(text("Open")),
                )
                .child(view().test_id("body").child(text("Body"))),
        )
}

/// [`TvScreen`] on `os` in `idiom`, with what the platform was sent.
fn start(os: Os, idiom: Idiom) -> (Headless, Arc<Mutex<Seen>>) {
    let mut config = EngineConfig {
        viewport: Size::new(px(1280.0), px(720.0)),
        ..EngineConfig::new(Root::of::<TvScreen>())
    };
    config.environment.os = os;
    config.environment.idiom = idiom;
    let mut app = Headless::start(config).expect("the engine starts");
    let seen = Arc::new(Mutex::new(Seen::default()));
    let into = seen.clone();
    app.observe(move |_, batch| {
        let mut seen = into.lock().unwrap();
        for item in &batch.items {
            match item {
                MountItem::ViewProps { id, props } => {
                    if let Some(test_id) = &props.test_id {
                        seen.test_ids.insert(test_id.to_string(), *id);
                    }
                }
                MountItem::Insert {
                    parent,
                    child,
                    before,
                } => {
                    seen.parents.insert(*child, *parent);
                    let stack = seen.stacks.entry(*parent).or_default();
                    stack.retain(|id| id != child);
                    let at = before
                        .and_then(|b| stack.iter().position(|id| *id == b))
                        .unwrap_or(stack.len());
                    stack.insert(at, *child);
                }
                _ => {}
            }
        }
    });
    app.first_render().unwrap();
    (app, seen)
}

/// What opening the popover put beside its part (around the app's
/// content), in order, the popup last: a backdrop under it on touch
/// screens.
fn opened(app: &mut Headless, seen: &Mutex<Seen>) -> (NodeId, Vec<NodeId>) {
    let before: HashMap<NodeId, Vec<NodeId>> = seen.lock().unwrap().stacks.clone();
    app.press("id:trigger").unwrap();
    app.wait_for("Body").unwrap();
    let seen = seen.lock().unwrap();
    let popup = seen.parents[&seen.test_ids["body"]];
    let parent = seen.parents[&popup];
    let old = before.get(&parent).cloned().unwrap_or_default();
    let added = seen.stacks[&parent]
        .iter()
        .copied()
        .filter(|id| !old.contains(id))
        .collect();
    (popup, added)
}

#[test]
fn is_tv_holds_in_the_tv_idiom_on_any_os_and_is_never_desktop() {
    for os in [
        Os::Android,
        Os::Web,
        Os::Ios,
        Os::MacOs,
        Os::Windows,
        Os::Linux,
    ] {
        let (app, _) = start(os, Idiom::Tv);
        assert!(
            app.shows("TV: true Desktop: false Idiom: Tv"),
            "{os:?}: {:?}",
            app.screen().texts()
        );
        assert_eq!(app.screen().count("id:tv-card").unwrap(), 1, "{os:?}");
    }
    for (os, idiom, shown) in [
        (
            Os::Android,
            Idiom::Phone,
            "TV: false Desktop: false Idiom: Phone",
        ),
        (Os::Web, Idiom::Pad, "TV: false Desktop: false Idiom: Pad"),
        (
            Os::MacOs,
            Idiom::Desktop,
            "TV: false Desktop: true Idiom: Desktop",
        ),
    ] {
        let (app, _) = start(os, idiom);
        assert!(
            app.shows(shown),
            "{os:?} {idiom:?}: {:?}",
            app.screen().texts()
        );
        assert_eq!(
            app.screen().count("id:tv-card").unwrap(),
            0,
            "{os:?} {idiom:?}"
        );
    }
}

#[test]
fn on_a_tv_a_popover_is_above_its_backdrop_as_on_a_phone() {
    for os in [Os::Android, Os::Web, Os::MacOs] {
        let (mut app, seen) = start(os, Idiom::Tv);
        let (popup, added) = opened(&mut app, &seen);
        assert_eq!(
            added.len(),
            2,
            "{os:?}: the backdrop and the popup: {added:?}"
        );
        assert_eq!(added.last(), Some(&popup), "{os:?}: the popup on top");
    }
    // A desktop's popover has no backdrop: the popup alone.
    let (mut app, seen) = start(Os::MacOs, Idiom::Desktop);
    let (popup, added) = opened(&mut app, &seen);
    assert_eq!(added, [popup], "a desktop popup has no backdrop");
}
