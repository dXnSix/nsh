//! The decisions of keyboard focus the backends share, over plain data:
//! the scope the focus moves in ([`focus_scope`]: the newest modal that
//! shows), its stops there in focus order ([`candidates`], without those a
//! backdrop over the page takes out of reach: [`covering_layer`]), where
//! Tab, Left / Right and Up / Down move ([`tab`], [`horizontal`],
//! [`vertical`]), how far a scroll view moves to bring the stop that takes
//! the focus into sight ([`reveal`]), and which stop takes the focus back
//! when a modal or a layer goes ([`Modals`], [`Layers`]).
//!
//! What a TV adds, over the same data ([`directional`]): the stop that
//! takes the focus as it appears (`autoFocus`, [`auto_focus`]) or where
//! the focus enters (`preferredFocus`), the app's own routes
//! (`nextFocusUp` / `Down` / `Left` / `Right`, [`NextFocus`]), focus
//! groups that are entered at their destinations and may trap the arrows
//! ([`Group`], `nsh/base`'s `FocusGroup`), and the rows of a list past
//! those mounted, which the list scrolls to by index before the focus
//! lands there ([`Target::Scroll`], [`next_row`]).
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
    /// It takes the focus when it appears (`autoFocus`, [`auto_focus`]).
    pub auto_focus: bool,
    /// It wins over geometry where the focus enters (`preferredFocus`): a
    /// focus group it is in, or a scope with nothing focused.
    pub preferred: bool,
    /// The stops the arrows go to from it instead of the nearest one
    /// (`nextFocusUp` / `Down` / `Left` / `Right`), as the backend found
    /// them: the stop each names, or the first stop inside the view it
    /// names.
    pub next: NextFocus,
    /// The innermost focus group around it (a `FocusGroup`'s view,
    /// [`Group`]).
    pub group: Option<NodeId>,
    /// The innermost scroll view or list around it.
    pub scroller: Option<NodeId>,
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

/// An arrow key, the way it moves the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
}

impl Direction {
    /// Left or Right.
    pub fn horizontal(self) -> bool {
        matches!(self, Direction::Left | Direction::Right)
    }

    /// Down or Right: toward the end of the content.
    pub fn forward(self) -> bool {
        matches!(self, Direction::Down | Direction::Right)
    }

    /// The arrow the other way.
    pub fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

/// The views the arrows take the focus to from a view instead of the
/// nearest stop (`nextFocusUp`, `nextFocusDown`, `nextFocusLeft`,
/// `nextFocusRight`): explicit routes, as a TV's remote needs between
/// rows that do not line up.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NextFocus {
    /// Where Up goes.
    pub up: Option<NodeId>,
    /// Where Down goes.
    pub down: Option<NodeId>,
    /// Where Left goes.
    pub left: Option<NodeId>,
    /// Where Right goes.
    pub right: Option<NodeId>,
}

impl NextFocus {
    /// Where the arrow toward `direction` goes, if anywhere.
    pub fn toward(&self, direction: Direction) -> Option<NodeId> {
        match direction {
            Direction::Up => self.up,
            Direction::Down => self.down,
            Direction::Left => self.left,
            Direction::Right => self.right,
        }
    }

    /// The route toward `direction`, set or cleared.
    pub fn set(&mut self, direction: Direction, to: Option<NodeId>) {
        match direction {
            Direction::Up => self.up = to,
            Direction::Down => self.down = to,
            Direction::Left => self.left = to,
            Direction::Right => self.right = to,
        }
    }

    /// No route is set.
    pub fn is_empty(&self) -> bool {
        *self == NextFocus::default()
    }
}

/// What a focus group's view carries (`nsh/base`'s `FocusGroup`), its
/// views found: in [`crate::ViewProps::focus_group`].
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupConfig {
    /// The arrows do not take the focus out of the group.
    pub trap: bool,
    /// The views the focus enters the group at, in order: the first one
    /// that is (or holds) a stop in reach wins.
    pub destinations: Vec<NodeId>,
}

/// A focus group as a backend describes it: a `FocusGroup`'s view, the
/// group around it, and what it carries ([`GroupConfig`]), its
/// destinations as stops.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    /// The group's view.
    pub id: NodeId,
    /// The group around it, if any.
    pub parent: Option<NodeId>,
    /// The arrows do not take the focus out of it.
    pub trap: bool,
    /// The stops it enters at, in order: the stop each destination names,
    /// or the first stop inside it.
    pub destinations: Vec<NodeId>,
}

