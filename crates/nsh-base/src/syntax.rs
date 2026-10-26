//! What `TextView`'s code blocks and the `Editor` share of syntax
//! highlighting: the kinds of token and their colors, Allsum's reading
//! palette, and (feature `highlight`) syntect's syntaxes, the lookup of a
//! language's syntax and the mapping of TextMate scopes to tokens.
//!
//! Syntect names what it matched with TextMate scopes
//! (`keyword.control.rust`, `string.quoted.double`). Those map to a few
//! kinds of token, each with a light and a dark color, so one parse gives
//! the styles of both appearances and the colors follow the appearance
//! without highlighting again. What maps to nothing keeps the code's own
//! color (the app's).

use nsh_core::ui::{StyleRefinement, prelude::*};

#[cfg(feature = "highlight")]
pub(crate) use self::syntect_syntaxes::{find_syntax, loaded, plain, scope_token, syntaxes};

/// What a platform does for the built-in highlighting (`highlight`). On
/// WebAssembly the syntaxes are not in the module (0.9 MB that compression
/// barely shrinks): the platform fetches their dump when a code block, an
/// editor or a diff first highlights, and code shows plain until it has
/// it, as it does natively while the syntaxes load. Native builds write
/// the dump ([`dump`](syntax_data::dump), what `nsh package --target web`
/// publishes).
#[cfg(feature = "highlight")]
pub mod syntax_data {
    /// The syntaxes' dump.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn dump() -> Vec<u8> {
        super::syntect_syntaxes::dump()
    }

    /// What asks the platform for the dump, once, when highlighting first
    /// wants it; the platform answers with [`provide`]. Without one, code
    /// stays plain.
    #[cfg(target_arch = "wasm32")]
    pub fn on_wanted(want: fn()) {
        super::fetched::SYNTAXES.on_wanted(want);
    }

    /// The platform's answer: the dump it fetched, or `None` when it could
    /// not (code then stays plain). Wakes what waits for it; the platform
    /// then runs the engines.
    #[cfg(target_arch = "wasm32")]
    pub fn provide(dump: Option<&[u8]>) {
        super::syntect_syntaxes::set(dump);
        super::fetched::SYNTAXES.finish();
    }
}

/// Until the syntaxes are loaded, or there are none to load (code stays
/// plain): what code blocks, editors and diffs wait for before they
/// highlight on WebAssembly, where the platform fetches them.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn syntaxes_loaded() {
    #[cfg(feature = "highlight")]
    fetched::SYNTAXES.wait().await;
}

