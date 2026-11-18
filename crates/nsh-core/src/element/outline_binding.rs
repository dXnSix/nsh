//! `Styled::outline` takes a binding, as `Styled::ring` does: an outline
//! whose width reads a theme token or a signal is drawn again at the new
//! width when it changes, and stays solid.

use super::theme_format_store::bare_core;
use crate::{
    BorderStyle, Core, MountBatch, MountItem,
    core::tests::FakeEngine,
    effects::Outline,
    element::{Patch, PatchValues, Root, TokenKey},
    prelude::*,
};

const DOCUMENT: &str = r##"{
  "tokens": {
    "border-width-focus-ring": { "kind": "number", "value": 2 }
  },
  "modifiers": {
    "density": { "default": "desktop",
      "contexts": { "desktop": {}, "touch": { "border-width-focus-ring": 4 } } }
  },
  "resolutionOrder": ["density"]
}"##;

const FOCUS_RING: TokenKey<Points> = TokenKey::new("border-width-focus-ring");

fn themed() -> Core<FakeEngine> {
    let mut c = bare_core();
    assert!(c.set_theme(Some(DOCUMENT)).is_empty());
    c.install_theme();
    c.commit();
    c
}

/// The last outline a batch sets.
fn last_outline(batch: &MountBatch) -> Option<Outline> {
    batch.items.iter().rev().find_map(|i| match i {
        MountItem::ViewStyle { style, .. } if style.outline != Outline::NONE => Some(style.outline),
        _ => None,
    })
}

#[component]
fn TokenOutline() -> impl IntoElement {
    view()
        .size_10()
        .outline(move || FOCUS_RING.get().unwrap_or(px(1.0)))
}

#[test]
fn an_outline_bound_to_a_token_takes_its_width() {
    let mut c = themed();
    c.mount_root(&Root::of::<TokenOutline>());
    let outline = last_outline(&c.commit().unwrap()).unwrap();
    assert_eq!(outline.width, px(2.0));
    assert_eq!(outline.style, BorderStyle::Solid);
}

#[test]
fn a_patch_of_the_token_redraws_the_outline_at_its_new_width() {
    let mut c = themed();
    c.mount_root(&Root::of::<TokenOutline>());
    c.commit();
    c.update_tokens(Patch::new().key(FOCUS_RING, px(3.0)));
    c.install_theme();
    let outline = last_outline(&c.commit().unwrap()).unwrap();
    assert_eq!(outline.width, px(3.0));
    assert_eq!(outline.style, BorderStyle::Solid);
}

#[test]
fn a_context_that_sets_the_token_redraws_the_outline() {
    let mut c = themed();
    c.mount_root(&Root::of::<TokenOutline>());
    c.commit();
    c.set_modifier("density", "touch");
    assert!(c.install_theme());
    assert_eq!(last_outline(&c.commit().unwrap()).unwrap().width, px(4.0));
}

thread_local! {
    static WIDTH: std::cell::Cell<Option<Signal<AbsoluteLength>>> = const { std::cell::Cell::new(None) };
}

#[component]
fn SignalOutline() -> impl IntoElement {
    let width = signal(AbsoluteLength::from(px(1.0)));
    WIDTH.with(|w| w.set(Some(width)));
    view().size_10().outline(width)
}

#[test]
fn an_outline_bound_to_a_signal_follows_it() {
    let mut c = themed();
    c.mount_root(&Root::of::<SignalOutline>());
    assert_eq!(last_outline(&c.commit().unwrap()).unwrap().width, px(1.0));
    let width = WIDTH.with(|w| w.get()).unwrap();
    {
        let _entered = c.test_runtime().enter();
        width.set(px(5.0).into());
    }
    assert_eq!(last_outline(&c.commit().unwrap()).unwrap().width, px(5.0));
}

#[component]
fn FixedOutline() -> impl IntoElement {
    view().size_10().outline(px(2.0))
}

#[test]
fn a_fixed_outline_is_set_once_as_before() {
    let mut c = themed();
    c.mount_root(&Root::of::<FixedOutline>());
    let outline = last_outline(&c.commit().unwrap()).unwrap();
    assert_eq!(
        (outline.width, outline.style),
        (px(2.0), BorderStyle::Solid)
    );
}
