//! The arrows, Enter and Space over views mounted as the engine mounts
//! them, in GPUI's test app (its timers and the scroll views' frames),
//! keys given by the names GPUI's keystrokes carry, as the desktops' and
//! the TV's arrows come. No engine runs: the events the surface sends are
//! read from [`Surface::heard`].

use std::{rc::Rc, time::Duration};

use gpui::TestAppContext;
use nsh_core::{
    AccessibilityRole, Command, EventKind, EventMask, InputConfig, ModalAnimation, ModalConfig,
    MountBatch, MountItem, NodeId, Points, ROOT_ID, ViewKind, ViewProps,
    overlay::{PlacedAnchor, Placement},
    tailwind::state::FOCUSED,
};

use crate::{geom::rect, input::Modifiers, surface::Surface};

/// The page, under the surface's root, as large as the window.
pub(super) const PAGE: NodeId = 2;

/// A surface 400 x 300 points with its page, in the test app.
pub(super) fn surface(cx: &mut TestAppContext) -> (Rc<Surface>, impl Drop) {
    let app = cx.update(crate::rt::init_test_app);
    let surface = Surface::new(Default::default());
    surface.resized(400.0, 300.0, 1.0);
    let mut tree = Tree::default();
    tree.view(PAGE, ROOT_ID, ViewKind::View, (0.0, 0.0, 400.0, 300.0));
    tree.apply(&surface);
    (surface, app)
}

/// Mount items, applied as one batch.
#[derive(Default)]
pub(super) struct Tree {
    pub(super) items: Vec<MountItem>,
}

impl Tree {
    /// A view of `kind` at `frame` in `parent`.
    pub(super) fn view(
        &mut self,
        id: NodeId,
        parent: NodeId,
        kind: ViewKind,
        (x, y, w, h): (f32, f32, f32, f32),
    ) -> &mut Self {
        self.items.push(MountItem::Create { id, kind });
        self.items.push(MountItem::Frame {
            id,
            frame: rect(x, y, w, h),
        });
        self.items.push(MountItem::Insert {
            parent,
            child: id,
            before: None,
        });
        self
    }

    /// A view that takes presses and hears its focus, with `focus:`
    /// styles (it tracks `FOCUSED`).
    pub(super) fn pressable(
        &mut self,
        id: NodeId,
        parent: NodeId,
        frame: (f32, f32, f32, f32),
    ) -> &mut Self {
        self.view(id, parent, ViewKind::View, frame);
        self.events(
            id,
            &[
                EventKind::PressDown,
                EventKind::PressUp,
                EventKind::Press,
                EventKind::Focus,
                EventKind::Blur,
            ],
        );
        self.items.push(MountItem::TrackStates {
            id,
            states: FOCUSED,
        });
        self
    }

    pub(super) fn events(&mut self, id: NodeId, kinds: &[EventKind]) -> &mut Self {
        let mut events = EventMask::default();
        for &kind in kinds {
            events.set(kind, true);
        }
        self.items.push(MountItem::Events { id, events });
        self
    }

    pub(super) fn props(&mut self, id: NodeId, props: ViewProps) -> &mut Self {
        self.items.push(MountItem::ViewProps {
            id,
            props: Box::new(props),
        });
        self
    }

    pub(super) fn apply(&mut self, surface: &Rc<Surface>) {
        let items = std::mem::take(&mut self.items);
        surface.apply_batch(&MountBatch { items });
    }
}

/// Presses the key GPUI names `name` ("down", "enter", ...), as the
/// surface's key listener gives it on (`crate::events`). Whether it was
/// taken.
pub(super) fn key(surface: &Rc<Surface>, name: &str) -> bool {
    let vk = crate::keys::vk_of(name).expect("a key GPUI names");
    crate::input::key(
        surface,
        crate::input::KeyEvent {
            vk,
            down: true,
            repeat: false,
            modifiers: Modifiers::default(),
        },
    )
}

/// The node with the keyboard focus: a field's or a view's.
pub(super) fn focused(surface: &Surface) -> Option<NodeId> {
    crate::focus::current(surface).map(|view| view.id())
}

/// The events sent since the last call.
pub(super) fn heard(surface: &Surface) -> Vec<(NodeId, EventKind)> {
    std::mem::take(&mut *surface.heard.borrow_mut())
}

pub(super) fn has_focused_state(surface: &Surface, id: NodeId) -> bool {
    surface.view(id).unwrap().state().states & FOCUSED != 0
}

#[gpui::test]
fn left_and_right_move_along_a_row(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0))
        .pressable(8, PAGE, (210.0, 10.0, 80.0, 40.0))
        // A row below: Right never goes down to it.
        .pressable(10, PAGE, (10.0, 100.0, 380.0, 40.0));
    tree.apply(&surface);
    // Nothing focused: the first stop.
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(4));
    assert!(key(&surface, "right"));
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(8));
    // The row's end: the key goes on (to the scroll views), the focus stays.
    assert!(!key(&surface, "right"));
    assert_eq!(focused(&surface), Some(8));
    assert!(key(&surface, "left"));
    assert_eq!(focused(&surface), Some(6));
}

