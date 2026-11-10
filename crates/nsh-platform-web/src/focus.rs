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
//! Depends on nothing of the browser: `tests/text` builds it on the host.

use nsh_core::{
    Bounds, NodeId, Points,
    focus::{self, Align, Candidate, Layer, Order},
    geometry::{point, size},
};
use serde_json::Value;

/// A stop as the page describes it, with what only this backend's rules
/// read beside the core's candidate.
#[derive(Clone, Copy, Debug)]
struct Stop {
    candidate: Candidate,
    /// The overlay anchored to another view it is inside (a menu's, a
    /// select's or a popover's popup), if any.
    popup: Option<NodeId>,
    /// It is a menu's item (role `menuitem`, `menuitemcheckbox` or
    /// `menuitemradio`).
    menu: bool,
}

impl AsRef<Candidate> for Stop {
    fn as_ref(&self) -> &Candidate {
        &self.candidate
    }
}

/// Where an arrow key moves the keyboard focus: the stop that takes it, or
/// none (the key goes on: the page scrolls with it). `json` is the page's
/// description:
///
/// ```json
/// { "key": "left" | "right" | "up" | "down",
///   "focused": 12,
///   "modals": [[30, true]],
///   "stops": [{ "id": 12, "frame": [x, y, width, height], "visible": true,
///               "editable": false, "scope": 1, "layer": 4, "popup": null,
///               "menu": false }],
///   "layers": [[1, [{ "id": 4, "frame": [x, y, width, height], "presses": false }]]] }
/// ```
///
/// `modals` are the presented ones, oldest first, each with whether it
/// shows; `stops` the focus stops in DOM order (their focus order), the
/// `focused` one among them when a stop has the focus; `layers` each root
/// view's (the root's, each modal's) children in order, `frame` null while
/// none of one is in sight. With the focus on a menu's item in a popup, the
/// arrows stay in the popup (the terminal's rule): Left and Right on an item
/// its menu does not take would leave the menu open behind the focus.
pub(crate) fn decide(json: &str) -> Option<NodeId> {
    let request: Value = serde_json::from_str(json).ok()?;
    let key = request.get("key")?.as_str()?;
    let focused = request.get("focused").and_then(id);
    let modals: Vec<(NodeId, bool)> = list(&request, "modals")
        .iter()
        .filter_map(|modal| {
            let modal = modal.as_array()?;
            Some((id(modal.first()?)?, modal.get(1)?.as_bool()?))
        })
        .collect();
    let ids: Vec<NodeId> = modals.iter().map(|&(id, _)| id).collect();
    let scope = focus::focus_scope(&ids, |id| modals.contains(&(id, true)));
    let layers: Vec<Layer> = list(&request, "layers")
        .iter()
        .filter_map(|entry| {
            let entry = entry.as_array()?;
            (id(entry.first()?)? == scope).then(|| layers(entry.get(1)))
        })
        .next()
        .unwrap_or_default();
    let stops = list(&request, "stops")
        .iter()
        .enumerate()
        .filter_map(|(order, value)| stop(value, order));
    let mut candidates = focus::candidates(stops, scope, &layers);
    let current = candidates
        .iter()
        .position(|stop| Some(stop.candidate.id) == focused);
    if let Some(popup) = current
        .map(|index| candidates[index])
        .filter(|stop| stop.menu)
        .and_then(|stop| stop.popup)
    {
        candidates.retain(|stop| stop.popup == Some(popup));
    }
    let current = candidates
        .iter()
        .position(|stop| Some(stop.candidate.id) == focused);
    let index = match key {
        "left" | "right" => focus::horizontal(&candidates, current, key == "right"),
        "up" | "down" => focus::vertical(&candidates, current, key == "down"),
        _ => None,
    }?;
    Some(candidates[index].candidate.id)
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
    Some(Stop {
        candidate,
        popup,
        menu: flag(value, "menu"),
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
