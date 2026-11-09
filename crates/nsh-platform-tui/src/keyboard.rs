//! Keyboard focus uses the same projected geometry as drawing and pointer input.
//!
//! The decisions are `nsh_core::focus`'s, over the Cells of the last frame
//! in points (`points`): the stops of the focus scope, the arrows' moves,
//! the layers a backdrop takes out of reach, reveal deltas and the focus a
//! modal or a layer gives back. This module applies them to the mount.

use nsh_core::{
    Bounds, EventKind, EventPayload, NodeId, PayloadValue, Point, Points, ROOT_ID, Size, ViewKind,
    focus::{self, Align, Candidate, Layer, Order},
    input::{has_press, next_states},
    keymap::{KeyMatch, Keystroke},
    tailwind::state::{FOCUS_WITHIN, FOCUSED},
};
use nsh_engine::EngineControl;
use ratatui_core::layout::Position;

use crate::{
    CellSize, Handled, geometry::CellBounds, key_scheme::Action, mount::Mounter, render::Geometry,
};

/// A link of a text that is a stop of Tab (`TextLink::focus_stop`): no
/// view of its own, it stands after its text `view` in the focus order,
/// `index` among the text's links in text order, and takes Return as a
/// click on its characters does.
#[derive(Clone, Debug)]
pub(crate) struct Span {
    pub(crate) view: NodeId,
    pub(crate) index: usize,
    pub(crate) events: nsh_core::EventMask,
    pub(crate) range: std::ops::Range<u32>,
}

/// The view `id` is, or the text view of link `id`.
pub(crate) fn host(tree: &Mounter, id: NodeId) -> NodeId {
    tree.spans.get(&id).map_or(id, |span| span.view)
}

/// The view around `id`: its parent, or a link's text view.
fn parent(tree: &Mounter, id: NodeId) -> Option<NodeId> {
    match tree.get(id) {
        Some(view) => view.parent,
        None => tree.spans.get(&id).map(|span| span.view),
    }
}

/// Where stop `id` comes in the focus order: its view's place in the tree,
/// a link right after its text, in text order.
fn order(tree: &Mounter, id: NodeId) -> Order {
    match tree.spans.get(&id) {
        Some(span) => Order::link(
            tree.get(span.view).map_or(usize::MAX, |v| v.tree_order),
            span.index,
        ),
        None => Order::view(tree.get(id).map_or(usize::MAX, |v| v.tree_order)),
    }
}

/// A stop of the focus scope: the core's candidate, and the Cells of the
/// last frame it was made from.
#[derive(Clone, Copy)]
pub(crate) struct Stop<'a> {
    node: Candidate,
    geometry: &'a Geometry,
}

impl AsRef<Candidate> for Stop<'_> {
    fn as_ref(&self) -> &Candidate {
        &self.node
    }
}

impl std::ops::Deref for Stop<'_> {
    type Target = Geometry;

    fn deref(&self) -> &Geometry {
        self.geometry
    }
}

/// Cells `bounds` in the surface's points, by the size of a Cell: each
/// edge is its Cell's, so the frame's right and bottom edges (its origin
/// plus its size, as the core finds them) are the next Cell's left and
/// top, and frames touch, overlap and come in order as their Cells do.
fn points(bounds: CellBounds, cells: CellSize) -> Bounds<Points> {
    let (x, width) = span(bounds.left, bounds.right, cells.width());
    let (y, height) = span(bounds.top, bounds.bottom, cells.height());
    Bounds::new(Point::new(x, y), Size::new(width, height))
}

/// The start of Cells `start` .. `end`, each `cell` long, and the length
/// that ends them exactly at `end`'s start.
fn span(start: i32, end: i32, cell: Points) -> (Points, Points) {
    let (start, end) = (cell.0 * start as f32, cell.0 * end as f32);
    let mut len = end - start;
    // The sum rounds: a length a step longer or shorter ends it there.
    for _ in 0..4 {
        let sum = start + len;
        if sum == end {
            break;
        }
        len = if sum < end {
            len.next_up()
        } else {
            len.next_down()
        };
    }
    (Points(start), Points(len))
}

/// The core's candidate for `g` when it can be a stop of `scope`: what
/// takes walks of the tree (its layer, a popup around it) only then.
fn candidate<'a>(
    cells: CellSize,
    tree: &Mounter,
    g: &'a Geometry,
    scope: NodeId,
) -> Option<Stop<'a>> {
    let mut node = Candidate::default();
    node.id = g.id;
    if let Some(span) = tree.spans.get(&g.id) {
        // A text's link: while its text is a part of the scope.
        node.scope = input_scope(tree, span.view);
        node.stop = true;
    } else {
        let view = tree.get(g.id)?;
        node.scope = input_scope(tree, g.id);
        // A field narrower than a Cell (an editor's sink at its caret, 1
        // pt wide) is laid out without columns, yet takes the typing as on
        // the other platforms; a control without Cells is not there.
        node.editable = view.input.as_ref().is_some_and(|config| config.editable);
        node.stop = !view.props.accessibility.state.disabled
            && view
                .props
                .focus_stop(has_press(view.events) || node.editable);
    }
    if node.scope != scope || !node.stop {
        return None;
    }
    node.frame = points(g.bounds, cells);
    node.visible = !g.visible.is_empty();
    node.order = order(tree, g.id);
    node.layer = layer(tree, g.id);
    node.popup = in_anchored_overlay(tree, g.id);
    Some(Stop { node, geometry: g })
}

