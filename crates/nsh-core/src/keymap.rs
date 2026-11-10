//! App-level keyboard shortcuts (`nsh/keymap`): keystrokes parsed from
//! their text (`"mod+k"`, `"ctrl+shift+p"`, `"escape"`, `"f5"`) and the
//! table of a surface's bindings.
//!
//! The syntax is the one `nsh_core::desktop::Shortcut` (menus, `nsh/desktop`)
//! is moving to; the two will share this parser ([`Keystroke::parse`]).
//!
//! The main thread never waits for JS: JS sends the whole table to the
//! platform ([`crate::Prop::Keymap`] on the root, mounted as
//! [`crate::MountItem::Keymap`]), the platform matches each key against it
//! on the spot ([`Keymap::find`]) and consumes the keys it binds, then
//! tells JS which binding matched ([`crate::EventKind::KeyCommand`]).
//!
//! Keys are named by the character they type on the US layout without
//! Shift (`"/"`, `"["`, `"1"`); Shift is a modifier (`"shift+/"` is `?`).
//! Platforms match a key by the character it types without modifiers, and
//! on a layout that types no ASCII there (Cyrillic, Greek, ...) by the US
//! layout's character of the same key, as menus do, so shortcuts work on
//! any layout.
//!
//! Media keys and a TV remote's keys have names of their own
//! (`"mediaplaypause"`, `"red"`, `"channelup"`, `"info"`, ...: [`Key`]),
//! from Windows' virtual keys, Android's key codes, the web's
//! `KeyboardEvent.key` (and Samsung TVs' key codes) and the GPUI backend's
//! key names (the Tizen platform's), wherever the platform reports them.

use std::fmt;

use crate::platform::Os;

/// The modifiers of a keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    /// Control.
    pub ctrl: bool,
    /// Option on Apple platforms, Alt elsewhere.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// Command on Apple platforms; the Super (Windows) key elsewhere.
    pub cmd: bool,
}

/// A key, by what it is on the US layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A key typing a printable ASCII character without Shift: a lower
    /// case letter, a digit, punctuation, or `' '` (space).
    Char(char),
    /// Return / Enter (the keypad's too).
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Backspace (the Mac's Delete).
    Backspace,
    /// Forward delete.
    Delete,
    /// Insert (Help on Apple keyboards).
    Insert,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// A function key, F1 to F24.
    F(u8),
    /// Play / Pause, one key that toggles (media keyboards, TV remotes).
    MediaPlayPause,
    /// Play (TV remotes).
    MediaPlay,
    /// Pause (TV remotes).
    MediaPause,
    /// Stop.
    MediaStop,
    /// Rewind.
    MediaRewind,
    /// Fast forward.
    MediaFastForward,
    /// Next track.
    MediaNext,
    /// Previous track.
    MediaPrevious,
    /// The TV remote's red key (the web's `ColorF0Red`).
    ColorRed,
    /// The TV remote's green key (`ColorF1Green`).
    ColorGreen,
    /// The TV remote's yellow key (`ColorF2Yellow`).
    ColorYellow,
    /// The TV remote's blue key (`ColorF3Blue`).
    ColorBlue,
    /// Channel up.
    ChannelUp,
    /// Channel down.
    ChannelDown,
    /// Info (the TV remote's information key).
    Info,
}

/// The US layout's punctuation keys' Windows virtual-key codes
/// (`VK_OEM_*`).
const OEM_KEYS: &[(u16, char)] = &[
    (0xBA, ';'),
    (0xBB, '='),
    (0xBC, ','),
    (0xBD, '-'),
    (0xBE, '.'),
    (0xBF, '/'),
    (0xC0, '`'),
    (0xDB, '['),
    (0xDC, '\\'),
    (0xDD, ']'),
    (0xDE, '\''),
];

impl Key {
    /// The key of Windows virtual-key code `vk` (`VK_*`, which the Windows
    /// and GPUI backends get); punctuation by the US layout's keys.
    pub fn from_virtual_key(vk: u16) -> Option<Key> {
        Some(match vk {
            0x08 => Key::Backspace,
            0x09 => Key::Tab,
            0x0D => Key::Enter,
            0x1B => Key::Escape,
            0x20 => Key::Char(' '),
            0x21 => Key::PageUp,
            0x22 => Key::PageDown,
            0x23 => Key::End,
            0x24 => Key::Home,
            0x25 => Key::Left,
            0x26 => Key::Up,
            0x27 => Key::Right,
            0x28 => Key::Down,
            0x2D => Key::Insert,
            0x2E => Key::Delete,
            // `0`-`9`, `A`-`Z`.
            0x30..=0x39 => Key::Char(char::from(vk as u8)),
            0x41..=0x5A => Key::Char(char::from(vk as u8).to_ascii_lowercase()),
            // The keypad's digits.
            0x60..=0x69 => Key::Char(char::from(b'0' + (vk - 0x60) as u8)),
            // F1 to F24.
            0x70..=0x87 => Key::F((vk - 0x6F) as u8),
            // `VK_MEDIA_NEXT_TRACK`, `VK_MEDIA_PREV_TRACK`, `VK_MEDIA_STOP`,
            // `VK_MEDIA_PLAY_PAUSE`; Windows has no virtual keys for the TV
            // remote's other keys.
            0xB0 => Key::MediaNext,
            0xB1 => Key::MediaPrevious,
            0xB2 => Key::MediaStop,
            0xB3 => Key::MediaPlayPause,
            _ => {
                return OEM_KEYS
                    .iter()
                    .find(|(k, _)| *k == vk)
                    .map(|&(_, c)| Key::Char(c));
            }
        })
    }

    /// The Windows virtual-key code of this key (the main keyboard's, not
    /// the keypad's): what a key event for it carries.
    pub fn virtual_key(self) -> Option<u16> {
        Some(match self {
            Key::Backspace => 0x08,
            Key::Tab => 0x09,
            Key::Enter => 0x0D,
            Key::Escape => 0x1B,
            Key::Char(' ') => 0x20,
            Key::PageUp => 0x21,
            Key::PageDown => 0x22,
            Key::End => 0x23,
            Key::Home => 0x24,
            Key::Left => 0x25,
            Key::Up => 0x26,
            Key::Right => 0x27,
            Key::Down => 0x28,
            Key::Insert => 0x2D,
            Key::Delete => 0x2E,
            Key::Char(c @ '0'..='9') => c as u16,
            Key::Char(c @ 'a'..='z') => c.to_ascii_uppercase() as u16,
            Key::F(n @ 1..=24) => 0x6F + u16::from(n),
            Key::MediaNext => 0xB0,
            Key::MediaPrevious => 0xB1,
            Key::MediaStop => 0xB2,
            Key::MediaPlayPause => 0xB3,
            Key::Char(c) => return OEM_KEYS.iter().find(|(_, k)| *k == c).map(|&(vk, _)| vk),
            Key::F(_)
            | Key::MediaPlay
            | Key::MediaPause
            | Key::MediaRewind
            | Key::MediaFastForward
            | Key::ColorRed
            | Key::ColorGreen
            | Key::ColorYellow
            | Key::ColorBlue
            | Key::ChannelUp
            | Key::ChannelDown
            | Key::Info => return None,
        })
    }

