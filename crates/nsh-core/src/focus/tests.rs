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

/// Rows, columns, an overlapping column, a popup, a stop out of sight and
/// two that are no candidates, in units scaled by `across` and `down`. No
/// two edges meet: edges that meet must meet exactly in the unit given.
fn scaled_layout(across: f32, down: f32) -> Vec<Candidate> {
    let at = |id: NodeId, order: usize, x: f32, y: f32, w: f32, h: f32| {
        let mut node = stop(id, x * across, y * down, w * across, h * down);
        node.order = Order::view(order);
        node
    };
    let mut popup = at(6, 2, 7.0, 9.0, 4.0, 2.0);
    popup.popup = true;
    let mut out_of_sight = at(7, 7, 0.0, 12.0, 4.0, 2.0);
    out_of_sight.visible = false;
    let mut elsewhere = at(8, 8, 2.0, 15.0, 4.0, 2.0);
    elsewhere.scope = 9;
    vec![
        at(1, 5, 0.0, 0.0, 4.0, 2.0),
        at(2, 1, 6.0, 1.0, 4.0, 2.0),
        at(3, 3, 12.0, 0.0, 4.0, 2.0),
        at(4, 4, 1.0, 5.0, 4.0, 2.0),
        at(5, 6, 20.0, 4.0, 4.0, 2.0),
        popup,
        out_of_sight,
        elsewhere,
        at(10, 0, 3.0, 18.0, 0.0, 2.0),
    ]
}

/// Every decision over `nodes`: the candidates, and Tab, Left / Right and
/// Up / Down from each of them and from none.
fn decisions(nodes: Vec<Candidate>, layers: &[Layer]) -> Vec<Vec<Option<usize>>> {
    let candidates = candidates(nodes, ROOT_ID, layers);
    let mut all = vec![candidates.iter().map(|c| Some(c.id as usize)).collect()];
    for current in std::iter::once(None).chain((0..candidates.len()).map(Some)) {
        let mut moves = Vec::new();
        for back in [false, true] {
            moves.push(tab(&candidates, current, None, None, back));
            moves.push(tab(&candidates, None, current, None, back));
            moves.push(tab(&candidates, current, None, Some(Order::view(4)), back));
            moves.push(horizontal(&candidates, current, back));
            moves.push(vertical(&candidates, current, back));
        }
        all.push(moves);
    }
    all
}

#[test]
fn frames_scaled_along_each_axis_on_its_own_move_the_focus_as_they_did() {
    let layers = |across: f32, down: f32| {
        let scaled = |x: f32, y: f32, w: f32, h: f32| {
            Some(frame(x * across, y * down, w * across, h * down))
        };
        [
            layer(1, 1, scaled(0.0, 0.0, 30.0, 20.0), false),
            layer(2, 20, scaled(-1.0, -1.0, 32.0, 22.0), true),
            layer(3, 30, scaled(5.0, 5.0, 4.0, 4.0), true),
        ]
    };
    let on_panel = |across, down| {
        let mut nodes = scaled_layout(across, down);
        nodes[3].layer = Some(3);
        nodes[4].layer = Some(3);
        nodes
    };
    let one = decisions(scaled_layout(1.0, 1.0), &[]);
    let without_popup = |across, down| {
        let mut nodes = scaled_layout(across, down);
        nodes.retain(|node| !node.popup);
        nodes
    };
    let one_without_popup = decisions(without_popup(1.0, 1.0), &[]);
    let one_covered = decisions(on_panel(1.0, 1.0), &layers(1.0, 1.0));
    // The one without an area and the one of another scope are left out.
    assert_eq!(one[0], [2, 6, 3, 4, 1, 5, 7].map(Some));
    assert_eq!(one_covered[0], [Some(4), Some(5)]);
    for (across, down) in [(8.0, 16.0), (7.3, 15.3), (15.3, 7.3)] {
        assert_eq!(decisions(scaled_layout(across, down), &[]), one);
        assert_eq!(
            decisions(without_popup(across, down), &[]),
            one_without_popup
        );
        assert_eq!(
            decisions(on_panel(across, down), &layers(across, down)),
            one_covered
        );
    }
    // `reveal` takes lengths along one axis, in points: a span that fits
    // moves as far again in a viewport as long again.
    for k in [0.5, 2.0] {
        assert_eq!(
            reveal(px(200.0 * k), px(16.0 * k), px(160.0 * k), Align::Center),
            Some(px(128.0 * k))
        );
    }
}