/// The children of `scope`'s view (the surface, or a modal) as the core's
/// layers, the page first.
fn layers(cells: CellSize, tree: &Mounter, geometry: &[Geometry], scope: NodeId) -> Vec<Layer> {
    let Some(root) = tree.get(scope) else {
        return Vec::new();
    };
    root.children
        .iter()
        .map(|&id| {
            let mut layer = Layer::default();
            layer.id = id;
            layer.frame = geometry
                .iter()
                .find(|g| g.id == id && !g.visible.is_empty())
                .map(|g| points(g.bounds, cells));
            if let Some(view) = tree.get(id) {
                layer.order = view.tree_order;
                layer.takes_presses = has_press(view.events)
                    && view.props.pointer_events != nsh_core::PointerEvents::None
                    && !view.style.hidden;
            }
            layer
        })
        .collect()
}

#[derive(Default)]
pub(crate) struct Keyboard {
    pub(crate) focused: Option<NodeId>,
    /// The size of a Cell, which the frames given to `nsh_core::focus` are
    /// in.
    cells: CellSize,
    /// The text of the last search match outside any control; traversal continues from it.
    pub(crate) search_point: Option<NodeId>,
    /// The control the focus last left for none (Escape out of a field):
    /// the next Tab continues after it, not from the top.
    left: Option<NodeId>,
    modals: focus::Modals,
    /// The layers the focus went into (a root view's children: the page,
    /// a sheet's panel, any overlay), each with the stop it came from: a
    /// layer that goes with the focus in it gives it back there, as a
    /// modal does.
    layers: focus::Layers,
    screens: Vec<NodeId>,
    screen_focus: Vec<(NodeId, Option<NodeId>)>,
    pending_scroll_focus: Option<ScrollFocus>,
    /// Focus changes made here, and those the app's keymap has caught up
    /// with: the app's bindings that hold only while a control is focused
    /// (an editor's Tab) change in effects on its focus, so until the
    /// engine has run them the mirrored keymap is the old focus's.
    moved: u64,
    caught_up: u64,
    /// The inbox the engine tells through that it caught up with a focus
    /// change: after the batches that change made, so the keymap they
    /// carry is applied first.
    pub(crate) sync: Option<(
        crossbeam_channel::Sender<crate::surface::Message>,
        std::sync::Arc<crate::surface::Wake>,
    )>,
}

/// The layer `id` is in: the child of its root view (the surface, or the
/// modal it is in) around it. An overlay's view is one of these, shown in
/// its root view whatever its place in the app.
fn layer(tree: &Mounter, id: NodeId) -> Option<NodeId> {
    let mut id = host(tree, id);
    loop {
        let parent = tree.get(id)?.parent?;
        if parent == ROOT_ID || tree.get(parent)?.kind == ViewKind::Modal {
            return Some(id);
        }
        id = parent;
    }
}

/// The covering layer (`focus::covering_layer`) over `id`'s layer in its
/// root view, if any: the root view's child above `id`'s that takes it out
/// of reach.
fn covering_root(
    cells: CellSize,
    tree: &Mounter,
    geometry: &[Geometry],
    id: NodeId,
) -> Option<NodeId> {
    let own = layer(tree, id)?;
    let scope = tree.get(own)?.parent?;
    focus::covered_by(&layers(cells, tree, geometry, scope), own)
}

/// The stops of `scope` in focus order (`focus::candidates`).
fn candidates_in<'a>(
    cells: CellSize,
    tree: &Mounter,
    geometry: &'a [Geometry],
    scope: NodeId,
) -> Vec<Stop<'a>> {
    focus::candidates(
        geometry
            .iter()
            .filter_map(|g| candidate(cells, tree, g, scope)),
        scope,
        &layers(cells, tree, geometry, scope),
    )
}

#[derive(Clone, Copy)]
enum FocusPosition {
    Nearest,
    RetainVisible,
}

#[derive(Clone, Copy)]
struct ScrollFocus {
    scroll: NodeId,
    anchor: Point<i32>,
    position: FocusPosition,
}

/// The terminal cursor of an editor's sink: an editable input without a caret
/// of its own and without columns (1 pt wide), which an `Editor` or a
/// `RichEditor` keeps at its caret. The cursor is the Cell at its origin, while
/// the nearest ancestor with Cells shows it.
fn sink_cursor(tree: &Mounter, geometry: &[Geometry], id: NodeId) -> Option<Position> {
    let find = |id: NodeId| geometry.iter().find(|g| g.id == id);
    let g = find(id)?;
    if g.bounds.right > g.bounds.left {
        return None;
    }
    let (x, y) = (
        u16::try_from(g.bounds.left).ok()?,
        u16::try_from(g.bounds.top).ok()?,
    );
    let at = Position::new(x, y);
    let mut parent = tree.get(id)?.parent;
    while let Some(id) = parent {
        if let Some(g) = find(id)
            && !g.visible.is_empty()
        {
            return g.visible.contains(at).then_some(at);
        }
        parent = tree.get(id)?.parent;
    }
    None
}

/// The view is inside an overlay anchored to another view (a menu's, a
/// select's or a popover's popup).
fn in_anchored_overlay(tree: &Mounter, id: NodeId) -> bool {
    anchored_overlay(tree, id).is_some()
}

/// The overlay anchored to another view that `id` is inside, if any.
fn anchored_overlay(tree: &Mounter, id: NodeId) -> Option<NodeId> {
    let mut id = host(tree, id);
    loop {
        let view = tree.get(id)?;
        if view.props.overlay_anchor.is_some() {
            return Some(id);
        }
        id = view.parent?;
    }
}

