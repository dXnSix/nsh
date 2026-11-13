//! What a TV adds to the arrows (`nsh_core::focus::directional`), over
//! views mounted as the engine mounts them, in GPUI's test app: the
//! preferred stop with nothing focused, `autoFocus` as a stop appears,
//! `nextFocus*` routes, focus groups that are entered at their
//! destinations and may trap the arrows, a scroll view that keeps the focus
//! while it scrolls on, and `Command::Focus` on a stop (a ref's `focus()`,
//! the row of a list the arrows reached). No engine runs.

use std::time::Duration;

use gpui::TestAppContext;
use nsh_core::{
    Command, ModalAnimation, ModalConfig, MountItem, NodeId, Points, ROOT_ID, ViewKind, ViewProps,
    focus::{GroupConfig, NextFocus},
};

use super::directional_focus::{PAGE, Tree, focused, key, surface};

fn preferred() -> ViewProps {
    ViewProps {
        preferred_focus: true,
        ..ViewProps::default()
    }
}

fn auto_focus() -> ViewProps {
    ViewProps {
        auto_focus: true,
        ..ViewProps::default()
    }
}

/// A focus group's view, trapping or not, entered at `destinations`.
fn group(trap: bool, destinations: &[NodeId]) -> ViewProps {
    let mut config = GroupConfig::default();
    config.trap = trap;
    config.destinations = destinations.to_vec();
    ViewProps {
        focus_group: Some(config),
        ..ViewProps::default()
    }
}

#[gpui::test]
fn with_nothing_focused_the_preferred_stop_takes_the_arrow(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0))
        .pressable(8, PAGE, (210.0, 10.0, 80.0, 40.0))
        .props(8, preferred());
    tree.apply(&surface);
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(8));
    // Then the arrows move as ever.
    assert!(key(&surface, "left"));
    assert_eq!(focused(&surface), Some(6));
}

#[gpui::test]
fn a_route_takes_the_arrow_past_geometry(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0))
        .pressable(10, PAGE, (10.0, 100.0, 80.0, 40.0));
    // A card that is no stop, holding one.
    tree.view(12, PAGE, ViewKind::View, (300.0, 250.0, 80.0, 40.0))
        .pressable(14, 12, (0.0, 0.0, 80.0, 40.0));
    let mut next = NextFocus::default();
    next.down = Some(12);
    tree.props(
        6,
        ViewProps {
            next_focus: next,
            ..ViewProps::default()
        },
    );
    tree.apply(&surface);
    assert!(key(&surface, "right"));
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(6));
    // Down goes to the stop inside the card named, not the nearest below.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(14));
    // Without routes, geometry: the nearest above, then its column.
    assert!(key(&surface, "up"));
    assert_eq!(focused(&surface), Some(10));
    assert!(key(&surface, "up"));
    assert_eq!(focused(&surface), Some(4));
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(10));
}

#[gpui::test]
fn a_trapping_group_keeps_the_arrows_inside(cx: &mut TestAppContext) {
    const GROUP: NodeId = 20;
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.view(GROUP, PAGE, ViewKind::View, (0.0, 0.0, 400.0, 60.0))
        .props(GROUP, group(true, &[]))
        .pressable(22, GROUP, (10.0, 10.0, 80.0, 40.0))
        .pressable(24, GROUP, (110.0, 10.0, 80.0, 40.0))
        .pressable(26, PAGE, (10.0, 100.0, 80.0, 40.0));
    tree.apply(&surface);
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(22));
    // Nothing below inside the group: the arrow goes nowhere.
    assert!(!key(&surface, "down"));
    assert_eq!(focused(&surface), Some(22));
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(24));
    // Open, it lets the arrow out.
    tree.props(GROUP, group(false, &[])).apply(&surface);
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(26));
}

#[gpui::test]
fn the_arrows_enter_a_group_at_its_destination(cx: &mut TestAppContext) {
    const GROUP: NodeId = 20;
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 100.0, 80.0, 40.0));
    tree.view(GROUP, PAGE, ViewKind::View, (0.0, 0.0, 400.0, 60.0))
        .props(GROUP, group(false, &[26]))
        .pressable(22, GROUP, (10.0, 10.0, 80.0, 40.0))
        .pressable(24, GROUP, (110.0, 10.0, 80.0, 40.0))
        .pressable(26, GROUP, (210.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(4));
    // Up reaches 22 by geometry; the group takes the focus to 26.
    assert!(key(&surface, "up"));
    assert_eq!(focused(&surface), Some(26));
    // Inside it, the nearest.
    assert!(key(&surface, "left"));
    assert_eq!(focused(&surface), Some(24));
    // Without destinations, its preferred stop.
    tree.props(GROUP, group(false, &[]))
        .props(22, preferred())
        .apply(&surface);
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(4));
    assert!(key(&surface, "up"));
    assert_eq!(focused(&surface), Some(22));
}

