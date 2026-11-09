//! The arrows move the focus between the same Cells whatever size a Cell
//! is in points: the focus rules see each Cell as one point.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use nsh_core::prelude::*;
use nsh_platform_tui::{App, AppOptions, AppView, CellSize, Handled, Input, Modifiers, Root};
use ratatui_core::{backend::TestBackend, terminal::Terminal};

use super::app::draw_until;

fn surface_with(width: f32, height: f32) -> (AppView, crossbeam_channel::Receiver<()>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut options = AppOptions::default();
    options.cells = CellSize::new(px(width), px(height)).unwrap();
    options.wake = Some(Arc::new(move || {
        let _ = tx.send(());
    }));
    options.cache = nsh_engine::CacheOptions::off();
    (AppView::new(options), rx)
}

/// A pressable card five rows tall from row `top`, and a button on the row
/// right under it from column 4, with Cells `width` × `height` points; the
/// log names each view that takes the focus.
fn card_and_button(width: f32, height: f32, top: f32) -> impl IntoElement {
    let log = signal(String::new());
    let focus = move |label: &'static str| {
        move |_: &()| {
            log.update(|log| {
                if !log.is_empty() {
                    log.push(',');
                }
                log.push_str(label);
            })
        }
    };
    view()
        .flex_1()
        .child(
            view()
                .absolute()
                .left(px(0.0))
                .top(px(height * top))
                .w(px(width * 3.0))
                .h(px(height * 5.0))
                .on_press(|_: &()| {})
                .on_focus(focus("card")),
        )
        .child(
            view()
                .absolute()
                .left(px(width * 4.0))
                .top(px(height * (top + 5.0)))
                .w(px(width * 6.0))
                .h(px(height))
                .on_press(|_: &()| {})
                .on_focus(focus("button")),
        )
        .child(
            text(move || format!("log {}.", log.get()))
                .absolute()
                .left(px(0.0))
                .top(px(height * 10.0)),
        )
}

#[component]
fn Whole() -> impl IntoElement {
    card_and_button(8.0, 16.0, 1.0)
}

#[component]
fn Fifths() -> impl IntoElement {
    card_and_button(8.4, 16.8, 1.0)
}

#[component]
fn Tenths() -> impl IntoElement {
    card_and_button(7.3, 15.3, 1.0)
}

/// The card from row -3, its first rows scrolled off above the surface.
#[component]
fn FifthsAbove() -> impl IntoElement {
    card_and_button(8.4, 16.8, -3.0)
}

#[component]
fn TenthsAbove() -> impl IntoElement {
    card_and_button(7.3, 15.3, -3.0)
}

fn key(view: &mut AppView, name: &str) -> Handled {
    view.handle(Input::Key {
        key: name.into(),
        modifiers: Modifiers::default(),
        release: false,
    })
}

/// Draws for a while and returns the log's line at the end: a key that
/// moves nothing leaves no commit to wait for.
fn settled_log(
    view: &mut AppView,
    terminal: &mut Terminal<TestBackend>,
    rx: &crossbeam_channel::Receiver<()>,
) -> String {
    let end = Instant::now() + Duration::from_millis(300);
    while Instant::now() < end {
        terminal
            .draw(|frame| view.render(frame, frame.area()))
            .unwrap();
        let _ = rx.recv_deadline(end);
    }
    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let contents: String = buffer.content.iter().map(|c| c.symbol()).collect();
    let start = contents.find("log ").expect("the log shows");
    let end = start + contents[start..].find('.').expect("the log ends");
    contents[start..=end].to_owned()
}

/// From the card, Right finds nothing on its rows and Down the button on
/// the row under them.
fn moves(app: App, width: f32, height: f32) {
    let (mut view, rx) = surface_with(width, height);
    let mut terminal = Terminal::new(TestBackend::new(30, 14)).unwrap();
    view.load(app).unwrap();
    draw_until(&mut view, &mut terminal, &rx, "log .");
    assert_eq!(key(&mut view, "Tab"), Handled::Yes);
    draw_until(&mut view, &mut terminal, &rx, "log card.");
    assert_eq!(key(&mut view, "Right"), Handled::Yes);
    assert_eq!(
        settled_log(&mut view, &mut terminal, &rx),
        "log card.",
        "Right at {width}x{height}"
    );
    assert_eq!(key(&mut view, "Down"), Handled::Yes);
    draw_until(&mut view, &mut terminal, &rx, "log card,button.");
    assert_eq!(key(&mut view, "Up"), Handled::Yes);
    draw_until(&mut view, &mut terminal, &rx, "log card,button,card.");
}

#[test]
fn whole_point_cells_move_down_to_the_row_under_a_card_and_not_right() {
    moves(App::Rust(Root::new::<Whole>(|| ())), 8.0, 16.0);
}

#[test]
fn fractional_cells_move_as_whole_point_cells_do() {
    moves(App::Rust(Root::new::<Fifths>(|| ())), 8.4, 16.8);
    moves(App::Rust(Root::new::<Tenths>(|| ())), 7.3, 15.3);
}

#[test]
fn fractional_cells_above_the_surface_move_as_whole_point_cells_do() {
    moves(App::Rust(Root::new::<FifthsAbove>(|| ())), 8.4, 16.8);
    moves(App::Rust(Root::new::<TenthsAbove>(|| ())), 7.3, 15.3);
}
