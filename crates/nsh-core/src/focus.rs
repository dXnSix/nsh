//! The decisions of keyboard focus the backends share, over plain data:
//! the scope the focus moves in ([`focus_scope`]: the newest modal that
//! shows), its stops there in focus order ([`candidates`], without those a
//! backdrop over the page takes out of reach: [`covering_layer`]), where
//! Tab, Left / Right and Up / Down move ([`tab`], [`horizontal`],
//! [`vertical`]), how far a scroll view moves to bring the stop that takes
//! the focus into sight ([`reveal`]), and which stop takes the focus back
//! when a modal or a layer goes ([`Modals`], [`Layers`]).
//!
//! The backends describe their views ([`Candidate`], [`Layer`]: ids,
//! frames, focus stops, scopes and tree order) and apply what comes back:
//! they move the focus, report `focused` / `focus-within` and `Focus` /
//! `Blur`, and scroll their views. The terminal backend uses it, its Cells
//! one point each, and so does the GPUI backend, over its views' frames in
//! the window's points.
//!
//! Frames are in the unit the backend lays out in, scaled along each axis
//! on its own: the surface's points, or a terminal's Cells, one point each
//! (8 × 16 points apiece by default, one scale across and another down).
//! Every rule compares one axis at a time (edges with edges, a gap with a
//! gap, a distance across with a distance across) and takes no length of
//! its own, so frames scaled by one factor across and another down move the
//! focus as they did; edges that meet must meet exactly in that unit. A new
//! rule keeps that property, or the terminal backend converts its Cells to
//! points for it. [`reveal`] is apart: it takes lengths along one axis in
//! points, the layout's, as [`scroll::reveal_delta`]'s margin is.

use std::cmp::Ordering;

use crate::{Bounds, NodeId, Points, ROOT_ID, input, scroll};

/// Where a stop comes in the focus order: its view's place in tree order
/// (`MountItem::TreeOrder`), and a text's link right after its text, in
/// text order.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Order {
    /// The view's place in tree order (a link's text's).
    pub tree: usize,
    /// 0 for a view, a link's place among its text's links from 1.
    pub link: usize,
}

impl Order {
    /// The order of a view at `tree` in tree order.
    pub fn view(tree: usize) -> Order {
        Order { tree, link: 0 }
    }

    /// The order of link `index` (from 0) of the text at `tree` in tree
    /// order.
    pub fn link(tree: usize, index: usize) -> Order {
        Order {
            tree,
            link: index.saturating_add(1),
        }
    }
}

/// A view, or a text's link, that may take the keyboard focus, as a
/// backend describes it.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Candidate {
    /// Its node (a link's own id).
    pub id: NodeId,
    /// Its frame: in the surface's points, or in any unit scaled along
    /// each axis on its own, as a terminal's Cells (the module's docs).
    pub frame: Bounds<Points>,
    /// Some of it is in sight: neither clipped away nor scrolled out.
    pub visible: bool,
    /// It is a focus stop: a view that is not disabled and that
    /// `ViewProps::focus_stop` makes one, or a link that is one
    /// (`TextLink::focus_stop`).
    pub stop: bool,
    /// It takes typing (an input that edits): a stop at any size, as an
    /// editor's sink narrower than a Cell.
    pub editable: bool,
    /// Its input scope: the modal it is in, or `ROOT_ID`.
    pub scope: NodeId,
    /// The layer it is in: the child of its root view (the surface, or its
    /// modal) around it.
    pub layer: Option<NodeId>,
    /// It is inside an overlay anchored to another view (a menu's, a
    /// select's or a popover's popup).
    pub popup: bool,
    /// Where it comes in the focus order.
    pub order: Order,
}

impl Candidate {
    /// Whether it takes the focus: a stop with an area, or one that takes
    /// typing.
    pub fn takes_focus(&self) -> bool {
        self.stop
            && (self.editable
                || (self.frame.size.width > Points::ZERO && self.frame.size.height > Points::ZERO))
    }
}

impl AsRef<Candidate> for Candidate {
    fn as_ref(&self) -> &Candidate {
        self
    }
}

/// A child of a root view (the surface, or a modal): the page first, then
/// what shows over it (a sheet's panel, a dialog's backdrop, an overlay).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layer {
    /// Its node.
    pub id: NodeId,
    /// Its place in tree order.
    pub order: usize,
    /// Its frame, in the unit of the candidates' (the surface's points, or
    /// any unit scaled along each axis on its own); `None` while none of it
    /// is in sight.
    pub frame: Option<Bounds<Points>>,
    /// It takes presses: it listens to them, pointer events reach it and
    /// it is not hidden.
    pub takes_presses: bool,
}

