//! Gradient tokens from TSX: a theme document's `gradient` token,
//! `token()` giving it as an object of its members, references resolved,
//! in the context picked, and its class `bg-*` drawing it.

use nsh_core::{Gradient, MountBatch, MountItem};
use nsh_engine::Environment;

use crate::components::{shows, start_with, until};

const APP: &str = r##"import { mount, setModifier, setTheme, signal, Text, token, View } from "nsh";

setModifier("colorMode", "__MODE__");
const step = signal(0);
setTimeout(() => {
  setTheme({
    tokens: {
      "background-image-hero": {
        kind: "gradient",
        value: { type: "radial", stops: [{ color: "#ffffff" }, { color: "#000000" }] },
      },
    },
  });
  step.set(1);
}, 30);

const shown = (name) => {
  const g = token(name);
  return g === undefined ? "undefined" : `${g.type} ${g.angle} ${g.stops.map((s) => s.color).join(",")}`;
};

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`hero ${shown("background-image-hero")}`}</Text>
    <Text>{`brand ${shown("background-image-brand")}`}</Text>
    <View class="h-4 bg-hero" />
    <View class="h-4 bg-brand" />
  </View>
));
"##;

const CONFIG: &str = r##"{ "theme": {
    "tokens": {
        "up": { "kind": "color", "value": "#00ff00" },
        "background-image-hero": { "kind": "gradient", "value": {
            "type": "linear", "angle": 135, "stops": [{ "color": "#7c3aed" }, { "color": "#db2777" }] } },
        "background-image-brand": { "kind": "gradient", "value": {
            "type": "linear", "angle": 90, "stops": [{ "color": "{up}" }, { "color": "#000000" }] } } },
    "modifiers": { "colorMode": { "default": "light", "contexts": {
        "light": {},
        "dark": {
            "up": "#004400",
            "background-image-hero": { "type": "linear", "angle": 135, "stops": [{ "color": "#a78bfa" }, { "color": "#f472b6" }] } } } } },
    "resolutionOrder": ["colorMode"] } }"##;

/// The gradients the batches gave views as backgrounds, in order.
fn images(batches: &[MountBatch]) -> Vec<String> {
    batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|i| match i {
            MountItem::ViewStyle { style, .. } => style
                .effects
                .as_ref()
                .filter(|e| !e.background_images.is_empty())
                .map(|e| {
                    e.background_images
                        .iter()
                        .map(Gradient::css)
                        .collect::<Vec<_>>()
                        .join(", ")
                }),
            _ => None,
        })
        .collect()
}

#[test]
fn a_gradient_token_is_read_and_drawn_in_the_context_picked() {
    for (mode, hero, brand, hero_css, brand_css) in [
        (
            "light",
            "linear 135 #7c3aed,#db2777",
            "linear 90 #00ff00,#000000",
            "linear-gradient(135deg, #7c3aedff, #db2777ff)",
            "linear-gradient(90deg, #00ff00ff, #000000ff)",
        ),
        (
            "dark",
            "linear 135 #a78bfa,#f472b6",
            "linear 90 #004400,#000000",
            "linear-gradient(135deg, #a78bfaff, #f472b6ff)",
            "linear-gradient(90deg, #004400ff, #000000ff)",
        ),
    ] {
        let app = APP.replace("__MODE__", mode);
        let (_engine, rx) = start_with(&app, Environment::default(), None, Some(CONFIG));
        let batches = until(&rx, |b| shows(b, "step 0"));
        let said = |text: String| batches.iter().any(|b| shows(b, &text));
        assert!(said(format!("hero {hero}")), "{mode}: {batches:?}");
        // A reference in a stop is the color the context gives it.
        assert!(said(format!("brand {brand}")), "{mode}: {batches:?}");
        // The classes draw the tokens.
        assert_eq!(images(&batches), [hero_css, brand_css], "{mode}");
        // Another document's gradient reaches what read the token and the
        // class; the token the new document lacks is gone.
        let batches = until(&rx, |b| shows(b, "step 1"));
        assert!(
            batches
                .iter()
                .any(|b| shows(b, "hero radial undefined #ffffff,#000000")),
            "{mode}: {batches:?}"
        );
        assert!(
            batches.iter().any(|b| shows(b, "brand undefined")),
            "{mode}: {batches:?}"
        );
        let radial = "radial-gradient(ellipse farthest-corner at 50% 50%, #ffffffff, #000000ff)";
        assert_eq!(images(&batches), [radial], "{mode}");
    }
}
