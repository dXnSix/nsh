//! `--idiom` and `nsh.json` `dev.idiom`: the idiom the GPUI host and the web
//! dev page run the app in (a TV's on the desktop), what each host is given,
//! and the hosts that take none.
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// An app with `config` as its `nsh.json`, and a runtime (`NSH_RUNTIME`)
/// whose hosts write the arguments they get to `args.txt`.
struct App(PathBuf);

impl Drop for App {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl App {
    fn new(config: &str) -> App {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = App(std::env::temp_dir().join(format!(
            "nsh-cli-dev-idiom-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir_all(&dir.0).unwrap();
        fs::write(
            dir.0.join("App.tsx"),
            "import { mount, Text } from 'nsh'; mount(() => <Text>Hello</Text>);",
        )
        .unwrap();
        fs::write(dir.0.join("nsh.json"), config).unwrap();
        for (platform, host) in [("gpui", "nsh-gpui-host"), ("macos", "nsh-macos-host")] {
            let path = dir.0.join("runtime").join(platform).join(host);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let args = dir.0.join("args.txt");
            fs::write(
                &path,
                format!("#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}'\n", args.display()),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        // The web runtime's files, as the page names them.
        let web = dir.0.join("runtime/web");
        fs::create_dir_all(&web).unwrap();
        fs::write(web.join("nsh.js"), "export function createPlayer() {}").unwrap();
        fs::write(web.join("nsh-dev.wasm"), "wasm").unwrap();
        dir
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nsh"));
        command
            .args(args)
            .arg(&self.0)
            .env("NSH_RUNTIME", self.0.join("runtime"))
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Runs `nsh` with `args` and the app until its host exits.
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    /// The arguments the last host got.
    fn host_args(&self) -> String {
        fs::read_to_string(self.0.join("args.txt")).unwrap_or_default()
    }

    /// The page the dev server of `nsh <args>` serves to browsers.
    fn dev_page(&self, args: &[&str]) -> String {
        let mut child = Kill(self.command(args).spawn().unwrap());
        let port = served_port(&mut child.0);
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        write!(
            stream,
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut page = Vec::new();
        let mut chunk = [0; 8192];
        while !String::from_utf8_lossy(&page).contains("</html>") {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => page.extend_from_slice(&chunk[..n]),
            }
        }
        String::from_utf8(page).unwrap()
    }
}

/// A command stopped with the test.
struct Kill(Child);

impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The port of the page the command prints (`http://127.0.0.1:<port>/`).
fn served_port(child: &mut Child) -> u16 {
    let (tx, rx) = mpsc::channel();
    let stderr = child.stderr.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let start = Instant::now();
    let mut seen = String::new();
    while start.elapsed() < Duration::from_secs(60) {
        let Ok(line) = rx.recv_timeout(Duration::from_millis(200)) else {
            assert!(child.try_wait().unwrap().is_none(), "nsh exited:\n{seen}");
            continue;
        };
        seen.push_str(&line);
        seen.push('\n');
        if let Some((_, rest)) = line.split_once("http://127.0.0.1:") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            return digits.parse().unwrap();
        }
    }
    panic!("no page address in:\n{seen}");
}

fn succeeded(output: &Output) -> String {
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{all}");
    all
}

fn has_idiom(args: &str, idiom: &str) -> bool {
    args.contains(&format!("--idiom {idiom}"))
}

const GPUI: &[&str] = &["run", "--target", "gpui", "--port", "0"];

#[test]
fn the_gpui_host_runs_in_the_idiom_of_the_flag_or_nsh_json() {
    // Neither: the host's own (no argument).
    let app = App::new("{}");
    succeeded(&app.run(GPUI));
    let args = app.host_args();
    assert!(args.contains("--dev-server ws://127.0.0.1:"), "{args}");
    assert!(!args.contains("--idiom"), "{args}");
    // The flag.
    succeeded(&app.run(&[GPUI, &["--idiom", "tv"]].concat()));
    assert!(has_idiom(&app.host_args(), "tv"), "{}", app.host_args());
    // nsh.json's development setting.
    let app = App::new(r#"{ "dev": { "idiom": "tv" } }"#);
    succeeded(&app.run(GPUI));
    assert!(has_idiom(&app.host_args(), "tv"), "{}", app.host_args());
    // The flag wins over nsh.json.
    succeeded(&app.run(&[GPUI, &["--idiom", "desktop"]].concat()));
    let args = app.host_args();
    assert!(
        has_idiom(&args, "desktop") && !has_idiom(&args, "tv"),
        "{args}"
    );
}

#[test]
fn hosts_that_take_no_idiom_refuse_the_flag_before_starting() {
    let app = App::new("{}");
    for target in [
        "macos", "linux", "windows", "catalyst", "ios", "android", "tui",
    ] {
        let output = app.run(&["run", "--target", target, "--port", "0", "--idiom", "tv"]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "--target {target} accepted it");
        assert!(
            stderr.contains("--idiom needs --target gpui or web"),
            "--target {target}: {stderr}"
        );
        assert!(
            app.host_args().is_empty(),
            "--target {target} started a host"
        );
    }
    // `nsh dev` the same.
    let output = app.run(&["dev", "--target", "macos", "--port", "0", "--idiom", "tv"]);
    assert!(!output.status.success());
    // A value no host takes.
    let output = app.run(&[GPUI, &["--idiom", "phone"]].concat());
    assert!(!output.status.success());
    assert!(app.host_args().is_empty());
}

#[test]
fn nsh_json_idiom_is_left_out_with_a_warning_where_it_does_not_apply() {
    // Another host: the app still runs, in its host's own idiom.
    let app = App::new(r#"{ "dev": { "idiom": "tv" } }"#);
    let shown = succeeded(&app.run(&["run", "--target", "macos", "--port", "0"]));
    assert!(
        shown.contains("nsh.json dev.idiom \"tv\" applies to --target gpui and web"),
        "{shown}"
    );
    assert!(!app.host_args().contains("--idiom"), "{}", app.host_args());
    // A value of a form it does not know (a newer NSH's, or a typo).
    let app = App::new(r#"{ "dev": { "idiom": "phone" } }"#);
    let shown = succeeded(&app.run(GPUI));
    assert!(shown.contains("dev.idiom"), "{shown}");
    assert!(!app.host_args().contains("--idiom"), "{}", app.host_args());
}

#[test]
fn the_web_dev_page_gives_the_player_the_idiom() {
    const WEB: &[&str] = &["run", "--target", "web", "--port", "0"];
    let app = App::new("{}");
    let page = app.dev_page(WEB);
    // The player measures it when the session sets none.
    assert!(page.contains("const IDIOM = null;"), "{page}");
    assert!(page.contains("idiom: IDIOM ?? undefined"), "{page}");
    let page = app.dev_page(&[WEB, &["--idiom", "tv"]].concat());
    assert!(page.contains("const IDIOM = \"tv\";"), "{page}");
    // nsh.json's, the flag over it, and the page `--server-only` serves.
    let app = App::new(r#"{ "dev": { "idiom": "tv" } }"#);
    assert!(app.dev_page(WEB).contains("const IDIOM = \"tv\";"));
    let page = app.dev_page(&[WEB, &["--idiom", "desktop"]].concat());
    assert!(page.contains("const IDIOM = \"desktop\";"), "{page}");
    let page = app.dev_page(&["dev", "--server-only", "--port", "0"]);
    assert!(page.contains("const IDIOM = \"tv\";"), "{page}");
}