/// Waiting for something the platform fetches when it is first wanted:
/// the first wait asks for it, every wait ends once the platform answered.
/// What `highlight` waits for on WebAssembly ([`syntax_data`]).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod fetched {
    use std::{
        future::Future,
        pin::Pin,
        sync::Mutex,
        task::{Context, Poll, Waker},
    };

    /// The syntaxes ([`super::syntax_data`]).
    #[cfg(all(feature = "highlight", target_arch = "wasm32"))]
    pub(super) static SYNTAXES: Fetched = Fetched::new();

    pub(super) struct Fetched(Mutex<State>);

    struct State {
        want: Option<fn()>,
        asked: bool,
        done: bool,
        waiting: Vec<Waker>,
    }

    impl Fetched {
        pub(super) const fn new() -> Self {
            Self(Mutex::new(State {
                want: None,
                asked: false,
                done: false,
                waiting: Vec::new(),
            }))
        }

        pub(super) fn on_wanted(&self, want: fn()) {
            self.lock().want = Some(want);
        }

        /// Whether the platform answered.
        pub(super) fn done(&self) -> bool {
            self.lock().done
        }

        /// Ends every wait, now and later.
        pub(super) fn finish(&self) {
            let waiting = {
                let mut state = self.lock();
                state.done = true;
                std::mem::take(&mut state.waiting)
            };
            waiting.into_iter().for_each(Waker::wake);
        }

        /// Until the platform answered; the first wait asks it (with
        /// nothing to ask, there is nothing to wait for).
        pub(super) fn wait(&self) -> Wait<'_> {
            Wait(self)
        }

        fn lock(&self) -> std::sync::MutexGuard<'_, State> {
            self.0.lock().unwrap_or_else(|e| e.into_inner())
        }
    }

    pub(super) struct Wait<'a>(&'a Fetched);

    impl Future for Wait<'_> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            let ask = {
                let mut state = self.0.lock();
                if state.done {
                    return Poll::Ready(());
                }
                if !state.waiting.iter().any(|w| w.will_wake(cx.waker())) {
                    state.waiting.push(cx.waker().clone());
                }
                if state.asked {
                    None
                } else {
                    state.asked = true;
                    Some(state.want)
                }
            };
            match ask {
                // The platform may answer at once: outside the lock.
                Some(Some(want)) => want(),
                Some(None) => self.0.finish(),
                None => {}
            }
            if self.0.done() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
            task::Wake,
        };

        use super::*;

        struct Count(AtomicUsize);

        impl Wake for Count {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        fn poll(wait: &mut Wait<'_>, waker: &Waker) -> Poll<()> {
            Pin::new(wait).poll(&mut Context::from_waker(waker))
        }

        #[test]
        fn the_first_wait_asks_once_and_every_wait_ends_when_the_platform_answers() {
            static DATA: Fetched = Fetched::new();
            static ASKED: AtomicUsize = AtomicUsize::new(0);
            DATA.on_wanted(|| {
                ASKED.fetch_add(1, Ordering::SeqCst);
            });
            let woken = Arc::new(Count(AtomicUsize::new(0)));
            let waker = Waker::from(woken.clone());
            let (mut a, mut b) = (DATA.wait(), DATA.wait());
            assert!(poll(&mut a, &waker).is_pending());
            assert!(poll(&mut b, &waker).is_pending());
            assert!(poll(&mut a, &waker).is_pending());
            assert_eq!(ASKED.load(Ordering::SeqCst), 1, "asked once");
            assert!(!DATA.done());
            DATA.finish();
            assert_eq!(woken.0.load(Ordering::SeqCst), 1, "the one waker, once");
            assert!(poll(&mut a, &waker).is_ready());
            assert!(poll(&mut b, &waker).is_ready());
            assert!(
                poll(&mut DATA.wait(), &waker).is_ready(),
                "later waits end at once"
            );
            assert_eq!(ASKED.load(Ordering::SeqCst), 1, "never asked again");
        }

        #[test]
        fn a_platform_that_answers_at_once_ends_the_first_wait() {
            static DATA: Fetched = Fetched::new();
            DATA.on_wanted(|| DATA.finish());
            let waker = Waker::from(Arc::new(Count(AtomicUsize::new(0))));
            assert!(poll(&mut DATA.wait(), &waker).is_ready());
        }

        #[test]
        fn with_nothing_to_ask_there_is_nothing_to_wait_for() {
            static DATA: Fetched = Fetched::new();
            let waker = Waker::from(Arc::new(Count(AtomicUsize::new(0))));
            assert!(poll(&mut DATA.wait(), &waker).is_ready());
            assert!(DATA.done());
        }
    }
}

/// What a span of code is, colored alike (as Allsum's `--hl-*` tokens
/// color highlight.js's classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Without the feature nothing parses code, so nothing makes tokens.
#[cfg_attr(not(feature = "highlight"), allow(dead_code))]
pub(crate) enum Token {
    Comment,
    /// Keywords, the words of operators (`as`, `typeof`), headings.
    Keyword,
    /// Strings, regular expressions, escapes, a diff's added lines.
    String,
    /// Numbers, constants and literals (`true`, `null`).
    Number,
    /// Names of functions and classes.
    Title,
    /// Attributes, parameters, properties, keys, shell variables,
    /// `self` / `this`.
    Attr,
    /// Built-ins: types, functions; tags.
    Built,
    /// A diff's removed lines.
    Deletion,
    Bold,
    Italic,
    /// An operator: a keyword when it is a word (Rust's `as`), else
    /// plain. Settled by its text ([`Token::settle`]).
    Operator,
    /// A grammar's plain `storage.type`, which some give declarations
    /// and types alike (Rust's `let` and `f32`). Settled by its text.
    Storage,
}

