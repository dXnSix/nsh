//! `FocusGroup`: a region the arrow keys enter at its destinations and,
//! with `trap`, do not leave. Its view hands the platforms the group's
//! rules (`ViewProps::focus_group`): whether it traps, and the nodes of
//! its destinations, found once they are mounted (its own children,
//! rendered after it) and again as the app's state changes them.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use nsh_base::focus_group;
use nsh_core::{
    MountItem, NodeId, Size,
    element::{FocusTarget, Root},
    focus::GroupConfig,
    platform::Os,
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

#[derive(Clone, PartialEq, Props)]
struct RowProps {}

/// A group of three items (`first`, `second`, `third`) entered at the
/// second, then, after a press on `switch`, at the third and trapping.
#[component]
fn Row(_props: RowProps) -> impl IntoElement {
    let (second, third) = (element_ref(), element_ref());
    let switched = signal(false);
    let item = |name: &'static str| {
        view()
            .test_id(name)
            .w(px(80.0))
            .h(px(40.0))
            .on_press(|| {})
            .child(text(name))
    };
    view()
        .child(
            focus_group()
                .trap(move || switched.get())
                .destinations(move || {
                    let to = if switched.get() { &third } else { &second };
                    vec![FocusTarget::from(to)]
                })
                .flex_row()
                .child(item("first"))
                .child(item("second").element_ref(&second))
                .child(item("third").element_ref(&third)),
        )
        .child(
            view()
                .test_id("switch")
                .on_press(move || switched.set(true))
                .child(text("Switch")),
        )
}

/// The nodes by test id, and the group config each view was last sent.
#[derive(Default)]
struct Seen {
    ids: HashMap<String, NodeId>,
    groups: HashMap<NodeId, Option<GroupConfig>>,
}

fn start() -> (Headless, Arc<Mutex<Seen>>) {
    let mut config = EngineConfig {
        viewport: Size::new(px(400.0), px(300.0)),
        clock: nsh_engine::Clock::manual(std::time::SystemTime::now()),
        ..EngineConfig::new(Root::new::<Row>(|| RowProps {}))
    };
    config.environment.os = Os::MacOs;
    let mut app = Headless::start(config).expect("the engine starts");
    let seen = Arc::new(Mutex::new(Seen::default()));
    let into = seen.clone();
    app.observe(move |_, batch| {
        let mut seen = into.lock().unwrap();
        for item in &batch.items {
            if let MountItem::ViewProps { id, props } = item {
                if let Some(test_id) = &props.test_id {
                    seen.ids.insert(test_id.to_string(), *id);
                }
                seen.groups.insert(*id, props.focus_group.clone());
            }
        }
    });
    app.wait_for("text:Switch").unwrap();
    (app, seen)
}

impl Seen {
    /// What the one view that is a group was last sent.
    fn group(&self) -> GroupConfig {
        let mut groups = self.groups.values().flatten();
        let group = groups.next().cloned().expect("a group");
        assert!(groups.next().is_none(), "one group");
        group
    }
}

#[test]
fn the_group_s_view_names_its_destinations_once_they_are_mounted() {
    let (_app, seen) = start();
    let seen = seen.lock().unwrap();
    let group = seen.group();
    assert!(!group.trap);
    assert_eq!(group.destinations, [seen.ids["second"]]);
}

#[test]
fn the_app_s_state_moves_the_destinations_and_sets_the_trap() {
    let (mut app, seen) = start();
    app.press("id:switch").unwrap();
    let seen = seen.lock().unwrap();
    let group = seen.group();
    assert!(group.trap);
    assert_eq!(group.destinations, [seen.ids["third"]]);
}

#[test]
fn a_view_that_is_no_group_carries_none() {
    let (_app, seen) = start();
    let seen = seen.lock().unwrap();
    for name in ["first", "second", "switch"] {
        assert_eq!(seen.groups[&seen.ids[name]], None, "{name}");
    }
}
