---
title: 用 Rust 开发
description: 完全用 Rust 编写 Native Shell 应用，在所有平台上都不带 QuickJS；或者在 Rust 之上用 TSX 编写页面：如何选择，以及两者如何共享组件和模块。
order: 5.5
group: start
---

# 用 Rust 开发

Native Shell 应用可以用 Rust 编写，也可以用 TSX，或者两者结合。三种方式运行在同一个引擎上：响应式图、渲染器、Taffy
布局以及原生模块（导航、对话框、键盘、状态栏）始终是 Rust，各平台都用自己的原生 UI 渲染。不同的是你自己的代码用什么语言，以及应用是否带着
JavaScript 虚拟机。

| | Rust | TSX | Rust + TSX |
|---|---|---|---|
| 你的 UI、状态和事件处理 | Rust | TSX，运行在 QuickJS 上 | Rust 组件和模块，TSX 页面使用它们 |
| 应用运行的内容 | 只有引擎和你的 crate，所有平台和浏览器都一样 | QuickJS 和引擎（浏览器里是它的 JavaScript 引擎） | QuickJS 和引擎，加上你的 crate（浏览器里是它的 JavaScript 引擎） |
| 适合 | 业务复杂度高的中大型应用 | 运行时下发的应用和模块、快速迭代 | Rust 核心加上经常变化的部分 |
| 修改代码 | 重新构建并重启 | 热更新 | TSX 热更新，Rust 重新构建并重启 |
| 创建 | `nsh init my-app --template rust` | `nsh init my-app --template ts` | `nsh init my-app --template mixed` |

## 如何选择

**Rust** 适合业务逻辑庞大、要求高的应用：交易、编辑器、数据密集的工具。整个应用都有 Rust 的类型系统和编译期检查，可以使用
crates.io 生态，你自己的代码（不只是 NSH 的代码）也是原生性能，还能用 `cargo test` 对真实 UI 做无界面测试。应用从头到尾都是
Rust 和原生代码：在 iOS、Android、macOS、Windows、Linux、GPUI 和 Web 上，应用里既不带也不运行 JavaScript 虚拟机。
组件是只运行一次的函数，把信号绑定到显示的内容上，和 TSX 一样（[响应式](./reactivity.md)），样式用类型化的方法而不是 class 字符串。

**TSX** 适合应用发布之后才到达的界面：原生应用在运行时加载的模块，由模型生成或由其他团队编写，以[签名包](./signed-packages.md)下发，运行在宿主配置的沙箱里，开发时支持热更新。

**Rust + TSX** 保留 Rust 核心，在值得的地方加上 TSX：页面、设置、引导、实验性功能，任何需要热更新快速迭代、或作为[签名包](./signed-packages.md)
在运行时下发的部分。TSX 像导入其他 NSH 模块一样导入你的 Rust 组件和模块，两部分通过类型化的接口衔接。TSX 运行在 QuickJS
上，这时应用会带上 QuickJS。

## 完全用 Rust 编写的应用

```sh
nsh init my-app --template rust
cd my-app
nsh run                        # 本机的后端；--target ios、android、web、macos、linux 或 windows
nsh package --target web       # 或 ios、ios-simulator、android、macos、linux、windows
```

