---
title: Tailwind
description: TSX 中以原生方式解析的 Tailwind CSS 类及其在 Rust 中的类型化写法：class 与 classList，按下状态、深色模式和断点等变体，nsh.json 中的主题，以及构建时的类名检查。
order: 33
group: ui
demo: guide-tailwind
---

# Tailwind

NSH 的样式遵循 Tailwind CSS 4。在 Rust 中，Tailwind 的工具类是名称和取值都相同的类型化样式方法（`.p_4()` 即 `p-4`，`.rounded_2xl()` 即 `rounded-2xl`，`ZINC.shade(900)` 即 `zinc-900`）；在 TSX 中，每个元素都可以直接在 `class` 中写这些类，它们会以 Tailwind 4 的默认主题为准，在原生层解析。下面每个示例先给出类型化方法，再给出类名：

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

类型化方法生成的样式规则与类名编译出的完全相同，无需解析字符串。对于类名，整个过程不生成 CSS，也不在 JavaScript 中运行任何代码：字符串被直接交给核心，核心对每个不同的类字符串只解析一次（并缓存），把它转换为样式规则后应用。依赖状态的变体（按下、深色模式、窗口宽度）会在相应状态变化时由核心重新应用；按下和聚焦状态在原生层跟踪，因此 `active:` 的反馈（Rust 中的 `.active(..)`）无需与你的代码往返。

类名和 [`style` 属性](./styling.md) 产生的是同样的原生样式，两者可以组合使用；本页介绍类名特有的部分及其类型化写法。[Tailwind 参考](../modules/tailwind-reference.md) 列出了所有支持的工具类。

## class 与 classList

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

- **`class`** 是一个类名字符串，可以是动态的：``class={`h-6 rounded bg-${color()}-500`}`` 可以正常工作，因为类名在运行时解析。
- **`classList`** 把类名映射到条件：值为真时该类生效。它在 `class` 之后应用。
- **在 Rust 中**，传入闭包或 signal 的方法会跟随它变化（`.bg(move || ..)`），`.style_when(condition, |s| ..)` 在条件成立时应用样式，`.when(condition, |v| ..)` 则在创建元素时一次性决定。

冲突的处理方式：

1. 同一字符串中**靠后的类优先**：`"p-2 p-4"` 的结果是 `p-4`。不需要 `tailwind-merge`：组件可以直接把调用方传入的 `class` 追加在后面。在 Rust 中同样是后调用的方法优先。
2. **带变体的类在普通类之后应用**，并按 Tailwind 的变体顺序排列（`hover:` 在 `active:` 之前，`max-*:` 从宽到窄，断点从小到大，`dark:` 最后），与它们在字符串中的位置无关。与 Tailwind 一样，先比较类中顺序最靠后的变体，再比较次靠后的变体，多一个变体的类排在后面：深色模式下悬停时 `dark:hover:bg-a dark:bg-b` 是 `bg-a`，宽度小于 `md` 时 `max-md:p-2 max-lg:p-4` 是 `p-2`。和 CSS 一样，`odd:bg-a last:bg-b` 会让位于奇数位置的最后一行得到 `bg-a`，因为 Tailwind 把 `odd` 排在 `last` 之后。
3. **`style` 优先于类名。** 样式键就是与它同名的 CSS 属性，也就是类名设置的那个属性，因此它只替换这一个属性：`style={{ paddingLeft: 8 }}` 替换 `pl-4`，保留 `pr-4`。
4. `!` 让一个类变为 important：`!p-0`。

Rust 组件（`nsh/base` 的以及应用自己 crate 中的组件）在 TSX 中同样接受 `class` 和 `classList`；你的类会合并在组件自身样式之后，同等优先级时以你的为准。

## 变体

