//! The symbol policy of a TV library, checked on an app's `libnsh.so` as
//! dXn.GpuiTizen's `cargo xtask check-symbols` checks the runtime's
//! (`xtask/src/symcheck/mod.rs` there): exactly one export, the entry
//! (`nsh_tizen_main`), nothing exported weak, and imports only of the
//! documented Tizen symbols of its `symbols.allow` (which the `tizen`
//! component ships, `runtime/tizen/targets/symbols.allow`), glibc,
//! `libgcc_s` and the loader. The input is the nightly's `llvm-nm -D
//! --defined-only`, `llvm-nm -D --undefined-only` and `llvm-readobj
//! --needed-libs` output (its `llvm-tools`); parsing and checking are pure.

use std::{fmt, fs, path::Path, process::Command};

use anyhow::{Context as _, bail};

/// Symbol-version prefixes of the system libraries a TV library may import
/// from: every glibc library and the loader version theirs `GLIBC_*`,
/// `libgcc_s` its own `GCC_*`.
const SYSTEM_VERSION_PREFIXES: &[&str] = &["GLIBC_", "GCC_"];

/// Unversioned weak imports of the C runtime's start files and Rust's
/// `std` that the loader resolves (or leaves null) without a library:
/// 32-bit glibc's 64-bit-time variants `std` falls back from, and glibc
/// functions newer than the TVs' 2.30 that `std::process` binds weakly.
const LOADER_WEAK_IMPORTS: &[&str] = &[
    "__gmon_start__",
    "_ITM_deregisterTMCloneTable",
    "_ITM_registerTMCloneTable",
    "__clock_gettime64",
    "__futimens64",
    "__pthread_get_minstack",
    "pidfd_getpid",
    "pidfd_spawnp",
    "posix_spawn_file_actions_addchdir",
];

/// The system libraries a TV library may need besides Tizen's: glibc's,
/// `libgcc_s` (and the loader, `ld-linux*`).
const SYSTEM_LIBRARIES: &[&str] = &[
    "libc.so.6",
    "libm.so.6",
    "libdl.so.2",
    "libpthread.so.0",
    "librt.so.1",
    "libgcc_s.so.1",
];

/// dXn.GpuiTizen's `symbols.allow`: the Tizen symbols a TV library may
/// import (`<symbol> <module>` lines) and the libraries they come from (the
/// `# lib<name>.so` line heading each group).
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Allowlist {
    pub(crate) symbols: Vec<String>,
    pub(crate) libraries: Vec<String>,
}

impl Allowlist {
    pub(crate) fn parse(text: &str) -> Allowlist {
        let mut allowlist = Allowlist::default();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            match line.strip_prefix('#') {
                Some(comment) => {
                    let comment = comment.trim();
                    if let Some(name) = comment
                        .strip_prefix("lib")
                        .and_then(|rest| rest.strip_suffix(".so"))
                        .filter(|name| !name.is_empty() && !name.contains(char::is_whitespace))
                    {
                        allowlist.libraries.push(name.to_owned());
                    }
                }
                None => {
                    let symbol = line.split_whitespace().next().unwrap_or(line);
                    allowlist.symbols.push(symbol.to_owned());
                }
            }
        }
        allowlist
    }

    /// Whether a `DT_NEEDED` entry is allowed: a system library, the
    /// loader, or `lib<name>.so[.*]` of one of [`Self::libraries`].
    fn library_allowed(&self, needed: &str) -> bool {
        if SYSTEM_LIBRARIES.contains(&needed)
            || (needed.starts_with("ld-linux") && needed.contains(".so."))
        {
            return true;
        }
        self.libraries.iter().any(|name| {
            needed
                .strip_prefix("lib")
                .and_then(|rest| rest.strip_prefix(name.as_str()))
                .is_some_and(|rest| rest == ".so" || rest.starts_with(".so."))
        })
    }
}

/// One line of `llvm-nm -D` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Symbol {
    pub(crate) name: String,
    /// The symbol version (`GLIBC_2.4` of `memcpy@GLIBC_2.4`), if any.
    pub(crate) version: Option<String>,
    /// nm's type letter: `T`, `D`, `B`, `W`, `V`, `U`, `w`, ...
    pub(crate) kind: char,
}

impl Symbol {
    /// Weak: `W`/`V` defined, `w`/`v` undefined.
    fn is_weak(&self) -> bool {
        matches!(self.kind, 'W' | 'V' | 'w' | 'v')
    }
}

/// Parses `llvm-nm -D` output (`[address] <type> <name>[@[@]version]`).
pub(crate) fn parse_nm(text: &str) -> Vec<Symbol> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace().rev();
            let name = fields.next()?;
            let kind = fields.next()?.chars().next()?;
            let (name, version) = match name.split_once('@') {
                Some((name, version)) => (name, Some(version.trim_start_matches('@').to_owned())),
                None => (name, None),
            };
            Some(Symbol {
                name: name.to_owned(),
                version,
                kind,
            })
        })
        .collect()
}