/// Whether `group` is `inner` or a group around it, among `groups`.
pub fn in_group(groups: &[Group], inner: Option<NodeId>, group: NodeId) -> bool {
    let mut at = inner;
    // A parent chain the backend made in a loop ends after every group.
    for _ in 0..=groups.len() {
        let Some(id) = at else { return false };
        if id == group {
            return true;
        }
        at = groups.iter().find(|g| g.id == id).and_then(|g| g.parent);
    }
    false
}

/// The groups from `inner` outwards, innermost first.
fn chain(groups: &[Group], inner: Option<NodeId>) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut at = inner;
    while let Some(id) = at {
        if out.contains(&id) {
            break;
        }
        out.push(id);
        at = groups.iter().find(|g| g.id == id).and_then(|g| g.parent);
    }
    out
}

/// Where an arrow takes the focus ([`directional`]).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The candidate at this index takes it.
    Stop(usize),
    /// The scroll view or list around the focused stop (its
    /// [`Candidate::scroller`]) scrolls on toward the arrow first, and
    /// keeps the focus: what comes into sight there takes it on a later
    /// arrow, and a list's next row takes it as soon as it is mounted
    /// ([`next_row`], `Core::focus_past`).
    Scroll(NodeId),
    /// Nowhere: the key goes on.
    None,
}

/// Where an arrow toward `direction` takes the focus from `current` among
/// `candidates` (in focus order), the TV's rules over the desktops' ones
/// ([`horizontal`], [`vertical`]), first to last:
///
/// - **Nothing focused**: the first `preferred` stop in sight (in an open
///   popup anchored to another view when one shows), else the geometric
///   first ([`horizontal`], [`vertical`]).
/// - **Overrides**: the stop `current`'s [`NextFocus`] names toward the
///   arrow, when it is one of `candidates`; one that names none (gone, out
///   of reach) leaves the arrow to the rules below. An override is the
///   app's own route: it may leave a trapped group.
/// - **Trap**: inside a group of `groups` that traps (the innermost one
///   around `current` that does), only the stops inside it are reached:
///   an arrow with nothing there goes nowhere.
/// - **Scroll first**: while the scroll view or list around `current`
///   (`Candidate::scroller`) can scroll on toward the arrow
///   (`can_scroll`), a stop outside it (`inside(scroller, stop)` false),
///   or none, is not reached: it scrolls on instead ([`Target::Scroll`]),
///   so the rows past those mounted are not skipped.
/// - **Entering a group**: a stop reached in a group `current` is not in
///   hands the focus to the outermost such group's first destination in
///   reach, else to its first `preferred` stop, else keeps it.
pub fn directional<C: AsRef<Candidate>>(
    candidates: &[C],
    current: Option<usize>,
    direction: Direction,
    groups: &[Group],
    can_scroll: impl Fn(NodeId) -> bool,
    inside: impl Fn(NodeId, NodeId) -> bool,
) -> Target {
    let node = |index: usize| candidates[index].as_ref();
    let position = |id: NodeId| candidates.iter().position(|c| c.as_ref().id == id);
    let Some(at) = current.filter(|&at| at < candidates.len()) else {
        // An open popup holds what the arrows are for, as in `vertical`.
        let popup = candidates
            .iter()
            .any(|c| c.as_ref().visible && c.as_ref().popup);
        if let Some(preferred) = candidates
            .iter()
            .map(AsRef::as_ref)
            .position(|c| c.preferred && c.visible && (!popup || c.popup))
        {
            return Target::Stop(preferred);
        }
        let first = if direction.horizontal() {
            horizontal(candidates, None, direction.forward())
        } else {
            vertical(candidates, None, direction.forward())
        };
        return first.map_or(Target::None, Target::Stop);
    };
    let from = *node(at);
    if let Some(to) = from.next.toward(direction).and_then(position)
        && to != at
    {
        return Target::Stop(to);
    }
    // The stops in reach: those of the innermost trapping group around it.
    let own = chain(groups, from.group);
    let trap = own
        .iter()
        .copied()
        .find(|&id| groups.iter().any(|g| g.id == id && g.trap));
    let pool: Vec<usize> = (0..candidates.len())
        .filter(|&i| trap.is_none_or(|g| in_group(groups, node(i).group, g)))
        .collect();
    let reach: Vec<&Candidate> = pool.iter().map(|&i| node(i)).collect();
    let here = pool.iter().position(|&i| i == at);
    let found = if direction.horizontal() {
        horizontal(&reach, here, direction.forward())
    } else {
        vertical(&reach, here, direction.forward())
    }
    .map(|i| pool[i])
    .filter(|&i| i != at);
    if let Some(scroller) = from.scroller
        && found.is_none_or(|to| !inside(scroller, node(to).id))
        && can_scroll(scroller)
    {
        return Target::Scroll(scroller);
    }
    let Some(to) = found else {
        return Target::None;
    };
    // The outermost group the stop reached is in and `current` is not.
    let entered = chain(groups, node(to).group)
        .into_iter()
        .rev()
        .find(|id| !own.contains(id));
    let Some(group) = entered.and_then(|id| groups.iter().find(|g| g.id == id)) else {
        return Target::Stop(to);
    };
    let member = |i: &usize| in_group(groups, node(*i).group, group.id);
    let destination = group
        .destinations
        .iter()
        .find_map(|&id| position(id).filter(|i| member(i) && pool.contains(i)));
    let preferred = || {
        pool.iter()
            .copied()
            .find(|i| member(i) && node(*i).preferred)
    };
    Target::Stop(destination.or_else(preferred).unwrap_or(to))
}

