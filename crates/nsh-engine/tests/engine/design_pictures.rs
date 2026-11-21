//! A design tool's pictures of the app's windows (`nsh/design`'s
//! `settled` and `picture`, `docs/dev/design-mode.md`): what the core does
//! for them, with a platform part that stands in for a real one as each
//! platform's `design.rs` does: it settles on the batches the app applied
//! and the animations of its graph (an [`Animator`] fed from the batches,
//! as the desktop backends feed theirs), and "draws" by writing the file
//! and noting what the animations show at the picture.
//!
//! - `settled` resolves once the app has been quiet, after an animation
//!   that ends by itself has ended, at once beside one that loops forever,
//!   and with `timedOut` past its deadline while batches keep coming.
//! - `picture` puts the tool's overrides on and lays them out first,
//!   answers with the size, the window root's full tree and its modules, and
//!   leaves `layers()`' "since the last call" as it was; with and without
//!   `rect`; a forever-animating window still has its picture, its loop at
//!   its first frame.
//! - Each is refused with a stable code: no such window, a `rect` outside
//!   the content, a path that cannot be written, a scale or path it does
//!   not take, and anywhere outside design mode.
//! - A hot update tells the tool the modules it ran again, by their hot
//!   keys.
//!
//! A process has one design host for its whole life, so the host's case
//! runs in a process of its own; the test plays the tool.

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender};
use nsh_core::{
    ApproxTextEngine, JsonValue, MountBatch, MountItem, Size,
    animation::Animator,
    element::Registry,
    geometry::px,
    platform::{Environment, Idiom, Os},
    runtime::design::{self, DesignHooks, Drawn, Failure, PictureHooks, Settling, codes},
};
use nsh_engine::{Bundle, Engine, EngineConfig, HotUpdate, MountSink};
use serde_json::json;

/// The window content's size the stand-in platform checks a `rect`
/// against, in points.
const CONTENT: (f32, f32) = (300.0, 200.0);

const CARD: &str = r#"import { Text, View } from "nsh";

export function Card(props: { title: string }) {
  return <View testID="card" class="p-2">
    <Text>{props.title}</Text>
  </View>;
}
"#;

/// The app: opens its windows, then asks for what the tests check and shows
/// it as one text, `read {...}`. `__DIR__` is where its pictures go.
const APP: &str = r#"import { Animated, createAnimatedValue, mount, signal, Text, View } from "nsh";
import { Window } from "nsh/desktop";
import { design } from "nsh/design";
import { Card } from "./Card";

const shown = signal("waiting");
const tick = signal(0);
mount(() => <View><Text>{shown()}</Text><Text>{`tick ${tick()}`}</Text></View>);

const spin = createAnimatedValue(0);
const fade = createAnimatedValue(0);
Window.open(() => <Card title="One" />, { id: "card" });
Window.open(() => <View testID="spin" style={{ opacity: spin }} />, { id: "spin" });
Window.open(() => <View testID="fade" style={{ opacity: fade }} />, { id: "fade" });

const code = async (p) => { try { await p; return "resolved"; } catch (e) { return e.code ?? e.message; } };
const timed = async (p) => { const t = Date.now(); const r = await p; return { ...r, ms: Date.now() - t }; };
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const dir = "__DIR__";

