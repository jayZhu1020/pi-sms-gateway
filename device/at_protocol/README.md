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

## Sending a command

`AtCommandWriter` owns a nonblocking `Write` transport while sending one command.
Construct it with the transport, printable ASCII command (starting with `AT`,
maximum 4096 bytes, no CR/LF) and a monotonic deadline. It appends the shared CR
constant automatically. Invalid input returns an IO error and drops the supplied
transport. This checks framing only, not whether a command is supported or safe.

Call `poll(Instant::now())` once per scheduling turn. It attempts one write,
returns false for incomplete progress, and returns true after all bytes are
accepted. Partial writes resume without repeating bytes. WouldBlock/Interrupted
leave it ready to retry; zero progress from a successful write, other IO errors,
or expiration permanently stop it. Continue polling while idle to enforce timeout.
The caller must supply nonblocking IO; this does not interrupt a blocking write.

After success, `finish()` returns the owned transport for reply handling.
It checks completion without writing any remaining bytes.
Before completion or after failure it returns the command writer instead. Drop
failed writers and recover the modem connection before another command; merely
reopening a device does not prove that stale replies are gone. Successful writing
is not a modem acknowledgment or UART-drain guarantee. No automatic retry of the
whole command, parser integration or recovery is included yet.

## Reading a diagnostic reply

`AtReplyReader` owns a nonblocking `Read` transport after a command has been sent.
Construct it with the sent command, expected reply prefix and deadline, on a
connection already synchronized by the caller. It combines `AtLineDecoder` with
`AtResponse`; the prefix determines which lines are data versus notifications.
The constructor returns errors for invalid prefixes and drops the supplied transport.

`poll(now)` attempts one byte read, returning `Some(ResponseEvent)` when a line
completes or `None` for partial/blank lines or temporary IO unavailability.
Poll regularly with current monotonic time even during silence to enforce timeout.
Echo, Data and Notification events preserve arrival order; Complete stops reading.
The caller must process notifications rather than discard them. Nothing accumulates
beyond the decoder's bounded line. IO errors, EOF, malformed input, modem rejection
or timeout permanently stop the reader; subsequent polls return Closed.

`finish()` returns the transport only after Complete; otherwise it returns the
reader without further IO. It deliberately reads one byte at a time so bytes after
OK remain on the transport for the next owner, including unsolicited notifications.
This trades throughput for simple ownership with no unread buffer to lose. It is a
diagnostic-command step, not a bulk-data reader or complete session implementation.
There is no recovery: a fresh reader on a stale connection can still mistake a late
OK for the current reply. Closing/reopening alone does not prove synchronization.

## One complete diagnostic exchange

`AtExchange` owns a synchronized, nonblocking `Read + Write` transport from sending
one command through receiving its final reply. Construct it with the command,
expected data prefix and a single deadline; both command framing and an empty
prefix are checked before IO. Invalid input drops the supplied transport.

Call `poll(Instant::now())` repeatedly. It first advances `AtCommandWriter` until
all command bytes are accepted, then transfers ownership to `AtReplyReader`.
Writing and reading share the original deadline; entering the reading phase does
not grant extra time. Each poll performs at most one IO operation and returns
None or an ordered response event. Process Data and Notification events as they
arrive. Stop polling after Complete and call `finish()` to recover the transport.
An incomplete finish returns the exchange without doing more IO.

On write/read/decode/response failure, the exchange drops its transport and cannot
be reused. An owned SerialPort attempts settings restoration as it is dropped;
this does not reset or resynchronize the modem. The exchange neither retries a
whole command nor opens a fresh connection. A synchronized starting connection
remains a caller precondition, so this is not yet an unattended modem session.
