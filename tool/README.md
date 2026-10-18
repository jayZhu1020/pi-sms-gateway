# SIM7600 read-only probe

`sim_probe` queries SIM PIN readiness once. When the SIM is READY, it then reads
circuit-switched registration with `AT+CREG?` and LTE/EPS registration with
`AT+CEREG?` on the same serial connection. It never supplies a PIN/PUK,
changes registration reporting or network settings, sends/deletes SMS, or places
calls. Modem commands and response validation live in `device/sim7600_probe.rs`, not in the CLI. This is a narrow
adapter operation, not yet a complete implementation of `SimDeviceInterface`.

## Build on a development machine or CI

On a Linux development machine with Bazelisk and a native toolchain:

```sh
bazel build //tool:sim_probe
bazel-bin/tool/sim_probe --help
```

The executable is Linux-only. A default
x86-64 Linux build cannot run on the Pi's ARM64 CPU. Build off-device with the Pi
configuration, then copy the executable to the Pi:

```sh
bazel build --config=pi //tool:sim_probe
install -m 755 bazel-bin/tool/sim_probe /tmp/sim_probe
scp /tmp/sim_probe USER@PI_HOST:/tmp/sim_probe
ssh USER@PI_HOST '/tmp/sim_probe --help'
```

The Pi requires `/bin/stty` and serial-device permissions. The updater remains
future work. Remove the temporary executable when finished.

## Run an ARM64 executable after installation

```sh
./sim_probe --help
./sim_probe /dev/ttyUSB2
```

`/dev/ttyUSB2` is an example: use the verified AT port or its stable by-id path,
not a guessed GPS/diagnostic port. The user needs read/write device access.
The runtime needs `/bin/stty`. SerialPort configures raw 8N1, takes an
advisory lock and restores terminal settings on close (best effort on errors).

Start with a synchronized modem connection and no other AT client. This
probe does not discover ports, drain old traffic, reset the modem or recover an
interrupted prior command. Restart/reinitialize the modem through your known
hardware procedure if the connection state is uncertain; reopening the port alone
is not synchronization. Do not automatically retry failed invocations.

Example output:

```text
SIM status: READY
Circuit-switched registration: registered (roaming)
LTE/EPS registration: registered (roaming)
Notifications observed: 0
```

Supported SIM states are READY, SIM PIN and SIM PUK. Exit 0 means the snapshot
queries and terminal restoration succeeded, including a locked SIM or a network
that is not registered; read the status values. Network queries are skipped for a locked SIM.
Exit 1 means argument/open/exchange/validation/restoration failure. An OK without
status, unknown status or duplicate status is an error. A missing SIM may produce
a modem error. READY establishes SIM readiness. CREG and CEREG are separate
circuit-switched and EPS registration observations; neither proves SMS reception. The snapshot
reports searching, denied, unknown and not-registered states without treating them
as transport failures. EPS also distinguishes SMS-only and emergency-only states.

The SIM exchange has a five-second deadline; the two network exchanges share a
separate five-second deadline. Opening/restoring the terminal and stty execution
are outside those deadlines. Polling occurs at millisecond intervals. Unrelated
notifications seen before OK are counted, but their contents are not printed or
stored. Location/cell fields in registration replies are discarded. Missing,
duplicate, unsupported or malformed replies fail the snapshot. Ambiguous
same-prefix registration notifications also fail rather than guessing which
status belongs to the query. There is no automatic retry. This is a diagnostic
tool, not a persistent notification consumer.

Protocol reference: [SIMCom AT manual, CPIN/CREG/CEREG sections](https://files.waveshare.com/wiki/SIM7600G-H/SIM7500_SIM7600_Series_AT_Command_Manual_V3.00.pdf).