/// What a TV adds: preferred and initial focus, overrides, focus groups
/// and lists scrolled by index ([`directional`], [`auto_focus`],
/// [`first_stop`], [`next_row`]).
mod tv {
    use super::*;

    /// Never scrolls; nothing is inside a scroll view.
    fn directional_plain(
        nodes: &[Candidate],
        current: Option<usize>,
        direction: Direction,
        groups: &[Group],
    ) -> Target {
        directional(nodes, current, direction, groups, |_| false, |_, _| false)
    }

    fn group(id: NodeId, parent: Option<NodeId>, trap: bool, destinations: &[NodeId]) -> Group {
        Group {
            id,
            parent,
            trap,
            destinations: destinations.to_vec(),
        }
    }

    /// A row of three stops 40 points apart, and one below the first.
    fn grid() -> Vec<Candidate> {
        vec![
            stop(1, 0.0, 0.0, 40.0, 20.0),
            stop(2, 60.0, 0.0, 40.0, 20.0),
            stop(3, 120.0, 0.0, 40.0, 20.0),
            stop(4, 0.0, 60.0, 40.0, 20.0),
        ]
    }

    #[test]
    fn without_the_tv_s_props_the_arrows_move_as_on_the_desktops() {
        let nodes = grid();
        for current in [None, Some(0), Some(1), Some(2), Some(3)] {
            for (direction, horizontal_axis, forward) in [
                (Direction::Left, true, false),
                (Direction::Right, true, true),
                (Direction::Up, false, false),
                (Direction::Down, false, true),
            ] {
                let expected = if horizontal_axis {
                    horizontal(&nodes, current, forward)
                } else {
                    vertical(&nodes, current, forward)
                }
                .filter(|&to| Some(to) != current);
                let target = directional_plain(&nodes, current, direction, &[]);
                assert_eq!(
                    target,
                    expected.map_or(Target::None, Target::Stop),
                    "{direction:?} from {current:?}"
                );
            }
        }
    }

    #[test]
    fn with_nothing_focused_the_preferred_stop_wins_over_geometry() {
        let mut nodes = grid();
        nodes[2].preferred = true;
        assert_eq!(
            directional_plain(&nodes, None, Direction::Down, &[]),
            Target::Stop(2)
        );
        assert_eq!(
            directional_plain(&nodes, None, Direction::Left, &[]),
            Target::Stop(2)
        );
        // An open popup comes first, as without the prop.
        let mut with_popup = nodes.clone();
        with_popup[3].popup = true;
        assert_eq!(
            directional_plain(&with_popup, None, Direction::Down, &[]),
            Target::Stop(3)
        );
        // One out of sight is no start.
        nodes[2].visible = false;
        assert_eq!(
            directional_plain(&nodes, None, Direction::Down, &[]),
            Target::Stop(0)
        );
    }

    #[test]
    fn an_override_routes_the_arrow_past_geometry() {
        let mut nodes = grid();
        // Down from the last of the row goes to the stop below the first.
        nodes[2].next.down = Some(4);
        assert_eq!(
            directional_plain(&nodes, Some(2), Direction::Down, &[]),
            Target::Stop(3)
        );
        // Only that arrow: Left keeps its neighbor.
        assert_eq!(
            directional_plain(&nodes, Some(2), Direction::Left, &[]),
            Target::Stop(1)
        );
        // A route to a view that is no stop in reach leaves the arrow to
        // geometry.
        nodes[0].next.right = Some(99);
        assert_eq!(
            directional_plain(&nodes, Some(0), Direction::Right, &[]),
            Target::Stop(1)
        );
        // It leaves a trapped group: the app's own route.
        for node in &mut nodes[..3] {
            node.group = Some(50);
        }
        let groups = [group(50, None, true, &[])];
        assert_eq!(
            directional_plain(&nodes, Some(2), Direction::Down, &groups),
            Target::Stop(3)
        );
    }

