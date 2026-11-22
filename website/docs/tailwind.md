---
title: Tailwind
description: Tailwind CSS classes resolved natively in TSX and their typed equivalents in Rust - class and classList, variants for pressed states, dark mode and breakpoints, the theme in nsh.json, and the build-time class check.
order: 33
group: ui
demo: guide-tailwind
---

# Tailwind

NSH's styles follow Tailwind CSS 4. In Rust, Tailwind's utilities are typed style methods with the same names and values (`.p_4()` is `p-4`, `.rounded_2xl()` is `rounded-2xl`, `ZINC.shade(900)` is `zinc-900`); in TSX, every element accepts the classes themselves in `class`, resolved natively against Tailwind 4's default theme. Each example below shows the typed methods first and the classes second:

```rust tabs
use nsh_core::element::palette::ZINC;

view()
    .flex_row()
    .items_center()
    .gap_3()
    .rounded_2xl()
    .bg(AdaptiveColor::new(white(), ZINC.shade(800)))
    .p_4()
    .shadow_lg()
    .active(|s| s.scale(0.98))
    .child(
        text("Tailwind on native views")
            .text_base()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(AdaptiveColor::new(ZINC.shade(900), white())),
    )
```

```tsx tabs
<View class="flex-row items-center gap-3 rounded-2xl bg-white p-4 shadow-lg active:scale-[0.98] dark:bg-zinc-800">
  <Text class="text-base font-semibold text-zinc-900 dark:text-white">Tailwind on native views</Text>
</View>
```

Typed methods lower into the same style rules the classes compile to, with no string to parse. For classes, no CSS is generated and nothing runs in JavaScript: the string is passed to the core, which parses each distinct class string once (and caches it), turns it into style rules, and applies them. Variants that depend on state (pressed, dark mode, window width) are re-applied by the core when that state changes, and pressed and focused states are tracked natively, so `active:` feedback (`.active(..)` in Rust) has no round trip to your code.

Classes and the [`style` prop](./styling.md) produce the same native styles and can be combined; this page covers what is specific to classes and their typed equivalents. The [Tailwind reference](../modules/tailwind-reference.md) lists every supported utility.

## class and classList

```rust tabs
view()
    .rounded_lg()
    .px_4()
    .py_2()
    .transition_colors()
    .bg(move || if selected.get() { INDIGO.shade(600) } else { ZINC.shade(200) })
    .style_when(busy, |s| s.opacity(0.5))
```

```tsx tabs
<View
  class="rounded-lg px-4 py-2 transition-colors"
  classList={{ "bg-indigo-600": selected(), "bg-zinc-200": !selected(), "opacity-50": busy() }}
/>
```

- **`class`** is a string of classes. It can be dynamic: ``class={`h-6 rounded bg-${color()}-500`}`` works, because classes are resolved at run time.
- **`classList`** maps classes to conditions: a class applies while its value is truthy. It is applied after `class`.
- **In Rust**, a method given a closure or a signal follows it (`.bg(move || ..)`), `.style_when(condition, |s| ..)` applies styles while a condition holds, and `.when(condition, |v| ..)` decides once, when the element is made.

How conflicts resolve:

1. **Later classes win** over earlier ones in the same string: `"p-2 p-4"` is `p-4`. There is no need for `tailwind-merge`: a component can append the `class` its caller passes. Later methods win in Rust too.
2. **Variant classes apply after plain ones**, ordered by Tailwind's variant order (`hover:` before `active:`, `max-*:` widest first, breakpoints ascending, `dark:` last), whatever their position in the string. As in Tailwind, a class's highest variant decides, then its next highest, and a class with one more variant comes later: `dark:hover:bg-a dark:bg-b` is `bg-a` while hovered in dark mode, and `max-md:p-2 max-lg:p-4` is `p-2` below `md`. As in CSS, `odd:bg-a last:bg-b` gives an odd last row `bg-a`, because Tailwind sorts `odd` after `last`.
3. **`style` wins over classes.** A style key is the CSS property of its name, the one the classes set, so it replaces only that property: `style={{ paddingLeft: 8 }}` replaces `pl-4` and keeps `pr-4`.
4. `!` makes a class important: `!p-0`.

