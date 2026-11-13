//! What a TV adds to the arrows on the web (`focus::decide` over
//! `focus::directional`, `focus::auto`, `focus::write_props`): the
//! preferred stop with nothing focused, `nextFocus*` routes, focus groups
//! entered at their destinations and trapping the arrows, a scroll view
//! that scrolls on before the focus leaves it, `autoFocus`, and the props
//! the page gets for them.

use nsh_core::{
    ViewProps,
    focus::{GroupConfig, NextFocus},
};
use serde_json::{Value, json};

use super::{page, stop};
use crate::focus::{Arrow, auto, decide, write_props};

/// What `key` does from `focused` among `stops`, with `extra` (groups,
/// scroll views, views) on the root's page.
fn arrow(key: &str, focused: Option<u32>, stops: &[Value], extra: Value) -> Option<Arrow> {
    let mut request =
        json!({ "key": key, "focused": focused, "modals": [], "stops": stops, "layers": page() });
    for (name, value) in extra.as_object().into_iter().flatten() {
        request[name] = value.clone();
    }
    decide(&request.to_string())
}

/// `stop` with `fields` set.
fn with(mut stop: Value, fields: Value) -> Value {
    for (name, value) in fields.as_object().into_iter().flatten() {
        stop[name] = value.clone();
    }
    stop
}

/// Three buttons in a row.
fn row() -> Vec<Value> {
    vec![
        stop(4, [10.0, 10.0, 80.0, 40.0]),
        stop(6, [110.0, 10.0, 80.0, 40.0]),
        stop(8, [210.0, 10.0, 80.0, 40.0]),
    ]
}

#[test]
fn with_nothing_focused_the_preferred_stop_takes_the_arrow() {
    let mut stops = row();
    stops[2] = with(stops[2].clone(), json!({ "preferred": true }));
    assert_eq!(
        arrow("down", None, &stops, json!({})),
        Some(Arrow::Focus(8))
    );
    assert_eq!(
        arrow("right", None, &stops, json!({})),
        Some(Arrow::Focus(8))
    );
    // Then the arrows move as ever.
    assert_eq!(
        arrow("left", Some(8), &stops, json!({})),
        Some(Arrow::Focus(6))
    );
    // One out of sight is not entered: the geometric first is.
    stops[2]["visible"] = json!(false);
    assert_eq!(
        arrow("right", None, &stops, json!({})),
        Some(Arrow::Focus(4))
    );
}

#[test]
fn a_route_takes_the_arrow_past_geometry() {
    let mut stops = row();
    stops.push(stop(10, [10.0, 100.0, 80.0, 40.0]));
    // Inside card 12, which is no stop.
    stops.push(stop(14, [300.0, 250.0, 80.0, 40.0]));
    stops[1] = with(
        stops[1].clone(),
        json!({ "next": { "down": 12, "up": 99 } }),
    );
    let views = json!({ "views": [[12, [14]]] });
    // Down goes to the stop inside the card named, not the nearest below.
    assert_eq!(
        arrow("down", Some(6), &stops, views.clone()),
        Some(Arrow::Focus(14))
    );
    // A route to a stop itself.
    stops[1]["next"] = json!({ "down": 10 });
    assert_eq!(
        arrow("down", Some(6), &stops, views.clone()),
        Some(Arrow::Focus(10))
    );
    // One naming nothing in reach leaves the arrow to geometry.
    stops[1]["next"] = json!({ "down": 99 });
    assert_eq!(
        arrow("down", Some(6), &stops, views.clone()),
        Some(Arrow::Focus(10))
    );
    // Without routes, the nearest.
    assert_eq!(arrow("up", Some(10), &stops, views), Some(Arrow::Focus(4)));
}

#[test]
fn a_trapping_group_keeps_the_arrows_inside() {
    let in_group = |id, x| with(stop(id, [x, 10.0, 80.0, 40.0]), json!({ "group": 20 }));
    let stops = vec![
        in_group(22, 10.0),
        in_group(24, 110.0),
        stop(26, [10.0, 100.0, 80.0, 40.0]),
    ];
    let group = |trap: bool| json!({ "groups": [{ "id": 20, "parent": null, "trap": trap, "destinations": [] }] });
    // Nothing below inside the group: the arrow goes nowhere.
    assert_eq!(arrow("down", Some(22), &stops, group(true)), None);
    assert_eq!(
        arrow("right", Some(22), &stops, group(true)),
        Some(Arrow::Focus(24))
    );
    // Open, it lets the arrow out.
    assert_eq!(
        arrow("down", Some(22), &stops, group(false)),
        Some(Arrow::Focus(26))
    );
    // A route of the app's leaves even a trap.
    let mut routed = stops.clone();
    routed[0]["next"] = json!({ "down": 26 });
    assert_eq!(
        arrow("down", Some(22), &routed, group(true)),
        Some(Arrow::Focus(26))
    );
}

