//! Gradient tokens from TSX: a theme file's `background-image-*` value
//! written as CSS gradients is a token, `token()` gives its CSS in the
//! appearance shown, and its class `bg-*` draws it.

use nsh_core::{Gradient, MountBatch, MountItem};
use nsh_engine::Environment;

use crate::components::{shows, start_with, until};

const APP: &str = r##"import { mount, setTheme, signal, Text, token, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  setTheme({ cssVars: { theme: { "background-image-hero": "radial-gradient(#ffffff, #000000)" } } });
  step.set(1);
}, 30);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`hero ${token("background-image-hero")}`}</Text>
    <Text>{`brand ${token("background-image-brand")}`}</Text>
    <View class="h-4 bg-hero" />
    <View class="h-4" style={{ backgroundImage: token("background-image-brand") }} />
  </View>
));
"##;

fn config(appearance: &str) -> String {
    format!(
        r##"{{ "appearance": "{appearance}", "theme": {{ "cssVars": {{
            "theme": {{ "background-image-brand": "linear-gradient(to right, var(--up), #000000)" }},
            "light": {{ "up": "#00ff00", "background-image-hero": "linear-gradient(135deg, #7c3aed, #db2777)" }},
            "dark": {{ "up": "#004400", "background-image-hero": "linear-gradient(135deg, #a78bfa, #f472b6)" }} }} }} }}"##
    )
}

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
fn a_gradient_token_is_read_and_drawn_in_the_appearance_shown() {
    for (appearance, hero, brand) in [
        (
            "light",
            "linear-gradient(135deg, #7c3aedff, #db2777ff)",
            "linear-gradient(90deg, #00ff00ff, #000000ff)",
        ),
        (
            "dark",
            "linear-gradient(135deg, #a78bfaff, #f472b6ff)",
            "linear-gradient(90deg, #004400ff, #000000ff)",
        ),
    ] {
        let (_engine, rx) =
            start_with(APP, Environment::default(), None, Some(&config(appearance)));
        let batches = until(&rx, |b| shows(b, "step 0"));
        let said = |text: String| batches.iter().any(|b| shows(b, &text));
        assert!(said(format!("hero {hero}")), "{appearance}: {batches:?}");
        // A `var()` in a stop is the theme's color in that appearance.
        assert!(said(format!("brand {brand}")), "{appearance}: {batches:?}");
        // The class draws the token; the inline style takes `token()`'s CSS.
        assert_eq!(images(&batches), [hero, brand], "{appearance}");
        // Another theme's gradient reaches what read the token and the
        // class; the token the new theme lacks is gone.
        let batches = until(&rx, |b| shows(b, "step 1"));
        let radial = "radial-gradient(ellipse farthest-corner at 50% 50%, #ffffffff, #000000ff)";
        assert!(
            batches.iter().any(|b| shows(b, &format!("hero {radial}"))),
            "{appearance}: {batches:?}"
        );
        assert!(
            batches.iter().any(|b| shows(b, "brand undefined")),
            "{appearance}: {batches:?}"
        );
        assert_eq!(images(&batches), [radial], "{appearance}");
    }
}