    /// The key of USB HID keyboard usage `usage` (usage page 7, which
    /// UIKit's `UIKey.keyCode` is): the key's place on the US layout,
    /// whatever layout types on it; the keypad's digits and Enter too.
    pub fn from_hid_usage(usage: u16) -> Option<Key> {
        Some(match usage {
            4..=29 => Key::Char(char::from(b'a' + (usage - 4) as u8)),
            30..=38 => Key::Char(char::from(b'1' + (usage - 30) as u8)),
            39 => Key::Char('0'),
            40 | 88 => Key::Enter,
            41 => Key::Escape,
            42 => Key::Backspace,
            43 => Key::Tab,
            44 => Key::Char(' '),
            58..=69 => Key::F((usage - 57) as u8),
            73 => Key::Insert,
            74 => Key::Home,
            75 => Key::PageUp,
            76 => Key::Delete,
            77 => Key::End,
            78 => Key::PageDown,
            79 => Key::Right,
            80 => Key::Left,
            81 => Key::Down,
            82 => Key::Up,
            89..=97 => Key::Char(char::from(b'1' + (usage - 89) as u8)),
            98 => Key::Char('0'),
            104..=115 => Key::F((usage - 91) as u8),
            _ => {
                return HID_KEYS
                    .iter()
                    .find(|(u, _)| *u == usage)
                    .map(|&(_, c)| Key::Char(c));
            }
        })
    }

    /// The USB HID keyboard usage of this key (the main keyboard's, not
    /// the keypad's): what a hardware keyboard sends for it. The media and
    /// TV keys have none: they are consumer usages (page 12), not keyboard
    /// ones.
    pub fn hid_usage(self) -> Option<u16> {
        Some(match self {
            Key::Char(c @ 'a'..='z') => 4 + u16::from(c as u8 - b'a'),
            Key::Char(c @ '1'..='9') => 30 + u16::from(c as u8 - b'1'),
            Key::Char('0') => 39,
            Key::Enter => 40,
            Key::Escape => 41,
            Key::Backspace => 42,
            Key::Tab => 43,
            Key::Char(' ') => 44,
            Key::F(n @ 1..=12) => 57 + u16::from(n),
            Key::F(n @ 13..=24) => 91 + u16::from(n),
            Key::Insert => 73,
            Key::Home => 74,
            Key::PageUp => 75,
            Key::Delete => 76,
            Key::End => 77,
            Key::PageDown => 78,
            Key::Right => 79,
            Key::Left => 80,
            Key::Down => 81,
            Key::Up => 82,
            Key::Char(c) => return HID_KEYS.iter().find(|(_, k)| *k == c).map(|&(u, _)| u),
            _ => return None,
        })
    }

    /// The key of Android key code `code` (`KeyEvent.KEYCODE_*`); the
    /// keypad's digits and Enter too, and the media and TV remote keys
    /// (`KEYCODE_MEDIA_*`, `KEYCODE_PROG_*`, `KEYCODE_CHANNEL_*`,
    /// `KEYCODE_INFO`).
    pub fn from_android_keycode(code: i32) -> Option<Key> {
        Some(match code {
            7..=16 => Key::Char(char::from(b'0' + (code - 7) as u8)),
            29..=54 => Key::Char(char::from(b'a' + (code - 29) as u8)),
            19 => Key::Up,
            20 => Key::Down,
            21 => Key::Left,
            22 => Key::Right,
            61 => Key::Tab,
            62 => Key::Char(' '),
            66 | 160 => Key::Enter,
            67 => Key::Backspace,
            92 => Key::PageUp,
            93 => Key::PageDown,
            111 => Key::Escape,
            112 => Key::Delete,
            122 => Key::Home,
            123 => Key::End,
            124 => Key::Insert,
            131..=142 => Key::F((code - 130) as u8),
            144..=153 => Key::Char(char::from(b'0' + (code - 144) as u8)),
            85 => Key::MediaPlayPause,
            86 => Key::MediaStop,
            87 => Key::MediaNext,
            88 => Key::MediaPrevious,
            89 => Key::MediaRewind,
            90 => Key::MediaFastForward,
            126 => Key::MediaPlay,
            127 => Key::MediaPause,
            165 => Key::Info,
            166 => Key::ChannelUp,
            167 => Key::ChannelDown,
            183 => Key::ColorRed,
            184 => Key::ColorGreen,
            185 => Key::ColorYellow,
            186 => Key::ColorBlue,
            _ => {
                return ANDROID_KEYS
                    .iter()
                    .find(|(k, _)| *k == code)
                    .map(|&(_, c)| Key::Char(c));
            }
        })
    }

    /// The Android key code of this key (`KeyEvent.KEYCODE_*`, the main
    /// keyboard's): what a key event for it carries.
    pub fn android_keycode(self) -> Option<i32> {
        Some(match self {
            Key::Char(c @ '0'..='9') => 7 + i32::from(c as u8 - b'0'),
            Key::Char(c @ 'a'..='z') => 29 + i32::from(c as u8 - b'a'),
            Key::Up => 19,
            Key::Down => 20,
            Key::Left => 21,
            Key::Right => 22,
            Key::Tab => 61,
            Key::Char(' ') => 62,
            Key::Enter => 66,
            Key::Backspace => 67,
            Key::PageUp => 92,
            Key::PageDown => 93,
            Key::Escape => 111,
            Key::Delete => 112,
            Key::Home => 122,
            Key::End => 123,
            Key::Insert => 124,
            Key::F(n @ 1..=12) => 130 + i32::from(n),
            Key::MediaPlayPause => 85,
            Key::MediaStop => 86,
            Key::MediaNext => 87,
            Key::MediaPrevious => 88,
            Key::MediaRewind => 89,
            Key::MediaFastForward => 90,
            Key::MediaPlay => 126,
            Key::MediaPause => 127,
            Key::Info => 165,
            Key::ChannelUp => 166,
            Key::ChannelDown => 167,
            Key::ColorRed => 183,
            Key::ColorGreen => 184,
            Key::ColorYellow => 185,
            Key::ColorBlue => 186,
            Key::Char(c) => return ANDROID_KEYS.iter().find(|(_, k)| *k == c).map(|&(k, _)| k),
            Key::F(_) => return None,
        })
    }

    /// A media key or a TV remote's key ([`Key::MediaPlayPause`] to
    /// [`Key::Info`]): keys a keyboard's layout has no part in, which
    /// edit no text.
    pub fn is_media_or_remote(self) -> bool {
        matches!(
            self,
            Key::MediaPlayPause
                | Key::MediaPlay
                | Key::MediaPause
                | Key::MediaStop
                | Key::MediaRewind
                | Key::MediaFastForward
                | Key::MediaNext
                | Key::MediaPrevious
                | Key::ColorRed
                | Key::ColorGreen
                | Key::ColorYellow
                | Key::ColorBlue
                | Key::ChannelUp
                | Key::ChannelDown
                | Key::Info
        )
    }
}

