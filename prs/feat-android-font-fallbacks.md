# android: Fall back along fontFamily lists

Branch: `feat/android-font-fallbacks` on `github.com/dXnSix/nsh`, stacked on `feat/font-fallbacks` (`text: Name the families
of fontFamily lists`, #82). One commit, `61d9331a`. Opened as a pull request into the fork's `main`; once the stack
below it merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

Android read a `fontFamily` list as one family name, which no typeface has: text was set in the default typeface.

### Changes

- **`TextLayouts.java`**: `resolve` takes the list: the first family Android has (registered, generic or installed; `Typeface.create` gives the default typeface for one it does not have) is the typeface. When that one is a registered font, the registered fonts after it in the list are its fallbacks for the characters it lacks, then the system's (`Typeface.CustomFallbackBuilder` with the fonts' files, Android 10 and later).

## Public API

None: `fontFamily` lists (#82) work on Android.

## Tests

- `javac --release 17` against the Android 36 SDK: compiles.
- Not run on a device: Fork CI's Android job runs the gallery.
