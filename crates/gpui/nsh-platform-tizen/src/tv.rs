//! The TV's side of `nsh/tizen` (`nsh_core::runtime::TizenHost`): the
//! product information `system_info` reports, and the remote's keys the
//! platform takes from the TV for the app.
//!
//! The Tizen platform grabs the remote's color, media and channel keys
//! (`dxn_gpui_tizen::keygrab::DEFAULT_GRABS`) for the app's whole time on
//! screen, so registering one of them succeeds at once. It grabs no other
//! key and changes no grab while the app runs: registering another key (Info)
//! or giving a grabbed one back to the TV is refused, saying so.

// Only the TV's host makes one; its logic is tested on the desktops.
#![cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]

use dxn_gpui_tizen::keygrab::DEFAULT_GRABS;
use nsh_core::{
    keymap::Key,
    runtime::{TizenHost, TvProduct},
};

use crate::keys::tizen_name;

/// What a grab of `key` is: `Ok` for a key the platform grabs, else why
/// the TV keeps it.
pub(crate) fn register(key: Key) -> Result<(), String> {
    match tizen_name(key) {
        Some(name) if DEFAULT_GRABS.contains(&name) => Ok(()),
        Some(name) => Err(format!(
            "the Tizen platform takes {name} from the TV only at start (RunOptions::grab_keys)"
        )),
        None => Err(format!("the Tizen platform names no key for {key:?}")),
    }
}

/// Giving `key` back to the TV while the app runs.
pub(crate) fn unregister(key: Key) -> Result<(), String> {
    match tizen_name(key) {
        Some(name) if DEFAULT_GRABS.contains(&name) => Err(format!(
            "the Tizen platform keeps {name} for the app while it is shown"
        )),
        // Never taken: the TV has it.
        Some(_) => Ok(()),
        None => Err(format!("the Tizen platform names no key for {key:?}")),
    }
}

/// The TV's side of `nsh/tizen`: product information from `product`.
pub(crate) struct Tv {
    product: TvProduct,
}

impl Tv {
    /// The TV whose product information is `product` (read once: it does
    /// not change while the app runs).
    pub(crate) fn new(product: TvProduct) -> Tv {
        Tv { product }
    }
}

impl TizenHost for Tv {
    fn product(&self) -> TvProduct {
        self.product.clone()
    }

    fn register_key(&self, key: Key) -> Result<(), String> {
        register(key)
    }

    fn unregister_key(&self, key: Key) -> Result<(), String> {
        unregister(key)
    }
}

/// The product information `system_info` reports (`system_info_get_
/// platform_string`), each key `None` where the TV does not say.
#[cfg(target_vendor = "tizen")]
pub(crate) fn product() -> TvProduct {
    use std::ffi::CString;

    let string = |key: &str| -> Option<String> {
        let key = CString::new(key).ok()?;
        let mut value: *mut std::ffi::c_char = std::ptr::null_mut();
        // SAFETY: a NUL-terminated key and an out pointer; on success the
        // value is a malloc'd string, which `malloced` frees.
        let code = unsafe {
            dxn_gpui_tizen_sys::system_info_get_platform_string(key.as_ptr(), &mut value)
        };
        if code != 0 {
            return None;
        }
        crate::tizen::malloced(value).filter(|v| !v.is_empty())
    };
    TvProduct::new(
        string("http://tizen.org/system/model_name"),
        string("http://tizen.org/feature/platform.version"),
        string("http://tizen.org/system/build.string"),
        string("http://tizen.org/system/manufacturer"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_the_platform_grabs_register_at_once() {
        for key in [
            Key::ColorRed,
            Key::ColorBlue,
            Key::MediaPlayPause,
            Key::MediaFastForward,
            Key::ChannelUp,
        ] {
            assert_eq!(register(key), Ok(()), "{key:?}");
            assert!(unregister(key).is_err(), "{key:?}");
        }
        // Info is not grabbed: it stays the TV's.
        assert!(register(Key::Info).unwrap_err().contains("XF86Info"));
        assert_eq!(unregister(Key::Info), Ok(()));
        // A key the platform has no name for.
        assert!(register(Key::F(5)).is_err());
    }

    #[test]
    fn the_tvs_product_information() {
        let tv = Tv::new(TvProduct::new(
            Some("QN65S95D".into()),
            Some("8.0".into()),
            None,
            None,
        ));
        let product = tv.product();
        assert_eq!(product.model.as_deref(), Some("QN65S95D"));
        assert_eq!(product.platform_version.as_deref(), Some("8.0"));
        assert_eq!(product.build, None);
    }
}
