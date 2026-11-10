//! Keys: GPUI's keystrokes as the virtual key codes ([`crate::input::KeyEvent::vk`],
//! Windows' `VK_*` values) the input, scrolling, keymap and editing code
//! shared with the Windows backend work with.
//!
//! The media and TV remote keys come with the names the Tizen platform gives
//! GPUI (`mediaplaypause`, `red`, `channelup`, `info`, ...). Windows has
//! virtual keys for four of them (`VK_MEDIA_*`); the others get codes of
//! this backend's own past Windows' 0xFF ([`TV_KEYS`]), which no Windows key
//! has, so the code shared with Windows never mistakes them for one.

use nsh_core::keymap::Key;

/// `VK_BACK`.
pub(crate) const VK_BACK: u16 = 0x08;
/// `VK_TAB`.
pub(crate) const VK_TAB: u16 = 0x09;
/// `VK_RETURN`.
pub(crate) const VK_RETURN: u16 = 0x0D;
/// `VK_SHIFT`.
pub(crate) const VK_SHIFT: u16 = 0x10;
/// `VK_CONTROL`.
pub(crate) const VK_CONTROL: u16 = 0x11;
/// `VK_MENU` (Alt).
pub(crate) const VK_MENU: u16 = 0x12;
/// `VK_ESCAPE`.
pub(crate) const VK_ESCAPE: u16 = 0x1B;
/// `VK_SPACE`.
pub(crate) const VK_SPACE: u16 = 0x20;
/// `VK_PRIOR` (Page Up).
pub(crate) const VK_PRIOR: u16 = 0x21;
/// `VK_NEXT` (Page Down).
pub(crate) const VK_NEXT: u16 = 0x22;
/// `VK_END`.
pub(crate) const VK_END: u16 = 0x23;
/// `VK_HOME`.
pub(crate) const VK_HOME: u16 = 0x24;
/// `VK_LEFT`.
pub(crate) const VK_LEFT: u16 = 0x25;
/// `VK_UP`.
pub(crate) const VK_UP: u16 = 0x26;
/// `VK_RIGHT`.
pub(crate) const VK_RIGHT: u16 = 0x27;
/// `VK_DOWN`.
pub(crate) const VK_DOWN: u16 = 0x28;
/// `VK_INSERT`.
pub(crate) const VK_INSERT: u16 = 0x2D;
/// `VK_DELETE`.
pub(crate) const VK_DELETE: u16 = 0x2E;
/// `VK_LWIN` (the Windows / Command / Super key).
pub(crate) const VK_LWIN: u16 = 0x5B;
/// `VK_BROWSER_BACK`.
pub(crate) const VK_BROWSER_BACK: u16 = 0xA6;
/// `VK_MEDIA_NEXT_TRACK`.
pub(crate) const VK_MEDIA_NEXT_TRACK: u16 = 0xB0;
/// `VK_MEDIA_PREV_TRACK`.
pub(crate) const VK_MEDIA_PREV_TRACK: u16 = 0xB1;
/// `VK_MEDIA_STOP`.
pub(crate) const VK_MEDIA_STOP: u16 = 0xB2;
/// `VK_MEDIA_PLAY_PAUSE`.
pub(crate) const VK_MEDIA_PLAY_PAUSE: u16 = 0xB3;

/// The TV remote's keys Windows has no virtual key for: their GPUI names,
/// the codes this backend gives them (past 0xFF) and their keys.
pub(crate) const TV_KEYS: &[(&str, u16, Key)] = &[
    ("mediaplay", 0x100, Key::MediaPlay),
    ("mediapause", 0x101, Key::MediaPause),
    ("mediarewind", 0x102, Key::MediaRewind),
    ("mediafastforward", 0x103, Key::MediaFastForward),
    ("red", 0x104, Key::ColorRed),
    ("green", 0x105, Key::ColorGreen),
    ("yellow", 0x106, Key::ColorYellow),
    ("blue", 0x107, Key::ColorBlue),
    ("channelup", 0x108, Key::ChannelUp),
    ("channeldown", 0x109, Key::ChannelDown),
    ("info", 0x10A, Key::Info),
];

/// The key of virtual key code `vk`: Windows' ([`Key::from_virtual_key`])
/// or one of [`TV_KEYS`].
pub(crate) fn key_of(vk: u16) -> Option<Key> {
    Key::from_virtual_key(vk).or_else(|| {
        TV_KEYS
            .iter()
            .find(|(_, code, _)| *code == vk)
            .map(|&(_, _, key)| key)
    })
}

/// The virtual key code of `key`, as [`key_of`] takes it.
pub(crate) fn virtual_key(key: Key) -> Option<u16> {
    key.virtual_key().or_else(|| {
        TV_KEYS
            .iter()
            .find(|(_, _, k)| *k == key)
            .map(|&(_, code, _)| code)
    })
}