/// Whether anchored overlay `popup` hangs from a trigger: its anchor is a
/// focus stop among `candidates`, or holds one.
fn triggered(tree: &Mounter, popup: NodeId, candidates: &[Stop<'_>]) -> bool {
    let Some(anchor) = tree
        .get(popup)
        .and_then(|view| view.props.overlay_anchor.as_ref())
        .and_then(|anchor| anchor.node)
    else {
        return false;
    };
    candidates
        .iter()
        .any(|g| contains(tree, anchor, g.id) || contains(tree, g.id, anchor))
}

pub(crate) fn contains(tree: &Mounter, ancestor: NodeId, mut id: NodeId) -> bool {
    loop {
        if id == ancestor {
            return true;
        }
        let Some(parent) = parent(tree, id) else {
            return false;
        };
        id = parent;
    }
}

impl Keyboard {
    /// The keyboard of a surface whose Cells are `cells` big.
    pub(crate) fn new(cells: CellSize) -> Self {
        Self {
            cells,
            ..Default::default()
        }
    }

    /// The stops of the focus scope in focus order. A backdrop over the
    /// page (a sheet's, a dialog's) takes what is under it out of reach:
    /// Tab stays in the layers above it, as in a modal, and enters them
    /// first.
    pub(crate) fn candidates<'a>(&self, tree: &Mounter, geometry: &'a [Geometry]) -> Vec<Stop<'a>> {
        candidates_in(
            self.cells,
            tree,
            geometry,
            crate::navigation::focus_scope(tree),
        )
    }

    pub(crate) fn focus(
        &mut self,
        id: Option<NodeId>,
        tree: &mut Mounter,
        control: &EngineControl,
    ) {
        let old = self.focused;
        let left = self.left;
        let old_span = tree.focused_span.filter(|&span| Some(span) == old);
        self.focused = id;
        self.search_point = None;
        self.left = if id.is_none() {
            old.or(self.left)
        } else {
            None
        };
        if old != id {
            if let Some(field) = old
                .and_then(|id| tree.get_mut(id))
                .and_then(|v| v.field.as_mut())
            {
                field.focused = false;
            }
            if let Some(field) = id
                .and_then(|id| tree.get_mut(id))
                .and_then(|v| v.field.as_mut())
            {
                field.focused = true;
                field.report(id.unwrap(), control, false);
            }
        }
        tree.focused_span = id.filter(|id| tree.spans.contains_key(id));
        // Into a layer from none (the stop a backdrop just covered, or
        // the field Escape left): it gives the focus back to that stop.
        if let (Some(old), Some(new)) = (old.or(left), id)
            && let (Some(from), Some(into)) = (layer(tree, old), layer(tree, new))
            && from != into
        {
            self.layers.entered(into, old);
        }
        if let Some(focused) = id {
            let mut parent = Some(host(tree, focused));
            while let Some(id) = parent {
                let Some(view) = tree.get(id) else {
                    break;
                };
                if view.kind == ViewKind::Modal {
                    break;
                }
                if view.kind == ViewKind::Screen {
                    if let Some((_, saved)) = self
                        .screen_focus
                        .iter_mut()
                        .find(|(screen, _)| *screen == id)
                    {
                        *saved = Some(focused);
                    } else {
                        self.screen_focus.push((id, Some(focused)));
                    }
                }
                parent = view.parent;
            }
        }
        // The core derives ancestor focus-within from focused transitions.
        // Clear the old focus first, even when the new node has a lower id.
        if old != id
            && let Some(view) = old.and_then(|id| tree.get_mut(id))
            && view.states & FOCUSED != 0
        {
            view.states &= !FOCUSED;
            control.set_states(old.unwrap(), view.states);
        }
        // A link has no view to hold its states: it is focused or not.
        if old != id {
            if let Some(old) = old_span {
                control.set_states(old, 0);
            }
            if let Some(id) = id.filter(|id| tree.spans.contains_key(id)) {
                control.set_states(id, FOCUSED);
            }
        }
        self.sync_states(tree, control);
        if old != id {
            if let Some(old) = old {
                control.dispatch_event(old, EventKind::Blur, EventPayload::None);
            }
            if let Some(id) = id {
                control.dispatch_event(id, EventKind::Focus, EventPayload::None);
            }
            self.moved += 1;
            if let Some((sender, wake)) = self.sync.clone() {
                let moved = self.moved;
                control.flush(move |_| {
                    if sender.send(crate::surface::Message::Focused(moved)).is_ok() {
                        wake.notify();
                    }
                });
            } else {
                self.caught_up = self.moved;
            }
        }
    }

    /// The engine has run what focus change `moved` made, and its batches are applied.
    pub(crate) fn is_caught_up(&self) -> bool {
        self.caught_up >= self.moved
    }

    pub(crate) fn caught_up(&mut self, moved: u64) {
        self.caught_up = self.caught_up.max(moved);
    }

    /// The stop a pointer's press on `node` gives the focus to: `node`
    /// or its nearest ancestor that Tab stops at, as terminal apps move
    /// the focus to what is clicked (`None`: the press is on no stop).
    pub(crate) fn pressed_stop(
        &self,
        node: NodeId,
        tree: &Mounter,
        geometry: &[Geometry],
    ) -> Option<NodeId> {
        let candidates = self.candidates(tree, geometry);
        let mut ancestor = Some(node);
        while let Some(id) = ancestor {
            if candidates.iter().any(|g| g.id == id) {
                return Some(id);
            }
            ancestor = tree.get(id).and_then(|view| view.parent);
        }
        None
    }

    /// Focuses the control around a search match, or none, keeping the match's text as the search point.
    pub(crate) fn focus_match(
        &mut self,
        text: NodeId,
        tree: &mut Mounter,
        geometry: &[Geometry],
        control: &EngineControl,
    ) {
        self.pending_scroll_focus = None;
        let candidates = self.candidates(tree, geometry);
        let mut ancestor = Some(text);
        while let Some(id) = ancestor
            && !candidates.iter().any(|g| g.id == id)
        {
            ancestor = tree.get(id).and_then(|view| view.parent);
        }
        self.focus(ancestor, tree, control);
        if ancestor.is_none() {
            self.search_point = Some(text);
            self.left = None;
        }
    }

    fn sync_states(&self, tree: &mut Mounter, control: &EngineControl) {
        for index in 0..tree.views.len() {
            let id = index as NodeId;
            let Some(view) = tree.get(id) else {
                continue;
            };
            // FOCUSED is reported whatever the view tracks (`next_states`):
            // the painter draws every focused control, styled or not.
            let mut states = view.states;
            for (bit, on) in [
                (FOCUSED, self.focused == Some(id)),
                (
                    FOCUS_WITHIN,
                    self.focused
                        .is_some_and(|focused| contains(tree, id, focused)),
                ),
            ] {
                if let Some(next) = next_states(view.tracked, states, bit, on) {
                    states = next;
                }
            }
            if states != view.states {
                tree.get_mut(id).unwrap().states = states;
                control.set_states(id, states);
            }
        }
    }

    pub(crate) fn reconcile(
        &mut self,
        tree: &mut Mounter,
        geometry: &[Geometry],
        control: &EngineControl,
    ) -> bool {
        let old_focus = self.focused;
        let active: Vec<_> = tree
            .modals
            .iter()
            .copied()
            .filter(|&id| crate::navigation::modal_visible(tree, id))
            .collect();
        let cells = self.cells;
        let moves = {
            let tree = &*tree;
            self.modals.update(
                &active,
                self.focused,
                |modal| {
                    candidates_in(cells, tree, geometry, modal)
                        .first()
                        .map(|g| g.id)
                },
                |scope, id| {
                    candidates_in(cells, tree, geometry, scope)
                        .iter()
                        .any(|g| g.id == id && !g.visible.is_empty())
                },
            )
        };
        let modal_changed = moves.is_some();
        for step in moves.into_iter().flatten() {
            self.focus(step.focus, tree, control);
            if step.reveal
                && let Some(id) = step.focus
            {
                reveal(tree, geometry, id);
            }
        }
        if let Some(focused) = self.focused
            && !self
                .candidates(tree, geometry)
                .iter()
                .any(|g| g.id == focused)
        {
            // Gone with its layer (a sheet closed, an overlay let go): the
            // focus goes back to where it came into that layer from, or
            // from the layer that one was in, as a modal gives it back.
            let restored = (!contains(tree, ROOT_ID, focused))
                .then(|| self.layer_restore(tree, geometry))
                .flatten();
            // Covered by a backdrop (a sheet opened from it): the layer
            // over it gives it back when it goes, whether the focus went
            // into that layer or Escape closed it first.
            if restored.is_none()
                && contains(tree, ROOT_ID, focused)
                && let Some(cover) = covering_root(cells, tree, geometry, focused)
            {
                self.layers.entered(cover, focused);
            }
            self.focus(restored, tree, control);
        } else if self.focused.is_none()
            && let Some(restored) = self.layer_restore(tree, geometry)
        {
            // The layer gone took no focus of its own (closed by Escape
            // with none in it): the stop it covered still takes it back.
            self.focus(Some(restored), tree, control);
        }
        self.layers.retain(|layer| contains(tree, ROOT_ID, layer));
        let first = {
            let tree = &*tree;
            self.modals.first_stop(self.focused, || {
                let scope = crate::navigation::focus_scope(tree);
                candidates_in(cells, tree, geometry, scope)
                    .first()
                    .map(|g| g.id)
            })
        };
        if let Some(id) = first {
            self.focus(Some(id), tree, control);
            reveal(tree, geometry, id);
        }
        let scope = crate::navigation::focus_scope(tree);
        let screens: Vec<_> = geometry
            .iter()
            .filter(|g| {
                input_scope(tree, g.id) == scope
                    && tree
                        .get(g.id)
                        .is_some_and(|view| view.kind == ViewKind::Screen)
            })
            .map(|g| g.id)
            .collect();
        if !modal_changed && self.focused.is_none() {
            let candidates = self.candidates(tree, geometry);
            let restored = screens
                .iter()
                .rev()
                .filter(|screen| !self.screens.contains(screen))
                .find_map(|screen| {
                    let (_, saved) = self.screen_focus.iter().find(|(seen, _)| seen == screen)?;
                    if let Some(id) = saved.filter(|id| {
                        candidates
                            .iter()
                            .any(|g| g.id == *id && !g.visible.is_empty())
                    }) {
                        Some((id, false))
                    } else {
                        candidates
                            .iter()
                            .find(|g| contains(tree, *screen, g.id))
                            .map(|g| (g.id, true))
                    }
                });
            if let Some((id, fallback)) = restored {
                self.focus(Some(id), tree, control);
                if fallback {
                    reveal(tree, geometry, id);
                }
            }
        }
        for &screen in &screens {
            if !self.screen_focus.iter().any(|(seen, _)| *seen == screen) {
                self.screen_focus.push((screen, None));
            }
        }
        self.apply_scroll_focus(tree, geometry, control);
        self.screens = screens;
        self.screen_focus
            .retain(|(screen, _)| tree.get(*screen).is_some());
        self.sync_states(tree, control);
        old_focus != self.focused || !tree.scrolled.is_empty()
    }

    /// The stop the latest layer gone from the tree took the focus from,
    /// walking back while that stop went too.
    fn layer_restore(&self, tree: &Mounter, geometry: &[Geometry]) -> Option<NodeId> {
        let candidates = self.candidates(tree, geometry);
        self.layers.restore(
            |layer| !contains(tree, ROOT_ID, layer),
            |id| {
                candidates
                    .iter()
                    .any(|g| g.id == id && !g.visible.is_empty())
            },
        )
    }

    fn queue_scroll_focus(
        &mut self,
        geometry: &[Geometry],
        scroll: NodeId,
        position: FocusPosition,
    ) {
        let Some(viewport) = geometry.iter().find(|g| g.id == scroll) else {
            return;
        };
        let focused = geometry
            .iter()
            .find(|g| Some(g.id) == self.focused && !g.visible.is_empty())
            .unwrap_or(viewport);
        self.pending_scroll_focus = Some(ScrollFocus {
            scroll,
            anchor: Point::new(
                i32::from(focused.visible.x) - viewport.bounds.left,
                i32::from(focused.visible.y) - viewport.bounds.top,
            ),
            position,
        });
    }

    fn apply_scroll_focus(
        &mut self,
        tree: &mut Mounter,
        geometry: &[Geometry],
        control: &EngineControl,
    ) {
        let Some(request) = self.pending_scroll_focus.take() else {
            return;
        };
        let scope = crate::navigation::focus_scope(tree);
        let Some(viewport) = geometry.iter().find(|g| {
            g.id == request.scroll && !g.visible.is_empty() && input_scope(tree, g.id) == scope
        }) else {
            return;
        };
        let Some(scroll) = tree
            .get(request.scroll)
            .and_then(|view| view.scroll.as_ref())
        else {
            return;
        };
        if matches!(request.position, FocusPosition::RetainVisible)
            && self
                .focused
                .is_some_and(|id| geometry.iter().any(|g| g.id == id && !g.visible.is_empty()))
        {
            return;
        }
        let candidates: Vec<_> = self
            .candidates(tree, geometry)
            .into_iter()
            .filter(|g| {
                !g.visible.is_empty()
                    && (matches!(request.position, FocusPosition::RetainVisible)
                        || contains(tree, request.scroll, g.id))
            })
            .collect();
        let anchor = Point::new(
            viewport.bounds.left + request.anchor.x,
            viewport.bounds.top + request.anchor.y,
        );
        let next = candidates
            .iter()
            .copied()
            .min_by_key(|g| {
                let dx = (i32::from(g.visible.x) - anchor.x).unsigned_abs();
                let dy = (i32::from(g.visible.y) - anchor.y).unsigned_abs();
                if matches!(request.position, FocusPosition::RetainVisible) {
                    (dx.saturating_add(dy), dy)
                } else if scroll.horizontal {
                    (dx, dy)
                } else {
                    (dy, dx)
                }
            })
            .map(|g| g.id);
        self.focus(next, tree, control);
        if next.is_none() {
            self.pending_scroll_focus = Some(request);
        }
    }

    pub(crate) fn wheel(
        &mut self,
        mouse: crate::MouseInput,
        area: ratatui_core::layout::Rect,
        cells: crate::CellSize,
        hits: &[crate::render::Hit],
        tree: &mut Mounter,
        geometry: &[Geometry],
    ) -> Handled {
        let crate::MouseKind::Wheel { x, y } = mouse.kind else {
            return Handled::Ignored;
        };
        let position = Position::new(mouse.column, mouse.row);
        if !area.contains(position) {
            return Handled::Ignored;
        }
        let scope = crate::navigation::focus_scope(tree);
        let unused = if scope == ROOT_ID {
            Handled::Ignored
        } else {
            Handled::Yes
        };
        let Some(hit) = hits
            .iter()
            .rev()
            .find(|hit| hit.area.contains(position) && contains(tree, scope, hit.node))
        else {
            return unused;
        };
        let (mut x, mut y) = (f32::from(x), f32::from(y));
        if mouse.modifiers.shift {
            x += y;
            y = 0.0;
        }
        let delta = Point::new(
            cells.width() * x * crate::input::WHEEL_COLUMNS,
            cells.height() * y * crate::input::WHEEL_ROWS,
        );
        let mut views = Vec::new();
        let mut id = hit.node;
        while let Some(view) = tree.get(id) {
            if view.scroll.is_some() {
                views.push(id);
            }
            if id == scope {
                break;
            }
            let Some(parent) = view.parent else {
                break;
            };
            id = parent;
        }
        let target = nsh_core::input::scroll_target(
            views,
            delta.x,
            delta.y,
            |id| {
                tree.get(*id)
                    .and_then(|v| v.scroll.as_ref())
                    .map(|s| s.horizontal)
            },
            |id| {
                tree.get(*id).is_some_and(|v| {
                    v.scroll
                        .as_ref()
                        .is_some_and(|s| s.can_scroll(v.frame.size, delta))
                })
            },
        );
        let Some(id) = target else {
            return unused;
        };
        let scroll = tree.get(id).unwrap().scroll.as_ref().unwrap();
        let to = if scroll.horizontal {
            let direction = if scroll.mirrored && x != 0.0 {
                -1.0
            } else {
                1.0
            };
            scroll.x
                + cells.width()
                    * crate::input::WHEEL_COLUMNS
                    * if x != 0.0 { x * direction } else { y }
        } else {
            scroll.y + delta.y
        };
        // The wheel scrolls the pane under the pointer and leaves the
        // keyboard focus where it is, in sight or not.
        let _ = geometry;
        move_scroll(tree, id, to);
        Handled::Yes
    }

    pub(crate) fn cursor(&self, tree: &Mounter, geometry: &[Geometry]) -> Option<Position> {
        let input = tree.get(self.focused?)?.input.as_ref()?;
        if !input.editable {
            return None;
        }
        if input.caret_hidden {
            return sink_cursor(tree, geometry, self.focused?);
        }
        let _ = geometry
            .iter()
            .find(|g| Some(g.id) == self.focused && !g.visible.is_empty())?;
        tree.get(self.focused?)?.field.as_ref()?.cursor()
    }

    pub(crate) fn shortcut(
        &self,
        tree: &Mounter,
        stroke: Keystroke,
        repeat: bool,
        command_key: bool,
        control: &EngineControl,
    ) -> bool {
        let editing = self
            .focused
            .and_then(|id| tree.get(id))
            .is_some_and(|v| v.kind == ViewKind::Input);
        let mut found = tree.keymap.find(stroke, editing, repeat);
        // A terminal that cannot report Cmd sends Ctrl for it on macOS: the
        // app's `mod-` / `cmd+` bindings take Ctrl there.
        if matches!(found, KeyMatch::None)
            && !command_key
            && crate::app::OS == nsh_core::platform::Os::MacOs
            && stroke.modifiers.ctrl
            && !stroke.modifiers.cmd
        {
            let mut command = stroke;
            command.modifiers.ctrl = false;
            command.modifiers.cmd = true;
            found = tree.keymap.find(command, editing, repeat);
        }
        if let Some(id) = found.fired() {
            control.dispatch_event(
                ROOT_ID,
                EventKind::KeyCommand,
                EventPayload::Value(PayloadValue::Number(id as f64)),
            );
        }
        found.consumed()
    }

    pub(crate) fn action(
        &mut self,
        action: Action,
        tree: &mut Mounter,
        geometry: &[Geometry],
        control: &EngineControl,
        modifiers: nsh_core::keymap::Modifiers,
    ) -> Handled {
        match action {
            Action::Next | Action::Previous | Action::Left | Action::Right => {
                self.pending_scroll_focus = None;
                let mut candidates = self.candidates(tree, geometry);
                // Tab stays in the open popup of a trigger the focus is in
                // (a date picker's calendar, a menu): its box covers the
                // page's controls around the trigger, which would take the
                // focus unseen, and it stays open until Escape or a choice
                // closes it, which gives the focus back to the trigger. A
                // popup anchored to no stop of its own is a part of the
                // page, which Tab passes through, and arrows may leave a
                // popup for the column beside it, but for a menu's: Left
                // and Right on an item its menu does not take (no
                // submenu) would leave the menu open behind the focus.
                let in_menu = self
                    .focused
                    .and_then(|id| tree.get(id))
                    .is_some_and(|view| {
                        matches!(
                            view.props.accessibility.role,
                            nsh_core::AccessibilityRole::Menuitem
                                | nsh_core::AccessibilityRole::MenuItemCheckbox
                                | nsh_core::AccessibilityRole::MenuItemRadio
                        )
                    });
                if (matches!(action, Action::Next | Action::Previous) || in_menu)
                    && let Some(popup) = self
                        .focused
                        .and_then(|id| anchored_overlay(tree, id))
                        .filter(|&popup| triggered(tree, popup, &candidates))
                {
                    candidates.retain(|g| contains(tree, popup, g.id));
                }
                if candidates.is_empty() {
                    return if crate::navigation::focus_scope(tree) == ROOT_ID {
                        Handled::Ignored
                    } else {
                        Handled::Yes
                    };
                }
                let current = candidates.iter().position(|g| Some(g.id) == self.focused);
                // Out of a field with Escape, Tab goes on from it.
                let left_id = current.is_none().then_some(self.left).flatten();
                let left = left_id.and_then(|id| candidates.iter().position(|g| g.id == id));
                // A plain-text search point continues to the controls beside it
                // in tree order, so a sidebar on the same rows is not visited first;
                // so does a field left that is no stop of its own (a rich
                // editor's sink, no Cell wide: its content took the Tab).
                let point = self
                    .search_point
                    .filter(|&id| geometry.iter().any(|g| g.id == id))
                    .or(left_id.filter(|_| left.is_none()))
                    .and_then(|id| tree.get(id))
                    .map(|view| Order::view(view.tree_order));
                let next = match action {
                    Action::Next | Action::Previous => focus::tab(
                        &candidates,
                        current,
                        left,
                        point,
                        matches!(action, Action::Previous),
                    ),
                    _ => focus::horizontal(&candidates, current, matches!(action, Action::Right)),
                };
                if let Some(index) = next {
                    let id = candidates[index].id;
                    self.focus(Some(id), tree, control);
                    reveal(tree, geometry, id);
                }
                Handled::Yes
            }
            Action::Down | Action::Up => {
                self.vertical(matches!(action, Action::Down), tree, geometry, control)
            }
            Action::Activate => {
                if self.pending_scroll_focus.is_some() {
                    return Handled::Yes;
                }
                let candidates = self.candidates(tree, geometry);
                let Some(g) = candidates
                    .iter()
                    .find(|g| Some(g.id) == self.focused && !g.visible.is_empty())
                else {
                    return if crate::navigation::focus_scope(tree) == ROOT_ID {
                        Handled::Ignored
                    } else {
                        Handled::Yes
                    };
                };
                // A link takes Return as a click on its characters does.
                let events = match (tree.spans.get(&g.id), tree.get(g.id)) {
                    (Some(span), _) => span.events,
                    (None, Some(view)) => view.events,
                    (None, None) => return Handled::Ignored,
                };
                if !has_press(events) {
                    return Handled::Ignored;
                }
                let position =
                    nsh_core::Point::new(g.frame.size.width / 2.0, g.frame.size.height / 2.0);
                let payload = EventPayload::Press(nsh_core::PressEvent {
                    position,
                    page_position: g.frame.origin + position,
                    modifiers,
                    clicks: 1,
                });
                for event in [EventKind::PressDown, EventKind::PressUp, EventKind::Press] {
                    if events.contains(event) {
                        control.dispatch_event(g.id, event, payload.clone());
                    }
                }
                Handled::Yes
            }
            Action::Back => {
                self.pending_scroll_focus = None;
                if crate::navigation::back(tree, control, true) {
                    Handled::Yes
                } else {
                    Handled::Ignored
                }
            }
            Action::PageDown | Action::PageUp | Action::Start | Action::End => {
                let target = self
                    .focused
                    .or(self.search_point)
                    .filter(|&id| {
                        geometry.iter().any(|g| g.id == id)
                            && input_scope(tree, id) == crate::navigation::focus_scope(tree)
                    })
                    .and_then(|id| scroll_ancestor(tree, id))
                    .or_else(|| {
                        geometry
                            .iter()
                            .find(|g| {
                                contains(tree, crate::navigation::focus_scope(tree), g.id)
                                    && tree.get(g.id).is_some_and(|v| v.scroll.is_some())
                            })
                            .map(|g| g.id)
                    });
                let Some(id) = target else {
                    return Handled::Ignored;
                };
                self.pending_scroll_focus = None;
                let candidates: Vec<_> = self
                    .candidates(tree, geometry)
                    .into_iter()
                    .filter(|g| contains(tree, id, g.id))
                    .map(|g| g.id)
                    .collect();
                if matches!(action, Action::Start | Action::End) {
                    let target = if matches!(action, Action::Start) {
                        candidates.first()
                    } else {
                        candidates.last()
                    };
                    if let Some(&target) = target {
                        self.focus(Some(target), tree, control);
                        reveal(tree, geometry, target);
                        return Handled::Yes;
                    }
                }
                let view = tree.get_mut(id).unwrap();
                let scroll = view.scroll.as_mut().unwrap();
                let (offset, max, page) = if scroll.horizontal {
                    (
                        scroll.x,
                        scroll.max(view.frame.size).x,
                        view.frame.size.width,
                    )
                } else {
                    (
                        scroll.y,
                        scroll.max(view.frame.size).y,
                        view.frame.size.height,
                    )
                };
                let to = match action {
                    Action::PageDown => offset + page,
                    Action::PageUp => offset - page,
                    Action::Start => nsh_core::Points::ZERO,
                    Action::End => max,
                    _ => unreachable!(),
                };
                if !candidates.is_empty() {
                    self.queue_scroll_focus(geometry, id, FocusPosition::Nearest);
                }
                move_scroll(tree, id, to);
                Handled::Yes
            }
            Action::Quit => {
                tree.exit = true;
                Handled::Yes
            }
            Action::Search(_) => Handled::Ignored,
        }
    }
}

