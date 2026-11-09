use super::*;
use crate::geometry::{point, px, size};

fn frame(x: f32, y: f32, width: f32, height: f32) -> Bounds<Points> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// A stop of the root scope in sight, its tree order its id.
fn stop(id: NodeId, x: f32, y: f32, width: f32, height: f32) -> Candidate {
    Candidate {
        id,
        frame: frame(x, y, width, height),
        visible: true,
        stop: true,
        scope: ROOT_ID,
        layer: Some(1),
        order: Order::view(id as usize),
        ..Default::default()
    }
}

fn layer(id: NodeId, order: usize, frame: Option<Bounds<Points>>, takes_presses: bool) -> Layer {
    Layer {
        id,
        order,
        frame,
        takes_presses,
    }
}

fn ids(candidates: &[Candidate]) -> Vec<NodeId> {
    candidates.iter().map(|c| c.id).collect()
}

#[test]
fn candidates_are_the_stops_of_the_scope_in_focus_order() {
    let mut hidden_field = stop(10, 0.0, 0.0, 0.0, 16.0);
    hidden_field.editable = true;
    let mut link = stop(11, 0.0, 16.0, 16.0, 16.0);
    link.order = Order::link(4, 0);
    let mut second_link = stop(12, 16.0, 16.0, 16.0, 16.0);
    second_link.order = Order::link(4, 1);
    let mut in_modal = stop(13, 0.0, 32.0, 8.0, 16.0);
    in_modal.scope = 7;
    let mut no_stop = stop(14, 0.0, 48.0, 8.0, 16.0);
    no_stop.stop = false;
    let nodes = [
        stop(6, 0.0, 0.0, 8.0, 16.0),
        stop(4, 8.0, 0.0, 8.0, 16.0),
        second_link,
        link,
        stop(5, 0.0, 64.0, 0.0, 16.0),
        hidden_field,
        in_modal,
        no_stop,
    ];
    // Ordered by tree order, a text's links right after it; an editable
    // field narrower than a Cell stays, a view without an area does not.
    assert_eq!(ids(&candidates(nodes, ROOT_ID, &[])), [4, 11, 12, 6, 10]);
    assert_eq!(ids(&candidates(nodes, 7, &[])), [13]);
}

#[test]
fn a_backdrop_over_the_page_takes_it_and_the_layers_under_it_out_of_reach() {
    let page = frame(0.0, 0.0, 800.0, 600.0);
    let layers = [
        layer(1, 1, Some(page), false),
        layer(2, 20, Some(frame(-10.0, 0.0, 820.0, 600.0)), true),
        layer(3, 30, Some(frame(100.0, 100.0, 50.0, 50.0)), true),
    ];
    assert_eq!(covering_layer(&layers).map(|l| l.id), Some(2));
    let mut on_backdrop = stop(21, 10.0, 10.0, 8.0, 16.0);
    on_backdrop.layer = Some(2);
    let mut on_panel = stop(31, 100.0, 100.0, 8.0, 16.0);
    on_panel.layer = Some(3);
    let nodes = [stop(5, 0.0, 0.0, 8.0, 16.0), on_backdrop, on_panel];
    // The page's stops and the backdrop's own are out of reach; those of
    // the layers above it stay.
    assert_eq!(ids(&candidates(nodes, ROOT_ID, &layers)), [31]);
    assert_eq!(covered_by(&layers, 1), Some(2));
    assert_eq!(covered_by(&layers, 2), None);
    assert_eq!(covered_by(&layers, 3), None);

    // A layer that does not cover the page, takes no presses or is out of
    // sight covers nothing.
    for cover in [
        layer(2, 20, Some(frame(0.0, 0.0, 800.0, 599.0)), true),
        layer(2, 20, Some(page), false),
        layer(2, 20, None, true),
    ] {
        let layers = [layer(1, 1, Some(page), false), cover];
        assert_eq!(covering_layer(&layers), None);
        assert_eq!(candidates(nodes, ROOT_ID, &layers).len(), 3);
    }
    // Nor does anything while the page is out of sight.
    assert_eq!(covering_layer(&[layer(1, 1, None, false), layers[1]]), None);
}

#[test]
fn the_focus_moves_in_the_newest_modal_that_shows() {
    assert_eq!(focus_scope(&[], |_| true), ROOT_ID);
    assert_eq!(focus_scope(&[7, 9], |_| true), 9);
    assert_eq!(focus_scope(&[7, 9], |id| id == 7), 7);
    assert_eq!(focus_scope(&[7, 9], |_| false), ROOT_ID);
}

