use super::*;
use crate::{
    effects::{BlendMode, Cursor, Filter, GradientKind, Keyframes},
    geometry::{DefiniteLength, deg, px},
    style::{Color, FontFamily, Style},
    transform::TransformKind,
};

/// The Tailwind theme with the design tokens apps have.
fn themed() -> Theme {
    let mut theme = Theme::default();
    theme.set_tokens(&ui_theme());
    theme
}

fn ui_theme() -> crate::element::Tokens {
    crate::element::tokens::sample()
}

fn cx(states: u16) -> (ClassEnvironment, u16) {
    (ClassEnvironment::default(), states)
}

/// Applies `classes` with the given environment and states.
fn styled_with(
    classes: &str,
    env: ClassEnvironment,
    window: (f32, f32),
    states: u16,
) -> (Style, ClassProps) {
    let theme = themed();
    let list = parse(&theme, classes);
    assert!(
        list.unknown().is_empty(),
        "unknown classes: {:?}",
        list.unknown()
    );
    let mut style = Style::default();
    let mut props = ClassProps::default();
    let group = |_: &str| 0;
    let context = Context {
        env,
        window: crate::geometry::size_pt(window.0, window.1),
        rtl: false,
        states,
        position: 0,
        group: &group,
    };
    list.apply(&context, &theme, &mut style, &mut props, |_| {});
    // As the core does: the style's node props.
    style.node_props(&mut props);
    (style, props)
}

fn styled(classes: &str) -> Style {
    let (env, states) = cx(0);
    styled_with(classes, env, (390.0, 844.0), states).0
}

#[test]
fn spacing_and_layout() {
    let s = styled(
        "flex flex-row items-center justify-between gap-3 p-4 px-2 -mt-1 w-1/2 h-12 min-w-0",
    );
    assert_eq!(s.layout.flex_direction, taffy::FlexDirection::Row);
    assert_eq!(s.layout.align_items, taffy::AlignItems::CENTER);
    assert_eq!(s.layout.justify_content, taffy::AlignContent::SPACE_BETWEEN);
    assert_eq!(s.layout.gap.width, taffy::LengthPercentage::length(12.0));
    assert_eq!(s.layout.padding.left, taffy::LengthPercentage::length(8.0));
    assert_eq!(s.layout.padding.top, taffy::LengthPercentage::length(16.0));
    assert_eq!(
        s.layout.margin.top,
        taffy::LengthPercentageAuto::length(-4.0)
    );
    assert_eq!(s.layout.size.width, taffy::Dimension::percent(0.5));
    assert_eq!(s.layout.size.height, taffy::Dimension::length(48.0));
    let s = styled("flex-[2]");
    assert_eq!(s.layout.flex_grow, 2.0);
    let s = styled("flex-[3_0_50%]");
    assert_eq!((s.layout.flex_grow, s.layout.flex_shrink), (3.0, 0.0));
    assert_eq!(s.layout.flex_basis, taffy::Dimension::percent(0.5));
    // CSS's shorthand, read as a style's `flex` is.
    let s = styled("flex-[auto]");
    assert_eq!((s.layout.flex_grow, s.layout.flex_shrink), (1.0, 1.0));
    assert_eq!(s.layout.flex_basis, taffy::Dimension::auto());
    let s = styled("flex-[2_1rem]");
    assert_eq!((s.layout.flex_grow, s.layout.flex_shrink), (2.0, 1.0));
    assert_eq!(s.layout.flex_basis, taffy::Dimension::length(16.0));
    let s = styled("p-[13px] m-auto inset-0 absolute size-10 basis-1/3 flex-1");
    assert_eq!(s.layout.padding.left, taffy::LengthPercentage::length(13.0));
    assert_eq!(s.layout.margin.left, taffy::LengthPercentageAuto::auto());
    assert_eq!(s.layout.position, taffy::Position::Absolute);
    assert_eq!(
        s.layout.inset.right,
        taffy::LengthPercentageAuto::length(0.0)
    );
    assert_eq!(s.layout.size.height, taffy::Dimension::length(40.0));
    assert_eq!(s.layout.flex_grow, 1.0);
    // CSS's automatic minimum; unset, a minimum is 0 (Yoga's).
    let s = styled("min-w-auto min-h-auto");
    assert_eq!(s.layout.min_size.width, taffy::LengthPercentageAuto::auto());
    assert_eq!(
        s.layout.min_size.height,
        taffy::LengthPercentageAuto::auto()
    );
}

#[test]
fn physical_classes_keep_their_sides_right_to_left() {
    // As in CSS, and as style keys do: the core lays left and right out
    // where they say in either direction, so a class needs no direction.
    let theme = themed();
    let resolve = |classes: &str, rtl: bool| {
        let list = parse(&theme, classes);
        assert_eq!(list.deps() & deps::RTL, 0, "{classes}");
        let group = |_: &str| 0;
        let cx = Context {
            env: ClassEnvironment::default(),
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl,
            states: 0,
            position: 0,
            group: &group,
        };
        let mut style = Style::default();
        list.apply(&cx, &theme, &mut style, &mut ClassProps::default(), |_| {});
        style
    };
    for rtl in [false, true] {
        let s = resolve("pl-4 mr-2 left-1", rtl);
        assert_eq!(s.layout.padding.left, taffy::LengthPercentage::length(16.0));
        assert_eq!(
            s.layout.margin.right,
            taffy::LengthPercentageAuto::length(8.0)
        );
        assert_eq!(
            s.layout.inset.left,
            taffy::LengthPercentageAuto::length(4.0)
        );
    }
    let s = resolve("text-left", true);
    assert_eq!(s.text.text_align, Some(crate::style::TextAlign::Left));
    assert!(s.text.text_align_physical);
    // `text-start` and `text-end` follow the direction, as CSS's `start`
    // and `end`.
    let s = resolve("text-end", true);
    assert_eq!(s.text.text_align, Some(crate::style::TextAlign::Right));
    assert!(!s.text.text_align_physical);
    let s = resolve(
        "text-start ps-4 me-2 start-1 rounded-s-lg border-e-2 px-2",
        true,
    );
    assert_eq!(s.text.text_align, Some(crate::style::TextAlign::Left));
    assert!(!s.text.text_align_physical);
}

#[test]
fn space_gives_the_children_margins() {
    // `space-x-4` is `:where(& > :not(:last-child)) { margin-inline-end }`:
    // the view's children get it, not the view (no gap).
    let (s, props) = styled_with(
        "gap-2 space-x-4 -space-y-px",
        ClassEnvironment::default(),
        (390.0, 844.0),
        0,
    );
    assert_eq!(s.layout.gap.width, taffy::LengthPercentage::length(8.0));
    assert_eq!(s.layout.gap.height, taffy::LengthPercentage::length(8.0));
    assert_eq!(
        s.layout.margin.right,
        taffy::LengthPercentageAuto::length(0.0)
    );
    let space = props.space.unwrap();
    let margin = |n: f32| crate::PropertyValue::Lpa(taffy::LengthPercentageAuto::length(n));
    assert_eq!(space.x, Some(margin(16.0)));
    assert_eq!(space.y, Some(margin(-1.0)));
    assert!(!space.reverse_x);
    let (_, props) = styled_with(
        "space-y-reverse space-y-2",
        ClassEnvironment::default(),
        (390.0, 844.0),
        0,
    );
    let space = props.space.unwrap();
    assert!(space.reverse_y);
    let mut child = Style::default();
    space.apply(&mut child, false, false);
    assert_eq!(
        child.layout.margin.top,
        taffy::LengthPercentageAuto::length(8.0)
    );
    assert_eq!(
        child.layout.margin.bottom,
        taffy::LengthPercentageAuto::length(0.0)
    );
}

#[test]
fn widths_without_a_color_are_current_color() {
    let theme = themed();
    let resolve = |classes: &str, current: CurrentColor| {
        let list = parse(&theme, classes);
        let group = |_: &str| 0;
        let cx = Context {
            env: ClassEnvironment::default(),
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &group,
        };
        let mut style = Style::default();
        list.apply_with(
            &cx,
            &no_data,
            &theme,
            current,
            DEFAULT_CH,
            &mut style,
            &mut ClassProps::default(),
            |_| {},
        );
        style
    };
    let white = Color(0xffff_ffff);
    let inherited = CurrentColor {
        inherited: Some(white),
        inline: None,
    };
    // Preflight's `border: 0 solid` is `currentColor`; so are a ring's and
    // an outline's: the inherited text color...
    let s = resolve("border-2 outline ring", inherited);
    assert_eq!(s.view.borders.colors, [white; 4]);
    assert_eq!(s.view.outline.color, white);
    let e = s.view.effects.unwrap();
    assert_eq!(e.box_shadows[0].color, white);
    assert_ne!(parse(&theme, "border").deps() & deps::CURRENT_COLOR, 0);
    assert_ne!(
        parse(&theme, "inset-ring-2").deps() & deps::CURRENT_COLOR,
        0
    );
    assert_eq!(
        parse(&theme, "border-red-500 p-4").deps() & deps::CURRENT_COLOR,
        0
    );
    // ...or the node's own, a class's or its inline style's...
    let red = Color(0xfb2c_36ff);
    let s = resolve("border-t text-red-500 inset-ring", inherited);
    assert_eq!(s.view.borders.colors[0], red);
    assert_eq!(s.view.effects.unwrap().box_shadows[0].color, red);
    let blue = Color(0x0000_ffff);
    let s = resolve(
        "border text-red-500",
        CurrentColor {
            inherited: Some(white),
            inline: Some(blue),
        },
    );
    assert_eq!(s.view.borders.colors[0], blue);
    // ...whichever comes first; a color class wins wherever it is.
    let s = resolve(
        "border-sky-500 border-2 outline-red-500 outline-dashed",
        inherited,
    );
    assert_eq!(s.view.borders.colors[0], Color(0x00a6_f4ff));
    assert_eq!(s.view.outline.color, red);
    // Without one, React Native's black, as inline styles have it.
    let s = resolve("border", CurrentColor::default());
    assert_eq!(s.view.borders.colors[0], Color::BLACK);
}

#[test]
fn later_classes_win() {
    let s = styled("p-4 p-2 bg-red-500 bg-blue-500");
    assert_eq!(s.layout.padding.top, taffy::LengthPercentage::length(8.0));
    assert_eq!(s.view.background_color, Color(0x2b7f_ffff));
    // Variants apply after plain utilities, whatever the order.
    let (s, _) = styled_with(
        "active:bg-red-500 bg-blue-500",
        ClassEnvironment::default(),
        (390.0, 844.0),
        state::PRESSED,
    );
    assert_eq!(s.view.background_color, Color(0xfb2c_36ff));
}

#[test]
fn colors_and_opacity() {
    let s = styled("bg-white/50 text-zinc-900 border border-[#ff0000] opacity-75");
    assert_eq!(s.view.background_color, Color(0xffff_ff80));
    assert_eq!(s.text.color, Some(Color(0x1818_1bff)));
    assert_eq!(s.view.borders.widths.to_array().map(|w| w.0), [1.0; 4]);
    assert_eq!(s.view.borders.colors[0], Color(0xff00_00ff));
    assert_eq!(s.view.opacity, 0.75);
    // `border-none` is `border-style: none`: no border, in any order, and a
    // later style shows it again.
    for classes in [
        "border-2 border-none",
        "border-none border-2",
        "border-hidden border-t-4",
    ] {
        let s = styled(classes);
        assert!(!s.view.borders.is_visible(), "{classes}");
        assert_eq!(s.layout.border.top, taffy::LengthPercentage::length(0.0));
    }
    let (env, states) = cx(0);
    let (s, _) = styled_with(
        "border border-none md:border-solid",
        env,
        (800.0, 844.0),
        states,
    );
    assert_eq!(s.view.borders.widths.to_array().map(|w| w.0), [1.0; 4]);
    let s = styled("bg-[rgb(0_0_255)]/25 text-black/[0.5]");
    assert_eq!(s.view.background_color, Color(0x0000_ff40));
    assert_eq!(s.text.color, Some(Color(0x0000_0080)));
}

#[test]
fn theme_tokens() {
    let ui = ui_theme();
    let dark = ClassEnvironment {
        dark: true,
        ..Default::default()
    };
    let s = styled("bg-primary text-muted-foreground border border-border rounded-(--radius)");
    assert_eq!(
        s.view.background_color,
        Color::from(ui.color("primary").unwrap().light)
    );
    assert_eq!(
        s.text.color,
        Some(Color::from(ui.color("muted-foreground").unwrap().light))
    );
    assert_eq!(
        s.view.borders.colors[0],
        Color::from(ui.color("border").unwrap().light)
    );
    assert_eq!(s.view.border_radius, px(ui.number("radius").unwrap()));
    let (s, _) = styled_with(
        "bg-primary text-(--muted-foreground)",
        dark,
        (390.0, 844.0),
        0,
    );
    assert_eq!(
        s.view.background_color,
        Color::from(ui.color("primary").unwrap().dark)
    );
    assert_eq!(
        s.text.color,
        Some(Color::from(ui.color("muted-foreground").unwrap().dark))
    );
    // Opacity modifiers, longer prefixes, variants, the long form.
    let s = styled("bg-primary/50 border-t-2 border-t-ring");
    assert_eq!(
        s.view.background_color,
        Color::from(ui.color("primary").unwrap().light).with_alpha(0.5)
    );
    assert_eq!(
        s.view.borders.colors[0],
        Color::from(ui.color("ring").unwrap().light)
    );
    // Parts and props: shadows, rings, gradients, the caret, placeholders.
    let (s, props) = styled_with(
        "shadow-md shadow-primary ring-2 ring-ring bg-linear-to-r from-primary to-accent caret-primary placeholder:text-muted-foreground",
        ClassEnvironment::default(),
        (390.0, 844.0),
        0,
    );
    assert_eq!(
        props.caret_color,
        Some(Color::from(ui.color("primary").unwrap().light))
    );
    assert_eq!(
        props.placeholder_color,
        Some(Color::from(ui.color("muted-foreground").unwrap().light))
    );
    let e = s.view.effects.as_ref().unwrap();
    assert!(
        e.box_shadows
            .iter()
            .any(|l| l.color == Color::from(ui.color("ring").unwrap().light))
    );
    // The shadow's color keeps each layer's alpha.
    assert!(
        e.box_shadows
            .iter()
            .any(|l| l.color.0 >> 8 == Color::from(ui.color("primary").unwrap().light).0 >> 8)
    );
    let g = &e.background_images[0];
    assert_eq!(
        g.stops[0].color,
        Color::from(ui.color("primary").unwrap().light)
    );
    let accent = ui.color("accent").unwrap();
    assert_eq!(g.stops.last().unwrap().color, Color::from(accent.light));
    let (s, _) = styled_with("dark:bg-accent", dark, (390.0, 844.0), 0);
    assert_eq!(s.view.background_color, Color::from(accent.dark));
    let s = styled("text-(length:--font-size) rounded-(--radius-lg)");
    assert_eq!(s.text.font_size, Some(px(ui.number("font-size").unwrap())));
    assert_eq!(s.view.border_radius, px(ui.number("radius-lg").unwrap()));
    // Later utilities still win, in both appearances.
    let (s, _) = styled_with("bg-primary bg-red-500", dark, (390.0, 844.0), 0);
    assert_eq!(s.view.background_color, Color(0xfb2c_36ff));
    let list = parse(&themed(), "bg-primary p-4");
    assert_ne!(list.deps() & deps::THEME, 0);
    assert_ne!(list.deps() & deps::DARK, 0);
    assert_eq!(parse(&themed(), "bg-red-500").deps() & deps::THEME, 0);
    // Names that are no token, and variables there are not.
    let list = parse(&themed(), "bg-nope text-(--nope) w-primary");
    assert_eq!(list.unknown().len(), 3);
}