/// The scope the keyboard focus moves in: the newest of `modals` (oldest
/// first) that `shows`, or the surface's root.
pub fn focus_scope(modals: &[NodeId], shows: impl Fn(NodeId) -> bool) -> NodeId {
    modals
        .iter()
        .rev()
        .copied()
        .find(|&id| shows(id))
        .unwrap_or(ROOT_ID)
}

/// The topmost of a root view's `layers` (its children, the page first)
/// that covers the page and takes presses (a sheet's or a dialog's
/// backdrop), if any: the stops of the layers under it are out of reach.
pub fn covering_layer(layers: &[Layer]) -> Option<&Layer> {
    let page = layers.first()?.frame?;
    layers.iter().skip(1).rev().find(|layer| {
        layer.takes_presses
            && layer.frame.is_some_and(|covers| {
                covers.left() <= page.left()
                    && covers.top() <= page.top()
                    && covers.right() >= page.right()
                    && covers.bottom() >= page.bottom()
            })
    })
}

/// The covering layer of `layers` ([`covering_layer`]) over layer `own`,
/// if any: the one above it that takes it out of reach.
pub fn covered_by(layers: &[Layer], own: NodeId) -> Option<NodeId> {
    let cover = covering_layer(layers)?;
    let own = layers.iter().find(|layer| layer.id == own)?;
    (cover.order > own.order).then_some(cover.id)
}

/// The stops of `scope` among `nodes`, in focus order (equal orders as
/// they come), less those of a covering layer of `layers` (the children of
/// the scope's root view) and of the layers under it: a backdrop over the
/// page (a sheet's, a dialog's) keeps the focus in the layers above it, as
/// a modal does.
pub fn candidates<C: AsRef<Candidate>>(
    nodes: impl IntoIterator<Item = C>,
    scope: NodeId,
    layers: &[Layer],
) -> Vec<C> {
    let mut candidates: Vec<C> = nodes
        .into_iter()
        .filter(|node| {
            let node = node.as_ref();
            node.scope == scope && node.takes_focus()
        })
        .collect();
    candidates.sort_by_key(|node| node.as_ref().order);
    if let Some(cover) = covering_layer(layers) {
        candidates.retain(|node| {
            node.as_ref()
                .layer
                .and_then(|id| layers.iter().find(|layer| layer.id == id))
                .is_some_and(|layer| layer.order > cover.order)
        });
    }
    candidates
}

/// Where Tab (Shift+Tab with `back`) moves among `candidates` (in focus
/// order), round them: on from `current`, or, with nothing focused, from
/// `left` (the stop the focus last left for none), or to the first stop
/// after `resume` in the focus order (a search match's text, a field left
/// that is no stop), the last one before it with `back`.
pub fn tab<C: AsRef<Candidate>>(
    candidates: &[C],
    current: Option<usize>,
    left: Option<usize>,
    resume: Option<Order>,
    back: bool,
) -> Option<usize> {
    if candidates.is_empty() {
        return None;
    }
    let order = |node: &C| node.as_ref().order;
    match resume {
        Some(point) if current.is_none() => Some(if back {
            candidates
                .iter()
                .rposition(|node| order(node) < point)
                .unwrap_or(candidates.len() - 1)
        } else {
            candidates
                .iter()
                .position(|node| order(node) > point)
                .unwrap_or(0)
        }),
        _ => input::tab_stop(current.or(left), candidates.len(), back, true),
    }
}

/// Where Right (`right`) or Left moves from `current` among `candidates`:
/// the nearest, by left edge, of those to that side on a row that overlaps
/// its own (the first of equals), or nowhere at the end of the row; the
/// first with nothing focused.
pub fn horizontal<C: AsRef<Candidate>>(
    candidates: &[C],
    current: Option<usize>,
    right: bool,
) -> Option<usize> {
    let Some(index) = current else {
        return (!candidates.is_empty()).then_some(0);
    };
    let from = candidates.get(index)?.as_ref().frame;
    candidates
        .iter()
        .enumerate()
        .filter(|(_, to)| {
            let to = to.as_ref().frame;
            to.top() < from.bottom()
                && from.top() < to.bottom()
                && if right {
                    to.left() > from.left()
                } else {
                    to.left() < from.left()
                }
        })
        .min_by(|(_, a), (_, b)| {
            let distance = |to: &C| (to.as_ref().frame.left().0 - from.left().0).abs();
            compare(distance(a), distance(b))
        })
        .map(|(index, _)| index)
}

