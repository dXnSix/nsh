//! nshx only (the dXnSix fork of nsh): where a released nshx CLI takes
//! nsh's crates for a TSX app's trimmed release runner
//! ([`super::runner_only`]).
//!
//! Upstream's CLI takes them from the checkout it was built in, else from
//! crates.io at its version. crates.io has upstream's crates only, and the
//! fork keeps upstream's version, so a released nshx CLI would build the
//! runner from upstream's nsh, without the fork's changes. The fork's
//! release build sets `NSHX_TAG` to the release's tag
//! (`.github/workflows/fork-release.yml`); a CLI built with it takes nsh's
//! crates from the fork's repository at that tag, as an app's crates that
//! name nsh from Git do ([`Nsh::Git`]). A CLI built without it (upstream's,
//! a fork build that is no release) is unchanged.

use std::{fs, path::Path};

use anyhow::{Context as _, bail};

use super::{AppCrate, Nsh, VERSION};

/// The fork's repository.
const REPOSITORY: &str = "https://github.com/dXnSix/nsh";

/// The nshx release this CLI was built for (`nshx-<version>-<n>`), if any.
const TAG: Option<&str> = option_env!("NSHX_TAG");

/// Where the runner takes nsh's crates from: `nsh`, as upstream chose it,
/// unless that is crates.io and this CLI is an nshx release.
pub(super) fn runner_nsh(nsh: Nsh, workspace: &Path) -> anyhow::Result<Nsh> {
    choose(nsh, TAG, REPOSITORY, workspace)
}

fn choose(nsh: Nsh, tag: Option<&str>, url: &str, workspace: &Path) -> anyhow::Result<Nsh> {
    match (nsh, tag) {
        (Nsh::Registry, Some(tag)) => at_tag(url, tag, workspace),
        (nsh, _) => Ok(nsh),
    }
}

/// The crate `<workspace>/nshx-tag`, which names nsh's crates from `url` at
/// `tag`. Cargo fetches that checkout as for an app's Git dependencies.
fn probe_manifest(url: &str, tag: &str) -> String {
    format!(
        "# nsh's crates for the release runner of an nshx CLI, from the fork's\n\
         # repository at the CLI's tag.\n\
         [package]\nname = \"nshx-tag\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\nnsh-core = {{ git = {url:?}, tag = {tag:?} }}\n\n\
         # Its own workspace, apart from the generated one around it.\n[workspace]\n"
    )
}

/// nsh's crates from `url` at `tag`: Cargo's checkout of it ([`Nsh::of`]).
fn at_tag(url: &str, tag: &str, workspace: &Path) -> anyhow::Result<Nsh> {
    let dir = workspace.join("nshx-tag");
    fs::create_dir_all(dir.join("src")).with_context(|| format!("create {}", dir.display()))?;
    fs::write(dir.join("Cargo.toml"), probe_manifest(url, tag))?;
    fs::write(dir.join("src/lib.rs"), "")?;
    let nsh = Nsh::of(&[AppCrate {
        package: "nshx-tag".into(),
        dir,
    }])
    .with_context(|| format!("nsh's crates from {url} at {tag}"))?;
    if !matches!(nsh, Nsh::Git { .. }) {
        bail!("{url} at {tag} has nsh's crates at another version than this CLI's ({VERSION})");
    }
    Ok(nsh)
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        process::Command,
    };

    use super::{Nsh, choose};

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=NSH test",
                "-c",
                "user.email=test@example.com",
            ])
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A repository of nsh's crates at `version`, tagged `tag`, and its URL.
    fn repository(dir: &Path, version: &str, tag: &str) -> String {
        let repo = dir.join("repo");
        for name in ["nsh-core", "nsh-base"] {
            let krate = repo.join("crates").join(name);
            std::fs::create_dir_all(krate.join("src")).unwrap();
            std::fs::write(
                krate.join("Cargo.toml"),
                format!("[package]\nname = {name:?}\nversion.workspace = true\n"),
            )
            .unwrap();
            std::fs::write(krate.join("src/lib.rs"), "").unwrap();
        }
        std::fs::write(
            repo.join("Cargo.toml"),
            format!(
                "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n[workspace.package]\nversion = {version:?}\n"
            ),
        )
        .unwrap();
        git(&repo, &["init"]);
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "fixture"]);
        git(&repo, &["tag", tag]);
        let path = dunce::canonicalize(&repo).unwrap();
        format!(
            "{}{}",
            if cfg!(windows) { "file:///" } else { "file://" },
            path.to_string_lossy().replace('\\', "/")
        )
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nsh-nshx-tag-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_release_takes_the_fork_s_crates_at_its_tag() {
        let dir = scratch("release");
        let url = repository(&dir, super::VERSION, "nshx-test-1");
        let ws = dir.join("app/.nsh/rust");
        let nsh = choose(Nsh::Registry, Some("nshx-test-1"), &url, &ws).unwrap();
        let probe = std::fs::read_to_string(ws.join("nshx-tag/Cargo.toml")).unwrap();
        assert!(
            probe.contains(&format!(
                "nsh-core = {{ git = {url:?}, tag = \"nshx-test-1\" }}"
            )),
            "{probe}"
        );
        let Nsh::Git {
            checkout,
            url: resolved,
        } = &nsh
        else {
            panic!("a release takes the fork's crates from Git: {nsh:?}");
        };
        assert_eq!(resolved, &url);
        assert!(checkout.join("crates/nsh-base/Cargo.toml").is_file());
        assert_ne!(
            checkout,
            &dir.join("repo"),
            "Cargo's checkout, not the repository"
        );
        // The generated manifests take nsh's crates from that checkout.
        assert!(
            nsh.dep("nsh-core", "")
                .contains(&*checkout.join("crates/nsh-core").to_string_lossy()),
            "{}",
            nsh.dep("nsh-core", "")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_fork_names_its_repository() {
        assert_eq!(super::REPOSITORY, "https://github.com/dXnSix/nsh");
        let probe = super::probe_manifest(super::REPOSITORY, "nshx-0.5.0-2");
        assert!(probe.contains(
            "nsh-core = { git = \"https://github.com/dXnSix/nsh\", tag = \"nshx-0.5.0-2\" }"
        ));
        assert!(probe.contains("\n[workspace]\n"), "{probe}");
    }

    #[test]
    fn a_tag_at_another_version_fails() {
        let dir = scratch("version");
        let url = repository(&dir, "0.0.1", "nshx-0.0.1-1");
        let err = choose(Nsh::Registry, Some("nshx-0.0.1-1"), &url, &dir.join("ws")).unwrap_err();
        assert!(format!("{err:#}").contains("another version"), "{err:#}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_a_tag_crates_io_as_before() {
        let dir = scratch("upstream");
        let nsh = choose(Nsh::Registry, None, "file:///nowhere", &dir).unwrap();
        assert_eq!(nsh, Nsh::Registry);
        assert!(!dir.join("nshx-tag").exists(), "nothing fetched");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_checkout_keeps_the_checkout() {
        let dir = scratch("checkout");
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for tag in [None, Some("nshx-0.5.0-2")] {
            let nsh = choose(Nsh::Source(checkout.clone()), tag, "file:///nowhere", &dir).unwrap();
            assert_eq!(nsh, Nsh::Source(checkout.clone()));
        }
        assert!(!dir.join("nshx-tag").exists(), "nothing fetched");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
