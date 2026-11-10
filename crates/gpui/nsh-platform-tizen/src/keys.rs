//! The TV remote's keys: the names the Tizen platform gives GPUI
//! (dXn.GpuiTizen's design `d14-keys`: `dxn_gpui_tizen::keys::NAMED_KEYS`)
//! as NSH reads them. `AppView` maps a name to an NSH key itself
//! ([`nsh_platform_gpui::nsh_key`]: `red` is `ColorRed`, `info` `Info`);
//! `back` is NSH's back navigation, and `exit` is the TV's own (it closes
//! the app; no app hears it).

use nsh_core::keymap::Key;

/// What a key name of the Tizen platform is to NSH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Remote {
    /// An NSH key: the app's bindings and controls hear it.
    Key(Key),
    /// Back: NSH's back navigation (a modal, a stack, `onBackPress`), then
    /// the question whether to leave the app ([`crate::back`]).
    Back,
    /// The TV's own key (`exit`): the TV closes the app.
    Tv,
}

/// The GPUI name of the TV's Exit key.
const EXIT: &str = "exit";
/// The GPUI name of the remote's Back key.
const BACK: &str = "back";

/// What GPUI key name `name` (a `Keystroke`'s `key`) is to NSH; `None` for
/// a key NSH does not use.
pub(crate) fn remote(name: &str) -> Option<Remote> {
    match name {
        BACK => Some(Remote::Back),
        EXIT => Some(Remote::Tv),
        _ => nsh_platform_gpui::nsh_key(name).map(Remote::Key),
    }
}

#[cfg(test)]
mod tests {
    use dxn_gpui_tizen::{keygrab::DEFAULT_GRABS, keys::NAMED_KEYS};

    use super::*;

    /// Every name the platform gives GPUI means something to NSH.
    #[test]
    fn every_name_of_the_platform_is_an_nsh_key_back_or_the_tvs() {
        for (tizen, gpui) in NAMED_KEYS {
            assert!(remote(gpui).is_some(), "{tizen} ({gpui})");
        }
        assert_eq!(remote("back"), Some(Remote::Back));
        assert_eq!(remote("exit"), Some(Remote::Tv));
        assert_eq!(remote("enter"), Some(Remote::Key(Key::Enter)));
        assert_eq!(remote("left"), Some(Remote::Key(Key::Left)));
        assert_eq!(remote("backspace"), Some(Remote::Key(Key::Backspace)));
        // Printable keys, by their character.
        assert_eq!(remote("a"), Some(Remote::Key(Key::Char('a'))));
        // A name NSH has no key for.
        assert_eq!(remote("xf86menu"), None);
    }

    /// The remote's TV keys are NSH's TV keys (`TV-1`).
    #[test]
    fn the_remotes_keys_are_nshs_tv_keys() {
        let keys = [
            ("red", Key::ColorRed),
            ("green", Key::ColorGreen),
            ("yellow", Key::ColorYellow),
            ("blue", Key::ColorBlue),
            ("mediaplay", Key::MediaPlay),
            ("mediapause", Key::MediaPause),
            ("mediaplaypause", Key::MediaPlayPause),
            ("mediastop", Key::MediaStop),
            ("mediarewind", Key::MediaRewind),
            ("mediafastforward", Key::MediaFastForward),
            ("medianext", Key::MediaNext),
            ("mediaprevious", Key::MediaPrevious),
            ("channelup", Key::ChannelUp),
            ("channeldown", Key::ChannelDown),
            ("info", Key::Info),
        ];
        for (name, key) in keys {
            assert_eq!(remote(name), Some(Remote::Key(key)), "{name}");
        }
    }

    /// The keys the platform grabs for the app (colors, media, channels)
    /// are NSH keys.
    #[test]
    fn grabbed_keys_are_nsh_keys() {
        for tizen in DEFAULT_GRABS {
            let (_, gpui) = NAMED_KEYS.iter().find(|(t, _)| t == tizen).unwrap();
            assert!(
                matches!(remote(gpui), Some(Remote::Key(_))),
                "{tizen} is no NSH key"
            );
        }
    }
}
