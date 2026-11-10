//! The arrow keys' focus moves of the page (`web/nsh.js` `arrowFocus`),
//! decided by [`nsh_core::focus`] as on the terminal and GPUI backends.
//!
//! The page describes what it renders ([`decide`]'s JSON): the focus stops
//! with their DOM rectangles (`getBoundingClientRect`, CSS pixels, which
//! the core's rules take as they are: they compare edges and distances
//! along one axis at a time), each one's input scope (the `Dialog` it is
//! in, or the root) and layer (the child of that root view around it), the
//! root views' layers and the modals. The core picks the scope (the newest
//! modal that shows), leaves out the layers under a covering backdrop and
//! decides where the arrow goes; the page focuses the stop and brings it
//! into sight, each scroll view around it by [`reveal`]'s delta.
//!
//! What a TV adds is decided here too ([`focus::directional`]), from the
//! props the page has of each view ([`write_props`]): `preferredFocus`,
//! the `nextFocus*` routes, the focus groups (`FocusGroup`), and the scroll
//! views around each stop, which scroll on before the focus leaves them
//! ([`Arrow::Scroll`]: a list goes on to its next row by index,
//! `EngineControl::focus_past`); `autoFocus` picks the stop that takes the
//! focus as it appears ([`auto`]). Without those props the moves are the
//! desktops' ([`focus::horizontal`], [`focus::vertical`]).
//!
//! Depends on nothing of the browser: `tests/text` builds it on the host.

use std::fmt::Write as _;

use nsh_core::{
    Bounds, NodeId, Points, ViewProps,
    focus::{self, Align, Candidate, Direction, Group, Layer, NextFocus, Order, Target},
    geometry::{point, size},
};
use serde_json::Value;

/// A stop as the page describes it, with what only this backend's rules
/// read beside the core's candidate.
#[derive(Clone, Debug)]
struct Stop {
    candidate: Candidate,
    /// The overlay anchored to another view it is inside (a menu's, a
    /// select's or a popover's popup), if any.
    popup: Option<NodeId>,
    /// It is a menu's item (role `menuitem`, `menuitemcheckbox` or
    /// `menuitemradio`).
    menu: bool,
    /// The scroll views around it, innermost first.
    scrollers: Vec<NodeId>,
}

impl AsRef<Candidate> for Stop {
    fn as_ref(&self) -> &Candidate {
        &self.candidate
    }
}

/// A scroll view around a stop, as the page describes it.
#[derive(Clone, Copy, Debug)]
struct ScrollView {
    id: NodeId,
    horizontal: bool,
    /// It can scroll on toward its start (up, left).
    start: bool,
    /// It can scroll on toward its end (down, right).
    end: bool,
}

/// What an arrow key does ([`decide`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arrow {
    /// The stop takes the focus.
    Focus(NodeId),
    /// The scroll view or list around the focused stop scrolls on toward
    /// the arrow and keeps the focus ([`Target::Scroll`]); the engine hears
    /// `focus_past`, so a list scrolls to its next row by index and that
    /// row's first stop takes the focus once it is mounted.
    Scroll(NodeId),
}

/// The page's description, read: the stops of the focus scope in focus
/// order, the focus groups and the scroll views around the stops.
struct Page {
    focused: Option<NodeId>,
    stops: Vec<Stop>,
    groups: Vec<Group>,
    scroll_views: Vec<ScrollView>,
}

