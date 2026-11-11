//! `token(name)` follows its token: a binding that read a token runs again
//! when that token's value changes (or the appearance does), and a change
//! of another token runs nothing of it. A binding that read two tokens
//! runs once when both change. A change of most of the tokens read (a new
//! theme) runs every reader once.

use nsh_core::{MountBatch, MountItem};
use nsh_engine::Environment;

/// A TSX app with a text per token read, each counting its runs, changing
/// its theme a step every 100 ms: each step a `setTheme` of the theme it
/// has with some values changed.
const READERS: &str = r##"import { mount, setColorScheme, setTheme, signal, Text, token, View } from "nsh";

const runs = { brand: 0, accent: 0, both: 0, extra: 0, others: 0 };
const status = signal("start");
let light = { brand: "#123456", accent: "#222222", one: "#111111", two: "#222222", three: "#333333" };
let shared = {};
const set = (colors, numbers = {}) => {
  light = { ...light, ...colors };
  shared = { ...shared, ...numbers };
  setTheme({ cssVars: { theme: shared, light } });
};
const steps = [
  () => {
    set({ brand: "#00ff00" });
    return "step 1";
  },
  () => {
    set({ accent: "#0000ff" });
    return "step 2";
  },
  () => {
    set({ brand: "#111111", accent: "#333333" });
    return "step 3";
  },
  () => {
    // A token the theme did not have: what read its name gets it.
    set({ extra: "#ff0000" });
    return "step 4";
  },
  () => {
    // A token nothing here read.
    set({}, { radius: "4px" });
    return "step 5";
  },
  () => {
    // Most of the six names read here: every reader runs, once.
    set({ brand: "#aaaaaa", accent: "#bbbbbb", one: "#010101", two: "#020202" });
    return "step 6";
  },
  () => {
    setColorScheme("dark");
    return "step 7";
  },
  () => "done",
];
steps.forEach((step, i) => setTimeout(() => status.set(step()), 100 * (i + 1)));

mount(() => (
  <View>
    <Text>{`brand ${token("brand")} run ${++runs.brand}`}</Text>
    <Text>{`accent ${token("accent")} run ${++runs.accent}`}</Text>
    <Text>{`both ${token("brand")} ${token("accent")} run ${++runs.both}`}</Text>
    <Text>{`extra ${token("extra")} run ${++runs.extra}`}</Text>
    <Text>{`others ${token("one")} ${token("two")} ${token("three")} run ${++runs.others}`}</Text>
    <Text>{status()}</Text>
  </View>
));
"##;

/// The texts the batches show that start with `prefix`, in order.
fn texts<'a>(batches: &'a [MountBatch], prefix: &str) -> Vec<&'a str> {
    batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::Text { text, .. } if text.text.starts_with(prefix) => Some(&*text.text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_binding_runs_again_only_for_the_tokens_it_read() {
    let config = r##"{ "theme": { "cssVars": { "light": { "brand": "#123456", "accent": "#222222", "one": "#111111", "two": "#222222", "three": "#333333" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(READERS, Environment::default(), None, Some(config));
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, "done"));
    let brand = texts(&batches, "brand ");
    let accent = texts(&batches, "accent ");
    let both = texts(&batches, "both ");
    let extra = texts(&batches, "extra ");
    let others = texts(&batches, "others ");
    // Its own token, both tokens, most tokens: one run each. Not for
    // `accent`, `extra` or `radius`.
    assert_eq!(
        brand[..4],
        [
            "brand #123456ff run 1",
            "brand #00ff00ff run 2",
            "brand #111111ff run 3",
            "brand #aaaaaaff run 4"
        ]
    );
    assert_eq!(
        accent[..4],
        [
            "accent #222222ff run 1",
            "accent #0000ffff run 2",
            "accent #333333ff run 3",
            "accent #bbbbbbff run 4"
        ]
    );
    // Either token; once when both change at once.
    assert_eq!(
        both[..5],
        [
            "both #123456ff #222222ff run 1",
            "both #00ff00ff #222222ff run 2",
            "both #00ff00ff #0000ffff run 3",
            "both #111111ff #333333ff run 4",
            "both #aaaaaaff #bbbbbbff run 5"
        ]
    );
    // A name the tokens lacked is followed too; a change of most tokens
    // runs it with the rest, its own unchanged.
    assert_eq!(
        extra[..3],
        [
            "extra undefined run 1",
            "extra #ff0000ff run 2",
            "extra #ff0000ff run 3"
        ]
    );
    // Untouched until most tokens change.
    assert_eq!(
        others[..2],
        [
            "others #111111ff #222222ff #333333ff run 1",
            "others #010101ff #020202ff #333333ff run 2"
        ]
    );
    // The appearance runs every one of them once more, and that is all:
    // `radius` ran none.
    for (texts, runs) in [
        (&brand, 5),
        (&accent, 5),
        (&both, 6),
        (&extra, 4),
        (&others, 3),
    ] {
        assert_eq!(texts.len(), runs, "{texts:?}");
        let last = texts.last().unwrap();
        assert!(last.ends_with(&format!("run {runs}")), "{last}");
    }
}

