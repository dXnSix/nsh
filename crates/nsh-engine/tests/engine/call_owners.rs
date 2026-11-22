//! A synchronous call from JS running its Rust reactive code under the Rust
//! owner tied to its JS owner (`CallCx::with_owner`), through an engine:
//! what a call in a TSX component's body provides is seen by the calls in
//! the components under it and by the Rust components JS makes there, not
//! by a sibling; its `cleanup` runs when the component goes; the Rust graph
//! is entered for the call and the core's executor is not
//! (`runtime_entered` stays `false`); a call with no JS owner runs under
//! none; the lazy children of a Rust component see what its view
//! provides, its eager children what is provided around it.

use std::{cell::RefCell, time::Duration};

use nsh_core::{element::Registry, prelude::*};

use crate::rust_classes::{shown, start};

thread_local! {
    /// What the tests' Rust code saw, in order.
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(line: String) {
    LOG.with_borrow_mut(|log| log.push(line));
}

/// What one call provided.
#[derive(Clone)]
struct Label(String);

fn label() -> String {
    use_context::<Label>().map_or_else(|| "none".into(), |label| label.0)
}

/// The calls TSX makes.
#[module]
pub(crate) struct Owners;

#[module]
impl Owners {
    /// Provides `label` to what the calling JS owner makes; whether a JS
    /// owner owns the call.
    pub(crate) fn provide(label: String, cx: CallCx) -> bool {
        cx.with_owner(|| provide_context(Label(label)));
        cx.has_owner()
    }

    /// The nearest label provided above the calling JS owner.
    pub(crate) fn read(cx: CallCx) -> String {
        cx.with_owner(label)
    }

    /// Logs `name` when the calling JS owner goes.
    pub(crate) fn on_cleanup(name: String, cx: CallCx) {
        cx.with_owner(|| cleanup(move || log(format!("cleanup {name}"))));
    }

    /// In the call's Rust code: whether the runtime counts as entered,
    /// whether a signal made there works, and whether a task starts.
    pub(crate) fn entered(cx: CallCx) -> String {
        cx.with_owner(|| {
            let count = signal(1);
            count.update(|n| *n += 1);
            let started = start_task(async {
                timer(Duration::from_millis(1)).await;
                log("task ran".into());
            })
            .is_ok();
            format!("{} {} {started}", runtime_entered(), count.get_untracked())
        })
    }

    /// What the Rust code logged.
    pub(crate) fn log() -> String {
        LOG.with_borrow(|log| log.join(","))
    }
}

/// Shows nothing; logs the label its view sees.
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct ShowLabelProps {
    /// What it logs the label under.
    #[prop(default)]
    pub(crate) name: Prop<SharedString>,
}

/// Logs `<name> <label>` when its view is made ([`ShowLabelProps`]).
#[component]
pub(crate) fn ShowLabel(props: ShowLabelProps) -> impl IntoElement {
    log(format!("{} {}", props.name.get_untracked(), label()));
    view()
}

/// A label for what it holds, made only while it places them.
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct LazyLabelProps {
    /// The label its view provides.
    #[prop(default)]
    pub(crate) label: Prop<SharedString>,
    /// Made once the view places them.
    #[prop(lazy)]
    pub(crate) children: Children,
}

/// Provides its `label`, then places its children ([`LazyLabelProps`]).
#[component]
pub(crate) fn LazyLabel(props: LazyLabelProps) -> impl IntoElement {
    provide_context(Label(props.label.get_untracked().to_string()));
    view().child(props.children)
}

/// A label for what it holds, made with it.
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct EagerLabelProps {
    /// The label its view provides.
    #[prop(default)]
    pub(crate) label: Prop<SharedString>,
    /// Made with it, before its view.
    pub(crate) children: Children,
}

/// Provides its `label`, then places its children ([`EagerLabelProps`]).
#[component]
pub(crate) fn EagerLabel(props: EagerLabelProps) -> impl IntoElement {
    provide_context(Label(props.label.get_untracked().to_string()));
    view().child(props.children)
}

fn set() -> Registry {
    let mut r = Registry::ui_set("test/call-owners");
    r.module::<Owners>()
        .register::<ShowLabel>()
        .register::<LazyLabel>()
        .register::<EagerLabel>();
    r
}

const APP: &str = r#"import { mount, signal, Text, View } from "nsh";
import { EagerLabel, LazyLabel, Owners, ShowLabel } from "test/call-owners";

const shown = signal("waiting");
const lines = [];
const open = signal(true);

function Inner() {
  lines.push(`inner ${Owners.read()}`);
  return <ShowLabel name="under inner" />;
}

function InLazy() {
  lines.push(`in lazy ${Owners.read()}`);
  return <ShowLabel name="under in lazy" />;
}

function InEager() {
  lines.push(`in eager ${Owners.read()}`);
  return <ShowLabel name="under in eager" />;
}

function Outer() {
  lines.push(`owner ${Owners.provide("outer")}`);
  Owners.onCleanup("outer");
  lines.push(`entered ${Owners.entered()}`);
  lines.push(`outer ${Owners.read()}`);
  return (
    <View>
      <Inner />
      <ShowLabel name="under outer" />
      <LazyLabel label="lazy"><InLazy /></LazyLabel>
      <EagerLabel label="eager"><InEager /></EagerLabel>
    </View>
  );
}

function Sibling() {
  lines.push(`sibling ${Owners.read()}`);
  return <ShowLabel name="under sibling" />;
}

setTimeout(() => {
  lines.push(`no owner ${Owners.provide("none")} ${Owners.read()}`);
  open.set(false);
  setTimeout(() => shown.set(`done ${lines.join("|")}|log ${Owners.log()}`), 50);
}, 50);

mount(() => (
  <View>
    <If when={open()}>
      <Outer />
    </If>
    <Sibling />
    <Text>{shown()}</Text>
  </View>
));
"#;

#[test]
fn a_call_runs_its_rust_code_under_the_owner_tied_to_its_js_owner() {
    let (_engine, rx) = start(APP, vec![set()]);
    let done = shown(&rx, "done");
    let (lines, logged) = done.split_once("|log ").expect("the log");
    assert_eq!(
        lines,
        "done owner true|entered false 2 true|outer outer|inner outer|in eager outer\
         |sibling none|in lazy lazy|no owner false none"
    );
    let mut logged: Vec<&str> = logged.split(',').collect();
    logged.sort_unstable();
    assert_eq!(
        logged,
        [
            "cleanup outer",
            "task ran",
            "under in eager eager",
            "under in lazy lazy",
            "under inner outer",
            "under outer outer",
            "under sibling none",
        ]
    );
}

const SIBLINGS: &str = r#"import { mount, signal, Text, View } from "nsh";
import { Owners } from "test/call-owners";

const shown = signal("waiting");
const lines = [];

function Provides(props) {
  Owners.provide(props.label);
  return <Text>{props.label}</Text>;
}

function Reads(props) {
  lines.push(`${props.name} ${Owners.read()}`);
  return <Text>{props.name}</Text>;
}

setTimeout(() => shown.set(`done ${lines.join("|")}`), 50);

mount(() => (
  <View>
    <If when={true}><Provides label="first" /></If>
    <If when={true}><Reads name="own owner" /></If>
    <If when={true}>
      <Provides label="shared" />
      <Reads name="same owner" />
    </If>
    <Text>{shown()}</Text>
  </View>
));
"#;

/// A tied owner is its JS owner's: components made under one JS owner (a
/// component's body has none of its own, as in Solid) share it, as they
/// share that owner's cleanups; one under an owner of its own (an `<If>`'s
/// branch, an `<Each>`'s row, a provider) does not see what another
/// provided.
#[test]
fn components_under_one_js_owner_share_its_tied_owner() {
    let (_engine, rx) = start(SIBLINGS, vec![set()]);
    assert_eq!(shown(&rx, "done"), "done own owner none|same owner shared");
}

const LAZY_FIRST: &str = r#"import { mount, signal, Text, View } from "nsh";
import { LazyLabel, Owners, ShowLabel } from "test/call-owners";

const shown = signal("waiting");
const lines = [];

function InLazy() {
  lines.push(`in lazy ${Owners.read()}`);
  return <ShowLabel name="first call in lazy" />;
}

setTimeout(() => shown.set(`done ${lines.join("|")}|log ${Owners.log()}`), 50);

mount(() => (
  <View>
    <LazyLabel label="lazy"><InLazy /></LazyLabel>
    <Text>{shown()}</Text>
  </View>
));
"#;

/// A Rust component's lazy children see its context even when the first
/// call that asks for an owner is made in them (no call ran `with_owner`
/// before the component was made).
#[test]
fn lazy_children_see_their_component_before_any_call_used_owners() {
    let (_engine, rx) = start(LAZY_FIRST, vec![set()]);
    assert_eq!(
        shown(&rx, "done"),
        "done in lazy lazy|log first call in lazy lazy"
    );
}