/// The stop of `candidates` (in focus order) that takes the focus as it
/// appears (`autoFocus`): the first that does and is not `seen` (one
/// already given the chance when it appeared, or that a backend left to
/// the user since).
pub fn auto_focus<C: AsRef<Candidate>>(
    candidates: &[C],
    seen: impl Fn(NodeId) -> bool,
) -> Option<usize> {
    candidates
        .iter()
        .map(AsRef::as_ref)
        .position(|c| c.auto_focus && !seen(c.id))
}

/// The stop a scope opened with nothing focused gives the focus to (a
/// modal presented, [`Modals::update`]'s `first`): the first that takes it
/// as it appears (`autoFocus`), else the first `preferred` one, else the
/// first.
pub fn first_stop<C: AsRef<Candidate>>(candidates: &[C]) -> Option<usize> {
    let all = || candidates.iter().map(AsRef::as_ref);
    all()
        .position(|c| c.auto_focus)
        .or_else(|| all().position(|c| c.preferred))
        .or((!candidates.is_empty()).then_some(0))
}

/// A row of a list, as the core knows it: what an arrow from a stop in it
/// moves along ([`next_row`]).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// The list.
    pub list: NodeId,
    /// The row's item.
    pub index: u32,
    /// How many items the list has.
    pub count: u32,
    /// Items per row (`numColumns`, at least 1).
    pub columns: u32,
    /// The list scrolls horizontally.
    pub horizontal: bool,
}

/// The item an arrow toward `direction` reaches from `row` along its list:
/// the next one along the list's axis (a row of columns further, in a
/// list of several), the one beside it across it in a list of columns, or
/// none at the ends and across a list of one column (the arrow leaves the
/// list there by geometry).
pub fn next_row(row: &Row, direction: Direction) -> Option<u32> {
    let columns = row.columns.max(1);
    let along = direction.horizontal() == row.horizontal;
    let step = if along { columns } else { 1 };
    if !along
        && (columns == 1 || {
            let column = row.index % columns;
            if direction.forward() {
                column + 1 == columns
            } else {
                column == 0
            }
        })
    {
        return None;
    }
    let to = if direction.forward() {
        row.index.checked_add(step)?
    } else {
        row.index.checked_sub(step)?
    };
    (to < row.count).then_some(to)
}

/// A list row an arrow went to before it was mounted: the list scrolls
/// to it, and its first stop takes the focus once it is mounted.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PendingRow {
    /// The list.
    pub list: NodeId,
    /// The item.
    pub index: u32,
    /// The stop the arrow left: the row is forgotten once the focus is
    /// elsewhere.
    pub from: NodeId,
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