/// Where Down (`down`) or Up moves from `current` among `candidates`: the
/// nearest below / above, one in a column that overlaps its own first,
/// then by the gap, then by left edge (the first of equals). With nothing
/// focused, the first / last one in sight, inside a popup anchored to
/// another view (a menu or a select opened by a press) when one shows: it
/// holds what the arrows are for.
pub fn vertical<C: AsRef<Candidate>>(
    candidates: &[C],
    current: Option<usize>,
    down: bool,
) -> Option<usize> {
    let all = candidates.iter().map(AsRef::as_ref).enumerate();
    let Some(index) = current else {
        let popup = candidates
            .iter()
            .map(AsRef::as_ref)
            .any(|node| node.visible && node.popup);
        let shown = all.filter(|(_, node)| node.visible && (!popup || node.popup));
        let best = if down {
            shown.min_by(|(_, a), (_, b)| {
                compare(a.frame.top().0, b.frame.top().0)
                    .then_with(|| compare(a.frame.left().0, b.frame.left().0))
            })
        } else {
            shown.max_by(|(_, a), (_, b)| {
                compare(a.frame.bottom().0, b.frame.bottom().0)
                    .then_with(|| compare(b.frame.left().0, a.frame.left().0))
            })
        };
        return best.map(|(index, _)| index);
    };
    let from = candidates.get(index)?.as_ref();
    let at = from.frame;
    let key = |to: &Candidate| {
        let to = to.frame;
        let gap = if down {
            to.top().0 - at.bottom().0
        } else {
            at.top().0 - to.bottom().0
        };
        let overlaps = to.left() < at.right() && at.left() < to.right();
        (!overlaps, gap, (to.left().0 - at.left().0).abs())
    };
    all.filter(|(_, to)| {
        to.id != from.id
            && if down {
                to.frame.top() >= at.bottom()
            } else {
                to.frame.bottom() <= at.top()
            }
    })
    .min_by(|(_, a), (_, b)| {
        let (a, b) = (key(a), key(b));
        a.0.cmp(&b.0)
            .then_with(|| compare(a.1, b.1))
            .then_with(|| compare(a.2, b.2))
    })
    .map(|(index, _)| index)
}

/// How a scroll view brings the stop that takes the focus into sight along
/// its axis ([`reveal`]).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// As little as shows it, with the margin of
    /// [`scroll::reveal_delta`].
    #[default]
    Nearest,
    /// Centered in the viewport; a span longer than the viewport keeps
    /// [`scroll::reveal_delta`]'s margin and start-first rule.
    Center,
}

/// How far a scroll view moves along its axis to bring a stop that takes
/// the focus into sight: the span `start` .. `start + len` from the
/// viewport's start (its offset applied), in a `viewport` that long, all
/// in the layout's points (the margin is points); `None` while it shows
/// whole already.
pub fn reveal(start: Points, len: Points, viewport: Points, align: Align) -> Option<Points> {
    if start >= Points::ZERO && start + len <= viewport {
        return None;
    }
    Some(match align {
        Align::Nearest => scroll::reveal_delta(start, start + len, viewport),
        Align::Center => {
            let centered = (viewport - len) / 2.0;
            start - centered + scroll::reveal_delta(centered, centered + len, viewport)
        }
    })
}

/// One change of the focus to make, in order.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Move {
    /// The stop that takes the focus, or none.
    pub focus: Option<NodeId>,
    /// It comes into sight as it takes the focus ([`reveal`]).
    pub reveal: bool,
}

/// A modal the focus went into, with the focus it gives back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModalFocus {
    id: NodeId,
    /// The scope it was presented over.
    parent: NodeId,
    /// The focus when it was presented.
    restore: Option<NodeId>,
    /// It had no stop when presented: its first one takes the focus.
    initial: bool,
}

/// The modals the focus went into, oldest first, each with the focus it
/// gives back when it closes.
#[non_exhaustive]
#[derive(Clone, Debug, Default)]
pub struct Modals {
    stack: Vec<ModalFocus>,
}

