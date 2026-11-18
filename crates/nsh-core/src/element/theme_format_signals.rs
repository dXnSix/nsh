//! Per-name signals (CORE-SIGNALS, B8): outside a `ThemeScope`, a Rust
//! reader of a token reruns only when that token changes, through
//! `token`, `TokenKey::get` and `role`, and a reader of the whole store
//! whenever any token does.

use std::{cell::Cell, rc::Rc};

use super::theme_format_store::bare_core;
use crate::{
    Core,
    core::tests::FakeEngine,
    element::{Patch, PatchValues, TokenKey, create_root, reactive::render_effect, role, token},
};

const DOCUMENT: &str = r##"{
  "tokens": {
    "a": { "kind": "number", "value": 1 },
    "b": { "kind": "number", "value": 2 },
    "surface": { "kind": "color", "value": "#ffffff" },
    "overlay": { "kind": "color", "value": "#eeeeee" }
  },
  "modifiers": {
    "m": { "default": "x", "contexts": { "x": {}, "y": { "a": 10, "surface": "#000000" } } }
  },
  "resolutionOrder": ["m"],
  "roles": { "popover": "surface" }
}"##;

fn themed() -> Core<FakeEngine> {
    let mut c = bare_core();
    assert!(c.set_theme(Some(DOCUMENT)).is_empty());
    c.install_theme();
    c
}

/// How many times a binding reading with `read` ran: once now, and again
/// whenever what it read changes.
fn runs(c: &Core<FakeEngine>, read: impl Fn() + 'static) -> Rc<Cell<u32>> {
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let _entered = c.test_runtime().enter();
    let _ = create_root(move || {
        render_effect(move || {
            read();
            counted.set(counted.get() + 1);
        });
    });
    runs
}

fn pick(c: &mut Core<FakeEngine>, context: &str) {
    c.set_modifier("m", context);
    assert!(c.install_theme());
}

/// B8: a modifier switch that changes token `a` but not `b` reruns a reader
/// of `a` once and a reader of `b` never.
#[test]
fn b8_a_reader_of_token_reruns_only_when_its_token_changes() {
    let mut c = themed();
    let a = runs(&c, || {
        let _ = token("a");
    });
    let b = runs(&c, || {
        let _ = token("b");
    });
    assert_eq!((a.get(), b.get()), (1, 1));
    pick(&mut c, "y");
    assert_eq!((a.get(), b.get()), (2, 1));
    pick(&mut c, "x");
    assert_eq!((a.get(), b.get()), (3, 1));
    // An override of `b` alone.
    c.update_tokens(Patch::new().value("b", 5.0));
    c.install_theme();
    assert_eq!((a.get(), b.get()), (3, 2));
}

#[test]
fn a_typed_key_reader_reruns_only_when_its_token_changes() {
    const A: TokenKey<f32> = TokenKey::new("a");
    const B: TokenKey<f32> = TokenKey::new("b");
    let mut c = themed();
    let seen = Rc::new(Cell::new(0.0));
    let shown = seen.clone();
    let a = runs(&c, move || shown.set(A.get().unwrap_or_default()));
    let b = runs(&c, || {
        let _ = B.get();
    });
    pick(&mut c, "y");
    assert_eq!((a.get(), b.get()), (2, 1));
    assert_eq!(seen.get(), 10.0);
}

#[test]
fn a_role_reader_reruns_when_its_tokens_value_or_its_mapping_changes() {
    let mut c = themed();
    let popover = runs(&c, || {
        let _ = role(role::POPOVER);
    });
    let b = runs(&c, || {
        let _ = token("b");
    });
    // The token it reads changes.
    pick(&mut c, "y");
    assert_eq!((popover.get(), b.get()), (2, 1));
    // Its mapping changes.
    c.update_tokens(Patch::new().role("popover", "overlay"));
    c.install_theme();
    assert_eq!((popover.get(), b.get()), (3, 1));
    // The token it read before changes: it reads `overlay` now.
    pick(&mut c, "x");
    assert_eq!((popover.get(), b.get()), (3, 1));
    c.update_tokens(Patch::new().value("overlay", crate::Hsla::from(crate::color::rgb(0x123456))));
    c.install_theme();
    assert_eq!((popover.get(), b.get()), (4, 1));
}

#[test]
fn a_reader_of_the_whole_store_reruns_on_any_change() {
    let mut c = themed();
    let all = runs(&c, || {
        let _ = crate::element::tokens().number("b");
    });
    let b = runs(&c, || {
        let _ = token("b");
    });
    pick(&mut c, "y");
    assert_eq!((all.get(), b.get()), (2, 1));
}
