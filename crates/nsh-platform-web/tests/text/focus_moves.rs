//! The arrow keys' focus moves (`focus::decide`, `nsh_focus_move`): the
//! page's description of its stops in CSS pixels reaches `nsh_core::focus`,
//! and the stop it picks comes back; `focus::reveal` (`nsh_focus_reveal`)
//! is the core's reveal delta.

use serde_json::{Value, json};

use crate::focus::{decide, reveal};

/// A stop `id` at `[x, y, width, height]` on the root's page (layer 2).
fn stop(id: u32, frame: [f32; 4]) -> Value {
    json!({ "id": id, "frame": frame, "visible": true, "editable": false,
            "scope": 1, "layer": 2, "popup": null, "menu": false })
}

/// The root's layers: the page (2) at 800 × 600.
fn page() -> Value {
    json!([[1, [{ "id": 2, "frame": [0, 0, 800, 600], "presses": false }]]])
}

fn moves(
    key: &str,
    focused: Option<u32>,
    stops: Vec<Value>,
    modals: Value,
    layers: Value,
) -> Option<u32> {
    let request = json!({ "key": key, "focused": focused, "modals": modals, "stops": stops, "layers": layers });
    decide(&request.to_string())
}

/// Three buttons in a row over two in a row below.
fn grid() -> Vec<Value> {
    vec![
        stop(10, [0.0, 0.0, 100.0, 40.0]),
        stop(11, [120.0, 0.0, 100.0, 40.0]),
        stop(12, [240.0, 0.0, 100.0, 40.0]),
        stop(20, [0.0, 60.0, 100.0, 40.0]),
        stop(21, [240.0, 60.0, 100.0, 40.0]),
    ]
}

#[test]
fn left_and_right_move_along_the_row_and_stop_at_its_ends() {
    let go = |key, from| moves(key, Some(from), grid(), json!([]), page());
    assert_eq!(go("right", 10), Some(11));
    assert_eq!(go("right", 11), Some(12));
    assert_eq!(go("right", 12), None);
    assert_eq!(go("left", 12), Some(11));
    assert_eq!(go("left", 10), None);
    // Nothing focused: the first stop.
    assert_eq!(moves("right", None, grid(), json!([]), page()), Some(10));
}

#[test]
fn up_and_down_take_the_nearest_in_an_overlapping_column() {
    let go = |key, from| moves(key, Some(from), grid(), json!([]), page());
    assert_eq!(go("down", 10), Some(20));
    assert_eq!(go("down", 12), Some(21));
    // No column over it: the nearest by left edge.
    assert_eq!(go("down", 11), Some(20));
    assert_eq!(go("up", 21), Some(12));
    assert_eq!(go("down", 20), None);
    // Nothing focused: the first in sight from the top, the last from the bottom.
    assert_eq!(moves("down", None, grid(), json!([]), page()), Some(10));
    assert_eq!(moves("up", None, grid(), json!([]), page()), Some(20));
}

#[test]
fn the_newest_modal_that_shows_is_the_scope() {
    let mut stops = grid();
    let in_modal = |id, scope, x| {
        json!({ "id": id, "frame": [x, 200.0, 100.0, 40.0], "visible": true, "editable": false,
                "scope": scope, "layer": scope + 1, "popup": null, "menu": false })
    };
    stops.push(in_modal(40, 30, 300.0));
    stops.push(in_modal(41, 30, 420.0));
    stops.push(in_modal(50, 35, 500.0));
    let layers = json!([
        [1, [{ "id": 2, "frame": [0, 0, 800, 600], "presses": false }]],
        [30, [{ "id": 31, "frame": [0, 0, 800, 600], "presses": false }]],
        [35, [{ "id": 36, "frame": [0, 0, 800, 600], "presses": false }]],
    ]);
    // The newer modal (35) is leaving: the older one (30) is the scope, and
    // the page's stops beside it are out of reach.
    let modals = json!([[30, true], [35, false]]);
    assert_eq!(
        moves(
            "right",
            Some(40),
            stops.clone(),
            modals.clone(),
            layers.clone()
        ),
        Some(41)
    );
    assert_eq!(
        moves(
            "left",
            Some(40),
            stops.clone(),
            modals.clone(),
            layers.clone()
        ),
        None
    );
    assert_eq!(
        moves("down", None, stops.clone(), modals, layers.clone()),
        Some(40)
    );
    // Both show: the newest.
    assert_eq!(
        moves(
            "right",
            None,
            stops.clone(),
            json!([[30, true], [35, true]]),
            layers.clone()
        ),
        Some(50)
    );
    // None shows: the page.
    assert_eq!(
        moves("right", Some(12), stops, json!([[30, false]]), layers),
        None
    );
}

