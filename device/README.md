# Device interface

`SimDeviceInterface` is the Rust boundary for read-only SMS intake. Modem adapters
own transport and AT commands; consumers receive health, capability flags, opaque
storage handles and raw SMS-DELIVER PDU bytes. A handle is not a durable message ID.
An adapter must reject stale handles, including when a modem reuses a storage slot.
The trait does not decode, persist, upload or delete messages yet.

## Validate on a development machine

Install Bazelisk; macOS needs Command Line Tools, Linux needs a C/C++ toolchain.
Run `bazelisk test //device:interface_test --test_output=all` from the repository root.
Bazel downloads the pinned Bazel/Rust versions on first use; network access is needed.
Expect three passing Rust unit tests covering non-mutating reads via a trait object,
unsupported/disconnected failures and stale-handle rejection. Fixtures use arbitrary
bytes to test transport independence; they do not validate PDU decoding or hardware.
No Pi, credentials, cellular operations or cleanup are required.

Bazel output and its generated resolution lock stay local in this first slice;
compiler/direct rule versions are pinned, but the transitive graph is not locked in Git.
Real SIM7600 access and Linux ARM64 packaging require subsequent implementations.
