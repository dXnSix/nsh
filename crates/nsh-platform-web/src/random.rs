//! getrandom's entropy on the web: the page's Web Crypto
//! (`crypto.getRandomValues`), which `web/nsh.js` hands the module as
//! `env.__nsh_random_fill`.
//!
//! Every web build selects getrandom's custom backend
//! (`getrandom_backend="custom"`, `.cargo/config.toml` and the CLI's
//! `runtime::web_build_flags`), so a crate of the app that uses getrandom
//! 0.3 or 0.4 (directly, or through `rand`, `uuid`, a CRDT) links against
//! the symbol defined here. The module has no JS interpreter, so nsh-js's
//! definition is not in it.

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn __nsh_random_fill(ptr: *mut u8, len: usize) -> i32;
}

// getrandom's custom backend is shared by its 0.3 and 0.4 releases. Only
// success is relied on: browser entropy failure becomes a generic error.
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    ptr: *mut u8,
    len: usize,
) -> Result<(), getrandom_stdlib::Error> {
    // SAFETY: getrandom provides a writable range of exactly `len` bytes;
    // the page fills that range synchronously before returning.
    if unsafe { __nsh_random_fill(ptr, len) } == 0 {
        Ok(())
    } else {
        Err(getrandom_stdlib::Error::UNSUPPORTED)
    }
}