impl Page {
    /// Reads `request`; `axis` is the arrow's (`true`: across), whose
    /// scroll views are the stops' scrollers, when an arrow moves.
    fn read(request: &Value, axis: Option<bool>) -> Page {
        let focused = request.get("focused").and_then(id);
        let modals: Vec<(NodeId, bool)> = list(request, "modals")
            .iter()
            .filter_map(|modal| {
                let modal = modal.as_array()?;
                Some((id(modal.first()?)?, modal.get(1)?.as_bool()?))
            })
            .collect();
        let ids: Vec<NodeId> = modals.iter().map(|&(id, _)| id).collect();
        let scope = focus::focus_scope(&ids, |id| modals.contains(&(id, true)));
        let layers: Vec<Layer> = list(request, "layers")
            .iter()
            .filter_map(|entry| {
                let entry = entry.as_array()?;
                (id(entry.first()?)? == scope).then(|| layers(entry.get(1)))
            })
            .next()
            .unwrap_or_default();
        let scroll_views: Vec<ScrollView> = list(request, "scrollViews")
            .iter()
            .filter_map(scroll_view)
            .collect();
        let stops = list(request, "stops")
            .iter()
            .enumerate()
            .filter_map(|(order, value)| stop(value, order));
        let mut stops = focus::candidates(stops, scope, &layers);
        for stop in &mut stops {
            stop.candidate.scroller = axis.and_then(|axis| {
                stop.scrollers.iter().copied().find(|&id| {
                    scroll_views
                        .iter()
                        .any(|view| view.id == id && view.horizontal == axis)
                })
            });
        }
        // Routes and destinations name views: the stops they reach.
        let inside: Vec<(NodeId, Vec<NodeId>)> = list(request, "views")
            .iter()
            .filter_map(|entry| {
                let entry = entry.as_array()?;
                let stops = entry.get(1)?.as_array()?.iter().filter_map(id).collect();
                Some((id(entry.first()?)?, stops))
            })
            .collect();
        let reached = |stops: &[Stop], view: NodeId| {
            if stops.iter().any(|stop| stop.candidate.id == view) {
                return Some(view);
            }
            let (_, inside) = inside.iter().find(|(id, _)| *id == view)?;
            inside
                .iter()
                .copied()
                .find(|&id| stops.iter().any(|stop| stop.candidate.id == id))
        };
        let routes: Vec<NextFocus> = stops
            .iter()
            .map(|stop| {
                let mut next = NextFocus::default();
                for direction in DIRECTIONS {
                    let to = stop.candidate.next.toward(direction);
                    next.set(direction, to.and_then(|view| reached(&stops, view)));
                }
                next
            })
            .collect();
        for (stop, next) in stops.iter_mut().zip(routes) {
            stop.candidate.next = next;
        }
        let groups = list(request, "groups")
            .iter()
            .filter_map(|value| {
                let mut group = Group::default();
                group.id = id(value.get("id")?)?;
                group.parent = value.get("parent").and_then(id);
                group.trap = flag(value, "trap");
                group.destinations = list(value, "destinations")
                    .iter()
                    .filter_map(id)
                    .filter_map(|view| reached(&stops, view))
                    .collect();
                Some(group)
            })
            .collect();
        Page {
            focused,
            stops,
            groups,
            scroll_views,
        }
    }
}

const DIRECTIONS: [Direction; 4] = [
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
];

