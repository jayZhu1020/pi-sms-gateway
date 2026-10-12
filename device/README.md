# Device components

## Serial port ownership

`SerialPort` in `serial_port.rs` opens a device path with nonblocking reads/writes,
locks it against other clients that also use `flock`, and configures 115200 baud,
8 data bits, no parity, one stop bit, raw bytes and no echo/flow control.
It uses `/bin/stty` on the already-open descriptor; no shell or second device-path
lookup is involved. Missing paths and devices without terminal support are errors.

Use the `Read`/`Write` traits to exchange bytes. Reads may return `WouldBlock`, and
writes may be partial; callers supply retry/deadline logic. `Write::flush` delegates
to the file and is not a guarantee that the UART has finished transmitting.
`close()` reports restoration errors; dropping makes a best-effort restoration.
Neither can guarantee restoration after a forced process kill or device disconnect.
The lock is advisory and does not stop programs that ignore it. Byte decoding,
reply tracking, timeout recovery and hardware discovery remain separate layers.

The serial integration tests use an OS pseudoterminal, not a modem. They check
exclusive ownership, raw reads/writes, idle behavior, restoration/reopening and
invalid-device rejection on Linux/macOS. The macOS snapshot comparison ignores
its transient PENDIN bookkeeping flag; Linux compares the full saved settings.