/// The US layout's punctuation keys' USB HID usages.
const HID_KEYS: &[(u16, char)] = &[
    (45, '-'),
    (46, '='),
    (47, '['),
    (48, ']'),
    (49, '\\'),
    (51, ';'),
    (52, '\''),
    (53, '`'),
    (54, ','),
    (55, '.'),
    (56, '/'),
];

/// The US layout's punctuation keys' Android key codes.
const ANDROID_KEYS: &[(i32, char)] = &[
    (55, ','),
    (56, '.'),
    (68, '`'),
    (69, '-'),
    (70, '='),
    (71, '['),
    (72, ']'),
    (73, '\\'),
    (74, ';'),
    (75, '\''),
    (76, '/'),
];

/// One key with its modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Keystroke {
    /// Modifiers held.
    pub modifiers: Modifiers,
    /// The key.
    pub key: Key,
}

/// Why a keystroke's text is not one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeystrokeError(pub String);

impl fmt::Display for KeystrokeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for KeystrokeError {}

/// Characters typed with Shift on the US layout, and the key typing them.
const SHIFTED: &[(char, char)] = &[
    ('~', '`'),
    ('!', '1'),
    ('@', '2'),
    ('#', '3'),
    ('$', '4'),
    ('%', '5'),
    ('^', '6'),
    ('&', '7'),
    ('*', '8'),
    ('(', '9'),
    (')', '0'),
    ('_', '-'),
    ('+', '='),
    ('{', '['),
    ('}', ']'),
    ('|', '\\'),
    (':', ';'),
    ('"', '\''),
    ('<', ','),
    ('>', '.'),
    ('?', '/'),
];

/// Named keys, by their names (lower case).
const NAMED: &[(&str, Key)] = &[
    ("enter", Key::Enter),
    ("return", Key::Enter),
    ("escape", Key::Escape),
    ("esc", Key::Escape),
    ("tab", Key::Tab),
    ("space", Key::Char(' ')),
    ("backspace", Key::Backspace),
    ("delete", Key::Delete),
    ("del", Key::Delete),
    ("insert", Key::Insert),
    ("up", Key::Up),
    ("arrowup", Key::Up),
    ("down", Key::Down),
    ("arrowdown", Key::Down),
    ("left", Key::Left),
    ("arrowleft", Key::Left),
    ("right", Key::Right),
    ("arrowright", Key::Right),
    ("home", Key::Home),
    ("end", Key::End),
    ("pageup", Key::PageUp),
    ("pagedown", Key::PageDown),
    // The plus key is the one typing `=`, as on the US layout.
    ("plus", Key::Char('=')),
    ("minus", Key::Char('-')),
    // Media keys and the TV remote's: named as the GPUI backend's key
    // names are, the web's `KeyboardEvent.key` names lowercased beside
    // them where they differ.
    ("mediaplaypause", Key::MediaPlayPause),
    ("mediaplay", Key::MediaPlay),
    ("mediapause", Key::MediaPause),
    ("mediastop", Key::MediaStop),
    ("mediarewind", Key::MediaRewind),
    ("mediafastforward", Key::MediaFastForward),
    ("medianext", Key::MediaNext),
    ("mediatracknext", Key::MediaNext),
    ("mediaprevious", Key::MediaPrevious),
    ("mediatrackprevious", Key::MediaPrevious),
    ("red", Key::ColorRed),
    ("colorf0red", Key::ColorRed),
    ("green", Key::ColorGreen),
    ("colorf1green", Key::ColorGreen),
    ("yellow", Key::ColorYellow),
    ("colorf2yellow", Key::ColorYellow),
    ("blue", Key::ColorBlue),
    ("colorf3blue", Key::ColorBlue),
    ("channelup", Key::ChannelUp),
    ("channeldown", Key::ChannelDown),
    ("info", Key::Info),
];

fn error(text: &str, reason: impl fmt::Display) -> KeystrokeError {
    KeystrokeError(format!("invalid keystroke {text:?}: {reason}"))
}

impl Keystroke {
    /// Parses `text`: modifiers and a key joined by `+` (`"mod+shift+k"`,
    /// `"mod+-"` for Command and minus), in any case (lowercase is the
    /// style; a capital letter is the same key, not Shift). Modifiers:
    /// `ctrl`, `alt` (`option`), `shift`, `cmd` (`super`, `win`) and `mod`,
    /// which is Command on Apple platforms (`os`, [`apple_keys`]) and
    /// Control elsewhere. Keys: a character of the US layout typed without Shift (`k`, `1`,
    /// `/`, `[`, `=`: the plus key is `=`, `shift+=` with Shift), `space`,
    /// `enter`, `escape`, `tab`, `backspace`, `delete`, the arrows `up` /
    /// `down` / `left` / `right`, `home`, `end`, `pageup`, `pagedown`, `f1`
    /// to `f24`; the media and TV remote keys `mediaplaypause`, `mediaplay`,
    /// `mediapause`, `mediastop`, `mediarewind`, `mediafastforward`,
    /// `medianext`, `mediaprevious`, `red`, `green`, `yellow`, `blue`,
    /// `channelup`, `channeldown`, `info`.
    pub fn parse(text: &str, os: Os) -> Result<Keystroke, KeystrokeError> {
        Keystroke::parse_with(text, apple_keys(os, false))
    }

    /// Parses `text` as [`Keystroke::parse`] does, with Apple's conventions
    /// (`mod` is Command) or not: a browser decides by the platform it runs
    /// on ([`apple_keys`]).
    pub fn parse_with(text: &str, apple: bool) -> Result<Keystroke, KeystrokeError> {
        // Case does not matter (lowercase is the style); messages quote
        // the text as written and suggest lowercase.
        let lower = text.to_ascii_lowercase();
        let (mods, key) = split(text, &lower)?;
        let mut modifiers = Modifiers::default();
        for name in mods {
            let flag = modifier(&mut modifiers, name, apple).map_err(|why| error(text, why))?;
            if *flag {
                return Err(error(text, format_args!("{name:?} twice")));
            }
            *flag = true;
        }
        Ok(Keystroke {
            modifiers,
            key: parse_key(text, key)?,
        })
    }

