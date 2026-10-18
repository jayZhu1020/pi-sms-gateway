# Building for Raspberry Pi 5

Run on a development machine with Bazelisk (available as `bazel`). The first
build downloads the pinned Rust and Zig toolchains. Docker and a compiler on
the Pi are unnecessary. On macOS, install Xcode Command Line Tools for the
small programs Bazel builds and runs on the Mac itself.

Use a Pi running a 64-bit Linux OS. Build an application target with the Pi
configuration, then copy the executable to the Pi:

```sh
# Requires the separate sim_probe change to be merged too.
bazel build --config=pi //tool:sim_probe
scp bazel-bin/tool/sim_probe USER@PI_HOST:/tmp/sim_probe
ssh USER@PI_HOST '/tmp/sim_probe --help'
```

The executable links the Rust runtime and musl libc statically, so there is
no shared-library bundle to copy and no dependency on the Pi's glibc version.
This does not package external programs, data, or permissions: the serial
library still needs `/bin/stty` and access to the serial device. Remove the
copied probe with `ssh USER@PI_HOST 'rm /tmp/sim_probe'` when finished.

Without `--config=pi`, Bazel builds for the development machine. Cross-built
executables require ARM64 Linux; do not run `bazel run --config=pi` on a Mac or
x86 Linux host.

## How linking works

`MODULE.bazel` requests Rust's `aarch64-unknown-linux-musl` standard library.
The `pi` configuration selects an ARM64 Linux platform and Zig's native
cross-toolchain. Rust's bundled `rust-lld` links Rust executables using its
self-contained musl runtime; Zig supplies the C/C++ toolchain required by
Bazel's Rust rules. Additional native dependencies must support the selected target.

The two-line `rules_rust_musl_linker.patch` makes generated ARM64 musl Rust
toolchains prefer `rust-lld`. In rules_rust 0.74.0 this cannot be selected per
target triple through the module extension. Setting the global linker flag
also changes host-tool linking, while Zig's compiler driver rejects Rust's
`--fix-cortex-a53-843419` linker argument. The patch preserves that argument
and leaves host linking unchanged. Reassess this patch when updating
rules_rust; it is applied to the pinned dependency, not the installed SDK.