impl Keyboard {
    /// Down / Up: the nearest control below / above the focused one, the
    /// one in an overlapping column first; with none focused, the first /
    /// last one in sight, in an open anchored popup when there is one. A control out of sight is approached one wheel
    /// step at a time while the scroll view around can still scroll there,
    /// and focus follows what is in sight then.
    fn vertical(
        &mut self,
        down: bool,
        tree: &mut Mounter,
        geometry: &[Geometry],
        control: &EngineControl,
    ) -> Handled {
        self.pending_scroll_focus = None;
        let scope = crate::navigation::focus_scope(tree);
        let unused = if scope == ROOT_ID {
            Handled::Ignored
        } else {
            Handled::Yes
        };
        let candidates = self.candidates(tree, geometry);
        let current = candidates.iter().position(|g| Some(g.id) == self.focused);
        // With none focused, a popup opened by a click (a menu, a select)
        // holds what the arrows are for: it is entered before the page.
        let best = focus::vertical(&candidates, current, down).map(|index| candidates[index]);
        if let Some(to) = best
            && !to.visible.is_empty()
        {
            self.focus(Some(to.id), tree, control);
            reveal(tree, geometry, to.id);
            return Handled::Yes;
        }
        // Out of sight or nothing there: scroll toward it.
        let from = self
            .focused
            .filter(|&id| geometry.iter().any(|g| g.id == id))
            .or(best.map(|g| g.id));
        let scroll = from
            .and_then(|id| vertical_scroll(tree, id, down))
            .or_else(|| {
                geometry
                    .iter()
                    .filter(|g| !g.visible.is_empty() && contains(tree, scope, g.id))
                    .find_map(|g| vertical_scroll(tree, g.id, down).filter(|&id| id == g.id))
            });
        let Some(id) = scroll else {
            if let Some(to) = best {
                self.focus(Some(to.id), tree, control);
                reveal(tree, geometry, to.id);
                return Handled::Yes;
            }
            return unused;
        };
        let Some(viewport) = geometry.iter().find(|g| g.id == id) else {
            return unused;
        };
        // A control within one step comes into sight as it takes focus.
        if let Some(to) = best
            && contains(tree, id, to.id)
            && if down {
                to.bounds.bottom - viewport.bounds.bottom
            } else {
                viewport.bounds.top - to.bounds.top
            } <= crate::input::WHEEL_ROWS as i32
        {
            self.focus(Some(to.id), tree, control);
            reveal(tree, geometry, to.id);
            return Handled::Yes;
        }
        let rows = (viewport.bounds.bottom - viewport.bounds.top).max(1) as f32;
        let row = viewport.frame.size.height / rows;
        let view = tree.get(id).unwrap();
        let offset = view.scroll.as_ref().unwrap().y;
        let step = row * crate::input::WHEEL_ROWS;
        self.queue_scroll_focus(geometry, id, FocusPosition::RetainVisible);
        move_scroll(tree, id, if down { offset + step } else { offset - step });
        Handled::Yes
    }
}