#[gpui::test]
fn up_and_down_move_along_a_column(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0))
        .pressable(8, PAGE, (10.0, 100.0, 80.0, 40.0))
        .pressable(10, PAGE, (110.0, 100.0, 80.0, 40.0))
        .pressable(12, PAGE, (10.0, 200.0, 300.0, 40.0));
    tree.apply(&surface);
    // Nothing focused: the first in sight from the top.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(4));
    // The one in the column below, not the nearer row's neighbor.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(8));
    assert!(key(&surface, "right"));
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(12));
    // Both above overlap its column: the nearer by left edge.
    assert!(key(&surface, "up"));
    assert_eq!(focused(&surface), Some(8));
    assert!(key(&surface, "up"));
    assert_eq!(focused(&surface), Some(4));
    // Nothing above: the key goes on.
    assert!(!key(&surface, "up"));
    assert_eq!(focused(&surface), Some(4));
}

#[gpui::test]
fn enter_and_space_press_the_focused_view(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0));
    // A stop that takes no presses (a chart, a slider): its keys are the
    // app's bindings'.
    tree.view(6, PAGE, ViewKind::View, (110.0, 10.0, 80.0, 40.0))
        .events(6, &[EventKind::Focus, EventKind::Blur])
        .props(
            6,
            ViewProps {
                focusable: Some(true),
                ..ViewProps::default()
            },
        );
    tree.apply(&surface);
    // Nothing focused: nothing pressed.
    assert!(!key(&surface, "enter"));
    assert!(key(&surface, "right"));
    heard(&surface);
    for name in ["enter", "space"] {
        assert!(key(&surface, name), "{name}");
        assert_eq!(
            heard(&surface),
            [
                (4, EventKind::PressDown),
                (4, EventKind::PressUp),
                (4, EventKind::Press)
            ],
            "{name}"
        );
    }
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(6));
    heard(&surface);
    assert!(!key(&surface, "enter"));
    assert_eq!(heard(&surface), []);
}

#[gpui::test]
fn the_focused_view_is_revealed_in_its_scroll_view(cx: &mut TestAppContext) {
    const SCROLL: NodeId = 4;
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.view(SCROLL, PAGE, ViewKind::Scroll, (0.0, 0.0, 400.0, 300.0));
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
    tree.pressable(6, SCROLL, (10.0, 10.0, 200.0, 40.0))
        .pressable(8, SCROLL, (10.0, 400.0, 200.0, 40.0))
        .pressable(10, SCROLL, (10.0, 800.0, 200.0, 40.0));
    tree.apply(&surface);
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(6));
    // Out of sight below: it takes the focus, and the view scrolls to it.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(8));
    std::thread::sleep(Duration::from_millis(400));
    surface.tick();
    let offset = surface
        .view(SCROLL)
        .unwrap()
        .state()
        .scroll
        .as_ref()
        .unwrap()
        .y;
    assert!(
        offset.0 >= 440.0 - 300.0 && offset.0 <= 400.0,
        "the row at 400..440 shows at offset {offset:?}"
    );
}

#[gpui::test]
fn focus_and_blur_are_reported_with_the_focused_state(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    heard(&surface);
    assert!(key(&surface, "right"));
    assert_eq!(heard(&surface), [(4, EventKind::Focus)]);
    assert!(has_focused_state(&surface, 4));
    // The one left hears its blur first.
    assert!(key(&surface, "right"));
    assert_eq!(
        heard(&surface),
        [(4, EventKind::Blur), (6, EventKind::Focus)]
    );
    assert!(!has_focused_state(&surface, 4));
    assert!(has_focused_state(&surface, 6));
    // A disabled stop loses it.
    let mut props = ViewProps::default();
    props.accessibility.state.disabled = true;
    tree.props(6, props).apply(&surface);
    assert_eq!(focused(&surface), None);
    assert_eq!(heard(&surface), [(6, EventKind::Blur)]);
    assert!(!has_focused_state(&surface, 6));
}

#[gpui::test]
fn fields_keep_the_arrows_they_use(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 200.0, 40.0));
    tree.view(6, PAGE, ViewKind::Input, (10.0, 80.0, 200.0, 30.0))
        .events(6, &[EventKind::Focus, EventKind::Blur]);
    tree.items.push(MountItem::Input {
        id: 6,
        config: Box::new(InputConfig::default()),
    });
    tree.pressable(8, PAGE, (10.0, 150.0, 200.0, 40.0))
        .pressable(10, PAGE, (250.0, 80.0, 100.0, 30.0));
    tree.apply(&surface);
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(4));
    heard(&surface);
    // Into the field: it takes the typing, the view hears its blur.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(6));
    assert_eq!(
        crate::text_input::focused(&surface).map(|v| v.id()),
        Some(6)
    );
    assert_eq!(crate::focus::focused(&surface), None);
    assert_eq!(
        heard(&surface)
            .into_iter()
            .filter(|(_, kind)| matches!(kind, EventKind::Focus | EventKind::Blur))
            .collect::<Vec<_>>(),
        [(4, EventKind::Blur), (6, EventKind::Focus)]
    );
    // Left and Right move its caret; the view beside it stays unfocused.
    assert!(key(&surface, "right"));
    assert!(key(&surface, "left"));
    assert_eq!(focused(&surface), Some(6));
    // A single line has no use for Down: it leaves the field.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(8));
    assert_eq!(crate::text_input::focused(&surface), None);
}