setTimeout(async () => {
  const out = {};
  await wait(200);
  out.quiet = await timed(design.settled("card"));

  // An animation that ends by itself: settled once it ended.
  Animated.timing(fade, { toValue: 1, duration: 300 }).start();
  out.fading = await timed(design.settled("fade"));

  // Batches that keep coming: given up at the deadline.
  const busy = setInterval(() => tick.set(tick() + 1), 10);
  out.busy = await timed(design.settled("card"));
  clearInterval(busy);

  // A loop with no end: settled at once, and pictured at its first frame.
  Animated.loop(Animated.timing(spin, { toValue: 1, duration: 1000 })).start();
  out.spinning = await timed(design.settled("spin"));
  await wait(400);
  out.spin = await design.picture("spin", { scale: 1, path: dir + "/spin.raw" });

  // The overrides put on and laid out before the tree is read; layers()'
  // own state is left as it is.
  design.setOverrides({ "__CARD_KEY__": { class: "w-[123px]", replace: false, states: 0 } });
  out.layersBefore = await design.layers("card");
  // Read until nothing changed since (the override's layout came after).
  for (let i = 0; i < 5 && (await design.layers("card")) !== null; i++);
  out.whole = await design.picture("card", { scale: 2, path: dir + "/card.raw" });
  out.part = await design.picture("card", { scale: 1.5, path: dir + "/part.raw", rect: { x: 10, y: 20, width: 40, height: 30 } });
  out.layersAfter = await design.layers("card");

  out.errors = {
    window: await code(design.picture("nope", { scale: 1, path: dir + "/x.raw" })),
    settledWindow: await code(design.settled("nope")),
    outside: await code(design.picture("card", { scale: 1, path: dir + "/x.raw", rect: { x: 290, y: 0, width: 20, height: 10 } })),
    path: await code(design.picture("card", { scale: 1, path: dir + "/missing/x.raw" })),
    scale: await code(design.picture("card", { scale: 0, path: dir + "/x.raw" })),
    relative: await code(design.picture("card", { scale: 1, path: "x.raw" })),
    options: await code(design.picture("card", { path: dir + "/x.raw" })),
  };
  shown.set("read " + JSON.stringify(out));
}, 0);
"#;

/// What the app outside design mode reads: each call refused.
const OUTSIDE: &str = r#"import { mount, signal, Text } from "nsh";
import { Window } from "nsh/desktop";
import { design } from "nsh/design";

const shown = signal("waiting");
mount(() => <Text>{shown()}</Text>);
Window.open(() => <Text>window</Text>, { id: "w" });
const code = async (p) => { try { await p; return "resolved"; } catch (e) { return e.code ?? e.message; } };
setTimeout(async () => {
  const settled = await code(design.settled("w"));
  const picture = await code(design.picture("w", { scale: 1, path: "/tmp/never.raw" }));
  shown.set("read " + JSON.stringify({ settled, picture }));
}, 50);
"#;

/// Forwards batches to the test and feeds the stand-in platform's animator.
struct Platform {
    tx: Sender<MountBatch>,
    state: Arc<Shared>,
}

/// What the stand-in platform knows on its "main thread".
#[derive(Default)]
struct Shared {
    animator: Mutex<Animator>,
    applied: AtomicU64,
    /// The opacities the bound views had when each picture was drawn: as
    /// the window showed them, and as the picture drew them.
    pictured: Mutex<Vec<(Vec<f32>, Vec<f32>)>>,
}

impl MountSink for Platform {
    fn submit(&self, batch: MountBatch) {
        {
            let mut animator = self.state.animator.lock().unwrap();
            for item in &batch.items {
                animator.apply(item);
            }
        }
        self.state.applied.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.send(batch);
    }
}

/// Seconds since the stand-in platform started: its animations' clock.
fn clock() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// The opacities of the views `animator` binds.
fn opacities(animator: &mut Animator) -> Vec<f32> {
    let mut out = Vec::new();
    animator.write(Some, |_, values, _| out.extend(values.opacity));
    out
}

/// The stand-in platform's part: settling by the core's rule on its
/// batches and animations (ticked as a display link would), and pictures
/// written as raw pixels of the asked size.
fn hooks(state: Arc<Shared>) -> PictureHooks {
    let settling = state.clone();
    PictureHooks {
        settled: Box::new(move |ask| {
            let state = settling.clone();
            std::thread::spawn(move || {
                let mut rule = Settling::new(Instant::now(), state.applied.load(Ordering::SeqCst));
                loop {
                    let busy = {
                        let mut animator = state.animator.lock().unwrap();
                        animator.tick(clock());
                        animator.finishing()
                    };
                    let applied = state.applied.load(Ordering::SeqCst);
                    if let Some(settled) = rule.step(Instant::now(), applied, busy) {
                        ask.reply.send(Ok(settled));
                        return;
                    }
                    std::thread::sleep(design::SETTLE_POLL);
                }
            });
        }),
        picture: Box::new(move |ask| {
            let (width, height) = ask
                .rect
                .map_or(CONTENT, |r| (r.size.width.0, r.size.height.0));
            if let Some(r) = ask.rect
                && (r.origin.x.0 + r.size.width.0 > CONTENT.0
                    || r.origin.y.0 + r.size.height.0 > CONTENT.1)
            {
                ask.reply
                    .send(Err(Failure::new(codes::RECT_OUTSIDE, "outside")));
                return;
            }
            let drawn = Drawn {
                width: (f64::from(width) * ask.scale).round() as u32,
                height: (f64::from(height) * ask.scale).round() as u32,
            };
            {
                let mut animator = state.animator.lock().unwrap();
                animator.tick(clock());
                let live = opacities(&mut animator);
                let still = opacities(&mut animator.at_loop_starts(clock()));
                state.pictured.lock().unwrap().push((live, still));
            }
            let pixels = vec![0x7f; (drawn.width * drawn.height * 4) as usize];
            match std::fs::write(&ask.path, pixels) {
                Ok(()) => ask.reply.send(Ok(drawn)),
                Err(e) => ask
                    .reply
                    .send(Err(Failure::new(codes::PATH, e.to_string()))),
            }
        }),
    }
}

