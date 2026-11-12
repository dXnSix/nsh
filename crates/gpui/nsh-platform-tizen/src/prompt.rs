//! How the question whether to leave the app ([`crate::back`]) reads, for
//! every TV user the app serves:
//!
//! - its words are in the TV's language (the locale the crate gives
//!   `AppView`), from a small table of the TVs' major languages ([`words`]),
//!   English for the others;
//! - its sizes follow the TV's font scale ([`scaled`]);
//! - it is an alert dialog for assistive technologies, named by its
//!   question, with its two answers as buttons, the chosen one the focused
//!   node ([`access`]): Voice Guide, which follows AT-SPI's focus, reads the
//!   question and the answer as it opens and each answer as it is chosen.
//!
//! `crate::app` draws it with these.

use gpui::accesskit::Role;
use nsh_platform_gpui::Locale;

use crate::back::Choice;

/// The question's words in one language: the question naming the app
/// (`{name}`), the question without a name, and the two answers.
struct Table {
    named: &'static str,
    unnamed: &'static str,
    stay: &'static str,
    exit: &'static str,
}

const ENGLISH: Table = Table {
    named: "Exit {name}?",
    unnamed: "Exit the app?",
    stay: "Stay",
    exit: "Exit",
};

const SIMPLIFIED_CHINESE: Table = Table {
    named: "要退出{name}吗？",
    unnamed: "要退出应用吗？",
    stay: "取消",
    exit: "退出",
};

const TRADITIONAL_CHINESE: Table = Table {
    named: "要結束{name}嗎？",
    unnamed: "要結束應用程式嗎？",
    stay: "取消",
    exit: "結束",
};

const KOREAN: Table = Table {
    named: "{name}을(를) 종료할까요?",
    unnamed: "앱을 종료할까요?",
    stay: "취소",
    exit: "종료",
};

const JAPANESE: Table = Table {
    named: "{name}を終了しますか？",
    unnamed: "アプリを終了しますか？",
    stay: "キャンセル",
    exit: "終了",
};

const SPANISH: Table = Table {
    named: "¿Salir de {name}?",
    unnamed: "¿Salir de la aplicación?",
    stay: "Quedarse",
    exit: "Salir",
};

const FRENCH: Table = Table {
    named: "Quitter {name}\u{a0}?",
    unnamed: "Quitter l’application\u{a0}?",
    stay: "Rester",
    exit: "Quitter",
};

const GERMAN: Table = Table {
    named: "{name} beenden?",
    unnamed: "App beenden?",
    stay: "Bleiben",
    exit: "Beenden",
};

const PORTUGUESE: Table = Table {
    named: "Sair de {name}?",
    unnamed: "Sair do aplicativo?",
    stay: "Ficar",
    exit: "Sair",
};

const RUSSIAN: Table = Table {
    named: "Выйти из {name}?",
    unnamed: "Выйти из приложения?",
    stay: "Остаться",
    exit: "Выйти",
};

/// The table of `locale`'s language, English when there is none for it.
fn table(locale: Option<&Locale>) -> &'static Table {
    let Some(locale) = locale else {
        return &ENGLISH;
    };
    match locale.language() {
        "zh" => {
            // The script when the locale names one, else the region's.
            let script = locale
                .as_str()
                .split('-')
                .skip(1)
                .find(|tag| tag.len() == 4);
            let traditional = match script {
                Some(script) => script.eq_ignore_ascii_case("Hant"),
                None => matches!(locale.region(), Some("TW" | "HK" | "MO")),
            };
            if traditional {
                &TRADITIONAL_CHINESE
            } else {
                &SIMPLIFIED_CHINESE
            }
        }
        "ko" => &KOREAN,
        "ja" => &JAPANESE,
        "es" => &SPANISH,
        "fr" => &FRENCH,
        "de" => &GERMAN,
        "pt" => &PORTUGUESE,
        "ru" => &RUSSIAN,
        _ => &ENGLISH,
    }
}

/// The question's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Words {
    /// The question.
    pub(crate) title: String,
    /// The answer that stays.
    pub(crate) stay: &'static str,
    /// The answer that leaves.
    pub(crate) exit: &'static str,
}

impl Words {
    /// The words of `choice`.
    pub(crate) fn answer(&self, choice: Choice) -> &'static str {
        match choice {
            Choice::Stay => self.stay,
            Choice::Exit => self.exit,
        }
    }
}

/// The question in `locale`'s language, naming the app `name` (`nsh.json`
/// `"name"`) when it has one.
pub(crate) fn words(locale: Option<&Locale>, name: Option<&str>) -> Words {
    let table = table(locale);
    let title = match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => table.named.replace("{name}", name),
        None => table.unnamed.to_owned(),
    };
    Words {
        title,
        stay: table.stay,
        exit: table.exit,
    }
}