    /// Why the platform keeps this keystroke for itself on `os` (quitting,
    /// hiding, minimizing the app or making it full screen), if it does:
    /// such keystrokes cannot be bound.
    pub fn reserved(&self, os: Os) -> Option<&'static str> {
        let m = self.modifiers;
        let only = |ctrl, alt, cmd| m.ctrl == ctrl && m.alt == alt && m.cmd == cmd && !m.shift;
        match os {
            Os::MacOs | Os::Ios => match self.key {
                Key::Char('q') if only(false, false, true) => Some("it quits the app"),
                Key::Char('h') if only(false, false, true) => Some("it hides the app"),
                Key::Char('h') if only(false, true, true) => Some("it hides the other apps"),
                Key::Char('m') if only(false, false, true) => Some("it minimizes the window"),
                Key::Char('f') if only(true, false, true) => Some("it toggles full screen"),
                _ => None,
            },
            Os::Linux | Os::Windows | Os::Android | Os::Tizen => match self.key {
                Key::Char('q') if only(true, false, false) => Some("it quits the app"),
                Key::F(4) if only(false, true, false) => Some("it closes the window"),
                Key::F(11) if m == Modifiers::default() => Some("it toggles full screen"),
                _ => None,
            },
            // The browser keeps its own keys: the page never sees them.
            Os::Web => None,
        }
    }

    /// Whether this is the development hosts' reload key on `os` (`mod+r`:
    /// Cmd+R, Ctrl+R): built with their `dev` feature, platforms leave it
    /// to the dev server client, whatever the app binds.
    pub fn reloads(&self, os: Os) -> bool {
        let m = self.modifiers;
        let apple = apple_keys(os, false);
        self.key == Key::Char('r') && !m.shift && !m.alt && m.cmd == apple && m.ctrl != apple
    }

    /// Whether a text input that has the keyboard focus uses this
    /// keystroke itself: keys that type, move the caret or delete, and the
    /// editing shortcuts (select all, cut, copy, paste, undo, redo).
    /// Bindings of these keystrokes do not fire while text is edited,
    /// unless they ask to ([`When::Always`]). The function, media and TV
    /// remote keys edit nothing.
    pub fn edits_text(&self) -> bool {
        let m = self.modifiers;
        match self.key {
            Key::F(_) => false,
            key if key.is_media_or_remote() => false,
            Key::Char(c) => !(m.cmd || m.ctrl) || matches!(c, 'a' | 'c' | 'v' | 'x' | 'y' | 'z'),
            _ => true,
        }
    }
}

/// The modifiers and the key of `text` (`"mod+shift+k"`).
fn split<'a>(text: &str, lower: &'a str) -> Result<(Vec<&'a str>, &'a str), KeystrokeError> {
    if text.is_empty() {
        return Err(error(text, "empty"));
    }
    if text.contains(char::is_whitespace) {
        return Err(error(
            text,
            "one keystroke per binding (key sequences such as \"mod+k mod+s\" are not supported)",
        ));
    }
    if lower == "+" || lower.ends_with("++") {
        let mods = lower.strip_suffix("++").unwrap_or("");
        let plus = if mods.is_empty() {
            "plus".to_owned()
        } else {
            format!("{mods}+plus")
        };
        return Err(error(
            text,
            format_args!("write the plus key as \"plus\": {plus:?}"),
        ));
    }
    let (mods, key) = match lower.rsplit_once('+') {
        Some((mods, key)) => (mods.split('+').collect(), key),
        None => (Vec::new(), lower),
    };
    if key.is_empty() {
        return Err(error(text, "no key after the modifiers"));
    }
    Ok((mods, key))
}

/// The flag of modifier `name` in `modifiers`, or why it is none.
fn modifier<'a>(
    modifiers: &'a mut Modifiers,
    name: &str,
    apple: bool,
) -> Result<&'a mut bool, String> {
    Ok(match name {
        "ctrl" | "control" => &mut modifiers.ctrl,
        "alt" | "option" | "opt" => &mut modifiers.alt,
        "shift" => &mut modifiers.shift,
        "cmd" | "command" | "meta" | "super" | "win" => &mut modifiers.cmd,
        "mod" | "cmdorctrl" | "commandorcontrol" if apple => &mut modifiers.cmd,
        "mod" | "cmdorctrl" | "commandorcontrol" => &mut modifiers.ctrl,
        "" => return Err("an empty modifier".into()),
        other => {
            return Err(format!(
                "unknown modifier {other:?} (ctrl, alt, shift, cmd or mod)"
            ));
        }
    })
}

fn parse_key(text: &str, key: &str) -> Result<Key, KeystrokeError> {
    if let Some(&(_, named)) = NAMED.iter().find(|(name, _)| *name == key) {
        return Ok(named);
    }
    if let Some(n) = key
        .strip_prefix('f')
        .filter(|n| !n.starts_with('0'))
        .and_then(|n| n.parse::<u8>().ok())
        && (1..=24).contains(&n)
    {
        return Ok(Key::F(n));
    }
    let mut chars = key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        let hint = if let Some((first, _)) = key.split_once('-')
            && modifier(&mut Modifiers::default(), first, false).is_ok()
        {
            // `mod--` is `mod+-`.
            let joined = match key.strip_suffix("--") {
                Some(mods) => format!("{}+-", mods.replace('-', "+")),
                None => key.replace('-', "+"),
            };
            format!(" (join modifiers and the key with \"+\": {joined:?})")
        } else {
            String::new()
        };
        return Err(error(text, format_args!("unknown key {key:?}{hint}")));
    };
    if let Some(&(_, base)) = SHIFTED.iter().find(|(shifted, _)| *shifted == c) {
        return Err(error(
            text,
            format_args!("{c:?} is typed with Shift: write \"shift+{base}\""),
        ));
    }
    if !c.is_ascii_graphic() {
        return Err(error(
            text,
            "keys are named by the US layout's characters (they match on any layout)",
        ));
    }
    Ok(Key::Char(c))
}

/// The character the key typing `c` types with Shift on the US layout
/// (`8` a `*`, `a` an `A`): what `shift+8` types.
pub fn shifted(c: char) -> char {
    SHIFTED
        .iter()
        .find(|(_, base)| *base == c)
        .map_or(c.to_ascii_uppercase(), |(shifted, _)| *shifted)
}

/// Whether keyboard shortcuts on `os` follow Apple's conventions: `mod` is
/// Command and labels use ⌘ ([`Keystroke::label`]). macOS and iOS do; a
/// browser does when it runs on an Apple platform (`apple_keyboard`,
/// [`crate::platform::Environment::apple_keyboard`]).
pub fn apple_keys(os: Os, apple_keyboard: bool) -> bool {
    match os {
        Os::MacOs | Os::Ios => true,
        Os::Web => apple_keyboard,
        Os::Android | Os::Windows | Os::Linux | Os::Tizen => false,
    }
}

impl Keystroke {
    /// The keys pressed for this keystroke: where Ctrl stands in for
    /// Command (`ctrl_for_command`, a terminal on macOS that cannot report
    /// Command: [`crate::platform::Environment::ctrl_for_command`]), its
    /// Command is Ctrl, so a label shows the key to press (`⌃O` for
    /// `mod+o`). A keystroke with both keeps them.
    pub fn pressed(mut self, ctrl_for_command: bool) -> Keystroke {
        if ctrl_for_command && self.modifiers.cmd && !self.modifiers.ctrl {
            self.modifiers.cmd = false;
            self.modifiers.ctrl = true;
        }
        self
    }

