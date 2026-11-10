//! A TV remote's keys, as `nsh/test`'s `app.keyboard.remote(<name>)`
//! names them ([`RemoteKey`]): the keys a remote has and apps hear, on
//! every platform's test agent and headless.
//!
//! They are the remote's navigation keys (the arrows, OK, Back) and the
//! media and TV keys `nsh_core::keymap` names ([`Key::MediaPlayPause`] to
//! [`Key::Info`]), by the names `keymap.bind` takes. The number keys, the
//! volume keys and Exit are not among them: Samsung's Smart Remote has no
//! number keys, and the system keeps the volume and Exit to itself.

use nsh_core::keymap::{Key, Keystroke, Modifiers};

/// A key of a TV remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteKey {
    /// A key the app's keymap hears as a hardware keyboard's key of the
    /// same name: an arrow, OK (Enter), a media or TV key.
    Key(Key),
    /// Back: the platform's going back (`app.back()`), which the app hears
    /// as its back press.
    Back,
}

/// Every remote key by name: the canonical name first, its aliases after
/// it (`ok` is Enter, as remotes label it).
const NAMES: &[(&str, RemoteKey)] = &[
    ("up", RemoteKey::Key(Key::Up)),
    ("down", RemoteKey::Key(Key::Down)),
    ("left", RemoteKey::Key(Key::Left)),
    ("right", RemoteKey::Key(Key::Right)),
    ("enter", RemoteKey::Key(Key::Enter)),
    ("ok", RemoteKey::Key(Key::Enter)),
    ("back", RemoteKey::Back),
    ("mediaplaypause", RemoteKey::Key(Key::MediaPlayPause)),
    ("mediaplay", RemoteKey::Key(Key::MediaPlay)),
    ("mediapause", RemoteKey::Key(Key::MediaPause)),
    ("mediastop", RemoteKey::Key(Key::MediaStop)),
    ("mediarewind", RemoteKey::Key(Key::MediaRewind)),
    ("mediafastforward", RemoteKey::Key(Key::MediaFastForward)),
    ("medianext", RemoteKey::Key(Key::MediaNext)),
    ("mediaprevious", RemoteKey::Key(Key::MediaPrevious)),
    ("red", RemoteKey::Key(Key::ColorRed)),
    ("green", RemoteKey::Key(Key::ColorGreen)),
    ("yellow", RemoteKey::Key(Key::ColorYellow)),
    ("blue", RemoteKey::Key(Key::ColorBlue)),
    ("channelup", RemoteKey::Key(Key::ChannelUp)),
    ("channeldown", RemoteKey::Key(Key::ChannelDown)),
    ("info", RemoteKey::Key(Key::Info)),
];

impl RemoteKey {
    /// The remote key named `name` (case does not matter), or an error
    /// that lists the names there are.
    pub fn parse(name: &str) -> Result<RemoteKey, String> {
        let lower = name.trim().to_ascii_lowercase();
        NAMES
            .iter()
            .find(|(n, _)| *n == lower)
            .map(|&(_, key)| key)
            .ok_or_else(|| {
                format!(
                    "{name:?} is no TV remote key: a remote's keys are {}",
                    RemoteKey::names()
                )
            })
    }

    /// The names [`RemoteKey::parse`] takes, joined by commas.
    pub fn names() -> String {
        NAMES
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Its keystroke (no modifiers: a remote has none); `None` for
    /// [`RemoteKey::Back`], which is no key of `nsh_core::keymap`.
    pub fn keystroke(self) -> Option<Keystroke> {
        match self {
            RemoteKey::Key(key) => Some(Keystroke {
                modifiers: Modifiers::default(),
                key,
            }),
            RemoteKey::Back => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name parses, whatever its case, and its keystroke is the
    /// keymap's key of that name.
    #[test]
    fn names_parse_to_the_keymap_s_keys() {
        for &(name, key) in NAMES {
            assert_eq!(RemoteKey::parse(name), Ok(key), "{name}");
            assert_eq!(
                RemoteKey::parse(&name.to_ascii_uppercase()),
                Ok(key),
                "{name}"
            );
            if let Some(stroke) = key.keystroke() {
                let named = Keystroke::parse(&stroke.to_string(), nsh_core::platform::Os::Linux);
                assert_eq!(named, Ok(stroke), "{name}");
            }
        }
        assert_eq!(RemoteKey::parse("ok"), RemoteKey::parse("enter"));
        assert_eq!(RemoteKey::parse("back"), Ok(RemoteKey::Back));
    }

    /// Every media and TV key of the keymap is a remote key.
    #[test]
    fn every_media_and_tv_key_is_one() {
        let keys: Vec<Key> = NAMES
            .iter()
            .filter_map(|(_, k)| k.keystroke())
            .map(|s| s.key)
            .filter(|k| k.is_media_or_remote())
            .collect();
        assert_eq!(keys.len(), 15);
    }

    /// An unknown name fails with the names there are.
    #[test]
    fn unknown_names_list_the_keys() {
        let error = RemoteKey::parse("power").unwrap_err();
        assert!(
            error.starts_with("\"power\" is no TV remote key"),
            "{error}"
        );
        assert!(
            error.ends_with(
                "up, down, left, right, enter, ok, back, mediaplaypause, mediaplay, \
                 mediapause, mediastop, mediarewind, mediafastforward, medianext, mediaprevious, \
                 red, green, yellow, blue, channelup, channeldown, info"
            ),
            "{error}"
        );
        // Keys of a keyboard that a remote has not.
        for name in ["escape", "tab", "mod+k", "1", "f1", "colorf0red"] {
            assert!(RemoteKey::parse(name).is_err(), "{name}");
        }
    }
}
