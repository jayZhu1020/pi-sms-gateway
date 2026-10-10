# Pi SMS gateway

The first product receives SMS on a Raspberry Pi/Waveshare modem and forwards
messages through a cloud service into an iOS app. Sending SMS and calls come later.

## How the Pi talks to the modem: AT commands

AT commands are short text instructions sent over a serial connection. The modem
handles the cellular network; the Pi asks it questions and reads its replies.
`AT` comes from “attention.” USB exposes serial ports, and the modem adapter will
select the correct control port. This protocol is separate from our cloud API.

For example, the Pi can ask whether the SIM is ready:

```text
Pi sends:     AT+CPIN? followed by a carriage-return byte
Modem replies:
+CPIN: READY
OK
```

`+CPIN: READY` is the answer; `OK` marks successful completion of this command.
A failure may end with `ERROR`, `+CME ERROR: ...` or `+CMS ERROR: ...`.
A modem can also echo the command back; that echo is not its answer.
These examples assume text result codes rather than numeric result codes.

Common command forms, when supported by that command:

| Form | Meaning | Example |
|---|---|---|
| `AT` | Check that the modem responds | `AT` |
| `AT+NAME?` | Read a setting or status | `AT+CPIN?` |
| `AT+NAME=?` | Ask which values are supported | `AT+CMGF=?` |
| `AT+NAME=value` | Change a setting | `AT+CMGF=0` selects SMS PDU mode |

The forms vary by command: for example, the signal query is `AT+CSQ`.
Changing settings is an operation, not just a diagnostic query.

## Messages the modem sends without being asked

The modem can report an event while the Pi is waiting for another reply.
These are called unsolicited notifications, or URCs (unsolicited result codes).
For example, with stored-message notifications configured:

```text
+CMTI: "SM",7
```

This announces a message stored at index 7 in SIM storage. It is not the SMS body.
The adapter reads that stored message separately. SMS can also use other storage
locations and notification modes; do not assume every firmware is configured alike.
The future session layer must distinguish these notifications from command replies.

## Why we need a line decoder

A serial read returns whatever bytes are available, not necessarily a whole line.
One read might contain `+CP`, and the next `IN: READY\r\nOK\r\n`.
The decoder joins them into `+CPIN: READY` and `OK`, in that order.

`\r` is carriage return (CR, byte 13); `\n` is line feed (LF, byte 10).
CRLF means those two bytes together. Our decoder also accepts LF alone and extra
CR bytes before LF. It ignores empty lines, but preserves nonempty notification
lines. These are decoder choices, not a claim that all modem formats are supported.

The decoder stops after invalid input or a line longer than 4096 bytes. After that,
the session layer must stop the current exchange and restore a known connection
state before creating another decoder. Simply clearing a buffer is insufficient:
an old command's delayed `OK` could otherwise be mistaken for a new command's reply.
Actual timeout and recovery handling are not implemented yet.

## SMS PDU mode and our Rust boundary

PDU mode represents an SMS as encoded bytes, including its address, text encoding
and possibly multipart information. The serial representation is hexadecimal ASCII.
Later layers convert it to bytes, decode the message, store it and upload it.
The AT line decoder is intended for ASCII control/PDU traffic, not arbitrary Unicode
SMS bodies in text mode or binary/audio streams. Some commands also use prompts
without a newline; a line decoder alone cannot handle those exchanges.

`device/lib.rs` defines the abstract `SimDeviceInterface`; modem-specific commands
belong inside an adapter. The serial decoder is being developed separately.
Raw modem input can contain private numbers and messages, so it should not be logged.

Protocol reference: [SIMCom SIM7500/SIM7600 AT command manual](https://files.waveshare.com/upload/6/68/SIM7500_SIM7600_Series_AT_Command_Manual_V2.00.pdf).
Consult the manual and actual firmware before configuring or executing commands.

## Automated checks

GitHub Actions runs on PR updates targeting `main`, every push to `main`, and
manual runs. It checks Rust source registration, formatting, Clippy, builds and
unit tests. No Pi or credentials are needed. A tracked `.rs` file omitted from
Bazel `srcs` fails the coverage check instead of silently escaping checks.

With Bazelisk installed (and `bazel` available), run the same checks locally:

```sh
python3 .github/scripts/check_rust_sources.py
bazel build --config=fmt //...
bazel build --config=lint //...
bazel build //...
bazel test //... --test_output=errors
```

Automatically format registered Rust sources with `bazel run @rules_rust//:rustfmt`.
CI reports formatting/lint failures; it does not rewrite files or push commits.
Future Swift/Python code needs its own language checks. Add the `Rust checks`
status to a GitHub branch ruleset to require it before merging.