#[test]
fn left_and_right_go_to_the_nearest_stop_on_an_overlapping_row() {
    let row = [
        stop(1, 0.0, 0.0, 40.0, 16.0),
        stop(2, 48.0, 8.0, 40.0, 16.0),
        stop(3, 96.0, 0.0, 40.0, 16.0),
        // Touching the row's bottom edge: not on it.
        stop(4, 56.0, 16.0, 40.0, 16.0),
        stop(5, 200.0, 32.0, 40.0, 16.0),
    ];
    assert_eq!(horizontal(&row, Some(0), true), Some(1));
    assert_eq!(horizontal(&row, Some(2), false), Some(1));
    assert_eq!(horizontal(&row, Some(2), true), None);
    assert_eq!(horizontal(&row, Some(0), false), None);
    // 2 overlaps both rows, and 4 is nearer by its left edge.
    assert_eq!(horizontal(&row, Some(1), true), Some(3));
    assert_eq!(horizontal(&row, None, true), Some(0));
    assert_eq!(horizontal::<Candidate>(&[], None, true), None);
    // Of two as near, the first in focus order.
    let tie = [
        stop(1, 50.0, 0.0, 10.0, 16.0),
        stop(2, 60.0, 0.0, 10.0, 16.0),
        stop(3, 60.0, 8.0, 10.0, 16.0),
    ];
    assert_eq!(horizontal(&tie, Some(0), true), Some(1));
}

#[test]
fn up_and_down_go_to_the_nearest_stop_an_overlapping_column_first() {
    let grid = [
        stop(1, 0.0, 0.0, 40.0, 16.0),
        // Nearer below, but in another column.
        stop(2, 100.0, 16.0, 40.0, 16.0),
        stop(3, 8.0, 48.0, 40.0, 16.0),
        stop(4, 0.0, 48.0, 40.0, 16.0),
        stop(5, 0.0, 96.0, 40.0, 16.0),
    ];
    // The column first, then the gap, then the left edge.
    assert_eq!(vertical(&grid, Some(0), true), Some(3));
    assert_eq!(vertical(&grid, Some(3), true), Some(4));
    assert_eq!(vertical(&grid, Some(4), false), Some(3));
    assert_eq!(vertical(&grid, Some(4), true), None);
    // Out of every column, the nearest gap, then the nearest left edge.
    assert_eq!(vertical(&grid, Some(1), true), Some(2));
    // A stop overlapping the one focused is neither above nor below it.
    let overlapping = [stop(1, 0.0, 0.0, 40.0, 16.0), stop(2, 0.0, 8.0, 40.0, 16.0)];
    assert_eq!(vertical(&overlapping, Some(0), true), None);
}

#[test]
fn up_and_down_with_nothing_focused_enter_what_is_in_sight_a_popup_first() {
    let mut out_of_sight = stop(1, 0.0, -32.0, 40.0, 16.0);
    out_of_sight.visible = false;
    let page = [
        out_of_sight,
        stop(2, 48.0, 0.0, 40.0, 16.0),
        stop(3, 0.0, 0.0, 40.0, 16.0),
        stop(4, 0.0, 64.0, 40.0, 16.0),
        stop(5, 48.0, 64.0, 40.0, 16.0),
    ];
    assert_eq!(vertical(&page, None, true), Some(2));
    assert_eq!(vertical(&page, None, false), Some(3));
    let mut item = stop(6, 0.0, 32.0, 40.0, 16.0);
    item.popup = true;
    let mut last_item = stop(7, 0.0, 48.0, 40.0, 16.0);
    last_item.popup = true;
    let with_popup = [page[1], page[2], item, last_item, page[3]];
    assert_eq!(vertical(&with_popup, None, true), Some(2));
    assert_eq!(vertical(&with_popup, None, false), Some(3));
}

#[test]
fn tab_goes_round_from_the_focus_the_stop_left_or_a_point_in_order() {
    let stops = [
        stop(2, 0.0, 0.0, 8.0, 16.0),
        stop(4, 0.0, 16.0, 8.0, 16.0),
        stop(6, 0.0, 32.0, 8.0, 16.0),
    ];
    assert_eq!(tab(&stops, Some(0), None, None, false), Some(1));
    assert_eq!(tab(&stops, Some(2), None, None, false), Some(0));
    assert_eq!(tab(&stops, Some(0), None, None, true), Some(2));
    assert_eq!(tab(&stops, None, None, None, false), Some(0));
    assert_eq!(tab(&stops, None, None, None, true), Some(2));
    assert_eq!(tab(&stops, None, Some(1), None, false), Some(2));
    // From a search match's text at 3 in tree order, both ways.
    assert_eq!(
        tab(&stops, None, None, Some(Order::view(3)), false),
        Some(1)
    );
    assert_eq!(tab(&stops, None, None, Some(Order::view(3)), true), Some(0));
    assert_eq!(
        tab(&stops, None, None, Some(Order::view(9)), false),
        Some(0)
    );
    assert_eq!(tab(&stops, None, None, Some(Order::view(1)), true), Some(2));
    // The focus wins over the point.
    assert_eq!(
        tab(&stops, Some(2), None, Some(Order::view(3)), true),
        Some(1)
    );
    assert_eq!(tab::<Candidate>(&[], None, None, None, false), None);
}