| 变体 | 生效条件 | Rust |
|---|---|---|
| `active:` | 视图被按下 | `.active(\|s\| ..)` |
| `hover:` | 指针悬停在视图上（iPad 和 Mac 的指针） | `.hover(\|s\| ..)` |
| `focus:` | 元素获得焦点 | `.focus(\|s\| ..)` |
| `focus-visible:` | 元素获得焦点并显示焦点：键盘把焦点移到了这里 | `.focus_visible(\|s\| ..)` |
| `focus-within:` | 视图或其后代获得焦点（`Overlay` 的内容是它所在视图的后代） | `.focus_within(..)` |
| `disabled:`、`enabled:` | 元素的 `disabled` 属性为 true / false | `.disabled_style(\|s\| ..)` |
| `selected:` | 元素处于选中状态（`Button` 的 `selected`、`accessibilityState.selected`） | `.selected_style(..)` |
| `checked:` | 元素处于勾选状态（`accessibilityState.checked` 为 `true`；`"mixed"` 不算） | `.checked_style(..)` |
| `indeterminate:` | 元素处于半选状态（`accessibilityState.checked` 为 `"mixed"`） | `.indeterminate_style(..)` |
| `aria-expanded:` | 元素处于展开状态（`accessibilityState.expanded` 为 `true`）：弹出层打开时 `Popover`、`Menu` 的触发器 | `.expanded_style(..)` |
| `aria-pressed:` | 元素是处于开启状态的切换按钮（`accessibilityState.pressed` 为 `true`；`"mixed"` 不算按下）。不同于 `active:`（正在按压） | `.aria_pressed(..)`、`.pressed_style(..)` |
| `aria-busy:`、`aria-checked:`、`aria-selected:` | `accessibilityState.busy`、`checked`（`true`，不含 `"mixed"`）或 `selected` 为 `true`，与 Tailwind 的 `aria-*` 变体一致：排在伪类之后，按名称排序 | |
| `data-<name>:` | 元素带有名为 `name` 的 `data-*` 属性，与 Tailwind 的布尔 `data-*` 变体一致（`&[data-range-start]`）：组件的部件在状态之外显示的信息，比如日历中区间起点的那一天（`data-range-start:`）。排在 `aria-*` 变体之后，按名称排序 | 组件的部件上用 `Interactive::project_data(name, ..)` 设置 |
| `group-active:`、`group-hover:`、`group-focus:`、`group-focus-within:`、`group-disabled:`、`group-selected:`、`group-checked:`、`group-indeterminate:`、`group-aria-expanded:`、`group-aria-pressed:`、`group-aria-busy:`、`group-aria-checked:`、`group-aria-selected:`、`group-data-<name>:` | 最近的带 `group` 类的祖先处于该状态，或带有该 `data-*` 属性 | 祖先上 `.group()`；内部 `.group_active(..)`、`.group_hover(..)`、`.group_focus_within(..)` |
| `group-active/name:` ... | 同上，针对标记为 `group/name` 的祖先 | `.group_named("name")`；`.group_active_named("name", ..)` 等 |
| `first:`、`last:`、`odd:`、`even:`、`only:` | 元素自身在兄弟节点中的位置。要按行的位置给行内标签上色，把 `last:text-*` 写在行上，标签会继承它 | 根据行的索引用普通 Rust 判断（`at.is_first()`、`at.is_last()`、`at.index() % 2`） |
| `placeholder:` | 设置输入框占位文字的样式（`placeholder:text-zinc-400`） | `.placeholder_color(..)` |
| `selection:` | 设置输入框或可选文本的选区高亮颜色，也作用于元素内的输入框和文本（`selection:bg-violet-200`） | `.selection_color(..)` |
| `dark:` | 系统外观为深色 | `AdaptiveColor`，或 `.dark(\|s\| ..)` |
| `sm:`、`md:`、`lg:`、`xl:`、`2xl:` | 窗口宽度至少为 640、768、1024、1280、1536 点 | `.style_when(\|\| window().width >= px(768.0), \|s\| ..)` |
| `max-sm:`、`max-md:` ... | 窗口宽度小于该断点 | 同上，使用 `<` |
| `min-[600px]:`、`max-[600px]:` | 任意断点 | 同上 |
| `portrait:`、`landscape:` | 窗口方向 | `window().width > window().height` |
| `ios:`、`android:`、`web:` | 平台 | `.when(os() == Os::Ios, \|v\| ..)` |
| `terminal:` | 在终端中绘制 | `.when(idiom() == Idiom::Terminal, \|v\| ..)` |
| `pointer-fine:`、`pointer-coarse:` | 主要指针是鼠标或触控板（桌面、桌面浏览器、终端）/ 手指 | `.when(is_desktop(), \|v\| ..)` |
| `rtl:`、`ltr:` | [布局方向](./layout.md#从右到左) | 视图的 `.direction(LayoutDirection::..)` |
| `motion-safe:`、`motion-reduce:` | 用户的“减弱动态效果”设置 | `reduce_motion()` |

变体可以组合：`dark:active:bg-zinc-700`、`md:landscape:flex-row`。在 Rust 中，`.active(..)` 样式里的 `AdaptiveColor` 就相当于 `dark:active:`。

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

任何元素上的 `disabled`（Rust 中的 `.disabled(..)`）都会应用 `disabled:` 类（`.disabled_style(..)`），并把它的按下处理器从挂载内容中移除，因此也不会显示按下反馈：

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

断点比较的是窗口（NSH 渲染所在的表面）而不是设备：在 iPad 分屏或可调整大小的窗口中，它们跟随窗口宽度变化，`window()` 也是如此。

## 与 Web 上的 Tailwind 的差异

- **单位是点（point）。** `p-4`（`.p_4()`）是 16 点（1 个间距步长 = 4pt，1rem = 16pt）。以 `px` 写的任意值也按点计算：`p-[13px]`（`.p(px(13.0))`）就是 13pt。
- **`flex` 是横向排列**，与 CSS 一致：`class="flex"` 会让子元素横向排列，除非有方向类另行指定（在字符串中的任何位置都可以：`flex-col flex` 是纵向）。样式中的 `display: "flex"` 和 Rust 的 `.flex()` 同样是横向，未设置 display 的视图保持 NSH 默认的纵向排列。`block` 和 `flow-root` 使用块布局，`inline` 和 `inline-block` 也是如此，与浏览器在 flex 或 grid 父元素中的布局一致。在 Rust 中由 `.flex_row()` 和 `.flex_col()` 设置方向。
- **视图上的文字类会被继承。** 视图上的 `text-*`、`font-*`、`leading-*`、`tracking-*` 等文字工具类（Rust 中的 `.text_color(..)`、`.font_weight(..)` 等）会像 CSS 继承那样传给其内部的文本和输入元素：`<View class="text-white">` 会让其中的标签变成白色。
- **每个视图都是独立的盒子。** `overflow-x-hidden` 和 `overflow-y-hidden` 与 `overflow-hidden` 一样裁剪两个方向（滚动由 [scroll](../elements/scroll.md) 元素负责）；`invisible` 隐藏视图及其中的一切；`mix-blend-*` 让视图与其后绘制的一切混合；`block` 盒子的外边距与其子元素的外边距互不合并；`absolute` 视图与 CSS 一样，相对最近的定位祖先放置；`fixed` 让视图在其根视图中布局（其他元素按 `absolute` 定位）。
- **文本保留空白。** 字符串中的空格和换行按原样绘制，`whitespace-*` 设置换行方式；JSX 编译器会像 JSX 一样合并字面 JSX 文本中的空白。
- **过渡按属性组生效**：`transition` 对颜色、不透明度、阴影和变换做动画，`transition-all` 还包括圆角和边框宽度；滤镜、背景滤镜、渐变和布局会直接取新值。
- **`focus-visible:` 与浏览器的 `:focus-visible` 一致**：在焦点显示时应用。Tab、组内的方向键和快捷键会显示焦点；按压或轻点不会；应用移动的焦点（`focus()`、日历或对话框打开时）在按键之后显示，在按压之后不显示；文本输入框获得焦点时总是显示。`focus:` 在元素获得焦点时就会应用，因此键盘的焦点环写在 `focus-visible:` 上。组件自己保留焦点时，按键作用的那一行（树的行、列表高亮的选项）在作为这一行时应用 `focus:`，组件的焦点显示时还应用 `focus-visible:`。
- **状态背景色显示在渐变之上。** 在 CSS 中渐变会盖住 `background-color`，因此 `bg-linear-*` 表面上的 `active:bg-*` 永远看不到；在 NSH 中，来自 `active:`、`hover:`、`focus:` 或 `group-*:` 的背景色会在该状态期间替换渐变（除非该状态设置了自己的渐变，例如 `active:from-*`）。`activeStyle` 等状态样式以及动画背景色也是如此。
- **文字与触摸的设置都是样式键**：`line-clamp-*` 设置 `numberOfLines`，`text-ellipsis` 和 `text-clip` 设置 `textOverflow`，`truncate` 两者都设置，`select-*` 设置 `userSelect`，`pointer-events-*` 设置 `pointerEvents`，因此它们可以用在状态变体中，并且和所有类一样会被内联 `style` 覆盖；`caret-*`、`placeholder:text-*` 和 `selection:bg-*` 同样设置 `caretColor`、`placeholderColor` 和 `selectionColor` 样式。在 Rust 中它们是 `Styled` 方法（`.number_of_lines(..)`、`.truncate()`、`.text_overflow(..)`、`.select_text()`、`.pointer_events(..)` 等）。`whitespace-nowrap` 以类的形式让文本保持一行。`object-*` 设置图片和视频的 `objectFit` 与 `objectPosition` 样式，样式中的同名键会像其他样式一样替换它们。
- **支持的范围**：Tailwind 4 中在原生层有意义的工具类和变体。`peer-*` 变体、伪元素（`before:`、`after:`）和任意属性（`[mask-type:luminance]`）会报告为未知类；NSH 在原生层没有对应含义的工具类也都是未知类，因此一个类不会有与浏览器不同的含义。渲染提示类（`antialiased`、`will-change-transform`、`appearance-none`、`box-border` 等）会被接受，但不改变任何东西。

其余部分都与 Tailwind 4 文档一致：调色板、透明度修饰符（`bg-black/40`）、任意值（`bg-[#34c759]`、`w-[72px]`、`grid-cols-[1fr_2fr]`；`p`、`px`、`py`、`pt`、`pr`、`pb`、`pl` 和 `m` 系列工具类还接受环境变量，`pt-[env(safe-area-inset-top)]`、`pb-[calc(env(keyboard-inset-height)+49px)]`，见[安全区域](../elements/safe-area.md)和[键盘](../elements/keyboard.md)）、分数（`w-1/2`、`translate-x-1/2`）、阴影和 ring、渐变（`bg-linear-to-r`、`bg-radial`、`bg-conic` 配合 `from-` / `via-` / `to-`，或直接写出的渐变 `bg-[linear-gradient(45deg,#f00,#00f)]`，也可带 Tailwind 的 `image:` 提示）、滤镜、`backdrop-blur-*`、`mix-blend-*`、变换、`transition-*` 和 `duration-*`、`animate-spin` / `ping` / `pulse` / `bounce`，以及网格，每一项在 Rust 中都有对应的类型化方法。[滤镜与混合](./styling.md#滤镜与混合) 的平台限制对类名同样适用。

## 主题

类名以 Tailwind 4 的默认主题为准解析。在 `nsh.json` 的 `"tailwind"` 下扩展它；你的条目会加入默认值（同名时替换默认值）：

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

有了这个主题，`bg-brand-500`、`text-brand`（即 `DEFAULT` 色阶）、`border-ink`、`rounded-card`、`font-display` 和 `tablet:flex-row` 都是有效的类名。

| 键 | 取值 | 生成 |
|---|---|---|
| `colors` | 一个 CSS 颜色，或色阶对象（`"DEFAULT"` 对应不带后缀的名称） | `bg-*`、`text-*`、`border-*`、`from-*`、`ring-*` 等 |
| `spacing` | 每个间距步长的点数（默认 4） | `p-*`、`m-*`、`gap-*`、`w-*` 等 |
| `rem` | 任意值中每个 `rem` 的点数（默认 16） | |
| `breakpoints` | 点 | `name:` 变体 |
| `container` | 点 | `max-w-*` 尺寸 |
| `fontSize` | 点，或 `[size, lineHeight]`，行高为字号的倍数 | `text-*` |
| `fontWeight` | 1–1000 | `font-*` |
| `fontFamily` | 字体族名（系统字体族或[导入字体](./fonts.md)的名称） | `font-*` |
| `tracking` | em | `tracking-*` |
| `leading` | 字号的倍数 | `leading-*` |
| `radius` | 点 | `rounded-*` |
| `shadow`、`insetShadow`、`dropShadow` | 一个 CSS `box-shadow` | `shadow-*`、`inset-shadow-*`、`drop-shadow-*` |
| `blur` | 点 | `blur-*`、`backdrop-blur-*` |
| `perspective` | 点 | `perspective-*` |
| `aspect` | 比例（`1.5`） | `aspect-*` |
| `ease` | `[x1, y1, x2, y2]` | `ease-*` |

NSH 读不懂的条目（不认识的键，或形式不认识的值）会给出警告并单独略过（`nsh.json: tailwind: unknown key 'colours', ignored`），运行时也一样，因此为更新的 NSH 写的主题在较旧的 NSH 上其余条目照常生效。主题会编译进 bundle，并在第一帧之前生效。

Tailwind 主题影响你的类名。类型化方法保持 Tailwind 的默认刻度，因此 Rust 组件在任何应用中看起来都一样，它们从应用主题的设计令牌，即 `nsh.json` 中的 `"theme"` 获取应用的外观（[用 Rust 开发](./rust.md#主题)）。

## 主题 token

应用的主题（`nsh.json` 中的 `"theme"`）保存它的设计令牌：shadcn/ui 的颜色（`background`、`foreground`、`primary`、`muted-foreground`、`border`、`ring`、`chart-1`、`sidebar` 等）、`radius`、字体 `font-sans` 和 `font-mono`，以及应用自己添加的令牌。类名像 Tailwind 引用 CSS `@theme` 变量那样引用它们，Rust 组件用 `nsh_base` 的 `theme()` 读取，TSX 用 `nsh` 的 `token()` 读取，所以 TSX 页面和 Rust 组件使用同一套主题：

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

token 的名称就是主题文件中 CSS 变量去掉 `--` 后的名称：`muted-foreground`、`radius`、`chart-1`。在 Rust 中，shadcn 的令牌是 `theme()` 的方法（`primary()`、`muted_foreground()`、`radius()`、`font_mono()`），所有令牌也都可以用 `theme().color("up")`、`theme().number("radius-card")` 或 `theme().font("font-display")` 读取。

| 写法 | 取值 | 示例 |
|---|---|---|
| 任意颜色类里的颜色名 | 主题颜色，也可带不透明度 | `bg-primary`、`text-muted-foreground/70`、`bg-up/30`、`border-t-border`、`ring-ring`、`from-accent`、`placeholder:text-muted-foreground` |
| `(--name)`，Tailwind 4 的变量简写 | 主题颜色或数值，或同名主题角色被映射到的 token | `bg-(--primary)`、`rounded-(--radius)`、`p-(--gap-card)`、`bg-(--popover)` |
| `(color:--name)`、`(length:--name)`、`(number:--name)`、`(family-name:--name)` | 同上，在一个类可以接受多种值时标明类型 | `text-(color:--up)`、`text-(length:--text-title)`、`font-(family-name:--font-mono)` |
| `font-sans`、`font-mono` | 主题设置的字体 | `font-mono` |
| 数字 token 名称在其命名空间的工具类中 | Tailwind 4 命名空间中的数字 token，按名称的其余部分 | `rounded-card`（`radius-card`）、`p-gutter`、`gap-x-gutter`（`spacing-gutter`）、`text-body`（`text-body`）、`font-strong`（`font-weight-strong`）、`tracking-label`、`leading-body`、`blur-soft` |

- **应用自己的令牌。** 主题文件中的每个名称都是 token，就像 CSS `@theme` 中的每个变量：`"up": "#16a34a"` 生成 `bg-up`、`text-up` 和 `bg-up/30`；`"radius-card": "14px"` 生成 `rounded-(--radius-card)`；`"font-display": "Pacifico"` 生成 `font-(family-name:--font-display)`。长度的单位是点；`(number:--name)` 传入不带单位的数值。
- **主题角色。** 变量也可以指名[主题角色](../modules/nsh.md#role)：当主题没有名为 `popover` 的 token，并把角色 `popover` 映射到自己的 token `overlay`（主题类型的 `role = …`，或主题文件的 `"roles": { "popover": "overlay" }`）时，`bg-(--popover)` 在每种外观下都是 `overlay` 的颜色，并随它变化。因此在多个应用之间共享的组件只需写一次 `bg-(--popover) rounded-(--radius)`，就适用于每个设计系统。同名的 token 优先，而不带括号的类（`bg-popover`）只指名 token。构建时会根据主题映射的角色检查该变量。
- **字体。** `font-sans` 是应用的字体：没有设置自己字体族的文字都用它绘制，就像网页使用 Tailwind 的 `--font-sans`。`font-mono` 是 `font-mono` 类和 `TextView` 中代码的字体。未设置时两者都是系统字体。
- **两种外观。** token 带有主题的浅色值和深色值：`bg-primary`、`theme().primary` 和 `token("primary")` 都会随外观自动切换，不需要 `dark:` 类。只在一种外观里设置的颜色在两种外观下都使用该值。数字也可以有深色值：在浅色的 `14px` 之外写 `"dark": { "radius-card": "6px" }`，`rounded-card` 和 `rounded-(--radius-card)` 在浅色外观下为 14 点，在深色外观下为 6 点，`token("radius-card")` 取当前显示外观的值；主题只给出一次的数字在两种外观下相同。
- **具名数字类名。** 名称以 Tailwind 4 某个命名空间开头的数字 token，是该命名空间工具类的一个类名，按名称的其余部分命名，就像 Tailwind 中 `--radius-card` 产生 `rounded-card`：`radius-` 对应圆角工具类（`rounded-card`、`rounded-t-card`、`rounded-bl-card`）；`spacing-` 对应内边距、外边距、间距、尺寸、偏移和平移（`p-gutter`、`-mt-gutter`、`gap-x-gutter`、`w-gutter`、`inset-x-gutter`）；`text-` 对应字号；`font-weight-` 对应字重（`"font-weight-strong": "650"` 产生 `font-strong`）；`tracking-`、`leading-` 和 `blur-` 对应各自的工具类。`tracking-` token 是以点为单位的长度（`px` 或 `rem`）；`em` 单位的值会被略过，shadcn 的 `tracking-normal: 0em` 一直如此。它们可以用在变体下，并像所有 token 类名一样跟随主题。`text-` 也是颜色工具类：同名的颜色在那里仍然优先；其他情况下较长的工具类优先（`inset-shadow-card` 是内阴影的颜色 `card`）。
- **渐变 token。** 写成 CSS 渐变的 `background-image-<n>`（`"background-image-hero": "linear-gradient(135deg, #7c3aed, #db2777)"`，可在 `light` 和 `dark` 中分别给值；线性、径向和锥形渐变，可重复，一个或多个）是渐变 token，与 Tailwind 4 的 `--background-image-*` 一样：类名 `bg-hero` 和 `bg-(image:--background-image-hero)` 按当前显示的外观绘制它。色标中的 `var()` 是主题在该外观下的颜色（在 `theme` 中写一次的 `linear-gradient(to right, var(--primary), var(--accent))` 在两种外观下各是一个渐变）。名为 `hero` 的颜色或 Tailwind 自己的类名会保留 `bg-hero` 的原意，构建时会像数字 token 一样列出该 token；这时用 `(image:--…)` 的形式读取它。在 Rust 中，`.adaptive_gradient(move || tokens().gradient("background-image-hero").cloned().unwrap_or_default())` 跟随主题和外观；`token("background-image-hero")` 是它的 CSS，可以直接用作 `backgroundImage` 样式。
- **阴影 token。** 写成 CSS `box-shadow` 的 `shadow-<n>`（`"shadow-raised": "0 1px 2px #0000001a"`，可在 `light` 和 `dark` 中分别给值）是阴影 token：类名 `shadow-raised` 和 `shadow-(--shadow-raised)` 按当前显示的外观绘制它的各层阴影。主题中其他 `shadow-*` 名称（shadcn 的 `shadow-color`、`shadow-opacity`）仍是原来的颜色和数字；`shadow-sm`、`shadow-md` 等仍是 Tailwind 的类名，和其他 Tailwind 名称一样，且不会提示（tweakcn 导出的主题会写它们）：用 `shadow-(--shadow-sm)` 读取这样的 token。在 Rust 中，`.adaptive_shadow(move || tokens().shadow("shadow-raised").cloned().unwrap_or_default())` 跟随主题和外观；`token("shadow-raised")` 是它的 CSS。
- **Tailwind 的名称优先。** 当某个 token 的命名空间类名与 Tailwind 自己的名称相同时，Tailwind 的名称保持原意：`"radius-lg"` 不改变 Tailwind 的 `rounded-lg`，`"spacing-md"` 让 `p-md` 成为该 token 的类名，而 `max-w-md` 仍是 Tailwind 的宽度。构建时会列出该 token 不对应的类名（`` `spacing-md` is no class where Tailwind's or a color's keeps its meaning: `max-w-md`, … ``），与颜色 `card` 同名的字号 `text-card` 也是如此；可以用 `rounded-(--radius-lg)` 读取这样的 token。
- **你的 Tailwind 颜色优先。** `"tailwind"` 的 `colors` 或 Tailwind 调色板里的名称就是那个颜色，所以 Tailwind 主题里没有 `primary` 这个颜色时，`bg-primary` 是主题 token。
- **主题可以在运行时改变。** 嵌入原生应用的 NSH 应用拿到宿主的主题时，以及主题每次变化时，引用 token 的类、读取 `theme()` 的 Rust 绑定和读取 `token()` 的 TSX 代码都会更新，其中类和绑定在原生侧更新。宿主主题里没有的 token 使用应用自己 `"theme"` 中的值（再没有则用默认主题的值）。只想加上自己品牌的宿主给出主题覆盖，即用同样的文件格式写出其品牌要改的 token（[iOS](./ios.md#主题覆盖)、[Android](./android.md#主题覆盖)，桌面端为 `AppView::set_theme_overrides`）。
- **应用可以换一个主题。** `nsh` 的 [`setTheme(theme)`](../modules/nsh.md#settheme) 在应用运行时把一个同样格式的主题设为应用的主题，取代它的 `"theme"`，在每个平台和 `nsh test` 里都一样；`setTheme(null)` 恢复应用自己的主题。`token()` 立即读到新的 token。主题要整个读得懂：有读不懂的值时抛出列出该值的 `TypeError`，正在显示的主题保持不变。请从应用自己的主题文件出发构造新主题，这样类名引用的每个 token 都还有值。Rust 应用用 `set_tokens(theme.into())` 设置一个 `nsh_base::Theme` 的 token。
- **与其他类名一样检查。** 既不是默认主题、也不是应用 `"theme"` 中 token 的名称，在构建时会报告为未知类（`unknown class 'bg-chart-up'`）。要使用只有宿主或运行时设置的主题提供的 token，在应用的 `"theme"` 里也给它一个值：应用单独运行时显示这个值。
- **主题类型。** 应用的 crate 注册了[主题类型](./rust.md#主题类型)时，各类型的 token 就是主题的 token，以类型的默认值为起点：类名像使用任何 token
  一样使用它们（`bg-surface-raised`、`text-action/70`，`radius-card` 对应 `rounded-card`，`spacing-gutter` 对应 `p-gutter`），`token()` 读取它们。
  角色是 NSH 自己的部分绘制时使用的，类名使用的是 token：设计系统把 `primary` 角色映射到 `action` 时，它的类名是 `bg-action`，而只有某个类型声明了
  `primary`，`bg-primary` 才是已知的类名。

引用 token 的类和其他类一样，每套主题只解析一次：绘制时的开销与调色板颜色相同。

### TSX 中的 `token()`

`nsh` 的 `token(name)` 是令牌在当前外观下的值：颜色是 `#rrggbbaa`，长度是以点为单位的数值，字体是字体族名称。它是响应式的，在组件、memo 或 effect 中读取它的代码会在主题或外观变化时重新运行，适合类名用不到的地方，例如画布或图表：

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

名称带有类型：NSH 根据主题把应用的令牌写入 `.nsh/types/nsh/tokens.d.ts`，所以主题中有 `"up"` 后 `token("up")` 的类型是 `string`，主题中没有的名称是类型错误。使用主题类型时，它列出各类型的 token，每个按其种类标注类型（`number`，颜色、字体和阴影为 `string`），并附带文档注释。

### 应用某一部分的 token

`ThemeScope` 让应用的一部分使用它自己的主题 token 值，就像 Web 上设置 CSS 变量的类一样。它的 `theme` 是一个主题文件，其中的值叠加在它外层的 token 之上；其中的 token 类、`token()` 和 Rust 组件在每种外观下都跟随它：

```rust tabs
use nsh_base::theme;
use nsh_core::prelude::*;

let brand = r##"{ "cssVars": { "light": { "primary": "#7c3aed" }, "dark": { "primary": "#a78bfa" } } }"##;

theme_scope().theme(Some(ScopeTheme::file(brand))).child(
    view()
        .rounded(move || theme().radius)
        .bg(move || theme().primary)
        .p_4()
        .child(text("Upgrade").text_color(move || theme().primary_foreground)),
)
```

```tsx tabs
import { Text, ThemeScope, View } from "nsh";

<ThemeScope theme={{ cssVars: { light: { primary: "#7c3aed" }, dark: { primary: "#a78bfa" } } }}>
  <View class="rounded-(--radius) bg-primary p-4">
    <Text class="text-primary-foreground">Upgrade</Text>
  </View>
</ThemeScope>
```

作用域只设置应用主题已有的 token 的值：主题没有的名称会被略过并给出警告，所以每个类在各处都对应一个 token。它没有设置的 token 仍是应用的，并跟随 `setTheme` 和 `updateTheme`；嵌套在内的作用域从外层作用域的值开始。设置 `appearance` 后，作用域还会以浅色或深色显示其内容（[设备与环境](./device-and-environment.md#a-part-of-the-app-in-its-own-appearance)）。

## 类名检查

`nsh build`、`nsh dev`、`nsh run` 和 `nsh test` 会用运行时相同的解析器和主题检查 JSX 中的静态类名（`class` 中的字符串字面量以及 `classList` 的键），并对不存在的类发出警告：

```text
 warn nsh::build App.tsx:12:36: unknown class `bg-brand-550`
```

警告不会导致构建失败。运行时拼接的类名（``bg-${color()}-500``）在运行时第一次遇到时检查，每个未知类记录一次日志（`unknown classes: ...`）。类型化方法由 Rust 编译器检查。

## 用类名还是样式？

两者都在原生层解析，开销相当。类名更简短，并且无需写代码就能使用变体（按下状态、深色模式、断点）。内联样式可以接收在 JavaScript 中计算出的值，也能接受[原生动画值](./animation.md)（`opacity: fade`、`translate: [x, 0]`）。大多数 TSX 应用用类名描述静态外观，用 `style` 处理需要计算或动画的部分。Rust 组件两者都使用类型化方法，并用 `.bind_opacity(..)`、`.bind_translate_x(..)` 等绑定动画值。

## 相关内容

- [样式](./styling.md)：style 属性、类型化方法以及各种效果的平台限制。
- [Tailwind 参考](../modules/tailwind-reference.md)：所有工具类和变体。
- [布局](./layout.md)：flexbox 与网格，同样可以用类名表达。
