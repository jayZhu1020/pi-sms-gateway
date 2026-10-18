# Device components

`SerialPort` in `serial_port.rs` owns an open Linux serial device. It takes an
advisory lock, saves its settings, and uses [TerminalOptions](os/README.md) to
configure raw mode, eight data bits, no parity, one stop bit, and no echo
or flow control, while preserving the current baud rate. All stty interaction remains inside the OS library.

`SerialPort::open(path)` returns a port or an IO error. USB modem drivers such as
Linux `option` do not support baud changes: requesting 115200 can make stty fail
even though the USB connection works. SerialPort leaves baud unchanged; failed
setup still restores the saved settings.

Standard `Read`/`Write`
traits exchange raw bytes: idle reads can return `WouldBlock`, writes can be
partial, and callers supply retry/deadline handling. `flush` does not guarantee
that the UART has finished transmitting. AT parsing is independent of transport.

`close()` consumes the owner and reports settings-restoration errors. Dropping
it also attempts restoration, but cannot report failures. Settings are restored
before the file closes and releases its lock. This also handles configuration
failure after capture. Forced process termination/device removal can prevent
restoration. The advisory lock does not exclude programs that ignore it.

## Diagnosing USB modem replies

SerialPort opens with `O_NOCTTY`, so the modem cannot become the process's
controlling terminal. Keep this flag when adapting the transport.

A shell experiment that uses `exec 3<>/dev/ttyUSB2` can acquire that terminal.
Running `timeout 3 cat <&3` then creates a separate process group; Linux can stop
the reader with `SIGTTIN`. A zero-byte count in that pipeline does not establish
that the modem sent nothing. Use `timeout --foreground 3 cat <&3` for such a
bounded diagnostic read. Timeout expiry is expected when cat keeps waiting after
a response. Save/restore settings, use a verified AT port with no concurrent
client, and do not log arbitrary modem input.

On a USB modem, the reported baud value is not the USB transfer rate. Preserve it
while applying raw mode. A modem reply such as `+CME ERROR: SIM failure` proves
that a command reached the modem but does not establish SIM readiness. A
SIM7600 readiness probe must still report that rejection as failure.
