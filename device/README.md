# Device components

`SerialPort` in `serial_port.rs` owns an open Linux serial device. It takes an
advisory lock, saves its settings, and uses [TerminalOptions](os/README.md) to
configure raw 115200 baud, eight data bits, no parity, one stop bit, and no echo
or flow control. All stty interaction remains inside the OS library.

`SerialPort::open(path)` returns a port or an IO error. Standard `Read`/`Write`
traits exchange raw bytes: idle reads can return `WouldBlock`, writes can be
partial, and callers supply retry/deadline handling. `flush` does not guarantee
that the UART has finished transmitting. AT parsing is independent of transport.

`close()` consumes the owner and reports settings-restoration errors. Dropping
it also attempts restoration, but cannot report failures. Settings are restored
before the file closes and releases its lock. This also handles configuration
failure after capture. Forced process termination/device removal can prevent
restoration. The advisory lock does not exclude programs that ignore it.

On Linux with Bazelisk and `/bin/stty`:

```sh
bazel test //device:serial_port_integration_test --test_output=all
```

`serial_port_integration_test.rs` is a separate test crate using only public APIs.
Its `SimulatedTerminal` fixture owns a paired OS pseudoterminal: SerialPort opens
`device_path`, while `send_to_port` and `expect_from_port` act as the peer.
Setup and descriptor ownership stay inside the fixture; dropping it closes both
ends. Two tests check competing opens, nonblocking idle
reads, byte transfers in both directions, close/drop restoration, reopening, and
invalid-device rejection. They exercise the real terminal-options library and
stty, without a modem or AT parser. Settings snapshots compare exactly. Fixtures
close automatically. macOS wildcard builds skip this Linux-only target; Ubuntu
CI executes it. Cross-compilation on Mac checks compilation only.
