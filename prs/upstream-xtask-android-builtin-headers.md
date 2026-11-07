# Upstream PR: xtask: Search the NDK's clang headers after bindgen's own

**Not to be opened: upstream fixed it itself.** huacnlee's #1189 (`beeb9fc77`, merged 2026-11-07 05:02 UTC, in
`updater: Add OTA updates of a packaged app's code through signed packages`) makes the same change in
`crates/xtask/src/android.rs` (`-idirafter` for the NDK's clang headers, with a comment naming `uint32_t`). It has no
test of its own. Checked against `upstream/main` `c015f2c5e` (2026-11-07). The fork's version remains below the line
as a record.

- Branch: `fix/xtask-android-builtin-headers`, one commit (`62145340b`) on `upstream/main` `7571948ee` (#1069).
- Fork PR: dXnSix/nsh#155, merged 2026-11-07 (`baaafe268`), in tag `fork-0.4.0-1` (`1d64f049d`).
- Found by: Fork CI's `android` job on the sync to `7571948ee` (dXnSix/nsh#152, run
  https://github.com/dXnSix/nsh/actions/runs/42311010274): `cargo xtask clippy-android` with NDK 27.3 failed
  `quickjs-jit-sys`'s bindings (`sysroot/usr/include/bits/pthread_types.h:35:3: error: unknown type name 'uint32_t'`).
  Upstream's own workflows do not build Android, so its CI does not run this.
- Checked: Fork CI's `android` job passed on the fork PR (https://github.com/dXnSix/nsh/actions/runs/42313540410) and
  on the fork's `main` at `1d64f049d` with all four sync fixes
  (https://github.com/dXnSix/nsh/actions/runs/42321466305). Locally: `cargo test -p xtask --bin xtask android`
  (3 passed), `cargo clippy -p xtask --all-targets -- -D warnings`, and `cargo xtask clippy-android` with NDK 27.2 and
  Homebrew's LLVM 23 (fails before, passes after).
- Next sync: `crates/xtask/src/android.rs` conflicts with `beeb9fc77` (`git merge-tree` of the fork's `main` with
  `upstream/main` `c015f2c5e`). Take upstream's lines. The fork's `bindgen_clang_args` and its test could later go
  upstream as a test-only PR, if the owner wants one.
- Title: `xtask: Search the NDK's clang headers after bindgen's own`.

---

## Description

#1071 added the NDK's clang headers (`toolchains/llvm/prebuilt/*/lib/clang/<v>/include`) to
`BINDGEN_EXTRA_CLANG_ARGS` with `-isystem`, for a libclang that has no headers of its own. bindgen's
`detect_include_paths` runs a clang with those arguments and also adds each directory it reports as `-isystem`. That
puts the NDK's copy of clang's `stdint.h` first. A libclang that has its own headers (Ubuntu's clang 18 on the Linux
runners, Homebrew's LLVM) then sees two copies with one include guard (`__CLANG_STDINT_H`). The second copy is
skipped, the sysroot's `stdint.h` is never read, and `quickjs-jit-sys`'s bindings fail:

```
sysroot/usr/include/bits/pthread_types.h:35:3: error: unknown type name 'uint32_t'
Unable to generate bindings: ClangDiagnostic(...)
```

`cargo xtask clippy-android`, `xtask android` and `xtask dist` failed this way on a Linux runner with NDK 27, and on
macOS with Homebrew's LLVM 23 and NDK 27.2.

`-idirafter` still gives a libclang without its own headers the NDK's copies (`stdbool.h` and the rest). It searches
them after the sysroot and the libclang's own headers. The arguments are now built in `bindgen_clang_args`, which has
a test.

## How to Test

- `cargo test -p xtask --bin xtask android`: `bindgen_searches_the_ndk_clang_headers_after_its_own` checks the
  arguments (`-idirafter`, no `-isystem`) for a toolchain with a `lib/clang/<v>/include`.
- `ANDROID_NDK_HOME=<NDK 27> cargo xtask clippy-android` with a system or Homebrew libclang: on `main`,
  `quickjs-jit-sys`'s build script fails with the error above; with this change the bindings are generated.
- `clang --target=aarch64-linux-android28 --sysroot=<sysroot>` on a file that includes `stdint.h`, `stdbool.h`,
  `stddef.h`, `stdarg.h` and `pthread.h`: with the NDK's headers as `-isystem`, 21 errors; as `-idirafter`, none. With
  `-nobuiltininc` (a libclang without its own headers) there are none either.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` and `cargo clippy -p xtask --all-targets -- -D warnings`.
- [x] Changes to what apps use: none (the Android build's bindgen arguments).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: a build fix; Android builds again.