/// The innermost vertical scroll view around `id` (itself included) that
/// can still scroll down (`down`) or up.
fn vertical_scroll(tree: &Mounter, id: NodeId, down: bool) -> Option<NodeId> {
    let mut id = host(tree, id);
    loop {
        let view = tree.get(id)?;
        if let Some(scroll) = view.scroll.as_ref()
            && !scroll.horizontal
        {
            let max = scroll.max(view.frame.size).y;
            if (down && scroll.y < max) || (!down && scroll.y > nsh_core::Points::ZERO) {
                return Some(id);
            }
        }
        id = view.parent?;
    }
}

fn scroll_ancestor(tree: &Mounter, id: NodeId) -> Option<NodeId> {
    let mut id = host(tree, id);
    loop {
        let view = tree.get(id)?;
        if view.scroll.is_some() {
            return Some(id);
        }
        id = view.parent?;
    }
}

pub(crate) fn move_scroll(
    tree: &mut Mounter,
    id: NodeId,
    to: nsh_core::Points,
) -> nsh_core::Points {
    let view = tree.get_mut(id).unwrap();
    let scroll = view.scroll.as_mut().unwrap();
    let max = scroll.max(view.frame.size);
    let (offset, max) = if scroll.horizontal {
        (&mut scroll.x, max.x)
    } else {
        (&mut scroll.y, max.y)
    };
    let old = *offset;
    *offset = to.max(nsh_core::Points::ZERO).min(max);
    let delta = *offset - old;
    if delta != nsh_core::Points::ZERO && !tree.scrolled.contains(&id) {
        tree.scrolled.push(id);
    }
    delta
}

