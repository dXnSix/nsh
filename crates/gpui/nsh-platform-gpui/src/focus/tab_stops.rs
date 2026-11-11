//! Tab and Shift + Tab over views mounted as the engine mounts them: they
//! stop at the arrows' stops (pressables and `focusable` views as well as
//! fields, less those with no area or under a backdrop), in tree order
//! through `nsh_core::input::tab_stop`, within the newest modal; the stop
//! the focus leaves the view from lets go of it.

use gpui::TestAppContext;
use nsh_core::{EventKind, InputConfig, MountItem, ROOT_ID, ViewKind, ViewProps};

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

#[gpui::test]
fn the_stop_tab_leaves_the_view_from_lets_go(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    assert!(tab(&surface, false));
    assert!(tab(&surface, false));
    assert!(!tab(&surface, false));
    heard(&surface);
    // The window has another tab stop: the focus leaves the view (as
    // `AppView::tab` does past the last stop).
    crate::input::tab_past_ends(&surface, false, || true);
    assert_eq!(focused(&surface), None);
    assert!(!has_focused_state(&surface, 6));
    assert_eq!(heard(&surface), [(6, EventKind::Blur)]);
    // None: around to the first stop, and the last lets go.
    assert!(tab(&surface, true));
    assert_eq!(focused(&surface), Some(6));
    heard(&surface);
    crate::input::tab_past_ends(&surface, false, || false);
    assert_eq!(focused(&surface), Some(4));
    assert!(!has_focused_state(&surface, 6));
    assert_eq!(
        heard(&surface),
        [(6, EventKind::Blur), (4, EventKind::Focus)]
    );
    // Shift + Tab out of the view from the first stop.
    crate::input::tab_past_ends(&surface, true, || true);
    assert_eq!(focused(&surface), None);
    assert_eq!(heard(&surface), [(4, EventKind::Blur)]);
}

#[gpui::test]
fn tab_skips_the_stops_under_a_backdrop(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0));
    // A sheet: a backdrop that closes it on a press, a panel over it.
    tree.view(40, ROOT_ID, ViewKind::View, (0.0, 0.0, 400.0, 300.0))
        .events(40, &[EventKind::Press])
        .props(
            40,
            ViewProps {
                focusable: Some(false),
                ..ViewProps::default()
            },
        );
    tree.view(42, ROOT_ID, ViewKind::View, (0.0, 200.0, 400.0, 100.0))
        .pressable(44, 42, (10.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    heard(&surface);
    assert!(tab(&surface, false));
    assert_eq!(
        focused(&surface),
        Some(44),
        "the page's pressable is covered"
    );
    assert!(!tab(&surface, false));
    assert!(!tab(&surface, true));
    assert!(!heard(&surface).contains(&(4, EventKind::Focus)));
    // Past the ends Tab wraps around to the sheet's stop, not the page's.
    crate::input::tab_past_ends(&surface, false, || false);
    assert_eq!(focused(&surface), Some(44));
}

#[gpui::test]
fn tab_skips_views_with_no_area(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 0.0, 0.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0))
        .pressable(8, PAGE, (210.0, 10.0, 80.0, 0.0));
    tree.apply(&surface);
    assert!(tab(&surface, false));
    assert_eq!(focused(&surface), Some(6));
    assert!(!tab(&surface, false));
    assert!(!tab(&surface, true));
    assert_eq!(focused(&surface), Some(6));
}