    /// The keystroke as menus and hints show it: `⌃⌥⇧⌘K` with Apple's
    /// conventions ([`apple_keys`]), else `Ctrl+Alt+Shift+K` (the Command
    /// modifier is `Win` on Windows, `Meta` in a browser, `Super`
    /// elsewhere).
    pub fn label(&self, os: Os, apple: bool) -> String {
        let m = self.modifiers;
        let mut s = String::new();
        if apple {
            for (on, glyph) in [(m.ctrl, '⌃'), (m.alt, '⌥'), (m.shift, '⇧'), (m.cmd, '⌘')] {
                if on {
                    s.push(glyph);
                }
            }
        } else {
            let cmd = match os {
                Os::Windows => "Win+",
                Os::Web => "Meta+",
                _ => "Super+",
            };
            for (on, name) in [
                (m.ctrl, "Ctrl+"),
                (m.alt, "Alt+"),
                (m.shift, "Shift+"),
                (m.cmd, cmd),
            ] {
                if on {
                    s.push_str(name);
                }
            }
        }
        let named = |mac: &'static str, other: &'static str| if apple { mac } else { other };
        match self.key {
            Key::Char(' ') => s.push_str("Space"),
            Key::Char(c) => s.push(c.to_ascii_uppercase()),
            Key::F(n) => {
                let _ = fmt::Write::write_fmt(&mut s, format_args!("F{n}"));
            }
            Key::Enter => s.push_str(named("↩", "Enter")),
            Key::Escape => s.push_str(named("⎋", "Esc")),
            Key::Tab => s.push_str(named("⇥", "Tab")),
            Key::Backspace => s.push_str(named("⌫", "Backspace")),
            Key::Delete => s.push_str(named("⌦", "Del")),
            Key::Insert => s.push_str(named("Help", "Ins")),
            Key::Up => s.push('↑'),
            Key::Down => s.push('↓'),
            Key::Left => s.push('←'),
            Key::Right => s.push('→'),
            Key::Home => s.push_str(named("↖", "Home")),
            Key::End => s.push_str(named("↘", "End")),
            Key::PageUp => s.push_str(named("⇞", "PgUp")),
            Key::PageDown => s.push_str(named("⇟", "PgDn")),
            Key::MediaPlayPause => s.push_str("Play/Pause"),
            Key::MediaPlay => s.push_str("Play"),
            Key::MediaPause => s.push_str("Pause"),
            Key::MediaStop => s.push_str("Stop"),
            Key::MediaRewind => s.push_str("Rewind"),
            Key::MediaFastForward => s.push_str("Fast Forward"),
            Key::MediaNext => s.push_str("Next Track"),
            Key::MediaPrevious => s.push_str("Previous Track"),
            Key::ColorRed => s.push_str("Red"),
            Key::ColorGreen => s.push_str("Green"),
            Key::ColorYellow => s.push_str("Yellow"),
            Key::ColorBlue => s.push_str("Blue"),
            Key::ChannelUp => s.push_str("Channel Up"),
            Key::ChannelDown => s.push_str("Channel Down"),
            Key::Info => s.push_str("Info"),
        }
        s
    }
}

impl fmt::Display for Keystroke {
    /// The canonical text: `ctrl+alt+shift+cmd+<key>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.modifiers;
        for (on, name) in [
            (m.ctrl, "ctrl+"),
            (m.alt, "alt+"),
            (m.shift, "shift+"),
            (m.cmd, "cmd+"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        match self.key {
            Key::Char(' ') => f.write_str("space"),
            Key::Char(c) => write!(f, "{c}"),
            Key::F(n) => write!(f, "f{n}"),
            key => {
                let name = NAMED.iter().find(|(_, k)| *k == key).map(|(n, _)| *n);
                f.write_str(name.unwrap_or("?"))
            }
        }
    }
}

/// When a binding fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum When {
    /// Unless a text input (or a hosted native view, such as a web view)
    /// has the keyboard focus and the keystroke is one it uses
    /// ([`Keystroke::edits_text`]): the default.
    #[default]
    Auto,
    /// Wherever the keyboard focus is, in text inputs too.
    Always,
    /// Only while no text input or hosted native view has the keyboard
    /// focus.
    NotEditing,
}

/// One binding of a [`Keymap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    /// What the platform reports when it fires
    /// ([`crate::EventKind::KeyCommand`]'s payload).
    pub id: u32,
    /// The keystroke.
    pub keystroke: Keystroke,
    /// It fires while a text input or hosted native view has the keyboard
    /// focus ([`When`], resolved).
    pub in_text: bool,
    /// It fires again on the auto-repeats of a held key; else once until
    /// the key is released.
    pub repeat: bool,
    /// It fires and lets the key go on as if there were no binding
    /// ([`KeyMatch::Notify`]): Tab still moves the focus, a text input
    /// still types. Else the platform consumes the key.
    pub pass_through: bool,
    /// The window it belongs to, as its root node ([`crate::ROOT_ID`] for
    /// the main window, a dialog root for a window of the app's own): it
    /// fires only for keys pressed there. `None`: in any window.
    pub window: Option<crate::NodeId>,
}

impl KeyBinding {
    /// A binding of `keystroke` firing as `when` says, on auto-repeats
    /// too when `repeat`.
    pub fn new(id: u32, keystroke: Keystroke, when: When, repeat: bool) -> KeyBinding {
        let in_text = match when {
            When::Auto => !keystroke.edits_text(),
            When::Always => true,
            When::NotEditing => false,
        };
        KeyBinding {
            id,
            keystroke,
            in_text,
            repeat,
            pass_through: false,
            window: None,
        }
    }

    /// The binding firing only in the window of root node `window`
    /// ([`Self::window`]).
    pub fn in_window(mut self, window: crate::NodeId) -> KeyBinding {
        self.window = Some(window);
        self
    }

    /// The binding firing without consuming the key when `pass_through`
    /// ([`Self::pass_through`]).
    pub fn passing_through(mut self, pass_through: bool) -> KeyBinding {
        self.pass_through = pass_through;
        self
    }
}

/// What a key does with a [`Keymap`] ([`Keymap::find`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMatch {
    /// No binding: the key goes on as if there were no keymap.
    None,
    /// It fires binding `id`: the platform consumes the key and reports it
    /// ([`crate::EventKind::KeyCommand`]).
    Fire(u32),
    /// An auto-repeat of a held key whose binding fires once: the platform
    /// consumes it and reports nothing.
    Held,
    /// It fires binding `id`, which passes the key through
    /// ([`KeyBinding::pass_through`]): the platform reports it
    /// ([`crate::EventKind::KeyCommand`]) and lets the key go on as if there
    /// were no keymap (Tab moves the focus after the report).
    Notify(u32),
}

impl KeyMatch {
    /// The platform consumes the key.
    pub fn consumed(self) -> bool {
        matches!(self, KeyMatch::Fire(_) | KeyMatch::Held)
    }

    /// The binding the key fires, which the platform reports
    /// ([`crate::EventKind::KeyCommand`]): a consumed one's or a passing
    /// one's.
    pub fn fired(self) -> Option<u32> {
        match self {
            KeyMatch::Fire(id) | KeyMatch::Notify(id) => Some(id),
            KeyMatch::None | KeyMatch::Held => None,
        }
    }
}

/// The bindings of a surface, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Keymap {
    bindings: Vec<KeyBinding>,
}