/// What an arrow key does: the stop that takes the focus, the scroll view
/// that scrolls on instead, or nothing (the key goes on: the page scrolls
/// with it). `json` is the page's description:
///
/// ```json
/// { "key": "left" | "right" | "up" | "down",
///   "focused": 12,
///   "modals": [[30, true]],
///   "stops": [{ "id": 12, "frame": [x, y, width, height], "visible": true,
///               "editable": false, "scope": 1, "layer": 4, "popup": null,
///               "menu": false, "auto": false, "preferred": false,
///               "next": { "down": 20 }, "group": 8, "scrollers": [5] }],
///   "layers": [[1, [{ "id": 4, "frame": [x, y, width, height], "presses": false }]]],
///   "groups": [{ "id": 8, "parent": null, "trap": false, "destinations": [14] }],
///   "scrollViews": [{ "id": 5, "horizontal": false, "start": false, "end": true }],
///   "views": [[20, [22, 24]]] }
/// ```
///
/// `modals` are the presented ones, oldest first, each with whether it
/// shows; `stops` the focus stops in DOM order (their focus order), the
/// `focused` one among them when a stop has the focus; `layers` each root
/// view's (the root's, each modal's) children in order, `frame` null while
/// none of one is in sight. With the focus on a menu's item in a popup, the
/// arrows stay in the popup (the terminal's rule): Left and Right on an item
/// its menu does not take would leave the menu open behind the focus.
///
/// A stop's `auto` and `preferred` are its `autoFocus` and
/// `preferredFocus`, or a view's around it; `next` its `nextFocus*`
/// routes (its own over those of the views around it), by view; `group`
/// the innermost focus group around it and `scrollers` the scroll views
/// around it, innermost first. `groups` are the focus groups, each with
/// the group around it and its destinations, by view; `scrollViews` the
/// scroll views, each with its axis and whether it can scroll on toward
/// its start and its end; `views` the stops inside each view a route or a
/// destination names, in DOM order. A view named reaches itself when it is
/// a stop, else the first of those that is. All of these may be left out.
pub(crate) fn decide(json: &str) -> Option<Arrow> {
    let request: Value = serde_json::from_str(json).ok()?;
    let direction = match request.get("key")?.as_str()? {
        "left" => Direction::Left,
        "right" => Direction::Right,
        "up" => Direction::Up,
        "down" => Direction::Down,
        _ => return None,
    };
    let page = Page::read(&request, Some(direction.horizontal()));
    let focused = page.focused;
    let mut candidates = page.stops;
    let current = candidates
        .iter()
        .position(|stop| Some(stop.candidate.id) == focused);
    if let Some(popup) = current
        .map(|index| &candidates[index])
        .filter(|stop| stop.menu)
        .and_then(|stop| stop.popup)
    {
        candidates.retain(|stop| stop.popup == Some(popup));
    }
    let current = candidates
        .iter()
        .position(|stop| Some(stop.candidate.id) == focused);
    let target = focus::directional(
        &candidates,
        current,
        direction,
        &page.groups,
        |scroller| {
            page.scroll_views
                .iter()
                .find(|view| view.id == scroller)
                .is_some_and(|view| {
                    if direction.forward() {
                        view.end
                    } else {
                        view.start
                    }
                })
        },
        |scroller, id| {
            candidates
                .iter()
                .find(|stop| stop.candidate.id == id)
                .is_some_and(|stop| stop.scrollers.contains(&scroller))
        },
    );
    match target {
        Target::Stop(index) if Some(index) != current => {
            Some(Arrow::Focus(candidates[index].candidate.id))
        }
        Target::Scroll(scroller) => Some(Arrow::Scroll(scroller)),
        _ => None,
    }
}

/// The stop that takes the focus as it appears (`autoFocus`,
/// [`focus::auto_focus`]): the first of the focus scope's stops with
/// `auto` that is not in the request's `seen` (the stops given their
/// chance already). `json` is [`decide`]'s description, without `key`,
/// with `seen`.
pub(crate) fn auto(json: &str) -> Option<NodeId> {
    let request: Value = serde_json::from_str(json).ok()?;
    let page = Page::read(&request, None);
    let seen: Vec<NodeId> = list(&request, "seen").iter().filter_map(id).collect();
    focus::auto_focus(&page.stops, |id| seen.contains(&id)).map(|i| page.stops[i].candidate.id)
}

