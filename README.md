# adb-rs

Android Debug Bridge (adb) client library.

For usage sample, see `adb-cli` crate.

## Limitations

- TCP transport only.
- Only `adb shell` (no interactive) and `adb push` are implemented.