/// The virtual key code of GPUI key name `key` (`Keystroke::key`), or
/// `None` for keys nsh does not use.
pub(crate) fn vk_of(key: &str) -> Option<u16> {
    Some(match key {
        "backspace" => VK_BACK,
        "tab" => VK_TAB,
        "enter" => VK_RETURN,
        "shift" => VK_SHIFT,
        "control" => VK_CONTROL,
        "alt" => VK_MENU,
        "platform" | "cmd" | "super" | "win" => VK_LWIN,
        "escape" => VK_ESCAPE,
        "space" => VK_SPACE,
        "pageup" => VK_PRIOR,
        "pagedown" => VK_NEXT,
        "end" => VK_END,
        "home" => VK_HOME,
        "left" => VK_LEFT,
        "up" => VK_UP,
        "right" => VK_RIGHT,
        "down" => VK_DOWN,
        "insert" => VK_INSERT,
        "delete" => VK_DELETE,
        "back" => VK_BROWSER_BACK,
        "medianext" => VK_MEDIA_NEXT_TRACK,
        "mediaprevious" => VK_MEDIA_PREV_TRACK,
        "mediastop" => VK_MEDIA_STOP,
        "mediaplaypause" => VK_MEDIA_PLAY_PAUSE,
        // Ecore's name of Info, which the Tizen platform may pass on.
        "xf86info" => virtual_key(Key::Info)?,
        ";" => 0xBA,
        "=" => 0xBB,
        "," => 0xBC,
        "-" => 0xBD,
        "." => 0xBE,
        "/" => 0xBF,
        "`" => 0xC0,
        "[" => 0xDB,
        "\\" => 0xDC,
        "]" => 0xDD,
        "'" => 0xDE,
        _ => {
            if let Some(&(_, code, _)) = TV_KEYS.iter().find(|(name, _, _)| *name == key) {
                return Some(code);
            }
            if let Some(n) = key.strip_prefix('f').and_then(|n| n.parse::<u16>().ok())
                && (1..=24).contains(&n)
            {
                return Some(0x6F + n);
            }
            let mut chars = key.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            match c {
                '0'..='9' => c as u16,
                'a'..='z' => c.to_ascii_uppercase() as u16,
                'A'..='Z' => c as u16,
                _ => return None,
            }
        }
    })
}

/// GPUI's modifiers as the input code's.
pub(crate) fn modifiers(m: &gpui::Modifiers) -> crate::input::Modifiers {
    crate::input::Modifiers {
        shift: m.shift,
        ctrl: m.control,
        alt: m.alt,
        meta: m.platform,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(vk_of("a"), Some(0x41));
        assert_eq!(vk_of("7"), Some(0x37));
        assert_eq!(vk_of("f5"), Some(0x74));
        assert_eq!(vk_of("enter"), Some(VK_RETURN));
        assert_eq!(vk_of("/"), Some(0xBF));
        assert_eq!(vk_of("é"), None);
    }

    /// The media and TV remote keys by the Tizen platform's names
    /// (dXn.GpuiTizen's design `d14-keys`).
    mod tv {
        use nsh_core::keymap::Key;

        use super::super::*;

        #[test]
        fn names() {
            let names = [
                ("mediaplaypause", Key::MediaPlayPause),
                ("mediaplay", Key::MediaPlay),
                ("mediapause", Key::MediaPause),
                ("mediastop", Key::MediaStop),
                ("mediarewind", Key::MediaRewind),
                ("mediafastforward", Key::MediaFastForward),
                ("medianext", Key::MediaNext),
                ("mediaprevious", Key::MediaPrevious),
                ("red", Key::ColorRed),
                ("green", Key::ColorGreen),
                ("yellow", Key::ColorYellow),
                ("blue", Key::ColorBlue),
                ("channelup", Key::ChannelUp),
                ("channeldown", Key::ChannelDown),
                ("info", Key::Info),
                ("xf86info", Key::Info),
            ];
            for (name, key) in names {
                let vk = vk_of(name).unwrap_or_else(|| panic!("{name}"));
                assert_eq!(key_of(vk), Some(key), "{name}");
                assert_eq!(key_of(virtual_key(key).unwrap()), Some(key), "{key:?}");
            }
            // Windows' own media keys keep their codes.
            assert_eq!(vk_of("mediaplaypause"), Some(0xB3));
            assert_eq!(vk_of("medianext"), Some(0xB0));
            // Back is the browser's back key, which navigation takes; Exit
            // is no key of NSH's.
            assert_eq!(vk_of("back"), Some(VK_BROWSER_BACK));
            assert_eq!(vk_of("exit"), None);
        }

        #[test]
        fn codes_of_their_own_are_no_windows_keys() {
            for &(_, code, _) in TV_KEYS {
                assert!(code > 0xFF);
                assert_eq!(Key::from_virtual_key(code), None);
            }
        }
    }
}
