//! End to end: a design system's UI set and the app's each register a theme
//! type. Each reads its own `theme_of`; the app's theme file and
//! `update_theme` change them; `nsh-base`'s `theme()` draws through the roles
//! the types map (`popover` is the design system's `surface-overlay`); an
//! embedding app's override by a role's name recolors the token it aliases.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, Color, MountBatch, MountItem, Size,
    element::{AdaptiveColor, AppTheme, Registry, Role, Root, TokenKind, TokenSpec, Tokens},
    prelude::*,
};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::ChannelSink;

#[derive(Debug, Clone, PartialEq)]
struct Ds {
    overlay: AdaptiveColor,
    action: AdaptiveColor,
}

impl Default for Ds {
    fn default() -> Self {
        Ds {
            overlay: AdaptiveColor::rgb(0x111111, 0x111111),
            action: AdaptiveColor::rgb(0x222222, 0x222222),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct DsPatch {
    action: Option<AdaptiveColor>,
}

impl AppTheme for Ds {
    type Patch = DsPatch;

    fn token_specs() -> &'static [TokenSpec] {
        static SPECS: std::sync::LazyLock<Vec<TokenSpec>> = std::sync::LazyLock::new(|| {
            vec![
                TokenSpec::new("surface-overlay", TokenKind::Color).role(Role::Popover),
                TokenSpec::new("action", TokenKind::Color).role(Role::Primary),
            ]
        });
        &SPECS
    }

    fn write_tokens(&self, prefix: &str, out: &mut Tokens) {
        out.set_color(&format!("{prefix}surface-overlay"), self.overlay);
        out.set_color(&format!("{prefix}action"), self.action);
    }

    fn read_tokens(tokens: &Tokens, prefix: &str) -> Self {
        let d = Ds::default();
        Ds {
            overlay: tokens
                .color(&format!("{prefix}surface-overlay"))
                .unwrap_or(d.overlay),
            action: tokens.color(&format!("{prefix}action")).unwrap_or(d.action),
        }
    }

    fn write_patch(patch: &DsPatch, prefix: &str, out: &mut Tokens) {
        if let Some(c) = patch.action {
            out.set_color(&format!("{prefix}action"), c);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Odds {
    up: AdaptiveColor,
}

impl Default for Odds {
    fn default() -> Self {
        Odds {
            up: AdaptiveColor::rgb(0x00aa00, 0x00aa00),
        }
    }
}

impl AppTheme for Odds {
    type Patch = ();

    fn token_specs() -> &'static [TokenSpec] {
        static SPECS: std::sync::LazyLock<Vec<TokenSpec>> =
            std::sync::LazyLock::new(|| vec![TokenSpec::new("odds-up", TokenKind::Color)]);
        &SPECS
    }

    fn write_tokens(&self, prefix: &str, out: &mut Tokens) {
        out.set_color(&format!("{prefix}odds-up"), self.up);
    }

    fn read_tokens(tokens: &Tokens, prefix: &str) -> Self {
        Odds {
            up: tokens
                .color(&format!("{prefix}odds-up"))
                .unwrap_or(Odds::default().up),
        }
    }

    fn write_patch(_: &(), _: &str, _: &mut Tokens) {}
}

fn hex(c: AdaptiveColor) -> String {
    format!("{:06x}", Color::from(c.light).0 >> 8)
}

/// Shows each type's theme and `nsh-base`'s, then changes the design
/// system's `action` with `update_theme`.
#[component]
fn Shows() -> impl IntoElement {
    set_timeout(Duration::from_millis(100), || {
        update_theme::<Ds>(DsPatch {
            action: Some(AdaptiveColor::rgb(0x0000ff, 0x0000ff)),
        })
    });
    text(move || {
        let (ds, odds, base) = (theme_of::<Ds>(), theme_of::<Odds>(), nsh_base::theme());
        format!(
            "overlay {} action {} up {} popover {} primary {}",
            hex(ds.overlay),
            hex(ds.action),
            hex(odds.up),
            hex(base.popover),
            hex(base.primary)
        )
    })
}

fn start(host: Option<&str>) -> (Engine, Receiver<MountBatch>) {
    let mut ds = Registry::ui_set("ds/native");
    ds.theme::<Ds>();
    let mut app = Registry::ui_set("app/native");
    app.theme::<Odds>();
    let root = Root::new::<Shows>(|| ())
        .theme(r##"{ "cssVars": { "light": { "odds-up": "#00ff00", "surface-overlay": "#333333", "primary": "#444444" } } }"##);
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        app: App::Rust(root),
        viewport: Size::new(px(390.0), px(844.0)),
        trace: false,
        content_culling: false,
        network: None,
        websocket: None,
        refuse_connections: None,
        filesystem: None,
        assets: None,
        environment: Environment::default(),
        header_heights: (nsh_core::Points(44.0), nsh_core::Points(96.0)),
        limits: nsh_engine::Limits::default(),
        ui_sets: vec![nsh_base::registry(), ds, app],
        runtime: None,
        data_dir: None,
        cache: nsh_engine::CacheOptions::off(),
        storage: None,
        clock: Default::default(),
        context: Default::default(),
        pinned: Default::default(),
        clipboard: None,
        host: Default::default(),
        picker: None,
        speech: None,
        audio: None,
        windows: None,
    };
    let host = host.map(str::to_owned);
    let engine = Engine::start_with(
        config,
        || ApproxTextEngine,
        ChannelSink(tx),
        move |control| {
            if let Some(json) = host {
                control.set_theme_overrides(json);
            }
        },
    )
    .unwrap();
    (engine, rx)
}

/// The texts the batches within a moment show, in order.
fn texts(rx: &Receiver<MountBatch>) -> Vec<String> {
    let mut seen = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(400)) {
        for item in batch.items {
            if let MountItem::Text { text, .. } = item {
                seen.push(text.text.to_string());
            }
        }
    }
    seen
}

#[test]
fn two_sets_types_read_their_own_and_base_draws_through_roles() {
    let (_engine, rx) = start(None);
    let seen = texts(&rx);
    assert_eq!(
        seen,
        [
            // The file over the types' defaults; `primary` is a role here,
            // so it sets the design system's `action`.
            "overlay 333333 action 444444 up 00ff00 popover 333333 primary 444444",
            // `update_theme` changes the field the patch sets.
            "overlay 333333 action 0000ff up 00ff00 popover 333333 primary 0000ff",
        ]
    );
}

#[test]
fn an_override_by_a_roles_name_recolors_the_token_it_aliases() {
    let (_engine, rx) = start(Some(
        r##"{ "cssVars": { "light": { "popover": "#abcdef", "brand": "#000000" } } }"##,
    ));
    let seen = texts(&rx);
    assert_eq!(
        seen.first().map(String::as_str),
        Some("overlay abcdef action 444444 up 00ff00 popover abcdef primary 444444"),
        "{seen:?}"
    );
}