/// Writes the focus props of a view that the arrows read into the page's
/// props JSON (`op_vp`), each only when set: `autoFocus`, `preferredFocus`,
/// `nextFocus` (`{"up": node, ...}`) and `focusGroup` (`{"trap": bool,
/// "destinations": [node, ...]}`).
pub(crate) fn write_props(s: &mut String, p: &ViewProps) {
    if p.auto_focus {
        s.push_str(",\"autoFocus\":true");
    }
    if p.preferred_focus {
        s.push_str(",\"preferredFocus\":true");
    }
    if !p.next_focus.is_empty() {
        s.push_str(",\"nextFocus\":{");
        let mut first = true;
        for (direction, name) in DIRECTIONS.into_iter().zip(["up", "down", "left", "right"]) {
            if let Some(to) = p.next_focus.toward(direction) {
                let _ = write!(s, "{}\"{name}\":{to}", if first { "" } else { "," });
                first = false;
            }
        }
        s.push('}');
    }
    if let Some(group) = &p.focus_group {
        let _ = write!(
            s,
            ",\"focusGroup\":{{\"trap\":{},\"destinations\":[",
            group.trap
        );
        for (i, to) in group.destinations.iter().enumerate() {
            let _ = write!(s, "{}{to}", if i == 0 { "" } else { "," });
        }
        s.push_str("]}");
    }
}

/// How far a scroll view moves along its axis to bring the stop that took
/// the focus into sight ([`focus::reveal`]): the stop's span starts `start`
/// from the viewport's start and is `len` long, in a `viewport` that long,
/// all in CSS pixels (the layout's points). Centered along a vertical
/// view, as little as shows it along a horizontal one, as on the terminal;
/// `None` while it shows whole already.
pub(crate) fn reveal(start: f32, len: f32, viewport: f32, vertical: bool) -> Option<f32> {
    let align = if vertical {
        Align::Center
    } else {
        Align::Nearest
    };
    focus::reveal(Points(start), Points(len), Points(viewport), align).map(|delta| delta.0)
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn id(value: &Value) -> Option<NodeId> {
    value.as_u64().and_then(|id| NodeId::try_from(id).ok())
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// `[x, y, width, height]`.
fn frame(value: &Value) -> Option<Bounds<Points>> {
    let numbers = value.as_array()?;
    let n = |i: usize| {
        numbers
            .get(i)
            .and_then(Value::as_f64)
            .map(|v| Points(v as f32))
    };
    Some(Bounds::new(point(n(0)?, n(1)?), size(n(2)?, n(3)?)))
}

fn stop(value: &Value, order: usize) -> Option<Stop> {
    let mut candidate = Candidate::default();
    candidate.id = id(value.get("id")?)?;
    candidate.frame = frame(value.get("frame")?)?;
    candidate.visible = flag(value, "visible");
    candidate.stop = true;
    candidate.editable = flag(value, "editable");
    candidate.scope = id(value.get("scope")?)?;
    candidate.layer = value.get("layer").and_then(id);
    let popup = value.get("popup").and_then(id);
    candidate.popup = popup.is_some();
    candidate.order = Order::view(order);
    candidate.auto_focus = flag(value, "auto");
    candidate.preferred = flag(value, "preferred");
    candidate.group = value.get("group").and_then(id);
    if let Some(next) = value.get("next") {
        for (direction, name) in DIRECTIONS.into_iter().zip(["up", "down", "left", "right"]) {
            candidate.next.set(direction, next.get(name).and_then(id));
        }
    }
    Some(Stop {
        candidate,
        popup,
        menu: flag(value, "menu"),
        scrollers: list(value, "scrollers").iter().filter_map(id).collect(),
    })
}

fn scroll_view(value: &Value) -> Option<ScrollView> {
    Some(ScrollView {
        id: id(value.get("id")?)?,
        horizontal: flag(value, "horizontal"),
        start: flag(value, "start"),
        end: flag(value, "end"),
    })
}

fn layers(value: Option<&Value>) -> Vec<Layer> {
    value
        .and_then(Value::as_array)
        .map(|layers| {
            layers
                .iter()
                .enumerate()
                .filter_map(|(order, value)| {
                    let mut layer = Layer::default();
                    layer.id = id(value.get("id")?)?;
                    layer.order = order;
                    layer.frame = value.get("frame").and_then(frame);
                    layer.takes_presses = flag(value, "presses");
                    Some(layer)
                })
                .collect()
        })
        .unwrap_or_default()
}
