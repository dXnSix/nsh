//! A slot's target through a component that only places its children, in
//! a TSX app: the app wraps its trigger in such a component
//! (`trigger={<Passage><View/></Passage>}`), and what the slot's component
//! gives its trigger (a role, the keyboard focus, an expanded state, its
//! presses) is on the app's element inside the wrapper, not on the
//! wrapper's host, as it is on an element the app passes with no wrapper.

use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use nsh_core::{
    AccessibilityRole, EventKind, EventPayload, MountBatch, MountItem, NodeId, ViewProps,
};
use nsh_engine::Environment;

use crate::components::start_with;

const APP: &str = r##"import { mount, signal, Text, View } from "nsh";
import { Disclosure, Passage } from "test/ui";

function App() {
  const presses = signal(0);
  return (
    <View>
      <Disclosure
        trigger={
          <Passage testID="wrapper">
            <View
              testID="toggle"
              class="w-[40px] h-[20px]"
              onPress={() => presses.set(presses() + 1)}
            >
              <Text>Toggle</Text>
            </View>
          </Passage>
        }
      />
      <Text>{`Presses: ${presses()}`}</Text>
    </View>
  );
}

mount(App);
"##;

/// What the app has mounted so far: every item, in order.
#[derive(Default)]
struct Mounted(Vec<MountItem>);

impl Mounted {
    /// Takes the batches until `done` holds (or 5 s passed).
    fn until(&mut self, rx: &Receiver<MountBatch>, done: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done(self) {
            let Ok(batch) = rx.recv_deadline(deadline) else {
                break;
            };
            self.0.extend(batch.items);
        }
    }

    fn shows(&self, wanted: &str) -> bool {
        self.0
            .iter()
            .any(|i| matches!(i, MountItem::Text { text, .. } if text.text == wanted))
    }

    /// The view with test id `test_id`.
    fn view(&self, test_id: &str) -> Option<NodeId> {
        self.0.iter().find_map(|i| match i {
            MountItem::ViewProps { id, props } if props.test_id.as_deref() == Some(test_id) => {
                Some(*id)
            }
            _ => None,
        })
    }

    /// The props of view `id` now.
    fn props(&self, id: NodeId) -> ViewProps {
        self.0
            .iter()
            .rev()
            .find_map(|i| match i {
                MountItem::ViewProps { id: view, props } if *view == id => Some((**props).clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Whether view `id` listens to presses now.
    fn hears_press(&self, id: NodeId) -> bool {
        self.0
            .iter()
            .rev()
            .find_map(|i| match i {
                MountItem::Events { id: view, events } if *view == id => {
                    Some(events.contains(EventKind::Press))
                }
                _ => None,
            })
            .unwrap_or(false)
    }

    /// Whether the element `test_id` is the disclosure's button now.
    fn is_button(&self, test_id: &str) -> bool {
        self.view(test_id)
            .is_some_and(|id| self.props(id).accessibility.role == AccessibilityRole::Button)
    }
}

#[test]
fn a_slots_props_go_to_the_apps_element_in_a_wrapper_that_only_places_it() {
    let (engine, rx) = start_with(APP, Environment::default(), None, None);
    let mut mounted = Mounted::default();
    mounted.until(&rx, |m| m.shows("Presses: 0") && m.is_button("toggle"));
    let toggle = mounted.view("toggle").expect("the app's element");
    let wrapper = mounted.view("wrapper").expect("the wrapper's host");

    // The app's element is the button: focusable, collapsed, pressed.
    let props = mounted.props(toggle);
    assert_eq!(props.accessibility.role, AccessibilityRole::Button);
    assert_eq!(props.focusable, Some(true));
    assert_eq!(props.accessibility.state.expanded, Some(false));
    assert!(mounted.hears_press(toggle));
    // The wrapper's host is none of it.
    let props = mounted.props(wrapper);
    assert_eq!(props.accessibility.role, AccessibilityRole::None);
    assert_eq!(props.focusable, None);
    assert_eq!(props.accessibility.state.expanded, None);
    assert!(!mounted.hears_press(wrapper));

    // A press on the app's element is the component's (it expands) and
    // the app's own.
    engine
        .control()
        .dispatch_event(toggle, EventKind::Press, EventPayload::None);
    mounted.until(&rx, |m| {
        m.shows("Presses: 1") && m.props(toggle).accessibility.state.expanded == Some(true)
    });
    assert!(mounted.shows("Presses: 1"));
    assert_eq!(
        mounted.props(toggle).accessibility.state.expanded,
        Some(true)
    );
}

const MARKED: &str = r##"import { mount, signal, Text, View } from "nsh";
import { Disclosure, Marker } from "test/ui";

function App() {
  const presses = signal(0);
  return (
    <View>
      <Disclosure
        trigger={
          <Marker testID="marker" outlined>
            <View
              testID="toggle"
              class="w-[40px] h-[20px]"
              onPress={() => presses.set(presses() + 1)}
            >
              <Text>Toggle</Text>
            </View>
          </Marker>
        }
      />
      <Text>{`Presses: ${presses()}`}</Text>
    </View>
  );
}

mount(App);
"##;

/// The same through a marker: a component with styles of its own (hidden
/// or outlined while empty) that places its children one by one.
#[test]
fn a_slots_props_go_to_the_apps_element_in_a_marker() {
    let (engine, rx) = start_with(MARKED, Environment::default(), None, None);
    let mut mounted = Mounted::default();
    mounted.until(&rx, |m| m.shows("Presses: 0") && m.is_button("toggle"));
    let toggle = mounted.view("toggle").expect("the app's element");
    let marker = mounted.view("marker").expect("the marker's host");
    assert!(!mounted.shows("Marker"), "its name only while empty");

    let props = mounted.props(toggle);
    assert_eq!(props.accessibility.role, AccessibilityRole::Button);
    assert_eq!(props.focusable, Some(true));
    assert_eq!(props.accessibility.state.expanded, Some(false));
    assert!(mounted.hears_press(toggle));
    let props = mounted.props(marker);
    assert_eq!(props.accessibility.role, AccessibilityRole::None);
    assert_eq!(props.focusable, None);
    assert!(!mounted.hears_press(marker));

    engine
        .control()
        .dispatch_event(toggle, EventKind::Press, EventPayload::None);
    mounted.until(&rx, |m| {
        m.shows("Presses: 1") && m.props(toggle).accessibility.state.expanded == Some(true)
    });
    assert!(mounted.shows("Presses: 1"));
    assert_eq!(
        mounted.props(toggle).accessibility.state.expanded,
        Some(true)
    );
}
