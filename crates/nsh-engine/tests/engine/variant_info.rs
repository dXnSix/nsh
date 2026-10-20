//! `Image.getSize` and `Image.prefetch` ask for the variant the views load
//! at the surface's pixel ratio; `getSize` gives the imported file's size.

use nsh_core::{Command, EventKind, EventPayload, MountItem, PayloadValue, ROOT_ID};

use crate::variant_repick::{batches, start};

const APP: &str = r#"import { mount, signal, Text } from "nsh";
import { Image } from "nsh/image";

const icon = { uri: "asset:/img.png", width: 40, height: 30, scales: [1, 2, 3] };
const shown = signal("waiting");
setTimeout(async () => {
  const size = await Image.getSize(icon);
  const plain = await Image.getSize({ uri: "asset:/single.png", width: 10, height: 10 });
  const ready = await Image.prefetch(icon);
  shown.set(`${size.width}x${size.height} ${plain.width}x${plain.height} ${ready}`);
}, 0);

mount(() => <Text>{shown()}</Text>);
"#;

/// Answers every image request with its file's pixel size (the variant's
/// scale times 40x30 for `img`, 10x10 for `single`), until the app shows
/// its result: the URIs asked for and the text shown.
fn run(pixel_ratio: f32) -> (Vec<String>, String) {
    let (engine, rx) = start(APP, pixel_ratio);
    let control = engine.control();
    let mut asked = Vec::new();
    let mut shown = String::new();
    for _ in 0..20 {
        for batch in batches(&rx) {
            for item in &batch.items {
                match item {
                    MountItem::Command {
                        id: ROOT_ID,
                        command: Command::ImageInfo { id, uri },
                    } => {
                        asked.push(uri.clone());
                        let scale = match uri.as_str() {
                            "asset:/img@2x.png" => 2.0,
                            "asset:/img@3x.png" => 3.0,
                            _ => 1.0,
                        };
                        let (w, h) = if uri.contains("single") {
                            (10.0, 10.0)
                        } else {
                            (40.0 * scale, 30.0 * scale)
                        };
                        control.dispatch_event(
                            ROOT_ID,
                            EventKind::ImageInfo,
                            EventPayload::Map(vec![
                                ("id", PayloadValue::Number(*id as f64)),
                                ("width", PayloadValue::Number(w)),
                                ("height", PayloadValue::Number(h)),
                            ]),
                        );
                    }
                    MountItem::Text { text, .. } => shown = text.text.clone(),
                    _ => {}
                }
            }
        }
        if shown != "waiting" && !shown.is_empty() {
            break;
        }
    }
    (asked, shown)
}

#[test]
fn variant_info_get_size_and_prefetch_ask_for_the_picked_variant() {
    let (asked, shown) = run(3.0);
    assert_eq!(
        asked,
        [
            "asset:/img@3x.png",
            "asset:/single.png",
            "asset:/img@3x.png"
        ]
    );
    // The imported file's size, whatever variant was asked for.
    assert_eq!(shown, "40x30 10x10 true");

    let (asked, shown) = run(2.0);
    assert_eq!(asked[0], "asset:/img@2x.png");
    assert_eq!(shown, "40x30 10x10 true");

    let (asked, shown) = run(1.0);
    assert_eq!(asked[0], "asset:/img.png");
    assert_eq!(shown, "40x30 10x10 true");
}
