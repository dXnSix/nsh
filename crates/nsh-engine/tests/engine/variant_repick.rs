//! Image variants follow the pixel ratio: a surface at 3 mounts the `@3x`
//! file of an image with scales, and moving to 2 sends that image again
//! as `@2x`, alone, in one batch; images without scales are not sent.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_core::{ApproxTextEngine, MountBatch, MountItem, NodeId, Size};
use nsh_engine::{Bundle, Engine, EngineConfig, Environment};

use crate::ChannelSink;

/// `app` (TSX), bundled, compiled and started on a surface at
/// `pixel_ratio`.
pub(crate) fn start(app: &str, pixel_ratio: f32) -> (Engine, Receiver<MountBatch>) {
    static APPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "nsh-variants-{}-{}",
        std::process::id(),
        APPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("App.tsx");
    std::fs::write(&entry, app).unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry,
        minify: false,
        split: false,
        ui_sets: Vec::new(),
    })
    .unwrap();
    let bytecode = nsh_js::compile_bytecode_with(&bundle.code, false, &[]).unwrap();
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        // SAFETY: compiled just now by this runtime.
        app: nsh_engine::App::Js(unsafe { Bundle::bytecode(bytecode) }),
        viewport: Size::new(390.0, 844.0),
        trace: false,
        network: None,
        assets: None,
        environment: Environment {
            pixel_ratio,
            ..Environment::default()
        },
        header_heights: (44.0, 96.0),
        limits: nsh_engine::Limits::default(),
        ui_sets: Vec::new(),
        runtime: None,
        data_dir: None,
        storage: None,
    };
    (
        Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap(),
        rx,
    )
}

/// The batches that arrive within a moment.
pub(crate) fn batches(rx: &Receiver<MountBatch>) -> Vec<MountBatch> {
    let mut out = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(300)) {
        out.push(batch);
    }
    out
}

/// The image items of `batch`: node and source URI.
fn images(batch: &MountBatch) -> Vec<(NodeId, String)> {
    batch
        .items
        .iter()
        .filter_map(|i| match i {
            MountItem::Image { id, source, .. } => Some((
                *id,
                source.as_ref().map_or(String::new(), |s| s.uri.to_string()),
            )),
            _ => None,
        })
        .collect()
}

const APP: &str = r#"import { mount, View } from "nsh";
import { Image } from "nsh/image";

const icon = { uri: "asset:/img.png", width: 40, height: 30, scales: [1, 2, 3] };
const single = { uri: "asset:/single.png", width: 10, height: 10 };
const big = { uri: "asset:/big.png", width: 20, height: 20, scales: [1, 2, 4] };

mount(() => (
  <View>
    <Image src={icon} />
    <Image src={single} />
    <Image src={big} />
  </View>
));
"#;

#[test]
fn variant_repick_sends_only_the_images_whose_variant_changes() {
    let mut environment = Environment {
        pixel_ratio: 3.0,
        ..Environment::default()
    };
    let (engine, rx) = start(APP, environment.pixel_ratio);
    let first = batches(&rx);
    let mounted: Vec<_> = first.iter().flat_map(images).collect();
    let uris: Vec<&str> = mounted.iter().map(|(_, u)| u.as_str()).collect();
    assert_eq!(
        uris,
        [
            "asset:/img@3x.png",
            "asset:/single.png",
            "asset:/big@4x.png"
        ]
    );
    let icon = mounted[0].0;

    // 3 → 2: both scaled images go to `@2x`; `single` is not sent.
    environment.pixel_ratio = 2.0;
    engine.control().set_environment(environment);
    let after = batches(&rx);
    let changed: Vec<_> = after.iter().map(images).filter(|i| !i.is_empty()).collect();
    assert_eq!(changed.len(), 1, "one batch: {changed:?}");
    let mut sent = changed[0].clone();
    sent.sort();
    let big = sent[1].0;
    assert_eq!(
        sent,
        [
            (icon, "asset:/img@2x.png".to_owned()),
            (big, "asset:/big@2x.png".to_owned())
        ]
    );
    // No relayout: no frames moved.
    assert!(
        !after
            .iter()
            .flat_map(|b| &b.items)
            .any(|i| matches!(i, MountItem::Frame { .. })),
        "no relayout"
    );

    // 2 → 2.5: the smallest scale at least 2.5: `@3x` and `@4x`.
    environment.pixel_ratio = 2.5;
    engine.control().set_environment(environment);
    let mut sent: Vec<_> = batches(&rx).iter().flat_map(images).collect();
    sent.sort();
    assert_eq!(
        sent,
        [
            (icon, "asset:/img@3x.png".to_owned()),
            (big, "asset:/big@4x.png".to_owned())
        ]
    );

    // 2.5 → 3: neither pick changes, nothing is sent.
    environment.pixel_ratio = 3.0;
    engine.control().set_environment(environment);
    let sent: Vec<_> = batches(&rx).iter().flat_map(images).collect();
    assert!(sent.is_empty(), "{sent:?}");
}

/// Many ratio changes keep the item count bounded: one image item per
/// image whose pick changed, each time.
#[test]
fn variant_repick_item_counts_stay_bounded() {
    let mut environment = Environment {
        pixel_ratio: 1.0,
        ..Environment::default()
    };
    let (engine, rx) = start(APP, environment.pixel_ratio);
    let _ = batches(&rx);
    for ratio in [2.0, 1.0, 2.0, 1.0, 3.0, 1.0] {
        environment.pixel_ratio = ratio;
        engine.control().set_environment(environment);
        let after = batches(&rx);
        let items: usize = after.iter().map(|b| b.items.len()).sum();
        let sent = after.iter().flat_map(images).count();
        assert_eq!(sent, 2, "at {ratio}: the icon and big");
        assert!(items <= 4, "at {ratio}: {items} items");
    }
}