#[test]
fn the_arrows_enter_a_group_at_its_destination() {
    let in_group = |id, x| with(stop(id, [x, 10.0, 80.0, 40.0]), json!({ "group": 20 }));
    let mut stops = vec![
        stop(4, [10.0, 100.0, 80.0, 40.0]),
        in_group(22, 10.0),
        in_group(24, 110.0),
        in_group(26, 210.0),
    ];
    let group = |destinations: Value| {
        json!({ "groups": [{ "id": 20, "parent": null, "trap": false, "destinations": destinations }],
                "views": [[30, [26]]] })
    };
    // Up reaches 22 by geometry; the group takes the focus to 26.
    assert_eq!(
        arrow("up", Some(4), &stops, group(json!([26]))),
        Some(Arrow::Focus(26))
    );
    // A destination that is a view holding a stop.
    assert_eq!(
        arrow("up", Some(4), &stops, group(json!([30]))),
        Some(Arrow::Focus(26))
    );
    // Inside it, the nearest.
    assert_eq!(
        arrow("left", Some(26), &stops, group(json!([26]))),
        Some(Arrow::Focus(24))
    );
    // Without destinations in reach, its preferred stop, else the nearest.
    assert_eq!(
        arrow("up", Some(4), &stops, group(json!([99]))),
        Some(Arrow::Focus(22))
    );
    stops[2]["preferred"] = json!(true);
    assert_eq!(
        arrow("up", Some(4), &stops, group(json!([]))),
        Some(Arrow::Focus(24))
    );
}

#[test]
fn a_scroll_view_that_can_scroll_on_keeps_the_focus() {
    // Rows 6 and 8 in scroll view 5, a button below it.
    let stops = vec![
        with(
            stop(6, [10.0, 10.0, 200.0, 40.0]),
            json!({ "scrollers": [5] }),
        ),
        with(
            stop(8, [10.0, 120.0, 200.0, 40.0]),
            json!({ "scrollers": [5] }),
        ),
        stop(10, [10.0, 250.0, 200.0, 40.0]),
    ];
    let view = |start: bool, end: bool| json!({ "scrollViews": [{ "id": 5, "horizontal": false, "start": start, "end": end }] });
    // A stop inside it is reached as ever.
    assert_eq!(
        arrow("down", Some(6), &stops, view(false, true)),
        Some(Arrow::Focus(8))
    );
    // More content below: it scrolls on, the focus stays in it.
    assert_eq!(
        arrow("down", Some(8), &stops, view(true, true)),
        Some(Arrow::Scroll(5))
    );
    // At its end the arrow leaves it.
    assert_eq!(
        arrow("down", Some(8), &stops, view(true, false)),
        Some(Arrow::Focus(10))
    );
    // Up at its start: nothing above, nothing to scroll.
    assert_eq!(arrow("up", Some(6), &stops, view(false, true)), None);
    assert_eq!(
        arrow("up", Some(6), &stops, view(true, true)),
        Some(Arrow::Scroll(5))
    );
    // A scroll view across the arrow's axis is not the one that scrolls.
    let across =
        json!({ "scrollViews": [{ "id": 5, "horizontal": true, "start": true, "end": true }] });
    assert_eq!(
        arrow("down", Some(8), &stops, across),
        Some(Arrow::Focus(10))
    );
    // Undescribed, it does not scroll.
    assert_eq!(
        arrow("down", Some(8), &stops, json!({})),
        Some(Arrow::Focus(10))
    );
}

#[test]
fn auto_focus_is_the_first_stop_not_seen_in_the_scope() {
    let mut stops = row();
    stops[1]["auto"] = json!(true);
    stops[2]["auto"] = json!(true);
    // In a modal that shows: the page's are out of scope.
    let in_modal = with(
        stop(32, [10.0, 200.0, 80.0, 40.0]),
        json!({ "scope": 30, "layer": 31, "auto": true }),
    );
    let request = |stops: &[Value], modals: Value, seen: Value| {
        json!({ "focused": null, "modals": modals, "stops": stops, "seen": seen,
                "layers": [[1, [{ "id": 2, "frame": [0, 0, 800, 600], "presses": false }]],
                           [30, [{ "id": 31, "frame": [0, 0, 800, 600], "presses": false }]]] })
        .to_string()
    };
    assert_eq!(auto(&request(&stops, json!([]), json!([]))), Some(6));
    assert_eq!(auto(&request(&stops, json!([]), json!([6]))), Some(8));
    assert_eq!(auto(&request(&stops, json!([]), json!([6, 8]))), None);
    stops.push(in_modal);
    assert_eq!(
        auto(&request(&stops, json!([[30, true]]), json!([]))),
        Some(32)
    );
    assert_eq!(
        auto(&request(&stops, json!([[30, false]]), json!([6]))),
        Some(8)
    );
    assert_eq!(auto("not json"), None);
}

#[test]
fn the_page_gets_the_focus_props_of_a_view() {
    let json = |props: &ViewProps| {
        let mut s = String::new();
        write_props(&mut s, props);
        s
    };
    assert_eq!(json(&ViewProps::default()), "");
    let mut next = NextFocus::default();
    next.down = Some(12);
    next.left = Some(4);
    let mut group = GroupConfig::default();
    group.trap = true;
    group.destinations = vec![26, 30];
    let props = ViewProps {
        auto_focus: true,
        preferred_focus: true,
        next_focus: next,
        focus_group: Some(group),
        ..ViewProps::default()
    };
    let written: Value = serde_json::from_str(&format!("{{\"a\":0{}}}", json(&props))).unwrap();
    assert_eq!(
        written,
        json!({ "a": 0, "autoFocus": true, "preferredFocus": true,
                "nextFocus": { "down": 12, "left": 4 },
                "focusGroup": { "trap": true, "destinations": [26, 30] } })
    );
    let open = ViewProps {
        focus_group: Some(GroupConfig::default()),
        ..ViewProps::default()
    };
    assert_eq!(
        json(&open),
        ",\"focusGroup\":{\"trap\":false,\"destinations\":[]}"
    );
}
