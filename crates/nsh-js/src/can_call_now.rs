//! Asking whether a JS value Rust keeps can be called now
//! (`Callback::can_call_now`, `JsHandle::can_call_now`), headless: yes
//! inside a synchronous call from JS of its VM, nested or in a posted
//! turn; no after the call, in a finalizer, once the VM is gone and for a
//! value of another VM; and a call does what the answer says.

use std::cell::RefCell;

use nsh_core::{element::Registry, prelude::*};

use crate::{Host, HostOptions};

/// What `Caller.keep` keeps: a function, an object and a stand-in.
type Kept = (Callback<(), i64>, JsHandle, JsSpeaker);

thread_local! {
    /// The values `Caller.keep` keeps.
    static KEPT: RefCell<Option<Kept>> = const { RefCell::new(None) };
    /// What the callers' drops answered.
    static DROPS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Speaks.
#[interface]
pub(crate) trait Speaker {
    /// Its name.
    fn name(&self) -> Result<String, String>;
    /// Says `text`.
    fn say(&self, text: String);
}

/// Asks whether the values it is given or keeps can be called now.
#[class]
pub(crate) struct Caller {
    /// What it answered about its constructor's values (`ask`'s line).
    asked: String,
    /// The label of its drop's answer.
    label: String,
    /// A function its drop asks about (a field of its own, so the class
    /// traces it).
    watched: Option<Callback<(), i64>>,
}

impl Drop for Caller {
    fn drop(&mut self) {
        if let Some(f) = &self.watched {
            let answer = f.can_call_now();
            DROPS.with(|d| d.borrow_mut().push(format!("{} {answer}", self.label)));
        }
    }
}

/// The three answers, then what `f()` and `speaker.name()` gave where the
/// answer was yes (`-` where it was no).
fn ask(f: &Callback<(), i64>, object: &JsHandle, speaker: &JsSpeaker) -> String {
    let (a, b, c) = (
        f.can_call_now(),
        object.can_call_now(),
        speaker.object().can_call_now(),
    );
    let called = if a {
        f.call(()).map_or_else(|e| e, |v| v.to_string())
    } else {
        "-".to_owned()
    };
    let named = if c {
        speaker.name().unwrap_or_else(|e| e)
    } else {
        "-".to_owned()
    };
    format!("{a} {b} {c} {called} {named}")
}

#[class]
impl Caller {
    /// Asks about `f`, `object` and `speaker` while JS constructs it.
    pub(crate) fn new(f: Callback<(), i64>, object: JsHandle, speaker: JsSpeaker) -> Self {
        Caller {
            asked: ask(&f, &object, &speaker),
            label: String::new(),
            watched: None,
        }
    }

    /// What it answered about its values in the constructor.
    pub(crate) fn asked(&self) -> String {
        self.asked.clone()
    }

    /// Asks about `f`, `object` and `speaker` in a method of an instance.
    pub(crate) fn ask_here(
        &self,
        f: Callback<(), i64>,
        object: JsHandle,
        speaker: JsSpeaker,
    ) -> String {
        ask(&f, &object, &speaker)
    }

    /// Asks about `f`, `object` and `speaker` in a static function.
    pub(crate) fn ask(f: Callback<(), i64>, object: JsHandle, speaker: JsSpeaker) -> String {
        ask(&f, &object, &speaker)
    }

    /// Asks about the values `keep` kept (`none` without them).
    pub(crate) fn ask_kept() -> String {
        KEPT.with(|k| {
            k.borrow()
                .as_ref()
                .map_or_else(|| "none".to_owned(), |(f, o, s)| ask(f, o, s))
        })
    }

    /// Calls `f` on the spot: its result, or what it threw.
    pub(crate) fn through(f: Callback<(), String>) -> String {
        f.call(()).unwrap_or_else(|e| e)
    }

    /// Keeps `f`, `object` and `speaker` for Rust.
    pub(crate) fn keep(f: Callback<(), i64>, object: JsHandle, speaker: JsSpeaker) {
        KEPT.with(|k| *k.borrow_mut() = Some((f, object, speaker)));
    }

    /// Asks about `f` when it drops, answering under `label`.
    pub(crate) fn watch(&mut self, label: String, f: Callback<(), i64>) {
        self.label = label;
        self.watched = Some(f);
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("test/can-call-now");
    r.interface::<JsSpeaker>().class::<Caller>();
    r
}

fn host() -> Host {
    Host::with_options(HostOptions {
        ui_sets: vec![registry()],
        ..HostOptions::default()
    })
    .unwrap()
}

/// What every test's speaker starts from.
const SPEAKER: &str = r#"
globalThis.said = [];
globalThis.speaker = {
    name() { return "ann"; },
    say(text) { said.push(text); },
};
"#;

/// Runs `code` (a module importing the class, after [`SPEAKER`]); the
/// texts it shows with `show(...)`.
fn run(host: &Host, code: &str) -> Result<Vec<String>, String> {
    host.eval_source(&format!(
        "import {{ text as show }} from \"nsh:rt\";\nimport {{ Caller }} from \"test/can-call-now\";\nglobalThis.Caller = Caller;\n{code}"
    ))
    .map_err(|e| e.message)?;
    let mut ops = Vec::new();
    host.take_ops(&mut ops);
    host.apply_posts(&mut ops);
    Ok(ops
        .into_iter()
        .filter_map(|op| match op {
            nsh_core::Op::CreateText { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect())
}

/// The values `Caller.keep` kept.
fn kept() -> Kept {
    KEPT.with(|k| k.borrow_mut().take()).expect("kept")
}

#[test]
fn yes_inside_a_call_from_js_and_in_one_nested_in_it() {
    let host = host();
    run(&host, SPEAKER).unwrap();
    let shown = run(
        &host,
        r#"
        const f = () => 42;
        const c = new Caller(f, {}, speaker);
        show(c.asked());
        show(c.askHere(f, {}, speaker));
        show(Caller.ask(f, f, speaker));
        // Rust calls JS on the spot, whose JS calls Rust again.
        show(Caller.through(() => Caller.ask(() => 7, {}, speaker)));
        show(Caller.through(() => Caller.through(() => c.askHere(f, speaker, speaker))));
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "true true true 42 ann",
            "true true true 42 ann",
            "true true true 42 ann",
            "true true true 7 ann",
            "true true true 42 ann",
        ]
    );
}

#[test]
fn yes_in_a_call_from_a_posted_turn() {
    let host = host();
    run(&host, SPEAKER).unwrap();
    run(
        &host,
        r#"
        globalThis.answers = [];
        Caller.keep(() => {
            answers.push(Caller.askKept());
            answers.push(Caller.ask(() => 3, {}, speaker));
            return 0;
        }, {}, speaker);
        "#,
    )
    .unwrap();
    let (posted, _, _) = kept();
    run(&host, "Caller.keep(() => 1, {}, speaker);").unwrap();
    // Rust posts a function; JS runs it in its next turn, calling Rust,
    // which asks about the values it kept and about new ones.
    posted.post(());
    host.run_posted_without_core();
    assert_eq!(
        run(&host, "show(answers.join(' | '));").unwrap(),
        ["true true true 1 ann | true true true 3 ann"]
    );
}

#[test]
fn no_after_the_call_returned_and_members_do_what_that_says() {
    let host = host();
    run(&host, SPEAKER).unwrap();
    run(
        &host,
        "globalThis.calls = 0; Caller.keep(() => ++calls, {}, speaker);",
    )
    .unwrap();
    let (f, object, speaker) = kept();
    assert!(!f.can_call_now());
    assert!(!object.can_call_now());
    assert!(!speaker.object().can_call_now());
    // A member returning `Result` fails; one returning `()` is posted.
    assert_eq!(
        speaker.name(),
        Err(
            "Speaker.name: called outside a synchronous call from JS (an `async` method is \
             called from a task)"
                .to_owned()
        )
    );
    speaker.say("later".to_owned());
    f.post(());
    assert_eq!(
        run(&host, "show(`${said.join(',')} ${calls}`);").unwrap(),
        [" 0"]
    );
    host.run_posted_without_core();
    assert_eq!(
        run(&host, "show(`${said.join(',')} ${calls}`);").unwrap(),
        ["later 1"]
    );
}

#[test]
fn no_in_a_finalizer_inside_a_call() {
    let host = host();
    run(&host, SPEAKER).unwrap();
    // Inside one call from JS: a caller freed by its count, then cycles
    // only the collector frees, made to run by what the call allocates.
    // Each drop asks about a live function; the call itself still may.
    let shown = run(
        &host,
        r#"
        const live = () => 1;
        for (let i = 0; i < 20; i++) {
            const c = new Caller(live, {}, speaker);
            c.watch("cycle", () => c.asked().length);
        }
        show(Caller.through(() => {
            let freed = new Caller(live, {}, speaker);
            freed.watch("count", live);
            freed = null;
            const junk = [];
            for (let i = 0; i < 300000; i++) junk.push({ i });
            return Caller.ask(live, {}, speaker);
        }));
        "#,
    )
    .unwrap();
    assert_eq!(shown, ["true true true 1 ann"]);
    let drops = DROPS.with(|d| std::mem::take(&mut *d.borrow_mut()));
    assert!(drops.contains(&"count false".to_owned()), "{drops:?}");
    // (The loop's last binding may outlive it.)
    let cycles = drops.iter().filter(|d| d.starts_with("cycle")).count();
    assert!(cycles >= 19, "collected in the call: {drops:?}");
    assert!(drops.iter().all(|d| d.ends_with(" false")), "{drops:?}");
    assert!(!host.has_exception());
}

#[test]
fn no_once_the_vm_is_gone() {
    let host = host();
    run(&host, SPEAKER).unwrap();
    run(&host, "Caller.keep(() => 1, {}, speaker);").unwrap();
    let (f, object, speaker) = kept();
    drop(host);
    assert!(!f.can_call_now());
    assert!(!object.can_call_now());
    assert!(!speaker.object().can_call_now());
}

#[test]
fn no_for_a_value_of_another_vm_while_that_one_is_in_a_call() {
    let first = host();
    run(&first, SPEAKER).unwrap();
    run(&first, "Caller.keep(() => 1, {}, speaker);").unwrap();
    // The first VM's values, asked about inside a call of the second.
    let second = host();
    run(&second, SPEAKER).unwrap();
    let shown = run(
        &second,
        r#"
        show(Caller.askKept());
        show(Caller.ask(() => 2, {}, speaker));
        "#,
    )
    .unwrap();
    assert_eq!(shown, ["false false false - -", "true true true 2 ann"]);
    drop(second);
    let (f, object, speaker) = kept();
    drop(first);
    drop((f, object, speaker));
}
