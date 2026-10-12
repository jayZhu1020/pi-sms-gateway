# Hayes AT protocol

AT is a text command language for controlling modems; AT means “attention.”
Modern cellular modems extend the Hayes command set with commands such as
`AT+CPIN?`. This folder implements a limited ASCII diagnostic-reply parser,
not every Hayes mode, binary stream, prompt, or SMS text encoding.

## From raw bytes to lines

Under typical default settings, the host ends a command with CR (`\r`, byte 13).
Text replies usually separate lines with CR followed by LF (`\n`, byte 10).
CR historically returns a cursor to the start; LF moves down a line. Our parser
uses them as separators; it never moves a cursor or overwrites buffered text.
A read may split anywhere, even between CR and LF, or contain several lines.

Detailed byte parsing is implemented by `AtLineDecoder` in `at_decoder.rs`.
It keeps partial lines between calls, accepts CRLF or LF-only endings (including
extra CR before LF), and strips those endings. It skips empty lines, preserves
text/notification order, accepts printable ASCII plus tabs, and caps text at
4096 bytes per line. These are our supported parsing rules, not universal AT rules.

| Bytes received (escapes denote actual control bytes) | Decoder result |
|---|---|
| `OK\r\n` | Valid: returns `OK` |
| `OK\n` or `OK\r\r\n` | Valid under our rules: returns `OK` |
| `+CP` then `IN: READY\r` then `\n` | Valid fragments: returns one `+CPIN: READY` line |
| `\r\nOK\r\n` | Valid: skips the blank line, returns `OK` |
| `OK\r` without more input | Incomplete: waits for LF; not an error yet |
| `OK\rX` | Invalid: text after CR without LF |
| NUL (`0x00`) or a byte `0xff` | Invalid for this ASCII decoder |
| 4097 text bytes before LF | Invalid: exceeds the line limit |

Invalid input stops the decoder; later calls return `Failed`. The caller must
recover the serial connection before replacing it. Decoder recovery does not
itself flush a port. Handling Unicode text mode or binary traffic needs another path.

## Command and reply exchange

With the default CR terminator, `AT+CPIN?\r` is a complete request;
`AT+CPIN?` alone is incomplete. An unsupported command may return `ERROR`: a
valid failure exchange, not malformed bytes. With echo and text results enabled:

```text
Host -> modem: AT+CPIN?\r
Modem -> host: AT+CPIN?\r\n       (echo; optional)
Modem -> host: +CPIN: READY\r\n   (reply data)
Modem -> host: +CMTI: "SM",7\r\n  (unrequested event; may be interleaved)
Modem -> host: OK\r\n            (command finished)
```

Detailed line classification is implemented by `AtResponse` in `at_response.rs`.
Give it the sent command, expected reply prefix and deadline; pass complete strings
without CR/LF. It returns events rather than sending commands or reading a port.

| Complete line / operation | Result for `AT+CPIN?`, prefix `+CPIN:` |
|---|---|
| `AT+CPIN?` | Valid echo: `Echo` |
| `+CPIN: READY` | Matching reply: `Data`; its value still needs adapter validation |
| `+CMTI: "SM",7` | Unrequested event: `Notification`; not command completion |
| `OK` before deadline | Successful completion: `Complete` |
| `ERROR`, `+CME ERROR: 10`, `+CMS ERROR: 500` | Valid failure replies: `Rejected` |
| Empty expected prefix at construction | Invalid configuration: `InvalidPrefix` |
| A line at/after deadline | Too late: `TimedOut`, even if it is `OK` |
| Another line after completion/error/timeout | Invalid reuse: `Closed` |

Unknown lines become notifications; the matcher does not prove they are valid
modem events. Numeric result codes, multiline payloads, prompts and prefixes shared
by replies/events need additional parsing. Poll `check_deadline` during silence.
After timeout, recover the port before a new command: a delayed old `OK` carries
no command ID, so a fresh tracker alone cannot tell which command it belongs to.

Reference: [SIMCom AT manual](https://files.waveshare.com/upload/6/68/SIM7500_SIM7600_Series_AT_Command_Manual_V2.00.pdf), sections 1.4, ATE, ATV and CPIN.

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
