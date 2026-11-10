//! The TV's side of `nsh/tizen` (`nsh_core::runtime::TizenHost`): the
//! product information `system_info` reports (`dxn_gpui_tizen::system_info`)
//! and the remote's keys the platform grabs for the app
//! (`dxn_gpui_tizen::keygrab`).
//!
//! The platform grabs the color, media and channel keys from the start
//! (`keygrab::DEFAULT_GRABS`) and the Info key when asked
//! (`keygrab::ON_REQUEST`); `registerKey` and `unregisterKey` grab and
//! release them while the app runs, the grabs following pause and resume.
//! The engine calls the host on its thread, and grabs are the main
//! thread's: a key is checked at once (`keygrab::validate`), then grabbed or
//! released on the main thread ([`serve`]).

// Only the TV's host makes one; its logic is tested on the desktops.
#![cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]

use dxn_gpui_tizen::{keygrab, system_info};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt as _;
use nsh_core::{
    keymap::Key,
    runtime::{TizenHost, TvProduct},
};

use crate::keys::tizen_name;

/// A change of the grabs, for the main thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grab {
    /// Grab the key (its Tizen name).
    Take(&'static str),
    /// Give it back to the TV.
    Release(&'static str),
}

/// The Tizen name of `key` when the platform can grab it, else why not.
pub(crate) fn grabbable(key: Key) -> Result<&'static str, String> {
    let name =
        tizen_name(key).ok_or_else(|| format!("the Tizen platform names no key for {key:?}"))?;
    let plan = keygrab::validate([name]);
    match plan.refused.first() {
        Some((_, refusal)) => Err(format!(
            "the Tizen platform does not grab {name} ({refusal:?})"
        )),
        None => Ok(name),
    }
}

/// The TV's side of `nsh/tizen`.
pub(crate) struct Tv {
    product: TvProduct,
    grabs: UnboundedSender<Grab>,
}

impl Tv {
    /// The TV whose product information is `product` (read once: it does
    /// not change while the app runs), and what its grabs go to.
    pub(crate) fn new(product: TvProduct) -> (Tv, UnboundedReceiver<Grab>) {
        let (grabs, receiver) = unbounded();
        (Tv { product, grabs }, receiver)
    }

    fn send(&self, grab: Grab) -> Result<(), String> {
        self.grabs
            .unbounded_send(grab)
            .map_err(|_| "the app is closing".to_owned())
    }
}

impl TizenHost for Tv {
    fn product(&self) -> TvProduct {
        self.product.clone()
    }

    fn register_key(&self, key: Key) -> Result<(), String> {
        self.send(Grab::Take(grabbable(key)?))
    }

    fn unregister_key(&self, key: Key) -> Result<(), String> {
        self.send(Grab::Release(grabbable(key)?))
    }
}

/// Grabs and releases what `grabs` asks for, on the main thread, while the
/// app runs.
pub(crate) fn serve(mut grabs: UnboundedReceiver<Grab>, cx: &mut gpui::App) {
    cx.spawn(async move |_| {
        while let Some(grab) = grabs.next().await {
            let refused = match grab {
                Grab::Take(name) => keygrab::grab([name]),
                Grab::Release(name) => keygrab::release([name]),
            };
            for (name, refusal) in refused {
                log::warn!("nsh/tizen: the platform refused {name}: {refusal:?}");
            }
        }
    })
    .detach();
}

/// The TV's product information (`system_info`; every field `None` off
/// Tizen).
pub(crate) fn product() -> TvProduct {
    let info = system_info::product_info();
    TvProduct::new(
        info.model_name,
        info.platform_version,
        info.build_string,
        info.manufacturer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_remotes_keys_are_grabbable_others_not() {
        for (key, name) in [
            (Key::ColorRed, "XF86Red"),
            (Key::ColorBlue, "XF86Blue"),
            (Key::MediaPlayPause, "XF86PlayBack"),
            (Key::MediaFastForward, "XF86AudioForward"),
            (Key::ChannelUp, "XF86RaiseChannel"),
            // On request.
            (Key::Info, "XF86Info"),
        ] {
            assert_eq!(grabbable(key), Ok(name), "{key:?}");
        }
        // Return is never grabbed; F5 has no Tizen name.
        assert!(grabbable(Key::Enter).unwrap_err().contains("Return"));
        assert!(grabbable(Key::F(5)).unwrap_err().contains("names no key"));
    }

    #[test]
    fn registering_sends_the_grab_to_the_main_thread() {
        let (tv, mut grabs) = Tv::new(TvProduct::default());
        assert_eq!(tv.register_key(Key::Info), Ok(()));
        assert_eq!(tv.unregister_key(Key::ColorRed), Ok(()));
        // Refused at once, nothing sent.
        assert!(tv.register_key(Key::Enter).is_err());
        assert_eq!(grabs.try_recv().ok(), Some(Grab::Take("XF86Info")));
        assert_eq!(grabs.try_recv().ok(), Some(Grab::Release("XF86Red")));
        // Nothing more waits.
        assert!(grabs.try_recv().is_err());
        // Once the app closed, nothing more is taken.
        drop(grabs);
        assert!(tv.register_key(Key::ColorGreen).is_err());
    }

    #[test]
    fn the_tvs_product_information() {
        let product = TvProduct::new(Some("QN65S95D".into()), Some("8.0".into()), None, None);
        let (tv, _grabs) = Tv::new(product);
        assert_eq!(tv.product().model.as_deref(), Some("QN65S95D"));
        assert_eq!(tv.product().platform_version.as_deref(), Some("8.0"));
        // Off Tizen the platform reports nothing.
        assert_eq!(super::product(), TvProduct::default());
    }
}