impl Keymap {
    /// A table of `bindings`, oldest first.
    pub fn new(bindings: Vec<KeyBinding>) -> Keymap {
        Keymap { bindings }
    }

    /// No bindings: the platform need not look at keys.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// The bindings, oldest first.
    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    /// What a key does: the newest binding of `keystroke` that fires while
    /// `editing` (a text input or hosted native view has the keyboard
    /// focus) takes it; `repeat` is an auto-repeat of a held key, which only
    /// fires bindings that ask for it. A binding that passes the key
    /// through gives [`KeyMatch::Notify`], and its held key's auto-repeats
    /// go on ([`KeyMatch::None`]) unless it repeats.
    /// The key is pressed in the main window ([`Self::find_in`]).
    pub fn find(&self, keystroke: Keystroke, editing: bool, repeat: bool) -> KeyMatch {
        self.find_in(crate::ROOT_ID, keystroke, editing, repeat)
    }

    /// [`Self::find`] for a key pressed in the window of root node
    /// `window`: only that window's bindings and those of no window
    /// ([`KeyBinding::window`]) take it.
    pub fn find_in(
        &self,
        window: crate::NodeId,
        keystroke: Keystroke,
        editing: bool,
        repeat: bool,
    ) -> KeyMatch {
        let found = self.bindings.iter().rev().find(|b| {
            b.keystroke == keystroke
                && (b.in_text || !editing)
                && b.window.is_none_or(|w| w == window)
        });
        match found {
            None => KeyMatch::None,
            Some(b) if repeat && !b.repeat && b.pass_through => KeyMatch::None,
            Some(b) if repeat && !b.repeat => KeyMatch::Held,
            Some(b) if b.pass_through => KeyMatch::Notify(b.id),
            Some(b) => KeyMatch::Fire(b.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_windows_binding_takes_only_its_windows_keys() {
        let escape = Keystroke::parse_with("escape", true).unwrap();
        let k = |id| KeyBinding::new(id, escape, When::Always, false);
        // JS's (any window), the main window's, then window 7's.
        let table = Keymap::new(vec![
            k(1),
            k(2).in_window(crate::ROOT_ID),
            k(3).in_window(7),
        ]);
        assert_eq!(table.find_in(7, escape, false, false), KeyMatch::Fire(3));
        assert_eq!(
            table.find_in(crate::ROOT_ID, escape, false, false),
            KeyMatch::Fire(2)
        );
        assert_eq!(table.find(escape, false, false), KeyMatch::Fire(2));
        // A window with none of its own: the one of no window.
        assert_eq!(table.find_in(9, escape, false, false), KeyMatch::Fire(1));
        let only = Keymap::new(vec![k(3).in_window(7)]);
        assert_eq!(only.find_in(9, escape, false, false), KeyMatch::None);
    }

    #[test]
    fn shift_types_the_us_layouts_characters() {
        assert_eq!(shifted('8'), '*');
        assert_eq!(shifted('2'), '@');
        assert_eq!(shifted('/'), '?');
        assert_eq!(shifted('a'), 'A');
        assert_eq!(shifted('*'), '*');
    }

    #[test]
    fn keys_have_windows_virtual_key_codes_both_ways() {
        assert_eq!(Key::from_virtual_key(0x4B), Some(Key::Char('k')));
        assert_eq!(Key::from_virtual_key(0x31), Some(Key::Char('1')));
        assert_eq!(Key::from_virtual_key(0x74), Some(Key::F(5)));
        assert_eq!(Key::from_virtual_key(0xBF), Some(Key::Char('/')));
        assert_eq!(Key::from_virtual_key(0x62), Some(Key::Char('2')));
        assert_eq!(Key::from_virtual_key(0x10), None);
        for vk in (0..=0xFF).filter(|vk| !(0x60..=0x69).contains(vk)) {
            if let Some(key) = Key::from_virtual_key(vk) {
                assert_eq!(key.virtual_key(), Some(vk), "{key:?}");
            }
        }
        assert_eq!(Key::Char('?').virtual_key(), None);
        assert_eq!(Key::F(25).virtual_key(), None);
    }

    #[test]
    fn keys_have_hid_usages_and_android_key_codes_both_ways() {
        assert_eq!(Key::from_hid_usage(4), Some(Key::Char('a')));
        assert_eq!(Key::from_hid_usage(39), Some(Key::Char('0')));
        assert_eq!(Key::from_hid_usage(81), Some(Key::Down));
        assert_eq!(Key::from_hid_usage(56), Some(Key::Char('/')));
        assert_eq!(Key::from_hid_usage(88), Some(Key::Enter));
        assert_eq!(Key::from_hid_usage(104), Some(Key::F(13)));
        assert_eq!(Key::from_hid_usage(225), None);
        // The keypad's keys have the main keyboard's usages back.
        for usage in (0..=0xFF).filter(|u| !(88..=98).contains(u)) {
            if let Some(key) = Key::from_hid_usage(usage) {
                assert_eq!(key.hid_usage(), Some(usage), "{key:?}");
            }
        }
        assert_eq!(Key::from_android_keycode(29), Some(Key::Char('a')));
        assert_eq!(Key::from_android_keycode(7), Some(Key::Char('0')));
        assert_eq!(Key::from_android_keycode(20), Some(Key::Down));
        assert_eq!(Key::from_android_keycode(76), Some(Key::Char('/')));
        assert_eq!(Key::from_android_keycode(160), Some(Key::Enter));
        assert_eq!(Key::from_android_keycode(59), None);
        for code in (0..=300).filter(|c| *c != 160 && !(144..=153).contains(c)) {
            if let Some(key) = Key::from_android_keycode(code) {
                assert_eq!(key.android_keycode(), Some(code), "{key:?}");
            }
        }
    }

    fn parse(text: &str) -> Result<Keystroke, KeystrokeError> {
        Keystroke::parse(text, Os::MacOs)
    }

    fn stroke(ctrl: bool, alt: bool, shift: bool, cmd: bool, key: Key) -> Keystroke {
        Keystroke {
            modifiers: Modifiers {
                ctrl,
                alt,
                shift,
                cmd,
            },
            key,
        }
    }

    #[test]
    fn parses_keystrokes() {
        let c = |c| Key::Char(c);
        assert_eq!(parse("k"), Ok(stroke(false, false, false, false, c('k'))));
        assert_eq!(
            parse("cmd+shift+k"),
            Ok(stroke(false, false, true, true, c('k')))
        );
        assert_eq!(
            parse("ctrl+alt+enter"),
            Ok(stroke(true, true, false, false, Key::Enter))
        );
        assert_eq!(
            parse("cmd+-"),
            Ok(stroke(false, false, false, true, c('-')))
        );
        assert_eq!(parse("-"), Ok(stroke(false, false, false, false, c('-'))));
        assert_eq!(
            parse("cmd+="),
            Ok(stroke(false, false, false, true, c('=')))
        );
        assert_eq!(
            parse("space"),
            Ok(stroke(false, false, false, false, c(' ')))
        );
        assert_eq!(
            parse("f5"),
            Ok(stroke(false, false, false, false, Key::F(5)))
        );
        assert_eq!(
            parse("shift+/"),
            Ok(stroke(false, false, true, false, c('/')))
        );
        assert_eq!(
            parse("option+escape"),
            Ok(stroke(false, true, false, false, Key::Escape))
        );
        // `mod` is Command on Apple platforms, Control elsewhere.
        assert_eq!(parse("mod+k"), parse("cmd+k"));
        // Case does not matter, and a capital letter is not Shift.
        assert_eq!(parse("Mod+Shift+N"), parse("mod+shift+n"));
        assert_eq!(parse("CMD+K"), parse("cmd+k"));
        assert_eq!(parse("mod+K"), parse("mod+k"));
        assert_eq!(parse("Escape"), parse("escape"));
        assert_eq!(parse("F5"), parse("f5"));
        assert_eq!(parse("Ctrl+Plus"), parse("ctrl+="));
        assert_eq!(parse("cmdorctrl+k"), parse("meta+k"));
        assert_eq!(parse("mod+plus"), parse("mod+="));
        assert_eq!(parse("ctrl+minus"), parse("ctrl+-"));
        assert_eq!(parse("arrowup"), parse("up"));
        assert_eq!(parse("insert").unwrap().key, Key::Insert);
        assert_eq!(
            Keystroke::parse("mod+k", Os::Linux),
            Keystroke::parse("ctrl+k", Os::Linux)
        );
        assert_eq!(parse("mod+k").unwrap().to_string(), "cmd+k");
        assert_eq!(
            parse("shift+cmd+alt+ctrl+pagedown").unwrap().to_string(),
            "ctrl+alt+shift+cmd+pagedown"
        );
    }

    #[test]
    fn follows_the_browsers_platform() {
        // A browser resolves `mod` by the platform it runs on.
        assert!(apple_keys(Os::Web, true));
        assert!(!apple_keys(Os::Web, false));
        assert!(apple_keys(Os::MacOs, false));
        assert_eq!(
            Keystroke::parse_with("mod+k", true),
            Keystroke::parse("cmd+k", Os::MacOs)
        );
        assert_eq!(
            Keystroke::parse("mod+k", Os::Web),
            Keystroke::parse("ctrl+k", Os::Web)
        );
        // Literal modifiers stay what they say.
        assert_eq!(
            Keystroke::parse_with("ctrl+k", true),
            Keystroke::parse("ctrl+k", Os::Linux)
        );
    }

    #[test]
    fn labels_keystrokes() {
        let label = |t, os, apple| Keystroke::parse_with(t, apple).unwrap().label(os, apple);
        assert_eq!(label("mod+shift+l", Os::Web, true), "⇧⌘L");
        assert_eq!(label("mod+shift+l", Os::Web, false), "Ctrl+Shift+L");
        assert_eq!(label("mod+backspace", Os::MacOs, true), "⌘⌫");
        assert_eq!(label("ctrl+alt+enter", Os::Linux, false), "Ctrl+Alt+Enter");
        assert_eq!(label("cmd+space", Os::Windows, false), "Win+Space");
        assert_eq!(label("f5", Os::Web, true), "F5");
        assert_eq!(label("mod+/", Os::Web, false), "Ctrl+/");
        // Where Ctrl stands in for Command, the label shows Ctrl.
        let pressed = |t| {
            Keystroke::parse_with(t, true)
                .unwrap()
                .pressed(true)
                .label(Os::MacOs, true)
        };
        assert_eq!(pressed("mod+o"), "⌃O");
        assert_eq!(pressed("mod+shift+t"), "⌃⇧T");
        assert_eq!(pressed("ctrl+cmd+f"), "⌃⌘F");
    }

    #[test]
    fn explains_invalid_keystrokes() {
        let err = |t| parse(t).unwrap_err().0;
        assert_eq!(err(""), "invalid keystroke \"\": empty");
        assert!(err("mod+k mod+s").contains("one keystroke per binding"));
        assert!(err("cmd+").contains("no key after the modifiers"));
        assert!(err("hyper+k").contains("unknown modifier \"hyper\""));
        assert!(err("cmd+cmd+k").contains("\"cmd\" twice"));
        assert!(err("cmd+kk").contains("unknown key \"kk\""));
        assert!(err("mod-k").contains("join modifiers and the key with \"+\": \"mod+k\""));
        assert!(err("ctrl-shift-p").contains("\"ctrl+shift+p\""));
        assert!(err("mod--").contains("\"mod+-\""));
        // Hints suggest lowercase, whatever case the text was written in.
        assert!(err("Mod-K").contains("join modifiers and the key with \"+\": \"mod+k\""));
        assert!(err("CTRL++").contains("\"ctrl+plus\""));
        assert!(err("cmd+?").contains("write \"shift+/\""));
        assert!(err("mod++").contains("write the plus key as \"plus\": \"mod+plus\""));
        assert!(err("+").contains("\"plus\""));
        assert!(err("cmd+é").contains("US layout"));
        assert!(err("f25").contains("unknown key"));
        assert!(err("f05").contains("unknown key"));
        assert!(err("cmd++k").contains("an empty modifier"));
    }

    #[test]
    fn reserves_what_the_platform_keeps() {
        let reserved = |t, os| Keystroke::parse(t, os).unwrap().reserved(os).is_some();
        assert!(reserved("mod+q", Os::MacOs));
        assert!(reserved("cmd+alt+h", Os::MacOs));
        assert!(reserved("ctrl+cmd+f", Os::MacOs));
        assert!(!reserved("cmd+shift+q", Os::MacOs));
        assert!(!reserved("ctrl+q", Os::MacOs));
        assert!(!reserved("cmd+w", Os::MacOs));
        assert!(reserved("mod+q", Os::Linux));
        assert!(reserved("f11", Os::Linux));
        assert!(reserved("alt+f4", Os::Windows));
        assert!(!reserved("cmd+q", Os::Linux));
    }

    #[test]
    fn the_dev_reload_key() {
        let reloads = |t, os| Keystroke::parse(t, os).unwrap().reloads(os);
        assert!(reloads("mod+r", Os::MacOs));
        assert!(reloads("ctrl+r", Os::Windows));
        assert!(!reloads("ctrl+r", Os::MacOs));
        assert!(!reloads("mod+shift+r", Os::Linux));
    }

    #[test]
    fn text_inputs_keep_their_keys() {
        let edits = |t| parse(t).unwrap().edits_text();
        assert!(edits("k"));
        assert!(edits("shift+k"));
        assert!(edits("alt+k"));
        assert!(edits("escape"));
        assert!(edits("cmd+left"));
        assert!(edits("cmd+c"));
        assert!(edits("cmd+shift+z"));
        assert!(edits("ctrl+y"));
        assert!(edits("cmd+y"));
        assert!(!edits("cmd+k"));
        assert!(!edits("ctrl+shift+p"));
        assert!(!edits("f5"));
    }

    #[test]
    fn the_newest_binding_that_fires_wins() {
        let k = parse("cmd+k").unwrap();
        let e = parse("escape").unwrap();
        let map = Keymap::new(vec![
            KeyBinding::new(1, k, When::Auto, false),
            KeyBinding::new(2, e, When::Auto, true),
            KeyBinding::new(3, k, When::NotEditing, false),
        ]);
        assert_eq!(map.find(k, false, false), KeyMatch::Fire(3));
        assert_eq!(map.find(k, true, false), KeyMatch::Fire(1));
        assert_eq!(map.find(e, false, false), KeyMatch::Fire(2));
        assert_eq!(map.find(e, true, false), KeyMatch::None);
        assert_eq!(
            map.find(parse("cmd+j").unwrap(), false, false),
            KeyMatch::None
        );
        // A held key fires once, unless its binding repeats.
        assert_eq!(map.find(k, false, true), KeyMatch::Held);
        assert_eq!(map.find(e, false, true), KeyMatch::Fire(2));
        let always = Keymap::new(vec![KeyBinding::new(4, e, When::Always, false)]);
        assert_eq!(always.find(e, true, false), KeyMatch::Fire(4));
    }

    #[test]
    fn a_passing_binding_fires_and_leaves_the_key() {
        let tab = parse("tab").unwrap();
        let map = Keymap::new(vec![
            KeyBinding::new(1, tab, When::Auto, false).passing_through(true),
        ]);
        let found = map.find(tab, false, false);
        assert_eq!(found, KeyMatch::Notify(1));
        assert_eq!(found.fired(), Some(1));
        assert!(!found.consumed());
        // Its held key's repeats go on, unreported.
        assert_eq!(map.find(tab, false, true), KeyMatch::None);
        let repeating = Keymap::new(vec![
            KeyBinding::new(2, tab, When::Auto, true).passing_through(true),
        ]);
        assert_eq!(repeating.find(tab, false, true), KeyMatch::Notify(2));
        assert_eq!(KeyMatch::Fire(3).fired(), Some(3));
        assert!(KeyMatch::Fire(3).consumed() && KeyMatch::Held.consumed());
        assert_eq!(KeyMatch::Held.fired(), None);
    }

    /// The media and TV remote keys: names, labels and the platforms'
    /// codes.
    mod tv {
        use super::*;

        /// Every TV key with its canonical name.
        const KEYS: &[(&str, Key)] = &[
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
        ];

        #[test]
        fn names_round_trip() {
            for &(name, key) in KEYS {
                let stroke = Keystroke::parse(name, Os::Linux).unwrap();
                assert_eq!(stroke.key, key, "{name}");
                assert_eq!(stroke.to_string(), name);
                assert!(key.is_media_or_remote(), "{name}");
                // In any case, with modifiers.
                let upper = Keystroke::parse(&name.to_ascii_uppercase(), Os::Linux).unwrap();
                assert_eq!(upper.key, key);
                let shifted = Keystroke::parse(&format!("shift+{name}"), Os::Linux).unwrap();
                assert_eq!(shifted.to_string(), format!("shift+{name}"));
            }
            assert!(!Key::Enter.is_media_or_remote());
            assert!(!Key::F(5).is_media_or_remote());
        }

        #[test]
        fn the_webs_names_are_aliases() {
            let key = |t| Keystroke::parse(t, Os::Web).unwrap().key;
            assert_eq!(key("MediaTrackNext"), Key::MediaNext);
            assert_eq!(key("MediaTrackPrevious"), Key::MediaPrevious);
            assert_eq!(key("ColorF0Red"), Key::ColorRed);
            assert_eq!(key("ColorF1Green"), Key::ColorGreen);
            assert_eq!(key("ColorF2Yellow"), Key::ColorYellow);
            assert_eq!(key("ColorF3Blue"), Key::ColorBlue);
            assert_eq!(key("MediaPlayPause"), Key::MediaPlayPause);
            assert_eq!(key("ChannelUp"), Key::ChannelUp);
            // The canonical text is the short name.
            assert_eq!(
                Keystroke::parse("colorf0red", Os::Web).unwrap().to_string(),
                "red"
            );
        }

        #[test]
        fn labels() {
            let label = |t, os, apple| Keystroke::parse_with(t, apple).unwrap().label(os, apple);
            assert_eq!(label("mediaplaypause", Os::Linux, false), "Play/Pause");
            assert_eq!(label("mediafastforward", Os::MacOs, true), "Fast Forward");
            assert_eq!(label("medianext", Os::Windows, false), "Next Track");
            assert_eq!(label("red", Os::Web, false), "Red");
            assert_eq!(label("channeldown", Os::Linux, false), "Channel Down");
            assert_eq!(label("shift+info", Os::MacOs, true), "⇧Info");
        }

        #[test]
        fn they_fire_in_text_inputs() {
            for &(name, _) in KEYS {
                let stroke = Keystroke::parse(name, Os::Linux).unwrap();
                assert!(!stroke.edits_text(), "{name}");
                assert!(KeyBinding::new(1, stroke, When::Auto, false).in_text);
                assert_eq!(stroke.reserved(Os::Linux), None);
            }
        }

        #[test]
        fn windows_virtual_keys() {
            assert_eq!(Key::from_virtual_key(0xB0), Some(Key::MediaNext));
            assert_eq!(Key::from_virtual_key(0xB1), Some(Key::MediaPrevious));
            assert_eq!(Key::from_virtual_key(0xB2), Some(Key::MediaStop));
            assert_eq!(Key::from_virtual_key(0xB3), Some(Key::MediaPlayPause));
            for (vk, key) in [
                (0xB0, Key::MediaNext),
                (0xB1, Key::MediaPrevious),
                (0xB2, Key::MediaStop),
                (0xB3, Key::MediaPlayPause),
            ] {
                assert_eq!(key.virtual_key(), Some(vk));
            }
            // Windows has no virtual keys for the remote's other keys.
            for key in [Key::MediaPlay, Key::ColorRed, Key::ChannelUp, Key::Info] {
                assert_eq!(key.virtual_key(), None, "{key:?}");
            }
        }

        #[test]
        fn android_key_codes() {
            let codes = [
                (85, Key::MediaPlayPause),
                (86, Key::MediaStop),
                (87, Key::MediaNext),
                (88, Key::MediaPrevious),
                (89, Key::MediaRewind),
                (90, Key::MediaFastForward),
                (126, Key::MediaPlay),
                (127, Key::MediaPause),
                (165, Key::Info),
                (166, Key::ChannelUp),
                (167, Key::ChannelDown),
                (183, Key::ColorRed),
                (184, Key::ColorGreen),
                (185, Key::ColorYellow),
                (186, Key::ColorBlue),
            ];
            for (code, key) in codes {
                assert_eq!(Key::from_android_keycode(code), Some(key), "{code}");
                assert_eq!(key.android_keycode(), Some(code), "{key:?}");
            }
            assert_eq!(codes.len(), KEYS.len());
        }

        #[test]
        fn hid_usages_have_none() {
            for &(_, key) in KEYS {
                assert_eq!(key.hid_usage(), None, "{key:?}");
            }
        }
    }
}
