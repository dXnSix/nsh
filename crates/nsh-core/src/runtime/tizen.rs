//! `nsh/tizen`: what only Samsung TVs have, for an app that branches on the
//! TV it runs on (`platform.os === "tizen"`, `platform.tv`). The TV's host
//! (`nsh-platform-tizen`) gives the module the TV's side ([`TizenHost`], as
//! [`TizenServices`] in the engine's context); everywhere else
//! `isAvailable()` is false and every other call rejects with
//! `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`, as it does for what a TV has no
//! public native API for (Smart Hub's preview, which a TV's web app has).
//!
//! - `productInfo()`: the TV's model, platform version, build and maker
//!   (`system_info`).
//! - `registerKey(key)` / `unregisterKey(key)`: the remote's keys the app
//!   takes from the TV (`keymap`'s names: `red`, `mediaplaypause`,
//!   `channelup`, `info`, ...; the arrows, Enter and Back always reach the
//!   app, Exit and Home never do).
//! - `exitApp()`: leaves the app.
//! - `setPreviewData(data)`: Smart Hub's preview tiles.

use std::sync::Arc;

use crate::{
    JsonValue,
    element::{JsError, JsObject, codes, engine_context, module},
    keymap::{Key, Keystroke},
};

/// What the TV's host gives `nsh/tizen`. Called on the engine's thread.
pub trait TizenHost: Send + Sync + 'static {
    /// The TV's product information.
    fn product(&self) -> TvProduct;

    /// Takes `key` from the TV for the app while it is shown; an error
    /// says why the TV keeps it.
    fn register_key(&self, key: Key) -> Result<(), String>;

    /// Gives `key` back to the TV; an error says why it cannot.
    fn unregister_key(&self, key: Key) -> Result<(), String>;
}

/// A TV's product information ([`TizenHost::product`]); `None` where the
/// TV does not say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TvProduct {
    /// The model's name (`system_info`'s `model_name`).
    pub model: Option<String>,
    /// The Tizen platform's version (`platform.version`: `8.0`).
    pub platform_version: Option<String>,
    /// The firmware's build (`build.string`).
    pub build: Option<String>,
    /// Who made it (`manufacturer`).
    pub manufacturer: Option<String>,
}

impl TvProduct {
    /// Product information with these values.
    pub fn new(
        model: Option<String>,
        platform_version: Option<String>,
        build: Option<String>,
        manufacturer: Option<String>,
    ) -> Self {
        Self {
            model,
            platform_version,
            build,
            manufacturer,
        }
    }
}

/// The TV's side of `nsh/tizen`, as a host puts it in the engine's context
/// (`EngineContext::with(TizenServices(host))`).
#[derive(Clone)]
pub struct TizenServices(pub Arc<dyn TizenHost>);

/// The TV's product information, as `productInfo()` gives it.
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
#[non_exhaustive]
pub(crate) struct ProductInfo {
    /// The model's name.
    pub(crate) model: Option<String>,
    /// The Tizen platform's version (`8.0`).
    pub(crate) platform_version: Option<String>,
    /// The firmware's build.
    pub(crate) build: Option<String>,
    /// Who made the TV.
    pub(crate) manufacturer: Option<String>,
}

/// The remote's keys an app may take from the TV: colors, media, channels
/// and Info.
const REGISTRABLE: &[Key] = &[
    Key::ColorRed,
    Key::ColorGreen,
    Key::ColorYellow,
    Key::ColorBlue,
    Key::MediaPlayPause,
    Key::MediaPlay,
    Key::MediaPause,
    Key::MediaStop,
    Key::MediaRewind,
    Key::MediaFastForward,
    Key::MediaNext,
    Key::MediaPrevious,
    Key::ChannelUp,
    Key::ChannelDown,
    Key::Info,
];

/// The TV's host, or why `call` cannot run here.
fn host(call: &str) -> Result<Arc<dyn TizenHost>, JsError> {
    engine_context::<TizenServices>()
        .map(|services| services.0.clone())
        .ok_or_else(|| unavailable(call, "it runs on a Samsung TV (Tizen) only"))
}

fn unavailable(call: &str, why: &str) -> JsError {
    JsError::Error(format!("nsh/tizen {call}: {why}")).with_code(codes::UNAVAILABLE)
}

/// The remote's key `name` names (`keymap`'s names), if an app may take it.
fn registrable(call: &str, name: &str) -> Result<Key, JsError> {
    let stroke = Keystroke::parse_with(name, true)
        .ok()
        .filter(|s| s.modifiers == Default::default())
        .ok_or_else(|| JsError::Type(format!("nsh/tizen {call}: {name:?} names no key")))?;
    if !REGISTRABLE.contains(&stroke.key) {
        return Err(JsError::Range(format!(
            "nsh/tizen {call}: {name:?} is no key the TV gives apps (the remote's colors, media, channel and info keys are)"
        )));
    }
    Ok(stroke.key)
}

/// Samsung TVs' own APIs.
#[module]
pub(crate) struct Tizen;

#[module(flat)]
impl Tizen {
    /// Whether the app runs on a Samsung TV: the other calls work only
    /// there (elsewhere they reject with
    /// `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`).
    pub(crate) fn is_available() -> bool {
        engine_context::<TizenServices>().is_some()
    }

    /// The TV's model, Tizen version, firmware build and maker (each
    /// `null` where the TV does not say).
    pub(crate) async fn product_info() -> Result<ProductInfo, JsError> {
        let product = host("productInfo")?.product();
        Ok(ProductInfo {
            model: product.model,
            platform_version: product.platform_version,
            build: product.build,
            manufacturer: product.manufacturer,
        })
    }