#[test]
fn tailwind_colors_win_over_tokens_and_tokens_come_from_the_theme() {
    let mut theme = Theme::from_json(r##"{ "colors": { "primary": "#ff0000" } }"##).unwrap();
    let mut ui = crate::element::Tokens::default();
    ui.set_number("radius-card", 10.0);
    ui.set_color(
        "chart-up",
        crate::element::AdaptiveColor::rgb(0x00ff00, 0x008800),
    );
    ui.set_color(
        "chart-grid",
        crate::element::AdaptiveColor::rgb(0x0000ff, 0x0000ff),
    );
    theme.set_tokens(&ui);
    let group = |_: &str| 0;
    let apply = |classes: &str, dark: bool| {
        let list = parse(&theme, classes);
        assert!(list.unknown().is_empty(), "{:?}", list.unknown());
        let mut style = Style::default();
        let context = Context {
            env: ClassEnvironment {
                dark,
                ..Default::default()
            },
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &group,
        };
        list.apply(
            &context,
            &theme,
            &mut style,
            &mut ClassProps::default(),
            |_| {},
        );
        style
    };
    assert_eq!(
        apply("bg-primary", false).view.background_color,
        Color(0xff00_00ff)
    );
    assert_eq!(
        apply("bg-chart-up", false).view.background_color,
        Color(0x00ff_00ff)
    );
    assert_eq!(
        apply("bg-chart-up", true).view.background_color,
        Color(0x0088_00ff)
    );
    // A color set in one mode only is used in both.
    let s = apply("border border-chart-grid rounded-(--radius-card)", true);
    assert_eq!(s.view.borders.colors[0], Color(0x0000_ffff));
    assert_eq!(s.view.border_radius, px(10.0));
}

#[test]
fn typography() {
    let s = styled(
        "text-lg font-semibold italic tracking-wide leading-tight underline decoration-wavy decoration-red-500 uppercase text-center tabular-nums",
    );
    assert_eq!(s.text.font_size, Some(px(18.0)));
    assert_eq!(s.text.font_weight, Some(600));
    assert_eq!(s.text.italic, Some(true));
    assert_eq!(s.text.letter_spacing_em, Some(0.025));
    assert_eq!(s.text.line_height_multiple, Some(1.25));
    assert!(s.text.decoration.unwrap().underline);
    assert_eq!(
        s.text.decoration_style,
        Some(crate::style::DecorationStyle::Wavy)
    );
    assert_eq!(s.text.decoration_color, Some(Color(0xfb2c_36ff)));
    assert_eq!(s.text.text_align, Some(crate::style::TextAlign::Center));
    assert!(
        s.text
            .font_variant
            .unwrap()
            .has(crate::style::FontVariant::TABULAR_NUMS)
    );
    // A unitless arbitrary line height is a multiple of the font size, as
    // in CSS (`line-height: 1.5`); lengths are points.
    let s = styled("text-base leading-[1.5]");
    assert_eq!(s.text.line_height_multiple, Some(1.5));
    assert_eq!(s.text.line_height, None);
    let s = styled("text-sm/[1.25]");
    assert_eq!(s.text.line_height_multiple, Some(1.25));
    assert_eq!(styled("leading-[150%]").text.line_height_em, Some(1.5));
    assert_eq!(styled("leading-[2em]").text.line_height_em, Some(2.0));
    assert_eq!(styled("leading-[22px]").text.line_height, Some(px(22.0)));
    assert_eq!(styled("leading-6").text.line_height, Some(px(24.0)));
    // `leading-*` wins over a size's own line height whatever the order
    // or the variant (`line-height: var(--tw-leading, ...)`).
    let s = styled("leading-tight text-lg");
    assert_eq!(s.text.font_size, Some(px(18.0)));
    assert_eq!(s.text.line_height_multiple, Some(1.25));
    let (env, states) = cx(0);
    let (s, _) = styled_with("leading-tight md:text-lg", env, (800.0, 844.0), states);
    assert_eq!(s.text.line_height_multiple, Some(1.25));
    assert_eq!(
        styled("text-lg").text.line_height_multiple,
        Some(1.75 / 1.125)
    );
    // An explicit `/<line>` is the size utility's own line height.
    assert_eq!(styled("text-lg/6").text.line_height, Some(px(24.0)));
    // `normal-nums` resets `font-variant-numeric` only.
    let s = styled("small-caps tabular-nums normal-nums");
    assert_eq!(
        s.text.font_variant,
        Some(crate::style::FontVariant(
            crate::style::FontVariant::SMALL_CAPS
        ))
    );
    let s = styled("text-sm/6 font-mono");
    assert_eq!(s.text.font_size, Some(px(14.0)));
    assert_eq!(s.text.line_height, Some(px(24.0)));
    assert_eq!(s.text.font_family.unwrap().name().as_ref(), "monospace");
    let (_, props) = styled_with("truncate", ClassEnvironment::default(), (390.0, 844.0), 0);
    assert_eq!(props.number_of_lines, Some(1));
    assert_eq!(props.no_wrap, Some(true));
}

#[test]
fn shadows_and_rings() {
    let s = styled("shadow-md");
    let e = s.view.effects.as_ref().unwrap();
    assert_eq!(e.box_shadows.len(), 2);
    assert_eq!(e.box_shadows[0].offset_y, px(4.0));
    assert_eq!(e.box_shadows[0].spread, px(-1.0));
    let s = styled("shadow-lg shadow-blue-500/50 ring-2 ring-offset-2 ring-red-500");
    let e = s.view.effects.as_ref().unwrap();
    // Ring offset, ring, then the two shadow layers.
    assert_eq!(e.box_shadows.len(), 4);
    assert_eq!(e.box_shadows[0].spread, px(2.0));
    assert_eq!(e.box_shadows[0].color, Color(0xffff_ffff));
    assert_eq!(e.box_shadows[1].spread, px(4.0));
    assert_eq!(e.box_shadows[1].color, Color(0xfb2c_36ff));
    assert_eq!(e.box_shadows[2].color, Color(0x2b7f_ff80));
    // A color replaces each layer's, alpha too (Tailwind's
    // `--tw-shadow-color` at `--tw-shadow-alpha`, 100%).
    let s = styled("shadow-sm shadow-red-500");
    let e = s.view.effects.as_ref().unwrap();
    assert_eq!(e.box_shadows[0].color, Color(0xfb2c_36ff));
    assert!(styled("shadow-md shadow-none").view.effects.is_none());
    // A size's modifier is the layers' alpha, and the color's.
    let s = styled("shadow-xl/20");
    let e = s.view.effects.as_ref().unwrap();
    assert_eq!(e.box_shadows[0].color, Color(0x0000_0033));
    let s = styled("shadow-red-500/50 shadow-xl/20");
    let e = s.view.effects.as_ref().unwrap();
    assert_eq!(e.box_shadows[0].color, Color(0xfb2c_361a));
    // Arbitrary layers take colors and modifiers too.
    let s = styled("shadow-[0_35px_35px_red]/20");
    let e = s.view.effects.as_ref().unwrap();
    assert_eq!(e.box_shadows[0].color, Color(0xff00_0033));
    // `currentColor` everywhere Tailwind has `-current`.
    let s =
        styled("text-red-500 shadow-md shadow-current/50 ring-2 ring-offset-2 ring-offset-current");
    let e = s.view.effects.as_ref().unwrap();
    let red = Color(0xfb2c_36ff);
    assert_eq!(e.box_shadows[0].color, red);
    assert_eq!(e.box_shadows[2].color, red.with_alpha(0.5));
    let s = styled("text-red-500 bg-current/50 decoration-current");
    assert_eq!(s.view.background_color, red.with_alpha(0.5));
    assert_eq!(s.text.decoration_color, Some(red));
    let s = styled("text-red-500 drop-shadow-md drop-shadow-current");
    match s.view.effects.as_ref().unwrap().filters[0] {
        Filter::DropShadow { color, .. } => assert_eq!(color, red),
        f => panic!("{f:?}"),
    }
    let s = styled("drop-shadow-blue-500/50 drop-shadow-lg");
    match s.view.effects.as_ref().unwrap().filters[0] {
        Filter::DropShadow { color, .. } => assert_eq!(color, Color(0x2b7f_ff80)),
        f => panic!("{f:?}"),
    }
}

#[test]
fn gradients() {
    let s = styled("bg-linear-to-r from-blue-500 via-purple-500 via-40% to-pink-500");
    let g = &s.view.effects.as_ref().unwrap().background_images[0];
    assert_eq!(g.kind, GradientKind::Linear { angle: deg(90.0) });
    // Tailwind's stops, and between them stops sampled in Oklab (Tailwind
    // writes `in oklab`).
    let blue = Color(0x2b7f_ffff);
    let purple = Color(0xad46_ffff);
    let pink = Color(0xf633_9aff);
    let at = |stop: &crate::effects::ColorStop| match stop.position {
        Some(DefiniteLength::Fraction(f)) => f.0,
        _ => panic!("placed by fractions"),
    };
    assert!(g.stops.len() > 3);
    assert_eq!((g.stops[0].color, at(&g.stops[0])), (blue, 0.0));
    let via = g.stops.iter().position(|s| s.color == purple).unwrap();
    assert!((at(&g.stops[via]) - 0.4).abs() < 1e-6);
    let last = g.stops.last().unwrap();
    assert_eq!((last.color, at(last)), (pink, 1.0));
    assert!(g.stops.windows(2).all(|w| at(&w[0]) < at(&w[1])));
    // Halfway from blue to purple, the Oklab mix (CSS's `color-mix(in
    // oklab, ...)`), not the sRGB one.
    let mid = g
        .stops
        .iter()
        .find(|s| (at(s) - 0.2).abs() < 1e-6)
        .expect("a stop halfway");
    assert_eq!(
        mid.color,
        crate::css::color("color-mix(in oklab, #2b7fff, #ad46ff)").unwrap()
    );
    let s = styled("bg-linear-to-br from-red-500");
    let g = &s.view.effects.as_ref().unwrap().background_images[0];
    assert!(matches!(
        g.kind,
        GradientKind::LinearCorner {
            right: true,
            bottom: true
        }
    ));
    assert_eq!(g.stops[1].color, Color::TRANSPARENT);
    let s = styled("bg-[linear-gradient(45deg,red,blue)]");
    let g = &s.view.effects.as_ref().unwrap().background_images[0];
    assert_eq!(g.kind, GradientKind::Linear { angle: deg(45.0) });
    assert_eq!(g.stops.len(), 2);
    // At an arbitrary position Tailwind writes no `in oklab`: CSS's default,
    // Oklab with the palette's (OKLCH) colors, sRGB with legacy ones.
    let stops = |classes: &str| {
        styled(classes).view.effects.unwrap().background_images[0]
            .stops
            .len()
    };
    assert!(stops("bg-linear-[25deg] from-red-500 to-blue-500") > 2);
    assert_eq!(stops("bg-linear-[25deg] from-[#f00] to-[rgb(0_0_255)]"), 2);
    assert!(stops("bg-linear-[25deg] from-[#f00] to-[oklch(60%_0.2_250)]") > 2);
    assert!(stops("bg-linear-45 from-[#f00] to-[#00f]") > 2);
    assert!(stops("bg-conic from-[#f00] to-[#00f]") > 2);
    assert_eq!(stops("bg-conic-[90deg] from-[#f00] to-[#00f]"), 2);
    let s = styled("bg-[repeating-linear-gradient(90deg,red_0px,blue_10px)]");
    let g = &s.view.effects.as_ref().unwrap().background_images[0];
    assert!(g.repeating);
    assert_eq!(g.stops[1].position, Some(DefiniteLength::from(px(10.0))));
}

#[test]
fn state_backgrounds_show_over_gradients() {
    let classes = "bg-linear-to-b from-zinc-700 to-zinc-800 active:bg-zinc-600";
    let pressed = |classes| {
        let (env, _) = cx(0);
        styled_with(classes, env, (390.0, 844.0), state::PRESSED).0
    };
    assert!(
        !styled(classes)
            .view
            .effects
            .as_ref()
            .unwrap()
            .background_images
            .is_empty()
    );
    let s = pressed(classes);
    assert!(s.view.effects.is_none());
    assert_eq!(s.view.background_color, Color(0x52_52_5c_ff));
    let s = pressed("text-red-500 bg-linear-to-b from-zinc-700 active:bg-current");
    assert!(s.view.effects.is_none());
    assert_eq!(s.view.background_color, s.text.color.unwrap());
    // A state that sets its own gradient keeps it.
    let s = pressed("bg-linear-to-b from-zinc-700 active:bg-zinc-600 active:from-zinc-500");
    assert_eq!(s.view.effects.as_ref().unwrap().background_images.len(), 1);
    // A later plain color is not feedback.
    let env = ClassEnvironment {
        dark: true,
        ..Default::default()
    };
    let (s, _) = styled_with(
        "bg-linear-to-b from-zinc-700 active:bg-zinc-600 dark:bg-black",
        env,
        (390.0, 844.0),
        state::PRESSED,
    );
    assert_eq!(s.view.effects.as_ref().unwrap().background_images.len(), 1);
    // Plain colors keep CSS's order: the gradient is drawn over them.
    let s = styled("bg-zinc-600 bg-linear-to-b from-zinc-700");
    assert_eq!(s.view.effects.as_ref().unwrap().background_images.len(), 1);
}

#[test]
fn transforms() {
    let s = styled("-translate-x-1/2 translate-y-2 rotate-45 scale-110");
    let ops = s.view.transform.ops();
    assert_eq!(ops[0].kind, TransformKind::TranslateXFraction);
    assert_eq!(ops[0].value, -0.5);
    assert_eq!(ops[1].kind, TransformKind::TranslateY);
    assert_eq!(ops[1].value, 8.0);
    assert_eq!(ops[2].kind, TransformKind::Rotate);
    assert_eq!(ops[3].kind, TransformKind::Scale);
    assert!((ops[3].value - 1.1).abs() < 1e-6);
    let t = s
        .view
        .transform
        .resolve_fractions(crate::geometry::size(px(100.0), px(40.0)));
    assert_eq!(t.ops()[0].kind, TransformKind::TranslateX);
    assert_eq!(t.ops()[0].value, -50.0);
    let kinds = |classes: &str| {
        styled(classes)
            .view
            .transform
            .ops()
            .iter()
            .map(|op| (op.kind, op.value))
            .collect::<Vec<_>>()
    };
    // `transform-none` is `transform: none`: the 3D rotations and the
    // skews; `translate`, `rotate` and `scale` are properties of their own.
    assert_eq!(
        kinds("translate-x-2 rotate-12 scale-90 rotate-x-45 skew-x-6 transform-none"),
        [
            (TransformKind::TranslateX, 8.0),
            (TransformKind::Rotate, 12.0),
            (TransformKind::Scale, 0.9),
        ]
    );
    assert_eq!(
        kinds("translate-x-2 rotate-12 scale-90 rotate-x-45 translate-none scale-none rotate-none"),
        [(TransformKind::RotateX, 45.0)]
    );
    // `rotate-z-*` is `rotateZ()` in `transform`, after `rotateX()` and
    // `rotateY()`, and composes with `rotate-*` (CSS's `rotate`, applied
    // after the transform).
    assert_eq!(
        kinds("rotate-z-30 rotate-x-45 rotate-12 scale-50"),
        [
            (TransformKind::Rotate, 12.0),
            (TransformKind::Scale, 0.5),
            (TransformKind::RotateX, 45.0),
            (TransformKind::Rotate, 30.0),
        ]
    );
    // `perspective-*` is CSS's `perspective`: its children's depth, no
    // step of its own transform.
    let s = styled("perspective-near rotate-x-45");
    assert_eq!(s.perspective, Some(px(300.0)));
    assert_eq!(
        kinds("perspective-near rotate-x-45"),
        [(TransformKind::RotateX, 45.0)]
    );
    // Every step Tailwind composes fits.
    assert_eq!(
        kinds(
            "translate-x-1 translate-y-2 rotate-3 scale-x-50 scale-y-75 rotate-x-6 rotate-y-12 rotate-z-45 skew-x-1 skew-y-2"
        )
        .len(),
        10
    );
}

#[test]
fn corner_shapes() {
    assert!(styled("rounded-xl corner-squircle").view.continuous_corners);
    assert!(
        !styled("corner-squircle corner-round")
            .view
            .continuous_corners
    );
}

#[test]
fn effects_and_interaction() {
    let s = styled(
        "blur-sm brightness-125 backdrop-blur-md mix-blend-multiply outline-2 outline-offset-2 outline-blue-500 invisible cursor-pointer",
    );
    let e = s.view.effects.as_ref().unwrap();
    assert_eq!(
        e.filters,
        vec![
            crate::effects::Filter::Blur(px(8.0)),
            crate::effects::Filter::Brightness(1.25)
        ]
    );
    assert_eq!(
        e.backdrop_filters,
        vec![crate::effects::Filter::Blur(px(12.0))]
    );
    assert_eq!(s.view.blend_mode, BlendMode::Multiply);
    // Every layer of a drop shadow is a `drop-shadow()` of the chain, after
    // the other filters, as Tailwind's `--tw-drop-shadow`.
    use crate::effects::Filter;
    let filters = |classes| {
        styled(classes)
            .view
            .effects
            .map(|e| e.filters.clone())
            .unwrap_or_default()
    };
    let drop = |y: f32, blur: f32, alpha: u32| Filter::DropShadow {
        offset_x: px(0.0),
        offset_y: px(y),
        blur: px(blur),
        color: Color(alpha),
    };
    assert_eq!(
        filters("drop-shadow blur-sm"),
        vec![
            Filter::Blur(px(8.0)),
            drop(1.0, 2.0, 0x1a),
            drop(1.0, 1.0, 0x0f)
        ]
    );
    assert_eq!(filters("drop-shadow-md").len(), 1);
    assert_eq!(
        filters("drop-shadow-[0_1px_1px_#000,0_2px_4px_#f00]").len(),
        2
    );
    assert!(filters("drop-shadow drop-shadow-none").is_empty());
    assert_eq!(s.view.outline.width, px(2.0));
    assert_eq!(s.view.outline.offset, px(2.0));
    assert!(s.view.hidden);
    // `outline-<n>` is `outline-style: var(--tw-outline-style)`: solid
    // unless a style utility is anywhere in the string.
    use crate::style::BorderStyle;
    let outline = |classes| {
        let o = styled(classes).view.outline;
        (o.style, o.width)
    };
    assert_eq!(outline("outline-2"), (BorderStyle::Solid, px(2.0)));
    assert_eq!(outline("outline"), (BorderStyle::Solid, px(1.0)));
    assert_eq!(
        outline("outline-dashed outline-2"),
        (BorderStyle::Dashed, px(2.0))
    );
    assert_eq!(
        outline("outline-2 outline-dotted"),
        (BorderStyle::Dotted, px(2.0))
    );
    assert_eq!(outline("outline-none outline-2").1, px(0.0));
    assert_eq!(outline("outline-2 outline-hidden").1, px(0.0));
    // A style alone is CSS's initial `medium` width.
    assert_eq!(outline("outline-dashed"), (BorderStyle::Dashed, px(3.0)));
    assert_eq!(outline("outline-solid"), (BorderStyle::Solid, px(3.0)));
    let (env, states) = cx(state::FOCUSED);
    let (focused, _) = styled_with(
        "outline-2 focus:outline-dashed",
        env,
        (390.0, 844.0),
        states,
    );
    assert_eq!(
        (focused.view.outline.style, focused.view.outline.width),
        (BorderStyle::Dashed, px(2.0))
    );
    assert_eq!(s.view.cursor, Cursor::Pointer);
}

#[test]
fn diagonal_resize_cursors_keep_their_axes() {
    use crate::style::StyleValue;

    for (name, expected) in [
        ("nwse-resize", Cursor::NwseResize),
        ("nw-resize", Cursor::NwseResize),
        ("se-resize", Cursor::NwseResize),
        ("nesw-resize", Cursor::NeswResize),
        ("ne-resize", Cursor::NeswResize),
        ("sw-resize", Cursor::NeswResize),
        ("ew-resize", Cursor::ColResize),
        ("ns-resize", Cursor::RowResize),
        ("move", Cursor::Move),
    ] {
        let mut inline = Style::default();
        inline.set("cursor", StyleValue::Str(name)).unwrap();
        assert_eq!(inline.view.cursor, expected, "inline {name}");
        assert_eq!(
            styled(&format!("cursor-{name}")).view.cursor,
            expected,
            "{name}"
        );
        assert_eq!(
            styled(&format!("cursor-[{name}]")).view.cursor,
            expected,
            "arbitrary {name}"
        );
        assert_eq!(Cursor::parse(expected.name()), Some(expected));
    }
}

#[test]
fn transitions_and_animations() {
    let s = styled("transition-colors duration-300 ease-out delay-75");
    let t = s.view.transition.unwrap();
    assert_eq!(t.duration, 300.0);
    assert_eq!(t.delay, 75.0);
    assert_eq!(t.easing, [0.0, 0.0, 0.2, 1.0]);
    assert!(t.properties.has(crate::effects::TransitionProps::COLORS));
    assert!(!t.properties.has(crate::effects::TransitionProps::TRANSFORM));
    let s = styled("transition");
    assert_eq!(s.view.transition.unwrap().duration, 150.0);
    let s = styled("animate-spin");
    assert_eq!(s.view.animation.unwrap().keyframes, Keyframes::Spin);
}

#[test]
fn display() {
    // As in CSS, a flex container is a row unless a direction utility says
    // otherwise, wherever that utility is in the string.
    let s = styled("flex");
    assert_eq!(s.layout.display, taffy::Display::Flex);
    assert_eq!(s.layout.flex_direction, taffy::FlexDirection::Row);
    assert_eq!(
        styled("inline-flex").layout.flex_direction,
        taffy::FlexDirection::Row
    );
    assert_eq!(
        styled("flex-col flex").layout.flex_direction,
        taffy::FlexDirection::Column
    );
    assert_eq!(
        styled("flex flex-col").layout.flex_direction,
        taffy::FlexDirection::Column
    );
    assert_eq!(
        styled("flex-col-reverse flex").layout.flex_direction,
        taffy::FlexDirection::ColumnReverse
    );
    // Without `flex`, views keep React Native's column.
    assert_eq!(
        styled("items-center").layout.flex_direction,
        taffy::FlexDirection::Column
    );
    // A variant `flex` is a row where it applies; a plain direction still wins.
    let (env, states) = cx(0);
    let wide = |c| styled_with(c, env, (800.0, 844.0), states).0;
    assert_eq!(
        wide("hidden md:flex").layout.flex_direction,
        taffy::FlexDirection::Row
    );
    assert_eq!(
        wide("flex-col md:flex").layout.flex_direction,
        taffy::FlexDirection::Column
    );
    assert_eq!(
        styled("hidden md:flex").layout.display,
        taffy::Display::None
    );
    assert_eq!(
        styled("hidden md:flex").layout.flex_direction,
        taffy::FlexDirection::Column
    );
    // Block formatting contexts use Taffy's block layout.
    assert_eq!(styled("block").layout.display, taffy::Display::Block);
    assert_eq!(styled("flow-root").layout.display, taffy::Display::FlowRoot);
    // Inline boxes in a flex or grid parent are blockified, as a browser
    // computes them (`display: inline-block` -> `block`).
    assert_eq!(styled("inline-block").layout.display, taffy::Display::Block);
    assert_eq!(styled("inline").layout.display, taffy::Display::Block);
    assert_eq!(
        wide("hidden md:inline-block").layout.display,
        taffy::Display::Block
    );
}

#[test]
fn grid() {
    let s = styled("grid grid-cols-3 gap-2 col-span-2");
    assert_eq!(s.layout.display, taffy::Display::Grid);
    assert_eq!(s.layout.grid_template_columns.len(), 1);
    assert_eq!(s.layout.grid_column.start, taffy::GridPlacement::Span(2));
    // Line names.
    let s = styled("grid grid-cols-[[a]_40px_[b]_1fr] col-[a/b] row-start-[main]");
    assert_eq!(s.layout.grid_template_columns.len(), 2);
    assert_eq!(s.layout.grid_template_column_names.len(), 3);
    let named = |n: &str| taffy::GridPlacement::NamedLine(n.to_owned(), 0);
    assert_eq!(s.layout.grid_column.start, named("a"));
    assert_eq!(s.layout.grid_column.end, named("b"));
    assert_eq!(s.layout.grid_row.start, named("main"));
    // Invalid arbitrary placements and tracks stay known classes that set
    // nothing (the style logs them); a negative named line is unknown.
    let theme = themed();
    let list = parse(
        &theme,
        "col-[1_2] row-start-[1a] auto-rows-[3zz] -col-start-[a]",
    );
    assert_eq!(list.unknown().len(), 2, "{:?}", list.unknown());
    let s = styled("col-[1_2] auto-rows-[3zz]");
    assert_eq!(s.layout.grid_column, Style::default().layout.grid_column);
    assert_eq!(
        s.layout.grid_auto_rows,
        Style::default().layout.grid_auto_rows
    );
}

/// Each alignment class against the declaration Tailwind CSS (v4.3)
/// generates for it, in the comment: the same property, the same value.
#[test]
fn alignment_follows_tailwind_css() {
    use taffy::{AlignContent as C, AlignItems as I};
    let content: &[(&str, C, C)] = &[
        // (class, align-content, justify-content)
        ("justify-start", C::NORMAL, C::FLEX_START), // justify-content: flex-start
        ("justify-end", C::NORMAL, C::FLEX_END),     // justify-content: flex-end
        ("justify-center", C::NORMAL, C::CENTER),    // justify-content: center
        ("justify-between", C::NORMAL, C::SPACE_BETWEEN), // justify-content: space-between
        ("justify-around", C::NORMAL, C::SPACE_AROUND), // justify-content: space-around
        ("justify-evenly", C::NORMAL, C::SPACE_EVENLY), // justify-content: space-evenly
        ("justify-stretch", C::NORMAL, C::STRETCH),  // justify-content: stretch
        ("justify-normal", C::NORMAL, C::NORMAL),    // justify-content: normal
        // align-content: normal, which is `stretch` in flex and grid
        // containers (the style's `normal` is the unset value).
        ("content-normal", C::STRETCH, C::NORMAL),
        ("content-start", C::FLEX_START, C::NORMAL), // align-content: flex-start
        ("content-end", C::FLEX_END, C::NORMAL),     // align-content: flex-end
        ("content-center", C::CENTER, C::NORMAL),    // align-content: center
        ("content-between", C::SPACE_BETWEEN, C::NORMAL), // align-content: space-between
        ("content-around", C::SPACE_AROUND, C::NORMAL), // align-content: space-around
        ("content-evenly", C::SPACE_EVENLY, C::NORMAL), // align-content: space-evenly
        ("content-stretch", C::STRETCH, C::NORMAL),  // align-content: stretch
        ("place-content-center", C::CENTER, C::CENTER), // place-content: center
        ("place-content-start", C::START, C::START), // place-content: start
        ("place-content-end", C::END, C::END),       // place-content: end
        ("place-content-between", C::SPACE_BETWEEN, C::SPACE_BETWEEN), /* place-content: space-between */
        ("place-content-stretch", C::STRETCH, C::STRETCH),             // place-content: stretch
    ];
    for &(class, align, justify) in content {
        let s = styled(class);
        assert_eq!(
            (s.layout.align_content, s.layout.justify_content),
            (align, justify),
            "{class}"
        );
    }
    let items: &[(&str, I, I)] = &[
        // (class, align-items, justify-items)
        ("items-start", I::FLEX_START, I::NORMAL), // align-items: flex-start
        ("items-end", I::FLEX_END, I::NORMAL),     // align-items: flex-end
        ("items-center", I::CENTER, I::NORMAL),    // align-items: center
        ("items-baseline", I::BASELINE, I::NORMAL), // align-items: baseline
        ("items-stretch", I::STRETCH, I::NORMAL),  // align-items: stretch
        ("justify-items-start", I::NORMAL, I::START), // justify-items: start
        ("justify-items-end", I::NORMAL, I::END),  // justify-items: end
        ("justify-items-center", I::NORMAL, I::CENTER), // justify-items: center
        ("justify-items-stretch", I::NORMAL, I::STRETCH), // justify-items: stretch
        ("justify-items-normal", I::NORMAL, I::NORMAL), // justify-items: normal
        ("place-items-center", I::CENTER, I::CENTER), // place-items: center
        ("place-items-start", I::START, I::START), // place-items: start
        ("place-items-end", I::END, I::END),       // place-items: end
        ("place-items-stretch", I::STRETCH, I::STRETCH), // place-items: stretch
    ];
    for &(class, align, justify) in items {
        let s = styled(class);
        assert_eq!(
            (s.layout.align_items, s.layout.justify_items),
            (align, justify),
            "{class}"
        );
    }
    let selves: &[(&str, Option<I>, Option<I>)] = &[
        // (class, align-self, justify-self); `auto` is unset.
        ("self-auto", None, None),                  // align-self: auto
        ("self-start", Some(I::FLEX_START), None),  // align-self: flex-start
        ("self-end", Some(I::FLEX_END), None),      // align-self: flex-end
        ("self-center", Some(I::CENTER), None),     // align-self: center
        ("self-stretch", Some(I::STRETCH), None),   // align-self: stretch
        ("self-baseline", Some(I::BASELINE), None), // align-self: baseline
        ("justify-self-auto", None, None),          // justify-self: auto
        ("justify-self-start", None, Some(I::FLEX_START)), // justify-self: flex-start
        ("justify-self-end", None, Some(I::FLEX_END)), // justify-self: flex-end
        ("justify-self-center", None, Some(I::CENTER)), // justify-self: center
        ("justify-self-stretch", None, Some(I::STRETCH)), // justify-self: stretch
        ("place-self-auto", None, None),            // place-self: auto
        ("place-self-start", Some(I::START), Some(I::START)), // place-self: start
        ("place-self-end", Some(I::END), Some(I::END)), // place-self: end
        ("place-self-center", Some(I::CENTER), Some(I::CENTER)), // place-self: center
        ("place-self-stretch", Some(I::STRETCH), Some(I::STRETCH)), // place-self: stretch
    ];
    for &(class, align, justify) in selves {
        let s = styled(class);
        assert_eq!(
            (s.layout.align_self, s.layout.justify_self),
            (align, justify),
            "{class}"
        );
    }
}

/// Grid utilities as Tailwind CSS generates them: a type hint is dropped
/// from the value (`col-[number:2]` is `grid-column: 2`), and the classes
/// it has no negative of are unknown.
#[test]
fn grid_utilities_follow_tailwind_css() {
    let s = styled("col-[number:2] row-span-[number:3] auto-cols-[length:13px]");
    assert_eq!(
        s.layout.grid_column.start,
        taffy::GridPlacement::Line(2.into())
    );
    assert_eq!(s.layout.grid_row.start, taffy::GridPlacement::Span(3));
    assert_eq!(s.layout.grid_auto_columns.len(), 1);
    // `-col-[2]` is `grid-column: calc(2 * -1)`.
    assert_eq!(
        styled("-col-[2]").layout.grid_column.start,
        taffy::GridPlacement::Line((-2).into())
    );
    // A number is no track size: the class sets nothing, as a browser
    // drops `grid-auto-rows: 2`.
    let s = styled("auto-rows-[number:2]");
    assert_eq!(
        s.layout.grid_auto_rows,
        Style::default().layout.grid_auto_rows
    );
    let theme = themed();
    let negatives = "-grid-cols-2 -grid-rows-[13px] -col-span-2 -row-span-1 -auto-cols-[13px] -auto-rows-fr -col-auto";
    let list = parse(&theme, negatives);
    assert_eq!(list.unknown().len(), 7, "{:?}", list.unknown());
}

#[test]
fn variants() {
    let theme = themed();
    let list = parse(
        &theme,
        "bg-white dark:bg-black md:p-8 hover:opacity-80 group-active:scale-95 ios:pt-1 android:pt-2 rtl:ml-2",
    );
    assert!(list.unknown().is_empty(), "{:?}", list.unknown());
    let d = list.deps();
    assert!(
        d & deps::DARK != 0 && d & deps::WIDTH != 0 && d & deps::STATE != 0 && d & deps::GROUP != 0
    );
    assert_eq!(list.tracked_states(), state::HOVERED);
    let dark = ClassEnvironment {
        dark: true,
        platform: Platform::Android,
        reduce_motion: false,
    };
    let (s, _) = styled_with("bg-white dark:bg-black", dark, (390.0, 844.0), 0);
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    let (s, _) = styled_with("p-2 md:p-8", ClassEnvironment::default(), (390.0, 844.0), 0);
    assert_eq!(s.layout.padding.top, taffy::LengthPercentage::length(8.0));
    let (s, _) = styled_with(
        "p-2 md:p-8",
        ClassEnvironment::default(),
        (1024.0, 768.0),
        0,
    );
    assert_eq!(s.layout.padding.top, taffy::LengthPercentage::length(32.0));
    let (s, _) = styled_with(
        "p-2 max-md:p-1",
        ClassEnvironment::default(),
        (390.0, 844.0),
        0,
    );
    assert_eq!(s.layout.padding.top, taffy::LengthPercentage::length(4.0));
    let (s, _) = styled_with("pt-1 ios:pt-3 android:pt-5", dark, (390.0, 844.0), 0);
    assert_eq!(s.layout.padding.top, taffy::LengthPercentage::length(20.0));
    let web = ClassEnvironment {
        platform: Platform::Web,
        ..ClassEnvironment::default()
    };
    let (s, _) = styled_with("pt-1 ios:pt-3 web:pt-5", web, (390.0, 844.0), 0);
    assert_eq!(s.layout.padding.top, taffy::LengthPercentage::length(20.0));
    let (s, _) = styled_with(
        "w-screen h-screen",
        ClassEnvironment::default(),
        (390.0, 844.0),
        0,
    );
    assert_eq!(s.layout.size.width, taffy::Dimension::length(390.0));
    assert_eq!(s.layout.size.height, taffy::Dimension::length(844.0));
}

#[test]
fn checked_variants() {
    let theme = themed();
    let list = parse(&theme, "checked:bg-black group-checked:translate-x-4");
    assert!(list.unknown().is_empty(), "{:?}", list.unknown());
    assert!(list.deps() & deps::STATE != 0 && list.deps() & deps::GROUP != 0);
    // The core sets it: nothing for the platform to report.
    assert_eq!(list.tracked_states(), 0);
    let env = ClassEnvironment::default();
    let (s, _) = styled_with(
        "bg-white checked:bg-black",
        env,
        (390.0, 844.0),
        state::CHECKED,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    // A value state: `selected:` and the interaction states go over it.
    let both = state::CHECKED | state::SELECTED;
    let (s, _) = styled_with(
        "selected:bg-white checked:bg-black",
        env,
        (390.0, 844.0),
        both,
    );
    assert_eq!(s.view.background_color, Color(0xffff_ffff));
    let hovered = state::CHECKED | state::HOVERED;
    let (s, _) = styled_with(
        "hover:bg-white checked:bg-black",
        env,
        (390.0, 844.0),
        hovered,
    );
    assert_eq!(s.view.background_color, Color(0xffff_ffff));
}

#[test]
fn expanded_variants() {
    let theme = themed();
    let list = parse(
        &theme,
        "aria-expanded:bg-black group-aria-expanded:rotate-180",
    );
    assert!(list.unknown().is_empty(), "{:?}", list.unknown());
    assert!(list.deps() & deps::STATE != 0 && list.deps() & deps::GROUP != 0);
    // The core sets it: nothing for the platform to report.
    assert_eq!(list.tracked_states(), 0);
    let env = ClassEnvironment::default();
    let (s, _) = styled_with(
        "bg-white aria-expanded:bg-black",
        env,
        (390.0, 844.0),
        state::EXPANDED,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    // Tailwind's `aria-*` variants come after the pseudo-classes: an open
    // trigger keeps its look while hovered or pressed.
    let pressed = state::EXPANDED | state::HOVERED | state::PRESSED;
    let (s, _) = styled_with(
        "hover:bg-white active:bg-white aria-expanded:bg-black",
        env,
        (390.0, 844.0),
        pressed,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
}

/// Applies `classes` to a node with the `data-*` attributes `own`, inside
/// a group (unnamed) with `group`, in `states`.
fn styled_data(classes: &str, own: &[&str], group: &[&str], states: u16) -> Style {
    let theme = themed();
    let list = parse(&theme, classes);
    assert!(list.unknown().is_empty(), "{:?}", list.unknown());
    let mut style = Style::default();
    let mut props = ClassProps::default();
    let groups = |_: &str| 0;
    let data = |g: Option<&str>, name: &str| match g {
        None => own.contains(&name),
        Some("") => group.contains(&name),
        Some(_) => false,
    };
    let context = Context {
        env: ClassEnvironment::default(),
        window: crate::geometry::size_pt(390.0, 844.0),
        rtl: false,
        states,
        position: 0,
        group: &groups,
    };
    list.apply_with(
        &context,
        &data,
        &theme,
        CurrentColor::default(),
        DEFAULT_CH,
        &mut style,
        &mut props,
        |_| {},
    );
    style
}

/// `data-<name>:` and `group-data-<name>:` as Tailwind v4 has them in their
/// boolean form (`&[data-range-start]`, `:where(.group)[data-range-start]
/// &`): the attributes a component's part shows (`project_data`), ordered
/// after the `aria-*` variants and by their names.
#[test]
fn data_variants() {
    let theme = themed();
    let list = parse(
        &theme,
        "data-range-start:bg-black data-range-end:rounded-r-md group-data-range-middle:opacity-50 \
         group-data-today/day:font-bold data-a1-b2:p-1",
    );
    assert!(list.unknown().is_empty(), "{:?}", list.unknown());
    assert!(list.deps() & deps::DATA != 0 && list.deps() & deps::GROUP != 0);
    // Not a state: nothing for the platform to report.
    assert_eq!(list.tracked_states(), 0);
    // Only the boolean form: a value (`data-[state=open]:`), an empty or an
    // upper case name are unknown.
    let unknown = parse(
        &theme,
        "data-[state=open]:bg-black data-:bg-black data-Open:bg-black group-data-[x]:p-1",
    );
    assert_eq!(unknown.unknown().len(), 4, "{:?}", unknown.unknown());
    // A view's own.
    let s = styled_data(
        "bg-white data-range-start:bg-black",
        &["range-start"],
        &[],
        0,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    let s = styled_data("bg-white data-range-start:bg-black", &["range-end"], &[], 0);
    assert_eq!(s.view.background_color, Color(0xffff_ffff));
    // Two: both are needed.
    let both = "bg-white data-range-start:data-range-end:bg-black";
    let s = styled_data(both, &["range-start"], &[], 0);
    assert_eq!(s.view.background_color, Color(0xffff_ffff));
    let s = styled_data(both, &["range-end", "range-start"], &[], 0);
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    // The group's, not the view's own.
    let grouped = "opacity-100 group-data-range-middle:opacity-50";
    let s = styled_data(grouped, &["range-middle"], &[], 0);
    assert_eq!(s.view.opacity, 1.0);
    let s = styled_data(grouped, &[], &["range-middle"], 0);
    assert_eq!(s.view.opacity, 0.5);
    // With a state of the same group.
    let hovered = "opacity-100 group-hover:group-data-range-middle:opacity-50";
    let s = styled_data(hovered, &[], &["range-middle"], 0);
    assert_eq!(s.view.opacity, 1.0);
    // After the `aria-*` variants: a selected day at the start of a range
    // keeps the start's look.
    let s = styled_data(
        "data-range-start:bg-black aria-selected:bg-white selected:bg-white",
        &["range-start"],
        &[],
        state::SELECTED,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    // Among themselves by their names, before `motion-safe:`.
    let s = styled_data("data-b:bg-black data-a:bg-white", &["a", "b"], &[], 0);
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    let s = styled_data("motion-safe:bg-white data-a:bg-black", &["a"], &[], 0);
    assert_eq!(s.view.background_color, Color(0xffff_ffff));
}

/// `aria-pressed:`, `aria-busy:`, `aria-checked:` and `aria-selected:`
/// as Tailwind v4 has them (`&[aria-pressed="true"]`, ...): the core's
/// states from the accessibility state, ordered by their names after the
/// pseudo-classes.
#[test]
fn aria_state_variants() {
    let theme = themed();
    let list = parse(
        &theme,
        "aria-pressed:bg-black aria-busy:opacity-50 aria-checked:p-1 aria-selected:p-2 \
         group-aria-pressed:rotate-180 group-aria-busy:opacity-0 group-aria-checked:p-3 \
         group-aria-selected:p-4",
    );
    assert!(list.unknown().is_empty(), "{:?}", list.unknown());
    assert!(list.deps() & deps::STATE != 0 && list.deps() & deps::GROUP != 0);
    // The core sets them: nothing for the platform to report.
    assert_eq!(list.tracked_states(), 0);
    // `aria-current:` and the states NSH does not have are unknown, as
    // Tailwind has no `aria-current` variant (`aria-[current=page]:` is
    // an arbitrary one).
    let unknown = parse(&theme, "aria-current:bg-black aria-readonly:bg-black");
    assert_eq!(unknown.unknown().len(), 2, "{:?}", unknown.unknown());
    let env = ClassEnvironment::default();
    let at = (390.0, 844.0);
    let (s, _) = styled_with(
        "bg-white aria-pressed:bg-black",
        env,
        at,
        state::ARIA_PRESSED,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    // Not a press in progress: `active:` waits for its own state.
    let (s, _) = styled_with("bg-white aria-pressed:bg-black", env, at, state::PRESSED);
    assert_eq!(s.view.background_color, Color(0xffff_ffff));
    let (s, _) = styled_with(
        "active:bg-white aria-pressed:bg-black",
        env,
        at,
        state::PRESSED,
    );
    assert_eq!(s.view.background_color, Color(0xffff_ffff));
    // The `aria-*` variants come after the pseudo-classes: a toggle that
    // is on keeps its look while pressed or hovered.
    let pressed = state::ARIA_PRESSED | state::HOVERED | state::PRESSED;
    let (s, _) = styled_with(
        "hover:bg-white active:bg-white aria-pressed:bg-black",
        env,
        at,
        pressed,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    // Among themselves, by their names: busy, checked, expanded, pressed,
    // selected.
    let all =
        state::BUSY | state::CHECKED | state::EXPANDED | state::ARIA_PRESSED | state::SELECTED;
    let order = [
        ("aria-busy", Color(0xfb2c_36ff)),
        ("aria-checked", Color(0x0000_00ff)),
        ("aria-expanded", Color(0xffff_ffff)),
        ("aria-pressed", Color(0x00bc_7dff)),
        ("aria-selected", Color(0x2b7f_ffff)),
    ];
    let classes = |order: &[(&str, Color)]| {
        let names = ["red-500", "black", "white", "emerald-500", "blue-500"];
        order
            .iter()
            .zip(names)
            .map(|((v, _), c)| format!("{v}:bg-{c}"))
            .collect::<Vec<_>>()
    };
    let mut reversed = classes(&order);
    reversed.reverse();
    let (s, _) = styled_with(&reversed.join(" "), env, at, all);
    assert_eq!(s.view.background_color, order[4].1);
    for i in 0..4 {
        let below = order[..=i].iter().fold(0, |states, (v, _)| {
            states
                | match *v {
                    "aria-busy" => state::BUSY,
                    "aria-checked" => state::CHECKED,
                    "aria-expanded" => state::EXPANDED,
                    "aria-pressed" => state::ARIA_PRESSED,
                    _ => state::SELECTED,
                }
        });
        let (s, _) = styled_with(&reversed.join(" "), env, at, below);
        assert_eq!(s.view.background_color, order[i].1, "{}", order[i].0);
    }
    // `aria-checked:` is checked, after `checked:`; `aria-selected:` is
    // selected, after `selected:`.
    let (s, _) = styled_with(
        "aria-checked:bg-black checked:bg-white",
        env,
        at,
        state::CHECKED,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    let (s, _) = styled_with(
        "aria-selected:bg-black selected:bg-white",
        env,
        at,
        state::SELECTED,
    );
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
}

#[test]
fn indeterminate_variants() {
    let theme = themed();
    let list = parse(
        &theme,
        "indeterminate:bg-black group-indeterminate:opacity-50",
    );
    assert!(list.unknown().is_empty(), "{:?}", list.unknown());
    assert!(list.deps() & deps::STATE != 0 && list.deps() & deps::GROUP != 0);
    // The core sets it: nothing for the platform to report.
    assert_eq!(list.tracked_states(), 0);
    let env = ClassEnvironment::default();
    let classes = "bg-white checked:bg-black indeterminate:bg-red-500";
    let (s, _) = styled_with(classes, env, (390.0, 844.0), state::INDETERMINATE);
    assert_eq!(s.view.background_color, Color(0xfb2c_36ff));
    // Not checked: `checked:` waits for its own state.
    let (s, _) = styled_with(classes, env, (390.0, 844.0), state::CHECKED);
    assert_eq!(s.view.background_color, Color(0x0000_00ff));
    // A value state: the interaction states go over it.
    let hovered = state::INDETERMINATE | state::HOVERED;
    let (s, _) = styled_with(
        "hover:bg-white indeterminate:bg-black",
        env,
        (390.0, 844.0),
        hovered,
    );
    assert_eq!(s.view.background_color, Color(0xffff_ffff));
}

#[test]
fn groups() {
    let theme = themed();
    let root = parse(&theme, "group/card p-2");
    assert_eq!(root.groups(), ["card".into()] as [Box<str>; 1]);
    let child = parse(&theme, "opacity-100 group-hover/card:opacity-50");
    let mut style = Style::default();
    let mut props = ClassProps::default();
    let group = |name: &str| if name == "card" { state::HOVERED } else { 0 };
    let context = Context {
        env: ClassEnvironment::default(),
        window: crate::geometry::size_pt(390.0, 844.0),
        rtl: false,
        states: 0,
        position: 0,
        group: &group,
    };
    child.apply(&context, &theme, &mut style, &mut props, |_| {});
    assert_eq!(style.view.opacity, 0.5);
    // Tailwind registers `group-*` before `first:` ... `even:`, so the
    // structural variants win over it whatever the order in the string
    // (`.group:hover .x` comes before `.y:last-child` in the CSS).
    let child = parse(&theme, "last:opacity-25 group-hover/card:opacity-50");
    let context = Context {
        position: position::LAST,
        ..context
    };
    child.apply(&context, &theme, &mut style, &mut props, |_| {});
    assert_eq!(style.view.opacity, 0.25);
}

#[test]
fn unknown_classes_are_reported() {
    let theme = themed();
    let list = parse(&theme, "p-4 bg-nope-500 flexx hover:wat antialiased");
    let unknown: Vec<&str> = list.unknown().iter().map(|s| &**s).collect();
    assert_eq!(unknown, ["bg-nope-500", "flexx", "hover:wat"]);
}

#[test]
fn classes_without_a_native_meaning_are_unknown() {
    let theme = themed();
    let unknown = |classes: &str| -> Vec<String> {
        parse(&theme, classes)
            .unknown()
            .iter()
            .map(|s| s.to_string())
            .collect()
    };
    let all_unknown = |classes: &str| {
        assert_eq!(
            unknown(classes),
            classes.split_whitespace().collect::<Vec<_>>(),
            "{classes}"
        );
    };
    // What Tailwind CSS generates that NSH cannot do: unknown, never a
    // different meaning.
    all_unknown(
        "transform-3d order-1 order-first -order-2 order-[3] select-all border-double outline-double",
    );
    all_unknown("min-w-fit min-h-max max-w-min max-h-fit");
    all_unknown(
        "snap-x snap-center scroll-smooth scroll-mt-4 scroll-px-2 overscroll-none touch-none isolate md:snap-x scroll-anything",
    );
    // What Tailwind CSS generates nothing for.
    all_unknown(
        "-gap-4 -gap-x-2 -space-x-1/2 -w-4 -h-2 -size-4 -min-w-0 -max-w-sm -basis-4 -flex-1 -grow -opacity-50 -text-lg -text-red-500 -bg-red-500 -rounded-lg -border-2 -blur-sm -p-4 -px-2 -leading-6 -shadow-md -ring-2 -from-red-500 -perspective-near -duration-100",
    );
    all_unknown(
        "p-1/2 p-full px-1/3 m-full m-1/2 -m-full mt-1/4 gap-1/2 gap-full gap-x-full space-y-1/2",
    );
    all_unknown("z-1.5 z-[1.5] -z-auto inset-shadow");
    // What stays: rendering hints, negatives and fractions Tailwind has.
    assert_eq!(
        unknown(
            "antialiased subpixel-antialiased box-border isolation-auto appearance-none appearance-auto will-change-transform will-change-[opacity] md:antialiased"
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        unknown(
            "-mt-2 -mx-px -m-4 -top-1/2 -inset-x-4 -start-2 -z-10 z-[100] -z-[5] -translate-x-1/2 -rotate-6 -rotate-x-12 -scale-x-100 -skew-y-3 -tracking-wide -underline-offset-2 -outline-offset-2 -hue-rotate-30 -backdrop-hue-rotate-15 -bg-linear-45 -bg-conic-90 -col-start-2 -row-end-1 -space-y-2 w-1/2 inset-full basis-1/3 translate-full"
        ),
        Vec::<String>::new()
    );
    assert_eq!(styled("-z-10").z_index, -10);
    assert_eq!(styled("z-[100]").z_index, 100);
    // A theme of the app's own may give the bare form a value, as a
    // `--inset-shadow` in Tailwind's `@theme` does.
    let theme =
        Theme::from_json(r#"{"insetShadow": {"": "inset 0 1px 2px rgb(0 0 0 / 0.1)"}}"#).unwrap();
    assert!(parse(&theme, "inset-shadow").unknown().is_empty());
}

#[test]
fn placeholder_and_props() {
    let (style, props) = styled_with(
        "placeholder:text-zinc-400 selection:bg-blue-500 caret-blue-500 line-clamp-2 pointer-events-none object-contain select-none",
        ClassEnvironment::default(),
        (390.0, 844.0),
        0,
    );
    assert_eq!(props.placeholder_color, Some(Color(0x9f9f_a9ff)));
    assert_eq!(props.caret_color, Some(Color(0x2b7f_ffff)));
    assert_eq!(props.selection_color, Some(Color(0x2b7f_ffff)));
    assert_eq!(props.number_of_lines, Some(2));
    assert_eq!(
        props.pointer_events,
        Some(crate::props::PointerEvents::None)
    );
    assert_eq!(
        style.pending.object_fit,
        Some(crate::ops::ResizeMode::Contain)
    );
    assert_eq!(props.selectable, Some(false));
    // CSS's `object-fit: none` (natural size) and `scale-down`.
    let resize = |c| {
        styled_with(c, ClassEnvironment::default(), (390.0, 844.0), 0)
            .0
            .pending
            .object_fit
    };
    assert_eq!(resize("object-none"), Some(crate::ops::ResizeMode::Center));
    assert_eq!(
        resize("object-scale-down"),
        Some(crate::ops::ResizeMode::ScaleDown)
    );
    // `::selection`'s background is the highlight; the color of the
    // selected glyphs and the other properties of the pseudo-elements
    // have no native counterpart.
    let theme = themed();
    let list = parse(
        &theme,
        "selection:text-blue-500 placeholder:italic placeholder:bg-red-500 selection:opacity-50 placeholder:text-muted-foreground selection:bg-primary",
    );
    let unknown: Vec<&str> = list.unknown().iter().map(|s| &**s).collect();
    assert_eq!(
        unknown,
        [
            "selection:text-blue-500",
            "placeholder:italic",
            "placeholder:bg-red-500",
            "selection:opacity-50"
        ]
    );
}

#[test]
fn font_tokens_are_tailwinds_sans_and_mono() {
    let mut theme = Theme::default();
    assert_eq!(
        styled_in(&theme, "font-mono").text.font_family,
        Some(FontFamily::new("monospace"))
    );
    let mut ui = crate::element::Tokens::default();
    ui.set_font(crate::element::FONT_SANS, "Inter");
    ui.set_font(crate::element::FONT_MONO, "JetBrains Mono");
    theme.set_tokens(&ui);
    assert_eq!(
        styled_in(&theme, "font-sans").text.font_family,
        Some(FontFamily::new("Inter"))
    );
    assert_eq!(
        styled_in(&theme, "font-mono").text.font_family,
        Some(FontFamily::new("JetBrains Mono"))
    );
    assert_eq!(
        styled_in(&theme, "font-(family-name:--font-mono)")
            .text
            .font_family,
        Some(FontFamily::new("JetBrains Mono"))
    );
    assert_eq!(
        styled_in(&theme, "font-[family-name:Fira_Code]")
            .text
            .font_family,
        Some(FontFamily::new("Fira Code"))
    );
    assert_ne!(parse(&theme, "font-sans").deps() & deps::THEME, 0);
}

/// `classes` applied with `theme`, in light appearance.
fn styled_in(theme: &Theme, classes: &str) -> Style {
    let group = |_: &str| 0;
    let list = parse(theme, classes);
    assert!(list.unknown().is_empty(), "{:?}", list.unknown());
    let mut style = Style::default();
    let context = Context {
        env: ClassEnvironment::default(),
        window: crate::geometry::size_pt(390.0, 844.0),
        rtl: false,
        states: 0,
        position: 0,
        group: &group,
    };
    list.apply(
        &context,
        theme,
        &mut style,
        &mut ClassProps::default(),
        |_| {},
    );
    style
}

#[test]
fn current_colors_follow_the_cascade_and_inherit() {
    for classes in [
        "text-red-500 border-2 border-blue-500 border-current ring ring-blue-500 ring-current outline-2 outline-blue-500 outline-current",
        "text-red-500 border-current border-2 ring-current ring outline-current outline-2",
    ] {
        let s = styled(classes);
        let color = s.text.color.unwrap();
        assert_eq!(s.view.borders.colors, [color; 4], "{classes}");
        assert_eq!(s.view.outline.color, color, "{classes}");
        assert!(
            s.view
                .effects
                .as_ref()
                .unwrap()
                .box_shadows
                .iter()
                .all(|shadow| shadow.color == color)
        );
    }
    assert_eq!(styled("text-red-500 text-inherit").text.color, None);
    assert_eq!(styled("text-red-500 text-current").text.color, None);
}

#[test]
fn arbitrary_current_colors_are_the_text_color() {
    let red = Color(0xff00_00ff);
    let s = styled(
        "text-[#ff0000] bg-[currentColor] border-2 border-[currentColor]/50 outline-2 outline-[color:currentColor]",
    );
    assert_eq!(s.view.background_color, red);
    assert_eq!(s.view.borders.colors[0].0 >> 8, 0xff_0000);
    assert!((0x7f..=0x80).contains(&(s.view.borders.colors[0].0 & 0xff)));
    assert_eq!(s.view.outline.color, red);
    // `currentColor` inside a value: filled in with the text color.
    let s = styled(
        "text-[#ff0000] decoration-[color-mix(in_srgb,currentColor_50%,#0000ff)] bg-[linear-gradient(currentColor,#0000ff)]",
    );
    let mix = s.text.decoration_color.unwrap().0;
    assert_eq!((mix >> 24, (mix >> 8) & 0xff), (0x80, 0x80), "{mix:x}");
    let stops = &s.view.effects.as_ref().unwrap().background_images[0].stops;
    assert_eq!(stops[0].color, red);
    let theme = themed();
    assert_ne!(
        parse(
            &theme,
            "bg-[color-mix(in_srgb,currentColor_50%,transparent)]"
        )
        .deps()
            & deps::CURRENT_COLOR,
        0
    );
    assert!(
        !parse(&theme, "bg-[color-mix(in_nope,currentColor,red)]")
            .unknown()
            .is_empty()
    );
}

#[test]
fn arbitrary_lengths_require_css_units_and_use_the_configured_rem() {
    let mut theme = themed();
    theme.rem = 20.0;
    for (classes, expected) in [("w-[2rem]", 40.0), ("w-[2pt]", 8.0 / 3.0)] {
        let list = parse(&theme, classes);
        let group = |_: &str| 0;
        let cx = Context {
            env: ClassEnvironment::default(),
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &group,
        };
        let mut s = Style::default();
        list.apply(&cx, &theme, &mut s, &mut ClassProps::default(), |_| {});
        assert_eq!(s.layout.size.width, taffy::Dimension::length(expected));
    }
    for classes in [
        "w-[100]",
        "p-[8]",
        "text-[14]",
        "rounded-[8]",
        "rotate-1.5",
        "scale-x-12.5",
    ] {
        assert_eq!(parse(&theme, classes).unknown().len(), 1, "{classes}");
    }
    assert_eq!(
        styled("inset-s-4").logical.inset[0],
        styled("start-4").logical.inset[0]
    );
    assert_eq!(
        styled("inset-e-4").logical.inset[1],
        styled("end-4").logical.inset[1]
    );
}

#[test]
fn configured_rem_reaches_composite_css_values() {
    let mut theme = themed();
    theme.rem = 20.0;
    let list = parse(&theme, "shadow-[0_1rem_2rem_3px_#000]");
    let mut style = Style::default();
    let group = |_: &str| 0;
    let context = Context {
        env: ClassEnvironment::default(),
        window: crate::geometry::size_pt(390.0, 844.0),
        rtl: false,
        states: 0,
        position: 0,
        group: &group,
    };
    list.apply(
        &context,
        &theme,
        &mut style,
        &mut ClassProps::default(),
        |_| {},
    );
    let effects = style.view.effects.unwrap();
    assert_eq!(effects.box_shadows[0].offset_y, px(20.0));
    assert_eq!(effects.box_shadows[0].blur, px(40.0));
    assert_eq!(effects.box_shadows[0].spread, px(3.0));
}

#[test]
fn object_positions() {
    use crate::style::{ObjectPosition, PositionAxis as A};
    let theme = themed();
    let at = |class: &str| styled_in(&theme, class).view.object_position;
    assert_eq!(at("object-center"), ObjectPosition::CENTER);
    assert_eq!(at("object-top"), ObjectPosition::new(A::CENTER, A::START));
    assert_eq!(at("object-bottom"), ObjectPosition::new(A::CENTER, A::END));
    assert_eq!(at("object-left"), ObjectPosition::new(A::START, A::CENTER));
    assert_eq!(at("object-right"), ObjectPosition::new(A::END, A::CENTER));
    for (a, b) in [
        ("object-top-left", "object-left-top"),
        ("object-top-right", "object-right-top"),
        ("object-bottom-left", "object-left-bottom"),
        ("object-bottom-right", "object-right-bottom"),
    ] {
        assert_eq!(at(a), at(b));
    }
    assert_eq!(
        at("object-top-left"),
        ObjectPosition::new(A::START, A::START)
    );
    assert_eq!(
        at("object-bottom-right"),
        ObjectPosition::new(A::END, A::END)
    );
    assert_eq!(
        at("object-[25%_75%]"),
        ObjectPosition::parse("25% 75%").unwrap()
    );
    assert_eq!(
        at("object-[right_10px_bottom_20%]"),
        ObjectPosition::parse("right 10px bottom 20%").unwrap()
    );
    // `object-fit` beside it.
    let (style, _) = styled_with(
        "object-cover object-top",
        ClassEnvironment::default(),
        (390.0, 844.0),
        0,
    );
    assert_eq!(
        style.pending.object_fit,
        Some(crate::ops::ResizeMode::Cover)
    );
    assert_eq!(style.view.object_position, at("object-top"));
    let list = parse(
        &theme,
        "object-[sideways] object-[1px_2px_3px] object-middle",
    );
    let unknown: Vec<&str> = list.unknown().iter().map(|s| &**s).collect();
    assert_eq!(
        unknown,
        ["object-[sideways]", "object-[1px_2px_3px]", "object-middle"]
    );
}

/// Tailwind 4.3's order, which `getClassOrder` gives: a class's highest
/// variant decides, then its next highest; one more variant comes later.
#[test]
fn variants_sort_as_tailwind_sorts_them() {
    let pad = |classes: &str, dark: bool, width: f32, states: u16| {
        let env = ClassEnvironment {
            dark,
            ..Default::default()
        };
        styled_with(classes, env, (width, 800.0), states)
            .0
            .layout
            .padding
            .left
    };
    let p = |n: f32| taffy::LengthPercentage::length(n);
    // `max-*` widest first: the narrowest wins.
    assert_eq!(pad("max-md:p-2 max-lg:p-4", false, 600.0, 0), p(8.0));
    assert_eq!(pad("max-lg:p-4 max-md:p-2", false, 600.0, 0), p(8.0));
    // `px` before `rem`: `max-sm` (40rem) after `max-[600px]`.
    assert_eq!(pad("max-sm:p-2 max-[600px]:p-4", false, 500.0, 0), p(8.0));
    // A second variant comes after the first alone.
    let hovered = state::HOVERED;
    assert_eq!(pad("dark:hover:p-2 dark:p-4", true, 390.0, hovered), p(8.0));
    assert_eq!(pad("md:hover:p-2 md:p-4", false, 800.0, hovered), p(8.0));
    // Groups by their state (`focus` after `hover`), not by position.
    let group = ClassEnvironment::default();
    let list = parse(&themed(), "group-focus:p-2 group-hover:p-4");
    let mut style = Style::default();
    let mut props = ClassProps::default();
    let g = |_: &str| state::HOVERED | state::FOCUSED;
    list.apply(
        &Context {
            env: group,
            window: crate::geometry::size_pt(390.0, 800.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &g,
        },
        &themed(),
        &mut style,
        &mut props,
        |_| {},
    );
    assert_eq!(style.layout.padding.left, p(8.0));
    // `indeterminate:` after `checked:`.
    let both = state::CHECKED | state::INDETERMINATE;
    assert_eq!(
        pad("indeterminate:p-2 checked:p-4", false, 390.0, both),
        p(8.0)
    );
}

/// Bare values as Tailwind takes them: written as JavaScript prints
/// numbers, integers where Tailwind wants them.
#[test]
fn bare_values_follow_tailwinds_grammar() {
    let theme = themed();
    for unknown in [
        "p-.5",
        "p-+4",
        "m-1e1",
        "border-1.5",
        "ring-2.5",
        "outline-0.5",
        "duration-1.5",
        "brightness-1.5",
        "grow-1.5",
        "line-clamp-1.5",
        "z-+4",
        "grid-cols-+2",
        "decoration-px",
        "outline-px",
        "outline-offset-px",
        "underline-offset-px",
        "w-prose",
        "min-w-prose",
        "size-screen",
        "w-1.5/2",
        "from-10.5%",
        "p-[-4px]",
        "w-[-10px]",
        "gap-[-2px]",
        "rounded-[-4px]",
        "text-[-4px]",
    ] {
        assert!(
            !parse(&theme, unknown).unknown().is_empty(),
            "{unknown} is no Tailwind class (or no valid CSS)"
        );
    }
    for known in [
        "p-0.5",
        "p-3.25",
        "opacity-2.5",
        "bg-red-500/2.5",
        "max-w-prose",
        "w-dvh",
        "size-dvw",
        "basis-3xs",
        "-m-[4px]",
        "z-[+4]",
        "perspective-none",
        "bg-mauve-500",
        "text-taupe-950",
        "border-olive-200",
        "ring-mist-400",
        "shadow-current",
        "inset-shadow-current",
        "text-shadow-current",
        "drop-shadow-current",
        "drop-shadow-red-500",
        "ring-offset-current",
        "decoration-current",
        "caret-current",
        "from-current",
        "via-current",
        "to-current",
        "bg-current/50",
        "shadow-xl/20",
        "drop-shadow-md/20",
        "text-shadow-md/20",
    ] {
        assert_eq!(
            parse(&theme, known).unknown(),
            &[] as &[Box<str>],
            "{known}"
        );
    }
}

#[test]
fn sticky_is_position_sticky() {
    // `sticky` is CSS's `position: sticky`; a later position wins.
    let s = styled("sticky top-0");
    assert!(s.pending.sticky_position);
    assert_eq!(s.layout.position, taffy::Position::Relative);
    assert!(!styled("sticky absolute").pending.sticky_position);
}

/// Tailwind CSS 4's `fill-*` / `stroke-*`: `fill: <color>`, `fill: none`,
/// `fill: currentcolor`, `fill: inherit`, `stroke: <color>`,
/// `stroke-width: <number>` and an arbitrary length.
#[test]
fn svg_paint_utilities_follow_tailwind_css() {
    use crate::style::SvgPaint;
    let s = styled("fill-red-500/50 stroke-blue-500 stroke-2");
    assert_eq!(s.paint.fill, Some(SvgPaint::Color(Color(0xfb2c_3680))));
    assert_eq!(s.paint.stroke, Some(SvgPaint::Color(Color(0x2b7f_ffff))));
    assert_eq!(s.paint.stroke_width, Some(px(2.0)));
    let s = styled("fill-none stroke-current stroke-[1.5px]");
    assert_eq!(s.paint.fill, Some(SvgPaint::None));
    assert_eq!(s.paint.stroke, Some(SvgPaint::CurrentColor));
    assert_eq!(s.paint.stroke_width, Some(px(1.5)));
    let s = styled("fill-current stroke-none stroke-0 stroke-[#00ff00]");
    assert_eq!(s.paint.fill, Some(SvgPaint::CurrentColor));
    assert_eq!(s.paint.stroke, Some(SvgPaint::Color(Color(0x00ff_00ff))));
    assert_eq!(s.paint.stroke_width, Some(px(0.0)));
    // `inherit` takes the parent's, which is what an unset property does.
    let s = styled("fill-red-500 fill-inherit");
    assert_eq!(s.paint.fill, None);
    // The design tokens, light and dark.
    let ui = ui_theme();
    let s = styled("stroke-primary");
    assert_eq!(
        s.paint.stroke,
        Some(SvgPaint::Color(Color::from(
            ui.color("primary").unwrap().light
        )))
    );
}

#[test]
fn env_in_arbitrary_spacing() {
    use crate::style::EnvVar::*;
    let s = styled(
        "pb-[env(safe-area-inset-bottom)] pt-[calc(env(safe-area-inset-top)+8px)] \
         mb-[env(keyboard-inset-height)] ml-[calc(4px_+_env(safe-area-inset-left))] \
         px-[env(safe-area-inset-right,_4px)]",
    );
    assert_eq!(
        s.pending.env.padding,
        [
            Some(SafeAreaInsetTop),
            Some(SafeAreaInsetRight),
            Some(SafeAreaInsetBottom),
            Some(SafeAreaInsetRight)
        ]
    );
    assert_eq!(s.layout.padding.top, taffy::LengthPercentage::length(8.0));
    assert_eq!(
        s.pending.env.margin,
        [
            None,
            None,
            Some(KeyboardInsetHeight),
            Some(SafeAreaInsetLeft)
        ]
    );
    assert_eq!(
        s.layout.margin.left,
        taffy::LengthPercentageAuto::length(4.0)
    );
    // A later utility wins, as in CSS.
    assert_eq!(
        styled("pb-[env(keyboard-inset-height)] pb-2")
            .pending
            .env
            .padding[2],
        None
    );
    // Unsupported: unknown classes.
    let list = parse(
        &themed(),
        "ps-[env(safe-area-inset-left)] -mb-[env(keyboard-inset-height)] pb-[env(nope)] \
         pb-[calc(env(safe-area-inset-top)*2)] w-[env(safe-area-inset-top)]",
    );
    assert_eq!(list.unknown().len(), 5, "{:?}", list.unknown());
}

/// Number tokens of Tailwind 4's namespaces are named classes
/// (`radius-card`: `rounded-card`), resolved by name, which Tailwind's own
/// names keep winning over.
mod token_namespaces {
    use super::*;

    fn tokens() -> crate::element::Tokens {
        let mut t = crate::element::tokens::sample();
        for (name, n) in [
            ("radius-card", 14.0),
            ("radius-lg", 99.0),
            ("spacing-gutter", 18.0),
            ("spacing-4", 99.0),
            ("text-body", 15.0),
            ("font-weight-strong", 650.0),
            ("tracking-label", 0.5),
            ("leading-body", 1.3),
            ("blur-soft", 6.0),
        ] {
            t.set_number(name, n);
        }
        // shadcn's colors have names a namespaced token may have too.
        t.set_color(
            "card",
            crate::element::AdaptiveColor::rgb(0xffffff, 0x000000),
        );
        t
    }

    /// The style `classes` give with `tokens` in the appearance `dark`.
    fn style(tokens: &crate::element::Tokens, classes: &str, dark: bool) -> Style {
        let mut theme = Theme::default();
        theme.set_tokens(tokens);
        let list = parse(&theme, classes);
        assert!(list.unknown().is_empty(), "{classes}: {:?}", list.unknown());
        let mut style = Style::default();
        let group = |_: &str| 0;
        let context = Context {
            env: ClassEnvironment {
                dark,
                ..Default::default()
            },
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &group,
        };
        list.apply(
            &context,
            &theme,
            &mut style,
            &mut ClassProps::default(),
            |_| {},
        );
        style
    }

    fn same(named: &str, arbitrary: &str) {
        same_with(&tokens(), named, arbitrary);
    }

    fn same_with(t: &crate::element::Tokens, named: &str, arbitrary: &str) {
        assert_eq!(
            format!("{:?}", style(t, named, false)),
            format!("{:?}", style(t, arbitrary, false)),
            "{named} as {arbitrary}"
        );
    }

    #[test]
    fn number_tokens_are_their_namespaces_classes() {
        same("rounded-card", "rounded-[14px]");
        same(
            "rounded-t-card rounded-bl-card",
            "rounded-t-[14px] rounded-bl-[14px]",
        );
        same(
            "p-gutter px-gutter mt-gutter gap-x-gutter",
            "p-[18px] px-[18px] mt-[18px] gap-x-[18px]",
        );
        same(
            "w-gutter h-gutter size-gutter inset-x-gutter top-gutter",
            "w-[18px] h-[18px] size-[18px] inset-x-[18px] top-[18px]",
        );
        same("-mt-gutter", "-mt-[18px]");
        same("text-body", "text-[15px]");
        same("font-strong", "font-[650]");
        same("tracking-label", "tracking-[0.5px]");
        same("leading-body", "leading-[1.3]");
        same("blur-soft", "blur-[6px]");
        assert_eq!(
            style(&tokens(), "rounded-card", false).view.border_radius,
            px(14.0)
        );
        // The color `card` stays its color utilities', and `text-card` the
        // color as it was, with a size token of that name too.
        assert_eq!(
            style(&tokens(), "bg-card", false).view.background_color,
            Color(0xffff_ffff)
        );
        let mut t = tokens();
        t.set_number("text-card", 30.0);
        assert_eq!(
            format!("{:?}", style(&t, "text-card", false)),
            format!("{:?}", style(&t, "text-[color:#ffffffff]", false))
        );
    }

    #[test]
    fn they_work_under_variants_and_follow_the_appearance_rules() {
        same(
            "hover:rounded-card dark:p-gutter",
            "hover:rounded-[14px] dark:p-[18px]",
        );
        let s = style(&tokens(), "dark:rounded-card", true);
        assert_eq!(s.view.border_radius, px(14.0));
    }

    #[test]
    fn tailwinds_own_names_keep_winning() {
        let t = tokens();
        // `radius-lg` and `spacing-4` exist, but `rounded-lg` and `p-4` stay
        // Tailwind's.
        assert_eq!(
            format!("{:?}", style(&t, "rounded-lg p-4", false)),
            format!(
                "{:?}",
                style(&crate::element::Tokens::default(), "rounded-lg p-4", false)
            )
        );
        let mut theme = Theme::default();
        theme.set_tokens(&t);
        let shadowed = theme.shadowed_tokens();
        let (_, radius) = shadowed.iter().find(|(t, _)| t == "radius-lg").unwrap();
        assert!(radius.contains(&"rounded-lg".to_owned()), "{radius:?}");
        assert!(radius.contains(&"rounded-t-lg".to_owned()), "{radius:?}");
        let (_, spacing) = shadowed.iter().find(|(t, _)| t == "spacing-4").unwrap();
        assert!(spacing.contains(&"p-4".to_owned()) && spacing.contains(&"w-4".to_owned()));
        // Tokens whose classes are all theirs are not listed.
        assert!(
            !shadowed
                .iter()
                .any(|(t, _)| t == "radius-card" || t == "spacing-gutter")
        );
    }

    /// `spacing-md` makes `p-md` the token's, while `max-w-md` stays
    /// Tailwind's container width: that one is listed.
    #[test]
    fn a_partly_shadowed_token_lists_the_classes_that_stay_tailwinds() {
        let mut t = tokens();
        t.set_number("spacing-md", 7.0);
        same_with(&t, "p-md", "p-[7px]");
        let mut theme = Theme::default();
        theme.set_tokens(&t);
        let shadowed = theme.shadowed_tokens();
        let (_, classes) = shadowed.iter().find(|(t, _)| t == "spacing-md").unwrap();
        assert!(classes.contains(&"max-w-md".to_owned()), "{classes:?}");
        assert!(!classes.contains(&"p-md".to_owned()), "{classes:?}");
    }

    /// A size `text-card` beside the color `card`: `text-card` stays the
    /// color, and the token is listed.
    #[test]
    fn a_text_size_hidden_by_a_color_is_listed() {
        let mut t = tokens();
        t.set_number("text-card", 30.0);
        let mut theme = Theme::default();
        theme.set_tokens(&t);
        let shadowed = theme.shadowed_tokens();
        assert!(
            shadowed.contains(&("text-card".to_owned(), vec!["text-card".to_owned()])),
            "{shadowed:?}"
        );
    }

    /// The longer utility wins: `inset-shadow-card` is the inset shadow's
    /// color `card`, whatever `spacing-shadow-card` is.
    #[test]
    fn the_longer_utility_wins_over_a_namespace() {
        let mut t = tokens();
        t.set_number("spacing-shadow-card", 5.0);
        same_with(&t, "inset-shadow-card", "inset-shadow-[color:#ffffffff]");
    }

    /// A weight a theme file sets is a number token, so its class works.
    #[test]
    fn a_theme_files_weight_is_a_named_class() {
        let (t, ignored) = crate::element::theme_file::parse(
            r#"{ "cssVars": { "theme": { "font-weight-strong": "650", "font-display": "Pacifico", "radius-card": "14px" } } }"#,
            &crate::element::Tokens::default(),
        )
        .unwrap();
        assert!(ignored.is_empty(), "{ignored:?}");
        assert_eq!(t.number("font-weight-strong"), Some(650.0));
        assert_eq!(t.font("font-display").map(|f| &**f), Some("Pacifico"));
        same_with(&t, "font-strong", "font-[650]");
    }

    /// Named classes depend on the theme, so the class cache parses them
    /// again when the tokens change.
    #[test]
    fn named_classes_depend_on_the_theme() {
        let mut theme = Theme::default();
        theme.set_tokens(&tokens());
        assert_ne!(
            parse(&theme, "rounded-card p-gutter text-body").deps() & deps::THEME,
            0
        );
    }

    #[test]
    fn a_name_no_token_has_stays_unknown() {
        let mut theme = Theme::default();
        theme.set_tokens(&tokens());
        let list = parse(&theme, "rounded-nothing p-card bg-gutter");
        assert_eq!(list.unknown().len(), 3, "{:?}", list.unknown());
    }

    /// A new value of the token restyles its class.
    #[test]
    fn they_follow_the_tokens_values() {
        let mut t = tokens();
        t.set_number("radius-card", 4.0);
        assert_eq!(style(&t, "rounded-card", false).view.border_radius, px(4.0));
    }
}

/// Number tokens with a value per appearance: `Tokens::number_in`, a theme
/// file's `cssVars.dark` numbers, and the classes naming them in each
/// appearance; a number with one value is as it was.
mod number_per_appearance {
    use super::*;
    use crate::element::{Tokens, theme_file};

    fn file(json: &str) -> Tokens {
        let (tokens, ignored) = theme_file::parse(json, &Tokens::default()).unwrap();
        assert!(ignored.is_empty(), "{ignored:?}");
        tokens
    }

    /// The style `classes` give with `tokens` in the appearance `dark`.
    fn style(tokens: &Tokens, classes: &str, dark: bool) -> Style {
        let mut theme = Theme::default();
        theme.set_tokens(tokens);
        let list = parse(&theme, classes);
        assert!(list.unknown().is_empty(), "{classes}: {:?}", list.unknown());
        let mut style = Style::default();
        let group = |_: &str| 0;
        let context = Context {
            env: ClassEnvironment {
                dark,
                ..Default::default()
            },
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &group,
        };
        list.apply(
            &context,
            &theme,
            &mut style,
            &mut ClassProps::default(),
            |_| {},
        );
        style
    }

    #[test]
    fn a_number_reads_its_dark_value_in_the_dark_appearance() {
        let mut t = Tokens::default();
        t.set_number("radius", 10.0);
        assert_eq!(
            (t.number_in("radius", false), t.number_in("radius", true)),
            (Some(10.0), Some(10.0))
        );
        t.set_dark_number("radius", 12.0);
        assert_eq!(t.number("radius"), Some(10.0));
        assert_eq!(
            (t.number_in("radius", false), t.number_in("radius", true)),
            (Some(10.0), Some(12.0))
        );
        // Setting the number sets it in both again.
        t.set_number("radius", 8.0);
        assert_eq!(t.number_in("radius", true), Some(8.0));
        assert_eq!(t.number_in("nothing", true), None);
    }

    #[test]
    fn over_carries_dark_values_and_a_number_without_one_takes_both() {
        let mut base = Tokens::default();
        base.set_number("radius", 10.0);
        base.set_dark_number("radius", 12.0);
        base.set_number("gap", 4.0);
        base.set_dark_number("gap", 6.0);
        let mut top = Tokens::default();
        top.set_number("radius", 20.0);
        let t = top.over(&base);
        assert_eq!(
            (t.number_in("radius", false), t.number_in("radius", true)),
            (Some(20.0), Some(20.0))
        );
        assert_eq!(t.number_in("gap", true), Some(6.0));
        let mut top = Tokens::default();
        top.set_number("gap", 5.0);
        top.set_dark_number("gap", 7.0);
        let t = top.over(&base);
        assert_eq!(
            (t.number_in("gap", false), t.number_in("gap", true)),
            (Some(5.0), Some(7.0))
        );
        // Put back from another set of tokens, the dark value comes too.
        let mut copy = Tokens::default();
        copy.copy_named(&base, "gap");
        assert_eq!(copy.number_in("gap", true), Some(6.0));
    }

    #[test]
    fn a_theme_files_dark_number_is_its_dark_value() {
        let t = file(
            r#"{ "cssVars": { "theme": { "radius": "10px", "gap": "4px" },
                "light": { "radius-card": "14px" }, "dark": { "radius": "12px", "radius-card": "6px", "only-dark": "3px" } } }"#,
        );
        assert_eq!(
            (t.number_in("radius", false), t.number_in("radius", true)),
            (Some(10.0), Some(12.0))
        );
        assert_eq!(
            (
                t.number_in("radius-card", false),
                t.number_in("radius-card", true)
            ),
            (Some(14.0), Some(6.0))
        );
        // One value: no dark value of its own, as before.
        assert_eq!(t.number_in("gap", true), Some(4.0));
        assert_eq!(t.dark_numbers().filter(|(n, _)| *n == "gap").count(), 0);
        // Given in the dark appearance only, it is both's, as a color is.
        assert_eq!(
            (
                t.number_in("only-dark", false),
                t.number_in("only-dark", true)
            ),
            (Some(3.0), Some(3.0))
        );
        // A weight too.
        let t = file(
            r#"{ "cssVars": { "light": { "font-weight-strong": "500" }, "dark": { "font-weight-strong": "700" } } }"#,
        );
        assert_eq!(
            (
                t.number_in("font-weight-strong", false),
                t.number_in("font-weight-strong", true)
            ),
            (Some(500.0), Some(700.0))
        );
    }

    #[test]
    fn classes_naming_a_number_follow_the_appearance() {
        let t = file(
            r#"{ "cssVars": { "light": { "radius-card": "14px", "gap": "4px" }, "dark": { "radius-card": "6px" } } }"#,
        );
        for classes in ["rounded-(--radius-card)", "rounded-card"] {
            assert_eq!(
                style(&t, classes, false).view.border_radius,
                px(14.0),
                "{classes}"
            );
            assert_eq!(
                style(&t, classes, true).view.border_radius,
                px(6.0),
                "{classes}"
            );
        }
        // A number with one value is the same in both.
        assert_eq!(
            format!("{:?}", style(&t, "p-(--gap)", true)),
            format!("{:?}", style(&t, "p-(--gap)", false))
        );
        let mut theme = Theme::default();
        theme.set_tokens(&t);
        assert_ne!(parse(&theme, "rounded-card").deps() & deps::THEME, 0);
    }
}

/// Shadow design tokens: `Tokens`' shadows, a theme file's `shadow-*`
/// box shadows per appearance, and the classes naming them.
mod shadow_tokens {
    use super::*;
    use crate::element::{AdaptiveShadow, Token, Tokens, theme_file};

    fn layer(y: f32, rgba: u32) -> crate::BoxShadow {
        crate::BoxShadow {
            offset_y: px(y),
            blur: px(2.0 * y),
            color: Color(rgba),
            ..Default::default()
        }
    }

    fn raised() -> AdaptiveShadow {
        AdaptiveShadow::new([layer(1.0, 0x0000_001a)], [layer(2.0, 0xffff_ff33)])
    }

    /// The box shadows `classes` give with `tokens` in the appearance
    /// `dark`.
    fn shadows(tokens: &Tokens, classes: &str, dark: bool) -> Vec<crate::BoxShadow> {
        let mut theme = Theme::default();
        theme.set_tokens(tokens);
        let list = parse(&theme, classes);
        assert!(list.unknown().is_empty(), "{classes}: {:?}", list.unknown());
        let mut style = Style::default();
        let group = |_: &str| 0;
        let context = Context {
            env: ClassEnvironment {
                dark,
                ..Default::default()
            },
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &group,
        };
        list.apply(
            &context,
            &theme,
            &mut style,
            &mut ClassProps::default(),
            |_| {},
        );
        style
            .view
            .effects
            .map(|e| e.box_shadows.clone())
            .unwrap_or_default()
    }

    #[test]
    fn tokens_hold_shadows_by_name() {
        let mut t = Tokens::default();
        t.set_shadow("shadow-raised", raised());
        assert_eq!(t.shadow("shadow-raised"), Some(&raised()));
        assert_eq!(t.get("shadow-raised"), Some(Token::Shadow(raised())));
        let mut top = Tokens::default();
        top.set_shadow("shadow-flat", [layer(0.0, 0)]);
        let t = top.over(&t);
        assert!(t.shadow("shadow-raised").is_some() && t.shadow("shadow-flat").is_some());
        assert!(!t.is_empty());
        assert_eq!(raised().css(false), "0px 1px 2px 0px #0000001a");
    }

    #[test]
    fn a_theme_files_shadows_are_read_per_appearance() {
        let (t, ignored) = theme_file::parse(
            r#"{ "cssVars": {
                "theme": { "shadow-sm": "0 1px 3px 0px hsl(0 0% 0% / 0.10), 0 1px 2px -1px hsl(0 0% 0% / 0.10)",
                           "shadow-color": "hsl(0 0% 0%)", "shadow-opacity": "0.1", "shadow-offset-x": "0" },
                "light": { "shadow-raised": "0 1px 2px #0000001a" },
                "dark": { "shadow-raised": "0 2px 4px #ffffff33" } } }"#,
            &Tokens::default(),
        )
        .unwrap();
        assert!(ignored.is_empty(), "{ignored:?}");
        assert_eq!(t.shadow("shadow-raised"), Some(&raised()));
        assert_eq!(t.shadow("shadow-sm").map(|s| s.light.len()), Some(2));
        // shadcn's other `shadow-*` names stay what they were.
        assert!(t.color("shadow-color").is_some());
        assert_eq!(t.number("shadow-opacity"), Some(0.1));
        assert_eq!(t.number("shadow-offset-x"), Some(0.0));
        assert!(t.shadow("shadow-color").is_none());
    }

    #[test]
    fn a_shadow_token_is_its_class_and_follows_the_appearance() {
        let mut t = crate::element::tokens::sample();
        t.set_shadow("shadow-raised", raised());
        for classes in ["shadow-raised", "shadow-(--shadow-raised)"] {
            assert_eq!(
                shadows(&t, classes, false),
                [layer(1.0, 0x0000_001a)],
                "{classes}"
            );
            assert_eq!(
                shadows(&t, classes, true),
                [layer(2.0, 0xffff_ff33)],
                "{classes}"
            );
        }
        let mut theme = Theme::default();
        theme.set_tokens(&t);
        assert!(parse(&theme, "hover:shadow-raised").unknown().is_empty());
        assert_ne!(parse(&theme, "shadow-raised").deps() & deps::THEME, 0);
        // Tailwind's own size keeps its meaning.
        t.set_shadow("shadow-sm", [layer(9.0, 0xff00_00ff)]);
        assert_eq!(
            shadows(&t, "shadow-sm", false),
            shadows(&Tokens::default(), "shadow-sm", false)
        );
        // A theme's `shadow-sm`, which Tailwind's keeps, is read by name.
        let mut sized = Tokens::default();
        sized.set_shadow("shadow-sm", [layer(9.0, 0xff00_00ff)]);
        assert_eq!(
            shadows(&sized, "shadow-(--shadow-sm)", false),
            [layer(9.0, 0xff00_00ff)]
        );
        // A name no token has stays unknown, and a shadow token named
        // without the namespace is no class.
        assert!(!parse(&theme, "shadow-nothing").unknown().is_empty());
        let mut bare = Tokens::default();
        bare.set_shadow("card", raised());
        let mut theme = Theme::default();
        theme.set_tokens(&bare);
        assert!(!parse(&theme, "card").unknown().is_empty());
        // Several layers, negative spread and inset round-trip through the
        // class.
        let mut t = Tokens::default();
        let layers = [
            crate::BoxShadow {
                offset_x: px(-1.0),
                offset_y: px(2.0),
                blur: px(3.0),
                spread: px(-1.0),
                color: Color(0x1122_3344),
                inset: false,
            },
            crate::BoxShadow {
                inset: true,
                ..layer(1.0, 0xffff_ff80)
            },
        ];
        t.set_shadow("shadow-deep", layers);
        assert_eq!(shadows(&t, "shadow-deep", false), layers);
    }
}

/// A theme whose numbers are in no namespace of Tailwind's (shadcn's, with
/// `radius` alone) parses its token classes without looking for named
/// numbers; once a namespaced number is among its tokens, its classes are.
mod namespaced_numbers_lookup {
    use super::*;
    use crate::element::{AdaptiveColor, Tokens};

    /// shadcn's kinds of tokens: colors and `radius`, no namespaced number.
    fn plain() -> Tokens {
        let mut t = Tokens::default();
        t.set_color("primary", AdaptiveColor::rgb(0x171717, 0xfafafa));
        t.set_color("card", AdaptiveColor::rgb(0xffffff, 0x0a0a0a));
        t.set_number("radius", 10.0);
        t
    }

    fn unknown(tokens: &Tokens, classes: &str) -> usize {
        let mut theme = Theme::default();
        theme.set_tokens(tokens);
        parse(&theme, classes).unknown().len()
    }

    #[test]
    fn a_theme_without_namespaced_numbers_has_no_named_classes() {
        let mut t = plain();
        t.set_number("card-padding", 18.0);
        assert_eq!(unknown(&t, "rounded-card p-padding"), 2);
        // Its colors stay their utilities'.
        assert_eq!(unknown(&t, "bg-primary text-card border-card/50"), 0);
    }

    #[test]
    fn a_namespaced_number_makes_its_classes() {
        let mut t = plain();
        assert_eq!(unknown(&t, "rounded-card p-gutter"), 2);
        t.set_number("radius-card", 14.0);
        assert_eq!(unknown(&t, "rounded-card p-gutter"), 1);
        t.set_number("spacing-gutter", 18.0);
        assert_eq!(unknown(&t, "rounded-card p-gutter"), 0);
    }

    /// A number with a dark value alone is a class where only the dark
    /// one is read (`dark:`).
    #[test]
    fn a_dark_only_namespaced_number_makes_its_dark_classes() {
        let mut t = plain();
        assert_eq!(unknown(&t, "dark:rounded-card"), 1);
        t.set_dark_number("radius-card", 16.0);
        assert_eq!(unknown(&t, "dark:rounded-card"), 0);
    }

    #[test]
    fn a_name_that_only_starts_like_a_namespace_is_none() {
        let mut t = plain();
        // `radiuscard` and `textbody` are no `radius-*` and `text-*`.
        t.set_number("radiuscard", 14.0);
        t.set_number("textbody", 15.0);
        assert_eq!(unknown(&t, "rounded-card text-body"), 2);
    }
}

/// A class list keeps the tokens its rules read, and a change of the
/// tokens' values resolves again only the cached classes that read one of
/// them ([`Tailwind::set_tokens`]).
mod tokens_by_name {
    use std::rc::Rc;

    use super::*;
    use crate::element::{AdaptiveColor, AdaptiveShadow, Tokens};

    fn tokens() -> Tokens {
        let mut t = Tokens::default();
        t.set_color("brand", AdaptiveColor::rgb(0x2563eb, 0x93c5fd));
        t.set_color("accent", AdaptiveColor::rgb(0xf5f5f5, 0x262626));
        t.set_color("card", AdaptiveColor::rgb(0xffffff, 0x171717));
        t.set_number("radius", 10.0);
        t.set_number("radius-card", 14.0);
        t.set_font("font-sans", "Inter");
        let layer = |y: f32| crate::BoxShadow {
            offset_y: px(y),
            blur: px(2.0 * y),
            color: Color(0x0000_001a),
            ..Default::default()
        };
        t.set_shadow(
            "shadow-raised",
            AdaptiveShadow::new([layer(1.0)], [layer(2.0)]),
        );
        t
    }

    fn list(classes: &str) -> ClassList {
        let mut theme = Theme::default();
        theme.set_tokens(&tokens());
        let list = parse(&theme, classes);
        assert!(list.unknown().is_empty(), "{classes}: {:?}", list.unknown());
        list
    }

    fn tailwind() -> Tailwind {
        let mut tailwind = Tailwind::new();
        tailwind.set_tokens(&tokens());
        tailwind
    }

    #[test]
    fn a_list_reads_the_tokens_its_classes_name() {
        let l = list("p-4 bg-brand border-accent/50 hover:text-brand");
        assert!(l.reads_token("brand") && l.reads_token("accent"));
        assert!(!l.reads_token("card"));
        let dark = list("dark:bg-card");
        assert!(dark.reads_token("card") && !dark.reads_token("brand"));
        let plain = list("p-4 bg-white rounded-lg");
        for name in ["brand", "accent", "card", "radius", "radius-card"] {
            assert!(!plain.reads_token(name), "{name}");
        }
    }

    #[test]
    fn a_variable_a_namespaced_number_a_font_and_a_shadow_are_read_by_name() {
        assert!(list("rounded-(--radius)").reads_token("radius"));
        assert!(list("bg-(--brand)").reads_token("brand"));
        // The number of the utility's namespace, not the color of its name.
        let card = list("rounded-card");
        assert!(card.reads_token("radius-card") && !card.reads_token("card"));
        assert!(list("font-sans").reads_token("font-sans"));
        assert!(list("shadow-raised").reads_token("shadow-raised"));
    }

    #[test]
    fn a_merged_list_reads_what_both_read() {
        let merged = list("bg-brand").merged(&list("text-accent"));
        assert!(merged.reads_token("brand") && merged.reads_token("accent"));
        assert!(!merged.reads_token("card"));
    }

    #[test]
    fn a_change_of_a_value_resolves_only_the_classes_reading_it() {
        let mut tailwind = tailwind();
        let brand = tailwind.classes("p-2 bg-brand");
        let accent = tailwind.classes("p-2 bg-accent");
        let plain = tailwind.classes("p-2");
        let mut t = tokens();
        t.set_color("brand", AdaptiveColor::rgb(0x16a34a, 0x4ade80));
        let changed = tailwind.set_tokens(&t).expect("a change");
        assert_ne!(changed, TokenChange::All);
        assert!(brand.reads(&changed));
        assert!(!accent.reads(&changed) && !plain.reads(&changed));
        // Parsed again with the new value; the others are the lists they were.
        let again = tailwind.classes("p-2 bg-brand");
        assert!(!Rc::ptr_eq(&brand, &again));
        assert_ne!(*brand, *again);
        assert!(Rc::ptr_eq(&accent, &tailwind.classes("p-2 bg-accent")));
        assert!(Rc::ptr_eq(&plain, &tailwind.classes("p-2")));
    }

    #[test]
    fn each_kind_of_token_is_a_change_of_its_name() {
        let change = |edit: fn(&mut Tokens)| {
            let mut tailwind = tailwind();
            let mut t = tokens();
            edit(&mut t);
            tailwind.set_tokens(&t).expect("a change")
        };
        let number = change(|t| t.set_number("radius-card", 20.0));
        assert!(list("rounded-card").reads(&number));
        assert!(!list("rounded-(--radius) bg-card").reads(&number));
        // A dark value alone, which the tokens before did not have.
        let dark = change(|t| t.set_dark_number("radius", 4.0));
        assert_ne!(dark, TokenChange::All);
        assert!(list("rounded-(--radius)").reads(&dark));
        assert!(!list("rounded-card").reads(&dark));
        let font = change(|t| t.set_font("font-sans", "Geist"));
        assert!(list("font-sans").reads(&font) && !list("bg-brand").reads(&font));
        let shadow = change(|t| {
            let layer = crate::BoxShadow {
                offset_y: px(8.0),
                ..Default::default()
            };
            t.set_shadow("shadow-raised", AdaptiveShadow::new([layer], [layer]));
        });
        assert!(list("shadow-raised").reads(&shadow) && !list("bg-brand").reads(&shadow));
    }

    #[test]
    fn new_names_resolve_every_class_and_the_same_tokens_none() {
        let mut tailwind = tailwind();
        let accent = tailwind.classes("p-2 bg-accent");
        assert_eq!(tailwind.set_tokens(&tokens()), None);
        assert!(Rc::ptr_eq(&accent, &tailwind.classes("p-2 bg-accent")));
        // `bg-extra` was no class: a new name may make any unknown one.
        let unknown = tailwind.classes("bg-extra");
        assert_eq!(unknown.unknown().len(), 1);
        let mut t = tokens();
        t.set_color("extra", AdaptiveColor::rgb(0xff0000, 0xff0000));
        let changed = tailwind.set_tokens(&t).expect("a change");
        assert_eq!(changed, TokenChange::All);
        assert!(accent.reads(&changed) && unknown.reads(&changed));
        assert!(tailwind.classes("bg-extra").unknown().is_empty());
        assert!(!Rc::ptr_eq(&accent, &tailwind.classes("p-2 bg-accent")));
    }
}

/// A variable in a class names a theme role: `bg-(--popover)` is the token
/// the theme maps `popover` onto, where no token has that name.
mod role_variables {
    use std::rc::Rc;

    use super::*;
    use crate::element::{AdaptiveColor, FontRole, NumberRole, Role, Tokens};

    const POPOVER: Role = Role::new("popover");
    const WARNING: Role = Role::new("warning");
    const RADIUS: NumberRole = NumberRole::new("radius");
    const FONT: FontRole = FontRole::new("font-mono");

    /// A design system with names of its own, and the roles it plays.
    fn brand() -> Tokens {
        let mut t = Tokens::default();
        t.set_color("overlay", AdaptiveColor::rgb(0xfafafa, 0x262626));
        t.set_color("caution", AdaptiveColor::rgb(0xb45309, 0xfbbf24));
        t.set_color("ink", AdaptiveColor::rgb(0x0a0a0a, 0xfafafa));
        t.set_number("corner", 6.0);
        t.set_dark_number("corner", 8.0);
        t.set_font("code", "Berkeley Mono");
        t.with_roles([(POPOVER, "overlay"), (WARNING, "caution")])
            .with_roles([(RADIUS, "corner")])
            .with_roles([(FONT, "code")])
    }

    fn theme(tokens: &Tokens) -> Theme {
        let mut theme = Theme::default();
        theme.set_tokens(tokens);
        theme
    }

    /// `classes` with `tokens`, all of them known.
    fn list(tokens: &Tokens, classes: &str) -> ClassList {
        let list = parse(&theme(tokens), classes);
        assert!(list.unknown().is_empty(), "{classes}: {:?}", list.unknown());
        list
    }

    /// The style `classes` give with `tokens` in the appearance `dark`, as
    /// its debug text.
    fn style(tokens: &Tokens, classes: &str, dark: bool) -> String {
        let theme = theme(tokens);
        let list = parse(&theme, classes);
        assert!(list.unknown().is_empty(), "{classes}: {:?}", list.unknown());
        let mut style = Style::default();
        let group = |_: &str| 0;
        let context = Context {
            env: ClassEnvironment {
                dark,
                ..Default::default()
            },
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &group,
        };
        list.apply(
            &context,
            &theme,
            &mut style,
            &mut ClassProps::default(),
            |_| {},
        );
        format!("{style:?}")
    }

    #[test]
    fn a_variable_reads_the_token_its_role_is_mapped_onto() {
        let t = brand();
        for dark in [false, true] {
            for (role, token) in [
                ("bg-(--popover)", "bg-overlay"),
                ("border-(--warning)", "border-caution"),
                ("text-(color:--warning)", "text-caution"),
                ("bg-(--popover)/50", "bg-overlay/50"),
                ("rounded-(--radius)", "rounded-(--corner)"),
                (
                    "font-(family-name:--font-mono)",
                    "font-(family-name:--code)",
                ),
                ("hover:bg-(--popover)", "hover:bg-overlay"),
                ("dark:bg-(--popover)", "dark:bg-overlay"),
            ] {
                assert_eq!(
                    style(&t, role, dark),
                    style(&t, token, dark),
                    "{role} dark: {dark}"
                );
            }
        }
        // Each appearance has the token's value in it.
        assert_ne!(
            style(&t, "bg-(--popover)", false),
            style(&t, "bg-(--popover)", true)
        );
        assert_ne!(
            style(&t, "rounded-(--radius)", false),
            style(&t, "rounded-(--radius)", true)
        );
    }

    #[test]
    fn a_token_of_the_name_is_read_before_the_role() {
        // The theme has a token `popover` and maps the role elsewhere: the
        // variable is the token's name, as it is without roles.
        let mut t = brand();
        t.set_color("popover", AdaptiveColor::rgb(0xff0000, 0x00ff00));
        assert_eq!(
            style(&t, "bg-(--popover)", false),
            style(&t, "bg-popover", false)
        );
        assert_ne!(
            style(&t, "bg-(--popover)", false),
            style(&t, "bg-overlay", false)
        );
        assert!(list(&t, "bg-(--popover)").reads_token("popover"));
        assert!(!list(&t, "bg-(--popover)").reads_token("overlay"));
    }

    #[test]
    fn a_gradient_token_of_the_name_is_read_before_the_role() {
        // A gradient is a token too: its name is the variable's, whatever
        // a role of that name is mapped onto.
        let hero = crate::element::AdaptiveGradient::parse("linear-gradient(#000, #fff)").unwrap();
        let mut t = brand();
        t.set_gradient("background-image-hero", hero);
        let t = t.with_roles([(
            Role::<AdaptiveColor>::new("background-image-hero"),
            "overlay",
        )]);
        let l = list(&t, "bg-(image:--background-image-hero)");
        assert!(l.reads_token("background-image-hero") && !l.reads_token("overlay"));
        assert_eq!(
            style(&t, "bg-(image:--background-image-hero)", false),
            style(&t, "bg-hero", false)
        );
    }

    #[test]
    fn a_bare_class_names_no_role() {
        let theme = theme(&brand());
        for class in ["bg-popover", "text-warning", "rounded-radius"] {
            assert_eq!(parse(&theme, class).unknown().len(), 1, "{class}");
        }
    }

    #[test]
    fn a_role_nothing_maps_and_one_mapped_onto_no_token_are_unknown() {
        let t = brand().with_roles([(Role::<AdaptiveColor>::new("accent"), "missing")]);
        let theme = theme(&t);
        for class in ["bg-(--success)", "bg-(--accent)", "rounded-(--success)"] {
            assert_eq!(parse(&theme, class).unknown().len(), 1, "{class}");
        }
        // Without roles a variable is what it was.
        let plain = self::theme(&Tokens::default());
        assert_eq!(parse(&plain, "bg-(--popover)").unknown().len(), 1);
    }

    #[test]
    fn a_class_reading_a_role_follows_its_token() {
        let t = brand();
        let l = list(&t, "p-2 bg-(--popover)");
        assert!(l.reads_token("overlay") && !l.reads_token("popover"));
        let mut tailwind = Tailwind::new();
        tailwind.set_tokens(&t);
        let role = tailwind.classes("bg-(--popover)");
        let other = tailwind.classes("bg-(--warning)");
        let mut next = brand();
        next.set_color("overlay", AdaptiveColor::rgb(0xeeeeee, 0x333333));
        let changed = tailwind.set_tokens(&next).expect("a change");
        assert!(matches!(changed, TokenChange::Values(_)));
        assert!(role.reads(&changed) && !other.reads(&changed));
        assert!(Rc::ptr_eq(&other, &tailwind.classes("bg-(--warning)")));
        assert!(!Rc::ptr_eq(&role, &tailwind.classes("bg-(--popover)")));
    }

    #[test]
    fn a_change_of_the_roles_mapped_resolves_every_class() {
        let mut tailwind = Tailwind::new();
        tailwind.set_tokens(&brand());
        let role = tailwind.classes("bg-(--popover)");
        let plain = tailwind.classes("p-2 bg-ink");
        assert_eq!(tailwind.set_tokens(&brand()), None);
        // The same tokens, `popover` played by another of them.
        let next = brand().with_roles([(POPOVER, "ink")]);
        let changed = tailwind.set_tokens(&next).expect("a change");
        assert_eq!(changed, TokenChange::All);
        assert!(role.reads(&changed) && plain.reads(&changed));
        let now = tailwind.classes("bg-(--popover)");
        assert!(now.reads_token("ink") && !now.reads_token("overlay"));
        // A scope with other roles differs in every class, one with the
        // same roles in what its values change.
        assert_eq!(
            tailwind.difference(&ClassTokens::new(&brand())),
            TokenChange::All
        );
        let mut scope = next.clone();
        scope.set_color("ink", AdaptiveColor::rgb(0x111111, 0xeeeeee));
        let difference = tailwind.difference(&ClassTokens::new(&scope));
        assert!(matches!(difference, TokenChange::Values(_)));
        assert!(now.reads(&difference));
    }
}

/// Gradient design tokens (`background-image-hero`): held by `Tokens` per
/// appearance, read from a theme file, the class `bg-hero` and the variable
/// `bg-(image:--background-image-hero)`.
mod gradient_tokens {
    use super::*;
    use crate::{
        effects::{Gradient, GradientKind},
        element::{AdaptiveColor, AdaptiveGradient, Token, Tokens, theme_file},
    };

    fn css(css: &str) -> Vec<Gradient> {
        crate::css::gradients(css).unwrap()
    }

    fn hero() -> AdaptiveGradient {
        AdaptiveGradient::new(
            css("linear-gradient(135deg, #7c3aed, #db2777)"),
            css("linear-gradient(135deg, #a78bfa, #f472b6)"),
        )
    }

    fn themed() -> Tokens {
        let mut t = Tokens::default();
        t.set_gradient("background-image-hero", hero());
        t.set_color("primary", AdaptiveColor::rgb(0x171717, 0xfafafa));
        t
    }

    /// The gradients `classes` give with `tokens` in the appearance `dark`,
    /// and the classes that are unknown.
    fn drawn(tokens: &Tokens, classes: &str, dark: bool) -> (Vec<Gradient>, Vec<String>) {
        let mut theme = Theme::default();
        theme.set_tokens(tokens);
        let list = parse(&theme, classes);
        let unknown = list.unknown().iter().map(|c| c.to_string()).collect();
        let mut style = Style::default();
        let group = |_: &str| 0;
        let context = Context {
            env: ClassEnvironment {
                dark,
                ..Default::default()
            },
            window: crate::geometry::size_pt(390.0, 844.0),
            rtl: false,
            states: 0,
            position: 0,
            group: &group,
        };
        list.apply(
            &context,
            &theme,
            &mut style,
            &mut ClassProps::default(),
            |_| {},
        );
        let images = style
            .view
            .effects
            .map(|e| e.background_images.clone())
            .unwrap_or_default();
        (images, unknown)
    }

    #[test]
    fn tokens_hold_gradients_by_name() {
        let t = themed();
        assert_eq!(t.gradient("background-image-hero"), Some(&hero()));
        assert_eq!(
            t.get("background-image-hero"),
            Some(Token::Gradient(hero()))
        );
        let mut top = Tokens::default();
        top.set_gradient(
            "background-image-scrim",
            css("linear-gradient(#000, #0000)"),
        );
        let t = top.over(&t);
        assert!(t.gradient("background-image-hero").is_some());
        assert!(t.gradient("background-image-scrim").is_some());
        assert!(!t.is_empty());
        assert_eq!(
            hero().css(false),
            "linear-gradient(135deg, #7c3aedff, #db2777ff)"
        );
        assert_eq!(AdaptiveGradient::default().css(true), "none");
    }

    #[test]
    fn a_gradient_is_written_as_css_that_reads_back() {
        for written in [
            "linear-gradient(135deg, #7c3aed, #db2777)",
            "linear-gradient(to right, #ff0000 10%, #00ff0080 50%, #0000ff 20px)",
            "linear-gradient(to top left, red, blue)",
            "repeating-linear-gradient(45deg, #000 0%, #fff 10%)",
            "radial-gradient(circle closest-side at 25% 75%, #fff, #000)",
            "radial-gradient(#fff, #000)",
            "conic-gradient(from 90deg at 30% 40%, red, blue 50%, red)",
            "linear-gradient(in oklch, #7c3aed, #db2777)",
        ] {
            let read = css(written);
            let again = AdaptiveGradient::from(read.clone()).css(false);
            assert_eq!(css(&again), read, "{written} written as {again}");
        }
        // Several, comma-separated.
        let two = css("linear-gradient(#000, #fff), radial-gradient(#f00, #00f)");
        let again = AdaptiveGradient::from(two.clone()).css(false);
        assert_eq!(css(&again), two, "{again}");
    }

    #[test]
    fn a_theme_files_gradients_are_read_per_appearance() {
        let mut base = Tokens::default();
        base.set_color("accent", AdaptiveColor::rgb(0x0000ff, 0x00ffff));
        let (t, ignored) = theme_file::parse(
            r##"{ "cssVars": {
                "theme": { "primary": "#171717",
                           "background-image-scrim": "linear-gradient(to top, #000000cc, #00000000)",
                           "background-image-brand": "linear-gradient(to right, var(--primary), var(--accent))",
                           "background-image-photo": "url(/hero.jpg)",
                           "background-image-none": "none" },
                "light": { "background-image-hero": "linear-gradient(135deg, #7c3aed, #db2777)" },
                "dark": { "primary": "#fafafa",
                          "background-image-hero": "linear-gradient(135deg, #a78bfa, #f472b6)" } } }"##,
            &base,
        )
        .unwrap();
        assert_eq!(ignored, Vec::<String>::new());
        assert_eq!(t.gradient("background-image-hero"), Some(&hero()));
        // One value for both appearances.
        let scrim = t.gradient("background-image-scrim").unwrap();
        assert_eq!(scrim.light, scrim.dark);
        // A `var()` in a stop is the theme's color in each appearance: the
        // file's `primary`, the base's `accent`.
        let brand = t.gradient("background-image-brand").unwrap();
        let colors = |g: &[Gradient]| g[0].stops.iter().map(|s| s.color.0).collect::<Vec<_>>();
        assert_eq!(colors(&brand.light), [0x1717_17ff, 0x0000_ffff]);
        assert_eq!(colors(&brand.dark), [0xfafa_faff, 0x00ff_ffff]);
        // What is no gradient is read as before: no token.
        assert_eq!(t.get("background-image-photo"), None);
        assert_eq!(t.get("background-image-none"), None);
        // Written back, it reads as the same tokens.
        let (again, _) = theme_file::parse(&theme_file::write(&t), &Tokens::default()).unwrap();
        assert_eq!(
            again.gradient("background-image-hero"),
            t.gradient("background-image-hero")
        );
    }

    #[test]
    fn a_gradient_token_is_the_class_of_its_name() {
        let t = themed();
        let (light, unknown) = drawn(&t, "bg-hero", false);
        assert!(unknown.is_empty(), "{unknown:?}");
        assert_eq!(light, hero().light.to_vec());
        let (dark, _) = drawn(&t, "bg-hero", true);
        assert_eq!(dark, hero().dark.to_vec());
        // By its variable, as an image.
        let (by_variable, unknown) = drawn(&t, "bg-(image:--background-image-hero)", true);
        assert!(unknown.is_empty(), "{unknown:?}");
        assert_eq!(by_variable, hero().dark.to_vec());
        // Under a variant.
        let (_, unknown) = drawn(&t, "hover:bg-hero dark:bg-hero", false);
        assert!(unknown.is_empty(), "{unknown:?}");
        // No opacity modifier, no bare variable (that is a color's), and no
        // other utility's value.
        for class in [
            "bg-hero/50",
            "bg-(--background-image-hero)",
            "text-hero",
            "bg-nope",
        ] {
            let (_, unknown) = drawn(&t, class, false);
            assert_eq!(unknown, [class], "{class}");
        }
    }

    #[test]
    fn a_color_and_tailwinds_own_classes_come_first() {
        let mut t = themed();
        // A color `hero` is what `bg-hero` means; the gradient is hidden.
        t.set_color("hero", AdaptiveColor::rgb(0xff0000, 0xff0000));
        // So is one named as a palette color or a utility of Tailwind's.
        t.set_gradient("background-image-black", hero());
        t.set_gradient("background-image-none", hero());
        let (images, unknown) = drawn(&t, "bg-hero bg-black bg-none", false);
        assert!(unknown.is_empty(), "{unknown:?}");
        assert_eq!(images, Vec::new());
        // The build says which tokens are hidden, and by which class.
        let mut theme = Theme::default();
        theme.set_tokens(&t);
        let hidden = theme.tokens().shadowed(&theme);
        assert_eq!(
            hidden,
            [
                (
                    "background-image-black".to_owned(),
                    vec!["bg-black".to_owned()]
                ),
                (
                    "background-image-hero".to_owned(),
                    vec!["bg-hero".to_owned()]
                ),
                (
                    "background-image-none".to_owned(),
                    vec!["bg-none".to_owned()]
                ),
            ]
        );
        // Both are still read by their variable.
        let (images, _) = drawn(&t, "bg-(image:--background-image-black)", false);
        assert_eq!(images, hero().light.to_vec());
    }

    #[test]
    fn a_class_reads_its_gradient_token_and_follows_only_it() {
        let mut theme = Theme::default();
        theme.set_tokens(&themed());
        let list = parse(&theme, "bg-hero p-2");
        assert!(list.reads_token("background-image-hero"));
        assert!(!list.reads_token("primary"));
        assert_ne!(list.deps() & deps::THEME, 0);
        // Another gradient is another value of the token.
        let mut other = themed();
        other.set_gradient("background-image-hero", css("linear-gradient(#000, #fff)"));
        let changed = ClassTokens::new(&themed()).changed(&ClassTokens::new(&other));
        assert_eq!(changed.len(), 1);
        assert!(list.reads(&TokenChange::Values(changed)));
        let kind = |g: &[Gradient]| g[0].kind;
        assert!(matches!(kind(&hero().light), GradientKind::Linear { .. }));
    }
}