impl Modals {
    /// Follows the modals that show, `active` (oldest first), with
    /// `focused` the focus now; `None` while they are the ones it follows,
    /// else the moves to make, in order. A modal presented takes the focus
    /// to its first stop (`first(modal)`, revealed), none while it has
    /// none. The newest modal closed gives it back to the stop it took it
    /// from, or, closed with the modals it was presented over, to the one
    /// the oldest of them took it from, when that is still a stop in sight
    /// in the scope it returns to (`available(scope, id)`), and to none
    /// otherwise.
    pub fn update(
        &mut self,
        active: &[NodeId],
        focused: Option<NodeId>,
        mut first: impl FnMut(NodeId) -> Option<NodeId>,
        mut available: impl FnMut(NodeId, NodeId) -> bool,
    ) -> Option<Vec<Move>> {
        if self
            .stack
            .iter()
            .map(|modal| modal.id)
            .eq(active.iter().copied())
        {
            return None;
        }
        let previous = std::mem::take(&mut self.stack);
        let mut focused = focused;
        let mut moves = Vec::new();
        let mut scope = previous.last().map_or(ROOT_ID, |modal| modal.id);
        if let Some(last) = previous.last()
            && !active.contains(&last.id)
        {
            let (restore, parent) = restoration(last.restore, last.parent, &previous, active);
            scope = parent;
            let restored = restore.filter(|&id| available(scope, id));
            moves.push(Move {
                focus: restored,
                reveal: false,
            });
            focused = restored;
        }
        for &id in active {
            if let Some(old) = previous.iter().find(|modal| modal.id == id) {
                let (restore, parent) = restoration(old.restore, old.parent, &previous, active);
                self.stack.push(ModalFocus {
                    restore,
                    parent,
                    ..*old
                });
            } else {
                let stop = first(id);
                self.stack.push(ModalFocus {
                    id,
                    parent: scope,
                    restore: focused,
                    initial: stop.is_none(),
                });
                moves.push(Move {
                    focus: stop,
                    reveal: stop.is_some(),
                });
                focused = stop;
                scope = id;
            }
        }
        Some(moves)
    }

    /// The stop the newest modal gives the focus to (and reveals) once it
    /// has one (`first`), when it had none when presented, has given none
    /// since and nothing has the focus (`focused`).
    pub fn first_stop(
        &mut self,
        focused: Option<NodeId>,
        first: impl FnOnce() -> Option<NodeId>,
    ) -> Option<NodeId> {
        if focused.is_some() {
            return None;
        }
        let modal = self.stack.last_mut().filter(|modal| modal.initial)?;
        let stop = first()?;
        modal.initial = false;
        Some(stop)
    }
}

/// The focus a modal gives back, and the scope it goes to: a modal closed
/// with the one it was presented over passes on that one's.
fn restoration(
    mut restore: Option<NodeId>,
    mut parent: NodeId,
    previous: &[ModalFocus],
    active: &[NodeId],
) -> (Option<NodeId>, NodeId) {
    while parent != ROOT_ID && !active.contains(&parent) {
        let Some(modal) = previous.iter().find(|modal| modal.id == parent) else {
            return (None, ROOT_ID);
        };
        restore = modal.restore;
        parent = modal.parent;
    }
    (restore, parent)
}

/// A layer the focus went into, with the stop it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LayerFocus {
    layer: NodeId,
    restore: NodeId,
}

/// The layers the focus went into (a root view's children: the page, a
/// sheet's panel, any overlay), each with the stop it came from: a layer
/// that goes gives the focus back there, as a modal does.
#[non_exhaustive]
#[derive(Clone, Debug, Default)]
pub struct Layers {
    entries: Vec<LayerFocus>,
}

impl Layers {
    /// The focus went into `layer` from stop `from`, or a backdrop
    /// `layer` covered it: `from` takes it back when the layer goes.
    pub fn entered(&mut self, layer: NodeId, from: NodeId) {
        self.entries.retain(|entry| entry.layer != layer);
        self.entries.push(LayerFocus {
            layer,
            restore: from,
        });
    }

    /// The stop the newest of the layers `gone` took the focus from,
    /// walking back while that stop is not `available` (a stop in sight).
    pub fn restore(
        &self,
        gone: impl Fn(NodeId) -> bool,
        available: impl Fn(NodeId) -> bool,
    ) -> Option<NodeId> {
        self.entries
            .iter()
            .rev()
            .filter(|entry| gone(entry.layer))
            .map(|entry| entry.restore)
            .find(|&id| available(id))
    }

    /// Forgets the layers that are not `kept`.
    pub fn retain(&mut self, kept: impl Fn(NodeId) -> bool) {
        self.entries.retain(|entry| kept(entry.layer));
    }
}

/// Lengths in order, with -0 and 0 equal (no NaN comes from a frame).
fn compare(a: f32, b: f32) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests;