/// More names than get a signal of their own: every one still follows its
/// token (through the signal of all of them).
const MANY_NAMES: &str = r##"import { mount, setTheme, signal, Text, token, View } from "nsh";

for (let i = 0; i < 1100; i++) token(`made-up-${i}`);
const status = signal("start");
setTimeout(() => {
  setTheme({ cssVars: { light: { late: "#00ff00" } } });
  status.set("done");
}, 100);

mount(() => (
  <View>
    <Text>{`late ${token("late")}`}</Text>
    <Text>{status()}</Text>
  </View>
));
"##;

#[test]
fn a_name_past_the_bound_still_follows_its_token() {
    let config = r##"{ "theme": { "cssVars": { "light": { "late": "#123456" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(MANY_NAMES, Environment::default(), None, Some(config));
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, "done"));
    assert_eq!(
        texts(&batches, "late "),
        ["late #123456ff", "late #00ff00ff"]
    );
}

/// A benchmark of `token()` readers, driven as `component_bench` drives
/// its screens: 300 cards styled with `token()` in inline styles (11 reads
/// a card), then the tokens change: one that a card reads once, and all of
/// them. With the first frame, which makes the readers.
///
/// `cargo test --release -p nsh-engine --test engine bench_token_readers -- --ignored --nocapture`
mod bench {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use nsh_core::{ApproxTextEngine, Core, Size, element::Tokens, geometry::px};
    use nsh_js::Host;

    const CARDS: &str = r#"import { mount, Scroll, Text, token, View } from "nsh";

function Card(props: { i: number }) {
  return (
    <View style={{ backgroundColor: token("card"), borderColor: token("border"), borderWidth: 1, borderRadius: token("radius"), padding: 16, gap: 8 }}>
      <Text style={{ color: token("card-foreground"), fontWeight: "600" }}>{`Card ${props.i}`}</Text>
      <Text style={{ color: token("muted-foreground"), fontSize: 14 }}>{`Subtitle of card ${props.i}`}</Text>
      <View style={{ flexDirection: "row", gap: 8 }}>
        <View style={{ backgroundColor: token("primary"), borderRadius: 6, paddingHorizontal: 12, paddingVertical: 8 }}>
          <Text style={{ color: token("primary-foreground"), fontSize: 14 }}>Open</Text>
        </View>
        <View style={{ backgroundColor: token("secondary"), borderRadius: 6, paddingHorizontal: 12, paddingVertical: 8 }}>
          <Text style={{ color: token("secondary-foreground"), fontSize: 14 }}>Share</Text>
        </View>
        <View style={{ backgroundColor: token("brand"), borderRadius: 999, paddingHorizontal: 8, paddingVertical: 4 }}>
          <Text style={{ color: token("background"), fontSize: 12 }}>New</Text>
        </View>
      </View>
    </View>
  );
}

const rows = Array.from({ length: 300 }, (_, i) => i);

mount(() => (
  <Scroll style={{ flex: 1 }}>
    <View style={{ padding: 16, gap: 8 }}>{rows.map((i) => <Card i={i} />)}</View>
  </Scroll>
));
"#;

    const COLORS: [&str; 10] = [
        "background",
        "card",
        "card-foreground",
        "muted-foreground",
        "primary",
        "primary-foreground",
        "secondary",
        "secondary-foreground",
        "border",
        "brand",
    ];