#[gpui::test]
fn a_modal_keeps_the_focus_and_gives_it_back(cx: &mut TestAppContext) {
    const MODAL: NodeId = 20;
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(4));
    tree.view(MODAL, ROOT_ID, ViewKind::Modal, (0.0, 0.0, 400.0, 300.0));
    tree.items.push(MountItem::ModalConfig {
        id: MODAL,
        config: ModalConfig {
            animation: ModalAnimation::None,
            ..ModalConfig::default()
        },
    });
    tree.pressable(22, MODAL, (10.0, 200.0, 80.0, 40.0))
        .pressable(24, MODAL, (110.0, 200.0, 80.0, 40.0));
    tree.apply(&surface);
    // Presented: the page's view leaves the focus until it closes.
    assert_eq!(focused(&surface), None);
    assert!(!has_focused_state(&surface, 4));
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(22));
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(24));
    // The page's views above are out of reach.
    assert!(!key(&surface, "up"));
    assert_eq!(focused(&surface), Some(24));
    tree.items.push(MountItem::Remove { id: MODAL });
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(4));
    assert!(has_focused_state(&surface, 4));
}

/// A trigger and a menu's popup anchored to it, with a view of the page
/// beside the popup's first item.
fn menu(surface: &Rc<Surface>) -> Tree {
    const POPUP: NodeId = 30;
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (300.0, 60.0, 80.0, 30.0));
    tree.view(POPUP, ROOT_ID, ViewKind::View, (10.0, 60.0, 200.0, 80.0))
        .props(
            POPUP,
            ViewProps {
                overlay_anchor: Some(PlacedAnchor {
                    bounds: rect(10.0, 10.0, 80.0, 40.0),
                    node: Some(4),
                    placement: Placement::Bottom,
                }),
                ..ViewProps::default()
            },
        );
    let mut item = ViewProps::default();
    item.accessibility.role = AccessibilityRole::Menuitem;
    tree.pressable(32, POPUP, (0.0, 0.0, 200.0, 30.0))
        .props(32, item.clone())
        .pressable(34, POPUP, (0.0, 40.0, 200.0, 30.0))
        .props(34, item);
    tree.apply(surface);
    tree
}

#[gpui::test]
fn a_menu_s_popup_takes_the_arrows_first_and_keeps_left_and_right(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    menu(&surface);
    // Nothing focused: Down enters the open popup, not the page above it.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(32));
    // Right on an item stays in the menu (the page's view on its row is
    // not reached).
    assert!(!key(&surface, "right"));
    assert_eq!(focused(&surface), Some(32));
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(34));
}

#[gpui::test]
fn a_popup_that_goes_gives_the_focus_back(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = menu(&surface);
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(4));
    // From the trigger into its popup, below it.
    assert!(key(&surface, "down"));
    assert_eq!(focused(&surface), Some(32));
    tree.items.push(MountItem::Remove { id: 30 });
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(4));
}

#[gpui::test]
fn a_backdrop_takes_the_page_out_of_reach(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(4));
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
    // Covered: the page's view loses the focus, and arrows stay above the
    // backdrop.
    assert_eq!(focused(&surface), None);
    assert!(key(&surface, "up"));
    assert_eq!(focused(&surface), Some(44));
    assert!(!key(&surface, "up"));
    // The sheet goes: the covered view takes the focus back.
    tree.items.push(MountItem::Remove { id: 40 });
    tree.items.push(MountItem::Remove { id: 42 });
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(4));
}

#[gpui::test]
fn blur_takes_the_focus_off_the_focused_view(cx: &mut TestAppContext) {
    let (surface, _app) = surface(cx);
    let mut tree = Tree::default();
    tree.pressable(4, PAGE, (10.0, 10.0, 80.0, 40.0))
        .pressable(6, PAGE, (110.0, 10.0, 80.0, 40.0));
    tree.apply(&surface);
    assert!(key(&surface, "right"));
    assert_eq!(focused(&surface), Some(4));
    heard(&surface);
    // `blur()` on another view leaves it.
    tree.items.push(MountItem::Command {
        id: 6,
        command: Command::Blur,
    });
    tree.apply(&surface);
    assert_eq!(focused(&surface), Some(4));
    assert_eq!(heard(&surface), []);
    tree.items.push(MountItem::Command {
        id: 4,
        command: Command::Blur,
    });
    tree.apply(&surface);
    assert_eq!(focused(&surface), None);
    assert!(!has_focused_state(&surface, 4));
    assert_eq!(heard(&surface), [(4, EventKind::Blur)]);
    // Enter presses nothing now.
    assert!(!key(&surface, "enter"));
    assert_eq!(heard(&surface), []);
}
