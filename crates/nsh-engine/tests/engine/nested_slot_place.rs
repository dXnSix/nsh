//! A slot's place as the child of a nested component, in a TSX app: the
//! view gives a component it nests the app's slot among its children
//! (`inset().child(props.title.place(|title| title))`), and that
//! component's view places the slot's holder where it places its
//! children, when the app fills the slot from the start and when it fills
//! it later. With a component of the tests' own that only places its
//! children (`Inset`, in `Titled`) and with `nsh/base`'s `Collapsible`
//! (in `Details`), whose own slot is `content`.

use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use nsh_core::{EventKind, EventPayload, MountBatch, MountItem, NodeId};
use nsh_engine::Environment;

use crate::components::start_with;

const APP: &str = r#"import { mount, signal, Text, View } from "nsh";
import { Details, Titled } from "test/ui";

function App() {
  const late = signal(false);
  return (
    <View>
      <Titled testID="titled" title={<Text>title at mount</Text>}>
        <Text>titled body</Text>
      </Titled>
      <Details testID="details" summary={<Text>summary at mount</Text>}>
        <Text>details body</Text>
      </Details>
      <Titled testID="titled-later" title={late() ? <Text>title later</Text> : undefined} />
      <Details testID="details-later" summary={late() ? <Text>summary later</Text> : undefined} />
      <View testID="fill" onPress={() => late.set(!late())} />
      <Text>{`late ${late()}`}</Text>
    </View>
  );
}

mount(App);
"#;

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

    /// The node showing `wanted` now.
    fn text(&self, wanted: &str) -> Option<NodeId> {
        self.0.iter().rev().find_map(|i| match i {
            MountItem::Text { id, text, .. } if text.text == wanted => Some(*id),
            _ => None,
        })
    }

    /// The view with test id `test_id`.
    fn view(&self, test_id: &str) -> NodeId {
        self.0
            .iter()
            .find_map(|i| match i {
                MountItem::ViewProps { id, props } if props.test_id.as_deref() == Some(test_id) => {
                    Some(*id)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no view {test_id}"))
    }

    /// The parent of `child` now: where it was inserted last, unless it
    /// was removed since.
    fn parent(&self, child: NodeId) -> Option<NodeId> {
        self.0.iter().rev().find_map(|i| match i {
            MountItem::Insert {
                parent, child: c, ..
            } if *c == child => Some(Some(*parent)),
            MountItem::Remove { id } if *id == child => Some(None),
            _ => None,
        })?
    }

    /// Whether the text `wanted` shows inside the view `test_id`.
    fn shows_in(&self, wanted: &str, test_id: &str) -> bool {
        let within = self.view(test_id);
        let mut at = self.text(wanted);
        while let Some(id) = at {
            if id == within {
                return true;
            }
            at = self.parent(id);
        }
        false
    }
}

/// A slot the app fills from the start shows at mount, in the nested
/// component: the tests' own, and `Collapsible`, with its content.
#[test]
fn a_place_given_to_a_nested_component_as_its_child_shows_at_mount() {
    let (_engine, rx) = start_with(APP, Environment::default(), None, None);
    let mut mounted = Mounted::default();
    mounted.until(&rx, |m| m.text("late false").is_some());
    assert!(mounted.shows_in("title at mount", "titled"));
    assert!(mounted.shows_in("titled body", "titled"));
    assert!(mounted.shows_in("summary at mount", "details"));
    assert!(mounted.shows_in("details body", "details"));
}

/// A slot the app fills later shows then, leaves when the app empties it,
/// and shows again when it fills it again.
#[test]
fn a_place_given_to_a_nested_component_as_its_child_shows_when_filled_later() {
    let (engine, rx) = start_with(APP, Environment::default(), None, None);
    let mut mounted = Mounted::default();
    mounted.until(&rx, |m| m.text("late false").is_some());
    assert_eq!(mounted.text("title later"), None);
    assert_eq!(mounted.text("summary later"), None);
    let fill = mounted.view("fill");
    let press = || {
        engine
            .control()
            .dispatch_event(fill, EventKind::Press, EventPayload::None)
    };
    let shown = |m: &Mounted| {
        m.shows_in("title later", "titled-later") && m.shows_in("summary later", "details-later")
    };
    press();
    mounted.until(&rx, shown);
    assert!(mounted.shows_in("title later", "titled-later"));
    assert!(mounted.shows_in("summary later", "details-later"));
    // Emptied: out of both.
    press();
    mounted.until(&rx, |m| m.text("late false").is_some() && !shown(m));
    mounted.until(&rx, |m| {
        !m.shows_in("title later", "titled-later") && !m.shows_in("summary later", "details-later")
    });
    assert!(!mounted.shows_in("title later", "titled-later"));
    assert!(!mounted.shows_in("summary later", "details-later"));
    // Filled again.
    let seen = mounted.0.len();
    press();
    mounted.until(&rx, |m| {
        m.0[seen..]
            .iter()
            .any(|i| matches!(i, MountItem::Text { text, .. } if text.text == "late true"))
            && shown(m)
    });
    assert!(mounted.shows_in("title later", "titled-later"));
    assert!(mounted.shows_in("summary later", "details-later"));
}