/// Parses `llvm-readobj --needed-libs` output: the names inside
/// `NeededLibraries [ ... ]`.
pub(crate) fn parse_needed(text: &str) -> Vec<String> {
    let mut inside = false;
    let mut needed = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with("NeededLibraries") {
            inside = true;
        } else if inside && line == "]" {
            break;
        } else if inside && !line.is_empty() {
            needed.push(line.to_owned());
        }
    }
    needed
}

/// What breaks the policy, by symbol or library name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Violation {
    /// The entry is not exported.
    MissingEntry(String),
    /// A global symbol other than the entry is exported.
    ExtraExport(String),
    /// A weak symbol is exported.
    WeakExport(String),
    /// An import neither in `symbols.allow` nor a system library's.
    UndocumentedImport(String),
    /// A `DT_NEEDED` library neither a system nor an allowed Tizen one.
    UnexpectedLibrary(String),
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::MissingEntry(name) => write!(f, "the entry `{name}` is not exported"),
            Violation::ExtraExport(name) => {
                write!(f, "`{name}` is exported; only the entry may be")
            }
            Violation::WeakExport(name) => write!(f, "`{name}` is a weak exported symbol"),
            Violation::UndocumentedImport(name) => write!(
                f,
                "`{name}` is imported but is neither in symbols.allow nor a glibc, libgcc_s or loader symbol"
            ),
            Violation::UnexpectedLibrary(name) => write!(
                f,
                "`{name}` is a needed library but neither glibc, libgcc_s, the loader nor a Tizen library of symbols.allow"
            ),
        }
    }
}

/// The violations of a library whose dynamic symbols are `defined` and
/// `undefined` (`llvm-nm -D` output) and whose needed libraries are
/// `needed` (`llvm-readobj --needed-libs` output), for the export `entry`.
pub(crate) fn check(
    defined: &str,
    undefined: &str,
    needed: &str,
    entry: &str,
    allowlist: &Allowlist,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    // `.dynsym` holds no local symbols: every defined one is an export,
    // whatever its letter (`i` for IFUNC, `u` for GNU-unique ones).
    let exports = parse_nm(defined);
    if !exports.iter().any(|s| s.name == entry && !s.is_weak()) {
        violations.push(Violation::MissingEntry(entry.to_owned()));
    }
    for symbol in &exports {
        if symbol.is_weak() {
            violations.push(Violation::WeakExport(symbol.name.clone()));
        } else if symbol.name != entry {
            violations.push(Violation::ExtraExport(symbol.name.clone()));
        }
    }
    for symbol in parse_nm(undefined) {
        let system = symbol
            .version
            .as_deref()
            .is_some_and(|v| SYSTEM_VERSION_PREFIXES.iter().any(|p| v.starts_with(p)));
        let loader = symbol.is_weak()
            && symbol.version.is_none()
            && LOADER_WEAK_IMPORTS.contains(&symbol.name.as_str());
        if !system && !loader && !allowlist.symbols.contains(&symbol.name) {
            violations.push(Violation::UndocumentedImport(symbol.name));
        }
    }
    for library in parse_needed(needed) {
        if !allowlist.library_allowed(&library) {
            violations.push(Violation::UnexpectedLibrary(library));
        }
    }
    violations
}