    #[test]
    fn a_trapping_group_keeps_the_arrows_inside() {
        let mut nodes = grid();
        for node in &mut nodes[..3] {
            node.group = Some(50);
        }
        let trapped = [group(50, None, true, &[])];
        assert_eq!(
            directional_plain(&nodes, Some(0), Direction::Down, &trapped),
            Target::None
        );
        assert_eq!(
            directional_plain(&nodes, Some(0), Direction::Right, &trapped),
            Target::Stop(1)
        );
        // Without the trap, Down leaves it.
        let open = [group(50, None, false, &[])];
        assert_eq!(
            directional_plain(&nodes, Some(0), Direction::Down, &open),
            Target::Stop(3)
        );
        // A trap around the group a stop is in holds it too.
        for node in &mut nodes[..2] {
            node.group = Some(51);
        }
        let nested = [group(50, None, true, &[]), group(51, Some(50), false, &[])];
        assert_eq!(
            directional_plain(&nodes, Some(1), Direction::Right, &nested),
            Target::Stop(2)
        );
        assert_eq!(
            directional_plain(&nodes, Some(1), Direction::Down, &nested),
            Target::None
        );
    }

    #[test]
    fn entering_a_group_goes_to_its_first_destination_in_reach() {
        let mut nodes = grid();
        // The row is a group entered at its last stop.
        for node in &mut nodes[..3] {
            node.group = Some(50);
        }
        let from_below = Some(3);
        let groups = [group(50, None, false, &[77, 3])];
        assert_eq!(
            directional_plain(&nodes, from_below, Direction::Up, &groups),
            Target::Stop(2),
            "77 is no stop: the next destination"
        );
        // Moving inside the group keeps geometry.
        assert_eq!(
            directional_plain(&nodes, Some(0), Direction::Right, &groups),
            Target::Stop(1)
        );
        // A destination outside the group is none of its own.
        let groups = [group(50, None, false, &[4])];
        assert_eq!(
            directional_plain(&nodes, from_below, Direction::Up, &groups),
            Target::Stop(0)
        );
    }

    #[test]
    fn entering_a_group_without_destinations_goes_to_its_preferred_stop() {
        let mut nodes = grid();
        for node in &mut nodes[..3] {
            node.group = Some(50);
        }
        nodes[1].preferred = true;
        let groups = [group(50, None, false, &[])];
        assert_eq!(
            directional_plain(&nodes, Some(3), Direction::Up, &groups),
            Target::Stop(1)
        );
        // Outside a group a preferred stop is reached by geometry only.
        let mut flat = grid();
        flat[1].preferred = true;
        assert_eq!(
            directional_plain(&flat, Some(3), Direction::Up, &[]),
            Target::Stop(0)
        );
    }