fn reveal(tree: &mut Mounter, geometry: &[Geometry], id: NodeId) {
    if let Some(target) = geometry.iter().find(|g| g.id == id) {
        reveal_frame(tree, geometry, id, target.frame);
    }
}

/// Scrolls the Scroll views around `id` until `frame`, in the surface's points, is in sight.
pub(crate) fn reveal_frame(
    tree: &mut Mounter,
    geometry: &[Geometry],
    id: NodeId,
    frame: nsh_core::Bounds<nsh_core::Points>,
) {
    let mut origin = frame.origin;
    let mut parent = parent(tree, id);
    while let Some(id) = parent {
        let view = tree.get(id).unwrap();
        parent = view.parent;
        let Some(scroll) = &view.scroll else {
            continue;
        };
        let Some(g) = geometry.iter().find(|g| g.id == id) else {
            continue;
        };
        let horizontal = scroll.horizontal;
        let relative = origin - g.frame.origin + g.scroll_origin - scroll.origin();
        let (start, len, viewport, offset) = if horizontal {
            (
                relative.x,
                frame.size.width,
                view.frame.size.width,
                scroll.x,
            )
        } else {
            (
                relative.y,
                frame.size.height,
                view.frame.size.height,
                scroll.y,
            )
        };
        // Centered vertically; oversized spans keep the core's margin and
        // start-first policy.
        let align = if horizontal {
            Align::Nearest
        } else {
            Align::Center
        };
        if let Some(delta) = focus::reveal(start, len, viewport, align) {
            // The terminal scrolls by whole Cells: the nearest offset that still
            // shows the span when it fits.
            let cells = if horizontal {
                g.bounds.right - g.bounds.left
            } else {
                g.bounds.bottom - g.bounds.top
            };
            let to = snap(offset + delta, viewport, cells, start + offset, len);
            let moved = move_scroll(tree, id, to);
            if horizontal {
                origin.x -= moved;
            } else {
                origin.y -= moved;
            }
        }
    }
}

