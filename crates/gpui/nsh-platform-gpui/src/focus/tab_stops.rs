//! Tab and Shift + Tab over views mounted as the engine mounts them: they
//! stop at pressables and `focusable` views as well as fields, in tree
//! order through `nsh_core::input::tab_stop`, within the newest modal.

use gpui::TestAppContext;
use nsh_core::{EventKind, InputConfig, MountItem, ViewKind, ViewProps};

use super::directional_focus::{PAGE, Tree, focused, has_focused_state, heard, key, surface};
use crate::input::Modifiers;

/// Presses Tab (Shift + Tab with `shift`). Whether it was taken.
fn tab(surface: &std::rc::Rc<crate::surface::Surface>, shift: bool) -> bool {
    crate::input::key(
        surface,
        crate::input::KeyEvent {
            vk: crate::keys::vk_of("tab").expect("GPUI names Tab"),
            down: true,
            repeat: false,
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
        },
    )
}

#[gpui::test]
fn tab_stops_at_pressables_and_fields_in_tree_order(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    // Tree order is not the order on screen: 6 is mounted before 4 but sits
    // to its right.
    tree.pressable(6, PAGE, (210.0, 10.0, 80.0, 40.0))
        .pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0));
    tree.view(8, PAGE, ViewKind::Input, (10.0, 80.0, 200.0, 30.0))
        .events(8, &[EventKind::Focus, EventKind::Blur]);
    tree.items.push(MountItem::Input {
        id: 8,
        config: Box::new(InputConfig::default()),
    });
    // Not a stop: no presses, not focusable.
    tree.view(10, PAGE, ViewKind::View, (10.0, 150.0, 200.0, 40.0));
    tree.apply(&surface);

    assert!(tab(&surface, false));
    assert_eq!(focused(&surface), Some(6));
    assert!(has_focused_state(&surface, 6));
    assert!(tab(&surface, false));
    assert_eq!(focused(&surface), Some(4));
    assert!(!has_focused_state(&surface, 6));
    assert!(tab(&surface, false));
    assert_eq!(focused(&surface), Some(8));
    assert_eq!(crate::focus::focused(&surface), None, "the field has it");
    // Past the last stop the key is not taken: the window's focus moves on.
    assert!(!tab(&surface, false));
    assert_eq!(focused(&surface), Some(8));
    assert!(tab(&surface, true));
    assert_eq!(focused(&surface), Some(4));
    assert!(tab(&surface, true));
    assert_eq!(focused(&surface), Some(6));
    assert!(!tab(&surface, true));
}

#[gpui::test]
fn tab_reports_focus_and_blur_and_enter_presses(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    heard(&surface);
    assert!(tab(&surface, false));
    assert_eq!(heard(&surface), [(4, EventKind::Focus)]);
    assert!(tab(&surface, false));
    assert_eq!(
        heard(&surface),
        [(4, EventKind::Blur), (6, EventKind::Focus)]
    );
    assert!(key(&surface, "enter"));
    assert!(
        heard(&surface).contains(&(6, EventKind::Press)),
        "Enter presses the view Tab reached"
    );
}

#[gpui::test]
fn focusable_views_are_stops_and_disabled_ones_are_not(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    let mut disabled = ViewProps::default();
    disabled.accessibility.state.disabled = true;
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .props(4, disabled);
    let focusable = ViewProps {
        focusable: Some(true),
        ..ViewProps::default()
    };
    tree.view(6, PAGE, ViewKind::View, (110.0, 10.0, 80.0, 40.0))
        .props(6, focusable);
    tree.apply(&surface);
    assert!(tab(&surface, false));
    assert_eq!(focused(&surface), Some(6));
    assert!(!tab(&surface, false), "the disabled pressable is skipped");
}