    #[test]
    fn a_scroll_view_that_can_scroll_on_keeps_the_focus() {
        // A list (60) of two mounted rows, a button below it.
        let mut nodes = vec![
            stop(1, 0.0, 0.0, 100.0, 20.0),
            stop(2, 0.0, 20.0, 100.0, 20.0),
            stop(3, 0.0, 200.0, 100.0, 20.0),
        ];
        nodes[0].scroller = Some(60);
        nodes[1].scroller = Some(60);
        let inside = |scroller: NodeId, id: NodeId| scroller == 60 && id <= 2;
        let scrolls = |more: bool| {
            move |nodes: &[Candidate], current| {
                directional(nodes, current, Direction::Down, &[], |_| more, inside)
            }
        };
        // A row mounted below: it takes the focus.
        assert_eq!(scrolls(true)(&nodes, Some(0)), Target::Stop(1));
        // The last mounted row, the list goes on: it scrolls, the button
        // below is not reached.
        assert_eq!(scrolls(true)(&nodes, Some(1)), Target::Scroll(60));
        // At the list's end the arrow leaves it.
        assert_eq!(scrolls(false)(&nodes, Some(1)), Target::Stop(2));
        // Nothing below at all: it scrolls while it can.
        nodes.pop();
        assert_eq!(scrolls(true)(&nodes, Some(1)), Target::Scroll(60));
        assert_eq!(scrolls(false)(&nodes, Some(1)), Target::None);
    }

    #[test]
    fn auto_focus_is_the_first_stop_that_asks_and_was_not_seen() {
        let mut nodes = grid();
        assert_eq!(auto_focus(&nodes, |_| false), None);
        nodes[1].auto_focus = true;
        nodes[3].auto_focus = true;
        assert_eq!(auto_focus(&nodes, |_| false), Some(1));
        assert_eq!(auto_focus(&nodes, |id| id == 2), Some(3));
        assert_eq!(auto_focus(&nodes, |_| true), None);
    }

    #[test]
    fn a_scope_opened_gives_the_focus_to_auto_focus_then_preferred_then_first() {
        let mut nodes = grid();
        assert_eq!(first_stop(&nodes), Some(0));
        nodes[2].preferred = true;
        assert_eq!(first_stop(&nodes), Some(2));
        nodes[3].auto_focus = true;
        assert_eq!(first_stop(&nodes), Some(3));
        assert_eq!(first_stop::<Candidate>(&[]), None);
    }

    #[test]
    fn the_next_row_of_a_list_is_one_along_its_axis() {
        let row = |index, count, columns, horizontal| Row {
            list: 60,
            index,
            count,
            columns,
            horizontal,
        };
        let list = row(4, 10, 1, false);
        assert_eq!(next_row(&list, Direction::Down), Some(5));
        assert_eq!(next_row(&list, Direction::Up), Some(3));
        // Across a list of one column: geometry's.
        assert_eq!(next_row(&list, Direction::Right), None);
        // The ends.
        assert_eq!(next_row(&row(9, 10, 1, false), Direction::Down), None);
        assert_eq!(next_row(&row(0, 10, 1, false), Direction::Up), None);
        // A horizontal list moves with Left and Right.
        assert_eq!(next_row(&row(4, 10, 1, true), Direction::Right), Some(5));
        assert_eq!(next_row(&row(4, 10, 1, true), Direction::Down), None);
        // A grid of three columns: a row of them along, one across.
        let grid = row(4, 10, 3, false);
        assert_eq!(next_row(&grid, Direction::Down), Some(7));
        assert_eq!(next_row(&grid, Direction::Up), Some(1));
        assert_eq!(next_row(&grid, Direction::Right), Some(5));
        assert_eq!(next_row(&grid, Direction::Left), Some(3));
        assert_eq!(next_row(&row(5, 10, 3, false), Direction::Right), None);
        assert_eq!(next_row(&row(8, 10, 3, false), Direction::Down), None);
        // No columns counts as one.
        assert_eq!(next_row(&row(4, 10, 0, false), Direction::Down), Some(5));
    }

    #[test]
    fn groups_are_found_through_their_parents() {
        let groups = [group(50, None, false, &[]), group(51, Some(50), true, &[])];
        assert!(in_group(&groups, Some(51), 50));
        assert!(in_group(&groups, Some(51), 51));
        assert!(!in_group(&groups, Some(50), 51));
        assert!(!in_group(&groups, None, 50));
        // A loop the backend made ends.
        let looped = [
            group(50, Some(51), false, &[]),
            group(51, Some(50), false, &[]),
        ];
        assert!(!in_group(&looped, Some(50), 99));
    }
}
