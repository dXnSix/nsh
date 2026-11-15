//! Tells the crates that depend on nsh-core directly whether the dev tools'
//! hooks are compiled in (feature `inspect`): their build scripts read
//! `DEP_NSH_CORE_INSPECT`, set to `1` with it and absent without it. A crate
//! that calls the hooks only in a dev build switches on it, since only a
//! platform's `dev` feature turns `inspect` on, whatever the profile.
//! Nothing here reaches the library.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_INSPECT").is_some() {
        println!("cargo::metadata=inspect=1");
    }
}