#[cfg_attr(not(feature = "highlight"), allow(dead_code))]
impl Token {
    /// Allsum's reading palette (its `--hl-*` tokens), light and dark.
    pub(crate) fn style(self) -> StyleRefinement {
        let s = StyleRefinement::default();
        let color = AdaptiveColor::rgb;
        match self {
            Self::Comment => s.text_color(color(0x6e7781, 0x7d7a72)).italic(),
            Self::Keyword => s.text_color(color(0x8250df, 0xc4a7ff)),
            Self::String => s.text_color(color(0x116329, 0x9fd68a)),
            Self::Number => s.text_color(color(0x953800, 0xf5b27a)),
            Self::Title => s.text_color(color(0x0550ae, 0x82b8ff)),
            Self::Attr => s.text_color(color(0x7d4e00, 0xf0c674)),
            Self::Built => s.text_color(color(0x0a7285, 0x6fd3d8)),
            Self::Deletion => s.text_color(color(0xcf222e, 0xf87171)),
            Self::Bold => s.font_weight(FontWeight::BOLD),
            Self::Italic => s.italic(),
            Self::Operator | Self::Storage => s,
        }
    }

    /// The token of `text` for the tokens its scope leaves open.
    fn settle(self, text: &str) -> Option<Self> {
        const DECLARING: &[&str] = &[
            "let",
            "const",
            "var",
            "static",
            "mut",
            "fn",
            "func",
            "function",
            "def",
            "class",
            "struct",
            "enum",
            "union",
            "trait",
            "impl",
            "type",
            "interface",
            "module",
            "mod",
            "namespace",
            "typedef",
            "val",
            "extern",
            "async",
        ];
        match self {
            Self::Operator => text
                .chars()
                .all(|c| c.is_ascii_alphabetic())
                .then_some(Self::Keyword),
            Self::Storage if DECLARING.contains(&text) => Some(Self::Keyword),
            Self::Storage => Some(Self::Built),
            token => Some(token),
        }
    }
}

#[cfg(feature = "highlight")]
mod syntect_syntaxes {
    use std::{cell::RefCell, collections::HashMap, sync::OnceLock};

    use syntect::parsing::{Scope, ScopeStack, SyntaxReference, SyntaxSet};

    use super::Token;

    /// Deserializing the syntaxes takes tens of milliseconds: once per
    /// process, on a blocking thread where the caller can (on WebAssembly,
    /// when the platform hands them over).
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();

    thread_local! {
        /// The token of each scope met (building a scope's name is slow).
        static TOKENS: RefCell<HashMap<Scope, Option<Token>>> = RefCell::default();
    }