Rust components (`nsh/base`'s and those of an app's own crate) accept `class` and `classList` from TSX too; your classes are merged after the component's own styles and win at equal rank.

## Variants

| Variant | Applies when | Rust |
|---|---|---|
| `active:` | The view is pressed | `.active(\|s\| ..)` |
| `hover:` | A pointer is over the view (iPad and Mac pointers) | `.hover(\|s\| ..)` |
| `focus:` | The element has focus | `.focus(\|s\| ..)` |
| `focus-visible:` | The element has focus and shows it: the keyboard moved it there | `.focus_visible(\|s\| ..)` |
| `focus-within:` | The view or a descendant has focus (the content of an `Overlay` is a descendant of the view it is in) | `.focus_within(..)` |
| `disabled:`, `enabled:` | The element's `disabled` prop is true / false | `.disabled_style(\|s\| ..)` |
| `selected:` | The element is selected (a `Button`'s `selected`, `accessibilityState.selected`) | `.selected_style(..)` |
| `checked:` | The element is checked (`accessibilityState.checked` is `true`; `"mixed"` is not checked) | `.checked_style(..)` |
| `indeterminate:` | The element is indeterminate (`accessibilityState.checked` is `"mixed"`) | `.indeterminate_style(..)` |
| `aria-expanded:` | The element is expanded (`accessibilityState.expanded` is `true`): a `Popover`'s or `Menu`'s trigger while its popup is open | `.expanded_style(..)` |
| `aria-pressed:` | The element is a toggle button that is on (`accessibilityState.pressed` is `true`; `"mixed"` is not pressed). Not `active:`, a press in progress | `.aria_pressed(..)`, `.pressed_style(..)` |
| `aria-busy:`, `aria-checked:`, `aria-invalid:`, `aria-selected:` | `accessibilityState.busy`, `checked` (`true`, not `"mixed"`), `invalid` or `selected` is `true`, as Tailwind's `aria-*` variants: after the pseudo-classes, in the order of their names | |
| `data-<name>:` | The element has the `data-*` attribute `name`, as Tailwind's boolean `data-*` variants (`&[data-range-start]`): what a component's part shows beyond its states, a calendar's day at the start of a range (`data-range-start:`). After the `aria-*` variants, in the order of their names | `Interactive::project_data(name, ..)` sets one on a component's part |
| `group-active:`, `group-hover:`, `group-focus:`, `group-focus-within:`, `group-disabled:`, `group-selected:`, `group-checked:`, `group-indeterminate:`, `group-aria-expanded:`, `group-aria-pressed:`, `group-aria-busy:`, `group-aria-checked:`, `group-aria-invalid:`, `group-aria-selected:`, `group-data-<name>:` | The nearest ancestor with the `group` class is in that state, or has that `data-*` attribute | `.group()` on the ancestor; `.group_active(..)`, `.group_hover(..)`, `.group_focus_within(..)` inside |
| `group-active/name:` ... | The same, for an ancestor marked `group/name` | `.group_named("name")`; `.group_active_named("name", ..)`, ... |
| `first:`, `last:`, `odd:`, `even:`, `only:` | The element's own position among its siblings. To color a row's label by the row's position, put `last:text-*` on the row: the label inherits it | Plain Rust from the row's index (`at.is_first()`, `at.is_last()`, `at.index() % 2`) |
| `placeholder:` | Styles an input's placeholder (`placeholder:text-zinc-400`) | `.placeholder_color(..)` |
| `selection:` | Colors the selection highlight of an input or a selectable text, and of those inside the element (`selection:bg-violet-200`) | `.selection_color(..)` |
| `dark:` | The system appearance is dark | An `AdaptiveColor`, or `.dark(\|s\| ..)` |
| `sm:`, `md:`, `lg:`, `xl:`, `2xl:` | The window is at least 640, 768, 1024, 1280, 1536 points wide | `.style_when(\|\| window().width >= px(768.0), \|s\| ..)` |
| `max-sm:`, `max-md:`, ... | The window is narrower than the breakpoint | The same, with `<` |
| `min-[600px]:`, `max-[600px]:` | Arbitrary breakpoints | The same |
| `portrait:`, `landscape:` | The window's orientation | `window().width > window().height` |
| `ios:`, `android:`, `web:` | The platform | `.when(os() == Os::Ios, \|v\| ..)` |
| `terminal:` | Drawn in a terminal | `.when(idiom() == Idiom::Terminal, \|v\| ..)` |
| `pointer-fine:`, `pointer-coarse:` | The primary pointer is a mouse or trackpad (the desktops, a desktop browser, a terminal) / a finger | `.when(is_desktop(), \|v\| ..)` |
| `rtl:`, `ltr:` | The [layout direction](./layout.md#right-to-left) | `.direction(LayoutDirection::..)` on a view |
| `motion-safe:`, `motion-reduce:` | The user's reduce motion setting | `reduce_motion()` |

Variants combine: `dark:active:bg-zinc-700`, `md:landscape:flex-row`. In Rust, an `AdaptiveColor` in an `.active(..)` style is `dark:active:`.

```rust tabs
view()
    .group()
    .flex_row()
    .items_center()
    .gap_3()
    .rounded_xl()
    .bg(white())
    .p_3()
    .active(|s| s.bg(ZINC.shade(50)))
    .child(text("Settings").flex_1().text_color(ZINC.shade(900)))
    .child(
        text("›")
            .text_color(ZINC.shade(300))
            .transition_transform()
            .group_active(|s| s.translate_x(px(4.0))),
    )
```

```tsx tabs
<View class="group flex-row items-center gap-3 rounded-xl bg-white p-3 active:bg-zinc-50">
  <Text class="flex-1 text-zinc-900">Settings</Text>
  <Text class="text-zinc-300 transition-transform group-active:translate-x-1">›</Text>
</View>
```

`disabled` on any element (`.disabled(..)` in Rust) applies `disabled:` classes (`.disabled_style(..)`) and removes its press handlers. While disabled, the element skips its own `hover:`, `active:`, `focus:`, `focus-visible:` and `focus-within:` classes. Value classes such as `checked:` still follow the control’s value.

```rust tabs
view()
    .disabled(saving)
    .on_press(save)
    .rounded_lg()
    .bg(EMERALD.shade(500))
    .px_4()
    .py_2()
    .active(|s| s.bg(EMERALD.shade(600)))
    .disabled_style(|s| s.opacity(0.4))
    .child(text("Save").font_weight(FontWeight::MEDIUM).text_color(white()))
```

```tsx tabs
<View disabled={saving()} onPress={save} class="rounded-lg bg-emerald-500 px-4 py-2 active:bg-emerald-600 disabled:opacity-40">
  <Text class="font-medium text-white">Save</Text>
</View>
```

Breakpoints compare against the window (the surface NSH renders into), not the device: in an iPad split view or a resizable window, they follow the window's width, as `window()` does.

## What differs from Tailwind on the web

- **Units are points.** `p-4` (`.p_4()`) is 16 points (1 spacing step = 4pt, 1rem = 16pt). Arbitrary values in `px` are points too: `p-[13px]` (`.p(px(13.0))`) is 13pt.
- **`flex` is a row**, as in CSS: `class="flex"` lays out its children in a row unless a direction class says otherwise (anywhere in the string: `flex-col flex` is a column). A style's `display: "flex"` and Rust's `.flex()` are rows the same way, and a view that sets no display keeps NSH's default column. `block` and `flow-root` use block layout, and so do `inline` and `inline-block`, as a browser lays them out in a flex or grid parent. In Rust, `.flex_row()` and `.flex_col()` set the direction.
- **Text classes on views are inherited.** `text-*`, `font-*`, `leading-*`, `tracking-*` and the other text utilities on a view (`.text_color(..)`, `.font_weight(..)`, ... in Rust) pass to the text and input elements inside it, as CSS inheritance does: `<View class="text-white">` makes its labels white.
- **Every view is a box of its own.** `overflow-x-hidden` and `overflow-y-hidden` clip both axes, as `overflow-hidden` does (scrolling is the [scroll](../elements/scroll.md) element's); `invisible` hides the view and everything in it; `mix-blend-*` blends a view with everything drawn behind it; `block` boxes keep their own margins and their children's apart; an `absolute` view is placed in its nearest positioned ancestor, as in CSS; `fixed` lays a view out in its root view (other elements are placed as `absolute`).
- **Text keeps its whitespace.** Spaces and line breaks in a string are drawn as written, and `whitespace-*` sets the wrapping; the JSX compiler collapses the whitespace of literal JSX text, as JSX does.
- **Transitions animate property groups**: `transition` animates colors, opacity, shadows and transforms, `transition-all` also corner radii and border widths; filters, backdrop filters, gradients and layout take their new values at once.
- **`focus-visible:` follows the browsers' `:focus-visible`**: it applies while the focus shows. Tab, the arrow keys of a group and shortcuts show it; a press or a tap does not; a move the app makes (`focus()`, a calendar or a dialog opening) shows it after a key and not after a press; a text input shows it whenever it has focus. `focus:` applies whenever the element has focus, so the keyboard's ring goes on `focus-visible:`. The row the keys act on inside a component that keeps the focus (a tree's row, a list's highlighted option) takes `focus:` while it is that row and `focus-visible:` while the component's focus shows too.
- **State backgrounds show over gradients.** In CSS a gradient covers `background-color`, so `active:bg-*` on a `bg-linear-*` surface would never be seen; in NSH a background color from `active:`, `hover:`, `focus:` or `group-*:` replaces the gradient while that state holds (unless the state sets a gradient of its own, such as `active:from-*`). So do `activeStyle` and the other state styles, and animated background colors.
- **Text and touch settings are style keys**: `line-clamp-*` sets `numberOfLines`, `text-ellipsis` and `text-clip` set `textOverflow`, `truncate` both, `select-*` sets `userSelect` and `pointer-events-*` sets `pointerEvents`, so they work in state variants and lose to the inline `style` as every class does; `caret-*`, `placeholder:text-*` and `selection:bg-*` set the `caretColor`, `placeholderColor` and `selectionColor` styles the same way. In Rust these are `Styled` methods (`.number_of_lines(..)`, `.truncate()`, `.text_overflow(..)`, `.select_text()`, `.pointer_events(..)`, ...). `whitespace-nowrap` (`.whitespace_nowrap()`) keeps a text on one line and `text-pretty` (`.text_pretty()`) keeps a lone word off its last line; they have no style key, and a view's reach the texts in it. `object-*` set the `objectFit` and `objectPosition` styles of images and videos, which a style's keys replace as any other.
- **The supported set**: utilities and variants of Tailwind 4 that have a native meaning. `peer-*` variants, pseudo-elements (`before:`, `after:`) and arbitrary properties (`[mask-type:luminance]`) are reported as unknown classes, and so is every utility NSH has no native meaning for, so a class never means something else than in a browser. Rendering hints (`antialiased`, `will-change-transform`, `appearance-none`, `box-border`, ...) are accepted and change nothing.

Everything else works as documented by Tailwind 4: the color palette, opacity modifiers (`bg-black/40`), arbitrary values (`bg-[#34c759]`, `w-[72px]`, `grid-cols-[1fr_2fr]`; on `p`, `px`, `py`, `pt`, `pr`, `pb`, `pl` and the `m` utilities also the environment variables, `pt-[env(safe-area-inset-top)]`, `pb-[calc(env(keyboard-inset-height)+49px)]`, see [Safe area](../elements/safe-area.md) and [Keyboard](../elements/keyboard.md)), fractions (`w-1/2`, `translate-x-1/2`), shadows and rings, gradients (`bg-linear-to-r`, `bg-radial`, `bg-conic` with `from-` / `via-` / `to-`, or one written out, `bg-[linear-gradient(45deg,#f00,#00f)]`, with Tailwind's `image:` hint too), filters, `backdrop-blur-*`, `mix-blend-*`, transforms, `transition-*` and `duration-*`, `animate-spin` / `ping` / `pulse` / `bounce`, and grid, each with its typed method in Rust. The platform limits of [filters and blending](./styling.md#filters-and-blending) apply to classes too.

## Theme

Classes resolve against Tailwind 4's default theme. Extend it in `nsh.json` under `"tailwind"`; your entries are added to the defaults (or replace the default of the same name):

```json
{
  "tailwind": {
    "colors": {
      "brand": { "50": "#f5f3ff", "500": "#7c3aed", "600": "#6d28d9", "DEFAULT": "#7c3aed" },
      "ink": "#1f2328"
    },
    "radius": { "card": 14 },
    "fontFamily": { "display": "Pacifico" },
    "breakpoints": { "tablet": 600 }
  }
}
```

With this theme, `bg-brand-500`, `text-brand` (the `DEFAULT` shade), `border-ink`, `rounded-card`, `font-display` and `tablet:flex-row` are valid classes.

| Key | Values | Makes |
|---|---|---|
| `colors` | A CSS color, or an object of shades (`"DEFAULT"` for the bare name) | `bg-*`, `text-*`, `border-*`, `from-*`, `ring-*`, ... |
| `spacing` | Points per spacing step (default 4) | `p-*`, `m-*`, `gap-*`, `w-*`, ... |
| `rem` | Points per `rem` in arbitrary values (default 16) | |
| `breakpoints` | Points | `name:` variants |
| `container` | Points | `max-w-*` sizes |
| `fontSize` | Points, or `[size, lineHeight]` with the line height a multiple of the size | `text-*` |
| `fontWeight` | 1–1000 | `font-*` |
| `fontFamily` | A family name (a system family or an [imported font's](./fonts.md) name) | `font-*` |
| `tracking` | Ems | `tracking-*` |
| `leading` | Multiples of the font size | `leading-*` |
| `radius` | Points | `rounded-*` |
| `shadow`, `insetShadow`, `dropShadow` | A CSS `box-shadow` | `shadow-*`, `inset-shadow-*`, `drop-shadow-*` |
| `blur` | Points | `blur-*`, `backdrop-blur-*` |
| `perspective` | Points | `perspective-*` |
| `aspect` | A ratio (`1.5`) | `aspect-*` |
| `ease` | `[x1, y1, x2, y2]` | `ease-*` |

An entry NSH cannot read, an unknown key or a value of a form it does not know, is a warning and is left out alone (`nsh.json: tailwind: unknown key 'colours', ignored`), at run time too, so a theme written for a newer NSH keeps its other entries on an older one. The theme is compiled into the bundle and applied before the first frame.

The Tailwind theme shapes your classes. Typed methods keep Tailwind's default scales, so a Rust component looks the same in every app, and take the app's look from the design tokens of its theme, `"theme"` in `nsh.json` ([Rust](./rust.md#theme)).

## Theme tokens

The app's theme (`"theme"` in `nsh.json`, a [theme document](./rust.md#theme)) holds its design tokens: shadcn/ui's colors (`background`, `foreground`, `primary`, `muted-foreground`, `border`, `ring`, `chart-1`, `sidebar`, ...), its `radius`, its fonts `font-sans` and `font-mono`, and any token the app adds. Classes read them as Tailwind 4 reads the variables of a CSS `@theme`, Rust components read them with `theme()` from `nsh_base`, and TSX reads them with `token()` from `nsh`, so a TSX screen and the Rust components follow the same theme:

```rust tabs
use nsh_base::theme;

view()
    .rounded(move || theme().radius)
    .border_1()
    .border_color(move || theme().border)
    .bg(move || theme().card)
    .p_4()
    .child(text("Balance").text_color(move || theme().card_foreground))
    .child(text("Updated just now").text_color(move || theme().muted_foreground))
    .child(
        view()
            .rounded(move || theme().radius)
            .bg(move || theme().primary)
            .px_4()
            .py_2()
            .active(|s| s.bg(move || theme().primary.opacity(0.9)))
            .child(text("Continue").text_color(move || theme().primary_foreground)),
    )
```

```tsx tabs
<View class="rounded-(--radius) border border-border bg-card p-4">
  <Text class="text-card-foreground">Balance</Text>
  <Text class="text-muted-foreground">Updated just now</Text>
  <View class="rounded-(--radius) bg-primary px-4 py-2 active:bg-primary/90">
    <Text class="text-primary-foreground">Continue</Text>
  </View>
</View>
```

A class reads a **role**: the token the theme maps that name onto (`"roles": { "popover": "overlay" }`), else the token of that name. So `bg-primary` is the token `primary`, or the one the theme maps `primary` onto, and a component shared between apps writes `bg-popover rounded-card` once for every design system. In Rust, shadcn's tokens are fields of `theme()` (`primary`, `muted_foreground`, `radius`, `font_mono`), and every token is `tokens().color("up")`, `tokens().number("radius-card")` or `tokens().font("font-display")`, or a typed key's `get()`, which follows that token alone ([Theme documents and typed keys](./rust.md#theme-documents-and-typed-keys)).

| Class | Reads the role | Of kind | Example |
|---|---|---|---|
| `<utility>-<n>` of any color utility, with an opacity too | `<n>` | `color` | `bg-primary`, `text-muted-foreground/70`, `bg-up/30`, `border-t-border`, `ring-ring`, `from-accent`, `placeholder:text-muted-foreground` |
| `rounded-<n>`, `rounded-t-<n>`, `rounded-bl-<n>`, ... | `radius-<n>` | `number` | `rounded-card` |
| `p-<n>`, `m-<n>`, `gap-<n>`, `w-<n>`, `h-<n>`, `size-<n>`, `inset-<n>`, `top-<n>`, `space-x-<n>`, `translate-x-<n>`, ... | `spacing-<n>` | `number` | `p-gutter`, `-mt-gutter`, `gap-x-gutter` |
| `text-<n>` | `text-<n>` | `typography`, or a `number` for the size alone | `text-title`, `text-body` |
| `font-<n>` | `font-<n>`, then `font-weight-<n>` | `font`, then `number` | `font-display`, `font-strong` |
| `tracking-<n>` | `tracking-<n>` | `number`, in points | `tracking-label` |
| `leading-<n>` | `leading-<n>` | `number`, a multiple of the size | `leading-body` |
| `blur-<n>`, `backdrop-blur-<n>` | `blur-<n>` | `number`, or a `blur`'s radius | `blur-soft`, `backdrop-blur-glass` |
| `duration-<n>`, `delay-<n>` | `duration-<n>` | `duration` | `duration-quick` |
| `ease-<n>` | `ease-<n>` | `easing` | `ease-snappy` |
| `shadow-<n>` | `shadow-<n>` | `shadow` | `shadow-raised` |
| `bg-<n>` | `background-image-<n>` | `gradient` | `bg-hero` |
| `(--name)`, Tailwind 4's variable shorthand, and `(color:--name)`, `(length:--name)`, `(number:--name)`, `(family-name:--name)`, `(image:--name)` | `name` | any, typed by the hint where a utility takes several | `bg-(--popover)`, `rounded-(--radius)`, `text-(length:--text-title)`, `font-(family-name:--font-mono)`, `bg-(image:--background-image-hero)` |

- **Two ways to the same result.** `rounded-card` draws `corner-card` when the theme declares `corner-card` and maps
  `"radius-card": "corner-card"`, and the same when the theme declares a token `radius-card` and maps nothing. A
  mapping onto a token of a kind its row does not take (`"radius-card"` onto a color) is reported when the theme loads
  (`role-type`) and left out, and the role reads the token of its own name. Lengths are in points; `(number:--name)`
  passes the plain number.
- **Which comes first.** Where two rows can name a class, the longer utility comes first (`inset-shadow-card` is the
  inset shadow's color `card`, whatever `spacing-shadow-card` is). At one utility a namespaced role comes before a
  color of the same name, except for `text-<n>`, where a color comes first, then a typography, then a number;
  `font-<n>` is a font, then a weight, and `bg-<n>` a color, then a gradient. A role of a kind its row does not take
  is skipped there: the class falls to the next row, else to Tailwind's own utility, and is an unknown class when
  neither applies.
- **Compounds are read whole.** `text-title` sets its typography's size, line height (a multiple or points), letter
  spacing, weight and family together; a `leading-*`, `tracking-*` or `font-*` class of the same element wins over
  those parts wherever it is, as Tailwind 4's `--text-*--line-height` and friends do. `shadow-raised` draws the
  shadow's layers, inset ones included; `bg-hero` the gradient, linear, radial or conic; `blur-glass` and
  `backdrop-blur-glass` put the blur's radius in the filters and the backdrop filters, whatever the blur's own type.
- **The theme wins over Tailwind's values.** A role or token whose class is also one of Tailwind's built-in values
  wins over it, as Tailwind 4's `@theme` does: a theme's `shadow-md` makes the class `shadow-md` its shadow, so
  `token("shadow-md")` and `shadow-md` draw the same, and `radius-lg`, `text-lg`, `blur-md`, `ease-in`,
  `duration-150` and `spacing-4` (for `p-4`) do so for theirs. Without one, Tailwind's value stays, and so it does
  when the theme's role has another kind: `shadow-md` mapped onto a number draws Tailwind's `shadow-md`. A color
  role wins over a color of `"tailwind"` `colors` or Tailwind's palette of the same name too. The modifiers Tailwind's
  own classes take apply to the theme's: `text-sm/6` and `text-title/6` set the theme's size (a typography whole)
  with the modifier's line height, and `shadow-md/20` and `shadow-raised/20` draw the theme's layers at that alpha;
  other theme classes take no modifier but a color's opacity.
- **Fonts.** `font-sans` is the app's font: every text that sets no family of its own is drawn with it, as a page is with Tailwind's `--font-sans`. `font-mono` is the font of `font-mono` and of code in `TextView`. Unset, both are the system's fonts.
- **One value at a time.** A token has one value: the one of the contexts the theme's modifiers show. A class is one
  rule with that value, and a pick of another context (the theme's dark values, a brand,
  [`set_modifier`](./rust.md#changing-tokens-while-the-app-runs)) resolves again the classes that read a token it
  changed, and no other. `dark:` follows the appearance (the system's, or `nsh.json`'s `"appearance"`), whatever the
  theme's modifiers are: a theme shows its dark values when the `dark` context of its light/dark modifier is picked.
- **The theme can change while the app runs.** When an app embedded in a native app is given that app's theme, and whenever the theme changes, the classes reading it, the Rust bindings reading `theme()` and the TSX code reading `token()` are updated, the classes and bindings natively. A host that only adds its brand gives theme overrides, a patch of the values and roles its brand changes ([iOS](./ios.md#theme-overrides), [Android](./android.md#theme-overrides), `AppView::set_theme_overrides` on the desktops). A new value resolves again the classes that read it, and a patch that only maps a role onto another token the classes that read that role.
- **The app can set another theme.** [`setTheme(theme)`](../modules/nsh.md#settheme) from `nsh` makes another theme document the app's in place of its `"theme"`, while it runs, on every platform and in `nsh test`; `setTheme(null)` gives the app's own back, and `token()` reads the new tokens in the frame after the call's turn. A Rust app calls `set_theme(Some(document))`. Build it from the app's own theme so every role the classes read keeps a token.
- **Checked like other classes.** A name that is no role of the default theme or of the app's `"theme"` is an unknown class when building (`unknown class 'bg-chart-up'`). To use a token only the embedding app or a theme set while the app runs provides, give it a value in the app's `"theme"` too: that value shows when the app runs on its own.

Classes reading the theme are resolved once per change of what they read, like any other class: drawing them costs what a palette color does. A class the theme serves no role of is told by one lookup when it is first parsed.

### `token()` in TSX

`token(name)` from `nsh` is a token's value in the theme format's canonical form: a color as `#rrggbb` (`#rrggbbaa` below full opacity), a length as a number of points, a font as its family list, a compound such as a shadow or a gradient as an object of its members ([`token`](../modules/nsh.md#token)). It is reactive, so code that reads it in a component, a memo or an effect runs again when the token changes (another theme, a patch, a pick of a modifier's context), which suits values that go where no class does, such as a canvas or a chart:

```tsx
import { Canvas, token } from "nsh";

<Canvas
  style={{ height: 160 }}
  draw={(cx, { width, height }) => {
    cx.beginPath();
    cx.moveTo(0, height);
    cx.lineTo(width, 0);
    cx.strokeStyle = token("chart-1");
    cx.lineWidth = 2;
    cx.stroke();
  }}
/>
```

The names are typed: NSH writes the app's tokens into `.nsh/types/nsh/tokens.d.ts` from its theme, so `token("up")` is a `string` once `"up"` is in the theme, and a name the theme lacks is a type error. Each is typed by its kind as `token()` gives it (a `string` for a color or a font, a `number`, a `boolean`, `` `${number}ms` `` for a duration, an object of its members for a compound, an array of layers for a shadow) with its doc.

### Tokens for a part of the app

`ThemeScope` gives a part of the app values of its own, as a class that sets CSS variables does on the web. Its `theme` is a patch of the theme's values and roles (`{ values: { ... }, roles: { ... } }`, as `updateTheme` takes one), set over the tokens around it, and its `modifiers` pick contexts of the theme's modifiers for what is in it. Token classes, `token()` and the Rust components in it follow:

```rust tabs
use nsh_base::theme;
use nsh_core::prelude::*;

let brand = Patch::new().value("primary", Hsla::from(rgb(0x7c3aed)));

theme_scope().theme(Some(brand)).child(
    view()
        .rounded(move || theme().radius)
        .bg(move || theme().primary)
        .p_4()
        .child(text("Upgrade").text_color(move || theme().primary_foreground)),
)
```

```tsx tabs
import { Text, ThemeScope, View } from "nsh";

<ThemeScope theme={{ values: { primary: "#7c3aed" } }}>
  <View class="rounded-(--radius) bg-primary p-4">
    <Text class="text-primary-foreground">Upgrade</Text>
  </View>
</ThemeScope>
```

A scope sets values of tokens the theme declares, each of its kind: an entry the theme refuses (a name it lacks, a value of another kind, a mapping onto a token of a kind the role does not take) is left out with a warning, so every class means one token of one kind everywhere. A mapping in it (`roles: { popover: "overlay" }`) moves the classes that read the role, in the scope alone. The tokens it leaves alone stay the app's and follow `setTheme` and `updateTheme`; a scope inside another starts from the outer one's values and picks. With `appearance` a scope also shows its content light or dark ([Device and environment](./device-and-environment.md#a-part-of-the-app-in-its-own-appearance)): that decides `dark:` classes there, never token values, so a scope shows the theme's dark values by also picking them, `modifiers={{ colorMode: "dark" }}` (`.modifiers(Some(ScopeModifiers::new([("colorMode", "dark")])))` in Rust).

## The class check

`nsh build`, `nsh dev`, `nsh run` and `nsh test` check the static class names in your JSX (string literals in `class` and the keys of `classList`) with the same parser and theme the runtime uses, and warn about classes that do not exist:

```text
 warn nsh::build App.tsx:12:36: unknown class `bg-brand-550`
```

Warnings do not fail the build. Classes built at run time (``bg-${color()}-500``) are checked when the runtime first sees them, which logs each unknown class once (`unknown classes: ...`). Typed methods are checked by the Rust compiler.

## Classes or styles?

Both are resolved natively and cost about the same. Classes are shorter and give you variants (pressed states, dark mode, breakpoints) without code. Inline styles take values computed in JavaScript and accept [native animated values](./animation.md) (`opacity: fade`, `translate: [x, 0]`). Most TSX apps use classes for the static look and `style` for what is computed or animated. Rust components use typed methods for both, and bind animated values with `.bind_opacity(..)`, `.bind_translate_x(..)` and the other bindings.

## Related

- [Styling](./styling.md): the style prop, the typed methods and the platform limits of effects.
- [Tailwind reference](../modules/tailwind-reference.md): every utility and variant.
- [Layout](./layout.md): flexbox and grid, also available as classes.