#[test]
fn a_stop_out_of_its_scroll_view_yields_a_reveal_delta() {
    let viewport = px(160.0);
    assert_eq!(reveal(px(0.0), px(16.0), viewport, Align::Nearest), None);
    assert_eq!(reveal(px(144.0), px(16.0), viewport, Align::Center), None);
    let below = (px(200.0), px(16.0));
    assert_eq!(
        reveal(below.0, below.1, viewport, Align::Nearest),
        Some(scroll::reveal_delta(below.0, below.0 + below.1, viewport))
    );
    // Centered: the span's middle at the viewport's.
    assert_eq!(
        reveal(below.0, below.1, viewport, Align::Center),
        Some(px(128.0))
    );
    assert_eq!(
        reveal(px(-40.0), px(16.0), viewport, Align::Center),
        Some(px(-112.0))
    );
    // Longer than the viewport: its start, 16 points (the margin) in.
    assert_eq!(
        reveal(px(300.0), px(400.0), viewport, Align::Center),
        Some(px(284.0))
    );
}

#[test]
fn a_modal_takes_the_focus_to_its_first_stop_and_gives_it_back_when_it_closes() {
    let mut modals = Modals::default();
    assert_eq!(modals.update(&[], Some(5), |_| None, |_, _| true), None);
    let opened = modals.update(&[7], Some(5), |_| Some(70), |_, _| true);
    assert_eq!(
        opened,
        Some(vec![Move {
            focus: Some(70),
            reveal: true
        }])
    );
    assert_eq!(modals.update(&[7], Some(71), |_| None, |_, _| true), None);
    let closed = modals.update(
        &[],
        Some(71),
        |_| None,
        |scope, id| scope == ROOT_ID && id == 5,
    );
    assert_eq!(
        closed,
        Some(vec![Move {
            focus: Some(5),
            reveal: false
        }])
    );
    // The view it came from is no stop in sight any more: no focus.
    modals.update(&[7], Some(5), |_| Some(70), |_, _| true);
    let closed = modals.update(&[], Some(70), |_| None, |_, _| false);
    assert_eq!(
        closed,
        Some(vec![Move {
            focus: None,
            reveal: false
        }])
    );
}

#[test]
fn modals_closed_together_give_the_focus_back_to_the_page() {
    let mut modals = Modals::default();
    let first = |modal: NodeId| Some(modal * 10);
    modals.update(&[7], Some(5), first, |_, _| true);
    let nested = modals.update(&[7, 9], Some(70), first, |_, _| true);
    assert_eq!(
        nested,
        Some(vec![Move {
            focus: Some(90),
            reveal: true
        }])
    );
    // Both close: the outer one's opener, on the page.
    let mut asked = Vec::new();
    let closed = modals.update(&[], Some(90), first, |scope, id| {
        asked.push((scope, id));
        true
    });
    assert_eq!(asked, [(ROOT_ID, 5)]);
    assert_eq!(
        closed,
        Some(vec![Move {
            focus: Some(5),
            reveal: false
        }])
    );
    // Two presented in one batch: each opens over the one before.
    let both = modals.update(&[7, 9], Some(5), first, |_, _| true);
    assert_eq!(
        both,
        Some(vec![
            Move {
                focus: Some(70),
                reveal: true
            },
            Move {
                focus: Some(90),
                reveal: true
            },
        ])
    );
    // The inner one closes: the outer one's stop.
    let inner = modals.update(&[7], Some(90), first, |scope, id| scope == 7 && id == 70);
    assert_eq!(
        inner,
        Some(vec![Move {
            focus: Some(70),
            reveal: false
        }])
    );
}

#[test]
fn an_empty_modal_gives_the_focus_to_its_first_stop_once_it_has_one() {
    let mut modals = Modals::default();
    let opened = modals.update(&[7], Some(5), |_| None, |_, _| true);
    assert_eq!(
        opened,
        Some(vec![Move {
            focus: None,
            reveal: false
        }])
    );
    assert_eq!(modals.first_stop(None, || None), None);
    assert_eq!(modals.first_stop(Some(3), || Some(70)), None);
    assert_eq!(modals.first_stop(None, || Some(70)), Some(70));
    assert_eq!(modals.first_stop(None, || Some(71)), None);
    // Closing it still gives back the focus from before it.
    let closed = modals.update(&[], Some(70), |_| None, |_, id| id == 5);
    assert_eq!(
        closed,
        Some(vec![Move {
            focus: Some(5),
            reveal: false
        }])
    );
}

#[test]
fn a_layer_that_goes_gives_the_focus_back_to_the_stop_it_came_from() {
    let mut layers = Layers::default();
    layers.entered(2, 5);
    layers.entered(3, 21);
    // The newest layer gone, back to an older one while its stop went too.
    assert_eq!(layers.restore(|layer| layer == 3, |_| true), Some(21));
    assert_eq!(layers.restore(|_| true, |id| id != 21), Some(5));
    assert_eq!(layers.restore(|_| false, |_| true), None);
    // Entering a layer again replaces its stop.
    layers.entered(2, 6);
    assert_eq!(layers.restore(|layer| layer == 2, |_| true), Some(6));
    layers.retain(|layer| layer != 2);
    assert_eq!(layers.restore(|_| true, |id| id != 21), None);
}