    /// bat's syntaxes (two-face's), loaded on the first call: syntaxes
    /// that expect each line with its line break.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn syntaxes() -> &'static SyntaxSet {
        SYNTAXES.get_or_init(two_face::syntax::extra_newlines)
    }

    /// The syntaxes the platform fetched (`syntax_data`) once they are
    /// [`loaded`]; none before (code stays plain), nor when the platform
    /// could not fetch them.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn syntaxes() -> &'static SyntaxSet {
        static NONE: OnceLock<SyntaxSet> = OnceLock::new();
        SYNTAXES
            .get()
            .unwrap_or_else(|| NONE.get_or_init(SyntaxSet::default))
    }

    /// Whether the syntaxes are loaded ([`syntaxes`] returns at once).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn loaded() -> bool {
        SYNTAXES.get().is_some()
    }

    /// Whether the platform answered (`syntax_data::provide`): with the
    /// syntaxes, or that there are none.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn loaded() -> bool {
        super::fetched::SYNTAXES.done()
    }

    /// Reads the dump the platform fetched (`None`: it could not), once.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn set(dump: Option<&[u8]>) {
        let set = dump.and_then(|dump| {
            syntect::dumps::from_reader::<SyntaxSet, _>(dump)
                .inspect_err(|e| log::warn!("the syntaxes the page fetched do not load: {e}"))
                .ok()
        });
        match set {
            Some(set) => {
                let _ = SYNTAXES.set(set);
            }
            None => log::warn!("no syntaxes: code is not highlighted"),
        }
    }

    /// The dump of the syntaxes the web runtime fetches (`syntax_data`):
    /// syntect's own format, the same bytes for the same syntaxes (it
    /// writes its maps in order).
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn dump() -> Vec<u8> {
        syntect::dumps::dump_binary(syntaxes())
    }

    /// Languages that are plain text.
    pub(crate) fn plain(language: &str) -> bool {
        ["", "text", "txt", "plain", "plaintext"]
            .iter()
            .any(|plain| language.eq_ignore_ascii_case(plain))
    }

    /// The syntax of a language: an extension or a name (case-insensitive),
    /// or one of the other names code fences use. Loads the syntaxes.
    pub(crate) fn find_syntax(language: &str) -> Option<&'static SyntaxReference> {
        let language = language.trim();
        if plain(language) {
            return None;
        }
        let alias = match language.to_ascii_lowercase().as_str() {
            "shell" | "console" | "shellscript" => "sh",
            "golang" => "go",
            // TSX reads JSX too (no JSX grammar of its own here).
            "jsx" => "tsx",
            "mjs" | "cjs" => "js",
            "c++" => "cpp",
            "c#" | "csharp" => "cs",
            "f#" | "fsharp" => "fs",
            "jsonc" | "json5" => "json",
            "objective-c" | "objc" => "m",
            "docker" => "Dockerfile",
            "make" => "Makefile",
            "vue" | "svelte" => "html",
            _ => language,
        };
        syntaxes()
            .find_syntax_by_token(alias)
            .filter(|syntax| syntax.name != "Plain Text")
    }

    /// The token of `text` (what syntect matched) under `scopes`: that of
    /// its innermost scope that has one, settled by the text.
    pub(crate) fn scope_token(scopes: &ScopeStack, text: &str) -> Option<Token> {
        let token = TOKENS.with_borrow_mut(|names| {
            scopes
                .as_slice()
                .iter()
                .rev()
                .find_map(|scope| *names.entry(*scope).or_insert_with(|| token(*scope)))
        });
        token.and_then(|token| token.settle(text.trim()))
    }

    /// The token of a TextMate scope, or `None` to let an enclosing scope
    /// decide. The website's Shiki themes (`website/src/*.theme.json`)
    /// color the same scopes alike.
    fn token(scope: Scope) -> Option<Token> {
        let scope = scope.build_string();
        let is = |prefix: &str| {
            scope
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
        };
        let token = if is("meta.object-literal.key") || is("meta.mapping.key") {
            Token::Attr
        } else if is("punctuation") || is("invalid") || is("meta") || is("source") || is("text") {
            // Delimiters belong to what they delimit; the rest are
            // containers with no color of their own.
            return None;
        } else if is("comment") {
            Token::Comment
        } else if is("string") || is("constant.character.escape") {
            Token::String
        } else if is("constant")
            || is("support.constant")
            || is("variable.other.constant")
            || is("entity.name.constant")
        {
            Token::Number
        } else if is("support.type.property-name")
            || is("entity.other.attribute-name")
            || is("variable.parameter")
            || is("variable.language")
            || is("variable.other.property")
            || is("variable.other.object.property")
            || is("variable.other.member")
            || is("entity.name.tag.yaml")
            || is("entity.name.tag.toml")
            || (is("variable.other") && scope.ends_with(".shell"))
        {
            Token::Attr
        } else if is("support.function.builtin")
            || is("support.macro")
            || is("support.type")
            || is("entity.name.tag")
        {
            Token::Built
        } else if is("keyword.operator") {
            // Words are keywords (`as`, `typeof`, `in`), symbols are plain.
            Token::Operator
        } else if is("storage.type") {
            // `storage.type.function.rust` declares, `storage.type.numeric.go`
            // is a type, `storage.type.rust` is either.
            match scope.split('.').count() {
                ..=3 => Token::Storage,
                _ if [
                    "class",
                    "enum",
                    "function",
                    "impl",
                    "interface",
                    "module",
                    "namespace",
                    "struct",
                    "trait",
                    "type.type",
                    "union",
                    "def",
                    "fn",
                ]
                .iter()
                .any(|kind| scope[13..].starts_with(kind)) =>
                {
                    Token::Keyword
                }
                _ => Token::Built,
            }
        } else if is("keyword")
            || is("storage")
            || is("entity.name.section")
            || is("markup.heading")
        {
            Token::Keyword
        } else if is("entity.name")
            || is("entity.other.inherited-class")
            || is("support.function")
            || is("support.class")
            || is("variable.function")
        {
            Token::Title
        } else if is("markup.inserted") {
            Token::String
        } else if is("markup.deleted") {
            Token::Deletion
        } else if is("markup.bold") {
            Token::Bold
        } else if is("markup.italic") {
            Token::Italic
        } else {
            return None;
        };
        Some(token)
    }
}