推荐用 [huacnlee/cargo](https://github.com/huacnlee/cargo) 构建，NSH 自己的开发也用它：一个可以直接替换官方 Cargo 的分支，每个依赖只为你所有的项目和 Git worktree 构建一次，构建目录自己控制在大小限制之内，不需要任何配置（[安装](./installation.md#用-rust-编写的应用)）。

`src/lib.rs` 包含整个应用：组件、状态、事件处理和任务。

```rust
use nsh_base::{button, theme};
use nsh_core::prelude::*;

/// TSX 通过 `app/native` 可以使用的组件。
#[native_module("app/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

#[component]
pub fn App() -> impl IntoElement {
    let count = signal(0_u32);
    let quote = signal(SharedString::from("…"));
    // 异步工作在任务里运行；请求走 NSH 的网络栈。
    spawn(async move {
        if let Ok(response) = fetch("https://api.example.com/quote").await {
            quote.set(response.text().into());
        }
    });
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_4()
        .child(text(move || format!("Count: {}", count.get())))
        .child(text(quote))
        .child(
            // nsh_base 的无样式按钮，用主题的颜色绘制。
            button()
                .px(px(20.0))
                .py(px(12.0))
                .rounded(move || theme().radius)
                .bg(move || theme().primary)
                .active(|s| s.bg(move || theme().primary.opacity(0.9)))
                .on_press(move || count.update(|c| *c += 1))
                .child(text("Tap").text_color(move || theme().primary_foreground)),
        )
}
```

元素和组件用同样的方式创建：一个函数创建它（`view()`、`text(..)`、`nsh_base` 的 `button()`、你的 `#[component] fn Chart`
生成的 `chart()`），它的方法设置 props（每个 prop 一个设置方法，接受值、signal 或闭包）和监听器（每个事件一个方法）。
监听器是接收事件的闭包（`move |text: &str| ..`），没有数据的事件写成 `move || ..`；`.child(..)`
接受一个元素，或元素的 `Option`、`Vec`、数组、迭代器（`.child(rows.iter().map(|r| row(r)))`）。

`nsh.json` 指定根组件，`nsh` 负责挂载它：

```json
{
  "name": "My App",
  "rust": ".",
  "root": "App",
  "theme": "theme.json",
  "appearance": "system"
}
```

`"root"` 是组件在 crate 中的路径（`App`、`shell::Gallery`；有[多个 crate](#多个-crate) 时在第一个中）；`"theme"` 是它的组件绘制时使用的[主题](#主题)，`"appearance"` 和 TSX 应用一样作用于它。
有了它，对应用目录执行 `nsh run` 和 `nsh package` 时，会为目标平台构建应用自己的运行器，直接挂载根组件，不链接 QuickJS：

| 目标 | 运行的内容 |
|---|---|
| macOS、Windows、Linux | 带有你的 crate 的发布运行器，在窗口中运行；`nsh run` 使用 debug 构建，并在终端显示应用日志 |
| iOS、iOS 模拟器、Mac Catalyst | 带有你的 crate 的 iOS 运行器 |
| Android | 带有你的 crate 的应用 `libnsh.so`，由运行时的 Activity 启动 |
| Web | 包含引擎和你的 crate 的 `nsh.wasm`（整个 Rust gallery 为 3.3 MB，brotli 压缩后 0.75 MB）；页面直接挂载根组件 |

`nsh` 只构建你的 crate 需要的东西：一个由你的 crate 和 NSH 核心组成的小程序，它向 CLI 描述 crate 的组件和模块，其余工作由
CLI 自己完成；再加上你运行或打包的每个目标的运行器。新 Rust 应用的第一次构建只编译你的 crate 和 NSH 的核心，大约 60 个
crate。`nsh test` 在同样构建的、包含你的 crate 的运行器中以无界面方式运行应用的测试。

在 Linux 上，由应用 crate 选择可选的平台功能。使用网页视图时，在 `nsh-platform-linux` 依赖上启用
`webview` feature；这会链接 WebKitGTK 6.0，构建和运行时都需要安装该系统库。生成的运行器通过 Cargo
的 feature 合并继承应用的选择。

`nsh run` 期间，修改 crate 的 Rust 代码会重新构建运行器并重启应用；在浏览器中，已打开的页面会自动加载新的运行时。
`cargo run` 在桌面后端上直接启动同一个根组件（`src/main.rs`）；嵌入 NSH 的原生应用在 macOS、Windows、Linux、GPUI 和
iOS 的 `AppView` 上用 `AppSource::Rust` 挂载 Rust 根组件。

要在包括 Web 在内的所有平台上运行的代码，使用 NSH 提供的服务：HTTP 用 `fetch`（[网络](./networking.md#在-rust-组件中)），
等待用 `timer`、`set_timeout` 和 `set_interval`，时钟用 `nsh_core::time::Instant` 和 `nsh_core::time::unix_ms()`。
`background` 和 `blocking` 在原生目标的线程上运行任务。`run_on_main_thread(f)` 在平台的主线程上运行 `f`，用于只能在主线程调用的 API
（AppKit 的状态栏项、UIKit）：它在本轮的视图挂载之后运行，结果是一个在引擎线程上 await 的 future。不是 `async` 的模块函数调用
`cx.run_on_main_thread(f)`。在 Web 上和 headless 测试中，`f` 在提交结束时于引擎线程上运行；在原生平台上则稍后运行，引擎同时继续工作：以 await 到结果为准判断 `f` 已运行。`f` 返回一个值，由 await 它的代码用这个值写 signal、调用 NSH 的函数。

## 桌面标题栏

Rust 应用通过 `nsh_core::element::desktop` 绘制自定义标题栏。用
`current_window().set(..)` 把窗口的 `title_bar` 属性设为 `TitleBarStyle::Hidden`，将背景设为
`.window_drag(true)`，并通过 `title_bar()` 为系统窗口按钮留出空间。
返回值的 `top` 是标题栏高度，`left` 和 `right` 是两侧系统按钮占用的宽度。
读取会订阅变化，包括进入和退出全屏。平台负责窗口拖动和用户设置的标题栏双击行为；
子元素上的 `.window_drag(false)` 可以排除父元素的拖动区域。

```rust
use nsh_core::{
    desktop::{TitleBarOptions, TitleBarStyle, WindowAttributes, WindowControl},
    element::desktop::{current_window, title_bar},
    prelude::*,
};

fn window_bar() -> impl IntoElement {
    let mut bar = TitleBarOptions::default();
    bar.style = TitleBarStyle::Hidden;
    bar.height = Some(px(42.0));
    let mut attributes = WindowAttributes::default();
    attributes.title_bar = Some(bar);
    current_window().set(attributes);
    view()
        .h(px(42.0))
        .flex_row()
        .items_center()
        .window_drag(true)
        .pl(move || title_bar().left + px(12.0))
        .pr(move || title_bar().right + px(12.0))
        .child(text("My app").flex_1())
        .child(text("Close").window_control(WindowControl::Close))
}
```

`TitleBarStyle::Default` 的标题栏恢复系统标题栏。带 `.window_control(..)` 的视图就是窗口的最小化、
最大化或关闭按钮：平台执行它的按下动作，关闭遵循窗口的 `close_on_press`。`WindowRef`
有窗口的其他请求（`set_mode`、`minimize`、`set_bounds`、`close`……），`window()` 打开应用自己的窗口，
这个模块还有菜单（`set_application_menu`、`popup_menu`）：见 [nsh/desktop](../modules/nsh-desktop.md)。

## 文件和文件夹

在桌面平台上，Rust 代码可以显示系统的文件对话框，并把本地路径交给系统处理，
每个结果都在任务中等待：`pick_paths` 接受 `OpenDialog`
（`OpenDialog::files()`、`::folders()` 或 `::files_and_folders()`），`pick_save_path`
接受 `SaveDialog`，`open_path`（用文件的默认应用打开）、`reveal_path`
（在访达、文件资源管理器或文件管理器中选中该项）和 `move_to_trash`
（macOS 告知时返回该项移到的位置）。用户取消时对话框返回 `Ok(None)`，
无法显示时返回错误（Linux 上没有文件选择 portal 时，以及 iOS、Android 和 Web 上；
在这些平台上 `open_path`、`reveal_path` 和 `move_to_trash` 也返回错误）。
请求失败时一律返回错误，不返回 `false`。

```rust
use nsh_core::{
    file_dialog::{FileFilter, OpenDialog},
    prelude::*,
};

fn open_notes() {
    let picked = pick_paths(
        OpenDialog::files()
            .multiple(true)
            .filter(FileFilter::new("Markdown", ["md", "markdown"]))
            .button_label("打开"),
    );
    spawn(async move {
        match picked.await {
            Ok(Some(paths)) => {
                for path in paths {
                    let _ = reveal_path(path).await;
                }
            }
            Ok(None) => {} // 用户取消。
            Err(error) => log::warn!("no file dialog: {error}"),
        }
    });
}
```

## 在网页上登录

`open_auth_session` 打开服务方的登录页（OAuth 授权地址），结果是登录页跳回的回调地址：iOS 用
`ASWebAuthenticationSession` 弹层显示（`.ephemeral(true)` 不与用户的 Safari 共用数据），Android 用 Custom Tab，
macOS、Windows 和 Linux 后端用系统浏览器。iOS 以外，回调以应用声明的 scheme（`nsh.json` 的 `schemes`）的链接回到应用，
由会话收下，`next_url_scheme_open` 收不到它。结果是 `Completed(url)` 或 `Cancelled`（用户关掉了页面，或没登录就回到应用）；
回调回不到应用时立即以 `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM` 失败：scheme 不是应用声明的会话如此；网页、GPUI
和终端后端收不到应用 scheme 的链接，那里的每个会话都如此。登录页的地址同 `open_url` 一样要 `openUrl` 能力。

登录页还开着、应用已从别处得知用户登录了（在不会跳回的页面上批准了设备码）时，`close_auth_session()` 关掉它：会话以
`Cancelled` 结束，iOS 和 Android 收起页面、回到应用；桌面浏览器里的页面不动。没有会话在等时什么也不做。

```rust
use nsh_core::{
    auth_session::{AuthSessionOutcome, AuthSessionRequest},
    prelude::*,
};

fn sign_in(authorize_url: String) {
    let session = open_auth_session(AuthSessionRequest::new(authorize_url, "myapp"));
    spawn(async move {
        match session.await {
            Ok(AuthSessionOutcome::Completed(callback)) => log::info!("signed in: {callback}"),
            Ok(_) => {} // 取消了。
            Err(error) => log::warn!("no sign-in page: {error}"),
        }
    });
}
```

测试里 `Headless::on_auth_session` 让平台有应用内会话、每个会话按给定的结果结束，`keep_auth_sessions_open` 则让
每个会话一直开着、直到应用关掉它（`screen().auth_session_showing()`）；都不设时登录页记在 `opened_urls()` 里，用
`open_with_url` 送回调（以网页身份运行的测试除外：会话立即失败）。

## 用 TSX 扩展 Rust 应用

crate 的 `#[native_module("app/native")]` 函数声明 TSX 可以导入的内容：组件（`registry.register::<Chart>()`）、函数模块
（`registry.module::<Portfolio>()`，在单元结构体及其 `impl` 上标注 `#[module]`）、类（`registry.class::<Ledger>()`，在结构体及其 `impl` 上标注
`#[class]`）和资源；这个属性还会在 crate 根部生成 `pub fn registry()`，给测试和其他 Rust 代码使用同一个集合
（`my_app_native::registry()`，与 `nsh_base::registry()` 并列）。在 `#[module]` 或 `#[class]` 的 `impl` 中，`pub fn`（包括 `pub(crate)`）会被导出；没有可见性修饰的函数是
辅助函数，只留在 Rust 中。它们的 TypeScript 声明（包括文档注释）由 Rust 类型生成到 `.nsh/types`，所以 `nsh check` 和编辑器会据此检查
TSX：

```rust
/// 价格图表。
#[derive(Clone, PartialEq, Props)]
pub struct ChartProps {
    /// 要绘制的代码。
    pub symbol: Prop<SharedString>,
    /// 显示的时间范围。
    #[prop(default = SharedString::from("1D"))]
    pub range: Prop<SharedString>,
}

#[derive(Event)]
pub enum ChartEvent {
    /// 选中了一个价格。
    #[args(price)]
    Select(f64),
}

/// 价格图表；按下时选中最新价格。
#[component]
pub fn Chart(props: ChartProps, cx: Scope<ChartEvent>) -> impl IntoElement {
    view()
        .h(px(160.0))
        .bg(move || theme().muted)
        .on_press(move || cx.emit(ChartEvent::Select(101.5)))
        .child(text(move || format!("{} · {}", props.symbol.get(), props.range.get())))
}

/// A ledger kept in Rust.
#[class]
pub struct Ledger {
    entries: Vec<f64>,
}

#[class]
impl Ledger {
    /// An empty ledger.
    pub fn new() -> Self {
        Ledger { entries: Vec::new() }
    }

    /// Adds `amount`; the new balance.
    pub fn add(&mut self, amount: f64) -> f64 {
        self.entries.push(amount);
        self.balance()
    }

    /// The balance.
    #[getter]
    pub fn balance(&self) -> f64 {
        self.entries.iter().sum()
    }

    /// Uploads the entries; resolves when the server has them.
    pub async fn upload(this: Instance<Ledger>, url: String) -> Result<(), String> {
        let body = this.with(|l| format!("{:?}", l.entries));
        fetch(HttpRequest::post(url, body)).await.map(|_| ())
    }
}

/// 应用的持仓。
#[module]
pub struct Portfolio;

#[module]
impl Portfolio {
    /// The value of the holdings.
    pub fn total() -> f64 {
        1250.0
    }
}
```

```tsx
import { mount, signal, View, Text } from "nsh";
import { Window } from "nsh/desktop";
import { Chart, Ledger, Portfolio } from "app/native";   // Rust

const ledger = new Ledger();   // 状态保存在 Rust 中

function Overview() {
  const balance = signal(ledger.add(Portfolio.total()));   // Rust，立即调用
  return (
    <View class="gap-2 p-4">
      <Text>Balance: {balance()}</Text>
      <Chart symbol="NSH" range="1M" onSelect={(e) => balance.set(ledger.add(e.price))} />
      <View onPress={() => ledger.upload("https://example.com/ledger")} role="button" class="rounded-lg bg-blue-600 px-4 py-2 active:bg-blue-700">
        <Text class="text-white">Upload</Text>
      </View>
    </View>
  );
}

mount(() => (
  <Window id="main">
    <Overview />
  </Window>
));
```

每个导出在 TypeScript 中的文档来自给它命名的那个 Rust 项的文档注释：组件来自 `#[component] fn`（`Chart`），模块和类来自它们的
结构体（`Portfolio`、`Ledger`），props、事件、函数和成员来自它们自己的文档注释（有 getter 的 setter 不需要）。`Props` 结构体和
事件枚举本身的文档注释，以及 `impl` 块上的文档注释，只留在 Rust 中；在 `#[module]` 或 `#[class]` 的 `impl` 块上写文档注释是编译
错误，因为 TypeScript 不会显示它。每个写入 `.nsh/types` 的命令都会对 TSX 看到的、没有文档注释的导出给出警告，并写出它的名字
（`Ledger.add`、`Chart.range`）。

TypeScript 只显示文档注释中第一个标题之前的部分。写给 Rust 调用者的说明，例如 `&self` 借用了什么、函数为什么接收 `cx`，放在标题
`# Rust`（或 rustdoc 的 `# Panics`、`# Examples`）下面，只留在 Rust 文档中：

```rust
/// 把 `text` 放到剪贴板；resolve 为复制的字符数。
///
/// # Rust
///
/// 接收 `cx` 让它成为一个 op，与 TS 在它之前做的 UI 改动按顺序执行。
pub fn copy(text: String, cx: &mut ModuleCx) -> u32 {
    let count = text.chars().count() as u32;
    cx.set_clipboard(text);
    count
}
```

props 是组件 `Props` 结构体的字段，事件是 `Event` 枚举的变体（`onSelect`），都用驼峰命名。Rust 代码用组件的构造函数放置同一个组件，每个 prop 一个设置方法，每个事件一个监听方法：`chart().symbol("NSH").range("1M").on_select(move |price: &f64| last.set(*price))`。Rust 组件自己的内容、事件和状态留在
Rust 中；TSX 传入 props、接收事件。类型不对的 prop（`u32` 类型的 prop 传 `level={1.5}`）和内置元素上一样给出警告：这个 prop 保持原值，组件的其余部分照常渲染。

函数和成员按 Rust 签名运行。不是 `async` 的在 TSX 调用中直接运行并返回值（`Portfolio.total()` 是数字，`ledger.balance` 是属性）：
它们计算并保存状态。`async fn` 返回 Promise，作为任务运行，可以使用 NSH 的服务（`fetch`、`timer`、对话框）；带
`cx: &mut ModuleCx` 参数的模块函数与 UI 按顺序作用于平台（toast、状态栏），还可以 `spawn` 一个任务，任务会运行到结束。组件 ref 的方法也与 UI 按顺序作用于平台；它们的返回类型决定 TSX
得到什么：返回 `()` 的得到 `void`，没有回复（TSX 调用后继续往下执行）；返回 `Result<(), E>` 的得到 `Promise<void>`，遇到 `Err`
时以其消息 reject（调用与 UI 的修改一起运行，那时 TSX 的调用已经返回，所以由 Promise 带回错误；没人 await 或 catch 的会记录为
[未处理的 rejection](./dev-server-and-debugging.md#日志)）；返回值的得到 `Promise<T>`。参数和返回值的类型来自签名：数字、
字符串、`Option`、`Vec`、你的 `#[derive(JsObject)]` 结构体和 `#[derive(PropEnum)]` 枚举、你的类的实例，以及 TSX 函数
（`Callback<A, R>`：Rust 在调用中或任务中立即调用它，或之后用 `post` 和 `call_async` 调用；`Callback<f64>` 在 TS 中是
`(value: number) => void`，`#[args(balance)] listener: Callback<f64>` 给参数命名，得到 `(balance: number) => void`），以及 Rust
原样保留的任意 TSX 对象（`JsHandle`，TS 中是 `object`：存进字段或映射，按身份比较，交还给 TSX 时仍是同一个对象），以及你的类本身
（`Constructor`，TS 中是 `Function`：`getRequiredService(I18nService)` 让 Rust 知道 TSX 指的是哪个类）。这些类型比 TS 应有的类型
宽松时，在成员或函数上写 `#[ts("<T>(token: { readonly prototype: T }): T")]`，按原文声明它的 TypeScript 签名（也可以是泛型的）；
编译器会检查它按顺序列出了 Rust 的参数。在类的结构体上写 `#[class(generic = "T")]`，类就声明为泛型
（`class ServiceToken<T>`），于是 `ServiceToken<I18nService>` 不能当作 `ServiceToken<Clock>`；Rust 自己的类型把它写作
`ServiceToken<any>`。签名共用的类型也属于 UI 集：`registry.ts_alias(TsAlias { name: "ServiceKey<T>", ts: "{ readonly
prototype: T } | ServiceToken<T>", doc: "…" })` 导出 `type ServiceKey<T>`。标了 `#[interface]` 的
trait 是 TSX 对象为 Rust 实现的 TypeScript 接口：UI 集声明 `export interface Startable`（每个 trait 方法一个方法，trait 有默认实现的
是可选成员，`#[getter]` 方法是 `readonly` 属性），Rust 通过 trait 的替身 `JsStartable` 调用这样的对象，它可以作为参数传入，也可以由
保留的对象构造（`JsStartable::new(handle)`）；用 `registry.interface::<JsStartable>()` 注册。不是 `async` 的方法在 TSX 的调用中立即
运行 TSX 的方法（在调用之外，返回 `()` 的方法在 TSX 的下一轮运行），`#[getter]` 和返回值的方法在那里立即运行，在任务或组件中也可以立即运行，
`Callback` 的 `call` 也是如此（`handle.get::<T>("name")` 以同样方式读取保留对象的任意属性），`async fn` 等待它的结果（普通值和 Promise 都可以：在它上面写 `#[ts("(): boolean")]`
声明为 `isInitialized(): boolean`），TSX 抛出的异常是返回 `Result` 的方法
的 `Err`。另一个 crate 可以给你的类添加方法：在它自己的某个类型的 `impl` 块上写 `#[class_extension(AppBuilder)]`，每个以
`this: Instance<AppBuilder>` 开头的函数就是 TSX 中 `AppBuilder` 的方法（`builder.addI18n((o) => …)`）；应用使用的某个 UI 集用
`registry.class_extension::<I18nBuilder>()` 注册它之后，每个实例上都有这些方法，类的声明也会列出它们。开始监听的函数命名为
`on(event, listener)`，返回 `Stop`（`Stop::new(|| …)`），TSX 得到的是停止监听的函数（`() => void`）；它只运行一次，在 TSX 调用它时，
或在调用 `on` 的一方（组件，或热重载下模块的这次运行）消失时。事件处理函数
接收一个对象：变体的值是它的字段 `value`，在变体上写 `#[args(price)]` 可以给它命名（`onSelect({ price })`），
有多个值的变体用 `#[args(open, reason)]` 给各个字段命名。类型不对的参数在调用中抛出 `TypeError`，
返回 Promise 的调用则以它 reject；`Err` 以其消息抛出（或 reject）。类的对象拥有它的
Rust 值：对象被回收时释放，或调用 `ledger.dispose()`（也可以 `using ledger = new Ledger()`）立即释放。指定 TSX 入口来运行它：`nsh run App.tsx` 或 `nsh package App.tsx --target …`。TSX 的修改是热更新，
Rust 的修改会重新构建并重启应用。CLI 根据 crate 的描述知道它导出了什么，自己打包和编译 TSX；开发宿主和 `nsh test` 的
无界面运行器则与你的 crate 和 QuickJS 一起构建。

仓库中的 `examples/rust-exports` 是导出的参考示例：一个小型 TSX 应用，它的 crate 导出一个组件（必填、有默认值和可选的
props，一个事件，子元素）、一个模块（每种函数各一个）和一个类（构造函数、方法、getter 和 setter、静态成员、`async` 成员、以另一个实例为参数、
Rust 用 `post` 调用的回调、`dispose()` 和 `using`），每个都有简短的文档注释；它的测试（Rust 的，以及 TypeScript 的 `App.test.tsx`）在应用中逐一点按，并检查每个导出显示的内容。

仓库中的 `examples/gallery-rs` 是一个完整的 Rust 应用：每个 demo 都是 Rust，`nsh run examples/gallery-rs --target web`
（或 `ios`、`android` 等）运行它的根组件，`nsh test examples/gallery-rs` 以无界面方式运行它的测试，`cargo run -p gallery-native`
在桌面后端上启动它。

### 渲染函数与应用的类

组件用渲染函数为它拥有的值（它的一行数据、文档中的一个代码块）绘制应用的内容：一个 `Option<Renderer<T>>` 属性，在 TypeScript 中是 `(value: T) => JSX.Element`。组件在内容所在的位置调用 `render(key, value)`，得到一个视图，像其他视图一样设定样式并放置（在 `text(..)` 中是一个行内视图）。只要值不变，内容按 key 保留；值改变时重新渲染。Rust 父组件把闭包传给该属性的设置方法（`quote().render_badge(|symbol| ..)`），由它生成 `Renderer`。

组件中的样式是类型化的；应用的 Tailwind 类通过一个类名属性和该部件元素上的 `.app_class(..)` 作用于组件的某个部件。它们在元素自身的样式之后应用，并使用应用的主题、`dark:` 和各种状态，就像元素上的 `class` 一样。

```rust
/// A quote: its symbol, then content of the app's.
#[derive(Clone, Copy, PartialEq, Props)]
pub struct QuoteProps {
    /// The symbol.
    pub symbol: Prop<SharedString>,
    /// Tailwind classes of the symbol's text.
    pub symbol_class: Prop<Option<SharedString>>,
    /// Content of the app's after the symbol (a change, a badge).
    pub render_badge: Prop<Option<Renderer<SharedString>>>,
}

/// A quote of a symbol.
#[component]
pub fn Quote(props: QuoteProps) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(text(props.symbol).app_class(props.symbol_class))
        .child(dynamic(move || {
            props.render_badge.get().map(|r| r.render("badge", props.symbol.get()).flex_1())
        }))
}
```

```tsx
<Quote
  symbol="NSH"
  symbolClass="text-lg font-semibold"
  renderBadge={(symbol) => <Text class="text-green-600">{`${symbol} ▲ 2.4%`}</Text>}
/>
```

`nsh/base` 中的 `TextView` 就是这样构建的：它的 `renderCodeBlock`、`renderMath` 等是渲染函数，它的 `classes` 是应用按部件给出的类（[TextView](../elements/text-view.md)）。

## 测试

Rust 应用的测试以无界面方式运行它，可以在 `cargo test` 中，也可以在 `nsh test` 中，使用同一个驱动：运行在真实的引擎上，没有窗口，以
60 Hz 提供显示帧，所以定时器、任务和动画都像在屏幕上一样运行。驱动启动应用，维护一份它所显示视图的模型，并在用选择器指定的视图上执行步骤（`press`、`type_text`、`submit`、`scroll`、`wait_for`），选择器可以是文本、无障碍标签或
test id（`text:`、`label:`、`id:`）；步骤无法执行时，会带着屏幕上的文字失败。

在 `cargo test` 中，`nsh_testing::Headless`（nsh-testing 的默认特性，不含 QuickJS）运行根组件（`app.screen()`：屏幕上的文字、应用发送的命令）；`EngineConfig::new`
是一个手机尺寸、其余都未设置的引擎。`nsh test` 在用你的 crate 构建的无界面运行器中运行 TypeScript 测试（[测试](./testing-and-trace.md)），其中
`open(".")` 启动 Rust 应用的根组件：

```toml
[dev-dependencies]
nsh-engine = "0.1"
nsh-testing = "0.1"
```

```rust tabs
// tests/app.rs：cargo test
use nsh_engine::{EngineConfig, Root};
use nsh_testing::Headless;

#[test]
fn a_tap_counts() {
    let mut app = Headless::start(EngineConfig::new(Root::of::<my_app::App>())).unwrap();
    app.wait_for("Count: 0").unwrap();
    app.press("Tap").unwrap();
    app.wait_for("Count: 1").unwrap();
}
```

```tsx tabs
// App.test.tsx，与 nsh.json 放在一起：nsh test
import { expect, open, test } from "nsh/test";

test("a tap counts", async () => {
  const app = await open(".");
  await app.press("Tap");
  expect(app.texts()).toContain("Count: 1");
});
```

文字默认按估算宽度排版（每个字符为字号的 `0.55 ×`）；测试用 `Headless::start_with_text_engine` 启动应用时，传入一个在引擎线程上
创建平台文字引擎的函数（`nsh_platform_macos::CoreTextEngine`、`nsh_platform_windows::DWriteTextEngine`），文字就按屏幕上的宽度量。
`app.screen().layout_nodes()` 按树的顺序返回屏幕上的视图及布局给出的位置（`LayoutNode`：种类、test id、父视图、
根视图坐标下未计变换的边界、是否隐藏、是否裁剪，`scroll` 里是滚动视图的方向和内容尺寸，`text` 里是文字视图的文字及其排版所用的框），
测试据此检查有没有视图超出父视图、被裁剪的视图截掉，或文字放不下。两者都是 `nsh_testing` 给 `cargo test` 用的 Rust API。

Web 应用的路由在它的页面上测：`Headless::start_at_page(config, "https://example.test/app/")` 以这个地址在 Web 上启动应用，
`app.browser()` 是它所在的浏览器，带着 Web 记下的历史（栈每推一页、每开一个对话框各一条，应用以
`set_app_url_with(url, HistoryBehavior::Push)` 设的地址各一条）和用户在其中的移动：`back()`、`forward()`、`go(delta)` 与
`type_address("#inbox")`，应用像在 Web 上一样收到它们（`next_app_url_change`、`next_url_scheme_open`）；`entries()`、`index()`
和 `url()` 读历史。

```rust
let mut app = Headless::start_at_page(config, "https://example.test/app/").unwrap();
app.press("Message").unwrap(); // 推一页并设 `?m=1`
assert_eq!(app.browser().url(), Some("https://example.test/app/?m=1"));
app.browser().back().unwrap(); // 弹掉这一页
app.browser().type_address("#inbox").unwrap(); // 原因为 `Address` 的变化
```

TSX 入口用 `nsh-testing` 的特性 `bundle` 按 `nsh run` 的方式构建，使用它的宿主提供的 UI 集合：NSH 的，以及每个 crate 由
`#[native_module]` 生成的 `registry()`：

```toml
[dev-dependencies]
nsh-testing = { version = "0.1", features = ["bundle"] }
nsh-base = "0.1"
```

```rust
#[test]
fn the_module_formats_money() {
    let sets = vec![nsh_base::registry(), my_app_native::registry()];
    let mut app = nsh_testing::tsx("../App.tsx", sets).unwrap();
    app.wait_for("Formatted at once: $1,234.50").unwrap();
}
```

`nsh_testing::compile_tsx` 只构建一次入口供多个测试使用，`nsh_testing::start` 把批次交给自己维护屏幕模型的测试。
`examples/rust-exports` 用这种方式测试它的 TSX 入口；`examples/gallery-rs` 用 TypeScript 测试它的 Rust 根组件，
大致每个 demo 一个测试文件。

## 主题

应用的外观由应用自己决定：NSH 提供基础元素和 `nsh_base` 中无样式的基础组件，你的 crate 是其上带样式的一层，即应用自己的组件库（[三层结构](./architecture.md#three-layers)）。Rust 组件的颜色、圆角和字体取自主题，即
`nsh_base` 的 `theme()`，所以一份主题文档就能改变应用中所有组件的样式。`.bg(move || theme().primary)` 这样的绑定会原生地跟随主题的每次变化，
不需要重新渲染。

主题采用 [shadcn/ui](https://ui.shadcn.com/docs/theming) 的设计令牌，默认主题是 shadcn 的 `neutral` 主题：黑白配色，圆角 10 点。
要使用自己的主题，在 `nsh.json` 中把 `"theme"` 设为主题文档的路径（相对于 `nsh.json`），或者直接写入文档内容；它整体替换默认主题。
如果只想修改主题的几个令牌、保留其余的，改为用补丁覆盖它们（[见下文](#在应用运行时修改令牌)）：

```json
{
  "name": "My App",
  "rust": ".",
  "theme": "brand-theme.json"
}
```

主题文档是 JSON：每个令牌带有它的 `kind` 和 `value`，修饰器的上下文会改变令牌的值（浅色和深色外观也是其中之一），角色则供 NSH 自身的部件读取：

```json
{
  "tokens": {
    "violet-9": { "kind": "color", "value": "#7c3aed" },
    "violet-4": { "kind": "color", "value": "#a78bfa" },
    "primary": { "kind": "color", "value": "{violet-9}" },
    "primary-foreground": { "kind": "color", "value": "#f5f3ff" },
    "ring": { "kind": "color", "value": "{primary}" },
    "up": { "kind": "color", "value": "#16a34a" },
    "radius": { "kind": "number", "value": 8 },
    "font-sans": { "kind": "font", "value": "Inter, sans-serif" }
  },
  "modifiers": {
    "colorMode": {
      "default": "light",
      "contexts": {
        "light": {},
        "dark": { "primary": "{violet-4}", "primary-foreground": "#2e1065", "up": "#4ade80" }
      }
    }
  },
  "resolutionOrder": ["colorMode"]
}
```

- 令牌的 `kind` 是它保存的内容，与名称无关：`color`（`#rgb`、`#rrggbb`、`#rrggbbaa`）、`number`（点，或无单位的数）、`font`（字体族列表：
  使用第一个非通用的字体族，第一个是通用字体族 `system-ui` 或 `sans-serif` 时使用系统字体）、`string`、`boolean`、`duration`（`"150ms"`）、
  `easing`（`[0.2, 0, 0, 1]` 或 `ease-out`），以及复合类型 `typography`、`shadow`、`gradient`、`blur`、`border` 和 `transition`。值可以是
  另一个令牌，即 `"{violet-9}"`：引用在其他一切之后解析，所以修改 `violet-9` 会传到 `primary` 和 `ring`。
- **浅色和深色。** 一个令牌只有一个值。主题的深色值是其浅色/深色修饰器（上例的 `colorMode`）的 `dark` 上下文，
  `set_modifier("colorMode", "dark")` 显示它们（见下文）。外观（`nsh.json` 的 `"appearance"`、系统的外观）决定 `dark:` 类名、`dark()`、
  `color(..)` 以及按外观给出的颜色（`AdaptiveColor::rgb(0xffffff, 0x0d1117)`），从不决定令牌的值。
- **其他模式。** 品牌、密度或对比度也是修饰器（[修饰器](#主题文档与类型化键)），`"resolutionOrder"` 按其上下文应用的顺序列出修饰器，靠后的优先。
- **渐变、阴影和文字样式。** `gradient` 令牌是一个 `Gradient`（`tokens().gradient("hero")`），`shadow` 令牌是它的各层
  （`tokens().shadow("shadow-raised")`），`typography` 令牌是一个 `Typography`，
  `.typography(move || tokens().typography("text-title").cloned().unwrap_or_default())` 把它整体设置到文字上（字号、行高、字间距、字重和字体族）。
  `.adaptive_gradient(..)` 和 `.adaptive_shadow(..)` 按外观各取一个：用两个令牌的层构造 `AdaptiveShadow::new(light, dark)`。
- **应用自己的令牌。** 其他任何名称都是应用的令牌，就像 Tailwind `@theme` 中的变量：上例的 `"up"` 在 Rust 中是 `token("up")`
  或类型化键的 `get()`（[见下文](#主题文档与类型化键)），在类名中是 `bg-up` / `text-up/70`，在 TSX 中是 `token("up")`
  （[主题 token](./tailwind.md#主题-token)）。数值如 `"radius-card"` 是 `TokenKey::<Points>::new("radius-card").get()` 和
  `rounded-(--radius-card)`。它们在读取处跟随各自的令牌；`theme().color(name)` 读取主题生成时的令牌，不跟随没有角色读取的令牌。
- 文档从不会让应用停下：它无法使用的内容（与种类不符的值、指向不存在令牌的引用）会被忽略并记录日志（target `nsh::theme`），其余照常生效。
  运行时拒绝旧格式的 shadcn 主题文件：`Root::theme` 和 `set_theme` 报告 `legacy-format`，不启用任何内容，主题保持不变。
  [`nsh theme convert`](./cli.md#nsh-theme) 可把这样的文件写成主题文档；`nsh.json` 的 `"theme"` 是旧格式时，`nsh build`、`nsh dev` 和
  `nsh run` 会给出警告（`legacy-format`）并忽略它。
- `nsh run` 和 `nsh package` 把主题交给应用的根组件，即 `Root::theme(json)`，所以它的读取方式与 TSX 应用相同，并且在第一帧就生效；
  `nsh init` 生成的 `cargo run` 可执行文件也会这样使用 `nsh.json` 的 `"theme"`。自己创建的根组件（例如测试中的
  `EngineConfig::new(Root::of::<App>())`）可以用 `.theme(include_str!("../brand-theme.json"))` 指定主题，用 `.theme_overrides(..)` 指定覆盖。

令牌及其默认的浅色和深色值（shadcn 的 `neutral`；深色值是 `colorMode` 的 `dark`）：

| 令牌 | `theme()` | 浅色 | 深色 | 用途 |
|---|---|---|---|---|
| `background` / `foreground` | `background` / `foreground` | `oklch(1 0 0)` / `oklch(0.145 0 0)` | `oklch(0.145 0 0)` / `oklch(0.985 0 0)` | 应用的背景，以及其上的文字 |
| `card` / `card-foreground` | `card` / `card_foreground` | `oklch(1 0 0)` / `oklch(0.145 0 0)` | `oklch(0.205 0 0)` / `oklch(0.985 0 0)` | 卡片等分组表面，以及其上的文字 |
| `popover` / `popover-foreground` | `popover` / `popover_foreground` | `oklch(1 0 0)` / `oklch(0.145 0 0)` | `oklch(0.205 0 0)` / `oklch(0.985 0 0)` | 弹出框、菜单和提示，以及其上的文字 |
| `primary` / `primary-foreground` | `primary` / `primary_foreground` | `oklch(0.205 0 0)` / `oklch(0.985 0 0)` | `oklch(0.922 0 0)` / `oklch(0.205 0 0)` | 主要操作（主按钮），以及其上的文字 |
| `secondary` / `secondary-foreground` | `secondary` / `secondary_foreground` | `oklch(0.97 0 0)` / `oklch(0.205 0 0)` | `oklch(0.269 0 0)` / `oklch(0.985 0 0)` | 次要操作，以及其上的文字 |
| `muted` / `muted-foreground` | `muted` / `muted_foreground` | `oklch(0.97 0 0)` / `oklch(0.556 0 0)` | `oklch(0.269 0 0)` / `oklch(0.708 0 0)` | 低调的表面；次要文字：说明、标签、占位符 |
| `accent` / `accent-foreground` | `accent` / `accent_foreground` | `oklch(0.97 0 0)` / `oklch(0.205 0 0)` | `oklch(0.269 0 0)` / `oklch(0.985 0 0)` | 悬停或高亮的项，例如菜单项、列表行，以及其文字 |
| `destructive` | `destructive` | `oklch(0.577 0.245 27.325)` | `oklch(0.704 0.191 22.216)` | 破坏性操作和错误 |
| `border` | `border` | `oklch(0.922 0 0)` | `oklch(1 0 0 / 10%)` | 分隔线和边框 |
| `input` | `input` | `oklch(0.922 0 0)` | `oklch(1 0 0 / 15%)` | 输入框的边框 |
| `ring` | `ring` | `oklch(0.708 0 0)` | `oklch(0.556 0 0)` | 焦点环 |
| `chart-1` … `chart-5` | `chart_1` … `chart_5` | `oklch(0.87 0 0)` … `oklch(0.269 0 0)` | 同浅色 | 图表的各个系列 |
| `sidebar`、`sidebar-foreground`、`sidebar-primary`、`sidebar-primary-foreground`、`sidebar-accent`、`sidebar-accent-foreground`、`sidebar-border`、`sidebar-ring` | `sidebar`、`sidebar_foreground` 等 | `sidebar` `oklch(0.985 0 0)`，`sidebar-primary` `oklch(0.205 0 0)` | `sidebar` `oklch(0.205 0 0)`，`sidebar-primary` `oklch(0.488 0.243 264.376)` | 侧边栏的表面、主要操作、高亮项、边框和焦点环 |
| `radius` | `radius` | `0.625rem`（10 点） | | 圆角 |
| `font-sans` | `font_sans` | 系统字体 | | 应用的字体：没有设置自己字体族的文字都使用它 |
| `font-mono` | `font_mono` | 系统等宽字体 | | 等宽字体：`font-mono`、`TextView` 中的代码 |

### 在应用运行时修改令牌

应用的覆盖是主题之上的一个补丁：先是其根组件 `Root::theme_overrides` 的条目，再是每次 `update_tokens(patch)`。要修改几个令牌并保留其余的，只传入
这些令牌；`reset_tokens` 把它列出的名称从补丁中移除，恢复为主题的值；`set_theme` 让另一份文档成为应用的主题：

```rust
use nsh_core::{element::set_modifier, prelude::*};

update_tokens(Patch::new().value("primary", Hsla::from(rgb(0xe11d48)))); // 只有 `primary` 改变
reset_tokens(["primary"]); // 恢复主题的 primary
set_modifier("colorMode", "dark"); // 主题的深色值
set_theme(Some(include_str!("../high-contrast.json"))); // 另一份主题文档
set_theme(None); // 恢复 `nsh.json` 的
```

- 修改与应用的其他操作按顺序生效，一轮中所有主题修改只解析一次。读取被修改令牌的内容（`theme()`、类名、TSX 中的 `token()`）在同一帧跟随变化，
  平台自己的控件也一样。
- 更新会替换补丁中同一令牌的条目，所以对一个令牌更新一千次也只有一个条目。覆盖优先于修饰器的上下文：把 `radius-card` 更新为 12，
  再选择紧凑密度，它仍是 12。主题没有声明的名称、或与令牌种类不同的值会记录日志并被忽略（在声明了它的主题下会再次生效）。
  `Patch::new().role("popover", "surface")` 映射一个角色。
- `reset_tokens(names)` 把每个名称从补丁中移除，令牌的覆盖和角色的映射都一样，于是主题的值（或所选上下文的值）恢复，并在切换修饰器后保持；
  `reset_all_tokens()` 清空补丁，包括 `Root::theme_overrides` 的条目。
- `set_theme(Some(json))` 返回它无法使用的文档内容。不是主题文档的文本会被拒绝，主题保持不变。另一个主题保留它具有的已选修饰器上下文，以及覆盖。
- 同步的 `#[module]` 函数可以用其 `ModuleCx` 的 `cx.update_tokens(..)`、`cx.reset_tokens(..)`、`cx.reset_all_tokens()`、
  `cx.set_modifier(..)` 和 `cx.set_theme(..)` 做同样的事。TSX：[`updateTheme` 和 `resetTheme`](../modules/nsh.md#updatetheme-resettheme)。
- 在任何来自 JS 的同步调用中（`#[class]` 成员、类扩展的方法、模块函数），`set_modifier`、`set_color_scheme`、`update_tokens`、
  `reset_tokens` 和 `reset_all_tokens` 随该调用所在轮次的操作一起发送，排在 JS 在调用之前发送的内容之后，与 TSX 的 `setModifier` 和
  `updateTheme` 一样：在 `mount` 之前选择应用主题上下文的构建器方法会出现在第一帧中。`set_theme` 则通过模块函数的 `cx.set_theme(..)`
  或 TSX 的 `setTheme`。
- 无论 Rust 在哪里调用 `set_color_scheme`，TSX 的 `colorScheme()` 都会跟随它，与跟随 `setColorScheme` 一样：在来自 JS 的同步调用返回时，
  以及在任务或组件设置它之后的下一个轮次。`Appearance::System` 会重新显示系统的配色方案。

### 主题文档与类型化键

有自己令牌结构的设计系统（按层级划分的表面、操作色、卡片圆角）以**主题文档**提供它的令牌：令牌还说明它在设计师的树中的位置和说明文字，
角色则把 NSH 自身的部件映射到这些令牌上。主题文档代替 nsh-base 的 shadcn 主题成为应用的主题；没有主题文档的应用继续使用上面的 shadcn 主题。

```json
{
  "tokens": {
    "surface-raised": { "kind": "color", "value": "#ffffff", "path": "surface.raised", "doc": "Cards and sheets." },
    "surface-overlay": { "kind": "color", "value": "#ffffff", "path": "surface.overlay", "doc": "Popovers and menus." },
    "action": { "kind": "color", "value": "#4f46e5", "doc": "The main action." },
    "radius-card": { "kind": "number", "value": 14, "path": "radius.card", "doc": "Cards' corners." }
  },
  "modifiers": {
    "colorMode": {
      "default": "light",
      "contexts": {
        "light": {},
        "dark": { "surface-raised": "#18181b", "surface-overlay": "#27272a", "action": "#818cf8" }
      }
    }
  },
  "resolutionOrder": ["colorMode"],
  "roles": { "popover": "surface-overlay", "primary": "action" }
}
```

- 令牌的名称就是类名、`{…}` 引用和 `token()` 使用的名称，它不说明令牌的种类：名为 `radius-card` 的数值像其他数值一样由 `rounded-card` 和
  `rounded-(--radius-card)` 读取。
- `path` 是令牌在设计师的树中的位置（`surface.raised`），`doc` 是它的说明，`.nsh/types/nsh/tokens.d.ts` 会在 `token()` 上显示它。
  一个路径只对应一个令牌：同一路径上的第二个令牌会被忽略并报告。
- 主题文档有这些部分：`name`、`tokens`、`modifiers`、`resolutionOrder` 和 `roles`（以及供编辑器使用的 `$schema`）。其他部分会被报告并忽略。
- `"roles"` 把 NSH 自身部件绘制所用的角色映射到文档的令牌上（见下文）。

UI 集在它的 `#[native_module]` 函数中注册文档，`theme_keys!` 为文档声明的每个令牌生成类型化键，按令牌的路径放在模块中：

```rust
use nsh_core::prelude::*;

#[native_module("ds/native")]
fn native(registry: &mut Registry) {
    registry.theme_document(include_str!("../theme.json"));
}

pub mod keys {
    nsh_core::element::theme_keys!("theme.json");
}

view()
    .bg(move || keys::surface::RAISED.get().unwrap_or_default())
    .rounded(move || px(keys::radius::CARD.get().unwrap_or_default()))
    .p_4()

update_tokens(Patch::new().key(keys::ACTION, rgb(0xe11d48).into()));
reset_tokens([keys::ACTION.name()]); // 恢复主题的 action
```

- 键是 `TokenKey<V>`：令牌的名称加上其值的类型。颜色是 `Hsla`；数值是 `f32`，即点数或无单位的数（`px(..)` 把它变成长度，如上例）；字体是 `Arc<str>`；字符串是 `String`；
  布尔值是 `bool`；时长是 `std::time::Duration`；缓动是 `CubicBezier`；文字样式是 `Typography`（`font_family`、`size`、`line_height`、
  `letter_spacing`、`font_weight`）；阴影是 `Arc<[BoxShadow]>`；渐变是 `Gradient`；模糊是 `Blur`，边框是 `Border`，过渡是 `TransitionToken`。
  键的说明就是令牌的说明。
- `key.get()` 读取使用处的令牌：最近的 [`ThemeScope`](../modules/nsh.md#themescope) 的令牌。它是响应式的，在作用域之外只在该令牌变化时重新运行。
  `tokens().key(key)` 从手上的令牌中读取，`tokens().get_as::<V>(name)` 按名称读取令牌，出错时说明令牌是缺失还是种类不同。
- `Patch::new().key(key, value)` 以类型化的方式写入令牌：把补丁交给 `update_tokens`。
- `theme_keys!` 在 crate 构建时读取文档（相对于它的 manifest），文档变化时重新构建 crate。路径的各段变成蛇形命名的模块和大写蛇形命名的键
  （`type-scale.gutter`：`type_scale::GUTTER`）；没有路径的令牌以其名称命名（`ACTION`）。
- `tokens()` 说明当前文档声明的内容：每个名称的种类（`kind_of`）、路径（`path_of`）和说明（`doc_of`），某路径上的令牌（`name_at`），以及所有名称（`names()`）。
- 不知道应用中名称的代码（例如 `nsh_base` 的组件）改为读取角色。

**角色。** NSH 自身的部件用角色绘制，角色是绘制它的 crate 的常量（`nsh_base::role`：`POPOVER`、`PRIMARY`、`BORDER`、`RADIUS`、
`FONT_SANS` 等；[主题角色](../modules/nsh-base.md#theme-roles)）。角色是一个名称，它的类型说明它读取哪种令牌：`Role` 读取颜色
（`Hsla`），`NumberRole` 读取数值，`FontRole` 读取字体，`Role<Typography>`、`Role<Arc<[BoxShadow]>>` 等读取其他种类。文档的
`"roles"` 把角色映射到它的令牌上，令牌的种类须是角色读取的种类，因此 `nsh_base` 的 `theme()` 字段和组件、标签栏、平台的菜单和警告框
以及默认字体都用文档的令牌绘制：上例中 `theme().popover` 是设计系统的 `surface-overlay`，`theme().primary` 是它的 `action`。
没有映射时，角色读取与它同名的令牌。一个令牌可以担任多个角色（`"card": "surface", "popover": "surface"`）：卡片和弹出层共用一个表面的
设计系统把两者都映射到它，两者都跟随它的每次变化。一个角色只由一个令牌担任。

- 角色只在当前生效的文档中解析，`nsh_base` 的角色与任何库的角色一样：文档既没有映射 nsh-base 的某个角色、也没有与它同名的令牌时，
  该角色什么也读不到，`theme()` 把这种颜色画成透明，`radius` 为 0。nsh-base 的默认文档只在它是当前文档时才是主题。
- 映射到主题中不存在的令牌，或种类与角色读取的种类不同的令牌，会被报告并忽略，角色读取没有这条映射时会读取的值（补丁的映射回退到文档的映射）。
- `role(role::POPOVER)` 在使用处按类型读取角色（颜色角色为 `Option<Hsla>`），在 [`ThemeScope`](../modules/nsh.md#themescope)
  之外只在它的令牌或映射变化时重新运行；`tokens().role(..)` 从手头的令牌中读取。`theme()` 以同样方式读取它的字段：读取它的代码在它的
  某个角色变化时重新运行，没有角色读取的令牌变化时不会。

有自己组件的 crate 声明它的角色，并在注册表中列出（不论是否被主题映射），以便工具链和主题检查知道它们：`registry.roles(role::all())`，其中

```rust
pub mod role {
    use nsh_core::element::{NumberRole, Role};

    nsh_core::roles! {
        /// A banner's surface.
        pub BANNER: Role = "banner" required,
        /// How round a chip is.
        pub CHIP_RADIUS: NumberRole = "chip-radius",
    }
}
```

生成这些常量以及旁边的 `all()`，每个角色带有它的文档和种类。`required` 标记每个主题都应映射的角色：未映射它的主题在加载时收到警告。
两个 UI 集以不同种类列出同一个角色时保留第一个，并给出警告。`nsh/base` 列出它自己的角色：48 个，都不是必需的。

**一次一份文档。** 应用的文档（`nsh.json` 的 `"theme"`、`Root::theme` 或 `set_theme` 设置的）就是主题；没有时，是第一个注册了文档的 UI 集的文档；
都没有时，是 nsh-base 的默认主题。文档从不合并，未生效文档的令牌在任何地方都读不到：在设计系统的令牌之外还有自己令牌（博彩应用的 `odds-up`）
的应用，提供一份同时包含两者的文档。写了它做不到的内容的文档（不存在的种类、种类不同的值、映射到与角色种类不同的令牌上的角色）
会加载其中正确的条目，并报告其余的。

**应用的覆盖。** 补丁，即应用根组件上的 `Root::theme_overrides(json)`，按名称修改主题的令牌，
它的 `"roles"` 把角色映射到其他令牌上：

```json
{
  "values": { "surface-overlay": "#fafafa", "action": "#e11d48", "radius-card": 12 },
  "roles": { "warning": "status-caution" }
}
```

主题没有声明的名称、或与令牌种类不同的值是警告，并被忽略。覆盖优先于每个修饰器的上下文，嵌入应用的覆盖优先于应用的覆盖。

**修饰器。** 主题的模式（浅色和深色值、品牌、密度、对比度）是它的 `"modifiers"`：每个都有 `default` 和 `contexts`，每个上下文是它改变的令牌值，
`"resolutionOrder"` 是它们应用的顺序（[`setModifier`](../modules/nsh.md#setmodifier-modifier-modifiers)）。令牌是文档的值，其上是每个修饰器所选的上下文，
再上是覆盖，最后解析引用。`set_modifier("brand", "forest")` 与应用的其他操作按顺序选择一个上下文（从应用的第一轮开始，所以在构建根组件时
做出的选择在第一帧就生效）；主题没有的修饰器或上下文会记录日志，什么都不改变。`modifier("brand")` 是当前选中的上下文（响应式），
`modifiers()` 列出各修饰器及其上下文和默认值，与 `tokens().modifiers()` 相同。`theme_keys!` 在 `keys::modifiers` 中为每个修饰器的上下文
生成一个枚举，由编译器检查名称：

```rust
use keys::modifiers::Brand;

Brand::Forest.set(); // set_modifier("brand", "forest")
let forest = Brand::current() == Some(Brand::Forest); // 响应式

// 应用的一部分使用 forest 品牌和高对比度，不论应用选择了什么。
theme_scope()
    .modifiers(Some(ScopeModifiers::from(Brand::Forest).and(HighContrast::On)))
    .child(preview())
```

应用的选择属于应用自己：另一个主题（`set_theme`、主题文件的热重载）保留应用选择的每个上下文（只要新主题还有它），其余恢复为默认值。
`ThemeScope` 的 `appearance` 只改变它的配色方案；它的 `modifiers` 同时选择 `colorMode` 的 `dark` 时，才显示主题的深色值。

**工具链。** `nsh build` 和 `nsh dev` 从应用的 crate 和主题中获知文档：类名使用它们的令牌（`bg-surface-raised`、`rounded-card`），
`.nsh/types/nsh/tokens.d.ts` 按种类为 `token()` 提供类型和说明（[主题 token](./tailwind.md#主题-token)）。

## 多个 crate

应用的 Rust 代码可以拆分成多个 crate，每个 crate 用自己的名称导入：`"rust"` 可以是 crate 路径的列表，列表中的每个 crate
都有自己的 `#[native_module]` 函数。

```json
{
  "rust": ["native/charts", "native/storage"],
  "root": "App"
}
```

```tsx
import { Chart } from "app/charts";        // native/charts：#[native_module("app/charts")]
import { Store } from "app/storage";       // native/storage：#[native_module("app/storage")]
```

每个运行器和宿主都链接全部这些 crate，`.nsh/types` 为每个名称生成声明。crate 之间通过 Cargo 路径依赖共享代码
（`shared = { path = "../shared" }`）：一个公司组件 crate 可以被多个 crate 使用、被多个应用列出，只有 `"rust"` 中列出的
crate 会导出给 TSX。列出的每个 crate 都有自己的包名和导入名称；重复时 `nsh` 会报告共用它的两个路径。Rust 应用的
`"root"` 是列出的第一个 crate 中的组件。`nsh run` 和 `nsh dev` 期间，修改其中任何一个 crate，或它们通过路径依赖的
crate，都会重新构建并重启应用。

### 组件库

被多个应用使用的组件库可以拥有自己的模块，这样每个应用（以及组件库自己的预览和示例）都用同一个名称导入它的组件。
组件库的 crate 在一个普通函数中创建它的组件集，每个应用的 crate 在自己的 `#[native_module]` 函数中提供它；应用的
`"rust"` 只列出自己的 crate：

```rust
// 组件库的 crate（acme-ui）。
pub fn set() -> Registry {
    let mut registry = Registry::ui_set("acme/ui");
    registry.register::<Chip>();
    registry
}

// 应用的 crate。
#[native_module("app/native")]
fn native(registry: &mut Registry) {
    registry.serve(acme_ui::set());
}
```

```tsx
import { Chip } from "acme/ui";
```

同一应用的两个 crate 可以提供同一个组件库：它只会被提供一次。一个组件只属于一个组件集：两个组件集注册同一个组件时，
`nsh` 会在构建任何内容之前报告，并指出这两个组件集。

## 在 Web 上使用浏览器 API

在 Web 上，crate 可以通过 wasm-bindgen 和 `web-sys` 调用浏览器的 API（WebSocket、`IndexedDB` 等）；`nsh`
会随运行时一起构建它们所需的胶水代码。见 [Web](./web.md#在-rust-中使用浏览器-api)。

## 另请参阅

- [项目结构](./project-structure.md)：`nsh.json` 的每个键。
- [Web](./web.md#带-rust-组件的应用)：浏览器中的 Rust 应用。
- [网络](./networking.md#在-rust-组件中)：在 Rust 中使用 `fetch`。
- [CLI](./cli.md#nsh-init)：`nsh init --template rust` 和 `--template mixed`。