#[test]
fn a_backdrop_over_the_page_takes_the_layers_under_it_out_of_reach() {
    let mut stops = grid();
    // A sheet's panel (layer 4) over its backdrop (layer 3), which covers
    // the page and closes the sheet when pressed.
    stops.push(
        json!({ "id": 60, "frame": [100.0, 400.0, 100.0, 40.0], "visible": true, "editable": false,
                       "scope": 1, "layer": 4, "popup": null, "menu": false }),
    );
    stops.push(
        json!({ "id": 61, "frame": [300.0, 400.0, 100.0, 40.0], "visible": true, "editable": false,
                       "scope": 1, "layer": 4, "popup": null, "menu": false }),
    );
    let layers = |presses: bool| {
        json!([[1, [
            { "id": 2, "frame": [0, 0, 800, 600], "presses": false },
            { "id": 3, "frame": [0, 0, 800, 600], "presses": presses },
            { "id": 4, "frame": [0, 380, 800, 220], "presses": false },
        ]]])
    };
    assert_eq!(
        moves("up", Some(60), stops.clone(), json!([]), layers(true)),
        None
    );
    assert_eq!(
        moves("right", Some(60), stops.clone(), json!([]), layers(true)),
        Some(61)
    );
    assert_eq!(
        moves("down", None, stops.clone(), json!([]), layers(true)),
        Some(60)
    );
    // A backdrop that takes no presses leaves the page in reach.
    assert_eq!(
        moves("up", Some(60), stops, json!([]), layers(false)),
        Some(11)
    );
}

#[test]
fn arrows_stay_in_the_popup_of_a_focused_menu_item() {
    let mut stops = grid();
    let item = |id, y| {
        json!({ "id": id, "frame": [130.0, y, 120.0, 30.0], "visible": true, "editable": false,
                "scope": 1, "layer": 5, "popup": 5, "menu": true })
    };
    stops.push(item(70, 50.0));
    stops.push(item(71, 80.0));
    let layers = json!([[1, [
        { "id": 2, "frame": [0, 0, 800, 600], "presses": false },
        { "id": 5, "frame": [130, 50, 120, 60], "presses": false },
    ]]]);
    // Right of the item is button 21's row: the menu keeps the focus.
    assert_eq!(
        moves("right", Some(70), stops.clone(), json!([]), layers.clone()),
        None
    );
    assert_eq!(
        moves("left", Some(70), stops.clone(), json!([]), layers.clone()),
        None
    );
    assert_eq!(
        moves("down", Some(70), stops.clone(), json!([]), layers.clone()),
        Some(71)
    );
    // Nothing focused: Down enters the popup first.
    assert_eq!(moves("down", None, stops, json!([]), layers), Some(70));
}

#[test]
fn stops_out_of_sight_and_odd_requests() {
    let mut stops = grid();
    stops[3]["visible"] = json!(false);
    stops[4]["visible"] = json!(false);
    // Out of sight is still a stop the arrows reach (the page reveals it).
    assert_eq!(
        moves("down", Some(10), stops.clone(), json!([]), page()),
        Some(20)
    );
    // But with nothing focused only what is in sight is entered.
    assert_eq!(moves("up", None, stops, json!([]), page()), Some(10));
    assert_eq!(decide("not json"), None);
    assert_eq!(moves("home", Some(10), grid(), json!([]), page()), None);
    assert_eq!(
        moves("right", Some(10), Vec::new(), json!([]), page()),
        None
    );
}

#[test]
fn reveal_is_the_cores_delta_centered_vertically() {
    // In sight whole: nothing.
    assert_eq!(reveal(10.0, 40.0, 300.0, true), None);
    assert_eq!(reveal(10.0, 40.0, 300.0, false), None);
    // Below a vertical viewport: centered.
    assert_eq!(reveal(400.0, 40.0, 300.0, true), Some(270.0));
    // Past a horizontal one: as little as shows it, with the core's margin.
    let delta = reveal(400.0, 40.0, 300.0, false).unwrap();
    let expected = nsh_core::scroll::reveal_delta(
        nsh_core::Points(400.0),
        nsh_core::Points(440.0),
        nsh_core::Points(300.0),
    );
    assert_eq!(delta, expected.0);
    assert!(delta >= 140.0);
}
