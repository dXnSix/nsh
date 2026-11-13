//! The Back key on a TV. Back is NSH's back navigation first: the newest
//! modal closes, the innermost stack pops, the app's `onBackPress` hears it.
//! When nothing takes it (the app is on its first screen), Samsung's rule
//! for TV apps applies: the app asks whether to leave instead of closing
//! at once. The question offers to stay or to exit, staying chosen; Left
//! and Right choose, Enter answers, Back (or Escape) stays.
//!
//! [`ExitPrompt`] is that question's state; `crate::app` draws it and
//! performs what it returns.

use nsh_core::keymap::Key;

use crate::keys::Remote;

/// An answer of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    /// Stay in the app (chosen first: a second Back press never leaves).
    Stay,
    /// Leave the app.
    Exit,
}

/// What the shell does after a key or an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    /// Nothing changed.
    None,
    /// Show the question (it takes the keys), or draw its new choice.
    Show,
    /// Hide it and give the keys back to the app.
    Hide,
    /// Leave the app.
    Exit,
}

/// The question whether to leave the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ExitPrompt {
    /// The chosen answer while it is shown.
    open: Option<Choice>,
}

impl ExitPrompt {
    /// The chosen answer, while the question is shown.
    pub(crate) fn shown(&self) -> Option<Choice> {
        self.open
    }

    /// A back press nothing in the app took: ask, staying chosen.
    pub(crate) fn unhandled_back(&mut self) -> Effect {
        if self.open.is_some() {
            return Effect::None;
        }
        self.open = Some(Choice::Stay);
        Effect::Show
    }

    /// A key while the question is shown (it has the keys then; others
    /// are ignored).
    pub(crate) fn key(&mut self, key: Remote) -> Effect {
        let Some(choice) = self.open else {
            return Effect::None;
        };
        match key {
            Remote::Back | Remote::Key(Key::Escape) => {
                self.open = None;
                Effect::Hide
            }
            Remote::Key(Key::Left | Key::Right) => {
                self.open = Some(match choice {
                    Choice::Stay => Choice::Exit,
                    Choice::Exit => Choice::Stay,
                });
                Effect::Show
            }
            Remote::Key(Key::Enter | Key::Char(' ')) => self.answer(choice),
            _ => Effect::None,
        }
    }

    /// `choice` was picked (Enter, or pointed at on a remote with a
    /// pointer).
    pub(crate) fn answer(&mut self, choice: Choice) -> Effect {
        if self.open.is_none() {
            return Effect::None;
        }
        self.open = None;
        match choice {
            Choice::Stay => Effect::Hide,
            Choice::Exit => Effect::Exit,
        }
    }

    /// The app was paused (the TV showed something else): the question
    /// goes, so the app comes back as it was.
    pub(crate) fn paused(&mut self) -> Effect {
        match self.open.take() {
            Some(_) => Effect::Hide,
            None => Effect::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_with_nothing_to_go_back_to_asks_staying_chosen() {
        let mut prompt = ExitPrompt::default();
        assert_eq!(prompt.shown(), None);
        assert_eq!(prompt.unhandled_back(), Effect::Show);
        assert_eq!(prompt.shown(), Some(Choice::Stay));
        // Asked once.
        assert_eq!(prompt.unhandled_back(), Effect::None);
        // Enter on the first choice stays.
        assert_eq!(prompt.key(Remote::Key(Key::Enter)), Effect::Hide);
        assert_eq!(prompt.shown(), None);
    }

    #[test]
    fn left_and_right_choose_and_enter_answers() {
        let mut prompt = ExitPrompt::default();
        prompt.unhandled_back();
        assert_eq!(prompt.key(Remote::Key(Key::Right)), Effect::Show);
        assert_eq!(prompt.shown(), Some(Choice::Exit));
        assert_eq!(prompt.key(Remote::Key(Key::Left)), Effect::Show);
        assert_eq!(prompt.shown(), Some(Choice::Stay));
        prompt.key(Remote::Key(Key::Left));
        assert_eq!(prompt.key(Remote::Key(Key::Enter)), Effect::Exit);
        assert_eq!(prompt.shown(), None);
    }

    #[test]
    fn back_again_stays_and_other_keys_do_nothing() {
        let mut prompt = ExitPrompt::default();
        prompt.unhandled_back();
        prompt.key(Remote::Key(Key::Right));
        for key in [
            Remote::Key(Key::Up),
            Remote::Key(Key::ColorRed),
            Remote::Key(Key::Char('a')),
            Remote::Tv,
        ] {
            assert_eq!(prompt.key(key), Effect::None, "{key:?}");
        }
        assert_eq!(prompt.shown(), Some(Choice::Exit));
        assert_eq!(prompt.key(Remote::Back), Effect::Hide);
        assert_eq!(prompt.shown(), None);
        // Escape (a keyboard on the TV) stays too.
        prompt.unhandled_back();
        assert_eq!(prompt.key(Remote::Key(Key::Escape)), Effect::Hide);
    }

    #[test]
    fn keys_and_answers_without_the_question_do_nothing() {
        let mut prompt = ExitPrompt::default();
        assert_eq!(prompt.key(Remote::Key(Key::Enter)), Effect::None);
        assert_eq!(prompt.answer(Choice::Exit), Effect::None);
        assert_eq!(prompt.paused(), Effect::None);
    }

    #[test]
    fn pausing_or_pointing_answers() {
        let mut prompt = ExitPrompt::default();
        prompt.unhandled_back();
        assert_eq!(prompt.paused(), Effect::Hide);
        assert_eq!(prompt.shown(), None);
        prompt.unhandled_back();
        assert_eq!(prompt.answer(Choice::Exit), Effect::Exit);
        prompt.unhandled_back();
        assert_eq!(prompt.answer(Choice::Stay), Effect::Hide);
    }
}