/// The 1-based line and column of `written` in `source`, as stamps have
/// them.
fn place(source: &str, written: &str) -> (usize, usize) {
    let (index, line) = source
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(written))
        .unwrap_or_else(|| panic!("no {written}"));
    (index + 1, line.find(written).unwrap() + 1)
}

/// The last text the app showed that starts with `prefix` (or 20 s
/// passed).
fn read(rx: &Receiver<MountBatch>, prefix: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut last = String::new();
    while let Ok(batch) = rx.recv_deadline(deadline) {
        for item in &batch.items {
            if let MountItem::Text { text, .. } = item {
                last = text.text.clone();
                if last.starts_with(prefix) {
                    return last;
                }
            }
        }
    }
    last
}

/// Bundles `app` (and the card) for development in `dir` and starts it on
/// a desktop that grants windows.
fn start(
    dir: &Path,
    app: &str,
    sink: impl MountSink + 'static,
) -> (Engine, nsh_bundler::HotBundle) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("App.tsx"), app).unwrap();
    std::fs::write(dir.join("Card.tsx"), CARD).unwrap();
    let sets = vec![nsh_base::registry()];
    let described: Vec<nsh_core::element::description::SetDescription> =
        Registry::served_sets(&sets)
            .into_iter()
            .map(Into::into)
            .collect();
    let bundle = nsh_bundler::bundle_hot(&dir.join("App.tsx"), &described).unwrap();
    let (code, map) = bundle.script();
    let mut config = EngineConfig::new(nsh_engine::App::Js(
        Bundle::source(code)
            .with_source_map(map.unwrap())
            .with_capabilities_json(Some(r#"{ "windows": true }"#))
            .unwrap(),
    ));
    config.environment = Environment {
        os: Os::MacOs,
        idiom: Idiom::Desktop,
        ..Environment::default()
    };
    config.viewport = Size::new(px(390.0), px(844.0));
    config.ui_sets = sets;
    let engine = Engine::start(config, || ApproxTextEngine, sink).unwrap();
    (engine, bundle)
}

/// Outside design mode every call is refused, as not active.
#[test]
fn pictures_are_refused_outside_design_mode() {
    assert!(!design::attached());
    let dir = std::env::temp_dir().join(format!("nsh-design-outside-{}", std::process::id()));
    let (tx, rx) = crossbeam_channel::unbounded();
    let (engine, _) = start(
        &dir,
        OUTSIDE,
        Platform {
            tx,
            state: Arc::default(),
        },
    );
    let text = read(&rx, "read ");
    drop(engine);
    let _ = std::fs::remove_dir_all(&dir);
    let read: JsonValue = serde_json::from_str(text.strip_prefix("read ").unwrap()).unwrap();
    assert_eq!(
        read,
        json!({ "settled": codes::NOT_ACTIVE, "picture": codes::NOT_ACTIVE })
    );
}

/// The settle rule alone: quiet for its time, busy until the deadline.
#[test]
fn the_settle_rule_waits_for_quiet_and_gives_up_at_its_limit() {
    let t0 = Instant::now();
    let mut rule = Settling::new(t0, 3);
    assert_eq!(rule.step(t0 + Duration::from_millis(30), 3, false), None);
    // A batch: quiet again from it.
    assert_eq!(rule.step(t0 + Duration::from_millis(40), 4, false), None);
    assert_eq!(rule.step(t0 + Duration::from_millis(80), 4, false), None);
    assert_eq!(
        rule.step(t0 + Duration::from_millis(90), 4, false),
        Some(design::Settled { timed_out: false })
    );
    let mut busy = Settling::new(t0, 0);
    for ms in (0..1000).step_by(10) {
        assert_eq!(busy.step(t0 + Duration::from_millis(ms), 0, true), None);
    }
    assert_eq!(
        busy.step(t0 + design::SETTLE_LIMIT, 0, true),
        Some(design::Settled { timed_out: true })
    );
}

/// Starts [`in_a_design_host`] in a process of its own as its design tool:
/// it listens, then checks what the host told it (a hot update's modules).
#[test]
fn a_design_host_settles_and_draws_its_windows() {
    let tool = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = tool.local_addr().unwrap().port();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "design_pictures::in_a_design_host",
            "--ignored",
            "--nocapture",
        ])
        .env(design::SOCKET, format!("tcp:127.0.0.1:{port}"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let (connection, _) = tool.accept().unwrap();
    connection
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let lines: Vec<JsonValue> = BufReader::new(connection)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect();
    let result = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        result.status.success() && stdout.contains("1 passed"),
        "{stdout}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(lines.first(), Some(&json!({ "hello": {} })));
    let notices: Vec<&JsonValue> = lines
        .iter()
        .filter(|l| l.get("hotUpdate").is_some())
        .collect();
    // The card's module ran again, named as its stamps name it; the app's
    // did not (the card takes its own updates).
    assert_eq!(
        notices,
        [&json!({ "hotUpdate": { "modules": ["Card.tsx"] } })],
        "{lines:?}"
    );
}

/// The process a design tool started: the host connects with the stand-in
/// platform's pictures, the app asks, and a hot update follows.
#[test]
#[ignore = "run by a_design_host_settles_and_draws_its_windows in a process of its own"]
fn in_a_design_host() {
    let endpoint = design::endpoint().expect("started with the tool's endpoint");
    assert!(design::connect(
        endpoint,
        DesignHooks {
            input: Box::new(|_| {}),
            closed: Box::new(|| {}),
        },
    ));
    let state = Arc::new(Shared::default());
    assert!(design::set_picture_hooks(hooks(state.clone())));
    let dir: PathBuf =
        std::env::temp_dir().join(format!("nsh-design-pictures-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (line, column) = place(CARD, "<Text");
    let app = APP
        .replace("__DIR__", &dir.to_string_lossy())
        .replace("__CARD_KEY__", &format!("Card.tsx:{line}:{column}"));
    let (tx, rx) = crossbeam_channel::unbounded();
    let (engine, mut bundle) = start(
        &dir,
        &app,
        Platform {
            tx,
            state: state.clone(),
        },
    );
    let text = read(&rx, "read ");
    let read: JsonValue = serde_json::from_str(
        text.strip_prefix("read ")
            .unwrap_or_else(|| panic!("the app read nothing: {text}")),
    )
    .unwrap();
    let ms = |key: &str| read[key]["ms"].as_f64().unwrap();
    let timed_out = |key: &str| read[key]["timedOut"].as_bool().unwrap();

    // Quiet: settled after the quiet time, not given up.
    assert!(!timed_out("quiet"), "{read}");
    assert!((45.0..600.0).contains(&ms("quiet")), "{read}");
    // An animation that ends by itself (300 ms): settled after it ended.
    assert!(!timed_out("fading"), "{read}");
    assert!(ms("fading") >= 290.0, "{read}");
    // Batches every 10 ms: given up at the limit.
    assert!(timed_out("busy"), "{read}");
    assert!(ms("busy") >= 990.0, "{read}");
    // A loop with no end does not hold it.
    assert!(!timed_out("spinning"), "{read}");
    assert!(ms("spinning") < 600.0, "{read}");

    // The spinner's picture: its loop at its first frame, while the window
    // showed it on its way.
    assert_eq!(read["spin"]["width"], 300);
    let pictured = state.pictured.lock().unwrap().clone();
    let (live, still) = &pictured[0];
    let between = |o: &f32| *o > 0.05 && *o < 0.95;
    assert!(
        live.iter().any(between),
        "the loop is not on its way: {live:?}"
    );
    assert!(
        !still.iter().any(between),
        "the picture's loop moved: {still:?}"
    );
    assert!(
        still
            .iter()
            .all(|o| o.abs() < 1e-3 || (o - 1.0).abs() < 1e-3),
        "the picture's loop is not at its first frame: {still:?} (live {live:?})"
    );
    assert!(
        still.iter().any(|o| o.abs() < 1e-3),
        "the picture's loop is not at its start: {still:?}"
    );

    // The whole window and a part of it, in the file the tool named.
    assert_eq!(
        (
            read["whole"]["width"].clone(),
            read["whole"]["height"].clone()
        ),
        (json!(600), json!(400))
    );
    assert_eq!(
        std::fs::metadata(dir.join("card.raw")).unwrap().len(),
        600 * 400 * 4
    );
    assert_eq!(
        (
            read["part"]["width"].clone(),
            read["part"]["height"].clone()
        ),
        (json!(60), json!(45))
    );
    assert_eq!(
        std::fs::metadata(dir.join("part.raw")).unwrap().len(),
        60 * 45 * 4
    );

    // Its tree: the card's layers under the window's root, the override put
    // on and laid out, the instance of the card, and the modules.
    let layers = read["whole"]["layers"].as_array().unwrap();
    let card = layers
        .iter()
        .find(|l| l["testId"] == "card")
        .unwrap_or_else(|| panic!("no card in {layers:?}"));
    assert_eq!(card["source"]["module"], "Card.tsx");
    let title = layers
        .iter()
        .find(|l| l["text"] == "One")
        .unwrap_or_else(|| panic!("no title in {layers:?}"));
    assert_eq!(title["overridden"], true, "{title}");
    assert_eq!(
        title["width"], 123.0,
        "the override is not laid out: {title}"
    );
    assert_eq!(title["parent"], card["id"]);
    assert!(
        layers
            .iter()
            .any(|l| l["kind"] == "component" && l["name"] == "Card"),
        "{layers:?}"
    );
    // Nodes in `layers()`' shape, with `slot` and `place`.
    assert!(
        layers
            .iter()
            .filter(|l| l["kind"] != "component")
            .all(|l| l.get("place").is_some() && l.get("slot").is_some() && l.get("x").is_some()),
        "{layers:?}"
    );
    assert_eq!(read["whole"]["modules"], json!(["App.tsx", "Card.tsx"]));
    assert_eq!(read["part"]["layers"], read["whole"]["layers"]);
    // `layers()` had read the card up to date before; the pictures left its
    // "since the last call" as it was: nothing changed since.
    assert!(read["layersBefore"]["roots"].get("").is_some(), "{read}");
    assert_eq!(read["layersAfter"], JsonValue::Null, "{read}");

    // Refused, each with its code.
    assert_eq!(
        read["errors"],
        json!({
            "window": codes::NO_WINDOW,
            "settledWindow": codes::NO_WINDOW,
            "outside": codes::RECT_OUTSIDE,
            "path": codes::PATH,
            "scale": codes::INVALID,
            "relative": codes::INVALID,
            "options": codes::INVALID,
        })
    );

    // A hot update of the card: the tool hears which module ran again.
    std::fs::write(dir.join("Card.tsx"), CARD.replace("p-2", "p-3")).unwrap();
    let sets = vec![nsh_base::registry()];
    let described: Vec<nsh_core::element::description::SetDescription> =
        Registry::served_sets(&sets)
            .into_iter()
            .map(Into::into)
            .collect();
    let next = nsh_bundler::bundle_hot(&dir.join("App.tsx"), &described).unwrap();
    let nsh_bundler::HotPatch::Update { code, source_map } = next.patch(&bundle) else {
        panic!("expected an update");
    };
    bundle = next;
    engine.control().hot_update(HotUpdate {
        name: "hot-1.js".to_owned(),
        code,
        source_map: source_map.map(Into::into),
    });
    // Applied: the next batch comes, and the notice was sent before it.
    let _ = rx.recv_timeout(Duration::from_secs(5));
    std::thread::sleep(Duration::from_millis(200));
    drop(bundle);
    drop(engine);
    let _ = std::fs::remove_dir_all(&dir);
}