#[gpui::test]
fn auto_focus_takes_the_focus_when_the_stop_appears(cx: &mut TestAppContext) {
    const MODAL: NodeId = 30;
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    assert_eq!(focused(&surface), None);
    // Two come with autoFocus: the first in tree order takes it.
    tree.pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0))
        .props(6, auto_focus())
        .pressable(8, PAGE, (210.0, 10.0, 80.0, 40.0))
        .props(8, auto_focus());
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(6));
    // Once: their props sent again take nothing from the user.
    assert!(key(&surface, "left"));
    tree.props(8, auto_focus()).apply(&surface);
    assert_eq!(focused(&surface), Some(4));
    // A modal presented: its autoFocus stop takes the focus.
    tree.view(MODAL, ROOT_ID, ViewKind::Modal, (0.0, 0.0, 400.0, 300.0));
    tree.items.push(MountItem::ModalConfig {
        id: MODAL,
        config: ModalConfig {
            animation: ModalAnimation::None,
            ..ModalConfig::default()
        },
    });
    tree.pressable(32, MODAL, (10.0, 200.0, 80.0, 40.0))
        .pressable(34, MODAL, (110.0, 200.0, 80.0, 40.0))
        .props(34, auto_focus());
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(34));
}

#[gpui::test]
fn focus_on_a_stop_moves_the_focus_there(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    tree.items.push(MountItem::Command {
        id: 6,
        command: Command::Focus,
    });
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(6));
    // A view that is no stop takes none.
    tree.items.push(MountItem::Command {
        id: PAGE,
        command: Command::Focus,
    });
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(6));
}

#[gpui::test]
fn a_scroll_view_that_can_scroll_on_keeps_the_focus(cx: &mut TestAppContext) {
    const SCROLL: NodeId = 4;
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.view(SCROLL, PAGE, ViewKind::Scroll, (0.0, 0.0, 400.0, 200.0));
    tree.items.push(MountItem::ScrollConfig {
        id: SCROLL,
        horizontal: false,
        scrollbar_visible: false,
        replaced: false,
        scroll_value: None,
        pull_value: None,
        keyboard_dismiss: Default::default(),
        mirrored: false,
    });
    tree.items.push(MountItem::ScrollContent {
        id: SCROLL,
        size: nsh_core::Size::new(Points(400.0), Points(1000.0)),
    });
    // The rows mounted so far, and a button below the scroll view.
    tree.pressable(6, SCROLL, (10.0, 10.0, 200.0, 40.0))
        .pressable(8, SCROLL, (10.0, 120.0, 200.0, 40.0))
        .pressable(10, PAGE, (10.0, 250.0, 200.0, 40.0));
    tree.apply(&surface);
    let offset = || {
        surface
            .view(SCROLL)
            .unwrap()
            .state()
            .scroll
            .as_ref()
            .unwrap()
            .y
    };
    assert!(key(&surface, "down"));
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(8));
    // More content below: the view scrolls, the focus stays in it.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(8));
    std::thread::sleep(Duration::from_millis(400));
    surface.tick();
    assert!(offset().0 > 0.0, "scrolled to {:?}", offset());
    // At its end the arrow leaves it.
    surface
        .view(SCROLL)
        .unwrap()
        .state_mut()
        .scroll
        .as_mut()
        .unwrap()
        .y = Points(800.0);
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(10));
}

#[gpui::test]
fn an_arrow_past_a_list_s_mounted_rows_asks_the_engine_for_the_next(cx: &mut TestAppContext) {
    const LIST: NodeId = 4;
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.view(LIST, PAGE, ViewKind::Scroll, (0.0, 0.0, 400.0, 200.0));
    tree.items.push(MountItem::ScrollConfig {
        id: LIST,
        horizontal: false,
        scrollbar_visible: false,
        replaced: false,
        scroll_value: None,
        pull_value: None,
        keyboard_dismiss: Default::default(),
        mirrored: false,
    });
    tree.items.push(MountItem::ScrollContent {
        id: LIST,
        size: nsh_core::Size::new(Points(400.0), Points(10_000.0)),
    });
    // The rows mounted: cells with a stop each.
    tree.view(20, LIST, ViewKind::View, (0.0, 0.0, 400.0, 100.0))
        .pressable(6, 20, (10.0, 10.0, 200.0, 80.0));
    tree.view(22, LIST, ViewKind::View, (0.0, 100.0, 400.0, 100.0))
        .pressable(8, 22, (10.0, 10.0, 200.0, 80.0));
    tree.apply(&surface);
    assert!(key(&surface, "down"));
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(8));
    assert!(surface.asked_past.borrow().is_empty());
    // No row below is mounted: the engine hears the arrow from the stop
    // that keeps the focus.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(8));
    assert_eq!(
        std::mem::take(&mut *surface.asked_past.borrow_mut()),
        [(8, nsh_core::focus::Direction::Down)]
    );
    // The core mounts the next row and focuses its first stop.
    tree.view(24, LIST, ViewKind::View, (0.0, 200.0, 400.0, 100.0))
        .pressable(10, 24, (10.0, 10.0, 200.0, 80.0));
    tree.items.push(MountItem::Command {
        id: 10,
        command: Command::Focus,
    });
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(10));
}