    /// The tokens the cards read; `k` picks their values, `brand` the
    /// brand's own.
    fn tokens(k: u32, brand: u32) -> Arc<Tokens> {
        let mut tokens = Tokens::default();
        for (i, name) in COLORS.iter().enumerate() {
            let light = 0x10_20_30 * (i as u32 + 1) + 0x01_01_01 * k;
            tokens.set_color(
                name,
                nsh_core::element::AdaptiveColor::rgb(light, 0xff_ff_ff - light),
            );
        }
        tokens.set_color(
            "brand",
            nsh_core::element::AdaptiveColor::rgb(brand, 0xff_ff_ff - brand),
        );
        tokens.set_number("radius", 10.0 + k as f32);
        Arc::new(tokens)
    }

    fn bytecode() -> Vec<u8> {
        let dir = std::env::temp_dir().join(format!("nsh-token-bench-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cards.tsx"), CARDS).unwrap();
        let ui_sets = [crate::test_ui::registry()];
        let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
            entry: dir.join("Cards.tsx"),
            minify: false,
            split: false,
            ui_sets: ui_sets.iter().map(Into::into).collect(),
            ..Default::default()
        })
        .unwrap();
        nsh_js::compile_bytecode_with(
            &bundle.code,
            true,
            &ui_sets.iter().map(Into::into).collect::<Vec<_>>(),
        )
        .unwrap()
    }

    struct App {
        host: Host,
        core: Core<ApproxTextEngine>,
        ops: Vec<nsh_core::Op>,
    }

    impl App {
        fn new() -> Self {
            let host = Host::with_options(nsh_js::HostOptions {
                ui_sets: vec![crate::test_ui::registry()],
                ..Default::default()
            })
            .unwrap();
            host.set_environment(&nsh_js::Environment {
                window: Size::new(px(390.0), px(844.0)),
                ..Default::default()
            });
            host.set_tokens(tokens(0, 0x2563eb));
            let mut core = Core::new(ApproxTextEngine, Size::new(px(390.0), px(844.0)));
            core.set_components(crate::test_ui::registry());
            Self {
                host,
                core,
                ops: Vec::new(),
            }
        }

        /// Runs `js`, then applies its ops and commits: the JS time, the
        /// core's and the ops applied.
        fn turn(&mut self, js: impl FnOnce(&Host)) -> (Duration, Duration, usize) {
            let t = Instant::now();
            js(&self.host);
            self.host.run_jobs();
            let js = t.elapsed();
            let t = Instant::now();
            self.host.take_ops(&mut self.ops);
            let ops = self.ops.len();
            for op in self.ops.drain(..) {
                self.core.apply(op).unwrap();
            }
            self.core.commit();
            (js, t.elapsed(), ops)
        }
    }

    fn report(what: &str, runs: &mut [(Duration, Duration, usize)]) {
        let us = |d: Duration| d.as_secs_f64() * 1e6;
        let mut js: Vec<_> = runs.iter().map(|r| r.0).collect();
        let mut core: Vec<_> = runs.iter().map(|r| r.1).collect();
        js.sort();
        core.sort();
        println!(
            "{what}: js {:.0} us (median {:.0}), core {:.0} us (median {:.0}), {} ops",
            us(js[0]),
            us(js[js.len() / 2]),
            us(core[0]),
            us(core[core.len() / 2]),
            runs[runs.len() - 1].2,
        );
    }

    #[test]
    #[ignore]
    fn bench_token_readers() {
        let code = bytecode();
        let mut first = Vec::new();
        let mut app = None;
        for _ in 0..30 {
            let mut a = App::new();
            first.push(a.turn(|h| h.eval_bytecode(&code).unwrap()));
            app = Some(a);
        }
        report("first frame, 3 300 token() reads", &mut first);
        let mut app = app.unwrap();
        for (what, changed) in [
            ("one token changed", tokens(0, 0xdb2777)),
            ("every token changed", tokens(1, 0x16a34a)),
        ] {
            let mut runs = Vec::new();
            for round in 0..200 {
                let next = if round % 2 == 0 {
                    changed.clone()
                } else {
                    tokens(0, 0x2563eb)
                };
                runs.push(app.turn(|h| h.set_tokens(next)));
            }
            report(what, &mut runs);
        }
    }
}