/// `pixels` (a size of the question at the default text size) at the TV's
/// font scale (`None`: the default).
pub(crate) fn scaled(pixels: f32, font_scale: Option<f32>) -> f32 {
    pixels * font_scale.unwrap_or(1.0)
}

/// An accessibility node of the question.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    pub(crate) role: Role,
    pub(crate) label: String,
    /// Reported as the focused node while the question has the keys.
    pub(crate) focused: bool,
}

/// The question's nodes: the dialog, then its answers (Stay, Exit), the
/// chosen one focused.
pub(crate) fn access(words: &Words, chosen: Choice) -> (Node, [Node; 2]) {
    let answer = |choice: Choice| Node {
        role: Role::Button,
        label: words.answer(choice).to_owned(),
        focused: choice == chosen,
    };
    (
        Node {
            role: Role::AlertDialog,
            label: words.title.clone(),
            focused: false,
        },
        [answer(Choice::Stay), answer(Choice::Exit)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale(tag: &str) -> Option<Locale> {
        Some(Locale::parse(tag).expect(tag))
    }

    #[test]
    fn english_by_default_and_for_languages_without_words() {
        let english = Words {
            title: "Exit Movies?".into(),
            stay: "Stay",
            exit: "Exit",
        };
        assert_eq!(words(None, Some("Movies")), english);
        assert_eq!(words(locale("en-GB").as_ref(), Some("Movies")), english);
        assert_eq!(words(locale("sw-KE").as_ref(), Some("Movies")), english);
        // No name (or a blank one): the app.
        assert_eq!(words(None, None).title, "Exit the app?");
        assert_eq!(words(None, Some("  ")).title, "Exit the app?");
    }

    #[test]
    fn chinese_in_its_script() {
        let simplified = words(locale("zh-CN").as_ref(), Some("影视"));
        assert_eq!(simplified.title, "要退出影视吗？");
        assert_eq!((simplified.stay, simplified.exit), ("取消", "退出"));
        assert_eq!(
            words(locale("zh-Hans-CN").as_ref(), None).title,
            "要退出应用吗？"
        );
        assert_eq!(words(locale("zh").as_ref(), None).exit, "退出");
        for tag in ["zh-TW", "zh-HK", "zh-Hant", "zh-Hant-TW", "zh_TW"] {
            let traditional = words(locale(tag).as_ref(), None);
            assert_eq!(traditional.title, "要結束應用程式嗎？", "{tag}");
            assert_eq!(traditional.exit, "結束", "{tag}");
        }
        // Simplified script named in Hong Kong.
        assert_eq!(words(locale("zh-Hans-HK").as_ref(), None).exit, "退出");
    }

    #[test]
    fn the_tvs_other_major_languages() {
        let name = Some("TV+");
        for (tag, title, stay, exit) in [
            ("ko-KR", "TV+을(를) 종료할까요?", "취소", "종료"),
            ("ja-JP", "TV+を終了しますか？", "キャンセル", "終了"),
            ("es-ES", "¿Salir de TV+?", "Quedarse", "Salir"),
            ("fr-FR", "Quitter TV+\u{a0}?", "Rester", "Quitter"),
            ("de-DE", "TV+ beenden?", "Bleiben", "Beenden"),
            ("pt-BR", "Sair de TV+?", "Ficar", "Sair"),
            ("ru-RU", "Выйти из TV+?", "Остаться", "Выйти"),
        ] {
            assert_eq!(
                words(locale(tag).as_ref(), name),
                Words {
                    title: title.into(),
                    stay,
                    exit
                },
                "{tag}"
            );
        }
    }

    #[test]
    fn sizes_follow_the_font_scale() {
        assert_eq!(scaled(24.0, None), 24.0);
        assert_eq!(scaled(24.0, Some(1.5)), 36.0);
        assert_eq!(scaled(18.0, Some(0.8)), 18.0 * 0.8);
    }

    #[test]
    fn an_alert_dialog_named_by_its_question_with_the_chosen_answer_focused() {
        let words = words(locale("zh-CN").as_ref(), Some("影视"));
        let (dialog, [stay, exit]) = access(&words, Choice::Stay);
        assert_eq!(dialog.role, Role::AlertDialog);
        assert_eq!(dialog.label, "要退出影视吗？");
        assert!(!dialog.focused);
        assert_eq!((stay.role, stay.label.as_str()), (Role::Button, "取消"));
        assert_eq!((exit.role, exit.label.as_str()), (Role::Button, "退出"));
        assert!(stay.focused && !exit.focused);
        let (_, [stay, exit]) = access(&words, Choice::Exit);
        assert!(!stay.focused && exit.focused);
    }
}