/// `to`, an offset of a viewport `viewport` long that is `cells` Cells, on
/// whole Cells: the nearest (an exact half back), moved a Cell further when
/// that leaves part of the span (`start` from the content's start, `len`
/// long) out of sight while it fits.
fn snap(
    to: nsh_core::Points,
    viewport: nsh_core::Points,
    cells: i32,
    start: nsh_core::Points,
    len: nsh_core::Points,
) -> nsh_core::Points {
    if cells <= 0 {
        return to;
    }
    let unit = viewport.0 / cells as f32;
    let units = to.0 / unit;
    let whole = units.floor();
    let mut snapped = if units - whole > 0.5 + 1.0e-4 {
        whole + 1.0
    } else {
        whole
    } * unit;
    if len.0 <= viewport.0 + 1.0e-4 {
        if start.0 - snapped < -1.0e-4 {
            snapped -= unit;
        } else if start.0 + len.0 - snapped > viewport.0 + 1.0e-4 {
            snapped += unit;
        }
    }
    nsh_core::Points(snapped.max(0.0))
}

pub(crate) fn input_scope(tree: &Mounter, id: NodeId) -> NodeId {
    let mut id = host(tree, id);
    while let Some(view) = tree.get(id) {
        if view.kind == ViewKind::Modal {
            return id;
        }
        let Some(parent) = view.parent else {
            break;
        };
        id = parent;
    }
    ROOT_ID
}