/// Checks the TV library `library` with the nightly's `llvm-nm` and
/// `llvm-readobj` (in `rustlib_bin`) against the allowlist at `allowlist`
/// (the component's `targets/symbols.allow`), failing with every
/// violation.
pub(crate) fn check_library(
    rustlib_bin: &Path,
    library: &Path,
    allowlist: &Path,
    entry: &str,
) -> anyhow::Result<()> {
    let allowlist = Allowlist::parse(&fs::read_to_string(allowlist).with_context(|| {
        format!(
            "read {} (a tizen component without the symbol allowlist: `nsh component remove tizen`, \
             then `nsh component add tizen`; in a checkout, `cargo xtask tizen`)",
            allowlist.display()
        )
    })?);
    let tool = |name: &str, args: &[&str]| -> anyhow::Result<String> {
        let program = rustlib_bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        let output = Command::new(&program)
            .args(args)
            .arg(library)
            .output()
            .with_context(|| {
                format!(
                    "run {} (the nightly's llvm-tools component)",
                    program.display()
                )
            })?;
        if !output.status.success() {
            bail!(
                "{name} failed on {}: {}",
                library.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let violations = check(
        &tool("llvm-nm", &["-D", "--defined-only"])?,
        &tool("llvm-nm", &["-D", "--undefined-only"])?,
        &tool("llvm-readobj", &["--needed-libs"])?,
        entry,
        &allowlist,
    );
    if !violations.is_empty() {
        let list: Vec<String> = violations.iter().map(|v| format!("  {v}")).collect();
        bail!(
            "{} breaks the TV library's symbol policy:\n{}",
            library.display(),
            list.join("\n")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOW: &str = "# Tizen symbols a TV library may import.\n\
        # One `<symbol> <module>` per line.\n\n\
        # libcapi-appfw-application.so\n\
        ui_app_main Application Framework > Application (appfw/app.h)\n\
        ui_app_exit Application Framework > Application (appfw/app.h)\n\n\
        # libecore_wl2.so\n\
        ecore_wl2_display_connect EFL > Ecore > Ecore_Wl2 (ecore-wl2-1/Ecore_Wl2.h)\n";

    const NEEDED: &str = "File: libnsh.so\nFormat: elf32-littlearm\nArch: arm\nAddressSize: 32bit\n\
        LoadName: <Not found>\nNeededLibraries [\n  ld-linux.so.3\n  libc.so.6\n  libcapi-appfw-application.so.0\n  \
        libecore_wl2.so.1\n  libgcc_s.so.1\n  libm.so.6\n]\n";

    fn allowlist() -> Allowlist {
        Allowlist::parse(ALLOW)
    }

    fn run(defined: &str, undefined: &str, needed: &str) -> Vec<Violation> {
        check(defined, undefined, needed, "nsh_tizen_main", &allowlist())
    }

    #[test]
    fn the_allowlist_names_its_symbols_and_their_libraries() {
        assert_eq!(
            allowlist(),
            Allowlist {
                symbols: vec![
                    "ui_app_main".into(),
                    "ui_app_exit".into(),
                    "ecore_wl2_display_connect".into()
                ],
                libraries: vec!["capi-appfw-application".into(), "ecore_wl2".into()],
            }
        );
    }

    #[test]
    fn nm_and_readobj_output_parse() {
        assert_eq!(
            parse_nm(
                "001b1b38 T nsh_tizen_main\n         U memcpy@GLIBC_2.4\n         w __gmon_start__\n\
                 00001000 i resolver\n00002000 D data@@VERS_1\n"
            ),
            [
                Symbol {
                    name: "nsh_tizen_main".into(),
                    version: None,
                    kind: 'T'
                },
                Symbol {
                    name: "memcpy".into(),
                    version: Some("GLIBC_2.4".into()),
                    kind: 'U'
                },
                Symbol {
                    name: "__gmon_start__".into(),
                    version: None,
                    kind: 'w'
                },
                Symbol {
                    name: "resolver".into(),
                    version: None,
                    kind: 'i'
                },
                Symbol {
                    name: "data".into(),
                    version: Some("VERS_1".into()),
                    kind: 'D'
                },
            ]
        );
        assert_eq!(
            parse_needed(NEEDED),
            [
                "ld-linux.so.3",
                "libc.so.6",
                "libcapi-appfw-application.so.0",
                "libecore_wl2.so.1",
                "libgcc_s.so.1",
                "libm.so.6"
            ]
        );
    }

    #[test]
    fn a_library_exporting_its_entry_alone_and_importing_what_is_allowed_passes() {
        let undefined = "         U ui_app_main\n         U ecore_wl2_display_connect\n\
                         U memcpy@GLIBC_2.4\n         U _Unwind_Resume@GCC_3.5\n\
                         w __gmon_start__\n         w __clock_gettime64\n";
        assert_eq!(run("001b1b38 T nsh_tizen_main\n", undefined, NEEDED), []);
    }

    #[test]
    fn another_export_a_weak_one_or_no_entry_fail_by_name() {
        assert_eq!(
            run(
                "001b1b38 T nsh_tizen_main\n00002000 T nsh_app_ui_sets\n00003000 W weakling\n",
                "",
                ""
            ),
            [
                Violation::ExtraExport("nsh_app_ui_sets".into()),
                Violation::WeakExport("weakling".into()),
            ]
        );
        assert_eq!(
            run("00003000 W nsh_tizen_main\n", "", ""),
            [
                Violation::MissingEntry("nsh_tizen_main".into()),
                Violation::WeakExport("nsh_tizen_main".into()),
            ]
        );
        assert_eq!(
            run("", "", ""),
            [Violation::MissingEntry("nsh_tizen_main".into())]
        );
    }

    #[test]
    fn imports_and_libraries_outside_the_policy_fail_by_name() {
        let undefined = "         U tbm_surface_create\n         U elm_init@ELM_1\n\
                         w pidfd_spawnp@GLIBC_2.35\n         w __clock_gettime64\n\
                         U __clock_gettime64\n";
        let needed = "NeededLibraries [\n  libc.so.6\n  libtbm.so.1\n  libecore_wl2-extra.so\n  libdlog.so.0\n]\n";
        assert_eq!(
            run("001b1b38 T nsh_tizen_main\n", undefined, needed),
            [
                Violation::UndocumentedImport("tbm_surface_create".into()),
                Violation::UndocumentedImport("elm_init".into()),
                // A loader import must be weak.
                Violation::UndocumentedImport("__clock_gettime64".into()),
                Violation::UnexpectedLibrary("libtbm.so.1".into()),
                Violation::UnexpectedLibrary("libecore_wl2-extra.so".into()),
                Violation::UnexpectedLibrary("libdlog.so.0".into()),
            ]
        );
        assert_eq!(
            Violation::ExtraExport("nsh_app_root".into()).to_string(),
            "`nsh_app_root` is exported; only the entry may be"
        );
    }
}