    /// Takes the remote's key `key` (`"red"`, `"mediaplaypause"`,
    /// `"channelup"`, `"info"`: `keymap`'s names) from the TV while the app
    /// is shown, so the app's key bindings hear it. Rejects with a
    /// `TypeError` for a name of no key and a `RangeError` for a key the TV
    /// does not give apps (the arrows, Enter and Back always reach the app;
    /// Exit and Home never do).
    pub(crate) async fn register_key(key: String) -> Result<(), JsError> {
        let host = host("registerKey")?;
        let key = registrable("registerKey", &key)?;
        host.register_key(key)
            .map_err(|why| unavailable("registerKey", &why))
    }

    /// Gives the remote's key `key` back to the TV. Rejects as
    /// `registerKey` does.
    pub(crate) async fn unregister_key(key: String) -> Result<(), JsError> {
        let host = host("unregisterKey")?;
        let key = registrable("unregisterKey", &key)?;
        host.unregister_key(key)
            .map_err(|why| unavailable("unregisterKey", &why))
    }

    /// Leaves the app (Samsung's rule: after asking, from its first
    /// screen).
    pub(crate) async fn exit_app() -> Result<(), JsError> {
        host("exitApp")?;
        crate::element::exit_app();
        Ok(())
    }

    /// Sets the app's Smart Hub preview (the tiles the TV shows for the app
    /// on its home screen) to `data`, in Samsung's preview format. The TV
    /// takes it from web apps (`tizen-web`); natively it rejects with
    /// `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`.
    pub(crate) async fn set_preview_data(data: JsonValue) -> Result<(), JsError> {
        host("setPreviewData")?;
        let _ = data;
        Err(unavailable(
            "setPreviewData",
            "Smart Hub's preview has no public native API; a TV's web app (tizen-web) sets it",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::EngineContext;

    struct Tv;

    impl TizenHost for Tv {
        fn product(&self) -> TvProduct {
            TvProduct::new(
                Some("QN65S95D".into()),
                Some("8.0".into()),
                None,
                Some("Samsung".into()),
            )
        }

        fn register_key(&self, key: Key) -> Result<(), String> {
            if key == Key::Info {
                Err("the TV keeps Info".into())
            } else {
                Ok(())
            }
        }

        fn unregister_key(&self, _key: Key) -> Result<(), String> {
            Ok(())
        }
    }

    fn on_tv<R>(f: impl FnOnce() -> R) -> R {
        let context = EngineContext::default().with(TizenServices(Arc::new(Tv)));
        let _guard = context.enter();
        f()
    }

    fn code(error: &JsError) -> Option<&str> {
        match error {
            JsError::Fields(fields) => fields
                .fields
                .iter()
                .find(|(key, _)| *key == "code")
                .and_then(|(_, value)| value.as_str()),
            _ => None,
        }
    }

    /// The outcome of a call that needs no turn of the engine.
    fn ready<T>(future: impl std::future::Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("the call waited"),
        }
    }

    #[test]
    fn available_only_with_the_tvs_host() {
        assert!(!Tizen::is_available());
        assert!(on_tv(Tizen::is_available));
    }

    #[test]
    fn elsewhere_every_call_rejects_as_unavailable() {
        let errors = [
            ready(Tizen::product_info()).unwrap_err(),
            ready(Tizen::register_key("red".into())).unwrap_err(),
            ready(Tizen::unregister_key("red".into())).unwrap_err(),
            ready(Tizen::exit_app()).unwrap_err(),
            ready(Tizen::set_preview_data(JsonValue::Null)).unwrap_err(),
        ];
        for error in errors {
            assert_eq!(code(&error), Some(codes::UNAVAILABLE), "{error:?}");
        }
    }

    #[test]
    fn product_info_is_the_tvs() {
        let info = on_tv(|| ready(Tizen::product_info())).unwrap();
        assert_eq!(info.model.as_deref(), Some("QN65S95D"));
        assert_eq!(info.platform_version.as_deref(), Some("8.0"));
        assert_eq!(info.build, None);
        assert_eq!(info.manufacturer.as_deref(), Some("Samsung"));
    }

    #[test]
    fn keys_are_checked_before_the_tv_hears_them() {
        on_tv(|| {
            for name in [
                "red",
                "ColorF0Red",
                "mediaplaypause",
                "channelup",
                "medianext",
            ] {
                assert!(ready(Tizen::register_key(name.into())).is_ok(), "{name}");
                assert!(ready(Tizen::unregister_key(name.into())).is_ok(), "{name}");
            }
            // No key.
            for name in ["", "reddish", "ctrl+red"] {
                let error = ready(Tizen::register_key(name.into())).unwrap_err();
                assert!(matches!(error, JsError::Type(_)), "{name}: {error:?}");
            }
            // Keys the TV does not give apps.
            for name in ["enter", "left", "escape", "a", "f5"] {
                let error = ready(Tizen::register_key(name.into())).unwrap_err();
                assert!(matches!(error, JsError::Range(_)), "{name}: {error:?}");
            }
            // The TV's answer.
            let error = ready(Tizen::register_key("info".into())).unwrap_err();
            assert_eq!(code(&error), Some(codes::UNAVAILABLE));
        });
    }

    #[test]
    fn smart_hub_preview_is_the_web_apps() {
        let error = on_tv(|| ready(Tizen::set_preview_data(JsonValue::Null))).unwrap_err();
        assert_eq!(code(&error), Some(codes::UNAVAILABLE));
    }
}
